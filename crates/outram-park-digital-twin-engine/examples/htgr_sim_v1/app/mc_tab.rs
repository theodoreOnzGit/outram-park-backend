//! # The MC page (gh:#1740, #1761, #1763; on the standard shell since gh:#1758)
//!
//! **The page runs the full HTR-10 Monte Carlo itself, natively and in the
//! browser** (maintainer, 2026-11-25: "I want the mc in htgrsimv1 to be a
//! full mc simulation built in, not a link. Like a copy exposing the htr10
//! mc simulation with Dem pour"). Since gh:#1763 it hosts the Monte Carlo
//! demo's app, `dhoby_ghaut::mc_demo::McApp`, opened on the HTR-10 whole
//! core (`?rung=htr10&view=core`): the DEM pour, the bed, the rod bank, the
//! worker pool and the k_eff and entropy plots. It is that demo's
//! simulation itself, not a second copy: one implementation, moved from the
//! `monte_carlo_web` example into dhoby-ghaut's library. The central view
//! is the demo's main view ([`McPage::main_view`]); the left panel holds
//! its controls, the coefficient sets and what this page measured
//! ([`McPage::controls`]). ~~In the browser an iframe of the demo page sat
//! over the central view (gh:#1761, `place_core_frame`); natively the page
//! showed only the coefficient sets.~~ Replaced by gh:#1763.
//!
//! **In the browser** the workers are the Monte Carlo demo's own
//! (`../monte-carlo/worker.js`, beside this page on the site): their tapes
//! come from that page's `data/`, and the processed-data cache is the one
//! the demo fills (the same origin). **Natively** they are threads, and the
//! tapes come from `reference-data/endf/`.
//!
//! **Send to plant (gh:#1763).** Each finished whole-core run is recorded
//! (`FinishedRun`: k ± σ, the eight zone temperatures, the rods, the bed).
//! The reader picks a base run and a second run; [`classify`] says what the
//! pair measures, and [`plant_set`] turns the measurements into the set the
//! plant takes:
//!
//! - **the bed coefficient**, from a pair on the same bed and rods whose
//!   only change is the fuel and moderator pebbles raised by the same ΔT
//!   (or the sum of a fuel-pebble-only pair and a moderator-pebble-only
//!   pair). α = (1/k₀ − 1/k₁)/ΔT, the twin's
//!   `twin::reactivity_coefficient`;
//! - **the bank worth**, from a pair at the same temperatures with every
//!   rod withdrawn and every rod inserted: 1/k_in − 1/k_out, the definition
//!   gh:#1739's record uses.
//!
//! **What the page cannot measure, said where it is used.** The plant's
//! fuel coefficient is the kernel's (Doppler), and its moderator
//! coefficient the graphite's, the fuel pebbles' included: gh:#1739's
//! record raised the kernel alone (its KERNEL case). The demo's sliders
//! raise the whole fuel pebble, kernel and graphite together, so a pair
//! here measures the bed's sum, not the split. The set sent therefore takes
//! the **fuel coefficient from the poured-bed record** and gives the
//! **moderator the measured bed coefficient minus it**; with no measured
//! worth, the worth is the record's too. Nothing is fitted: every number is
//! a measured pair or a committed record, and the set's source line names
//! which.
//!
//! GUI drawing code ([`McPage::controls`], [`McPage::main_view`]): exempt
//! from a reaching test (it needs a live egui frame). The arithmetic
//! ([`classify`], [`Measurements`], [`plant_set`]) is tested below.
//!
//! Research, education and V&V only. NOT VALIDATED.

use dhoby_ghaut::mc_demo::htr10::core::screen::FinishedRun;
use dhoby_ghaut::mc_demo::McApp;
use egui::{RichText, Ui};
use outram_park_digital_twin_engine::app_scaffold::SharedState;
use outram_park_digital_twin_engine::twin::{reactivity_coefficient, KeffResult};
use uom::si::f64::TemperatureInterval;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::temperature_interval::kelvin;

use crate::app::panels::coefficient_picker;
use crate::app::state::HtgrSnapshot;
use crate::physics::coefficient_set::{CoefficientSet, MeasuredSet, POURED_BED};

/// What the hosted app opens on: the Monte Carlo demo's HTR-10 whole core.
pub const CORE_QUERY: [(&str, &str); 2] = [("rung", "htr10"), ("view", "core")];

/// The Monte Carlo demo's worker script, relative to this page
/// (`demos/htgr/` beside `demos/monte-carlo/`). Browser only: natively the
/// workers are threads.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub const WORKER_JS: &str = "../monte-carlo/worker.js";

/// Zone indices in `Htr10Zone::ALL` order.
const FUEL_PEBBLE: usize = 0;
const MODERATOR_PEBBLE: usize = 1;

/// One measured quantity: value and σ.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measured {
    pub value: f64,
    pub sigma: f64,
}

/// What a pair of runs measures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PairKind {
    /// The fuel and moderator pebbles raised together: the bed coefficient
    /// \[1/K\].
    Bed(Measured),
    /// The fuel pebbles only (kernel and graphite) \[1/K\].
    FuelPebble(Measured),
    /// The moderator pebbles only \[1/K\].
    ModeratorPebble(Measured),
    /// All rods out against all in: the bank worth \[dk/k\].
    Bank(Measured),
}

/// Which quantity a pair of finished runs measures, or why it measures
/// none the plant takes. The two runs must be on the same bed (code and
/// pour) and differ in one thing only: the pebbles' temperatures, or the
/// rods all out against all in.
pub fn classify(base: &FinishedRun, other: &FinishedRun) -> Result<PairKind, String> {
    if base.bed != other.bed || base.pour_version != other.pour_version {
        return Err(format!("different beds: {} and {}", base.bed_label, other.bed_label));
    }
    let same_rods = base.rods == other.rods;
    let changed: Vec<usize> = (0..8).filter(|&z| base.temps_k[z] != other.temps_k[z]).collect();
    let k = |r: &FinishedRun| KeffResult { k: r.k, sigma: r.sigma };
    match (same_rods, changed.as_slice()) {
        (true, []) => Err("the same state twice: nothing changed between the runs".into()),
        (true, zones) => {
            let dt = other.temps_k[zones[0]] - base.temps_k[zones[0]];
            if zones.iter().any(|&z| other.temps_k[z] - base.temps_k[z] != dt) {
                return Err("the zones changed by different amounts".into());
            }
            let c = reactivity_coefficient(k(base), k(other), TemperatureInterval::new::<kelvin>(dt));
            let m = Measured { value: c.value.get::<per_kelvin>(), sigma: c.sigma.get::<per_kelvin>() };
            match zones {
                [FUEL_PEBBLE, MODERATOR_PEBBLE] => Ok(PairKind::Bed(m)),
                [FUEL_PEBBLE] => Ok(PairKind::FuelPebble(m)),
                [MODERATOR_PEBBLE] => Ok(PairKind::ModeratorPebble(m)),
                _ => Err("other zones changed too (coolant, reflectors, rods or bricks): the plant's coefficients are the bed's pebbles".into()),
            }
        }
        (false, []) => {
            let (out, inn) = match (base.rods, other.rods) {
                (b, o) if b.iter().all(|&r| r == 0.0) && o.iter().all(|&r| r == 1.0) => (base, other),
                (b, o) if b.iter().all(|&r| r == 1.0) && o.iter().all(|&r| r == 0.0) => (other, base),
                _ => return Err("the bank worth needs every rod withdrawn in one run and every rod inserted in the other".into()),
            };
            let value = 1.0 / inn.k - 1.0 / out.k;
            let (a, b) = (out.sigma / (out.k * out.k), inn.sigma / (inn.k * inn.k));
            Ok(PairKind::Bank(Measured { value, sigma: (a * a + b * b).sqrt() }))
        }
        (false, _) => Err("both the temperatures and the rods changed: change one at a time".into()),
    }
}

/// What the page has measured so far, the latest of each kind.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Measurements {
    pub bed: Option<Measured>,
    pub fuel_pebble: Option<Measured>,
    pub moderator_pebble: Option<Measured>,
    pub bank: Option<Measured>,
}

impl Measurements {
    /// Keep a pair's measurement (the latest of its kind wins).
    pub fn take(&mut self, kind: PairKind) {
        match kind {
            PairKind::Bed(m) => self.bed = Some(m),
            PairKind::FuelPebble(m) => self.fuel_pebble = Some(m),
            PairKind::ModeratorPebble(m) => self.moderator_pebble = Some(m),
            PairKind::Bank(m) => self.bank = Some(m),
        }
    }

    /// The bed coefficient: measured directly, else the fuel-pebble and
    /// moderator-pebble coefficients summed (independent runs: σ in
    /// quadrature).
    pub fn bed_total(&self) -> Option<Measured> {
        self.bed.or_else(|| {
            let (f, m) = (self.fuel_pebble?, self.moderator_pebble?);
            Some(Measured { value: f.value + m.value, sigma: (f.sigma * f.sigma + m.sigma * m.sigma).sqrt() })
        })
    }
}

/// The set the plant takes from the page's measurements, or why there is
/// none yet. Fuel: the poured-bed record's kernel coefficient (the page
/// cannot raise the kernel alone); moderator: the measured bed coefficient
/// minus it; worth: measured, else the record's. The record's numbers carry
/// no σ here (`POURED_BED` stores the values only), so the σ given is the
/// page's own.
pub fn plant_set(m: &Measurements) -> Result<MeasuredSet, String> {
    let bed = m.bed_total().ok_or("no bed coefficient yet: run the bed's pebbles at two temperatures")?;
    let (fuel, _, record_worth) = POURED_BED.ok_or("the poured-bed record is not in")?;
    let (worth, worth_sigma, worth_measured) = match m.bank {
        Some(b) => (b.value, b.sigma, true),
        None => (record_worth, 0.0, false),
    };
    Ok(MeasuredSet {
        fuel,
        moderator: bed.value - fuel,
        worth,
        fuel_sigma: 0.0,
        moderator_sigma: bed.sigma,
        worth_sigma,
        worth_measured,
    })
}

/// The MC page's state: the hosted demo (started the first time the page
/// is shown), the reader's pair and what it measured.
#[derive(Default)]
pub struct McPage {
    app: Option<McApp>,
    base: usize,
    other: usize,
    measured: Measurements,
    note: String,
}

impl McPage {
    /// Start the demo on the HTR-10 core if it is not running.
    fn app(&mut self, ctx: &egui::Context) -> &mut McApp {
        self.app.get_or_insert_with(|| {
            #[cfg(target_arch = "wasm32")]
            dhoby_ghaut::mc_demo::set_worker_js(WORKER_JS);
            McApp::hosted(ctx, CORE_QUERY.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
        })
    }

    /// Once a frame on every page once started: the demo's pools keep
    /// working whatever is on screen.
    pub fn pump(&mut self, ctx: &egui::Context) {
        if let Some(a) = self.app.as_mut() {
            a.pump(ctx);
        }
    }

    /// The central view: the demo's main view in `rect`.
    pub fn main_view(&mut self, ui: &mut Ui, rect: egui::Rect) {
        let ctx = ui.ctx().clone();
        let app = self.app(&ctx);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        child.set_clip_rect(rect);
        app.main_view(&mut child);
    }

    /// The left panel on the MC page: the demo's controls, the pair the
    /// reader measures with, "Send to plant", and the coefficient sets.
    pub fn controls(&mut self, ui: &mut Ui, physics: &SharedState<HtgrSnapshot>, snapshot: &HtgrSnapshot) {
        let ctx = ui.ctx().clone();
        egui::CollapsingHeader::new(RichText::new("Monte Carlo: HTR-10 whole core").strong()).default_open(true).show(ui, |ui| {
            self.app(&ctx).controls(ui);
        });
        ui.separator();
        self.measure_ui(ui, physics, snapshot);
        ui.separator();
        draw_sets(ui, physics, snapshot);
    }

    fn measure_ui(&mut self, ui: &mut Ui, physics: &SharedState<HtgrSnapshot>, snapshot: &HtgrSnapshot) {
        ui.label(RichText::new("Measure, then send to the plant").strong());
        let runs: Vec<FinishedRun> = self.app.as_ref().map_or(Vec::new(), |a| a.core_runs().to_vec());
        if runs.len() < 2 {
            ui.small(format!(
                "{} finished run(s). Run the core twice, changing one thing: the fuel and moderator pebbles' temperatures together (the bed coefficient), or every rod out then every rod in (the bank worth).",
                runs.len()
            ));
        } else {
            let label = |i: usize, r: &FinishedRun| {
                let t = |z: usize| r.temps_k[z] - 273.15;
                let rods = if r.rods.iter().all(|&x| x == 0.0) { "rods out".to_string() } else if r.rods.iter().all(|&x| x == 1.0) { "rods in".to_string() } else { "rods set".to_string() };
                format!("#{} k = {:.5} ± {:.5} · fuel {:.0} °C, mod. {:.0} °C · {rods}", i + 1, r.k, r.sigma, t(FUEL_PEBBLE), t(MODERATOR_PEBBLE))
            };
            self.base = self.base.min(runs.len() - 1);
            self.other = self.other.min(runs.len() - 1);
            for (name, pick) in [("Base", &mut self.base), ("Changed", &mut self.other)] {
                egui::ComboBox::from_id_salt(("mc_pair", name)).width(ui.available_width() - 60.0).selected_text(format!("{name}: {}", label(*pick, &runs[*pick]))).show_ui(ui, |ui| {
                    for (i, r) in runs.iter().enumerate() {
                        ui.selectable_value(pick, i, label(i, r));
                    }
                });
            }
            match classify(&runs[self.base], &runs[self.other]) {
                Ok(kind) => {
                    let (what, m, unit) = match kind {
                        PairKind::Bed(m) => ("bed coefficient", m, 1e5),
                        PairKind::FuelPebble(m) => ("fuel-pebble coefficient", m, 1e5),
                        PairKind::ModeratorPebble(m) => ("moderator-pebble coefficient", m, 1e5),
                        PairKind::Bank(m) => ("bank worth", m, 100.0),
                    };
                    let suffix = if matches!(kind, PairKind::Bank(_)) { "% Δk/k" } else { "pcm/K" };
                    ui.label(format!("This pair measures the {what}: {:.2} ± {:.2} {suffix}", m.value * unit, m.sigma * unit));
                    if ui.button("Keep this measurement").clicked() {
                        self.measured.take(kind);
                        self.note.clear();
                    }
                }
                Err(e) => {
                    ui.small(format!("This pair measures nothing the plant takes: {e}."));
                }
            }
        }
        let m = self.measured;
        let row = |ui: &mut Ui, name: &str, v: Option<Measured>, unit: f64, suffix: &str| {
            ui.label(name);
            ui.label(v.map_or("not measured".to_string(), |v| format!("{:.2} ± {:.2} {suffix}", v.value * unit, v.sigma * unit)));
            ui.end_row();
        };
        egui::Grid::new("mc_measured").num_columns(2).show(ui, |ui| {
            row(ui, "Bed (pebbles together)", m.bed, 1e5, "pcm/K");
            row(ui, "Fuel pebbles only", m.fuel_pebble, 1e5, "pcm/K");
            row(ui, "Moderator pebbles only", m.moderator_pebble, 1e5, "pcm/K");
            row(ui, "Bank worth", m.bank, 100.0, "% Δk/k");
        });
        match plant_set(&m) {
            Ok(set) => {
                ui.label(format!(
                    "To the plant: fuel {:.2} pcm/K (the poured-bed record's kernel coefficient: the sliders raise the whole fuel pebble, so the kernel alone cannot be measured here), moderator {:.2} ± {:.2} pcm/K (the bed minus that), bank {:.2} {}% Δk/k ({}).",
                    set.fuel * 1e5,
                    set.moderator * 1e5,
                    set.moderator_sigma * 1e5,
                    set.worth * 100.0,
                    if set.worth_measured { format!("± {:.2} ", set.worth_sigma * 100.0) } else { String::new() },
                    if set.worth_measured { "measured" } else { "the record's" }
                ));
                if ui.button("Send to plant").clicked() {
                    physics.update(|s| s.coefficient_set = CoefficientSet::Measured(set));
                    self.note = "Sent: the plant runs on the measured set (it keeps its state, so a hot plant steps in reactivity).".into();
                }
            }
            Err(e) => {
                ui.small(format!("Nothing to send yet: {e}."));
            }
        }
        if matches!(snapshot.coefficient_set, CoefficientSet::Measured(_)) {
            ui.small("The plant is on a measured set.");
        }
        if !self.note.is_empty() {
            ui.small(&self.note);
        }
    }
}

/// The coefficient sets beside each other, and the picker.
pub fn draw_sets(ui: &mut Ui, physics: &SharedState<HtgrSnapshot>, snapshot: &HtgrSnapshot) {
    ui.label(RichText::new("Coefficient sets").strong());
    ui.label(
        RichText::new(
            "The plant's feedback uses a fuel (kernel Doppler) and a moderator (graphite) temperature \
             coefficient, and the ten-rod bank a total worth. Each set gives all three.",
        )
        .small(),
    );
    let mut sets: Vec<CoefficientSet> = CoefficientSet::ALL.to_vec();
    if matches!(snapshot.coefficient_set, CoefficientSet::Measured(_)) {
        sets.push(snapshot.coefficient_set);
    }
    for set in sets {
        let in_use = set == snapshot.coefficient_set;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let title = if in_use { format!("{}  (in use)", set.label()) } else { set.label().to_string() };
            ui.label(RichText::new(title).strong());
            match set.values() {
                Some((fuel, moderator, worth)) => {
                    egui::Grid::new(("mc_set", set.label())).num_columns(2).show(ui, |ui| {
                        ui.label("Fuel coefficient");
                        ui.label(format!("{:.2} pcm/K", fuel * 1e5));
                        ui.end_row();
                        ui.label("Moderator coefficient");
                        ui.label(format!("{:.2} pcm/K", moderator * 1e5));
                        ui.end_row();
                        ui.label("Rod-bank worth");
                        ui.label(format!("{:.2} % Δk/k", worth * 100.0));
                        ui.end_row();
                    });
                }
                None => {
                    ui.label(format!("Pending: the committed Monte Carlo record of gh:#{} is not in yet.", crate::physics::coefficient_set::POURED_BED_ISSUE));
                }
            }
            ui.small(set.source());
        });
        ui.add_space(4.0);
    }
    coefficient_picker(ui, physics, snapshot);
    ui.small("A switch keeps the plant's state and reference temperatures, so a hot plant steps in reactivity when the set changes.");
}

#[cfg(test)]
mod tests {
    use super::*;
    use dhoby_ghaut::mc_web::keff::KeffConfig;

    fn run(k: f64, fuel_k: f64, mod_k: f64, rods: f64) -> FinishedRun {
        let mut temps_k = [300.15; 8];
        temps_k[FUEL_PEBBLE] = fuel_k;
        temps_k[MODERATOR_PEBBLE] = mod_k;
        FinishedRun {
            k,
            sigma: 0.002,
            temps_k,
            rods: [rods; 10],
            bed: 12,
            pour_version: 0,
            bed_label: "lattice".into(),
            cfg: KeffConfig { n_particles: 1000, n_inactive: 10, n_active: 20, seed: 1, point_source: false, want_sites: false },
        }
    }

    /// gh:#1763: each pair is read as what it measures, with the twin's
    /// coefficient definition, and a pair that measures nothing the plant
    /// takes is refused with the reason.
    #[test]
    fn a_pair_is_read_as_what_it_measures() {
        let base = run(0.94516, 600.0, 600.0, 0.0);
        let bed = classify(&base, &run(0.90000, 1200.0, 1200.0, 0.0)).expect("bed");
        let want = (1.0 / 0.94516 - 1.0 / 0.90000) / 600.0;
        let PairKind::Bed(m) = bed else { panic!("{bed:?}") };
        assert_eq!(m.value, want);
        assert!(m.sigma > 0.0);
        assert!(matches!(classify(&base, &run(0.93, 1200.0, 600.0, 0.0)), Ok(PairKind::FuelPebble(_))));
        assert!(matches!(classify(&base, &run(0.93, 600.0, 1200.0, 0.0)), Ok(PairKind::ModeratorPebble(_))));
        // Bank: either order, 1/k_in − 1/k_out.
        let inn = run(0.77950, 600.0, 600.0, 1.0);
        let PairKind::Bank(w) = classify(&base, &inn).expect("bank") else { panic!() };
        assert_eq!(w.value, 1.0 / 0.77950 - 1.0 / 0.94516);
        assert_eq!(classify(&inn, &base), classify(&base, &inn));
        // Refusals.
        assert!(classify(&base, &base).unwrap_err().contains("same state"));
        assert!(classify(&base, &run(0.9, 1200.0, 900.0, 0.0)).unwrap_err().contains("different amounts"));
        assert!(classify(&base, &run(0.9, 1200.0, 1200.0, 1.0)).unwrap_err().contains("both"));
        assert!(classify(&base, &run(0.9, 600.0, 600.0, 0.5)).unwrap_err().contains("every rod"));
        let mut other_bed = run(0.9, 1200.0, 1200.0, 0.0);
        other_bed.bed = 20;
        assert!(classify(&base, &other_bed).unwrap_err().contains("different beds"));
        let mut coolant = run(0.9, 600.0, 600.0, 0.0);
        coolant.temps_k[2] = 900.0;
        assert!(classify(&base, &coolant).unwrap_err().contains("other zones"));
    }

    /// gh:#1763: the plant's set is the record's kernel coefficient as the
    /// fuel, the measured bed minus it as the moderator, and the measured
    /// worth (else the record's). Fed the record's own five k, the page
    /// gives back the record's set exactly, up to the bed's two secants
    /// being one pair here.
    #[test]
    fn the_plant_set_is_the_bed_minus_the_records_kernel() {
        let mut m = Measurements::default();
        assert!(plant_set(&m).unwrap_err().contains("no bed coefficient"));
        let (f, _, w) = POURED_BED.expect("record");
        m.take(PairKind::FuelPebble(Measured { value: -1.0e-4, sigma: 3e-6 }));
        m.take(PairKind::ModeratorPebble(Measured { value: -0.5e-4, sigma: 4e-6 }));
        let s = plant_set(&m).expect("summed");
        assert_eq!((s.fuel, s.worth, s.worth_measured), (f, w, false));
        assert!((s.moderator - (-1.5e-4 - f)).abs() < 1e-18);
        assert!((s.moderator_sigma - 5e-6).abs() < 1e-18);
        // A direct bed pair wins over the sum; a measured worth is used.
        m.take(PairKind::Bed(Measured { value: -2.0e-4, sigma: 1e-6 }));
        m.take(PairKind::Bank(Measured { value: 0.2, sigma: 0.004 }));
        let s = plant_set(&m).expect("direct");
        assert_eq!((s.moderator, s.moderator_sigma, s.worth, s.worth_sigma, s.worth_measured), (-2.0e-4 - f, 1e-6, 0.2, 0.004, true));
        // The record's own runs: kernel + (fuel graphite + dummies) is the
        // bed, so bed − kernel is the record's moderator.
        let (rf, rm, _) = POURED_BED.expect("record");
        let bed = Measured { value: rf + rm, sigma: 0.0 };
        let s = plant_set(&Measurements { bed: Some(bed), ..Default::default() }).expect("record");
        assert!((s.moderator - rm).abs() < 1e-18, "{} {rm}", s.moderator);
        assert_eq!(s.fuel, rf);
    }
}
