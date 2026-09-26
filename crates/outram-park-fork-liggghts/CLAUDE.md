# CLAUDE.md — outram-park-fork-liggghts


Pure-Rust granular-DEM library: particles, contact mechanics, thermal DEM,
pebble/packed-bed physics. Ports LIGGGHTS/LAMMPS-granular.

> This crate is a member of the **OUTRAM PARK** workspace
> (`crates/outram-park-fork-liggghts`). See the workspace root `CLAUDE.md` for
> the shared dependency policy. Dependencies are inherited from the root
> `[workspace.dependencies]` — do not pin versions in this crate's
> `Cargo.toml`.

## Reactor geometry is DRAWN for a human to check before it is trusted (HARD RULE)

**Maintainer direction, 2026-09-25.** Binds this crate. The same rule is in the
`CLAUDE.md` of `outram-mc-libs`, `nee_soon`, every `outram-foam-*` crate and
every crate downstream of them; a crate that newly depends on one of those
takes the rule into its own `CLAUDE.md` (check with `cargo metadata`).

**Whenever you build or change a complex reactor geometry** — CSG cells and
surfaces, lattices, pebble beds, TRISO particles, reflector zones, control-rod
bands, a CFD/FEM mesh, anything a solver will transport or integrate through —
**draw it, as images a human can open (PNG, or JPG/SVG), and hand them over**
before any result computed on it is reported as more than tentative.

- **Draw what the solver sees, not what you meant.** Render from the ASSEMBLED
  geometry (cell / material lookup at each pixel, or the mesh itself), never
  from the named constants. A picture of the constants hides exactly the
  defects this rule exists to catch.
- **Minimum set:** an axial slice (R-Z / x-z) of the whole model; radial (x-y)
  slices at the heights that matter; and, for nested geometry, zoomed slices at
  every level down to the smallest (pebble, TRISO particle). Colour by
  material, with a legend and the key dimensions marked.
- **Commit the images with the change** (beside the V&V record or the
  manuscript package) and point the human at them by path in your summary.
  Regenerate them whenever the geometry changes.
- **Say what you checked in them, and what you could not** — an image nobody
  was told to look at checks nothing.

**Why.** On 2026-09-24/25 the HTR-10 model carried, at once: a bottom reflector
mirrored from the top and up to 107 cm short; a core cavity that grew with the
bed; pebbles interpenetrating by 1.1 cm with 4.8 % of core carbon clipped away
(gh:#309, #310); and a TRISO lattice holding 8240 particles while reporting
8340 (gh:#316, +353 pcm). Every run completed with green diagnostics. Each was
found by looking at the built geometry, not by the eigenvalue.

**Tools.** ~~`outram_mc_libs::geometry::plot` samples a slice of an assembled
CSG geometry; OpenMC-parity image output (PNG/JPG) is the preferred path once
it lands.~~ **UPDATED 2026-09-25 — it has landed (gh:#268):**
`outram_mc_libs::geometry::plot` is a port of OpenMC's plotter that writes PNG
directly — slices, wireframe and solid ray traces — verified pixel-for-pixel
against `openmc --plot`
(`crates/outram-mc-libs/verification_and_validation/geometry_plotting/`).
`render_material_slice` draws a material-coloured slice with a legend and cm
axes in one call; `crates/nee_soon/examples/htr10_geometry_images.rs` is the
worked example. For meshes, plot the mesh itself (cells, patches, zones).

## Maturity: DECLARED MATURE (2026-09-06)

The API-usability rules in the root `CLAUDE.md` ("Human interface layer", and
the Haiku dogfooding hard rule) **are in force for this crate**. See the
maturity gate in that file for what this means and how the bar is revised.

Declared because LIGGGHTS/DEM is now on the HTR-10 pebble-bed critical path
(`op-jyyp`) and on the HTGR calibration path (gh #148), so its public API needs
the same quality bar — **not** because its granular physics is validated. Those
are different claims and this entry keeps them apart.

- **2026-09-06 — mature.** Bar: the **numerics** are verified against
  closed-form solutions. The integrator reproduces free flight under gravity as
  the analytical parabola and `omega(t) = (tau / I) * t` for angular
  acceleration; the Hertz/Hooke/Mindlin contact force laws are checked against
  **hand-computed analytical values**; signed-distance functions for plane,
  wall, box, cylinder and sphere are checked at known points (after I. Quilez,
  "Distance functions"). Evidence class: **analytical / manufactured
  solution**, supported by unit tests and internal consistency.

  Measured at declaration: **72 tests pass, 0 fail, 0 ignored.**

  **What this bar does NOT cover, and the distinction matters.** What is
  verified is the *algebra and the integrator*: given a contact law, is it
  evaluated correctly; given forces, is the trajectory integrated correctly.

  What is **not** validated is the **granular physics** — packing fractions,
  bulk flow behaviour, heat transfer through a bed, wall effects, or agreement
  with LIGGGHTS proper on any case. There is no cross-code comparison against
  upstream LIGGGHTS and no experimental comparison in this repository.

  **Do not cite this declaration as evidence that a DEM-settled pebble bed is
  physically right.** A correct integrator over an uncalibrated contact model
  produces a precisely-computed wrong answer. Closing that gap needs a
  cross-code run against LIGGGHTS proper or a published granular benchmark,
  and neither exists yet.

- **2026-09-15 — cross-code leg added; the 2026-09-06 entry above stands as
  what was accepted before.** The "no cross-code comparison against upstream
  LIGGGHTS" gap it records is now **closed**; the "no experimental comparison"
  gap is **not**. Upstream LIGGGHTS-PUBLIC (commit `3d5c00f2`) was **built from
  source and run**; its trajectories are committed under
  `reference-data/liggghts/` (same standard as `reference-data/gsl/` in
  `petir`). Evidence class: **cross-code comparison**, on top of the
  analytical/MMS leg above.

  Bar: reproduce upstream LIGGGHTS to floating-point round-off on the
  deterministic contact cases, and to better than 2 % bulk packing fraction on
  a settled bed. Measured — every sampled frame compared, not just endpoints:

  | Case | Frames | agreement |
  |---|---|---|
  | head-on, Hertz | 251 | **bit-identical** |
  | head-on, Hooke | 251 | **bit-identical** |
  | wall bounce + gravity (400 000 steps) | 2001 | **bit-identical** |
  | rolling resistance, CDT (`µ_r = 0.1`) | 201 | **bit-identical** |
  | oblique + friction (shear history, slip, torque) | 251 | `max|Δv| = 1.11e-16 m/s`, `max|Δω| = 5.68e-14 rad/s` (≈1–3 ulp) |
  | oblique, **no-history** — the stateless `contact`+`simulation` path | 251 | `max|Δv| = 1.11e-16 m/s`, `max|Δω| = 1.42e-14 rad/s` |
  | bulk bed, 354 pebbles, `D/d = 6` | settled state | `φ = 0.5571` vs `0.5582` — **0.20 %** |
  | angle of repose, 656 pebbles, lifting cylinder (mesh + `move/mesh`) | settled heap | ~~`12.78°` vs `15.43°` — 2.65°~~ **`14.09°` vs `15.43°` — 1.34°** (**CORRECTED 2026-09-18**: 12.78 came from the pre-`op-t3l.9` nondeterministic build and is not reproducible; the gap HALVES. The mesh-contact attribution is now UNSUPPORTED — a resolution sweep is flat within a measured 0.266° realisation scatter. See the V&V doc.) |
  | **HTR-10 full core**, 27 554 pebbles, `D/d = 30`, `E = 5e8` | settled state | `φ = 0.5732` vs `0.5732` — **4 decimals**; median pebble **61 µm** apart, 99.93 % within 1 mm |

  The HTR-10 row (added 2026-09-17) is the one that matters for the pebble-bed
  work, because it is the geometry every downstream consumer uses. Its
  per-particle result is stronger than any bulk number: after 50 000
  independently integrated steps each, the two codes put essentially every
  individual pebble in the same place. The consolidated single-table view of
  every case, with the upstream model file each is judged against and what the
  comparison does NOT cover, is
  [`docs/cross-code-summary.md`](./docs/cross-code-summary.md).

  Measured at this entry: **114 tests pass** (108 unit + 6 cross-code), plus two
  slow cases that were `#[ignore]`d at the time. **Both now run in an ordinary
  suite** (2026-09-16), gated on the default-on `long-tests` feature — see the
  "Any test over 5 minutes" HARD RULE in the workspace `CLAUDE.md`. Neither
  number in the table above was protected by the default suite before this,
  and both had to be timed for the first time to gate them: bulk packing
  **317 s** (documented as ~210 s) and angle of repose **2313 s / 38.5 min**
  (documented as ~10 min here and 25 min in this file). Both passed. A default
  `cargo test` for this crate therefore now costs about 45 minutes; use
  `cargo quick-test -p outram-park-fork-liggghts` while iterating. Full methodology and results:
  [`docs/verification-and-validation.md`](docs/verification-and-validation.md).

  **Defects found — and, as of 2026-09-16, FIXED rather than merely
  documented** (full detail in that file's § 4):
  `Particle::integrate` is not symplectic despite its doc comment claiming it
  was (an elastic collision rebounded with restitution `1.0031` at
  `dt = 1 µs` — it manufactured energy); `contact.rs` used the particle radii
  where upstream uses the contact radii `c_r = r − δ_n/2`; and `rolling.rs`'s
  CDT used the damped `|F_n|` where upstream uses the elastic `k_n·δ_n`
  (21 % apart) and kept the torsion component upstream drops. All three are now
  upstream's, both engines run `integrator::VelocityVerlet`, and the stateless
  path is itself cross-code verified against `tangential no_history`.

  One divergence recorded on 2026-09-15 turned out **not to be real**: the
  claim that `contact.rs` mishandles the tangential-damping branch came from
  comparing it against upstream's *history* model. Against
  `tangential_model_no_history.h`, which is what it implements, upstream caps
  the damping coefficient exactly as `contact.rs` does. Nothing was changed
  there and the claim is retracted in the V&V document.

  **What this still does NOT establish.** It shows this crate reproduces
  LIGGGHTS. It does **not** show LIGGGHTS' granular physics is right for an
  HTR-10 bed: there is still **no experimental comparison** anywhere in this
  repository. An **angle-of-repose** case — the canonical granular validation,
  and the most valuable single addition this crate could get — was attempted
  on 2026-09-15 and **abandoned unresolved**; the three setups tried and why
  each was invalid are recorded in the V&V document § 4.5, so the next attempt
  does not repeat them. **Do not quote an angle-of-repose number from this
  work.** For reference, the settled voidage both codes produce
  (`ε ≈ 0.442`) sits 2.2 percentage points above the Dixon (1988) correlation
  for `D/d = 6` (`ε = 0.4198`) — an observation, not a validation, and no
  calibration of `µ`/`e`/`E` against bed data has been attempted.

  Sphere packing does **not** live in this crate — it is in
  `outram-mc-libs/src/pebble_beds/crp_packing.rs` (Jodrey-Tory CRP). Whether it
  belongs here instead is an open architectural question (gh #65); DEM-settled
  beds and CRP-generated beds are different things carrying different validity
  claims.

- **2026-09-17 — porosity map added as a regression; no change to the bar.**
  `tests/pebble_bed_porosity.rs` maps the **radial and axial porosity profile**
  of LIGGGHTS' own settled bed and commits all 30 radial + 128 axial bins as a
  fixture. This adds *structure* to what was previously a single bulk number,
  and it is the quantity wall channelling depends on. Section 3.1 of
  `docs/verification-and-validation.md` has the full methodology and results.

  Headline: the profile is **oscillatory**, `eps = 0.8753` in the wall bin,
  first minimum **0.2631 at `y/d = 0.55`**, first maximum **0.5533 at
  `y/d = 1.05`**, period `0.90 d`, amplitude damping `0.2902 -> 0.1519`. The
  estimator is deterministic, and its area-weighted radial mean (`0.442993`)
  and axial interior mean (`0.440089`) both reproduce the independently
  recorded bulk voidage `0.4429` to within 0.3 %.

  **Two things this deliberately does not claim.** It is not validated against
  a published radial-voidage correlation — Mueller (1992), de Klerk (2003),
  Benenati and Brosilow (1962) are the obvious gates and **none is catalogued
  in `crates/kovan-literature`**, so the test asserts structural properties and
  a self-regression only. And the reference bed is `D/d = 6`, too narrow to
  watch the oscillation damp to bulk; HTR-10 is `D/d = 30`.

  **It also records a structural contradiction with `tampines`.**
  `tampines::pebble_bed::zbs::ZbsBed::wall_region_porosity` is a monotonic
  exponential and can never dip below `eps_bulk`; the measured profile reaches
  0.263, and the sign of the error flips between the first minimum and the
  first maximum. That placeholder cannot be repaired by refitting — it has the
  wrong shape. Changing `tampines` is a separate change with its own V&V and
  was **not** made on the strength of this.

  Version is `0.0.0` and the crate has **no prelude** at declaration time. Both
  tracked in gh #64. Absent prelude was the single most predictive defect
  across the seven crates dogfooded in gh #58 — the two worst failures there
  were both prelude failures — so this crate starts from behind.

- **2026-09-17 — mesh-wall cross-code leg extended to the HTR-10 DISCHARGE
  geometry, plus a reproducibility defect found and fixed. The bar itself is
  UNCHANGED; revising it is a maintainer decision.** The 2026-09-15 entry above
  stands as what was accepted before.

  **Why this entry exists.** The recirculation study runs on the published
  bottom conus and fuel discharge tube — a *triangulated mesh wall*, not the
  primitive cylinder+plane every previous HTR-10 result used. The only other
  mesh-wall comparison in this crate is the angle-of-repose case, which is the
  **weakest row in the whole cross-code table** (~~12.78°~~ **14.09°** vs 15.43°,
  corrected 2026-09-18 — still the weakest, but by 1.34° not 2.65°). Drawing a
  physics conclusion from untested geometry would have been exactly the mistake
  the workspace V&V rules exist to prevent, so the conus got its own
  comparison first: `tests/htr10_conus_cross_code.rs`, upstream deck
  `reference-data/liggghts/in.htr10_conus`.

  Both codes start from the identical configuration (LIGGGHTS' own settled bed,
  via the new `csv2data.sh`) and read the **same STL**. Measured:

  | step | `φ` ours vs LIGGGHTS | surface height | per-particle median |
  |---|---|---|---|
  | 2 000 | 0.5646 vs 0.5646, **−0.00 %** | **+0.0 mm** | **11 µm**, 27 554/27 554 within 1 mm |
  | 6 000 | 0.5759 vs 0.5759, **−0.00 %** | **+0.1 mm** | 1.10 mm, 12 755/27 554 within 1 mm |

  **The mesh-wall contact path is verified** — at early time, before chaos acts,
  every one of 27 554 pebbles is within 1 mm and the median is 11 µm. The
  later growth is **Lyapunov divergence, not disagreement**: this case drains a
  2.17 m column 17 cm into a funnel, a large rearrangement in which pebbles
  change neighbours, and dense granular flow is chaotic. Bulk statistics agree
  to four decimals throughout, which is what survives. The test asserts
  accordingly — tightly early, loosely late — and says so.

  **A real defect, found while parallelising and now fixed.** The neighbour grid
  was a `std::collections::HashMap`, whose iteration order is **randomly seeded
  per process**; since force accumulation is floating-point and addition is not
  associative, *every run gave a different answer*. Three identical 200-step
  runs of the settled bed gave kinetic energies `2.81519841188424304e-2`,
  `…18857e-2`, `…32977e-2` — the 13th significant figure, in a system chaotic
  over 50 000 steps. **`htr10_settled_ours.csv` could not be regenerated
  exactly**, and the single 2.91e-2 m outlier in the per-particle comparison
  above is consistent with this mechanism. Replaced by a deterministic
  counting-sorted flat cell list. Bead `op-t3l.9`.

  **Compute backends.** `ComputeType` / `ThreadCount` (`src/compute.rs`) mirror
  `outram-mc-libs`, with one semantic deliberately stricter: the rayon backend
  must be **bit-identical** to the scalar reference, not merely agree within
  uncertainty, because this crate's whole claim is bit-identical agreement with
  LIGGGHTS. Verified by identical checksums. Measured on the settled HTR-10
  bed: 55.95 → **18.34** ms/step scalar (defect fixes) → **12.85** ms/step on 12
  threads. The parallel gain is capped by Amdahl — bit-identity requires an
  ordered accumulation phase that cannot be parallelised.

  **Bed structure is now measured, not just density.** `src/rdf.rs` +
  `tests/htr10_rdf.rs` compute `g(r)`; both codes agree on contact coordination
  number (**8.16**) and peak position (`0.990 d`), with peak heights 17.635 vs
  17.588. Full methodology and results in
  [`docs/verification-and-validation.md`](docs/verification-and-validation.md)
  §§ 3.2–3.3.

  **The 0.61 question is answered, and the answer is friction.** A 2x2 friction
  ablation (V&V § 4.9) reaches a whole-core closure of **0.6047 against the
  published 0.61** at `mu = 0.1, mu_r = 0` — the *literature* value for
  graphite-on-graphite, graphite being a solid lubricant — where this crate's
  previous `mu = 0.4` gave 0.5729. `mu` is worth `+0.0196` in `phi` and `mu_r`
  a further `+0.007..0.012`. Nothing was fitted: all four cells were run and all
  four are reported.

  **Slow recirculation does NOT explain the gap**, contrary to the hypothesis on
  record in GitHub issue #216. Over 7.3 % of the bed it moves `phi` by at most
  0.007, against friction's 0.031 — and its **sign depends on friction**:
  `+0.0025` at `mu = 0.1, mu_r = 0` but `-0.0073` at `mu = 0.4, mu_r = 0.1`.
  Low-friction beds re-compact after each disturbance; high-friction beds
  cannot, and dilate.

  **Every bed is a random packing, measured not assumed.** `g(r)` for all 11
  committed beds shows `g(sqrt2 d)` as a trough (0.617-0.691) with no FCC peak,
  and split second peaks at `sqrt3 d` / `2 d`. The **densest** bed is
  simultaneously the **most** random-close-packed and the **least**
  crystalline.

  **A caveat that was measured, not smoothed over — and it revised two
  numbers.** The 40-batch runs used a 2 000-step settle window and came in at
  `1.57e-2` against the quasi-static bound of `1e-2`, i.e. **outside the regime
  they claim**. Raising the window to 8 000 steps drops that to `~2e-4`, and the
  committed test now uses 8 000. Relaxing the threshold instead was considered
  and rejected: the bound is the only reason the case can claim to measure
  creep rather than avalanching.

  A rate-independence control (two cells re-run at 8 000 steps from the
  identical settled beds, both genuinely quasi-static at `2.2e-4` and `4.8e-4`)
  then tested whether the result survived. **The sign flip does** — `+0.0015`
  at `mu = 0.1` against `-0.0016` at `mu = 0.4` over the same 500 pebbles — but
  the 2 000-step **magnitudes were inflated about twofold**, and its `-0.00245`
  whole-core loss at `mu = 0.1` was an outright artefact (`+0.0001` once
  relaxed). **Quote the control, not the 40-batch table.** The qualitative
  conclusion is unchanged and slightly strengthened: the quasi-static effect is
  smaller still, so recirculation explains even less of the 0.61 gap.

  **Still NOT closed by any of this:** there remains **no experimental
  comparison** anywhere in this repository, and no published `g(r)` or
  radial-voidage correlation is catalogued in `crates/kovan-literature` to gate
  against. Everything above is verification.

## Scope

Modules: `bonded`, `boundary`, `compute`, `contact`, `coupling`, `gnn_bridge`
(feature `gnn`), `gpu` (non-Android, non-wasm), `granular`, `granular_system`,
`integrator`, `mesh_wall`, `particle`, `rdf`, `rolling`, `simulation`,
`thermal`, `thermal_radiation`, `timestep`.

Three of those are newer than the rest and are worth naming here:

- **`compute`** — the `ComputeType` / `ThreadCount` backend selector, mirroring
  `outram-mc-libs`. Read its module docs before using the parallel path: the
  rayon backend is **bit-identical** to the scalar one by design, and that
  constraint is why the speedup is modest.
- **`rdf`** — radial distribution function `g(r)`, i.e. bed *structure* rather
  than bed density. Distinguishes a random packing from a crystallising one,
  which a packing fraction cannot.
- **`gpu`** — one headless `wgpu` kernel, for the RDF histogram only. The DEM
  timestep deliberately has **no** GPU path; `compute`'s docs say why.

**Which engine to use.** There are two, deliberately:

- **`granular` + `granular_system`** — the LIGGGHTS-faithful path: shear
  history, upstream contact kinematics, kick-drift-kick velocity-Verlet. **Use
  this for anything that must settle, pack, or hold a static assembly**, i.e.
  every pebble-bed case. It is the path verified against upstream.
- **`contact` + `simulation`** — the original stateless path. No tangential
  spring (`ξ_t = 0` hard-coded), so a static assembly cannot carry shear and a
  heap has zero angle of repose. Kept for the instantaneous-force queries and
  constant-force cases it was written and tested for.

`integrator::VelocityVerlet` is a translation of `fix_nve_sphere.cpp` and is
what `granular_system` runs; `Particle::integrate` is **not** velocity-Verlet
(see its doc comment and the V&V document) and should not drive contacts.

`timestep` ports `fix check/timestep/gran` — use `TimestepEstimate::
recommended_dt` to pick `dt` rather than guessing; an over-long explicit step
does not merely lose accuracy, it ejects particles.

## Licensing

LIGGGHTS-PUBLIC is GPL-2-or-later (GPL-3-compatible; see `NOTICE`). This
translation is distributed under `GPL-3.0-only`, the same as the rest of the
workspace.
