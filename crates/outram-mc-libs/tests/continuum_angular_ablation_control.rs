//! **Regression gate for the MF=6 LAW=1 continuum angular law** (bead
//! `op-og56`) and for the ablation control that prices it.
//!
//! # What this guards
//!
//! Until `op-og56`, `njoy-outram-park-fork`'s MF=6 parser read ENDF `NA` only to
//! compute the row stride, kept `f₀`, and discarded `f₁ … f_NA`; `LANG` was never
//! read. So every MT=91 continuum neutron and every MT=16 (n,2n) neutron left
//! the collision isotropically in the frame the law names, on evaluations that
//! say otherwise.
//!
//! This file is the cheap half that belongs in the suite. It guards two separate
//! things, and the second matters as much as the first:
//!
//! - **The physics is wired in at all.** A regression that dropped the
//!   coefficients again would be *invisible*: the sampler would keep returning
//!   cosines, they would simply all be flat, and no existing test would fail.
//! - **The ablation control actually ablates.** A control that silently fails to
//!   switch the mechanism off reports "no difference" and reads as "this physics
//!   does not matter" — the worst failure mode an ablation study has, and the
//!   reason `tests/inelastic_anisotropy_ablation_control.rs` exists one channel
//!   over.
//!
//! # The result that shapes this test: WHERE the anisotropy lives
//!
//! `njoy-outram-park-fork`'s `tests/mf6_continuum_angular_vs_endf.rs` reports
//! that 8652 of U-238's 8654 MT=91 rows carry `NA > 0`, with a peak `|⟨μ⟩|` of
//! **0.557**. Taken alone that number badly overstates the physics, and reading
//! it that way would have produced a test asserting something false.
//!
//! Weighted by each row's own emission probability `f₀` — i.e. the `⟨μ_cm⟩` a
//! neutron actually experiences — the law is **exactly isotropic near threshold
//! and only turns on above a few MeV** (measured 2026-09-16, ENDF/B-VIII.0,
//! branch 0):
//!
//! | `E_in` | pdf-weighted `⟨μ_cm⟩` | peak `|⟨μ⟩|` in that table |
//! |---|---|---|
//! | 0.4356 MeV (threshold) | 0.000000 | 0.000000 |
//! | 1.02 MeV | 0.000000 | 0.000000 |
//! | 1.34 MeV | −0.012053 | 0.022707 |
//! | 2.17 MeV | +0.000105 | 0.030456 |
//! | 3.00 MeV | +0.001684 | 0.035691 |
//! | 4.50 MeV | +0.009076 | 0.105401 |
//! | 8.50 MeV | +0.073219 | 0.190664 |
//! | 14.0 MeV | +0.272349 | 0.334664 |
//! | 20.0 MeV | +0.387079 | 0.451866 |
//!
//! The 0.557 peak lives in the far tail of a high-energy table, where `f₀` is
//! negligible. So this law is a **high-energy** correction, unlike the discrete
//! levels of `op-tm9f`, which are anisotropic from ~1 MeV upward — right in the
//! bulk of a fission spectrum — and were worth −198 pcm on Godiva.
//!
//! **Predicted worth, stated before measuring it** (the rule in the workspace
//! `CLAUDE.md`, and the lesson of `op-mzvp.2.12`): forward-peaked emission
//! raises `⟨μ⟩`, lowers `Σ_tr = Σ_t(1 − ⟨μ⟩)`, lengthens the transport mean free
//! path and so *increases* leakage — **`k` goes down**, the same direction
//! `op-tm9f` moved. The magnitude should be **small — well under 50 pcm on
//! Godiva, plausibly under 20** — because a fission spectrum puts only a percent
//! or two of its flux above 8 MeV, which is where this law has any structure at
//! all. If a measurement comes back much larger, the hypothesis is wrong and the
//! wiring should be suspected before the physics.
//!
//! # The spectral side effect: PREDICTED HARDER, MEASURED (weakly) SOFTER
//!
//! For a centre-of-mass law the lab energy is
//! `E' = E_cm + E_trans + 2·μ_cm·√(E_cm·E_trans)`, so a forward-peaked cosine
//! raises `⟨E'_lab⟩` per collision. That argument was recorded here as
//! "this hardens the spectrum and moves `op-os8x` the wrong way".
//!
//! **Measured 2026-09-16** (`examples/godiva_continuum_spectrum_ablation.rs`,
//! 8 seeds per arm), ANISO against ISO:
//!
//! | measure | relative change | |
//! |---|---|---|
//! | mean `E` | **−0.076 % ± 0.109** | 0.7 σ |
//! | mean `ln E` | **−0.008 % ± 0.008** | 1.0 σ |
//! | flux fraction below 300 keV | **+0.157 % ± 0.226** | 0.7 σ |
//!
//! **Nothing is resolved at 2 σ, and all three central values point the other
//! way** — softer, not harder. The prediction is therefore *not supported*; it
//! is also not refuted, and the honest statement is a bound: the spectral
//! effect of this law is smaller than about `0.22 %` in mean `E`, against the
//! `+0.45 %` residual `op-os8x` is about. **It cannot be the explanation for
//! `op-os8x`, in either direction.**
//!
//! Note the three measures are *not* three independent votes — they are taken
//! from the same tallied spectrum in the same runs and are strongly correlated.
//! Their agreeing in sign is close to one observation, not three.
//!
//! An after-the-fact hypothesis, labelled as such because it was formed after
//! seeing the numbers and has not been tested: in a 55.8 %-leakage bare sphere,
//! raising the transport mean free path preferentially removes the *fast*
//! neutrons most likely to escape, which softens the surviving in-core flux.
//! That would oppose the per-collision hardening and is tied to the same
//! leakage that produced the reactivity effect. **The measurement that would
//! separate them is the same spectrum comparison run with a reflective
//! boundary** (`k_inf`, no leakage), where only the per-collision term
//! survives — the same decomposition `op-tm9f` used.
//!
//! # Methodology
//!
//! U-238 (ENDF/B-VIII.0, `reference-data/endf/n-092_U_238.endf`) is built
//! HIGH-tier and its MT=91 [`ContinuumEmission`] taken from
//! [`Nuclide::continuum_law`]. Assertions, in the order they appear:
//!
//! 1. The law reaches the sampler as `ContinuumAngular::Legendre` — not
//!    `EvaluatedIsotropic` (which would mean the coefficients were dropped) and
//!    not `Unported` (which would mean the representation changed under us).
//! 2. The **profile above is reproduced**: isotropic at threshold, monotonically
//!    rising through the MeV range, strongly forward-peaked by 14 MeV. A
//!    mis-indexed incident-energy lookup would not survive this, and neither
//!    would a coefficient vector read one row out of step.
//! 3. The **sampler inverts the law it was given**: drawing from a row's cosine
//!    CDF reproduces that row's own `⟨μ⟩ = a₁ = f₁/f₀`, which comes straight off
//!    the tape and never passes through the linearisation. This is the check
//!    that the adaptive tabulation and the CDF inversion are both right, and it
//!    is independent of anything the transport layer does.
//! 4. The ablation **changes the emitted directions** where the law has
//!    structure, so the control is not a no-op.
//! 5. The ablation **leaves the RNG stream bit-identical** — the final seed
//!    after N collisions matches between arms. This is what makes a paired
//!    measurement attributable: the two arms spend the same variates in the same
//!    order, one on a CDF inversion and one on a linear map. Without it a
//!    measured Δk could be a re-randomisation artefact, the trap gh:#196 records
//!    for the seed-ensemble baselines.
//!
//!    Note the outgoing **lab energy** is deliberately *not* asserted invariant.
//!    It cannot be: the CM→lab transform couples angle to energy, so changing
//!    `μ_cm` must change `E'_lab`. An earlier draft of this file asserted the
//!    lab energy was bit-identical, and it failed 4091/4096 — correctly, because
//!    the invariant was false. It is recorded here so nobody re-derives it.
//! 6. F-19's MT=91, which the evaluation declares isotropic throughout, is
//!    **bit-identical across both arms** — the negative control. If the ablation
//!    changed F-19, the sampler would be manufacturing angular structure the
//!    tape does not contain.
//!
//! # Results (2026-09-16, ENDF/B-VIII.0, `cargo test --release`, 101 s)
//!
//! **Sampler against the tape.** The most anisotropic row of U-238's 14 MeV
//! table carries `a₁ = f₁/f₀ = +0.334664` on the tape. Drawing 200 000
//! stratified variates through its linearised cosine CDF gives
//! `⟨μ⟩ = +0.334445` — agreement to **2.2e-4**. `a₁` never passes through the
//! linearisation, so this checks the adaptive tabulation *and* the inversion,
//! not merely that a number comes back.
//!
//! **Ablation, `⟨μ_lab⟩` over 4096 draws per arm:**
//!
//! | `E_in` | evaluated | ablated | difference | RNG streams |
//! |---|---|---|---|---|
//! | 2 MeV | +0.00991 | +0.01010 | −0.00019 ± 0.01271 | identical |
//! | 14 MeV | **+0.25353** | −0.00426 | **+0.25779 ± 0.01238** (21σ) | identical |
//!
//! The 2 MeV row agreeing is the *correct* result, not a weak one — the law is
//! flat there. The 14 MeV row is the control demonstrating it ablates.
//!
//! **Negative control.** F-19 MT=91 at 8 MeV: **4096/4096** samples identical
//! across the ablation in both energy and angle, and the RNG streams match.
//!
//! **This is verification, not validation.** It establishes that the evaluated
//! angular law is read, sampled correctly, and separable from everything else in
//! the collision. What it is worth in `k` is a *separate* measurement and must be
//! a paired-seed ensemble — a single Godiva-scale run carries ~200 pcm of
//! seed-to-seed spread, an order larger than the effect predicted above.

use njoy_outram_park_fork::nuclear_data::secondary::{ContinuumAngular, ContinuumEmission};
use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::scatter::{
    continuum_inelastic_scatter_evaluated_with, ContinuumAngularMode,
};

const TEMP_K: f64 = 293.6;
/// U-238's MT=91 continuum inelastic channel.
const MT_91: i32 = 91;
/// U-238's MT=91 `QI` \[eV\] — the channel threshold term used for the two-body
/// kinematic cap. Negative, as an endothermic channel must be.
const Q_MT91_EV: f64 = -4.356e5;
const DRAWS: usize = 4096;

/// U-238's atomic weight ratio (ENDF/B-VIII.0 MAT 9237).
const AWR_U238: f64 = 236.0058;
/// F-19's atomic weight ratio (ENDF/B-VIII.0 MAT 925).
const AWR_F19: f64 = 18.835_00;
/// F-19's MT=91 `QI` \[eV\].
const Q_F19_MT91_EV: f64 = -5.9368e6;

fn z_dir() -> Direction {
    Direction::new(0.0, 0.0, 1.0)
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

/// Standard error of the mean, so a difference is judged resolved rather than
/// eyeballed.
fn sem(v: &[f64]) -> f64 {
    let m = mean(v);
    let var = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (v.len() as f64 - 1.0);
    (var / v.len() as f64).sqrt()
}

/// One arm of the ablation: `n` collisions from a fixed seed.
///
/// Returns the sampled lab cosines and the **final RNG seed**. The incident
/// direction is `+z`, so the outgoing `w` component *is* the laboratory
/// scattering cosine — no projection, and no chance of measuring a rotation
/// artefact instead of the physics. The final seed is returned because it is the
/// invariant that proves the ablation did not perturb the random stream.
fn sample_arm(
    law: &ContinuumEmission,
    awr: f64,
    q: f64,
    e_in: f64,
    mode: ContinuumAngularMode,
    n: usize,
    seed: u64,
) -> (Vec<f64>, Vec<f64>, u64) {
    let mut s = seed;
    let mut mus = Vec::with_capacity(n);
    let mut es = Vec::with_capacity(n);
    for _ in 0..n {
        let (e_out, u_out) = continuum_inelastic_scatter_evaluated_with(
            e_in,
            z_dir(),
            awr,
            q,
            Some(law),
            mode,
            &mut s,
        );
        mus.push(u_out.w);
        es.push(e_out);
    }
    (mus, es, s)
}

/// The pdf-weighted `⟨μ_cm⟩` of one incident-energy table — the mean cosine a
/// neutron emitted at that incident energy actually experiences, as opposed to
/// the largest cosine anywhere in the table.
fn pdf_weighted_mubar(law: &ContinuumEmission, branch: usize, table: usize) -> Option<f64> {
    let b = law.branches.get(branch)?;
    let ContinuumAngular::Legendre(tabs) = &b.angular else {
        return None;
    };
    let t = tabs.get(table)?;
    let ce = b.spectrum.tables.get(table)?;
    let (mut num, mut den) = (0.0, 0.0);
    for (k, r) in t.rows.iter().enumerate() {
        let w = *ce.pdf.get(k)?;
        num += w * r.mubar;
        den += w;
    }
    if den > 0.0 {
        Some(num / den)
    } else {
        Some(0.0)
    }
}

/// The evaluated law is read, has the energy profile the tape carries, and the
/// sampler inverts it correctly.
#[test]
fn u238_continuum_angular_law_is_read_and_has_the_evaluated_energy_profile() {
    let Some(tape) = reference_file_or_skip(
        "endf",
        "n-092_U_238.endf",
        "U-238 evaluation (continuum angular ablation control)",
    ) else {
        return;
    };

    let nuc = Nuclide::from_endf_file(&tape, "U238", TEMP_K, 1.0e-3).expect("U-238 reconstructs");
    let law = nuc
        .continuum_law(MT_91)
        .expect("U-238 carries an MF=6 LAW=1 law for MT=91");

    // 1: the Legendre law reached the sampler.
    for (b, branch) in law.branches.iter().enumerate() {
        match &branch.angular {
            ContinuumAngular::Legendre(_) => {}
            other => panic!(
                "U-238 MT=91 branch {b} reached transport as {other:?}, not Legendre. The tape \
                 declares LANG=1 with NA>0 on 8652 of 8654 rows, so this means the angular half \
                 of the MF=6 law stopped being read (bead op-og56) -- and the failure is SILENT \
                 in every other test, because the sampler keeps returning cosines, just flat ones."
            ),
        }
        assert!(
            branch.angular.is_anisotropic(),
            "U-238 MT=91 branch {b}: every row came back isotropic."
        );
    }

    // 2: the profile. Isotropic at threshold, forward-peaked at 14 MeV.
    let incident = &law.branches[0].spectrum.incident;
    let find = |target: f64| -> usize {
        incident
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                (*a - target)
                    .abs()
                    .partial_cmp(&(*b - target).abs())
                    .unwrap()
            })
            .map(|(i, _)| i)
            .unwrap()
    };

    println!("U-238 MT=91 pdf-weighted <mu_cm> against incident energy:");
    let mut profile = Vec::new();
    for &target in &[4.356e5_f64, 1.02e6, 3.0e6, 8.5e6, 1.4e7, 2.0e7] {
        let i = find(target);
        let mubar = pdf_weighted_mubar(law, 0, i).expect("Legendre law");
        println!(
            "  E_in = {:>10.4e} eV   <mu_cm> = {:+.6}",
            incident[i], mubar
        );
        profile.push((incident[i], mubar));
    }

    // Near threshold the evaluation is flat. This is the assertion that makes a
    // null result at 2 MeV *interpretable* rather than alarming, and it is a
    // real property of the tape rather than a tolerance chosen to pass.
    let (e_thr, mu_thr) = profile[0];
    assert!(
        mu_thr.abs() < 1.0e-6,
        "at {e_thr:.4e} eV (MT=91 threshold) the law should be exactly isotropic; got \
         <mu_cm> = {mu_thr:+.8}"
    );

    // By 14 MeV it is strongly forward-peaked.
    let (e_14, mu_14) = profile[4];
    assert!(
        mu_14 > 0.15,
        "at {e_14:.4e} eV the law should be strongly forward-peaked (measured +0.272 on \
         ENDF/B-VIII.0 in 2026-09); got <mu_cm> = {mu_14:+.6}. A value near zero means the \
         coefficients are not reaching the sampler; a mis-indexed incident-energy lookup would \
         also land here."
    );

    // And it rises monotonically through the high-energy range, which a
    // one-table-off indexing error would break even while the endpoints passed.
    for w in profile[2..].windows(2) {
        let ((e0, m0), (e1, m1)) = (w[0], w[1]);
        assert!(
            m1 > m0,
            "<mu_cm> must rise with incident energy, but {e0:.3e} eV gives {m0:+.6} and \
             {e1:.3e} eV gives {m1:+.6}"
        );
    }

    // 3: the sampler inverts the law it was given. Take the most anisotropic
    // row of the 14 MeV table and check the sampled mean reproduces its own a1,
    // which comes straight off the tape and never passes through the
    // linearisation -- so this tests the tabulation AND the inversion, not just
    // that a number comes back.
    let i14 = find(1.4e7);
    let ContinuumAngular::Legendre(tabs) = &law.branches[0].angular else {
        unreachable!("checked above")
    };
    let t = &tabs[i14];
    let (k, row) = t
        .rows
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.mubar.abs().partial_cmp(&b.mubar.abs()).unwrap())
        .expect("the 14 MeV table has rows");
    let n = 200_000usize;
    let drawn: Vec<f64> = (0..n)
        .map(|j| row.sample_mu((j as f64 + 0.5) / n as f64))
        .collect();
    let sampled = mean(&drawn);
    println!(
        "  sampler check: 14 MeV row {k}, tape a1 = {:+.6}, sampled <mu> = {sampled:+.6} \
         ({n} stratified draws)",
        row.mubar
    );
    assert!(
        (sampled - row.mubar).abs() < 0.01,
        "the sampled mean cosine {sampled:+.6} does not reproduce the row's own a1 = {:+.6} \
         (tape value, never passed through the linearisation). Either the Legendre series is \
         being linearised wrongly or the CDF inversion is wrong.",
        row.mubar
    );
    for &m in &drawn {
        assert!(
            (-1.0..=1.0).contains(&m) && m.is_finite(),
            "sampler produced a non-physical cosine {m}"
        );
    }
}

/// The ablation removes exactly the angular law: directions change, the random
/// stream does not.
#[test]
fn u238_continuum_angular_ablation_changes_angles_and_nothing_else() {
    let Some(tape) = reference_file_or_skip(
        "endf",
        "n-092_U_238.endf",
        "U-238 evaluation (continuum angular ablation control)",
    ) else {
        return;
    };

    let nuc = Nuclide::from_endf_file(&tape, "U238", TEMP_K, 1.0e-3).expect("U-238 reconstructs");
    let law = nuc.continuum_law(MT_91).expect("U-238 carries MT=91 MF=6");

    println!("U-238 MT=91 continuum angular ablation, {DRAWS} draws per arm:");
    println!(
        "  {:>10} {:>14} {:>14} {:>22} {:>10}",
        "E [eV]", "<mu> evaluated", "<mu> ablated", "difference", "seeds match"
    );

    // 2 MeV: the law is flat there, so the arms must AGREE -- a case where "no
    // difference" is the correct answer, included so the test cannot be read as
    // demanding a difference everywhere.
    // 14 MeV: the law is strongly forward-peaked, so the arms must DIFFER.
    let mut resolved_at_high_energy = false;
    for &e_in in &[2.0e6_f64, 1.4e7_f64] {
        let seed = 0x00C0_DEu64.wrapping_mul(e_in as u64).wrapping_add(1);
        let (mu_eval, _e_eval, s_eval) = sample_arm(
            law,
            AWR_U238,
            Q_MT91_EV,
            e_in,
            ContinuumAngularMode::Evaluated,
            DRAWS,
            seed,
        );
        let (mu_abl, _e_abl, s_abl) = sample_arm(
            law,
            AWR_U238,
            Q_MT91_EV,
            e_in,
            ContinuumAngularMode::IsotropicAblation,
            DRAWS,
            seed,
        );

        let (m_e, m_a) = (mean(&mu_eval), mean(&mu_abl));
        let d_sem = (sem(&mu_eval).powi(2) + sem(&mu_abl).powi(2)).sqrt();
        println!(
            "  {:>10.3e} {:>14.5} {:>14.5} {:>22} {:>10}",
            e_in,
            m_e,
            m_a,
            format!("{:+.5} +/- {:.5}", m_e - m_a, d_sem),
            s_eval == s_abl,
        );

        // 5: THE key invariant. The two arms must spend the same random variates
        // in the same order, so a paired worth measurement is attributable to
        // the physics and not to a re-randomised stream.
        //
        // Note this is asserted on the SEED, not on the outgoing lab energy.
        // The lab energy cannot be invariant: for a CM-frame law
        // E' = E_cm + E_trans + 2*mu_cm*sqrt(E_cm*E_trans), so changing the
        // cosine changes the energy by construction. An earlier draft asserted
        // the lab energy and failed 4091/4096 -- correctly.
        assert_eq!(
            s_eval, s_abl,
            "at {e_in:.3e} eV the two ablation arms left the RNG in different states \
             ({s_eval} vs {s_abl}). They must consume identical variates in identical order, or \
             any Delta-k measured from a paired run includes a re-randomisation artefact rather \
             than physics."
        );

        for (arm, v) in [("evaluated", &mu_eval), ("ablated", &mu_abl)] {
            for &m in v.iter() {
                assert!(
                    (-1.0..=1.0).contains(&m) && m.is_finite(),
                    "{arm} arm at {e_in:.3e} eV produced a non-physical lab cosine {m}"
                );
            }
        }

        if e_in > 1.0e7 {
            // 4: the control actually controls, where there is something to control.
            let changed = mu_eval
                .iter()
                .zip(&mu_abl)
                .filter(|(a, b)| a.to_bits() != b.to_bits())
                .count();
            assert!(
                changed > DRAWS / 2,
                "at {e_in:.3e} eV the ablation changed only {changed}/{DRAWS} cosines. A control \
                 that does not control reports 'no difference' and reads as 'this physics does \
                 not matter'."
            );
            // Forward-peaked: the evaluated arm's mean cosine must be resolvably
            // ABOVE the isotropic arm's. The sign matters -- it is the one that
            // increases leakage and so lowers k on a bare sphere.
            assert!(
                m_e - m_a > 3.0 * d_sem,
                "at {e_in:.3e} eV the evaluated arm's <mu_lab> = {m_e:+.5} is not resolvably \
                 above the isotropic arm's {m_a:+.5} (difference {:+.5}, 1 sigma {d_sem:.5}). \
                 U-238's MT=91 is forward-peaked in the CM at 14 MeV (<mu_cm> = +0.272); a null \
                 or negative difference means the coefficients are read but not applied, or \
                 applied with the wrong sign.",
                m_e - m_a
            );
            resolved_at_high_energy = true;
        }
    }

    assert!(
        resolved_at_high_energy,
        "the high-energy arm never ran, so nothing was actually controlled"
    );
}

/// F-19's MT=91 is isotropic on the tape, so the ablation must be a **no-op**
/// there — bit-identical in both energy and angle.
///
/// This is the negative control, and it is what separates "we read the
/// evaluation" from "we apply anisotropy everywhere". Without it, a sampler that
/// manufactured angular structure — a row-stride error, a coefficient vector
/// read one row late — would pass every assertion above, because those only ask
/// that *something* anisotropic comes out of U-238.
///
/// F-19 is also the nuclide whose inelastic channel carried the FHR pebble's
/// entire +4004 pcm residual (GitHub #193), so its continuum law is worth
/// pinning on its own account.
#[test]
fn f19_continuum_is_isotropic_so_the_ablation_is_a_no_op() {
    let Some(tape) = reference_file_or_skip(
        "endf",
        "n-009_F_019-ENDF8.0.endf",
        "F-19 evaluation (continuum angular negative control)",
    ) else {
        return;
    };

    let nuc = Nuclide::from_endf_file(&tape, "F19", TEMP_K, 1.0e-3).expect("F-19 reconstructs");
    let law = nuc
        .continuum_law(MT_91)
        .expect("F-19 carries an MF=6 LAW=1 law for MT=91");

    for (b, branch) in law.branches.iter().enumerate() {
        assert!(
            matches!(branch.angular, ContinuumAngular::EvaluatedIsotropic),
            "F-19 MT=91 branch {b} reached transport as {:?}. The tape declares NA=0 on every \
             one of its 13 incident energies, so anything else means the reader is \
             manufacturing angular structure -- most likely a row-stride error.",
            branch.angular,
        );
    }

    // 8 MeV: above F-19's MT=91 threshold of 5.9368 MeV.
    let e_in = 8.0e6_f64;
    let seed = 0xF19u64;
    let (mu_e, e_e, s_e) = sample_arm(
        law,
        AWR_F19,
        Q_F19_MT91_EV,
        e_in,
        ContinuumAngularMode::Evaluated,
        DRAWS,
        seed,
    );
    let (mu_a, e_a, s_a) = sample_arm(
        law,
        AWR_F19,
        Q_F19_MT91_EV,
        e_in,
        ContinuumAngularMode::IsotropicAblation,
        DRAWS,
        seed,
    );

    let identical = mu_e
        .iter()
        .zip(&mu_a)
        .zip(e_e.iter().zip(&e_a))
        .filter(|((ma, mb), (ea, eb))| ma.to_bits() == mb.to_bits() && ea.to_bits() == eb.to_bits())
        .count();
    println!("F-19 MT=91 at {e_in:.3e} eV: {identical}/{DRAWS} samples identical across ablation");
    assert_eq!(s_e, s_a, "F-19 arms left the RNG in different states");
    assert_eq!(
        identical, DRAWS,
        "F-19's MT=91 is isotropic on the tape, so switching the angular law off must change \
         NOTHING. {identical}/{DRAWS} matched, so the evaluated path is applying angular \
         structure F-19 does not have."
    );
}
