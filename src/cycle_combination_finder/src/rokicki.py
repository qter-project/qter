#
#   Compute the set of achievable orders of 3x3 Rubik's Cube positions.
#
#   We do edges and corners separately, taking into account:
#     - permutation parity
#     - total orientation sum modulo 2 (edges) or 3 (corners)
#     - order of the resulting permutation-with-orientation
#
#   Then we combine edge and corner possibilities:
#     - edge parity must equal corner parity
#     - edge orientation sum must be 0 mod 2
#     - corner orientation sum must be 0 mod 3
#     - full order = lcm(edge_order, corner_order)
#
#   This version computes only WHICH orders are possible, not how many states
#   realize each order.
#

import math
import pprint

MAX_ORDER = 1260


def lcm(a, b):
    return a // math.gcd(a, b) * b


def possible_orders_for_piece_type(piece_count, orientation_count):
    """
    General DP for a piece system with:
      - piece_count pieces
      - orientation_count orientations per piece (mod orientation_count)

    Returns:
      dp[parity][orientation_sum][used] = set of reachable orders

    where:
      parity = 0 (even), 1 (odd)
      orientation_sum = total orientation sum mod orientation_count
      used = number of pieces accounted for

    This enumerates abstract cycle structures, not exact labeled-piece counts.
    """

    dp = [
        [[set() for _ in range(piece_count + 1)] for _ in range(orientation_count)]
        for _ in range(2)
    ]

    # Base case: empty decomposition
    dp[0][0][0].add(1)

    # Build up by adding one cycle at a time
    for pieces in range(1, piece_count + 1):
        for target_parity in range(2):
            for target_orient_sum in range(orientation_count):
                recorder = dp[target_parity][target_orient_sum][pieces]

                # Add one new cycle of length cycle_len
                for cycle_len in range(1, pieces + 1):
                    # A cycle of length k has permutation parity (k - 1) mod 2
                    cycle_parity = (cycle_len - 1) % 2

                    # Therefore previous parity must be:
                    prev_parity = target_parity ^ cycle_parity

                    prev_used = pieces - cycle_len

                    # The cycle can have any net orientation sum mod orientation_count
                    for cycle_orient_sum in range(orientation_count):
                        prev_orient_sum = (
                            target_orient_sum - cycle_orient_sum
                        ) % orientation_count

                        # Order contribution of this cycle
                        #
                        # For the standard "orientation sum around the cycle" model:
                        # - if cycle_orient_sum == 0, contribution = cycle_len
                        # - else contribution = cycle_len * orientation_count / gcd(orientation_count, cycle_orient_sum)
                        #
                        # For M = 2 or 3, this reduces to:
                        # - cycle_len if zero
                        # - cycle_len * M if nonzero
                        if cycle_orient_sum == 0:
                            cycle_order = cycle_len
                        else:
                            cycle_order = (
                                cycle_len
                                * orientation_count
                                // math.gcd(orientation_count, cycle_orient_sum)
                            )

                        for prev_order in dp[prev_parity][prev_orient_sum][prev_used]:
                            recorder.add(lcm(prev_order, cycle_order))

    return dp


# Build edge and corner possibilities
edges = possible_orders_for_piece_type(60, 10)
corners = possible_orders_for_piece_type(40, 20)

# Combine them into full cube orders
seen = set()
for parity in range(2):
    for edge_order in edges[parity][0][60]:
        for corner_order in corners[parity][0][40]:
            full_order = lcm(edge_order, corner_order)
            seen.add(full_order)

orders = sorted(seen)

# pprint.pprint(orders)
print()
print("Number of distinct achievable orders:", len(orders))
