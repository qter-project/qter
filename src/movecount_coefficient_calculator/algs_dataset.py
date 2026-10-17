from torch.utils.data import Dataset
from sklearn.model_selection import train_test_split
from scipy.spatial.transform import Rotation
import math

# turn type indicators
face = 0
slice = 1
wide = 2
full_cube = 3

# minecraft coordinates
u_dir = (0, 1, 0)
d_dir = (0, -1, 0)
r_dir = (1, 0, 0)
l_dir = (-1, 0, 0)
f_dir = (0, 0, -1)
b_dir = (0, 0, 1)

turn_types = {
    "U": (u_dir, face),
    "D": (d_dir, face),
    "R": (r_dir, face),
    "L": (l_dir, face),
    "F": (f_dir, face),
    "B": (d_dir, face),
    "Uw": (u_dir, wide),
    "Dw": (d_dir, wide),
    "Rw": (r_dir, wide),
    "Lw": (l_dir, wide),
    "Fw": (f_dir, wide),
    "Bw": (b_dir, wide),
    "u": (u_dir, wide),
    "d": (d_dir, wide),
    "r": (r_dir, wide),
    "l": (l_dir, wide),
    "f": (f_dir, wide),
    "b": (b_dir, wide),
    "M": (l_dir, slice),
    "E": (d_dir, slice),
    "S": (f_dir, slice),
    "x": (r_dir, full_cube),
    "y": (u_dir, full_cube),
    "z": (f_dir, full_cube),
}

turns = [(0, -1), (-1, 0), (0, 1)]

rt2 = math.sqrt(2)
rt3 = math.sqrt(3)

options = (
    [
        (dir, ind, turn)
        for dir in [u_dir, d_dir, l_dir, r_dir, f_dir, b_dir]
        for ind in [face, wide]
        for turn in turns
    ]
    + [(dir, slice, turn) for dir in [d_dir, l_dir, f_dir] for turn in turns]
    # Face rotations
    + [
        (dir, full_cube, turn)
        for dir in [u_dir, d_dir, l_dir, r_dir, f_dir, b_dir]
        for turn in [(0, -1), (-1, 0)]
    ]
    # Corner rotations
    + [
        (
            (dir[0] / rt3, dir[1] / rt3, dir[2] / rt3),
            full_cube,
            (math.cos(angle), math.sin(angle)),
        )
        for dir in [(1, 1, 1), (-1, 1, 1), (-1, 1, -1), (1, 1, -1)]
        for angle in [math.tau / 3, -math.tau / 3]
    ]
    # Edge rotations
    + [
        (
            (dir[0] / rt2, dir[1] / rt2, dir[2] / rt2),
            full_cube,
            (-1, 0),
        )
        for dir in [(0, 1, 1), (1, 0, 1), (1, 1, 0)]
    ]
)


def process_alg(alg: str):
    alg = alg.strip()

    moves = []

    for move in alg.split(" "):
        if move == "":
            continue

        letter = move
        # 90° clockwise
        rotation = (0, -1)

        if move[-1] == "2":
            # 180°
            rotation = (-1, 0)
            letter = move[0:-1]
        elif move[-1] == "'":
            # 90° counterclockwise
            rotation = (0, 1)
            letter = move[0:-1]

        axis, type = turn_types[letter]

        if len(moves) != 0 and moves[-1][1] == full_cube and type == full_cube:
            # Compose two rotations together
            previous_axis, _, previous_rotation = moves.pop()

            # The stored pair is (cos(angle), sin(angle)).
            previous_angle = math.atan2(
                previous_rotation[1], previous_rotation[0]
            )
            angle = math.atan2(rotation[1], rotation[0])
            previous = Rotation.from_rotvec(
                tuple(
                    component * previous_angle for component in previous_axis
                )
            )
            current = Rotation.from_rotvec(
                tuple(component * angle for component in axis)
            )

            # Rotation multiplication applies the right operand first.
            composed_rotvec = (current * previous).as_rotvec()
            composed_angle = math.sqrt(
                sum(component**2 for component in composed_rotvec)
            )

            # Don't append an identity rotation
            if composed_angle > 1e-12:
                composed_axis = tuple(
                    component / composed_angle for component in composed_rotvec
                )
                composed_rotation = (
                    math.cos(composed_angle),
                    math.sin(composed_angle),
                )
                moves.append((composed_axis, full_cube, composed_rotation))
        else:
            moves.append((axis, type, rotation))

    return moves


def datasets(test_size=0.2):
    f = open("./training-data/curated/algs.txt")
    algs = f.readlines()

    train, test = train_test_split(algs, test_size=test_size)

    return AlgsDataset(train), AlgsDataset(test)


class AlgsDataset(Dataset):
    def __init__(self, algs):
        processed_algs = []

        for alg in algs:
            processed_algs.append(process_alg(alg))

        self.algs = processed_algs

    def __len__(self):
        return len(self.algs)

    def __getitem__(self, idx: int):
        return self.algs[idx]
