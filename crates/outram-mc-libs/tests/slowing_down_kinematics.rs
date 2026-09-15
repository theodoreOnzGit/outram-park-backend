//! **Elastic slowing-down kinematics against a closed-form oracle — no library,
//! no NJOY, no remembered numbers.**
//!
//! Run ad-hoc as `examples/epithermal_slowing_down.rs` while excluding "the
//! moderator's energy transfer per collision" from the FHR ring-RPT residual
//! (#186). It was the right exclusion and it is worth keeping: the per-collision
//! kernel sits under every thermal and epithermal result this crate produces,
//! and the oracle for it is exact.
//!
//! # The oracle
//!
//! For elastic scattering off a target **at rest** with centre-of-mass cosine
//! `μ_cm`, two-body kinematics give exactly
//!
//! ```text
//! E′/E = (A² + 2A·μ_cm + 1) / (A + 1)²
//! ```
//!
//! so, averaging and writing `α = ((A−1)/(A+1))²`,
//!
//! ```text
//! ⟨E′/E⟩ = (A² + 1 + 2A·μ̄_cm) / (A + 1)²      which for μ̄_cm = 0 is (1 + α)/2
//! ξ      = ⟨ln(E/E′)⟩ = 1 + α·ln α / (1 − α)   for isotropic CM
//! ```
//!
//! and `E′ ≥ α·E` is a **hard kinematic bound** — no elastic collision off a
//! target at rest may drop a neutron below it, at any angle.
//!
//! Both forms are used: the isotropic one against `elastic_scatter`, and the
//! anisotropic one — which this crate supplies `μ̄_cm` for through
//! `Nuclide::elastic_mubar` — against the real ENDF MF=4 angular law. The second
//! is the stronger test, because it holds the sampler to the *nuclide's own*
//! angular distribution rather than to an idealisation of it.
//!
//! # Why the energies are above 10 eV
//!
//! Below `400·kT` the transport kernel samples the target's thermal motion
//! (`free_gas_elastic_scatter`), and below the S(α,β) cutoff it uses the
//! bound-atom law — neither of which is target-at-rest, so neither obeys the
//! oracle above. Those branches have their own tests
//! (`physics::scatter`'s Maxwellian fixed-point tests, and
//! `tests/thermal_laws_vs_njoy_thermr.rs`). This file tests the branch in
//! between, which is where resonance escape is decided.

use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::scatter::{elastic_scatter, two_body_scatter_with_mu};

/// The eight moderator and structural nuclides of the FHR pebble and the
/// LEU-COMP-THERM-008 lattice, plus U-238 as the heavy control: its `α = 0.983`,
/// so a correct kernel can barely move it and a broken one shows immediately.
const NUCLIDES: &[(&str, &str)] = &[
    ("H1", "n-001_H_001-ENDF8.0-Beta6.endf"),
    ("Li6", "n-003_Li_006-ENDF8.0.endf"),
    ("Li7", "n-003_Li_007-ENDF8.0.endf"),
    ("Be9", "n-004_Be_009-ENDF8.0.endf"),
    ("C12", "n-006_C_012-ENDF8.0.endf"),
    ("O16", "n-008_O_016-ENDF8.0.endf"),
    ("F19", "n-009_F_019-ENDF8.0.endf"),
    ("Si28", "n-014_Si_028-ENDF8.0.endf"),
    ("U238", "n-092_U_238.endf"),
];

/// Above the free-gas threshold (`400·kT` = 20.7 eV at 600 K) and inside the
/// resolved-resonance band where resonance escape is decided.
const PROBE_EV: &[f64] = &[50.0, 500.0, 5.0e3, 1.0e4];
const TEMP: f64 = 600.0;
const N: usize = 100_000;

fn load(name: &str, file: &str) -> Option<Nuclide> {
    let Some(path) = njoy_outram_park_fork::reference_data::reference_endf(file) else {
        println!("[{name}] SKIP: {file} not in reference-data/endf/");
        return None;
    };
    Nuclide::from_endf_file(&path, name, TEMP, 1.0e-3).ok()
}

/// **Isotropic-CM elastic scattering off a target at rest reproduces the
/// closed-form `⟨E′/E⟩` and `ξ`, and never violates the `α·E` floor.**
///
/// # Results (2026-09-11, ENDF/B-VIII.0, 100 000 samples per nuclide-energy)
///
/// Every nuclide matches `(1 + α)/2` and `ξ₀ = 1 + α ln α/(1 − α)` to well under
/// 1 %, at every probe energy, with **zero** floor violations. Worst deviations
/// are Monte-Carlo noise at the 0.1–0.3 % level; the heavy control U-238
/// (`α = 0.9832`, `ξ₀ = 0.008378`) is reproduced along with the light moderators.
///
/// # Why the floor check is separate
///
/// `E′ ≥ α·E` is not a statistical statement — it is a bound every single sample
/// must satisfy. A kernel that is right on average and violates the floor is
/// broken in a way no moment can see, and a violation localises immediately to
/// the `cm_to_lab` mapping.
#[test]
fn isotropic_elastic_kinematics_match_the_closed_form() {
    let mut seed = 4_242_424_242_u64;
    let mut tested = 0usize;
    for &(name, file) in NUCLIDES {
        let Some(nuc) = load(name, file) else { continue };
        let a = nuc.awr;
        let alpha = ((a - 1.0) / (a + 1.0)).powi(2);
        let mean_ratio_exact = (a * a + 1.0) / (a + 1.0).powi(2);
        let xi_exact = if alpha > 0.0 && (1.0 - alpha).abs() > 1.0e-12 {
            1.0 + alpha * alpha.ln() / (1.0 - alpha)
        } else {
            1.0
        };
        for &e in PROBE_EV {
            let (mut sum_r, mut sum_xi, mut violations) = (0.0, 0.0, 0usize);
            for _ in 0..N {
                let (ep, _u) = elastic_scatter(e, Direction::new(0.0, 0.0, 1.0), a, &mut seed);
                // A relative slack of 1e-12 covers round-off in cm_to_lab.
                if ep < alpha * e * (1.0 - 1.0e-12) {
                    violations += 1;
                }
                sum_r += ep / e;
                sum_xi += (e / ep).ln();
            }
            let (m, xi) = (sum_r / N as f64, sum_xi / N as f64);
            assert_eq!(
                violations, 0,
                "{name} at {e} eV: {violations} of {N} elastic scatters fell below the \
                 kinematic floor alpha*E = {:.6e} eV. That bound is exact for a target \
                 at rest at every angle",
                alpha * e
            );
            let rel_m = m / mean_ratio_exact - 1.0;
            let rel_xi = if xi_exact.abs() > 1.0e-9 { xi / xi_exact - 1.0 } else { 0.0 };
            println!(
                "  {name:<5} {e:>8.1e}  alpha {alpha:.5}  <E'/E> {m:.6} (exact \
                 {mean_ratio_exact:.6}, {:+.3} %)  xi {xi:.6} (exact {xi_exact:.6}, {:+.3} %)",
                100.0 * rel_m,
                100.0 * rel_xi
            );
            assert!(
                rel_m.abs() < 0.01,
                "{name} at {e} eV: <E'/E> is {:+.3} % from the exact (1+alpha)/2",
                100.0 * rel_m
            );
            assert!(
                rel_xi.abs() < 0.02,
                "{name} at {e} eV: xi is {:+.3} % from the exact 1 + alpha ln alpha/(1-alpha)",
                100.0 * rel_xi
            );
            tested += 1;
        }
    }
    assert!(tested >= 8, "only {tested} nuclide-energy pairs ran; tapes missing?");
}

/// **The sampled outgoing energy is consistent with the sampled CM angle, at
/// energies where the ENDF MF=4 law is strongly anisotropic — and
/// `Nuclide::elastic_mubar` is a stub that disagrees with it by up to 0.91.**
///
/// # The oracle
///
/// Drawing `μ_cm` from the nuclide's own MF=4 law and `E′` from the same
/// collision, the two-body relation must hold on the average of the pair:
///
/// ```text
/// ⟨E′/E⟩ = (A² + 1 + 2A·⟨μ_cm⟩) / (A + 1)²
/// ```
///
/// with `⟨μ_cm⟩` **measured from the same samples**. This is an internal
/// consistency oracle — it catches a CM/lab confusion, a wrong mass factor in
/// `cm_to_lab`, or an angle drawn from one energy and applied at another — and
/// unlike the isotropic check it has teeth precisely where scattering is *not*
/// isotropic.
///
/// # Why it runs up to 14 MeV
///
/// A first version of this test used the probe energies of the isotropic one
/// (50 eV – 10 keV), where every nuclide's `μ̄_cm` is ~1e-3 and the anisotropic
/// oracle collapses onto the isotropic one. It passed, and it was testing
/// nothing. Elastic anisotropy only becomes large in the fast range, which is
/// also where it matters — Godiva and Jemima are fast systems.
///
/// # Results (2026-09-11, ENDF/B-VIII.0, 200 000 samples per point)
///
/// ```text
///          E [eV]   sampled μ̄_cm   elastic_mubar()
///   C12      1e3         +0.0013         +0.0000
///   C12      1e5         +0.0181         +0.0000
///   C12      1e6         +0.0833         +0.0000
///   C12      5e6         +0.2644         +0.0000
///   C12    1.4e7         +0.6004         +0.0000
///   H1     1.4e7         −0.0146         +0.0000
///   U238     1e6         +0.4734         +0.0000
///   U238   1.4e7         +0.9103         +0.0000
/// ```
///
/// H-1 stays isotropic in the CM to 14 MeV, as it should; C-12 and U-238 become
/// strongly forward-peaked. `⟨E′/E⟩` tracks `⟨μ_cm⟩` through the closed form at
/// every point, within Monte-Carlo noise.
///
/// # The stub, which this test found by trying to use it as an oracle
///
/// `Nuclide::elastic_mubar` returns **0.0 for the entire HIGH (`Pointwise`/ENDF)
/// tier**, regardless of the MF=4 data the sampler is reading — see
/// `material/nuclide.rs`, where the arm is `XsSource::Pointwise { .. } => 0.0`.
/// So a public accessor named for the mean CM cosine disagrees with this crate's
/// own sampler by up to **0.91** on U-238 at 14 MeV.
///
/// It is documented in the source as the GPU path's isotropic-CM treatment, so
/// it is a known limitation rather than a surprise — but a value of `0.0` and a
/// value of `0.91` are not distinguishable to a caller, and the accessor offers
/// no way to tell. Pinned here (GitHub #189) so that implementing it breaks this
/// test and whoever does gets told to re-point the oracle above onto it, which
/// would make this a genuinely independent check instead of a consistency one.
#[test]
fn sampled_elastic_energy_is_consistent_with_the_sampled_cm_angle() {
    /// Energies spanning isotropic CM through strongly forward-peaked.
    const FAST_EV: &[f64] = &[1.0e3, 1.0e5, 1.0e6, 5.0e6, 1.4e7];
    let mut seed = 99_887_766_u64;
    let mut worst_sampled_mubar = 0.0_f64;
    let mut tested = 0usize;

    for &(name, file) in NUCLIDES {
        let Some(nuc) = load(name, file) else { continue };
        let a = nuc.awr;
        for &e in FAST_EV {
            let (mut sum_mu, mut sum_r) = (0.0, 0.0);
            for _ in 0..N {
                let mu = nuc
                    .sample_elastic_mu_cm(e, &mut seed)
                    .unwrap_or_else(|| 2.0 * rand_unit(&mut seed) - 1.0);
                let (ep, _u) = two_body_scatter_with_mu(
                    e,
                    Direction::new(0.0, 0.0, 1.0),
                    a,
                    0.0,
                    mu,
                    &mut seed,
                );
                sum_mu += mu;
                sum_r += ep / e;
            }
            let (mubar, m) = (sum_mu / N as f64, sum_r / N as f64);
            let exact = (a * a + 1.0 + 2.0 * a * mubar) / (a + 1.0).powi(2);
            let rel = m / exact - 1.0;
            println!(
                "  {name:<5} {e:>9.2e}  sampled mubar {mubar:>+8.4}  <E'/E> {m:.6} \
                 (closed form {exact:.6}, {:+.3} %)   elastic_mubar() {:>+7.4}",
                100.0 * rel,
                nuc.elastic_mubar(e)
            );
            assert!(
                rel.abs() < 0.005,
                "{name} at {e:.2e} eV: <E'/E> is {:+.3} % from the two-body closed form \
                 built on the SAMPLED mubar {mubar:.4}. The angle and the energy come \
                 from the same collision, so they cannot disagree unless cm_to_lab is \
                 wrong",
                100.0 * rel
            );
            // The known stub — see the doc comment.
            assert_eq!(
                nuc.elastic_mubar(e),
                0.0,
                "Nuclide::elastic_mubar now returns a non-zero value on the HIGH tier for \
                 {name} at {e:.2e} eV. That is an improvement (GitHub #189) -- and this \
                 test's oracle should now be built from it rather than from the sampled \
                 mean, which would make it independent instead of self-consistent"
            );
            if mubar.abs() > worst_sampled_mubar {
                worst_sampled_mubar = mubar.abs();
            }
            tested += 1;
        }
    }
    assert!(tested >= 10, "only {tested} nuclide-energy pairs ran; tapes missing?");
    println!("  largest |sampled mubar_cm|: {worst_sampled_mubar:.4}");
    assert!(
        worst_sampled_mubar > 0.5,
        "the largest sampled |mubar_cm| over this set is only {worst_sampled_mubar:.4}. \
         Elastic scattering off C-12 and U-238 is strongly forward-peaked at 14 MeV, so \
         a near-isotropic result means the MF=4 angular law has stopped reaching the \
         sampler -- the same silent failure mode as the S(alpha,beta) wiring defect of \
         2026-08-14"
    );
}

/// A uniform deviate from the same LCG the crate uses, for the fallback when a
/// nuclide carries no MF=4 law.
fn rand_unit(seed: &mut u64) -> f64 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((*seed >> 11) as f64) / ((1u64 << 53) as f64)
}
