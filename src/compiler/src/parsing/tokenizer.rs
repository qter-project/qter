use std::{mem, sync::Arc};

use ariadne::Span as AriadneSpan;

use ariadne::{Label, Report, ReportKind};
use internment::ArcIntern;
use itertools::Itertools;
use puzzle_theory::{
    numbers::{Int, U},
    span::{File, Span, WithSpan},
};
use rhai::Position;

use crate::Reporter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encloser {
    /// `( ... )`
    Paren,
    /// `{ ... }`
    Brace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    /// `,`
    Comma,
    /// `<-` or `←`
    AssignArrow,
    /// `=>`
    DefineArrow,
    /// `:`
    Colon,
}

impl Symbol {
    fn as_str(self) -> &'static str {
        match self {
            Symbol::Comma => ",",
            Symbol::AssignArrow => "<-",
            Symbol::DefineArrow => "=>",
            Symbol::Colon => ":",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum SpecialSym {
    Symbol(Symbol),
    Open(Encloser),
    Close(Encloser),
    /// `//`
    LineCommentStart,
    /// `/*`
    BlockCommentStart,
    Quote,
    NewLine,
}

#[derive(Clone)]
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
            let span = self.iter.next().unwrap().span().clone();
            self.iter.report(
                Report::build(ReportKind::Error, span)
                    .with_message("Unexpected token")
                    .finish(),
            );
        }

        let span = self.iter.cash_in(marker);

        res.map(|v| span.with(v))
    }
}

#[derive(Clone)]
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
                self.report(
                    Report::build(ReportKind::Error, self.cash_in(self.marker()))
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
            let v = Arc::try_unwrap(new_self.reporter).expect("the reporter not to be kept");

            for report in v {
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

    pub fn whitespace(&mut self) -> Option<Span> {
        let token = self.get(self.spot)?;
        let span = token.span().clone();
        match &**token {
            TokenNL::Whitespace => {
                self.spot += 1;
                Some(span)
            }
            _ => None,
        }
    }

    pub fn nl(&mut self) -> Option<Span> {
        let token = self.get(self.spot)?;
        let span = token.span().clone();
        match &**token {
            TokenNL::NewLine => {
                self.spot += 1;
                Some(span)
            }
            _ => None,
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

    pub fn number(&mut self) -> Option<WithSpan<Int<U>>> {
        self.filter("a non-negative integer", |t| match t {
            Token::Number(num) => Some(*num),
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
            Token::Directive(ident) => (format!("`.{ident}`"), span),
            Token::Constant(ident) => (format!("`${ident}`"), span),
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

#[derive(Clone)]
pub enum Token {
    Ident(ArcIntern<str>),
    Directive(ArcIntern<str>),
    Constant(ArcIntern<str>),
    Number(Int<U>),
    Symbol(Symbol),
    Enclosure(Encloser, WithSpan<TokenEnclosure>),
    RhaiCode(RhaiCode),
    EndOfEnclosure(Option<Encloser>),
}

pub enum TokenFlat {
    Ident(ArcIntern<str>),
    Directive(ArcIntern<str>),
    Constant(ArcIntern<str>),
    Number(Int<U>),
    Symbol(Symbol),
    OpenEncloser(Encloser),
    CloseEncloser(Encloser),
}

#[derive(Clone)]
enum TokenNL {
    NewLine,
    Whitespace,
    Token(Token),
}

pub enum TokenNLFlat {
    NewLine,
    Whitespace,
    Token(TokenFlat),
}

pub fn tokenize(qat: &File, reporter: Reporter) -> Option<TokenEnclosure> {
    let mut t = Tokenizer::new(qat.to_owned(), Arc::clone(&reporter));

    t.skip_shebang();

    let mut stack = Vec::<(usize, usize, Encloser, Vec<WithSpan<TokenNL>>)>::new();
    let mut out = Vec::new();

    let mut before = t.spot;

    while !t.done() {
        let token = t.next()?;
        let span = token.span().clone();
        match token.into_inner() {
            TokenNLFlat::NewLine => out.push(span.with(TokenNL::NewLine)),
            TokenNLFlat::Whitespace => out.push(span.with(TokenNL::Whitespace)),
            TokenNLFlat::Token(token) => match token {
                TokenFlat::Ident(v) => out.push(span.with(TokenNL::Token(Token::Ident(v)))),
                TokenFlat::Directive(v) => {
                    if v == "start-rhai" {
                        let rhai_code = t.take_rhai()?;
                        out.push(
                            Span::new(qat.clone(), span.start(), t.spot)
                                .with(TokenNL::Token(Token::RhaiCode(RhaiCode(rhai_code)))),
                        );
                    } else {
                        out.push(span.with(TokenNL::Token(Token::Directive(v))));
                    }
                }
                TokenFlat::Constant(v) => out.push(span.with(TokenNL::Token(Token::Constant(v)))),
                TokenFlat::Number(v) => out.push(span.with(TokenNL::Token(Token::Number(v)))),
                TokenFlat::Symbol(v) => out.push(span.with(TokenNL::Token(Token::Symbol(v)))),
                TokenFlat::OpenEncloser(e) => stack.push((before, t.spot, e, mem::take(&mut out))),
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
                                    ariadne::Label::new(t.mk_span(before_t, after_t))
                                        .with_message("Opening delimiter found here"),
                                )
                                .finish(),
                        );
                        return None;
                    }

                    out.push(span.with(TokenNL::Token(Token::EndOfEnclosure(Some(e)))));

                    let contents = mem::replace(&mut out, prev_out);

                    out.push(
                        t.mk_span(before_t, t.spot)
                            .with(TokenNL::Token(Token::Enclosure(
                                e,
                                t.mk_span(after_t, before).with(TokenEnclosure {
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

        before = t.spot;
    }

    if !stack.is_empty() {
        for (before, after, _, _) in stack {
            reporter.push(
                Report::build(ReportKind::Error, t.mk_span(before, after))
                    .with_message("Unmatched opening delimiter")
                    .finish(),
            );
        }

        return None;
    }

    assert_eq!(t.spot, qat.inner().len());

    out.push(
        t.mk_span(qat.inner().len(), qat.inner().len())
            .with(TokenNL::Token(Token::EndOfEnclosure(None))),
    );

    Some(TokenEnclosure {
        iter: TokenIter {
            tokens: out.into(),
            reporter,
            spot: 0,
            file: qat.clone(),
        },
    })
}

#[derive(Clone)]
struct Tokenizer {
    file: File,
    qat: ArcIntern<str>,
    spot: usize,
    reporter: Reporter,
}

impl Tokenizer {
    fn new(file: File, reporter: Reporter) -> Tokenizer {
        Tokenizer {
            qat: file.inner(),
            file,
            spot: 0,
            reporter,
        }
    }

    fn qat(&self) -> &str {
        &self.qat[self.spot..]
    }

    fn skip_shebang(&mut self) {
        if self.qat().starts_with("#!") {
            while self.peek(0) != Some('\n') {
                self.advance(1);
            }

            if self.peek(0).is_some() {
                self.advance(1);
            }
        }
    }

    fn peek(&self, n: usize) -> Option<char> {
        self.qat().chars().nth(n)
    }

    fn advance(&mut self, n: usize) {
        self.spot += self
            .qat()
            .char_indices()
            .nth(n)
            .map_or(self.qat().len(), |v| v.0);
    }

    fn whitespace_amt(&self) -> usize {
        self.qat()
            .find(|c| c != ' ' && c != '\t' && c != '\r')
            .unwrap_or(self.qat().len())
    }

    fn skip_whitespace(&mut self) -> bool {
        let amt = self.whitespace_amt();
        self.spot += amt;
        amt != 0
    }

    fn mk_span(&self, start: usize, end: usize) -> Span {
        Span::new(self.file.clone(), start, end)
    }

    /// If the next input contains a special character or symbol, return the character and how many actual characters should be advanced to skip over it.
    fn special_sym(&self) -> Option<(SpecialSym, usize)> {
        use Encloser::*;
        use SpecialSym as S;
        use Symbol::*;

        Some(match (self.peek(0)?, self.peek(1)) {
            (',', _) => (S::Symbol(Comma), 1),
            (':', _) => (S::Symbol(Colon), 1),
            ('←', _) => (S::Symbol(AssignArrow), 1),
            ('<', Some('-')) => (S::Symbol(AssignArrow), 2),
            ('⇒', _) => (S::Symbol(DefineArrow), 1),
            ('=', Some('>')) => (S::Symbol(DefineArrow), 2),
            ('{', _) => (S::Open(Brace), 1),
            ('}', _) => (S::Close(Brace), 1),
            ('(', _) => (S::Open(Paren), 1),
            (')', _) => (S::Close(Paren), 1),
            ('/', Some('/')) => (S::LineCommentStart, 2),
            ('/', Some('*')) => (S::BlockCommentStart, 2),
            ('"', _) => (S::Quote, 1),
            ('\n', _) => (S::NewLine, 1),
            _ => return None,
        })
    }

    fn take_rhai(&mut self) -> Option<Span> {
        let spot = self.spot;
        let Some(end) = self.qat().find(".end-rhai") else {
            let span = self.mk_span(spot - 11, spot);
            self.reporter.push(
                Report::build(ReportKind::Error, span.clone())
                    .with_message("Unterminated Rhai block")
                    .with_label(Label::new(span))
                    .finish(),
            );
            return None;
        };

        self.spot += end + 9;

        Some(self.mk_span(spot, spot + end))
    }

    fn done(&self) -> bool {
        self.qat().is_empty()
    }

    fn next(&mut self) -> Option<WithSpan<TokenNLFlat>> {
        let before = self.spot;
        if self.skip_whitespace() {
            return Some(
                self.mk_span(before, self.spot)
                    .with(TokenNLFlat::Whitespace),
            );
        }

        assert!(!self.done());

        if let Some((sym, amt)) = self.special_sym() {
            return Some(self.mk_span(before, self.spot).with(match sym {
                SpecialSym::Symbol(sym) => {
                    self.advance(amt);
                    TokenNLFlat::Token(TokenFlat::Symbol(sym))
                }
                SpecialSym::Open(encloser) => {
                    self.advance(amt);
                    TokenNLFlat::Token(TokenFlat::OpenEncloser(encloser))
                }
                SpecialSym::Close(encloser) => {
                    self.advance(amt);
                    TokenNLFlat::Token(TokenFlat::CloseEncloser(encloser))
                }
                SpecialSym::Quote => {
                    self.advance(amt);

                    let mut text = String::new();

                    loop {
                        let eof = || {
                            self.reporter.push(
                                Report::build(ReportKind::Error, self.mk_span(before, self.spot))
                                    .with_message("Unclosed quotation")
                                    .finish(),
                            );
                        };

                        match self.peek(0) {
                            None => {
                                eof();
                                return None;
                            }
                            Some('"') => {
                                self.advance(1);
                                break TokenNLFlat::Token(TokenFlat::Ident(ArcIntern::from(text)));
                            }
                            Some('\\') => {
                                match self.peek(1) {
                                    Some(c) => text.push(c),
                                    None => {
                                        eof();
                                        return None;
                                    }
                                }

                                self.advance(2);
                            }
                            Some(c) => {
                                self.advance(1);
                                text.push(c);
                            }
                        }
                    }
                }
                SpecialSym::NewLine => {
                    // We should only give one newline even if there are a bunch of newlines in a row
                    while self.peek(0) == Some('\n') {
                        self.advance(1);
                        self.skip_whitespace();
                    }

                    TokenNLFlat::NewLine
                }
                SpecialSym::LineCommentStart => {
                    while self.peek(0).is_some_and(|v| v != '\n') {
                        self.advance(1);
                    }

                    return self.next();
                }
                SpecialSym::BlockCommentStart => {
                    let comment_start = self.spot;
                    self.advance(amt);

                    loop {
                        let (Some(c1), Some(c2)) = (self.peek(0), self.peek(1)) else {
                            self.reporter.push(
                                Report::build(
                                    ReportKind::Error,
                                    self.mk_span(comment_start, self.spot),
                                )
                                .with_message("Unclosed block comment")
                                .finish(),
                            );
                            return None;
                        };

                        if (c1, c2) == ('*', '/') {
                            self.advance(2);
                            break;
                        }

                        self.advance(1);
                    }

                    return self.next();
                }
            }));
        }

        let mut ident = String::new();

        let ident_start = self.spot;

        while let Some(c) = self.peek(0)
            && self.special_sym().is_none()
            && ![' ', '\t', '\r'].contains(&c)
        {
            ident.push(c);
            self.advance(1);
        }

        let span = self.mk_span(ident_start, self.spot);

        Some(span.with(TokenNLFlat::Token(
            if let Some(directive) = ident.strip_prefix('.') {
                TokenFlat::Directive(ArcIntern::from(directive))
            } else if let Some(constant) = ident.strip_prefix('$') {
                TokenFlat::Constant(ArcIntern::from(constant))
            } else if let Ok(num) = ident.parse::<Int<U>>() {
                TokenFlat::Number(num)
            } else {
                TokenFlat::Ident(ArcIntern::from(ident))
            },
        )))
    }
}

#[derive(Clone)]
pub struct RhaiCode(Span);

impl RhaiCode {
    pub fn span(&self) -> &Span {
        &self.0
    }

    pub fn pos_to_span(&self, pos: Position) -> Option<Span> {
        let (mut line, pos) = match (pos.line(), pos.position()) {
            (Some(line), Some(pos)) => (line - 1, pos - 1),
            (Some(line), None) => (line - 1, 0),
            _ => return None,
        };

        let source = self.span().source().inner();

        let mut start = self.span().start();

        while line != 0 {
            start += source[start..].find('\n')? + 1;
            line -= 1;
        }

        start += pos;

        Some(Span::new(self.span().source(), start, start + 1))
    }
}
