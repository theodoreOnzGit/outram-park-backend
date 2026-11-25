//! Tests of the twin template on a small test plant written for them.
//!
//! The test plant is NOT a reactor model and its numbers are not data: a
//! one-node core whose power follows a prompt-jump reactivity balance and
//! whose temperature follows one heat balance, with round illustrative
//! constants. It exists so the template's wiring (pages, coefficient
//! sources, send-to-plant, the headless outputs) can be checked with no
//! real model behind it.

use super::*;
use uom::si::f64::{Mass, Power, TemperatureInterval, ThermodynamicTemperature, Time, Volume};
use uom::si::mass::kilogram;
use uom::si::power::megawatt;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::temperature_interval::kelvin as dk;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volume::cubic_meter;

/// Illustrative test-plant constants (not data).
const T_REF_K: f64 = 900.0;
const T_COOLANT_K: f64 = 800.0;
const P_NOMINAL_MW: f64 = 10.0;
const BETA: f64 = 0.007;
/// Heat capacity [MJ/K] and removal conductance [MW/K]: settles at 900 K.
const C_MJ_PER_K: f64 = 50.0;
const H_MW_PER_K: f64 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Alphas {
    fuel: f64,
    moderator: f64,
}

#[derive(Clone, Debug)]
enum Cmd {
    Rod(f64),
}

#[derive(Clone, Debug, PartialEq)]
struct Snap {
    t: f64,
    power_mw: f64,
    temperature_k: f64,
    rho: f64,
}

impl TwinSnapshot for Snap {
    fn columns() -> Vec<&'static str> {
        vec!["time_s", "power_mw", "temperature_k", "reactivity"]
    }
    fn values(&self) -> Vec<f64> {
        vec![self.t, self.power_mw, self.temperature_k, self.rho]
    }
}

struct TestPlant {
    t: f64,
    temperature_k: f64,
    rod: f64,
    alpha: Alphas,
}

impl TestPlant {
    fn rho(&self) -> f64 {
        self.rod + (self.alpha.fuel + self.alpha.moderator) * (self.temperature_k - T_REF_K)
    }
    /// Prompt jump, P = P₀ β/(β − ρ), held at zero above prompt critical
    /// (the test plant never goes there).
    fn power_mw(&self) -> f64 {
        let rho = self.rho();
        if rho >= BETA { 0.0 } else { P_NOMINAL_MW * BETA / (BETA - rho) }
    }
}

impl PlantModel for TestPlant {
    type Config = f64;
    type Command = Cmd;
    type Snapshot = Snap;
    fn build(initial_k: f64) -> Result<Self, TwinError> {
        if !(initial_k > 0.0) {
            return Err(TwinError::Plant(format!("initial temperature {initial_k} K")));
        }
        Ok(TestPlant { t: 0.0, temperature_k: initial_k, rod: 0.0, alpha: Alphas { fuel: 0.0, moderator: 0.0 } })
    }
    fn apply(&mut self, c: Cmd) -> Result<(), TwinError> {
        match c {
            Cmd::Rod(r) if r.abs() < BETA => {
                self.rod = r;
                Ok(())
            }
            Cmd::Rod(r) => Err(TwinError::Plant(format!("rod {r} is prompt critical"))),
        }
    }
    fn advance(&mut self, dt: Time) -> Result<Snap, TwinError> {
        let h = dt.get::<second>();
        let q = self.power_mw() - H_MW_PER_K * (self.temperature_k - T_COOLANT_K);
        self.temperature_k += h * q / C_MJ_PER_K;
        self.t += h;
        Ok(self.snapshot())
    }
    fn snapshot(&self) -> Snap {
        Snap { t: self.t, power_mw: self.power_mw(), temperature_k: self.temperature_k, rho: self.rho() }
    }
}

impl ControlVolumeMap for TestPlant {
    fn control_volumes(&self) -> ControlVolumes {
        let flow = MassRate::new::<kilogram_per_second>(5.0);
        ControlVolumes {
            volumes: vec![
                ControlVolume {
                    id: "core".into(),
                    name: "Core".into(),
                    medium: "fuel".into(),
                    role: CvRole::Balance,
                    volume: Some(Volume::new::<cubic_meter>(1.5)),
                    temperature: Some(ThermodynamicTemperature::new::<kelvin>(self.temperature_k)),
                    mass: Some(Mass::new::<kilogram>(1000.0)),
                    on_pressure_boundary: true,
                },
                ControlVolume {
                    id: "sink".into(),
                    name: "Heat sink".into(),
                    medium: "coolant".into(),
                    role: CvRole::Boundary,
                    volume: None,
                    temperature: Some(ThermodynamicTemperature::new::<kelvin>(T_COOLANT_K)),
                    mass: None,
                    on_pressure_boundary: false,
                },
            ],
            links: vec![
                CvLink { from: "core".into(), to: "sink".into(), kind: CvLinkKind::Flow(flow) },
                CvLink {
                    from: "core".into(),
                    to: "sink".into(),
                    kind: CvLinkKind::Heat(Some(Power::new::<megawatt>(H_MW_PER_K * (self.temperature_k - T_COOLANT_K)))),
                },
            ],
        }
    }
}

impl FeedbackCoupling<Alphas> for TestPlant {
    fn set_coefficients(&mut self, c: &Alphas) -> Result<(), TwinError> {
        if c.fuel > 0.0 || c.moderator > 0.0 {
            return Err(TwinError::Plant("the test plant takes negative coefficients only".into()));
        }
        self.alpha = *c;
        Ok(())
    }
}

impl FeedbackCoupling<()> for TestPlant {
    fn set_coefficients(&mut self, _c: &()) -> Result<(), TwinError> {
        Ok(())
    }
}

/// An analytic "Monte Carlo" case: `1/k = 1/k₀ − α_r ΔT` for the region
/// raised, so the template must recover `α_r` to round-off.
#[derive(Clone)]
struct AnalyticMc {
    k0: f64,
    sigma: f64,
    truth: Alphas,
    delta_k: f64,
}

impl MonteCarloCase for AnalyticMc {
    type Coefficients = Alphas;
    fn runs(&self) -> Vec<KeffRun> {
        let d = TemperatureInterval::new::<dk>(self.delta_k);
        vec![
            KeffRun { label: "base".into(), region: None, delta: TemperatureInterval::new::<dk>(0.0) },
            KeffRun { label: "fuel raised".into(), region: Some("fuel".into()), delta: d },
            KeffRun { label: "moderator raised".into(), region: Some("moderator".into()), delta: d },
        ]
    }
    fn run(&mut self, index: usize) -> Result<KeffResult, TwinError> {
        let a = match index {
            0 => 0.0,
            1 => self.truth.fuel,
            2 => self.truth.moderator,
            _ => return Err(TwinError::MonteCarlo(format!("no run {index}"))),
        };
        Ok(KeffResult { k: 1.0 / (1.0 / self.k0 - a * self.delta_k), sigma: self.sigma })
    }
    fn coefficients(&self, r: &[KeffResult]) -> Result<Alphas, TwinError> {
        let runs = self.runs();
        if r.len() != runs.len() {
            return Err(TwinError::MonteCarlo(format!("{} results for {} runs", r.len(), runs.len())));
        }
        Ok(Alphas {
            fuel: reactivity_coefficient(r[0], r[1], runs[1].delta).value.get::<per_kelvin>(),
            moderator: reactivity_coefficient(r[0], r[2], runs[2].delta).value.get::<per_kelvin>(),
        })
    }
    fn describe(&self, c: &Alphas) -> Vec<String> {
        vec![format!("fuel {:.4e} /K", c.fuel), format!("moderator {:.4e} /K", c.moderator)]
    }
}

#[derive(Clone)]
struct TestMultiphysics;

impl MultiphysicsCase for TestMultiphysics {
    type Result = f64;
    fn availability(&self, r: MeshResolution) -> Availability {
        match r {
            MeshResolution::Coarse => Availability::Built(MultiphysicsBackend::GenFoam),
            MeshResolution::Fine => Availability::NotBuilt { issue: 1746 },
        }
    }
    fn cross_sections(&self) -> XsSource {
        XsSource::MonteCarlo
    }
    fn run(&mut self, _r: MeshResolution) -> Result<f64, TwinError> {
        Ok(1.25)
    }
    fn describe(&self, peaking: &f64) -> Vec<String> {
        vec![format!("radial peaking {peaking:.3} (test value)")]
    }
}

const CITED: Alphas = Alphas { fuel: -2.0e-5, moderator: -1.0e-5 };
const TRUTH: Alphas = Alphas { fuel: -4.0e-5, moderator: -3.0e-5 };

type Full = MultiFidelityTwin<TestPlant, AnalyticMc, TestMultiphysics>;

fn full() -> Full {
    full_at(T_REF_K)
}

/// The full twin with the core starting at `t_k` (900 K is the test
/// plant's equilibrium, where stepping changes nothing).
fn full_at(t_k: f64) -> Full {
    let plant = TestPlant::build(t_k).expect("plant");
    let cited = CoefficientSource::Cited { value: CITED, citation: "test plant, illustrative".into() };
    MultiFidelityTwin::new(plant, cited, 1744, 1746)
        .expect("twin")
        .with_monte_carlo(AnalyticMc { k0: 1.02, sigma: 1.0e-4, truth: TRUTH, delta_k: 50.0 })
        .with_multiphysics(TestMultiphysics)
}

/// **Methodology.** The template's coefficient is the definition
/// `α = (1/k₀ − 1/k₁)/ΔT`, the same arithmetic as MSRE's
/// `coefficient_from_pair`, which works per °F. On the MSRE N-3 record's
/// fuel pair (1.02232 → 1.01837 over 100 °F, σ = 0.0001 each, an
/// illustrative σ) the two must agree after the unit change (×1.8).
/// **Result (2026-11-25):** equal to round-off (pinned below 1e-12
/// relative); α = −6.83e-5 /K = −3.79e-5 /°F, the record's value.
#[test]
fn the_coefficient_is_msres_arithmetic_in_si() {
    let (base, raised) = (KeffResult { k: 1.02232, sigma: 1e-4 }, KeffResult { k: 1.01837, sigma: 1e-4 });
    let c = reactivity_coefficient(base, raised, TemperatureInterval::new::<dk>(100.0 / 1.8));
    let (a_f, s_f) = crate::msre::session::coefficient_from_pair(1.02232, 1e-4, 1.01837, 1e-4, 100.0);
    let a = c.value.get::<per_kelvin>();
    let s = c.sigma.get::<per_kelvin>();
    assert!(((a - 1.8 * a_f) / a).abs() < 1e-12, "{a} against {}", 1.8 * a_f);
    assert!(((s - 1.8 * s_f) / s).abs() < 1e-12, "{s} against {}", 1.8 * s_f);
    assert!((a_f + 3.79e-5).abs() < 0.005e-5, "{a_f}");
}

/// The cited set is applied to the plant at construction; a Monte Carlo
/// run recovers the analytic truth; it reaches the plant only on "send to
/// plant", and the source in use says so.
#[test]
fn a_live_run_reaches_the_plant_only_when_sent() {
    let mut twin = full();
    assert_eq!(twin.plant().alpha, CITED);
    assert!(twin.coefficients().provenance().starts_with("cited:"));
    assert_eq!(twin.send_to_plant(), Ok(false), "nothing to send before a run");

    let mut seen = Vec::new();
    let live = twin.run_monte_carlo(|done, total, label, _| seen.push(format!("{done}/{total} {label}"))).expect("run").clone();
    assert_eq!(seen, ["1/3 base", "2/3 fuel raised", "3/3 moderator raised"]);
    let got = *live.value();
    assert!(((got.fuel - TRUTH.fuel) / TRUTH.fuel).abs() < 1e-9, "{got:?}");
    assert!(((got.moderator - TRUTH.moderator) / TRUTH.moderator).abs() < 1e-9, "{got:?}");
    assert_eq!(twin.plant().alpha, CITED, "a run alone does not touch the plant");
    assert!(twin.live_coefficients().is_some());

    assert_eq!(twin.send_to_plant(), Ok(true));
    assert_eq!(twin.plant().alpha, got);
    assert_eq!(twin.coefficients().provenance(), "live Monte Carlo run, 3 k_eff runs this session");
    assert!(twin.live_coefficients().is_none());

    // A host that ran the transport elsewhere records the same results.
    let results: Vec<KeffResult> = match twin.coefficients() {
        CoefficientSource::Live { runs, .. } => runs.iter().map(|r| r.1).collect(),
        _ => unreachable!("sent above"),
    };
    assert!(twin.record_monte_carlo(results[..2].to_vec()).is_err(), "one result per run");
    assert_eq!(*twin.record_monte_carlo(results).expect("record").value(), got);
    assert_eq!(twin.send_to_plant(), Ok(true));
    let committed = CoefficientSource::Committed { value: TRUTH, record: "test record".into() };
    twin.use_coefficients(committed).expect("committed");
    assert_eq!(twin.coefficients().provenance(), "committed Monte Carlo record: test record");
    // The plant refuses a set it cannot take, and the set in use stays.
    let bad = CoefficientSource::Cited { value: Alphas { fuel: 1e-5, moderator: 0.0 }, citation: "bad".into() };
    assert!(matches!(twin.use_coefficients(bad), Err(TwinError::Plant(_))));
    assert_eq!(twin.plant().alpha, TRUTH);
}

/// The plant CSV is headed, has one row per step plus the initial row,
/// applies the script when plant time reaches it, and is byte-identical
/// run to run (the headless rule). Bounds: the temperature stays between
/// the coolant and 1500 K (a harness check, not validation).
#[test]
fn the_plant_csv_is_deterministic_and_follows_the_script() {
    let script = [(Time::new::<second>(10.0), Cmd::Rod(0.001))];
    let dt = Time::new::<second>(1.0);
    let end = Time::new::<second>(100.0);
    let a = full().plant_csv(&script, dt, end).expect("csv");
    let b = full().plant_csv(&script, dt, end).expect("csv");
    assert_eq!(a, b);
    let lines: Vec<&str> = a.lines().collect();
    assert_eq!(lines[0], "time_s,power_mw,temperature_k,reactivity");
    assert_eq!(lines.len(), 1 + 1 + 100);
    let col = |l: &str, i: usize| l.split(',').nth(i).unwrap().parse::<f64>().unwrap();
    assert_eq!(col(lines[10], 3), 0.0, "rod not yet in at t = 9 s");
    assert!(col(lines[12], 3) > 0.0, "rod in from t = 10 s");
    for l in &lines[1..] {
        let t = col(l, 2);
        assert!(t >= T_COOLANT_K && t < 1500.0, "{l}");
    }
    assert!(matches!(full().plant_csv(&script, Time::new::<second>(0.0), end), Err(TwinError::BadArgument(_))));
    let mut refused = full();
    assert!(matches!(refused.plant_csv(&[(Time::new::<second>(0.0), Cmd::Rod(0.01))], dt, end), Err(TwinError::Plant(_))));
}

/// The CV dump is read from the plant as it is: two headed blocks, no
/// dangling link, and the core temperature is the plant's.
#[test]
fn the_cv_dump_is_the_plant_now() {
    let mut twin = full();
    let cvs = twin.control_volumes();
    assert!(cvs.dangling_links().is_empty());
    assert_eq!(cvs.get("core").map(|v| v.role), Some(CvRole::Balance));
    let text = twin.dump_cvs(0, Time::new::<second>(1.0)).expect("dump");
    assert!(text.starts_with(ControlVolumes::VOLUME_HEADER));
    assert!(text.contains("\n\nfrom,to,kind,rate\n"));
    assert!(text.contains("core,Core,fuel,balance,1.500000,true,900.000,1000.000000\n"), "{text}");
    assert!(text.contains("sink,Heat sink,coolant,boundary,,false,800.000,\n"), "{text}");
    assert!(text.contains("core,sink,flow_kg_per_s,5.000000\n"));
    assert!(text.contains("core,sink,heat_w,10000000.000\n"));
    // Off equilibrium, the dump after stepping follows the plant.
    let mut cold = full_at(850.0);
    let later = cold.dump_cvs(50, Time::new::<second>(1.0)).expect("dump");
    let t = cold.plant().temperature_k;
    assert!(t > 850.0 && t < T_REF_K, "{t}");
    assert!(later.contains(&format!("core,Core,fuel,balance,1.500000,true,{t:.3},")), "{later}");
    let broken = ControlVolumes { volumes: vec![], links: cvs.links.clone() };
    assert_eq!(broken.dangling_links().len(), 2);
}

/// A page with no model says "not built" with its issue; a twin with only
/// a plant compiles with the empty types and refuses the other pages.
#[test]
fn missing_pages_say_not_built_with_their_issue() {
    let mut twin = full();
    assert_eq!(twin.page_status(TwinPage::Plant), Ok(()));
    assert_eq!(twin.page_status(TwinPage::ControlVolumes), Ok(()));
    assert_eq!(twin.page_status(TwinPage::MonteCarlo), Ok(()));
    assert_eq!(twin.page_status(TwinPage::Graphs), Ok(()));
    assert_eq!(twin.page_status(TwinPage::Cfd), Ok(()));
    assert_eq!(twin.resolution_status(MeshResolution::Coarse), Ok(()));
    assert_eq!(twin.resolution_status(MeshResolution::Fine), Err(TwinError::NotBuilt { page: TwinPage::Cfd, issue: 1746 }));
    // Kinetics and Surrogate are greyed until the adopter declares them
    // (gh:#1756); a later declaration replaces an earlier one.
    assert_eq!(twin.page_status(TwinPage::Kinetics), Err(TwinError::NotBuilt { page: TwinPage::Kinetics, issue: UNDECLARED_VIEW_ISSUE }));
    let mut twin = twin
        .with_view(TwinPage::Kinetics, ViewState::NotBuilt { issue: 42 })
        .with_view(TwinPage::Kinetics, ViewState::Built)
        .with_view(TwinPage::Surrogate, ViewState::NotBuilt { issue: 43 });
    assert_eq!(twin.page_status(TwinPage::Kinetics), Ok(()));
    assert_eq!(twin.page_status(TwinPage::Surrogate), Err(TwinError::NotBuilt { page: TwinPage::Surrogate, issue: 43 }));
    assert_eq!(twin.run_multiphysics(MeshResolution::Coarse), Ok(1.25));
    assert!(twin.run_multiphysics(MeshResolution::Fine).is_err());
    assert_eq!(twin.multiphysics().map(|m| m.cross_sections()), Some(XsSource::MonteCarlo));
    assert_eq!(twin.multiphysics().map(|m| m.describe(&1.25)), Some(vec!["radial peaking 1.250 (test value)".to_string()]));

    let plant = TestPlant::build(T_REF_K).expect("plant");
    let cited = CoefficientSource::Cited { value: (), citation: "none".into() };
    let mut bare: MultiFidelityTwin<TestPlant, NoMonteCarlo, NoMultiphysics> =
        MultiFidelityTwin::new(plant, cited, 1313, 1747).expect("twin");
    assert!(bare.monte_carlo().is_none() && bare.multiphysics().is_none());
    let mc = Err(TwinError::NotBuilt { page: TwinPage::MonteCarlo, issue: 1313 });
    assert_eq!(bare.page_status(TwinPage::MonteCarlo), mc);
    assert_eq!(bare.page_status(TwinPage::Cfd), Err(TwinError::NotBuilt { page: TwinPage::Cfd, issue: 1747 }));
    assert!(bare.run_monte_carlo(|_, _, _, _| {}).is_err());
    assert_eq!(bare.coefficient_report().unwrap_err().to_string(), "Monte Carlo, full core is not built yet (gh:#1313)");
    assert_eq!(bare.run_multiphysics(MeshResolution::Coarse).unwrap_err().to_string(), "CFD, core and primary loop is not built yet (gh:#1747)");
    bare.set_page(TwinPage::ControlVolumes);
    assert_eq!(bare.page(), TwinPage::ControlVolumes);
}

/// The headless entry point: `--dump-cvs [steps]`, `--coefficients`, and
/// the plant CSV with `--seconds`; bad numbers are refused.
#[test]
fn the_headless_flags_reach_each_output() {
    let args = |s: &[&str]| s.iter().map(|a| a.to_string()).collect::<Vec<_>>();
    let dt = Time::new::<second>(1.0);
    let end = Time::new::<second>(5.0);
    let csv = full().headless(&[], &[], dt, end).expect("csv");
    assert_eq!(csv.lines().count(), 1 + 1 + 5);
    let short = full().headless(&args(&["--seconds", "2"]), &[], dt, end).expect("csv");
    assert_eq!(short.lines().count(), 1 + 1 + 2);
    let cvs = full_at(850.0).headless(&args(&["--dump-cvs"]), &[], dt, end).expect("cvs");
    assert!(cvs.starts_with(ControlVolumes::VOLUME_HEADER));
    assert!(cvs.contains(",850.000,"), "{cvs}");
    let cvs_later = full_at(850.0).headless(&args(&["--dump-cvs", "20", "--x"]), &[], dt, end).expect("cvs");
    assert_ne!(cvs, cvs_later);
    let report = full().headless(&args(&["--coefficients"]), &[], dt, end).expect("report");
    assert!(report.starts_with("before: cited: test plant, illustrative\n"), "{report}");
    assert!(report.contains("run 2/3 fuel raised: k_eff = "), "{report}");
    assert!(report.contains("fuel -4.0000e-5 /K\n"), "{report}");
    assert!(report.ends_with("after: live Monte Carlo run, 3 k_eff runs this session\n"), "{report}");
    assert!(matches!(full().headless(&args(&["--seconds", "x"]), &[], dt, end), Err(TwinError::BadArgument(_))));
    assert!(matches!(full().headless(&args(&["--dump-cvs", "x"]), &[], dt, end), Err(TwinError::BadArgument(_))));
}

/// The fixed vocabularies the pages and CSVs print.
#[test]
fn the_page_and_role_words_are_fixed() {
    let tabs: Vec<&str> = TwinPage::ALL.iter().map(|p| p.tab_label()).collect();
    assert_eq!(tabs, ["Plant", "Graphs", "Kinetics", "Freq.", "Nodes", "CFD", "MC"]);
    let titles: Vec<&str> = TwinPage::ALL.iter().map(|p| p.title()).collect();
    assert_eq!(
        titles,
        [
            "Plant",
            "Graphs",
            "Moltres / burnup / precursors",
            "Frequency response / surrogate models",
            "System-code nodalisation (control volumes)",
            "CFD, core and primary loop",
            "Monte Carlo, full core"
        ]
    );
    assert!(TwinPage::ALL.iter().enumerate().all(|(i, p)| p.index() == i));
    assert_eq!(MeshResolution::Coarse.page(), TwinPage::Cfd);
    assert_eq!([MeshResolution::Coarse.word(), MeshResolution::Fine.word()], ["coarse", "fine"]);
    assert!(ViewState::Built.is_built() && !ViewState::NotBuilt { issue: 1 }.is_built());
    let roles = [CvRole::Balance, CvRole::Junction, CvRole::Inventory, CvRole::NoVolume, CvRole::Boundary];
    assert_eq!(roles.map(|r| r.word()), ["balance", "junction", "inventory", "no-volume", "boundary"]);
    assert_eq!(MeshResolution::Fine.page(), TwinPage::Cfd);
    assert_eq!(MultiphysicsBackend::Bedok.name(), "bedok");
    assert_eq!(MultiphysicsBackend::GenFoam.name(), "GeN-Foam port");
    assert_eq!(TwinError::Plant("x".into()).to_string(), "plant model: x");
    assert_eq!(TwinError::MonteCarlo("x".into()).to_string(), "Monte Carlo: x");
    assert_eq!(TwinError::Multiphysics("x".into()).to_string(), "multiphysics: x");
    assert_eq!(TwinError::BadArgument("x".into()).to_string(), "bad argument: x");
    assert!(TestPlant::build(-1.0).is_err());
    let mut mc = AnalyticMc { k0: 1.0, sigma: 0.0, truth: TRUTH, delta_k: 1.0 };
    assert!(mc.run(9).is_err());
    assert!(mc.coefficients(&[]).is_err());
}

/// **The native host (gh:#1745).** The plant steps on its own thread
/// (history grows, a command reaches it); a Monte Carlo request runs on
/// another thread while the plant keeps stepping, its runs and live set
/// show on the page's data, and "send to plant" puts it in use; a
/// multiphysics request fills its page or says "not built"; dropping the
/// host stops the plant thread.
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
#[test]
fn the_local_host_runs_the_twin_off_the_ui_thread() {
    use super::app::{TwinAction, TwinHost};
    use super::local::{LocalHost, PlantControls};
    struct NoControls;
    impl PlantControls<TestPlant> for NoControls {
        fn controls(&mut self, _ui: &mut egui::Ui, _latest: Option<&Snap>) -> Vec<Cmd> {
            Vec::new()
        }
    }
    let wait = |what: &str, done: &dyn Fn() -> bool| {
        let t0 = std::time::Instant::now();
        while !done() {
            assert!(t0.elapsed().as_secs() < 30, "{what} not done in 30 s");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    };
    let mut host = LocalHost::start(full_at(850.0), NoControls, "test twin".into(), Time::new::<second>(0.002), 1.0);
    assert_eq!(host.title(), "test twin");
    assert_eq!(host.columns(), Snap::columns());
    wait("plant steps", &|| host.history().len() > 5);
    host.send(Cmd::Rod(0.001)).expect("rod");
    wait("rod reaches the plant", &|| host.history().last().is_some_and(|r| r[3] > 0.0));
    assert!(host.plant_error().is_none());
    assert!(host.monte_carlo().in_use.starts_with("cited:"));
    assert_eq!(host.monte_carlo().runs.len(), 3);

    host.act(TwinAction::RunMonteCarlo);
    wait("Monte Carlo", &|| !host.monte_carlo_running());
    let v = host.monte_carlo();
    assert!(v.error.is_none(), "{:?}", v.error);
    assert!(v.runs.iter().all(|r| r.result.is_some()));
    assert_eq!(v.live, vec!["fuel -4.0000e-5 /K".to_string(), "moderator -3.0000e-5 /K".to_string()]);
    host.act(TwinAction::SendToPlant);
    assert!(host.monte_carlo().in_use.starts_with("live Monte Carlo run"));
    let a = host.twin().read().unwrap().plant().alpha;
    assert!(((a.fuel - TRUTH.fuel) / TRUTH.fuel).abs() < 1e-9 && ((a.moderator - TRUTH.moderator) / TRUTH.moderator).abs() < 1e-9, "{a:?}");

    host.act(TwinAction::RunMultiphysics(MeshResolution::Coarse));
    wait("coarse multiphysics", &|| host.multiphysics(MeshResolution::Coarse) == vec!["radial peaking 1.250 (test value)".to_string()]);
    host.act(TwinAction::RunMultiphysics(MeshResolution::Fine));
    assert_eq!(host.multiphysics(MeshResolution::Fine), vec!["CFD, core and primary loop is not built yet (gh:#1746)".to_string()]);
    assert_eq!(host.page_status(TwinPage::Cfd), Ok(()));
    assert!(host.control_volumes().get("core").is_some());
    drop(host);
}
