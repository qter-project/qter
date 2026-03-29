from math import gcd

from sympy import divisors, primerange


def possible_orbit_orders(N, O):
    if N == 1:
        return {1}
    if N <= 0:
        return set()
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
        for d in range(0, N, gcd(N, O)):
            if d != 1:
                try:
                    all.remove(N * d)
                except:
                    pass
    return all
