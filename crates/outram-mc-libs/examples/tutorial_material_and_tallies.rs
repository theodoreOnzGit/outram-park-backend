//! # Tutorial 2: Godiva, diagnostics
//!
//! **What a run can tell you besides k** (beginner API `outram_mc_libs::model`,
//! epic gh #925; diagnostics #940, report #941; tutorial list on #943). The
//! model is tutorial 1's bare HEU sphere; the new line is one argument:
//!
//! ```rust,ignore
//! let diagnostics = Diagnostics::reactor_physics()
//!     .with_spectrum_per_component(&godiva, EnergyGrid::default_spectrum());
//! let run = godiva.keff(KeffSettings::new(MCSettings::Auto, Some(diagnostics)))?;
//! run.diagnostics_report_titled("Godiva").write_zip("target/tutorials/godiva_diagnostics/godiva.zip")?;
//! ```
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example tutorial_material_and_tallies            # Auto
//! cargo run --release -p outram-mc-libs --example tutorial_material_and_tallies -- --fast  # smoke test
//! ```
//!
//! (This file used to be the low-level "materials, k-eff and the tally API"
//! tutorial, whose own text said k-eff runs could not carry tallies; that
//! is no longer true, and the file was repurposed as tutorial 2 on
//! 2026-10-17, as the maintainer asked on #943: tutorials refactor existing
//! examples.)
//!
//! ## What the lesson shows
//!
//! - **The run is unchanged by asking.** Diagnostics only score; they draw
//!   no random number. So k here is tutorial 1's k, bit for bit, at the same
//!   preset and seed.
//! - **Godiva is fast, so the six-factor formula does not apply.** Nothing
//!   is absorbed below 0.625 eV, every factor divides by a thermal rate, and
//!   the run says "undefined" with the reason instead of printing zeros.
//!   (Tutorial 3, HST-009, is the thermal contrast.) What does apply: the
//!   three-group absorption, production and leakage, and `P / (A + L)`.
//! - **Where neutrons go**: about 55 % leak out (Godiva's well-known
//!   leakiness), and the rest are absorbed, mostly in fission.
//! - **EALF**, the energy of the average lethargy causing fission, a
//!   spectral index criticality safety uses (NUREG-1520 Rev. 2, §5).
//! - **β_eff and Λ by IFP** (adjoint-weighted, `physics::ifp`, ported from
//!   OpenMC).
//! - **Maps**: the fission rate and the flux on a 15 × 15 × 15 mesh.
//! - **The report**: `godiva.zip` with `report.md`, CSVs and PNG plots.
//!
//! ## What this checks (V&V methodology)
//!
//! Each a check that can fail, none tuned:
//!
//! 1. **k against the record** (as tutorial 1): within 4σ of the recorded
//!    `−6 ± 5 pcm` (`examples/godiva_keff_ensemble.rs`, 1024 seeds).
//! 2. **Two estimators of one k agree**: the track-length ν-fission per
//!    source neutron `P` against the power iteration's k, within 4σ.
//! 3. **The balance closes**: track-length absorption + leakage against the
//!    analog count of how neutrons ended, `0` within 4σ.
//! 4. **The spectrum integrates to the total flux**: the whole-model
//!    spectrum summed over all bins equals the balance tally's total flux
//!    to 1e-12 (two tallies, the same track lengths).
//! 5. **IFP against the k-ratio route** (`verification_and_validation/
//!    kinetics/godiva_k_ratio_2026_09_22.md`: β_eff `688 ± 77 pcm`, Λ
//!    `6.21 ns`, two-nuclide Godiva, 24 seeds): IFP's β_eff within 4σ, and
//!    Λ reported beside it. The two are different definitions (adjoint- and
//!    un-weighted), so this is a consistency check, not a validation.
//!
//! ## Results (2026-10-17, ENDF/B-VIII.0 from `reference-data/endf/`)
//!
//! Hardware: Intel i9-13900K (16 logical cores, shared), 62.5 GiB RAM,
//! Linux, CPU only; transport single-threaded, data on 4 threads. Full
//! record, with the OpenMC comparison: `verification_and_validation/
//! diagnostics/README.md`.
//!
//! - **Auto, seed 1:** `k = 1.00099 ± 0.00089`, tutorial 1's bits exactly;
//!   +1.17σ from the record. `P` −1.41σ from k; balance closure −2.12σ
//!   (+0.94σ on an independent run: not a resolved bias); spectrum total /
//!   balance flux − 1 = 2e-16; β_eff 647.6 ± 23.7 pcm (−0.50σ from the
//!   k-ratio route), Λ 5.609 ± 0.026 ns; EALF 8.292e5 ± 850 eV; leakage
//!   0.5708 of every neutron that ends. OpenMC (same model, openmc.org data):
//!   β_eff 643.7 ± 9.5 pcm, Λ 5.616 ± 0.011 ns, EALF 8.291e5 eV.
//! - **Wall time:** data 29.5 s; transport 37.7 s with diagnostics against
//!   10.9 s without (tutorial 1, same machine): 3.5×.
//! - **`--fast`:** `k = 1.00369 ± 0.00183` (a smoke test, as it says).

use std::path::PathBuf;
use std::time::Instant;

use outram_mc_libs::geometry::plot::PlotBasis;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::perf_report::HardwareInfo;
use outram_mc_libs::vv::godiva;
use uom::si::f64::{Length, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::thermodynamic_temperature::kelvin;

/// The recorded Godiva offset from HEU-MET-FAST-001 \[pcm\]
/// (`examples/godiva_keff_ensemble.rs`, 1024 seeds, 2026-10-05) and its
/// standard error.
const RECORDED_PCM: f64 = -6.0;
const RECORDED_SEM_PCM: f64 = 5.0;
/// β_eff by the k-ratio route \[pcm\] and its σ, Λ \[s\]
/// (`verification_and_validation/kinetics/godiva_k_ratio_2026_09_22.md`).
const K_RATIO_BETA_PCM: (f64, f64) = (688.0, 77.0);
const K_RATIO_LAMBDA_S: f64 = 6.212e-9;
/// Gate width in combined σ.
const K_SIGMA: f64 = 4.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fast = std::env::args().any(|a| a == "--fast");
    let data = match NuclearData::workspace_reference() {
        Ok(d) => d,
        Err(e) => {
            println!("No ENDF tapes: {e}. Clone with the reference data.");
            return Ok(());
        }
    };

    // ── 1. Tutorial 1's model ───────────────────────────────────────────────
    let mut godiva = Model::new(data);
    let core = godiva.component("Core");
    let spec = |name: &str| {
        godiva::NUCLIDES
            .iter()
            .find(|n| n.0 == name)
            .map_or(0.0, |n| n.2)
    };
    let heu = Material::from_atom_densities([
        (U234, atoms_per_barn_cm(spec("U234"))),
        (U235, atoms_per_barn_cm(spec("U235"))),
        (U238, atoms_per_barn_cm(spec("U238"))),
    ])
    .named("Godiva HEU");
    godiva.add(
        Sphere::radius(Length::new::<centimeter>(godiva::RADIUS_CM))
            .filled_with(heu)
            .in_component(core),
    );
    godiva.set_temperature(Temperature::Uniform(
        ThermodynamicTemperature::new::<kelvin>(godiva::TEMPERATURE_K),
    ));

    // ── 2. Look at it first (the same sphere as tutorial 1) ─────────────────
    let out =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/tutorials/godiva_diagnostics");
    std::fs::create_dir_all(&out)?;
    let assembled = godiva.assemble()?;
    for (basis, name) in [
        (PlotBasis::Xy, "godiva_xy.png"),
        (PlotBasis::Xz, "godiva_xz.png"),
    ] {
        let img = assembled.draw_slice(basis, 600, &format!("Godiva, {} slice", basis.name()));
        img.write_png(out.join(name))?;
        println!("drew {}", out.join(name).display());
    }

    // ── 3. Ask for the diagnostics ──────────────────────────────────────────
    // The reactor-physics set: the "wanted by everyone" default (run health,
    // neutron balance, EALF, leakage, six factors, whole-model spectrum)
    // plus kinetics by IFP and 15^3 maps; and one spectrum per component.
    let diagnostics = Diagnostics::reactor_physics()
        .with_spectrum_per_component(&godiva, EnergyGrid::default_spectrum());
    let preset = if fast {
        MCSettings::Fast
    } else {
        MCSettings::Auto
    };

    // ── 4. Run (data, then transport, timed apart) ──────────────────────────
    let t = Instant::now();
    let nuclides = assembled.load_nuclides(godiva.data())?;
    let data_s = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let run = assembled.keff_with(&nuclides, &KeffSettings::new(preset, Some(diagnostics)))?;
    let run_s = t.elapsed().as_secs_f64();
    // (`godiva.keff(..)` does both in one call.)
    println!("\n{run}");
    println!(
        "  wall time: data {data_s:.1} s, transport with diagnostics {run_s:.1} s single-threaded\n  hardware: {}",
        HardwareInfo::detect().headline()
    );

    // ── 5. Read the results ─────────────────────────────────────────────────
    let k = run.keff();
    let sf = run.six_factors()?;
    println!(
        "\nSix factors: {}",
        sf.undefined_because.as_deref().unwrap_or("defined")
    );
    let names = ["thermal", "resonance", "fast"];
    for g in 0..3 {
        println!(
            "  {:<9} absorption {:.5} ± {:.5}  nu-fission {:.5} ± {:.5}  leakage {:.5} ± {:.5}",
            names[g],
            sf.absorption[g].mean,
            sf.absorption[g].std,
            sf.production[g].mean,
            sf.production[g].std,
            sf.leakage[g].mean,
            sf.leakage[g].std
        );
    }
    println!(
        "  P (track-length k) = {:.5} ± {:.5};  P/(A+L) = {:.5} ± {:.5}",
        sf.k_tracklength.mean,
        sf.k_tracklength.std,
        sf.production_over_losses.mean,
        sf.production_over_losses.std
    );
    let bal = run.neutron_balance()?;
    println!(
        "\nBalance per source neutron: fission {:.4}, capture {:.4}, leakage {:.4}; ends {:.5}; closure {:+.1e} ± {:.1e}",
        bal.total.fission.mean,
        bal.total.capture.mean,
        bal.leakage.mean,
        bal.ends_per_source.mean,
        bal.closure.mean,
        bal.closure.std
    );
    let leak = run.leakage()?;
    println!(
        "Leakage: {:.4} ± {:.4} of every neutron that ends",
        leak.fraction_of_ends.mean, leak.fraction_of_ends.std
    );
    let ealf = run.ealf()?;
    println!(
        "EALF: {:.4e} ± {:.1e} eV (binning bound {:.2} %)",
        ealf.ealf_ev.mean,
        ealf.ealf_ev.std,
        100.0 * (ealf.discretisation_bound_ln.exp() - 1.0)
    );
    let kin = run.kinetics()?;
    println!(
        "IFP ({} generations, {} scoring cycles): beta_eff = {:.1} ± {:.1} pcm, Lambda = {:.3} ± {:.3} ns",
        kin.ifp_generations,
        kin.scoring_cycles,
        kin.beta_eff.mean * 1e5,
        kin.beta_eff.std * 1e5,
        kin.generation_time_s.mean * 1e9,
        kin.generation_time_s.std * 1e9
    );
    let whole = run.spectrum(Region::WholeModel)?;
    let peak = whole
        .per_lethargy
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.mean.total_cmp(&b.1.mean))
        .map_or(0.0, |(i, _)| {
            (whole.edges_ev[i] * whole.edges_ev[i + 1]).sqrt()
        });
    println!("Spectrum: peak flux per lethargy near {peak:.3e} eV (a fission-spectrum system)");
    let core_phi = run.spectrum(Region::Component(core))?;
    println!("  the Core component's spectrum: {} bins (Godiva has one component, so it is the whole model's)", core_phi.flux.len());
    let fm = run.fission_map()?;
    println!(
        "Maps: {:?} cells of {:.3} cm",
        fm.mesh.dimension, fm.cell_cm[0]
    );

    // ── 6. The report ───────────────────────────────────────────────────────
    let report = run.diagnostics_report_titled("Godiva");
    let zip = out.join("godiva.zip");
    report.write_zip(&zip)?;
    println!(
        "\nwrote {} ({} files):",
        zip.display(),
        report.files().len()
    );
    for f in report.files() {
        println!("  {:<44} {:>9} bytes", f.path, f.bytes.len());
    }
    report.write_unzipped_to_dir(out.join("unzipped"))?;

    // ── 7. The checks (V&V) ─────────────────────────────────────────────────
    let sigma = k.std.max(run.provenance.sigma_batch_means.unwrap_or(0.0));
    let pcm = (k.mean - godiva::BENCHMARK_K) * 1e5;
    let combined = ((sigma * 1e5).powi(2) + RECORDED_SEM_PCM.powi(2)).sqrt();
    let z = |a: f64, sa: f64, b: f64, sb: f64| (a - b) / (sa * sa + sb * sb).sqrt();
    let checks = [
        ("k vs record [sigma]", (pcm - RECORDED_PCM) / combined),
        (
            "P vs k [sigma]",
            z(sf.k_tracklength.mean, sf.k_tracklength.std, k.mean, k.std),
        ),
        (
            "balance closure [sigma]",
            bal.closure.mean / bal.closure.std,
        ),
        (
            "IFP beta_eff vs k-ratio [sigma]",
            z(
                kin.beta_eff.mean * 1e5,
                kin.beta_eff.std * 1e5,
                K_RATIO_BETA_PCM.0,
                K_RATIO_BETA_PCM.1,
            ),
        ),
    ];
    println!(
        "\nk_eff = {k} ({pcm:+.0} pcm from HEU-MET-FAST-001); Lambda by IFP / by k-ratio = {:.3}",
        kin.generation_time_s.mean / K_RATIO_LAMBDA_S
    );
    for (name, v) in checks {
        println!("  {name:<34} {v:+.2}");
        assert!(
            v.abs() <= K_SIGMA,
            "{name} = {v:+.2}: beyond {K_SIGMA} sigma. Find out why; do not widen the gate."
        );
    }
    let all_bins = whole.total_in_grid.mean + whole.outside_grid.0 + whole.outside_grid.1;
    println!(
        "  spectrum total / balance flux - 1   {:+.1e}",
        all_bins / bal.total.flux.mean - 1.0
    );
    assert!((all_bins / bal.total.flux.mean - 1.0).abs() < 1e-12);
    Ok(())
}
