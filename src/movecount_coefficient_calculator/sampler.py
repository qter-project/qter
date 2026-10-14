import torch
import math
from algs_dataset import options

def random(leng):
    sample = []

    for _ in range(leng):
        i = torch.randint(0, len(options), (1,)).item()
        sample.append(options[i])

    return sample
    

def metropolis(sample, model, iterations):
    with torch.no_grad():
        energy = sum(model.forward(sample)).item()
        
        # How many iterations of metropolis to do
        for _ in range(iterations):
            # Pick an index to mutate
            i = torch.randint(0, len(sample), (1,)).item()
            # Pick a move to swap in
            j = torch.randint(0, len(options), (1,)).item()

            current = sample[i]
            sample[i] = options[j]

            new_energy = sum(model.forward(sample)).item()

            if new_energy < energy:
                # keep it
                energy = new_energy
                continue

            if math.exp(-new_energy) / math.exp(-energy) > torch.rand((1,)).item():
                # keep it
                energy = new_energy
                continue

            # otherwise discard the proposed transition
            sample[i] = current

class Sampler:
    def __init__(self, num_per_len):
        self.samples = {}
        self.num_per_len = num_per_len

    def sample(self, leng, model):
        if not leng in self.samples:
            samples = []

            for _ in range(self.num_per_len):
                samples.append(random(leng))

            for sample in samples:
                metropolis(sample, model, leng * len(options))

            self.samples[leng] = (0, samples)

        which, samples = self.samples[leng]

        for sample in samples:
            metropolis(sample, model, leng)

        which += 1

        if which == self.num_per_len:
            which = 0

        self.samples[leng] = (which, samples)

        return samples[which]

