//! # Bed-to-helium conjugate heat transfer — the outermost of the nested scales
//!
//! Particle-to-fluid convective coupling in a packed bed of spheres: the
//! Nusselt number, the heat transfer coefficient it implies, the bed's
//! specific surface area, and the volumetric coefficient that a porous-medium
//! energy equation actually needs.
//!
//! ## Where this sits in the nest
//!
//! Level 3 of three, and the boundary condition for the other two. The
//! coefficient computed here sets the pebble **surface** temperature that
//! [`super::pebble`] takes as its outer boundary condition, which in turn sets
//! the TRISO particle surface temperature in [`super::triso`]. Together with
//! [`super::zbs`] (effective conduction through the bed) it closes the solid
//! side of a pebble-bed thermal model.
//!
//! ## The correlation
//!
//! **Wakao-Funazkri**, for particle-to-fluid heat transfer in a packed bed:
//!
//! ```text
//! Nu = 2 + 1.1 * Pr^(1/3) * Re^0.6
//! ```
//!
//! with `Nu = h d / k_f` and `Re = rho u d / mu` both formed on the **particle
//! (pebble) diameter** `d`, and `u` the **superficial** velocity — the volume
//! flow divided by the *empty* bed cross-section, not the interstitial
//! velocity. Getting that wrong changes `Re` by a factor of `1/eps` (about 2.6
//! for the HTR-10 bed), so [`PackedBedConvection::reynolds_number`] is provided
//! to make the convention explicit rather than assumed.
//!
//! The additive 2 is the exact conduction limit for an isolated sphere in a
//! stagnant infinite medium, so the correlation degenerates correctly as the
//! flow stops — the limit that matters most for a gas-cooled reactor, because
//! it is the loss-of-forced-cooling case.
//!
//! **Validity:** `Re` from about 15 to 8500, the range over which Wakao and
//! co-workers regressed the correlation against packed-bed data. Outside it
//! the expression still evaluates (and still tends to 2 as `Re -> 0`), but the
//! answer is an extrapolation; [`PackedBedConvection::is_within_validity_range`]
//! reports which side of that line a given `Re` falls on rather than silently
//! deciding for the caller.
//!
//! **Citation:** Wakao, N. and Funazkri, T. (1978), *Effect of fluid
//! dispersion coefficients on particle-to-fluid mass transfer coefficients in
//! packed beds*, Chemical Engineering Science 33(10), 1375-1384.
//!
//! *Attribution note, for honesty:* that 1978 paper establishes the
//! mass-transfer (Sherwood) form of this correlation; the identically-shaped
//! heat-transfer (Nusselt) version is commonly attributed to the companion
//! paper, Wakao, Kaguei and Funazkri (1979), Chem. Eng. Sci. 34, 325-336.
//! Neither was consulted at page level in this session — the equation form,
//! its coefficients and its stated `Re` range are transcribed from the
//! secondary literature, and a human should confirm them against the primary
//! source before this module is promoted past Prototype in the V&V pipeline.
//!
//! ## Why this is implemented here and not consumed from TUAS
//!
//! [`tuas_boussinesq_solver`] carries a `WakaoData` Nusselt correlation, and
//! it would be the natural thing to reuse. **It must not be used**: its
//! implementation computes
//!
//! ```text
//! Nu = 2 + 1.1 * Re^0.333 * Pr^0.6        (TUAS -- exponents transposed)
//! ```
//!
//! — the Reynolds and Prandtl exponents are swapped relative to the published
//! correlation. This is not a rounding difference. At `Re = 1000`, `Pr = 0.71`
//! (representative of helium in an HTR-10 bed) the two forms differ by a
//! factor of about 5.8; see
//! `tests::divergence_from_the_tuas_wakao_implementation` for the measured
//! numbers. The defect is tracked in this workspace's issue tracker as
//! **`op-4542`**; TUAS is not modified from here, so this module implements
//! the correct form independently. **Do not "unify" the two until `op-4542`
//! is closed** — unification in the wrong direction would silently import the
//! defect.
//!
//! ## Status
//!
//! **NOT VALIDATED.** Verified against analytic limits and hand evaluations
//! only; no comparison against any packed-bed heat-transfer measurement, and
//! none against HTR-10. AI-assisted draft pending human review per
//! `RESPONSIBLE_USE.md`.
//!
//! **Belongs here:** particle-to-fluid convective closure and bed surface-area
//! geometry. **Does not belong here:** helium property data (that is
//! [`outram_park_fork_coolprop`]'s), bed pressure drop, effective conduction
//! through the bed ([`super::zbs`]), or anything inside a pebble.

use uom::si::f64::{
    DynamicViscosity, HeatTransfer, Length, LinearNumberDensity, MassDensity, Ratio,
    ThermalConductivity, Velocity,
};
use uom::si::length::meter;
use uom::si::linear_number_density::per_meter;
use uom::si::ratio::ratio;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::{Quantity, ISQ, SI};
use uom::typenum::{N1, N3, P1, Z0};

use crate::TampinesError;

/// Specific surface area of a packed bed — heat transfer area per unit **bed**
/// volume, 1/m.
///
/// An alias for `uom`'s [`LinearNumberDensity`], whose dimension (1/length) is
/// exactly right but whose name says nothing useful in this context. Produced
/// by [`PackedBedConvection::specific_surface_area`].
pub type SpecificSurfaceArea = LinearNumberDensity;

/// Volumetric heat transfer coefficient, W/(m^3 K) — the product `h * a_v` of
/// a surface coefficient and a specific surface area.
///
/// `uom` has no named quantity of this dimension (M L^-1 T^-3 Th^-1), so the
/// alias is spelled out here rather than leaking a raw
/// `Quantity<ISQ<...>, SI<f64>, f64>` into a public signature, per the
/// workspace's human-interface rule. This is the coefficient a porous-medium
/// two-temperature energy equation multiplies by `(T_solid - T_fluid)` to get
/// a volumetric power density.
pub type VolumetricHeatTransferCoefficient =
    Quantity<ISQ<N1, P1, N3, Z0, N1, Z0, Z0>, SI<f64>, f64>;

/// Lowest particle Reynolds number of the Wakao correlation's regressed range,
/// 15 (dimensionless).
pub const WAKAO_MIN_REYNOLDS: f64 = 15.0;

/// Highest particle Reynolds number of the Wakao correlation's regressed
/// range, 8500 (dimensionless).
pub const WAKAO_MAX_REYNOLDS: f64 = 8500.0;

/// A packed bed of monosized spheres, described by the two parameters the
/// particle-to-fluid convective closure needs.
///
/// Plain data; the physics lives in
/// [`PackedBedConvection::nusselt_number`] and its derived coefficients.
/// Construct with [`PackedBedConvection::new`] (checked) or
/// [`PackedBedConvection::htr10`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PackedBedConvection {
    /// Pebble (sphere) diameter, metres — the length scale of both the Nusselt
    /// and the Reynolds number, and the scale that sets the bed's surface area
    /// per unit volume. HTR-10: 0.06 m.
    pub pebble_diameter: Length,
    /// Bed porosity (void fraction), dimensionless, strictly between 0 and 1.
    /// Enters only through the specific surface area `a_v = 6(1 - eps)/d`, not
    /// through the Nusselt number itself. HTR-10: 0.39 (filling fraction 0.61,
    /// IAEA-TECDOC-1382 part 2, Chapter 4, Open tier).
    pub porosity: Ratio,
}

impl PackedBedConvection {
    /// Builds a packed-bed convective closure from the pebble diameter
    /// (metres) and the bed porosity (dimensionless, strictly in `(0, 1)`).
    ///
    /// Returns [`TampinesError::InvalidInput`] for a non-positive diameter or
    /// a porosity outside `(0, 1)` — a bed that is all solid or all void is
    /// not a packed bed, and both give a nonsensical surface area.
    pub fn new(pebble_diameter: Length, porosity: Ratio) -> Result<Self, TampinesError> {
        if pebble_diameter.get::<meter>() <= 0.0 {
            return Err(TampinesError::InvalidInput(format!(
                "pebble diameter must be strictly positive, got {} m",
                pebble_diameter.get::<meter>()
            )));
        }

        let void_fraction = porosity.get::<ratio>();
        if !(void_fraction > 0.0 && void_fraction < 1.0) {
            return Err(TampinesError::InvalidInput(format!(
                "bed porosity must lie strictly between 0 and 1, got {void_fraction}"
            )));
        }

        Ok(Self {
            pebble_diameter,
            porosity,
        })
    }

    /// The HTR-10 pebble bed: 6.0 cm pebbles at a porosity of 0.39
    /// (volumetric filling fraction of balls 0.61), both from
    /// IAEA-TECDOC-1382 part 2, Chapter 4, Table 4-17 (Open tier). These are
    /// the same two figures [`super::zbs::ZbsBed::htr10`] uses, so the
    /// conduction and convection sides of the bed describe one geometry.
    pub fn htr10() -> Self {
        Self::new(Length::new::<meter>(0.06), Ratio::new::<ratio>(0.39))
            .expect("the published HTR-10 bed geometry is valid")
    }

    /// Particle Reynolds number, `Re = rho u d / mu`, dimensionless.
    ///
    /// `superficial_velocity` is the **superficial** (empty-tube) velocity:
    /// the volumetric flow divided by the full bed cross-sectional area, *not*
    /// the interstitial velocity between pebbles. The two differ by the
    /// porosity — for the HTR-10 bed, using the interstitial velocity by
    /// mistake would inflate `Re` by 1/0.39 = 2.56 and the Nusselt number by
    /// `2.56^0.6 = 1.73`.
    ///
    /// `density` is the fluid mass density (kg/m^3) and `dynamic_viscosity`
    /// the fluid dynamic viscosity (Pa s), both evaluated at the local fluid
    /// condition — this module holds no property data of its own.
    pub fn reynolds_number(
        &self,
        superficial_velocity: Velocity,
        density: MassDensity,
        dynamic_viscosity: DynamicViscosity,
    ) -> Ratio {
        density * superficial_velocity * self.pebble_diameter / dynamic_viscosity
    }

    /// Whether the given particle Reynolds number lies inside the range
    /// Wakao and co-workers regressed the correlation over,
    /// `[15, 8500]`.
    ///
    /// Outside that range [`PackedBedConvection::nusselt_number`] still
    /// returns a value — the correlation is well behaved and tends to the
    /// exact stagnant-sphere limit of 2 as `Re -> 0` — but the value is an
    /// extrapolation. Callers doing anything safety-relevant should check this
    /// and say so in their own output; nothing is clamped or silently
    /// substituted here.
    pub fn is_within_validity_range(&self, reynolds: Ratio) -> bool {
        (WAKAO_MIN_REYNOLDS..=WAKAO_MAX_REYNOLDS).contains(&reynolds.get::<ratio>())
    }

    /// Particle-to-fluid Nusselt number of the bed, dimensionless:
    ///
    /// `Nu = 2 + 1.1 Pr^(1/3) Re^0.6`
    ///
    /// with both dimensionless groups formed on the pebble diameter (see the
    /// module documentation for the correlation's provenance, its `Re`
    /// validity range, and why it is **not** taken from
    /// [`tuas_boussinesq_solver`]).
    ///
    /// `reynolds` and `prandtl` are dimensionless `uom` [`Ratio`]s and must be
    /// non-negative and finite; `prandtl` must additionally be strictly
    /// positive, since a zero Prandtl number is not a fluid. Returns
    /// [`TampinesError::InvalidInput`] otherwise. No error is raised for a
    /// `Re` outside `[15, 8500]` — use
    /// [`PackedBedConvection::is_within_validity_range`] for that.
    pub fn nusselt_number(&self, reynolds: Ratio, prandtl: Ratio) -> Result<Ratio, TampinesError> {
        let re = reynolds.get::<ratio>();
        let pr = prandtl.get::<ratio>();

        if !(re >= 0.0) || !re.is_finite() {
            return Err(TampinesError::InvalidInput(format!(
                "Reynolds number must be non-negative and finite, got {re}"
            )));
        }
        if !(pr > 0.0) || !pr.is_finite() {
            return Err(TampinesError::InvalidInput(format!(
                "Prandtl number must be strictly positive and finite, got {pr}"
            )));
        }

        Ok(Ratio::new::<ratio>(2.0 + 1.1 * pr.cbrt() * re.powf(0.6)))
    }

    /// Particle-to-fluid heat transfer coefficient, W/(m^2 K):
    /// `h = Nu k_f / d`.
    ///
    /// `fluid_conductivity` is the thermal conductivity of the coolant at the
    /// local condition (helium, about 0.3 W/(m K) at HTR-10 core
    /// temperatures); it must be strictly positive. The Reynolds and Prandtl
    /// arguments are passed straight to
    /// [`PackedBedConvection::nusselt_number`] and carry the same
    /// requirements.
    ///
    /// The area this coefficient refers to is the **pebble surface** area, not
    /// the bed volume; for a porous-medium energy equation use
    /// [`PackedBedConvection::volumetric_heat_transfer_coefficient`] instead.
    pub fn heat_transfer_coefficient(
        &self,
        reynolds: Ratio,
        prandtl: Ratio,
        fluid_conductivity: ThermalConductivity,
    ) -> Result<HeatTransfer, TampinesError> {
        if fluid_conductivity.get::<watt_per_meter_kelvin>() <= 0.0 {
            return Err(TampinesError::InvalidInput(format!(
                "fluid thermal conductivity must be strictly positive, got {} W/(m K)",
                fluid_conductivity.get::<watt_per_meter_kelvin>()
            )));
        }

        let nusselt = self.nusselt_number(reynolds, prandtl)?;

        Ok(nusselt * fluid_conductivity / self.pebble_diameter)
    }

    /// Specific surface area of the bed, `a_v = 6 (1 - eps) / d`, in 1/m —
    /// pebble surface area per unit **bed** volume.
    ///
    /// The expression is exact for monosized spheres and needs no correlation:
    /// a sphere of diameter `d` has surface `pi d^2` and volume
    /// `pi d^3 / 6`, so its area-to-volume ratio is `6/d`; multiplying by the
    /// solid fraction `(1 - eps)` converts area per unit *solid* volume into
    /// area per unit *bed* volume. It therefore holds for any monosized sphere
    /// packing regardless of how the spheres are arranged. HTR-10:
    /// `6 * 0.61 / 0.06 = 61 1/m`.
    pub fn specific_surface_area(&self) -> SpecificSurfaceArea {
        let solid_fraction = 1.0 - self.porosity.get::<ratio>();
        SpecificSurfaceArea::new::<per_meter>(
            6.0 * solid_fraction / self.pebble_diameter.get::<meter>(),
        )
    }

    /// Volumetric heat transfer coefficient of the bed, W/(m^3 K):
    /// `h a_v = Nu k_f / d * 6 (1 - eps) / d`.
    ///
    /// This is the form a porous-medium two-temperature energy equation wants:
    /// multiplied by the local solid-to-fluid temperature difference it gives
    /// the volumetric power exchanged between the pebbles and the helium,
    /// W/m^3. Arguments and their requirements are as for
    /// [`PackedBedConvection::heat_transfer_coefficient`].
    pub fn volumetric_heat_transfer_coefficient(
        &self,
        reynolds: Ratio,
        prandtl: Ratio,
        fluid_conductivity: ThermalConductivity,
    ) -> Result<VolumetricHeatTransferCoefficient, TampinesError> {
        let coefficient = self.heat_transfer_coefficient(reynolds, prandtl, fluid_conductivity)?;

        Ok(coefficient * self.specific_surface_area())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::dynamic_viscosity::pascal_second;
    use uom::si::heat_transfer::watt_per_square_meter_kelvin;
    use uom::si::mass_density::kilogram_per_cubic_meter;
    use uom::si::velocity::meter_per_second;

    /// Asserts that `measured` matches `expected` to within `max_relative`
    /// relative error. See the identical macro in [`super::super::triso`] for
    /// why `tampines` does not use the `approx` crate here.
    macro_rules! assert_relative_eq {
        ($expected:expr, $measured:expr, max_relative = $tolerance:expr) => {{
            let expected: f64 = $expected;
            let measured: f64 = $measured;
            let relative_error = if expected == 0.0 {
                measured.abs()
            } else {
                ((measured - expected) / expected).abs()
            };
            assert!(
                relative_error < $tolerance,
                "expected {expected}, measured {measured}, relative error \
                 {relative_error:e} exceeds {}",
                $tolerance
            );
        }};
    }

    /// V&V test: the Nusselt number tends to the exact stagnant-sphere limit
    /// of 2 as the Reynolds number goes to zero.
    ///
    /// **Methodology:** the steady conduction solution for an isolated sphere
    /// of diameter `d` in a stagnant infinite medium of conductivity `k` gives
    /// `h = 2 k / d`, i.e. `Nu = 2` exactly. Evaluate
    /// [`PackedBedConvection::nusselt_number`] at helium's Prandtl number
    /// (0.71) for `Re` decreasing over `{1, 1e-2, 1e-4, 1e-6, 1e-8, 0}` and
    /// require the sequence to decrease monotonically toward 2, to reach 2
    /// within 1e-3 by `Re = 1e-6`, and to equal 2 *exactly* at `Re = 0`.
    ///
    /// **Results (2026-08-11):** measured Nu = **2.9813233544901983** at
    /// Re = 1 (where the flow term is exactly `1.1 Pr^(1/3)` = 0.98132),
    /// **2.061917317782553** at Re = 1e-2, **2.0039067186405415** at
    /// Re = 1e-4, **2.000246497281907** at Re = 1e-6,
    /// **2.0000155529270414** at Re = 1e-8, and **exactly 2.0** at Re = 0. The
    /// sequence is monotone decreasing throughout, and the residual falls by a
    /// factor of 15.85 = `100^0.6` per two decades of `Re`, as the
    /// correlation's form requires.
    ///
    /// **Interpretation:** the correlation degenerates to the exact
    /// conduction limit, which is the limit that matters for a gas-cooled
    /// reactor — it is the loss-of-forced-cooling case, where the bed must
    /// still reject decay heat with the blowers stopped. A correlation without
    /// this property would be unusable for HTGR safety work. Verification
    /// against an analytic limit; not a validation against data.
    #[test]
    fn nusselt_tends_to_two_in_the_stagnant_limit() {
        let bed = PackedBedConvection::htr10();
        let prandtl = Ratio::new::<ratio>(0.71);

        let mut previous = f64::INFINITY;
        for reynolds_value in [1.0, 1e-2, 1e-4, 1e-6, 1e-8, 0.0] {
            let nusselt = bed
                .nusselt_number(Ratio::new::<ratio>(reynolds_value), prandtl)
                .unwrap()
                .get::<ratio>();
            println!("Re = {reynolds_value:e}: Nu = {nusselt}");
            assert!(
                nusselt < previous,
                "Nusselt number must fall monotonically as Re falls"
            );
            assert!(nusselt >= 2.0, "Nusselt number cannot fall below 2");
            previous = nusselt;
        }

        let stagnant = bed
            .nusselt_number(Ratio::new::<ratio>(0.0), prandtl)
            .unwrap()
            .get::<ratio>();
        assert_eq!(stagnant, 2.0);

        let nearly_stagnant = bed
            .nusselt_number(Ratio::new::<ratio>(1e-6), prandtl)
            .unwrap()
            .get::<ratio>();
        assert!((nearly_stagnant - 2.0).abs() < 1e-3);
    }

    /// V&V test: the Nusselt number and the heat transfer coefficient at a
    /// stated operating point, against hand evaluation.
    ///
    /// **Methodology:** evaluate [`PackedBedConvection::nusselt_number`] and
    /// [`PackedBedConvection::heat_transfer_coefficient`] for the HTR-10 bed
    /// (6.0 cm pebbles) at **Re = 1000, Pr = 0.71**, with a fluid conductivity
    /// of 0.30 W/(m K) — figures representative of helium in an HTR-10 core,
    /// used here as a *stated evaluation point*, not as a claim about any
    /// particular HTR-10 operating state. Compare against
    /// `Nu = 2 + 1.1 * 0.71^(1/3) * 1000^0.6` and `h = Nu k / d` recomputed
    /// inside the test. Pass criterion: 1e-12 relative. Also confirm this `Re`
    /// is reported as inside the correlation's validity range, and that
    /// Re = 10 and Re = 10000 are reported as outside it.
    ///
    /// **Results (2026-08-11):** at Re = 1000, Pr = 0.71 the measured Nusselt
    /// number was **63.91731778255307**, matching the in-test hand evaluation
    /// exactly (relative difference 0), and the heat transfer coefficient
    /// **319.58658891276536 W/(m^2 K)**. The validity-range predicate returned
    /// true at Re = 1000 and false at both Re = 10 and Re = 10000.
    ///
    /// **Interpretation:** an `h` of about 320 W/(m^2 K) on 6 cm pebbles is
    /// the expected order for forced-convection helium in a pebble bed, and
    /// with the bed's 61 1/m surface area gives a volumetric coefficient near
    /// 1.95e4 W/(m^3 K) — a strong solid-fluid coupling, consistent with the
    /// small pebble-to-gas temperature differences pebble-bed designs rely on.
    /// This is an arithmetic check against the transcribed correlation, not a
    /// validation against packed-bed data.
    #[test]
    fn nusselt_and_coefficient_at_a_stated_operating_point() {
        let bed = PackedBedConvection::htr10();
        let reynolds = Ratio::new::<ratio>(1000.0);
        let prandtl = Ratio::new::<ratio>(0.71);
        let fluid_conductivity = ThermalConductivity::new::<watt_per_meter_kelvin>(0.30);

        let hand_nusselt = 2.0 + 1.1 * 0.71_f64.cbrt() * 1000.0_f64.powf(0.6);
        let measured_nusselt = bed
            .nusselt_number(reynolds, prandtl)
            .unwrap()
            .get::<ratio>();
        println!("Re = 1000, Pr = 0.71: Nu measured {measured_nusselt}, hand {hand_nusselt}");
        assert_relative_eq!(hand_nusselt, measured_nusselt, max_relative = 1e-12);

        let hand_coefficient = hand_nusselt * 0.30 / 0.06;
        let measured_coefficient = bed
            .heat_transfer_coefficient(reynolds, prandtl, fluid_conductivity)
            .unwrap()
            .get::<watt_per_square_meter_kelvin>();
        println!("h measured {measured_coefficient} W/(m^2 K), hand {hand_coefficient} W/(m^2 K)");
        assert_relative_eq!(hand_coefficient, measured_coefficient, max_relative = 1e-12);

        assert!(bed.is_within_validity_range(reynolds));
        assert!(!bed.is_within_validity_range(Ratio::new::<ratio>(10.0)));
        assert!(!bed.is_within_validity_range(Ratio::new::<ratio>(10000.0)));
    }

    /// V&V test: the bed specific surface area and the volumetric coefficient
    /// it produces.
    ///
    /// **Methodology:** `a_v = 6 (1 - eps) / d` is exact for monosized
    /// spheres, so it can be checked two ways. First, evaluate
    /// [`PackedBedConvection::specific_surface_area`] for the HTR-10 bed
    /// (eps = 0.39, d = 0.06 m) against the hand value `6 * 0.61 / 0.06`, to
    /// 1e-12 relative. Second, check it against first principles: the total
    /// surface of `N` pebbles in one cubic metre of bed, where `N` follows
    /// from the solid fraction and the pebble volume — an independent route to
    /// the same number. Then confirm
    /// [`PackedBedConvection::volumetric_heat_transfer_coefficient`] equals
    /// `h * a_v` at Re = 1000, Pr = 0.71, k = 0.30 W/(m K).
    ///
    /// **Results (2026-08-11):** measured `a_v` = **61.00000000000001 1/m**
    /// against the hand value 61.00000000000001 1/m (relative difference 0),
    /// and against the first-principles count of **5393.584182558677 pebbles
    /// per m^3 of bed** at 1.1309733552923252e-4 m^3 each — total surface
    /// **61.000000000000014 m^2 per m^3**, agreeing to f64 roundoff. The
    /// volumetric coefficient at the stated operating point measured
    /// **19494.78192367869 W/(m^3 K)**, equal to `h * a_v` =
    /// 319.58658891276536 * 61.00000000000001 to f64 roundoff.
    ///
    /// **Interpretation:** the two independent derivations of `a_v` agreeing
    /// confirms the solid-fraction weighting is applied once, not twice — the
    /// most common error in this expression. Exact geometry; no correlation
    /// and no data are involved, so there is no physical uncertainty to quote.
    #[test]
    fn specific_surface_area_and_volumetric_coefficient() {
        let bed = PackedBedConvection::htr10();

        let hand_area = 6.0 * (1.0 - 0.39) / 0.06;
        let measured_area = bed.specific_surface_area().get::<per_meter>();
        println!("a_v measured {measured_area} 1/m, hand {hand_area} 1/m");
        assert_relative_eq!(hand_area, measured_area, max_relative = 1e-12);

        // independent route: count the pebbles in a cubic metre of bed
        let pebble_radius: f64 = 0.03;
        let pebble_volume = 4.0 / 3.0 * std::f64::consts::PI * pebble_radius.powi(3);
        let pebble_surface = 4.0 * std::f64::consts::PI * pebble_radius.powi(2);
        let pebbles_per_cubic_metre = (1.0 - 0.39) / pebble_volume;
        let surface_per_cubic_metre = pebbles_per_cubic_metre * pebble_surface;
        println!(
            "first principles: {pebbles_per_cubic_metre} pebbles per m^3 at \
             {pebble_volume} m^3 each, total surface {surface_per_cubic_metre} m^2/m^3"
        );
        assert_relative_eq!(surface_per_cubic_metre, measured_area, max_relative = 1e-12);

        // volumetric coefficient equals h * a_v
        let reynolds = Ratio::new::<ratio>(1000.0);
        let prandtl = Ratio::new::<ratio>(0.71);
        let fluid_conductivity = ThermalConductivity::new::<watt_per_meter_kelvin>(0.30);
        let coefficient = bed
            .heat_transfer_coefficient(reynolds, prandtl, fluid_conductivity)
            .unwrap()
            .get::<watt_per_square_meter_kelvin>();
        let volumetric = bed
            .volumetric_heat_transfer_coefficient(reynolds, prandtl, fluid_conductivity)
            .unwrap()
            .value;
        println!(
            "h = {coefficient} W/(m^2 K), a_v = {measured_area} 1/m, h a_v = \
             {volumetric} W/(m^3 K)"
        );
        assert_relative_eq!(
            coefficient * measured_area,
            volumetric,
            max_relative = 1e-12
        );
    }

    /// V&V test (defect documentation): how far this module's Wakao
    /// implementation diverges from TUAS's, which has the exponents
    /// transposed.
    ///
    /// **Methodology:** TUAS's `WakaoData`
    /// (`crates/tuas_boussinesq_solver/src/lib/heat_transfer_correlations/nusselt_number_correlations/input_structs.rs`,
    /// around line 152) computes `Nu = 2 + 1.1 Re^0.333 Pr^0.6`, transposing
    /// the Reynolds and Prandtl exponents of the published
    /// `Nu = 2 + 1.1 Pr^(1/3) Re^0.6`. TUAS is read-only from this crate and
    /// is **not** called here; the transposed expression is recomputed inside
    /// this test as plain arithmetic so the divergence can be measured without
    /// depending on the defective code. Evaluate both forms at
    /// (Re, Pr) = (100, 0.71), (1000, 0.71) and (5000, 0.71) — helium-like
    /// Prandtl numbers spanning the correlation's validity range — and record
    /// the ratio. Pass criterion: the divergence is large (ratio above 2 at
    /// Re = 1000), i.e. this is a physics defect and not a rounding
    /// disagreement, and the two forms coincide only where both `Re` and `Pr`
    /// are 1.
    ///
    /// **Results (2026-08-11):** correct form against TUAS's transposed form:
    /// at Re = 100, **17.55292704134619 vs 6.150951837469186** (ratio 2.85);
    /// at Re = 1000, **63.91731778255307 vs 10.936093297424236** (ratio
    /// **5.84**); at Re = 5000, **164.62755672997943 vs 17.272309119929908**
    /// (ratio 9.53). The divergence grows with Reynolds number
    /// because the transposition puts the small exponent on the large group.
    /// At Re = Pr = 1 both forms return exactly 3.1, confirming the difference
    /// is entirely in the exponents.
    ///
    /// **Interpretation:** a TUAS-based pebble-bed model would under-predict
    /// the particle-to-fluid heat transfer coefficient by a factor of about
    /// 2.9 to 9.5 across the correlation's validity range, which would show up
    /// as a spuriously large
    /// pebble-to-gas temperature difference. This is tracked as **`op-4542`**;
    /// this test exists to keep the measured size of the defect on record
    /// until it is fixed upstream, and to fail loudly if anyone "unifies" the
    /// two implementations in the wrong direction.
    #[test]
    fn divergence_from_the_tuas_wakao_implementation() {
        let bed = PackedBedConvection::htr10();
        let prandtl_value = 0.71;
        let prandtl = Ratio::new::<ratio>(prandtl_value);

        for reynolds_value in [100.0, 1000.0, 5000.0] {
            let correct = bed
                .nusselt_number(Ratio::new::<ratio>(reynolds_value), prandtl)
                .unwrap()
                .get::<ratio>();

            // TUAS's transposed expression, recomputed here rather than called
            let tuas_transposed = 2.0 + 1.1 * reynolds_value.powf(0.333) * prandtl_value.powf(0.6);

            println!(
                "Re = {reynolds_value}, Pr = {prandtl_value}: correct Nu = {correct}, \
                 TUAS transposed Nu = {tuas_transposed}, ratio {:.2}",
                correct / tuas_transposed
            );
            assert!(
                correct > tuas_transposed,
                "the transposed form under-predicts Nu across this range"
            );
        }

        let ratio_at_1000 = bed
            .nusselt_number(Ratio::new::<ratio>(1000.0), prandtl)
            .unwrap()
            .get::<ratio>()
            / (2.0 + 1.1 * 1000.0_f64.powf(0.333) * prandtl_value.powf(0.6));
        assert!(
            ratio_at_1000 > 2.0,
            "the divergence at Re = 1000 must be recorded as large, not marginal"
        );

        // both forms coincide when both groups are 1
        let unity = bed
            .nusselt_number(Ratio::new::<ratio>(1.0), Ratio::new::<ratio>(1.0))
            .unwrap()
            .get::<ratio>();
        println!("Re = Pr = 1: both forms give {unity}");
        assert_relative_eq!(3.1, unity, max_relative = 1e-12);
    }

    /// V&V test: the Reynolds number uses the superficial velocity, and
    /// invalid inputs are rejected.
    ///
    /// **Methodology:** compute [`PackedBedConvection::reynolds_number`] for
    /// the HTR-10 bed at a superficial velocity of 2.0 m/s, a density of
    /// 2.0 kg/m^3 and a viscosity of 4.0e-5 Pa s — helium-like magnitudes at
    /// 3 MPa and core temperature, used as a stated evaluation point — and
    /// compare against `rho u d / mu` by hand, to 1e-12 relative. Then confirm
    /// that mistakenly passing the *interstitial* velocity `u/eps` inflates
    /// `Re` by exactly `1/eps`, quantifying the error that convention mix-up
    /// causes. Finally, require a negative Reynolds number, a zero Prandtl
    /// number, a non-positive conductivity, a zero diameter and a porosity of
    /// 1 all to return [`TampinesError::InvalidInput`].
    ///
    /// **Results (2026-08-11):** measured Re = **5999.999999999999** against
    /// the hand value 5999.999999999999 (relative difference 0). Passing the
    /// interstitial velocity 2.0/0.39 = 5.128 m/s instead gave
    /// Re = **15384.615384615381**, a factor of **2.5641 = 1/0.39** too high,
    /// which inflates the *flow* term of the Nusselt number by
    /// `2.5641^0.6 = 1.7594` — a **76% over-prediction** of the flow
    /// contribution to the heat transfer coefficient. All five invalid inputs
    /// returned `InvalidInput`.
    ///
    /// **Interpretation:** the superficial-velocity convention is worth the
    /// explicit helper: getting it wrong is a silent ~76% error in the flow
    /// term of `h`, not a compile failure, because both velocities have the
    /// same units and `uom` cannot tell them apart.
    #[test]
    fn reynolds_uses_the_superficial_velocity_and_inputs_are_checked() {
        let bed = PackedBedConvection::htr10();
        let velocity = Velocity::new::<meter_per_second>(2.0);
        let density = MassDensity::new::<kilogram_per_cubic_meter>(2.0);
        let viscosity = DynamicViscosity::new::<pascal_second>(4.0e-5);

        let hand_reynolds = 2.0 * 2.0 * 0.06 / 4.0e-5;
        let measured_reynolds = bed
            .reynolds_number(velocity, density, viscosity)
            .get::<ratio>();
        println!("Re measured {measured_reynolds}, hand {hand_reynolds}");
        assert_relative_eq!(hand_reynolds, measured_reynolds, max_relative = 1e-12);

        let interstitial = Velocity::new::<meter_per_second>(2.0 / 0.39);
        let inflated = bed
            .reynolds_number(interstitial, density, viscosity)
            .get::<ratio>();
        let inflation = inflated / measured_reynolds;
        println!(
            "using the interstitial velocity instead: Re = {inflated} \
             ({inflation:.4} times too high, inflating Nu by {:.4})",
            inflation.powf(0.6)
        );
        assert_relative_eq!(1.0 / 0.39, inflation, max_relative = 1e-12);

        // invalid inputs
        let prandtl = Ratio::new::<ratio>(0.71);
        assert!(matches!(
            bed.nusselt_number(Ratio::new::<ratio>(-1.0), prandtl),
            Err(TampinesError::InvalidInput(_))
        ));
        assert!(matches!(
            bed.nusselt_number(Ratio::new::<ratio>(100.0), Ratio::new::<ratio>(0.0)),
            Err(TampinesError::InvalidInput(_))
        ));
        assert!(matches!(
            bed.heat_transfer_coefficient(
                Ratio::new::<ratio>(100.0),
                prandtl,
                ThermalConductivity::new::<watt_per_meter_kelvin>(0.0)
            ),
            Err(TampinesError::InvalidInput(_))
        ));
        assert!(matches!(
            PackedBedConvection::new(Length::new::<meter>(0.0), Ratio::new::<ratio>(0.39)),
            Err(TampinesError::InvalidInput(_))
        ));
        assert!(matches!(
            PackedBedConvection::new(Length::new::<meter>(0.06), Ratio::new::<ratio>(1.0)),
            Err(TampinesError::InvalidInput(_))
        ));
    }
}
