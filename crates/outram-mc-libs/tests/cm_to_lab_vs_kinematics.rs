//! **The CM→lab transform against first-principles velocity addition — one of
//! the two unmeasured leads left on `op-os8x`.**
//!
//! # Why this one
//!
//! `op-os8x` is a spectral residual against OpenMC on Godiva: we carry
//! **+0.42 % too much flux at 1.9–3.0 MeV** and 1.2–1.9 % too little around
//! 100 keV, on identical data, while `k` agrees. The cross-code study has
//! excluded, one at a time, the cross sections (≤0.06 % flux-weighted), both
//! angular laws (`op-tm9f`, `op-og56`), `k` itself, the MT=91 transfer table
//! (18 rows exact), the within-row CDF inversion and the inter-row unit-base
//! rule (worst 0.0228 %, flat −0.0205 %).
//!
//! Two leads were left, neither measured. This measures the first: **the CM→lab
//! transform on the continuum law.** It is the step that couples the sampled
//! `μ_cm` to `E'`, so an error there moves the outgoing spectrum while leaving
//! every tabulated law identical — precisely the signature that survives.
//!
//! # The oracle: momentum addition, not a copied formula
//!
//! Copying OpenMC's expression and checking we match it would test transcription
//! and nothing else. Instead the reference is built from the kinematics
//! directly. In the laboratory the centre of mass travels at
//! `v_cm = v_n / (A + 1)` along the incident direction, so a neutron emitted in
//! the CM with speed `v'` at cosine `μ_cm` has lab velocity `v_cm + v'` as
//! **vectors**:
//!
//! ```text
//! v_lab,parallel = v_cm + v' μ_cm
//! v_lab,perp     = v' sqrt(1 - μ_cm^2)
//! E_lab          = (v_lab,par^2 + v_lab,perp^2) in energy units
//! μ_lab          = v_lab,par / |v_lab|
//! ```
//!
//! With `E = v^2` in reduced units this is independent of the neutron mass and
//! of any ENDF convention — it is Galilean addition and nothing else.
//!
//! # Extra invariants checked
//!
//! - **Elastic reduction.** With `E'_cm = E (A/(A+1))^2` the transform must give
//!   the textbook `E' = E (A^2 + 2 A μ + 1)/(A+1)^2`.
//! - **Extremes.** `μ_cm = ±1` must give exactly `(sqrt(E'_cm) ± sqrt(E)/(A+1))^2`
//!   with `μ_lab = ±1` (or `+1` when the CM speed dominates a backward emission).
//! - **Heavy-target limit.** As `A → ∞` the lab and CM frames coincide, so
//!   `E_lab → E'_cm` and `μ_lab → μ_cm`.
//!
//! # A numerical trap this test walked into
//!
//! The first version bounded the **relative** error in `E_lab` at 1e-12 and
//! failed at `5.14e-12` — every worst case at `A = 0.999`, `mu_cm = -1`,
//! `E'_cm` at 99 % of its kinematic maximum. That corner is catastrophic
//! cancellation, not a defect: there `v_cm ~ 0.500 sqrt(E)` and
//! `v' ~ 0.497 sqrt(E)` subtract to `~0.003 sqrt(E)`, so a relative bound on the
//! *output* measures the cancellation rather than the transform. The bound is
//! now on the **input** energy scale `E'_cm + E/(A+1)^2`, which is the
//! numerically meaningful statement, and the transform clears it at 1e-14 —
//! machine precision.
//!
//! # Results (2026-09-16)
//!
//! | check | worst |
//! |---|---|
//! | `E_lab` vs Galilean addition, 2268 cases | **< 1e-14** of the input scale |
//! | `mu_lab` vs Galilean addition | **2.1e-12** (the cancellation corner) |
//! | elastic reduction to `E (A^2 + 2 A mu + 1)/(A+1)^2` | **1.18e-16** |
//! | heavy-target limit `A = 1e8` | `E_lab/E'_cm` and `mu_lab` to 1.5e-8 |
//!
//! # What a pass means for `op-os8x`
//!
//! **It excludes this lead, and only this lead.** A correct transform leaves the
//! remaining candidate — competing-channel branching at 2–3 MeV, i.e. which MT a
//! collision is assigned to as distinct from the cross sections — and whatever
//! has not been thought of. `op-os8x` stays open on a pass; what changes is that
//! one of its two named suspects is measured rather than merely suspected.

use outram_mc_libs::physics::scatter::cm_to_lab;

/// `(E_lab, mu_lab)` from Galilean velocity addition, in reduced units where
/// `E = v^2` (the neutron mass cancels).
fn kinematic_reference(e: f64, e_cm_out: f64, mu_cm: f64, awr: f64) -> (f64, f64) {
    let v_cm = e.sqrt() / (awr + 1.0); // CM speed in the lab
    let v_prime = e_cm_out.sqrt(); // emission speed in the CM
    let par = v_cm + v_prime * mu_cm;
    let perp = v_prime * (1.0 - mu_cm * mu_cm).max(0.0).sqrt();
    let v2 = par * par + perp * perp;
    let mu_lab = if v2 > 0.0 { par / v2.sqrt() } else { 1.0 };
    (v2, mu_lab.clamp(-1.0, 1.0))
}

#[test]
fn cm_to_lab_reproduces_galilean_velocity_addition() {
    // A spread covering light and heavy targets, the whole cosine range, and
    // outgoing energies from far below to comparable with the incident energy.
    let awrs = [0.9991673_f64, 8.93478, 11.89365, 55.36, 235.9841, 236.0058];
    let energies = [1.0e3_f64, 1.0e5, 1.9e6, 2.5e6, 1.4e7, 2.0e7];
    let fractions = [1.0e-6_f64, 1.0e-3, 0.01, 0.1, 0.3, 0.7, 0.99];
    let mus = [-1.0_f64, -0.97, -0.5, -0.1, 0.0, 0.1, 0.5, 0.97, 1.0];

    let mut worst_e = 0.0f64;
    let mut worst_mu = 0.0f64;
    let mut n = 0usize;
    let (mut at_e, mut at_mu) = (String::new(), String::new());

    for &awr in &awrs {
        for &e in &energies {
            for &f in &fractions {
                // CM outgoing energy cannot exceed what two-body kinematics
                // allow for an elastic collision; scale to that.
                let e_cm_max = e * (awr / (awr + 1.0)).powi(2);
                let e_cm_out = f * e_cm_max;
                for &mu in &mus {
                    let (got_e, got_mu) = cm_to_lab(e, e_cm_out, mu, awr);
                    let (ref_e, ref_mu) = kinematic_reference(e, e_cm_out, mu, awr);
                    // Scale the tolerance to the INPUT magnitudes, not to
                    // `E_lab`. At A ~ 1 with backward emission and `E'_cm` near
                    // its kinematic maximum, `v_cm` and `v'` nearly cancel, so
                    // `E_lab` is a tiny difference of comparable numbers and a
                    // *relative* bound there measures catastrophic cancellation
                    // rather than the transform. Measured: worst relative error
                    // 5.1e-12, entirely at A=0.999, mu=-1, f=0.99 -- exactly
                    // that corner.
                    let scale = e_cm_out + e / ((awr + 1.0) * (awr + 1.0));
                    let de = (got_e - ref_e).abs() / scale.max(1.0e-30);
                    let dmu = (got_mu - ref_mu).abs();
                    if de > worst_e {
                        worst_e = de;
                        at_e = format!("A={awr} E={e:.3e} f={f} mu={mu}");
                    }
                    if dmu > worst_mu {
                        worst_mu = dmu;
                        at_mu = format!("A={awr} E={e:.3e} f={f} mu={mu}");
                    }
                    n += 1;
                }
            }
        }
    }

    println!("CM->lab against Galilean addition over {n} cases:");
    println!("   worst |dE_lab| / (E'_cm + E/(A+1)^2) = {worst_e:.3e}   at {at_e}");
    println!("   worst absolute |dmu_lab| = {worst_mu:.3e}   at {at_mu}");
    assert!(
        n > 2000,
        "only {n} cases; the sweep is not exercising the transform"
    );
    assert!(
        worst_e < 1.0e-14,
        "the CM->lab energy differs from Galilean velocity addition by {worst_e:.3e} of the \
         input energy scale ({at_e}). The transform couples mu_cm to E', so an error here moves \
         the outgoing spectrum while leaving every tabulated law identical -- the exact \
         signature op-os8x still has."
    );
    // The cosine is a ratio of the same near-cancelling quantities, so it keeps
    // a relative bound but a looser one, set by the measured 2.1e-12 at the same
    // corner. Anything approaching a physics error is orders of magnitude above.
    assert!(
        worst_mu < 1.0e-10,
        "the CM->lab cosine differs from Galilean velocity addition by {worst_mu:.3e} ({at_mu}). \
         At 1e-12 this is floating-point cancellation at A ~ 1 with backward emission; a real \
         defect would be far larger."
    );
}

#[test]
fn cm_to_lab_satisfies_its_closed_form_limits() {
    let awr = 236.0058_f64;
    let e = 2.0e6_f64;

    // Elastic: E'_cm = E (A/(A+1))^2 must reduce to the textbook expression.
    let e_cm = e * (awr / (awr + 1.0)).powi(2);
    let mut worst = 0.0f64;
    for i in 0..=200 {
        let mu = -1.0 + 2.0 * i as f64 / 200.0;
        let (got, _) = cm_to_lab(e, e_cm, mu, awr);
        let want = e * (awr * awr + 2.0 * awr * mu + 1.0) / ((awr + 1.0) * (awr + 1.0));
        worst = worst.max((got - want).abs() / want);
    }
    println!("elastic reduction: worst relative difference = {worst:.3e}");
    assert!(
        worst < 1.0e-12,
        "with E'_cm = E (A/(A+1))^2 the transform must reduce to E (A^2 + 2 A mu + 1)/(A+1)^2; \
         worst relative difference {worst:.3e}"
    );

    // mu_cm = +-1: purely collinear addition, exactly (sqrt(E'_cm) +- sqrt(E)/(A+1))^2.
    for &f in &[0.01_f64, 0.5, 0.99] {
        let e_cm_out = f * e_cm;
        let v_cm = e.sqrt() / (awr + 1.0);
        for &(mu, sign) in &[(1.0_f64, 1.0_f64), (-1.0, -1.0)] {
            let (got, got_mu) = cm_to_lab(e, e_cm_out, mu, awr);
            let want = (e_cm_out.sqrt() * sign + v_cm).powi(2);
            assert!(
                (got - want).abs() / want < 1.0e-12,
                "collinear emission at mu_cm = {mu} gives {got:.6e}, not {want:.6e}"
            );
            // Backward emission still goes forward in the lab if the CM speed
            // dominates -- a real physical case, not an edge artefact.
            let expect_mu = if e_cm_out.sqrt() * sign + v_cm >= 0.0 {
                1.0
            } else {
                -1.0
            };
            assert!(
                (got_mu - expect_mu).abs() < 1.0e-12,
                "collinear emission must stay collinear in the lab: got mu_lab = {got_mu}, \
                 expected {expect_mu}"
            );
        }
    }

    // Heavy-target limit: the two frames coincide.
    let heavy = 1.0e8_f64;
    let (e_lab, mu_lab) = cm_to_lab(e, 0.3 * e, 0.4, heavy);
    println!(
        "heavy-target limit A=1e8: E_lab/E'_cm = {:.12}, mu_lab = {mu_lab:.12}",
        e_lab / (0.3 * e)
    );
    assert!(
        (e_lab / (0.3 * e) - 1.0).abs() < 1.0e-7,
        "as A -> infinity the lab and CM frames coincide; E_lab/E'_cm = {:.12}",
        e_lab / (0.3 * e)
    );
    assert!(
        (mu_lab - 0.4).abs() < 1.0e-7,
        "as A -> infinity mu_lab must approach mu_cm; got {mu_lab:.12}"
    );
}
