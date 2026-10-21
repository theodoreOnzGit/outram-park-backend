#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Compare, case by case, what OpenMC's Model.plot drew and coloured
(openmc_reference/) against what outram-mc-libs' emitted scripts draw and
colour (outram/): every PNG pixel (RGBA) and every entry of the id map's cell
and material channels. The cell-instance channel is not compared: Model.plot
never reads it.

Usage: python compare.py      (run the Rust test with OUTRAM_PLOT_SCRIPT_OUT=outram first)
"""
import os
import subprocess
import sys

import matplotlib.image as mpimg
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
REF, OURS = os.path.join(HERE, "openmc_reference"), os.path.join(HERE, "outram")

cases = sorted(f[:-3] for f in os.listdir(OURS) if f.endswith(".py"))
bad = 0
print(f"{'case':<32} {'pixels':>8} {'differ':>7} {'cell ids':>9} {'mat ids':>8}")
for c in cases:
    idmap = os.path.join(OURS, c + "_idmap.npy")
    subprocess.run([sys.executable, os.path.join(OURS, c + ".py"), os.path.join(OURS, c + ".png")],
                   check=True, env={**os.environ, "OUTRAM_IDMAP_OUT": idmap})
    a, b = mpimg.imread(os.path.join(OURS, c + ".png")), mpimg.imread(os.path.join(REF, c + ".png"))
    px = int(np.any(a != b, axis=-1).sum()) if a.shape == b.shape else -1
    ia, ib = np.load(idmap), np.load(os.path.join(REF, c + "_idmap.npy"))
    dc = int((ia[..., 0] != ib[..., 0]).sum())
    dm = int((ia[..., 2] != ib[..., 2]).sum())
    note = ""
    if dc and c == "region_union":
        # Region.plot makes a cell with an AUTO id; only the partition must agree.
        same = len(set(zip(ia[..., 0].ravel(), ib[..., 0].ravel()))) == len(np.unique(ib[..., 0]))
        note = " (auto cell id; partition identical)" if same else " (partition differs)"
        dc = 0 if same else dc
    print(f"{c:<32} {a.shape[0]*a.shape[1]:>8} {px:>7} {dc:>9} {dm:>8}{note}")
    bad += (px != 0) + (dc != 0) + (dm != 0)
print("ALL IDENTICAL" if bad == 0 else f"{bad} MISMATCHES")
sys.exit(1 if bad else 0)
