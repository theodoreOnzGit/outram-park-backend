// SPDX-License-Identifier: AGPL-3.0-only
//
// boon-lay particle mechanics — provenance
// ----------------------------------------
// Reference : Hales, J.D., Novascone, S.R., Pastore, G., Perez, D.M., Spencer,
//             B.W., Williamson, R.L., "BISON Theory Manual: The Equations
//             Behind Nuclear Fuel Analysis", INL/EXT-13-29930, Idaho National
//             Laboratory, October 2013. Sections 2, 5, 10.1, 10.2, 13.3.
// Status    : the manual carries no reuse licence; it is in the maintainer's
//             private corpus (`propreitrary-literature-theodore`,
//             `reports/inl-ext-13-29930-bison-theory-manual-2013.pdf`). Only
//             its EQUATIONS and constants are reproduced here, cited by
//             equation number, as scientific facts. No prose is copied.
// Nature    : written FROM SCRATCH (maintainer direction, 2026-11-28: "implement
//             bison eqns from scratch, put them in boon lay"). BISON's source
//             code was never obtained or read. This is not BISON and not a
//             port of it: call it "boon-lay particle mechanics", and keep
//             "BISON" for INL's code and the manual's own statements.

//! # boon-lay particle mechanics — a spherically symmetric TRISO particle, from the BISON equations
//!
//! > **Research, education and V&V only.** Not for reactor operation,
//! > licensing, fuel qualification, safety decisions or emergency response.
//! > AI-written first draft (2026-11-28), **not reviewed by a human**, and
//! > **verified against closed forms only**: nothing here is validated.
//!
//! ## Naming: this is boon-lay particle mechanics, NOT BISON
//!
//! BISON is Idaho National Laboratory's finite-element fuel performance code.
//! Its source is not open, and this project has never had it. What this
//! project has is the public **BISON Theory Manual** (Hales et al. 2013,
//! INL/EXT-13-29930), which prints the governing equations and some material
//! correlations. This module implements those equations from scratch, on the
//! finite-element layer of `farrer-park`. A result computed here is
//! **boon-lay particle mechanics'**, never "BISON's".
//!
//! ## The model
//!
//! A TRISO particle is a stack of concentric spherical shells (kernel,
//! buffer, inner PyC, SiC, outer PyC). Under spherical symmetry the only
//! unknown is the radial displacement `u(r)`, and the problem is 1D. That is
//! the "1D spherical" geometry the manual's introduction lists (Section 1).
//!
//! | Physics | Equation | Source | Here |
//! |---|---|---|---|
//! | Static equilibrium `∇·σ + ρf = 0`, no body force | (2.6) | Section 2 | [`mechanics`] |
//! | Heat conduction `ρCp ∂T/∂t + ∇·q − e_f Ḟ = 0`, `q = −k∇T`, **steady state** | (2.1), (2.2) | Section 2 | [`thermal`] |
//! | Spherical strains `ε_rr = ∂u/∂r`, `ε_θθ = ε_φφ = u/r` (the linear part) | (5.2)–(5.4) | Section 5 | [`mechanics`] |
//! | Isotropic elasticity (`λ`, `μ`) | (6.2) | Section 6 | [`material`] |
//! | Buffer (low-density PyC) irradiation strain rate | (10.1) | Section 10.1 | [`material::DimensionalChange::Bison2013Buffer`] |
//! | Dense PyC irradiation strain rate, radial and tangential | (10.2), (10.3) | Section 10.1 | [`material::DimensionalChange::Bison2013DensePyc`] |
//! | PyC irradiation creep `ε̇₁ = K[σ₁ ∓ ν_c(σ₂+σ₃)]Φ̇`, `ν_c = 0.5` | (10.4) | Section 10.1 | [`material::IrradiationCreep`] |
//! | PyC creep coefficient `K = K₀[1 + 2.38(1.9 − ρ)]M`, `K₀(T)` | (10.5), (10.6) | Section 10.1 | [`material::pyc_creep_coefficient_bison2013`] |
//! | SiC irradiation creep `ε̇ = Kσφ`, `K` at 640 °C and 900 °C | (10.7) | Section 10.2 | [`material::SIC_CREEP_COEFFICIENT_640C_PER_PA_PER_FLUENCE`] |
//! | Ideal-gas cavity pressure `P = nRT/V` | (13.15) | Section 13.3 | the pressure history is an input; `crate::fuel_failure::pressure::internal_gas_pressure` is that law |
//! | Weibull weakest-link failure of a layer | not in the manual | Weibull (1951), see [`weibull`] | [`weibull`] |
//!
//! The strain is split additively into elastic, thermal, irradiation
//! dimensional change and irradiation creep. Creep is integrated **implicitly**
//! (backward Euler), which the manual recommends for the PyC law ("allowing
//! arbitrarily large time steps without unstable creep response", Section
//! 10.1). Because every law here is linear in stress, each step is one linear
//! solve. There is no Newton loop and none is needed.
//!
//! ## Two findings in the manual, stated rather than silently resolved
//!
//! 1. **The sign in Eq (10.4).** The manual prints `ε̇₁ = K[σ₁ + ν_c(σ₂ + σ₃)]Φ̇`
//!    with `ν_c = 0.5`. With a `+`, a hydrostatic stress `σ` gives
//!    `ε̇ = 2Kσ Φ̇` in every direction, so a PyC layer would creep in
//!    **volume**. A creep Poisson's ratio of 0.5 exists to make creep
//!    volume-conserving, which needs a `−`. The default is therefore
//!    [`material::CreepLawSign::VolumeConserving`] (`−`), and the printed form
//!    is kept as [`material::CreepLawSign::AsPrinted`] (`+`) so a reader can
//!    run both. `tests::the_printed_creep_sign_creeps_in_volume` measures the
//!    difference. The manual's own references [55] (Petti et al. 2004) and
//!    [56] (Powers and Wirth 2010) were **not read**, so which one BISON's
//!    source uses is not known here. **Flagged for the maintainer** (gh:#1965).
//! 2. **The units of `K` in Eqs (10.5)–(10.6) are not printed.** `K₀` is
//!    given with `T` in °C and `ρ` in g/cm³, but no unit for `K`. This module
//!    reads it as **(MPa·n/m²)⁻¹**. That is an inference, not a reading: in
//!    (Pa·n/m²)⁻¹ a 100 MPa stress over 10²⁵ n/m² would give a creep strain
//!    near 10⁴, against about 10⁻² in (MPa·n/m²)⁻¹. For comparison, the manual
//!    states the SiC `K` of Eq (10.7) in (Pa·n/m²)⁻¹ and it is eight orders
//!    smaller. **Flagged for the maintainer (gh:#1965); not checked against [55].**
//!
//! ## What the manual does NOT give, so it is an input here
//!
//! The manual gives no elastic constants, thermal expansion or thermal
//! conductivity for PyC, the buffer or SiC, no kernel swelling correlation for
//! particle fuel, no CO production model, and no Weibull parameters. Every
//! one is a constructor argument of [`material::LayerMaterial`] or
//! [`weibull::WeibullParameters`], with its unit stated. **Nothing is guessed,
//! and no default value stands in for a missing one.** The fluence in
//! Eqs (10.1)–(10.6) is "fast" fluence, and the manual does not state its
//! energy threshold.
//!
//! ## What is deliberately absent
//!
//! - **Layer debonding and the buffer–IPyC gap.** Layers are bonded. The
//!   manual's gap heat transfer and mechanical contact (Sections 13.1–13.2)
//!   are not implemented, so the mechanical domain is usually started at the
//!   IPyC, with the gas pressure on its inner surface.
//! - **Finite strain.** The manual's Eqs (5.2)–(5.4) carry quadratic terms
//!   and Section 3 an incremental logarithmic-strain kinematics. Only the
//!   linear strain is used ("We can recover the linear strain by ignoring the
//!   higher-order terms", Section 5).
//! - **Transient conduction.** [`thermal`] solves the steady equation only.
//! - **2D and 3D, asphericity, cracking.** Spherical symmetry only.
//!
//! ## Units
//!
//! Every public constructor takes `uom` quantities. Internally, the assembly
//! works in bare `f64` SI (metres, pascals, kelvin, n/m²), as `farrer-park`
//! does, because a stiffness entry has no single `uom` type.
//!
//! ## Layout
//!
//! | Module | Contents |
//! |---|---|
//! | [`geometry`] | layers, the radial mesh |
//! | [`material`] | elastic constants and the BISON 2013 laws |
//! | [`mechanics`] | weak form, assembly, the implicit creep step |
//! | [`thermal`] | steady spherical conduction and its closed form |
//! | [`analytic`] | closed-form multilayer Lamé solution with eigenstrain, the reference the FEM is checked against |
//! | [`weibull`] | volume-integrated weakest-link failure probability |
//! | [`history`] | the irradiation-history driver |
//!
//! Verification record, with methodology and results:
//! `crates/boon-lay/verification_and_validation/triso_particle_mechanics/README.md`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

pub mod analytic;
pub mod geometry;
pub mod history;
pub mod material;
pub mod mechanics;
pub mod thermal;
pub mod weibull;

#[cfg(test)]
mod tests;

/// Why a particle-mechanics input was refused or a solve failed.
#[derive(Debug, Clone, PartialEq)]
pub enum ParticleMechanicsError {
    /// The layers do not form a contiguous stack of positive-thickness
    /// shells, or a layer has no elements. The text says which check failed.
    InvalidGeometry(String),
    /// A material constant is out of its physical range (non-positive
    /// Young's modulus, Poisson's ratio outside (-1, 0.5), negative creep
    /// coefficient, ...). The text says which.
    InvalidMaterial(String),
    /// A load step or history point is out of range (decreasing fluence,
    /// non-finite pressure, non-positive temperature, ...).
    InvalidLoad(String),
    /// `farrer-park`'s Krylov solve did not converge; its message is carried.
    LinearSolve(String),
    /// An index named a layer the mesh does not have.
    NoSuchLayer(usize),
}

impl core::fmt::Display for ParticleMechanicsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidGeometry(s) => write!(f, "invalid particle geometry: {s}"),
            Self::InvalidMaterial(s) => write!(f, "invalid layer material: {s}"),
            Self::InvalidLoad(s) => write!(f, "invalid load: {s}"),
            Self::LinearSolve(s) => write!(f, "linear solve failed: {s}"),
            Self::NoSuchLayer(i) => write!(f, "no layer with index {i}"),
        }
    }
}
