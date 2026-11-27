//! # MSRE on the multifidelity twin template (gh:#1748, epic gh:#1743)
//!
//! [`MsreTwin`] is the MSRE's [`MultiFidelityTwin`]: the plant is the
//! existing [`Session`] (TM-1070's equations, `MsreModel`), the Monte Carlo
//! case is the N-3 split the `msre_web` k_eff tab runs (gh:#1574), and
//! the template wires them as it wires every adopter. **No physics here and
//! nothing re-implemented:** this module adapts types.
//!
//! | Page | MSRE |
//! |---|---|
//! | Plant | [`MsrePlant`]: [`Session`], unchanged (pinned bit for bit) |
//! | Control volumes | the model's two core lumps (fuel salt, graphite) and the stream points it reports, from the live snapshot |
//! | Coarse / fine multiphysics | not built: see below |
//! | Monte Carlo | [`MsreSplit`]: `nee_soon::msre_n1`'s full core, runs A (1131 °F), C (salt +100 °F), D (graphite +100 °F), paired by [`coefficient_from_pair`] |
//!
//! **Coefficient sources.** Cited: ORNL-TM-1070 Table 1's 1965 predictions
//! ([`Coefficients::Ornl1965`], the session's default). Committed: this
//! workspace's MSRE N-3 record ([`Coefficients::MonteCarloN3`]). Live: a
//! split run in this session ([`Coefficients::Measured`]), which reaches the
//! plant only through `send_to_plant`.
//!
//! **Multiphysics: not built (recorded on gh:#1743, 2026-11-25).** The
//! workspace's Moltres route (`msre::moltres_drift`) is a finite-volume
//! model of precursor drift around the loop beside the plant. It has no
//! neutronics-thermal-hydraulics coupling on a mesh, so it is not counted as
//! a coarse-mesh multiphysics backend. Both pages say so and name the epic.
//!
//! **Control volumes.** The model is TM-1070's lumped network: it holds heat
//! capacities, not volumes or masses, so those fields are empty rather than
//! made up. The fuel-salt and graphite core lumps are balance volumes with
//! the model's own lump temperatures. The other entries are the points at
//! which the model reports a stream temperature (core inlet and outlet, the
//! heat exchanger's four nozzles, the radiator outlets); they are junctions
//! with no hold-up of their own. Loop mass flows are the flow fraction times
//! TM-0728's design volume flow times the TUAS salt density.
//!
//! Research, education and V&V only. NOT VALIDATED. AI draft (gh:#1748).

use tuas_boussinesq_solver::boussinesq_thermophysical_properties::liquid_database::msre_coolant_salt_tm2316::msre_coolant_salt_rho;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::liquid_database::msre_fuel_salt_tm0728::msre_fuel_salt_rho;
use uom::si::f64::{MassRate, Power, TemperatureInterval, ThermodynamicTemperature, Time};
use uom::si::temperature_interval::degree_fahrenheit as delta_f;
use uom::si::thermodynamic_temperature::degree_fahrenheit;
use uom::si::time::second;

use super::design::MsreDesignPoint;
use super::first_criticality::Study;
use super::session::{
    coefficient_from_pair, Coefficients, Command, Session, SessionConfig, Snapshot, MAX_ADVANCE_S, SPLIT_BASE_F,
    SPLIT_STEP_F,
};
use crate::twin::{
    CoefficientSource, ControlVolume, ControlVolumeMap, ControlVolumes, CvLink, CvLinkKind, CvRole, FeedbackCoupling,
    KeffResult, KeffRun, MonteCarloCase, MultiFidelityTwin, NoMultiphysics, PlantModel, TwinError, TwinSnapshot,
};

/// Where the MSRE's missing multiphysics pages point (module docs).
pub const MULTIPHYSICS_ISSUE: u32 = 1743;
/// The committed Monte Carlo record behind [`Coefficients::MonteCarloN3`].
pub const N3_RECORD: &str = "crates/nee_soon/verification_and_validation/msre_n1_first_criticality/README.md (MSRE N-3, gh:#1129)";
/// The citation behind [`Coefficients::Ornl1965`].
pub const ORNL_1965_CITATION: &str = "ORNL-TM-1070 Table 1 (1965 predictions): fuel -4.84, graphite -3.7 x 1e-5 /F";

/// The MSRE twin: plant, the N-3 split, no multiphysics.
pub type MsreTwin = MultiFidelityTwin<MsrePlant, MsreSplit, NoMultiphysics>;

/// The MSRE plant on the template: a [`Session`], unchanged.
pub struct MsrePlant {
    session: Session,
}

impl MsrePlant {
    /// The session, for what the template does not cover (the freeze
    /// guard, the pacer).
    pub fn session(&self) -> &Session {
        &self.session
    }
}

fn plant_error(e: super::model::MsreModelError) -> TwinError {
    TwinError::Plant(e.to_string())
}

impl TwinSnapshot for Snapshot {
    fn columns() -> Vec<&'static str> {
        Snapshot::FIELDS.to_vec()
    }
    fn values(&self) -> Vec<f64> {
        self.to_vec()
    }
}

impl PlantModel for MsrePlant {
    type Config = SessionConfig;
    type Command = Command;
    type Snapshot = Snapshot;

    fn build(config: SessionConfig) -> Result<Self, TwinError> {
        Ok(MsrePlant { session: Session::new(config).map_err(plant_error)? })
    }

    fn apply(&mut self, command: Command) -> Result<(), TwinError> {
        self.session.apply(command).map_err(plant_error)
    }

    /// `dt` in [`MAX_ADVANCE_S`] chunks, as `session::headless_csv` runs.
    fn advance(&mut self, dt: Time) -> Result<Snapshot, TwinError> {
        let mut left = dt.get::<second>();
        while left > 1e-9 {
            let chunk = left.min(MAX_ADVANCE_S);
            self.session.advance(chunk).map_err(plant_error)?;
            left -= chunk;
        }
        Ok(self.session.snapshot())
    }

    fn snapshot(&self) -> Snapshot {
        self.session.snapshot()
    }
}

impl FeedbackCoupling<Coefficients> for MsrePlant {
    /// "Send to plant": `Command::Coefficients`, which keeps the state and
    /// the reference temperatures (`MsreModel::set_temperature_coefficients`).
    fn set_coefficients(&mut self, c: &Coefficients) -> Result<(), TwinError> {
        self.apply(Command::Coefficients(*c))
    }
}

fn f_to_t(f: f64) -> Option<ThermodynamicTemperature> {
    Some(ThermodynamicTemperature::new::<degree_fahrenheit>(f))
}

impl ControlVolumeMap for MsrePlant {
    fn control_volumes(&self) -> ControlVolumes {
        let s = self.session.snapshot();
        let d = MsreDesignPoint::tm0728();
        let mid = |a: ThermodynamicTemperature, b: ThermodynamicTemperature| {
            ThermodynamicTemperature::new::<degree_fahrenheit>(0.5 * (a.get::<degree_fahrenheit>() + b.get::<degree_fahrenheit>()))
        };
        // Design mass flows at the design mean temperatures, as
        // `MsreDesignPoint::fuel_heat_capacity_rate` evaluates them.
        let fuel_design: MassRate = msre_fuel_salt_rho(mid(d.fuel_reactor_inlet, d.fuel_reactor_outlet)) * d.fuel_flow;
        let coolant_design: MassRate = msre_coolant_salt_rho(mid(d.coolant_hx_inlet, d.coolant_hx_outlet)) * d.coolant_flow;
        let fuel = fuel_design * s.fuel_flow;
        let coolant = coolant_design * s.coolant_flow;
        let cv = |id: &str, name: &str, medium: &str, role: CvRole, t_f: f64, boundary: bool| ControlVolume {
            id: id.into(),
            name: name.into(),
            medium: medium.into(),
            role,
            volume: None,
            temperature: f_to_t(t_f),
            mass: None,
            on_pressure_boundary: boundary,
        };
        let flow = |from: &str, to: &str, m: MassRate| CvLink { from: from.into(), to: to.into(), kind: CvLinkKind::Flow(m) };
        let heat = |from: &str, to: &str, p: Option<Power>| CvLink { from: from.into(), to: to.into(), kind: CvLinkKind::Heat(p) };
        ControlVolumes {
            volumes: vec![
                cv("fuel.core_inlet", "Core inlet (fuel salt)", "fuel salt", CvRole::Junction, s.core_inlet_f, true),
                cv("fuel.core", "Core fuel-salt lump", "fuel salt", CvRole::Balance, s.fuel_f, true),
                cv("graphite.core", "Core graphite", "graphite", CvRole::Balance, s.graphite_f, true),
                cv("fuel.core_outlet", "Core outlet (fuel salt)", "fuel salt", CvRole::Junction, s.core_outlet_f, true),
                cv("fuel.hx_inlet", "Heat exchanger fuel inlet", "fuel salt", CvRole::Junction, s.hx_fuel_in_f, true),
                cv("fuel.hx_outlet", "Heat exchanger fuel outlet", "fuel salt", CvRole::Junction, s.hx_fuel_out_f, true),
                cv("coolant.hx_inlet", "Heat exchanger coolant inlet", "coolant salt", CvRole::Junction, s.hx_coolant_in_f, false),
                cv("coolant.hx_outlet", "Heat exchanger coolant outlet", "coolant salt", CvRole::Junction, s.hx_coolant_out_f, false),
                cv("coolant.radiator_outlet", "Radiator coolant outlet", "coolant salt", CvRole::Junction, s.radiator_coolant_out_f, false),
                cv("air.radiator_outlet", "Radiator air outlet", "air", CvRole::Boundary, s.radiator_air_out_f, false),
            ],
            links: vec![
                flow("fuel.core_inlet", "fuel.core", fuel),
                flow("fuel.core", "fuel.core_outlet", fuel),
                flow("fuel.core_outlet", "fuel.hx_inlet", fuel),
                flow("fuel.hx_inlet", "fuel.hx_outlet", fuel),
                flow("fuel.hx_outlet", "fuel.core_inlet", fuel),
                flow("coolant.hx_inlet", "coolant.hx_outlet", coolant),
                flow("coolant.hx_outlet", "coolant.radiator_outlet", coolant),
                flow("coolant.radiator_outlet", "coolant.hx_inlet", coolant),
                // The model exposes neither rate: the paths are named, the
                // rates left empty.
                heat("fuel.core", "graphite.core", None),
                heat("fuel.hx_inlet", "coolant.hx_inlet", None),
                heat("coolant.radiator_outlet", "air.radiator_outlet", None),
            ],
        }
    }
}

/// The MSRE N-3 split on the full-core Monte Carlo model: runs A, C, D at
/// the record's temperatures with these run settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MsreSplit {
    pub n_particles: usize,
    pub n_inactive: usize,
    pub n_active: usize,
    pub seed: u64,
    /// The whole TM-0728 shield (the default) or none (the ablation).
    pub shield: bool,
    pub u235_scale: f64,
}

impl MsreSplit {
    /// The study of run `i` (0 A, 1 C, 2 D), as the k_eff tab's `Split`.
    pub fn study(&self, i: usize) -> Study {
        let (b, up) = (SPLIT_BASE_F, SPLIT_BASE_F + SPLIT_STEP_F);
        let (salt_f, graphite_f) = match i {
            0 => (b, b),
            1 => (up, b),
            _ => (b, up),
        };
        Study { salt_f, graphite_f, u235_scale: self.u235_scale, shield: self.shield, ..Study::default() }
    }
}

impl MonteCarloCase for MsreSplit {
    type Coefficients = Coefficients;

    fn runs(&self) -> Vec<KeffRun> {
        let d = TemperatureInterval::new::<delta_f>(SPLIT_STEP_F);
        vec![
            KeffRun { label: format!("A: salt and graphite {SPLIT_BASE_F} F"), region: None, delta: TemperatureInterval::new::<delta_f>(0.0) },
            KeffRun { label: format!("C: salt +{SPLIT_STEP_F} F"), region: Some("fuel salt".into()), delta: d },
            KeffRun { label: format!("D: graphite +{SPLIT_STEP_F} F"), region: Some("graphite".into()), delta: d },
        ]
    }

    /// Natively: process the study's tapes on one `KeffWorker`, assemble,
    /// and run `run_keff_csg_par` on every core. The k_eff tab's pool
    /// gives the same `k`, bit for bit (`msre::web::keff_pool`'s
    /// `a_pooled_run_is_run_keff_csg_par_bit_for_bit`). In the browser the
    /// page runs the split on its own worker pool instead.
    fn run(&mut self, index: usize) -> Result<KeffResult, TwinError> {
        if index > 2 {
            return Err(TwinError::MonteCarlo(format!("the split has runs 0-2, not {index}")));
        }
        native_keff(self, &self.study(index))
    }

    fn coefficients(&self, r: &[KeffResult]) -> Result<Coefficients, TwinError> {
        let [a, c, d] = r else {
            return Err(TwinError::MonteCarlo(format!("the split needs 3 runs, got {}", r.len())));
        };
        let (f, sf) = coefficient_from_pair(a.k, a.sigma, c.k, c.sigma, SPLIT_STEP_F);
        let (g, sg) = coefficient_from_pair(a.k, a.sigma, d.k, d.sigma, SPLIT_STEP_F);
        Ok(Coefficients::Measured { fuel_per_f: f, fuel_sigma: sf, graphite_per_f: g, graphite_sigma: sg })
    }

    fn describe(&self, c: &Coefficients) -> Vec<String> {
        vec![c.describe()]
    }
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn native_keff(split: &MsreSplit, study: &Study) -> Result<KeffResult, TwinError> {
    use super::web::keff::{entropy_mesh, jobs, native_tape, run_settings, KeffWorker};
    use dhoby_ghaut::mc_web::keff::KeffConfig;
    let err = TwinError::MonteCarlo;
    let mut w = KeffWorker::default();
    for (j, job) in jobs(study).iter().enumerate() {
        let bytes = match job.tape() {
            Some(t) => native_tape(t).map_err(err)?,
            None => Vec::new(),
        };
        w.process(study, j, &bytes).map_err(err)?;
    }
    w.assemble(study, native_tape, || 0.0).map_err(err)?;
    let md = w.model.as_ref().ok_or_else(|| TwinError::MonteCarlo("no model after assembly".into()))?;
    let cfg = KeffConfig {
        n_particles: split.n_particles,
        n_inactive: split.n_inactive,
        n_active: split.n_active,
        seed: split.seed,
        point_source: false,
        want_sites: false,
    };
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    let r = outram_mc_libs::physics::transport_csg::run_keff_csg_par(
        &md.geometry,
        &md.materials,
        &md.nuclides,
        &[],
        Some(&entropy_mesh()),
        super::first_criticality::source_box(),
        &run_settings(&cfg, md.salt_k),
        None,
        &[],
        None,
        outram_mc_libs::physics::compute::ThreadCount::Fixed(threads),
    );
    Ok(KeffResult { k: r.k_mean, sigma: r.k_std })
}

/// In the browser and on Android the split runs on the page's worker pool
/// (`msre::web::keff_pool`), not through the template.
#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn native_keff(_split: &MsreSplit, _study: &Study) -> Result<KeffResult, TwinError> {
    Err(TwinError::MonteCarlo("the blocking split runs natively only; the page runs it on its worker pool".into()))
}

/// The MSRE twin at `config`, on the cited set or whichever `config`
/// names, with the split attached. Its coefficient source is named from
/// `config.coefficients`.
pub fn msre_twin(config: SessionConfig, split: MsreSplit) -> Result<MsreTwin, TwinError> {
    let source = match config.coefficients {
        Coefficients::Ornl1965 => CoefficientSource::Cited { value: Coefficients::Ornl1965, citation: ORNL_1965_CITATION.into() },
        Coefficients::MonteCarloN3 => CoefficientSource::Committed { value: Coefficients::MonteCarloN3, record: N3_RECORD.into() },
        m @ Coefficients::Measured { .. } => CoefficientSource::Committed { value: m, record: "a measured set passed in at start".into() },
    };
    let plant = MsrePlant::build(config)?;
    Ok(MultiFidelityTwin::new(plant, source, 1748, MULTIPHYSICS_ISSUE)?.with_monte_carlo(split))
}

/// The split the k_eff tab's quickest preset runs (`keff_pool::PRESETS[0]`:
/// 4000 × [30 + 140], seed 1), on the default study (whole shield).
pub fn quick_split() -> MsreSplit {
    MsreSplit { n_particles: 4_000, n_inactive: 30, n_active: 140, seed: 1, shield: true, u235_scale: 1.0 }
}

/// The demonstration transient of `session::demo_script`, in template time.
pub fn demo_script() -> Vec<(Time, Command)> {
    super::session::demo_script().into_iter().map(|(t, c)| (Time::new::<second>(t), c)).collect()
}

#[cfg(test)]
mod tests;
