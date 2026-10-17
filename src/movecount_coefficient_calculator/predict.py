from pathlib import Path
import sys

import torch

import rnn
from algs_dataset import process_alg


def main():
    model_path = Path(__file__).with_name("model.pth")
    checkpoint = torch.load(model_path, map_location="cpu", weights_only=True)

    model = rnn.RNNModel(
        recurrent=checkpoint["recurrent"],
        thickness=checkpoint["thickness"],
    )
    model.load_state_dict(checkpoint["state_dict"])
    model.eval()

    for line in sys.stdin:
        alg = process_alg(line)
        with torch.no_grad():
            move_energies = model(alg)

        energies = [energy.item() if isinstance(energy, torch.Tensor) else energy for energy in move_energies]
        total_energy = sum(energies)
        labels = line.split()
        if len(labels) != len(energies):
            labels = [f"move {index + 1}" for index in range(len(energies))]

        for label, energy in zip(labels, energies):
            print(f"{label:>12}  {energy:>12.6f}")
        print(f"{'Total':>12}  {total_energy:>12.6f}", flush=True)


if __name__ == "__main__":
    main()
