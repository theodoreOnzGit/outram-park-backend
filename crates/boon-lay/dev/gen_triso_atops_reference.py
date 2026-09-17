#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0
"""Generate the TRISO-ATOPS code-to-code reference fixture for `boon-lay`.

This harness drives the **upstream INL TRISO-ATOPS Python** (MIT, commit
``de374c8``) over a branch-covering input grid and writes every reference value
to ``tests/data/triso_atops_reference.csv``. The Rust regression test
``tests/triso_atops_code_to_code.rs`` replays that fixture through the Rust port
in ``src/triso_atops_fork/`` and asserts agreement.

Why a Python harness lives in this repo
---------------------------------------
The workspace rule "No Python for documentation or accounting" scopes itself to
documentation generation and repository accounting. This script is neither: it
*executes the third-party upstream reference implementation* to produce V&V
reference data, exactly as ``crates/outram-park-fork-coolprop/dev/*.py`` reads a
gitignored upstream clone to emit committed Rust. That precedent is called out in
the workspace ``CLAUDE.md`` as explicitly not covered by the rule.

Upstream clone (gitignored, reference-only, never compiled into the crate):
    crates/boon-lay/upstream_source/TRISO-ATOPS  @ de374c8

    git clone https://github.com/IdahoLabResearch/TRISO-ATOPS.git
    git -C TRISO-ATOPS checkout de374c8

Usage:
    python3 dev/gen_triso_atops_reference.py          # writes the CSV fixture
    python3 dev/gen_triso_atops_reference.py --check   # regenerate + diff only

Requires: numpy. (Upstream also ``import pandas as pd`` in
``calculation_functions.py`` but never references it -- verified 0 ``pd.``
occurrences at de374c8 -- so the harness injects an empty stub module rather
than pulling in an unused heavy dependency.)
"""

from __future__ import annotations

import argparse
import math
import sys
import types
from pathlib import Path

CRATE_ROOT = Path(__file__).resolve().parent.parent
UPSTREAM = CRATE_ROOT / "upstream_source" / "TRISO-ATOPS" / "trisoatops" / "utility_functions"
FIXTURE = CRATE_ROOT / "tests" / "data" / "triso_atops_reference.csv"
UPSTREAM_COMMIT = "de374c8"


def load_upstream():
    if not UPSTREAM.is_dir():
        sys.exit(
            f"upstream TRISO-ATOPS clone not found at {UPSTREAM}\n"
            "  git clone https://github.com/IdahoLabResearch/TRISO-ATOPS.git "
            f"{CRATE_ROOT / 'upstream_source' / 'TRISO-ATOPS'}\n"
            f"  git -C ... checkout {UPSTREAM_COMMIT}"
        )
    # Upstream imports pandas but never uses it in calculation_functions.py.
    sys.modules.setdefault("pandas", types.ModuleType("pandas"))
    sys.path.insert(0, str(UPSTREAM))
    import calculation_functions as calc  # noqa: E402

    return calc


def fmt(x) -> str:
    """Full round-trip f64 formatting; non-finite values stay parseable."""
    x = float(x)
    if math.isnan(x):
        return "nan"
    if math.isinf(x):
        return "inf" if x > 0 else "-inf"
    return repr(x)


ROWS: list[tuple[str, list[float], float, float]] = []


def case(fn: str, args: list, expected, cond: float = 1.0) -> None:
    """Record one reference case.

    ``cond`` is the *cancellation ratio* of the upstream evaluation --
    ``largest intermediate term / |result|``, or ``1/x`` where the formula
    evaluates ``1 - exp(-x)`` at small ``x``. It is 1.0 for well-conditioned
    results. The Rust test widens that case's tolerance to
    ``max(group_tol, 8 * eps * cond)``: where upstream's own arithmetic cannot
    resolve the answer to better than some relative precision, neither
    implementation is wrong for differing at that level, and asserting tighter
    would be asserting noise. Recording it per case keeps the well-conditioned
    majority at full tightness instead of loosening a whole group.
    """
    ROWS.append((fn, [float(a) for a in args], float(expected), float(cond)))


def cond_one_minus_exp(x: float) -> float:
    """Conditioning of ``1 - exp(-x)``: absolute error is ~eps near 1, so the
    relative error of the result is ~eps/x once x << 1."""
    x = abs(float(x))
    return 1.0 / x if 0.0 < x < 1.0 else 1.0


# ── input grids, chosen to straddle every branch/clamp in the upstream source ──
# kernel-temperature grid: 490/550/700/800 clamps and the 1500 degC kernel branch
T_KERNEL = [300, 489, 490, 491, 549, 550, 551, 699, 700, 701,
            799, 800, 801, 1000, 1200, 1499, 1500, 1501, 2000, 2400]
T_GRAPH = [300, 489, 490, 491, 549, 550, 551, 799, 800, 801, 1000, 1500, 2400]
# one representative z per upstream branch, plus the 1e-19 fallback (z=60)
Z_DIFFUSION = [34, 36, 52, 53, 54, 37, 55, 38, 56, 63, 46, 47, 60]


def cond_breakthrough(int_dp: float, int_dt: float, a: float, r: float, calc) -> float:
    """Cancellation ratio of ``breakthrough_model_transient``.

    The result is ``line - cst - ser`` where the three terms are individually
    O(0.1) but can cancel to O(1e-13). Returns largest|term| / |result|.
    """
    import numpy as np

    total = 0.0
    for n in range(1, 1000):
        total += (-1) ** n / ((n * np.pi) ** 2) * np.exp(-((n * np.pi) ** 2) * int_dp)
    line = 3 * int_dt / a / r
    cst = a / 2 / r
    ser = 6 * a / r * total
    res = line - cst - ser
    if res <= 0.0 or res >= 1.0:
        return 1.0  # clamped by upstream; the clamp is exact
    biggest = max(abs(line), abs(cst), abs(ser))
    return biggest / abs(res) if res != 0.0 else 1.0


def gen_diffusion(calc):
    """Diffusion coefficients over every branch and clamp.

    ``diffusion_coefficient`` is *separable*: in upstream de374c8 the kernel
    coefficient reads only ``T`` and the graphite coefficient reads only
    ``T_graph`` -- except for the Kr/Te/I/Xe/Se group, where ``D_graph = D`` and
    so both read ``T``. A full ``T x T_graph`` cross product therefore emits
    thousands of rows that are exact duplicates of a 1-D sweep and add no branch
    coverage. We sweep each variable against the other held at a fixed
    mid-range value, then add a deliberately small cross block on one z per
    branch to catch any coupling this separability argument would miss.
    """
    T_FIXED, TG_FIXED = 1000, 1000
    for z in Z_DIFFUSION:
        for t in T_KERNEL:  # kernel sweep (also covers D_graph = D group)
            d, dg = calc.diffusion_coefficient(z, float(t), float(TG_FIXED))
            case("diffusion_coefficient.kernel", [z, t, TG_FIXED], d)
            case("diffusion_coefficient.graphite", [z, t, TG_FIXED], dg)
        for tg in T_GRAPH:  # graphite sweep
            d, dg = calc.diffusion_coefficient(z, float(T_FIXED), float(tg))
            case("diffusion_coefficient.kernel", [z, T_FIXED, tg], d)
            case("diffusion_coefficient.graphite", [z, T_FIXED, tg], dg)
    # coupling check: one representative z per upstream branch, full cross
    for z in [53, 55, 38, 47, 60]:
        for t in [300, 700, 1000, 1500, 2400]:
            for tg in [300, 490, 550, 800, 1500]:
                d, dg = calc.diffusion_coefficient(z, float(t), float(tg))
                case("diffusion_coefficient.kernel", [z, t, tg], d)
                case("diffusion_coefficient.graphite", [z, t, tg], dg)
    for t in T_KERNEL:
        case("diffusion_coefficient_sic_ag", [t], calc.diffusion_coefficient_SiC_Ag(float(t)))


def gen_rb_fail_noble(calc):
    # Only the z values upstream actually resolves: z==36, or z==54, or z in
    # halogens. Other noble gases (2/10/18/86) hit an UnboundLocalError upstream.
    zs = [36, 54] + calc.halogens
    for z in zs:
        for lam in [1e-9, 1e-7, 1e-5, 1e-3, 1e-1, 1.0]:
            for t in [300, 700, 1000, 1500, 2400]:
                case("rb_fail_noble_gases", [z, lam, t],
                     calc.RB_fail_Noble_Gases(z, lam, float(t)))


def gen_steady_release(calc):
    for d in [1e-20, 1e-18, 1e-16, 1e-14, 1e-12]:
        for t in [1e5, 1e7, 1e8, 1e9]:
            for a in [1e-5, 3.5e-5, 1e-4]:
                for r in [2.13e-4, 5e-4]:
                    case("breakthrough_model", [d, t, a, r],
                         calc.breakthrough_model(d, t, a, r),
                         cond_breakthrough(d * t / a / a, d * t, a, r, calc))
                case("booth_longlived", [d, t, a], calc.booth_longlived(d, t, a))
    for d in [1e-20, 1e-18, 1e-16, 1e-14, 1e-12]:
        for lam in [1e-9, 1e-7, 1e-5, 1e-3, 1e-1]:
            for a in [1e-5, 3.5e-5, 1e-4]:
                case("booth_shortlived_fast_diffuse", [d, lam, a],
                     calc.booth_shortlived_fastdiffuse(d, lam, a))
    for dg in [1e-20, 1e-18, 1e-16, 1e-14, 1e-12]:
        for t in [1e5, 1e7, 1e8, 1e9]:
            for a in [1e-3, 4.5e-3, 1e-2]:
                case("attenuation_factor", [dg, t, a], calc.attenuation_factor(dg, t, a))


def gen_transient(calc):
    for idp in [0.0, 1e-9, 1e-6, 1e-4, 1e-2, 0.1, 1.0, 10.0]:
        case("booth_transient", [idp], calc.booth_transient(idp))
        for idt in [0.0, 1e-18, 1e-14, 1e-10]:
            for a in [3.5e-5, 1e-4]:
                for r in [2.13e-4]:
                    case("breakthrough_model_transient", [idp, idt, a, r],
                         calc.breakthrough_model_transient(idp, idt, a, r),
                         cond_breakthrough(idp, idt, a, r, calc))
    for val in [0.0, 1e-12, 1e-9, 1e-7, 1e-5, 1e-3]:
        for a in [1e-3, 4.5e-3, 1e-2]:
            import math as _m
            x1 = (_m.pi ** 2) * val / 4.0 / (a * a)
            case("rf_graph", [val, a], calc.RF_Graph(val, a), cond_one_minus_exp(x1))


def gen_activities(calc):
    """Circulating / plate-out / clean-up activity bookkeeping.

    Covers both the steady-state and time-dependent forms, with and without a
    clean-up system, and with a non-zero parent contribution so the decay-chain
    coupling term is exercised rather than multiplied by zero.
    """
    lams = [1e-9, 1e-7, 1e-5, 1e-3]
    for s_rate in [1.0, 1e6, 1e12]:
        for kp in [0.0, 7.5e-4, 1e-2]:
            for lam in lams:
                for kc in [0.0, 8.77e-5]:
                    for par in [0.0, 1e5]:
                        case("circulating_steadystate", [s_rate, kp, lam, kc, par],
                             calc.circulating_steadystate(s_rate, kp, lam, kc, par))
                        case("plate_out_steadystate", [kp, s_rate, lam, kc, par],
                             calc.plate_out_steadystate(kp, s_rate, lam, kc, par))
                        case("clean_up_steadystate", [kp, s_rate, lam, kc, par],
                             calc.clean_up_steadystate(kp, s_rate, lam, kc, par))
                        for t in [1e5, 1e8, 1e9]:
                            c = calc.circulating(s_rate, kp, lam, t, kc, par)
                            case("circulating", [s_rate, kp, lam, t, kc, par], c)
                            # plate_out guards `beta - lam == 0` and returns 0,
                            # DROPPING P_parent; that path is reached here
                            # whenever k_plate == k_clean == 0.
                            case("plate_out", [kp, s_rate, lam, t, c, kc, par],
                                 calc.plate_out(kp, s_rate, lam, t, c, kc, par))
                            # UPSTREAM DEFECT (de374c8): clean_up carries no such
                            # guard, so the same beta == lam input raises
                            # ZeroDivisionError instead of returning 0. Upstream
                            # never reaches it because higher_activities only
                            # calls clean_up when clean is True (=> k_clean > 0),
                            # so it is latent, not live. No reference value can
                            # exist for those inputs; the Rust test asserts that
                            # divergence separately rather than skipping it.
                            if kp + kc != 0.0:
                                case("clean_up", [kp, s_rate, lam, t, c, kc, par],
                                     calc.clean_up(kp, s_rate, lam, t, c, kc, par))


def gen_dispatchers(calc):
    import numpy as np

    a_grain, a_sic, r = 1e-5, 3.5e-5, 2.13e-4
    # R_B_fail across every group branch: noble/halogen, special metal (both
    # short- and long-lived), Ag/Pd breakthrough, and the 1e-5 fallback.
    for z in [36, 54, 53, 52, 55, 37, 38, 56, 63, 47, 46, 60, 40]:
        for sl in [True, False]:
            for lam in [1e-9, 1e-6, 1e-3]:
                for temp in [700.0, 1000.0, 1500.0]:
                    for t in [1e8, 1e9]:
                        d, _ = calc.diffusion_coefficient(z, temp, temp)
                        out = calc.R_B_fail(z, sl, lam, np.array([temp]), t,
                                            a_grain, a_sic, r, np.array([d]))
                        case("rb_fail", [z, 1.0 if sl else 0.0, lam, temp, t,
                                         a_grain, a_sic, r, d], np.asarray(out).ravel()[0])
    # release_rate: the three fraction-weighting branches x short/long lived
    fractions = np.array([1e-4, 1e-4, 2.3e-5, 3.6e-5])
    for z in [54, 53, 47, 55, 60]:
        for sl in [True, False]:
            for lam in [1e-9, 1e-6, 1e-3]:
                for t in [1e8, 1e9]:
                    for rb in [1e-8, 1e-5, 1e-2]:
                        out = calc.release_rate(np.array([rb]), z, fractions,
                                                np.array([44.5]), sl, t, lam)
                        case("release_rate", [rb, z, 1.0 if sl else 0.0, t, lam, 44.5,
                                              *fractions], np.asarray(out).ravel()[0])
    # base_activities: volatile (S=R, G=0) vs held-up (attenuation) branches
    for z in [54, 53, 55, 38, 47, 60]:
        for lam in [1e-9, 1e-6, 1e-3]:
            for t in [1e8, 1e9]:
                for dg in [1e-18, 1e-14, 1e-12]:
                    for rr in [1.0, 1e6]:
                        s, g = calc.base_activities(z, lam, t, 4.5e-3,
                                                    np.array([dg]), np.array([rr]))
                        case("base_activities.source", [z, lam, t, 4.5e-3, dg, rr],
                             np.asarray(s).ravel()[0])
                        case("base_activities.graphite", [z, lam, t, 4.5e-3, dg, rr],
                             np.asarray(g).ravel()[0])


def gen_release_fraction(calc):
    """Transient (accident) release-fraction dispatcher.

    Covers both materials and every branch: kernel non-silver (Booth
    transient), kernel silver (breakthrough through SiC), graphite volatile
    (identically zero) and graphite metal (RF_Graph).
    """
    import numpy as np

    fractions = np.array([1e-4, 1e-4, 2.3e-5, 3.6e-5])
    for z in [54, 53, 55, 38, 47, 60]:
        for integral in [0.0, 1e-18, 1e-14, 1e-12, 1e-10]:
            for a_primary in [2.13e-4, 4.5e-3]:
                for a_secondary in [3.5e-5, 1e-4]:
                    for material in ["kernel", "graphite"]:
                        out = calc.release_fraction(z, fractions, np.array([integral]),
                                                    a_primary, a_secondary, material)
                        # the dispatcher inherits the conditioning of whichever
                        # model it selects
                        if material == "kernel":
                            cond = (cond_breakthrough(integral / a_secondary / a_secondary,
                                                      integral, a_secondary, a_primary, calc)
                                    if z == 47 else 1.0)
                        elif z in calc.noble_gases or z in calc.halogens:
                            cond = 1.0  # identically zero, exact
                        else:
                            import math as _m
                            x1 = (_m.pi ** 2) * integral / 4.0 / (a_primary * a_primary)
                            cond = cond_one_minus_exp(x1)
                        case(f"release_fraction.{material}",
                             [z, integral, a_primary, a_secondary],
                             np.asarray(out).ravel()[0], cond)


def gen_integrate(calc):
    """Cumulative time-integral of D over a temperature history.

    Upstream ``integrate`` works on a (n_radial, n_times, n_axial) array with
    ``np.diff(times, prepend=0)`` and a cumulative sum, so an ordering or
    off-by-one slip is silent. We drive a single radial/axial node and emit one
    row per output time step, so every partial sum is pinned individually.

    Row layout (variable length, read positionally by the Rust test):
        [z, n, t_0..t_{n-1}, T_0..T_{n-1}, out_index]
    """
    import numpy as np

    histories = [
        ([0.0, 1e3, 1e4, 1e5], [600.0, 900.0, 1200.0, 1500.0]),
        ([0.0, 3.6e3, 7.2e3, 1.08e4, 1.44e4], [1000.0, 1400.0, 1600.0, 1400.0, 1000.0]),
        ([0.0, 1e5, 2e5], [300.0, 800.0, 2400.0]),
    ]
    for z in [53, 55, 38, 47, 60]:
        for times, temps in histories:
            n = len(times)
            for material in ["kernel", "graphite"]:
                shape = np.array([1, n, 1])
                arr = calc.integrate(z, shape,
                                     np.array(times),
                                     np.array(temps).reshape(1, n, 1),
                                     material)
                flat = np.asarray(arr).reshape(n)
                for i in range(n):
                    case(f"integrate.{material}",
                         [z, n, *times, *temps, i], flat[i])


def gen_normal_operation_node(calc):
    """End-to-end normal-operation chain for one node.

    Reproduces the composition in upstream ``trisoatops.py`` (the normal-
    operation driver), which is what the port's ``normal_operation_node``
    corresponds to -- NOT ``higher_activities`` alone. The group-dependent
    zeroing of k_plate (noble gases) and k_clean (non-halogens) happens at that
    call site upstream, so it is part of the reference, not a port invention.

    Row layout:
        [z, a_mass, sl, inventory_ci, T_core, T_graph, clean, k_plate, k_clean,
         a_graph, a_grain, a_SiC, r, t_run, t_irad,
         f_hm, f_sic, f_inc, f_inc_sic, c_par, p_par, hps_par]
    """
    import numpy as np

    f = [1e-4, 1e-4, 2.3e-5, 3.6e-5]
    a_graph, a_grain, a_sic, r = 4.5e-3, 1e-5, 3.5e-5, 2.13e-4
    yr = 3.15576e7
    t_run, t_irad = 40.0 * yr, 3.0 * yr

    for name in ["Xe-133", "I-131", "Cs-137", "Sr-90", "Ag-110m", "Ce-144"]:
        nuc = calc.nuclides.get(name)
        if nuc is None:
            continue
        z, a_mass, lam = nuc.z, nuc.a, nuc.lam
        for sl in [True, False]:
            for temp in [700.0, 900.0, 1200.0]:
                for clean in [True, False]:
                    for (c_par, p_par, hps_par) in [(0.0, 0.0, 0.0), (1e5, 1e4, 1e3)]:
                        k_plate, k_clean = 7.5e-4, 8.77e-5
                        inv_ci = 44.5
                        d_kern, d_graph = calc.diffusion_coefficient(z, temp, temp)
                        rb = calc.R_B_fail(z, sl, lam, np.array([temp]), t_irad,
                                           a_grain, a_sic, r, np.array([d_kern]))
                        rr = calc.release_rate(rb, z, np.array(f), np.array([inv_ci]),
                                               sl, t_irad, lam)
                        s_rate, g = calc.base_activities(z, lam, t_irad, a_graph,
                                                         np.array([d_graph]), rr)
                        # group dispatch exactly as trisoatops.py does it
                        if z in calc.noble_gases:
                            kp, kc = 0.0, k_clean
                        elif z in calc.halogens:
                            kp, kc = k_plate, k_clean
                        else:
                            kp, kc = k_plate, 0.0
                        c, p, hps = calc.higher_activities(
                            z, kp, lam, t_run, clean, kc, s_rate,
                            np.array([c_par]), np.array([p_par]), np.array([hps_par]))
                        args = [z, a_mass, 1.0 if sl else 0.0, inv_ci, temp, temp,
                                1.0 if clean else 0.0, k_plate, k_clean,
                                a_graph, a_grain, a_sic, r, t_run, t_irad,
                                *f, c_par, p_par, hps_par]
                        flat = lambda x: float(np.asarray(x).ravel()[0])
                        case("node.release_rate", args, flat(rr))
                        case("node.source_rate", args, flat(s_rate))
                        case("node.graphite_activity", args, flat(g))
                        case("node.circulating_activity", args, flat(c))
                        case("node.plate_out_activity", args, flat(p))
                        if hps is not None:
                            case("node.clean_up_activity", args, flat(hps))


def gen_nuclides(calc):
    """Decay constants straight out of the upstream nuclide table."""
    for name in sorted(calc.nuclides):
        case_name = name
        nuc = calc.nuclides[name]
        case(f"nuclide.lam:{case_name}", [], float(nuc.lam))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true",
                    help="regenerate and diff against the committed fixture")
    args = ap.parse_args()

    calc = load_upstream()
    gen_diffusion(calc)
    gen_rb_fail_noble(calc)
    gen_steady_release(calc)
    gen_transient(calc)
    gen_activities(calc)
    gen_dispatchers(calc)
    gen_release_fraction(calc)
    gen_integrate(calc)
    gen_normal_operation_node(calc)
    gen_nuclides(calc)

    out = [
        f"# TRISO-ATOPS code-to-code reference values, generated by "
        f"dev/gen_triso_atops_reference.py",
        f"# upstream: IdahoLabResearch/TRISO-ATOPS @ {UPSTREAM_COMMIT} (MIT)",
        f"# DO NOT EDIT BY HAND -- regenerate with the script above.",
        "function,args,expected,cond",
    ]
    # The sweeps and the coupling block legitimately overlap, and for the
    # Kr/Te/I/Xe/Se group D_graph is independent of T_graph, so the same
    # (function, args) case can be produced twice. Emit each exactly once,
    # first-occurrence order preserved, so the fixture stays reviewable.
    seen: set[str] = set()
    dupes = 0
    for fn, a, exp, cond in ROWS:
        line = f"{fn},{';'.join(fmt(x) for x in a)},{fmt(exp)},{fmt(cond)}"
        if line in seen:
            dupes += 1
            continue
        seen.add(line)
        out.append(line)
    print(f"  ({dupes} duplicate cases collapsed)", file=sys.stderr)
    text = "\n".join(out) + "\n"

    if args.check:
        if not FIXTURE.exists():
            print("fixture missing", file=sys.stderr)
            return 1
        same = FIXTURE.read_text() == text
        print("fixture up to date" if same else "FIXTURE DIFFERS")
        return 0 if same else 1

    FIXTURE.parent.mkdir(parents=True, exist_ok=True)
    FIXTURE.write_text(text)
    print(f"wrote {FIXTURE} ({len(out) - 4} cases)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
