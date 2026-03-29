from math import gcd

from sympy import divisors, primerange


def possible_orbit_orders(N, O):
    if N <= 1:
        return {1}
    dp = [[] for _ in range(N + 1)]
    dp[0].append(1)
    for prime in primerange(N + 1):
        if not isinstance(prime, int):
            raise
        for i in range(N, prime - 1, -1):
            prime_power = prime
            while prime_power <= i:
                dp[i] += [s * prime_power for s in dp[i - prime_power]]
                prime_power *= prime
    all = {o * f for d in divisors(O) for s in dp for o in s for f in (1, d)}
    if N in dp[N]:
        for d in range(gcd(N, O), N, gcd(N, O)):
            if d != 1 and O % d == 0:
                all.remove(N * d)
    return all


assert len(possible_orbit_orders(120, 20)) == 99622
assert len(possible_orbit_orders(113, 20)) == 73860
assert len(possible_orbit_orders(64, 20)) == 6222
assert len(possible_orbit_orders(120, 13)) == 75770
assert len(possible_orbit_orders(113, 13)) == 55880
assert len(possible_orbit_orders(64, 13)) == 4526
assert len(possible_orbit_orders(120, 16)) == 89594
assert len(possible_orbit_orders(113, 16)) == 66402
assert len(possible_orbit_orders(64, 16)) == 5534
