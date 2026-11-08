use std::sync::Arc;

use ariadne::Report;
use qter_core::span::Span;

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

impl<'a> IntoIterator for &'a Reporter {
    type Item = (usize, &'a ariadne::Report<'static, qter_core::span::Span>);
    type IntoIter = boxcar::vec::Iter<'a, ariadne::Report<'static, qter_core::span::Span>>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
