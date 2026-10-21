"""HTR-10 k_eff against loading height on Seker & Colak (2003)'s 13-ball bed,
the full 22-run sweep at 10 000 x [5 + 135] re-measured on the bounded
delta-tracking majorant (GitHub #589), against the superseded 2026-10-01 record
at the same statistics (`../htr10_seker_2026_10_01_10k/`), Li, Yu & Wei (2014)
RMC and the paper's two MCNP columns. Copied 2026-10-07 from
`../htr10_seker_2026_10_05_majorant_fix/plot_keff_vs_height.py` with labels
changed only; the reference and shift handling are unchanged.

Usage: python plot_keff_vs_height.py <log dir> <out dir>

Writes into <out dir>:
  keff_vs_height_endf8_endf7.png   both libraries (new filled, old hollow)
  keff_vs_height_endf8.png         ENDF/B-VIII.0 only
  keff_vs_height_endf7.png         ENDF/B-VII.0 only (if a VII.0 run exists)
  results_table.md / results_table.csv   new points, same columns as the old record
  shift_table.md / shift_table.csv       new minus old at each re-measured N
  summary.md                             mean shift, and residual statistics new vs old at the same N

Reads `run_{e7,e8}_N{10..20}.log` (the bounded-majorant runs) and
`run_e8_N14_control_old_majorant.log` (same code, old majorant) if present. The old
k come from the old record's `results_table.csv`, never retyped. Each point is
placed at the height where Seker's model holds as many balls as the built bed
(gh:#472); residuals use the references linearly interpolated there. Error
bars are within-run 1 sigma; the references carry none.
"""
import csv
import pathlib
import re
import sys

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

HERE = pathlib.Path(__file__).resolve().parent
MOD_RS = HERE.parents[1] / "src" / "htr10_rmc" / "mod.rs"
OLD_CSV = HERE.parent / "htr10_seker_2026_10_01_10k" / "results_table.csv"

# Reference palette (dataviz skill, light mode). Slots 1-2 for the two
# libraries; the references are ink and muted grey, not categorical hues.
VIII = "#2a78d6"
VII = "#eb6834"
INK = "#0b0b0b"
MUTED = "#8a8985"
GRID = "#e4e3df"
SURFACE = "#fcfcfb"
TEXT2 = "#52514e"

LIBS = {"e8": (VIII, "o", "this model, ENDF/B-VIII.0 (30P graphite)", "VIII.0"),
        "e7": (VII, "s", "this model, ENDF/B-VII.0 (crystalline graphite)", "VII.0")}


def table(name):
    src = MOD_RS.read_text()
    body = re.search(rf"pub const {name}: &\[\(f64, f64\)\] = &\[(.*?)\];", src, re.S).group(1)
    pts = re.findall(r"\(([\d.]+),\s*([\d_.]+)\)", body)
    return [(float(h), float(k.replace("_", ""))) for h, k in pts]


def interp(tab, h):
    for (h0, k0), (h1, k1) in zip(tab, tab[1:]):
        if h0 <= h <= h1:
            return k0 + (k1 - k0) * (h - h0) / (h1 - h0)
    raise ValueError(f"height {h} outside the tabulated range")


def run(log, n):
    t = log.read_text()
    k, s = re.search(r"k_eff\s+=\s+([\d.]+) \+/- ([\d.]+)", t).groups()
    hb, h, rmc = re.search(
        r"HEIGHT-MATCHED: bed ([\d.]+) cm built; reference read at ([\d.]+) cm.*RMC\(interp\) = ([\d.]+)",
        t).groups()
    stats = re.search(r"(\d+) histories x \[(\d+) inactive \+ (\d+) active\], seed (\d+)", t)
    data = re.search(r"nuclear data :\s+([\d.]+) s", t)
    transport = re.search(r"transport    :\s+([\d.]+) s", t)
    hw = re.search(r"hardware     : (.*)", t)
    lost = re.search(r"lost locate  = (\d+)", t)
    return dict(
        n=n, k=float(k), s=float(s), hb=float(hb), h=float(h), rmc=float(rmc),
        stats=stats.groups() if stats else None,
        data=float(data.group(1)) if data else None,
        transport=float(transport.group(1)) if transport else None,
        hw=hw.group(1).strip() if hw else "hardware not recorded",
        lost=int(lost.group(1)) if lost else None,
    )


def style(fig, axes):
    for a in axes:
        a.set_facecolor(SURFACE)
        a.grid(True, color=GRID, linewidth=0.8)
        a.set_axisbelow(True)
        for sp in ("top", "right"):
            a.spines[sp].set_visible(False)


def old_points():
    """The 2026-10-01 record's points, from its own CSV, keyed by (lib, N)."""
    out = {}
    with open(OLD_CSV) as f:
        for row in csv.DictReader(f):
            lib = "e8" if row["library"] == "VIII.0" else "e7"
            out[(lib, int(row["N"]))] = dict(n=int(row["N"]), k=float(row["k"]), s=float(row["sigma"]),
                                             h=float(row["ref_height_cm"]), rmc=float(row["rmc"]))
    return out


def plot(runs, old, libs, refs, out, title, control=None):
    rmc, t3, t4 = refs
    plt.rcParams.update({"font.size": 10, "axes.edgecolor": MUTED, "axes.labelcolor": TEXT2,
                         "xtick.color": TEXT2, "ytick.color": TEXT2})
    fig, (ax, ax2) = plt.subplots(2, 1, figsize=(8, 8.8), sharex=True,
                                  gridspec_kw={"height_ratios": [3, 2]}, facecolor=SURFACE)
    style(fig, (ax, ax2))

    ax.plot(*zip(*rmc), color=INK, lw=2, label="RMC, Li et al. 2014, ENDF/B-VII.0 (reference)")
    ax.plot(*zip(*t3), color=MUTED, lw=1.2, ls="--", label="MCNP, Li Table 3 = Seker vacuum, ENDF/B-VI (gauge)")
    ax.plot(*zip(*t4), color=MUTED, lw=1.2, ls=":", label="MCNP, Li Table 4 = Seker helium, ENDF/B-VI (gauge)")
    ax.axhline(1.0, color=MUTED, lw=0.8)
    dodge = {"e8": -0.9, "e7": 0.9} if len(libs) > 1 else {"e8": 0.0, "e7": 0.0}
    for lib in libs:
        c, m, lab, short = LIBS[lib]
        p = runs[lib]
        if not p:
            continue
        # Old (superseded) points at the same N only: hollow, faded, shifted
        # 1.6 cm left so the error bars do not overlap.
        o = [old[(lib, r["n"])] for r in p if (lib, r["n"]) in old]
        ax.errorbar([r["h"] + dodge[lib] - 1.6 for r in o], [r["k"] for r in o], yerr=[r["s"] for r in o],
                    color=c, marker=m, ms=7, ls="none", elinewidth=1, capsize=3, mfc="none",
                    mew=1.2, alpha=0.45, label=f"{short}, 2026-10-01 record, OLD under-bound majorant (superseded)")
        ax.errorbar([r["h"] + dodge[lib] for r in p], [r["k"] for r in p], yerr=[r["s"] for r in p],
                    color=c, marker=m, ms=8, ls="none", elinewidth=2, capsize=4, mec=SURFACE,
                    mew=1.5, label=lab + ", bounded majorant (2026-10-07), ±1σ")
    if control and "e8" in libs:
        ax.errorbar([control["h"] + 1.6], [control["k"]], yerr=[control["s"]], color=TEXT2, marker="D",
                    ms=7, ls="none", elinewidth=1.5, capsize=3, mfc="none", mew=1.4,
                    label="control: VIII.0, today's code, OLD majorant (ablation)")
    ax.set_ylabel("k_eff")
    ax.set_title(title, loc="left", color=INK, fontsize=12)
    ax.legend(frameon=False, fontsize=7.5, loc="upper left")

    ax2.axhspan(-1000, 1000, color=GRID, alpha=0.6, lw=0)
    ax2.axhspan(-500, 500, color=GRID, lw=0)
    ax2.axhline(0, color=INK, lw=1)
    ax2.text(96, 760, "±1000 pcm", color=TEXT2, fontsize=8)
    ax2.text(96, 260, "±500 pcm", color=TEXT2, fontsize=8)
    lo = -1000
    for lib in libs:
        c, m, _, short = LIBS[lib]
        p = runs[lib]
        if not p:
            continue
        o = [old[(lib, r["n"])] for r in p if (lib, r["n"]) in old]
        ax2.errorbar([r["h"] + dodge[lib] - 1.6 for r in o], [(r["k"] - r["rmc"]) * 1e5 for r in o],
                     yerr=[r["s"] * 1e5 for r in o], color=c, marker=m, ms=7, ls="none", elinewidth=1,
                     capsize=3, mfc="none", mew=1.2, alpha=0.45, label=f"{short} − RMC, old (superseded)")
        d = [(r["k"] - r["rmc"]) * 1e5 for r in p]
        lo = min(lo, min(v - r["s"] * 1e5 for v, r in zip(d, p)))
        ax2.errorbar([r["h"] + dodge[lib] for r in p], d, yerr=[r["s"] * 1e5 for r in p],
                     color=c, marker=m, ms=8, ls="none", elinewidth=2, capsize=4, mec=SURFACE,
                     mew=1.5, label=f"{short} − RMC, new")
    if control and "e8" in libs:
        ax2.errorbar([control["h"] + 1.6], [(control["k"] - control["rmc"]) * 1e5], yerr=[control["s"] * 1e5],
                     color=TEXT2, marker="D", ms=7, ls="none", elinewidth=1.5, capsize=3, mfc="none",
                     mew=1.4, label="control − RMC (old majorant)")
    ax2.set_ylim(lo - 300, 1200)
    ax2.set_ylabel("k − RMC [pcm]")
    ax2.set_xlabel("loading height at equal ball count with Seker's model [cm] (gh:#472)")
    ax2.set_xlim(90, 212)
    ax2.legend(frameon=False, fontsize=7.5, loc="lower right", ncol=2)

    allp = [r for lib in libs for r in runs[lib]]
    hws = sorted({r["hw"] for r in allp})
    st = allp[0]["stats"]
    fig.text(0.01, 0.005,
             f"New: {st[0]} histories × [{st[1]} inactive + {st[2]} active], seed {st[3]}, one seed per point, "
             "7 threads. Old: 10 000 × [5 + 135], same seed.\n"
             "Error bars: within-run 1σ (references carry none). Helium coolant, natural C, real Ni/Fe rod steel, "
             "14 rings, Seker 13-ball bed.\n"
             "Host (new): " + "; ".join(re.sub(r"^.*? / \w+, ", "", h) for h in hws),
             fontsize=7, color=TEXT2)
    fig.tight_layout(rect=(0, 0.065, 1, 1))
    fig.savefig(out, dpi=150, facecolor=SURFACE)
    plt.close(fig)


def main():
    logdir, outdir = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
    outdir.mkdir(parents=True, exist_ok=True)
    runs = {}
    for lib in ("e8", "e7"):
        pts = []
        for n in range(10, 21):
            f = logdir / f"run_{lib}_N{n}.log"
            if f.exists() and "k_eff  " in f.read_text():
                pts.append(run(f, n))
        runs[lib] = sorted(pts, key=lambda r: r["h"])
    cf = logdir / "run_e8_N14_control_old_majorant.log"
    control = run(cf, 14) if cf.exists() and "k_eff  " in cf.read_text() else None
    old = old_points()

    refs = [table(n) for n in (
        "RMC_KEFF_VS_HEIGHT", "MCNP_TABLE3_KEFF_VS_HEIGHT", "MCNP_TABLE4_KEFF_VS_HEIGHT")]
    _, t3, t4 = refs

    libs = [lib for lib in ("e8", "e7") if runs[lib]]
    plot(runs, old, libs, refs, outdir / "keff_vs_height_endf8_endf7.png",
         "HTR-10 k_eff vs loading height, bounded majorant (#589)", control)
    plot(runs, old, ["e8"], refs, outdir / "keff_vs_height_endf8.png",
         "HTR-10 k_eff vs loading height, ENDF/B-VIII.0, bounded majorant (#589)", control)
    if runs["e7"]:
        plot(runs, old, ["e7"], refs, outdir / "keff_vs_height_endf7.png",
             "HTR-10 k_eff vs loading height, ENDF/B-VII.0, bounded majorant (#589)")

    rows, shifts = [], []
    for lib in ("e8", "e7"):
        for r in runs[lib]:
            m3, m4 = interp(t3, r["h"]), interp(t4, r["h"])
            rows.append(dict(
                library=LIBS[lib][3], N=r["n"], built_height_cm=f"{r['hb']:.3f}",
                ref_height_cm=f"{r['h']:.3f}", k=f"{r['k']:.6f}", sigma=f"{r['s']:.6f}",
                rmc=f"{r['rmc']:.6f}", mcnp_t3_vacuum=f"{m3:.6f}", mcnp_t4_helium=f"{m4:.6f}",
                d_rmc_pcm=f"{(r['k'] - r['rmc']) * 1e5:+.0f}",
                d_t3_pcm=f"{(r['k'] - m3) * 1e5:+.0f}", d_t4_pcm=f"{(r['k'] - m4) * 1e5:+.0f}",
                sigma_pcm=f"{r['s'] * 1e5:.0f}",
                n_sigma_rmc=f"{(r['k'] - r['rmc']) / r['s']:+.2f}",
                lost_locates=r["lost"], data_s=r["data"], transport_s=r["transport"]))
            o = old.get((lib, r["n"]))
            if o:
                sc = (r["s"] ** 2 + o["s"] ** 2) ** 0.5
                shifts.append(dict(
                    library=LIBS[lib][3], N=r["n"], old_k=f"{o['k']:.6f}", old_sigma=f"{o['s']:.6f}",
                    new_k=f"{r['k']:.6f}", new_sigma=f"{r['s']:.6f}",
                    shift_pcm=f"{(r['k'] - o['k']) * 1e5:+.0f}", combined_sigma_pcm=f"{sc * 1e5:.0f}",
                    shift_n_sigma=f"{(r['k'] - o['k']) / sc:+.2f}",
                    old_d_rmc_pcm=f"{(o['k'] - o['rmc']) * 1e5:+.0f}",
                    new_d_rmc_pcm=f"{(r['k'] - r['rmc']) * 1e5:+.0f}",
                    new_d_t3_pcm=f"{(r['k'] - m3) * 1e5:+.0f}", new_d_t4_pcm=f"{(r['k'] - m4) * 1e5:+.0f}"))
    if control:
        o = old[("e8", 14)]
        sc = (control["s"] ** 2 + o["s"] ** 2) ** 0.5
        shifts.append(dict(
            library="VIII.0 control (old majorant, today's code)", N=14, old_k=f"{o['k']:.6f}",
            old_sigma=f"{o['s']:.6f}", new_k=f"{control['k']:.6f}", new_sigma=f"{control['s']:.6f}",
            shift_pcm=f"{(control['k'] - o['k']) * 1e5:+.0f}", combined_sigma_pcm=f"{sc * 1e5:.0f}",
            shift_n_sigma=f"{(control['k'] - o['k']) / sc:+.2f}",
            old_d_rmc_pcm=f"{(o['k'] - o['rmc']) * 1e5:+.0f}",
            new_d_rmc_pcm=f"{(control['k'] - control['rmc']) * 1e5:+.0f}",
            new_d_t3_pcm=f"{(control['k'] - interp(t3, control['h'])) * 1e5:+.0f}",
            new_d_t4_pcm=f"{(control['k'] - interp(t4, control['h'])) * 1e5:+.0f}"))
    with open(outdir / "results_table.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]))
        w.writeheader()
        w.writerows(rows)
    with open(outdir / "shift_table.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(shifts[0]))
        w.writeheader()
        w.writerows(shifts)

    md = []
    for lib in ("VIII.0", "VII.0"):
        if not any(r["library"] == lib for r in rows):
            continue
        md.append(f"### ENDF/B-{lib}\n")
        md.append("| N | built [cm] | ref. height [cm] | k ± 1σ | RMC | MCNP T3 (vac) | MCNP T4 (He) "
                  "| k − RMC [pcm] | k − T3 [pcm] | k − T4 [pcm] | (k − RMC)/σ |")
        md.append("|---|---|---|---|---|---|---|---|---|---|---|")
        for r in rows:
            if r["library"] != lib:
                continue
            md.append(f"| {r['N']} | {r['built_height_cm']} | {r['ref_height_cm']} | {r['k']} ± {r['sigma']} "
                      f"| {r['rmc']} | {r['mcnp_t3_vacuum']} | {r['mcnp_t4_helium']} "
                      f"| {r['d_rmc_pcm']} ± {r['sigma_pcm']} | {r['d_t3_pcm']} | {r['d_t4_pcm']} "
                      f"| {r['n_sigma_rmc']} |")
        md.append("")
    (outdir / "results_table.md").write_text("\n".join(md))
    sm = ["| library | N | old k ± 1σ | new k ± 1σ | new − old [pcm] | combined σ [pcm] | (new − old)/σ "
          "| old − RMC | new − RMC | new − MCNP T3 | new − MCNP T4 |",
          "|---|---|---|---|---|---|---|---|---|---|---|"]
    for r in shifts:
        sm.append(f"| {r['library']} | {r['N']} | {r['old_k']} ± {r['old_sigma']} | {r['new_k']} ± {r['new_sigma']} "
                  f"| {r['shift_pcm']} | {r['combined_sigma_pcm']} | {r['shift_n_sigma']} | {r['old_d_rmc_pcm']} "
                  f"| {r['new_d_rmc_pcm']} | {r['new_d_t3_pcm']} | {r['new_d_t4_pcm']} |")
    (outdir / "shift_table.md").write_text("\n".join(sm) + "\n")
    # Summary: weighted mean shift per library (chi2 about it), and residual
    # statistics new vs old at the SAME N (old record restricted to those N).
    su = ["| library | points | weighted mean shift [pcm] | ±1σ | χ² / dof about the mean |",
          "|---|---|---|---|---|"]
    for lib in ("VIII.0", "VII.0"):
        sh = [r for r in shifts if r["library"] == lib]
        if not sh:
            continue
        v = [float(r["shift_pcm"]) for r in sh]
        e = [float(r["combined_sigma_pcm"]) for r in sh]
        w = [1 / x ** 2 for x in e]
        m = sum(a * b for a, b in zip(v, w)) / sum(w)
        chi = sum(((a - m) / b) ** 2 for a, b in zip(v, e))
        su.append(f"| {lib} | {len(sh)} | {m:+.0f} | {sum(w) ** -0.5:.0f} | "
                  + (f"{chi:.2f} / {len(sh) - 1} |" if len(sh) > 1 else "n/a |"))
    su += ["", "| library | which | vs | mean [pcm] | RMS [pcm] | max abs [pcm] | within ±500 | within ±1000 |",
           "|---|---|---|---|---|---|---|---|"]
    for lib, key in (("VIII.0", "e8"), ("VII.0", "e7")):
        p = runs[key]
        if not p:
            continue
        o = [old[(key, r["n"])] for r in p]
        for which, pts in (("new (bounded)", p), ("old, same N", o)):
            for vs, ref in (("RMC", None), ("MCNP T3", t3), ("MCNP T4", t4)):
                d = [(r["k"] - (r["rmc"] if ref is None else interp(ref, r["h"]))) * 1e5 for r in pts]
                su.append(f"| {lib} | {which} | {vs} | {sum(d) / len(d):+.0f} | {(sum(x * x for x in d) / len(d)) ** 0.5:.0f} "
                          f"| {max(abs(x) for x in d):.0f} | {sum(abs(x) <= 500 for x in d)}/{len(d)} "
                          f"| {sum(abs(x) <= 1000 for x in d)}/{len(d)} |")
    (outdir / "summary.md").write_text("\n".join(su) + "\n")
    print("\n".join(md))
    print("\n".join(sm))
    print("\n".join(su))
    for r in rows:
        print(f"{r['library']} N={r['N']} lost={r['lost_locates']} data={r['data_s']} s "
              f"transport={r['transport_s']} s")


if __name__ == "__main__":
    main()
