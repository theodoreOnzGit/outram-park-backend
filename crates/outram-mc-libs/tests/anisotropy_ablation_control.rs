//! **Regression gate for the anisotropy ablation control itself.**
//!
//! `examples/godiva_anisotropy_ablation.rs` prices elastic angular anisotropy at
//! **+10511 ± 67 pcm** on Godiva by running the case twice, once with the
//! evaluated ENDF MF=4 cosine and once with
//! [`Nuclide::with_isotropic_elastic_scattering`]. That measurement is only
//! meaningful if the ablation actually ablates — a silently no-op control would
//! report "no difference" and be read as "anisotropy does not matter", which is
//! the worst possible failure mode for an ablation study.
//!
//! The full ablation is ~35 minutes and belongs in an example. This is the cheap
//! part that belongs in the suite: that the control does what it claims, on real
//! evaluated data, in a few seconds.
//!
//! # Methodology
//!
//! U-234 (ENDF/B-VIII.0, the fastest of the three ICSBEP nuclides to
//! reconstruct) is built HIGH-tier, then ablated. At 2 MeV — inside Godiva's
//! flux — assert that:
//!
//! 1. the evaluated nuclide returns an anisotropic CM cosine **every** time;
//! 2. the ablated nuclide returns `None` every time, so the transport kernel
//!    falls back to isotropic CM through its existing path;
//! 3. the evaluated distribution is **genuinely anisotropic** (`|⟨μ⟩|` well
//!    above zero), so there was something to remove;
//! 4. everything else about the nuclide is untouched — σ_t, σ_f and ν̄σ_f are
//!    bit-identical before and after, so the ablation changes the angular
//!    distribution and nothing else. Without this the measured worth could be
//!    contaminated by a cross-section change.
//!
//! # Results (2026-09-15)
//!
//! U-234 at 2 MeV: evaluated returns a cosine 512/512 times, ablated 0/512;
//! cross sections bit-identical across the ablation.

use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use outram_mc_libs::material::nuclide::Nuclide;

const TEMP_K: f64 = 293.6;
/// Inside Godiva's flux, and well above any resonance structure.
const PROBE_EV: f64 = 2.0e6;
const DRAWS: usize = 512;

#[test]
fn ablation_removes_the_evaluated_cosine_and_nothing_else() {
    let Some(tape) = reference_file_or_skip(
        "endf",
        "n-092_U_234-ENDF8.0.endf",
        "U-234 evaluation (anisotropy ablation control)",
    ) else {
        return;
    };

    let evaluated = Nuclide::from_endf_file(&tape, "U234", TEMP_K, 1.0e-3)
        .expect("U-234 reconstructs");
    let ablated = evaluated.clone().with_isotropic_elastic_scattering();

    // 1 + 2: the control actually controls.
    let (mut s1, mut s2) = (0xC0FFEEu64, 0xC0FFEEu64);
    let before = (0..DRAWS)
        .filter_map(|_| evaluated.sample_elastic_mu_cm(PROBE_EV, &mut s1))
        .collect::<Vec<_>>();
    let after = (0..DRAWS)
        .filter_map(|_| ablated.sample_elastic_mu_cm(PROBE_EV, &mut s2))
        .count();
    assert_eq!(
        before.len(),
        DRAWS,
        "the EVALUATED nuclide returned an anisotropic cosine only {}/{DRAWS} times at \
         {PROBE_EV:.1e} eV; the ablation study's baseline arm is not what it claims",
        before.len()
    );
    assert_eq!(
        after, 0,
        "with_isotropic_elastic_scattering left {after}/{DRAWS} evaluated cosines in place -- \
         the ablation is a no-op, and any 'anisotropy does not matter' conclusion drawn from \
         it would be an artefact"
    );

    // 3: there was something worth removing.
    let mubar = before.iter().sum::<f64>() / before.len() as f64;
    assert!(
        mubar.abs() > 0.05,
        "the evaluated cosine averages {mubar:.4} at {PROBE_EV:.1e} eV, which is near-isotropic \
         -- ablating it would measure nothing. U-234 elastic is forward-peaked at MeV energies; \
         if this fires, the MF=4 reading is suspect."
    );

    // 4: nothing else moved.
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
            "{name} changed across the ablation ({a:.9e} -> {b:.9e}). The control must alter \
             the ANGULAR distribution only; a cross-section change would contaminate the \
             measured worth of anisotropy."
        );
    }

    println!(
        "  U-234 @ {PROBE_EV:.1e} eV: evaluated {}/{DRAWS} cosines (mu-bar {mubar:+.4}), \
         ablated {after}/{DRAWS}; cross sections bit-identical",
        before.len()
    );
}
