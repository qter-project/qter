#!/usr/bin/env python3
"""Collect algorithms from the F2L and ZBLL CSV sheets."""

import csv
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parent
INPUT_DIRS = ("f2l-sheet", "zbll-sheet")
OUTPUT = ROOT / "algs.txt"

# Move tokens used in the sheets, including rotations and wide moves.
MOVE = re.compile(r"(?:[URFDLBMESurfdlbxyz](?:w)?(?:2|')?|\((?:U|U'|U2)\))")


def is_algorithm(line: str) -> bool:
    tokens = line.split()
    return len(tokens) >= 2 and all(MOVE.fullmatch(token) for token in tokens)


def algorithms_in_cell(cell: str, remove_bracketed_u: bool, remove_initial_f2l_u: bool) -> list[str]:
    found = []
    for line in cell.splitlines():
        line = line.strip()
        if not is_algorithm(line):
            continue
        if remove_bracketed_u:
            line = re.sub(r"\s*\((?:U|U'|U2)\)\s*", " ", line).strip()
            line = re.sub(r"\s+", " ", line)
        elif remove_initial_f2l_u:
            line = re.sub(r"^\((?:U|U'|U2)\)\s*", "", line)
        line = line.replace("(", "").replace(")", "")
        if line:
            found.append(line)
    return found


def collect() -> list[str]:
    result = []
    for directory in INPUT_DIRS:
        for path in sorted((ROOT / directory).glob("*.csv")):
            with path.open(newline="", encoding="utf-8-sig") as stream:
                rows = list(csv.reader(stream))

            # ZBLL files have a header naming the 3x3 and OH algorithm columns.
            # We don't care about one-handed algorithms so we ignore those
            alg_columns = None
            if directory == "zbll-sheet":
                for row in rows:
                    three_by_three = [i for i, name in enumerate(row) if "algorithm" in name.lower() and "oh" not in name.lower()]
                    if three_by_three:
                        alg_columns = three_by_three
                        break

            for row in rows:
                cells = row
                if alg_columns is not None:
                    cells = [row[i] for i in alg_columns if i < len(row)]
                for cell in cells:
                    result.extend(algorithms_in_cell(
                        cell,
                        remove_bracketed_u=directory == "zbll-sheet",
                        remove_initial_f2l_u=directory == "f2l-sheet",
                    ))
    return result


if __name__ == "__main__":
    OUTPUT.write_text("\n".join(collect()) + "\n", encoding="utf-8")
