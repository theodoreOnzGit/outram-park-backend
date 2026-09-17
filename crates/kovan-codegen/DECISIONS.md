# kovan-codegen — design decisions and status

Agent I (Code Generation) implementation notes for the two `// TODO(kovan)`
stubs in `src/lib.rs`. Records what is fully generated vs scaffolded, the
templating approach, how generated-code correctness was verified, and open
questions for human review.

Spec: `docs/kovan.md` § "KOVAN Codegen". Status as of 2026-07-15.

## Template approach: string templating (`include_str!`), not proc-macros

Generation is **deterministic string templating**. Each method's source lives in
a human-readable template file under `src/<family>/templates/<method>.rs`, and
[`generate`] returns it verbatim via `include_str!`. No formatting, no
substitution, no randomness, no I/O — the same `Method` always yields
byte-identical output.

**Why not `quote` / `syn` / proc-macros for the codegen path:**

- **Offline + Android-first (hard requirement).** `include_str!` is pure
  `core`/`std`; it cross-compiles to `aarch64-linux-android` with zero native
  dependencies. `cargo check -p kovan-codegen --target aarch64-linux-android`
  passes. Adding `syn`/`quote`/`proc-macro2` is avoidable weight.
- **Traceability (priority #2 in the spec).** A template file is a plain,
  reviewable, diffable Rust source file that a human can read top-to-bottom and
  that `git blame` tracks. A `quote!{}` token soup is neither.
- **Determinism is structural, not asserted.** Because the templates are
  compile-time constants, determinism is guaranteed by construction (and still
  tested).

Proc-macros are still *scaffolded* for the separate "Macro Support" axis (see
below), where compile-time AST rewriting is the actual goal rather than emitting
standalone source.

## The verification link: emitted string === compiled-and-tested source

Each template file is used **two ways**:

1. returned verbatim by `generate` (via `include_str!`), and
2. compiled into that family's `reference` module (via `include!`).

Because both refer to the *same file*, the numerical-correctness tests that run
the `reference::*` functions are executing the exact bytes `generate` emits. A
top-level test (`emitted_source_is_the_compiled_reference_verbatim`) asserts the
`include_str!` output equals the `include!`d source, so the two can never drift.

**How correctness was verified (methodology + results, per workspace V&V rule):**

- **Root finders** — recover `sqrt(2)` (root of `x^2 - 2`) from a bracket/guess.
  Results: bisection/regula-falsi/secant/Newton/Brent all match
  `1.4142135623730951` to `< 1e-10`.
- **LU solve** — solve a 3×3 system with exact solution `[20, -11, 30]`; a
  zero-leading-pivot 2×2 (forces a row swap); a rank-deficient matrix returns
  `None`. Results: match to `< 1e-12`; pivoting and singularity detection pass.
- **Newton for systems** — circle ∩ line, root `(sqrt2, sqrt2)`; matches to
  `< 1e-10`. Fixed-point — `x = cos x` → Dottie number `0.7390851332…`, `< 1e-10`.
- **ODE steppers** — `dy/dt = y`, `y(0)=1` over `[0,1]`, exact `e`. RK4 (h=0.01)
  matches `e` to `< 1e-8`; RK2 beats explicit Euler at the same step; backward
  Euler stays bounded and monotonically decays on the stiff `dy/dt = -1000 y`
  where explicit Euler would diverge.
- **PDE** — 1-D Poisson `-u'' = pi^2 sin(pi x)`, `u(0)=u(1)=0`, exact
  `sin(pi x)`; central FD + Thomas solve on 200 cells, max nodal error `< 1e-4`
  (consistent with `O(dx^2)`).
- **Patterns** — RMS norm of `[3,4]` = `sqrt(12.5)`; under-relaxation blend;
  scale-free / zero-safe relative change.
- **Independent check** — the emitted RK4 + LU + Brent snippets were concatenated
  into one file and compiled standalone with `rustc --crate-type=lib -D
  warnings` (exit 0), confirming the generated source compiles outside this
  crate, warning-clean, with no external deps.

Test totals: **37 unit tests + 1 doctest pass**, release mode.

## What is fully generated vs scaffolded

Fully generated (real, tested, compilable output):

| Family | Methods |
|---|---|
| Root finding | bisection, regula falsi, secant, Newton–Raphson, Brent |
| Linear | dense LU (Gaussian elimination + partial pivoting) |
| Nonlinear | scalar fixed-point, Newton for systems (embeds its own LU solve) |
| ODE | explicit Euler, explicit-midpoint RK2, classical RK4, backward Euler |
| PDE | 1-D finite-difference Poisson (central stencil + Thomas tridiagonal solve) |
| Patterns | RMS residual norm, under-/over-relaxation update, relative-change criterion |
| Macros | `kovan_fixed_point!` declarative macro (real, tested) |

Catalogued but **not yet generated** — `generate` returns
`CodegenError::Unimplemented("<family>: <Method>")`:

- Root: Illinois, Pegasus (regula-falsi variants).
- Linear: Jacobi, Gauss–Seidel, SOR, conjugate gradient, BiCGSTAB, GMRES, QR,
  Cholesky.
- Nonlinear: quasi-Newton, Broyden, trust region.
- ODE: Dormand–Prince (adaptive RK45), Crank–Nicolson.
- PDE: finite-volume 1-D diffusion, general boundary-condition scaffold.

## Macro Support framework (`src/macros_support.rs`)

- **Declarative macros — fully realised.** `kovan_fixed_point!` is a working,
  tested `#[macro_export]` `macro_rules!` (compile-time, dependency-free — the
  Android/offline-friendly workhorse). `generate_macro_scaffold(MacroKind)`
  emits it as text too.
- **Derive / attribute / procedural macros — scaffold text only.** A proc-macro
  must live in its own `proc-macro = true` crate (and pulls in `syn`/`quote`), so
  it *cannot* be defined inside this library crate. `generate_macro_scaffold`
  returns a documented skeleton (separate-crate `Cargo.toml` stanza + macro entry
  point with `// TODO(kovan)` markers) for a human to drop into a companion
  `kovan-codegen-macros` crate. Deliberately not compiled in-crate.
- **`build.rs` generation — scaffold text only.** Emits the standard
  `OUT_DIR` + `include!` offline codegen skeleton.

## Engineering Pattern Library (`src/patterns.rs`)

Starter library of the small building blocks *around* solvers: RMS residual
norm (grid-independent convergence measure), under-/over-relaxation update, and a
scale-free/zero-safe relative-change stopping criterion. Same template +
`reference` + test structure as the numerical methods; all three are real and
tested. New patterns are added as an `EngineeringPattern` variant plus a template
file.

## Changes outside the two stubs

- Added `Method::Pde(PdeScheme)` to the `Method` enum and a `pde` module, since
  the spec lists PDE infrastructure under "Numerical Methods". This is additive;
  the enum stays closed and enum-dispatched (no trait objects).
- Rewrote `examples/catalogue.rs` to actually generate and print a kernel.
- **kovan-cli impact:** `kovan-cli`'s `methods` command constructs `Method`
  variants and calls `generate` but does *not* `match` on `Method` exhaustively,
  so the new `Pde` variant does not break it (verified: `kovan-cli`/`kovan-tui`
  still build). Its listing does not yet enumerate PDE schemes, and its
  `stub_status` comment ("all stubbed today") is now stale — a cosmetic follow-up
  for the CLI owner.

## Needs from `kovan-common` (not edited, per instructions)

**None required.** These numerical templates are intentionally `uom`-agnostic
plain-`f64` math kernels (as directed), and they carry no KOVAN domain types, so
they do not touch `kovan-common`. A *future* provenance feature — tagging a
generated kernel with the `KovanDocument` / `KovanCorrelation` it implements (the
"Paper → Equation → Correlation → Implementation → Validation" vision) — would
want a small `GeneratedArtifact { method, source, source_document_id,
correlation_id }` record in `kovan-common`. Flagged for human review, not built.

## Intended beads (KOVAN epic op-5v5 is JSONL-only / not in local Dolt)

Recorded here rather than created, per instructions:

- Generate the remaining root finders (Illinois, Pegasus).
- Generate iterative linear solvers (Jacobi, Gauss–Seidel, SOR, CG, BiCGSTAB,
  GMRES) and QR / Cholesky factorisations.
- Generate quasi-Newton / Broyden / trust-region nonlinear solvers.
- Generate Dormand–Prince (adaptive) and Crank–Nicolson ODE integrators.
- Generate finite-volume PDE assembly + a general BC scaffold.
- Stand up the companion `kovan-codegen-macros` proc-macro crate from the
  scaffolds; wire a `build.rs` OUT_DIR demo.
- Update kovan-cli `methods` to list PDE schemes and drop the stale "all stubbed"
  comment.
- Consider a `GeneratedArtifact` provenance type in `kovan-common`.

## Open questions for human review

1. **Emission granularity.** `generate` emits one method's source per call
   (bare `pub fn`s, no module wrapper) so it drops into any module. Should there
   be a "bundle" mode that emits a whole family as a ready-to-`mod`-wrap file with
   a `//!` header? Kept out for now to avoid guessing the target layout.
2. **Vector vs scalar ODE steppers.** The ODE templates are scalar (`f64`).
   Reactor work needs systems (e.g. point-kinetics `Vec<f64>` state). Add
   `*_step_vec` variants, or generate from a state-dimension parameter?
3. **Signature conventions.** Root finders take `Fn(f64)->f64`; matrices are
   `&[Vec<f64>]`. A flat `&[f64]` + `stride` row-major convention would be more
   cache-friendly and alloc-free — worth standardising before more linear-algebra
   templates land.
4. **`uom` boundary.** Per the task these kernels are deliberately `uom`-free
   plain math. Confirm the eventual consumer wraps them at its own boundary
   rather than expecting `uom` signatures from codegen.
