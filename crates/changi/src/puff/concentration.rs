// SPDX-License-Identifier: GPL-3.0
//
// Ported from puff (R, MIT) `R/helpers.R` — `gpuff`.
// Upstream: Hammerling-Research-Group/puff @ 5213d58. See ../mod.rs for the
// full provenance block.

//! The Gaussian puff concentration kernel.

use uom::si::f64::{Length, Mass, MassDensity};
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_density::kilogram_per_cubic_meter;

use super::dispersion::pasquill_gifford_sigmas;
use super::stability::StabilityClass;

/// Upstream's `conversion.factor`: `(1e6) * 1.524`, turning kg/m^3 into
/// parts-per-million **of methane**.
///
/// The `1e6` is the ppm scaling; the `1.524` is the molar-volume ratio that
/// converts a methane mass concentration to a volume mixing ratio at upstream's
/// implied reference temperature and pressure (`M_air / M_CH4 / rho_air`
/// ≈ `28.96 / 16.04 / 1.185` ≈ `1.524 m^3/kg`).
///
/// # This factor is species-specific and must not be reused
///
/// It encodes methane's molar mass. Applying it to a radionuclide — CHANGI's
/// actual subject — would be wrong twice over: the molar mass is different, and
/// ppm is the wrong unit for an activity concentration, which belongs in
/// Bq/m^3. Use [`gaussian_puff_concentration`], which returns a mass density,
/// and convert with the species' own factor. [`gaussian_puff_methane_ppm`]
/// exists so the code-to-code comparison against upstream has something to
/// compare, not because ppm is the right output for this crate.
pub const METHANE_PPM_PER_KG_PER_M3: f64 = 1e6 * 1.524;

/// Concentration at a receptor from one Gaussian puff, as a **mass density**.
///
/// Ports the body of `gpuff`, less its methane unit conversion. The puff is a
/// three-dimensional Gaussian of total mass `Q` centred on `(x_p, y_p, H)`,
/// with a mirror image at `(x_p, y_p, -H)` that enforces zero flux through the
/// ground:
///
/// ```text
///   C = Q / ((2 pi)^{3/2} sigma_y^2 sigma_z)
///       * exp(-((x_r - x_p)^2 + (y_r - y_p)^2) / (2 sigma_y^2))
///       * [ exp(-(z_r - H)^2 / (2 sigma_z^2)) + exp(-(z_r + H)^2 / (2 sigma_z^2)) ]
/// ```
///
/// The horizontal spread is **isotropic** — `sigma_y` is used for both `x` and
/// `y`, so the puff is circular in plan rather than elongated along the wind.
/// That is a real modelling choice of upstream's, not an oversight: a puff
/// model represents along-wind spread by the *spacing* of successive puffs, so
/// giving each puff an along-wind `sigma_x` as well would double-count it.
///
/// # Arguments
/// - `mass` — the puff's total mass `Q`.
/// - `class` — stability class. Upstream accepts a vector here and silently
///   uses only its first element; this takes the single class the caller means,
///   which is what [`super::stability::StabilitySet::primary`] supplies.
/// - `puff_x`, `puff_y` — the puff centre's horizontal position.
/// - `source_height` — release height `H` above ground, which is also the
///   height of the reflected image below it.
/// - `receptor` — where the concentration is wanted, `(x, y, z)`.
/// - `travel_distance` — how far the puff has travelled from its source. This
///   drives the dispersion coefficients and is **not** the puff-to-receptor
///   distance.
///
/// # Returns
/// Mass concentration at the receptor. Zero when the puff has not yet moved:
/// [`pasquill_gifford_sigmas`] is undefined at zero distance (upstream's `NA`),
/// and upstream's final `ifelse(is.na(C), 0, C)` turns that into a zero. That
/// zero is a modelling artefact, not physics — a freshly emitted puff has a
/// very high concentration at its own centre, and this model reports none.
///
/// # Note on `U`
/// Upstream's `gpuff` declares a wind-speed parameter `U` and **never
/// references it in the body**. It is not taken here. See
/// `docs/puff-code-to-code.md`, upstream defect 1.
#[must_use]
pub fn gaussian_puff_concentration(
    mass: Mass,
    class: StabilityClass,
    puff_x: Length,
    puff_y: Length,
    source_height: Length,
    receptor: (Length, Length, Length),
    travel_distance: Length,
) -> MassDensity {
    let Some(sigmas) = pasquill_gifford_sigmas(class, travel_distance) else {
        // Upstream: sigma is NA, C is NA, and the trailing ifelse maps it to 0.
        return MassDensity::new::<kilogram_per_cubic_meter>(0.0);
    };

    let sy = sigmas.sigma_y.get::<meter>();
    let sz = sigmas.sigma_z.get::<meter>();
    let q = mass.get::<kilogram>();
    let h = source_height.get::<meter>();
    let (xr, yr, zr) = (
        receptor.0.get::<meter>(),
        receptor.1.get::<meter>(),
        receptor.2.get::<meter>(),
    );
    let dx = xr - puff_x.get::<meter>();
    let dy = yr - puff_y.get::<meter>();

    let two_pi_three_halves = (2.0 * core::f64::consts::PI).powf(1.5);
    let amplitude = q / (two_pi_three_halves * sy * sy * sz);
    let horizontal = (-0.5 * (dx * dx + dy * dy) / (sy * sy)).exp();
    let vertical = (-0.5 * (zr - h) * (zr - h) / (sz * sz)).exp()
        + (-0.5 * (zr + h) * (zr + h) / (sz * sz)).exp();

    let c = amplitude * horizontal * vertical;
    // Upstream's `ifelse(is.na(C), 0, C)` also swallows a NaN arising any other
    // way. Reproduced, so a degenerate sigma cannot propagate a NaN into a sum.
    let c = if c.is_nan() { 0.0 } else { c };
    MassDensity::new::<kilogram_per_cubic_meter>(c)
}

/// [`gaussian_puff_concentration`] expressed as parts-per-million of methane.
///
/// This is upstream's `gpuff` exactly, including its unit conversion. It exists
/// for the code-to-code comparison and for callers actually modelling methane.
/// **For any other species the factor is wrong** — see
/// [`METHANE_PPM_PER_KG_PER_M3`].
///
/// # Why this returns a bare `f64` and not a `uom::Ratio`
///
/// ppm is dimensionless, so `uom`'s `Ratio` is the obvious type — and it
/// silently destroys small values here. `Ratio` stores its magnitude in the
/// **base** unit, so constructing one from a ppm figure divides by `1e6` and
/// reading it back multiplies by `1e6`. Puff concentrations reach far enough
/// below `1e-300` that the divided value lands in the **subnormal** range,
/// where the mantissa is truncated and the multiply back cannot recover it.
///
/// Measured on the fixture, at `class A, Q = 1e-6 kg, travel = 1 m,
/// receptor (-10, -10, 2)`:
///
/// | | value |
/// |---|---|
/// | upstream R | `7.4821302396969955e-307` |
/// | through `Ratio` | `7.48213023968e-307` |
///
/// — five significant digits gone, a `2.3e-12` relative error, with no warning.
/// The physical concentration there is negligible, but the mechanism is not
/// specific to negligible values: it applies to anything whose ppm figure is
/// below about `2.2e-302`, and it would corrupt a sum just as quietly. The
/// dimensioned return is [`gaussian_puff_concentration`], which stores kg/m^3
/// directly and does not round-trip.
#[must_use]
pub fn gaussian_puff_methane_ppm(
    mass: Mass,
    class: StabilityClass,
    puff_x: Length,
    puff_y: Length,
    source_height: Length,
    receptor: (Length, Length, Length),
    travel_distance: Length,
) -> f64 {
    let density = gaussian_puff_concentration(
        mass,
        class,
        puff_x,
        puff_y,
        source_height,
        receptor,
        travel_distance,
    );
    density.get::<kilogram_per_cubic_meter>() * METHANE_PPM_PER_KG_PER_M3
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::length::meter as m;

    fn len(v: f64) -> Length {
        Length::new::<m>(v)
    }

    /// Upstream's own documented example:
    /// `gpuff(Q=1, stab_class="D", x_p=0, y_p=0, x_r_vec=100, y_r_vec=0,
    ///        z_r_vec=2, total_dist=100, H=2, U=5)`
    /// returns `2.702735e-30` ppm, measured by running the R at `5213d58`.
    #[test]
    fn matches_upstreams_documented_example() {
        let ppm = gaussian_puff_methane_ppm(
            Mass::new::<kilogram>(1.0),
            StabilityClass::D,
            len(0.0),
            len(0.0),
            len(2.0),
            (len(100.0), len(0.0), len(2.0)),
            len(100.0),
        );
        assert!(
            (ppm - 2.702735e-30).abs() / 2.702735e-30 < 1e-6,
            "got {ppm:e}"
        );
    }

    /// A puff that has not moved reports zero, by upstream's NA-to-zero path.
    /// Pinned because it is an artefact a reader would otherwise mistake for
    /// physics.
    #[test]
    fn an_unmoved_puff_reports_zero_not_infinity() {
        let c = gaussian_puff_concentration(
            Mass::new::<kilogram>(1.0),
            StabilityClass::D,
            len(0.0),
            len(0.0),
            len(2.0),
            (len(0.0), len(0.0), len(2.0)),
            len(0.0),
        );
        assert_eq!(c.get::<kilogram_per_cubic_meter>(), 0.0);
    }

    /// Ground reflection: at `z = 0` the two image terms coincide, so the
    /// surface concentration is exactly twice the unreflected Gaussian.
    #[test]
    fn ground_reflection_doubles_the_surface_value() {
        let common = |z: f64| {
            gaussian_puff_concentration(
                Mass::new::<kilogram>(1.0),
                StabilityClass::D,
                len(0.0),
                len(0.0),
                len(10.0),
                (len(50.0), len(0.0), len(z)),
                len(500.0),
            )
            .get::<kilogram_per_cubic_meter>()
        };
        let sigmas = pasquill_gifford_sigmas(StabilityClass::D, len(500.0)).unwrap();
        let sz = sigmas.sigma_z.get::<m>();
        let sy = sigmas.sigma_y.get::<m>();
        let unreflected = 1.0 / ((2.0 * core::f64::consts::PI).powf(1.5) * sy * sy * sz)
            * (-0.5 * 2500.0 / (sy * sy)).exp()
            * (-0.5 * 100.0 / (sz * sz)).exp();
        assert!((common(0.0) - 2.0 * unreflected).abs() / (2.0 * unreflected) < 1e-12);
    }

    /// The horizontal profile is isotropic: crosswind and along-wind offsets of
    /// equal magnitude give equal concentration. This is a modelling choice,
    /// so pin it — a future "improvement" adding `sigma_x` would double-count
    /// the along-wind spread the puff spacing already represents.
    #[test]
    fn the_horizontal_profile_is_circular_in_plan() {
        let at = |x: f64, y: f64| {
            gaussian_puff_concentration(
                Mass::new::<kilogram>(1.0),
                StabilityClass::C,
                len(0.0),
                len(0.0),
                len(2.0),
                (len(x), len(y), len(2.0)),
                len(300.0),
            )
            .get::<kilogram_per_cubic_meter>()
        };
        assert!((at(40.0, 0.0) - at(0.0, 40.0)).abs() < 1e-30);
        assert!((at(40.0, 0.0) - at(-40.0, 0.0)).abs() < 1e-30);
    }

    /// Mass scaling is exactly linear — the kernel is a density times `Q`.
    #[test]
    fn concentration_is_linear_in_puff_mass() {
        let at = |q: f64| {
            gaussian_puff_concentration(
                Mass::new::<kilogram>(q),
                StabilityClass::E,
                len(10.0),
                len(-5.0),
                len(3.0),
                (len(100.0), len(20.0), len(2.0)),
                len(250.0),
            )
            .get::<kilogram_per_cubic_meter>()
        };
        assert!((at(2.0) - 2.0 * at(1.0)).abs() / at(1.0) < 1e-12);
    }
}
