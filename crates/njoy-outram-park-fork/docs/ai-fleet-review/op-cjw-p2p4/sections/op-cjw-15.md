# op-cjw.15 — GROUPR/GAMINR group-averaging engine + GENDF writer

**Bead:** op-cjw.15 (P4). **Crate:** `njoy-outram-park-fork`.
**Upstream:** NJOY2016 `src/groupr.f90` (12690 lines) + `src/gaminr.f90` (1517
lines), commit `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`.
**Status: PARTIAL — the 1-D vector group-average path is ported and tested; the
group-to-group matrix + self-shielding path is honestly `NotPorted`.**

> TOP HUMAN-VERIFY ASK: the numeric quadrature in `panel.rs`
> (`group_integral` / `group_average_vector`) is AI-authored and must be
> independently checked against a real NJOY GROUPR run before it is trusted for
> anything. The unit tests assert *properties* (constant-in → constant-out,
> flat-weight = integral mean, bounded-by-extremes, geometric convergence to an
> analytic integral) and one hand-computable value; they do **not** yet compare
> against a golden NJOY GENDF tape. See "Verification gap" below.

## Files changed / added (all under `src/groupr/` and `src/gaminr/`)

New files:

| File | Lines | Role |
|---|---|---|
| `src/groupr/panel.rs` | 556 | Panel group-integration quadrature (vector reduction of `panel`), `getsig`/`getflx` feeders |
| `src/groupr/gendf.rs` | 471 | GENDF group-record writer/reader (CONT + LIST) for a 1-D vector; `GendfTape` container |
| `src/groupr/kinematics.rs` | 63 | `cm2lab` CM→lab stub — documented `NotPorted`, names every missing matrix-path routine |
| `src/gaminr/gpanel.rs` | 112 | GAMINR photon-interaction vector path — re-exports the shared engine; `gtff` matrix stub |

Modified files (module wiring / doc-status only — no logic change):

- `src/groupr/mod.rs` — `pub mod {panel, gendf, kinematics}` + re-exports; updated
  port-status doc from "engine not ported" to "vector path ported, matrix not".
- `src/gaminr/mod.rs` — `pub mod gpanel` + re-exports; updated gap list.

## Provenance — Fortran routine → Rust map

| Fortran (`groupr.f90` / `gaminr.f90`) | Lines | Rust item | Notes |
|---|---|---|---|
| `panel` / `gpanel` (vector reduction) | g:5858–6091 / ga:874–1011 | `panel::group_integral` | `nl=nz=1`, `ff≡1`; Lobatto-on-linear-rr collapses to trapezoid on the union grid, matching NJOY exactly for the vector case |
| initial-energy group loop | g:510–580 | `panel::group_average_vector` | drives `group_integral` group by group |
| `getsig` / `gtsig` | g:6646–6800 / ga:1133–1160 | `panel::PointwiseXs` (`.value`, `.next_break`) | `Constant` + lin-lin `LinLin`; zero outside range (`gety1`) |
| `getflx` / `gtflx` | g:6439–6519 / ga:825–872 | `panel::GroupFlux` (`.value`, `.next_break`) | `Flat`/`Tabulated`/`Analytic`/`Spectrum`; geometric `step=1.05` refinement for smooth weights (`gtflx` `enxt=step*e`) |
| `displa` / `dspla` (vector, `mfd=23`) division | g:6093–6437 / ga:1013–1131 | `panel::GroupIntegral::average` | `result = ans(1,1,2)/ans(1,1,1)`; zero-flux → 0 guard |
| GENDF HEAD + LIST group records | g:882–946 | `gendf::GendfSection::{to_rows, from_rows}`, `GendfGroupRecord` | HEAD `[ZA,ZAM,NL,NZ,LRFLAG,NGN]`; LIST `[temp,0,NG2,IG2LO,NW,ig]` + `[flux_g, sigma_g]` |
| GENDF tape framing | (in-memory) | `gendf::GendfTape::{to_image, from_image}` | crate in-memory framing; not byte-identical to an NJOY ASCII GENDF file (documented) |
| `cm2lab` / `f6cm`/`f6ddx`/`f6dis`/`bach`/`ll2lab`/`f6lab`/`getdis`/`getaed` | g:8135–10239 | `kinematics::cm2lab` **(stub)** | returns `NotPorted("groupr::cm2lab")` |
| `gtff` matrix branches (`mtd=502/504/516`) | ga:1162–1514 | `gpanel::gtff_matrix` **(stub)** | returns `NotPorted("gaminr::gtff")` |

Reused existing crate primitives (not reinvented): `endf::interp::terp1`/`IntLaw`
(lin-lin), `endf::records::{Cont, List, SectionCursor}` (GENDF record substrate),
`groupr::weights::AnalyticWeight`, `nuclear_data::WeightingSpectrum`.

## DONE vs PARTIAL/stub

**DONE (ported + tested):**

- Flux-weighted **1-D vector** group average `sigma_g = ∫sigma*phi / ∫phi` over an
  arbitrary group structure and weight (flat / tabulated / analytic `iwt` /
  Watt·1-over-E·Maxwellian spectrum).
- The `getsig` / `getflx` pointwise feeders (constant + lin-lin σ; four weight kinds).
- GENDF **vector** section + tape write→read round trip through the ENDF
  CONT/LIST layout.
- The GAMINR photon-interaction **vector** path (shares the same engine).

**PARTIAL / still `NjoyError::NotPorted` (honest gap list, each names the Fortran
routine + line range):**

- `cm2lab` + `f6cm`(g:8260–8518) + `f6ddx`(g:8520–8717) + `f6dis`(g:8719–8810) +
  `bach`(g:8812–8932) + `ll2lab`(g:8934–9061) + `f6lab`(g:9063–9338) +
  `getdis`(g:9340–9677) + `getaed`(g:9874–10239) — CM→lab kinematics + the
  scatter/production **matrix** feed function `ff(il,ig)`. Stubbed in
  `kinematics::cm2lab`.
- `getmf6`(g:7527–8133) / `getff`(g:7038–7412) / `getyld`(g:6521–6644) — File-6
  feed functions and yields. Not ported.
- `genflx`(g:5309–5684) / `getfwt`(g:5686–5775) / `stounr`(g:6802–6894) /
  `getunr`(g:6896–6994) — infinite-medium flux calculator + URR self-shielding.
  Not ported.
- `gtff`(ga:1162–1514) coherent/incoherent/pair-production photon matrices +
  `dspla` matrix branch (ga:1083–1128). Stubbed in `gpanel::gtff_matrix`.
- The `findf`/`gety1` PENDF tape retrieval that would feed a real reconstructed
  `sigma(E)` grid into `PointwiseXs::LinLin` (the panel average of a *supplied*
  grid is ported; wiring to a live PENDF/GENDF tape control flow is not).
- Discontinuity-nudging (`delta`/`rndoff`/`sigfig`, g:5895–5896,6013) omitted for
  the continuous-feeder core; only matters for tabulated data with true jumps.

## Public collapse / group-average entry point (for op-6tz.6.3 mgxs consumer)

The clean, documented entry point another subagent's mgxs test can consume:

```rust
use njoy_outram_park_fork::groupr::panel::{group_average_vector, GroupFlux, PointwiseXs};

// sigma_g[g] = ∫ sigma*phi dE / ∫ phi dE  over group [bounds[g], bounds[g+1])
let sigma_g: Vec<f64> = group_average_vector(&sigma, &flux, &group_bounds);
```

- `PointwiseXs::{Constant(f64), LinLin(Arc<Vec<(f64,f64)>>)}` — the σ(E) source.
- `GroupFlux::{Flat, Tabulated, Analytic{weight,temp_k,step}, Spectrum{spectrum,step}}`
  — the weight; `GroupFlux::analytic(..)` / `GroupFlux::spectrum(..)` use the
  default refinement step.
- Lower-level: `group_integral(sigma, flux, e_lo, e_hi) -> GroupIntegral` with
  `.flux` (∫phi), `.rate` (∫σφ), `.average()`.
- GAMINR re-exports the identical API under `gaminr::gpanel` /
  `gaminr::{group_average_vector, PointwiseXs, GroupFlux}`.

## Tests added (14 total; every one asserts a real property)

`groupr::panel` (7):

1. `constant_xs_averages_to_constant_under_any_weight` — σ≡3.7 → 3.7 in every
   group under flat, 1/E, and Watt weights (< 1e-12). Defining property ∫cφ/∫φ=c.
2. `flat_weight_linear_xs_equals_midpoint_value` — flat weight + linear σ ⇒
   average = σ at group midpoint = **3.5 barn** (hand-computable, < 1e-9).
3. `average_is_bounded_by_xs_extremes` — non-monotone σ∈[1,9] under 1/E ⇒ average
   ∈ [1,9] (≈4.16). Quadrature bounds invariant.
4. `constant_under_one_over_e_is_exact` — σ≡6 under 1/E ⇒ 6 (< 1e-12);
   independent check that numerator/denominator share the refinement grid.
5. `geometric_refinement_converges_to_analytic_integral` — linear σ under 1/E vs
   the closed-form `(E2-E1)/1e6 / ln(E2/E1) = 0.214976`; fine step within 1e-4 and
   no worse than coarse. Convergence property.
6. `feeder_tab_evaluation_and_breaks` — lin-lin feeder zero outside table,
   interpolates inside, `next_break` walks the grid upward.
7. `zero_flux_group_returns_zero` — zero weight over a group ⇒ 0.0, not NaN.

`groupr::gendf` (3):

8. `vector_section_round_trips` — build → `to_rows` → `from_rows` preserves every
   field + all group records; `sigma_vector` places σ at the right 1-based group.
9. `record_fields_are_in_endf_slots` — HEAD `[ZA,ZAM,NL,NZ,LRFLAG,NGN]` and LIST
   `[temp,0,NG2,IG2LO,NW,ig]` + `[flux,sigma]` land in the documented slots.
10. `tape_image_round_trips_and_rejects_truncation` — 2-section tape survives the
    image round trip; a clipped image returns `EndfParse` (no panic).

`groupr::kinematics` (1):

11. `cm2lab_reports_not_ported` — the CM→lab transform honestly reports
    `NotPorted("groupr::cm2lab")` (no fabricated kinematics).

`gaminr::gpanel` (3):

12. `photon_vector_average_via_shared_engine` — photoatomic σ≡0.42 → 0.42 in every
    photon group via the shared engine.
13. `photon_gendf_section_round_trips` — `mf=23` photoatomic section round-trips.
14. `gtff_matrix_reports_not_ported` — photon matrix feed function reports
    `NotPorted("gaminr::gtff")`.

## Cargo result

- `cargo build -p njoy-outram-park-fork --release` — clean.
- `cargo check -p njoy-outram-park-fork --lib --tests --release` — **0 warnings**
  (my four files); line counts 556/471/63/112 (all < 1000-line cap).
- Per-module test runs (memory-capped `scripts/test.sh`):
  - `groupr::panel` → `test result: ok. 7 passed; 0 failed`.
  - `groupr::gendf` → `test result: ok. 3 passed; 0 failed`.
  - `groupr::kinematics` → `test result: ok. 1 passed; 0 failed`.
  - `gaminr::gpanel` → `test result: ok. 3 passed; 0 failed`.
- Full lib suite (`scripts/test.sh`): `324 passed; 2 failed`. **The 2 failures are
  in concurrent peers' in-progress modules** (`covr::boxer::symmetric_round_trip_and_mirror`,
  `mixr::driver::driver_matches_direct_mix`) — **not** in `groupr`/`gaminr`, and
  outside this bead's scope. All 14 tests added here are in the 324 passing.
  (Caveat: this shared worktree has ~7 concurrent agents; the tree intermittently
  fails to compile mid-run as peers save files — the numbers above are from a
  momentarily-consistent tree.)

## Verification gap (for the human reviewer)

The quadrature and the GENDF layout reproduce the *documented* NJOY algorithm and
record fields, and pass property + one hand-value test. What is **not** yet done:
a golden-file comparison against an actual NJOY2016 GROUPR/GAMINR GENDF tape for a
real nuclide (e.g. a coarse-group `sigma_g` for U-238 capture vs upstream). That
is the recommended next V&V step before promoting this past "Prototype".
