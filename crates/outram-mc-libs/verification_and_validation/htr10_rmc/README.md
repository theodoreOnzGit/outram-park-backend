# HTR-10 vs the RMC benchmark — V&V record

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Status as of 2026-09-17: the infrastructure is built and gated; the
eigenvalue comparison has been ATTEMPTED AND FAILED.** Read that sentence before
quoting anything below. `bn:op-867c`, gh #214.

## The attempt, and why it failed

`nee_soon/examples/htr10_rmc_keff.rs` assembles an explicit-TRISO core (four
coordinate levels: root -> bed hex lattice -> pebble -> TRISO rect lattice ->
particle) with a region-local majorant and hybrid tracking. First run,
8 rings x 12 layers, 1500 histories x [30 inactive + 70 active]:

```text
k_eff        = 0.000000 +/- 0.000000
virtual coll = 2820163146          (18,801 per history)
entropy      = 0.0000 -> 0.0000
wall clock   = 80.0 s
```

**Two separate findings.**

1. **The majorant cost is as predicted and is NOT the failure.** Measured in
   `examples/htr10_majorant_diagnosis.rs`: the bound is the UO2 kernel at
   4.18 cm^-1 while the volume-weighted local total is ~0.18, giving
   `p_accept ~ 0.0432` — about **22 rejections per real collision**, matching
   the ~25x this crate measured independently for an undiluted kernel. That is
   expensive, not fatal, and it is exactly the effect the region-local majorant
   was built to bound.
2. **k = 0 is a distinct bug, and BOTH the geometry and the tracker are now
   ruled out.**

   Running the **identical geometry with surface tracking only**
   (`OUTRAM_HTR10_SURFACE=1`) also gives `k = 0.000000`, with **zero** virtual
   collisions, in 1.8 s instead of 80 s. So the delta path is not at fault — it
   was doing exactly what it should (22x rejection, as predicted) on a model
   that produces no fission either way. The 2.8e9 virtual collisions were a
   symptom of the expensive path being taken, not the cause.

   That elimination is only possible *because* the hybrid tracker had already
   been proven equivalent to surface tracking at 0.62 sigma (`op-867c.7`): a
   disagreement here would have implicated the tracker, and the agreement
   implicates the model.

   **Three eliminations, and the materials are ruled out too.**
   `nee_soon/examples/htr10_material_check.rs` evaluates
   `fuel_pebble_materials` directly: the UO2 kernel carries
   **nu-fission = 5.691 cm^-1 at thermal** (fission 2.342), and every
   non-fuel layer is correctly zero. So the fuel is real and strongly
   multiplying.

   The geometry is also ruled out:
   `nee_soon/examples/htr10_locate_probe.rs` puts 20,000 uniform probes through
   the assembled core: **0 lost**, all eight materials reached, `Delta` reported
   inside the bed and `Surface` in the reflector, four-level descent where it
   should occur, and the kernel's 0.025 % probe share matching its ~1 %
   by-volume estimate. So `locate` and the delta path's `material_at` both work.
   The failure is in the flight, not the assembly. Still Zero fission sites and zero
   entropy mean nothing was produced at all, which 22x rejection does not
   explain. Candidates not yet discriminated: the source box may not intersect
   the bed; helium is modelled as an empty material so a flight through it can
   only reject; or `material_at` returns `None` inside the delta region, making
   every flight report `Exhausted`.

What the attempt DOES establish: the geometry assembles at four levels, the
hybrid dispatch engages, and the instrumentation reports — the virtual-collision
counter and the entropy trace both did their job, and it is *because* they
report that the failure is diagnosable at all rather than silent.

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
