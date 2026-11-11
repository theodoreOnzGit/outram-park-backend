//! Bit-identity pin of [`SixGroupPRKE`] (gh:#1082).
//!
//! **Why this exists.** gh:#1082 replaces the fixed six-group PRKE core with
//! an N-group one ([`crate::zero_power_prke::n_group_prke`]) so that one
//! implementation remains. The issue requires six groups to stay
//! **bit-identical** with the pre-change `SixGroupPRKE`. These fingerprints
//! were recorded **before** any code was changed (2026-11-01, worktree commit
//! `c9a3afcaf`), by running the scripted transient below against the
//! original hand-unrolled 7×7 implicit solver and RKF45 explicit solver. Any
//! change to the arithmetic (operation order, a fused multiply, a different
//! summation order of `beta_i`) changes at least one bit and fails the pin.
//!
//! **Methodology.** For each of U-233, U-235 and Pu-239 delayed data, start
//! from the crate default state (n = 1 /m³, precursors zero) and advance a
//! fixed script of steps with varying timestep, reactivity (positive,
//! negative, super-prompt), generation time and external source. Every
//! component of the returned 7-vector, at every step, is folded into a
//! 64-bit FNV-1a hash of its `f64::to_bits()`. One hash for the implicit
//! solver, one for the explicit solver, per nuclide. The final neutron
//! density bits are also pinned, so a failure says which number moved.
//!
//! **Results (2026-11-01).** Fingerprints recorded from the original code are
//! the constants in the tests. After the N-group refactor the same
//! constants still pass: six-group results are bit-identical.

use uom::si::f64::*;
use uom::si::ratio::ratio;
use uom::si::time::second;
use uom::si::volumetric_number_rate::per_cubic_meter_second;

use super::six_group_precursor_prke::six_group_constants::FissioningNuclideType;
use super::six_group_precursor_prke::SixGroupPRKE;

/// (timestep s, reactivity, generation time s, source /m³/s)
const SCRIPT: [(f64, f64, f64, f64); 12] = [
    (1.0e-3, 0.0, 1.0e-4, 0.0),
    (1.0e-2, 0.001, 1.0e-4, 0.0),
    (0.1, 0.003, 1.0e-4, 0.0),
    (0.05, 0.0065, 2.0e-5, 0.0),
    (1.0e-4, 0.008, 2.0e-5, 0.0),
    (0.2, -0.01, 1.0e-4, 0.0),
    (1.0, -0.05, 1.0e-4, 10.0),
    (0.5, 0.0, 5.0e-4, 1.0e3),
    (2.0e-3, 0.002, 1.0e-6, 0.0),
    (0.3, -0.002, 3.0e-4, 0.5),
    (5.0, 0.0, 1.0e-4, 0.0),
    (1.0e-2, 0.004, 1.0e-4, 2.0),
];

fn fnv1a(hash: u64, bits: u64) -> u64 {
    let mut h = hash;
    for byte in bits.to_le_bytes() {
        h ^= byte as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Runs the script; returns (hash, final neutron density bits).
pub(crate) fn fingerprint(nuclide: FissioningNuclideType, implicit: bool) -> (u64, u64) {
    let mut prke = SixGroupPRKE::default();
    prke.delayed_group_mode = nuclide;
    // The textbook (pre-gh:#1319) arrays, not the nuclide defaults: this pin
    // is about the PRKE arithmetic, so its input data must not move when a
    // default data set changes. gh:#1319 (2026-11-10) switched U-235's
    // default to ENDF/B-VIII.0; these are the arrays the pin was recorded
    // with, so the fingerprints are unchanged.
    prke.decay_constant_array = nuclide.textbook_decay_constant_array();
    prke.delayed_fraction_array = nuclide.textbook_delayed_fraction_array();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    // repeat the script three times so precursors build up
    for _ in 0..3 {
        for (dt, rho, lam, s) in SCRIPT {
            let dt = Time::new::<second>(dt);
            let rho = Ratio::new::<ratio>(rho);
            let lam = Time::new::<second>(lam);
            let s = VolumetricNumberRate::new::<per_cubic_meter_second>(s);
            let out = if implicit {
                prke.step_implicit(dt, rho, lam, s).unwrap()
            } else {
                prke.step_explicit(dt, rho, lam, s).unwrap()
            };
            for v in out.iter() {
                hash = fnv1a(hash, v.value.to_bits());
            }
        }
    }
    (
        hash,
        prke.get_current_neutron_population_density()
            .value
            .to_bits(),
    )
}

/// Pinned implicit fingerprints (hash, final n bits) for U-233, U-235, Pu-239,
/// recorded from the pre-gh:#1082 implementation.
const IMPLICIT_PIN: [(u64, u64); 3] = PIN_IMPLICIT;
/// Pinned explicit (RKF45) fingerprints, same layout.
const EXPLICIT_PIN: [(u64, u64); 3] = PIN_EXPLICIT;

include!("six_group_bit_identity_pin_values.rs");

const NUCLIDES: [FissioningNuclideType; 3] = [
    FissioningNuclideType::U233,
    FissioningNuclideType::U235,
    FissioningNuclideType::Pu239,
];

/// Prediction (written before the refactor): both hashes unchanged, i.e.
/// zero bits differ, because the N-group core performs the same `uom`
/// operations in the same order.
#[test]
fn six_group_implicit_is_bit_identical_to_pre_1082_pin() {
    for (k, nuclide) in NUCLIDES.iter().enumerate() {
        assert_eq!(fingerprint(*nuclide, true), IMPLICIT_PIN[k], "{nuclide:?}");
    }
}

/// See [`six_group_implicit_is_bit_identical_to_pre_1082_pin`].
#[test]
fn six_group_explicit_is_bit_identical_to_pre_1082_pin() {
    for (k, nuclide) in NUCLIDES.iter().enumerate() {
        assert_eq!(fingerprint(*nuclide, false), EXPLICIT_PIN[k], "{nuclide:?}");
    }
}

/// Prints the fingerprints; used once to record the pin.
#[test]
#[ignore]
fn print_six_group_fingerprints() {
    for implicit in [true, false] {
        for nuclide in NUCLIDES {
            let (h, n) = fingerprint(nuclide, implicit);
            println!("{implicit} {nuclide:?} (0x{h:016x}, 0x{n:016x})");
        }
    }
}
