use std::ops::{Deref, Index, IndexMut};

#[derive(Clone, Debug)]
pub struct LeastOneVec<T>(Vec<T>);

#[derive(Clone, Copy, Debug)]
pub struct LeastOneSlice<'a, T>(&'a [T]);

impl<T> TryFrom<Vec<T>> for LeastOneVec<T> {
    type Error = ();

    fn try_from(value: Vec<T>) -> Result<Self, Self::Error> {
        if value.len() > 1 {
            Ok(Self(value))
        } else {
            Err(())
        }
    }
}

impl<T> Deref for LeastOneVec<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Deref for LeastOneSlice<'_, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<T> Index<usize> for LeastOneVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl<T> IndexMut<usize> for LeastOneVec<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl<T> LeastOneVec<T> {
    /// Returns a mutable reference to an element or subslice, without doing
    /// bounds checking.
    ///
    /// # Safety
    ///
    /// See `[<[T]>::get_unchecked_mut]`
    pub unsafe fn get_unchecked_mut(&mut self, i: usize) -> &mut T {
        // SAFETY: provided by caller
        unsafe { self.0.get_unchecked_mut(i) }
    }

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
    pub fn as_slice(&self) -> LeastOneSlice<'_, T> {
        LeastOneSlice(&self.0)
    }
}
