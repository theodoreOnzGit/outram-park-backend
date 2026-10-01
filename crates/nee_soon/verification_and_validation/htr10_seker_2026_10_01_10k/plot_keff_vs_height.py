"""HTR-10 k_eff against loading height on Seker & Colak (2003)'s 13-ball bed
(gh:#472), at the reference paper's statistics (10 000 histories x [5 inactive
+ 135 active]): this model's ENDF/B-VII.0 and VIII.0 runs against Li, Yu & Wei
(2014) RMC and the paper's two MCNP columns. Adapted 2026-10-01 from
`../htr10_seker_2026_10_01/plot_keff_vs_height.py`.

Usage: python plot_keff_vs_height.py <log dir> <out dir>

Writes into <out dir>:
  keff_vs_height_endf8_endf7.png   both libraries
  keff_vs_height_endf8.png         ENDF/B-VIII.0 only
  keff_vs_height_endf7.png         ENDF/B-VII.0 only
  results_table.md / results_table.csv

Reads the run logs written by `examples/htr10_rmc_keff.rs`
(`run_{e7,e8}_N{10..20}.log`; N = 9 is not run, see RUN_PARAMETERS.md). Reads the reference curves from
`crates/nee_soon/src/htr10_rmc/mod.rs` itself, so no number is copied by hand.
Each point is placed at the height where Seker's model holds as many balls as
the built bed (gh:#472), taken from each log's HEIGHT-MATCHED line, and the
residual is against RMC and MCNP linearly interpolated there. Error bars are the
within-run 1 sigma printed by the example (single seed per point); the
references carry no quoted uncertainty.
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


def plot(runs, libs, refs, out, title):
    rmc, t3, t4 = refs
    plt.rcParams.update({"font.size": 10, "axes.edgecolor": MUTED, "axes.labelcolor": TEXT2,
                         "xtick.color": TEXT2, "ytick.color": TEXT2})
    fig, (ax, ax2) = plt.subplots(2, 1, figsize=(8, 8.4), sharex=True,
                                  gridspec_kw={"height_ratios": [3, 2]}, facecolor=SURFACE)
    style(fig, (ax, ax2))

    ax.plot(*zip(*rmc), color=INK, lw=2, label="RMC, Li et al. 2014, ENDF/B-VII.0 (reference)")
    ax.plot(*zip(*t3), color=MUTED, lw=1.2, ls="--", label="MCNP, Li Table 3 = Seker vacuum, ENDF/B-VI (gauge)")
    ax.plot(*zip(*t4), color=MUTED, lw=1.2, ls=":", label="MCNP, Li Table 4 = Seker helium, ENDF/B-VI (gauge)")
    ax.axhline(1.0, color=MUTED, lw=0.8)
    # Markers only, no connecting lines (a line would read as a model
    # curve); dodged +/-0.9 cm when both libraries are on one plot.
    dodge = {"e8": -0.9, "e7": 0.9} if len(libs) > 1 else {"e8": 0.0, "e7": 0.0}
    for lib in libs:
        c, m, lab, _ = LIBS[lib]
        p = runs[lib]
        if not p:
            continue
        ax.errorbar([r["h"] + dodge[lib] for r in p], [r["k"] for r in p], yerr=[r["s"] for r in p],
                    color=c, marker=m, ms=8, ls="none", elinewidth=2, capsize=4, mec=SURFACE,
                    mew=1.5, label=lab + ", ±1σ")
    ax.set_ylabel("k_eff")
    ax.set_title(title, loc="left", color=INK, fontsize=12)
    ax.legend(frameon=False, fontsize=8.5, loc="upper left")

    # Bottom: residual against RMC (filled) and MCNP Table 4 / helium (hollow).
    ax2.axhspan(-1000, 1000, color=GRID, alpha=0.6, lw=0)
    ax2.axhspan(-500, 500, color=GRID, lw=0)
    ax2.axhline(0, color=INK, lw=1)
    ax2.text(96, 760, "±1000 pcm", color=TEXT2, fontsize=8)
    ax2.text(96, 260, "±500 pcm", color=TEXT2, fontsize=8)
    for lib in libs:
        c, m, _, short = LIBS[lib]
        p = runs[lib]
        if not p:
            continue
        x = [r["h"] + dodge[lib] for r in p]
        ax2.errorbar(x, [(r["k"] - r["rmc"]) * 1e5 for r in p], yerr=[r["s"] * 1e5 for r in p],
                     color=c, marker=m, ms=8, ls="none", elinewidth=2, capsize=4, mec=SURFACE,
                     mew=1.5, label=f"{short} − RMC")
        ax2.errorbar([v + 0.45 * (1 if dodge[lib] >= 0 else -1) for v in x],
                     [(r["k"] - interp(t4, r["h"])) * 1e5 for r in p], yerr=[r["s"] * 1e5 for r in p],
                     color=c, marker=m, ms=7, ls="none", elinewidth=1, capsize=3, mfc="none",
                     mew=1.3, alpha=0.8, label=f"{short} − MCNP T4 (helium)")
    ax2.set_ylabel("k − reference [pcm]")
    ax2.set_xlabel("loading height at equal ball count with Seker's model [cm] (gh:#472)")
    ax2.set_xlim(90, 212)
    ax2.legend(frameon=False, fontsize=8, loc="lower right", ncol=2)

    allp = [r for lib in libs for r in runs[lib]]
    hws = sorted({r["hw"] for r in allp})
    st = allp[0]["stats"]
    fig.text(0.01, 0.005,
             f"{st[0]} histories × [{st[1]} inactive + {st[2]} active], seed {st[3]}, one seed per point. "
             "Error bars: within-run 1σ (references carry none).\n"
             "Helium coolant, natural C, real Ni/Fe rod steel, 14 rings, Seker 13-ball bed. "
             "Transport on CPU, 5 threads per run.\n"
             "Host: " + "; ".join(re.sub(r"^.*? / \w+, ", "", h) for h in hws)
             + " (the logs also list a GPU; it was detected, not used)",
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

    refs = [table(n) for n in (
        "RMC_KEFF_VS_HEIGHT", "MCNP_TABLE3_KEFF_VS_HEIGHT", "MCNP_TABLE4_KEFF_VS_HEIGHT")]
    _, t3, t4 = refs

    plot(runs, ["e8", "e7"], refs, outdir / "keff_vs_height_endf8_endf7.png",
         "HTR-10 k_eff vs loading height, ENDF/B-VIII.0 and VII.0")
    plot(runs, ["e8"], refs, outdir / "keff_vs_height_endf8.png",
         "HTR-10 k_eff vs loading height, ENDF/B-VIII.0")
    plot(runs, ["e7"], refs, outdir / "keff_vs_height_endf7.png",
         "HTR-10 k_eff vs loading height, ENDF/B-VII.0")

    rows = []
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
    with open(outdir / "results_table.csv", "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0]))
        w.writeheader()
        w.writerows(rows)

    md = []
    for lib in ("VIII.0", "VII.0"):
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
    print("\n".join(md))
    for r in rows:
        print(f"{r['library']} N={r['N']} lost={r['lost_locates']} data={r['data_s']} s "
              f"transport={r['transport_s']} s")


if __name__ == "__main__":
    main()
