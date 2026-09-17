//! **MF=6 LAW=7 is sampled by the transport kernel — measured, not asserted.**
//!
//! # What this closes
//!
//! Be-9 MT=16's emission law is ENDF MF=6 **LAW=7** (lab-frame angle-then-
//! energy). Until 2026-09-16 the parser for it never read a single record (it
//! followed ACER's internal-File-6 layout instead of ENDF-6 — see
//! `njoy-outram-park-fork`'s `tests/mf6_law7_mu_weights.rs`), so
//! `ContinuumEmission::from_endf_mf6` fell back and every (n,2n) neutron came
//! out of the Weisskopf evaporation stand-in, **isotropically**. The evaluation
//! carries a strongly forward-peaked law: `<mu>` is `+0.25` to `+0.58` across
//! its incident range.
//!
//! # Methodology
//!
//! Drive `continuum_inelastic_scatter_evaluated_with` directly with Be-9's
//! converted law at a fixed incident energy, incident direction `+z` so the
//! outgoing `w` component **is** the laboratory cosine (no projection, so no
//! chance of measuring a rotation artefact), and compare the sampled mean cosine
//! against the law's own pdf-weighted `<mu>` — a closed-form oracle computed
//! from the density, not a second estimate from samples.
//!
//! Run in both `ContinuumAngularMode` arms. The ablation arm is the control: it
//! must come back statistically isotropic, **and must leave the RNG stream
//! exactly where the evaluated arm left it**, since both spend one variate on
//! the cosine. Without that invariant a measured difference could be
//! re-randomisation rather than physics.
//!
//! # Results (2026-09-16, ENDF/B-VIII.0 Be-9, 200 000 collisions at 14 MeV)
//!
//! | quantity | value |
//! |---|---|
//! | law's own `<mu>`, closed form | **+0.253844** |
//! | evaluated arm, sampled | **+0.254104 +- 0.001210** (0.21 sigma from the oracle) |
//! | isotropic-ablation arm, sampled | **+0.001801 +- 0.001290** (1.4 sigma from zero) |
//! | final RNG seed, both arms | identical |
//!
//! So a law carrying `<mu> = +0.25` in the laboratory was being emitted
//! isotropically before this change.
//!
//! **One correction worth keeping.** The first version of this test weighted the
//! oracle by `pdf[k]`, copying the older Legendre control, and read **3.30
//! sigma** — close enough to pass a 4 sigma gate and wrong. The sampler selects
//! row `k` with probability `cdf[k+1] - cdf[k]`, not `pdf[k]`, because
//! `sample_ct_table_indexed` returns the lower edge of the CDF bin. Weighting it
//! the way the code actually behaves gives 0.21 sigma. The defect was in the
//! oracle, not the sampler, and a looser gate would have buried the distinction
//! instead of exposing it. (The older control is unaffected: it uses its
//! pdf-weighted value to *characterise the law's shape* against incident energy,
//! not to judge a sampled mean.)
//!
//! # What this does NOT claim
//!
//! It is a kernel check, not physics validation. Be-9 appears in none of this
//! workspace's criticality cases, so nothing here moves a `k_eff`; what it
//! establishes is that the law reaches the sampler and is drawn from correctly.
//! The frame matters and is asserted: LAW=7 is laboratory-frame by definition,
//! so a `cm_frame` law here would be put through a CM->lab transform it must
//! never see.

use njoy_outram_park_fork::nuclear_data::secondary::{ContinuumAngular, ContinuumEmission};
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::physics::scatter::{
    continuum_inelastic_scatter_evaluated_with, ContinuumAngularMode,
};

fn z_dir() -> Direction {
    Direction {
        u: 0.0,
        v: 0.0,
        w: 1.0,
    }
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

fn sem(v: &[f64]) -> f64 {
    let m = mean(v);
    let var = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (v.len() as f64 - 1.0);
    (var / v.len() as f64).sqrt()
}

/// The law's own `<mu>` at one incident table — the closed-form oracle the
/// sampled mean is judged against.
///
/// # This weights by CDF bin, not by pdf, and the difference is not cosmetic
///
/// The sampler returns the **lower edge** of the CDF bin its uniform draw landed
/// in (`sample_ct_table_indexed`), and uses *that* row's cosine law. So row `k`
/// is selected with probability `cdf[k+1] - cdf[k]`, and the mean cosine a
/// neutron actually experiences is that discrete weighting — not the continuous
/// pdf-weighted average of `mubar` over `E'`.
///
/// Weighting by `pdf[k]` instead (as the older Legendre control does) is a
/// different quantity. It happens to be close, but on Be-9 at 14 MeV the two
/// differ by about 3 sigma at 200 000 collisions, which would read as a sampler
/// defect at a tighter gate or a larger sample. An oracle should describe what
/// the code does; making it agree by loosening the gate would hide exactly the
/// kind of off-by-one-row error this test exists to catch.
fn oracle_mubar(law: &ContinuumEmission, table: usize) -> f64 {
    let b = &law.branches[0];
    let ContinuumAngular::LabTabulated(tabs) = &b.angular else {
        panic!("Be-9 MT=16 should convert to ContinuumAngular::LabTabulated");
    };
    let t = &tabs[table];
    let ce = &b.spectrum.tables[table];
    let n = ce.cdf.len();
    let (mut num, mut den) = (0.0, 0.0);
    // k runs over the bins the search can leave `k` on: 0 ..= n-2.
    for k in 0..n.saturating_sub(1) {
        let w = ce.cdf[k + 1] - ce.cdf[k];
        num += w * t.rows[k].mubar;
        den += w;
    }
    num / den
}

#[test]
fn be9_law7_reaches_the_sampler_and_is_forward_peaked() {
    let Some(p) = reference_endf_or_skip("n-004_Be_009-ENDF8.0.endf", "Be-9 (MF=6 LAW=7)") else {
        return;
    };
    let tape = njoy_outram_park_fork::endf::tape::Tape::read_file(&p).expect("Be-9 tape parses");
    let mat = tape.materials()[0];
    let law = ContinuumEmission::from_endf_mf6(&tape, mat, 16)
        .expect("MF=6 parses")
        .expect("Be-9 MT=16 LAW=7 must now convert rather than fall back");

    assert!(
        !law.cm_frame,
        "LAW=7 is laboratory-frame by definition; a CM law here would be transformed twice"
    );

    // Be-9: AWR from the tape's MF=3 MT=16 head; Q is unused on the lab path but
    // the signature takes it.
    let awr = 8.934780_f64;
    let q = -1.665_0e6;

    // Sample at the incident energy nearest 14 MeV, and take the oracle from
    // that same table so the two are the same point of the law.
    let e_in = 14.0e6_f64;
    let table = law.branches[0]
        .spectrum
        .incident
        .iter()
        .enumerate()
        .min_by(|a, b| (a.1 - e_in).abs().partial_cmp(&(b.1 - e_in).abs()).unwrap())
        .map(|(i, _)| i)
        .expect("incident grid is non-empty");
    let e_in = law.branches[0].spectrum.incident[table];
    let expect = oracle_mubar(&law, table);

    const N: usize = 200_000;
    const SEED: u64 = 0x5eed_1a77;

    let mut arms = Vec::new();
    for mode in [
        ContinuumAngularMode::Evaluated,
        ContinuumAngularMode::IsotropicAblation,
    ] {
        let mut s = SEED;
        let mut mus = Vec::with_capacity(N);
        for _ in 0..N {
            let (_e_out, u_out) = continuum_inelastic_scatter_evaluated_with(
                e_in,
                z_dir(),
                awr,
                q,
                Some(&law),
                mode,
                &mut s,
            );
            mus.push(u_out.w);
        }
        arms.push((mode, mean(&mus), sem(&mus), s));
    }

    println!(
        "Be-9 MT=16 (MF=6 LAW=7) at E_in = {:.4} MeV, {N} collisions:",
        e_in / 1.0e6
    );
    println!("   law's own <mu> (closed form, CDF-bin weighted) = {expect:+.6}");
    for (mode, m, s, _) in &arms {
        println!("   {mode:?}: sampled <mu> = {m:+.6} +- {s:.6}");
    }

    let (_, ev_mean, ev_sem, ev_seed) = arms[0];
    let (_, ab_mean, ab_sem, ab_seed) = arms[1];

    // The RNG-stream invariant. Without it the difference below could be
    // re-randomisation rather than the angular law.
    assert_eq!(
        ev_seed, ab_seed,
        "the two arms ended on different RNG seeds, so they did not spend the same number of \
         variates. Any measured difference between them would then be partly re-randomisation, \
         not the angular physics."
    );

    // The evaluated arm reproduces the law's own closed-form mean.
    let z = (ev_mean - expect).abs() / ev_sem;
    println!("   evaluated arm vs oracle: {z:.2} sigma");
    assert!(
        z < 4.0,
        "sampled <mu> = {ev_mean:+.6} +- {ev_sem:.6} is {z:.2} sigma from the law's own \
         closed-form {expect:+.6}. The sampler is not drawing from the distribution the \
         conversion built."
    );

    // The law is substantially forward-peaked -- this is what was being thrown
    // away, and the number that makes the fix worth having.
    assert!(
        expect > 0.15,
        "Be-9's LAW=7 came back with <mu> = {expect:+.6}, near isotropic. The evaluation is \
         forward-peaked (+0.25 to +0.58 across its range); a flat result means the per-cosine \
         weights are not reaching the conditional."
    );

    // The ablation arm is the control: statistically isotropic.
    assert!(
        ab_mean.abs() < 4.0 * ab_sem,
        "the isotropic ablation arm returned <mu> = {ab_mean:+.6} +- {ab_sem:.6}, which is not \
         consistent with zero. The control is not controlling."
    );
}
