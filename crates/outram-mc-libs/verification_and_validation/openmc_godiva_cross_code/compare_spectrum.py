#!/usr/bin/env python3
"""Difference Godiva's flux spectrum between this crate and OpenMC.

The last measurement of the Godiva investigation. Everything integrated has
been compared already -- cross sections, nu-bar, elastic <mu>, chi, k_inf,
k_eff -- and what survives is a ~69 +/- 23 pcm *relative* k_inf excess, i.e. a
residual that is still there with the boundary removed and is therefore
spectral. An eigenvalue cannot say WHERE in energy that comes from. A spectrum
can, and the energy where the two codes part company names the reaction
responsible.

Both sides tally track-length flux on the SAME 50 log bins over 1e-3 .. 2e7 eV
and are normalised to unit integral: neither carries a volume or power
normalisation, so only the SHAPE is meaningful.

Multiple seeds per side are required, not optional. A single seed pair cannot
separate a real spectral difference from re-randomisation -- the per-bin
seed-to-seed scatter at these statistics is comparable to the differences being
looked for. Give at least three files per side; the script uses the
seed-to-seed spread as the uncertainty and reports each bin's difference in
sigma.

    # generate: 4 seeds each side
    for s in 1 2 3 4; do
      OURS_SEED=$s OURS_SPECTRUM=/tmp/ours_spec_$s.csv \
        cargo run --release -p outram-mc-libs --example godiva_spectrum_vs_openmc
      OPENMC_SPECTRUM=/tmp/omc_spec_$s.csv python3 godiva.py --spectrum --seed=$s
    done

    # compare
    python3 compare_spectrum.py --ours /tmp/ours_spec_*.csv \
                                --openmc /tmp/omc_spec_*.csv
"""
import sys
import math
import argparse


def load(path):
    """(lo, hi, value) triples from one `e_lo,e_hi,flux_norm` CSV."""
    rows = []
    with open(path) as f:
        f.readline()  # header
        for line in f:
            line = line.strip()
            if not line:
                continue
            lo, hi, v = line.split(",")
            rows.append((float(lo), float(hi), float(v)))
    return rows


def load_many(paths):
    """Per-bin (mean, sem, n) over several seeds, plus the shared bin edges.

    Asserts every file shares the same grid -- differencing spectra on
    different edges compares rebinning, not physics.
    """
    runs = [load(p) for p in paths]
    n = len(runs)
    assert n >= 1, "no input files"
    edges = [(lo, hi) for lo, hi, _ in runs[0]]
    for r, p in zip(runs[1:], paths[1:]):
        assert len(r) == len(runs[0]), f"{p}: bin count differs"
        for (lo, hi, _), (a, b) in zip(r, edges):
            assert abs(lo - a) / max(a, 1e-30) < 1e-9, f"{p}: bin edges differ"
            assert abs(hi - b) / max(b, 1e-30) < 1e-9, f"{p}: bin edges differ"
    out = []
    for i in range(len(edges)):
        vals = [r[i][2] for r in runs]
        m = sum(vals) / n
        if n > 1:
            sd = math.sqrt(sum((v - m) ** 2 for v in vals) / (n - 1))
            sem = sd / math.sqrt(n)
        else:
            sem = 0.0
        out.append((m, sem, n))
    return edges, out



# ---------------------------------------------------------------------------
# Aggregate hardness statistics.
#
# The per-bin table above is the diagnostic -- it says WHERE -- but it is not
# the statistic that resolves the question. Each bin is one number out of
# fifty, so the per-bin sigmas are individually unconvincing and collectively
# un-poolable (they are not independent: the spectra are normalised, so a
# deficit somewhere forces an excess elsewhere). A handful of scalars that
# integrate the whole shape do resolve it, and each carries a single honest
# seed-to-seed uncertainty.
# ---------------------------------------------------------------------------

def hardness(rows):
    """Scalar hardness measures of one normalised spectrum.

    Bin midpoints are the geometric mean of the edges -- the natural midpoint
    on a log axis, and the same convention the tally grid was built on.
    """
    m_e = m_ln = below = above = tot = 0.0
    for lo, hi, v in rows:
        mid = math.sqrt(lo * hi)
        m_e += v * mid
        m_ln += v * math.log(mid)
        tot += v
        if hi <= 3.0e5:          # below the 300 keV deficit seen in the table
            below += v
        if lo >= 4.8e6:          # the high-energy excess
            above += v
    return {
        "mean E (eV)": m_e / tot,
        "mean ln E": m_ln / tot,
        "fraction below 300 keV": below / tot,
        "fraction above 4.8 MeV": above / tot,
    }


def mean_sem(xs):
    n = len(xs)
    m = sum(xs) / n
    if n < 2:
        return m, 0.0
    sd = math.sqrt(sum((v - m) ** 2 for v in xs) / (n - 1))
    return m, sd / math.sqrt(n)


def report_hardness(ours_paths, theirs_paths):
    a_runs = [hardness(load(p)) for p in ours_paths]
    b_runs = [hardness(load(p)) for p in theirs_paths]
    keys = list(a_runs[0].keys())
    print()
    print("aggregate hardness (uncertainties are seed-to-seed):")
    print(f"  {'quantity':>24} {'ours':>13} {'OpenMC':>13} {'difference':>22} {'sigma':>7}")
    print("  " + "-" * 84)
    for k in keys:
        am, ase = mean_sem([r[k] for r in a_runs])
        bm, bse = mean_sem([r[k] for r in b_runs])
        d = am - bm
        sig = math.sqrt(ase * ase + bse * bse)
        nsig = abs(d) / sig if sig > 0 else float("inf")
        rel = d / bm * 100.0 if bm else float("nan")
        print(f"  {k:>24} {am:13.6g} {bm:13.6g} "
              f"{d:+12.5g} ({rel:+6.2f}%) {nsig:7.1f}")


def main(argv):
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--ours", nargs="+", required=True,
                    help="one CSV per seed from godiva_spectrum_vs_openmc")
    ap.add_argument("--openmc", nargs="+", required=True,
                    help="one CSV per seed from godiva.py --spectrum")
    ap.add_argument("--min-share", type=float, default=0.005,
                    help="only flag bins carrying at least this fraction of the "
                         "flux (default 0.005 = 0.5%%) -- a huge relative "
                         "difference in an empty bin means nothing")
    ap.add_argument("--flag-sigma", type=float, default=3.0,
                    help="flag a bin at or beyond this many sigma (default 3)")
    args = ap.parse_args(argv)

    edges_o, ours = load_many(args.ours)
    edges_t, theirs = load_many(args.openmc)
    assert len(edges_o) == len(edges_t), "bin count differs between the two sides"
    for (a, _), (b, _) in zip(edges_o, edges_t):
        assert abs(a - b) / max(b, 1e-30) < 1e-9, \
            "bin EDGES differ between the two sides -- comparing rebinning, not physics"

    n_ours = ours[0][2]
    n_theirs = theirs[0][2]
    print(f"ours: {n_ours} seed(s)   OpenMC: {n_theirs} seed(s)   "
          f"(uncertainties are seed-to-seed, not the per-run tally sigma)")
    print()
    print(f"{'E_lo (eV)':>12} {'E_hi (eV)':>12} {'ours':>10} {'OpenMC':>10} "
          f"{'rel diff':>10} {'sigma':>7} {'share':>7}")
    print("-" * 76)

    shifted = 0.0
    flagged = []
    for (lo, hi), (o, so, _), (t, st, _) in zip(edges_o, ours, theirs):
        if o <= 0 and t <= 0:
            continue
        shifted += abs(o - t)
        if t <= 0:
            print(f"{lo:12.3e} {hi:12.3e} {o:10.5f} {t:10.5f} "
                  f"{'n/a':>10} {'n/a':>7} {0.0:7.2%}")
            continue
        rel = (o - t) / t
        sig = math.sqrt(so * so + st * st)
        nsig = abs(o - t) / sig if sig > 0 else float("inf")
        mark = ""
        if t >= args.min_share and nsig >= args.flag_sigma:
            mark = "  <<<"
            flagged.append((lo, hi, rel, nsig, t))
        print(f"{lo:12.3e} {hi:12.3e} {o:10.5f} {t:10.5f} "
              f"{rel:+10.2%} {nsig:7.1f} {t:7.2%}{mark}")

    print()
    print(f"total absolute shape difference (sum |ours - OpenMC| over bins) = {shifted:.4f}")
    print(f"  i.e. ~{shifted / 2:.2%} of the spectrum sits in different bins between the two codes")
    if flagged:
        print(f"  bins carrying >={args.min_share:.1%} of the flux and differing "
              f"by >={args.flag_sigma:g} sigma:")
        for lo, hi, rel, nsig, share in flagged:
            print(f"    {lo:.3e} .. {hi:.3e} eV: {rel:+.2%} "
                  f"({nsig:.1f} sigma, {share:.2%} of the flux)")
    else:
        print(f"  no bin carrying >={args.min_share:.1%} of the flux differs by "
              f">={args.flag_sigma:g} sigma")

    report_hardness(args.ours, args.openmc)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
