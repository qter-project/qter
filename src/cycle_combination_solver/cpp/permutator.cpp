#include "permutator.h"

#include <utility>

namespace {

/// Reverse the elements in `perm` from index `s` to index `e`, inclusive.
///
/// # Safety
///
/// `s` and `e` must be valid indices for `perm`, and `s` must be less than or
/// equal to `e`.
void reverse_unchecked(uint8_t *perm, size_t s, size_t e) {
    while (s < e) {
        std::swap(perm[s], perm[e]);
        s += 1;
        e -= 1;
    }
}

} // namespace

void qter_pandita2(uint8_t *perm, size_t len) {
    // Benchmarked on a 2025 Mac M4: 170.49ns (test_big) 2.84ns (test_small)

    size_t i = len - 2;
    while (perm[i] >= perm[i + 1]) {
        if (i == 0) {
            return;
        }
        i -= 1;
    }
    size_t j = len - 1;
    while (perm[j] <= perm[i]) {
        j -= 1;
    }
    std::swap(perm[i], perm[j]);
    reverse_unchecked(perm, i + 1, len - 1);
}
