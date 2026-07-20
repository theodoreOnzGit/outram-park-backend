// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! KNP (Kurganov–Noelle–Petrova) central-upwind shock-capturing flux, adapted
//! from OUTRAM PARK's `rhoCentralFoam` port for use as a **Mach-weighted
//! deferred-correction dissipation** term on top of the pressure-based PIMPLE
//! array [`super::TampinesSteamArray`].
//!
//! ## Why this module exists (the problem it solves)
//!
//! This is the machinery behind step 7 of the solver's derivation (see the
//! [`super`] module doc, "The all-Mach hybrid"). Short version: a pressure-based
//! PIMPLE solver has **no numerical dissipation at the shortest wavelengths**, so
//! at a sharp near-sonic flashing front it *rings* — the front develops
//! Gibbs-like oscillations the central flux cannot damp. A density-based scheme
//! solves this with an *upwind* flux, whose built-in viscosity keys off the local
//! characteristic wave speeds `a = U_n ± c`. The KNP flux is the canonical
//! central-upwind form of that idea.
//!
//! The trick here is to **not** switch solvers. We keep the implicit-acoustics
//! PIMPLE array (so low-Mach flow stays cheap, per derivation step 2) and add
//! *only* the KNP jump term as a correction, weighted by a Mach blend `β(Ma)` so
//! it is **identically zero away from the sonic front** and the default
//! [`super::SolverMode::Pimple`] path is bit-for-bit unchanged. Concretely the
//! dissipation is `knp − central` (the full KNP flux minus the same flux with its
//! jump term zeroed), so it *is* precisely the upwind viscosity and nothing else.
//!
//! The one thing this fluid demands that a perfect gas does not: the wave speed
//! `c` must be the **HEM equilibrium** sound speed ([`hem_sound_speed_ph`], using
//! the Kieffer closure in the dome), because the whole point is to sit the
//! characteristics correctly *through a flashing interface* where the phases are
//! exchanging mass. A frozen (Wood–Wallis) speed would misplace them. The energy
//! variable is likewise the **static** enthalpy density `ρ·he`, matching the
//! array's segregated EEqn, so the dissipation is consistent with the transport it
//! corrects (details below).
//!
//! ## What this provides (and what it deliberately drops)
//!
//! The upstream `rhoCentralFoam` is a *stand-alone density-based marcher* that
//! updates the conserved variables `[ρ, ρU, ρE]` explicitly. Here we keep the
//! implicit-acoustics PIMPLE solver and borrow **only the KNP face-flux math**,
//! re-expressed in this crate's own `openfoam_source` field types. The
//! stand-alone marcher, its `RhoCentralFoam` struct and `run()` loop are *not*
//! copied.
//!
//! Two changes versus the density-based port:
//!
//! - **No perfect-gas EOS.** The upstream code closes the state with
//!   `p = (γ−1)ρe` and `c = √(γp/ρ)`. That is deleted. The pressure `p` and the
//!   **HEM equilibrium sound speed** `c` are passed in per face state
//!   ([`FaceState`]) — computed by [`hem_sound_speed_ph`] from the real
//!   IAPWS-IF97 `(p, h)` closure, never Wood–Wallis / frozen / perfect-gas.
//! - **Static-enthalpy energy form.** The energy variable is the static
//!   specific-enthalpy density `ρ·he` (not the total energy `ρE`), with
//!   convective flux `ρ·U_n·he` — matching the array's segregated EEqn, which
//!   advances `∂(ρh)/∂t + ∇·(φh) = dp/dt` in static enthalpy with the pressure
//!   work carried by a separate `dp/dt` source. Dissipating `ρE` instead would
//!   double-count that pressure work and over-cool the near-break cell during
//!   the strong rarefaction (see [`knp_face_flux`]).
//!
//! ## How the dissipation is used
//!
//! For each internal face the assembler forms the KNP flux [`knp_face_flux`]
//! and the **jump-free central flux** [`central_face_flux`] (the same flux with
//! the `a_L·a_R·(W_R − W_L)` dissipation term zeroed). Their difference is the
//! pure numerical dissipation. It is scaled by the Mach-blend weight
//! `β(Ma) ∈ [0, 1]` ([`mach_blend`]) and injected as a deferred correction into
//! the **continuity** (`ρ`) and **momentum** (`ρU`) equations: subsonic faces
//! (`β = 0`) get **identically zero** added flux, so the default `Pimple` path
//! is bit-identical; only near-sonic faces (`β → 1`, the flashing front) receive
//! the shock-capturing damping.
//!
//! The **energy** dissipation is *not* a separate source. The continuity
//! dissipation is folded into `phi` before the array's segregated static-
//! enthalpy EEqn recomputes `∇·(φh)`, so that convection carries the enthalpy
//! shock-capturing implicitly while preserving the plateau-fix cancellation
//! `(ρ_cont − ρ_old)/dt = −∇·φ`. Adding a standalone `ρ·he` energy source on top
//! double-counts that transport and destabilises the flashing plateau (it
//! over-drained / over-heated the near-break cell out of the `(p,h)` validity
//! range); see [`super::HybridDissipation`]. `β`'s Mach number and the KNP wave
//! speeds still use the HEM equilibrium sound speed throughout.

use crate::openfoam_algorithms::openfoam_source::boundary::bc::{BoundaryCondition, PatchField};
use crate::openfoam_algorithms::openfoam_source::field::Field;
use crate::openfoam_algorithms::openfoam_source::vol_field::{VolScalarField, VolVectorField};
use crate::openfoam_algorithms::openfoam_source::Vector3;

use crate::interfaces::functional_programming::ph_flash_eqm::{
    ph_flash_region, s_ph_eqm, t_ph_eqm,
};
use crate::interfaces::functional_programming::pt_flash_eqm::FwdEqnRegion;
use crate::region_1_subcooled_liquid::w_tp_1;
use crate::region_2_vapour::w_tp_2;
use crate::region_3_single_phase_plus_supercritical_steam::w_tp_3;
use crate::region_4_vap_liq_equilibrium::{
    w_ps_eqm_region4_finite_diff_vol, w_ps_eqm_region4_kieffer,
};
use crate::region_5_steam_at_800_plus_degc::w_tp_5;

use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{AvailableEnergy, Pressure};
use uom::si::pressure::pascal;
use uom::si::velocity::meter_per_second;

/// Defensive lower bound \[m/s\] on the HEM sound speed. The equilibrium
/// two-phase speed can legitimately fall to a few tens of m/s near the
/// bubble/dew points, and the Kieffer closure ([`w_ps_eqm_region4_kieffer`]) is
/// AI-generated/unvalidated, so a non-finite or non-positive result is clamped
/// to this floor rather than allowed to poison the Mach number or the KNP wave
/// speeds. It sits far below any physical water/steam sound speed, so it never
/// perturbs `β` in the regimes the blend actually acts on.
pub(crate) const C_MIN_MPS: f64 = 1.0;

/// A reconstructed face state feeding the KNP flux: the MUSCL owner-biased
/// (`pos`) or neighbour-biased (`neg`) value of each primitive at a face.
///
/// All quantities are plain SI `f64` (this layer carries no `uom` bookkeeping,
/// matching the surrounding `openfoam_source` fields); the units are spelled
/// out per field.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FaceState {
    /// Density ρ \[kg/m³\].
    pub rho: f64,
    /// Velocity **U** \[m/s\].
    pub u: Vector3,
    /// Static specific enthalpy `he` \[J/kg\].
    pub he: f64,
    /// Pressure `p` \[Pa\].
    pub p: f64,
    /// HEM equilibrium sound speed `c` \[m/s\] (see [`hem_sound_speed_ph`]).
    pub c: f64,
}

/// A face flux in the face-normal direction, per unit face area.
///
/// Multiply each component by the face area magnitude `|Sf|` \[m²\] to obtain
/// the total flux through the face.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FaceFlux {
    /// Continuity (mass) flux `ρ·U_n` \[kg/(m²·s)\].
    pub cont: f64,
    /// Momentum flux `ρ·U_n·U + p·n` \[Pa\].
    pub mom: Vector3,
    /// Static-enthalpy energy flux `ρ·U_n·he` \[W/m²\].
    ///
    /// Completes the KNP flux for the `[ρ, ρU, ρ·he]` system and is exercised by
    /// the unit tests, but is not injected by the hybrid assembler: the energy
    /// shock-capturing rides on the continuity dissipation folded into `phi`
    /// (see the module doc / [`super::HybridDissipation`]), so this component has
    /// no lib-side consumer today. Kept for flux completeness and future use.
    #[allow(dead_code)]
    pub ener: f64,
}

/// Continuous Mach-blend weight `β(Ma) = clamp((Ma − lo)/(hi − lo), 0, 1)`.
///
/// - Returns `0` for `Ma ≤ lo` (subsonic ⇒ **no** added dissipation, so the
///   default PIMPLE path is preserved bit-for-bit),
/// - ramps linearly to `1` at `Ma ≥ hi` (near-sonic ⇒ full shock-capturing),
/// - is monotone non-decreasing in `Ma` and clamped to `[0, 1]`.
///
/// `lo` and `hi` are dimensionless Mach thresholds with `hi > lo`; the array
/// defaults are `lo = 0.3`, `hi = 1.0`. A degenerate `hi ≤ lo` falls back to a
/// hard step at `hi`.
#[inline]
pub(crate) fn mach_blend(ma: f64, lo: f64, hi: f64) -> f64 {
    if hi > lo {
        ((ma - lo) / (hi - lo)).clamp(0.0, 1.0)
    } else if ma >= hi {
        1.0
    } else {
        0.0
    }
}

/// HEM **equilibrium** sound speed `c` \[m/s\] at a cell's `(p, h)` state.
///
/// This is the sound speed used for *both* the regime Mach number *and* the KNP
/// characteristic wave speeds `u ± c`. It is the true homogeneous-equilibrium
/// speed, never Wood–Wallis (frozen) or perfect-gas:
///
/// - **Two-phase dome (Region 4)**: entropy `s` from the `(p, h)` flash
///   ([`s_ph_eqm`]) feeds Kieffer eq. 28 ([`w_ps_eqm_region4_kieffer`]) —
///   equilibrium derivatives along the saturation curve. If that (unvalidated)
///   closure returns a non-finite/non-positive value it falls back to the
///   simpler finite-difference speed [`w_ps_eqm_region4_finite_diff_vol`].
/// - **Single phase (Regions 1/2/3/5)**: the region forward speed
///   `w_tp_{1,2,3,5}(T, p)` at `T = t_ph_eqm(p, h)`.
///
/// The result is clamped to `c ≥ c_min` (`c_min` in m/s, e.g. [`C_MIN_MPS`]) so
/// a pathological non-finite value cannot poison downstream arithmetic.
///
/// ## Parameters
/// - `p_pa`  — pressure \[Pa\]
/// - `h_jkg` — static specific enthalpy \[J/kg\]
/// - `c_min` — defensive lower bound on the returned speed \[m/s\]
pub(crate) fn hem_sound_speed_ph(p_pa: f64, h_jkg: f64, c_min: f64) -> f64 {
    let p = Pressure::new::<pascal>(p_pa);
    let h = AvailableEnergy::new::<joule_per_kilogram>(h_jkg);

    let c = match ph_flash_region(p, h) {
        FwdEqnRegion::Region4 => {
            let s = s_ph_eqm(p, h);
            let c_kieffer = w_ps_eqm_region4_kieffer(p, s).get::<meter_per_second>();
            if c_kieffer.is_finite() && c_kieffer > 0.0 {
                c_kieffer
            } else {
                w_ps_eqm_region4_finite_diff_vol(p, s).get::<meter_per_second>()
            }
        }
        FwdEqnRegion::Region1 => w_tp_1(t_ph_eqm(p, h), p).get::<meter_per_second>(),
        FwdEqnRegion::Region2 => w_tp_2(t_ph_eqm(p, h), p).get::<meter_per_second>(),
        FwdEqnRegion::Region3 => w_tp_3(t_ph_eqm(p, h), p).get::<meter_per_second>(),
        FwdEqnRegion::Region5 => w_tp_5(t_ph_eqm(p, h), p).get::<meter_per_second>(),
    };

    if c.is_finite() && c > c_min {
        c
    } else {
        c_min
    }
}

/// Shared KNP flux kernel. With `with_dissipation = true` this is the full KNP
/// flux; with `false` the `a_L·a_R·(W_R − W_L)` jump term is dropped, giving the
/// jump-free central flux. See [`knp_face_flux`] / [`central_face_flux`].
#[inline]
fn face_flux_impl(l: &FaceState, r: &FaceState, n_f: Vector3, with_dissipation: bool) -> FaceFlux {
    // Face-normal velocities.
    let u_n_l = l.u.dot(n_f);
    let u_n_r = r.u.dot(n_f);

    // KNP one-sided local wave-speed estimates (clamped so a_R > a_L).
    let a_l = (u_n_l - l.c).min(u_n_r - r.c).min(0.0);
    let a_r = (u_n_l + l.c).max(u_n_r + r.c).max(0.0);
    let da = (a_r - a_l).max(1e-10);

    // Conserved variables W = [ρ, ρU, ρ·he] transported by this system.
    //
    // The energy variable is the **static** specific-enthalpy density `ρ·he`,
    // NOT the total energy `ρE = ρ·(he + ½|U|²) − p`. This deliberately matches
    // the array's segregated EEqn, which advances `∂(ρh)/∂t + ∇·(φh) = dp/dt` in
    // *static* enthalpy with the pressure work carried by a separate `dp/dt`
    // source. Dissipating `ρE` here would re-inject `−Δp` pressure work (and the
    // kinetic term) into that equation — during the strong Edwards rarefaction
    // the large `Δp` across the flashing front then over-cools the near-break
    // cell straight through the 273.15 K isotherm. Dissipating `ρ·he` keeps the
    // shock-capturing viscosity consistent with the enthalpy transport it is
    // correcting. The acoustic wave speeds `u ± c` (`c` the HEM equilibrium
    // sound speed) are unchanged.
    let w_rho_l = l.rho;
    let w_rho_r = r.rho;
    let w_rhou_l = l.rho * l.u;
    let w_rhou_r = r.rho * r.u;
    let w_rhoe_l = l.rho * l.he;
    let w_rhoe_r = r.rho * r.he;

    // Physical (convective + pressure) fluxes in the face-normal direction.
    let f_cont_l = l.rho * u_n_l;
    let f_cont_r = r.rho * u_n_r;
    let f_mom_l = (l.rho * u_n_l) * l.u + l.p * n_f;
    let f_mom_r = (r.rho * u_n_r) * r.u + r.p * n_f;
    let f_ener_l = l.rho * u_n_l * l.he;
    let f_ener_r = r.rho * u_n_r * r.he;

    // Dissipation jump term a_L·a_R·(W_R − W_L)/da (zeroed for the central flux).
    let (j_rho, j_rhou, j_rhoe) = if with_dissipation {
        (
            a_l * a_r * (w_rho_r - w_rho_l),
            a_l * a_r * (w_rhou_r - w_rhou_l),
            a_l * a_r * (w_rhoe_r - w_rhoe_l),
        )
    } else {
        (0.0, Vector3::ZERO, 0.0)
    };

    FaceFlux {
        cont: (a_r * f_cont_l - a_l * f_cont_r + j_rho) / da,
        mom: (a_r * f_mom_l - a_l * f_mom_r + j_rhou) * (1.0 / da),
        ener: (a_r * f_ener_l - a_l * f_ener_r + j_rhoe) / da,
    }
}

/// KNP central-upwind numerical flux at a face (Kurganov, Noelle & Petrova,
/// SIAM J. Sci. Comp. 2001), per unit face area:
///
/// ```text
/// F_KNP = (a_R·F_L − a_L·F_R + a_L·a_R·(W_R − W_L)) / (a_R − a_L)
/// a_L = min(U_n,L − c_L,  U_n,R − c_R,  0)
/// a_R = max(U_n,L + c_L,  U_n,R + c_R,  0)
/// ```
///
/// with `W = [ρ, ρU, ρ·he]` (the **static** enthalpy density — see the module
/// doc for why not the total energy `ρE`) and `c` the HEM equilibrium sound
/// speed passed in via each [`FaceState`]. `n_f` is the unit face normal
/// (owner → neighbour).
pub(crate) fn knp_face_flux(l: &FaceState, r: &FaceState, n_f: Vector3) -> FaceFlux {
    face_flux_impl(l, r, n_f, true)
}

/// The KNP flux with the `a_L·a_R·(W_R − W_L)` dissipation jump term zeroed —
/// i.e. the central, non-dissipative reference flux. The deferred-correction
/// dissipation used by the hybrid solver is `knp − central`, so at `L == R`
/// (uniform field) it is identically zero.
pub(crate) fn central_face_flux(l: &FaceState, r: &FaceState, n_f: Vector3) -> FaceFlux {
    face_flux_impl(l, r, n_f, false)
}

/// Extract one component (0=x, 1=y, 2=z) of a [`VolVectorField`] as a
/// [`VolScalarField`], carrying the matching scalar boundary conditions so the
/// MUSCL reconstruction's cell gradients are correct at the boundaries.
///
/// Mirrors OpenFOAM's `U.component(cmpt)` used inside `rhoCentralFoam` to build
/// the per-component `interpolate(U, pos/neg)` face states.
pub(crate) fn velocity_component(u: &VolVectorField, comp: usize) -> VolScalarField {
    let pick = |v: Vector3| match comp {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    };
    let internal: Vec<f64> = u.internal.as_slice().iter().map(|v| pick(*v)).collect();
    let boundary: Vec<PatchField<f64>> = u
        .boundary
        .iter()
        .map(|pf| {
            let bc = match &pf.bc {
                BoundaryCondition::FixedValue(v) => BoundaryCondition::FixedValue(pick(*v)),
                BoundaryCondition::FixedField(ff) => BoundaryCondition::FixedField(Field::new(
                    ff.as_slice().iter().map(|v| pick(*v)).collect(),
                )),
                BoundaryCondition::Calculated(ff) => BoundaryCondition::Calculated(Field::new(
                    ff.as_slice().iter().map(|v| pick(*v)).collect(),
                )),
                BoundaryCondition::ZeroGradient => BoundaryCondition::ZeroGradient,
                BoundaryCondition::Symmetry => BoundaryCondition::Symmetry,
                BoundaryCondition::Empty => BoundaryCondition::Empty,
            };
            let values = Field::new(pf.values.as_slice().iter().map(|v| pick(*v)).collect());
            PatchField { bc, values }
        })
        .collect();
    VolScalarField::new(
        format!("{}_{comp}", u.name),
        u.mesh.clone(),
        Field::new(internal),
        boundary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L == R (uniform field) ⇒ the KNP flux equals the central flux, i.e. the
    /// deferred-correction dissipation `knp − central` is identically zero. This
    /// is the property that makes a uniform / subsonic field see no added flux.
    #[test]
    fn identical_states_give_zero_dissipation() {
        let s = FaceState {
            rho: 950.0,
            u: Vector3::new(3.0, 0.0, 0.0),
            he: 5.0e5,
            p: 7.0e6,
            c: 1200.0,
        };
        let n_f = Vector3::new(1.0, 0.0, 0.0);
        let knp = knp_face_flux(&s, &s, n_f);
        let cen = central_face_flux(&s, &s, n_f);
        assert!(
            (knp.cont - cen.cont).abs() < 1e-9,
            "cont dissipation nonzero"
        );
        assert!(
            (knp.mom.x - cen.mom.x).abs() < 1e-6,
            "mom dissipation nonzero"
        );
        assert!(
            (knp.ener - cen.ener).abs() < 1e-3,
            "ener dissipation nonzero"
        );
    }

    /// Fully supersonic one-sided flow (both sides have `U_n − c > 0`) ⇒ `a_L = 0`
    /// so the KNP flux collapses to the pure upwind (left) physical flux.
    #[test]
    fn supersonic_one_sided_is_upwind() {
        let n_f = Vector3::new(1.0, 0.0, 0.0);
        // U_n = 500 m/s > c = 300 m/s on both sides ⇒ a_L = 0.
        let l = FaceState {
            rho: 20.0,
            u: Vector3::new(500.0, 0.0, 0.0),
            he: 2.6e6,
            p: 1.0e6,
            c: 300.0,
        };
        let r = FaceState {
            rho: 18.0,
            u: Vector3::new(520.0, 0.0, 0.0),
            he: 2.5e6,
            p: 0.9e6,
            c: 300.0,
        };
        let knp = knp_face_flux(&l, &r, n_f);
        // Pure upwind (left) physical fluxes (static-enthalpy energy form).
        let u_n_l = l.u.dot(n_f);
        let f_cont_l = l.rho * u_n_l;
        let f_ener_l = l.rho * u_n_l * l.he;
        assert!(
            (knp.cont - f_cont_l).abs() / f_cont_l.abs() < 1e-9,
            "supersonic KNP cont {} != upwind {}",
            knp.cont,
            f_cont_l
        );
        assert!(
            (knp.ener - f_ener_l).abs() / f_ener_l.abs() < 1e-9,
            "supersonic KNP ener {} != upwind {}",
            knp.ener,
            f_ener_l
        );
    }

    /// `β(Ma)` is zero below `lo`, one above `hi`, linear in between, and
    /// monotone non-decreasing and clamped to `[0, 1]`.
    #[test]
    fn mach_blend_monotone_and_clamped() {
        assert_eq!(mach_blend(0.1, 0.3, 1.0), 0.0);
        assert_eq!(mach_blend(0.3, 0.3, 1.0), 0.0);
        assert_eq!(mach_blend(1.0, 0.3, 1.0), 1.0);
        assert_eq!(mach_blend(1.5, 0.3, 1.0), 1.0);
        assert!((mach_blend(0.65, 0.3, 1.0) - 0.5).abs() < 1e-12);
        // Monotone non-decreasing over a sweep.
        let mut prev = -1.0;
        for i in 0..=20 {
            let ma = i as f64 / 20.0 * 1.2;
            let b = mach_blend(ma, 0.3, 1.0);
            assert!(b >= prev - 1e-12 && (0.0..=1.0).contains(&b));
            prev = b;
        }
    }

    /// HEM sound speed wiring: subcooled liquid water (7 MPa, h ≈ 1.0 MJ/kg,
    /// Region 1) gives a physical liquid speed (~1000–1700 m/s), so a modest
    /// velocity has a small Mach number; the value is finite/positive and above
    /// the `c_min` floor.
    #[test]
    fn hem_c_subcooled_liquid_and_mach() {
        // 7 MPa, 1.0 MJ/kg is subcooled liquid (h_f(7 MPa) ≈ 1.267 MJ/kg).
        let c = hem_sound_speed_ph(7.0e6, 1.0e6, C_MIN_MPS);
        assert!(c.is_finite() && c > C_MIN_MPS, "c = {c} not physical");
        assert!(
            (800.0..2000.0).contains(&c),
            "liquid-water sound speed {c} m/s outside expected band"
        );
        // Ma wiring on a uniform field: |u| / c.
        let u = 5.0_f64;
        let ma = u / c;
        assert!(ma < 0.02, "subcooled liquid Mach {ma} should be tiny");
        assert_eq!(mach_blend(ma, 0.3, 1.0), 0.0, "subsonic ⇒ β = 0");
    }

    /// HEM sound speed in the two-phase dome (Region 4) is finite and positive
    /// (Kieffer closure, or its finite-difference fallback), and is far lower
    /// than the single-phase liquid speed — the regime the blend targets.
    #[test]
    fn hem_c_two_phase_is_finite_and_lower() {
        // 1 bar, 1.5 MJ/kg ⇒ two-phase (x ≈ 0.48).
        let c = hem_sound_speed_ph(1.0e5, 1.5e6, C_MIN_MPS);
        assert!(
            c.is_finite() && c > C_MIN_MPS,
            "two-phase c = {c} not physical"
        );
        assert!(
            c < 800.0,
            "two-phase HEM speed {c} m/s should be well below liquid"
        );
    }
}
