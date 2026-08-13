# CLAUDE.md

Guidance for Claude Code (and other AI assistants) working in the
`outram-park-digital-twin-engine` crate. The workspace-root `CLAUDE.md` applies
in full — this file adds only what is specific to this crate.

## What this is

The reusable **visualization framework** for OUTRAM PARK digital twins, plus
the offline example simulators (`htgr_sim_v1`, `fhr_sim_v2`) built on it. It
turns physics state into on-screen process objects: cell count drives displayed
cells, temperature drives cell colour, mass flow drives tracer direction,
residence time drives tracer travel time.

| Composes | Crate | Role |
|---|---|---|
| Thermal-hydraulic physics | `tampines` | component state to visualize |
| Reactor-vessel / instrumentation | `nee_soon` | neutronics/kinetics state to visualize |
| Process control | `chem-eng-real-time-process-control-simulator` | controller state to visualize |

## The one rule that matters most here

**No new physics in this crate's library.** If a visualization needs a physical
quantity `tampines`/`nee_soon` do not yet expose, add it *there*, not here.
`src/` is presentation only: visual wrappers, colour maps, tracer kinematics,
and the app scaffold.

The examples are the exception — `examples/htgr_sim_v1/physics/` is that
simulator's *own* lumped plant model, which is allowed to own its correlations.
Even there, pull real property data from the workspace libraries
(`outram-park-fork-coolprop`, `tampines-steam-tables`) rather than hardcoding
constants.

## Module layout

| Module | Contains |
|---|---|
| `animation/` | Tracer kinematics: `TracerTrain`, `residence_time_from_flow`, `FlowTracer`/`TravelTime`. **Must stay `egui`-free** — it is the only module that builds for Android. |
| `color_maps/` | Ported hot/cold + steam-quality colour functions. Real, already-validated code — do not "improve" the maps; call sites depend on the exact values. |
| `components/` | One file per visual process object, each composing its physics counterpart plus visual-only fields and an `egui::Widget` impl. |
| `app_scaffold/` | `SharedState`, monitored physics threads, panel dispatch, crash modal. |

## Crate-specific conventions

- **Tracer state is application-owned.** Visual components are `egui::Widget`s
  consumed by value and rebuilt every repaint. A `TracerTrain` owned by a
  widget would reset its phase to zero each frame, so the *app* owns the train,
  advances it once per frame, and copies it into the widget at build time. Do
  not "simplify" this by moving the train into the widget.
- **Keep `animation/` `egui`-free.** Rendering of tracers belongs with each
  visual component (which is already gated off Android), not in `animation/`.
  Adding an `egui` import there breaks the crate's Android build.
- **Enum dispatch, never trait objects.** `PipeVisualState` is the pattern:
  a closed set of state sources matched exhaustively. Same for any future
  multi-source widget.
- **Scalar-backed widgets are not placeholders.** `PipeVisual::from_scalars`
  exists because a `tampines::components::Pipe` needs a whole fluid array
  behind it, which is disproportionate for a schematic connector. Callers pass
  *real* state from their own model. Do not document this path as a stub, and
  do not fabricate values to feed it.

## Examples are offline demonstrations

`RESPONSIBLE_USE.md` binds here directly: the example simulators must never be
connected to live operational systems, plant systems, safety-critical
infrastructure, or restricted infrastructure. When editing example docs, do not
soften the "demonstration model, not a validated model" framing, and do not
describe illustrative plant data (loop geometry, `UA` values, efficiencies,
inventories, controller constants) as though it came from a specific design.

## Human review caught what the tests did not (recorded 2026-08-14)

**This section is evidence, not exhortation.** `RESPONSIBLE_USE.md` says
AI-assisted output is untrusted draft material until a human reviews it. On
2026-08-14 that rule earned its keep twice in one session, on work that
compiled, passed its own tests, and carried V&V doc comments with measured
numbers in them. Both catches came from the maintainer reading the *design*,
not from anything automated.

**1. A second-law violation that no test was looking for.** The helium was
leaving the core hotter than the graphite heating it. It had been visible in
the recorded results of `the_plant_outer_correctors_converge` for some time --
`T_out = 949.7 K` against `T_bed = 896.6 K` -- sitting in a table, in a passing
test, in a doc comment. The assistant had even *reported* the inversion and
moved on, treating "pre-existing" as though it meant "not mine". The maintainer
said it was unacceptable, and it turned out to have three stacked causes: an
arithmetic-mean driving temperature that inverts above NTU = 2, two modules
deriving the same outlet with different `c_p`, and an invented 5 s gas lag.
None of the three would have been found by making the existing tests stricter,
because none of them was testing the invariant at all.

*Lesson:* a passing suite is evidence about the properties someone thought to
assert. Physical invariants -- second law, mass conservation, bounded
temperatures -- must be asserted **explicitly**, and noticing a violation is
not the same as fixing it. `the_helium_never_leaves_the_core_hotter_than_the_bed`
now asserts it at every step.

**2. A control architecture chosen for the wrong reason.** The assistant built
feedforward-plus-PI because that is the common industrial arrangement and reads
as the sophisticated answer. The maintainer asked for **feedback only**. That
is the better choice here and the assistant should have seen why: the
feedforward was an open-loop inverse model of the very exchanger whose duty
depends on the flow it was setting, so it was confidently wrong and the trim
spent its authority undoing it. Removing it improved the settled error from
-0.024 K to +0.000 K.

*Lesson:* "what a real plant usually does" is not the same as "what this model
should do". Prefer the architecture whose assumptions this model can actually
support.

**3. A related instance, same session: `c_p` where an enthalpy inverse
belonged.** The maintainer also pointed out that deriving an outlet temperature
as `T + Q/(m c_p)` is a first-order approximation, exact only for a vanishing
temperature change, and that the energy balance should instead add enthalpy and
invert the EOS for temperature. Measured afterwards: over an HTGR-sized
2 MJ/kg core rise the `c_p` shortcut is **0.733 K** off the exact inverse. That
is small, and it is precisely the class of small inconsistency that produced
cause (2) of the inversion above. `outram_park_fork_coolprop::flash::temperature_hrho`
now provides the exact backward `T(rho, h)`.

**What to do differently.** Before reporting work as done: list the physical
invariants the change could violate and assert them; when an anomaly is
noticed, chase it or say plainly that it is unfixed and why, rather than
filing it as an observation; and when a design has a conventional answer and a
simpler one, say which assumptions each needs and let the human choose.

## V&V documentation

Per the workspace rule, any test that checks physics against a reference must
document **both** methodology and results (measured numbers, the date, the
interpretation) in its `///` doc comment. The existing examples to follow:

- `animation::tests::residence_time_matches_analytical_identity`
- `htgr_sim_v1::physics::primary_loop::tests::ihx_respects_the_pinch_in_both_directions`
- `htgr_sim_v1::physics::secondary_loop::tests::saturation_temperature_matches_if97_reference`

## Build & test

```bash
cargo build --release -p outram-park-digital-twin-engine
cargo test --release -p outram-park-digital-twin-engine --lib --tests --examples
```

`--examples` matters: the HTGR plant model's tests live in `examples/`, so a
run without it silently skips them.

Android check (library reduces to `animation/`):

```bash
cargo check -p outram-park-digital-twin-engine --target aarch64-linux-android
```
