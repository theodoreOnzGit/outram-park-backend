//! # Dhoby Ghaut's guided high-fidelity workbench (gh:#561), first slice
//!
//! A native window that walks a guided build: pick a reactor type by
//! generation, Basic or Advanced, the HTGR core, then Steps 0–11 with every
//! value prefilled and cited, a literature pane beside the model, and a
//! recipe (kovan markdown) to save and load. ~~Steps 0–5 work for HTGR →
//! Basic → pebble bed (HTR-10); Steps 6–11 are shown with the issue that will
//! build each.~~ **UPDATED 2026-10-05:** all twelve steps work for HTGR →
//! Basic → pebble bed (HTR-10): Step 6 (branch; the reactivity map,
//! `step6.rs`), Step 7 (meshing, gh:#572), Step 8 (MGXS, gh:#573), Steps 9–10
//! (a SIMPLIFIED coupled case, gh:#574: r-z porous-core thermal-hydraulics;
//! ~~prescribed power shape, lumped feedback~~ since gh:#591 the power and k
//! are solved by diffusion on Steps 7-8, the prescribed shape an ablation)
//! and Step 11 (exports,
//! `step11.rs`). What each step does not model is listed in its panel.
//!
//! ```text
//! cargo run --release -p dhoby-ghaut --bin dhoby-ghaut
//! cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --recipe my_recipe.md
//! cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --headless-geometry
//! cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --scan-endf ~/ENDF-B-VIII.0
//! cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --render-review out_dir
//! cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --headless-multiphysics \
//!     --case mgxs_case_dir [--boundary face|cell|zero] [--n-cell 30 --th-cell 15] \
//!     [--rings 40 --axial 200] [--isothermal-k-only] [--out out_dir] \
//!     [--diagnostic-bed-sigma-scale 0.61]
//! cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --headless-multiphysics \
//!     --prescribed-power [--uniform-power] [--rings 5 --axial 40] [--out out_dir]
//! cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --headless-keff \
//!     [--particles 500 --inactive 10 --active 20 --threads 8] [--out out_dir]
//! cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --headless-map \
//!     [--synthetic | --sweep 300,600,900] [--order 1] [--basis sqrt|ln|linear] [--out out_dir]
//! cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --headless-mesh [--out case_dir]
//! cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --headless-mgxs [--out case_dir] \
//!     [--temperatures 300.15,600 --groups 2 --particles 2000 --inactive 10 --active 20 --threads 8]
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
//! `--headless-multiphysics` runs Steps 9-10 and prints the coupling console
//! and the TENTATIVE comparison with Gao & Shi (2002). By default the power
//! is solved (gh:#591): it rebuilds Step 7's meshes, reads Step 8's
//! `mgxs_set.toml` + `nuclearData` from `--case DIR` (written by
//! `--headless-mgxs --out DIR`), compares the isothermal diffusion `k` with
//! Step 8's Monte Carlo `k` at each state point, then runs the Picard
//! coupling (`spatial.rs`). `--prescribed-power` is the ablation with the
//! J0 x cosine shape and lumped feedback; its summary is pinned by
//! `tests/fixtures/dhoby_ghaut_multiphysics.csv`.
//!
//! `--headless-mesh` builds Step 7's three meshes and six maps, writes the
//! GeN-Foam case (`constant/<region>/polyMesh` with cellZones), the review
//! images (`images/`) and `meshes.csv`; a test runs it at a coarse size.
//! `--headless-mgxs` does that and then Step 8's state points, writing
//! `constant/neutroRegion/nuclearData` and `mgxs.csv` (minutes: no test).

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
mod gpu_view;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod literature;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod mp_headless;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod mp_preset;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod porous_core;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod spatial;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod spatial_draw;
mod meshing;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod mgxs_run;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod steps78;
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
    if args.iter().any(|a| a == "--headless-mesh") || args.iter().any(|a| a == "--headless-mgxs") {
        let r = recipe.unwrap_or_else(preset::htr10);
        let out = arg("--out").unwrap_or_else(|| "target/dhoby-ghaut_out/genfoam_case".into());
        let mut plan78 = dhoby_ghaut::workbench::mgxs::MgxsPlan::default();
        let num = |k: &str| arg(k).and_then(|v| v.parse::<usize>().ok());
        plan78.particles = num("--particles").unwrap_or(plan78.particles);
        plan78.inactive = num("--inactive").unwrap_or(plan78.inactive);
        plan78.active = num("--active").unwrap_or(plan78.active);
        plan78.threads = num("--threads").unwrap_or(plan78.threads);
        if let Some(t) = arg("--temperatures") {
            plan78.temperatures_k = t.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        }
        if let Some(g) = num("--groups") {
            use dhoby_ghaut::workbench::mgxs::GroupPreset;
            plan78.groups = match g {
                4 => GroupPreset::Four,
                8 => GroupPreset::Eight,
                _ => GroupPreset::Two,
            };
        }
        let mgxs = args.iter().any(|a| a == "--headless-mgxs");
        return headless::mesh_and_mgxs(&r, std::path::Path::new(&out), mgxs.then_some(plan78));
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
        // The prescribed shape is an explicit, labelled ablation (gh:#591).
        if args.iter().any(|a| a == "--prescribed-power" || a == "--uniform-power") {
            setup = mp_preset::prescribed(setup);
        }
        if args.iter().any(|a| a == "--uniform-power") {
            setup.neutronics.shape = dhoby_ghaut::workbench::multiphysics::PowerShape::Uniform;
        }
        let out = arg("--out").unwrap_or_else(|| "target/dhoby-ghaut_out".into());
        let out = std::path::Path::new(&out);
        if !setup.neutronics.shape.is_solved() {
            return mp_headless::headless(&r, &setup, out);
        }
        let case = arg("--case").ok_or(
            "the solved power shape needs Step 8's output: run `--headless-mgxs --out DIR` first \
             and pass `--case DIR`, or ask for the ablation with `--prescribed-power`",
        )?;
        let boundary = match arg("--boundary").as_deref() {
            None | Some("face") => spatial::NeutronBoundary::MarshakFace,
            Some("cell") => spatial::NeutronBoundary::MarshakCell,
            Some("zero") => spatial::NeutronBoundary::ZeroFlux,
            Some(o) => return Err(format!("--boundary {o}: face, cell or zero")),
        };
        let fnum = |k: &str| arg(k).and_then(|v| v.parse::<f64>().ok());
        let mut mgxs = mp_headless::load_mgxs(std::path::Path::new(&case))?;
        let built = headless::build_meshes(
            &r,
            &out.join("genfoam_case"),
            fnum("--n-cell"),
            fnum("--th-cell"),
        )?;
        if let Some(f) = fnum("--diagnostic-bed-sigma-scale") {
            let bed = mp_headless::diagnostic_bed_scale(&mut mgxs, &built.set, f, out)?;
            println!("DIAGNOSTIC: regions {bed:?} scaled by {f} (Sigma x f, D / f); NOT Step 8's data");
        }
        let only_k = args.iter().any(|a| a == "--isothermal-k-only");
        return mp_headless::headless_spatial(&r, &setup, built, mgxs, boundary, out, only_k);
    }
    let start_step: Option<u8> = arg("--step").and_then(|v| v.parse().ok());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1500.0, 950.0])
            .with_title("Dhoby Ghaut workbench"),
        ..Default::default()
    };
    // `--open-step N` opens the wizard at step N (for review screenshots);
    // `--auto-build` then builds Step 7's meshes as soon as the geometry is in.
    let open_step = arg("--open-step").and_then(|v| v.parse::<u8>().ok());
    let auto_build = args.iter().any(|a| a == "--auto-build");
    // `--load-mgxs DIR` takes Step 8's result from a `--headless-mgxs` case
    // folder; `--auto-run` then starts Step 10 as soon as Step 7's meshes are
    // in (both for review screenshots of the solved coupled run, gh:#591).
    let load_mgxs = match arg("--load-mgxs") {
        Some(d) => Some(mp_headless::load_mgxs(std::path::Path::new(&d))?),
        None => None,
    };
    let auto_run = args.iter().any(|a| a == "--auto-run");
    eframe::run_native(
        "Dhoby Ghaut workbench",
        options,
        Box::new(move |cc| {
            let mut a = app::App::new(cc, recipe, recipe_step9);
            // `--step N` / `--open-step N`: open the wizard at Step N, e.g.
            // for screenshots. Navigation only: the review gate still guards
            // every Monte Carlo run.
            if let Some(n) = start_step.or(open_step) {
                a.enter_wizard();
                if let Some(s) = dhoby_ghaut::workbench::steps::WizardStep::ALL.into_iter().find(|s| s.number() == Some(n)) {
                    a.step = s;
                }
            }
            a.s78.auto_build = auto_build;
            if let Some(m) = load_mgxs {
                a.s78.mgxs = Some(m.clone());
                a.s78.preloaded_mgxs = Some(m);
            }
            a.mp.auto_run = auto_run;
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

    /// [`assemble`] for the tests.
    #[cfg(test)]
    pub fn assemble_for_test(engine: &mut Engine, r: &Recipe) -> Option<AssemblyInfo> {
        assemble(engine, r)
    }

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

    /// Steps 7 (and, with a plan, 8) with no window: assemble, mesh, map,
    /// write the GeN-Foam case and the review images, print the summary CSV;
    /// then the MGXS state points, printing their CSV.
    /// Step 7's meshes (default plan, or the neutronics / TH cell sizes
    /// given, cm) built into `out`, for the headless Step 10.
    pub fn build_meshes(
        r: &Recipe,
        out: &Path,
        n_cell_cm: Option<f64>,
        th_cell_cm: Option<f64>,
    ) -> Result<crate::meshing::BuiltMeshes, String> {
        let mut engine = Engine::default();
        let a = assemble(&mut engine, r).ok_or("assembly failed")?;
        let d = crate::meshing::domain_from(&a);
        let mut plan = dhoby_ghaut::workbench::meshes::MeshPlan::default_for(&d);
        if let Some(h) = n_cell_cm {
            plan.roles[0].cell_size_cm = h;
        }
        if let Some(h) = th_cell_cm {
            plan.roles[1].cell_size_cm = h;
        }
        let t = std::time::Instant::now();
        crate::meshing::build(&d, &plan, out, &mut |p| {
            let crate::meshing::MeshProgress::Stage(s, f) = p;
            eprintln!("[{:5.1} s] {:3.0} % {s}", t.elapsed().as_secs_f64(), 100.0 * f);
        })
    }

    pub fn mesh_and_mgxs(
        r: &Recipe,
        out: &Path,
        mgxs: Option<dhoby_ghaut::workbench::mgxs::MgxsPlan>,
    ) -> Result<(), String> {
        let mut engine = Engine::default();
        let a = assemble(&mut engine, r).ok_or("assembly failed")?;
        let d = crate::meshing::domain_from(&a);
        let plan = dhoby_ghaut::workbench::meshes::MeshPlan::default_for(&d);
        let t = std::time::Instant::now();
        let built = crate::meshing::build(&d, &plan, out, &mut |p| {
            let crate::meshing::MeshProgress::Stage(s, f) = p;
            eprintln!("[{:5.1} s] {:3.0} % {s}", t.elapsed().as_secs_f64(), 100.0 * f);
        })?;
        let mut imgs = built.images.clone();
        imgs.push(("regions_rz".into(), crate::meshing::draw_region_map(&d, &plan.regions, 900)));
        for p in crate::meshing::write_images(&imgs, &out.join("images"))? {
            eprintln!("wrote {}", p.display());
        }
        let csv = crate::meshing::summary_csv(&built.set);
        std::fs::write(out.join("meshes.csv"), &csv).map_err(|e| e.to_string())?;
        println!("{csv}");
        let Some(mplan) = mgxs else { return Ok(()) };
        let core = engine_core(&mut engine, r).ok_or("no core")?;
        let nm = &built.meshes[dhoby_ghaut::workbench::meshes::MeshRole::Neutronics.index()];
        let job = crate::mgxs_run::MgxsJob {
            plan: mplan,
            core,
            mesh: std::sync::Arc::new(nm.with_unit(outram_blender::unstructured::LengthUnit::Centimetre)),
            cell_region: built.set.meshes[0].cell_region.clone(),
            regions: plan.regions.regions.clone(),
            out_dir: out.to_path_buf(),
            live: Default::default(),
        };
        let t = std::time::Instant::now();
        let set = crate::mgxs_run::run(&job, &mut |p| match p {
            crate::mgxs_run::MgxsProgress::Stage { state, what } => eprintln!("[{:6.1} s] state {}: {what}", t.elapsed().as_secs_f64(), state + 1),
            crate::mgxs_run::MgxsProgress::StateDone(s) => eprintln!(
                "[{:6.1} s] state {} K: k = {:.5} +/- {:.5}, data {:.1} s, transport {:.1} s",
                t.elapsed().as_secs_f64(),
                s.temperature_k,
                s.k,
                s.k_sigma,
                s.data_s,
                s.transport_s
            ),
            _ => {}
        })?;
        let p = crate::mgxs_run::write_csv(&set, out)?;
        println!("{}", crate::mgxs_run::csv(&set));
        for n in &set.notes {
            println!("note: {n}");
        }
        eprintln!("wrote {} and {}", p.display(), set.nuclear_data_path.clone().unwrap_or_default());
        Ok(())
    }

    /// The assembled core the engine holds (assembling again if needed).
    fn engine_core(engine: &mut Engine, r: &Recipe) -> Option<std::sync::Arc<nee_soon::htr10_rmc::core_model::AssembledCore>> {
        let mut core = None;
        let design = crate::design::plan(r).design;
        engine.handle(
            Req::Assemble { rings: r.pebble_bed.rings, layers: r.pebble_bed.layers, design },
            &mut |e| {
                if let Ev::Assembled(_, c) = e {
                    core = Some(c);
                }
            },
        );
        core
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

    /// Step 7 headless, at a coarse size (gh:#572). Builds the three meshes of
    /// the assembled HTR-10 preset and checks what the hand-off promises:
    ///
    /// - **Methodology.** Assemble the preset, derive the R-Z domain, mesh at
    ///   40 / 25 / 40 cm (neutronics / TH / structural), write the GeN-Foam
    ///   case into a temporary folder, read every `polyMesh` and `cellZones`
    ///   back with the GeN-Foam port's own reader, convert the structural
    ///   mesh to farrer-park, and check the six maps.
    /// - **Pass criteria.** Each mesh's volume within 3 % of the exact R-Z
    ///   volume (the inscribed 32-gon alone is −0.64 %); the structural mesh
    ///   is all `Tet4` and converts to farrer-park; the TH mesh carries
    ///   `inlet`, `outlet`, `cavity_wall` and `bed_wall` and holds only the
    ///   two in-core regions; the neutronics map covers the TH and structural
    ///   meshes to ≥ 99 %; a constant maps to itself on every covered cell;
    ///   a power density mapped neutronics → TH keeps its integral over the
    ///   overlap to 1e-4 (the TH cells' coverage by the neutronics mesh).
    /// - **Results (2026-10-05, default sizes, `--headless-mesh`).** 14 041 /
    ///   20 952 / 94 392 cells; volumes −1.24 % / −1.29 % / −0.91 % against
    ///   exact; N → TH covers 99.9999 %, N → S 99.70 %; 18 s on 16 threads.
    ///   The region volumes differ from exact by up to +90 % for the 10 cm
    ///   cold-gas plenum band on the 30 cm neutronics mesh (centroid
    ///   assignment, gh:#594); the bed is within 0.3 %.
    #[test]
    fn the_headless_meshes_hold_what_the_hand_off_promises() {
        use dhoby_ghaut::workbench::meshes::{MeshPlan, MeshRole};
        let r = super::preset::htr10();
        let mut engine = crate::engine::Engine::default();
        let a = super::headless::assemble_for_test(&mut engine, &r).expect("assembly");
        let d = crate::meshing::domain_from(&a);
        let mut plan = MeshPlan::default_for(&d);
        plan.roles[0].cell_size_cm = 40.0;
        plan.roles[1].cell_size_cm = 25.0;
        plan.roles[2].cell_size_cm = 40.0;
        let dir = std::env::temp_dir().join(format!("dhoby_ghaut_mesh_test_{}", std::process::id()));
        let b = crate::meshing::build(&d, &plan, &dir, &mut |_| {}).expect("meshes");
        for m in &b.set.meshes {
            let rel = m.volume_cm3 / m.exact_cm3 - 1.0;
            assert!(rel.abs() < 0.03, "{:?} volume off by {rel}", m.role);
            let pm = std::path::PathBuf::from(m.polymesh_dir.clone().expect("written"));
            let fv = outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh(&pm).expect("polyMesh reads back");
            assert_eq!(fv.n_cells, m.cells);
            let zones = outram_foam_appbuilder_lib::io::poly_mesh::read_cell_zones(&pm).expect("cellZones read back");
            let in_zones: usize = zones.iter().map(|z| z.cells.len()).sum();
            assert_eq!(in_zones, m.cells, "{:?}: every cell in exactly one zone", m.role);
        }
        let s = &b.meshes[MeshRole::Structural.index()];
        assert!((0..s.n_cells()).all(|c| s.cell_kind(c) == outram_blender::unstructured::ElementKind::Tet4));
        outram_blender::unstructured::convert::fem::to_fem_mesh(s).expect("structural -> farrer-park");
        let th = b.set.mesh(MeshRole::ThermalHydraulics).unwrap();
        for p in ["inlet", "outlet", "cavity_wall", "bed_wall"] {
            assert!(th.patches.iter().any(|q| q.0 == p && q.1 > 0), "TH patch {p}");
        }
        assert!(th.regions.iter().all(|r| r.cells == 0 || plan.regions.regions[r.region].in_core));
        for to in [MeshRole::ThermalHydraulics, MeshRole::Structural] {
            let m = b.set.mapping(MeshRole::Neutronics, to).unwrap();
            assert!(m.to_coverage > 0.99, "N -> {to:?} covers {}", m.to_coverage);
        }
        for m in &b.set.mappings {
            let n_from = b.meshes[m.from.index()].n_cells();
            let n_to = b.meshes[m.to.index()].n_cells();
            let mut out = vec![-1.0; n_to];
            m.map(&vec![3.5; n_from], &mut out);
            for (c, row) in m.weights.iter().enumerate() {
                if !row.is_empty() {
                    assert!((out[c] - 3.5).abs() < 1e-9, "{:?}->{:?} cell {c}", m.from, m.to);
                }
            }
        }
        // Power density neutronics -> TH: the integral over the overlap is kept.
        let nm = &b.meshes[0];
        let tm = &b.meshes[1];
        let o = outram_blender::unstructured::overlap::MeshOverlap::new(nm, tm);
        let q: Vec<f64> = (0..nm.n_cells()).map(|c| 1.0 + nm.cell_centre(c)[2].abs()).collect();
        let mut qt = vec![0.0; tm.n_cells()];
        o.map_onto_target(&q, &mut qt);
        let vt = outram_blender::unstructured::overlap::cell_volumes_cm3(tm);
        let lhs: f64 = (0..tm.n_cells()).map(|t| vt[t] * qt[t]).sum();
        // What the neutronics cells put into the TH domain: each source
        // value times its overlap volume with the TH mesh.
        let rhs: f64 = o.src_to_tgt.iter().enumerate().map(|(c, row)| q[c] * row.iter().map(|x| x.1).sum::<f64>()).sum();
        assert!((lhs - rhs).abs() < 1e-4 * rhs.abs(), "power on TH {lhs} vs deposited {rhs}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
