#!/usr/bin/env python3
"""Collect algorithms from the F2L and ZBLL CSV sheets."""

import csv
import re
from pathlib import Path
from collections.abc import Callable


ROOT = Path(__file__).resolve().parent
OUTPUT = ROOT / "algs.txt"
ONE_LLL_CSV = ROOT / "1LLL V6.xlsx - 1LLL List.csv"

# Move tokens used in the sheets, including rotations and wide moves.
MOVE = re.compile(r"(?:[URFDLBMESurfdlbxyz](?:w)?(?:2|')?|\((?:U|U'|U2)\))")


def is_algorithm(line: str) -> bool:
    tokens = line.split()
    return len(tokens) >= 2 and all(MOVE.fullmatch(token) for token in tokens)


def algorithms_in_cell(cell: str, remove_bracketed_u: bool, remove_initial_f2l_u: bool) -> list[str]:
    found = []
    for line in cell.splitlines():
        line = line.replace("\u00a0", " ").strip()
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


def collect_f2l(folder) -> list[str]:
    def collect_file(path: Path) -> list[str]:
        with path.open(newline="", encoding="utf-8-sig") as stream:
            rows = list(csv.reader(stream))

        result = []
        for row in rows:
            for cell in row:
                result.extend(algorithms_in_cell(
                    cell,
                    remove_bracketed_u=False,
                    remove_initial_f2l_u=True,
                ))
        return result

    return collect_folder(folder, collect_file)


def collect_zbll() -> list[str]:
    def collect_file(path: Path) -> list[str]:
        with path.open(newline="", encoding="utf-8-sig") as stream:
            rows = list(csv.reader(stream))

        # ZBLL files have a header naming the 3x3 and OH algorithm columns.
        # We don't care about one-handed algorithms so we ignore those.
        alg_columns = None
        for row in rows:
            three_by_three = [i for i, name in enumerate(row) if "algorithm" in name.lower() and "oh" not in name.lower()]
            if three_by_three:
                alg_columns = three_by_three
                break

        result = []
        for row in rows:
            cells = [row[i] for i in alg_columns if i < len(row)]
            for cell in cells:
                result.extend(algorithms_in_cell(
                    cell,
                    remove_bracketed_u=True,
                    remove_initial_f2l_u=False,
                ))
        return result

    return collect_folder("zbll-sheet", collect_file)


def collect_1lll() -> list[str]:
    with ONE_LLL_CSV.open(newline="", encoding="utf-8-sig") as stream:
        rows = csv.reader(stream)
        algorithm_column = None
        algorithms = []
        for row in rows:
            if algorithm_column is None:
                algorithm_column = next(
                    (i for i, name in enumerate(row) if name.strip().lower() == "algorithm"),
                    None,
                )
                continue

            if algorithm_column >= len(row):
                continue
            algorithm = row[algorithm_column].strip()
            algorithm = re.sub(r"^(?:\(U(?:'|2)?\)\s*)+", "", algorithm)
            if algorithm:
                algorithms.append(algorithm)
    return algorithms


def collect_vls() -> list[str]:
    def collect_file(path: Path) -> list[str]:
        with path.open(newline="", encoding="utf-8-sig") as stream:
            rows = csv.reader(stream)

            result = []
            for row in rows:
                for cell in row:
                    for line in cell.splitlines():
                        line = re.sub(r"\([^)]*\)", "", line).strip()
                        line = re.sub(r"\s+", " ", line)
                        if is_algorithm(line):
                            result.append(line)
            return result

    return collect_folder("vls-sheet", collect_file)


def collect_roux() -> list[str]:
    algorithms = []
    for path in sorted((ROOT / "roux-sheet").glob("*.txt")):
        with path.open(encoding="utf-8-sig") as stream:
            for line in stream:
                line = line.replace("’", "'").replace("\u00a0", " ").strip()
                if ":" in line:
                    line = line.rsplit(":", 1)[1]

                run = []
                for token in line.split():
                    token = token.strip("(),")
                    if MOVE.fullmatch(token):
                        run.append(token)
                    else:
                        if len(run) >= 2:
                            algorithms.append(" ".join(run))
                        run = []
                if len(run) >= 2:
                    algorithms.append(" ".join(run))
    return algorithms


def collect_folder(directory: str, collect_file: Callable[[Path], list[str]]) -> list[str]:
    result = []
    for path in sorted((ROOT / directory).glob("*.csv")):
        result.extend(collect_file(path))
    return result


def collect() -> list[str]:
    return list(dict.fromkeys(collect_f2l("f2l-sheet") + collect_zbll() + collect_1lll() + collect_vls() + collect_roux() + collect_f2l("zzls-sheet") + collect_f2l("ltct-sheet")))


if __name__ == "__main__":
    OUTPUT.write_text("\n".join(collect()) + "\n", encoding="utf-8")
