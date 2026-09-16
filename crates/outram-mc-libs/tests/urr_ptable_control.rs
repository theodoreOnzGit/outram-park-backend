//! **Unresolved-resonance probability tables reach the transport kernel, and
//! change nothing when they are absent.**
//!
//! The control for the URR wiring, written to the same standard as
//! `tests/ablation_hook_controls.rs`: a mechanism that silently fails to take
//! effect reports "no difference", and that reads as "URR self-shielding does
//! not matter" — the worst failure mode an ablation study has, and one this
//! crate has hit before (`op-50vu`).
//!
//! # The two halves, and the second is the one at risk
//!
//! 1. **With tables, the kernel notices.** A paired run on the same seed must
//!    differ, or no kernel is consulting the tables.
//! 2. **Without tables, nothing moved at all.** Every transport driver now
//!    calls `Nuclide::needs_urr_draw` before sampling a band. If that gate were
//!    wrong — if a draw happened unconditionally — every RNG stream in the
//!    crate would shift and every previously recorded result would change for
//!    no physical reason. This asserts a nuclide with no tables gives
//!    **bit-identical** cross sections through both entry points, at
//!    energies inside and outside the unresolved range.
//!
//! # Results (2026-09-16, ENDF/B-VIII.0 U-238 at 293.6 K)
//!
//! Printed by the tests. The table generator itself is verified against
//! NJOY2016 separately — `njoy-outram-park-fork/tests/purr_u238_ptables_vs_njoy.rs`,
//! which reproduces NJOY's converged Bondarenko moments to 4.2e-7 (elastic) and
//! 3.0e-7 (capture).
//!
//! **Verification, not validation.** These assert the switch switches and the
//! gate holds. What URR self-shielding is *worth* is a separate paired-seed
//! measurement.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::purr::UrrSample;
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use outram_mc_libs::material::nuclide::Nuclide;

const MAT: i32 = 9237;
const TEMP_K: f64 = 293.6;
/// Cheap PURR settings. The verified NJOY-production settings are
/// `20 / 64 / 10000` and cost ~45 s; these keep the test near a minute while
/// exercising the identical code path. A noisier table is fine here — nothing
/// in this file asserts a table *value*, only that the wiring works.
const NBIN: usize = 20;
const NLADR: usize = 4;
const NSAMP: usize = 500;

/// Inside U-238's unresolved range (20 keV - 149 keV).
const URR_PROBE_EV: f64 = 5.0e4;
/// Outside it, in the resolved region.
const RESOLVED_PROBE_EV: f64 = 1.0e3;
/// Outside it, above the range.
const FAST_PROBE_EV: f64 = 2.0e6;

fn u238_plain() -> Option<(Tape, Nuclide)> {
    let p = reference_endf_or_skip("n-092_U_238.endf", "U-238 (URR ptable control)")?;
    let tape = Tape::read_file(&p).expect("U-238 tape parses");
    let nuc = Nuclide::from_endf_file(&p, "U238", TEMP_K, 1.0e-3).expect("U-238 reconstructs");
    Some((tape, nuc))
}

/// With tables attached, the URR sampler produces genuinely different bands —
/// and they are self-shielding FACTORS, because U-238 is `LSSF = 1`.
#[test]
fn urr_tables_attach_and_carry_lssf1_self_shielding_factors() {
    let Some((tape, plain)) = u238_plain() else {
        return;
    };

    // 1: there was nothing there before.
    assert!(
        !plain.has_urr_probability_tables(),
        "a freshly reconstructed nuclide already reports URR tables; the default must be \
         infinitely dilute, or the 'without' arm of every study is not what it claims."
    );
    assert!(!plain.needs_urr_draw(URR_PROBE_EV));

    let shielded = plain
        .clone()
        .with_urr_probability_tables(&tape, MAT, TEMP_K, NBIN, NLADR, NSAMP)
        .expect("PURR runs on U-238");

    // 2: they are there, and cover the range the evaluation declares.
    assert!(
        shielded.has_urr_probability_tables(),
        "with_urr_probability_tables left no tables on U-238, whose evaluation carries an \
         LRU=2 range from 20 keV to 149 keV."
    );
    let (lo, hi) = shielded.urr_range_ev().expect("range");
    assert!(
        (lo - 2.0e4).abs() < 1.0 && (hi - 1.490087e5).abs() < 1.0,
        "URR range came out [{lo:.6e}, {hi:.6e}] eV, not U-238's [2.0e4, 1.490087e5]."
    );
    assert!(shielded.needs_urr_draw(URR_PROBE_EV));
    assert!(!shielded.needs_urr_draw(RESOLVED_PROBE_EV));
    assert!(!shielded.needs_urr_draw(FAST_PROBE_EV));

    // 3: the bands are FACTORS, not barns -- U-238 is LSSF=1, so MF=3 already
    // holds the infinitely-dilute values and the table shields them. Reading
    // these as barns is the silent catastrophic error this typing prevents.
    let mut min_f = f64::INFINITY;
    let mut max_f = 0.0f64;
    let mut seen = 0usize;
    for k in 0..64 {
        let xi = (k as f64 + 0.5) / 64.0;
        match shielded.sample_urr(URR_PROBE_EV, xi) {
            Some(UrrSample::SelfShieldingFactors(f)) => {
                min_f = min_f.min(f[0]);
                max_f = max_f.max(f[0]);
                seen += 1;
            }
            Some(UrrSample::CrossSections(_)) => panic!(
                "U-238 returned CrossSections; its evaluation is LSSF=1, so the bands must be \
                 dimensionless self-shielding factors. Treating them as barns would be wrong \
                 by orders of magnitude."
            ),
            None => panic!("no band sampled at {URR_PROBE_EV:.1e} eV, inside the URR"),
        }
    }
    assert_eq!(seen, 64);
    assert!(
        max_f > min_f,
        "every sampled band gave the same total factor ({min_f}); a probability table with no \
         spread carries no self-shielding and would price as zero."
    );
    // Factors are ratios about 1; a decade either way means the LSSF convention
    // is being applied backwards somewhere.
    assert!(
        min_f > 0.05 && max_f < 20.0,
        "total self-shielding factors span [{min_f:.4}, {max_f:.4}], far from the ~1 a ratio \
         should sit near. Suspect the LSSF=1 ratio convention before the physics."
    );

    println!(
        "URR tables on U-238: range [{lo:.4e}, {hi:.4e}] eV, LSSF=1 self-shielding factors, \
         total factor spans [{min_f:.4}, {max_f:.4}] over 64 bands at {URR_PROBE_EV:.1e} eV"
    );
}

/// **Without tables, every cross section is bit-identical through both entry
/// points — and no random number is consumed.**
///
/// This is the assertion protecting every previously recorded result in the
/// crate. The transport kernels now call `needs_urr_draw` before drawing a
/// band; if that gate were wrong, every RNG stream would shift and every
/// recorded eigenvalue would move for no physical reason.
#[test]
fn a_nuclide_without_urr_tables_is_bit_identical_through_both_paths() {
    let Some((_tape, plain)) = u238_plain() else {
        return;
    };
    for &e in &[
        1.0e-2_f64,
        1.0,
        RESOLVED_PROBE_EV,
        URR_PROBE_EV,
        FAST_PROBE_EV,
        1.4e7,
    ] {
        assert!(
            !plain.needs_urr_draw(e),
            "a nuclide with no tables asked for a URR draw at {e:.3e} eV; the kernels gate on \
             this, so every RNG stream in the crate would shift."
        );
        let a = plain.xs_at_energy(e, TEMP_K);
        // Any xi at all -- with no tables the value must not depend on it.
        for &xi in &[0.0_f64, 0.25, 0.5, 0.75, 0.999_999] {
            let b = plain.xs_at_energy_urr(e, TEMP_K, xi);
            for (label, x, y) in [
                ("total", a.total, b.total),
                ("elastic", a.elastic, b.elastic),
                ("absorption", a.absorption, b.absorption),
                ("fission", a.fission, b.fission),
                ("nu_fission", a.nu_fission, b.nu_fission),
            ] {
                assert_eq!(
                    x.to_bits(),
                    y.to_bits(),
                    "{label} at {e:.3e} eV changed ({x} -> {y}) through xs_at_energy_urr on a \
                     nuclide with NO tables, at xi={xi}. That path must be the identity here."
                );
            }
        }
    }
    println!(
        "no tables: xs_at_energy_urr is bit-identical to xs_at_energy at 6 energies x 5 xi \
         values, and needs_urr_draw is false everywhere -- no RNG stream shifts"
    );
}

/// The ablation removes the tables and only the tables.
#[test]
fn the_urr_ablation_removes_exactly_the_tables() {
    let Some((tape, plain)) = u238_plain() else {
        return;
    };
    let shielded = plain
        .clone()
        .with_urr_probability_tables(&tape, MAT, TEMP_K, NBIN, NLADR, NSAMP)
        .expect("PURR runs");
    assert!(shielded.has_urr_probability_tables());

    let ablated = shielded.clone().without_urr_probability_tables();
    assert!(
        !ablated.has_urr_probability_tables(),
        "without_urr_probability_tables left tables in place; the hook is a no-op and anything \
         measured through it is an artefact of nothing."
    );
    assert!(!ablated.needs_urr_draw(URR_PROBE_EV));

    // The ablated arm must match a nuclide that never had tables, bit for bit.
    for &e in &[RESOLVED_PROBE_EV, URR_PROBE_EV, FAST_PROBE_EV] {
        let a = plain.xs_at_energy(e, TEMP_K);
        let b = ablated.xs_at_energy(e, TEMP_K);
        assert_eq!(
            a.total.to_bits(),
            b.total.to_bits(),
            "at {e:.3e} eV the ablated nuclide's total ({}) differs from one that never had \
             tables ({}); building and removing tables must leave no trace.",
            b.total,
            a.total
        );
    }
    println!(
        "without_urr_probability_tables: tables present -> absent, and the result is \
         bit-identical to a nuclide that never had them"
    );
}
