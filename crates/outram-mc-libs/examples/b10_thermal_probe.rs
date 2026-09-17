//! **B-10 and H-1 thermal cross sections from this crate's own reconstruction,
//! against the 2200 m/s standards and the free-gas/bound limits.**
//!
//! # Why this exists
//!
//! It is the exclusion that forced the LEU-COMP-THERM-008 investigation onto the
//! *water*. Those cases are poisoned with soluble boron, and the measured
//! case-to-case differential implies the dissolved boron absorbs about 10.9 %
//! less than it should. Two things could do that: a wrong B-10 cross section, or
//! a wrong thermal flux in the water the boron is dissolved in.
//!
//! This program rules out the first. B-10's absorption is 3845.9 b at 0.0253 eV
//! against the 3835 b standard — **+0.28 %** — and it is 1/v to 0.04 % up to
//! 1 eV. A 0.28 % error cannot produce a 10.9 % absorption deficit, so the
//! deficit is in the flux, which is set by the H-in-H₂O scattering law
//! (`examples/h2o_kernel_vs_njoy_thermr.rs`, GitHub #188).
//!
//! The H-1 columns are the other half of the same argument: the bound-water
//! cross section must exceed the free-gas one at thermal energies and converge
//! onto it as binding stops mattering. It does — 1.724x at 0.0253 eV falling to
//! 1.054x at 1 eV — which shows the *magnitude* of the water law is sound even
//! though its *shape* is not. A magnitude check alone would have cleared #188.
//!
//! # V&V results
//!
//! Recorded in the gate at the bottom of this file, with the date, the standards
//! cited, and the tolerances. The fast-running regression form of the same
//! comparisons is `tests/thermal_standards.rs`.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example b10_thermal_probe
//! ```
use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;

const T: f64 = 293.6;

fn main() {
    let load = |name: &str, file: &str| {
        let p = reference_endf(file).unwrap_or_else(|| panic!("missing {file}"));
        Nuclide::from_endf_file(&p, name, T, 1.0e-3).expect("reconstruct")
    };
    let b10 = load("B10", "n-005_B_010-ENDF8.0.endf");
    let h1 = load("H1", "n-001_H_001-ENDF8.0-Beta6.endf");
    let o16 = load("O16", "n-008_O_016-ENDF8.0.endf");

    println!(
        "{:>10}  {:>12}  {:>12}  {:>12}  {:>12}",
        "E [eV]", "B10 total", "B10 abs", "B10 elastic", "abs*sqrt(E)"
    );
    for &e in &[1.0e-4_f64, 1.0e-3, 0.0253, 0.1, 1.0, 10.0, 100.0] {
        let x = b10.xs_at_energy(e, T);
        println!(
            "{e:>10.2e}  {:>12.4}  {:>12.4}  {:>12.4}  {:>12.4}",
            x.total,
            x.absorption - x.fission,
            x.elastic,
            (x.absorption - x.fission) * (e / 0.0253).sqrt()
        );
    }
    println!(
        "\n  B-10 absorption at 0.0253 eV = {:.2} b (2200 m/s standard: 3835 b)",
        {
            let x = b10.xs_at_energy(0.0253, T);
            x.absorption - x.fission
        }
    );

    println!(
        "\n{:>10}  {:>12}  {:>12}  {:>12}",
        "E [eV]", "H1 free tot", "H1 abs", "O16 abs"
    );
    for &e in &[0.0253_f64, 0.1, 1.0] {
        let x = h1.xs_at_energy(e, T);
        let o = o16.xs_at_energy(e, T);
        println!(
            "{e:>10.4}  {:>12.4}  {:>12.4}  {:>12.6}",
            x.total,
            x.absorption - x.fission,
            o.absorption - o.fission
        );
    }

    // With the bound law attached, as the transport uses it.
    let sab = ThermalScattering::from_endf_file(
        reference_endf("tsl-HinH2O.endf")
            .expect("tape")
            .to_str()
            .unwrap(),
        1,
        T,
        "c_H_in_H2O",
    )
    .expect("sab");
    let h1b = load("H1", "n-001_H_001-ENDF8.0-Beta6.endf").with_thermal_scattering(sab);
    println!(
        "\n{:>10}  {:>14}  {:>14}",
        "E [eV]", "H1 bound tot", "H1 bound scat"
    );
    for &e in &[0.001_f64, 0.0253, 0.1, 1.0, 4.0, 10.0, 20.0] {
        let x = h1b.xs_at_energy(e, T);
        println!("{e:>10.4}  {:>14.4}  {:>14.4}", x.total, x.elastic);
    }

    vv_gate(&b10, &h1, &h1b);
}

/// V&V gate: the thermal standards, asserted.
///
/// # Methodology and oracles
///
/// Three independent oracles, none of them another code:
///
/// 1. **The 2200 m/s standard.** B-10's absorption cross section at 0.0253 eV is
///    a measured, internationally tabulated constant: **3835 b**. This is an
///    experiment, not an evaluation.
/// 2. **The 1/v law.** B-10(n,α) is a pure 1/v absorber well past 1 eV, so
///    `σ_a·sqrt(E)` must be *constant*. This is a shape claim and it is asserted
///    far more tightly than the magnitude one, because it is a property of the
///    reconstruction's own internal consistency rather than of the evaluation.
/// 3. **The bound/free limit.** Chemical binding raises hydrogen's scattering
///    cross section at thermal energies (a bound proton cannot recoil freely)
///    and stops mattering as the incident energy exceeds the binding, so the
///    ratio must be > 1 at 0.0253 eV and fall monotonically towards 1.
///
/// # Results (2026-09-11, ENDF/B-VIII.0 @ 293.6 K)
///
/// ```text
///   B-10 sigma_a(0.0253 eV)            3845.92 b   vs 3835 b standard   +0.28 %
///   B-10 sigma_a*sqrt(E/0.0253):
///       1e-4 eV                        3846.06 b                        +0.004 %
///       1e-3 eV                        3846.04 b                        +0.003 %
///       0.0253 eV                      3845.92 b                         0 (ref)
///       0.1 eV                         3846.04 b                        +0.003 %
///       1.0 eV                         3844.70 b                        −0.032 %
///       10 eV                          3840.23 b                        −0.148 %
///       100 eV                         3826.54 b                        −0.504 %
///   H-1 bound/free total:
///       0.0253 eV                      52.4747 / 30.4441  =  1.724
///       0.1 eV                         33.0647 / 23.1994  =  1.425
///       1.0 eV                         21.8691 / 20.7490  =  1.054
/// ```
///
/// # Tolerances
///
/// **3 %** on the 2200 m/s standard — loose, because it compares an ENDF/B-VIII.0
/// evaluation against a measured constant and the two are allowed to differ by
/// the evaluation's own uncertainty. **0.2 %** on the 1/v law up to 1 eV — tight,
/// because that is this crate's arithmetic being checked against itself, with no
/// evaluation uncertainty in the way. The gap between the two numbers is the
/// point: a 3 % envelope on the magnitude would not notice a broken 1/v law, and
/// a 0.2 % envelope on the magnitude would fail for reasons that are not this
/// crate's fault.
///
/// # Interpretation
///
/// B-10 is right, to 0.28 %. The 10.9 % absorption deficit inferred from
/// LEU-COMP-THERM-008 is therefore **not** the boron cross section, and the
/// search moves to the thermal flux — which is what
/// `examples/h2o_kernel_vs_njoy_thermr.rs` then found (GitHub #188).
fn vv_gate(b10: &Nuclide, h1_free: &Nuclide, h1_bound: &Nuclide) {
    use outram_mc_libs::vv::{assert_monotone, assert_relative};

    println!("\n=== V&V gate: thermal standards ===");

    /// The 2200 m/s (0.0253 eV) B-10 absorption standard, barns.
    const B10_2200_MS_STANDARD_B: f64 = 3835.0;

    let abs = |n: &Nuclide, e: f64| {
        let x = n.xs_at_energy(e, T);
        x.absorption - x.fission
    };

    let sigma_2200 = abs(b10, 0.0253);
    assert_relative(
        "B-10 absorption at 0.0253 eV vs the 2200 m/s standard",
        sigma_2200,
        B10_2200_MS_STANDARD_B,
        0.03,
    );

    // The 1/v law, checked against this crate's own 0.0253 eV value so that the
    // evaluation's offset from the standard cancels and only the SHAPE is tested.
    for &e in &[1.0e-4_f64, 1.0e-3, 0.1, 1.0] {
        let scaled = abs(b10, e) * (e / 0.0253).sqrt();
        assert_relative(
            &format!("B-10 is exactly 1/v at {e:.0e} eV"),
            scaled,
            sigma_2200,
            0.002,
        );
    }
    // Above 1 eV the resonance structure begins to pull it away; 100 eV is
    // −0.50 %, which is physics rather than a defect. Recorded, not asserted
    // tightly.
    let scaled_100 = abs(b10, 100.0) * (100.0f64 / 0.0253).sqrt();
    assert_relative(
        "B-10 remains near-1/v at 100 eV (resonance structure begins here)",
        scaled_100,
        sigma_2200,
        0.01,
    );

    // Binding raises hydrogen's thermal scattering and stops mattering with energy.
    let ratios: Vec<f64> = [0.0253_f64, 0.1, 1.0]
        .iter()
        .map(|&e| h1_bound.xs_at_energy(e, T).total / h1_free.xs_at_energy(e, T).total)
        .collect();
    println!(
        "  H-1 bound/free total: {:.3} (0.0253 eV), {:.3} (0.1 eV), {:.3} (1 eV)",
        ratios[0], ratios[1], ratios[2]
    );
    assert!(
        ratios[0] > 1.5,
        "bound H-in-H2O scattering is only {:.3}x the free-gas value at 0.0253 eV. \
         A bound proton cannot recoil freely, so the ratio must be well above 1 \
         (recorded 1.724); this close to 1 means the bound law is not being \
         applied at all.",
        ratios[0]
    );
    assert_monotone(
        "H-1 bound/free ratio falls towards 1 as binding stops mattering",
        &ratios,
        false,
        0.0,
    );
    assert!(
        (ratios[2] - 1.0).abs() < 0.15,
        "at 1 eV the bound/free ratio is {:.3}; binding should be nearly spent by \
         there (recorded 1.054). A ratio still far from 1 means the law's upper \
         limit or its normalisation has moved.",
        ratios[2]
    );
}
