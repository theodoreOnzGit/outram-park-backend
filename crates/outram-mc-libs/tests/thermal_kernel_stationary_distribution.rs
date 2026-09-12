//! **Does the S(α,β) scattering kernel have the right fixed point?**
//!
//! Every oracle this crate has aimed at a thermal scattering law so far has been
//! a *per-collision* one: σ(E) against THERMR's MT=222/230, ⟨E′⟩/E and ξ against
//! THERMR's MF=6 matrix, μ̄ and the kernel width against the same. Those are
//! necessary and they are all in `tests/thermal_laws_vs_njoy_thermr.rs`.
//!
//! They are not sufficient, and bead `op-02jx` (GitHub #179) says why. A sampler
//! that is applied *repeatedly* has a **fixed point**, and the fixed point is a
//! property of the whole kernel, not of any one of its moments. A kernel can
//! reproduce ⟨E′⟩/E at every incident energy and still relax a neutron
//! population onto the wrong Maxwellian, because ⟨E′⟩/E constrains only the
//! forward transfer and equilibrium is set by the *ratio* of the forward and
//! reverse transfers. `op-50vu` is the worked example of exactly that failure in
//! this crate: per-collision kinematics were exact (ξ/ξ₀ = 1.000, no α·E floor
//! violations) while the *sequence* had no fixed point at all, because a target
//! held at rest can only ever take energy away.
//!
//! This file supplies the missing oracle for the **bound** kernel. The free-gas
//! one already has it (`physics::scatter::free_gas_equilibrium_density_is_maxwellian`).
//!
//! # The fixed point, stated before the test is written
//!
//! In an infinite, purely-scattering medium at temperature `T`, the equilibrium
//! neutron **density** is Maxwell-Boltzmann,
//!
//! ```text
//!   n(E) ∝ √E · exp(−E / kT),     ⟨E⟩ = 1.5 kT,  ⟨E²⟩ = 3.75 (kT)²
//! ```
//!
//! and a scattering kernel reaches it if and only if it satisfies detailed
//! balance, `σ(E→E′)·E·e^(−E/kT) = σ(E′→E)·E′·e^(−E′/kT)`.
//!
//! **The random walk does not sample `n(E)` directly.** Each step of the walk is
//! a *collision*, so the chain's stationary density is the collision density
//! `F(E) = Σ_s(E)·φ(E) ∝ σ_thermal(E)·√E·n(E)`, where `σ_thermal(E)` is the
//! law's own effective cross section (which already carries the binding, exactly
//! as the free-gas effective σ carries `v̄_rel`). Recovering `n(E)` therefore
//! needs the weight
//!
//! ```text
//!   w(E) = 1 / (σ_thermal(E) · √E)
//! ```
//!
//! Omitting that weight is the mistake `op-02jx` records two earlier oracles
//! making — they were wrong by 3.9–5.3 % purely from the missing collision-rate
//! weighting, which is the same size as the effect being hunted.
//!
//! # Results measured here — BOTH BOUND LAWS FAIL THE FIXED-POINT TEST
//!
//! 2026-09-12, 4000 independent chains, 400 burn-in + 6400 tallied collisions
//! each (4x the committed test's statistics, run once to show the numbers below
//! are converged and not estimator bias). Errors are the standard error over the
//! 4000 chains:
//!
//! ```text
//!   law                     T_eff [K]          vs nominal    <E^2>/<E>^2   vs 5/3
//!   free gas C-12, 600 K    600.44 +/- 0.31      +0.07 %     1.6660        -0.04 %
//!   c_H_in_H2O,   293.6 K   294.31 +/- 0.12      +0.24 %     1.6315        -2.11 %
//!   c_Graphite,   600 K     607.42 +/- 0.45      +1.24 %     1.6282        -2.31 %
//! ```
//!
//! The free-gas row is the **control**: the identical estimator, identical chain
//! lengths, identical weighting, driven through a kernel that is known to satisfy
//! detailed balance. It lands on its own temperature to +0.07 % (1.4 sigma) and
//! on the Maxwellian shape to -0.04 %. So the method resolves a tenth of a
//! percent, and the two bound rows are the laws', not the estimator's.
//!
//! Two distinct defects are visible:
//!
//! 1. **`c_Graphite` equilibrates 1.24 % hot** (16 sigma). A neutron population
//!    in infinite graphite at 600 K settles onto a Maxwellian at 607.4 K. This
//!    is the FHR pebble's moderator, and it is a pure "where the thermal
//!    population sits" error — exactly the kind that moves the fraction of
//!    neutrons crossing 0.625 eV without moving any reaction-rate ratio.
//! 2. **Both bound laws are ~2 % narrow in the second moment** while free gas is
//!    0.04 %. The fixed point is not just displaced, it is the wrong *shape* —
//!    the equiprobable outgoing-energy tables cannot reproduce the Maxwellian's
//!    tails, and the deficit is the same size (-2.1 %, -2.3 %) for two laws with
//!    completely different physics, which points at the tabulation rather than
//!    at either evaluation.
//!
//! # The emission-table resize halved the graphite displacement, and this is how
//! # much of it was left
//!
//! The rows above are measured **after** the 2026-09-12 emission-table resize
//! (`5916b917`, `f540b6ac`, GitHub #190), which took the incident-energy grid
//! from 48 points to 384 and the equiprobable outgoing energies from 16 to 64.
//! Running the same walk on the pre-resize constants gives, at the committed
//! 1600 tallied collisions:
//!
//! ```text
//!   c_Graphite, 600 K     emission table   T_eff [K]          <E^2>/<E>^2
//!   before (#190 open)    48 x 16          614.28 +/- 0.83    1.5652  (-6.09 %)
//!   after  (#190 fixed)   384 x 64         607.21 +/- 0.89    1.6286  (-2.31 %)
//! ```
//!
//! So the resize is worth **-7.1 K of fixed-point displacement and two thirds of
//! the shape deficit** -- a real improvement that the per-collision moment tests
//! which motivated it could only see side-on. It is also the strongest evidence
//! that the residual displacement is the *same class* of defect and not a
//! separate one: the pre-tabulation is still too coarse to carry detailed
//! balance, just less so.
//!
//! These are recorded, not fixed, and are gated against drift below. The
//! tracking beads are named in the tests.
//!
//! # Why it is worth running against the +4004 pcm FHR residual
//!
//! The ring-RPT record localises the whole residual to "the fraction of neutrons
//! that cross 0.625 eV" — production per absorption agrees to 0.07 % inside the
//! thermal group and 0.02 % inside the fast group, so no reaction-rate *ratio*
//! can be the cause. The quantity that sets that fraction is the moderator
//! kernel, and the one property of a moderator kernel that no test in this crate
//! constrained until now is its fixed point.
//!
//! A kernel whose fixed point is a Maxwellian at `T_eff ≠ T` puts the entire
//! thermal population at the wrong energy, which is precisely a "fraction
//! crossing 0.625 eV" error and precisely *not* a reaction-rate-ratio error.
//!
//! Data-gated: skips (passes) when `reference-data/endf/` is absent, same
//! contract as `tests/thermal_laws_vs_njoy_thermr.rs`.

use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::physics::scatter::{free_gas_elastic_scatter, K_BOLTZMANN_EV_PER_K};
use outram_mc_libs::rng::lcg::prn;

/// Independent chains. Each one is its own statistical sample, so the spread
/// across chains is an honest standard error despite the within-chain
/// correlation that makes a naive per-collision σ meaningless.
const N_CHAINS: usize = 4_000;
/// Collisions discarded before tallying, per chain.
const N_BURN: usize = 400;
/// Collisions tallied per chain.
const N_TALLY: usize = 1_600;

/// `Some(law)` when the tape is present, else `None` after printing a skip note.
fn law_or_skip(file: &str, mat: i32, temp_k: f64, name: &str) -> Option<ThermalScattering> {
    let Some(path) = njoy_outram_park_fork::reference_data::reference_endf(file) else {
        println!("[{name}] SKIP: {file} not in reference-data/endf/");
        return None;
    };
    match ThermalScattering::from_endf_file(path.to_str().expect("path"), mat, temp_k, name) {
        Ok(l) => Some(l),
        Err(e) => {
            println!("[{name}] SKIP: {e:?}");
            None
        }
    }
}

/// One chain's raw density-weighted sums. Deliberately **not** reduced to a
/// ratio here: a ratio-of-sums formed per chain and then averaged over chains is
/// biased at `O(1/n_eff)`, and with the within-chain autocorrelation of a
/// thermalising random walk `n_eff` is small enough for that bias to reach the
/// few-tenths-of-a-percent scale this test is trying to resolve. The central
/// value is formed by pooling the sums across all chains; the per-chain ratios
/// are used only for the standard error, where the bias cancels out of the
/// spread.
#[derive(Clone, Copy)]
struct ChainSums {
    /// Σ w.
    w: f64,
    /// Σ w·E \[eV\].
    we: f64,
    /// Σ w·E² \[eV²\].
    we2: f64,
}

/// Mean and standard error of a per-chain quantity.
fn mean_and_sem(xs: &[f64]) -> (f64, f64) {
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (n - 1.0);
    (mean, (var / n).sqrt())
}

/// Run one chain through a closure that performs one collision, and return the
/// density-weighted moments. `sigma` supplies the law's effective cross section
/// at the current energy, which is the collision-rate weight that has to be
/// divided out.
fn walk<S, C>(e_start: f64, seed: &mut u64, sigma: S, mut collide: C) -> ChainSums
where
    S: Fn(f64) -> f64,
    C: FnMut(f64, &mut u64) -> f64,
{
    let mut e = e_start;
    for _ in 0..N_BURN {
        e = collide(e, seed);
    }
    let (mut w_sum, mut w_e, mut w_e2) = (0.0, 0.0, 0.0);
    for _ in 0..N_TALLY {
        e = collide(e, seed);
        assert!(e > 0.0 && e.is_finite(), "energy left the domain: {e}");
        let s = sigma(e);
        assert!(s > 0.0 && s.is_finite(), "σ({e} eV) = {s}, cannot weight");
        let w = 1.0 / (s * e.sqrt());
        w_sum += w;
        w_e += w * e;
        w_e2 += w * e * e;
    }
    ChainSums {
        w: w_sum,
        we: w_e,
        we2: w_e2,
    }
}

/// Drive `N_CHAINS` chains from `e_start` and report `(T_eff, sem, shape, shape_sem)`
/// where `T_eff = ⟨E⟩ / (1.5 k)` \[K\] and `shape = ⟨E²⟩ / ⟨E⟩²` (exactly
/// `3.75 / 1.5² = 5/3` for a Maxwellian, independent of temperature).
fn equilibrium(
    e_start: f64,
    seed0: u64,
    sigma: &dyn Fn(f64) -> f64,
    collide: &dyn Fn(f64, &mut u64) -> f64,
) -> (f64, f64, f64, f64) {
    let mut t_eff = Vec::with_capacity(N_CHAINS);
    let mut shape = Vec::with_capacity(N_CHAINS);
    let (mut pw, mut pwe, mut pwe2) = (0.0, 0.0, 0.0);
    for c in 0..N_CHAINS {
        // Distinct, well-separated streams; the LCG is advanced by the walk.
        let mut seed = seed0.wrapping_add((c as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let m = walk(e_start, &mut seed, sigma, |e, s| collide(e, s));
        pw += m.w;
        pwe += m.we;
        pwe2 += m.we2;
        let m1 = m.we / m.w;
        t_eff.push(m1 / (1.5 * K_BOLTZMANN_EV_PER_K));
        shape.push(m.we2 / m.w / (m1 * m1));
    }
    // Central values from the pooled sums (unbiased); spreads from the per-chain
    // ratios (the O(1/n_eff) bias is common to every chain, so it drops out).
    let m1 = pwe / pw;
    let t = m1 / (1.5 * K_BOLTZMANN_EV_PER_K);
    let s = pwe2 / pw / (m1 * m1);
    let (_, t_sem) = mean_and_sem(&t_eff);
    let (_, s_sem) = mean_and_sem(&shape);
    (t, t_sem, s, s_sem)
}

/// A Maxwellian's shape factor `⟨E²⟩/⟨E⟩²` — `3.75 / 1.5² = 5/3`, exactly, at
/// every temperature. It is the one gate here that cannot be met by a kernel
/// that merely lands on the right *mean* energy with the wrong distribution.
const MAXWELLIAN_SHAPE: f64 = 5.0 / 3.0;

fn z_dir() -> Direction {
    Direction {
        u: 0.0,
        v: 0.0,
        w: 1.0,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Control: the method itself, driven through a kernel already known to be right
// ─────────────────────────────────────────────────────────────────────────────

/// **The estimator reproduces 600 K from the free-gas kernel it is calibrated
/// on — so a deviation on a bound law below is the law's, not the estimator's.**
///
/// # Methodology
///
/// Free-gas elastic scattering off C-12 (`awr = 11.89365`) at 600 K, isotropic
/// in CM, driven by [`free_gas_elastic_scatter`] — the same routine transport
/// uses. `N_CHAINS` independent chains, `N_BURN` burn-in + `N_TALLY` tallied
/// collisions each. The collision-rate weight for free gas is `1/(σ·v)` with
/// `σ ∝ v̄_rel(E)/v(E)`, i.e. `w = 1/v̄_rel(E)`, the standard closed form.
///
/// Chains start **hot** (`e_start = 40 kT ≈ 2.07 eV`) so that reaching 600 K is
/// a statement about the kernel relaxing, not about the starting guess.
///
/// # Measured (2026-09-12)
///
/// `T_eff = 600.44 +/- 0.31 K` against 600 K (**+0.07 %**, 1.4 sigma) and
/// `<E^2>/<E>^2 = 1.6660 +/- 0.0005` against `5/3` (**-0.04 %**) at 4x the
/// committed statistics; `601.28 +/- 0.63 K` and `1.6650 +/- 0.0010` at the
/// committed 1600 tallied collisions. The gate below is set at 0.5 % in
/// temperature and 0.5 % in shape, which is loose enough not to fire on the
/// committed statistics and tight enough that the bound laws' 1.24 % and 2.3 %
/// deviations cannot be blamed on the method.
#[test]
fn the_estimator_recovers_the_gas_temperature_it_is_calibrated_on() {
    const TEMP_K: f64 = 600.0;
    const KT: f64 = K_BOLTZMANN_EV_PER_K * TEMP_K;
    const AWR_C12: f64 = 11.893_65;

    let sigma = |e: f64| mean_relative_speed(e, AWR_C12, KT) / e.sqrt();
    let collide = |e: f64, seed: &mut u64| {
        let mu = 2.0 * prn(seed) - 1.0;
        free_gas_elastic_scatter(e, z_dir(), AWR_C12, KT, mu, seed).0
    };
    let (t, t_sem, shape, shape_sem) = equilibrium(40.0 * KT, 0x5EED_0001, &sigma, &collide);

    println!(
        "[free-gas C-12] T_eff = {t:.2} +/- {t_sem:.2} K (nominal {TEMP_K}), \
         shape = {shape:.4} +/- {shape_sem:.4} (Maxwellian {MAXWELLIAN_SHAPE:.4})"
    );
    let dt = (t - TEMP_K) / TEMP_K;
    assert!(
        dt.abs() < 0.005,
        "the free-gas control itself is off: T_eff = {t:.2} +/- {t_sem:.2} K \
         against {TEMP_K} K ({:+.2} %) -- the estimator is wrong, not the law",
        100.0 * dt
    );
    let ds = (shape - MAXWELLIAN_SHAPE) / MAXWELLIAN_SHAPE;
    assert!(
        ds.abs() < 0.005,
        "the free-gas control's shape is off: {shape:.4} +/- {shape_sem:.4} \
         against {MAXWELLIAN_SHAPE:.4} ({:+.2} %)",
        100.0 * ds
    );
}

/// `v̄_rel(E)` in `|v| = √E` units — the mean speed of a neutron of energy `E`
/// relative to a Maxwellian gas of mass ratio `awr` at `kt`. Same closed form
/// the in-crate free-gas equilibrium test uses.
fn mean_relative_speed(e: f64, awr: f64, kt: f64) -> f64 {
    let a = (awr * e / kt).sqrt();
    e.sqrt()
        * ((1.0 + 1.0 / (2.0 * a * a)) * erf(a)
            + (-a * a).exp() / (a * std::f64::consts::PI.sqrt()))
}

/// Abramowitz & Stegun 7.1.26 — `|error| < 1.5e-7`.
fn erf(x: f64) -> f64 {
    const P: f64 = 0.327_591_1;
    const A: [f64; 5] = [
        0.254_829_592,
        -0.284_496_736,
        1.421_413_741,
        -1.453_152_027,
        1.061_405_429,
    ];
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + P * x);
    let poly = A.iter().rev().fold(0.0, |acc, &c| (acc + c) * t);
    sign * (1.0 - poly * (-x * x).exp())
}

// ─────────────────────────────────────────────────────────────────────────────
// The bound laws
// ─────────────────────────────────────────────────────────────────────────────

/// Walk one chain on a bound law, falling back to nothing: at 600 K the
/// Maxwellian tail above the 4 eV thermal cutoff is `e^(−77)`, so a walker that
/// leaves the bound region is a defect, not a physical excursion, and the
/// assertion says so.
fn bound_collide(law: &ThermalScattering, e: f64, seed: &mut u64) -> f64 {
    match law.sample(e, seed) {
        Some((e_out, _mu)) if e_out > 0.0 && e_out.is_finite() => e_out,
        other => panic!(
            "the bound law declined to scatter at {e} eV (cutoff {} eV): {other:?}",
            law.cutoff_ev()
        ),
    }
}

/// **`c_Graphite` at 600 K — does the FHR pebble's moderator thermalise onto
/// 600 K?**
///
/// # Methodology
///
/// `ThermalScattering::from_endf_file` on `reference-data/endf/tsl-crystalline-graphite.endf`
/// (MAT 30, ENDF/B-VIII.0 crystalline graphite) at 600 K — the same law, built
/// the same way, that `examples/fhr_ring_rpt_endf.rs` attaches to the pebble's
/// matrix, shell and coating carbon. `N_CHAINS` chains, `N_BURN` burn-in +
/// `N_TALLY` tallied collisions, weight `1/(σ_thermal(E)·√E)`.
///
/// Both channels are sampled as transport samples them: coherent elastic with
/// probability `σ_el/σ_thermal` (lab-elastic, `E′ = E`, which the stationary
/// distribution tolerates because it is symmetric in `E ↔ E′`), incoherent
/// inelastic otherwise.
///
/// Run from two starting energies two decades apart — hot (`40 kT ≈ 2.07 eV`,
/// just under the 4 eV cutoff) and cold (`0.2 kT ≈ 0.0103 eV`) — because two
/// chains that agree have demonstrably forgotten where they started, and one
/// that has not is measuring its own burn-in.
///
/// # Measured (2026-09-12) — THIS IS A DEFECT, RECORDED NOT FIXED
///
/// ```text
///   hot  start (2.07 eV)   T_eff = 607.21 +/- 0.89 K   shape = 1.6286 +/- 0.0015
///   cold start (0.0103 eV) T_eff = 607.20 +/- 0.88 K   shape = 1.6286 +/- 0.0015
///   4x statistics          T_eff = 607.42 +/- 0.45 K   shape = 1.6282 +/- 0.0008
/// ```
///
/// The two starting energies agree to **0.01 K (0.01 sigma)**, so this is a
/// fixed point and not a burn-in artefact, and 4x the statistics moves it by
/// 0.2 K, so it is converged. Against a nominal 600 K that is **+1.24 %**,
/// 16 sigma, on a control that resolves 0.07 %.
///
/// **What it means.** A neutron population in infinite 600 K graphite, scattered
/// only by this law, settles onto a Maxwellian at 607.4 K instead of 600 K, and
/// one that is 2.3 % narrow in `<E^2>/<E>^2` besides. Every thermal reaction
/// rate in a graphite-moderated calculation is then evaluated on a spectrum
/// sitting at the wrong energy.
///
/// **What it is.** The tail of GitHub #190. The same walk on the pre-resize
/// constants (48 incident energies x 16 equiprobable outgoing energies) gives
/// `614.28 +/- 0.83 K` and `shape = 1.5652`, so resizing the table to 384 x 64
/// bought **-7.1 K** of the displacement and two thirds of the shape deficit.
/// What is left is the same defect, smaller: an equiprobable pre-tabulation fine
/// enough to reproduce the per-collision first moment is still not fine enough
/// to carry detailed balance across a whole thermalisation.
///
/// Tracked as `op-bo02` / GitHub #191.
#[test]
fn graphite_thermalises_onto_its_own_temperature() {
    const TEMP_K: f64 = 600.0;
    const KT: f64 = K_BOLTZMANN_EV_PER_K * TEMP_K;
    let Some(law) = law_or_skip("tsl-crystalline-graphite.endf", 30, TEMP_K, "c_Graphite") else {
        return;
    };
    let sigma = |e: f64| law.total_xs(e);
    let collide = |e: f64, seed: &mut u64| bound_collide(&law, e, seed);

    let hot = equilibrium(40.0 * KT, 0x5EED_0002, &sigma, &collide);
    let cold = equilibrium(0.2 * KT, 0x5EED_0003, &sigma, &collide);
    report(
        "c_Graphite",
        TEMP_K,
        Recorded {
            t_eff_k: 607.21,
            t_tol_k: 3.0,
            shape: 1.6286,
            shape_tol: 0.006,
        },
        hot,
        cold,
    );
}

/// **`c_H_in_H2O` at 293.6 K — does the LEU-COMP-THERM-008 moderator thermalise
/// onto 293.6 K?**
///
/// # Methodology
///
/// As `graphite_thermalises_onto_its_own_temperature`, on
/// `reference-data/endf/tsl-HinH2O.endf` (MAT 1) at 293.6 K. Water has no
/// elastic channel, so every collision is incoherent-inelastic and the walk is
/// a pure test of the emission tables.
///
/// This law is the one GitHub #188 reports as **too narrow** against THERMR's
/// MF=6. A width error is a second-moment statement about a single collision;
/// this test asks the independent question of whether the same law has the right
/// fixed point.
///
/// # Measured (2026-09-12) — a smaller defect of the same family
///
/// ```text
///   hot  start (1.01 eV)    T_eff = 294.28 +/- 0.23 K   shape = 1.6317 +/- 0.0009
///   cold start (0.00506 eV) T_eff = 294.67 +/- 0.24 K   shape = 1.6300 +/- 0.0009
///   4x statistics           T_eff = 294.31 +/- 0.12 K   shape = 1.6315 +/- 0.0004
/// ```
///
/// **+0.24 %** in temperature (6 sigma) — a fifth of graphite's displacement —
/// but **-2.11 %** in shape, which is the same deficit graphite shows (-2.31 %).
/// Two evaluations with nothing physical in common, one with an elastic channel
/// and one without, landing on the same shape error, is an argument that the
/// shape error belongs to the tabulation both share rather than to either
/// evaluation.
///
/// The same walk on the pre-resize emission table (48 x 16, GitHub #190 open)
/// gives `291.14 +/- 0.22 K` (**-0.84 %**) and `shape = 1.5806` (**-5.17 %**).
/// So the resize moved water's fixed point *through* the nominal temperature,
/// from 2.5 K low to 0.9 K high, while monotonically repairing the shape. A
/// displacement that changes sign under a refinement is a discretisation error,
/// not a physics one -- which is the cleanest statement this file can make about
/// where the defect lives.
///
/// Read against GitHub #188 (this law's kernel is too *narrow* against THERMR's
/// MF=6): a per-collision width error and a fixed-point error are independent
/// statements, and this test says the fixed point is nearly right while the
/// shape around it is not.
///
/// Tracked as `op-bo02` / GitHub #191.
#[test]
fn light_water_thermalises_onto_its_own_temperature() {
    const TEMP_K: f64 = 293.6;
    const KT: f64 = K_BOLTZMANN_EV_PER_K * TEMP_K;
    let Some(law) = law_or_skip("tsl-HinH2O.endf", 1, TEMP_K, "c_H_in_H2O") else {
        return;
    };
    let sigma = |e: f64| law.total_xs(e);
    let collide = |e: f64, seed: &mut u64| bound_collide(&law, e, seed);

    let hot = equilibrium(40.0 * KT, 0x5EED_0004, &sigma, &collide);
    let cold = equilibrium(0.2 * KT, 0x5EED_0005, &sigma, &collide);
    report(
        "c_H_in_H2O",
        TEMP_K,
        Recorded {
            t_eff_k: 294.48,
            t_tol_k: 1.0,
            shape: 1.6309,
            shape_tol: 0.006,
        },
        hot,
        cold,
    );
}

/// What a bound law's fixed point **is**, as measured on 2026-09-12 — not what
/// it should be. Both rows are defects; see the module doc.
struct Recorded {
    /// Fixed-point temperature the law actually reaches \[K\], at the committed
    /// `N_CHAINS`/`N_BURN`/`N_TALLY` and the seeds below.
    t_eff_k: f64,
    /// How far `t_eff_k` may move before the gate fires \[K\]. Sized at ~4x the
    /// standard error of the hot/cold mean, so it rides out the statistics but
    /// fires when the defect is fixed — at which point the right response is to
    /// record the new value and tighten, not to widen this.
    t_tol_k: f64,
    /// `<E^2>/<E>^2` at the fixed point (Maxwellian: `5/3`).
    shape: f64,
    /// Tolerance on `shape`, same sizing argument.
    shape_tol: f64,
}

/// Print both starting energies, assert the chain has converged (they agree),
/// then assert the converged fixed point against what was recorded.
///
/// Three claims, in order of what they are worth:
///
/// 1. **The two starting energies agree.** Without this, neither number is a
///    fixed point — it is a burn-in measurement. Gated at 6 sigma.
/// 2. **The fixed point is where it was recorded.** This is the drift gate. It
///    fires both ways: on a regression, and on the defect being fixed.
/// 3. **The fixed point has not gone wild.** A hard 5 % ceiling on the
///    temperature, so a catastrophic regression fails as a physics error rather
///    than as a drift one, whatever the recorded value happens to be.
fn report(
    name: &str,
    temp_k: f64,
    rec: Recorded,
    hot: (f64, f64, f64, f64),
    cold: (f64, f64, f64, f64),
) {
    let (th, th_sem, sh, sh_sem) = hot;
    let (tc, tc_sem, sc, sc_sem) = cold;
    println!(
        "[{name}] hot  start: T_eff = {th:.2} +/- {th_sem:.2} K, shape = {sh:.4} +/- {sh_sem:.4}"
    );
    println!(
        "[{name}] cold start: T_eff = {tc:.2} +/- {tc_sem:.2} K, shape = {sc:.4} +/- {sc_sem:.4}"
    );

    let n_sigma = (th - tc).abs() / (th_sem * th_sem + tc_sem * tc_sem).sqrt().max(1.0e-12);
    let t = 0.5 * (th + tc);
    let shape = 0.5 * (sh + sc);
    println!(
        "[{name}] fixed point {t:.2} K against a nominal {temp_k} K ({:+.2} %), \
         shape {shape:.4} against {MAXWELLIAN_SHAPE:.4} ({:+.2} %); \
         hot-cold {:+.2} K ({n_sigma:.2} sigma); recorded {:.2} K / {:.4}",
        100.0 * (t - temp_k) / temp_k,
        100.0 * (shape - MAXWELLIAN_SHAPE) / MAXWELLIAN_SHAPE,
        th - tc,
        rec.t_eff_k,
        rec.shape,
    );

    assert!(
        n_sigma < 6.0,
        "[{name}] the two starting energies do not agree ({th:.2} vs {tc:.2} K, \
         {n_sigma:.1} sigma) -- the chain is still remembering where it started, \
         so neither number is a fixed point"
    );
    assert!(
        (t - rec.t_eff_k).abs() < rec.t_tol_k,
        "[{name}] the kernel's fixed point moved: {t:.2} K against the recorded \
         {:.2} +/- {:.2} K (nominal {temp_k} K). If the law was FIXED this is the \
         expected failure -- record the new value and tighten. If nothing was \
         meant to change here, it is a regression.",
        rec.t_eff_k,
        rec.t_tol_k,
    );
    assert!(
        (shape - rec.shape).abs() < rec.shape_tol,
        "[{name}] the fixed point's shape moved: <E^2>/<E>^2 = {shape:.4} against \
         the recorded {:.4} +/- {:.4} (Maxwellian {MAXWELLIAN_SHAPE:.4})",
        rec.shape,
        rec.shape_tol,
    );

    let dt = (t - temp_k) / temp_k;
    assert!(
        dt.abs() < 0.05,
        "[{name}] the kernel's fixed point is {t:.2} K against a nominal \
         {temp_k} K ({:+.2} %) -- past the 5 % ceiling this test holds \
         regardless of what is recorded",
        100.0 * dt
    );
}
