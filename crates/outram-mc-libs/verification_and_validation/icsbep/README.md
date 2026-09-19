# ICSBEP benchmark specifications

Committed OpenMC input files for the ICSBEP criticality benchmarks this crate
runs as **external** oracles — cases whose answer does not come from any deck
under test, because an ICSBEP *critical* configuration has benchmark
`k_eff = 1.0000` to within its evaluated experimental uncertainty by
construction.

| Directory | Benchmark | Character | Example |
|---|---|---|---|
| `leu-comp-therm-008/` | LEU-COMP-THERM-008 — B&W critical lattices | thermal, 2.459 w/o UO₂ rods in 1511 ppm borated water; U-238 is 97.5 % of the heavy metal | `examples/lct008_keff.rs` |

Godiva (HEU-MET-FAST-001), Jemima (IEU-MET-FAST-002) and HEU-SOL-THERM-009 are
run by `examples/godiva_keff_endf_local.rs`, `examples/jemima_keff.rs` and
`examples/hst009_keff.rs` from atom densities and radii written into those
files: each is four numbers and three concentric surfaces, small enough to read
in the source. LEU-COMP-THERM-008 is not — 22 distinct 15 × 15 pin lattices
inside a 7 × 7 core lattice — so its specification is **committed here and
parsed at run time** instead of transcribed. The model is then the sourced file,
and a transcription error cannot be introduced by the example.

## Provenance and licence

The files in `leu-comp-therm-008/` are taken verbatim from
[`mit-crpg/benchmarks`](https://github.com/mit-crpg/benchmarks), the OpenMC
developers' collection of ICSBEP models, retrieved 2026-09-11:

| File | Source in that repo | Role |
|---|---|---|
| `materials.xml`, `geometry.xml`, `settings.xml` | `icsbep/leu-comp-therm-008/openmc/case-1/` | the model `examples/lct008_keff.rs` parses and runs |
| `mcnp_case-1.input` | `icsbep/leu-comp-therm-008/mcnp/case-1/input` | the **original MCNP deck** for the same case — a second, independent source |

The MCNP deck is committed because it writes the same geometry a completely
different way: not as lattice maps at all, but as an inner core plus six
explicit `px`/`py` rectangles, an "axially uniform quadrant" with reflective
faces on `x = 0` and `y = 0`. That makes the two check each other, and
`tests/ring_rpt_hunt_lessons.rs::the_lct008_core_matches_the_original_mcnp_deck_pin_for_pin`
does exactly that: **4961 fuel pins on each side, 0 mismatches in 11 025
lattice positions**. Parsing the XML removes transcription error; the second
source is what rules out running a plausible *wrong core*.

It also carries the full Al-6061 composition (`m2`), which is where the list of
trace alloying elements this environment has no tape for comes from;
`examples/lct008_keff.rs --clad-omission-bound` bounds their worth by
measurement.

That repository carries the **MIT licence, © 2011-2024 Paul Romano and other
contributors** — the notice as written in its own `LICENSE`, copied verbatim to
`leu-comp-therm-008/LICENSE`. MIT is GPL-3-compatible.

This satisfies `DATA_POLICY.md`: properly licensed public benchmark data, from a
public source, traceable to it.

## What is *not* here, and why it does not need to be

The evaluated `k_eff` and its uncertainty are **not** in that repository and the
ICSBEP handbook is not reachable from this environment. They are not needed. A
benchmark model reduced from a delayed-critical experiment is critical by
construction, so `1.0000` is the reference and the only open question is the
width of the band — 0.1–0.6 % for a lattice of this kind, and the examples use
the pessimistic `± 0.006`.

That is reasoning from what an ICSBEP benchmark *is*. It is not a licence to
recall the rest: **atom densities and geometry must come from the file**, never
from memory or a web summary, or the result looks like validation without being
it.

---

# Do the ICSBEP benchmarks match? — determination of 2026-09-19

**Short answer: one case matches on defensible grounds, two probably match but
against bands we chose ourselves, and one is at genuine risk.** The leading
explanation for the residuals has been tested and REFUTED.

## The pooled results

All four re-measured over 32 independent seeds (`gh:#196` / `bn:op-awwi` in
substance). Single-seed values are superseded — on Jemima the single draw read
`+6 ± 173`, which looked like near-perfect agreement and was a 1.3-sd
excursion.

| case | spectrum | dominant | pooled | benchmark band | verdict |
|---|---|---|---|---|---|
| Godiva HEU-MET-FAST-001 | fast | U-235 (93.7 %) | **−55 ± 34** | ±100, **real ICSBEP** | **matches** |
| Jemima IEU-MET-FAST-002 | fast | U-238 (83 % HM) | **−253 ± 34** | ±300, *pessimistic stand-in* | **at risk** |
| HST-009 HEU-SOL-THERM-009 | thermal | U-235 | **−38 ± 36** | ±600, *assumed worst case* | probably, not established |
| LCT-008 LEU-COMP-THERM-008 | thermal | U-238 (LEU) | **+165 ± 25** | ±600, *assumed worst case* | probably, not established |

**Only Godiva is judged against a quoted ICSBEP uncertainty.** The other three
bands are self-selected, two of them as the WIDEST plausible value because the
handbook was not reachable — the assumption that makes "inside the band"
easiest to satisfy. Getting the real case uncertainties is a DATA task and is
the single thing that would settle three of these four verdicts.

## The pattern that motivated the ablation

The residuals do not scatter randomly. They sort by **U-238 content**, and the
two U-238-heavy cases disagree in **sign** — a 418 pcm swing, negative fast and
positive thermal. Both U-235-dominated cases sit near zero across BOTH spectra,
so the U-235 physics looks sound. A single scalar error in U-238 cannot produce
an opposite-signed pair; an energy-dependent shape error can.

That, plus the HTR-10 result of the previous day — where the whole residual
turned out to be the data library (`+385 ± 310` on ENDF/B-VII.0 against
`−1259` on VIII.0) — made "the U-238 evaluation differs" the obvious candidate.

## The ablation: U-238 alone swapped to ENDF/B-VII.0

Every other nuclide held at VIII.0. A whole-library swap could not tell U-238
apart from U-235, which is the distinction the pattern turns on.
`OUTRAM_U238_ENDF7=1` on all four examples.

| case | VIII.0 | U-238 → VII.0 | shift | |
|---|---|---|---|---|
| Godiva | −55 ± 34 | −43 ± 32 | **+12 ± 47** | 0.26 σ |
| Jemima | −253 ± 34 | −216 ± 42 | **+37 ± 54** | 0.68 σ |
| HST-009 | +0 ± 50 | +132 ± 103 | **+132 ± 114** | 1.15 σ |
| LCT-008 | +108 ± 60 | +10 ± 122 | **−98 ± 136** | 0.72 σ |

**Not one shift is resolved.** The largest is 1.15 σ.

### PREDICTION REFUTED, and it was recorded before measuring

The prediction filed in advance was that Jemima and LCT-008 would move
**"substantially — order 100s of pcm"**. Jemima moved `+37 ± 54`, consistent
with zero, at 32-seed precision. The prediction failed.

**The U-238 evaluation is not the explanation.** What made it attractive was
the HTR-10 result, and the reason it does not transfer is now clear: HTR-10's
residual lived in the thermal/epithermal range where VII.0 and VIII.0 genuinely
diverge, while Jemima is a fast metal assembly. Consistent picture, wrong
transfer.

### Honest statement of resolving power — the two halves differ

- **Fast cases, 32 seeds per arm:** bounds the U-238 evaluation term below
  **~150 pcm**. A real exclusion.
- **Thermal cases, 4 seeds per arm** (maintainer chose the seed count): bounds
  it only below **~340 pcm at 3 σ**. An effect of 100–200 pcm would be
  invisible here. **This is a weaker exclusion and must not be quoted as
  equivalent to the fast one.** Settling it needs ~32 seeds per thermal arm.

The 4-seed baselines do reproduce the 32-seed pooled means — HST-009 at
0.62 σ, LCT-008 at 0.88 σ — so the thermal arms are measuring the right thing,
just with less precision.

## What remains, for Jemima specifically

None of these is touched by a library swap:

- **U-238 inelastic angular treatment.** `op-os8x`'s open Godiva residual was
  localised to a down-scatter deficit out of the MeV window, and inelastic is
  the only channel that moves a 2 MeV neutron to ~100 keV in one collision.
  Jemima is far more U-238-dominated than Godiva, so the same defect would bite
  harder there. **This is the leading candidate.**
- **U-238 fast fission** above its ~1 MeV threshold.
- **ν̄(E)** for U-238.

## What this determination does NOT claim

It does not say the benchmarks fail. Three of four residuals are small by any
reasonable standard, and the fourth is only "at risk" because its band is a
stand-in rather than a quoted value. What it says is that the residuals are
**structured, not noise**, that the cheapest explanation for that structure has
been tested and rejected, and that the remaining candidates are transport-side
rather than data-side.

## Second ablation: U-238 inelastic angular treatment — also ELIMINATED

The remaining leading candidate after the library swap. `op-tm9f` wired in the
ENDF MF=4/MT=51..90 discrete inelastic angular distributions; before it, every
inelastic collision drew `mu_cm = 2*prn − 1`. Jemima is **83 % U-238 by heavy
metal and 99.3 % U-238 in its reflector** against Godiva's ~5 %, so it is far
more exposed to the same treatment.

Measured by ablating it (`OUTRAM_JEMIMA_ISO_INELASTIC=1`), 32 seeds per arm:

| case | ANISO (current) | ISO (ablated) | the term is worth | |
|---|---|---|---|---|
| Godiva | +45 ± 32 | +269 ± 30 | **+224 ± 44 pcm** | 5.1 σ |
| Jemima | −253 ± 34 | +90 ± 31 | **+343 ± 46 pcm** | 7.5 σ |

The term IS worth more on Jemima — 1.53× — exactly as U-238 dominance
predicts. That part of the physics behaves as expected. But it cannot carry
the residual, by two independent arguments:

**1. The residuals do not scale with the sensitivities.** If Jemima's residual
came from this treatment, the residual ratio should track the sensitivity
ratio, **1.53**. The measured residual ratio is 253/55 = **4.6**. A common
cause scaled by exposure would give the same number twice; it does not.

**2. A sufficient error is excluded by Godiva.** Explaining −253 pcm on Jemima
needs a **74 % error** in the treatment. The same 74 % error on Godiva, where
the term is worth 224 pcm, would put Godiva at **+165 pcm**. Godiva is measured
at **−55 ± 34**. Those are ~6 σ apart.

**Eliminated**, on the same standard as the library swap.

## Where the ICSBEP determination now stands

Two candidates tested, both with predictions recorded in advance, both refuted:

| candidate | result | bound |
|---|---|---|
| U-238 evaluation (VII.0 vs VIII.0) | not resolved | < ~150 pcm on the fast cases |
| U-238 inelastic angular (MF=4/MT=51..90) | excluded at ~6 σ | cannot produce 253 pcm without breaking Godiva |

**Still open, and the honest list is short:**

- **U-238 fast fission** above its ~1 MeV threshold. Jemima's spectrum sits
  where this matters and Godiva's largely does not, so unlike the two
  eliminated candidates this one CAN break the 1.53 scaling that killed them.
  It is the natural next ablation.
- **ν̄(E) for U-238.**
- **The benchmark band itself.** Jemima's ±300 is a stand-in the code calls
  pessimistic; if the true ICSBEP uncertainty is nearer ±200, −253 ± 34 is a
  real disagreement, and if it is wider, there may be nothing to explain. This
  is not resolvable from this environment — see below.

### Why the band cannot be settled here (checked, not assumed)

The network IS reachable, so an earlier claim in this repository that the
handbook is unreachable is **stale and is corrected here**. What blocks it is
licensing, not connectivity: the ICSBEP handbook is NEA-licensed and not
freely redistributable, so fetching it would breach `DATA_POLICY.md`. There is
no ICSBEP material in `crates/kovan-literature`, in `reference-data/`, or in
the local OpenMC checkout.

**Obtaining the real case uncertainties requires a handbook licence and is the
single action that would settle three of the four verdicts.** It is a data
task, not a compute one, and no amount of further ablation substitutes for it.
