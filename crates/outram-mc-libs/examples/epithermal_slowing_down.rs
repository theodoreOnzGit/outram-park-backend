//! Mean log energy decrement **above** the S(α,β) cutoff — the part of the
//! slowing-down range where resonance escape is actually decided, and the part
//! `graphite_energy_decrement.rs` never reached.
//!
//! # Why this exists
//!
//! The FHR pebble sits ~+1.7 % above the OpenMC reference, with `p` too high and
//! `ε` too low — a spectrum that is too soft. Every named data mechanism is
//! excluded by measurement: point σ against NJOY's PENDF (±0.04 %), the capture
//! **resonance integral** against NJOY *and* the published RI_∞
//! (`u238_resonance_integral.rs`, +0.00 %), graphite σ against THERMR (±0.05 %).
//!
//! What was measured of the *kernel* was `ξ = ⟨ln(E/E′)⟩` from 0.0253 eV to
//! **3.9 eV** — entirely inside the bound-atom regime, where
//! `ThermalScattering::sample` runs. But U-238's resonances span **6 eV to
//! 20 keV**, and there the transport kernel takes a completely different branch:
//! `sample_thermal` returns `None` and the collision goes through
//! `sample_elastic_mu_cm` / `elastic_scatter`, i.e. target-at-rest two-body
//! kinematics. That branch has never been measured.
//!
//! It is exactly the branch that would produce the observed signature. Too much
//! energy per collision above the cutoff means fewer collisions inside the
//! resonance band, so fewer resonance captures (`p` up), a softer fast spectrum
//! (`ε` down) and a higher `k` — all three, from one defect.
//!
//! The per-collision kinematics turned out to be exact (`ξ/ξ₀ = 1.000` from 4 eV
//! to 10 keV, no kinematic-floor violations). The defect was one level up, and
//! only the second section below can see it.
//!
//! # The oracle is analytic, so no library is needed
//!
//! For isotropic-CM elastic scattering off a target at rest,
//!
//! ```text
//! α   = ((A−1)/(A+1))²
//! ξ   = 1 + α·ln α / (1 − α)          ⟨E′/E⟩ = (1 + α)/2
//! ```
//!
//! with `A` the nuclide's own AWR. Anisotropy in the CM shifts `ξ` by a small
//! amount proportional to `μ̄_cm`, which for carbon is well under 1 % below
//! ~100 keV — so a multi-percent departure is a defect, not anisotropy. The
//! floor `α·E` is a hard kinematic bound: **no** elastic collision may drop the
//! neutron below it, and a violation is unambiguous.
//!
//! # What each section measures
//!
//! **Section 1 — per-collision kinematics vs the analytic oracle.** The bound law
//! where a nuclide has one, otherwise the *target-at-rest* two-body form, which is
//! the branch the oracle above describes exactly. Every non-thermal row must land
//! on `ξ₀` and must respect the `α·E` floor.
//!
//! **Section 2 — the thermal fixed point.** The full transport branch, free-gas
//! target motion included (`free_gas_elastic_scatter`, gated at `400·kT` as
//! OpenMC gates it). Section 1 cannot see what this measures: each individual
//! target-at-rest collision is valid, and yet the sequence of them has **no
//! equilibrium** — held-at-rest nuclei can only take energy away, so a neutron
//! random walk cools without bound. That was the defect (bead `op-50vu`), and
//! this section is where it shows and where the fix is checked.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example epithermal_slowing_down
//! ```

use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::physics::scatter::{
    elastic_scatter, free_gas_elastic_scatter, two_body_scatter_with_mu, K_BOLTZMANN_EV_PER_K,
};
use outram_mc_libs::geometry::position::Direction;

const TEMP: f64 = 600.0;
const N: usize = 200_000;

/// Energies spanning the thermal range, the S(α,β) seam and the whole U-238
/// resonance band. `kT = 0.0517 eV` at 600 K, so the first three rows are inside
/// the Maxwellian where a free-gas target's own motion dominates.
const PROBE_EV: &[f64] = &[
    0.0253, 0.0517, 0.2, 1.0, 2.0, 3.9, 4.1, 5.0, 6.674, 10.0, 20.87, 100.0, 1.0e3, 1.0e4, 1.0e5,
    1.0e6, 5.0e6,
];

/// OpenMC's `FREE_GAS_THRESHOLD` — below `400·kT` a scatter off a nuclide with no
/// S(α,β) table must sample the target's thermal motion.
const FREE_GAS_THRESHOLD: f64 = 400.0;

fn main() {
    let dir = endf_dir();
    let sab = ThermalScattering::from_endf_file(
        dir.join("tsl-crystalline-graphite.endf")
            .to_str()
            .expect("path"),
        30,
        TEMP,
        "c_Graphite",
    )
    .expect("crystalline-graphite S(a,b)");

    // The moderating nuclides of the pebble, plus U-238 as the heavy control:
    // its alpha is 0.983, so a correct kernel can barely move it.
    let cases: Vec<(String, Nuclide)> = vec![
        (
            "C12 (graphite S(a,b))".into(),
            load("C12", "n-006_C_012-ENDF8.0.endf", &dir).with_thermal_scattering(sab.clone()),
        ),
        (
            "C12 (free gas, kernel C)".into(),
            load("C12", "n-006_C_012-ENDF8.0.endf", &dir),
        ),
        (
            "Be9  (FLiBe)".into(),
            load("Be9", "n-004_Be_009-ENDF8.0.endf", &dir),
        ),
        (
            "F19  (FLiBe)".into(),
            load("F19", "n-009_F_019-ENDF8.0.endf", &dir),
        ),
        (
            "Li7  (FLiBe)".into(),
            load("Li7", "n-003_Li_007-ENDF8.0.endf", &dir),
        ),
        (
            "O16  (UCO kernel)".into(),
            load("O16", "n-008_O_016-ENDF8.0.endf", &dir),
        ),
        (
            "Si28 (SiC coating)".into(),
            load("Si28", "n-014_Si_028-ENDF8.0.endf", &dir),
        ),
        (
            "U238 (heavy control)".into(),
            load("U238", "n-092_U_238.endf", &dir),
        ),
    ];

    let mut seed = 20_260_911_u64;
    let mut any_violation = false;
    let mut kinematics: Vec<KinRow> = Vec::new();

    for (label, nuc) in &cases {
        let a = nuc.awr;
        let alpha = ((a - 1.0) / (a + 1.0)).powi(2);
        let xi_iso = 1.0 + alpha * alpha.ln() / (1.0 - alpha);
        let ratio_iso = 0.5 * (1.0 + alpha);

        println!("\n=== {label}   AWR = {a:.5}   alpha = {alpha:.6} ===");
        println!(
            "isotropic-CM, target-at-rest oracle:  xi = {xi_iso:.6}   <E'/E> = {ratio_iso:.6}"
        );
        println!(
            "{:>10} {:>7} {:>11} {:>11} {:>9} {:>9} {:>8} {:>9}",
            "E [eV]", "thermal", "<E'/E>", "xi", "xi/xi_0", "ratio/r0", "up-sc %", "below aE"
        );

        for &e in PROBE_EV {
            let (mut sum_ratio, mut sum_xi) = (0.0_f64, 0.0_f64);
            let (mut n_up, mut n_thermal, mut n_below) = (0usize, 0usize, 0usize);
            let u = Direction::new(0.0, 0.0, 1.0);
            for _ in 0..N {
                // Exactly the branch order of `keff_delta::simulate` and
                // `transport_csg`: bound law if the nuclide has one and it
                // covers this energy, otherwise the elastic kernel.
                let ep = if let Some((e_out, _mu)) = nuc.sample_thermal(e, &mut seed) {
                    n_thermal += 1;
                    e_out
                } else {
                    match nuc.sample_elastic_mu_cm(e, &mut seed) {
                        Some(mu_cm) => two_body_scatter_with_mu(e, u, a, 0.0, mu_cm, &mut seed).0,
                        None => elastic_scatter(e, u, a, &mut seed).0,
                    }
                };
                if ep <= 0.0 {
                    continue;
                }
                sum_ratio += ep / e;
                sum_xi += (e / ep).ln();
                if ep > e {
                    n_up += 1;
                }
                // Kinematic floor. Only meaningful outside the bound regime --
                // the S(a,b) law legitimately breaks it via lattice recoil.
                if ep < alpha * e * (1.0 - 1.0e-9) {
                    n_below += 1;
                }
            }
            let k = N as f64;
            let (ratio, xi) = (sum_ratio / k, sum_xi / k);
            let th_frac = 100.0 * n_thermal as f64 / k;
            let below = 100.0 * n_below as f64 / k;
            // A free-kinematics violation is a hard defect; inside the bound
            // regime it is expected, so only flag it where thermal is off.
            if th_frac < 1.0 && below > 0.01 {
                any_violation = true;
            }
            println!(
                "{e:>10.4e} {th_frac:>6.1}% {ratio:>11.5} {xi:>11.5} {:>9.4} {:>9.4} {:>7.1} {below:>8.2}%",
                xi / xi_iso,
                ratio / ratio_iso,
                100.0 * n_up as f64 / k,
            );
            kinematics.push(KinRow {
                label: label.clone(),
                e,
                th_frac,
                xi,
                xi_iso,
                ratio,
                ratio_iso,
                up_pct: 100.0 * n_up as f64 / k,
                below_pct: below,
            });
        }
    }

    let equilibrium = thermalization_demo(&cases);
    scattering_cross_sections(&cases);

    println!(
        "\nHow to read this.\n\
         - `thermal` is the share of collisions that took the S(a,b) branch. Where\n\
           it is 0 the row is pure two-body kinematics and the oracle is exact.\n\
         - `xi/xi_0` and `ratio/r0` must both be 1.000 on those rows, up to the CM\n\
           anisotropy of the evaluation (sub-percent for carbon below ~100 keV,\n\
           growing in the MeV range). Which way each moves is nuclide-specific:\n\
           carbon is forward-peaked at 1 MeV, Li-7 backward-peaked at the same\n\
           energy, and O-16 raises both moments at once (concavity of ln, not a\n\
           contradiction). Above ~10 keV the durable claims are per-sample ones.\n\
         - `below aE` must be 0.00 % on every non-thermal row: alpha*E is a hard\n\
           kinematic floor for elastic scattering off a target at rest.\n\
         - `up-sc %` must be 0.0 on every non-thermal row for the same reason."
    );
    vv_gate(&kinematics, &equilibrium, any_violation);
}

/// Highest energy at which CM anisotropy is still negligible for these
/// moderators, in eV. Above it `xi/xi_0` legitimately departs from 1 — forward
/// scattering sets in, `<E'/E>` rises and `xi` falls — so the exact-kinematics
/// claim is made below this bound and the anisotropy claim above it.
const ANISOTROPY_ONSET_EV: f64 = 1.0e4;

/// Absolute envelope on `xi` and on `<E'/E>` below [`ANISOTROPY_ONSET_EV`].
///
/// **Absolute, not relative, and deliberately so.** For near-elastic scattering
/// `xi ~= -ln<E'/E>`, so an error in `<E'/E>` propagates into `xi` amplified by
/// `1/xi_0`. Carbon's `xi_0` is 0.159 (amplification 6x); U-238's is 0.00845
/// (amplification 118x). A uniform *relative* envelope is therefore wrong by
/// construction — 1 % would be a reasonable bar on Li-7 and an impossible one on
/// U-238, whose measured `xi/xi_0 = 0.9904` at 10 keV is a 1e-4 departure in
/// `<E'/E>` and nothing more. In absolute terms the two quantities agree to the
/// same 1e-4, uniformly across mass number, which is what this envelope checks.
///
/// Worst measured: **7.7e-4** (Be-9 at 20.87 eV), against the 1.5e-3 bound.
const XI_ABS_TOL: f64 = 1.5e-3;

/// One row of the per-collision kinematics sweep, kept for the V&V gate.
struct KinRow {
    label: String,
    /// Incident energy, eV.
    e: f64,
    /// Percentage of collisions that took the S(a,b) branch.
    th_frac: f64,
    /// Sampled mean log energy decrement `<ln(E/E')>`.
    xi: f64,
    /// Closed-form `xi_0` for isotropic CM scattering off a target at rest.
    xi_iso: f64,
    /// Sampled `<E'/E>`.
    ratio: f64,
    /// Closed-form `(1 + alpha)/2`.
    ratio_iso: f64,
    /// Percentage of collisions that gained energy.
    up_pct: f64,
    /// Percentage of collisions that landed below the `alpha*E` floor.
    below_pct: f64,
}

/// V&V gate: two-body kinematics against their closed form, and thermal
/// equilibrium against its analytic fixed point.
///
/// # Methodology and oracles
///
/// Three oracles, all analytic — no library, no second code:
///
/// 1. **Isotropic-CM elastic scattering off a target at rest.** For a nuclide of
///    mass ratio `A`, `alpha = ((A-1)/(A+1))^2`, and then `xi_0 = 1 + alpha
///    ln(alpha)/(1-alpha)` and `<E'/E>_0 = (1+alpha)/2` exactly. Below
///    [`ANISOTROPY_ONSET_EV`] the evaluations' CM anisotropy is negligible for
///    these moderators, so the sampled moments must land on the closed form.
/// 2. **The kinematic floor.** `E' >= alpha*E` is a hard bound for elastic
///    scattering off a target at rest. A single violation is unambiguous — there
///    is no tolerance to argue about. Inside the bound (S(a,b)) regime it is
///    *expected*, because lattice recoil legitimately breaks it, so the claim is
///    made only on rows where the thermal branch is not being taken.
/// 3. **The thermal fixed point.** A random walk over *collisions* in a free gas
///    relaxes to the collision density `n(E) v sigma ~ E exp(-E/kT)` — a
///    Gamma(2, kT) with mean **2 kT**, not the neutron density's 1.5 kT.
///
/// # Results (2026-09-11, ENDF/B-VIII.0)
///
/// **Kinematics.** 106 non-thermal rows from 0.0253 eV to 10 keV, across all
/// eight nuclides, match the closed form. Worst `|xi - xi_0| = 7.7e-4` (Be-9 at
/// 20.87 eV) against the [`XI_ABS_TOL`] = 1.5e-3 envelope, and `<E'/E>` agrees
/// to the same order. See [`XI_ABS_TOL`] for why the envelope is absolute rather
/// than relative — in relative terms the same rows run from 0.1 % on Li-7 to
/// 1.0 % on U-238 purely because of how small `xi_0` is for a heavy nuclide. Zero kinematic-floor violations and zero
/// up-scatters anywhere outside the bound regime.
///
/// Above 10 keV the moments leave the isotropic-CM oracle as real CM anisotropy
/// sets in, and **there is less that can honestly be asserted there than it
/// first appears**. The sign of the departure is not universal — C-12 is
/// forward-peaked at 1 MeV (`xi/xi_0 = 0.913`, `ratio/r0 = 1.014`) while Li-7 is
/// *backward*-peaked there (1.044 and 0.989) — and the two moments need not even
/// move opposite ways, because `ln` is concave and extra spread in the outgoing
/// distribution raises `xi` at fixed `<E'/E>`. O-16 at 1 MeV moves both up
/// (1.0010 and 1.0079).
///
/// What stays rigorous above the onset is **per-sample**: `alpha*E <= E' <= E`
/// for every collision off a target at rest, whatever the angular distribution.
/// Those two are asserted on every row. The exact `mubar_cm` ↔ `<E'/E>` relation
/// is checked per-sample in `tests/slowing_down_kinematics.rs`.
///
/// **Thermal equilibrium**, `<E>/2kT` after 300 collisions from 1 eV:
///
/// ```text
///   C12 (graphite S(a,b))    0.9706      O16  (UCO kernel)     0.9846
///   C12 (free gas)           0.9801      Si28 (SiC coating)    0.9969
///   Be9  (FLiBe)             0.9788      Li7  (FLiBe)          0.9520
///   F19  (FLiBe)             0.9892      U238 (heavy control)  1.2908
/// ```
///
/// The seven light nuclides sit within 5 % of the fixed point. **U-238 at 1.29
/// is expected and is not a defect**: a mass-238 target takes `~1/xi = 172`
/// collisions per decade of lethargy against carbon's 6, so 300 collisions is
/// nowhere near enough to relax it. It is in the table as the control that shows
/// the relaxation is being driven by the mass ratio and not by the harness.
///
/// # Interpretation
///
/// The per-collision kinematics above the S(a,b) cutoff are **exact**. This was
/// the branch suspected of producing the ring-RPT residual — too much energy per
/// collision above the cutoff would give fewer resonance captures (`p` up), a
/// softer fast spectrum (`epsilon` down) and a higher `k`, all three from one
/// defect — and the measurement excludes it.
fn vv_gate(kinematics: &[KinRow], equilibrium: &[(String, f64)], any_violation: bool) {
    use outram_mc_libs::vv::assert_absolute;

    println!("\n=== V&V gate: two-body kinematics and the thermal fixed point ===");

    assert!(
        !any_violation,
        "the alpha*E kinematic floor was violated outside the bound regime. That \
         is a hard bound for elastic scattering off a target at rest — there is \
         no tolerance to argue about here, and a violation means the elastic \
         kernel is producing outgoing energies that no two-body collision can."
    );
    println!("  [PASS] no kinematic-floor violations outside the bound regime");

    let mut n_exact = 0usize;
    let mut worst = (0.0_f64, String::new(), 0.0_f64);
    for row in kinematics {
        let KinRow {
            label,
            e,
            th_frac,
            xi,
            xi_iso,
            ratio,
            ratio_iso,
            up_pct,
            below_pct,
        } = row;
        let (e, up_pct, below_pct) = (*e, *up_pct, *below_pct);
        let xi_rel = &(xi / xi_iso);
        let ratio_rel = &(ratio / ratio_iso);
        // Rows where the S(a,b) branch is taken are not described by the
        // target-at-rest oracle at all, so they are skipped rather than excused.
        if *th_frac >= 1.0 {
            continue;
        }
        assert_eq!(
            up_pct, 0.0,
            "{label} at {e} eV up-scattered on {up_pct} % of collisions outside the \
             bound regime. A target at rest has no energy to give; only the S(a,b) \
             branch may up-scatter."
        );
        assert_eq!(
            below_pct, 0.0,
            "{label} at {e} eV dropped below alpha*E on {below_pct} % of collisions"
        );
        if e <= ANISOTROPY_ONSET_EV {
            n_exact += 1;
            if (xi - xi_iso).abs() > worst.0 {
                worst = ((xi - xi_iso).abs(), label.clone(), e);
            }
            // Both claims are ABSOLUTE, not relative, and that is the whole
            // point -- see the doc comment. xi is ill-conditioned for a heavy
            // nuclide (U-238's xi_0 is 0.00845, so a 1e-4 error in <E'/E>
            // becomes a 1 % error in xi) and a uniform RELATIVE envelope would
            // be far too tight on U-238 and far too loose on Li-7.
            assert_absolute(
                &format!(
                    "{label} @ {e:.4e} eV: xi vs the closed form {xi_iso:.6} \
                     (isotropic CM, target at rest)"
                ),
                *xi,
                *xi_iso,
                XI_ABS_TOL,
            );
            assert_absolute(
                &format!("{label} @ {e:.4e} eV: <E'/E> vs the closed form (1+alpha)/2"),
                *ratio,
                *ratio_iso,
                XI_ABS_TOL,
            );
        } else {
            // Above the anisotropy onset the departure from the isotropic-CM
            // oracle is real physics, and there is LESS to assert here than it
            // first appears. Two drafts of this gate over-claimed and both were
            // caught by the measurement:
            //
            //   1. "forward scattering, so xi falls and <E'/E> rises." Not
            //      universal. C-12 is forward-peaked at 1 MeV (xi/xi_0 = 0.913,
            //      ratio/r0 = 1.014); Li-7 is BACKWARD-peaked at the same energy
            //      (1.044 and 0.989).
            //   2. "then at least they must move OPPOSITE ways." Also wrong.
            //      That holds for a pure SHIFT in mubar_cm, but the CM
            //      distribution is not a one-parameter family: ln is concave, so
            //      extra spread lowers <ln E'> at fixed mean E', and a
            //      distribution can raise both <E'/E> and xi at once. O-16 at
            //      1 MeV does exactly that (1.0010 and 1.0079).
            //
            // What IS rigorous above the onset is per-sample, not per-moment:
            // alpha*E <= E' <= E for every single collision off a target at
            // rest, whatever the angular distribution. Those two are asserted
            // above for every row. Beyond them, the honest claim is boundedness.
            // The exact mubar_cm <-> <E'/E> relation is checked per-sample in
            // `tests/slowing_down_kinematics.rs`, which is where it belongs.
            assert!(
                *xi_rel > -2.0 && *xi_rel < 2.0 && *ratio_rel > 0.5 && *ratio_rel < 1.5,
                "{label} at {e} eV has xi/xi_0 = {xi_rel:.4}, ratio/r0 = \
                 {ratio_rel:.4}. CM anisotropy cannot move the moments this far; \
                 the outgoing-energy sampling has broken down."
            );
        }
    }
    assert!(
        n_exact >= 50,
        "only {n_exact} non-thermal rows below {ANISOTROPY_ONSET_EV} eV were \
         compared against the closed form; the sweep should produce well over 50 \
         across eight nuclides. Either the probe list or the thermal branch has \
         changed and most rows are being skipped."
    );
    println!(
        "  [PASS] {n_exact} non-thermal rows match the closed-form kinematics; \
         worst |xi - xi_0| = {:.2e} ({} @ {:.4e} eV), envelope {XI_ABS_TOL:.1e}",
        worst.0, worst.1, worst.2,
    );

    // The thermal fixed point. U-238 is deliberately exempt: see the doc comment.
    for (label, ratio) in equilibrium {
        if label.starts_with("U238") {
            println!(
                "  [skip] {label}: <E>/2kT = {ratio:.4}. A mass-238 target needs \
                 ~172 collisions per lethargy decade against carbon's 6, so 300 \
                 collisions cannot relax it. This row is the control, not a claim."
            );
            continue;
        }
        assert_absolute(
            &format!("{label}: <E>/2kT at the free-gas thermal fixed point"),
            *ratio,
            1.0,
            0.10,
        );
    }
    assert!(
        equilibrium.len() >= 8,
        "the thermalization demo returned {} media; eight were configured",
        equilibrium.len()
    );
}

/// The other half of `ξ·Σ_s`: the elastic cross section itself, against published
/// free-atom values.
///
/// `ξ` is verified exactly against two-body kinematics above, and the U-238
/// capture resonance integral is verified against NJOY — so if the epithermal
/// flux per unit lethargy (`≈ S/(ξΣ_s)`) is wrong, `Σ_s` is the only term left.
/// Between about 1 eV and 10 keV a light nuclide's elastic cross section is flat
/// at its potential-scattering value, which is tabulated:
///
/// | nuclide | free-atom σ_s \[b\] |
/// |---|---|
/// | C-12 | 4.746 |
/// | Be-9 | 6.151 |
/// | F-19 | 3.641 |
/// | Li-7 | 0.97 |
/// | O-16 | 3.761 |
/// | Si-28 | 2.04 |
///
/// These are round numbers from neutron-scattering tables, good to a few percent,
/// not an oracle at NJOY's 0.1 % — they are here to catch a *gross* error, which
/// is what the ring-RPT residual would need (a ~20 % shift in `ξΣ_s`). A real
/// check against NJOY's own PENDF for these nuclides has never been run; only
/// U-235, U-238 and graphite's thermal law have.
fn scattering_cross_sections(cases: &[(String, Nuclide)]) {
    println!("\n\n=== Elastic cross section vs published free-atom values ===");
    println!("(flat potential scattering is expected between ~1 eV and ~10 keV)\n");
    print!("{:<28}", "nuclide");
    for e in [1.0, 10.0, 100.0, 1.0e3, 1.0e4, 1.0e5] {
        print!("{:>12}", format!("{e:.0e} eV"));
    }
    println!();
    for (label, nuc) in cases {
        print!("{label:<28}");
        for e in [1.0, 10.0, 100.0, 1.0e3, 1.0e4, 1.0e5] {
            print!("{:>12.4}", nuc.xs_at_energy(e, TEMP).elastic);
        }
        println!();
    }
    println!(
        "\nA ~20 % error in one of these would be enough to explain the ring-RPT\n\
         residual on its own; a 0.1 % one would not, and is NJOY's job to find."
    );
}

/// Where does a neutron end up after many scatters in a single-nuclide medium at
/// 600 K?
///
/// This is the question the per-collision table cannot answer, because the defect
/// it looks for is not in any one collision — each is a valid target-at-rest
/// scatter — but in the **absence of an equilibrium**. A real free gas at
/// temperature `T` up-scatters as well as down-scatters, and a neutron random-
/// walking in it relaxes to a Maxwellian with `⟨E⟩ = 1.5·kT` and stays there. A
/// target held at rest can only take energy away, so the walk has no fixed point
/// and `E → 0` without bound.
///
/// Graphite is the control: it carries the S(α,β) law, so it must equilibrate.
fn thermalization_demo(cases: &[(String, Nuclide)]) -> Vec<(String, f64)> {
    const SCATTERS: usize = 400;
    const WALKERS: usize = 20_000;
    let kt = K_BOLTZMANN_EV_PER_K * TEMP;
    // The walk visits COLLISIONS, not the neutron density, so its fixed point is
    // the collision density n(E)*v*sigma ~ E*exp(-E/kT) (constant sigma, the CXS
    // approximation) -- a Gamma(2, kT) with mean 2kT, not the density's 1.5 kT.
    let e_fixed_point = 2.0 * kt;

    println!(
        "\n\n=== Thermal equilibrium: {WALKERS} walkers x {SCATTERS} scatters, start 1 eV ==="
    );
    println!(
        "kT = {kt:.6} eV at {TEMP} K;  a free gas must relax to <E> = 2 kT = {e_fixed_point:.6} eV"
    );
    println!(
        "free-gas target motion is required below 400 kT = {:.3} eV",
        FREE_GAS_THRESHOLD * kt
    );
    println!(
        "\n{:<28} {:>14} {:>14} {:>12}",
        "medium", "<E> final [eV]", "<E>/2kT", "median [eV]"
    );

    let mut seed = 77_030_412_u64;
    let mut out: Vec<(String, f64)> = Vec::new();
    for (label, nuc) in cases {
        let a = nuc.awr;
        let mut finals: Vec<f64> = Vec::with_capacity(WALKERS);
        let u = Direction::new(0.0, 0.0, 1.0);
        for _ in 0..WALKERS {
            let mut e = 1.0_f64;
            for _ in 0..SCATTERS {
                e = if let Some((e_out, _mu)) = nuc.sample_thermal(e, &mut seed) {
                    e_out
                } else {
                    // The transport kernel's own branch, so this measures what
                    // the Monte Carlo does, fix included.
                    let mu_cm = nuc
                        .sample_elastic_mu_cm(e, &mut seed)
                        .unwrap_or_else(|| 2.0 * outram_mc_libs::rng::lcg::prn(&mut seed) - 1.0);
                    free_gas_elastic_scatter(e, u, a, kt, mu_cm, &mut seed).0
                };
                if !(e > 0.0) || !e.is_finite() {
                    e = f64::MIN_POSITIVE;
                    break;
                }
            }
            finals.push(e);
        }
        finals.sort_by(|x, y| x.partial_cmp(y).unwrap());
        let mean = finals.iter().sum::<f64>() / finals.len() as f64;
        let median = finals[finals.len() / 2];
        println!(
            "{label:<28} {mean:>14.4e} {:>14.4e} {median:>12.4e}",
            mean / e_fixed_point
        );
        out.push((label.clone(), mean / e_fixed_point));
    }
    println!(
        "\nA medium whose `<E>/2kT` is not order 1 has no thermal equilibrium: its\n\
         nuclei are being held at rest, so every collision removes energy and the\n\
         random walk never turns around."
    );
    out
}

fn endf_dir() -> std::path::PathBuf {
    njoy_outram_park_fork::reference_data::reference_endf("n-006_C_012-ENDF8.0.endf")
        .expect("reference-data/endf/")
        .parent()
        .expect("parent")
        .to_path_buf()
}

fn load(name: &str, file: &str, dir: &std::path::Path) -> Nuclide {
    let p = dir.join(file);
    eprint!("  reconstructing {name:<6} … ");
    let t0 = std::time::Instant::now();
    let n = Nuclide::from_endf_file(&p, name, TEMP, 1.0e-3)
        .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display()));
    eprintln!("{:.1?}", t0.elapsed());
    n
}
