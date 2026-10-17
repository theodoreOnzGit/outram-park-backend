//! # Tutorial 3: HEU-SOL-THERM-009, a thermal system
//!
//! **Three nested shapes, water, and the six-factor formula** (beginner API
//! `outram_mc_libs::model`, epic gh #925; tutorial list and decisions on
//! #943). Tutorials 1 and 2 were Godiva: one sphere, all fast. This is its
//! thermal contrast: a sphere of highly enriched uranium oxyfluoride
//! solution in a thin aluminium tank, reflected by water. New in this
//! lesson:
//!
//! - **nested shells**, built inside out, **one `.containing` per
//!   statement** (maintainer, #943);
//! - **light water** as a moderator: `.bound_in(Moderator::LightWater)`
//!   attaches the H in H2O thermal scattering law to H-1;
//! - an **alloy preset with its citation** (`Material::aluminium_1100()`);
//! - **steam-table water against the spec's water** (the spec wins; the
//!   difference is printed as a lesson, not applied as a correction);
//! - **the six-factor formula**, which Godiva could not test, checked
//!   against k;
//! - **spectra per component**: the solution's thermal peak.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example tutorial_hst009                  # MCSettings::Auto
//! cargo run --release -p outram-mc-libs --example tutorial_hst009 -- --fast        # smoke test
//! cargo run --release -p outram-mc-libs --example tutorial_hst009 -- --seeds 8     # + 8 pooled seeds
//! cargo run --release -p outram-mc-libs --example tutorial_hst009 -- --seeds 8 --water  # + steam-table arm
//! ```
//!
//! (Tutorials refactor existing examples (#943). The HST-009 example
//! `examples/hst009_keff.rs` is a V&V case and keeps its role, as decided
//! on #943, and so do the other files that run this case
//! (`icsbep_five_route_keff.rs`, `h2o_*_vs_njoy_thermr.rs`). This file
//! repurposes their material, not their code: the case description and
//! the stated approximations of `hst009_keff.rs`, and the one transcription
//! of the specification, `outram_mc_libs::vv::hst009`, which every recorded
//! HST-009 number in this crate uses.)
//!
//! ## The case
//!
//! ICSBEP **HEU-SOL-THERM-009 case 1**, as the OpenMC model in
//! `mit-crpg/benchmarks` (`icsbep/heu-sol-therm-009/openmc/case-1`, Paul
//! Romano, 2013) gives it, transcribed in `vv::hst009`:
//!
//! | shape | outer radius \[cm\] | material |
//! |---|---|---|
//! | solution | 11.5177 | U-234/235/236/238, F-19, O, H; H in H2O |
//! | tank | 11.6764 | 1100 aluminium (Al-27, Si, Mn-55) |
//! | reflector | 35.0, vacuum outside | water; H in H2O |
//!
//! The model's two stated approximations, carried unchanged (`vv.rs`):
//! O-17 folded into O-16, and Cu and Zn left out of the tank.
//! Benchmark `k = 0.9990 ± 0.0043` (van der Marck, Nucl. Data Sheets 107
//! (2006) 3061, an open secondary source; the handbook itself is licence
//! restricted). Temperature 293.6 K, set on the geometry.
//!
//! ## What this checks (V&V methodology)
//!
//! Each check can fail; nothing is tuned (the run sizes are the presets'
//! or the seed count given on the command line).
//!
//! 1. **The model is the V&V model.** The assembled geometry has the same
//!    surfaces, regions and fills as `vv::hst009::geometry()`, and each
//!    transport material the same nuclides at the same atom densities as
//!    `vv::hst009::materials()`, to one ulp (bit identity is not reached:
//!    `AtomDensity` stores per m³ and the conversion from per barn·cm is
//!    not one-to-one on `f64`; 3 of the 14 come back one ulp off). Pinned
//!    without nuclear data by `tests/model_hst009_tutorial.rs` and by
//!    `model::model::tests::nested_shells_lower_like_the_hand_built_hst009`.
//! 2. **k against the record.** The crate's record for this model on the
//!    ENDF route (route 4: ENDF/B-VIII.0 tapes processed by the Rust NJOY
//!    port, LEAPR H in H2O, URR and DBRC on) is `k = 1.00072 ± 0.00021`
//!    (32 seeds at 5000 × [40 + 120], 2026-10-15, U-236 in the model;
//!    `verification_and_validation/icsbep/five_route_keff_2026_09_29.md`).
//!    A run must sit within 4σ of it (σ combining the run's own, the
//!    larger of naive and batch-means, and the record's). A regression gate
//!    sized off what a run resolves, not an accuracy claim.
//! 3. **The six-factor product against k.** The product
//!    `η f p ε P_FNL P_TNL` telescopes to `P / (A + L)` (track-length
//!    production over absorption plus leakage, per source neutron); `A + L`
//!    is `1 +` the net (n,xn) gain, so the product sits below k by that
//!    gain, and `P` itself is the track-length k. Both are compared with
//!    the power iteration's k; per seed with `--seeds` (the difference is
//!    pooled per seed, so the correlation between estimators from one run
//!    is accounted for).
//! 4. **Spec water against steam-table water** (`--water`): densities
//!    printed both ways, and Δk between the two models on the same seeds.
//! 5. **The geometry is drawn** from the assembled CSG (crate HARD RULE):
//!    x-y and x-z slices through the centre, and a zoom on the tank wall,
//!    written to `target/tutorials/hst009/` (gitignored).
//!
//! ## Predictions (written before the quoted runs, gh #943)
//!
//! 1. k within 2σ of the record.
//! 2. Product − k ≈ −(n,xn) gain ≈ −14 pcm (the Fast smoke run measured
//!    A + L − 1 = 1.37e-4), and P − k ≈ 0; both below what 16 seeds
//!    resolve, so the claim can only be "consistent with".
//! 3. Steam-table water at 293.6 K, 1 atm is 0.134 % denser than the spec's:
//!    a denser reflector, so Δk positive and ≲ +20 pcm, not resolvable at
//!    16 seeds; the claim is a bound.
//!
//! ## Results (2026-10-18, ENDF/B-VIII.0 from `reference-data/endf/`)
//!
//! `--seeds 16 --water --workers 4`. Hardware: Intel i9-13900K (32 logical
//! cores, shared with other jobs; this run pinned to 4 of them), 62.5 GiB
//! RAM, Linux, CPU only. Data processed on 4 threads; the Auto run's
//! transport single-threaded; the seeds 4 at a time.
//!
//! | check | result | verdict |
//! |---|---|---|
//! | 1. lowering | surfaces, cells, fills identical; 14 densities, 11 bit-identical, 3 one ulp off (U-236, F-19, Mn-55) | same model, not bit for bit (see above) |
//! | 2. Auto, seed 1 | `k = 1.00154 ± 0.00079` (batch means 0.00067); +254 ± 79 pcm from the benchmark | +1.00σ from the record |
//! | 2. 16 seeds, 10 000 × [90 + 250] | `k = 1.00093 ± 0.00011` (sd per run 45 pcm); **+193 ± 11 pcm** from the benchmark (0.9990 ± 0.0043: 0.45 benchmark σ) | **+22 pcm = +0.92σ from the record** |
//! | 3. six-factor product − k, paired per seed | **−7.9 ± 16.0 pcm** | consistent with the predicted −14 (0.4σ) and with 0 |
//! | 3. P − k, paired | +10.6 ± 24.2 pcm | consistent with 0 |
//! | 4. k(steam water) − k(spec water), paired | **−33 ± 29 pcm** (−1.2σ) | **not resolved**; 2σ interval −90 to +24 pcm. The predicted +0 to +20 sits inside it at its upper end |
//!
//! - **Six factors (Auto, seed 1):** η 2.0223, f 0.4523, p 0.8194,
//!   ε 1.3718, P_FNL 0.98557, P_TNL 0.98673; product 0.99978 ± 0.00087;
//!   net (n,xn) gain 1.08e-4 ± 0.07e-4. f is low because "thermal
//!   absorption" includes the reflector's water, where 70 % of that
//!   component's flux is thermal.
//! - **Spectra:** the solution is hard (5.7 % of its flux below 0.625 eV,
//!   flux per lethargy rising all the way to 1 eV, no Maxwellian maximum:
//!   H/U-235 ≈ 36, a strongly absorbing solution); the reflector is thermal
//!   (69.8 % below 0.625 eV, peak near 0.043 eV); the tank in between
//!   (20.3 %).
//! - **Water:** spec 0.99678 g/cm³ (H-1 and O-16 only); steam tables at
//!   293.6 K, 1 atm 0.99811 g/cm³ (+0.134 %, natural H and O: adds H-2
//!   1.04e-5 and O-17 1.26e-5 /b·cm), at 298.15 K 0.99705 g/cm³ (+0.027 %).
//!   The spec's water is close to 25 °C water. The spec wins.
//! - **Statistics:** the 16 seeds are under-dispersed against their own σ
//!   (χ²/dof 0.33, p = 0.993; seed sd 45 pcm against an internal 80); the
//!   steam arm on the same seeds was not (its sem 23 pcm, sd ≈ 92). Quoted
//!   σ's are the seed-to-seed ones. Recorded, not explained.
//! - **Wall time:** data 50 s (12 nuclides); Auto transport with
//!   diagnostics 347 s single-threaded; 16 seeds with diagnostics 2637 s
//!   on 4 workers; 16 steam seeds without diagnostics 1236 s on 4 workers.
//!   Fast smoke run: k = 1.00134 ± 0.00163 (+0.34σ from the record),
//!   transport with diagnostics 100 s.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use outram_mc_libs::geometry::plot::PlotBasis;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::SurfaceKind;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::model::ResolvedMaterial;
use outram_mc_libs::perf_report::HardwareInfo;
use outram_mc_libs::stats::ensemble::{consecutive_seeds, default_workers, run_seeds, EnsembleReport};
use outram_mc_libs::vv::hst009;
use uom::si::f64::{Length, Pressure, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::mass_density::gram_per_cubic_centimeter;
use uom::si::pressure::atmosphere;
use uom::si::thermodynamic_temperature::kelvin;

/// The crate's record for this model on the ENDF route (route 4), with
/// U-236: `k` and its standard error over 32 seeds at 5000 × [40 + 120]
/// (`five_route_keff_2026_09_29.md`, "HST-009: U-236 added", 2026-10-15).
const RECORDED_K: (f64, f64) = (1.000717, 0.000208);
/// Gate width in combined σ.
const K_SIGMA: f64 = 4.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let fast = args.iter().any(|a| a == "--fast");
    let water_arm = args.iter().any(|a| a == "--water");
    let arg = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let n_seeds: usize = arg("--seeds").and_then(|s| s.parse().ok()).unwrap_or(0);
    let data = match arg("--endf") {
        Some(dir) => NuclearData::endf_folder(&PathBuf::from(dir)),
        None => NuclearData::workspace_reference(),
    };
    let data = match data {
        Ok(d) => d,
        Err(e) => {
            println!("No ENDF tapes: {e}. Clone with the reference data, or pass --endf <folder>.");
            return Ok(());
        }
    };

    // ── 1. The model and its three components ───────────────────────────────
    let mut model = Model::new(data);
    let solution_c = model.component("Fuel solution");
    let tank_c = model.component("Tank");
    let water_c = model.component("Reflector");

    // ── 2. Materials: the specification's own atom densities ────────────────
    // Read from `vv::hst009`, the one transcription every recorded HST-009
    // number uses. `spec(material, nuclide)` is that table's entry.
    let spec = |mat: usize, name: &str| {
        atoms_per_barn_cm(
            hst009::MATERIALS[mat]
                .1
                .iter()
                .find(|(n, _)| *n == name)
                .map_or(0.0, |(_, d)| *d),
        )
    };
    let solution = Material::from_atom_densities([
        (U234, spec(hst009::MAT_SOLUTION, "U234")),
        (U235, spec(hst009::MAT_SOLUTION, "U235")),
        (U236, spec(hst009::MAT_SOLUTION, "U236")),
        (U238, spec(hst009::MAT_SOLUTION, "U238")),
        (F19, spec(hst009::MAT_SOLUTION, "F19")),
        (O16, spec(hst009::MAT_SOLUTION, "O16")), // O-17 folded in (vv.rs)
        (H1, spec(hst009::MAT_SOLUTION, "H1")),
    ])
    .bound_in(Moderator::LightWater)
    .named("uranium oxyfluoride solution");
    // The tank: a preset, with its citation in its doc (the same model's
    // 1100 aluminium, Cu and Zn left out). A preset is a starting point;
    // your own alloy goes in with `Material::from_fractions(Basis::Weight, ..)`.
    let tank = Material::aluminium_1100();
    let water = Material::from_atom_densities([
        (H1, spec(hst009::MAT_WATER, "H1")),
        (O16, spec(hst009::MAT_WATER, "O16")), // O-17 folded in (vv.rs)
    ])
    .bound_in(Moderator::LightWater)
    .named("water reflector");

    // ── 2b. Aside: the same water from the steam tables ─────────────────────
    // Steam-table water is PURE water: natural H and O at the IF97 density.
    // The spec's water is what the experimenters' water was modelled as.
    // The spec wins for the benchmark; the difference is a lesson.
    let t_model = ThermodynamicTemperature::new::<kelvin>(hst009::TEMPERATURE_K);
    let one_atm = Pressure::new::<atmosphere>(1.0);
    let steam_water = Material::water(t_model, one_atm)?.named("steam-table water");
    let masses = model.data().atomic_masses();
    let water_r = water.resolve(&masses)?;
    let steam_r = steam_water.resolve(&masses)?;
    let rho = |r: &ResolvedMaterial| r.report.total_density.get::<gram_per_cubic_centimeter>();
    let at_25c = Material::water(ThermodynamicTemperature::new::<kelvin>(298.15), one_atm)?
        .resolve(&masses)?;
    println!(
        "Water two ways (masses: {}):\n{}",
        masses.source(),
        water_r.compare_number_densities(&steam_r)
    );
    println!(
        "  density: spec {:.5} g/cm3; steam tables at {} K, 1 atm {:.5} g/cm3 ({:+.3} %); at 298.15 K {:.5} g/cm3 ({:+.3} %)\n",
        rho(&water_r),
        hst009::TEMPERATURE_K,
        rho(&steam_r),
        100.0 * (rho(&steam_r) / rho(&water_r) - 1.0),
        rho(&at_25c),
        100.0 * (rho(&at_25c) / rho(&water_r) - 1.0),
    );

    // ── 3. Geometry: inside out, one `.containing` per statement ────────────
    let cm = Length::new::<centimeter>;
    let solution_ball = Sphere::radius(cm(hst009::R_SOLUTION))
        .filled_with(solution)
        .in_component(solution_c);
    let tank_ball = Sphere::radius(cm(hst009::R_TANK))
        .filled_with(tank.clone())
        .in_component(tank_c)
        .containing(solution_ball.clone());
    let reflector = Sphere::radius(cm(hst009::R_REFLECTOR))
        .filled_with(water)
        .in_component(water_c)
        .containing(tank_ball);
    model.add(reflector);

    // ── 4. The temperature, set on the geometry ─────────────────────────────
    model.set_temperature(Temperature::Uniform(t_model));

    // ── 5. Check: this is the V&V model, exactly ───────────────────────────
    let assembled = model.assemble()?;
    let vv_geom = hst009::geometry();
    let same_surfaces = assembled.geometry.surfaces.len() == vv_geom.surfaces.len()
        && assembled
            .geometry
            .surfaces
            .iter()
            .zip(&vv_geom.surfaces)
            .all(|(a, b)| match (a, b) {
                (SurfaceKind::Sphere(p), SurfaceKind::Sphere(q)) => {
                    (p.x0, p.y0, p.z0, p.r, p.bc) == (q.x0, q.y0, q.z0, q.r, q.bc)
                }
                _ => false,
            });
    let same_cells = assembled.geometry.cells.len() == vv_geom.cells.len()
        && assembled
            .geometry
            .cells
            .iter()
            .zip(&vv_geom.cells)
            .all(|(a, b)| a.region == b.region && a.fill == b.fill);
    // Transport materials against the V&V model's, nuclide by nuclide, in
    // ulp (units in the last place of an f64).
    let vv_mats = hst009::materials();
    let mut worst_ulp = 0_i64;
    let mut pairs = 0;
    let mut same_atoms = assembled.materials.len() == vv_mats.len();
    for (ours, theirs) in assembled.materials.iter().zip(&vv_mats) {
        same_atoms &= ours.components.len() == theirs.components.len();
        for c in &theirs.components {
            let name = hst009::NUCLIDES[c.nuclide_idx].0;
            match ours
                .components
                .iter()
                .find(|o| assembled.nuclides[o.nuclide_idx].isotope.name() == name)
            {
                Some(o) => {
                    let u =
                        (o.atom_density.to_bits() as i64 - c.atom_density.to_bits() as i64).abs();
                    worst_ulp = worst_ulp.max(u);
                    pairs += 1;
                }
                None => same_atoms = false,
            }
        }
    }
    same_atoms &= worst_ulp <= 1;
    println!(
        "The model is vv::hst009: surfaces {same_surfaces}, cells {same_cells}, {pairs} atom densities \
         {same_atoms} (largest difference {worst_ulp} ulp: AtomDensity stores per m3, so a number \
         given per barn-cm can come back one ulp off)"
    );
    assert!(
        same_surfaces && same_cells && same_atoms,
        "the tutorial's model must lower to exactly the V&V model"
    );

    // ── 6. Look at it before running it (crate HARD RULE) ───────────────────
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/tutorials/hst009");
    std::fs::create_dir_all(&out)?;
    for (basis, name) in [
        (PlotBasis::Xy, "hst009_xy.png"),
        (PlotBasis::Xz, "hst009_xz.png"),
    ] {
        let title = format!(
            "HEU-SOL-THERM-009, {} slice through the centre",
            basis.name()
        );
        assembled
            .draw_slice(basis, 800, &title)
            .write_png(out.join(name))?;
        println!("drew {}", out.join(name).display());
    }
    // The tank wall is 1.587 mm: invisible at full scale, so zoom on it
    // (a 1 cm window centred on the wall, on the +x axis).
    let wall_mid = 0.5 * (hst009::R_SOLUTION + hst009::R_TANK);
    let zoom = assembled.draw_window(
        PlotBasis::Xy,
        Position::new(wall_mid, 0.0, 0.0),
        [1.0, 1.0],
        800,
        "HEU-SOL-THERM-009, the tank wall (1 cm window, x-y)",
    );
    zoom.write_png(out.join("hst009_tank_wall.png"))?;
    println!("drew {}", out.join("hst009_tank_wall.png").display());

    // ── 7. Run, with the diagnostics that need a thermal system ────────────
    // The default set (six factors among them) and one spectrum per
    // component: the solution, the tank and the reflector.
    let diagnostics =
        Diagnostics::default().with_spectrum_per_component(&model, EnergyGrid::default_spectrum());
    let preset = if fast {
        MCSettings::Fast
    } else {
        MCSettings::Auto
    };
    let t = Instant::now();
    let nuclides = assembled.load_nuclides(model.data())?;
    let data_s = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let run = assembled.keff_with(
        &nuclides,
        &KeffSettings::new(preset, Some(diagnostics.clone())),
    )?;
    let run_s = t.elapsed().as_secs_f64();
    // (`model.keff(..)` does the two steps in one call.)
    println!("\n{run}");
    println!(
        "  wall time: data {data_s:.1} s ({} nuclides, processed in parallel), transport with diagnostics {run_s:.1} s single-threaded\n  hardware: {}",
        nuclides.len(),
        HardwareInfo::detect().headline()
    );

    // ── 8. Read the results ────────────────────────────────────────────────
    let k = run.keff();
    let sigma = k.std.max(run.provenance.sigma_batch_means.unwrap_or(0.0));
    let sf = run.six_factors()?;
    println!(
        "\nSix factors ({} eV / {} eV):",
        sf.edges_ev.0, sf.edges_ev.1
    );
    for (name, e) in [
        ("eta", sf.eta),
        ("f", sf.f),
        ("p", sf.p),
        ("epsilon", sf.epsilon),
        ("P_FNL", sf.p_fnl),
        ("P_TNL", sf.p_tnl),
        ("product", sf.k_from_factors),
        ("P (track-length k)", sf.k_tracklength),
    ] {
        println!("  {name:<20} {:.5} ± {:.5}", e.mean, e.std);
    }
    let bal = run.neutron_balance()?;
    println!(
        "  A + L - 1 (net (n,xn) gain per source neutron): {:+.2e} ± {:.1e}",
        bal.ends_per_source.mean - 1.0,
        bal.ends_per_source.std
    );
    for (c, label) in [
        (solution_c, "solution"),
        (tank_c, "tank"),
        (water_c, "reflector"),
    ] {
        let s = run.spectrum(Region::Component(c))?;
        let (i, _) = s
            .per_lethargy
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.mean.total_cmp(&b.1.mean))
            .unwrap_or((0, &s.per_lethargy[0]));
        let below: f64 = s
            .flux
            .iter()
            .enumerate()
            .filter(|(j, _)| s.edges_ev[j + 1] <= sf.edges_ev.0 * (1.0 + 1e-9))
            .map(|(_, f)| f.mean)
            .sum();
        // The largest flux per lethargy below 1 eV: a Maxwellian peak if
        // there is one (in the reflector), or just the top of the
        // slowing-down tail (in the strongly absorbing solution).
        let thermal = s
            .per_lethargy
            .iter()
            .enumerate()
            .filter(|(j, _)| s.edges_ev[j + 1] <= 1.0)
            .max_by(|a, b| a.1.mean.total_cmp(&b.1.mean))
            .map_or(0.0, |(j, _)| (s.edges_ev[j] * s.edges_ev[j + 1]).sqrt());
        println!(
            "  spectrum in the {label:<9}: peak flux per lethargy near {:.3e} eV, largest below 1 eV near {thermal:.3e} eV; {:.1} % of its flux below {} eV",
            (s.edges_ev[i] * s.edges_ev[i + 1]).sqrt(),
            100.0 * below / s.total_in_grid.mean,
            sf.edges_ev.0
        );
    }
    let report = run.diagnostics_report_titled("HEU-SOL-THERM-009 case 1");
    report.write_zip(out.join("hst009.zip"))?;
    println!(
        "wrote {} ({} files)",
        out.join("hst009.zip").display(),
        report.files().len()
    );

    // ── 9. The checks (V&V) ────────────────────────────────────────────────
    let z = |a: f64, sa: f64, b: f64, sb: f64| (a - b) / (sa * sa + sb * sb).sqrt();
    let to_bench = (k.mean - hst009::BENCHMARK_K) * 1e5;
    let to_record = z(k.mean, sigma, RECORDED_K.0, RECORDED_K.1);
    println!(
        "\nk_eff = {k} (batch means σ {:.5}); {to_bench:+.0} ± {:.0} pcm from the benchmark {} ± {}",
        run.provenance.sigma_batch_means.unwrap_or(f64::NAN),
        sigma * 1e5,
        hst009::BENCHMARK_K,
        hst009::BENCHMARK_SIGMA
    );
    println!(
        "  against the record {:.5} ± {:.5}: {:+.0} pcm = {to_record:+.2} sigma (gate {K_SIGMA})",
        RECORDED_K.0,
        RECORDED_K.1,
        (k.mean - RECORDED_K.0) * 1e5
    );
    println!(
        "  six-factor product - k = {:+.0} ± {:.0} pcm ({:+.2} sigma, uncorrelated combination: conservative)",
        (sf.k_from_factors.mean - k.mean) * 1e5,
        (sf.k_from_factors.std.powi(2) + k.std.powi(2)).sqrt() * 1e5,
        z(sf.k_from_factors.mean, sf.k_from_factors.std, k.mean, k.std)
    );
    println!(
        "  P - k = {:+.0} ± {:.0} pcm ({:+.2} sigma)",
        (sf.k_tracklength.mean - k.mean) * 1e5,
        (sf.k_tracklength.std.powi(2) + k.std.powi(2)).sqrt() * 1e5,
        z(sf.k_tracklength.mean, sf.k_tracklength.std, k.mean, k.std)
    );
    assert!(
        sf.undefined_because.is_none(),
        "HST-009 is thermal: the six factors must be defined"
    );
    assert!(
        to_record.abs() <= K_SIGMA,
        "HST-009 through the beginner API is {to_record:+.2} sigma from the record. \
         Find out what changed; do not widen this gate."
    );

    // ── 10. Optional: pooled seeds, and the steam-table arm on the same seeds
    if n_seeds > 1 {
        let p = &run.provenance;
        let size = |seed| {
            MCSettings::Custom(CustomRun {
                particles: p.active_particles,
                inactive: p.inactive_cycles,
                active: p.active_cycles,
                seed,
            })
        };
        let workers = arg("--workers")
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(default_workers);
        let seeds = consecutive_seeds(n_seeds);
        // Per seed: k, the product - k and P - k, from one run each.
        let extra = Mutex::new(vec![(f64::NAN, f64::NAN); n_seeds]);
        let t = Instant::now();
        let ks = run_seeds(&seeds, workers, |seed| {
            let r = assembled
                .keff_with(
                    &nuclides,
                    &KeffSettings::new(size(seed), Some(diagnostics.clone())),
                )
                .expect("a seed run");
            let sf = r.six_factors().expect("six factors requested");
            let k = r.keff();
            if let Ok(mut e) = extra.lock() {
                e[(seed - 1) as usize] = (
                    (sf.k_from_factors.mean - k.mean) * 1e5,
                    (sf.k_tracklength.mean - k.mean) * 1e5,
                );
            }
            (k.mean, k.std)
        });
        let pool_s = t.elapsed().as_secs_f64();
        let extra = extra.into_inner().unwrap_or_default();
        let pooled = |v: Vec<f64>| {
            let n = v.len() as f64;
            let m = v.iter().sum::<f64>() / n;
            let sd = (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
            (m, sd / n.sqrt())
        };
        let (dprod, dprod_se) = pooled(extra.iter().map(|e| e.0).collect());
        let (dp, dp_se) = pooled(extra.iter().map(|e| e.1).collect());
        let rep = EnsembleReport::from_runs(ks.clone());
        println!(
            "\n{n_seeds} seeds x Custom {{ {} per cycle, {} inactive, {} active }} ({pool_s:.0} s on {workers} workers):",
            p.active_particles, p.inactive_cycles, p.active_cycles
        );
        println!(
            "  k = {:.5} ± {:.5} (sd per run {:.0} pcm); {:+.0} ± {:.0} pcm from the benchmark; against the record {:+.0} pcm = {:+.2} sigma",
            rep.mean,
            rep.sem,
            rep.sd * 1e5,
            (rep.mean - hst009::BENCHMARK_K) * 1e5,
            rep.sem * 1e5,
            (rep.mean - RECORDED_K.0) * 1e5,
            z(rep.mean, rep.sem, RECORDED_K.0, RECORDED_K.1)
        );
        print!("{}", rep.consistency_summary("k"));
        println!(
            "  six-factor product - k = {dprod:+.1} ± {dprod_se:.1} pcm; P - k = {dp:+.1} ± {dp_se:.1} pcm (paired per seed)"
        );
        assert!(
            z(rep.mean, rep.sem, RECORDED_K.0, RECORDED_K.1).abs() <= K_SIGMA,
            "the pooled HST-009 moved more than {K_SIGMA} sigma from the record"
        );

        if water_arm {
            // The same model with steam-table water in the reflector.
            let mut steam_model = Model::new(model.data().clone());
            let (s_c, t_c, w_c) = (
                steam_model.component("Fuel solution"),
                steam_model.component("Tank"),
                steam_model.component("Reflector"),
            );
            let solution_ball = Sphere::radius(cm(hst009::R_SOLUTION))
                .filled_with(solution_ball.material().clone())
                .in_component(s_c);
            let tank_ball = Sphere::radius(cm(hst009::R_TANK))
                .filled_with(tank)
                .in_component(t_c)
                .containing(solution_ball);
            let reflector = Sphere::radius(cm(hst009::R_REFLECTOR))
                .filled_with(steam_water)
                .in_component(w_c)
                .containing(tank_ball);
            steam_model.add(reflector);
            steam_model.set_temperature(Temperature::Uniform(t_model));
            let steam_asm = steam_model.assemble()?;
            let steam_nuc = steam_asm.load_nuclides(steam_model.data())?;
            let t = Instant::now();
            let ks2 = run_seeds(&seeds, workers, |seed| {
                let r = steam_asm
                    .keff_with(&steam_nuc, &KeffSettings::new(size(seed), None))
                    .expect("a seed run");
                (r.keff().mean, r.keff().std)
            });
            let steam_s = t.elapsed().as_secs_f64();
            let rep2 = EnsembleReport::from_runs(ks2.clone());
            let (dk, dk_se) = pooled(
                ks.iter()
                    .zip(&ks2)
                    .map(|(a, b)| (b.value - a.value) * 1e5)
                    .collect(),
            );
            println!(
                "\nSteam-table water in the reflector, same seeds ({steam_s:.0} s): k = {:.5} ± {:.5}",
                rep2.mean, rep2.sem
            );
            println!(
                "  k(steam) - k(spec) = {dk:+.1} ± {dk_se:.1} pcm (paired per seed), {:.2} sigma",
                dk / dk_se
            );
        }
    }
    Ok(())
}
