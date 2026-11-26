//! # EBR-II on the multifidelity twin template (gh:#1749, epic gh:#1743)
//!
//! [`Ebr2Twin`] is EBR-II's [`MultiFidelityTwin`]: the plant is the
//! existing [`Session`] (`Ebr2Model`, SHRT-17 / SHRT-45R), wired as every
//! adopter is. **No physics here and nothing re-implemented.**
//!
//! | Page | EBR-II |
//! |---|---|
//! | Plant | [`Ebr2Plant`]: [`Session`], unchanged (pinned bit for bit) |
//! | Control volumes | the model's four thermal nodes (fuel, core coolant, IHX primary, pool), its sodium masses, the primary and intermediate flows, from the live snapshot |
//! | Coarse / fine multiphysics | not built: a bedok-style nodal model is the candidate (gh:#1747) |
//! | Monte Carlo | not built: EBR-II has no whole-core Monte Carlo model yet (gh:#1313, #1314) |
//!
//! **Coefficients.** The plant runs on Argonne's VARIANT set (T1819 TABLE
//! 37, cited by `ReactivityCoefficients::argonne`). There is no Monte Carlo
//! case to produce a live set, so the case type is [`NoEbr2MonteCarlo`], an
//! uninhabited type whose coefficients are [`ReactivityCoefficients`]: the
//! page says "not built" and names gh:#1313, while the coefficient source
//! is still named. When a whole-core model lands, its split replaces this
//! type and `send_to_plant` goes through
//! [`Session::set_reactivity_coefficients`] (added for this, gh:#1749),
//! which keeps the feedback references at the initial steady state.
//!
//! SHRT-17's power is tabulated (a boundary condition, no feedback): a
//! twin on SHRT-17 keeps the cited source and refuses a coefficient
//! change, as the model does.
//!
//! Research, education and V&V only. NOT VALIDATED. AI draft (gh:#1749).

use uom::si::f64::{Mass, MassRate, ThermodynamicTemperature, Time};
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;

use super::kinetics::ArgonneSolver;
use super::model::ReactivityCoefficients;
use super::session::{Command, Session, SessionConfig, Snapshot, MAX_ADVANCE_S};
use crate::twin::{
    CoefficientSource, ControlVolume, ControlVolumeMap, ControlVolumes, CvLink, CvLinkKind, CvRole, FeedbackCoupling,
    KeffResult, KeffRun, MonteCarloCase, MultiFidelityTwin, NoMultiphysics, PlantModel, TwinError, TwinSnapshot,
};

/// The issue a whole-core Monte Carlo model is tracked under.
pub const MONTE_CARLO_ISSUE: u32 = 1313;
/// The issue the bedok multiphysics backend is tracked under.
pub const MULTIPHYSICS_ISSUE: u32 = 1747;
/// The citation behind the plant's coefficients.
pub const ARGONNE_CITATION: &str = "Argonne VARIANT, ANL T1819 TABLE 37 (gh:#1314)";

/// The EBR-II twin: plant only, for now.
pub type Ebr2Twin = MultiFidelityTwin<Ebr2Plant, NoEbr2MonteCarlo, NoMultiphysics>;

/// EBR-II's Monte Carlo case until a whole-core model exists: no values,
/// so it is never attached, but its coefficients are the plant's type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoEbr2MonteCarlo {}

impl MonteCarloCase for NoEbr2MonteCarlo {
    type Coefficients = ReactivityCoefficients;
    fn runs(&self) -> Vec<KeffRun> {
        match *self {}
    }
    fn run(&mut self, _index: usize) -> Result<KeffResult, TwinError> {
        match *self {}
    }
    fn coefficients(&self, _results: &[KeffResult]) -> Result<ReactivityCoefficients, TwinError> {
        match *self {}
    }
    fn describe(&self, _c: &ReactivityCoefficients) -> Vec<String> {
        match *self {}
    }
}

/// The EBR-II plant on the template: a [`Session`], unchanged.
pub struct Ebr2Plant {
    session: Session,
}

impl Ebr2Plant {
    /// The session, for what the template does not cover (the guard, the
    /// t = 0 comparison).
    pub fn session(&self) -> &Session {
        &self.session
    }
}

fn plant_error(e: super::model::Ebr2Error) -> TwinError {
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

impl PlantModel for Ebr2Plant {
    type Config = SessionConfig;
    type Command = Command;
    type Snapshot = Snapshot;

    fn build(config: SessionConfig) -> Result<Self, TwinError> {
        Ok(Ebr2Plant { session: Session::new(config).map_err(plant_error)? })
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

impl FeedbackCoupling<ReactivityCoefficients> for Ebr2Plant {
    /// Point kinetics: [`Session::set_reactivity_coefficients`]. Tabulated
    /// power (SHRT-17) has no feedback: the set it already reports (the
    /// configuration's) is accepted unchanged, any other is refused.
    fn set_coefficients(&mut self, c: &ReactivityCoefficients) -> Result<(), TwinError> {
        match self.session.reactivity_coefficients() {
            Some(_) => self.session.set_reactivity_coefficients(*c).map_err(plant_error),
            None if *c == ReactivityCoefficients::argonne(ArgonneSolver::Variant) => Ok(()),
            None => Err(TwinError::Plant("tabulated power (SHRT-17) has no feedback coefficients".into())),
        }
    }
}

impl ControlVolumeMap for Ebr2Plant {
    fn control_volumes(&self) -> ControlVolumes {
        let s = self.session.snapshot();
        let c = |x: f64| Some(ThermodynamicTemperature::new::<degree_celsius>(x));
        let kg = |x: f64| Some(Mass::new::<kilogram>(x));
        let cv = |id: &str, name: &str, medium: &str, role: CvRole, t: Option<ThermodynamicTemperature>, m: Option<Mass>| ControlVolume {
            id: id.into(),
            name: name.into(),
            medium: medium.into(),
            role,
            volume: None,
            temperature: t,
            mass: m,
            on_pressure_boundary: medium == "sodium" || medium == "fuel",
        };
        let primary = MassRate::new::<kilogram_per_second>(s.primary_flow_kg_s);
        let intermediate = MassRate::new::<kilogram_per_second>(s.intermediate_flow_kg_s);
        let flow = |from: &str, to: &str, m: MassRate| CvLink { from: from.into(), to: to.into(), kind: CvLinkKind::Flow(m) };
        let heat = |from: &str, to: &str| CvLink { from: from.into(), to: to.into(), kind: CvLinkKind::Heat(None) };
        ControlVolumes {
            volumes: vec![
                cv("core.fuel", "Fuel (U-5Fs pins)", "fuel", CvRole::Balance, c(s.fuel_c), None),
                cv("primary.core", "Core coolant (with cladding and in-core steel)", "sodium", CvRole::Balance, c(s.core_mean_c), kg(s.core_na_kg)),
                cv("primary.core_outlet", "Core outlet (mixed mean)", "sodium", CvRole::Junction, c(s.core_outlet_c), None),
                cv("primary.ihx", "IHX primary side", "sodium", CvRole::Balance, c(s.ihx_outlet_c), kg(s.ihx_na_kg)),
                cv("primary.pool", "Primary pool (core inlet)", "sodium", CvRole::Balance, c(s.core_inlet_c), kg(s.pool_na_kg)),
                cv("intermediate.inlet", "Intermediate loop inlet", "intermediate sodium", CvRole::Boundary, c(s.intermediate_inlet_c), None),
            ],
            links: vec![
                flow("primary.pool", "primary.core", primary),
                flow("primary.core", "primary.core_outlet", primary),
                flow("primary.core_outlet", "primary.ihx", primary),
                flow("primary.ihx", "primary.pool", primary),
                flow("intermediate.inlet", "primary.ihx", intermediate),
                heat("core.fuel", "primary.core"),
                heat("primary.ihx", "intermediate.inlet"),
            ],
        }
    }
}

/// The EBR-II twin at `config` on Argonne's VARIANT set, with the missing
/// pages naming their issues.
pub fn ebr2_twin(config: SessionConfig) -> Result<Ebr2Twin, TwinError> {
    let plant = Ebr2Plant::build(config)?;
    let cited = CoefficientSource::Cited { value: ReactivityCoefficients::argonne(ArgonneSolver::Variant), citation: ARGONNE_CITATION.into() };
    MultiFidelityTwin::new(plant, cited, MONTE_CARLO_ISSUE, MULTIPHYSICS_ISSUE)
}

/// `session::demo_script` in template time (empty: each SHRT test is its
/// own transient).
pub fn demo_script() -> Vec<(Time, Command)> {
    super::session::demo_script().into_iter().map(|(t, c)| (Time::new::<second>(t), c)).collect()
}

#[cfg(test)]
mod tests;
