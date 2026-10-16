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
//! implicit-acoustics PIMPLE solver and borrow **only the KNP face-flux math**.
//! ~~That math is re-expressed in this crate's own `openfoam_source` field
//! types.~~ **CORRECTED 2026-10-16 (gh #901):** the face-flux math is no longer
//! copied here. It is the shared, EOS-free kernel
//! `outram_foam_basic_lib::fv_operators::fvc::central_upwind`, which this
//! module calls (as do the CoolProp twin and appbuilder's `RhoCentralFoam`).
//! This module keeps only what is specific to IAPWS-IF97: the HEM equilibrium
//! sound speed ([`hem_sound_speed_ph`]) and the choice of the static-enthalpy
//! energy pair when the hybrid assembler builds each face state
//! (`KnpFaceState::static_enthalpy`). The stand-alone marcher, its
//! `RhoCentralFoam` struct and `run()` loop are *not* copied.
//!
//! Two choices versus the density-based solver:
//!
//! - **No perfect-gas EOS.** The pressure `p` and the **HEM equilibrium sound
//!   speed** `c` are passed in per face state, computed by
//!   [`hem_sound_speed_ph`] from the real IAPWS-IF97 `(p, h)` closure, never
//!   Wood–Wallis / frozen / perfect-gas.
//! - **Static-enthalpy energy form.** The energy pair is `w_e = ρ·he`,
//!   `f_e = he` (not the total energy `ρE`, `H`), matching the array's
//!   segregated EEqn, which advances `∂(ρh)/∂t + ∇·(φh) = dp/dt` in static
//!   enthalpy with the pressure work carried by a separate `dp/dt` source.
//!   Dissipating `ρE` instead would re-inject `−Δp` pressure work (and the
//!   kinetic term) into that equation: during the strong Edwards rarefaction
//!   the large `Δp` across the flashing front then over-cools the near-break
//!   cell straight through the 273.15 K isotherm.
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

use crate::interfaces::functional_programming::ph_flash_eqm::w_ph_eqm_hem;

use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{AvailableEnergy, Pressure};
use uom::si::pressure::pascal;
use uom::si::velocity::meter_per_second;

/// The shared KNP kernel this module's hybrid assembler calls (gh #901). Re-
/// exported so the assembler in [`super`] names one module for the whole
/// hybrid path.
pub(crate) use outram_foam_basic_lib::fv_operators::fvc::central_upwind::{
    central_face_flux, knp_face_flux, mach_blend, KnpFaceState,
};

/// Defensive lower bound \[m/s\] on the HEM sound speed. The equilibrium
/// two-phase speed can legitimately fall to a few tens of m/s near the
/// bubble/dew points, and the Kieffer closure
/// ([`crate::region_4_vap_liq_equilibrium::w_ps_eqm_region4_kieffer`]) is
/// AI-generated/unvalidated, so a non-finite or non-positive result is clamped
/// to this floor rather than allowed to poison the Mach number or the KNP wave
/// speeds. It sits far below any physical water/steam sound speed, so it never
/// perturbs `β` in the regimes the blend actually acts on.
pub(crate) const C_MIN_MPS: f64 = 1.0;

/// HEM **equilibrium** sound speed `c` \[m/s\] at a cell's `(p, h)` state,
/// floored at `c_min`.
///
/// This is the sound speed used for *both* the regime Mach number *and* the KNP
/// characteristic wave speeds `u ± c`. It is the public equilibrium speed
/// [`w_ph_eqm_hem`] (Kieffer eq. 28 in the dome with a finite-difference
/// fallback, the region forward speed outside it), never Wood–Wallis (frozen)
/// or perfect-gas, converted to raw SI and clamped to `c ≥ c_min` (`c_min` in
/// m/s, e.g. [`C_MIN_MPS`]) so a pathological non-finite value cannot poison
/// downstream arithmetic.
///
/// ## Parameters
/// - `p_pa`  — pressure \[Pa\]
/// - `h_jkg` — static specific enthalpy \[J/kg\]
/// - `c_min` — defensive lower bound on the returned speed \[m/s\]
pub(crate) fn hem_sound_speed_ph(p_pa: f64, h_jkg: f64, c_min: f64) -> f64 {
    let p = Pressure::new::<pascal>(p_pa);
    let h = AvailableEnergy::new::<joule_per_kilogram>(h_jkg);

    let c = w_ph_eqm_hem(p, h).get::<meter_per_second>();

    if c.is_finite() && c > c_min {
        c
    } else {
        c_min
    }
}

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

    // The flux-kernel unit tests that used to live here (`L == R` gives zero
    // dissipation, supersonic flow is pure upwind, `β(Ma)` is monotone and
    // clamped) moved with the kernel to
    // `outram_foam_basic_lib::fv_operators::fvc::central_upwind::tests`
    // (gh #901). What stays is what is specific to this crate: the IF97 HEM
    // sound speed and the bit-identity gate of the hybrid assembler.

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

    /// Hash of the pre-refactor dissipation under
    /// `KnpFaceClosure::ReconstructedPressure` (see the gate below).
    const PINNED_HASH_RECONSTRUCTED: u64 = 0x8232_9ef7_7a39_dde1;
    /// Hash of the pre-refactor dissipation under
    /// `KnpFaceClosure::EosConsistentPressure` (see the gate below).
    const PINNED_HASH_EOS_CONSISTENT: u64 = 0xd37b_0afb_8e70_77f1;

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

    /// A dense, near-sonic two-phase state on which the hybrid KNP dissipation
    /// is genuinely active: 5.5 → 4.5 MPa, low-quality two-phase enthalpy
    /// (1.20 → 1.40 MJ/kg, mixture density far above the 100 kg/m³ taper) and
    /// a velocity ramp of 20 → 115 m/s against an equilibrium sound speed of
    /// tens of m/s, so faces sit in the `β(Ma) > 0` band.
    fn dense_near_sonic_array() -> super::super::TampinesSteamArray {
        use uom::si::area::square_meter;
        use uom::si::f64::{Area, Length, Time};
        use uom::si::length::meter;
        use uom::si::time::second;
        let n = 20_usize;
        let mut a = super::super::TampinesSteamArray::new(
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(0.01),
            n as i64,
            Time::new::<second>(1e-4),
        )
        .expect("valid 1-D geometry");
        for c in 0..n {
            let s = c as f64 / (n - 1) as f64;
            a.p.internal[c] = 5.5e6 - 1.0e6 * s;
            a.he.internal[c] = 1.20e6 + 0.20e6 * s;
            a.u.internal[c] = Vector3::new(20.0 + 95.0 * s, 0.0, 0.0);
        }
        a.correct_thermo()
            .expect("test state lies inside the IF97 (p,h) range");
        a
    }

    /// **Bit-identity gate for the shared-kernel refactor (gh #901, M3).**
    ///
    /// Methodology: assemble the `HybridAllMach` deferred-correction
    /// dissipation (`assemble_hybrid_dissipation`, this crate's only consumer
    /// of the KNP flux) on [`dense_near_sonic_array`] under both
    /// `KnpFaceClosure` options, and hash every output word. The pinned hashes
    /// were measured on the pre-refactor code (base commit `10af85038`, where
    /// the KNP flux was a private copy in this file). Pass criterion: exact
    /// equality, i.e. moving the flux onto
    /// `outram_foam_basic_lib::fv_operators::fvc::central_upwind` changed no
    /// bit. The test also asserts that the dissipation is non-zero, so the
    /// hash cannot pass vacuously on a state where every face has `β = 0`.
    #[test]
    fn hybrid_dissipation_is_bit_identical_to_pre_refactor() {
        let mut a = dense_near_sonic_array();
        let d = a.assemble_hybrid_dissipation();
        let active = d.d_phi.iter().filter(|v| **v != 0.0).count();
        assert!(
            active > 0,
            "dissipation inactive: the gate would be vacuous"
        );
        let h_rec = hash_dissipation(&d);

        a.knp_face_closure = super::super::KnpFaceClosure::EosConsistentPressure;
        let d_eos = a.assemble_hybrid_dissipation();
        let h_eos = hash_dissipation(&d_eos);
        println!(
            "MEASURE active faces = {active}/{}, hash(Reconstructed) = {h_rec:#018x}, \
             hash(EosConsistent) = {h_eos:#018x}",
            d.d_phi.len()
        );
        assert_eq!(h_rec, PINNED_HASH_RECONSTRUCTED);
        assert_eq!(h_eos, PINNED_HASH_EOS_CONSISTENT);
    }
}
