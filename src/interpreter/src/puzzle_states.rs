use std::{
    io::{self, BufRead, BufReader, Write},
    net::TcpStream,
    sync::Arc,
};

use log::trace;
use puzzle_theory::{
    numbers::{I, Int, U, lcm_iter},
    permutations::{Algorithm, Permutation, PermutationGroup},
};
use qter_core::{
    Program, PuzzleIdx, TheoreticalIdx,
    architectures::{chromatic_orders_by_facelets, decode},
};

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

    /// Initialize the `Puzzle` in the solved state
    fn initialize(perm_group: Arc<PermutationGroup>, args: Self::InitializationArgs) -> Self;

    /// Perform an algorithm on the puzzle state
    fn compose_into(&mut self, alg: &Algorithm);

    /// Check whether the given facelets are solved
    fn facelets_solved(&mut self, facelets: &[usize]) -> bool;

    /// Decode the permutation using the register generator and the given facelets.
    ///
    /// In general, an arbitrary scramble cannot be decoded. If this is the case, the function will return `None`.
    ///
    /// This function should not alter the cube state unless it returns `None`.
    fn print(&mut self, facelets: &[usize], generator: &Algorithm) -> Option<Int<U>>;

    /// Decode the register without requiring the cube state to be unaltered.
    fn halt(&mut self, facelets: &[usize], generator: &Algorithm) -> Option<Int<U>> {
        self.print(facelets, generator)
    }

    /// Repeat the algorithm until the given facelets are solved.
    ///
    /// Returns None if the facelets cannot be solved by repeating the algorithm.
    fn repeat_until(&mut self, facelets: &[usize], generator: &Algorithm) -> Option<()>;

    /// Bring the puzzle to the solved state
    fn solve(&mut self);
}

pub trait RobotLike {
    type InitializationArgs;

    /// Initialize the puzzle in the solved state
    fn initialize(perm_group: Arc<PermutationGroup>, args: Self::InitializationArgs) -> Self;

    /// Perform an algorithm on the puzzle
    fn compose_into(&mut self, alg: &Algorithm);

    /// Return the puzzle state as a permutation
    fn take_picture(&mut self) -> &Permutation;

    /// Solve the puzzle
    fn solve(&mut self);
}

pub trait RobotLikeDyn {
    fn compose_into(&mut self, alg: &Algorithm);

    fn take_picture(&mut self) -> &Permutation;

    fn solve(&mut self);
}

impl<R: RobotLike> RobotLikeDyn for R {
    fn compose_into(&mut self, alg: &Algorithm) {
        <Self as RobotLike>::compose_into(self, alg);
    }

    fn take_picture(&mut self) -> &Permutation {
        <Self as RobotLike>::take_picture(self)
    }

    fn solve(&mut self) {
        <Self as RobotLike>::solve(self);
    }
}

pub struct RobotState<R: RobotLike> {
    robot: R,
    perm_group: Arc<PermutationGroup>,
}

impl<R: RobotLike> PuzzleState for RobotState<R> {
    type InitializationArgs = R::InitializationArgs;

    fn compose_into(&mut self, alg: &Algorithm) {
        self.robot.compose_into(alg);
    }

    fn initialize(perm_group: Arc<PermutationGroup>, args: Self::InitializationArgs) -> Self {
        RobotState {
            perm_group: Arc::clone(&perm_group),
            robot: R::initialize(perm_group, args),
        }
    }

    fn facelets_solved(&mut self, facelets: &[usize]) -> bool {
        let state = self.robot.take_picture();

        for &facelet in facelets {
            let maps_to = state.goes_to().get(facelet);
            if self.perm_group.facelet_colors()[maps_to]
                != self.perm_group.facelet_colors()[facelet]
            {
                return false;
            }
        }

        true
    }

    fn print(&mut self, facelets: &[usize], generator: &Algorithm) -> Option<Int<U>> {
        let before = self.robot.take_picture().to_owned();

        let c = self.halt(facelets, generator)?;

        let mut exponentiated = generator.to_owned();
        exponentiated.exponentiate(c.into());

        self.compose_into(&exponentiated);

        if &before != self.robot.take_picture() {
            eprintln!("Printing did not return the cube to the original state!");
            return None;
        }
        Some(c)
    }

    fn halt(&mut self, facelets: &[usize], generator: &Algorithm) -> Option<Int<U>> {
        let mut generator = generator.to_owned();
        generator.exponentiate(-Int::<U>::one());

        let mut sum = Int::<U>::zero();

        let chromatic_orders = chromatic_orders_by_facelets(&generator);
        let order = lcm_iter(facelets.iter().map(|&i| chromatic_orders[i]));

        while !self.facelets_solved(facelets) {
            sum += Int::<U>::one();

            if sum >= order {
                eprintln!(
                    "Decoding failure! Performed as many cycles as the size of the register."
                );
                return None;
            }

            self.compose_into(&generator);
        }

        Some(sum)
    }

    fn repeat_until(&mut self, facelets: &[usize], generator: &Algorithm) -> Option<()> {
        // Halting has the same behavior as repeat_until
        self.halt(facelets, generator).map(|_| ())
    }

    fn solve(&mut self) {
        self.robot.solve();
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

    fn initialize(perm_group: Arc<PermutationGroup>, (): ()) -> Self {
        SimulatedPuzzle {
            state: Permutation::identity(),
            perm_group,
        }
    }

    fn compose_into(&mut self, alg: &Algorithm) {
        self.state.compose_into(alg.permutation());
    }

    fn facelets_solved(&mut self, facelets: &[usize]) -> bool {
        for &facelet in facelets {
            let maps_to = self.state.goes_to().get(facelet);
            if self.perm_group.facelet_colors()[maps_to]
                != self.perm_group.facelet_colors()[facelet]
            {
                return false;
            }
        }

        true
    }

    fn print(&mut self, facelets: &[usize], generator: &Algorithm) -> Option<Int<U>> {
        decode(&self.state, facelets, generator)
    }

    fn solve(&mut self) {
        self.state = Permutation::identity();
    }

    fn repeat_until(&mut self, facelets: &[usize], generator: &Algorithm) -> Option<()> {
        let mut generator = generator.to_owned();
        generator.exponentiate(-Int::<U>::one());
        let v = decode(&self.state, facelets, &generator)?;
        generator.exponentiate(-v);
        <Self as PuzzleState>::compose_into(self, &generator);
        Some(())
    }
}

impl RobotLike for SimulatedPuzzle {
    type InitializationArgs = ();

    fn initialize(perm_group: Arc<PermutationGroup>, (): ()) -> Self {
        <Self as PuzzleState>::initialize(perm_group, ())
    }

    fn compose_into(&mut self, alg: &Algorithm) {
        <Self as PuzzleState>::compose_into(self, alg);
    }

    fn take_picture(&mut self) -> &Permutation {
        self.puzzle_state()
    }

    fn solve(&mut self) {
        <Self as PuzzleState>::solve(self);
    }
}

/// A collection of the states of every puzzle and theoretical register
pub struct PuzzleStates<P: PuzzleState> {
    theoretical_states: Vec<TheoreticalState>,
    puzzle_states: Vec<P>,
}

impl<P: PuzzleState> PuzzleStates<P>
where
    P::InitializationArgs: Clone,
{
    #[must_use]
    pub fn new(program: &Program, args: P::InitializationArgs) -> Self {
        let theoretical_states = program
            .theoretical
            .iter()
            .map(|order| TheoreticalState {
                value: Int::zero(),
                order: **order,
            })
            .collect();

        let puzzle_states = program
            .puzzles
            .iter()
            .map(|perm_group| P::initialize(Arc::clone(perm_group), args.clone()))
            .collect();

        PuzzleStates {
            theoretical_states,
            puzzle_states,
        }
    }
}

impl<P: PuzzleState> PuzzleStates<P> {
    #[must_use]
    pub fn new_only_one_puzzle(program: &Program, args: P::InitializationArgs) -> Self {
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
            vec![P::initialize(Arc::clone(&program.puzzles[0]), args)]
        } else {
            panic!("Expected at most one puzzle in the program");
        };

        PuzzleStates {
            theoretical_states,
            puzzle_states,
        }
    }

    #[must_use]
    pub fn theoretical_state(&self, idx: TheoreticalIdx) -> &TheoreticalState {
        &self.theoretical_states[idx.0]
    }

    #[must_use]
    pub fn puzzle_state(&self, idx: PuzzleIdx) -> &P {
        &self.puzzle_states[idx.0]
    }

    pub fn theoretical_state_mut(&mut self, idx: TheoreticalIdx) -> &mut TheoreticalState {
        &mut self.theoretical_states[idx.0]
    }

    pub fn puzzle_state_mut(&mut self, idx: PuzzleIdx) -> &mut P {
        &mut self.puzzle_states[idx.0]
    }
}

pub trait Connection {
    type Reader: BufRead;
    type Writer: Write;

    fn reader(&mut self) -> &mut Self::Reader;
    fn writer(&mut self) -> &mut Self::Writer;
}

impl<R: BufRead, W: Write> Connection for (R, W) {
    type Reader = R;
    type Writer = W;

    fn reader(&mut self) -> &mut Self::Reader {
        &mut self.0
    }

    fn writer(&mut self) -> &mut Self::Writer {
        &mut self.1
    }
}

impl Connection for BufReader<TcpStream> {
    type Reader = Self;
    type Writer = TcpStream;

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

impl<C: Connection> RobotLike for RemoteRobot<C> {
    type InitializationArgs = C;

    fn initialize(_: Arc<PermutationGroup>, mut conn: C) -> Self {
        let writer = conn.writer();
        writer.flush().unwrap();

        RemoteRobot {
            conn,
            current_state: None,
        }
    }

    fn compose_into(&mut self, alg: &Algorithm) {
        self.current_state = None;
        let writer = self.conn.writer();
        writeln!(
            writer,
            "{}",
            alg.move_seq_iter()
                .map(|v| &**v)
                .collect::<Vec<_>>()
                .join(" ")
        )
        .unwrap();
        writer.flush().unwrap();
    }

    fn take_picture(&mut self) -> &Permutation {
        self.current_state.get_or_insert_with(|| {
            let writer = self.conn.writer();
            writeln!(writer, "!PICTURE").unwrap();
            writer.flush().unwrap();

            let mut perm_str = String::new();
            self.conn.reader().read_line(&mut perm_str).unwrap();
            perm_str.parse::<Permutation>().unwrap()
        })
    }

    fn solve(&mut self) {
        self.current_state = Some(Permutation::identity());

        let writer = self.conn.writer();
        writeln!(writer, "!SOLVE").unwrap();
        writer.flush().unwrap();
    }
}

pub fn run_robot_server<C: Connection, R: RobotLike>(
    mut conn: C,
    robot: &mut R,
    group: &Arc<PermutationGroup>,
) -> Result<(), io::Error> {
    let mut puzzle_def = String::new();
    conn.reader().read_line(&mut puzzle_def)?;

    if puzzle_def.is_empty() {
        return Ok(());
    }

    loop {
        let mut command = String::new();
        conn.reader().read_line(&mut command)?;

        if command.is_empty() {
            return Ok(());
        }

        trace!("{command}");

        let command = command.trim();

        if command == "!SOLVE" {
            robot.solve();
        } else if command == "!PICTURE" {
            let state = robot.take_picture();
            let writer = conn.writer();
            writeln!(writer, "{state}")?;
            writer.flush()?;
        } else {
            let alg =
                Algorithm::parse_from_string(Arc::clone(group), command).ok_or_else(|| {
                    io::Error::other(format!("Could not parse {command} as an algorithm"))
                })?;

            robot.compose_into(&alg);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{self, BufReader, Read, Write},
        sync::Arc,
    };

    use puzzle_theory::{
        permutations::{Algorithm, Permutation, PermutationGroup},
        puzzle_geometry::parsing::puzzle,
    };

    use crate::puzzle_states::{RemoteRobot, RobotLike, run_robot_server};

    #[test]
    fn remote_robot() {
        let cube3 = puzzle("3x3").permutation_group();

        let (mut rx, tx_robot) = io::pipe().unwrap();
        let (rx_robot, mut tx) = io::pipe().unwrap();

        writeln!(tx, "(1, 0)").unwrap();
        drop(tx);

        let rx_robot = BufReader::new(rx_robot);

        {
            let mut remote_robot =
                RemoteRobot::initialize(Arc::clone(&cube3), (rx_robot, tx_robot));

            remote_robot.compose_into(
                &Algorithm::parse_from_string(Arc::clone(&cube3), "U D U2 D2 U' D'").unwrap(),
            );
            assert_eq!(
                remote_robot.take_picture(),
                &Permutation::from_cycles(vec![vec![0, 1]])
            );
            assert_eq!(
                remote_robot.take_picture(),
                &Permutation::from_cycles(vec![vec![0, 1]])
            );
            remote_robot.solve();
            assert_eq!(remote_robot.take_picture(), &Permutation::identity());
        }

        let mut data = String::new();
        rx.read_to_string(&mut data).unwrap();
        assert_eq!(data, "U D U2 D2 U' D'\n!PICTURE\n!SOLVE\n");
    }

    #[test]
    fn robot_server() {
        struct TestRobot(usize, Arc<PermutationGroup>, Permutation);

        impl RobotLike for TestRobot {
            type InitializationArgs = ();

            fn initialize(perm_group: Arc<PermutationGroup>, (): Self::InitializationArgs) -> Self {
                TestRobot(0, perm_group, Permutation::from_cycles(vec![vec![0, 1]]))
            }

            fn compose_into(&mut self, alg: &Algorithm) {
                assert_eq!(self.0, 0);
                self.0 += 1;
                assert_eq!(
                    alg,
                    &Algorithm::parse_from_string(Arc::clone(&self.1), "U D U2 D2 U' D'").unwrap()
                );
            }

            fn take_picture(&mut self) -> &Permutation {
                assert_eq!(self.0, 1);
                self.0 += 1;
                &self.2
            }

            fn solve(&mut self) {
                assert_eq!(self.0, 2);
                self.0 += 1;
            }
        }

        let (mut rx, tx_robot) = io::pipe().unwrap();
        let (rx_robot, mut tx) = io::pipe().unwrap();

        write!(tx, "3x3\nU D U2 D2 U' D'\n!PICTURE\n!SOLVE\n").unwrap();
        drop(tx);

        let rx_robot = BufReader::new(rx_robot);

        let group = puzzle("3x3").permutation_group();
        let mut robot = TestRobot::initialize(Arc::clone(&group), ());

        run_robot_server::<_, TestRobot>((rx_robot, tx_robot), &mut robot, &group).unwrap();

        assert_eq!(robot.0, 3);

        let mut out = String::new();
        rx.read_to_string(&mut out).unwrap();

        assert_eq!(out, "(0, 1)\n");
    }
}
