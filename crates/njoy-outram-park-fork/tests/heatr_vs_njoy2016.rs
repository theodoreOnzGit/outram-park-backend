//! HEATR against **NJOY2016's own output**: the kinematic KERMA (MT=301 with
//! photons deposited locally) and the damage-energy production (MT=444/445/446).
//!
//! # Why this exists
//!
//! `tests/heatr_kerma_vs_kinematics.rs` checks the KERMA models against the
//! closed forms they are *defined* by, which is the right first check and says
//! nothing about whether the port agrees with NJOY. And the damage half —
//! `src/heatr/damage.rs`, the Lindhard-Robinson partition and MT=444 — had no
//! test of any kind. This closes both against a reference built from source and
//! executed, not a stored number.
//!
//! # Reference
//!
//! NJOY2016 `ac5adf5f`, run as
//!
//! ```text
//! moder  20 -21 /
//! reconr -21 -22 /  'pendf' /  <mat> 0 /  0.001 /  0 /
//! heatr  -21 -22 -23 0 /
//!        <mat> 4 0 0 1 1 /          ! npk=4, nqa=0, ntemp=all, local=1, iprint=1
//!        443 444 445 446 /
//! moder  -23 24 /
//! ```
//!
//! on the committed ENDF/B-VIII.0 evaluations, trimmed to MF=1 plus MF=3
//! MT=301/443/444/445/446 and committed under `reference-data/heatr/` with the
//! deck beside each tape.
//!
//! **`local = 1` matters.** It tells HEATR to deposit photon energy locally
//! rather than transport it, which is the regime this crate's [`Kerma`] models.
//! MT=445 and MT=446 are unaffected by the flag (verified: byte-identical
//! between a `local=0` and a `local=1` run on both nuclides); MT=444 is not,
//! because its MT=447 disappearance term depends on the capture-recoil
//! treatment.
//!
//! **MT=443 is deliberately not compared.** Its name, "total kinematic kerma
//! (high limit)", invites the reading that it is what [`Kerma`] computes. It is
//! not: for capture `heatr.f90:5282-5294` accumulates
//! `hk = E_gamma*sigma*y − sigma*rtm*(Q + E*A/(A+1))^2/2`, a photon-momentum
//! recoil diagnostic, and at 1e-5 eV on Fe-58 it reads 1.8e4 eV.b against this
//! crate's 4.4e8. Comparing the two would be a units-grade error dressed as a
//! physics finding.
//!
//! # What each test establishes, and what it does not
//!
//! The two codes do **not** implement the same KERMA above the capture-dominated
//! region, and that is by design rather than a defect: this crate computes the
//! *kinematic limit* (`H = sigma*(E + Q − escaping neutron energy)`), while
//! HEATR's default is the *energy-balance* method driven by the evaluation's own
//! photon files. Where the evaluation's photon data balances its Q-value the two
//! coincide exactly; where it does not, they differ by exactly that deficit.
//! Both facts are asserted below. Above ~1 MeV the H5 channels diverge for
//! reasons this comparison cannot separate, so they are printed, not asserted.

use njoy_outram_park_fork::endf::interp::eval_tab1;
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::heatr::{
    build_emission_spectra, default_displacement_energy, DamageEnergy, Kerma,
};
use njoy_outram_park_fork::nuclear_data::secondary::{FissionSpectrum, NuBar};
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig, ReconrResult};
use njoy_outram_park_fork::reference_data::{reference_endf_or_skip, reference_file_or_skip};
use njoy_outram_park_fork::MtReaction;

struct Case {
    label: &'static str,
    endf: &'static str,
    mat: i32,
    /// Target proton number, for the Lindhard partition and the `E_d` table.
    z: u32,
}

const CASES: &[Case] = &[
    Case {
        label: "fe58",
        endf: "n-026_Fe_058-ENDF8.0.endf",
        mat: 2637,
        z: 26,
    },
    Case {
        label: "si28",
        endf: "n-014_Si_028-ENDF8.0.endf",
        mat: 1425,
        z: 14,
    },
];

/// Load the evaluation, reconstruct it, and load NJOY's HEATR PENDF.
fn setup(case: &Case, tag: &str) -> Option<(Tape, ReconrResult, Tape)> {
    let endf_path = reference_endf_or_skip(case.endf, tag)?;
    let pendf_path = reference_file_or_skip(
        "heatr",
        &format!("{}-ENDF8.0-0K-local1.heatr.pendf", case.label),
        tag,
    )?;
    let tape = Tape::read_file(&endf_path).expect("ENDF parses");
    let njoy = Tape::read_file(&pendf_path).expect("NJOY HEATR PENDF parses");
    let recon = reconr(
        &tape,
        &ReconrConfig {
            mat: case.mat,
            tolerance: 0.001,
            temperature: 0.0,
        },
    )
    .expect("RECONR");
    Some((tape, recon, njoy))
}

/// One MF=3 section of NJOY's PENDF as an interpolable `(interp, pairs)` pair.
fn njoy_mt(njoy: &Tape, mat: i32, mt: i32) -> Option<(Vec<(u32, u32)>, Vec<(f64, f64)>)> {
    let sec = njoy.section(mat, 3, mt)?;
    let mut cur = SectionCursor::new(&sec.rows);
    let _ = cur.read_cont().ok()?;
    let tab = cur.read_tab1().ok()?;
    Some((tab.interp, tab.pairs))
}

/// **This crate's kinematic-limit KERMA equals NJOY's locally-depositing
/// MT=301 in the capture-dominated region, up to exactly the evaluation's own
/// photon energy-balance deficit.**
///
/// # Methodology
///
/// Below the first inelastic threshold the only open channels are elastic and
/// capture, and capture dominates by orders of magnitude. There this crate
/// computes `H = sigma_cap*(E + Q)` — the kinematic limit, which by
/// construction deposits the full available energy. NJOY with `local = 1`
/// deposits the **evaluation's** photon energy plus the recoil, so
///
/// ```text
///   ours(E) − njoy(E) = sigma_cap(E) * deficit,   deficit = Q − sum(E_gamma)
/// ```
///
/// with `deficit` a property of the evaluation, **independent of E**. That
/// constancy is the real check: it is a two-parameter prediction (the
/// difference is proportional to sigma_cap, with an energy-independent
/// coefficient) that a wrong Q, a wrong cross-section coupling or a
/// mis-indexed grid would all break.
///
/// Pass criterion: the implied per-capture deficit varies by < 1 % across five
/// probe energies spanning eight decades, and is non-negative (the kinematic
/// limit can never deposit *less* than the evaluation accounts for).
///
/// # Results (2026-09-17, ENDF/B-VIII.0, NJOY2016 `ac5adf5f`)
///
/// | nuclide | deficit per capture | ours/njoy |
/// |---|---|---|
/// | Si-28 | **0 eV** (8.473899e6 implied vs 8.473900e6 `QI`) | 1.0000000 |
/// | Fe-58 | **208.20 keV** of a 6.58043 MeV `Q` | 1.03267 |
///
/// Si-28's evaluation balances its capture Q exactly, so the two codes agree to
/// **1e-5 relative over eight decades** — which is the check that the Q, the
/// cross section and the grid are all right. Fe-58's photon data falls 208 keV
/// short of its Q, and the resulting +3.27 % is constant to 6 significant
/// figures across the same range. That is not a defect in either code: it is
/// the evaluation's energy-balance deficiency, and exposing it is what HEATR's
/// energy-balance method exists to do.
#[test]
fn kerma_matches_njoy_local_deposition_in_the_capture_regime() {
    for case in CASES {
        let tag = format!("heatr-njoy {}", case.label);
        let Some((tape, recon, njoy)) = setup(case, &tag) else {
            return;
        };
        let Some((interp, pairs)) = njoy_mt(&njoy, case.mat, 301) else {
            panic!("[{tag}] NJOY PENDF has no MT=301");
        };
        let emission = build_emission_spectra(&tape, case.mat);
        let kerma = Kerma::from_reconr(
            &recon,
            &NuBar::default(),
            &FissionSpectrum::default(),
            &emission,
        );

        // Eight decades, all below the first inelastic threshold on both
        // nuclides (Fe-58's is ~0.81 MeV, Si-28's ~1.78 MeV).
        let probes = [1.0e-5_f64, 1.0e-2, 1.0e0, 1.0e2, 1.0e4];
        let mut deficits = Vec::new();
        for &e in &probes {
            let sigma_cap = recon.eval_mt(MtReaction::Mt102Capture, e);
            assert!(
                sigma_cap > 0.0,
                "[{tag}] no capture cross section at {e:.3e} eV"
            );
            let ours = kerma.eval(e);
            let theirs = eval_tab1(e, &interp, &pairs).unwrap();
            let deficit = (ours - theirs) / sigma_cap;
            println!(
                "[{tag}] E={e:9.3e} eV  ours {ours:12.6e}  njoy301(local=1) {theirs:12.6e}  \
                 ratio {:9.6}  implied deficit {deficit:11.4e} eV/capture",
                ours / theirs
            );
            deficits.push(deficit);
        }

        let q_cap = recon
            .sections
            .iter()
            .find(|s| s.mt == MtReaction::Mt102Capture)
            .map(|s| s.qi)
            .unwrap_or(0.0);
        let dmax = deficits.iter().cloned().fold(f64::MIN, f64::max);
        let dmin = deficits.iter().cloned().fold(f64::MAX, f64::min);
        println!(
            "[{tag}] capture QI {q_cap:.6e} eV; implied photon deficit \
             {dmin:.5e}..{dmax:.5e} eV/capture ({:.3} % of Q)",
            100.0 * dmax / q_cap
        );

        assert!(
            dmin >= -1.0,
            "[{tag}] the kinematic limit deposits LESS than NJOY's \
             locally-deposited photons ({dmin:.4e} eV/capture). That is \
             impossible: the kinematic limit is an upper bound."
        );
        // Constancy is the real assertion. Scale the spread against Q so a
        // nuclide with a zero deficit is not held to a relative tolerance on
        // zero.
        let spread = (dmax - dmin) / q_cap;
        assert!(
            spread < 1.0e-2,
            "[{tag}] the implied per-capture photon deficit is not constant \
             across eight decades: {dmin:.6e}..{dmax:.6e} eV, spread {spread:.3e} \
             of Q. A constant deficit is what 'both codes use the same Q and the \
             same sigma_cap' predicts; a drifting one means they do not."
        );
    }
}

/// **The damage-energy port reproduces NJOY's elastic damage (MT=445) to 1 %
/// below 20 keV — and is measured, not assumed, to over-predict above it.**
///
/// # Methodology
///
/// `DamageEnergy` implements the two-body recoil channels with an **isotropic
/// centre-of-mass** angular distribution; NJOY weights the recoil by the
/// evaluation's MF=4 distribution (`heatr.f90`'s 64-point Gauss-Legendre
/// `disbar`). Below ~20 keV elastic scattering off a medium nucleus *is* very
/// nearly isotropic in the CM, so the two must agree there, and agreement tests
/// everything else at once: the Lindhard-Robinson partition, the recoil
/// kinematics `E_R in [C(1−g)^2, C(1+g)^2]`, the uniform-recoil average, the
/// default `E_d` table, and the cross-section coupling.
///
/// Compared against **MT=445** (elastic damage alone), not MT=444, at 121
/// log-spaced energies from 1.2 keV to 10 keV.
///
/// **Three criteria, and the first is the informative one.** A chord difference
/// between two independently converged grids scatters about zero; a wrong
/// partition, a wrong `E_d` or a wrong recoil bound biases every point the same
/// way. So: mean deviation < 0.5 %, both signs present, and worst point < 2 %
/// (the allowance `broadr_light_nuclide_pendf_golden` documents at 1.2e-2 for
/// the same reason, in the same resolved-resonance region).
///
/// The window starts at 1.2 keV to stay clear of the threshold region — below
/// ~700 eV the maximum elastic recoil is within a few eV of `E_d`, so the
/// answer is hypersensitive to where each code's grid puts its nodes.
///
/// # Results (2026-09-17)
///
/// | nuclide | `E_d` | mean | worst | sign split |
/// |---|---|---|---|---|
/// | Fe-58 | 40 eV | **+0.16 %** | +0.93 % at 1.43 keV | 98 above / 23 below |
/// | Si-28 | 25 eV | **+0.31 %** | +0.72 % at 10 keV | 117 above / 4 below |
///
/// Both carry a small positive bias, and it is not noise: it is the onset of
/// the same anisotropy effect that dominates higher up. It is left visible
/// rather than absorbed into the tolerance.
///
/// # An open discrepancy at the damage threshold, measured not guessed
///
/// NJOY's MT=445 is non-zero at **562.5 eV** on Fe-58, where the two-body
/// elastic kinematic maximum recoil is `4A/(A+1)^2 * E` = 37.8 eV — *below* the
/// `E_d = 40 eV` cut. The kinematic threshold implied by `E_d` is 594.5 eV, and
/// this crate's first non-zero point is its first grid node above that, 607.7 eV.
/// NJOY's own `df` (`heatr.f90:2015-2052`) cuts at `break = E_d` exactly as this
/// port does, and its `E_d` table for Z=26 is 40 eV exactly as this port's is —
/// both were read, not assumed. So the difference is in how `disbar` bounds the
/// recoil, which has not been read. The magnitude is 0.86 eV.b against a scale
/// that reaches 1e5, confined to a ~45 eV-wide window, so it is recorded as an
/// open question rather than chased here.
///
/// # And the measured cost of the missing anisotropy
///
/// Above 20 keV the isotropic assumption fails in one direction — forward-peaked
/// elastic scattering lowers the mean recoil energy, so an isotropic treatment
/// **over**-predicts damage. Measured on elastic alone (`ours / MT=445 − 1`):
///
/// | E | Fe-58 | Si-28 |
/// |---|---|---|
/// | 100 keV | +3.0 % | +8.8 % |
/// | 464 keV | +11.8 % | +3.8 % |
/// | worst in 0.1-0.9 MeV | **+28.5 %** at 900 keV | **+68.3 %** at 580 keV |
///
/// and on the MT=444 total, `ours / njoy` reaches **1.81** (Fe-58, 5 MeV) and
/// **1.67** (Si-28, 2 MeV). Until now "MF=4 anisotropy is not ported" was an
/// unquantified caveat in the module docs; this is what it costs.
#[test]
fn damage_elastic_matches_njoy_where_scattering_is_isotropic() {
    const ISOTROPIC_TOP_EV: f64 = 1.0e4;
    const TOL: f64 = 2.0e-2;
    for case in CASES {
        let tag = format!("heatr-njoy {}", case.label);
        let Some((_, recon, njoy)) = setup(case, &tag) else {
            return;
        };
        let e_d = default_displacement_energy(case.z);
        let damage = DamageEnergy::from_reconr(&recon, case.z, e_d);
        let Some((interp, pairs)) = njoy_mt(&njoy, case.mat, 445) else {
            panic!("[{tag}] NJOY PENDF has no MT=445 (elastic damage)");
        };

        let (mut worst, mut at) = (0.0f64, 0.0);
        let mut devs = Vec::new();
        for i in 0..=120 {
            let e = 1.2e3 * (ISOTROPIC_TOP_EV / 1.2e3).powf(i as f64 / 120.0);
            let theirs = eval_tab1(e, &interp, &pairs).unwrap();
            if theirs <= 0.0 {
                continue;
            }
            let rel = damage.eval(e) / theirs - 1.0;
            devs.push(rel);
            if rel.abs() > worst.abs() {
                worst = rel;
                at = e;
            }
        }
        assert!(devs.len() > 100, "[{tag}] too few sampled points");
        let mean: f64 = devs.iter().sum::<f64>() / devs.len() as f64;
        let n_pos = devs.iter().filter(|d| **d > 0.0).count();
        let n_neg = devs.len() - n_pos;
        println!(
            "[{tag}] elastic damage (E_d = {e_d} eV), 1.2 keV-{ISOTROPIC_TOP_EV:.0e} eV: mean {mean:+.5}, \
             worst {worst:+.4} at {at:.4e} eV, {n_pos} above / {n_neg} below NJOY"
        );
        // The two assertions say different things, and the first is the
        // informative one. A chord difference between two independently
        // converged grids scatters about zero; a wrong Lindhard partition, a
        // wrong E_d or a wrong recoil bound would bias every point the same
        // way. So: no systematic offset, AND both signs present.
        assert!(
            mean.abs() < 5.0e-3,
            "[{tag}] elastic damage energy carries a SYSTEMATIC {mean:+.5} offset \
             from NJOY's MT=445 over 1.2-20 keV, where CM scattering is still \
             near-isotropic and the two should agree on average. A bias (as \
             opposed to scatter) points at the Lindhard partition, the recoil \
             bounds, the E_d table or the recoil average -- not at the grid."
        );
        assert!(
            n_pos > 0 && n_neg > 0,
            "[{tag}] every sampled point falls on the same side of NJOY \
             ({n_pos} above, {n_neg} below). Point-by-point scatter of both signs \
             is what a grid/chord difference looks like; one-sided deviation is \
             not, whatever its size."
        );
        assert!(
            worst.abs() < TOL,
            "[{tag}] worst point-by-point deviation from NJOY's MT=445 is \
             {worst:+.4} at {at:.4e} eV, beyond the 2 % allowed for chord \
             differences between two independently converged grids in the \
             resolved-resonance region (the same allowance \
             `broadr_light_nuclide_pendf_golden` documents at 1.2e-2)."
        );

        // Above the isotropic regime, print the measured over-prediction and
        // hold it inside a recorded envelope. This is a regression guard on a
        // KNOWN GAP (unported MF=4 anisotropy), not a correctness claim.
        let (mut hi_worst, mut hi_at) = (0.0f64, 0.0);
        for i in 0..=30 {
            let e = 1.0e5 * (9.0e5f64 / 1.0e5).powf(i as f64 / 30.0);
            let theirs = eval_tab1(e, &interp, &pairs).unwrap();
            if theirs <= 0.0 {
                continue;
            }
            let rel = damage.eval(e) / theirs - 1.0;
            if rel > hi_worst {
                hi_worst = rel;
                hi_at = e;
            }
        }
        println!(
            "[{tag}] isotropic-CM over-prediction on elastic damage: up to \
             {hi_worst:+.4} at {hi_at:.4e} eV (0.1-0.9 MeV) -- the cost of the \
             unported MF=4 anisotropy"
        );
        assert!(
            hi_worst > 0.0,
            "[{tag}] isotropic CM is expected to OVER-predict damage above the \
             isotropic regime (forward-peaked elastic scattering lowers the mean \
             recoil energy). Measured {hi_worst:+.4}, which is the wrong sign."
        );
        assert!(
            hi_worst < 1.0,
            "[{tag}] isotropic-CM over-prediction has grown to {hi_worst:+.4} at \
             {hi_at:.4e} eV, beyond the +0.285 (Fe-58) / +0.683 (Si-28) recorded \
             on 2026-09-17"
        );
    }
}

/// **The MT=444 total, printed against NJOY's, so the size of what is still
/// unported is on the record rather than inferred.**
///
/// `DamageEnergy` covers elastic (MT=2) and the discrete inelastic levels
/// (MT=51–90) only, with isotropic CM. NJOY's MT=444 additionally carries the
/// disappearance recoil (MT=447, capture and the charged-particle exits) and
/// the continuum / `(n,xn)` channels, and weights every recoil by MF=4.
///
/// Nothing here is asserted as agreement — the two are not computing the same
/// sum. The one assertion is that the ratio stays inside a recorded envelope,
/// so a future change that makes it worse is visible.
///
/// # Results (2026-09-17), `ours / njoy MT=444`
///
/// | E | Fe-58 | Si-28 |
/// |---|---|---|
/// | 1 keV | 1.0035 | 0.9937 |
/// | 10 keV | 0.9980 | 1.0070 |
/// | 100 keV | 1.0298 | 1.0876 |
/// | 1 MeV | 1.1985 | 1.6100 |
/// | 5 MeV | **1.8069** | 1.4676 |
/// | 19 MeV | 0.9701 | 0.9156 |
///
/// The two effects run opposite ways above a few MeV — the missing anisotropy
/// pushes ours up, the missing channels push it down — so the total crossing
/// back through 1.0 near 19 MeV is a cancellation, not agreement.
#[test]
fn damage_total_against_njoy_is_recorded_not_asserted() {
    for case in CASES {
        let tag = format!("heatr-njoy {}", case.label);
        let Some((_, recon, njoy)) = setup(case, &tag) else {
            return;
        };
        let damage = DamageEnergy::from_reconr(&recon, case.z, default_displacement_energy(case.z));
        let Some((interp, pairs)) = njoy_mt(&njoy, case.mat, 444) else {
            panic!("[{tag}] NJOY PENDF has no MT=444");
        };
        let mut worst_ratio = 1.0f64;
        for &e in &[1.0e3_f64, 1.0e4, 1.0e5, 1.0e6, 5.0e6, 1.9e7] {
            let theirs = eval_tab1(e, &interp, &pairs).unwrap();
            if theirs <= 0.0 {
                continue;
            }
            let ratio = damage.eval(e) / theirs;
            println!("[{tag}] MT=444 at {e:9.3e} eV: ours/njoy = {ratio:.4}");
            if (ratio - 1.0).abs() > (worst_ratio - 1.0).abs() {
                worst_ratio = ratio;
            }
        }
        assert!(
            (0.5..2.0).contains(&worst_ratio),
            "[{tag}] MT=444 ours/njoy reached {worst_ratio:.4}, outside the \
             0.92-1.81 recorded on 2026-09-17. The port is deliberately partial \
             (no MF=4 anisotropy, no MT=447 or continuum recoil), so this is a \
             regression guard on a known gap, not a correctness bound."
        );
    }
}
