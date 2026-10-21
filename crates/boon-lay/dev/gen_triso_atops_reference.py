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


def load_upstream_driver():
    """Import upstream's top-level `trisoatops` module.

    Separate from `load_upstream` because the driver needs the *package* root
    on `sys.path` (it imports `utility_functions.calculation_functions`), and
    because only the end-to-end `accident_case` fixture needs it -- everything
    else drives `calculation_functions` directly.

    Returns `(trisoatops_module, calculation_functions_module)`. The two
    `calculation_functions` instances are the same module object, so patching
    one is visible to the other.
    """
    package_root = UPSTREAM.parent
    sys.modules.setdefault("pandas", types.ModuleType("pandas"))
    sys.path.insert(0, str(package_root))
    sys.path.insert(0, str(UPSTREAM))
    import trisoatops as driver  # noqa: E402
    import utility_functions.calculation_functions as calc  # noqa: E402

    return driver, calc


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
    # Ag-in-SiC is a bare Arrhenius with no branch, so density costs nothing and
    # the useful check is that the port's exponent and pre-factor agree across
    # the full span where D spreads over ~30 decades.
    for t in sorted(set(T_KERNEL + list(range(250, 2501, 25)))):
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
    """Accident-path release fractions.

    ``booth_transient`` is swept densely across the whole useful range rather
    than at a handful of decades, because it carries TWO regime changes that a
    coarse sweep steps straight over:

    * the ``int_Dp == 0`` early return, and
    * the ``RF < 1e-6 -> 0`` floor, which bites somewhere around
      ``int_Dp ~ 3e-9`` and is a discontinuity, not a rounding detail.

    The sweep therefore walks 1e-12 .. 1e2 at three points per decade and adds
    an explicitly refined block straddling the floor, so the crossing sample is
    in the fixture instead of being interpolated over.
    """
    import math as _m

    # three points per decade, 1e-12 .. 1e2, plus the exact-zero branch
    idp_sweep = [0.0]
    for e in range(-12, 3):
        for m in (1.0, 2.15, 4.64):
            idp_sweep.append(m * 10.0 ** e)
    # refine around the 1e-6 release-fraction floor
    idp_sweep += [2.0e-9, 2.5e-9, 3.0e-9, 3.5e-9, 4.0e-9, 5.0e-9, 6.0e-9]
    for idp in sorted(set(idp_sweep)):
        case("booth_transient", [idp], calc.booth_transient(idp))

    for idp in [0.0, 1e-9, 1e-6, 1e-4, 1e-2, 0.1, 1.0, 10.0]:
        for idt in [0.0, 1e-18, 1e-14, 1e-10]:
            for a in [3.5e-5, 1e-4]:
                for r in [2.13e-4]:
                    case("breakthrough_model_transient", [idp, idt, a, r],
                         calc.breakthrough_model_transient(idp, idt, a, r),
                         cond_breakthrough(idp, idt, a, r, calc))

    # RF_Graph is a function of the single group val/a^2, but it is NOT written
    # that way upstream -- it divides by 4 and by a**2 separately -- so sweep
    # both arguments independently rather than collapsing them, and span the
    # saturated end where every series term reaches 1 - exp(-inf) = 1.
    rf_vals = [0.0]
    for e in range(-14, -1):
        for m in (1.0, 3.16):
            rf_vals.append(m * 10.0 ** e)
    for val in sorted(set(rf_vals)):
        for a in [1e-4, 1e-3, 4.5e-3, 1e-2, 5e-2]:
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


def gen_release_activity(calc):
    """`release_activity` — the accident-path inventory drawdown.

    Driven per node (upstream works on a whole radial x axial array; a
    1x1x1 array isolates one node and keeps the fixture scalar like the rest).

    The six fractions are `[f_hm, f_sic, f_inc, f_inc_sic, f_inc_acc,
    f_inc_sic_acc]`, exactly as `trisoatops.py::accident_case` assembles them.

    Emits the STOCK behaviour, cadmium typo and all -- `z` is passed through
    to upstream untouched, so `z == 48` takes the silver branch and `z == 46`
    does not. The Rust side replays this with `upstream_cadmium_typo = true`.
    """
    import numpy as np

    fr = np.array([1e-4, 1e-4, 2.3e-5, 3.6e-5, 5e-5, 7e-5])
    # z spans every branch: noble gas, halogen, silver(47), the typo'd 48,
    # palladium(46) which the typo excludes, a special metal and an "other".
    for z in [54, 53, 47, 48, 46, 55, 60]:
        for inv in [0.0, 1e9, 1e14]:
            for graph in [0.0, 1e6]:
                for circ in [0.0, 1e5]:
                    for plate in [0.0, 1e3, 1e13]:
                        for hps in [0.0, 1e4]:
                            for clean in [True, False]:
                                for rf in [0.0, 1e-6, 0.5]:
                                    nodal = np.zeros((7, 1, 1))
                                    nodal[0, 0, 0] = inv
                                    nodal[3, 0, 0] = graph
                                    nodal[4, 0, 0] = circ
                                    nodal[5, 0, 0] = plate
                                    nodal[6, 0, 0] = hps
                                    rf_vals = np.full((1, 1, 1), rf)
                                    for material in ["kernel", "graphite"]:
                                        out = calc.release_activity(
                                            z, fr, nodal, rf_vals, clean, material)
                                        case(f"release_activity.{material}",
                                             [z, inv, graph, circ, plate, hps,
                                              1.0 if clean else 0.0, rf, *fr],
                                             np.asarray(out).ravel()[0])


def gen_coolant_release(calc):
    """`coolant_release` — vented coolant fraction over a depressurisation.

    Upstream takes the full ``(radial, time, axial)`` temperature field and
    reduces it internally, averaging ``dT/dt`` over radial AND axial while
    reading the *absolute* temperature from one designated hot node. The Rust
    port splits that reduction out into `mean_temperature_rate`, so the fixture
    records BOTH: the mean dT/dt upstream computes, and the release fraction it
    derives.

    The earlier version of this generator only ever passed a ``(1, n, 1)``
    field, which makes the mean over nodes a no-op and the hot-node index
    trivially ``0`` — so the averaging and the node selection, the two things
    the split-out function actually has to get right, were never exercised.
    Multi-node fields are swept here, with the hot node chosen both at
    upstream's default (``floor(n_axial / 2)``) and off it.

    Row layout, flattened so the Rust side can rebuild the same field:

        [n_nodes, n_t,
         t_0 .. t_{n_t-1},
         T(node 0, t_0..t_{n_t-1}), ... T(node n_nodes-1, ...),
         hot_node, pressure_kPa, out_index]
    """
    import numpy as np

    # Each entry: (times, temps[radial][time][axial]) as nested lists.
    fields = []

    # -- single node, as before: the baseline cases -------------------------
    fields.append(([0.0, 100.0, 200.0, 300.0],
                   [[[500.0], [600.0], [700.0], [800.0]]]))
    fields.append(([0.0, 3600.0, 7200.0, 10800.0, 14400.0],
                   [[[600.0], [900.0], [1100.0], [1000.0], [850.0]]]))
    fields.append(([0.0, 1000.0, 2000.0],
                   [[[700.0], [700.0], [700.0]]]))

    # -- one ring, three axial nodes: averaging over axial is now real ------
    fields.append(([0.0, 3600.0, 7200.0, 10800.0],
                   [[[400.0, 900.0, 500.0],
                     [450.0, 1150.0, 560.0],
                     [520.0, 1260.0, 610.0],
                     [560.0, 1180.0, 640.0]]]))

    # -- two rings, three axial nodes: averaging over BOTH axes -------------
    fields.append(([0.0, 1800.0, 5400.0, 12600.0, 25200.0],
                   [[[380.0, 860.0, 470.0],
                     [430.0, 1090.0, 540.0],
                     [500.0, 1240.0, 600.0],
                     [540.0, 1150.0, 630.0],
                     [520.0, 980.0, 600.0]],
                    [[300.0, 520.0, 360.0],
                     [330.0, 610.0, 400.0],
                     [370.0, 700.0, 440.0],
                     [400.0, 690.0, 460.0],
                     [390.0, 620.0, 450.0]]]))

    # -- KNIFE EDGE: two nodes heating and cooling at equal and opposite rates,
    #    so the mean dT/dt is *exactly* zero at every sample while each node is
    #    still moving. Upstream's venting mask is `dTdt_avg >= 0`, so upstream
    #    keeps every sample; the port does not, and the reason is worth pinning
    #    rather than hiding -- see `coolant_release_knife_edge_...` in
    #    tests/triso_atops_code_to_code.rs. Emitted under its own group name so
    #    the ordinary comparison stays an ordinary comparison.
    knife_edge = ([0.0, 500.0, 1000.0, 1500.0],
                  [[[600.0, 800.0],
                    [700.0, 700.0],
                    [800.0, 600.0],
                    [900.0, 500.0]]])

    pressures = [101.325, 1.0, 5000.0]

    for times, nested in fields + [knife_edge]:
        tag = ".knife_edge" if (times, nested) == knife_edge else ""
        t_arr = np.array(times, dtype=float)
        temp_field = np.array(nested, dtype=float)  # (radial, time, axial)
        n_rad, n_t, n_ax = temp_field.shape
        # Flatten (radial, axial) into a node list, keeping the time axis: this
        # is exactly what the port's `mean_temperature_rate` consumes.
        nodes = [temp_field[r, :, k].tolist()
                 for r in range(n_rad) for k in range(n_ax)]
        n_nodes = len(nodes)
        flat_temps = [v for node in nodes for v in node]

        dTdt = np.diff(temp_field, axis=1) / (np.diff(t_arr)[:, None] + np.finfo(float).eps)
        dTdt = np.pad(dTdt, ((0, 0), (1, 0), (0, 0)), mode='constant', constant_values=0)
        dTdt_avg = np.mean(dTdt, axis=(0, 2))

        prefix = [n_nodes, n_t, *times, *flat_temps]

        # The mean dT/dt is independent of the hot node and the pressure, so
        # record it once per field rather than once per combination.
        for i in range(n_t):
            case(f"coolant_release{tag}.mean_dtdt", [*prefix, 0, 101.325, i], dTdt_avg[i])

        for hot_r in range(n_rad):
            for hot_ax in sorted({0, n_ax - 1, int(np.floor(n_ax / 2))}):
                hot_node = hot_r * n_ax + hot_ax
                for P in pressures:
                    frac, vent_times = calc.coolant_release(
                        t_arr, temp_field, P=P,
                        hottest_radial=hot_r, hottest_axial=hot_ax)
                    tail = [hot_node, P]
                    for i in range(len(frac)):
                        case(f"coolant_release{tag}.fraction", [*prefix, *tail, i], frac[i])
                    for i in range(len(vent_times)):
                        case(f"coolant_release{tag}.vent_time", [*prefix, *tail, i], vent_times[i])


def gen_inventory_processing(calc):
    """`inventory_processing` — axial split of a per-ring inventory."""
    import numpy as np

    for n_axial in [1, 2, 3, 5, 7, 11, 16, 64, 365]:
        for inv in [0.0, 1.0, 3.0, 44.5, 1e-30, 1e6, 1e22, 6.022e23]:
            # upstream's 2-D branch: (n_nuclides, n_radial) -> split over axial
            arr = np.array([[inv]])
            out = np.asarray(calc.inventory_processing(arr, n_axial)).ravel()
            # Emit every axial slot for the small splits -- the operation is a
            # divide *and* a repeat, and checking one element verifies only the
            # divide -- and first/middle/last for the large ones, where the
            # remaining slots are bit-identical repeats that buy no coverage.
            slots = (range(n_axial) if n_axial <= 16
                     else sorted({0, n_axial // 2, n_axial - 1}))
            for k in slots:
                case("inventory_processing", [n_axial, inv, k], out[k])


def gen_nuclide_selection(calc):
    """`nuclide_import` / `nuclide_import_accident` classification.

    EXHAUSTIVE: every nuclide in the upstream table, against a spread of
    irradiation and accident times that straddles each ratio threshold. The
    earlier pass sampled ten nuclides; the classification is a per-nuclide
    decision driven by that nuclide's own half-life, so sampling it was
    leaving 74 decisions unchecked for no reason.
    """
    # Times chosen so that, across the 84 half-lives (6.6e1 s .. 7.2e22 s),
    # BOTH outcomes occur for the thresholds 0.2 and 0.04.
    irrad_times = [8.64e4, 3.1536e6, 3.15576e7, 9.46728e7, 1.262304e9, 3.15576e10]
    accident_times = [3.6e3, 8.64e4, 2.592e5, 2.592e6, 3.1536e7]
    for name in sorted(calc.nuclides):
        nuc = calc.nuclides[name]
        for t_irrad in irrad_times:
            sl = 1.0 if (nuc.hl / t_irrad) < 0.2 else 0.0
            case(f"nuclide_import.short_lived:{name}", [t_irrad], sl)
        for t_acc in accident_times:
            keep = 1.0 if (nuc.hl / t_acc) >= 0.04 else 0.0
            case(f"nuclide_import_accident.retained:{name}", [t_acc], keep)


def gen_name_normalisation(calc):
    """Nuclide-name normalisation, against upstream's OWN regex.

    Upstream normalises with
        re.match(r'([a-z]{1,2})(?:[-]?)([0-9]+)([a-z]?)', name.lower())
    then reassembles `f"{element.capitalize()}-{main_number}{suffix}"`.

    The port hand-rolls this to avoid a `regex` dependency, so the hand-rolled
    version is checked against the real thing rather than against my reading of
    it. Encoded as: 1.0 if upstream produces the canonical name the port also
    produces, 0.0 if upstream rejects it. The expected NAME travels in the
    function tag, so a mismatch names both sides.
    """
    import re

    spellings = [
        # canonical and case variants
        "Cs-137", "cs-137", "CS-137", "cS-137",
        # no hyphen
        "Cs137", "cs137", "CS137",
        # single-letter elements
        "I-131", "i131", "I131", "Y-91", "y91",
        # metastable suffixes
        "Kr-83m", "kr83m", "KR-83M", "Tc-99m", "tc99m", "Ag-110m", "ag110m",
        # two-letter with three-digit mass
        "Pr-143", "pr143", "La-140", "Nd-144",
        # things upstream's regex rejects outright
        "plutonium", "137", "", "-137", "x",
        # things upstream ACCEPTS by prefix-matching that a reader may not expect
        "Cs-137xyz", "Cs-137-extra", "cs137mm",
    ]
    for raw in spellings:
        m = re.match(r'([a-z]{1,2})(?:[-]?)([0-9]+)([a-z]?)', raw.lower())
        if m:
            element, main_number, suffix = m.groups()
            canonical = f"{element.capitalize()}-{main_number}{suffix}"
            # 1.0 = upstream parsed it; the canonical form is in the tag.
            case(f"name_normalisation.accepted:{raw}|{canonical}", [], 1.0)
        else:
            case(f"name_normalisation.rejected:{raw}", [], 0.0)


def gen_convert_time(calc):
    """`run_functions.convert_time` — the unit factor table."""
    import sys
    sys.path.insert(0, str(UPSTREAM.parent))
    for unit in ["s", "min", "hr", "d", "yr"]:
        factor = {"s": 1, "min": 60, "hr": 3600, "d": 3600 * 24,
                  "yr": 365 * 24 * 3600}[unit]
        case(f"convert_time:{unit}", [], float(factor))


def gen_nuclide_sort(calc):
    """`run_functions.nuclide_sort` — verify it is the NO-OP we claim.

    The port's `sort_parents_before_daughters` deliberately does what upstream
    says it does rather than what it does. That claim is only worth making if
    upstream's actual behaviour is pinned, so this records, for each ordering,
    whether upstream moved anything: 1.0 if the output order differs from the
    input, 0.0 if unchanged.

    Reproduced inline rather than called, because `nuclide_sort` takes 2-D
    numpy arrays and a logger and returns early on shapes this fixture does not
    need; the branch under test is the single `par in list(...)` comparison,
    which is transcribed verbatim.
    """
    orderings = [
        ["Xe-135", "I-135"],          # daughter first: SHOULD reorder, does not
        ["I-135", "Xe-135"],          # already correct
        ["Rh-105", "Ru-105"],
        ["Cs-137", "I-131", "Sr-90"], # no parent relationships at all
        ["La-140", "Ba-140"],
    ]
    for order in orderings:
        moved = 0.0
        for n in order:
            try:
                par = calc.nuclides[n].parents
            except KeyError:
                par = None
            # VERBATIM upstream: a list tested for membership in a list of str.
            if par is not None and par in list(order):
                moved = 1.0
        case(f"nuclide_sort.reorders:{'+'.join(order)}", [], moved)


# ── end-to-end: accident_case ────────────────────────────────────────────────

#: Scenario fields for `gen_accident_case`, as
#: ``(label, times, temps[radial][time][axial], clean)``.
ACCIDENT_SCENARIOS = [
    (
        "heat_then_cool",
        [0.0, 1800.0, 5400.0, 12600.0, 25200.0],
        [[[600.0, 560.0], [900.0, 820.0], [1150.0, 1000.0],
          [1050.0, 930.0], [900.0, 820.0]],
         [[500.0, 470.0], [700.0, 650.0], [860.0, 790.0],
          [800.0, 740.0], [700.0, 650.0]]],
        False,
    ),
    (
        "heat_then_cool_hps",
        [0.0, 1800.0, 5400.0, 12600.0, 25200.0],
        [[[600.0, 560.0], [900.0, 820.0], [1150.0, 1000.0],
          [1050.0, 930.0], [900.0, 820.0]],
         [[500.0, 470.0], [700.0, 650.0], [860.0, 790.0],
          [800.0, 740.0], [700.0, 650.0]]],
        True,
    ),
    (
        "gappy_vent_mask",
        [0.0, 3600.0, 7200.0, 14400.0, 28800.0, 43200.0],
        [[[700.0, 660.0], [1000.0, 940.0], [950.0, 900.0],
          [1200.0, 1120.0], [1100.0, 1030.0], [980.0, 920.0]]],
        False,
    ),
]

#: Constants array laid out as upstream's `constants[...]` indices.
ACCIDENT_CONSTANTS = [
    1e-5,      # 0  f_hm        fraction of heavy metal contamination
    2e-5,      # 1  f_sic       defective SiC fraction
    3e-5,      # 2  f_inc       as-fabricated failed fraction
    4e-5,      # 3  f_inc_sic   in-pile SiC failure fraction
    4.5e-3,    # 4  a           graphite slab half-thickness, m
    1e-5,      # 5  a_grain     kernel grain size, m
    7.5e-4,    # 6  k_plate     plate-out constant, 1/s
    3.15576e7, # 7  t           reactor runtime, s
    0.0,       # 8  (unused by accident_case)
    8.77e-5,   # 9  k_clean     clean-up constant, 1/s
    2.13e-4,   # 10 r           kernel radius, m
    3.5e-5,    # 11 a_SiC       SiC layer thickness, m
    5e-5,      # 12 f_inc_acc   accident-induced failure fraction
    6e-5,      # 13 f_inc_sic_acc
    0.1,       # 14 x_liftoff   plate-out lift-off fraction
]

#: Nuclides driven through the end-to-end case, one per upstream branch:
#: noble gas (z=54), halogen (z=53), the Te-counted-as-halogen case (z=52),
#: silver (z=47, the `fract = 1` branch), and two ordinary metals.
ACCIDENT_NUCLIDES = ["Xe-133", "I-131", "Te-132", "Ag-110m", "Sr-90", "Cs-137"]


def _normop_field(index: int, n_axial: int, n_radial: int):
    """Deterministic normal-operation nodal array for one nuclide.

    Shape is ``(7, n_axial, n_radial)`` -- note the **axial-major** layout,
    which is not a typo: upstream's `release_activity` transposes with
    ``activities.T`` before broadcasting against the ``(radial, time, axial)``
    release-fraction array, so the normal-operation channels come in
    transposed relative to the temperature field. Getting that wiring wrong is
    exactly the kind of defect this end-to-end fixture exists to catch, so the
    values are made distinct per channel and per node rather than uniform.
    """
    import numpy as np

    n = 7 * n_axial * n_radial
    scale = 10.0 ** (10 + index)
    return np.arange(1, n + 1, dtype=float).reshape(7, n_axial, n_radial) * scale


def gen_accident_case(driver, calc):
    """End-to-end `accident_case`: the composition, not the pieces.

    Every function `accident_case` calls is already covered case-by-case
    elsewhere in this fixture. What is *not* otherwise covered is how they are
    wired together -- the argument order, the `(7, axial, radial)` transpose,
    the atoms-to-curies conversion applied to each path separately, the
    truncation of the temperature field to the venting window, and the fact
    that the circulating + lifted-off plate-out term is added **outside** the
    vent-fraction scaling. A port can get every individual formula right and
    still assemble them wrongly; this is the group that would notice.

    Upstream formats its return values through `vectorized_format`, which is
    `np.format_float_scientific(x, precision=2)` -- three significant digits,
    as strings, for display. That is a *presentation* step, not part of the
    calculation, and comparing against it would cap this group's resolution at
    ~1e-3. It is therefore patched to an identity for the duration of the call
    so the fixture records upstream's full-precision values. Nothing else is
    patched, and the patch is reverted immediately afterwards.

    Row layout. The scenario is written **once**, as its own row, and the
    value rows refer to it by index -- repeating ~170 numbers on each of 432
    value rows made the committed fixture 44 % scenario prefix, and the whole
    file is `include_str!`d into the test binary:

        accident_case.scenario:<id>   args = [n_radial, n_axial, n_t, clean,
                                              c_0 .. c_14,
                                              t_0 .. t_{n_t-1},
                                              T(r, t, k) in C order,
                                              normop(channel, k, r) in C order,
                                              one block per nuclide]
        accident_case.dropped:<nuc>   args = [id]
        accident_case.total:<nuc>     args = [id, time_index]
        accident_case.nodal_*:<nuc>   args = [id, r, t, k]

    `z` and the decay constant are deliberately **not** in the row: the Rust
    side looks them up in its own nuclide database, so the end-to-end check
    exercises that table too.
    """
    import logging

    import numpy as np

    log = logging.getLogger("triso_atops_fixture")
    log.addHandler(logging.NullHandler())
    log.propagate = False

    consts = np.array(ACCIDENT_CONSTANTS, dtype=float)
    nuclide_list = [[name, 1.0] for name in ACCIDENT_NUCLIDES]

    for label, times, nested, clean in ACCIDENT_SCENARIOS:
        t_arr = np.array(times, dtype=float)
        temps = np.array(nested, dtype=float)  # (radial, time, axial)
        n_radial, n_t, n_axial = temps.shape

        normop = {
            name: _normop_field(i, n_axial, n_radial)
            for i, name in enumerate(ACCIDENT_NUCLIDES)
        }

        scenario = [
            n_radial, n_axial, n_t, 1.0 if clean else 0.0,
            *ACCIDENT_CONSTANTS,
            *times,
            *temps.ravel(order="C"),
        ]
        for name in ACCIDENT_NUCLIDES:
            scenario += list(normop[name].ravel(order="C"))
        scenario_id = ACCIDENT_SCENARIOS.index((label, times, nested, clean))
        case(f"accident_case.scenario:{scenario_id}", scenario, float(scenario_id))

        saved = calc.vectorized_format
        calc.vectorized_format = lambda x: np.asarray(x, dtype=float)
        try:
            totals, nodal = driver.accident_case(
                consts, nuclide_list, normop, temps, t_arr, log, clean=clean)
        finally:
            calc.vectorized_format = saved

        for name in ACCIDENT_NUCLIDES:
            if name not in totals:
                # Dropped by nuclide_import_accident for this accident length.
                # Recorded as an explicit absence so the Rust side can assert
                # the same nuclide is dropped rather than silently skipping.
                case(f"accident_case.dropped:{name}", [scenario_id], 1.0)
                continue
            case(f"accident_case.dropped:{name}", [scenario_id], 0.0)

            kernel = np.asarray(nodal[name].kernel, dtype=float)
            graphite = np.asarray(nodal[name].graphite, dtype=float)
            n_keep = kernel.shape[1]

            # Conditioning is INHERITED, not re-derived: the composition adds
            # only multiplications and sums, so the ill-conditioning is exactly
            # that of the release-fraction evaluation underneath -- the same
            # quantity `gen_release_fraction` records for the standalone group.
            # The integrals `accident_case` used are still on the returned
            # dataset, so this reads them rather than recomputing them.
            z = calc.nuclides[name].z
            int_kernel = np.asarray(nodal[name].integral_kernel, dtype=float)
            int_graphite = np.asarray(nodal[name].integral_graphite, dtype=float)
            a_sic = ACCIDENT_CONSTANTS[11]
            r_kernel = ACCIDENT_CONSTANTS[10]
            a_graph = ACCIDENT_CONSTANTS[4]
            volatile = z in calc.noble_gases or z in calc.halogens

            cond_k = np.ones_like(kernel)
            cond_g = np.ones_like(graphite)
            for r in range(n_radial):
                for ti in range(n_keep):
                    for k in range(n_axial):
                        ik = int_kernel[r, ti, k]
                        if z == 47:
                            cond_k[r, ti, k] = cond_breakthrough(
                                ik / a_sic / a_sic, ik, a_sic, r_kernel, calc)
                        if not volatile:
                            x1 = (math.pi ** 2) * int_graphite[r, ti, k] / 4.0 / (a_graph ** 2)
                            cond_g[r, ti, k] = cond_one_minus_exp(x1)

            for r in range(n_radial):
                for ti in range(n_keep):
                    for k in range(n_axial):
                        case(f"accident_case.nodal_kernel:{name}",
                             [scenario_id, r, ti, k], kernel[r, ti, k], cond_k[r, ti, k])
                        case(f"accident_case.nodal_graphite:{name}",
                             [scenario_id, r, ti, k], graphite[r, ti, k], cond_g[r, ti, k])

            total = np.asarray(totals[name], dtype=float).ravel()
            lam = calc.nuclides[name].lam
            circ = np.sum(normop[name][4, :, :]) * lam / 3.7e10
            plate = ACCIDENT_CONSTANTS[14] * np.sum(normop[name][5, :, :]) * lam / 3.7e10
            for i in range(total.size):
                # Cancellation ratio of the sum itself -- largest contributing
                # term over the result -- multiplied by the worst conditioning
                # already carried by those terms.
                terms = list(kernel[:, i, :].ravel()) + list(graphite[:, i, :].ravel()) \
                    + [circ, plate]
                biggest = max(abs(v) for v in terms) if terms else 0.0
                ratio = biggest / abs(total[i]) if total[i] != 0.0 else 1.0
                inherited = max(cond_k[:, i, :].max(), cond_g[:, i, :].max())
                case(f"accident_case.total:{name}", [scenario_id, i], total[i],
                     max(1.0, ratio) * inherited)


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
    gen_release_activity(calc)
    gen_coolant_release(calc)
    gen_inventory_processing(calc)
    gen_nuclide_selection(calc)
    gen_name_normalisation(calc)
    gen_convert_time(calc)
    gen_nuclide_sort(calc)
    gen_nuclides(calc)
    driver, driver_calc = load_upstream_driver()
    gen_accident_case(driver, driver_calc)

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
