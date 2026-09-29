#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0
#
# OpenMC's transport channel list and fission cross section for ACE tables,
# the reference for GitHub #366 (outram-mc ACE route sampled MT=4 with Q = 0
# instead of the inelastic levels, and lost U-234's partial-only fission).
#
# Own work (outram-park-backend), GPL-3.0. Uses the OpenMC Python API (MIT) as
# a library only.
#
# For each table this writes:
#   channels,<lib>,<nuclide>,<space-separated MTs>
#       the reactions OpenMC samples: every reaction `IncidentNeutron.from_ace`
#       does NOT mark redundant (openmc/data/neutron.py:634-640 marks a
#       reaction redundant whenever any SUM_RULES component is present), minus
#       elastic (MT=2, sampled separately) and the always-derived 203-207/444.
#   fission,<lib>,<nuclide>,<E eV>,<sigma_f barn>
#       the fission cross section OpenMC transports: MT=18 when it is not
#       redundant, else the sum of the non-redundant partials 19/20/21/38,
#       each evaluated with OpenMC's own Tabulated1D at 294 K.
#   chimean,<lib>,U234,<E eV>,<mixture mean>,<MT=19-only mean>
#       mean prompt fission-neutron energy [eV] under OpenMC's sampling:
#       partial k is picked with probability sigma_k(E)/sum sigma (OpenMC
#       samples a fission reaction by its cross section), then its
#       ContinuousTabular law is sampled (src/distribution_energy.cpp:
#       statistical interpolation between incident tables i, i+1 with lin-lin
#       r, then envelope scaling). The mean of that scheme is exact:
#       sum_l w_l [E_1 + (m_l - E_{l,1}) (E_K - E_1)/(E_{l,K} - E_{l,1})],
#       m_l the table's own mean (histogram or lin-lin, no discrete lines).
#       The MT=19-only column is what taking the first fission law would give.
#
# Usage:  openmc-env; python transport_channels_reference.py <out.csv> <lib>=<dir> ...
#   where <dir> holds U234.ace, U235.ace, U238.ace (plain or .gz).

import gzip
import os
import shutil
import sys
import tempfile

import openmc
import openmc.data as od

NUCLIDES = ["U234", "U235", "U238"]
PROBE_EV = [2.53e-2, 1.0e3, 1.0e6, 2.0e6, 7.0e6, 1.4e7]
CHI_EV = [1.0e6, 7.0e6, 1.4e7, 1.9e7]


def table_mean(eo):
    import numpy as np
    x, p = np.asarray(eo.x), np.asarray(eo.p)
    assert getattr(eo, "_n_discrete", 0) == 0
    if eo.interpolation == "histogram":
        return float(np.sum(0.5 * (x[1:] + x[:-1]) * p[:-1] * np.diff(x))
                     / np.sum(p[:-1] * np.diff(x)))
    assert eo.interpolation == "linear-linear", eo.interpolation
    # integral of E p(E) for linear p on each segment, normalised
    h = np.diff(x)
    m1 = np.sum(h * (x[:-1] * (2 * p[:-1] + p[1:]) + x[1:] * (p[:-1] + 2 * p[1:])) / 6.0)
    m0 = np.sum(h * (p[:-1] + p[1:]) / 2.0)
    return float(m1 / m0)


def ct_mean(ct, e):
    import numpy as np
    assert len(ct.interpolation) <= 1 and (len(ct.interpolation) == 0
                                            or ct.interpolation[0] == 2)
    g = np.asarray(ct.energy)
    i = int(np.clip(np.searchsorted(g, e, side="right") - 1, 0, len(g) - 2))
    r = float(np.clip((e - g[i]) / (g[i + 1] - g[i]), 0.0, 1.0))
    a, b = ct.energy_out[i], ct.energy_out[i + 1]
    e1 = a.x[0] + r * (b.x[0] - a.x[0])
    ek = a.x[-1] + r * (b.x[-1] - a.x[-1])
    out = 0.0
    for w, t in ((1 - r, a), (r, b)):
        if w > 0:
            out += w * (e1 + (table_mean(t) - t.x[0]) * (ek - e1) / (t.x[-1] - t.x[0]))
    return out


def load(path):
    if path.endswith(".gz"):
        tmp = tempfile.NamedTemporaryFile(suffix=".ace", delete=False)
        with gzip.open(path, "rb") as src:
            shutil.copyfileobj(src, tmp)
        tmp.close()
        try:
            return od.IncidentNeutron.from_ace(tmp.name)
        finally:
            os.unlink(tmp.name)
    return od.IncidentNeutron.from_ace(path)


def main(out, specs):
    rows = []
    for spec in specs:
        lib, d = spec.split("=", 1)
        for n in NUCLIDES:
            p = os.path.join(d, n + ".ace")
            if not os.path.exists(p):
                p += ".gz"
            data = load(p)
            temp = list(data.energy)[0]
            mts = sorted(mt for mt, rx in data.reactions.items()
                         if not rx.redundant and mt != 2
                         and mt not in (203, 204, 205, 206, 207, 444))
            rows.append(f"channels,{lib},{n}," + " ".join(map(str, mts)))
            fis = [mt for mt in (18, 19, 20, 21, 38)
                   if mt in data.reactions and not data.reactions[mt].redundant]
            for e in PROBE_EV:
                s = sum(float(data.reactions[mt].xs[temp](e)) for mt in fis)
                rows.append(f"fission,{lib},{n},{e:.6e},{s:.12e}")
            print(lib, n, "channels", len(mts), "fission from", fis)
            if n == "U234":
                for e in CHI_EV:
                    ws = [float(data.reactions[mt].xs[temp](e)) for mt in fis]
                    ms = []
                    for mt in fis:
                        prompt = [p for p in data.reactions[mt].products
                                  if p.particle == "neutron"
                                  and p.emission_mode in ("prompt", "total")][0]
                        ms.append(ct_mean(prompt.distribution[0].energy, e))
                    mix = sum(w * m for w, m in zip(ws, ms)) / sum(ws)
                    rows.append(f"chimean,{lib},{n},{e:.6e},{mix:.10e},{ms[0]:.10e}")
                    print("   chi", e, "weights", [round(w / sum(ws), 4) for w in ws],
                          "mix", mix, "mt19", ms[0])
    with open(out, "w") as f:
        f.write("# generated by transport_channels_reference.py, openmc "
                + openmc.__version__ + "\n")
        f.write("\n".join(rows) + "\n")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2:])
