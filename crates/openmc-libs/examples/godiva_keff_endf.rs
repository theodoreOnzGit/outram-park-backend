//! Godiva bare-sphere criticality — **HIGH-fidelity** run on device-reconstructed
//! ENDF cross sections.
//!
//! This is the HIGH-tier counterpart to [`godiva_keff`](godiva_keff), the offline
//! LOW-tier run. Same geometry, same material, same power-iteration driver — the
//! *only* change is where the cross sections come from:
//!
//! | | LOW (`godiva_keff`) | HIGH (`godiva_keff_endf`) |
//! |---|---|---|
//! | Source | embedded WMP + fast MGXS | raw ENDF downloaded + reconstructed |
//! | Fast range | 10-group, Watt-collapsed, infinite dilution | continuous-energy pointwise |
//! | Self-shielding | none (group averages) | implicit (σ sampled at actual E) |
//! | ν̄ | constant stopgap table | energy-dependent, ENDF MF=1/452 |
//! | Doppler | analytic (WMP) below `e_max` only | BROADR over the whole range |
//! | Needs network | no | yes (cached after first run) |
//!
//! For each of the three uranium isotopes in HEU metal this:
//! downloads the ENDF/B-VII.1 neutron tape from the pinned IAEA upstream, runs
//! RECONR (full resonance reconstruction), BROADR (Doppler to 293.6 K), and reads
//! ν̄(E) from MF=1/452 — all on device, via [`Nuclide::from_endf`]. ENDF/B-VII.1
//! is used (not VIII.0) because its U resonances are Reich-Moore (LRF=3), which
//! the RECONR port reconstructs; VIII.0 U is LRF=7 (not yet ported).
//!
//! Run with (needs the `net-fetch` feature and a network connection):
//! ```text
//! cargo run --release -p openmc-libs --features net-fetch --example godiva_keff_endf
//! ```
//!
//! The first run downloads ~150 MB of ENDF tapes (U-234/235/238) and spends most
//! of its wall-clock in RECONR reconstructing the resonance range; both the tapes
//! and — on re-run — the OS disk cache make subsequent runs faster. Reconstruction
//! dominates startup; the power iteration itself is comparable to the LOW run.
//!
//! # V&V — methodology and results
//!
//! **Methodology.** Same Godiva model as the LOW run (bare HEU sphere,
//! r = 8.7407 cm, atom densities below), same power iteration (5000 histories ×
//! [40 inactive + 110 active]), judged against ICSBEP HEU-MET-FAST-001
//! (k_eff = 1.0000 ± 0.0010). The only variable changed is the cross-section
//! source: continuous-energy ENDF/B-VII.1 reconstructed on device (RECONR 0.1%
//! tol + BROADR to 293.6 K + MF=1/452 ν̄), vs the LOW tier's embedded
//! WMP + infinite-dilution fast MGXS.
//!
//! **Results (2026-07, ENDF/B-VII.1).**
//!
//! | Run | k_eff | Δk vs benchmark |
//! |---|---|---|
//! | LOW (`godiva_keff`) | 1.12852 ± 0.00174 | +12 852 pcm |
//! | HIGH (`godiva_keff_endf`) | 1.12451 ± 0.00202 | +12 451 pcm |
//!
//! **Interpretation — the key finding.** Going from coarse infinite-dilution
//! group data to full continuous-energy reconstructed data moves k_eff by only
//! **~400 pcm**, far short of the ~12 500 pcm bias. The overprediction is
//! therefore **not** data-fidelity-limited; it is dominated by the transport
//! physics *shared* by both runs — inelastic and (n,xn) scattering lumped into an
//! elastic-like event (no real energy-loss law) plus isotropic-CM elastic
//! scatter, which together keep the neutron spectrum too hard regardless of how
//! accurate the cross sections are. Closing the gap needs a proper inelastic
//! energy-loss law and anisotropic elastic scatter, not better data. See
//! `docs/development-history.md`.

#[cfg(not(feature = "net-fetch"))]
fn main() {
    eprintln!(
        "this example needs the `net-fetch` feature:\n  \
         cargo run --release -p openmc-libs --features net-fetch --example godiva_keff_endf"
    );
    std::process::exit(2);
}

#[cfg(feature = "net-fetch")]
fn main() {
    use njoy_outram_park_fork::acquire::EndfLibrary;
    use openmc_libs::material::material::{Material, NuclideComponent};
    use openmc_libs::material::nuclide::Nuclide;
    use openmc_libs::physics::keff::{run_keff, KeffSettings};
    use std::time::Instant;

    let temp_k = 293.6; // Godiva material temperature
    let lib = EndfLibrary::EndfBVII1;

    // ── Nuclear data: reconstruct the three HEU isotopes from raw ENDF. ───────
    println!("Reconstructing HEU isotopes from {} (RECONR + BROADR @ {temp_k} K)…", lib.label());
    let t0 = Instant::now();
    let nuclides: Vec<Nuclide> = ["U234", "U235", "U238"]
        .iter()
        .map(|name| {
            let t = Instant::now();
            let n = Nuclide::from_endf(lib, name, temp_k, 1.0e-3)
                .unwrap_or_else(|e| panic!("reconstruct {name}: {e}"));
            println!("  {name}: reconstructed in {:.1} s", t.elapsed().as_secs_f64());
            n
        })
        .collect();
    println!("Nuclear data ready in {:.1} s.\n", t0.elapsed().as_secs_f64());

    // ── Material: Godiva atom densities [atoms/barn·cm] (HEU-MET-FAST-001). ────
    let material = Material {
        id: 1,
        name: "Godiva HEU".into(),
        temperature: temp_k,
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 }, // U-234
            NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 }, // U-235
            NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 }, // U-238
        ],
    };

    // ── Power iteration (same settings as the LOW-tier example). ──────────────
    let settings = KeffSettings {
        n_particles: 5000,
        n_inactive: 40,
        n_active: 110,
        temperature_k: temp_k,
        ..KeffSettings::default()
    };

    let radius_cm = 8.7407;
    println!("Godiva bare-sphere Keff — HIGH fidelity  (r = {radius_cm} cm)");
    println!(
        "  {} histories/gen, {} inactive + {} active generations\n",
        settings.n_particles, settings.n_inactive, settings.n_active
    );

    let t_mc = Instant::now();
    let result = run_keff(radius_cm, &material, &nuclides, &settings);
    println!("  transport: {:.1} s", t_mc.elapsed().as_secs_f64());

    println!("  k_eff = {:.5} ± {:.5}", result.k_mean, result.k_std);
    println!("  ICSBEP benchmark = 1.0000 ± 0.0010");
    let pcm = (result.k_mean - 1.0) * 1.0e5;
    println!("  Δk from benchmark = {pcm:+.0} pcm");
}
