//! Unit tests for `heatr` (moved verbatim out of `heatr/mod.rs` on split).

use super::damage::*;
use super::kerma::*;
use super::spectra::*;
use crate::endf::MtReaction;
use crate::nuclear_data::secondary::{FissionSpectrum, NuBar};
use crate::reconr::ReconrResult;
use crate::reconr::{MaterialInfo, ReconrSection};

fn material(awr: f64) -> MaterialInfo {
    MaterialInfo {
        za: 1001.0,
        awr,
        lrp: 0,
        lfi: 0,
        nlib: 0,
        elis: 0.0,
        nfor: 6,
        emax: 2.0e7,
    }
}

fn recon(awr: f64, sections: Vec<ReconrSection>) -> ReconrResult {
    ReconrResult {
        material: material(awr),
        sections,
        resonance_upper_limit: None,
    }
}

/// Hydrogen (A=1) elastic scattering loses on average exactly half the
/// neutron's energy per collision — the textbook check for this formula.
#[test]
fn hydrogen_elastic_loses_half_its_energy() {
    let sec = ReconrSection {
        mt: MtReaction::Mt2Elastic,
        qi: 0.0,
        pairs: vec![(1.0e5, 20.0), (1.0e6, 20.0), (1.0e7, 20.0)], // flat 20 b
    };
    let kerma = Kerma::from_reconr(
        &recon(1.0, vec![sec]),
        &NuBar::default(),
        &FissionSpectrum::default(),
        &[],
    );
    for &e in &[1.0e5, 1.0e6, 1.0e7] {
        let h = kerma.eval(e);
        let expected = 20.0 * e * 0.5; // σ·E/2
        assert!(
            (h - expected).abs() / expected < 1.0e-9,
            "H({e})={h}, want {expected}"
        );
    }
}

/// A heavy target (A≫1) transfers only a small fraction of the incident
/// energy per elastic collision — `H(E)/(:σE) → 2/A` as `A → ∞`.
#[test]
fn heavy_target_transfers_small_fraction() {
    let sec = ReconrSection {
        mt: MtReaction::Mt2Elastic,
        qi: 0.0,
        pairs: vec![(1.0e6, 5.0), (2.0e6, 5.0)],
    };
    let awr = 238.0;
    let kerma = Kerma::from_reconr(
        &recon(awr, vec![sec]),
        &NuBar::default(),
        &FissionSpectrum::default(),
        &[],
    );
    let h = kerma.eval(1.0e6);
    let sigma_e = 5.0 * 1.0e6;
    let frac = h / sigma_e;
    let approx_2_over_a = 2.0 / awr;
    assert!(
        frac > 0.0 && frac < 0.01,
        "heavy-target fraction {frac} should be small"
    );
    assert!(
        (frac - approx_2_over_a).abs() / approx_2_over_a < 0.02,
        "frac={frac} ≈ 2/A={approx_2_over_a}"
    );
}

/// **H5, no-spectrum guard.** A multi-neutron reaction (MT=16 (n,2n)) with no
/// emission spectrum supplied contributes nothing — H5 has no closed-form
/// mean, so without the secondary spectrum the dispatch excludes the section
/// entirely (empty grid) rather than guessing an energy loss. Passing the
/// spectrum (see [`n2n_heating_subtracts_two_neutron_means`]) turns it on.
#[test]
fn multineutron_without_spectrum_contributes_nothing() {
    let sec = ReconrSection {
        mt: MtReaction::Mt16N2n,
        qi: -8.0e6,
        pairs: vec![(1.0e5, 1.0), (1.0e6, 1.0)],
    };
    let kerma = Kerma::from_reconr(
        &recon(56.0, vec![sec]),
        &NuBar::default(),
        &FissionSpectrum::default(),
        &[], // no emission spectrum for MT=16
    );
    assert!(
        kerma.energy.is_empty(),
        "(n,2n) with no supplied spectrum ⇒ empty grid"
    );
    assert_eq!(kerma.eval(5.0e5), 0.0);
}

/// **H2.** Radiative capture (MT=102) deposits all of `E+Q` locally — no
/// escaping neutron, so nothing is subtracted from the energy balance.
#[test]
fn capture_deposits_e_plus_q() {
    let q = 6.0e6; // representative (n,γ) Q-value
    let sec = ReconrSection {
        mt: MtReaction::Mt102Capture,
        qi: q,
        pairs: vec![(1.0e5, 2.0), (1.0e6, 2.0)],
    };
    let kerma = Kerma::from_reconr(
        &recon(56.0, vec![sec]),
        &NuBar::default(),
        &FissionSpectrum::default(),
        &[],
    );
    for &e in &[1.0e5, 1.0e6] {
        let h = kerma.eval(e);
        let expected = 2.0 * (e + q);
        assert!(
            (h - expected).abs() / expected < 1.0e-9,
            "H({e})={h}, want {expected}"
        );
    }
}

/// **H2.** A charged-particle-only exit, e.g. MT=107 `(n,α)`, uses the same
/// `E+Q` local-deposition formula as capture — no escaping neutron.
#[test]
fn charged_particle_only_exit_deposits_e_plus_q() {
    let q = 2.0e6;
    let sec = ReconrSection {
        mt: MtReaction::Mt107NAlpha,
        qi: q,
        pairs: vec![(1.0e6, 0.5)],
    };
    let kerma = Kerma::from_reconr(
        &recon(27.0, vec![sec]),
        &NuBar::default(),
        &FissionSpectrum::default(),
        &[],
    );
    let h = kerma.eval(1.0e6);
    let expected = 0.5 * (1.0e6 + q);
    assert!(
        (h - expected).abs() / expected < 1.0e-9,
        "H={h}, want {expected}"
    );
}

/// **H3, `Q=0` reduction.** A discrete "level" with `qi=0.0` (unphysical for
/// a real level, but a clean check) must reproduce H1's pure elastic
/// formula exactly — confirming the two-body-with-Q formula's `Q→0` limit.
#[test]
fn single_neutron_at_q_zero_matches_elastic_formula() {
    let awr = 12.0;
    let level = ReconrSection {
        mt: MtReaction::from_any(52),
        qi: 0.0,
        pairs: vec![(1.0e6, 3.0)],
    };
    let kerma = Kerma::from_reconr(
        &recon(awr, vec![level]),
        &NuBar::default(),
        &FissionSpectrum::default(),
        &[],
    );
    let h = kerma.eval(1.0e6);
    let expected = 3.0 * 1.0e6 * 2.0 * awr / (awr + 1.0).powi(2);
    assert!(
        (h - expected).abs() / expected < 1.0e-9,
        "H={h}, want {expected}"
    );
}

/// **H3.** A discrete inelastic level with a real (negative) Q-value: the
/// per-event heating `H(E)/σ(E) = E·2A/(A+1)² + Q/(A+1)` is *less* than the
/// pure-elastic term because the excitation energy `|Q|` is carried away by
/// the residual nucleus's internal state — consistent with `E+Q < E`
/// available for redistribution.
#[test]
fn discrete_level_heating_is_reduced_by_negative_q() {
    let awr = 12.0;
    let q = -4.4e6; // e.g. carbon-12's first excited state
    let level = ReconrSection {
        mt: MtReaction::from_any(51),
        qi: q,
        pairs: vec![(1.0e7, 1.0)],
    };
    let kerma = Kerma::from_reconr(
        &recon(awr, vec![level]),
        &NuBar::default(),
        &FissionSpectrum::default(),
        &[],
    );
    let h = kerma.eval(1.0e7);
    let expected = 1.0 * (1.0e7 * 2.0 * awr / (awr + 1.0).powi(2) + q / (awr + 1.0));
    assert!(
        (h - expected).abs() / expected.abs() < 1.0e-9,
        "H={h}, want {expected}"
    );
    let h_elastic_only = 1.0 * 1.0e7 * 2.0 * awr / (awr + 1.0).powi(2);
    assert!(
        h < h_elastic_only,
        "negative Q must reduce heating below the Q=0 term"
    );
}

/// **H3.** An `(n,n'α)`-family reaction (MT=22): single escaping neutron,
/// the alpha stays local — same formula as a discrete level, using MT=22's
/// own Q-value.
#[test]
fn nn_alpha_family_uses_single_neutron_formula() {
    let awr = 27.0;
    let q = -3.0e6;
    let sec = ReconrSection {
        mt: MtReaction::Mt22NnAlpha,
        qi: q,
        pairs: vec![(8.0e6, 0.2)],
    };
    let kerma = Kerma::from_reconr(
        &recon(awr, vec![sec]),
        &NuBar::default(),
        &FissionSpectrum::default(),
        &[],
    );
    let h = kerma.eval(8.0e6);
    let expected = 0.2 * (8.0e6 * 2.0 * awr / (awr + 1.0).powi(2) + q / (awr + 1.0));
    assert!(
        (h - expected).abs() / expected.abs() < 1.0e-9,
        "H={h}, want {expected}"
    );
}

/// H1+H2+H3 all sum additively on the shared union grid.
#[test]
fn all_three_phases_sum_additively() {
    let awr = 56.0;
    let elastic = ReconrSection {
        mt: MtReaction::Mt2Elastic,
        qi: 0.0,
        pairs: vec![(1.0e6, 10.0)],
    };
    let capture = ReconrSection {
        mt: MtReaction::Mt102Capture,
        qi: 6.0e6,
        pairs: vec![(1.0e6, 1.0)],
    };
    let level = ReconrSection {
        mt: MtReaction::from_any(51),
        qi: -1.0e6,
        pairs: vec![(1.0e6, 0.3)],
    };
    let kerma = Kerma::from_reconr(
        &recon(awr, vec![elastic, capture, level]),
        &NuBar::default(),
        &FissionSpectrum::default(),
        &[],
    );
    let h = kerma.eval(1.0e6);
    let f = 2.0 * awr / (awr + 1.0).powi(2);
    let h_elastic = 10.0 * 1.0e6 * f;
    let h_capture = 1.0 * (1.0e6 + 6.0e6);
    let h_level = 0.3 * (1.0e6 * f + (-1.0e6) / (awr + 1.0));
    let expected = h_elastic + h_capture + h_level;
    assert!(
        (h - expected).abs() / expected < 1.0e-9,
        "got {h}, want {expected}"
    );
}

/// H1 and H2 sum additively on the same union grid — a material with both
/// elastic and capture data gets both contributions at each energy.
#[test]
fn elastic_and_capture_sum_additively() {
    let elastic = ReconrSection {
        mt: MtReaction::Mt2Elastic,
        qi: 0.0,
        pairs: vec![(1.0e6, 10.0)],
    };
    let capture = ReconrSection {
        mt: MtReaction::Mt102Capture,
        qi: 6.0e6,
        pairs: vec![(1.0e6, 1.0)],
    };
    let awr = 56.0;
    let kerma = Kerma::from_reconr(
        &recon(awr, vec![elastic, capture]),
        &NuBar::default(),
        &FissionSpectrum::default(),
        &[],
    );
    let h = kerma.eval(1.0e6);
    let h_elastic = 10.0 * 1.0e6 * 2.0 * awr / (awr + 1.0).powi(2);
    let h_capture = 1.0 * (1.0e6 + 6.0e6);
    assert!(
        (h - (h_elastic + h_capture)).abs() < 1.0,
        "got {h}, want {}",
        h_elastic + h_capture
    );
}

/// Union grid + summation across two elastic-tagged sections at different
/// energy grids (only one physical elastic section normally exists per
/// material, but the union-grid machinery must still be exercised).
#[test]
fn eval_interpolates_between_grid_points() {
    let sec = ReconrSection {
        mt: MtReaction::Mt2Elastic,
        qi: 0.0,
        pairs: vec![(1.0e6, 10.0), (2.0e6, 10.0)],
    };
    let kerma = Kerma::from_reconr(
        &recon(1.0, vec![sec]),
        &NuBar::default(),
        &FissionSpectrum::default(),
        &[],
    );
    let h_mid = kerma.eval(1.5e6);
    // σ is flat at 10 b, so H(E) = 10*E/2 exactly (linear in E), and lin-lin
    // interpolation between the two grid points must land on that line.
    assert!((h_mid - 10.0 * 1.5e6 * 0.5).abs() < 1.0, "got {h_mid}");
}

/// **H4.** Fission heating matches `σ_f·[E + Q_fission − ν̄·⟨E'⟩]` exactly for
/// a constant ν̄ and the fixed-parameter Watt χ (closed-form mean).
#[test]
fn fission_heating_matches_energy_balance_formula() {
    let q_fission = 200.0e6; // representative U-235 fission Q-value
    let sigma_f = 2.0; // barn, flat for simplicity
    let sec = ReconrSection {
        mt: MtReaction::Mt18Fission,
        qi: q_fission,
        pairs: vec![(1.0e5, sigma_f), (1.0e6, sigma_f)],
    };
    let nu = NuBar {
        energy: vec![1.0e5, 1.0e6],
        nu_total: vec![2.4, 2.6],
    };
    let chi = FissionSpectrum::Watt {
        a: 0.988e6,
        b: 2.249e-6,
    };
    let kerma = Kerma::from_reconr(&recon(235.0, vec![sec]), &nu, &chi, &[]);

    let mean_e_prime = 1.5 * 0.988e6 + 0.25 * 0.988e6 * 0.988e6 * 2.249e-6;
    for &e in &[1.0e5, 1.0e6] {
        let h = kerma.eval(e);
        let nu_at_e = nu.at(e);
        let expected = sigma_f * (e + q_fission - nu_at_e * mean_e_prime);
        assert!(
            (h - expected).abs() / expected.abs() < 1.0e-9,
            "H({e})={h}, want {expected}"
        );
    }
}

/// **H4.** Fission heating is *positive and order-190 MeV/fission* for a
/// realistic case (Q≫ν̄·⟨E'⟩ at typical incident energies) — a basic
/// physical sanity bound distinct from the exact-formula check above
/// (catches a sign error that the exact check, using the same formula,
/// structurally cannot).
#[test]
fn fission_heating_is_positive_and_order_200_mev() {
    let q_fission = 200.0e6;
    let sigma_f = 1.5;
    let sec = ReconrSection {
        mt: MtReaction::Mt18Fission,
        qi: q_fission,
        pairs: vec![(1.0e5, sigma_f), (1.0e6, sigma_f)],
    };
    let nu = NuBar {
        energy: vec![1.0e5, 1.0e6],
        nu_total: vec![2.44, 2.44],
    };
    let chi = FissionSpectrum::default(); // thermal-Watt stand-in
    let kerma = Kerma::from_reconr(&recon(235.0, vec![sec]), &nu, &chi, &[]);
    let h = kerma.eval(1.0e5);
    let heating_number_ev = h / sigma_f; // H(E)/σ(E), eV per fission
    assert!(
        heating_number_ev > 0.0,
        "fission heating must be positive, got {heating_number_ev}"
    );
    // ~200 MeV per fission minus a few MeV carried off by ~2.4 fission
    // neutrons at ~2 MeV each — should land well within [150, 200] MeV.
    assert!(
        (150.0e6..200.0e6).contains(&heating_number_ev),
        "heating number {heating_number_ev} eV should be ~190 MeV/fission"
    );
}

/// H1+H2+H3+H4 all sum additively on the shared union grid — the full
/// tonight-scoped kinematic-limit KERMA.
#[test]
fn all_four_phases_sum_additively() {
    let awr = 235.0;
    let elastic = ReconrSection {
        mt: MtReaction::Mt2Elastic,
        qi: 0.0,
        pairs: vec![(1.0e6, 10.0)],
    };
    let capture = ReconrSection {
        mt: MtReaction::Mt102Capture,
        qi: 6.0e6,
        pairs: vec![(1.0e6, 1.0)],
    };
    let level = ReconrSection {
        mt: MtReaction::from_any(51),
        qi: -1.0e6,
        pairs: vec![(1.0e6, 0.3)],
    };
    let fission = ReconrSection {
        mt: MtReaction::Mt18Fission,
        qi: 200.0e6,
        pairs: vec![(1.0e6, 1.2)],
    };
    let nu = NuBar {
        energy: vec![1.0e6],
        nu_total: vec![2.5],
    };
    let chi = FissionSpectrum::Watt {
        a: 0.988e6,
        b: 2.249e-6,
    };
    let kerma = Kerma::from_reconr(
        &recon(awr, vec![elastic, capture, level, fission]),
        &nu,
        &chi,
        &[],
    );
    let h = kerma.eval(1.0e6);

    let f = 2.0 * awr / (awr + 1.0).powi(2);
    let h_elastic = 10.0 * 1.0e6 * f;
    let h_capture = 1.0 * (1.0e6 + 6.0e6);
    let h_level = 0.3 * (1.0e6 * f + (-1.0e6) / (awr + 1.0));
    let mean_e_prime = 1.5 * 0.988e6 + 0.25 * 0.988e6 * 0.988e6 * 2.249e-6;
    let h_fission = 1.2 * (1.0e6 + 200.0e6 - 2.5 * mean_e_prime);
    let expected = h_elastic + h_capture + h_level + h_fission;
    assert!(
        (h - expected).abs() / expected < 1.0e-9,
        "got {h}, want {expected}"
    );
}

/// **H5.** An (n,2n) reaction (MT=16) subtracts the mean energy of *two*
/// escaping neutrons from the `E + Q` energy balance: with a fixed-parameter
/// Watt emission spectrum (closed-form mean `1.5a + ¼a²b`) the heating is
/// `σ·(E + Q − 2·⟨E'⟩)` exactly.
#[test]
fn n2n_heating_subtracts_two_neutron_means() {
    let q = -6.0e6; // representative (n,2n) threshold Q
    let sigma = 0.7; // barn, flat
    let sec = ReconrSection {
        mt: MtReaction::Mt16N2n,
        qi: q,
        pairs: vec![(1.0e7, sigma), (1.4e7, sigma)],
    };
    // Reuse the general MF=5 machinery: a Watt-shaped emitted-neutron spectrum.
    let (a, b) = (0.5e6, 4.0e-6);
    let spectrum = FissionSpectrum::Watt { a, b };
    let mean_e_prime = 1.5 * a + 0.25 * a * a * b;
    let kerma = Kerma::from_reconr(
        &recon(56.0, vec![sec]),
        &NuBar::default(),
        &FissionSpectrum::default(),
        &[(MtReaction::Mt16N2n, EmissionSpectrum::Mf5(spectrum))],
    );
    for &e in &[1.0e7, 1.4e7] {
        let h = kerma.eval(e);
        let expected = sigma * (e + q - 2.0 * mean_e_prime);
        assert!(
            (h - expected).abs() / expected.abs() < 1.0e-9,
            "H({e})={h}, want {expected}"
        );
    }
}

/// **H5, multiplicity.** (n,3n) (MT=17) removes *three* neutron means, (n,4n)
/// (MT=37) *four* — the multiplicity comes from the MT, so with the same
/// emission spectrum the (n,3n) heating is lower than (n,2n)'s by exactly one
/// extra `σ·⟨E'⟩`.
#[test]
fn n3n_and_n4n_use_higher_multiplicity() {
    let q = -12.0e6;
    let sigma = 0.4;
    let (a, b) = (0.6e6, 3.0e-6);
    let mean_e_prime = 1.5 * a + 0.25 * a * a * b;
    let e = 2.0e7;
    for (mt, yld) in [(MtReaction::Mt17N3n, 3.0), (MtReaction::Mt37N4n, 4.0)] {
        let sec = ReconrSection {
            mt,
            qi: q,
            pairs: vec![(e, sigma)],
        };
        let kerma = Kerma::from_reconr(
            &recon(90.0, vec![sec]),
            &NuBar::default(),
            &FissionSpectrum::default(),
            &[(mt, EmissionSpectrum::Mf5(FissionSpectrum::Watt { a, b }))],
        );
        let expected = sigma * (e + q - yld * mean_e_prime);
        let h = kerma.eval(e);
        assert!(
            (h - expected).abs() / expected.abs() < 1.0e-9,
            "MT={:?}: H={h}, want {expected}",
            mt
        );
    }
}

/// **H5.** Continuum inelastic (MT=91) is the yield-1 member of the family:
/// a single escaping neutron carrying the emission spectrum's mean, using the
/// section's own Q — the continuum analogue of a discrete level (H3), but with
/// the *spectrum* mean instead of the two-body kinematic mean.
#[test]
fn continuum_inelastic_is_single_neutron_with_spectrum_mean() {
    let q = -2.0e6;
    let sigma = 1.1;
    let (a, b) = (0.8e6, 2.0e-6);
    let mean_e_prime = 1.5 * a + 0.25 * a * a * b;
    let e = 5.0e6;
    let sec = ReconrSection {
        mt: MtReaction::Mt91NnContinuum,
        qi: q,
        pairs: vec![(e, sigma)],
    };
    let kerma = Kerma::from_reconr(
        &recon(28.0, vec![sec]),
        &NuBar::default(),
        &FissionSpectrum::default(),
        &[(
            MtReaction::Mt91NnContinuum,
            EmissionSpectrum::Mf5(FissionSpectrum::Watt { a, b }),
        )],
    );
    let h = kerma.eval(e);
    let expected = sigma * (e + q - 1.0 * mean_e_prime);
    assert!(
        (h - expected).abs() / expected.abs() < 1.0e-9,
        "H={h}, want {expected}"
    );
}

/// **H5, physical sanity.** A 14-MeV (n,2n) on a mid-mass nucleus deposits a
/// small *positive* heating: after the ~8 MeV threshold Q and two ~1–2 MeV
/// neutrons escape, only a few MeV of recoil + charged-particle energy is
/// left local — distinct from the exact-formula check (catches a sign or
/// multiplicity blunder the algebra-mirroring check structurally cannot).
#[test]
fn n2n_heating_is_small_and_positive_at_14_mev() {
    let q = -8.0e6;
    let sigma = 0.5;
    let e = 1.4e7;
    let sec = ReconrSection {
        mt: MtReaction::Mt16N2n,
        qi: q,
        pairs: vec![(e, sigma)],
    };
    // Evaporation-like emitted neutrons, ~1.5 MeV mean (θ ≈ 0.75 MeV, ⟨E'⟩=2θ).
    let spectrum = FissionSpectrum::Watt {
        a: 0.5e6,
        b: 3.0e-6,
    };
    let kerma = Kerma::from_reconr(
        &recon(56.0, vec![sec]),
        &NuBar::default(),
        &FissionSpectrum::default(),
        &[(MtReaction::Mt16N2n, EmissionSpectrum::Mf5(spectrum))],
    );
    let heating_number_ev = kerma.eval(e) / sigma; // H/σ, eV per event
    assert!(
        heating_number_ev > 0.0,
        "(n,2n) heating must be positive, got {heating_number_ev}"
    );
    assert!(
        heating_number_ev < e + q,
        "escaping neutrons must reduce heating below E+Q={}",
        e + q
    );
    assert!(
        (0.0..5.0e6).contains(&heating_number_ev),
        "14-MeV (n,2n) heating {heating_number_ev} eV should be a few MeV"
    );
}

/// **H5, additive with H1–H4.** A section list mixing elastic, capture, a
/// discrete level, fission, and an (n,2n) sums all five models on the shared
/// union grid.
#[test]
fn h5_sums_additively_with_all_prior_phases() {
    let awr = 235.0;
    let e = 1.4e7;
    let elastic = ReconrSection {
        mt: MtReaction::Mt2Elastic,
        qi: 0.0,
        pairs: vec![(e, 5.0)],
    };
    let capture = ReconrSection {
        mt: MtReaction::Mt102Capture,
        qi: 6.0e6,
        pairs: vec![(e, 0.1)],
    };
    let level = ReconrSection {
        mt: MtReaction::from_any(51),
        qi: -1.0e6,
        pairs: vec![(e, 0.2)],
    };
    let fission = ReconrSection {
        mt: MtReaction::Mt18Fission,
        qi: 200.0e6,
        pairs: vec![(e, 1.0)],
    };
    let n2n = ReconrSection {
        mt: MtReaction::Mt16N2n,
        qi: -8.0e6,
        pairs: vec![(e, 0.6)],
    };
    let nu = NuBar {
        energy: vec![e],
        nu_total: vec![2.6],
    };
    let chi = FissionSpectrum::Watt {
        a: 0.988e6,
        b: 2.249e-6,
    };
    let (a, b) = (0.5e6, 4.0e-6);
    let kerma = Kerma::from_reconr(
        &recon(awr, vec![elastic, capture, level, fission, n2n]),
        &nu,
        &chi,
        &[(
            MtReaction::Mt16N2n,
            EmissionSpectrum::Mf5(FissionSpectrum::Watt { a, b }),
        )],
    );
    let h = kerma.eval(e);

    let f = 2.0 * awr / (awr + 1.0).powi(2);
    let mean_chi = 1.5 * 0.988e6 + 0.25 * 0.988e6 * 0.988e6 * 2.249e-6;
    let mean_n2n = 1.5 * a + 0.25 * a * a * b;
    let expected = 5.0 * e * f
        + 0.1 * (e + 6.0e6)
        + 0.2 * (e * f + (-1.0e6) / (awr + 1.0))
        + 1.0 * (e + 200.0e6 - 2.6 * mean_chi)
        + 0.6 * (e - 8.0e6 - 2.0 * mean_n2n);
    assert!(
        (h - expected).abs() / expected.abs() < 1.0e-9,
        "got {h}, want {expected}"
    );
}

// ── H7: damage energy (MT=444) ─────────────────────────────────────────

/// **H7.** The default displacement-threshold table matches NJOY's built-in
/// values (C=31, Al=27, Fe=40, W=90, Pb=25, and the 25 eV fallback).
#[test]
fn default_displacement_energies_match_njoy_table() {
    assert_eq!(default_displacement_energy(6), 31.0); // carbon
    assert_eq!(default_displacement_energy(13), 27.0); // aluminium
    assert_eq!(default_displacement_energy(26), 40.0); // iron
    assert_eq!(default_displacement_energy(74), 90.0); // tungsten
    assert_eq!(default_displacement_energy(82), 25.0); // lead
    assert_eq!(default_displacement_energy(1), 25.0); // fallback
}

/// **H7.** The Lindhard partition is 0 below the displacement threshold, and
/// above it never exceeds the recoil energy (electronic losses only ever
/// *remove* energy from the displacement channel).
#[test]
fn lindhard_partition_bounds() {
    let (z, a, e_d) = (26.0, 56.0, 40.0); // iron
    assert_eq!(lindhard_damage(30.0, z, a, z, a, e_d), 0.0, "below E_d ⇒ 0");
    for &er in &[50.0, 1.0e3, 1.0e5, 1.0e7] {
        let df = lindhard_damage(er, z, a, z, a, e_d);
        assert!(df > 0.0 && df <= er, "0 < df({er})={df} ≤ E_R");
    }
}

/// **H7.** At low recoil energy (just above threshold) almost all energy
/// stays in the displacement channel (`df ≈ E_R`); at very high recoil energy
/// electronic stopping dominates and the damage fraction collapses
/// (`df/E_R ≪ 1`) — the qualitative signature of the Lindhard partition.
#[test]
fn lindhard_damage_fraction_falls_with_energy() {
    let (z, a, e_d) = (26.0, 56.0, 40.0);
    let low = lindhard_damage(100.0, z, a, z, a, e_d) / 100.0;
    let high = lindhard_damage(1.0e7, z, a, z, a, e_d) / 1.0e7;
    assert!(low > 0.7, "near threshold df/E_R={low} should be ≈1");
    assert!(high < 0.1, "at 10 MeV df/E_R={high} should be ≪1");
    assert!(low > high, "damage fraction must fall with recoil energy");
}

/// **H7.** Elastic damage cross section: zero at incident energies so low the
/// maximum recoil `E_max = 4A/(A+1)²·E` cannot reach `E_d`, positive above,
/// and monotonically increasing — and the per-collision damage energy never
/// exceeds the per-collision *heating* (H1), since displacement energy is a
/// partition of the recoil energy that heating already counts in full.
#[test]
fn elastic_damage_is_bounded_by_heating() {
    let awr = 56.0; // iron
    let z = 26;
    let e_d = default_displacement_energy(z);
    let sigma = 3.0; // barn, flat
    let sec = ReconrSection {
        mt: MtReaction::Mt2Elastic,
        qi: 0.0,
        pairs: vec![
            (1.0e2, sigma),
            (1.0e5, sigma),
            (1.0e6, sigma),
            (1.4e7, sigma),
        ],
    };
    let dmg = DamageEnergy::from_reconr(&recon(awr, vec![sec]), z, e_d);

    // E_max at 100 eV: 4·56/57²·100 ≈ 6.9 eV < E_d=40 ⇒ no damage.
    assert_eq!(dmg.eval(1.0e2), 0.0, "below-threshold recoil ⇒ zero damage");

    let f = 2.0 * awr / (awr + 1.0).powi(2); // H1 heating factor
    let mut prev = 0.0;
    for &e in &[1.0e5, 1.0e6, 1.4e7] {
        let d = dmg.eval(e);
        let heating = sigma * e * f; // H1 elastic heating (eV·barn)
        assert!(d > 0.0, "damage must be positive at {e} eV, got {d}");
        assert!(
            d < heating,
            "damage {d} must be < heating {heating} at {e} eV"
        );
        assert!(d > prev, "damage must increase with energy ({e} eV)");
        prev = d;
    }
}

/// **H7.** A material with no elastic section yields an empty MT=444 table
/// (only the two-body recoil channels are ported so far).
#[test]
fn damage_empty_without_recoil_channels() {
    // Pure capture (MT=102): no two-body neutron-scattering recoil channel.
    let sec = ReconrSection {
        mt: MtReaction::Mt102Capture,
        qi: 6.0e6,
        pairs: vec![(1.0e5, 2.0), (1.0e6, 2.0)],
    };
    let dmg = DamageEnergy::from_reconr(&recon(56.0, vec![sec]), 26, 40.0);
    assert!(dmg.energy.is_empty());
    assert_eq!(dmg.eval(5.0e5), 0.0);
}

/// **H7, recoil bounds.** Elastic (`Q=0`) recoil spans `[0, 4C]`; a discrete
/// level (`Q<0`) narrows the window to `[C(1−g)², C(1+g)²]` with `g<1`, so its
/// maximum recoil is strictly below the elastic backscatter maximum — the
/// residual nucleus keeps `|Q|` as excitation, leaving less for recoil.
#[test]
fn two_body_recoil_bounds_narrow_with_negative_q() {
    let awr = 56.0_f64;
    let e = 5.0e6_f64;
    let c = awr / (awr + 1.0).powi(2) * e;
    let (el_min, el_max) = two_body_recoil_bounds(e, 0.0, awr);
    assert!((el_min).abs() < 1.0, "elastic E_min≈0, got {el_min}");
    assert!(
        (el_max - 4.0 * c).abs() / (4.0 * c) < 1e-12,
        "elastic E_max=4C"
    );

    let (lv_min, lv_max) = two_body_recoil_bounds(e, -2.0e6, awr);
    assert!(lv_min > 0.0, "level E_min>0, got {lv_min}");
    assert!(lv_max < el_max, "level E_max {lv_max} < elastic {el_max}");
}

/// **H7.** A discrete inelastic level (MT=51) produces a positive damage
/// cross section above its threshold, and it sums additively with elastic on
/// the shared union grid — the total MT=444 exceeds the elastic-only part.
#[test]
fn discrete_level_adds_to_damage() {
    let awr = 56.0; // iron
    let z = 26;
    let e_d = default_displacement_energy(z);
    let elastic = ReconrSection {
        mt: MtReaction::Mt2Elastic,
        qi: 0.0,
        pairs: vec![(1.0e6, 3.0), (5.0e6, 3.0)],
    };
    let level = ReconrSection {
        mt: MtReaction::from_any(51),
        qi: -0.85e6, // Fe-56 first level ~0.85 MeV
        pairs: vec![(1.0e6, 1.0), (5.0e6, 1.0)],
    };
    let elastic_only = DamageEnergy::from_reconr(&recon(awr, vec![elastic]), z, e_d);
    let both = DamageEnergy::from_reconr(
        &recon(
            awr,
            vec![
                ReconrSection {
                    mt: MtReaction::Mt2Elastic,
                    qi: 0.0,
                    pairs: vec![(1.0e6, 3.0), (5.0e6, 3.0)],
                },
                level,
            ],
        ),
        z,
        e_d,
    );
    let e = 5.0e6; // well above the ~0.85 MeV level threshold
    let d_el = elastic_only.eval(e);
    let d_both = both.eval(e);
    assert!(d_el > 0.0, "elastic damage positive, got {d_el}");
    assert!(
        d_both > d_el,
        "discrete level must add damage: both={d_both} > elastic-only={d_el}"
    );
}
