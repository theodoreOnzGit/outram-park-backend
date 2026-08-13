# outram-foam-basic-lib

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping pass" command). A crate is **complete** only once the maintainer has personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.


> **This is OUTRAM PARK's independent Rust translation of selected OpenFOAM®
> algorithms.** It is not the official OpenFOAM® software and is not
> affiliated with, endorsed by, or sanctioned by OpenCFD Ltd. or the ESI
> Group. OpenFOAM® is a registered trademark of OpenCFD Limited — see
> [`TRADEMARKS.md`](./TRADEMARKS.md) for the full attribution and
> non-affiliation notice. Translated from
> [`OpenFOAM/OpenFOAM-dev`](https://github.com/OpenFOAM/OpenFOAM-dev),
> `master` branch — no commit is pinned (translation was done by reading the
> C++ source directly, not from an ongoing codegen-from-clone pipeline); see
> `upstream_source/README.md` for the full provenance record.

Pure-Rust translation of the OpenFOAM primitive and finite-volume library layer —
tensor algebra, polynomial solvers, ODE solvers, interpolation, thermophysics,
fields, mesh, FV operators, and fluid/solid thermodynamics needed to build
compressible and conjugate-heat-transfer CFD solvers.

## Quick start

```toml
[dependencies]
outram-foam-basic-lib = "0.1.0"
```

```rust
use outram_foam_basic_lib::prelude::*;

// Tensor algebra
let u = Vector3::new(1.0, 0.0, 0.0);
let v = Vector3::new(0.0, 1.0, 0.0);
let cross = u.cross(v);              // z-axis unit vector

// Polynomial root finding
let cubic = CubicEqn::new(1.0, -6.0, 11.0, -6.0); // (x-1)(x-2)(x-3)
let roots = cubic.roots();

// Dense LU solve (no external BLAS required)
let mut m = SquareMatrix::new(3);
m.set(0, 0, 2.0); m.set(0, 1, 1.0); m.set(0, 2, 0.0);
m.set(1, 0, 1.0); m.set(1, 1, 3.0); m.set(1, 2, 1.0);
m.set(2, 0, 0.0); m.set(2, 1, 1.0); m.set(2, 2, 4.0);
let x = m.solve(&[7.0, 10.0, 15.0]);  // returns Result<Vec<f64>, MatrixError>

// FV operators
use outram_foam_basic_lib::fv_operators::{fvm, fvc};
// fvm::ddt, fvm::laplacian, fvm::div, fvm::laplacian_vec, fvm::div_vec,
// fvm::ddt_coeff, fvm::ddt_coeff_vec
// fvc::grad, fvc::div, fvc::flux, fvc::reconstruct, fvc::buoyancy_flux, ...
```

## What's implemented

### Cross-layer — hybrid execution backend

Dispatch only; no kernels live here. Both accelerator features are **off by
default**, so a plain build pulls neither `rayon` nor `wgpu` and stays
Android/Termux clean. `ComputeBackend::Serial` is always compiled and is the
reference implementation the other backends are verified against.

| Module | Rust type / fn | Notes |
|---|---|---|
| `compute` | `ComputeBackend` | `Serial` / `CpuMulti` / `Gpu`. `Serial` is the `#[default]` and the oracle. `resolve()` degrades an unavailable choice instead of failing, so a missing GPU adapter is a normal outcome, not an error. |
| `compute` | `select_backend` | The single named backend-selection policy — the size rule lives in one findable function rather than scattered inline thresholds. |
| `compute` | `ThreadCount` | `Auto` / `Fixed(n)` / `Fraction(f)` worker sizing for `CpuMulti`; always resolves to `>= 1`. |
| `compute` | `gpu_adapter_present` | One-shot cached adapter probe. Never panics; `false` on Android, without the `gpu` feature, or with no adapter. |
| `compute` | `CPU_MULTI_MIN_WORK_ITEMS`, `GPU_MIN_WORK_ITEMS` | Crossover thresholds, **unmeasured placeholders** (bead `op-yvj.4.7`). Measurement has already shown `CPU_MULTI_MIN_WORK_ITEMS = 4096` to be ~6x *slower* than serial for the memory-bound field kernels, which override it — treat it as a floor against absurdity, not a tuned value. |
| `fields::parallel` | element-wise `add`/`sub`/`scale`/`axpy` (+ `_assign`), `pointwise_mul`/`_div`, `scale_by_field`, `dot_field` | `ComputeBackend`-dispatched, generic over `f64`/`Vector3`/`Tensor`/`SymmTensor`. Bit-identical to `Serial` on every backend and size. |
| `fields::parallel` | reductions `sum`, `mean`, `min`, `max`, `l2_norm`, `dot` | Fixed-chunk tree reduction (`REDUCTION_CHUNK = 4096`), summed in index order: **bit-reproducible run-to-run and across thread counts**, but not bit-equal to `Serial` — measured worst-case relative deviation `7.857e-14`, asserted tolerance `1e-11`. `min`/`max` *are* bit-identical. |
| `fields::parallel` | `{add,sub,scale}_vol(_assign)`, `axpy_vol_assign`, `{add,sub,scale}_surface`, `axpy_surface_assign` | Field wrappers; each copies the left operand's name verbatim. Guarded by three name-growth regression tests (64/256-round self-referential reassignment) against the 2^step bug that once cost 24 GB and a SIGTERM. |
| `fields::parallel` | `vol_integral`, `vol_average`, `vol_l2_norm`, `vol_min`, `vol_max` | Mesh-volume-weighted; `vol_integral` carries units `[phi]·m³`. Interior cells only — boundary patches excluded by design. |
| `fields::parallel` | `should_parallelise`, `field_parallel_crossover`, `FIELD_PARALLEL_CROSSOVER` | The single dispatch decision point for this module; no operator has its own size test. Crossover **measured at 131 072** on 4 cores — but it is a band (65 536–262 144), not a point. `Gpu` routes to the best CPU path: there is no GPU field kernel. |
| `ldu_matrix::parallel` | `HybridLdu`, `LduTopology` | Cell-gather SpMV, residual, and diagonal reciprocal on the hybrid backend. `LduTopology` inverts the face addressing into a per-cell CRS list so each output has exactly one writer — no atomics, no colouring. Faces are stored in **ascending index**, the order the serial scatter visits them, so the parallel result is **bit-for-bit identical** to `LduMatrix::multiply`, not merely close. Verified by `to_bits()` at 24/512/32 768 cells and 1/2/4/8 threads. |
| `ldu_matrix::parallel` | `dot`, `axpy`, `norm_l1`, `norm_l2` | Krylov vector operations. Fixed 1024-element blocks combined in block order: bitwise identical across backends and thread counts, but **not** bitwise equal to `krylov::vecops`' flat sums — worst measured `dot` deviation `2.3744e-13` raw at n = 4.19 M, `7.9476e-17` when scaled by Σ\|aᵢbᵢ\| (the raw figure inflates because the test vectors cancel). |
| `ldu_matrix::parallel` | `SPMV_MIN_CELLS` = 4 096, `VECOP_MIN_ELEMENTS` = 262 144 | **Measured, and they differ by 64×** — one crate-wide threshold would be wrong for both. SpMV breaks even exactly at 4 096 cells; `axpy` at 4 096 elements runs at **0.05×**, twenty times slower, and is reliable only from ~1 M. |
| `math::parallel` | `solve_bracketed_batch`, `solve_newton_batch` | Batched root finding over a caller-supplied residual — `RootMethod::{Bisection, Brent}`, plus bracket-safeguarded Newton (Newton step only while it stays in the bracket and at least halves it; bisection otherwise). **Bitwise identical to serial at 1/2/4/8 threads**, asserted on a deliberately imbalanced batch: a root batch has no cross-lane arithmetic, so there is no summation order to perturb. |
| `math::parallel` | `RootSolution`, `RootBatch`, `RootStatus` | Non-convergence is per-lane and hard to ignore by construction: `root()` returns `Some` only when converged, the raw value sits behind `last_iterate()`, and a failed bracket reports `NaN` — **never a clamped bracket endpoint**, which is asserted. |
| `math::parallel` | `{linear,quadratic,cubic}_roots_batch` | Batched closed-form roots, wrapping the existing `polynomial` scalar solvers rather than reimplementing them. No OpenFOAM equivalent — upstream has no batched root finder. |
| `math::parallel` | `ROOT_BATCH_MIN_PROBLEMS` = 256, `POLY_ROOTS_MIN_EQUATIONS` = 1 024 | Measured. **16× *below* the crate placeholder**, the opposite direction to the field kernels — root finding is compute-dense per lane where field algebra is memory-bound. The iterative crossover genuinely depends on the caller's residual cost, so no constant can be right for every caller. |
| `math::minimise` | `golden_section_batch`, `Sense` | Batched 1-D golden-section extremum search. **Generalised from `tampines-steam-tables`' `golden_section_max_g`** (cited to Price & Robertson, 2012) rather than written afresh. `Sense::Maximise` is first-class because the production caller maximises; nothing is negated internally, so returned values keep the caller's sign. Bitwise identical `Serial`/`CpuMulti` at 1/2/4/8 threads. |
| `math::minimise` | `MinProblem`, `MinSettings`, `MinStatus`, `MinSolution`, `MinBatch` | Mirrors `math::parallel`'s root-finding vocabulary 1:1, with three deliberate divergences: **no `f_tol`** (a value criterion is wrong for minimisation — values agree to second order before arguments do), **no `NotBracketed` status** (unimodality cannot be checked from finitely many evaluations, so a status implying it would promise a guarantee that does not exist), and **`extremum()` vs `extremal_value()`** kept apart, since conflating argmin with min value is the classic minimisation bug. |
| `math::minimise` | `MINIMISE_BATCH_MIN_PROBLEMS` = 256, `SQRT_EPSILON` | Crossover measured at 256, the same as root finding — both compute-bound per lane. **Accuracy floor is not a constant of the method**: it is `sqrt(2·eps·\|f(x*)\|/\|f''(x*)\|)`, a property of the objective near its own extremum. Measured worst \|x−x0\|: `1.053671e-8` for `1+(x−x0)²`, `1.026485e-4` for `1+(x−x0)⁴` — **9 742× worse** — and `8.881784e-16` when the minimum value is exactly zero. |
| `math::minimise` | *(caller precondition)* | **Unimodality is unchecked and uncheckable.** On a multimodal bracket the search returns *a* local extremum, reporting `Converged`. A test demonstrates this rather than only warning about it: given a deep narrow well beside a shallow wide one, it returns the **shallow** one in 41 iterations, half as deep as the true global minimum. |
| `ode::parallel` | `integrate_ensemble`, `integrate_ensemble_mixed` | N independent IVPs across `ComputeBackend`, driving the crate's **existing** `Euler`/`Rkf45`/`Rosenbrock23` — no new integrator written. Bitwise identical to serial at 1/2/4/8 threads; each lane clones the stepper prototype so identity does not depend on every stepper's buffer discipline. Crossover **16 lanes** (measured) — the lowest in the crate. |
| `ode::parallel` | `quadrature_batch`, `QuadratureRule`, `GaussOrder` | Batched composite trapezoid / Simpson / Gauss-Legendre. **Gauss nodes are computed** by Newton on the Legendre recurrence, not transcribed — they agree with the in-workspace A&S 25.4.30 table to `1.110223e-16` (nodes) and `1.249001e-16` (weights), so this is an independent route to a published result and adds no literature dependency. Exact to degree `2n−1`, verified: worst relative error `5.769990e-16` over G2–G8. Crossover **32 intervals**. |
| `ode::parallel` | `adaptive_quadrature_batch`, `AdaptiveSettings` | Adaptive Simpson per lane; CPU-only by design (divergent control flow). **Cannot handle an endpoint singularity** — it evaluates `f(a)`/`f(b)` on the first step, so `ln(x)` at `x=0` gives `NotFinite`. This inverts the naive expectation: for endpoint singularities use `GaussLegendre`, whose nodes are strictly interior. |
| `ode::parallel` | `OdeEnsemble`, `QuadratureBatch`, `*Status`, `*Failure` | Per-lane failure reporting. A budget-failed lane keeps its **genuine partial state at `x_reached`**, not at `x_end` — so a truncated integration cannot be mistaken for a completed one. Stiffness is never switched behind the caller's back: `Rkf45` on a stiff pair fails visibly at 10 001 steps having reached only `x = 3.638735e-1`, where `Rosenbrock23` completes in 1 568. |
| `tests/` | `hybrid_parity.rs` | Cross-cutting parity gate over **47 kernel outputs** from all five hybrid-backend modules, driven by one `run_all_kernels(backend)` inventory — a new kernel is covered by every gate the moment it is added there. Asserts bitwise identity by `to_bits()` for the 40 kernels that claim it (never a tolerance, which would silently accept a regression from bitwise to merely-close), and the module's own documented tolerance for the 7 that legitimately re-associate. Batched-solver outputs are compared including **iteration counts and status codes**, so a changed algorithm fails even when the answers agree. Also gates thread-count invariance at 1/2/4/8 workers and carries a `Gpu`-degradation tripwire. |

### Layers 1a–1h — Primitives and thermophysics

| Module | Rust type / fn | Notes |
|---|---|---|
| `primitives` | `Vector3`, `Tensor`, `SymmTensor`, `SphericalTensor` | Full OpenFOAM tensor algebra; `SMALL`, `VSMALL`, `GREAT`, etc. |
| `primitives::eigen` | `eigen_values`, `eigen_values_symm`, `eigen_vectors`, `eigen_vectors_symm` | Spectral decomposition of 3x3 tensors via the characteristic cubic (reuses `CubicEqn`). Ascending eigenvalues, orthonormal eigenvector rows, degenerate and triple-eigenvalue fallbacks. The basis every isotropic tensor function (log/exp/sqrt) is built on. |
| `polynomial` | `LinearEqn`, `QuadraticEqn`, `CubicEqn` | FMA-accurate discriminants; all root branches |
| `polynomial` | `Polynomial<const N>` | Horner eval, derivative, integral, integral_minus1 (log term) |
| `polynomial` | `Roots<const N>`, `RootType` | 3-bit-per-root type encoding |
| `math` | `erf_inv`, `inc_gamma_ratio_p/q`, `inc_gamma_p/q`, `inv_inc_gamma` | DiDonato–Morris (1986) |
| `matrix` | `SquareMatrix` | Row-major n×n; LU with scaled partial pivoting; `lu_decompose`, `lu_back_substitute`, `solve` |
| `ode` | `Euler`, `Rkf45`, `Rosenbrock23` | Adaptive explicit (RKF45) and stiff (W-method) solvers; `OdeSystem` trait |
| `ode::integrator` | `OdeSolver`, `OdeIntegrator`, `TypedStateIntegrator`, `DynSystemIntegrator`, `SharedOdeSystem`, `NoTypedSystem` | No C++ counterpart — replaces OpenFOAM's `ODESolver::New` runtime-selection table. Enum dispatch over stepper choice *and* over how the system is owned: `TypedState` (concrete system by value, static dispatch, preferred) or `DynSystem` (`Arc<dyn OdeSystem + Send + Sync>`, kept by maintainer decision for flexibility). Neither borrows, so an integrator is storable as a struct field with no lifetime parameter. |
| `interpolation` | `interpolate_xy`, `interpolate_spline_xy` | Linear and Catmull-Rom cubic 1-D |
| `thermophysics::eos` | `PerfectGas`, `RhoConst`, `IcoPolynomial<N>`, `PengRobinsonGas` | `EquationOfState` trait; ρ, ψ, Z, departure functions |
| `thermophysics::thermo` | `HConstThermo`, `JanafThermo`, `HPolynomialThermo<N>`, `HTabulatedThermo` | `ThermoModel` trait; Cp, Ha, Hs, S; Newton T(H) iteration |
| `thermophysics::transport` | `ConstTransport`, `SutherlandTransport`, `PolynomialTransport<N>`, `TabulatedTransport` | `TransportModel` trait; μ, κ |

### Layer 2 — Fields and mesh

| Module | Rust type | Notes |
|---|---|---|
| `fields` | `Field<T>`, `VolField<T>`, `SurfaceField<T>` | Generic field containers; `BoundaryCondition<T>`, `PatchField<T>` |
| `fields::boundary` | `BoundaryCondition::FixedGradient` | `fixedGradientFvPatchField.H` — Neumann `φ_face = φ_cell + g·delta` |
| `fields::boundary` | `BoundaryCondition::Mixed` | `mixedFvPatchField.H` — Robin blend of fixedValue (weight `value_fraction`) and fixedGradient; also the albedo BC |
| `fields::boundary` | `BoundaryCondition::InletOutlet` | `inletOutletFvPatchField.H` — fixedValue on inflow, zeroGradient on outflow (flux-switched on `φ_f=U·Sf`) |
| `fields::boundary` | `BoundaryCondition::OutletInlet` | `outletInletFvPatchField.H` — fixedValue on outflow, zeroGradient on inflow |
| `fields::boundary` | `BoundaryCondition::Slip` | `slipFvPatchField.H` — vector: normal component removed, tangential zeroGradient; scalar: zeroGradient |
| `fields::boundary` | `BoundaryCondition::NoSlip` | `noSlipFvPatchField.H` — velocity fixedValue = 0 |
| `fields::boundary` | `BoundaryCondition::Wedge` | `wedgeFvPatchField.H` — axisymmetric wedge; **first pass: zeroGradient stand-in** (rotation transform not yet implemented) |
| `fields::boundary` | `BoundaryCondition::Freestream` | `freestreamFvPatchField.H` — far-field inletOutlet: `freestreamValue` on inflow, zeroGradient on outflow (flux-switched); self-contained |
| `fields::boundary` | `BoundaryCondition::PressureInletOutletVelocity` | `pressureInletOutletVelocityFvPatchVectorField.H` — outflow zeroGradient, inflow `U=(φ_f/\|S_f\|)·n̂`; solver refreshes values via `update_pressure_inlet_outlet_velocity` |
| `fields::boundary` | `BoundaryCondition::FixedFluxPressure` | `fixedFluxPressureFvPatchScalarField.H` — fixedGradient `snGrad(p)=(φ_HbyA−φ_target)/(D_p·\|S_f\|)`; solver-set gradient (`fixed_flux_pressure_sn_grad`) |
| `fields::boundary` | `BoundaryCondition::TotalPressure` | `totalPressureFvPatchScalarField.H` — `p=p0−0.5ρ\|U\|²` (incompressible; compressible deferred); cross-field, solver refreshes via `update_total_pressure` |
| `fields::boundary` | `BoundaryCondition::FlowRateInletVelocity` | `flowRateInletVelocityFvPatchVectorField.H` — uniform inlet `U=−(Q/A_patch)·n̂`, `A_patch=Σ\|S_f\|`; solver/geometry-driven via `update_flow_rate_inlet_velocity` |
| `fields` | `VolScalarField`, `VolVectorField`, `VolTensorField`, `VolSymmTensorField` | Typed aliases |
| `fields` | `SurfaceScalarField`, `SurfaceVectorField` | Face-centred typed aliases |
| `mesh` | `FvMesh`, `FvMeshBuilder`, `BoundaryPatch`, `PatchKind` | Unstructured polyhedral mesh |
| `mesh` | `BoundaryPatch::new_cyclic`, `CyclicCoupling`, `FvMesh::periodic_1d` | `cyclicPolyPatch` — cyclic (periodic) patch pairing: matched half0/half1 faces + across-seam owner↔owner cell couplings; `periodic_1d` builds a 1-D periodic ring programmatically |
| `mesh::ami` | `AmiCoupling`, `AmiWeight`, `AmiOverlap`, `overlap_weights_1d`, `FvMesh::periodic_ring_ami`, `PatchKind::CyclicAmi` | `cyclicAMIPolyPatch` / `AMIInterpolation` — **non-conformal** periodic (arbitrary-mesh-interface) seams: each target face couples to a geometric-overlap-weighted set of source cells (Σ weights = 1, conservative). `overlap_weights_1d` computes planar/1-D-structured interval overlaps; `periodic_ring_ami` builds a non-conformal periodic ring programmatically. **First pass: 1-D/planar-structured overlap only** — general 3-D polygon clipping, two-axis tiling, and rotational transforms deferred |
| `mesh` | `RegionInterface` | Matching and non-matching multi-region face coupling for CHT |
| `ldu_matrix` | `LduMatrix`, `FvMatrix`, `FvVectorMatrix` | Sparse LDU system; scalar and vector implicit equation assembly |
| `ldu_matrix` | `gauss_seidel`, `conjugate_gradient` | Iterative LDU solvers (no external BLAS). CG is **DIC-preconditioned** (`Foam::DICPreconditioner`) and accepts an optional initial guess (`x0`) for warm starts |
| `ldu_matrix` | `gamg` | Algebraic multigrid (`Foam::GAMGSolver` + `algebraicPairGAMGAgglomeration`) for symmetric systems; near mesh-independent V-cycle count |
| `ldu_matrix` | `FvMatrix::solve_cg`, `FvMatrix::solve_cg_with_guess` | Cold- and warm-started PCG for the symmetric pressure system |
| `ldu_matrix` | `FvMatrix::solve_gamg`, `FvMatrix::solve_gamg_with_guess` | Cold- and warm-started GAMG for the symmetric pressure system |
| `ldu_matrix` | `SolverSettings`, `SolverPerformance` | Tolerance / iteration control and convergence reporting |
| `ldu_matrix` | `krylov_solve`, `KrylovMethod`, `KrylovOptions`, `PreconditionerKind` | Bridge from `SolverSettings`/`SolverPerformance` onto the `krylov` module's BiCGStab/GMRES kernels; method and preconditioner selected by `Copy` enum (no trait objects) |
| `ldu_matrix` | `FvMatrix::solve_bicgstab{,_with_guess}`, `FvMatrix::solve_gmres{,_with_guess}`, `FvMatrix::solve_krylov` | Preconditioned Krylov solve for the **asymmetric** (convection-bearing) scalar system — the analogue of `Foam::PBiCGStab` + `DILU`, where PCG/GAMG do not apply. Measured V&V in `tests/krylov_convection_diffusion.rs` |
| `ldu_matrix` | `FvVectorMatrix::solve_bicgstab`, `solve_gmres`, `solve_krylov` | Same, per velocity component, for the asymmetric momentum matrix |
| `krylov` | `bicgstab`, `gmres` | Pure-Rust Krylov solvers for **nonsymmetric** LDU systems (analogue of `Foam::PBiCGStab`); GMRES(m) is right-preconditioned. Added for the pflotran RICHARDS Jacobian, which is asymmetric under upstream weighting |
| `krylov` | `Preconditioner` (`Identity`/`Jacobi`/`Ilu0`) | Enum-dispatched preconditioners; ILU(0) is a genuine incomplete-LU (exact for tridiagonal), Jacobi the robust fallback |
| `krylov` | `KrylovSettings`, `KrylovResult`, `vecops` | Tolerance/restart control, convergence reporting, and BLAS-1 helpers (`dot`/`nrm2`/`axpy`/`scal`) |
| `limiters` | `FluxLimiter` (`psi(r)`) | Field-agnostic TVD flux limiters translated from OpenFOAM `limitedSchemes/` (`vanLeer`/`vanAlbada`/`Minmod`/`SuperBee`/`MUSCL`/`UMIST`/`OSPRE`/`limitedLinear`). r-based only; NVD schemes (QUICK/Gamma) omitted. Reusable by any FV code (e.g. pflotran transport TVD) |

### Layer 3 — Finite-volume operators

| Function | Description |
|---|---|
| `fvm::ddt(phi, phi_old, dt)` | Implicit Euler ∂φ/∂t → `FvMatrix` |
| `fvm::ddt_coeff(coeff, phi, phi_old, dt)` | Density/rho_cp-weighted implicit ddt: ∂(coeff·φ)/∂t → `FvMatrix` |
| `fvm::ddt_vec(U, U_old, dt, mesh)` | Implicit Euler ∂U/∂t → `FvVectorMatrix` |
| `fvm::ddt_coeff_vec(coeff, U, U_old, dt, mesh)` | Density-weighted implicit ddt: ∂(ρU)/∂t → `FvVectorMatrix` |
| `fvm::laplacian(gamma, phi)` | Diffusion −∇·(γ∇φ) → `FvMatrix` (couples cyclic/periodic seam faces as internal faces via `cyclicFvPatchField`; **cyclicAMI** non-conformal seams as overlap-weighted partial internal faces via `cyclicAMIFvPatchField`) |
| `fvm::laplacian_vec(gamma, U)` | Diffusion −∇·(γ∇U) → `FvVectorMatrix` (cyclic + cyclicAMI seam coupling) |
| `fvm::div(phi, psi)` | Upwind convection ∇·(φψ) → `FvMatrix` (cyclic + cyclicAMI seam coupling; per-target flux split by overlap weight) |
| `fvm::div_vec(phi, U)` | Upwind convection ∇·(φU) → `FvVectorMatrix` (cyclic + cyclicAMI seam coupling) |
| `fvm::laplacian_corrected(gamma, phi, grad_phi, scheme)` | Laplacian with **non-orthogonality correction** (`Foam::correctedSnGrad`): implicit `1/max(n·d, 0.05\|d\|)` part + explicit `k_f·(∇φ)_f`, scheme selected by the `NonOrthoScheme` enum (`Orthogonal` / `Corrected` / `Limited(w)`); `Orthogonal` is bit-for-bit `fvm::laplacian`. Internal + pure-Dirichlet boundary faces only — cyclic/cyclicAMI seams and the other BC arms keep the orthogonal treatment |
| `fvm::solve_laplacian_non_orthogonal(gamma, phi, scheme, n_correctors, settings)` | The deferred-correction loop (`nNonOrthogonalCorrectors`): solve → recompute least-squares gradient → re-assemble → re-solve |
| `fvm::max_non_orthogonality_deg(mesh)` / `fvm::non_ortho_geometry(sf, d)` / `NonOrthoGeometry` | `checkMesh`-equivalent max non-orthogonality statistic, and the per-face `Δ_f` / `k_f` / angle it is computed from |
| `fvc::grad(phi)` | Explicit cell-centred Gauss gradient → `VolVectorField`. **Not exact for a linear field on a non-orthogonal mesh** — see `grad_least_squares` |
| `fvc::grad_least_squares(phi)` | Least-squares cell gradient (`Foam::leastSquaresGrad`), inverse-distance weighted, with rank repair for 2-D/1-D meshes. Exact for a linear field on **any** mesh; the gradient the non-orthogonal correction must be driven by. V&V in `tests/non_orthogonal_laplacian.rs` |
| `fvc::div(phi, psi)` | Explicit scalar divergence → `VolScalarField` |
| `fvc::div_flux(phi)` | Divergence of face flux → `VolScalarField` |
| `fvc::interpolate(phi)` | Linear face interpolation → `SurfaceScalarField` (interpolates across the cyclic seam to the paired cell) |
| `fvc::sn_grad(phi)` | Surface-normal gradient → `SurfaceScalarField` (gradient across the cyclic seam to the paired cell) |
| `fvc::flux(U)` | Face flux φ = U·Sf → `SurfaceScalarField` |
| `fvc::reconstruct(phi)` | Least-squares VolVectorField from face flux → `VolVectorField` |
| `fvc::ddt_corr(U_old, phi_old, dt)` | PISO flux consistency correction → `SurfaceScalarField` |
| `fvc::buoyancy_flux(rho, g)` | ρ_f·(g·Sf) per face → `SurfaceScalarField` |
| `adjust_phi(phi, U)` | Global mass-balance correction |

### Layer 3 — Optional equation sources (`fvOptions` / `fvModels`)

Terms a *case* attaches to an equation the *solver* knows nothing about. ESI
OpenFOAM calls the mechanism `fvOptions`; the OpenFOAM Foundation split it into
`fvModels` (sources) and `fvConstraints` (constraints). This port follows the
Foundation split and implements the source half; constraints are not yet ported.

| Module | Rust type / fn | Notes |
|---|---|---|
| `fv_options` | `FvModel`, `FvModels` | `fvModel` / `fvModels` — the model enum (closed set, enum dispatch rather than upstream's runtime-selection table) and the collection a solver hands to each equation. `FvModels::add_source_scalar` / `add_source_vector` place terms into an `FvMatrix` / `FvVectorMatrix` |
| `fv_options` | `SourceContribution`, `EquationField` | Explicit (`Su`) / implicit (`Sp`) split, **per unit volume**; models are attached to equations by solved-field name, as upstream does |
| `fv_options` | `CellSelection` | `fvCellZone` / `cellSetOption` — whole mesh or an explicit cell list, shared behind an `Arc` |
| `fv_options` | `SemiImplicitSource` | `semiImplicitSource` — a general explicit + implicit source on one named field |
| `fv_options` | `SolidificationMelting`, `SolidificationMeltingCoefficients` | `solidificationMelting` (ESI: `solidificationMeltingSource`) — enthalpy-porosity melting: liquid fraction with a eutectic-shifted effective liquidus, Carman-Kozeny Darcy momentum sink `-Cu(1-α₁)²/(α₁³+q)`, Boussinesq buoyancy, and latent heat `∂(ρα₁)/∂t` in either the temperature or enthalpy form. Upstream's once-per-timestep `curTimeIndex_` update guard is reproduced via `advance_time`. **Deviation:** signs are *not* a literal transcription of upstream's `addSup` — see below. `Cp` is a coefficient (upstream's `CpRef` mode); the thermophysical-model lookup and mesh topology changes are not ported |
| `fv_options` | `TemperatureTable` | The `table` entry of `Function1<scalar>`, as consumed by `porosityModels::solidification` (`D`) and `VoFSolidificationMelting` (`alphaSolidT`) — ascending temperature knots, linear interpolation, end-clamping. Not a general `Function1` port: constants, polynomials, CSV and coded entries are out of scope |
| `fv_options` | `SolidificationPorosity`, `MomentumEquationForm` | `porosityModels::solidification` — solidification as a bare temperature-tabulated Darcy blockage `S = -α·ρ·D(T)·U`. **No latent heat and no buoyancy**; upstream states the temperature is unchanged by the modelled phase change. Upstream's runtime `UEqn.dimensions() == dimensions::force` branch becomes the explicit `MomentumEquationForm` enum. **Note the sign contrast with `SolidificationMelting`:** this one is handed the solver's own `UEqn` and mutates it in place, so upstream's `+=` with a *positive* `D` is transcribed unchanged |
| `fv_options` | `VofSolidificationMelting` | `fv::VoFSolidificationMelting` (compressibleVoF module) — VoF variant keyed on a solid-fraction-vs-temperature table; its solid fraction is driven by a VoF phase fraction supplied through `FvModels::correct` rather than lazily from `addSup` |

**Sign convention, and why this port differs from upstream line-by-line.**
Upstream's `addSup` writes into an intermediate `fvModels` matrix that the
solver then *subtracts*, since `solve(UEqn == fvModels.source(...))` and
`operator==(A, B)` expands to `A - B`. Every coefficient an `addSup` writes is
therefore negated before it reaches the system actually solved. This port places
terms into the solved system directly — `source += V·explicit`,
`diag -= V·implicit` — so it reproduces upstream's *solved equation*, not its
intermediate matrix. Transcribing `Sp[celli] += Vc*S` literally would invert the
Darcy drag into a momentum source inside the solid and destroy diagonal
dominance exactly where the coefficient is largest; two tests in
`fv_options/solidification_melting/tests.rs` exist specifically to catch that.

### Layer 4 — Field-level fluid thermodynamics

| Type | Description |
|---|---|
| `FluidThermo` | Trait: `rho`, `mu`, `kappa`, `alpha_h`, `T`, `he`, `update` |
| `PsiThermo<M>` | Compressible ψ-based thermo for sonicFoam / rhoPimpleFoam: ρ = ψ·p |
| `RhoThermo<M>` | Density-based thermo: ρ from EOS |
| `SolidThermo` | Trait for solid region CHT: `rho_cp`, `kappa`, `T` |
| `ConstSolidThermo` | Constant-property solid thermo |

## Limitations

Read this before depending on the crate. Every item below is grounded in the
current source; nothing here is aspirational. See also the
[unverified-until-validated banner](#outram-foam-basic-lib) at the top.

### Scope boundaries

- **Layers 1–4 only — no solver loops.** This crate provides the mathematical
  building blocks (tensor algebra, primitives, FV operators, thermophysics
  kernels, field thermodynamics). The **Layer 5** logic — PISO/PIMPLE loops,
  multi-region coupling drivers, turbulence-model registries — lives in separate
  downstream crates (`openfoam-icof`, `openfoam-cht`, `openfoam-rho`), which are
  **not yet published**. On their own the operators here do not run a
  time-marching CFD simulation.
- **No turbulence models.** `LaminarModel` / `kOmegaSST` and `divDevRhoReff` are
  planned for the CHT solver crate, not present here. Only laminar terms can be
  assembled.
- **No OpenFOAM case / `polyMesh` file I/O.** Meshes are constructed in code via
  `FvMeshBuilder` (you supply `owner`/`neighbour` connectivity and the geometric
  Vecs) or via `interface::one_dimensional_meshing::create_one_d_mesh` for 1-D
  system-code meshes. Reading an OpenFOAM `constant/polyMesh` directory is a
  downstream concern, not in this crate. (The DIC/warm-start discussion below
  refers to a `read_poly_mesh` that lives in the appbuilder crate.)
- **Serial only.** No MPI / domain decomposition; there are no processor
  boundary patches. The linear solvers and operators run single-threaded on one
  mesh.

### Finite-volume numerics

- **Implicit convection is first-order upwind only.** `fvm::div` / `fvm::div_vec`
  assemble a first-order upwind matrix (`src/fv_operators/fvm/div.rs`). There is
  **no** implicit higher-order/TVD convection (`linearUpwind`, `limitedLinear`,
  `vanLeer`, etc.). Second-order TVD *reconstruction* exists only explicitly, via
  `fvc::muscl` (`Upwind` / `Linear` / limiter variants), intended for
  density-based central-upwind fluxes — the Kurganov–Tadmor flux assembly itself
  is Layer-5 work and is not wired up here.
- **Laplacian is Gauss-orthogonal only — no non-orthogonal / skewness
  correction.** `fvm::laplacian` and `fvc::sn_grad` use the orthogonal
  face-area-over-centre-distance coefficient
  (`src/fv_operators/fvm/laplacian.rs`), ignoring the angle between the face-area
  vector and the owner→neighbour vector. Diffusion is therefore accurate on
  orthogonal meshes (hex / 1-D) but **under-resolved on non-orthogonal or skewed
  meshes**; there is no over-relaxed/minimum-correction non-orthogonal term.
- **Time integration is first-order.** Only implicit Euler `fvm::ddt` (plus
  `fvm::ddt_coeff`, the vector forms, and a `d2dt2`) is provided. No
  higher-order/backward/Crank–Nicolson time schemes.
- **Boundary conditions are a closed enum set.** `BoundaryCondition`
  (`src/fields/boundary/bc.rs`) supports `FixedValue`, `FixedField`,
  `ZeroGradient`, `FixedGradient` (non-zero), `Mixed`/Robin, `InletOutlet`,
  `OutletInlet`, `Slip`, `NoSlip`, `Wedge` (zero-gradient stand-in),
  `Symmetry`, `Empty`, `Calculated`, and the flow-context BCs `Freestream`
  (self-contained, flux-switched), `PressureInletOutletVelocity`,
  `FixedFluxPressure`, `TotalPressure`, and `FlowRateInletVelocity` (the last
  four **solver-driven** — the solver refreshes their stored values/gradient
  each iteration via the documented `update_*` / `*_value` hooks). **Cyclic
  (periodic) patches** are now
  functional at the topology/operator level — `PatchKind::Cyclic` patch pairs
  couple across the seam like internal faces in `fvm::laplacian(_vec)` /
  `fvm::div(_vec)` / `fvc::interpolate` / `fvc::sn_grad` (build one with
  `FvMesh::periodic_1d` or `BoundaryPatch::new_cyclic`). Still **no**
  wall-function or processor BC, and the `polyMesh` parser does **not yet read
  the cyclic `neighbourPatch` ordering**, so cyclic pairs must be wired
  programmatically (not from a read `constant/polyMesh`) for now. Cyclic support
  is an **untrusted AI-assisted draft pending human V&V** (verified against the
  equivalent all-internal ring mesh — see the `vv_cyclic_*` tests — not yet
  human-reviewed).
- **Non-conformal periodic (`cyclicAMI`) patches** are functional at the
  topology/operator level for the **1-D / planar-structured** case
  (`PatchKind::CyclicAmi`, `mesh::ami`). Each target seam face couples to a
  geometric-overlap-weighted set of source cells (weights sum to 1,
  conservative) in `fvm::laplacian(_vec)` / `fvm::div(_vec)`; build a
  non-conformal periodic ring with `FvMesh::periodic_ring_ami`. **Deferred
  (documented in `mesh::ami`):** general 3-D polygon-clipping overlap, two-axis
  transverse tiling, rotational-transform AMI, `fvc::interpolate`/`sn_grad`
  across AMI seams, and reading `cyclicAMI` from a `polyMesh`. Verified in the
  matching-mesh limit (AMI == plain cyclic) and for a 2:1 non-conformal
  conservation case (`vv_ami_*` tests); **untrusted AI-assisted draft pending
  human V&V** — not yet human-reviewed.

### Linear solvers

- **Krylov / multigrid solvers assume symmetric positive-definite systems.**
  `conjugate_gradient` (DIC-preconditioned) and `gamg` target the symmetric
  pressure system. Asymmetric systems fall back to `gauss_seidel`; there is no
  BiCGStab/GMRES.
- **GAMG rebuilds its hierarchy on every solve** and is **slower than DIC-PCG
  below ~10^4–10^5 cells** (see the performance section below). It is
  mesh-independent in V-cycle count but not yet cached across time steps or usable
  as a CG preconditioner. PCG remains the default.
- **Dense matrices provide LU only.** `SquareMatrix` implements LU with scaled
  partial pivoting (`solve` returns `Result<Vec<f64>, MatrixError>`). The
  QR / Cholesky / SVD factorisations named in the design doc are **not
  implemented**.

### Thermophysics accuracy (known-failing / unverified)

- **Peng–Robinson EOS is inaccurate above the critical pressure.** Density errors
  of **7–26 % vs NIST** are observed for `Pr > 1` / near-critical states; the
  corresponding NIST cross-checks are `#[ignore]`-d pending a suspected
  Z-root-selection or α-function fix (`src/thermophysics/eos/peng_robinson.rs`,
  `docs/porting-roadmap.md`). Prefer `PerfectGas`, `RhoConst`, or `IcoPolynomial`
  where density accuracy matters.
- **JANAF `T(H)` inversion can fail to converge from a far initial guess.** The
  Newton solve stalls crossing the `Tcommon = 1000 K` coefficient discontinuity
  when started far away (e.g. `t0 = 100 K` targeting 3000 K); one such test is
  `#[ignore]`-d (`src/thermophysics/thermo/janaf.rs`). Seed the iteration with a
  reasonable temperature and handle `Err(NonConvergent)`.

### ODE and polynomial restrictions

- **The stiff `Rosenbrock23` solver needs a user Jacobian.** `OdeSystem::jacobian`
  has a default that **panics** (`unimplemented!`, `src/ode/mod.rs`); only the
  explicit `Euler` and `Rkf45` solvers work without one.
- **`Polynomial<N>::integral()` (degree-raising form) is not implemented.** The
  type-level definite integral that returns a `Polynomial<{N+1}>` needs nightly
  `generic_const_exprs`; use the scalar `integral(x1, x2) -> f64` /
  `integral_minus1` forms instead (`src/polynomial/polynomial.rs`).

### Validation status

- **Unverified until validated.** This crate's own test suite is unit /
  verification tests against analytic and reference values, not full-solver
  validation. The only end-to-end physics validation cited (lid-driven cavity,
  Ghia Re = 100) is run in the downstream `outram-foam-appbuilder-lib` crate, not
  here. Treat all outputs as untrusted draft until a specific V&V case covers
  your usage. Not for nuclear facility operation, reactor control,
  safety-critical, or licensing decisions.

### Platform / build

- **Pure Rust, no BLAS required by the library.** Runtime dependencies are only
  `thiserror`, `uom`, and `ndarray`, so the library builds headless (including
  Android and other no-system-BLAS targets). The LAPACK comparison benchmark
  (`tests/matrix_bench.rs`) *does* need system OpenBLAS (Linux) / Intel MKL
  (Windows/macOS); it is a **dev-dependency, target-gated off Android**, and is
  not part of the library build.

## SquareMatrix vs LAPACK benchmark

`SquareMatrix::solve` (pure-Rust LU, no external BLAS) compared to
`ndarray-linalg` / OpenBLAS `Array2::solve` (LAPACK DGESV) — release mode,
Linux x86-64, 2026-06-24:

| n | SquareMatrix (ns) | OpenBLAS (ns) | ratio |
|---|---|---|---|
| 5 | 193 | 371 | **0.52 — SquareMatrix 1.9× faster** |
| 10 | 352 | 512 | **0.69 — SquareMatrix 1.5× faster** |
| 20 | 1 446 | 1 614 | **0.90 — roughly equal** |
| 50 | 17 018 | 7 891 | 2.16 — OpenBLAS faster |
| 100 | 135 705 | 27 845 | 4.87 — OpenBLAS faster |
| 200 | 1 112 109 | 357 281 | 3.11 — OpenBLAS faster |

`SquareMatrix` is faster for n ≤ 10 because OpenBLAS DGESV has ~300–400 ns
of per-call FFI overhead that dominates at small sizes. The crossover is around
n ≈ 20–50. For typical finite-volume networks (10–50 unknowns per implicit
system), `SquareMatrix` eliminates the system BLAS dependency with no
performance penalty. Reproduce with:

```bash
cargo test -p outram-foam-basic-lib --test matrix_bench --release -- --nocapture
```

## Pressure linear-solver performance (DIC preconditioner + warm start)

The pressure Poisson solve dominates the cost of an incompressible transient
run, so the PCG path here mirrors OpenFOAM's two key efficiency choices. The
reasoning, measured on the `pimple_foam_cavity` Ghia Re=100 case
(`outram-foam-appbuilder-lib`, 41×41 mesh, 6000 steps to t=12 s):

**The diagnosis.** Instrumenting every pressure solve showed 12 000 solves
(6000 steps × 2 PISO correctors), each running a *flat 272 PCG iterations* — the
same count on the first step and the last — for ≈ 3.26 million sparse
matrix-vector products. That was essentially the whole wall-clock time. Two
things were wrong:

1. **Cold start every solve.** `conjugate_gradient` began from `x = 0` each
   call, so even near steady state — where the pressure barely changes between
   steps — it paid full convergence from scratch.
2. **Jacobi preconditioner.** The preconditioner was diagonal only
   (`M⁻¹ = 1/diag`), whose PCG iteration count scales with the mesh
   (∝ √κ ≈ O(Nₓ)).

**The fix (both taken straight from the OpenFOAM source).**

- **Warm start.** `conjugate_gradient` now accepts an initial guess `x0`, and
  `FvMatrix::solve_cg_with_guess` seeds the solve with the previous time step's
  field. This is exactly what `Foam::PCG::scalarSolve` does — it computes the
  initial residual as `source − A·psi` from the *incoming* `psi`, i.e. the field
  passed in is the initial guess. A transient solver near steady state then
  converges in a few iterations (the solver returns immediately, 0 iterations,
  if the guess already meets the tolerance).
- **DIC preconditioner.** The Jacobi preconditioner was replaced with DIC
  (Diagonal-based Incomplete Cholesky), a faithful port of
  `Foam::DICPreconditioner`: a one-time reciprocal-diagonal factorisation
  (`calcReciprocalD`) plus a forward/backward face sweep (`precondition`).
  It requires faces in upper-triangular order (`owner[f] < neighbour[f]`), which
  is how `read_poly_mesh` loads OpenFOAM `polyMesh` files.

**Result.** Per-solve iterations dropped from a flat 272 to ~73 (cold, DIC
alone) and ~40 near steady state (DIC + warm start) — **6.6× fewer total CG
iterations** (3.26M → 0.50M) and the cavity test roughly halved in wall time,
with the solution field bit-for-bit unchanged (Ghia RMS 0.0113).

**GAMG (algebraic multigrid) — implemented; mesh-independent but not a win at
small sizes.** [`gamg`](src/ldu_matrix/solvers/gamg.rs) is a serial port of
OpenFOAM's `GAMGSolver` with `algebraicPairGAMGAgglomeration`: pairwise
agglomeration on the matrix coefficients (`|upper|`), Galerkin coarse operators
(`Pᵀ A P`), a recursive correction-scheme V-cycle with Gauss-Seidel pre/post
smoothing and an optimal line-search correction scale, and a dense-LU coarsest
solve. Its V-cycle count stays flat as the mesh is refined (verified in
`gamg::tests::gamg_cycle_count_is_mesh_independent`) — the property PCG lacks.

On the 1681-cell cavity, however, GAMG reproduces the PCG solution **bit-for-bit
(field diff 2.6e-9)** but runs *slower* (42.8 s vs 12.4 s): at this scale
DIC-PCG already converges in ~40 cheap iterations per corrector, below the
crossover (~10⁴–10⁵ cells) where multigrid's mesh-independence pays off, and the
free `gamg` function rebuilds the hierarchy on every solve. So PCG stays the
default; GAMG is the right tool once larger meshes are in play. Two follow-ups
would make GAMG competitive at scale: **cache the agglomeration** across time
steps (re-restrict coefficients only), and use **GAMG as a CG preconditioner**
for Krylov robustness.

## Prelude

```rust
use outram_foam_basic_lib::prelude::*;
```

Includes all tensor types, polynomial solvers, math functions, `SquareMatrix`,
ODE solvers, interpolation, all thermophysics types, all field and mesh types,
all LDU matrix types, FV operator modules (`fvc`, `fvm`), `adjust_phi`, and
all fluid/solid thermo types.

## Running tests

```bash
# Library unit tests (no external BLAS required)
cargo test -p outram-foam-basic-lib --lib --tests

# Matrix benchmark (release mode for meaningful numbers)
cargo test -p outram-foam-basic-lib --test matrix_bench --release -- --nocapture
```

## Layer roadmap

```
✅ Layer 1a — Tensor algebra      (Vector3, Tensor, SymmTensor, SphericalTensor)
✅ Layer 1b — Dense matrices      (SquareMatrix: LU with scaled partial pivoting)
✅ Layer 1c — Polynomial eqns     (LinearEqn, QuadraticEqn, CubicEqn, Roots<N>)
✅ Layer 1d — Polynomial eval     (Polynomial<N>: Horner, derivative, integral)
✅ Layer 1e — ODE solvers         (Euler, Rkf45, Rosenbrock23; OdeSystem trait,
                                   OdeSolver/OdeIntegrator enum dispatch)
✅ Layer 1f — Interpolation       (interpolate_xy, interpolate_spline_xy)
✅ Layer 1g — Math functions      (erf_inv, inc_gamma_*, inv_inc_gamma)
✅ Layer 1h — Thermophysics       (EOS, Thermo, Transport traits + 4 impls each)
✅ Layer 2  — Fields + Mesh       (VolField, SurfaceField, FvMesh, LduMatrix,
                                   FvMatrix, FvVectorMatrix, RegionInterface,
                                   Gauss-Seidel + CG solvers)
✅ Layer 3  — FV operators        (fvm: ddt, ddt_coeff, ddt_vec, ddt_coeff_vec,
                                        laplacian, laplacian_vec, div, div_vec
                                   fvc: grad, div, interpolate, sn_grad, flux,
                                        reconstruct, ddt_corr, buoyancy_flux
                                   adjust_phi)
✅ Layer 4  — Field thermodynamics (FluidThermo, PsiThermo, RhoThermo,
                                    SolidThermo, ConstSolidThermo)
⬜ Layer 5  — Solver logic         (icoFoam PISO loop → openfoam-icof;
                                    chtMultiRegionFoam → openfoam-cht;
                                    rhoPimpleFoam → openfoam-rho)
```

## Changelog

### Unreleased

- **Added GAMG (algebraic multigrid) solver** (`ldu_matrix::gamg`,
  `FvMatrix::solve_gamg{,_with_guess}`). Serial port of `Foam::GAMGSolver` +
  `algebraicPairGAMGAgglomeration`: pairwise agglomeration, Galerkin coarse
  operators, recursive V-cycle (GS pre/post-smoothing, line-search correction
  scale, dense-LU coarsest). V-cycle count is mesh-independent. On the
  1681-cell cavity it reproduces the PCG field exactly but is slower at that
  scale (see "Pressure linear-solver performance"); PCG remains the default.

- **Pressure PCG: DIC preconditioner + warm start (≈6.6× fewer iterations).**
  `conjugate_gradient` now uses a DIC preconditioner (`Foam::DICPreconditioner`
  port) instead of Jacobi and takes an optional initial guess `x0`;
  `FvMatrix::solve_cg_with_guess` warm-starts from the previous field. On the
  `pimple_foam_cavity` Ghia Re=100 case (41×41) this cut per-solve PCG
  iterations from a flat 272 to ~40 and total CG iterations 3.26M → 0.50M, with
  the solution unchanged. See "Pressure linear-solver performance" above.
  *Note:* `conjugate_gradient` gained a parameter (`x0`) — update callers from
  `conjugate_gradient(&ldu, &b, &settings)` to
  `conjugate_gradient(&ldu, &b, None, &settings)`.

- **Fixed an exponential-memory bug in field arithmetic (the "24 GB cavity").**
  `VolField`/`SurfaceField`'s `Add`/`Sub`/`Neg` rebuilt the field's `name`
  string on every call (`self.name = format!("({} + {})", self.name, rhs.name)`).
  In a solver loop where a persistent field is reassigned from an expression
  containing itself — e.g. `rho = rho + div(phi)`, with `phi`'s name embedding
  `interpolate(rho)` — each operation glued two copies of the previous name
  together, so the `name` String **doubled every timestep** (`2^step`). The
  rhoPimpleFoam `compressible_lid_cavity` test grew to ~1.3 GB by step 24 and
  was killed by OOM at step 25, with each step running ~2× slower than the last.

  The trap: the field *data* was completely healthy — `internal`/`boundary`
  Vecs stayed the correct size and the physics (`|U|`, `|p|`, `|phi|`) stayed
  bounded — so it presented as a "leak"/hang rather than a numerical blow-up.
  It was localised by printing `field.name.len()` per step: `rho`/`phi` names
  doubled while freshly-`solve()`d fields (`U`, `p`, `he`) stayed constant.

  Fix: arithmetic operators now leave `self.name` as the left operand's name.
  This also matches OpenFOAM, where a `GeometricField` keeps its fixed
  registered `IOobject` name and solvers *print* residual labels rather than
  accumulating an expression string onto the field. After the fix the cavity
  runs at a flat ~0.45 ms/step and ~3.7 MB RSS. See the Translation-notes
  callout in `CLAUDE.md` for the full write-up.

## License

GPL-3.0-only (matching the upstream OpenFOAM sources).

## Copyright

Copyright (C) 2026 Ong Kay Chen Theodore, Professor Per F. Peterson,
University of California, Berkeley Thermal Hydraulics Lab,
Singapore Nuclear Research and Safety Institute (SNRSI),
National University of Singapore (NUS), Repository Contributors.
