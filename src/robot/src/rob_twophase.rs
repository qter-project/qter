use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Error, Write},
    process::{ChildStdin, ChildStdout, Command, Stdio},
    sync::{Arc, LazyLock, Mutex},
    thread::available_parallelism,
};

use internment::ArcIntern;
use itertools::Itertools;
use log::{trace, warn};
use puzzle_theory::permutations::{Algorithm, Permutation};

use crate::CUBE3;

static COLOR_MAPPING: LazyLock<HashMap<ArcIntern<str>, char>> = LazyLock::new(|| {
    let mut v = HashMap::new();

    v.insert(ArcIntern::from("white"), 'U');
    v.insert(ArcIntern::from("green"), 'F');
    v.insert(ArcIntern::from("yellow"), 'D');
    v.insert(ArcIntern::from("red"), 'R');
    v.insert(ArcIntern::from("blue"), 'B');
    v.insert(ArcIntern::from("orange"), 'L');

    v
});

fn mk_rob_twophase_input(perm: &Permutation) -> String {
    let cube3 = &*CUBE3;
    let color_mapping = &*COLOR_MAPPING;

    let mut faces = Vec::new();

    for (mut chunk, current) in (0..)
        .map(|idx| perm.comes_from().get(idx))
        .chunks(8)
        .into_iter()
        .zip(['U', 'L', 'F', 'R', 'B', 'D'])
    {
        let mut str = String::new();

        for item in chunk.by_ref().take(4) {
            str.push(*color_mapping.get(&cube3.facelet_colors()[item]).unwrap());
        }

        str.push(current);

        for item in chunk {
            str.push(*color_mapping.get(&cube3.facelet_colors()[item]).unwrap());
        }

        faces.push(str);
    }

    // rob-twophase requires U R F D L B order
    [
        &faces[0], &faces[3], &faces[2], &faces[5], &faces[1], &faces[4],
    ]
    .into_iter()
    .join("")
}

pub fn solve_rob_twophase(perm: &Permutation) -> Result<Algorithm, std::io::Error> {
    solve_rob_twophase_string(&mk_rob_twophase_input(perm))
}

pub fn solve_rob_twophase_string(rob_twophase_string: &str) -> Result<Algorithm, std::io::Error> {
    static ROB_TWOPHASE: Mutex<Option<(ChildStdin, BufReader<ChildStdout>)>> = Mutex::new(None);

    let mut maybe_rob_twophase = ROB_TWOPHASE.lock().unwrap();

    let (twophase_stdin, twophase_stdout) = if let Some(v) = &mut *maybe_rob_twophase {
        v
    } else {
        // rob-twophase will dump tables in its current directory; lets have it dump them in some cache
        let mut cache = dirs::cache_dir().unwrap();
        cache.push("rob-twophase-tables");
        fs::create_dir_all(&cache)?;

        let child = Command::new("twophase")
            .current_dir(cache)
            .args(["-c", "-m", "30", "-t"])
            .arg(match available_parallelism() {
                Ok(v) => v.to_string(),
                Err(e) => {
                    warn!(
                        "{} {e}",
                        "Failed to get available parallelism; defaulting to 1:"
                    );
                    (1).to_string()
                }
            })
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdin = child.stdin.unwrap();
        let stdout = BufReader::new(child.stdout.unwrap());

        maybe_rob_twophase.insert((stdin, stdout))
    };

    /*
    Rob Twophase TUI looks like

    ```
    This is rob-twophase v2.0; copyright Elias Frantar 2020.

    Loading tables ...
    Done. 0.518s

    Enter >>solve FACECUBE<< to solve, >>scramble<< to scramble or >>bench<< to benchmark.

    Ready!
    solve LBDLULDDURDRRRFRURBFFRFBFRDLDBDDBDFBBULRLFFBUFLUUBUULL
    30.177ms
    R F2 R' U R U2 F2 U2 F' D' R D2 L2 D2 L' U2 F2 (17)
    Ready!
    solve ABCDEF
    Face-error 2.
    Ready!
    ```
    */

    // Wait until rob-twophase tells us that its ready
    loop {
        let mut string = String::new();
        twophase_stdout.read_line(&mut string)?;
        trace!("{string}");

        if string == "Ready!\n" {
            break;
        }
    }

    writeln!(twophase_stdin, "solve {}", rob_twophase_string)?;
    trace!("solve {rob_twophase_string}");

    // Captures either `30.177ms` or `Error.`
    let mut string = String::new();
    twophase_stdout.read_line(&mut string)?;
    trace!("{string}");

    if string.starts_with("Face-error") {
        return Err(Error::other("Invalid rob_twophase input string"));
    }

    // Captures the alg
    let mut result = String::new();
    twophase_stdout.read_line(&mut result)?;
    trace!("{result}");

    // Remove parentheses and newline
    let alg = result.replace(['(', ')', '\n'], "");

    // Split the string and remove the final move count
    Ok(Algorithm::new_from_move_seq(
        Arc::clone(&CUBE3),
        alg.split(' ')
            .filter(|v| v.chars().next().is_some_and(|v| !v.is_ascii_digit()))
            .map(ArcIntern::from)
            .collect(),
    )
    .unwrap())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use itertools::Itertools;
    use puzzle_theory::{numbers::{I, Int, U}, permutations::{Algorithm, Permutation}};

    use crate::{
        CUBE3,
        rob_twophase::{mk_rob_twophase_input, solve_rob_twophase, solve_rob_twophase_string},
    };

    static TESTS: [[&str; 2]; 60] = [
        ["", "UUUUUUUUURRRRRRRRRFFFFFFFFFDDDDDDDDDLLLLLLLLLBBBBBBBBB"],
        [
            "U",
            "UUUUUUUUUBBBRRRRRRRRRFFFFFFDDDDDDDDDFFFLLLLLLLLLBBBBBB",
        ],
        [
            "U2 R2 L D2 L F2 B2 U' D' F U R' L2 U2 D L F' B2 D R2",
            "FLLLULFRFRUURRRBBBLDDFFUBRUDFRDDUFLDUFUFLDLBRBDRBBULBD",
        ],
        [
            "L U' R2 F B2 R2 L U' D2 R2 F B' U R' U2 R2 D2 F2 U2 L'",
            "BLBLUFFFFDULURFRRULDLBFRDBUFDFBDDDBRDUUFLULRRUDRLBRBLB",
        ],
        [
            "R L2 U2 D2 R2 U2 R' D2 R' F' R L2 B R2 L' F' B2 U' D' F B2 U R' L2",
            "DUULUFBDDRRFURDBFLRBBUFBLBLULURDLLBDFUUFLRBDFRFRLBDFRD",
        ],
        [
            "L2 U2 D' L2 U2 L2 F2 U2 D' F2 B2 L2 U B U2 D R2 U' F2 D R2 L'",
            "BULRUDFDDFLDRRRBFBLFRUFFFBLULUFDLLDRRDURLLBURFBUBBUDBD",
        ],
        [
            "U2 L D2 R2 U2 L F2 B2 D' R' L' F' B2 R' L2 F2 B' R U2 D F B2 R L'",
            "BRRRUBLRBDDFBRRUDRBDRLFLDLFLURDDFUFBLBUBLFLLFDUDFBUUUF",
        ],
        [
            "F U2 D2 R2 F' B2 D2 F B D2 L2 U2 D R F2 B' L' B R L U D2 R2 L'",
            "RFDUUDBFRFBFRRDBBUDRUDFBBDLLLULDUDLLUFRRLRRUDLLBFBUFBF",
        ],
        [
            "U D F' B' L' B R L2 F B R2 L' U F2 B R2 F2 B2 U' F2 B2 U D R2 D'",
            "BDUFULLBFRBRURBFUBFRDUFRUFRFRULDBDLRUDDFLFLULBRLDBLDDB",
        ],
        [
            "R2 D' B2 U' D' R2 U' L2 U' L2 B' R' F D F' B U2 L'",
            "BUDFUDRFDBRFURBFDRDULRFBUBURDLFDLRDDULFULBBRFLRLLBLBFU",
        ],
        [
            "F2 L' D2 F2 B2 R' U2 B2 R' F' B2 R' D F R2 L2 D' R L2 F2 B R2",
            "LFDRUFUBLFRBURDBRRFLDFFLBFLRUUUDUULFDBLDLDFBURLBRBBDDR",
        ],
        [
            "F2 U F2 R2 L2 U2 D' L2 U2 D2 F2 B R2 L U D' L' U2 R' L2 F B2 R'",
            "FBLDUUBFUBFDLRUDLRLDRRFFUDRRLFRDULDDLBDBLUUFFFRUBBLBRB",
        ],
        [
            "R2 B2 D2 R2 U2 R' L2 F' B R2 L' D' F2 B' U' R' L2 F U2 D F B'",
            "BDRDURBLLFFDBRFRBLUUDUFLUDFRBDUDRLFFRRLDLRBBFBFUUBLULD",
        ],
        [
            "F2 U D2 R2 D' F' D R L2 U2 D B2 R' U2 D2 R L2 B2 R' L' U2 L'",
            "FFBRUDBDDRLDRRBLFFUFBBFDBBFRRURDUFURRULLLULFULLDDBBULD",
        ],
        [
            "F2 B2 R2 L' U2 R2 U2 D2 F2 R2 L' F2 L U B2 U2 D' L U2 D2 F' B' D' F2 B2",
            "RUFDUDBFLFLRBRDLUBLLDLFRBFFURUBDLRBRUFDULBFDLURBRBFDUD",
        ],
        [
            "L D2 R' L' D2 F' R' L' F B2 U' D F B' R' U2 L",
            "FRFDUDULRUBDFRBFFRLDBLFRDULRRDLDUBLULFBBLFDBBRDURBUFUL",
        ],
        [
            "U D' B2 U D2 B R' L F B2 R2 L' U D F2 U2 D2 R L2 D2 R2 L B2 L D2 B2",
            "LFFLURDLULBLURURDRBUBDFRRFUDDFFDLBRBUBLULBRLFDRFFBBUDD",
        ],
        [
            "L2 F2 B2 U' L2 U D2 F2 B2 U2 L2 B' U D2 L U D F' U2 B' R2 L'",
            "RBBBURBLLDFRRRFULUUUBDFBRULUFFRDDLUFDLLRLFDUBDDFLBDRBF",
        ],
        [
            "U' R2 L2 F B R' F2 B2 D' F' R U R2 B2 R2 F2 U' R2 L2 U' D",
            "URFLUDULULFLDRRRLBRBBRFBRBDDUBDDULRDBFFFLFFLFDURBBULDU",
        ],
        [
            "L2 F U2 D' R L2 B L' U' D' F' R2 F' D2 F' B' U2 L2 F B2 D2",
            "LBLUULUFBLDUFRUDFRRLDDFRFUBDRRDDDDBBUBFFLBRRLBRFLBUULF",
        ],
        [
            "B' U2 B U2 D R' L' D F B R F' D2 L2 F' R2 L2 F' D2 F2 R2 L2",
            "UFLBURFFLBFBLRRFRRLUUDFUBBDDRRFDDFBBRUULLLLLRDDFUBBUDD",
        ],
        [
            "U2 D2 F2 D2 F2 B2 R2 L' F2 R' L2 B' L' U2 D' B2 R' L' U D2 R L U D2 F B2",
            "RDDBUFRBUBLRURRLFLFLRLFRFBBDUDLDDUFFBRUULDBULFBDDBFURL",
        ],
        [
            "R2 D2 B' D2 F B L2 U2 D2 F2 L' U D F' B R F R' L2 B",
            "RUURULDDRBFLFRRBLLLLDUFRRUUDBRDDBLRBFBFDLFUFFFLUDBBDUB",
        ],
        [
            "D L' F B' R L2 D' R2 F' B R L2 F L2 D2 F2 R2 L2 F' B2 U2 F2 B",
            "DRDRULBDUBFFDRFLBDURRUFLUFFRUUBDUBBBFFLBLLDLFLURDBRRDL",
        ],
        [
            "R L2 U2 R2 L B2 L' F2 U2 R' U2 D' F' R' L2 F2 B R L' F2 R2 L' D' F2",
            "DFBLURFFDBDLDRLUFFLLLDFBDFRFRBRDDDURRBURLLFURUUBUBBUBL",
        ],
        [
            "F B2 R2 L F B R2 L U' F2 B R2 L' F B R2 L2 U' D2 R2 U D2 B2 R2 L2 U R2 L2",
            "URDRULRDLBURLRBFUDURUFFBDDUBLLBDFRRFRUBDLLDULFFFDBFLBB",
        ],
        [
            "R L2 U2 D2 F2 L U' D F B' U L B' L2 B' U2 F2 B R2 L2 D2",
            "LFRLUULLLBBDURRDBBDBUBFFULRFDFRDRFFUFUBFLDRDLBDDUBLRRU",
        ],
        [
            "L2 B2 R2 U' D2 B2 R2 L2 U' D' B2 U' R2 L U F L F2 B' U D2 R' L'",
            "RUBRULRDLFFUFRULFUDLDUFRLBFBRULDDDBFBDFBLRLUURBDFBLRDB",
        ],
        [
            "F U' F' B' L' U F B R F' B D2 F B2 U2 R2 U2 D2 F B",
            "RRDBUFDLBLLFLRRURBRUDLFDLRRUUBUDBRBLDDBDLBUFFLDFFBFUUF",
        ],
        [
            "F U2 F' B' R U2 L U' D' B' R F2 B2 R U2 F2 B2 L F2 U2 D2 R",
            "FLDUUFRLULRLURRUBDDDBRFRBLLDFFUDLRBFUFFFLBBBRBURDBDLDU",
        ],
        [
            "R' L2 B' U D F2 B' R' U2 B' R' L F2 B D2 F2 B' D2 L2 F2 B' R2 U2 D2",
            "UFLLUUBDBRRBFRDFUDDLUUFRFBUULLLDBDDFLFRBLFRURDDBBBRLRF",
        ],
        [
            "L' U2 D2 F2 R' L2 U2 R' F2 B2 R' F2 B2 D F2 B U2 L' U2 D' R' L U' D' F2 B2",
            "RDLLUUDBFLRURRFDDLRLUDFBURLBFBUDBFLDFDBFLRDBRBFUUBLFUR",
        ],
        [
            "F2 R' L' B' U' D R L2 F2 B' R L U2 D' F2 B' R2 F R2 L2 U2 B' U2 R2",
            "BUFFUFULBLDDURDLFRRDDFFBDLULBBBDLFRBDRFRLUUDFRLRRBUUBL",
        ],
        [
            "R L2 U2 R2 L F2 B R' U' D2 R' L U F B L2 U2 L D2 F2 R' F2",
            "RDFUUBFLBLUUDRFUDDRBDUFLUFFLRRBDBBFFBLDLLRURBLRDUBFLDR",
        ],
        [
            "F2 B R' F' R L' U D2 F B2 L U R' D2 R2 F' B2 L2 U2 F' B'",
            "BRDBUURFFDFRURUDLRFDLFFRURBFFRLDUBRBLLUBLLLDLFBDBBDUDU",
        ],
        [
            "U2 F2 B2 R L F2 R L2 U2 R L U D' F' B U F' R L2 F2 R2 L U' D' R L'",
            "FLBDURDBRUFLBRLLDFLUBFFRDRBFUDLDRBURLFFBLLRURUDUBBDUFD",
        ],
        [
            "F B2 R L2 U2 D R L2 D2 R2 L2 F' B' L F2 B' L F2 B2 D2 R L2 B2 R' L2 F2 B2",
            "RLURUURBDFRBURDLBRBURLFFLDDDLFDDLFRBFFDDLUURBLFUFBBUBL",
        ],
        [
            "B2 U2 D2 R L2 D2 R2 L' U2 D2 L B2 R D F2 B2 R2 L' U2 D R2 L2 F' B2 D' R' L2",
            "LUDRUDLRURBRFRLBBDUUFDFUBLLLFUDDRRUBFDFFLLBFDFLDBBRRBU",
        ],
        [
            "R' L2 U2 L2 U2 B2 R L2 U' D2 R L2 F2 B2 U D F' B2 U D' F2 R U D",
            "DLRBUUBRLUBBLRLBFRLFFBFBUDRLLDUDDDUUFRDRLDFRBUFRUBDFFL",
        ],
        [
            "U D2 B' R L' B R2 B' R' U' F' B U2 D2 L2 F2 U2 R2 L",
            "UFDLUFDUURDFLRUBRLFLFLFDDFLRRUBDBBRBFFRRLBUDBLULBBUDDR",
        ],
        [
            "L2 F2 B2 U D2 R2 D R2 L2 F2 R L2 F' B' U D2 R L2 U2 F' B' R' L2 U2 D R' L",
            "LFLFULLDRUDUURBDLFFRBRFLLUBDRRFDFFURUDDLLBDRBBUFDBBUBR",
        ],
        [
            "F B R L' F L U' D' B' U D F B2 D B2 D2 F2 R B2 D2",
            "URFUUDRLLDRRBRFDFLDBBDFRLDBUBRUDLBDDBLFBLLLFFUURRBUFFU",
        ],
        [
            "R' B2 L B2 R' F B U' D2 F' B2 U' D R F' D F B' U D",
            "FRURUDRBFDBBFRLRRDBLLLFDDLBFFUFDUFDLUFDBLDUURLBRUBUBRL",
        ],
        [
            "B R' L' U' F2 B' R' U D2 F2 B' D2 R2 L2 F' R2 F B2 U2 R2 D2 R2 L2",
            "FRLBUDDDFDFURRFLLUBBLFFBBBFDLULDDRRRUULFLRFURBDRUBLBUD",
        ],
        [
            "F2 R2 L' F2 U2 L B2 D2 R L2 U D R2 L' U' F' B U F D' B' R' L2",
            "RUUUURFFLDULBRUBDFDLBRFRBBLDDURDLRBRDLLFLDBFRFFFBBDULU",
        ],
        [
            "U2 D F' B' U' F' R' U D R U' R2 L F2 B' D2 B2 D2 R2 B'",
            "DRURUFBDDBUFFRDFFUDBLUFLFRURBLRDDLBRLDRBLLUUDRFFLBUBLB",
        ],
        [
            "B U' D2 F2 B' U2 D' L' U D2 F2 B R L' U F2 U2 D' F2 U' D' B2 R2 F2",
            "DURRURUFLDFDBRDRDFLUBUFDFRBUUUFDLDDULBBBLBRLRBLFFBLLRF",
        ],
        [
            "L' F B2 L D' R' L' U' F2 B U' R2 F' U2 R2 L2 F D2 B' L2 U2",
            "DUUDUBBDUFURLRFBURUBLRFDFBRDRDBDFBFUFRLRLFDLLBLRDBUFLL",
        ],
        [
            "F2 B2 R' L2 D2 R' U2 D2 F2 R2 L U' D2 L2 D L' F B2 U D2 F2 R L' F'",
            "LUURUFBRBDDRURLLLRLDRLFFDUUFBBBDDLLUUFDDLFDRRBRFBBBFUF",
        ],
        [
            "R F' B' R B2 U F B2 U D F2 B U' D2 R2 B2 D2 F2 D R2",
            "FRDRUUFLBLFBRRFDLRLBDUFDLLFBDLDDUFRBUFULLBDBURURDBFUBR",
        ],
        [
            "D2 R B2 L' D2 R U D2 B2 R' L' U2 D' F U2 R2 L F' R2 L'",
            "LLRLUUDRURLFURFRLDRBFRFBLDBFRUDDFLFLBDBFLUFBDDBUDBUBRU",
        ],
        [
            "U2 D2 R L2 F2 B L D R L B' U' R2 L U2 D F2 L2 U' D' R2 U' R2",
            "LDURUFUURBUBLRBBFUBRDDFFFLDRDLBDRRFLFBRLLRULDLBDUBUFDF",
        ],
        [
            "F2 B2 R' U2 R' F2 D2 R F2 B' R2 D' F U F2 B' R L2 U' D'",
            "LRDDUDBUUFFRDRBBUUDFLRFRLBDBLLFDLULFFBRULURLUFFDRBBRDB",
        ],
        [
            "F2 B R L F' B2 L2 U' D2 R2 L' F L' U D2 F' U2 D2 F B D2 R2 F B2",
            "FLLFULDLFUBBBRFBBULDRUFULFRBDDRDDUULRLFBLRRDUDUDRBRFFB",
        ],
        [
            "B L2 U2 D F B R2 U' D2 R' F' B2 U R2 B2 U' D' B2 D R2 U2 D' R2 L2",
            "BBLUUDLDUBBFURLDFBFLRLFFLRRDBFRDDFRRULDULBRFBUULFBRDDU",
        ],
        [
            "R2 U2 L' B2 R2 L' B2 U2 D2 L' F' B2 U' F' B' U2 D' F' R' U' L",
            "FLUBULFRDBUBFRUBFFUDLDFDDLURBRFDRRBLLURFLBFLBLDURBUDRD",
        ],
        [
            "U F2 B2 U B2 U2 D R' L U' D' F' B2 D F2 B U' R2 F' B2 U2 D'",
            "DDFBUFDRDFURFRFULBRURBFDULLBDFFDUBBDLDBBLULLRURFRBLLRU",
        ],
        [
            "U R' L2 U2 D R L U R B R' L F B U' D2 F2 R2 U' D2 L2 U' R2",
            "ULDBUDRLUFRRURBBDFBULUFFRDLFFURDLLLLBRDULRBFUFBRDBBDFD",
        ],
        [
            "U' F2 U2 F2 B2 U F2 R2 F U' D R' L' U' D2 F2 B D2 F' B2 R' L'",
            "BBRBULUFBUFULRULFLFDLBFURRDUFFLDUFBBDDLRLRDDBFURRBDDLR",
        ],
        [
            "U2 D2 L2 F' B L2 B2 U2 F' R L2 F' B2 U2 D F' U' D2 R U2 D F2",
            "LBDLULDDURDRRRFRURBFFRFBFRDLDBDDBDFBBULRLFFBUFLUUBUULL",
        ],
    ];

    #[test]
    fn test_puzzle_state_with_rob_string() {
        for [seq, rob_string] in TESTS {
            let alg = Algorithm::parse_from_string(Arc::clone(&CUBE3), seq).unwrap();

            assert_eq!(mk_rob_twophase_input(alg.permutation()), rob_string);
        }
    }

    #[test]
    fn rob_twophase_solver() {
        let identity = Permutation::identity();

        for [seq, _] in TESTS {
            let alg = Algorithm::parse_from_string(Arc::clone(&CUBE3), seq).unwrap();

            let solution = solve_rob_twophase(alg.permutation()).unwrap();

            let mut hopefully_identity = alg.permutation().clone();
            hopefully_identity.compose_into(solution.permutation());

            assert_eq!(hopefully_identity, identity);
        }
    }

    #[test]
    fn rob_twophase_error_handling() {
        assert!(solve_rob_twophase_string("UFRBL").is_err());
    }
}
