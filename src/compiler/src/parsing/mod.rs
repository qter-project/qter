use std::rc::Rc;

use ariadne::{Label, Report, ReportKind, Source};
use qter_core::span::{File, Span, WithSpan};
use sarlacc::Intern;

use crate::{
    BlockID, ParsedSyntax, Reporter, builtin_macros::builtin_macros, parsing::level_two::tokenize,
};

mod level_two;
mod parser;
mod tokenizer;

thread_local! {
    static PRELUDE: ParsedSyntax = {
        let prelude = File::new(Intern::from_ref("prelude.qat"), Intern::from_ref(include_str!("../../../qter_core/prelude.qat")));

        let reporter = Reporter::default();

        let Some(mut parsed_prelude) = parse(
            prelude,
            &Rc::new(|_: &str| {
                panic!(
                    "Prelude should not import files (because it's easier not to implement; message henry if you need this feature)"
                )
            }),
            true,
            &reporter,
        ) else {
            for report in reporter.iter() {
                report.1.eprint((Intern::from_ref("prelude.qat"), Source::from(prelude.inner()))).unwrap();
            }

            panic!("Failed building the prelude with {} errors", reporter.count())
        };

        let builtin_macros = builtin_macros(prelude);
        parsed_prelude
            .expansion_info
            .available_macros
            .extend(builtin_macros.keys().map(|source_and_macro_name| {
                (
                    *source_and_macro_name,
                    source_and_macro_name.0,
                )
            }));
        parsed_prelude.expansion_info.macros.extend(builtin_macros);

        parsed_prelude.into_inner()
    };
}

pub fn parse(
    qat: File,
    find_import: &Rc<impl Fn(&str) -> Result<Intern<str>, String> + 'static>,
    is_prelude: bool,
    reporter: &Reporter,
) -> Option<WithSpan<ParsedSyntax>> {
    let enclosure = tokenize(qat, reporter.clone())?;

    let v = enclosure.parse(|iter| {
        assert!(iter.r().ptr_eq(reporter));
        parser::parse(iter, find_import, is_prelude)
    });

    if reporter.count() != 0 { None } else { v }
}

fn merge_files(
    importer: &mut ParsedSyntax,
    importer_contents: File,
    mut importee: ParsedSyntax,
    span: Span,
    reporter: &Reporter,
) -> Option<()> {
    match (
        &importer.expansion_info.registers,
        importee.expansion_info.registers,
    ) {
        (None, Some(regs)) => importer.expansion_info.registers = Some(regs),
        (Some(regs1), Some(regs2)) => {
            reporter.push(
                Report::build(ReportKind::Error, span)
                    .with_message("Importing this file introduces a second registers declaration")
                    .with_note("A QAT program may only contain one registers declaration")
                    .with_label(
                        Label::new(regs1.span()).with_message("Current registers declaration"),
                    )
                    .with_label(
                        Label::new(regs2.span()).with_message("Introduced registers declaration"),
                    )
                    .finish(),
            );
            return None;
        }
        (_, None) => {}
    }

    // Block numbers shouldn't be defined deeper than the root in this stage
    let block_offset = importer.expansion_info.block_info.block_counter;

    let mut max_block = 0;

    for (block_id, block_info) in importee.expansion_info.block_info.blocks {
        max_block = max_block.max(block_id.0);

        importer
            .expansion_info
            .block_info
            .blocks
            .insert(BlockID(block_id.0 + block_offset), block_info);
    }

    importer
        .expansion_info
        .macros
        .extend(importee.expansion_info.macros);
    for (source_and_macro_name, macro_file) in importee.expansion_info.available_macros {
        // Imports should not shadow existing macros
        importer
            .expansion_info
            .available_macros
            .entry((importer_contents, source_and_macro_name.1))
            .or_insert_with(|| macro_file);

        importer
            .expansion_info
            .available_macros
            .insert(source_and_macro_name, macro_file);
    }
    importer
        .expansion_info
        .rhai_macros
        .extend(importee.expansion_info.rhai_macros);

    importee.code.iter_mut().for_each(|tagged_instruction| {
        if let Some(block_id) = &mut tagged_instruction.1 {
            block_id.0 += block_offset;
        }
    });
    importer.code.extend(importee.code);

    Some(())
}

#[cfg(test)]
pub(crate) mod tests {
    use std::rc::Rc;

    use qter_core::span::File;
    use sarlacc::Intern;

    use super::parse;
    use crate::Reporter;

    pub(crate) fn file(str: &'static str) -> File {
        File::new(Intern::from_ref("<static>"), Intern::from_ref(str))
    }

    #[test]
    fn bruh() {
        let code = "
            .registers {
                A, B ← 3x3 builtin ( 90 , 90 )
                /*(
                    c, d ← 3x3 builtin (210, 24)
                    d, e, f ← 3x3 builtin (30, 30, 30)
                )*/
                G, H ← 3x3 (U , D    )
            }

            .macro bruh {
                ( lmao $a:reg) => add 1 $a
                (oofy $a:reg ) => {
                    bruh:
                    add 1 $a
                    goto bruh
                }
            }

            .start-rhai
                fn bruh() {
                    print(\"skibidi\")
                }
            .end-rhai

            bruh:
            bruhy:
            add 1 A
            goto bruh

            rhai bruh( 1,2 , 3)

            .define $yeet rhai bruh(1, 2, 3)
            .define $pog 4

            .import \"pog.qat\"
        ";

        let reporter = Reporter::default();

        match parse(
            file(code),
            &Rc::new(|name: &str| {
                assert_eq!(name, "pog.qat");
                Ok(Intern::from_ref("add 1 a"))
            }),
            false,
            &reporter,
        ) {
            Some(_) => {}
            None => {
                let mut cache = ariadne::sources(
                    [("pog.qat", "add 1 a"), ("<static>", code)]
                        .into_iter()
                        .map(|(a, b)| (Intern::from_ref(a), Intern::from_ref(b))),
                );

                for (_, report) in reporter.iter() {
                    report.eprint(&mut cache).unwrap();
                }

                panic!();
            }
        }
    }

    #[test]
    fn conflicting_macro_branches() {
        let options = [
            (
                "
                    .macro six_seven {
                        (six) => six
                        ($seven:ident) => {}
                    }
                ",
                Some("six_seven six"),
            ),
            (
                "
                    .macro six_seven {
                        (six) => six
                        ($seven:reg) => {}
                    }
                ",
                None,
            ),
            (
                "
                    .macro easy {
                        (six $a:int) => six
                        (six $b:int) => {}
                    }
                ",
                Some("easy six 123"),
            ),
            (
                "
                    .macro easy {
                        (seven $a:ident) => six
                        (seven $b:ident) => {}
                    }
                ",
                Some("easy seven skibidi"),
            ),
            (
                "
                    .macro easy {
                        ($a:reg) => six
                        ($b:reg) => {}
                    }
                ",
                Some("easy A"),
            ),
            (
                "
                    .macro easy {
                        ($a:block $q:reg) => six
                        ($b:block $q:reg) => {}
                    }
                ",
                Some("easy { } A"),
            ),
        ];

        for (code, counterexample) in options {
            let reporter = Reporter::default();

            match parse(file(code), &Rc::new(|_: &str| panic!()), false, &reporter) {
                Some(_) => {
                    assert!(counterexample.is_none());
                }
                None => {
                    let Some(counterexample) = counterexample else {
                        panic!()
                    };

                    let mut cache = ariadne::sources(
                        [("<static>", code)]
                            .into_iter()
                            .map(|(a, b)| (Intern::from_ref(a), Intern::from_ref(b))),
                    );

                    assert_eq!(reporter.count(), 1);

                    for (_, report) in reporter.iter() {
                        let mut data = Vec::<u8>::new();
                        report.write(&mut cache, &mut data).unwrap();
                        let st = str::from_utf8(&data).unwrap();
                        assert!(st.contains(counterexample), "{st}");
                    }
                }
            }
        }
    }
}
