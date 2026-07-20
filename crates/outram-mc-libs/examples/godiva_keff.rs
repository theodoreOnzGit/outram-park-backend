//! Godiva bare-sphere criticality — the first end-to-end Keff in `outram-mc-libs`.
//!
//! Godiva is ICSBEP benchmark **HEU-MET-FAST-001**: a bare sphere of highly
//! enriched uranium metal, r ≈ 8.7407 cm, benchmark **k_eff = 1.0000 ± 0.0010**.
//! It is the canonical fast-spectrum criticality check.
//!
//! This example composes the whole transport stack — nuclear data pulled from
//! `njoy-outram-park-fork` (WMP + fast MGXS), the bare-sphere geometry, analog
//! collision physics, and fission-source power iteration — and prints the
//! converged eigenvalue. Cross sections ship *inside* the crates, so it runs with
//! no downloads and no HDF5.
//!
//! Run with:
//! ```text
//! cargo run --release -p outram-mc-libs --example godiva_keff
//! ```
//!
//! Fidelity caveat: infinite-dilution fast group data (no self-shielding). The
//! LOW tier now models both inelastic down-scatter and forward-peaked elastic
//! from *group-averaged* data (see the V&V note below), so it lands close to 1.0,
//! but it still lacks self-shielding and resolves neither inelastic levels nor the
//! full elastic angular shape. See `docs/keff-doppler-roadmap.md`.
//!
//! For the HIGH-fidelity counterpart — the *same* run on continuous-energy ENDF
//! cross sections reconstructed on device — see the `godiva_keff_endf` example
//! (`--features net-fetch`) and the LOW-vs-HIGH comparison in
//! `docs/development-history.md`.
//!
//! # V&V — methodology and results
//!
//! **Methodology.** Godiva bare HEU sphere (r = 8.7407 cm, HEU-MET-FAST-001 atom
//! densities below), 5000 histories × [40 inactive + 110 active], judged against
//! ICSBEP HEU-MET-FAST-001 (k_eff = 1.0000 ± 0.0010). Cross sections are the
//! embedded LOW tier: WMP below each nuclide's `e_max`, Watt-collapsed 10-group
//! fast MGXS above it. The fast MGXS carries, per group: σ_t/σ_el/σ_f/σ_γ/νσ_f
//! **and** a mean elastic cosine μ̄ baked from ENDF/B-VIII.0 MF=4. Inelastic is
//! the group remainder σ_t − σ_el − σ_f − σ_γ, down-scattered by the Weisskopf
//! evaporation law; elastic is forward-scattered by a maximum-entropy exponential
//! angular law that reproduces μ̄ (valid even where μ̄ ≫ 1/3, unlike a P1 law).
//!
//! **Results (2026-07, ENDF/B-VIII.0 group data) — adding scatter physics to the
//! embedded tier.**
//!
//! | LOW-tier model | k_eff | Δk vs benchmark |
//! |---|---|---|
//! | elastic-only, isotropic-CM (before) | 1.12852 ± 0.00174 | +12 852 pcm |
//! | + inelastic (evaporation) + **forward elastic (μ̄)** | **1.01022 ± 0.00177** | **+1 022 pcm** |
//!
//! **Interpretation.** The two scatter mechanisms remove ~11 800 pcm and bring the
//! offline/embedded tier essentially to the benchmark — the same two levers that
//! dominated the HIGH-tier study (`docs/development-history.md`), reproduced from
//! coarse group data plus a single per-group μ̄. The residual +1 022 pcm is
//! consistent with the LOW tier's remaining approximations (no self-shielding,
//! one mean cosine instead of the full MF=4 shape, evaporation as a stand-in for
//! resolved inelastic levels); it should not be read as each sub-model being
//! individually exact.

use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{run_keff, KeffSettings};

fn main() {
    // ── Nuclear data: the three uranium isotopes in HEU metal. ────────────────
    let nuclides = vec![
        Nuclide::from_core("U234").expect("U234 in CORE WMP library"),
        Nuclide::from_core("U235").expect("U235 in CORE WMP library"),
        Nuclide::from_core("U238").expect("U238 in CORE WMP library"),
    ];

    // ── Material: Godiva atom densities [atoms/barn·cm] (HEU-MET-FAST-001). ────
    let material = Material {
        id: 1,
        name: "Godiva HEU".into(),
        temperature: 293.6, // K
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 }, // U-234
            NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 }, // U-235
            NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 }, // U-238
        ],
    };

    // ── Power iteration. ──────────────────────────────────────────────────────
    let settings = KeffSettings {
        n_particles: 5000,
        n_inactive: 40,
        n_active: 110,
        ..KeffSettings::default()
    };

    let radius_cm = 8.7407;
    println!("Godiva bare-sphere Keff  (r = {radius_cm} cm)");
    println!(
        "  {} histories/gen, {} inactive + {} active generations\n",
        settings.n_particles, settings.n_inactive, settings.n_active
    );

    let result = run_keff(radius_cm, &material, &nuclides, &settings);

    println!("  k_eff = {:.5} ± {:.5}", result.k_mean, result.k_std);
    println!("  ICSBEP benchmark = 1.0000 ± 0.0010");
    let pcm = (result.k_mean - 1.0) * 1.0e5;
    println!("  Δk from benchmark = {pcm:+.0} pcm");
}
