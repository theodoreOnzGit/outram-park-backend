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

**Not declared mature.** The workspace's API-dogfooding rule therefore does not
apply yet, and nothing in this crate may be described as validated. Unit tests
and cross-checks against independent implementations are *verification*, and
only at the unit level.

Proposing maturity is allowed; declaring it is the maintainer's call alone.
