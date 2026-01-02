use std::{
    borrow::Cow,
    collections::BTreeMap,
    fmt::Debug,
    sync::{Arc, OnceLock},
};

use internment::ArcIntern;
use itertools::Itertools;
use puzzle_theory::{
    numbers::{I, Int, U, chinese_remainder_theorem, lcm, lcm_iter},
    permutations::{Algorithm, Permutation, PermutationGroup},
    puzzle_geometry::PuzzleGeometry,
    span::WithSpan,
};

use crate::{Facelets, shared_facelet_detection::algorithms_to_cycle_generators, table_encoding};

pub(crate) const OPTIMIZED_TABLES: [&[u8]; 5] = [
    include_bytes!("../puzzles/210-24.bin"),
    include_bytes!("../puzzles/30-30-30.bin"),
    include_bytes!("../puzzles/30-18-10-9.bin"),
    include_bytes!("../puzzles/90-90.bin"),
    include_bytes!("../puzzles/4-4.bin"),
];

/// The definition of a puzzle parsed from the custom format
#[derive(Debug)]
pub struct PuzzleDefinition {
    /// The permutation group of the puzzle
    pub perm_group: Arc<PermutationGroup>,
    /// A list of preset architectures
    pub presets: Vec<Arc<Architecture>>,
}

impl PuzzleDefinition {
    // If they want the cycles in a different order, create a new architecture with the cycles shuffled
    fn adapt_architecture(
        architecture: &Arc<Architecture>,
        orders: &[Int<U>],
    ) -> Option<Arc<Architecture>> {
        let mut used = vec![false; orders.len()];
        let mut swizzle = vec![0; orders.len()];

        for (i, order) in orders.iter().enumerate() {
            let mut found_one = false;

            for (j, cycle) in architecture.registers.iter().enumerate() {
                if !used[j] && cycle.order() == *order {
                    used[j] = true;
                    found_one = true;
                    swizzle[i] = j;
                    break;
                }
            }

            if !found_one {
                return None;
            }
        }

        if swizzle.iter().enumerate().all(|(v, i)| v == *i) {
            return Some(Arc::clone(architecture));
        }

        let mut new_arch = Architecture::clone(architecture);

        new_arch.decoded_table = OnceLock::new();

        for i in 0..swizzle.len() {
            new_arch.registers.swap(i, swizzle[i]);

            for j in i..swizzle.len() {
                if i == swizzle[j] {
                    swizzle[j] = swizzle[i];
                    break;
                }
            }
        }

        Some(Arc::new(new_arch))
    }

    /// Find a preset with the specified cycle orders
    #[must_use]
    pub fn get_preset(&self, orders: &[Int<U>]) -> Option<Arc<Architecture>> {
        for preset in &self.presets {
            if preset.registers.len() != orders.len() {
                continue;
            }

            if let Some(arch) = Self::adapt_architecture(preset, orders) {
                return Some(arch);
            }
        }

        None
    }
}

/// A cycle of facelets that is part of the generator of a register
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct CycleGeneratorSubcycle {
    pub(crate) facelet_cycle: Vec<usize>,
    pub(crate) chromatic_order: Int<U>,
}

impl CycleGeneratorSubcycle {
    /// Get the cycle of facelets
    #[must_use]
    pub fn facelet_cycle(&self) -> &[usize] {
        &self.facelet_cycle
    }

    /// Get the order of the cycle after accounting for colors
    #[must_use]
    pub fn chromatic_order(&self) -> Int<U> {
        self.chromatic_order
    }
}

/// Create an `Algorithm` from what values it should add to which registers.
///
/// `effect` is a list of tuples of register indices and how much to add to add to them.
#[allow(clippy::missing_panics_doc)]
pub fn new_from_effect(arch: &Architecture, effect: Vec<(usize, Int<U>)>) -> Algorithm {
    let mut move_seq = Vec::new();

    let mut expanded_effect = vec![Int::<U>::zero(); arch.registers().len()];

    for (register, amt) in effect {
        expanded_effect[register] = amt % arch.registers()[register].order();
    }

    let table = arch.decoding_table();
    let orders = table.orders();

    while expanded_effect.iter().any(|v| !v.is_zero()) {
        let (true_effect, alg) = table.closest_alg(&expanded_effect);

        expanded_effect
            .iter_mut()
            .zip(true_effect.iter().copied())
            .zip(orders.iter().copied())
            .for_each(|((expanded_effect, true_effect), order)| {
                *expanded_effect = if *expanded_effect < true_effect {
                    *expanded_effect + order - true_effect
                } else {
                    *expanded_effect - true_effect
                }
            });

        move_seq.extend_from_slice(alg);
    }

    Algorithm::new_from_move_seq(arch.group_arc(), move_seq).unwrap()
}

/// Calculate the order of every cycle of facelets created by seeing this `Algorithm` instance as a register generator.
///
/// Returns a list of chromatic orders where the index is the facelet.
pub fn chromatic_orders_by_facelets(alg: &Algorithm) -> Vec<Int<U>> {
    let mut out = vec![Int::one(); alg.group().facelet_count()];

    alg.permutation().cycles().iter().for_each(|cycle| {
        let chromatic_order = length_of_substring_that_this_string_is_n_repeated_copies_of(
            cycle.iter().map(|&idx| &*alg.group().facelet_colors()[idx]),
        );

        for &facelet in cycle {
            out[facelet] = Int::from(chromatic_order);
        }
    });

    out
}

/// A generator for a register in an architecture
#[derive(Debug, Clone)]
pub struct CycleGenerator {
    algorithm: Algorithm,
    unshared_cycles: Vec<CycleGeneratorSubcycle>,
    order: Int<U>,
}

impl CycleGenerator {
    pub(crate) fn new(
        algorithm: Algorithm,
        unshared_cycles: Vec<CycleGeneratorSubcycle>,
    ) -> CycleGenerator {
        CycleGenerator {
            algorithm,
            order: unshared_cycles.iter().fold(Int::one(), |acc, subcycle| {
                lcm(acc, subcycle.chromatic_order)
            }),
            unshared_cycles,
        }
    }

    pub fn algorithm(&self) -> &Algorithm {
        &self.algorithm
    }

    /// Get the cycles of the permutation that are unshared by other cycles in the architecture
    pub fn unshared_cycles(&self) -> &[CycleGeneratorSubcycle] {
        &self.unshared_cycles
    }

    /// Get the order of the register
    pub fn order(&self) -> Int<U> {
        self.order
    }

    /// Find a collection of facelets that allow decoding the register and that allow determining whether the register is solved
    #[allow(clippy::missing_panics_doc)]
    pub fn signature_facelets(&self) -> Facelets {
        // This will never fail when `remainder_mod` is the order.
        self.signature_facelets_mod(self.order()).unwrap()
    }

    /// Find a collection of facelets that allow decoding the register modulo a particular number.
    ///
    /// With some registers, you can decode cycles individually and pick out information about the register modulo some number. This will attempt to do so for a given remainder to target. It will return `None` if it's impossible to decode the given modulus from the register.
    #[allow(clippy::missing_panics_doc)]
    pub fn signature_facelets_mod(&self, remainder_mod: Int<U>) -> Option<Facelets> {
        let mut cycles_with_extras = vec![];

        // Create a list of all cycles
        for (i, cycle) in self.unshared_cycles().iter().enumerate() {
            if cycle.chromatic_order() != Int::<U>::one()
                && (remainder_mod % cycle.chromatic_order()).is_zero()
            {
                cycles_with_extras.push((cycle.chromatic_order(), i));
            }
        }

        if lcm_iter(cycles_with_extras.iter().map(|v| v.0)) != remainder_mod {
            // We couldn't pick out the modulus from the register
            return None;
        }

        // Remove all of the cycles that don't contribute to the order of the register, removing the smallest ones first
        cycles_with_extras.sort_unstable_by(|a, b| a.0.cmp(&b.0));

        let mut cycles = Vec::<(Int<U>, usize)>::new();

        for (i, &(cycle_order, cycle_idx)) in cycles_with_extras.iter().enumerate() {
            let lcm_without = lcm_iter(
                cycles
                    .iter()
                    .map(|&(chromatic_order, _)| chromatic_order)
                    .chain((i + 1..cycles_with_extras.len()).map(|idx| cycles_with_extras[idx].0)),
            );

            if remainder_mod != lcm_without {
                cycles.push((cycle_order, cycle_idx));
            }
        }

        // Note that since pieces all move together, including all stickers on a piece doesn't change the order of the cycle or include anything that shouldn't be included.

        let mut facelets = Vec::new();
        let mut pieces = Vec::new();

        // Track uncovered facelets and the cycles that they belong to
        let mut facelet_cycle_membership = BTreeMap::new();
        for (_, idx) in cycles {
            let cycle = &self.unshared_cycles()[idx];
            for facelet in cycle.facelet_cycle() {
                facelet_cycle_membership.insert(facelet, idx);
            }
        }

        let group = self.algorithm.group();

        while let Some((sticker, _)) = facelet_cycle_membership.first_key_value() {
            let piece = &group.piece_assignments()[**sticker];
            pieces.push(ArcIntern::clone(piece));

            // Include all other stickers on the same piece
            let rest_to_include = group
                .piece_assignments()
                .iter()
                .enumerate()
                .filter(|(_, v)| *v == piece)
                .filter_map(|(i, _)| facelet_cycle_membership.remove(&i).map(|v| (i, v)))
                .collect_vec();

            for (sticker, cycle) in rest_to_include {
                facelets.push(sticker);

                let color = &group.facelet_colors()[sticker];

                for cycle_member in self.unshared_cycles()[cycle].facelet_cycle() {
                    if &group.facelet_colors()[*cycle_member] != color {
                        facelet_cycle_membership.remove(cycle_member);
                    }
                }
            }
        }

        Some(Facelets::new(facelets, pieces, remainder_mod))
    }
}

#[derive(Debug, Clone)]
pub struct DecodingTable {
    orders: Vec<Int<U>>,
    table: BTreeMap<Vec<Int<U>>, Vec<ArcIntern<str>>>,
}

impl DecodingTable {
    /// Find the algorithm that creates the requested cycle combination as closely as possible, as a sum of all offsets left over.
    #[must_use]
    pub fn closest_alg<'s, 't>(
        &'s self,
        target: &'t [Int<U>],
    ) -> (&'s [Int<U>], &'s [ArcIntern<str>]) {
        let mut closest: Option<(Int<U>, &'s [Int<U>], &'s [ArcIntern<str>])> = None;

        let mut update_closest = |achieves: &'s [Int<U>], alg: &'s [ArcIntern<str>]| {
            let dist = achieves
                .iter()
                .copied()
                .zip(target.iter().copied())
                .zip(self.orders.iter().copied())
                .map(|((achieves, target), order)| {
                    let dist = achieves.abs_diff(&target);

                    if dist > order / Int::<U>::from(2_u32) {
                        order - dist
                    } else {
                        dist
                    }
                })
                .sum::<Int<U>>();

            let mut min_dist = dist;

            if match closest {
                Some((old_dist, _, _)) => {
                    min_dist = old_dist;
                    old_dist > dist
                }
                None => true,
            } {
                closest = Some((dist, achieves, alg));
            }

            min_dist
        };

        // Iterate radially away from the closest value lexicographically, hopefully the true closest is nearby

        let mut end_range = self.table.range(target.to_vec()..).chain(self.table.iter());
        let mut take_end = true;
        let mut start_range = self
            .table
            .range(..=target.to_vec())
            .rev()
            .chain(self.table.iter().rev());
        let mut take_start = true;

        let mut amt_taken = 0;

        while (take_end || take_start) && amt_taken < self.table.len() {
            if take_start {
                // Wrapping around should be impossible
                let (achieves, alg) = start_range.next().unwrap();

                amt_taken += 1;

                let min_dist = update_closest(achieves, alg);

                // Taking from here can no longer generate closer values
                if min_dist < target[0].abs_diff(&achieves[0]) {
                    take_start = false;
                }
            }

            if take_end {
                let (achieves, alg) = end_range.next().unwrap();

                amt_taken += 1;

                let min_dist = update_closest(achieves, alg);

                // Taking from here can no longer generate closer values
                if min_dist < achieves[0].abs_diff(&target[0]) {
                    take_end = false;
                }
            }
        }

        let (_, remaining_offset, alg) = closest.unwrap();

        (remaining_offset, alg)
    }

    pub(crate) fn orders(&self) -> &[Int<U>] {
        &self.orders
    }
}

/// An architecture of a `PermutationGroup`
#[derive(Debug, Clone)]
pub struct Architecture {
    perm_group: Arc<PermutationGroup>,
    registers: Vec<CycleGenerator>,
    shared_facelets: Vec<usize>,
    optimized_table: Option<Cow<'static, [u8]>>,
    decoded_table: OnceLock<DecodingTable>,
}

impl Architecture {
    /// Create a new architecture from a permutation group and a list of algorithms.
    ///
    /// # Errors
    ///
    /// If the algorithms are invalid, it will return an error
    pub fn new<T: AsRef<str>>(
        perm_group: Arc<PermutationGroup>,
        algorithms: &[Vec<T>],
    ) -> Result<Architecture, &T> {
        let (registers, shared_facelets) = algorithms_to_cycle_generators(&perm_group, algorithms)?;

        Ok(Architecture {
            perm_group,
            registers,
            shared_facelets,
            optimized_table: None,
            decoded_table: OnceLock::new(),
        })
    }

    /// Insert a table of optimized algorithms into the architecture. The algorithms are expected to be compressed using `table_encoding::encode`. Inverses and the values that registers that define the architecture need not be optimized, they will be included automatically. You may optimize them anyways and values encoded later in the table will be prioritized.
    ///
    /// `self.get_table()` will panic if the table is encoded incorrectly and it will ignore invalid entries.
    pub fn set_optimized_table(&mut self, optimized_table: Cow<'static, [u8]>) {
        self.optimized_table = Some(optimized_table);
    }

    /// Retrieve a table of optimized algorithms by how they affect each cycle type.
    pub fn decoding_table(&self) -> &DecodingTable {
        self.decoded_table.get_or_init(|| {
            let table = match &self.optimized_table {
                Some(encoded) => {
                    table_encoding::decode_table(&mut encoded.iter().copied()).unwrap()
                }
                None => Vec::new(),
            };

            let registers_decoding_info = self
                .registers()
                .iter()
                .map(|register| (register.signature_facelets(), &register.algorithm))
                .collect_vec();

            let mut data = BTreeMap::new();

            let mut add_permutation = |alg: Vec<ArcIntern<str>>| {
                let permutation =
                    Algorithm::new_from_move_seq(self.group_arc(), alg.clone()).unwrap();

                let maybe_decoded = registers_decoding_info
                    .iter()
                    .map(|(facelets, generators)| {
                        decode(permutation.permutation(), facelets.facelets(), generators)
                    })
                    .collect::<Option<Vec<_>>>();

                if let Some(decoded) = maybe_decoded {
                    data.insert(decoded, alg);
                }
            };

            for item in self.registers().iter().flat_map(|register| {
                let mut inverse = register.algorithm.clone();
                inverse.exponentiate(-Int::<I>::one());
                [
                    register.algorithm.move_seq_iter().cloned().collect_vec(),
                    inverse.move_seq_iter().cloned().collect_vec(),
                ]
            }) {
                add_permutation(item);
            }

            for item in table.iter().map(|inverse| {
                let mut inverse = inverse.to_owned();
                self.perm_group.invert_generator_moves(&mut inverse);
                inverse
            }) {
                add_permutation(item);
            }

            for item in table {
                add_permutation(item);
            }

            DecodingTable {
                table: data,
                orders: self.registers().iter().map(CycleGenerator::order).collect(),
            }
        })
    }

    /// Get the underlying permutation group
    pub fn group(&self) -> &PermutationGroup {
        &self.perm_group
    }

    /// Get the underlying permutation group as an owned Rc
    pub fn group_arc(&self) -> Arc<PermutationGroup> {
        Arc::clone(&self.perm_group)
    }

    /// Get all of the registers of the architecture
    pub fn registers(&self) -> &[CycleGenerator] {
        &self.registers
    }

    /// Get all of the facelets that are shared in the architecture
    pub fn shared_facelets(&self) -> &[usize] {
        &self.shared_facelets
    }
}

/// Get any presets associated with the given `PuzzleGeometry`
///
/// # Panics
///
/// The span attached to `geometry` must be the true puzzle definition. This is trivially satisfiable by acquiring the `PuzzleGeometry` from either either the `puzzle_geometry` or `puzzle` functions.
pub fn with_presets(geometry: WithSpan<Arc<PuzzleGeometry>>) -> WithSpan<PuzzleDefinition> {
    let group = geometry.permutation_group();

    geometry
        .span()
        .clone()
        .with(if geometry.span().slice() == "3x3" {
            let presets: [Arc<Architecture>; 6] = [
                (&["R U2 D' B D'"] as &[&str], None),
                (&["U", "D"], Some(4)),
                (
                    &["R' F' L U' L U L F U' R", "U F R' D' R2 F R' U' D"],
                    Some(3),
                ),
                (&["U R U' D2 B", "B U2 B' L' U2 B U L' B L B2 L"], Some(0)),
                (
                    &[
                        "U L2 B' L U' B' U2 R B' R' B L",
                        "R2 L U' R' L2 F' D R' D L B2 D2",
                        "L2 F2 U L' F D' F' U' L' F U D L' U'",
                    ],
                    Some(1),
                ),
                (
                    &[
                        "U L B' L B' U R' D U2 L2 F2",
                        "D L' F L2 B L' F' L B' D' L'",
                        "R' U' L' F2 L F U F R L U'",
                        "B2 U2 L F' R B L2 D2 B R' F L",
                    ],
                    Some(2),
                ),
            ]
            .map(|(algs, maybe_index): (&[&str], Option<usize>)| {
                let mut arch = Architecture::new(
                    Arc::clone(&group),
                    &algs
                        .iter()
                        .map(|alg| alg.split(' ').map(ArcIntern::from).collect_vec())
                        .collect_vec(),
                )
                .unwrap();

                if let Some(index) = maybe_index {
                    arch.set_optimized_table(Cow::Borrowed(OPTIMIZED_TABLES[index]));
                }

                Arc::new(arch)
            });

            PuzzleDefinition {
                perm_group: group,
                presets: presets.into(),
            }
        } else {
            PuzzleDefinition {
                perm_group: group,
                presets: Vec::new(),
            }
        })
}

/// This function does what it says on the tin.
///
/// "AAAA"  → 1
/// "ABAB"  → 2
/// "ABCA"  → 4
/// "ABABA" → 5
///
/// Every string given by the iterator is treated as a unit rather than split apart, so `["Yellow", "Green", "Yellow", "Green"]` would return `2`.
///
/// This function is important for computing the chromatic order of cycles.
pub fn length_of_substring_that_this_string_is_n_repeated_copies_of<'a>(
    colors: impl Iterator<Item = &'a str>,
) -> usize {
    let mut found = vec![];
    let mut current_repeat_length = 1;

    for (i, color) in colors.enumerate() {
        found.push(color);

        if found[i % current_repeat_length] != color {
            current_repeat_length = i + 1;
        }
    }

    // We didn't match the substring a whole number of times; it actually doesn't work
    if found.len() % current_repeat_length != 0 {
        current_repeat_length = found.len();
    }

    current_repeat_length
}

/// Decode the permutation using the register generator and the given facelets.
///
/// In general, an arbitrary scramble cannot be decoded. If this is the case, the function will return `None`.
pub fn decode(
    permutation: &Permutation,
    facelets: &[usize],
    generator: &Algorithm,
) -> Option<Int<U>> {
    chinese_remainder_theorem(facelets.iter().map(|&facelet| {
        let maps_to = permutation.mapping().get(facelet);

        let chromatic_order = chromatic_orders_by_facelets(generator)[facelet];

        if maps_to == facelet {
            return Some((Int::zero(), chromatic_order));
        }

        let mut i = Int::<U>::one();
        let mut maps_to_found_at = None;
        let mut facelet_at = generator.permutation().mapping().get(facelet);

        while facelet_at != facelet {
            if facelet_at == maps_to {
                maps_to_found_at = Some(i);
                break;
            }

            facelet_at = generator.permutation().mapping().get(facelet_at);
            i += Int::<U>::one();
        }

        maps_to_found_at.map(|found_at| (found_at % chromatic_order, chromatic_order))
    }))
}

#[cfg(test)]
mod tests {

    use std::sync::Arc;

    use internment::ArcIntern;
    use itertools::Itertools;
    use puzzle_theory::{
        numbers::{Int, U},
        permutations::{Algorithm, Permutation},
        puzzle_geometry::parsing::puzzle,
    };

    use crate::architectures::{
        decode, length_of_substring_that_this_string_is_n_repeated_copies_of, with_presets,
    };

    use super::Architecture;

    #[test]
    fn three_by_three() {
        let cube_def = with_presets(puzzle("3x3"));

        for (arch, expected) in &[
            (&["U", "D"][..], &[4, 4][..]),
            (
                &["R' F' L U' L U L F U' R", "U F R' D' R2 F R' U' D"],
                &[90_u64, 90],
            ),
            (
                &["U R U' D2 B", "B U2 B' L' U2 B U L' B L B2 L"],
                &[210, 24],
            ),
            (
                &[
                    "U L2 B' L U' B' U2 R B' R' B L",
                    "R2 L U' R' L2 F' D R' D L B2 D2",
                    "L2 F2 U L' F D' F' U' L' F U D L' U'",
                ],
                &[30, 30, 30],
            ),
        ] {
            let arch = Architecture::new(
                Arc::clone(&cube_def.perm_group),
                &arch
                    .iter()
                    .map(|alg| alg.split(' ').map(ArcIntern::from).collect_vec())
                    .collect_vec(),
            )
            .unwrap();

            for (register, expected) in arch.registers.iter().zip(expected.iter()) {
                assert_eq!(register.order(), Int::<U>::from(*expected));
            }
        }
    }

    #[test]
    fn length_of_substring_whatever() {
        assert_eq!(
            length_of_substring_that_this_string_is_n_repeated_copies_of(
                ["a", "a", "a", "a"].into_iter()
            ),
            1
        );

        assert_eq!(
            length_of_substring_that_this_string_is_n_repeated_copies_of(
                ["a", "b", "a", "b"].into_iter()
            ),
            2
        );

        assert_eq!(
            length_of_substring_that_this_string_is_n_repeated_copies_of(
                ["a", "b", "a", "b", "a"].into_iter()
            ),
            5
        );

        assert_eq!(
            length_of_substring_that_this_string_is_n_repeated_copies_of(
                ["a", "b", "c", "d", "e"].into_iter()
            ),
            5
        );
    }

    #[test]
    fn test_decode() {
        let cube_def = puzzle("3x3").permutation_group();

        let mut cube = Permutation::identity();

        let permutation =
            Algorithm::new_from_move_seq(Arc::clone(&cube_def), vec![ArcIntern::from("U")])
                .unwrap();

        assert_eq!(decode(&cube, &[8], &permutation).unwrap(), Int::<U>::zero());

        cube.compose_into(permutation.permutation());
        assert_eq!(decode(&cube, &[8], &permutation).unwrap(), Int::<U>::one());

        cube.compose_into(permutation.permutation());
        assert_eq!(decode(&cube, &[8], &permutation).unwrap(), Int::from(2));

        cube.compose_into(permutation.permutation());
        assert_eq!(decode(&cube, &[8], &permutation).unwrap(), Int::from(3));

        cube.compose_into(permutation.permutation());
        assert_eq!(decode(&cube, &[8], &permutation).unwrap(), Int::from(0));
    }
}
