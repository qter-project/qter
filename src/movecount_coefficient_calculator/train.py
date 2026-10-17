import math
from pathlib import Path
import torch.nn as nn
import torch
from torch.utils.data import DataLoader
from torch.utils.data.dataset import random_split
import rnn
import altair as alt
import polars as pl
from sampler import Sampler
from algs_dataset import datasets, options, process_alg


def train_loop(dataset, model, optimizer, sampler):
    model.train()

    batch_total = 0

    count = 0

    for epoch in range(2):
        for i, algs in enumerate(
            random_split(dataset, [1/300 for _ in range(300)])
        ):
            total_energy = 0
            total_symbols = 0

            for alg in algs:
                energies_1 = model.forward(alg)
                t_1 = sum(energies_1)
                total_energy += t_1
                (t_1).backward(retain_graph=True)
                batch_total -= t_1.item()

                s = sampler.sample(len(alg), model)
                energies_2 = model.forward(s)
                t_2 = sum(energies_2)
                total_energy += t_2
                (-t_2).backward(retain_graph=True)
                batch_total += t_2.item()

                total_symbols += len(alg) * 2
                # total_symbols += len(alg)

            ((total_energy / total_symbols) ** 2 * 1000).backward()

            optimizer.step()
            optimizer.zero_grad()

            count += 1

            # loss = loss.item()
            # total += loss
            # batch_total += loss

            # if i % 5 == 0:
            if True:
                print(f"count: {count}; differentiation: {batch_total:>7f}; total symbols: {total_symbols}; avg energy: {total_energy / total_symbols}")
                batch_total = 0


def test_loop(train, test, model):
    model.eval()

    with torch.no_grad():
        total = 0

        for alg in train:
            energies = model.forward(alg)

            total += sum(energies)

        print(f"Train avg energy: {total/len(train):>7f}")

        total = 0

        for alg in test:
            energies = model.forward(alg)

            total += sum(energies)

        print(f"Test avg energy: {total/len(test):>7f}")


train, test = datasets()
print(len(train), len(test))
model = rnn.RNNModel(recurrent=80, thickness=100)
print(sum(p.numel() for p in model.parameters()))

sampler = Sampler(10)

# with torch.no_grad():
#     s = sample(10, model)
#     print(s)
#     e = model.forward(s)
#     print(f"Energies {e}")
#     print(f"Energy: {sum(e)}")

optimizer = torch.optim.Adam(model.parameters(), lr=1e-3)

train_loop(train, model, optimizer, sampler)

torch.save(
    {
        "recurrent": 80,
        "thickness": 100,
        "state_dict": model.state_dict(),
    },
    Path(__file__).with_name("model.pth"),
)

print(model.initial_state)

with torch.no_grad():
    a = model.forward(process_alg("L  F2 B' R2 D2 F' R' U  R' U  F'"))
    print(f"Scores: {a}; Total: {sum(a)}")
    b = model.forward(process_alg("U2 R  U' R2 F  R2 F' U  R  U2 R2"))
    print(f"Scores: {b}; Total: {sum(b)}")

test_loop(train, test, model)

# alt.Chart(pl.DataFrame({
#     "len": map(lambda v: len(v), algs())
# })).mark_bar().encode(
#     alt.X("len"),
#     alt.Y("count()"),
# ).save("/tmp/chart.png")
