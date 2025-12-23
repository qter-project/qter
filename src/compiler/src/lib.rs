#![warn(clippy::pedantic)]
#![allow(
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::single_match_else
)]

use std::{collections::HashMap, sync::Arc};

use chumsky::error::Rich;
use internment::ArcIntern;
use lua::LuaMacros;
use parsing::parse;
use puzzle_theory::{numbers::{Int, ParseIntError, U}, span::{File, Span, WithSpan}};
use qter_core::{
    Program, architectures::Architecture,
};
use strip_expanded::strip_expanded;

use crate::macro_expansion::expand;

mod builtin_macros;
mod lua;
mod macro_expansion;
mod optimization;
mod parsing;
mod strip_expanded;

/// Compiles a QAT program into a Q program
///
/// # Errors
///
/// Returns an error if the QAT program is invalid or if the macro expansion fails
pub fn compile(
    qat: &File,
    find_import: impl Fn(&str) -> Result<ArcIntern<str>, String> + 'static,
) -> Result<Program, Vec<Rich<'static, char, Span>>> {
    let parsed = parse(qat, find_import, false)?;

    let expanded = expand(parsed)?;

    strip_expanded(expanded)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Label {
    name: ArcIntern<str>,
    public: bool,
    maybe_block_id: Option<BlockID>,
    available_in_blocks: Option<Vec<BlockID>>,
}

#[derive(Hash, PartialEq, Eq, Clone, Debug)]
struct LabelReference {
    name: ArcIntern<str>,
    block_id: BlockID,
}

type TaggedInstruction = (Instruction, Option<BlockID>);

#[derive(Clone, Debug)]
struct Block {
    code: Vec<WithSpan<TaggedInstruction>>,
    maybe_id: Option<BlockID>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RegisterReference {
    reg_name: WithSpan<ArcIntern<str>>,
    modulus: Option<Int<U>>,
}

impl RegisterReference {
    fn parse(name: WithSpan<ArcIntern<str>>) -> Result<RegisterReference, ParseIntError<U>> {
        match Self::try_parse_mod(&name) {
            Some(Ok((s, mod_))) => Ok(RegisterReference {
                reg_name: WithSpan::new(ArcIntern::from(s), name.span().to_owned()),
                modulus: Some(mod_),
            }),
            Some(Err(e)) => Err(e),
            None => Ok(RegisterReference {
                reg_name: name,
                modulus: None,
            }),
        }
    }

    fn try_parse_mod(name: &str) -> Option<Result<(&str, Int<U>), ParseIntError<U>>> {
        let idx = name.rfind('%')?;
        let num = match name[idx + 1..].parse::<Int<U>>() {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        };
        Some(Ok((&name[0..idx], num)))
    }
}

#[derive(Clone, Debug)]
enum Primitive {
    Add {
        amt: WithSpan<Int<U>>,
        register: RegisterReference,
    },
    Goto {
        label: WithSpan<LabelReference>,
    },
    SolvedGoto {
        label: WithSpan<LabelReference>,
        register: RegisterReference,
    },
    Input {
        message: WithSpan<String>,
        register: RegisterReference,
    },
    Halt {
        message: WithSpan<String>,
        register: Option<RegisterReference>,
    },
    Print {
        message: WithSpan<String>,
        register: Option<RegisterReference>,
    },
}

#[derive(Clone, Debug)]
enum Value {
    Int(Int<U>),
    Constant(ArcIntern<str>),
    Ident(ArcIntern<str>),
    Block(Block),
}

#[derive(Clone, Debug)]
struct MacroCall {
    name: WithSpan<ArcIntern<str>>,
    arguments: WithSpan<Vec<WithSpan<Value>>>,
}

#[derive(Clone, Debug)]
enum Code {
    Primitive(Primitive),
    Macro(MacroCall),
}

#[derive(Clone, Debug)]
struct LuaCall {
    function_name: WithSpan<ArcIntern<str>>,
    args: Vec<WithSpan<Value>>,
}

#[derive(Clone, Debug)]
enum Instruction {
    Label(Label),
    Code(Code),
    Constant(ArcIntern<str>),
    LuaCall(LuaCall),
    Define(Define),
}

#[derive(Clone, Copy, Debug)]
enum MacroArgTy {
    Int,
    Reg,
    Block,
    Ident,
}

#[derive(Clone, Debug)]
enum MacroPatternComponent {
    Argument {
        name: WithSpan<ArcIntern<str>>,
        ty: WithSpan<MacroArgTy>,
    },
    Word(ArcIntern<str>),
}

impl MacroPatternComponent {
    /// Returns `None` if the patterns do not conflict, otherwise returns a counterexample that would match both patterns.
    fn conflicts_with(&self, other: &MacroPatternComponent) -> Option<ArcIntern<str>> {
        use MacroArgTy as A;
        use MacroPatternComponent as P;

        match (self, other) {
            (P::Argument { name: _, ty: a }, P::Argument { name: _, ty: b }) => match (**a, **b) {
                (A::Int, A::Int) => Some(ArcIntern::from("123")),
                (A::Reg | A::Ident, A::Reg | A::Ident) => Some(ArcIntern::from("a")),
                (A::Block, A::Block) => Some(ArcIntern::from("{ }")),
                _ => None,
            },
            (P::Argument { name: _, ty }, P::Word(word))
            | (P::Word(word), P::Argument { name: _, ty }) => match **ty {
                A::Ident | A::Reg => Some(ArcIntern::clone(word)),
                _ => None,
            },
            (P::Word(a), P::Word(b)) => (a == b).then(|| ArcIntern::clone(a)),
        }
    }
}

#[derive(Clone, Debug)]
struct MacroPattern(Vec<WithSpan<MacroPatternComponent>>);

impl MacroPattern {
    /// Returns `None` if the patterns do not conflict, otherwise returns a counterexample that would match both patterns.
    pub fn conflicts_with(&self, macro_name: &str, other: &MacroPattern) -> Option<String> {
        if self.0.len() != other.0.len() {
            return None;
        }

        self.0
            .iter()
            .zip(other.0.iter())
            .map(|(a_component, b_component)| a_component.conflicts_with(b_component))
            .try_fold(String::new(), |mut acc, maybe_counterexample| {
                let counterexample = maybe_counterexample?;

                acc.push(' ');
                acc.push_str(&counterexample);
                Some(acc)
            })
            .map(|e| format!("{macro_name}{e}"))
    }
}

#[derive(Clone, Debug)]
struct MacroBranch {
    pattern: WithSpan<MacroPattern>,
    code: Vec<WithSpan<TaggedInstruction>>,
}

#[derive(Clone, Debug)]
enum Macro {
    UserDefined {
        branches: Vec<WithSpan<MacroBranch>>,
        after: Option<WithSpan<ArcIntern<str>>>,
    },
    Builtin(
        fn(
            &ExpansionInfo,
            WithSpan<Vec<WithSpan<Value>>>,
            BlockID,
        ) -> Result<Vec<Instruction>, Rich<'static, char, Span>>,
    ),
}

#[derive(Clone, Debug)]
enum ValueOrReg {
    Value(Value),
    Register(RegisterReference),
}

#[derive(Clone, Debug)]
enum DefineValue {
    Value(WithSpan<Value>),
    LuaCall(WithSpan<LuaCall>),
}

#[derive(Clone, Debug)]
struct Define {
    name: WithSpan<ArcIntern<str>>,
    value: DefineValue,
}

#[derive(Clone, Debug)]
enum Puzzle {
    Theoretical {
        name: WithSpan<ArcIntern<str>>,
        order: WithSpan<Int<U>>,
    },
    Real {
        architectures: Vec<(Vec<WithSpan<ArcIntern<str>>>, WithSpan<Arc<Architecture>>)>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct BlockID(pub usize);

#[derive(Clone, Debug)]
struct RegistersDecl {
    puzzles: Vec<Puzzle>,
}

impl RegistersDecl {
    fn get_register(&self, reference: &RegisterReference) -> Option<(RegisterReference, &Puzzle)> {
        let reg_name = reference.reg_name.clone();

        for puzzle in &self.puzzles {
            match puzzle {
                Puzzle::Theoretical {
                    name: found_name,
                    order: _,
                } => {
                    if *reg_name == **found_name {
                        return Some((
                            RegisterReference {
                                reg_name,
                                modulus: reference.modulus,
                            },
                            puzzle,
                        ));
                    }
                }
                Puzzle::Real { architectures } => {
                    for (names, _) in architectures {
                        for found_name in names {
                            if *reg_name == **found_name {
                                return Some((
                                    RegisterReference {
                                        reg_name,
                                        modulus: reference.modulus,
                                    },
                                    puzzle,
                                ));
                            }
                        }
                    }
                }
            }
        }

        None
    }
}

#[derive(Debug, Clone)]
struct BlockInfo {
    parent_block: Option<BlockID>,
    child_blocks: Vec<BlockID>,
    defines: Vec<Define>,
    labels: Vec<Label>,
}

#[derive(Debug, Clone)]
struct BlockInfoTracker(HashMap<BlockID, BlockInfo>);

impl BlockInfoTracker {
    fn label_scope(&self, reference: &LabelReference) -> Option<LabelReference> {
        let mut current = reference.block_id;

        loop {
            let info = self.0.get(&current)?;

            for label in info
                .labels
                .iter()
                .filter(|label| label.name == reference.name)
            {
                if let Some(available_in) = &label.available_in_blocks {
                    if available_in.contains(&reference.block_id) {
                        return Some(LabelReference {
                            name: ArcIntern::clone(&reference.name),
                            block_id: current,
                        });
                    }
                } else {
                    return Some(LabelReference {
                        name: ArcIntern::clone(&reference.name),
                        block_id: current,
                    });
                }
            }

            current = info.parent_block?;
        }
    }
}

#[derive(Clone, Debug)]
struct ExpansionInfo {
    registers: Option<WithSpan<RegistersDecl>>,
    // Each block gets an ID and `block_parent` maps a block ID to it's parent
    // The global scope is block zero and if the block/label hasn't been expanded its ID is None
    block_counter: usize,
    block_info: BlockInfoTracker,
    /// Map (file contents containing macro definition, macro name) to a macro
    macros: HashMap<(ArcIntern<str>, ArcIntern<str>), WithSpan<Macro>>,
    /// Map each (file contents containing macro call, macro name) to the file contents that the macro definition is in
    available_macros: HashMap<(ArcIntern<str>, ArcIntern<str>), ArcIntern<str>>,
    /// Each file has its own `LuaMacros`; use the file contents as the key
    lua_macros: HashMap<ArcIntern<str>, LuaMacros>,
}

impl ExpansionInfo {
    fn get_register(&self, reference: &RegisterReference) -> Option<(RegisterReference, &Puzzle)> {
        match &self.registers {
            Some(regs) => regs.get_register(reference),
            None => None,
        }
    }
}

#[derive(Clone, Debug)]
struct ParsedSyntax {
    expansion_info: ExpansionInfo,
    code: Vec<WithSpan<TaggedInstruction>>,
}

#[derive(Clone, Debug)]
enum ExpandedCodeComponent {
    Instruction(Box<Primitive>, BlockID),
    Label(Label),
}

#[derive(Clone, Debug)]
struct ExpandedCode {
    registers: RegistersDecl,
    block_info: BlockInfoTracker,
    expanded_code_components: Vec<WithSpan<ExpandedCodeComponent>>,
}
