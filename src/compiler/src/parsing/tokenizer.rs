use std::{mem, sync::Arc};

use ariadne::Span as AriadneSpan;
use polonius_the_crab::{polonius, polonius_return, polonius_try};

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

pub struct TokenizerState(Tokenizer);

impl TokenizerState {
    pub fn new(file: File, reporter: Reporter) -> TokenizerState {
        TokenizerState(Tokenizer::new(file, reporter))
    }
}

pub struct TokenEnclosure<'a> {
    state: &'a mut Tokenizer,
}

impl<'a> TokenEnclosure<'a> {
    pub fn new(state: &mut TokenizerState) -> TokenEnclosure {
        TokenEnclosure {
            state: &mut state.0,
        }
    }

    fn mk_iter(self) -> TokenIter<'a> {
        TokenIter {
            start: self.state.spot,
            state: self.state,
            done: None,
        }
    }

    pub fn parse<T>(self, f: impl FnOnce(&mut TokenIter<'a>) -> Option<T>) -> Option<WithSpan<T>> {
        let mut iter = self.mk_iter();

        let res = f(&mut iter);

        // It's a bug if we parse successfully without consuming all of the input
        // assert!(res.is_none() || iter.done.is_some());

        let span = iter.run_to_end()?;

        res.map(|v| span.with(v))
    }

    pub fn discard(self) -> Option<Span> {
        self.mk_iter().run_to_end()
    }
}

pub struct TokenIter<'a> {
    state: &'a mut Tokenizer,
    start: usize,
    done: Option<Span>,
}

impl<'a> TokenIter<'a> {
    pub fn next<'b>(&'b mut self) -> Option<TokenW<'b>> {
        let mut this = self;
        loop {
            polonius!(|this| -> Option<TokenW<'polonius>> {
                let TokenNLW { token, reporter } = polonius_try!(this.next_nl());
                match token {
                    TokenNL::NewLine(_) => {}
                    TokenNL::Token(token) => polonius_return!(Some(TokenW { token, reporter })),
                }
            });
        }
    }

    pub fn next_nl<'b>(&'b mut self) -> Option<TokenNLW<'b>> {
        if self.is_empty() {
            None
        } else {
            let reporter = self.r();
            let file = self.state.file.clone();
            let mut this = self;
            polonius!(|this| -> Option<TokenNLW<'polonius>> {
                match polonius_try!(this.state.next()) {
                    TokenNL::Token(Token::EndOfEnclosure(parent, span)) => {
                        this.done = Some(Span::new(file, this.start, span.end()));
                        polonius_return!(Some(TokenNLW {
                            token: TokenNL::Token(Token::EndOfEnclosure(parent, span)),
                            reporter
                        }))
                    }
                    next => polonius_return!(Some(TokenNLW {
                        token: next,
                        reporter
                    })),
                }
            });
            unreachable!()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.done.is_some()
    }

    pub fn attempt<'b, T>(
        &'b mut self,
        step: impl FnOnce(&mut Self, &mut bool) -> T,
    ) -> Attempt<T> {
        let t2 = self.state.clone();
        let d2 = self.done.clone();

        let marker = self.marker();

        let mut commit = false;

        let reporter = Reporter::default();
        let prev = mem::replace(&mut self.state.reporter, Arc::clone(&reporter));

        let outcome = step(self, &mut commit);

        if commit {
            self.state.reporter = prev;

            let v = Arc::try_unwrap(reporter).expect("the reporter not to be kept");

            for report in v {
                self.state.reporter.push(report);
            }

            Attempt::Taken(outcome)
        } else {
            let span = self.cash_in(marker);
            *self.state = t2;
            self.done = d2;
            Attempt::NotTaken(span)
        }
    }

    pub fn report(&self, report: Report<'static, Span>) {
        self.state.reporter.push(report);
    }

    pub fn r(&self) -> Reporter {
        self.state.reporter.clone()
    }

    fn run_to_end(mut self) -> Option<Span> {
        loop {
            match self.done {
                Some(v) => break Some(v),
                None => {
                    if let TokenNL::Token(Token::Enclosure(_, enclosure)) = self.next_nl()?.token {
                        enclosure.discard();
                    }
                }
            }
        }
    }

    pub fn take_rhai(&mut self) -> Option<(Span, impl Fn(Position) -> Option<Span> + use<>)> {
        self.state.take_rhai().map(|span| {
            let span2 = span.clone();

            let f = move |pos: Position| {
                let (mut line, pos) = match (pos.line(), pos.position()) {
                    (Some(line), Some(pos)) => (line - 1, pos - 1),
                    (Some(line), None) => (line - 1, 0),
                    _ => return None,
                };

                let source = span2.source().inner();

                let mut start = span2.start();

                while line != 0 {
                    start += source[start..].find('\n')? + 1;
                    line -= 1;
                }

                start += pos;

                Some(Span::new(span2.source(), start, start + 1))
            };

            (span, f)
        })
    }

    pub fn marker(&self) -> SpanMarker {
        SpanMarker(self.state.spot)
    }

    pub fn cash_in(&self, marker: SpanMarker) -> Span {
        self.state.mk_span(marker.0, self.state.spot)
    }

    pub fn skip_shebang(&mut self) {
        self.state.skip_shebang();
    }

    pub fn file(&self) -> &File {
        &self.state.file
    }

    pub fn whitespace(&self) -> Option<Span> {
        self.state.whitespace()
    }
}

pub enum Attempt<T> {
    NotTaken(Span),
    Taken(T),
}

#[derive(Clone, Copy)]
pub struct SpanMarker(usize);

pub struct TokenW<'a> {
    pub token: Token<'a>,
    pub(super) reporter: Reporter,
}

pub enum Token<'a> {
    Ident(WithSpan<ArcIntern<str>>),
    Directive(WithSpan<ArcIntern<str>>),
    Constant(WithSpan<ArcIntern<str>>),
    Number(WithSpan<Int<U>>),
    Symbol(WithSpan<Symbol>),
    Enclosure(Encloser, TokenEnclosure<'a>),
    EndOfEnclosure(Option<Encloser>, Span),
}

impl<'a> TokenW<'a> {
    pub fn ident(self) -> Option<WithSpan<ArcIntern<str>>> {
        match self.token {
            Token::Ident(ident) => Some(ident),
            _ => self.unexpected("an identifier"),
        }
    }

    pub fn number(self) -> Option<WithSpan<Int<U>>> {
        match self.token {
            Token::Number(num) => Some(num),
            _ => self.unexpected("a non-negative integer"),
        }
    }

    pub fn word(self, word: &str) -> Option<()> {
        if let Some(word) = word.strip_prefix(".") {
            match self.token {
                Token::Directive(directive) if &**directive == word => Some(()),
                _ => self.unexpected(&format!("`{word}`")),
            }
        } else {
            match self.token {
                Token::Ident(ident) if &**ident == word => Some(()),
                _ => self.unexpected(&format!("`{word}`")),
            }
        }
    }

    pub fn symbol(self, sym: Symbol) -> Option<()> {
        match self.token {
            Token::Symbol(symbol) if *symbol == sym => Some(()),
            _ => self.unexpected(&format!("`{}`", sym.as_str())),
        }
    }

    pub fn enclosure(self, target_enloser: Encloser) -> Option<TokenEnclosure<'a>> {
        match self.token {
            Token::Enclosure(encloser, enclosed) if encloser == target_enloser => {
                return Some(enclosed);
            }
            _ => self.unexpected(match target_enloser {
                Encloser::Paren => "a parenthesized expression",
                Encloser::Brace => "a block with curly braces",
            }),
        }
    }

    pub fn one_of<T, const N: usize>(self, choices: [(&'static str, T); N]) -> Option<(T, Span)> {
        let names = choices.each_ref().map(|v| v.0);

        match &self.token {
            Token::Ident(ident) => {
                for choice in choices {
                    if &**ident == choice.0 {
                        return Some((choice.1, ident.span().clone()));
                    }
                }

                self.unexpected(&format!(
                    "one of {}",
                    names.into_iter().map(|v| format!("`{v}`")).format(", ")
                ))
            }
            _ => self.unexpected(&format!(
                "one of {}",
                names.into_iter().map(|v| format!("`{v}`")).format(", ")
            )),
        }
    }

    pub fn unexpected<T>(self, expected: &str) -> Option<T> {
        let (found, span) = match self.token {
            Token::Ident(ident) => (format!("`{}`", &**ident), ident.span().clone()),
            Token::Directive(ident) => (format!("`.{}`", &**ident), ident.span().clone()),
            Token::Constant(ident) => (format!("`${}`", &**ident), ident.span().clone()),
            Token::Number(num) => ("a number".to_owned(), num.span().clone()),
            Token::Symbol(symbol) => ("a symbol".to_owned(), symbol.span().clone()),
            Token::Enclosure(encloser, enclosed) => {
                let span = enclosed.discard()?;

                (
                    match encloser {
                        Encloser::Paren => "a parenthesized expression",
                        Encloser::Brace => "a block",
                    }
                    .to_owned(),
                    span,
                )
            }
            Token::EndOfEnclosure(parent, span) => (
                match parent {
                    Some(Encloser::Paren) => "the end of the parenthesized expression",
                    Some(Encloser::Brace) => "the end of the block",
                    None => "the end of the file",
                }
                .to_owned(),
                span,
            ),
        };

        self.reporter.push(
            Report::build(ReportKind::Error, span.clone())
                .with_message(format!("Expected {expected} but found {found}."))
                .with_label(Label::new(span).with_message("here"))
                .finish(),
        );

        None
    }
}

pub struct TokenNLW<'a> {
    pub token: TokenNL<'a>,
    pub reporter: Reporter,
}

pub enum TokenNL<'a> {
    NewLine(Span),
    Token(Token<'a>),
}

impl<'a> TokenNLW<'a> {
    pub fn nl(self) -> Option<()> {
        let reporter = self.reporter;
        match self.token {
            TokenNL::NewLine(_) => Some(()),
            TokenNL::Token(token) => TokenW { token, reporter }.unexpected("a line break"),
        }
    }

    pub fn token(self, expected: &str) -> Option<TokenW<'a>> {
        let reporter = self.reporter;
        match self.token {
            TokenNL::NewLine(span) => {
                reporter.push(
                    Report::build(ReportKind::Error, span)
                        .with_message(format!("Expected {expected}, found a line break"))
                        .finish(),
                );
                None
            }
            TokenNL::Token(token) => Some(TokenW { token, reporter }),
        }
    }
}

#[derive(Clone)]
struct Tokenizer {
    enclosers: Vec<(Encloser, usize, usize)>,
    file: File,
    qat: ArcIntern<str>,
    spot: usize,
    reporter: Reporter,
}

impl Tokenizer {
    fn new(file: File, reporter: Reporter) -> Tokenizer {
        Tokenizer {
            enclosers: Vec::new(),
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
            .map(|v| v.0)
            .unwrap_or(self.qat().len());
    }

    fn whitespace_amt(&self) -> usize {
        self.qat()
            .find(|c| c != ' ' && c != '\t' && c != '\r')
            .unwrap_or(self.qat().len())
    }

    fn skip_whitespace(&mut self) {
        self.spot += self.whitespace_amt();
    }

    fn whitespace(&self) -> Option<Span> {
        let amt = self.whitespace_amt();
        if amt == 0 {
            None
        } else {
            Some(self.mk_span(self.spot, self.spot + self.whitespace_amt()))
        }
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

    fn next(&mut self) -> Option<TokenNL> {
        self.skip_whitespace();

        if self.qat().is_empty() {
            if self.enclosers.is_empty() {
                return Some(TokenNL::Token(Token::EndOfEnclosure(
                    None,
                    self.mk_span(self.spot, self.spot),
                )));
            }

            for (_, char_start, char_end) in &self.enclosers {
                self.reporter.push(
                    Report::build(ReportKind::Error, self.mk_span(*char_start, *char_end))
                        .with_message("Unclosed delimiter")
                        .finish(),
                );
            }

            return None;
        }

        if let Some((sym, amt)) = self.special_sym() {
            return Some(match sym {
                SpecialSym::Symbol(sym) => {
                    let before = self.spot;
                    self.advance(amt);
                    TokenNL::Token(Token::Symbol(self.mk_span(before, self.spot).with(sym)))
                }
                SpecialSym::Open(encloser) => {
                    let before = self.spot;
                    self.advance(amt);
                    self.enclosers.push((encloser, before, self.spot));
                    TokenNL::Token(Token::Enclosure(encloser, TokenEnclosure { state: self }))
                }
                SpecialSym::Close(encloser) => match self.enclosers.pop() {
                    Some((opener, _, _)) if opener == encloser => {
                        let spot = self.spot;
                        self.advance(amt);
                        TokenNL::Token(Token::EndOfEnclosure(
                            Some(opener),
                            self.mk_span(spot, self.spot),
                        ))
                    }
                    v => {
                        let mut report = Report::build(
                            ReportKind::Error,
                            self.mk_span(self.spot, self.spot + amt),
                        )
                        .with_message("Closing delimiter without corresponding opening delimiter");

                        if let Some((_, char_start, char_end)) = v {
                            report = report.with_label(
                                Label::new(self.mk_span(char_start, char_end)).with_message(
                                    "This is the delimiter that it would like to pair with",
                                ),
                            )
                        }

                        self.reporter.push(report.finish());

                        return None;
                    }
                },
                SpecialSym::Quote => {
                    let quote_start = self.spot;
                    self.advance(amt);

                    let mut text = String::new();

                    loop {
                        let eof = || {
                            self.reporter.push(
                                Report::build(
                                    ReportKind::Error,
                                    self.mk_span(quote_start, self.spot),
                                )
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
                                break TokenNL::Token(Token::Ident(
                                    self.mk_span(quote_start, self.spot)
                                        .with(ArcIntern::from(text)),
                                ));
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
                                text.push(c)
                            }
                        }
                    }
                }
                SpecialSym::NewLine => {
                    let spot = self.spot;
                    // We should only give one newline even if there are a bunch of newlines in a row
                    while self.peek(0) == Some('\n') {
                        self.advance(1);
                        self.skip_whitespace();
                    }

                    TokenNL::NewLine(self.mk_span(spot, self.spot))
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
            });
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

        Some(TokenNL::Token(
            if let Some(directive) = ident.strip_prefix('.') {
                Token::Directive(span.with(ArcIntern::from(directive)))
            } else if let Some(constant) = ident.strip_prefix('$') {
                Token::Constant(span.with(ArcIntern::from(constant)))
            } else if let Ok(num) = ident.parse::<Int<U>>() {
                Token::Number(span.with(num))
            } else {
                Token::Ident(span.with(ArcIntern::from(ident)))
            },
        ))
    }
}
