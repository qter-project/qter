unsafe extern "C" {
    /// Implemented in `cpp/permutator.cpp`, compiled and linked by `build.rs`.
    fn qter_pandita2(perm: *mut u8, len: usize);
}

/// Use an alternative implementation of the Pandita algorithm to compute the
/// next lexicographic permutation of a slice. From: <https://www.geeksforgeeks.org/lexicographic-permutations-of-string/>
///
/// # Safety
///
/// `perm` must have length at least two and must contain unique elements.
pub unsafe fn pandita2(perm: &mut [u8]) {
    // SAFETY: `perm` is a valid buffer of `perm.len()` bytes, and the caller
    // upholds the length and uniqueness requirements
    unsafe {
        qter_pandita2(perm.as_mut_ptr(), perm.len());
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
