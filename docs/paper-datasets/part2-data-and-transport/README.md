# Part II — nuclear data preparation and Monte Carlo transport on singly-heterogeneous benchmarks

**Scope:** everything needed to establish that the transport kernel and the
cross sections are right, on **classical ICSBEP criticality benchmarks** —
bare metal, reflected metal, homogeneous solution, and a pin lattice. Double
heterogeneity (TRISO-in-pebble) is deliberately **not** here; it is Part III.

**Source records:** `crates/outram-mc-libs/verification_and_validation/` —
`icsbep/README.md` for the benchmark specifications and provenance, and the
Interpretation sections of `ring_rpt/ring_rpt_vs_openmc.md` for the measured
results and their oracles.

## Why this is its own paper

Part III's contribution is a *method* for doubly heterogeneous media. A method
paper that has to first establish its own cross sections, its own transport
kernel, *and* its own geometry treatment is carrying three arguments and
defending none of them well.

Splitting lets Part II be the boring, reviewable, necessary paper — "here is the
code, here are four measured criticals, here is how the data were prepared and
checked" — so Part III can assume it and spend its length on what is new.

It also puts the NJOY verification where it belongs. NJOY is used in this
project for exactly one purpose: preparing cross sections for `outram-mc-libs`.
In Part II that is **foundational**; bolted onto Part III it would read as a
defensive appendix.

## Files

| File | Contents |
|---|---|
| `data_prep_verification.csv` | 10 rows vs NJOY2016 rebuilt in-session, or analytic limits |
| `icsbep_benchmarks.csv` | the four criticals — spectrum × U-238 loading |
| `lct008_boron_series.csv` | the poison trend across LCT-008 cases 1, 2, 8 |

## The result, stated honestly

Three of four benchmarks agree with measured criticals; the fourth does not, and
the paper's job is to say **where** rather than to average it away.

| | low U-238 (5 % of HM) | high U-238 (83–97.5 %) |
|---|---|---|
| **fast** | Godiva `+57 ± 173` | Jemima `+6 ± 173` |
| **thermal** | HST-009 `−18 ± 171` | LCT-008 **`+2950 ± 61`** |

The four were chosen to span spectrum × U-238 loading, so the failing corner is
identified by construction rather than by hindsight: LCT-008 is the only one that
is thermal **and** strongly self-shielded in U-238 **and** heterogeneous
(1.03 cm UO₂ pellets, 176 mfp).

The boron series then shows the residual **tracks the poison** — across cases
1/2/8 it moves `+2950 → +2271 → +1713` as soluble boron falls
`1511 → 1335.5 → 794` ppm, a spread of `1237 ± 86` pcm at **14σ** on cases
sharing one pin cell. A pure cross-section error would give the same Δk
everywhere; this does not.

**Localising against measured experiments rather than against another code is
the methodological point.** A code-to-code comparison cannot distinguish "we are
wrong" from "they are wrong." An ICSBEP critical configuration is `k = 1.0000`
by construction, so there is no second opinion to argue with.

## Nuclear data preparation — brief, and framed as a comparison

`data_prep_verification.csv`, every row against **NJOY2016 rebuilt in-session**
or an analytic limit:

| | |
|---|---|
| U-238 / U-235 point σ, 19 probe energies | ±0.04 % / ±0.06 % |
| U-238 capture **area** (exact integral vs published `RI_∞`) | +0.00 % |
| U-238 capture **shape**, 6 resonances | worst 0.12 %, rms 0.044 % |
| U-235 fission / capture RI | +0.00 % |
| graphite S(α,β) σ | ±0.05 % |
| moderator σ_t / σ_s, all 8 nuclides | ≤0.05 % |
| moderator thermal capture @ 0.0253 eV | ≤0.03 % |
| B-10 thermal absorption | 3845.9 b; 1/v to 4e-5 |
| H-1 free-gas elastic | ≤0.09 % |

**Defend it as a comparison, not as a theory claim.** The argument is "we
rebuilt NJOY2016, ran the same tapes through both codes, here is the agreement"
— which requires no position on R-matrix formalism or probability-table methods.
State the NJOY2016 version and that it was built in-session.

The oracles are committed as **golden data**
(`tests/u238_vs_njoy_pendf_golden.rs`, `tests/thermal_laws_vs_njoy_thermr.rs`),
so a reader re-checks the comparison **without building NJOY2016**. Say so.

**Check the `+0.00 %` entries before printing them.** They will draw a referee's
eye, and "+0.00 %" reads as suspiciously perfect. If the underlying figure is
`< 0.005 %`, write that instead — more honest and less likely to be questioned.

**Out of scope, and genuinely not on the Monte Carlo path:** ERRORR / MF=32
covariance, GROUPR / GAMINR / COVR, WIMSR, LEAPR, SAMM LRF=7, windowed
multipole. That record exists (`crates/njoy-outram-park-fork/verification_and_validation/`,
1,174 lines) and can be cited if a referee asks — do not lead with it.

## Also belongs here

- **Tracking-method equivalence** — delta (Woodcock) vs surface-tracked CSG on
  identical geometry: `18 pcm, 0.05σ`. A transport-kernel verification, and it
  is what licenses Part III's use of delta tracking.
- **The thermal-kernel moment checks** (2026-09-12): scattering **angle**
  excluded at `≤0.0085` absolute on μ̄ against NJOY THERMR MF=6/MT=229; kernel
  **width** is a *real* defect at `−11 %` to `+39 %` on the second moment, but
  priced at only `−63 pcm`. Report it as found **and insufficient** — it does
  not explain `+2950`, and folding it into the headline would overstate the
  diagnosis.

## Publication order matters

Part III's absolute-k bias is explained by this paper's LCT-008 finding, so
**Part II must be posted first, or the same day**, and Part III must cite it by
arXiv number. Post Part II, wait for the identifier, then post Part III.

## Regenerating

```bash
cargo run --release -p outram-mc-libs --example godiva_keff_endf_local
cargo run --release -p outram-mc-libs --example jemima_keff
cargo run --release -p outram-mc-libs --example hst009_keff
cargo run --release -p outram-mc-libs --example lct008_keff
```

Needs the ENDF/B-VIII.0 tapes in `reference-data/endf/`. **These were not re-run
to build this dataset** — the numbers are extracted from the committed record.
