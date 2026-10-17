// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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

//! # Beginner thermal profile (gh:#949, under #945)
//!
//! A coarse steady temperature field for **any CSG shape**, from a stated
//! power and a ballpark way of removing it:
//!
//! ```rust
//! use outram_mc_libs::thermal_profile::{HeatRemoval, ThermalProblem};
//! use uom::si::{f64::Power, power::kilowatt};
//! let thermal = ThermalProblem::uniform_heat(Power::new::<kilowatt>(1.0))
//!     .removed_by(HeatRemoval::WaterCooled);
//! # let _ = thermal;
//! ```
//!
//! then [`ThermalProblem::solve`] on a geometry, naming which materials
//! conduct heat and which of them generate it ([`ThermalRegion`]).
//!
//! **What happens underneath.** The conducting regions are cut into cubic
//! voxels by locating each voxel centre in the CSG geometry
//! ([`outram_foam_basic_lib::steady_conduction::voxel_mesh`]). The heated
//! voxels share the power uniformly (`q''' = P / V_heated`, with `V_heated`
//! the voxel volume, so the total is exactly `P`). Every face between a
//! conducting voxel and anything else is the cooled surface. The steady
//! conduction solve is `outram-foam-basic-lib`'s, with the material
//! conductivities of [`ThermalMaterial`] re-evaluated at the local
//! temperature. The result carries the **energy balance**.
//!
//! **The staircase surface.** A voxelised surface has the wrong area (a
//! sphere's tends to 3/2 of the true one), which matters only for a
//! convective preset. Each surface face's film is therefore weighted by
//! `|n_axis . n_csg|`, where `n_csg` is the normal of the CSG surface the
//! body actually exits through, found by tracking from the voxel centre
//! along the face normal. Faces where no CSG surface is found keep weight 1
//! and are **counted** in [`ThermalProfile::faces_without_csg_normal`].
//! Measured effect on a sphere: `outram-foam-basic-lib`'s
//! `vv_voxel_sphere_converges_only_with_the_film_area_correction`.
//!
//! **This is an educational ballpark estimate, not a thermal-hydraulic
//! analysis**, and the result says so as a fact
//! ([`ThermalProfile::kind`]). Serious thermal-hydraulics is `nee_soon`'s.
//! Projecting the field onto transport temperatures is #945's, not this
//! module's; [`ThermalProfile::temperature_at`] is the lookup it will use.
//!
//! Units: positions are cm (`Position`), as everywhere in this crate's
//! geometry; every other physical quantity at this API is a `uom` type.
//!
//! Status: untrusted AI-generated draft (2026-10-17) per `RESPONSIBLE_USE.md`.
//! For research, education and V&V only.

mod materials;
mod presets;
pub mod render;
#[cfg(test)]
mod tests;

pub use materials::ThermalMaterial;
pub use render::{SliceColour, SlicePlane};
pub use presets::{
    HeatRemoval, AIR_AMBIENT_TEMPERATURE_K, AIR_COOLED_HTC_W_PER_M2_K, WATER_COOLANT_TEMPERATURE_K,
    WATER_COOLED_HTC_W_PER_M2_K,
};

use outram_foam_basic_lib::steady_conduction::{
    voxel_mesh, EnergyBalance, PatchCondition, SteadyConductionError, SteadyConductionProblem,
    SteadyConductionSettings, SteadyConductionSolution, ThermalBoundary, VoxelGrid, VoxelMesh,
};
use thiserror::Error;
use uom::si::f64::{
    HeatTransfer, Length, Power, ThermodynamicTemperature, Volume, VolumetricPowerDensity,
};
use uom::si::length::{centimeter, meter};
use uom::si::power::watt;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::volume::cubic_meter;
use uom::si::volumetric_power_density::watt_per_cubic_meter;

use crate::geometry::cell::SurfaceToken;
use crate::geometry::geometry::{Crossing, Geometry};
use crate::geometry::position::{Direction, Position};

/// What went wrong building or solving a beginner thermal profile.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ThermalProfileError {
    /// The problem statement is incomplete or inconsistent.
    #[error("invalid thermal problem: {0}")]
    InvalidInput(String),
    /// The conducting body reaches the edge of the meshed box, so part of it
    /// would be cut off and its cut face wrongly cooled.
    #[error("the conducting body touches the edge of the thermal mesh box; enlarge the box")]
    BodyTouchesBox,
    /// The conduction solve failed.
    #[error(transparent)]
    Conduction(#[from] SteadyConductionError),
}

/// One material of the geometry that conducts heat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThermalRegion {
    /// Index of the material in the transport model (the `CellFill::Material`
    /// index the geometry uses).
    pub material: usize,
    /// Its conductivity model.
    pub conductor: ThermalMaterial,
    /// Whether it shares the generated power.
    pub heated: bool,
}

impl ThermalRegion {
    /// A material that generates heat (fuel).
    #[must_use]
    pub fn heated(material: usize, conductor: ThermalMaterial) -> Self {
        Self {
            material,
            conductor,
            heated: true,
        }
    }

    /// A material that conducts but does not generate heat (cladding,
    /// reflector).
    #[must_use]
    pub fn unheated(material: usize, conductor: ThermalMaterial) -> Self {
        Self {
            material,
            conductor,
            heated: false,
        }
    }
}

/// The box the conducting body is meshed in, and how finely.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThermalMesh {
    /// Lower corner of the box, cm.
    pub lower: Position,
    /// Upper corner of the box, cm.
    pub upper: Position,
    /// Voxels across the longest side of the box. Coarse is the point
    /// (gh:#949); 24 to 48 is typical.
    pub voxels_across: usize,
}

/// What kind of result a [`ThermalProfile`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileKind {
    /// An educational estimate from ballpark heat-removal presets and a
    /// coarse voxel mesh. Not a thermal-hydraulic analysis.
    EducationalBallparkEstimate,
}

/// The heat-removal values actually used.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatRemovalUsed {
    /// The preset.
    pub preset: HeatRemoval,
    /// Its film coefficient (`None` for a fixed surface temperature).
    pub heat_transfer_coefficient: Option<HeatTransfer>,
    /// Its coolant temperature, or the fixed surface temperature.
    pub reference_temperature: ThermodynamicTemperature,
    /// Whether both values carry a literature citation.
    pub fully_cited: bool,
}

/// The user's thermal problem: a power, generated uniformly in the heated
/// regions, and how it is removed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThermalProblem {
    power: Power,
    removal: Option<HeatRemoval>,
}

impl ThermalProblem {
    /// Heat generated uniformly in the heated regions at total power `power`.
    #[must_use]
    pub fn uniform_heat(power: Power) -> Self {
        Self {
            power,
            removal: None,
        }
    }

    /// How the heat leaves (required: there is no default).
    #[must_use]
    pub fn removed_by(mut self, removal: HeatRemoval) -> Self {
        self.removal = Some(removal);
        self
    }

    /// Mesh the conducting regions of `geometry`, solve, and return the
    /// profile.
    ///
    /// # Errors
    /// [`ThermalProfileError::InvalidInput`] for no heat removal, a negative
    /// or non-finite power, no heated region, a material listed twice, or a
    /// bad box; [`ThermalProfileError::BodyTouchesBox`];
    /// [`ThermalProfileError::Conduction`] when the mesh or the solve fails.
    pub fn solve(
        &self,
        geometry: &Geometry,
        regions: &[ThermalRegion],
        mesh: &ThermalMesh,
    ) -> Result<ThermalProfile, ThermalProfileError> {
        let removal = self.removal.ok_or_else(|| {
            ThermalProfileError::InvalidInput(
                "say how the heat is removed: .removed_by(HeatRemoval::...)".into(),
            )
        })?;
        let power_w = self.power.get::<watt>();
        if !(power_w >= 0.0) || !power_w.is_finite() {
            return Err(ThermalProfileError::InvalidInput(format!(
                "power must be finite and non-negative, got {power_w} W"
            )));
        }
        if !regions.iter().any(|r| r.heated) {
            return Err(ThermalProfileError::InvalidInput(
                "no region is marked heated".into(),
            ));
        }
        for (i, r) in regions.iter().enumerate() {
            if regions[..i].iter().any(|o| o.material == r.material) {
                return Err(ThermalProfileError::InvalidInput(format!(
                    "material {} is listed twice",
                    r.material
                )));
            }
        }
        let grid = voxel_grid(mesh)?;
        let zone_of =
            |m: Option<usize>| m.and_then(|m| regions.iter().position(|r| r.material == m));
        let vm = voxel_mesh(&grid, |p| {
            let r = to_position(p);
            zone_of(
                geometry
                    .locate(r, Direction::new(1.0, 0.0, 0.0), SurfaceToken::NONE)?
                    .material,
            )
        })?;
        if vm
            .cell_voxel
            .iter()
            .any(|v| (0..3).any(|a| v[a] == 0 || v[a] + 1 == grid.counts[a]))
        {
            return Err(ThermalProfileError::BodyTouchesBox);
        }

        // Uniform q''' over the heated voxels, so the total is exactly P.
        let fv = vm.conduction.mesh.clone();
        let heated_volume: f64 = (0..fv.n_cells)
            .filter(|c| regions[vm.conduction.cell_zone[*c]].heated)
            .map(|c| fv.cell_volumes[c])
            .sum();
        let q = power_w / heated_volume;
        let mut zone_q =
            vec![VolumetricPowerDensity::new::<watt_per_cubic_meter>(0.0); vm.conduction.n_zones];
        for (z, r) in regions.iter().enumerate().take(zone_q.len()) {
            if r.heated {
                zone_q[z] = VolumetricPowerDensity::new::<watt_per_cubic_meter>(q);
            }
        }

        let (condition, missing) = match removal {
            HeatRemoval::FixedSurfaceTemperature(t) => (
                PatchCondition::uniform(ThermalBoundary::FixedTemperature(t)),
                0,
            ),
            convective => {
                let boundary = ThermalBoundary::Convective {
                    coefficient: convective
                        .heat_transfer_coefficient()
                        .expect("convective presets carry h"),
                    ambient: convective.reference_temperature(),
                };
                let (fraction, missing) = film_fractions(geometry, regions, &vm);
                (
                    PatchCondition::with_film_area_fraction(boundary, fraction),
                    missing,
                )
            }
        };
        let problem = SteadyConductionProblem::new(vm.conduction.clone(), zone_q, vec![condition])?;
        let solution = problem.solve(
            |z, t| regions[z].conductor.conductivity(t),
            &SteadyConductionSettings::default(),
        )?;

        let mut outside = Vec::new();
        for c in 0..solution.n_cells() {
            let r = regions[vm.conduction.cell_zone[c]];
            if let Some((lo, hi)) = r.conductor.validity_kelvin() {
                let t = solution.temperature(c).get::<kelvin>();
                if (t < lo || t > hi) && !outside.contains(&r.conductor) {
                    outside.push(r.conductor);
                }
            }
        }
        let heated_zones: Vec<usize> = (0..regions.len()).filter(|z| regions[*z].heated).collect();
        Ok(ThermalProfile {
            kind: ProfileKind::EducationalBallparkEstimate,
            heat_removal: HeatRemovalUsed {
                preset: removal,
                heat_transfer_coefficient: removal.heat_transfer_coefficient(),
                reference_temperature: removal.reference_temperature(),
                fully_cited: removal.fully_cited(),
            },
            power: self.power,
            heat_source: VolumetricPowerDensity::new::<watt_per_cubic_meter>(q),
            heated_volume: Volume::new::<cubic_meter>(heated_volume),
            peak_temperature: solution.peak_temperature().1,
            mean_heated_temperature: solution
                .volume_mean_temperature(&heated_zones)
                .expect("a heated region has cells"),
            mean_temperature: solution.volume_mean_temperature(&[]).expect("cells exist"),
            mean_surface_temperature: solution
                .surface_mean_temperature(0)
                .expect("a bounded body has a surface"),
            energy_balance: solution.energy_balance.clone(),
            faces_without_csg_normal: missing,
            conductivity_outside_range: outside,
            voxel_size: grid.spacing,
            mesh: vm,
            solution,
        })
    }
}

/// The voxel grid covering `mesh`'s box, in metres.
fn voxel_grid(mesh: &ThermalMesh) -> Result<VoxelGrid, ThermalProfileError> {
    let lo = [mesh.lower.x, mesh.lower.y, mesh.lower.z];
    let hi = [mesh.upper.x, mesh.upper.y, mesh.upper.z];
    let side = (0..3)
        .map(|a| hi[a] - lo[a])
        .fold(f64::NEG_INFINITY, f64::max);
    if mesh.voxels_across == 0
        || (0..3).any(|a| !(hi[a] > lo[a]) || !hi[a].is_finite() || !lo[a].is_finite())
    {
        return Err(ThermalProfileError::InvalidInput(
            "the thermal mesh box needs upper > lower on every axis and voxels_across > 0".into(),
        ));
    }
    let h = side / mesh.voxels_across as f64;
    let counts = [0, 1, 2].map(|a| ((hi[a] - lo[a]) / h).ceil().max(1.0) as usize);
    Ok(VoxelGrid {
        origin: lo.map(Length::new::<centimeter>),
        spacing: Length::new::<centimeter>(h),
        counts,
    })
}

/// A point of the FV mesh (metres) as a geometry position (cm).
fn to_position(p: [Length; 3]) -> Position {
    Position::new(
        p[0].get::<centimeter>(),
        p[1].get::<centimeter>(),
        p[2].get::<centimeter>(),
    )
}

/// `|n_axis . n_csg|` for every surface face, and how many faces found no
/// CSG surface (those keep 1).
///
/// From the owner voxel's centre, track along the face's axis normal across
/// CSG surfaces until the material beyond is not a conducting region; the
/// normal of the surface crossed there is the body's true normal.
fn film_fractions(
    geometry: &Geometry,
    regions: &[ThermalRegion],
    vm: &VoxelMesh,
) -> (Vec<f64>, usize) {
    const NUDGE_CM: f64 = 1.0e-7;
    const MAX_CROSSINGS: usize = 64;
    let mesh = &vm.conduction.mesh;
    let start = mesh.patches[0].start;
    let conducting = |m: Option<usize>| m.is_some_and(|m| regions.iter().any(|r| r.material == m));
    let mut missing = 0;
    let fractions = (0..vm.n_surface_faces())
        .map(|i| {
            let owner = mesh.owner[start + i];
            let c = mesh.cell_centres[owner];
            let n = vm.surface_normal[i];
            let u = Direction::new(n[0], n[1], n[2]);
            let mut r = Position::new(
                Length::new::<meter>(c.x).get::<centimeter>(),
                Length::new::<meter>(c.y).get::<centimeter>(),
                Length::new::<meter>(c.z).get::<centimeter>(),
            );
            for _ in 0..MAX_CROSSINGS {
                let Some(path) = geometry.locate(r, u, SurfaceToken::NONE) else {
                    break;
                };
                let hit = geometry.distance_to_boundary(&path);
                if !hit.distance.is_finite() {
                    break;
                }
                let x = r + Position::new(u.u, u.v, u.w) * hit.distance;
                let next = x + Position::new(u.u, u.v, u.w) * NUDGE_CM;
                if let Crossing::Surface(s) = hit.crossing {
                    let beyond = geometry
                        .locate(next, u, SurfaceToken::NONE)
                        .and_then(|p| p.material);
                    if !conducting(beyond) {
                        let local = x - path.levels[hit.coord_level].offset;
                        let nn = geometry.surfaces[s].normal(local);
                        return (n[0] * nn.u + n[1] * nn.v + n[2] * nn.w).abs().min(1.0);
                    }
                }
                r = next;
            }
            missing += 1;
            1.0
        })
        .collect();
    (fractions, missing)
}

/// A solved beginner thermal profile (see the module docs).
#[derive(Debug, Clone)]
pub struct ThermalProfile {
    /// What this result is: always an educational ballpark estimate.
    pub kind: ProfileKind,
    /// The heat-removal preset and the values it used.
    pub heat_removal: HeatRemovalUsed,
    /// The stated power.
    pub power: Power,
    /// Volume of the heated voxels (differs from the CSG volume by the
    /// staircase; the power is exact).
    pub heated_volume: Volume,
    /// The uniform `q'''` in the heated voxels.
    pub heat_source: VolumetricPowerDensity,
    /// Hottest voxel-centre temperature.
    pub peak_temperature: ThermodynamicTemperature,
    /// Volume-mean temperature of the heated regions.
    pub mean_heated_temperature: ThermodynamicTemperature,
    /// Volume-mean temperature of all conducting regions.
    pub mean_temperature: ThermodynamicTemperature,
    /// Mean outer-surface temperature, weighted by true (film) area.
    pub mean_surface_temperature: ThermodynamicTemperature,
    /// Power generated against power removed.
    pub energy_balance: EnergyBalance,
    /// Surface faces whose film weight could not be taken from a CSG
    /// surface normal and was left at 1 (always 0 for a fixed temperature).
    pub faces_without_csg_normal: usize,
    /// Materials whose conductivity was evaluated outside the correlation's
    /// stated range somewhere in the body.
    pub conductivity_outside_range: Vec<ThermalMaterial>,
    /// Voxel edge length.
    pub voxel_size: Length,
    mesh: VoxelMesh,
    solution: SteadyConductionSolution,
}

impl ThermalProfile {
    /// The temperature at `r` (cm): the value of the voxel containing it, or
    /// `None` outside the conducting body.
    #[must_use]
    pub fn temperature_at(&self, r: Position) -> Option<ThermodynamicTemperature> {
        let p = [r.x, r.y, r.z].map(Length::new::<centimeter>);
        self.mesh.cell_at(p).map(|c| self.solution.temperature(c))
    }

    /// Number of voxels in the conducting body.
    #[must_use]
    pub fn n_voxels(&self) -> usize {
        self.solution.n_cells()
    }
}
