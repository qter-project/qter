use std::{
    cmp::Ordering,
    f32::consts::LN_2,
    simd::{
        cmp::{SimdOrd, SimdPartialEq, SimdPartialOrd},
        f32x32,
        num::{SimdFloat, SimdUint},
        u8x32,
    },
};

#[derive(Copy, Clone, Debug, PartialEq)]
struct PrimePowerNum(u8x32);

// note that this only works up to u8x32 since the 64th prime is more than what fits into a u8
const FIRST_32_PRIMES: u8x32 = u8x32::from_array([
    2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89, 97,
    101, 103, 107, 109, 113, 127, 131,
]);
const FIRST_32_PRIMES_LN: f32x32 = f32x32::from_array([
    LN_2, 1.0986123, 1.609438, 1.9459101, 2.3978953, 2.5649493, 2.8332133, 2.944439, 3.1354942,
    3.3672957, 3.4339871, 3.6109178, 3.713572, 3.7612002, 3.8501475, 3.9702919, 4.0775375,
    4.1108737, 4.204693, 4.26268, 4.2904596, 4.3694477, 4.4188404, 4.4886365, 4.574711, 4.6151204,
    4.634729, 4.6728287, 4.691348, 4.727388, 4.8441873, 4.8751974,
]);
// const MAX_EXPONENT: u8x32 = u8x32::from_array(const {
//     let mut ret = [0; 32];
//     let mut i = 0;
//     while i < 32 {
//         ret[i] = u8::MAX / FIRST_32_PRIMES.to_array()[i];
//         i += 1;
//     }
//     ret
// });

impl PrimePowerNum {
    pub fn lcm(&self, other: Self) -> Self {
        PrimePowerNum(self.0.simd_max(other.0))
    }
}

impl PartialOrd for PrimePowerNum {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let self_ln = self.0.cast::<f32>() * FIRST_32_PRIMES_LN;
        let other_ln = other.0.cast::<f32>() * FIRST_32_PRIMES_LN;
        self_ln.reduce_sum().partial_cmp(&other_ln.reduce_sum())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foo() {
        let a = [
            1.0986123_f32,
            1.609438,
            1.9459101,
            2.3978953,
            2.5649493,
            2.8332133,
            2.944439,
            3.1354942,
            3.3672957,
            3.4339871,
            3.6109178,
            3.713572,
            3.7612002,
            3.8501475,
            3.9702919,
            4.0775375,
            4.1108737,
            4.204693,
            4.26268,
            4.2904596,
            4.3694477,
            4.4188404,
            4.4886365,
            4.574711,
            4.6151204,
            4.634729,
            4.6728287,
            4.691348,
            4.727388,
            4.8441873,
            4.8751974,
        ];
        println!(
            "{:?}",
            a.iter().map(|&i| (i * 52.0).round()).collect::<Vec<_>>()
        );
    }
}
