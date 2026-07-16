//! Assign the cycles that realize each register of a cycle combination to
//! concrete orbits of the puzzle, or prove that no such assignment exists.
//!
//! This is the "MKP" (multidimensional knapsack problem) step of the CCF: the
//! search tree only prunes candidates by the *total* minimum piece count, so a
//! candidate may pass even though its cycles cannot actually be packed into
//! the individual orbits. [`CycleCombinationDetails::new`] performs the exact
//! per-orbit packing and doubles as the proof of feasibility that downstream
//! consumers (the CCS) need.
//!
//! For every prime power `p^e` of a register's order, the fundamental theorem
//! is that it must be realized by a *single* atom, because element orders
//! combine by lcm, not by product: a cycle of length `p^(e - k)` whose
//! accumulated orientation has order `p^k`, for some `0 <= k <=
//! min(e, v_p(orientation count))`. When `k == e` the "cycle" is length one
//! and the prime power rides purely on orientation, either on its own piece
//! or hosted by another cycle in the same orbit. Merging two coprime prime
//! powers into one cycle is never cheaper (`p * q > p + q`), so single-prime
//! cycles are complete.
//!
//! On top of the per-prime assignment, two puzzle constraints are handled:
//!
//! - Orientation sum ([`OrientationSumConstraint::Zero`]): the orientations
//!   chosen for the atoms of an orbit must sum to zero. A small DP over (sum,
//!   delivered prime powers) decides whether the twists can cancel for free, be
//!   absorbed by other cycles of the register in the orbit, or require up to
//!   two extra in-place oriented pieces.
//! - Permutation parity: every register is a standalone puzzle group element,
//!   so for each even parity constraint the parities of its orbits must sum to
//!   zero. Violations are fixed by "junk" 2-cycles (which are only legal when 2
//!   divides the register's order, and always do divide it when a violation can
//!   occur), solved over GF(2) against the reduced constraint matrix.
//!
//! TODO: registers are packed fully disjointly. Sharing junk 2-cycles (and
//! parity pieces in general) between registers can save pieces and admit
//! strictly more cycle combinations.

use std::rc::Rc;

use crate::{
    FIRST_129_PRIMES,
    cycle_combinations_tree::DisjointRegisters,
    finder::PossibleOrder,
    orderexps::OrderExps,
    puzzle::{OrientationStatus, OrientationSumConstraint, PuzzleDef},
};

/// One cycle of pieces within an orbit, as part of realizing a register. A
/// `length` of one denotes a piece that is not permuted, only oriented in
/// place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CyclePartition {
    pub length: u16,
    /// The order of the orientation accumulated along the cycle. One means
    /// the cycle does not orient. The order of the register restricted to
    /// this cycle is `length * orientation_order`.
    pub orientation_order: u16,
}

/// The cycles realizing a single register. `partitions[orbit_index]` lists
/// the cycles the register occupies in that orbit, including parity-fixing
/// junk 2-cycles and orientation compensation pieces.
#[derive(Debug, Clone)]
pub struct Cycle {
    partitions: Vec<Vec<CyclePartition>>,
}

#[derive(Debug)]
pub struct CycleCombinationDetails {
    cycles: Vec<Cycle>,
}

/// Caches the Pareto-minimal per-register decompositions, which depend
/// only on the possible order, across `CycleCombinationDetails::new`
/// calls. One per details thread.
#[derive(Debug)]
pub struct RegisterOptionsCache {
    options: Vec<Option<Rc<[RegisterOption]>>>,
}

impl RegisterOptionsCache {
    #[must_use]
    pub fn new(possible_orders_len: usize) -> Self {
        RegisterOptionsCache {
            options: vec![None; possible_orders_len],
        }
    }
}

impl Cycle {
    #[must_use]
    pub fn partitions(&self) -> &[Vec<CyclePartition>] {
        &self.partitions
    }
}

/// A prime power `prime^exp` of a register's order, at index `prime_index`
/// into [`FIRST_129_PRIMES`].
#[derive(Debug, Clone, Copy)]
struct PrimePower {
    prime_index: usize,
    prime: u16,
    exp: u8,
}

/// One way to realize a [`PrimePower`]: a cycle of length
/// `prime^(exp - absorbed_exp)` in `orbit_index` whose accumulated
/// orientation has order `prime^absorbed_exp`.
#[derive(Debug, Clone, Copy)]
struct PrimePowerPlacement {
    orbit_index: usize,
    absorbed_exp: u8,
    length: u16,
}

/// The exponent of `prime` in `n`
fn valuation(mut n: u16, prime: u16) -> u8 {
    let mut exp = 0;
    while n.is_multiple_of(prime) {
        n /= prime;
        exp += 1;
    }
    exp
}

fn pow_checked(prime: u16, exp: u8) -> Option<u16> {
    let mut result = 1u16;
    for _ in 0..exp {
        result = result.checked_mul(prime)?;
    }
    Some(result)
}

/// The state of one orbit of one register before orientations are chosen.
#[derive(Debug, Default, Clone)]
struct OrbitAtoms {
    /// `(length, prime_index, absorbed_exp)` with `length >= 2`
    cycles: Vec<(u16, usize, u8)>,
    /// `(prime_index, exp)` prime powers riding purely on orientation
    orientation_only: Vec<(usize, u8)>,
}

/// A resolved candidate decomposition of one register, comparable by
/// per-orbit piece usage.
#[derive(Debug, Clone)]
struct RegisterOption {
    piece_usage: Vec<u16>,
    partitions: Vec<Vec<CyclePartition>>,
}

/// Choose an orientation value in `Z_count` for every atom of an orbit such
/// that every orientation-riding prime power is delivered by some atom, no
/// atom's orientation order exceeds what divides the register's order, and
/// (if constrained) the values sum to zero. Atoms are the orbit's cycles
/// plus `extra_lone_pieces` in-place oriented pieces. Returns the chosen
/// orientation orders, cycles first then lone pieces.
#[allow(clippy::too_many_lines)]
fn choose_orientations<const N: usize>(
    atoms: &OrbitAtoms,
    order: &OrderExps<N>,
    count: u16,
    require_zero_sum: bool,
    extra_lone_pieces: u16,
) -> Option<Vec<u16>> {
    debug_assert!(count > 1);
    let deliveries = &atoms.orientation_only;
    // Excessive orientation-riding prime powers do not occur for real
    // puzzles; reject rather than overflow the delivery bitmask.
    if deliveries.len() >= 16 {
        return None;
    }
    let full_mask = (1u16 << deliveries.len()) - 1;

    // For each value of Z_count, precompute the exponents of its order at
    // the primes dividing count, and whether the order divides the register
    // order (necessary for the atom's contribution to not change the order).
    let count_primes = FIRST_129_PRIMES
        .iter()
        .copied()
        .take_while(|&p| p <= count)
        .enumerate()
        .filter(|&(_, p)| count.is_multiple_of(p))
        .collect::<Vec<_>>();
    let value_infos = (0..count)
        .map(|value| {
            let ord = count / gcd(count, value);
            let exps = count_primes
                .iter()
                .map(|&(_, p)| valuation(ord, p))
                .collect::<Vec<_>>();
            let divides_order = count_primes
                .iter()
                .zip(&exps)
                .all(|(&(prime_index, _), &exp)| exp <= order.0[prime_index]);
            (exps, divides_order)
        })
        .collect::<Vec<_>>();

    let delivered_mask = |value: u16| -> u16 {
        let (exps, _) = &value_infos[value as usize];
        let mut mask = 0;
        for (i, &(prime_index, exp)) in deliveries.iter().enumerate() {
            if let Some(count_prime_pos) =
                count_primes.iter().position(|&(pi, _)| pi == prime_index)
                && exps[count_prime_pos] == exp
            {
                mask |= 1 << i;
            }
        }
        mask
    };

    // An atom accepts a value if its designated prime is oriented by exactly
    // `absorbed_exp` (an excess would overshoot the prime power, a deficit
    // would under-deliver it) and the order divides the register order.
    let cycle_accepts = |value: u16, prime_index: usize, absorbed_exp: u8| -> bool {
        let (exps, divides_order) = &value_infos[value as usize];
        if !divides_order {
            return false;
        }
        match count_primes.iter().position(|&(pi, _)| pi == prime_index) {
            Some(count_prime_pos) => exps[count_prime_pos] == absorbed_exp,
            None => absorbed_exp == 0,
        }
    };

    // Lone pieces accept any nontrivial orientation dividing the register
    // order (a trivial one would be a wasted piece).
    let lone_accepts = |value: u16| -> bool { value != 0 && value_infos[value as usize].1 };

    let atom_count = atoms.cycles.len() + usize::from(extra_lone_pieces);
    let state_count = usize::from(count) << deliveries.len();
    // parents[atom][state] = (previous state, value) of one path reaching
    // `state` after assigning the first `atom + 1` atoms
    let mut parents = vec![vec![None::<(usize, u16)>; state_count]; atom_count];
    let mut reachable = vec![false; state_count];
    reachable[0] = true;

    for (atom_index, parent) in parents.iter_mut().enumerate() {
        let mut next_reachable = vec![false; state_count];
        for (state, _) in reachable
            .iter()
            .enumerate()
            .filter(|&(_, &is_reachable)| is_reachable)
        {
            let sum = state % usize::from(count);
            let mask = state / usize::from(count);
            for value in 0..count {
                let accepted = match atoms.cycles.get(atom_index) {
                    Some(&(_, prime_index, absorbed_exp)) => {
                        cycle_accepts(value, prime_index, absorbed_exp)
                    }
                    None => lone_accepts(value),
                };
                if !accepted {
                    continue;
                }
                let next_sum = (sum + usize::from(value)) % usize::from(count);
                let next_mask = mask | usize::from(delivered_mask(value));
                let next_state = next_mask * usize::from(count) + next_sum;
                if parent[next_state].is_none() {
                    parent[next_state] = Some((state, value));
                    next_reachable[next_state] = true;
                }
            }
        }
        reachable = next_reachable;
    }

    let final_state = (0..state_count).find(|&state| {
        let sum = state % usize::from(count);
        let mask = state / usize::from(count);
        let reached = if atom_count == 0 {
            state == 0
        } else {
            reachable[state]
        };
        reached && mask == usize::from(full_mask) && (!require_zero_sum || sum == 0)
    })?;

    let mut values = vec![0u16; atom_count];
    let mut state = final_state;
    for atom_index in (0..atom_count).rev() {
        let (prev_state, value) = parents[atom_index][state].unwrap();
        values[atom_index] = value;
        state = prev_state;
    }
    Some(
        values
            .into_iter()
            .map(|value| count / gcd(count, value))
            .collect(),
    )
}

fn gcd(a: u16, b: u16) -> u16 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// Resolve the orientations of one orbit of one register, preferring the
/// fewest extra in-place oriented pieces. One extra piece can deliver every
/// orientation-riding prime power at once and a second can always fix the
/// orientation sum, so two extra pieces always suffice.
fn resolve_orbit<const N: usize>(
    atoms: &OrbitAtoms,
    order: &OrderExps<N>,
    count: u16,
    require_zero_sum: bool,
) -> Option<Vec<CyclePartition>> {
    if count == 1 {
        debug_assert!(atoms.orientation_only.is_empty());
        return Some(
            atoms
                .cycles
                .iter()
                .map(|&(length, _, _)| CyclePartition {
                    length,
                    orientation_order: 1,
                })
                .collect(),
        );
    }
    (0..=2u16).find_map(|extra_lone_pieces| {
        let orientation_orders =
            choose_orientations(atoms, order, count, require_zero_sum, extra_lone_pieces)?;
        Some(
            atoms
                .cycles
                .iter()
                .map(|&(length, _, _)| length)
                .chain(std::iter::repeat_n(1, usize::from(extra_lone_pieces)))
                .zip(orientation_orders)
                .map(|(length, orientation_order)| CyclePartition {
                    length,
                    orientation_order,
                })
                .collect(),
        )
    })
}

/// Fix violated even parity constraints by placing junk 2-cycles, returning
/// every minimal-ish choice of orbits to place them in. The constraint
/// matrix is Gauss-Jordan reduced, so setting the pivot orbit of each
/// violated row is always a solution; the null space enumerates the
/// alternatives.
fn junk_placements<const N: usize>(
    puzzle_def: &PuzzleDef<N>,
    orbit_parities: &[bool],
) -> Vec<Vec<usize>> {
    let constraints = puzzle_def.even_parity_constraints();
    let rows = constraints.rows();
    let cols = constraints.cols();

    let violated = (0..rows)
        .map(|row| {
            (0..cols)
                .filter(|&col| constraints.bit(row, col))
                .fold(false, |parity, col| parity ^ orbit_parities[col])
        })
        .collect::<Vec<_>>();
    if violated.iter().all(|&v| !v) {
        return vec![vec![]];
    }

    let pivots = (0..rows)
        .map(|row| (0..cols).find(|&col| constraints.bit(row, col)).unwrap())
        .collect::<Vec<_>>();
    let particular = (0..rows)
        .filter(|&row| violated[row])
        .map(|row| pivots[row])
        .collect::<Vec<_>>();

    // Free columns that appear in some constraint yield alternative
    // placements; cap the enumeration for exotic puzzles.
    let free_cols = (0..cols)
        .filter(|&col| !pivots.contains(&col) && (0..rows).any(|row| constraints.bit(row, col)))
        .take(4)
        .collect::<Vec<_>>();

    let mut placements = vec![];
    for free_subset in 0..(1usize << free_cols.len()) {
        let mut in_solution = vec![false; cols];
        for row in 0..rows {
            in_solution[pivots[row]] = violated[row];
        }
        for (i, &free_col) in free_cols.iter().enumerate() {
            if free_subset & (1 << i) == 0 {
                continue;
            }
            in_solution[free_col] ^= true;
            for row in 0..rows {
                if constraints.bit(row, free_col) {
                    in_solution[pivots[row]] ^= true;
                }
            }
        }
        placements.push(
            in_solution
                .iter()
                .enumerate()
                .filter_map(|(orbit_index, &set)| set.then_some(orbit_index))
                .collect(),
        );
    }
    debug_assert!(placements.contains(&particular));
    placements
}

/// Enumerate every Pareto-minimal way to realize `order` on the puzzle,
/// disregarding the other registers.
fn register_options<const N: usize>(
    order: &OrderExps<N>,
    puzzle_def: &PuzzleDef<N>,
) -> Vec<RegisterOption> {
    let orbit_defs = puzzle_def.orbit_defs();
    let orientations_exps = puzzle_def.orientations_exps();

    let prime_powers = order
        .0
        .as_array()
        .iter()
        .take(N)
        .enumerate()
        .filter(|&(_, &exp)| exp != 0)
        .map(|(prime_index, &exp)| PrimePower {
            prime_index,
            prime: FIRST_129_PRIMES[prime_index],
            exp,
        })
        .collect::<Vec<_>>();

    let placements_per_prime_power = prime_powers
        .iter()
        .map(|prime_power| {
            let mut placements = vec![];
            for (orbit_index, orbit_def) in orbit_defs.iter().enumerate() {
                let max_absorbed_exp = prime_power
                    .exp
                    .min(orientations_exps[orbit_index].0[prime_power.prime_index]);
                for absorbed_exp in 0..=max_absorbed_exp {
                    let Some(length) =
                        pow_checked(prime_power.prime, prime_power.exp - absorbed_exp)
                    else {
                        continue;
                    };
                    if length <= orbit_def.piece_count.get() {
                        placements.push(PrimePowerPlacement {
                            orbit_index,
                            absorbed_exp,
                            length,
                        });
                    }
                }
            }
            placements
        })
        .collect::<Vec<_>>();
    if placements_per_prime_power
        .iter()
        .any(std::vec::Vec::is_empty)
    {
        return vec![];
    }

    let orbit_count = orbit_defs.len().get();
    let mut options = vec![];
    let mut chosen = vec![0usize; prime_powers.len()];
    'assignments: loop {
        // At a leaf, `chosen` selects one placement per prime power
        let mut per_orbit = vec![OrbitAtoms::default(); orbit_count];
        for ((prime_power, placements), &placement_index) in prime_powers
            .iter()
            .zip(&placements_per_prime_power)
            .zip(&chosen)
        {
            let placement = placements[placement_index];
            let orbit = &mut per_orbit[placement.orbit_index];
            if placement.length == 1 {
                orbit
                    .orientation_only
                    .push((prime_power.prime_index, placement.absorbed_exp));
            } else {
                orbit.cycles.push((
                    placement.length,
                    prime_power.prime_index,
                    placement.absorbed_exp,
                ));
            }
        }
        collect_assignment_options(order, puzzle_def, &per_orbit, &mut options);

        // Advance the mixed-radix counter over placements
        for (digit, placements) in chosen.iter_mut().zip(&placements_per_prime_power) {
            *digit += 1;
            if *digit < placements.len() {
                continue 'assignments;
            }
            *digit = 0;
        }
        pareto_prune(&mut options);
        return options;
    }
}

/// Resolve orientations and parity for one placement assignment and push the
/// resulting options.
fn collect_assignment_options<const N: usize>(
    order: &OrderExps<N>,
    puzzle_def: &PuzzleDef<N>,
    per_orbit: &[OrbitAtoms],
    options: &mut Vec<RegisterOption>,
) {
    let orbit_defs = puzzle_def.orbit_defs();
    let mut partitions = Vec::with_capacity(per_orbit.len());
    for (orbit_index, atoms) in per_orbit.iter().enumerate() {
        let orbit_def = orbit_defs[orbit_index];
        let count = u16::from(orbit_def.orientation_count().get());
        let require_zero_sum = matches!(
            orbit_def.orientation,
            OrientationStatus::CanOrient {
                sum_constraint: OrientationSumConstraint::Zero,
                ..
            }
        );
        let Some(orbit_partitions) = resolve_orbit(atoms, order, count, require_zero_sum) else {
            return;
        };
        partitions.push(orbit_partitions);
    }

    // A cycle of even length is an odd permutation
    let orbit_parities = partitions
        .iter()
        .map(|cycles| {
            cycles
                .iter()
                .filter(|cycle| cycle.length.is_multiple_of(2))
                .count()
                % 2
                == 1
        })
        .collect::<Vec<_>>();
    for junk_orbits in junk_placements(puzzle_def, &orbit_parities) {
        // A junk 2-cycle contributes order 2, which must divide the
        // register's order. Parity violations can only arise from
        // even-length cycles, so this only rejects wasteful null space
        // alternatives.
        if !junk_orbits.is_empty() && order.two_exponent() == 0 {
            continue;
        }
        let mut junk_partitions = partitions.clone();
        for &orbit_index in &junk_orbits {
            junk_partitions[orbit_index].push(CyclePartition {
                length: 2,
                orientation_order: 1,
            });
        }
        let piece_usage = junk_partitions
            .iter()
            .zip(orbit_defs.iter())
            .map(|(cycles, orbit_def)| {
                let pieces = cycles.iter().map(|c| u32::from(c.length)).sum::<u32>();
                (pieces <= u32::from(orbit_def.piece_count.get()))
                    .then(|| u16::try_from(pieces).unwrap())
            })
            .collect::<Option<Vec<_>>>();
        if let Some(piece_usage) = piece_usage {
            options.push(RegisterOption {
                piece_usage,
                partitions: junk_partitions,
            });
        }
    }
}

fn pareto_prune(options: &mut Vec<RegisterOption>) {
    let mut pruned: Vec<RegisterOption> = vec![];
    for option in options.drain(..) {
        if pruned.iter().any(|kept| {
            kept.piece_usage
                .iter()
                .zip(&option.piece_usage)
                .all(|(kept_usage, usage)| kept_usage <= usage)
        }) {
            continue;
        }
        pruned.retain(|kept| {
            !option
                .piece_usage
                .iter()
                .zip(&kept.piece_usage)
                .all(|(usage, kept_usage)| usage <= kept_usage)
        });
        pruned.push(option);
    }
    *options = pruned;
}

/// Pick one option per register such that the per-orbit piece budgets hold.
fn pack_registers(
    options_per_register: &[Rc<[RegisterOption]>],
    suffix_min_usage: &[Vec<u32>],
    remaining: &mut [u32],
    chosen: &mut Vec<usize>,
) -> bool {
    let register_index = chosen.len();
    if register_index == options_per_register.len() {
        return true;
    }
    if remaining
        .iter()
        .zip(&suffix_min_usage[register_index])
        .any(|(&remaining, &suffix_min)| suffix_min > remaining)
    {
        return false;
    }
    for (option_index, option) in options_per_register[register_index].iter().enumerate() {
        if option
            .piece_usage
            .iter()
            .zip(remaining.iter())
            .any(|(&usage, &remaining)| u32::from(usage) > remaining)
        {
            continue;
        }
        for (remaining, &usage) in remaining.iter_mut().zip(&option.piece_usage) {
            *remaining -= u32::from(usage);
        }
        chosen.push(option_index);
        if pack_registers(options_per_register, suffix_min_usage, remaining, chosen) {
            return true;
        }
        chosen.pop();
        for (remaining, &usage) in remaining.iter_mut().zip(&option.piece_usage) {
            *remaining += u32::from(usage);
        }
    }
    false
}

impl CycleCombinationDetails {
    /// # Panics
    ///
    /// This method panics if a register indexes out of bounds of
    /// `possible_orders_except_one` or of a `cache` constructed with a
    /// smaller possible orders length.
    #[must_use]
    pub fn new<const N: usize>(
        registers: DisjointRegisters,
        possible_orders_except_one: &[PossibleOrder<N>],
        puzzle_def: &PuzzleDef<N>,
        cache: &mut RegisterOptionsCache,
    ) -> Option<Self> {
        let options_per_register = registers
            .iter()
            .map(|register_index| {
                let register_index = register_index as usize;
                cache.options[register_index]
                    .get_or_insert_with(|| {
                        let mut options = register_options(
                            &possible_orders_except_one[register_index].order,
                            puzzle_def,
                        );
                        options.sort_unstable_by_key(|option| {
                            option
                                .piece_usage
                                .iter()
                                .map(|&u| u32::from(u))
                                .sum::<u32>()
                        });
                        Rc::from(options)
                    })
                    .clone()
            })
            .collect::<Vec<_>>();
        if options_per_register
            .iter()
            .any(|options| options.is_empty())
        {
            return None;
        }

        let orbit_count = puzzle_def.orbit_defs().len().get();
        // The componentwise minimum may not be jointly achievable, which
        // keeps the pruning bound admissible
        let mut suffix_min_usage = vec![vec![0u32; orbit_count]];
        for options in options_per_register.iter().rev() {
            let min_usage = options.iter().map(|option| &option.piece_usage).fold(
                vec![u32::MAX; orbit_count],
                |min_usage, piece_usage| {
                    min_usage
                        .into_iter()
                        .zip(piece_usage)
                        .map(|(min, &usage)| min.min(u32::from(usage)))
                        .collect()
                },
            );
            let next = suffix_min_usage
                .last()
                .unwrap()
                .iter()
                .zip(min_usage)
                .map(|(&suffix, min)| suffix + min)
                .collect();
            suffix_min_usage.push(next);
        }
        suffix_min_usage.reverse();

        let mut remaining = puzzle_def
            .orbit_defs()
            .iter()
            .map(|orbit_def| u32::from(orbit_def.piece_count.get()))
            .collect::<Vec<_>>();
        let mut chosen = Vec::with_capacity(options_per_register.len());
        pack_registers(
            &options_per_register,
            &suffix_min_usage,
            &mut remaining,
            &mut chosen,
        )
        .then(|| CycleCombinationDetails {
            cycles: options_per_register
                .iter()
                .zip(&chosen)
                .map(|(options, &option_index)| Cycle {
                    partitions: options[option_index].partitions.clone(),
                })
                .collect(),
        })
    }
}

impl CycleCombinationDetails {
    #[must_use]
    pub fn cycles(&self) -> &[Cycle] {
        &self.cycles
    }
}

#[cfg(test)]
mod tests {
    use std::num::{NonZeroU16, NonZeroU32};

    use super::{CycleCombinationDetails, RegisterOptionsCache};
    use crate::{
        cycle_combinations_tree::DisjointRegisters,
        finder::PossibleOrder,
        nonemptyvec::NonemptySlice,
        orderexps::OrderExps,
        puzzle::{
            OrientationStatus, PuzzleDef,
            cubeN::{CUBE2, CUBE3},
            minxN::MINX3,
        },
    };

    /// Build an [`OrderExps`] from a plain integer order.
    fn exps<const N: usize>(n: u16) -> OrderExps<N> {
        OrderExps::try_from(NonZeroU16::new(n).unwrap()).unwrap()
    }

    fn gcd_u64(a: u64, b: u64) -> u64 {
        if b == 0 { a } else { gcd_u64(b, a % b) }
    }

    fn lcm_u64(a: u64, b: u64) -> u64 {
        a / gcd_u64(a, b) * b
    }

    /// Structurally validate an accepted assignment against the puzzle and the
    /// requested register orders. Panics on any inconsistency.
    fn verify<const N: usize>(
        details: &CycleCombinationDetails,
        puzzle_def: &PuzzleDef<N>,
        orders: &[OrderExps<N>],
    ) {
        let orbit_defs = puzzle_def.orbit_defs();
        let orbit_count = orbit_defs.len().get();

        assert_eq!(
            details.cycles().len(),
            orders.len(),
            "one cycle per register"
        );

        let mut total_usage = vec![0u32; orbit_count];

        for (cycle, order) in details.cycles().iter().zip(orders) {
            assert_eq!(
                cycle.partitions().len(),
                orbit_count,
                "one partition list per orbit"
            );

            let mut reconstructed_order = 1u64;
            let mut orbit_parities = vec![false; orbit_count];

            for (orbit_index, partitions) in cycle.partitions().iter().enumerate() {
                let orbit_def = orbit_defs[orbit_index];
                let orientation_count = u16::from(orbit_def.orientation_count().get());
                let can_orient =
                    matches!(orbit_def.orientation, OrientationStatus::CanOrient { .. });

                let mut even_cycles = 0usize;
                for partition in partitions {
                    assert!(partition.length >= 1, "cycle length must be positive");
                    assert!(
                        partition.orientation_order >= 1,
                        "orientation order must be positive"
                    );
                    assert_eq!(
                        orientation_count % partition.orientation_order,
                        0,
                        "orientation order must divide the orbit's orientation count"
                    );
                    if partition.orientation_order > 1 {
                        assert!(
                            can_orient,
                            "an oriented cycle must live in an orientable orbit"
                        );
                    }

                    reconstructed_order = lcm_u64(
                        reconstructed_order,
                        u64::from(partition.length) * u64::from(partition.orientation_order),
                    );
                    total_usage[orbit_index] += u32::from(partition.length);
                    if partition.length % 2 == 0 {
                        even_cycles += 1;
                    }
                }
                orbit_parities[orbit_index] = even_cycles % 2 == 1;
            }

            let expected_order = u64::try_from(order.as_bigint()).unwrap();
            assert_eq!(
                reconstructed_order, expected_order,
                "reconstructed register order must match exactly"
            );

            // Every even parity constraint must be satisfied by this register.
            let constraints = puzzle_def.even_parity_constraints();
            for row in 0..constraints.rows() {
                let parity = (0..constraints.cols())
                    .filter(|&col| constraints.bit(row, col))
                    .fold(false, |acc, col| acc ^ orbit_parities[col]);
                assert!(!parity, "even parity constraint {row} violated");
            }
        }

        for (orbit_index, &usage) in total_usage.iter().enumerate() {
            assert!(
                usage <= u32::from(orbit_defs[orbit_index].piece_count.get()),
                "orbit {orbit_index} overcommitted: {usage} pieces used"
            );
        }
    }

    /// Run `CycleCombinationDetails::new` for the given register orders,
    /// verifying any accepted assignment before returning it.
    fn details<const N: usize>(
        puzzle_def: &PuzzleDef<N>,
        orders: &[OrderExps<N>],
    ) -> Option<CycleCombinationDetails> {
        let possible_orders = orders
            .iter()
            .map(|order| PossibleOrder {
                order: order.clone(),
                min_piece_count: NonZeroU32::new(1).unwrap(),
            })
            .collect::<Vec<_>>();
        let indices = (0..u32::try_from(orders.len()).unwrap()).collect::<Vec<_>>();
        let registers = DisjointRegisters::from(NonemptySlice::try_from(&indices[..]).unwrap());

        let mut cache = RegisterOptionsCache::new(possible_orders.len());
        let maybe_details =
            CycleCombinationDetails::new(registers, &possible_orders, puzzle_def, &mut cache);
        if let Some(details) = &maybe_details {
            verify(details, puzzle_def, orders);
        }
        maybe_details
    }

    /// Total pieces used per orbit across every register of an assignment.
    fn total_usage<const N: usize>(
        details: &CycleCombinationDetails,
        puzzle_def: &PuzzleDef<N>,
    ) -> Vec<u32> {
        let orbit_count = puzzle_def.orbit_defs().len().get();
        let mut usage = vec![0u32; orbit_count];
        for cycle in details.cycles() {
            for (orbit_index, partitions) in cycle.partitions().iter().enumerate() {
                for partition in partitions {
                    usage[orbit_index] += u32::from(partition.length);
                }
            }
        }
        usage
    }

    #[test_log::test]
    fn cube3_90_90() {
        let cube3 = &*CUBE3;
        let orders = [exps::<8>(90), exps::<8>(90)];
        let details = details(cube3, &orders).expect("90, 90 must fit on the 3x3");
        // The only fit is (4 corners, 6 edges) per register.
        assert_eq!(
            total_usage(&details, cube3),
            vec![8, 12],
            "must use exactly 8 corners and 12 edges"
        );
    }

    #[test_log::test]
    fn cube3_63_63_infeasible() {
        let cube3 = &*CUBE3;
        let orders = [exps::<8>(63), exps::<8>(63)];
        // 63 = 3^2 * 7: no pair of options fits 8 corners / 12 edges, even
        // though the total piece count (20) passes the min-piece-count prune.
        assert!(details(cube3, &orders).is_none());
    }

    #[test_log::test]
    fn cube3_90_90_90_infeasible() {
        let cube3 = &*CUBE3;
        let orders = [exps::<8>(90), exps::<8>(90), exps::<8>(90)];
        // Three registers exhaust the 8 corners.
        assert!(details(cube3, &orders).is_none());
    }

    fn singleton_sweep<const N: usize>(puzzle_def: &PuzzleDef<N>) {
        let set = puzzle_def.possible_orders(None).unwrap();
        set.remove(&OrderExps::one());
        let mut orders = set.into_iter().collect::<Vec<_>>();
        orders.sort_unstable();
        for order in orders {
            assert!(
                details(puzzle_def, std::slice::from_ref(&order)).is_some(),
                "every possible order must be realizable by a single element, but {} was not",
                order.as_bigint()
            );
        }
    }

    #[test_log::test]
    fn singleton_possible_orders_realizable() {
        singleton_sweep(&*CUBE2);
        singleton_sweep(&*CUBE3);
        singleton_sweep(&*MINX3);
    }

    #[test_log::test]
    fn minx3_2520_630_420() {
        let minx3 = &*MINX3;
        let orders = [exps::<16>(2520), exps::<16>(630), exps::<16>(420)];
        // Under fully disjoint packing this does not fit: 2520 and 420 each
        // carry an odd edge permutation, and their parity 2-cycles push every
        // assignment over the 30 edges. Cross-register piece sharing (the
        // TODO in the module docs) may make it feasible; revisit this pin
        // when that lands.
        assert!(details(minx3, &orders).is_none());
    }
}
