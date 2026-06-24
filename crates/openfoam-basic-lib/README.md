# openfoam-basic-lib

Pure-Rust translation of the OpenFOAM primitive layer — tensor algebra,
polynomial solvers, and math special functions needed to build compressible
CFD solvers equivalent to **rhoPimpleFoam** and **sonicFoam**.

## Quick start

```toml
[dependencies]
openfoam-basic-lib = { path = "../openfoam-basic-lib" }
```

```rust
use openfoam_basic_lib::prelude::*;

// Tensor algebra
let u = Vector3::new(1.0, 0.0, 0.0);
let v = Vector3::new(0.0, 1.0, 0.0);
let cross = u.cross(v);              // z-axis unit vector

let t = SymmTensor::IDENTITY;
let dev = t.dev();                   // deviatoric part

// Polynomial root finding
let cubic = CubicEqn::new(1.0, -6.0, 11.0, -6.0); // (x-1)(x-2)(x-3)
let roots = cubic.roots();
// roots[0], roots[1], roots[2] are the three real roots

// Polynomial evaluation
let cp = Polynomial::new([1000.0, 0.5, -1e-4]); // Cp = 1000 + 0.5T - 1e-4 T²
let cp_at_500 = cp.value(500.0);
let cp_integral = cp.integral(300.0, 500.0);

// Math special functions
let q = inc_gamma_ratio_q(2.0, 3.0);    // Q(2, 3) ≈ 0.199
let x = inv_inc_gamma(2.0, 0.5);        // x such that P(2,x) = 0.5
```

## What's implemented

| Module | Rust type / fn | C++ source | Notes |
|---|---|---|---|
| `primitives` | `type Scalar = f64` | `Scalar/scalar.H` | + `Label`, `SMALL`, `VSMALL`, `GREAT`, `VGREAT`, `ROOT_SMALL`, `ROOT_VSMALL`, `ROOT_GREAT` |
| `primitives` | `struct SphericalTensor` | `SphericalTensor/SphericalTensorI.H` | `ii·I`; trace, det, inv, double_inner |
| `primitives` | `struct Vector3` | `Vector/VectorI.H` | cross, dot, normalise, lerp, outer product |
| `primitives` | `struct SymmTensor` | `SymmTensor/SymmTensorI.H` | dev, dev2, sph, det, inv, safe_inv, mat_vec, hodge_dual |
| `primitives` | `struct Tensor` | `Tensor/TensorI.H` | transpose, det, inv, safe_inv, symm, skew, dev, mat_mul, mat_vec, hodge_dual |
| `polynomial` | `enum RootType` | `polynomialEqns/Roots.H` | Real, Complex, PosInf, NegInf, Nan |
| `polynomial` | `struct Roots<const N: usize>` | `polynomialEqns/RootsI.H` | 3-bit-per-root type encoding |
| `polynomial` | `struct LinearEqn` | `polynomialEqns/linearEqn/` | `a·x + b = 0`; value, derivative, error, roots |
| `polynomial` | `struct QuadraticEqn` | `polynomialEqns/quadraticEqn/` | FMA-accurate discriminant; real/complex roots |
| `polynomial` | `struct CubicEqn` | `polynomialEqns/cubicEqn/` | Cardano + Kahan-compensated p,q; all case branches |
| `polynomial` | `struct Polynomial<const N: usize>` | `functions/Polynomial/Polynomial.H` | Horner eval, derivative, integral, integral_minus1 (log term) |
| `math` | `fn erf_inv(y)` | `functions/Math/erfInv.C` | Winitzki (2008) approximation |
| `math` | `fn inc_gamma_ratio_q(a, x)` | `functions/Math/incGamma.C` | DiDonato–Morris (1986); full branch coverage |
| `math` | `fn inc_gamma_ratio_p(a, x)` | same | `1 - Q(a, x)` |
| `math` | `fn inc_gamma_q(a, x)` | same | `Q(a,x) · Γ(a)` |
| `math` | `fn inc_gamma_p(a, x)` | same | `P(a,x) · Γ(a)` |
| `math` | `fn inv_inc_gamma(a, p)` | `functions/Math/invIncGamma.C` | DiDonato–Morris inverse; ~3–4 sig figs for a < 1 |
| `matrix` | `struct SquareMatrix` | `matrices/scalarMatrices/scalarMatrices.C` | row-major n×n; LU with scaled partial pivoting; `lu_decompose`, `lu_back_substitute`, `solve` |
| `ode` | `trait OdeSystem` | `ODE/ODESystem/ODESystem.H` | `n_eqns`, `derivatives`, `jacobian` |
| `ode` | `struct Euler` | `ODE/ODESolvers/Euler/Euler.C` | explicit 1st-order adaptive; `solve_step`, `integrate` |
| `ode` | `struct Rkf45` | `ODE/ODESolvers/RKF45/RKF45.C` | explicit RKF 4(5) adaptive; 6-stage Butcher tableau |
| `ode` | `struct Rosenbrock23` | `ODE/ODESolvers/Rosenbrock23/Rosenbrock23.C` | W-method stiff adaptive; requires Jacobian; 3 LU back-solves per step |
| `interpolation` | `fn interpolate_xy` | `interpolations/interpolateXY/interpolateXY.C` | linear 1-D; binary search; clamps at endpoints |
| `interpolation` | `fn interpolate_spline_xy` | `interpolations/interpolateSplineXY/interpolateSplineXY.C` | Catmull-Rom cubic; ghost-point boundary extension |
| `thermophysics` | `type Compressibility` | — | Custom uom quantity ψ = ∂ρ/∂p|T, s²/m² |
| `thermophysics::eos` | `trait EquationOfState` | `specie/equationOfState/` | rho, psi, Z, CpMCv, h/e/s EOS departures; full uom types |
| `thermophysics::eos` | `struct PerfectGas` | `equationOfState/perfectGas/` | p = ρRT; Z=1; ρ=p/(RT) via uom arithmetic |
| `thermophysics::eos` | `struct RhoConst` | `equationOfState/rhoConst/` | incompressible ρ=const; ψ=0 |
| `thermophysics::eos` | `struct IcoPolynomial<const N>` | `equationOfState/icoPolynomial/` | incompressible ρ=1/poly(T); ψ=0; h_eos=p/ρ |
| `thermophysics::thermo` | `trait ThermoModel` | `specie/thermo/thermo/` | Cp, Ha, Hs, Hc, S, Cv; Newton T(H/Hs/e) iteration |
| `thermophysics::thermo` | `struct HConstThermo<E>` | `thermo/hConst/` | const Cp; Hs = Cp·(T−Tref)+Hsref |
| `thermophysics::thermo` | `struct JanafThermo<E>` | `thermo/janaf/` | NASA 7-coeff dual-range polynomial; Hc at T_std |
| `thermophysics::thermo` | `struct HPolynomialThermo<E, const N>` | `thermo/hPolynomial/` | Cp = poly(T); Ha via poly.integral; S via integral_minus1 |
| `thermophysics::transport` | `trait TransportModel` | `specie/transport/` | mu, kappa; default alpha_h = kappa/Cp |
| `thermophysics::transport` | `struct ConstTransport<T>` | `transport/const/` | const mu + Pr; kappa = Cp·mu/Pr |
| `thermophysics::transport` | `struct SutherlandTransport<T>` | `transport/sutherland/` | mu = As√T/(1+Ts/T); Eucken kappa; two-point constructor |
| `thermophysics::transport` | `struct PolynomialTransport<T, const N>` | `transport/polynomial/` | mu(T) and kappa(T) as independent Polynomial<N> |

## Prelude

The `prelude` module re-exports the most commonly used items:

```rust
use openfoam_basic_lib::prelude::*;
```

Includes: scalar constants, `Vector3`, `Tensor`, `SymmTensor`, `SphericalTensor`,
`RootType`, `Roots`, `LinearEqn`, `QuadraticEqn`, `CubicEqn`, `Polynomial`,
and all math special functions.

## Running tests

```bash
cargo test -p openfoam-basic-lib --lib
```

## Layer roadmap

```
✅ Layer 1a — Tensor algebra   (Vector3, Tensor, SymmTensor, SphericalTensor)
✅ Layer 1b — Dense matrices   (SquareMatrix + LU decompose / back-substitute)
✅ Layer 1c — Polynomial eqns  (LinearEqn, QuadraticEqn, CubicEqn, Roots<N>)
✅ Layer 1d — Polynomial eval  (Polynomial<N>)
✅ Layer 1e — ODE solvers      (Euler, Rkf45, Rosenbrock23)
✅ Layer 1f — Interpolation    (interpolate_xy, interpolate_spline_xy)
✅ Layer 1g — Math functions   (erf_inv, inc_gamma_*, inv_inc_gamma)
🔶 Layer 1h — Thermophysics   (EOS: PerfectGas, RhoConst; Thermo: HConstThermo, JanafThermo; Transport: ConstTransport, SutherlandTransport — IcoPolynomial, HPolynomialThermo, PolynomialTransport, TabulatedTransport, PengRobinsonGas pending)
⬜ Layer 2  — Fields + Mesh
⬜ Layer 3  — FV operators
⬜ Layer 4  — Thermophysics
⬜ Layer 5  — Solver logic
```

## License

GPL-3.0-only (matching the upstream OpenFOAM sources).
