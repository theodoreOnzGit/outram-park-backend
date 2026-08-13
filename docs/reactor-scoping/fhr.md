# FHR — fluoride-salt-cooled high-temperature reactor (Mk1 PB-FHR architecture)

Scoping document for the FHR digital-twin simulator in
`crates/outram-park-digital-twin-engine`, with its coupled intermediate and
secondary loops.

> **Intended use.** Education, research, capability building, and V&V only. This
> is an offline demonstration with no connection to any operational system. See
> `RESPONSIBLE_USE.md`.
>
> **Status of this document.** Capability findings come from a codebase audit
> performed 2026-08-06. Validation source identifiers are **deliberately
> unverified** — see [Open validation data](#open-validation-data).

Relates to beads `op-wqk.8` (`fhr_sim_v2` migration) and `op-wqk.13` (CIET
simulator v2).

## 1. Status — the most developed reactor in the slate

FHR is furthest along by a wide margin, and it is the only case in this slate
with **recorded, quantitative validation results against experimental data**.

There are three near-clone FHR simulators in an evolutionary chain, plus a
verbatim mirror:

| Simulator | Kinetics | Secondary loop |
|---|---|---|
| `crates/teh-o-prke/examples/fhr_sim_v1/` | Six-group PRKE | **None** |
| `crates/tampines-steam-tables/examples/fhr_sim_v1/` | Six-group PRKE | Lumped steam generator plus IF97 Rankine states |
| `crates/outram-park-digital-twin-engine/examples/fhr_sim_v2/` | Prompt excursion plus five-group delayed layer | Fifteen-cell compressible steam-generator tube |

`fhr_sim_v2` is canonical.

## 2. Correction to the bead record

**Bead `op-wqk.8` is half done, and its own comment is wrong.** A comment dated
2026-07-16 says it "looks DONE… close candidate." It is not.

The example imports **only the crash-detection scaffold** from the engine —
`spawn_monitored`, `ThreadHealth`, and the crash modal. **All rendering is still
local**, in `examples/fhr_sim_v2/app/local_widgets_and_buttons/`. Step one
(migrate into the engine crate) is complete; step two (rework to use engine
widgets) has not started.

**Do not close that bead.** This is also precisely the work item behind bringing
the FHR widgets into the shared library.

## 3. Plant configuration

| Loop | Fluid | Purpose |
|---|---|---|
| Primary | FLiBe | Four-branch network through a pebble-bed core |
| Intermediate | HITEC nitrate salt | Two-branch network, salt-to-salt intermediate heat exchanger |
| Secondary | Water / steam | Rankine cycle: steam generator, turbine, condenser, feed pump |

Geometry is sized from a **public** generic FHR core model, cited in-code.

## 4. Capability audit

Audited 2026-08-06 against the workspace at commit `ebbde1b`.

### HAVE

| Capability | Where | Notes |
|---|---|---|
| **Genuinely coupled three-loop plant** | `examples/fhr_sim_v2/app/thermal_hydraulics_backend/` | Four-branch primary and two-branch intermediate networks solved by root-finding over a fluid-component super-collection |
| Real salt-to-salt intermediate heat exchanger | `crates/tuas_boussinesq_solver/.../gfhr_pipe_tests/components.rs:541` | Shell FLiBe, tube HITEC |
| **Bidirectional kinetics/thermal-hydraulics closure** | `.../prke_backend/mod.rs:439`, `.../thermal_hydraulics_backend/mod.rs:1281` | Power out, coolant temperature back in, through shared state |
| Secondary Rankine cycle through IF97 | `.../secondary_loop/mod.rs` | Condenser, pump, steam generator, turbine. SG tube is a persistent fifteen-cell compressible array |
| **Regression tests for the kinetics fix** | `.../prke_backend/mod.rs:779,831` | Recorded result: prompt-only oscillation of 194.2 MW peak-to-peak on a 31.8 MW mean reduced to 0.0086 MW on a 35.8 MW mean once the delayed layer was added |
| **FLiBe, FLiNaK, HITEC, Dowtherm and oil properties** | `crates/tuas_boussinesq_solver/.../liquid_database/` | Traceable sourcing; the heat-capacity doc comment even adjudicates between two published values and their stated uncertainties |
| **CIET — validated against experiment** | `crates/tuas_boussinesq_solver/.../ciet_steady_state_natural_circulation_test_components/` | Roughly 39,800 lines, 164 tests. Validated against CIET experimental data and an independent code |
| Recorded V&V results | `crates/tuas_boussinesq_solver/verification_and_validation/` | See below |
| DRACS natural-circulation loops (CIET geometry) | `.../dracs_loop_components.rs` and calibration ladder | Includes a real mesh-refinement study at 2x, 5x, 10x and 20x |
| **TRISO fission-product release, partly validated** | `crates/boon-lay` | Roughly 15,150 lines, 175 tests. Walk-on-spheres first passage plus a port of an external release code, with a V&V document comparing against an analytical series solution |
| Wakao packed-bed Nusselt correlation | `crates/tuas_boussinesq_solver/.../nusselt_number_correlations/enums.rs:141` | Exists and is wired — though in practice used for a heater insert, not a pebble bed |
| Validated molten-salt heat exchanger dataset | `.../shell_and_tube_heat_exchanger/tests/hitec_molten_salt_to_yd325_du_heat_exchanger/` | Roughly 4,070 lines against a published prototype salt heat-exchanger study |
| CIET simulator v2 with OPC-UA telemetry | `crates/outram-park-digital-twin-engine/src/bin/ciet_educational_simulator_v2/`, `src/ciet_opcua/` | Roughly 11,550 and 4,200 lines |

#### The recorded validation results

Two committed V&V documents carry methodology **and** measured numbers, which is
what the workspace V&V rule requires:

- **Coupled natural circulation against experiment and an independent code** — a
  full 25-case table across three data sets. Recorded outcome: maximum absolute
  error of **6.80%** on the DRACS side and **5.60%** on the primary side, against
  **6.76%** and **6.65%** for the independent code on the same data. Generated by
  a test, with a provenance section that explicitly disclaims fabrication.
- **A form-loss recalibration study**, methodology and pass criteria set to the
  independent code's own published agreement band.

Twenty-five per-case CSVs are committed alongside. The documents record a passing
release-mode run, 25 of 25, dated 2026-07-15.

**Caveat, stated plainly:** both documents carry an **AI-generated, requires
human review** banner. They are not maintainer-signed.

### SCAFFOLD — do not count as working

- **The core is not a porous-media component.** It is a plain insulated fluid
  component with a hardcoded form loss of 5.05 and the comment "not putting in
  ergun equation yet" (`.../gfhr_pipe_tests/components.rs:59`). Nusselt is
  pipe-Gnielinski, not a packed-bed correlation.
- **The pebble bed is a lumped UO2 slug** with a constant-heat-capacity
  heuristic — no graphite matrix, no TRISO layers, no intra-pebble conduction.
  One of its temperature getters is an outright `todo!()`.
- **The intermediate heat exchanger is wall-resistance-limited by construction** —
  the tube side uses an idealised infinite Nusselt number.
- **Steam-generator duty is a user slider**, not physics. The critical-heat-flux
  path is disabled by a hardcoded flag, and the departure-from-nucleate-boiling
  model it would use is self-described as improvised — a hand-fitted curve.
- Turbine geometry and efficiency are arbitrary, self-documented as "I don't have
  actual figures." The generator parameters carry an AI-generated marker.
- Six-factor feedback terms are literals with the comment "some of these are
  arbitrary"; control-rod worth comes from "an arbitrary map."
- **The parallel-branch flow calculator carries three live `todo!()` calls and a
  header saying it needs work and does not work now.**
- `crates/nee_soon` is roughly 1,000 lines and self-describes as mostly scaffold.
  Its workflow aimed at reproducing a published Mk1 PB-FHR control-rod-removal
  transient is a documented placeholder, entirely unimplemented — though the
  source dissertation is already in the repo root.
- Twelve `tampines` component methods return not-implemented, so the heat
  exchanger and condenser widgets render flat rectangles by design.

### MISSING

| Gap | Size |
|---|---|
| **Migrate `fhr_sim_v2` onto engine widgets** — step two of `op-wqk.8` | Medium |
| CIET v2 to v1 port-equivalence V&V (`op-wqk.13.6`, priority 1, methodology already written) | Small |
| Wire Ergun or a pebble-bed friction correlation into the core; replace the hardcoded form loss | Small–Medium |
| **An FHR DRACS loop** — there is none. The CIET pattern exists in full but needs FLiBe and PB-FHR geometry. Passive decay-heat removal is unmodelled | Medium |
| Pebble and TRISO thermal model: graphite heat capacity, effective bed conductivity, intra-pebble radial conduction; retire the UO2 slug | Medium |
| Physical steam generator: replace the slider with a nodalised exchanger; a defensible CHF correlation | Medium |
| Flesh out the `tampines` component layer | Medium |
| FLiBe-specific heat-exchanger validation | Medium |
| Reactivity coefficient set from data — Doppler, coolant density, graphite | Large, blocked on the nuclear-data pipeline |
| Gas Brayton secondary — nothing exists workspace-wide | Large |
| Tritium transport in FLiBe, and salt redox and corrosion chemistry | Large |

**Property test coverage is thinner than it looks.** FLiBe has two tests, FLiNaK
one, HITEC one, the oil none. Those that exist are self-consistency checks against
the correlations' own tabulated values, not independent validation against
measurement scatter. Density is linear-only by documented choice; enthalpy uses a
constant heat capacity.

## 5. Open validation data

**Access tier: the best in this slate — and much of it is already used in-repo.**

> **No report identifiers, benchmark numbers, or measured values are asserted
> here.** They must be obtained from the actual documents.

### High confidence, and already cited in the codebase

| Source | Role |
|---|---|
| **CIET experimental natural-circulation data**, via two published studies from a US national laboratory group | The single most directly usable FHR-surrogate dataset. Already exploited across 25 coupled cases. The accepted manuscript is noted in-repo as openly available |
| **A UC Berkeley doctoral thesis on experimental validation of FHR passive safety systems** | Publicly available. Used for form losses and parasitic heat-loss data |
| **A fluoride-salt coolant property review** and a **national-laboratory liquid-salt property database** | Both public; already the FLiBe and FLiNaK basis |
| **A UC Berkeley doctoral dissertation** on Mk1 PB-FHR control-rod-removal transients with three-dimensional neutronics and porous-media thermal hydraulics | **The PDF is already in the repo root**, and a scaffolded workflow exists specifically to reproduce one of its figures. A ready-made validation target |
| **A public generic FHR core model** from a commercial vendor | Already the basis for the simulator's geometry. A public benchmark specification is believed to exist — unverified |
| **A public preliminary design report for the Mk1 PB-FHR** | Believed downloadable. Fetch it rather than citing figures from memory |

### Medium-to-high confidence, not yet used

- The original **packed-bed pressure-drop** and **packed-bed Nusselt** papers,
  both already named in code as the intended correlations.
- **German nuclear standards for pebble-bed pressure drop and heat transfer** —
  publicly published, and the obvious reference for the missing friction path.
- **Pebble-bed effective conductivity** — a well-documented correlation model,
  plus a European effective-conductivity experiment campaign. Medium confidence
  on raw data availability; the correlations themselves are well documented.

### Not present in-repo

`reference-data/` contains only an ENDF README. There are no raw CIET time-series
files, no design tables and no property measurement tables committed — **every
experimental number currently in the codebase is a literal transcribed into a
Rust source file or doc comment.** That is worth knowing before treating any of
it as a reproducible data pipeline.

## 6. Recommended next steps

1. **Finish `op-wqk.8` step two** — migrate the local FHR widgets into the engine
   component library. This is the immediate task, and it benefits every other
   reactor in the slate, since they will all draw from the same library.
2. **Run the CIET v2 port-equivalence check** (`op-wqk.13.6`). It is priority 1,
   the methodology is already written, and until it runs, v2 must not be
   described as validated.
3. **Replace the hardcoded core form loss** with a real packed-bed correlation.
   Everything downstream of the core flow depends on it.
4. **Build the FHR DRACS loop.** Passive decay-heat removal is the FHR safety
   case, the CIET pattern exists in full, and it is unmodelled here.
5. Get maintainer sign-off on the two V&V documents, or explicitly mark them as
   provisional.

## 7. Proposed work breakdown

| Bead | Work | Depends on |
|---|---|---|
| `op-wqk.8` step two | Migrate FHR local widgets into the engine library | — |
| `op-wqk.13.6` | CIET v2 port-equivalence run | — |
| Pebble-bed friction in the FHR core | Replace the hardcoded form loss | Ergun implementation |
| FHR DRACS loop | FLiBe plus PB-FHR geometry on the CIET pattern | — |
| Pebble and TRISO thermal model | Graphite properties, bed conductivity, radial conduction | Graphite properties |
| Physical steam generator | Nodalised exchanger plus a defensible CHF correlation | — |
| Fix the parallel-branch flow calculator | Three live `todo!()` calls; header says it does not work | — |
| FLiBe heat-exchanger validation | Against a published salt dataset | — |
| Reproduce the published control-rod-removal transient | The scaffolded workflow's stated goal | Reactivity coefficients |
| Maintainer sign-off on the V&V documents | Currently AI-generated and unreviewed | — |

## 8. Open questions for the maintainer

1. **Which of the three FHR simulators survive?** Three near-clones plus a mirror
   is a maintenance liability. Consolidating on `fhr_sim_v2` seems right, but the
   older two live in other crates' example directories and may serve as those
   crates' documentation.
2. **Do the two V&V documents get your sign-off?** They carry real measured
   numbers and good provenance discipline, but an AI-generated banner. They are
   the strongest validation evidence in the workspace, and their status matters.
3. **Is the FHR DRACS in scope now?** It is the passive safety case and currently
   absent, but it is a medium-sized piece of new work.
