//! # The multifidelity digital twin template (epic gh:#1743, core gh:#1744)
//!
//! One generic struct, [`MultiFidelityTwin`], that every multifidelity
//! digital twin in this workspace (MSRE, EBR-II, HTR-10) is built from. It
//! wires a real-time plant model to the higher-fidelity models that inform
//! it, and gives each fidelity level a page:
//!
//! | Page ([`TwinPage`]) | Fidelity | Supplied by |
//! |---|---|---|
//! | Plant | lumped / 0-D, real time | [`PlantModel`] |
//! | Control volumes | the plant's own CVs, read from the assembled model | [`ControlVolumeMap`] |
//! | Coarse-mesh multiphysics | nodal / coarse porous media | [`MultiphysicsCase`] at [`MeshResolution::Coarse`] |
//! | Fine-mesh multiphysics | fine finite-volume mesh | [`MultiphysicsCase`] at [`MeshResolution::Fine`] |
//! | Monte Carlo | continuous-energy `k_eff` and coefficients | [`MonteCarloCase`] |
//!
//! **How the levels talk.** A Monte Carlo run produces temperature
//! coefficients ([`MonteCarloCase::coefficients`]); "send to plant"
//! ([`MultiFidelityTwin::send_to_plant`]) hands them to the plant through
//! [`FeedbackCoupling`]. A multiphysics run informs the plant through the
//! same seam. The set the plant is using is always named: a
//! [`CoefficientSource`] is cited, a committed record, or a live run, and
//! never anonymous.
//!
//! **Design rules** (root `AGENTS.md`): the seams are traits consumed
//! generically, never `dyn`; closed sets are enums ([`MultiphysicsBackend`],
//! [`TwinPage`], [`CvRole`]); no `Box`, no lifetimes, no macros; physical
//! quantities cross the API as `uom`. A twin with no Monte Carlo or no
//! multiphysics model uses the empty types [`NoMonteCarlo`] and
//! [`NoMultiphysics`]; its pages then say "not built" with the issue,
//! never a placeholder that pretends.
//!
//! **Headless** (the workspace's mandatory headless mode): the struct owns
//! the plant CSV ([`MultiFidelityTwin::plant_csv`]), the CV dump
//! ([`MultiFidelityTwin::dump_cvs`]) and the coefficient run
//! ([`MultiFidelityTwin::coefficient_report`]), all reached from
//! [`MultiFidelityTwin::headless`] by command-line flags.
//!
//! This module adds **no physics**: every number comes from the adopter's
//! models. The one formula here, [`reactivity_coefficient`], is the
//! definition of a reactivity temperature coefficient from two `k_eff`.
//!
//! Research, education and V&V only. NOT VALIDATED. AI draft (gh:#1744).

use uom::si::f64::{
    Mass, MassRate, Power, TemperatureCoefficient, TemperatureInterval, ThermodynamicTemperature, Time, Volume,
};
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::temperature_interval::kelvin as kelvin_interval;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volume::cubic_meter;

// ------------------------------------------------------------------ errors

/// Why a twin operation failed.
#[derive(Clone, Debug, PartialEq)]
pub enum TwinError {
    /// The plant model refused (its own message).
    Plant(String),
    /// A Monte Carlo run failed (its own message).
    MonteCarlo(String),
    /// A multiphysics run failed (its own message).
    Multiphysics(String),
    /// The page's model is not built for this twin; `issue` tracks it.
    NotBuilt { page: TwinPage, issue: u32 },
    /// A headless argument could not be read.
    BadArgument(String),
}

impl core::fmt::Display for TwinError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TwinError::Plant(m) => write!(f, "plant model: {m}"),
            TwinError::MonteCarlo(m) => write!(f, "Monte Carlo: {m}"),
            TwinError::Multiphysics(m) => write!(f, "multiphysics: {m}"),
            TwinError::NotBuilt { page, issue } => write!(f, "{} is not built yet (gh:#{issue})", page.title()),
            TwinError::BadArgument(m) => write!(f, "bad argument: {m}"),
        }
    }
}

// ------------------------------------------------------------------- pages

/// The five pages, in fidelity order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TwinPage {
    Plant,
    ControlVolumes,
    CoarseMultiphysics,
    FineMultiphysics,
    MonteCarlo,
}

impl TwinPage {
    /// Every page, in tab order.
    pub const ALL: [TwinPage; 5] = [
        TwinPage::Plant,
        TwinPage::ControlVolumes,
        TwinPage::CoarseMultiphysics,
        TwinPage::FineMultiphysics,
        TwinPage::MonteCarlo,
    ];

    /// The tab label.
    pub fn title(self) -> &'static str {
        match self {
            TwinPage::Plant => "Plant",
            TwinPage::ControlVolumes => "Control volumes",
            TwinPage::CoarseMultiphysics => "Coarse-mesh multiphysics",
            TwinPage::FineMultiphysics => "Fine-mesh multiphysics",
            TwinPage::MonteCarlo => "Monte Carlo",
        }
    }
}

// ------------------------------------------------------------------- plant

/// A plant state flattened to numbers, for the CSV and the worker wire.
pub trait TwinSnapshot: Clone {
    /// Column names, the same length as [`Self::values`]. The first is the
    /// plant time.
    fn columns() -> Vec<&'static str>;
    /// The values, in [`Self::columns`] order.
    fn values(&self) -> Vec<f64>;
}

/// Page 1: the real-time plant model. The MSRE and EBR-II sessions already
/// have this shape (`new`, `apply`, `advance`, `snapshot`).
pub trait PlantModel {
    type Config;
    type Command: Clone;
    type Snapshot: TwinSnapshot;
    /// Build the plant at its initial state.
    fn build(config: Self::Config) -> Result<Self, TwinError>
    where
        Self: Sized;
    /// Apply an operator command.
    fn apply(&mut self, command: Self::Command) -> Result<(), TwinError>;
    /// Advance plant time by `dt` and return the new state.
    fn advance(&mut self, dt: Time) -> Result<Self::Snapshot, TwinError>;
    /// The current state.
    fn snapshot(&self) -> Self::Snapshot;
}

/// How a higher-fidelity result reaches the plant: the plant accepts a
/// coefficient set of type `C` and uses it from then on.
pub trait FeedbackCoupling<C> {
    fn set_coefficients(&mut self, coefficients: &C) -> Result<(), TwinError>;
}

// --------------------------------------------------------- control volumes

/// What kind of entry a control volume is (the roles of `htgr_sim_v1`'s CV
/// map, gh:#1718).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CvRole {
    /// Mass and energy balance: the model integrates its state.
    Balance,
    /// A junction with no hold-up of its own.
    Junction,
    /// Mass inventory only, no energy balance.
    Inventory,
    /// A component the model carries with no volume.
    NoVolume,
    /// A boundary condition (source or sink).
    Boundary,
}

impl CvRole {
    /// The CSV word.
    pub fn word(self) -> &'static str {
        match self {
            CvRole::Balance => "balance",
            CvRole::Junction => "junction",
            CvRole::Inventory => "inventory",
            CvRole::NoVolume => "no-volume",
            CvRole::Boundary => "boundary",
        }
    }
}

/// One control volume, read from the assembled plant model. A value the
/// model does not have is `None`, never a made-up number.
#[derive(Clone, Debug, PartialEq)]
pub struct ControlVolume {
    /// Stable identifier (`primary.hot_leg`), what a fault tree names.
    pub id: String,
    pub name: String,
    /// The medium in it (`helium`, `fuel salt`, `sodium`).
    pub medium: String,
    pub role: CvRole,
    pub volume: Option<Volume>,
    pub temperature: Option<ThermodynamicTemperature>,
    pub mass: Option<Mass>,
    /// Inside the primary pressure boundary.
    pub on_pressure_boundary: bool,
}

/// What a link between two control volumes carries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CvLinkKind {
    /// Advected mass, positive `from -> to`.
    Flow(MassRate),
    /// A heat path, with its rate where the model has one.
    Heat(Option<Power>),
}

/// A connection between two control volumes.
#[derive(Clone, Debug, PartialEq)]
pub struct CvLink {
    pub from: String,
    pub to: String,
    pub kind: CvLinkKind,
}

/// The plant's control volumes and links.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ControlVolumes {
    pub volumes: Vec<ControlVolume>,
    pub links: Vec<CvLink>,
}

/// Page 2: the plant exposes its control volumes, read from the ASSEMBLED
/// model at its current state, never from a separate table of constants.
pub trait ControlVolumeMap {
    fn control_volumes(&self) -> ControlVolumes;
}

fn opt(v: Option<f64>, digits: usize) -> String {
    v.map(|x| format!("{x:.digits$}")).unwrap_or_default()
}

impl ControlVolumes {
    /// The `--dump-cvs` header.
    pub const VOLUME_HEADER: &'static str = "id,name,medium,role,volume_m3,on_pressure_boundary,temperature_k,mass_kg";
    /// The links header.
    pub const LINK_HEADER: &'static str = "from,to,kind,rate";

    /// The entry with id `id`.
    pub fn get(&self, id: &str) -> Option<&ControlVolume> {
        self.volumes.iter().find(|v| v.id == id)
    }

    /// Links that name an id that is not a volume: a structural defect.
    pub fn dangling_links(&self) -> Vec<&CvLink> {
        self.links.iter().filter(|l| self.get(&l.from).is_none() || self.get(&l.to).is_none()).collect()
    }

    /// The volumes then the links, as two headed CSV blocks separated by a
    /// blank line. Fixed precision, so the dump is deterministic.
    pub fn to_csv(&self) -> String {
        let mut out = String::from(Self::VOLUME_HEADER);
        out.push('\n');
        for v in &self.volumes {
            out.push_str(&format!(
                "{},{},{},{},{},{},{},{}\n",
                v.id,
                v.name,
                v.medium,
                v.role.word(),
                opt(v.volume.map(|x| x.get::<cubic_meter>()), 6),
                v.on_pressure_boundary,
                opt(v.temperature.map(|t| t.get::<kelvin>()), 3),
                opt(v.mass.map(|m| m.get::<kilogram>()), 6),
            ));
        }
        out.push('\n');
        out.push_str(Self::LINK_HEADER);
        out.push('\n');
        for l in &self.links {
            let (kind, rate) = match l.kind {
                CvLinkKind::Flow(m) => ("flow_kg_per_s", opt(Some(m.get::<kilogram_per_second>()), 6)),
                CvLinkKind::Heat(p) => ("heat_w", opt(p.map(|p| p.get::<watt>()), 3)),
            };
            out.push_str(&format!("{},{},{},{}\n", l.from, l.to, kind, rate));
        }
        out
    }
}

// ------------------------------------------------------------- Monte Carlo

/// One `k_eff` result with its one-sigma statistical uncertainty.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeffResult {
    pub k: f64,
    pub sigma: f64,
}

/// One run a Monte Carlo case asks for: the base state, or one region
/// raised by `delta`.
#[derive(Clone, Debug, PartialEq)]
pub struct KeffRun {
    /// What it is (`base`, `fuel +55.6 K`).
    pub label: String,
    /// The region perturbed, `None` for the base case.
    pub region: Option<String>,
    pub delta: TemperatureInterval,
}

/// A reactivity temperature coefficient with its one-sigma uncertainty.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coefficient {
    pub value: TemperatureCoefficient,
    pub sigma: TemperatureCoefficient,
}

/// The reactivity temperature coefficient from a base and a raised `k_eff`:
/// `α = (ρ₁ − ρ₀)/ΔT = (1/k₀ − 1/k₁)/ΔT`, with `ρ = 1 − 1/k`. The two runs
/// are independent, so `σ_ρ = σ_k/k²` adds in quadrature.
///
/// The same definition as `msre::session::coefficient_from_pair` (which
/// works per °F); the tests pin the two equal.
pub fn reactivity_coefficient(base: KeffResult, raised: KeffResult, delta: TemperatureInterval) -> Coefficient {
    let dt = delta.get::<kelvin_interval>();
    let alpha = (1.0 / base.k - 1.0 / raised.k) / dt;
    let s0 = base.sigma / (base.k * base.k);
    let s1 = raised.sigma / (raised.k * raised.k);
    let sigma = (s0 * s0 + s1 * s1).sqrt() / dt.abs();
    Coefficient {
        value: TemperatureCoefficient::new::<per_kelvin>(alpha),
        sigma: TemperatureCoefficient::new::<per_kelvin>(sigma),
    }
}

/// Page 5: a Monte Carlo case. The adopter runs the transport (on its
/// worker pool, or blocking here for headless); the template orders the
/// runs, turns them into coefficients and hands them to the plant.
pub trait MonteCarloCase {
    /// What the plant receives ([`FeedbackCoupling`]).
    type Coefficients: Clone;
    /// The runs, base first.
    fn runs(&self) -> Vec<KeffRun>;
    /// Run one of [`Self::runs`] (blocking).
    fn run(&mut self, index: usize) -> Result<KeffResult, TwinError>;
    /// The coefficients from the finished runs, in [`Self::runs`] order.
    fn coefficients(&self, results: &[KeffResult]) -> Result<Self::Coefficients, TwinError>;
    /// One line per coefficient, for the page and the headless report.
    fn describe(&self, coefficients: &Self::Coefficients) -> Vec<String>;
}

/// Where the coefficient set the plant is using comes from. Always shown.
#[derive(Clone, Debug, PartialEq)]
pub enum CoefficientSource<C> {
    /// Published values, with the citation.
    Cited { value: C, citation: String },
    /// A committed Monte Carlo record, with its path or issue.
    Committed { value: C, record: String },
    /// This session's own run.
    Live { value: C, runs: Vec<(KeffRun, KeffResult)> },
}

impl<C> CoefficientSource<C> {
    /// The coefficients.
    pub fn value(&self) -> &C {
        match self {
            CoefficientSource::Cited { value, .. }
            | CoefficientSource::Committed { value, .. }
            | CoefficientSource::Live { value, .. } => value,
        }
    }

    /// Where they come from, in one line.
    pub fn provenance(&self) -> String {
        match self {
            CoefficientSource::Cited { citation, .. } => format!("cited: {citation}"),
            CoefficientSource::Committed { record, .. } => format!("committed Monte Carlo record: {record}"),
            CoefficientSource::Live { runs, .. } => format!("live Monte Carlo run, {} k_eff runs this session", runs.len()),
        }
    }
}

/// The Monte Carlo case of a twin that has none. It has no values, so
/// nothing can be built with it; its `Option` is always `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoMonteCarlo {}

impl MonteCarloCase for NoMonteCarlo {
    type Coefficients = ();
    fn runs(&self) -> Vec<KeffRun> {
        match *self {}
    }
    fn run(&mut self, _index: usize) -> Result<KeffResult, TwinError> {
        match *self {}
    }
    fn coefficients(&self, _results: &[KeffResult]) -> Result<(), TwinError> {
        match *self {}
    }
    fn describe(&self, _coefficients: &()) -> Vec<String> {
        match *self {}
    }
}

// ------------------------------------------------------------ multiphysics

/// The multiphysics codes a page can run on. A closed set: each wraps an
/// existing crate, nothing is re-implemented here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MultiphysicsBackend {
    /// The GeN-Foam port (`outram-foam-appbuilder-lib::genfoam`, MGXS via
    /// `nee_soon::mgxs_genfoam`).
    GenFoam,
    /// `bedok` (Than Yan Ren's crate, used as a dependency only): nodal
    /// diffusion, channel thermal hydraulics, fuel-rod conduction.
    Bedok,
}

impl MultiphysicsBackend {
    pub fn name(self) -> &'static str {
        match self {
            MultiphysicsBackend::GenFoam => "GeN-Foam port",
            MultiphysicsBackend::Bedok => "bedok",
        }
    }
}

/// The two multiphysics pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshResolution {
    Coarse,
    Fine,
}

impl MeshResolution {
    pub fn page(self) -> TwinPage {
        match self {
            MeshResolution::Coarse => TwinPage::CoarseMultiphysics,
            MeshResolution::Fine => TwinPage::FineMultiphysics,
        }
    }
}

/// Whether a resolution has a backend for this twin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    Built(MultiphysicsBackend),
    /// Not built; the issue that tracks it.
    NotBuilt { issue: u32 },
}

/// Where a multiphysics run takes its cross sections from.
#[derive(Clone, Debug, PartialEq)]
pub enum XsSource {
    /// A published set, with the citation.
    Cited(String),
    /// Multigroup sets generated from this twin's Monte Carlo case.
    MonteCarlo,
}

/// Pages 3 and 4: a multiphysics case at two resolutions.
pub trait MultiphysicsCase {
    /// What a run produces (power shape, peaking, feedback, ...).
    type Result: Clone;
    fn availability(&self, resolution: MeshResolution) -> Availability;
    fn cross_sections(&self) -> XsSource;
    /// Run at `resolution` (blocking; seconds to hours).
    fn run(&mut self, resolution: MeshResolution) -> Result<Self::Result, TwinError>;
    /// One line per quantity of a result.
    fn describe(&self, result: &Self::Result) -> Vec<String>;
}

/// The multiphysics case of a twin that has none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoMultiphysics {}

impl MultiphysicsCase for NoMultiphysics {
    type Result = ();
    fn availability(&self, _resolution: MeshResolution) -> Availability {
        match *self {}
    }
    fn cross_sections(&self) -> XsSource {
        match *self {}
    }
    fn run(&mut self, _resolution: MeshResolution) -> Result<(), TwinError> {
        match *self {}
    }
    fn describe(&self, _result: &()) -> Vec<String> {
        match *self {}
    }
}

// -------------------------------------------------------------- the struct

/// A multifidelity digital twin: a plant, and optionally the Monte Carlo
/// and multiphysics models that inform it. See the module docs.
pub struct MultiFidelityTwin<P, M, F>
where
    P: PlantModel + ControlVolumeMap + FeedbackCoupling<M::Coefficients>,
    M: MonteCarloCase,
    F: MultiphysicsCase,
{
    plant: P,
    monte_carlo: Option<M>,
    /// The issue that tracks a missing Monte Carlo model.
    monte_carlo_issue: u32,
    multiphysics: Option<F>,
    /// The issue that tracks a missing multiphysics model.
    multiphysics_issue: u32,
    /// The set the plant is using now.
    in_use: CoefficientSource<M::Coefficients>,
    /// The latest live Monte Carlo set, not yet sent to the plant.
    live: Option<CoefficientSource<M::Coefficients>>,
    page: TwinPage,
}

impl<P, M, F> MultiFidelityTwin<P, M, F>
where
    P: PlantModel + ControlVolumeMap + FeedbackCoupling<M::Coefficients>,
    M: MonteCarloCase,
    F: MultiphysicsCase,
{
    /// A twin with only its plant, using `coefficients` (applied to the
    /// plant here, so the plant and the named source cannot disagree).
    /// `monte_carlo_issue` and `multiphysics_issue` are the issues the
    /// missing pages name until a model is attached.
    pub fn new(
        mut plant: P,
        coefficients: CoefficientSource<M::Coefficients>,
        monte_carlo_issue: u32,
        multiphysics_issue: u32,
    ) -> Result<Self, TwinError> {
        plant.set_coefficients(coefficients.value())?;
        Ok(MultiFidelityTwin {
            plant,
            monte_carlo: None,
            monte_carlo_issue,
            multiphysics: None,
            multiphysics_issue,
            in_use: coefficients,
            live: None,
            page: TwinPage::Plant,
        })
    }

    /// Attach the Monte Carlo case.
    pub fn with_monte_carlo(mut self, case: M) -> Self {
        self.monte_carlo = Some(case);
        self
    }

    /// Attach the multiphysics case.
    pub fn with_multiphysics(mut self, case: F) -> Self {
        self.multiphysics = Some(case);
        self
    }

    pub fn plant(&self) -> &P {
        &self.plant
    }

    pub fn plant_mut(&mut self) -> &mut P {
        &mut self.plant
    }

    pub fn monte_carlo(&self) -> Option<&M> {
        self.monte_carlo.as_ref()
    }

    pub fn multiphysics(&self) -> Option<&F> {
        self.multiphysics.as_ref()
    }

    pub fn page(&self) -> TwinPage {
        self.page
    }

    pub fn set_page(&mut self, page: TwinPage) {
        self.page = page;
    }

    /// The coefficient set the plant is using, with its source.
    pub fn coefficients(&self) -> &CoefficientSource<M::Coefficients> {
        &self.in_use
    }

    /// The latest live Monte Carlo set, if one was run and not yet sent.
    pub fn live_coefficients(&self) -> Option<&CoefficientSource<M::Coefficients>> {
        self.live.as_ref()
    }

    /// Whether a page has its model; `Err` names the issue if not. The plant
    /// and control-volume pages always have one.
    pub fn page_status(&self, page: TwinPage) -> Result<(), TwinError> {
        match page {
            TwinPage::Plant | TwinPage::ControlVolumes => Ok(()),
            TwinPage::MonteCarlo => match self.monte_carlo {
                Some(_) => Ok(()),
                None => Err(TwinError::NotBuilt { page, issue: self.monte_carlo_issue }),
            },
            TwinPage::CoarseMultiphysics | TwinPage::FineMultiphysics => {
                let resolution = if page == TwinPage::CoarseMultiphysics { MeshResolution::Coarse } else { MeshResolution::Fine };
                match &self.multiphysics {
                    None => Err(TwinError::NotBuilt { page, issue: self.multiphysics_issue }),
                    Some(f) => match f.availability(resolution) {
                        Availability::Built(_) => Ok(()),
                        Availability::NotBuilt { issue } => Err(TwinError::NotBuilt { page, issue }),
                    },
                }
            }
        }
    }

    /// Apply an operator command to the plant.
    pub fn apply(&mut self, command: P::Command) -> Result<(), TwinError> {
        self.plant.apply(command)
    }

    /// Advance the plant.
    pub fn advance(&mut self, dt: Time) -> Result<P::Snapshot, TwinError> {
        self.plant.advance(dt)
    }

    /// The control volumes of the plant as it is now.
    pub fn control_volumes(&self) -> ControlVolumes {
        self.plant.control_volumes()
    }

    /// Run every Monte Carlo run in order (blocking), calling `progress`
    /// after each with `(done, total, label, result)`, and keep the result
    /// as the live set. It is NOT sent to the plant; see
    /// [`Self::send_to_plant`].
    pub fn run_monte_carlo(
        &mut self,
        mut progress: impl FnMut(usize, usize, &str, KeffResult),
    ) -> Result<&CoefficientSource<M::Coefficients>, TwinError> {
        let Some(case) = self.monte_carlo.as_mut() else {
            return Err(TwinError::NotBuilt { page: TwinPage::MonteCarlo, issue: self.monte_carlo_issue });
        };
        let runs = case.runs();
        let mut results = Vec::with_capacity(runs.len());
        for (i, r) in runs.iter().enumerate() {
            let k = case.run(i)?;
            progress(i + 1, runs.len(), &r.label, k);
            results.push(k);
        }
        let value = case.coefficients(&results)?;
        self.live = Some(CoefficientSource::Live { value, runs: runs.into_iter().zip(results).collect() });
        Ok(self.live.as_ref().expect("set just above"))
    }

    /// Hand a coefficient set to the plant and make it the set in use.
    pub fn use_coefficients(&mut self, source: CoefficientSource<M::Coefficients>) -> Result<(), TwinError> {
        self.plant.set_coefficients(source.value())?;
        self.in_use = source;
        Ok(())
    }

    /// "Send to plant": the latest live Monte Carlo set becomes the set in
    /// use. Returns `false` (and changes nothing) when there is none.
    pub fn send_to_plant(&mut self) -> Result<bool, TwinError> {
        match self.live.take() {
            None => Ok(false),
            Some(s) => {
                self.use_coefficients(s)?;
                Ok(true)
            }
        }
    }

    /// Run a multiphysics case at `resolution` (blocking).
    pub fn run_multiphysics(&mut self, resolution: MeshResolution) -> Result<F::Result, TwinError> {
        self.page_status(resolution.page())?;
        self.multiphysics.as_mut().expect("page_status checked it").run(resolution)
    }

    // ---------------------------------------------------------- headless

    /// The plant trace as CSV: apply each `(time, command)` of `script`
    /// when plant time reaches it, step by `dt` to `end`, one row per step
    /// (and the initial row). Deterministic: no clock, no thread.
    pub fn plant_csv(&mut self, script: &[(Time, P::Command)], dt: Time, end: Time) -> Result<String, TwinError> {
        if !(dt.get::<second>() > 0.0) {
            return Err(TwinError::BadArgument(format!("time step {} s is not positive", dt.get::<second>())));
        }
        let mut out = P::Snapshot::columns().join(",");
        out.push('\n');
        let row = |s: &P::Snapshot| s.values().iter().map(|v| format!("{v:.9e}")).collect::<Vec<_>>().join(",") + "\n";
        out.push_str(&row(&self.plant.snapshot()));
        let mut t = Time::new::<second>(0.0);
        let mut next = 0;
        let steps = (end.get::<second>() / dt.get::<second>()).round() as usize;
        for _ in 0..steps {
            while next < script.len() && script[next].0 <= t {
                self.plant.apply(script[next].1.clone())?;
                next += 1;
            }
            let s = self.plant.advance(dt)?;
            t += dt;
            out.push_str(&row(&s));
        }
        Ok(out)
    }

    /// `--dump-cvs`: the control volumes after advancing `steps` steps of
    /// `dt` from the current state.
    pub fn dump_cvs(&mut self, steps: usize, dt: Time) -> Result<String, TwinError> {
        for _ in 0..steps {
            self.plant.advance(dt)?;
        }
        Ok(self.control_volumes().to_csv())
    }

    /// `--coefficients`: run the Monte Carlo case, send it to the plant,
    /// and report the runs, the coefficients and the source in use.
    pub fn coefficient_report(&mut self) -> Result<String, TwinError> {
        let mut out = format!("before: {}\n", self.in_use.provenance());
        let mut lines = Vec::new();
        self.run_monte_carlo(|done, total, label, k| {
            lines.push(format!("run {done}/{total} {label}: k_eff = {:.6} +/- {:.6}", k.k, k.sigma));
        })?;
        for l in lines {
            out.push_str(&l);
            out.push('\n');
        }
        self.send_to_plant()?;
        let case = self.monte_carlo.as_ref().expect("ran above");
        for l in case.describe(self.in_use.value()) {
            out.push_str(&l);
            out.push('\n');
        }
        out.push_str(&format!("after: {}\n", self.in_use.provenance()));
        Ok(out)
    }

    /// The headless entry point. `--dump-cvs [steps]` dumps the control
    /// volumes, `--coefficients` runs the Monte Carlo case, and otherwise
    /// the plant CSV runs `script` to `--seconds <s>` (default `end`).
    pub fn headless(
        &mut self,
        args: &[String],
        script: &[(Time, P::Command)],
        dt: Time,
        end: Time,
    ) -> Result<String, TwinError> {
        let after = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1));
        if args.iter().any(|a| a == "--dump-cvs") {
            let steps = match after("--dump-cvs") {
                Some(s) if !s.starts_with("--") => s.parse::<usize>().map_err(|_| TwinError::BadArgument(format!("--dump-cvs {s}")))?,
                _ => 0,
            };
            return self.dump_cvs(steps, dt);
        }
        if args.iter().any(|a| a == "--coefficients") {
            return self.coefficient_report();
        }
        let end = match after("--seconds") {
            Some(s) => Time::new::<second>(s.parse::<f64>().map_err(|_| TwinError::BadArgument(format!("--seconds {s}")))?),
            None => end,
        };
        self.plant_csv(script, dt, end)
    }
}

#[cfg(test)]
mod tests;
