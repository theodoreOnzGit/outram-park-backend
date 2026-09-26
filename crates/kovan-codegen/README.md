# kovan-codegen

**KOVAN** — **K**nowledge **O**riented **V**&V **A**nalysis for **N**uclear
science and engineering. This crate is part of KOVAN: deterministic generation
of Rust source for known numerical methods.

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`.

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

GPL-3.0. Part of the [OUTRAM PARK](../../README.md) workspace.
