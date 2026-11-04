// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NEW WORK (gh:#1149), not a port. Assembled from outram-foam-basic-lib's
// `fvm::laplacian` (OpenFOAM `laplacianFoam` TEqn) and its Robin (`mixed`)
// boundary arm; see the module docs of `super`. This file is part of OUTRAM
// PARK, distributed under GPL-3.0 like the rest of this crate's GeN-Foam
// module.

//! The solid conduction field: assembly, Picard iteration, steady and
//! transient solves (see [`super`] for the equations).

use petir::mathf::RealMath;
use std::sync::Arc;

use uom::si::energy::joule;
use uom::si::f64::{
    Energy, HeatTransfer, Length, Power, Pressure, Ratio, TemperatureInterval, ThermodynamicTemperature, Time,
    VolumetricPowerDensity,
};
use uom::si::length::meter;
use uom::si::pressure::pascal;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::temperature_interval::kelvin as delta_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volumetric_power_density::watt_per_cubic_meter;

use outram_foam_basic_lib::fields::{
    BoundaryCondition, Field, PatchField, SurfaceScalarField, VolScalarField,
};
use outram_foam_basic_lib::fv_operators::fvm;
use outram_foam_basic_lib::fv_operators::fvm::DeltaCoeff;
use outram_foam_basic_lib::ldu_matrix::SolverSettings;
use outram_foam_basic_lib::mesh::FvMesh;

use super::gap::RegionGap;
use super::VolumetricHeatTransferCoefficient;
use super::properties::{ConductivityModel, HeatCapacityModel};
use super::rz_mesh::{RzMesh, RzPatch};
use super::SolidConductionError;
use crate::genfoam::thermal_hydraulics::boundary_conditions::blackbody_radiation::STEFAN_BOLTZMANN_SIGMA;

/// The thermal condition on one physical boundary of an [`RzMesh`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SolidBoundary {
    /// No heat crosses (symmetry, insulation, or the axis).
    Adiabatic,
    /// Surface held at a temperature.
    FixedTemperature(ThermodynamicTemperature),
    /// `q'' = h (T_s - T_ambient)` to a fluid.
    Convective {
        /// Film coefficient.
        coefficient: HeatTransfer,
        /// Fluid temperature.
        ambient: ThermodynamicTemperature,
    },
    /// `q'' = eps sigma (T_s^4 - T_ambient^4)` to large black surroundings
    /// (e.g. a cavity wall at `T_ambient`).
    Radiative {
        /// Surface emissivity, `(0, 1]`.
        emissivity: Ratio,
        /// Temperature of the surroundings.
        ambient: ThermodynamicTemperature,
    },
    /// Radiation as [`Self::Radiative`] **plus** natural convection of the
    /// gas in the enclosure between the surface and a cold wall at
    /// `ambient`, in parallel:
    /// `q'' = eps sigma (T_s^4 - T_a^4) + h_nc(T_s, T_a) (T_s - T_a)`, with
    /// `h_nc` from [`EnclosureNaturalConvection`] at the surface
    /// temperature of the previous Picard pass (gh:#1173).
    RadiativeAndConvective {
        /// Effective emissivity of the radiative exchange, `(0, 1]`.
        emissivity: Ratio,
        /// Temperature of the cold wall (and of the radiation sink).
        ambient: ThermodynamicTemperature,
        /// The enclosure whose natural convection runs beside the radiation.
        convection: EnclosureNaturalConvection,
    },
}

/// Natural convection of **air** across a tall vertical enclosure (a
/// cavity between a heated wall and a cooled wall), per unit area of the
/// heated wall (gh:#1173).
///
/// **Correlation.** MacGregor & Emery (1969), "Free convection through
/// vertical plane layers: moderate and high Prandtl number fluids", J. Heat
/// Transfer 91, 391-403, the high-Rayleigh form
///
/// ```text
///   Nu_L = h L / k = 0.046 Ra_L^(1/3),   Ra_L = g beta (T_s - T_a) L^3 / (nu alpha)
/// ```
///
/// (as Incropera & DeWitt, *Fundamentals of Heat and Mass Transfer*, 6th
/// ed., eq. 9.53, with its stated range `1 < H/L < 40`, `1 < Pr < 20`,
/// `1e6 < Ra_L < 1e9`), floored at `Nu_L = 1` (conduction across the gap).
/// `Ra^(1/3)` makes `h` independent of `L`: the boundary-layer regime, two
/// wall layers in series through a mixed core. **Out of range, stated:**
/// air's `Pr ~ 0.7` is below the stated 1, and see
/// [`Self::rayleigh_number`] for `Ra_L`.
///
/// **Air properties** at the film temperature `(T_s + T_a)/2` and
/// [`Self::pressure`] ([`air_properties`]): Sutherland viscosity and
/// conductivity, ideal-gas density, `c_p` from Incropera Tab A.4; `beta =
/// 1/T_film` (ideal gas). Checked against CoolProp's `Air` in the tests.
///
/// **Curvature.** For coaxial cylinders, hot wall inside at `r_h`, cold
/// wall at `r_c`, the two wall layers have areas in the ratio `r_h/r_c`.
/// The plane-layer `h` is the two equal layers in series (`h_layer = 2 h`),
/// so per unit hot-wall area `h_h = 2 h / (1 + r_h/r_c)`
/// ([`Self::radius_ratio`] = 1 is the plane layer). Derived, an assumption.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnclosureNaturalConvection {
    /// Wall-to-wall gap `L`.
    pub gap: Length,
    /// Enclosure height `H` (checked, and reported against the
    /// correlation's range; `h` does not depend on it).
    pub height: Length,
    /// `r_hot / r_cold` (1 for a plane layer).
    pub radius_ratio: Ratio,
    /// Air pressure.
    pub pressure: Pressure,
}

/// Standard gravity, m/s^2.
const STANDARD_GRAVITY: f64 = 9.806_65;

/// Air's specific heat, J/(kg K), at 1 atm: Incropera & DeWitt,
/// *Fundamentals of Heat and Mass Transfer*, 6th ed., Tab A.4, 300-600 K
/// (linear between the printed points, held at the ends).
const AIR_CP_TAB_A4: [(f64, f64); 7] = [
    (300.0, 1007.0),
    (350.0, 1009.0),
    (400.0, 1014.0),
    (450.0, 1021.0),
    (500.0, 1030.0),
    (550.0, 1040.0),
    (600.0, 1051.0),
];

/// Air properties at one temperature (K) and pressure (Pa): `(nu, alpha,
/// k)` in SI.
///
/// - viscosity, Sutherland's law with the standard constants,
///   `mu = 1.458e-6 T^1.5 / (T + 110.4)` Pa s;
/// - conductivity, Sutherland's law with White's constants for air
///   (`k0 = 0.0241` W/(m K) at 273 K, `S = 194` K; F. M. White, *Viscous
///   Fluid Flow*, 3rd ed., Tab 1-3), the form
///   `sembawang::bwr_mark1::containment::air_conductivity` uses (ported here,
///   not imported: `sembawang` is downstream of this crate);
/// - density, ideal gas, `R = 287.05` J/(kg K);
/// - `c_p`, [`AIR_CP_TAB_A4`].
///
/// Pure Rust and dependency-free on purpose (no new library edge for one
/// boundary); verified against CoolProp's `Air` (Lemmon et al. 2000 EOS,
/// Lemmon & Jacobsen 2004 transport) at 1 atm, 300-600 K, in the tests.
///
/// # Errors
/// [`SolidConductionError::InvalidInput`] for a non-positive or
/// non-finite temperature or pressure.
pub fn air_properties(t_k: f64, p_pa: f64) -> Result<(f64, f64, f64), SolidConductionError> {
    if !(t_k > 0.0 && t_k.is_finite() && p_pa > 0.0 && p_pa.is_finite()) {
        return Err(SolidConductionError::InvalidInput(format!("air at {t_k} K, {p_pa} Pa")));
    }
    let mu = 1.458e-6 * t_k * t_k.sqrt() / (t_k + 110.4);
    let r = t_k / 273.0;
    let k = 0.0241 * r * r.sqrt() * (273.0 + 194.0) / (t_k + 194.0);
    let rho = p_pa / (287.05 * t_k);
    let tab = &AIR_CP_TAB_A4;
    let cp = if t_k <= tab[0].0 {
        tab[0].1
    } else if t_k >= tab[tab.len() - 1].0 {
        tab[tab.len() - 1].1
    } else {
        let i = tab.iter().position(|(t, _)| *t > t_k).unwrap_or(tab.len() - 1);
        let ((t0, c0), (t1, c1)) = (tab[i - 1], tab[i]);
        c0 + (c1 - c0) * (t_k - t0) / (t1 - t0)
    };
    Ok((mu / rho, k / (rho * cp), k))
}

impl EnclosureNaturalConvection {
    /// The correlation's Rayleigh number `Ra_L` for a hot surface at
    /// `t_surface_k` and a cold wall at `t_cold_k`, K.
    ///
    /// # Errors
    /// [`SolidConductionError::InvalidInput`] if the air properties fail.
    pub fn rayleigh_number(&self, t_surface_k: f64, t_cold_k: f64) -> Result<f64, SolidConductionError> {
        let tf = 0.5 * (t_surface_k + t_cold_k);
        let (nu, alpha, _) = air_properties(tf, self.pressure.get::<pascal>())?;
        let l = self.gap.get::<meter>();
        Ok(STANDARD_GRAVITY * (t_surface_k - t_cold_k).abs() / tf * l * l * l / (nu * alpha))
    }

    /// The film coefficient per unit hot-wall area, W/(m^2 K), for a hot
    /// surface at `t_surface_k` and a cold wall at `t_cold_k`, K.
    ///
    /// # Errors
    /// [`SolidConductionError::InvalidInput`] if the air properties fail.
    pub fn coefficient_si(&self, t_surface_k: f64, t_cold_k: f64) -> Result<f64, SolidConductionError> {
        let tf = 0.5 * (t_surface_k + t_cold_k);
        let (_, _, k) = air_properties(tf, self.pressure.get::<pascal>())?;
        let ra = self.rayleigh_number(t_surface_k, t_cold_k)?;
        let nu_l = (0.046 * ra.r_cbrt()).max(1.0);
        Ok(nu_l * k / self.gap.get::<meter>() * 2.0 / (1.0 + self.radius_ratio.get::<ratio>()))
    }

    /// [`Self::coefficient_si`] as a `uom` quantity.
    ///
    /// # Errors
    /// As [`Self::coefficient_si`].
    pub fn coefficient(
        &self,
        surface: ThermodynamicTemperature,
        cold: ThermodynamicTemperature,
    ) -> Result<HeatTransfer, SolidConductionError> {
        Ok(HeatTransfer::new::<watt_per_square_meter_kelvin>(
            self.coefficient_si(surface.get::<kelvin>(), cold.get::<kelvin>())?,
        ))
    }

    fn check(&self) -> Result<(), String> {
        let (l, h, r, p) = (
            self.gap.get::<meter>(),
            self.height.get::<meter>(),
            self.radius_ratio.get::<ratio>(),
            self.pressure.get::<pascal>(),
        );
        let ok = l > 0.0 && l.is_finite() && h > 0.0 && h.is_finite() && r > 0.0 && r <= 1.0 && p > 0.0 && p.is_finite();
        if !ok {
            return Err(format!(
                "enclosure: gap {l} m, height {h} m and pressure {p} Pa must be positive, radius ratio {r} in (0, 1]"
            ));
        }
        Ok(())
    }
}

/// One [`SolidBoundary`] per physical patch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RzBoundaries {
    /// `r = r_min`.
    pub inner: SolidBoundary,
    /// `r = r_max`.
    pub outer: SolidBoundary,
    /// `z = z_min`.
    pub bottom: SolidBoundary,
    /// `z = z_max`.
    pub top: SolidBoundary,
}

impl RzBoundaries {
    /// The same condition on all four.
    #[must_use]
    pub fn all(b: SolidBoundary) -> Self {
        Self {
            inner: b,
            outer: b,
            bottom: b,
            top: b,
        }
    }

    /// The condition on `patch`.
    #[must_use]
    pub fn get(&self, patch: RzPatch) -> SolidBoundary {
        match patch {
            RzPatch::Inner => self.inner,
            RzPatch::Outer => self.outer,
            RzPatch::Bottom => self.bottom,
            RzPatch::Top => self.top,
        }
    }
}

/// A homogenised material region.
#[derive(Debug, Clone)]
pub struct SolidRegion {
    /// Label (drawings, reports).
    pub name: String,
    /// `lambda(T)`.
    pub conductivity: ConductivityModel,
    /// `rho c_p (T)` (only the transient solve reads it).
    pub heat_capacity: HeatCapacityModel,
    /// Uniform volumetric heat source of the region.
    pub heat_source: VolumetricPowerDensity,
}

/// Iteration controls.
#[derive(Debug, Clone, Copy)]
pub struct SolidConductionSettings {
    /// Stop the Picard passes when no cell moves more than this.
    pub picard_tolerance: TemperatureInterval,
    /// Give up after this many passes.
    pub max_picard_iterations: usize,
    /// Conjugate-gradient settings of each linear solve.
    pub linear: SolverSettings,
}

impl Default for SolidConductionSettings {
    fn default() -> Self {
        Self {
            picard_tolerance: TemperatureInterval::new::<delta_kelvin>(1e-8),
            max_picard_iterations: 500,
            linear: SolverSettings {
                tolerance: 1e-14,
                max_iter: 200_000,
            },
        }
    }
}

/// What one steady solve or time step took.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PicardReport {
    /// Passes performed (a constant-property, radiation-free problem takes 2).
    pub iterations: usize,
    /// Largest cell change of the last pass.
    pub last_change: TemperatureInterval,
    /// CG iterations of the last linear solve.
    pub linear_iterations: usize,
}

/// Heat generated against heat leaving through the four physical patches.
#[derive(Debug, Clone)]
pub struct SolidEnergyBalance {
    /// `sum q''' V` over the cells whose temperature is solved for (cells
    /// held by [`SolidConductionField::set_fixed_temperatures`] excluded).
    pub generated: Power,
    /// Net heat the held cells deliver to the rest of the body and out of
    /// their own boundary faces (zero when no cell is held). This is the
    /// "prescribed temperature" side's duty: a held region stands in for a
    /// gas or a structure whose temperature is imposed, and this is what it
    /// gives up at that temperature.
    pub supplied_by_fixed_cells: Power,
    /// Leaving through each patch, in [`RzPatch::ALL`] order, each face's flux
    /// recomputed from its own condition and the surface temperature.
    pub removed_per_patch: [Power; 4],
    /// Given up to a coexisting fluid through the per-cell exchange of
    /// [`SolidConductionField::set_cell_exchange`] (gh:#1150), `sum h_v V
    /// (T - T_fluid)` over the free cells; zero without an exchange. Negative
    /// when the fluid heats the solid.
    pub removed_to_fluid: Power,
}

impl SolidEnergyBalance {
    /// Total leaving.
    #[must_use]
    pub fn removed(&self) -> Power {
        self.removed_per_patch
            .iter()
            .fold(self.removed_to_fluid, |a, b| a + *b)
    }

    /// `|generated + supplied - removed| / (generated + supplied)` (the
    /// absolute imbalance in W when nothing enters).
    #[must_use]
    pub fn relative_imbalance(&self) -> f64 {
        let g = self.generated.get::<watt>() + self.supplied_by_fixed_cells.get::<watt>();
        let d = (g - self.removed().get::<watt>()).abs();
        if g.abs() > 0.0 {
            d / g.abs()
        } else {
            d
        }
    }
}

/// The R-Z solid conduction field (THERMIX-like), new work for gh:#1149.
///
/// Construct with [`Self::new`], then call [`Self::solve_steady`] or march
/// [`Self::step`]. Temperatures live per cell; read them with
/// [`Self::temperature`] or [`Self::temperature_at`].
#[derive(Debug, Clone)]
pub struct SolidConductionField {
    mesh: RzMesh,
    regions: Vec<SolidRegion>,
    boundaries: RzBoundaries,
    gaps: Vec<RegionGap>,
    /// Per internal face: `(gap index, owner cell is on region_a)`.
    face_gap: Vec<Option<(usize, bool)>>,
    /// W/m^3 per cell.
    cell_source: Vec<f64>,
    /// K per cell.
    temperature: Vec<f64>,
    /// Per cell: `Some(T)` when the cell is held at `T` K (a Dirichlet
    /// condition on the cell, eliminated from the system as OpenFOAM's
    /// `fvMatrix::setValues` does), `None` when it is solved for.
    fixed: Vec<Option<f64>>,
    /// Surface temperature of each boundary face (global face minus
    /// `n_internal_faces`), K: the previous iterate's, used to linearise
    /// radiation.
    boundary_surface: Vec<f64>,
    /// Surface temperatures (owner side, neighbour side) of each internal
    /// face, K; read only on gap faces.
    gap_surface: Vec<(f64, f64)>,
    /// Per cell: volumetric exchange coefficient to a coexisting fluid,
    /// W/(m^3 K), and the fluid temperature, K (gh:#1150). Zero by default.
    exchange: Vec<(f64, f64)>,
    /// Per cell: multiplier on the region's volumetric heat capacity
    /// (gh:#1151 part B). 1 by default.
    capacity_factor: Vec<f64>,
    settings: SolidConductionSettings,
    time: f64,
}

impl SolidConductionField {
    /// Set up a field at a uniform initial temperature.
    ///
    /// `regions[i]` describes region `i` of the mesh. Each [`RegionGap`]
    /// applies to every face between its two regions.
    ///
    /// # Errors
    /// [`SolidConductionError::InvalidInput`] for a region count that does
    /// not match the mesh, a negative or non-finite heat source, a gap
    /// naming an unknown region or the same region twice, a radiative
    /// emissivity outside `(0, 1]` or a non-positive convective `h`.
    pub fn new(
        mesh: RzMesh,
        regions: Vec<SolidRegion>,
        boundaries: RzBoundaries,
        gaps: Vec<RegionGap>,
        initial: ThermodynamicTemperature,
    ) -> Result<Self, SolidConductionError> {
        if regions.len() != mesh.n_regions() {
            return Err(SolidConductionError::InvalidInput(format!(
                "{} regions for a mesh with {}",
                regions.len(),
                mesh.n_regions()
            )));
        }
        for r in &regions {
            let q = r.heat_source.get::<watt_per_cubic_meter>();
            if !q.is_finite() {
                return Err(SolidConductionError::InvalidInput(format!(
                    "region {}: heat source must be finite",
                    r.name
                )));
            }
        }
        for p in RzPatch::ALL {
            match boundaries.get(p) {
                SolidBoundary::Convective { coefficient, .. } => {
                    let h = coefficient.get::<watt_per_square_meter_kelvin>();
                    if !(h > 0.0) || !h.is_finite() {
                        return Err(SolidConductionError::InvalidInput(format!(
                            "{}: h must be positive, got {h} W/(m^2 K)",
                            p.name()
                        )));
                    }
                }
                SolidBoundary::Radiative { emissivity, .. } => {
                    let e = emissivity.get::<ratio>();
                    if !(e > 0.0 && e <= 1.0) {
                        return Err(SolidConductionError::InvalidInput(format!(
                            "{}: emissivity must be in (0, 1], got {e}",
                            p.name()
                        )));
                    }
                }
                SolidBoundary::RadiativeAndConvective { emissivity, ambient, convection } => {
                    let e = emissivity.get::<ratio>();
                    if !(e > 0.0 && e <= 1.0) {
                        return Err(SolidConductionError::InvalidInput(format!(
                            "{}: emissivity must be in (0, 1], got {e}",
                            p.name()
                        )));
                    }
                    convection
                        .check()
                        .map_err(|m| SolidConductionError::InvalidInput(format!("{}: {m}", p.name())))?;
                    // The air properties must evaluate where the boundary works.
                    convection.coefficient_si(ambient.get::<kelvin>() + 50.0, ambient.get::<kelvin>())?;
                }
                _ => {}
            }
        }
        for g in &gaps {
            if g.region_a == g.region_b
                || g.region_a >= regions.len()
                || g.region_b >= regions.len()
            {
                return Err(SolidConductionError::InvalidInput(format!(
                    "gap between regions {} and {} is not between two distinct existing regions",
                    g.region_a, g.region_b
                )));
            }
        }
        let fv = mesh.mesh().clone();
        let face_gap = (0..fv.n_internal_faces)
            .map(|f| {
                let ro = mesh.cell_region(fv.owner[f]);
                let rn = mesh.cell_region(fv.neighbour[f]);
                gaps.iter().enumerate().find_map(|(gi, g)| {
                    if g.region_a == ro && g.region_b == rn {
                        Some((gi, true))
                    } else if g.region_a == rn && g.region_b == ro {
                        Some((gi, false))
                    } else {
                        None
                    }
                })
            })
            .collect();
        let t0 = initial.get::<kelvin>();
        let n = mesh.n_cells();
        let cell_source = (0..n)
            .map(|c| regions[mesh.cell_region(c)].heat_source.get::<watt_per_cubic_meter>())
            .collect();
        Ok(Self {
            boundary_surface: vec![t0; fv.n_faces - fv.n_internal_faces],
            gap_surface: vec![(t0, t0); fv.n_internal_faces],
            temperature: vec![t0; n],
            fixed: vec![None; n],
            exchange: vec![(0.0, t0); n],
            capacity_factor: vec![1.0; n],
            cell_source,
            mesh,
            regions,
            boundaries,
            gaps,
            face_gap,
            settings: SolidConductionSettings::default(),
            time: 0.0,
        })
    }

    /// Replace the iteration controls.
    #[must_use]
    pub fn with_settings(mut self, settings: SolidConductionSettings) -> Self {
        self.settings = settings;
        self
    }

    /// Replace the per-region sources by a per-cell source (e.g. a power
    /// density mapped from neutronics, or a manufactured source).
    ///
    /// # Errors
    /// [`SolidConductionError::InvalidInput`] for the wrong length or a
    /// non-finite value.
    pub fn set_cell_heat_source(
        &mut self,
        source: &[VolumetricPowerDensity],
    ) -> Result<(), SolidConductionError> {
        if source.len() != self.mesh.n_cells() {
            return Err(SolidConductionError::InvalidInput(format!(
                "{} sources for {} cells",
                source.len(),
                self.mesh.n_cells()
            )));
        }
        let v: Vec<f64> = source.iter().map(|q| q.get::<watt_per_cubic_meter>()).collect();
        if v.iter().any(|q| !q.is_finite()) {
            return Err(SolidConductionError::InvalidInput("non-finite heat source".into()));
        }
        self.cell_source = v;
        Ok(())
    }

    /// Couple every cell to a coexisting fluid (the gas of a porous region):
    /// a sink `h_v (T - T_fluid)` per unit volume, treated **implicitly** in
    /// the cell temperature (`h_v V` on the diagonal, `h_v V T_fluid` in the
    /// source), so a large `h_v` cannot destabilise the solve. Pass `h_v = 0`
    /// for a cell without fluid. Replaces any earlier exchange.
    ///
    /// **New work (gh:#1150), not a port.** It is the solid half of the
    /// fluid-structure heat source GeN-Foam linearises semi-implicitly
    /// (`structure::linearizedSemiImplicitHeatSource`), with the fluid
    /// temperature held for this solve; the HTR-10 gas-solid outer iteration
    /// ([`crate::htr10_rz::flow`]) alternates it with the gas energy solve.
    /// The heat given to the fluid is
    /// [`SolidEnergyBalance::removed_to_fluid`].
    ///
    /// # Errors
    /// [`SolidConductionError::InvalidInput`] for the wrong length, a
    /// negative or non-finite coefficient, or a non-positive fluid
    /// temperature where the coefficient is positive.
    pub fn set_cell_exchange(
        &mut self,
        exchange: &[(VolumetricHeatTransferCoefficient, ThermodynamicTemperature)],
    ) -> Result<(), SolidConductionError> {
        if exchange.len() != self.mesh.n_cells() {
            return Err(SolidConductionError::InvalidInput(format!(
                "{} exchange entries for {} cells",
                exchange.len(),
                self.mesh.n_cells()
            )));
        }
        let mut v = Vec::with_capacity(exchange.len());
        for (h, t) in exchange {
            let (h, t) = (h.value, t.get::<kelvin>());
            if !(h >= 0.0) || !h.is_finite() || (h > 0.0 && (!(t > 0.0) || !t.is_finite())) {
                return Err(SolidConductionError::InvalidInput(format!(
                    "exchange h_v = {h} W/(m^3 K) to {t} K: h_v must be finite and non-negative, T positive"
                )));
            }
            v.push((h, t));
        }
        self.exchange = v;
        Ok(())
    }

    /// Scale the region heat capacity of each cell by `factor[c]` (default
    /// 1 everywhere): the storage term becomes `f_c (rho c)(T) V dT/dt`.
    ///
    /// **New work (gh:#1151 part B), not a port.** It lets a coupled model
    /// carry part of a homogenised region's heat capacity in its own node
    /// (the HTR-10 fuel zones of the pebbles in a bed cell) without the
    /// field counting it twice. It changes only transient storage and
    /// [`Self::thermal_energy`]; a steady solve is unaffected.
    ///
    /// # Errors
    /// [`SolidConductionError::InvalidInput`] for the wrong length or a
    /// factor that is not finite and positive.
    pub fn set_cell_capacity_factor(&mut self, factor: &[f64]) -> Result<(), SolidConductionError> {
        if factor.len() != self.mesh.n_cells() {
            return Err(SolidConductionError::InvalidInput(format!(
                "{} capacity factors for {} cells",
                factor.len(),
                self.mesh.n_cells()
            )));
        }
        if let Some(f) = factor.iter().find(|f| !(**f > 0.0) || !f.is_finite()) {
            return Err(SolidConductionError::InvalidInput(format!(
                "capacity factor must be finite and positive, got {f}"
            )));
        }
        self.capacity_factor = factor.to_vec();
        Ok(())
    }

    /// Set every cell's temperature (and reset the surface estimates to the
    /// cell values).
    ///
    /// # Errors
    /// [`SolidConductionError::InvalidInput`] for the wrong length.
    pub fn set_temperatures(
        &mut self,
        t: &[ThermodynamicTemperature],
    ) -> Result<(), SolidConductionError> {
        if t.len() != self.mesh.n_cells() {
            return Err(SolidConductionError::InvalidInput(format!(
                "{} temperatures for {} cells",
                t.len(),
                self.mesh.n_cells()
            )));
        }
        for (dst, src) in self.temperature.iter_mut().zip(t) {
            *dst = src.get::<kelvin>();
        }
        let fv = self.mesh.mesh().clone();
        for (k, s) in self.boundary_surface.iter_mut().enumerate() {
            *s = self.temperature[fv.owner[fv.n_internal_faces + k]];
        }
        for f in 0..fv.n_internal_faces {
            self.gap_surface[f] = (self.temperature[fv.owner[f]], self.temperature[fv.neighbour[f]]);
        }
        Ok(())
    }

    /// Hold cells at prescribed temperatures (a Dirichlet condition on the
    /// cell, not on a face): the gas side, or any body whose temperature is
    /// imposed rather than solved for. Replaces any earlier set; an empty
    /// slice releases every cell.
    ///
    /// The held rows are eliminated from the linear system the way
    /// OpenFOAM's `fvMatrix::setValues` does it: the held cell's row becomes
    /// `a_P T_P = a_P T_fixed`, and each coupling to a free neighbour is
    /// moved to that neighbour's source and zeroed, so the matrix stays
    /// symmetric for the conjugate-gradient solve. A held cell's own heat
    /// source is not counted in [`SolidEnergyBalance::generated`]; the heat
    /// it gives up is [`SolidEnergyBalance::supplied_by_fixed_cells`].
    ///
    /// # Errors
    /// [`SolidConductionError::InvalidInput`] for a cell index out of range
    /// or a non-positive or non-finite temperature.
    pub fn set_fixed_temperatures(
        &mut self,
        cells: &[(usize, ThermodynamicTemperature)],
    ) -> Result<(), SolidConductionError> {
        let n = self.mesh.n_cells();
        let mut fixed = vec![None; n];
        for (c, t) in cells {
            let tk = t.get::<kelvin>();
            if *c >= n || !(tk > 0.0) || !tk.is_finite() {
                return Err(SolidConductionError::InvalidInput(format!(
                    "held cell {c} at {tk} K: the cell must exist and the temperature be positive"
                )));
            }
            fixed[*c] = Some(tk);
        }
        for (c, f) in fixed.iter().enumerate() {
            if let Some(tk) = f {
                self.temperature[c] = *tk;
            }
        }
        self.fixed = fixed;
        Ok(())
    }

    /// Whether cell `c` is held by [`Self::set_fixed_temperatures`].
    #[must_use]
    pub fn is_fixed(&self, c: usize) -> bool {
        self.fixed[c].is_some()
    }

    /// The mesh.
    #[must_use]
    pub fn mesh(&self) -> &RzMesh {
        &self.mesh
    }

    /// The regions.
    #[must_use]
    pub fn regions(&self) -> &[SolidRegion] {
        &self.regions
    }

    /// Simulated time marched by [`Self::step`].
    #[must_use]
    pub fn time(&self) -> Time {
        Time::new::<second>(self.time)
    }

    /// Temperature of cell `c`.
    #[must_use]
    pub fn temperature(&self, c: usize) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(self.temperature[c])
    }

    /// Temperature of the cell containing `(r, z)`.
    #[must_use]
    pub fn temperature_at(
        &self,
        r: uom::si::f64::Length,
        z: uom::si::f64::Length,
    ) -> Option<ThermodynamicTemperature> {
        self.mesh.cell_at(r, z).map(|c| self.temperature(c))
    }

    /// Internal faces that carry a gap.
    #[must_use]
    pub fn gap_faces(&self) -> Vec<usize> {
        (0..self.face_gap.len()).filter(|f| self.face_gap[*f].is_some()).collect()
    }

    /// Surface temperature of local face `local` of `patch`, as last
    /// iterated (fixed: the imposed value; adiabatic: the cell value).
    #[must_use]
    pub fn surface_temperature(&self, patch: RzPatch, local: usize) -> ThermodynamicTemperature {
        let fv = self.mesh.mesh();
        let gf = fv.patches[patch.index()].start + local;
        ThermodynamicTemperature::new::<kelvin>(self.boundary_surface[gf - fv.n_internal_faces])
    }

    /// Surface temperatures `(owner side, neighbour side)` of internal face
    /// `face`, as last iterated (meaningful on a gap face).
    #[must_use]
    pub fn gap_surface_temperatures(
        &self,
        face: usize,
    ) -> (ThermodynamicTemperature, ThermodynamicTemperature) {
        let (a, b) = self.gap_surface[face];
        (
            ThermodynamicTemperature::new::<kelvin>(a),
            ThermodynamicTemperature::new::<kelvin>(b),
        )
    }

    /// `sum_c V_c int_{T_ref}^{T_c} rho c_p dT`: the thermal energy above
    /// `reference`, exact for every closed-form property model.
    #[must_use]
    pub fn thermal_energy(&self, reference: ThermodynamicTemperature) -> Energy {
        let tr = reference.get::<kelvin>();
        let fv = self.mesh.mesh();
        let e: f64 = (0..self.mesh.n_cells())
            .map(|c| {
                let m = &self.regions[self.mesh.cell_region(c)].heat_capacity.0;
                self.capacity_factor[c] * fv.cell_volumes[c] * m.integral(tr, self.temperature[c])
            })
            .sum();
        Energy::new::<joule>(e)
    }

    /// Heat generated against heat leaving, from the current temperatures.
    #[must_use]
    pub fn energy_balance(&self) -> SolidEnergyBalance {
        let fv = self.mesh.mesh().clone();
        let k = self.conductivities().unwrap_or_else(|_| vec![f64::NAN; self.mesh.n_cells()]);
        let generated: f64 = (0..self.mesh.n_cells())
            .filter(|c| self.fixed[*c].is_none())
            .map(|c| self.cell_source[c] * fv.cell_volumes[c])
            .sum();
        let mut supplied = 0.0;
        for f in 0..fv.n_internal_faces {
            let (o, nb) = (fv.owner[f], fv.neighbour[f]);
            let into_free = match (self.fixed[o].is_some(), self.fixed[nb].is_some()) {
                (true, false) => self.temperature[o] - self.temperature[nb],
                (false, true) => self.temperature[nb] - self.temperature[o],
                _ => continue,
            };
            let (r, ..) = self.face_resistance(&fv, &k, f);
            supplied += fv.face_areas[f] / r * into_free;
        }
        let mut removed = [Power::new::<watt>(0.0); 4];
        for p in RzPatch::ALL {
            let patch = &fv.patches[p.index()];
            let mut sum = 0.0;
            for local in 0..patch.size {
                let gf = patch.start + local;
                let c = fv.owner[gf];
                let area = fv.face_areas[gf];
                let d = DeltaCoeff::Orthogonal.boundary_delta(&fv, gf);
                let ts = self.boundary_surface[gf - fv.n_internal_faces];
                let out = match self.boundaries.get(p) {
                    SolidBoundary::Adiabatic => 0.0,
                    SolidBoundary::FixedTemperature(_) => {
                        if d > 0.0 {
                            k[c] * area / d * (self.temperature[c] - ts)
                        } else {
                            0.0
                        }
                    }
                    SolidBoundary::Convective {
                        coefficient,
                        ambient,
                    } => {
                        let h = coefficient.get::<watt_per_square_meter_kelvin>();
                        area * (self.temperature[c] - ambient.get::<kelvin>()) / (1.0 / h + d / k[c])
                    }
                    SolidBoundary::Radiative {
                        emissivity,
                        ambient,
                    } => {
                        let ta = ambient.get::<kelvin>();
                        area * emissivity.get::<ratio>()
                            * STEFAN_BOLTZMANN_SIGMA
                            * (ts * ts * ts * ts - ta * ta * ta * ta)
                    }
                    SolidBoundary::RadiativeAndConvective {
                        emissivity,
                        ambient,
                        convection,
                    } => {
                        let ta = ambient.get::<kelvin>();
                        // A failed air property shows as NaN in the balance.
                        let hc = convection.coefficient_si(ts, ta).unwrap_or(f64::NAN);
                        area * (emissivity.get::<ratio>()
                            * STEFAN_BOLTZMANN_SIGMA
                            * (ts * ts * ts * ts - ta * ta * ta * ta)
                            + hc * (ts - ta))
                    }
                };
                if self.fixed[c].is_some() {
                    supplied += out;
                }
                sum += out;
            }
            removed[p.index()] = Power::new::<watt>(sum);
        }
        let to_fluid: f64 = (0..self.mesh.n_cells())
            .filter(|c| self.fixed[*c].is_none())
            .map(|c| {
                let (h, tf) = self.exchange[c];
                h * fv.cell_volumes[c] * (self.temperature[c] - tf)
            })
            .sum();
        SolidEnergyBalance {
            generated: Power::new::<watt>(generated),
            supplied_by_fixed_cells: Power::new::<watt>(supplied),
            removed_per_patch: removed,
            removed_to_fluid: Power::new::<watt>(to_fluid),
        }
    }

    /// Solve the steady problem from the current temperatures.
    ///
    /// # Errors
    /// [`SolidConductionError::InvalidInput`] when heat is generated but
    /// every boundary is adiabatic; otherwise as [`Self::step`].
    pub fn solve_steady(&mut self) -> Result<PicardReport, SolidConductionError> {
        let heated = self.cell_source.iter().any(|q| *q != 0.0) || self.fixed.iter().any(Option::is_some);
        let open = RzPatch::ALL
            .iter()
            .any(|p| !matches!(self.boundaries.get(*p), SolidBoundary::Adiabatic))
            || self.exchange.iter().any(|(h, _)| *h > 0.0);
        if heated && !open {
            return Err(SolidConductionError::InvalidInput(
                "heat is generated but every boundary is adiabatic: no steady state exists".into(),
            ));
        }
        self.picard(None)
    }

    /// Advance one backward-Euler step of `dt` (Picard-converged).
    ///
    /// # Errors
    /// [`SolidConductionError::InvalidInput`] for a non-positive `dt`;
    /// [`SolidConductionError::BadProperty`],
    /// [`SolidConductionError::LinearSolveFailed`],
    /// [`SolidConductionError::PicardNotConverged`].
    pub fn step(&mut self, dt: Time) -> Result<PicardReport, SolidConductionError> {
        let dt = dt.get::<second>();
        if !(dt > 0.0) || !dt.is_finite() {
            return Err(SolidConductionError::InvalidInput(format!(
                "time step must be positive, got {dt} s"
            )));
        }
        let report = self.picard(Some(dt))?;
        self.time += dt;
        Ok(report)
    }

    /// `lambda(T_c)` of every cell.
    fn conductivities(&self) -> Result<Vec<f64>, SolidConductionError> {
        (0..self.mesh.n_cells())
            .map(|c| {
                let reg = self.mesh.cell_region(c);
                let v = self.regions[reg].conductivity.0.value(self.temperature[c]);
                if v > 0.0 && v.is_finite() {
                    Ok(v)
                } else {
                    Err(SolidConductionError::BadProperty {
                        region: reg,
                        property: "conductivity",
                        temperature_kelvin: self.temperature[c],
                        value: v,
                    })
                }
            })
            .collect()
    }

    /// Series resistance `d_P/k_P [+ 1/h_gap] + d_N/k_N` of internal face `f`
    /// (per unit area), and the gap `h` used (0 without a gap).
    fn face_resistance(&self, fv: &FvMesh, k: &[f64], f: usize) -> (f64, f64, f64, f64) {
        let o = fv.owner[f];
        let nb = fv.neighbour[f];
        let d_p = (fv.face_centres[f] - fv.cell_centres[o]).mag();
        let d_n = (fv.cell_centres[nb] - fv.face_centres[f]).mag();
        let mut r = d_p / k[o] + d_n / k[nb];
        let mut h = 0.0;
        if let Some((gi, owner_is_a)) = self.face_gap[f] {
            let (ts_o, ts_n) = self.gap_surface[f];
            let g = &self.gaps[gi].gap;
            h = if owner_is_a {
                g.coefficient_si(ts_o, ts_n)
            } else {
                g.swapped().coefficient_si(ts_n, ts_o)
            };
            r += 1.0 / h;
        }
        (r, d_p, d_n, h)
    }

    /// Effective film coefficient and reference temperature of a Robin
    /// boundary face, linearised at surface temperature `ts`.
    ///
    /// # Errors
    /// [`SolidConductionError::InvalidInput`] if an enclosure's air
    /// properties fail.
    fn robin(b: SolidBoundary, ts: f64) -> Result<Option<(f64, f64)>, SolidConductionError> {
        if let SolidBoundary::RadiativeAndConvective { emissivity, ambient, convection } = b {
            // Radiation (tangent form, as below) and convection in parallel;
            // `h_nc` at the previous surface temperature (secant in `h`).
            let (hr, tr) = Self::robin(SolidBoundary::Radiative { emissivity, ambient }, ts)?.unwrap_or((0.0, 0.0));
            let ta = ambient.get::<kelvin>();
            let hc = convection.coefficient_si(ts.max(1.0), ta)?;
            let h = hr + hc;
            return Ok(Some((h, (hr * tr + hc * ta) / h)));
        }
        Ok(match b {
            SolidBoundary::Convective {
                coefficient,
                ambient,
            } => Some((
                coefficient.get::<watt_per_square_meter_kelvin>(),
                ambient.get::<kelvin>(),
            )),
            SolidBoundary::Radiative {
                emissivity,
                ambient,
            } => {
                // Newton (tangent) linearisation at the previous surface
                // temperature: q(T) ~ q(T*) + 4 eps sigma T*^3 (T - T*),
                // written as h (T - T_ref). The secant form
                // eps sigma (T*^2 + Ta^2)(T* + Ta) (T - Ta) was tried first and
                // oscillated without converging (map slope about
                // -3 (Ts - Ta)/Ts, i.e. below -1 once Ts > 1.5 Ta: case (a),
                // 865 K into 300 K, did not settle in 500 passes).
                let ta = ambient.get::<kelvin>();
                let es = emissivity.get::<ratio>() * STEFAN_BOLTZMANN_SIGMA;
                let t = ts.max(1.0);
                let h = 4.0 * es * t * t * t;
                let q = es * (t * t * t * t - ta * ta * ta * ta);
                Some((h, t - q / h))
            }
            _ => None,
        })
    }

    /// The Picard loop of one steady solve (`dt = None`) or time step.
    fn picard(&mut self, dt: Option<f64>) -> Result<PicardReport, SolidConductionError> {
        let fv: Arc<FvMesh> = self.mesh.mesh().clone();
        let n = self.mesh.n_cells();
        let n_int = fv.n_internal_faces;
        let tol = self.settings.picard_tolerance.get::<delta_kelvin>();
        let t_old = self.temperature.clone();
        let mut change = f64::INFINITY;

        for it in 1..=self.settings.max_picard_iterations {
            let k = self.conductivities()?;

            // Face conductivity: series resistance (harmonic) plus any gap.
            let internal: Vec<f64> = (0..n_int)
                .map(|f| {
                    let d = DeltaCoeff::Orthogonal.interior_delta(&fv, f);
                    d / self.face_resistance(&fv, &k, f).0
                })
                .collect();
            let gamma_b = fv
                .patches
                .iter()
                .map(|p| PatchField {
                    bc: BoundaryCondition::ZeroGradient,
                    values: Field::new((0..p.size).map(|i| k[fv.owner[p.start + i]]).collect()),
                })
                .collect();
            let gamma = SurfaceScalarField::new("lambda", fv.clone(), Field::new(internal), gamma_b);

            // Temperature boundary conditions.
            let t_bcs = fv
                .patches
                .iter()
                .enumerate()
                .map(|(pi, patch)| -> Result<PatchField<f64>, SolidConductionError> {
                    let bc = if pi < 4 {
                        let b = self.boundaries.get(RzPatch::ALL[pi]);
                        match b {
                            SolidBoundary::Adiabatic => BoundaryCondition::ZeroGradient,
                            SolidBoundary::FixedTemperature(ts) => {
                                BoundaryCondition::FixedValue(ts.get::<kelvin>())
                            }
                            _ => {
                                let mut w = Vec::with_capacity(patch.size);
                                let mut tr = Vec::with_capacity(patch.size);
                                for local in 0..patch.size {
                                    let gf = patch.start + local;
                                    let d = DeltaCoeff::Orthogonal.boundary_delta(&fv, gf);
                                    let ts = self.boundary_surface[gf - n_int];
                                    let (h, ta) = Self::robin(b, ts)?.unwrap_or((0.0, 0.0));
                                    // externalWallHeatFluxTemperature: hp/(hp + k deltaCoeffs).
                                    w.push(h * d / (h * d + k[fv.owner[gf]]));
                                    tr.push(ta);
                                }
                                BoundaryCondition::MixedField {
                                    value_fraction: Field::new(w),
                                    ref_value: Field::new(tr),
                                    ref_grad: Field::zeros(patch.size),
                                }
                            }
                        }
                    } else {
                        // Wedge front/back: zero normal gradient, exact for a scalar.
                        BoundaryCondition::ZeroGradient
                    };
                    Ok(PatchField {
                        bc,
                        values: Field::zeros(patch.size),
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let t_field =
                VolScalarField::new("T", fv.clone(), Field::new(self.temperature.clone()), t_bcs);

            let mut eqn = fvm::laplacian(&gamma, &t_field);
            for c in 0..n {
                eqn.source[c] += self.cell_source[c] * fv.cell_volumes[c];
                let (h, tf) = self.exchange[c];
                if h > 0.0 {
                    eqn.ldu.diag[c] += h * fv.cell_volumes[c];
                    eqn.source[c] += h * fv.cell_volumes[c] * tf;
                }
            }
            if let Some(dt) = dt {
                for c in 0..n {
                    let m = &self.regions[self.mesh.cell_region(c)].heat_capacity.0;
                    let tk = self.temperature[c];
                    let cap = m.value(tk);
                    if !(cap > 0.0) || !cap.is_finite() {
                        return Err(SolidConductionError::BadProperty {
                            region: self.mesh.cell_region(c),
                            property: "volumetric heat capacity",
                            temperature_kelvin: tk,
                            value: cap,
                        });
                    }
                    let v_dt = self.capacity_factor[c] * fv.cell_volumes[c] / dt;
                    eqn.ldu.diag[c] += cap * v_dt;
                    // Enthalpy form: H(T^{k+1}) ~ H(T^k) + C(T^k)(T^{k+1} - T^k),
                    // so storage is conservative once Picard converges. A
                    // constant C reduces it to C T^n exactly (no round-off).
                    eqn.source[c] += if m.is_constant() {
                        cap * v_dt * t_old[c]
                    } else {
                        v_dt * (cap * tk - m.integral(t_old[c], tk))
                    };
                }
            }
            // Held cells: eliminate their rows (OpenFOAM fvMatrix::setValues).
            if self.fixed.iter().any(Option::is_some) {
                for f in 0..n_int {
                    let (o, nb) = (fv.owner[f], fv.neighbour[f]);
                    match (self.fixed[o], self.fixed[nb]) {
                        (None, None) => continue,
                        (Some(vo), None) => eqn.source[nb] -= eqn.ldu.lower[f] * vo,
                        (None, Some(vn)) => eqn.source[o] -= eqn.ldu.upper[f] * vn,
                        (Some(_), Some(_)) => {}
                    }
                    eqn.ldu.upper[f] = 0.0;
                    eqn.ldu.lower[f] = 0.0;
                }
                // A held row is decoupled: `a T = a T_fixed` for any `a > 0`.
                // Use the mean free-cell diagonal, so a held body of very high
                // conductivity does not dominate the solver's residual
                // normalisation and loosen the free cells' convergence.
                let (sum, count) = self
                    .fixed
                    .iter()
                    .zip(eqn.ldu.diag.iter())
                    .filter(|(f, _)| f.is_none())
                    .fold((0.0, 0usize), |(s, n), (_, d)| (s + d.abs(), n + 1));
                let scale = if count > 0 && sum > 0.0 { sum / count as f64 } else { 1.0 };
                for (c, f) in self.fixed.iter().enumerate() {
                    if let Some(v) = f {
                        eqn.ldu.diag[c] = scale;
                        eqn.source[c] = scale * v;
                    }
                }
            }
            let (solved, perf) = eqn.solve_cg_with_guess("T", &t_field, self.settings.linear);
            if !perf.converged {
                return Err(SolidConductionError::LinearSolveFailed {
                    residual: perf.final_residual,
                    iterations: perf.n_iterations,
                });
            }
            let linear_iterations = perf.n_iterations;
            let new = solved.internal.as_slice();
            change = new
                .iter()
                .zip(self.temperature.iter())
                .map(|(a, b)| (a - b).abs())
                .fold(0.0_f64, f64::max);

            // Surface temperatures from the fluxes of this pass (they
            // linearise the next pass's radiation and gaps).
            let mut gap_update = Vec::new();
            for f in 0..n_int {
                if self.face_gap[f].is_some() {
                    let (r, d_p, d_n, _) = self.face_resistance(&fv, &k, f);
                    let (o, nb) = (fv.owner[f], fv.neighbour[f]);
                    let q = (new[o] - new[nb]) / r;
                    gap_update.push((f, (new[o] - q * d_p / k[o], new[nb] + q * d_n / k[nb])));
                }
            }
            for (f, s) in gap_update {
                self.gap_surface[f] = s;
            }
            for (pi, p) in RzPatch::ALL.iter().enumerate() {
                let patch = &fv.patches[pi];
                let b = self.boundaries.get(*p);
                for local in 0..patch.size {
                    let gf = patch.start + local;
                    let c = fv.owner[gf];
                    let d = DeltaCoeff::Orthogonal.boundary_delta(&fv, gf);
                    let slot = gf - n_int;
                    self.boundary_surface[slot] = match b {
                        SolidBoundary::Adiabatic => new[c],
                        SolidBoundary::FixedTemperature(ts) => ts.get::<kelvin>(),
                        _ => {
                            let (h, ta) = Self::robin(b, self.boundary_surface[slot])?
                                .unwrap_or((0.0, 0.0));
                            let q = (new[c] - ta) / (1.0 / h + d / k[c]);
                            new[c] - q * d / k[c]
                        }
                    };
                }
            }
            for p in 4..fv.patches.len() {
                let patch = &fv.patches[p];
                for local in 0..patch.size {
                    let gf = patch.start + local;
                    self.boundary_surface[gf - n_int] = new[fv.owner[gf]];
                }
            }
            self.temperature.copy_from_slice(new);
            if change < tol {
                return Ok(PicardReport {
                    iterations: it,
                    last_change: TemperatureInterval::new::<delta_kelvin>(change),
                    linear_iterations,
                });
            }
        }
        Err(SolidConductionError::PicardNotConverged {
            last_change_kelvin: change,
            iterations: self.settings.max_picard_iterations,
        })
    }
}
