from math import gcd

from sympy import divisors, primerange


def possible_orbit_orders(N: int, O: int):
    if N <= 1:
        return {1}

    primes = list(primerange(N + 1))
    divs = list(divisors(O))
    even_parity_orders = set()
    odd_parity_orders = set()
    n_prime_power = False

    stack = [(0, N, 1)]

    c = 0
    while len(stack):
        c += 1
        i, remaining, prod = stack.pop()
        if i == len(primes):
            odd_parity = prod % 2 == 0
            if odd_parity:
                for d in divs:
                    odd_parity_orders.add(prod * d)
                if remaining >= 2:
                    for d in divs:
                        even_parity_orders.add(prod * d)
            else:
                for d in divs:
                    even_parity_orders.add(prod * d)
                if remaining == 2:
                    prod *= 2
                    for d in divs:
                        odd_parity_orders.add(prod * d)
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
        for d in range(g, N + g, g):
            if d != 1 and O % d == 0:
                odd_parity_orders.remove(N * d)
                even_parity_orders.remove(N * d)

    return even_parity_orders, odd_parity_orders


# [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 14, 15, 16, 18, 20, 21, 22, 24, 28, 30, 35, 36, 40, 42, 48, 56, 60, 70, 84, 120]
# {1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 14, 15, 16, 18, 20, 21, 22, 24, 28, 30, 35, 36, 40, 42, 48, 56, 60, 70, 84, 120}

print([len(i) for i in possible_orbit_orders(120, 2)])
print([len(i) for i in possible_orbit_orders(120, 3)])
print([len(i) for i in possible_orbit_orders(120, 4)])
print([len(i) for i in possible_orbit_orders(120, 6)])
# print(len(possible_orbit_orders(64, 2)))
# print(len(possible_orbit_orders(113, 2)))
assert len(possible_orbit_orders(120, 20)) == 99622
# assert len(possible_orbit_orders(113, 20)) == 73860
# assert len(possible_orbit_orders(64, 20)) == 6222
# assert len(possible_orbit_orders(120, 13)) == 75770
# assert len(possible_orbit_orders(113, 13)) == 55880
# assert len(possible_orbit_orders(64, 13)) == 4526
# assert len(possible_orbit_orders(120, 16)) == 89594
# assert len(possible_orbit_orders(113, 16)) == 66402
# assert len(possible_orbit_orders(64, 16)) == 5534
