#![warn(clippy::pedantic)]
#![allow(
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::single_match_else
)]

use std::{
    collections::HashMap,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicUsize, Ordering},
    },
};

use chumsky::error::Rich;
use internment::ArcIntern;
use parsing::parse;
use puzzle_theory::{
    numbers::{Int, ParseIntError, U},
    span::{File, Span, WithSpan},
};
use qter_core::{Program, architectures::Architecture};
use rhai::RhaiMacros;
use strip_expanded::strip_expanded;

use crate::macro_expansion::expand;

mod builtin_macros;
mod macro_expansion;
mod optimization;
mod parsing;
pub mod q_emitter;
mod rhai;
mod strip_expanded;

/// Compiles a QAT program into a Q program while returning the register architecture used.
///
/// # Errors
///
/// Returns an error if the QAT program is invalid or if the macro expansion fails
pub fn compile(
    qat: &File,
    find_import: impl Fn(&str) -> Result<ArcIntern<str>, String> + 'static,
) -> Result<(Program, Option<WithSpan<RegistersDecl>>), Vec<Rich<'static, char, Span>>> {
    let parsed = parse(qat, find_import, false)?;

    let arch = parsed.expansion_info.registers.clone();

    let expanded = expand(parsed)?;

    strip_expanded(expanded).map(|v| (v, arch))
}

#[expect(clippy::manual_try_fold)] // We are not reimplementing it
pub fn collect_flat_err<C, I, IE>(iter: impl Iterator<Item = Result<I, IE>>) -> Result<C, Vec<IE::Item>>
where
    C: Default + Extend<I::Item>,
    I: IntoIterator,
    IE: IntoIterator,
{
    iter.fold(Ok(C::default()), |acc, v| match (acc, v) {
        (Ok(mut collection), Ok(v)) => {
            collection.extend(v);
            Ok(collection)
        }
        (Ok(_), Err(errs)) => Err(errs.into_iter().collect()),
        (Err(errs), Ok(_)) => Err(errs),
        (Err(mut errs), Err(new_errs)) => {
            errs.extend(new_errs);
            Err(errs)
        }
    })
}

pub fn collect_err<C, V, IE>(iter: impl Iterator<Item = Result<V, IE>>) -> Result<C, Vec<IE::Item>>
where
    C: Default + Extend<V>,
    IE: IntoIterator,
{
    collect_flat_err(iter.map(|v| match v {
        Ok(v) => Ok([v]),
        Err(e) => Err(e),
    }))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Label {
    name: ArcIntern<str>,
    maybe_block_id: Option<BlockID>,
    /// This is only used during parsing. We can't set branch keys during parsing because of chumsky limitations :(
    public: bool,
    branch_key: Option<MacroBranchKey>,
}

#[derive(Hash, PartialEq, Eq, Clone, Debug)]
struct LabelReference {
    name: ArcIntern<str>,
    block_id: BlockID,
    branch_key: Option<MacroBranchKey>,
}

impl LabelReference {
    fn with_branch_key(mut self, branch_key: Option<MacroBranchKey>) -> Self {
        self.branch_key = branch_key;
        self
    }
}

/// This is the mechanism by which we enable private labels in macros. Each macro branch gets its own key and instructions defined in the macro get access to the key and nothing else. Label references are required to have the key to reference labels that require the key.
/// It is the responsibility of the parser to insert branch keys in each branch definition and the responsibility of rhai macro callers to insert a fresh branch key at every call site and into every instruction given by the rhai macro.
#[derive(Hash, PartialEq, Eq, Clone, Copy, Debug)]
struct MacroBranchKey(usize);

type TaggedInstruction = (Instruction, Option<BlockID>, Option<MacroBranchKey>);

fn tag_with_key(instr: &mut TaggedInstruction, key: Option<MacroBranchKey>) {
    instr.2 = key;

    match &mut instr.0 {
        Instruction::Label(label) => {
            if !label.public {
                label.branch_key = key;
            }
        }
        Instruction::Code(Code::Primitive(_))
        | Instruction::Constant(_)
        | Instruction::RhaiCall(_)
        | Instruction::Define(_) => {}
        Instruction::Code(Code::Macro(macro_call)) => {
            for arg in &mut **macro_call.arguments {
                match &mut **arg {
                    Value::Resolved(resolved_value) => match resolved_value {
                        ResolvedValue::Int(_)
                        | ResolvedValue::Ident {
                            ident: _,
                            as_reg: _,
                        } => {}
                        ResolvedValue::Block(block) => {
                            for instr in &mut block.code {
                                tag_with_key(instr, key);
                            }
                        }
                    },
                    Value::Constant(_) => {}
                }
            }
        }
        Instruction::Block(block) => {
            for instr in &mut block.code {
                tag_with_key(instr, key);
            }
        }
    }
}

/// Resolve all defines in this block of code that are found in the table. This is used to resolve macro arguments. Note that macro arguments can't be sugared to define statements because those don't support scoping to a particular macro branch and it would be the case that a constant in a block intended to refer to a macro argument would actually refer to a define statement inside the other macro that the block is an argument of.
fn resolve_just_these_defines(
    this: &mut WithSpan<TaggedInstruction>,
    defines: &HashMap<ArcIntern<str>, Define<WithSpan<ResolvedValue>>>,
) -> Result<(), Vec<Rich<'static, char, Span>>> {
    match &mut this.0 {
        Instruction::Label(_) | Instruction::Define(_) | Instruction::Code(Code::Primitive(_)) => {
            Ok(())
        }
        Instruction::Code(Code::Macro(call)) => collect_err(
            call.arguments
                .iter_mut()
                .map(|arg| arg.resolve_just_these_defines(defines)),
        ),
        Instruction::Constant(arc_intern) => match defines.get(arc_intern) {
            None => Ok(()),
            Some(value) => match &*value.value {
                ResolvedValue::Int(_) => todo!(),
                ResolvedValue::Ident {
                    ident: _,
                    as_reg: _,
                } => todo!(),
                ResolvedValue::Block(block) => {
                    this.0 = Instruction::Block(block.clone());
                    Ok(())
                }
            },
        },
        Instruction::RhaiCall(rhai_call) => collect_err(
            rhai_call
                .args
                .iter_mut()
                .map(|arg| arg.resolve_just_these_defines(defines)),
        ),
        Instruction::Block(block) => collect_err(
            block
                .code
                .iter_mut()
                .map(|instr| resolve_just_these_defines(instr, defines)),
        ),
    }
}

#[derive(Clone, Debug)]
struct Block {
    code: Vec<WithSpan<TaggedInstruction>>,
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

impl ToString for RegisterReference {
    fn to_string(&self) -> String {
        match self.modulus {
            Some(modulus) => format!("{}%{}", &**self.reg_name, modulus),
            None => (**self.reg_name).to_owned(),
        }
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

impl Primitive {
    fn insert_branch_key(self, key: Option<MacroBranchKey>) -> Self {
        match self {
            Primitive::Goto { label } => Primitive::Goto {
                label: label.map(|label| label.with_branch_key(key)),
            },
            Primitive::SolvedGoto { label, register } => Primitive::SolvedGoto {
                label: label.map(|v| v.with_branch_key(key)),
                register,
            },
            _ => self,
        }
    }
}

#[derive(Clone, Debug)]
enum Value {
    Resolved(ResolvedValue),
    Constant(ArcIntern<str>),
}

impl Value {
    fn resolve_just_these_defines(
        &mut self,
        defines: &HashMap<ArcIntern<str>, Define<WithSpan<ResolvedValue>>>,
    ) -> Result<(), Vec<Rich<'static, char, Span>>> {
        match self {
            Value::Resolved(value) => match value {
                ResolvedValue::Int(_)
                | ResolvedValue::Ident {
                    ident: _,
                    as_reg: _,
                } => Ok(()),
                ResolvedValue::Block(block) => collect_err(
                    block
                        .code
                        .iter_mut()
                        .map(|instr| resolve_just_these_defines(instr, defines)),
                ),
            },
            Value::Constant(arc_intern) => match defines.get(arc_intern) {
                None => Ok(()),
                Some(value) => {
                    *self = Value::Resolved(value.value.value.clone());
                    Ok(())
                }
            },
        }
    }
}

type RegisterInfo = (RegisterReference, RhaiRegInfo);

#[derive(Clone, Debug)]
enum ResolvedValue {
    Int(Int<U>),
    Ident {
        ident: WithSpan<ArcIntern<str>>,
        as_reg: OnceLock<Option<RegisterInfo>>,
    },
    Block(Block),
}

impl ResolvedValue {
    /// If this value is not an identifier, returns `None`. If this value is an identifier that does not refer to a register, returns `Some(Err(_))`. Otherwise returns `Some(Ok(_))`
    pub fn as_reg(
        &self,
        info: &ExpansionInfo,
    ) -> Option<Result<&(RegisterReference, RhaiRegInfo), &WithSpan<ArcIntern<str>>>> {
        match self {
            ResolvedValue::Ident { ident, as_reg } => Some(
                as_reg
                    .get_or_init(|| {
                        let reg_ref = RegisterReference::parse(ident.clone()).ok()?;

                        info.register_exists(&reg_ref)
                            .map(|reg_info| (reg_ref, reg_info))
                    })
                    .as_ref()
                    .ok_or(ident),
            ),
            _ => None,
        }
    }
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
struct RhaiCall {
    function_name: WithSpan<ArcIntern<str>>,
    args: Vec<WithSpan<Value>>,
}

impl RhaiCall {
    fn perform(
        self,
        span: Span,
        info: &ExpansionInfo,
        block_id: BlockID,
    ) -> Result<WithSpan<ResolvedValue>, Vec<Rich<'static, char, Span>>> {
        let args = collect_err(
            self.args
                .into_iter()
                .map(|value| info.resolve(DefineValue::Value(value), block_id)),
        )?;

        let rhai = info.rhai_macros.get(&span.source()).unwrap();

        let result = rhai.do_rhai_call(&self.function_name, args, span.clone(), info)?;

        Ok(span.with(result))
    }
}

#[derive(Clone, Debug)]
enum Instruction {
    Label(Label),
    Code(Code),
    Constant(ArcIntern<str>),
    RhaiCall(RhaiCall),
    Define(DefineUnresolved),
    Block(Block),
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

    fn matches(&self, value: &ResolvedValue, info: &ExpansionInfo) -> bool {
        match (self, value) {
            (MacroPatternComponent::Argument { name: _, ty }, value) => match (**ty, value) {
                (MacroArgTy::Int, ResolvedValue::Int(_))
                | (
                    MacroArgTy::Ident,
                    ResolvedValue::Ident {
                        ident: _,
                        as_reg: _,
                    },
                )
                | (MacroArgTy::Block, ResolvedValue::Block(_)) => true,
                (
                    MacroArgTy::Reg,
                    ResolvedValue::Ident {
                        ident: _,
                        as_reg: _,
                    },
                ) => value.as_reg(info).is_some(),
                _ => false,
            },
            (MacroPatternComponent::Word(word), ResolvedValue::Ident { ident, as_reg: _ }) => {
                word == &**ident
            }
            _ => false,
        }
    }

    fn mk_define(
        &self,
        value: WithSpan<ResolvedValue>,
    ) -> Option<(ArcIntern<str>, DefineResolved)> {
        match (self, &*value) {
            (MacroPatternComponent::Argument { name, ty: _ }, _) => Some((
                ArcIntern::clone(name),
                DefineResolved {
                    name: name.clone(),
                    value,
                },
            )),
            (MacroPatternComponent::Word(word), ResolvedValue::Ident { ident, as_reg: _ }) => {
                assert_eq!(word, &**ident);
                None
            }
            _ => unreachable!(),
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

    /// Determines whether a series of arguments matches the pattern. If so, returns a series of define statements to be inserted into the new block. Otherwise, returns the list of arguments unchanged.
    pub fn matches(
        &self,
        components: Vec<WithSpan<ResolvedValue>>,
        info: &ExpansionInfo,
    ) -> Result<Vec<(ArcIntern<str>, DefineResolved)>, Vec<WithSpan<ResolvedValue>>> {
        if components.len() != self.0.len()
            || !self
                .0
                .iter()
                .zip(&components)
                .all(|(pattern, component)| pattern.matches(component, info))
        {
            return Err(components);
        }

        Ok(self
            .0
            .iter()
            .zip(components)
            .filter_map(|(pattern, component)| pattern.mk_define(component))
            .collect())
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
    },
    Builtin(
        fn(
            &ExpansionInfo,
            WithSpan<Vec<WithSpan<Value>>>,
            BlockID,
        ) -> Result<Primitive, Rich<'static, char, Span>>,
    ),
}

#[derive(Clone, Debug)]
enum DefineValue {
    Value(WithSpan<Value>),
    RhaiCall(WithSpan<RhaiCall>),
}

#[derive(Clone, Debug)]
struct Define<V> {
    name: WithSpan<ArcIntern<str>>,
    value: V,
}

type DefineUnresolved = Define<DefineValue>;
type DefineResolved = Define<WithSpan<ResolvedValue>>;

#[derive(Clone, Debug)]
pub enum Puzzle {
    Theoretical {
        name: WithSpan<ArcIntern<str>>,
        order: WithSpan<Int<U>>,
    },
    Real {
        // The extra span is that of the puzzle definition itself
        architectures: Vec<(
            Vec<WithSpan<ArcIntern<str>>>,
            WithSpan<Arc<Architecture>>,
            Span,
        )>,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct BlockID(pub usize);

#[derive(Clone, Debug)]
pub struct RegistersDecl {
    puzzles: Vec<Puzzle>,
}

impl RegistersDecl {
    fn register_exists(&self, reference: &RegisterReference) -> Option<RhaiRegInfo> {
        let reg_name = reference.reg_name.clone();

        let modulus = reference.modulus;

        for puzzle in &self.puzzles {
            match puzzle {
                Puzzle::Theoretical {
                    name: found_name,
                    order,
                } => {
                    if *reg_name == **found_name {
                        return Some(RhaiRegInfo {
                            modulus: modulus.unwrap_or(**order),
                            order: **order,
                        });
                    }
                }
                Puzzle::Real { architectures } => {
                    for (names, arch, _) in architectures {
                        for (i, found_name) in names.iter().enumerate() {
                            if *reg_name == **found_name {
                                let reg = &arch.value.registers()[i];

                                return Some(RhaiRegInfo {
                                    modulus: modulus.unwrap_or(reg.order()),
                                    order: reg.order(),
                                });
                            }
                        }
                    }
                }
            }
        }

        None
    }

    #[must_use]
    pub fn puzzles(&self) -> &[Puzzle] {
        &self.puzzles
    }
}

#[derive(Clone, Debug)]
struct RhaiRegInfo {
    modulus: Int<U>,
    order: Int<U>,
}

#[derive(Debug, Clone)]
struct BlockInfo {
    parent_block: Option<BlockID>,
    child_blocks: Vec<BlockID>,
    defines: HashMap<ArcIntern<str>, DefineResolved>,
    labels: Vec<Label>,
}

#[derive(Debug, Clone)]
struct BlockInfoTracker {
    blocks: HashMap<BlockID, BlockInfo>,
    block_counter: usize,
}

impl BlockInfoTracker {
    /// Resolves a reference to a label. The `block_id` of `reference` is the scope where the label is referenced, and the `block_id` of the return value is the block ID where the label exists. If the label does not require a `MacroBranchKey` to be accessed, that parameter will be set to `None`.
    fn label_scope(&self, reference: &LabelReference) -> Option<LabelReference> {
        let mut current = reference.block_id;

        loop {
            let info = self.blocks.get(&current)?;

            for label in info
                .labels
                .iter()
                .filter(|label| label.name == reference.name)
            {
                if let Some(branch_key) = &label.branch_key {
                    if Some(*branch_key) == reference.branch_key {
                        return Some(LabelReference {
                            name: ArcIntern::clone(&reference.name),
                            block_id: current,
                            branch_key: reference.branch_key,
                        });
                    }
                } else {
                    return Some(LabelReference {
                        name: ArcIntern::clone(&reference.name),
                        block_id: current,
                        branch_key: None,
                    });
                }
            }

            current = info.parent_block?;
        }
    }

    fn get_define(&self, mut block_id: BlockID, name: &ArcIntern<str>) -> Option<&DefineResolved> {
        loop {
            let block = self
                .blocks
                .get(&block_id)
                .expect("the block id to be valid");
            match block.defines.get(name) {
                Some(v) => return Some(v),
                None => block_id = block.parent_block?,
            }
        }
    }

    fn resolve(&self, block_id: BlockID, value: Value) -> Option<ResolvedValue> {
        Some(match value {
            Value::Resolved(resolved) => resolved,
            Value::Constant(arc_intern) => (*self.get_define(block_id, &arc_intern)?.value).clone(),
        })
    }

    fn resolve_ref<'a>(&'a self, block_id: BlockID, value: &'a Value) -> Option<&'a ResolvedValue> {
        Some(match value {
            Value::Resolved(resolved) => resolved,
            Value::Constant(arc_intern) => &self.get_define(block_id, arc_intern)?.value,
        })
    }

    fn new_block(&mut self, parent: BlockID) -> (BlockID, &mut BlockInfo) {
        let new_id = BlockID(self.block_counter);
        self.block_counter += 1;

        self.blocks
            .get_mut(&parent)
            .unwrap()
            .child_blocks
            .push(new_id);

        self.blocks.insert(
            new_id,
            BlockInfo {
                parent_block: Some(parent),
                child_blocks: Vec::new(),
                defines: HashMap::new(),
                labels: Vec::new(),
            },
        );

        (new_id, self.blocks.get_mut(&new_id).unwrap())
    }
}

#[derive(Clone, Debug)]
struct ExpansionInfo {
    registers: Option<WithSpan<RegistersDecl>>,
    // Each block gets an ID and `block_parent` maps a block ID to it's parent
    // The global scope is block zero and if the block/label hasn't been expanded its ID is None
    block_info: BlockInfoTracker,
    /// Map (file contents containing macro definition, macro name) to a macro
    macros: HashMap<(File, ArcIntern<str>), WithSpan<Macro>>,
    /// Map each (file contents containing macro call, macro name) to the file contents that the macro definition is in
    available_macros: HashMap<(File, ArcIntern<str>), File>,
    /// Each file has its own `LuaMacros`; use the file contents as the key
    rhai_macros: HashMap<File, RhaiMacros>,
    /// Counter to give fresh instances of `MacroBranchKey`
    branch_count: Arc<AtomicUsize>,
}

impl ExpansionInfo {
    fn register_exists(&self, reference: &RegisterReference) -> Option<RhaiRegInfo> {
        match &self.registers {
            Some(regs) => regs.register_exists(reference),
            None => None,
        }
    }

    fn resolve(
        &self,
        value: DefineValue,
        block_id: BlockID,
    ) -> Result<WithSpan<ResolvedValue>, Vec<Rich<'static, char, Span>>> {
        match value {
            DefineValue::Value(v) => {
                let span = v.span().clone();
                let value = v.into_inner();
                let Some(resolved) = self.block_info.resolve(block_id, value) else {
                    return Err(vec![Rich::custom(span, "Constant not found in this scope")]);
                };

                Ok(span.with(resolved))
            }
            DefineValue::RhaiCall(call) => {
                let span = call.span().clone();
                call.into_inner().perform(span, self, block_id)
            }
        }
    }

    fn fresh_branch_key(&self) -> impl Fn() -> MacroBranchKey + 'static {
        let branch_count = Arc::clone(&self.branch_count);

        move || {
            let num = branch_count.fetch_add(1, Ordering::Relaxed);
            MacroBranchKey(num)
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

#[cfg(test)]
mod tests {
    use internment::ArcIntern;
    use puzzle_theory::span::File;

    use crate::{compile, q_emitter::emit_q};

    #[test]
    fn test_define() {
        let code = "
            .registers {
                A <- 3x3 (U)
            }

            .define one 1
            .define var A

            .define X {
                add $var $one
            }
            .define Y $X
            .define Z $Y

            $X
            $Y
            $Z
        ";

        let (program, _) = match compile(
            &File::new(ArcIntern::from("code.qat"), ArcIntern::from(code)),
            |_| unreachable!(),
        ) {
            Ok(v) => v,
            Err(e) => panic!("{e:?}"),
        };

        let q_code = emit_q(&program, "code.q".into()).unwrap().0;

        assert_eq!(
            q_code.inner(),
            r"Puzzles
A: 3x3

0 | U'
"
        );
    }

    #[test]
    fn test_recursion_limit() {
        let code = "
            .registers {
                A <- 3x3 (U)
            }

            .define X {
                $X
            }

            $X
        ";

        match compile(
            &File::new(ArcIntern::from("code.qat"), ArcIntern::from(code)),
            |_| unreachable!(),
        ) {
            Ok(v) => panic!("{v:?}"),
            Err(e) => {
                assert_eq!(e.len(), 1);

                assert_eq!(e[0].span().line(), 7);
                assert_eq!(e[0].span().slice(), "$X");
            }
        }
    }
}
