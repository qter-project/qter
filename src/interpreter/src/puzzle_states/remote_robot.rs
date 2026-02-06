use std::{io::Error, sync::Arc};

use log::trace;
use puzzle_theory::permutations::{Algorithm, Permutation, PermutationGroup};
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader,
};

use crate::puzzle_states::RobotLike;

pub trait Connection {
    type Reader: AsyncBufRead + Unpin + ?Sized;
    type Writer: AsyncWrite + Unpin + ?Sized;

    fn reader(&mut self) -> &mut Self::Reader;
    fn writer(&mut self) -> &mut Self::Writer;
}

impl<R: AsyncBufRead + Unpin, W: AsyncWrite + Unpin> Connection for (R, W) {
    type Reader = R;
    type Writer = W;

    fn reader(&mut self) -> &mut Self::Reader {
        &mut self.0
    }

    fn writer(&mut self) -> &mut Self::Writer {
        &mut self.1
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> Connection for BufReader<T> {
    type Reader = Self;
    type Writer = T;

    fn reader(&mut self) -> &mut Self::Reader {
        self
    }

    fn writer(&mut self) -> &mut Self::Writer {
        self.get_mut()
    }
}

pub struct RemoteRobot<C: Connection> {
    conn: C,
    current_state: Option<Permutation>,
}

async fn ack_or_err<C: Connection>(conn: &mut C) -> Result<(), Error> {
    let reader = conn.reader();

    let mut which = [0; 5];
    reader.read_exact(&mut which).await?;

    if which == *b"!ACK\n" {
        Ok(())
    } else if which == *b"!ERR\n" {
        let mut len_be = [0; 2];
        reader.read_exact(&mut len_be).await?;
        let len = u16::from_be_bytes(len_be) as usize;

        let mut message = Box::from(vec![0; len]);
        reader.read_exact(&mut message).await?;

        // Refactor once https://github.com/rust-lang/rust/issues/129436 is stable
        Err(Error::other(String::from_utf8_lossy(&message)))
    } else {
        Err(Error::other("Server did not correctly acknowledge command"))
    }
}

impl<C: Connection> RobotLike for RemoteRobot<C> {
    type InitializationArg = C;
    type Error = Error;

    async fn initialize(group: Arc<PermutationGroup>, mut conn: C) -> Result<Self, Self::Error> {
        let mut encoded = serde_json::to_vec(&*group).map_err(|e| Error::other(e.to_string()))?;
        encoded.push(b'\n');

        let Ok(len) = u16::try_from(encoded.len()) else {
            return Err(Error::other(format!(
                "Cannot send a group with such a large encoding to the server ({} > 65535)",
                encoded.len()
            )));
        };

        let len = len.to_be_bytes();

        conn.writer().write_all(&len).await?;
        conn.writer().write_all(&encoded).await?;

        ack_or_err(&mut conn).await?;

        Ok(RemoteRobot {
            conn,
            current_state: None,
        })
    }

    async fn compose_into(&mut self, alg: &Algorithm) -> Result<(), Self::Error> {
        self.current_state = None;
        let writer = self.conn.writer();
        writer
            .write_all(
                alg.move_seq_iter()
                    .map(|v| &**v)
                    .collect::<Vec<_>>()
                    .join(" ")
                    .as_bytes(),
            )
            .await?;
        writer.write_all("\n".as_bytes()).await?;
        writer.flush().await?;
        ack_or_err(&mut self.conn).await
    }

    async fn take_picture(&mut self) -> Result<&Permutation, Self::Error> {
        // Note that I can't check for `Some` and return early because the borrow checker isn't smart enough to recognize that that is okay
        if self.current_state.is_none() {
            let writer = self.conn.writer();
            writer.write_all(b"!PICTURE\n").await?;
            writer.flush().await?;

            ack_or_err(&mut self.conn).await?;

            let mut perm_str = String::new();
            self.conn.reader().read_line(&mut perm_str).await?;
            let state = perm_str.parse::<Permutation>().map_err(Error::other)?;
            let _ = self.current_state.insert(state);
        }

        Ok(self.current_state.as_ref().unwrap())
    }

    async fn solve(&mut self) -> Result<(), Self::Error> {
        self.current_state = Some(Permutation::identity());

        let writer = self.conn.writer();
        writer.write_all(b"!SOLVE\n").await?;
        writer.flush().await?;
        ack_or_err(&mut self.conn).await
    }
}

async fn send_ack<V, C: Connection>(
    conn: &mut C,
    maybe_err: Result<V, String>,
) -> Result<Option<V>, Error> {
    let writer = conn.writer();

    match maybe_err {
        Ok(v) => {
            writer.write_all(b"!ACK\n").await?;
            Ok(Some(v))
        }
        Err(e) => {
            let bytes = e.as_bytes();
            let len: u16 = bytes.len().try_into().unwrap_or(u16::MAX);
            writer.write_all(b"!ERR\n").await?;
            writer.write_all(&len.to_be_bytes()).await?;
            writer.write_all(&bytes[0..len as usize]).await?;
            Ok(None)
        }
    }
}

/// Enable remote control of a robot through the given connection.
///
/// # Errors
///
/// Returns an error in case of an IO failure of the channel. All errors given by the robot are forwarded through the connection by stringifying them.
#[allow(clippy::large_stack_arrays)]
pub async fn run_robot_server<C: Connection, R: RobotLike>(
    mut conn: C,
    args: R::InitializationArg,
) -> Result<(), Error>
where
    R::Error: ToString,
{
    let mut len = [0; 2];
    conn.reader().read_exact(&mut len).await?;
    let len = u16::from_be_bytes(len);
    let mut data = vec![0; len as usize];
    conn.reader().read_exact(&mut data).await?;

    let Some((group, mut robot)) = send_ack(
        &mut conn,
        async {
            let group = Arc::new(
                serde_json::from_slice::<PermutationGroup>(&data).map_err(|e| e.to_string())?,
            );

            let robot = R::initialize(Arc::clone(&group), args)
                .await
                .map_err(|e| e.to_string())?;

            Ok((group, robot))
        }
        .await,
    )
    .await?
    else {
        return Ok(());
    };

    loop {
        let mut command = String::new();
        conn.reader()
            .take(u64::from(u16::MAX))
            .read_line(&mut command)
            .await?;

        if command.is_empty() {
            return Ok(());
        }

        trace!("{command}");

        let command = command.trim();

        if command == "!SOLVE" {
            send_ack(&mut conn, robot.solve().await.map_err(|v| v.to_string())).await?;
        } else if command == "!PICTURE" {
            let Some(state) = send_ack(
                &mut conn,
                robot.take_picture().await.map_err(|v| v.to_string()),
            )
            .await?
            else {
                continue;
            };

            let writer = conn.writer();
            writer.write_all(state.to_string().as_bytes()).await?;
            writer.write_all("\n".as_bytes()).await?;
        } else {
            send_ack(
                &mut conn,
                match Algorithm::parse_from_string(Arc::clone(&group), command) {
                    Some(alg) => robot.compose_into(&alg).await.map_err(|v| v.to_string()),
                    None => Err(format!("Could not parse {command} as an algorithm")),
                },
            )
            .await?;
        }

        conn.writer().flush().await?;
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, sync::Arc};

    use puzzle_theory::{
        permutations::{Algorithm, Permutation, PermutationGroup},
        puzzle_geometry::parsing::puzzle,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
    use tokio_util::io::simplex;

    use super::{RemoteRobot, RobotLike, run_robot_server};

    #[tokio::test]
    async fn remote_robot() {
        let cube3 = puzzle("3x3").permutation_group();

        // Yes, we do in fact have to use the `tokio-util` simplex instead of the `tokio` simplex
        // https://github.com/tokio-rs/tokio/issues/6914
        let (mut tx, robot_rx) = simplex::new(1000);
        let (robot_tx, mut rx) = simplex::new(1000);

        let task = tokio::spawn(async move {
            tx.write_all(
                b"!ACK\n!ACK\n!ACK\n(1, 0)\n!ACK\n!ERR\n\0\x03ABC!ERR\n\0\x04ABCD!ERR\n\0\x05ABCDE",
            )
            .await
            .unwrap();
            println!("Dropping");
            drop(tx);

            let mut data = String::new();
            rx.read_to_string(&mut data).await.unwrap();
            assert_eq!(
                data,
                "\0\x06\"3x3\"\nU D U2 D2 U' D'\n!PICTURE\n!SOLVE\nU\n!PICTURE\n!SOLVE\n"
            );
        });

        let robot_rx = BufReader::new(robot_rx);

        let mut remote_robot = RemoteRobot::initialize(Arc::clone(&cube3), (robot_rx, robot_tx))
            .await
            .unwrap();

        remote_robot
            .compose_into(
                &Algorithm::parse_from_string(Arc::clone(&cube3), "U D U2 D2 U' D'").unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            remote_robot.take_picture().await.unwrap(),
            &Permutation::from_cycles(vec![vec![0, 1]])
        );
        assert_eq!(
            remote_robot.take_picture().await.unwrap(),
            &Permutation::from_cycles(vec![vec![0, 1]])
        );
        remote_robot.solve().await.unwrap();
        assert_eq!(
            remote_robot.take_picture().await.unwrap(),
            &Permutation::identity()
        );
        assert!(
            remote_robot
                .compose_into(&Algorithm::parse_from_string(Arc::clone(&cube3), "U").unwrap(),)
                .await
                .is_err()
        );
        assert!(remote_robot.take_picture().await.is_err());
        assert!(remote_robot.solve().await.is_err());

        drop(remote_robot);

        task.await.unwrap();
    }

    #[derive(PartialEq, Eq, Debug)]
    enum Command {
        ComposeInto {
            expected: Algorithm,
            response: Result<(), String>,
        },
        TakePicture {
            response: Result<Permutation, String>,
        },
        Solve {
            response: Result<(), String>,
        },
    }

    struct TestRobot<'a>(&'a mut VecDeque<Command>, Option<Permutation>);

    impl<'a> RobotLike for TestRobot<'a> {
        type InitializationArg = &'a mut VecDeque<Command>;
        type Error = String;

        async fn initialize(
            perm_group: Arc<PermutationGroup>,
            commands: &'a mut VecDeque<Command>,
        ) -> Result<Self, String> {
            assert_eq!(perm_group, puzzle("3x3").permutation_group());
            Ok(TestRobot(commands, None))
        }

        async fn compose_into(&mut self, alg: &Algorithm) -> Result<(), String> {
            let expected = self.0.pop_front().unwrap();
            let Command::ComposeInto { expected, response } = expected else {
                panic!()
            };

            assert_eq!(alg, &expected);

            response
        }

        async fn take_picture(&mut self) -> Result<&Permutation, String> {
            let expected = self.0.pop_front().unwrap();
            let Command::TakePicture { response } = expected else {
                panic!("{expected:?}");
            };

            response.map(|v| &*self.1.insert(v))
        }

        async fn solve(&mut self) -> Result<(), String> {
            let expected = self.0.pop_front().unwrap();
            let Command::Solve { response } = expected else {
                panic!()
            };

            response
        }
    }

    #[tokio::test]
    async fn robot_server() {
        let (mut tx, robot_rx) = simplex::new(1000);
        let (robot_tx, mut rx) = simplex::new(1000);

        let task = tokio::spawn(async move {
            tx.write_all(
                b"\0\x06\"3x3\"\nU D U2 D2 U' D'\n!PICTURE\n!SOLVE\nU\n!PICTURE\n!SOLVE\n",
            )
            .await
            .unwrap();
            drop(tx);

            let mut out = String::new();
            rx.read_to_string(&mut out).await.unwrap();

            assert_eq!(
                out,
                "!ACK\n!ACK\n!ACK\n(0, 1)\n!ACK\n!ERR\n\0\x03ABC!ERR\n\0\x04ABCD!ERR\n\0\x05ABCDE"
            );
        });

        let robot_rx = BufReader::new(robot_rx);

        let group = puzzle("3x3").permutation_group();

        let mut commands = VecDeque::from(vec![
            Command::ComposeInto {
                expected: Algorithm::parse_from_string(Arc::clone(&group), "U D U2 D2 U' D'")
                    .unwrap(),
                response: Ok(()),
            },
            Command::TakePicture {
                response: Ok(Permutation::from_cycles(vec![vec![0, 1]])),
            },
            Command::Solve { response: Ok(()) },
            Command::ComposeInto {
                expected: Algorithm::parse_from_string(Arc::clone(&group), "U").unwrap(),
                response: Err("ABC".to_owned()),
            },
            Command::TakePicture {
                response: Err("ABCD".to_owned()),
            },
            Command::Solve {
                response: Err("ABCDE".to_owned()),
            },
        ]);

        run_robot_server::<_, TestRobot>((robot_rx, robot_tx), &mut commands)
            .await
            .unwrap();

        task.await.unwrap();

        assert_eq!(commands, VecDeque::new());
    }
}
