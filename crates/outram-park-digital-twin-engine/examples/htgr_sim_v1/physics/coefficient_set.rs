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
//!   **Pending:** until that record is committed, [`POURED_BED`] is `None`
//!   and the selector shows the entry disabled with the issue. No number
//!   stands in for it.
//!
//! A switch goes through `HtgrKinetics::set_temperature_coefficients` and
//! `HtgrPlant::set_bank_worth_dk_k`: the plant keeps its state and its
//! reference temperatures, so a hot plant steps in reactivity at the switch
//! (gh:#1740's test records the size).
//!
//! Research, education and V&V only. NOT VALIDATED.

use uom::si::f64::TemperatureCoefficient;

use super::HtgrPlant;

/// The poured-bed set `(fuel, moderator, bank worth dk/k)`, from gh:#1739's
/// committed record; `None` until it exists.
pub const POURED_BED: Option<(f64, f64, f64)> = None;

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
    }
}
