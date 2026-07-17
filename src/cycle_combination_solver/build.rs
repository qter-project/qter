use cfg_aliases::cfg_aliases;

fn main() {
    cfg_aliases! {
        simd8: {
            all(
                any(
                    target_arch = "aarch64",
                    target_arch = "arm64ec",
                    all(
                        target_arch = "arm",
                        target_feature = "v7"
                    )
                ),
                target_feature = "neon",
                target_endian = "little"
            )
        },
        simd16: {
            any(
                target_feature = "ssse3",
                target_feature = "simd128",
                all(
                    any(
                        target_arch = "aarch64",
                        target_arch = "arm64ec"
                    ),
                    target_feature = "neon",
                    target_endian = "little"
                ),
                all(
                    target_arch = "arm",
                    target_feature = "v7",
                    target_feature = "neon",
                    target_endian = "little"
                )
            )
        },
        simd8and16: {
            all(
                simd8,
                simd16
            )
        },
        // simd8and16: { not(l) }, // true
        // simd8and16: { l }, // false
        avx2: {
            target_feature = "avx2"
        },
        // avx2: { not(l) }, // true
        // avx2: { l }, // false
    }

    println!("cargo:rerun-if-changed=cpp/permutator.cpp");
    println!("cargo:rerun-if-changed=cpp/permutator.h");
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .file("cpp/permutator.cpp")
        .compile("qter_permutator");
}
