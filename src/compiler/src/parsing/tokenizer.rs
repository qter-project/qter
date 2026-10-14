use ariadne::{Label, Report, ReportKind};
use internment::ArcIntern;
use puzzle_theory::{
    numbers::{Int, U},
    span::{File, Span, WithSpan},
};

use crate::Reporter;
use crate::rhai::RhaiCode;

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
    /// `%`
    Modulus,
    /// `.`
    Dot,
}

impl Symbol {
    pub fn as_str(self) -> &'static str {
        match self {
            Symbol::Comma => ",",
            Symbol::AssignArrow => "<-",
            Symbol::DefineArrow => "=>",
            Symbol::Colon => ":",
            Symbol::Modulus => "%",
            Symbol::Dot => ".",
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

#[derive(Debug)]
pub enum TokenFlat {
    Ident(ArcIntern<str>),
    Name(ArcIntern<str>),
    Constant(ArcIntern<str>),
    Number(Int<U>),
    Symbol(Symbol),
    RhaiCode(RhaiCode),
    OpenEncloser(Encloser),
    CloseEncloser(Encloser),
}

#[derive(Debug)]
pub enum TokenNLFlat {
    NewLine,
    Whitespace,
    Token(TokenFlat),
}

#[derive(Clone)]
pub(crate) struct Tokenizer {
    file: File,
    qat: ArcIntern<str>,
    pub spot: usize,
    spot_byte: usize,
    reporter: Reporter,
}

impl Tokenizer {
    pub fn new(file: File, reporter: Reporter) -> Tokenizer {
        Tokenizer {
            qat: file.inner(),
            file,
            spot: 0,
            spot_byte: 0,
            reporter,
        }
    }

    fn qat(&self) -> &str {
        &self.qat[self.spot_byte..]
    }

    pub fn skip_shebang(&mut self) {
        if self.qat().starts_with("#!") {
            while self.peek(0).is_some_and(|v| v != '\n') {
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
        self.spot += n;
        self.spot_byte += self
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
        self.spot_byte += amt;
        amt != 0
    }

    pub fn mk_span(&self, start: usize, end: usize) -> Span {
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
            ('%', _) => (S::Symbol(Modulus), 1),
            ('.', _) => (S::Symbol(Dot), 1),
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

        while !self.qat().is_empty() && !self.qat().starts_with(".end-rhai") {
            self.advance(1);
        }

        if self.qat().is_empty() {
            let span = self.mk_span(spot - ".start-rhai".len(), spot);
            self.reporter.push(
                Report::build(ReportKind::Error, span.clone())
                    .with_message("Unterminated Rhai block")
                    .with_label(Label::new(span))
                    .finish(),
            );
            return None;
        }

        let span = self.mk_span(spot, self.spot);

        self.advance(".end-rhai".len());

        Some(span)
    }

    pub fn done(&self) -> bool {
        self.qat().is_empty()
    }
}

impl Iterator for Tokenizer {
    type Item = WithSpan<TokenNLFlat>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done() {
            return None;
        }
        
        let before = self.spot;
        if self.skip_whitespace() {
            return Some(
                self.mk_span(before, self.spot)
                    .with(TokenNLFlat::Whitespace),
            );
        }

        if self.qat().starts_with(".start-rhai") {
            self.advance(".start-rhai".len());
            let rhai = self.take_rhai()?;
            return Some(
                self.mk_span(before, self.spot)
                    .with(TokenNLFlat::Token(TokenFlat::RhaiCode(RhaiCode::new(rhai)))),
            );
        }

        if let Some((sym, amt)) = self.special_sym() {
            let found = match sym {
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

                    if self.peek(0).is_some() {
                        self.advance(1);
                    }

                    TokenNLFlat::NewLine
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

                    TokenNLFlat::Whitespace
                }
            };
            
            return Some(self.mk_span(before, self.spot).with(found));
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

        Some(span.clone().with(TokenNLFlat::Token(
            if let Some(constant) = ident.strip_prefix('$') {
                TokenFlat::Constant(ArcIntern::from(constant))
            } else if let Ok(num) = ident.parse::<Int<U>>() {
                TokenFlat::Number(num)
            } else if ident.chars().any(char::is_uppercase) {
                TokenFlat::Name(ArcIntern::from(ident))
            } else {
                TokenFlat::Ident(ArcIntern::from(ident))
            },
        )))
    }
}
