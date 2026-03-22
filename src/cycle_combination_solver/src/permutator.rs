use std::ptr;

/// Reverse the elements in the slice `perm` from index `s` to index `e`,
/// inclusive.
///
/// # Safety
///
/// `s` and `e` must be valid indices for `perm`, and `s` must be less than or
/// equal to `e`.
unsafe fn reverse_unchecked(perm: &mut [u8], mut s: usize, mut e: usize) {
    while s < e {
        // SAFETY: `perm` is a mutable slice, and `s` and `e` are valid indices
        // for `perm` by the caller
        unsafe {
            swap_unchecked(perm, s, e);
        }
        s += 1;
        e -= 1;
    }
}

/// Swaps the elements at indices `i` and `j` in the slice `perm`.
///
/// # Safety
///
/// `i` and `j` must be valid indices for `perm`
unsafe fn swap_unchecked(perm: &mut [u8], i: usize, j: usize) {
    unsafe {
        ptr::swap(perm.as_mut_ptr().add(i), perm.as_mut_ptr().add(j));
    }
}

/// Use an alternative implementation of the Pandita algorithm to compute the
/// next lexicographic permutation of a slice. From: <https://www.geeksforgeeks.org/lexicographic-permutations-of-string/>
///
/// # Safety
///
/// `perm` must have length at least two and must contain unique elements.
pub unsafe fn pandita2(perm: &mut [u8]) {
    // Benchmarked on a 2025 Mac M4: 170.49ns (test_big) 2.84ns (test_small)

    let len = perm.len();
    let mut i = len - 2;
    // SAFETY: The safety backed by the correctness of the implementation of the
    // given algorithm
    unsafe {
        while perm.get_unchecked(i) >= perm.get_unchecked(i + 1) {
            if i == 0 {
                return;
            }
            i -= 1;
        }
        let mut j = len - 1;
        while perm.get_unchecked(j) <= perm.get_unchecked(i) {
            j -= 1;
        }
        swap_unchecked(perm, i, j);
        reverse_unchecked(perm, i + 1, len - 1);
    }
}

#[cfg(test)]
mod tests {
    extern crate test;

    use itertools::Itertools;

    use super::*;

    const PERM_FIVE: [[u8; 5]; 120] = [
        [0, 1, 2, 3, 4],
        [0, 1, 2, 4, 3],
        [0, 1, 3, 2, 4],
        [0, 1, 3, 4, 2],
        [0, 1, 4, 2, 3],
        [0, 1, 4, 3, 2],
        [0, 2, 1, 3, 4],
        [0, 2, 1, 4, 3],
        [0, 2, 3, 1, 4],
        [0, 2, 3, 4, 1],
        [0, 2, 4, 1, 3],
        [0, 2, 4, 3, 1],
        [0, 3, 1, 2, 4],
        [0, 3, 1, 4, 2],
        [0, 3, 2, 1, 4],
        [0, 3, 2, 4, 1],
        [0, 3, 4, 1, 2],
        [0, 3, 4, 2, 1],
        [0, 4, 1, 2, 3],
        [0, 4, 1, 3, 2],
        [0, 4, 2, 1, 3],
        [0, 4, 2, 3, 1],
        [0, 4, 3, 1, 2],
        [0, 4, 3, 2, 1],
        [1, 0, 2, 3, 4],
        [1, 0, 2, 4, 3],
        [1, 0, 3, 2, 4],
        [1, 0, 3, 4, 2],
        [1, 0, 4, 2, 3],
        [1, 0, 4, 3, 2],
        [1, 2, 0, 3, 4],
        [1, 2, 0, 4, 3],
        [1, 2, 3, 0, 4],
        [1, 2, 3, 4, 0],
        [1, 2, 4, 0, 3],
        [1, 2, 4, 3, 0],
        [1, 3, 0, 2, 4],
        [1, 3, 0, 4, 2],
        [1, 3, 2, 0, 4],
        [1, 3, 2, 4, 0],
        [1, 3, 4, 0, 2],
        [1, 3, 4, 2, 0],
        [1, 4, 0, 2, 3],
        [1, 4, 0, 3, 2],
        [1, 4, 2, 0, 3],
        [1, 4, 2, 3, 0],
        [1, 4, 3, 0, 2],
        [1, 4, 3, 2, 0],
        [2, 0, 1, 3, 4],
        [2, 0, 1, 4, 3],
        [2, 0, 3, 1, 4],
        [2, 0, 3, 4, 1],
        [2, 0, 4, 1, 3],
        [2, 0, 4, 3, 1],
        [2, 1, 0, 3, 4],
        [2, 1, 0, 4, 3],
        [2, 1, 3, 0, 4],
        [2, 1, 3, 4, 0],
        [2, 1, 4, 0, 3],
        [2, 1, 4, 3, 0],
        [2, 3, 0, 1, 4],
        [2, 3, 0, 4, 1],
        [2, 3, 1, 0, 4],
        [2, 3, 1, 4, 0],
        [2, 3, 4, 0, 1],
        [2, 3, 4, 1, 0],
        [2, 4, 0, 1, 3],
        [2, 4, 0, 3, 1],
        [2, 4, 1, 0, 3],
        [2, 4, 1, 3, 0],
        [2, 4, 3, 0, 1],
        [2, 4, 3, 1, 0],
        [3, 0, 1, 2, 4],
        [3, 0, 1, 4, 2],
        [3, 0, 2, 1, 4],
        [3, 0, 2, 4, 1],
        [3, 0, 4, 1, 2],
        [3, 0, 4, 2, 1],
        [3, 1, 0, 2, 4],
        [3, 1, 0, 4, 2],
        [3, 1, 2, 0, 4],
        [3, 1, 2, 4, 0],
        [3, 1, 4, 0, 2],
        [3, 1, 4, 2, 0],
        [3, 2, 0, 1, 4],
        [3, 2, 0, 4, 1],
        [3, 2, 1, 0, 4],
        [3, 2, 1, 4, 0],
        [3, 2, 4, 0, 1],
        [3, 2, 4, 1, 0],
        [3, 4, 0, 1, 2],
        [3, 4, 0, 2, 1],
        [3, 4, 1, 0, 2],
        [3, 4, 1, 2, 0],
        [3, 4, 2, 0, 1],
        [3, 4, 2, 1, 0],
        [4, 0, 1, 2, 3],
        [4, 0, 1, 3, 2],
        [4, 0, 2, 1, 3],
        [4, 0, 2, 3, 1],
        [4, 0, 3, 1, 2],
        [4, 0, 3, 2, 1],
        [4, 1, 0, 2, 3],
        [4, 1, 0, 3, 2],
        [4, 1, 2, 0, 3],
        [4, 1, 2, 3, 0],
        [4, 1, 3, 0, 2],
        [4, 1, 3, 2, 0],
        [4, 2, 0, 1, 3],
        [4, 2, 0, 3, 1],
        [4, 2, 1, 0, 3],
        [4, 2, 1, 3, 0],
        [4, 2, 3, 0, 1],
        [4, 2, 3, 1, 0],
        [4, 3, 0, 1, 2],
        [4, 3, 0, 2, 1],
        [4, 3, 1, 0, 2],
        [4, 3, 1, 2, 0],
        [4, 3, 2, 0, 1],
        [4, 3, 2, 1, 0],
    ];

    const PERM_FOUR: [[u8; 4]; 24] = [
        [0, 1, 2, 3],
        [0, 1, 3, 2],
        [0, 2, 1, 3],
        [0, 2, 3, 1],
        [0, 3, 1, 2],
        [0, 3, 2, 1],
        [1, 0, 2, 3],
        [1, 0, 3, 2],
        [1, 2, 0, 3],
        [1, 2, 3, 0],
        [1, 3, 0, 2],
        [1, 3, 2, 0],
        [2, 0, 1, 3],
        [2, 0, 3, 1],
        [2, 1, 0, 3],
        [2, 1, 3, 0],
        [2, 3, 0, 1],
        [2, 3, 1, 0],
        [3, 0, 1, 2],
        [3, 0, 2, 1],
        [3, 1, 0, 2],
        [3, 1, 2, 0],
        [3, 2, 0, 1],
        [3, 2, 1, 0],
    ];

    #[test]
    fn test_pandita2() {
        let mut len = 5;
        let mut perm = (0..len).collect_vec();
        let mut i = 1;

        while i < PERM_FIVE.len() {
            unsafe { pandita2(&mut perm) };
            assert_eq!(perm, PERM_FIVE[i]);
            i += 1;
        }

        len = 4;
        perm = (0..len).collect_vec();
        i = 1;
        while i < PERM_FOUR.len() {
            unsafe { pandita2(&mut perm) };
            assert_eq!(perm, PERM_FOUR[i]);
            i += 1;
        }
    }

    #[bench]
    fn bench_pandita2_small(b: &mut test::Bencher) {
        let len = 12;
        let mut perm = (0..len).collect_vec().into_boxed_slice();
        b.iter(|| unsafe {
            pandita2(test::black_box(&mut perm));
        });
    }

    #[bench]
    fn bench_pandita2_big(b: &mut test::Bencher) {
        let len = 5;
        let mut perm = vec![0; len as usize].into_boxed_slice();
        b.iter(|| {
            for i in 0..len {
                perm[i as usize] = i;
            }
            let mut i = 1;
            while i < test::black_box(PERM_FIVE.len()) {
                unsafe { pandita2(test::black_box(&mut perm)) };
                i += 1;
            }
        });
    }
}
