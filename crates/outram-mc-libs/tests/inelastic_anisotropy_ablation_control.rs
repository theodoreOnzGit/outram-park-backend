//! **Regression gate for the discrete-inelastic angular distributions** (bead
//! `op-tm9f`) and for the ablation control that prices them.
//!
//! # What this guards
//!
//! Until `op-tm9f`, every inelastic collision in this crate drew
//! `mu_cm = 2*prn − 1` — isotropic in the centre of mass — while elastic
//! correctly used the evaluation's ENDF MF=4 tabulated cosine. The discrete
//! levels are not isotropic, and treating them as such understates `⟨μ⟩`,
//! inflates `Σ_tr = Σ_t(1 − ⟨μ⟩)`, suppresses leakage and raises `k`. On Godiva
//! (55.8 % leakage) `examples/godiva_inelastic_anisotropy_ablation.rs` measures
//! that at roughly **−250 pcm**.
//!
//! That example takes ~30 minutes and belongs in an example. This is the cheap
//! part that belongs in the suite, and it guards two different things:
//!
//! - **The physics is wired in at all.** A regression that silently dropped the
//!   MF=4 inelastic tables would put Godiva back at `+214 pcm` with nothing in
//!   the suite failing. Asserting a cosine actually comes back is what catches
//!   it in seconds instead of in a 30-minute ensemble.
//! - **The ablation control actually ablates.** A no-op control reports "no
//!   difference" and reads as "this physics does not matter" — the worst failure
//!   mode an ablation study has.
//!
//! # Methodology
//!
//! U-238 (ENDF/B-VIII.0) is built HIGH-tier, then ablated with
//! [`Nuclide::with_isotropic_inelastic_scattering`]. U-238 rather than U-234
//! because its low-lying levels are the anisotropic ones: U-235's MT=51/52/54
//! are near-isotropic in the evaluation (`⟨μ⟩ ≈ 0` at every tabulated energy, a
//! real property of the data, not a parse defect), so a test written against
//! them would assert the wrong thing.
//!
//! At 2 MeV — inside Godiva's flux, above the MT=51 threshold — assert that:
//!
//! 1. the evaluated nuclide returns a CM cosine for MT=51 **every** time;
//! 2. the ablated nuclide returns `None` every time, so transport falls back to
//!    isotropic-CM through its existing path;
//! 3. the evaluated distribution is **genuinely anisotropic** and **forward**
//!    peaked, so there was something to remove and its sign is the one that
//!    lowers `k`;
//! 4. anisotropy **grows with energy**, which is the qualitative shape of the
//!    evaluated data and would not survive a mis-indexed incident-energy lookup;
//! 5. nothing else about the nuclide moved — σ_t, σ_f, σ_inel and ν̄σ_f
//!    bit-identical across the ablation, so the measured worth is the angular
//!    law and not a contaminating cross-section change.
//!
//! # Results (2026-09-15, ENDF/B-VIII.0 U-238)
//!
//! MT=51 at 2 MeV: evaluated returns a cosine 512/512 times with sampled
//! `⟨μ_cm⟩ = +0.0803`; ablated 0/512. Sampled `⟨μ_cm⟩` rises `+0.048` (1 MeV) →
//! `+0.266` (5 MeV) → `+0.519` (14 MeV). Cross sections bit-identical; both
//! tests pass in 285 s.
//!
//! Those sampled means sit on the distributions' own analytic
//! [`mean_cosine`](njoy_outram_park_fork::acer::angular::ElasticAngular::mean_cosine)
//! of `+0.033` / `+0.250` / `+0.510` to within the `±0.026` standard error of
//! 512 draws — an independent check that the sampler inverts the same CDF the
//! parser built, rather than only that it returns *something*.

use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use outram_mc_libs::material::nuclide::Nuclide;

const TEMP_K: f64 = 293.6;
/// Inside Godiva's flux and above U-238's MT=51 threshold (44.9 keV).
const PROBE_EV: f64 = 2.0e6;
/// U-238's first discrete inelastic level.
const MT_51: i32 = 51;
const DRAWS: usize = 512;

/// Mean of `n` sampled CM cosines for `mt` at `e`, or `None` if the nuclide
/// carries no distribution for that level.
fn mean_mu(nuc: &Nuclide, mt: i32, e: f64, n: usize, seed: u64) -> Option<f64> {
    let mut s = seed;
    let v: Vec<f64> = (0..n)
        .filter_map(|_| nuc.sample_inelastic_mu_cm(mt, e, &mut s))
        .collect();
    if v.len() < n {
        return None;
    }
    Some(v.iter().sum::<f64>() / v.len() as f64)
}

#[test]
fn inelastic_levels_are_anisotropic_and_the_ablation_removes_exactly_that() {
    let Some(tape) = reference_file_or_skip(
        "endf",
        "n-092_U_238.endf",
        "U-238 evaluation (inelastic anisotropy ablation control)",
    ) else {
        return;
    };

    let evaluated =
        Nuclide::from_endf_file(&tape, "U238", TEMP_K, 1.0e-3).expect("U-238 reconstructs");
    let ablated = evaluated.clone().with_isotropic_inelastic_scattering();

    // 1: the evaluated nuclide really carries MT=51 angular data.
    let mut s = 0xC0FFEEu64;
    let drawn: Vec<f64> = (0..DRAWS)
        .filter_map(|_| evaluated.sample_inelastic_mu_cm(MT_51, PROBE_EV, &mut s))
        .collect();
    assert_eq!(
        drawn.len(),
        DRAWS,
        "the EVALUATED nuclide returned an inelastic cosine only {}/{DRAWS} times for MT=51 at \
         {PROBE_EV:.1e} eV. Either the MF=4/MT=51 section stopped being read, or the level \
         lookup regressed -- Godiva goes back to +214 pcm if so.",
        drawn.len()
    );

    // 2: the control actually controls.
    let mut s = 0xC0FFEEu64;
    let after = (0..DRAWS)
        .filter_map(|_| ablated.sample_inelastic_mu_cm(MT_51, PROBE_EV, &mut s))
        .count();
    assert_eq!(
        after, 0,
        "with_isotropic_inelastic_scattering left {after}/{DRAWS} evaluated cosines in place -- \
         the ablation is a no-op, and any conclusion drawn from it would be an artefact"
    );

    // 3: it is genuinely anisotropic, and FORWARD peaked (the sign that lowers k).
    let mubar = drawn.iter().sum::<f64>() / drawn.len() as f64;
    assert!(
        mubar > 0.05,
        "MT=51 averages mu_cm = {mubar:+.4} at {PROBE_EV:.1e} eV. U-238's discrete levels are \
         forward-peaked at MeV energies; a value near zero means the MF=4 reading is producing \
         an isotropic distribution, and a NEGATIVE one would raise k rather than lower it."
    );

    // 4: anisotropy grows with energy -- the shape of the evaluated data, and a
    //    check that the incident-energy lookup is not mis-indexed.
    let m1 = mean_mu(&evaluated, MT_51, 1.0e6, DRAWS, 0xABCD).expect("MT=51 at 1 MeV");
    let m5 = mean_mu(&evaluated, MT_51, 5.0e6, DRAWS, 0xABCD).expect("MT=51 at 5 MeV");
    let m14 = mean_mu(&evaluated, MT_51, 1.4e7, DRAWS, 0xABCD).expect("MT=51 at 14 MeV");
    assert!(
        m1 < m5 && m5 < m14,
        "mu-bar should rise with incident energy (evaluation gives +0.03 -> +0.25 -> +0.51 at \
         1 / 5 / 14 MeV); sampled {m1:+.3} / {m5:+.3} / {m14:+.3}. A non-monotone result points \
         at the incident-energy bracketing in sample_mf4_mu_cm."
    );

    // 5: nothing else moved.
    let (xe, xa) = (
        evaluated.xs_at_energy(PROBE_EV, TEMP_K),
        ablated.xs_at_energy(PROBE_EV, TEMP_K),
    );
    for (name, a, b) in [
        ("total", xe.total, xa.total),
        ("elastic", xe.elastic, xa.elastic),
        ("fission", xe.fission, xa.fission),
        ("absorption", xe.absorption, xa.absorption),
        ("inelastic", xe.inelastic, xa.inelastic),
        ("nu_fission", xe.nu_fission, xa.nu_fission),
    ] {
        assert_eq!(
            a, b,
            "{name} changed across the ablation ({a:.9e} -> {b:.9e}). The control must alter the \
             ANGULAR distribution only; a cross-section change would contaminate the measured \
             worth of the anisotropy."
        );
    }

    println!(
        "  U-238 MT=51 @ {PROBE_EV:.1e} eV: evaluated {}/{DRAWS} cosines (mu-bar {mubar:+.4}), \
         ablated {after}/{DRAWS}; mu-bar {m1:+.3} / {m5:+.3} / {m14:+.3} at 1 / 5 / 14 MeV; \
         cross sections bit-identical",
        drawn.len()
    );
}

/// The elastic ablation must stay independent of the inelastic one — they price
/// different channels, and a shared code path that cleared both would make each
/// measurement silently include the other.
#[test]
fn the_two_ablations_are_independent() {
    let Some(tape) = reference_file_or_skip(
        "endf",
        "n-092_U_238.endf",
        "U-238 evaluation (ablation independence)",
    ) else {
        return;
    };
    let nuc = Nuclide::from_endf_file(&tape, "U238", TEMP_K, 1.0e-3).expect("U-238 reconstructs");

    // Ablating inelastic must leave elastic alone…
    let inel_gone = nuc.clone().with_isotropic_inelastic_scattering();
    let mut s = 1;
    assert!(
        inel_gone.sample_elastic_mu_cm(PROBE_EV, &mut s).is_some(),
        "with_isotropic_inelastic_scattering also cleared the ELASTIC distribution; the two \
         ablations would then measure each other"
    );

    // …and ablating elastic must leave the inelastic levels alone.
    let el_gone = nuc.with_isotropic_elastic_scattering();
    let mut s = 1;
    assert!(
        el_gone
            .sample_inelastic_mu_cm(MT_51, PROBE_EV, &mut s)
            .is_some(),
        "with_isotropic_elastic_scattering also cleared the INELASTIC levels; the elastic \
         ablation's +10511 pcm would silently include the inelastic share"
    );
}
