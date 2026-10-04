// SPDX-License-Identifier: GPL-3.0-only
//! The steady Gaussian plume, one table at a time: ground-level centreline
//! dilution factor `chi/Q` against distance for each stability class, and
//! where its peak falls.
//!
//! > **Research, education and V&V only** (`RESPONSIBLE_USE.md`). No release,
//! > no nuclide and no dose: `chi/Q` is per unit release, set by geometry and
//! > weather alone.
//!
//! Written for the dispersion lesson track (gh:#530, rungs 1 and 2) as the
//! "use, then modify" example: change `RELEASE_HEIGHT_M` or `WIND_AT_10M_M_PER_S`
//! and run it again.
//!
//! ```text
//! cargo run --release -p buangkok --example plume_chi_over_q
//! ```
//!
//! # Methodology
//!
//! - **Model.** `buangkok::pydoseia::dispersion`, the code-to-code verified
//!   port of pyDOSEIA's single-plume master equation (Hukkoo and Bapat eq.
//!   2.5) with pyDOSEIA's BARC/AERB sigma fits, through
//!   `dilution_single_plume_no_met` (one value per class A-F).
//! - **Geometry.** Release at 30 m, wind measured at 10 m, receptor on the
//!   ground on the plume centreline. No plume rise, no building wake, no
//!   deposition, no decay (pyDOSEIA's own dose path uses none of them).
//! - **Wind.** `MeanSpeedScaling::PerClass` with the same speed for every
//!   class, so the table is for that wind at 10 m, scaled to the release
//!   height by pyDOSEIA's power law.
//! - **Peak search.** A log-spaced scan of 2000 distances from 50 m to 50 km
//!   (a resolution of 0.3 %), reporting the distance of the largest value.
//!   The scan's spacing is stated so the printed peak position is read with
//!   that resolution, not more.
//! - **The `sigma_z = H / sqrt(2)` rule.** If `sigma_y` were proportional to
//!   `sigma_z`, the ground-level peak would sit where `sigma_z = H / sqrt(2)`.
//!   The example prints `sigma_z` at the scanned peak beside `H / sqrt(2)`; the
//!   two differ because pyDOSEIA's `sigma_y` and `sigma_z` grow at different
//!   rates. That difference is the point, not an error.
//!
//! # Results
//!
//! Run 2026-10-04 on `develop` `d4428668be` plus this file (`--release`, one
//! core). Ground-level centreline `chi/Q` [s/m^3], H = 30 m, 2 m/s at 10 m:
//!
//! | x [m] | A | B | C | D | E | F |
//! |---|---|---|---|---|---|---|
//! | 100 | 4.659e-5 | 1.610e-5 | 4.715e-7 | 1.228e-12 | 4.204e-19 | 2.112e-41 |
//! | 400 | 1.933e-5 | 4.297e-5 | 5.991e-5 | 4.019e-5 | 1.015e-5 | 7.668e-8 |
//! | 1000 | 1.674e-6 | 8.784e-6 | 1.911e-5 | 3.643e-5 | 3.623e-5 | 2.104e-5 |
//! | 5000 | 1.317e-8 | 3.674e-7 | 1.155e-6 | 4.477e-6 | 7.412e-6 | 1.381e-5 |
//! | 10000 | 1.648e-9 | 9.200e-8 | 3.301e-7 | 1.655e-6 | 3.077e-6 | 6.547e-6 |
//!
//! Ground-level peak (log scan, 0.3 % resolution in x):
//!
//! | class | x_peak [m] | chi/Q peak | sigma_z at peak [m] | H/sqrt 2 [m] |
//! |---|---|---|---|---|
//! | A | 164 | 7.010e-5 | 22.4 | 21.2 |
//! | B | 216 | 7.038e-5 | 21.6 | 21.2 |
//! | C | 314 | 6.519e-5 | 21.3 | 21.2 |
//! | D | 571 | 4.988e-5 | 20.4 | 21.2 |
//! | E | 902 | 3.666e-5 | 20.0 | 21.2 |
//! | F | 1568 | 3.070e-5 | 19.3 | 21.2 |
//!
//! Interpretation: the textbook `sigma_z = H / sqrt 2` rule holds to within
//! 6 % (A) to 9 % (F) with pyDOSEIA's fits. For this elevated release the
//! stable classes peak further out **and lower** than the unstable ones; the
//! first draft of this example's reading note said "far downwind and high",
//! which the run contradicted, and it was corrected before commit.

use buangkok::pydoseia::dispersion::{
    dilution_single_plume_no_met, sigma_z, MeanSpeedScaling, PlumeGeometry, Receptor,
    StabilityClass,
};
use uom::si::f64::{Length, Velocity};
use uom::si::length::meter;
use uom::si::velocity::meter_per_second;

/// Release height `H` (effective: there is no plume rise in this model).
const RELEASE_HEIGHT_M: f64 = 30.0;
/// Height at which the wind is measured.
const MEASUREMENT_HEIGHT_M: f64 = 10.0;
/// Wind speed at the measurement height, the same for every class here.
const WIND_AT_10M_M_PER_S: f64 = 2.0;

fn geometry() -> PlumeGeometry {
    PlumeGeometry {
        release_height: Length::new::<meter>(RELEASE_HEIGHT_M),
        measurement_height: Length::new::<meter>(MEASUREMENT_HEIGHT_M),
        receptor: Receptor::GroundLevelCentreline,
    }
}

fn scaling() -> MeanSpeedScaling {
    MeanSpeedScaling::PerClass([Velocity::new::<meter_per_second>(WIND_AT_10M_M_PER_S); 6])
}

fn chi_over_q(x_m: f64) -> [f64; 6] {
    dilution_single_plume_no_met(Length::new::<meter>(x_m), geometry(), scaling())
        .map(|d| d.seconds_per_cubic_meter())
}

fn main() {
    println!("Steady Gaussian plume, ground-level centreline chi/Q [s/m^3]");
    println!(
        "H = {RELEASE_HEIGHT_M} m, wind {WIND_AT_10M_M_PER_S} m/s at {MEASUREMENT_HEIGHT_M} m, \
         pyDOSEIA (BARC/AERB) sigmas, no rise/wake/deposition/decay\n"
    );
    println!(
        "{:>8}  {:>10} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "x [m]", "A", "B", "C", "D", "E", "F"
    );
    for x in [100.0, 200.0, 400.0, 800.0, 1000.0, 2000.0, 5000.0, 10_000.0] {
        let v = chi_over_q(x);
        print!("{x:>8}  ");
        for c in v {
            print!("{c:>10.3e} ");
        }
        println!();
    }

    println!("\nWhere the ground-level peak falls (log scan, 2000 points, 50 m to 50 km):");
    println!(
        "{:>5}  {:>10}  {:>12}  {:>16}  {:>12}",
        "class", "x_peak [m]", "chi/Q peak", "sigma_z(x_peak)", "H / sqrt 2"
    );
    let n = 2000;
    let (lo, hi) = (50.0_f64.ln(), 50_000.0_f64.ln());
    let xs: Vec<f64> = (0..n)
        .map(|i| (lo + (hi - lo) * i as f64 / (n - 1) as f64).exp())
        .collect();
    let table: Vec<[f64; 6]> = xs.iter().map(|&x| chi_over_q(x)).collect();
    for class in StabilityClass::ALL {
        let i = class.index();
        let (k, peak) = table
            .iter()
            .enumerate()
            .map(|(k, v)| (k, v[i]))
            .fold((0, f64::MIN), |a, b| if b.1 > a.1 { b } else { a });
        let szp = sigma_z(class, Length::new::<meter>(xs[k])).get::<meter>();
        println!(
            "{:>5}  {:>10.0}  {:>12.3e}  {:>16.1}  {:>12.1}",
            format!("{class:?}"),
            xs[k],
            peak,
            szp,
            RELEASE_HEIGHT_M / core::f64::consts::SQRT_2
        );
    }
    println!(
        "\nReading notes:\n\
         - chi/Q is per unit release: multiply by Bq released for Bq s/m^3.\n\
         - The stable classes (E, F) peak far downwind, the unstable ones (A, B) close in; for this\n\
           elevated release the stable peaks are also the LOWER ones.\n\
         - sigma_z at the peak is not exactly H/sqrt(2): pyDOSEIA's sigma_y and sigma_z grow at\n\
           different rates, so the textbook rule is a guide, not an identity.\n\
         - Below about 100 m and beyond about 10 km the sigma fits are extrapolated."
    );
}
