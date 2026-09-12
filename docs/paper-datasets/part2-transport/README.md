# Part II — Monte Carlo transport and criticality benchmarks

**Source record:** `crates/outram-mc-libs/verification_and_validation/ring_rpt/ring_rpt_vs_openmc.md`
(1,547 lines), plus `.../icsbep/README.md` for the benchmark specifications and
their provenance.

## The argument the data supports

The paper is not "we ported OpenMC and here is a k-eff." It is a **defect
localisation**, and the benchmark set is a designed experiment rather than a
list of cases that happened to be available.

An FHR TRISO pebble came out **+4004 pcm** against the reference deck. Rather
than tune anything, four ICSBEP benchmarks were chosen to span a 2 x 2 in the
two variables that could plausibly carry the error — **neutron spectrum** and
**U-238 loading** — with heterogeneity as the third axis:

| | low U-238 (5 % of HM) | high U-238 (83–97.5 %) |
|---|---|---|
| **fast** | Godiva `+57 ± 173` | Jemima `+6 ± 173` |
| **thermal** | HST-009 `−18 ± 171` | LCT-008 **`+2950 ± 61`** |

Three corners agree with measured criticals. The fourth does not, and it is the
only one that is thermal **and** strongly self-shielded in U-238 **and**
heterogeneous. That is the localisation, and it is made from experiments rather
than from another code — which matters, because a code-to-code comparison
cannot distinguish "we are wrong" from "they are wrong."

The boron series then shows the residual **tracks the poison**: across LCT-008
cases 1/2/8 the discrepancy moves `+2950 → +2271 → +1713` as soluble boron falls
`1511 → 1335.5 → 794` ppm — a spread of `1237 ± 86` pcm at **14σ**, on cases
sharing one pin cell. A pure cross-section error would give the same Δk
everywhere.

`mechanism_elimination.csv` is the centrepiece table: 21 candidate mechanisms,
each with the oracle used to bound it and the measured bound. Most are excluded
against **analytic limits or NJOY2016**, not against opinion.

## Files

| File | Contents |
|---|---|
| `icsbep_benchmarks.csv` | the 2 x 2 — benchmark, spectrum, U-238 share, fuel form, Δk, σ |
| `lct008_boron_series.csv` | the poison trend across LCT-008 cases 1, 2, 8 |
| `data_prep_verification.csv` | the 10-row NJOY2016 / analytic data-prep section |
| `mechanism_elimination.csv` | 21 mechanisms × oracle × measured bound × excluded? |
| `ring_rpt_equivalence_history.csv` | RPT − explicit across the two-defect fix history |

`data_prep_verification.csv` overlaps `mechanism_elimination.csv` by
construction — see the scope section below. The overlap is the point, not
duplication to resolve.

## Two findings that must not be softened in the writing

1. **The +163 pcm agreement was coincidental.** Before either fix, RPT −
   explicit read `+163 ± 316` pcm — apparently excellent. It was two defects of
   opposite sign cancelling: missing free-gas target motion (`op-50vu`) and a
   packing fraction 2.6 % over the deck (`op-8l2e`). Fixing one exposed the
   other at `+1130 ± 319` (3.54σ); fixing both gives `+226 ± 316` (0.71σ). An
   agreeing number is not evidence of a correct model, and this is the cleanest
   demonstration of that in the repository. It belongs in the paper.
2. **A real defect was found that is *not* the one being hunted.** The thermal
   kernel's second moment is off by `−11 %` to `+39 %` against NJOY's THERMR
   MF=6 matrix — genuinely wrong, and worth `−63 pcm`. Too small to explain
   `+2950`. Report it as found and as insufficient; folding it into the headline
   would overstate the diagnosis.

## Nuclear data preparation: verify it here, briefly (scope decision, 2026-09-12)

NJOY is used in this project for **one thing** — preparing cross sections for
`outram-mc-libs`. The paper must therefore carry **enough verification to
establish the data are trustworthy**, because the first question a referee asks
a transport paper reconstructing its own cross sections is "how do you know
those are right?" — and "see a future paper" is not an answer.

It must *not* carry a full nuclear-data treatment. That is a different paper for
different reviewers, and the authors do not claim nuclear-data-specialist
competence.

**`data_prep_verification.csv` is the section.** Ten rows, every one a numerical
comparison against **NJOY2016 rebuilt in-session** or an analytic limit:

| | |
|---|---|
| U-238 / U-235 point σ, 19 probe energies | ±0.04 % / ±0.06 % |
| U-238 capture **area** (exact integral vs published `RI_∞`) | **+0.00 %** |
| U-238 capture **shape**, 6 resonances | worst 0.12 %, rms 0.044 % |
| U-235 fission / capture RI | **+0.00 %** |
| graphite S(α,β) σ | ±0.05 % |
| moderator σ_t / σ_s, all 8 nuclides | ≤0.05 % |
| moderator thermal capture @ 0.0253 eV | ≤0.03 % |
| B-10 thermal absorption | 3845.9 b; 1/v to 4e-5 |
| H-1 free-gas elastic | ≤0.09 % |

**Two things make this cheap to defend.**

1. **It is a comparison, not a theory claim.** The argument is "we rebuilt
   NJOY2016, ran the same tapes through both codes, and here is the agreement."
   Defending that needs no position on R-matrix formalism or probability-table
   methods — only that the comparison was done correctly. State the NJOY2016
   version and that it was built in-session.
2. **These rows are already load-bearing in the localisation.** Each one
   simultaneously verifies the data preparation *and* excludes a mechanism in
   `mechanism_elimination.csv`. The section is structurally required by the
   argument, so it reads as method rather than as a defensive appendix.

The oracles are committed as **golden data**
(`tests/u238_vs_njoy_pendf_golden.rs`, `tests/thermal_laws_vs_njoy_thermr.rs`),
so a reader can re-check the comparison without building NJOY2016. Say so — it
is the difference between a reproducible claim and a trust-me one.

**Deliberately out of scope**, and not on the Monte Carlo path at all: ERRORR /
MF=32 covariance, GROUPR / GAMINR / COVR multigroup, WIMSR, LEAPR, SAMM LRF=7,
and windowed multipole. Those 1,174 lines in
`crates/njoy-outram-park-fork/verification_and_validation/` are banked for a
separate paper with a co-author carrying nuclear-data expertise. If a referee
asks for more, that record exists and can be cited — but do not lead with it.

## Regenerating the underlying results

```bash
cargo run --release -p outram-mc-libs --example godiva_keff_endf_local
cargo run --release -p outram-mc-libs --example jemima_keff
cargo run --release -p outram-mc-libs --example hst009_keff
cargo run --release -p outram-mc-libs --example lct008_keff
cargo run --release -p outram-mc-libs --features endf-pebble-cases \
    --example fhr_ring_rpt_endf
```

Needs the ENDF/B-VIII.0 tapes in `reference-data/endf/` (git-tracked, excluded
from `cargo publish`). **These were not re-run to build this dataset** — the
numbers are extracted from the committed record.
