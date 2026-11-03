// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK.

//! # `htr10_coupling` — HTR-10 Monte Carlo iterated directly against its thermal-hydraulics
//!
//! gh:#1186 (epic gh:#1183). The whole-core HTR-10 Monte Carlo model
//! ([`ParametricStudy`], the one the web demo and `htr10_parametric` use)
//! iterated by Picard against a thermal-hydraulic target until the zone
//! temperatures and `k` stop moving. The neutronics is the Monte Carlo itself;
//! no multigroup constants are involved (that chain is [`crate::mgxs_genfoam`]).
//!
//! ## The shortest thing that works
//!
//! ```no_run
//! use nee_soon::direct_coupling::Convergence;
//! use nee_soon::htr10_coupling::{Htr10DirectCoupling, ThermalTarget};
//! use nee_soon::htr10_rmc::parametric::{ParametricStudy, StudyBed};
//! use uom::si::f64::Power;
//! use uom::si::power::megawatt;
//!
//! let study = ParametricStudy { bed: StudyBed::Seker { layers: 17 }, ..ParametricStudy::default() };
//! let target = ThermalTarget::htr10_rz_fpic().unwrap(); // the TECDOC-1694 R-Z core at FPIC
//! let mut coupling = Htr10DirectCoupling::new(study, target, Power::new::<megawatt>(10.0));
//! let state = coupling.solve(Convergence::default(), |line| println!("{line}")).unwrap();
//! println!("k = {:?} at fuel {:?}", state.k_eff, state.zone_temperatures.get(
//!     nee_soon::htr10_rmc::zones::Htr10Zone::FuelPebble));
//! ```
//!
//! ## One Picard iteration
//!
//! ```text
//!   1. Monte Carlo at the current zone temperatures, with an R-Z KappaFission
//!      tally over the bed (McPowerTally::rz_mesh)              -> k, power shape
//!   2. power_shape: the tally as an R-Z table, put on the TH cells, the
//!      total power conserved to round-off
//!   3. the TH solve (lumped node or the htr10_rz core's steady state)
//!   4. power-weighted fuel and moderator temperatures, volume-averaged
//!      reflector / brick / rod temperatures, the mean gas    -> ZoneTemperatures
//!   5. under-relax (omega), round to the temperature quantum, and
//!      REPROCESS the nuclear data EXACTLY at the new temperatures, every
//!      (nuclide, temperature) job cached so a temperature reached again is
//!      not reprocessed; the bed majorant is rebuilt from the new nuclides,
//!      so it bounds them
//! ```
//!
//! **Why reprocessing and not `Material.temperature`.** HTR-10's pointwise
//! (NJOY) nuclides carry their data temperature; the transport does not
//! re-broaden them, so setting a material's temperature alone moves only the
//! free-gas kinematics and leaves the Doppler broadening and the `S(alpha,
//! beta)` laws where they were: a silent no-op for the feedback. Every
//! temperature here goes through [`ParametricStudy::plan`], which processes
//! each (nuclide, temperature) exactly and generates each bound law by LEAPR
//! at its zone's temperature.
//!
//! ## Module map
//!
//! | item | what |
//! |---|---|
//! | [`Htr10DirectCoupling`] | the coupling: build, set relaxation / quantum / threads / power mesh, [`Htr10DirectCoupling::solve`] |
//! | [`ThermalTarget`] | the TH side, a closed enum: GeN-Foam's [`LumpedThermal`] node, or the `htr10_rz` R-Z core ([`RzTarget`]) |
//! | [`PicardIteration`], [`CoupledHtr10State`] | the history (k +/- sigma, zone temperatures in / from the TH / out, residuals, costs) and the result |
//! | [`write_history_png`] | the history as a picture |
//! | [`zone_temperatures_from_rz`], [`relax_and_quantise`] | the pieces, public so they can be checked alone |
//!
//! ## Known limitations, stated rather than discovered
//!
//! - **Statistical noise is in the loop.** Each iteration's `k` and shape
//!   carry Monte Carlo noise; a `k` tolerance below the run's sigma will not
//!   be met except by chance ([`Convergence::k_eff`]).
//! - **The frames are tied at the bed floor** ([`crate::power_shape::FrameMap`]):
//!   a Monte Carlo loading shorter than the TECDOC full core (N < 18)
//!   heats only the lower part of the R-Z bed, and the R-Z model's flow and
//!   heat capacity are the full core's.
//! - **The Monte Carlo geometry is cold**: no thermal expansion; the helium
//!   density follows the coolant temperature at the model's pressure.
//! - **The conus and discharge tube hold no fuel** in the Monte Carlo model,
//!   so the tally covers the cylindrical bed only.
//!
//! Status: untrusted AI-generated draft (2026-11-03); research, education and
//! V&V only, never operation or safety (`RESPONSIBLE_USE.md`).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use njoy_outram_park_fork::endf::tape::Tape as EndfTape;
use outram_foam_appbuilder_lib::genfoam::multi_region::coupling_fields::CouplingRegion;
use outram_foam_appbuilder_lib::genfoam::multi_region::outer_iteration::{
    lumped_cell_mesh, LumpedThermal, RegionKernel,
};
use outram_foam_appbuilder_lib::htr10_rz::geometry::idx;
use outram_foam_appbuilder_lib::htr10_rz::power_map::PowerMapCell;
use outram_foam_appbuilder_lib::htr10_rz::{
    Htr10RzError, Htr10RzOptions, Htr10RzTransientCore, OperatingPoint, PowerShape as RzShape,
    TransientCoreOptions,
};
use outram_foam_basic_lib::prelude::{BoundaryCondition, Field, PatchField, VolScalarField};
use outram_mc_libs::geometry::plot::{annotate_image, ImageData, LegendEntry, Rgb};
use outram_mc_libs::material::nuclide::Nuclide;
use uom::si::f64::{Length, Power, Ratio, TemperatureInterval, ThermodynamicTemperature, Time, Volume};
use uom::si::length::{centimeter, meter};
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::temperature_interval;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volume::cubic_meter;
use uom::si::volumetric_power_density::watt_per_cubic_meter;

use crate::direct_coupling::Convergence;
use crate::htr10_rmc::data::{Htr10DataError, Tape};
use crate::htr10_rmc::data_jobs::{JobProduct, TemperedJob};
use crate::htr10_rmc::parametric::{ParametricStudy, StudyPlan};
use crate::htr10_rmc::zones::{max_temperature, min_temperature, Htr10Zone, ZoneTemperatures};
use crate::power_shape::{
    line_plot, CellGeometry, FrameMap, McPowerTally, PowerCells, PowerShape, TECDOC_BED_FLOOR_Z_CM,
};

/// Why the coupled solve failed.
#[derive(Debug, thiserror::Error)]
pub enum Htr10CouplingError {
    /// The cap was reached; the residuals say whether it was close.
    #[error(
        "the HTR-10 coupling did not converge in {iterations} iterations: last max |dT| = {last_dt_k:.3} K, last |dk| = {last_dk:.6}"
    )]
    NotConverged {
        /// Iterations performed.
        iterations: usize,
        /// Last largest zone-temperature residual, K.
        last_dt_k: f64,
        /// Last `k` change.
        last_dk: f64,
        /// The history so far.
        history: Vec<PicardIteration>,
    },
    /// The Monte Carlo side failed (settings, data, transport).
    #[error("Monte Carlo: {0}")]
    MonteCarlo(String),
    /// The power could not be put on the TH cells.
    #[error("power shape: {0}")]
    Power(#[from] crate::power_shape::PowerShapeError),
    /// The thermal-hydraulic side failed.
    #[error("thermal-hydraulics: {0}")]
    Thermal(String),
}

impl From<Htr10RzError> for Htr10CouplingError {
    fn from(e: Htr10RzError) -> Self {
        Self::Thermal(e.to_string())
    }
}

/// The `htr10_rz` R-Z core as a TH target, with its operating point.
#[derive(Debug, Clone)]
pub struct RzTarget {
    /// The R-Z core (its power map is replaced every iteration).
    pub core: Htr10RzTransientCore,
    /// Inlet temperature, pressure and flow; its `power` is overwritten by
    /// the coupling's.
    pub operating_point: OperatingPoint,
}

/// The thermal-hydraulic side of the coupling: a closed set.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)] // one value per coupling; no Box by the workspace rule
pub enum ThermalTarget {
    /// GeN-Foam's lumped node ([`LumpedThermal`]), marched in pseudo-time to
    /// its steady state each iteration. Its one temperature is given to the
    /// fuel and the moderator pebbles; the other zones keep theirs.
    Lumped {
        /// The node.
        thermal: LumpedThermal,
        /// The heated volume (positive).
        volume: Volume,
        /// Pseudo time step of the march (positive).
        time_step: Time,
    },
    /// The TECDOC-1694 R-Z core ([`Htr10RzTransientCore`]): the Monte Carlo
    /// shape is put on its heated bed cells and its steady state solved.
    Rz(RzTarget),
}

impl ThermalTarget {
    /// The R-Z core at the FPIC specification (10 MW, 250 °C, 3 MPa,
    /// 4.32 kg/s), on the 20 cm mesh of the LOFC base options with the FPIC
    /// Tab 2.6 shape (replaced by the Monte Carlo's at the first iteration).
    ///
    /// # Errors
    /// The core cannot be built.
    pub fn htr10_rz_fpic() -> Result<Self, Htr10RzError> {
        let options = TransientCoreOptions { shape: RzShape::FpicTab26, ..TransientCoreOptions::htr10_lofc() };
        Ok(Self::Rz(RzTarget {
            core: Htr10RzTransientCore::build(options)?,
            operating_point: OperatingPoint::fpic_specification(),
        }))
    }

    /// As [`Self::htr10_rz_fpic`] on the coarse Fig 2.5 mesh (21 x 39
    /// cells, no refinement), for tests and quick looks.
    ///
    /// # Errors
    /// The core cannot be built.
    pub fn htr10_rz_fpic_coarse() -> Result<Self, Htr10RzError> {
        let mut options = TransientCoreOptions { shape: RzShape::FpicTab26, ..TransientCoreOptions::htr10_lofc() };
        options.solid = Htr10RzOptions { max_cell: None, ..options.solid };
        Ok(Self::Rz(RzTarget {
            core: Htr10RzTransientCore::build(options)?,
            operating_point: OperatingPoint::fpic_specification(),
        }))
    }
}

/// One Picard iteration, kept so the approach can be seen.
#[derive(Debug, Clone, PartialEq)]
pub struct PicardIteration {
    /// 1-based.
    pub iteration: usize,
    /// The zone temperatures the Monte Carlo ran at.
    pub zone_temperatures_in: ZoneTemperatures,
    /// `k_eff`.
    pub k_eff: Ratio,
    /// Its one-sigma statistical uncertainty.
    pub k_std: Ratio,
    /// The zone temperatures the TH solve returned (unrelaxed).
    pub zone_temperatures_th: ZoneTemperatures,
    /// The next iteration's (relaxed, quantised).
    pub zone_temperatures_out: ZoneTemperatures,
    /// Fixed-point residual: the largest `|T_th - T_in|` over the zones.
    pub temperature_residual: TemperatureInterval,
    /// `|k - k_previous|` (`None` at the first iteration).
    pub k_change: Option<Ratio>,
    /// Peak over mean power density of the TH cells.
    pub peaking: Ratio,
    /// Collisions above the majorant (must be zero).
    pub majorant_violations: u64,
    /// Processing jobs run this iteration.
    pub jobs_processed: usize,
    /// Jobs taken from the cache.
    pub jobs_cached: usize,
    /// Wall time processing nuclear data.
    pub processing_time: Time,
    /// Wall time transporting.
    pub transport_time: Time,
    /// Wall time of the TH solve.
    pub thermal_time: Time,
}

/// A converged coupled state.
#[derive(Debug, Clone)]
pub struct CoupledHtr10State {
    /// The zone temperatures (the last Monte Carlo ran at these).
    pub zone_temperatures: ZoneTemperatures,
    /// `k_eff` of the last iteration.
    pub k_eff: Ratio,
    /// Its one-sigma uncertainty.
    pub k_std: Ratio,
    /// The last measured power shape (Monte Carlo frame).
    pub power_shape: PowerShape,
    /// Every iteration.
    pub history: Vec<PicardIteration>,
}

/// **The HTR-10 Monte Carlo iterated against a thermal-hydraulic target.**
///
/// Build with [`Self::new`], adjust with the `with_*` builders, then
/// [`Self::solve`]. See the module docs for the loop.
pub struct Htr10DirectCoupling {
    study: ParametricStudy,
    target: ThermalTarget,
    power: Power,
    relaxation: Ratio,
    quantum: TemperatureInterval,
    threads: usize,
    mesh: Option<(Vec<Length>, Vec<Length>)>,
    cache: Vec<(TemperedJob, JobProduct)>,
}

impl Htr10DirectCoupling {
    /// The coupling of `study` (its temperatures are the starting guess) to
    /// `target` at thermal power `power` (positive).
    ///
    /// Defaults: relaxation `0.5`, temperature quantum 1 K, 4 threads, the
    /// power mesh of [`Self::default_power_mesh`].
    #[must_use]
    pub fn new(study: ParametricStudy, target: ThermalTarget, power: Power) -> Self {
        Self {
            study,
            target,
            power,
            relaxation: Ratio::new::<ratio>(0.5),
            quantum: TemperatureInterval::new::<temperature_interval::kelvin>(1.0),
            threads: 4,
            mesh: None,
            cache: Vec::new(),
        }
    }

    /// Under-relaxation `omega` in `(0, 1]`: `T_next = T_in + omega (T_th -
    /// T_in)`. `1` is plain Picard.
    #[must_use]
    pub fn with_relaxation(mut self, omega: Ratio) -> Self {
        self.relaxation = omega;
        self
    }

    /// Round every zone temperature to a multiple of `quantum` (positive)
    /// before reprocessing, so temperatures reached again hit the cache. The
    /// data are then processed EXACTLY at the rounded temperature, so the
    /// transport is never at a temperature other than its data's; the
    /// quantum only discretises the feedback. Keep it well below the
    /// convergence tolerance.
    #[must_use]
    pub fn with_temperature_quantum(mut self, quantum: TemperatureInterval) -> Self {
        self.quantum = quantum;
        self
    }

    /// No rounding: every Picard temperature is processed as it comes (the
    /// cache then helps only when a temperature repeats exactly).
    #[must_use]
    pub fn without_temperature_quantum(mut self) -> Self {
        self.quantum = TemperatureInterval::new::<temperature_interval::kelvin>(0.0);
        self
    }

    /// Threads for processing and transport (at least one).
    #[must_use]
    pub fn with_threads(mut self, threads: usize) -> Self {
        self.threads = threads.max(1);
        self
    }

    /// The Monte Carlo power mesh: ring edges and axial edges (Monte Carlo
    /// frame), both ascending.
    #[must_use]
    pub fn with_power_mesh(mut self, r_edges: Vec<Length>, z_edges: Vec<Length>) -> Self {
        self.mesh = Some((r_edges, z_edges));
        self
    }

    /// The default power mesh of a bed of half-height `h`: nine 10 cm rings
    /// to the 90 cm bed wall, and the cylindrical bed `-h..h` in slices of
    /// about 10 cm (the conus holds no fuel, module docs).
    #[must_use]
    pub fn default_power_mesh(h: Length) -> (Vec<Length>, Vec<Length>) {
        let cm = Length::new::<centimeter>;
        let hc = h.get::<centimeter>();
        let nz = ((2.0 * hc / 10.0).round() as usize).max(1);
        (
            (0..=9).map(|i| cm(10.0 * i as f64)).collect(),
            (0..=nz).map(|k| cm(-hc + 2.0 * hc * k as f64 / nz as f64)).collect(),
        )
    }

    /// The study (its temperatures are the last ones run).
    #[must_use]
    pub fn study(&self) -> &ParametricStudy {
        &self.study
    }

    /// The thermal target in its current state.
    #[must_use]
    pub fn target(&self) -> &ThermalTarget {
        &self.target
    }

    /// Processed (nuclide, temperature) jobs held in the cache.
    #[must_use]
    pub fn cached_jobs(&self) -> usize {
        self.cache.len()
    }

    /// **Iterate to a coupled steady state.** `progress` gets a line per
    /// stage.
    ///
    /// # Errors
    /// [`Htr10CouplingError::NotConverged`] at the cap (with the history);
    /// the other variants when a side fails.
    pub fn solve(
        &mut self,
        criteria: Convergence,
        mut progress: impl FnMut(&str),
    ) -> Result<CoupledHtr10State, Htr10CouplingError> {
        let omega = self.relaxation.get::<ratio>();
        if !(omega > 0.0 && omega <= 1.0) {
            return Err(Htr10CouplingError::MonteCarlo(format!("relaxation {omega} outside (0, 1]")));
        }
        let mut history: Vec<PicardIteration> = Vec::new();
        let tol_t = criteria.temperature.get::<temperature_interval::kelvin>();
        let tol_k = criteria.k_eff.get::<ratio>();
        let (mut last_dt, mut last_dk) = (f64::INFINITY, f64::INFINITY);
        self.study.temperatures = relax_and_quantise(
            &self.study.temperatures,
            &self.study.temperatures,
            Ratio::new::<ratio>(1.0),
            self.quantum,
        );
        for iteration in 1..=criteria.max_iterations {
            let t_in = self.study.temperatures;
            // 1. Monte Carlo with the power tally.
            let t0 = Instant::now();
            let plan = self.study.plan().map_err(|e| Htr10CouplingError::MonteCarlo(e.to_string()))?;
            let (nuclides, processed, cached) = self.nuclides(&plan, &mut progress)?;
            let t1 = Instant::now();
            let core = self.study.core(&plan).map_err(Htr10CouplingError::MonteCarlo)?;
            let frame = FrameMap::from_core(&core);
            let (r_edges, z_edges) = self
                .mesh
                .clone()
                .unwrap_or_else(|| Self::default_power_mesh(frame.bed_half_height));
            let mut tally = McPowerTally::rz_mesh(&r_edges, &z_edges)?;
            progress(&format!("iteration {iteration}: transport"));
            let mc = self
                .study
                .transport_with_tally(&plan, &nuclides, self.threads, Some(&mut tally))
                .map_err(Htr10CouplingError::MonteCarlo)?;
            let t2 = Instant::now();
            // 2. The measured shape.
            let shape = PowerShape::from_tally(&tally, self.study.statistics.active)?;
            // 3-4. TH and the zone temperatures it gives.
            let (t_th, peaking) = self.thermal(&shape, frame, &t_in)?;
            let t3 = Instant::now();
            // 5. Relax, quantise; the next iteration reprocesses.
            let t_out = relax_and_quantise(&t_in, &t_th, self.relaxation, self.quantum);
            let residual = max_difference(&t_in, &t_th);
            let k_change = history.last().map(|p| (mc.k_mean - p.k_eff.get::<ratio>()).abs());
            last_dt = residual;
            last_dk = k_change.unwrap_or(f64::INFINITY);
            let it = PicardIteration {
                iteration,
                zone_temperatures_in: t_in,
                k_eff: Ratio::new::<ratio>(mc.k_mean),
                k_std: Ratio::new::<ratio>(mc.k_std),
                zone_temperatures_th: t_th,
                zone_temperatures_out: t_out,
                temperature_residual: TemperatureInterval::new::<temperature_interval::kelvin>(residual),
                k_change: k_change.map(Ratio::new::<ratio>),
                peaking,
                majorant_violations: mc.majorant_violations,
                jobs_processed: processed,
                jobs_cached: cached,
                processing_time: Time::new::<second>((t1 - t0).as_secs_f64()),
                transport_time: Time::new::<second>((t2 - t1).as_secs_f64()),
                thermal_time: Time::new::<second>((t3 - t2).as_secs_f64()),
            };
            progress(&format!(
                "iteration {iteration}: k = {:.5} +/- {:.5}, fuel {:.1} K -> TH {:.1} K, max |dT| {:.2} K, {} jobs processed ({} cached), {:.0} s data + {:.0} s MC + {:.1} s TH",
                mc.k_mean,
                mc.k_std,
                t_in.kelvin(Htr10Zone::FuelPebble),
                t_th.kelvin(Htr10Zone::FuelPebble),
                residual,
                processed,
                cached,
                (t1 - t0).as_secs_f64(),
                (t2 - t1).as_secs_f64(),
                (t3 - t2).as_secs_f64()
            ));
            history.push(it);
            if residual < tol_t && last_dk < tol_k {
                return Ok(CoupledHtr10State {
                    zone_temperatures: t_in,
                    k_eff: Ratio::new::<ratio>(mc.k_mean),
                    k_std: Ratio::new::<ratio>(mc.k_std),
                    power_shape: shape,
                    history,
                });
            }
            self.study.temperatures = t_out;
        }
        Err(Htr10CouplingError::NotConverged {
            iterations: criteria.max_iterations,
            last_dt_k: last_dt,
            last_dk,
            history,
        })
    }

    /// The nuclide array of `plan`: cached jobs reused, the rest processed
    /// (in parallel) and cached. Returns `(nuclides, processed, cached)`.
    fn nuclides(
        &mut self,
        plan: &StudyPlan,
        progress: &mut impl FnMut(&str),
    ) -> Result<(Vec<Nuclide>, usize, usize), Htr10CouplingError> {
        let mc = |e: String| Htr10CouplingError::MonteCarlo(e);
        let hit: Vec<Option<usize>> = plan
            .jobs
            .iter()
            .map(|j| self.cache.iter().position(|(c, _)| job_key_eq(c, j)))
            .collect();
        let missing: Vec<usize> = (0..plan.jobs.len()).filter(|i| hit[*i].is_none()).collect();
        let read = |t: &Tape| {
            EndfTape::read_file(&t.path()).map_err(|e| Htr10DataError::InvalidConfig(format!("{}: {e}", t.file)))
        };
        let lines: Mutex<Vec<String>> = Mutex::new(Vec::new());
        let products: Mutex<Vec<Option<Result<JobProduct, String>>>> =
            Mutex::new((0..missing.len()).map(|_| None).collect());
        let next = AtomicUsize::new(0);
        let study = &self.study;
        std::thread::scope(|s| {
            for _ in 0..self.threads.min(missing.len()).max(1) {
                s.spawn(|| loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    if k >= missing.len() {
                        break;
                    }
                    let i = missing[k];
                    let t0 = Instant::now();
                    let r = study
                        .job_tape(plan, i)
                        .and_then(|t| t.map(|t| read(&t)).transpose())
                        .and_then(|t| study.process_job(plan, i, t.as_ref()))
                        .map_err(|e| e.to_string());
                    if let Ok(mut l) = lines.lock() {
                        l.push(format!("  processed {} in {:.1} s", plan.jobs[i].label(), t0.elapsed().as_secs_f64()));
                    }
                    if let Ok(mut p) = products.lock() {
                        p[k] = Some(r);
                    }
                });
            }
        });
        for l in lines.into_inner().unwrap_or_default() {
            progress(&l);
        }
        let products = products.into_inner().map_err(|_| mc("a processing thread panicked".into()))?;
        for (k, p) in products.into_iter().enumerate() {
            let p = p.ok_or_else(|| mc(format!("job {} did not run", missing[k])))?.map_err(mc)?;
            self.cache.push((plan.jobs[missing[k]].clone(), p));
        }
        let cache = &self.cache;
        let nuclides = self
            .study
            .assemble_nuclides(
                plan,
                |i| {
                    cache
                        .iter()
                        .find(|(c, _)| job_key_eq(c, &plan.jobs[i]))
                        .map(|(_, p)| p.clone())
                        .ok_or_else(|| Htr10DataError::InvalidConfig(format!("job {i} not cached")))
                },
                read,
            )
            .map_err(|e| mc(e.to_string()))?;
        Ok((nuclides, missing.len(), plan.jobs.len() - missing.len()))
    }

    /// Put the shape on the target, solve it, read the zone temperatures.
    /// Returns them and the peaking of the TH cells' power density.
    fn thermal(
        &mut self,
        shape: &PowerShape,
        frame: FrameMap,
        t_in: &ZoneTemperatures,
    ) -> Result<(ZoneTemperatures, Ratio), Htr10CouplingError> {
        match &mut self.target {
            ThermalTarget::Lumped {
                thermal,
                volume,
                time_step,
            } => {
                let q = self.power.get::<watt>() / volume.get::<cubic_meter>();
                let mut region = lumped_region(volume.get::<cubic_meter>());
                if let Some(f) = region.scalar_mut("powerDensityNeutronics") {
                    for v in f.internal.as_mut_slice() {
                        *v = q;
                    }
                }
                let mut prev = f64::INFINITY;
                for _ in 0..100_000 {
                    thermal.begin_step();
                    thermal
                        .correct(&mut region, *time_step, false)
                        .map_err(|e| Htr10CouplingError::Thermal(format!("{e:?}")))?;
                    let t = thermal.fuel_temperature().get::<kelvin>();
                    if (t - prev).abs() < 1e-9 * t.abs().max(1.0) {
                        break;
                    }
                    prev = t;
                }
                let t = thermal.fuel_temperature();
                let out = t_in
                    .with(Htr10Zone::FuelPebble, t)
                    .with(Htr10Zone::ModeratorPebble, t);
                Ok((out, Ratio::new::<ratio>(1.0)))
            }
            ThermalTarget::Rz(target) => {
                let mesh = target.core.solid().field().mesh();
                let heated = target.core.heated_cells().to_vec();
                let mut is_heated = vec![false; mesh.n_cells()];
                for c in &heated {
                    is_heated[*c] = true;
                }
                let cells = PowerCells::from_rz_mesh(mesh, frame, |c| is_heated[c]);
                let q = shape.cell_power_density(&cells, self.power)?;
                let map = rz_power_map(&cells, &q);
                let peaking = peaking_of(&cells, &q);
                target.core.set_power_map(&map)?;
                let op = OperatingPoint {
                    power: self.power,
                    ..target.operating_point
                };
                let report = target.core.solve_steady(&op)?;
                let mut out = zone_temperatures_from_rz(&target.core, t_in);
                let gas = 0.5 * (op.inlet.get::<kelvin>() + report.outlet_temperature.get::<kelvin>());
                out.set(Htr10Zone::Coolant, ThermodynamicTemperature::new::<kelvin>(gas));
                Ok((out, peaking))
            }
        }
    }
}

/// Two tempered jobs are the same work when the job and the temperature
/// are equal to the bit.
fn job_key_eq(a: &TemperedJob, b: &TemperedJob) -> bool {
    a.job == b.job && a.kelvin().to_bits() == b.kelvin().to_bits()
}

/// A one-cell coupling region carrying `powerDensityNeutronics` and
/// `TFuel`, as [`crate::direct_coupling`] builds it.
fn lumped_region(volume_m3: f64) -> CouplingRegion {
    let mesh = lumped_cell_mesh(volume_m3);
    let mut region = CouplingRegion::new("core", Arc::clone(&mesh));
    for name in ["powerDensityNeutronics", "TFuel"] {
        let boundary = mesh
            .patches
            .iter()
            .map(|p| PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::uniform(p.size, 0.0),
            })
            .collect();
        region.insert_scalar(VolScalarField::new(name, Arc::clone(&mesh), Field::uniform(mesh.n_cells, 0.0), boundary));
    }
    region
}

/// The cell densities as an `htr10_rz` power map: one map cell per ring
/// cell, TECDOC cm and W/cm^3 (only ring cells are mapped).
fn rz_power_map(cells: &PowerCells, q: &[crate::power_shape::PowerDensity]) -> Vec<PowerMapCell> {
    cells
        .cells
        .iter()
        .zip(q)
        .filter_map(|(c, q)| match c {
            CellGeometry::Annulus { r, z } => {
                let zc = |v: Length| v.get::<centimeter>();
                Some(PowerMapCell {
                    z_cm: [zc(z[0]).min(zc(z[1])), zc(z[0]).max(zc(z[1]))],
                    r_cm: [r[0].get::<centimeter>(), r[1].get::<centimeter>()],
                    q_w_per_cm3: q.get::<watt_per_cubic_meter>() * 1.0e-6,
                })
            }
            _ => None,
        })
        .collect()
}

/// Peak over volume-mean power density of the cells.
fn peaking_of(cells: &PowerCells, q: &[crate::power_shape::PowerDensity]) -> Ratio {
    let (mut p, mut v, mut peak) = (0.0, 0.0, 0.0f64);
    for (c, q) in cells.cells.iter().zip(q) {
        let vol = c.volume().get::<cubic_meter>();
        let qv = q.get::<watt_per_cubic_meter>();
        p += qv * vol;
        v += vol;
        peak = peak.max(qv);
    }
    Ratio::new::<ratio>(if p > 0.0 { peak * v / p } else { 0.0 })
}

/// Largest `|a - b|` over the zones, K.
fn max_difference(a: &ZoneTemperatures, b: &ZoneTemperatures) -> f64 {
    Htr10Zone::ALL
        .into_iter()
        .map(|z| (a.kelvin(z) - b.kelvin(z)).abs())
        .fold(0.0, f64::max)
}

/// **The next Picard temperatures**: `T_in + omega (T_th - T_in)` per zone,
/// rounded to the nearest multiple of `quantum` (none when the quantum is
/// zero) and kept inside the zones' range (20..=1800 °C).
#[must_use]
pub fn relax_and_quantise(
    t_in: &ZoneTemperatures,
    t_th: &ZoneTemperatures,
    omega: Ratio,
    quantum: TemperatureInterval,
) -> ZoneTemperatures {
    let w = omega.get::<ratio>();
    let dq = quantum.get::<temperature_interval::kelvin>();
    let (lo, hi) = (min_temperature().get::<kelvin>(), max_temperature().get::<kelvin>());
    let mut k = [0.0; 8];
    for z in Htr10Zone::ALL {
        let t = t_in.kelvin(z) + w * (t_th.kelvin(z) - t_in.kelvin(z));
        let t = if dq > 0.0 { (t / dq).round() * dq } else { t };
        k[z.index()] = t.clamp(lo, hi);
    }
    ZoneTemperatures::from_kelvin(k)
}

/// The `htr10_rz` region's Monte Carlo temperature zone, by its region and
/// TECDOC `z` (cm): graphite above the Monte Carlo cavity top (`z <
/// 180 - 221.818`) is the top reflector, below the bed floor (`z > 180`)
/// the bottom, else the side; the boronated bricks are their own zone.
/// Gas, steel and the bed are `None` (the bed's temperatures are
/// power-weighted separately; steel is outside the Monte Carlo model).
#[must_use]
pub fn rz_zone_of(region: usize, z_tecdoc_cm: f64) -> Option<Htr10Zone> {
    use idx::*;
    let cavity_top = TECDOC_BED_FLOOR_Z_CM - crate::htr10_rmc::core_model::HTR10_CORE_CAVITY_CM;
    match region {
        CARBON_BRICK => Some(Htr10Zone::BoronatedBricks),
        SIDE_GRAPHITE | TOP_BOTTOM_GRAPHITE | CR_RING | RISERS | SIDE_INNER_GRAPHITE | UPPER_BOTTOM_REFL
        | LOWER_BOTTOM_REFL | BOTTOM_REFL_SOLID | TOP_FLOW_ZONE | DISCHARGE_THROTTLE | CR_EXIT_THROTTLE => {
            Some(if z_tecdoc_cm < cavity_top {
                Htr10Zone::TopReflector
            } else if z_tecdoc_cm > TECDOC_BED_FLOOR_Z_CM {
                Htr10Zone::BottomReflector
            } else {
                Htr10Zone::SideReflector
            })
        }
        _ => None,
    }
}

/// **The Monte Carlo zone temperatures an R-Z core state gives**:
///
/// - fuel and moderator pebbles: the core's power-weighted feedback
///   temperatures ([`Htr10RzTransientCore::feedback`]);
/// - top, side and bottom reflector and the boronated bricks:
///   volume-weighted means of their cells ([`rz_zone_of`]);
/// - control rods: the volume mean of the control-rod channel ring
///   (`CR_RING`), where the withdrawn rods sit;
/// - the coolant is kept from `t_in` (the coupling sets it from the gas).
///
/// A zone with no cells keeps its `t_in` value.
#[must_use]
pub fn zone_temperatures_from_rz(core: &Htr10RzTransientCore, t_in: &ZoneTemperatures) -> ZoneTemperatures {
    let f = core.solid().field();
    let mesh = f.mesh();
    let vol = &mesh.mesh().cell_volumes;
    let mut sum = [0.0f64; 8];
    let mut w = [0.0f64; 8];
    for c in 0..mesh.n_cells() {
        let region = mesh.cell_region(c);
        let z = mesh.cell_centre(c).1.get::<meter>() * 100.0;
        let t = f.temperature(c).get::<kelvin>();
        if let Some(zone) = rz_zone_of(region, z) {
            sum[zone.index()] += vol[c] * t;
            w[zone.index()] += vol[c];
        }
        if region == idx::CR_RING {
            let k = Htr10Zone::ControlRods.index();
            sum[k] += vol[c] * t;
            w[k] += vol[c];
        }
    }
    let fb = core.feedback();
    let mut out = *t_in;
    out.set(Htr10Zone::FuelPebble, fb.fuel);
    out.set(Htr10Zone::ModeratorPebble, fb.moderator);
    for z in [
        Htr10Zone::TopReflector,
        Htr10Zone::SideReflector,
        Htr10Zone::BottomReflector,
        Htr10Zone::ControlRods,
        Htr10Zone::BoronatedBricks,
    ] {
        if w[z.index()] > 0.0 {
            out.set(z, ThermodynamicTemperature::new::<kelvin>(sum[z.index()] / w[z.index()]));
        }
    }
    out
}

/// **The history as a PNG**: `k` with its one-sigma bars against the
/// iteration (left), and the fuel, moderator, side-reflector and coolant
/// temperatures the Monte Carlo ran at (right).
///
/// # Errors
/// The file cannot be written.
pub fn write_history_png(history: &[PicardIteration], path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
    let x: Vec<f64> = history.iter().map(|h| h.iteration as f64).collect();
    let k: Vec<f64> = history.iter().map(|h| h.k_eff.get::<ratio>()).collect();
    let s: Vec<f64> = history.iter().map(|h| h.k_std.get::<ratio>()).collect();
    let red = Rgb::new(200, 30, 30);
    let left = line_plot(&[(red, x.clone(), k.clone(), s)], 360, 260);
    let zones = [
        (Htr10Zone::FuelPebble, Rgb::new(200, 30, 30)),
        (Htr10Zone::ModeratorPebble, Rgb::new(60, 60, 60)),
        (Htr10Zone::SideReflector, Rgb::new(190, 140, 40)),
        (Htr10Zone::Coolant, Rgb::new(40, 90, 200)),
    ];
    let series: Vec<(Rgb, Vec<f64>, Vec<f64>, Vec<f64>)> = zones
        .iter()
        .map(|(z, c)| {
            let mut xs = x.clone();
            let mut ys: Vec<f64> = history.iter().map(|h| h.zone_temperatures_in.kelvin(*z)).collect();
            // The last relaxed output, so the curve ends where the loop stopped.
            if let Some(last) = history.last() {
                xs.push(last.iteration as f64 + 1.0);
                ys.push(last.zone_temperatures_out.kelvin(*z));
            }
            (*c, xs, ys.clone(), vec![0.0; ys.len()])
        })
        .collect();
    let right = line_plot(&series, 360, 260);
    let mut both = ImageData::filled(left.width + right.width + 20, left.height, Rgb::new(255, 255, 255));
    for (img, x0) in [(&left, 0), (&right, left.width + 20)] {
        for yy in 0..img.height {
            for xx in 0..img.width {
                both.set(x0 + xx, yy, img.get(xx, yy));
            }
        }
    }
    let kmin = k.iter().copied().fold(f64::INFINITY, f64::min);
    let kmax = k.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let tmin = series.iter().flat_map(|s| s.2.iter().copied()).fold(f64::INFINITY, f64::min);
    let tmax = series.iter().flat_map(|s| s.2.iter().copied()).fold(f64::NEG_INFINITY, f64::max);
    let mut legend = vec![LegendEntry::new(red, format!("LEFT: K {kmin:.4}-{kmax:.4} +/- 1 SIGMA"))];
    for (z, c) in zones {
        legend.push(LegendEntry::new(c, format!("RIGHT: {} T", z.key().to_uppercase())));
    }
    legend.push(LegendEntry::new(Rgb::new(0, 0, 0), format!("RIGHT RANGE {tmin:.0}-{tmax:.0} K")));
    legend.push(LegendEntry::new(Rgb::new(0, 0, 0), format!("X: PICARD ITERATION 1-{}", history.len())));
    annotate_image(&both, "HTR-10 MC <-> TH PICARD HISTORY", &legend).write_png(path)
}

#[cfg(test)]
#[path = "htr10_coupling_tests.rs"]
mod tests;
