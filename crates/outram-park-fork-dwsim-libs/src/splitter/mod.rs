//! Splitter (stream splitter / tee): a pure mass-balance unit operation that
//! divides one inlet material stream into N outlet streams.
//!
//! Pure-Rust port of DWSIM's splitter, from `UnitOperations/Splitter.vb`
//! (GPL-3.0, `Public Overrides Sub Calculate`, lines 201-395). Upstream
//! copyright: 2008 Daniel Wagner O. de Medeiros.
//!
//! A splitter routes the inlet's flow to several outlets **without changing the
//! intensive state**. Every outlet inherits the inlet's temperature, pressure,
//! specific enthalpy, and composition unchanged (Splitter.vb:258-266 copies
//! `temperature`, `pressure`, `enthalpy`, and each compound's mole/mass fraction
//! straight onto every outlet); only the *extensive* flow is divided
//! (Splitter.vb:268 `massflow = W * Ratios(i)`). DWSIM itself describes it as
//! "a mass balance unit operation — splits a material stream into two or three
//! other streams with different overall flow rates but with the same
//! composition" (Splitter.vb:209-210).
//!
//! # Split-specification modes (ported as the [`SplitSpec`] enum)
//!
//! DWSIM's `OpMode` enum (Splitter.vb:38-42) offers three specification modes;
//! this port mirrors them as one enum (no `dyn` dispatch, per the workspace
//! design rules):
//!
//! - [`SplitSpec::Fractions`] — DWSIM `OpMode.SplitRatios` (Splitter.vb:239-276):
//!   each outlet `i` gets a fraction `f_i` of the inlet flow, `Σ f_i = 1`,
//!   `w_i = f_i · w_in` (Splitter.vb:268).
//! - [`SplitSpec::MassFlows`] — DWSIM `OpMode.StreamMassFlowSpec`
//!   (Splitter.vb:278-332): fixed **mass** flows to the leading outlets, the
//!   remainder to the last (Splitter.vb:293 `wn(1) = W - w1`, :303
//!   `wn(2) = W - w1 - w2`).
//! - [`SplitSpec::MoleFlows`] — DWSIM `OpMode.StreamMoleFlowSpec`
//!   (Splitter.vb:334-389): the same, on a **mole** flow basis
//!   (Splitter.vb:349 `mn(1) = M - m1`, :359 `mn(2) = M - m1 - m2`).
//!
//! # Flash boundary — none needed for the intensive state
//!
//! Unlike [`crate::mixer`] and [`crate::expander`], a splitter needs **no**
//! property-package / flash call to fix its outlet intensive state: because the
//! outlets share the inlet's temperature, pressure, specific enthalpy, and
//! composition byte-for-byte, DWSIM copies the *already-solved* inlet state
//! directly onto each outlet (Splitter.vb:258-260 sets outlet `temperature`,
//! `pressure`, `enthalpy` equal to the inlet's) rather than re-flashing. DWSIM
//! still tags each outlet `SpecType = Pressure_and_Enthalpy` (Splitter.vb:272)
//! so the flowsheet solver stays consistent, but the outlet temperature is
//! already known from the inlet. This port therefore carries the inlet
//! [`IntensiveState`] through to every [`OutletStream`] verbatim and requires no
//! caller-supplied flash closure. (A caller who *changes* an outlet's pressure
//! downstream of the tee would flash there, but that is outside the splitter.)
//!
//! # Excluded DWSIM behavior
//!
//! Deliberately **not** ported (GUI / solver / persistence plumbing, no physics):
//! the `EditingForm_Splitter` editor and icon/bitmap resources
//! (Splitter.vb:32, :559-595), XML/JSON serialization and I/O
//! (`CloneXML`/`CloneJSON`/`SaveData`/`LoadData`, :73-126), the flowsheet
//! `Inspector` trace paragraphs (:203-209), `GetCalculationModes` /
//! `SetCalculationMode` string plumbing (:53-71), `RunDynamicModel`
//! (:148-199) and `DeCalculate` outlet clearing (:397-426), and the
//! property-grid reflection accessors
//! (`GetPropertyValue`/`GetProperties`/`SetPropertyValue`/`GetPropertyUnit`,
//! :428-557). DWSIM's two/three-outlet GUI cap is dropped — this port accepts
//! any number of outlets `N >= 1`.

use uom::si::available_energy::joule_per_kilogram;
use uom::si::catalytic_activity::katal;
use uom::si::f64::{
    AvailableEnergy, CatalyticActivity, MassRate, Pressure, Ratio, ThermodynamicTemperature,
};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

/// Molar (mole) flow rate — amount of substance per unit time.
///
/// `uom` 0.38 has no dedicated `MolarFlowRate` quantity, but a molar flow is
/// dimensionally `T⁻¹N` (mol · s⁻¹), which is exactly the SI unit **katal**
/// (`CatalyticActivity`). This alias gives the splitter's public API a
/// human-readable name for that quantity (per the workspace "named type alias"
/// rule) while reusing the correct `uom` dimension. Base unit: **katal =
/// mol/s** (`uom::si::catalytic_activity::katal`).
pub type MolarFlowRate = CatalyticActivity;

/// Tolerance on the split-fraction sum and on flow-remainder non-negativity.
///
/// [`SplitSpec::Fractions`] requires `|Σ f_i − 1| <= FRACTION_SUM_TOLERANCE`
/// (dimensionless), and the flow-spec modes reject a remainder more negative
/// than `−FRACTION_SUM_TOLERANCE · w_in` (i.e. the fixed flows over-drawing the
/// inlet). `1e-9` comfortably absorbs `f64` round-off in a sum of order-1
/// fractions while still catching genuine mis-specifications.
pub const FRACTION_SUM_TOLERANCE: f64 = 1.0e-9;

/// The intensive (flow-independent, per-unit-mass) thermodynamic state that a
/// splitter passes unchanged from its inlet to every outlet.
///
/// A splitter alters only extensive flows, never this state — so the same
/// `IntensiveState` is shared by the inlet and all outlets
/// (Splitter.vb:258-266). Units (all SI, `uom`-typed):
/// - `temperature` — `T` \[K\], `> 0`. Copied verbatim from the inlet
///   (Splitter.vb:258).
/// - `pressure` — `p` \[Pa\], `> 0` (Splitter.vb:259).
/// - `specific_enthalpy` — `h` \[J/kg\], mass basis, matching DWSIM's
///   `Phases(0).Properties.enthalpy` (Splitter.vb:260). Any real value; only the
///   datum must be consistent with the caller's convention.
/// - `mole_fractions` — overall composition as mole fractions `y_i`
///   (dimensionless \[0, 1\], summing to 1), one entry per compound
///   (Splitter.vb:263-266). Empty is allowed for a composition-agnostic
///   flow-only split.
#[derive(Debug, Clone, PartialEq)]
pub struct IntensiveState {
    /// Temperature `T` \[K\], `> 0`.
    pub temperature: ThermodynamicTemperature,
    /// Pressure `p` \[Pa\], `> 0`.
    pub pressure: Pressure,
    /// Specific enthalpy `h` \[J/kg\], mass basis.
    pub specific_enthalpy: AvailableEnergy,
    /// Overall composition as mole fractions `y_i` (dimensionless \[0, 1\]).
    pub mole_fractions: Vec<Ratio>,
}

impl IntensiveState {
    /// Convenience constructor from SI scalars: `temperature` \[K\],
    /// `pressure` \[Pa\], `specific_enthalpy` \[J/kg\], and a slice of mole
    /// fractions (dimensionless \[0, 1\]).
    pub fn from_si(
        temperature: f64,
        pressure: f64,
        specific_enthalpy: f64,
        mole_fractions: &[f64],
    ) -> Self {
        Self {
            temperature: ThermodynamicTemperature::new::<kelvin>(temperature),
            pressure: Pressure::new::<pascal>(pressure),
            specific_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(specific_enthalpy),
            mole_fractions: mole_fractions
                .iter()
                .map(|&y| Ratio::new::<ratio>(y))
                .collect(),
        }
    }
}

/// How the inlet flow is divided among the outlets — DWSIM's `Splitter.OpMode`
/// enum (Splitter.vb:38-42). Modeled as an enum (no `dyn` dispatch), per the
/// workspace design rules. The specification list is owned **by value** (a
/// `Vec`, indexed by `usize`), never by reference — no lifetimes.
#[derive(Debug, Clone, PartialEq)]
pub enum SplitSpec {
    /// `OpMode.SplitRatios` (Splitter.vb:239-276). Each outlet `i` receives a
    /// fraction `f_i` of the inlet flow; the vector has one entry per outlet and
    /// **must** satisfy `Σ f_i = 1` (validated to within
    /// [`FRACTION_SUM_TOLERANCE`]) with every `f_i >= 0`.
    ///
    /// Divergence from DWSIM: DWSIM does not validate the sum — it *overwrites*
    /// the last ratio with `1 − Σ(others)` (Splitter.vb:247-250), silently
    /// "fixing" an inconsistent input. This port instead treats the fractions as
    /// caller-authoritative and errors on a non-unit sum (workspace honesty
    /// rule: do not silently repair a mis-specification).
    Fractions(Vec<Ratio>),
    /// `OpMode.StreamMassFlowSpec` (Splitter.vb:278-332). Fixed **mass** flows
    /// `w_1 … w_{N-1}` to the leading `N−1` outlets; the last outlet gets the
    /// remainder `w_in − Σ w_k` (Splitter.vb:293, :303). The vector holds the
    /// `N−1` fixed flows (\[kg/s\], each `>= 0`); an empty vector yields a single
    /// pass-through outlet carrying the whole inlet flow.
    MassFlows(Vec<MassRate>),
    /// `OpMode.StreamMoleFlowSpec` (Splitter.vb:334-389). As [`Self::MassFlows`]
    /// but on a **mole** flow basis: fixed mole flows `m_1 … m_{N-1}` to the
    /// leading outlets, remainder `m_in − Σ m_k` to the last (Splitter.vb:349,
    /// :359). Units: katal (mol/s), each `>= 0`.
    MoleFlows(Vec<MolarFlowRate>),
}

/// Errors from resolving a [`SplitSpec`].
///
/// These correspond to DWSIM's `Throw New Exception(...)` guards
/// (Splitter.vb:295, :305, :351, :361 for over-drawn flow specs) plus this
/// port's stricter fraction-sum check (see [`SplitSpec::Fractions`]).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SplitError {
    /// A [`SplitSpec::Fractions`] entry was negative. `index` is its position;
    /// `value` is the offending fraction.
    #[error("split fraction at index {index} is negative ({value})")]
    NegativeFraction {
        /// Position of the offending fraction in the vector.
        index: usize,
        /// The negative fraction value (dimensionless).
        value: f64,
    },
    /// The [`SplitSpec::Fractions`] entries do not sum to 1 within
    /// [`FRACTION_SUM_TOLERANCE`]. `sum` is `Σ f_i`.
    #[error("split fractions sum to {sum}, not 1 (tolerance {tolerance})")]
    FractionSumNotUnity {
        /// The actual `Σ f_i` (dimensionless).
        sum: f64,
        /// The tolerance applied ([`FRACTION_SUM_TOLERANCE`]).
        tolerance: f64,
    },
    /// A fixed flow in [`SplitSpec::MassFlows`] / [`SplitSpec::MoleFlows`] was
    /// negative. `index` is its position; `value` is the offending flow (SI:
    /// kg/s for mass, katal for mole).
    #[error("fixed flow at index {index} is negative ({value})")]
    NegativeFixedFlow {
        /// Position of the offending fixed flow.
        index: usize,
        /// The negative flow value in SI units (kg/s or katal).
        value: f64,
    },
    /// The fixed flows exceed the inlet flow, so the remainder to the last
    /// outlet would be negative — DWSIM `Throw New Exception` when
    /// `W < Σ spec` (Splitter.vb:290-296, :298-305, :346-351, :354-361).
    /// `inlet` and `fixed_total` are in SI units (kg/s or katal).
    #[error("fixed flows total {fixed_total} exceed inlet flow {inlet}; remainder would be negative")]
    InsufficientInletFlow {
        /// Inlet flow in SI units (kg/s or katal).
        inlet: f64,
        /// Sum of the fixed flows in SI units (kg/s or katal).
        fixed_total: f64,
    },
    /// A flow-spec mode needs a strictly positive inlet flow to convert flows to
    /// fractions, but the inlet flow was `<= 0`. `value` is the inlet flow in SI
    /// units (kg/s or katal).
    #[error("inlet flow {value} is not positive; cannot resolve a flow-spec split")]
    NonPositiveInletFlow {
        /// The non-positive inlet flow in SI units (kg/s or katal).
        value: f64,
    },
    /// A [`SplitSpec::Fractions`] had no entries — a splitter needs `>= 1`
    /// outlet.
    #[error("split specification has no outlets")]
    NoOutlets,
}

/// The resolved per-outlet split produced by [`split`].
///
/// All three vectors have the same length `N` (the outlet count) and are ordered
/// by outlet index. Because the intensive state is uniform, mass and mole flow
/// are both simply the inlet flow scaled by the *same* fraction, so all three
/// stay mutually consistent.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitResult {
    /// Per-outlet split fractions `f_i` (dimensionless \[0, 1\]), `Σ f_i = 1`.
    pub fractions: Vec<Ratio>,
    /// Per-outlet mass flows `w_i = f_i · w_in` \[kg/s\] (Splitter.vb:268).
    pub mass_flows: Vec<MassRate>,
    /// Per-outlet mole flows `m_i = f_i · m_in` \[katal = mol/s\]
    /// (Splitter.vb:381).
    pub mole_flows: Vec<MolarFlowRate>,
}

/// One outlet material stream produced by [`split_streams`]: the inlet's
/// [`IntensiveState`] carried through unchanged, tagged with this outlet's flow.
///
/// The `state` field is identical (`==`) to the splitter inlet's intensive state
/// — that is the defining property of a splitter (Splitter.vb:258-266).
#[derive(Debug, Clone, PartialEq)]
pub struct OutletStream {
    /// Intensive state, identical to the inlet's (T, p, h, composition).
    pub state: IntensiveState,
    /// This outlet's split fraction `f_i` (dimensionless \[0, 1\]).
    pub split_fraction: Ratio,
    /// This outlet's mass flow `w_i = f_i · w_in` \[kg/s\].
    pub mass_flow: MassRate,
    /// This outlet's mole flow `m_i = f_i · m_in` \[katal = mol/s\].
    pub mole_flow: MolarFlowRate,
}

/// Resolve a [`SplitSpec`] into per-outlet split fractions `f_i` (dimensionless,
/// `Σ f_i = 1`).
///
/// This is the core of the DWSIM `Calculate` routine (Splitter.vb:237-391)
/// reduced to its dimensionless essence: whatever the mode, the outcome is a set
/// of fractions of the inlet flow. The flow-spec modes need the inlet flow (mass
/// or mole, in SI units) to convert fixed flows to fractions; [`SplitSpec::Fractions`]
/// ignores it.
///
/// - `inlet_mass_flow_si` — inlet mass flow \[kg/s\], used only by
///   [`SplitSpec::MassFlows`].
/// - `inlet_mole_flow_si` — inlet mole flow \[katal = mol/s\], used only by
///   [`SplitSpec::MoleFlows`].
///
/// # Errors
/// See [`SplitError`] — negative/over-unity fractions, non-unit fraction sum,
/// negative fixed flows, fixed flows exceeding the inlet, non-positive inlet
/// flow for a flow-spec mode, or an empty fraction spec.
pub fn resolve_fractions(
    spec: &SplitSpec,
    inlet_mass_flow_si: f64,
    inlet_mole_flow_si: f64,
) -> Result<Vec<Ratio>, SplitError> {
    match spec {
        SplitSpec::Fractions(fractions) => {
            if fractions.is_empty() {
                return Err(SplitError::NoOutlets);
            }
            let mut sum = 0.0;
            for (index, f) in fractions.iter().enumerate() {
                let value = f.get::<ratio>();
                if value < 0.0 {
                    return Err(SplitError::NegativeFraction { index, value });
                }
                sum += value;
            }
            if (sum - 1.0).abs() > FRACTION_SUM_TOLERANCE {
                return Err(SplitError::FractionSumNotUnity {
                    sum,
                    tolerance: FRACTION_SUM_TOLERANCE,
                });
            }
            Ok(fractions.clone())
        }
        SplitSpec::MassFlows(fixed) => {
            let fixed_si: Vec<f64> = fixed.iter().map(|w| w.get::<kilogram_per_second>()).collect();
            fractions_from_fixed_flows(&fixed_si, inlet_mass_flow_si)
        }
        SplitSpec::MoleFlows(fixed) => {
            let fixed_si: Vec<f64> = fixed.iter().map(|m| m.get::<katal>()).collect();
            fractions_from_fixed_flows(&fixed_si, inlet_mole_flow_si)
        }
    }
}

/// Shared helper for the two flow-spec modes: fixed flows to the leading
/// outlets, remainder `inlet − Σ fixed` to the last (Splitter.vb:293, :303, :349,
/// :359). Works in whatever SI unit both arguments share (kg/s or katal), since
/// the result is dimensionless fractions.
fn fractions_from_fixed_flows(fixed_si: &[f64], inlet_si: f64) -> Result<Vec<Ratio>, SplitError> {
    if inlet_si <= 0.0 {
        return Err(SplitError::NonPositiveInletFlow { value: inlet_si });
    }
    let mut fixed_total = 0.0;
    for (index, &value) in fixed_si.iter().enumerate() {
        if value < 0.0 {
            return Err(SplitError::NegativeFixedFlow { index, value });
        }
        fixed_total += value;
    }
    let remainder = inlet_si - fixed_total;
    if remainder < -FRACTION_SUM_TOLERANCE * inlet_si {
        return Err(SplitError::InsufficientInletFlow {
            inlet: inlet_si,
            fixed_total,
        });
    }
    // N outlets = (N-1) fixed + 1 remainder. Clamp a tiny negative remainder
    // (round-off within tolerance) to exactly zero.
    let mut fractions: Vec<Ratio> = fixed_si
        .iter()
        .map(|&w| Ratio::new::<ratio>(w / inlet_si))
        .collect();
    fractions.push(Ratio::new::<ratio>(remainder.max(0.0) / inlet_si));
    Ok(fractions)
}

/// Resolve a [`SplitSpec`] into per-outlet flows, given the inlet mass and mole
/// flows (DWSIM `Calculate`, Splitter.vb:237-391).
///
/// Computes the fractions via [`resolve_fractions`], then scales **both** the
/// inlet mass flow and inlet mole flow by each fraction:
/// `w_i = f_i · w_in` (Splitter.vb:268) and `m_i = f_i · m_in`
/// (Splitter.vb:381). This is exact because the splitter leaves the intensive
/// state (hence the mixture molar mass) uniform across all outlets, so mass and
/// mole flow scale by the identical fraction.
///
/// - `inlet_mass_flow` — inlet mass flow `w_in` \[kg/s\].
/// - `inlet_mole_flow` — inlet mole flow `m_in` \[katal = mol/s\].
///
/// # Errors
/// Propagates every [`SplitError`] from [`resolve_fractions`].
pub fn split(
    spec: &SplitSpec,
    inlet_mass_flow: MassRate,
    inlet_mole_flow: MolarFlowRate,
) -> Result<SplitResult, SplitError> {
    let w_in = inlet_mass_flow.get::<kilogram_per_second>();
    let m_in = inlet_mole_flow.get::<katal>();
    let fractions = resolve_fractions(spec, w_in, m_in)?;
    let mass_flows = fractions
        .iter()
        .map(|f| MassRate::new::<kilogram_per_second>(f.get::<ratio>() * w_in))
        .collect();
    let mole_flows = fractions
        .iter()
        .map(|f| MolarFlowRate::new::<katal>(f.get::<ratio>() * m_in))
        .collect();
    Ok(SplitResult {
        fractions,
        mass_flows,
        mole_flows,
    })
}

/// Build the full set of [`OutletStream`]s: every outlet carries the inlet's
/// [`IntensiveState`] unchanged, tagged with its own split flows
/// (DWSIM `Calculate`, Splitter.vb:254-276 and the analogous flow-spec loops).
///
/// This is the complete splitter: the intensive state (`inlet_state`) is cloned
/// verbatim onto each outlet — no flash is performed or needed (see the
/// module-level "Flash boundary" note) — while the flows come from [`split`].
///
/// - `inlet_state` — the inlet's intensive state (T, p, h, composition).
/// - `inlet_mass_flow` — inlet mass flow `w_in` \[kg/s\].
/// - `inlet_mole_flow` — inlet mole flow `m_in` \[katal = mol/s\].
///
/// # Errors
/// Propagates every [`SplitError`] from [`split`].
pub fn split_streams(
    spec: &SplitSpec,
    inlet_state: &IntensiveState,
    inlet_mass_flow: MassRate,
    inlet_mole_flow: MolarFlowRate,
) -> Result<Vec<OutletStream>, SplitError> {
    let result = split(spec, inlet_mass_flow, inlet_mole_flow)?;
    Ok(result
        .fractions
        .iter()
        .zip(result.mass_flows.iter())
        .zip(result.mole_flows.iter())
        .map(|((&f, &w), &m)| OutletStream {
            state: inlet_state.clone(),
            split_fraction: f,
            mass_flow: w,
            mole_flow: m,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    //! # Verification tests (methodology + measured results)
    //!
    //! Verification only — these check the port implements DWSIM's splitter
    //! algebra correctly against hand calculations, **not** validation against
    //! experiment or a physical benchmark (a splitter is a pure mass balance; no
    //! flash or property package is invoked here). Measured numbers recorded
    //! 2026-07-24.
    use super::*;
    use approx::assert_relative_eq;

    fn mass(kg_s: f64) -> MassRate {
        MassRate::new::<kilogram_per_second>(kg_s)
    }
    fn mole(mol_s: f64) -> MolarFlowRate {
        MolarFlowRate::new::<katal>(mol_s)
    }

    /// Methodology: a 30/70 fraction split of `w_in = 10 kg/s`
    /// (`m_in = 4 mol/s`). Hand values by `w_i = f_i · w_in` (Splitter.vb:268):
    /// `w_0 = 0.30·10 = 3.0`, `w_1 = 0.70·10 = 7.0 kg/s`; mole flows
    /// `m_0 = 0.30·4 = 1.2`, `m_1 = 0.70·4 = 2.8 mol/s`.
    /// Result (2026-07-24): mass = (3.000000, 7.000000) kg/s;
    /// mole = (1.200000, 2.800000) mol/s.
    #[test]
    fn thirty_seventy_fraction_split() {
        let spec = SplitSpec::Fractions(vec![Ratio::new::<ratio>(0.30), Ratio::new::<ratio>(0.70)]);
        let r = split(&spec, mass(10.0), mole(4.0)).unwrap();
        assert_relative_eq!(r.mass_flows[0].get::<kilogram_per_second>(), 3.0, epsilon = 1e-12);
        assert_relative_eq!(r.mass_flows[1].get::<kilogram_per_second>(), 7.0, epsilon = 1e-12);
        assert_relative_eq!(r.mole_flows[0].get::<katal>(), 1.2, epsilon = 1e-12);
        assert_relative_eq!(r.mole_flows[1].get::<katal>(), 2.8, epsilon = 1e-12);
    }

    /// Methodology: mass conservation `Σ w_i = w_in` for a 3-way fraction split
    /// `f = [0.2, 0.5, 0.3]` of `w_in = 12.5 kg/s`. Hand value: outlets
    /// `2.5, 6.25, 3.75`, summing to `12.5 kg/s`.
    /// Result (2026-07-24): Σ w_i = 12.500000 kg/s (matches inlet exactly);
    /// individual outlets 2.500000, 6.250000, 3.750000 kg/s.
    #[test]
    fn fraction_split_conserves_total_flow() {
        let spec = SplitSpec::Fractions(vec![
            Ratio::new::<ratio>(0.2),
            Ratio::new::<ratio>(0.5),
            Ratio::new::<ratio>(0.3),
        ]);
        let r = split(&spec, mass(12.5), mole(3.0)).unwrap();
        assert_relative_eq!(r.mass_flows[0].get::<kilogram_per_second>(), 2.5, epsilon = 1e-12);
        assert_relative_eq!(r.mass_flows[1].get::<kilogram_per_second>(), 6.25, epsilon = 1e-12);
        assert_relative_eq!(r.mass_flows[2].get::<kilogram_per_second>(), 3.75, epsilon = 1e-12);
        let total: f64 = r
            .mass_flows
            .iter()
            .map(|w| w.get::<kilogram_per_second>())
            .sum();
        assert_relative_eq!(total, 12.5, epsilon = 1e-12);
    }

    /// Methodology: mass-flow spec routes the remainder to the last outlet
    /// (DWSIM `StreamMassFlowSpec`, Splitter.vb:293 `wn(1) = W - w1`). Inlet
    /// `w_in = 10 kg/s`; fixed `w_0 = 3 kg/s`. Hand value: outlet 0 = 3.0,
    /// outlet 1 (remainder) = `10 − 3 = 7.0 kg/s`; fractions 0.3 and 0.7.
    /// A 3-outlet check: fixed `[2, 3]` of `10` → remainder `5 kg/s`.
    /// Result (2026-07-24): 2-way (3.000000, 7.000000) kg/s, fractions
    /// (0.300000, 0.700000); 3-way (2.000000, 3.000000, 5.000000) kg/s.
    #[test]
    fn mass_flow_spec_routes_remainder() {
        let spec = SplitSpec::MassFlows(vec![mass(3.0)]);
        let r = split(&spec, mass(10.0), mole(4.0)).unwrap();
        assert_relative_eq!(r.mass_flows[0].get::<kilogram_per_second>(), 3.0, epsilon = 1e-12);
        assert_relative_eq!(r.mass_flows[1].get::<kilogram_per_second>(), 7.0, epsilon = 1e-12);
        assert_relative_eq!(r.fractions[0].get::<ratio>(), 0.3, epsilon = 1e-12);
        assert_relative_eq!(r.fractions[1].get::<ratio>(), 0.7, epsilon = 1e-12);

        let spec3 = SplitSpec::MassFlows(vec![mass(2.0), mass(3.0)]);
        let r3 = split(&spec3, mass(10.0), mole(4.0)).unwrap();
        assert_relative_eq!(r3.mass_flows[0].get::<kilogram_per_second>(), 2.0, epsilon = 1e-12);
        assert_relative_eq!(r3.mass_flows[1].get::<kilogram_per_second>(), 3.0, epsilon = 1e-12);
        assert_relative_eq!(r3.mass_flows[2].get::<kilogram_per_second>(), 5.0, epsilon = 1e-12);
    }

    /// Methodology: mole-flow spec, same remainder logic on a mole basis
    /// (DWSIM `StreamMoleFlowSpec`, Splitter.vb:349 `mn(1) = M - m1`). Inlet
    /// `m_in = 4 mol/s`; fixed `m_0 = 1.5 mol/s`. Hand value: outlet 0 = 1.5,
    /// remainder = `4 − 1.5 = 2.5 mol/s`. Mass flows scale by the same fractions
    /// (0.375, 0.625) of `w_in = 8 kg/s` → 3.0, 5.0 kg/s.
    /// Result (2026-07-24): mole (1.500000, 2.500000) mol/s; mass
    /// (3.000000, 5.000000) kg/s.
    #[test]
    fn mole_flow_spec_routes_remainder() {
        let spec = SplitSpec::MoleFlows(vec![mole(1.5)]);
        let r = split(&spec, mass(8.0), mole(4.0)).unwrap();
        assert_relative_eq!(r.mole_flows[0].get::<katal>(), 1.5, epsilon = 1e-12);
        assert_relative_eq!(r.mole_flows[1].get::<katal>(), 2.5, epsilon = 1e-12);
        assert_relative_eq!(r.mass_flows[0].get::<kilogram_per_second>(), 3.0, epsilon = 1e-12);
        assert_relative_eq!(r.mass_flows[1].get::<kilogram_per_second>(), 5.0, epsilon = 1e-12);
    }

    /// Methodology: fractions that do not sum to 1 must error (this port's
    /// honest divergence from DWSIM's silent last-ratio overwrite,
    /// Splitter.vb:247-250). `[0.3, 0.5]` sums to 0.8 ≠ 1; a negative entry
    /// `[-0.1, 1.1]` must also error.
    /// Result (2026-07-24): `[0.3,0.5]` → `FractionSumNotUnity{sum:0.8}`;
    /// `[-0.1,1.1]` → `NegativeFraction{index:0}`; empty → `NoOutlets`.
    #[test]
    fn fractions_not_summing_to_one_error() {
        let spec = SplitSpec::Fractions(vec![Ratio::new::<ratio>(0.3), Ratio::new::<ratio>(0.5)]);
        match split(&spec, mass(10.0), mole(4.0)) {
            Err(SplitError::FractionSumNotUnity { sum, .. }) => {
                assert_relative_eq!(sum, 0.8, epsilon = 1e-12);
            }
            other => panic!("expected FractionSumNotUnity, got {other:?}"),
        }

        let neg = SplitSpec::Fractions(vec![Ratio::new::<ratio>(-0.1), Ratio::new::<ratio>(1.1)]);
        assert!(matches!(
            split(&neg, mass(10.0), mole(4.0)),
            Err(SplitError::NegativeFraction { index: 0, .. })
        ));

        let empty = SplitSpec::Fractions(vec![]);
        assert_eq!(split(&empty, mass(10.0), mole(4.0)), Err(SplitError::NoOutlets));
    }

    /// Methodology: fixed flows exceeding the inlet must error (DWSIM
    /// `Throw New Exception` when `W < Σ spec`, Splitter.vb:290-296). Fixed
    /// `[8, 5]` mass flows total 13 kg/s > inlet 10 kg/s → remainder negative.
    /// Result (2026-07-24): `InsufficientInletFlow{inlet:10, fixed_total:13}`.
    #[test]
    fn overdrawn_fixed_flow_errors() {
        let spec = SplitSpec::MassFlows(vec![mass(8.0), mass(5.0)]);
        match split(&spec, mass(10.0), mole(4.0)) {
            Err(SplitError::InsufficientInletFlow { inlet, fixed_total }) => {
                assert_relative_eq!(inlet, 10.0, epsilon = 1e-12);
                assert_relative_eq!(fixed_total, 13.0, epsilon = 1e-12);
            }
            other => panic!("expected InsufficientInletFlow, got {other:?}"),
        }
    }

    /// Methodology: the defining splitter property — every outlet inherits the
    /// inlet's intensive state unchanged (Splitter.vb:258-266). Inlet at
    /// `T = 350 K`, `p = 5e5 Pa`, `h = 1.2e6 J/kg`, composition
    /// `y = [0.4, 0.6]`, split 30/70. Each outlet's `state` must equal the
    /// inlet's exactly, while only the flows differ.
    /// Result (2026-07-24): both outlets carry T=350.000000 K, p=500000.000000
    /// Pa, h=1200000.000000 J/kg, y=(0.400000, 0.600000); flows 3.0 and 7.0
    /// kg/s. `state == inlet_state` for every outlet.
    #[test]
    fn intensive_state_preserved_on_every_outlet() {
        let inlet = IntensiveState::from_si(350.0, 5.0e5, 1.2e6, &[0.4, 0.6]);
        let spec = SplitSpec::Fractions(vec![Ratio::new::<ratio>(0.30), Ratio::new::<ratio>(0.70)]);
        let outlets = split_streams(&spec, &inlet, mass(10.0), mole(4.0)).unwrap();
        assert_eq!(outlets.len(), 2);
        for out in &outlets {
            // Intensive state identical to the inlet (T, p, h, composition).
            assert_eq!(out.state, inlet);
            assert_relative_eq!(out.state.temperature.get::<kelvin>(), 350.0, epsilon = 1e-9);
            assert_relative_eq!(out.state.pressure.get::<pascal>(), 5.0e5, epsilon = 1e-6);
            assert_relative_eq!(
                out.state.specific_enthalpy.get::<joule_per_kilogram>(),
                1.2e6,
                epsilon = 1e-3
            );
        }
        assert_relative_eq!(outlets[0].mass_flow.get::<kilogram_per_second>(), 3.0, epsilon = 1e-12);
        assert_relative_eq!(outlets[1].mass_flow.get::<kilogram_per_second>(), 7.0, epsilon = 1e-12);
    }

    /// Methodology: an empty `MassFlows` spec is a single pass-through outlet
    /// carrying the whole inlet (this port's sensible generalization of DWSIM's
    /// degenerate 1-outlet flow-spec case). Inlet 10 kg/s → one outlet at 10
    /// kg/s, fraction 1.
    /// Result (2026-07-24): one outlet, 10.000000 kg/s, fraction 1.000000.
    #[test]
    fn empty_mass_flow_spec_is_passthrough() {
        let spec = SplitSpec::MassFlows(vec![]);
        let r = split(&spec, mass(10.0), mole(4.0)).unwrap();
        assert_eq!(r.mass_flows.len(), 1);
        assert_relative_eq!(r.mass_flows[0].get::<kilogram_per_second>(), 10.0, epsilon = 1e-12);
        assert_relative_eq!(r.fractions[0].get::<ratio>(), 1.0, epsilon = 1e-12);
    }
}
