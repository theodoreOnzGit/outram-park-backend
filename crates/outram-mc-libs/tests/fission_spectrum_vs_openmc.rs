//! **V&V: this crate's sampled fission spectrum against OpenMC's, on the same
//! evaluation — the check that clears χ of the Godiva *spectral* residual.**
//!
//! # Why the mean outgoing energy
//!
//! `outram-mc-libs` is `+250 ± 51 pcm` above OpenMC on Godiva. A `k_inf`
//! comparison splits that into ~181 pcm of leakage and **~69 ± 23 pcm that is
//! spectral** — it survives with the boundary removed, so it is secondary-energy
//! or reaction sampling, not geometry
//! (`verification_and_validation/openmc_godiva_cross_code/`).
//!
//! The fission spectrum is the largest single lever on that: every neutron is
//! born from it, and in a bare fast assembly a harder or softer χ shifts the
//! whole balance — U-238 threshold fission above ~1 MeV, and `ν̄(E)`, both climb
//! with energy. `⟨E_out⟩` is the one number that summarises it.
//!
//! # Methodology
//!
//! U-235 (94 % of Godiva), ENDF/B-VIII.0, HIGH tier. Sample
//! [`Nuclide::sample_fission_energy`] many times at three incident energies and
//! compare the mean against OpenMC 0.15.3's `⟨E_out⟩` for the same evaluation,
//! from ACE built by NJOY2016 `ac5adf5f` from the same tape.
//!
//! **The oracle is interpolated between OpenMC's tabulated incident energies.**
//! That grid is coarse — 22 points over `1e-5 … 3e7 eV`, the first step running
//! straight from `1e-5` to `5e5` — so taking the *nearest* tabulated point
//! instead makes χ look 0.5 % soft at 1 keV purely as an artefact of the
//! comparison. It was measured that way first; the difference between the two
//! readings is entirely in the oracle, not in this crate.
//!
//! # Results (2026-09-15)
//!
//! | incident E (eV) | this crate ⟨E_out⟩ | OpenMC ⟨E_out⟩ | relative |
//! |---|---|---|---|
//! | 1.0e3 | 1.99946e6 | 1.99982e6 | −0.018 % |
//! | 1.0e6 | 2.02443e6 | 2.02459e6 | −0.008 % |
//! | 2.0e6 | 2.05370e6 | 2.05387e6 | −0.008 % |
//!
//! **χ is therefore cleared**, and with it the largest spectral candidate. The
//! ~69 pcm spectral residual is not the fission spectrum; the remaining
//! candidates are the inelastic secondary energies — level selection and the
//! continuum law — which are not checked here.
//!
//! Together with `tests/elastic_mubar_vs_openmc.rs` (elastic `⟨μ⟩` to 5.6e-4)
//! and the cross-section comparison in the V&V study (every reaction to 0.06 %
//! flux-weighted), this leaves the Godiva discrepancy attributed to the
//! **inelastic** distributions on both axes: angular for the leakage share,
//! and — by elimination, not yet by measurement — secondary energy for the
//! spectral one.

use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use outram_mc_libs::material::nuclide::Nuclide;

const TEMP_K: f64 = 293.6;
const DRAWS: usize = 400_000;

/// `(incident energy [eV], OpenMC ⟨E_out⟩ [eV])`, interpolated in incident
/// energy — see "Methodology" for why interpolated and not nearest.
const OPENMC_MEAN_EOUT: &[(f64, f64)] =
    &[(1.0e3, 1.99982e6), (1.0e6, 2.02459e6), (2.0e6, 2.05387e6)];

/// Relative tolerance on `⟨E_out⟩`. The Monte Carlo error on 400 000 draws of a
/// Watt-like spectrum is ~0.1 %, so this is set by the sampling, not by any
/// physics allowance.
const TOL: f64 = 3.0e-3;

#[test]
fn fission_spectrum_mean_energy_matches_openmc() {
    let Some(tape) = reference_file_or_skip(
        "endf",
        "n-092_U_235-ENDF8.0.endf",
        "U-235 evaluation (fission spectrum vs OpenMC)",
    ) else {
        return;
    };
    let nuc = Nuclide::from_endf_file(&tape, "U235", TEMP_K, 1.0e-3).expect("U-235 reconstructs");

    println!(
        "{:>12} {:>14} {:>14} {:>11}",
        "Ein (eV)", "ours", "OpenMC", "rel diff"
    );
    for &(ein, theirs) in OPENMC_MEAN_EOUT {
        let mut seed = 7u64;
        let mut acc = 0.0_f64;
        for _ in 0..DRAWS {
            acc += nuc.sample_fission_energy(ein, &mut seed);
        }
        let ours = acc / DRAWS as f64;
        let rel = (ours - theirs) / theirs;
        println!("{ein:12.3e} {ours:14.5e} {theirs:14.5e} {rel:+11.3}%");
        assert!(
            rel.abs() <= TOL,
            "fission spectrum mean outgoing energy at Ein={ein:.3e} eV is {ours:.5e} eV here \
             against OpenMC's {theirs:.5e} eV ({:+.3}% relative, tolerance {TOL:.0e}).\n\
             chi sets every neutron's birth energy, so a real disagreement would shift the \
             whole spectrum and with it U-238 threshold fission and nu-bar(E). If this fires, \
             the ~69 pcm spectral residual recorded in verification_and_validation/\
             openmc_godiva_cross_code/ has a new and larger explanation.",
            rel * 100.0
        );
    }
    println!(
        "  chi agrees with OpenMC to better than {:.1}% at every point",
        TOL * 100.0
    );
}
