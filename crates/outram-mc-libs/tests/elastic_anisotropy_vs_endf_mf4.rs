//! **Elastic scattering is anisotropic in CM above a few hundred keV, and this
//! crate samples it — checked against the ENDF MF=4 data it samples from.**
//!
//! # Why this exists
//!
//! Two reasons, and the second is the one that made it urgent.
//!
//! **1. The band this crate verifies its slowing-down in is the band where
//! anisotropy does not exist.** `ring_rpt_hunt_lessons` and
//! `examples/epithermal_slowing_down.rs` establish `ξ/ξ₀ = 1.000` on eight
//! nuclides from 4 eV to 10 keV. Elastic scattering is isotropic in CM
//! throughout that band, so `ξ/ξ₀ = 1.000` there is a statement about the
//! two-body kinematics and says nothing whatever about the angular law. The
//! anisotropy that sets the slowing-down power *above* the resonances — and with
//! it the fast-fission factor ε — lives at MeV energies and was unverified.
//!
//! **2. A null result from switching a mechanism off is only readable if the
//! switch did something.** `examples/fhr_ring_rpt_endf.rs` prices anisotropy by
//! dropping every MF=4 distribution (`OUTRAM_RINGRPT_ISOTROPIC_ELASTIC=1`) and
//! measures −81 ± 317 pcm. That number means "anisotropy is worth nothing here"
//! only if anisotropy was there to begin with; if MF=4 had silently failed to
//! parse, the same measurement would read the same and mean the opposite. This
//! file is what makes that difference decidable.
//!
//! # The oracle
//!
//! [`Nuclide::sample_elastic_mu_cm`] locates an incident-energy bin, picks one of
//! the two bracketing tables by ACE statistical interpolation, and inverts that
//! table's cosine CDF. [`Nuclide::elastic_mubar_cm`] integrates the *same*
//! tabulated distribution directly. They must agree.
//!
//! This is deliberately an **internal-consistency** oracle, not a comparison
//! against another library, and it is stronger than it looks for the defect it
//! is aimed at: a sampler that quietly returns isotropic (empty table, failed
//! parse, a bad bracket search) disagrees with the quadrature immediately,
//! whereas a comparison against NJOY would need the whole reconstruction chain to
//! be running before it could say anything at all.
//!
//! # Measured (2026-09-12), ENDF/B-VIII.0, 200 000 samples per point
//!
//! The sampler reproduces its own MF=4 mean cosine to **0.0073 worst** over 24
//! points, and the moderators go from isotropic at 1 keV to μ̄ = +0.60…+0.73 at
//! 14 MeV. So the angular data is parsed, the sampler uses it, and the pebble's
//! −81 ± 317 pcm price for anisotropy is a measurement of a mechanism that was
//! present, not of a switch that did nothing.
//!
//! Full tables in the tests' own doc comments.
//!
//! Data-gated: skips (passes) when `reference-data/endf/` is absent, same
//! contract as `tests/thermal_laws_vs_njoy_thermr.rs`.

use outram_mc_libs::material::nuclide::Nuclide;

/// Temperature \[K\] for the reconstruction. Irrelevant to MF=4 — angular
/// distributions are not Doppler-broadened — but `from_endf_file` needs one, and
/// 600 K matches the FHR pebble this was written for.
const TEMP_K: f64 = 600.0;
/// Samples per (nuclide, energy) point. 200 000 puts the standard error of μ̄ at
/// under 0.002 for a distribution bounded on \[−1, 1\], which is an order of
/// magnitude below the tolerance.
const N_SAMPLES: usize = 200_000;

/// `Some(nuclide)` when the tape is present, else `None` after printing a skip.
fn nuclide_or_skip(file: &str, name: &str) -> Option<Nuclide> {
    let Some(path) = njoy_outram_park_fork::reference_data::reference_endf(file) else {
        println!("[{name}] SKIP: {file} not in reference-data/endf/");
        return None;
    };
    match Nuclide::from_endf_file(&path, name, TEMP_K, 1.0e-3) {
        Ok(n) => Some(n),
        Err(e) => {
            println!("[{name}] SKIP: {e:?}");
            None
        }
    }
}

/// Sampled ⟨μ_cm⟩ at incident energy `e` \[eV\]. A `None` from the sampler is
/// isotropic-CM, which contributes a mean of zero — counted as such rather than
/// skipped, because silently dropping the isotropic draws would hide exactly the
/// failure this file exists to catch.
fn sampled_mubar(n: &Nuclide, e: f64, seed: &mut u64) -> f64 {
    let mut sum = 0.0;
    for _ in 0..N_SAMPLES {
        sum += match n.sample_elastic_mu_cm(e, seed) {
            Some(mu) => mu,
            None => 2.0 * outram_mc_libs::rng::lcg::prn(seed) - 1.0,
        };
    }
    sum / N_SAMPLES as f64
}

/// The moderators of the FHR pebble, plus O-16 from the fuel kernel. Every one
/// of them is in `examples/fhr_ring_rpt_endf.rs`'s nuclide list, and the three
/// light ones carry ~90 % of the pebble's slowing-down power.
const MODERATORS: [(&str, &str); 4] = [
    ("n-006_C_012-ENDF8.0.endf", "C12"),
    ("n-004_Be_009-ENDF8.0.endf", "Be9"),
    ("n-009_F_019-ENDF8.0.endf", "F19"),
    ("n-008_O_016-ENDF8.0.endf", "O16"),
];

/// Incident energies \[eV\]: two decades below the onset of anisotropy, then
/// across it, then into the fission source.
const ENERGIES_EV: [f64; 6] = [1.0e3, 1.0e5, 5.0e5, 1.0e6, 5.0e6, 1.4e7];

/// **The elastic angular sampler reproduces the mean cosine of the tabulated
/// MF=4 distribution it samples from, at every energy, for every moderator in
/// the FHR pebble.**
///
/// # Methodology
///
/// `N_SAMPLES` draws from [`Nuclide::sample_elastic_mu_cm`] against
/// [`Nuclide::elastic_mubar_cm`]'s direct quadrature of the same tabulated
/// distribution, at `ENERGIES_EV`, for `MODERATORS`. An absolute tolerance is
/// used rather than a relative one because μ̄ passes through zero.
///
/// # Measured (2026-09-12), ENDF/B-VIII.0
///
/// Worst `|sampled − tabulated|` over 24 points (4 nuclides x 6 energies) is
/// **0.0073**, at O-16 / 500 keV (`+0.4170` sampled against `+0.4243`
/// tabulated). Everything else is at or under 0.0035, and eleven of the
/// twenty-four are under 0.001.
///
/// The one outlier sits where O-16's angular distribution changes fastest with
/// incident energy, which is where the two sides differ by construction rather
/// than by defect: the sampler picks one of the two bracketing tables with
/// probability `r` (ACE statistical interpolation), so it reproduces the linear
/// interpolation of μ̄ *in expectation*, while the quadrature evaluates that
/// interpolation exactly. The sampling standard error at `N_SAMPLES` is under
/// 0.002, so the outlier is a few sigma and worth re-examining if it grows —
/// which is why the tolerance is set just above it rather than comfortably
/// clear of it.
///
/// # Measured (2026-09-12)
///
/// Printed by the test; the worst absolute deviation is asserted below.
#[test]
fn the_elastic_angular_sampler_reproduces_the_tabulated_mean_cosine() {
    /// Worst allowed |sampled μ̄ − tabulated μ̄|. The sampling standard error is
    /// under 0.002 at `N_SAMPLES`, so this is ~5x it and fires on a real
    /// sampler defect, not on statistics.
    const MUBAR_ABS_TOL: f64 = 0.01;

    let mut worst = 0.0_f64;
    let mut worst_at = String::new();
    let mut any = false;
    for (file, name) in MODERATORS {
        let Some(n) = nuclide_or_skip(file, name) else {
            continue;
        };
        any = true;
        for e in ENERGIES_EV {
            let mut seed = 0xA17E_0001_u64 ^ (e as u64);
            let sampled = sampled_mubar(&n, e, &mut seed);
            let tabulated = n.elastic_mubar_cm(e);
            let d = (sampled - tabulated).abs();
            println!(
                "[{name}] E = {e:>9.3e} eV   sampled mu-bar {sampled:+.4}   \
                 MF=4 {tabulated:+.4}   diff {:+.4}",
                sampled - tabulated
            );
            if d > worst {
                worst = d;
                worst_at = format!("{name} at {e:.3e} eV ({sampled:+.4} vs {tabulated:+.4})");
            }
        }
    }
    if !any {
        return;
    }
    println!("worst |sampled - MF=4| = {worst:.4} at {worst_at}");
    assert!(
        worst < MUBAR_ABS_TOL,
        "the elastic angular sampler does not reproduce its own MF=4 data: \
         worst |sampled - tabulated| = {worst:.4} at {worst_at}, tolerance \
         {MUBAR_ABS_TOL}"
    );
}

/// **Elastic scattering off the pebble's moderators is isotropic in CM at 1 keV
/// and strongly forward-peaked at 14 MeV — so the data is there, and a run with
/// it switched off is a real experiment.**
///
/// # Why the first half matters as much as the second
///
/// `ξ/ξ₀ = 1.000` from 4 eV to 10 keV is only a correct verification of the
/// two-body kinematics if scattering really is isotropic there. The 1 keV row
/// asserts that, so the older result rests on a measurement rather than on an
/// assumption; and the 14 MeV rows assert that the same nuclides are *not*
/// isotropic where that check never reached.
///
/// # Why the forward gate is at 14 MeV and not at 1 MeV
///
/// Because it was written at 1 MeV first, and **O-16 failed it honestly**:
///
/// ```text
///   O-16 MF=4 mu-bar    1 keV   -0.0003
///                     100 keV   -0.0323   (backward)
///                     500 keV   +0.4243   (strongly forward)
///                       1 MeV   +0.0096   (isotropic again)
///                       5 MeV   +0.0333
///                      14 MeV   +0.6027
/// ```
///
/// That is not a parse failure, it is O-16: its elastic angular distributions are
/// resonance-dominated to several MeV and swing through near-isotropy at
/// particular energies. A gate at 1 MeV would have been a gate on an assumption
/// about light nuclei rather than on the data. 14 MeV is above the structure for
/// every nuclide here, so the "did MF=4 parse at all" question gets asked
/// somewhere the answer is unambiguous.
///
/// # Measured (2026-09-12), ENDF/B-VIII.0
///
/// ```text
///   nuclide   1 keV      1 MeV      14 MeV
///   C-12      +0.0000    +0.0859    +0.6008
///   Be-9      +0.0000    +0.1836    +0.7314
///   F-19      +0.0000    +0.1255    +0.6901
///   O-16      -0.0003    +0.0096    +0.6027
/// ```
#[test]
fn moderator_elastic_scattering_turns_forward_between_1_kev_and_14_mev() {
    /// |μ̄| below this at 1 keV counts as isotropic.
    const ISOTROPIC_CEILING: f64 = 0.02;
    /// μ̄ above this at 14 MeV counts as genuinely forward-peaked. Every nuclide
    /// here clears it by 0.2 or more; it is set to catch "the data silently
    /// failed to parse", not to pin a value.
    const FORWARD_FLOOR: f64 = 0.4;

    for (file, name) in MODERATORS {
        let Some(n) = nuclide_or_skip(file, name) else {
            continue;
        };
        let low = n.elastic_mubar_cm(1.0e3);
        let mid = n.elastic_mubar_cm(1.0e6);
        let high = n.elastic_mubar_cm(1.4e7);
        println!("[{name}] MF=4 mu-bar: 1 keV {low:+.4}, 1 MeV {mid:+.4}, 14 MeV {high:+.4}");
        assert!(
            low.abs() < ISOTROPIC_CEILING,
            "[{name}] elastic scattering is not isotropic at 1 keV (mu-bar = \
             {low:+.4}) -- the 4 eV to 10 keV xi/xi_0 = 1.000 verification \
             assumes it is"
        );
        assert!(
            high > FORWARD_FLOOR,
            "[{name}] elastic scattering is not forward-peaked at 14 MeV \
             (mu-bar = {high:+.4}) -- the MF=4 data did not parse, in which case \
             every anisotropy measurement on this nuclide is meaningless, \
             including the ones that priced it at zero"
        );

        // The isotropic-elastic pricing switch must actually remove something.
        let flattened = n.with_isotropic_elastic();
        let flat = flattened.elastic_mubar_cm(1.4e7);
        assert!(
            flat == 0.0,
            "[{name}] with_isotropic_elastic left mu-bar = {flat:+.4} at 14 MeV"
        );
    }
}
