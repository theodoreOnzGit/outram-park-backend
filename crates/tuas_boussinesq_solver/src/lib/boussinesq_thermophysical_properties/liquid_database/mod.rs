//! Liquid coolant thermophysical-property correlations.
//!
//! Each submodule holds the temperature-dependent property correlations for
//! one liquid coolant in the TUAS database: density (kg/m^3), dynamic
//! viscosity (Pa·s), thermal conductivity (W/(m·K)), constant-pressure
//! specific heat capacity (J/(kg·K)), specific enthalpy (J/kg), and the
//! inverse enthalpy-to-temperature map. Every correlation is range-checked
//! against its coded validity window (in kelvin or degrees Celsius) and
//! returns a `uom`-typed quantity.
//!
//! Fluids covered: Dowtherm A / Therminol VP-1 (`dowtherm_a`), HITEC nitrate
//! salt (`hitec_nitrate_salt`), YD-325 heat-transfer oil
//! (`yd_325_heat_transfer_oil`), FLiBe (`flibe`), and FLiNaK (`flinak`).
//! **CORRECTED 2026-11-10:** that list is incomplete; the database also
//! holds `CustomLiquid` constructors it omitted: the MSRE fuel
//! salt (`msre_fuel_salt_tm0728`), the MSRE coolant salt
//! (`msre_coolant_salt_tm2316`), and, new on that date, liquid sodium
//! (`liquid_sodium_coolprop`, gh:#1320).
//! `custom_liquid_material` provides generic, user-supplied-correlation
//! helpers for a liquid not otherwise in the database.

/// property correlations for dowtherm_a,
/// also known as therminol vp1
pub mod dowtherm_a;

/// property correlations for hitec (a nitrate salt)
/// Du, Bao-Cun, et al. "Investigation on
/// heat transfer characteristics of molten salt in a
/// shell-and-tube heat exchanger." International Communications
/// in Heat and Mass Transfer 96 (2018): 61-68.
pub mod hitec_nitrate_salt;

/// property correlations for YD-325 heat transfer oil
/// Du, Bao-Cun, et al. "Investigation on
/// heat transfer characteristics of molten salt in a
/// shell-and-tube heat exchanger." International Communications
/// in Heat and Mass Transfer 96 (2018): 61-68.
///
///
/// Qiu, Y., Li, M. J., Wang, W. Q., Du, B. C., & Wang, K. (2018).
/// An experimental study on the heat transfer performance of a prototype
/// molten-salt rod baffle heat exchanger for concentrated solar power.
/// Energy, 156, 63-72.
pub mod yd_325_heat_transfer_oil;

/// FLiBe,
///
/// Composition:
/// LiF 67 mol%
/// BeF2 33 mol%
///
/// Viscosity correlations for FLiBe may differ in composition slightly,
/// due to different compositions of FLiBe used for different ranges
///
/// Thermal conductivity is in the range of 1.1 W/(m K) in 873K to 1073K,
/// and Sohal's correlation was originally for 500-650 K
/// Which is strangely below the melting
/// point of flibe
/// but based on Romatoski's data, I found that Romatoski's data
/// could fit Sohal's correlation to within 10% error
/// even up to 1123 K in my PhD Thesis
///
/// Therefore I could use it in the whole temperature range from
/// 500 - 1123K .
///
///
/// Ong, T. K. C. (2024). Digital Twins as Testbeds for
/// Iterative Simulated Neutronics Feedback Controller
/// Development (Doctoral dissertation, UC Berkeley).
///
///
///
/// Romatoski, R. R., & Hu, L. W. (2017). Fluoride salt coolant properties
/// for nuclear reactor applications: A review. Annals
/// of Nuclear Energy, 109, 635-647.
///
/// Sohal, M. S., Ebner, M. A., Sabharwall, P., & Sharpe, P. (2010).
/// Engineering database of liquid salt thermophysical and thermochemical
/// properties (No. INL/EXT-10-18297). Idaho National Lab.(INL),
/// Idaho Falls, ID (United States).

pub mod flibe;

/// MSRE U-235 fuel salt (LiF-BeF2-ZrF4-UF4 65-29.1-5-0.9), ORNL-TM-0728
/// Table 2.1 design values, constant over the 1175-1225 F fuel band
/// (MSRE TH-6, gh:#1133). A `CustomLiquid`; see the module doc.
pub mod msre_fuel_salt_tm0728;

/// MSRE coolant and flush salt LiF-BeF2 66-34 ("L2B"), ORNL-TM-2316
/// correlations, 458-800 C (MSRE TH-6, gh:#1133). A `CustomLiquid`.
pub mod msre_coolant_salt_tm2316;

/// FLiNaK
/// 46.5-11.5-42.0 mol% of LiF, NaF and KF respectively,
/// melting temperature is commonly in literature
/// 454 C, though 462 C is a safer (more conservative) bet
///
/// Romatoski, R. R., & Hu, L. W. (2017). Fluoride salt coolant properties
/// for nuclear reactor applications: A review. Annals
/// of Nuclear Energy, 109, 635-647.
///
/// Sohal, M. S., Ebner, M. A., Sabharwall, P., & Sharpe, P. (2010).
/// Engineering database of liquid salt thermophysical and thermochemical
/// properties (No. INL/EXT-10-18297). Idaho National Lab.(INL),
/// Idaho Falls, ID (United States).
///
///
///
pub mod flinak;

/// Liquid sodium, 400-1000 K, from CoolProp's `LiqNa` fit through the
/// CoolProp port (gh:#1320, EBR-II groundwork #1312). A `CustomLiquid`:
/// use `Material::Liquid(liquid_sodium_coolprop::liquid_sodium())`.
pub mod liquid_sodium_coolprop;

/// properties for a custom liquid material
/// not covered in the database
/// You'll need to define your own functions for this to work
pub mod custom_liquid_material;
