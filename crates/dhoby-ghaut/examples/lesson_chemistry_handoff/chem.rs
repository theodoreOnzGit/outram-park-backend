//! The calls behind three Fission products lessons: "Air on hot graphite"
//! (`chem-air.html`), "Steam on graphite and bare kernels"
//! (`chem-steam.html`) and "Into the source term" (`atops-source-term.html`).
//!
//! - `boon_lay::chemistry::graphite_air` (Contescu et al., ORNL/TM-2022/1839,
//!   Table 3, row Contescu 2011), `graphite_steam` (Wang & Sun 2023, NED 410,
//!   112382, Table 8 "Unknown n") and `kernel_hydrolysis` (IAEA-TECDOC-978
//!   Eq. 5-2): three cited rate laws, each a transcription, **not a port** and
//!   not a mechanistic attack model (`crates/boon-lay/src/chemistry/mod.rs`).
//! - The source term is **not recomputed**: it is the dispersion demo's
//!   recorded file, `examples/dispersion_web/recorded.rs`, included here by
//!   `#[path]` so the two cannot drift.
//!
//! Public API only. Shared by `main.rs` and `tests/lesson_chemistry_handoff.rs`
//! through `#[path]`. Research, education and V&V only (`RESPONSIBLE_USE.md`).
#![allow(dead_code)]

use boon_lay::chemistry::{graphite_air, graphite_steam, kernel_hydrolysis};
use uom::si::f64::{Pressure, ThermodynamicTemperature};
use uom::si::frequency::hertz;
use uom::si::pressure::kilopascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_celsius;

#[allow(dead_code)]
#[path = "../dispersion_web/recorded.rs"]
pub mod recorded;

/// A temperature in degrees Celsius.
pub fn temp(c: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(c)
}
/// A pressure in kilopascals.
pub fn kpa(p: f64) -> Pressure {
    Pressure::new::<kilopascal>(p)
}

/// The illustrative graphite mass and O2 supply of boon-lay's own check
/// (`graphite_air::tests::table_3_is_transcribed_and_supply_binds_at_core_temperature`):
/// 5000 kg of graphite, 1 mol/s of O2. Not a plant value.
pub const ILLUSTRATIVE_GRAPHITE_KG: f64 = 5000.0;
/// See [`ILLUSTRATIVE_GRAPHITE_KG`].
pub const ILLUSTRATIVE_O2_SUPPLY_MOL_PER_S: f64 = 1.0;

/// `graphite_air::kinetic_specific_rate` on a temperature grid (degC) at
/// `o2_kpa`: the rate (1/s) and whether it was inside the measured range.
pub fn air_curve(o2_kpa: f64, grid_c: &[f64]) -> Vec<(f64, bool)> {
    grid_c
        .iter()
        .map(|&c| {
            let (r, v) = graphite_air::kinetic_specific_rate(temp(c), kpa(o2_kpa));
            (r.get::<hertz>(), v == graphite_air::Validity::InsideMeasuredRange)
        })
        .collect()
}

/// `graphite_air::gasification_rate` with the illustrative mass and supply:
/// carbon gasified (mol/s) and whether the oxygen supply set it.
pub fn air_gasified(t_c: f64, o2_kpa: f64) -> (f64, bool) {
    let (mol, limit, _) = graphite_air::gasification_rate(
        temp(t_c),
        kpa(o2_kpa),
        ILLUSTRATIVE_GRAPHITE_KG,
        ILLUSTRATIVE_O2_SUPPLY_MOL_PER_S,
    );
    (mol, limit == graphite_air::Limit::OxygenSupply)
}

/// The kinetic carbon rate (mol/s) the illustrative graphite would take if
/// the supply were unlimited: `gasification_rate` called with an infinite
/// supply, so the code's own branch returns the kinetic value.
pub fn air_kinetic_mol(t_c: f64, o2_kpa: f64) -> f64 {
    graphite_air::gasification_rate(temp(t_c), kpa(o2_kpa), ILLUSTRATIVE_GRAPHITE_KG, f64::INFINITY).0
}

/// The temperature (degC) at which `gasification_rate` switches from
/// `Limit::Kinetic` to `Limit::OxygenSupply`, by bisection on the code's own
/// flag between `lo` and `hi`, to 0.01 degC. `None` if the flag does not
/// change across the bracket.
pub fn air_crossover_c(o2_kpa: f64, lo: f64, hi: f64) -> Option<f64> {
    let (mut a, mut b) = (lo, hi);
    if air_gasified(a, o2_kpa).1 || !air_gasified(b, o2_kpa).1 {
        return None;
    }
    while b - a > 0.01 {
        let mid = 0.5 * (a + b);
        if air_gasified(mid, o2_kpa).1 {
            b = mid;
        } else {
            a = mid;
        }
    }
    Some(0.5 * (a + b))
}

/// `graphite_steam::specific_rate` (Wang & Sun Table 8) on a grid (degC) at
/// `steam_kpa` and `h2_kpa`: the rate (1/s) and whether it was inside the
/// measured box.
pub fn steam_curve(steam_kpa: f64, h2_kpa: f64, grid_c: &[f64]) -> Vec<(f64, bool)> {
    let c = graphite_steam::BlhCoefficients::wang_sun_2023_ig110();
    grid_c
        .iter()
        .map(|&t| {
            let (r, v) = graphite_steam::specific_rate(temp(t), kpa(steam_kpa), kpa(h2_kpa), &c);
            (r.get::<hertz>(), v == graphite_steam::Validity::InsideMeasuredRange)
        })
        .collect()
}

/// `kernel_hydrolysis::stored_gas_fraction` on a grid (degC) at `steam_kpa`:
/// the fraction and its flag as a code, 0 inside the fitted range, 1
/// extrapolated, 2 clamped to the whole inventory.
pub fn hydrolysis_curve(steam_kpa: f64, grid_c: &[f64]) -> Vec<(f64, u8)> {
    grid_c
        .iter()
        .map(|&t| {
            let (f, v) = kernel_hydrolysis::stored_gas_fraction(temp(t), kpa(steam_kpa));
            let code = match v {
                kernel_hydrolysis::Validity::InsideFittedRange => 0,
                kernel_hydrolysis::Validity::Extrapolated => 1,
                kernel_hydrolysis::Validity::ClampedToWholeInventory => 2,
            };
            (f.get::<ratio>(), code)
        })
        .collect()
}

/// The steam pressure (kPa) at which `stored_gas_fraction` first reports
/// `ClampedToWholeInventory` at `t_c`, by bisection in log pressure on the
/// code's own flag between 1e-3 and 1e4 kPa. `None` if it is clamped at the
/// bottom of that bracket or not at the top.
pub fn hydrolysis_whole_inventory_kpa(t_c: f64) -> Option<f64> {
    let clamped = |p: f64| hydrolysis_curve(p, &[t_c])[0].1 == 2;
    let (mut a, mut b) = (-3.0f64, 4.0f64);
    if clamped(10f64.powf(a)) || !clamped(10f64.powf(b)) {
        return None;
    }
    while b - a > 1e-6 {
        let mid = 0.5 * (a + b);
        if clamped(10f64.powf(mid)) {
            b = mid;
        } else {
            a = mid;
        }
    }
    Some(10f64.powf(0.5 * (a + b)))
}

/// One recorded nuclide of the source term: name, released over 96 h (Bq),
/// core inventory (Bq), and their ratio.
#[derive(Debug, Clone, PartialEq)]
pub struct Released {
    /// Nuclide name as recorded.
    pub name: &'static str,
    /// Released over the 96 h window, Bq (recorded).
    pub released_bq: f64,
    /// Core inventory, Bq (Liu & Cao 2002 Table 1, recorded).
    pub inventory_bq: f64,
    /// `released_bq / inventory_bq`.
    pub fraction: f64,
}

/// The recorded release (`recorded::RELEASE`), unchanged, with the released
/// fraction of the inventory beside each nuclide.
pub fn released() -> Vec<Released> {
    recorded::RELEASE
        .iter()
        .map(|&(name, released_bq, inventory_bq)| Released { name, released_bq, inventory_bq, fraction: released_bq / inventory_bq })
        .collect()
}
