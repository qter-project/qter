//! SIMD optimized implementations for N-cube corners

#[cfg(avx2)]
pub use avx2::CubeNCorners;
#[cfg(not(any(avx2, simd8)))]
pub use fallback::CubeNCorners;
#[cfg(all(not(avx2), simd8))]
pub use simd8::CubeNCorners;

pub(in crate::orbit_puzzle) mod avx2;
pub(in crate::orbit_puzzle) mod fallback;
pub(in crate::orbit_puzzle) mod simd8;
