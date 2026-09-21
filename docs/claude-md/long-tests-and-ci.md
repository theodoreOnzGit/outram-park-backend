<!-- Moved verbatim from the workspace CLAUDE.md on 2026-09-21 (maintainer direction: physics
     rules stay in the root, everything else here behind a pointer). STILL BINDING. -->

### TUAS natural-circulation tests are VERY long running — run them in parallel

**HARD RULE.** The CIET coupled-DRACS natural-circulation regression tests and
simulations in `crates/tuas_boussinesq_solver` (under
`pre_built_components/ciet_nat_circ_tests/`, including
`coupled_dracs_loop_tests/` and `para_heat_loss_regr_tests/`) integrate a
coupled loop at a 0.1 s timestep for 2000–2500 s of simulated time to reach
steady state — see the crate `CLAUDE.md` "Testing Notes".

**They must be run in parallel, not serially.** Let cargo's test harness use
all cores: `cargo test --release -p tuas_boussinesq_solver`. **Do not add
`--test-threads=1`**, and do not wrap them in anything that serialises
execution. If a specific case needs isolation, isolate that case rather than
the whole suite.

- **Budget real wall-clock time.** A default 120 s command timeout will kill
  them mid-run; give them a generous timeout or run them in the background.
- **Run the targeted subset** while iterating (`cargo test --release -p
  tuas_boussinesq_solver <substring>`) and the full suite only when finishing.
- **A timeout is not a failure.** Do not report a killed run as a failing test,
  and never loosen a tolerance because a long test was inconvenient.

### Any test over 5 minutes is gated behind `long-tests` (HARD RULE)

**Every individual test whose runtime exceeds 5 minutes MUST be gated behind
its crate's `long-tests` feature, and that feature MUST be in the crate's
`default` set.** Both halves bind: the gate is mandatory so a fast run is
possible at all, and the default-on is mandatory so the slow tests still run
unless someone deliberately turns them off.

**The threshold is per test, not per suite.** Three tiers — do not collapse
them together:

| runtime | treatment | in a default `cargo test`? |
|---|---|---|
| under 5 min | nothing — a plain `#[test]` | yes |
| 5 min to about an hour | `#[cfg_attr(not(feature = "long-tests"), ignore = "...")]` | **yes**, skipped only under `--no-default-features` |
| multiple hours | plain unconditional `#[ignore = "..."]` | no — opt in with `--ignored` |

**Measure, never inherit a number.** Runtimes in this workspace's own comments
have been wrong by a factor of four (see the rationale doc). Record the number
**with its date**, and re-measure rather than trusting a figure written by a
previous session on different hardware. When it is close to the threshold,
gate it. Do **not** promote an hours-long test into the middle tier, and do
**not** demote a mid-tier test into the bottom one — if a test is the evidence
for a claim quoted in this file, it belongs in the default run.

**A second, orthogonal criterion shares the same flag: a test built on heavy
reference data goes behind `long-tests` even when its runtime is well under 5
minutes** (maintainer direction, 2026-09-20). Cargo gives one lever, not two.
The point is that a *short* run must need no heavy reference data, which is
what makes the CI split below possible. **State which criterion applies in the
`ignore` message and the doc comment** — a reader who sees a 3.3 s test behind
`long-tests` will otherwise assume the runtime figure is wrong.

**This rule is about RUNTIME (and reference data) ONLY. It un-ignores
nothing.** An `#[ignore]` that exists for any other reason keeps it, at any
runtime: measurements and diagnostics that assert nothing, unimplemented or
won't-port work, and anything deliberately on hold by maintainer decision.
Before converting any `#[ignore]` to the feature gate, read its message. If it
does not say the test is slow, leave it alone.

**How to gate one:**

```rust
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "long test (~9 min); runs by default, skipped under --no-default-features"
)]
fn coupled_dracs_loop_reaches_steady_state() { /* ... */ }
```

```toml
[features]
default = ["long-tests"]
long-tests = []
```

**Use `#[cfg_attr(..., ignore)]`, never `#![cfg(feature = ...)]`.** The
`ignore` form keeps the test compiled and reports it as `ignored` when
switched off. Blanking it out with `cfg` makes it disappear silently, and a
test that can silently vanish is a test that rots.

**Turning them off, for iteration only:** `cargo quick-test` (whole workspace)
or `cargo quick-test -p <crate>`. The alias lives in `.cargo/config.toml` and
expands to `cargo test --release --lib --tests --no-default-features`.

**Work is NOT done on a `quick-test` run.** Before reporting work complete,
before committing, and before any hand-off, run
`cargo test --workspace --lib --tests --release`. **Say which of the two you
ran.** "Tests pass (quick-test; long tests not run)" is an honest report.
"Tests pass" after a `quick-test` is not.

**When you write a test that crosses the threshold, gate it in the same
change** — add the feature to the crate's `Cargo.toml` if it has none, and
state the measured runtime in the `ignore` message. Do not guess; measure.

### CI runs short on `develop` and long on `main`

| branch | workflow | command | `long-tests` |
|---|---|---|---|
| `develop` | `.github/workflows/fast-tests.yml` | `cargo quick-test` | **off** |
| `main` | `.github/workflows/full-tests.yml` | `cargo test --workspace --lib --tests --release` | **on** |
| *on request, any branch* | `.github/workflows/manual-tests.yml` | either, chosen by input | selectable |

`manual-tests.yml` fills the gap the split leaves — running the FULL suite
against `develop` on demand. It has **two** triggers because GitHub exposes
`workflow_dispatch` only for workflows present on the **default branch**, and
`main` has no `.github` directory at all. The second trigger is a push to a
`ci-run/**` branch, which a push event runs from the pushed ref itself:

```bash
git push -f origin develop:ci-run/develop     # full suite, develop's exact tree
```

Force-push is expected — `ci-run/**` branches are disposable triggers, never
merged from, safe to delete. The pattern excludes `develop` and `main` so it
cannot fire on ordinary work.

**Not yet enabled, deliberately:** `OUTRAM_PARK_REQUIRE_REFERENCE_DATA=1` on
the full job, which turns a data-skip into a hard failure. Whether the whole
workspace passes with it on has **not been measured**; switching it on
unmeasured would paint the job red for the wrong reason. Measure, then enable.
