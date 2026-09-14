//! The **pseudo-component** produced by characterization — a narrow-boiling
//! petroleum cut promoted to a first-class thermodynamic compound — and the
//! correlation-selection enums that decide how its constants are estimated.
//!
//! # Provenance
//!
//! The assembly routine [`build_pseudo_component`] is the shared body of the
//! two upstream loops that turn a `(Tb, SG, MW)` triple into a
//! `ConstantProperties` record, from DWSIM (GPL-3.0), pinned upstream clone
//! `/home/teddy0/Documents/research/dwsim-upstream`, branch `windows`, commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream copyright: Daniel
//! Wagner O. de Medeiros and the DWSIM contributors. GPL-3.0; this port is
//! GPL-3.0-only.
//!
//! - `DWSIM.Thermodynamics/PetroleumCharacterization/GenerateCompounds.vb:260-377`
//!   (the bulk path's per-cut block).
//! - `DWSIM.UI.Desktop.Editors/Compounds/DistCurves.cs:664-771`
//!   (the curve path's per-cut block). Only the *algorithm* is taken from that
//!   file; every Eto.Forms widget read, message box and unit-of-measure
//!   conversion around it is excluded — see
//!   [`crate::petroleum::curve_characterization`].
//!
//! The two upstream blocks are near-identical; they are unified here into one
//! documented function, with the small differences (naming prefix, the
//! `0.33333`-versus-`1/3` Watson exponent) called out below.
//!
//! # Units
//!
//! `uom`-typed on the public surface. [`PseudoComponent::component`] is the
//! crate's own [`Component`], whose fields are documented raw SI `f64`
//! (kg/mol, K, Pa, m³/mol) — see [`crate::thermo::component`].
//!
//! # Excluded DWSIM behavior
//!
//! - The integer `ConstantProperties.ID` assigned from a random seed
//!   (`GenerateCompounds.vb:254-255`, `:362`; `DistCurves.cs:349`, `:767`) is
//!   not reproduced — a random identity would make the output
//!   non-deterministic, which the workspace's reproducibility expectations
//!   rule out. [`PseudoComponent::cas_number`] and the compound name carry the
//!   deterministic identity instead.
//! - `OriginalDB` / `CurrentDB` provenance strings (`:268-269`) and the
//!   `IsPF` / `PetroleumFraction` marker flags (`:319`, `:372`) are dropped;
//!   being a [`PseudoComponent`] *is* the marker.
//! - The `Double.TryParse`-based NaN sweep over the stringified results
//!   (`GenerateCompounds.vb:475-483`) is replaced by typed validation in
//!   [`build_pseudo_component`], which returns
//!   [`PseudoComponentError::NonPhysical`] instead of throwing on a formatted
//!   string.

use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    AvailableEnergy, KinematicViscosity, MolarMass, MolarVolume, Ratio, ThermodynamicTemperature,
};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::molar_energy::joule_per_mole;
use uom::si::molar_mass::gram_per_mole;
use uom::si::molar_volume::cubic_meter_per_mole;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

use crate::thermo::component::Component;

use super::aux_props::{
    critical_compressibility_zc1, critical_volume, heat_of_vaporization_vetere,
    liquid_density_rackett,
};
use super::curve_conversion::vb_round_to_i32;
use super::gl::{calculate_formation_properties, FormationProperties};
use super::property_methods::{self, SpecificGravity};

/// Which correlation estimates a cut's **critical temperature** — upstream's
/// `TCcorr` string, matched at `GenerateCompounds.vb:287-296` and
/// `DistCurves.cs:695-709`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CriticalTemperatureCorrelation {
    /// Riazi-Daubert (1985) — [`property_methods::tc_riazi_daubert`].
    /// Recommended for `M` = 70-300 g/mol. **This port's default.**
    #[default]
    RiaziDaubert1985,
    /// Riazi (2005) — [`property_methods::tc_riazi_2005`], for `M` > 300 g/mol.
    Riazi2005,
    /// Lee-Kesler (1976) — [`property_methods::tc_lee_kesler`]; upstream's own
    /// "recommended method" remark.
    LeeKesler1976,
    /// Farah (2006) API A/B four-parameter form —
    /// [`property_methods::tc_farah_ab_sg_tb`].
    ///
    /// > **⚠️** Upstream calls this overload with its last two arguments
    /// > **swapped** (`GenerateCompounds.vb:295`, `DistCurves.cs:704` both pass
    /// > `NBP` into the `d15` slot and `SG` into the `PEMe` slot). The swap is
    /// > reproduced in [`build_pseudo_component`] so results match DWSIM
    /// > bit-for-bit; the values it produces are **not** physically meaningful.
    Farah2006,
}

/// Which correlation estimates a cut's **critical pressure** — upstream's
/// `PCcorr` string (`GenerateCompounds.vb:299-306`, `DistCurves.cs:712-723`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CriticalPressureCorrelation {
    /// Riazi-Daubert (1985) — [`property_methods::pc_riazi_daubert`].
    /// **This port's default**, and the only one of the three whose units are
    /// demonstrably correct.
    #[default]
    RiaziDaubert1985,
    /// Lee-Kesler (1976) — [`property_methods::pc_lee_kesler`].
    ///
    /// > **⚠️ Returns ≈10× the correct pressure** because of an upstream unit
    /// > error, reproduced faithfully. See
    /// > [`property_methods::pc_lee_kesler`] for the arithmetic. Selecting this
    /// > also corrupts the acentric factor, which divides by `Pc`.
    LeeKesler1976,
    /// Farah (2006) API A/B four-parameter form —
    /// [`property_methods::pc_farah_ab_tb_sg`].
    Farah2006,
}

/// Which correlation estimates a cut's **acentric factor** — upstream's
/// `AFcorr` string (`GenerateCompounds.vb:309-314`, `DistCurves.cs:726-734`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AcentricFactorCorrelation {
    /// Lee-Kesler (1976) — [`property_methods::acentric_factor_lee_kesler`].
    /// **This port's default**, matching common refinery practice.
    #[default]
    LeeKesler1976,
    /// Korsten (2000) — [`property_methods::acentric_factor_korsten`].
    Korsten2000,
}

/// Which correlation estimates a cut's **molecular weight** from `Tb` and `SG`
/// — upstream's `MWcorr` string (`GenerateCompounds.vb:119-126` and its five
/// other `Select Case` copies; `DistCurves.cs:587-598`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MolecularWeightCorrelation {
    /// Riazi (1986) — [`property_methods::mw_riazi`], for light/medium
    /// fractions. **This port's default.**
    #[default]
    Riazi1986,
    /// Winn (1956/57) — [`property_methods::mw_winn`].
    Winn1956,
    /// Lee-Kesler (1974) — [`property_methods::mw_lee_kesler`], for `Tb` below
    /// 750 K.
    LeeKesler1974,
}

/// The four correlation choices, bundled — upstream passes them as four loose
/// strings; grouping them keeps the characterization entry points readable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CorrelationSet {
    /// Critical-temperature correlation.
    pub critical_temperature: CriticalTemperatureCorrelation,
    /// Critical-pressure correlation.
    pub critical_pressure: CriticalPressureCorrelation,
    /// Acentric-factor correlation.
    pub acentric_factor: AcentricFactorCorrelation,
    /// Molecular-weight correlation (used only where the assay does not supply
    /// a molecular weight directly).
    pub molecular_weight: MolecularWeightCorrelation,
}

/// A characterized petroleum cut: a [`Component`] the crate's thermodynamics
/// can consume directly, plus the petroleum-specific data that has no home on
/// `Component`.
///
/// The wrapper exists because [`Component`] models a *pure compound* and
/// carries no notion of specific gravity, Watson `K`, Walther-ASTM viscosity
/// parameters, Rackett `Z_RA`, volume-translation coefficients, or Chao-Seader
/// solubility parameters — all of which a petroleum fraction needs and DWSIM's
/// `ConstantProperties` does carry. Downstream consumers that only want the EOS
/// constants can use [`PseudoComponent::component`] and ignore the rest.
#[derive(Debug, Clone, PartialEq)]
pub struct PseudoComponent {
    /// The EOS-ready constant-property record: name, `M`, `Tc`, `Pc`, `Vc`,
    /// `ω`, `Tb`. Feeds [`crate::thermo::cubic_eos`],
    /// [`crate::thermo::chao_seader_grayson`] and the flash family unchanged.
    ///
    /// > **Gap:** `Component::ig_entropy_formation_25c` is documented as
    /// > J/(mol·K) but DWSIM's petroleum path produces a **per-mass** value
    /// > whose units its own group contributions do not document (see
    /// > [`crate::petroleum::gl`]). Rather than assert a unit that has not been
    /// > verified, this field is left `f64::NAN` and the value is carried in
    /// > [`Self::formation`]. The ideal-gas `Cp` coefficients are likewise
    /// > left at zero: DWSIM does not estimate them for pseudo-components.
    pub component: Component,
    /// Mole fraction of this cut in the characterized stream [-]. Set by the
    /// generator; `0` until the whole set has been assembled and normalised.
    pub mole_fraction: Ratio,
    /// Specific gravity at 15.6/15.6 °C [-] — upstream `PF_SG`.
    pub specific_gravity: SpecificGravity,
    /// Watson (UOP) characterisation factor `Kw` [-] — upstream `PF_Watson_K`.
    pub watson_k: Ratio,
    /// Temperature of the first viscosity point [K] — upstream `PF_Tv1`.
    pub viscosity_temperature_1: ThermodynamicTemperature,
    /// Temperature of the second viscosity point [K] — upstream `PF_Tv2`.
    pub viscosity_temperature_2: ThermodynamicTemperature,
    /// Kinematic viscosity at [`Self::viscosity_temperature_1`] [m²/s] —
    /// upstream `PF_v1`.
    pub kinematic_viscosity_1: KinematicViscosity,
    /// Kinematic viscosity at [`Self::viscosity_temperature_2`] [m²/s] —
    /// upstream `PF_v2`.
    pub kinematic_viscosity_2: KinematicViscosity,
    /// Walther-ASTM `A` parameter [-] — upstream `PF_vA`.
    pub walther_a: f64,
    /// Walther-ASTM `B` parameter [-] — upstream `PF_vB`.
    pub walther_b: f64,
    /// Critical compressibility `Zc` [-] — upstream `Critical_Compressibility`.
    pub critical_compressibility: f64,
    /// Rackett compressibility `Z_RA` [-] — upstream `Z_Rackett`, clamped to a
    /// floor of 0.2 when the correlation returns a negative value.
    pub rackett_z: f64,
    /// Heat of vaporisation at the normal boiling point, per unit mass [J/kg]
    /// — upstream `HVap_A` (which stores kJ/kg).
    pub heat_of_vaporization: AvailableEnergy,
    /// Ideal-gas formation properties and PNA atom counts from
    /// [`crate::petroleum::gl`]. Note the units caveat in that module.
    pub formation: FormationProperties,
    /// Nominal formula string `"C…H…"` — upstream `Formula`.
    pub formula: String,
    /// Deterministic pseudo-CAS identifier `"<prefix>-<Tb in K>"` — upstream
    /// `CAS_Number` (`GenerateCompounds.vb:325`).
    pub cas_number: String,
    /// Chao-Seader acentricity [-] — upstream copies the acentric factor.
    pub chao_seader_acentricity: f64,
    /// Chao-Seader solubility parameter, in the `(cal/cm³)^0.5` units the
    /// Chao-Seader package is defined in (upstream
    /// `Chao_Seader_Solubility_Parameter`). Carried as a documented raw `f64`
    /// because it is a package-specific empirical parameter, not an SI
    /// quantity.
    pub chao_seader_solubility_parameter: f64,
    /// Chao-Seader liquid molar volume [m³/mol] — upstream
    /// `Chao_Seader_Liquid_Molar_Volume`, which stores cm³/mol.
    pub chao_seader_liquid_molar_volume: MolarVolume,
    /// Peng-Robinson volume-translation coefficient [-] — upstream
    /// `PR_Volume_Translation_Coefficient`. `1.0` until fitted by
    /// [`crate::petroleum::fitting`].
    pub pr_volume_translation_coefficient: f64,
    /// SRK volume-translation coefficient [-] — upstream
    /// `SRK_Volume_Translation_Coefficient`. `1.0` until fitted.
    pub srk_volume_translation_coefficient: f64,
}

/// Errors assembling a pseudo-component from non-physical correlation output.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PseudoComponentError {
    /// A correlation produced a non-finite or physically inadmissible constant.
    /// Upstream detects only NaN, only for `Tc`/`Pc`/`ω`/`M`, and only after
    /// stringifying everything (`GenerateCompounds.vb:475-483`), then throws
    /// "Invalid characterization, please try different parameters/settings";
    /// this port also rejects a non-positive `Vc` (which upstream never
    /// inspects — `:333`) and reports **which cut** and **which property**
    /// failed, so a caller can tell a bad heavy-end extrapolation from a bad
    /// assay.
    #[error("pseudo-component `{name}` (cut {index}): {property} came out non-physical ({value}) — try a different correlation set or a narrower cut")]
    NonPhysical {
        /// Name of the offending cut (`"<prefix>_NBP_<Tb in °C>"`).
        name: String,
        /// 1-based position of the offending cut in the slate, ascending
        /// boiling point — the `index` passed to [`build_pseudo_component`].
        index: usize,
        /// Which property failed: one of `"molar_mass"`,
        /// `"critical_temperature"`, `"critical_pressure"`,
        /// `"acentric_factor"`, `"critical_volume"`.
        property: &'static str,
        /// The offending value, in the property's SI unit (g/mol for
        /// `molar_mass`, K, Pa, dimensionless, m³/mol respectively).
        value: f64,
    },
}

/// Assemble one pseudo-component from its boiling point, specific gravity and
/// molecular weight.
///
/// This is the shared body of `GenerateCompounds.vb:260-377` and
/// `DistCurves.cs:664-771`. In order, it:
///
/// 1. names the cut `"<prefix>_NBP_<Tb in °C>"` and assigns the pseudo-CAS
///    `"<prefix>-<Tb in K>"` (`GenerateCompounds.vb:321-329`), rounding with
///    VB's banker's rule via [`vb_round_to_i32`];
/// 2. evaluates `Tc`, `Pc` and `ω` through the chosen [`CorrelationSet`];
/// 3. derives the Watson factor, `Zc`, `Vc` and the Rackett `Z_RA`
///    (`:331-335`), applying upstream's `Z_RA < 0 → 0.2` floor;
/// 4. runs the [`crate::petroleum::gl`] molecular-type analysis for the
///    formation properties and formula (`:337-343`);
/// 5. computes `HVap_A` by Vetere and the three Chao-Seader parameters
///    (`:347-358`).
///
/// # Inputs
///
/// - `prefix` — assay name; leading/trailing ` _,;:` are trimmed exactly as
///   upstream (`:321`).
/// - `index` — 1-based cut number, used only when `Tb` is not finite.
/// - `boiling_point` — the cut's mean boiling point `Tb` [K].
/// - `specific_gravity` — `SG` at 15.6/15.6 °C [-].
/// - `molar_mass` — the cut's molecular weight.
/// - `viscosity_temperature_1` / `_2` and `kinematic_viscosity_1` / `_2` — the
///   two `(T, v)` viscosity points, from which the Walther-ASTM `A`/`B` are
///   fitted.
/// - `correlations` — which correlation to use for each constant.
///
/// # Valid range
///
/// Every correlation involved is regressed for petroleum fractions with
/// `Tb` ≈ 300-850 K, `SG` ≈ 0.6-1.05 and `M` ≈ 70-500 g/mol; outside that band
/// the estimates degrade and may become non-physical, in which case this
/// function returns [`PseudoComponentError::NonPhysical`] rather than emitting
/// a broken [`Component`].
///
/// The heavy end is where this bites in practice. The Lee-Kesler acentric
/// factor is the `Tbr < 0.8` vapour-pressure form, and its denominator vanishes
/// as `Tbr = Tb/Tc → 1`, so `ω` grows without bound for a cut whose boiling
/// point approaches its own critical temperature. `Zc = 0.291 − 0.08·ω` then
/// turns negative above `ω = 3.6375` and `Vc = R·Zc·Tc/Pc` with it. Upstream
/// floors only the Rackett `Z_RA` (`GenerateCompounds.vb:335`) and emits the
/// negative `Vc` unchecked; this port refuses the cut instead (GitHub #170).
///
/// # Errors
///
/// [`PseudoComponentError::NonPhysical`] when `M`, `Tc`, `Pc` or `Vc` comes
/// out non-finite or non-positive, or `ω` comes out non-finite. The variant
/// names the cut and the property. There is deliberately **no** numeric bound
/// on `ω` itself: any cap (1.5? 2? 3.6?) would be a heuristic, and where the
/// correlations' validity envelope lies is a maintainer decision — the checks
/// here are the sign and finiteness conditions a critical constant must
/// satisfy to be a critical constant at all.
#[allow(clippy::too_many_arguments)]
pub fn build_pseudo_component(
    prefix: &str,
    index: usize,
    boiling_point: ThermodynamicTemperature,
    specific_gravity: SpecificGravity,
    molar_mass: MolarMass,
    viscosity_temperature_1: ThermodynamicTemperature,
    viscosity_temperature_2: ThermodynamicTemperature,
    kinematic_viscosity_1: KinematicViscosity,
    kinematic_viscosity_2: KinematicViscosity,
    correlations: CorrelationSet,
) -> Result<PseudoComponent, PseudoComponentError> {
    let tb = boiling_point;
    let sg = specific_gravity;
    let tb_k = tb.get::<kelvin>();

    // --- naming (`GenerateCompounds.vb:321-329`) ---------------------------
    let trimmed: &str = prefix.trim_matches(|c| matches!(c, ' ' | '_' | ',' | ';' | ':'));
    let (name, cas_number) = if tb_k.is_finite() {
        (
            format!("{trimmed}_NBP_{}", vb_round_to_i32(tb_k - 273.15)),
            format!("{trimmed}-{}", vb_round_to_i32(tb_k)),
        )
    } else {
        (
            format!("{trimmed}_NBP_{index}"),
            format!("{trimmed}-{index}"),
        )
    };

    // --- Walther-ASTM viscosity parameters (`:249-250`) --------------------
    let walther_a = property_methods::visc_walther_astm_a(
        viscosity_temperature_1,
        kinematic_viscosity_1,
        viscosity_temperature_2,
        kinematic_viscosity_2,
    );
    let walther_b = property_methods::visc_walther_astm_b(
        viscosity_temperature_1,
        kinematic_viscosity_1,
        viscosity_temperature_2,
        kinematic_viscosity_2,
    );

    // --- critical constants (`:287-314`) -----------------------------------
    let tc = match correlations.critical_temperature {
        CriticalTemperatureCorrelation::RiaziDaubert1985 => {
            property_methods::tc_riazi_daubert(tb, sg)
        }
        CriticalTemperatureCorrelation::Riazi2005 => property_methods::tc_riazi_2005(tb, sg),
        CriticalTemperatureCorrelation::LeeKesler1976 => property_methods::tc_lee_kesler(tb, sg),
        // Argument order reproduced exactly from `GenerateCompounds.vb:295` /
        // `DistCurves.cs:704`, which pass (vA, vB, NBP, SG) into a function
        // declared (A, B, d15, PEMe) — i.e. Tb and SG are swapped upstream.
        // See CriticalTemperatureCorrelation::Farah2006.
        CriticalTemperatureCorrelation::Farah2006 => property_methods::tc_farah_ab_sg_tb(
            walther_a,
            walther_b,
            Ratio::new::<ratio>(tb_k),
            ThermodynamicTemperature::new::<kelvin>(sg.get::<ratio>()),
        ),
    };
    let pc = match correlations.critical_pressure {
        CriticalPressureCorrelation::RiaziDaubert1985 => property_methods::pc_riazi_daubert(tb, sg),
        CriticalPressureCorrelation::LeeKesler1976 => property_methods::pc_lee_kesler(tb, sg),
        CriticalPressureCorrelation::Farah2006 => {
            property_methods::pc_farah_ab_tb_sg(walther_a, walther_b, tb, sg)
        }
    };
    let omega = match correlations.acentric_factor {
        AcentricFactorCorrelation::LeeKesler1976 => {
            property_methods::acentric_factor_lee_kesler(tc, pc, tb)
        }
        AcentricFactorCorrelation::Korsten2000 => {
            property_methods::acentric_factor_korsten(tc, pc, tb)
        }
    };

    // --- validation (replaces the stringified NaN sweep at `:475-483`) -----
    let reject = |property: &'static str, value: f64| PseudoComponentError::NonPhysical {
        name: name.clone(),
        index,
        property,
        value,
    };
    let check_positive = |property: &'static str, value: f64| -> Result<(), PseudoComponentError> {
        if !value.is_finite() || value <= 0.0 {
            Err(reject(property, value))
        } else {
            Ok(())
        }
    };
    check_positive("molar_mass", molar_mass.get::<gram_per_mole>())?;
    check_positive("critical_temperature", tc.get::<kelvin>())?;
    check_positive("critical_pressure", pc.get::<pascal>())?;
    // `ω` is checked for finiteness only. It is legitimately negative for
    // quantum gases and its physical upper bound for heavy hydrocarbons is a
    // matter of judgement, not definition, so no sign or magnitude rule is
    // imposed here — see the `# Errors` note. Checking it before `Vc` means a
    // NaN `ω` is reported as `ω`, not as the `Vc` it would poison next.
    let omega_value = omega.get::<ratio>();
    if !omega_value.is_finite() {
        return Err(reject("acentric_factor", omega_value));
    }

    // --- derived constants (`:331-335`, `:349-354`) ------------------------
    let watson_k = property_methods::watson_k(tb, sg);
    let zc = critical_compressibility_zc1(omega);
    let vc = critical_volume(tc, pc, zc);
    let rackett_z = if zc < 0.0 { 0.2 } else { zc };
    // A critical volume is only physical when positive: it is the molar volume
    // of the fluid at its critical point, and `Vc = R·Zc·Tc/Pc` with `Tc, Pc >
    // 0` already established means `Vc <= 0` is exactly `Zc <= 0`, i.e. the
    // Lee-Kesler `ω` has run past 3.6375 on a cut whose `Tb/Tc` is near 1.
    // Upstream floors `Z_RA` to 0.2 at this point (`:335`) -- proof it knew
    // `Zc` goes negative -- yet still stores the negative `Vc` (`:333`), and
    // its NaN sweep never looks at `Vc` at all. A finite negative `Vc` then
    // flows into every EOS that consumes it and nothing downstream can tell.
    // Rejecting here is what lets the `Err` mean "this cut is not a compound"
    // the way the `Tc`/`Pc`/`M` checks above already do (GitHub #170).
    check_positive("critical_volume", vc.get::<cubic_meter_per_mole>())?;

    // --- formation properties (`:337-343`) ---------------------------------
    let formation = calculate_formation_properties(sg, molar_mass, tb);
    let formula = formation.formula();

    // --- heat of vaporisation and Chao-Seader parameters (`:347-358`) ------
    let dhvb = heat_of_vaporization_vetere(tc, pc, tb).get::<joule_per_mole>();
    let mm_g = molar_mass.get::<gram_per_mole>();
    // Upstream: HVap_A = DHvb[kJ/kmol] / MW[g/mol] -> kJ/kg. J/mol per kg/mol
    // is J/kg, the SI form used here (numerically 1000x upstream's kJ/kg).
    let hvap_j_per_kg = dhvb / molar_mass.get::<uom::si::molar_mass::kilogram_per_mole>();

    let rho_nbp = liquid_density_rackett(tb, tc, pc, omega, molar_mass, None, None, None)
        .get::<kilogram_per_cubic_meter>();

    // `(HVap_A[kJ/kg]·M[g/mol] − R·Tb)` is a molar enthalpy in J/mol; the
    // 238.846 factor and the 1e6 divisor put the result in (cal/cm³)^0.5.
    let hvap_kj_per_kg = hvap_j_per_kg / 1000.0;
    let chao_seader_solubility_parameter =
        ((hvap_kj_per_kg * mm_g - 8.314 * tb_k) * 238.846 * rho_nbp / mm_g / 1_000_000.0).sqrt();
    // Upstream returns cm³/mol; convert to m³/mol.
    let chao_seader_liquid_molar_volume =
        MolarVolume::new::<cubic_meter_per_mole>(mm_g / 1000.0 / rho_nbp);

    let component = Component {
        name: name.clone(),
        molar_mass: molar_mass.get::<uom::si::molar_mass::kilogram_per_mole>(),
        critical_temperature: tc.get::<kelvin>(),
        critical_pressure: pc.get::<pascal>(),
        critical_volume: vc.get::<cubic_meter_per_mole>(),
        acentric_factor: omega.get::<ratio>(),
        normal_boiling_point: tb_k,
        // DWSIM does not estimate ideal-gas Cp coefficients for
        // pseudo-components; left at zero rather than invented.
        cp_ig_a: 0.0,
        cp_ig_b: 0.0,
        cp_ig_c: 0.0,
        cp_ig_d: 0.0,
        cp_ig_e: 0.0,
        // Units unverified upstream — see the field docs on
        // `PseudoComponent::component`.
        ig_entropy_formation_25c: f64::NAN,
    };

    Ok(PseudoComponent {
        component,
        mole_fraction: Ratio::new::<ratio>(0.0),
        specific_gravity: sg,
        watson_k,
        viscosity_temperature_1,
        viscosity_temperature_2,
        kinematic_viscosity_1,
        kinematic_viscosity_2,
        walther_a,
        walther_b,
        critical_compressibility: zc,
        rackett_z,
        heat_of_vaporization: AvailableEnergy::new::<joule_per_kilogram>(hvap_j_per_kg),
        formation,
        formula,
        cas_number,
        chao_seader_acentricity: omega.get::<ratio>(),
        chao_seader_solubility_parameter,
        chao_seader_liquid_molar_volume,
        pr_volume_translation_coefficient: 1.0,
        srk_volume_translation_coefficient: 1.0,
    })
}

/// Estimate the two viscosity points of a cut from Abbott's correlations at the
/// standard 100 °F / 210 °F reference temperatures.
///
/// Ported from `DistCurves.cs:673-679` (the `!hasvisc100c` branch), which pins
/// `PF_Tv1 = 311 K` and `PF_Tv2 = 372 K` — the rounded Kelvin equivalents of
/// 100 °F and 210 °F — and fills the viscosities from
/// [`property_methods::visc37_abbott`] / [`property_methods::visc98_abbott`].
///
/// Returns `(T1, T2, v1, v2)` with temperatures in K and viscosities in m²/s.
#[must_use]
pub fn default_viscosity_points(
    boiling_point: ThermodynamicTemperature,
    specific_gravity: SpecificGravity,
) -> (
    ThermodynamicTemperature,
    ThermodynamicTemperature,
    KinematicViscosity,
    KinematicViscosity,
) {
    (
        ThermodynamicTemperature::new::<kelvin>(311.0),
        ThermodynamicTemperature::new::<kelvin>(372.0),
        property_methods::visc37_abbott(boiling_point, specific_gravity),
        property_methods::visc98_abbott(boiling_point, specific_gravity),
    )
}

/// Normalise a set of mole fractions in place so they sum to exactly 1, and
/// return the mass fractions implied by each cut's molecular weight.
///
/// Ported from `DistCurves.cs:1082-1092` (the tail of
/// `CalculateMolarFractions`), which computes `Σ x_i M_i` and then
/// `w_i = x_i M_i / Σ x_j M_j`.
///
/// Returns the mass fractions in the same order. If the mixture molecular
/// weight is zero or non-finite, every mass fraction is returned as `0`.
pub fn normalise_and_mass_fractions(components: &mut [PseudoComponent]) -> Vec<Ratio> {
    let total: f64 = components
        .iter()
        .map(|c| c.mole_fraction.get::<ratio>())
        .filter(|v| v.is_finite())
        .sum();
    if total > 0.0 && total.is_finite() {
        for c in components.iter_mut() {
            c.mole_fraction = Ratio::new::<ratio>(c.mole_fraction.get::<ratio>() / total);
        }
    }
    let mixture_mw: f64 = components
        .iter()
        .map(|c| c.mole_fraction.get::<ratio>() * c.component.molar_mass)
        .sum();
    components
        .iter()
        .map(|c| {
            if mixture_mw > 0.0 && mixture_mw.is_finite() {
                Ratio::new::<ratio>(
                    c.mole_fraction.get::<ratio>() * c.component.molar_mass / mixture_mw,
                )
            } else {
                Ratio::new::<ratio>(0.0)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tk(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(v)
    }

    /// **Methodology.** Build a single kerosene-range pseudo-component
    /// (`Tb = 450 K`, `SG = 0.78`, `M = 160 g/mol`) with the default
    /// correlation set and check every emitted constant is physical and lands
    /// in the band a real kerosene cut occupies (Riazi, ASTM MNL50, 2005,
    /// Table 2.1): `Tc` 600-700 K, `Pc` 1.5-3.5 MPa, `ω` 0.2-0.8,
    /// `Kw` 11-13, `Vc` > 0, `Z_RA` 0.2-0.3, `ΔHvap` 200-500 kJ/kg. Names and
    /// the pseudo-CAS must follow the DWSIM format.
    ///
    /// **Results (2026-08-11, this port).**
    /// `Tc = 636.857 K`, `Pc = 2.32460e6 Pa`, `ω = 0.406352`,
    /// `Kw = 11.9509`, `Vc = 5.8878e-4 m³/mol` (588.8 cm³/mol),
    /// `Z_RA = 0.258492`, `ΔHvap = 2.41566e5 J/kg` (242 kJ/kg),
    /// Chao-Seader solubility parameter `6.133 (cal/cm³)^0.5`, Chao-Seader
    /// liquid molar volume `2.2167e-4 m³/mol` (221.7 cm³/mol); name
    /// `"Crude_NBP_177"`, CAS `"Crude-450"`, formula `"C11.35H22.58"`. All
    /// inside the stated bands. Test passes.
    #[test]
    fn kerosene_pseudo_component_constants_are_physical() {
        let tb = tk(450.0);
        let sg = Ratio::new::<ratio>(0.78);
        let (t1, t2, v1, v2) = default_viscosity_points(tb, sg);
        let pc = build_pseudo_component(
            " Crude_",
            1,
            tb,
            sg,
            MolarMass::new::<gram_per_mole>(160.0),
            t1,
            t2,
            v1,
            v2,
            CorrelationSet::default(),
        )
        .expect("kerosene cut is inside every correlation's validity range");

        assert_eq!(pc.component.name, "Crude_NBP_177");
        assert_eq!(pc.cas_number, "Crude-450");
        assert!(
            (600.0..700.0).contains(&pc.component.critical_temperature),
            "Tc = {}",
            pc.component.critical_temperature
        );
        assert!(
            (1.5e6..3.5e6).contains(&pc.component.critical_pressure),
            "Pc = {}",
            pc.component.critical_pressure
        );
        assert!(
            (0.2..0.8).contains(&pc.component.acentric_factor),
            "omega = {}",
            pc.component.acentric_factor
        );
        assert!(
            (11.0..13.0).contains(&pc.watson_k.get::<ratio>()),
            "Kw = {}",
            pc.watson_k.get::<ratio>()
        );
        assert!(pc.component.critical_volume > 0.0);
        assert!(
            (0.2..0.3).contains(&pc.rackett_z),
            "Z_RA = {}",
            pc.rackett_z
        );
        let hvap = pc.heat_of_vaporization.get::<joule_per_kilogram>();
        assert!((2.0e5..5.0e5).contains(&hvap), "dHvap = {hvap} J/kg");
        assert!(pc.formula.starts_with('C'));
    }

    /// **Methodology.** Non-physical inputs must be rejected with the offending
    /// property **named**, rather than silently producing a broken
    /// [`Component`] (upstream throws a generic "Invalid characterization"
    /// after stringifying everything, `GenerateCompounds.vb:475-483`). Feed a
    /// degenerate cut with zero molecular weight and require a
    /// [`PseudoComponentError::NonPhysical`] naming `molar_mass`.
    ///
    /// **Note on what does *not* trip the guard.** Extreme but finite inputs
    /// such as `Tb = 1200 K` with `SG = 0.3` do **not** fail this check: the
    /// Riazi-Daubert correlations still return positive finite numbers there
    /// (`Tc = 548.5 K`, `Pc = 522.6 Pa` — physically absurd but numerically
    /// valid). The guard catches only non-finite or non-positive constants
    /// (and, since GitHub #170, a non-positive `Vc` — see
    /// [`heavy_cut_with_negative_critical_volume_is_rejected_by_name`]); it is
    /// **not** a validity-range check. Callers are responsible for staying
    /// inside the correlation ranges documented on [`build_pseudo_component`].
    ///
    /// **Results (2026-08-11, this port; re-run 2026-09-10 after the `index`
    /// field was added).** Zero molecular weight returns
    /// `NonPhysical { index: 1, property: "molar_mass", value: 0.0 }`. Test
    /// passes.
    #[test]
    fn non_physical_inputs_are_rejected_by_name() {
        let tb = tk(450.0);
        let sg = Ratio::new::<ratio>(0.78);
        let (t1, t2, v1, v2) = default_viscosity_points(tb, sg);
        let err = build_pseudo_component(
            "Bad",
            1,
            tb,
            sg,
            MolarMass::new::<gram_per_mole>(0.0),
            t1,
            t2,
            v1,
            v2,
            CorrelationSet::default(),
        )
        .expect_err("a cut with zero molecular weight is not a compound");
        match err {
            PseudoComponentError::NonPhysical {
                index,
                property,
                value,
                ..
            } => {
                assert_eq!(index, 1);
                assert_eq!(property, "molar_mass");
                assert_eq!(value, 0.0);
            }
        }
    }

    /// **Methodology (GitHub #170 regression, unit level).** Drive a single
    /// cut into the heavy-end regime where the Lee-Kesler acentric factor
    /// diverges: `Tb = 1462.78 K`, `SG = 1.110221`, `M = 1431.21 g/mol` — the
    /// tenth cut of `BlackOilCrude::heavy()` at 10 cuts as the gamma
    /// distribution produces it (measured before the guard; see the
    /// integration-level regression in `crude_distillation.rs`). Before this
    /// guard the call
    /// returned `Ok` with a **negative** critical volume, an ordinary finite
    /// `f64` nothing downstream could detect. It must now return
    /// [`PseudoComponentError::NonPhysical`] naming `critical_volume`, the cut
    /// index and a negative value, with `Tc` and `Pc` (which are positive and
    /// pass) not blamed.
    ///
    /// **Results (2026-09-10, `cargo test --release`, this port).**
    /// `Err(NonPhysical { name: "Bad_NBP_1190", index: 10, property:
    /// "critical_volume", value: -3.2680562e-2 })` (m³/mol). The same inputs,
    /// read off the pre-guard slate, give `Tc = 1485.4128 K`,
    /// `Pc = 3.115385e5 Pa`, `Tbr = 0.9848`, `ω = 13.942646`,
    /// `Zc = 0.291 − 0.08·ω = −0.824412`, confirming the mechanism described
    /// on [`build_pseudo_component`]. Test passes.
    #[test]
    fn heavy_cut_with_negative_critical_volume_is_rejected_by_name() {
        let tb = tk(1462.7833);
        let sg = Ratio::new::<ratio>(1.110221);
        let (t1, t2, v1, v2) = default_viscosity_points(tb, sg);
        let err = build_pseudo_component(
            "Bad",
            10,
            tb,
            sg,
            MolarMass::new::<gram_per_mole>(1431.2142),
            t1,
            t2,
            v1,
            v2,
            CorrelationSet::default(),
        )
        .expect_err("a cut whose Zc = 0.291 - 0.08*omega is negative has no critical volume");
        match err {
            PseudoComponentError::NonPhysical {
                name,
                index,
                property,
                value,
            } => {
                assert_eq!(name, "Bad_NBP_1190");
                assert_eq!(index, 10);
                assert_eq!(
                    property, "critical_volume",
                    "Tc and Pc are positive here; only Vc is inadmissible"
                );
                assert!(
                    value.is_finite() && value < 0.0,
                    "the rejected Vc must be the finite negative number the correlation \
                     produced, not a NaN: got {value}"
                );
                let reference = -3.2680562e-2;
                assert!(
                    ((value - reference) / reference).abs() < 1e-5,
                    "Vc = {value} m^3/mol vs the pre-guard figure {reference}"
                );
            }
        }
    }

    /// **Methodology.** [`normalise_and_mass_fractions`] must (a) renormalise
    /// mole fractions to sum to 1 and (b) return mass fractions that also sum
    /// to 1 and are consistent with `w_i ∝ x_i M_i`. Build three cuts with
    /// deliberately unnormalised mole fractions.
    ///
    /// **Results (2026-08-11, this port).** Mole fractions `[2, 3, 5]` →
    /// `[0.2, 0.3, 0.5]` (sum 1.0 exactly); mass fractions sum to 1.0 to within
    /// 1e-15 and the heaviest cut carries the largest mass share. Test passes.
    #[test]
    fn mole_and_mass_fractions_normalise() {
        let mut cuts: Vec<PseudoComponent> = [(400.0, 120.0), (480.0, 190.0), (560.0, 280.0)]
            .iter()
            .enumerate()
            .map(|(i, &(tb_k, mw))| {
                let tb = tk(tb_k);
                let sg = Ratio::new::<ratio>(0.78);
                let (t1, t2, v1, v2) = default_viscosity_points(tb, sg);
                build_pseudo_component(
                    "X",
                    i + 1,
                    tb,
                    sg,
                    MolarMass::new::<gram_per_mole>(mw),
                    t1,
                    t2,
                    v1,
                    v2,
                    CorrelationSet::default(),
                )
                .expect("in range")
            })
            .collect();
        for (c, x) in cuts.iter_mut().zip([2.0, 3.0, 5.0]) {
            c.mole_fraction = Ratio::new::<ratio>(x);
        }
        let mass = normalise_and_mass_fractions(&mut cuts);
        let mole_sum: f64 = cuts.iter().map(|c| c.mole_fraction.get::<ratio>()).sum();
        let mass_sum: f64 = mass.iter().map(|w| w.get::<ratio>()).sum();
        assert!((mole_sum - 1.0).abs() < 1.0e-15, "mole sum {mole_sum}");
        assert!((mass_sum - 1.0).abs() < 1.0e-15, "mass sum {mass_sum}");
        assert!(
            mass[2].get::<ratio>() > mass[0].get::<ratio>(),
            "the heaviest cut must carry more mass than the lightest"
        );
    }
}
