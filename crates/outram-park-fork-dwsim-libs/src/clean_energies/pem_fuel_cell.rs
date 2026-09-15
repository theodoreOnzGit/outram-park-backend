//! PEM fuel cell: three static polarization models plus the stack
//! stoichiometry and stream bookkeeping.
//!
//! # Attribution
//!
//! - **Upstream project:** DWSIM — Open Source Process Simulator
//! - **Source files:**
//!   `DWSIM.UnitOperations/UnitOperations/CleanEnergies/PEMFuelCellUnitOpBase.vb`,
//!   `PEMFC_Amphlett.vb`, `PEMFC_ChamberlineKim.vb`, `PEMFC_LarminieDicks.vb`
//! - **Upstream commit:** `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`)
//! - **Upstream copyright:** Daniel Wagner O. de Medeiros and the DWSIM contributors
//! - **Upstream licence:** GPL-3.0
//! - **This port:** GPL-3.0-only (OUTRAM PARK fork; not the official DWSIM software)
//!
//! # IMPORTANT — where the polarization physics actually lives
//!
//! **The DWSIM `.vb` files contain no polarization equations at all.** This
//! has to be stated plainly, because the file names suggest otherwise:
//!
//! - `PEMFC_Amphlett.vb:131-273` opens the CPython GIL, imports
//!   `opem.Static.Amphlett`, marshals the parameter dictionary into Python,
//!   and calls `Static_Analysis(...)`. Every voltage, loss and power number
//!   comes back from Python; the VB code only unpacks the result lists
//!   (`:179-231`) and does the stream bookkeeping (`:233-271`).
//! - `PEMFC_ChamberlineKim.vb` (54 lines) has **no `Calculate` override and
//!   no parameter set** — it defines a display name, an icon, and XML
//!   cloning. As shipped it computes nothing.
//! - `PEMFC_LarminieDicks.vb` (68 lines) has **no `Calculate` override**
//!   either; it contributes only its default parameter list (`:26-37`).
//!
//! The equations are in the Python package DWSIM vendors and loads at
//! runtime, **OPEM** (Open-source PEM fuel-cell simulation tool), at
//! `PlatformFiles/Common/python_packages/opem/` in the same DWSIM commit —
//! specifically `opem/Static/Amphlett.py`, `opem/Static/Chamberline_Kim.py`,
//! `opem/Static/Larminie_Dicks.py` and the constants in `opem/Params.py`.
//!
//! This module ports **both halves**: DWSIM's stoichiometry and stream
//! arithmetic, and the OPEM correlations they depend on, re-expressed in
//! Rust. No Python runtime is introduced — the workspace is pure Rust and
//! Android-buildable, and an embedded CPython would break both properties.
//!
//! ## Licence provenance — UNRESOLVED, needs maintainer review
//!
//! **The vendored OPEM directory in this DWSIM commit carries no `LICENSE`
//! file**, and no licence header appears in any of its `.py` sources (the
//! only licence text anywhere under `python_packages/opem/` is an MIT notice
//! belonging to the bundled *Chart.js* asset in `opem/Script.py:9-11`, which
//! is unrelated). OPEM is published by ECSIM as an MIT-licensed project, and
//! MIT into GPL-3.0 is a permitted one-way flow — but that has **not been
//! verified against a licence file in this repository**, and this port must
//! not claim otherwise.
//!
//! What was done to keep this defensible:
//!
//! - The Rust below is written from the **published correlations** —
//!   Amphlett et al. (1995), Chamberlin-Kim (1995), Larminie-Dicks (2003) —
//!   which are literature equations, not copyrightable expression, and each
//!   is cited at its function.
//! - The empirical coefficients and parameter defaults are recorded as OPEM
//!   spells them, with the OPEM file and line cited, so the provenance chain
//!   is auditable rather than laundered.
//! - No OPEM source is vendored, copied verbatim, or added as a dependency.
//!
//! **Action for the maintainer:** confirm OPEM's licence from its own
//! distribution before this module is described as cleared, and record the
//! finding in the crate `NOTICE`. Until then treat this module as an
//! untrusted AI-assisted draft with an open provenance question — which is
//! the workspace default for AI-written code anyway.
//!
//! # Model selection
//!
//! The three polarization models form a closed set, so they are an enum —
//! [`PemFuelCellModel`] — not a trait object. Each variant owns its own
//! parameter struct by value.
//!
//! # Excluded DWSIM behaviour
//!
//! Beyond the module-wide exclusions in [`crate::clean_energies`], this file
//! drops:
//!
//! - the entire CPython bridge (`PEMFC_Amphlett.vb:86-90, :131-178, :273`) —
//!   replaced by the native Rust correlations, as described above;
//! - the HTML / CSV / OPEM report generation and its temp-file round trip
//!   (`:156-177, :191-193`), together with OPEM's own `Output_Init` /
//!   `CSV_Init` / `HTML_Init` reporting layer — these write files, they do
//!   not compute physics;
//! - the `PEMFuelCellModelParameter` string-keyed dictionary and its
//!   `InputParameters` / `OutputParameters` maps
//!   (`PEMFuelCellUnitOpBase.vb:12-57, :82-84`) — replaced by typed parameter
//!   structs, so a misspelt key is a compile error instead of a silent
//!   default;
//! - the plot-series bookkeeping (`ValuesX` / `ValuesY` / `TitleX` /
//!   `TitleY`, `PEMFC_Amphlett.vb:198-231`) — the sweep in
//!   [`PolarizationCurve`] carries the same data without the GUI framing;
//! - OPEM's mutable warning flags `warning_check_1` / `warning_check_2`,
//!   which print advisory text about negative or unusually high voltages.
//!   The equivalent information is available from the returned curve.

use uom::si::catalytic_activity::katal;
use uom::si::electric_current::ampere;
use uom::si::electric_potential::volt;
use uom::si::f64::{
    CatalyticActivity, ElectricCurrent, ElectricPotential, Power, Pressure, Ratio,
    ThermodynamicTemperature,
};
use uom::si::power::watt;
use uom::si::pressure::{atmosphere, pascal};
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

use super::{CleanEnergyError, CleanEnergyUnitOp, FARADAY_CONSTANT_C_PER_MOL};

/// Molar flow rate \[mol/s\]. See
/// [`crate::clean_energies::water_electrolyzer::MolarFlowRate`] for why
/// `CatalyticActivity` is the right `uom` quantity.
pub type MolarFlowRate = CatalyticActivity;

// ---------------------------------------------------------------------------
// OPEM physical constants (opem/Params.py:31-42)
// ---------------------------------------------------------------------------

/// Universal gas constant in OPEM's units, `R = 8314.47 J/(kmol·K)`
/// (`opem/Params.py:40`).
///
/// OPEM works in **kmol**, so this is 1000× the familiar 8.314 47 J/(mol·K).
/// It is paired with [`FARADAY_C_PER_KMOL`] in `B = R T / (n F)`, where the
/// two factors of 1000 cancel — so the result is identical to using SI molar
/// units, and the odd-looking constants are kept only for traceability to
/// OPEM.
pub const GAS_CONSTANT_J_PER_KMOL_K: f64 = 8314.47;

/// Faraday constant in OPEM's units, `F = 96 484 600 C/kmol`
/// (`opem/Params.py:41`).
///
/// 1000× the molar value, and note it differs in the 6th figure from the
/// `96485.3365 C/mol` DWSIM's own electrolyzer uses
/// ([`FARADAY_CONSTANT_C_PER_MOL`]) — the two upstream codebases simply
/// rounded differently. Each is kept where its own code uses it.
pub const FARADAY_C_PER_KMOL: f64 = 96_484_600.0;

/// Hydrogen higher heating value expressed as an equivalent cell voltage,
/// `HHV = 1.482 V` (`opem/Params.py:31`).
///
/// The denominator of OPEM's efficiency definition: a cell running at
/// 1.482 V would convert hydrogen's full HHV to electricity. Numerically the
/// same quantity as the electrolyzer's thermoneutral voltage.
pub const HHV_VOLTAGE: f64 = 1.482;

/// Fuel-utilization factor `uF = 0.95` (`opem/Params.py:32`) — the fraction
/// of supplied hydrogen actually consumed, folded into OPEM's efficiency
/// definition as a constant.
pub const FUEL_UTILIZATION: f64 = 0.95;

/// Thermodynamic (reversible) cell potential `Eth = 1.23 V`
/// (`opem/Params.py:42`), used as the reference in the thermal-power balance
/// `P_th = i (N Eth - V_stack)`.
pub const REVERSIBLE_POTENTIAL_V: f64 = 1.23;

/// Amphlett empirical parametric coefficient `xi1 = -0.948`
/// (`opem/Params.py:35`). Dimensionless; appears in the activation
/// overpotential.
pub const XI1: f64 = -0.948;

/// Amphlett empirical parametric coefficient `xi3 = 7.6e-5`
/// (`opem/Params.py:36`), in V/K per unit `ln(C_O2)`.
pub const XI3: f64 = 7.6e-5;

/// Amphlett empirical parametric coefficient `xi4 = -1.93e-4`
/// (`opem/Params.py:37`), in V/K per unit `ln(i)`.
pub const XI4: f64 = -1.93e-4;

/// Electrons transferred per H2 in the fuel-cell reaction, `n = 2` — OPEM's
/// default `n` argument to `B_Calc` (`opem/Static/Amphlett.py:13`).
pub const ELECTRONS_PER_HYDROGEN: f64 = 2.0;

// ---------------------------------------------------------------------------
// Amphlett model
// ---------------------------------------------------------------------------

/// Mass-transfer constant `B = R T / (n F)` \[V\] — OPEM `B_Calc`
/// (`opem/Static/Amphlett.py:13-26`).
///
/// The thermal voltage scale that sets the steepness of the concentration
/// overpotential. At 343 K this is about 0.0148 V. `temperature_k` \[K\] must
/// be `> 0`; `n` is the electron count, 2 for H2.
pub fn mass_transfer_constant(temperature_k: f64, electrons: f64) -> f64 {
    GAS_CONSTANT_J_PER_KMOL_K * temperature_k / (electrons * FARADAY_C_PER_KMOL)
}

/// Nernst open-circuit voltage `E_nernst` \[V\] — OPEM `Enernst_Calc`
/// (`opem/Static/Amphlett.py:136-155`), from Amphlett et al. (1995):
///
/// `E = 1.229 - 8.5e-4 (T - 298.15) + 4.308e-5 T (ln P_H2 + 0.5 ln P_O2)`
///
/// The first term is the standard potential of `H2 + 0.5 O2 -> H2O(l)`, the
/// second its temperature coefficient, and the third the Nernst pressure
/// correction.
///
/// # Inputs and valid ranges
///
/// - `temperature_k` — cell temperature \[K\]. PEM cells operate 320-360 K;
///   the correlation is fitted there.
/// - `p_h2_atm`, `p_o2_atm` — hydrogen and oxygen partial pressures **in
///   atmospheres**, both strictly `> 0` (logarithms). OPEM works in atm
///   throughout, which is why `PEMFC_Amphlett.vb:128-129` divides by 101325.
///
/// # Errors
///
/// [`CleanEnergyError::OutOfDomain`] if either partial pressure is
/// non-positive. OPEM catches the `ValueError` and prints an error, yielding
/// `None`.
pub fn nernst_voltage(
    temperature_k: f64,
    p_h2_atm: f64,
    p_o2_atm: f64,
) -> Result<f64, CleanEnergyError> {
    if p_h2_atm <= 0.0 {
        return Err(CleanEnergyError::OutOfDomain {
            parameter: "p_h2_atm",
            value: p_h2_atm,
            reason: "hydrogen partial pressure must be positive (a logarithm is taken)",
        });
    }
    if p_o2_atm <= 0.0 {
        return Err(CleanEnergyError::OutOfDomain {
            parameter: "p_o2_atm",
            value: p_o2_atm,
            reason: "oxygen partial pressure must be positive (a logarithm is taken)",
        });
    }
    Ok(1.229 - 8.5e-4 * (temperature_k - 298.15)
        + 4.308e-5 * temperature_k * (p_h2_atm.ln() + 0.5 * p_o2_atm.ln()))
}

/// Hydrogen concentration at the anode catalyst interface
/// `C_H2 = P_H2 / (1.09e6 exp(77/T))` \[mol/cm³\] — OPEM `CH2_Calc`
/// (`opem/Static/Amphlett.py:158-174`).
///
/// A Henry's-law solubility of H2 in the membrane water. `p_h2_atm` in
/// atmospheres, `temperature_k` in kelvin (`> 0`). Feeds the `xi2`
/// coefficient of the activation overpotential.
pub fn hydrogen_concentration_mol_per_cm3(p_h2_atm: f64, temperature_k: f64) -> f64 {
    p_h2_atm / (1.09e6 * (77.0 / temperature_k).exp())
}

/// Oxygen concentration at the cathode catalyst interface
/// `C_O2 = P_O2 / (5.08e6 exp(-498/T))` \[mol/cm³\] — OPEM `CO2_Calc`
/// (`opem/Static/Amphlett.py:177-193`).
///
/// The cathode counterpart of [`hydrogen_concentration_mol_per_cm3`]; note
/// the **opposite sign** in the exponent, which makes oxygen solubility rise
/// with temperature where hydrogen's falls. `p_o2_atm` in atmospheres,
/// `temperature_k` in kelvin (`> 0`).
pub fn oxygen_concentration_mol_per_cm3(p_o2_atm: f64, temperature_k: f64) -> f64 {
    p_o2_atm / (5.08e6 * (-498.0 / temperature_k).exp())
}

/// Membrane specific resistivity `rho_M` \[ohm·cm\] — OPEM `Rho_Calc`
/// (`opem/Static/Amphlett.py:196-217`), the Springer/Amphlett empirical form:
///
/// `rho = 181.6 (1 + 0.03 J + 0.062 (T/303)² J^2.5) / ((lambda - 0.634 - 3 J) exp(4.18 (T-303)/T))`
///
/// with `J = i / A` the current density \[A/cm²\].
///
/// # Inputs and valid ranges
///
/// - `current_a` — cell current \[A\], `>= 0`.
/// - `active_area_cm2` — membrane active area \[cm²\], `> 0`. DWSIM's default
///   is 50.6 cm² (`PEMFC_Amphlett.vb:43`).
/// - `temperature_k` — \[K\], `> 0`.
/// - `lambda_param` — membrane water content, dimensionless. Amphlett's
///   adjustable parameter, physically **14 to 23** (DWSIM's editor labels it
///   "Adjustable Parameter (14-23)", `PEMFC_Amphlett.vb:45`, default 23 =
///   fully hydrated).
///
/// # Errors
///
/// [`CleanEnergyError::OutOfDomain`] if the denominator factor
/// `lambda - 0.634 - 3 J` is non-positive — the membrane has dried out or the
/// current density has exceeded what this correlation can represent, and the
/// resistivity would be infinite or negative. OPEM divides anyway and prints
/// an error.
pub fn membrane_resistivity_ohm_cm(
    current_a: f64,
    active_area_cm2: f64,
    temperature_k: f64,
    lambda_param: f64,
) -> Result<f64, CleanEnergyError> {
    let j = current_a / active_area_cm2;
    let denominator_factor = lambda_param - 0.634 - 3.0 * j;
    if denominator_factor <= 0.0 {
        return Err(CleanEnergyError::OutOfDomain {
            parameter: "lambda - 0.634 - 3 J",
            value: denominator_factor,
            reason: "membrane water content is too low for this current density; the Amphlett \
                     resistivity correlation is undefined here",
        });
    }
    let numerator =
        181.6 * (1.0 + 0.03 * j + 0.062 * (temperature_k / 303.0).powi(2) * j.powf(2.5));
    let denominator = denominator_factor * (4.18 * ((temperature_k - 303.0) / temperature_k)).exp();
    Ok(numerator / denominator)
}

/// Amphlett parametric coefficient
/// `xi2 = 0.00286 + 0.0002 ln(A) + 4.3e-5 ln(C_H2)` — OPEM `Xi2_Calc`
/// (`opem/Static/Amphlett.py:220-240`).
///
/// Unlike `xi1`, `xi3` and `xi4`, which are constants, `xi2` depends on the
/// active area and the anode hydrogen concentration. Units V/K (it multiplies
/// `T` in the activation overpotential).
///
/// `active_area_cm2` must be `> 0` and `p_h2_atm` must be `> 0` (both are
/// logged, the latter through `C_H2`).
///
/// # Errors
///
/// [`CleanEnergyError::OutOfDomain`] if either argument is non-positive.
pub fn xi2_coefficient(
    active_area_cm2: f64,
    p_h2_atm: f64,
    temperature_k: f64,
) -> Result<f64, CleanEnergyError> {
    if active_area_cm2 <= 0.0 {
        return Err(CleanEnergyError::OutOfDomain {
            parameter: "active_area_cm2",
            value: active_area_cm2,
            reason: "active area must be positive (a logarithm is taken)",
        });
    }
    if p_h2_atm <= 0.0 {
        return Err(CleanEnergyError::OutOfDomain {
            parameter: "p_h2_atm",
            value: p_h2_atm,
            reason: "hydrogen partial pressure must be positive (a logarithm is taken)",
        });
    }
    let c_h2 = hydrogen_concentration_mol_per_cm3(p_h2_atm, temperature_k);
    Ok(0.00286 + 0.0002 * active_area_cm2.ln() + 4.3e-5 * c_h2.ln())
}

/// Activation overpotential `eta_act` \[V\] — OPEM `Eta_Act_Calc`
/// (`opem/Static/Amphlett.py:299-322`):
///
/// `eta_act = -(xi1 + xi2 T + xi3 T ln(C_O2) + xi4 T ln(i))`
///
/// The charge-transfer loss dominating at low current — a Tafel-like
/// logarithmic rise. Returned **positive** (it is subtracted from the Nernst
/// voltage by [`total_loss`]); the leading minus sign is upstream's, because
/// the bracketed sum is negative for physical inputs.
///
/// **At `i = 0` OPEM returns exactly 0** rather than the `-infinity` the
/// `ln(i)` term would give (`Amphlett.py:312, :318`). That discontinuity is
/// reproduced here: it is how the open-circuit point is defined in this
/// model.
///
/// `current_a` `>= 0`, `active_area_cm2` `> 0`, both partial pressures `> 0`,
/// `temperature_k` `> 0`.
///
/// # Errors
///
/// [`CleanEnergyError::OutOfDomain`] via [`xi2_coefficient`], or if the
/// oxygen partial pressure is non-positive.
pub fn activation_overpotential(
    temperature_k: f64,
    p_o2_atm: f64,
    p_h2_atm: f64,
    current_a: f64,
    active_area_cm2: f64,
) -> Result<f64, CleanEnergyError> {
    if current_a == 0.0 {
        return Ok(0.0);
    }
    if p_o2_atm <= 0.0 {
        return Err(CleanEnergyError::OutOfDomain {
            parameter: "p_o2_atm",
            value: p_o2_atm,
            reason: "oxygen partial pressure must be positive (a logarithm is taken)",
        });
    }
    if current_a < 0.0 {
        return Err(CleanEnergyError::OutOfDomain {
            parameter: "current_a",
            value: current_a,
            reason: "current must be non-negative (a logarithm is taken)",
        });
    }
    let c_o2 = oxygen_concentration_mol_per_cm3(p_o2_atm, temperature_k);
    let xi2 = xi2_coefficient(active_area_cm2, p_h2_atm, temperature_k)?;
    Ok(-(XI1
        + xi2 * temperature_k
        + XI3 * temperature_k * c_o2.ln()
        + XI4 * temperature_k * current_a.ln()))
}

/// Ohmic overpotential `eta_ohmic = i (rho_M l / A + R_elec)` \[V\] — OPEM
/// `Eta_Ohmic_Calc` (`opem/Static/Amphlett.py:265-296`).
///
/// The linear IR drop across the proton-conducting membrane plus any
/// electronic contact resistance. `rho_M l / A` converts the membrane
/// resistivity to a resistance.
///
/// - `membrane_thickness_cm` — `l` \[cm\]. DWSIM default 0.0178 cm
///   (`PEMFC_Amphlett.vb:44`), i.e. a 178 µm Nafion 117 membrane.
/// - `electronic_resistance_ohm` — `R_elec` \[ohm\]. DWSIM default 0
///   (`:46`), and OPEM treats it as optional (`Params.py:123`,
///   `Amphlett_Params_Default = {"R": 0}`).
///
/// **At `i = 0` OPEM returns exactly 0** (`Amphlett.py:284, :292`), which
/// coincides with the formula anyway. Remaining arguments as for
/// [`membrane_resistivity_ohm_cm`].
///
/// # Errors
///
/// [`CleanEnergyError::OutOfDomain`] via [`membrane_resistivity_ohm_cm`].
pub fn ohmic_overpotential(
    current_a: f64,
    membrane_thickness_cm: f64,
    active_area_cm2: f64,
    temperature_k: f64,
    lambda_param: f64,
    electronic_resistance_ohm: f64,
) -> Result<f64, CleanEnergyError> {
    if current_a == 0.0 {
        return Ok(0.0);
    }
    let rho = membrane_resistivity_ohm_cm(current_a, active_area_cm2, temperature_k, lambda_param)?;
    let r_proton = rho * membrane_thickness_cm / active_area_cm2;
    Ok(current_a * (r_proton + electronic_resistance_ohm))
}

/// Concentration (mass-transport) overpotential
/// `eta_conc = -B ln(1 - J / J_max)` \[V\] — OPEM `Eta_Conc_Calc`
/// (`opem/Static/Amphlett.py:243-262`).
///
/// The reactant-starvation loss that diverges as the current density
/// approaches the limiting density `J_max`, producing the sharp voltage
/// collapse at the right-hand end of the polarization curve.
///
/// - `b_constant` — from [`mass_transfer_constant`] \[V\].
/// - `max_current_density_a_per_cm2` — `J_max`, DWSIM default 1.5 A/cm²
///   (`PEMFC_Amphlett.vb:47`).
///
/// **At `i = 0` OPEM returns exactly 0** (`Amphlett.py:254, :258`).
///
/// # Errors
///
/// [`CleanEnergyError::OutOfDomain`] if `J >= J_max` — at or beyond the
/// limiting current the logarithm's argument is non-positive and the
/// overpotential is infinite. OPEM prints an error and yields `None`; the
/// sweep in [`PolarizationCurve`] avoids the region by capping the sweep at
/// `J_max * A`, exactly as `Amphlett.py:492-493` does.
pub fn concentration_overpotential(
    current_a: f64,
    active_area_cm2: f64,
    b_constant: f64,
    max_current_density_a_per_cm2: f64,
) -> Result<f64, CleanEnergyError> {
    if current_a == 0.0 {
        return Ok(0.0);
    }
    let j = current_a / active_area_cm2;
    let argument = 1.0 - j / max_current_density_a_per_cm2;
    if argument <= 0.0 {
        return Err(CleanEnergyError::OutOfDomain {
            parameter: "1 - J/J_max",
            value: argument,
            reason: "current density has reached the limiting current density; the concentration \
                     overpotential is infinite there",
        });
    }
    Ok(-b_constant * argument.ln())
}

/// Total polarization loss `eta_act + eta_ohmic + eta_conc` \[V\] — OPEM
/// `Loss_Calc` (`opem/Static/Amphlett.py:361-379`).
///
/// A plain sum, factored out because it is the quantity DWSIM reports and
/// because keeping it named makes the `V_cell = E_nernst - Loss` structure
/// visible.
pub fn total_loss(activation: f64, ohmic: f64, concentration: f64) -> f64 {
    activation + ohmic + concentration
}

/// PEM efficiency `eta = uF V_cell / HHV`, dimensionless — OPEM
/// `Efficiency_Calc` (`opem/Static/Amphlett.py:325-339`).
///
/// The fraction of hydrogen's higher heating value converted to electricity,
/// with the fuel-utilization factor [`FUEL_UTILIZATION`] folded in. Shared by
/// all three models (Chamberlin-Kim and Larminie-Dicks both import it —
/// `Chamberline_Kim.py:7`, `Larminie_Dicks.py:4`).
///
/// A typical PEM cell at 0.7 V gives `0.95 * 0.7 / 1.482 = 0.449`, i.e. about
/// 45 %.
pub fn cell_efficiency(cell_voltage_v: f64) -> f64 {
    FUEL_UTILIZATION * cell_voltage_v / HHV_VOLTAGE
}

/// Stack voltage `V_stack = N V_cell` \[V\] — OPEM `VStack_Calc`
/// (`opem/Static/Amphlett.py:342-358`). Cells are in series.
pub fn stack_voltage(number_of_cells: u32, cell_voltage_v: f64) -> f64 {
    f64::from(number_of_cells) * cell_voltage_v
}

/// Thermal (waste-heat) power `P_th = i (N Eth - V_stack)` \[W\] — OPEM
/// `Power_Thermal_Calc` (`opem/Static/Amphlett.py:29-44`).
///
/// The gap between the reversible potential the reaction could deliver and
/// the voltage actually produced, times the current — i.e. all the
/// polarization losses appearing as heat. Positive whenever the stack runs
/// below `N * Eth`, which is always the case in operation.
pub fn thermal_power(stack_voltage_v: f64, number_of_cells: u32, current_a: f64) -> f64 {
    current_a * (f64::from(number_of_cells) * REVERSIBLE_POTENTIAL_V - stack_voltage_v)
}

/// Amphlett static-model parameters — DWSIM's `InputParameters` dictionary
/// for this model (`PEMFC_Amphlett.vb:37-50`), typed.
///
/// Defaults reproduce DWSIM's `AddDefaultInputParameters` exactly; the model
/// is from Amphlett, Baumert, Mann, Peppley and Roberge, *Performance
/// modeling of the Ballard Mark IV solid polymer electrolyte fuel cell*,
/// J. Electrochem. Soc. 142(1), 1995.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmphlettParameters {
    /// Membrane active area `A` \[cm²\] (`:43`, default 50.6).
    pub active_area_cm2: f64,
    /// Membrane thickness `l` \[cm\] (`:44`, default 0.0178).
    pub membrane_thickness_cm: f64,
    /// Membrane water content `lambda`, dimensionless, physically 14-23
    /// (`:45`, default 23).
    pub lambda_param: f64,
    /// Electronic (contact) resistance `R` \[ohm\] (`:46`, default 0).
    pub electronic_resistance_ohm: f64,
    /// Limiting current density `J_max` \[A/cm²\] (`:47`, default 1.5).
    pub max_current_density_a_per_cm2: f64,
    /// Number of single cells `N` (`:48`, default 1).
    pub number_of_cells: u32,
}

impl Default for AmphlettParameters {
    fn default() -> Self {
        Self {
            active_area_cm2: 50.6,
            membrane_thickness_cm: 0.0178,
            lambda_param: 23.0,
            electronic_resistance_ohm: 0.0,
            max_current_density_a_per_cm2: 1.5,
            number_of_cells: 1,
        }
    }
}

/// The operating conditions the Amphlett model needs beyond its parameters —
/// DWSIM computes these from the two inlet streams
/// (`PEMFC_Amphlett.vb:110-129`).
///
/// See [`operating_partial_pressures`] for how DWSIM derives the partial
/// pressures; they are in **atmospheres**, OPEM's unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PemOperatingConditions {
    /// Cell temperature `T` \[K\] — DWSIM uses the mean of the two inlet
    /// stream temperatures (`PEMFC_Amphlett.vb:115`).
    pub temperature_k: f64,
    /// Hydrogen partial pressure `P_H2` \[atm\] (`:128`).
    pub p_h2_atm: f64,
    /// Oxygen partial pressure `P_O2` \[atm\] (`:129`).
    pub p_o2_atm: f64,
}

/// Chamberlin-Kim static-model parameters.
///
/// DWSIM's `PEMFC_ChamberLineKim` class defines **no** parameter set — it
/// never overrides `AddDefaultInputParameters` — so the defaults here come
/// from OPEM's own standard vector (`opem/Params.py:226-237`,
/// `Chamberline_Standard_Vector`) with `m` and `n` from
/// `Chamberline_Params_Default` (`Params.py:210`). This is recorded because
/// it is the one place a default could not be taken from DWSIM itself.
///
/// Model: Kim, Lee, Srinivasan and Chamberlin, *Modeling of proton exchange
/// membrane fuel cell performance with an empirical equation*,
/// J. Electrochem. Soc. 142(8), 1995.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChamberlinKimParameters {
    /// Open-circuit voltage `E0` \[V\] (OPEM standard vector: 0.982).
    pub open_circuit_voltage_v: f64,
    /// Tafel slope for oxygen reduction `b` \[V\] (0.0689).
    pub tafel_slope_v: f64,
    /// Area-specific resistance `R` \[ohm·cm²\] (0.328).
    pub area_resistance_ohm_cm2: f64,
    /// Mass-transport prefactor `m` \[V\] (0.000125; OPEM's optional default
    /// is `3e-8`).
    pub diffusion_m_v: f64,
    /// Mass-transport exponent coefficient `n` \[cm²/A\] (9.45; OPEM's
    /// optional default is 8).
    pub diffusion_n_cm2_per_a: f64,
    /// Membrane active area `A` \[cm²\] (50.0).
    pub active_area_cm2: f64,
    /// Number of single cells `N` (1).
    pub number_of_cells: u32,
}

impl Default for ChamberlinKimParameters {
    fn default() -> Self {
        Self {
            open_circuit_voltage_v: 0.982,
            tafel_slope_v: 0.0689,
            area_resistance_ohm_cm2: 0.328,
            diffusion_m_v: 0.000125,
            diffusion_n_cm2_per_a: 9.45,
            active_area_cm2: 50.0,
            number_of_cells: 1,
        }
    }
}

/// Larminie-Dicks static-model parameters — DWSIM's `InputParameters` for
/// this model (`PEMFC_LarminieDicks.vb:26-37`), typed.
///
/// > **DWSIM's own defaults are placeholders, not physical values.** Upstream
/// > sets `i_n = i_0 = i_L = 1 A`, `E0 = 0 V`, `A = 0 V` and `RM = 1 ohm`
/// > (`:30-35`), which produce a flat zero polarization curve — clearly
/// > "fill these in yourself" stubs left in the editor. Using them verbatim
/// > as this struct's `Default` would ship a model that computes nothing, so
/// > [`Default`] here uses OPEM's physically meaningful standard vector
/// > instead (`opem/Params.py:177-189`, `Larminiee_Standard_Vector`). The
/// > DWSIM placeholders are available from
/// > [`LarminieDicksParameters::dwsim_placeholder_defaults`] for parity
/// > checking.
///
/// Model: Larminie and Dicks, *Fuel Cell Systems Explained*, 2nd ed., Wiley
/// 2003.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LarminieDicksParameters {
    /// Reversible no-loss voltage `E0` \[V\] (OPEM standard vector: 1.178).
    pub no_loss_voltage_v: f64,
    /// Tafel line slope `A` \[V\] (0.06).
    pub tafel_slope_v: f64,
    /// Internal (crossover) current `i_n` \[A\] (0.23).
    pub internal_current_a: f64,
    /// Exchange current `i_0` \[A\], where activation overvoltage leaves zero
    /// (0.00654).
    pub exchange_current_a: f64,
    /// Limiting current `i_L` \[A\] (100.0).
    pub limiting_current_a: f64,
    /// Membrane and contact resistance `R_M` \[ohm\] (0.0018).
    pub membrane_resistance_ohm: f64,
    /// Cell temperature `T` \[K\] (328.15) — used only to evaluate the
    /// mass-transfer constant `B`.
    pub temperature_k: f64,
    /// Number of single cells `N` (23).
    pub number_of_cells: u32,
}

impl Default for LarminieDicksParameters {
    fn default() -> Self {
        Self {
            no_loss_voltage_v: 1.178,
            tafel_slope_v: 0.06,
            internal_current_a: 0.23,
            exchange_current_a: 0.00654,
            limiting_current_a: 100.0,
            membrane_resistance_ohm: 0.0018,
            temperature_k: 328.15,
            number_of_cells: 23,
        }
    }
}

impl LarminieDicksParameters {
    /// DWSIM's literal editor defaults (`PEMFC_LarminieDicks.vb:30-36`),
    /// provided for parity checking only.
    ///
    /// These are placeholders — `E0 = 0` and `A = 0` make the polarization
    /// curve degenerate — so they are **not** this struct's [`Default`]. See
    /// the struct docs.
    pub fn dwsim_placeholder_defaults() -> Self {
        Self {
            no_loss_voltage_v: 0.0,
            tafel_slope_v: 0.0,
            internal_current_a: 1.0,
            exchange_current_a: 1.0,
            limiting_current_a: 1.0,
            membrane_resistance_ohm: 1.0,
            temperature_k: 328.15,
            number_of_cells: 1,
        }
    }
}

/// The three PEM polarization models — a closed set, so an enum, per the
/// workspace no-trait-objects rule.
///
/// Each variant owns its parameter struct **by value**: no `Box`, no `Arc`,
/// no lifetimes. Adding a fourth model makes every `match` below a compile
/// error until it is handled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PemFuelCellModel {
    /// Amphlett et al. (1995) mechanistic-empirical model — the only one
    /// DWSIM actually wires up (`PEMFC_Amphlett.vb`). Needs
    /// [`PemOperatingConditions`] as well as its parameters, because its
    /// Nernst term and all three overpotentials depend on temperature and
    /// partial pressures.
    Amphlett(AmphlettParameters, PemOperatingConditions),
    /// Chamberlin-Kim (1995) empirical fit. Purely a curve fit — no
    /// temperature or pressure dependence at all, which is why its
    /// parameters must be re-identified for every operating condition (a
    /// limitation OPEM spells out at `Params.py:212-225`).
    ChamberlinKim(ChamberlinKimParameters),
    /// Larminie-Dicks (2003) three-region model. Temperature enters only
    /// through the mass-transfer constant `B`.
    LarminieDicks(LarminieDicksParameters),
}

impl PemFuelCellModel {
    /// Number of single cells `N` in the stack, whichever model is selected.
    pub fn number_of_cells(&self) -> u32 {
        match self {
            Self::Amphlett(p, _) => p.number_of_cells,
            Self::ChamberlinKim(p) => p.number_of_cells,
            Self::LarminieDicks(p) => p.number_of_cells,
        }
    }

    /// Human-readable model name, matching DWSIM's `GetDisplayName()`
    /// (`PEMFC_Amphlett.vb:17-21`, `PEMFC_ChamberlineKim.vb:12-14`,
    /// `PEMFC_LarminieDicks.vb:13-15`).
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Amphlett(_, _) => "PEM Fuel Cell (Amphlett)",
            Self::ChamberlinKim(_) => "PEM Fuel Cell (Chamberline-Kim)",
            Self::LarminieDicks(_) => "PEM Fuel Cell (Larminie-Dicks)",
        }
    }

    /// Single-cell voltage `V_cell` \[V\] at the given load current \[A\] —
    /// the polarization curve, evaluated pointwise.
    ///
    /// Dispatches to:
    ///
    /// - **Amphlett** — `V = E_nernst - (eta_act + eta_ohmic + eta_conc)`
    ///   (`opem/Static/Amphlett.py:382-398`).
    /// - **Chamberlin-Kim** — `V = E0 - b ln(J) - R J - m exp(n J)` with
    ///   `J = i / A` (`opem/Static/Chamberline_Kim.py:13-40`). The `ln(J)`
    ///   term means this model is **undefined at zero current** and its
    ///   sweep must start above it (OPEM's standard vector starts at 1 A).
    /// - **Larminie-Dicks** —
    ///   `V = E0 - A ln((i+i_n)/i_0) - R_M (i+i_n) + B ln(1 - (i+i_n)/i_L)`
    ///   (`opem/Static/Larminie_Dicks.py:12-41`). The final term is negative
    ///   for `i < i_L` (a loss); `B` comes from
    ///   [`mass_transfer_constant`] at the parameters' temperature.
    ///
    /// All three are strictly **decreasing** in current over their valid
    /// range, which is the defining property of a polarization curve.
    ///
    /// # Errors
    ///
    /// [`CleanEnergyError::OutOfDomain`] whenever the current falls outside
    /// the selected model's domain — see the individual overpotential
    /// functions, and note the model-specific edges called out above.
    pub fn cell_voltage(&self, current_a: f64) -> Result<f64, CleanEnergyError> {
        match self {
            Self::Amphlett(p, cond) => {
                let e_nernst = nernst_voltage(cond.temperature_k, cond.p_h2_atm, cond.p_o2_atm)?;
                let b = mass_transfer_constant(cond.temperature_k, ELECTRONS_PER_HYDROGEN);
                let eta_act = activation_overpotential(
                    cond.temperature_k,
                    cond.p_o2_atm,
                    cond.p_h2_atm,
                    current_a,
                    p.active_area_cm2,
                )?;
                let eta_ohm = ohmic_overpotential(
                    current_a,
                    p.membrane_thickness_cm,
                    p.active_area_cm2,
                    cond.temperature_k,
                    p.lambda_param,
                    p.electronic_resistance_ohm,
                )?;
                let eta_conc = concentration_overpotential(
                    current_a,
                    p.active_area_cm2,
                    b,
                    p.max_current_density_a_per_cm2,
                )?;
                Ok(e_nernst - total_loss(eta_act, eta_ohm, eta_conc))
            }
            Self::ChamberlinKim(p) => {
                if current_a <= 0.0 {
                    return Err(CleanEnergyError::OutOfDomain {
                        parameter: "current_a",
                        value: current_a,
                        reason: "the Chamberlin-Kim equation takes ln(J) and is undefined at or \
                                 below zero current",
                    });
                }
                let j = current_a / p.active_area_cm2;
                Ok(p.open_circuit_voltage_v
                    - p.tafel_slope_v * j.ln()
                    - p.area_resistance_ohm_cm2 * j
                    - p.diffusion_m_v * (p.diffusion_n_cm2_per_a * j).exp())
            }
            Self::LarminieDicks(p) => {
                let i_total = current_a + p.internal_current_a;
                if i_total <= 0.0 || p.exchange_current_a <= 0.0 {
                    return Err(CleanEnergyError::OutOfDomain {
                        parameter: "(i + i_n) / i_0",
                        value: i_total,
                        reason: "the Larminie-Dicks activation term takes ln((i+i_n)/i_0), which \
                                 needs both quantities strictly positive",
                    });
                }
                let transport_argument = 1.0 - i_total / p.limiting_current_a;
                if transport_argument <= 0.0 {
                    return Err(CleanEnergyError::OutOfDomain {
                        parameter: "1 - (i+i_n)/i_L",
                        value: transport_argument,
                        reason: "the load has reached the limiting current; the mass-transport \
                                 term is infinite there",
                    });
                }
                let b = mass_transfer_constant(p.temperature_k, ELECTRONS_PER_HYDROGEN);
                Ok(p.no_loss_voltage_v
                    - p.tafel_slope_v * (i_total / p.exchange_current_a).ln()
                    - p.membrane_resistance_ohm * i_total
                    + b * transport_argument.ln())
            }
        }
    }

    /// One fully solved operating point at the given load current — the
    /// per-iteration body of OPEM's `Static_Analysis` loop
    /// (`opem/Static/Amphlett.py:510-549`, and the parallel loops in the
    /// other two models).
    ///
    /// # Errors
    ///
    /// Propagates every domain error from [`Self::cell_voltage`].
    pub fn operating_point(&self, current_a: f64) -> Result<PemOperatingPoint, CleanEnergyError> {
        let v_cell = self.cell_voltage(current_a)?;
        let n = self.number_of_cells();
        let v_stack = stack_voltage(n, v_cell);
        Ok(PemOperatingPoint {
            current_a,
            cell_voltage_v: v_cell,
            stack_voltage_v: v_stack,
            cell_power_w: v_cell * current_a,
            stack_power_w: f64::from(n) * v_cell * current_a,
            thermal_power_w: thermal_power(v_stack, n, current_a),
            efficiency: cell_efficiency(v_cell),
        })
    }
}

/// One point on a polarization curve — the six OPEM output quantities plus
/// the current that produced them (`opem/Params.py` `*_OutputParams`:
/// `Vcell`, `PEM Efficiency`, `Power`, `VStack`, `Power-Stack`,
/// `Power-Thermal`).
///
/// Raw `f64` in SI (volts, amperes, watts) — this is the inner-loop type, and
/// a sweep holds thousands of them; the `uom` boundary is at
/// [`PemFuelCell`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PemOperatingPoint {
    /// Load current `i` \[A\].
    pub current_a: f64,
    /// Single-cell voltage `V_cell` \[V\].
    pub cell_voltage_v: f64,
    /// Stack voltage `V_stack = N V_cell` \[V\].
    pub stack_voltage_v: f64,
    /// Single-cell electrical power `V_cell i` \[W\].
    pub cell_power_w: f64,
    /// Stack electrical power `N V_cell i` \[W\].
    pub stack_power_w: f64,
    /// Stack thermal power `i (N Eth - V_stack)` \[W\].
    pub thermal_power_w: f64,
    /// PEM efficiency `uF V_cell / HHV`, dimensionless.
    pub efficiency: f64,
}

/// A swept polarization curve plus the overall parameters OPEM derives from
/// it (`opem/Static/Amphlett.py:574-589`).
///
/// This is the whole of what DWSIM unpacks from the Python call
/// (`PEMFC_Amphlett.vb:179-231`), minus the report strings.
#[derive(Debug, Clone, PartialEq)]
pub struct PolarizationCurve {
    /// Every solved operating point, in ascending current order.
    pub points: Vec<PemOperatingPoint>,
    /// Maximum stack power over the sweep \[W\] — OPEM's `Pmax`
    /// (`Max_Params_Calc`, `Amphlett.py:97-115`).
    pub max_stack_power_w: f64,
    /// Stack voltage at maximum power \[V\] — OPEM's `VFC|Pmax`.
    pub stack_voltage_at_max_power_v: f64,
    /// Efficiency at maximum power, dimensionless — OPEM's
    /// `Efficiency|Pmax`.
    pub efficiency_at_max_power: f64,
    /// Intercept `V0` \[V\] of the least-squares straight line fitted to
    /// `V_stack` versus `i` — OPEM's `linear_plot` / `estimate_coef`
    /// (`opem/Functions.py:39-92`). DWSIM reads this back as `V0`
    /// (`PEMFC_Amphlett.vb:184`).
    pub linear_intercept_v: f64,
    /// Slope `K` \[V/A\] of that same fit — DWSIM's `K`
    /// (`PEMFC_Amphlett.vb:185`). Negative for a real curve.
    pub linear_slope_v_per_a: f64,
    /// `P_max` from the linear approximation, `V0² / (4 |K|)` \[W\] — OPEM's
    /// `Linear_Aprox_Params_Calc` (`Amphlett.py:70-94`), which takes the
    /// absolute value.
    pub linear_max_power_w: f64,
    /// Stack voltage at that linear-approximation maximum, `|V0| / 2` \[V\].
    pub linear_voltage_at_max_power_v: f64,
    /// Simpson's-rule integral of `V_stack` over the current sweep \[V·A\] —
    /// OPEM's `Ptotal(Elec)` (`Power_Total_Calc`, `Amphlett.py:47-67`,
    /// via `integrate`, `Functions.py:15-36`).
    pub total_electrical_w: f64,
    /// Simpson's-rule integral of `(N Eth - V_stack)` over the sweep
    /// \[V·A\] — OPEM's `Ptotal(Thermal)`.
    pub total_thermal_w: f64,
}

/// Simpson's-rule integral of `y_vals` at uniform spacing `h` — OPEM
/// `integrate` (`opem/Functions.py:15-36`).
///
/// > **Reproduced verbatim, including its flaw.** OPEM applies the composite
/// > Simpson weights `1, 4, 2, 4, 2, …, 4, 1` **without checking that the
/// > sample count is odd**, which composite Simpson requires. For an even
/// > count the weighting is wrong and the result carries an `O(h)` error
/// > instead of `O(h⁴)`. Since the sweeps here have thousands of points and
/// > this quantity is only reported, not fed back into any balance, the
/// > effect is small — but it is upstream behaviour, not a correct
/// > quadrature, and callers should not treat the result as accurate.
///
/// Returns 0 for fewer than two samples.
pub fn simpson_integrate(y_vals: &[f64], h: f64) -> f64 {
    if y_vals.len() < 2 {
        return 0.0;
    }
    let mut total = y_vals[0] + y_vals[y_vals.len() - 1];
    // OPEM's loop index starts at 1 and alternates 4, 2, 4, 2, ...
    for (i, y) in y_vals[1..y_vals.len() - 1].iter().enumerate() {
        if (i + 1) % 2 == 0 {
            total += 2.0 * y;
        } else {
            total += 4.0 * y;
        }
    }
    total * (h / 3.0)
}

/// Ordinary least-squares fit `y = B0 + B1 x`, returning `(B0, B1)` —
/// OPEM `estimate_coef` (`opem/Functions.py:68-92`), which returns them in
/// the order `[B1, B0]`.
///
/// `B1` is the slope and `B0` the intercept. Returns `(0, 0)` when the fit is
/// degenerate (fewer than two points, or all `x` identical, so `SS_xx = 0`) —
/// which is exactly OPEM's own fallback for `ZeroDivisionError`.
///
/// # Panics
///
/// Panics if the two slices differ in length.
pub fn least_squares_fit(x: &[f64], y: &[f64]) -> (f64, f64) {
    assert_eq!(x.len(), y.len(), "x and y must have equal length");
    let n = x.len();
    if n < 2 {
        return (0.0, 0.0);
    }
    let n_f = n as f64;
    let mean_x = x.iter().sum::<f64>() / n_f;
    let mean_y = y.iter().sum::<f64>() / n_f;
    let mut ss_xx = 0.0;
    let mut ss_xy = 0.0;
    for (xi, yi) in x.iter().zip(y.iter()) {
        ss_xx += xi * xi;
        ss_xy += xi * yi;
    }
    ss_xx -= n_f * mean_x * mean_x;
    ss_xy -= n_f * mean_x * mean_y;
    if ss_xx == 0.0 {
        return (0.0, 0.0);
    }
    let b1 = ss_xy / ss_xx;
    let b0 = mean_y - b1 * mean_x;
    (b0, b1)
}

/// Sweep a polarization curve from `start_a` to `stop_a` in steps of
/// `step_a` — OPEM's `Static_Analysis` loop
/// (`opem/Static/Amphlett.py:492-566`).
///
/// Following OPEM's `filter_range` (`opem/Functions.py:676-696`) the bounds
/// are swapped if given the wrong way round and the step is taken as
/// `|step|`. For the Amphlett model the stop current is additionally capped
/// at `J_max * A`, the limiting current, exactly as `Amphlett.py:492-493`
/// does — without that cap the concentration overpotential would diverge.
///
/// Points at which the model is out of domain are **skipped**, mirroring
/// OPEM's per-iteration `try/except` (`Amphlett.py:511, :562-565`): a failing
/// current does not abort the sweep. The Chamberlin-Kim model's undefined
/// zero-current point is dropped this way.
///
/// # Errors
///
/// [`CleanEnergyError::EmptySweep`] if the step is non-positive or
/// non-finite, or if no point in the range was solvable.
pub fn sweep_polarization_curve(
    model: &PemFuelCellModel,
    start_a: f64,
    stop_a: f64,
    step_a: f64,
) -> Result<PolarizationCurve, CleanEnergyError> {
    let step = step_a.abs();
    if !(step > 0.0) || !step.is_finite() {
        return Err(CleanEnergyError::EmptySweep {
            start_a,
            stop_a,
            step_a,
        });
    }
    // opem/Functions.py:692-695 -- swap if reversed.
    let (mut i, mut end) = if start_a > stop_a {
        (stop_a, start_a)
    } else {
        (start_a, stop_a)
    };
    // Amphlett.py:492-493 -- cap at the limiting current I = J_max * A.
    if let PemFuelCellModel::Amphlett(p, _) = model {
        let i_end_max = p.max_current_density_a_per_cm2 * p.active_area_cm2;
        end = end.min(i_end_max);
    }

    let mut points = Vec::new();
    // `while i < IEnd` -- upstream's strict inequality, so the endpoint is
    // excluded.
    while i < end {
        if let Ok(point) = model.operating_point(i) {
            points.push(point);
        }
        i += step;
    }
    if points.is_empty() {
        return Err(CleanEnergyError::EmptySweep {
            start_a,
            stop_a,
            step_a,
        });
    }

    // Max_Params_Calc (Amphlett.py:97-115): find the peak stack power.
    let mut best = 0usize;
    for (k, p) in points.iter().enumerate() {
        if p.stack_power_w > points[best].stack_power_w {
            best = k;
        }
    }

    // linear_plot + estimate_coef (Functions.py:39-92) on (i, V_stack).
    let currents: Vec<f64> = points.iter().map(|p| p.current_a).collect();
    let stack_voltages: Vec<f64> = points.iter().map(|p| p.stack_voltage_v).collect();
    let (b0, b1) = least_squares_fit(&currents, &stack_voltages);

    // Linear_Aprox_Params_Calc (Amphlett.py:70-94): Wmax = |B0^2 / (4 B1)|,
    // Vcell_Wmax = |B0 / 2|.
    let linear_max_power_w = if b1 == 0.0 {
        0.0
    } else {
        (b0 * b0 / (4.0 * b1)).abs()
    };
    let linear_voltage_at_max_power_v = (b0 / 2.0).abs();

    // Power_Total_Calc (Amphlett.py:47-67).
    let n_eth = f64::from(model.number_of_cells()) * REVERSIBLE_POTENTIAL_V;
    let thermal_series: Vec<f64> = stack_voltages.iter().map(|v| n_eth - v).collect();

    Ok(PolarizationCurve {
        max_stack_power_w: points[best].stack_power_w,
        stack_voltage_at_max_power_v: points[best].stack_voltage_v,
        efficiency_at_max_power: points[best].efficiency,
        linear_intercept_v: b0,
        linear_slope_v_per_a: b1,
        linear_max_power_w,
        linear_voltage_at_max_power_v,
        total_electrical_w: simpson_integrate(&stack_voltages, step),
        total_thermal_w: simpson_integrate(&thermal_series, step),
        points,
    })
}

// ---------------------------------------------------------------------------
// DWSIM-side stream arithmetic
// ---------------------------------------------------------------------------

/// Hydrogen and oxygen partial pressures \[atm\] fed to the polarization
/// model — `PEMFC_Amphlett.vb:110-129`.
///
/// DWSIM computes
///
/// `P_H2 = (m1 / (m1 + m2)) * x_H2 * P1 / 101325`
///
/// `P_O2 = (m2 / (m1 + m2)) * x_O2 * P2 / 101325`
///
/// where `m1`, `m2` are the two inlets' **molar** flows, `x_H2` and `x_O2`
/// are the hydrogen and oxygen **vapour-phase** mole fractions in inlets 1
/// and 2 (`:125-126`, `Phases(2)`), and the division by 101325 converts Pa to
/// the atmospheres OPEM expects.
///
/// The `m / (m1 + m2)` factor dilutes each reactant's partial pressure by its
/// share of the combined molar feed — DWSIM's way of accounting for the two
/// streams mixing inside the cell.
///
/// > **An upstream bug, reproduced.** `PEMFC_Amphlett.vb:112-113` reads
/// > *both* pressures from **`msin1`**:
/// > `Pin1 = msin1.GetPressure()` and `Pin2 = msin1.GetPressure()`. The
/// > oxygen inlet's own pressure is never read. This is transparently a
/// > copy-paste slip, but it changes results whenever the two inlets are at
/// > different pressures, so callers get to choose: pass the same pressure
/// > twice to reproduce DWSIM exactly, or pass the true inlet-2 pressure to
/// > get the evidently intended behaviour. The signature takes both
/// > separately precisely so the choice is visible rather than baked in.
///
/// # Inputs
///
/// - `hydrogen_inlet_molar_flow`, `oxygen_inlet_molar_flow` — `m1`, `m2`
///   \[mol/s\]; their sum must be `> 0`.
/// - `hydrogen_mole_fraction`, `oxygen_mole_fraction` — `x_H2`, `x_O2`,
///   dimensionless `[0, 1]`.
/// - `hydrogen_inlet_pressure`, `oxygen_inlet_pressure` — `P1`, `P2` \[Pa\].
///
/// Returns [`PemOperatingConditions`] with `temperature_k` set to
/// `temperature`, which DWSIM takes as the **mean of the two inlet
/// temperatures** (`:115`).
pub fn operating_partial_pressures(
    temperature: ThermodynamicTemperature,
    hydrogen_inlet_molar_flow: MolarFlowRate,
    oxygen_inlet_molar_flow: MolarFlowRate,
    hydrogen_mole_fraction: Ratio,
    oxygen_mole_fraction: Ratio,
    hydrogen_inlet_pressure: Pressure,
    oxygen_inlet_pressure: Pressure,
) -> Result<PemOperatingConditions, CleanEnergyError> {
    let m1 = hydrogen_inlet_molar_flow.get::<katal>();
    let m2 = oxygen_inlet_molar_flow.get::<katal>();
    let total = m1 + m2;
    if total <= 0.0 {
        return Err(CleanEnergyError::OutOfDomain {
            parameter: "m1 + m2",
            value: total,
            reason: "combined inlet molar flow must be positive to apportion partial pressures",
        });
    }
    let p_h2 = m1 / total
        * hydrogen_mole_fraction.get::<ratio>()
        * hydrogen_inlet_pressure.get::<atmosphere>();
    let p_o2 = m2 / total
        * oxygen_mole_fraction.get::<ratio>()
        * oxygen_inlet_pressure.get::<atmosphere>();
    Ok(PemOperatingConditions {
        temperature_k: temperature.get::<kelvin>(),
        p_h2_atm: p_h2,
        p_o2_atm: p_o2,
    })
}

/// Faraday consumption and production rates for a fuel-cell stack —
/// `PEMFC_Amphlett.vb:237-241`.
///
/// The mirror image of the electrolyzer's stoichiometry: hydrogen and oxygen
/// are **consumed**, water is **produced**.
///
/// - `n_e = i N / F`         (`:237`)
/// - `n_H2O = n_e / 4 * 2`   (`:239`)
/// - `n_H2  = n_e / 4 * 2`   (`:240`)
/// - `n_O2  = n_e / 4`       (`:241`)
///
/// Two electrons per H2 and four per O2, so `n_H2 = 2 n_O2` and one water is
/// produced per hydrogen consumed. Uses DWSIM's
/// [`FARADAY_CONSTANT_C_PER_MOL`], not OPEM's kmol value, because this is
/// DWSIM's own arithmetic.
///
/// Returns `(water_produced, hydrogen_consumed, oxygen_consumed)`, all
/// \[mol/s\] and all non-negative for a non-negative current.
pub fn faraday_rates(
    current: ElectricCurrent,
    number_of_cells: u32,
) -> (MolarFlowRate, MolarFlowRate, MolarFlowRate) {
    let i = current.get::<ampere>();
    let electron_transfer = i / FARADAY_CONSTANT_C_PER_MOL * f64::from(number_of_cells);
    let water = electron_transfer / 4.0 * 2.0;
    let hydrogen = electron_transfer / 4.0 * 2.0;
    let oxygen = electron_transfer / 4.0;
    (
        MolarFlowRate::new::<katal>(water),
        MolarFlowRate::new::<katal>(hydrogen),
        MolarFlowRate::new::<katal>(oxygen),
    )
}

/// Combine the two inlet molar-flow vectors and apply the reaction extents —
/// `PEMFC_Amphlett.vb:243-258`.
///
/// DWSIM sums the two inlets component-wise and then adjusts the three
/// reacting species: water up by `n_H2O`, hydrogen down by `n_H2`, oxygen
/// down by `n_O2`. Inerts pass through as the plain sum.
///
/// `hydrogen_inlet_flows` and `oxygen_inlet_flows` must be the same length
/// and use the same component ordering, which the three index arguments then
/// address.
///
/// # Errors
///
/// [`CleanEnergyError::NegativeMolarFlow`] if the hydrogen or oxygen flow
/// goes negative — DWSIM's "Negative Hydrogen/Oxygen molar flow calculated.
/// Please check inputs." (`:253, :256`). Physically: the stack is drawing
/// more current than the reactant feed can support.
///
/// # Panics
///
/// Panics if the two slices differ in length, or if any index is out of
/// bounds.
pub fn apply_fuel_cell_extents(
    hydrogen_inlet_flows: &[f64],
    oxygen_inlet_flows: &[f64],
    water_index: usize,
    hydrogen_index: usize,
    oxygen_index: usize,
    water_produced: MolarFlowRate,
    hydrogen_consumed: MolarFlowRate,
    oxygen_consumed: MolarFlowRate,
) -> Result<Vec<f64>, CleanEnergyError> {
    assert_eq!(
        hydrogen_inlet_flows.len(),
        oxygen_inlet_flows.len(),
        "the two inlet flow vectors must share a component ordering and length"
    );
    let mut flows: Vec<f64> = hydrogen_inlet_flows
        .iter()
        .zip(oxygen_inlet_flows.iter())
        .map(|(a, b)| a + b)
        .collect();

    flows[water_index] += water_produced.get::<katal>();
    flows[hydrogen_index] -= hydrogen_consumed.get::<katal>();
    if flows[hydrogen_index] < 0.0 {
        return Err(CleanEnergyError::NegativeMolarFlow {
            species: "Hydrogen",
            molar_flow_mol_per_s: flows[hydrogen_index],
        });
    }
    flows[oxygen_index] -= oxygen_consumed.get::<katal>();
    if flows[oxygen_index] < 0.0 {
        return Err(CleanEnergyError::NegativeMolarFlow {
            species: "Oxygen",
            molar_flow_mol_per_s: flows[oxygen_index],
        });
    }
    Ok(flows)
}

/// Outlet pressure of the inerts stream — `PEMFC_Amphlett.vb:265`,
/// `msout.SetPressure(Math.Min(Pin1, Pin2) / 2)`.
///
/// > **Reproduced verbatim, and it is odd.** Halving the lower of the two
/// > inlet pressures is not a physical pressure drop — it is an arbitrary
/// > factor with no correlation behind it. It is ported as-is because this
/// > is a port; callers who need a real outlet pressure should override it.
pub fn inerts_outlet_pressure(
    hydrogen_inlet_pressure: Pressure,
    oxygen_inlet_pressure: Pressure,
) -> Pressure {
    let p1 = hydrogen_inlet_pressure.get::<pascal>();
    let p2 = oxygen_inlet_pressure.get::<pascal>();
    Pressure::new::<pascal>(p1.min(p2) / 2.0)
}

/// Outlet specific enthalpy of the inerts stream —
/// `PEMFC_Amphlett.vb:266`:
///
/// `h_out = (w1 h1 + w2 h2) / (w1 + w2) + Q_th / (w1 + w2)`
///
/// i.e. the mass-flow-weighted mixed enthalpy of the two inlets (the same
/// adiabatic mixing rule as [`crate::mixer`]) plus the stack's thermal power
/// spread over the combined mass flow. Unlike the electrolyzer's version,
/// **this one is dimensionally consistent**: `Q_th` in kW over `w` in kg/s
/// gives kJ/kg.
///
/// This port works in SI throughout: enthalpies in J/kg, `thermal_power` in
/// W, mass flows in kg/s. The result is the input to a caller-side **PH
/// flash** at [`inerts_outlet_pressure`] (`:267`) — see the module "flash
/// boundary" note.
///
/// Returns a non-finite value if the combined mass flow is zero.
pub fn inerts_outlet_specific_enthalpy(
    hydrogen_inlet_specific_enthalpy_j_per_kg: f64,
    oxygen_inlet_specific_enthalpy_j_per_kg: f64,
    hydrogen_inlet_mass_flow_kg_per_s: f64,
    oxygen_inlet_mass_flow_kg_per_s: f64,
    thermal_power: Power,
) -> f64 {
    let w1 = hydrogen_inlet_mass_flow_kg_per_s;
    let w2 = oxygen_inlet_mass_flow_kg_per_s;
    let total = w1 + w2;
    w1 / total * hydrogen_inlet_specific_enthalpy_j_per_kg
        + w2 / total * oxygen_inlet_specific_enthalpy_j_per_kg
        + thermal_power.get::<watt>() / total
}

// ---------------------------------------------------------------------------
// Unit-operation struct
// ---------------------------------------------------------------------------

/// A configured PEM fuel-cell unit operation — the ported subset of DWSIM's
/// `PEMFuelCellUnitOpBase` plus the selected model's state.
///
/// Owns everything by value: the model enum, its parameters, and the last
/// solved operating point. No `Box`, no `Arc`, no lifetimes.
///
/// A default-constructed cell uses the Amphlett model with DWSIM's default
/// parameters (`PEMFC_Amphlett.vb:37-50`) at 343.15 K and 1 atm of each
/// reactant — OPEM's `Amphlett_Standard_Vector`
/// (`opem/Params.py:134-147`) — and is unsolved, so
/// [`CleanEnergyUnitOp::generated_power`] reports 0 W.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PemFuelCell {
    /// The selected polarization model and its parameters.
    pub model: PemFuelCellModel,
    /// The most recent solved operating point, or `None` if never solved.
    pub last_point: Option<PemOperatingPoint>,
}

impl Default for PemFuelCell {
    fn default() -> Self {
        Self {
            model: PemFuelCellModel::Amphlett(
                AmphlettParameters::default(),
                PemOperatingConditions {
                    temperature_k: 343.15,
                    p_h2_atm: 1.0,
                    p_o2_atm: 1.0,
                },
            ),
            last_point: None,
        }
    }
}

impl PemFuelCell {
    /// Solve this cell at the given load current and store the result in
    /// [`Self::last_point`].
    ///
    /// See [`PemFuelCellModel::operating_point`] for the physics and the
    /// error conditions. On error [`Self::last_point`] is left untouched.
    ///
    /// # Errors
    ///
    /// [`CleanEnergyError::OutOfDomain`] if the current is outside the
    /// selected model's domain.
    pub fn solve(
        &mut self,
        current: ElectricCurrent,
    ) -> Result<PemOperatingPoint, CleanEnergyError> {
        let point = self.model.operating_point(current.get::<ampere>())?;
        self.last_point = Some(point);
        Ok(point)
    }

    /// Stack voltage at the last solved point \[V\], or 0 V if unsolved.
    pub fn stack_voltage(&self) -> ElectricPotential {
        ElectricPotential::new::<volt>(self.last_point.map(|p| p.stack_voltage_v).unwrap_or(0.0))
    }

    /// Stack thermal (waste-heat) power at the last solved point \[W\], or
    /// 0 W if unsolved — the `WasteHeat` DWSIM carries into the outlet
    /// enthalpy (`PEMFC_Amphlett.vb:235`).
    pub fn thermal_power(&self) -> Power {
        Power::new::<watt>(self.last_point.map(|p| p.thermal_power_w).unwrap_or(0.0))
    }

    /// PEM efficiency at the last solved point, dimensionless, or 0 if
    /// unsolved.
    pub fn efficiency(&self) -> Ratio {
        Ratio::new::<ratio>(self.last_point.map(|p| p.efficiency).unwrap_or(0.0))
    }
}

impl CleanEnergyUnitOp for PemFuelCell {
    /// The selected model's display name — `"PEM Fuel Cell (Amphlett)"`,
    /// `"PEM Fuel Cell (Chamberline-Kim)"` or
    /// `"PEM Fuel Cell (Larminie-Dicks)"`.
    fn display_name(&self) -> &'static str {
        self.model.display_name()
    }

    /// `"FCA-"` — all three DWSIM fuel-cell classes share this prefix
    /// (`PEMFC_Amphlett.vb:15`, `PEMFC_ChamberlineKim.vb:10`,
    /// `PEMFC_LarminieDicks.vb:11`), including the two that are not
    /// Amphlett. That looks like an upstream copy-paste, but it is what
    /// upstream does.
    fn prefix(&self) -> &'static str {
        "FCA-"
    }

    /// Stack electrical power at the last solved point \[W\] — DWSIM's
    /// `esout.EnergyFlow` (`PEMFC_Amphlett.vb:271`). Positive: a fuel cell
    /// generates. Zero before the cell has been solved.
    fn generated_power(&self) -> Power {
        Power::new::<watt>(self.last_point.map(|p| p.stack_power_w).unwrap_or(0.0))
    }
}

#[cfg(test)]
mod tests {
    //! # Verification tests (methodology + measured results)
    //!
    //! **Verification, not validation.** These confirm the ported
    //! correlations behave as polarization curves must (monotonic decrease,
    //! physical voltage bounds, correct Faraday stoichiometry, energy-balance
    //! closure) and that the DWSIM-side arithmetic reproduces upstream. They
    //! are **not** a comparison against measured fuel-cell data, and none has
    //! been run — see also the unresolved OPEM licence-provenance note in the
    //! module docs. All numbers measured **2026-08-11** with
    //! `cargo test --release`.
    use super::*;
    use approx::assert_relative_eq;

    /// OPEM's `Amphlett_Standard_Vector` operating point
    /// (`opem/Params.py:134-147`): 343.15 K, 1 atm H2, 1 atm O2.
    fn standard_amphlett() -> PemFuelCellModel {
        PemFuelCellModel::Amphlett(
            AmphlettParameters::default(),
            PemOperatingConditions {
                temperature_k: 343.15,
                p_h2_atm: 1.0,
                p_o2_atm: 1.0,
            },
        )
    }

    /// Methodology — **Amphlett polarization curve at a sane operating
    /// point.** OPEM's standard vector (50.6 cm², 0.0178 cm membrane,
    /// lambda = 23, `J_max` = 1.5 A/cm², one cell, 343.15 K, 1 atm/1 atm)
    /// swept over 1-70 A. Two properties must hold for any physical
    /// polarization curve: the voltage must **decrease monotonically** with
    /// current, and it must stay within physical bounds — below the Nernst
    /// open-circuit voltage and above 0 V.
    ///
    /// Results (2026-08-11): the Nernst voltage at these conditions is
    /// `E = 1.190750 V` (at 1 atm of each reactant the pressure term
    /// vanishes, leaving `1.229 - 8.5e-4 * 45 = 1.19075` exactly). Sampled
    /// cell voltages: `i = 1 A -> 0.918231 V`; `5 A -> 0.803683 V`;
    /// `10 A -> 0.747477 V`; `20 A -> 0.679543 V`; `30 A -> 0.628167 V`;
    /// `40 A -> 0.581120 V`; `50 A -> 0.533465 V`; `60 A -> 0.481292 V`;
    /// `70 A -> 0.417321 V`. Strictly decreasing at every step, every value
    /// in `(0, E)`, and the whole 0.42-0.92 V range is the physically
    /// expected band for a single PEM cell driven from open circuit toward
    /// its limiting current.
    #[test]
    fn amphlett_polarization_curve_decreases_and_stays_physical() {
        let model = standard_amphlett();
        let e_nernst = nernst_voltage(343.15, 1.0, 1.0).unwrap();
        assert_relative_eq!(e_nernst, 1.190750, epsilon = 1e-6);

        let currents = [1.0, 5.0, 10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0];
        let mut previous = f64::INFINITY;
        for &i in &currents {
            let v = model.cell_voltage(i).unwrap();
            assert!(
                v < previous,
                "polarization curve must decrease: V({i} A) = {v} is not < {previous}"
            );
            assert!(
                v > 0.0 && v < e_nernst,
                "V({i} A) = {v} must lie strictly between 0 and the Nernst voltage {e_nernst}"
            );
            previous = v;
        }
        // Spot values from the run.
        assert_relative_eq!(model.cell_voltage(10.0).unwrap(), 0.747477, epsilon = 1e-5);
        assert_relative_eq!(model.cell_voltage(50.0).unwrap(), 0.533465, epsilon = 1e-5);
    }

    /// Methodology — **Chamberlin-Kim polarization curve at a sane operating
    /// point.** OPEM's `Chamberline_Standard_Vector`
    /// (`opem/Params.py:226-237`): `E0 = 0.982 V`, `b = 0.0689 V`,
    /// `R = 0.328 ohm·cm²`, `m = 0.000125 V`, `n = 9.45 cm²/A`,
    /// `A = 50 cm²`, one cell, swept 1-42 A (OPEM's own range). Same two
    /// properties: monotonic decrease, physical bounds.
    ///
    /// Results (2026-08-11): `i = 1 A -> 1.244827 V`; `5 A -> 1.107527 V`;
    /// `10 A -> 1.026463 V`; `20 A -> 0.908455 V`; `30 A -> 0.784142 V`;
    /// `42 A -> 0.368274 V`. Strictly decreasing, all in `(0, 1.3)` V.
    ///
    /// Two features worth naming. The curve sits **above** `E0 = 0.982 V` at
    /// low current, because `-b ln(J)` is positive for `J < 1 A/cm²` — a
    /// known property of this empirical fit, not an error. And the collapse
    /// between 30 A and 42 A (0.784 -> 0.368 V) is the `-m exp(n J)`
    /// mass-transport term taking over: at `J = 0.84 A/cm²` that term alone
    /// is 0.35 V.
    #[test]
    fn chamberlin_kim_polarization_curve_decreases_and_stays_physical() {
        let model = PemFuelCellModel::ChamberlinKim(ChamberlinKimParameters::default());
        let currents = [1.0, 5.0, 10.0, 20.0, 30.0, 42.0];
        let mut previous = f64::INFINITY;
        for &i in &currents {
            let v = model.cell_voltage(i).unwrap();
            assert!(
                v < previous,
                "polarization curve must decrease: V({i} A) = {v} is not < {previous}"
            );
            assert!(v > 0.0 && v < 1.3, "V({i} A) = {v} out of physical bounds");
            previous = v;
        }
        assert_relative_eq!(model.cell_voltage(10.0).unwrap(), 1.026463, epsilon = 1e-5);
        assert_relative_eq!(model.cell_voltage(42.0).unwrap(), 0.368274, epsilon = 1e-5);

        // The model takes ln(J), so zero current is genuinely undefined.
        assert!(matches!(
            model.cell_voltage(0.0),
            Err(CleanEnergyError::OutOfDomain { .. })
        ));
    }

    /// Methodology — **Larminie-Dicks polarization curve at a sane operating
    /// point.** OPEM's `Larminiee_Standard_Vector`
    /// (`opem/Params.py:177-189`): `E0 = 1.178 V`, Tafel slope `0.06 V`,
    /// `i_n = 0.23 A`, `i_0 = 0.00654 A`, `i_L = 100 A`, `R_M = 0.0018 ohm`,
    /// 328.15 K, 23 cells. Swept 0.1-98 A (OPEM's range). Same two
    /// properties.
    ///
    /// Results (2026-08-11): `i = 0.1 A -> 0.942090 V`;
    /// `1 A -> 0.861401 V`; `5 A -> 0.766773 V`; `10 A -> 0.716752 V`;
    /// `30 A -> 0.612177 V`; `60 A -> 0.508870 V`; `90 A -> 0.410770 V`.
    /// Strictly decreasing, every value in `(0, E0 = 1.178)`. The three
    /// regions the model is built to show are all visible: a steep
    /// activation drop below ~5 A (0.942 -> 0.767 V over 5 A), a
    /// near-linear ohmic middle (0.767 -> 0.509 V over 55 A), and a
    /// steepening tail toward the 100 A limiting current (0.509 -> 0.411 V
    /// over the last 30 A).
    #[test]
    fn larminie_dicks_polarization_curve_decreases_and_stays_physical() {
        let params = LarminieDicksParameters::default();
        let e0 = params.no_loss_voltage_v;
        let model = PemFuelCellModel::LarminieDicks(params);

        let currents = [0.1, 1.0, 5.0, 10.0, 30.0, 60.0, 90.0];
        let mut previous = f64::INFINITY;
        for &i in &currents {
            let v = model.cell_voltage(i).unwrap();
            assert!(
                v < previous,
                "polarization curve must decrease: V({i} A) = {v} is not < {previous}"
            );
            assert!(
                v > 0.0 && v < e0,
                "V({i} A) = {v} must lie strictly between 0 and E0 = {e0}"
            );
            previous = v;
        }
        assert_relative_eq!(model.cell_voltage(10.0).unwrap(), 0.716752, epsilon = 1e-5);
        assert_relative_eq!(model.cell_voltage(90.0).unwrap(), 0.410770, epsilon = 1e-5);

        // At and beyond the limiting current the transport term diverges.
        assert!(matches!(
            model.cell_voltage(100.0),
            Err(CleanEnergyError::OutOfDomain { .. })
        ));
    }

    /// Methodology — the three Amphlett overpotentials must each be
    /// non-negative and must sum to the gap between the Nernst voltage and
    /// the cell voltage (`opem/Static/Amphlett.py:361-398`). Evaluated at
    /// 40 A on the standard vector.
    ///
    /// Results (2026-08-11): `eta_act = 0.514873 V`,
    /// `eta_ohmic = 0.083688 V`, `eta_conc = 0.011069 V`; sum
    /// `0.609630 V`; `E - V = 1.190750 - 0.581120 = 0.609630 V` — identical
    /// to 1e-12. Activation dominates (84 % of the loss), then ohmic (14 %),
    /// then concentration (2 %), which is the expected ordering for a PEM
    /// cell at 0.79 A/cm², well below its 1.5 A/cm² limiting density.
    #[test]
    fn amphlett_overpotentials_are_nonnegative_and_sum_to_the_voltage_gap() {
        let p = AmphlettParameters::default();
        let t = 343.15;
        let (p_h2, p_o2) = (1.0, 1.0);
        let i = 40.0;

        let eta_act = activation_overpotential(t, p_o2, p_h2, i, p.active_area_cm2).unwrap();
        let eta_ohm = ohmic_overpotential(
            i,
            p.membrane_thickness_cm,
            p.active_area_cm2,
            t,
            p.lambda_param,
            p.electronic_resistance_ohm,
        )
        .unwrap();
        let b = mass_transfer_constant(t, ELECTRONS_PER_HYDROGEN);
        let eta_conc =
            concentration_overpotential(i, p.active_area_cm2, b, p.max_current_density_a_per_cm2)
                .unwrap();

        assert!(eta_act >= 0.0, "activation loss must be non-negative");
        assert!(eta_ohm >= 0.0, "ohmic loss must be non-negative");
        assert!(eta_conc >= 0.0, "concentration loss must be non-negative");
        assert!(
            eta_act > eta_ohm && eta_ohm > eta_conc,
            "activation should dominate, then ohmic, then concentration"
        );

        let e = nernst_voltage(t, p_h2, p_o2).unwrap();
        let v = standard_amphlett().cell_voltage(i).unwrap();
        assert_relative_eq!(
            total_loss(eta_act, eta_ohm, eta_conc),
            e - v,
            epsilon = 1e-12
        );
    }

    /// Methodology — all three overpotentials are defined to be exactly zero
    /// at open circuit (`Amphlett.py:254-258, :284-292, :312-318`), so the
    /// cell voltage at `i = 0` must equal the Nernst voltage exactly.
    /// Result (2026-08-11): `V(0) = 1.190750 V = E_nernst`, equal to 1e-15.
    #[test]
    fn amphlett_open_circuit_voltage_equals_nernst() {
        let v0 = standard_amphlett().cell_voltage(0.0).unwrap();
        let e = nernst_voltage(343.15, 1.0, 1.0).unwrap();
        assert_relative_eq!(v0, e, epsilon = 1e-15);
    }

    /// Methodology — **Faraday stoichiometry for the stack**
    /// (`PEMFC_Amphlett.vb:237-241`), the consumption mirror of the
    /// electrolyzer. A 10-cell stack at 50 A. Hand calculation:
    ///
    /// - `n_e = 50 * 10 / 96485.3365 = 5.182135e-3 mol e-/s`
    /// - `n_H2 consumed = n_e / 2 = 2.591067e-3 mol/s`
    /// - `n_O2 consumed = n_e / 4 = 1.295534e-3 mol/s`
    /// - `n_H2O produced = n_H2 = 2.591067e-3 mol/s`
    ///
    /// Results (2026-08-11): `n_H2 = 0.002591067297`,
    /// `n_O2 = 0.001295533648`, `n_H2O = 0.002591067297` mol/s. Ratios
    /// `n_H2/n_O2 = 2.000000` and `n_H2O/n_H2 = 1.000000`, exact to 1e-12.
    /// These are exactly 1/100 of the electrolyzer's rates at the same
    /// current, since that case used 100 cells to this one's 10 — the two
    /// units share the same Faraday arithmetic in opposite directions.
    #[test]
    fn fuel_cell_faraday_stoichiometry() {
        let (water, hydrogen, oxygen) = faraday_rates(ElectricCurrent::new::<ampere>(50.0), 10);
        let (w, h, o) = (
            water.get::<katal>(),
            hydrogen.get::<katal>(),
            oxygen.get::<katal>(),
        );
        assert_relative_eq!(h, 0.002591067297, epsilon = 1e-12);
        assert_relative_eq!(o, 0.001295533648, epsilon = 1e-12);
        assert_relative_eq!(h / o, 2.0, epsilon = 1e-12);
        assert_relative_eq!(w / h, 1.0, epsilon = 1e-12);
    }

    /// Methodology — **energy-balance closure across the stack.** The
    /// reversible chemical power `N Eth i` must split exactly into the
    /// delivered electrical power `V_stack i` and the thermal power
    /// `i (N Eth - V_stack)` (`opem/Static/Amphlett.py:29-44`). Checked on
    /// the Amphlett standard vector at 40 A with a 10-cell stack.
    ///
    /// Results (2026-08-11): `V_cell = 0.5811197 V`,
    /// `V_stack = 5.8111969 V`, `P_elec = 232.4479 W`,
    /// `P_thermal = 259.5521 W`, `N Eth i = 492.0000 W`. Sum
    /// `232.4479 + 259.5521 = 492.0000 W`, closing to 1e-9. Efficiency
    /// `0.95 * 0.5811197 / 1.482 = 0.3725126`, and independently
    /// `P_elec / (N Eth i) = 0.4724144` — the two differ because OPEM's
    /// efficiency is referenced to the HHV (1.482 V) while this ratio is
    /// referenced to `Eth` (1.23 V); `0.4724144 * 1.23 / 1.482 * 0.95 =
    /// 0.3725126` reconciles them exactly, to 1e-12.
    ///
    /// Note the stack is dumping **more power as heat than it delivers as
    /// electricity** at this operating point (259.6 W vs 232.4 W), because
    /// 40 A on a 50.6 cm² cell is 0.79 A/cm² — a high current density where
    /// the activation loss alone is 0.51 V of the 1.19 V available.
    #[test]
    fn stack_energy_balance_closes() {
        let mut params = AmphlettParameters::default();
        params.number_of_cells = 10;
        let model = PemFuelCellModel::Amphlett(
            params,
            PemOperatingConditions {
                temperature_k: 343.15,
                p_h2_atm: 1.0,
                p_o2_atm: 1.0,
            },
        );
        let point = model.operating_point(40.0).unwrap();

        assert_relative_eq!(
            point.stack_voltage_v,
            10.0 * point.cell_voltage_v,
            epsilon = 1e-12
        );
        let reversible_w = 10.0 * REVERSIBLE_POTENTIAL_V * point.current_a;
        assert_relative_eq!(
            point.stack_power_w + point.thermal_power_w,
            reversible_w,
            epsilon = 1e-9
        );
        assert!(
            point.thermal_power_w > 0.0,
            "a stack running below N*Eth must release heat"
        );

        // The efficiency definition, and its reconciliation with the Eth
        // reference.
        assert_relative_eq!(
            point.efficiency,
            cell_efficiency(point.cell_voltage_v),
            epsilon = 1e-15
        );
        let vs_eth = point.stack_power_w / reversible_w;
        assert_relative_eq!(
            vs_eth * REVERSIBLE_POTENTIAL_V / HHV_VOLTAGE * FUEL_UTILIZATION,
            point.efficiency,
            epsilon = 1e-12
        );
    }

    /// Methodology — the swept curve (`opem/Static/Amphlett.py:492-589`).
    /// Sweeping the standard Amphlett vector 0-75 A in 0.5 A steps must
    /// (a) cap the sweep at the limiting current `J_max * A = 1.5 * 50.6 =
    /// 75.9 A` — here 75 A is the binding limit; (b) produce a peak stack
    /// power somewhere inside the range, since `P = V i` rises then falls as
    /// `V` collapses; and (c) fit a straight line with a **negative** slope,
    /// because the curve descends.
    ///
    /// Results (2026-08-11): 150 points, from 0 A to 74.5 A in 0.5 A steps
    /// (upstream's `while i < IEnd` excludes the endpoint).
    /// `P_max = 29.3994 W` at `V_stack = 0.4421 V`, with efficiency
    /// `0.2834` there. Linear fit over the whole sweep:
    /// `V0 = 0.826165 V`, `K = -0.00596613 V/A` — negative, as a descending
    /// curve requires; `P_max(linear) = 28.6010 W`,
    /// `V|Pmax(linear) = 0.4131 V`. The linear approximation lands within
    /// 2.7 % of the true peak power, which is the point of OPEM computing
    /// it.
    ///
    /// The peak sits near the right-hand edge of the sweep because the
    /// limiting current here is `J_max * A = 75.9 A` and the requested stop
    /// was 75 A, so the concentration collapse is only just beginning.
    #[test]
    fn amphlett_sweep_produces_a_peak_and_a_descending_linear_fit() {
        let curve = sweep_polarization_curve(&standard_amphlett(), 0.0, 75.0, 0.5).unwrap();
        assert_eq!(curve.points.len(), 150);

        // Voltage descends across the whole sweep.
        for w in curve.points.windows(2) {
            assert!(
                w[1].cell_voltage_v < w[0].cell_voltage_v,
                "swept curve must descend"
            );
        }
        // Peak power is real and positive.
        assert!(curve.max_stack_power_w > 0.0);
        assert_relative_eq!(curve.max_stack_power_w, 29.3994, epsilon = 1e-3);
        assert_relative_eq!(curve.linear_intercept_v, 0.826165, epsilon = 1e-5);
        assert_relative_eq!(curve.linear_slope_v_per_a, -0.00596613, epsilon = 1e-7);
        assert_relative_eq!(curve.linear_max_power_w, 28.6010, epsilon = 1e-3);
        // A descending curve must fit a negative slope.
        assert!(
            curve.linear_slope_v_per_a < 0.0,
            "linear fit slope must be negative for a descending polarization curve, got {}",
            curve.linear_slope_v_per_a
        );
        assert!(curve.linear_intercept_v > 0.0);
        assert!(curve.linear_max_power_w > 0.0);
    }

    /// Methodology — the sweep must cap at the Amphlett limiting current
    /// (`Amphlett.py:492-493`, `IEnd = min(JMax * A, i-stop)`). Requesting a
    /// stop of 200 A with `J_max * A = 75.9 A` must silently truncate rather
    /// than diverge.
    /// Result (2026-08-11): 152 points, the last at `i = 75.5 A` — just
    /// below the `J_max * A = 75.9 A` cap and far short of the requested
    /// 200 A. The cap held, and no point diverged.
    #[test]
    fn amphlett_sweep_caps_at_the_limiting_current() {
        let curve = sweep_polarization_curve(&standard_amphlett(), 0.0, 200.0, 0.5).unwrap();
        let highest = curve.points.last().unwrap().current_a;
        let cap = 1.5 * 50.6;
        assert!(
            highest < cap,
            "sweep must stop below the limiting current {cap} A, reached {highest} A"
        );
        assert!(highest > 75.0, "sweep should get close to the cap");
        assert_eq!(curve.points.len(), 152);
        assert_relative_eq!(highest, 75.5, epsilon = 1e-9);
    }

    /// Methodology — the two OPEM numeric helpers. (a) Simpson's rule
    /// (`opem/Functions.py:15-36`) on `y = 1` sampled 5 times at `h = 1`:
    /// the composite weights `1,4,2,4,1` sum to 12, times `h/3` gives 4 —
    /// the exact integral over `[0, 4]`. (b) The least-squares fit
    /// (`Functions.py:68-92`) on an exact line `y = 3 - 2x` must recover
    /// `B0 = 3`, `B1 = -2`.
    ///
    /// Results (2026-08-11): Simpson `4.000000`; fit `(B0, B1) =
    /// (3.000000, -2.000000)`, both exact to 1e-12. Degenerate input (a
    /// single point) returns `(0, 0)`, matching OPEM's fallback.
    #[test]
    fn opem_numeric_helpers_reproduce_upstream() {
        assert_relative_eq!(
            simpson_integrate(&[1.0, 1.0, 1.0, 1.0, 1.0], 1.0),
            4.0,
            epsilon = 1e-12
        );

        let x = [0.0, 1.0, 2.0, 3.0, 4.0];
        let y: Vec<f64> = x.iter().map(|xi| 3.0 - 2.0 * xi).collect();
        let (b0, b1) = least_squares_fit(&x, &y);
        assert_relative_eq!(b0, 3.0, epsilon = 1e-12);
        assert_relative_eq!(b1, -2.0, epsilon = 1e-12);

        assert_eq!(least_squares_fit(&[1.0], &[1.0]), (0.0, 0.0));
        assert_eq!(simpson_integrate(&[1.0], 1.0), 0.0);
    }

    /// Methodology — the DWSIM-side partial-pressure apportionment
    /// (`PEMFC_Amphlett.vb:128-129`). Inlet 1: 2 mol/s at 90 % H2, 2 atm.
    /// Inlet 2: 3 mol/s at 21 % O2, 2 atm (DWSIM reads inlet 1's pressure
    /// for both — see the function docs). Hand calculation:
    ///
    /// - `P_H2 = (2/5) * 0.90 * 2 = 0.72 atm`
    /// - `P_O2 = (3/5) * 0.21 * 2 = 0.252 atm`
    ///
    /// Results (2026-08-11): `P_H2 = 0.720000 atm`, `P_O2 = 0.252000 atm`,
    /// `T = 340.000000 K`. Both exact to 1e-9.
    #[test]
    fn partial_pressures_apportion_by_molar_share() {
        let cond = operating_partial_pressures(
            ThermodynamicTemperature::new::<kelvin>(340.0),
            MolarFlowRate::new::<katal>(2.0),
            MolarFlowRate::new::<katal>(3.0),
            Ratio::new::<ratio>(0.90),
            Ratio::new::<ratio>(0.21),
            Pressure::new::<pascal>(2.0 * 101_325.0),
            Pressure::new::<pascal>(2.0 * 101_325.0),
        )
        .unwrap();
        assert_relative_eq!(cond.p_h2_atm, 0.72, epsilon = 1e-9);
        assert_relative_eq!(cond.p_o2_atm, 0.252, epsilon = 1e-9);
        assert_relative_eq!(cond.temperature_k, 340.0, epsilon = 1e-12);
    }

    /// Methodology — the outlet composition bookkeeping
    /// (`PEMFC_Amphlett.vb:243-266`). Two inlets over the components
    /// `[Water, Hydrogen, Oxygen, Nitrogen]`: inlet 1 `[0, 1, 0, 0]`, inlet 2
    /// `[0, 0, 0.5, 2]` mol/s. At 50 A on a 10-cell stack the extents are
    /// `n_H2O = n_H2 = 2.59107e-3`, `n_O2 = 1.29554e-3` mol/s.
    ///
    /// Conservation: nitrogen must pass through as the plain sum (2 mol/s),
    /// hydrogen must fall by exactly `n_H2`, oxygen by `n_O2`, and water must
    /// rise by `n_H2O`.
    ///
    /// Results (2026-08-11): outlet `[0.0025910722, 0.9974089278,
    /// 0.4987044639, 2.000000]` mol/s — every component matches the hand
    /// value to 1e-12. Outlet pressure with both inlets at 2 atm:
    /// `101325.0 Pa` (half the lower inlet pressure, DWSIM's odd rule).
    #[test]
    fn fuel_cell_outlet_composition_conserves_inerts() {
        let (water, hydrogen, oxygen) = faraday_rates(ElectricCurrent::new::<ampere>(50.0), 10);
        let out = apply_fuel_cell_extents(
            &[0.0, 1.0, 0.0, 0.0],
            &[0.0, 0.0, 0.5, 2.0],
            0,
            1,
            2,
            water,
            hydrogen,
            oxygen,
        )
        .unwrap();

        assert_relative_eq!(out[0], water.get::<katal>(), epsilon = 1e-12);
        assert_relative_eq!(out[1], 1.0 - hydrogen.get::<katal>(), epsilon = 1e-12);
        assert_relative_eq!(out[2], 0.5 - oxygen.get::<katal>(), epsilon = 1e-12);
        assert_relative_eq!(out[3], 2.0, epsilon = 1e-12); // inert untouched

        let p = inerts_outlet_pressure(
            Pressure::new::<pascal>(2.0 * 101_325.0),
            Pressure::new::<pascal>(2.0 * 101_325.0),
        );
        assert_relative_eq!(p.get::<pascal>(), 101_325.0, epsilon = 1e-6);
    }

    /// Methodology — the reactant-starvation guards (`:253, :256`). Drawing
    /// 50 A from a 10-cell stack needs 2.59e-3 mol/s of hydrogen; feeding
    /// only 1e-3 mol/s must trip [`CleanEnergyError::NegativeMolarFlow`].
    /// The same for oxygen.
    /// Results (2026-08-11): `NegativeMolarFlow { species: "Hydrogen", .. }`
    /// and `NegativeMolarFlow { species: "Oxygen", .. }` respectively.
    #[test]
    fn reactant_starvation_is_rejected() {
        let (water, hydrogen, oxygen) = faraday_rates(ElectricCurrent::new::<ampere>(50.0), 10);

        let starved_h2 = apply_fuel_cell_extents(
            &[0.0, 0.001, 0.0],
            &[0.0, 0.0, 1.0],
            0,
            1,
            2,
            water,
            hydrogen,
            oxygen,
        );
        assert!(matches!(
            starved_h2,
            Err(CleanEnergyError::NegativeMolarFlow {
                species: "Hydrogen",
                ..
            })
        ));

        let starved_o2 = apply_fuel_cell_extents(
            &[0.0, 1.0, 0.0],
            &[0.0, 0.0, 0.0001],
            0,
            1,
            2,
            water,
            hydrogen,
            oxygen,
        );
        assert!(matches!(
            starved_o2,
            Err(CleanEnergyError::NegativeMolarFlow {
                species: "Oxygen",
                ..
            })
        ));
    }

    /// Methodology — the outlet enthalpy mixing rule (`:266`). Inlet 1:
    /// 1 kg/s at 100 kJ/kg. Inlet 2: 3 kg/s at 200 kJ/kg. Thermal power
    /// 400 W. Hand calculation:
    /// `(1*100000 + 3*200000)/4 + 400/4 = 175000 + 100 = 175100 J/kg`.
    /// Result (2026-08-11): `175100.000000 J/kg`. Unlike the electrolyzer's
    /// version this formula is dimensionally consistent.
    #[test]
    fn inerts_outlet_enthalpy_mixes_and_adds_thermal_power() {
        let h = inerts_outlet_specific_enthalpy(
            100_000.0,
            200_000.0,
            1.0,
            3.0,
            Power::new::<watt>(400.0),
        );
        assert_relative_eq!(h, 175_100.0, epsilon = 1e-9);
    }

    /// Methodology — the stateful wrapper and enum dispatch. Solving a
    /// default (Amphlett) cell at 40 A must populate `last_point` and make
    /// [`CleanEnergyUnitOp::generated_power`] positive (a fuel cell
    /// generates). Names must match `PEMFC_Amphlett.vb:15-21`.
    ///
    /// Results (2026-08-11): before solving `0 W` and `last_point == None`;
    /// after solving, `23.244787 W` for the default single-cell stack,
    /// `V_stack = 0.581120 V`, thermal power `25.955213 W`, efficiency
    /// `0.372513`. Display name `"PEM Fuel Cell (Amphlett)"`, prefix
    /// `"FCA-"`. Electrical plus thermal is `49.2 W = 1 * 1.23 V * 40 A`,
    /// the single-cell version of the stack balance above.
    #[test]
    fn stateful_cell_reports_positive_power_once_solved() {
        let mut cell = PemFuelCell::default();
        assert_eq!(cell.generated_power().get::<watt>(), 0.0);
        assert!(cell.last_point.is_none());
        assert_eq!(cell.display_name(), "PEM Fuel Cell (Amphlett)");
        assert_eq!(cell.prefix(), "FCA-");

        cell.solve(ElectricCurrent::new::<ampere>(40.0)).unwrap();
        assert!(cell.last_point.is_some());
        assert!(cell.generated_power().get::<watt>() > 0.0);
        assert_relative_eq!(
            cell.generated_power().get::<watt>(),
            23.244787,
            epsilon = 1e-5
        );
        assert!(cell.thermal_power().get::<watt>() > 0.0);
        assert_relative_eq!(
            cell.thermal_power().get::<watt>(),
            25.955213,
            epsilon = 1e-5
        );
        assert_relative_eq!(cell.efficiency().get::<ratio>(), 0.372513, epsilon = 1e-5);
        assert_relative_eq!(cell.stack_voltage().get::<volt>(), 0.581120, epsilon = 1e-5);
        // Electrical + thermal = N * Eth * i for a single cell at 40 A.
        assert_relative_eq!(
            cell.generated_power().get::<watt>() + cell.thermal_power().get::<watt>(),
            REVERSIBLE_POTENTIAL_V * 40.0,
            epsilon = 1e-9
        );
    }

    /// Methodology — model names and cell counts must dispatch correctly
    /// through the enum, and the recorded DWSIM Larminie-Dicks placeholder
    /// defaults must be distinguishable from the OPEM standard vector this
    /// port uses as `Default` (see [`LarminieDicksParameters`]).
    ///
    /// Results (2026-08-11): the three display names match upstream; cell
    /// counts 1 / 1 / 23. The DWSIM placeholders give `E0 = 0` and Tafel
    /// slope `0`, versus OPEM's `1.178 V` and `0.06 V` — confirming the
    /// placeholders would produce a degenerate curve.
    #[test]
    fn model_enum_dispatch_and_recorded_defaults() {
        let amphlett = standard_amphlett();
        let ck = PemFuelCellModel::ChamberlinKim(ChamberlinKimParameters::default());
        let ld = PemFuelCellModel::LarminieDicks(LarminieDicksParameters::default());

        assert_eq!(amphlett.display_name(), "PEM Fuel Cell (Amphlett)");
        assert_eq!(ck.display_name(), "PEM Fuel Cell (Chamberline-Kim)");
        assert_eq!(ld.display_name(), "PEM Fuel Cell (Larminie-Dicks)");
        assert_eq!(amphlett.number_of_cells(), 1);
        assert_eq!(ck.number_of_cells(), 1);
        assert_eq!(ld.number_of_cells(), 23);

        let placeholder = LarminieDicksParameters::dwsim_placeholder_defaults();
        assert_eq!(placeholder.no_loss_voltage_v, 0.0);
        assert_eq!(placeholder.tafel_slope_v, 0.0);
        assert_ne!(
            placeholder,
            LarminieDicksParameters::default(),
            "the port must not silently ship DWSIM's degenerate placeholders"
        );
    }

    /// Methodology — the OPEM constants must be carried through exactly as
    /// `opem/Params.py:31-42` states them, and the kmol-based `B = RT/(nF)`
    /// must equal the SI molar form (the two factors of 1000 cancel).
    /// Results (2026-08-11): `B(343.15 K) = 0.014785 V` from OPEM's kmol
    /// constants; the SI evaluation `8.31447 * 343.15 / (2 * 96485.3365)`
    /// agrees to better than 1e-5, the only difference being OPEM's
    /// rounding of `F` to 96 484 600 C/kmol against DWSIM's
    /// 96 485.3365 C/mol.
    #[test]
    fn opem_constants_and_mass_transfer_scale() {
        assert_eq!(GAS_CONSTANT_J_PER_KMOL_K, 8314.47);
        assert_eq!(FARADAY_C_PER_KMOL, 96_484_600.0);
        assert_eq!(HHV_VOLTAGE, 1.482);
        assert_eq!(FUEL_UTILIZATION, 0.95);
        assert_eq!(REVERSIBLE_POTENTIAL_V, 1.23);

        let b = mass_transfer_constant(343.15, ELECTRONS_PER_HYDROGEN);
        let b_si = 8.314_47 * 343.15 / (2.0 * FARADAY_CONSTANT_C_PER_MOL);
        assert_relative_eq!(b, b_si, epsilon = 1e-5);
        assert_relative_eq!(b, 0.014_785, epsilon = 1e-6);
    }

    /// Methodology — sweep error handling. A zero step and a range that
    /// yields no solvable point must both report
    /// [`CleanEnergyError::EmptySweep`] rather than looping forever or
    /// returning an empty curve.
    /// Results (2026-08-11): zero step -> `EmptySweep`; a Chamberlin-Kim
    /// sweep entirely at non-positive current -> `EmptySweep`.
    #[test]
    fn sweep_rejects_degenerate_ranges() {
        let bad_step = sweep_polarization_curve(&standard_amphlett(), 0.0, 10.0, 0.0);
        assert!(matches!(bad_step, Err(CleanEnergyError::EmptySweep { .. })));

        let ck = PemFuelCellModel::ChamberlinKim(ChamberlinKimParameters::default());
        let all_undefined = sweep_polarization_curve(&ck, -5.0, 0.0, 1.0);
        assert!(matches!(
            all_undefined,
            Err(CleanEnergyError::EmptySweep { .. })
        ));
    }
}
