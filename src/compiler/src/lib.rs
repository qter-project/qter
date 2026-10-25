#![warn(clippy::pedantic)]
#![allow(
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::single_match_else,
    clippy::enum_glob_use,
    clippy::question_mark
)]

use std::{
    collections::HashMap,
    fmt::Display,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use ariadne::{Report, ReportKind};
use internment::ArcIntern;
use parsing::parse;
use puzzle_theory::{
    numbers::{Int, U},
};
use qter_core::{Program, architectures::Architecture, span::{File, Span, WithSpan}};
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

#[derive(Clone, Default, Debug)]
pub struct Reporter(Arc<boxcar::Vec<Report<'static, Span>>>);

impl Reporter {
    pub fn push(&self, report: Report<'static, Span>) {
        self.0.push(report);
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.0.count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }

    #[must_use]
    pub fn iter(&self) -> boxcar::vec::Iter<'_, Report<'static, Span>> {
        self.0.iter()
    }

    #[must_use]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    #[must_use]
    pub fn drain(mut self) -> impl IntoIterator<Item = Report<'static, Span>> {
        Arc::try_unwrap(std::mem::take(&mut self.0)).expect("the reporter not to be kept")
    }
}

/// Compiles a QAT program into a Q program while returning the register architecture used.
///
/// # Errors
///
/// Returns an error if the QAT program is invalid or if the macro expansion fails
pub fn compile(
    qat: &File,
    find_import: impl Fn(&str) -> Result<ArcIntern<str>, String> + 'static,
    reporter: &Reporter,
) -> Option<(Program, Option<WithSpan<RegistersDecl>>)> {
    let parsed = parse(qat, &Rc::new(find_import), false, reporter)?;

    let arch = parsed.expansion_info.registers.clone();

    let expanded = expand(parsed.into_inner(), reporter)?;

    strip_expanded(expanded, reporter).map(|v| (v, arch))
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
                        | ResolvedValue::Register(_)
                        | ResolvedValue::Ident(_) => {}
                        ResolvedValue::Block(block) => {
                            for instr in &mut block.code {
                                tag_with_key(instr, key);
                            }
                        }
                    },
                    Value::Constant { name: _, mods: _ } => {}
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

/// Resolve all defines in this block of code that are found in the table. This is used to resolve macro arguments. Note that macro arguments can't be desugared to define statements because those don't support scoping to a particular macro branch and it would be the case that a constant in a block intended to refer to a macro argument would actually refer to a define statement inside the other macro that the block is an argument of.
fn resolve_just_these_defines(
    this: &mut WithSpan<TaggedInstruction>,
    defines: &HashMap<ArcIntern<str>, Define<WithSpan<ResolvedValue>>>,
    r: &Reporter,
) -> Option<()> {
    match &mut this.0 {
        Instruction::Label(_) | Instruction::Define(_) | Instruction::Code(Code::Primitive(_)) => {
            Some(())
        }
        Instruction::Code(Code::Macro(call)) => {
            call.arguments
                .iter_mut()
                .map(|arg| value_resolve_just_these_defines(arg, defines, r))
                .collect::<Option<Vec<_>>>()?;
            Some(())
        }
        Instruction::Constant(arc_intern) => match defines.get(arc_intern) {
            None => Some(()),
            Some(value) => {
                let whoops = |which| {
                    r.push(
                        Report::build(ReportKind::Error, this.span().clone())
                            .with_message(format!("This constant cannot be expanded as source code because it defines {which}, not a code block"))
                            .finish(),
                    );
                    None
                };

                match &*value.value {
                    ResolvedValue::Int(_) => whoops("an integer"),
                    ResolvedValue::Ident(_) => whoops("an identifier"),
                    ResolvedValue::Register(_) => whoops("a register"),
                    ResolvedValue::Block(block) => {
                        this.0 = Instruction::Block(block.clone());
                        Some(())
                    }
                }
            }
        },
        Instruction::RhaiCall(rhai_call) => {
            rhai_call
                .args
                .iter_mut()
                .map(|arg| value_resolve_just_these_defines(arg, defines, r))
                .collect::<Option<Vec<_>>>()?;
            Some(())
        }
        Instruction::Block(block) => {
            block
                .code
                .iter_mut()
                .map(|instr| resolve_just_these_defines(instr, defines, r))
                .collect::<Option<Vec<_>>>()?;
            Some(())
        }
    }
}

#[derive(Clone, Debug)]
struct Block {
    code: Vec<WithSpan<TaggedInstruction>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RegisterModifiers {
    modulus: Option<Int<U>>,
    negative: bool,
}

impl RegisterModifiers {
    fn is_plain(self) -> bool {
        self.modulus.is_none() && !self.negative
    }

    fn found(self) -> &'static str {
        match (self.modulus, self.negative) {
            (None, true) => "negative alias",
            (None, false) => unreachable!(),
            (Some(_), true) => "negative alias and modulus value",
            (Some(_), false) => "modulus value",
        }
    }

    fn compose(
        self,
        new: RegisterModifiers,
        old_span: &Span,
        new_span: &Span,
        r: &Reporter,
    ) -> Option<RegisterModifiers> {
        Some(RegisterModifiers {
            modulus: match (self.modulus, new.modulus) {
                (None, None) => None,
                (None, Some(v)) | (Some(v), None) => Some(v),
                (Some(a), Some(b)) => {
                    if !(a % b).is_zero() {
                        r.push(
                            Report::build(ReportKind::Error, new_span.clone())
                                .with_label(
                                    ariadne::Label::new(new_span.clone())
                                        .with_message(format!("{b} does not divide {a} so the modulus can't be refined to {b}")),
                                )
                                .with_label(
                                    ariadne::Label::new(old_span.clone())
                                        .with_message("Constant was defined here"),
                                )
                                .finish(),
                        );
                        return None;
                    }

                    Some(b)
                }
            },
            negative: self.negative ^ new.negative,
        })
    }

    fn format_with(self, other: &str) -> String {
        format!(
            "{}{other}{}",
            if self.negative { "-" } else { "" },
            match self.modulus {
                Some(m) => format!("%{m}"),
                None => String::new(),
            }
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RegisterReference {
    reg_name: WithSpan<ArcIntern<str>>,
    mods: RegisterModifiers,
}

impl RegisterReference {
    fn is_plain(&self) -> bool {
        self.mods.is_plain()
    }
}

impl Display for RegisterReference {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.mods.format_with(&self.reg_name))?;

        Ok(())
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
    Constant {
        name: ArcIntern<str>,
        mods: RegisterModifiers,
    },
}

fn value_resolve_just_these_defines(
    value: &mut WithSpan<Value>,
    defines: &HashMap<ArcIntern<str>, Define<WithSpan<ResolvedValue>>>,
    r: &Reporter,
) -> Option<()> {
    let span = value.span().clone();
    match &mut **value {
        Value::Resolved(value) => match value {
            ResolvedValue::Int(_) | ResolvedValue::Register(_) | ResolvedValue::Ident(_) => {
                Some(())
            }
            ResolvedValue::Block(block) => {
                block
                    .code
                    .iter_mut()
                    .map(|instr| resolve_just_these_defines(instr, defines, r))
                    .collect::<Option<Vec<_>>>()?;
                Some(())
            }
        },
        Value::Constant {
            name: arc_intern,
            mods,
        } => match defines.get(arc_intern) {
            None => Some(()),
            Some(define) => {
                value.value =
                    Value::Resolved(compose_mod(define.value.clone(), *mods, &span, r)?.value);
                Some(())
            }
        },
    }
}

fn compose_mod(
    value: WithSpan<ResolvedValue>,
    mods: RegisterModifiers,
    ref_span: &Span,
    r: &Reporter,
) -> Option<WithSpan<ResolvedValue>> {
    if mods.is_plain() {
        return Some(value);
    }

    let def_span = value.span().clone();

    let ResolvedValue::Register(mut reg) = value.value else {
        r.push(
            Report::build(ReportKind::Error, ref_span.clone())
                .with_label(ariadne::Label::new(ref_span.clone()).with_message(format!(
                    "Register modifiers are not meaningful for non-register constants. Found a {}",
                    mods.found()
                )))
                .with_label(ariadne::Label::new(def_span).with_message("Constant was defined here"))
                .finish(),
        );
        return None;
    };

    reg.mods = reg.mods.compose(mods, &def_span, ref_span, r)?;

    Some(def_span.with(ResolvedValue::Register(reg)))
}

type RegisterInfo = (RegisterReference, RhaiRegInfo);

#[derive(Clone, Debug)]
enum ResolvedValue {
    Int(Int<U>),
    Ident(WithSpan<ArcIntern<str>>),
    Register(RegisterReference),
    Block(Block),
}

enum MaybeReg<'a> {
    Reg(&'a RegisterReference, RhaiRegInfo),
    NotAReg(&'static str),
    RegNotFound(&'a Span),
}

impl ResolvedValue {
    /// If this value is not an identifier, returns `None`. If this value is an identifier that does not refer to a register, returns `Some(Err(_))`. Otherwise returns `Some(Ok(_))`
    pub fn as_reg(&self, info: &ExpansionInfo) -> MaybeReg<'_> {
        match self {
            ResolvedValue::Register(reg) => {
                match info.register_exists(&reg).map(|reg_info| (reg, reg_info)) {
                    Some((reg, reg_info)) => MaybeReg::Reg(reg, reg_info),
                    None => MaybeReg::RegNotFound(reg.reg_name.span()),
                }
            }
            ResolvedValue::Int(_) => MaybeReg::NotAReg("an integer"),
            ResolvedValue::Ident(_) => MaybeReg::NotAReg("an identifier"),
            ResolvedValue::Block(_) => MaybeReg::NotAReg("a code block"),
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
    args: Box<[WithSpan<Value>]>,
}

impl RhaiCall {
    fn perform(
        self,
        span: Span,
        info: &ExpansionInfo,
        block_id: BlockID,
        r: &Reporter,
    ) -> Option<WithSpan<ResolvedValue>> {
        let args = self
            .args
            .into_iter()
            .map(|value| info.resolve(DefineValue::Value(value), block_id, r))
            .collect::<Option<Vec<_>>>()?;

        let rhai = info.rhai_macros.get(&span.source()).unwrap();

        let result = rhai.do_rhai_call(&self.function_name, args, span.clone(), info, r)?;

        Some(span.with(result))
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
                | (MacroArgTy::Ident, ResolvedValue::Ident(_))
                | (MacroArgTy::Block, ResolvedValue::Block(_)) => true,
                (MacroArgTy::Reg, ResolvedValue::Register(_)) => {
                    matches!(value.as_reg(info), MaybeReg::Reg(_, _))
                }
                _ => false,
            },
            (MacroPatternComponent::Word(word), ResolvedValue::Ident(ident)) => word == &**ident,
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
            (MacroPatternComponent::Word(word), ResolvedValue::Ident(ident)) => {
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
    code: WithSpan<TaggedInstruction>,
}

#[derive(Clone, Debug)]
enum Macro {
    UserDefined {
        branches: Vec<WithSpan<MacroBranch>>,
    },
    Builtin(
        fn(&ExpansionInfo, WithSpan<Vec<WithSpan<Value>>>, BlockID, &Reporter) -> Option<Primitive>,
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
    puzzles: Box<[Puzzle]>,
}

impl RegistersDecl {
    fn register_exists(&self, reference: &RegisterReference) -> Option<RhaiRegInfo> {
        let reg_name = reference.reg_name.clone();

        let modulus = reference.mods.modulus;

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
    reporter: Reporter,
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

    fn resolve(
        &self,
        block_id: BlockID,
        value: WithSpan<Value>,
    ) -> Option<WithSpan<ResolvedValue>> {
        let span = value.span().clone();
        Some(span.clone().with(match value.into_inner() {
            Value::Resolved(resolved) => resolved,
            Value::Constant {
                name: arc_intern,
                mods,
            } => {
                let Some(v) = self.get_define(block_id, &arc_intern) else {
                    self.reporter.push(
                        Report::build(ReportKind::Error, span.clone())
                            .with_message("Constant not found in this scope")
                            .finish(),
                    );
                    return None;
                };

                compose_mod(v.value.clone(), mods, &span, &self.reporter)?.into_inner()
            }
        }))
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
        r: &Reporter,
    ) -> Option<WithSpan<ResolvedValue>> {
        match value {
            DefineValue::Value(v) => self.block_info.resolve(block_id, v),
            DefineValue::RhaiCall(call) => {
                let span = call.span().clone();
                call.into_inner().perform(span, self, block_id, r)
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
    use qter_core::span::File;

    use crate::{Reporter, compile, q_emitter::emit_q};

    #[test]
    fn test_define() {
        let code = "
            .registers {
                A <- 3x3 (U)
            }

            .define $one 1
            .define $var A

            .define $X {
                add $var $one
            }
            .define $Y $X
            .define $Z $Y

            $X
            $Y
            $Z
        ";

        let reporter = Reporter::default();

        let Some((program, _)) = compile(
            &File::new(ArcIntern::from("code.qat"), ArcIntern::from(code)),
            |_| unreachable!(),
            &reporter,
        ) else {
            for report in reporter.iter() {
                report
                    .1
                    .print(ariadne::sources(
                        [("code.qat", code)]
                            .into_iter()
                            .map(|(a, b)| (ArcIntern::from(a), ArcIntern::from(b))),
                    ))
                    .unwrap();
            }
            panic!();
        };

        let q_code = emit_q(&program, "code.q".into(), &reporter).unwrap().0;

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

            .define $X {
                $X
            }

            $X
        ";

        let reporter = Reporter::default();

        match compile(
            &File::new(ArcIntern::from("code.qat"), ArcIntern::from(code)),
            |_| unreachable!(),
            &reporter,
        ) {
            Some(v) => panic!("{v:?}"),
            None => {
                assert_eq!(reporter.iter().count(), 1);
            }
        }
    }

    #[test]
    fn test_rhai_error_reporting() {
        let code = "
            .registers {
                A <- 3x3 (U)
            }

            .start-rhai
            fn SKIBIDI
            .end-rhai
        ";

        let reporter = Reporter::default();

        match compile(
            &File::new(ArcIntern::from("code.qat"), ArcIntern::from(code)),
            |_| unreachable!(),
            &reporter,
        ) {
            Some(v) => panic!("{v:?}"),
            None => {
                assert_eq!(reporter.iter().count(), 1);
            }
        }
    }
}
