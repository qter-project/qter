use std::ops::{Deref, DerefMut};

use ariadne::Span as _;
use sarlacc::Intern;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct File {
    name: Intern<str>,
    contents: Intern<str>,
}

impl File {
    #[must_use]
    pub fn new(name: Intern<str>, contents: Intern<str>) -> File {
        File { name, contents }
    }

    #[must_use]
    pub fn inner(&self) -> Intern<str> {
        self.contents
    }

    #[must_use]
    pub fn name(&self) -> Intern<str> {
        self.name
    }
}

/// A slice of the original source code; to be attached to pieces of data for error reporting
#[derive(Clone, Copy)]
pub struct Span {
    source: File,
    start: usize,
    end: usize,
    start_idx: usize,
    end_idx: usize,
}

impl PartialEq for Span {
    fn eq(&self, other: &Self) -> bool {
        if self.source.contents == other.source.contents {
            self.start == other.start && self.end == other.end
        } else {
            self.slice() == other.slice()
        }
    }
}

impl Eq for Span {}

impl Span {
    /// Creates a new `Span` from the given source and start/end positions
    ///
    /// # Panics
    ///
    /// Panics if the start or end positions are out of bounds, or if the start is greater than the end
    #[must_use]
    pub fn new(source: File, start: usize, end: usize) -> Span {
        assert!(start <= end);
        let count = source.contents.chars().count();
        assert!(start <= count);
        assert!(end <= count);

        Span {
            start_idx: source
                .inner()
                .char_indices()
                .nth(start)
                .map_or(source.inner().len(), |(a, _)| a),
            end_idx: source
                .inner()
                .char_indices()
                .nth(end)
                .map_or(source.inner().len(), |(a, _)| a),
            source,
            start,
            end,
        }
    }

    #[cfg(test)]
    #[must_use]
    pub fn from_static(str: &'static str) -> Span {
        Span::new(
            File::new(Intern::from_ref("<static>"), Intern::from_ref(str)),
            0,
            str.len(),
        )
    }

    #[must_use]
    pub fn slice(&self) -> &str {
        &self.source.contents[self.start_idx..self.end_idx]
    }

    #[must_use]
    pub fn after(mut self) -> Span {
        self.start = self.end;
        self
    }

    #[must_use]
    pub fn source(&self) -> File {
        self.source
    }

    /// Merges two spans into one, keeping the earliest start and latest end
    ///
    /// TODO: Get rid of this function
    ///
    /// # Panics
    ///
    /// Panics if the two spans are from different sources
    #[must_use]
    pub fn merge(self, other: Span) -> Span {
        if self.source == other.source {
            Span {
                source: self.source,
                start: self.start.min(other.start),
                start_idx: self.start_idx.min(other.start_idx),
                end: self.end.max(other.end),
                end_idx: self.end_idx.max(other.end_idx),
            }
        } else {
            // This is really stupid but there isn't really a great way to deal with this. The proper way would be for spans to have "children" but we don't have that implemented
            self
        }
    }

    pub fn with<T>(self, v: T) -> WithSpan<T> {
        WithSpan::new(v, self)
    }
}

impl AsRef<str> for Span {
    fn as_ref(&self) -> &str {
        self.slice()
    }
}

impl ariadne::Span for Span {
    type SourceId = Intern<str>;

    fn source(&self) -> &Self::SourceId {
        &self.source.name
    }

    fn start(&self) -> usize {
        self.start
    }

    fn end(&self) -> usize {
        self.end
    }
}

impl core::fmt::Debug for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slice())
    }
}

impl core::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", self.start(), self.end())
    }
}

/// A value attached to a `Span`
#[derive(Clone, Copy)]
pub struct WithSpan<T> {
    pub value: T,
    span: Span,
}

impl<T: core::fmt::Debug> core::fmt::Debug for WithSpan<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        core::fmt::Debug::fmt(&self.value, f)
    }
}

impl<T> Deref for WithSpan<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for WithSpan<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T> WithSpan<T> {
    pub fn new(value: T, span: Span) -> WithSpan<T> {
        WithSpan { value, span }
    }

    pub fn into_inner(self) -> T {
        self.value
    }

    pub fn map<V>(self, f: impl FnOnce(T) -> V) -> WithSpan<V> {
        WithSpan {
            value: f(self.value),
            span: self.span,
        }
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

impl<T> WithSpan<MaybeErr<T>> {
    pub fn spanspose(self) -> MaybeErr<WithSpan<T>> {
        self.value.map(|v| self.span.with(v))
    }
}

impl<T: PartialEq> PartialEq for WithSpan<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T: Eq> Eq for WithSpan<T> {}

impl<T: core::hash::Hash> core::hash::Hash for WithSpan<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

pub enum MaybeErr<T> {
    Some(T),
    None,
}

impl<T> MaybeErr<T> {
    pub fn map<X>(self, f: impl FnOnce(T) -> X) -> MaybeErr<X> {
        match self {
            MaybeErr::Some(v) => MaybeErr::Some(f(v)),
            MaybeErr::None => MaybeErr::None,
        }
    }

    pub fn option(self) -> Option<T> {
        match self {
            MaybeErr::Some(v) => Some(v),
            MaybeErr::None => None,
        }
    }
}

impl<T> MaybeErr<MaybeErr<T>> {
    pub fn flatten(self) -> MaybeErr<T> {
        match self {
            MaybeErr::Some(v) => v,
            MaybeErr::None => MaybeErr::None,
        }
    }
}

impl<T: Default> Default for MaybeErr<T> {
    fn default() -> Self {
        MaybeErr::Some(T::default())
    }
}
