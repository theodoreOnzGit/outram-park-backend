//! The `lct008` rung: ICSBEP LEU-COMP-THERM-008 case 1, rods of low-enriched
//! UO₂ in borated water, the light-water reactor's lattice (rung 4 of the
//! Monte Carlo ladder, gh:#526). Watch mode, like `triso/` (see `rungs.rs`
//! for the contract), plus two pieces since 2026-10-05 (gh:#549): a **pitch
//! slider** that runs a real `k_inf` of one LCT-008 rod in a reflective square
//! cell at the reader's pitch, streamed one generation at a time and drawn
//! against the recorded sweep, and the **σ(E) panel** (U-238 capture, U-235
//! fission, H-1 scattering in water) beside the core.
//!
//! The core is the real benchmark model: the committed `mit-crpg/benchmarks`
//! cards, parsed by outram-mc-libs' shared `lct008_model.rs` (module
//! [`spec`]), 4961 fuel rods in nested lattices. Each track is one real
//! history through that lattice. No Run k_eff here: the eigenvalue of this
//! core takes ~10⁸ histories to pin down, which is not a browser job. The
//! lesson quotes the recorded result instead.

pub mod model;
#[cfg(not(target_arch = "wasm32"))]
pub mod render;
pub mod sim;

/// The shared LCT-008 model code (XML cards, materials, nested-lattice CSG),
/// used here unchanged so the demo and the V&V runs build one model.
#[path = "../../../../outram-mc-libs/examples/common/lct008_model.rs"]
#[allow(clippy::all)]
pub mod spec;

/// The reflective pin cell of the recorded pitch sweep (`lct008_pitch_sweep.rs`),
/// shared unchanged so the slider and the record transport one model.
#[path = "../../../../outram-mc-libs/examples/common/lct008_pin_cell.rs"]
pub mod pin_cell;

/// The recorded sweep the slider's points are drawn against: outram-mc and
/// OpenMC, 11-nuclide tier, case-1 borated water, 5000 × [50 + 200], seed 1,
/// 2026-10-04 (`verification_and_validation/tutorial_rung4/pitch_sweep.md`).
/// Read from the committed CSVs at build time, never retyped.
const SWEEP_OURS_CSV: &str =
    include_str!("../../../../outram-mc-libs/verification_and_validation/tutorial_rung4/data/pitch_sweep_cheap11_case1.csv");
const SWEEP_OPENMC_CSV: &str =
    include_str!("../../../../outram-mc-libs/verification_and_validation/tutorial_rung4/data/openmc_cheap11_case1.csv");
/// LCT-008's own lattice pitch [cm] (the committed `geometry.xml`; the sweep
/// example asserts it against the file).
pub const PITCH_LCT008: f64 = 1.63576;

use crate::anim::Spectrum;
use crate::engine::{Tier, CHAIN_SEED};
use crate::history::History;
use crate::keff::{csv_points, Generation, KeffConfig, KinfCase, KinfGeneration, LineStyle, RecordedCurve};
use crate::xs::{Channel, XsCurve};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::CsgPowerIteration;
use crate::rungs::{LoadedRung, McRung, RungBuilder, RungInfo};
use dhoby_ghaut::web_demo::view::View;
use egui::{Color32, Rect, Sense, Stroke, Vec2};

/// The marker type `rung_table!` names.
pub struct Lct008;

const VACUUM: Color32 = Color32::from_rgb(18, 20, 26);
const WATER: Color32 = Color32::from_rgb(28, 62, 112);
const CLAD: Color32 = Color32::from_rgb(150, 150, 154);
const FUEL: Color32 = Color32::from_rgb(214, 96, 52);
const OTHER: Color32 = Color32::from_rgb(200, 200, 120);

/// Pin centres for the picture, from the assembled geometry (built once on
/// the UI side; parsing the cards takes milliseconds).
static PINS: std::sync::OnceLock<Vec<(f64, f64, model::PinKind)>> = std::sync::OnceLock::new();

fn pins() -> &'static [(f64, f64, model::PinKind)] {
    PINS.get_or_init(|| match model::materials() {
        Ok(m) => model::pins(&model::build_geometry(&m)),
        Err(_) => Vec::new(),
    })
}

impl McRung for Lct008 {
    const INFO: RungInfo = RungInfo {
        name: "lct008",
        title: "LEU-COMP-THERM-008: rods in water",
        lesson: "tutorials/monte-carlo/lct008.html",
        spectrum: Spectrum::Thermal,
    };
    type Builder = model::DataBuilder;

    fn jobs() -> &'static [(&'static str, &'static str)] {
        &model::JOBS
    }
    /// Always the loosened 0.01 tier: Watch mode is an illustration (module
    /// docs of [`model`]).
    fn tier(_requested: Tier) -> Tier {
        Tier::Loose
    }
    /// Native seconds per job at tolerance 0.01, measured 2026-10-04 by the
    /// rung's ignored test (i9-13900K, one thread, shared machine; 0.0 s
    /// jobs floored at 0.1); progress-bar weights only.
    fn job_weights(_tier: Tier) -> Vec<f64> {
        vec![0.1, 0.1, 0.1, 0.8, 14.8, 13.0, 0.1, 0.1, 0.1, 0.1, 0.5, 16.2]
    }
    fn half_extent() -> f64 {
        model::R_CORE
    }
    /// The core seen from above: water inside the r = 76.2 cm cylinder,
    /// vacuum outside, and every rod (clad ring, fuel pellet) at its centre
    /// as read from the assembled geometry. Zoomed out, a rod is a dot.
    fn draw(painter: &egui::Painter, rect: Rect, view: &View) {
        let s = view.scale as f32;
        let o = view.to_screen(rect, 0.0, 0.0);
        painter.rect_filled(painter.clip_rect(), 0.0, VACUUM);
        painter.circle_filled(o, model::R_CORE as f32 * s, WATER);
        let clip = painter.clip_rect().expand(8.0);
        let (rc, rf) = (model::R_CLAD as f32 * s, model::R_FUEL as f32 * s);
        for &(x, y, k) in pins() {
            let c = view.to_screen(rect, x, y);
            if !clip.contains(c) {
                continue;
            }
            let col = if k == model::PinKind::Fuel { FUEL } else { OTHER };
            if rc < 1.5 {
                painter.circle_filled(c, 1.0, col);
                continue;
            }
            painter.circle_filled(c, rc, CLAD);
            painter.circle_filled(c, rf, col);
        }
        painter.circle_stroke(o, model::R_CORE as f32 * s, Stroke::new(1.5, Color32::from_rgb(170, 120, 255)));
        painter.text(
            o + Vec2::new(0.0, -(model::R_CORE as f32) * s - 6.0),
            egui::Align2::CENTER_BOTTOM,
            "core cylinder r = 76.2 cm · vacuum outside (leaks)",
            egui::FontId::proportional(12.0),
            Color32::from_rgb(190, 170, 230),
        );
    }
    fn notes() -> &'static [&'static str] {
        &[
            "Model: ICSBEP LEU-COMP-THERM-008 case 1 (Babcock & Wilcox critical lattice): 4961 rods of 2.459 w/o UO₂ in Al-6061 clad, 1.63576 cm pitch, in water with 1511 ppm boron, arranged as a 7 × 7 core lattice of 15 × 15 pin lattices. Read at run time from the committed mit-crpg/benchmarks OpenMC cards by the same code the recorded result was measured with.",
            "Data: ENDF/B-VIII.0, processed in this browser by OUTRAM PARK's NJOY port at tolerance 0.01 (NJOY uses 0.001; measured effect on this lattice −65 ± 40 pcm). Hydrogen in the water uses the H in H₂O S(α,β) thermal-scattering law. 11 nuclides: the clad's trace elements (Mg, Ti, Cr, Fe, Cu, Zn) and B-11 are left out, as in the quoted result.",
            "Each track is one real history (a one-particle fixed-source run through the nested lattices), drawn projected from above. Chaining one neutron after the next is the illustration.",
            "There is no Run k_eff for this rung: pinning down this core's k takes about 10⁸ histories. The recorded result (+236 ± 8 pcm from k = 1, 96 seeds) is on the lesson page.",
            "\"pitch k∞\" runs a real, single-threaded power iteration of ONE rod in a reflective square cell (an infinite lattice, no leakage) at the pitch you choose, on the same data, and draws it against the recorded sweep. It is k∞ of the pin lattice, not the core's k_eff.",
            "The σ(E) panel shows the cross sections this tab processed for transport (tolerance 0.01), decimated for drawing: U-238 capture (σ_a − σ_f), U-235 fission and H-1 scattering with the water's S(α,β) law below its cutoff.",
            "Animation speed is proportional to the neutron's real speed (classical kinetic energy, v proportional to √E).",
            "Education and research only. Not for reactor operation, licensing or safety decisions.",
        ]
    }
    fn loading_note(_tier: Tier) -> Option<&'static str> {
        Some("Twelve tapes, including the water's thermal-scattering law: allow a few minutes in a browser, longer on a phone.")
    }
    /// The pitch slider: k∞ of one rod in a reflective square cell. Default
    /// settings 1000 × [20 + 30]: the issue's cost reference (native, about
    /// 8 s on 2 threads, gh:#549); the browser's single thread is measured
    /// on the issue.
    fn kinf_case() -> Option<KinfCase> {
        Some(KinfCase {
            title: "k∞ against pitch: one rod, infinite lattice",
            param: ("pitch", "cm"),
            range: (1.25, 3.30),
            default: PITCH_LCT008,
            choices: Vec::new(),
            marks: vec![(PITCH_LCT008, "LCT-008")],
            cfg: KeffConfig { n_particles: 1000, n_inactive: 20, n_active: 30, seed: 1, point_source: false, want_sites: false },
            curves: vec![
                RecordedCurve {
                    label: "ours: outram-mc, recorded 2026-10-04".into(),
                    style: LineStyle::Ours,
                    points: csv_points(SWEEP_OURS_CSV, "pitch_cm", "k_inf", "k_std"),
                },
                RecordedCurve {
                    label: "reference code: OpenMC, recorded 2026-10-04".into(),
                    style: LineStyle::Reference,
                    points: csv_points(SWEEP_OPENMC_CSV, "pitch_cm", "k_gen_mean", "k_gen_sem"),
                },
            ],
            notes: vec![
                "One LCT-008 rod (UO₂ 2.459 w/o, Al clad) in case-1 water with 1511 ppm boron, in a square cell with reflective walls: an infinite lattice, so this is k∞ (no leakage), not the core's k_eff.",
                "A real power iteration by outram-mc-libs (CsgPowerIteration, the single-thread reference stepped one generation per worker message), on the data this tab processed at tolerance 0.01; the record used NJOY's 0.001 (measured effect on the full lattice −65 ± 40 pcm).",
                "Your ± is the standard error over the active generations of ONE run: it understates the true spread, because generations are correlated.",
                "Recorded curves (pitch_sweep.md, 2026-10-04, seed 1, 5000 × [50 + 200], NJOY tolerance 0.001, 11 nuclides): dotted = outram-mc (ours), dashed = OpenMC 0.16.1-dev on NJOY2016 ACE, the same model (a reference code). Verification, not validation: no experiment measured these pitches.",
            ],
        })
    }
    fn legend(ui: &mut egui::Ui) {
        ui.strong("Materials");
        for (c, name) in [
            (FUEL, "UO₂ fuel, 2.459 w/o U-235"),
            (CLAD, "Al-6061 clad"),
            (WATER, "Water, 1511 ppm boron"),
            (VACUUM, "Vacuum (outside the core)"),
        ] {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                ui.painter().rect_filled(r, 2.0, c);
                ui.label(name);
            });
        }
    }
}

impl RungBuilder for model::DataBuilder {
    type Loaded = Loaded;
    fn new(_tier: Tier) -> Self {
        model::DataBuilder::default()
    }
    fn step(&mut self, bytes: &[u8], store: &mut crate::processed_cache::DataStore) -> Result<(), String> {
        model::DataBuilder::step(self, bytes, store)
    }
    fn finish(self) -> Result<Loaded, String> {
        Ok(Loaded { phys: sim::Physics::new(model::DataBuilder::finish(self)?), chain: sim::Chain::new(CHAIN_SEED), kinf: None })
    }
}

/// The rung once its data are processed.
pub struct Loaded {
    pub phys: sim::Physics,
    pub chain: sim::Chain,
    /// The running pitch case, if any.
    pub kinf: Option<PinRun>,
}

/// A pitch case in progress: its pin cell and power iteration.
pub struct PinRun {
    pub pitch: f64,
    pub geometry: Geometry,
    pub it: CsgPowerIteration,
    pub total: usize,
}

impl PinRun {
    /// Build the reflective pin cell at `pitch` and sample its initial source,
    /// as `lct008_pitch_sweep.rs` does (same cell, same source box), on the
    /// rung's own case-1 materials and nuclides.
    pub fn new(phys: &sim::Physics, pitch: f64, cfg: KeffConfig) -> Result<Self, String> {
        if !(pitch > 2.0 * model::R_CLAD && pitch < 10.0) {
            return Err(format!("pitch {pitch} cm is outside (rod diameter, 10 cm)"));
        }
        let geometry = pin_cell::pin_cell(0.5 * pitch, model::R_FUEL, model::R_CLAD, model::TEMP_K);
        let settings = KeffSettings {
            n_particles: cfg.n_particles,
            n_inactive: cfg.n_inactive,
            n_active: cfg.n_active,
            seed: cfg.seed,
            temperature_k: model::TEMP_K,
            ..KeffSettings::default()
        };
        let (m, n) = (&phys.data.materials, &phys.data.nuclides);
        let it = CsgPowerIteration::new(&geometry, m, n, pin_cell::source_box(model::R_FUEL), &settings);
        Ok(Self { pitch, geometry, it, total: cfg.n_inactive + cfg.n_active })
    }
    pub fn step(&mut self, phys: &sim::Physics) -> Option<KinfGeneration> {
        let r = self.it.step(&self.geometry, &phys.data.materials, &phys.data.nuclides)?;
        let (mean, sem) = r.k_mean.unwrap_or((f64::NAN, f64::NAN));
        Some(KinfGeneration { param: self.pitch, index: r.index, total: self.total, active: r.active, k: r.k, mean, sem })
    }
}

impl LoadedRung for Loaded {
    fn run_next(&mut self) -> History {
        self.chain.run_next(&self.phys)
    }
    fn keff_start(&mut self, _cfg: KeffConfig) -> Result<Vec<[f32; 2]>, String> {
        Err("the LCT-008 rung has no k_eff mode".into())
    }
    fn keff_step(&mut self) -> Option<Generation> {
        None
    }
    fn keff_finished(&self) -> bool {
        true
    }
    fn kinf_start(&mut self, param: f64, cfg: KeffConfig) -> Result<(), String> {
        self.kinf = Some(PinRun::new(&self.phys, param, cfg)?);
        Ok(())
    }
    fn kinf_step(&mut self) -> Option<KinfGeneration> {
        let phys = &self.phys;
        self.kinf.as_mut()?.step(phys)
    }
    fn xs_curves(&self) -> Vec<XsCurve> {
        crate::xs::curves(
            &self.phys.data.nuclides,
            model::TEMP_K,
            &[
                ("U-238 capture", model::N_U238, Channel::Capture),
                ("U-235 fission", model::N_U235, Channel::Fission),
                ("H-1 scattering (bound in H₂O below the S(α,β) cutoff)", model::N_H1, Channel::Scatter),
            ],
        )
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use outram_mc_libs::physics::track_output::TrackEvent;

    /// The picture and the births use the rods read back from the assembled
    /// geometry; case 1 has 4961 fuel rods and nothing else (ring-RPT V&V
    /// record, Interpretation 18), so a wrong walk of the nested lattices
    /// shows here. And the shared model's own 200 000-point self-check of
    /// `locate` against hand arithmetic passes on the geometry this rung builds.
    #[test]
    fn the_core_has_the_benchmark_rods_and_passes_its_self_check() {
        let mats = model::materials().expect("materials");
        let geom = model::build_geometry(&mats);
        let p = model::pins(&geom);
        assert_eq!(p.iter().filter(|q| q.2 == model::PinKind::Fuel).count(), 4961);
        assert_eq!(p.len(), 4961);
        let shares = spec::check_geometry(&geom, &mats);
        assert_eq!(shares.len(), 3);
    }

    /// The pitch case's recorded curves are the committed sweep, read back:
    /// 11 pitches each, LCT-008's own among them, and a k∞ message crosses
    /// the worker boundary bit for bit.
    #[test]
    fn the_pitch_case_reads_the_recorded_sweep() {
        let c = Lct008::kinf_case().expect("lct008 has a pitch case");
        assert_eq!(c.curves.len(), 2);
        for curve in &c.curves {
            assert_eq!(curve.points.len(), 11, "{}", curve.label);
            let at = curve.points.iter().find(|p| (p.0 - PITCH_LCT008).abs() < 1e-9).expect("LCT-008 pitch");
            assert!((at.1 - 1.06).abs() < 0.01 && at.2 > 0.0 && at.2 < 0.002, "{}: {at:?}", curve.label);
            assert!(curve.points.windows(2).all(|w| w[0].0 < w[1].0));
        }
        assert!(c.range.0 <= c.curves[0].points[0].0 && c.range.1 >= c.curves[0].points[10].0);
        let g = KinfGeneration { param: 1.7, index: 3, total: 50, active: false, k: 1.01, mean: f64::NAN, sem: f64::NAN };
        let back = crate::engine::decode_kinf(&crate::engine::encode_kinf(&g)).unwrap();
        assert_eq!((back.param, back.index, back.total, back.active, back.k), (g.param, g.index, g.total, g.active, g.k));
        assert!(back.mean.is_nan() && back.sem.is_nan());
    }

    /// The slider's run, natively, at the slider's defaults (1000 × [20 +
    /// 30], seed 1) and LCT-008's pitch, on the rung's tolerance-0.01 data:
    /// it streams one generation per step, ends at the last, and its k∞ is
    /// within 5 of its own σ of the recorded 1.06403 ± 0.00117 (a harness
    /// check, not V&V: different tier, settings and random stream). Also
    /// prints the σ(E) curves' sizes and the timings quoted on gh:#549.
    #[test]
    #[ignore = "processes 12 ENDF tapes (~1 min); run with --ignored"]
    fn the_pitch_slider_runs_the_pin_cell() {
        let mut b = model::DataBuilder::default();
        for (_, tape) in model::JOBS {
            let raw = std::fs::read(njoy_outram_park_fork::reference_data::reference_endf(tape).expect("tape")).expect("read");
            b.step(&crate::tapes::strip_covariances(&raw), &mut crate::processed_cache::DataStore::off()).expect("step");
        }
        let mut l = Loaded { phys: sim::Physics::new(b.finish().expect("finish")), chain: sim::Chain::new(1), kinf: None };
        let t = std::time::Instant::now();
        let xs = l.xs_curves();
        eprintln!("  sigma(E) curves in {:.2} s: {:?}", t.elapsed().as_secs_f64(), xs.iter().map(|c| (c.label.clone(), c.points.len())).collect::<Vec<_>>());
        assert_eq!(xs.len(), 3);
        let cfg = Lct008::kinf_case().unwrap().cfg;
        let t = std::time::Instant::now();
        l.kinf_start(PITCH_LCT008, cfg).expect("start");
        let mut n = 0;
        let mut last = None;
        while let Some(g) = l.kinf_step() {
            assert_eq!(g.index, n);
            n += 1;
            last = Some(g);
        }
        let g = last.expect("generations");
        eprintln!("  pin cell {} x [{} + {}] in {:.1} s (one thread): k_inf = {:.5} +/- {:.5}", cfg.n_particles, cfg.n_inactive, cfg.n_active, t.elapsed().as_secs_f64(), g.mean, g.sem);
        assert_eq!(n, 50);
        assert!((g.mean - 1.06403).abs() < 5.0 * g.sem, "k_inf {} +/- {}", g.mean, g.sem);
    }

    /// Process the tapes (tolerance 0.01) and run real histories through the
    /// core: each ends in a leak, capture or fission, inside the core
    /// cylinder, and most of them reach thermal energies (this is a
    /// water-moderated lattice). Needs `reference-data/endf/` and about a
    /// minute natively, so it is opt-in, as the crate's other data tests.
    #[test]
    #[ignore = "processes 12 ENDF tapes (~1 min); run with --ignored"]
    fn histories_through_the_lattice_end_properly_and_thermalise() {
        let mut b = model::DataBuilder::default();
        for (label, tape) in model::JOBS {
            let t = std::time::Instant::now();
            let path = njoy_outram_park_fork::reference_data::reference_endf(tape).expect("tape");
            let raw = std::fs::read(path).expect("read");
            b.step(&crate::tapes::strip_covariances(&raw), &mut crate::processed_cache::DataStore::off()).expect("step");
            eprintln!("  {label:<16} {:6.1} s", t.elapsed().as_secs_f64());
        }
        let phys = sim::Physics::new(b.finish().expect("finish"));
        assert_eq!(phys.fuel.len(), 4961);
        let mut chain = sim::Chain::new(3);
        let (mut thermal, n) = (0, 60);
        let t = std::time::Instant::now();
        for _ in 0..n {
            let h = chain.run_next(&phys);
            assert!(
                matches!(h.outcome, Some(TrackEvent::Leak | TrackEvent::Fission | TrackEvent::Absorption)),
                "history {}: {}",
                h.index,
                crate::history::outcome_name(h.outcome)
            );
            assert_eq!(h.track.dropped_states, 0);
            for s in &h.track.states {
                let r = (s.r.x * s.r.x + s.r.y * s.r.y).sqrt();
                assert!(r <= model::R_CORE * (1.0 + 1e-9) && s.r.z.abs() <= model::Z_HI * (1.0 + 1e-9));
            }
            thermal += h.thermalised() as usize;
        }
        eprintln!("  {n} histories in {:.2} s, {thermal} thermalised", t.elapsed().as_secs_f64());
        assert!(thermal > n / 2, "a water lattice thermalises most neutrons; got {thermal} of {n}");
    }
}
