# Part III — double heterogeneity: explicit TRISO vs ring-RPT in an FHR pebble

**Scope:** the doubly heterogeneous problem only. Cross-section preparation,
transport-kernel verification and the classical ICSBEP criticals are **Part II**
and are cited, not repeated.

**Source record:**
`crates/outram-mc-libs/verification_and_validation/ring_rpt/ring_rpt_vs_openmc.md`
(1,547 lines) — Results and Interpretation sections.

## The contribution

A reactivity-equivalent physical transformation (RPT) replaces the stochastic
TRISO distribution in a pebble's fuel zone with a homogeneous fuel *shell* at a
tuned inner radius. The question is whether that equivalence reproduces in an
independent code, and what it costs.

| configuration | RPT − explicit | σ-distance |
|---|---|---|
| reflective sphere, both defects fixed | **+226 ± 316 pcm** | 0.71σ |
| sphere, packing-fraction defect present | +1130 ± 319 pcm | 3.54σ |
| sphere, before free-gas target motion too | +163 ± 281 pcm | 0.58σ |
| OpenMC reference (deck author) | −31 ± 92 pcm | 0.34σ |

## The finding that must lead, not be buried

**The `+163 ± 316` pcm agreement was coincidental.** Before either fix it read
as excellent — 0.58σ, a result most authors would have published. It was two
defects of opposite sign cancelling:

- missing free-gas target motion in the thermal kernel (`op-50vu`)
- an explicit pebble packed **2.6 % over the deck's own definition** (`op-8l2e`)

Fixing one exposed the other at `+1130 ± 319` (3.54σ). Fixing both gives
`+226 ± 316` (0.71σ). **An agreeing number is not evidence of a correct model**,
and this is a clean, quantified demonstration of it on a real benchmark.

That is the most transferable result in the paper and it should be in the
abstract. It is also the natural bridge to the human-in-the-loop V&V manuscript
(`../human-in-the-loop-vv/`), where the same lesson appears as a defect class.

## Why the equivalence claim survives Part II's bad news

Part II reports a `+2950 ± 61` pcm absolute-k discrepancy on thermal,
strongly-self-shielded, heterogeneous U-238 (LCT-008), and the FHR pebble
inherits it — the explicit pebble reads `+4004 pcm` absolute against the
reference deck.

**The RPT equivalence is a *difference* between two models sharing identical
nuclear data, so that bias largely cancels.** Say this explicitly and early: the
paper's headline result is insensitive to the absolute-k limitation established
in Part II, which is precisely why the two papers split cleanly. Do not
overstate it — "largely cancels" is the honest phrasing, not "cancels."

## Files

| File | Contents |
|---|---|
| `ring_rpt_equivalence_history.csv` | RPT − explicit across the two-defect fix history |
| `mechanism_elimination.csv` | 21 mechanisms × oracle × measured bound × excluded? |

`mechanism_elimination.csv` belongs to this paper because it is the pebble
hunt's own table — but its ICSBEP and NJOY rows are **results imported from Part
II**, cited rather than re-derived. Mark them as such in the caption; presenting
Part II's measurements as new work here would be self-plagiarism.

## Also belongs here

- **Two transport defects found by this work**, both fixed: GH #168, a
  concentric-sphere CSG leak (k 0.24 → 1.39, leakage 0.87 → 1e-4), root-caused
  to cell membership being re-derived from a floating-point sense evaluation at
  a point lying exactly on the surface; and GH #169, charged-particle
  disappearance channels (MT 103–117) missing from absorption — Li-6(n,t)α came
  back as 0.04 b against a true 938 b.
- **Delta (Woodcock) tracking for doubly heterogeneous media**, licensed by Part
  II's `18 pcm, 0.05σ` delta-vs-CSG equivalence.
- **The RPT radius is a fitted, code-dependent parameter.** The deck author
  fitted 1.493359375 cm against OpenMC. Quoting `RPT − explicit` at a radius
  tuned for another code measures *that* code's fit, not this one's method
  error — the example carries a `--search-rpt-radius` mode for exactly this, and
  the gap between the two radii is the honest statement of code-dependence.

## Publication order

Cite Part II by arXiv number. Post Part II first, wait for the identifier, then
post this. See `../part2-data-and-transport/README.md`.

## Regenerating

```bash
cargo run --release -p outram-mc-libs --features endf-pebble-cases \
    --example fhr_ring_rpt_endf
cargo run --release -p outram-mc-libs --features endf-pebble-cases \
    --example fhr_ring_rpt_endf -- --search-rpt-radius
```

**Not re-run to build this dataset** — extracted from the committed record.
