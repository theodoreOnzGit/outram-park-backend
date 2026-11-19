#!/usr/bin/env python3
"""Runs the REAL PRISMS-Fatigue functions (commit 2c8fc9a2, unmodified) on a
deterministic synthetic per-element history and dumps inputs and outputs as
IEEE-754 bit patterns, for the Rust comparison test (gh:#1434, case 2).

Usage (inside a venv with numpy and pandas):
    python -I fip_driver.py <PRISMS-Fatigue>/src <scratch dir> | grep -E "^[PIFDBN] " > reference_fip.txt
(upstream prints its own progress lines to stdout; the grep drops them)

Nothing from upstream is copied or edited: `calculate_FIPs.import_PRISMS_data`
and `volume_average_FIPs.Al7075_band_averaging` are imported and called.
Output lines (all floats are u64 bit patterns in hex):
  P <k> <sigma_y>
  I <elem> <sys> <slip_tension> <slip_compression> <normal_tension>   (as pandas loaded them)
  F <elem> <sys> <fip>                                                (upstream result)
  D <grain> <plane> <layer> <e1,e2,..>                                (band definition, 1-based elements)
  B <grain> <slip_system> <layer> <band_averaged_fip>                 (upstream result)
  N <count of CSV values pandas parsed differently from correctly rounded float()>
"""
import os
import pickle
import struct
import sys

sys.path.insert(0, sys.argv[1])
scratch = os.path.abspath(sys.argv[2])
os.makedirs(scratch, exist_ok=True)

import numpy as np
import pandas as pd
import calculate_FIPs as cf
import volume_average_FIPs as vf


def bits(x):
    return "%016x" % struct.unpack("<Q", struct.pack("<d", float(x)))[0]


# Deterministic generator (same on every machine): 64-bit LCG, top 53 bits.
state = [0x9E3779B97F4A7C15]


def u01():
    state[0] = (state[0] * 6364136223846793005 + 1442695040888963407) & 0xFFFFFFFFFFFFFFFF
    return (state[0] >> 11) / float(1 << 53)


NX = NY = NZ = 3
NEL = NX * NY * NZ
NSYS = 12
K, SIGMA_Y = 10.0, 517.0


def make_file(name, tension):
    rows = []
    for e in range(NEL):
        x, y, z = e % NX, (e // NX) % NY, e // (NX * NY)
        row = [float(e // 9), float(x), float(y), float(z)]
        for s in range(NSYS):  # plastic shear strain on each slip system
            base = 1.0e-3 * (u01() - 0.5)
            row.append(base + (3.0e-3 * (u01() + 0.2) if tension else 0.0))
        for s in range(NSYS):  # stress normal to slip plane, +-600 MPa
            row.append(1200.0 * (u01() - 0.5) if tension else 0.0)
        for s in range(9):
            row.append(1.0e-3 * (u01() - 0.5))
        row.append(1.0e-3 * u01())
        rows.append(row)
    with open(os.path.join(scratch, name), "w") as f:
        for row in rows:
            f.write(",".join("%.17g" % v for v in row) + "\n")


make_file("MaxComp_0.csv", False)
make_file("MaxTen_0.csv", True)

# Upstream run, unmodified.
cf.import_PRISMS_data(scratch, np.asarray([NX, NY, NZ]), 0, "FS_FIP", NSYS, [K, SIGMA_Y], False, False)
fips = pickle.load(open(os.path.join(scratch, "PRISMS_FS_FIP_0.p"), "rb"))

# The values pandas loaded (same call as upstream makes), and the parse count.
tens = pd.read_csv(os.path.join(scratch, "MaxTen_0.csv"), index_col=False, header=None)
comp = pd.read_csv(os.path.join(scratch, "MaxComp_0.csv"), index_col=False, header=None)
differ = 0
for name, df in (("MaxTen_0.csv", tens), ("MaxComp_0.csv", comp)):
    with open(os.path.join(scratch, name)) as f:
        for r, line in enumerate(f):
            for c, text in enumerate(line.strip().split(",")):
                if float(text) != df.iloc[r, c]:
                    differ += 1

print("P %s %s" % (bits(K), bits(SIGMA_Y)))
for e in range(NEL):
    for s in range(NSYS):
        print("I %d %d %s %s %s" % (e, s, bits(tens.iloc[e, 4 + s]), bits(comp.iloc[e, 4 + s]), bits(tens.iloc[e, 16 + s])))
        print("F %d %d %s" % (e, s, bits(fips[e, s])))

# Band averaging on 8 grains, 4 planes, 2 layers per plane; deterministic subsets.
NGR, NPL, NLAY = 8, 4, 2
number_of_layers = np.full((NGR, NPL), NLAY, dtype=int)
band_sets = {}
for g in range(NGR):
    for p in range(NPL):
        for l in range(NLAY):
            members = sorted({1 + int(u01() * NEL) % NEL for _ in range(7)})
            band_sets[g, p, l] = members
            print("D %d %d %d %s" % (g, p, l, ",".join(str(m) for m in members)))
vf.Al7075_band_averaging(fips, "FS_FIP", number_of_layers, band_sets, 0, NGR, scratch)
avg = pickle.load(open(os.path.join(scratch, "band_averaged_FS_FIP_pickle_0.p"), "rb"))
for (g, s, l), v in sorted(avg.items()):
    print("B %d %d %d %s" % (g, s, l, bits(v)))
print("N %d" % differ)
