# raffles

**RAFFLES** — **R**isk **A**nalysis **F**ramework **F**or **L**earning &
**E**nsemble **S**imulation.

An independent pure-Rust port of the uncertainty-quantification (UQ) and
risk-analysis core of [RAVEN](https://github.com/idaholab/raven), the
probabilistic risk-analysis, UQ and model-reduction framework developed by
Idaho National Laboratory.

> **⚠️ SCAFFOLD ONLY — nothing is implemented.** This crate currently contains
> module boundaries, licensing and provenance, and nothing else. No
> distribution, no sampler, no sensitivity estimator, no surrogate. There has
> been no human V&V. Do not describe any part of it as working, verified or
> validated.

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
| `surrogate` | Reduced-order models — polynomial chaos, Gaussian process, regression. **Placeholder; no work scheduled** |

**Out of scope:** physics of any kind, simulation drivers, job scheduling,
input-file/XML parsing, databases, plotting. RAVEN is a whole workflow
application; RAFFLES ports only its statistical core. The caller runs their own
model and hands RAFFLES arrays of numbers.

Which RAVEN capabilities are in, which are out, and in what order they are
approached is written up in the workspace-root scoping document
**`docs/raven-port-scoping.md`**.

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
See the workspace `TRADEMARKS.md`.

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

## Android / Termux

The crate is **Android-clean by construction** and must stay that way. Its only
dependency is `thiserror`. There is no `ndarray-linalg`, no BLAS/LAPACK, no C
or Fortran toolchain, no GUI.

If linear algebra becomes necessary — surrogate fitting, correlated
multivariate sampling — reach first for the pure-Rust `faer` already in the
root `[workspace.dependencies]`. Anything BLAS-backed must be declared under a
`[target.'cfg(not(target_os = "android"))'.dependencies]` table in the same
change, never unconditionally.

Proxy check (all targets, not just the library):

```bash
cargo check -p raffles --all-targets --target aarch64-linux-android
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

Per the workspace V&V rule, the documentation of each gate must state **both**
the methodology (reference, inputs, tolerances, pass criterion) and the
**results** actually measured (numbers with uncertainty, and the date).

## Build

```bash
cargo check -p raffles --lib
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
