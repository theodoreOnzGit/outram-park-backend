# Rationale: build, test and CI policy — measurements and history

> Split out of the root `CLAUDE.md` on 2026-09-21 to keep that file under the
> 150k-character context limit. **This is the full original text, verbatim.**
> The binding rules live in `CLAUDE.md` under "Build & test"; what is kept
> here is the measured disk-usage evidence, the CI-trigger investigation and
> the long-tests worked examples.

## Build & test

A system BLAS is **only** needed to run `outram-foam-basic-lib`'s
`matrix_bench` test (its sole remaining `ndarray-linalg` dev-dependency) — no
library in the workspace needs it. If you want that bench:

```bash
# Arch / EndeavourOS
sudo pacman -S openblas
# Debian / Ubuntu / Mint
sudo apt install libopenblas-dev
```

**This workspace has a submodule as of 2026-09-20** — `reference-data/ace`
(`theodoreOnzGit/ace_and_other_data`), holding the gzipped NJOY2016 ACE tables
that are too large to track here directly. Clone with
`git clone --recurse-submodules`, or run `git submodule update --init
reference-data/ace` afterwards. A plain clone leaves that path an **empty
directory rather than an error**, so nothing complains until something looks
for a table and does not find one.

```bash
cargo build --workspace --release                  # all libraries
cargo check --release --workspace --lib --tests     # type-check (see note below)
cargo test  --workspace --lib --tests --release    # run the test suites
```

Note: a bare `cargo test --workspace` also compiles the **examples**. Use
`--lib --tests` to skip them.

**Type-check in RELEASE too — `--release` on `cargo check` as well (maintainer
direction, 2026-09-17).** The check line above used to omit `--release` and was
annotated "mode-independent". It is mode-independent in what it *reports* — the
same type errors either way — but not in what it *costs*: without `--release`
cargo builds a second, complete `target/debug` tree beside the release one, so a
40+ crate workspace pays for two full sets of artifacts.

Measured on this workspace, 2026-09-17: `target/debug` had grown to **3.7 GB**
beside a 13 GB `target/release`, on a container whose writable allowance is
roughly 38 GB — of which ~11 GB is the base image and toolchain before any build
starts. Deleting `target/debug` was pointless while the mandated check command
rebuilt it: it was back to 3.7 GB within two runs. Re-run as `cargo check
--release --workspace --lib --tests`, it finished in **85 s against the existing
release tree, exit 0, with `target/release` unchanged at 13 GB** and no debug
tree created at all.

So: **pass `--release` to `cargo check` as well**, and keep a single build tree.
This is the same reasoning as the release-mode rule under "Workflow rules" — it
simply extends it to the type-check, which had been the one command still
pulling in the dev profile. `cargo quick-test` (`.cargo/config.toml`) already
expands to a `--release` invocation and is unaffected.

### EVERYTHING is release — every profile, every target, no exceptions

**Maintainer direction, 2026-09-19: "everything should be release, no debug".**
The 2026-09-17 entry above fixed the workspace type-check; this generalises it.
**Every `cargo build`, `check`, `test`, `run`, `clippy` and `bench` in this
workspace passes `--release`** — in a command you type, in a command a doc
tells someone to type, and in a script.

**The cross-compilation targets were the larger hole, not the host.** The host
`target/debug` that prompted the first rule was 3.7 GB. Measured 2026-09-19,
after that rule was already in force:

| tree | size | built by |
|---|---|---|
| `target/wasm32-unknown-unknown/` (dev) | **5.7 GB** | `scripts/check-wasm.sh`, 34 crates |
| `target/debug/` | 4.2 GB | per-crate `cargo check` with no profile |
| `target/release/` | 4.0 GB | everything else |
| `target/aarch64-linux-android/` (dev) | 651 MB | the Android gate |
| `target/thumbv7em-none-eabihf/` (dev) | 371 MB | petir's `no_std` checks |

The wasm gate alone was carrying more artifacts than the entire release build,
because a `--target` invocation still defaults to the dev profile — it just
puts the result under a different directory where nobody looks. Fixed in
`scripts/check-wasm.sh` and in the eight crate `CLAUDE.md` files whose
prescribed commands omitted it (16 command lines), plus 14 more in seven
`README.md` files and four in `crates/petir/docs/verification-summary.md`.

**Measured after, same session.** The gate re-run in release reports
**37 ok, 0 failed, 6 excluded** — identical to the dev-profile result, which
is the point: the profile changes what it costs, not what it reports. The
tree it leaves behind is **744 MB instead of 5.7 GB**, a 7.7x reduction for
the same answer. Across all five trees the workspace went from roughly 15 GB
to 5.2 GB, and the container from 13 GB free to 23 GB.

**What this does NOT change.** Historical records stay as they were run:
`verification_and_validation/generated/`, `debug_markdowns/`, the
`docs/<crate>-api.md` mirrors and the V&V logs record commands that were
actually executed, and rewriting them would falsify the record. Fix the
instruction, never the receipt.

**`cargo install`, `cargo publish` and `cargo fmt` need nothing** — the first
two build in release already, the third builds nothing.

### TUAS natural-circulation tests are VERY long running — run them in parallel

**HARD RULE.** The CIET coupled-DRACS natural-circulation regression tests and
simulations in `crates/tuas_boussinesq_solver` (under
`pre_built_components/ciet_nat_circ_tests/`,
including `coupled_dracs_loop_tests/` and the
`para_heat_loss_regr_tests/`) take a **very** long time. They
integrate a coupled loop at a 0.1 s timestep for 2000–2500 s of simulated time
to reach steady state — see the crate `CLAUDE.md` "Testing Notes".

**They must be run in parallel, not serially.** Let cargo's test harness use
all cores rather than forcing a single thread:

```bash
# GOOD — the harness parallelises across tests by default
cargo test --release -p tuas_boussinesq_solver

# BAD — serialises every case; these tests are far too slow for this
cargo test --release -p tuas_boussinesq_solver -- --test-threads=1
```

So: **do not add `--test-threads=1`**, and do not wrap them in anything that
serialises execution. If a specific case needs isolation, isolate that case
rather than the whole suite.

Practical consequences for an agent or a CI step:
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

**The threshold is per test, not per suite.** A test binary that takes 12
minutes across 40 tests running in parallel contains no long test; a single
test that takes 6 minutes does. Measure the test, not the `cargo test`
invocation.

**Three tiers, and this rule only creates the middle one.** The workspace
already had the other two; do not collapse them together.

| runtime | treatment | in a default `cargo test`? |
|---|---|---|
| under 5 min | nothing — a plain `#[test]` | yes |
| 5 min to about an hour | `#[cfg_attr(not(feature = "long-tests"), ignore = "...")]` | **yes**, skipped only under `--no-default-features` |
| multiple hours | plain unconditional `#[ignore = "..."]` | no — opt in with `--ignored` |

**Measure, never inherit a number.** Runtimes in this workspace's own comments
have been wrong by a factor of four. On 2026-09-16 the two
`outram-park-fork-liggghts` cases were timed for the first time:
`bulk_packing_matches_liggghts` was documented at ~210 s and measured **317 s**
— which moved it across the threshold and changed how it had to be gated —
and `lifting_cylinder_heap_matches_liggghts` was documented at ~10 min in the
test and 25 min in the crate's `CLAUDE.md`, and measured **2313 s (38.5 min)**.
Runtime is also machine-dependent, so record the number **with its date**, and
re-measure rather than trusting a figure written by a previous session on
different hardware. A test sitting just under the threshold on one machine is
over it on another; when it is close, gate it.

**Do NOT promote an hours-long test into the middle tier.** Several already
exist — TUAS's `dracs_mesh_refinement/mesh_refinement_{10,20}_times.rs` carry
`#[ignore = "regression test takes several hours"]` across 18 tests. Putting
those behind `long-tests` would make the ordinary suite take a day, which
defeats the purpose of having a suite that anyone runs. They stay
unconditionally ignored and opt-in.

**Do NOT demote a mid-tier test into the bottom one either.** An
unconditional `#[ignore]` on a 6-minute test is how a V&V claim ends up
protected by nothing: `outram-park-fork-liggghts`'s bulk-packing and
angle-of-repose cases each back a number quoted in this file's maturity
roster, and neither had ever run in a normal suite. If a test is the evidence
for a claim, it belongs in the default run.

### `long-tests` also gates the REFERENCE-DATA tier (maintainer direction, 2026-09-20)

**A second, orthogonal criterion shares the same feature flag: a test built on
heavy reference data goes behind `long-tests` even when its runtime is well
under 5 minutes.** Cargo gives one lever, not two, so both criteria land in
`long-tests`.

The point is that a *short* run must need no heavy reference data, which is
what makes the CI split below possible. First applied to
`njoy-outram-park-fork`'s NJOY2016 oracle tier:

| test | measured runtime | gated on |
|---|---|---|
| `acer_ce_esz_vs_njoy2016` | 88.5 s (2 tests) | **data** |
| `acer_broadening_vs_njoy2016` | 69 s | **data** |
| `acer_thermal_vs_njoy2016` | 3.3 s | **data** |

None qualifies on runtime. All three read oracles extracted from the
`reference-data/ace` submodule's NJOY tables (316 MB for U-235 alone).

**State which criterion applies in the `ignore` message and the doc comment.**
A reader who sees a 3.3 s test behind `long-tests` will otherwise assume the
runtime figure is wrong, which is exactly the kind of mistrust that gets a gate
deleted.

### CI runs short on `develop` and long on `main`

**Maintainer direction, 2026-09-20.** Until that date `fast-tests.yml` covered
**both** branches and passed `--no-default-features` on both, so **CI never
exercised the `long-tests` tier at all** — the workflow's own header said "a
green badge from this file means 'nothing fast is broken', never 'the suite
passes'", and nothing ran the other half.

| branch | workflow | command | `long-tests` |
|---|---|---|---|
| `develop` | `.github/workflows/fast-tests.yml` | `cargo quick-test` | **off** |
| `main` | `.github/workflows/full-tests.yml` | `cargo test --workspace --lib --tests --release` | **on** |
| *on request, any branch* | `.github/workflows/manual-tests.yml` | either, chosen by input | selectable |

`fast-tests.yml` is now `develop`-only: the full command is a strict superset
of `quick-test`, so running both on `main` would be pure duplication. The full
job checks out submodules and allows 360 minutes; it is Linux-only, because the
long tier is dominated by TUAS's coupled natural-circulation regressions and a
three-OS matrix buys little there.

**`manual-tests.yml` (added 2026-09-20) fills the gap the two-branch split
leaves: running the FULL suite against `develop` on demand.** Neither of the
other two can — `fast-tests.yml` is short by construction, and `full-tests.yml`
only fires on `main`.

**It has TWO triggers, and the reason is a GitHub constraint worth knowing.**
`workflow_dispatch` is exposed ONLY for workflows present on the **default
branch**. Measured 2026-09-20: `full-tests.yml` already carried
`workflow_dispatch:`, and `POST /actions/workflows/full-tests.yml/dispatches`
still returned **404**, with `GET /actions/workflows` listing only
`fast-tests.yml` — because `main` has no `.github` directory at all, sitting
2056 commits behind `develop`. So adding another dispatch-only workflow to
`develop` would have changed nothing.

The second trigger is therefore a **push to a `ci-run/**` branch**, which the
default-branch rule does not cover, because a push event runs the workflow file
from the pushed ref itself:

```bash
git push -f origin develop:ci-run/develop     # full suite, develop's exact tree
```

Force-push is expected — `ci-run/**` branches are disposable triggers, never
merged from, safe to delete. The pattern deliberately excludes `develop` and
`main` so it cannot fire on ordinary work. That path takes no inputs (a push
carries none) and always runs the full scope; the dispatch path adds
scope/crate/filter inputs **once the file reaches `main`**, which as of
2026-09-20 it has not.

**Not yet enabled, and deliberately:** `OUTRAM_PARK_REQUIRE_REFERENCE_DATA=1`
on the full job, which turns a data-skip into a hard failure and is the real
guard against a test "passing in 0.00 s having asserted nothing". Whether the
whole workspace passes with it on has **not been measured**, and switching it on
unmeasured would paint the job red for the wrong reason. Measure, then enable.

**This rule is about RUNTIME ONLY. It says nothing about why else a test may
be ignored, and it un-ignores nothing on its own.** An `#[ignore]` that exists
for any other reason keeps it, at any runtime:

- **Measurements and diagnostics that assert nothing** — `ldu_matrix`'s
  `"measurement, ~31 s"` benchmarks, `keff.rs`'s hardware-dependent timing
  runs, `reactor_physics.rs`'s `"a spectrum-direction diagnostic, not a
  gate"`. A test that cannot fail is not protecting anything, so putting it in
  the default suite buys nothing and costs wall clock.
- **Unimplemented or won't-port work** — most of
  `outram-mc-libs/tests/openmc_notebooks/`, each carrying its bead id.
- **Deliberately on hold** by maintainer decision.

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

and in that crate's `Cargo.toml`:

```toml
[features]
default = ["long-tests"]
long-tests = []
```

**Use `#[cfg_attr(..., ignore)]`, never `#![cfg(feature = ...)]`.** The
`ignore` form keeps the test compiled and reports it as `ignored` when
switched off, so a skipped long test is visible in the output. Blanking it
out with `cfg` makes it disappear silently, and a test that can silently
vanish is a test that rots. The state a reader wants — "this ran", "this was
deliberately skipped", "this does not exist" — must stay distinguishable.

**Turning them off, for iteration only:**

```bash
cargo quick-test                 # whole workspace, long tests skipped
cargo quick-test -p <crate>      # one crate
```

The alias lives in `.cargo/config.toml` and expands to `cargo test --release
--lib --tests --no-default-features`. Cargo has no flag to disable a single
default feature, so `--no-default-features` is the only mechanism; it also
drops `kovan`'s `gui` and `petir`'s `transfer-fn`, both of which build and
test fine without, and dropping `gui` is desirable headless anyway.

**Work is NOT done on a `quick-test` run.** `cargo quick-test` is for the
edit-compile-check loop. Before reporting work complete, before committing,
and before any hand-off, run the real thing:

```bash
cargo test --workspace --lib --tests --release
```

Reporting a green `quick-test` as if it were a green suite is the failure
mode this rule creates, so it is called out explicitly: **say which of the
two you ran.** "Tests pass (quick-test; long tests not run)" is an honest
report. "Tests pass" after a `quick-test` is not.

**When you write a test that crosses the threshold, gate it in the same
change** — add the feature to the crate's `Cargo.toml` if it has none, and
state the measured runtime in the `ignore` message so the next reader knows
what they are skipping. Do not guess the number; measure it.

**This does not change the TUAS rule above.** Those tests are long *and* must
run in parallel; gating them does not license `--test-threads=1`, and a
timeout is still not a failure.

