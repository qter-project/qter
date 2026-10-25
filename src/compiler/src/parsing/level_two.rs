use std::{cell::Cell, mem, sync::Arc};

use ariadne::{Label, Report, ReportKind, Span as _};
use internment::ArcIntern;
use itertools::{Itertools, MultiPeek};
use puzzle_theory::{
    numbers::{Int, U},
};

use qter_core::span::{File, Span, WithSpan};
use crate::{
    RegisterModifiers, RegisterReference, Reporter,
    parsing::tokenizer::{Encloser, Symbol, TokenFlat, TokenNLFlat, Tokenizer},
    rhai::RhaiCode,
};

pub fn tokenize(qat: &File, reporter: Reporter) -> Option<TokenEnclosure> {
    let mut t = Tokenizer::new(qat.to_owned(), reporter.clone());

    t.skip_shebang();

    let mk_span = |start, end| Span::new(qat.clone(), start, end);

    let last = Cell::new(t.spot);
    let mut t = t
        .inspect(|v| {
            assert_eq!(v.span().start(), last.get());
            last.set(v.span().end());
        })
        .multipeek();

    let mut stack = Vec::<(usize, usize, Encloser, Vec<WithSpan<TokenNL>>)>::new();
    let mut out = Vec::new();

    let num_chars = qat.inner().chars().count();

    while last.get() != num_chars {
        let token = t.next()?;
        let span = token.span().clone();
        match token.into_inner() {
            TokenNLFlat::NewLine => out.push(span.with(TokenNL::NewLine)),
            TokenNLFlat::Whitespace => out.push(span.with(TokenNL::Whitespace)),
            TokenNLFlat::Token(token) => match token {
                TokenFlat::Ident(v) => out.push(span.with(TokenNL::Token(Token::Ident(v)))),
                TokenFlat::Name(name) => {
                    let mut reg = RegisterReference {
                        reg_name: span.clone().with(name),
                        mods: RegisterModifiers {
                            modulus: None,
                            negative: false,
                        },
                    };

                    let mut end = span.end();

                    take_modifier_postfix(&mut reg.mods, &mut t, &mut end);

                    out.push(mk_span(span.start(), end).with(TokenNL::Token(Token::Register(reg))));
                }
                TokenFlat::Constant(v) => {
                    let mut mods = RegisterModifiers {
                        modulus: None,
                        negative: false,
                    };

                    let mut end = span.end();

                    take_modifier_postfix(&mut mods, &mut t, &mut end);

                    out.push(
                        mk_span(span.start(), end).with(TokenNL::Token(Token::Constant(v, mods))),
                    )
                }
                TokenFlat::Number(v) => out.push(span.with(TokenNL::Token(Token::Number(v)))),
                TokenFlat::Symbol(v) => {
                    if v == Symbol::Dot
                        && let Some(v) = t.peek()
                        && let TokenNLFlat::Token(TokenFlat::Ident(v)) = &**v
                    {
                        out.push(span.with(TokenNL::Token(Token::Directive(v.clone()))));
                        t.next()?;
                    } else if v == Symbol::Negative
                        && let Some(v) = t.peek()
                        && matches!(
                            &**v,
                            TokenNLFlat::Token(TokenFlat::Name(_) | TokenFlat::Constant(_))
                        )
                    {
                        let mut mods = RegisterModifiers {
                            modulus: None,
                            negative: true,
                        };

                        let token = t.next().unwrap();
                        let name_span = token.span().clone();

                        let mut end = token.span().end();

                        take_modifier_postfix(&mut mods, &mut t, &mut end);

                        out.push(mk_span(span.start(), end).with(match token.into_inner() {
                            TokenNLFlat::Token(TokenFlat::Name(name)) => {
                                TokenNL::Token(Token::Register(RegisterReference {
                                    reg_name: name_span.with(name),
                                    mods,
                                }))
                            }
                            TokenNLFlat::Token(TokenFlat::Constant(const_name)) => {
                                TokenNL::Token(Token::Constant(const_name, mods))
                            }
                            _ => unreachable!(),
                        }));
                    } else {
                        out.push(span.with(TokenNL::Token(Token::Symbol(v))));
                    }
                }
                TokenFlat::RhaiCode(v) => out.push(span.with(TokenNL::Token(Token::RhaiCode(v)))),
                TokenFlat::OpenEncloser(e) => {
                    stack.push((span.start(), span.end(), e, mem::take(&mut out)));
                }
                TokenFlat::CloseEncloser(e) => {
                    let Some((before_t, after_t, encloser, prev_out)) = stack.pop() else {
                        reporter.push(
                            Report::build(ReportKind::Error, span.clone())
                                .with_message("Unmatched closing delimiter")
                                .finish(),
                        );
                        return None;
                    };

                    if e != encloser {
                        reporter.push(
                            Report::build(ReportKind::Error, span.clone())
                                .with_label(
                                    ariadne::Label::new(span.clone())
                                        .with_message("Mismatched delimiter"),
                                )
                                .with_label(
                                    ariadne::Label::new(mk_span(before_t, after_t))
                                        .with_message("Opening delimiter found here"),
                                )
                                .finish(),
                        );
                        return None;
                    }

                    out.push(
                        span.clone()
                            .with(TokenNL::Token(Token::EndOfEnclosure(Some(e)))),
                    );

                    let contents = mem::replace(&mut out, prev_out);

                    out.push(
                        mk_span(before_t, span.end()).with(TokenNL::Token(Token::Enclosure(
                            e,
                            mk_span(after_t, span.start()).with(TokenEnclosure {
                                iter: TokenIter {
                                    tokens: contents.into(),
                                    reporter: reporter.clone(),
                                    spot: 0,
                                    file: qat.clone(),
                                },
                            }),
                        ))),
                    );
                }
            },
        }
    }

    if !stack.is_empty() {
        for (before, after, _, _) in stack {
            reporter.push(
                Report::build(ReportKind::Error, mk_span(before, after))
                    .with_message("Unmatched opening delimiter")
                    .finish(),
            );
        }

        return None;
    }

    out.push(mk_span(num_chars, num_chars).with(TokenNL::Token(Token::EndOfEnclosure(None))));

    Some(TokenEnclosure {
        iter: TokenIter {
            tokens: out.into(),
            reporter,
            spot: 0,
            file: qat.clone(),
        },
    })
}

fn take_modifier_postfix<I: Iterator<Item = WithSpan<TokenNLFlat>>>(
    m: &mut RegisterModifiers,
    t: &mut MultiPeek<I>,
    span_end: &mut usize,
) {
    if let Some(peek) = t.peek()
        && matches!(
            &**peek,
            TokenNLFlat::Token(TokenFlat::Symbol(Symbol::Modulus))
        )
        && let Some(maybe_num) = t.peek()
        && let TokenNLFlat::Token(TokenFlat::Number(num)) = &**maybe_num
    {
        m.modulus = Some(*num);
        *span_end = maybe_num.span().end();
        t.next().unwrap();
        t.next().unwrap();
    } else {
        t.reset_peek();
    }

    // TODO: Subscripting
}

#[derive(Clone, Debug)]
pub struct TokenEnclosure {
    iter: TokenIter,
}

impl TokenEnclosure {
    pub fn parse<T>(mut self, f: impl FnOnce(&mut TokenIter) -> Option<T>) -> Option<WithSpan<T>> {
        assert!(matches!(
            &**self.iter.tokens.last().unwrap(),
            TokenNL::Token(Token::EndOfEnclosure(_))
        ));

        let marker = self.iter.marker();

        let res = f(&mut self.iter);

        if res.is_some() && self.iter.spot < self.iter.tokens.len() - 1 {
            let token = self.iter.next().unwrap();
            println!("{token:?}");
            let span = token.span().clone();
            self.iter.report(
                Report::build(ReportKind::Error, span.clone())
                    .with_label(Label::new(span).with_message("Unexpected token"))
                    .finish(),
            );
        }

        let span = self.iter.cash_in(marker);

        res.map(|v| span.with(v))
    }
}

#[derive(Clone, Debug)]
pub struct TokenIter {
    tokens: Arc<[WithSpan<TokenNL>]>,
    reporter: Reporter,
    spot: usize,
    file: File,
}

impl TokenIter {
    fn get(&self, idx: usize) -> Option<&WithSpan<TokenNL>> {
        match self.tokens.get(idx) {
            Some(v) => Some(v),
            None => {
                let span = self.tokens.last().unwrap().span();
                self.report(
                    Report::build(ReportKind::Error, span.clone())
                        .with_label(Label::new(span.clone()).with_message("Here"))
                        .with_message("Expected token, found end of enclosure")
                        .finish(),
                );
                None
            }
        }
    }

    pub fn next(&mut self) -> Option<WithSpan<Token>> {
        loop {
            let token = self.get(self.spot)?.clone();
            self.spot += 1;

            let span = token.span().clone();

            match token.into_inner() {
                TokenNL::NewLine | TokenNL::Whitespace => {}
                TokenNL::Token(token) => return Some(span.with(token)),
            }
        }
    }

    pub fn attempt<T>(&mut self, step: impl FnOnce(&mut Self, &mut bool) -> T) -> Attempt<T> {
        let mut new_self = self.clone();
        new_self.reporter = Reporter::default();

        let marker = new_self.marker();

        let mut commit = false;

        let outcome = step(&mut new_self, &mut commit);

        if commit {
            for report in new_self.reporter.drain() {
                self.reporter.push(report);
            }

            self.spot = new_self.spot;

            Attempt::Taken(outcome)
        } else {
            let span = self.cash_in(marker);
            Attempt::NotTaken(span)
        }
    }

    pub fn report(&self, report: Report<'static, Span>) {
        self.reporter.push(report);
    }

    pub fn r(&self) -> Reporter {
        self.reporter.clone()
    }

    pub fn marker(&self) -> SpanMarker {
        SpanMarker(match self.tokens.get(self.spot) {
            Some(v) => v.span().start(),
            None => self.tokens.last().unwrap().span().end(),
        })
    }

    pub fn cash_in(&self, marker: SpanMarker) -> Span {
        let end = match self.tokens.get(self.spot) {
            Some(v) => v.span().start(),
            None => self.tokens.last().unwrap().span().end(),
        };
        Span::new(self.file.clone(), marker.0, end)
    }

    pub fn file(&self) -> &File {
        &self.file
    }

    pub fn on_whitespace(&self) -> Option<Span> {
        let token = self.get(self.spot)?;

        if let TokenNL::Whitespace | TokenNL::NewLine = &**token {
            return Some(token.span().clone());
        }

        None
    }

    pub fn nl(&mut self) -> Option<Span> {
        loop {
            let token = self.get(self.spot)?;
            let span = token.span().clone();
            match &**token {
                TokenNL::NewLine => {
                    self.spot += 1;
                    return Some(span);
                }
                TokenNL::Token(Token::EndOfEnclosure(_)) => {
                    return Some(span);
                }
                TokenNL::Whitespace => {
                    self.spot += 1;
                }
                TokenNL::Token(t) => return self.unexpected(span.with(t.clone()), "newline"),
            }
        }
    }

    fn filter<T>(
        &mut self,
        expected: &str,
        picker: impl FnOnce(&Token) -> Option<T>,
    ) -> Option<WithSpan<T>> {
        let t = self.next()?;
        let span = t.span().clone();

        match picker(&t) {
            Some(v) => Some(span.with(v)),
            None => self.unexpected(t, expected),
        }
    }

    pub fn ident(&mut self) -> Option<WithSpan<ArcIntern<str>>> {
        self.filter("an identifier", |t| match t {
            Token::Ident(ident) => Some(ident.clone()),
            _ => None,
        })
    }

    pub fn reg(&mut self) -> Option<WithSpan<RegisterReference>> {
        self.filter("a register", |t| match t {
            Token::Register(reg) => Some(reg.clone()),
            _ => None,
        })
    }

    pub fn number(&mut self) -> Option<WithSpan<Int<U>>> {
        self.filter("a non-negative integer", |t| match t {
            Token::Number(num) => Some(*num),
            _ => None,
        })
    }

    pub fn constant(&mut self) -> Option<WithSpan<(ArcIntern<str>, RegisterModifiers)>> {
        self.filter("a constant", |t| match t {
            Token::Constant(c, mods) => Some((c.clone(), *mods)),
            _ => None,
        })
    }

    pub fn word(&mut self, word: &str) -> Option<WithSpan<()>> {
        self.filter(&format!("`{word}`"), |t| {
            if let Some(word) = word.strip_prefix(".") {
                if matches!(t, Token::Directive(directive) if directive == word) {
                    return Some(());
                }
            } else if matches!(t, Token::Ident(ident) if ident == word) {
                return Some(());
            }

            None
        })
    }

    pub fn symbol(&mut self, sym: Symbol) -> Option<WithSpan<()>> {
        self.filter(&format!("`{}`", sym.as_str()), |t| match t {
            Token::Symbol(symbol) if *symbol == sym => Some(()),
            _ => None,
        })
    }

    pub fn enclosure(&mut self, target_enloser: Encloser) -> Option<WithSpan<TokenEnclosure>> {
        self.filter(
            match target_enloser {
                Encloser::Paren => "a parenthesized expression",
                Encloser::Brace => "a block with curly braces",
            },
            |t| match t {
                Token::Enclosure(encloser, enclosed) if *encloser == target_enloser => {
                    Some(enclosed.clone().into_inner())
                }
                _ => None,
            },
        )
    }

    pub fn end_of_enclosure(&mut self) -> Option<WithSpan<()>> {
        self.filter("end of enclosure", |t| match t {
            Token::EndOfEnclosure(_) => Some(()),
            _ => None,
        })
    }

    pub fn one_of<T, const N: usize>(
        &mut self,
        choices: [(&'static str, T); N],
    ) -> Option<WithSpan<T>> {
        let names = choices.each_ref().map(|v| v.0);

        self.filter(
            &format!(
                "one of {}",
                names.into_iter().map(|v| format!("`{v}`")).format(", ")
            ),
            |t| match t {
                Token::Ident(ident) => {
                    for choice in choices {
                        if ident == choice.0 {
                            return Some(choice.1);
                        }
                    }

                    None
                }
                _ => None,
            },
        )
    }

    pub fn unexpected<T>(&self, t: WithSpan<Token>, expected: &str) -> Option<T> {
        let span = t.span().clone();

        let (found, span) = match t.into_inner() {
            Token::Ident(ident) => (format!("`{ident}`"), span),
            Token::Register(reg) => (format!("a register `{reg}`"), span),
            Token::Directive(dir) => (format!("directive `.{dir}`"), span),
            Token::Constant(ident, mods) => (
                format!("`{}`", mods.format_with(&format!("${ident}"))),
                span,
            ),
            Token::Number(_) => ("a number".to_owned(), span),
            Token::Symbol(_) => ("a symbol".to_owned(), span),
            Token::Enclosure(encloser, enclosed) => (
                match encloser {
                    Encloser::Paren => "a parenthesized expression",
                    Encloser::Brace => "a block",
                }
                .to_owned(),
                enclosed.span().clone(),
            ),
            Token::RhaiCode(_) => ("rhai code".to_owned(), span),
            Token::EndOfEnclosure(parent) => (
                match parent {
                    Some(Encloser::Paren) => "the end of the parenthesized expression",
                    Some(Encloser::Brace) => "the end of the block",
                    None => "the end of the file",
                }
                .to_owned(),
                span,
            ),
        };

        self.r().push(
            Report::build(ReportKind::Error, span.clone())
                .with_message(format!("Expected {expected} but found {found}."))
                .with_label(Label::new(span).with_message("here"))
                .finish(),
        );

        None
    }

    pub fn parse_list<T>(
        &mut self,
        mut item: impl FnMut(&mut TokenIter, &mut bool) -> Option<T>,
        mut delim: impl FnMut(&mut TokenIter, &mut bool) -> Option<()>,
    ) -> Option<Box<[T]>> {
        let mut maybe_out = Some(Vec::new());

        loop {
            let spot = self.spot;
            match self.attempt(&mut item) {
                Attempt::NotTaken(_) => break maybe_out.map(Into::into),
                Attempt::Taken(Some(v)) => {
                    if let Some(out) = &mut maybe_out {
                        out.push(v);
                    }
                }
                Attempt::Taken(None) => maybe_out = None,
            }

            match self.attempt(&mut delim) {
                Attempt::NotTaken(_) => break maybe_out.map(Into::into),
                Attempt::Taken(Some(())) => {}
                Attempt::Taken(None) => maybe_out = None,
            }

            // No progress made across either thingy
            if spot == self.spot {
                if maybe_out.is_none() {
                    return None;
                }

                panic!(
                    "Should not go through the loop without making progress; {spot} - {:?}",
                    maybe_out.as_ref().map(Vec::len)
                );
            }
        }
    }
}

pub enum Attempt<T> {
    NotTaken(Span),
    Taken(T),
}

impl<T> Attempt<Option<T>> {
    /// Nested `attempt` call combinator
    pub fn c(self, commit: &mut bool) -> Option<T> {
        match self {
            Attempt::NotTaken(_) => {
                *commit = false;
                None
            }
            Attempt::Taken(v) => {
                *commit = true;
                v
            }
        }
    }
}

#[derive(Clone, Copy)]
pub struct SpanMarker(usize);

#[derive(Clone, Debug)]
pub enum Token {
    Ident(ArcIntern<str>),
    Register(RegisterReference),
    Directive(ArcIntern<str>),
    Constant(ArcIntern<str>, RegisterModifiers),
    Number(Int<U>),
    Symbol(Symbol),
    Enclosure(Encloser, WithSpan<TokenEnclosure>),
    RhaiCode(RhaiCode),
    EndOfEnclosure(Option<Encloser>),
}

#[derive(Clone, Debug)]
enum TokenNL {
    NewLine,
    Whitespace,
    Token(Token),
}
