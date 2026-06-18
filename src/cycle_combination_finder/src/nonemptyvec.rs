use std::{
    num::NonZeroUsize,
    ops::{Deref, DerefMut},
    slice,
};

#[derive(Clone, Debug)]
pub struct NonemptyVec<T>(Vec<T>);

#[derive(Debug)]
pub struct NonemptySlice<'a, T>(&'a [T]);

impl<T> Clone for NonemptySlice<'_, T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for NonemptySlice<'_, T> {}

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

impl<'a, T> TryFrom<&'a [T]> for NonemptySlice<'a, T> {
    type Error = ();

    fn try_from(value: &'a [T]) -> Result<Self, Self::Error> {
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
    type Target = Vec<T>;

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
    pub fn len(&self) -> NonZeroUsize {
        unsafe { NonZeroUsize::new_unchecked(self.0.len()) }
    }

    #[must_use]
    pub fn split_last(&self) -> (&T, &[T]) {
        // SAFETY: this collection has at least one element
        unsafe { self.0.split_last().unwrap_unchecked() }
    }

    pub fn first_mut(&mut self) -> &mut T {
        // SAFETY: this collection has at least one element
        unsafe { self.0.first_mut().unwrap_unchecked() }
    }

    #[must_use]
    pub fn last(&self) -> &T {
        // SAFETY: this collection has at least one element
        unsafe { self.0.last().unwrap_unchecked() }
    }

    #[must_use]
    pub fn first(&self) -> &T {
        // SAFETY: this collection has at least one element
        unsafe { self.0.first().unwrap_unchecked() }
    }

    #[must_use]
    pub fn as_slice(&self) -> NonemptySlice<'_, T> {
        // Construction is allowed because this collection has at least one element
        NonemptySlice(&self.0)
    }
}

impl<'a, T> NonemptySlice<'a, T> {
    /// # Safety
    /// 
    /// Follow `slice::from_raw_parts`
    pub unsafe fn from_raw_parts(data: *const T, len: NonZeroUsize) -> Self {
        // SAFETY: upheld by caller
        Self(unsafe { slice::from_raw_parts(data, len.get()) })
    }

    #[must_use]
    pub fn split_first(self) -> (&'a T, &'a [T]) {
        // SAFETY: this collection has at least one element
        unsafe { self.0.split_first().unwrap_unchecked() }
    }

    #[must_use]
    pub fn split_last(self) -> (&'a T, &'a [T]) {
        // SAFETY: this collection has at least one element
        unsafe { self.0.split_last().unwrap_unchecked() }
    }
}
