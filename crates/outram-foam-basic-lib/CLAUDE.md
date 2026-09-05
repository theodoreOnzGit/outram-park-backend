# CLAUDE.md — outram-foam-basic-lib

This crate is a pure-Rust translation of the OpenFOAM C++ primitive and
finite-volume library layer, scoped to the primitives needed to implement
compressible solvers equivalent to **rhoPimpleFoam**, **sonicFoam**, and
**rhoCentralFoam** (Kurganov-Tadmor central-upwind, density-based explicit).

The reference C++ source lives at:
`/home/teddy0/Documents/research/openfoam/`

## Maturity: DECLARED MATURE (2026-09-05)

The API-usability rules in the root `CLAUDE.md` ("Human interface layer",
and the Haiku dogfooding hard rule) **are in force for this crate**. See the
maturity gate in that file for what this means and how the bar is revised.

- **2026-09-05 — mature.** Bar: the `vv_*` verification suite passes —
  conservation to machine precision (`epsilon = 1e-12`) across cyclic, AMI
  matching and AMI non-conformal advection, diffusion and vector-Laplacian
  paths; agreement with analytic references on reductions, volume integrals
  and the Robin convective boundary; and **observed convergence order matching
  theory** for the differentiation schemes. Evidence class: **analytical /
  manufactured solution** plus internal consistency, supported by cross-code
  comparison against OpenFOAM (563 in-source references, including a test that
  the first application is unchanged from the OpenFOAM spelling).

  Measured at declaration: 581 `#[test]` markers in-crate and the suite green
  (individual per-crate pass count not separately recorded at declaration time).

  Note the bar here is deliberately about *the numerics*, not about a physical
  benchmark: this crate is the primitive and finite-volume layer, so
  conservation and convergence order are the properties that matter. Physical
  validation belongs to the solvers built on it.


---

## Why this crate exists

OpenFOAM's C++ codebase is the negative example this port is designed to undo:

- **Runtime type registration** via macros (`addToRunTimeSelectionTable`) hides the
  class hierarchy from any static tool — you cannot hover over a type and find its
  implementors.
- **Dictionary-driven input** (`fvSolution`, `fvSchemes`) — valid keys exist only in
  source comments and forum posts, not in any machine-readable interface.
- **wmake** — a bespoke build system nothing else can consume; OpenFOAM cannot be
  used as a library by another project.
- **No units discipline** — `scalar` is `double`; passing pressure where velocity is
  expected compiles silently and produces a wrong answer.

This crate replaces each of those with the Rust equivalent:
- Traits make the type hierarchy explicit and statically navigable via rust-analyzer.
- Struct fields with `///` doc comments replace runtime dictionaries — valid "inputs"
  are visible on hover.
- Cargo makes this crate a normal library dependency.
- `uom` makes unit errors compile errors.

**The mandatory consequence:** every public item must be navigable with rust-analyzer
alone, by a developer with no prior OpenFOAM knowledge. See the root `CLAUDE.md`
"Human interface layer" section for the full rule.

**Layer 5 (solver logic — PISO/PIMPLE loops, multi-region coupling, turbulence
model registries) is intentionally excluded from this crate.** It belongs in
separate solver crates (`openfoam-icof`, `openfoam-cht`, `openfoam-rho`) that
depend on this crate. See the workspace `CLAUDE.md` for the planned crate list.

---

## Goal and scope

The crate climbs the OpenFOAM stack from the bottom up. Each layer depends
only on the one below it:

```
Layer 5  Solver logic       rhoPimpleFoam / sonicFoam / rhoCentralFoam loops
Layer 4  Thermophysics ← THIS CRATE
           • Fluid thermo     fluidThermo / psiThermo / rhoThermo (fluid_thermo)
Layer 3  FV operators ← THIS CRATE
           • fvm:: / fvc::    implicit + explicit operators (fv_operators)
           • Source terms     fvOptions / fvModels (fv_options)
           • Flux limiters    TVD limiters (limiters)
Layer 2  Fields + Mesh ← THIS CRATE
           • Fields           volScalarField, surfaceField (fields)
           • Mesh             fvMesh, AMI, region interfaces (mesh)
           • Matrices         lduMatrix, fvMatrix + solvers (ldu_matrix, krylov)
           • Case I/O         polyMesh / dictionary / field files (io)
Layer 1  Primitives ← THIS CRATE
           • Tensor algebra   Vector3, Tensor, SymmTensor, SphericalTensor
           • Dense matrices   scalarSquareMatrix, LU/QR/Cholesky/SVD
           • Polynomial math  linearEqn, quadraticEqn, cubicEqn, Polynomial<N>
           • ODE solvers      Euler, RKF45, RKDP45, Rosenbrock23/34, seulex, …
           • Interpolation    interpolationTable, interpolateXY, spline
           • Math functions   erfInv, incGamma, invIncGamma, …
           • Thermophysics    specie-level EOS / thermo / transport kernels (1h)
```

`outram-foam-basic-lib` covers **Layers 1–4**. Layer 5 (the solver loops) lives
in separate crates that depend on this one — see "Layer 5 is intentionally
excluded" above and the workspace `CLAUDE.md` for the planned crate list.

---

## Design rules (see also root CLAUDE.md)

### Enum dispatch for physics models

Every group of interchangeable physics models in this crate uses an enum, not a
trait object. The concrete models for EOS, thermo, and transport are a closed set —
enums give exhaustiveness checking and zero heap allocation.

```rust
// EOS enum — rust-analyzer shows all options; adding one forces all match sites to update
pub enum Eos {
    PerfectGas(PerfectGas),
    RhoConst(RhoConst),
    IcoPolynomial(IcoPolynomial<8>),
    PengRobinson(PengRobinsonGas),
}

// Thermo enum wraps an EOS variant
pub enum Thermo {
    HConst(HConstThermo<Eos>),
    Janaf(JanafThermo<Eos>),
    HPolynomial(HPolynomialThermo<Eos, 8>),
}
```

Trait definitions (`EquationOfState`, `ThermoModel`, `TransportModel`) remain as
compiler contracts on each concrete struct — they are not used for `dyn` dispatch.

### No `Box<T>`, no lifetime parameters, no trait objects

Follows the workspace rule. All uom quantity types are `Copy` — own them by value.
Shared mesh or field data: `Arc<FvMesh>`, `Arc<RwLock<VolScalarField>>`.

## Implementation rules

### `extern "C"` policy

`extern "C"` blocks are **permitted if and only if** the called function
compiles and links natively on **all five** target platforms:

| Platform | C runtime | Notes |
|---|---|---|
| Linux | glibc / musl (`libm`) | always available |
| macOS | Apple `libSystem` | always available |
| Windows | MSVC CRT (VS 2013+) or MinGW | always available on modern toolchains |
| Android | Bionic libc | `erf`/`erfc`/`lgamma` since API 9; `tgamma` since API 21 |
| iOS | Apple `libSystem` | always available |

If a function is missing from **any** of these five (e.g. it only exists on
POSIX systems), it must be implemented in pure Rust instead.

The current `extern "C"` calls to `erf`, `erfc`, `tgamma`, and `lgamma` in
`inc_gamma.rs` and `inv_inc_gamma.rs` satisfy this rule (minimum Android
API 21 for `tgamma`, which covers ~98% of active devices).

Prefer Rust stdlib equivalents where they exist — `f64::gamma()` and
`f64::ln_gamma()` (stable since Rust 1.83) can replace `tgamma`/`lgamma`
with no FFI at all.

---

## Porting workflow (MANDATORY — follow for every new port)

After adding any new type, function, or module, you MUST do **both**:

### 1. Update `src/prelude.rs`

Add all new public items to the appropriate `pub use` block in `src/prelude.rs`
so that a wildcard import `use outram_foam_basic_lib::prelude::*` exposes them.

### 2. Update `README.md`

Add a row for each newly ported item to the **Ported items** table in
`README.md`.  The table lives under the `## What's implemented` heading.
Format: `| Module | Rust type / fn | C++ source | Notes |`.

Verification:
```bash
cargo test -p outram-foam-basic-lib --lib --release   # must be green before committing
cargo test -p outram-foam-basic-lib --doc --release   # must be green before committing
```

**Rule: always use `--release` for builds and tests.** Never run in debug mode.

### Doc-comment code blocks

Any ```` ```rust ```` block in a doc comment is compiled **and executed** as a
doctest (`cargo test --doc`). Rules:

- **Never use `ignore` to silence a failing doctest.** Fix the code instead.
- `crate::` does not work in doctests — rustdoc compiles each snippet as an
  external user of the crate. Use the external crate name instead:
  `outram_foam_basic_lib::` (e.g. `use outram_foam_basic_lib::thermophysics::imports::*;`).
- Do not use `crate::` in doc-comment examples at all; always write the
  fully-qualified external path so the snippet is self-contained.
- ```` ```rust,no_run ```` is acceptable only for snippets that are genuinely
  side-effectful or require external resources at runtime.

---

## Critical translation gotcha — field `name` must not grow

**Never build a field's `name` String compositionally inside an arithmetic
operator.** `Field`/`VolField`/`SurfaceField`'s `Add`/`Sub`/`Neg`/`Mul` must
leave `self.name` as the left operand's name — they must **not** do
`self.name = format!("({} + {})", self.name, rhs.name)`.

A solver repeatedly reassigns a persistent field from an expression containing
that same field (e.g. `rho = rho + div(phi)`). With compositional naming the
`name` string then **doubles in length every timestep** (`2^step` growth) —
invisible in the field *data* but blows the process to tens of GB within ~25
steps. This exact bug hung the rhoPimpleFoam `compressible_lid_cavity` test
(24 GB, SIGTERM). The fix lives in `fields/vol_field.rs` and
`fields/surface_field.rs`; keep the name a short, stable label (matching
OpenFOAM's fixed `IOobject` name), not an audit trail. Full write-up in
`docs/cpp-source-reference.md` (Translation notes).

---

## Reference material (read on demand, not per turn)

- **`docs/cpp-source-reference.md`** — the full C++ source → Rust map for
  Layers 1–5, the planned Rust module layout, and all per-type translation
  notes. Read the relevant layer's section when porting that layer.
- **`docs/porting-roadmap.md`** — remaining work before the solver crates can
  be written (icoFoam / chtMultiRegionFoam prerequisites), the `#[ignore]`-d
  known test failures to investigate, and the prioritised test backlog (P0–P2).
