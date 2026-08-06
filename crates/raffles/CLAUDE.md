# CLAUDE.md — raffles

Crate-specific guidance for Claude Code and other AI assistants working in
`crates/raffles`. The workspace-root `CLAUDE.md` still binds in full — this
file adds to it, and relaxes nothing.

**RAFFLES** — **R**isk **A**nalysis **F**ramework **F**or **L**earning &
**E**nsemble **S**imulation. An independent pure-Rust port of the
uncertainty-quantification and risk-analysis core of
[RAVEN](https://github.com/idaholab/raven) (Idaho National Laboratory,
Apache-2.0).

**Current state: scaffold.** Module boundaries, licensing and provenance only.
No distribution, sampler, estimator or surrogate exists. Do not describe any
part of this crate as working, verified or validated.

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

The crate is Android-clean by construction — its only dependency is
`thiserror`. Keep it that way.

- **Never** add `ndarray-linalg`, or anything needing system BLAS/LAPACK, a C
  or Fortran toolchain, or windowing GUI, as an unconditional dependency.
- Surrogate fitting and correlated multivariate sampling are where this
  temptation will appear. Reach first for the pure-Rust **`faer`** already in
  the root `[workspace.dependencies]`.
- If something BLAS-backed is genuinely unavoidable, declare it under
  `[target.'cfg(not(target_os = "android"))'.dependencies]` **in the same
  change**, and note it in the README.
- Examples, tests and benches are **not** exempt — a native Termux build
  compiles them all.
- Sampling will need an RNG. Add it to the **root** `[workspace.dependencies]`
  (single source of truth), pick a pure-Rust one, and require explicit seeding
  so designs are reproducible.

Proxy check, all targets:

```bash
cargo check -p raffles --all-targets --target aarch64-linux-android
```

The authoritative check is a native build inside Termux.

---

## Scope boundaries (do not widen without the owner's say-so)

**In scope:** probability distributions, sampling strategies, sensitivity
measures, surrogate models — the statistical core.

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
cargo check -p raffles --lib
cargo test  -p raffles --lib --tests --release
```

Always `--release` for builds and tests, per the workspace rule.

---

## Intended use

Education, research, capability building and V&V only. Despite the "risk
analysis" name, RAFFLES is **not** for nuclear facility operation, reactor
control, licensing decisions, probabilistic safety assessment of a real
facility, safety-critical decision-making, or emergency response. Do not frame
outputs, examples or docs as authoritative for any of those. See the workspace
`RESPONSIBLE_USE.md`.
