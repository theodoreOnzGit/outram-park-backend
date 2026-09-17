# HTR-10 vs the RMC benchmark — V&V record

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Status as of 2026-09-17: the core now runs and produces an eigenvalue. It is
NOT yet in agreement with the benchmark.** `bn:op-867c`, gh #214.

Measured 2026-09-17, 14 rings x 25 layers (13,675 tiles, a 93.6 cm-radius by
122.5 cm bed against the benchmark's 123.576 cm critical height), 2000 histories
x [30 inactive + 70 active], surface tracking, ENDF/B-VIII.0:

```text
k_eff        = 0.857140 +/- 0.003028     (RMC 1.004288, difference -14715 pcm)
collisions   = 458.09 per history
lost locate  = 0     stuck events = 0     negative distances = 0
leak vacuum  = 7.965 %
```

**-14,715 pcm is far outside the 500-1000 pcm gate.** This is reported as a
failure to agree, not as a result. What changed today is that the model now
transports neutrons correctly, so the residual is attributable to the MODEL
rather than to the tracker or the geometry engine.

### How it got here, measured at each step

| configuration | `k_eff` | change |
|---|---|---|
| before the lattice fix | **0.000000** | — |
| 12 rings x 20 layers (98.0 cm bed) | 0.707506 +/- 0.006010 | first nonzero |
| 13 rings x 25 layers (critical height) | 0.803706 +/- 0.003286 | +9,620 pcm |
| packing corrected 0.5811 -> 0.610 | **0.857140 +/- 0.003028** | +5,343 pcm |

### Ablations that bound the residual

| ablation | `k` | reads as |
|---|---|---|
| reflective outer boundary (no leakage) | 0.806182 +/- 0.002811 | leakage is worth only ~250 pcm -- the 100 cm reflector already returns nearly everything, so the deficit is in the BED, not the boundary |
| all fuel, no dummy balls | 0.915527 +/- 0.003962 | the 57:43 graphite dilution costs **~11,000 pcm**, which is what dummy balls are for |
| all boron removed | **0.943105 +/- 0.003170** | boron is worth **8,597 pcm** on the core (`-6,118 pcm` from RMC) |

The first two are at the pre-packing-fix geometry, so they bound components
rather than adding to the current number. The boron arm is at the current
geometry and is directly comparable to the 0.857140 above.

### The boron reading now dominates, and it is an INPUT question

Boron is worth **8,597 pcm here against 1,487 pcm on a bare pebble** -- roughly
six times -- because the core surrounds the fuel with 100 cm of reflector
graphite and 43 % graphite dummy balls, and a thermal neutron spends most of its
life in graphite where a 3840 b absorber at a few ppm competes directly with
carbon's own 0.0035 b. The bare-pebble sensitivity study therefore **understates
this term badly**, and citing its 1,487 pcm as the scale of the boron
uncertainty for a whole core would be wrong.

This matters because the reading is genuinely ambiguous, as
`examples/htr10_pebble_delta_tracking.rs` already records: Table 2's "ppm" has
no stated basis, and is taken here as *natural* boron *by weight*. The
reflector's boron comes from a different source again -- TECDOC Table 4-3's
`natural_boron` column, 4.738e-7 against carbon 8.824e-2, i.e. 5.4 ppm atomic --
and is multiplied by 0.199 to get B-10. **If that column is already an
absorbing-species ("boron equivalent") density rather than elemental natural
boron, the 0.199 must not be applied and the model is currently UNDER-absorbing
in the reflector** -- which would make the disagreement worse, not better.

**Removing the boron is not a fix and must not be read as one.** Real nuclear
graphite carries it, and the 0.943105 arm is unphysical. What the ablation
establishes is that ~8,600 pcm of this model's answer rests on an input reading
that the source documents do not state unambiguously, and that resolving that
reading against TECDOC-1382 is worth more than any further transport work.

### What is NOT yet explained

~14,700 pcm. The bed's own `k_inf` is ~0.86 where a core critical at this height
needs ~1.1. Ruled out so far: leakage (~250 pcm), the tracker (surface and delta
agree), the geometry engine (0 lost / 0 stuck / 0 negative), the TRISO loading
(1.006 of intended), the materials (cross sections printed and checked per
material), and the packing (now 0.610 by construction).

**One comparison that looked like a contradiction and is not.** The recorded
single-pebble `k_inf = 1.68515 +/- 0.00178` is a **bare pebble** -- reflective at
r = 3.0 cm, i.e. pure pebble material at packing 1.0 with no interstitial void.
The bed is 39 % void between pebbles, so the two are not the same problem and
the difference is not by itself evidence of a defect. A like-for-like comparison
needs an infinite medium of pebbles at 0.61 packing, which the
`OUTRAM_HTR10_NOREFL` knob does not yet deliver (a zero-thickness reflector
makes the bed envelope and the outer boundary coincident, and 99.3 % of
histories are then lost at the seam). That is the next measurement.

**The reflector is ruled out as the cause, and is in fact optimistic.** The
model uses TECDOC Table 4-3 zone 22 for the whole 100 cm reflector. Zone 22 is
the **cleanest graphite in the table** -- carbon 8.824e-2, the highest listed,
with natural boron 4.738e-7, near the lowest. The boronated zones (17, 19, 27,
46, 64, carrying ~3.4e-3 natural boron, i.e. 7,000x more) are **omitted
entirely**. Both simplifications push `k` UP, so the real 83-zone reflector
would give a lower answer, not a higher one. Whatever the missing reactivity
is, the reflector is not it -- and the current number benefits from that
optimism, which is why it is stated here rather than left implicit.

Leading remaining candidates, none yet measured: boron treatment (independently
worth ~1,487 pcm on a bare pebble); spectral effects of ballistic streaming
through the interstitial void, which a bare-pebble model cannot exhibit; and
the double-heterogeneity resonance treatment in the explicit TRISO lattice.

Checked and NOT the cause: the kernel's thermal `nu-Sigma_f / Sigma_a` is
2.038, against 2.07 for pure U-235 and ~2.03 expected for 17 %-enriched UO2 --
so the enrichment and the kernel's thermal behaviour are right, and any fuel
problem lives in the resonance range rather than at thermal.

## The k = 0 failure and its root cause — RESOLVED 2026-09-17

~~The eigenvalue comparison has been ATTEMPTED AND FAILED; k = 0.000000.~~
**CORRECTED 2026-09-17** — the cause was a **port defect in
`HexLattice::distance`**, not the model, the tracker, or the fuel loading.

`HexLattice::distance` reconstructs a lattice-frame position from the caller's
tile-local one. It reconstructed **all three** components; the axial test at the
end of that function compares `z` against `+/- 0.5 * pitch[1]`, which is a
**tile-local** half-height. The comparison was therefore wrong by the tile's own
`z` offset and returned a **negative** distance-to-boundary.

OpenMC builds the hybrid -- x,y lattice-frame, z tile-local -- at the *call*
site (`src/geometry.cpp:459-467`) and guards the result with
`if (d_lat < 0) p.mark_as_lost(...)`. This port had neither.

**Why it survived.** The error cancels **exactly** when the tile z-offset is
zero, i.e. `n_axial == 1`, and `from_rings_3d` had unit tests only -- no
integration test and no example. Every existing test sat on the one
configuration that hides it.

**Why it was invisible in `k`.** A negative distance steps the neutron
backwards, so it re-crosses the same boundary until the per-history event budget
kills it -- and a budget-exhausted history is **scored as a leak**, so the
neutron balance closes and nothing in the output points at geometry.

**Measured, on this model:**

| `n_axial` | negative distances before | after |
|---|---|---|
| 1 | 0 | 0 |
| 2 | 8,199,697 | 0 |
| 20 | 17,498,719 (worst -7.7e3 cm) | 0 |

and the history-termination histogram, 12 rings x 20 layers:

| end | before | after |
|---|---|---|
| stuck on the event budget | **68.5 %** | 0 % |
| lost in `locate` | 14.5 % | 0 % |
| genuine vacuum leak | 5.4 % | — |
| collisions per history | 4.2 (denominator-corrected 105.6) | 498.9 |
| `k_eff` | 0.000000 | **0.707506 +/- 0.006010** |

Gated by `tests/hex_lattice_axial_frame.rs`. Filed as a P0 bug.

### How it was found — the instrumentation is the finding

`k` alone could not distinguish "absorbed" from "lost", because both leak arms
scored identically. Five counters were added to the CSG driver and are now part
of `KeffResult`: `histories`, `collisions`, `lost_locate`, `stuck_events`,
`leak_vacuum`, `leak_infinity`, plus `neg_dist`/`neg_from_lattice`/
`neg_from_surface`. Each step below eliminated a hypothesis:

1. **Reflective outer boundary changed `k` bit-for-bit not at all** -> no
   history was reaching the boundary; this is not leakage.
2. **Macroscopic cross sections printed per material** -> every material is
   correct (graphite absorption/total 1.0e-3 thermal, 3.3e-6 at 1 MeV; the
   kernel's nu-fission 5.69 against absorption 2.79). Not the materials.
3. **The history denominator was wrong** -- rates were being divided by the
   *planned* history count while the run died after 4 generations. Correcting it
   turned "2.7 % stuck, 4.2 collisions/history" into "68.5 % stuck, 105.6
   collisions/history", which is what made the defect visible at all.
4. **Negative distances split by source** -> 100 % from the lattice, 0 % from
   any CSG surface.
5. **Swept `n_axial`** -> 1 layer gives exactly zero negatives, more gives
   millions. That named the axial branch.
6. **Read OpenMC's caller before patching** (workspace rule) -> found the
   hybrid position it builds, which is the fix.

**Two of my own hypotheses were wrong and were measured down rather than
assumed away**, and both are recorded because a discarded hypothesis is
evidence: a degenerate ball-tangent-to-prism geometry (it was real, and fixing
it changed nothing), and the TRISO lattice failing to cover its fuel zone (also
real, also not this). A third -- clipping the tile universes with explicit
planes -- was implemented, made things worse, and was reverted.

## The target

Li, Yu & Wei (2014), *Research on Benchmark Calculation and Analysis of HTR-10
with RMC Code*, HTR 2014 Weihai, paper HTR2014-51207. Catalogued **proprietary**
(no licence statement on its pages) as `li2014htr10rmc`.

Critical loading height **123.576 cm**: RMC **k = 1.004288**, MCNP **1.0033**.

**The reference quotes no uncertainty on any of its 12 values.** At its stated
1.35 M active histories the implied σ is ~60–100 pcm, but it is never printed,
so "agreement to 100 pcm" against it is not a well-posed claim. The gate is
**500–1000 pcm** (maintainer decision), which matches the existing bar recorded
in `nee_soon::htr10_rmc` — *"~500 pcm would be success, 50 pcm would be
suspicious."*

## What has been built and measured

| Component | Evidence | Measured |
|---|---|---|
| Hybrid delta/surface tracking | `tests/hybrid_tracking_equivalence.rs` | hybrid vs surface **−133 ± 215 pcm (0.62 σ)** at 12,000 histories |
| — absorber isolation | same | region-local vs global majorant **448×** in virtual collisions, `k` unchanged (1.32 σ) |
| Majorant price of a rod | `examples/majorant_absorber_price.rs` | **26.3×** at the thermal peak, **1.00×** above ~1 keV |
| Boundary handoff unbiased | `tests/bounded_delta_flight.rs` | one region vs two + handoff, **1.38 σ** on collided fraction |
| Depth-3 lattice descent | `tests/nested_lattice_depth3.rs` | 3 levels, streaming stops at the **inner** tile edge (0.050000 cm) |
| Shannon entropy in the driver | `tests/shannon_entropy_in_keff.rs` | plateaus at 5.32 bits below the log2(64) ceiling; `k` bit-identical with/without |
| TRISO radii adjudicated | `op-867c.12` | TECDOC-1382 states 90 µm twice, in two units; `TrisoRadii::HTR10` corrected from 95 |
| Cubic TRISO array | `tests/cubic_triso_array.rs` | **8340** particles, **+0.060 %** vs the stated 8335 |
| Bed 57:43 split | `nee_soon` `htr10_rmc::bed` | 0.569995 at 18,930 tiles, within one tile at **every prefix** |
| Reflector, 82 zones | `nee_soon` `htr10_rmc::reflector` | densest zone **100.0 %** of solid graphite at the separately-stated 1.76 g/cm³ |
| Geometry integrity | `nee_soon` `tests/htr10_geometry_integrity.rs` | 27,038 balls vs stated 27,000, **+0.142 %** |
| Core assembly cost | `nee_soon` `examples/htr10_core_scaling.rs` | **547× tiles → 5 % locate time** |

## Three findings that changed the plan

1. **8,335 TRISO is unattainable.** The count moves in symmetry shells (8336 →
   8240 in one step of pitch); nearest reachable are 8330 and 8340. The paper's
   arrangement is therefore *not* exactly the one specified — its zone radius,
   particle radius or rejection rule must differ in the last digit.
2. **One shared surface cannot clip the conus.** Region surfaces inside a
   lattice tile are evaluated in the **tile-local** frame, so each boundary tile
   needs its own translated copy (`surface_in_tile_frame`).
3. **Scale is not a runtime risk.** The plan treated the 650× gap to full core
   as its main threat. Locate cost is flat, because lattice indexing is O(1)
   arithmetic rather than a search.

## What is NOT done, and must not be implied

- **No eigenvalue has been computed for HTR-10.** The assembled core carries a
  **homogenised** fuel zone, not an explicit TRISO lattice, so the double
  heterogeneity is absent and its `k` is not comparable to the reference.
- **The data library differs from every reference.** RMC, MCNP, Serpent and HCP
  all used **ENDF/B-VII.0**; this workspace has **VIII.0**. On a
  graphite-moderated LEU system that is worth hundreds of pcm, so a
  disagreement could not be attributed to transport.
- **The reflector densities are homogenised in R-Z.** TECDOC says explicitly
  that a 3-D model must correct them for the boring geometries; using them
  unadjusted smears the control-rod and helium-flow channels uniformly.
- **This crate's thermal accuracy floor is ~200–400 pcm**, not 100 — LCT-008
  sits at +87 to +237 pcm against ICSBEP with a ~69 pcm spectral residual still
  open (`op-os8x`, gh #206).
- **No control rods or absorber balls** are modelled.

## The fuel deficit — real, but NOT the cause of `k = 0`

~~`k = 0` traces to the model carrying far too little fuel.~~
**CORRECTED 2026-09-17** — it does not. The cause was the `HexLattice::distance`
axial-frame defect recorded above; `k = 0` persisted through every fuel-loading
fix in this section and vanished the moment the lattice defect was fixed, with
the fuel loading unchanged. The packing finding below is nonetheless **real and
was fixed**, so it is kept — as a loading correction, not as a diagnosis.

The reasoning that went wrong is worth keeping too: a ~5x fuel deficit was
measured, `k = 0` was attributed to it, and the attribution was never tested
against the alternative that neutrons were being *destroyed*. They were. The
lesson is the one the instrumentation section states — `k` alone could not
distinguish a model that under-produces from one that loses its histories, and
no amount of reasoning about fuel fractions could substitute for counting how
each history actually ended.

The deficit as originally measured:

| quantity | value |
|---|---|
| expected kernel volume fraction of an HTR-10 bed | **1.676e-3** |
| measured source acceptance over the box | **3.375e-4** |
| ratio | **~5x too little fuel** |

**Half of that factor is found exactly.** `HexBedCell::from_paper()` carries
`balls: 2.0` — the paper's hexagonal prism holds **two** balls, one per
close-packed layer — but the assembled lattice places **one pebble universe per
tile**:

| | packing |
|---|---|
| one ball per tile (what is built) | **0.3050** |
| two balls per tile (the paper) | **0.6100** ← the stated 0.61 |

Exactly 2x, reproducing the published filling fraction to four digits. Not an
approximation — the defect.

The confusion underneath is worth stating because it is easy to repeat: a hex
**lattice** places one universe at each tile centre, while the paper's **cell**
is a two-layer prism carrying half-spheres on its faces and full balls between.
They are not the same object. `HexBedCell`'s arithmetic is correct and gated —
it predicts 27,038 balls against a stated 27,000 — but it was never reconciled
with the lattice that consumes it, and nothing checked that the geometry
realised the packing the arithmetic assumed.

~~**Residual: 2.31x**, most likely the per-tile TRISO count.~~
**CORRECTED 2026-09-17 — the TRISO count is right.** Measured directly
(`nee_soon/examples/htr10_triso_count.rs`): the assembly builds **8,385**
particles for a realised packing of **0.050550** against the `TrisoSpec`
intended **0.050248** — a ratio of **1.006**, i.e. 0.6 % high, not 2.31x low.
The tile-centre keep rule and `cubic_array_in_ball`'s whole-particle rule agree
to within one part in 170 at this pitch.

**The packing fix, and a false start inside it.** The two-balls-per-tile factor
was closed by keeping the paper's pitch (6.6106 cm) and halving its height
(9.79796 -> 4.899 cm), so one ball per tile reproduces 0.610 exactly with the
ball clipped axially. An earlier attempt set the pitch to the **ball diameter**
(6.0 cm) instead; that also gives ~0.61, but makes the ball exactly tangent to
all six prism faces. That degeneracy is a genuine defect and was measured --
but fixing it changed the negative-distance count not at all, which is how it
was ruled out as the cause.

**What is established now:** the loading is right to ~1 %, the transport is
clean (0 lost, 0 stuck, 0 negative), and the remaining -29,678 pcm is a
MODEL-completeness question -- starting with the fact that the run above is a
98.0 cm bed against the benchmark's 123.576 cm critical height.

## Reproducing what exists

```bash
cargo test -p outram-mc-libs --release --test hybrid_tracking_equivalence
cargo test -p outram-mc-libs --release --test bounded_delta_flight
cargo test -p outram-mc-libs --release --test nested_lattice_depth3
cargo test -p outram-mc-libs --release --test cubic_triso_array
cargo test -p nee_soon        --release --test htr10_geometry_integrity
cargo run  -p nee_soon        --release --example htr10_core_scaling
cargo run  -p outram-mc-libs  --release --example majorant_absorber_price
```
