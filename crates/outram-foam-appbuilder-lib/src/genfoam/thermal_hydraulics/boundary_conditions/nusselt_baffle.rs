// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/boundaryConditions/
//             NusseltThermalBaffle1D/NusseltThermalBaffle1DFvPatchScalarField.{C,H}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (sradman@pm.me; EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `nusselt_baffle` — 1-D through-wall conduction coupled baffle
//!
//! **CORRECTED 2026-11-27 (gh:#1871, round 51): the ARITHMETIC is ported** (see "Ported" below); ~~**Not
//! implemented.**~~ The text that follows describes the earlier scaffold and is kept for context. This module only declares the public data shape of
//! GeN-Foam's `NusseltThermalBaffle1DFvPatchScalarField` so the rest of the
//! `boundary_conditions` module tree can reference its types; every method
//! that would compute a physical result is `unimplemented!()`. See bead
//! op-p6p.7.13 (follow-up) for the full port.
//!
//! ## What upstream does (for the next porter's context)
//!
//! Two GeN-Foam patches (a "master" and a "slave", named via the `samplePatch`
//! entry) sit on opposite faces of a **thin** wall — thin meaning the wall's
//! own thermal inertia is neglected, so its two surface temperatures are set
//! by an **instantaneous heat-flux balance** rather than a transient
//! conduction solve:
//!
//! ```text
//! h_master * (T_fluid,master - T_wall,master)
//!     = kappa_wall / thickness * (T_wall,master - T_wall,slave)
//!     = h_slave  * (T_wall,slave  - T_fluid,slave)
//! ```
//!
//! `h_master`/`h_slave` are convective heat-transfer coefficients from a
//! Nusselt-number correlation of the form
//!
//! ```text
//! Nu = const + coeff * Re^expRe * Pr^expPr
//! ```
//!
//! with per-side `(const, coeff, expRe, expPr)` — the slave side may omit any
//! of the four and inherit the master's value (upstream: "If not provided,
//! defaults to master value").
//!
//! The update is **implicit**: upstream's `updateCoeffs()` on the master patch
//! also reaches into the slave patch's `valueFraction`/`refValue`/`refGradient`
//! (a `mixed` boundary condition) to solve the three-way balance above as one
//! coupled system, and — in the two-phase case — additionally couples across
//! *both* fluid phases' temperature fields simultaneously. That
//! cross-patch/cross-phase coupling is genuinely solver-shaped (it needs
//! mutable access to another patch's BC state inside one patch's coefficient
//! update), which is why this is scaffolded rather than fully ported here: it
//! does not fit this module's "plain struct + pure function over dimensioned
//! scalars" contract without first deciding how the porous-solver bead
//! (op-p6p.7.11) represents inter-patch coupling.
//!
//! ## Ported (gh:#1871, round 51): the arithmetic, with explicit inputs
//!
//! Translated line by line from `NusseltThermalBaffle1DFvPatchScalarField.C` (GeN-Foam 652b3da):
//!
//! | upstream | Rust |
//! |---|---|
//! | constructor: `hw_ = max(kw_/max(tw_, 1e-69), 1e-69)` and `hw_ = max(hw_, 1e-69)` | [`hw_upstream`] |
//! | `calcH` (with and without the `f` factor of a `twoPhase` pair) | [`calc_h_upstream`] |
//! | `updateCoeffs()`, `!twoPhase_` branch | [`update_one_phase`] |
//! | `updateCoeffs()`, `twoPhaseOwner_` branch | [`update_two_phase`] |
//! | `evaluate()` (`mixed` with `valueFraction`, `refValue`, `refGrad`) | [`evaluate_upstream`] |
//!
//! **What is NOT ported and why (refused by name; a solver hook is needed):** the `mappedPatchBase` neighbour
//! lookup and `distribute` of the neighbour's fields (the caller supplies the neighbour's face values already
//! mapped), the `fluid` / `FSPair` / `kappaEff` / `Re` / `Pr` / `Dh` / `f` lookups (inputs here), `setPtrs`,
//! the owner/neighbour state writes (the functions RETURN the new `refGrad`s), the dictionary reader and
//! `write`. The class was NOT run as a class against the binary: the only shipped consumer is
//! `fmuCases/2D_LFRpowerPlant` (FMU), and constructing a `fluid` + `FSPair` + mapped baffle pair is not possible
//! without a solver. The arithmetic is compared bit for bit with the driver's literal transcription of the same
//! upstream lines on real OpenFOAM `scalarField`s (`tests/genfoam_nusselt_baffle_vs_transcription.rs`), which
//! checks the translation, not the upstream class.
//!
//! ## Not fabricated here (earlier scaffold text)
//!
//! No formula below is evaluated — the structs only carry the dictionary
//! entries upstream reads (`thickness`, `kappa`, `const`/`coeff`/`expRe`/
//! `expPr` per side) and the method signatures document, but do not compute,
//! the flux balance above. Every method body is `unimplemented!()`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use petir::mathf::RealMath;
use uom::si::f64::{Length, Ratio, ThermalConductivity, ThermodynamicTemperature};

use super::super::units::HeatTransferCoefficient;

/// Which side of the baffle (master or slave patch) a query refers to.
///
/// Mirrors upstream's `owner()` distinction between the patch that owns the
/// dictionary's `thickness`/`kappa` entries (the master, `samplePatch`
/// pointing at the slave) and the patch that reads them from its neighbour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaffleSide {
    /// The patch that declares `thickness` and `kappa` directly.
    Master,
    /// The patch that reads `thickness`/`kappa` from its `samplePatch` (the
    /// master).
    Slave,
}

/// Nusselt-number correlation coefficients, `Nu = const + coeff * Re^expRe *
/// Pr^expPr` — one instance per baffle side.
///
/// Mirrors the upstream `const`, `coeff`, `expRe`, `expPr` dictionary entries
/// verbatim. All four are dimensionless (the correlation itself is
/// dimensionless; `Re` and `Pr` are dimensionless inputs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NusseltCorrelationCoefficients {
    /// The additive constant term in the Nusselt correlation.
    pub const_term: f64,
    /// The multiplicative coefficient on `Re^expRe * Pr^expPr`.
    pub coeff: f64,
    /// The Reynolds-number exponent.
    pub exp_reynolds: f64,
    /// The Prandtl-number exponent.
    pub exp_prandtl: f64,
}

/// The 1-D through-wall conduction baffle (arithmetic of `NusseltThermalBaffle1D`; gh:#1871, round 51).
///
/// ~~Placeholder, every method `unimplemented!()`~~ **CORRECTED 2026-11-27**: [`Self::convective_htc`] and
/// [`Self::coupled_wall_temperatures`] compute, and the per-face functions of this module carry the whole
/// `updateCoeffs()` / `evaluate()` arithmetic. See the [module documentation](self) for what is not ported.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NusseltThermalBaffle1DBc {
    /// Baffle (wall) thickness, upstream `thickness`.
    pub thickness: Length,
    /// Baffle (wall) thermal conductivity, upstream `kappa`.
    pub wall_conductivity: ThermalConductivity,
    /// Nusselt correlation coefficients for the master-side convective
    /// coupling.
    pub master_correlation: NusseltCorrelationCoefficients,
    /// Nusselt correlation coefficients for the slave-side convective
    /// coupling (defaults to `master_correlation` upstream when the
    /// dictionary omits them — that default-inheritance is case-I/O and is
    /// the caller's responsibility here, not this type's).
    pub slave_correlation: NusseltCorrelationCoefficients,
}

impl NusseltThermalBaffle1DBc {
    /// Build a baffle BC from its dictionary entries. Pure data assembly — no
    /// physics is evaluated by this constructor.
    #[must_use]
    pub fn new(
        thickness: Length,
        wall_conductivity: ThermalConductivity,
        master_correlation: NusseltCorrelationCoefficients,
        slave_correlation: NusseltCorrelationCoefficients,
    ) -> Self {
        Self {
            thickness,
            wall_conductivity,
            master_correlation,
            slave_correlation,
        }
    }

    /// The convective heat-transfer coefficient on one side of the baffle,
    /// `h = Nu(Re, Pr) * k_fluid / D_h` (upstream `calcH`, the `onePhase` form without the `f` factor).
    ///
    /// gh:#1871: implemented (was `unimplemented!()`); see [`calc_h_upstream`] for the per-face form.
    #[must_use]
    pub fn convective_htc(
        &self,
        side: BaffleSide,
        reynolds: Ratio,
        prandtl: Ratio,
        fluid_conductivity: ThermalConductivity,
        hydraulic_diameter: Length,
    ) -> HeatTransferCoefficient {
        let c = match side {
            BaffleSide::Master => &self.master_correlation,
            BaffleSide::Slave => &self.slave_correlation,
        };
        let h = calc_h_upstream(
            c,
            &[reynolds.get::<uom::si::ratio::ratio>()],
            &[prandtl.get::<uom::si::ratio::ratio>()],
            &[fluid_conductivity.get::<uom::si::thermal_conductivity::watt_per_meter_kelvin>()],
            &[hydraulic_diameter.get::<uom::si::length::meter>()],
            None,
        );
        HeatTransferCoefficient::new::<uom::si::heat_transfer::watt_per_square_meter_kelvin>(h[0])
    }

    /// The coupled master/slave wall-face temperatures of the one-phase `updateCoeffs()` balance
    /// (`Tw`, `nbrTw`), for one face pair. gh:#1871: implemented (was `unimplemented!()`); `hw = kappa/thickness`
    /// is taken from this struct through [`hw_upstream`].
    #[must_use]
    pub fn coupled_wall_temperatures(
        &self,
        master_fluid_temperature: ThermodynamicTemperature,
        slave_fluid_temperature: ThermodynamicTemperature,
        master_htc: HeatTransferCoefficient,
        slave_htc: HeatTransferCoefficient,
    ) -> (ThermodynamicTemperature, ThermodynamicTemperature) {
        use uom::si::heat_transfer::watt_per_square_meter_kelvin as w;
        let hw = hw_upstream(
            self.wall_conductivity.get::<uom::si::thermal_conductivity::watt_per_meter_kelvin>(),
            self.thickness.get::<uom::si::length::meter>(),
        );
        let u = update_one_phase(
            hw,
            &[master_htc.get::<w>()],
            &[slave_htc.get::<w>()],
            &[master_fluid_temperature.get::<uom::si::thermodynamic_temperature::kelvin>()],
            &[slave_fluid_temperature.get::<uom::si::thermodynamic_temperature::kelvin>()],
            &[1.0],
            &[1.0],
        );
        (
            ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(u.tw[0]),
            ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(u.nbr_tw[0]),
        )
    }
}

/// `1e-69`: upstream's floor in every denominator of this class.
const TINY: f64 = 1e-69;

fn max_with(a: f64, b: f64) -> f64 {
    if a > b {
        a
    } else {
        b
    }
}

/// Wall conductance `hw_` as upstream's constructor computes it: `hw_ = max(kw_/max(tw_, 1e-69), 1e-69)`, then
/// `hw_ = max(hw_, 1e-69)`.
#[must_use]
pub fn hw_upstream(kappa: f64, thickness: f64) -> f64 {
    let hw = max_with(kappa / max_with(thickness, TINY), TINY);
    max_with(hw, TINY)
}

/// Upstream `calcH`: `(cnst + coeff*pow(Re, expRe)*pow(Pr, expPr))*f*k/Dh` (`f` only for a pair that has one,
/// `pair.fPtr().valid()`; without it `(...)*k/Dh`), evaluated left to right as C++ does.
#[must_use]
pub fn calc_h_upstream(
    c: &NusseltCorrelationCoefficients,
    re: &[f64],
    pr: &[f64],
    k: &[f64],
    dh: &[f64],
    f: Option<&[f64]>,
) -> Vec<f64> {
    let n = re.len();
    assert!(pr.len() == n && k.len() == n && dh.len() == n);
    (0..n)
        .map(|i| {
            let nu = c.const_term + c.coeff * re[i].r_powf(c.exp_reynolds) * pr[i].r_powf(c.exp_prandtl);
            match f {
                Some(f) => nu * f[i] * k[i] / dh[i],
                None => nu * k[i] / dh[i],
            }
        })
        .collect()
}

/// The result of one `updateCoeffs()` pair update (one-phase).
#[derive(Debug, Clone, PartialEq)]
pub struct OnePhaseUpdate {
    /// `Tw`: new wall temperature on this side.
    pub tw: Vec<f64>,
    /// `nbrTw`: new wall temperature on the neighbour side.
    pub nbr_tw: Vec<f64>,
    /// `this->refGrad()`.
    pub ref_grad: Vec<f64>,
    /// `nbrPatchFieldPtr_->refGrad()`.
    pub nbr_ref_grad: Vec<f64>,
}

/// Upstream `updateCoeffs()`, `!twoPhase_` branch (`NusseltThermalBaffle1DFvPatchScalarField.C`):
///
/// ```text
/// A = 1 + hf/hw;  B = (hf/hw)*Ti;  nbrA = 1 + nbrHf/hw;  nbrB = (nbrHf/hw)*nbrTi;
/// oneByAnbrAminOne = max(A*nbrA - 1, 1e-69);
/// Tw = (B*nbrA + nbrB)/oneByAnbrAminOne;  nbrTw = (nbrB*A + B)/oneByAnbrAminOne;
/// refGrad = (hf/kfEff)*(Tw - Ti);  nbrRefGrad = (nbrHf/nbrKfEff)*(nbrTw - nbrTi);
/// ```
/// Inputs are per face; the neighbour's fields must already be mapped onto this patch (`distribute`).
#[must_use]
pub fn update_one_phase(
    hw: f64,
    hf: &[f64],
    nbr_hf: &[f64],
    ti: &[f64],
    nbr_ti: &[f64],
    kf_eff: &[f64],
    nbr_kf_eff: &[f64],
) -> OnePhaseUpdate {
    let n = hf.len();
    let mut out = OnePhaseUpdate { tw: vec![0.0; n], nbr_tw: vec![0.0; n], ref_grad: vec![0.0; n], nbr_ref_grad: vec![0.0; n] };
    for i in 0..n {
        let a = 1.0 + hf[i] / hw;
        let b = (hf[i] / hw) * ti[i];
        let nbr_a = 1.0 + nbr_hf[i] / hw;
        let nbr_b = (nbr_hf[i] / hw) * nbr_ti[i];
        let one_by = max_with(a * nbr_a - 1.0, TINY);
        let tw = (b * nbr_a + nbr_b) / one_by;
        let nbr_tw = (nbr_b * a + b) / one_by;
        out.tw[i] = tw;
        out.nbr_tw[i] = nbr_tw;
        out.ref_grad[i] = (hf[i] / kf_eff[i]) * (tw - ti[i]);
        out.nbr_ref_grad[i] = (nbr_hf[i] / nbr_kf_eff[i]) * (nbr_tw - nbr_ti[i]);
    }
    out
}

/// The result of one `updateCoeffs()` update of the controlling fluid of a `twoPhase` pair.
#[derive(Debug, Clone, PartialEq)]
pub struct TwoPhaseUpdate {
    /// `Tw` and `nbrTw`.
    pub tw: Vec<f64>,
    /// See `tw`.
    pub nbr_tw: Vec<f64>,
    /// `this->refGrad()`.
    pub ref_grad: Vec<f64>,
    /// `nbrPatchFieldPtr_->refGrad()`.
    pub nbr_ref_grad: Vec<f64>,
    /// `otherFluidPatchFieldPtr_->refGrad()`.
    pub other_ref_grad: Vec<f64>,
    /// `otherFluidNbrPatchFieldPtr_->refGrad()`.
    pub other_nbr_ref_grad: Vec<f64>,
}

/// One face's inputs to [`update_two_phase`] (this fluid, its neighbour patch, the other fluid on both).
#[derive(Debug, Clone, PartialEq)]
pub struct TwoPhaseInputs {
    /// `hf`, `nbrHf`, `otherFluidHf`, `otherFluidNbrHf`.
    pub hf: Vec<f64>,
    /// See `hf`.
    pub nbr_hf: Vec<f64>,
    /// See `hf`.
    pub other_hf: Vec<f64>,
    /// See `hf`.
    pub other_nbr_hf: Vec<f64>,
    /// `Ti`, `nbrTi`, `otherFluidTi`, `otherFluidNbrTi` (patch-internal cell temperatures).
    pub ti: Vec<f64>,
    /// See `ti`.
    pub nbr_ti: Vec<f64>,
    /// See `ti`.
    pub other_ti: Vec<f64>,
    /// See `ti`.
    pub other_nbr_ti: Vec<f64>,
    /// `kfEff`, `nbrKfEff`, `otherFluidKfEff`, `otherFluidNbrKfEff`.
    pub kf_eff: Vec<f64>,
    /// See `kf_eff`.
    pub nbr_kf_eff: Vec<f64>,
    /// See `kf_eff`.
    pub other_kf_eff: Vec<f64>,
    /// See `kf_eff`.
    pub other_nbr_kf_eff: Vec<f64>,
}

/// Upstream `updateCoeffs()`, `twoPhaseOwner_` branch:
///
/// ```text
/// hm = hf + otherFluidHf;  nbrHm = nbrHf + otherFluidNbrHf;   (hm, nbrHm are computed but unused)
/// A = 1 + hm/hw;  B = (hf*Ti + otherFluidHf*otherFluidTi)/hw;
/// nbrA = 1 + nbrHf/hw;  nbrB = (nbrHf*nbrTi + otherFluidNbrHf*otherFluidNbrTi)/hw;
/// oneByAnbrAminOne = max(A*nbrA - 1, 1e-69);   Tw = (B*nbrA + nbrB)/..;   nbrTw = (nbrB*A + B)/..;
/// refGrad = (hf/kfEff)*(Tw - Ti);  nbr: (nbrHf/nbrKfEff)*(nbrTw - nbrTi);
/// other: (otherFluidHf/otherFluidKfEff)*(Tw - otherFluidTi);  otherNbr: (otherFluidNbrHf/..)*(nbrTw - otherFluidNbrTi)
/// ```
/// (upstream's `A` uses `hm = hf + otherFluidHf` while `nbrA` uses `nbrHf` ALONE: reproduced as written.)
#[must_use]
pub fn update_two_phase(hw: f64, x: &TwoPhaseInputs) -> TwoPhaseUpdate {
    let n = x.hf.len();
    let mut o = TwoPhaseUpdate {
        tw: vec![0.0; n],
        nbr_tw: vec![0.0; n],
        ref_grad: vec![0.0; n],
        nbr_ref_grad: vec![0.0; n],
        other_ref_grad: vec![0.0; n],
        other_nbr_ref_grad: vec![0.0; n],
    };
    for i in 0..n {
        let hm = x.hf[i] + x.other_hf[i];
        let a = 1.0 + hm / hw;
        let b = (x.hf[i] * x.ti[i] + x.other_hf[i] * x.other_ti[i]) / hw;
        let nbr_a = 1.0 + x.nbr_hf[i] / hw;
        let nbr_b = (x.nbr_hf[i] * x.nbr_ti[i] + x.other_nbr_hf[i] * x.other_nbr_ti[i]) / hw;
        let one_by = max_with(a * nbr_a - 1.0, TINY);
        let tw = (b * nbr_a + nbr_b) / one_by;
        let nbr_tw = (nbr_b * a + b) / one_by;
        o.tw[i] = tw;
        o.nbr_tw[i] = nbr_tw;
        o.ref_grad[i] = (x.hf[i] / x.kf_eff[i]) * (tw - x.ti[i]);
        o.nbr_ref_grad[i] = (x.nbr_hf[i] / x.nbr_kf_eff[i]) * (nbr_tw - x.nbr_ti[i]);
        o.other_ref_grad[i] = (x.other_hf[i] / x.other_kf_eff[i]) * (tw - x.other_ti[i]);
        o.other_nbr_ref_grad[i] = (x.other_nbr_hf[i] / x.other_nbr_kf_eff[i]) * (nbr_tw - x.other_nbr_ti[i]);
    }
    o
}

/// Upstream `evaluate()`: `valueFraction*refValue + (1 - valueFraction)*(Ti + refGrad/max(deltaCoeffs, 1e-69))`.
#[must_use]
pub fn evaluate_upstream(
    value_fraction: &[f64],
    ref_value: &[f64],
    ti: &[f64],
    ref_grad: &[f64],
    delta_coeffs: &[f64],
) -> Vec<f64> {
    (0..ti.len())
        .map(|i| value_fraction[i] * ref_value[i] + (1.0 - value_fraction[i]) * (ti[i] + ref_grad[i] / max_with(delta_coeffs[i], TINY)))
        .collect()
}
