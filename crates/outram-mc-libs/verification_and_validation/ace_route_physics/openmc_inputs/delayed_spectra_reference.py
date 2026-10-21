#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0
#
# Exact mean fission-neutron birth energy under OpenMC's sample_fission_neutron
# (src/physics.cpp): delayed with probability nu_d/nu_t, group by its yield,
# energy from that group's law; otherwise the prompt law. For GitHub #365 audit
# (delayed spectra reach outram-mc's transport on both data routes).
#
# Own work (outram-park-backend), GPL-3.0; OpenMC Python API (MIT) as a library.
#
# ACE rows use OpenMC's ACE reader of the NJOY2016 tables; ENDF rows use
# OpenMC's ENDF reader of the same evaluations (no NJOY), so the ENDF route is
# checked against an independent parse of the evaluation, not against ACER.
#
# Means are exact for the schemes involved:
#   ContinuousTabular (ACE LAW=4, ENDF LF=1): statistical interpolation between
#     incident tables (r lin-lin) + envelope scaling -> exact from table means.
#   GeneralEvaporation (ENDF LF=5) with theta == 1 (asserted): mean of g.
# Output rows: <route>,<nuclide>,<E eV>,<beta>,<prompt mean>,<delayed mean>,<mixture mean>
# Usage: openmc-env; python delayed_spectra_reference.py <ace dir> <endf dir> <out.csv>

import sys
import numpy as np
import openmc
import openmc.data as od

NUC = {"U235": "n-092_U_235-ENDF8.0.endf", "U238": "n-092_U_238.endf"}
ENERGIES = [1.0e6, 5.0e6]


def tab_mean(x, p, interp):
    x, p = np.asarray(x, float), np.asarray(p, float)
    h = np.diff(x)
    if interp == "histogram":
        return float(np.sum(0.5 * (x[1:] + x[:-1]) * p[:-1] * h) / np.sum(p[:-1] * h))
    m1 = np.sum(h * (x[:-1] * (2 * p[:-1] + p[1:]) + x[1:] * (p[:-1] + 2 * p[1:])) / 6.0)
    return float(m1 / np.sum(h * (p[:-1] + p[1:]) / 2.0))


def energy_mean(dist, e):
    en = dist.energy if type(dist).__name__ == "UncorrelatedAngleEnergy" else dist
    name = type(en).__name__
    if name == "ContinuousTabular":
        g = np.asarray(en.energy)
        i = int(np.clip(np.searchsorted(g, e, side="right") - 1, 0, len(g) - 2))
        r = float(np.clip((e - g[i]) / (g[i + 1] - g[i]), 0.0, 1.0))
        a, b = en.energy_out[i], en.energy_out[i + 1]
        e1 = a.x[0] + r * (b.x[0] - a.x[0])
        ek = a.x[-1] + r * (b.x[-1] - a.x[-1])
        out = 0.0
        for w, t in ((1 - r, a), (r, b)):
            if w > 0:
                m = tab_mean(t.x, t.p, t.interpolation)
                out += w * (e1 + (m - t.x[0]) * (ek - e1) / (t.x[-1] - t.x[0]))
        return out
    if name == "ArbitraryTabulated":
        # ENDF LF=1, sampled (by outram's ENDF route and by a ContinuousTabular
        # conversion) with the same statistical interpolation + envelope scaling.
        g = np.asarray(en.energy)
        i = int(np.clip(np.searchsorted(g, e, side="right") - 1, 0, len(g) - 2))
        r = float(np.clip((e - g[i]) / (g[i + 1] - g[i]), 0.0, 1.0))
        a, b = en.pdf[i], en.pdf[i + 1]
        kind = lambda t: {1: "histogram", 2: "linear-linear"}[int(t.interpolation[0])]
        e1 = a.x[0] + r * (b.x[0] - a.x[0])
        ek = a.x[-1] + r * (b.x[-1] - a.x[-1])
        out = 0.0
        for w, t in ((1 - r, a), (r, b)):
            if w > 0:
                m = tab_mean(t.x, t.y, kind(t))
                out += w * (e1 + (m - t.x[0]) * (ek - e1) / (t.x[-1] - t.x[0]))
        return out
    if name == "GeneralEvaporation":
        assert np.all(np.asarray(en.theta.y) == 1.0)
        g = en.g
        interp = {1: "histogram", 2: "linear-linear"}[int(g.interpolation[0])]
        return tab_mean(g.x, g.y, interp)
    raise ValueError(name)


def endf_prompt_law(path):
    # OpenMC's IncidentNeutron.from_endf leaves the prompt fission neutron's
    # distribution empty for these evaluations; read MF=5/MT=18 (one LF
    # subsection) with OpenMC's own EnergyDistribution.from_endf instead.
    import io
    from openmc.data.endf import get_head_record, get_tab1_record
    from openmc.data.energy_distribution import EnergyDistribution
    ev = od.endf.Evaluation(path)
    fo = io.StringIO(ev.section[5, 18])
    head = get_head_record(fo)
    assert head[4] == 1, "one MF=5 subsection expected"
    params, _p = get_tab1_record(fo)
    return EnergyDistribution.from_endf(fo, params)


def rows_for(route, name, data, prompt_law=None):
    fis = data.reactions[18]
    prompt = [p for p in fis.products if p.emission_mode == "prompt"][0]
    prompt_dist = prompt.distribution[0] if prompt.distribution else prompt_law
    delayed = [p for p in fis.products if p.emission_mode == "delayed"]
    total = [p for p in fis.derived_products if p.emission_mode == "total"]
    out = []
    for e in ENERGIES:
        nu_p = float(prompt.yield_(e))
        yk = [float(p.yield_(e)) for p in delayed]
        nu_d = sum(yk)
        nu_t = float(total[0].yield_(e)) if total else nu_p + nu_d
        beta = nu_d / nu_t
        mp = energy_mean(prompt_dist, e)
        md = sum(y * energy_mean(p.distribution[0], e) for y, p in zip(yk, delayed)) / nu_d
        mix = (1 - beta) * mp + beta * md
        out.append(",".join([route, name] + [repr(float(v)) for v in (e, beta, mp, md, mix)]))
        print(route, name, e, "beta", beta, "prompt", mp, "delayed", md, "mix", mix)
    return out


def main(ace_dir, endf_dir, csv):
    rows = []
    for name, tape in NUC.items():
        rows += rows_for("ace", name, od.IncidentNeutron.from_ace(f"{ace_dir}/{name}.ace"))
        rows += rows_for("endf", name, od.IncidentNeutron.from_endf(f"{endf_dir}/{tape}"),
                         endf_prompt_law(f"{endf_dir}/{tape}"))
    with open(csv, "w") as f:
        f.write("# generated by delayed_spectra_reference.py, openmc " + openmc.__version__ + "\n")
        f.write("\n".join(rows) + "\n")


if __name__ == "__main__":
    main(*sys.argv[1:4])
