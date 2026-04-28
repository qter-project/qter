use ariadne::{Label, Report, ReportKind};
use internment::ArcIntern;
use puzzle_theory::span::{File, Span, WithSpan};

use crate::{BlockID, ParsedSyntax, Reporter, builtin_macros::builtin_macros, parsing::tokenizer::{TokenEnclosure, TokenizerState}};

mod parser;
mod tokenizer;

thread_local! {
    static PRELUDE: ParsedSyntax = {
        let prelude = File::new(ArcIntern::from("prelude.qat"), ArcIntern::from(include_str!("../../../qter_core/prelude.qat")));

        let reporter = Reporter::default();

        let mut parsed_prelude = match parse(
            &prelude,
            |_| {
                panic!(
                    "Prelude should not import files (because it's easier not to implement; message henry if you need this feature)"
                )
            },
            true,
            reporter.clone(),
        ) {
            Some(v) => v,
            None => {
                for report in reporter.iter() {
                    // report.1.eprint();
                }

                panic!()
            },
        };

        let builtin_macros = builtin_macros(&prelude);
        parsed_prelude
            .expansion_info
            .available_macros
            .extend(builtin_macros.keys().map(|source_and_macro_name| {
                (
                    source_and_macro_name.to_owned(),
                    source_and_macro_name.0.clone(),
                )
            }));
        parsed_prelude.expansion_info.macros.extend(builtin_macros);

        parsed_prelude.into_inner()
    };
}

pub fn parse(
    qat: &File,
    find_import: impl Fn(&str) -> Result<ArcIntern<str>, String> + 'static,
    is_prelude: bool,
    reporter: Reporter,
) -> Option<WithSpan<ParsedSyntax>> {
    let mut state = TokenizerState::new(qat.clone(), reporter);
    let enclosure = TokenEnclosure::new(&mut state);

    enclosure.parse(|iter| {
        parser::parse(iter, find_import, is_prelude)
    })
}

fn merge_files(
    importer: &mut ParsedSyntax,
    importer_contents: &File,
    mut importee: ParsedSyntax,
    span: Span,
    reporter: Reporter,
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
                    .with_label(Label::new(regs1.span().clone()).with_message("Current registers declaration"))
                    .with_label(Label::new(regs2.span().clone()).with_message("Introduced registers declaration"))
                    .finish(),
            );
            return None;
        },
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
            .entry((
                importer_contents.clone(),
                ArcIntern::clone(&source_and_macro_name.1),
            ))
            .or_insert_with(|| macro_file.clone());

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
    use chumsky::Parser;
    use internment::ArcIntern;
    use puzzle_theory::span::File;

    use super::{ident, number, parse, registers};

    pub(crate) fn file(str: &'static str) -> File {
        File::new(ArcIntern::from("<static>"), ArcIntern::from(str))
    }

    #[test]
    fn test_number() {
        number::<()>().parse(file("123")).unwrap();
        number::<()>().parse(file("12398263596868928956891896286935689869218695689689297479561963469856981968423679569173479159")).unwrap();

        assert!(number::<()>().parse(file("")).has_errors());
        assert!(number::<()>().parse(file("3x3")).has_errors());
        assert!(number::<()>().parse(file("0.12")).has_errors());
        assert!(number::<()>().parse(file("-11")).has_errors());
        assert!(number::<()>().parse(file("-11")).has_errors());
    }

    #[test]
    fn test_ident() {
        ident::<()>().parse(file("a")).unwrap();
        ident::<()>().parse(file("A")).unwrap();
        ident::<()>().parse(file("3x3")).unwrap();
        ident::<()>().parse(file("thingy")).unwrap();
        ident::<()>().parse(file("prhaih")).unwrap();
        ident::<()>().parse(file("->")).unwrap();
        ident::<()>().parse(file("\"345\"")).unwrap();
        ident::<()>().parse(file("\"rhai\"")).unwrap();

        assert!(ident::<()>().parse(file("345")).has_errors());
        assert!(ident::<()>().parse(file("rhai")).has_errors());
        assert!(ident::<()>().parse(file("thing<-thing")).has_errors());
        assert!(ident::<()>().parse(file("aa.aa")).has_errors());
        assert!(ident::<()>().parse(file("!aaaa")).has_errors());
        assert!(ident::<()>().parse(file("aaaa)")).has_errors());
    }

    #[test]
    fn test_registers() {
        let code = "
            .registers {
                a, b <- 3x3 builtin (90, 90)
                (
                    c, d ← 3x3 builtin (210, 24)
                    d, e, f ← 3x3 builtin (30, 30, 30)
                )
                f ← theoretical 90
                g, h ← 3x3 (U, D)
            }
        ";

        let errs = registers().parse(file(code)).into_errors();

        for err in &errs {
            println!("{err}; {:?}", err.span().line_and_col());
        }

        assert!(errs.is_empty());
    }

    #[test]
    fn bruh() {
        let code = "
            .registers {
                a, b ← 3x3 builtin ( 90 , 90 )
                (
                    c, d ← 3x3 builtin (210, 24)
                    d, e, f ← 3x3 builtin (30, 30, 30)
                )
                f ← theoretical 90
                g, h ← 3x3 (U , D    )
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
            end-rhai

            bruh :
            bruhy:
            add 1 a
            goto bruh

            rhai bruh( 1,2 , 3)

            .define yeet rhai bruh(1, 2, 3)
            .define pog 4

            .import pog.qat
            .import \"pog.qat\"
        ";

        match parse(
            &file(code),
            |name| {
                assert_eq!(name, "pog.qat");
                Ok(ArcIntern::from("add 1 a"))
            },
            false,
        ) {
            Ok(_) => {}
            Err(errs) => {
                for err in &errs {
                    println!(
                        "{err}; {:?}; `{}`",
                        err.span().line_and_col(),
                        err.span().slice()
                    );
                }

                panic!();
            }
        }
    }
}
