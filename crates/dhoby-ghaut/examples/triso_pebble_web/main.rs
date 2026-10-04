//! **TRISO pebble** — a 2D analogue of an HTR-10 fuel pebble in a reflective
//! cell, transporting ONE neutron at a time on real ENDF/B-VIII.0 data and
//! drawing each neutron's track as it goes. Single-threaded; runs natively and
//! in the browser (`web/triso_pebble/`).
//!
//! ```text
//! cargo run -p dhoby-ghaut --example triso_pebble_web --release
//! cargo run -p dhoby-ghaut --example triso_pebble_web --release -- --headless [n] [seed]
//! cargo run -p dhoby-ghaut --example triso_pebble_web --release -- --render-geometry <dir>
//! cargo run -p dhoby-ghaut --example triso_pebble_web --release -- --prepare-web-data <dir>
//! cargo test -p dhoby-ghaut --example triso_pebble_web --release
//! ```
//!
//! - **Physics**: outram-mc-libs continuous-energy transport, unmodified — see
//!   [`sim`]. Graphite carries the crystalline-graphite S(alpha,beta).
//! - **Data**: ENDF/B-VIII.0 tapes from `reference-data/endf/`, processed by
//!   the workspace's own NJOY port (RECONR + BROADR) — in the browser, on the
//!   user's machine. At the `VeryFast` tier (tolerance 0.01), an approximation
//!   chosen for this demo; see [`model::SPEED`].
//! - **Geometry**: IAEA-TECDOC-1382 dimensions, with what the 2D reduction does
//!   and does not preserve set out in [`model`].
//!
//! Education and research only, per the workspace `RESPONSIBLE_USE.md`. Not for
//! reactor operation, licensing, or safety-critical decisions.

#[cfg(not(target_os = "android"))]
mod app;
#[cfg(not(target_os = "android"))]
mod model;
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
mod render;
#[cfg(not(target_os = "android"))]
mod sim;

/// Android stub: windowing GUIs are out of scope on Termux (the workspace
/// example rule — a blanked file gives "main function not found").
#[cfg(target_os = "android")]
fn main() {
    eprintln!("triso_pebble_web is a windowing GUI and is not built for Android.");
}

/// Read a reference tape from `reference-data/endf/` and strip its covariances
/// — the same bytes the browser gets once it has inflated the download.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn native_tape(tape: &str) -> Result<Vec<u8>, String> {
    let path = njoy_outram_park_fork::reference_data::reference_endf(tape)
        .ok_or_else(|| format!("reference tape {tape} is not present"))?;
    let raw = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(model::strip_covariances(&raw))
}

/// Process every tape, in order. `report` sees each job's label and seconds.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn load_native(mut report: impl FnMut(&str, f64)) -> Result<sim::Physics, String> {
    let mut b = model::DataBuilder::default();
    while let Some(job) = b.next_job() {
        let t = std::time::Instant::now();
        b.step(&native_tape(job.tape)?)?;
        report(job.label, t.elapsed().as_secs_f64());
    }
    Ok(sim::Physics::new(b.finish()?))
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| args.get(i).map(String::as_str);
    let timing = |label: &str, s: f64| eprintln!("  {label:<16} {s:6.1} s");
    match arg(0) {
        Some("--headless") => {
            let n = arg(1).map_or(Ok(200), str::parse).map_err(|e| format!("n: {e}"))?;
            let seed = arg(2).map_or(Ok(1), str::parse).map_err(|e| format!("seed: {e}"))?;
            eprintln!("Processing ENDF/B-VIII.0 (tier {:?}, {} K):", model::SPEED, model::TEMPERATURE_K);
            let phys = load_native(timing)?;
            print!("{}", sim::headless_csv(&phys, n, seed));
            Ok(())
        }
        Some("--render-geometry") => {
            let dir = arg(1).ok_or("--render-geometry needs an output directory")?;
            render::render_all(std::path::Path::new(dir))
        }
        Some("--prepare-web-data") => {
            let dir = std::path::Path::new(arg(1).ok_or("--prepare-web-data needs an output directory")?);
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            println!("{:<34} {:>10} {:>10} {:>10}", "tape", "raw MB", "stripped", "on wire");
            let (mut raw_t, mut wire_t) = (0usize, 0usize);
            for job in model::JOBS {
                let path = njoy_outram_park_fork::reference_data::reference_endf(job.tape)
                    .ok_or_else(|| format!("missing {}", job.tape))?;
                let raw = std::fs::read(&path).map_err(|e| e.to_string())?;
                let stripped = model::strip_covariances(&raw);
                let wire = model::compress(&stripped);
                std::fs::write(dir.join(model::wire_name(job.tape)), &wire).map_err(|e| e.to_string())?;
                let mb = |n: usize| n as f64 / 1.0e6;
                println!("{:<34} {:>10.2} {:>10.2} {:>10.2}", job.tape, mb(raw.len()), mb(stripped.len()), mb(wire.len()));
                raw_t += raw.len();
                wire_t += wire.len();
            }
            println!("total: {:.1} MB of tapes -> {:.1} MB downloaded", raw_t as f64 / 1e6, wire_t as f64 / 1e6);
            Ok(())
        }
        Some(other) => Err(format!("unknown argument {other}")),
        None => {
            let options = eframe::NativeOptions::default();
            eframe::run_native(
                "TRISO pebble — one neutron at a time",
                options,
                Box::new(|cc| Ok(Box::new(app::TrisoApp::new(cc, app::Source::native())))),
            )
            .map_err(|e| e.to_string())
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    let web_options = eframe::WebOptions {
        // WebGL2 rather than WebGPU, which is still not universal.
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window().expect("no window").document().expect("no document");
        let canvas = document
            .get_element_by_id("triso_canvas")
            .expect("no #triso_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#triso_canvas is not a canvas");
        let started = eframe::WebRunner::new()
            .start(canvas, web_options, Box::new(|cc| Ok(Box::new(app::TrisoApp::new(cc, app::Source::web(&cc.egui_ctx))))))
            .await;
        if let Some(el) = document.get_element_by_id("loading") {
            match started {
                Ok(()) => el.remove(),
                Err(e) => el.set_inner_html(&format!("<p>Failed to start: {e:?}</p>")),
            }
        }
    });
}

// ─── Tests (headless; workspace hard rule) ───────────────────────────────────

#[cfg(all(test, not(target_os = "android"), not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use outram_mc_libs::geometry::cell::SurfaceToken;
    use outram_mc_libs::geometry::position::{Direction, Position};
    use outram_mc_libs::material::material::{Material, NuclideComponent};
    use std::sync::OnceLock;

    const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/triso_pebble_web_headless.csv");
    const FIXTURE_N: usize = 40;
    const FIXTURE_SEED: u64 = 1;

    fn physics() -> &'static sim::Physics {
        static P: OnceLock<sim::Physics> = OnceLock::new();
        P.get_or_init(|| load_native(|_, _| {}).expect("load nuclear data"))
    }

    /// Covariance stripping must change nothing transport reads. Checked on
    /// O-16 (39 of its 42 MB are covariances) and U-235 (whose stripped MF 35
    /// is the fission-spectrum covariance — so the spectrum itself must be
    /// untouched): cross sections bit-identical on a dense log grid, and
    /// fission-energy samples bit-identical for the same seeds.
    #[test]
    fn stripping_covariances_changes_nothing_transport_reads() {
        for (label, tape) in [("O-16", "n-008_O_016-ENDF8.0.endf"), ("U-235", "n-092_U_235-ENDF8.0.endf")] {
            let path = njoy_outram_park_fork::reference_data::reference_endf(tape).expect("tape present");
            let raw = std::fs::read(path).unwrap();
            let stripped = model::strip_covariances(&raw);
            assert!(stripped.len() < raw.len() / 2, "{label}: stripping removed too little");
            let full = model::nuclide_from_bytes(&raw, label).unwrap();
            let thin = model::nuclide_from_bytes(&stripped, label).unwrap();
            assert_eq!(full.awr.to_bits(), thin.awr.to_bits(), "{label}: AWR");
            let one = Material {
                id: 1,
                name: label.into(),
                components: vec![NuclideComponent { nuclide_idx: 0, atom_density: 1.0 }],
                temperature: model::TEMPERATURE_K,
            };
            for k in 0..=2000 {
                let e = 1.0e-5 * 10f64.powf(12.3 * k as f64 / 2000.0); // 1e-5 eV .. 20 MeV
                let (a, b) = (one.macro_xs(e, std::slice::from_ref(&full)), one.macro_xs(e, std::slice::from_ref(&thin)));
                for (what, x, y) in [
                    ("total", a.total, b.total), ("elastic", a.elastic, b.elastic),
                    ("absorption", a.absorption, b.absorption), ("fission", a.fission, b.fission),
                    ("nu_fission", a.nu_fission, b.nu_fission),
                ] {
                    assert_eq!(x.to_bits(), y.to_bits(), "{label}: {what} at {e:e} eV: {x} vs {y}");
                }
            }
            if label == "U-235" {
                for (seed, e_in) in (0..200u64).zip([0.0253, 1.0, 1.0e3, 1.0e6].into_iter().cycle()) {
                    let (mut s1, mut s2) = (seed, seed);
                    let (x, y) = (full.sample_fission_energy(e_in, &mut s1), thin.sample_fission_energy(e_in, &mut s2));
                    assert_eq!(x.to_bits(), y.to_bits(), "U-235 fission spectrum, seed {seed}");
                }
            }
        }
    }

    #[test]
    fn the_2d_reduction_keeps_the_pebble_inventory_ratios() {
        // 8335 x 0.0455 / 2.5 = 151.7 -> 152 rods.
        assert_eq!(model::particles_2d(), 152);
        assert!((model::particle_r() - 0.0455).abs() < 1e-12);
        let p = model::half_pitch();
        let area_fraction = std::f64::consts::PI * model::PEBBLE_R.powi(2) / (2.0 * p).powi(2);
        assert!((area_fraction - model::CORE_FILLING_FRACTION).abs() < 1e-12);
    }

    /// The assembled geometry, queried as the solver queries it.
    #[test]
    fn the_assembled_geometry_puts_every_material_where_it_belongs() {
        let c = model::particle_centres(model::LAYOUT_SEED);
        let g = model::build_geometry(&c);
        let dir = Direction::new(1.0, 0.0, 0.0);
        let at = |x: f64, y: f64| g.locate(Position::new(x, y, 0.0), dir, SurfaceToken::NONE).map(|p| p.material);
        let rp = model::particle_r();
        for (i, &(x, y)) in c.iter().enumerate() {
            assert!((x * x + y * y).sqrt() + rp < model::FUEL_ZONE_R, "particle {i} crosses the fuel-zone edge");
            for &(a, b) in &c[i + 1..] {
                assert!(((x - a).powi(2) + (y - b).powi(2)).sqrt() > 2.0 * rp, "particles overlap");
            }
            // Probe the middle of every layer along +x.
            let mids = [
                (0.0, model::MAT_KERNEL),
                ((model::KERNEL_R + model::buffer_r()) / 2.0, model::MAT_BUFFER),
                ((model::buffer_r() + model::ipyc_r()) / 2.0, model::MAT_IPYC),
                ((model::ipyc_r() + model::sic_r()) / 2.0, model::MAT_SIC),
                ((model::sic_r() + rp) / 2.0, model::MAT_OPYC),
            ];
            for (dx, m) in mids {
                assert_eq!(at(x + dx, y), Some(Some(m)), "particle {i}, expected {}", model::MATERIAL_NAMES[m]);
            }
        }
        assert_eq!(at(0.0, (model::FUEL_ZONE_R + model::PEBBLE_R) / 2.0), Some(Some(model::MAT_SHELL)));
        let p = model::half_pitch();
        assert_eq!(at(0.98 * p, 0.98 * p), Some(None), "cell corner is helium (void)");
        assert!(at(1.01 * p, 0.0).is_none(), "outside the reflective cell is outside the geometry");
    }

    #[test]
    fn the_headless_run_is_deterministic() {
        assert_eq!(sim::headless_csv(physics(), 12, 7), sim::headless_csv(physics(), 12, 7));
    }

    /// A harness check, NOT validation: every history must end in a physical
    /// absorption inside the cell, with no lost or truncated tracks.
    #[test]
    fn every_history_ends_physically_inside_the_cell() {
        let phys = physics();
        let mut chain = sim::Chain::new(3);
        let p = model::half_pitch();
        for _ in 0..60 {
            let h = chain.run_next(phys);
            assert!(matches!(sim::outcome_name(h.outcome), "fission" | "capture"), "history {}: {}", h.index, sim::outcome_name(h.outcome));
            assert_eq!(h.track.dropped_states, 0, "history {} truncated", h.index);
            assert!(h.track.states.len() >= 2);
            for s in &h.track.states {
                assert!(s.r.x.abs() <= p + 1e-9 && s.r.y.abs() <= p + 1e-9, "history {} left the cell", h.index);
                assert!(s.energy > 1.0e-6 && s.energy < 3.0e7, "history {}: energy {} eV", h.index, s.energy);
            }
            assert!(h.path_cm.is_finite() && h.path_cm > 0.0);
        }
    }

    #[test]
    fn the_headless_run_matches_the_committed_fixture() {
        let got = sim::headless_csv(physics(), FIXTURE_N, FIXTURE_SEED);
        if std::env::var_os("TRISO_BLESS").is_some() {
            std::fs::write(FIXTURE, &got).unwrap();
        }
        let want = std::fs::read_to_string(FIXTURE).expect("fixture missing: rerun with TRISO_BLESS=1");
        assert_eq!(got, want, "headless trace differs from {FIXTURE}");
    }
}
