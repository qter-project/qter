#
#   Count the number of positions of the Rubik's cube of a particular order.
#
#   Do corners and edges separately, taking into account the orientations of
#   each, and then combine the results.
#
#   For simplicity we make use of the fact that the maximum order is 1260.
#   We do not attempt to make this particularly fast.
#
#   We use dynamic programming, and build a matrix that is
#
# (parity: 0..1) x (totp: 0..o-1) x (cubies:  0..n) x (order: 0..1260) -> count
#
#   Given a number of cubies and an orientation modulo, build an array as
#   described above.
#
import math

maxorder = 1260


def lcm(a, b):
    return a // math.gcd(a, b) * b


# I think this is "a choose b × number of ways to permute a things"; or the P(a, b) function we learned in CS182
def perm(a, b):
    r = 1
    for i in range(b):
        r = r * (a - i)
    return r


def order_of_just_permutations(thing_count, rotations):
    matrix_of_order_counts = [[], []]

    for parity in range(2):
        for first_rotation in range(rotations):
            matrix_of_order_counts[parity].append([])
            matrix_of_order_counts[parity][first_rotation].append([0])

    matrix_of_order_counts[0][0][0].append(1)

    for cycle_computing in range(1, thing_count + 1):
        for parity in range(2):
            matrix_of_order_counts[parity].append([])
            for first_rotation in range(rotations):
                matrix_of_order_counts[parity][first_rotation].append([])
                for composing_cycle in range(1, cycle_computing + 1):
                    for second_rotation in range(rotations):
                        rotation_composition = (
                            second_rotation + first_rotation
                        ) % rotations

                        order_contribution = composing_cycle
                        if second_rotation != 0:
                            order_contribution *= rotations
                        if composing_cycle % 2 == 0:
                            signature = 1 - parity
                        else:
                            signature = parity

                        cnt = perm(cycle_computing - 1, composing_cycle - 1) * pow(
                            rotations, composing_cycle - 1
                        )
                        for order in range(
                            1,
                            len(
                                matrix_of_order_counts[signature][rotation_composition][
                                    cycle_computing - composing_cycle
                                ]
                            ),
                        ):
                            new_order = lcm(order_contribution, order)

                            while new_order >= len(
                                matrix_of_order_counts[parity][first_rotation][
                                    cycle_computing
                                ]
                            ):
                                matrix_of_order_counts[parity][first_rotation][
                                    cycle_computing
                                ].append(0)

                            matrix_of_order_counts[parity][first_rotation][
                                cycle_computing
                            ][new_order] += (
                                cnt
                                * matrix_of_order_counts[signature][
                                    rotation_composition
                                ][cycle_computing - composing_cycle][order]
                            )

    return matrix_of_order_counts


edges = order_of_just_permutations(12, 2)
corners = order_of_just_permutations(8, 3)
totals = [0] * (maxorder + 1)

for p in range(2):
    for eo in range(1, len(edges[p][0][12])):
        if edges[p][0][12][eo] == 0:
            continue
        for co in range(1, len(corners[p][0][8])):
            v = lcm(eo, co)
            if corners[p][0][8][co] > 0 and v <= maxorder:
                totals[v] += edges[p][0][12][eo] * corners[p][0][8][co]
            elif corners[p][0][8][co] > 0:
                print("Unexpected", v, corners[p][0][8][co])

order_sum = 0
cube_states = 0

for order in range(len(totals)):
    cube_states += totals[order]

for order in range(len(totals)):
    if totals[order] > 0:
        order_sum += order * totals[order]
        percent = totals[order] * 100 / cube_states
        print(order, totals[order], percent)

order_sum_without_trivial = order_sum - 1 - 4 * 12 - 2 * 6
cube_states_without_trivial = cube_states - 19

g1 = math.gcd(order_sum, cube_states)
order_sum //= g1
cube_states //= g1

g2 = math.gcd(order_sum_without_trivial, cube_states_without_trivial)
order_sum_without_trivial //= g2
cube_states_without_trivial //= g2

print(order_sum, "/", cube_states)
print(order_sum / cube_states)
print(order_sum_without_trivial, "/", cube_states_without_trivial)
print(order_sum_without_trivial / cube_states_without_trivial)
