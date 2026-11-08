use std::sync::Arc;

use ariadne::Report;
use qter_core::span::Span;

#[derive(Clone, Default, Debug)]
pub struct Reporter(Arc<boxcar::Vec<Report<'static, Span>>>);

impl Reporter {
    pub fn push(&self, report: Report<'static, Span>) -> ReportToken {
        self.0.push(report);
        ReportToken(())
    }

    /// Marks the current position in the reporter so that `errors_since` can check whether any reports were pushed after this point
    pub fn count_token(&self) -> ReportCount {
        ReportCount(self.count())
    }

    /// Returns proof of an error if any reports were pushed since `marker` was created
    ///
    /// # Errors
    ///
    /// Returns an error if any reports were pushed since `marker` was created
    pub fn errors_since(&self, marker: ReportCount) -> OrErrorProof<()> {
        if self.count() > marker.0 {
            Err(ReportToken(()))
        } else {
            Ok(())
        }
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

#[must_use]
#[derive(Debug)]
pub struct ReportToken(());

#[must_use]
#[derive(Clone, Copy, Debug)]
pub struct ReportCount(usize);

pub type OrErrorProof<T> = core::result::Result<T, ReportToken>;
