import math

from sympy import primerange

def possible_orders1(piece_count):
    possible_orders = [{1} for _ in range(piece_count + 1)]
    count = 0
    for dst_piece_count in range(piece_count + 1):
        for adding_piece_count in range(1, dst_piece_count + 1):
            src_piece_count = dst_piece_count - adding_piece_count
            for src_possible_order in possible_orders[src_piece_count]:
                count += 1
                possible_orders[dst_piece_count].add(math.lcm(src_possible_order, adding_piece_count))
    return possible_orders, count


def possible_orders2(piece_count, orient_count):
    possible_orders = [{(1, 0, piece_count)} for _ in range(piece_count + 1)]
    count = 0
    for adding_piece_count in primerange(2, piece_count + 1):
        if type(adding_piece_count) is not int:
            exit()
        for dst_piece_count in range(piece_count, adding_piece_count - 1, -1):
            prime_power = adding_piece_count
            while prime_power <= dst_piece_count:
                src_piece_count = dst_piece_count - prime_power
                for (
                    src_possible_order,
                    oriented_count,
                    src_available_pieces,
                ) in possible_orders[src_piece_count]:
                    count += 1
                    possible_orders[dst_piece_count].add(
                        (
                            src_possible_order * prime_power,
                            oriented_count,
                            src_available_pieces - prime_power,
                        )
                    )
                    count += 1
                    possible_orders[dst_piece_count].add(
                        (
                            src_possible_order
                            * prime_power
                            * (orient_count if oriented_count == 0 else 1),
                            oriented_count + 1,
                            src_available_pieces - prime_power,
                        )
                    )
                prime_power *= adding_piece_count

    return list(
        sorted(
            set(
                possible_order
                for possible_order, oriented_count, available_pieces in possible_orders[
                    piece_count
                ]
                if available_pieces != 0
                or (
                    orient_count > 2
                    and oriented_count != 1
                    or orient_count == 2
                    and oriented_count % 2 != 1
                )
            )
        )
    ), count


a = possible_orders2(8, 4)
exp = [1, 2, 3, 4, 5, 6, 7, 8, 10, 12, 14, 15, 16, 20, 24, 28, 30, 40, 48, 60]
assert a[0] == exp, f"\n{a[0]}\n{exp}"
breakpoint()
# assert a[0] == corners, f"\n{a[0]}\n{corners}"
# a = possible_orders2(120, 2)
# assert a[0] == edges, f"\n{a[0]}\n{edges}"
