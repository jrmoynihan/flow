#!/usr/bin/env python3
"""Time ShobiStassen/PARC on a pre-written CSV matrix (load outside the timer).

Invoked by `compare_with_python` (Rust). Writes a small JSON object to --out-json.

Example:
  python compare_with_python.py --matrix case/matrix.csv --out-json case/py.json \\
    --knn 30 --warmup 0 --reps 1
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
from contextlib import redirect_stdout
from io import StringIO


def load_csv(path: str):
    import numpy as np

    rows = []
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            rows.append([float(x) for x in line.split(",")])
    if not rows:
        raise SystemExit(f"empty matrix: {path}")
    return np.asarray(rows, dtype=np.float64)


# Import once at process start so timed reps exclude module load.
from parc import PARC  # noqa: E402


def run_once(X, knn: int, seed: int):
    # Match Rust ParcConfig::default() / Python reference defaults.
    p = PARC(
        X,
        knn=knn,
        dist_std_local=3,
        jac_std_global="median",
        keep_all_local_dist="auto",
        too_big_factor=0.4,
        small_pop=10,
        n_iter_leiden=5,
        random_seed=seed,
        distance="l2",
        resolution_parameter=1.0,
        hnsw_param_ef_construction=150,
        num_threads=-1,
    )
    sink = StringIO()
    with redirect_stdout(sink):
        p.run_PARC()
    n_clusters = len(set(int(x) for x in p.labels))
    return n_clusters


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--matrix", required=True)
    ap.add_argument("--out-json", required=True)
    ap.add_argument("--knn", type=int, default=30)
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--warmup", type=int, default=0)
    ap.add_argument("--reps", type=int, default=1)
    args = ap.parse_args()

    X = load_csv(args.matrix)
    n, d = X.shape

    for _ in range(max(0, args.warmup)):
        run_once(X, args.knn, args.seed)

    times = []
    n_clusters = None
    for _ in range(max(1, args.reps)):
        t0 = time.perf_counter()
        n_clusters = run_once(X, args.knn, args.seed)
        times.append(time.perf_counter() - t0)

    times.sort()
    median = times[len(times) // 2]
    mean = sum(times) / len(times)
    payload = {
        "implementation": "python-parc",
        "parc_package": "parc",
        "n": n,
        "d": d,
        "knn": args.knn,
        "n_clusters": n_clusters,
        "reps": len(times),
        "secs_median": median,
        "secs_mean": mean,
        "secs_all": times,
        "evt_per_s_median": n / median if median > 0 else None,
        "python": sys.version.split()[0],
        "pid": os.getpid(),
    }
    try:
        import parc as parc_mod
        import importlib.metadata as md

        payload["parc_version"] = md.version("parc")
        payload["parc_file"] = getattr(parc_mod, "__file__", None)
    except Exception as e:  # noqa: BLE001
        payload["parc_version_error"] = str(e)

    with open(args.out_json, "w", encoding="utf-8") as f:
        json.dump(payload, f, indent=2)
        f.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
