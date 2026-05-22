use std::ops::{Deref, DerefMut};

#[derive(Clone, Debug)]
pub struct NonemptyVec<T>(Vec<T>);

#[derive(Clone, Copy, Debug)]
pub struct NonemptySlice<'a, T>(&'a [T]);

impl<T> TryFrom<Vec<T>> for NonemptyVec<T> {
    type Error = ();

    fn try_from(value: Vec<T>) -> Result<Self, Self::Error> {
        if value.is_empty() {
            Err(())
        } else {
            Ok(Self(value))
        }
    }
}

impl<T> DerefMut for NonemptyVec<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> Deref for NonemptyVec<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Deref for NonemptySlice<'_, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<T> NonemptyVec<T> {
    #[must_use]
    pub fn split_first(&self) -> (&T, &[T]) {
        // SAFETY: this collection has at least one element
        unsafe { self.0.split_first().unwrap_unchecked() }
    }

    #[must_use]
    pub fn last(&self) -> &T {
        // SAFETY: this collection has at least one element
        unsafe { self.0.last().unwrap_unchecked() }
    }

    #[must_use]
    pub fn as_slice(&self) -> NonemptySlice<'_, T> {
        NonemptySlice(&self.0)
    }
}
