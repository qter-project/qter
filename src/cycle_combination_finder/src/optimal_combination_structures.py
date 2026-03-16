"""
Finds pairs of commutative cycles on a Rubik's cube that have high products of
orders. The output of this phase is directly used in the CCS.
"""

import collections
import dataclasses
import enum
import functools
import itertools
import math
import operator
import timeit

import puzzle_orbit_definitions
from common_types import (
    EvenParityConstraint,  # noqa: F401
    Orbit,  # noqa: F401
    OrientationStatus,
    OrientationSumConstraint,
    PuzzleOrbitDefinition,  # noqa: F401
)

CycleCombination = collections.namedtuple(
    "CycleCombination",
    [
        "used_cubie_counts",
        "order_product",
        "share_orders",
        "cycle_combination",
    ],
)

Cycle = collections.namedtuple(
    "Cycle",
    [
        "order",
        "share",
        "partition_objs",
    ],
)

CubiePartition = collections.namedtuple(
    "CubiePartition",
    [
        "name",
        "partition",
        "order",
        "always_orient",
        "critical_orient",
    ],
)


# EvenParityConstraintsHelper = collections.namedtuple(
#     "EvenParityConstraintsHelper",
#     [
#         "first_constraint_indicies",
#         "rest_constraint_flags",
#         "constraint_orbit_flags",
#     ],
# )
@dataclasses.dataclass(frozen=True, unsafe_hash=True)
class EvenParityConstraintsHelper:
    first_constraint_indicies: tuple[int]
    rest_constraint_flags: tuple[tuple[bool]]
    constraint_orbit_flags: tuple[bool]

    @classmethod
    def from_puzzle_orbit_definition(
        cls,
        puzzle_orbit_definition,
    ):
        all_first_index_and_rest_constraint_flags = []
        constraint_orbit_flags = [False] * len(puzzle_orbit_definition.orbits)
        for even_parity_constraint in puzzle_orbit_definition.even_parity_constraints:
            add_to_rest = False
            first_index = None
            rest_constraint_flags = []
            constraint_flag_count = 0
            for i, orbit in enumerate(puzzle_orbit_definition.orbits):
                constraint_flag = any(
                    orbit.name == orbit_name
                    for orbit_name in even_parity_constraint.orbit_names
                )
                if constraint_flag:
                    constraint_orbit_flags[i] = True
                    constraint_flag_count += 1
                if add_to_rest:
                    rest_constraint_flags.append(constraint_flag)
                elif constraint_flag:
                    first_index = i
                    add_to_rest = True
            if constraint_flag_count != len(even_parity_constraint.orbit_names):
                raise ValueError(
                    f"Invalid orbit names {even_parity_constraint.orbit_names}"
                )
            all_first_index_and_rest_constraint_flags.append(
                (
                    first_index,
                    tuple(rest_constraint_flags),
                )
            )
        all_first_index_and_rest_constraint_flags.sort(
            reverse=True, key=operator.itemgetter(0)
        )

        first_constraint_indicies = []
        all_rest_constraint_flags = []
        for (
            first_index_and_rest_constraint_flags
        ) in all_first_index_and_rest_constraint_flags:
            first_index, rest_constraint_flags = first_index_and_rest_constraint_flags
            first_constraint_indicies.append(first_index)
            all_rest_constraint_flags.append(rest_constraint_flags)

        return cls(
            first_constraint_indicies=tuple(first_constraint_indicies),
            rest_constraint_flags=tuple(all_rest_constraint_flags),
            constraint_orbit_flags=tuple(constraint_orbit_flags),
        )


class ShareState(enum.Enum):
    FREE = enum.auto()
    CANNOT_SHARE_ORIENTATION = enum.auto()
    MUST_SHARE_ORIENTATION = enum.auto()


@functools.cache
def integer_partitions(n):
    """
    Find the [integer partition](https://en.wikipedia.org/wiki/Integer_partition)
    of n.

    Intuitively, this represents the permutation orders of all possible sets of
    cycles that use n cubies, ignoring parity and orientation concerns.

    Group theory-wise, the positions of edge and corner pieces are isomorphic to
    the symmetric groups of size 12 and 8 respectively. Since the conjugacy
    classes of a symmetric group [correspond](https://en.wikipedia.org/wiki/Symmetric_group#Conjugacy_classes)
    to integer partitions, this can also be thought of as a representation of the
    conjugacy classes of those symmetric groups.

    Taken from <https://stackoverflow.com/a/10036764/12230735>.
    """
    if n == 0:
        return {()}
    answer = {(n,)}
    for x in range(1, n):
        for y in integer_partitions(n - x):
            answer.add(tuple(sorted((x,) + y)))
    return answer


# https://stackoverflow.com/a/6285330/12230735
def unique_permutations(iterable, r=None):
    previous = ()
    for p in itertools.permutations(sorted(iterable), r):
        if p > previous:
            previous = p
            yield p


def p_adic_valuation(n, p):
    """
    Calculate the [p-adic valuation](https://en.wikipedia.org/wiki/P-adic_valuation).
    """
    exponent = 0
    while n % p == 0 and n != 0:
        n //= p
        exponent += 1
    return exponent


@functools.cache
def sign(partition):
    """
    Calculate the [signature](https://en.wikipedia.org/wiki/Parity_of_a_permutation)
    of a partition, made easy by having all cycle lengths.
    """
    return sum(partition) - len(partition)


def cycle_combination_dominates(this, other):
    # A modification of the weakly dominates condition in the pareto efficient
    # algorithm
    different_orders = False
    same_cycle = this.share_orders == other.share_orders
    for this_cycle, other_cycle in zip(this.cycle_combination, other.cycle_combination):
        if other_cycle.order > this_cycle.order:
            return False
        if different_orders:
            continue
        different_orders |= this_cycle.order > other_cycle.order
        same_cycle &= all(
            this_cubie_partition == other_cubie_partition
            for this_cubie_partition, other_cubie_partition in zip(
                map(
                    operator.attrgetter("partition"),
                    this_cycle.partition_objs,
                ),
                map(
                    operator.attrgetter("partition"),
                    other_cycle.partition_objs,
                ),
            )
        )

    return different_orders or same_cycle


def optimal_cycle_combinations(puzzle_orbit_definition, num_cycles, cache_clear=True):
    even_parity_constraints_helper = (
        EvenParityConstraintsHelper.from_puzzle_orbit_definition(
            puzzle_orbit_definition
        )
    )

    cycle_combination_objs = []
    # TODO(pri 1/5): upper bound of LCM is math.lcm(*range(1, <max orbit cubie count> + 1))
    # TODO(pri 4/5): derive all lesser structures from max cubie count usage and fix only 1s, note that 1s are currently allowed in cannotorient orbits
    # TODO(pri 5/5): share parity
    for used_cubie_counts in itertools.product(
        # when 0, the partition is all zeros which is disallowed later
        *(range(1, orbit.cubie_count + 1) for orbit in puzzle_orbit_definition.orbits)
    ):
        for all_partition_cubie_counts in itertools.product(
            *map(integer_partitions, used_cubie_counts),
        ):
            all_partition_cubie_counts = list(all_partition_cubie_counts)
            if any(
                len(partition_cubie_counts) > num_cycles
                for partition_cubie_counts in all_partition_cubie_counts
            ):
                continue
            for i, partition_cubie_counts in enumerate(all_partition_cubie_counts):
                all_partition_cubie_counts[i] += (0,) * (
                    num_cycles - len(partition_cubie_counts)
                )
            seen_cycle_cubie_counts = set()
            # TODO: permuting can be done within integer_partitions itself
            for all_permuted_partition_cubie_counts in itertools.product(
                *map(unique_permutations, all_partition_cubie_counts)
            ):
                all_cycle_cubie_counts = []
                continue_outer = False
                for cubie_counts in zip(*all_permuted_partition_cubie_counts):
                    # TODO(pri 5/5 blocked on derive all lesser): henry's faster impl
                    if all(
                        cubie_count == 0
                        or orbit.orientation_status == OrientationStatus.CannotOrient()
                        and cubie_count == 1
                        for orbit, cubie_count in zip(
                            puzzle_orbit_definition.orbits, cubie_counts
                        )
                    ):
                        continue_outer = True
                        break
                    all_cycle_cubie_counts.append(cubie_counts)
                if continue_outer:
                    continue
                all_cycle_cubie_counts = tuple(
                    sorted(all_cycle_cubie_counts, reverse=True)
                )
                if all_cycle_cubie_counts in seen_cycle_cubie_counts:
                    continue
                seen_cycle_cubie_counts.add(all_cycle_cubie_counts)
                for shared_cycle_combination in recursive_shared_cycle_combinations(
                    all_cycle_cubie_counts,
                    puzzle_orbit_definition,
                    even_parity_constraints_helper,
                ):
                    orbits_can_share = [False] * len(puzzle_orbit_definition.orbits)
                    share_orbit_counts = [0] * len(puzzle_orbit_definition.orbits)
                    for cycle in shared_cycle_combination:
                        for i in range(len(puzzle_orbit_definition.orbits)):
                            orbits_can_share[i] |= (
                                cycle.share[i] is False
                                and 1 in cycle.partition_objs[i].partition
                            )
                            share_orbit_counts[i] += cycle.share[i]
                    if any(
                        share_orbit_count != 0 and not orbit_can_share
                        for share_orbit_count, orbit_can_share in zip(
                            share_orbit_counts, orbits_can_share
                        )
                    ):
                        continue
                    # just because we sort the parititons earlier doesnt mean the
                    # orders will be sorted
                    descending_order_cycle_combination = sorted(
                        shared_cycle_combination,
                        key=lambda cycle: (
                            cycle.order,
                            *map(
                                operator.attrgetter("partition"),
                                cycle.partition_objs,
                            ),
                        ),
                        reverse=True,
                    )
                    for i, start_cycle_to_permute in enumerate(
                        descending_order_cycle_combination
                    ):
                        if i == 0:
                            start_permuted_descending_order_cycle_combination = (
                                descending_order_cycle_combination
                            )
                        else:
                            # We only permute the cycles that have the same maximum
                            # order because the partition permutation for same order
                            # cycles matters for the CCS. Don't permute the rest
                            # because that logic is implemented in phase 3 (more
                            # efficient to do this in phase 3 vs here).
                            if (
                                start_cycle_to_permute.order
                                != descending_order_cycle_combination[0].order
                            ):
                                break
                            if all(
                                prev_cubie_partition == curr_cubie_partition
                                for prev_cubie_partition, curr_cubie_partition in zip(
                                    map(
                                        operator.attrgetter("partition"),
                                        descending_order_cycle_combination[
                                            i - 1
                                        ].partition_objs,
                                    ),
                                    map(
                                        operator.attrgetter("partition"),
                                        start_cycle_to_permute.partition_objs,
                                    ),
                                )
                            ):
                                continue
                            start_permuted_descending_order_cycle_combination = (
                                descending_order_cycle_combination.copy()
                            )
                            (
                                start_permuted_descending_order_cycle_combination[0],
                                start_permuted_descending_order_cycle_combination[i],
                            ) = (
                                start_permuted_descending_order_cycle_combination[i],
                                start_permuted_descending_order_cycle_combination[0],
                            )

                        for j in range(len(puzzle_orbit_definition.orbits)):
                            orbits_can_share[j] = False
                        all_share_orbit_cycle_candidates = [
                            [] for _ in range(len(puzzle_orbit_definition.orbits))
                        ]

                        order_product = 1
                        for j, cycle in enumerate(
                            start_permuted_descending_order_cycle_combination
                        ):
                            for k in range(len(puzzle_orbit_definition.orbits)):
                                if (
                                    orbits_can_share[k]
                                    and 1 in cycle.partition_objs[k].partition
                                ):
                                    all_share_orbit_cycle_candidates[k].append(j)
                                orbits_can_share[k] |= (
                                    1 in cycle.partition_objs[k].partition
                                )
                            order_product *= cycle.order

                        assert all(
                            share_orbit_count == 0
                            or len(share_orbit_cycle_candidates) != 0
                            for share_orbit_cycle_candidates, share_orbit_count in zip(
                                all_share_orbit_cycle_candidates, share_orbit_counts
                            )
                        )

                        share_orders = [
                            tuple(
                                tuple(
                                    j in share_orbit_indicies
                                    for share_orbit_indicies in all_share_orbit_indicies
                                )
                                for j in range(
                                    len(
                                        start_permuted_descending_order_cycle_combination
                                    )
                                )
                            )
                            for all_share_orbit_indicies in itertools.product(
                                # given a list "share_edge_candidates", what are all ways to
                                # pick "share_edge_count" numbers from the list
                                *(
                                    itertools.combinations(
                                        share_orbit_cycle_candidates,
                                        share_orbit_count,
                                    )
                                    for share_orbit_cycle_candidates, share_orbit_count in zip(
                                        all_share_orbit_cycle_candidates,
                                        share_orbit_counts,
                                    )
                                )
                            )
                        ]

                        # According to
                        # https://github.com/nestordemeure/paretoFront/blob/2aea69c371f70de4665f8abf24f6fda4ef0a8a70/src/pareto_front_implementation/pareto_front.rs#L265
                        # it is not worth removing redundant cycles
                        # intermediately
                        cycle_combination_objs.append(
                            CycleCombination(
                                used_cubie_counts=used_cubie_counts,
                                order_product=order_product,
                                share_orders=share_orders,
                                cycle_combination=start_permuted_descending_order_cycle_combination,
                            )
                        )
    if cache_clear:
        recursive_shared_cycle_combinations.cache_clear()
        highest_order_cycles_from_cubie_counts.cache_clear()
        reduced_integer_partitions.cache_clear()
    return pareto_efficient_cycle_combinations(cycle_combination_objs)


# do not flush cache it is used across used cubie counts
@functools.cache
def recursive_shared_cycle_combinations(
    all_cycle_cubie_counts, puzzle_orbit_definition, even_parity_constraints_helper
):
    if len(all_cycle_cubie_counts) == 0:
        return ((),)
    return tuple(
        (shared_cycle,) + rest_combination
        for shared_cycle in highest_order_cycles_from_cubie_counts(
            all_cycle_cubie_counts[0],
            puzzle_orbit_definition,
            even_parity_constraints_helper,
        )
        for rest_combination in recursive_shared_cycle_combinations(
            all_cycle_cubie_counts[1:],
            puzzle_orbit_definition,
            even_parity_constraints_helper,
        )
    )


# TODO(pri 3/5): on bigger cubes where the CCS is not applicable, do special
# optimizations that make this faster. only find the highest order
# product cycle dont care abt duplicates
@functools.cache
def highest_order_cycles_from_cubie_counts(
    cycle_cubie_counts, puzzle_orbit_definition, even_parity_constraints_helper
):
    shared_cycles = []
    highest_order = 1
    share_states = []
    free_share_count = 0
    for i, cubie_count in enumerate(cycle_cubie_counts):
        if (
            cubie_count == 0
            # TODO(pri 3/5 blocked on deriving lesser): cubie_count == used_cubie_counts[i]
            or puzzle_orbit_definition.orbits[i].orientation_status
            == OrientationStatus.CannotOrient()
        ):
            share_states.append(ShareState.CANNOT_SHARE_ORIENTATION)
        elif cubie_count == 1:
            share_states.append(ShareState.MUST_SHARE_ORIENTATION)
        else:
            share_states.append(ShareState.FREE)
            free_share_count += 1
    for free_share in itertools.product(
        (False, True),
        repeat=free_share_count,
    ):
        share = []
        free_share_next_index = 0
        for share_state in share_states:
            match share_state:
                case ShareState.FREE:
                    share.append(free_share[free_share_next_index])
                    free_share_next_index += 1
                case ShareState.CANNOT_SHARE_ORIENTATION:
                    share.append(False)
                case ShareState.MUST_SHARE_ORIENTATION:
                    share.append(True)
        all_reduced_integer_partitions = [
            reduced_integer_partitions(
                cycle_cubie_counts[i],
                i,
                share[i],
                puzzle_orbit_definition,
                even_parity_constraints_helper,
            )
            for i in range(len(cycle_cubie_counts))
        ]

        rest_upper_bounds = []
        cycles = []
        partition_obj_path = [None] * len(all_reduced_integer_partitions)
        rest_upper_bound = 1

        for partition_obj in map(
            operator.itemgetter(0), all_reduced_integer_partitions
        ):
            rest_upper_bounds.append(rest_upper_bound)
            rest_upper_bound *= partition_obj.order

        stack = [(len(all_reduced_integer_partitions) - 1, 1, None, 0)]
        while stack:
            i, running_order, partition_obj, next_even_parity_constraint_index = (
                stack.pop()
            )
            if partition_obj is not None:
                partition_obj_path[i + 1] = partition_obj
            continue_outer = False
            while (
                next_even_parity_constraint_index
                < len(even_parity_constraints_helper.first_constraint_indicies)
                and i + 1
                == even_parity_constraints_helper.first_constraint_indicies[
                    next_even_parity_constraint_index
                ]
            ):
                if (
                    sign(partition_obj.partition)
                    + sum(
                        sign(partition)
                        for j, partition in enumerate(
                            map(
                                operator.attrgetter("partition"),
                                partition_obj_path[i + 2 :],
                            )
                        )
                        if even_parity_constraints_helper.rest_constraint_flags[
                            next_even_parity_constraint_index
                        ][j]
                    )
                ) % 2 != 0:
                    continue_outer = True
                    break
                next_even_parity_constraint_index += 1
            if continue_outer:
                continue

            if i != -1:
                for partition_obj in all_reduced_integer_partitions[i]:
                    rest_upper_bound = running_order * partition_obj.order
                    if rest_upper_bound * rest_upper_bounds[i] < highest_order:
                        break
                    stack.append(
                        (
                            i - 1,
                            rest_upper_bound
                            // math.gcd(running_order, partition_obj.order),
                            partition_obj,
                            next_even_parity_constraint_index,
                        )
                    )
                continue
            if running_order > highest_order:
                cycles.clear()
            if running_order < highest_order:
                continue
            highest_order = running_order
            cycles.append(
                Cycle(
                    order=running_order,
                    share=share,
                    partition_objs=partition_obj_path.copy(),
                )
            )
        shared_cycles.extend(cycles)
    return shared_cycles


@functools.cache
def reduced_integer_partitions(
    cycle_cubie_count,
    orbit_index,
    s,
    puzzle_orbit_definition,
    even_parity_constraints_helper,
):
    orbit = puzzle_orbit_definition.orbits[orbit_index]
    partition_objs = []
    for partition in integer_partitions(cycle_cubie_count):
        if s:
            partition = (1,) + partition
        lcm = math.lcm(*partition)
        order = lcm

        always_orient = None
        critical_orient = None
        if isinstance(orbit.orientation_status, OrientationStatus.CanOrient):
            orientation_count = orbit.orientation_status.count
            orientation_sum_constraint = orbit.orientation_status.sum_constraint
            max_p_adic_valuation = -1

            for j, permutation_order in enumerate(partition):
                curr_p_adic_valuation = p_adic_valuation(
                    permutation_order,
                    orientation_count,
                )
                if curr_p_adic_valuation > max_p_adic_valuation:
                    max_p_adic_valuation = curr_p_adic_valuation
                    critical_orient = [j]
                elif curr_p_adic_valuation == max_p_adic_valuation:
                    critical_orient.append(j)
                if permutation_order == 1:
                    if always_orient is None:
                        always_orient = [j]
                    else:
                        always_orient.append(j)

            match orientation_sum_constraint:
                case OrientationSumConstraint.NONE:
                    if critical_orient is not None:
                        order *= orientation_count
                case OrientationSumConstraint.ZERO:
                    orient_count = 0 if always_orient is None else len(always_orient)
                    critical_is_disjoint = critical_orient is not None and (
                        always_orient is None
                        or all(j not in always_orient for j in critical_orient)
                    )
                    if critical_is_disjoint:
                        orient_count += 1
                    unorient_critical = orient_count == len(partition) and (
                        orientation_count == 2
                        and orient_count % 2 == 1
                        or orientation_count > 2
                        and orient_count == 1
                    )
                    if unorient_critical:
                        if not critical_is_disjoint:
                            continue
                        assert len(critical_orient) == 1, critical_orient
                        critical_orient = None
                    # this is equivalent to len(partition) != 0
                    # how is an exercise left to the reader
                    elif orient_count != 0:
                        order *= orientation_count

        partition_objs.append(
            CubiePartition(
                name=orbit.name,
                partition=partition,
                order=order,
                always_orient=always_orient,
                critical_orient=critical_orient,
            )
        )

    partition_objs.sort(reverse=True, key=operator.attrgetter("order"))
    dominated = [False] * len(partition_objs)
    reduced_partition_objs = []
    for i in range(len(partition_objs)):
        if dominated[i]:
            continue
        curr_partition_obj = partition_objs[i]
        reduced_partition_objs.append(curr_partition_obj)
        for j in range(i + 1, len(partition_objs)):
            if (
                curr_partition_obj.order % partition_objs[j].order == 0
                and curr_partition_obj.order != partition_objs[j].order
                and (
                    not even_parity_constraints_helper.constraint_orbit_flags[
                        orbit_index
                    ]
                    or (
                        sign(curr_partition_obj.partition)
                        + sign(partition_objs[j].partition)
                    )
                    % 2
                    == 0
                )
            ):
                dominated[j] = True
    return reduced_partition_objs


def pareto_efficient_cycle_combinations(cycle_combination_objs):
    # This isnt the exact pareto efficient algorithm because I had trouble
    # getting it to work for some reason. The actual algorithm will be used in
    # the Rust verison of this code.
    cycle_combination_objs.sort(
        key=lambda cycle_combination_obj: (
            cycle_combination_obj.order_product,
            *map(operator.attrgetter("order"), cycle_combination_obj.cycle_combination),
        ),
        reverse=True,
    )
    pareto_points = []
    for maybe_redundant in cycle_combination_objs:
        if all(
            not cycle_combination_dominates(not_redundant, maybe_redundant)
            for not_redundant in pareto_points
        ):
            pareto_points.append(maybe_redundant)
    return pareto_points


def cycle_combination_objs_stats(cycle_combination_objs):
    stats = collections.defaultdict(int)
    for cycle_combination_obj in cycle_combination_objs:
        stats[
            tuple(
                map(
                    operator.attrgetter("order"),
                    cycle_combination_obj.cycle_combination,
                )
            )
        ] += len(cycle_combination_obj.share_orders)
    return dict(stats)


def main():
    start = timeit.default_timer()
    cycle_combinations = optimal_cycle_combinations(
        puzzle_orbit_definition=puzzle_orbit_definitions.PUZZLE_3x3,
        num_cycles=3,
    )
    print(timeit.default_timer() - start)
    print(recursive_shared_cycle_combinations.cache_info())
    print(highest_order_cycles_from_cubie_counts.cache_info())
    print(reduced_integer_partitions.cache_info())
    return cycle_combinations


if __name__ == "__main__":
    cycle_combination_objs = main()
    try:
        stats = cycle_combination_objs_stats(cycle_combination_objs)
    except Exception:
        stats = None
    with open("./output.py", "w") as f:
        f.write(
            f"Cycle = 1\nCycleCombination = 1\nCubiePartition = 1\n{stats}\n{cycle_combination_objs}"
        )
