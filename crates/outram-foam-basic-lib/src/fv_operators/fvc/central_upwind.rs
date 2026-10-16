// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.org)
// Copyright (C) 2011-2022 OpenFOAM Foundation
// Upstream: OpenFOAM-10, applications/solvers/compressible/rhoCentralFoam/
//           rhoCentralFoam.C and centralCourantNo.H
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

//! Kurganov–Noelle–Petrova (KNP) central-upwind face flux for the compressible
//! Euler equations — **one EOS-free kernel** shared by every solver in the
//! workspace that needs it (epic gh #896, issue gh #901).
//!
//! # What it computes
//!
//! For a face with unit normal `n` (owner → neighbour) and the two
//! reconstructed face states `L` (owner-biased, OpenFOAM's `pos`) and `R`
//! (neighbour-biased, `neg`), the KNP flux per unit face area is
//!
//! ```text
//! F_KNP = (a⁺·F(W_L) − a⁻·F(W_R) + a⁺·a⁻·(W_R − W_L)) / (a⁺ − a⁻)
//! a⁻ = min(u_n,L − c_L,  u_n,R − c_R,  0)
//! a⁺ = max(u_n,L + c_L,  u_n,R + c_R,  0)
//! ```
//!
//! with `W = [ρ, ρU, w_e]` and physical flux
//! `F(W) = [ρ u_n,  ρ u_n U + p n,  ρ u_n f_e]`, where `u_n = U·n`.
//! Kurganov, A., Noelle, S. & Petrova, G. (2001), "Semidiscrete central-upwind
//! schemes for hyperbolic conservation laws and Hamilton–Jacobi equations",
//! *SIAM J. Sci. Comput.* 23(3), 707–740, eq. (3.7) with the one-sided local
//! speeds of eq. (3.8).
//!
//! # Why the energy pair `(w_e, f_e)` and not a fixed energy variable
//!
//! The flux needs only `ρ, U, p, c` and **one energy pair per side**: the
//! energy density that is conserved (`w_e`, J/m³) and the energy carried per
//! unit mass flux (`f_e`, J/kg). Letting the caller choose them lets the same
//! kernel serve both energy formulations in this workspace:
//!
//! | solver family | `w_e` | `f_e` | constructor |
//! |---|---|---|---|
//! | density-based, total energy (`rhoCentralFoam`) | `ρE`, `E = e + ½\|U\|²` | `H = E + p/ρ` | [`KnpFaceState::total_energy`] |
//! | pressure-based, static enthalpy (the `HybridAllMach` PIMPLE arrays) | `ρ·h` | `h` | [`KnpFaceState::static_enthalpy`] |
//!
//! The kernel has **no equation of state**. The pressure `p` and the sound
//! speed `c` arrive with each face state; which sound speed that is (perfect
//! gas `√(γp/ρ)`, or a two-phase homogeneous-*equilibrium* speed) is the
//! caller's physics. For a flashing two-phase flow it must be the equilibrium
//! speed, because the wave speeds `a±` are where the characteristics sit.
//!
//! # Relation to upstream `rhoCentralFoam.C`
//!
//! OpenFOAM writes the same flux in a weighted form: `a_pos = ap/(ap − am)`,
//! `aSf = am·a_pos`, `aphiv_pos = a_pos·phiv_pos − aSf`,
//! `aphiv_neg = a_neg·phiv_neg + aSf`, and e.g.
//! `phi = aphiv_pos·rho_pos + aphiv_neg·rho_neg`. Expanding those gives the KNP
//! expression above term for term; this port keeps the paper's form because
//! the three solvers it replaces (gh #901) were all written in it, and the
//! refactor had to leave their results bit-identical. OpenFOAM's `Tadmor`
//! flux-scheme switch (`aSf = −½·amaxSf`, `a_pos = ½`) is **not** ported.
//!
//! # Units
//!
//! Raw SI `f64` throughout, like the rest of the finite-volume operator layer:
//! kg/m³, m/s, Pa, J/m³, J/kg. Fluxes are per unit face area; multiply by
//! `|Sf|` (m²) for the face total.
//!
//! # What is not here
//!
//! Reconstruction (use [`super::reconstruct_pos_neg`]), boundary ghost states,
//! the time update, and any clamping of non-physical states. Those belong to
//! the solver, which is where the policy (clamp, or report and stop) is set.

use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
use crate::fields::field::Field;
use crate::fields::vol_field::{VolScalarField, VolVectorField};
use crate::primitives::vector::Vector3;

/// Floor on the KNP denominator `a⁺ − a⁻` \[m/s\]. It only acts when both
/// one-sided speeds vanish (`u = 0` and `c = 0` on both sides), where the flux
/// is `0/0`; the floor turns that into a zero flux instead of a NaN. Inherited
/// unchanged from the three copies this kernel replaced.
pub const KNP_MIN_WAVE_SPEED_SPAN: f64 = 1e-10;

/// One side's reconstructed state at a face, as the KNP flux consumes it.
///
/// Plain SI `f64`; see the module doc for the meaning of the energy pair.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnpFaceState {
    /// Density ρ \[kg/m³\].
    pub rho: f64,
    /// Velocity **U** \[m/s\]. Only its normal component `U·n` enters the wave
    /// speeds; the full vector is transported by the momentum flux.
    pub u: Vector3,
    /// Static pressure `p` \[Pa\].
    pub p: f64,
    /// Sound speed `c` \[m/s\] that sets the wave speeds `U·n ± c`. Must be
    /// non-negative; for two-phase flow, the equilibrium speed.
    pub c: f64,
    /// Conserved energy density `w_e` \[J/m³\] (`ρE`, or `ρ·h`).
    pub w_e: f64,
    /// Energy carried per unit mass flux `f_e` \[J/kg\] (`H`, or `h`), so the
    /// physical energy flux is `ρ u_n f_e`.
    pub f_e: f64,
}

impl KnpFaceState {
    /// A face state for a **total-energy** solver (`rhoCentralFoam`):
    /// `w_e = ρE` with `E = e + ½|U|²`, and `f_e = H = E + p/ρ`.
    ///
    /// Arguments: density `rho` \[kg/m³\] (must be > 0), velocity `u` \[m/s\],
    /// pressure `p` \[Pa\], sound speed `c` \[m/s\], specific internal energy `e`
    /// \[J/kg\].
    pub fn total_energy(rho: f64, u: Vector3, p: f64, c: f64, e: f64) -> Self {
        let e_tot = e + 0.5 * (u.x * u.x + u.y * u.y + u.z * u.z);
        Self {
            rho,
            u,
            p,
            c,
            w_e: rho * e_tot,
            f_e: e_tot + p / rho,
        }
    }

    /// A face state for a **static-enthalpy** solver (the segregated
    /// `∂(ρh)/∂t + ∇·(φh) = dp/dt` energy equation of the PIMPLE arrays):
    /// `w_e = ρ·h` and `f_e = h`.
    ///
    /// Arguments: density `rho` \[kg/m³\], velocity `u` \[m/s\], pressure `p`
    /// \[Pa\], sound speed `c` \[m/s\], static specific enthalpy `h` \[J/kg\].
    pub fn static_enthalpy(rho: f64, u: Vector3, p: f64, c: f64, h: f64) -> Self {
        Self {
            rho,
            u,
            p,
            c,
            w_e: rho * h,
            f_e: h,
        }
    }
}

/// A face flux per unit face area, in the face-normal direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnpFaceFlux {
    /// Mass flux `ρ u_n` \[kg/(m²·s)\].
    pub mass: f64,
    /// Momentum flux `ρ u_n U + p n` \[Pa\].
    pub momentum: Vector3,
    /// Energy flux `ρ u_n f_e` \[W/m²\].
    pub energy: f64,
}

/// The KNP one-sided local wave speeds of a face \[m/s\].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnpWaveSpeeds {
    /// `a⁻ = min(u_n,L − c_L, u_n,R − c_R, 0) ≤ 0` (upstream `am`, per unit area).
    pub a_minus: f64,
    /// `a⁺ = max(u_n,L + c_L, u_n,R + c_R, 0) ≥ 0` (upstream `ap`, per unit area).
    pub a_plus: f64,
}

/// The one-sided local wave speeds `a⁻`, `a⁺` of a face (KNP 2001 eq. 3.8;
/// upstream `am`, `ap` divided by `|Sf|`). `n_f` is the unit face normal.
#[inline]
pub fn knp_wave_speeds(l: &KnpFaceState, r: &KnpFaceState, n_f: Vector3) -> KnpWaveSpeeds {
    let u_n_l = l.u.dot(n_f);
    let u_n_r = r.u.dot(n_f);
    KnpWaveSpeeds {
        a_minus: (u_n_l - l.c).min(u_n_r - r.c).min(0.0),
        a_plus: (u_n_l + l.c).max(u_n_r + r.c).max(0.0),
    }
}

/// The physical (Euler) flux `F(W) = [ρ u_n, ρ u_n U + p n, ρ u_n f_e]` of a
/// single state through a face with unit normal `n_f`, per unit area.
///
/// This is what a solver applies on a boundary face whose ghost state equals
/// the owner state (zero-gradient extrapolation), and it is the `F(W_L)`,
/// `F(W_R)` of the KNP flux.
#[inline]
pub fn physical_face_flux(s: &KnpFaceState, n_f: Vector3) -> KnpFaceFlux {
    physical_flux_with_normal_velocity(s, s.u.dot(n_f), n_f)
}

#[inline]
fn physical_flux_with_normal_velocity(s: &KnpFaceState, u_n: f64, n_f: Vector3) -> KnpFaceFlux {
    KnpFaceFlux {
        mass: s.rho * u_n,
        momentum: (s.rho * u_n) * s.u + s.p * n_f,
        energy: s.rho * u_n * s.f_e,
    }
}

/// Shared body of [`knp_face_flux`] and [`central_face_flux`]. With
/// `with_dissipation = false` the `a⁺a⁻(W_R − W_L)` jump term is dropped.
///
/// **Rounding is part of the contract.** The momentum component is scaled by
/// `1/(a⁺ − a⁻)` (a multiply by the reciprocal), mass and energy are divided
/// by it. That is the arithmetic of the steam-tables and CoolProp copies this
/// kernel replaced, kept so their `HybridAllMach` results stayed bit-identical
/// through the refactor (gh #901). It differs from a division by at most one
/// ulp per component.
#[inline]
fn face_flux_impl(
    l: &KnpFaceState,
    r: &KnpFaceState,
    n_f: Vector3,
    with_dissipation: bool,
) -> KnpFaceFlux {
    let u_n_l = l.u.dot(n_f);
    let u_n_r = r.u.dot(n_f);

    let a_l = (u_n_l - l.c).min(u_n_r - r.c).min(0.0);
    let a_r = (u_n_l + l.c).max(u_n_r + r.c).max(0.0);
    let da = (a_r - a_l).max(KNP_MIN_WAVE_SPEED_SPAN);

    let f_l = physical_flux_with_normal_velocity(l, u_n_l, n_f);
    let f_r = physical_flux_with_normal_velocity(r, u_n_r, n_f);

    let (j_rho, j_rhou, j_e) = if with_dissipation {
        (
            a_l * a_r * (r.rho - l.rho),
            a_l * a_r * (r.rho * r.u - l.rho * l.u),
            a_l * a_r * (r.w_e - l.w_e),
        )
    } else {
        (0.0, Vector3::ZERO, 0.0)
    };

    KnpFaceFlux {
        mass: (a_r * f_l.mass - a_l * f_r.mass + j_rho) / da,
        momentum: (a_r * f_l.momentum - a_l * f_r.momentum + j_rhou) * (1.0 / da),
        energy: (a_r * f_l.energy - a_l * f_r.energy + j_e) / da,
    }
}

/// The KNP central-upwind numerical flux through a face, per unit area (see
/// the module doc for the formula and the reference). `n_f` is the unit face
/// normal pointing from the `l` (owner) side to the `r` (neighbour) side.
///
/// Consistency: for `l == r` it equals [`physical_face_flux`] up to the
/// rounding of `(a⁺ − a⁻)/(a⁺ − a⁻)`.
pub fn knp_face_flux(l: &KnpFaceState, r: &KnpFaceState, n_f: Vector3) -> KnpFaceFlux {
    face_flux_impl(l, r, n_f, true)
}

/// The KNP flux with its `a⁺a⁻(W_R − W_L)` jump term removed: the central,
/// non-dissipative part of the flux.
///
/// `knp_face_flux − central_face_flux` is therefore exactly the KNP numerical
/// dissipation, which is what the pressure-based `HybridAllMach` solvers add
/// as a deferred correction. It vanishes identically when `l == r`.
pub fn central_face_flux(l: &KnpFaceState, r: &KnpFaceState, n_f: Vector3) -> KnpFaceFlux {
    face_flux_impl(l, r, n_f, false)
}

/// The largest local characteristic speed at a face, `max(|a⁻|, |a⁺|)`
/// \[m/s\].
///
/// This is upstream `rhoCentralFoam.C`'s **first** `amaxSf = max(mag(am),
/// mag(ap))`, per unit area. It is the speed a classical CFL bound uses:
/// `Δt ≤ Co·Δx / max_f a_max,f`. Compare [`central_flux_speed_max`], which
/// is the quantity upstream finally feeds to its Courant number.
pub fn local_wave_speed_max(l: &KnpFaceState, r: &KnpFaceState, n_f: Vector3) -> f64 {
    let w = knp_wave_speeds(l, r, n_f);
    w.a_minus.abs().max(w.a_plus.abs())
}

/// Upstream `rhoCentralFoam.C`'s **reused** `amaxSf`, per unit area \[m/s\]:
/// `max(|aphiv_pos|, |aphiv_neg|)` with `a_pos = a⁺/(a⁺ − a⁻)`,
/// `aSf = a⁻·a_pos`, `aphiv_pos = a_pos·u_n,L − aSf` and
/// `aphiv_neg = (1 − a_pos)·u_n,R + aSf`.
///
/// Upstream's `centralCourantNo.H` computes
/// `CoNum = ½·max_cells(Σ_faces amaxSf / V)·Δt` from this quantity, and
/// `setDeltaT.H` scales `Δt` by `maxCo/CoNum`. A solver that wants to
/// reproduce upstream's time-step control uses this function with that
/// formula. For a quiescent face it equals `c/2` (where
/// [`local_wave_speed_max`] gives `c`), and the `½·Σ_faces` then makes
/// upstream's `CoNum` half the classical `c·Δt/Δx` in 1-D.
///
/// The denominator `a⁺ − a⁻` is floored at [`KNP_MIN_WAVE_SPEED_SPAN`]
/// (upstream divides unguarded).
pub fn central_flux_speed_max(l: &KnpFaceState, r: &KnpFaceState, n_f: Vector3) -> f64 {
    let w = knp_wave_speeds(l, r, n_f);
    let a_pos = w.a_plus / (w.a_plus - w.a_minus).max(KNP_MIN_WAVE_SPEED_SPAN);
    let a_sf = w.a_minus * a_pos;
    let aphiv_pos = a_pos * l.u.dot(n_f) - a_sf;
    let aphiv_neg = (1.0 - a_pos) * r.u.dot(n_f) + a_sf;
    aphiv_pos.abs().max(aphiv_neg.abs())
}

/// Continuous Mach-blend weight `β(Ma) = clamp((Ma − lo)/(hi − lo), 0, 1)`.
///
/// Used by the pressure-based `HybridAllMach` solvers to switch the KNP
/// dissipation on only near the sonic point: `β = 0` for `Ma ≤ lo` (no added
/// dissipation), `β = 1` for `Ma ≥ hi`, linear and monotone in between.
/// `lo`, `hi` are dimensionless Mach numbers with `hi > lo` (the arrays default
/// to `0.3` and `1.0`); a degenerate `hi ≤ lo` becomes a hard step at `hi`.
#[inline]
pub fn mach_blend(ma: f64, lo: f64, hi: f64) -> f64 {
    if hi > lo {
        ((ma - lo) / (hi - lo)).clamp(0.0, 1.0)
    } else if ma >= hi {
        1.0
    } else {
        0.0
    }
}

/// Extract one Cartesian component (`comp` = 0 → x, 1 → y, any other → z) of a
/// [`VolVectorField`] as a [`VolScalarField`], with matching scalar boundary
/// conditions, so that MUSCL reconstruction of that component sees correct
/// boundary gradients.
///
/// Mirrors OpenFOAM's `U.component(cmpt)`. `rhoCentralFoam` needs it because
/// it reconstructs `U` component by component (`interpolate(U, pos/neg)`).
///
/// Every `BoundaryCondition` variant is mapped explicitly (no catch-all), so a
/// new variant is a compile error here rather than a silently wrong boundary:
/// variants with a vector payload are mapped component-wise, payload-free
/// markers are kept as they are, and `FlowRateInletVelocity`'s scalar
/// whole-patch flow rate is carried through unchanged.
pub fn velocity_component(u: &VolVectorField, comp: usize) -> VolScalarField {
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
                BoundaryCondition::FixedGradient(g) => BoundaryCondition::FixedGradient(pick(*g)),
                BoundaryCondition::Mixed {
                    value_fraction,
                    ref_value,
                    ref_grad,
                } => BoundaryCondition::Mixed {
                    value_fraction: *value_fraction,
                    ref_value: pick(*ref_value),
                    ref_grad: pick(*ref_grad),
                },
                BoundaryCondition::MixedField {
                    value_fraction,
                    ref_value,
                    ref_grad,
                } => BoundaryCondition::MixedField {
                    value_fraction: value_fraction.clone(),
                    ref_value: Field::new(ref_value.as_slice().iter().map(|v| pick(*v)).collect()),
                    ref_grad: Field::new(ref_grad.as_slice().iter().map(|v| pick(*v)).collect()),
                },
                BoundaryCondition::InletOutlet { inlet_value } => BoundaryCondition::InletOutlet {
                    inlet_value: pick(*inlet_value),
                },
                BoundaryCondition::OutletInlet { outlet_value } => BoundaryCondition::OutletInlet {
                    outlet_value: pick(*outlet_value),
                },
                BoundaryCondition::Freestream { freestream_value } => {
                    BoundaryCondition::Freestream {
                        freestream_value: pick(*freestream_value),
                    }
                }
                BoundaryCondition::FixedFluxPressure { gradient } => {
                    BoundaryCondition::FixedFluxPressure {
                        gradient: pick(*gradient),
                    }
                }
                BoundaryCondition::TotalPressure { p0 } => {
                    BoundaryCondition::TotalPressure { p0: pick(*p0) }
                }
                BoundaryCondition::FlowRateInletVelocity {
                    volumetric_flow_rate,
                } => BoundaryCondition::FlowRateInletVelocity {
                    volumetric_flow_rate: *volumetric_flow_rate,
                },
                BoundaryCondition::Symmetry => BoundaryCondition::Symmetry,
                BoundaryCondition::Slip => BoundaryCondition::Slip,
                BoundaryCondition::NoSlip => BoundaryCondition::NoSlip,
                BoundaryCondition::Wedge => BoundaryCondition::Wedge,
                BoundaryCondition::PressureInletOutletVelocity => {
                    BoundaryCondition::PressureInletOutletVelocity
                }
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

    const X: Vector3 = Vector3 {
        x: 1.0,
        y: 0.0,
        z: 0.0,
    };

    fn liquid() -> KnpFaceState {
        KnpFaceState::static_enthalpy(950.0, Vector3::new(3.0, 0.0, 0.0), 7.0e6, 1200.0, 5.0e5)
    }

    /// Ported from the steam-tables copy: `L == R` ⇒ the KNP flux equals the
    /// central flux, so the deferred-correction dissipation `knp − central` is
    /// zero. This is what makes a uniform field see no added flux.
    #[test]
    fn identical_states_give_zero_dissipation() {
        let s = liquid();
        let knp = knp_face_flux(&s, &s, X);
        let cen = central_face_flux(&s, &s, X);
        assert!(
            (knp.mass - cen.mass).abs() < 1e-9,
            "mass dissipation nonzero"
        );
        assert!(
            (knp.momentum.x - cen.momentum.x).abs() < 1e-6,
            "momentum dissipation nonzero"
        );
        assert!(
            (knp.energy - cen.energy).abs() < 1e-3,
            "energy dissipation nonzero"
        );
    }

    /// Ported from the steam-tables copy: fully supersonic flow on both sides
    /// (`u_n − c > 0`) ⇒ `a⁻ = 0`, so the KNP flux is the pure upwind (left)
    /// physical flux.
    #[test]
    fn supersonic_one_sided_is_upwind() {
        let l =
            KnpFaceState::static_enthalpy(20.0, Vector3::new(500.0, 0.0, 0.0), 1.0e6, 300.0, 2.6e6);
        let r =
            KnpFaceState::static_enthalpy(18.0, Vector3::new(520.0, 0.0, 0.0), 0.9e6, 300.0, 2.5e6);
        let knp = knp_face_flux(&l, &r, X);
        let up = physical_face_flux(&l, X);
        assert!((knp.mass - up.mass).abs() / up.mass.abs() < 1e-9);
        assert!((knp.momentum.x - up.momentum.x).abs() / up.momentum.x.abs() < 1e-9);
        assert!((knp.energy - up.energy).abs() / up.energy.abs() < 1e-9);
        assert_eq!(knp_wave_speeds(&l, &r, X).a_minus, 0.0);
    }

    /// Mirror image: fully supersonic flow towards the owner ⇒ `a⁺ = 0` and the
    /// flux is the right-hand physical flux.
    #[test]
    fn supersonic_reverse_is_upwind_from_the_right() {
        let l = KnpFaceState::static_enthalpy(
            20.0,
            Vector3::new(-520.0, 0.0, 0.0),
            1.0e6,
            300.0,
            2.6e6,
        );
        let r = KnpFaceState::static_enthalpy(
            18.0,
            Vector3::new(-500.0, 0.0, 0.0),
            0.9e6,
            300.0,
            2.5e6,
        );
        let knp = knp_face_flux(&l, &r, X);
        let up = physical_face_flux(&r, X);
        assert_eq!(knp_wave_speeds(&l, &r, X).a_plus, 0.0);
        assert!((knp.mass - up.mass).abs() / up.mass.abs() < 1e-9);
        assert!((knp.energy - up.energy).abs() / up.energy.abs() < 1e-9);
    }

    /// Ported from the steam-tables copy: `β(Ma)` is zero below `lo`, one above
    /// `hi`, linear between, monotone and clamped to `[0, 1]`; a degenerate
    /// window is a hard step.
    #[test]
    fn mach_blend_monotone_and_clamped() {
        assert_eq!(mach_blend(0.1, 0.3, 1.0), 0.0);
        assert_eq!(mach_blend(0.3, 0.3, 1.0), 0.0);
        assert_eq!(mach_blend(1.0, 0.3, 1.0), 1.0);
        assert_eq!(mach_blend(1.5, 0.3, 1.0), 1.0);
        assert!((mach_blend(0.65, 0.3, 1.0) - 0.5).abs() < 1e-12);
        let mut prev = -1.0;
        for i in 0..=20 {
            let ma = i as f64 / 20.0 * 1.2;
            let b = mach_blend(ma, 0.3, 1.0);
            assert!(b >= prev - 1e-12 && (0.0..=1.0).contains(&b));
            prev = b;
        }
        assert_eq!(mach_blend(0.4, 0.5, 0.5), 0.0);
        assert_eq!(mach_blend(0.5, 0.5, 0.5), 1.0);
    }

    /// The two energy constructors put the documented pair into `(w_e, f_e)`,
    /// and the energy flux of a single state is `ρ u_n f_e`.
    #[test]
    fn energy_constructors_set_the_documented_pair() {
        let u = Vector3::new(3.0, 4.0, 0.0); // |U|² = 25
        let te = KnpFaceState::total_energy(2.0, u, 1.0e5, 300.0, 1.0e5);
        assert_eq!(te.w_e, 2.0 * (1.0e5 + 12.5));
        assert_eq!(te.f_e, 1.0e5 + 12.5 + 1.0e5 / 2.0);
        let se = KnpFaceState::static_enthalpy(2.0, u, 1.0e5, 300.0, 3.0e5);
        assert_eq!(se.w_e, 6.0e5);
        assert_eq!(se.f_e, 3.0e5);
        let f = physical_face_flux(&te, X);
        assert_eq!(f.mass, 6.0);
        assert_eq!(f.energy, 6.0 * te.f_e);
        assert_eq!(f.momentum, Vector3::new(6.0 * 3.0 + 1.0e5, 6.0 * 4.0, 0.0));
    }

    /// Consistency `F_KNP(W, W) = F(W)` and conservation under reversal:
    /// swapping the sides and flipping the normal negates the flux.
    #[test]
    fn consistent_and_antisymmetric_under_reversal() {
        let s = liquid();
        let k = knp_face_flux(&s, &s, X);
        let f = physical_face_flux(&s, X);
        assert!((k.mass - f.mass).abs() <= 1e-12 * f.mass.abs());
        assert!((k.momentum.x - f.momentum.x).abs() <= 1e-12 * f.momentum.x.abs());
        assert!((k.energy - f.energy).abs() <= 1e-12 * f.energy.abs());

        let l = KnpFaceState::total_energy(1.0, Vector3::new(10.0, 0.0, 0.0), 1.0e5, 374.2, 2.5e5);
        let r =
            KnpFaceState::total_energy(0.125, Vector3::new(-5.0, 0.0, 0.0), 1.0e4, 334.7, 2.0e5);
        let fwd = knp_face_flux(&l, &r, X);
        let rev = knp_face_flux(&r, &l, Vector3::new(-1.0, 0.0, 0.0));
        assert!((fwd.mass + rev.mass).abs() <= 1e-12 * fwd.mass.abs().max(1.0));
        assert!((fwd.momentum.x + rev.momentum.x).abs() <= 1e-9 * fwd.momentum.x.abs());
        assert!((fwd.energy + rev.energy).abs() <= 1e-9 * fwd.energy.abs());
    }

    /// The two wave-speed measures against hand values: a quiescent face gives
    /// `max(|a±|) = c` and upstream's reused `amaxSf = c/2`; a supersonic face
    /// gives `u + c` and `u`.
    #[test]
    fn wave_speed_measures_match_hand_values() {
        let q = KnpFaceState::total_energy(1.0, Vector3::ZERO, 1.0e5, 340.0, 2.5e5);
        assert_eq!(local_wave_speed_max(&q, &q, X), 340.0);
        assert_eq!(central_flux_speed_max(&q, &q, X), 170.0);

        let s = KnpFaceState::total_energy(1.0, Vector3::new(500.0, 0.0, 0.0), 1.0e5, 340.0, 2.5e5);
        assert_eq!(local_wave_speed_max(&s, &s, X), 840.0);
        assert_eq!(central_flux_speed_max(&s, &s, X), 500.0);

        // Degenerate face (u = 0, c = 0): finite, zero, no NaN.
        let z = KnpFaceState::static_enthalpy(1.0, Vector3::ZERO, 0.0, 0.0, 0.0);
        assert_eq!(central_flux_speed_max(&z, &z, X), 0.0);
        let k = knp_face_flux(&z, &z, X);
        assert!(k.mass == 0.0 && k.energy == 0.0 && k.momentum == Vector3::ZERO);
    }

    /// The KNP flux of the Sod initial discontinuity (left 1 kg/m³, 1e5 Pa;
    /// right 0.125 kg/m³, 1e4 Pa; at rest; γ = 1.4) against the hand-evaluated
    /// formula. At rest `a⁺ = c_L = −a⁻`, so `mass = c_L·(ρ_R − ρ_L)·(−½)`.
    #[test]
    fn sod_interface_flux_matches_hand_evaluation() {
        let gamma = 1.4_f64;
        let c_l = (gamma * 1.0e5 / 1.0).sqrt();
        let c_r = (gamma * 1.0e4 / 0.125).sqrt();
        let l = KnpFaceState::total_energy(1.0, Vector3::ZERO, 1.0e5, c_l, 1.0e5 / 0.4);
        let r = KnpFaceState::total_energy(0.125, Vector3::ZERO, 1.0e4, c_r, 1.0e4 / (0.4 * 0.125));
        let k = knp_face_flux(&l, &r, X);
        let a = c_l.max(c_r);
        let expect_mass = -a * a * (0.125 - 1.0) / (2.0 * a);
        let expect_mom = (a * 1.0e5 + a * 1.0e4) / (2.0 * a);
        let expect_e = -a * a * (r.w_e - l.w_e) / (2.0 * a);
        assert!((k.mass - expect_mass).abs() < 1e-9 * expect_mass.abs());
        assert!((k.momentum.x - expect_mom).abs() < 1e-9 * expect_mom);
        assert!((k.energy - expect_e).abs() < 1e-9 * expect_e.abs());
        assert!(k.mass > 0.0, "mass must flow from high to low pressure");
    }

    /// **Bit-identity pin (gh #901).** The kernel must reproduce, bit for bit,
    /// the arithmetic of the private KNP copy the steam-tables and CoolProp
    /// `HybridAllMach` arrays carried before the refactor (inlined below as
    /// `legacy`, transcribed from `tampines-steam-tables` at 10af85038,
    /// `rhoPimpleFoam/central_upwind.rs::face_flux_impl`). That is what kept
    /// their `HybridAllMach` fields bit-identical through the refactor; this
    /// test stops a later "tidy-up" (e.g. dividing the momentum instead of
    /// multiplying by the reciprocal) from changing them silently.
    #[test]
    fn matches_pre_refactor_hybrid_copy_bit_for_bit() {
        fn legacy(
            l: &KnpFaceState,
            r: &KnpFaceState,
            n: Vector3,
            diss: bool,
        ) -> (f64, Vector3, f64) {
            let (hl, hr) = (l.f_e, r.f_e);
            let u_n_l = l.u.dot(n);
            let u_n_r = r.u.dot(n);
            let a_l = (u_n_l - l.c).min(u_n_r - r.c).min(0.0);
            let a_r = (u_n_l + l.c).max(u_n_r + r.c).max(0.0);
            let da = (a_r - a_l).max(1e-10);
            let f_cont_l = l.rho * u_n_l;
            let f_cont_r = r.rho * u_n_r;
            let f_mom_l = (l.rho * u_n_l) * l.u + l.p * n;
            let f_mom_r = (r.rho * u_n_r) * r.u + r.p * n;
            let f_ener_l = l.rho * u_n_l * hl;
            let f_ener_r = r.rho * u_n_r * hr;
            let (j_rho, j_rhou, j_rhoe) = if diss {
                (
                    a_l * a_r * (r.rho - l.rho),
                    a_l * a_r * (r.rho * r.u - l.rho * l.u),
                    a_l * a_r * (r.rho * hr - l.rho * hl),
                )
            } else {
                (0.0, Vector3::ZERO, 0.0)
            };
            (
                (a_r * f_cont_l - a_l * f_cont_r + j_rho) / da,
                (a_r * f_mom_l - a_l * f_mom_r + j_rhou) * (1.0 / da),
                (a_r * f_ener_l - a_l * f_ener_r + j_rhoe) / da,
            )
        }
        // Deterministic spread of states: liquid, two-phase (slow c), vapour,
        // both flow directions, oblique normal.
        let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut next = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let n_obl = Vector3::new(0.6, 0.8, 0.0);
        for i in 0..2000 {
            let mut st = || {
                KnpFaceState::static_enthalpy(
                    1.0 + 999.0 * next(),
                    Vector3::new(400.0 * next() - 200.0, 50.0 * next() - 25.0, 0.0),
                    1.0e5 + 1.0e7 * next(),
                    1.0 + 1500.0 * next(),
                    1.0e5 + 2.7e6 * next(),
                )
            };
            let (l, r) = (st(), st());
            let n = if i % 2 == 0 { X } else { n_obl };
            for diss in [true, false] {
                let k = if diss {
                    knp_face_flux(&l, &r, n)
                } else {
                    central_face_flux(&l, &r, n)
                };
                let (m, mo, e) = legacy(&l, &r, n, diss);
                assert_eq!(k.mass.to_bits(), m.to_bits(), "mass, case {i}");
                assert_eq!(k.momentum.x.to_bits(), mo.x.to_bits(), "mom x, case {i}");
                assert_eq!(k.momentum.y.to_bits(), mo.y.to_bits(), "mom y, case {i}");
                assert_eq!(k.momentum.z.to_bits(), mo.z.to_bits(), "mom z, case {i}");
                assert_eq!(k.energy.to_bits(), e.to_bits(), "energy, case {i}");
            }
        }
    }

    /// `velocity_component` picks the requested component in the interior and
    /// maps vector boundary payloads component-wise.
    #[test]
    fn velocity_component_maps_interior_and_boundaries() {
        use crate::interface::one_dimensional_meshing::create_one_d_mesh;
        use std::sync::Arc;
        use uom::si::area::square_meter;
        use uom::si::f64::{Area, Length};
        use uom::si::length::meter;
        let mesh = Arc::new(
            create_one_d_mesh(
                Length::new::<meter>(1.0),
                Area::new::<square_meter>(0.01),
                4,
            )
            .expect("mesh"),
        );
        let mut u = VolVectorField::zero("U", mesh.clone());
        for c in 0..4 {
            u.internal.as_mut_slice()[c] = Vector3::new(c as f64, 10.0 + c as f64, -1.0);
        }
        u.boundary[0].bc = BoundaryCondition::FixedValue(Vector3::new(7.0, 8.0, 9.0));
        let uy = velocity_component(&u, 1);
        assert_eq!(uy.internal.as_slice(), &[10.0, 11.0, 12.0, 13.0]);
        assert!(matches!(uy.boundary[0].bc, BoundaryCondition::FixedValue(v) if v == 8.0));
        let uz = velocity_component(&u, 2);
        assert!(uz.internal.as_slice().iter().all(|v| *v == -1.0));
        assert_eq!(uz.name, "U_2");
    }
}
