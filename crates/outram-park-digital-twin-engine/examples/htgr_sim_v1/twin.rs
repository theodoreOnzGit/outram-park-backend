//! # HTR-10 on the multifidelity twin template (gh:#1750, epic gh:#1743)
//!
//! `htgr_sim_v1`'s plant as a [`MultiFidelityTwin`]: the plant is
//! [`HtgrPlant`] stepped at [`PLANT_TIMESTEP_S`], unchanged; the control
//! volumes are its own map (`physics::control_volumes`, gh:#1718),
//! converted entry for entry; the Monte Carlo case is the whole-core
//! `nee_soon` model on the poured DEM bed (gh:#1739), run case by case
//! through `nee_soon::htr10_rmc::parametric::run_worth_case`. **No physics
//! here and nothing re-implemented**: this file adapts types.
//!
//! | Page | HTR-10 |
//! |---|---|
//! | Plant | [`HtgrPlant`], pinned bit for bit against a bare plant |
//! | Control volumes | `control_volume_map(&plant)`, every entry and link |
//! | Coarse / fine multiphysics | not built yet: the GeN-Foam backend is gh:#1746 |
//! | Monte Carlo | [`Htr10PouredBed`]: BASE, KERNEL, FUEL-GRAPHITE, DUMMIES, RODS-IN on the poured bed, gh:#1174's statistics |
//!
//! **Coefficients** ([`Htr10Coefficients`]): fuel and moderator temperature
//! coefficients and the rod-bank worth. The cited set is the plant's own
//! default (Chen 2009 total split by Hu 2006's share; TECDOC-1382's VSOP
//! bank worth). A live set is computed with `α = (1/k₀ − 1/k₁)/ΔT`
//! (`twin::reactivity_coefficient`): the fuel from KERNEL, the moderator
//! from FUEL-GRAPHITE plus DUMMIES, and the bank worth `1/k_in − 1/k_base`.
//! It reaches the plant only on "send to plant", through
//! `HtgrKinetics::set_temperature_coefficients` and
//! `HtgrPlant::set_bank_worth_dk_k` (gh:#1740), which keep the state.
//!
//! The coefficients are 600 → 1200 K secants (gh:#1174's protocol), used by
//! the plant as its coefficients near operating temperature; the poured
//! bed is one pour (#798's scatter is not measured).
//!
//! Research, education and V&V only. NOT VALIDATED. AI draft (gh:#1750).

use outram_park_digital_twin_engine::twin::{
    reactivity_coefficient, CoefficientSource, ControlVolume, ControlVolumeMap, ControlVolumes, CvLink, CvLinkKind,
    CvRole, FeedbackCoupling, KeffResult, KeffRun, MonteCarloCase, MultiFidelityTwin, NoMultiphysics, PlantModel,
    TwinError, TwinSnapshot,
};
use uom::si::f64::{MassRate, TemperatureCoefficient, TemperatureInterval, ThermodynamicTemperature, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::megawatt;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::temperature_interval::kelvin as delta_k;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use crate::physics::control_volumes::{self as cv, control_volume_map};
use crate::physics::{control_rods, HtgrPlant, PlantCommands, PLANT_TIMESTEP_S};

/// The issue the multiphysics pages name.
pub const MULTIPHYSICS_ISSUE: u32 = 1746;
/// The issue behind the Monte Carlo case and its record.
pub const MONTE_CARLO_ISSUE: u32 = 1739;
/// The cited set's citation.
pub const CITED: &str =
    "Chen et al. 2009 isothermal -1.4e-4 /K split by Hu et al. 2006's fuel share 0.7117; bank worth 15.24 % dk/k (IAEA-TECDOC-1382, VSOP)";

/// The HTR-10 twin: plant, the poured-bed case, no multiphysics yet.
pub type Htr10Twin = MultiFidelityTwin<Htr10Plant, Htr10PouredBed, NoMultiphysics>;

/// What the plant's feedback and rods run on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Htr10Coefficients {
    pub fuel: TemperatureCoefficient,
    pub moderator: TemperatureCoefficient,
    /// Bank worth, dimensionless `dk/k`.
    pub bank_worth_dk_k: f64,
}

impl Htr10Coefficients {
    /// The plant's default set (module docs).
    pub fn cited() -> Self {
        let (fuel, moderator) = HtgrPlant::new().kinetics.temperature_coefficients();
        Self { fuel, moderator, bank_worth_dk_k: control_rods::bank_worth_dk_k() }
    }

    /// One line per quantity, in pcm/K and % dk/k.
    pub fn describe(&self) -> Vec<String> {
        vec![
            format!("fuel {:.3} pcm/K", self.fuel.get::<per_kelvin>() * 1e5),
            format!("moderator {:.3} pcm/K", self.moderator.get::<per_kelvin>() * 1e5),
            format!("rod bank {:.3} % dk/k", self.bank_worth_dk_k * 100.0),
        ]
    }
}

/// An operator command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Htr10Command {
    /// Rod-bank insertion fraction, 0 withdrawn to 1 inserted.
    Rods(f64),
    /// Helium flow set point, kg/s.
    HeliumFlowKgS(f64),
}

/// The plant now, in plain numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Htr10Snapshot {
    pub time_s: f64,
    pub power_mw: f64,
    pub fuel_k: f64,
    pub bed_k: f64,
    pub reactivity_dollars: f64,
    pub rods: f64,
    pub helium_kg_s: f64,
}

impl TwinSnapshot for Htr10Snapshot {
    fn columns() -> Vec<&'static str> {
        vec!["time_s", "power_mw", "fuel_k", "bed_k", "reactivity_dollars", "rod_insertion", "helium_kg_s"]
    }
    fn values(&self) -> Vec<f64> {
        vec![self.time_s, self.power_mw, self.fuel_k, self.bed_k, self.reactivity_dollars, self.rods, self.helium_kg_s]
    }
}

/// The plant and the operator's standing commands.
pub struct Htr10Plant {
    plant: HtgrPlant,
    commands: PlantCommands,
}

impl Htr10Plant {
    /// The wrapped plant.
    pub fn plant(&self) -> &HtgrPlant {
        &self.plant
    }
}

impl PlantModel for Htr10Plant {
    type Config = PlantCommands;
    type Command = Htr10Command;
    type Snapshot = Htr10Snapshot;

    /// The plant at its opening state under `commands` (the GUI's and the
    /// headless mode's default is `PlantCommands::default()`).
    fn build(commands: PlantCommands) -> Result<Self, TwinError> {
        Ok(Htr10Plant { plant: HtgrPlant::new(), commands })
    }

    fn apply(&mut self, c: Htr10Command) -> Result<(), TwinError> {
        match c {
            Htr10Command::Rods(x) if (0.0..=1.0).contains(&x) => self.commands.control_rod_insertion_fraction = x,
            Htr10Command::HeliumFlowKgS(m) if m.is_finite() && m >= 0.0 => {
                self.commands.helium_flow_setpoint = MassRate::new::<kilogram_per_second>(m)
            }
            other => return Err(TwinError::Plant(format!("out of range: {other:?}"))),
        }
        Ok(())
    }

    /// `dt` as whole plant steps of [`PLANT_TIMESTEP_S`] (at least one).
    fn advance(&mut self, dt: Time) -> Result<Htr10Snapshot, TwinError> {
        let n = (dt.get::<second>() / PLANT_TIMESTEP_S).round().max(1.0) as usize;
        let step = Time::new::<second>(PLANT_TIMESTEP_S);
        for _ in 0..n {
            self.plant.step(step, self.commands.clone());
        }
        Ok(self.snapshot())
    }

    fn snapshot(&self) -> Htr10Snapshot {
        let p = &self.plant;
        Htr10Snapshot {
            time_s: p.sim_time.get::<second>(),
            power_mw: p.kinetics.total_power().get::<megawatt>(),
            fuel_k: p.kinetics.fuel_temperature().get::<kelvin>(),
            bed_k: p.core.temperature().get::<kelvin>(),
            reactivity_dollars: p.kinetics.net_reactivity_dollars(),
            rods: self.commands.control_rod_insertion_fraction,
            helium_kg_s: p.primary.mass_flow().get::<kilogram_per_second>(),
        }
    }
}

fn role(r: cv::CvRole) -> CvRole {
    match r {
        cv::CvRole::Balance => CvRole::Balance,
        cv::CvRole::Junction => CvRole::Junction,
        cv::CvRole::Inventory => CvRole::Inventory,
        cv::CvRole::NoVolume => CvRole::NoVolume,
        cv::CvRole::Boundary => CvRole::Boundary,
    }
}

impl ControlVolumeMap for Htr10Plant {
    /// gh:#1718's map, entry for entry; the medium is its `Medium` name.
    fn control_volumes(&self) -> ControlVolumes {
        let map = control_volume_map(&self.plant);
        ControlVolumes {
            volumes: map
                .volumes
                .iter()
                .map(|v| ControlVolume {
                    id: v.id.clone(),
                    name: v.name.clone(),
                    medium: format!("{:?}", v.medium),
                    role: role(v.role),
                    volume: v.volume,
                    temperature: v.temperature,
                    mass: v.mass,
                    on_pressure_boundary: v.inside_primary_boundary,
                })
                .collect(),
            links: map
                .links
                .iter()
                .map(|l| CvLink {
                    from: l.from.clone(),
                    to: l.to.clone(),
                    kind: match l.kind {
                        cv::LinkKind::Flow(m) => CvLinkKind::Flow(m),
                        cv::LinkKind::Heat(p) => CvLinkKind::Heat(p),
                    },
                })
                .collect(),
        }
    }
}

impl FeedbackCoupling<Htr10Coefficients> for Htr10Plant {
    fn set_coefficients(&mut self, c: &Htr10Coefficients) -> Result<(), TwinError> {
        self.plant.kinetics.set_temperature_coefficients(Some((c.fuel, c.moderator))).map_err(TwinError::Plant)?;
        self.plant.set_bank_worth_dk_k(Some(c.bank_worth_dk_k)).map_err(TwinError::Plant)
    }
}

/// The whole-core Monte Carlo case on the poured bed (gh:#1739's runs).
#[derive(Clone, Debug, PartialEq)]
pub struct Htr10PouredBed {
    /// The DEM CSV's path.
    pub dem_csv: String,
    pub base_k: f64,
    pub delta_k: f64,
    pub particles: usize,
    pub inactive: usize,
    pub active: usize,
    pub threads: usize,
}

impl Htr10PouredBed {
    /// gh:#1174's protocol and statistics on the record's pour.
    pub fn record() -> Self {
        Self {
            dem_csv: "reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv".into(),
            base_k: 600.0,
            delta_k: 600.0,
            particles: 3000,
            inactive: 20,
            active: 80,
            threads: std::thread::available_parallelism().map_or(1, |n| n.get()),
        }
    }

    /// The case names, in [`MonteCarloCase::runs`] order.
    pub const CASES: [&'static str; 5] = ["BASE", "KERNEL", "FUEL-GRAPHITE", "DUMMIES", "RODS-IN"];
}

impl MonteCarloCase for Htr10PouredBed {
    type Coefficients = Htr10Coefficients;

    fn runs(&self) -> Vec<KeffRun> {
        let d = TemperatureInterval::new::<delta_k>(self.delta_k);
        let zero = TemperatureInterval::new::<delta_k>(0.0);
        let run = |name: &str, region: Option<&str>, delta| KeffRun { label: format!("{name} (poured bed)"), region: region.map(String::from), delta };
        vec![
            run("BASE", None, zero),
            run("KERNEL", Some("kernels and coatings"), d),
            run("FUEL-GRAPHITE", Some("fuel-pebble graphite"), d),
            run("DUMMIES", Some("dummy pebbles"), d),
            run("RODS-IN", Some("all ten rods inserted"), zero),
        ]
    }

    fn run(&mut self, index: usize) -> Result<KeffResult, TwinError> {
        use nee_soon::htr10_rmc::parametric::{graphite_worth_cases, poured_bed, rods_in_case, run_worth_case};
        use nee_soon::htr10_rmc::keff_vs_height::SweepStatistics;
        let name = *Self::CASES.get(index).ok_or_else(|| TwinError::MonteCarlo(format!("no run {index}")))?;
        let err = TwinError::MonteCarlo;
        let text = std::fs::read_to_string(&self.dem_csv).map_err(|e| err(format!("{}: {e}", self.dem_csv)))?;
        let bed = poured_bed(&text).map_err(err)?;
        let base = ThermodynamicTemperature::new::<kelvin>(self.base_k);
        let case = if name == "RODS-IN" {
            rods_in_case(base)
        } else {
            graphite_worth_cases(base, self.delta_k).into_iter().find(|c| c.name == name).ok_or_else(|| err(format!("no case {name}")))?
        };
        let stats = SweepStatistics { particles: self.particles, inactive: self.inactive, active: self.active, ..SweepStatistics::QUICK };
        let r = run_worth_case(&bed, &case, stats, self.threads, |_| {}).map_err(err)?;
        Ok(KeffResult { k: r.k_mean, sigma: r.k_std })
    }

    fn coefficients(&self, r: &[KeffResult]) -> Result<Htr10Coefficients, TwinError> {
        let [base, kernel, fuel_graphite, dummies, rods] = r else {
            return Err(TwinError::MonteCarlo(format!("the poured-bed case needs 5 runs, got {}", r.len())));
        };
        let d = TemperatureInterval::new::<delta_k>(self.delta_k);
        let fuel = reactivity_coefficient(*base, *kernel, d).value;
        let moderator = reactivity_coefficient(*base, *fuel_graphite, d).value + reactivity_coefficient(*base, *dummies, d).value;
        let bank_worth_dk_k = 1.0 / rods.k - 1.0 / base.k;
        Ok(Htr10Coefficients { fuel, moderator, bank_worth_dk_k })
    }

    fn describe(&self, c: &Htr10Coefficients) -> Vec<String> {
        c.describe()
    }
}

/// The HTR-10 twin under `commands`, on the cited set, with the poured-bed
/// case attached.
pub fn htr10_twin(commands: PlantCommands, case: Htr10PouredBed) -> Result<Htr10Twin, TwinError> {
    let plant = Htr10Plant::build(commands)?;
    let cited = CoefficientSource::Cited { value: Htr10Coefficients::cited(), citation: CITED.into() };
    Ok(MultiFidelityTwin::new(plant, cited, MONTE_CARLO_ISSUE, MULTIPHYSICS_ISSUE)?.with_monte_carlo(case))
}

/// `--coefficients` (CLI glue): the poured-bed case run and sent to the
/// plant, through the template's `coefficient_report` (tested in the
/// library).
pub fn coefficient_report() -> Result<String, TwinError> {
    htr10_twin(PlantCommands::default(), Htr10PouredBed::record())?.coefficient_report()
}

/// The operator's controls on the twin's panel (GUI code).
#[derive(Default)]
struct Controls {
    rods: f64,
}

impl outram_park_digital_twin_engine::twin::local::PlantControls<Htr10Plant> for Controls {
    fn controls(&mut self, ui: &mut egui::Ui, latest: Option<&Htr10Snapshot>) -> Vec<Htr10Command> {
        if let Some(s) = latest {
            self.rods = if self.rods == 0.0 { s.rods } else { self.rods };
            ui.label(format!("t = {:.0} s · {:.2} MW · bed {:.0} K", s.time_s, s.power_mw, s.bed_k));
        }
        let mut out = Vec::new();
        if ui.add(egui::Slider::new(&mut self.rods, 0.0..=1.0).text("rod insertion")).changed() {
            out.push(Htr10Command::Rods(self.rods));
        }
        out
    }
}

/// `--twin` (GUI glue): the template's app on the HTR-10 twin, the plant on
/// its own thread at real time.
pub fn gui() -> eframe::Result<()> {
    use outram_park_digital_twin_engine::twin::{app::TwinApp, local::LocalHost};
    struct Window(TwinApp<LocalHost<Htr10Plant, Htr10PouredBed, NoMultiphysics, Controls>>);
    impl eframe::App for Window {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            self.0.ui(ui);
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    let twin = match htr10_twin(PlantCommands::default(), Htr10PouredBed::record()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let host = LocalHost::start(twin, Controls::default(), "HTR-10 twin".into(), Time::new::<second>(PLANT_TIMESTEP_S), 1.0);
    eframe::run_native("HTR-10 twin", eframe::NativeOptions::default(), Box::new(|_| Ok(Box::new(Window(TwinApp::new(host))))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_park_digital_twin_engine::twin::TwinPage;

    /// **gh:#1750: the plant on the template is the plant, bit for bit.**
    /// 1200 steps (120 s) from the opening state under the default
    /// commands, rods to 0.5 at 60 s, through the twin and on a bare
    /// `HtgrPlant`; the cited set is sent to the twin's plant at build, so
    /// this also pins that re-sending it changes nothing.
    #[test]
    fn the_twin_plant_is_the_plant_bit_for_bit() {
        let mut twin = htr10_twin(PlantCommands::default(), Htr10PouredBed::record()).expect("twin");
        let mut bare = HtgrPlant::new();
        let mut c = PlantCommands::default();
        let dt = Time::new::<second>(PLANT_TIMESTEP_S);
        for step in 0..1200 {
            if step == 600 {
                twin.apply(Htr10Command::Rods(0.5)).expect("rods");
                c.control_rod_insertion_fraction = 0.5;
            }
            let s = twin.advance(dt).expect("step");
            bare.step(dt, c.clone());
            assert_eq!(s.power_mw.to_bits(), bare.kinetics.total_power().get::<megawatt>().to_bits(), "step {step}");
            assert_eq!(s.bed_k.to_bits(), bare.core.temperature().get::<kelvin>().to_bits(), "step {step}");
        }
        assert!(matches!(twin.apply(Htr10Command::Rods(1.5)), Err(TwinError::Plant(_))));
        assert!(matches!(twin.apply(Htr10Command::HeliumFlowKgS(f64::NAN)), Err(TwinError::Plant(_))));
        twin.apply(Htr10Command::HeliumFlowKgS(4.32)).expect("helium");
    }

    /// The CV page is gh:#1718's map entry for entry, with no dangling link;
    /// the missing multiphysics pages name gh:#1746; the cited source is
    /// named; the poured-bed coefficients follow from the five runs.
    #[test]
    fn the_cvs_sources_and_coefficients_are_the_plants() {
        let twin = htr10_twin(PlantCommands::default(), Htr10PouredBed::record()).expect("twin");
        let cvs = twin.control_volumes();
        let map = control_volume_map(twin.plant().plant());
        assert_eq!(cvs.volumes.len(), map.volumes.len());
        assert_eq!(cvs.links.len(), map.links.len());
        assert!(cvs.dangling_links().is_empty());
        for (a, b) in cvs.volumes.iter().zip(&map.volumes) {
            assert_eq!((a.id.as_str(), a.temperature, a.on_pressure_boundary), (b.id.as_str(), b.temperature, b.inside_primary_boundary));
        }
        for r in [cv::CvRole::Balance, cv::CvRole::Junction, cv::CvRole::Inventory, cv::CvRole::NoVolume, cv::CvRole::Boundary] {
            assert_eq!(format!("{:?}", role(r)), format!("{r:?}"));
        }
        assert_eq!(twin.page_status(TwinPage::CoarseMultiphysics), Err(TwinError::NotBuilt { page: TwinPage::CoarseMultiphysics, issue: 1746 }));
        assert!(twin.coefficients().provenance().starts_with("cited: Chen et al. 2009"));
        let cited = Htr10Coefficients::cited();
        assert_eq!(cited.describe().len(), 3);
        assert!((cited.bank_worth_dk_k - 0.1524).abs() < 1e-15);

        let case = Htr10PouredBed::record();
        assert_eq!(case.runs().len(), Htr10PouredBed::CASES.len());
        let k = |k: f64| KeffResult { k, sigma: 0.002 };
        // A synthetic set (not data): the coefficients are the definitions.
        let r = [k(0.95), k(0.94), k(0.90), k(0.92), k(0.80)];
        let c = case.coefficients(&r).expect("five runs");
        let per = |a: f64, b: f64| (1.0 / a - 1.0 / b) / 600.0;
        assert!((c.fuel.get::<per_kelvin>() - per(0.95, 0.94)).abs() < 1e-18);
        assert!((c.moderator.get::<per_kelvin>() - per(0.95, 0.90) - per(0.95, 0.92)).abs() < 1e-18);
        assert!((c.bank_worth_dk_k - (1.0 / 0.80 - 1.0 / 0.95)).abs() < 1e-15);
        assert!(case.coefficients(&r[..4]).is_err());
        let mut bad = case.clone();
        assert!(bad.run(9).is_err());
        bad.dem_csv = "no/such/file.csv".into();
        assert!(bad.run(0).is_err());
    }

    /// A poured-bed-like set (synthetic numbers of gh:#1174's size, not
    /// the record's) reaches the plant through the twin and the plant
    /// reports it back.
    #[test]
    fn a_set_reaches_the_plant() {
        let mut twin = htr10_twin(PlantCommands::default(), Htr10PouredBed::record()).expect("twin");
        let set = Htr10Coefficients {
            fuel: TemperatureCoefficient::new::<per_kelvin>(-2.5e-5),
            moderator: TemperatureCoefficient::new::<per_kelvin>(-16.7e-5),
            bank_worth_dk_k: 0.176,
        };
        twin.use_coefficients(CoefficientSource::Committed { value: set, record: "synthetic".into() }).expect("set");
        assert_eq!(twin.plant().plant().kinetics.temperature_coefficients(), (set.fuel, set.moderator));
        assert_eq!(twin.plant().plant().bank_worth_dk_k(), 0.176);
    }
}
