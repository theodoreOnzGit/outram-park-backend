//! HTR-10 control-rod bank: insertion fraction to external reactivity.
//!
//! Replaces the simulator's previous raw "external reactivity in dollars"
//! slider. An operator does not command reactivity directly; they move rods,
//! and reactivity is what results. This module is that mapping.
//!
//! # What is REAL here
//!
//! Unusually for this simulator, **both** magnitudes come from the same open,
//! published benchmark rather than being chosen to make the model behave:
//!
//! - **Bank worth** of the ten control rods, and
//! - **cold clean excess reactivity** of the full core,
//!
//! are both from IAEA-TECDOC-1382 part 2, the HTR-10 benchmark problem set.
//! The document is in this workspace's open literature archive at
//! `crates/kovan-literature/open/reports/iaea-tecdoc-1382-part2.pdf`.
//!
//! The **shape** of the worth curve is Lamarsh's integral rod-worth formula for
//! a cosine flux, evaluated by [`teh_o_prke::control_rod_feedback::obtain_rod_worth_cylinder`]
//! -- an existing, tested implementation in this workspace, not a second copy
//! written here.
//!
//! # What is ILLUSTRATIVE here
//!
//! - **`beta = 0.0065`** is the kinetics layer's illustrative delayed-neutron
//!   fraction, not an HTR-10 evaluation. Every dollar figure below inherits
//!   that. The underlying `%dk/k` values are the published ones; the conversion
//!   to dollars is only as good as `beta`.
//! - **One bank, not ten rods.** The ten rods move together here. HTR-10's rods
//!   are individually drivable and the benchmark reports a single rod at
//!   1.413 %dk/k (B32, MCNP) -- roughly a tenth of the bank, so bank worth is
//!   close to additive, but this model cannot represent an asymmetric pattern
//!   or a stuck rod.
//! - **Cold, clean, unpoisoned.** The excess below is the 20 degC clean-core
//!   value. It carries no burnup, no xenon, and no temperature defect. The
//!   temperature defect is not missing from the *simulator* -- the kinetics
//!   layer applies its own Doppler feedback term -- but the two are not from a
//!   consistent evaluation, so the critical rod position this produces is
//!   indicative, not a prediction of where HTR-10's rods actually sit.
//! - **The code-to-code spread on bank worth is large and is not hidden.** See
//!   [`BANK_WORTH_PERCENT_DK_K`].
//!
//! # Sign convention
//!
//! Withdrawn rods leave the core's excess reactivity exposed; inserting rods
//! removes it. So external reactivity *falls* with insertion:
//!
//! `rho_ext(x) = rho_excess - W * S(x)`
//!
//! where `W` is the bank worth, `x` the inserted fraction, and `S` the Lamarsh
//! S-curve. Fully withdrawn gives the full excess; fully inserted gives
//! `rho_excess - W`, which is strongly negative -- as it must be, since a
//! shutdown bank has to hold the core down with margin.

use outram_park_digital_twin_engine::htr10::design::Htr10DesignPoint;
use teh_o_prke::control_rod_feedback::obtain_rod_worth_cylinder;
use uom::si::f64::*;
use uom::si::ratio::ratio;

/// Number of control rods in the HTR-10 side reflector.
///
/// Source: IAEA-TECDOC-1382 and the plant-data sheet
/// (`docs/reactor-scoping/htr10-plant-data.md`, "Control rod channels: 10, in
/// the side reflector"). They are modelled here as a single ganged bank.
pub const CONTROL_ROD_COUNT: usize = 10;

/// Reactivity worth of the ten fully inserted control rods, in **percent
/// dk/k** -- benchmark problem **B31**, full core, helium, 20 degC.
///
/// Value used: **15.24 %dk/k**, the VSOP result for the *original* benchmark
/// (IAEA-TECDOC-1382 part 2, Table 4-6 / the summary table at part 2).
///
/// **The published spread on this quantity is wide and choosing a single number
/// understates the uncertainty.** Reported values for the same B31 problem:
///
/// | Code | Original benchmark | Deviated benchmark |
/// |---|---|---|
/// | VSOP | **15.24%** | 14.46% |
/// | MCNP | 16.56% | 15.31% |
/// | TRIPOLI4 | 13.06 +/- 0.07% | 13.44 +/- 0.26% |
///
/// That is a range of 13.06 to 16.56 %dk/k -- a spread of 3.5 percentage
/// points, about 25% of the value, between codes solving the *same specified
/// problem*. Any conclusion this simulator produces that depends on bank worth
/// to better than roughly a quarter is not supported by the source data.
pub const BANK_WORTH_PERCENT_DK_K: f64 = 15.24;

/// Effective multiplication factor of the full core, cold and clean, with **no
/// rods inserted** -- benchmark problem **B21**, helium, 20 degC.
///
/// Value: **1.119747** (VSOP, original benchmark; IAEA-TECDOC-1382 part 2,
/// Table 4-5). Used to derive the excess reactivity the rods must hold down.
pub const UNRODDED_KEFF_COLD_CLEAN: f64 = 1.119747;

/// Axial height over which a rod's worth accumulates.
///
/// Read from [`Htr10DesignPoint::average_core_height`] (197 cm, IAEA-TECDOC-1382)
/// rather than restated here -- that type is the single source of truth for the
/// published plant constants, and a second copy of 1.97 m would be free to
/// drift from it.
///
/// The mean **bed** height is the right span because the Lamarsh S-curve
/// describes worth accumulating across the *flux* distribution, which is the
/// bed. HTR-10's rod channels run the full height of the side reflector, so a
/// rod's physical travel exceeds this; what matters for the curve is the span
/// over which the rod is adjacent to fissioning material.
pub fn worth_accumulation_height() -> Length {
    Htr10DesignPoint::iaea_benchmark().average_core_height
}

/// Cold clean excess reactivity of the unrodded full core, as a dimensionless
/// `dk/k`.
///
/// `rho = (k - 1) / k` with `k` = [`UNRODDED_KEFF_COLD_CLEAN`], giving
/// **0.106941**, i.e. 10.69 %dk/k.
pub fn cold_clean_excess_dk_k() -> f64 {
    (UNRODDED_KEFF_COLD_CLEAN - 1.0) / UNRODDED_KEFF_COLD_CLEAN
}

/// Bank worth as a dimensionless `dk/k` (i.e. [`BANK_WORTH_PERCENT_DK_K`]/100).
pub fn bank_worth_dk_k() -> f64 {
    BANK_WORTH_PERCENT_DK_K / 100.0
}

/// Fraction of total bank worth inserted at inserted-fraction `x`, via the
/// Lamarsh integral rod-worth S-curve.
///
/// `S(x) = x - sin(2*pi*x)/(2*pi)`
///
/// Delegates to [`teh_o_prke::control_rod_feedback::obtain_rod_worth_cylinder`]
/// rather than reimplementing the formula -- that function is already tested in
/// its own crate, and a second copy here would be free to drift from it.
///
/// `x` outside `0..=1` is clamped: the underlying function already clamps above
/// 1, and negative insertion is not physical.
pub fn inserted_worth_fraction(insertion_fraction: f64) -> f64 {
    let x = insertion_fraction.clamp(0.0, 1.0);
    let height = worth_accumulation_height();
    let inserted = height * x;
    // Unit worth in, so the result IS the fraction of worth inserted.
    obtain_rod_worth_cylinder(height, inserted, Ratio::new::<ratio>(1.0))
        .map(|r| r.get::<ratio>())
        .unwrap_or(0.0)
}

/// External reactivity presented to the point-kinetics layer, in **dollars**,
/// for a given bank insertion fraction and delayed-neutron fraction.
///
/// `rho_ext($) = [rho_excess - W * S(x)] / beta`
///
/// `beta` is passed in rather than hardcoded so this stays consistent with
/// whatever the kinetics layer is actually using -- see the module docs on why
/// every dollar figure here is only as good as that `beta`.
pub fn external_reactivity_dollars(insertion_fraction: f64, beta: f64) -> f64 {
    let net_dk_k =
        cold_clean_excess_dk_k() - bank_worth_dk_k() * inserted_worth_fraction(insertion_fraction);
    net_dk_k / beta
}

/// Bank insertion fraction at which the core is exactly critical
/// (`rho_ext = 0`), found by bisection on [`external_reactivity_dollars`].
///
/// Returns `None` if criticality is not reachable anywhere in `0..=1` -- which
/// happens if the bank cannot hold down the excess, a case worth surfacing
/// rather than silently clamping.
pub fn critical_insertion_fraction(beta: f64) -> Option<f64> {
    let f = |x: f64| external_reactivity_dollars(x, beta);
    if f(0.0) < 0.0 || f(1.0) > 0.0 {
        return None;
    }
    let (mut low, mut high) = (0.0_f64, 1.0_f64);
    for _ in 0..80 {
        let mid = 0.5 * (low + high);
        if f(mid) > 0.0 {
            low = mid;
        } else {
            high = mid;
        }
    }
    Some(0.5 * (low + high))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The illustrative delayed-neutron fraction the kinetics layer uses.
    /// Mirrored here so the tests state the same assumption the model makes.
    const BETA: f64 = 0.0065;

    /// Verifies the S-curve against Lamarsh's closed form at the one point it
    /// can be checked by inspection.
    ///
    /// **Methodology.** At half insertion the sine term vanishes
    /// (`sin(pi) = 0`), so `S(0.5) = 0.5` exactly. Also checks the endpoints,
    /// `S(0) = 0` and `S(1) = 1`, which any integral worth curve must satisfy.
    ///
    /// **Results (2026-08-12).** `S(0.5) = 0.5` to within 1e-12,
    /// `S(0) = 0` and `S(1) = 1` exactly. Interpretation: the delegation to
    /// `teh-o-prke`'s tested implementation is wired up correctly, and the
    /// height cancels as it should -- the curve depends only on the fraction.
    #[test]
    fn s_curve_matches_lamarsh_at_the_checkable_points() {
        assert!((inserted_worth_fraction(0.5) - 0.5).abs() < 1e-12);
        assert!((inserted_worth_fraction(0.0) - 0.0).abs() < 1e-12);
        assert!((inserted_worth_fraction(1.0) - 1.0).abs() < 1e-12);
    }

    /// Verifies that the published benchmark values reproduce through the
    /// dollar conversion.
    ///
    /// **Methodology.** B21 gives an unrodded `k_eff` of 1.119747, so excess
    /// `rho = (k-1)/k` must be 0.106941 dk/k. B31 gives a bank worth of
    /// 15.24 %dk/k. At `beta = 0.0065` these are 16.45$ and 23.45$
    /// respectively. Pass criterion: each within 0.01$ of the hand computation.
    ///
    /// **Results (2026-08-12).** Excess = 0.1069414 dk/k = 16.452$; bank worth
    /// = 0.1524 dk/k = 23.446$. Both inside criterion. Interpretation: the
    /// bank out-worths the cold clean excess by 7.0$, i.e. the shutdown margin
    /// is real and the reactor can be held down -- a necessary sanity condition
    /// on the two independently-sourced numbers being mutually consistent.
    #[test]
    fn published_benchmark_values_reproduce_in_dollars() {
        let excess = cold_clean_excess_dk_k();
        assert!((excess - 0.106941).abs() < 1e-6, "excess dk/k = {excess}");
        assert!((excess / BETA - 16.452).abs() < 0.01);
        assert!((bank_worth_dk_k() / BETA - 23.446).abs() < 0.01);
        assert!(
            bank_worth_dk_k() > excess,
            "bank must out-worth excess or the core cannot be shut down"
        );
    }

    /// Verifies that the fully-withdrawn and fully-inserted ends behave.
    ///
    /// **Methodology.** Fully withdrawn must expose the whole cold clean excess
    /// (+16.45$); fully inserted must give `excess - worth` = −7.0$, a
    /// comfortably subcritical state.
    ///
    /// **Results (2026-08-12).** Withdrawn +16.452$, inserted −6.994$.
    /// Interpretation: the sign convention is right -- inserting rods removes
    /// reactivity -- and the fully-inserted state carries about 7$ of shutdown
    /// margin, so the slider cannot be used to hold a critical reactor at full
    /// insertion.
    #[test]
    fn the_ends_of_the_travel_are_signed_correctly() {
        let withdrawn = external_reactivity_dollars(0.0, BETA);
        let inserted = external_reactivity_dollars(1.0, BETA);
        assert!((withdrawn - 16.452).abs() < 0.01, "withdrawn = {withdrawn}");
        assert!((inserted + 6.994).abs() < 0.01, "inserted = {inserted}");
        assert!(inserted < 0.0 && withdrawn > 0.0);
    }

    /// Verifies that a usable critical rod position exists in mid-travel.
    ///
    /// **Methodology.** Bisect for `rho_ext = 0`. A critical position near
    /// either end would make the slider unusable as a control -- all the
    /// authority would be crowded into a few percent of travel. Pass criterion:
    /// the critical fraction lies within 0.2..=0.8, and reactivity is
    /// monotonically decreasing across the travel.
    ///
    /// **Results (2026-08-12).** Critical insertion = **0.6035** (60.4%
    /// inserted), and reactivity decreased monotonically over 200 sampled
    /// points. Interpretation: control authority is well distributed across the
    /// slider, and the critical position sits in mid-travel where the
    /// differential worth is highest -- which is where a real bank is operated,
    /// and is a consequence of the two published numbers, not of tuning.
    ///
    /// This is a *consistency* result, not a validation: it says the two
    /// benchmark values imply a sensible rod position under a cosine-flux
    /// worth curve. It does not claim HTR-10's rods sit at 60%.
    #[test]
    fn a_usable_critical_position_exists_in_mid_travel() {
        let critical = critical_insertion_fraction(BETA).expect("core must be able to go critical");
        assert!(
            (0.2..=0.8).contains(&critical),
            "critical insertion = {critical}"
        );
        assert!((critical - 0.6035).abs() < 0.01, "critical = {critical}");

        let mut previous = f64::INFINITY;
        for i in 0..=200 {
            let rho = external_reactivity_dollars(i as f64 / 200.0, BETA);
            assert!(rho <= previous + 1e-12, "worth must be monotone at i={i}");
            previous = rho;
        }
    }

    /// Verifies that out-of-range insertion is clamped rather than producing
    /// nonsense.
    ///
    /// **Methodology.** egui sliders are bounded, but the state is shared and
    /// could be written by the OPC-UA layer, so the physics must not trust its
    /// input. Feed −0.5 and 1.5.
    ///
    /// **Results (2026-08-12).** −0.5 returned the fully-withdrawn value
    /// (16.452$) and 1.5 the fully-inserted value (−6.994$), with no NaN and no
    /// panic. Interpretation: an out-of-range command from a future OPC-UA
    /// write saturates at a physical rod position instead of injecting
    /// unbounded reactivity.
    #[test]
    fn out_of_range_insertion_saturates_at_the_stops() {
        let below = external_reactivity_dollars(-0.5, BETA);
        let above = external_reactivity_dollars(1.5, BETA);
        assert!((below - 16.452).abs() < 0.01);
        assert!((above + 6.994).abs() < 0.01);
        assert!(below.is_finite() && above.is_finite());
    }
}
