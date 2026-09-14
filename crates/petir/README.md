# PETIR

**P**olynomials, **E**quations, **T**ransforms, **I**ntegration and **R**oots —
the OUTRAM PARK workspace's core numerics library, and its first `no_std` crate.

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified
> and untrusted** unless a specific verification & validation (V&V) case
> demonstrates otherwise. V&V cases are human-reviewed and are intended for
> journal / arXiv publication — that is the trust workflow. See the workspace
> `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear
> facility operation, reactor control, safety-critical, or licensing decisions.

Named for Petir on the Bukit Panjang LRT, following the workspace's
Singapore-place-name convention. *Petir* is Malay for lightning, which suits a
library of iterative routines better than it has any right to.

## What it is

A `no_std` port of parts of the [GNU Scientific Library](https://www.gnu.org/software/gsl/),
together with numerical kernels lifted from elsewhere in this workspace and
transfer-function blocks derived from the GNU Octave control package.

It exists because there was no core maths crate. `outram-foam-basic-lib` had
become one by accretion — roughly 27k lines of general numerics under an
OpenFOAM-branded name — and two consumers that need those numerics
(`njoy-outram-park-fork` and `raffles`) are deliberately dependency-lean and
cannot take a finite-volume CFD library to get them.

## The governing rule: ported, never invented

> Every numerical routine here is **ported from an upstream library that has its
> own test suite**, never written from a textbook formula.

This is the crate's constraint, not a style preference. A kernel written from a
formula has no lineage and no reference suite, so a reviewer has nothing to diff
it against. A port can be opened next to its upstream file and read line for
line, and it inherits that upstream's V&V.

Because a rule is only worth having if it is checkable, **every module declares
which of three lineages it belongs to**:

| Lineage | Meaning |
|---|---|
| **Ported** | Read from a vendored upstream at a recorded commit; the upstream file and line are named in the doc comment |
| **Lifted verbatim** | Copied byte-for-byte from an in-workspace crate, with only mechanical `std` → `core`/`alloc` edits, each listed in the file's PROVENANCE block |
| **Delegated** | Forwarded to a dependency that already implements it to a known standard |

`tests/verbatim_provenance.rs` enforces the middle row mechanically: it
re-applies the documented substitutions to each source and compares, so a lift
that drifts from its origin fails the build rather than rotting quietly.

## What is here today

| Module | Lineage | Covers |
|---|---|---|
| `linalg` | lifted + ported | Dense `n×n` Crout LU with scaled partial pivoting, determinant, log-determinant, explicit inverse, level-1 BLAS |
| `poly` | lifted + ported | Horner evaluation and derivatives, Newton divided differences, exact linear / quadratic / cubic root finders |
| `specfunc` | ported + lifted + delegated | Error-function family including the scaled `erfcx`, gamma family with GSL's Padé branches at the zeros, incomplete gamma and its inverse |
| `transfer_fn` | ported | Continuous and discrete SISO transfer functions, `c2d` / `d2c`, and the O(1) fixed-state recurrence blocks |
| `real` | — | The `no_std` float-math shim |
| `scalar` | lifted | Guard constants and machine epsilons |

**Not here yet**, and tracked as beads rather than stubbed: quadrature,
one-dimensional root finding, minimisation, numerical differentiation, ODE
integration, interpolation, and Chebyshev fitting. An empty module that looks
like an API is worse than an absent one.

## `no_std` is the contract

The crate is `#![no_std]` unconditionally. There is no `std` feature to turn on,
because a numerical kernel that only sometimes builds for a microcontroller is
one refactor away from not building for one at all.

Verified on every commit:

```bash
cargo build -p petir --target thumbv7em-none-eabihf   # bare-metal Cortex-M4F
cargo build -p petir --target wasm32-unknown-unknown
cargo check -p petir --all-targets --target aarch64-linux-android
cargo build -p petir --no-default-features            # numerics only, no uom
```

Consequences that are load-bearing rather than incidental:

- **`alloc` is required**, not bare `no_std` — a runtime-sized coefficient array
  is meaningless without a heap.
- **`libm` supplies the transcendentals.** `core` has `abs`, `min`, `max` and
  `signum` but not `sqrt`, `exp`, `ln` or `powf`. `real::Real` restores them as
  methods, so ported bodies need no rewriting. One fixed implementation also
  means results are bit-identical across platforms.
- **Errors are returned, never fatal.** GSL's `gsl_error` calls a handler that
  aborts the process by default; `PetirError` exists instead, and each variant
  names the GSL code it stands in for.
- **Callbacks are generic `F: Fn(f64) -> f64`**, never `dyn Fn`.

## Units: bare `f64`, with one deliberate exception

The numerics are dimensionless. `linalg`, `poly`, `specfunc` and everything that
follows them take and return bare `f64` — `uom` belongs at the physics crates'
API boundaries, not inside a polynomial evaluator.

The exception is `transfer_fn`, where a sample time genuinely *is* a `Time` and a
dimensionless signal *is* a `Ratio` in the blocks being ported. It sits behind
the default-on `transfer-fn` feature, so a caller who wants only the numerics
gets no `uom` in their dependency graph at all.

Even there, **polynomial coefficients stay bare `f64`**: the coefficient of
$s^{k}$ carries units of $s^{k}$, so they differ term by term and no single `uom`
quantity can type a coefficient vector.

### A `no_std` `uom` gotcha worth knowing

`uom` gates `Quantity::sqrt`, `exp` and the rest of the floating-point family on
**its own `std` feature** — with `std` off it aliases `uom::num::Float` to
`num_traits::FloatCore`, which has none of them. In a `no_std` build those
methods simply do not exist, and a call fails with `no method named 'sqrt' found
for struct Quantity`, an error that points at `uom` and gives no hint about
feature flags.

Adding `num-traits` with its `libm` feature does **not** fix it: `uom` never
looks at num-traits' features, only at its own. `transfer_fn::ratio_ext`
restores `sqrt` and `exp` for dimensionless `Ratio` instead.

## Two upstreams, two conventions

GSL writes polynomial coefficients **ascending** (`c[0]` is the constant term);
Octave writes them **descending**. PETIR keeps both, each in its own namespace,
so a ported routine reads like its source. Every function that takes
coefficients says which order it means — reversing them silently reverses the
polynomial, and it is the one place in this crate where a careless call compiles
and is wrong.

## Licence

GPL-3.0-only. GSL is GPL-3.0-**or-later**, verified on 2026-09-14 by reading its
per-file headers — not its `COPYING`, whose "version 3 or later" line belongs to
the *How to Apply These Terms* appendix and is boilerplate rather than GSL's own
grant. Porting GPL source creates a derivative work, so the flow is **one-way**:
code here cannot go back upstream under another licence.

Full provenance — upstream commit SHA, `COPYING` digest, per-module copyright
holders — is in [`NOTICE`](NOTICE) and
[`upstream_source/README.md`](upstream_source/README.md).

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
