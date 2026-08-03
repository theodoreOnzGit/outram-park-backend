# NEE_SOON

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping pass" command). A crate is **complete** only once the maintainer has personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

**NEE_SOON** — **N**eutron **E**nergy-dependent **S**imulation using
**O**pen-source **O**bject-**O**riented **N**umerics.

NEE_SOON is the **coupling / integration layer** of the OUTRAM PARK suite. It
does not implement transport, nuclear-data processing, or kinetics itself —
those live in dedicated crates. Instead it composes them behind a single,
human-navigable object-oriented API so a user can assemble the simulation
pieces they want without wiring the crates together by hand.

## Status — honest summary

**Mostly scaffold; the prompt-excursion path is real.**
`NeeSoon::new_prompt_excursion_model` is real, wired code — a thin pass-through
to `teh-o-prke`'s `NordheimFuchsExactTimestepper`, backed by an integration
test. The nuclear-data (`njoy-outram-park-fork`) and Monte Carlo
(`outram-mc-libs`) integration points are declared as dependencies but the
coupling logic for them is future work, deliberately out of scope for this
pass. The `xin_wang_sp3_workflow` is a documented four-stage scaffold whose
stage `run()` methods return `WorkflowError::NotYetImplemented` (each naming its
tracking bead), while carrying real Mk1 case data.

## What it composes

| Piece | Provided by | Role |
|---|---|---|
| Nuclear data / cross sections | `njoy-outram-park-fork` | energy-dependent σ(E), ν̄, χ, WMP |
| Monte Carlo transport | `outram-mc-libs` | CSG geometry, k-eigenvalue, Woodcock tracking |
| Point reactor kinetics | `teh-o-prke` | PRKE precursor / reactivity time response |
| Prompt excursion (Nordheim-Fuchs) | `teh-o-prke::nordheim_fuchs` | real-time-friendly closed-form prompt excursion + adiabatic fuel feedback, the "Prompt Excursion Layer" beneath full PRKE |
| GeN-Foam SP3 multiphysics | `outram-foam-appbuilder-lib::genfoam` | SP3 neutronics + porous-media TH + multi-region coupling (host for the Xin Wang workflow) |

## Entry point — the `NeeSoon` facade

The whole crate is reached through **one struct**, `NeeSoon`. It is the
object-oriented facade: the user constructs a `NeeSoon`, then asks it to create
the relevant simulation pieces (a data provider, a transport model, a kinetics
model, a coupled run) rather than importing each underlying crate directly.
This keeps the mental context load low — one type to learn, with
`rust-analyzer` autocompletion revealing the available pieces.

### Real, wired: `new_prompt_excursion_model`

`NeeSoon::new_prompt_excursion_model` is the one fully-wired construction path
today. It exposes `teh-o-prke`'s `NordheimFuchsExactTimestepper` — the "Prompt
Excursion Layer" of the recommended OUTRAM PARK architecture: a
real-time-friendly, closed-form model of a prompt reactivity excursion with
adiabatic fuel-temperature feedback, distinct from (and much cheaper than) full
point reactor kinetics. `NeeSoon` does not reimplement or wrap the physics; it
only exposes the constructor through the single-facade entry point.

An integration test (`prompt_excursion_model_matches_direct_teh_o_prke_construction`)
confirms the facade constructor behaves identically to calling `teh-o-prke`
directly, and that the
`chem-eng-real-time-process-control-simulator → teh-o-prke → nee_soon`
dependency chain actually compiles and runs.

```rust
use nee_soon::NeeSoon;
use uom::si::f64::*;
use uom::si::{
    heat_capacity::joule_per_kelvin, power::watt, ratio::ratio,
    temperature_coefficient::per_kelvin, thermodynamic_temperature::kelvin,
    time::second,
};

let nee_soon = NeeSoon::default();
let mut excursion = nee_soon
    .new_prompt_excursion_model(
        Time::new::<second>(1.0e-5),                          // prompt generation time Λ
        Ratio::new::<ratio>(0.007),                           // delayed neutron fraction β
        HeatCapacity::new::<joule_per_kelvin>(1.0e5),         // fuel heat capacity C_f
        TemperatureCoefficient::new::<per_kelvin>(-1.0e-5),   // fuel feedback α_f (< 0)
        ThermodynamicTemperature::new::<kelvin>(900.0),       // fuel reference temperature
        ThermodynamicTemperature::new::<kelvin>(900.0),       // initial fuel temperature
        Power::new::<watt>(1.0),                              // initial power
    )
    .unwrap();

excursion.set_external_reactivity(Ratio::new::<ratio>(1.5 * 0.007));
excursion.step(Time::new::<second>(1.0e-3));
```

## Worked coupling: the Xin Wang SP3 workflow (scaffold)

`xin_wang_sp3_workflow` is a **scaffold** of the four-stage
njoy → openmc → genfoam pipeline that reproduces **Figure 4.29** (the maximum
fuel temperature during a Mk1 PB-FHR control-rod-removal transient) of Xin
Wang's 2018 UC Berkeley PhD dissertation, *"Coupled neutronics and
thermal-hydraulics modeling for pebble-bed FHR"*
(<https://escholarship.org/uc/item/40q3985m>, open literature). Wang used
Serpent (Monte Carlo) + COMSOL (SP3 via user-defined PDEs); OUTRAM PARK
re-implements that on njoy → openmc → genfoam. The extracted methodology and
case data live in the crate's `docs/xin-wang-thesis/`.

| Stage | Type | Role | Bead |
|---|---|---|---|
| 1 | `mgxs::MgxsGenerationStage` | 8-group MGXS from ENDF via `njoy` | op-fr2.2.2 |
| 2 | `mesh_mc::MeshMonteCarloStage` | Mk1 mesh + Monte Carlo model + MGXS/power tallies via `outram-mc` | op-fr2.2.3 |
| 3 | `sp3_multiphysics::Sp3MultiphysicsStage` | GeN-Foam SP3 neutronics + porous-media TH transient | op-fr2.2.4 |
| 4 | `validation::Fig429ValidationStage` | compare vs the Fig. 4.29 reference | op-fr2.2.5 |

Every stage's `run()` is a documented **placeholder** that returns
`WorkflowError::NotYetImplemented` naming its tracking bead. No MGXS is
generated, no MC model built, no SP3 transient run, and Fig. 4.29 is **not**
reproduced. What *is* real now is the **case data** — the 8-group structure,
the transient definition, and the digitised Fig. 4.29 reference curve — so the
coupling surface compiles and each stage is a navigable, beaded Rust type. The
dependency ordering is **MGXS → mesh → SP3 → validation**; several stages are
blocked on capabilities still being built in `njoy-outram-park-fork`,
`outram-mc-libs`, and the in-progress GeN-Foam SP3 port.

```rust
use nee_soon::xin_wang_sp3_workflow::{XinWangSp3Workflow, WorkflowStage, WorkflowError};

let workflow = XinWangSp3Workflow::new();

// The case data is real and available now:
assert_eq!(workflow.mgxs().group_lower_bounds().len(), 8);

// Running any stage is a documented placeholder for now:
let err = workflow.mgxs().run().unwrap_err();
assert!(matches!(
    err,
    WorkflowError::NotYetImplemented { stage: WorkflowStage::MgxsGeneration, .. }
));
```

## What belongs here / what does not

- **Belongs here:** orchestration, the object-oriented facade, cross-crate glue
  types, ergonomic constructors, coupling schedules, and any *new* user-facing
  functionality that only makes sense once the pieces are joined.
- **Does NOT belong here:** raw physics kernels. New cross-section code goes to
  `njoy-outram-park-fork`; new transport code to `outram-mc-libs`; new kinetics
  to `teh-o-prke`. NEE_SOON only *exposes and integrates* them.

All public physical quantities exchanged across the API are dimensioned via
`uom`, never bare `f64`.

## Build & test

Always `--release` (workspace rule). System OpenBLAS is required (pulled in via
`outram-mc-libs`); on Debian/Ubuntu `sudo apt install libopenblas-dev`.

```bash
cargo build --release -p nee_soon
cargo test  --release -p nee_soon
```

## License

GPL-3.0. Member of the OUTRAM PARK workspace (`crates/nee_soon`).

## Copyright

Copyright (C) 2026 Ong Kay Chen Theodore, Professor Per F. Peterson,
University of California, Berkeley Thermal Hydraulics Lab,
Singapore Nuclear Research and Safety Institute (SNRSI),
National University of Singapore (NUS), Repository Contributors.
