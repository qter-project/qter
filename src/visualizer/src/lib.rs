use std::{
    collections::HashMap, fs, net::SocketAddr, path::PathBuf, sync::{Arc, LazyLock}
};

use bevy::prelude::*;
use bevy_simple_text_input::TextInputPlugin;
use compiler::compile;
use cube_viz::CubeViz;
use internment::{ArcIntern, Intern};
use interpreter_loop::{CUBE3, CUBE3_DEF};
use interpreter_plugin::{InterpretationCommand, InterpreterPlugin};
use puzzle_theory::{permutations::Permutation, span::File};
use qter_core::{
     Program,
    architectures::{Architecture },
};

use crate::{code_viz::CodeViz, io_viz::IOViz};

mod code_viz;
mod cube_viz;
mod interpreter_loop;
mod interpreter_plugin;
mod io_viz;

struct ProgramInfo {
    program: Arc<Program>,
    architecture: Arc<Architecture>,
    solved_goto_pieces: Vec<Vec<usize>>,
    code: String,
}

fn load_file(name: &str) -> Result<ArcIntern<str>, String> {
    let path = PathBuf::from(name);

    if path.ancestors().count() > 1 {
        // Easier not to implement relative paths and stuff
        return Err("Imported files must be in the same path".to_owned());
    }

    match fs::read_to_string(path) {
        Ok(s) => Ok(ArcIntern::from(s)),
        Err(e) => Err(e.to_string()),
    }
}

static PROGRAMS: LazyLock<HashMap<Intern<str>, ProgramInfo>> = LazyLock::new(|| {
    let mut programs = HashMap::new();

    programs.insert(
        Intern::from("simple"),
        ProgramInfo {
            program: Arc::new(
                compile(
                    &File::from(include_str!("../../compiler/tests/simple/simple.qat")),
                    load_file,
                )
                .unwrap(),
            ),
            architecture: Arc::new(Architecture::new(Arc::clone(&CUBE3), &[vec!["U"], vec!["D'"]]).unwrap()),
            solved_goto_pieces: vec![
                vec![7, 18, 24], // UFR
                vec![23, 29, 42], // DFR
            ],
            code: r#"
0 | input "First number:" U max-input 3
1 | input "Second number:" D' max-input 3
2 | repeat until DFR solved
		U D
3 | halt "(A + B) % 4 =" until UFR solved U'
                "#
            .to_owned(),
        },
    );

    programs.insert(
        Intern::from("avg"),
        ProgramInfo {
            program: Arc::new(
                compile(
                    &File::from(include_str!(
                        "../../compiler/tests/average/average_transform.qat"
                    )),
                    load_file,
                )
                .unwrap(),
            ),
            architecture: CUBE3_DEF.get_preset(&[90_u32, 90].map(Into::into)).unwrap(),
            solved_goto_pieces: vec![
                vec![23, 29, 42], // DFR
                vec![20, 27],     // FR
                vec![5, 16, 10],  // ULF
                vec![3, 9],       // UL
            ],
            code: r#"
0  | input "First number"
          R' F' L U' L U L F U' R
          max-input 90
1  | input "Second number"
          U F R' D' R2 F R' U' D
          max-input 90
2  | solved-goto DFR FR 8
3  | B2 R L2 D L' F' D2 F' L2
     B' U' R D' L' B2 R F
4  | solved-goto ULF UL 14
5  | goto 2
6  | R' F' L U' L U L F U' R
7  | R' U F' L' U' L' U L' F R
8  | solved-goto ULF UL 13
9  | R' U F' L' U' L' U L' F R
10 | solved-goto ULF UL 13
11 | U F R' D' R2 F R' U' D
12 | goto 4
13 | halt until DFR FR solved
          "The average is"
          D' U R F' R2 D R F' U'

14 | solved-goto DFR FR 17
15 | B2 R L2 D L' F' D2 F' L2
    B' U' R D' L' B2 R F
16 | goto 14
17 | R' F' L U' L U L F U' R
18 | R' U F' L' U' L' U L' F R
19 | solved-goto ULF UL 24
20 | R' U F' L' U' L' U L' F R
21 | solved-goto ULF UL 24
22 | U F R' D' R2 F R' U' D
23 | goto 18
24 | halt until DFR FR solved
          "The average is"
          D' U R F' R2 D R F' U'
"#
            .to_owned(),
        },
    );

    println!("{:#?}", programs.get(&Intern::from("avg")).unwrap().program.instructions);
    println!("{:#?}", programs.get(&Intern::from("avg")).unwrap().program.instructions.len());

    programs.insert(
        Intern::from("fib"),
        ProgramInfo {
            program: Arc::new(
                compile(
                    &File::from(include_str!(
                        "../../compiler/tests/fib/fib_transform.qat"
                    )),
                    load_file,
                )
                .unwrap(),
            ),
            architecture: CUBE3_DEF
                .get_preset(&[30_u32, 18, 10, 9].map(Into::into))
                .unwrap(),
            solved_goto_pieces: vec![
                vec![7, 18, 24],  // UFR
                vec![23, 29, 42], // DFR
                vec![6, 17],      // UF
                vec![20, 27],     // FR
                vec![14, 23],     // DL
                vec![5, 16, 10],  // UFL
            ],
            code: r#"
0  | input
     "Which Fibonacci number to calculate:"
     B2 U2 L F' R B L2 D2 B R' F L
     max-input 7
1  | solved-goto UFR 3
2  | goto 4
3  | halt "The number is: 0"
4  | D L' F L2 B L' F' L B' D' L'
5  | L' F' R B' D2 L2 B' R' F L' U2 B2
6  | solved-goto UFR 8
7  | goto 9
8  | halt "The number is"
          L D B L' F L B' L2 F' L D'
          counting-until DL DFL
9 | repeat until DL DFL solved
            L U' B R' L B' L' U'
            L U R2 B R2 D2 R2 D'
10 | L' F' R B' D2 L2 B' R' F L' U2 B2
11 | solved-goto UFR 13
12 | goto 14
13 | halt "The number is"
          F2 L2 U2 D' R U' B L' B L' U'
          counting-until FR DRF
14 | repeat until FR DRF solved
            D' B' U2 B D' F' D L' D2
            F' R' D2 F2 R F2 R2 U' R'
15 | L' F' R B' D2 L2 B' R' F L' U2 B2
16 | solved-goto UFR 18
17 | goto 19
18 | halt "The number is"
          U L' R' F' U' F' L' F2 L U R
          counting-until UF
19 | repeat until UF solved
            B R2 D' R B D F2 U2 D'
            F' L2 F D2 F B2 D' L' U'
20 | goto 5
"#
            .to_owned(),
        },
    );

    programs.insert(
        Intern::from("multiply"),
        ProgramInfo {
            program: Arc::new(
                compile(
                    &File::from(include_str!(
                        "../../compiler/tests/multiply/multiply_transform.qat"
                    )),
                    load_file,
                )
                .unwrap(),
            ),
            architecture: CUBE3_DEF
                .get_preset(&[30_u32, 30, 30].map(Into::into))
                .unwrap(),
            solved_goto_pieces: vec![
                vec![7, 18, 24],  // UFR
                vec![20, 27],     // FR
                vec![4, 25],      // UR
                vec![5, 16, 10],  // UFL
                vec![13, 39, 45], // DBL
                vec![1, 33],      // UB
            ],
            code: r#"
0  | input "Enter number X"
          L2 F2 U L' F D' F' U' L' F U D L' U'
          max-input 29
1  | input "Enter number Y"
          R2 L U' R' L2 F' D R' D L B2 D2
          max-input 29
2  | solved-goto FR UFR 75
3  | solved-goto UB DLB 77
4  | solved-goto UB 10
5  | F2 B2 U F2 B2 D' B L2 D2 B' D B2 D L D2 B2 D'
6  | solved-goto UB 25
7  | F2 B2 U F2 B2 D' B L2 D2 B' D B2 D L D2 B2 D'
8  | goto 4
9 | repeat until ULF solved
            D B2 D2 L' D' B2 D' B D2
            L2 B' D F2 B2 U' F2 B2
10 | repeat until UR ULF solved
            D' R L' U' F' B2 L B
            U B L U R' D2 B' U'
11 | repeat until UB DLB solved
            F' B' D2 R' B2 R U R2 B2
            L' B' U B R2 L2 F R L'
12 | repeat until ULF solved
            D B2 D2 L' D' B2 D' B D2
            L2 B' D F2 B2 U' F2 B2
13 | repeat until UR ULF solved
            D' R L' U' F' B2 L B
            U B L U R' D2 B' U'
14 | repeat until FR UFR solved
            B2 L F D R2 F R' U
            F' R2 F D2 L2 D L B2
15 | repeat until UR solved
            D2 L D' F' D' R D' R
            U2 B R B2 U R' U F D
16 | repeat until UR ULF solved
            F2 L' B' D F2 U' R2 F
            U2 R' D' B U2 F' L2 U
17 | goto 4
18 | repeat until UB DLB solved
            U' R2 L2 U R2 F2 D2 R'
            F2 L' U2 L U L' B D' B
19 | repeat until ULF solved
            D B2 D2 L' D' B2 D' B D2
            L2 B' D F2 B2 U' F2 B2
20 | repeat until UR ULF solved
            D' R L' U' F' B2 L B
            U B L U R' D2 B' U'
21 | repeat until FR UFR solved
            U' R' L2 B' L' D' F' R
            F' D R' L B2 R2 L2 U' R2
22 | repeat until UR solved
            D2 L D' F' D' R D' R
            U2 B R B2 U R' U F D
23 | repeat until UR ULF solved
            F2 L' B' D F2 U' R2 F
            U2 R' D' B U2 F' L2 U
24 | goto 27
25 | repeat until ULF solved
            D B2 D2 L' D' B2 D' B D2
            L2 B' D F2 B2 U' F2 B2
26 | repeat until UR ULF solved
            D' R L' U' F' B2 L B
            U B L U R' D2 B' U'
27 | solved-goto DLB 18
28 | D2 B2 L' D' R D' F R L2 U R2 L'
29 | solved-goto UB DLB 77
30 | goto 33
31 | solved-goto UB 43
32 | D2 B2 L' D' R D' F R L2 U R2 L'
33 | U L2 B' L U' B' U2 R B' R' B L
34 | solved-goto UB 52
35 | F2 B2 U F2 B2 D' B L2 D2 B' D B2 D L D2 B2 D'
36 | solved-goto UB 52
37 | F2 B2 U F2 B2 D' B L2 D2 B' D B2 D L D2 B2 D'
38 | solved-goto UB 52
39 | F2 B2 U F2 B2 D' B L2 D2 B' D B2 D L D2 B2 D'
40 | solved-goto UB 52
41 | F2 B2 U F2 B2 D' B L2 D2 B' D B2 D L D2 B2 D'
42 | goto 31
43 | repeat until ULF solved
            D B2 D2 L' D' B2 D' B D2
            L2 B' D F2 B2 U' F2 B2
44 | repeat until UR ULF solved
            D' R L' U' F' B2 L B
            U B L U R' D2 B' U'
45 | repeat until UB DLB solved
            F2 B2 U F2 D L' B D2 B2
            D L2 D' B R' L2 B2 R L
46 | repeat until ULF solved
            D B2 D2 L' D' B2 D' B D2
            L2 B' D F2 B2 U' F2 B2
47 | repeat until UR ULF solved
            D' R L' U' F' B2 L B
            U B L U R' D2 B' U'
48 | repeat until FR UFR solved
            D2 R' F' D2 R F R F' R'
            B' D F' L' D' B' L2 U' B2
49 | repeat until UR solved
            D2 L D' F' D' R D' R
            U2 B R B2 U R' U F D
50 | repeat until UR ULF solved
            F2 L' B' D F2 U' R2 F
            U2 R' D' B U2 F' L2 U
51 | goto 31
52 | repeat until ULF solved
            D B2 D2 L' D' B2 D' B D2
            L2 B' D F2 B2 U' F2 B2
53 | repeat until UR ULF solved
            D' R L' U' F' B2 L B
            U B L U R' D2 B' U'
54 | D2 B2 L' D' R D' F R L2 U R2 L'
55 | solved-goto UB 65
56 | R2 L U' R' L2 F' D R' D L B2 D2
57 | repeat until UB DLB solved
            F' D' F' U' R B2 U2 D'
            R D F2 L B2 L D2 L2 D2
58 | repeat until ULF solved
            D B2 D2 L' D' B2 D' B D2
            L2 B' D F2 B2 U' F2 B2
59 | repeat until UR ULF solved
            D' R L' U' F' B2 L B
            U B L U R' D2 B' U'
60 | repeat until FR UFR solved
            D2 F U2 R' U D2 F D'
            R D R2 D F' R U R
61 | repeat until UR solved
            D2 L D' F' D' R D' R
            U2 B R B2 U R' U F D
62 | repeat until UR ULF solved
            F2 L' B' D F2 U' R2 F
            U2 R' D' B U2 F' L2 U
63 | goto 54
64 | D2 B2 L' D' R D' F R L2 U R2 L'
65 | solved-goto UB DLB 77
66 | R2 L U' R' L2 F' D R' D L B2 D2
67 | repeat until UB DLB solved
            R' B' R D F L2 U' B2 L2
            B' U L2 U L' U' B2 L2 F'
68 | repeat until ULF solved
            D B2 D2 L' D' B2 D' B D2
            L2 B' D F2 B2 U' F2 B2
69 | repeat until UR ULF solved
            D' R L' U' F' B2 L B
            U B L U R' D2 B' U'
70 | repeat until FR UFR solved
            R' F2 D F' B2 L2 U L2 U
            F' B2 R D2 R' D' F2 D'
71 | repeat until UR solved
            D2 L D' F' D' R D' R
            U2 B R B2 U R' U F D
72 | repeat until UR ULF solved
            F2 L' B' D F2 U' R2
            F U2 R' D' B U2 F' L2 U
73 | goto 64
74 | repeat until UB DLB solved
            D2 B2 L' D' R D' F R L2 U R2 L'
75 | goto 77
76 | repeat until FR UFR solved
            U L U' D' F' L U F D F' L U' F2 L2
77 | halt until FR UFR solved
          "(X * Y) mod 30 ="
          U L U' D' F' L U F D F' L U' F2 L2
"#
            .to_owned(),
        },
    );

    // for (name, program) in &programs {
    //       println!("{name}, {}", program.program.instructions.len());
    //       if &**name == "fib" {
    //           println!("{:#?}", program.program.instructions);
    //       }
    // }

    programs
});

#[derive(Resource)]
struct CurrentState(Permutation);

pub fn visualizer(remote: Option<SocketAddr>) {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .add_plugins(InterpreterPlugin { remote })
        .add_plugins(CubeViz)
        .add_plugins(CodeViz)
        .add_plugins(IOViz)
        .add_plugins(TextInputPlugin)
        .run();
}
