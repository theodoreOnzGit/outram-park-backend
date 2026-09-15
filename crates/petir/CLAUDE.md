# CLAUDE.md — `petir`

Crate-specific guidance. The workspace `CLAUDE.md` still binds; this adds to it
and, where noted, is stricter.

## Read the epic first

The settled design decisions for this crate live in the kopi-beans epic
**`op-chyp`**, not here. Read it (`bn show op-chyp`) before proposing anything
structural. The short version, all of it maintainer-decided:

1. PETIR is a **`no_std` port of GSL**, plus selected GNU Octave maths.
2. **Numerics are PORTED from a real library with real V&V — never written from
   scratch by an AI.** This is the governing constraint and it outranks
   convenience.
3. Chebyshev comes from **GSL specifically** (`cheb.c` / `gsl_chebyshev.h`).
4. `no_std` is for **portability** — embedded, wasm, Android, anywhere.
5. **Raw `f64`, not `uom`**, in the numerics. See the carve-out below.

## Rule 2 is the one that gets broken. Here is how not to break it

"Ported" means *you read the upstream source*. It does not mean you recalled a
formula, or reproduced a coefficient table you have seen before, or derived a
continued fraction that happens to be standard. Those are all writing from
scratch, and they produce code that looks identical to a port and carries none
of its value.

The upstream is vendored at `upstream_source/GSL` (gitignored; clone it with the
command in `upstream_source/README.md` if it is missing — a fresh container will
not have it). **If you cannot read the source, you cannot port the routine.**
Say so and stop, rather than producing something that will be mistaken for a
port later.

When you do port, the doc comment names the **upstream file and line**, and the
NOTICE names the copyright holder for that module — GSL's holders differ per
file, so re-read each header rather than copying one across.

### Four non-GSL ports exist, and they were maintainer-directed

Added 2026-09-15 at the maintainer's direction — "port select code from roots
and peroxide to complement petir's capabilities, credit the author's
upstream":

| module | upstream | licence |
|---|---|---|
| `src/poly/quartic.rs` | `roots` 0.0.8 (Mikhail Vorotilov) | BSD-2-Clause |
| `src/poly/companion.rs` | `roots` 0.0.8, and through it JAMA / EISPACK | BSD-2-Clause |
| `src/poly/dense.rs` | `peroxide` 0.41.2 (Tae Geun Kim) | MIT (of MIT OR Apache-2.0) |
| `src/integration/gauss_legendre*.rs` | `peroxide` 0.41.2 (Tae Geun Kim) | MIT (of MIT OR Apache-2.0) |

Five things about them bind future work:

- **Both flows are one-way.** BSD-2-Clause and MIT into GPL-3.0-only. Each
  file carries the full upstream notice and copyright in its header, as source
  redistribution requires. Do not strip them, do not "tidy" them into the
  NOTICE only, and do not contribute anything derived from them back upstream
  without the author's agreement.
- **`companion.rs` names five people, on purpose.** Stepan Yakovenko's own
  header asks to be mentioned in the source code. That request is honoured;
  do not remove it in a refactor.
- **Do not widen this into the GSL-verified modules.** Both upstreams have
  routines overlapping `crate::roots`, `crate::min`, `crate::integration` and
  `crate::interp`, but this crate's maturity rests on bit-identity with
  compiled GSL — five of seven numerics surfaces agree exactly, every
  iterate. `roots`' Brent is not GSL's Brent, so re-porting would break those
  comparisons by construction for no capability gain. Every port above is
  ADDITIVE, filling a gap GSL does not cover. A port that replaces a verified
  routine needs the maintainer, not an agent.
- **Do not port `roots`' `find_roots_sturm`.** It is broken, and this was
  measured, not assumed: against upstream compiled and run it never returns
  more than three roots at any degree (3, 3, 3, 1, 3, 1, 1 for degrees 4 to
  10) and reports no error when it drops the rest. `tests/roots_companion_code_to_code.rs`
  pins the measurement. If that test ever fails because upstream fixed it,
  reconsider — do not simply loosen it.

- **Do not "restore" the Gauss-Legendre tables to match upstream.** Six of
  the 928 values deliberately differ: three `peroxide` node values are wrong
  (order 11 once, order 12 twice, each mirrored by symmetry), one of them a
  single-digit typo costing its 12-point rule ten significant figures.
  `tests/gauss_legendre_table_audit.rs` verifies every value against Bonnet's
  recurrence and will fail if they are reverted. The corrections agree with
  the standard published tables. All weights are correct and were never
  touched.

**`peroxide` as a DEPENDENCY remains ruled out** (it pulls
`blas`/`lapack`/`netcdf`/`arrow`, which the "Dependencies" section below
forbids). That is a separate question from porting a pure-Rust routine out of
it, which is fine and is what `dense.rs` did — do not conflate the two, as an
earlier session did.

### The three lineages, and saying which one you are in

Every module declares itself as **ported**, **lifted verbatim**, or
**delegated** (see the README table). A routine in none of those three does not
belong in this crate. If something genuinely has no suitable upstream, the
honest move is a bead saying so, not a hand-rolled implementation with a
confident doc comment.

A worked example of rule 2 being followed properly: `specfunc::ln_gamma` was
first written from a remembered Lanczos table, then checked against the vendored
`specfunc/gamma.c` — the coefficients and the formula matched, and reading the
source *also* revealed the two Padé branches at the zeros that the from-memory
version had omitted. The memory version would have passed its tests and been
silently worse. That is the failure mode rule 2 prevents.

## Verbatim lifts: `tests/verbatim_provenance.rs` is not optional

Files lifted from `outram-foam-basic-lib` and
`chem-eng-real-time-process-control-simulator` must keep diffing clean against
their sources. The test re-applies the documented `std` → `core`/`alloc`
substitutions and compares substantive lines.

- **Do not "improve" a lifted file.** Fix the bug upstream and re-lift, so both
  crates get the fix. A divergence that exists only here is a defect waiting to
  be rediscovered.
- **Do not add doc comments to lifted files** to satisfy `missing_docs`. The
  lint is suppressed at the `pub mod` declaration in the parent `mod.rs`, and
  what the fields mean is documented there. Documenting them properly means
  doing it upstream and re-lifting.
- If a divergence is genuinely necessary, record it in the file's PROVENANCE
  block **and** raise `allowed_deviations` in the test with a reason. There are
  currently exactly two, both the `extern "C"` → `libm` FFI removal.

## `no_std` is the contract, not a configuration

There is no `std` feature and there must not be one. Before calling anything
done:

```bash
cargo test  -p petir --release
cargo build -p petir --target thumbv7em-none-eabihf
cargo build -p petir --target wasm32-unknown-unknown
cargo check -p petir --all-targets --target aarch64-linux-android
cargo build -p petir --no-default-features
```

The host build passing means nothing on its own — `cargo test` links `std`, so
a `std::` path that sneaks in compiles fine there and fails only on the
bare-metal target.

### Float maths goes through `crate::real::Real`

`core` has `abs`, `min`, `max`, `signum`, `copysign`, `recip` and the
classification predicates. It does **not** have `sqrt`, `exp`, `ln`, `powf`,
`powi`, the trig or hyperbolic families, `floor`, `ceil`, `round`, `trunc`,
`fract`, `mul_add` or `rem_euclid`. `Real` supplies those from `libm`.

Import it as:

```rust
#[allow(unused_imports)]
use crate::real::Real;
```

The `allow` is required, not sloppiness: under a `std`-linked build (`cargo
test`) `f64`'s inherent methods shadow the trait ones and the import reads as
unused. Do not "clean it up".

## The `uom` carve-out — narrow, and keep it narrow

The epic says raw `f64`. That holds for `linalg`, `poly`, `specfunc` and
everything that follows them, and it is not being relaxed.

`uom` appears **only** in `transfer_fn`, behind the default-on `transfer-fn`
feature, because the blocks being ported genuinely type a sample time as `Time`
and a signal as `Ratio`, and stripping that would be a regression against the
source. Do not widen it into the numerics layers. Polynomial coefficients stay
bare `f64` even inside `transfer_fn` — the coefficient of `s^k` carries units of
`s^k`, so no single quantity can type a coefficient vector.

### Known `uom` limitation, so you do not rediscover it

`uom` aliases `uom::num::Float` to `num_traits::FloatCore` when its own `std`
feature is off, so **`Quantity::sqrt` / `exp` / `powf` do not exist in a `no_std`
build**. The error names `Quantity` and says nothing about features.

Adding `num-traits` with `libm` does **not** help — `uom` keys the alias on its
own `std` feature and never consults num-traits' features. This was tried.
`transfer_fn::ratio_ext::RatioExt` restores `sqrt` and `exp` for dimensionless
`Ratio`, which is where every affected call site actually was. Keep it to
`Ratio`: a dimensioned `sqrt` must halve type-level exponents, and that is
`uom`'s macro's job, not something to hand-roll.

## Dependencies

`libm` is inherited from the workspace. **`uom` is not, and cannot be**: the root
entry carries default features (which include `std`), and Cargo's workspace
inheritance can only add features, never remove them, so a `no_std` `uom` is
unobtainable through inheritance. It is declared crate-locally with a duplicated
version string, and `tests/dependency_policy.rs` enforces that the two stay in
step. **Bump both in the same change.**

Nothing may be added that pulls system BLAS/LAPACK, a C or Fortran toolchain,
`std`, threads, or an allocator beyond `alloc`. No `rayon` — callers parallelise
above this crate.

## Maturity

**DECLARED MATURE 2026-09-15 by the maintainer.** The workspace's
API-dogfooding rule therefore now applies to this crate — see "What maturity
obliges" below.

- 2026-09-15 — mature. Bar: **agreement with GSL 2.8 compiled and run.**
  Evidence class: cross-code. Measured, against a GSL built from the vendored
  tree with its output committed under `reference-data/gsl/`:

  | surface | bit-identical | worst relative difference |
  |---|---|---|
  | `cheb` pipeline (`init`/`eval`/`deriv`/`integ`) | 4707/6114 (77.0 %) | — |
  | `cheb::eval_mode`, default `order_sp` | 461/615 (75.0 %) | 1.06e-15 results, 1.05e-2 error estimates |
  | `cheb::eval_mode`, reduced `order_sp` | 357/369 (96.7 %) | — |
  | `linalg::qr` factorisation and least squares | 81/85 (95.3 %) | 4.27e-16 factor, 4.94e-16 solution |
  | `expint`, `gamma_inc` | see `gsl_expint.rs`, `gsl_gamma_inc.rs` | — |
  | `roots` (bracketing / polishing), `min`, `interp`, `ode` (RKF45) | 100 % — every iterate, value and error estimate | 0 |
  | `deriv` | 76/78 (97.4 %) | 3.27e-16 |
  | `integration` (6 Kronrod rules + QAG) | 27/28 (96.4 %) | 1.82e-16 |
  | `fast_exp` / `fast_log` / `fast_pow` | 100 % vs ARM optimized-routines | 0 |

  The declaration was made on the condition "if it agrees with GSL". It does.

### What the bar does NOT cover, and it matters

**Nothing here has been compared against a published benchmark.** Cross-code
agreement with GSL establishes that this is a faithful port of GSL; it does
*not* establish that GSL is right for your problem, and it is not validation
in the sense `VERIFICATION_AND_VALIDATION.md` uses. Do not describe any PETIR
result as validated.

**Both bookkeeping axes in `README.md` remain unsigned** (❌). Maturity and
bookkeeping sign-off are different things: the maintainer declared the former
on 2026-09-15 and described the latter as "almost" ready on the same day.
An assistant must not flip either axis — see the workspace `CLAUDE.md`.

**Evidence WAS uneven at the moment of declaration; it no longer is.**
Compiled-GSL references existed for `cheb`, `expint`, `gamma_inc` and
`linalg::qr`, while `roots`, `min`, `deriv`, `interp`, `integration` and `ode`
had only GSL's own assertions and analytical results. Those six were given
compiled references the same day
(`tests/gsl_numerics_code_to_code.rs`), and five of the seven surfaces came
back **bit-identical throughout** — every iterate of every root finder and
minimiser, every interpolated value, every RKF45 step and its error estimate.
The full table is in [`docs/verification-summary.md`](docs/verification-summary.md).

The 19 verbatim lifts are a separate lineage entirely (OpenFOAM via
`outram-foam-basic-lib`, GNU Octave via `chem-eng`), pinned by
`tests/verbatim_provenance.rs` rather than by any GSL comparison.

### What maturity obliges

The workspace rule "if it is too complex for Haiku, it is a bad API" now
binds this crate. The Rust half applies directly: any trait whose bound a
caller can plausibly fail should carry
`#[diagnostic::on_unimplemented]`. PETIR's public trait surface is small —
`real::Real` is the one callers name — so this is a bounded job rather than
the 65-trait audit `op-wiep` describes for the workspace.

Revising the bar is a maintainer decision; record it as a further dated entry
above and keep the superseded ones.
