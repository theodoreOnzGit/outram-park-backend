# kovan-codegen

**KOVAN** — **K**nowledge **O**riented **V**&V **A**nalysis for **N**uclear
**S**ciences. This crate is part of KOVAN: deterministic generation
of Rust source for known numerical methods.

> **On crates.io (2026-10-07):** the KOVAN app is published as
> `knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan`, its backronym
> spelled out (`kovan` on crates.io is an unrelated crate). Install it with
> `cargo install knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan`;
> the binaries are still `kovan`, `kovan-cli` and `kovan-tui`, and the library
> is still `kovan`.

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`.

**Scope.** Emitting the source of textbook numerical methods, byte for byte
the same every time. **The one question it answers:** what is a plain,
readable Rust implementation of this known method, and does it pass its own
analytical check?

**Knowledge it makes studyable.** Each method is one short template file
(for example `src/root_finding/templates/brent.rs`) that a student can read
top to bottom, and the same bytes are compiled and tested against an exact
answer (`sqrt(2)`, `e`, the Dottie number, `sin(pi x)`). A reader can check
the algorithm against a textbook, rerun its test, and challenge the
tolerance it was judged by.

**Status:** AI draft (`maturity = 1` in `Cargo.toml`); no function carries a
human review stamp.

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | No literature is cited: the methods are standard textbook algorithms, and the templates carry no references. The Zotero schema tables come from Zotero's `schema.json` (zotero-schema schema version 45, commit `b86c79b56479`, AGPL-3.0; see [`NOTICE`](NOTICE)). |
| Mathematical equations | The model problems are stated in [`DECISIONS.md`](DECISIONS.md): `x^2 - 2 = 0`, a 3×3 linear system, `x = cos x`, `dy/dt = y` and stiff `dy/dt = -1000 y`, and 1-D Poisson `-u'' = pi^2 sin(pi x)`. The templates themselves state no equations beyond their doc comments. |
| Source code | `src/root_finding`, `src/linear`, `src/nonlinear`, `src/ode`, `src/pde`, `src/patterns` (each with a `templates/` folder and a `reference` module), `src/macros_support.rs`, `src/zotero/`. |
| Interactive simulations | None yet. The `catalogue` example and `kovan-cli methods` / `kovan-cli gen` print the catalogue and the generated source. |
| Visualisations | None yet. |
| Verification and validation evidence | Unit tests in each family's `mod.rs` and `tests/zotero_schema_regen.rs`. Recorded in [`DECISIONS.md`](DECISIONS.md) (status as of 2026-07-15): root finders match `1.4142135623730951` to `< 1e-10`; LU to `< 1e-12`; RK4 (h = 0.01) matches `e` to `< 1e-8`; 1-D Poisson on 200 cells, max nodal error `< 1e-4`; "37 unit tests + 1 doctest pass". Not re-checked against today's test count. |
| Human reviews and corrections | 0 functions carry a kovan human-review stamp. No entry in `docs/human-corrections-to-ai-work.md`. |

The analytical checks above are verification of the templates only; nothing
here is validated, and no human has reviewed the generated code.

## What the crate does

`generate` takes a `Method` (an enum over root finders, linear solvers,
nonlinear solvers, ODE integrators and PDE schemes) and returns the source of
that method. It emits implementations of **known** algorithms only. It does not
generate arbitrary software and it is not an AI coding assistant. The same
`Method` always yields byte-identical output, because each method's source is a
template file under `src/<family>/templates/`, returned by `include_str!`. The
same file is compiled into that family's `reference` module by `include!`, so
the crate's own tests run the exact bytes `generate` emits.

## What is generated and what is not

Per [`DECISIONS.md`](DECISIONS.md):

| Family | Generated | Catalogued, returns `CodegenError::Unimplemented` |
|---|---|---|
| Root finding | bisection, regula falsi, secant, Newton-Raphson, Brent | Illinois, Pegasus |
| Linear | dense LU with partial pivoting | Jacobi, Gauss-Seidel, SOR, CG, BiCGSTAB, GMRES, QR, Cholesky |
| Nonlinear | Newton for systems; scalar fixed-point through `nonlinear::generate_fixed_point` (not a `Method` variant) | quasi-Newton, Broyden, trust region |
| ODE | explicit Euler, RK2 (midpoint), RK4, backward Euler | Dormand-Prince, Crank-Nicolson |
| PDE | 1-D finite-difference Poisson (central stencil, Thomas solve) | finite-volume 1-D diffusion, general BC scaffold |

The `patterns` module holds three small building blocks used around solvers:
RMS residual norm, under-/over-relaxation update and a relative-change
stopping criterion. The `macros_support` module has one working declarative
macro, `kovan_fixed_point!`. Its procedural-macro and `build.rs` generators
emit **scaffold text only**, with `// TODO(kovan)` markers for a human to
complete. The templates are plain-`f64` kernels and carry no `uom` types.

The correctness checks behind each generated method (methodology and the
tolerances reached) are listed in `DECISIONS.md`.

## Zotero schema tables

`kovan_codegen::zotero::generate_schema_rs` (GitHub #748) reads Zotero's
`schema.json` and emits the Rust tables committed as
`kovan-common/src/zotero/schema_generated.rs`. Unlike the method templates it
reads input, but it is still deterministic: the output is a pure function of
the input text. Regenerate and check with

```bash
cargo run --release -p kovan-codegen --example zotero_schema -- \
    vendor/zotero-schema/schema.json crates/kovan-common/src/zotero/schema_generated.rs
cargo test --release -p kovan-codegen --test zotero_schema_regen
```

The test skips (and says so) when `vendor/zotero-schema/` is absent, as on CI.
Ported from Zotero (AGPL-3.0); see [`NOTICE`](NOTICE).

## Example

```bash
cargo run -p kovan-codegen --release --example catalogue
```

From the CLI: `kovan-cli methods` lists the catalogue and `kovan-cli gen`
generates source. The public API mirror is
[`docs/kovan-codegen-api.md`](docs/kovan-codegen-api.md).

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

~~GPL-3.0.~~ **AGPL-3.0-only since 2026-10-07**, like all of kovan (see [`NOTICE`](NOTICE)). Part of the [OUTRAM PARK](../../README.md) workspace.
