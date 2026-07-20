// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/thermoMechanics/legacyThermoMechanics/
//                    include/solveThermalMechanics.H  (axial fuel/CR expansion +
//                    meshDisp assembly), thermoMechanics.C (deformMesh)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal author: Carlo Fiorina (EPFL)
//   Upstream license: GPL-3.0
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
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # Thermal-expansion geometry feedback
//!
//! The geometry/density feedback that thermo-mechanics produces for the
//! neutronics and thermal-hydraulics regions: the axial thermal expansion of
//! the fuel and control-rod columns, and the assembly of the mesh-displacement
//! vector `meshDisp` that GeN-Foam maps back onto the other meshes to move
//! points (`deformMesh`) and shift cross-sections.
//!
//! ## Axial fuel / control-rod expansion (the reactor-physics feedback)
//!
//! GeN-Foam solves two 1-D scalar transport equations along the pin direction
//! `ê` (`globalOptions/pinDirection`) for the accumulated axial displacement of
//! the fuel and control-rod stacks (`include/solveThermalMechanics.H`):
//!
//! ```text
//!   div(ê·Sf, fuelDisp) = α_fuel (T_fuel − T_fuelRef)
//!   div(−ê·Sf, CRDisp)  = −α_CR   (T_struct − T_CRRef)
//! ```
//!
//! On a 1-D column of cells stacked along `ê` this upwind divergence is exactly
//! a cumulative integral of the local thermal strain: the displacement at the
//! outlet face of cell `i` is `Σ_{j≤i} α ΔT_j Δz_j`. Physically it is the
//! free axial thermal growth of a bar, `u(z) = ∫₀^z α (T(z') − T_ref) dz'`.
//! Axial fuel expansion lengthens the active fuel column (a negative,
//! stabilising reactivity feedback in fast reactors); control-rod-stack
//! expansion inserts the rods further. Both are the geometry changes this
//! module feeds to neutronics.
//!
//! ## Mesh-displacement assembly
//!
//! The vector actually mapped to the other meshes is (upstream):
//!
//! ```text
//!   meshDisp = (disp − (disp·ê) ê) + (fuelDisp + CRDisp) ê
//! ```
//!
//! i.e. the radial (transverse-to-pin) part of the elastic displacement plus the
//! axial fuel/CR expansion along `ê`. The axial component of the 2-D/3-D elastic
//! displacement is *removed* and replaced by the physically-resolved 1-D axial
//! expansion, so that spurious axial variation of the plane solve does not leak
//! into the coupled geometry. See [`assemble_mesh_displacement`].

use super::material::{Displacement, ThermalExpansionCoeff};
use outram_foam_basic_lib::primitives::Vector3;
use uom::si::f64::{Length, TemperatureInterval};
use uom::si::length::meter;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::temperature_interval::kelvin;

/// Free axial thermal expansion of a uniformly-heated bar, `Δu = α ΔT L`.
///
/// The closed-form limit of the axial-expansion transport equation for a bar of
/// length `L` at a uniform temperature rise `ΔT` above its stress-free
/// reference. This is the total elongation of the fuel (or control-rod) stack.
///
/// # Parameters
///
/// - `alpha` — linear thermal-expansion coefficient `α` (1/K).
/// - `delta_t` — uniform temperature rise `ΔT = T − T_ref` (kelvin interval).
/// - `length` — undeformed bar length `L` (m).
///
/// # Returns
///
/// The axial elongation `Δu` as a [`Displacement`] (metres).
pub fn free_axial_expansion(
    alpha: ThermalExpansionCoeff,
    delta_t: TemperatureInterval,
    length: Length,
) -> Displacement {
    // α ΔT is dimensionless strain; × L gives a length.
    let strain = alpha.get::<per_kelvin>() * delta_t.get::<kelvin>();
    Length::new::<meter>(strain * length.get::<meter>())
}

/// Cumulative axial-displacement profile of a stack of cells along the pin.
///
/// Ports the upwind `div(ê·Sf, fuelDisp) = α (T − T_ref)` solve on a 1-D column:
/// the returned value `u[i]` is the axial displacement of the **outlet face** of
/// cell `i`, i.e. the running sum `Σ_{j≤i} α ΔT_j Δz_j` of per-cell thermal
/// growth. The inlet face (base of the stack) is anchored at zero displacement.
///
/// # Parameters
///
/// - `alpha` — expansion coefficient `α` (1/K), assumed uniform along the stack.
/// - `delta_t` — per-cell temperature rise `ΔT_i = T_i − T_ref` (kelvin
///   interval), ordered from the anchored base to the free tip.
/// - `cell_height` — per-cell height `Δz_i` (m); same length/order as `delta_t`.
///
/// # Returns
///
/// A `Vec` of cumulative axial [`Displacement`]s, one per cell (outlet-face
/// value). Empty inputs yield an empty vector. If the two slices differ in
/// length the shorter is used.
pub fn axial_expansion_profile(
    alpha: ThermalExpansionCoeff,
    delta_t: &[TemperatureInterval],
    cell_height: &[Length],
) -> Vec<Displacement> {
    let a = alpha.get::<per_kelvin>();
    let mut running = 0.0_f64;
    let n = delta_t.len().min(cell_height.len());
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        running += a * delta_t[i].get::<kelvin>() * cell_height[i].get::<meter>();
        out.push(Length::new::<meter>(running));
    }
    out
}

/// Assemble the coupled mesh-displacement vector `meshDisp` (metres).
///
/// `meshDisp = (disp − (disp·ê) ê) + axial ê`: the transverse-to-pin part of the
/// elastic displacement `disp` plus the axially-resolved fuel/control-rod
/// expansion `axial` along the (normalised) pin direction `ê`. This is the
/// vector GeN-Foam maps onto the neutronics/TH meshes and passes to
/// `movePoints`.
///
/// # Parameters
///
/// - `disp` — the elastic displacement vector at the point, in **metres**
///   (`outram-foam-basic-lib` has no `uom` vector type; unit is documented).
/// - `axial` — the axial fuel+CR expansion `fuelDisp + CRDisp` along `ê`
///   ([`Displacement`], metres).
/// - `pin_direction` — the pin orientation `ê`; normalised internally (a zero
///   vector yields just the axial term dropped and `disp` returned unchanged).
///
/// # Returns
///
/// The mesh-displacement vector in **metres** (a [`Vector3`]).
pub fn assemble_mesh_displacement(
    disp: Vector3,
    axial: Displacement,
    pin_direction: Vector3,
) -> Vector3 {
    let mag = pin_direction.mag();
    if mag <= outram_foam_basic_lib::primitives::VSMALL {
        return disp;
    }
    let e_hat = pin_direction / mag;
    // Remove the axial component of the plane displacement, then add the
    // 1-D-resolved axial expansion along ê.
    let radial = disp - e_hat * disp.dot(e_hat);
    radial + e_hat * axial.get::<meter>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::length::{meter, millimeter};

    /// V&V — free thermal expansion of a bar. Methodology: a bar of length
    /// L = 1 m at uniform ΔT = 300 K with α = 1.2e-5/K elongates by
    /// Δu = αΔT L = 3.6 mm (analytical). Result (2026-07-15): Δu = 3.600000 mm,
    /// |err| < 1e-9 mm. Pass.
    #[test]
    fn free_expansion_matches_analytical() {
        let du = free_axial_expansion(
            ThermalExpansionCoeff::new::<per_kelvin>(1.2e-5),
            TemperatureInterval::new::<kelvin>(300.0),
            Length::new::<meter>(1.0),
        );
        assert!(
            (du.get::<millimeter>() - 3.6).abs() < 1e-9,
            "Δu = {} mm",
            du.get::<millimeter>()
        );
    }

    /// V&V — cumulative profile reduces to the closed form for a uniform stack.
    /// Methodology: 10 cells of 0.1 m at a uniform ΔT = 300 K must accumulate to
    /// the same 3.6 mm total tip displacement as [`free_axial_expansion`], and be
    /// linear in height (u[i] = αΔT·(i+1)·Δz). Result (2026-07-15): tip = 3.600 mm,
    /// mid-stack (i=4) = 1.800 mm; both within 1e-9 mm. Pass.
    #[test]
    fn cumulative_profile_matches_free_expansion_at_tip() {
        let alpha = ThermalExpansionCoeff::new::<per_kelvin>(1.2e-5);
        let dts: Vec<_> = (0..10)
            .map(|_| TemperatureInterval::new::<kelvin>(300.0))
            .collect();
        let dz: Vec<_> = (0..10).map(|_| Length::new::<meter>(0.1)).collect();
        let profile = axial_expansion_profile(alpha, &dts, &dz);
        assert_eq!(profile.len(), 10);
        let tip = profile.last().unwrap().get::<millimeter>();
        assert!((tip - 3.6).abs() < 1e-9, "tip = {tip} mm");
        // Cell index 4 = outlet of 5th cell ⇒ 5·(αΔT·Δz) = 1.8 mm.
        assert!((profile[4].get::<millimeter>() - 1.8).abs() < 1e-9);
    }

    /// V&V — mesh-displacement assembly geometry. Methodology: with pin
    /// direction ê = +z, a displacement disp = (0.001, 0.002, 0.005) m and an
    /// axial expansion of 3 mm, the assembled meshDisp must keep the radial
    /// (x,y) parts, drop the original z = 5 mm, and substitute the axial 3 mm.
    /// Result (2026-07-15): meshDisp = (0.001, 0.002, 0.003) m exactly. Pass.
    #[test]
    fn mesh_displacement_replaces_axial_component() {
        let disp = Vector3::new(0.001, 0.002, 0.005);
        let axial = Length::new::<millimeter>(3.0);
        let e = Vector3::new(0.0, 0.0, 1.0);
        let md = assemble_mesh_displacement(disp, axial, e);
        assert!((md.x - 0.001).abs() < 1e-12);
        assert!((md.y - 0.002).abs() < 1e-12);
        assert!((md.z - 0.003).abs() < 1e-12, "z = {}", md.z);
    }
}
