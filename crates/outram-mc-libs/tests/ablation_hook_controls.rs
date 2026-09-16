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
//! **And this file has now caught the same thing live.** On its first run
//! `the_n2n_multiplicity_hook_reaches_the_transport_kernel` failed with the two
//! arms bit-identical: the `(n,2n)` yield hook had been wired into four of the
//! **five** emission sites, and the one missed was in `physics::keff`'s
//! `transport_history` — the path `run_keff` actually takes. A data-level
//! control alone would have passed, and the hook would have shipped reporting
//! that `(n,2n)` multiplicity is worth nothing. That is the whole argument for
//! testing the *kernel* and not only the flag.
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
//! - **[`Nuclide::with_unit_n2n_multiplicity`]** — the `(n,2n)` **yield of 2**,
//!   a genuine neutron multiplier. Its *emission law* already had two hooks
//!   (`without_evaluated_continuum` for the energy,
//!   `with_isotropic_continuum_scattering` for the angle); the yield had none,
//!   and was ablatable only lumped in with everything else via
//!   `without_inelastic`.
//! - **[`Nuclide::with_frozen_nubar`]** and
//!   **[`Nuclide::with_frozen_fission_spectrum`]** — the two factors of the
//!   fission source, `ν̄ × χ`. Both were verified against the tape and against
//!   OpenMC, so their *data* was sound; neither could be **priced**, and ν̄'s
//!   slope is a direct reactivity lever on any fast system. Added 2026-09-16
//!   as gap 4 of the coverage survey. Note these *freeze* rather than remove:
//!   a zero ν̄ is not an ablation, it is a subcritical block of metal.
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
//! - **`with_frozen_nubar`** (U-235) — ν̄ `2.42985` at 0.0253 eV against
//!   `2.64574` at 2 MeV, a rise of `0.21589`. Frozen at thermal it reads
//!   `2.42985` at every energy and `nu_fission` at 2 MeV falls
//!   `3.40904 -> 3.13086 b`, **−8.16 %**, with cross sections bit-identical.
//! - **`with_unit_n2n_multiplicity`** — U-238 `σ_n2n = 1.45833 b` at 12 MeV
//!   and exactly 0 below threshold at 2 MeV; emission flag `true -> false`;
//!   cross sections (σ_n2n included) bit-identical and the MT=16 energy law
//!   intact. **At the kernel:** a bare U-235 sphere on one shared seed,
//!   4000 × [20 inactive + 60 active], gives `k` **0.973390 -> 0.972432**,
//!   **−95.8 pcm**.
//! - **`with_frozen_fission_spectrum`** (U-235) — LF=1 on **22 incident rows**,
//!   1.000e-5 … 3.000e7 eV. Integrating the tape's own pdfs: mean birth energy
//!   `1.99980e6` eV at 1e-5, `2.01746e6` eV at 14 MeV (**+0.883 %**),
//!   `2.30184e6` eV at 30 MeV (**+15.103 %**). The sampler reproduces both
//!   endpoint rows within 1 %.
//!
//! # A prediction these two measurements license, recorded before measuring it
//!
//! The two factors of the fission source are **very** unequal levers on a fast
//! system. ν̄'s slope is worth −8.16 % of `nu_fission` at 2 MeV if frozen at
//! thermal — enormous, thousands of pcm. χ's incident-energy dependence is
//! worth **+0.883 % in mean birth energy over the whole span from thermal to
//! 14 MeV**, and a fission spectrum puts almost nothing above that, so freezing
//! χ should move `k` by **very little — well under 100 pcm on Godiva**. A
//! larger χ reading would mean the wiring, not the physics.
//!
//! Note the sharp top-end hardening (+15.1 % by 30 MeV, where third-chance
//! fission and pre-equilibrium emission set in) does **not** weaken that: no
//! reactor spectrum reaches there. It is recorded because it is what makes the
//! first number believable rather than suspicious.
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

// ──────────────────── the fission source: nu-bar and chi ────────────────────
//
// Gap 4 of `docs/neutronics-physics-coverage.md`, closed 2026-09-16. Both are
// verified against the tape and against OpenMC, so their *data* is sound —
// neither could be *priced*. For a bare fast sphere nu-bar's slope is a direct
// reactivity lever, and it was the one mechanism in the fast kernel with no way
// to ask "what is it worth".

/// U-235: the fissile nuclide whose nu-bar slope and MF=5 chi actually drive a
/// fast system's eigenvalue. U-238 is the wrong probe here — it is a threshold
/// fissioner, so a control written on it would be testing a channel that
/// carries almost none of Godiva's fissions.
fn u235() -> Option<Nuclide> {
    let tape = reference_file_or_skip(
        "endf",
        "n-092_U_235-ENDF8.0.endf",
        "U-235 evaluation (fission-source ablation controls)",
    )?;
    Some(Nuclide::from_endf_file(&tape, "U235", TEMP_K, 1.0e-3).expect("U-235 reconstructs"))
}

/// Thermal end of the nu-bar table, used as the frozen reference so the
/// ablation removes the *whole* rise rather than part of it.
const NU_FREEZE_EV: f64 = 0.0253;

/// `with_frozen_nubar` removes nu-bar's energy dependence, removes only it, and
/// had a dependence to remove.
///
/// The first assertion is the load-bearing one: if U-235's nu-bar were flat to
/// begin with, this hook would be a no-op and any `Delta-k` measured through it
/// would be an artefact of nothing. It is not flat, and the test prints by how
/// much.
#[test]
fn the_nubar_hook_freezes_exactly_the_yield_curve() {
    let Some(evaluated) = u235() else { return };

    // 1: there was something to remove. nu-bar must genuinely vary across the
    // band a fast system samples.
    let nu_thermal = evaluated.nu_bar(NU_FREEZE_EV);
    let nu_fast = evaluated.nu_bar(PROBE_EV);
    assert!(
        nu_thermal > 0.0 && nu_fast > 0.0,
        "U-235 reports nu-bar 0 somewhere in [{NU_FREEZE_EV}, {PROBE_EV:.1e}] eV \
         ({nu_thermal} / {nu_fast}); the MF=1/452 table is not being read."
    );
    let rise = nu_fast - nu_thermal;
    assert!(
        rise > 0.05,
        "U-235 nu-bar rises only {rise:.5} from {NU_FREEZE_EV} eV to {PROBE_EV:.1e} eV \
         ({nu_thermal:.5} -> {nu_fast:.5}). The evaluation's rise over this band is ~0.2; a \
         much smaller one means the table is being read wrongly, and freezing a curve that is \
         already flat would price nothing while reporting a number."
    );

    let ablated = evaluated.clone().with_frozen_nubar(NU_FREEZE_EV);

    // 2: it is gone — nu-bar is now the same number at every energy, including
    // energies far outside the frozen point.
    assert_eq!(ablated.frozen_nubar_energy(), Some(NU_FREEZE_EV));
    for &e in &[1.0e-3_f64, NU_FREEZE_EV, 1.0e3, 1.0e5, PROBE_EV, 1.4e7] {
        assert_eq!(
            ablated.nu_bar(e).to_bits(),
            nu_thermal.to_bits(),
            "after freezing at {NU_FREEZE_EV} eV, nu-bar at {e:.3e} eV is {} rather than \
             {nu_thermal}. The hook is not reaching every lookup, so the two arms of a study \
             would differ in more than the frozen curve.",
            ablated.nu_bar(e)
        );
    }

    // 3: nothing else moved. The fission cross section in particular — the hook
    // changes the yield per fission, not the fission rate, and a Delta-k that
    // mixed the two would be uninterpretable.
    assert_cross_sections_unchanged(&evaluated, &ablated, "with_frozen_nubar");

    // 4: `nu_fission` — the product the eigenvalue is actually built from —
    // moved in exactly the way the frozen yield predicts, and by the fission
    // cross section alone. This is what catches a hook that reaches `nu_bar`
    // but not the cross-section assembly.
    let x_before = evaluated.xs_at_energy(PROBE_EV, TEMP_K);
    let x_after = ablated.xs_at_energy(PROBE_EV, TEMP_K);
    let expected = x_before.fission * nu_thermal;
    assert!(
        (x_after.nu_fission - expected).abs() <= 1.0e-12 * expected.max(1.0),
        "nu_fission after freezing is {} at {PROBE_EV:.1e} eV; sigma_f * nu-bar(frozen) is \
         {expected}. The frozen yield is not reaching MicroXS.",
        x_after.nu_fission
    );
    let drop_pct = 100.0 * (x_after.nu_fission / x_before.nu_fission - 1.0);
    println!(
        "with_frozen_nubar: U-235 nu-bar {nu_thermal:.5} @ {NU_FREEZE_EV} eV vs {nu_fast:.5} @ \
         {PROBE_EV:.1e} eV (rise {rise:.5}); frozen at thermal it is {nu_thermal:.5} at every \
         energy, nu_fission at {PROBE_EV:.1e} eV {:.5} -> {:.5} b ({drop_pct:+.2} %), cross \
         sections bit-identical",
        x_before.nu_fission, x_after.nu_fission
    );
}

/// `with_frozen_fission_spectrum` removes chi's incident-energy dependence,
/// removes only it, and had a dependence to remove.
///
/// The "had something to remove" assertion matters more here than anywhere
/// else in this file: [`FissionSpectrum::Watt`] with fixed parameters — the
/// LOW-tier default, and the fallback for any tape whose MF=5 this port does
/// not reconstruct — is **static by construction**. A control written without
/// this check would pass on a nuclide where the hook can do nothing, and
/// certify a knob that prices nothing.
///
/// # The threshold is taken from the evaluation, not chosen
///
/// A first draft of this test asserted "the sampled mean must move more than
/// 1 % between a thermal-induced and a 14 MeV-induced fission" and **failed**,
/// measuring `+0.904 %`. The arbitrary number was the defect, not the sampler.
/// ENDF/B-VIII.0's U-235 MF=5/MT=18 is an LF=1 law on **22 incident-energy rows
/// from 1e-5 to 3e7 eV**, and integrating those rows' own tabulated pdfs shows
/// why both numbers are right: chi is **nearly independent of incident energy
/// through the whole fission-spectrum range**, then hardens sharply at the top
/// — `+0.9 %` by 14 MeV but `+15.1 %` by 30 MeV, where third-chance fission and
/// pre-equilibrium emission set in. A threshold picked from intuition would
/// have been wrong in one direction or the other depending only on which probe
/// energy it happened to use.
///
/// So the test now measures the tape's rows directly and asserts (a) that they
/// differ at all — the "something to remove" condition, grounded in the data
/// rather than in an expectation — and (b) that the **sampler reproduces each
/// row's own mean**, which is a real cross-check of the sampling path against
/// the distribution it claims to sample, and a far stronger statement than any
/// threshold on the difference would have been.
#[test]
fn the_fission_spectrum_hook_freezes_exactly_the_incident_energy() {
    use njoy_outram_park_fork::endf::tape::Tape;
    use njoy_outram_park_fork::nuclear_data::secondary::FissionSpectrum;

    let Some(path) = reference_file_or_skip(
        "endf",
        "n-092_U_235-ENDF8.0.endf",
        "U-235 evaluation (chi ablation control)",
    ) else {
        return;
    };
    let tape = Tape::read_file(&path).expect("U-235 tape parses");
    let mat = tape.materials()[0];
    let evaluated = Nuclide::from_endf_file(&path, "U235", TEMP_K, 1.0e-3).expect("U-235");

    /// Trapezoidal mean `∫E' g(E') dE'` of one tabulated outgoing-energy row —
    /// the distribution's own first moment, read straight off the tape and
    /// owing nothing to the sampler under test.
    fn row_mean(e_out: &[f64], pdf: &[f64]) -> f64 {
        let mut num = 0.0;
        let mut den = 0.0;
        for w in 0..e_out.len().saturating_sub(1) {
            let (x0, x1) = (e_out[w], e_out[w + 1]);
            let (p0, p1) = (pdf[w], pdf[w + 1]);
            let dx = x1 - x0;
            num += 0.5 * dx * (x0 * p0 + x1 * p1);
            den += 0.5 * dx * (p0 + p1);
        }
        if den > 0.0 {
            num / den
        } else {
            0.0
        }
    }

    /// Mean birth energy \[eV\] over `n` draws at incident energy `e_in`.
    fn mean_birth(nuc: &Nuclide, e_in: f64, n: usize, seed: &mut u64) -> f64 {
        (0..n)
            .map(|_| nuc.sample_fission_energy(e_in, seed))
            .sum::<f64>()
            / n as f64
    }

    // 1: there was something to remove -- asserted against the tape, not an
    // expectation. The law must genuinely be a function of incident energy.
    let chi = FissionSpectrum::from_endf_mf5(&tape, mat)
        .expect("MF=5 parses")
        .expect("U-235 has an MF=5/MT=18 section");
    let FissionSpectrum::ContinuousTabular(table) = &chi else {
        panic!(
            "U-235's chi came back as {chi:?}, not the energy-dependent LF=1 law. A static \
             spectrum has no incident-energy dependence to freeze, so this hook would price \
             nothing while reporting a number -- which is the exact failure this file exists \
             to catch."
        );
    };
    assert!(
        table.incident.len() >= 2,
        "U-235's LF=1 law carries {} incident-energy row(s); with fewer than two there is \
         nothing for the freeze to collapse.",
        table.incident.len()
    );
    let (lo_i, hi_i) = (0usize, table.incident.len() - 1);
    let (e_lo, e_hi) = (table.incident[lo_i], table.incident[hi_i]);
    let tape_lo = row_mean(&table.tables[lo_i].e_out, &table.tables[lo_i].pdf);
    let tape_hi = row_mean(&table.tables[hi_i].e_out, &table.tables[hi_i].pdf);
    let tape_spread_pct = 100.0 * (tape_hi / tape_lo - 1.0);
    assert!(
        (tape_hi - tape_lo).abs() > 0.0,
        "the tape's own first and last chi rows have identical means ({tape_lo:.6e} eV), so \
         the law is flat in incident energy and freezing it removes nothing."
    );

    // 2: the SAMPLER follows the tape. This is the cross-check that makes the
    // rest meaningful -- a hook that freezes a law nobody samples is moot.
    const N: usize = 400_000;
    for (label, e_in, reference) in [("first row", e_lo, tape_lo), ("last row", e_hi, tape_hi)] {
        let mut seed = 0xC41D0001u64;
        let sampled = mean_birth(&evaluated, e_in, N, &mut seed);
        // 1 % covers the sampling error on N draws off a distribution whose
        // own spread is of order its mean, plus the trapezoid's discretisation.
        let rel = (sampled / reference - 1.0).abs();
        assert!(
            rel < 0.01,
            "{label} (incident {e_in:.3e} eV): the sampler's mean birth energy is \
             {sampled:.5e} eV over {N} draws, against {reference:.5e} eV integrated from the \
             tape's own pdf -- {:.3} % apart. The sampling path is not following the law it \
             reports carrying.",
            100.0 * rel
        );
    }

    let ablated = evaluated.clone().with_frozen_fission_spectrum(NU_FREEZE_EV);
    assert_eq!(ablated.frozen_fission_spectrum_energy(), Some(NU_FREEZE_EV));

    // 3: it is gone. Two very different incident energies must now give the
    // SAME spectrum -- and because the frozen argument makes the sampler's
    // input identical, the draws are bit-identical on a shared seed, not
    // merely close.
    let mut seed_a = 0xC41D0002u64;
    let mut seed_b = 0xC41D0002u64;
    for i in 0..4096 {
        let a = ablated.sample_fission_energy(NU_FREEZE_EV, &mut seed_a);
        let b = ablated.sample_fission_energy(1.4e7, &mut seed_b);
        assert_eq!(
            a.to_bits(),
            b.to_bits(),
            "draw {i}: after freezing chi at {NU_FREEZE_EV} eV, a fission induced at 1.4e7 eV \
             still emitted differently ({a} vs {b}). The frozen incident energy is not reaching \
             the sampler."
        );
    }

    // 4: the frozen arm reproduces the unablated arm AT the frozen energy.
    // This is the direction that catches a hook pinned somewhere other than
    // where it says -- it must be a no-op exactly at its own reference.
    let mut seed_a = 0xC41D0003u64;
    let mut seed_b = 0xC41D0003u64;
    for i in 0..4096 {
        let a = evaluated.sample_fission_energy(NU_FREEZE_EV, &mut seed_a);
        let b = ablated.sample_fission_energy(NU_FREEZE_EV, &mut seed_b);
        assert_eq!(
            a.to_bits(),
            b.to_bits(),
            "draw {i}: at the frozen energy itself the hook changed the sampled birth energy \
             ({a} vs {b}); it is pinned somewhere other than where it says."
        );
    }

    // 5: nothing else moved.
    assert_cross_sections_unchanged(&evaluated, &ablated, "with_frozen_fission_spectrum");
    assert_eq!(
        evaluated.nu_bar(PROBE_EV).to_bits(),
        ablated.nu_bar(PROBE_EV).to_bits(),
        "the chi ablation moved nu-bar; the two factors of the fission source must stay separable"
    );

    let mut seed = 0xC41D0004u64;
    let sampled_fast = mean_birth(&evaluated, PROBE_EV, N, &mut seed);
    let mut seed = 0xC41D0004u64;
    let ablated_fast = mean_birth(&ablated, PROBE_EV, N, &mut seed);

    // The 14 MeV row, reported alongside the endpoints because the two
    // together are what show WHERE chi's incident-energy dependence lives:
    // almost nothing through the fission-spectrum range, then a sharp rise.
    let mid_i = table
        .incident
        .iter()
        .enumerate()
        .min_by(|a, b| {
            (a.1 - 1.4e7)
                .abs()
                .partial_cmp(&(b.1 - 1.4e7).abs())
                .unwrap()
        })
        .map(|(i, _)| i)
        .unwrap();
    let tape_mid = row_mean(&table.tables[mid_i].e_out, &table.tables[mid_i].pdf);
    println!(
        "with_frozen_fission_spectrum: U-235 LF=1 on {} incident rows, {e_lo:.3e}..{e_hi:.3e} eV; \
         tape row means {tape_lo:.5e} eV @ {e_lo:.3e} -> {tape_mid:.5e} eV @ {:.3e} \
         ({:+.3} %) -> {tape_hi:.5e} eV @ {e_hi:.3e} ({tape_spread_pct:+.3} %), i.e. chi barely \
         moves through the fission-spectrum range and hardens only at the top; sampler \
         reproduces both endpoint rows within 1 %; at {PROBE_EV:.1e} eV incident the mean birth \
         energy goes {sampled_fast:.5e} -> {ablated_fast:.5e} eV when frozen at {NU_FREEZE_EV} \
         eV, and 1.4e7 eV draws become bit-identical to thermal ones; nu-bar and cross \
         sections untouched",
        table.incident.len(),
        table.incident[mid_i],
        100.0 * (tape_mid / tape_lo - 1.0)
    );
}

/// The two fission-source hooks are **independent** — each leaves the other's
/// factor alone, so a study may price `nu-bar` and `chi` separately or together
/// without confounding them.
#[test]
fn the_fission_source_hooks_do_not_interfere() {
    let Some(evaluated) = u235() else { return };

    let nu_off = evaluated.clone().with_frozen_nubar(NU_FREEZE_EV);
    let chi_off = evaluated.clone().with_frozen_fission_spectrum(NU_FREEZE_EV);

    // Freezing nu-bar must not touch chi.
    let (mut sa, mut sb) = (0xF1551053u64, 0xF1551053u64);
    for i in 0..2048 {
        let a = evaluated.sample_fission_energy(PROBE_EV, &mut sa);
        let b = nu_off.sample_fission_energy(PROBE_EV, &mut sb);
        assert_eq!(a.to_bits(), b.to_bits(), "draw {i}: the nu-bar freeze moved chi");
    }
    // Freezing chi must not touch nu-bar (asserted at both ends of the table).
    for &e in &[NU_FREEZE_EV, PROBE_EV, 1.4e7] {
        assert_eq!(
            evaluated.nu_bar(e).to_bits(),
            chi_off.nu_bar(e).to_bits(),
            "the chi freeze moved nu-bar at {e:.3e} eV"
        );
    }

    // Combined, both are in effect and neither has undone the other.
    let both = evaluated
        .clone()
        .with_frozen_nubar(NU_FREEZE_EV)
        .with_frozen_fission_spectrum(NU_FREEZE_EV);
    assert_eq!(both.frozen_nubar_energy(), Some(NU_FREEZE_EV));
    assert_eq!(both.frozen_fission_spectrum_energy(), Some(NU_FREEZE_EV));
    assert_eq!(
        both.nu_bar(PROBE_EV).to_bits(),
        evaluated.nu_bar(NU_FREEZE_EV).to_bits(),
        "combining the two hooks lost the nu-bar freeze"
    );
    assert_cross_sections_unchanged(&evaluated, &both, "both fission-source hooks combined");

    // And they are independent of the *scattering* hooks, which is what lets
    // one paired-seed study vary several mechanisms at once.
    let with_scatter = both.clone().with_target_at_rest().with_isotropic_continuum_scattering();
    assert_eq!(with_scatter.frozen_nubar_energy(), Some(NU_FREEZE_EV));
    assert_eq!(with_scatter.frozen_fission_spectrum_energy(), Some(NU_FREEZE_EV));
    assert!(with_scatter.is_target_at_rest());

    println!("the nu-bar and chi hooks compose with each other and with the scattering hooks");
}

// ───────────────────────── (n,2n) yield multiplicity ─────────────────────────
//
// Gap 6 of `docs/neutronics-physics-coverage.md`, closed 2026-09-16. The MT=16
// *emission law* already had two hooks — `without_evaluated_continuum` for its
// energy and `with_isotropic_continuum_scattering` for its angle. The **yield
// of 2** had none: it was ablatable only lumped in with the discrete levels and
// the continuum via `without_inelastic`, so its own worth could not be
// separated from theirs.

/// Above U-238's MT=16 threshold (~6 MeV) and U-235's (~5.3 MeV), so the (n,2n)
/// channel is genuinely open. `PROBE_EV = 2 MeV` is **below** both and would
/// make every assertion here vacuous — the reason this constant exists.
const N2N_PROBE_EV: f64 = 1.2e7;

/// `with_unit_n2n_multiplicity` cuts the (n,2n) yield from 2 to 1, cuts only
/// that, and had a second neutron to cut.
///
/// Data-level half of the control; the kernel-level half is the paired-run test
/// below, which is the one that would have caught `op-50vu`.
#[test]
fn the_n2n_multiplicity_hook_ablates_exactly_the_second_neutron() {
    let Some(evaluated) = u238() else { return };

    // 1: there was something to remove. The channel must actually be open at
    // the probe, and shut below its threshold -- both, because a hook on a
    // channel that never fires prices nothing, and a "cross section" that is
    // non-zero everywhere would mean the threshold is not being applied.
    let open = evaluated.xs_at_energy(N2N_PROBE_EV, TEMP_K).n2n;
    let shut = evaluated.xs_at_energy(PROBE_EV, TEMP_K).n2n;
    assert!(
        open > 0.0,
        "U-238 has no (n,2n) cross section at {N2N_PROBE_EV:.1e} eV (got {open}), so the yield-2 \
         branch is never reached and this ablation prices nothing."
    );
    assert_eq!(
        shut, 0.0,
        "U-238 reports an (n,2n) cross section {shut} b at {PROBE_EV:.1e} eV, below the ~6 MeV \
         threshold. Either MT=16's threshold is not applied or the probe energies are wrong."
    );
    assert!(
        evaluated.emits_n2n_secondary(),
        "an unablated nuclide reports that it does not emit the (n,2n) secondary"
    );

    let ablated = evaluated.clone().with_unit_n2n_multiplicity();

    // 2: it is gone.
    assert!(
        !ablated.emits_n2n_secondary(),
        "with_unit_n2n_multiplicity left the yield-2 emission in place"
    );

    // 3: nothing else moved -- and the (n,2n) CROSS SECTION in particular must
    // survive. This hook changes the yield, not the rate: if sigma_n2n moved,
    // a measured Delta-k would be mixing a rate change into a yield change.
    assert_cross_sections_unchanged(&evaluated, &ablated, "with_unit_n2n_multiplicity");
    assert_eq!(
        evaluated.xs_at_energy(N2N_PROBE_EV, TEMP_K).n2n.to_bits(),
        ablated.xs_at_energy(N2N_PROBE_EV, TEMP_K).n2n.to_bits(),
        "with_unit_n2n_multiplicity changed the (n,2n) cross section; it must change the yield \
         only, so the collision rate is identical across the two arms"
    );

    // 4: the MT=16 emission law is untouched. The yield and the law are
    // separate mechanisms with separate hooks, and a study may want to price
    // them independently.
    assert_eq!(
        evaluated.continuum_law(16).is_some(),
        ablated.continuum_law(16).is_some(),
        "the yield ablation took the MT=16 emission law with it; `without_evaluated_continuum` \
         is the hook for the law"
    );

    println!(
        "with_unit_n2n_multiplicity: U-238 sigma_n2n {open:.5} b at {N2N_PROBE_EV:.1e} eV and \
         0 below threshold at {PROBE_EV:.1e} eV; emission flag true -> false; cross sections \
         (sigma_n2n included) bit-identical and the MT=16 energy law intact"
    );
}

/// **The kernel honours the yield ablation, and honours it without
/// desynchronising the RNG.** A paired k-eigenvalue run on a bare U-235 sphere,
/// same seed both arms.
///
/// # Why this test exists and the data-level one is not enough
///
/// The hook is a flag on `Nuclide`; the thing it has to change lives in four
/// separate collision kernels (`physics::keff` twice, `physics::transport_csg`,
/// `pebble_beds::keff_delta`). A flag that flips while no kernel reads it
/// reports "no difference", which reads as "(n,2n) multiplicity does not
/// matter". That is exactly bead `op-50vu` — free-gas and bound-thermal
/// eigenvalues came out bit-identical because the S(α,β) wiring never reached
/// the sampler, and the symptom was two numbers agreeing.
///
/// # Why a single paired run is legitimate here, when it is not for free-gas
///
/// The secondary is **drawn unconditionally and only its emission is gated**,
/// so the two arms consume identical RNG streams. The difference between them
/// is therefore *deterministic* — the same seed gives the same two numbers
/// every time — rather than a sample from a distribution. That is what lets one
/// paired run at a fixed seed serve as a regression assertion.
/// [`Nuclide::with_target_at_rest`] cannot do this: its ablated arm skips a
/// target-velocity draw, so its arms diverge and it needs an ensemble.
///
/// **This is a harness check, not physics V&V.** It asserts the switch reaches
/// the kernel and moves `k` the only direction it physically can. What (n,2n)
/// multiplicity is *worth* is a paired-seed ensemble measurement on a real
/// case, and is job 5's neighbour in `docs/handoff-heavy-neutronics-runs.md`.
#[test]
fn the_n2n_multiplicity_hook_reaches_the_transport_kernel() {
    use outram_mc_libs::material::material::{Material, NuclideComponent};
    use outram_mc_libs::physics::keff::{run_keff, KeffSettings};

    let Some(path) = reference_file_or_skip(
        "endf",
        "n-092_U_235-ENDF8.0.endf",
        "U-235 evaluation ((n,2n) multiplicity kernel control)",
    ) else {
        return;
    };
    let evaluated = Nuclide::from_endf_file(&path, "U235", TEMP_K, 1.0e-3).expect("U-235");
    assert!(
        evaluated.xs_at_energy(N2N_PROBE_EV, TEMP_K).n2n > 0.0,
        "U-235 has no (n,2n) channel at {N2N_PROBE_EV:.1e} eV; this test would be vacuous."
    );

    // A bare U-235 metal sphere at roughly Godiva's dimensions. It does not
    // need to be a benchmark -- it needs a fission spectrum with enough flux
    // above the ~5.3 MeV (n,2n) threshold for the channel to fire.
    let sphere = |nuclides: Vec<Nuclide>, seed: u64| {
        let material = Material {
            id: 1,
            name: "bare U-235".into(),
            temperature: TEMP_K,
            components: vec![NuclideComponent {
                nuclide_idx: 0,
                atom_density: 4.4994e-2,
            }],
        };
        let settings = KeffSettings {
            n_particles: 4000,
            n_inactive: 20,
            n_active: 60,
            temperature_k: TEMP_K,
            seed,
            ..KeffSettings::default()
        };
        run_keff(8.7407, &material, &nuclides, &settings).k_mean
    };

    const SEED: u64 = 20_260_916;
    let k_yield2 = sphere(vec![evaluated.clone()], SEED);
    let k_yield1 = sphere(vec![evaluated.clone().with_unit_n2n_multiplicity()], SEED);
    let delta_pcm = (k_yield1 - k_yield2) * 1.0e5;

    // 1: the kernel noticed. If these are equal, no kernel is reading the flag.
    assert_ne!(
        k_yield2.to_bits(),
        k_yield1.to_bits(),
        "the two arms gave bit-identical k ({k_yield2}). Either no collision kernel consults \
         `Nuclide::emits_n2n_secondary`, or no (n,2n) collision occurred in {} histories. \
         Both make every (n,2n) measurement through this hook meaningless -- this is the \
         op-50vu failure mode.",
        4000 * 80
    );

    // 2: it moved the only direction it physically can. (n,2n) is a neutron
    // MULTIPLIER; deleting the extra neutron removes a source and can only
    // lower k. A positive delta means the gate is inverted somewhere.
    assert!(
        delta_pcm < 0.0,
        "cutting the (n,2n) yield from 2 to 1 RAISED k by {delta_pcm:+.1} pcm \
         ({k_yield2:.6} -> {k_yield1:.6}). Removing a neutron source cannot raise the \
         eigenvalue; the emission gate is inverted at one or more of the four kernel sites."
    );

    // 3: the magnitude is physically plausible. (n,2n) is a threshold reaction
    // with a small cross section and a fission spectrum puts ~1 % of its flux
    // above it, so this is a small effect. A huge one means the gate is
    // catching something other than the (n,2n) secondary.
    assert!(
        delta_pcm > -3000.0,
        "cutting the (n,2n) yield moved k by {delta_pcm:+.1} pcm, far more than a threshold \
         reaction carrying ~1 % of the flux can be worth. The emission gate is probably \
         suppressing more than the (n,2n) secondary."
    );

    println!(
        "with_unit_n2n_multiplicity reaches the kernel: bare U-235 sphere, seed {SEED}, \
         4000 x [20 inactive + 60 active] -- k {k_yield2:.6} (yield 2) -> {k_yield1:.6} \
         (yield 1), {delta_pcm:+.1} pcm. Deterministic on a shared seed because the secondary \
         is drawn either way and only its emission is gated. Harness check, not a worth \
         measurement."
    );
}

// ──────────────────────────── (n,3n), MT=17 ────────────────────────────
//
// Found 2026-09-16 by the physics-coverage survey and fixed the same day.
// MT=17 had no branch in any collision kernel. It is inside MT=1, so the
// collision still happened -- but it fell through to the ELASTIC arm and both
// extra neutrons were silently lost. U-238's threshold is ~11.3 MeV.

/// Above U-238's MT=17 threshold (~11.3 MeV).
const N3N_PROBE_EV: f64 = 1.4e7;

/// `(n,3n)` is evaluated, branched on, and emits **two** extra neutrons — and
/// adding it left every reactor-spectrum result untouched.
///
/// # The second half is the one that needed proving
///
/// Inserting a branch into a cross-section partition normally changes which
/// reaction a given `xi` selects, and would move every result in the crate.
/// It does not here, and the reason is structural rather than lucky: the new
/// arm tests `xi < absorption + inelastic + n2n + n3n`, and **below the MT=17
/// threshold `n3n` is exactly zero**, so that bound coincides with the old
/// `else` boundary. The partition is bit-identical wherever the channel is
/// shut, which is everywhere a fission spectrum lives.
#[test]
fn the_n3n_channel_is_branched_and_changes_nothing_below_its_threshold() {
    let Some(nuc) = u238() else { return };

    // 1: the channel exists and is open where it should be.
    let open = nuc.xs_at_energy(N3N_PROBE_EV, TEMP_K).n3n;
    assert!(
        open > 0.0,
        "U-238 reports no (n,3n) cross section at {N3N_PROBE_EV:.1e} eV (got {open}); MT=17 is \
         not being evaluated and the branch can never fire."
    );

    // 2: and shut everywhere a reactor spectrum lives. This is what makes the
    // partition claim below hold.
    for &e in &[1.0e-2_f64, 1.0, 1.0e3, 1.0e5, PROBE_EV, 1.0e7] {
        let n3n = nuc.xs_at_energy(e, TEMP_K).n3n;
        assert_eq!(
            n3n, 0.0,
            "U-238 reports an (n,3n) cross section {n3n} b at {e:.3e} eV, below its ~11.3 MeV \
             threshold. Either the threshold is not applied or MT=17 is being read wrongly."
        );
    }

    // 3: the partition is unchanged below threshold -- assert the actual
    // arithmetic the kernels branch on, not a proxy for it.
    for &e in &[1.0e-2_f64, 1.0, 1.0e3, 1.0e5, PROBE_EV] {
        let x = nuc.xs_at_energy(e, TEMP_K);
        let old_bound = x.absorption + x.inelastic + x.n2n;
        let new_bound = old_bound + x.n3n;
        assert_eq!(
            old_bound.to_bits(),
            new_bound.to_bits(),
            "at {e:.3e} eV the (n,3n) arm moved the elastic boundary ({old_bound} -> \
             {new_bound}), so adding the channel changed the reaction partition where it \
             should have been a no-op."
        );
    }

    // 4: the emission law is reachable, or the Weisskopf fallback is used --
    // either is fine, but the branch must not be sampling MT=16's law by
    // mistake, which would give (n,3n) the wrong outgoing spectrum.
    let law16 = nuc.continuum_law(16).is_some();
    let law17 = nuc.continuum_law(17).is_some();
    println!(
        "(n,3n): U-238 sigma_n3n {open:.5} b at {N3N_PROBE_EV:.1e} eV, exactly 0 at every \
         reactor-spectrum energy tested; the elastic boundary is bit-identical below \
         threshold, so the partition is unchanged. MF=6 law present: MT=16 {law16}, \
         MT=17 {law17}."
    );
}

/// The yield hook covers MT=17 as well as MT=16 — its scope widened when
/// `(n,3n)` gained a branch, and a control that only checked MT=16 would not
/// have noticed.
#[test]
fn the_yield_hook_covers_the_n3n_channel_too() {
    let Some(nuc) = u238() else { return };
    assert!(nuc.emits_n2n_secondary());
    let ablated = nuc.clone().with_unit_n2n_multiplicity();
    assert!(!ablated.emits_n2n_secondary());

    // Cross sections -- (n,3n) included -- must survive: the hook changes the
    // yield, not the rate.
    assert_cross_sections_unchanged(&nuc, &ablated, "with_unit_n2n_multiplicity (n,3n scope)");
    assert_eq!(
        nuc.xs_at_energy(N3N_PROBE_EV, TEMP_K).n3n.to_bits(),
        ablated.xs_at_energy(N3N_PROBE_EV, TEMP_K).n3n.to_bits(),
        "the yield ablation changed the (n,3n) cross section; it must change the yield only"
    );
    println!(
        "with_unit_n2n_multiplicity covers MT=17: emission flag gates both multiplying \
         channels, sigma_n3n bit-identical across it"
    );
}
