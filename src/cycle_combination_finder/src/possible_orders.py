from math import gcd

from sympy import divisors, primerange

# def possible_orbit_orders(N, O):
#     if N <= 1:
#         return {1}
#     dp = [[] for _ in range(N + 1)]
#     dp[0].append(1)
#     for prime in primerange(N + 1):
#         if not isinstance(prime, int):
#             raise
#         for i in range(N, prime - 1, -1):
#             prime_power = prime
#             while prime_power <= i:
#                 # dp[i] += [s * prime_power for s in dp[i - prime_power]]
#                 dp[i].append((prime_power, i - prime_power))
#                 if i == N and prime_power == 9:
#                     breakpoint()
#                 prime_power *= prime
#     dp2 = [[1]]
#     for sub in dp[1:]:
#         act = []
#         for prime_power, s in sub:
#             if prime_power in [2, 4, 8]:
#                 base = 2
#             elif prime_power in [3, 9]:
#                 base = 3
#             else:
#                 base = prime_power
#             if prime_power
#             act.extend(dp2[s])
#         dp2.append(act)
#     # breakpoint()
#     all = {o * f for d in divisors(O) for s in dp2 for o in s for f in (1, d)}
#     if N in dp2[N]:
#         for d in range(gcd(N, O), N, gcd(N, O)):
#             if d != 1 and O % d == 0:
#                 all.remove(N * d)
#     return all


def possible_orbit_orders(N: int, O: int):
    if N <= 1:
        return {1}

    primes = list(primerange(N + 1))
    divs = divisors(O)
    out = set()
    n_prime_power = False

    stack = [(0, N, 1)]

    while len(stack):
        i, remaining, prod = stack.pop()
        if i == len(primes):
            for d in divs:
                out.add(prod * d)
            continue

        p = primes[i]
        if not isinstance(p, int):
            raise

        stack.append((i + 1, remaining, prod))
        pp = p
        while pp <= remaining:
            if pp == N:
                n_prime_power = True
            stack.append((i + 1, remaining - pp, prod * pp))
            pp *= p

    if n_prime_power:
        g = gcd(N, O)
        for d in range(g, N, g):
            if d != 1 and O % d == 0:
                out.remove(N * d)

    return out


possible_orbit_orders(12, 2)

assert len(possible_orbit_orders(120, 20)) == 99622
assert len(possible_orbit_orders(113, 20)) == 73860
assert len(possible_orbit_orders(64, 20)) == 6222
assert len(possible_orbit_orders(120, 13)) == 75770
assert len(possible_orbit_orders(113, 13)) == 55880
assert len(possible_orbit_orders(64, 13)) == 4526
assert len(possible_orbit_orders(120, 16)) == 89594
assert len(possible_orbit_orders(113, 16)) == 66402
assert len(possible_orbit_orders(64, 16)) == 5534
