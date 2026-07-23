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
//! | Inelastic | group remainder, evaporation (no levels) | explicit MT=51…91 energy-loss law |
//! | Elastic angle | forward-peaked from group μ̄ (max-entropy) | anisotropic (full ENDF MF=4) |
//! | (n,2n) | lumped in group total (no multiplication) | MT=16, yield-2 multiplicity |
//! | Fission χ | fixed thermal-Watt spectrum | energy-dependent ENDF MF=5/MT=18 (LF=1) |
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
//! cargo run --release -p outram-mc-libs --features net-fetch --example godiva_keff_endf
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
//! [40 inactive + 120 active]), judged against ICSBEP HEU-MET-FAST-001
//! (k_eff = 1.0000 ± 0.0010). The only variable changed is the cross-section
//! source: continuous-energy ENDF/B-VII.1 reconstructed on device (RECONR 0.1%
//! tol + BROADR to 293.6 K + MF=1/452 ν̄), vs the LOW tier's embedded
//! WMP + infinite-dilution fast MGXS.
//!
//! **Results (2026-07-03, ENDF/B-VII.1) — the HIGH tier reaching the benchmark.**
//!
//! | Run | k_eff | Δk vs benchmark |
//! |---|---|---|
//! | LOW (`godiva_keff`) | 1.12852 ± 0.00174 | +12 852 pcm |
//! | HIGH: CE data only, elastic-lumped | 1.12451 ± 0.00202 | +12 451 pcm |
//! | HIGH: + inelastic energy-loss law | 1.09942 ± 0.00169 | +9 942 pcm |
//! | HIGH: + anisotropic MF=4 elastic | 0.99701 ± 0.00168 | −299 pcm |
//! | HIGH: + (n,2n) yield-2 multiplicity | 0.99872 ± 0.00173 | −128 pcm |
//! | HIGH: + **energy-dependent MF=5 χ** | **1.00367 ± 0.00182** | **+367 pcm** |
//!
//! **Interpretation — the key findings** (full derivation in
//! `docs/development-history.md`). Three effects were isolated in sequence, and
//! their ranking is the durable lesson:
//!
//! 1. **Data fidelity is not the bottleneck** — full continuous-energy
//!    reconstructed data over coarse group data moved k_eff only **~400 pcm**.
//! 2. **Inelastic scattering** (discrete-level two-body + evaporation continuum,
//!    MT=51…91) softened the spectrum and removed **~2 510 pcm**.
//! 3. **Anisotropic elastic scatter** (ENDF MF=4) is by far the largest lever,
//!    **~10 300 pcm**: forward-peaked elastic off heavy U cuts the transport cross
//!    section and lets the bare sphere leak the reactivity the isotropic
//!    approximation had retained. This brings Godiva to **k_eff = 0.99701 ±
//!    0.00168 (−299 pcm)** — agreement with the benchmark.
//! 4. **(n,2n) yield-2 multiplicity** (MT=16) adds **+171 ± 241 pcm** (to 0.99872,
//!    −128 pcm) — the correct sign but only ~0.7σ, *not* resolved from zero. A
//!    fidelity fix, not a lever: U (n,2n) is a ~5–6 MeV threshold reaction sampling
//!    only the fission-spectrum tail, so its Godiva worth is genuinely tens of pcm.
//! 5. **Energy-dependent MF=5 χ** (real ENDF fission birth spectrum in place of the
//!    fixed thermal Watt) adds **+495 ± 251 pcm** (to 1.00367, +367 pcm) — positive
//!    and ~2.0σ, *marginally* resolved. The U-235 χ mean (~2.03 MeV) ≈ the Watt
//!    mean, so the worth is in the *shape*: the tabulated spectrum keeps more births
//!    in the productive 1–3 MeV band and fewer in the leaky tail.
//!
//! For fast bare-metal criticality the transport angular/energy-transfer physics
//! dominates cross-section-data fidelity by more than an order of magnitude. The
//! near-perfect landing likely involves some cancellation of the residual
//! approximations (no fast self-shielding; Weisskopf stand-in for the MF=6 (n,2n)
//! emission law), so it should not be read as each sub-model being individually
//! exact. The LOW (embedded) tier carries the same elastic + inelastic levers from
//! *group* data (no (n,2n) column, no MF=5 χ yet) and lands at 1.01024
//! (+1 024 pcm); see the `godiva_keff` (LOW) example for that V&V.

#[cfg(not(feature = "net-fetch"))]
fn main() {
    eprintln!(
        "this example needs the `net-fetch` feature:\n  \
         cargo run --release -p outram-mc-libs --features net-fetch --example godiva_keff_endf"
    );
    std::process::exit(2);
}

#[cfg(feature = "net-fetch")]
fn main() {
    use njoy_outram_park_fork::acquire::EndfLibrary;
    use outram_mc_libs::material::material::{Material, NuclideComponent};
    use outram_mc_libs::material::nuclide::Nuclide;
    use outram_mc_libs::physics::keff::{run_keff, KeffSettings};
    use std::time::Instant;

    let temp_k = 293.6; // Godiva material temperature
    let lib = EndfLibrary::EndfBVII1;

    // ── Nuclear data: reconstruct the three HEU isotopes from raw ENDF. ───────
    println!("Reconstructing HEU isotopes from {} (RECONR + BROADR @ {temp_k} K)…", lib.label());
    let t0 = Instant::now();
    let mut nuclides: Vec<Nuclide> = Vec::with_capacity(3);
    for name in ["U234", "U235", "U238"] {
        let t = Instant::now();
        match Nuclide::from_endf(lib, name, temp_k, 1.0e-3) {
            Ok(n) => {
                println!("  {name}: reconstructed in {:.1} s", t.elapsed().as_secs_f64());
                nuclides.push(n);
            }
            // Fail gracefully rather than panicking with a backtrace: the usual
            // cause is no outbound network (offline / restrictive proxy). Print
            // an honest explanation and point at the offline LOW-tier twin, then
            // exit non-zero cleanly.
            Err(e) => {
                eprintln!(
                    "\ncould not obtain ENDF data for {name}: {e}\n\n\
                     This HIGH-fidelity example downloads the ENDF/B-VII.1 neutron tapes\n\
                     (~150 MB for U-234/235/238) from the IAEA Nuclear Data Services and\n\
                     reconstructs them on device (RECONR + BROADR), so it needs outbound\n\
                     network access to https://www-nds.iaea.org. If you are offline or\n\
                     behind a restrictive proxy, run the offline LOW-tier twin instead\n\
                     (embedded WMP data, no network):\n  \
                     cargo run --release -p outram-mc-libs --example godiva_keff"
                );
                std::process::exit(1);
            }
        }
    }
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
        n_active: 120,
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
