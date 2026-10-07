//! The `htr10` rung: TRISO particles inside pebbles inside the HTR-10 core
//! (rung 5 of the Monte Carlo ladder, gh:#528; lesson
//! `tutorials/monte-carlo/triso.html`). Watch mode, with no neutron tracks:
//!
//! - **geometry**: a zoom ladder core → bed → pebble → TRISO → kernel, each
//!   view a live slice of the core `nee_soon` assembles for the recorded runs
//!   ([`crate::raster`], [`model::core`]);
//! - **layers**: the recorded k against loading height for N = 10–20 layers,
//!   read with a slider, beside the bed at that N, against RMC and MCNP
//!   ([`crate::sweep`]);
//! - **fuel-zone k∞**: a real, streamed power iteration of the record's fuel-zone
//!   cube with the kernels resolved or homogenised (#549's `kinf_case`), the
//!   double-heterogeneity worth by hand.
//!
//! ~~**No neutron tracks and no k_eff of the core in the browser**: the recorded
//! runs took 2379–4137 s of transport for 1.4 M histories on 5 threads (about
//! 8.5–14.8 ms of CPU per history, gh:#528), i.e. hours on one browser thread,
//! and tracks through the core would need every core material's data. The
//! layers view shows the recorded results instead.~~ **CORRECTED 2026-10-08
//! (gh:#786):** the **whole core** view ([`core`]) loads every core
//! material's data on a pool of Web Workers (67–128 s measured on 2–4
//! workers of a desktop) and runs a low-statistics k_eff of the core
//! (1000 × [5 + 20] in 75–150 s), with its neutrons drawn; the record's
//! statistics are still hours, so the layers view keeps the recorded results.
//! Measurements: `verification_and_validation/htr10_full_core_web/`.

pub mod core;
pub mod model;
#[cfg(not(target_arch = "wasm32"))]
pub mod render;

use crate::anim::Spectrum;
use crate::engine::Tier;
use crate::history::History;
use crate::keff::{Generation, KeffConfig, KinfCase, KinfGeneration, LineStyle, RecordedCurve};
use crate::raster::{Basis, Preset, RasterInfo, RasterReq};
use crate::rungs::{LoadedRung, McRung, RungBuilder, RungInfo};
use crate::sweep::{RecordedSweep, SweepCurve, SweepPoint};
use dhoby_ghaut::web_demo::view::View;
use egui::{Color32, Rect};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::pebble_beds::keff_delta::{DeltaDomain, DeltaPowerIteration};
use outram_mc_libs::physics::keff::KeffSettings;

/// The marker type `rung_table!` names.
pub struct Htr10;

/// The 2026-10-07 k-vs-height table (both libraries, N = 10–20, 10 000 ×
/// [5 + 135] per point on the bounded delta-tracking majorant of #589), read
/// at build time.
///
/// ~~`SEKER_CSV` = the 2026-10-01 record (shown faded, superseded) and
/// `SEKER_CSV_BOUNDED` = the 2026-10-05 re-measurement at 5 points, 10 000 ×
/// [5 + 20]~~ **CORRECTED 2026-10-07** (gh:#782): the 2026-10-07 record
/// re-measures all 22 points at full statistics and supersedes both, so the
/// demo shows it alone. Both older records stay in `nee_soon`'s V&V folder.
const SEKER_CSV: &str = include_str!("../../../../nee_soon/verification_and_validation/htr10_seker_2026_10_07_10k/results_table.csv");

/// The fuel-zone record, quoted from
/// `crates/outram-mc-libs/verification_and_validation/tutorial_rung5/README.md`
/// (2026-10-05, `c5a6ca4ae`): 10 000 × [50 + 200], seed 1, single thread.
pub const RECORD_HOM: (f64, f64) = (1.44684, 0.00090);
pub const RECORD_HET: (f64, f64) = (1.57136, 0.00088);

/// The plane the ladder's x-y steps cut: through a TRISO particle of the fuel
/// pebble centred at the origin at N = 12, as located on the assembled
/// geometry by `nee_soon`'s `htr10_geometry_images` (2026-10-05:
/// pebble (0, 0, 0), particle (0.0254, 0.0722, 0.1385)); pinned by
/// `the_ladder_lands_on_a_kernel`.
pub const LADDER_Z: f64 = 0.1385;
pub const PARTICLE_XY: [f64; 2] = [0.0254, 0.0722];
/// N of the ladder's core (Şeker's critical row, 123.576 cm).
pub const LADDER_N: f64 = 12.0;

fn colour(c: outram_mc_libs::geometry::plot::Rgb) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

impl McRung for Htr10 {
    const INFO: RungInfo = RungInfo {
        name: "htr10",
        title: "HTR-10: pebbles, TRISO, kernels",
        lesson: "tutorials/monte-carlo/triso.html",
        spectrum: Spectrum::Thermal,
    };
    type Builder = model::DataBuilder;

    /// Every tape (published once by `--prepare-web-data`); a Watch load
    /// processes none ([`McRung::jobs_for`]).
    fn jobs() -> &'static [(&'static str, &'static str)] {
        &model::JOBS
    }
    fn jobs_for(tier: Tier) -> &'static [(&'static str, &'static str)] {
        match tier {
            Tier::Loose => &[],
            Tier::Exact => &model::JOBS,
        }
    }
    /// The geometry and the recorded sweep need no data (the loose tier
    /// loads none); the fuel-zone k∞ processes at NJOY's tolerance, like the
    /// record.
    fn tier(requested: Tier) -> Tier {
        requested
    }
    /// Rough native seconds per tape at tolerance 0.001 (progress bar only):
    /// U-235 and U-238 about 70 s each (#528), the rest small.
    fn job_weights(tier: Tier) -> Vec<f64> {
        match tier {
            Tier::Loose => Vec::new(),
            Tier::Exact => vec![70.0, 70.0, 10.0, 3.0, 2.0, 2.0, 2.0, 5.0],
        }
    }
    fn half_extent() -> f64 {
        200.0
    }
    /// The picture is the live slice ([`crate::raster`]); nothing static.
    fn draw(_painter: &egui::Painter, _rect: Rect, _view: &View) {}
    fn notes() -> &'static [&'static str] {
        &[
            "Geometry: the HTR-10 core exactly as nee_soon assembles it for the recorded runs (assemble_explicit_triso, 14 rings, N Şeker layers): every slice is the material Geometry::locate finds at each pixel, drawn in this tab's worker. Nothing is re-modelled for the picture.",
            "Layers: recorded results only (ENDF/B-VIII.0 and VII.0: 10 000 × [5 + 135] per point, all 22 points measured 2026-10-07 on the bounded delta-tracking majorant, #589) against RMC (Li, Yu & Wei 2014) and MCNP (Şeker & Çolak 2003). Nothing is computed in this tab. All 22 points are within ±1000 pcm of RMC, 19 of 22 within ±500; the 6 points at N = 10–12 sit 3 to 8σ low. The earlier record (2026-10-01), measured on a majorant under-bound 14× at 661 eV, read 233 ± 45 pcm (VIII.0) and 310 ± 44 pcm (VII.0) higher on average; it is superseded and not shown.",
            "Fuel-zone k∞: a real single-threaded power iteration in this tab's worker, on ENDF/B-VIII.0 processed here at NJOY's tolerance, with delta tracking. A fuel-zone cube, not the reactor.",
            "Whole core (since 2026-10-08, gh:#786): every core material's data processed on a pool of Web Workers, then a low-statistics k_eff of the whole core (σ about 1000 pcm at 1000 neutrons per generation) with its neutrons drawn. The record's statistics (10 000 × [5 + 135]) would still take hours here.",
            "Education and research only. Not for reactor operation, licensing or safety decisions.",
        ]
    }
    fn loading_note(tier: Tier) -> Option<&'static str> {
        (tier == Tier::Exact).then_some(
            "The fuel-zone k∞ processes eight tapes at NJOY's tolerance 0.001, as the record did: several minutes in a browser. The geometry and layers views need none.",
        )
    }
    fn has_tracks() -> bool {
        false
    }
    /// The liberties toggle: lattice bed beside the DEM bed (gh:#787).
    fn beds() -> bool {
        true
    }
    /// The whole core, live on a worker pool (gh:#786, [`core`]).
    fn core_pool() -> bool {
        true
    }
    fn raster_info() -> Option<RasterInfo> {
        let palette = nee_soon::htr10_rmc::plots::palette().into_iter().map(|(c, n)| (colour(c), n)).collect();
        let xy = |label, centre, half| Preset { label, basis: Basis::Xy, centre, depth: LADDER_Z, half };
        Some(RasterInfo {
            palette,
            ladder: vec![
                Preset { label: "core (side)", basis: Basis::Xz, centre: [0.0, -15.0], depth: 0.0, half: 200.0 },
                xy("bed (plan)", [0.0, 0.0], 95.0),
                xy("pebble", [0.0, 0.0], 3.3),
                xy("TRISO", PARTICLE_XY, 0.12),
                xy("kernel", PARTICLE_XY, 0.035),
            ],
            source: "nee_soon assemble_explicit_triso(14, N, 0): the recorded runs' geometry, sliced by Geometry::locate",
            start: 2,
            param: LADDER_N,
        })
    }
    fn sweep() -> Option<RecordedSweep> {
        use nee_soon::htr10_rmc::{MCNP_TABLE3_KEFF_VS_HEIGHT, MCNP_TABLE4_KEFF_VS_HEIGHT, RMC_KEFF_VS_HEIGHT};
        let n_of = |h: f64| ((h - 6.0) / 9.798).round();
        let lit = |label: &str, colour, pts: &[(f64, f64)]| SweepCurve {
            label: label.into(),
            style: LineStyle::Published,
            colour,
            points: pts.iter().map(|&(h, k)| SweepPoint { x: h, k, sigma: 0.0, tag: Some(n_of(h)), detail: String::new() }).collect(),
        };
        let ours = |csv: &str, lib: &str, colour, label: String, when: &str| {
            let mut rows = csv.lines();
            let head: Vec<&str> = rows.next().unwrap_or("").split(',').collect();
            let col = |n: &str| head.iter().position(|h| *h == n).unwrap_or(usize::MAX);
            let (c_lib, c_n, c_b, c_r, c_k, c_s, c_rmc, c_d, c_ns) =
                (col("library"), col("N"), col("built_height_cm"), col("ref_height_cm"), col("k"), col("sigma"), col("rmc"), col("d_rmc_pcm"), col("n_sigma_rmc"));
            let points = rows
                .filter_map(|l| {
                    let f: Vec<&str> = l.split(',').collect();
                    let num = |i: usize| f.get(i).and_then(|v| v.trim().parse::<f64>().ok());
                    (f.get(c_lib).copied() == Some(lib)).then_some(())?;
                    let (n, b, r, k, s) = (num(c_n)?, num(c_b)?, num(c_r)?, num(c_k)?, num(c_s)?);
                    Some(SweepPoint {
                        x: r,
                        k,
                        sigma: s,
                        tag: Some(n),
                        detail: format!(
                            "{lib} {when}: k = {k:.5} ± {s:.5}, RMC {:.5}: {:+.0} pcm ({:+.1}σ); N = {n}, built bed {b:.1} cm, compared at {r:.1} cm (same ball count)",
                            num(c_rmc).unwrap_or(f64::NAN),
                            num(c_d).unwrap_or(f64::NAN),
                            num(c_ns).unwrap_or(f64::NAN)
                        ),
                    })
                })
                .collect();
            SweepCurve { label, style: LineStyle::Ours, colour, points }
        };
        Some(RecordedSweep {
            title: "k_eff against loading height",
            param: "layers N",
            range: (10.0, 20.0),
            default: LADDER_N,
            x_label: "height (cm); ours at the height with the same ball count",
            curves: {
                let mut c = vec![
                lit("RMC (Li, Yu & Wei 2014): the reference", Color32::from_rgb(235, 235, 235), RMC_KEFF_VS_HEIGHT),
                lit("MCNP vacuum (Şeker & Çolak 2003; Li 2014 Table 3): a gauge", Color32::from_rgb(150, 150, 160), MCNP_TABLE3_KEFF_VS_HEIGHT),
                lit("MCNP helium (Şeker & Çolak 2003; Li 2014 Table 4): a gauge", Color32::from_rgb(110, 160, 140), MCNP_TABLE4_KEFF_VS_HEIGHT),
                ours(SEKER_CSV, "VIII.0", Color32::from_rgb(120, 170, 255), "ours, ENDF/B-VIII.0 (2026-10-07), ±1σ".into(), "(2026-10-07)"),
                ours(SEKER_CSV, "VII.0", Color32::from_rgb(255, 170, 90), "ours, ENDF/B-VII.0 (2026-10-07), ±1σ".into(), "(2026-10-07)"),
                ];
                // A library absent from the table has no curve.
                c.retain(|c| !c.points.is_empty());
                c
            },
            notes: vec![
                "Our points: the 2026-10-07 record (nee_soon htr10_seker_2026_10_07_10k), 10 000 × [5 + 135] per point (σ about 100 pcm), one seed per point, on the bounded delta-tracking majorant (#589). It supersedes the 2026-10-01 record (majorant under-bound 14× at 661 eV; about 230–310 pcm too high on average) and the 2026-10-05 re-measurement of 5 points at [5 + 20].",
                "Pebbles on Şeker & Çolak (2003)'s regular 13-ball lattice cell, not the real random bed. The references use the same lattice, so the comparison is like for like with them, not with the reactor.",
                "Every pebble whole: balls crossing the wall are rejected (gh:#472), so the built height (9.798 N + 6 cm) holds fewer balls than Şeker's; points are compared at equal ball count, by interpolation.",
                "TRISO particles on a lattice inside each fuel pebble, not randomly packed.",
                "Helium at an assumed atmospheric pressure (Şeker & Çolak 2003 p.267), 300.15 K.",
                "The VII.0 arm uses VIII.0 tapes for helium and the rod metals (no VII.0 tapes in the checkout).",
                "The references quote no uncertainty; MCNP is ENDF/B-VI on an independent model, a gauge only.",
                "No k exists for a random (DEM) bed of this core; none is shown or implied.",
            ],
        })
    }
    /// The fuel-zone cube: kernels homogenised (0) or resolved (1).
    /// Defaults 1000 × [20 + 60]. Measured 2026-10-05 in headless Chromium
    /// (2 shared cores, 2.1 GHz Xeon, software rendering): data 283 s, then
    /// 97–124 s per case (43–50 s majorant and source, 54–75 s transport);
    /// resolved 1.5696 ± 0.0050, homogenised 1.4430 ± 0.0049, worth
    /// −12 653 ± 706 pcm (record −12 452 ± 126). Natively the same digits.
    fn kinf_case() -> Option<KinfCase> {
        Some(KinfCase {
            title: "fuel-zone k∞: kernels resolved or smeared",
            param: ("kernels", ""),
            range: (0.0, 1.0),
            default: 1.0,
            choices: vec![(0.0, "homogenised"), (1.0, "resolved")],
            marks: Vec::new(),
            cfg: KeffConfig { n_particles: 1000, n_inactive: 20, n_active: 60, seed: 1, point_source: false, want_sites: false },
            curves: vec![RecordedCurve {
                label: "ours, recorded 2026-10-05 (10 000 × [50 + 200])".into(),
                style: LineStyle::Ours,
                points: vec![(0.0, RECORD_HOM.0, RECORD_HOM.1), (1.0, RECORD_HET.0, RECORD_HET.1)],
            }],
            notes: vec![
                "The record's fuel zone (htr10_fuel_zone_kinf.rs): a 2 cm reflective cube of HTR-10 fuel-zone material, 1018 RSA-packed UO₂ kernels (r = 0.025 cm, IAEA-TECDOC-1382 Table 4-38 densities) in graphite matrix, against the SAME atoms smeared uniformly. Coatings are not resolved (smeared into the matrix), as in the record.",
                "Delta (Woodcock) tracking with the library's Majorant::bounding (a bound by construction since #585), ENDF/B-VIII.0 processed in this tab at NJOY's tolerance 0.001, graphite S(α,β) (30 % porosity law), 293.15 K. A real power iteration, one generation per worker message.",
                "Recorded: resolved 1.57136 ± 0.00088, homogenised 1.44684 ± 0.00090, so smearing the kernels costs −12 452 ± 126 pcm (2026-10-05, c5a6ca4ae). That is the double-heterogeneity worth: lumps (the kernels) shield themselves, as rung 3's lumps did.",
                "Your ± is the spread of one run's active generations, which understates the true σ. This is k∞ of a fuel-zone cube, not the reactor.",
            ],
        })
    }
}

impl RungBuilder for model::DataBuilder {
    type Loaded = Loaded;
    fn new(tier: Tier) -> Self {
        model::DataBuilder::new(tier)
    }
    fn step(&mut self, bytes: &[u8]) -> Result<(), String> {
        model::DataBuilder::step(self, bytes)
    }
    fn finish(self) -> Result<Loaded, String> {
        Ok(Loaded { core: None, fuel: model::DataBuilder::finish(self)?, run: None })
    }
}

/// A fuel-zone run in progress.
pub struct FuelRun {
    pub resolved: bool,
    pub it: DeltaPowerIteration,
    pub total: usize,
}

pub struct Loaded {
    /// The core at the layer count last asked for.
    pub core: Option<(usize, nee_soon::htr10_rmc::core_model::AssembledCore)>,
    pub fuel: Option<model::FuelZone>,
    pub run: Option<FuelRun>,
}

impl LoadedRung for Loaded {
    /// No tracks on this rung (`has_tracks` is false); never asked for.
    fn run_next(&mut self) -> History {
        History::from_track(0, Position::ZERO, 0.0, false, Default::default())
    }
    fn keff_start(&mut self, _cfg: KeffConfig) -> Result<Vec<[f32; 2]>, String> {
        Err("the HTR-10 rung has no k_eff mode".into())
    }
    fn keff_step(&mut self) -> Option<Generation> {
        None
    }
    fn keff_finished(&self) -> bool {
        true
    }
    fn raster(&mut self, req: &RasterReq) -> Result<Vec<u8>, String> {
        let n = (req.param.round() as usize).clamp(10, 20);
        if self.core.as_ref().is_none_or(|c| c.0 != n) {
            self.core = Some((n, model::core(n)));
        }
        let (_, core) = self.core.as_ref().expect("built above");
        Ok(crate::raster::raster(&core.geometry, req))
    }
    /// Start the fuel-zone cube with the kernels resolved (`param` 1) or
    /// homogenised (0): builds the arm's majorant on first use, then samples
    /// the initial source as `run_keff_delta_seq_in` does.
    fn kinf_start(&mut self, param: f64, cfg: KeffConfig) -> Result<(), String> {
        let fuel = self.fuel.as_mut().ok_or("the fuel-zone data are not loaded")?;
        let resolved = param > 0.5;
        let settings = KeffSettings {
            n_particles: cfg.n_particles,
            n_inactive: cfg.n_inactive,
            n_active: cfg.n_active,
            seed: cfg.seed,
            temperature_k: model::TEMP_K,
            ..KeffSettings::default()
        };
        let (mats, _, nucs, packed) = fuel.arm(resolved);
        let domain = DeltaDomain::Cube { half: model::HALF_CM };
        let it = if resolved {
            let q = |p: Position| Some(if packed.is_inside_kernel(p) { 0usize } else { 1usize });
            DeltaPowerIteration::new(domain, mats, nucs, &q, &settings)
        } else {
            DeltaPowerIteration::new(domain, mats, nucs, &|_p: Position| Some(0usize), &settings)
        };
        self.run = Some(FuelRun { resolved, it, total: cfg.n_inactive + cfg.n_active });
        Ok(())
    }
    fn kinf_step(&mut self) -> Option<KinfGeneration> {
        let run = self.run.as_mut()?;
        let fuel = self.fuel.as_mut()?;
        let (mats, maj, nucs, packed) = fuel.arm(run.resolved);
        let g = if run.resolved {
            let q = |p: Position| Some(if packed.is_inside_kernel(p) { 0usize } else { 1usize });
            run.it.step(mats, nucs, maj, &q)?
        } else {
            run.it.step(mats, nucs, maj, &|_p: Position| Some(0usize))?
        };
        let (mean, sem) = g.k_mean.unwrap_or((f64::NAN, f64::NAN));
        Some(KinfGeneration { param: if run.resolved { 1.0 } else { 0.0 }, index: g.index, total: run.total, active: g.active, k: g.k, mean, sem })
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use outram_mc_libs::geometry::cell::SurfaceToken;
    use outram_mc_libs::geometry::position::Direction;

    /// The ladder's x-y plane runs through a kernel of the fuel pebble at the
    /// origin of the N = 12 core: the TRISO and kernel steps show fuel.
    #[test]
    fn the_ladder_lands_on_a_kernel() {
        let core = model::core(LADDER_N as usize);
        let p = Position::new(PARTICLE_XY[0], PARTICLE_XY[1], LADDER_Z);
        let m = core.geometry.locate(p, Direction::new(0.0, 0.0, 1.0), SurfaceToken::NONE).and_then(|l| l.material);
        assert_eq!(m, Some(nee_soon::htr10_rmc::core_model::mat::KERNEL));
        // A raster of the kernel step has kernel at its centre.
        let mut l = Loaded { core: None, fuel: None, run: None };
        let req = RasterReq { id: 1, basis: Basis::Xy, centre: PARTICLE_XY, depth: LADDER_Z, width: [0.07, 0.07], px: [21, 21], param: LADDER_N };
        let map = l.raster(&req).unwrap();
        assert_eq!(map.len(), 441);
        assert_eq!(map[10 * 21 + 10] as usize, nee_soon::htr10_rmc::core_model::mat::KERNEL);
    }

    /// The sweep reads every recorded row of the 2026-10-07 record (gh:#782;
    /// 11 points per library, each with its N) and the three reference
    /// curves, and nothing from the superseded 2026-10-01 / 2026-10-05
    /// records. Pinned on the record's VII.0 N = 12 row (k = 0.996470).
    #[test]
    fn the_sweep_reads_the_record() {
        let s = Htr10::sweep().unwrap();
        let ours: Vec<_> = s.curves.iter().filter(|c| c.style == LineStyle::Ours).collect();
        assert_eq!(s.curves.len() - ours.len(), 3);
        assert_eq!(ours.len(), 2);
        for c in &ours {
            assert_eq!(c.points.len(), 11, "{}", c.label);
            assert!(c.points.iter().all(|p| p.sigma > 0.0 && p.tag.is_some()), "{}", c.label);
            assert!(c.label.contains("2026-10-07"), "{}", c.label);
        }
        let d12 = s.details(12.0);
        assert!(d12.iter().any(|d| d.contains("VII.0 (2026-10-07): k = 0.99647")), "{d12:?}");
        assert!(d12.iter().all(|d| !d.contains("superseded")), "{d12:?}");
    }

    /// The fuel-zone case natively at the slider's defaults, both arms, on
    /// tolerance-0.001 data: prints the timings quoted on gh:#528 and checks
    /// each arm is within 5 of its own sigma of the record (a harness check:
    /// other settings and stream). Opt-in (processes 8 tapes, minutes).
    #[test]
    #[ignore = "processes 8 ENDF tapes (minutes); run with --ignored"]
    fn the_fuel_zone_case_runs_both_arms() {
        let mut b = model::DataBuilder::new(Tier::Exact);
        let t = std::time::Instant::now();
        for (label, tape) in model::JOBS {
            let raw = std::fs::read(njoy_outram_park_fork::reference_data::reference_endf(tape).expect("tape")).expect("read");
            let t1 = std::time::Instant::now();
            b.step(&crate::tapes::strip_covariances(&raw)).expect("step");
            eprintln!("  {label:<16} {:6.1} s", t1.elapsed().as_secs_f64());
        }
        let mut l = Loaded { core: None, fuel: b.finish().expect("finish"), run: None };
        eprintln!("  data {:.1} s", t.elapsed().as_secs_f64());
        let cfg = Htr10::kinf_case().unwrap().cfg;
        for (arm, rec) in [(1.0, RECORD_HET), (0.0, RECORD_HOM)] {
            let t = std::time::Instant::now();
            l.kinf_start(arm, cfg).expect("start");
            let t_start = t.elapsed().as_secs_f64();
            // The majorant must bound Sigma_t on this rung's data (#585):
            // Majorant::audit over 2 M log energies plus every breakpoint.
            {
                let fuel = l.fuel.as_mut().unwrap();
                let (mats, maj, nucs, _) = fuel.arm(arm > 0.5);
                let a = maj.audit(mats, nucs, 1.0e-5, 2.0e7, 2_000_000);
                eprintln!("  arm {arm}: majorant audit worst Sigma_t/Sigma_maj = {:.4} at {:.4e} eV (material {}, {} energies)", a.worst_ratio, a.energy_ev, a.material, a.energies_checked);
                assert!(a.worst_ratio <= 1.0, "under-bound majorant: {}", a.worst_ratio);
            }
            let t = std::time::Instant::now();
            let mut last = None;
            while let Some(g) = l.kinf_step() {
                last = Some(g);
            }
            let g = last.unwrap();
            eprintln!(
                "  arm {arm}: majorant + source {t_start:.1} s, transport {:.1} s, {} x [{} + {}]: k_inf = {:.5} +/- {:.5} (record {:.5} +/- {:.5})",
                t.elapsed().as_secs_f64(), cfg.n_particles, cfg.n_inactive, cfg.n_active, g.mean, g.sem, rec.0, rec.1
            );
            assert!((g.mean - rec.0).abs() < 5.0 * g.sem, "arm {arm}");
        }
    }
}
