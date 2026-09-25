# CLAUDE.md — outram-foam-turbulence-lib


Pure-Rust port of the OpenFOAM turbulence model library (RAS and LES).
This crate sits **between** `outram-foam-basic-lib` (Layer 1–3 primitives + FV
operators) and `outram-foam-appbuilder-lib` (solver loops + I/O).

> Workspace member of the **OUTRAM PARK** backend. See the root `CLAUDE.md`
> for the shared dependency policy. All dep versions come from
> `[workspace.dependencies]` — do not pin versions locally.

---

## Reactor geometry is DRAWN for a human to check before it is trusted (HARD RULE)

**Maintainer direction, 2026-09-25.** Binds this crate. The same rule is in the
`CLAUDE.md` of `outram-mc-libs`, `nee_soon`, every `outram-foam-*` crate and
every crate downstream of them; a crate that newly depends on one of those
takes the rule into its own `CLAUDE.md` (check with `cargo metadata`).

**Whenever you build or change a complex reactor geometry** — CSG cells and
surfaces, lattices, pebble beds, TRISO particles, reflector zones, control-rod
bands, a CFD/FEM mesh, anything a solver will transport or integrate through —
**draw it, as images a human can open (PNG, or JPG/SVG), and hand them over**
before any result computed on it is reported as more than tentative.

- **Draw what the solver sees, not what you meant.** Render from the ASSEMBLED
  geometry (cell / material lookup at each pixel, or the mesh itself), never
  from the named constants. A picture of the constants hides exactly the
  defects this rule exists to catch.
- **Minimum set:** an axial slice (R-Z / x-z) of the whole model; radial (x-y)
  slices at the heights that matter; and, for nested geometry, zoomed slices at
  every level down to the smallest (pebble, TRISO particle). Colour by
  material, with a legend and the key dimensions marked.
- **Commit the images with the change** (beside the V&V record or the
  manuscript package) and point the human at them by path in your summary.
  Regenerate them whenever the geometry changes.
- **Say what you checked in them, and what you could not** — an image nobody
  was told to look at checks nothing.

**Why.** On 2026-09-24/25 the HTR-10 model carried, at once: a bottom reflector
mirrored from the top and up to 107 cm short; a core cavity that grew with the
bed; pebbles interpenetrating by 1.1 cm with 4.8 % of core carbon clipped away
(gh:#309, #310); and a TRISO lattice holding 8240 particles while reporting
8340 (gh:#316, +353 pcm). Every run completed with green diagnostics. Each was
found by looking at the built geometry, not by the eigenvalue.

**Tools.** ~~`outram_mc_libs::geometry::plot` samples a slice of an assembled
CSG geometry; OpenMC-parity image output (PNG/JPG) is the preferred path once
it lands.~~ **UPDATED 2026-09-25 — it has landed (gh:#268):**
`outram_mc_libs::geometry::plot` is a port of OpenMC's plotter that writes PNG
directly — slices, wireframe and solid ray traces — verified pixel-for-pixel
against `openmc --plot`
(`crates/outram-mc-libs/verification_and_validation/geometry_plotting/`).
`render_material_slice` draws a material-coloured slice with a legend and cm
axes in one call; `crates/nee_soon/examples/htr10_geometry_images.rs` is the
worked example. For meshes, plot the mesh itself (cells, patches, zones).

## Why this crate exists

OpenFOAM's turbulence model infrastructure is the textbook example of C++
runtime-registration opacity:

- Models are registered via `addToRunTimeSelectionTable` macros — the list of
  available models exists only at runtime, not statically. There is no way to
  hover over `turbulenceModel` in an editor and see what models are available.
- Selecting a model requires knowing the exact string key (`"kOmegaSST"`,
  `"kEpsilon"`) to put in `fvSolution` — this is undocumented except in source
  comments and forum posts.
- Model coefficients are read from a runtime dictionary (`turbulenceProperties`)
  with no static schema.

This crate replaces that with a `TurbulenceModel` trait — every implementor is
visible to rust-analyzer, every coefficient is a named struct field with a `///`
doc comment, and selecting a model is a normal Rust type choice checked at
compile time.

**The mandatory consequence:** every public item must be navigable with rust-analyzer
alone, by a developer with no prior OpenFOAM knowledge. See the root `CLAUDE.md`
"Human interface layer" section for the full rule.

---

## Crate dependency position

```
outram-foam-basic-lib  (primitives, FV ops, fields, mesh)
        ↓
outram-foam-turbulence-lib   ← THIS CRATE
        ↓
outram-foam-appbuilder-lib  (solver loops, I/O)
```

Layer 5 solver logic (PIMPLE/PISO loops, time loops) is intentionally
**excluded** from this crate — it belongs in `outram-foam-appbuilder-lib`.

---

## Core trait

```rust
/// Mirrors Foam::compressible::turbulenceModel (and the incompressible variant).
pub trait TurbulenceModel {
    /// Assemble the turbulent deviatoric stress divergence term:
    ///   ∇·(−2 μ_eff · dev(symm(∇U)))
    /// Returns an FvVectorMatrix to add to the momentum equation.
    fn div_dev_rho_reff(&self, u: &VolVectorField) -> FvVectorMatrix;

    /// Update turbulence fields (k, ε/ω, ν_t/μ_t) after each time step.
    fn correct(&mut self);

    /// Effective dynamic viscosity: μ_eff = μ + μ_t
    fn mu_eff(&self, p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity;

    /// Turbulent kinematic viscosity ν_t (incompressible) or μ_t/ρ (compressible)
    fn nu_t(&self) -> &VolScalarField;

    /// Effective thermal diffusivity: α_eff = α + α_t (= κ_eff / Cp)
    fn alpha_eff(&self, alpha: &VolScalarField) -> VolScalarField;
}
```

---

## Design rules (see also root CLAUDE.md)

### Enum dispatch for turbulence models

The `TurbulenceModel` type in this crate is an **enum**, not a trait object.
The set of supported models is closed and known at compile time.

```rust
// Trait is a compiler contract on each concrete struct — not used for dyn dispatch
pub trait TurbulenceKernel {
    fn div_dev_rho_reff(&self, u: &VolVectorField) -> FvVectorMatrix;
    fn correct(&mut self);
    fn mu_eff(&self, p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity;
    fn alpha_eff(&self, alpha: &VolScalarField) -> VolScalarField;
}

// Enum dispatches without Box or dyn — adding a model is a compile-time forcing function
pub enum TurbulenceModel {
    Laminar(LaminarModel),
    KOmegaSST(KOmegaSSTModel),
    KEpsilon(KEpsilonModel),
    SpalartAllmaras(SpalartAllmarasModel),
}

impl TurbulenceModel {
    pub fn correct(&mut self) {
        match self {
            Self::Laminar(m)          => m.correct(),
            Self::KOmegaSST(m)        => m.correct(),
            Self::KEpsilon(m)         => m.correct(),
            Self::SpalartAllmaras(m)  => m.correct(),
        }
    }
}
```

No `Box<dyn TurbulenceModel>`, no lifetime parameters, no `Box<T>`.
Model coefficients are owned fields on each concrete struct, not read from a
runtime dictionary.

## Model reference (read on demand)

The OpenFOAM C++ source map, the planned module list, turbulent transport-field
units, the k-ω SST constants (Menter 1994), and wall-function routines all live
in **`docs/model-reference.md`**.

---

## Conventions

- All public method parameters and return values use `uom` dimensioned
  quantities — no bare `f64` SI values at API boundaries.
- `correct()` is called once per time step **after** the momentum and pressure
  solves — do not call it from inside the turbulence transport equation assembly.
- Transport equations for k and ε/ω are assembled with `fvm::` operators from
  `outram-foam-basic-lib`; this crate does not re-implement FV operators.

---

## Build and test

**Rule: always use `--release` for builds and tests.** Never run in debug mode.

```bash
cargo check --release -p outram-foam-turbulence-lib --lib
cargo test  -p outram-foam-turbulence-lib --lib --release
```

No system BLAS required (depends only on `outram-foam-basic-lib`, which uses
pure-Rust LU solving).
