# CFD, thermal-hydraulics and GeN-Foam web demos — scoping

**Status: scoping only. Nothing here is built.** Written 2026-10-13 at the
maintainer's request ("take a look at the htr10 Monte Carlo demo … think of how
you can do it for outram foam, web demos … things that help me understand CFD
and thermal hydraulics … and also GeNFoam").

This document says what the Monte Carlo demo already gives us for free, the one
structural difference that matters, the one piece of framework that is missing,
and three proposed rung ladders. The governing rules are
[`lesson-philosophy.md`](./lesson-philosophy.md) and
[`../claude-md/mobile-first-tutorials-and-demos.md`](../claude-md/mobile-first-tutorials-and-demos.md);
nothing here relaxes either.

---

## 1. What transfers for free

`dhoby_ghaut::web_demo` was extracted from `monte_carlo_web` on 2026-10-04
(gh:#521) precisely so a second track would not copy it, and six tracks now
build on it (`monte_carlo`, `delta_tracking`, `nuclear_data`, `dispersion`,
`triso_atops`, `dem`). A CFD track is the seventh and needs no new plumbing:

| piece | module | fits CFD as-is? |
|---|---|---|
| main view, zoom/pan/fit, zoom buttons, scale bar | `web_demo::view` | yes — a mesh is a world-cm object like a CSG slice |
| folding side panel, mobile layout | `web_demo::panel` | yes |
| worker/thread link, `Request`/`Event`, non-blocking drain | `web_demo::link` | yes — see §2 for what the messages carry |
| rung table, `?rung=`, lesson links, CI cross-check | `web_demo::lesson` | yes |
| worker pool sizing | `web_demo::pool` | only for a parameter sweep; one worker is enough per case |
| job-progress loading card | `web_demo::loading` | **largely unneeded** — see §2 |
| processed-nuclear-data IndexedDB cache | `web_demo::data_cache` | **unneeded** for CFD/TH; needed for the GeN-Foam track only if it generates its own `nuclearData` |

Every crate a CFD track would call is already **wasm in-scope**:
`scripts/check-wasm.sh` excludes only `kovan`, `kovan-discovery`,
`kovan-metrics`, `kovan-semantics` and `bedok`. `outram-foam-basic-lib`,
`outram-foam-appbuilder-lib` (which hosts the GeN-Foam port),
`outram-foam-turbulence-lib`, `outram-foam-multiphase`,
`tuas_boussinesq_solver` and `tampines` all pass the gate today.

**Caveat the gate does not cover** (stated in the root `CLAUDE.md`): compiling
is not running. The gate is `--lib` only and `std::thread`, `Instant` and
`std::fs` all compile for wasm and fail at run time. Each track's first task is
to find out which of these the solver loops actually touch — not to assume the
gate settles it. **Done for `RhoCentralFoam`, 2026-10-16 (gh:#893): zero host
imports, 0.26 s in a worker for the full Sod case — see §9.** Every other
solver still needs the same check.

## 2. The one structural difference: there is no data to prepare

This is the whole design consequence, so it goes first.

- **Monte Carlo's cost is front-loaded.** Processing U-235 and U-238 from ENDF
  is ~70 s *each* (gh:#528), the htr10 full-core load is 67–128 s on 2–4
  workers, and only then do histories run. `Loading`, `data_cache`, the worker
  pool and the tape-publishing step in `build-pages.sh` all exist to survive
  that.
- **CFD's cost is the solve itself.** There are no tapes, no download, no
  IndexedDB, no progress bar before first paint. A rung opens on a mesh
  *immediately*. The expense is the timestep loop with a linear solve inside
  it.

So the streaming unit changes from *a history / a generation* to **a timestep,
or one outer PIMPLE corrector**. Concretely the `Event` stream a CFD engine
sends back is not tracks but:

1. a **field snapshot** every N steps (cell values, flat `f32`, plus the
   min/max for the colour bar), and
2. a **residual line** per outer iteration — initial and final residual per
   equation, which is exactly what an OpenFOAM user watches scroll past and
   least understands.

That second one is the teaching payload, and it is free: the solvers already
compute it. A rung that shows residuals falling, then shows what a bad scheme
choice or a too-large Δt does to them, teaches more about practical CFD than
any field plot.

**This also makes the demos cheaper and more robust than the MC ones.** No
tape publishing to add to `build-pages.sh`, no cache-miss UI, and the
"no lagging" hard rule is easier to meet because a timestep is naturally short.

## 3. The one piece of framework that is missing

Everything in §1 transfers except the **main view's content**. The MC demos
draw neutron tracks over a CSG material slice (`raster.rs`, `anim.rs`). CFD
needs a field view, and all three proposed tracks need the same one, so it
belongs in the library as a seventh `web_demo` module — presentation only, no
physics, per the crate rule:

`web_demo::field` —
- a cell-coloured mesh plot (one filled polygon or quad per cell, from the
  `FvMesh`'s own cell/face geometry — **drawn from the mesh the solver holds,
  never a re-model**, per the drawing hard rule in every `outram-foam-*`
  `CLAUDE.md`);
- a colour bar with units and min/max, and a fixed-vs-autoscale toggle (an
  autoscaling colour bar hides a diverging solution — a real teaching point);
- optional vector glyphs on cell centres, and a streamline option later;
- a **probe**: tap a cell, read its value — the mobile-friendly equivalent of
  `postProcessing`.

Plus a small `web_demo::residual` for the log-y convergence plot. **Both must
be hand-drawn with the `egui::Painter`**: `egui_plot` is declared only in
`dhoby-ghaut`'s native-only dev-dependency table and is *not* available on
wasm. `examples/monte_carlo_web/sweep.rs` is the worked example to copy — grid,
axes, error bars, and `LineStyle::{Ours, Reference}` as dotted vs dashed, which
is already the house convention for "our result against the reference".

## 4. Track A — `cfd_web`: how a finite-volume solver works

The ladder follows **the discretisation, not the application**, because that is
what a reader needs to understand before any applied case means anything. Every
rung is cheap and every rung has a reference that can fail.

| Rung | System | Adds | Check that can fail |
|---|---|---|---|
| `diffusion` | 1-D heat conduction in a slab | the cell, the face, the flux, `fvm::laplacian`, the matrix it assembles | analytic steady profile; conservation to 1e-12 (the crate's own maturity bar) |
| `advect` | 1-D scalar in a uniform flow | convection, and the first real problem: upwind smears, linear oscillates | exact profile; `fvc::Limiter`'s Upwind/Linear/VanLeer/Minmod side by side |
| `nonortho` | the same slab on a **sheared** mesh | mesh non-orthogonality; `deltaCoeffs` vs `nonOrthDeltaCoeffs` and the 0.05 floor | **a real port defect worth 609 pcm**, found by reading upstream (see below) |
| `cavity` | 2-D lid-driven cavity | pressure–velocity coupling, why `p` needs its own equation, PISO/PIMPLE | Ghia et al. centreline profiles; the 20/40/80 grid-convergence study already in `tutorials/pimple_foam_cavity.rs` |
| `sod` | 1-D shock tube | compressibility; shock, contact and expansion | **the exact Riemann solution** — already this crate's declared maturity bar |
| `turb` | cavity or channel with k-ω SST | why RANS exists; what a wall function does | the log law; the upstream OpenFOAM reference run |

**`nonortho` should land early, and is the single best teaching artifact this
workspace owns.** On a sheared mesh `uncorrected` is *not* `orthogonal`:
OpenFOAM's `uncorrectedSnGrad::deltaCoeffs()` returns `nonOrthDeltaCoeffs`,
`1/max(n̂·d, 0.05|d|)`. The port used the plain centre-to-centre distance, and
that one substitution was worth **609 pcm** of the 788 pcm MSFR discrepancy
against upstream GeN-Foam — the largest of the three causes, ahead of the
unported precursor drift (178 pcm) and the cross-section parametrisation
(14 pcm, which was already correct). A rung that lets the reader flip the
delta-coefficient mode on a sheared mesh and watch the answer move is a lesson
in the "read upstream first" rule *and* in why mesh quality is not cosmetic.

## 5. Track B — `thermal_hydraulics_web`: from one node to a loop

TUAS and `tampines`. This is the "helps me understand thermal hydraulics" half,
and it is the track with **real experimental validation** at the top.

| Rung | System | Adds | Check |
|---|---|---|---|
| `node` | one control volume, heater on | the energy balance and its time constant | analytic exponential |
| `pipe` | 1-D fluid array, inlet and outlet | axial advection, the enthalpy march | analytic; TUAS's own unit tests |
| `htc` | the same pipe, Nusselt correlations | Dittus-Boelter vs Gnielinski, laminar ↔ turbulent | **TUAS's 2 % Gnielinski bar** |
| `natcirc` | heated leg, cooled leg, a loop | buoyancy driving flow; the ΔP balance that sets it | **CIET, outlet temperature within 0.2 °C** — real validation |
| `dracs` | the coupled DRACS loop | two coupled natural-circulation loops | the recorded CIET/SAM comparison in TUAS's V&V folder |
| `pebblebed` | HTR-10 bed: KTA friction, ZBS conductivity | porous media and effective properties | the KTA/ZBS correlations; links across to the MC track's `htr10` rung |

**`natcirc` and `dracs` must be recorded, not live.** The root `CLAUDE.md` is
explicit that the CIET coupled-DRACS cases integrate 2000–2500 s of simulated
time at a 0.1 s timestep and take a very long wall clock — hours. That is the
same situation as the MC track's `layers` view, and it has the same answer: the
framework already supports a recorded curve read with a slider
(`sweep.rs`/`RecordedSweep`) and recorded playback
(`McRung::recorded_tracks`). The live rungs are `node` through `htc`, which are
genuinely cheap; the loop rungs show the record, faithfully labelled, with the
live solver offered as a native run.

## 6. Track C — `genfoam_web`: neutronics coupled to heat

Where the two tracks above meet. Note the upstream tutorial ladder is itself
called `guidedCases` and the `foam_vv_cases` suite already mirrors its first
rung (`1_reactorSlab_1D_1Gr_neutronicDiffusion` → `genfoam_slab`), so the
ladder is partly specified by upstream already.

| Rung | System | Adds | Check |
|---|---|---|---|
| `slab` | 1-group diffusion in a 1-D slab | diffusion as an **eigenvalue** problem, not a transient | the exact analytic slab k — already the `genfoam_slab` V&V case |
| `twogroup` | 2-group slab | fast and thermal groups, the scattering matrix | analytic |
| `albedo` | slab with an albedo boundary | what a reflector does; the Robin BC | upstream GeN-Foam |
| `sp3` | the same case, diffusion vs SP3 | the transport correction, and where diffusion fails | upstream GeN-Foam, the two drawn side by side |
| `drift` | circulating fuel (MSFR) | **delayed-neutron precursor drift**: the fuel moves, so the delayed source moves with it | **upstream GeN-Foam to +0.2 pcm** |
| `coupled` | slab plus TH feedback | Picard coupling, Doppler, the feedback that makes a reactor stable | the workbench's Step 10 mesh study |

**`drift` is the capstone.** It cannot be explained without both halves of the
track, and it has the most surprising demo in the whole set: the same geometry,
the pump switched off and on, and k changing because the delayed neutrons are
now born somewhere the fuel has carried them. The measured verification table
behind it — reflective 1.075834 vs upstream 1.075830, vacuum 0.950415 vs
0.950409, albedo 0.973858 vs 0.973858, per-cell with drift 0.960314 vs
0.960312 — is the kind of check the lesson philosophy asks for, and it exists
already.

## 7. Sequencing, and what to build first

1. **`web_demo::field` + `web_demo::residual`** (§3). Nothing else can start
   without them, they are presentation-only, and all three tracks share them.
2. **Track A rungs `diffusion` and `nonortho`.** Smallest possible solver
   surface, no mesh I/O, exact references, and `nonortho` immediately pays for
   the whole exercise as a lesson.
3. **Track C rung `slab`**, because `genfoam_slab` already exists as a V&V case
   with an analytic reference — it is the cheapest proof that the GeN-Foam port
   can drive a browser demo at all.
4. Then `cavity`/`sod` (Track A), `node`→`htc` (Track B), and the recorded loop
   rungs last.

Each needs its lesson page in a new mdBook under
`crates/outram-foam-appbuilder-lib/docs/tutorial/` (and one under
`crates/tuas_boussinesq_solver/docs/tutorial/` for Track B), mirroring
`crates/outram-mc-libs/docs/tutorial/`, plus the rung cross-check wired into
`scripts/build-pages.sh` the way the Monte Carlo, dispersion and nuclear-data
tracks already are.

## 8. Open questions for the maintainer

- **Three tracks or one?** Written here as three ladders because CFD
  discretisation, system thermal-hydraulics and coupled reactor physics are
  genuinely different subjects with different references. One merged ladder
  would be long and would mix fidelities.
- **Does Track B belong to TUAS or to `tampines`?** The rungs span both.
- **Is a native-only rung acceptable** for the hours-long CIET loop cases, with
  the browser showing the record? That is what §5 assumes.

## 9. What has NOT been established

Stated plainly, because none of it has been measured yet:

- ~~**No timing has been taken in wasm.**~~ **CORRECTED 2026-10-16 (gh:#893)**:
  measured. The same Sod solve (100 cells, Δt = 1e-6, 6325 steps), built for
  `wasm32-unknown-unknown` and run in a Node `worker_threads` Worker on V8,
  takes **0.26 s** against **0.24 s** native. Across four Δt the ratio is
  1.1–1.6×, about 1.3×. This scoping had guessed "slower by a factor of a
  few"; the guess was pessimistic. Answers agree to round-off (≤ 9e-16
  relative on the mass checksum), not bit for bit. Record, methodology and
  how to reproduce:
  `crates/outram-foam-appbuilder-lib/verification_and_validation/wasm_worker_timing/`.
  (The original native figure stands: the four-test Sod suite takes 1.25 s,
  measured 2026-10-13.) **Still unmeasured:** a real browser (Safari is not
  V8), a phone, and any solver with an implicit pressure or eigenvalue solve.
  Sod is fully explicit and therefore the cheapest case.
- ~~**No foam solver has been run in a Web Worker at all.** Whether the solver
  loops touch `std::thread`, `Instant` or `std::fs` — all of which compile for
  wasm and fail at run time — is unchecked.~~ **CORRECTED 2026-10-16**:
  `RhoCentralFoam` has run in one. Its wasm module has **zero imports**, so
  the solve path makes no host calls. `rayon` is behind the off-by-default
  `parallel` feature of `outram-foam-basic-lib`, and it stays unchecked on
  wasm.
- **`outram-foam-appbuilder-lib` has no field writer**: every function in
  `io::output` is `todo!()`. That does not block a demo (the demo reads the
  solver's public fields directly, as its tests do) but it does mean there is
  no file-based path for getting fields out.
- The Ghia et al. cavity reference and the log-law reference named in §4 are
  **not** in `kovan-literature` — checked 2026-10-13 against
  `crates/kovan-literature/CATALOGUE.md` (761 lines): no entry matches `ghia`,
  `lid-driven`, `log-law` or `cavity` other than an unrelated HTR-10
  core-cavity line. The maintainer supplies the kovan corpus, so both must be
  catalogued by them (through `kovan lit import`, with the access tier decided
  from the copyright page) before any page quotes them — the literature hard rule. Until then the `cavity` and `turb` rungs
  have no citable reference and cannot claim one.
