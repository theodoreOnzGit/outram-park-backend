//! # Dhoby Ghaut's guided high-fidelity workbench (gh:#561), first slice
//!
//! A native window that walks a guided build: pick a reactor type by
//! generation, Basic or Advanced, the HTGR core, then Steps 0–11 with every
//! value prefilled and cited, a literature pane beside the model, and a
//! recipe (kovan markdown) to save and load. Steps 0–5 work for HTGR →
//! Basic → pebble bed (HTR-10); ~~Steps 6–11 are shown with the issue that will
//! build each~~ **UPDATED 2026-10-05:** Step 6 (branch; the reactivity map,
//! `step6.rs`), Steps 9–10 (a SIMPLIFIED coupled case, gh:#574: r-z porous-core
//! thermal-hydraulics, prescribed power shape, lumped feedback) and Step 11
//! (exports, `step11.rs`) work too; Steps 7–8 are shown with the issue that
//! will build each.
//!
//! ```text
//! cargo run --release -p dhoby-ghaut --bin dhoby-ghaut
//! cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --recipe my_recipe.md
//! cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --headless-geometry
//! cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --scan-endf ~/ENDF-B-VIII.0
//! cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --render-review out_dir
//! cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --headless-multiphysics \
//!     [--rings 5 --axial 40 --uniform-power] [--out out_dir]
//! cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --headless-keff \
//!     [--particles 500 --inactive 10 --active 20 --threads 8] [--out out_dir]
//! cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --headless-map \
//!     [--synthetic | --sweep 300,600,900] [--order 1] [--basis sqrt|ln|linear] [--out out_dir]
//! ```
//!
//! Every headless mode also takes `--rod-insertion F` (0 withdrawn to 1 fully
//! inserted, gh:#580) and `--endf-dir DIR` (the folder the k_eff load reads
//! its tapes from, flat or an extracted library, gh:#581), overriding the
//! recipe. `--scan-endf DIR` also prints where the load would read each tape.
//!
//! **Research, education and V&V only.** The HTR-10 model is the TENTATIVE
//! model of the RMC code-to-code record (`nee_soon::htr10_rmc`); see the V&V
//! status the preset card shows. An offline demonstration: never connect it
//! to an operational system.
//!
//! **Headless modes.** `--headless-geometry` assembles the recipe's
//! geometry and prints one CSV row of its facts (cells, tiles, balls, …),
//! pinned by `tests/fixtures/dhoby_ghaut_geometry.csv`. `--render-review`
//! writes the review gate's images. `--headless-keff` runs Step 5 with no
//! window, prints the console and the spectrum, and saves the recipe with the
//! run appended. No test runs it: the nuclear data alone take minutes.
//! `--headless-map` (`headless_map.rs`) fits the reactivity map over the
//! recipe's runs, SYNTHETIC runs (`--synthetic`, pinned by a test) or a real
//! Monte Carlo sweep (`--sweep 300,600,900`), and writes Step 11's exports.
//!
//! `--headless-multiphysics` runs Steps 9-10 (the simplified coupled case)
//! and prints the coupling console and the TENTATIVE comparison with Gao &
//! Shi (2002); its summary is pinned by
//! `tests/fixtures/dhoby_ghaut_multiphysics.csv`.

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod app;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod coupled;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod coupled_ui;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod dem;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod design;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod engine;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod literature;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod mp_headless;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod mp_preset;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod porous_core;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod preset;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod results;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod slice_view;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod headless_map;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod step11;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod step6;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod steps_ui;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod view3d;

#[cfg(any(target_os = "android", target_arch = "wasm32"))]
fn main() {
    eprintln!("dhoby-ghaut is a native desktop GUI; it is not built for Android or the browser.");
}

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |k: &str| {
        args.iter()
            .position(|a| a == k)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let mut recipe_step9 = None;
    let recipe = match arg("--recipe") {
        Some(p) => {
            let text = std::fs::read_to_string(&p).map_err(|e| format!("{p}: {e}"))?;
            recipe_step9 = dhoby_ghaut::workbench::multiphysics::MultiphysicsSetup::from_recipe_markdown(&text)
                .map(|r| r.map_err(|e| format!("{p}: {e}")));
            Some(
                dhoby_ghaut::workbench::recipe::Recipe::from_markdown(&text)
                    .map_err(|e| format!("{p}: {e}"))?,
            )
        }
        None => None,
    };
    // Overrides of the recipe for the headless modes: the rods' insertion
    // (fraction of travel, gh:#580) and the ENDF folder (gh:#581).
    let mut recipe = recipe;
    let overrides = |r: &mut dhoby_ghaut::workbench::recipe::Recipe| -> Result<(), String> {
        if let Some(v) = arg("--rod-insertion") {
            r.monte_carlo.rod_insertion = v.parse().map_err(|e| format!("--rod-insertion {v}: {e}"))?;
        }
        if let Some(d) = arg("--endf-dir") {
            r.nuclear_data.endf_dir = d;
        }
        Ok(())
    };
    if arg("--rod-insertion").is_some() || arg("--endf-dir").is_some() {
        let mut r = recipe.take().unwrap_or_else(preset::htr10);
        overrides(&mut r)?;
        recipe = Some(r);
    }
    if let Some(dir) = arg("--scan-endf") {
        let (layout, tapes) = engine::scan_endf(std::path::Path::new(&dir));
        println!("{dir}: {layout:?}, {} tapes", tapes.len());
        for n in engine::needed_tapes() {
            let found = n.identity().find(&tapes).map(|t| t.file.clone());
            println!(
                "{:<34} MAT {:>5?} NSUB {:>6?} -> {}",
                n.file,
                n.mat,
                n.nsub,
                found.unwrap_or_else(|e| format!("MISSING ({e:?})"))
            );
        }
        // What the k_eff load would read, resolved the way it resolves them.
        use nee_soon::htr10_rmc::data::{resolve_tapes, Htr10DataConfig, Htr10NuclideLayout};
        let cfg = Htr10DataConfig {
            tapes: engine::tape_source(std::path::Path::new(&dir)),
            ..Htr10DataConfig::default()
        };
        let layout = Htr10NuclideLayout::plan(&cfg).map_err(|e| e.to_string())?;
        match resolve_tapes(&cfg, &layout) {
            Ok(v) => {
                println!("the k_eff load would read ({:?}):", cfg.tapes);
                for (n, p) in v {
                    println!("  {n:<20} {}", p.display());
                }
            }
            Err(e) => println!("the k_eff load would stop: {e}"),
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--headless-geometry") {
        let r = recipe.unwrap_or_else(preset::htr10);
        match arg("--centres") {
            Some(f) => println!("{}", headless::geometry_csv_from_centres(&r, std::path::Path::new(&f))?),
            None => println!("{}", headless::geometry_csv(&r)),
        }
        return Ok(());
    }
    if let Some(dir) = arg("--render-review") {
        let r = recipe.unwrap_or_else(preset::htr10);
        return headless::render_review(&r, std::path::Path::new(&dir), arg("--centres").as_deref().map(std::path::Path::new));
    }
    if args.iter().any(|a| a == "--headless-map") {
        return headless_map::run(&args, recipe);
    }
    if args.iter().any(|a| a == "--headless-keff") {
        let mut r = recipe.unwrap_or_else(preset::htr10);
        let num = |k: &str| arg(k).and_then(|v| v.parse::<usize>().ok());
        let mc = &mut r.monte_carlo;
        mc.particles = num("--particles").unwrap_or(mc.particles);
        mc.inactive = num("--inactive").unwrap_or(mc.inactive);
        mc.active = num("--active").unwrap_or(mc.active);
        mc.threads = num("--threads").unwrap_or(mc.threads);
        let out = arg("--out").unwrap_or_else(|| "target/dhoby-ghaut_out".into());
        return headless::keff(r, std::path::Path::new(&out));
    }
    if args.iter().any(|a| a == "--headless-multiphysics") {
        let r = recipe.unwrap_or_else(preset::htr10);
        let mut setup = match recipe_step9 {
            Some(s) => s?,
            None => mp_preset::htr10(r.nuclear_data.temperature_k),
        };
        let num = |k: &str| arg(k).and_then(|v| v.parse::<usize>().ok());
        setup.foam.radial_rings = num("--rings").unwrap_or(setup.foam.radial_rings);
        setup.foam.axial_nodes = num("--axial").unwrap_or(setup.foam.axial_nodes);
        if args.iter().any(|a| a == "--uniform-power") {
            setup.neutronics.shape = dhoby_ghaut::workbench::multiphysics::PowerShape::Uniform;
        }
        let out = arg("--out").unwrap_or_else(|| "target/dhoby-ghaut_out".into());
        return mp_headless::headless(&r, &setup, std::path::Path::new(&out));
    }
    let start_step: Option<u8> = arg("--step").and_then(|v| v.parse().ok());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1500.0, 950.0])
            .with_title("Dhoby Ghaut workbench"),
        ..Default::default()
    };
    eframe::run_native(
        "Dhoby Ghaut workbench",
        options,
        Box::new(move |cc| {
            let mut a = app::App::new(cc, recipe, recipe_step9);
            // `--step N` (with `--recipe`): open the wizard at Step N, e.g.
            // for screenshots. Navigation only: the review gate still guards
            // every Monte Carlo run.
            if let Some(n) = start_step {
                if let Some(s) = dhoby_ghaut::workbench::steps::WizardStep::ALL.into_iter().find(|s| s.number() == Some(n)) {
                    a.step = s;
                }
            }
            Ok(Box::new(a))
        }),
    )
    .map_err(|e| e.to_string())
}

#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod headless {
    use std::path::Path;

    use dhoby_ghaut::web_demo::link::NativeEngine;
    use dhoby_ghaut::workbench::recipe::{now_rfc3339, Recipe, RunRecord};

    use crate::engine::{AssemblyInfo, Engine, Ev, KeffJob, Req};

    /// Assemble `r`'s geometry, built to its design ([`crate::design::plan`]);
    /// what the design cannot represent is printed to stderr.
    fn assemble(engine: &mut Engine, r: &Recipe) -> Option<AssemblyInfo> {
        let plan = crate::design::plan(r);
        for n in &plan.not_built {
            eprintln!("NOT built: {n}");
        }
        let mut info = None;
        engine.handle(
            Req::Assemble {
                rings: r.pebble_bed.rings,
                layers: r.pebble_bed.layers,
                design: plan.design,
            },
            &mut |e| match e {
                Ev::Assembled(i, _) => info = Some(i),
                Ev::Error(m) => eprintln!("{m}"),
                _ => {}
            },
        );
        info
    }

    /// One CSV header and row of the assembled geometry's facts. Timing is
    /// left out so the row is reproducible.
    pub fn geometry_csv(r: &Recipe) -> String {
        let mut engine = Engine::default();
        let Some(a) = assemble(&mut engine, r) else {
            return "assembly failed".into();
        };
        let f = |v: Option<[f64; 3]>| {
            v.map_or("none".to_string(), |p| {
                format!("{:.4} {:.4} {:.4}", p[0] + 0.0, p[1] + 0.0, p[2] + 0.0)
            })
        };
        format!(
            "rings,layers,tiles,cells,universes,balls,bed_radius_cm,bed_height_cm,z_bottom_cm,z_top_cm,fuel_pebble_cm,triso_cm\n\
             {},{},{},{},{},{},{:.4},{:.4},{:.4},{:.4},{},{}",
            a.rings,
            a.layers,
            a.tiles,
            a.cells,
            a.universes,
            a.balls.map_or("none".into(), |b| b.to_string()),
            a.bed_radius,
            a.bed_height,
            a.z_range[0],
            a.z_range[1],
            f(a.pebble),
            f(a.particle),
        )
    }

    /// Pebble centres \[m, DEM frame\] from a `reference-data/liggghts/`-format
    /// CSV (`id,x,y,z,...`).
    fn read_centres(csv: &Path) -> Result<Vec<[f64; 3]>, String> {
        let text = std::fs::read_to_string(csv).map_err(|e| format!("{}: {e}", csv.display()))?;
        text.lines()
            .skip(1)
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                let f: Vec<f64> = l.split(',').filter_map(|v| v.trim().parse().ok()).collect();
                if f.len() < 4 { Err(format!("bad row: {l}")) } else { Ok([f[1], f[2], f[3]]) }
            })
            .collect()
    }

    /// [`geometry_csv`] for a bed of pebble centres read from a CSV in the
    /// `reference-data/liggghts/` format (`id,x,y,z,...`, metres, DEM frame),
    /// e.g. the output of `outram-park-fork-liggghts`' `htr10_fresh_fill`.
    pub fn geometry_csv_from_centres(r: &Recipe, csv: &Path) -> Result<String, String> {
        let centres_m = read_centres(csv)?;
        let mut engine = Engine::default();
        let mut info = None;
        let design = crate::design::plan(r).design;
        engine.handle(Req::AssembleFromCentres { centres_m, rings: r.pebble_bed.rings, design }, &mut |e| match e {
            Ev::Assembled(i, _) => info = Some(i),
            Ev::Error(m) => eprintln!("{m}"),
            _ => {}
        });
        let a = info.ok_or("assembly from centres failed")?;
        Ok(format!(
            "dem_pebbles,tiles,cells,universes,core_balls,bed_height_cm,z_bottom_cm,z_top_cm,fuel_pebble_found,seconds\n\
             {},{},{},{},{},{:.4},{:.4},{:.4},{},{:.1}",
            a.dem_pebbles.unwrap_or(0),
            a.tiles,
            a.cells,
            a.universes,
            a.balls.map_or("none".into(), |b| b.to_string()),
            a.bed_height,
            a.z_range[0],
            a.z_range[1],
            a.pebble.is_some(),
            a.seconds
        ))
    }

    pub fn render_review(r: &Recipe, dir: &Path, centres: Option<&Path>) -> Result<(), String> {
        let mut engine = Engine::default();
        let a = match centres {
            None => assemble(&mut engine, r).ok_or("assembly failed")?,
            Some(csv) => {
                let centres_m = read_centres(csv)?;
                let mut info = None;
                let design = crate::design::plan(r).design;
                engine.handle(Req::AssembleFromCentres { centres_m, rings: r.pebble_bed.rings, design }, &mut |e| {
                    if let Ev::Assembled(i, _) = e {
                        info = Some(i);
                    }
                });
                info.ok_or("assembly from centres failed")?
            }
        };
        let presets = crate::steps_ui::review_presets_for(&a);
        for p in presets {
            let origin = crate::slice_view::SliceView::origin(p.basis, p.depth, p.centre);
            let name = p.name.to_lowercase().replace([' ', '(', ')', '-'], "_");
            let path = dir.join(format!("review_{name}.png"));
            let title = format!(
                "HTR-10 {}X{}{}: {}",
                r.pebble_bed.rings,
                r.pebble_bed.layers,
                rods_label(a.design.rod_insertion),
                p.name.to_uppercase()
            );
            let w = 2.0 * p.half_extent;
            engine.handle(
                Req::ExportPng {
                    basis: p.basis,
                    origin,
                    width: [w, w],
                    pixels: [1000, 1000],
                    title,
                    path,
                },
                &mut |e| match e {
                    Ev::Exported(p) => println!("wrote {}", p.display()),
                    Ev::Error(m) => eprintln!("{m}"),
                    _ => {}
                },
            );
        }
        Ok(())
    }

    /// ", RODS 0.50 IN" in an image title when the rods are not withdrawn.
    pub fn rods_label(f: f64) -> String {
        if f > 0.0 {
            format!(", RODS {f:.2} IN")
        } else {
            String::new()
        }
    }

    pub fn keff(mut r: Recipe, out: &Path) -> Result<(), String> {
        let mut engine = Engine::default();
        let a = assemble(&mut engine, &r).ok_or("assembly failed")?;
        println!(
            "assembled: {} cells, {} tiles, {:?} balls, rods at z_T {:?} cm ({:.1} s)",
            a.cells, a.tiles, a.balls, a.rod_lower_end_zt_cm, a.seconds
        );
        let tapes = crate::engine::tape_source(Path::new(&r.nuclear_data.endf_dir));
        println!("tapes: {tapes:?}");
        let mc = r.monte_carlo.clone();
        let label = format!("Run {}", mc.runs.len() + 1);
        let job = KeffJob {
            live: Default::default(),
            label: label.clone(),
            particles: mc.particles,
            inactive: mc.inactive,
            active: mc.active,
            seed: mc.seed,
            threads: mc.threads,
            temperature_k: r.nuclear_data.temperature_k,
            bins_per_decade: mc.spectrum_bins_per_decade,
            tapes,
        };
        let mut outcome = None;
        let mut err = None;
        engine.handle(Req::RunKeff(job), &mut |e| match e {
            Ev::Data(nee_soon::htr10_rmc::data::LoadProgress::Finished { item, seconds }) => {
                println!("  data {item:<20} {seconds:6.1} s")
            }
            Ev::DataReady { seconds, .. } => println!("nuclear data ready ({seconds:.1} s)"),
            Ev::KeffStarted { planned_histories } => {
                println!("transport: {planned_histories} histories planned")
            }
            Ev::KeffDone(o) => outcome = Some(o),
            Ev::Error(m) => err = Some(m),
            _ => {}
        });
        if let Some(m) = err {
            return Err(m);
        }
        let o = outcome.ok_or("no result")?;
        println!(" Bat./Gen.      k       Entropy         Average k");
        for l in crate::results::console_lines(&o, mc.inactive) {
            println!("{l}");
        }
        println!(
            " k-effective = {:.5} +/- {:.5}  ({} histories, {} lost, {:.1} s)",
            o.k, o.sigma, o.histories, o.lost_locate, o.transport_s
        );
        for n in &o.notes {
            println!(" note: {n}");
        }
        std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
        let mut csv = String::from("e_lo_ev,e_hi_ev,phi_per_lethargy,rel_err\n");
        for (i, (v, e)) in o.phi_per_lethargy.iter().enumerate() {
            csv.push_str(&format!(
                "{:.6e},{:.6e},{v:.6e},{e:.4e}\n",
                o.edges[i],
                o.edges[i + 1]
            ));
        }
        let spec = out.join("spectrum.csv");
        std::fs::write(&spec, csv).map_err(|e| e.to_string())?;
        r.monte_carlo.runs.push(RunRecord {
            label,
            rod_insertion: o.rod_insertion,
            temperature_k: r.nuclear_data.temperature_k,
            particles: mc.particles,
            inactive: mc.inactive,
            active: mc.active,
            seed: mc.seed,
            k: o.k,
            sigma: o.sigma,
            transport_s: o.transport_s,
        });
        let md = r.to_markdown(&now_rfc3339()).map_err(|e| e.to_string())?;
        let rp = out.join("recipe.md");
        std::fs::write(&rp, md).map_err(|e| e.to_string())?;
        println!("wrote {} and {}", spec.display(), rp.display());
        Ok(())
    }
}

#[cfg(all(test, not(any(target_os = "android", target_arch = "wasm32"))))]
mod tests {
    /// The assembled HTR-10 preset geometry matches the committed fixture:
    /// cell and tile counts, the Şeker ball count, the model's extent, and
    /// the fuel pebble the review gate zooms to. A change to the geometry
    /// builder shows up here (regenerate with `--headless-geometry` once the
    /// change is reviewed, with images).
    #[test]
    fn the_headless_geometry_matches_the_committed_fixture() {
        let got = super::headless::geometry_csv(&super::preset::htr10());
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/dhoby_ghaut_geometry.csv");
        let want = std::fs::read_to_string(&path).expect("fixture");
        assert_eq!(
            got.trim(),
            want.trim(),
            "regenerate {} only after reviewing the geometry",
            path.display()
        );
    }
}
