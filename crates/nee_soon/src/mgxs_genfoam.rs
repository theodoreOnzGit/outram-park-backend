// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK.

//! # `mgxs_genfoam` — HTR-10 Monte Carlo MGXS into a GeN-Foam R-Z diffusion core
//!
//! gh:#1187 (epic gh:#1183). The second coupling chain: **Monte Carlo ->
//! multigroup constants -> GeN-Foam**. Four isothermal whole-core Monte Carlo
//! records (`htr10_mgxs` cases `core_n12_t{300.15, 600, 900, 1200}`) become one
//! temperature-parametrised `nuclearData`, homogenised into the regions of an
//! R-Z mesh that is the **`htr10_rz` mesh** (the TH model's own lines),
//! cropped to the Monte Carlo model and with each cell's region read from the
//! **assembled Monte Carlo geometry** at the cell.
//!
//! ## The shortest thing that works
//!
//! ```no_run
//! use nee_soon::mgxs_genfoam::{Htr10MgxsRegions, Htr10RzNeutronicsMesh, MgxsStatePoints};
//! use uom::si::f64::ThermodynamicTemperature;
//! use uom::si::thermodynamic_temperature::kelvin;
//!
//! let points = MgxsStatePoints::htr10_records().unwrap();          // the four on-disk records
//! let mesh = Htr10RzNeutronicsMesh::for_records(&points).unwrap(); // R-Z mesh + zone_of_cell
//! let regions = Htr10MgxsRegions::new(&mesh);                    // rebalanced, descending, SI: all ON
//! let nd = regions.nuclear_data(&points).unwrap();               // xsVariables { TFuel log; }
//! let t = vec![ThermodynamicTemperature::new::<kelvin>(600.0); mesh.n_cells()];
//! let sol = mesh.solve_diffusion(&nd, &t).unwrap();
//! println!("k = {}", sol.k_eff);
//! ```
//!
//! ## Module map
//!
//! | item | what |
//! |---|---|
//! | [`MgxsStatePoints`] | the state points: one Monte Carlo MGXS library and `k` per fuel temperature |
//! | [`Htr10RzNeutronicsMesh`] | the R-Z mesh, `zone_of_cell`, the regions' recipes, its picture, the diffusion solve with per-cell `TFuel` |
//! | [`RegionRecipe`] | how one neutronics region's constants are made from the record's per-material constants |
//! | [`Htr10MgxsRegions`] | regions -> collapse -> rebalance -> descending order -> SI units -> stacked `NuclearDataInput`; each step ON by default, `without_*` to ablate |
//! | [`stack_states`] | single-state inputs -> one input with `xsVariables { TFuel <law>; }` (ported from `dhoby-ghaut`) |
//!
//! ## The traps, each handled by a default (and each ablation pinned by a test)
//!
//! - **Rebalance** (`MgxsLibrary::rebalanced`): `Sigma_t = Sigma_a + sum
//!   Sigma_s` row by row. The HTR-10 record went from -17 133 to -353 pcm
//!   against the Monte Carlo `k_inf` with it. Ablation:
//!   [`Htr10MgxsRegions::without_rebalance`].
//! - **Group order**: GeN-Foam reads group 0 as the fastest; the records are
//!   tallied ascending, and the converter cannot tell. Ablation:
//!   [`Htr10MgxsRegions::without_descending_order`]. **Measured 2026-11-03:
//!   it does not move the steady `k`** (a consistent reversal of every
//!   group axis is a relabelling, 1.02789 both ways); it does make the
//!   inverse velocities `genfoam_xs` writes wrong, which matters for a
//!   transient.
//! - **Units**: the records are cm; GeN-Foam is SI (`Sigma` x 100, `D` x
//!   0.01). Ablation: [`Htr10MgxsRegions::without_unit_conversion`].
//! - **One zone for the whole core** costs about -27 000 pcm: the regions
//!   here are the bed, the fuel-free pebble bed of the cone and the discharge
//!   tube, the cavity, and **one region per Monte Carlo reflector material**
//!   (each TECDOC-1382 zone composition sits where the Monte Carlo puts it).
//!
//! ## Assumptions, stated
//!
//! - **The bed's helium flux.** The records tally per *material*, and helium
//!   fills both the bed's voids and the cavity above it, so the bed's helium
//!   flux cannot be separated from the cavity's. The bed region is therefore
//!   built by **volume mixing**: the solids' flux-weighted constants times
//!   their volume fraction `f` (sampled from the assembled geometry), plus the
//!   helium's times `1 - f`, i.e. the flux per unit volume is assumed equal in
//!   the pebbles and in the helium between them. The cone and the discharge
//!   tube (dummy balls only) are built the same way from the pebble graphite.
//!   The pebble-graphite constants are flux-weighted over every pebble's
//!   graphite, fuelled bed and cone alike.
//! - **P0 diffusion**: `D = 1/(3 Sigma_t)`, no transport correction (no P1
//!   matrix was tallied); in graphite that under-states `D` by about
//!   `mu_bar = 2/(3A) = 5.6 %`.
//! - **The cavity** (helium at 1 atm, `D` of order 10^2 m) is a diffusion
//!   region: diffusion cannot represent streaming across it.
//! - **Withdrawn rods** are not regions of their own: a cell's region is the
//!   material most of its sample points hit.
//! - **Boundaries**: Marshak vacuum (GeN-Foam albedo `gamma = 0.5`) on the
//!   outer radius, top and bottom of the Monte Carlo model; the axis
//!   reflective.
//!
//! Status: untrusted AI-generated draft (2026-11-03), research, education
//! and V&V only (`RESPONSIBLE_USE.md`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use outram_foam_appbuilder_lib::genfoam::neutronics::albedo::AlbedoLinearisation;
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::{DiffusionNeutronics, DiffusionSettings};
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::input::NuclearDataInput;
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::variables::{VariableLaw, XsVariable};
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solid_conduction::{AxisEdges, RzGeometry, RzMesh, RzPatch};
use outram_foam_appbuilder_lib::htr10_rz::geometry::{radial_lines_cm, AXIAL_LINES_CM};
use outram_foam_appbuilder_lib::htr10_rz::CoreVesselPlacement;
use outram_foam_basic_lib::prelude::BoundaryCondition;
use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::plot::{annotate_image, ImageData, LegendEntry, Rgb};
use outram_mc_libs::geometry::position::{Direction, Position};
use petir::mathf::RealMath;
use uom::si::angle::radian;
use uom::si::f64::{Angle, Length, Ratio, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

use crate::genfoam_xs::{to_nuclear_data_input, CM_INV_TO_M_INV, CM_TO_M};
use crate::htr10_mgxs::{CaseRecord, Collapse};
use crate::htr10_rmc::core_model::{assemble_explicit_triso, mat, AssembledCore, HTR10_REFLECTOR_OUTER_CM};
use crate::htr10_rmc::parametric::RECORD_RINGS;
use crate::mgxs::{MgxsLibrary, ZoneMgxs};
use crate::power_shape::{Frame, FrameMap};

/// What went wrong.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MgxsGenfoamError {
    /// A record could not be read or the state points disagree.
    #[error("state points: {0}")]
    StatePoints(String),
    /// The constants could not be built.
    #[error("constants: {0}")]
    Constants(String),
    /// The mesh could not be built.
    #[error("mesh: {0}")]
    Mesh(String),
    /// The diffusion solve failed.
    #[error("diffusion: {0}")]
    Diffusion(String),
}

// ─────────────────────────────────────────────────────────────────────────
// State points
// ─────────────────────────────────────────────────────────────────────────

/// One isothermal Monte Carlo MGXS state point.
#[derive(Debug, Clone)]
pub struct MgxsStatePoint {
    /// The temperature every material was at.
    pub temperature: ThermodynamicTemperature,
    /// The per-material library, as tallied (ascending groups, cm units).
    pub library: MgxsLibrary,
    /// The Monte Carlo `k_eff` of the run.
    pub k_eff: Ratio,
    /// Its one-sigma uncertainty.
    pub k_std: Ratio,
    /// Şeker layers of the run's bed (12 for every `core_n12_*` record).
    pub layers: usize,
    /// The case name.
    pub case: String,
}

/// **The state points GeN-Foam interpolates between**: one MGXS library per
/// fuel temperature, ascending in temperature, the first the reference.
#[derive(Debug, Clone)]
pub struct MgxsStatePoints {
    /// The points, ascending in temperature.
    pub points: Vec<MgxsStatePoint>,
}

/// The four isothermal full-core records (`htr10_mgxs` gh:#845), in
/// ascending temperature.
pub const HTR10_ISOTHERMAL_RECORDS: [&str; 4] =
    ["core_n12_t300", "core_n12_t600", "core_n12_t900", "core_n12_t1200"];

impl MgxsStatePoints {
    /// The folder holding the `htr10_mgxs` case records.
    #[must_use]
    pub fn records_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/htr10_mgxs_runs")
    }

    /// **The four isothermal HTR-10 records** ([`HTR10_ISOTHERMAL_RECORDS`]).
    ///
    /// # Errors
    /// As [`Self::from_record_dirs`].
    pub fn htr10_records() -> Result<Self, MgxsGenfoamError> {
        let root = Self::records_root();
        let dirs: Vec<PathBuf> = HTR10_ISOTHERMAL_RECORDS.iter().map(|c| root.join(c)).collect();
        Self::from_record_dirs(&dirs)
    }

    /// State points from `htr10_mgxs` case folders (each with `record.json`).
    ///
    /// # Errors
    /// A record that does not read; records on different group structures,
    /// zone lists or bed heights; two at one temperature.
    pub fn from_record_dirs(dirs: &[PathBuf]) -> Result<Self, MgxsGenfoamError> {
        let mut points = Vec::new();
        for d in dirs {
            let r = CaseRecord::read(d).map_err(MgxsGenfoamError::StatePoints)?;
            let (k, s) = r.k();
            let layers = layers_of(&r.model).unwrap_or(12);
            points.push(MgxsStatePoint {
                temperature: ThermodynamicTemperature::new::<kelvin>(r.temperature_k),
                library: r.library,
                k_eff: Ratio::new::<ratio>(k),
                k_std: Ratio::new::<ratio>(s),
                layers,
                case: r.case,
            });
        }
        Self::new(points)
    }

    /// From points in any order; sorted ascending in temperature.
    ///
    /// # Errors
    /// No points; points on different group structures, zone lists or bed
    /// heights; two at one temperature.
    pub fn new(mut points: Vec<MgxsStatePoint>) -> Result<Self, MgxsGenfoamError> {
        if points.is_empty() {
            return Err(MgxsGenfoamError::StatePoints("no state points".into()));
        }
        points.sort_by(|a, b| a.temperature.get::<kelvin>().total_cmp(&b.temperature.get::<kelvin>()));
        let first = &points[0];
        for p in &points[1..] {
            if p.library.groups.edges() != first.library.groups.edges() {
                return Err(MgxsGenfoamError::StatePoints(format!("{}: another group structure", p.case)));
            }
            let names = |l: &MgxsLibrary| l.zones.iter().map(|z| z.name.clone()).collect::<Vec<_>>();
            if names(&p.library).len() != names(&first.library).len() {
                return Err(MgxsGenfoamError::StatePoints(format!("{}: another zone list", p.case)));
            }
            if p.layers != first.layers {
                return Err(MgxsGenfoamError::StatePoints(format!("{}: another bed", p.case)));
            }
        }
        if points.windows(2).any(|w| w[0].temperature == w[1].temperature) {
            return Err(MgxsGenfoamError::StatePoints("two points at one temperature".into()));
        }
        Ok(Self { points })
    }

    /// Şeker layers of the bed every point was run on.
    #[must_use]
    pub fn layers(&self) -> usize {
        self.points[0].layers
    }

    /// Whether material slot `s` carries flux in every group of the
    /// eight-group structure ([`Collapse::Eight`], the finest coarse
    /// structure offered) at every point: the slots a region can be made of.
    ///
    /// ~~Every one of the 70 tally groups~~ **CORRECTED 2026-11-03**: that
    /// excluded most reflector slots (no neutron reaches some of the top
    /// fine groups there), and the mesh then labelled the top reflector
    /// helium. A region's constants are only ever used collapsed.
    #[must_use]
    pub fn slot_has_flux(&self, s: usize) -> bool {
        let coarse = Collapse::Eight.structure();
        self.points.iter().all(|p| {
            p.library.zones.get(s).is_some_and(|z| {
                let one = MgxsLibrary { groups: p.library.groups.clone(), zones: vec![z.clone()] };
                one.collapse(&coarse).is_ok_and(|c| c.zones[0].flux.iter().all(|f| *f > 0.0))
            })
        })
    }
}

/// `N` of "... N = 12 ..." in a record's model line.
fn layers_of(model: &str) -> Option<usize> {
    let i = model.find("N = ")?;
    model[i + 4..].split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

// ─────────────────────────────────────────────────────────────────────────
// Regions
// ─────────────────────────────────────────────────────────────────────────

/// How one neutronics region's constants are made from a per-material
/// library (fine groups, before collapse).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RegionRecipe {
    /// The fuelled bed: the pebbles' solids (kernel, coatings, pebble
    /// graphite) flux-weighted together, volume-mixed with helium at
    /// `solid_fraction` (module docs).
    Bed {
        /// Volume fraction of solids in the bed, `(0, 1)`.
        solid_fraction: f64,
    },
    /// The cone and discharge tube: pebble graphite volume-mixed with helium.
    DummyBed {
        /// Volume fraction of graphite, `(0, 1)`.
        solid_fraction: f64,
    },
    /// The helium of the cavity (and of any cell mostly gas).
    Cavity,
    /// One Monte Carlo material slot's own constants (a reflector zone).
    Slot(usize),
}

impl RegionRecipe {
    /// The region's constants from `lib` (one zone per material slot), named
    /// `name`.
    ///
    /// # Errors
    /// A slot outside the library.
    pub fn build(&self, lib: &MgxsLibrary, name: &str) -> Result<ZoneMgxs, MgxsGenfoamError> {
        let zone = |s: usize| {
            lib.zones
                .get(s)
                .cloned()
                .ok_or_else(|| MgxsGenfoamError::Constants(format!("no material slot {s}")))
        };
        let mut z = match *self {
            Self::Bed { solid_fraction } => {
                let solids: Vec<usize> = (mat::KERNEL..=mat::GRAPHITE).collect();
                let s = lib.homogenised_subset(&solids, "solids").zones.remove(0);
                mix_by_volume(&[(&s, solid_fraction), (&zone(mat::HELIUM)?, 1.0 - solid_fraction)])
            }
            Self::DummyBed { solid_fraction } => {
                mix_by_volume(&[(&zone(mat::GRAPHITE)?, solid_fraction), (&zone(mat::HELIUM)?, 1.0 - solid_fraction)])
            }
            Self::Cavity => zone(mat::HELIUM)?,
            Self::Slot(s) => zone(s)?,
        };
        z.name = name.to_string();
        Ok(z)
    }
}

/// **Volume mixing** of zones with equal flux per unit volume: every cross
/// section is `sum_i f_i Sigma_i`; the flux spectrum is the volume-weighted
/// sum of each component's normalised spectrum; `chi` is weighted by each
/// component's fission source. Standard errors are dropped (the mixture's
/// are not those of any tally).
#[must_use]
pub fn mix_by_volume(parts: &[(&ZoneMgxs, f64)]) -> ZoneMgxs {
    let g = parts.first().map_or(0, |p| p.0.flux.len());
    let lin = |pick: &dyn Fn(&ZoneMgxs) -> &Vec<f64>| -> Vec<f64> {
        (0..g).map(|k| parts.iter().map(|(z, f)| f * pick(z).get(k).copied().unwrap_or(0.0)).sum()).collect()
    };
    let mat = |pick: &dyn Fn(&ZoneMgxs) -> &Vec<Vec<f64>>| -> Vec<Vec<f64>> {
        if parts.iter().any(|(z, _)| pick(z).len() != g) {
            return Vec::new();
        }
        (0..g)
            .map(|a| (0..g).map(|b| parts.iter().map(|(z, f)| f * pick(z)[a][b]).sum()).collect())
            .collect()
    };
    let spectrum = |z: &ZoneMgxs| -> Vec<f64> {
        let s: f64 = z.flux.iter().sum();
        z.flux.iter().map(|x| if s > 0.0 { x / s } else { 0.0 }).collect()
    };
    let flux: Vec<f64> = (0..g).map(|k| parts.iter().map(|(z, f)| f * spectrum(z)[k]).sum()).collect();
    // chi weighted by fission source per unit volume f_i sum_g nuSigma_f phi_hat.
    let src: Vec<f64> = parts
        .iter()
        .map(|(z, f)| f * z.nu_fission.iter().zip(spectrum(z)).map(|(a, b)| a * b).sum::<f64>())
        .collect();
    let ts: f64 = src.iter().sum();
    let chi: Vec<f64> = (0..g)
        .map(|k| {
            if ts > 0.0 {
                parts.iter().zip(&src).map(|((z, _), s)| s / ts * z.chi.get(k).copied().unwrap_or(0.0)).sum()
            } else {
                0.0
            }
        })
        .collect();
    ZoneMgxs {
        name: "mixture".into(),
        flux,
        total: lin(&|z| &z.total),
        absorption: lin(&|z| &z.absorption),
        fission: lin(&|z| &z.fission),
        nu_fission: lin(&|z| &z.nu_fission),
        kappa_fission: lin(&|z| &z.kappa_fission),
        scatter: mat(&|z| &z.scatter),
        scatter_only: mat(&|z| &z.scatter_only),
        chi,
        sigma: None,
    }
}

/// **The constants pipeline**, every trap handled by default:
/// regions ([`RegionRecipe`]) -> collapse to `collapse` (4 groups default)
/// -> rebalance -> descending energy -> SI units -> one `NuclearDataInput`
/// with `xsVariables { TFuel log; }`.
#[derive(Debug, Clone, PartialEq)]
pub struct Htr10MgxsRegions {
    /// The regions, in zone order: `(name, recipe)`.
    pub regions: Vec<(String, RegionRecipe)>,
    /// The coarse structure (an exact collapse of the tally's WIMS-69 + 1).
    pub collapse: Collapse,
    rebalance: bool,
    descending: bool,
    si_units: bool,
    law: VariableLaw,
}

impl Htr10MgxsRegions {
    /// The regions of `mesh`, with every default ON and 4 groups.
    #[must_use]
    pub fn new(mesh: &Htr10RzNeutronicsMesh) -> Self {
        Self::from_regions(mesh.regions.clone())
    }

    /// Given regions (zone order), every default ON, 4 groups.
    #[must_use]
    pub fn from_regions(regions: Vec<(String, RegionRecipe)>) -> Self {
        Self {
            regions,
            collapse: Collapse::Four,
            rebalance: true,
            descending: true,
            si_units: true,
            law: VariableLaw::Log,
        }
    }

    /// Collapse to `c` instead of four groups.
    #[must_use]
    pub fn with_collapse(mut self, c: Collapse) -> Self {
        self.collapse = c;
        self
    }

    /// The `TFuel` interpolation law (default `log`, GeN-Foam's for Doppler).
    #[must_use]
    pub fn with_law(mut self, law: VariableLaw) -> Self {
        self.law = law;
        self
    }

    /// **Ablation**: keep the tallied `Sigma_t` (no `Sigma_t = Sigma_a +
    /// sum Sigma_s` rebalance).
    #[must_use]
    pub fn without_rebalance(mut self) -> Self {
        self.rebalance = false;
        self
    }

    /// **Ablation**: hand GeN-Foam the tally's ascending group order (it reads
    /// group 0 as the fastest).
    #[must_use]
    pub fn without_descending_order(mut self) -> Self {
        self.descending = false;
        self
    }

    /// **Ablation**: hand GeN-Foam the cm constants as if they were SI.
    #[must_use]
    pub fn without_unit_conversion(mut self) -> Self {
        self.si_units = false;
        self
    }

    /// Whether the rebalance, the descending order and the SI conversion are
    /// on, in that order.
    #[must_use]
    pub fn defaults(&self) -> (bool, bool, bool) {
        (self.rebalance, self.descending, self.si_units)
    }

    /// The regions' constants at one state point: built, collapsed, and
    /// (by default) rebalanced and in descending order. cm units.
    ///
    /// # Errors
    /// A recipe or the collapse fails.
    pub fn library(&self, lib: &MgxsLibrary) -> Result<MgxsLibrary, MgxsGenfoamError> {
        let zones = self
            .regions
            .iter()
            .map(|(n, r)| r.build(lib, n))
            .collect::<Result<Vec<_>, _>>()?;
        let fine = MgxsLibrary {
            groups: lib.groups.clone(),
            zones,
        };
        let mut out = fine
            .collapse(&self.collapse.structure())
            .map_err(|e| MgxsGenfoamError::Constants(format!("{e:?}")))?;
        if self.rebalance {
            out = out.rebalanced();
        }
        if self.descending {
            out = out.in_descending_energy();
        }
        Ok(out)
    }

    /// One state point as a single-state GeN-Foam input (SI unless
    /// ablated).
    ///
    /// # Errors
    /// As [`Self::library`] and [`to_nuclear_data_input`].
    pub fn single_state(&self, lib: &MgxsLibrary) -> Result<NuclearDataInput, MgxsGenfoamError> {
        let mut nd = to_nuclear_data_input(&self.library(lib)?).map_err(|e| MgxsGenfoamError::Constants(e.to_string()))?;
        if !self.si_units {
            for s in &mut nd.states {
                for z in &mut s.zones {
                    let back = |v: &mut Vec<f64>| v.iter_mut().for_each(|x| *x /= CM_INV_TO_M_INV);
                    back(&mut z.nu_sigma_eff);
                    back(&mut z.sigma_pow);
                    back(&mut z.sigma_removal);
                    z.d.iter_mut().for_each(|x| *x /= CM_TO_M);
                    for m in &mut z.scattering {
                        for row in m {
                            back(row);
                        }
                    }
                }
            }
        }
        Ok(nd)
    }

    /// **Every state point stacked** into one input, `xsVariables { TFuel
    /// <law>; }`, the coldest point the reference.
    ///
    /// # Errors
    /// As [`Self::single_state`].
    pub fn nuclear_data(&self, points: &MgxsStatePoints) -> Result<NuclearDataInput, MgxsGenfoamError> {
        let inputs = points
            .points
            .iter()
            .map(|p| Ok((p.temperature.get::<kelvin>(), self.single_state(&p.library)?)))
            .collect::<Result<Vec<_>, MgxsGenfoamError>>()?;
        Ok(stack_states(&inputs, inputs[0].0, self.law))
    }
}

/// **Stack single-state inputs** into one with `xsVariables { TFuel <law>;
/// }`: the state at `reference` K is GeN-Foam's `reference` state, the others
/// follow named `T<K>K`, each carrying its `TFuel`.
///
/// Ported from `dhoby-ghaut`'s `src/bin/dhoby-ghaut/mgxs_run.rs::stack_states`
/// (gh:#573), which cannot be imported (dhoby-ghaut depends on this crate).
#[must_use]
pub fn stack_states(inputs: &[(f64, NuclearDataInput)], reference: f64, law: VariableLaw) -> NuclearDataInput {
    let ri = inputs.iter().position(|x| x.0 == reference).unwrap_or(0);
    let mut nd = inputs[ri].1.clone();
    nd.xs_variables = vec![XsVariable::new("TFuel", law)];
    nd.states.clear();
    let mut order: Vec<usize> = vec![ri];
    order.extend((0..inputs.len()).filter(|&i| i != ri));
    for i in order {
        let (t_k, inp) = &inputs[i];
        let mut s = inp.states[0].clone();
        s.name = if i == ri { "reference".into() } else { format!("T{t_k:.0}K") };
        s.parameters.clear();
        s.parameters.insert("TFuel".into(), *t_k);
        nd.states.push(s);
    }
    nd
}

// ─────────────────────────────────────────────────────────────────────────
// The R-Z neutronics mesh
// ─────────────────────────────────────────────────────────────────────────

/// **The R-Z neutronics mesh**: the `htr10_rz` mesh lines (Fig 2.5), cropped
/// to the Monte Carlo model (`r <= 190` cm, its top and bottom) with lines
/// added at the Monte Carlo bed top, cavity top and model ends, cells no
/// larger than `max_cell`. Each cell's region comes from sampling the
/// **assembled** Monte Carlo geometry over the cell (module docs). TECDOC
/// frame (`z` down, metres in the mesh).
#[derive(Debug, Clone)]
pub struct Htr10RzNeutronicsMesh {
    /// The mesh.
    pub mesh: RzMesh,
    /// Region (GeN-Foam zone) of each cell.
    pub zone_of_cell: Vec<usize>,
    /// The regions, in zone order.
    pub regions: Vec<(String, RegionRecipe)>,
    /// The frame map of the Monte Carlo loading.
    pub frame: FrameMap,
    /// The sampled solid fraction of the fuelled bed and of the cone/tube.
    pub solid_fractions: (f64, f64),
}

impl Htr10RzNeutronicsMesh {
    /// The mesh for the bed the state points were run on (20 cm cells).
    ///
    /// # Errors
    /// As [`Self::build`].
    pub fn for_records(points: &MgxsStatePoints) -> Result<Self, MgxsGenfoamError> {
        let core = assemble_explicit_triso(RECORD_RINGS, points.layers(), 0);
        let present: Vec<bool> = (0..points.points[0].library.zones.len()).map(|s| points.slot_has_flux(s)).collect();
        Self::build(&core, &present, Length::new::<centimeter>(20.0))
    }

    /// The mesh for `core` (the record model's, non-zoned slots). `present[s]`
    /// says whether slot `s` has constants (flux in every group); a cell is
    /// never given a slot without them.
    ///
    /// # Errors
    /// The mesh does not assemble.
    pub fn build(core: &AssembledCore, present: &[bool], max_cell: Length) -> Result<Self, MgxsGenfoamError> {
        let frame = FrameMap::from_core(core);
        let tz = |z_mc: f64| frame.z_from_mc(Frame::Tecdoc, Length::new::<centimeter>(z_mc)).get::<centimeter>();
        let (z_top, z_bot) = (tz(core.refl_top), tz(core.refl_bottom));
        let r_max = HTR10_REFLECTOR_OUTER_CM;
        let mut rl: Vec<f64> = radial_lines_cm(CoreVesselPlacement::Fig25).into_iter().filter(|r| *r <= r_max).collect();
        rl.push(r_max);
        let mut zl: Vec<f64> = AXIAL_LINES_CM.iter().copied().filter(|z| *z > z_top && *z < z_bot).collect();
        zl.extend([z_top, z_bot, tz(core.bed_half_height), tz(-core.bed_half_height), tz(core.cavity_top), tz(core.conus_floor)]);
        let fine = |mut v: Vec<f64>| -> Vec<f64> {
            v.sort_by(f64::total_cmp);
            v.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
            let m = max_cell.get::<centimeter>();
            let mut out = vec![v[0]];
            for w in v.windows(2) {
                let n = ((w[1] - w[0]) / m).ceil().max(1.0) as usize;
                for k in 1..=n {
                    out.push(w[0] + (w[1] - w[0]) * k as f64 / n as f64);
                }
            }
            out
        };
        let (re, ze) = (fine(rl), fine(zl));
        let (nr, nz) = (re.len() - 1, ze.len() - 1);
        // Classify each cell from samples of the assembled geometry.
        let mut regions: Vec<(String, RegionRecipe)> = Vec::new();
        let mut kinds: Vec<CellKind> = Vec::with_capacity(nr * nz);
        let (mut bed_s, mut bed_n, mut dum_s, mut dum_n) = (0usize, 0usize, 0usize, 0usize);
        for j in 0..nz {
            for i in 0..nr {
                let counts = sample_cell(core, &frame, [re[i], re[i + 1]], [ze[j], ze[j + 1]]);
                let (rc, zc) = (0.5 * (re[i] + re[i + 1]), 0.5 * (ze[j] + ze[j + 1]));
                let z_mc = frame.z_to_mc(Frame::Tecdoc, Length::new::<centimeter>(zc)).get::<centimeter>();
                let solids: usize = counts.iter().enumerate().filter(|(s, _)| *s != mat::HELIUM && *s < counts.len() - 1).map(|(_, c)| c).sum();
                let total: usize = counts.iter().sum();
                let kind = if rc < core.bed_radius && z_mc.abs() < core.bed_half_height {
                    bed_s += solids;
                    bed_n += total;
                    CellKind::Bed
                } else if counts[mat::GRAPHITE] > 0 {
                    dum_s += solids;
                    dum_n += total;
                    CellKind::DummyBed
                } else {
                    // Majority slot with constants; helium or void -> cavity.
                    let best = (0..counts.len())
                        .filter(|s| counts[*s] > 0 && (*s == counts.len() - 1 || *s == mat::HELIUM || present.get(*s).copied().unwrap_or(false)))
                        .max_by_key(|s| counts[*s]);
                    match best {
                        Some(s) if s != mat::HELIUM && s != counts.len() - 1 => CellKind::Slot(s),
                        _ => CellKind::Cavity,
                    }
                };
                kinds.push(kind);
            }
        }
        let f_bed = if bed_n > 0 { bed_s as f64 / bed_n as f64 } else { 0.61 };
        let f_dum = if dum_n > 0 { dum_s as f64 / dum_n as f64 } else { f_bed };
        let mut zone_of = |k: CellKind| -> usize {
            let (name, recipe) = match k {
                CellKind::Bed => ("bed".to_string(), RegionRecipe::Bed { solid_fraction: f_bed }),
                CellKind::DummyBed => ("dummyBed".to_string(), RegionRecipe::DummyBed { solid_fraction: f_dum }),
                CellKind::Cavity => ("cavity".to_string(), RegionRecipe::Cavity),
                CellKind::Slot(s) => (format!("slot{s}"), RegionRecipe::Slot(s)),
            };
            if let Some(p) = regions.iter().position(|(n, _)| *n == name) {
                p
            } else {
                regions.push((name, recipe));
                regions.len() - 1
            }
        };
        let zone_of_cell: Vec<usize> = kinds.iter().map(|k| zone_of(*k)).collect();
        let m = |v: &[f64]| -> Result<AxisEdges, MgxsGenfoamError> {
            let l = |x: f64| Length::new::<centimeter>(x);
            let segs: Vec<(Length, usize)> = v[1..].iter().map(|x| (l(*x), 1)).collect();
            AxisEdges::segments(l(v[0]), &segs).map_err(|e| MgxsGenfoamError::Mesh(e.to_string()))
        };
        let zc = zone_of_cell.clone();
        let (re2, ze2) = (re.clone(), ze.clone());
        let mesh = RzMesh::build(
            RzGeometry::Axisymmetric { wedge_angle: Angle::new::<radian>(2.0 * std::f64::consts::PI) },
            m(&re)?,
            m(&ze)?,
            regions.len(),
            move |r, z| {
                let (r, z) = (r.get::<centimeter>(), z.get::<centimeter>());
                let i = re2.partition_point(|e| *e <= r).saturating_sub(1).min(nr - 1);
                let j = ze2.partition_point(|e| *e <= z).saturating_sub(1).min(nz - 1);
                zc[i + nr * j]
            },
        )
        .map_err(|e| MgxsGenfoamError::Mesh(e.to_string()))?;
        // The mesh's own regions must be the classification's.
        for c in 0..mesh.n_cells() {
            if mesh.cell_region(c) != zone_of_cell[c] {
                return Err(MgxsGenfoamError::Mesh(format!("cell {c}: region lookup disagrees")));
            }
        }
        Ok(Self {
            mesh,
            zone_of_cell,
            regions,
            frame,
            solid_fractions: (f_bed, f_dum),
        })
    }

    /// Cells of the mesh.
    #[must_use]
    pub fn n_cells(&self) -> usize {
        self.mesh.n_cells()
    }

    /// The flux boundary conditions, one per patch: zero gradient on the
    /// axis and the wedges; the outer radius, top and bottom get the Marshak
    /// vacuum albedo in [`Self::solve_diffusion`].
    #[must_use]
    pub fn flux_boundary(&self) -> Vec<BoundaryCondition<f64>> {
        vec![BoundaryCondition::ZeroGradient; self.mesh.mesh().patches.len()]
    }

    /// **The diffusion `k` and power shape at per-cell fuel temperatures**
    /// (`t_fuel`, one per cell; GeN-Foam evaluates every cell's constants at
    /// its own `TFuel`, `DiffusionNeutronics::new_with_cell_parameters`).
    /// Marshak vacuum (`gamma = 0.5`) on the outer, top and bottom patches.
    ///
    /// # Errors
    /// The constants do not build, a temperature count differs from the
    /// cells', or the solve fails.
    pub fn solve_diffusion(
        &self,
        nd: &NuclearDataInput,
        t_fuel: &[ThermodynamicTemperature],
    ) -> Result<RzDiffusionSolution, MgxsGenfoamError> {
        if t_fuel.len() != self.n_cells() {
            return Err(MgxsGenfoamError::Diffusion(format!("{} temperatures for {} cells", t_fuel.len(), self.n_cells())));
        }
        let xs = CrossSectionData::from_input(nd).map_err(|e| MgxsGenfoamError::Diffusion(format!("{e:?}")))?;
        let params: Vec<f64> = if xs.variable_names().is_empty() {
            Vec::new()
        } else {
            t_fuel.iter().map(|t| t.get::<kelvin>()).collect()
        };
        let mut dn = if params.is_empty() {
            DiffusionNeutronics::new(Arc::clone(self.mesh.mesh()), &xs, &self.zone_of_cell, &[], &self.flux_boundary(), DiffusionSettings::default())
        } else {
            DiffusionNeutronics::new_with_cell_parameters(
                Arc::clone(self.mesh.mesh()),
                &xs,
                &self.zone_of_cell,
                &params,
                &self.flux_boundary(),
                DiffusionSettings::default(),
            )
        }
        .map_err(|e| MgxsGenfoamError::Diffusion(format!("{e:?}")))?;
        for p in [RzPatch::Outer, RzPatch::Bottom, RzPatch::Top] {
            dn.set_albedo_boundary(p.index(), 0.5, AlbedoLinearisation::CellValue);
        }
        let report = dn.solve_eigenvalue().map_err(|e| MgxsGenfoamError::Diffusion(format!("{e:?}")))?;
        Ok(RzDiffusionSolution {
            k_eff: report.k_eff,
            outer_iterations: report.outer_iterations,
            converged: report.converged,
            power_density: dn.state().power_density().internal.as_slice().to_vec(),
        })
    }

    /// **Draw the mesh's regions** (R-Z, TECDOC frame: the top of the picture
    /// is the physical top), each cell coloured by its region, with a legend
    /// naming the region and its recipe, at `px_per_cm`.
    ///
    /// # Errors
    /// The file cannot be written.
    pub fn write_png(&self, path: impl AsRef<Path>, px_per_cm: f64) -> std::io::Result<()> {
        let (re, ze) = (self.mesh.r_edges(), self.mesh.z_edges());
        let (nr, nz) = (re.n_cells(), ze.n_cells());
        let cm = |l: Length| l.get::<centimeter>();
        let (r1, z0, z1) = (cm(re.edge(nr)), cm(ze.edge(0)), cm(ze.edge(nz)));
        let w = (r1 * px_per_cm).round() as usize;
        let h = ((z1 - z0) * px_per_cm).round() as usize;
        let palette = |k: usize| -> Rgb {
            let base = [
                (200, 40, 40), (90, 90, 90), (235, 235, 255), (210, 180, 120), (120, 60, 160), (60, 150, 60),
                (40, 110, 200), (230, 140, 40), (150, 200, 200), (200, 120, 170), (120, 120, 40), (40, 40, 120),
            ];
            let (r, g, b) = base[k % base.len()];
            let s = 1.0 - 0.25 * (k / base.len()) as f64;
            Rgb::new((r as f64 * s) as u8, (g as f64 * s) as u8, (b as f64 * s) as u8)
        };
        let mut img = ImageData::filled(w.max(1), h.max(1), Rgb::new(255, 255, 255));
        for py in 0..h {
            let z = z0 + (py as f64 + 0.5) / px_per_cm;
            let j = (0..nz).find(|j| z <= cm(ze.edge(j + 1))).unwrap_or(nz - 1);
            for px in 0..w {
                let r = (px as f64 + 0.5) / px_per_cm;
                let i = (0..nr).find(|i| r <= cm(re.edge(i + 1))).unwrap_or(nr - 1);
                let on_edge = (r - cm(re.edge(i))) * px_per_cm < 1.0 || (z - cm(ze.edge(j))) * px_per_cm < 1.0;
                let c = palette(self.zone_of_cell[i + nr * j]);
                img.set(px, py, if on_edge { Rgb::new((c.r as f64 * 0.7) as u8, (c.g as f64 * 0.7) as u8, (c.b as f64 * 0.7) as u8) } else { c });
            }
        }
        let legend: Vec<LegendEntry> = self
            .regions
            .iter()
            .enumerate()
            .map(|(k, (n, r))| {
                let what = match r {
                    RegionRecipe::Bed { solid_fraction } => format!("BED, SOLIDS {solid_fraction:.3}"),
                    RegionRecipe::DummyBed { solid_fraction } => format!("CONE/TUBE BALLS {solid_fraction:.3}"),
                    RegionRecipe::Cavity => "HELIUM".into(),
                    RegionRecipe::Slot(s) => format!("MC MATERIAL SLOT {s}"),
                };
                LegendEntry::new(palette(k), format!("{} {}", n.to_uppercase(), what))
            })
            .chain(std::iter::once(LegendEntry::new(
                Rgb::new(0, 0, 0),
                format!("R 0-{r1:.0} CM, Z {z0:.0}-{z1:.0} CM DOWN"),
            )))
            .collect();
        annotate_image(&img, "HTR-10 R-Z NEUTRONICS ZONES (FROM THE MC GEOMETRY)", &legend).write_png(path)
    }
}

/// A diffusion eigenvalue on the R-Z mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct RzDiffusionSolution {
    /// `k_eff` (dimensionless).
    pub k_eff: f64,
    /// Outer iterations.
    pub outer_iterations: usize,
    /// Whether the outer loop converged.
    pub converged: bool,
    /// Power density per cell, W/m^3 at GeN-Foam's arbitrary normalisation
    /// (scale to a total power as needed).
    pub power_density: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CellKind {
    Bed,
    DummyBed,
    Cavity,
    Slot(usize),
}

/// Material slot counts of sample points over the ring cell `r x z_tecdoc`
/// (cm); the last entry counts void / lost points.
fn sample_cell(core: &AssembledCore, frame: &FrameMap, r: [f64; 2], z: [f64; 2]) -> Vec<usize> {
    let n_slots = mat::COUNT;
    let mut counts = vec![0usize; n_slots + 1];
    let u = Direction::new(0.0, 0.0, 1.0);
    let (nr, nz, nt) = (6, 6, 8);
    for a in 0..nr {
        // Equal-area radial samples.
        let f = (a as f64 + 0.5) / nr as f64;
        let rr = (r[0] * r[0] + f * (r[1] * r[1] - r[0] * r[0])).sqrt();
        for b in 0..nz {
            let zt = z[0] + (b as f64 + 0.5) / nz as f64 * (z[1] - z[0]);
            let zm = frame.z_to_mc(Frame::Tecdoc, Length::new::<centimeter>(zt)).get::<centimeter>();
            for t in 0..nt {
                // An irrational offset so the azimuths do not align with the lattice.
                let th = (t as f64 + 0.318) * 2.0 * std::f64::consts::PI / nt as f64 + 0.1 * a as f64;
                let (s, c) = th.r_sin_cos();
                let m = core
                    .geometry
                    .locate(Position::new(rr * c, rr * s, zm), u, SurfaceToken::NONE)
                    .and_then(|p| p.material);
                match m {
                    Some(k) if k < n_slots => counts[k] += 1,
                    _ => counts[n_slots] += 1,
                }
            }
        }
    }
    counts
}

#[cfg(test)]
#[path = "mgxs_genfoam_tests.rs"]
mod tests;
