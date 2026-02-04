#![expect(async_fn_in_trait)] // Our interpreter doesn't care whether whether our futures are `Send` and any code using the interpreter is likely to hardcode a particular `PuzzleState` impl so will know statically whether the future is `Send`

use std::{convert::Infallible, io::Error, sync::Arc};

use log::trace;
use puzzle_theory::{
    numbers::{I, Int, U, lcm_iter},
    permutations::{Algorithm, Permutation, PermutationGroup},
};
use qter_core::{
    Program, PuzzleIdx, TheoreticalIdx,
    architectures::{chromatic_orders_by_facelets, decode},
};
use tokio::io::{
    AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader,
};
use tokio_stream::StreamExt;

/// An instance of a theoretical register. Analagous to the `Puzzle` structure.
pub struct TheoreticalState {
    value: Int<U>,
    order: Int<U>,
}

impl TheoreticalState {
    pub fn add_to_i(&mut self, amt: Int<I>) {
        self.add_to(amt % self.order);
    }

    pub fn add_to(&mut self, amt: Int<U>) {
        self.value += amt % self.order;

        if self.value >= self.order {
            self.value -= self.order;
        }
    }

    pub fn zero_out(&mut self) {
        self.value = Int::zero();
    }

    #[must_use]
    pub fn order(&self) -> Int<U> {
        self.order
    }

    #[must_use]
    pub fn value(&self) -> Int<U> {
        self.value
    }
}

pub trait PuzzleState {
    type InitializationArgs;
    type Error;

    /// Initialize the `Puzzle` in the solved state
    async fn initialize(
        perm_group: Arc<PermutationGroup>,
        args: Self::InitializationArgs,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized;

    /// Perform an algorithm on the puzzle state
    async fn compose_into(&mut self, alg: &Algorithm) -> Result<(), Self::Error>;

    /// Check whether the given facelets are solved
    async fn facelets_solved(&mut self, facelets: &[usize]) -> Result<bool, Self::Error>;

    /// Decode the permutation using the register generator and the given facelets.
    ///
    /// In general, an arbitrary scramble cannot be decoded. If this is the case, the function will return `None`.
    ///
    /// This function should not alter the cube state unless it returns `None`.
    async fn print(
        &mut self,
        facelets: &[usize],
        generator: &Algorithm,
    ) -> Result<Option<Int<U>>, Self::Error>;

    /// Decode the register without requiring the cube state to be unaltered.
    async fn halt(
        &mut self,
        facelets: &[usize],
        generator: &Algorithm,
    ) -> Result<Option<Int<U>>, Self::Error> {
        self.print(facelets, generator).await
    }

    /// Repeat the algorithm until the given facelets are solved.
    ///
    /// Returns None if the facelets cannot be solved by repeating the algorithm.
    async fn repeat_until(
        &mut self,
        facelets: &[usize],
        generator: &Algorithm,
    ) -> Result<Option<()>, Self::Error>;

    /// Bring the puzzle to the solved state
    async fn solve(&mut self) -> Result<(), Self::Error>;
}

pub trait RobotLike {
    type InitializationArgs;
    type Error;

    /// Initialize the puzzle in the solved state
    async fn initialize(
        perm_group: Arc<PermutationGroup>,
        args: Self::InitializationArgs,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized;

    /// Perform an algorithm on the puzzle
    async fn compose_into(&mut self, alg: &Algorithm) -> Result<(), Self::Error>;

    // Wait for all queued moves to finish. Returns a oneshot that will be triggered either when all previously queued moves are finished, or
    // async fn await_moves(
    //     &mut self,
    // ) -> Result<impl Future<Output = Result<(), Self::Error>>, Self::Error>;

    /// Return the puzzle state as a permutation
    async fn take_picture(&mut self) -> Result<&Permutation, Self::Error>;

    /// Solve the puzzle
    async fn solve(&mut self) -> Result<(), Self::Error>;
}

pub struct RobotState<R: RobotLike> {
    robot: R,
    perm_group: Arc<PermutationGroup>,
}

impl<R: RobotLike> PuzzleState for RobotState<R> {
    type InitializationArgs = R::InitializationArgs;
    type Error = R::Error;

    async fn compose_into(&mut self, alg: &Algorithm) -> Result<(), Self::Error> {
        self.robot.compose_into(alg).await
    }

    async fn initialize(
        perm_group: Arc<PermutationGroup>,
        args: Self::InitializationArgs,
    ) -> Result<Self, Self::Error> {
        Ok(RobotState {
            perm_group: Arc::clone(&perm_group),
            robot: R::initialize(perm_group, args).await?,
        })
    }

    async fn facelets_solved(&mut self, facelets: &[usize]) -> Result<bool, Self::Error> {
        let state = self.robot.take_picture().await?;

        for &facelet in facelets {
            let maps_to = state.mapping().get(facelet);
            if self.perm_group.facelet_colors()[maps_to]
                != self.perm_group.facelet_colors()[facelet]
            {
                return Ok(false);
            }
        }

        Ok(true)
    }

    async fn print(
        &mut self,
        facelets: &[usize],
        generator: &Algorithm,
    ) -> Result<Option<Int<U>>, Self::Error> {
        let before = self.robot.take_picture().await?.to_owned();

        let Some(c) = self.halt(facelets, generator).await? else {
            return Ok(None);
        };

        let mut exponentiated = generator.to_owned();
        exponentiated.exponentiate(c.into());

        self.compose_into(&exponentiated).await?;

        if &before != self.robot.take_picture().await? {
            eprintln!("Printing did not return the cube to the original state!");
            return Ok(None);
        }
        Ok(Some(c))
    }

    async fn halt(
        &mut self,
        facelets: &[usize],
        generator: &Algorithm,
    ) -> Result<Option<Int<U>>, Self::Error> {
        let mut generator = generator.to_owned();
        generator.exponentiate(-Int::<U>::one());

        let mut sum = Int::<U>::zero();

        let chromatic_orders = chromatic_orders_by_facelets(&generator);
        let order = lcm_iter(facelets.iter().map(|&i| chromatic_orders[i]));

        while !self.facelets_solved(facelets).await? {
            sum += Int::<U>::one();

            if sum >= order {
                eprintln!(
                    "Decoding failure! Performed as many cycles as the size of the register."
                );
                return Ok(None);
            }

            self.compose_into(&generator).await?;
        }

        Ok(Some(sum))
    }

    async fn repeat_until(
        &mut self,
        facelets: &[usize],
        generator: &Algorithm,
    ) -> Result<Option<()>, Self::Error> {
        // Halting has the same behavior as repeat_until
        Ok(self.halt(facelets, generator).await?.map(|_| ()))
    }

    async fn solve(&mut self) -> Result<(), Self::Error> {
        self.robot.solve().await
    }
}

#[derive(Clone, Debug)]
pub struct SimulatedPuzzle {
    perm_group: Arc<PermutationGroup>,
    pub(crate) state: Permutation,
}

impl SimulatedPuzzle {
    /// Get the state underlying the puzzle
    pub fn puzzle_state(&self) -> &Permutation {
        &self.state
    }
}

impl PuzzleState for SimulatedPuzzle {
    type InitializationArgs = ();
    type Error = Infallible;

    async fn initialize(perm_group: Arc<PermutationGroup>, (): ()) -> Result<Self, Infallible> {
        Ok(SimulatedPuzzle {
            state: Permutation::identity(),
            perm_group,
        })
    }

    async fn compose_into(&mut self, alg: &Algorithm) -> Result<(), Infallible> {
        self.state.compose_into(alg.permutation());
        Ok(())
    }

    async fn facelets_solved(&mut self, facelets: &[usize]) -> Result<bool, Infallible> {
        for &facelet in facelets {
            let maps_to = self.state.mapping().get(facelet);
            if self.perm_group.facelet_colors()[maps_to]
                != self.perm_group.facelet_colors()[facelet]
            {
                return Ok(false);
            }
        }

        Ok(true)
    }

    async fn print(
        &mut self,
        facelets: &[usize],
        generator: &Algorithm,
    ) -> Result<Option<Int<U>>, Infallible> {
        Ok(decode(&self.state, facelets, generator))
    }

    async fn solve(&mut self) -> Result<(), Infallible> {
        self.state = Permutation::identity();
        Ok(())
    }

    async fn repeat_until(
        &mut self,
        facelets: &[usize],
        generator: &Algorithm,
    ) -> Result<Option<()>, Infallible> {
        let mut generator = generator.to_owned();
        generator.exponentiate(-Int::<U>::one());
        let Some(v) = decode(&self.state, facelets, &generator) else {
            return Ok(None);
        };
        generator.exponentiate(-v);
        <Self as PuzzleState>::compose_into(self, &generator).await?;
        Ok(Some(()))
    }
}

impl RobotLike for SimulatedPuzzle {
    type InitializationArgs = ();
    type Error = Infallible;

    async fn initialize(perm_group: Arc<PermutationGroup>, (): ()) -> Result<Self, Infallible> {
        <Self as PuzzleState>::initialize(perm_group, ()).await
    }

    async fn compose_into(&mut self, alg: &Algorithm) -> Result<(), Infallible> {
        <Self as PuzzleState>::compose_into(self, alg).await
    }

    async fn take_picture(&mut self) -> Result<&Permutation, Infallible> {
        Ok(self.puzzle_state())
    }

    async fn solve(&mut self) -> Result<(), Infallible> {
        <Self as PuzzleState>::solve(self).await
    }
}

/// A collection of the states of every puzzle and theoretical register
pub(crate) struct PuzzleStates<P: PuzzleState> {
    theoretical_states: Vec<TheoreticalState>,
    puzzle_states: Vec<P>,
}

impl<P: PuzzleState> PuzzleStates<P>
where
    P::InitializationArgs: Clone,
{
    pub(crate) async fn new(
        program: &Program,
        args: P::InitializationArgs,
    ) -> Result<Self, P::Error> {
        let theoretical_states = program
            .theoretical
            .iter()
            .map(|order| TheoreticalState {
                value: Int::zero(),
                order: **order,
            })
            .collect();

        let puzzle_states = tokio_stream::iter(program.puzzles.iter())
            .then(|perm_group| P::initialize(Arc::clone(perm_group), args.clone()))
            .collect::<Result<Vec<_>, _>>()
            .await?;

        Ok(PuzzleStates {
            theoretical_states,
            puzzle_states,
        })
    }
}

impl<P: PuzzleState> PuzzleStates<P> {
    pub(crate) async fn new_only_one_puzzle(
        program: &Program,
        args: P::InitializationArgs,
    ) -> Result<Self, P::Error> {
        let theoretical_states = program
            .theoretical
            .iter()
            .map(|order| TheoreticalState {
                value: Int::zero(),
                order: **order,
            })
            .collect();

        let puzzle_states = if program.puzzles.is_empty() {
            Vec::new()
        } else if program.puzzles.len() == 1 {
            vec![P::initialize(Arc::clone(&program.puzzles[0]), args).await?]
        } else {
            panic!("Expected at most one puzzle in the program");
        };

        Ok(PuzzleStates {
            theoretical_states,
            puzzle_states,
        })
    }

    #[must_use]
    pub fn theoretical_state(&self, idx: TheoreticalIdx) -> &TheoreticalState {
        &self.theoretical_states[idx.0]
    }

    pub fn theoretical_state_mut(&mut self, idx: TheoreticalIdx) -> &mut TheoreticalState {
        &mut self.theoretical_states[idx.0]
    }

    pub fn puzzle_state_mut(&mut self, idx: PuzzleIdx) -> &mut P {
        &mut self.puzzle_states[idx.0]
    }
}

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
    type InitializationArgs = C;
    type Error = Error;

    async fn initialize(_: Arc<PermutationGroup>, conn: C) -> Result<Self, Self::Error> {
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
/// Returns an error in case of an IO failure of the channel. All errors given by the robot are forwarded through the connection.
pub async fn run_robot_server<C: Connection, R: RobotLike>(
    mut conn: C,
    robot: &mut R,
    group: &Arc<PermutationGroup>,
) -> Result<(), Error>
where
    R::Error: ToString,
{
    loop {
        let mut command = String::new();
        conn.reader().read_line(&mut command).await?;

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
                match Algorithm::parse_from_string(Arc::clone(group), command) {
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

    use crate::puzzle_states::{RemoteRobot, RobotLike, run_robot_server};

    #[tokio::test]
    async fn remote_robot() {
        let cube3 = puzzle("3x3").permutation_group();

        // Yes, we do in fact have to use the `tokio-util` simplex instead of the `tokio` simplex
        // https://github.com/tokio-rs/tokio/issues/6914
        let (mut tx, robot_rx) = simplex::new(1000);
        let (robot_tx, mut rx) = simplex::new(1000);

        let task = tokio::spawn(async move {
            tx.write_all(
                b"!ACK\n!ACK\n(1, 0)\n!ACK\n!ERR\n\0\x03ABC!ERR\n\0\x04ABCD!ERR\n\0\x05ABCDE",
            )
            .await
            .unwrap();
            println!("Dropping");
            drop(tx);

            let mut data = String::new();
            rx.read_to_string(&mut data).await.unwrap();
            assert_eq!(
                data,
                "U D U2 D2 U' D'\n!PICTURE\n!SOLVE\nU\n!PICTURE\n!SOLVE\n"
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

    struct TestRobot(VecDeque<Command>, Option<Permutation>);

    impl RobotLike for TestRobot {
        type InitializationArgs = ();
        type Error = String;

        async fn initialize(
            perm_group: Arc<PermutationGroup>,
            (): Self::InitializationArgs,
        ) -> Result<Self, String> {
            Ok(TestRobot(
                VecDeque::from(vec![
                    Command::ComposeInto {
                        expected: Algorithm::parse_from_string(
                            Arc::clone(&perm_group),
                            "U D U2 D2 U' D'",
                        )
                        .unwrap(),
                        response: Ok(()),
                    },
                    Command::TakePicture {
                        response: Ok(Permutation::from_cycles(vec![vec![0, 1]])),
                    },
                    Command::Solve { response: Ok(()) },
                    Command::ComposeInto {
                        expected: Algorithm::parse_from_string(Arc::clone(&perm_group), "U")
                            .unwrap(),
                        response: Err("ABC".to_owned()),
                    },
                    Command::TakePicture {
                        response: Err("ABCD".to_owned()),
                    },
                    Command::Solve {
                        response: Err("ABCDE".to_owned()),
                    },
                ]),
                None,
            ))
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
            tx.write_all(b"U D U2 D2 U' D'\n!PICTURE\n!SOLVE\nU\n!PICTURE\n!SOLVE\n")
                .await
                .unwrap();
            drop(tx);

            let mut out = String::new();
            rx.read_to_string(&mut out).await.unwrap();

            assert_eq!(
                out,
                "!ACK\n!ACK\n(0, 1)\n!ACK\n!ERR\n\0\x03ABC!ERR\n\0\x04ABCD!ERR\n\0\x05ABCDE"
            );
        });

        let robot_rx = BufReader::new(robot_rx);

        let group = puzzle("3x3").permutation_group();

        let mut robot = TestRobot::initialize(Arc::clone(&group), ()).await.unwrap();

        run_robot_server::<_, TestRobot>((robot_rx, robot_tx), &mut robot, &group)
            .await
            .unwrap();

        assert_eq!(robot.0, VecDeque::new());

        task.await.unwrap();
    }
}
