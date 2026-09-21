# raffles

**RAFFLES** — **R**isk **A**nalysis **F**ramework **F**or **L**earning &
**E**nsemble **S**imulation.

An independent pure-Rust port of the uncertainty-quantification (UQ) and
risk-analysis core of [RAVEN](https://github.com/idaholab/raven), the
probabilistic risk-analysis, UQ and model-reduction framework developed by
Idaho National Laboratory.

> **⚠️ IMPLEMENTED IN PART, WITH NO HUMAN V&V.** Distributions, samplers,
> sensitivity measures, Bayesian model updating, statistical distances,
> Approximate Bayesian Computation, imprecise probability, model selection,
> fault-tree quantification, graph neural networks and surrogate models all
> carry working, tested code.
> Every one of them is **AI-assisted draft material** under the workspace
> `RESPONSIBLE_USE.md` rules until the maintainer and the crate owner have
> reviewed it. Read "verified" throughout this crate as "checked against a
> reference by an automated test", which is what it is — and do not describe
> any part of it as validated.

> **Intended use:** education, research, capability building and V&V only.
> Despite the name, RAFFLES is **not** for nuclear facility operation, reactor
> control, licensing decisions, probabilistic safety assessment of a real
> facility, safety-critical decision-making, or emergency response. See the
> workspace `RESPONSIBLE_USE.md`.

## Ownership

**This crate belongs to Adolphus Lye.** They chose the RAFFLES backronym, and
the direction, scope and priorities of the crate are theirs to set. Anyone else
— human or AI assistant — working in here is a contributor, not a decision
maker: propose changes of direction to them rather than making them.

## Scope

| Module | What it holds |
|---|---|
| `distributions` | Probability distributions — densities, CDFs, inverse CDFs, analytic moments; continuous and discrete, truncated variants, multivariate |
| `samplers` | Sampling strategies — Monte Carlo, Latin hypercube (RAVEN calls it `Stratified`), grid / full-factorial |
| `sensitivity` | Importance measures from an evaluated sample — Sobol first-order and total indices, Pearson / Spearman / partial correlation |
| `bayesian` | Bayesian model updating — independent priors, Metropolis–Hastings and affine-invariant ensemble moves, TMCMC and TEMCMC with the log-evidence as a by-product, and both published tempering criteria |
| `distance` | Statistical distances between two sample sets — Euclidean on summaries, Bhattacharyya, Hellinger, Jensen–Shannon, Bray–Curtis, 1-Wasserstein (the area metric) |
| `abc` | Approximate Bayesian Computation — three kernels, rejection ABC, and an approximate log-likelihood the transitional samplers consume directly |
| `imprecise` | Imprecise probability — intervals, probability boxes, Clopper–Pearson confidence boxes, coherent-system reliability with or without a dependence assumption |
| `model_selection` | Comparing models by evidence — Bayes factors, posterior model probabilities, the Kass–Raftery scale |
| `scram` | Fault trees, after [SCRAM](https://github.com/rakhimov/scram) — build a tree, generate its minimal cut sets (classical MOCUS, or a **ZBDD** for scale; coherent **and** non-coherent), derive its **prime implicants** where cut sets would be conservative, quantify the top event by rare-event, MCUB, exact inclusion-exclusion **or a BDD**, and rank the basic events by the five standard importance measures. Handles house events; no preprocessor, no XML input |
| `gnn` | Graph neural networks for physics — message-passing topology, the physics-guided bound on message-passing iterations, and (behind `burn`) the network itself |
| `surrogate` | Reduced-order models — polynomial regression, and a `burn`-trained neural regressor behind the `burn` feature. Gaussian processes and sparse-grid polynomial chaos are **not** implemented |

**Out of scope:** physics of any kind, simulation drivers, job scheduling,
input-file/XML parsing, databases, plotting. RAVEN is a whole workflow
application; RAFFLES ports only its statistical core. The caller runs their own
model and hands RAFFLES arrays of numbers.

Which RAVEN capabilities are in, which are out, and in what order they are
approached is written up in the workspace-root scoping document
**`docs/raven-port-scoping.md`**.

**`scram` widens that scope, and was added at the workspace maintainer's
direction rather than the crate owner's** (2026-09-21). Fault-tree
quantification is not part of RAVEN's statistical core, so whether it belongs
in RAFFLES at all is Adolphus Lye's call to confirm or reverse — recorded here
so it is visible rather than absorbed silently.

## Attribution and licensing

| File | What it is |
|---|---|
| `NOTICE` | This crate's provenance record: upstream URL, referenced commit, date accessed, the required Apache-2.0 attribution, and the one-way licence direction |
| `LICENSE-APACHE-RAVEN` | Upstream RAVEN's `LICENSE.txt`, preserved **verbatim** (Apache-2.0) |
| `NOTICE-RAVEN` | Upstream RAVEN's `NOTICE.txt`, preserved **verbatim** — Apache-2.0 section 4(d) requires it to travel with the derivative work |
| `upstream_source/README.md` | Clone command, commit, and the map from RAFFLES modules to upstream paths |

**Upstream:** RAVEN, <https://github.com/idaholab/raven>, branch `devel`,
commit `01216937967c38ee287859270c035c8eca906dc6` (2026-07-14); latest release
at time of access `RAVENv3.2` (2026-03-12). Accessed 2026-08-06. Licensed
**Apache-2.0**, `Copyright 2017 Battelle Energy Alliance, LLC`, produced for
the U.S. Department of Energy Office of Nuclear Energy under Contract
No. DE-AC07-05ID14517.

### Apache-2.0 into GPL-3.0 is ONE-WAY

RAFFLES is **GPL-3.0-only**, the Outram Park workspace default.

Apache-2.0 is one-way compatible with GPLv3. Apache-licensed code may be taken
into a GPLv3 work and the result is governed by GPLv3; GPLv3 code may **not**
be taken into an Apache-2.0 work.

- Code may flow **RAVEN (Apache-2.0) into RAFFLES (GPL-3.0-only)**.
- Code may **not** flow **RAFFLES into RAVEN**.

So: do not contribute RAFFLES code, patches or translated files upstream to
RAVEN, and do not offer RAFFLES code to anyone under Apache-2.0 terms. Once a
translation lands here it is GPLv3 and it stays GPLv3. This constrains the
direction of *code* flow only — reading RAVEN's papers and documentation and
implementing the published algorithms is unaffected.

### Per-file attribution headers

Every RAFFLES file derived from a RAVEN file must carry a header naming the
upstream project, the upstream source file, the version/commit, the copyright
holder and the licence, and that header must survive refactors. The worked
example to copy is in this crate's `CLAUDE.md`.

Note that RAVEN itself vendors third-party **BSD** code (AMSC, from the
University of Utah; and NGL). Files derived from those parts take the BSD
attribution, not the Apache-2.0 one — check which upstream component a file
comes from before writing its header.

### Independence

This is an independent fork. RAFFLES is not the RAVEN project, is not a release
of RAVEN, and is not endorsed by or affiliated with RAVEN, Idaho National
Laboratory, Battelle Energy Alliance, LLC, or the U.S. Department of Energy.
The same holds for the other upstreams: RAFFLES is not SCRAM, is not a release
of SCRAM, and is not endorsed by or affiliated with SCRAM or Olzhas Rakhimov.
See the workspace `TRADEMARKS.md`.

## Provenance beyond RAVEN

RAFFLES started as a RAVEN port and has since taken in work from three other
directions. The licensing is not uniform, and the difference decides what could
be done:

| Source | Licence | What that allowed |
|---|---|---|
| [RAVEN](https://github.com/idaholab/raven) | Apache-2.0 | Code may be ported into this GPL-3.0 crate, one-way, with the attribution header in `CLAUDE.md` |
| [Physics-guided-MPNN](https://github.com/mikelunizar/Physics-guided-MPNN) | GPL-3.0 | Same licence as this workspace, so `gnn::mpnn` **is** a port and carries its attribution header |
| [SCRAM](https://github.com/rakhimov/scram) | GPL-3.0-or-later | Same licence as this crate, so `scram::probability`, `scram::importance` and `scram::fault_tree`'s connective taxonomy **are** ports and carry their attribution headers. `scram::mocus` is **not** — it is the published MOCUS algorithm, verified *against* SCRAM rather than translated from it, and says so |
| Adolphus Lye's `Bayesian-Model-Updating-Tutorials` | GPL-3.0 (LICENSE file) | Portable, same as above; `bayesian::case_studies` **is** a port and carries its header |
| Adolphus Lye's six other repositories (workspace issue #158) | GPL-3.0 **by direct grant** from the author, who is the copyright holder and this crate's owner — stated to the maintainer 2026-09-09, reaffirmed 2026-09-16. No `LICENSE` file in the repositories as of 2026-09-16 | Portable. Nothing has been taken from them so far: the Bayesian, distance, ABC and imprecise modules were written from the published papers, each cited with its DOI |

Two things about that last row, kept apart because they are different claims.

**The licence is settled.** Adolphus Lye owns the copyright in all seven
repositories and has granted GPL-3.0 — the same licence as this crate — so
there is no compatibility question and no one-way constraint of the kind that
applies to the RAVEN port. A code-level port from any of the seven is
permitted, with the attribution header in `CLAUDE.md`.

**The checkability is not.** Six of the seven carry no `LICENSE` file, so a
reader of this repository cannot confirm the grant from the upstream sources
themselves — they have to take this record on trust.
`RESEARCH_INTEGRITY_AND_PROVENANCE.md` expects better than that. The grant is
therefore recorded in `NOTICE` with its date and how it was communicated, and
the real fix — one commit per repository adding a `LICENSE` — is the author's
to make. Tracked as `op-dwqw.1`; full detail in
[`docs/adolphus-uq-port-scoping.md`](../../docs/adolphus-uq-port-scoping.md).

Note the distinction the modules keep: a file **written from a paper** cites
the DOI and carries no "ported from" header, because nothing was ported. A
file **derived from upstream source** carries the header. Do not attach one to
work that did not come from there.

## Design rules

RAVEN is deeply inheritance-based. That structure must not be transcribed into
Rust as trait objects. Per the workspace design rules:

- **Enum dispatch** for every family of distribution, sampler, estimator and
  surrogate — never `Box<dyn Trait>`, `&dyn Trait` or `Arc<dyn Trait>`. A trait
  is still fine as a compiler-enforced contract on the concrete structs.
- **No `Box<T>`** — own by value, or share with `Arc<T>`.
- **No lifetime parameters** on structs, traits or impls.

## Units

RAFFLES quantities are dimensionless by nature — probabilities, quantiles,
variance fractions, correlation coefficients, counts — so `uom` is deliberately
not used. Sample values are plain `f64` in whatever units the caller's model
uses; RAFFLES never interprets them physically. Probabilities and Sobol indices
lie in `[0, 1]`; correlation coefficients lie in `[-1, 1]`.

## Optional features

| Feature | Default | What it adds |
|---|---|---|
| `burn` | off | Neural-network surrogates backed by [burn](https://github.com/tracel-ai/burn), this workspace's replacement for PyTorch. Pulls `burn` in `no_std` + `alloc` mode with the pure-Rust, libm-backed `ndarray` backend — no system BLAS and no GPU, so the Android and wasm builds stay clean. |

```bash
cargo build -p raffles --release --features burn
```

Nothing in `src/surrogate.rs` consumes `burn` yet; the feature wires the
dependency up so that work can start behind a flag. GPU backends (`wgpu`,
`vulkan`, `metal`, `webgpu`) are deliberately out of scope for this first pass.

## Android / Termux

The crate is **Android-clean by construction** and must stay that way. Its
dependencies are `thiserror`, `outram-mc-libs` (the RNG), and — only when the
optional `burn` feature is turned on — `burn` with its `ndarray` backend. Every
one of them is pure Rust: there is no `ndarray-linalg`, no BLAS/LAPACK, no C or
Fortran toolchain, no GUI.

If linear algebra becomes necessary — surrogate fitting, correlated
multivariate sampling — reach first for the pure-Rust `faer` already in the
root `[workspace.dependencies]`. Anything BLAS-backed must be declared under a
`[target.'cfg(not(target_os = "android"))'.dependencies]` table in the same
change, never unconditionally.

Proxy check (all targets, not just the library):

```bash
cargo check --release -p raffles --all-targets --target aarch64-linux-android
```

The authoritative check is still a native build inside Termux.

## Verification requirement

Nothing here is "done" until it is checked against something known
independently of the implementation:

- **Distributions** — analytic mean/variance/skewness, CDF and inverse-CDF
  round trip over the support, published tabulated quantiles.
- **Samplers** — the structural property the method guarantees (one point per
  stratum for Latin hypercube; exact tensor product for grid), moment
  convergence at the expected rate for Monte Carlo, and bit-for-bit
  reproducibility from a fixed seed.
- **Sensitivity estimators** — the closed-form Sobol indices of the **Ishigami
  function**, an additive linear model where first-order indices sum to 1 and
  equal the total indices, and the Sobol g-function for a strongly interacting
  case.
- **Surrogates** — exact reproduction of a polynomial at the matching
  expansion order, plus a published test problem.
- **Fault trees** — upstream **SCRAM built from source and run**, on
  upstream's own input models, end to end: cut sets generated here compared
  set-for-set against the products SCRAM found (438 of 438 across 8 models),
  and the totals and importance factors compared on top. The oracles are committed
  under `reference-data/scram/`; the record, including what it does *not*
  establish, is
  [`docs/scram-port-verification.md`](docs/scram-port-verification.md).

Per the workspace V&V rule, the documentation of each gate must state **both**
the methodology (reference, inputs, tolerances, pass criterion) and the
**results** actually measured (numbers with uncertainty, and the date).

## Build

```bash
cargo check --release -p raffles --lib
cargo test  -p raffles --lib --tests --release
```

Always `--release` for builds and tests, per the workspace rule.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

## License

GPL-3.0-only. See the workspace root `LICENSE`, and this crate's `NOTICE`,
`LICENSE-APACHE-RAVEN` and `NOTICE-RAVEN` for upstream provenance and the
required Apache-2.0 attribution.
