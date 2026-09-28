"""HTR-10 k_eff against loading height: this model's single-seed ENDF/B-VII.0
and VIII.0 runs on the fast ENDF path, against Li, Yu & Wei (2014) RMC and the
paper's two MCNP columns.

Usage: python plot_keff_vs_height.py <log dir> <out.png>

Reads the run logs written by `examples/htr10_rmc_keff.rs`
(`fastpath_{e7,e8}_n{20,25,41}.log`). Reads the reference curves from
`crates/nee_soon/src/htr10_rmc/mod.rs` itself, so no number is copied by hand.
Heights are the paper's whole-ball-extent convention (gh:#333), taken from
each log's HEIGHT-MATCHED line.
"""
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


def table(name):
    src = MOD_RS.read_text()
    body = re.search(rf"pub const {name}: &\[\(f64, f64\)\] = &\[(.*?)\];", src, re.S).group(1)
    pts = re.findall(r"\(([\d.]+),\s*([\d_.]+)\)", body)
    return [(float(h), float(k.replace("_", ""))) for h, k in pts]


def run(log):
    t = log.read_text()
    k, s = re.search(r"k_eff\s+=\s+([\d.]+) \+/- ([\d.]+)", t).groups()
    h, rmc = re.search(r"= ([\d.]+) cm ball extent.*RMC\(interp\) = ([\d.]+)", t).groups()
    data = re.search(r"nuclear data :\s+([\d.]+) s", t)
    transport = re.search(r"transport    :\s+([\d.]+) s", t)
    hw = re.search(r"hardware     : (.*)", t)
    return dict(
        k=float(k), s=float(s), h=float(h), rmc=float(rmc),
        data=float(data.group(1)) if data else None,
        transport=float(transport.group(1)) if transport else None,
        hw=hw.group(1).strip() if hw else "hardware not recorded",
    )


def main():
    logdir, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
    runs = {}
    for lib in ("e8", "e7"):
        pts = []
        for n in (20, 25, 41):
            f = logdir / f"fastpath_{lib}_n{n}.log"
            if f.exists() and "k_eff  " in f.read_text():
                pts.append(run(f))
        runs[lib] = sorted(pts, key=lambda r: r["h"])

    rmc, t3, t4 = (table(n) for n in (
        "RMC_KEFF_VS_HEIGHT", "MCNP_TABLE3_KEFF_VS_HEIGHT", "MCNP_TABLE4_KEFF_VS_HEIGHT"))

    plt.rcParams.update({"font.size": 10, "axes.edgecolor": MUTED, "axes.labelcolor": TEXT2,
                         "xtick.color": TEXT2, "ytick.color": TEXT2})
    fig, (ax, ax2) = plt.subplots(2, 1, figsize=(8, 8.2), sharex=True,
                                  gridspec_kw={"height_ratios": [3, 2]}, facecolor=SURFACE)
    for a in (ax, ax2):
        a.set_facecolor(SURFACE)
        a.grid(True, color=GRID, linewidth=0.8)
        a.set_axisbelow(True)
        for sp in ("top", "right"):
            a.spines[sp].set_visible(False)

    # Top: k against height.
    ax.plot(*zip(*rmc), color=INK, lw=2, label="RMC, Li et al. 2014 (reference)")
    ax.plot(*zip(*t3), color=MUTED, lw=1.2, ls="--", label="MCNP, paper Table 3 (gauge)")
    ax.plot(*zip(*t4), color=MUTED, lw=1.2, ls=":", label="MCNP, paper Table 4 (gauge)")
    ax.axhline(1.0, color=MUTED, lw=0.8)
    styles = {"e8": (VIII, "o", "this model, ENDF/B-VIII.0 (30P graphite)"),
              "e7": (VII, "s", "this model, ENDF/B-VII.0 (crystalline graphite)")}
    for lib, (c, m, lab) in styles.items():
        p = runs[lib]
        if not p:
            continue
        ax.errorbar([r["h"] for r in p], [r["k"] for r in p], yerr=[r["s"] for r in p],
                    color=c, marker=m, ms=8, lw=2, capsize=4, mec=SURFACE, mew=1.5, label=lab)
    ax.set_ylabel("k_eff")
    ax.set_title("HTR-10 k_eff vs loading height, fast ENDF path, single seed",
                 loc="left", color=INK, fontsize=12)
    ax.legend(frameon=False, fontsize=9, loc="upper left")

    # Bottom: residual against the height-matched RMC value.
    ax2.axhspan(-1000, 1000, color=GRID, alpha=0.6, lw=0)
    ax2.axhspan(-500, 500, color=GRID, lw=0)
    ax2.axhline(0, color=INK, lw=1)
    ax2.text(96, 760, "±1000 pcm", color=TEXT2, fontsize=8)
    ax2.text(96, 260, "±500 pcm", color=TEXT2, fontsize=8)
    for lib, (c, m, _) in styles.items():
        p = runs[lib]
        if not p:
            continue
        ax2.errorbar([r["h"] for r in p], [(r["k"] - r["rmc"]) * 1e5 for r in p],
                     yerr=[r["s"] * 1e5 for r in p], color=c, marker=m, ms=8, lw=2,
                     capsize=4, mec=SURFACE, mew=1.5)
        last = p[-1]
        ax2.annotate("VIII.0" if lib == "e8" else "VII.0",
                     (last["h"], (last["k"] - last["rmc"]) * 1e5),
                     xytext=(8, 0), textcoords="offset points", va="center", color=TEXT2)
    ax2.set_ylabel("k − RMC [pcm]")
    ax2.set_xlabel("loading height, whole-ball extent [cm] (gh:#333)")
    ax2.set_xlim(90, 212)

    hws = sorted({r["hw"] for p in runs.values() for r in p})
    fig.text(0.01, 0.005,
             "2000 histories × [30 inactive + 70 active], seed 20260917. Error bars: within-run 1σ. "
             "Ni→Fe and Fe-57→Fe-56 in rod steel.\n" + "; ".join(hws),
             fontsize=7, color=TEXT2)
    fig.tight_layout(rect=(0, 0.04, 1, 1))
    fig.savefig(out, dpi=150, facecolor=SURFACE)

    for lib, p in runs.items():
        for r in p:
            print(f"{lib} h={r['h']:.3f} k={r['k']:.6f}±{r['s']:.6f} "
                  f"dk={(r['k'] - r['rmc']) * 1e5:+.0f} pcm data={r['data']} s "
                  f"transport={r['transport']} s | {r['hw']}")


if __name__ == "__main__":
    main()
