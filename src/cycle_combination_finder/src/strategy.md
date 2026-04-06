Start off with 28: 2^2 * 7^1
Assume only edges exist

- Find an allocation of the multiset of {2^2, 7^1}
- The set {2^2, 7^1} must be allocated to the edges cycle 
- Find piece costs of 2^2 and 7^1
- 7^1 is not 2 or 3; piece cost is 7 (trivial)
    - subtract 7 from the 12 edges allocated: 5
- 2 is a special prime because the edges have 2 orientations.
    - We follow the cost sharing algorithm 2^n -> 2^(n - 1) + if shared { 1 } else { 0 }
        - 2^2 or greater is always better 
            - we have a factor of 2, which means parity can be bad because we have a 
        - 2^1 then we try both cases
            - embed the "shared register" check as this one
        - 2^0 we don't worry about allocating edge orientation. e.g. if the register order is 81 we dont want eo
            - we also have good parity
    - 2^2 -> 2^1 + 1 = 4 -> 3 but we don't count the last 1 because it's for shared last register
    - Hey, I notice that we have orientation! And that there is another cycle in this register without orientation. Therefore we make the 7 orient. If there is nothing that has another cycle, then we note down that we need a shared piece and continue with henry's recursive algorithm
    - subtract 2 from the 5 edges allocated: 3
- Check parity. We have a 7 cycle and a 2 cycle. We have odd parity
- (7+, 2+)
- add a 2 cycle now to fix parity

28
(1+, 2+, 2+, 7+)
(2, 2+, 7+)

- 1 cycle with orientation then you can merge

- we make it 0 pieces if we can combine it with perm

Strategy
- Do the 2s and 3s first because their value are the same as the orientations of orbits
- We only care about powers of 2 for parity since all other primes are odd which have even parity
    - Check in advance
- 14: 7 and 2. We can do the 2 cycle on edges as a 2 cycle permutation swap of edges (bad parity) or a 2 cycle orientation swap on 2 edges (good parity) (or on 1 edge with a shared edge)
