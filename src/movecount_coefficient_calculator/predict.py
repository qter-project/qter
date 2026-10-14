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
            total_energy = sum(model(alg))

        energy = total_energy.item() if isinstance(total_energy, torch.Tensor) else total_energy
        print(energy, flush=True)


if __name__ == "__main__":
    main()
