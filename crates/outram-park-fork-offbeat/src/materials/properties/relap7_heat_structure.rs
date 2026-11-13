// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Not a port of an OFFBEAT file. Assembles correlations already in this
// crate (and three added for gh:#1070) into the property set the RELAP-7
// Theory Manual (INL/EXT-14-31366, §4.2) describes for its heat structures.
// The correlations are MATPRO's (NUREG/CR-6150).
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! The **RELAP-7 heat-structure property set**: UO2 fuel, Zircaloy cladding
//! and the fuel-rod gap gas, as one named bundle (gh:#1070).
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! # What this is
//!
//! RELAP-7's heat-conduction model (Theory Manual §4.1) needs `k`, `rho` and
//! `Cp` for three materials, and §4.2 says which: MATPRO's correlations, with
//! a constant reference density multiplied by a temperature-dependent `Cp`.
//! [`Relap7HeatStructure`] collects exactly that choice, built from this
//! crate's own correlation enums so that nothing is implemented twice:
//!
//! | Material | Property | Model here | Source |
//! |---|---|---|---|
//! | UO2 | `k` | [`ConductivityModel::MatproFthconUo2`] at `D = 0.95` | MATPRO §2.3; manual Eqs 228–230 |
//! | UO2 | `Cp` | [`HeatCapacityModel::MatproUo2`], O/M = 2 | MATPRO §2.2 |
//! | UO2 | `rho` | [`RELAP7_UO2_REFERENCE_DENSITY`] = 10 980 kg/m³ | manual §4.2.1 |
//! | Zircaloy | `k` | [`ConductivityModel::MatproCthconZircaloy`] | MATPRO §4.4; manual Eq 231, Table 8 |
//! | Zircaloy | `Cp` | [`HeatCapacityModel::MatproZircaloy`], held constant below 300 K | MATPRO Table 4-2 |
//! | Zircaloy | `rho` | [`RELAP7_ZIRCALOY_REFERENCE_DENSITY`] = 6 551 kg/m³ | manual §4.2.2 |
//! | gap gas | `k` | [`GapGasMixture::conductivity_with_fit`] with [`GasConductivityFit::Matpro`] | MATPRO §12.1.1; manual Eqs 232–234 |
//! | gap gas | `rho`, `Cp` | ideal gas, [`GapGasMixture::density`], [`GapGasMixture::specific_heat_capacity`] | manual §4.2.3 |
//!
//! **OFFBEAT's own defaults are untouched.** Nothing in this module changes
//! what any existing variant returns; it only selects.
//!
//! # Ranges
//!
//! The manual holds every property constant outside its stated range. Each
//! accessor here does the same, by the correlation's own clamp
//! ([`ConductivityModel::value`], [`HeatCapacityModel::value`]) or, where the
//! correlation's window is wider than RELAP-7's, by an explicit clamp that
//! the accessor's doc names.
//!
//! # Known departures and open questions
//!
//! - **UO2 porosity slope.** The manual prints `0.00649`; there is reason to
//!   suspect `0.00469`. This set uses the printed value. See
//!   [`ConductivityModel::MatproFthconUo2`] — up to 12 % at 1364 K.
//! - **`T''`** in the UO2 phonon term is undefined in the cited text; `T'' =
//!   T` is assumed.
//! - **Gap-gas mixing rule.** The manual's Eq 233 prints `(M_i − M_j)²`; this
//!   crate uses the standard `(M_i + M_j)²`. See the [`crate::gap::gas`]
//!   module doc.
//! - **Gas constant in the UO2 `Cp` defect term.** The manual prints
//!   `8.3143`; [`HeatCapacityModel::MatproUo2`] uses the CODATA value
//!   `8.314 462…`. The difference is about `1e-4` relative in the defect term
//!   alone (which vanishes below ~1500 K), and is left as is.
//! - **Zircaloy `Cp` at 1093 K** carries the upstream OFFBEAT table step
//!   documented on [`HeatCapacityModel::MatproZircaloy`].
//! - **Density consistency.** RELAP-7 forms `rho*Cp` from the reference
//!   densities with no porosity factor while evaluating UO2 `k` at 95 % TD.
//!   Reproduced, not reconciled.
//!
//! # Units
//!
//! Raw `f64` in strict SI, per the crate's units policy: K, W/(m K),
//! J/(kg K), kg/m³, J/(m³ K), Pa.

use crate::error::Result;
use crate::gap::gas::{GapGasMixture, GapGasSpecies, GasConductivityFit, N_SPECIES};
use crate::materials::properties::conductivity::ConductivityModel;
use crate::materials::properties::density::{
    RELAP7_UO2_REFERENCE_DENSITY, RELAP7_ZIRCALOY_REFERENCE_DENSITY,
};
use crate::materials::properties::heat_capacity::HeatCapacityModel;
use crate::materials::MaterialState;

/// Fraction of theoretical density \[-\] RELAP-7 assumes for the UO2
/// conductivity: `0.95` (manual §4.2.1).
pub const RELAP7_FUEL_DENSITY_FRACTION: f64 = 0.95;

/// RELAP-7 gap-gas mole fractions \[-\]: He 0.1066, Kr 0.1340, Xe 0.7594
/// (manual §4.2.3), indexed by [`GapGasSpecies::index`]. A representative
/// fill-plus-fission-gas mixture.
pub const RELAP7_GAP_GAS_MOLE_FRACTIONS: [f64; N_SPECIES] = {
    let mut x = [0.0; N_SPECIES];
    x[GapGasSpecies::Helium as usize] = 0.1066;
    x[GapGasSpecies::Krypton as usize] = 0.1340;
    x[GapGasSpecies::Xenon as usize] = 0.7594;
    x
};

/// Representative fuel-rod internal pressure \[Pa\] RELAP-7 uses to fix the
/// gap-gas density: 4.1 MPa (manual §4.2.3).
pub const RELAP7_GAP_GAS_PRESSURE: f64 = 4.1e6;

/// Temperature \[K\] at which RELAP-7 evaluates the (then constant) gap-gas
/// density: 300 K (manual §4.2.3).
pub const RELAP7_GAP_GAS_REFERENCE_TEMPERATURE: f64 = 300.0;

/// Temperature window \[K\] over which RELAP-7 tabulates the gap-gas
/// conductivity, `(300, 3000)` (manual §4.2.3). Held constant outside.
pub const RELAP7_GAP_GAS_CONDUCTIVITY_RANGE: (f64, f64) = (300.0, 3000.0);

/// Lower end \[K\] of RELAP-7's Zircaloy `Cp` table range (300–1248 K, manual
/// §4.2.2). [`HeatCapacityModel::MatproZircaloy`] itself extends down to
/// 273 K by linear extrapolation; RELAP-7 instead holds the 300 K value.
pub const RELAP7_ZIRCALOY_CP_T_LOW: f64 = 300.0;

/// The RELAP-7 heat-structure property set: one named bundle of MATPRO
/// correlations for UO2 fuel, Zircaloy cladding and the gap gas.
///
/// See the [module documentation](self) for which correlation fills which
/// slot, and for the open questions. Fields are public so that any one
/// choice can be ablated explicitly (for example, swap
/// `fuel_conductivity` to the transposed porosity slope) while the rest of
/// the set stays as published.
///
/// ```
/// use outram_park_fork_offbeat::materials::properties::relap7_heat_structure::Relap7HeatStructure;
///
/// let set = Relap7HeatStructure::new().unwrap();
/// // UO2 near 1000 K: a few W/(m K).
/// assert!((set.fuel_conductivity(1000.0) - 3.9016).abs() < 1e-3);
/// // Zircaloy above its melting point: the constant 36 W/(m K).
/// assert_eq!(set.cladding_conductivity(2500.0), 36.0);
/// // Gap gas, Xe-rich: about 0.0104 W/(m K) at 300 K.
/// assert!((set.gap_gas_conductivity(300.0).unwrap() - 0.010392).abs() < 1e-5);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Relap7HeatStructure {
    /// UO2 conductivity model; [`ConductivityModel::matpro_fthcon_uo2`].
    pub fuel_conductivity: ConductivityModel,
    /// UO2 specific heat model; [`HeatCapacityModel::MatproUo2`].
    pub fuel_heat_capacity: HeatCapacityModel,
    /// UO2 reference density \[kg/m³\]; [`RELAP7_UO2_REFERENCE_DENSITY`].
    pub fuel_reference_density: f64,
    /// Fraction of theoretical density \[-\] at which the UO2 conductivity is
    /// evaluated; [`RELAP7_FUEL_DENSITY_FRACTION`].
    pub fuel_density_fraction: f64,
    /// Zircaloy conductivity model; [`ConductivityModel::MatproCthconZircaloy`].
    pub cladding_conductivity: ConductivityModel,
    /// Zircaloy specific heat model; [`HeatCapacityModel::MatproZircaloy`].
    pub cladding_heat_capacity: HeatCapacityModel,
    /// Zircaloy reference density \[kg/m³\];
    /// [`RELAP7_ZIRCALOY_REFERENCE_DENSITY`].
    pub cladding_reference_density: f64,
    /// Gap-gas composition; [`RELAP7_GAP_GAS_MOLE_FRACTIONS`]. Its stored
    /// mass is irrelevant to every property here (all intensive).
    pub gap_gas: GapGasMixture,
    /// Which pure-gas conductivity fits the gap gas uses;
    /// [`GasConductivityFit::Matpro`].
    pub gap_gas_conductivity_fit: GasConductivityFit,
    /// Pressure \[Pa\] fixing the gap-gas density; [`RELAP7_GAP_GAS_PRESSURE`].
    pub gap_gas_pressure: f64,
    /// Temperature \[K\] fixing the gap-gas density;
    /// [`RELAP7_GAP_GAS_REFERENCE_TEMPERATURE`].
    pub gap_gas_reference_temperature: f64,
}

impl Relap7HeatStructure {
    /// The set as the RELAP-7 Theory Manual §4.2 describes it.
    ///
    /// # Errors
    ///
    /// Only if the constant gap-gas composition were rejected by
    /// [`GapGasMixture::from_mole_fractions`], which cannot happen for the
    /// published fractions (the unit tests construct it). Returned as a
    /// `Result` rather than unwrapped so that no panic path exists.
    pub fn new() -> Result<Self> {
        Ok(Self {
            fuel_conductivity: ConductivityModel::matpro_fthcon_uo2(),
            fuel_heat_capacity: HeatCapacityModel::MatproUo2,
            fuel_reference_density: RELAP7_UO2_REFERENCE_DENSITY,
            fuel_density_fraction: RELAP7_FUEL_DENSITY_FRACTION,
            cladding_conductivity: ConductivityModel::MatproCthconZircaloy,
            cladding_heat_capacity: HeatCapacityModel::MatproZircaloy,
            cladding_reference_density: RELAP7_ZIRCALOY_REFERENCE_DENSITY,
            gap_gas: GapGasMixture::from_mole_fractions(RELAP7_GAP_GAS_MOLE_FRACTIONS, 0.0)?,
            gap_gas_conductivity_fit: GasConductivityFit::Matpro,
            gap_gas_pressure: RELAP7_GAP_GAS_PRESSURE,
            gap_gas_reference_temperature: RELAP7_GAP_GAS_REFERENCE_TEMPERATURE,
        })
    }

    /// The [`MaterialState`] at which the fuel properties are evaluated:
    /// fresh, stoichiometric UO2 at `t` \[K\] with `porosity = 1 -
    /// fuel_density_fraction`.
    #[must_use]
    pub fn fuel_state(&self, t: f64) -> MaterialState {
        let mut state = MaterialState::fresh(t);
        state.porosity = 1.0 - self.fuel_density_fraction;
        state
    }

    /// UO2 thermal conductivity \[W/(m K)\] at `t` \[K\], clamped into the
    /// conductivity model's window (300–3113 K for the default).
    #[must_use]
    pub fn fuel_conductivity(&self, t: f64) -> f64 {
        self.fuel_conductivity.value(&self.fuel_state(t))
    }

    /// UO2 specific heat \[J/(kg K)\] at `t` \[K\], clamped into the model's
    /// window (300–3113 K for the default).
    #[must_use]
    pub fn fuel_specific_heat(&self, t: f64) -> f64 {
        self.fuel_heat_capacity.value(&self.fuel_state(t))
    }

    /// UO2 volumetric heat capacity `rho*Cp` \[J/(m³ K)\] at `t` \[K\]: the
    /// constant reference density times [`Self::fuel_specific_heat`], as the
    /// manual forms it.
    #[must_use]
    pub fn fuel_volumetric_heat_capacity(&self, t: f64) -> f64 {
        self.fuel_reference_density * self.fuel_specific_heat(t)
    }

    /// Zircaloy thermal conductivity \[W/(m K)\] at `t` \[K\]: the cubic to
    /// 2098 K, 36 W/(m K) above, 300 K value below 300 K.
    #[must_use]
    pub fn cladding_conductivity(&self, t: f64) -> f64 {
        self.cladding_conductivity.value(&MaterialState::fresh(t))
    }

    /// Zircaloy specific heat \[J/(kg K)\] at `t` \[K\]. The temperature is
    /// first raised to [`RELAP7_ZIRCALOY_CP_T_LOW`] (RELAP-7 holds the 300 K
    /// value below its table), then the model's own clamp applies.
    #[must_use]
    pub fn cladding_specific_heat(&self, t: f64) -> f64 {
        self.cladding_heat_capacity
            .value(&MaterialState::fresh(t.max(RELAP7_ZIRCALOY_CP_T_LOW)))
    }

    /// Zircaloy volumetric heat capacity `rho*Cp` \[J/(m³ K)\] at `t` \[K\].
    #[must_use]
    pub fn cladding_volumetric_heat_capacity(&self, t: f64) -> f64 {
        self.cladding_reference_density * self.cladding_specific_heat(t)
    }

    /// Gap-gas mixture conductivity \[W/(m K)\] at `t` \[K\], with `t` clamped
    /// into [`RELAP7_GAP_GAS_CONDUCTIVITY_RANGE`].
    ///
    /// # Errors
    ///
    /// [`crate::error::OffbeatError::Unphysical`] for a non-finite `t`
    /// (a NaN survives the clamp), and
    /// [`crate::error::OffbeatError::NotImplemented`] if
    /// [`Self::gap_gas`] has been changed to hold a species the chosen fit
    /// set has no fit for.
    pub fn gap_gas_conductivity(&self, t: f64) -> Result<f64> {
        let (low, high) = RELAP7_GAP_GAS_CONDUCTIVITY_RANGE;
        self.gap_gas
            .conductivity_with_fit(self.gap_gas_conductivity_fit, t.clamp(low, high))
    }

    /// Gap-gas density \[kg/m³\]: ideal gas at [`Self::gap_gas_pressure`] and
    /// [`Self::gap_gas_reference_temperature`], constant thereafter, as the
    /// manual takes it. 183.05 kg/m³ for the default set (manual: 183.06).
    #[must_use]
    pub fn gap_gas_density(&self) -> f64 {
        self.gap_gas
            .density(self.gap_gas_pressure, self.gap_gas_reference_temperature)
    }

    /// Gap-gas specific heat \[J/(kg K)\]: ideal monatomic gas, `5/2 R_mix`.
    /// 186.65 J/(kg K) for the default set (manual: 186.65).
    #[must_use]
    pub fn gap_gas_specific_heat(&self) -> f64 {
        self.gap_gas.specific_heat_capacity()
    }

    /// Gap-gas volumetric heat capacity `rho*Cp` \[J/(m³ K)\].
    #[must_use]
    pub fn gap_gas_volumetric_heat_capacity(&self) -> f64 {
        self.gap_gas_density() * self.gap_gas_specific_heat()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::materials::properties::conductivity::MATPRO_FTHCON_POROSITY_SLOPE_TRANSPOSED;

    /// **Verification of the assembled set against hand values** (gh:#1070).
    ///
    /// *Methodology.* Every accessor of [`Relap7HeatStructure::new`] is
    /// evaluated and compared with an independent double-precision hand
    /// calculation (Python, 2026-11-01) of the MATPRO forms as reproduced in
    /// RELAP-7 Theory Manual §4.2:
    ///
    /// | Quantity | T \[K\] | Hand value |
    /// |---|---|---|
    /// | UO2 `k` (D = 0.95, slope 0.00649) | 1000 | 3.901590299 W/(m K) |
    /// | UO2 `Cp` = 229.0909 + 7.29 + ~0 | 300 | 236.3809083 J/(kg K) |
    /// | UO2 `Cp` | 2000 | 375.0908075 J/(kg K) |
    /// | UO2 `rho Cp` = 10980 × 236.3809 | 300 | 2.595462e6 J/(m³ K) |
    /// | Zry `k` | 300 / 2500 | 12.68209 / 36 W/(m K) |
    /// | Zry `Cp` = table 281; below 300 K held | 300 / 200 | 281 / 281 J/(kg K) |
    /// | Zry `Cp` = 331 + 44 × 360/450 | 1000 | 366.2 J/(kg K) |
    /// | Zry `rho Cp` = 6551 × 281 | 300 | 1 840 831 J/(m³ K) |
    /// | gas `k` (MATPRO fits, `(M_i+M_j)²`) | 300 / 200 / 3000 / 4000 | 0.0103922505 / same / 0.0709318908 / same |
    /// | gas `rho` (4.1 MPa, 300 K) | — | 183.0532809 kg/m³ (manual 183.06) |
    /// | gas `Cp` = 2.5 R / 0.111365 | — | 186.6487533 J/(kg K) (manual 186.65) |
    ///
    /// Pass criterion: `< 1e-9` relative (round-off; the hand values carry
    /// at least ten significant figures).
    ///
    /// *Result* (2026-11-01, `cargo test --release`): all pass. The
    /// gap-gas `rho` and `Cp` also reproduce the manual's two stated numbers
    /// to its printed precision. Interpretation: the set selects and wires
    /// the intended correlations; this is verification of the assembly, not
    /// validation of RELAP-7's material choices.
    #[test]
    fn relap7_set_matches_hand_values() {
        let set = Relap7HeatStructure::new().unwrap();
        let close = |got: f64, want: f64| ((got - want) / want).abs() < 1e-9;

        assert!(close(set.fuel_conductivity(1000.0), 3.901_590_299_182_574_4));
        assert!(close(set.fuel_specific_heat(300.0), 236.380_908_344_590_8));
        assert!(close(set.fuel_specific_heat(2000.0), 375.090_807_506_470_7));
        assert!(close(set.fuel_volumetric_heat_capacity(300.0), 2_595_462.373_623_607));

        assert!(close(set.cladding_conductivity(300.0), 12.682_09));
        assert_eq!(set.cladding_conductivity(2500.0), 36.0);
        assert!(close(set.cladding_specific_heat(300.0), 281.0));
        assert!(close(set.cladding_specific_heat(200.0), 281.0));
        assert!(close(set.cladding_specific_heat(1000.0), 366.2));
        assert!(close(set.cladding_volumetric_heat_capacity(300.0), 1_840_831.0));

        let k300 = set.gap_gas_conductivity(300.0).unwrap();
        assert!(close(k300, 0.010_392_250_534_062_232), "{k300}");
        assert_eq!(set.gap_gas_conductivity(200.0).unwrap(), k300);
        let k3000 = set.gap_gas_conductivity(3000.0).unwrap();
        assert!(close(k3000, 0.070_931_890_776_786_9), "{k3000}");
        assert_eq!(set.gap_gas_conductivity(4000.0).unwrap(), k3000);
        assert!(set.gap_gas_conductivity(f64::NAN).is_err());

        assert!(close(set.gap_gas_density(), 183.053_280_904_974_86));
        assert!(close(set.gap_gas_specific_heat(), 186.648_753_290_595_15));
        assert!(close(
            set.gap_gas_volumetric_heat_capacity(),
            183.053_280_904_974_86 * 186.648_753_290_595_15
        ));
    }

    /// **Self-consistency check** — the set wires the named constants and
    /// models, and an ablation of one slot leaves the others as published.
    ///
    /// *Result* (2026-11-01): the default fields are the documented ones;
    /// swapping the UO2 porosity slope to the transposed value moves the UO2
    /// conductivity at 1000 K from 3.9016 to 3.5796 W/(m K) (hand values in
    /// `conductivity::tests::matpro_fthcon_uo2_matches_hand_values`) and moves
    /// nothing else.
    #[test]
    fn relap7_set_wires_named_constants_and_ablates_one_slot() {
        let set = Relap7HeatStructure::new().unwrap();
        assert_eq!(set.fuel_reference_density, 10980.0);
        assert_eq!(set.cladding_reference_density, 6551.0);
        assert_eq!(set.fuel_state(600.0).density_fraction(), 0.95);
        assert_eq!(set.gap_gas_conductivity_fit, GasConductivityFit::Matpro);
        let xe = set.gap_gas.mole_fraction(GapGasSpecies::Xenon);
        assert!((xe - 0.7594).abs() < 1e-12, "{xe}");

        let mut ablated = set;
        ablated.fuel_conductivity = ConductivityModel::MatproFthconUo2 {
            porosity_slope: MATPRO_FTHCON_POROSITY_SLOPE_TRANSPOSED,
        };
        let k = ablated.fuel_conductivity(1000.0);
        assert!(((k - 3.579_616_589_703_979) / k).abs() < 1e-9, "{k}");
        assert_eq!(ablated.cladding_conductivity(900.0), set.cladding_conductivity(900.0));
        assert_eq!(ablated.fuel_specific_heat(900.0), set.fuel_specific_heat(900.0));
    }
}
