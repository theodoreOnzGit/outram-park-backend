# MSRE — Molten Salt Reactor Experiment

Scoping document for an offline digital-twin simulator of the ORNL Molten Salt
Reactor Experiment, built in `crates/outram-park-digital-twin-engine` with a
coupled secondary loop.

> **Intended use.** Education, research, capability building, and V&V only. This
> is an offline demonstration with no connection to any operational system. See
> `RESPONSIBLE_USE.md`.
>
> **Status of this document.** Capability findings below come from a codebase
> audit performed 2026-08-06 and were spot-checked against the code. Validation
> source identifiers are **deliberately unverified** — see
> [Open validation data](#open-validation-data).

## 1. Why this reactor

MSRE is the only reactor in the target slate whose **fuel is dissolved in the
flowing coolant**. That single property exercises a part of the OUTRAM PARK
stack no other case touches: delayed-neutron precursors are born in the core,
advected out of it, and decay in the external loop. Reactivity therefore depends
on pump state, and the effective delayed-neutron fraction is lower than the
static one.

It is also small (8 MWth), thoroughly documented in openly published national
laboratory reporting, and was deliberately operated as a physics experiment —
which means the measured quantities are the ones a twin would want to reproduce.

Secondary value: MSRE ran on U-235 and later U-233. The workspace already
carries Keepin-style precursor constants for both
(`crates/teh-o-prke/src/zero_power_prke/.../six_group_constants.rs`).

## 2. Plant configuration

| Loop | Fluid | Purpose |
|---|---|---|
| Primary | Fueled salt, LiF-BeF2-ZrF4-UF4 | Fuel + coolant, circulated through a graphite-moderated core |
| Secondary | Coolant salt, LiF-BeF2 (approx. FLiBe) | Carries heat from the primary heat exchanger to the radiator |
| Heat rejection | Air | Air-cooled radiator with blowers and adjustable doors |

**The secondary side is not a steam cycle.** MSRE rejected heat to atmosphere
through a salt-to-air radiator; blower and door position were the actual
load-following control element. This matters for the engine, because the
existing `fhr_sim_v2` secondary loop is a Rankine steam cycle and is the wrong
topology here — it must be replaced, not adapted.

Signature features worth representing: fuel drain tanks and freeze valves, and
helium sparging that stripped xenon from the salt.

## 3. The physics that makes this case distinctive

Precursor transport, as implemented in `crates/outram-park-fork-moltres/src/precursors.rs`:

$$\frac{\partial C_i}{\partial t} + \nabla \cdot (u C_i) - \nabla \cdot (D_C \nabla C_i) + \lambda_i C_i = \frac{\beta_i}{k} S_f$$

A reduced real-time form additionally needs the recirculation-return source —
precursors that survive a loop transit of duration $\tau_L$ and re-enter the
core:

$$S_{\text{return}} = \sum_i \lambda_i C_i(t - \tau_L) e^{-\lambda_i \tau_L}$$

Standard point kinetics has neither term and is not valid for this reactor.

## 4. Capability audit

Audited 2026-08-06 against the workspace at commit `ebbde1b`.

### HAVE

| Capability | Where | Notes |
|---|---|---|
| Precursor advection-diffusion-decay | `crates/outram-park-fork-moltres/src/precursors.rs:74` | Upwind `fvm::div`; steady solve and backward-Euler transient step |
| Coupled circulating-fuel k-eigenvalue | `crates/outram-park-fork-moltres/src/circulating.rs:117` | Multigroup diffusion + drifting precursors; delayed source taken from the advected field |
| Closed core-plus-loop ring topology | `crates/outram-park-fork-moltres/src/ring_mesh.rs:61` | Periodic 1-D loop, two-zone core/external map — matches MSRE topology |
| Salt energy equation + Picard coupling | `crates/outram-park-fork-moltres/src/thermal.rs:93,239` | Slug-flow energy equation with volumetric HX sink; couples neutronics to temperature feedback |
| Coolant-salt properties (FLiBe) | `crates/tuas_boussinesq_solver/.../liquid_database/flibe.rs` | Density, viscosity, heat capacity, conductivity, range-clamped |
| Loop hydraulics (series/parallel, natural circulation) | `crates/tuas_boussinesq_solver/.../fluid_component_collection/` | Tested pressure-drop/flow solvers |
| Shell-and-tube heat exchanger | `crates/tuas_boussinesq_solver/src/lib/pre_built_components/shell_and_tube_heat_exchanger/` | Stand-in for the MSRE primary HX |
| Working FLiBe natural-circulation loop | `crates/tuas_boussinesq_solver/.../uw_madison_flibe_loop_components/` | Best structural template for an MSRE loop |
| Pipe widget driven by real state | `crates/outram-park-digital-twin-engine/src/components/pipe.rs:55` | Accepts temperature, mass flow and residence time from a lumped model |

**Verification actually run:** the moltres library test suite passes (20 tests,
release). Two results are worth recording:

- `zero_flow_matches_static_solver` — agreement with the static solver at
  $u \to 0$ to $|\Delta k| \approx 2 \times 10^{-16}$.
- `flow_reduces_reactivity_monotonically` — reactivity loss rises monotonically
  with flow velocity and saturates below $\beta$, which is the correct
  qualitative circulation-loss signature.

### SCAFFOLD — do not count as working

- **The moltres constants are not MSRE constants.** `circulating.rs:302` states
  in its own doc comment that the data are order-of-magnitude MSRE-like, not
  evaluated MSRE values. The reactivity-loss magnitude it produces is the right
  order but is not an MSRE calculation.
- **The crate is unreviewed.** Its README bookkeeping block has both the V&V and
  human-interface axes unchecked, and its doc comments carry the untrusted
  AI-assisted draft marker. Per `RESPONSIBLE_USE.md` it is draft material.
- **It is wired to nothing.** No crate in the workspace depends on
  `outram-park-fork-moltres`; the engine has never been connected to it.
- GeN-Foam's point kinetics explicitly defers the liquid-fuel precursor-advection
  variant (`crates/outram-foam-appbuilder-lib/.../point_kinetics/mod.rs:82`).
- `crates/nee_soon` self-describes as mostly scaffold; only the prompt-excursion
  passthrough is wired.
- Engine widgets for heat exchanger, pump, condenser, steam generator and
  instrumentation are single-rectangle placeholders.

### MISSING

| Gap | Size | Notes |
|---|---|---|
| **Real-time circulating-fuel PRKE** | Small–Medium | The blocker. See below |
| Coupled flux transient in moltres | Medium | Crate is steady-eigenvalue only for the coupled system |
| MSRE fuel-salt properties (LiF-BeF2-ZrF4-UF4) | Small | `LiquidMaterial::CustomLiquid` is a fully wired escape hatch — four functions plus bounds, no solver changes |
| Air-cooled radiator component | Medium | Nothing named radiator exists in TUAS; build on the existing air-cooled tube-bank component |
| Coupled secondary salt loop | Medium | Replaces moltres's prescribed constant-temperature HX sink, and replaces the `fhr_sim_v2` steam cycle |
| Precursor-borne decay heat, xenon transport out of core | Medium | Current models assume in-core retention; MSRE stripped xenon with helium sparging |
| MSRE-specific widget art (vessel, drain tanks, freeze valves, radiator) | Medium | `fhr_sim_v2`'s reactor art is FHR-specific and not reusable |
| Wiring moltres into the engine | Small | Currently a zero-dependent island |
| MSRE geometry and kinetics parameters | Small | Data entry, but must be sourced and cited |

#### The real-time constraint

The coupled eigenvalue solver measured at roughly 1.2 s per solve on a 300-cell
single-group ring in release build, against the engine's 1 ms physics budget —
about three orders of magnitude too slow to sit in the GUI loop. A reduced
circulating-fuel point-kinetics model must be written for the real-time path.

This yields an unusually good verification story: derive the effective delayed
fraction and circulation reactivity loss from the spatial solver offline, then
verify the cheap real-time model against it. Both sides already exist and the
spatial side already passes its tests.

Natural home for the reduced model is `crates/teh-o-prke`, consistent with
`crates/nee_soon`'s stated split of new kinetics into that crate.

## 5. Open validation data

**Access tier: openly published national-laboratory reporting.** The MSRE
program produced a large body of ORNL technical reports that are publicly
hosted and not subject to membership or licensing restrictions. This is the most
open tier available among the reactors in this slate.

> **No report identifiers are asserted in this document.** Per the workspace
> no-fabrication rule, specific report numbers, DOIs, and access terms must be
> confirmed against the actual documents before being cited. The categories
> below describe what is expected to exist, with confidence stated.

| Category | Confidence | Relevance |
|---|---|---|
| Design and construction descriptions — geometry, salt compositions, primary loop, heat exchanger, radiator | High | Supplies nearly all model input data |
| Zero-power physics experiments, including **direct measurement of the reactivity loss caused by fuel circulation** | High | The single highest-value validation target; this is exactly what the circulating solver computes |
| Dynamics and frequency-response testing | High | MSRE was deliberately operated to study circulating-fuel dynamics |
| Operating experience and program summaries, including the U-233 phase | High | Context and steady-state operating points |
| Salt thermophysical property compilations | Medium–High for the *fueled* salt specifically | Coolant-salt properties are already sourced in-repo |
| Modern MSRE benchmark cases from the molten-salt simulation literature | Medium — verify before relying | — |

**Immediately available on disk:** the upstream Moltres source is already
vendored at `crates/outram-park-fork-moltres/upstream_source/`. Its own test and
benchmark cases are the fastest concrete validation targets in this scoping
document, requiring no document retrieval at all.

**Expect to digitize.** Raw machine-readable time-series instrument data from
MSRE operation should not be assumed available. What is public is overwhelmingly
reports containing tables and plots, so validation will involve figure
digitization — which must itself be documented as a processing step per
`RESEARCH_INTEGRITY_AND_PROVENANCE.md`.

## 6. Proposed work breakdown

Suggested bead structure under the `op-wqk` epic. Dependency edges reflect real
ordering constraints, not just narrative sequence.

| Bead | Work | Depends on |
|---|---|---|
| `msre_sim_v1` | Parent task for the simulator | — |
| Fuel-salt properties via `CustomLiquid` | Four correlations plus valid temperature bounds, sourced and cited | — |
| Circulating-fuel PRKE in `teh-o-prke` | Reduced real-time model with core dwell and loop transit time | — |
| PRKE verified against the spatial solver | Offline cross-check of effective delayed fraction and circulation reactivity loss | Circulating-fuel PRKE |
| Air-cooled radiator component | Salt-to-air crossflow with blower and door control | — |
| Secondary coolant-salt loop | Replaces the prescribed-temperature sink | Radiator, fuel-salt properties |
| Wire moltres into the engine | Dependency edge plus state plumbing | Circulating-fuel PRKE |
| MSRE widget art | Vessel with graphite stringers, drain tanks, freeze valves, radiator | — |
| MSRE parameter set | Geometry and kinetics constants, sourced and cited | — |
| V&V against zero-power circulation measurement | Methodology and measured results per the workspace V&V rule | All of the above |

## 7. Open questions for the maintainer

1. **Drain tanks and freeze valves** — model them, or draw them as static
   annotation? Full modelling is Medium–Large and is the signature MSRE safety
   feature, but is not needed for a first twin.
2. **Which fuel loading** — U-235 phase, U-233 phase, or selectable? Both
   precursor constant sets already exist in `teh-o-prke`.
3. **Does moltres need human V&V sign-off before the engine depends on it?** It
   is currently untrusted draft material with both bookkeeping axes unchecked,
   and wiring it into a simulator raises what that draft status implies.
