// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/src/thermophysicalProperties/H2/H2I.H
//     (`densityH`, `densityH2`, `rho`, `psi`, `CpMCv`)
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # H/H2 equation of state — density and `Cp - Cv`
//!
//! Atomic hydrogen H is modelled as an ideal (perfect) gas — [`density_atomic_h`]
//! is a bare `p = rho R T / W_H`. Molecular hydrogen H2 is modelled with a
//! Soave-Redlich-Kwong-style cubic equation of state with the `alpha0`, `b`,
//! `c` coefficients from [`constants`](super::constants), solved in closed
//! form for the density root ([`density_molecular_h2`]) via Cardano's cubic
//! formula. The bulk mixture density [`density`] mass-fraction-blends the two,
//! weighted by [`dissociation::mass_fraction_atomic_h`](super::dissociation::mass_fraction_atomic_h).
//!
//! [`mass_basis_eos_state`] is a shared internal helper: both `CpMCv` (this
//! file's [`cp_minus_cv`]) and upstream's `heatCapacityH2`
//! ([`heat_capacity::heat_capacity_molecular_h2`](super::heat_capacity::heat_capacity_molecular_h2))
//! differentiate the *same* mass-basis cubic EOS (`dP/dT`, `dP/dV`, hence
//! `dV/dT` by the implicit-function rule); upstream duplicates that whole block
//! verbatim in both C++ methods; this port factors it out once.
//!
//! ## A faithfully-ported upstream unit quirk
//!
//! The mass-basis recomputation inside [`mass_basis_eos_state`] rescales the
//! *molar* critical volume `Vc` (m^3/kmol) to a supposed mass basis via
//! `Vc * 1e-3`, not `Vc / W` (which is what every other mass-basis quantity in
//! the same block uses, e.g. `b_spec = b / W`). `1e-3 != 1/W` (`W ~= 2.016`), so
//! this looks like an upstream unit-conversion slip. **It is ported exactly as
//! written** — this crate's guardrail is "never paper over" an upstream
//! oddity, and [`heat_capacity`](super::heat_capacity)'s and this file's V&V
//! tests check against upstream's *own* reference numbers, which already bake
//! in this exact formula.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use petir::mathf::RealMath;
use super::constants::{alpha0, b, c, n, PC, R, TC, VC, W, W_H};

/// Density of pure atomic hydrogen as an ideal gas, `rho_H = p W_H / (R T)`.
///
/// `p` in Pa, `T` in K; returns kg/m^3.
#[inline]
pub(super) fn density_atomic_h(p: f64, t: f64) -> f64 {
    p * W_H / (R * t)
}

/// Density of pure molecular hydrogen H2 from the cubic SRK-style equation of
/// state, solved analytically (Cardano's formula) for the physical real root.
///
/// The EOS is set up directly in mass-density form,
/// `A rho^3 + B rho^2 + C rho + D = 0`, by substituting `V = W/rho` into the
/// molar cubic and clearing denominators (hence the `W`, `W^2`, `W^3` factors
/// in `B`, `C`, `D` below). `p` in Pa, `T` in K; returns kg/m^3.
#[inline]
pub(super) fn density_molecular_h2(p: f64, t: f64) -> f64 {
    let cubic = ReducedCubic::at(p, t);
    let (a_coeff, b_coeff, p_, q_) = (cubic.a_coeff, cubic.b_coeff, cubic.p_, cubic.q_);

    if cubic.discriminant < 0.0 {
        2.0 * (-p_ / 3.0).sqrt()
            * ((3.0 * q_ / (2.0 * p_) * (-3.0 / p_).sqrt()).r_acos() / 3.0).r_cos()
            - b_coeff / (3.0 * a_coeff)
    } else {
        let u1 = -q_ / 2.0 + cubic.discriminant.sqrt();
        let u2 = -q_ / 2.0 - cubic.discriminant.sqrt();
        u1.r_cbrt() + u2.r_cbrt() - b_coeff / (3.0 * a_coeff)
    }
}

/// The coefficients of the mass-density cubic and of its reduced form (upstream `densityH2` up to the
/// discriminant test), split out so that the V&V test can say which branch a grid point takes
/// (`discriminant < 0`: three real roots, trigonometric solution; otherwise Cardano's cube roots).
pub(super) struct ReducedCubic {
    pub(super) a_coeff: f64,
    pub(super) b_coeff: f64,
    pub(super) p_: f64,
    pub(super) q_: f64,
    pub(super) discriminant: f64,
}

impl ReducedCubic {
    /// Upstream's operation order; upstream's `pow(x, 3.0)` is `r_powf(x, 3.0)` here, not a
    /// multiplication chain (gh:#1872 round 50).
    pub(super) fn at(p: f64, t: f64) -> Self {
        let alpha0_ = alpha0();
        let b_ = b();
        let c_ = c();
        let alpha_ = alpha0_ / (t / TC).r_powf(n());

        let a_coeff = alpha_ * (c_ - b_);
        let b_coeff = W * (alpha_ + p * b_ * (c_ - b_) - R * t * b_);
        let c_coeff = W * W * (p * c_ - R * t);
        let d_coeff = W.r_powf(3.0) * p;

        // Reduced (depressed) cubic form t^3 + p_*t + q_ = 0.
        // https://en.wikipedia.org/wiki/Cubic_equation#Trigonometric_and_hyperbolic_solutions
        let p_ = (3.0 * a_coeff * c_coeff - b_coeff * b_coeff) / (3.0 * (a_coeff * a_coeff));
        let q_ = (2.0 * b_coeff.r_powf(3.0) - 9.0 * a_coeff * b_coeff * c_coeff
            + 27.0 * (a_coeff * a_coeff) * d_coeff)
            / (27.0 * a_coeff.r_powf(3.0));
        let discriminant = p_.r_powf(3.0) / 27.0 + q_ * q_ / 4.0;
        Self { a_coeff, b_coeff, p_, q_, discriminant }
    }
}

/// Bulk mixture density, mass-fraction-blending [`density_atomic_h`] and
/// [`density_molecular_h2`] by the equilibrium atomic mass fraction
/// (upstream `H2::rho`).
///
/// `p` in Pa, `T` in K; returns kg/m^3.
#[inline]
pub(super) fn density(p: f64, t: f64) -> f64 {
    let w_h = super::dissociation::mass_fraction_atomic_h(p, t);
    w_h * density_atomic_h(p, t) + (1.0 - w_h) * density_molecular_h2(p, t)
}

/// Compressibility `psi = rho/p` (upstream `H2::psi`, comment "currently it is assumed the liquid is
/// incompressible"). It IS called by GeN-Foam: OpenFOAM's `heRhoThermo::calculate` stores
/// `mixture_.psi(p, T)` in the thermo's `psi` field, which the `onePhase`/`twoPhase` pressure equations read
/// (`fluid_.thermo().psi()`). Translated in gh:#1872 round 50 (an earlier note here said it had no caller).
#[inline]
pub(super) fn compressibility(p: f64, t: f64) -> f64 {
    density(p, t) / p
}

/// Shared state from differentiating the **mass-basis** recomputation of the
/// molecular-H2 cubic EOS: `dP/dT`, `dP/dV` at fixed `T`, and `dV/dT = -(dP/dT)/(dP/dV)`
/// by the implicit-function theorem (valid because the EOS is `f(P,V,T) = 0`).
/// Feeds both [`cp_minus_cv`] and
/// [`heat_capacity::heat_capacity_molecular_h2`](super::heat_capacity::heat_capacity_molecular_h2).
///
/// See the module-level note on the `Vc * 1e-3` quirk this recomputation
/// inherits from upstream.
pub(super) struct MassBasisEosState {
    /// Specific gas constant `R / W` — J / (kg K).
    pub(super) rmass: f64,
    /// SRK temperature exponent `n` (shared with the molar-basis EOS).
    pub(super) n: f64,
    /// Mass-basis attraction term `alpha` at this `T`.
    pub(super) alpha: f64,
    /// Mass-basis covolume `b_spec = b / W` — m^3/kg.
    pub(super) b_spec: f64,
    /// Specific volume `V = 1 / rho_H2` — m^3/kg.
    pub(super) v: f64,
    /// `dP/dT` at fixed `V` — Pa/K.
    pub(super) dp_dt: f64,
    /// `dV/dT` at fixed `P`, from the implicit-function rule — m^3/(kg K).
    pub(super) dv_dt: f64,
}

impl MassBasisEosState {
    /// Differentiate the mass-basis molecular-H2 EOS at `(p, T)`. `p` in Pa,
    /// `T` in K.
    pub(super) fn at(p: f64, t: f64) -> Self {
        let rmass = R / W;
        let n_ = n();
        let alpha0_spec = alpha0() / (W * W);
        let alpha = alpha0_spec / (t / TC).r_powf(n_);
        let b_spec = b() / W;
        // See module doc: this is a faithful port of upstream's `Vc()*1e-3`,
        // not the `Vc()/W()` a true mass-basis rescale would use.
        let vc_local = VC * 1.0e-3;
        let c_local =
            rmass * TC / (PC + alpha0_spec / (vc_local * (vc_local + b_spec))) + b_spec - vc_local;
        let v = 1.0 / density_molecular_h2(p, t);

        let dp_dt = rmass / (v - b_spec + c_local) + n_ * alpha / (t * v * (v + b_spec));
        let dp_dv = -rmass * t / (v - b_spec + c_local).r_powi(2)
            + alpha * (b_spec + 2.0 * v) / (v * (v + b_spec)).r_powi(2);
        let dv_dt = -dp_dt / dp_dv;

        Self {
            rmass,
            n: n_,
            alpha,
            b_spec,
            v,
            dp_dt,
            dv_dt,
        }
    }
}

/// `Cp - Cv` of molecular H2 from the mass-basis EOS derivatives,
/// `T (dP/dT)(dV/dT)` (upstream `H2::CpMCv`).
///
/// `p` in Pa, `T` in K; returns J/(kg K).
#[inline]
pub(super) fn cp_minus_cv(p: f64, t: f64) -> f64 {
    let s = MassBasisEosState::at(p, t);
    t * s.dp_dt * s.dv_dt
}

#[cfg(test)]
mod tests {
    //! # V&V — bulk mixture density `rho`
    //!
    //! **Methodology.** Reproduces GeN-Foam's own regression fixture
    //! `Test-hydrogenThermophysicalProperties.C::test_rho`: 112 `(p, T) -> rho`
    //! points over 16 temperatures (83-3333 K) at 7 pressures
    //! (1-4500 psia), checked against upstream's own reference density (external
    //! reference values printed to ~9 significant figures). This is a **port
    //! verification** against upstream's regression data, not an independent
    //! literature validation of the SRK-style EOS itself. Pass criterion:
    //! symmetric relative error `2|ref-value|/(ref+value) < 0.15` (15%),
    //! matching `main()`'s `test_rho(0.15)` call — upstream's tolerance is
    //! looser than usual because the EOS loses accuracy near the H2 critical
    //! point (Tc = 33 K, Pc = 13 atm; the coldest test rows are only ~2.5x Tc
    //! but at high reduced pressure).
    //!
    //! **Results (measured 2026-07-15, `cargo test --release`).** All 112
    //! points pass; **max symmetric relative error `0.1150` (11.5%)**, inside
    //! the 15% threshold. The largest deviations are the coldest/highest-
    //! pressure rows (near the H2 critical region), exactly where upstream flags
    //! the EOS as least accurate — the port therefore reproduces upstream's
    //! density (and its known near-critical error) rather than machine-precision
    //! self-agreement, confirming a faithful translation of the cubic EOS and
    //! the dissociation blend.
    use super::*;

    const PSIA: f64 = 6894.76;

    #[test]
    fn density_matches_upstream_regression_fixture() {
        let temperatures_k: [f64; 16] = [
            83.33334, 111.11112, 222.22224, 333.33336, 444.44448, 555.5556, 833.3334, 1111.1112,
            1388.889, 1666.6668, 1944.4446, 2222.2224, 2500.0002, 2777.778, 3055.5558, 3333.3336,
        ];
        // (p [psia], rho references [kg/m3] for the 16 temperatures above)
        let rows: [(f64, [f64; 16]); 7] = [
            (
                1.0,
                [
                    0.020101565,
                    0.015004491,
                    0.007325242,
                    0.005005769,
                    0.003756329,
                    0.003006665,
                    0.002005511,
                    0.001508939,
                    0.001204588,
                    0.001004357,
                    0.000858589,
                    0.000744858,
                    0.000639137,
                    0.000527007,
                    0.000410073,
                    0.00031236,
                ],
            ),
            (
                15.0,
                [
                    0.301552315,
                    0.225059363,
                    0.109877025,
                    0.075086531,
                    0.056344933,
                    0.045099974,
                    0.030082668,
                    0.022637288,
                    0.018030379,
                    0.015031723,
                    0.012885249,
                    0.011251366,
                    0.00992664,
                    0.008754088,
                    0.007627991,
                    0.006519513,
                ],
            ),
            (
                50.0,
                [
                    1.009060462,
                    0.750197343,
                    0.366255681,
                    0.250288438,
                    0.187816444,
                    0.150333247,
                    0.10027556,
                    0.075459761,
                    0.059846568,
                    0.049935947,
                    0.04283176,
                    0.037473585,
                    0.033223888,
                    0.029674197,
                    0.026539385,
                    0.02365446,
                ],
            ),
            (
                150.0,
                [
                    2.667643847,
                    2.25059363,
                    1.098767042,
                    0.747941944,
                    0.561847485,
                    0.449998588,
                    0.299314536,
                    0.225530306,
                    0.179317049,
                    0.14965887,
                    0.128397568,
                    0.112390321,
                    0.099785395,
                    0.089443877,
                    0.08056965,
                    0.072608475,
                ],
            ),
            (
                500.0,
                [
                    10.29643702,
                    7.501978233,
                    3.662553601,
                    2.446899857,
                    1.846127515,
                    1.482588565,
                    0.990653649,
                    0.747693657,
                    0.597462928,
                    0.498691502,
                    0.427878696,
                    0.374599696,
                    0.332754673,
                    0.298638557,
                    0.269673978,
                    0.243949933,
                ],
            ),
            (
                1500.0,
                [
                    29.33204765,
                    21.23380307,
                    10.4762955,
                    7.084964858,
                    5.387008098,
                    4.34901189,
                    2.936984641,
                    2.22292174,
                    1.78117266,
                    1.488595488,
                    1.27763237,
                    1.119530169,
                    0.995307012,
                    0.894310622,
                    0.809252599,
                    0.734686668,
                ],
            ),
            (
                4500.0,
                [
                    58.11917933,
                    47.59262431,
                    27.13438061,
                    19.77198564,
                    15.27520346,
                    12.46156096,
                    8.541042872,
                    6.514307221,
                    5.242841958,
                    4.394664501,
                    3.785963021,
                    3.319585558,
                    2.958129008,
                    2.662428237,
                    2.41462266,
                    2.203259081,
                ],
            ),
        ];

        let mut max_rel_err = 0.0_f64;
        for (p_psia, refs) in rows {
            let p_pa = p_psia * PSIA;
            for (t, rho_ref) in temperatures_k.into_iter().zip(refs) {
                let rho = density(p_pa, t);
                let rel_err = 2.0 * (rho_ref - rho).abs() / (rho_ref + rho);
                max_rel_err = max_rel_err.max(rel_err);
                assert!(
                    rel_err < 0.15,
                    "rho(p={p_psia} psia, T={t} K) = {rho} kg/m3 (ref {rho_ref}), rel err {rel_err}"
                );
            }
        }
        eprintln!("rho fixture: max symmetric rel err = {max_rel_err} (threshold 0.15)");
    }
}
