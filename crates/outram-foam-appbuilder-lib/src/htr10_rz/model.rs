// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! Assemble the HTR-10 R-Z solid model on a [`SolidConductionField`], set the
//! boundaries and the case, solve, and report.
//!
//! ## Boundary conditions
//!
//! - **Axis** (`r = 0`): adiabatic (symmetry).
//! - **RPV side wall to the RCCS** (`r = 218 cm`): grey radiation to the
//!   water-cooling panels held at **50 deg C** (TECDOC-1694 Sec 2.1.1.4).
//!   Both surfaces take the Tab 2.13 "air cavity outside the RPV" emissivity
//!   0.85. The exchange is the long concentric-cylinder form (Kugeler et al.
//!   2017, EUR 28712 EN, eq. 125, open):
//!   `eps_eff = 1 / (1/eps_1 + (r_1/r_2)(1/eps_2 - 1))` with `r_1 = 218 cm`
//!   and the panels at `r_2 = 285.4 cm` (the input sheet's reading of region
//!   39, **Uncertain**): `eps_eff = 0.7626`. ~~Air conduction and natural
//!   convection in the cavity are **not** modelled (under-states removal).~~
//!   **CORRECTED 2026-11-03 (gh:#1173):** natural convection of the cavity
//!   air runs in parallel with the radiation, **ON by default**
//!   ([`Htr10RzOptions::cavity_convection`], ablation
//!   [`Htr10RzOptions::without_cavity_convection`]): TECDOC-1694 Sec 2.1.1.4
//!   says "the RPV transfers heat to the water cooling panels through
//!   radiation and natural convection". It is the MacGregor & Emery (1969)
//!   vertical-enclosure correlation ([`EnclosureNaturalConvection`]) on the
//!   gap from the RPV (`r = 218 cm`) to the panels (`r = 285.4 cm`),
//!   `L = 67.4 cm`, over the RPV side height `H = 10.16 m` (`z -351..665`),
//!   with air at 1 atm (the cavity pressure is not stated: **Assumed**) at
//!   the film temperature; derived, not fitted. It is applied to the
//!   panels in both views.
//!   Sensitivity `RCCS-SHIELD` puts the region-42 insulating plate at
//!   `r = 283.4 cm` in the view as a thin two-sided radiation shield of the
//!   same emissivity ([`RccsView::InsulatingPlateShield`]).
//! - **RPV heads** (top `z = -351`, bottom `z = 665 cm`): the cavities
//!   beyond them (regions 26, 36, 37), the insulation (17, properties
//!   Unknown) and the concrete (15, held at 50 deg C) are not resolved in the
//!   input sheet. The default radiates each head to a 50 deg C surface as two
//!   parallel grey plates of 0.85, `eps_eff = 1/(2/0.85 - 1) = 0.7391`,
//!   which ignores the insulation and so **over-states** that path;
//!   sensitivity `HEADS-ADIABATIC` bounds it from the other side.
//!
//! ## The cases of gh:#1149 (a)
//!
//! "The bed" below is the core with its cone (model region `R1 PEBBLE BED
//! (CORE)`, the Tab 2.3 5 m^3 core volume); the discharge tube is a separate
//! free region of the same material.
//!
//! - [`Htr10RzCase::BedMeanTemperature`]: the bed generates power with the
//!   FPIC Tab 2.6 shape, and the power is found (secant iteration) at which
//!   the bed's **volume-mean** temperature is the given value, stagnant. With
//!   950 K this is the instrument of the `htgr_sim_v1` chain's 254.0 kW
//!   (`decay_heat_removal.rs`, "stagnant, 950 K bed": a lumped bed node at
//!   950 K behind a mean-to-surface conductance, in equilibrium). **This is
//!   the like-for-like comparison.**
//! - [`Htr10RzCase::HeldBed`]: the bed is **held** isothermal at a
//!   temperature up to its surface. Its cells are held, and its conductivity
//!   is replaced by a large constant (1e6 W/(m K)) so that the half cell
//!   between a held centre and the bed surface carries no drop; without
//!   that, the held boundary sits half a cell inside the bed and the duty
//!   converges only at first order in an offset set by how each Fig 2.5
//!   interval happens to be split (found in the first mesh study,
//!   2026-11-03; see the V&V README). Upper bound of the mean-950 K duty.
//! - [`Htr10RzCase::DecayPower`]: the bed generates a given power with the
//!   FPIC shape, and the temperatures that reject it are solved for. At
//!   steady state the duty equals the power, so the result is the
//!   temperature field and the split between the RCCS and the heads.

use uom::si::f64::{Length, Power, Ratio, ThermodynamicTemperature, VolumetricPowerDensity};
use uom::si::angle::radian;
use uom::si::f64::Angle;
use uom::si::length::centimeter;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::temperature_interval::kelvin as delta_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::volumetric_power_density::watt_per_cubic_meter;

use super::geometry::{
    idx, radial_lines_cm, region_at_cm, region_rects, regions, CoreVesselPlacement, RegionSpec,
    AXIAL_LINES_CM, N_REGIONS,
};
use super::materials::{MaterialOptions, RegionMaterial};
use super::power_map::{cell_average_w_per_m3, fpic_power_map, map_power_w};
use super::Htr10RzError;
use crate::genfoam::thermal_hydraulics::solid_conduction::{
    AxisEdges, ConductivityModel, EnclosureNaturalConvection, PicardReport, PropertyModel, RzBoundaries, RzGeometry, RzMesh, RzPatch, SolidBoundary,
    SolidConductionField, SolidConductionSettings, SolidRegion,
};
use outram_foam_basic_lib::ldu_matrix::SolverSettings;

/// RCCS panel and concrete temperature, K (50 deg C, TECDOC-1694
/// Sec 2.1.1.4).
pub const RCCS_TEMPERATURE_K: f64 = 323.15;
/// Emissivity of both surfaces of the cavity outside the RPV (Tab 2.13).
pub const CAVITY_EMISSIVITY: f64 = 0.85;
/// RPV outer radius, m (Tab 2.3: 420/2 + 8 cm).
pub const RPV_OUTER_RADIUS_M: f64 = 2.18;
/// RCCS panel radius, m: the input sheet's region-39 reading (Uncertain).
pub const RCCS_RADIUS_M: f64 = 2.854;
/// Region-42 insulating-plate radius, m (input sheet reading, Uncertain).
pub const INSULATING_PLATE_RADIUS_M: f64 = 2.834;
/// RPV side-wall height, m: the model's outer patch, `z = -351..665 cm`
/// (Tab 2.3 via the input sheet's Fig 2.5 axial lines).
pub const RPV_SIDE_HEIGHT_M: f64 = 10.16;
/// Reactor-cavity air pressure, Pa: one standard atmosphere (not stated in
/// any source; **Assumed**).
pub const CAVITY_AIR_PRESSURE_PA: f64 = 101_325.0;

/// The air enclosure between the RPV side wall and the RCCS panels
/// (gh:#1173): gap `RCCS_RADIUS_M - RPV_OUTER_RADIUS_M` = 0.674 m, height
/// [`RPV_SIDE_HEIGHT_M`], radius ratio 2.18/2.854, air at
/// [`CAVITY_AIR_PRESSURE_PA`].
#[must_use]
pub fn rpv_cavity_enclosure() -> EnclosureNaturalConvection {
    use uom::si::f64::Pressure;
    use uom::si::length::meter;
    use uom::si::pressure::pascal;
    EnclosureNaturalConvection {
        gap: Length::new::<meter>(RCCS_RADIUS_M - RPV_OUTER_RADIUS_M),
        height: Length::new::<meter>(RPV_SIDE_HEIGHT_M),
        radius_ratio: Ratio::new::<ratio>(RPV_OUTER_RADIUS_M / RCCS_RADIUS_M),
        pressure: Pressure::new::<pascal>(CAVITY_AIR_PRESSURE_PA),
    }
}

/// What the RPV side wall sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RccsView {
    /// The RCCS panels directly. Default (the instrument of `htgr_sim_v1`
    /// and of GAMMA+, Jun et al. 2009 Sec 2.4).
    #[default]
    DirectToPanels,
    /// The region-42 insulating plate as a thin radiation shield in front of
    /// the panels (sensitivity `RCCS-SHIELD`).
    InsulatingPlateShield,
}

impl RccsView {
    /// The effective emissivity of the RPV-to-panel exchange (per unit RPV
    /// area).
    #[must_use]
    pub fn effective_emissivity(self) -> f64 {
        let e = CAVITY_EMISSIVITY;
        let (r1, r2, rs) = (RPV_OUTER_RADIUS_M, RCCS_RADIUS_M, INSULATING_PLATE_RADIUS_M);
        match self {
            Self::DirectToPanels => 1.0 / (1.0 / e + r1 / r2 * (1.0 / e - 1.0)),
            Self::InsulatingPlateShield => {
                1.0 / ((1.0 / e + r1 / rs * (1.0 / e - 1.0))
                    + r1 / rs * (1.0 / e + rs / r2 * (1.0 / e - 1.0)))
            }
        }
    }
}

/// The RPV top and bottom heads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HeadBoundary {
    /// Radiate to 50 deg C as parallel grey plates of 0.85. Default.
    #[default]
    RadiateTo50C,
    /// Adiabatic (sensitivity `HEADS-ADIABATIC`).
    Adiabatic,
}

impl HeadBoundary {
    fn condition(self) -> SolidBoundary {
        match self {
            Self::RadiateTo50C => SolidBoundary::Radiative {
                emissivity: Ratio::new::<ratio>(1.0 / (2.0 / CAVITY_EMISSIVITY - 1.0)),
                ambient: ThermodynamicTemperature::new::<kelvin>(RCCS_TEMPERATURE_K),
            },
            Self::Adiabatic => SolidBoundary::Adiabatic,
        }
    }
}

/// Everything that is a modelling choice, each defaulting to the base case.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Htr10RzOptions {
    /// Material choices (bed model, side reflector below 300 deg C, helium
    /// pressure).
    pub materials: MaterialOptions,
    /// Core-vessel placement (input sheet N5).
    pub vessel: CoreVesselPlacement,
    /// RPV heads.
    pub heads: HeadBoundary,
    /// RPV side wall view.
    pub rccs: RccsView,
    /// Natural convection of the cavity air beside the RPV-to-RCCS
    /// radiation ([`rpv_cavity_enclosure`]). Default ON (gh:#1173);
    /// `false` is the ablation `CAVITY-NC-OFF`
    /// ([`Self::without_cavity_convection`]).
    pub cavity_convection: bool,
    /// Largest cell size: every Fig 2.5 mesh interval is split into
    /// `ceil(length / max_cell)` equal cells. `None` keeps the Fig 2.5 mesh
    /// itself (21 x 39 cells here, THERMIX's own resolution). Default 5 cm.
    pub max_cell: Option<Length>,
    /// Uniform subdivision `[radial, axial]` applied **after**
    /// [`Self::max_cell`]: every cell of that mesh is split into
    /// `refinement[0] x refinement[1]` equal cells (gh:#1176). Unlike a
    /// smaller `max_cell`, which re-splits each Fig 2.5 interval by
    /// `ceil(length / max)` and so refines different intervals by different
    /// ratios, this halves (`[2, 2]`) every cell in both directions, so a
    /// `[1, 1]`, `[2, 2]`, `[4, 4]` sequence is geometric with ratio 2 and
    /// every region boundary stays on a face. Default `[1, 1]` (no change).
    pub refinement: [usize; 2],
}

impl Default for Htr10RzOptions {
    fn default() -> Self {
        Self {
            materials: MaterialOptions::default(),
            vessel: CoreVesselPlacement::default(),
            heads: HeadBoundary::default(),
            rccs: RccsView::default(),
            cavity_convection: true,
            max_cell: Some(Length::new::<centimeter>(5.0)),
            refinement: [1, 1],
        }
    }
}

impl Htr10RzOptions {
    /// The ablation `CAVITY-NC-OFF`: the RPV side wall radiates only (the
    /// gh:#1149-#1151 records before gh:#1173).
    #[must_use]
    pub fn without_cavity_convection(mut self) -> Self {
        self.cavity_convection = false;
        self
    }

    /// Split every cell of the `max_cell` mesh uniformly, `radial x axial`
    /// (see [`Self::refinement`]; gh:#1176).
    #[must_use]
    pub fn with_refinement(mut self, radial: usize, axial: usize) -> Self {
        self.refinement = [radial, axial];
        self
    }

    /// The RPV side-wall boundary these options give.
    #[must_use]
    pub fn rpv_side_boundary(&self) -> SolidBoundary {
        let emissivity = Ratio::new::<ratio>(self.rccs.effective_emissivity());
        let ambient = ThermodynamicTemperature::new::<kelvin>(RCCS_TEMPERATURE_K);
        if self.cavity_convection {
            SolidBoundary::RadiativeAndConvective { emissivity, ambient, convection: rpv_cavity_enclosure() }
        } else {
            SolidBoundary::Radiative { emissivity, ambient }
        }
    }
}

/// What drives the solid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Htr10RzCase {
    /// The bed generates the power (FPIC shape) that makes its volume-mean
    /// temperature this value (stagnant).
    BedMeanTemperature {
        /// The target volume-mean bed temperature.
        temperature: ThermodynamicTemperature,
    },
    /// The bed held isothermal at this temperature up to its surface
    /// (stagnant).
    HeldBed {
        /// The held temperature.
        temperature: ThermodynamicTemperature,
    },
    /// The bed generates this total power with the FPIC Tab 2.6 shape.
    DecayPower {
        /// Total power.
        power: Power,
    },
}

fn axis(lines_cm: &[f64], max_cell: Option<Length>, refinement: usize) -> Result<AxisEdges, Htr10RzError> {
    if refinement == 0 {
        return Err(Htr10RzError::Mesh("refinement factor must be at least 1".into()));
    }
    let h = max_cell.map(|l| l.get::<centimeter>());
    let segs: Vec<(Length, usize)> = lines_cm
        .windows(2)
        .map(|w| {
            let n = h.map_or(1, |h| ((w[1] - w[0]) / h - 1e-9).ceil().max(1.0) as usize);
            (Length::new::<centimeter>(w[1]), n * refinement)
        })
        .collect();
    AxisEdges::segments(Length::new::<centimeter>(lines_cm[0]), &segs)
        .map_err(|e| Htr10RzError::Mesh(e.to_string()))
}

/// Build the R-Z mesh of the model (full `2 pi` wedge, so volumes and
/// powers are those of the whole reactor), each cell's region by the
/// painting rule at its centre.
///
/// # Errors
/// [`Htr10RzError::Mesh`] if the mesh fails to assemble.
pub fn build_mesh(options: &Htr10RzOptions) -> Result<RzMesh, Htr10RzError> {
    let (default_region, rects) = region_rects(options.vessel);
    RzMesh::build(
        RzGeometry::Axisymmetric {
            wedge_angle: Angle::new::<radian>(2.0 * std::f64::consts::PI),
        },
        axis(&radial_lines_cm(options.vessel), options.max_cell, options.refinement[0])?,
        axis(&AXIAL_LINES_CM, options.max_cell, options.refinement[1])?,
        N_REGIONS,
        |r, z| region_at_cm(&rects, default_region, r.get::<centimeter>(), z.get::<centimeter>()),
    )
    .map_err(|e| Htr10RzError::Mesh(e.to_string()))
}

/// Temperature span of one region after a solve, and whether it left the
/// range its closures cover (see `materials.rs`).
#[derive(Debug, Clone, PartialEq)]
pub struct RegionTemperatureRange {
    /// Model region index.
    pub region: usize,
    /// Its label.
    pub name: &'static str,
    /// Coldest cell.
    pub min: ThermodynamicTemperature,
    /// Hottest cell.
    pub max: ThermodynamicTemperature,
    /// True when a cell sits where a held (clamped) property value is used.
    pub outside_closure_range: bool,
}

/// The outcome of one solve.
#[derive(Debug, Clone)]
pub struct Htr10RzResult {
    /// Heat to the RCCS through the RPV side wall.
    pub rccs_side: Power,
    /// Heat leaving the RPV top head.
    pub top_head: Power,
    /// Heat leaving the RPV bottom head.
    pub bottom_head: Power,
    /// Heat generated in free cells.
    pub generated: Power,
    /// Heat given up by the held bed cells (zero for a power case).
    pub supplied_by_held_bed: Power,
    /// `|in - out| / in`.
    pub relative_imbalance: f64,
    /// Volume-mean temperature of the bed (core and cone).
    pub bed_mean_temperature: ThermodynamicTemperature,
    /// Secant iterations of a [`Htr10RzCase::BedMeanTemperature`] case
    /// (0 otherwise).
    pub power_iterations: usize,
    /// Hottest cell.
    pub max_temperature: ThermodynamicTemperature,
    /// Its centre `(r, z)`, cm (TECDOC `z`).
    pub max_at_cm: (f64, f64),
    /// Per-region temperature spans.
    pub region_ranges: Vec<RegionTemperatureRange>,
    /// Side-reflector cells (regions 2-side, 3, 4, 14, 30) below 300 deg C,
    /// where the side-reflector fit is replaced (the gh:#1148 flag).
    pub side_reflector_cells_below_300c: usize,
    /// Picard passes and the last change.
    pub picard: PicardReport,
    /// Cells in the mesh.
    pub n_cells: usize,
}

impl Htr10RzResult {
    /// Regions that reached a held property value.
    #[must_use]
    pub fn range_flags(&self) -> Vec<&RegionTemperatureRange> {
        self.region_ranges.iter().filter(|r| r.outside_closure_range).collect()
    }
}

/// The assembled model.
#[derive(Debug, Clone)]
pub struct Htr10RzModel {
    field: SolidConductionField,
    options: Htr10RzOptions,
    case: Htr10RzCase,
    specs: [RegionSpec; N_REGIONS],
    /// FPIC Tab 2.6 shape per cell, W/m^3 per W of total power.
    unit_source: Vec<f64>,
    /// Total power last mapped onto the cells, W.
    power_w: f64,
}

impl Htr10RzModel {
    /// Build the model for a case.
    ///
    /// # Errors
    /// [`Htr10RzError`] if the mesh, a property table or the field fails to
    /// build, or a held temperature or power is not positive.
    pub fn build(options: Htr10RzOptions, case: Htr10RzCase) -> Result<Self, Htr10RzError> {
        let mesh = build_mesh(&options)?;
        let specs = regions(options.vessel);
        let zero = VolumetricPowerDensity::new::<watt_per_cubic_meter>(0.0);
        let held = matches!(case, Htr10RzCase::HeldBed { .. });
        let solid_regions = specs
            .iter()
            .enumerate()
            .map(|(i, s)| {
                Ok(SolidRegion {
                    name: s.name.to_string(),
                    // A held bed is isothermal to its surface (module docs).
                    conductivity: if held && i == idx::BED {
                        ConductivityModel(PropertyModel::Constant(1.0e6))
                    } else {
                        s.material.conductivity_model(&options.materials)?
                    },
                    heat_capacity: s.material.heat_capacity_model(&options.materials)?,
                    heat_source: zero,
                })
            })
            .collect::<Result<Vec<_>, Htr10RzError>>()?;
        let rccs = options.rpv_side_boundary();
        let boundaries = RzBoundaries {
            inner: SolidBoundary::Adiabatic,
            outer: rccs,
            // Mesh z_min is the physical top (TECDOC z is downward).
            bottom: options.heads.condition(),
            top: options.heads.condition(),
        };
        let field = SolidConductionField::new(
            mesh,
            solid_regions,
            boundaries,
            vec![],
            ThermodynamicTemperature::new::<kelvin>(600.0),
        )
        .map_err(|e| Htr10RzError::Solve(e.to_string()))?
        .with_settings(SolidConductionSettings {
            picard_tolerance: uom::si::f64::TemperatureInterval::new::<delta_kelvin>(1e-6),
            max_picard_iterations: 2000,
            linear: SolverSettings {
                tolerance: 1e-12,
                max_iter: 400_000,
            },
        });
        let bed_cells: Vec<usize> = (0..field.mesh().n_cells())
            .filter(|c| field.mesh().cell_region(*c) == idx::BED)
            .collect();
        let map = fpic_power_map();
        let per_watt = 1.0 / map_power_w(&map);
        let (re, ze) = (field.mesh().r_edges().clone(), field.mesh().z_edges().clone());
        let nr = re.n_cells();
        let unit_source: Vec<f64> = (0..field.mesh().n_cells())
            .map(|c| {
                if field.mesh().cell_region(c) != idx::BED {
                    return 0.0;
                }
                let (i, j) = (c % nr, c / nr);
                per_watt
                    * cell_average_w_per_m3(
                        &map,
                        [re.edge(i).get::<centimeter>(), re.edge(i + 1).get::<centimeter>()],
                        [ze.edge(j).get::<centimeter>(), ze.edge(j + 1).get::<centimeter>()],
                    )
            })
            .collect();
        let mut model = Self {
            field,
            options,
            case,
            specs,
            unit_source,
            power_w: 0.0,
        };
        match case {
            Htr10RzCase::HeldBed { temperature } => {
                if !(temperature.get::<kelvin>() > 0.0) {
                    return Err(Htr10RzError::Input("held temperature must be positive".into()));
                }
                let held: Vec<_> = bed_cells.iter().map(|c| (*c, temperature)).collect();
                model
                    .field
                    .set_fixed_temperatures(&held)
                    .map_err(|e| Htr10RzError::Solve(e.to_string()))?;
            }
            Htr10RzCase::DecayPower { power } => {
                let p = power.get::<watt>();
                if !(p > 0.0) || !p.is_finite() {
                    return Err(Htr10RzError::Input("power must be positive".into()));
                }
                model.set_power(p)?;
            }
            Htr10RzCase::BedMeanTemperature { temperature } => {
                if !(temperature.get::<kelvin>() > RCCS_TEMPERATURE_K) {
                    return Err(Htr10RzError::Input(
                        "the target bed temperature must be above the 50 C sink".into(),
                    ));
                }
                model.set_power(2.0e5)?;
            }
        }
        Ok(model)
    }

    fn set_power(&mut self, p: f64) -> Result<(), Htr10RzError> {
        self.power_w = p;
        let src: Vec<VolumetricPowerDensity> = self
            .unit_source
            .iter()
            .map(|q| VolumetricPowerDensity::new::<watt_per_cubic_meter>(q * p))
            .collect();
        self.field
            .set_cell_heat_source(&src)
            .map_err(|e| Htr10RzError::Solve(e.to_string()))
    }

    /// Volume-mean temperature of the bed (core and cone), K.
    fn bed_mean_k(&self) -> f64 {
        let fv = self.field.mesh().mesh();
        let (mut vt, mut v) = (0.0, 0.0);
        for c in 0..self.field.mesh().n_cells() {
            if self.field.mesh().cell_region(c) == idx::BED {
                vt += fv.cell_volumes[c] * self.field.temperature(c).get::<kelvin>();
                v += fv.cell_volumes[c];
            }
        }
        vt / v
    }

    /// Power density mapped onto cell `c` (zero outside the bed and for a
    /// held case).
    #[must_use]
    pub fn cell_power_density(&self, c: usize) -> VolumetricPowerDensity {
        VolumetricPowerDensity::new::<watt_per_cubic_meter>(self.unit_source[c] * self.power_w)
    }

    /// The field, for a coupled driver in this crate (gh:#1150 sets the
    /// gas-solid exchange through it).
    pub(crate) fn field_mut(&mut self) -> &mut SolidConductionField {
        &mut self.field
    }

    /// The solid conduction field (mesh, temperatures, regions).
    #[must_use]
    pub fn field(&self) -> &SolidConductionField {
        &self.field
    }

    /// The options it was built with.
    #[must_use]
    pub fn options(&self) -> &Htr10RzOptions {
        &self.options
    }

    /// The region table.
    #[must_use]
    pub fn regions(&self) -> &[RegionSpec; N_REGIONS] {
        &self.specs
    }

    /// Temperature of the cell containing `(r, z)` in cm (TECDOC `z`).
    #[must_use]
    pub fn temperature_at_cm(&self, r_cm: f64, z_cm: f64) -> Option<ThermodynamicTemperature> {
        self.field.temperature_at(
            Length::new::<centimeter>(r_cm),
            Length::new::<centimeter>(z_cm),
        )
    }

    /// Solve the steady state and report.
    ///
    /// # Errors
    /// [`Htr10RzError::Solve`] if the Picard iteration or a linear solve
    /// fails.
    pub fn solve_steady(&mut self) -> Result<Htr10RzResult, Htr10RzError> {
        let solve = |m: &mut Self| {
            m.field
                .solve_steady()
                .map_err(|e| Htr10RzError::Solve(e.to_string()))
        };
        let Htr10RzCase::BedMeanTemperature { temperature } = self.case else {
            let picard = solve(self)?;
            return Ok(self.report(picard, 0));
        };
        // Secant on the power, from 200 kW and a linear estimate. The mean
        // bed temperature rises monotonically with the power.
        let target = temperature.get::<kelvin>();
        let sink = RCCS_TEMPERATURE_K;
        let mut p0 = 2.0e5;
        self.set_power(p0)?;
        solve(self)?;
        let mut t0 = self.bed_mean_k();
        let mut p1 = p0 * (target - sink) / (t0 - sink);
        for it in 1..=40 {
            self.set_power(p1)?;
            let picard = solve(self)?;
            let t1 = self.bed_mean_k();
            if (t1 - target).abs() < 1e-4 {
                return Ok(self.report(picard, it + 1));
            }
            let p2 = p1 + (target - t1) * (p1 - p0) / (t1 - t0);
            (p0, t0, p1) = (p1, t1, p2.max(1.0));
        }
        Err(Htr10RzError::Solve(format!(
            "power search for a {target} K bed mean did not converge (last {:.3} K)",
            self.bed_mean_k()
        )))
    }

    fn report(&self, picard: PicardReport, power_iterations: usize) -> Htr10RzResult {
        let f = &self.field;
        let bal = f.energy_balance();
        let w = |p: RzPatch| bal.removed_per_patch[p.index()];
        let n = f.mesh().n_cells();
        let mut tmax = f64::NEG_INFINITY;
        let mut at = (0.0, 0.0);
        let mut lo = [f64::INFINITY; N_REGIONS];
        let mut hi = [f64::NEG_INFINITY; N_REGIONS];
        let mut below = 0;
        for c in 0..n {
            let t = f.temperature(c).get::<kelvin>();
            let reg = f.mesh().cell_region(c);
            lo[reg] = lo[reg].min(t);
            hi[reg] = hi[reg].max(t);
            if t > tmax {
                tmax = t;
                let (r, z) = f.mesh().cell_centre(c);
                at = (r.get::<centimeter>(), z.get::<centimeter>());
            }
            if matches!(self.specs[reg].material, RegionMaterial::SideReflector { .. }) && t < 573.15 {
                below += 1;
            }
        }
        let region_ranges = (0..N_REGIONS)
            .filter(|r| lo[*r].is_finite())
            .map(|r| {
                let (vlo, vhi) = self.specs[r].material.valid_range_kelvin(&self.options.materials);
                RegionTemperatureRange {
                    region: r,
                    name: self.specs[r].name,
                    min: ThermodynamicTemperature::new::<kelvin>(lo[r]),
                    max: ThermodynamicTemperature::new::<kelvin>(hi[r]),
                    outside_closure_range: lo[r] < vlo || hi[r] > vhi,
                }
            })
            .collect();
        Htr10RzResult {
            rccs_side: w(RzPatch::Outer),
            top_head: w(RzPatch::Bottom),
            bottom_head: w(RzPatch::Top),
            generated: bal.generated,
            supplied_by_held_bed: bal.supplied_by_fixed_cells,
            relative_imbalance: bal.relative_imbalance(),
            bed_mean_temperature: ThermodynamicTemperature::new::<kelvin>(self.bed_mean_k()),
            power_iterations,
            max_temperature: ThermodynamicTemperature::new::<kelvin>(tmax),
            max_at_cm: at,
            region_ranges,
            side_reflector_cells_below_300c: below,
            picard,
            n_cells: n,
        }
    }
}
