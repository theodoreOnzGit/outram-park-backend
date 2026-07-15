//! `depletion` notebook -> outram-mc verification (LIVE, honest one-group scope).
//!
//! ⚠️ AI-GENERATED DRAFT — HUMAN REVIEW REQUIRED per `RESPONSIBLE_USE.md`.
//!
//! Notebook: `depletion.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Depletes a 4.25%-enriched UO₂ fuel pin at a
//! linear power of 174 W/cm over six 30-day steps (a six-month cycle) with a
//! coupled transport/transmutation operator (`deplete.CoupledOperator` +
//! `deplete.PredictorIntegrator`) using the 9-nuclide `chain_simple.xml`
//! (I135, Xe135, Xe136, Cs135, Gd157, Gd156, U234, U235, U238), and inspects the
//! nuclide inventories over burnup.
//!
//! **Notebook reference outputs (from the `.ipynb` cell outputs).** k-effective
//! over the seven report points:
//! `1.46478 ± 0.00392`, `1.43891 ± 0.00466`, `1.43604 ± 0.00493`,
//! `1.42959 ± 0.00429`, `1.42755 ± 0.00464`, `1.42575 ± 0.00483`,
//! `1.42368 ± 0.00460` (t = 0, 30, 60, 90, 120, 150, 180 days) — a monotonic
//! decrease of ≈ −4110 pcm over the cycle. Nuclide trend: U-235 decreases
//! monotonically; Xe-135 rises then plateaus (xenon poisoning).
//!
//! # What outram-mc does today (this run wires it — op-6tz.18)
//!
//! The depletion driver ([`outram_mc_libs::depletion`]) now exists:
//! - **CRAM** matrix-exponential solver ([`outram_mc_libs::depletion::cram::cram16`])
//!   — verified to machine precision against analytic Bateman solutions.
//! - **`DepletionChain`** ([`outram_mc_libs::depletion::chain`]) — the exact
//!   `chain_simple.xml` chain, consuming ENDF/B-8 decay data + ENDF/B-VIII.0
//!   fission yields.
//! - **Burnup loop** ([`outram_mc_libs::depletion::operator::deplete_predictor`])
//!   — predictor / forward-Euler, with one-group reaction rates built from the
//!   `njoy-outram-park-fork` CORE cross sections and a power-normalised flux.
//!
//! # V&V — methodology and results
//!
//! **Methodology.** This is a **one-group, infinite-medium** reproduction of the
//! notebook case (fidelity caveats in the `operator` module docs): cross sections
//! are evaluated at the 0.0253 eV thermal point rather than from a
//! transport-tallied spectrum, and `k_inf` is the one-group ratio over the chain
//! nuclides — a *relative trend* indicator, **not** comparable in absolute value
//! to the notebook's continuous-energy pin-cell MC `k`. The transmutation step
//! itself (CRAM) is separately verified to analytic accuracy in
//! `src/depletion/cram.rs`. The LIVE assertions here are the
//! **physically-required trends the notebook exhibits**, which are reproducible:
//! 1. U-235 depletes monotonically across all six steps.
//! 2. The fission products (I-135, Xe-135, Cs-135) build up from zero.
//! 3. Xe-135 reaches its flux equilibrium within the first 30-day step and then
//!    holds there (its later increments are a small fraction of the initial
//!    jump) — the xenon-poisoning saturation the notebook shows as a plateau.
//! 4. `k_inf` decreases over the cycle (same sign as the notebook's −4110 pcm).
//! 5. The magnitude of U-235 destroyed is consistent (order of magnitude) with
//!    the energy produced: `fissioned_atoms ≈ P·t / Q`.
//!
//! Data: `Nuclide::from_core` LOW-tier WMP/MGXS for every chain nuclide (all 9
//! are in the CORE-125 set); `chain_simple.xml` decay/yield structure.
//!
//! **Results (2026-07-15, this harness; asserted at run time).** Recorded in
//! `docs/ai-fleet-review/op-6tz-depletion/REVIEW_MANIFEST.md` and the generated
//! comparison CSV
//! `verification_and_validation/openmc_notebook_comparisons/depletion.csv`
//! (gitignored).
//!
//! A genuine Monte Carlo `k` on the evolved actinide inventory is exercised by
//! [`depletion_mc_transport_coupling`] to show the transport path is wired
//! (fast-spectrum bare sphere — absolute value not comparable to the notebook).

use outram_mc_libs::depletion::chain::DepletionChain;
use outram_mc_libs::depletion::operator::{deplete_predictor, BurnupResult, BurnupSettings};

/// Beginning-of-life UO₂ inventory for the notebook pin (4.25% enrichment,
/// 10.4 g/cm³), in atoms/(barn·cm). O-16 is omitted — it does not transmute in
/// `chain_simple` and the one-group infinite-medium `k_inf` ignores moderation.
///
/// UO₂ molar mass ≈ 269.9 g/mol ⇒ N(UO₂) = 10.4/269.9 · N_A · 1e-24
/// ≈ 0.02320 atoms/(barn·cm) of uranium; split by enrichment.
fn bol_inventory() -> Vec<(String, f64)> {
    let n_u = 0.02320_f64; // total U atom density [atoms/(barn·cm)]
    vec![
        ("U234".to_string(), 0.0),
        ("U235".to_string(), 0.0425 * n_u),
        ("U238".to_string(), 0.9575 * n_u),
    ]
}

/// Run the notebook burnup case once and return the trajectory.
fn run_notebook_case() -> BurnupResult {
    let chain = DepletionChain::simple();
    let settings = BurnupSettings::default(); // 174 W, π·0.42² cm³, 6 × 30 days
    deplete_predictor(&chain, &bol_inventory(), &settings)
}

/// LIVE: the depletion driver reproduces the notebook's inventory + k trends.
///
/// Asserts the five physically-required trends listed in the module docs. These
/// hold by physics for any correct depletion of this chain at power, so they are
/// a real (not fake-green) check of the coupled loop, while the CRAM step's
/// numerical accuracy is proven separately in `src/depletion/cram.rs`.
#[test]
fn depletion_burnup() {
    let result = run_notebook_case();
    assert_eq!(result.steps.len(), 7, "BOL + six 30-day steps");

    let u235: Vec<f64> = result.steps.iter().map(|s| s.density("U235")).collect();
    let xe135: Vec<f64> = result.steps.iter().map(|s| s.density("Xe135")).collect();
    let i135: Vec<f64> = result.steps.iter().map(|s| s.density("I135")).collect();
    let cs135: Vec<f64> = result.steps.iter().map(|s| s.density("Cs135")).collect();
    let kinf: Vec<f64> = result.steps.iter().map(|s| s.k_inf).collect();

    // (1) U-235 depletes monotonically.
    for w in u235.windows(2) {
        assert!(w[1] < w[0], "U-235 must decrease each step: {} -> {}", w[0], w[1]);
    }
    assert!(u235[0] > 0.0);

    // (2) Fission products build up from zero.
    assert_eq!(xe135[0], 0.0);
    assert!(xe135.last().unwrap() > &0.0, "Xe-135 must accumulate");
    assert!(i135.last().unwrap() > &0.0, "I-135 must accumulate");
    assert!(cs135.last().unwrap() > &0.0, "Cs-135 must accumulate");

    // (3) Xe-135 reaches its flux equilibrium within the first step and then
    // holds there (xenon's ~9 h half-life equilibrates far inside a 30-day
    // step): the initial jump is large and every later step's |increment| is a
    // small fraction of it. This is the xenon-poisoning saturation the notebook
    // shows as a plateau over its 30-day report points.
    let first_inc = xe135[1] - xe135[0];
    let last_inc = xe135[6] - xe135[5];
    assert!(first_inc > 0.0, "Xe-135 jumps up in the first step");
    assert!(
        last_inc.abs() < 0.1 * first_inc,
        "Xe-135 should be at equilibrium: first inc {first_inc:e}, last inc {last_inc:e}"
    );

    // (4) k_inf decreases over the cycle (same sign as the notebook's −4110 pcm).
    assert!(
        kinf[6] < kinf[0],
        "k_inf must fall over burnup: {} -> {}",
        kinf[0],
        kinf[6]
    );

    // (5) Order-of-magnitude energy balance: the fissile destroyed over the run
    // is consistent with P·t / Q. P = 174 W, t = 180 d, Q ≈ 193.4 MeV ⇒
    // ~1.5e-4 atoms/(barn·cm) fissioned in the 0.554 cm³ pin. U-235 consumed
    // (fission + capture) should be within an order of magnitude of that.
    let u235_consumed = u235[0] - u235[6];
    assert!(
        (1.0e-5..1.0e-3).contains(&u235_consumed),
        "U-235 consumed {u235_consumed:e} outside the physical band"
    );

    // Emit the comparison CSV (gitignored) and a human-readable trace.
    write_comparison_csv(&result);

    eprintln!("depletion (one-group) trajectory:");
    eprintln!("  day   U235[a/b·cm]   Xe135         I135          k_inf");
    for s in &result.steps {
        eprintln!(
            "  {:>3.0}   {:.6e}   {:.6e}   {:.6e}   {:.5}",
            s.time_days,
            s.density("U235"),
            s.density("Xe135"),
            s.density("I135"),
            s.k_inf
        );
    }
    eprintln!(
        "  k_inf swing: {:.5} -> {:.5}  ({:+.0} pcm);  notebook MC k: 1.46478 -> 1.42368 (-4110 pcm)",
        kinf[0],
        kinf[6],
        (kinf[6] - kinf[0]) * 1.0e5
    );
}

/// LIVE demonstration: the evolved actinide inventory feeds a real Monte Carlo
/// `k_eff` transport solve. This is a **fast-spectrum bare sphere**, so its `k`
/// is far below the notebook's moderated pin cell and is *not* a benchmark —
/// its purpose is to show the depletion→transport coupling path executes on a
/// depleted inventory and returns a finite eigenvalue.
#[test]
fn depletion_mc_transport_coupling() {
    use outram_mc_libs::depletion::operator::mc_keff_of_actinide_sphere;
    use outram_mc_libs::physics::keff::KeffSettings;

    let result = run_notebook_case();
    // Modest, seeded (deterministic) MC run.
    let settings =
        KeffSettings { n_particles: 500, n_inactive: 10, n_active: 30, ..KeffSettings::default() };

    let bol: Vec<(String, f64)> =
        result.bol().densities.iter().filter(|(n, _)| n.starts_with('U')).cloned().collect();
    let eol: Vec<(String, f64)> =
        result.eol().densities.iter().filter(|(n, _)| n.starts_with('U')).cloned().collect();

    let k_bol = mc_keff_of_actinide_sphere(&bol, 12.0, &settings);
    let k_eol = mc_keff_of_actinide_sphere(&eol, 12.0, &settings);

    assert!(k_bol.k_mean.is_finite() && k_bol.k_mean > 0.0, "BOL MC k must be finite/positive");
    assert!(k_eol.k_mean.is_finite() && k_eol.k_mean > 0.0, "EOL MC k must be finite/positive");
    eprintln!(
        "MC bare-sphere k (fast, NOT notebook-comparable): BOL {:.5} ± {:.5}, EOL {:.5} ± {:.5}",
        k_bol.k_mean, k_bol.k_std, k_eol.k_mean, k_eol.k_std
    );
}

/// Write the notebook-comparison CSV to the gitignored comparisons folder.
///
/// One row per burnup step. Columns follow the crate V&V convention adapted for
/// a depletion time series (see this crate's `CLAUDE.md`).
fn write_comparison_csv(result: &BurnupResult) {
    use std::io::Write;
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/verification_and_validation/openmc_notebook_comparisons"
    );
    let _ = std::fs::create_dir_all(dir);
    let path = format!("{dir}/depletion.csv");
    let Ok(mut f) = std::fs::File::create(&path) else { return };

    // Notebook MC k per report point (t = 0..180 d).
    let ref_k = [1.46478, 1.43891, 1.43604, 1.42959, 1.42755, 1.42575, 1.42368];
    let ref_sigma = [0.00392, 0.00466, 0.00493, 0.00429, 0.00464, 0.00483, 0.00460];

    let _ = writeln!(
        f,
        "date,notebook,case,day,our_k_inf_onegroup,ref_mc_k,ref_mc_sigma,\
         our_U235,our_Xe135,our_I135,our_Cs135,flux_n_cm2_s,data_used,note"
    );
    for (i, s) in result.steps.iter().enumerate() {
        let _ = writeln!(
            f,
            "2026-07-15,depletion.ipynb@cf1e5db,pin_UO2_4.25pct,{:.0},{:.6},{:.5},{:.5},\
             {:.6e},{:.6e},{:.6e},{:.6e},{:.4e},\
             CORE-WMP+MGXS(one-group 0.0253eV)|chain_simple.xml|ENDF-B-VIII.0 yields,\
             one-group k_inf trend only; absolute k NOT comparable to notebook MC pin k",
            s.time_days,
            s.k_inf,
            ref_k[i.min(6)],
            ref_sigma[i.min(6)],
            s.density("U235"),
            s.density("Xe135"),
            s.density("I135"),
            s.density("Cs135"),
            s.flux,
        );
    }
    eprintln!("wrote comparison CSV: {path}");
}
