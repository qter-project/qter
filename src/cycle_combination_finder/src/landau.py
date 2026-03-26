import itertools
import math

from sympy import primerange

edges = [
    1,
    2,
    3,
    4,
    5,
    6,
    7,
    8,
    9,
    10,
    11,
    12,
    14,
    15,
    16,
    18,
    20,
    21,
    22,
    24,
    28,
    30,
    35,
    40,
    42,
    60,
    70,
]

corners = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 15, 18, 21, 30, 36, 45]

# def possible_orders1(piece_count):
#     possible_orders = [{1} for _ in range(piece_count + 1)]
#     count = 0
#     for dst_piece_count in range(piece_count + 1):
#         for adding_piece_count in range(1, dst_piece_count + 1):
#             src_piece_count = dst_piece_count - adding_piece_count
#             for src_possible_order in possible_orders[src_piece_count]:
#                 count += 1
#                 possible_orders[dst_piece_count].add(math.lcm(src_possible_order, adding_piece_count))
#     return possible_orders, count


def possible_orders2(piece_count, orient_count):
    possible_orders = [{(1, 0)} for _ in range(piece_count + 1)]
    count = 0
    for adding_piece_count in itertools.chain([1], primerange(2, piece_count + 1)):
        for dst_piece_count in range(piece_count, adding_piece_count - 1, -1):
            prime_power = adding_piece_count
            while prime_power <= dst_piece_count:
                src_piece_count = dst_piece_count - prime_power
                for src_possible_order, oriented_count in possible_orders[
                    src_piece_count
                ]:
                    if oriented_count == 0:
                        count += 1
                        possible_orders[dst_piece_count].add(
                            (
                                src_possible_order * prime_power,
                                0,
                            )
                        )
                        count += 1
                        possible_orders[dst_piece_count].add(
                            (
                                src_possible_order * prime_power * orient_count,
                                oriented_count + 1,
                            )
                        )
                    else:
                        count += 1
                        possible_orders[dst_piece_count].add(
                            (
                                src_possible_order * prime_power,
                                oriented_count,
                            )
                        )
                        count += 1
                        possible_orders[dst_piece_count].add(
                            (
                                src_possible_order * prime_power,
                                oriented_count + 1,
                            )
                        )
                prime_power *= adding_piece_count
                if adding_piece_count == 1:
                    break
    return list(
        sorted(
            set(
                possible_order
                for possible_order, oriented_count in itertools.chain(
                    possible_orders[piece_count],
                )
                if oriented_count != 1
            )
        )
    ), count


a = possible_orders2(8, 3)
assert a[0] == corners, f"\n{a[0]}\n{corners}"
a = possible_orders2(12, 2)
assert a[0] == edges, f"\n{a[0]}\n{edges}"
