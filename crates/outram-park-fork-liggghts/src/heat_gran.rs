// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// PORTED FROM UPSTREAM — provenance (see the crate NOTICE):
//   Upstream project : LIGGGHTS-PUBLIC (DCS Computing GmbH / JKU Linz)
//   Upstream files   : src/fix_heat_gran_conduction.cpp
//                      (`FixHeatGranCond::post_force_eval`),
//                      src/fix_heat_gran.cpp (flux reset in
//                      `initial_integrate`),
//                      src/fix_scalar_transport_equation.cpp
//                      (`advanceQtyExplicit`, `final_integrate`)
//   Upstream commit  : 3d5c00f20519e6bb6eb6756f51f1ad36564e649d (2024-06-07)
//   Upstream licence : "GNU Public License, version 2 or later" -> used here
//                      under the "or later" option as GPL-3.0.
//   Upstream copyright: Copyright 2012- DCS Computing GmbH, Linz;
//                       Copyright 2009-2012 JKU Linz.
// Do not strip this block during refactors (workspace CLAUDE.md).
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

//! # `fix heat/gran/conduction` — LIGGGHTS' contact conduction (gh:#1412)
//!
//! A translation of upstream's particle–particle contact conduction and its
//! explicit temperature update, **alongside** the clean-room
//! [`crate::thermal`] (kept callable as the named alternative; the two are
//! compared in `tests/heat_conduction_cross_code.rs`).
//!
//! ## The model (as upstream implements it)
//!
//! For every pair in the neighbour list that overlaps (`rsq < (r_i+r_j)^2`):
//!
//! ```text
//!   A    = -pi/4 (r-ri-rj)(r+ri-rj)(r-ri+rj)(r+ri+rj) / r^2     (contact_area overlap)
//!        = pi min(ri,rj)^2   if r < max(ri,rj)                  (one sphere inside the other)
//!   hc   = 4 k_i k_j / (k_i + k_j) sqrt(A)      (0 if either k < 1e-6)
//!   flux = (T_j - T_i) hc
//!   heatFlux[i] += flux;   heatFlux[j] -= flux
//!   T_i += (heatFlux_i + source_i) dt / (m_i cp_i)    at final_integrate
//! ```
//!
//! `hc = 4 k_s' sqrt(A)` with `A = pi a^2` gives `hc = 2 sqrt(pi) k_s a`, where
//! `a` is the **geometric** chord radius of the overlapping spheres; the
//! clean-room model uses `2 k_s a_c` with the Hertzian `a_c = sqrt(R* delta)`.
//! Their ratio is measured in the cross-code test.
//!
//! ## Order matters
//!
//! The flux sum of a particle is accumulated in the order of the pair list, so
//! the temperatures are bit-identical to upstream only with
//! [`crate::neighbor_list::NeighborOrder::Liggghts`]; with the default cell
//! grid they agree to round-off.
//!
//! ## Not translated
//!
//! `contact_area constant` and `projection` are translated; `area_correction
//! yes` (needs `youngsModulusOriginal` coarse-graining), `store_contact_data`,
//! the directional heat flux, the heat sources (`fix heat/gran` source terms
//! are zero here), wall conduction (`fix wall/gran ... heattransfer`), the
//! implicit integrator (`advanceQtyImplicit`) and `fix_modify every`.
//! `thermalConductivity` and `thermalCapacity` are per atom type, as upstream.

use crate::particle::Particle;
use crate::DemError;

/// `SMALL_FIX_HEAT_GRAN` (`fix_heat_gran.h`): below this conductivity a
/// contact does not conduct.
const SMALL_FIX_HEAT_GRAN: f64 = 1.0e-6;

/// `SMALL` of `fix_scalar_transport_equation.cpp`: a capacity this small
/// freezes the temperature.
const SMALL_CAPACITY: f64 = 1.0e-8;

/// How the conduction contact area is obtained (`contact_area` keyword).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ConductionContactArea {
    /// The area of the circle where the two overlapping spheres intersect
    /// (upstream default, `contact_area overlap`).
    #[default]
    Overlap,
    /// A fixed area `[m^2]` (`contact_area constant <A>`).
    Constant(f64),
    /// The projected area of the larger sphere, `pi max(ri,rj)^2`
    /// (`contact_area projection`).
    Projection,
}

/// `fix heat/gran/conduction` for a particle ensemble: per-type thermal
/// conductivity `[W/m/K]` and heat capacity `[J/kg/K]`, and the contact-area
/// mode. The temperatures live in [`Particle::temperature`].
#[derive(Debug, Clone, PartialEq)]
pub struct HeatGranConduction {
    conductivity: Vec<f64>,
    capacity: Vec<f64>,
    area: ConductionContactArea,
}

impl HeatGranConduction {
    /// Per-type conductivity and capacity (index = atom type, 0-based).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if the lists are empty or of different
    /// length, or a conductivity is negative (`Fix heat/gran/conduction:
    /// Thermal conductivity must not be < 0`).
    pub fn new(conductivity: Vec<f64>, capacity: Vec<f64>) -> Result<Self, DemError> {
        if conductivity.is_empty() || conductivity.len() != capacity.len() {
            return Err(DemError::InvalidInput(
                "conductivity and capacity must be non-empty and of one length (per atom type)"
                    .into(),
            ));
        }
        if conductivity.iter().any(|&k| k < 0.0 || !k.is_finite()) {
            return Err(DemError::InvalidInput(
                "thermal conductivity must be finite and not negative".into(),
            ));
        }
        Ok(Self {
            conductivity,
            capacity,
            area: ConductionContactArea::Overlap,
        })
    }

    /// One material for every particle (`peratomtype` with a single type).
    ///
    /// # Errors
    ///
    /// As [`HeatGranConduction::new`].
    pub fn single_type(conductivity: f64, capacity: f64) -> Result<Self, DemError> {
        Self::new(vec![conductivity], vec![capacity])
    }

    /// Choose the contact-area mode.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for `Constant(a)` with `a <= 0`
    /// (`'contact_area constant' value must be > 0`).
    pub fn with_contact_area(mut self, area: ConductionContactArea) -> Result<Self, DemError> {
        if let ConductionContactArea::Constant(a) = area {
            if !(a > 0.0) {
                return Err(DemError::InvalidInput(
                    "'contact_area constant' value must be > 0".into(),
                ));
            }
        }
        self.area = area;
        Ok(self)
    }

    /// The contact area `[m^2]` of two overlapping spheres of radii `radi`,
    /// `radj` at centre distance `r`, in the configured mode
    /// (`post_force_eval`).
    #[must_use]
    pub fn contact_area(&self, radi: f64, radj: f64, r: f64) -> f64 {
        match self.area {
            ConductionContactArea::Overlap => {
                if r < radi.max(radj) {
                    // one sphere is inside the other: area of the smaller sphere
                    let mut a = radi.min(radj);
                    a *= a * std::f64::consts::PI;
                    a
                } else {
                    -std::f64::consts::PI / 4.0
                        * ((r - radi - radj)
                            * (r + radi - radj)
                            * (r - radi + radj)
                            * (r + radi + radj))
                        / (r * r)
                }
            }
            ConductionContactArea::Constant(a) => a,
            ConductionContactArea::Projection => {
                let rmax = radi.max(radj);
                std::f64::consts::PI * rmax * rmax
            }
        }
    }

    /// The pair conductance `hc` `[W/K]` for conductivities `tcoi`, `tcoj` and
    /// contact area `area`: `4 k_i k_j / (k_i + k_j) sqrt(A)`, zero if either
    /// conductivity is below `1e-6`.
    #[must_use]
    pub fn pair_conductance(tcoi: f64, tcoj: f64, area: f64) -> f64 {
        if tcoi < SMALL_FIX_HEAT_GRAN || tcoj < SMALL_FIX_HEAT_GRAN {
            0.0
        } else {
            4.0 * tcoi * tcoj / (tcoi + tcoj) * area.sqrt()
        }
    }

    /// Accumulate every contact's heat flux into `flux` (zeroed by the
    /// caller), visiting `pairs` in order: `flux[i] += q; flux[j] -= q` with
    /// `q = (T_j - T_i) hc` (`post_force_eval`, `newton off`, `i` and `j` both
    /// owned). `types[p]` is the atom type of particle `p`.
    pub fn accumulate(
        &self,
        pairs: &[(u32, u32)],
        particles: &[Particle],
        types: &[usize],
        flux: &mut [f64],
    ) {
        for &(i, j) in pairs {
            let (i, j) = (i as usize, j as usize);
            let (a, b) = (&particles[i], &particles[j]);
            let delx = a.position.x - b.position.x;
            let dely = a.position.y - b.position.y;
            let delz = a.position.z - b.position.z;
            let rsq = delx * delx + dely * dely + delz * delz;
            let (radi, radj) = (a.radius, b.radius);
            let radsum = radi + radj;
            if rsq < radsum * radsum {
                let r = rsq.sqrt();
                let area = self.contact_area(radi, radj, r);
                let hc =
                    Self::pair_conductance(self.conductivity[types[i]], self.conductivity[types[j]], area);
                let q = (b.temperature - a.temperature) * hc;
                flux[i] += q;
                flux[j] -= q;
            }
        }
    }

    /// `advanceQtyExplicit` with a per-type capacity and zero source:
    /// `T_i += (flux_i + source_i * nevery) dt / (m_i cp_i)` for
    /// `|cp_i| > 1e-8`.
    pub fn advance(&self, particles: &mut [Particle], types: &[usize], flux: &[f64], dt: f64) {
        for (i, p) in particles.iter_mut().enumerate() {
            let capacity = self.capacity[types[i]];
            if capacity.abs() > SMALL_CAPACITY {
                let source = 0.0_f64;
                p.temperature += (flux[i] + source * 1.0) * dt / (p.mass * capacity);
            }
        }
    }

    /// Number of atom types this fix has properties for.
    #[must_use]
    pub fn n_types(&self) -> usize {
        self.conductivity.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::particle::Vec3;
    use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
    use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

    fn sphere(x: f64, t: f64) -> Particle {
        Particle::new(
            Vec3::new(x, 0.0, 0.0),
            Vec3::zero(),
            Vec3::zero(),
            Mass::new::<kilogram>(0.01),
            Length::new::<meter>(0.005),
            ThermodynamicTemperature::new::<kelvin>(t),
        )
        .unwrap()
    }

    /// Closed form: touching-with-overlap pair, flux antisymmetric, energy
    /// `sum m cp dT` conserved by the explicit update.
    #[test]
    fn two_body_exchange_conserves_energy() {
        let h = HeatGranConduction::single_type(50.0, 500.0).unwrap();
        let mut ps = vec![sphere(-0.0048, 400.0), sphere(0.0048, 300.0)];
        let mut flux = vec![0.0; 2];
        h.accumulate(&[(0, 1)], &ps, &[0, 0], &mut flux);
        assert!(flux[0] < 0.0 && flux[1] > 0.0, "heat flows hot -> cold");
        assert_eq!(flux[0], -flux[1]);
        let e0: f64 = ps.iter().map(|p| p.mass * 500.0 * p.temperature).sum();
        h.advance(&mut ps, &[0, 0], &flux, 1e-3);
        let e1: f64 = ps.iter().map(|p| p.mass * 500.0 * p.temperature).sum();
        assert!(((e1 - e0) / e0).abs() < 1e-14);
        assert!(ps[0].temperature < 400.0 && ps[1].temperature > 300.0);
    }

    /// Non-overlapping pairs in the list do not conduct; low-k contacts do
    /// not conduct; the areas follow the three modes.
    #[test]
    fn guards_and_area_modes() {
        let h = HeatGranConduction::single_type(50.0, 500.0).unwrap();
        let ps = vec![sphere(0.0, 400.0), sphere(0.0101, 300.0)];
        let mut flux = vec![0.0; 2];
        h.accumulate(&[(0, 1)], &ps, &[0, 0], &mut flux);
        assert_eq!(flux, vec![0.0, 0.0]);
        assert_eq!(HeatGranConduction::pair_conductance(1e-7, 50.0, 1.0), 0.0);
        let r = 0.0096;
        let a = h.contact_area(0.005, 0.005, r);
        let want = -std::f64::consts::PI / 4.0 * ((r - 0.01) * r * r * (r + 0.01)) / (r * r);
        assert!((a - want).abs() < 1e-18 && a > 0.0);
        // r < max(ri, rj): the smaller sphere's area
        let inside = h.contact_area(0.005, 0.003, 0.004);
        assert!((inside - std::f64::consts::PI * 0.003 * 0.003).abs() < 1e-18);
        let c = h.clone().with_contact_area(ConductionContactArea::Constant(2e-6)).unwrap();
        assert_eq!(c.contact_area(0.005, 0.005, r), 2e-6);
        let p = h.with_contact_area(ConductionContactArea::Projection).unwrap();
        assert_eq!(p.contact_area(0.005, 0.003, r), std::f64::consts::PI * 0.005 * 0.005);
        assert!(HeatGranConduction::single_type(1.0, 1.0)
            .unwrap()
            .with_contact_area(ConductionContactArea::Constant(0.0))
            .is_err());
        assert!(HeatGranConduction::new(vec![-1.0], vec![1.0]).is_err());
    }
}
