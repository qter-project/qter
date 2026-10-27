#![no_main]

use compiler::{Reporter, compile, q_emitter::emit_q};
use sarlacc::Intern;
use libfuzzer_sys::fuzz_target;
use qter_core::span::File;

fuzz_target!(|code: &str| {
    let reporter = Reporter::default();
    let compiled = compile(File::new(Intern::from_ref("<fuzzing>"), Intern::from_ref(code)), |n| {
        if n.len() % 2 == 0 {
            Ok(Intern::from_ref("add A 1"))
        } else {
            Err("File not found".to_owned())
        }
    }, &reporter);

    let program = match compiled {
        Some((p, _)) => {
            if !reporter.is_empty() {
                println!("`{code}`");
                println!("{p:?}");

                let mut cache = ariadne::sources(
                    [("<static>", code)]
                        .into_iter()
                        .map(|(a, b)| (Intern::from_ref(a), Intern::from_ref(b))),
                );

                for report in reporter.iter() {
                    report.1.print(&mut cache).unwrap();
                }
                
                panic!()
            }
            p
        },
        None => {
            assert!(!reporter.is_empty(), "`{code}`");
            return
        },
    };

    let reporter = Reporter::default();
    let q_code = emit_q(&program, Intern::from_ref("<compiled-q>"), &reporter);

    match q_code {
        Some((_, _)) => assert!(reporter.is_empty()),
        None => assert!(!reporter.is_empty()),
    }
});
