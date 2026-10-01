//! The **steady Gaussian plume** `chi/Q` field the Map tab can overlay on the
//! live Gaussian-puff field (GitHub issue #470, re-scoped by the maintainer
//! 2026-10-01: *"don't worry about looking bad yet, just overlay, i will
//! inspect directly."*).
//!
//! Research and education only; not a dose to any real person and not for
//! emergency, licensing or operational use (`RESPONSIBLE_USE.md`).
//!
//! # Model: buangkok's pyDOSEIA port, as the audited example uses it
//!
//! This is the plume `crates/sembawang/examples/htr10_air_ingress_human_audited_example.rs`
//! uses for its 400 m dose: `buangkok::pydoseia::dispersion`'s single-plume
//! master equation (Hukkoo-Bapat eq. 2.5) with buangkok's BARC/AERB
//! power-law sigmas, a **ground-level release** (`H = 0`), a 10 m wind
//! measurement height (pyDOSEIA raises a release below 10 m to 10 m in its
//! height correction, so the correction factor is exactly 1 and the wind is
//! the speed given), and a ground-level receptor (`z = 0`) with total
//! reflection. **No plume formula is written here**: every cell is
//! [`sigma_y`], [`sigma_z`], [`height_correction_factor`] and
//! [`master_equation_single_plume`] called at that cell's downwind and
//! crosswind distance -- the same calls `dilution_single_plume_no_met` makes,
//! for one class instead of six (so a dragged wind slider does not pay for
//! five unused classes). `tests::centreline_at_400_m_is_buangkoks_direct_call`
//! pins it to that function.
//!
//! # What differs from the live puff underneath -- read before comparing
//!
//! - **Steady vs instantaneous.** The plume is the settled field of a
//!   constant release in a constant wind; the puff field is the marched
//!   population at this instant, carrying its history.
//! - **Release height.** The plume is a ground release, as the audited
//!   example (#436: no stack credit); the puff is released at the 40 m stack
//!   (`Htr10SiteInputs::RELEASE_HEIGHT_M`). Near the stack the two differ by
//!   orders of magnitude for that reason alone.
//! - **Sigma fits.** The class letter is the puff's, but the sigmas are
//!   buangkok's BARC/AERB power laws, not changi's Pasquill-Gifford fits
//!   (buangkok's own module doc: "the two are not interchangeable").
//!
//! # Evaluated per cell, not interpolated
//!
//! The grid is changi's `FieldGrid` (same cells, same half-width, same
//! north-up row-major layout as the puff field), and every cell centre gets
//! its own evaluation. Cells at or upwind of the stack (`x <= 0`) are zero:
//! the Gaussian plume has no upwind branch.

use buangkok::pydoseia::dispersion::{
    height_correction_factor, master_equation_single_plume, sigma_y, sigma_z, Receptor,
    StabilityClass,
};
use uom::si::f64::Length;
use uom::si::length::meter;

/// Release height \[m\]: ground level, as the audited example (#436).
pub const RELEASE_HEIGHT_M: f64 = 0.0;
/// Wind measurement height \[m\], as the audited example: with a release
/// below 10 m pyDOSEIA's height correction is `(10/10)^p = 1`.
pub const MEASUREMENT_HEIGHT_M: f64 = 10.0;

/// The live puff's class letter (`HtgrSnapshot::stability_class`) as
/// buangkok's class. `None` for an empty or unknown letter.
pub fn class_from_letter(letter: &str) -> Option<StabilityClass> {
    match letter.trim() {
        "A" | "a" => Some(StabilityClass::A),
        "B" | "b" => Some(StabilityClass::B),
        "C" | "c" => Some(StabilityClass::C),
        "D" | "d" => Some(StabilityClass::D),
        "E" | "e" => Some(StabilityClass::E),
        "F" | "f" => Some(StabilityClass::F),
        _ => None,
    }
}

/// Single-plume `chi/Q` \[s/m^3\] at downwind distance `x_m` and crosswind
/// offset `y_m`, ground receptor, for a wind of `speed_m_per_s` at the
/// measurement height. Zero for `x_m <= 0` (upwind / at the stack) or a
/// non-positive speed.
pub fn chi_over_q_at(class: StabilityClass, speed_m_per_s: f64, x_m: f64, y_m: f64) -> f64 {
    if !(x_m > 0.0) || !(speed_m_per_s > 0.0) {
        return 0.0;
    }
    let h = Length::new::<meter>(RELEASE_HEIGHT_M);
    let x = Length::new::<meter>(x_m);
    let factor = height_correction_factor(class, h, Length::new::<meter>(MEASUREMENT_HEIGHT_M));
    let terms = master_equation_single_plume(
        sigma_y(class, x),
        sigma_z(class, x),
        factor,
        h,
        Receptor::Offset {
            y: Length::new::<meter>(y_m),
            z: Length::new::<meter>(0.0),
        },
    );
    // `dilution_single_plume_no_met` with `MeanSpeedScaling::PerClass`:
    // `(pre * expo * 1 * 3600) / 3600`, then divided by the class's speed.
    terms.pre_expo * terms.expo / speed_m_per_s
}

/// The steady-plume `chi/Q` field \[s/m^3\] on the puff field's grid:
/// `cells x cells`, row-major, north-up, spanning `+/- half_width_m` about the
/// stack, for a wind blowing **from** `wind_from_deg` (meteorological, clockwise
/// from north -- the same field the puff reads) at `speed_m_per_s`.
///
/// Cost: measured 6.3 ms for a 512 x 512 grid (release build, one thread,
/// 2026-10-01), so the Map tab can re-evaluate it on a wind change within a
/// frame; it caches it otherwise.
pub fn chi_over_q_field(
    cells: usize,
    half_width_m: f64,
    wind_from_deg: f64,
    speed_m_per_s: f64,
    class: StabilityClass,
) -> Vec<f64> {
    let grid = changi::puff::wgsl::FieldGrid {
        cells,
        half_width_m: half_width_m as f32,
        source_height_m: RELEASE_HEIGHT_M as f32,
    };
    // Unit vector the plume TRAVELS along, (east, north).
    let travel = (wind_from_deg + 180.0).to_radians();
    let (tx, ty) = (travel.sin(), travel.cos());
    let mut out = Vec::with_capacity(cells * cells);
    for row in 0..cells {
        for column in 0..cells {
            let (e, n) = grid.cell_centre(column, row);
            let (e, n) = (e as f64, n as f64);
            let x = e * tx + n * ty;
            let y = -e * ty + n * tx;
            out.push(chi_over_q_at(class, speed_m_per_s, x, y));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use buangkok::pydoseia::dispersion::{
        dilution_single_plume_no_met, MeanSpeedScaling, PlumeGeometry,
    };
    use uom::si::f64::Velocity;
    use uom::si::velocity::meter_per_second;

    fn direct(class: StabilityClass, speed: f64, x: f64, y: f64) -> f64 {
        let geometry = PlumeGeometry {
            release_height: Length::new::<meter>(RELEASE_HEIGHT_M),
            measurement_height: Length::new::<meter>(MEASUREMENT_HEIGHT_M),
            receptor: if y == 0.0 {
                Receptor::GroundLevelCentreline
            } else {
                Receptor::Offset {
                    y: Length::new::<meter>(y),
                    z: Length::new::<meter>(0.0),
                }
            },
        };
        let v = Velocity::new::<meter_per_second>(speed);
        dilution_single_plume_no_met(
            Length::new::<meter>(x),
            geometry,
            MeanSpeedScaling::PerClass([v; 6]),
        )[class.index()]
        .seconds_per_cubic_meter()
    }

    /// **Methodology.** The overlay's per-cell evaluation at 400 m downwind on
    /// the centreline, and 60 m off it, against
    /// `buangkok::pydoseia::dispersion::dilution_single_plume_no_met` called
    /// directly with the audited example's geometry (ground release, 10 m
    /// measurement height, ground receptor), every class, at 1 m/s (the
    /// audited example's speed) and 2.5 m/s. Pass: relative difference
    /// <= 1e-12 (the only difference is `x * 3600 / 3600` and dividing by the
    /// speed after vs before, i.e. rounding).
    ///
    /// **Result (2026-10-01).** Passes for every class and both speeds; at
    /// 1 m/s the class-F centreline value is the audited example's largest
    /// (worst-class) chi/Q at 400 m. Verification of the call path only, not
    /// validation of the plume.
    #[test]
    fn centreline_at_400_m_is_buangkoks_direct_call() {
        for class in StabilityClass::ALL {
            for speed in [1.0, 2.5] {
                for y in [0.0, 60.0] {
                    let ours = chi_over_q_at(class, speed, 400.0, y);
                    let theirs = direct(class, speed, 400.0, y);
                    assert!(theirs > 0.0);
                    let rel = (ours - theirs).abs() / theirs;
                    assert!(
                        rel <= 1e-12,
                        "{class:?} u={speed} y={y}: ours {ours:e} vs buangkok {theirs:e}"
                    );
                }
            }
        }
    }

    /// The field's cell nearest 400 m downwind is that cell's own direct
    /// evaluation (the field is per cell, not a resampled curve), for a wind
    /// from the west (plume travels east along a grid row).
    #[test]
    fn field_cell_is_the_direct_evaluation() {
        let cells = 100; // 25 m cells over +/-1250 m
        let field = chi_over_q_field(cells, 1250.0, 270.0, 1.0, StabilityClass::D);
        let grid = changi::puff::wgsl::FieldGrid {
            cells,
            half_width_m: 1250.0,
            source_height_m: 0.0,
        };
        // Column whose centre is 412.5 m east; row whose centre is 12.5 m north.
        let (column, row) = (66, 49);
        let (e, n) = grid.cell_centre(column, row);
        let want = direct(StabilityClass::D, 1.0, e as f64, n as f64);
        let got = field[row * cells + column];
        assert!((got - want).abs() / want <= 1e-6, "{got:e} vs {want:e}");
    }

    /// Symmetric about the wind axis and zero upwind, for a wind from the
    /// north (plume travels south, down the columns) on an even grid, whose
    /// cell centres mirror about the north-south axis. Tolerance 1e-9 relative:
    /// changi's `FieldGrid::cell_centre` is `f32`, so mirrored centres agree
    /// only to f32 rounding, which the far tail's `exp(-y^2/2 sigma_y^2)`
    /// amplifies (measured 1.7e-12 at a 1e-252 cell).
    #[test]
    fn field_is_symmetric_about_the_wind_axis_and_zero_upwind() {
        let cells = 64;
        let field = chi_over_q_field(cells, 1250.0, 0.0, 1.0, StabilityClass::B);
        let mut downwind_positive = 0;
        for row in 0..cells {
            for column in 0..cells {
                let v = field[row * cells + column];
                let mirror = field[row * cells + (cells - 1 - column)];
                assert!(
                    (v - mirror).abs() <= 1e-9 * v.abs().max(mirror.abs()),
                    "asymmetric at ({column},{row}): {v:e} vs {mirror:e}"
                );
                if row < cells / 2 {
                    // Northern half: upwind of a north wind.
                    assert_eq!(v, 0.0, "upwind cell ({column},{row}) = {v:e}");
                } else if v > 0.0 {
                    downwind_positive += 1;
                }
            }
        }
        assert!(downwind_positive > cells * cells / 8);
        // The plume points south: the centreline below the stack carries more
        // than a cell well off-axis at the same row.
        let row = cells - 10;
        assert!(field[row * cells + cells / 2] > field[row * cells + 2]);
    }

    /// Pointing: a wind from the east puts the plume in the west half only.
    #[test]
    fn wind_from_the_east_puts_the_plume_west() {
        let cells = 32;
        let field = chi_over_q_field(cells, 1250.0, 90.0, 1.0, StabilityClass::D);
        for row in 0..cells {
            for column in cells / 2..cells {
                assert_eq!(field[row * cells + column], 0.0);
            }
        }
        assert!(field[(cells / 2) * cells + 2] > 0.0);
    }

    #[test]
    fn class_letters_parse() {
        assert_eq!(class_from_letter("B"), Some(StabilityClass::B));
        assert_eq!(class_from_letter(""), None);
    }
}
