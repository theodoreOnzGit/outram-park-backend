# CIET Educational Simulator v2 in the browser: record (gh:#924)

> Research, education and V&V only. An offline demonstration, never connected
> to a plant; not for reactor control, licensing or safety decisions.

**What this checks.** The browser demo (`web/ciet/`, `examples/ciet_web`) is meant
to be the desktop simulator v2, not a model of its own. This record shows that
it computes the same plant, step for step, and measures whether it keeps real
time. It says nothing new about the physics against the CIET facility: that is
v1's validation (in `crates/tuas_boussinesq_solver/verification_and_validation/`),
and **v1-to-v2 equivalence is still not verified**.

## Architecture

| desktop v2 | browser |
|---|---|
| GUI thread draws | the page's main thread draws (plain JS, canvas) |
| physics thread: heat transfer, controllers, real-time pacing | plant worker (`plant-worker.js`) |
| per step, on threads: DRACS flow solve, primary flow solve (which itself forks a two-branch solve) | DRACS-flow worker and primary-flow worker (`flow-worker.js`); the two-branch solve runs inline in the primary worker |

v2's flow solves read the components as they stand at the start of a step and
their flows are used on the next step. In the browser the plant worker sends a
**flow snapshot** (the fluid temperatures of the components each solve reads,
116 numbers for the DRACS and 187 for the primary loop, with the pump and valve
settings) to the flow workers, does the step's heat transfer meanwhile
(`FlowSource::Later`), and finishes the step with their flows
(`CietLoop::finish_step`). Each flow worker keeps its own copy of the loop and
sets the snapshot's temperatures into it.

## Methodology and results (2026-10-17, release, snrsi desktop)

| # | check | instrument | result |
|---|---|---|---|
| 1 | The move of v2's loop body into `CietLoop::step` changed nothing | the loop as at `c02bbf5108`, copied, run N steps with its sleeps removed, against `CietLoop::step` from the same state (temporary test, not committed: a 1900-line copy) | **equal in every field** of the plant state after 400 steps at 8 kW pump off, 400 steps at 8 kW and 4 kPa, 200 steps at 0 kW and -3 kPa |
| 2 | Flow solves inline (as on `wasm32`) give the threaded result | `stepper::tests::the_stepper_is_deterministic_and_inline_equals_threaded`: 300 steps at 8 kW, threaded twice, inline once | **equal in every field after every step**; native cost 4.8 ms/step threaded, 8.4 ms/step inline |
| 3 | Flows solved from a snapshot on other loop copies are the flows solved in the step | `stepper::tests::flows_solved_from_snapshots_are_the_flows_solved_in_the_step`: 300 steps, natural circulation then the pump at 2 kPa from 15 s | **equal in every field after every step** |
| 4 | The browser's three-worker step is its one-worker step | `ciet_web::tests::the_three_worker_step_is_the_one_worker_step` (the wasm-facing `Plant` and `FlowSolver`, natively) | **equal readouts after every step** of 300 |
| 5 | The browser's plant is the desktop stepper | `ciet_web::tests::the_browser_plant_is_the_desktop_stepper` | equal after 200 steps; controls clamp as the desktop's |
| 6 | Real time in a browser | headless Chromium 154, the built demo, 8 kW from rest, real-time pacing; the page's own counters (wall time in the workers) | see below |

**A finding from check 1: the desktop's timestep is an `f32`.** The loop's first
timestep is `get_timestep_seconds() as f64`, so 0.1 s is taken as
0.10000000149 s, and the loop keeps it while it keeps up with real time; it
switches to exactly 0.1 s only when it falls behind. Stepping exactly 0.1 s
from the start differs in the 10th figure after 400 steps. The browser plant
takes the desktop's real-time step.

### Real time (check 6)

Desktop Chromium, 1280 x 800, heater 8 kW from a cold start (21 degC):

| mode | ms per 0.1 s step (plant worker, incl. waiting) | heat transfer | DRACS flow | primary flows | could run at | behind the clock |
|---|---|---|---|---|---|---|
| 3 workers, t = 12 s | 4.9 | 3.8 | 0.13 | 3.9 | 20.6x | 0.00 s |
| 3 workers, t = 22 s | 15.4 (slowest 19.0) | 4.9 | 0.17 | 15.0 | 6.5x | 0.00 s |
| 3 workers, as fast as possible, the 8 s after t = 16 s | 13.1 | 4.1 | 0.09 | 12.8 | 7.7x | |
| 1 worker, as fast as possible, the next 8 s (ending at t = 121.6 s) | 15.1 | 15.1 (all) | | | 6.6x | |
| 3 workers, real time, t = 16 s (the same run as the two rows above) | 45.4 | 4.2 | 0.14 | 45.1 | 2.2x | |

- **Real time is reached with margin on this desktop**: the plant never fell
  behind the clock, and a step costs 5 to 45 ms against its 100 ms budget.
- **Three workers help less than one might hope**, because the critical path is
  the primary flow solve (13 to 45 ms), not the heat transfer (4 to 5 ms): 13.1
  against 15.1 ms per step flat out. The DRACS solve is cheap here only because
  the DRACS is still at rest (DRACS flow 0.0009 kg/s at 125 s). A fourth worker
  for v2's two-branch solve (forked inside the primary solve) would shorten the
  critical path; not done.
- **Not measured: a phone.** Chromium's CPU throttle (`Emulation.setCPUThrottlingRate`,
  4x and 8x tried) slows the page's main thread and not Web Workers: both gave
  the same 15 ms per step as no throttle. So this record has no phone figure.
  The page shows its step cost and "behind the clock" live, so opening it on a
  phone answers the question.

### Mobile-first

Checked at 390 x 844 (2x): the control panel opens folded with "Controls >>"
on the main view; zoom in / out / reset are 36 px buttons top right; drag,
pinch and wheel supplement them. At a phone's fitted scale the on-drawing
labels would overlap, so the drawing keeps the equipment names and a card under
it carries the temperatures and flows; zooming in brings the labels back.

## What the page draws, and what it does not

- Colour: each component's first control volume, 20 to 100 degC, v2's own map
  (`useful_functions.rs`, `hot_to_cold_colour`); the heater, DHX, CTAH and TCHX
  are shaded between their inlet and outlet thermocouples.
- Arrows: the sign of each branch's flow (TUAS counts the primary branches
  positive from the top mixing node to the bottom one). The DRACS solve returns
  a magnitude, drawn in the direction the model assumes.
- No moving tracers: the crate's animation rule takes a tracer's speed from
  residence time, and the plant state does not carry the fluid inventory that
  would need.
