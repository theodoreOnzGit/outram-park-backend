//! Godiva bare-sphere criticality on the **HIGH tier**, from the repo's own
//! ENDF/B-VIII.0 tapes — no network.
//!
//! # Why this exists
//!
//! The FHR pebble study (`verification_and_validation/ring_rpt/`) sits ~+1.7 %
//! above its OpenMC reference and every *data* mechanism has been excluded by
//! measurement: point σ vs NJOY's PENDF (±0.04 %), the capture **resonance
//! integral** vs NJOY and vs the published RI_∞ (+0.00 %,
//! `u238_resonance_integral.rs`), graphite σ vs THERMR (±0.05 %), and the
//! slowing-down kernel above the S(α,β) cutoff against analytic two-body
//! kinematics (`epithermal_slowing_down.rs`, ξ/ξ₀ = 1.000).
//!
//! What has never been done is to point the **same HIGH data path and the same
//! power-iteration driver** at a problem with an *external, measured* answer.
//! Every comparison so far has been against another code. Godiva is ICSBEP
//! **HEU-MET-FAST-001**: a bare HEU metal sphere, r = 8.7407 cm, benchmark
//! `k_eff = 1.0000 ± 0.0010`. That is an experiment, not a code.
//!
//! It also splits the search cleanly, because Godiva is a *fast* system:
//!
//! | if HIGH-tier Godiva … | then the pebble offset is … |
//! |---|---|
//! | lands on 1.0000 | specific to thermal / epithermal physics, not the shared path |
//! | sits ~+1–2 % high too | a **general** bias in the HIGH path (ν̄, χ, fast σ, or the driver) |
//!
//! Godiva exercises ν̄(E), the MF=5 fission spectrum, U-235 fast fission,
//! inelastic levels and (n,2n) — and **no** thermal scattering and **no**
//! resonance escape. It is the complement of the pebble.
//!
//! `godiva_keff_endf` already does this, but only with `--features net-fetch`
//! against ENDF/B-VII.1 from the IAEA, which this environment cannot reach.
//! This twin reads `reference-data/endf/` instead and uses **ENDF/B-VIII.0** —
//! the same library and the same tapes the pebble study runs on, which is what
//! makes the split above valid.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example godiva_keff_endf_local
//! ```

use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{run_keff, KeffSettings};
use std::time::Instant;

/// Godiva material temperature \[K\] (room temperature; the benchmark is a metal
/// assembly, not a reactor).
const TEMP_K: f64 = 293.6;

fn main() {
    println!("Reconstructing HEU isotopes from ENDF/B-VIII.0 (RECONR + BROADR @ {TEMP_K} K)…");
    let t0 = Instant::now();
    let nuclides: Vec<Nuclide> = [
        ("U234", "n-092_U_234-ENDF8.0.endf"),
        ("U235", "n-092_U_235-ENDF8.0.endf"),
        ("U238", "n-092_U_238.endf"),
    ]
    .iter()
    .map(|(name, file)| {
        let p = reference_endf(file).unwrap_or_else(|| panic!("missing reference tape {file}"));
        let t = Instant::now();
        let n = Nuclide::from_endf_file(&p, name, TEMP_K, 1.0e-3)
            .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display()));
        println!(
            "  {name}: reconstructed in {:.1} s",
            t.elapsed().as_secs_f64()
        );
        n
    })
    .collect();
    println!(
        "Nuclear data ready in {:.1} s.\n",
        t0.elapsed().as_secs_f64()
    );

    // HEU-MET-FAST-001 atom densities [atoms/barn·cm] — the same numbers the
    // LOW-tier `godiva_keff` and the net-fetch `godiva_keff_endf` use, so the
    // three runs differ only in where σ comes from.
    let material = Material {
        id: 1,
        name: "Godiva HEU".into(),
        temperature: TEMP_K,
        components: vec![
            NuclideComponent {
                nuclide_idx: 0,
                atom_density: 4.9184e-4,
            }, // U-234
            NuclideComponent {
                nuclide_idx: 1,
                atom_density: 4.4994e-2,
            }, // U-235
            NuclideComponent {
                nuclide_idx: 2,
                atom_density: 2.4984e-3,
            }, // U-238
        ],
    };

    let settings = KeffSettings {
        n_particles: 5000,
        n_inactive: 40,
        n_active: 120,
        temperature_k: TEMP_K,
        ..KeffSettings::default()
    };

    let radius_cm = 8.7407;
    println!("Godiva bare-sphere Keff — HIGH fidelity, ENDF/B-VIII.0  (r = {radius_cm} cm)");
    println!(
        "  {} histories/gen, {} inactive + {} active generations\n",
        settings.n_particles, settings.n_inactive, settings.n_active
    );

    let t_mc = Instant::now();
    let result = run_keff(radius_cm, &material, &nuclides, &settings);
    println!("  transport: {:.1} s", t_mc.elapsed().as_secs_f64());
    println!("  k_eff = {:.5} ± {:.5}", result.k_mean, result.k_std);
    println!("  ICSBEP HEU-MET-FAST-001 = 1.0000 ± 0.0010");
    let pcm = (result.k_mean - 1.0) * 1.0e5;
    let sigma = result.k_std * 1.0e5;
    println!("  Δk from benchmark = {pcm:+.0} ± {sigma:.0} pcm");
}
