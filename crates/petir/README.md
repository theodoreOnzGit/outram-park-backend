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
| `cheb`, `cheb_slice` | ported | **The whole of GSL's `cheb/`** — interpolation at the Gauss nodes, least-squares fitting at arbitrary points, Clenshaw evaluation with an error estimate, precision-mode evaluation, series derivative and integral, and borrowed-slice evaluators for both coefficient conventions |
| `deriv` | ported | Numerical differentiation: central, forward and backward rules with automatic step refinement and an error estimate |
| `integration` | ported | Adaptive Gauss-Kronrod quadrature (QUADPACK): six rules and the `qag` adaptive driver (GSL); plus non-adaptive **Gauss-Legendre** to order 30 and **Newton-Cotes** (`peroxide`) |
| `interp` | ported | Interpolation of tabulated data: linear and natural cubic spline, with derivatives |
| `linalg` | lifted + ported | Dense `n×n` Crout LU with scaled partial pivoting, determinant, log-determinant, explicit inverse, level-1 BLAS, symmetric tridiagonal solve, and rectangular Householder **QR with least-squares solve** |
| `min` | ported | One-dimensional minimisation over a bracketing triple: golden section and Brent |
| `ode` | ported | Initial-value ODE integration: RK4, embedded RKF45, and an adaptive driver |
| `poly` | lifted + ported | Horner evaluation and derivatives, Newton divided differences, exact linear / quadratic / cubic root finders (OpenFOAM); closed-form **quartic** and biquadratic, and **all roots of any degree** by companion-matrix eigenvalues (the `roots` crate); polynomial **algebra** — multiply, long-divide, translate — with Lagrange interpolation and the **Legendre / Chebyshev / Hermite / Bessel** families (`peroxide`). See Prior art |
| `roots` | ported | Bracketing (bisection, false position, Brent) and derivative-based (Newton, secant, Steffenson) root finders, with GSL's three convergence tests |
| `specfunc`, `expint`, `gamma_inc` | ported + lifted + delegated | Error-function family including the scaled `erfcx`, gamma family with GSL's Padé branches at the zeros, incomplete gamma and its inverse, exponential integral `E_1` |
| `transfer_fn` | ported | Continuous and discrete SISO transfer functions, `c2d` / `d2c`, and the O(1) fixed-state recurrence blocks |
| `fast_exp`, `fast_log`, `fast_pow` | ported | ARM optimized-routines `exp`, `log` and `pow` — bit-identical to upstream |
| `real` | — | The `no_std` float-math shim |
| `scalar` | lifted + additions | OpenFOAM guard constants, extended here with GSL's epsilon constants (`bn:op-l87q`) |

**Deliberate subsets rather than gaps**, each named where it matters and
tracked as a bead: QAGS and the infinite-range and weighted quadrature
variants; implicit and stiff ODE methods; Akima, Steffen and periodic splines;
general-degree complex polynomial roots; and rank-deficient least squares (the
QR is unpivoted, so a dependent design matrix is *reported*, not solved —
`op-4m4b`). An empty module that looks like an API is worse than an absent one.

One gap is blocked on a **source**, not on effort: **adaptive Chebyshev degree
by tail-chopping** (`op-0sl9`). GSL has no adaptive-degree routine, so there is
nothing vendored to port, and deriving a chopping rule from a description is
what this crate's porting rule exists to prevent. SLATEC's `INITS`/`INITDS` is
the named candidate and lives on netlib, which this environment's gateway
refuses outright (`403` to `CONNECT www.netlib.org:443`). "Not done" is the
honest state.

## Two ways to build a Chebyshev series, and which to use

| | `ChebSeries::new` | `ChebSeries::fit` |
|---|---|---|
| Input | a function you can **call** | data you **already have** |
| Abscissae | the Chebyshev nodes, chosen for you | wherever your samples are |
| Method | cosine transform (`cheb/init.c`) | Householder QR (`linalg/qr.c`) |
| Samples needed | exactly `order + 1` | at least `order + 1`, usually many more |
| Gives you | the interpolant | coefficients, residual, RMS, rank indicator |

`new` is exact and fast and should be the default when you can evaluate the
function at will. `fit` is for measured or expensively-simulated data, and for
samples someone else's sampler chose.

**Fitting is done in the Chebyshev basis rather than the monomial one on
purpose.** A monomial design matrix is a Vandermonde matrix whose condition
number grows exponentially with degree, so a `polyfit` past degree ~10 is
dominated by rounding. The Chebyshev basis is near-orthogonal on the interval,
which is what keeps the same fit solvable much further up. For the same
reason the solve goes through QR rather than the normal equations `AᵀA c =
Aᵀy`: forming `AᵀA` squares the condition number, turning an ordinary `1e8`
problem into an unsolvable `1e16` one.

Given `order + 1` samples taken exactly at the Chebyshev nodes the two paths
must agree, and they do — to 2.13e-14 pointwise at order 12 on `exp(x)sin(3x)`
over `[-2, 3]`. They share no code below the basis recurrence, so that
agreement cross-checks both.

### GSL's `cheb/` is covered completely

All fourteen public entry points of `gsl_chebyshev.h` are accounted for, and
`the_gsl_chebyshev_header_has_no_entry_point_petir_lacks` reads the vendored
header on every run so the claim cannot rot. `gsl_cheb_alloc` and
`gsl_cheb_free` are the only two without an equivalent: a `ChebSeries` owns its
coefficients and the compiler drops it, so there is nothing for them to do.

`gsl_cheb_eval_mode` / `_mode_e` are ported as `eval_mode` / `eval_mode_err`,
with `gsl_mode_t` becoming the `Precision` enum. A caveat worth stating,
because the API implies more than it delivers: **`gsl_cheb_alloc` sets
`order_sp = order` and nothing in GSL ever changes it**, so the reduced
precision modes are identical to full evaluation unless you call
`set_order_sp` yourself. That is upstream's behaviour reproduced faithfully,
not a gap in the port — GSL's own header says the reduced order is
"specific to the approximated function" and leaves it to the caller.

## Errors are returned, and no routine panics on an index

Two gates in `tests/no_panic_gate.rs` hold this up, and they cover different
things:

1. **No explicit panicking construct** in library code — no `panic!`,
   `unwrap()`, `expect(..)`, `unreachable!`, `todo!` or `assert*!`.
2. **No subscript that can fail at run time**, anywhere in the crate — the
   verbatim lifts included. The only form allowed is a literal index into a
   `const` array whose length is also a literal, which rustc's deny-by-default
   `unconditional_panic` lint checks at compile time; the test parses those
   declarations itself and compares the index against the length, rather than
   trusting that a literal looks safe. Everything else goes through `get` /
   `first` / `last` / `split_first` / `split_last`, a slice pattern, or a
   `zip` — which is what the crate-internal `zip_flat!` exists for, so a
   five-way parallel-array loop does not have to bind `((((a, b), c), d), e)`.

Why it is worth the trouble: on `thumbv7em-none-eabihf` a panic is not a stack
trace and a non-zero exit, it is the end of the program in a device that may be
controlling something.

The lifts needed no exemption in the end, but only because the subscripts they
carried were fixed **upstream** — in `outram-foam-basic-lib` and
`chem-eng-real-time-process-control-simulator` — and then re-lifted. That is
the route any future one has to take: editing a lift here would break the
byte-identical property it exists for.

Neither gate covers allocation failure (`Vec` aborts; `alloc` has no stable
fallible API) or `usize` overflow under `debug_assertions`. Saying so is the
point: a crate claiming "no panics" while those held would be overclaiming.

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

## Prior art in Rust

PETIR is not the first attempt to bring these algorithms to Rust, and two
crates got here earlier:

- **[peroxide](https://github.com/Axect/Peroxide)** (MIT OR Apache-2.0) by
  Tae Geun Kim — a full numeric stack: linear algebra including a `QR`
  decomposition, ODE integration, interpolation and splines, quadrature, root
  finding, optimisation, Chebyshev polynomials and nodes, plus eigenvalues,
  sparse matrices, automatic differentiation and a dataframe that PETIR has
  no equivalent for.
- **[roots](https://github.com/vorot/roots)** (BSD-2-Clause) by Mikhail
  Vorotilov — closed-form polynomial solvers **up to quartic**, and the
  bracketed iterative finders.

The newest work here overlaps them most directly: the QR, the least-squares
path and the Chebyshev machinery are ground peroxide had already covered.
Arriving at the same place later by a different route is not discovery, and
both crates were early to this in Rust when the ecosystem was still thin.

### Three modules are genuinely derived from them

| module | from | licence |
|---|---|---|
| `src/poly/quartic.rs` | `roots` 0.0.8 — closed-form quartic and biquadratic | BSD-2-Clause |
| `src/poly/companion.rs` | `roots` 0.0.8 — companion-matrix eigenvalue root finder | BSD-2-Clause |
| `src/poly/dense.rs` | `peroxide` 0.41.2 — polynomial algebra, Legendre/Chebyshev/Hermite/Bessel | MIT (of MIT OR Apache-2.0) |
| `src/integration/gauss_legendre*.rs` | `peroxide` 0.41.2 — Gauss-Legendre and Newton-Cotes quadrature, and the order-2-to-30 node/weight tables | MIT (of MIT OR Apache-2.0) |

All were ported on 2026-09-15, with the full upstream notice and
copyright kept in each file's header where source redistribution requires them.
Both **BSD-2-Clause into GPL-3.0-only and MIT into GPL-3.0-only are one-way**:
this code cannot flow back to either crate under its original licence without
its author's agreement.

`companion.rs` carries the longest chain in the crate, and every link in it
asked to be named — Martin and Wilkinson's 1971 Algol `hqr2`, EISPACK, JAMA's
public-domain Java, **Stepan Yakovenko**, whose hand-transpilation header asks
*"hopefully someone will appreciate my one day of manual code conversion
nightmare and mention me in the source code"*, and **Mikhail Vorotilov**, who
added it to `roots` at his request. All five are named in the file.

**Three upstream node values were wrong and are corrected here.** An audit of
all 928 tabulated Gauss-Legendre values against Bonnet's recurrence
(`tests/gauss_legendre_table_audit.rs`) found three bad nodes in `peroxide` —
one of them a single-digit typo costing its 12-point rule about ten
significant figures. The corrections agree with the standard published tables;
every weight was correct. Reported upstream.

Outside those files, **no code in PETIR is copied from, translated from,
or derived from either crate.** Every other routine traces to the upstream in
its own file header, and `tests/verbatim_provenance.rs` plus the code-to-code
reference sets hold that claim to account. Neither crate is a *dependency* —
all three modules were ported, not linked.

**Every port is additive, and that is the rule.** Each fills a gap GSL does
not cover: no closed form exists past the quartic; GSL's general-degree solver
is four files over a complex layer this crate does not have; and GSL has no
polynomial algebra at all. None of them replaces anything verified — PETIR's
maturity rests on **bit-identity with GSL 2.8 compiled and run**, and
re-porting a GSL-verified routine from either crate would break those
comparisons by construction for no capability gain.

Two things were deliberately **not** taken. `roots`' `find_roots_sturm` is
broken — measured against upstream compiled and run, it never returns more
than three roots at any degree and reports no error when it drops the rest;
a test pins that so a future upstream fix gets noticed. And `peroxide` remains
impossible as a *dependency* (it pulls `blas`, `lapack`, `netcdf`, `arrow`),
which has no bearing on porting a pure-Rust routine out of it.

PETIR otherwise exists alongside them because it is `no_std` unconditionally
and carries provenance to a specific upstream commit for every routine —
constraints particular to this workspace, not deficiencies in either crate.

The full acknowledgement, including a module-by-module overlap table and the
derivation record, is in [`NOTICE`](NOTICE).

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
