# CLAUDE.md — raffles

Crate-specific guidance for Claude Code and other AI assistants working in
`crates/raffles`. The workspace-root `CLAUDE.md` still binds in full — this
file adds to it, and relaxes nothing.

**RAFFLES** — **R**isk **A**nalysis **F**ramework **F**or **L**earning &
**E**nsemble **S**imulation. An independent pure-Rust port of the
uncertainty-quantification and risk-analysis core of
[RAVEN](https://github.com/idaholab/raven) (Idaho National Laboratory,
Apache-2.0).

**Current state: implemented in part, with no human V&V.** Distributions,
samplers, sensitivity, Bayesian model updating, distances, ABC, imprecise
probability, model selection, fault-tree quantification, GNNs and surrogates
all carry working, tested code. All of it is AI-assisted draft material until
the maintainer and the crate owner have reviewed it, so do not describe any
part of it as validated, and read "verified" as "checked against a reference by
an automated test".

---

## Ownership — Adolphus Lye

**This crate belongs to Adolphus Lye**, a colleague of the workspace
maintainer. They chose the RAFFLES backronym, and **direction, scope and
priorities for this crate are theirs to set.**

What that means in practice:

- A change of direction — adding or dropping a module, changing what the crate
  is for, restructuring the public API, taking on optimisation or the
  simulation-driver layer that is currently out of scope — is **their call, not
  yours and not another agent's**. Propose it to them; do not decide it.
- If a task you are given conflicts with what this file or the README records
  as the crate's scope, say so and ask, rather than quietly widening the scope.
- Attribute their ownership accurately in anything you write about the crate.
- **Use they/them for Adolphus Lye.** Their pronouns have not been stated, so
  singular *they* is the correct default here — do not guess otherwise.

Filling in a module the crate already declares, to the design rules below, is
ordinary contribution and does not need a fresh decision.

---

## Upstream provenance

| | |
|---|---|
| Project | RAVEN (Risk Analysis Virtual ENvironment) |
| Developer | Idaho National Laboratory (INL) |
| Repository | <https://github.com/idaholab/raven> |
| Licence | **Apache-2.0** |
| Copyright | `Copyright 2017 Battelle Energy Alliance, LLC` |
| Branch / commit referenced | `devel` @ `01216937967c38ee287859270c035c8eca906dc6` (2026-07-14) |
| Latest release at time of access | `RAVENv3.2` (2026-03-12) |
| Date accessed | 2026-08-06 |

Verbatim upstream files preserved in this crate: `LICENSE-APACHE-RAVEN`
(upstream `LICENSE.txt`) and `NOTICE-RAVEN` (upstream `NOTICE.txt`). Both were
fetched, not retyped. Do not reformat, truncate or "tidy" them.

The crate `NOTICE` is the authoritative provenance record.
`upstream_source/README.md` holds the clone command and the map from RAFFLES
modules to upstream paths.

### Apache-2.0 into GPL-3.0 is ONE-WAY — the rule you must not get wrong

RAFFLES is **GPL-3.0-only**. Apache-2.0 is one-way compatible with GPLv3:

- Code may flow **RAVEN (Apache-2.0) into RAFFLES (GPL-3.0-only)**.
- Code may **not** flow **RAFFLES (GPL-3.0-only) into RAVEN (Apache-2.0)**.

Never contribute RAFFLES code, patches or translated files upstream to RAVEN,
never open a pull request against `idaholab/raven` out of this workspace, and
never offer RAFFLES code to anyone under Apache-2.0 terms. Once a translation
lands here it is GPLv3 and stays GPLv3. Relicensing would require every RAFFLES
copyright holder's agreement — which no AI assistant may arrange, and no
contributor may arrange unilaterally.

This restricts the direction of *code* flow only. Reading RAVEN's papers,
theory manual and documentation and implementing the published algorithms is
unaffected — and where the algorithm is published, an independent
implementation from the paper is usually the better route anyway.

### SCRAM is a second upstream, and it is NOT Apache-2.0

`src/scram/` is ported from **[SCRAM](https://github.com/rakhimov/scram)**
(Olzhas Rakhimov), commit `b85b78940de38996eeffec54d946824bd4280a1c`,
accessed 2026-09-21. SCRAM is **GPL-3.0-or-later** — the same licence family
as this crate — so it carries **none** of the one-way constraint the RAVEN
grant does, and the Apache-2.0 header template below is **wrong** for those
files. Use the GPL header the existing `src/scram/` files carry.

**Half of `src/scram/` is a port and half deliberately is not, and the
difference is load-bearing.** Ported, with attribution headers:
`probability.rs`, `importance.rs`, and `fault_tree.rs`'s `Connective`
taxonomy. **Not** ports, each saying so in its own doc instead:

- `mocus.rs` — upstream's MOCUS drives a ZBDD over a preprocessed Boolean
  graph (`zbdd` + `pdag` + `preprocessor` + `bdd` = 9,076 lines), none of it
  ported. This is the classical top-down expansion from the literature.
- The exact top-event probability — inclusion-exclusion, not a BDD traversal.
- The Birnbaum factor — its definition, not a BDD derivative.
- The rest of `fault_tree.rs` — a plain indexed structure, not upstream's
  XML-driven `Initializer`/`Model`/`Formula`.

**Do not retrofit an attribution header onto any of those.** A header is a
statement about where a file came from, and the whole value of these four is
that they are *independent* of upstream: two unrelated algorithms agreeing is
evidence, a translation agreeing with its original is much weaker. That is the
same rule the paper-derived Bayesian modules follow.

Absent: ZBDD, the preprocessor, prime implicants, XML input, event trees,
alignments, CCF groups, house events.

~~BDD~~ **CORRECTED 2026-09-21** — `src/scram/bdd.rs` landed the same day: a
Bryant-style diagram built straight from the tree, with upstream's probability
recurrence ported verbatim. It removed the largest limitation the port had —
exact probability no longer needs cut sets, so it is no longer capped at 20 of
them. Verified against SCRAM's own BDD on 10 models, including `Aralia/das9601`
(288 gates, non-coherent, 386,261 nodes) which `mocus` cannot touch.

~~complement elimination (so **non-coherent trees are refused, not
approximated**)~~ **CORRECTED 2026-09-21** — complement elimination landed the
same day. `mocus` now expands negated gates through their De Morgan duals,
drops contradictory partial sets, deletes the complements and re-minimises,
matching what upstream's `Zbdd::EliminateComplement` does. **The answers mean
something different, though**: minimal cut sets of a non-coherent tree are
conservative, so quantifying them is an upper bound, not the probability.
Measured on the fixture's small non-coherent model, ours is `0.622` against
SCRAM's exact BDD value of `0.5032` — `+23.6 %`, and correct. Do not "fix"
that gap.

~~upstream's probability cut-off on products~~ **CORRECTED 2026-09-21** — that
was listed as absent, wrongly. SCRAM 0.16.2 stores and validates
`Settings::cut_off_` but **never reads it**: the getter has no callers, and
`--cut-off 0.5` on a model whose top-event probability is `1.17e-3` discards
none of its 392 products. There is nothing to port, and truncating by cut-set
order only is not a divergence. Do not "fix" this by implementing one.

Verification record: [`docs/scram-port-verification.md`](docs/scram-port-verification.md),
oracle in `reference-data/scram/`. Upstream was **built and run** — nothing in
that fixture was reasoned out from reading SCRAM's source.

### Third-party BSD code inside RAVEN

RAVEN's own `NOTICE.txt` discloses vendored third-party code under separate BSD
licences:

- **AMSC** — Copyright 2014 University of Utah, Scientific Computing and
  Imaging Institute (3-clause BSD).
- **NGL** — Copyright 2012 Carlos D. Correa (2-clause BSD).

Both are GPLv3-compatible, but **neither is covered by the Apache-2.0 grant**.
They sit in the topological-decomposition / Morse-Smale area, adjacent to
sensitivity analysis. **Check which upstream component a file comes from before
writing its header**, and use the BSD attribution for those. Read
`NOTICE-RAVEN` for the exact text.

---

## Per-file attribution header — the convention, with a worked example

**Every file in this crate that is derived from an upstream RAVEN file must
carry an attribution header naming the upstream project, the upstream source
file, the version/commit it was taken from, the copyright holder, and the
licence.** Put it at the very top of the file, above the `//!` module doc. Do
not strip it during refactors, do not move it into a separate document, and do
not summarise it away.

Copy this and edit the fields:

```rust
// ---------------------------------------------------------------------------
// Ported from RAVEN (Risk Analysis Virtual ENvironment).
//
//   Upstream project: RAVEN — Idaho National Laboratory
//   Upstream repo:    https://github.com/idaholab/raven
//   Upstream file:    ravenframework/Samplers/MonteCarlo.py
//   Upstream commit:  01216937967c38ee287859270c035c8eca906dc6  (branch devel)
//   Accessed:         2026-08-06
//
//   Copyright 2017 Battelle Energy Alliance, LLC
//   Licensed under the Apache License, Version 2.0 (the "License");
//   you may not use this file except in compliance with the License.
//   You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
//   Unless required by applicable law or agreed to in writing, software
//   distributed under the License is distributed on an "AS IS" BASIS,
//   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//   See the License for the specific language governing permissions and
//   limitations under the License.
//
// This Rust translation is part of RAFFLES / Outram Park and is distributed
// under GPL-3.0-only. Apache-2.0 -> GPLv3 is a ONE-WAY relicensing: this file
// may NOT be contributed back to RAVEN or redistributed under Apache-2.0.
//
// Translation notes: <what changed and why — e.g. RAVEN's Sampler class
// hierarchy is flattened into the `Sampler` enum; NumPy array operations are
// expressed over slices; RAVEN's XML input handling is out of scope.>
// ---------------------------------------------------------------------------
```

Rules for filling it in:

- **`Upstream file`** is a real path in the upstream repo. Verify it exists at
  the commit you name; do not guess a plausible-looking path.
- **`Upstream commit`** is the commit the translation was actually read from.
  If you work from a newer clone, update the commit *and* the `Accessed` date
  in the header, and say so in the crate `NOTICE` if the reference point for
  the whole crate moved.
- **The licence block is Apache-2.0** for files derived from RAVEN proper. For
  a file derived from the vendored **AMSC** or **NGL** code, replace it with
  that component's BSD text and copyright holder instead — see above.
- **`Translation notes` is not optional.** Say what you changed structurally
  and what you deliberately did not port. This is the honest record of how far
  the translation goes.
- **A file written from a published paper rather than from upstream source is
  not a port.** Do not give it this header — cite the paper in the module doc
  comment instead, and say plainly that it is an independent implementation.

---

## Design: RAVEN's inheritance must become Rust enums

This is the single biggest structural decision in the port, and the workspace
rule is hard.

RAVEN is deeply inheritance-based. Its samplers descend through
`Sampler` -> `ForwardSampler` -> `MonteCarlo` / `Grid` / `Stratified` / …, its
distributions through a `Distribution` base into `Uniform`, `Normal`,
`Gamma`, …, and dispatch happens through Python's dynamic method lookup plus a
`Factory` that instantiates classes by name from XML. **None of that maps to
`Box<dyn Trait>` in this workspace.**

Per the root `CLAUDE.md` "Rust design rules":

- **No trait objects** — no `Box<dyn Trait>`, `&dyn Trait`, `Arc<dyn Trait>`
  for dispatch. Each family becomes **one enum with a variant per concrete
  model**, dispatched by `match`.
- **No `Box<T>`** — own by value, or share with `Arc<T>`.
- **No lifetime parameters** on structs, traits or impls. A sampler owns its
  distributions (or shares them via `Arc`); it does not borrow them.

A trait is still useful as a **compiler-enforced contract** on each concrete
struct — the compiler then checks that every distribution really does provide a
CDF and an inverse CDF. It is just not the dispatch mechanism. The pattern:

```rust
/// Compiler-enforced contract every concrete distribution must satisfy.
pub trait ContinuousDistribution {
    /// Probability density at `x`.
    fn pdf(&self, x: f64) -> f64;
    /// Cumulative probability at `x`; result lies in `[0, 1]`.
    fn cdf(&self, x: f64) -> f64;
    /// Inverse CDF (percent-point function); `p` must lie in `[0, 1]`.
    fn ppf(&self, p: f64) -> crate::Result<f64>;
}

/// Dispatches without `Box` or `dyn`.
pub enum Distribution {
    Uniform(UniformDistribution),
    Normal(NormalDistribution),
    LogNormal(LogNormalDistribution),
    // adding a variant here is a compile error at every `match` that forgot it
}

impl Distribution {
    /// Inverse CDF of whichever distribution this is.
    pub fn ppf(&self, p: f64) -> crate::Result<f64> {
        match self {
            Self::Uniform(d)   => d.ppf(p),
            Self::Normal(d)    => d.ppf(p),
            Self::LogNormal(d) => d.ppf(p),
        }
    }
}
```

Why it matters here specifically: adding a distribution or a sampler is exactly
the change most likely to be made later, and the enum makes every site that
forgot to handle it a compile error rather than a silent runtime fallthrough.

Two related translation notes:

- **Drop RAVEN's `Factory` / XML-name-based instantiation entirely.** RAFFLES
  callers construct the enum variant they want in Rust. Input-file parsing is
  out of scope for this crate.
- **Do not reproduce RAVEN's mutable "handler" objects.** Prefer functions that
  take data and return owned results.

---

## Verification: nothing is done without a verification path

**Every ported distribution and every estimator needs a verification path
before it counts as done.** A module that computes numbers nobody has checked
is a draft, and must be described as one.

The reference for each family:

| What | Verify against |
|---|---|
| Continuous distribution | Analytic mean / variance / skewness; CDF and inverse-CDF round trip across the support; published tabulated quantiles |
| Discrete distribution | Exact analytic moments; probability mass summing to 1 over the support |
| Truncated distribution | Renormalisation — mass over the truncated support integrates to 1, and moments match the closed form for the truncated case |
| Monte Carlo sampler | Sample moments converge to the analytic moments at the expected `1/sqrt(N)` rate; a fixed seed reproduces a design bit-for-bit |
| Latin hypercube sampler | Exactly one point per equiprobable stratum per variable; marginals match the requested distributions |
| Grid sampler | Point count and coordinates match the tensor product exactly |
| Sobol indices | The **Ishigami function** — closed-form first-order and total indices at the conventional parameters. Plus an additive linear model (first-order indices sum to 1 and equal the total indices) and the Sobol g-function for a strongly interacting case |
| Correlation measures | A construction with a known correlation matrix; and a monotone non-linear transform of it, where Spearman is preserved and Pearson is not |
| Surrogate | Exact reproduction of a polynomial at the matching expansion order; a published test problem with reported error metrics |
| Fault trees | Upstream **SCRAM built from source and run** on upstream's own input models — cut sets compared set-for-set against SCRAM's products, then totals in all three modes and all five importance factors for every basic event. Fixtures in `reference-data/scram/` |

**Document methodology AND results.** Per the workspace V&V rule, a test whose
docs say only what it does is incomplete. State the reference, the inputs,
sample size, tolerances and pass criterion — and the numbers actually measured,
with their sampling uncertainty and the date they were taken. Never write down
a result that was not produced by running the check.

Note the distinction the workspace draws: these are **verification** gates
("is it implemented correctly?"). Validation ("does it represent reality well
enough?") is a separate question and is not answered by any of the above.

---

## Android / Termux

The crate is Android-clean, and stays that way. Its dependencies are
`thiserror`, `outram-mc-libs` (the RNG — see below), and the optional `burn`
(see "Machine learning" below); all three build for `aarch64-linux-android` —
`cargo check -p raffles --all-targets --features burn --target
aarch64-linux-android` was clean on 2026-09-16 (burn 0.21.0, rustc 1.94.1).

- **Never** add `ndarray-linalg`, or anything needing system BLAS/LAPACK, a C
  or Fortran toolchain, or windowing GUI, as an unconditional dependency.
- Surrogate fitting and correlated multivariate sampling are where this
  temptation will appear. Reach first for the pure-Rust **`faer`** already in
  the root `[workspace.dependencies]`.
- Examples, tests and benches are **not** exempt — a native Termux build
  compiles them all.

### Follow `outram-mc-libs`'s gating conventions — do not invent a third

If RAFFLES ever needs something Android-hostile, gate it **exactly the way
`outram-mc-libs` already does**, in the same change. Read that crate's
`Cargo.toml` and `src/gpu/` for the worked pattern; do not devise a new one:

- Target-conditional dependency tables —
  `[target.'cfg(not(target_os = "android"))'.dependencies]` (that is how
  `outram-mc-libs` keeps `wgpu` off Android) and the matching
  `…'.dev-dependencies]` for dev-only tools (how it keeps `naga` off).
- `#[cfg(not(target_os = "android"))]` on the items that use them, with a
  CPU-only shim on the Android side so the library still builds headless.
- **Android stub `main`** in any example or binary, under
  `#[cfg(target_os = "android")]`, since a blanked file gives "main function
  not found".

Note the workspace rule while you are here: Android's `target_os` is
**`"android"`, not `"linux"`**, so a `cfg(target_os = "linux")` gate does
**not** mean "not Android" — never rely on one to exclude Android.

### RNG — reuse, do not add or hand-roll one

**Sampling draws from `outram_mc_libs::rng::lcg`** (OpenMC's 64-bit LCG port),
not from a `rand` crate and not from a PRNG written inside RAFFLES. Whether the
workspace should take a general `rand` dependency is an open maintainer
question (`docs/raven-port-scoping.md` section 10, question 1) and is not a
port agent's call. Seeding stays **explicit** — every `generate` takes a
`master_seed`, and independent dimensions/replicates get non-overlapping
streams via `future_seed` jump-ahead. See `src/samplers.rs`.

**Resolved upstream defect — the warning that used to sit here is obsolete.**
`outram_mc_libs::rng::lcg::init_seed` was wrong: it added where OpenMC
multiplies, so consecutive `id`s landed one LCG *step* apart instead of one
*stride*, making per-stream derivation produce near-perfectly correlated
streams. **Fixed in `op-rbo`** — it now matches
`openmc/src/random_lcg.cpp:60`, and is safe to use.

`src/samplers.rs` still calls `future_seed` directly with explicit stride
arithmetic. That is now a *preference*, not a workaround: it makes the stride
widening visible at the call site when a design needs more draws than
`DEFAULT_STRIDE`. Either route is correct today.

**Note the generator's output changed** (`op-jis`): `prn` now applies OpenMC's
PCG-RXS-M-XS output permutation instead of returning raw state bits, which fixed
a measured lattice defect — successive pairs previously occupied 1 of 1024 bins
along the dual-lattice normal. RAFFLES' statistical gates still pass, but any
*recorded* number in `src/samplers.rs` doc comments that was measured before
that change is stale and needs re-measuring before it is cited

Proxy check, all targets:

```bash
cargo check --release -p raffles --all-targets --target aarch64-linux-android
```

The authoritative check is a native build inside Termux.

---

## Machine learning — `burn` replaces PyTorch, and is optional here

Upstream RAVEN backs its machine-learning surrogates with **PyTorch**. The
Rust replacement for PyTorch in this workspace is **`burn`** (tracel-ai,
MIT OR Apache-2.0 — permissive into GPL-3.0, one-way, like RAVEN itself), per
maintainer direction on 2026-09-16: reach for `burn` wherever the upstream
reaches for PyTorch, as far as `burn` will go. Do not add a second ML
framework, and do not hand-roll a tensor/autodiff layer alongside it — the same
reasoning as the RNG rule above.

**It is optional and off by default.** `burn` is declared once in the root
`[workspace.dependencies]` with `default-features = false`, and RAFFLES takes
it as `burn = { workspace = true, optional = true }` behind its own `burn`
feature:

```bash
cargo build -p raffles --release --features burn
```

- **`no_std` + `alloc`.** That is what `default-features = false` buys, and it
  is deliberate: `burn` runs on `core` + `alloc`, which is all a tensor library
  needs, and it keeps the Android/Termux and wasm builds clean. `alloc` is
  implicit in that mode (`burn` declares `extern crate alloc` itself — there is
  no `alloc` feature to name). A `std` binary links a `no_std` `burn` happily,
  so do not turn the defaults back on for convenience; the `std` feature is
  where `burn`'s dataset/network/train/sqlite machinery lives.
- **Backend: `ndarray` only, for now.** The crate's `burn` feature enables
  `burn/ndarray` — the one backend that is pure Rust and libm-backed, with no
  system BLAS and no C/Fortran toolchain. Never swap it for `tch`, `candle`,
  `cuda`, `rocm` or any `blas-*` feature: every one of those violates the
  Android rule above.
- **GPU is out of scope for this first pass** (maintainer direction,
  2026-09-16). When `wgpu`/`vulkan`/`metal`/`webgpu` is wanted, it goes behind
  its own feature under `cfg(not(target_os = "android"))`, following
  `outram-mc-libs`'s gating convention — not into the default build.
- **Three modules consume it:** `surrogate::neural` (a feed-forward regressor),
  `gnn::mpnn` (the message-passing network) and `gnn::training`. Everything
  else in the crate builds without it, which is why it stays off by default.
  `autodiff` is not separable from `ndarray` here — training needs gradients,
  and a `burn` feature that could only run a forward pass would be a trap.
  These are subject to the same verification requirement as everything else —
  a fitted network that reproduces its training data is not a verified
  surrogate.

Verified clean on 2026-09-16 with `burn` 0.21.0 and rustc 1.94.1 (warnings in
the output came from `outram-mc-libs`/`njoy-outram-park-fork` and predate this):

```bash
cargo check --release -p raffles --lib         --features burn
cargo check --release -p raffles --all-targets --features burn --target aarch64-linux-android
cargo check --release -p raffles --lib         --features burn --target wasm32-unknown-unknown
```

---

## Scope boundaries (do not widen without the owner's say-so)

**In scope:** probability distributions, sampling strategies, sensitivity
measures, surrogate models — the statistical core. Plus, since 2026-09-21,
fault-tree quantification in `src/scram/` (see below).

**`src/scram/` was added at the workspace maintainer's direction, not the crate
owner's.** It is a port of SCRAM's quantification layer and it widens this
crate past the RAVEN-derived statistical core it was scoped to. That is a
direction call, and by the ownership rule above it is **Adolphus Lye's to
confirm or reverse** — it is recorded here so it is visible to them rather than
absorbed silently. Do not build further on it, or extend it toward cut-set
generation, without checking with them first.

**Out of scope:** physics of any kind; simulation drivers, job scheduling and
run-directory management; input-file / XML parsing; databases; plotting and
reporting; RAVEN's optimisers; adaptive / model-in-the-loop samplers (they need
the model-evaluation loop this crate deliberately does not own).

The caller runs their own model and hands RAFFLES arrays of numbers.

The full port scope — which RAVEN capabilities are in, which are out, and in
what order — lives in the workspace-root **`docs/raven-port-scoping.md`**.

---

## Build and test

```bash
cargo check --release -p raffles --lib
cargo test  -p raffles --lib --tests --release
```

Always `--release` for builds and tests, per the workspace rule.

The optional `burn` feature is **not** in the default build, so add it
explicitly when the work touches the surrogate path:

```bash
cargo check --release -p raffles --lib --features burn
cargo test  -p raffles --lib --tests --release --features burn
```

---

## Intended use

Education, research, capability building and V&V only. Despite the "risk
analysis" name, RAFFLES is **not** for nuclear facility operation, reactor
control, licensing decisions, probabilistic safety assessment of a real
facility, safety-critical decision-making, or emergency response. Do not frame
outputs, examples or docs as authoritative for any of those. See the workspace
`RESPONSIBLE_USE.md`.
