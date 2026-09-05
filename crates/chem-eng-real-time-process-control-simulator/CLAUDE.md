# CLAUDE.md

Guidance for Claude Code (and other AI assistants) working in this crate.

## Maturity: DECLARED MATURE (2026-09-05)

The API-usability rules in the root `CLAUDE.md` ("Human interface layer",
and the Haiku dogfooding hard rule) **are in force for this crate**. See the
maturity gate in that file for what this means and how the bar is revised.

- **2026-09-05 — mature.** Bar: every discretisation reproduces its
  **closed-form continuous-time solution** — ZOH first-order coefficients match
  the analytic closed form, ZOH second-order step response is **exact at the
  sample points**, Tustin reduces to the trapezoidal rule and round-trips back
  to the continuous coefficients, prewarping matches the continuous frequency
  response at `w0`, and pole-zero matching preserves both pole/zero locations
  and DC gain. First-order, second-order, decaying-sinusoid and
  decaying-exponential recurrences each match analytic superposition.
  Evidence class: **analytical solution**, supported by **cross-code
  comparison against Scilab**. **33 tests pass, 0 fail, 0 ignored** — nothing
  carrying this bar is `#[ignore]`d.

  **Provenance of the Scilab comparison.** The maintainer validated this
  simulator against Scilab as part of their PhD dissertation (Theodore Ong, UC
  Berkeley). That is the authoritative record and the reason this crate is
  declared. Recording the limit honestly: the in-source trace of it is a
  single `/// validated with scilab` comment on
  `stable_first_order_with_delay_simulation_no_zeroes`
  (`src/examples/first_order_demos.rs`). The dissertation comparison is not
  reproducible from this repository alone.

  **Therefore the bar above is written against the analytical tests**, which
  *are* reproducible here and are strong on their own — "exact at samples" is
  a stronger statement than agreement to a tolerance. Worth doing when
  convenient: port the specific Scilab cases into the test suite with their
  expected outputs, so the dissertation result becomes checkable in CI rather
  than cited. Until then the Scilab agreement is supporting evidence, not the
  measured bar.


> This crate is a member of the **OUTRAM PARK** workspace
> (`crates/chem-eng-real-time-process-control-simulator`). See the workspace root
> `CLAUDE.md` for the shared dependency policy and full migration history.
> Dependencies are inherited from the root `[workspace.dependencies]` — do not
> pin versions in this crate's `Cargo.toml`.

## What this is

**chem-eng-real-time-process-control-simulator** — a real-time process-control
library for chemical (and general) engineering: transfer functions and
controllers (PID and friends) intended to run inside time-stepping simulators.
Within the suite it supplies the **PID controllers** used by the TUAS natural-
circulation loops and the FHR educational simulators.

**License: GPL-3.0-only** — relicensed from Apache-2.0 on 2026-08-11 by
explicit maintainer direction (sole copyright holder, verified from git
history — see the crate `NOTICE`). The `Cargo.toml` now inherits
`license.workspace = true` like the rest of the workspace. **Versions
published to crates.io before the relicense (0.0.1-0.1.1) remain Apache-2.0
forever**; the relicense affects future versions only. Do not flip this
back without the maintainer's explicit direction.

## Layout (`src/lib/`)

API stability tiers (import from the tier you want):

- `stable/` — stable API.
- `beta_testing/` — recommended for new code; mostly stable. Also hosts
  `z_domain/`, the SISO port of the GNU Octave control package's
  `c2d`/`d2c`/`tf`/`filt` core (`ContinuousTransferFn`,
  `DiscreteTransferFn`, `C2dMethod`, `D2cMethod`). `z_domain` exists in
  `beta_testing` **only** — it has no `alpha_nightly` twin and the
  byte-identical-twin rule below does not apply to it. Its ported files
  carry upstream GPL attribution headers (GPLv3-or-later side of the
  mixed-licence upstream; no BSD-3 SLICOT material — see `NOTICE`); keep
  those headers intact.
- `alpha_nightly/` — unstable; `controllers/`, `stable_transfer_functions/`,
  `transfer_fn_wrapper_and_enums/`, `errors/`.

Targets: `[lib]` is `chem_eng_real_time_process_control_simulator`
(`src/lib/lib.rs`); there is also a `library_demo` `[[bin]]` (`src/main.rs`).

**`alpha_nightly/stable_transfer_functions/` and
`beta_testing/stable_transfer_functions/` are maintained as byte-identical
twins**, differing only in the `use crate::<tier>::errors::…` import. If you
change one, change the other in the same commit — `alpha_nightly` is the tier
TUAS actually imports, so fixing only `beta_testing` leaves the real consumer
broken, and vice versa. The one exception is `decaying_exponentials.rs`, which
exists in `alpha_nightly` only.

## Transfer functions are O(1) recurrences — do not reintroduce a history vector

**Every transfer-function block advances by an exact constant-time recurrence
and carries a fixed handful of state variables. Never store a `Vec` of past
step responses and re-sum it.** This was the pre-0.2.0 design and it cost up
to 2330 us per step in a 1 ms PID loop, growing without bound whenever the run
never reached the `20 * tau` retirement horizon (bead `op-fm5`).

The recurrence is the **zero-order-hold (step-invariant) discrete equivalent**
of the block. It is *exact*, not an approximation, for input held constant
between calls — that is the load-bearing claim, and the module docs cite it
(Astrom and Wittenmark, *Computer-Controlled Systems: Theory and Design*,
Prentice Hall; Seborg, Edgar, Mellichamp and Doyle, *Process Dynamics and
Control*, Wiley; Franklin, Powell and Workman; Ogata; Hochbruck and Ostermann,
"Exponential integrators", *Acta Numerica*; Oppenheim and Schafer; Smith,
*Introduction to Digital Filters*, W3K Publishing, which is open access).
**Do not add edition numbers, years or page numbers to those citations** —
none has been verified against a physical copy.

Guardrails already in the tree, in
`*/stable_transfer_functions/recurrence_tests.rs`:

- each block's recurrence is checked against the closed-form analytic
  superposition (the `FirstOrderResponse` / `SecondOrderStableStepResponse` /
  `DecaySinusoidResponse` / `DecaySecondOrderExponentialResponse` structs are
  retained purely as that reference, and are `#[allow(dead_code)]`);
- `block_state_size_does_not_grow_with_step_index` and the step-cost tests
  fail if a growing history is reintroduced.

Two properties must be preserved by any future change: **irregular timesteps**
(the recurrence uses the actual elapsed time per call, not a fixed grid) and
**dead time** (queued in a `VecDeque` bounded by `delay/dt`, empty when there
is no dead time). Simulation time must be non-decreasing across calls.

## Build, test, run

**Rule: always use `--release` for builds and tests.** Never run in debug mode.

```bash
cargo test -p chem-eng-real-time-process-control-simulator --release
cargo run  -p chem-eng-real-time-process-control-simulator --bin library_demo --release
```

## Migration notes (read on demand)

The 2026-06 consolidation log for this crate lives in **`docs/notes.md`**.
