//! An AVX2 optimized implementation for 3x3 cubes

#![cfg_attr(not(avx2), allow(dead_code, unused_variables))]

use super::common::{CornersTransformation, Cube3OrbitType, Cube3State, EdgesTransformation};
use crate::{
    orbit_puzzle::exact_hasher_orbit,
    puzzle::{SortedCycleStructureRef, cube3::CUBE_3_SORTED_ORBIT_DEFS},
};
use std::{
    fmt::{self, Formatter},
    hash::Hash,
    num::NonZeroU8,
    simd::{
        cmp::{SimdOrd, SimdPartialEq, SimdPartialOrd},
        num::SimdInt,
        u8x8, u8x16, u8x32,
    },
};

#[cfg(all(avx2, target_arch = "x86"))]
use core::arch::x86::_mm256_shuffle_epi8;
#[cfg(all(avx2, target_arch = "x86_64"))]
use core::arch::x86_64::_mm256_shuffle_epi8;

/// A representation of a 3x3 cube in a __m256i vector. The following design
/// has been taken from Andrew Skalski's [vcube].
///
/// Low 128 bits:
///
/// ```text
/// ---OEEEE
/// ---OEEEE
/// ---OEEEE
/// ---OEEEE
/// ---OEEEE
/// ---OEEEE
/// ---OEEEE
/// ---OEEEE
/// ---OEEEE
/// ---OEEEE
/// ---OEEEE
/// ---OEEEE
/// ----11--
/// ----11-1
/// ----111-
/// ----1111
/// ```
///
/// dash = unused (zero) \
/// E = edge index (0-11) \
/// O = edge orientation (0-1)
///
/// High 128 bits:
///
/// ```text
/// --OO-CCC
/// --OO-CCC
/// --OO-CCC
/// --OO-CCC
/// --OO-CCC
/// --OO-CCC
/// --OO-CCC
/// --OO-CCC
/// ----1---
/// ----1--1
/// ----1-1-
/// ----1-11
/// ----11--
/// ----11-1
/// ----111-
/// ----1111
/// ```
///
/// dash = unused (zero) \
/// C = corner index (0-7) \
/// O = corner orientation (0-2)
///
/// It is important for the unused bytes to correspond to their index for the
/// `_mm256_shuffle_epi8` instruction to work correctly.
///
/// [vcube]: https://github.com/Voltara/vcube
#[allow(clippy::derived_hash_with_manual_eq)]
#[derive(Clone, Hash)]
pub struct Cube3(u8x32);

/// Extract the permutation bits from the cube state for u8x32.
const PERM_MASK_1: u8x32 = u8x32::splat(0b0000_1111);
/// Extract the permutation bits from the cube state for u8x16.
const PERM_MASK_2: u8x16 = u8x16::splat(0b0000_1111);
/// Extract the permutation bits from the cube state for u8x8.
const PERM_MASK_3: u8x8 = u8x8::splat(0b0000_1111);
/// Extract the orientation bits from the cube state.
const ORI_MASK: u8x32 = u8x32::splat(0b0011_0000);
/// The carry constant used to fix orientation bits after composition.
const ORI_CARRY: u8x32 = u8x32::from_array([
    0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20, 0x20,
    0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
]);
/// The carry constant used to fix orientation bits after inversion.
const ORI_CARRY_INVERSE: u8x32 = u8x32::from_array([
    0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10,
    0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
]);
/// The identity cube state.
const IDENTITY: u8x32 = u8x32::from_array([
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
    13, 14, 15,
]);
/// An unitialized cube state.
const BLANK: u8x32 = u8x32::from_array([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 12, 13, 14, 15, 0, 0, 0, 0, 0, 0, 0, 0, 8, 9, 10, 11, 12,
    13, 14, 15,
]);
/// The starting index for edge bits.
const EDGE_START: usize = 0;
/// The starting index for corner bits.
const CORNER_START: usize = 16;

/// A zero-cost wrapper around `_mm256_shuffle_epi8`.
fn avx2_swizzle_lo(a: u8x32, b: u8x32) -> u8x32 {
    #[cfg(avx2)]
    // SAFETY: cfg guarantees that AVX2 is available
    unsafe {
        _mm256_shuffle_epi8(a.into(), b.into()).into()
    }
    #[cfg(not(avx2))]
    unimplemented!()
}

/// Extract the edge bits from a permutation identity equality bitmask
fn edge_bits(bitmask: u64) -> u64 {
    bitmask & u64::from(!0_u16)
}

/// Extract the corner bits from a permutation identity equality bitmask
fn corner_bits(bitmask: u64) -> u64 {
    bitmask >> CORNER_START
}

impl PartialEq for Cube3 {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        #[cfg(avx2)]
        extern "vectorcall" fn eq_vectorcall(a: &Cube3, b: &Cube3) -> bool {
            a.0 == b.0
        }
        #[cfg(not(avx2))]
        fn eq_vectorcall(a: &Cube3, b: &Cube3) -> bool {
            unimplemented!()
        }
        eq_vectorcall(self, other)
    }
}

impl fmt::Debug for Cube3 {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut ep = [0; 16];
        let mut eo = [0; 16];
        let mut cp = [0; 16];
        let mut co = [0; 16];

        for i in 0..16 {
            ep[i] = self.0[i] & 0b1111;
            eo[i] = self.0[i] >> 4;
        }

        for i in 16..32 {
            cp[i - 16] = self.0[i] & 0b111;
            co[i - 16] = self.0[i] >> 4;
        }

        f.debug_struct("Cube3")
            .field("ep", &ep)
            .field("eo", &eo)
            .field("cp", &cp)
            .field("co", &co)
            .finish()
    }
}

impl Cube3State for Cube3 {
    type OrbitBytesBuf = u8x16;

    fn from_corner_and_edge_transformations(
        corners_transformation: CornersTransformation,
        edges_transformation: EdgesTransformation,
    ) -> Self {
        let corners_transformation = corners_transformation.get();
        let edges_transformation = edges_transformation.get();
        let mut cube = BLANK;

        for i in 0..12 {
            let (perm, ori) = edges_transformation[i];
            cube[i] = perm | (ori << 4);
        }

        for i in 16..24 {
            let (perm, ori) = corners_transformation[i - 16];
            cube[i] = perm | (ori << 4);
        }

        Cube3(cube)
    }

    #[inline(always)]
    fn replace_compose(&mut self, a: &Self, b: &Self) {
        // Benchmarked on a 2x Intel Xeon E5-2667 v3: 1.41ns
        fn inner(dst: &mut Cube3, a: &Cube3, b: &Cube3) {
            // First use _mm256_shuffle_epi8 to compose the permutation. Note
            // that the SIMD instruction shuffles its argument bytes by the
            // lower four bits of the second argument, meaning orientation will
            // not interfere.
            let mut composed = avx2_swizzle_lo(a.0, b.0);
            // Composing permutation composes the orientation bits too. "The
            // Cubie Level" of Kociemba's [website] explains that orientation
            // during composition changes like so: (A*B)(x).o=A(B(x).c).o+B(x).o
            // We've just done the first part, so we now need to add the add
            // the orientation bits of the second argument to the first.
            //
            // [website]: https://kociemba.org/cube.htm
            composed += b.0 & ORI_MASK;
            // Once added, the orientation bits may be in an invalid state. Each
            // corner orientation index is defined as 0, 1, or 2, but it may be
            // 4 or 5 after the addition. We subtract the value 0b0011_0000 or 3
            // from each orientation value and minimize it with the original
            // value. This will do nothing to values that are already 0, 1, or 2
            // because of overflow, but it will set the values 3 and 4 to 0 and
            // 1 respectively.
            composed = composed.simd_min(composed - ORI_CARRY);

            dst.0 = composed;
        }
        // The vectorcall ABI is used to speed up the function calls
        #[cfg(avx2)]
        extern "vectorcall" fn replace_compose_vectorcall(dst: &mut Cube3, a: &Cube3, b: &Cube3) {
            inner(dst, a, b)
        }
        #[cfg(not(avx2))]
        fn replace_compose_vectorcall(dst: &mut Cube3, a: &Cube3, b: &Cube3) {
            inner(dst, a, b);
        }
        replace_compose_vectorcall(self, a, b);
    }

    #[inline(always)]
    fn replace_inverse(&mut self, a: &Self) {
        // Benchmarked on a 2x Intel Xeon E5-2667 v3: 5.58ns
        fn inner(dst: &mut Cube3, a: &Cube3) {
            // Permutation inversion taken from Andrew Skalski's [vcube].
            //
            // 27720 (11*9*8*7*5) is the LCM of all possible cycle
            // decompositions, so we can invert the permutation by raising it to
            // the 27719th power. The addition chain for 27719 was generated
            // using the calculator provided by Achim Flammenkamp on his
            // [website].
            //
            // [vcube]: https://github.com/Voltara/vcube
            // [website]: http://wwwhomes.uni-bielefeld.de/achim/addition_chain.html

            // Extract the permutation bits from the cube state
            let perm = a.0 & PERM_MASK_1;

            let pow_3 = avx2_swizzle_lo(avx2_swizzle_lo(perm, perm), perm);
            let mut inverse = avx2_swizzle_lo(pow_3, pow_3);
            inverse = avx2_swizzle_lo(inverse, inverse);
            inverse = avx2_swizzle_lo(avx2_swizzle_lo(inverse, inverse), pow_3);
            inverse = avx2_swizzle_lo(inverse, inverse);
            inverse = avx2_swizzle_lo(inverse, inverse);
            inverse = avx2_swizzle_lo(inverse, inverse);
            inverse = avx2_swizzle_lo(avx2_swizzle_lo(inverse, inverse), perm);
            inverse = avx2_swizzle_lo(inverse, inverse);
            inverse = avx2_swizzle_lo(inverse, inverse);
            inverse = avx2_swizzle_lo(inverse, inverse);
            inverse = avx2_swizzle_lo(inverse, inverse);
            inverse = avx2_swizzle_lo(avx2_swizzle_lo(inverse, inverse), pow_3);
            inverse = avx2_swizzle_lo(avx2_swizzle_lo(inverse, inverse), perm);

            // Compose orientation as explained in `replace_compose`
            // xoring this with `perm` does not make this faster
            let mut added_ori = a.0 & ORI_MASK;
            added_ori += added_ori;
            // The orientation for edges remain the same during inversion, so
            // we slightly modify the carry constant
            added_ori = added_ori.simd_min(added_ori - ORI_CARRY_INVERSE);

            // Use the inverse permutation to permute the already inversed
            // orientation bits
            added_ori = avx2_swizzle_lo(added_ori, inverse);
            *dst = Cube3(inverse | added_ori);
        }
        // The vectorcall ABI is used to speed up the function calls
        #[cfg(avx2)]
        extern "vectorcall" fn replace_inverse_vectorcall(dst: &mut Cube3, a: &Cube3) {
            inner(dst, a);
        }
        #[cfg(not(avx2))]
        fn replace_inverse_vectorcall(dst: &mut Cube3, a: &Cube3) {
            inner(dst, a);
        }
        replace_inverse_vectorcall(self, a);
    }

    fn induces_sorted_cycle_structure(
        &self,
        sorted_cycle_structure: SortedCycleStructureRef,
    ) -> bool {
        // Benchmarked on a 2x Intel Xeon E5-2667 v3: 36.47ns (worst) 7.33ns (average)
        //
        // The cycle structure of a state is a sorted list of
        // (int: cycle_length, bool: is_oriented). For example, the corner
        // permutation:
        //
        // index 0 1 2 3 4 5 6 7
        // perm  0 2 3 5 4 1 7 6
        //
        // has cycle lengths 2 and 4. is_oriented is true for each cycle length
        // when the sum of orientation values of each piece in the cycle is
        // nonzero modulo the orbit's orientation count.
        //
        // To structure if a state induces a sorted cycle structure we use a
        // SIMD shuffling and counting approach. We perform the following for
        // the corners and edges orbit. We first focus on finding the cycle
        // lengths. To do so, repeatedly compose the permutation with its
        // original value and equality mask it with the identity permutation. If
        // any element is set, then there are X / Y cycles of length Y, where X
        // is the number of set elements and Y is the number of repititions.
        // This should make sense; after two repetitions, if there are six set
        // elements, there must be three cycles of length two. Each of the two
        // pieces of the three cycles are returning to their original position.
        // To account for orientation, we first make note of the nonobvious fact
        // that every piece's orientation is the same when cycled back to their
        // original positions. In a cycle, think of each piece position as an
        // addition to the visitor's orientation count. Each piece cycles
        // through each position exactly once, so each piece will be added to by
        // the same value (the sum of each piece's orientation value). Since the
        // identity state has every orientation value the same (zero), each
        // piece must therefore have the same orientation value after a full
        // cycle. Thus, we can find how many of those cycles of the same length
        // are oriented cycles by using same technique.

        // Helps avoid bound checks in codegen
        #![allow(clippy::int_plus_one)]

        let sorted_corners_cycle_structure =
            unsafe { sorted_cycle_structure.inner.get_unchecked(0) };
        let sorted_edges_cycle_structure = unsafe { sorted_cycle_structure.inner.get_unchecked(1) };

        // The orientation to add every composition repetition
        let compose_ori = self.0 & ORI_MASK;
        // A rolling mask of the pieces that have been seen. Once there are
        // no more pieces to see, we have our result
        let mut seen_perm = (self.0 & PERM_MASK_1).simd_eq(IDENTITY);

        // Create a mask of cycles that are (1, true), or just orient in place.
        // Special case for this first cycle because it is convenient and fast
        let oriented_one_cycle_corner_mask = seen_perm & compose_ori.simd_ne(u8x32::splat(0));

        // We need a way to cross-reference the computed cycles with the given
        // cycle structure to test if the current state induces it. I settled on
        // using an index pointer to the given cycle structure list, taking
        // advantage of the fact that the list is sorted.
        //
        // Let's say we test against the cycle structure [(1, true), (1, true),
        // (2, false), (4, true)]. We have already presented an algorithm to
        // compute the number of cycles of each length. For every cycle length,
        // we can add that number to the index pointer starting at -1 and check
        // if that index is that computed cycle length. It is trivial to extend
        // this approach to account for orientations.
        //
        // We initialize the index pointer to the number of oriented one cycles
        let mut corner_cycle_structure_pointer =
            corner_bits(oriented_one_cycle_corner_mask.to_bitmask()).count_ones() as usize;
        // Subtract one to make it zero indexed
        corner_cycle_structure_pointer = corner_cycle_structure_pointer.wrapping_sub(1);

        // Error checking for the (1, true) cycle structure. This isn't actually
        // necessary. It's just a hot path for failing early
        if corner_cycle_structure_pointer == usize::MAX {
            // If the state has no (1, true) cycle structures, then get the
            // first cycle of the specified sorted cycle structure
            if let Some(&first_cycle) = sorted_corners_cycle_structure.first()
                // If sorted cycle structure has a first cycle, it must not be
                // (1, true) because this branch only runs when the state has no
                // (1, true) cycle structures
                && first_cycle == (1.try_into().unwrap(), true)
            {
                // So we short circuit
                return false;
            }
        } else if
        // If the corner cycle structure pointer is out of range then something
        // is wrong
        corner_cycle_structure_pointer >= sorted_corners_cycle_structure.len()
            // If the corner cycle structure is in range, but it doesn't point
            // to (1, true) then the cycle structure is mismatched
            || sorted_corners_cycle_structure[corner_cycle_structure_pointer] != (1.try_into().unwrap(), true)
        {
            // In both cases short circuit
            return false;
        }

        // We use the same techniques as above but for edges
        let oriented_one_cycle_edge_mask = seen_perm & compose_ori.simd_ne(u8x32::splat(0));
        let mut edge_cycle_structure_pointer =
            edge_bits(oriented_one_cycle_edge_mask.to_bitmask()).count_ones() as usize;
        edge_cycle_structure_pointer = edge_cycle_structure_pointer.wrapping_sub(1);

        if edge_cycle_structure_pointer == usize::MAX {
            if let Some(&first_cycle) = sorted_edges_cycle_structure.first()
                && first_cycle == (1.try_into().unwrap(), true)
            {
                return false;
            }
        } else if edge_cycle_structure_pointer >= sorted_edges_cycle_structure.len()
            || sorted_edges_cycle_structure[edge_cycle_structure_pointer]
                != (1.try_into().unwrap(), true)
        {
            return false;
        }

        // The main loop. We repeatedly compose the permutation with its
        // original value and check if any new pieces have been seen. First
        // initialize the number of composition reptitions to 2, because we just
        // processed the first composition, and we are about to process the
        // second. We use a `NonZeroU8` to avoid division bounds checking
        let mut reps = NonZeroU8::new(2).unwrap();
        // The repeatedly composed permutation
        let mut iter = self.0;
        while !seen_perm.all() {
            // Compose the permutation with its original value without fixing
            // orientation. This is important and will be utilized later
            iter = avx2_swizzle_lo(iter, self.0) + compose_ori;

            // SIMD mask the iterated permutation with the identity and remove
            // already seen pieces to get the iteration's new pieces
            let perm_identity_eq = (iter & PERM_MASK_1).simd_eq(IDENTITY);
            let new_pieces = perm_identity_eq & !seen_perm;
            seen_perm |= perm_identity_eq;

            let new_pieces_bitmask = new_pieces.to_bitmask();

            // Variables prefixed with `reps_` are that value times the number
            // of repetitions. This variable for example is the number of corner
            // cycles times the number of repetitions. Recall from earlier that
            // the number of corner cycles is the number of bits set during the
            // iteration divided by the number of repetitions
            let reps_corner_cycle_count = corner_bits(new_pieces_bitmask).count_ones();
            // If there are any corner cycles, we need to check if they match
            // the specified sorted cycle structure. Note that this if statement
            // is compiled to more optimized assembly during codegen
            if reps_corner_cycle_count > 0 {
                // We now calculate how many of those corner cycles are
                // oriented. Recall a corner is oriented if O % 3 != 0, where O
                // is the orientation value. [This blog] demonstrates how the
                // expression is exactly equivalent to O * 171 > 85 for u8 sized
                // arithmetic. Rust was not optimizing this expression so I
                // opened this [issue].
                //
                // [This blog]: https://lomont.org/posts/2017/divisibility-testing
                // [issue]: https://github.com/rust-lang/portable-simd/issues/453
                let iter_co_mod =
                    // Move corner orientation bits to the LSB
                    (iter >> u8x32::splat(4))
                    * u8x32::splat(171);
                let oriented_corner_mask = new_pieces & iter_co_mod.simd_gt(u8x32::splat(85));
                // Simply counting the bits gives us the number of oriented
                // corner cycles times reps.
                let reps_oriented_corner_cycle_count =
                    corner_bits(oriented_corner_mask.to_bitmask()).count_ones();

                // The number of unoriented cycles is the number of corner
                // cycles minus the number of oriented cycles. If there are
                // unoriented corner cycles we need to advance the corner cycle
                // type pointer
                if reps_oriented_corner_cycle_count != reps_corner_cycle_count {
                    // and then divide it by the number of repetitions to get
                    // the number of unoriented cycles
                    corner_cycle_structure_pointer = corner_cycle_structure_pointer.wrapping_add(
                        ((reps_corner_cycle_count - reps_oriented_corner_cycle_count)
                            / u32::from(reps.get())) as usize,
                    );
                    // Same error checking as before
                    if corner_cycle_structure_pointer >= sorted_corners_cycle_structure.len()
                        || sorted_corners_cycle_structure[corner_cycle_structure_pointer]
                            != (reps, false)
                    {
                        return false;
                    }
                }

                // Perform the same logic for oriented corner cycles
                if reps_oriented_corner_cycle_count != 0 {
                    corner_cycle_structure_pointer = corner_cycle_structure_pointer.wrapping_add(
                        (reps_oriented_corner_cycle_count / u32::from(reps.get())) as usize,
                    );
                    if corner_cycle_structure_pointer >= sorted_corners_cycle_structure.len()
                        || sorted_corners_cycle_structure[corner_cycle_structure_pointer]
                            != (reps, true)
                    {
                        return false;
                    }
                }
            }

            // Repeat everything for edges
            let reps_edge_cycle_count = edge_bits(new_pieces_bitmask).count_ones();
            if reps_edge_cycle_count > 0 {
                // The only notable difference is that O % 2 != 0 is equivalent
                // to O & 1 != 0 so this becomes easier
                let iter_eo_mod = iter
                    & u8x32::splat(
                        1
                        // we avoid shifting the edge orientation bits by
                        // shifting the mask instead
                        << 4,
                    );
                let oriented_edge_mask = new_pieces & iter_eo_mod.simd_ne(u8x32::splat(0));
                let reps_oriented_edge_cycle_count =
                    edge_bits(oriented_edge_mask.to_bitmask()).count_ones();

                if reps_oriented_edge_cycle_count != reps_edge_cycle_count {
                    edge_cycle_structure_pointer = edge_cycle_structure_pointer.wrapping_add(
                        ((reps_edge_cycle_count - reps_oriented_edge_cycle_count)
                            / u32::from(reps.get())) as usize,
                    );
                    if edge_cycle_structure_pointer >= sorted_edges_cycle_structure.len()
                        || sorted_edges_cycle_structure[edge_cycle_structure_pointer]
                            != (reps, false)
                    {
                        return false;
                    }
                }

                if reps_oriented_edge_cycle_count != 0 {
                    edge_cycle_structure_pointer = edge_cycle_structure_pointer.wrapping_add(
                        (reps_oriented_edge_cycle_count / u32::from(reps.get())) as usize,
                    );
                    if edge_cycle_structure_pointer >= sorted_edges_cycle_structure.len()
                        || sorted_edges_cycle_structure[edge_cycle_structure_pointer]
                            != (reps, true)
                    {
                        return false;
                    }
                }
            }
            // SAFETY: this loop will only ever run 12 times at max because that
            // is the longest cycle length among the orbits
            reps = unsafe { NonZeroU8::new_unchecked(reps.get() + 1) };
        }

        // Finally, confirm that the cycle structure pointers have visited every
        // cycle structure in the sorted cycle structure list. If so, then
        // return true
        corner_cycle_structure_pointer == sorted_corners_cycle_structure.len().wrapping_sub(1)
            && edge_cycle_structure_pointer == sorted_edges_cycle_structure.len().wrapping_sub(1)
    }

    fn orbit_bytes(&self, orbit_type: Cube3OrbitType) -> (u8x16, u8x16) {
        let orbit = match orbit_type {
            Cube3OrbitType::Corners => self.0.extract::<CORNER_START, 16>(),
            Cube3OrbitType::Edges => self.0.extract::<EDGE_START, 16>(),
        };
        let perm = orbit & PERM_MASK_2;
        let ori = orbit >> 4;
        (perm, ori)
    }

    fn exact_hasher_orbit(&self, orbit_type: Cube3OrbitType) -> u64 {
        match orbit_type {
            Cube3OrbitType::Corners => {
                const PIECE_COUNT: u16 = CUBE_3_SORTED_ORBIT_DEFS[0].piece_count.get() as u16;
                const ORI_COUNT: u16 = CUBE_3_SORTED_ORBIT_DEFS[0].orientation_count.get() as u16;
                const LEN: usize = PIECE_COUNT.next_power_of_two() as usize;

                let corners = self.0.extract::<CORNER_START, LEN>();
                let perm = corners & PERM_MASK_3;
                let ori = corners >> 4;
                exact_hasher_orbit::<PIECE_COUNT, ORI_COUNT, LEN>(perm, ori)
            }
            Cube3OrbitType::Edges => {
                const PIECE_COUNT: u16 = CUBE_3_SORTED_ORBIT_DEFS[1].piece_count.get() as u16;
                const ORI_COUNT: u16 = CUBE_3_SORTED_ORBIT_DEFS[1].orientation_count.get() as u16;
                const LEN: usize = PIECE_COUNT.next_power_of_two() as usize;

                let edges = self.0.extract::<EDGE_START, LEN>();
                let perm = edges & PERM_MASK_2;
                let ori = edges >> 4;
                exact_hasher_orbit::<PIECE_COUNT, ORI_COUNT, LEN>(perm, ori)
            }
        }
    }

    fn approximate_hash_orbit(&self, orbit_type: Cube3OrbitType) -> impl Hash {
        match orbit_type {
            Cube3OrbitType::Corners => self.0.extract::<CORNER_START, 16>(),
            Cube3OrbitType::Edges => self.0.extract::<EDGE_START, 16>(),
        }
    }
}

impl Cube3 {
    /// An alternative to `replace_inverse` that uses a brute force approach to
    /// find the inverse of a cube state. Not really useful as it is slower.
    #[inline(always)]
    pub fn replace_inverse_brute(&mut self, a: &Self) {
        // Benchmarked on a 2x Intel Xeon E5-2667 v3: 7.31ns
        fn inner(dst: &mut Cube3, a: &Cube3) {
            let perm = a.0 & PERM_MASK_1;

            let mut inverse = BLANK;

            // Build the inverse permutation one at a time
            macro_rules! brute_unroll {
                ($i:literal) => {
                    let inv_trial = u8x32::splat($i);
                    let inv_correct = IDENTITY
                        .simd_eq(avx2_swizzle_lo(perm, inv_trial))
                        .to_int()
                        .cast();
                    inverse |= inv_trial & inv_correct;
                };
            }

            brute_unroll!(0);
            brute_unroll!(1);
            brute_unroll!(2);
            brute_unroll!(3);
            brute_unroll!(4);
            brute_unroll!(5);
            brute_unroll!(6);
            brute_unroll!(7);
            brute_unroll!(8);
            brute_unroll!(9);
            brute_unroll!(10);
            brute_unroll!(11);

            // Explained in `replace_compose`
            let mut added_ori = a.0 & ORI_MASK;
            added_ori += added_ori;
            added_ori = added_ori.simd_min(added_ori - ORI_CARRY_INVERSE);
            added_ori = avx2_swizzle_lo(added_ori, inverse);
            *dst = Cube3(inverse | added_ori);
        }
        #[cfg(avx2)]
        extern "vectorcall" fn replace_inverse_brute_vectorcall(dst: &mut Cube3, a: &Cube3) {
            inner(dst, a);
        }
        #[cfg(not(avx2))]
        fn replace_inverse_brute_vectorcall(dst: &mut Cube3, a: &Cube3) {
            inner(dst, a);
        }
        replace_inverse_brute_vectorcall(self, a);
    }

    /// An alternative to `replace_inverse` that uses a raw permutation
    /// inversion algorithm. Not really useful as it is slower.
    #[inline(always)]
    pub fn replace_inverse_raw(&mut self, a: &Self) {
        // Benchmarked on a 2x Intel Xeon E5-2667 v3: 13.2ns
        fn inner(dst: &mut Cube3, a: &Cube3) {
            let mut perm = BLANK;
            // LLVM unrolls this loop
            for i in 0..12 {
                // SAFETY: the permutation vector is guaranteed to be valid
                // indicies for swizzling
                unsafe {
                    *perm
                        .as_mut_array()
                        .get_unchecked_mut(a.0[i as usize] as usize) = i;
                    if i < 8 {
                        *perm
                            .as_mut_array()
                            .get_unchecked_mut(a.0[i as usize + 16] as usize + 16) = i;
                    }
                }
            }
            let mut added_ori = a.0 & ORI_MASK;
            added_ori += added_ori;
            added_ori = added_ori.simd_min(added_ori - ORI_CARRY_INVERSE);
            added_ori = avx2_swizzle_lo(added_ori, perm);
            *dst = Cube3(perm | added_ori);
        }
        #[cfg(avx2)]
        extern "vectorcall" fn replace_inverse_raw_vectorcall(dst: &mut Cube3, a: &Cube3) {
            inner(dst, a);
        }
        #[cfg(not(avx2))]
        fn replace_inverse_raw_vectorcall(dst: &mut Cube3, a: &Cube3) {
            inner(dst, a);
        }
        replace_inverse_raw_vectorcall(self, a);
    }
}

#[cfg(test)]
mod tests {
    extern crate test;
    use super::*;
    use crate::puzzle::{PuzzleDef, apply_moves};
    use generativity::make_guard;
    use puzzle_theory::puzzle_geometry::parsing::puzzle;

    #[test]
    #[cfg_attr(not(avx2), ignore = "AVX2 not enabled")]
    fn test_brute_force_inversion() {
        make_guard!(guard);
        let cube3_def = PuzzleDef::<Cube3>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let solved = cube3_def.new_solved_state();
        let mut result = solved.clone();

        let state_r2_b_prime = apply_moves(&cube3_def, &solved, "R2 B'", 1);
        result.replace_inverse_brute(&state_r2_b_prime);

        let state_b_r2 = apply_moves(&cube3_def, &solved, "B R2", 1);
        assert_eq!(result, state_b_r2);

        let in_r_f_cycle = apply_moves(&cube3_def, &solved, "R F", 40);
        result.replace_inverse_brute(&in_r_f_cycle);

        let remaining_r_f_cycle = apply_moves(&cube3_def, &solved, "R F", 65);
        assert_eq!(result, remaining_r_f_cycle);

        for i in 1..=5 {
            let state = apply_moves(&cube3_def, &solved, "L F L' F'", i);
            result.replace_inverse_brute(&state);
            let remaining_state = apply_moves(&cube3_def, &solved, "L F L' F'", 6 - i);
            assert_eq!(result, remaining_state);
        }

        for _ in 0..100 {
            let mut result_1 = solved.clone();
            let mut result_2 = solved.clone();
            for _ in 0..20 {
                let move_index = fastrand::choice(0_u8..18).unwrap();
                let move_ = &cube3_def.moves[move_index as usize];
                result_1.replace_compose(&result_2, &move_.puzzle_state);
                std::mem::swap(&mut result_2, &mut result_1);
            }
            result_1.replace_inverse_brute(&result_2);
            result_2.replace_compose(&result_1, &result_2.clone());
            assert_eq!(result_2, solved);
        }
    }

    #[test]
    #[cfg_attr(not(avx2), ignore = "AVX2 not enabled")]
    fn test_raw_inversion() {
        make_guard!(guard);
        let cube3_def = PuzzleDef::<Cube3>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let solved = cube3_def.new_solved_state();
        let mut result = solved.clone();

        let state_r2_b_prime = apply_moves(&cube3_def, &solved, "R2 B'", 1);
        result.replace_inverse_raw(&state_r2_b_prime);

        let state_b_r2 = apply_moves(&cube3_def, &solved, "B R2", 1);
        assert_eq!(result, state_b_r2);

        let in_r_f_cycle = apply_moves(&cube3_def, &solved, "R F", 40);
        result.replace_inverse_raw(&in_r_f_cycle);

        let remaining_r_f_cycle = apply_moves(&cube3_def, &solved, "R F", 65);
        assert_eq!(result, remaining_r_f_cycle);

        for i in 1..=5 {
            let state = apply_moves(&cube3_def, &solved, "L F L' F'", i);
            result.replace_inverse_raw(&state);
            let remaining_state = apply_moves(&cube3_def, &solved, "L F L' F'", 6 - i);
            assert_eq!(result, remaining_state);
        }

        for _ in 0..100 {
            let mut result_1 = solved.clone();
            let mut result_2 = solved.clone();
            for _ in 0..20 {
                let move_index = fastrand::choice(0_u8..18).unwrap();
                let move_ = &cube3_def.moves[move_index as usize];
                result_1.replace_compose(&result_2, &move_.puzzle_state);
                std::mem::swap(&mut result_2, &mut result_1);
            }
            result_1.replace_inverse_raw(&result_2);
            result_2.replace_compose(&result_1, &result_2.clone());
            assert_eq!(result_2, solved);
        }
    }

    #[bench]
    #[cfg_attr(not(avx2), ignore = "AVX2 not enabled")]
    fn bench_brute_force_inversion(b: &mut test::Bencher) {
        make_guard!(guard);
        let cube3_def = PuzzleDef::<Cube3>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let solved = cube3_def.new_solved_state();
        let mut result = solved.clone();
        let order_1260 = apply_moves(&cube3_def, &solved, "R U2 D' B D'", 100);
        b.iter(|| {
            test::black_box(&mut result).replace_inverse_brute(test::black_box(&order_1260));
        });
    }

    #[bench]
    #[cfg_attr(not(avx2), ignore = "AVX2 not enabled")]
    fn bench_raw_inversion(b: &mut test::Bencher) {
        make_guard!(guard);
        let cube3_def = PuzzleDef::<Cube3>::new(&puzzle("3x3").ksolve(), guard).unwrap();
        let solved = cube3_def.new_solved_state();
        let mut result = solved.clone();
        let order_1260 = apply_moves(&cube3_def, &solved, "R U2 D' B D'", 100);
        b.iter(|| {
            test::black_box(&mut result).replace_inverse_raw(test::black_box(&order_1260));
        });
    }
}
