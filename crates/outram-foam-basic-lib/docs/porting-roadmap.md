# outram-foam-basic-lib — porting roadmap & test backlog

Reference material for porting work. Consulted on demand, not per turn. The
mandatory porting *rules* live in this crate's CLAUDE.md.

## Remaining work before solver crates can be written

> **Status note (bookkeeping pass, 2026-08-07).** This section was written when
> the crate stopped at Layer 1. It is now a **historical checklist**: the crate
> covers Layers 1–4, and **13 workspace crates already depend on it** (including
> `outram-foam-appbuilder-lib`, which hosts the pimpleFoam/GeN-Foam port). Every
> icoFoam prerequisite below is implemented; the chtMultiRegionFoam list is
> partly done. The per-item statuses were verified against the source in this
> pass; where a status says "not verified", it means this pass did not check it,
> not that it is absent.

### For `openfoam-icof` (icoFoam) — all prerequisites implemented

1. ✅ **`fvVectorMatrix`** — a vector variant of `FvMatrix`. Implemented as
   `FvVectorMatrix` (`src/ldu_matrix/fv_vector_matrix.rs`): scalar LDU
   coefficients with a `Field<Vector3>` source, so no generic `FvMatrix<T>` was
   needed.

2. ✅ **`FvMatrix::A()` and `FvMatrix::H()`** — implemented as
   `FvMatrix::a_field()` / `FvMatrix::h_field(x)` (`src/ldu_matrix/fv_matrix.rs`),
   with the same pair on `FvVectorMatrix`. Note the shipped convention is
   `A[c] = diag[c]` (**not** `diag[c]/V[c]`) and `H[c] = source[c] − Σ off-diag·x`
   (not divided by `V`); callers form `rAU = 1/a_field()` directly.

3. ✅ **`fvc::flux(U)`** — `src/fv_operators/fvc/flux.rs`.

4. ✅ **`fvc::reconstruct(phi)`** — `src/fv_operators/fvc/reconstruct.rs`
   (least-squares `(Σ_f Sf⊗Sf)·U = Σ_f phi·Sf`).

5. ✅ **`fvc::ddtCorr(U, phi, dt)`** — `src/fv_operators/fvc/ddt_corr.rs`.
   **Euler only**, and knowingly inconsistent with a BDF2 (`Backward`) ddt
   scheme — see the open defect in "Known limitations" below.

6. ✅ **Reference cell constraint** — `FvMatrix::set_reference(cell, value)` and
   `FvVectorMatrix::set_reference(cell, value)`.

7. ✅ **`adjustPhi`** — `adjust_phi(phi, u)` in `src/fv_operators/adjust_phi.rs`.

No new external Rust crates were required.

### For `openfoam-cht` (chtMultiRegionFoam)

On top of all icoFoam requirements:

1. **Turbulence models** — ✅ **moved out of this crate.** Per the workspace
   Layer-5 rule, turbulence closures live in `outram-foam-turbulence-lib`
   (k-ω SST implemented; k-ε / k-ω / Spalart-Allmaras / Smagorinsky scaffolded),
   which depends on this crate. Nothing further is owed here.

2. **Multi-region mesh coupling** — ✅ **partly done in this crate.**
   `RegionInterface` (`src/mesh/region_interface.rs`) provides the face-to-face
   coupling map between two regions' patches, and `mesh::ami` provides the
   non-conformal (`cyclicAMI`) face-overlap weighting. The per-timestep T /
   heat-flux *exchange* is solver-loop logic and belongs in a Layer-5 crate,
   not here.

3. **Solid energy equation assembly** — ingredients present (`SolidThermo`,
   `fvm::ddt_coeff`, `fvm::laplacian`); assembling the equation is Layer-5 work.

4. **Buoyancy source** — ✅ ingredients present: `fvc::buoyancy_flux`
   (`φ_b[f] = ρ_f·(g·S_f)`, `src/fv_operators/fvc/flux.rs`) plus
   `fvc::reconstruct`.

5. **Wall distance field** — ❌ **still open.** No `yWallDist` equivalent was
   found in this crate in this pass.

---

## Known limitations (open defects, deliberately not fixed)

### `fvc::ddt_corr` is Euler-only and inconsistent with a BDF2 ddt scheme

`fvc::ddt_corr` hardcodes the implicit-Euler Rhie–Chow correction `phiCorr/Δt`
and takes no ddt-scheme argument. A BDF2 (`Backward`) ddt puts `1.5·V/Δt` on the
momentum diagonal, shrinking the `rAU` that multiplies this correction, while
`ddt_corr` still divides by `Δt` — so the two disagree by a ratio tending to
**1.5** as `Δt → 0`, an inconsistency that does not vanish under time-step
refinement.

Measured downstream (`outram-foam-appbuilder-lib`,
`tests/fv_scheme_selection.rs`, 2026-08-07): Euler-vs-`Backward` lid-driven-cavity
steady states differ by 1.0e-2 to 2.9e-2 m/s (1–3 % of `U_lid`), and the gap
*grows* as `Δt` is refined.

Closing it needs OpenFOAM's `backwardDdtScheme<Type>::fvcDdtPhiCorr`, which
belongs in this crate and is **not yet ported**. Until then `Backward` must not
be described as verified second-order time integration. The limitation is
documented at the point of use in `ddt_corr`'s doc comment.

---

## Known test failures (marked `#[ignore]`, needs investigation)

These tests were written but fail with errors larger than expected. They are
`#[ignore]`-d so the CI suite stays green. They may indicate a deeper bug in
the implementation, not just a tolerance issue — investigate before un-ignoring.

### `janaf::tests::newton_converges_from_bad_initial_guess`
Newton iteration starting from `t0 = 100 K` targeting `ha(3000 K)` stalls at
~1152 K and never converges. The JANAF discontinuity at `Tcommon = 1000 K`
(different ha values in low vs high range) may cause Newton to settle at a
spurious root. Possible causes:
- The standard JANAF coefficients used in the test (N₂ proxy, GRI-Mech 3.0)
  have a large discontinuity at Tcommon, so there is a jump in ha that Newton
  cannot cross.
- The DTMAX=500 K clamp combined with the discontinuity may pin the iterate
  near Tcommon indefinitely.

### `peng_robinson::tests::co2_nist_density_400k_10mpa`
PR EOS gives 163.1 kg/m³ vs NIST 197.6 kg/m³ (17% error at Pr = 1.36).
Much larger than the expected ≤ 8% PR EOS error.

### `peng_robinson::tests::n2_nist_density_300k_10mpa`
PR EOS gives 113.6 kg/m³ vs NIST 105.8 kg/m³ (7% error at Pr = 2.94).

### `peng_robinson::tests::n2_nist_density_200k_5mpa`
PR EOS gives 95.5 kg/m³ vs NIST 75.5 kg/m³ (26% error at Tr = 1.59, Pr = 1.47).

The three PR EOS failures may share a root cause — possibly the Z-root
selection, the Soave α-function, or a unit/constant mismatch in
`peng_robinson.rs`. Review against the OpenFOAM C++ source in
`src/thermophysicalModels/specie/equationOfState/PengRobinsonGas/`.

---

## Test backlog

> **Status note (bookkeeping pass, 2026-08-07).** The original framing —
> "must clear before adding downstream crates" — is **superseded**: 13 workspace
> crates already depend on this one, so this is now a live coverage backlog
> rather than a gate. The P0/P1/P2 priorities are kept for ordering. Items
> marked ✅ were verified present in the source during this pass; items with no
> mark were **not** checked in this pass and should be treated as unknown, not
> as absent. **No item here has had human V&V review.**

### 🔴 P0

#### `SquareMatrix::solve` failure-mode tests

- ✅ **Singular matrix** — `singular_matrix_returns_err` (`src/matrix/square_matrix.rs`).
- ✅ **Ill-conditioned (Hilbert n=5)** — `hilbert_5x5_residual_acceptable`. The
  `n=10` case was not found.
- **Scaled-partial-pivoting path** — construct a matrix where naïve pivoting fails but scaled pivoting succeeds; confirm correct result. *(not verified in this pass)*
- ✅ **API decision — DONE.** `SquareMatrix::solve` returns `Result<Vec<f64>, MatrixError>` (see `src/matrix/square_matrix.rs`), so singular matrices surface as `Err(MatrixError::Singular)` rather than being masked.

#### Newton `T(H)` iteration robustness (JANAF)

- ⚠️ Convergence from a deliberately bad `t0` — `newton_converges_from_bad_initial_guess` exists but is **`#[ignore]`d and failing** (see "Known test failures" above).
- ✅ Behaviour at the `T_MIN = 100 K` and `T_MAX = 6000 K` clamps — `newton_t_min_clamp_returns_err` / `newton_t_max_clamp_returns_err`.
- ✅ JANAF discontinuity at `Tcommon` — `newton_crosses_tcommon_discontinuity`.
- `MAX_ITER = 50` exhaustion path — must return `Err(NonConvergent)`, not silently return the last iterate. *(not verified in this pass)*

#### Mixture blending invariants

- `(a += b)` conserves mole fractions (sum to 1 before and after).
- Roundtrip: `t_from_ha(ha(p, T), p, T) ≈ T` to relative tolerance 1e-6.

> No multi-species mixture/blending module was found in this crate in this pass;
> these items may be misfiled here rather than merely untested.

---

### 🟠 P1

> The original heading read "Required before the FV operator port (Layer 3)".
> **Layer 3 is ported**, so this is now ordinary backlog priority.

#### Tensor algebra invariants

- ✅ `cross(a, b) · a == 0` and `cross(a, b) · b == 0` — `cross_orthogonal_to_both_inputs` (`src/primitives/vector.rs`).
- ✅ `T == symm(T) + skew(T)` — `symm_and_skew_sum_to_original` (`src/primitives/tensor.rs`).
- ✅ `det(T · T⁻¹) ≈ 1` and `(T⁻¹)⁻¹ ≈ T` — `inv_roundtrip_is_identity`.
- ✅ `inner(T1, T2) == inner(T2, T1)` — `double_inner_is_symmetric`.
- ✅ `SymmTensor::dev()` has trace 0 — `dev_traceless`.
- ✅ **`dev2` regression test** — `dev2_regression` (`src/primitives/symm_tensor.rs`). OpenFOAM's `dev2 = T − (2/3)·tr·I`, *not* the standard `(1/3)·tr·I`.

#### FV operator method-of-manufactured-solutions

These are the riskiest area of the port — test each operator in isolation on a mesh with a known analytic field:

- ✅ `fvc::grad(linear field)` — `linear_field_constant_x_grad` (`src/fv_operators/fvc/grad.rs`), and `fvc::grad_least_squares` is verified exact on a *non-orthogonal* mesh in `tests/non_orthogonal_laplacian.rs`.
- ✅ `fvm::laplacian(γ, T)` recovering an analytic `T` — in `src/fv_operators/fvm/laplacian.rs` (orthogonal), extended to non-orthogonal meshes with measured results in `tests/non_orthogonal_laplacian.rs`.
- `fvc::flux(U) → fvc::reconstruct → U` roundtrip on a divergence-free field. *(not verified in this pass)*
- Conservation: `Σ fvc::div(φψ) · V == boundary flux` (discrete divergence theorem). *(not verified in this pass)*

---

### 🟡 P2 — Robustness; defer if time-boxed

#### Polynomial root finding (`CubicEqn`) — ✅ covered

All four items below have tests in `src/polynomial/cubic_eqn.rs`:

- ✅ Triple root `(x − 2)³` — `triple_root`.
- ✅ One real + complex conjugate pair (negative discriminant) — `one_real_two_complex`.
- ✅ Near-zero leading coefficient degrading to `QuadraticEqn` — `degenerate_to_quadratic`.
- ✅ Correct `RootType` tagging — `one_real_two_complex_root_type_tags`, `degenerate_nan_tag`.

#### ODE solvers

- Linear decay `dy/dt = −λy` — compare to exact exponential reference across all solvers.
- Order verification: halve `dt`, confirm the global error drops by `2^p` (order `p` of each solver).
- Stiffness test (Van der Pol or Robertson) — `Rosenbrock23` must converge; `RKF45` is expected to be slow or fail; validates the stiff/non-stiff split.

> No order-verification or stiffness test was found in `src/ode/rkf45.rs` or
> `src/ode/rosenbrock23.rs` in this pass; treat this subsection as open.

#### `PengRobinsonGas` Z-root selection

> Open, and **the highest-value item in this file**: three NIST comparison tests
> are `#[ignore]`d and failing by 7–26 % (see "Known test failures" above), which
> is the symptom this subsection was written to diagnose.

- **Vapour branch:** largest real Z root must be selected.
- **Liquid branch:** smallest real Z root must be selected.
- **NIST reference points for validation** — test at least these two gases across a `(p, T)` grid:
  - **CO₂:** critical point `Tc = 304.13 K`, `pc = 7.377 MPa`, `ω = 0.2239`. Test at `(10 MPa, 320 K)` (supercritical), `(5 MPa, 280 K)` (liquid), `(1 MPa, 350 K)` (vapour). Reference densities from NIST WebBook.
  - **N₂:** `Tc = 126.19 K`, `pc = 3.396 MPa`, `ω = 0.0372`. Test at `(20 MPa, 300 K)`, `(5 MPa, 150 K)`, `(0.1 MPa, 300 K)`. Reference densities from NIST WebBook.
  - Target tolerance: `|ρ_PR − ρ_NIST| / ρ_NIST ≤ 3%` for points away from the critical point; accept wider tolerance within `|T − Tc| / Tc < 0.05`.
