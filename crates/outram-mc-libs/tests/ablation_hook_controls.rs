//! **Every `Nuclide` ablation hook actually ablates, and ablates only what it
//! claims.** The controls the coverage survey
//! (`docs/neutronics-physics-coverage.md`) found missing.
//!
//! # Why an unguarded ablation hook is worse than no hook
//!
//! An ablation that silently fails to take effect reports "no difference", and
//! that reads as **"this physics does not matter"**. It is the worst failure
//! mode an ablation study has, because the null result is indistinguishable
//! from a real one and nobody re-runs a measurement that already produced a
//! number.
//!
//! This crate has hit it. Bead `op-50vu` records free-gas and bound-thermal
//! eigenvalues coming out **bit-identical** because the S(α,β) wiring was not
//! reaching the sampler at all — a whole scattering law absent, and the symptom
//! was two numbers agreeing.
//!
//! So each hook needs three assertions, and the first is the one usually
//! skipped:
//!
//! 1. **There was something to remove.** Assert the *unablated* nuclide carries
//!    the mechanism. Without this, a hook that removes nothing passes a
//!    difference test trivially when the data was already flat.
//! 2. **It is gone afterwards.**
//! 3. **Nothing else moved.** Cross sections bit-identical across the ablation,
//!    so a measured `Δk` is attributable to the one mechanism and not to a
//!    second thing changing at the same time.
//!
//! # What this file covers, and why these two
//!
//! `tests/anisotropy_ablation_control.rs` and
//! `tests/inelastic_anisotropy_ablation_control.rs` already guard the elastic
//! and discrete-inelastic angular hooks. The survey found two hooks with **no
//! control at their own level**:
//!
//! - **[`Nuclide::with_isotropic_continuum_scattering`]** — what
//!   `examples/godiva_continuum_anisotropy_ablation.rs` actually calls to
//!   produce its `−38 ± 23 pcm`. Its existing control
//!   (`tests/continuum_angular_ablation_control.rs`) tests
//!   `ContinuumAngularMode`, the enum handed to the *scatter function* — a
//!   different code path. Only the untested one produces a published number.
//! - **[`Nuclide::without_inelastic`]** — removes the largest single reactivity
//!   mechanism this crate has measured (−4190 pcm on the FHR pebble, the row
//!   that localised GitHub #193 to F-19's inelastic channel). A silent failure
//!   there would have derailed that investigation.
//! - **[`Nuclide::with_target_at_rest`]** — added the same day the survey found
//!   free-gas target motion had **no in-process hook at all**, only a
//!   process-wide environment variable in one example. Worth **−2242 pcm** on
//!   the FHR pebble, the positive control that says that pricing table's
//!   instrument can register a large effect.
//!
//! # Results (2026-09-16, ENDF/B-VIII.0 U-238 at 293.6 K, `cargo test --release`)
//!
//! Printed by each test; 5 passed, 0 failed.
//!
//! - **`with_isotropic_continuum_scattering`** — U-238 carries a continuum
//!   angular law on MT=91 and MT=16 before ablation and neither after, with the
//!   MT=91/16 *energy* laws intact and cross sections bit-identical.
//! - **`without_inelastic`** — inelastic `3.14977 b -> 0` at 2.0e6 eV while the
//!   **total is held at 7.28152 b**: the hook changes the reaction *partition*,
//!   not the collision rate, exactly as its doc says. No sampled outcome falls
//!   below the elastic floor `α·E = 1.9664e6 eV` afterwards.
//! - **`with_target_at_rest`** — `kT 2.530049e-2 -> 0` eV; up-scatter
//!   **4207/8192 -> 0/8192** at 0.0253 eV; ablated outcomes confined to
//!   `[2.487485e-2, 2.529998e-2]` eV, inside `[α·E, E]` with `α = 0.98319`;
//!   cross sections bit-identical. Above the `1.0120e1` eV free-gas threshold
//!   it is a **no-op to the bit**: 2048/2048 paired draws at 2 MeV identical,
//!   RNG streams in lockstep.
//!
//! **Verification, not validation.** These assert that a switch switches. What
//! the mechanism is *worth* is a separate paired-seed measurement.

use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use outram_mc_libs::material::nuclide::Nuclide;

const TEMP_K: f64 = 293.6;
/// Inside Godiva's flux, above U-238's MT=51 threshold (44.9 keV) and its MT=91
/// threshold (435.6 keV), so every inelastic channel is open.
const PROBE_EV: f64 = 2.0e6;

fn u238() -> Option<Nuclide> {
    let tape = reference_file_or_skip(
        "endf",
        "n-092_U_238.endf",
        "U-238 evaluation (ablation hook controls)",
    )?;
    Some(Nuclide::from_endf_file(&tape, "U238", TEMP_K, 1.0e-3).expect("U-238 reconstructs"))
}

/// Sample one inelastic outcome's outgoing energy, or `None` when the nuclide
/// has no inelastic channel left to sample.
///
/// Used to check the *behavioural* half of `without_inelastic`: the cross
/// section going to zero is bookkeeping, but what the ablation is for is that
/// no collision can shed excitation energy afterwards.
fn sample_inelastic_outgoing(nuc: &Nuclide, e: f64, seed: &mut u64) -> Option<f64> {
    use outram_mc_libs::material::nuclide::Inelastic;
    use outram_mc_libs::geometry::position::Direction;
    use outram_mc_libs::physics::scatter::{two_body_scatter, continuum_inelastic_scatter_evaluated};
    if nuc.xs_at_energy(e, TEMP_K).inelastic <= 0.0 {
        return None;
    }
    let u = Direction::new(0.0, 0.0, 1.0);
    Some(match nuc.sample_inelastic(e, seed) {
        Inelastic::Level { q, .. } => two_body_scatter(e, u, nuc.awr, q, seed).0,
        Inelastic::Continuum { q } => {
            continuum_inelastic_scatter_evaluated(e, u, nuc.awr, q, nuc.continuum_law(91), seed).0
        }
    })
}

/// Assert an ablation changed **nothing** about the cross sections.
///
/// Bit-identical, not "close": these hooks are defined to touch a sampling law
/// and nothing else, so any difference at all is a defect. A `Δk` measured
/// across a hook that also perturbs σ would be attributing a cross-section
/// change to the mechanism under test.
fn assert_cross_sections_unchanged(before: &Nuclide, after: &Nuclide, who: &str) {
    for &e in &[1.0e-2_f64, 1.0, 1.0e3, 1.0e5, PROBE_EV, 1.4e7] {
        let (a, b) = (before.xs_at_energy(e, TEMP_K), after.xs_at_energy(e, TEMP_K));
        for (label, x, y) in [
            ("total", a.total, b.total),
            ("elastic", a.elastic, b.elastic),
            ("absorption", a.absorption, b.absorption),
            ("fission", a.fission, b.fission),
        ] {
            assert_eq!(
                x.to_bits(),
                y.to_bits(),
                "{who} changed the {label} cross section at {e:.3e} eV ({x} -> {y}). This hook \
                 is defined to change a sampling law and nothing else; a Delta-k measured across \
                 it would be attributing a cross-section change to the mechanism under test."
            );
        }
    }
}

/// `with_isotropic_continuum_scattering` removes the continuum angular law,
/// removes only it, and had something to remove.
///
/// This is the hook `examples/godiva_continuum_anisotropy_ablation.rs` calls to
/// produce its published `−38 ± 23 pcm`. Until this test it was unguarded at
/// its own level.
#[test]
fn the_continuum_angular_hook_ablates_exactly_the_angular_law() {
    let Some(evaluated) = u238() else { return };

    // 1: there was something to remove. Without this the test passes trivially
    // on a nuclide whose evaluation happened to be isotropic (F-19's MT=91 is
    // exactly that, which is why the assertion is not optional).
    assert!(
        evaluated.has_continuum_anisotropy(),
        "U-238 reports no continuum angular law before ablation. Either the MF=6 coefficients \
         stopped being read (bead op-og56) or the tape changed -- and every ablation of this \
         mechanism would then be measuring nothing while reporting a number."
    );

    let ablated = evaluated.clone().with_isotropic_continuum_scattering();

    // 2: it is gone.
    assert!(
        !ablated.has_continuum_anisotropy(),
        "with_isotropic_continuum_scattering left a continuum angular law in place. The hook is \
         a no-op, and the -38 +/- 23 pcm measured through it would be an artefact of nothing."
    );

    // 3: nothing else moved.
    assert_cross_sections_unchanged(
        &evaluated,
        &ablated,
        "with_isotropic_continuum_scattering",
    );

    // The energy law must survive: this ablates the ANGLE only. If the
    // outgoing-energy law went with it, the two arms would differ in how far
    // neutrons are thrown as well as where, and the measurement would be
    // uninterpretable.
    for mt in [91, 16] {
        let before = evaluated.continuum_law(mt).is_some();
        let after = ablated.continuum_law(mt).is_some();
        assert_eq!(
            before, after,
            "MT={mt}: the continuum ENERGY law's presence changed across the angular ablation \
             ({before} -> {after}). This hook must touch the angle only -- `without_evaluated_\
             continuum` is the hook for the energy law."
        );
    }
    println!(
        "with_isotropic_continuum_scattering: U-238 anisotropic before, isotropic after, \
         MT=91/16 energy laws intact, cross sections bit-identical"
    );
}

/// `without_inelastic` removes the inelastic channel, removes only it, and had
/// something to remove.
///
/// The hook behind GitHub #193's pricing table, where switching the inelastic
/// channel off was worth **−4190 pcm** on the FHR pebble and localised the
/// defect to one nuclide. That table is only as good as this switch.
#[test]
fn the_inelastic_hook_ablates_exactly_the_inelastic_channel() {
    let Some(evaluated) = u238() else { return };

    // 1: there was something to remove.
    let xs_before = evaluated.xs_at_energy(PROBE_EV, TEMP_K);
    assert!(
        xs_before.inelastic > 0.0,
        "U-238 has no inelastic cross section at {PROBE_EV:.1e} eV before ablation \
         (got {}), so there is nothing for `without_inelastic` to remove and any price \
         measured through it is meaningless.",
        xs_before.inelastic
    );

    let ablated = evaluated.clone().without_inelastic();
    let xs_after = ablated.xs_at_energy(PROBE_EV, TEMP_K);

    // 2: it is gone.
    assert_eq!(
        xs_after.inelastic, 0.0,
        "without_inelastic left {} b of inelastic cross section at {PROBE_EV:.1e} eV. The hook \
         is a no-op, and GitHub #193's -4190 pcm row rests on it.",
        xs_after.inelastic
    );

    // 3: THE TOTAL MUST NOT MOVE. This is the hook's documented contract and
    // it is the opposite of what a first reading suggests, so it is asserted
    // explicitly rather than assumed:
    //
    //   "Only the reaction *partition*, never the total. The transport kernel
    //    splits a collision on `absorption | inelastic | (n,2n) | elastic`
    //    shares of `MicroXS::total`; with no levels, `MicroXS::inelastic` is
    //    zero and that share falls through to the elastic branch."
    //
    // So an inelastic collision becomes an ELASTIC one and the collision RATE
    // is held fixed. That is the right knob for pricing inelastic scattering
    // against a slowing-down residual: what is removed is the excitation energy
    // loss and nothing else. A hook that also dropped the total would confound
    // "inelastic loses energy" with "fewer collisions happen", and the measured
    // worth would be a mixture of the two.
    //
    // An earlier draft of this test asserted the total falls by the removed
    // inelastic. It failed -- correctly -- and the doc had said so all along.
    for (label, x, y) in [
        ("total", xs_before.total, xs_after.total),
        ("elastic", xs_before.elastic, xs_after.elastic),
        ("absorption", xs_before.absorption, xs_after.absorption),
        ("fission", xs_before.fission, xs_after.fission),
    ] {
        assert_eq!(
            x.to_bits(),
            y.to_bits(),
            "without_inelastic changed the {label} cross section at {PROBE_EV:.1e} eV \
             ({x} -> {y}). It must change the reaction PARTITION only: the total holds the \
             collision rate fixed, and elastic/absorption/fission are not its to touch."
        );
    }

    // 4: the behavioural consequence, which is what actually matters and what a
    // partition bug would break silently. With the levels gone, the inelastic
    // share falls through to elastic, and elastic off a heavy actinide can only
    // take the neutron down to `alpha*E` with
    // `alpha = ((A-1)/(A+1))^2 = 0.9832` for U-238. Before ablation an
    // inelastic collision drops it far below that.
    let alpha = ((evaluated.awr - 1.0) / (evaluated.awr + 1.0)).powi(2);
    let floor = alpha * PROBE_EV;
    let mut seed = 0xAB1A7Eu64;
    let mut n_below_elastic_floor = 0usize;
    for _ in 0..4096 {
        if let Some(e_out) = sample_inelastic_outgoing(&ablated, PROBE_EV, &mut seed) {
            if e_out < floor * 0.999 {
                n_below_elastic_floor += 1;
            }
        }
    }
    assert_eq!(
        n_below_elastic_floor, 0,
        "after without_inelastic, {n_below_elastic_floor}/4096 sampled inelastic outcomes fell \
         below the elastic floor alpha*E = {floor:.4e} eV (alpha = {alpha:.5}). The levels are \
         supposed to be gone, so no collision can shed excitation energy any more."
    );
    println!(
        "without_inelastic: U-238 inelastic {:.5} b -> 0 at {PROBE_EV:.1e} eV, total held at \
         {:.5} b (partition-only, as documented), no outcome below the elastic floor \
         {floor:.4e} eV",
        xs_before.inelastic, xs_before.total
    );
}

/// The two hooks are **independent** — each leaves the other's mechanism alone,
/// so they can be combined in one study without confounding.
///
/// Worth asserting because they touch neighbouring data: the continuum angular
/// law hangs off the same `ContinuumLaws` the inelastic channel feeds, and an
/// over-eager `without_inelastic` could plausibly take the angular law with it.
#[test]
fn the_two_hooks_do_not_interfere() {
    let Some(evaluated) = u238() else { return };

    // Ablating the angle must not touch the inelastic cross section.
    let angle_off = evaluated.clone().with_isotropic_continuum_scattering();
    assert_eq!(
        evaluated.xs_at_energy(PROBE_EV, TEMP_K).inelastic.to_bits(),
        angle_off.xs_at_energy(PROBE_EV, TEMP_K).inelastic.to_bits(),
        "the continuum ANGULAR ablation changed the inelastic cross section"
    );

    // Ablating the channel may remove the angular law with it (the channel is
    // gone, so its angle is moot) -- but it must not be an error, and combining
    // them must be stable rather than panicking or resurrecting anything.
    let both = evaluated.clone().without_inelastic().with_isotropic_continuum_scattering();
    assert_eq!(
        both.xs_at_energy(PROBE_EV, TEMP_K).inelastic,
        0.0,
        "combining both hooks left an inelastic cross section in place"
    );
    assert!(
        !both.has_continuum_anisotropy(),
        "combining both hooks left a continuum angular law in place"
    );
    assert_cross_sections_unchanged(
        &evaluated.clone().without_inelastic(),
        &both,
        "with_isotropic_continuum_scattering applied after without_inelastic",
    );
    println!("the two hooks compose without interfering");
}

// ───────────────────────── free-gas target motion ─────────────────────────
//
// Gap 3 of `docs/neutronics-physics-coverage.md`, closed 2026-09-16. Before
// this the mechanism was reachable only by zeroing one example's transport
// temperature through `OUTRAM_RINGRPT_TARGET_AT_REST`, a process-wide switch
// that cannot put two arms in one paired-seed study.

/// Thermal probe: `0.0253 eV` is **below** `FREE_GAS_THRESHOLD * kT`
/// (`400 * 0.0253 = 10.12 eV` at 293.6 K), so the free-gas kernel is live here
/// on every nuclide regardless of mass. Choosing a probe above that threshold
/// would have made the whole test vacuous — the production path already holds a
/// heavy target at rest up there, which is the separate assertion below.
const THERMAL_PROBE_EV: f64 = 0.0253;

/// Draw `n` free-gas elastic outcomes off `nuc` at energy `e`, through the same
/// call the transport drivers make, and return how many came out **above** the
/// incident energy.
///
/// Up-scatter is the signature of target motion and nothing else: a target at
/// rest can only take energy away, so `n_up > 0` on one arm and `n_up == 0` on
/// the other is the mechanism appearing and disappearing.
fn count_upscatter(nuc: &Nuclide, e: f64, n: usize, seed: &mut u64) -> usize {
    use outram_mc_libs::geometry::position::Direction;
    use outram_mc_libs::physics::scatter::free_gas_elastic_scatter;
    let u = Direction::new(0.0, 0.0, 1.0);
    let kt = nuc.free_gas_kt(TEMP_K);
    (0..n)
        .filter(|_| {
            let mu_cm = nuc
                .sample_elastic_mu_cm(e, seed)
                .unwrap_or_else(|| 2.0 * outram_mc_libs::rng::lcg::prn(seed) - 1.0);
            free_gas_elastic_scatter(e, u, nuc.awr, kt, mu_cm, seed).0 > e
        })
        .count()
}

/// `with_target_at_rest` removes free-gas target motion, removes only it, and
/// had something to remove.
///
/// The positive control in GitHub #193's pricing table — **−2242 pcm** on the
/// FHR pebble, the largest single effect in it, and the row that says the
/// harness can see a large effect at all. A table of null results is only worth
/// reading if the instrument that produced it can produce a non-null one, so
/// this switch working is load-bearing for every *other* row in that table.
#[test]
fn the_target_motion_hook_ablates_exactly_the_target_velocity() {
    let Some(evaluated) = u238() else { return };

    // 1: there was something to remove, asserted two ways.
    //
    // (a) The bookkeeping: the kinematics temperature is the material's.
    let kt_expected = 8.617_333_262e-5 * TEMP_K;
    assert!(
        (evaluated.free_gas_kt(TEMP_K) - kt_expected).abs() < 1.0e-18,
        "unablated free_gas_kt({TEMP_K}) = {} eV, expected {kt_expected} eV. The hook's whole \
         mechanism is this value, so a wrong one here makes the ablation measure something else.",
        evaluated.free_gas_kt(TEMP_K)
    );
    assert!(!evaluated.is_target_at_rest());

    // (b) The behaviour, which is what a wiring failure would break silently:
    // up-scatter must actually be reachable before ablation. Bead `op-50vu` is
    // exactly this assertion going unmade — the S(alpha,beta) wiring was not
    // reaching the sampler, and the symptom was two eigenvalues agreeing.
    let mut seed = 0x7A46E7u64;
    let up_before = count_upscatter(&evaluated, THERMAL_PROBE_EV, 8192, &mut seed);
    assert!(
        up_before > 0,
        "no up-scatter in 8192 free-gas collisions at {THERMAL_PROBE_EV} eV before ablation. \
         Either the free-gas kernel is not being reached or the threshold gate changed -- and \
         ablating target motion would then be removing nothing while reporting a number."
    );

    let ablated = evaluated.clone().with_target_at_rest();

    // 2: it is gone. Both the flag and the behaviour.
    assert!(ablated.is_target_at_rest());
    assert_eq!(
        ablated.free_gas_kt(TEMP_K).to_bits(),
        0.0_f64.to_bits(),
        "with_target_at_rest left a non-zero kinematics temperature, so free_gas_elastic_scatter \
         will still sample a target velocity and the ablation is a no-op."
    );
    let mut seed = 0x7A46E7u64;
    let up_after = count_upscatter(&ablated, THERMAL_PROBE_EV, 8192, &mut seed);
    assert_eq!(
        up_after, 0,
        "{up_after}/8192 collisions gained energy after with_target_at_rest. A target held at \
         rest can only take energy away; any up-scatter means target motion survived the hook."
    );

    // 3: nothing else moved. On the HIGH tier the cross sections were
    // Doppler-broadened at construction and do not depend on the transport
    // temperature at all, so this hook is kinematics-only by construction --
    // but that is the claim the measured Delta-k rests on, so it is asserted
    // rather than argued.
    assert_cross_sections_unchanged(&evaluated, &ablated, "with_target_at_rest");

    // 4: the ablated arm obeys the target-at-rest energy bounds exactly.
    // alpha = ((A-1)/(A+1))^2 = 0.9832 for U-238.
    use outram_mc_libs::geometry::position::Direction;
    use outram_mc_libs::physics::scatter::free_gas_elastic_scatter;
    let alpha = ((ablated.awr - 1.0) / (ablated.awr + 1.0)).powi(2);
    let (mut lo, mut hi) = (f64::INFINITY, 0.0_f64);
    let mut seed = 0x5EED_0001u64;
    for _ in 0..8192 {
        let mu_cm = ablated
            .sample_elastic_mu_cm(THERMAL_PROBE_EV, &mut seed)
            .unwrap_or_else(|| 2.0 * outram_mc_libs::rng::lcg::prn(&mut seed) - 1.0);
        let e_out = free_gas_elastic_scatter(
            THERMAL_PROBE_EV,
            Direction::new(0.0, 0.0, 1.0),
            ablated.awr,
            ablated.free_gas_kt(TEMP_K),
            mu_cm,
            &mut seed,
        )
        .0;
        lo = lo.min(e_out);
        hi = hi.max(e_out);
    }
    assert!(
        lo >= alpha * THERMAL_PROBE_EV * (1.0 - 1.0e-12) && hi <= THERMAL_PROBE_EV * (1.0 + 1.0e-12),
        "ablated outcomes spanned [{lo:.6e}, {hi:.6e}] eV, outside the target-at-rest window \
         [{:.6e}, {THERMAL_PROBE_EV:.6e}] eV (alpha = {alpha:.5}).",
        alpha * THERMAL_PROBE_EV
    );

    println!(
        "with_target_at_rest: U-238 kT {kt_expected:.6e} -> 0 eV; up-scatter {up_before}/8192 \
         -> {up_after}/8192 at {THERMAL_PROBE_EV} eV; ablated outcomes confined to \
         [{lo:.6e}, {hi:.6e}] eV within [alpha*E, E] (alpha = {alpha:.5}); cross sections \
         bit-identical"
    );
}

/// Above `FREE_GAS_THRESHOLD * kT` the hook is a **bit-for-bit no-op**, because
/// the production path already holds a heavy target at rest there.
///
/// This is not a nicety — it is what makes the hook's recorded prediction a
/// prediction. `with_target_at_rest` is expected to be worth ~nothing on a bare
/// fast metal sphere such as Godiva, whose flux is almost entirely above
/// `400 * kT` (10.12 eV at 293.6 K). That expectation is only sound if the two
/// arms are *identical* up there rather than merely similar, so a future Godiva
/// ablation reading non-zero means the wiring, not the physics.
#[test]
fn the_target_motion_hook_is_a_no_op_above_the_free_gas_threshold() {
    use outram_mc_libs::geometry::position::Direction;
    use outram_mc_libs::physics::scatter::{free_gas_elastic_scatter, FREE_GAS_THRESHOLD};
    let Some(evaluated) = u238() else { return };
    let ablated = evaluated.clone().with_target_at_rest();

    let kt = evaluated.free_gas_kt(TEMP_K);
    let threshold = FREE_GAS_THRESHOLD * kt;
    assert!(
        PROBE_EV > threshold,
        "probe {PROBE_EV:.3e} eV is not above the free-gas threshold {threshold:.3e} eV; \
         this test would be asserting the wrong branch."
    );

    // Paired seeds: identical stream in, identical stream out. Bit-identical,
    // not "close" -- above the threshold the two arms take the same branch of
    // `free_gas_elastic_scatter` and consume the same variates.
    let u = Direction::new(0.0, 0.0, 1.0);
    let mut n = 0usize;
    for i in 0..2048u64 {
        let seed0 = 0xF7EE_6A50u64.wrapping_add(i.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let (mut sa, mut sb) = (seed0, seed0);
        let mu_a = evaluated
            .sample_elastic_mu_cm(PROBE_EV, &mut sa)
            .unwrap_or_else(|| 2.0 * outram_mc_libs::rng::lcg::prn(&mut sa) - 1.0);
        let mu_b = ablated
            .sample_elastic_mu_cm(PROBE_EV, &mut sb)
            .unwrap_or_else(|| 2.0 * outram_mc_libs::rng::lcg::prn(&mut sb) - 1.0);
        let a = free_gas_elastic_scatter(PROBE_EV, u, evaluated.awr, kt, mu_a, &mut sa);
        let b = free_gas_elastic_scatter(
            PROBE_EV,
            u,
            ablated.awr,
            ablated.free_gas_kt(TEMP_K),
            mu_b,
            &mut sb,
        );
        assert_eq!(
            a.0.to_bits(),
            b.0.to_bits(),
            "draw {i}: outgoing energy differed above the free-gas threshold ({} vs {}). The \
             hook is documented as a no-op here; if it is not, its predicted ~zero worth on a \
             fast spectrum is unfounded.",
            a.0,
            b.0
        );
        assert_eq!(sa, sb, "draw {i}: the RNG streams diverged above the threshold");
        n += 1;
    }
    println!(
        "with_target_at_rest above the {threshold:.4e} eV threshold: {n}/{n} draws \
         bit-identical at {PROBE_EV:.1e} eV, RNG streams in lockstep"
    );
}
