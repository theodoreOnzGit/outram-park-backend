//! # Which coefficient set the plant runs on (gh:#1740)
//!
//! The operator's selector, as MSRE's simulator has one (gh:#1574):
//!
//! - [`CoefficientSet::Cited`], the default: Chen et al. (2009)'s isothermal
//!   coefficient split by Hu et al. (2006)'s fuel share, and
//!   IAEA-TECDOC-1382's VSOP bank worth (`kinetics`, `control_rods`).
//! - [`CoefficientSet::MonteCarloPouredBed`]: the whole-core Monte Carlo
//!   fuel and moderator coefficients and rod-bank worth on the poured DEM
//!   bed, from the committed record of gh:#1739
//!   (`nee_soon/verification_and_validation/htr10_dem_bed_coefficients_2026_11_25/`).
//!   ~~**Pending:** until that record is committed, [`POURED_BED`] is `None`
//!   and the selector shows the entry disabled with the issue.~~ **Filled
//!   2026-11-25** from the committed record: fuel −1.68 ± 0.52 pcm/K,
//!   moderator −16.57 ± 0.81 pcm/K, bank 22.49 ± 0.40 % Δk/k (one pour, one
//!   seed, 600 → 1200 K secants; the bank is about 1.5 times the cited
//!   15.24 %, and the record says why that is not yet explained).
//!
//! A switch goes through `HtgrKinetics::set_temperature_coefficients` and
//! `HtgrPlant::set_bank_worth_dk_k`: the plant keeps its state and its
//! reference temperatures, so a hot plant steps in reactivity at the switch
//! (gh:#1740's test records the size).
//!
//! Research, education and V&V only. NOT VALIDATED.

use uom::si::f64::TemperatureCoefficient;

use super::control_rods;
use super::kinetics::{FeedbackSplit, HtgrKinetics};
use super::HtgrPlant;

/// The poured-bed set `(fuel, moderator, bank worth dk/k)`, from gh:#1739's
/// committed record (`nee_soon/verification_and_validation/
/// htr10_dem_bed_coefficients_2026_11_25/`, its logs): fuel
/// `(1/k_BASE − 1/k_KERNEL)/600 K`, moderator the FUEL-GRAPHITE plus the
/// DUMMIES coefficient, bank `1/k_RODS-IN − 1/k_BASE`, with BASE 0.94516,
/// KERNEL 0.93624, FUEL-GRAPHITE 0.89446, DUMMIES 0.91117, RODS-IN 0.77950.
/// Computed from those five printed `k`, not re-run.
pub const POURED_BED: Option<(f64, f64, f64)> = Some((
    (1.0 / 0.94516 - 1.0 / 0.93624) / 600.0,
    (1.0 / 0.94516 - 1.0 / 0.89446) / 600.0 + (1.0 / 0.94516 - 1.0 / 0.91117) / 600.0,
    1.0 / 0.77950 - 1.0 / 0.94516,
));

/// The issue the poured-bed set comes from.
pub const POURED_BED_ISSUE: u32 = 1739;

/// A coefficient set the operator can choose.
#[cfg_attr(target_arch = "wasm32", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CoefficientSet {
    #[default]
    Cited,
    MonteCarloPouredBed,
}

impl CoefficientSet {
    /// Every set, in selector order.
    pub const ALL: [Self; 2] = [Self::Cited, Self::MonteCarloPouredBed];

    /// Selector label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Cited => "Cited (Chen 2009 / Hu 2006, TECDOC-1382)",
            Self::MonteCarloPouredBed => "Monte Carlo, poured bed (gh:#1739)",
        }
    }

    /// Whether the set exists yet.
    pub fn available(self) -> bool {
        match self {
            Self::Cited => true,
            Self::MonteCarloPouredBed => POURED_BED.is_some(),
        }
    }

    /// The set's `(fuel, moderator)` coefficients \[1/K\] and bank worth
    /// \[dk/k\], for the Monte Carlo tab's comparison; `None` while pending.
    /// The cited set is the plant's own defaults: Chen's isothermal
    /// coefficient split by Hu's fuel share, and TECDOC-1382's bank.
    pub fn values(self) -> Option<(f64, f64, f64)> {
        match self {
            Self::Cited => {
                let f = FeedbackSplit::HU_FUEL_SHARE_OF_ISOTHERMAL;
                let a = HtgrKinetics::HTR10_TEMPERATURE_COEFFICIENT_PER_K;
                Some((f * a, (1.0 - f) * a, control_rods::bank_worth_dk_k()))
            }
            Self::MonteCarloPouredBed => POURED_BED,
        }
    }

    /// Where the set's numbers come from, one line.
    pub fn source(self) -> &'static str {
        match self {
            Self::Cited => "Chen et al. (2009) isothermal coefficient, split by Hu et al. (2006); IAEA-TECDOC-1382 VSOP bank worth",
            Self::MonteCarloPouredBed => {
                "nee_soon whole-core Monte Carlo on the poured DEM bed, 600 to 1200 K secants, ENDF/B-VIII.0 (gh:#1739)"
            }
        }
    }

    /// Send this set to `plant` (the cited one by `None`, the plant's own
    /// defaults). Refused, changing nothing, for a set not available.
    pub fn apply(self, plant: &mut HtgrPlant) -> Result<(), String> {
        match self {
            Self::Cited => {
                plant.kinetics.set_temperature_coefficients(None)?;
                plant.set_bank_worth_dk_k(None)
            }
            Self::MonteCarloPouredBed => {
                let (fuel, moderator, worth) = POURED_BED.ok_or_else(|| format!("the poured-bed set is pending gh:#{POURED_BED_ISSUE}"))?;
                use uom::si::temperature_coefficient::per_kelvin;
                plant.kinetics.set_temperature_coefficients(Some((
                    TemperatureCoefficient::new::<per_kelvin>(fuel),
                    TemperatureCoefficient::new::<per_kelvin>(moderator),
                )))?;
                plant.set_bank_worth_dk_k(Some(worth))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cited set is the default and applying it is the plant's own
    /// defaults; the poured-bed set is refused, changing nothing, while it
    /// is pending.
    #[test]
    fn the_selector_applies_only_what_exists() {
        assert_eq!(CoefficientSet::default(), CoefficientSet::Cited);
        assert!(CoefficientSet::Cited.available());
        let mut plant = HtgrPlant::new();
        let before = plant.kinetics.temperature_coefficients();
        CoefficientSet::Cited.apply(&mut plant).expect("cited");
        assert_eq!(plant.kinetics.temperature_coefficients(), before);
        assert_eq!(CoefficientSet::MonteCarloPouredBed.available(), POURED_BED.is_some());
        if POURED_BED.is_none() {
            let e = CoefficientSet::MonteCarloPouredBed.apply(&mut plant).unwrap_err();
            assert!(e.contains("gh:#1739"), "{e}");
            assert_eq!(plant.kinetics.temperature_coefficients(), before);
        }
        assert_eq!(CoefficientSet::ALL.map(|s| s.label().is_empty()), [false, false]);
        assert_eq!(CoefficientSet::ALL.map(|s| s.source().is_empty()), [false, false]);
    }

    /// The Monte Carlo tab's cited row is exactly what a fresh plant runs on
    /// (bit for bit), and the poured-bed row is the committed constant.
    #[test]
    fn the_tab_shows_the_values_the_plant_uses() {
        use uom::si::temperature_coefficient::per_kelvin;
        let plant = HtgrPlant::new();
        let (f, m) = plant.kinetics.temperature_coefficients();
        let (cf, cm, cw) = CoefficientSet::Cited.values().expect("cited");
        assert_eq!((cf, cm, cw), (f.get::<per_kelvin>(), m.get::<per_kelvin>(), plant.bank_worth_dk_k()));
        assert_eq!(CoefficientSet::MonteCarloPouredBed.values(), POURED_BED);
    }

    /// gh:#1739/#1740: the poured-bed set is the record's (to its printed
    /// precision: fuel −1.68, moderator −16.57 pcm/K, bank 22.485 %), it is
    /// available, and applying it puts exactly those numbers in the plant.
    #[test]
    fn the_poured_bed_set_is_the_records_and_reaches_the_plant() {
        use uom::si::temperature_coefficient::per_kelvin;
        let (f, m, w) = POURED_BED.expect("committed");
        assert!((f * 1e5 + 1.680).abs() < 5e-3, "{f}");
        assert!((m * 1e5 + 16.573).abs() < 5e-3, "{m}");
        assert!((w - 0.22485).abs() < 5e-5, "{w}");
        assert!(CoefficientSet::MonteCarloPouredBed.available());
        let mut plant = HtgrPlant::new();
        CoefficientSet::MonteCarloPouredBed.apply(&mut plant).expect("poured bed");
        let (pf, pm) = plant.kinetics.temperature_coefficients();
        assert_eq!((pf.get::<per_kelvin>(), pm.get::<per_kelvin>(), plant.bank_worth_dk_k()), (f, m, w));
        CoefficientSet::Cited.apply(&mut plant).expect("back to cited");
        assert_eq!(plant.bank_worth_dk_k(), crate::physics::control_rods::bank_worth_dk_k());
    }
}
