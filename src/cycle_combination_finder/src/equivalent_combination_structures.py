"""
Finds pairs of commutative cycles on a Rubik's cube that have high products of
orders.

This is more efficient version of the optimal CCF. The goal is to return
one structure for each combination, rather than every structure.
There are also a few more assumptions, and as such there may be some missed combinations.
"""

# TODO allow for orientation to be composite
import collections
import copy
import math
import operator
import sys
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


def assignments_to_combo(
    assignments, registers, cycle_cubie_counts, puzzle_orbit_definition
):
    cycle_combination = []
    for r, reg in enumerate(registers):
        partitions = []
        for o, orbit in enumerate(puzzle_orbit_definition.orbits):
            lcm = 1
            for a in assignments[r][o]:
                lcm = math.lcm(lcm, a)
            if isinstance(orbit.orientation_status, OrientationStatus.CanOrient):
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
    return CycleCombination(
        used_cubie_counts=cycle_cubie_counts,
        order_product=math.prod(x.order for x in registers),
        cycle_combination=cycle_combination,
    )


def efficient_cycle_combinations(puzzle_orbit_definition, num_registers):
    cycle_cubie_counts = ()
    max_orient = [0] * 4
    for orbit in puzzle_orbit_definition.orbits:
        if isinstance(orbit.orientation_status, OrientationStatus.CanOrient):
            max_orient[orbit.orientation_status.count] = orbit.cubie_count - 1
            cycle_cubie_counts = cycle_cubie_counts + (orbit.cubie_count - 1,)
        else:
            cycle_cubie_counts = cycle_cubie_counts + (orbit.cubie_count,)

    total_cubies = sum(cycle_cubie_counts)
    cubies_per_register = total_cubies // num_registers
    possible_orders = possible_order_list(
        cubies_per_register,
        min(max(cycle_cubie_counts), cubies_per_register),
        max_orient,
    )

    for order in possible_orders:
        print("testing order", order.order)

        unorientable_excess = 0
        for o in range(len(order.values) - 1, -1, -1):
            if order.values[o] % 2 == 0:
                orientable = min(
                    max_orient[2] // max(1, order.piece_counts[o]), num_registers
                )
                unorientable_excess += (num_registers - orientable) * (
                    order.values[o] - order.piece_counts[o]
                )
            elif order.values[o] % 3 == 0:
                orientable = min(
                    max_orient[3] // max(1, order.piece_counts[o]), num_registers
                )
                unorientable_excess += (num_registers - orientable) * (
                    order.values[o] - order.piece_counts[o]
                )
            else:
                break

        if unorientable_excess + num_registers * sum(order.piece_counts) > total_cubies:
            continue

        assignments = cycle_combo_test(
            [order] * num_registers, cycle_cubie_counts, puzzle_orbit_definition
        )
        if assignments is not None:
            return [
                assignments_to_combo(
                    assignments,
                    [order] * num_registers,
                    cycle_cubie_counts,
                    puzzle_orbit_definition,
                )
            ]
    sys.exit()


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
    cycle_combinations = efficient_cycle_combinations(
        puzzle_orbit_definition=puzzle_orbit_definitions.PUZZLE_6x6,
        num_registers=3,
    )
    cycle_combinations.sort(key=lambda x: x.order_product, reverse=True)
    print(timeit.default_timer() - start)
    return cycle_combinations


if __name__ == "__main__":
    cycle_combination_objs = main()
    stats = cycle_combination_objs_stats(cycle_combination_objs)
    with open("./output_equivalent.py", "w") as f:
        f.write(
            f"Cycle = 1\nCycleCombination = 1\nCubiePartition = 1\n{stats}\n{cycle_combination_objs}"
        )
