#pragma once

#include <cstddef>
#include <cstdint>

extern "C" {
/// Use an alternative implementation of the Pandita algorithm to compute the
/// next lexicographic permutation of a buffer. From:
/// https://www.geeksforgeeks.org/lexicographic-permutations-of-string/
///
/// # Safety
///
/// `perm` must point to at least `len` bytes, `len` must be at least two, and
/// the buffer must contain unique elements.
void qter_pandita2(uint8_t *perm, size_t len);
}
