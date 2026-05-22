"""
Finds pairs of commutative cycles on a Rubik's cube that have high products of
orders.

This is more efficient version of the optimal CCF. The goal is to return
one structure for each combination, rather than every structure.
There are also a few more assumptions, and as such there may be some missed combinations.
"""
# TODO allow for orientation to be composite
import timeit
import collections
import copy
import math
import operator
import puzzle_orbit_definitions
from sympy import primerange
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


def prime_powers_below_n(n, max_orient):
    prime_powers = []

    for prime in primerange(n + 1):
        if len(max_orient) > prime and max_orient[prime] > 0:
            orient = prime
            piece_check = prime
            prime_powers.append(
                (
                    PrimePower(value=1, pieces=0),
                    PrimePower(value=prime, pieces=0),
                )
            )
        else:
            orient = 1
            piece_check = prime**2
            prime_powers.append(
                (
                    PrimePower(value=1, pieces=0),
                    PrimePower(value=prime, pieces=prime),
                )
            )

        while piece_check <= n:
            prime_powers[-1] += (
                PrimePower(value=orient * piece_check, pieces=piece_check),
            )
            piece_check *= prime
            if orient > 1 and piece_check > max_orient[prime]:
                piece_check *= orient
                orient = 1

    return prime_powers


def possible_order_list(total_pieces, partition_max, max_orient):
    prime_powers = prime_powers_below_n(partition_max, max_orient)

    paths = []
    stack = [[len(prime_powers) - 1, 0, 1, [], []]]

    while stack:
        i, piece_count, product, powers, pieces = stack.pop()
        if i == -1 or prime_powers[i][1].pieces + piece_count > total_pieces:
            paths.append(PrimeCombo(product, powers, sum(pieces), pieces))
            continue

        for p in prime_powers[i]:
            new_pieces = piece_count + p.pieces
            if (
                p.pieces > 0 and p.pieces % 2 == 0 and True
            ):  # False for 4x4, True else TODO fix this
                new_pieces += 2
            if new_pieces <= total_pieces:
                if p.value == 1:
                    stack.append([i - 1, new_pieces, product, powers, pieces])
                else:
                    stack.append(
                        [
                            i - 1,
                            new_pieces,
                            product * p.value,
                            powers + [p.value],
                            pieces + [p.pieces],
                        ]
                    )

    paths = sorted(paths, key=lambda x: x[0], reverse=True)

    return paths


def cycle_combo_test(registers, cycle_cubie_counts, puzzle_orbit_definition):
    stack = [
        [
            0,
            0,
            [0] * len(cycle_cubie_counts),
            [[[] for y in cycle_cubie_counts] for x in registers],
            sum(cycle_cubie_counts) - sum([sum(x.piece_counts) for x in registers]),
        ]
    ]
    loops = 0
    while stack:
        loops += 1
        if loops == 10000:
            return None
        r, p, orbit_sums, assignments, available_pieces = stack.pop()
        # print(len(stack), r)
        seen = []

        while p == len(registers[r].values):
            p = 0
            r += 1
            if r == len(registers):
                break

        if r == len(registers):
            return assignments
        # TODO no duplicates

        for i, orbit in enumerate(puzzle_orbit_definition.orbits):
            if orbit.orientation_status == OrientationStatus.CannotOrient:
                if cycle_cubie_counts[i] in seen:
                    continue
                else:
                    seen.append(cycle_cubie_counts[i])

            if (
                isinstance(
                    orbit.orientation_status,
                    OrientationStatus.CanOrient,
                )
                and registers[r].values[p] % orbit.orientation_status.count == 0
            ):
                new_cycle = registers[r].piece_counts[p]
                new_available = available_pieces
            elif (
                registers[r].values[p] - registers[r].piece_counts[p]
                <= available_pieces
            ):
                new_cycle = registers[r].values[p]
                new_available = (
                    available_pieces
                    - registers[r].values[p]
                    + registers[r].piece_counts[p]
                )
            else:
                continue
            if new_cycle == 0 and len(assignments[r][i]) == 0:
                if available_pieces == 0:
                    continue
                new_cycle = 1

            parity = 0
            if new_cycle % 2 == 0 and new_cycle > 0:
                parity = 2

            if new_cycle + parity + orbit_sums[i] <= cycle_cubie_counts[i]:
                stack.append(
                    [
                        r,
                        p + 1,
                        orbit_sums.copy(),
                        copy.deepcopy(assignments),
                        new_available,
                    ]
                )
                stack[-1][2][i] += new_cycle
                if new_cycle > 0:
                    stack[-1][3][r][i].append(new_cycle)
                    if parity > 0:
                        stack[-1][2][i] += 2
                        stack[-1][3][r][i].append(2)


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
    cycle_combinations = sorted(
        cycle_combinations, key=lambda x: x.order_product, reverse=True
    )
    print(timeit.default_timer() - start)
    return cycle_combinations


if __name__ == "__main__":
    cycle_combination_objs = main()
    try:
        stats = cycle_combination_objs_stats(cycle_combination_objs)
    except Exception:
        stats = None
    with open("./output_equivalent.py", "w") as f:
        f.write(
            f"Cycle = 1\nCycleCombination = 1\nCubiePartition = 1\n{stats}\n{cycle_combination_objs}"
        )
