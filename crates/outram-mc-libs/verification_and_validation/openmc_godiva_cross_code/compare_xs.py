#!/usr/bin/env python3
"""Difference this crate's reconstructed cross sections against the NJOY-produced
ACE that OpenMC reads -- the DATA half of the Godiva investigation.

`outram-mc-libs` sits +250 +/- 51 pcm above OpenMC on nominally identical nuclear
data. Either the data is not actually identical, or the offset is in transport.
This separates the two before any neutron is transported.

Reports, per nuclide and reaction:
  * the FLUX-WEIGHTED mean relative difference over a Watt fission spectrum,
    which is what an eigenvalue responds to,
  * the worst pointwise relative difference above 1 keV, for context only.

Pointwise agreement inside individual resonances is NOT expected and is not
chased: two independently thinned grids never line up there.

WHICH REACTIONS, AND WHY NOT "TOTAL". Only the channels the transport kernel
actually partitions on are compared -- elastic, fission, capture, inelastic,
(n,2n) -- plus nu-bar, which enters k directly. A "total" comparison was tried
first and dropped: these ACE files carry NO MT=1, and MT=4 only for U-235, so any
total has to be hand-assembled from partials, and the comparison then measures
the assembly rather than the data. The first version of this script did exactly
that and reported a spurious +28% for U-238 because its fallback sum silently
omitted every inelastic level.

    WORK_DIR=... python3 compare_xs.py ours.csv
"""
import os, sys, math, collections
import numpy as np
import openmc.data

WORK = os.environ.get("WORK_DIR", "./work")
ours_csv = sys.argv[1] if len(sys.argv) > 1 else "ours.csv"
T = "294K"

rows = collections.defaultdict(list)
with open(ours_csv) as f:
    hdr = f.readline().strip().split(",")
    for line in f:
        p = line.strip().split(",")
        rows[p[0]].append([float(v) for v in p[1:]])
COLS = hdr[2:]
IDX = {c: i for i, c in enumerate(COLS)}

def watt(e, a=0.988e6, b=2.249e-6):
    return math.exp(-e / a) * math.sinh(math.sqrt(b * e))

print(f"{'nuclide':8s} {'quantity':12s} {'flux-wtd rel diff':>19s} {'worst >1keV':>13s} "
      f"{'at E (eV)':>12s}")
print("-" * 72)

flagged = []
for nuc_name, data in rows.items():
    arr = np.array(data)
    E = arr[:, 0]
    nuc = openmc.data.IncidentNeutron.from_hdf5(f"{WORK}/{nuc_name}.h5")
    w = np.array([watt(e) for e in E])

    def xs(mts):
        out = np.zeros_like(E)
        for mt in mts:
            if mt in nuc.reactions:
                out = out + nuc.reactions[mt].xs[T](E)
        return out

    # Total nu-bar = every neutron product on MT=18 (prompt + each delayed group).
    def nubar():
        if 18 not in nuc.reactions:
            return None
        tot = np.zeros_like(E)
        for p in nuc.reactions[18].products:
            if p.particle == "neutron":
                tot = tot + p.yield_(E)
        return tot

    inelastic_mts = [mt for mt in range(51, 92) if mt in nuc.reactions]
    theirs = {
        "elastic": xs([2]),
        "fission": xs([18]) if 18 in nuc.reactions else xs([19, 20, 21, 38]),
        "inelastic": xs(inelastic_mts),
        "n2n": xs([16]),
    }
    nb = nubar()
    if nb is not None:
        theirs["nu_fission"] = theirs["fission"] * nb
    # our "absorption" is fission + capture, so compare capture via the difference
    theirs["absorption"] = theirs["fission"] + xs([102])

    for q, their in theirs.items():
        if q not in IDX:
            continue
        mine = arr[:, 1 + IDX[q]]
        ok = (their > 0) & np.isfinite(their) & np.isfinite(mine)
        if not np.any(ok):
            continue
        rel = np.zeros_like(E)
        rel[ok] = (mine[ok] - their[ok]) / their[ok]
        fw = float(np.sum(w[ok] * np.abs(rel[ok])) / np.sum(w[ok]))
        fw_signed = float(np.sum(w[ok] * rel[ok]) / np.sum(w[ok]))
        hi = ok & (E > 1.0e3)
        j = int(np.argmax(np.abs(rel * hi))) if np.any(hi) else 0
        mark = "  <<<" if fw > 0.002 else ""
        if fw > 0.002:
            flagged.append((nuc_name, q, fw_signed))
        print(f"{nuc_name:8s} {q:12s} {fw_signed:+19.4%} {rel[j]:+13.2%} {E[j]:12.3e}{mark}")

print()
print("Flux-weighted differences above 0.2% are flagged: a ~0.1% error in a "
      "dominant\nreaction is roughly 100 pcm of reactivity.")
if flagged:
    print("\nFLAGGED:")
    for n, q, v in flagged:
        print(f"  {n} {q}: {v:+.3%} flux-weighted")
else:
    print("\nNothing above 0.2% flux-weighted. The DATA is not the explanation "
          "for the\n+250 pcm -- the offset is in TRANSPORT.")
