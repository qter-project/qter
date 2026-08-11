"""
Finds pairs of commutative cycles on a Rubik's cube that have high products of
orders.

This is more efficient version of the optimal CCF. The goal is to return
one structure for each combination, rather than every structure.
There are also a few more assumptions, and as such there may be some missed combinations.
"""

import collections
import copy
import math
import operator
import timeit

from sympy import primerange

import puzzle_orbit_definitions
from common_types import OrientationStatus  # , OrientationSumConstraint

CycleCombination = collections.namedtuple(
    "CycleCombination",
    [
        "used_cubie_counts",
        "order_product",
        # "share_orders", assuming this is always true
        "cycle_combination",
    ],
)

Cycle = collections.namedtuple(
    "Cycle",
    [
        "order",
        # "share", assuming this is always true
        "partition_objs",
    ],
)

CubiePartition = collections.namedtuple(
    "CubiePartition",
    [
        "name",
        "partition",
        "order",
        # "always_orient", assuming this is always true
        # "critical_orient", assuming this is always true
    ],
)

PrimePower = collections.namedtuple(
    "PrimePower",
    ["value", "pieces"],
)

PrimeCombo = collections.namedtuple(
    "PrimePower",
    ["order", "values", "piece_total", "piece_counts"],
)


def recursive_cycle_combinations(
    remaining_pieces,
    remaining_registers,
    possible_orders,
    min_indices,
    registers,
    cycle_cubie_counts,
    puzzle_orbit_definition,
    cycle_combinations,
    prior_index,
):
    if remaining_registers == 1:
        max_used = True
        for order in possible_orders[max(prior_index, min_indices[remaining_pieces]) :]:
            if len(registers) > 0 and order.order > registers[-1].order:
                continue

            assignments = cycle_combo_test(
                registers + [order], cycle_cubie_counts, puzzle_orbit_definition
            )
            if assignments is not None:
                registers = registers + [order]
                cycle_combination = []
                for r, reg in enumerate(registers):
                    partitions = []
                    for o, orbit in enumerate(puzzle_orbit_definition.orbits):
                        lcm = 1
                        for a in assignments[r][o]:
                            lcm = math.lcm(lcm, a)
                        if isinstance(
                            orbit.orientation_status, OrientationStatus.CanOrient
                        ):
                            lcm *= (
                                orbit.orientation_status.count
                            )  # TODO fix this, it's not always accurate
                            assignments[r][o] = [1] + assignments[r][o]

                        partitions.append(
                            CubiePartition(
                                orbit.name,
                                assignments[r][o],
                                lcm,
                            )
                        )
                    cycle_combination.append(Cycle(reg.order, partitions))
                new_combo = CycleCombination(
                    used_cubie_counts=cycle_cubie_counts,
                    order_product=math.prod(x.order for x in registers),
                    cycle_combination=cycle_combination,  # shouldn't need to sort with new version
                    # sorted(
                    # cycle_combination, key=lambda x: x.order, reverse=True
                    # ),
                )

                for c in range(len(cycle_combinations) - 1, -1, -1):
                    if cycle_combination_dominates(cycle_combinations[c], new_combo):
                        return cycle_combinations, max_used
                    elif cycle_combination_dominates(new_combo, cycle_combinations[c]):
                        cycle_combinations.pop(c)
                cycle_combinations.append(new_combo)
                return cycle_combinations, max_used
            max_used = False

        return cycle_combinations, False

    max_tracker = True
    minimum_checked = remaining_pieces
    minimum_maxxed = remaining_pieces + 1

    for o in range(
        max(min_indices[remaining_pieces], prior_index), len(possible_orders)
    ):
        minimum_checked = min(minimum_checked, possible_orders[o].piece_total)
        if possible_orders[o].piece_total >= minimum_maxxed:
            continue
        if len(registers) == 0:
            print("Checking first register", possible_orders[o].order)
        # if len(registers) > 0 and possible_orders[o].order > registers[-1].order:
        # continue
        cycle_combination, max_used = recursive_cycle_combinations(
            remaining_pieces - possible_orders[o].piece_total,
            remaining_registers - 1,
            possible_orders,
            min_indices,
            registers + [possible_orders[o]],
            cycle_cubie_counts,
            puzzle_orbit_definition,
            cycle_combinations,
            o,
        )
        if max_used and minimum_checked == 0:
            return cycle_combinations, max_tracker
        elif max_used:
            minimum_maxxed = possible_orders[o].piece_total
            o = min(min_indices[minimum_maxxed - 1] - 1, o)
        else:
            max_tracker = False

    return cycle_combinations, max_tracker


def efficient_cycle_combinations(puzzle_orbit_definition, num_registers):
    cycle_cubie_counts = ()
    max_orient = [0] * 4
    for orbit in puzzle_orbit_definition.orbits:
        if isinstance(orbit.orientation_status, OrientationStatus.CanOrient):
            max_orient[orbit.orientation_status.count] = orbit.cubie_count
            cycle_cubie_counts = cycle_cubie_counts + (orbit.cubie_count - 1,)
        else:
            cycle_cubie_counts = cycle_cubie_counts + (orbit.cubie_count,)

    total_cubies = sum(cycle_cubie_counts)
    possible_orders = possible_order_list(
        total_cubies, max(cycle_cubie_counts), max_orient
    )

    min_indices = [len(possible_orders)] * (total_cubies + 1)
    for i, order in enumerate(possible_orders):
        for pieces in range(order.piece_total, total_cubies + 1):
            if i > min_indices[pieces]:
                break
            min_indices[pieces] = i

    return recursive_cycle_combinations(
        total_cubies,
        num_registers,
        possible_orders,
        min_indices,
        [],
        cycle_cubie_counts,
        puzzle_orbit_definition,
        [],
        0,
    )


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
        ]
    return dict(stats)


def cycle_combination_dominates(this, other):
    if other.order_product > this.order_product:
        return False
    for this_cycle, other_cycle in zip(this.cycle_combination, other.cycle_combination):
        if other_cycle.order > this_cycle.order:
            return False

    return True


def main():
    start = timeit.default_timer()
    cycle_combinations, _ = efficient_cycle_combinations(
        puzzle_orbit_definition=puzzle_orbit_definitions.PUZZLE_4x4,
        num_registers=2,
    )
    cycle_combinations = sorted(
        cycle_combinations, key=lambda x: x.order_product, reverse=True
    )
    print(timeit.default_timer() - start)
    return cycle_combinations


if __name__ == "__main__":
    cycle_combination_objs = main()
    stats = cycle_combination_objs_stats(cycle_combination_objs)
    with open("./output_efficient.py", "w") as f:
        f.write(
            f"Cycle = 1\nCycleCombination = 1\nCubiePartition = 1\n{stats}\n{cycle_combination_objs}"
        )
