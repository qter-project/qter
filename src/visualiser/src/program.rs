use std::sync::Arc;

use ariadne::{Report as AriadneReport, ReportKind, Source as AriadneSource, Span as _};
use internment::ArcIntern;
use puzzle_theory::{
    puzzle_geometry::{PuzzleGeometry, parsing::puzzle},
    span::{File, Span},
};
use qter_core::architectures::Architecture;
use serde::Serialize;
use tsify::Tsify;
use wasm_bindgen::prelude::*;

use crate::{BigInt, declare_ts_alias};

#[wasm_bindgen]
pub struct Program {
    pub(crate) inner: Arc<qter_core::Program>,
    pub(crate) registers: Vec<Register>,
    pub(crate) arch: Arc<Architecture>,
    pub(crate) puzzle: Arc<PuzzleGeometry>,
    pub(crate) q_text: File,
    pub(crate) instr_spans: Box<[Span]>,
}

#[wasm_bindgen]
impl Program {
    #[wasm_bindgen(constructor)]
    pub fn new(s: &str) -> Result<Self, Vec<CompileError>> {
        let s = File::new("<inner>".into(), s.into());
        let mk_error = |msg: &str, span: Option<Span>| {
            let span = span.unwrap_or_else(|| Span::new(s.clone(), 0, s.inner().len()));
            vec![CompileError {
                report: AriadneReport::build(ReportKind::Error, span)
                    .with_message(msg)
                    .finish(),
                source: s.clone(),
            }]
        };

        let reporter = compiler::Reporter::default();

        let (program, regs) =
            match compiler::compile(&s, |_| Err("Imports are not allowed".to_owned()), &reporter) {
                Some(v) => v,
                None => {
                    let reports = Arc::try_unwrap(reporter)
                        .expect("reporter should be uniquely owned after compile")
                        .into_iter();
                    return Err(reports
                        .map(|report| CompileError {
                            report,
                            source: s.clone(),
                        })
                        .collect::<Vec<_>>());
                }
            };

        let Some(regs) = regs else {
            return Err(mk_error(
                "No registers declaration supplied. There must be a registers declaration with exactly one 3x3 puzzle.",
                None,
            ));
        };
        let [compiler::Puzzle::Real { architectures }] = regs.puzzles() else {
            return Err(mk_error(
                "You must supply exactly one puzzle",
                Some(regs.span().clone()),
            ));
        };
        let [(names, arch, _)] = &**architectures else {
            return Err(mk_error(
                "Unexpected error: architecture switching",
                Some(regs.span().clone()),
            ));
        };

        let registers = arch
            .registers()
            .iter()
            .enumerate()
            .map(|(i, reg)| Register {
                label: names[i].clone().into_inner(),
                order: BigInt::from(reg.order()),
                cycles: reg
                    .unshared_cycles()
                    .iter()
                    .map(|cycle| RegisterCycle {
                        order: BigInt::from(cycle.chromatic_order()),
                        facelets: cycle.facelet_cycle().iter().map(|&v| v as u8).collect(),
                    })
                    .collect(),
            })
            .collect();

        let (q_text, instr_spans) =
            match compiler::q_emitter::emit_q(&program, "<output>".into(), &reporter) {
                Some(v) => v,
                None => {
                    let reports = Arc::try_unwrap(reporter)
                        .expect("reporter should be uniquely owned after emit_q")
                        .into_iter();
                    return Err(reports
                        .map(|report| CompileError {
                            report,
                            source: s.clone(),
                        })
                        .collect::<Vec<_>>());
                }
            };

        Ok(Self {
            inner: Arc::new(program),
            registers,
            arch: arch.clone().into_inner(),
            puzzle: puzzle("3x3"),
            q_text,
            instr_spans,
        })
    }

    pub fn q_text(&self) -> String {
        (*self.q_text.inner()).to_owned()
    }

    #[wasm_bindgen(unchecked_return_type = "Register[]")]
    pub fn registers(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&self.registers).unwrap()
    }

    pub fn instr_span(&self, idx: usize) -> StartEnd {
        let span = &self.instr_spans[idx];
        let before_start = span.source().inner()[..span.start()].encode_utf16().count();
        let between_start_and_end = span.slice().encode_utf16().count();
        StartEnd {
            start: before_start,
            end: before_start + between_start_and_end,
        }
    }
}

#[wasm_bindgen]
pub struct CompileError {
    report: ariadne::Report<'static, puzzle_theory::span::Span>,
    source: File,
}

#[wasm_bindgen]
impl CompileError {
    // TODO: Fork ariadne to extract span info out of reports?
    pub fn render(&self) -> String {
        let mut buf = std::io::Cursor::new(Vec::<u8>::new());
        self.report
            .write(
                (
                    self.source.name(),
                    AriadneSource::from(&*self.source.inner()),
                ),
                &mut buf,
            )
            .unwrap();
        String::from_utf8(buf.into_inner()).unwrap()
    }
}

declare_ts_alias!(type ArcInternStr = ArcIntern<str> = "string");

fn serialize_arc_intern_str<S: serde::Serializer>(
    s: &ArcIntern<str>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    (**s).serialize(serializer)
}

#[derive(Tsify, Serialize)]
pub struct Register {
    #[serde(serialize_with = "serialize_arc_intern_str")]
    label: ArcInternStr,
    order: BigInt,
    cycles: Vec<RegisterCycle>,
}

#[derive(Tsify, Serialize)]

pub struct RegisterCycle {
    order: BigInt,
    facelets: Vec<u8>,
}

#[derive(Tsify, Serialize)]
#[tsify(into_wasm_abi)]
pub struct StartEnd {
    start: usize,
    end: usize,
}
