# CLAUDE.md


Guidance for Claude Code (and other AI assistants) working in the
`outram-park-digital-twin-engine` crate. The workspace-root `CLAUDE.md` applies
in full — this file adds only what is specific to this crate.

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

**Tools.** `outram_mc_libs::geometry::plot` samples a slice of an assembled
CSG geometry; OpenMC-parity image output (PNG/JPG) is the preferred path once
it lands. For meshes, plot the mesh itself (cells, patches, zones).

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

## PHYSICAL CORRECTNESS IS THE FIRST PRIORITY IN THIS SIMULATOR (HARD RULE)

**Maintainer direction, 2026-09-17, stated in these words: "I want the models
physically correct in this simulator, this is the most important thing."**
Where physical correctness conflicts with anything else — a passing test, a
tidy interface, a convenient constant, a deadline, a number that matches a
published figure — **correctness wins and the other thing changes.**

This outranks the rest of this file. It does not outrank the workspace
compliance rules (`RESPONSIBLE_USE.md`, `DATA_POLICY.md`), which are about
what may be modelled, not how well.

### What this requires, concretely

- **Derive from physics; never calibrate to the answer.** Compute a quantity
  from geometry, material properties and the governing equation, then compare
  it to the published value as a **check**. Tuning a free parameter until the
  model reproduces the reference destroys the only evidence that the model is
  right — the agreement becomes a restatement of the input. A 13 % or 40 %
  disagreement that is *honestly derived* is worth more than an exact match
  that was fitted, because only the first one can be wrong.
- **Every transfer path carries all of its real mechanisms.** If a leg
  conducts, radiates and convects, model all three or state in the doc comment
  which is omitted and why. Radiation is the usual casualty: it goes as `T⁴`,
  so a constant `UA` fitted at one temperature is silently wrong everywhere
  else — and a transient exists precisely to leave the design point.
- **Put transfer terms inside the control volume's own balance**, implicitly,
  not as an adjustment to its source. A `−UA(T − T_nb)` on the matrix diagonal
  is self-limiting at any timestep; the same quantity subtracted from `Q`
  outside the solve can drive the source negative and needs guarding. If a
  term needs a guard to stay physical, the formulation is wrong — fix the
  formulation, do not add the guard.
- **Label every invented or fitted number as such, at its definition.** A
  placeholder that reads like a measurement will be cited as one. Say what
  would replace it.
- **A model that cannot answer the question must say so.** A lumped bed node
  gives a volume average, so it cannot be compared to a peak-fuel limit. State
  the limitation where a reader meets the result, not only in a design doc.

### Evidence (2026-09-17)

Three defects in one session, all of which passed their own tests:

| Defect | What was wrong |
|---|---|
| Decay-heat loss subtracted from `Q` (`physics/mod.rs`) | Not a CV coupling — could make the fission source negative, and corrupted the LTNE solid/fluid split. Belongs on the matrix diagonal. |
| `UA` sized against a ~200 K ΔT instead of the ~627 K core-to-RCCS ΔT | 3× too conductive; drained the core and walked the secondary to the triple point. Arithmetic, not structure — good structure did not catch it. |
| Two of four chain legs had **no radiation term**, only fitted constant `UA`s | The legs with the highest temperatures were the ones missing `T⁴`. Fitted at 950 K, wrong everywhere else. |

A fourth was proposed and rejected by the maintainer in the same session:
calibrating an effective axial length until the chain reproduced the published
206 kW. It would have matched exactly and proved nothing.

**Planned future exception (maintainer direction, 2026-08-17, not yet in
force — see `op-76hu`).** Once a given example's reactor model or widget is
implemented, tested, AND verified working and physically accurate by a
**human** (not just passing its own AI-written tests — see "Human review
caught what the tests did not" below), it is meant to be promoted from
`examples/*/physics/` (or wherever it lives) into `src/` as real library code,
the same standard for every example model and widget in this crate, not only
HTGR. **This rule stays "no new physics in the library" until that promotion
trigger fires for a specific model** — do not move any example physics into
`src/` on your own initiative; the human-V&V half of the trigger cannot be
satisfied by an AI assistant.

## ANIMATION IS DERIVED FROM PHYSICS, NEVER HARDCODED (HARD RULE)

**Maintainer direction, 2026-09-21, stated in these words: "never hardcode —
that is a shortcut that is never to be done in this engine", in terms of
animation.**

Every animated quantity in this crate must be **computed from the physics
state the caller supplies**. Concretely:

- **Direction of travel comes from the sign of the mass flow.** Never from the
  layout, never from a constant, never from "this is the return leg so it runs
  the other way", and never from which end the artwork happens to start at.
- **Speed comes from the residence time** — a mark crosses the run in exactly
  one residence time (`1/residence_time` of the run per second). Never a fixed
  points-per-second, never a frame counter, never a rate picked to look right.
- **Rotation, phase, amplitude and any other motion follow the same rule** — a
  pump impeller turns at its own shaft speed, not at a nominal rate.

### Why this is a hard rule and not a preference

**An animation that is hardcoded cannot be wrong on screen, so it can never
reveal a fault.** That is the whole failure: it does not merely fail to help,
it actively misleads. A hardcoded direction keeps animating a plant state the
model no longer has, and it keeps doing so most confidently in exactly the
situations the operator most needs to see — a circulator trip, a reversed
natural-circulation leg, a stalled loop. A tracer that is still marching in a
LOFC transient is a lie drawn on top of a correct model.

The inverse is what makes this crate worth anything: when direction and speed
are derived, **the animation is a readout**. Raise the helium flow and the
tracers visibly speed up. Trip the circulator and they stop. Reverse the flow
and they reverse. None of that needs any extra code — it falls out of having
refused the shortcut once.

This is not a new idea here, it is the crate's founding one, promoted to a
hard rule because it was broken: the top of this file already says the engine
turns physics into process objects where *"mass flow drives tracer direction,
residence time drives tracer travel time"*.

### Worked example — the coaxial duct, 2026-09-21

`CoaxialDuctVisual` (`src/components/pipe.rs`) draws two streams sharing one
duct body. The first draft hardcoded the annulus as *always* running opposite
to `screen_vector`, reasoning that a coaxial duct is built for counter-current
flow and the inner stream defines the axis. It even documented the assumption,
which made it look deliberate rather than wrong.

It was wrong. `screen_vector` is the duct **axis**, not a flow direction. The
correct version takes each stream's direction from the sign of **its own**
`PipeScalars::mass_flow`, so counter-current motion is a *consequence* of the
state the caller passes rather than something the widget imposes. What that
bought, immediately and for free:

- a co-current duct, or one stream stalled, now draws correctly;
- **a flow that reverses in a transient reverses its tracers** — which for the
  HTR-10 hot gas duct is precisely the LOFC behaviour the simulator exists to
  show.

### How to comply

If you are about to write a literal that decides **which way** something moves
or **how fast**, stop — that is the defect. Ask which physical quantity the
caller already has that determines it, and take it from there. If the caller
does not yet expose that quantity, add it to their state (or to the widget's
scalar interface), rather than guessing it here.

A hardcoded *geometry* — where a nozzle sits, how thick a wall draws — is a
different thing and is fine, provided it is derived from the artwork's own
rectangle rather than eyeballed in screen coordinates. This rule is about
**motion**.

### The only two ways a hardcoded value is permitted

**Deriving from state is the default. Hardcoding is the exception, and it must
be one of these two — never a silent third.**

1. **The maintainer asked for it.** If the maintainer specifies a hardcoded
   value, hardcode it. Record in the doc comment that it was their call and
   when, so the next reader does not "fix" it back.
2. **You can justify it with physically correct reasoning, written down.** A
   constant is acceptable where the physics genuinely makes it constant, or
   where the quantity is a drawing parameter with no physical counterpart at
   all. The justification goes in the `///` doc comment, next to the value,
   and must say *why the physics makes it so* — not merely that it looks right.

**"It looks right", "it is close enough", "the real one is usually like this",
and "the caller does not expose it yet" are NOT justifications.** The last one
is a task: expose the quantity.

**State honestly which of the two you used**, and if a value is an indicative
drawing choice rather than a plant dimension, say so at the point of use so it
cannot be quoted back as data. `CoaxialDuctGeometry::htr10_hot_gas_duct` is the
shape to follow: the 300 mm and 900 mm bores are cited plant dimensions, while
the insulation thickness is marked as an indicative drawing fraction precisely
because no source states it, and the duct length is deliberately absent from
the type for the same reason.

## Every egui simulator ships a headless mode (HARD RULE)

**Any example or binary with an egui/eframe GUI MUST also provide a headless
execution path** that runs the underlying model with no window, no event loop
and no GUI thread, and emits a machine-readable trace on stdout.

```
cargo run --release --example <sim> -- --headless [steps] [sample_every]
```

**WHY: a GUI-only simulator cannot be tested, and cannot be trusted.** An agent
or a CI job cannot open a window, so without this the model can only be checked
by a human watching it — which means in practice it is not checked at all. Every
claim about what the simulator does becomes unfalsifiable.

It also makes a specific, recurring class of bug invisible. `htgr_sim_v1`'s
`PlantCommands::default()` was documented as starting *"near steady state
rather than on a prompt excursion"*. The first headless run ever taken of it
(2026-09-06) showed power **overshooting to 27.8 MW — roughly 2.8x nominal —
before settling near 8.1 MW**, with the bed temperature still drifting downward
1200 s in. The docstring was wrong and had been wrong unnoticed, because nobody
could run the thing without watching it.

**Requirements:**

- **No GUI, no window, no event loop, no spawned physics thread.** The headless
  path drives the model directly.
- **Deterministic.** No wall clock, no RNG seeded from time, no I/O inside the
  loop. Same config in, byte-identical trace out. Assert this in a test — it is
  the property every committed fixture depends on.
- **Machine-readable output**, CSV or equivalent, with a stable header and fixed
  precision so a committed fixture diffs cleanly.
- **A regression test that calls the headless path directly**, not through the
  GUI.
- Where the model can leave physical range, assert bounds in that test. Loose
  bounds that catch divergence are worth far more than none; **this is a
  harness check, not physics V&V, and must not be described as validation.**

**This is a precondition for declaring any simulator's behaviour, not an
optional convenience.** A simulator without a headless mode has no reference
baseline, so it cannot be refactored safely and cannot be shown to still work
afterwards.
This rule applies to **every** egui/eframe simulator in the workspace, not only
this crate's. It moved here from the workspace `CLAUDE.md` on 2026-09-21; the
other crates with GUI simulator examples (`tuas_boussinesq_solver`,
`tampines-steam-tables`, `teh-o-prke`, `boon-lay`, and `tampines`,
`outram-park-fork-cfmesh`, `dhoby-ghaut`, which have no `CLAUDE.md` yet)
fall under it too.

## Module layout

| Module | Contains |
|---|---|
| `animation/` | Tracer kinematics: `TracerTrain`, `residence_time_from_flow`, `FlowTracer`/`TravelTime`. **Must stay `egui`-free** — it is the only module that builds for Android. |
| `color_maps/` | Ported hot/cold + steam-quality colour functions. Real, already-validated code — do not "improve" the maps; call sites depend on the exact values. |
| `components/` | One file per visual process object, each composing its physics counterpart plus visual-only fields and an `egui::Widget` impl. |
| `app_scaffold/` | `SharedState`, monitored physics threads, panel dispatch, crash modal. |

## Crate-specific conventions

- **Scripted edits: python, never perl (HARD RULE, maintainer direction
  2026-09-22).** Perl `s#…#…#` substitutions abort on `#` (`#[test]`,
  `#[derive]`), and `s|…|…|` silently turns `\|` into regex alternation — both
  happened in one session, the second corrupting a doc comment. Use the Edit
  tool, or a `python3` heredoc with exact-string replacement that asserts the
  old text was found. (An ad-hoc editing tool, not a tracked script, so the
  workspace "no Python for docs/accounting" rule does not apply.)
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
- `htgr_sim_v1::physics::temperature_cross::bedok_enthalpy_march::tests::a_crossed_htr10_profile_comes_back_cross_free`
  (**CORRECTED 2026-09-18** — this list previously cited
  `primary_loop::tests::ihx_respects_the_pinch_in_both_directions`, which no
  longer exists: the IHX/pinch logic moved into `steam_generator.rs` on
  2026-08-12 and no equivalent test survived under that name. Verified by
  grep across `examples/htgr_sim_v1/physics/` that the pinch-repair
  methodology+results example now lives in `bedok_enthalpy_march.rs`.)
- `htgr_sim_v1::physics::secondary_loop::tests::saturation_temperature_matches_if97_reference`

## Build & test

```bash
cargo build --release -p outram-park-digital-twin-engine
cargo test --release -p outram-park-digital-twin-engine --lib --tests --examples
```

`--examples` matters: the HTGR plant model's tests live in `examples/`, so a
run without it silently skips them.

Android check (library reduces to `animation/`):

> ~~The library reduces to `animation/` on Android and this check passes.~~
> **CORRECTED 2026-09-21**: this check FAILS. Measured with the command
> below: `src/app_scaffold/csv_display.rs:64` and `src/app_scaffold/crash.rs`
> (lines 560, 614, 664) use `egui` without an Android gate, so
> `app_scaffold/` does not reduce away. The failure predates the
> 2026-09-21 widget work: it reproduces with that work stashed. Tracked in
> GitHub issue #239.

```bash
cargo check --release -p outram-park-digital-twin-engine --target aarch64-linux-android
```
