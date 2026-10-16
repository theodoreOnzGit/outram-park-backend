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
//! implicit-acoustics PIMPLE solver and borrow **only the KNP face-flux math**.
//! ~~That math is re-expressed in this crate's own `openfoam_source` field
//! types.~~ **CORRECTED 2026-10-16 (gh #901):** the face-flux math is no longer
//! copied here. It is the shared, EOS-free kernel
//! `outram_foam_basic_lib::fv_operators::fvc::central_upwind`, which this
//! module re-exports (as do the steam-tables twin and appbuilder's
//! `RhoCentralFoam`). What is specific to this crate is the sound speed,
//! [`crate::flash::hem_sound_speed_ph`] (public since the same change), and the
//! static-enthalpy energy pair chosen when the assembler builds each face
//! state (`KnpFaceState::static_enthalpy`). The stand-alone marcher, its
//! `RhoCentralFoam` struct and `run()` loop are *not* copied.
//!
//! Two changes versus the density-based port:
//!
//! - **No perfect-gas EOS.** The upstream code closes the state with
//!   `p = (γ−1)ρe` and `c = √(γp/ρ)`. That is deleted. The pressure `p` and the
//!   sound speed `c` are passed in per face state (`KnpFaceState`) —
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
//! For each internal face the assembler forms the KNP flux (`knp_face_flux`)
//! and the **jump-free central flux** (`central_face_flux`) (the same flux with
//! the `a_L·a_R·(W_R − W_L)` dissipation term zeroed). Their difference is the
//! pure numerical dissipation. It is scaled by the Mach-blend weight
//! `β(Ma) ∈ [0, 1]` (`mach_blend`) and injected as a deferred correction into
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

/// The CoolProp HEM sound speed, public in [`crate::flash`] since gh #901.
pub(crate) use crate::flash::hem_sound_speed_ph;

/// The shared KNP kernel this module's hybrid assembler calls (gh #901).
pub(crate) use outram_foam_basic_lib::fv_operators::fvc::central_upwind::{
    central_face_flux, knp_face_flux, mach_blend, KnpFaceState,
};

/// Defensive lower bound \[m/s\] on the sound speed. The equilibrium two-phase
/// speed can legitimately fall to a few tens of m/s near the bubble/dew points,
/// so a non-finite or non-positive result (or a two-phase FD that degenerates)
/// is clamped to this floor rather than allowed to poison the Mach number or the
/// KNP wave speeds. It sits far below any physical fluid sound speed, so it never
/// perturbs `β` in the regimes the blend actually acts on.
pub(crate) const C_MIN_MPS: f64 = 1.0;

/// Extract one component (0=x, 1=y, 2=z) of a [`VolVectorField`] as a
/// [`VolScalarField`], carrying the matching scalar boundary conditions so the
/// MUSCL reconstruction's cell gradients are correct at the boundaries.
///
/// Mirrors OpenFOAM's `U.component(cmpt)` used inside `rhoCentralFoam` to build
/// the per-component `interpolate(U, pos/neg)` face states.
///
/// **Still a copy (gh #901).** `outram_foam_basic_lib::fv_operators::fvc::central_upwind::velocity_component`
/// is the shared version, but it takes basic-lib's `VolVectorField`, while this
/// crate's `openfoam_source` fields (and their smaller `BoundaryCondition`
/// enum) are still a vendored copy. This one goes when those field types are
/// unified (the #492 follow-on, epic #896 step M10).
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
    use crate::flash::state_ph;
    use crate::fluid::Fluid;
    use crate::props::state_trho;
    use crate::vle::{phase_at_ph, saturation_at_temperature, saturation_temperature, PhaseAtPh};

    // The flux-kernel unit tests that used to live here (`L == R` gives zero
    // dissipation, supersonic flow is pure upwind, `β(Ma)` is monotone and
    // clamped) moved with the kernel to
    // `outram_foam_basic_lib::fv_operators::fvc::central_upwind::tests`
    // (gh #901). What stays is what is specific to this crate: the CoolProp
    // sound speed and the bit-identity gate of the hybrid assembler.

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

    /// Hash of the pre-refactor dissipation (see the gate below).
    const PINNED_HASH_DENSE_NITROGEN: u64 = 0x298e_7560_4114_1afa;

    /// FNV-1a over the IEEE-754 bit patterns of the assembled hybrid
    /// dissipation, so a one-ulp change anywhere in the KNP arithmetic moves
    /// the hash.
    fn hash_dissipation(d: &super::super::HybridDissipation) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |x: f64| {
            for b in x.to_bits().to_le_bytes() {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        for v in &d.d_phi {
            eat(*v);
        }
        for m in &d.mom_src {
            eat(m.x);
            eat(m.y);
            eat(m.z);
        }
        h
    }

    /// **Bit-identity gate for the shared-kernel refactor (gh #901, M3).**
    ///
    /// Methodology: dense supercritical-pressure Nitrogen (20 → 18 MPa,
    /// 300 → 330 K, so ρ ≈ 200 kg/m³, far above the default 100 kg/m³ taper)
    /// with a velocity ramp of 150 → 450 m/s against a sound speed of roughly
    /// 400–450 m/s, so faces sit in the `β(Ma) > 0` band. Assemble the
    /// `HybridAllMach` deferred-correction dissipation
    /// (`assemble_hybrid_dissipation`, this crate's only consumer of the KNP
    /// flux) and hash every output word. The pinned hash was measured on the
    /// pre-refactor code (base commit `10af85038`, where the KNP flux was a
    /// private copy in this file). Pass criterion: exact equality. The test
    /// also asserts that the dissipation is non-zero, so it cannot pass
    /// vacuously.
    #[test]
    fn hybrid_dissipation_is_bit_identical_to_pre_refactor() {
        use uom::si::area::square_meter;
        use uom::si::f64::{Area, Length, Time};
        use uom::si::length::meter;
        use uom::si::time::second;
        let n = 20_usize;
        let mut a = super::super::OPCPFluidArray::new(
            Fluid::Nitrogen,
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(0.01),
            n as i64,
            Time::new::<second>(1e-4),
        )
        .expect("valid 1-D geometry");
        for c in 0..n {
            let s = c as f64 / (n - 1) as f64;
            let p = 2.0e7 - 2.0e6 * s;
            let t = 300.0 + 30.0 * s;
            a.p.internal[c] = p;
            a.he.internal[c] = crate::flash::state_pt(Fluid::Nitrogen, t, p)
                .expect("single-phase N2")
                .enthalpy;
            a.u.internal[c] = Vector3::new(150.0 + 300.0 * s, 0.0, 0.0);
        }
        a.correct_thermo();
        let d = a.assemble_hybrid_dissipation();
        let active = d.d_phi.iter().filter(|v| **v != 0.0).count();
        assert!(
            active > 0,
            "dissipation inactive: the gate would be vacuous"
        );
        let h = hash_dissipation(&d);
        println!(
            "MEASURE active faces = {active}/{}, hash = {h:#018x}",
            d.d_phi.len()
        );
        assert_eq!(h, PINNED_HASH_DENSE_NITROGEN);
    }
}
