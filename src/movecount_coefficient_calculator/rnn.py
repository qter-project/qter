import torch.nn as nn
import torch
from algs_dataset import full_cube

i = lambda b: 1 if b else 0

# 3 for direction, 2 for rotation, 4 for type indicator
move_params = 3 + 2 + 4


class RNNModel(nn.Module):
    def __init__(self, recurrent: int, thickness: int):
        super().__init__()

        self.initial_state = nn.Parameter(torch.randn(recurrent))
        self.recurrent_stack = nn.Sequential(
            nn.Linear(move_params + recurrent, thickness),
            nn.ReLU(),
            nn.Linear(thickness, thickness),
            nn.ReLU(),
            nn.Linear(thickness, recurrent + 1),
        )

    def forward(self, alg):
        state = self.initial_state
        energy = 0

        energies = []

        for move in alg:
            flip = 1

            # There are two different ways to represent any full cube rotation; randomly choose between them during training to force the model to learn that symmetry
            if self.training and move[1] == full_cube and (torch.randint(0, 2, (1,)) == 0).item():
                flip = -1

            input = torch.tensor(
                [
                    move[0][0] * flip,
                    move[0][1] * flip,
                    move[0][2] * flip,
                    i(move[1] == 0),
                    i(move[1] == 1),
                    i(move[1] == 2),
                    i(move[1] == 3),
                    # cosine is even
                    move[2][0],
                    # sine is odd
                    move[2][1] * flip,
                ]
            )

            out = self.recurrent_stack.forward(torch.cat((state, input)))

            energies.append(out[0])

            state = state + out[1:]

        return energies
