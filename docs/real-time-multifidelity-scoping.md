# Real-time, multi-fidelity models of the scoped test reactors — scoping

**Date:** 2026-08-07 · **Status:** scoping analysis, not an approved plan.
**Audited against:** `develop` at `8c9ddd05`.

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified
> and untrusted** unless a specific verification & validation (V&V) case
> demonstrates otherwise. See `VERIFICATION_AND_VALIDATION.md` and
> `RESPONSIBLE_USE.md`. This document is AI-assisted draft material and has not
> been human-reviewed.

> **Intended use.** Education, research, capability building, and V&V only.
> Nothing here is for nuclear facility operation, reactor control, licensing,
> safety-critical decision-making, emergency response, or operational digital
> twin deployment. Every simulator discussed is an **offline demonstration**.

## How to read this

This document answers one question: *what would it actually take to run the
reactors already scoped in `docs/reactor-scoping/` as real-time, multi-fidelity
models?*

It does **not** re-derive the reactor scoping. It sits on top of
`docs/reactor-scoping/` (README plus `bwr.md`, `ebr2.md`, `fhr.md`,
`htr10.md`, `ipwr.md`, `msre.md`, `vtb-findings.md`),
`docs/type-i-digital-twin-scoping.md`, `docs/outram-park-dt-plan.md`,
`docs/architecture.md` and `docs/human-in-the-loop-ciet-v2-case-study.md`, and
corrects them where the code disagrees (§6).

**Measurement discipline.** Every timing number below is labelled by source:

- **[M]** — measured by this audit on this machine, 2026-08-07, release mode,
  with the command recorded so it can be re-run.
- **[C]** — committed measurement already in the repository (a V&V file, a
  README, a test doc comment), cited to `file:line` or file path.
- **[E]** — engineering estimate derived from an [M] or [C] figure by explicit
  arithmetic. Never a guess dressed as a measurement.

Machine for all **[M]** figures: the maintainer's Linux x86-64 development box,
release profile, 12 logical cores. **Timings are machine-specific.** Nothing
below was extrapolated to hardware not tested.

---

## 0. Verdict in one page

**One fidelity band in this workspace runs in real time today, and it is the
lowest one.** Lumped/system thermal-hydraulics plus point kinetics, wall-clock
paced, is demonstrated working — CIET v2 held **94.7 s of simulated time in
94.7 s of wall-clock** with a median compute cost of **27 ms per 100 ms
timestep** ([M], §3.1). Nothing above that band is close.

The measured gap to the next rung up is roughly **three orders of magnitude**,
and it does not close by optimisation:

| Band | Measured cost | Real-time verdict |
|---|---|---|
| Point kinetics (`teh-o-prke`) | **0.7–1.0 ns/step** for the delayed layer [C]; ~250 ns for the 7x7 implicit PRKE [E] | **Yes, with enormous headroom** |
| Lumped/system TH (`tuas`, `tampines`) | 27 ms per 0.1 s step [M]; 10x–25x faster than real time in committed regression tests [C] | **Yes** |
| Process control (`chem-eng-…`) | **per-step cost grows without bound** — 49 µs → 1.8 ms over 40k steps [C] | **No, as currently used — see §2 Band 1b** |
| Nodal diffusion + channel TH (`bedok`) | *cannot be measured — does not run* | **Unknown; not demonstrable** |
| CFD (`outram-foam-*`) | ~1.2 µs per cell per step [E] | **No above ~10^4 cells** |
| Circulating-fuel spatial eigenvalue (`moltres`) | 1.14 s per solve, 300 cells [M] | **No — 11x too slow for a 0.1 s TH step** |
| Monte Carlo (`outram-mc-libs`) | 1.4e6 histories/s [C] | **No — by 2 to 4 orders of magnitude** |
| Fuel performance (`offbeat`) | not measurable — no thermal solve | **No** |

**Two findings deserve the front page.**

**First: the control layer has unbounded per-step cost growth, and it is still
in the loop.** `chem-eng-real-time-process-control-simulator`'s transfer
functions accumulate a `Vec` of superposed step responses and sum it every step
(`crates/chem-eng-real-time-process-control-simulator/src/lib/beta_testing/stable_transfer_functions/first_order_transfer_fn.rs:16,113,134`,
with an O(n^2)-shaped prune at `:147-230`). `teh-o-prke` **measured** this exact
mechanism at **49 µs per step rising to 1.8 ms per step between step 1,000 and
step 40,000** and replaced it in its own delayed-neutron layer
(`crates/teh-o-prke/src/delayed_neutron_layer.rs:88`), dropping to **0.7–1.0
ns/step**, flat [C]. **The unbounded version is still the PID path TUAS imports
for its control loops.** A crate named "real-time process control simulator" is
the one component in this workspace whose cost is not bounded per step. Fixing
that is Tier 1.

**Second: nothing in this workspace is a surrogate or reduced-order model.** The
crate expected to supply them, `raffles`, has an empty `src/surrogate.rs` — 54
lines of doc comment and **zero lines of code** — and its own header says no
work is scheduled. Every multi-fidelity architecture proposed below therefore
has to either build its ROMs or do without them.

**There is also no benchmarking infrastructure at all.** No `criterion`
dependency and no `benches/` directory anywhere in the workspace; the single
in-repo microbenchmark is a hand-rolled `Instant::now()` pair inside a unit test
(`crates/teh-o-prke/src/delayed_neutron_layer.rs:554`). Every other performance
claim in the repository is prose in a doc comment, asserted by nothing.

**The recommended first demo is MSRE**, not FHR — see §5.3. It is the only case
where the expensive model and the cheap model both already exist, which makes
the surrogate's verification target free.

---

## 1. Per-reactor status

### 1.1 The slate is six reactors plus one facility

`docs/reactor-scoping/README.md` lists six reactors: HTR-10, MSRE, iPWR, BWR,
FHR, EBR-II. **That list is complete** — a sweep of every `examples/` and
`src/bin/` directory in the workspace found no seventh reactor.

It is, however, **incomplete in a different direction**: it omits the two
things that actually run.

- **CIET** is a thermal-hydraulic *test facility*, not a reactor, and so is
  absent from the reactor slate. It is nonetheless the only genuinely real-time,
  operator-in-the-loop, experimentally-validated model in the workspace, and
  every real-time claim below is anchored to it.
- **`htgr_sim_v1` is a prismatic HTGR**, distinct from the pebble-bed HTR-10 it
  is filed under. `docs/reactor-scoping/htr10.md:19-25` says so; the README's
  readiness table does not, and reads as though HTR-10 has an app shell.

Full inventory of simulator-shaped targets found:

| Target | Location | Kind |
|---|---|---|
| `ciet_educational_simulator_v2` | `crates/outram-park-digital-twin-engine/src/bin/` | bin, GUI + headless + OPC-UA |
| `ciet_educational_simulator` (v1) | `crates/tuas_boussinesq_solver/examples/` | example, GUI |
| `ciet_v2_opcua_client` | `crates/outram-park-digital-twin-engine/src/bin/` | bin, client only |
| `fhr_sim_v2` | `crates/outram-park-digital-twin-engine/examples/` | example, GUI |
| `fhr_sim_v2` (duplicate) | `crates/tampines/examples/` | example |
| `fhr_sim_v1` | `crates/tampines-steam-tables/examples/` | example |
| `fhr_sim_v1` | `crates/teh-o-prke/examples/` | example |
| `htgr_sim_v1` | `crates/outram-park-digital-twin-engine/examples/` | example, GUI |
| `widget_studio` | `crates/outram-park-digital-twin-engine/examples/` | widget gallery, not a sim |
| `triso_simulator`, `boon_lay_decay_simulator`, `first_passage_realtime` | `crates/boon-lay/examples/` | fuel/source-term, GUI |

**Four near-clone FHR simulators exist across four crates.** `fhr.md:19-32`
records three; the fourth (`crates/tampines/examples/fhr_sim_v2`) is a second
copy of the canonical one. This is a maintenance liability and a real-time
liability both — see §5.4.

### 1.2 Status table

"Fidelity levels that exist" means *levels that execute*, not levels that are
scoped. A `todo!()` on the coupling path means the level does not exist,
however much code sits either side of it.

| Reactor | Scoped as | What actually executes today | Fidelity levels that exist | Real-time today? |
|---|---|---|---|---|
| **CIET** (facility, not in the slate) | Not scoped as a reactor | **Full plant, GUI + headless + OPC-UA.** Coupled DRACS + primary natural circulation on `tuas` CIET components; user-settable Courant-clamped timestep; wall-clock-paced physics thread at `crates/outram-park-digital-twin-engine/src/bin/ciet_educational_simulator_v2/ciet_simulator_v2/app/panels_and_pages/full_simulation/mod.rs:546` | **1 — lumped/system TH.** Plus a runtime mesh-fidelity switch (`HeaterType`, 8 vs 15 axial nodes, `src/ciet_opcua/state.rs:68`) | **Yes, measured** [M] |
| **FHR** (gFHR / Mk1 PB-FHR) | Three-loop plant, PRKE + TH + Rankine secondary | `fhr_sim_v2` runs: 3 wall-paced threads (PRKE 1 ms, TH 0.1 s, plots). Four-branch primary + two-branch intermediate solved by root-finding; 15-cell compressible SG tube | **2 — point kinetics (`nordheim_fuchs` + 5-group delayed) and lumped TH.** No spatial neutronics | **Yes by construction; three live `todo!()` panic paths in the flow solver** (`examples/fhr_sim_v2/app/thermal_hydraulics_backend/.../parallel_branch_flow_calculator.rs:411,454,470`) |
| **HTGR (prismatic)** — the thing filed under HTR-10 | Pebble-bed HTR-10 | `htgr_sim_v1` runs: prismatic core, CoolProp helium EOS, IF97 Rankine secondary, 12/12 tests pass. **Open-loop pacing** — fixed 10 ms sleep, no drift correction (`examples/htgr_sim_v1/app/mod.rs:42-49,100`) | **2 — prompt excursion + delayed layer, lumped TH** | **Approximately, but uncompensated** — drifts by the compute cost each tick |
| **HTR-10** (pebble bed) | Pebble bed, helical-coil SG, cavity cooling | **Nothing.** Core model is a rewrite. Packed-bed friction is `todo!()`; bed conductivity is zero code; no graphite properties anywhere | **0** | No |
| **MSRE** | Circulating fuel, salt-to-air radiator | `outram-park-fork-moltres` runs standalone: 20 tests pass, coupled circulating-fuel eigenvalue + precursor drift + Picard salt-thermal coupling. **Zero dependents** — wired into nothing | **1 — spatial (1-D ring) steady eigenvalue only.** No coupled flux transient; no reduced circulating-fuel PRKE | **No — 1.14 s per solve** [M] |
| **BWR** (natural circulation) | Direct cycle, chimney, separator, void feedback | **Closures only.** `outram-foam-multiphase` 67 tests of real drift-flux / RPI wall boiling / CHF; `tampines/src/multiphase_1d/drift_flux.rs` 1-D marcher. **No heated channel, no separator, no loop closure, no void reactivity in any kinetics path** | **0 as an integrated plant** | No |
| **iPWR** | Integral PWR, helical-coil once-through SG, pressuriser | **Nothing assembled.** IF97 is the workspace's strongest asset (937 tests, tested at 100–220 bar); TUAS has no water arm and is pressure-blind; no pressuriser code exists anywhere | **0** | No |
| **EBR-II** | Pool SFR, intermediate Na loop, steam plant | **Nothing.** Sodium/NaK properties exist in CoolProp and are tested, but `LiquidMaterial` has no sodium arm so the TH stack cannot reach them. No pool control volume exists in the workspace. None of the four SFR expansion feedbacks is available as a reactivity coefficient | **0** | No |

### 1.3 The `bedok` correction — it does not run at all

`docs/reactor-scoping/bwr.md:64` lists, under **HAVE**, "3-D two-group nodal
diffusion coupled to channel TH". **That is not true today, and the gap is
larger than "the benchmark gates are ignored".**

The three pieces exist separately and are never joined:

- The nodal solver is real — `crates/bedok/src/reference/nodal/` (~3,900 lines,
  `sanm_solver.rs`, `finite_difference_solver.rs`) with analytic-limit unit
  tests. Its own doc concedes it is "unverified against the benchmarks"
  (`crates/bedok/src/reference/nodal/mod.rs:92-98`).
- The channel TH is real — `crates/bedok/src/reference/th/`, 4,678 lines.
- The coupling driver is real — `coupling/steady.rs`, `transient.rs`,
  `critical_boron.rs`.

**But every call the driver makes into neutronics and TH goes through
`crates/bedok/src/reference/coupling/seam.rs`, whose bodies are `todo!()`** —
`:782`, `:804`, `:826`, `:849`, `:890`, `:925`, `:955`, `:984`, `:1011`, and
`:471`. Call sites at `steady.rs:239,270,309,355` and
`transient.rs:290-294,299,473-481,645`. **Nothing outside `reference::nodal`
ever imports `reference::nodal`.** A coupled solve panics on its first call.

Further, `crates/bedok/tests/support/mod.rs:573-576`'s
`solve_iaea3d_reference()` returns `None`, and the benchmark gates
**skip-and-pass** when un-ignored (`support/mod.rs:570-571`) — so a green suite
there would mean nothing. All 15 real `#[ignore]` attributes are accounted for:
7 IAEA-3D gates in `tests/benchmark/main.rs`, 8 parity gates in
`tests/parity/main.rs`.

There is **no NEACRP result of any kind**. The BWR case (D1, 17x17x14
cold-water injection) and the PWR case (A2) *builders* are complete and tested
— 9 and 11 tests respectively — and the CSV data is committed and `include_str!`-embedded
(`crates/bedok/src/reference/cases/data/`). But **no benchmark test exists for
either case**; `tests/benchmark/main.rs` covers only IAEA-3D.

The only timing number in the crate is not Rust: `tests/fixtures/iaea3d/PROVENANCE.md`
records 76.5 s for IAEA-3D on a 17x17x19 grid under **GNU Octave**, running
Yan Ren's original MATLAB.

**Consequence for this document: the nodal-diffusion fidelity band cannot be
costed, because it has never executed.** Any real-time plan that routes through
`bedok` must first close the seam.

---

## 2. The fidelity ladder

Seven bands. For each: who owns it, what its native timestep is set by, what a
step costs, and whether it can plausibly be paced against a wall clock.

### Band 0 — Point kinetics · `teh-o-prke`

**What executes.** Two distinct solvers, and the distinction matters enormously
for real time:

- `NordheimFuchsExactTimestepper` (`crates/teh-o-prke/src/nordheim_fuchs.rs:145`)
  — a **closed-form analytic** integrator for a prompt excursion with adiabatic
  fuel feedback. Its module doc states the design intent outright
  (`nordheim_fuchs.rs:6-17`): it exists because "a standard ODE stepper's
  `dt << Lambda` stability restriction (Lambda can be ~1e-8 s for fast-spectrum
  systems) is prohibitively expensive for real-time frame rates". **It carries
  no stability restriction relating `dt` to the generation time at all.**
- `SixGroupPRKE` with both an explicit and an **implicit** stepper
  (`crates/teh-o-prke/src/zero_power_prke/six_group_precursor_prke/implicit_solver.rs:18`).
  The implicit path assembles a **7x7** matrix (six precursor groups plus the
  neutron population, `implicit_solver.rs:145,210`) and solves it by dense LU
  with the crate's own inlined `SquareMatrix` (`crates/teh-o-prke/src/matrix.rs:63`,
  `solve` at `:210`). The explicit path wraps the ported **RKF45**
  (`.../explicit_solver.rs:14`, `src/time_stepping/openfoam_rfk45.rs:71`).
- `DelayedNeutronLayer` (`crates/teh-o-prke/src/delayed_neutron_layer.rs:205`,
  `advance()` at `:393`) — **five** groups, backward-Euler in closed form,
  `C_i^{n+1} = (C_i^n + dt*(beta_i/Lambda)*P) / (1 + dt*lambda_i)`. Five
  multiply-adds, **no history retained**. This is the layer both engine examples
  actually run.

**Cost — the one real microbenchmark in the workspace.** [C]
`crates/teh-o-prke/src/delayed_neutron_layer.rs:549,554-601`
(`precursor_update_is_o1_per_step`) records **~0.7–1.0 ns per step**, flat
across a 100k-step early block versus a late block after 20x more steps. A 7x7
dense LU for the full PRKE path sits between the n=5 (193 ns) and n=10 (352 ns)
rows of the committed matrix benchmark
(`crates/outram-foam-basic-lib/README.md`, measured 2026-06-24) — call it
**~250 ns per step** [E]. `NordheimFuchs` has no matrix at all.

**Real-time verdict: yes, with three-to-four orders of magnitude of headroom.**
This band is free. `fhr_sim_v2` runs it at a 1 ms timestep and sleeps out the
remainder (`examples/fhr_sim_v2/app/prke_backend/mod.rs:96,203`), and the
comment at `:59-64` states plainly that the 1 ms step — up from an earlier
25 µs — is possible *because* Nordheim-Fuchs carries no `dt << Lambda`
restriction.

**The `O(1)` property was won, not given.** The same file records at `:88` that
the layer's predecessor — built on `chem-eng`'s transfer functions — measured
**~49 µs/step at step 1,000 rising to ~1.8 ms/step at step 40,000**, unbounded
`O(n)` growth. This is the single most important real-time lesson in the
workspace, and §2 Band 1b explains why it is not yet fully learned.

**Caveats that are not performance caveats.** `crates/teh-o-prke/verification_and_validation/`
contains only a README — there is no V&V case for the kinetics crate. Of 21
tests, **only one (`zero_power_prke/tests.rs:57`) exercises the implicit solver
and none exercises the RKF45 path.** The six-group constants return the **same
decay-constant array for U-233, U-235 and Pu-239** and say so in their own
comment (`.../six_group_constants.rs:24`). `DecayHeat` (7 precursor groups,
`decay_heat.rs:16`) carries the author's own comment at `:11` — *"i think this
is slightly buggy … the precursors are energy units, not power"* — and **no test
covers it at all**.

### Band 1 — Lumped / system thermal-hydraulics · `tuas_boussinesq_solver`, `tampines`, `chem-eng-…`

**What executes.** `tuas` is the workspace's only experimentally-validated
physics: the CIET component library, validated against facility data and an
independent code across 25 coupled cases with maximum absolute error 6.80%
(DRACS) / 5.60% (primary), recorded with methodology and results in
`crates/tuas_boussinesq_solver/verification_and_validation/` [C]. `chem-eng`
supplies PID and transfer-function blocks built for real-time loops.

**What does not.** `crates/tampines/src/lib.rs:35-38` says **"Scaffold only."**
— 3,335 lines with **15 `NotYetImplemented` sites**. `SteamGenerator::step`,
`Turbine::expand_to`, `Condenser::condense`, `HeatExchanger::calculate`, pump,
valve and cooling tower all return not-implemented; only `Pipe::step` is real.
Five of its 26 files (`single_phase/`, `compressible/`, `heat_transfer/`,
`hem/`, `critical_flow/`) are **pure `pub use` re-export files** and say so. The
engine widgets that wrap the stubs therefore render as flat rectangles by design
(`crates/outram-park-digital-twin-engine/src/components/condenser.rs:34-38`).

**The one exception is worth flagging as a risk, not an asset.** `DriftFlux1d`
(`crates/tampines/src/multiphase_1d/drift_flux.rs:232`, `step()` at `:569`) is a
genuine, unusually well-documented 1,081-line semi-implicit four-equation
drift-flux solver with a Thomas tridiagonal pressure solve and secant
re-linearisation across the saturation line (where compressibility jumps by a
measured factor of **1.32e3**, `:625-635`). **No test in the crate ever calls
`step()`** — the four tests cover `thomas_solve`, the compressibility jump and
pipe geometry only. `bwr.md:63` names this module "the closest thing to a BWR
channel solver in the repo"; it is also the largest untested solver in the
workspace.

**Native timestep.** Set by advection Courant, not acoustics. CIET v2 clamps to
**0.1 s** (`full_simulation/mod.rs:154`; `MAX_TIMESTEP_SECONDS` at
`src/ciet_opcua/state.rs:499`), user-settable down to 0.001 s.

**Cost.** **[M]** — CIET v2 headless, 95 s run. The model carries **47 distinct
named pipe/component objects** across the coupled DRACS and primary loops plus a
shell-and-tube heat exchanger, each an axially-discretised fluid array with
solid shell and insulation layers laterally coupled to it. Measured: **median
27 ms, p90 45 ms, min 5 ms per 0.1 s step**, with occasional outliers to
1276 ms. See §3.1.

Corroborating committed figures [C], all prose in test doc comments rather than
machine-checked assertions:

| Where | Claim | Implied factor |
|---|---|---|
| `crates/tuas_boussinesq_solver/.../parasitic_heat_loss_regression_tests/coupled_dracs_loop_ver_1_uncalibrated/mod.rs:61,76` | 3000 s simulated in **118–210 s** wall (i5-13500H / i7-10875H) | **14x–25x** |
| `.../ciet_three_branch_plus_dracs/ciet_educational_simulator_loop_prototypes/version_3/mod.rs:154` | 400 s simulated in ~30 s at dt = 0.2 s | ~13x ("about 10 times") |
| `version_3/mod.rs:267-276` | 5x single-thread, ~9x multithreaded; ~2x at dt = 0.04 s | 5x–9x |

**Read the 14x–25x figure carefully.** It is a **live, non-ignored** test
(`regression_long_test_uncalibrated_dracs_loop_set_c`, `:68`) but it runs **nine
CIET data points concurrently on 8 cores / 16 threads**, so it measures
*throughput*, not the single-case latency a real-time loop cares about. The
per-case single-thread figure — 5x — is the one to plan against, and it is
consistent with the 27% duty cycle measured in §3.1.

**Real-time verdict: yes.** This is the only band where that statement is backed
by a measurement rather than an argument.

**One structural inefficiency worth naming.**
`crates/tuas_boussinesq_solver/src/lib/array_control_vol_and_fluid_component_collections/standalone_fluid_nodes/mod.rs:36`
(`solve_conductance_matrix_power_vector`) copies the conductance matrix into a
**dense** `SquareMatrix` and runs full pivoted LU **every timestep, per node
array** — even though the assembled matrix is **tridiagonal**
(`conductance_array_functions.rs:311-331` only ever sets `[i,i-1]`, `[i,i]`,
`[i,i+1]`). An `O(n)` Thomas solve already exists in the workspace
(`crates/tampines/src/multiphase_1d/mod.rs:175`, tested). Replacing `O(n^3)`
with `O(n)` here is the cheapest headroom available in Band 1, and it matters
most exactly where headroom is scarcest — Android/Termux.

### Band 1b — Process control · `chem-eng-real-time-process-control-simulator`

Broken out because **it is the one band whose per-step cost is not bounded**,
and because it is a real dependency of both `teh-o-prke` and `tuas`.

**What executes.** The working PID lives in the `alpha_nightly` tree —
`AnalogController::new_pi_controller` / `new_filtered_pid_controller` /
`new_filtered_pd_controller` (`src/lib/alpha_nightly/controllers/mod.rs:28,38,52`).
That is what TUAS actually imports
(`.../coupled_dracs_loop_tests/dataset_a.rs:910`).

**What does not.** 8,850 lines carrying **46 `todo!()`**, all in the transfer-
function enum dispatch: every `Unstable` and `ConstantValueUndamped` arm
(`beta_testing/transfer_fn_wrapper_and_enums/generic_first_order.rs:82,83,114,115,127,128`),
`impl Default` at `:66`, and construction-time
`todo!("unstable system, not implemented")` at `:191,194`. **Only the `Stable`
path exists.** `src/lib/beta_testing/controllers/mod.rs` is **0 bytes**;
`beta_testing/prelude.rs` is entirely commented out; `src/lib/stable/mod.rs` is
one line. The crate has **4 tests total**, two of which only check a `uom` unit.

**The real-time defect.** `src/lib/beta_testing/stable_transfer_functions/first_order_transfer_fn.rs`
represents a transfer function as a **growing `Vec` of superposed step
responses**: `:16` declares it, `:113` pushes one per input change, `:134` sums
the whole vector every evaluation, and `clear_first_order_response_vector`
(`:147-230`) prunes with a repeated `.position()` + `.remove(index)` loop that
is `O(n^2)`-shaped.

This is **the same mechanism** `teh-o-prke` measured and discarded: 49 µs/step
at step 1,000 → 1.8 ms/step at step 40,000 [C]
(`crates/teh-o-prke/src/delayed_neutron_layer.rs:88`). At a 1 ms kinetics
timestep, 40,000 steps is **40 seconds of simulated time** — well inside a
demo — and 1.8 ms/step is already **1.8x over a 1 ms budget**, still climbing.

**Real-time verdict: not real-time as written.** A controller must be `O(1)` per
step, which for a first-order lag means carrying **one state variable**, not a
history of every input it has ever seen. The correct form is the same recurrence
the delayed-neutron layer now uses. Until that lands, any long-running
control-in-the-loop demo degrades continuously, and it degrades *slowly enough
that a short test will not catch it*. That is the worst failure mode a real-time
system can have.

### Band 2 — 1-D compressible / HEM steam · `tampines-steam-tables` `TampinesSteamArray`

Called out separately from Band 1 because **its native timestep is acoustic,
not advective**, and that changes everything.

`crates/outram-park-digital-twin-engine/examples/fhr_sim_v2/app/thermal_hydraulics_backend/secondary_loop/mod.rs:37-38`
records it precisely: `dx/c = (2/15)/1450 = 9e-5 s`, and the constant is set to
`SG_TUBE_DT_S = 5.0e-5` s. That is **2000 sub-steps per 0.1 s outer TH step**
to be time-accurate.

The workspace does not pay that. `SG_TUBE_SUBSTEPS_PER_TH_STEP = 25`
(`secondary_loop/mod.rs:46`) — so the tube advances **1.25 ms of its own time
per 100 ms of host time, a factor of 80 behind the clock**. This is documented
and deliberate: `crates/tampines/docs/steam_generator_tube_integration.md:30-34`
calls it "a **quasi-steady** sub-model … not re-converged each call … nudged a
little each TH step".

**Real-time verdict: not time-accurate, and honestly labelled as such.** It is
a relaxation model wearing a transient model's clothes. That is a legitimate
engineering choice for a steam generator that physically takes seconds to heat
up; it is **not** legitimate for anything where the acoustic timescale is the
phenomenon (steam-line break, water hammer, main-steam-isolation transients) —
which is exactly what a BWR turbine-trip or an iPWR steam-line-break demo would
need.

### Band 3 — Nodal diffusion + channel TH · `bedok`

**Cannot be costed. Does not execute.** See §1.3.

For reference only, the original MATLAB solves IAEA-3D (17x17x19) in **76.5 s
under Octave** [C]. If the Rust port achieved a 10x speedup over Octave — which
is plausible but has not been demonstrated — a steady statepoint would still be
~8 s, i.e. **80x too slow for a 0.1 s TH step**. Transients reuse the factorised
operators and are cheaper per step, but there is no measurement to build on.

**Real-time verdict: unknown and not demonstrable.** Treat any real-time claim
for this band as unfounded until the seam is closed and a step is timed.

### Band 4 — CFD · `outram-foam-basic-lib`, `-appbuilder-lib`, `-turbulence-lib`, `-multiphase`

**What executes, and well.** A real PISO/PIMPLE loop with a solved, benchmarked
case: `crates/outram-foam-appbuilder-lib/tutorials/pimple_foam_cavity.rs` matches
icoFoam to ~1.4% on the velocity field and Ghia 1982 to max |err| 0.063 / RMS
0.036 [C]. Sod shock tube, Stefan and gallium melting, and a GeN-Foam
neutronics slab all carry measured, dated results.

**Cost.** **[C]**, from `crates/outram-foam-basic-lib/README.md`: the
`pimple_foam_cavity` Ghia Re=100 case on the **41x41 (1681-cell)** mesh, 6000
steps to t = 12 s, runs in **12.4 s wall** with DIC-PCG and warm start (42.8 s
with GAMG, which loses below the ~10^4-cell crossover).

Two derived figures [E]:

- **~2.07 ms per step**, i.e. **~1.23 µs per cell per step** for 2-D laminar
  incompressible flow.
- **Real-time factor ~0.97x** — a 1681-cell laminar cavity is *just* real time.

**Real-time verdict: no, above about 10^4 cells.** Scaling the measured
per-cell cost, a 10^5-cell mesh costs ~0.12 s per step and a 10^6-cell mesh
~1.2 s per step [E]. A reactor CFD case is 10^6 to 10^7 cells with turbulence
closure, energy and phase change on top — three to four orders of magnitude
beyond real time on this hardware. **CFD is an offline reference generator for
this workspace, permanently.**

Additional blockers independent of speed: the OpenFOAM I/O layer is stubbed —
`crates/outram-foam-appbuilder-lib/src/io/control_dict/mod.rs:82`,
`io/fv_schemes/mod.rs:97`, `io/fv_solution/mod.rs:90` all `todo!()` on read, and
`io/output/mod.rs:49,61,74` `todo!()` on every write. **Cases are built in Rust
code and results are never written out.** The turbulence crate has 16 tests
across 5 models and no benchmark validation
(`crates/outram-foam-turbulence-lib/src/lib.rs:41-45`), which is why the
NACA0012 aerofoil tutorial's three tests are ignored.

### Band 4b — Circulating-fuel spatial neutronics · `outram-park-fork-moltres`

Broken out because the MSRE case depends on it and it has a clean measurement.

**What executes.** 3,070 lines, 20 tests, zero `todo!()`. Multigroup diffusion
(`diffusion.rs:142`), precursor advection-diffusion-decay (`precursors.rs:74`),
coupled circulating-fuel eigenvalue (`circulating.rs:95`), Picard
neutronics-thermal coupling (`thermal.rs:239`), periodic ring mesh
(`ring_mesh.rs:61`).

**Cost.** **[M]** — reproduced this audit:

```
cargo test --release -p outram-park-fork-moltres --lib \
  circulating::tests::flow_reduces_reactivity_monotonically -- --exact
```

7.08 s wall for a **6-velocity sweep** on a **300-cell, single-group ring**
(`circulating.rs:334,435`), including ~0.25 s of cargo/harness startup →
**~1.14 s per coupled circulating-fuel eigenvalue solve**. This independently
reproduces the ~1.2 s figure recorded in `docs/reactor-scoping/msre.md:126`,
which was not otherwise verifiable (the crate contains **zero** `Instant::now`
call sites).

A Picard-coupled thermal statepoint costs more: `temperature_feedback_reduces_k_with_power`
took 18.2 s for two power points at 8 and 9 Picard iterations → **~9 s per
coupled statepoint** [M], consistent with ~1 s per inner eigenvalue solve.

**Real-time verdict: no.** Against the 0.1 s TH budget it is **11x too slow**;
against the 1 ms PRKE budget, **1140x too slow**. `msre.md`'s "three orders of
magnitude" framing is right for the PRKE thread and overstated for the TH
thread — the honest statement is *one order of magnitude too slow for the TH
loop, three for the kinetics loop*.

### Band 5 — Monte Carlo transport · `outram-mc-libs` + `njoy-outram-park-fork`

**What executes.** Genuinely working, with the best evidence trail in the
repository: CSG geometry with nested universes and lattices, k-eigenvalue with
five backends, Woodcock delta tracking for doubly heterogeneous media, CRAM
depletion, and 21 OpenMC-notebook port tests. Wired to `njoy` for real — not
stubbed — at `crates/outram-mc-libs/src/material/nuclide.rs:23-31,254`.

**Cost.** **[C]**, `crates/outram-mc-libs/examples/godiva_gpu_benchmark.rs:83-89`,
re-measured 2026-08-06: Godiva bare sphere, **1,399,230 histories/s**, 0.572 s
wall, k_eff 1.01107 ± 0.00170. The older sweep in
`verification_and_validation/gpu_batched_transport/README.md` gives 0.843 s for
10^6 histories on `CpuMultiThread`, consistent at ~1.2e6 histories/s.

Note the GPU path **does not help transport**: the GPU cross-section kernel is
not wired into the collision sweep, and the committed sweep shows the batched
GPU backend *losing* to `CpuMultiThread` at every batch size.

**Real-time verdict: no, by two to four orders of magnitude.** [E] A whole-core
statepoint needing 10^8 histories costs ~71 s; 10^9 costs ~12 minutes. Godiva
is a bare sphere with three nuclides at LOW-tier embedded data; a real core with
continuous-energy data and depleted compositions is slower per history, not
faster.

**MC's role in a real-time architecture is offline generation of the data the
fast models consume** — group constants, reactivity coefficients, kinetics
parameters, form factors. That is exactly the role `docs/architecture.md`
already assigns it, and the measurement confirms it is the only viable one.

### Band 6 — Fuel performance · `outram-park-fork-offbeat`

**What executes.** More than the Members table suggests: 59,772 lines, 545
tests, **zero `todo!()`/`unimplemented!()`**. A real segregated small-strain FV
mechanics solver (`src/mechanics/solver.rs:350`, `solve_creep_step:775`), ~16k
lines of code_aster rheology ports verified against code_aster's own `astest`
decks (`tests/astest_ssnv101a.rs`, `tests/astest_ssnv126a.rs`), gap conductance,
FGR, burnup and corrosion.

**What does not.** **There is no heat-conduction / temperature-field solver.**
The crate computes mechanics *given* a temperature field and does not produce
one, so the "strongly coupled loop" its own `src/lib.rs:38-45` describes is not
closed inside the crate, and there is no fuel-rod driver that runs an
irradiation history. Its own doc (`src/lib.rs:70-72`) says "Scaffold / early.
This crate has had no human verification or validation."

**Real-time verdict: no, and the question is premature.** Fuel performance is
inherently a **burnup-timescale** model — its natural time coordinate is days
to years, not seconds. It has no business inside a real-time loop. Its correct
coupling to a real-time twin is as a **precomputed history**: run it offline
over an irradiation history, tabulate gap conductance, fuel conductivity
degradation and stored energy against burnup, and let the twin interpolate.
That is a table lookup, and it is free.

---

## 3. What "real-time" actually requires

### 3.1 The measurement that anchors everything

**[M], 2026-08-07.** Built and ran the real binary:

```bash
cargo build --release -p outram-park-digital-twin-engine \
  --bin ciet_educational_simulator_v2
./target/release/ciet_educational_simulator_v2 \
  --headless --bind 127.0.0.1 --no-advertise --port 48401
```

95 s of run, 48 status samples, default 0.1 s timestep, 15-node fine heater
mesh:

| Quantity | Value |
|---|---|
| Simulated time at cut-off | 94.7 s |
| Wall-clock at cut-off | 94.7 s |
| **Real-time factor** | **1.00x** (paced; it sleeps out the surplus) |
| Compute per timestep, median | **27 ms** |
| Compute per timestep, p90 | 45 ms |
| Compute per timestep, min | 5 ms |
| Compute per timestep, max | 1276 ms |
| **Duty cycle at the median** | **27%** → ~3.7x headroom |
| **Duty cycle at p90** | 45% → ~2.2x headroom |

**Two honest caveats, both material:**

1. **This is the idle state.** Heater power 0 kW, CTAH pump 0 Pa, FM-40
   0.0000 kg/s. The branch flow solver is root-finding against a near-zero
   driving head, which converges trivially. **A heated, flowing case will cost
   more than 27 ms per step — this figure is a lower bound, not a typical
   value.** No OPC-UA client was available in this environment to command power
   over the wire, so the loaded number was not obtained. This is the single
   most valuable missing measurement in this document.
2. **Two outliers exceeded the timestep** — 1276 ms and 704 ms, i.e. a 12.8x
   and 7x overrun of the 100 ms budget. Simulated time fell behind wall-clock
   by 1.2 s and 0.6 s respectively and **recovered within two samples**, because
   the pacing is deadline-compensated. That recovery behaviour is the property
   that makes this loop usable; see §3.4.

### 3.2 Wall-clock budget per simulated second

The budget is set by the operator, not by the physics. The targets below are
**engineering judgement, not values taken from a cited standard** — they are
stated so the arithmetic that follows has something to be checked against, and
should be replaced with ANSI/ANS-3.5 figures if and when that anchor is settled
(see §7 item 7).

| Requirement | Target | Source of the number |
|---|---|---|
| Sustained real-time factor | **>= 1.0x**, averaged over minutes | Definition of Type I (`docs/type-i-digital-twin-scoping.md:32-36`) |
| Control-action to visible response | **<= 200 ms** | Human perception of causality; standard HMI practice |
| Display refresh | **>= 20 Hz** | Already what the engine does — `PLOT_TICK = 50 ms` (`examples/htgr_sim_v1/app/mod.rs:49`), CIET v1 clone-rate ~20 Hz |
| Physics step deadline | **<= the timestep**, with headroom for the tail | 0.1 s in CIET v2 |
| Worst-case overrun before it is visible | **~2 timesteps** | Below this the pacing absorbs it |

Applying those to the measured numbers: the **200 ms control-response target is
what actually binds**, not the 1.0x factor. With a 0.1 s timestep, a control
write is drained at the top of the next step
(`crates/outram-park-digital-twin-engine/src/ciet_opcua/user_controls.rs`) and
its effect published at the end of that step — so worst-case control latency is
**one timestep of queueing plus one of compute**, i.e. 100 ms + 27 ms typical,
100 + 1276 ms in the tail. **The median is comfortably inside budget; the tail
is not.**

### 3.3 What timestep each physics demands

This is where the multi-rate structure comes from. Each band's timestep is set
by a different physical timescale, and they span **nine orders of magnitude**:

| Physics | Timescale that sets `dt` | Required `dt` | Ratio to a 0.1 s host step |
|---|---|---|---|
| Prompt neutron kinetics (explicit) | Prompt generation time `Lambda` | 1e-8 s (fast) to 1e-4 s (thermal) | 10^3 to 10^7 sub-steps |
| Prompt neutron kinetics (**Nordheim-Fuchs, closed form**) | **none** | any | **1** |
| Delayed precursors (implicit, 7x7) | Longest precursor half-life ~55 s; **implicit removes the stiffness restriction** | 1e-3 s used in practice | 100 |
| Compressible / acoustic (`TampinesSteamArray`) | `dx/c`; measured `9e-5 s` at `dx = 0.133 m`, `c = 1450 m/s` | 5e-5 s | **2000** |
| Advective TH (`tuas` fluid arrays) | Courant on flow velocity | 0.1 s (clamped) | 1 |
| Fuel-pin conduction | Fuel thermal diffusion time, seconds | 0.01–0.1 s | 1–10 |
| Xenon / iodine | Hours | 1–60 s | can be super-stepped |
| Fuel performance / burnup | Days to years | offline | not in the loop |

**The stiffness problem, stated precisely.** Point kinetics is the classic stiff
system: with `Lambda = 1e-4 s` and a longest precursor decay constant of
`~0.0124 s^-1`, the eigenvalue spread is **~10^6**, and for a fast-spectrum
core with `Lambda = 1e-7 s` it is **~10^9**. An explicit stepper is limited by
the fastest mode; a real-time loop cannot afford 10^6 sub-steps per displayed
second.

**This workspace has two of the three answers and is missing the third:**

- ✅ **Analytic closure.** `NordheimFuchsExactTimestepper` sidesteps the
  restriction entirely for the prompt-excursion regime.
- ✅ **Implicit (backward Euler) on the 7x7 PRKE system.** Unconditionally
  stable, ~250 ns per step [E].
- ❌ **A stiff adaptive integrator.** `crates/teh-o-prke/src/time_stepping/`
  ports **only RKF45** (`openfoam_rfk45.rs:71`) — an *explicit* adaptive method
  that will step itself down to `Lambda` on a stiff problem and stall. The
  OpenFOAM `Rosenbrock12.C`, `Rosenbrock34.C`, `rodas23.C` and `rodas34.C`
  sources **are vendored** in
  `crates/teh-o-prke/src/time_stepping/openfoam_source_files/` **but none is
  ported to Rust.** Porting one Rosenbrock is the cheapest robustness win in
  this whole document — the reference implementation is already on disk. Note
  also that **no test exercises the RKF45 path**, so its behaviour on a stiff
  problem is not merely unsuitable, it is unobserved.

**Stiffness elsewhere in the stack is handled correctly**, and worth recording
so it is not re-litigated:

- TUAS conduction is implicit —
  `crates/tuas_boussinesq_solver/src/lib/array_control_vol_and_fluid_component_collections/conductance_array_functions.rs:228`,
  with the doc at `:298-306` noting larger Fourier numbers are allowed but that
  lagged conductances keep `Fo` at 0.25–1.0 in practice.
- `TampinesSteamArray` is pressure-implicit (PIMPLE/PISO/SIMPLE) explicitly to
  escape the acoustic CFL
  (`crates/tampines-steam-tables/src/openfoam_algorithms/rhoPimpleFoam/mod.rs:75-88`)
  — though §2 Band 2 shows that in the FHR simulator it is nonetheless
  sub-cycled far short of time accuracy.
- `DriftFlux1d` is semi-implicit with SIMPLE outer correctors and re-linearises
  compressibility across saturation (`crates/tampines/src/multiphase_1d/drift_flux.rs:620-660`).

### 3.4 The pacing mechanism, and two defects in it

CIET v2's loop (`full_simulation/mod.rs:546` … `:2040`) is a **deadline-compensated**
pacer, which is the right design:

1. Stamp loop start (`:555`).
2. Integrate one timestep.
3. Measure elapsed compute (`:1980-1983`).
4. Sleep `timestep - compute` (`:1985-1991`, `:2016`).
5. If behind, **do not sleep** and reset the timestep to the maximum (`:2033-2038`).

That last branch is what recovered the 1276 ms outlier. `htgr_sim_v1` does
**not** do this — `examples/htgr_sim_v1/app/mod.rs:100` sleeps a fixed 10 ms
regardless of compute cost, so it drifts slower than its nominal rate by exactly
the compute time, silently.

Two defects found in the CIET pacer while reading it:

- **Sign collapse.** `full_simulation/mod.rs:1985-1988` computes
  `(timestep_ms - calc_ms).round().abs()`. The `.abs()` turns an *overrun* into
  a *positive sleep budget*. A step that took 150 ms against a 100 ms timestep
  yields `time_to_sleep = 50 ms` and `real_time_in_current_timestep = true` —
  so if the run is still ahead cumulatively, it **sleeps 49 ms after already
  overrunning**. The cumulative guard usually masks it, but it is the wrong
  arithmetic and it makes the overrun path harder to reason about.
- **Unconditional underflow.** `:1990` computes
  `Duration::from_millis(time_to_sleep_milliseconds - 1)` *before* the
  `> 1` guard at `:1992`. When the value is 0 this underflows `u64`. In release
  it wraps (and the result is then unused, so it is currently harmless); in a
  debug build it panics. The workspace mandates release builds, so this is
  latent rather than live — but it should not be latent.

The same two patterns appear in `nat_circ_simulation/mod.rs:952-960`, so the
fix is in two places.

### 3.5 Which bands are inherently not real-time

Stated as a design rule, not a to-do:

| Band | Real-time? | If not, what it must become |
|---|---|---|
| Point kinetics | **Yes** | — |
| Lumped/system TH | **Yes** | — |
| Control / PID | **Not as written** | Rewrite the first/second-order transfer functions as `O(1)` recurrences (§2 Band 1b) |
| 1-D acoustic (`TampinesSteamArray`) | **No, at 2000 sub-steps** | Quasi-steady relaxation sub-model (what it already is), or a lumped inventory model |
| Nodal diffusion | **Unknown; probably not at full mesh** | Precomputed reactivity coefficients + axial/radial form factors; or a coarse-mesh transient with the operators factorised once |
| Circulating-fuel spatial | **No (11x–1140x)** | Reduced circulating-fuel PRKE with `beta_eff` and loop-transit reactivity loss taken from the spatial solver offline |
| CFD | **No (10^3–10^4x)** | Offline reference; closure calibration; occasionally a POD/ROM if one is ever built |
| Monte Carlo | **No (10^2–10^4x)** | Offline data generation only — group constants, coefficients, kinetics parameters |
| Fuel performance | **No (wrong timescale entirely)** | Precomputed tables against burnup |

### 3.6 Where the existing egui engine fits

`crates/outram-park-digital-twin-engine` is **already the real-time
infrastructure layer**, and it is better than its scoping documents suggest. It
builds clean (`cargo check --all-targets` exits 0), ships 5 runnable targets,
and provides:

- `SharedState<T>(Arc<RwLock<T>>)` — `src/app_scaffold/mod.rs:37` — deliberately
  `RwLock` over `Mutex` per the workspace rule, with `snapshot()`/`update()`
  designed so the lock is held only for a clone.
- `spawn_physics_thread_monitored` / `spawn_monitored` —
  `src/app_scaffold/crash.rs:198,164` — a **panic-caught physics thread with a
  GUI restart modal**. This matters more than it sounds: the IF97 façade panics
  on out-of-range input at ~40 sites, and a transient overshoot kills the
  physics thread. The crash modal is what turns that from "the simulator froze"
  into "the simulator told you and offered to restart".
- An OPC-UA server (36 nodes) with a **separate control-request struct drained
  into plant state**, which fixes a lost-update race — the architecture the
  maintainer specified, recorded in
  `docs/human-in-the-loop-ciet-v2-case-study.md:66-79`.
- Real widgets where the physics exists (`steam_generator.rs` 1938 lines,
  `pump.rs` 1673, `fhr_reactor_vessel.rs` 1363, `htr10_reactor_vessel.rs` 1390)
  and honest one-rectangle stubs where it does not (`condenser.rs` 41 lines).

**What it is missing for Type I** is exactly what
`docs/type-i-digital-twin-scoping.md:118-121` already identified, and the
priority ordering there is right: **snapshot / restore / backtrack does not
exist**, and it is cheap now and invasive later. Every instructor-station
feature — freeze, save, restore, replay, malfunction rewind — depends on it, and
so does the **fidelity-switching architecture in §4**, because switching
fidelity is a state-serialisation problem in disguise.

---

## 4. Multi-fidelity coupling architecture

### 4.1 The workspace already does multi-rate coupling — that is the foundation

Before designing anything, note what `fhr_sim_v2` already does
(`examples/fhr_sim_v2/main.rs:364,370,378`): **three wall-clock-paced threads at
three different rates**, sharing one state object.

| Thread | Rate | Physics |
|---|---|---|
| `fhr-prke` | **1 ms** | Nordheim-Fuchs + 5-group delayed layer + six-factor feedback + Xe-135 |
| `fhr-thermal-hydraulics` | **0.1 s** | Four-branch primary + two-branch intermediate + secondary Rankine |
| `fhr-plot-updater` | ~50 ms | Ring-buffer sampling for the GUI |

That is a **100:1 rate separation with bidirectional coupling** (power out at
`prke_backend/mod.rs:439`, coolant temperature back in at
`thermal_hydraulics_backend/mod.rs:1281`) running today. Multi-fidelity coupling
should be built as a generalisation of this, not as a new invention.

Nested inside it there is already a *third* rate: the 15-cell steam-generator
tube at 5e-5 s, sub-cycled 25 times per TH step (§2, Band 2). So the existing
architecture is already **three-level multi-rate**.

### 4.2 Enum-dispatched fidelity selection

The workspace's no-trait-objects rule is not a constraint here — it is the right
answer, and there are already two working precedents:

- `PipeBackend` (`crates/tampines/src/components/pipe.rs:25`) — a closed set of
  four flow models (`Lumped`, `Compressible`, `SteamHem`, `InsulatedPipe`)
  behind one enum, with the doc comment explicitly citing the no-trait-objects
  rule.
- `HeaterType` (`crates/outram-park-digital-twin-engine/src/ciet_opcua/state.rs:68`)
  — a **runtime mesh-fidelity switch**, 8 vs 15 axial nodes, whose doc says
  outright that the coarse mesh is what "keeps the simulation in real time" on
  slower hardware. **Both meshes are built up front and the thread switches
  which one it advances** (`state.rs:82-85`). That is exactly the pattern a
  fidelity ladder needs, already proven.

The generalisation:

```
NeutronicsFidelity :=
    PointKinetics(SixGroupPRKE)              // Band 0, ~250 ns
  | PromptExcursion(NordheimFuchs)           // Band 0, closed form
  | CirculatingPointKinetics(...)            // Band 0, MISSING — see §5
  | NodalDiffusion(BedokSolver)              // Band 3, blocked on the seam
  | Precomputed(ReactivityTable)             // surrogate

ThermalHydraulicFidelity :=
    Lumped(TuasNetwork)                      // Band 1, 27 ms/step
  | SteamHem(TampinesSteamArray)             // Band 2, quasi-steady
  | ChannelTh(BedokTh)                       // Band 3, blocked
  | Cfd(PimpleFoam)                          // Band 4, offline only
```

Exhaustive `match` at every consumer means adding a band is a compile error at
every site that must handle it — which is the whole point of the rule.

**Two design constraints the rule imposes that are worth stating.** No
`Box<dyn>` means a heavyweight variant (a CFD mesh, a nodal operator set) sits
*inline* in the enum, so `size_of` is the largest variant. Use `Arc<T>` for the
big read-only pieces (mesh topology, factorised operators) per the workspace
rule — that keeps the enum small and is already the mandated pattern for shared
immutable state.

### 4.3 State handoff between fidelities

**This is the hard part, and the workspace has more of it than expected.**

The field-transfer machinery already exists, ported from GeN-Foam and tested:

| Piece | Where | Tests |
|---|---|---|
| Mesh-to-mesh field transfer (`map` / `mapTgtToSrc`) | `crates/outram-foam-appbuilder-lib/src/genfoam/multi_region/mesh_to_mesh.rs` | 5 |
| RBF / polyharmonic-spline interpolation onto target cell centres | `.../multi_region/rbf_mapping.rs` | 5 |
| Bounded table interpolation with out-of-range modes | `.../common/interpolate_table.rs` | 9 |

Those three cover **coarse ↔ fine spatial handoff** and **table-driven
surrogate evaluation**. They are currently used only inside GeN-Foam's
multi-region coupling; nothing in the digital-twin path calls them.

**Conservation on switching — the rule that must not be broken.** Switching
fidelity mid-run changes the discretisation, and a naive interpolation of
intensive variables (temperature, pressure, void fraction) **does not conserve
mass or energy**. The correct handoff is:

1. **Down-switch (fine → coarse):** integrate the conserved quantity over each
   coarse cell's constituent fine cells — `sum(rho_i * V_i)`,
   `sum(rho_i * h_i * V_i)` — then divide by the coarse volume to recover the
   intensive value. Volume-weighted, never arithmetic-mean.
2. **Up-switch (coarse → fine):** distribute the coarse cell's conserved total
   across its fine cells, then **relax** toward the fine model's own profile
   over a few steps rather than imposing a discontinuous profile. The
   `TampinesSteamArray` quasi-steady pattern
   (`crates/tampines/docs/steam_generator_tube_integration.md:30-34`) is the
   working precedent for exactly this relaxation.
3. **Assert the invariant.** Total mass and total energy before and after the
   switch must agree to solver tolerance. This should be a test, not a comment.
4. **Never switch inside a corrector loop.** Switch at a timestep boundary,
   after publication to shared state, so the GUI never observes a half-switched
   plant.

**The prerequisite nobody has built.** A fidelity switch is a
*serialise-then-deserialise-into-a-different-representation* operation. That is
the same machinery as snapshot/restore, which
`docs/type-i-digital-twin-scoping.md:118-121` already flags as absent and as
"the cheapest thing on this list to fix now and the most expensive to retrofit
later". **Do snapshot/restore first and fidelity switching falls out of it.**
Doing them in the other order means building it twice.

### 4.4 Where the ROMs and surrogates come from — `raffles` is not the answer

`raffles` was expected to be the surrogate source. **It is not, and the reason
is specific rather than "it's a scaffold".**

The crate is in fact *more* implemented than its own documentation admits —
`Cargo.toml:18` and `src/lib.rs:16-19` both say "SCAFFOLD ONLY: no distribution,
sampler or estimator is implemented", which is **false** (see §6). Verified this
audit: `cargo test -p raffles` → **34 unit tests + 8 doctests, 0 failed** [M].

| Module | State |
|---|---|
| `src/distributions.rs` | **Real** — 8 distributions, pdf/cdf/ppf/moments, hand-rolled special functions, verified against published quantiles |
| `src/samplers.rs` | **Real** — Monte Carlo, Latin hypercube, grid; seeded from the `outram-mc-libs` LCG jump-ahead |
| `src/sensitivity.rs` | **Real** — Pearson/Spearman, Saltelli-Sobol, verified against the analytic Ishigami and Sudret indices |
| `src/surrogate.rs` | **54 lines of doc comment and ZERO lines of code.** `src/surrogate.rs:1-5`: *"UNIMPLEMENTED, AND NO WORK IS SCHEDULED"* |

**Assessment: `raffles` is the right *home* for surrogates and the wrong
*source* today.** There is no Gaussian process, no polynomial chaos, no kriging,
no regression, no cross-validation anywhere in the crate — or, per a
workspace-wide search, anywhere in the workspace.

What `raffles` *does* give, and it is not nothing, is the **other half of a
surrogate workflow**: a sampler to build the design of experiments, and
sensitivity analysis to decide which inputs the surrogate needs at all. A
Sobol screening step that shows three of eleven inputs carry 95% of the variance
turns an intractable eleven-dimensional fit into a tractable three-dimensional
one. That is real value available today.

**Where the surrogates should actually come from, ranked by cost:**

1. **Tabulated coefficients from offline high-fidelity runs — the default.**
   This is not a machine-learning problem. A reactivity coefficient table
   against (fuel temperature, coolant density, boron, rod position) generated by
   `outram-mc-libs`, interpolated by the *already-tested*
   `genfoam/common/interpolate_table.rs`, is a surrogate. It is cheap, it is
   auditable, it has bounded extrapolation modes, and it needs no new crate.
2. **Physics-form reduced models fitted to the spatial solver.** The MSRE case
   is the archetype: derive `beta_eff` and the circulation reactivity loss from
   `moltres`'s 300-cell spatial solve offline, then run a **reduced
   circulating-fuel PRKE** in the loop. The reduced model has the *right
   functional form* from first principles and only its parameters come from the
   expensive model. This is far more defensible than a black-box fit and is what
   `msre.md:124-137` already recommends.
3. **POD / projection-based ROM — only if 1 and 2 fail.** Genuinely useful for
   spatial fields (axial power shape, void profile), genuinely a research
   project. `raffles` is the right home; nothing exists.
4. **Neural surrogates — not recommended here.** They defeat the
   `RESPONSIBLE_USE.md` auditability posture, they need training data this
   workspace does not have, and every use case above is served more cheaply by
   options 1 and 2.

**Recommendation: do not block real-time work on `raffles`.** Build surrogates
as tables and reduced physics models, in the crates that own the physics, and
revisit `raffles::surrogate` only when a spatial-field ROM is genuinely needed.
Its `samplers` and `sensitivity` modules should be used *now* for design-of-
experiments and screening — that is a real, immediately available capability
that the crate's own docs currently hide.

### 4.5 The recommended runtime shape

```
GUI thread (egui, 20-60 Hz)
   |  reads snapshot()  -- never blocks the solver
   v
SharedState<PlantState>  =  Arc<RwLock<...>>       [exists]
   ^                    ^                    ^
   |                    |                    |
[kinetics thread]  [TH thread]        [slow thread]
  1-10 ms            50-200 ms          1-60 s
  enum:              enum:              xenon, decay heat,
  PointKinetics      Lumped             burnup table lookup
  | PromptExcursion  | SteamHem
  | CirculatingPke   | (ChannelTh)
  | Precomputed
   |                    |
   +--- power ---------->
   <--- T_fuel, T_coolant, void, rho_coolant ---+

offline, not in the loop:
  outram-mc-libs  ->  group constants, reactivity coefficients, kinetics params
  outram-foam-*   ->  closure calibration, form factors
  offbeat         ->  gap conductance / stored energy vs burnup tables
  moltres/bedok   ->  beta_eff, circulation loss, axial shapes
```

Three rules that fall out of the measurements:

1. **The fidelity enum lives on the thread that owns the physics, not on the
   shared state.** Shared state carries *results* (scalars, small arrays), never
   solver objects. This is what makes `snapshot()` a cheap clone and what lets
   the GUI run at 60 Hz against a 0.1 s solver.
2. **A slower band never blocks a faster one.** If the TH thread overruns, the
   kinetics thread keeps its own clock and uses the last published temperature.
   `fhr_sim_v2` already works this way.
3. **A band that cannot meet its deadline is not allowed in the loop.** It goes
   offline and its output becomes a table. This is the rule that keeps CFD,
   Monte Carlo, spatial circulating-fuel neutronics and fuel performance out.

---

## 5. Gaps and phasing

### 5.1 Ranked by value per effort

**Tier 1 — high value, small effort, unblocks multiple reactors**

| # | Gap | Why it ranks here |
|---|---|---|
| 1 | **Fix the three `todo!()` panic paths in the FHR parallel-branch flow solver** (`examples/fhr_sim_v2/.../parallel_branch_flow_calculator.rs:411,454,470`) | These are *live panic paths inside the running TH thread* of the workspace's flagship reactor simulator. The crash modal catches it, but a demo that can panic is not a demo. Also flagged in `fhr.md:118` |
| 2 | **Rewrite `chem-eng`'s first/second-order transfer functions as `O(1)` recurrences** (§2 Band 1b) | The only unbounded-per-step component in the workspace, in the crate named "real-time", still in TUAS's PID path. `teh-o-prke` already did this exact fix and measured it (49 µs→1.8 ms becomes 0.7–1.0 ns) — copy the pattern |
| 3 | **Bounds-checked IF97 façade returning `Result`** | ~40 panic sites; every reactor touching steam inherits the hazard; any transient that overshoots kills the physics thread. Already identified in `ipwr.md:110-121` |
| 4 | **Measure the loaded CIET step cost** | One OPC-UA write and a 5-minute run. It converts the central real-time claim in this document from a lower bound into a real number |
| 5 | **Fix the two pacing defects** (§3.4) in both loops | Twenty lines. The sign collapse makes overrun behaviour unanalysable |
| 6 | **Port one Rosenbrock stiff integrator** from the already-vendored `crates/teh-o-prke/src/time_stepping/openfoam_source_files/Rosenbrock12.C` | The reference implementation is on disk. It is the only general answer to stiffness the crate lacks; the ported RKF45 is explicit and untested |
| 7 | **Replace the dense LU on the tridiagonal conductance matrix** in TUAS with the existing `thomas_solve` | `O(n^3)` → `O(n)` in the hottest loop of the only real-time band, using a tool already in the workspace and tested |
| 8 | **A `criterion` bench harness and one committed baseline per band** | There is none. Every performance claim here except §3.1 and the delayed-neutron layer is prose asserted by nothing, so regressions are invisible |
| 9 | **Correct the false "scaffold only" claims** in `raffles`, the stale BLAS claims, and `docs/architecture.md` (§6) | Documentation that under-reports capability causes work to be redone |

**Tier 2 — high value, medium effort, the actual real-time capability**

| # | Gap | Why |
|---|---|---|
| 10 | **Snapshot / restore / backtrack** | Prerequisite for the instructor station *and* for fidelity switching (§4.3). Cheap now, invasive later |
| 11 | **Reduced circulating-fuel PRKE in `teh-o-prke`**, with `beta_eff` and loop-transit reactivity loss cross-checked against `moltres` | Unblocks MSRE. Verification target already exists and passes its own tests. See §5.3 |
| 12 | **Close the `bedok` seam** (10 `todo!()`s in `coupling/seam.rs`) and time one coupled step | Until this happens, the nodal-diffusion band is not merely unvalidated, it is unmeasurable. Blocks any credible BWR |
| 13 | **Void reactivity in the kinetics path** | The prompt-excursion timestepper carries exactly one feedback (fuel temperature) and its constructor *requires* it to be negative (`nordheim_fuchs.rs:145`). No moderator-density term exists in the layer both engine examples are built on. Blocks BWR and iPWR |
| 14 | **A test that actually calls `DriftFlux1d::step()`** | 1,081 lines of the workspace's most BWR-relevant solver, never executed by any test |
| 15 | **Finish the `tampines` component layer** (15 `NotYetImplemented`) | The algebra exists in `outram-park-fork-dwsim-libs`; this is wiring. Unblocks the flat-rectangle widgets across every reactor |
| 16 | **A `FidelityLevel` enum + conservative state handoff, with conservation asserted by test** | The actual subject of this document. Depends on 10 |

**Tier 3 — necessary eventually, large effort**

Packed-bed closures (KTA friction, Zehner-Bauer-Schlünder conductivity, graphite
properties — all three with reference values already extracted in
`vtb-findings.md:197-236`); heated boiling channel + separator + loop closure
for BWR; helical-coil once-through SG and pressuriser for iPWR; sodium reaching
TUAS and the four SFR expansion feedbacks for EBR-II; OpenFOAM I/O layer so CFD
results can be written at all.

### 5.2 Ordering

```
Tier 1 (1-9)  ──►  10 snapshot/restore  ──┬──►  11 circulating PRKE  ──►  MSRE demo
                                           ├──►  16 fidelity enum
                                           └──►  13 void reactivity ──► 12 bedok seam ──► BWR
```

Within Tier 1, items **1, 2 and 5** are the ones that make the *existing* demos
trustworthy rather than merely runnable, and should go first. Item 8 (a bench
harness) should land early enough that items 2 and 7 can be shown to have
worked.

### 5.3 The recommended first demo: **MSRE**, not FHR

`docs/reactor-scoping/README.md:120-133` recommends finishing the FHR widget
migration first. **For a real-time multi-fidelity demo specifically, MSRE is the
better first target**, for four reasons that are properties of the code rather
than of the reactor:

1. **Both fidelities already exist and both already pass tests.** The expensive
   model (`moltres`, 20 tests, measured at 1.14 s/solve [M]) and the cheap
   substrate (`teh-o-prke`) are both real. Every other reactor is missing at
   least one side.
2. **The verification target is free.** Derive `beta_eff` and the circulation
   reactivity loss from the spatial solver offline; verify the reduced model
   against it. No document retrieval, no digitisation, no benchmark access
   negotiation — the check runs on this machine today. `msre.md:130-134` makes
   the same argument.
3. **There is a genuine quantitative external check waiting.** `vtb-findings.md:176-195`
   extracted the CNRS benchmark's precursor-drift worth — **~60 pcm (roughly
   0.09 dollars of reactivity)**,
   isolated by construction from two gold eigenvalues that differ *only* in
   whether precursors drift. That turns `moltres`'s current qualitative test
   into a quantitative one.
4. **The physics is the point.** Circulating-fuel kinetics is the one case where
   a *reduced* model is not merely a speed hack — standard point kinetics is
   simply invalid, so the reduced model has to be genuinely new physics with a
   recirculation-return source. That makes the demo publishable rather than
   merely fast.

**What a genuinely-working first demo needs, concretely:**

| Item | Size | Status |
|---|---|---|
| Reduced circulating-fuel PRKE (core dwell + loop transit + return source) | Small–Medium | **Missing** |
| Offline cross-check against `moltres` spatial solver | Small | Both sides exist |
| MSRE fuel-salt properties via `LiquidMaterial::CustomLiquid` | Small | **Correlations already extracted** — `vtb-findings.md:69-83` |
| Air-cooled radiator component (salt-to-air, blower + door control) | Medium | **Missing**; build on the air-cooled tube bank |
| Secondary coolant-salt loop | Medium | Replaces `moltres`'s prescribed-temperature sink |
| Wire `moltres` in as the offline reference (not in the loop) | Small | Currently a zero-dependent island |
| MSRE widget art | Medium | **Missing**; FHR art is not reusable |
| Bounds-checked IF97 façade | Small–Medium | Tier 1 item 2 |
| Fidelity switch: `Precomputed` vs `CirculatingPke` | Small | Once the enum exists |

**Honest scope statement for that demo:** it would be a *real-time
circulating-fuel point-kinetics twin verified against a spatial solver*, not a
validated MSRE model. Validation against the measured zero-power circulation
reactivity loss requires ORNL report retrieval and figure digitisation, and must
not be claimed until it is done.

### 5.4 Two things to do before any of this

- **Consolidate the four FHR simulators.** Four near-clones across four crates
  is a maintenance liability, and it means the pacing defects in §3.4 have to be
  fixed four times. `fhr.md:216-219` raises the same question.
- **Decide whether `bedok` is the neutronics path.** It carries the committed
  BWR benchmark data, real void feedback and 57 deliberately-preserved upstream
  defects — and it does not run. Committing to it means closing the seam first;
  not committing to it means the BWR needs a different spatial neutronics
  source. `bwr.md:252-255` raises this; the seam finding in §1.3 sharpens it.

---

## 6. Corrections to existing documents

Found by checking the code, in the order the documents were read.

| Document | Claim | Reality |
|---|---|---|
| `docs/architecture.md:110-114` | `outram-mc-libs → njoy-outram-park-fork` wiring "deferred" | **Stale — the wiring is done.** `crates/outram-mc-libs/src/material/nuclide.rs:23-31,254` imports njoy's `WindowedMultipole`, `MgxsLibrary`, `ElasticAngular`, `Tab1` and `EndfLibrary`; S(a,b) at `src/material/thermal.rs:127-129` |
| `docs/architecture.md:99-106,129` | GeN-Foam inside `outram-foam-appbuilder-lib` is "*planned*", "on hold" | **Stale.** `crates/outram-foam-appbuilder-lib/src/genfoam/` is **32,256 lines with 262 tests** across neutronics, thermal-hydraulics, thermo-mechanics and multi-region |
| `docs/architecture.md` diagram | `nee-soon` "PRKE + surrogates; exposes CFD-coupling interfaces" | Only the PRKE pass-through is real (`crates/nee_soon/src/lib.rs:117-136`, 10 lines). `NeeSoon` is an empty struct at `:98`. The `xin_wang_sp3_workflow/` stages all return `NotYetImplemented` (`mgxs.rs:98`, `mesh_mc.rs:110`, `sp3_multiphysics.rs:136`, `validation.rs:97`) — one of its own tests is named `every_stage_run_is_a_beaded_placeholder` |
| `docs/reactor-scoping/bwr.md:64` | Lists 3-D nodal diffusion coupled to channel TH under **HAVE** | **The coupling does not execute.** 10 `todo!()`s in `crates/bedok/src/reference/coupling/seam.rs`. See §1.3 |
| `docs/reactor-scoping/bwr.md:154` | "Run `crates/bedok`'s benchmark case end to end. There is currently zero evidence it executes." | Correct, and understated — there is **no NEACRP benchmark test at all**, only case *builders*. `tests/benchmark/main.rs` covers IAEA-3D only, and its gates skip-and-pass when un-ignored |
| `docs/reactor-scoping/msre.md:126` | Coupled eigenvalue "roughly 1.2 s per solve on a 300-cell single-group ring" | **Confirmed independently** — 1.14 s/solve measured this audit [M]. Note the crate contains zero timing instrumentation, so the original figure was not re-checkable; it now is, via the command in §2 Band 4b |
| `docs/reactor-scoping/msre.md:127-128` | "about three orders of magnitude too slow to sit in the GUI loop" against "the engine's 1 ms physics budget" | Right for the **kinetics** thread (1140x). The engine's **TH** thread runs at 0.1 s, against which it is 11x too slow. Both budgets exist; the doc cites only one |
| `crates/raffles/Cargo.toml:18`, `crates/raffles/src/lib.rs:16-19` | "SCAFFOLD ONLY: no distribution, sampler or estimator is implemented" / "Nothing is implemented" | **False.** ~4,800 lines across `distributions.rs`, `samplers.rs`, `sensitivity.rs`; **34 unit + 8 doctests pass** [M]. Only `src/surrogate.rs` is empty |
| Workspace `CLAUDE.md` Members table | `raffles` — "Scaffold only, nothing implemented" | Same correction |
| `crates/outram-foam-turbulence-lib/src/les/mod.rs:24-25`, `src/prelude.rs:25` | Smagorinsky "is a **scaffold** (its trait methods `todo!()`-panic)" | **Stale** — `src/les/smagorinsky.rs` is 461 lines with no panic macros. `src/lib.rs:40-42` already says so; the two older notes contradict it |
| `crates/tampines-steam-tables/Cargo.toml:33-34` | "`tuas_boussinesq_solver` pulls in system BLAS via `ndarray-linalg`" | **False today.** TUAS has no `ndarray-linalg` dependency; all its temperature solves go through a crate-local LU (`.../standalone_fluid_nodes/mod.rs:32-58`, which documents the removal). Matters because this claim is used to justify avoiding a dependency edge |
| `crates/teh-o-prke/README.md:35` | "You'll need openblas to run this on linux." | **False today** for the library — `teh-o-prke` depends on `approx, ndarray, thiserror, uom, chem-eng` and uses its own inlined `SquareMatrix`. Only the GUI example's transitive graph could matter, and that is gated off Android |
| `docs/reactor-scoping/bwr.md:63` | `crates/tampines/src/multiphase_1d/drift_flux.rs` is "the closest thing to a BWR channel solver in the repo" | True, and it is **never executed by any test** — `step()` has no caller in the crate's test suite |
| `docs/reactor-scoping/README.md` readiness table | Reads as though HTR-10 has a reusable app shell | True of the *prismatic* `htgr_sim_v1`; `htr10.md:19-25` is explicit that the pebble-bed core is a rewrite. The README does not carry that caveat forward |
| `docs/outram-park-dt-plan.md:20-24` | "this repo holds reusable frameworks only" | Superseded by the 2026-07-29 maintainer decision recorded in `docs/type-i-digital-twin-scoping.md:42-46`. The dt-plan does not cross-reference it |

**Not stale, worth confirming:** `docs/type-i-digital-twin-scoping.md:81-87`'s
claim that CIET v2 "is already a Type I DT for one loop" **holds up under
measurement** — §3.1 is the evidence for it.

---

## 7. What could not be determined

Stated plainly rather than guessed at.

1. **The loaded CIET step cost.** §3.1's 27 ms median is the *idle* state. No
   OPC-UA client was available in this environment to command heater power and
   pump pressure over the wire, and the binary has no CLI flag for either. **The
   real duty cycle under load is unknown and is certainly higher.** This is the
   single most important missing measurement in this document.
2. **Any timing for `bedok`.** It does not execute. The 76.5 s Octave figure is
   the original MATLAB, on different hardware, and says nothing about the port.
3. **Any timing for `njoy-outram-park-fork`.** The crate has exactly two
   `Instant::now` sites, both in an example, and its
   `verification_and_validation/gpu_wmp_benchmark.md` is explicitly "a
   methodology template, not a results table" with results going to a gitignored
   directory. **No cross-section lookup rate exists anywhere in the crate.**
   That matters: the XS lookup rate is what sets whether an on-line multigroup
   evaluation could ever sit in a real-time loop.
4. **Whether the CIET v2 physics matches v1.** Port-equivalence V&V
   (`op-wqk.13.6`) has not been run —
   `docs/human-in-the-loop-ciet-v2-case-study.md:168-172` says so. v2 is "a
   faithful port of validated physics", not "validated".
5. **Android/Termux real-time behaviour.** The `aarch64-linux-android` check is
   a compile proxy; no on-device build or run has been done. Since the
   `HeaterType` coarse-mesh switch exists specifically for slower hardware, an
   on-device measurement would be genuinely informative, and there is none.
6. **GUI behaviour.** No egui window was opened in this session (no display).
   Every GUI claim here is compile- and code-verified only.
7. **Whether a Type I V&V anchor is obtainable.** `docs/type-i-digital-twin-scoping.md:239-244`
   flags that ANSI/ANS-3.5 validation needs reference-plant data that
   `DATA_POLICY.md` forbids. That decision is still open and it gates any claim
   that a real-time twin is *validated* rather than merely *fast*.
8. **The actual growth rate of the `chem-eng` transfer-function cost in a
   current TUAS control loop.** The 49 µs → 1.8 ms figure is `teh-o-prke`'s
   measurement of the *predecessor* mechanism on its own workload
   (`delayed_neutron_layer.rs:88`). The same code shape is still in the PID path
   TUAS imports, but nobody has measured it *there*, and the growth rate depends
   on how often the input changes. **The mechanism is confirmed by reading the
   code; the magnitude in the CIET control loop is not measured.** It should be
   — it is one `Instant::now()` pair inside the existing long regression test.
9. **The full `tuas_boussinesq_solver` test suite was not run** by this audit —
   it contains multi-hour regressions and 46 `#[ignore]`d tests, ~48 more with
   the `#[ignore]` commented out by hand. That hand-commenting convention means
   the suite's actual pass state depends on who edited it last, which is worth
   fixing independently of anything in this document.

---

## 8. Provenance

- All **[M]** figures were produced on the maintainer's Linux x86-64 box on
  2026-08-07 in release mode. Commands are recorded inline in §2 and §3.1 so
  they can be re-run. Timings are machine-specific; re-measure before citing.
- All **[C]** figures are cited to the committed file that records them.
- All **[E]** figures state the arithmetic that produced them.
- No benchmark value, report identifier, DOI or measured experimental number is
  asserted here that was not read out of a file in this repository.
- Sources internal to the workspace: `docs/reactor-scoping/` (all eight files),
  `docs/type-i-digital-twin-scoping.md`, `docs/outram-park-dt-plan.md`,
  `docs/architecture.md`, `docs/human-in-the-loop-ciet-v2-case-study.md`,
  `crates/tampines/docs/steam_generator_tube_integration.md`,
  `crates/outram-foam-basic-lib/README.md`,
  `crates/outram-mc-libs/verification_and_validation/`,
  `crates/njoy-outram-park-fork/verification_and_validation/`.
- Per `RESPONSIBLE_USE.md` and `AI_USAGE.md`, this document is **AI-assisted
  draft material and remains untrusted until human-reviewed.**
