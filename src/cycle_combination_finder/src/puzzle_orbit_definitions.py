from common_types import (
    PuzzleOrbitDefinition,
    OrientationSumConstraint,
    OrientationStatus,
    Orbit,
    EvenParityConstraint,
)


def cube(N):
    # start with corners since all sized cubes N>1 have 8 corners
    orbits = [
        Orbit(
            name="corners",
            cubie_count=8,
            orientation_status=OrientationStatus.CanOrient(
                count=3,
                sum_constraint=OrientationSumConstraint.ZERO,
            ),
        ),
    ]
    even_parity_constraints = []

    if N % 2 == 1:
        # if N is odd, the cube has 12 edges and the edge parity is equivalent to corner parity
        orbits.append(
            Orbit(
                name="edges",
                cubie_count=12,
                orientation_status=OrientationStatus.CanOrient(
                    count=2,
                    sum_constraint=OrientationSumConstraint.ZERO,
                ),
            ),
        )
        even_parity_constraints.append(
            EvenParityConstraint(
                orbit_names=(
                    "edges",
                    "corners",
                ),
            ),
        )

        # if N is odd, the cube has N//2 - 1 sets of 24 centers. these are called +centers since they form a + shape
        # each has parity determined by the corners and the wings it shares a slice with
        for c2 in range(1, N // 2):
            orbits.append(
                Orbit(
                    name=f"+centers{c2}",
                    cubie_count=24,
                    orientation_status=OrientationStatus.CannotOrient(),
                ),
            )
            even_parity_constraints.append(
                EvenParityConstraint(
                    orbit_names=(
                        "corners",
                        f"wings{c2}",
                        f"+centers{c2}",
                    ),
                ),
            )

    # the cube has N//2 - 1 sets of 24 wings.
    for w in range(1, N // 2):
        orbits.append(
            Orbit(
                name=f"wings{w}",
                cubie_count=24,
                orientation_status=OrientationStatus.CannotOrient(),
            ),
        )

    # the cube has (N//2 - 1)^2 sets of 24 centers.
    for c1 in range(1, N // 2):
        for c2 in range(1, N // 2):
            # the centers with equal indices are called xcenters since they form an x shape
            # xcenter parity is only determined by the corners, since the associated wing parity doubles and therefore always cancels out
            if c1 == c2:
                orbits.append(
                    Orbit(
                        name=f"xcenters{c1}",
                        cubie_count=24,
                        orientation_status=OrientationStatus.CannotOrient(),
                    ),
                )
                even_parity_constraints.append(
                    EvenParityConstraint(
                        orbit_names=(
                            "corners",
                            f"xcenters{c1}",
                        ),
                    ),
                )

            # the other centers are called obliques, they fall on a skewed slope from the cube's sides
            # oblique parity is determined by the corners, and both sets of wings that it shares a slice with
            else:
                orbits.append(
                    Orbit(
                        name=f"obliques{c1};{c2}",
                        cubie_count=24,
                        orientation_status=OrientationStatus.CannotOrient(),
                    ),
                )
                even_parity_constraints.append(
                    EvenParityConstraint(
                        orbit_names=(
                            "corners",
                            f"wings{c1}",
                            f"wings{c2}",
                            f"obliques{c1};{c2}",
                        ),
                    ),
                )

    return PuzzleOrbitDefinition(
        orbits=tuple(orbits),
        even_parity_constraints=tuple(even_parity_constraints),
    )


def minx(N):
    # start with corners since all sized minxes N>1 have 20 corners
    orbits = [
        Orbit(
            name="corners",
            cubie_count=20,
            orientation_status=OrientationStatus.CanOrient(
                count=3,
                sum_constraint=OrientationSumConstraint.ZERO,
            ),
        ),
    ]
    even_parity_constraints = []

    # all piece types on the minxes must have even parity since every move induces only 5-cycles
    even_parity_constraints.append(
        EvenParityConstraint(
            orbit_names=("corners",),
        ),
    )

    if N % 2 == 1:
        # if N is odd, the minx has 30 edges
        orbits.append(
            Orbit(
                name="edges",
                cubie_count=30,
                orientation_status=OrientationStatus.CanOrient(
                    count=2,
                    sum_constraint=OrientationSumConstraint.ZERO,
                ),
            ),
        )
        even_parity_constraints.append(
            EvenParityConstraint(
                orbit_names=("edges",),
            ),
        )

        # if N is odd, the minx has N//2 - 1 sets of 60 +centers
        for c2 in range(1, N // 2):
            orbits.append(
                Orbit(
                    name=f"+centers{c2}",
                    cubie_count=60,
                    orientation_status=OrientationStatus.CannotOrient(),
                ),
            )
            even_parity_constraints.append(
                EvenParityConstraint(
                    orbit_names=(f"+centers{c2}",),
                ),
            )

    # the minx has N//2 - 1 sets of 60 wings.
    for w in range(1, N // 2):
        orbits.append(
            Orbit(
                name=f"wings{w}",
                cubie_count=60,
                orientation_status=OrientationStatus.CannotOrient(),
            ),
        )
        even_parity_constraints.append(
            EvenParityConstraint(
                orbit_names=(f"wings{w}",),
            ),
        )

    # the minx has (N//2 - 1)^2 sets of 60 centers.
    for c1 in range(1, N // 2):
        for c2 in range(1, N // 2):
            # the centers with equal indices are called xcenters, following from the cube naming
            if c1 == c2:
                orbits.append(
                    Orbit(
                        name=f"xcenters{c1}",
                        cubie_count=60,
                        orientation_status=OrientationStatus.CannotOrient(),
                    ),
                )
                even_parity_constraints.append(
                    EvenParityConstraint(
                        orbit_names=(f"xcenters{c1}",),
                    ),
                )

            # the other centers are called obliques
            else:
                orbits.append(
                    Orbit(
                        name=f"obliques{c1};{c2}",
                        cubie_count=60,
                        orientation_status=OrientationStatus.CannotOrient(),
                    ),
                )
                even_parity_constraints.append(
                    EvenParityConstraint(
                        orbit_names=(f"obliques{c1};{c2}",),
                    ),
                )

    return PuzzleOrbitDefinition(
        orbits=tuple(orbits),
        even_parity_constraints=tuple(even_parity_constraints),
    )


PUZZLE_4x4 = cube(4)
PUZZLE_2x2 = cube(2)
PUZZLE_3x3 = cube(3)
PUZZLE_5x5 = cube(5)
PUZZLE_6x6 = cube(6)
PUZZLE_KILOMINX = minx(2)
PUZZLE_MEGAMINX = minx(3)
PUZZLE_MASTERKILOMINX = minx(4)
PUZZLE_GIGAMINX = minx(5)
