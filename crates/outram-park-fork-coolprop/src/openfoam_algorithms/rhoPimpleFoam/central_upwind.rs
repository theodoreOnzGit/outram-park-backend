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
//! array [`super::OPCPFluidArray`]. This is the CoolProp-fork twin of
//! `tampines-steam-tables`'s `rhoPimpleFoam/central_upwind.rs`; only the
//! sound-speed closure differs (CoolProp Helmholtz EOS + Maxwell VLE here,
//! IAPWS-IF97 there).
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
//!   sound speed `c` are passed in per face state ([`FaceState`]) —
//!   `c` from [`hem_sound_speed_ph`], the CoolProp closure (see its doc for the
//!   single-phase EOS speed of sound and the **two-phase HEM equilibrium**
//!   finite-difference), never Wood–Wallis / frozen / perfect-gas.
//! - **Static-enthalpy energy form.** The energy variable is the static
//!   specific-enthalpy density `ρ·he` (not the total energy `ρE`), with
//!   convective flux `ρ·U_n·he` — matching the array's segregated EEqn, which
//!   advances `∂(ρh)/∂t + ∇·(φh) = dp/dt` in static enthalpy with the pressure
//!   work carried by a separate `dp/dt` source. Dissipating `ρE` instead would
//!   double-count that pressure work.
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
//! is bit-identical; only near-sonic faces (`β → 1`) receive the shock-capturing
//! damping. The **energy** dissipation is *not* a separate source — the
//! continuity dissipation is folded into `phi` before the array's segregated
//! static-enthalpy EEqn recomputes `∇·(φh)`, so that convection carries the
//! enthalpy shock-capturing implicitly (see [`super::HybridDissipation`]).

use crate::openfoam_algorithms::openfoam_source::boundary::bc::{BoundaryCondition, PatchField};
use crate::openfoam_algorithms::openfoam_source::field::Field;
use crate::openfoam_algorithms::openfoam_source::vol_field::{VolScalarField, VolVectorField};
use crate::openfoam_algorithms::openfoam_source::Vector3;

use crate::flash::state_ph;
use crate::fluid::Fluid;
use crate::props::state_trho;
use crate::vle::{phase_at_ph, saturation_at_temperature, saturation_temperature, PhaseAtPh};

/// Defensive lower bound \[m/s\] on the sound speed. The equilibrium two-phase
/// speed can legitimately fall to a few tens of m/s near the bubble/dew points,
/// so a non-finite or non-positive result (or a two-phase FD that degenerates)
/// is clamped to this floor rather than allowed to poison the Mach number or the
/// KNP wave speeds. It sits far below any physical fluid sound speed, so it never
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
    /// Sound speed `c` \[m/s\] (see [`hem_sound_speed_ph`]).
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

/// Sound speed `c` \[m/s\] at a cell's `(p, h)` state, from the **CoolProp
/// Helmholtz EOS** (single phase) or the **HEM equilibrium** finite-difference
/// (two phase). This is the speed used for *both* the regime Mach number *and*
/// the KNP characteristic wave speeds `u ± c`.
///
/// - **Single phase** (the state `(p, h)` classifies outside the saturation
///   dome, or the fluid ships no VLE ancillaries): the thermodynamic sound speed
///   `√((∂p/∂ρ)_s)` from the EOS ([`crate::props::FluidState::speed_of_sound`],
///   reached by the single-phase `(p, h)` flash). In a single phase the
///   equilibrium and frozen sound speeds *coincide* — there is no phase change to
///   lag — so this EOS value **is** the equilibrium sound speed, not a frozen
///   approximation of one.
/// - **Two-phase dome** (Region between the saturated-liquid and -vapour lines,
///   detected by [`crate::vle::phase_at_ph`]): the true
///   **homogeneous-equilibrium (HEM)** sound speed `a = √((∂p/∂ρ)_s)` evaluated
///   along the *equilibrium* isentrope by a central finite difference —
///   [`hem_c_two_phase_fd`]. This is the CoolProp analogue of how the
///   `tampines-steam-tables` twin uses IAPWS-IF97's `w_ps_eqm_region4_kieffer`:
///   the mixture density is re-equilibrated (re-flashed on the saturation curve)
///   at each perturbed pressure at fixed entropy, so the flashing compliance
///   `(v_g − v_f)·dx/dp` is included — never a frozen / Wood–Wallis / single-phase
///   speed. If that FD degenerates (perturbation leaves the dome, or a
///   near-critical VLE solve fails) it is clamped to `c_min`.
///
/// The result is clamped to `c ≥ c_min` (m/s) so a pathological non-finite value
/// cannot poison downstream arithmetic.
///
/// ## Parameters
/// - `fluid` — working fluid (its EOS / VLE close the sound speed)
/// - `p_pa`  — pressure \[Pa\]
/// - `h_jkg` — static specific enthalpy \[J/kg\]
/// - `c_min` — defensive lower bound on the returned speed \[m/s\]
pub(crate) fn hem_sound_speed_ph(fluid: Fluid, p_pa: f64, h_jkg: f64, c_min: f64) -> f64 {
    let c = match phase_at_ph(fluid, p_pa, h_jkg) {
        PhaseAtPh::TwoPhase { .. } => hem_c_two_phase_fd(fluid, p_pa, h_jkg).unwrap_or(c_min),
        PhaseAtPh::SinglePhase => match state_ph(fluid, p_pa, h_jkg) {
            Ok(s) => s.speed_of_sound,
            // Single-phase flash did not converge (e.g. unreachable branch); the
            // caller's `safe[]` gate keeps such faces out of the dissipation, so
            // the floor here is only a defensive placeholder for the Mach number.
            Err(_) => c_min,
        },
    };

    if c.is_finite() && c > c_min {
        c
    } else {
        c_min
    }
}

/// Equilibrium (HEM) mixture mass density \[kg/m³\] at pressure `p` \[Pa\] and
/// specific entropy `s` \[J/(kg·K)\], on the saturation dome.
///
/// Re-flashes the Maxwell VLE at `p` ([`crate::vle::saturation_temperature`] then
/// [`crate::vle::saturation_at_temperature`]), reads the saturated-liquid /
/// -vapour entropies, and picks the equilibrium quality `x = (s − s')/(s'' − s')`
/// that reproduces `s`; the mixture density follows from
/// `1/ρ = (1−x)/ρ' + x/ρ''`. Returns [`None`] if the state is not sub-critical /
/// two-phase at `p` or the VLE solve fails (near-critical). `x` is clamped to
/// `[0, 1]` so a small isentrope excursion just past a saturation line maps to
/// the nearest saturated single-phase density rather than an extrapolated one.
fn mixture_rho_ps_eqm(fluid: Fluid, p: f64, s: f64) -> Option<f64> {
    let t_sat = saturation_temperature(fluid, p)?;
    let sat = saturation_at_temperature(fluid, t_sat)?;
    let s_l = state_trho(fluid, t_sat, sat.rho_liquid).entropy;
    let s_v = state_trho(fluid, t_sat, sat.rho_vapour).entropy;
    // Require a well-ordered dome (`s'' > s'`); a NaN entropy fails this and, if
    // it slipped through, is caught by the `v.is_finite()` guard below.
    if s_v <= s_l {
        return None;
    }
    let x = ((s - s_l) / (s_v - s_l)).clamp(0.0, 1.0);
    let v = (1.0 - x) / sat.rho_liquid + x / sat.rho_vapour;
    if v.is_finite() && v > 0.0 {
        Some(1.0 / v)
    } else {
        None
    }
}

/// HEM **equilibrium** two-phase sound speed \[m/s\] at `(p, h)` by a central
/// finite difference of the equilibrium isentrope: `a = √((∂p/∂ρ)_s)`.
///
/// The mixture entropy `s` at `(p, h)` is obtained from the saturated-phase
/// enthalpies/entropies at `T_sat(p)`; the density is then re-equilibrated at
/// `p ± dp` **at fixed `s`** ([`mixture_rho_ps_eqm`]) and
/// `a² = 2·dp / (ρ(p+dp) − ρ(p−dp))`. Because the quality is re-solved at each
/// perturbed pressure, the derivative includes the flashing term — this is the
/// equilibrium speed, not a frozen one. Returns [`None`] if any VLE solve on the
/// perturbed points fails or the difference is non-physical (caller clamps to
/// `c_min`).
fn hem_c_two_phase_fd(fluid: Fluid, p: f64, h: f64) -> Option<f64> {
    let t_sat = saturation_temperature(fluid, p)?;
    let sat = saturation_at_temperature(fluid, t_sat)?;
    let sl = state_trho(fluid, t_sat, sat.rho_liquid);
    let sv = state_trho(fluid, t_sat, sat.rho_vapour);
    let (h_l, h_v) = (sl.enthalpy, sv.enthalpy);
    let (s_l, s_v) = (sl.entropy, sv.entropy);
    // Well-ordered dome guard (`h'' > h'`); a NaN is rejected here and, failing
    // that, by the `a2.is_finite()` guard below.
    if h_v <= h_l {
        return None;
    }
    let x = ((h - h_l) / (h_v - h_l)).clamp(0.0, 1.0);
    let s = (1.0 - x) * s_l + x * s_v;

    // Central FD step in pressure. Kept a modest fraction of `p` and floored so it
    // does not underflow at very low pressures; both perturbed points must stay
    // sub-critical for the VLE solve, which the `?` on `mixture_rho_ps_eqm` below
    // enforces (a near-critical perturbation returns None ⇒ clamp to c_min).
    let dp = (p * 1.0e-3).max(50.0);
    let rho_hi = mixture_rho_ps_eqm(fluid, p + dp, s)?;
    let rho_lo = mixture_rho_ps_eqm(fluid, p - dp, s)?;
    let drho = rho_hi - rho_lo;
    // Density must rise with pressure along the isentrope; a NaN `drho` yields a
    // NaN `a2`, caught by the `a2.is_finite()` guard below.
    if drho <= 0.0 {
        return None;
    }
    let a2 = 2.0 * dp / drho;
    if a2.is_finite() && a2 > 0.0 {
        Some(a2.sqrt())
    } else {
        None
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
    // kinetic term) into that equation. The acoustic wave speeds `u ± c` (`c` the
    // CoolProp EOS / HEM equilibrium sound speed) are unchanged.
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
/// doc for why not the total energy `ρE`) and `c` the CoolProp EOS / HEM
/// equilibrium sound speed passed in via each [`FaceState`]. `n_f` is the unit
/// face normal (owner → neighbour).
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

    /// CoolProp single-phase sound-speed wiring: Nitrogen gas at (1 bar, 300 K)
    /// gives a physical vapour speed (~350 m/s), matches the direct EOS
    /// `speed_of_sound`, and yields a tiny Mach number for a modest velocity
    /// (⇒ `β = 0`, no dissipation).
    #[test]
    fn hem_c_single_phase_gas_matches_eos() {
        // (1 bar, 300 K) Nitrogen enthalpy → single-phase (p, h).
        let st = state_ph(Fluid::Nitrogen, 1.0e5, {
            crate::flash::state_pt(Fluid::Nitrogen, 300.0, 1.0e5)
                .unwrap()
                .enthalpy
        })
        .unwrap();
        let h = st.enthalpy;
        let c = hem_sound_speed_ph(Fluid::Nitrogen, 1.0e5, h, C_MIN_MPS);
        assert!(c.is_finite() && c > C_MIN_MPS, "c = {c} not physical");
        assert!(
            (c - st.speed_of_sound).abs() / st.speed_of_sound < 1e-9,
            "single-phase HEM speed {c} should equal the EOS speed_of_sound {}",
            st.speed_of_sound
        );
        assert!(
            (300.0..420.0).contains(&c),
            "N2 gas sound speed {c} m/s outside expected band"
        );
        // Ma wiring on a uniform field: |u| / c.
        let ma = 5.0_f64 / c;
        assert!(ma < 0.02, "subsonic gas Mach {ma} should be tiny");
        assert_eq!(mach_blend(ma, 0.3, 1.0), 0.0, "subsonic ⇒ β = 0");
    }

    /// CoolProp two-phase HEM equilibrium sound speed (Water inside the dome):
    /// finite, positive, and well below the single-phase liquid/vapour speeds —
    /// the near-sonic regime the blend targets. Exercises the FD-of-(p,s)
    /// equilibrium path through the Maxwell VLE construction.
    #[test]
    fn hem_c_two_phase_equilibrium_is_finite_and_low() {
        // 1 bar. Build an in-dome enthalpy from the saturated-phase enthalpies at
        // T_sat(1 bar) so the point is guaranteed two-phase for this fluid.
        let p = 1.0e5_f64;
        let t_sat = saturation_temperature(Fluid::Water, p).expect("water sat T at 1 bar");
        let sat = saturation_at_temperature(Fluid::Water, t_sat).unwrap();
        let h_l = state_trho(Fluid::Water, t_sat, sat.rho_liquid).enthalpy;
        let h_v = state_trho(Fluid::Water, t_sat, sat.rho_vapour).enthalpy;
        let h = 0.5 * (h_l + h_v); // x ≈ 0.5, squarely in-dome
        assert!(
            matches!(phase_at_ph(Fluid::Water, p, h), PhaseAtPh::TwoPhase { .. }),
            "sample must classify two-phase"
        );
        let c = hem_sound_speed_ph(Fluid::Water, p, h, C_MIN_MPS);
        assert!(
            c.is_finite() && c > C_MIN_MPS,
            "two-phase HEM c = {c} not physical"
        );
        assert!(
            c < 800.0,
            "two-phase HEM speed {c} m/s should be well below the liquid speed"
        );
    }
}
