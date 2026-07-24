//! Mixer: adiabatic multi-stream mass and energy balance.
//!
//! Pure-Rust port of DWSIM's stream mixer, from
//! `UnitOperations/Mixer.vb` (GPL-3.0, `Public Overrides Sub Calculate`,
//! lines 85-241). Upstream copyright: 2008 Daniel Wagner O. de Medeiros.
//!
//! A mixer combines up to N inlet material streams into one outlet, closing the
//! steady-state **mass** and **adiabatic energy** balances:
//!
//! - **Total mass flow** — `w = Σ w_i` (Mixer.vb:143-144, `W += We`).
//! - **Mixed specific enthalpy** — `h = Σ(w_i·h_i) / Σ w_i`, the adiabatic
//!   (zero heat loss, zero shaft work) energy balance (Mixer.vb:145 `H += We*enthalpy`,
//!   :165 `Hs = H / W`).
//! - **Outlet pressure** — one of three user-selected modes ported as the
//!   [`PressureBehavior`] enum: Minimum, Maximum, or Average of the inlet
//!   pressures (Mixer.vb:40-44 `Enum PressureBehavior`, :124-139, :167).
//! - **Composition** — mass-flow-weighted mass fractions, with an optional
//!   mass-to-mole-fraction conversion, ported as free functions
//!   ([`mixed_mass_fraction`], [`mass_to_mole_fractions`]; Mixer.vb:184-232).
//!
//! # Flash boundary (pushed to the caller)
//!
//! DWSIM finishes by writing `(P, Hs, W)` onto the outlet stream and letting the
//! flowsheet solver run a **pressure-enthalpy (PH) flash** to recover the outlet
//! temperature and phase split (Mixer.vb:98-100, :234
//! `StreamSpec.Pressure_and_Enthalpy`). This crate has no property-package /
//! flash access of its own (see the crate top-level docs), so — exactly as in
//! [`crate::expander`] — the flash is **not** performed here. [`mix`] returns the
//! mixed [`MixerOutlet`] `(pressure, specific_enthalpy, mass_flow)`, and the
//! caller runs the PH flash on `(p_out, h)` to obtain outlet `T` and phase.
//!
//! # Excluded DWSIM behavior
//!
//! Deliberately **not** ported (GUI / solver / persistence plumbing, no physics):
//! the `EditingForm_Mixer` editor and icon/bitmap resources (Mixer.vb:38, :302-338),
//! XML/JSON serialization (`CloneXML`/`CloneJSON`/`SaveData`, :69-77), the
//! flowsheet `Inspector` trace paragraphs (:87-101, :120-142, :201-205), the
//! dynamic-mode backward pressure propagation (:169-179), `DeCalculate` outlet
//! clearing (:243-261), and the property-grid reflection accessors
//! (`GetPropertyValue`/`GetProperties`/`GetPropertyUnit`, :263-300). DWSIM's
//! six-inlet GUI cap is dropped — this port accepts any number of inlets.

use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{AvailableEnergy, MassRate, MolarMass, Pressure, Ratio};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;

/// Outlet-pressure calculation mode — DWSIM's `Mixer.PressureBehavior` enum
/// (Mixer.vb:40-44). Selects how the single outlet pressure is derived from the
/// inlet pressures. Modeled as an enum (no `dyn` dispatch), per the workspace
/// design rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureBehavior {
    /// Outlet pressure = minimum of the inlet pressures (Mixer.vb:124-129).
    /// This is DWSIM's default (`m_pressurebehavior = PressureBehavior.Minimum`,
    /// Mixer.vb:46) and the physically conservative choice: a passive tee cannot
    /// raise a stream above the lowest feed pressure without a pump/compressor.
    Minimum,
    /// Outlet pressure = maximum of the inlet pressures (Mixer.vb:130-135).
    Maximum,
    /// Outlet pressure = arithmetic mean of the inlet pressures
    /// (Mixer.vb:136-139 accumulate, :167 `P = P / (i - 1)`).
    Average,
}

/// One inlet material stream to the mixer, owned by value (no references, no
/// lifetimes). Only the quantities the mass/energy/pressure balance needs are
/// carried; composition is handled separately (see module docs).
///
/// Units (all SI, `uom`-typed):
/// - `mass_flow` — mass flow rate `w_i`, kg/s. Must be finite and `>= 0`.
/// - `specific_enthalpy` — specific enthalpy `h_i`, J/kg (mass basis, matching
///   DWSIM's `Phases(0).Properties.enthalpy`). Any real value; the datum only
///   needs to be consistent across all inlets and the caller's outlet flash.
/// - `pressure` — stream pressure `p_i`, Pa. Must be finite and `> 0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InletStream {
    /// Mass flow rate `w_i` \[kg/s\], `>= 0`.
    pub mass_flow: MassRate,
    /// Specific enthalpy `h_i` \[J/kg\], mass basis.
    pub specific_enthalpy: AvailableEnergy,
    /// Stream pressure `p_i` \[Pa\], `> 0`.
    pub pressure: Pressure,
}

impl InletStream {
    /// Convenience constructor from SI scalars: `mass_flow` \[kg/s\],
    /// `specific_enthalpy` \[J/kg\], `pressure` \[Pa\].
    pub fn from_si(mass_flow: f64, specific_enthalpy: f64, pressure: f64) -> Self {
        Self {
            mass_flow: MassRate::new::<kilogram_per_second>(mass_flow),
            specific_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(specific_enthalpy),
            pressure: Pressure::new::<pascal>(pressure),
        }
    }
}

/// The mixed outlet state produced by [`mix`]. These are exactly the three
/// quantities DWSIM writes onto the outlet stream before the PH flash
/// (Mixer.vb:211-213). Outlet temperature and phase are **not** here — the
/// caller obtains them by flashing `(pressure, specific_enthalpy)` (see the
/// module-level "Flash boundary" note).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MixerOutlet {
    /// Outlet pressure `p_out` \[Pa\], per the chosen [`PressureBehavior`].
    pub pressure: Pressure,
    /// Mixed specific enthalpy `h = Σ(w_i·h_i)/Σ w_i` \[J/kg\], mass basis.
    pub specific_enthalpy: AvailableEnergy,
    /// Total mass flow `w = Σ w_i` \[kg/s\].
    pub mass_flow: MassRate,
}

/// Errors from the mixer balance. Ported behavior differs from DWSIM here: DWSIM
/// silently substitutes `Hs = 0` / `T = 273.15 K` when the total mass flow is
/// zero (Mixer.vb:165, :199); this port instead reports the condition, because a
/// specific enthalpy is genuinely undefined for zero flow (`0/0`), and hiding
/// that would violate the workspace honesty rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixerError {
    /// No inlet streams were supplied (empty slice). DWSIM guards the analogous
    /// "no attached streams" case by throwing (Mixer.vb:102-104, :121).
    NoInlets,
    /// The total mass flow is zero, so the mixed specific enthalpy
    /// `Σ(w_i·h_i)/Σ w_i` is `0/0` and undefined.
    ZeroTotalMassFlow,
}

impl core::fmt::Display for MixerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MixerError::NoInlets => write!(f, "mixer has no inlet streams"),
            MixerError::ZeroTotalMassFlow => {
                write!(f, "mixer total mass flow is zero; mixed enthalpy is undefined")
            }
        }
    }
}

impl std::error::Error for MixerError {}

/// Total outlet mass flow `w = Σ w_i` (Mixer.vb:143-144).
///
/// Returns `0 kg/s` for an empty slice (an empty sum), so this is
/// error-free by construction. Units: kg/s in, kg/s out.
pub fn total_mass_flow(inlets: &[InletStream]) -> MassRate {
    inlets
        .iter()
        .fold(MassRate::new::<kilogram_per_second>(0.0), |acc, s| {
            acc + s.mass_flow
        })
}

/// Mixed (flow-weighted) specific enthalpy `h = Σ(w_i·h_i) / Σ w_i` — the
/// adiabatic energy balance (Mixer.vb:145 `H += We*enthalpy`, :165 `Hs = H/W`).
///
/// This is the mass-flow-weighted mean of the inlet specific enthalpies; it
/// assumes no heat exchange with the surroundings and no shaft work (adiabatic
/// tee). Units: J/kg out.
///
/// # Errors
/// - [`MixerError::NoInlets`] if `inlets` is empty.
/// - [`MixerError::ZeroTotalMassFlow`] if `Σ w_i == 0` (enthalpy is `0/0`).
pub fn mixed_specific_enthalpy(inlets: &[InletStream]) -> Result<AvailableEnergy, MixerError> {
    if inlets.is_empty() {
        return Err(MixerError::NoInlets);
    }
    let w = total_mass_flow(inlets).get::<kilogram_per_second>();
    if w == 0.0 {
        return Err(MixerError::ZeroTotalMassFlow);
    }
    // Σ(w_i·h_i): kg/s · J/kg = W (J/s). Divide by kg/s → J/kg.
    let h_flow: f64 = inlets
        .iter()
        .map(|s| {
            s.mass_flow.get::<kilogram_per_second>()
                * s.specific_enthalpy.get::<joule_per_kilogram>()
        })
        .sum();
    Ok(AvailableEnergy::new::<joule_per_kilogram>(h_flow / w))
}

/// Outlet pressure per the selected [`PressureBehavior`] (Mixer.vb:124-139, :167).
///
/// This port takes the plain min / max / arithmetic-mean over the actual inlet
/// pressures. DWSIM's imperative code uses a `P = 0` "unset" sentinel while
/// scanning (Mixer.vb:125-135), which is an initialization artifact, not a
/// physical rule; the set-based min/max/mean here is equivalent for physical
/// (`p_i > 0`) inlets and is what DWSIM intends. Units: Pa out.
///
/// # Errors
/// [`MixerError::NoInlets`] if `inlets` is empty (min/max/mean undefined).
pub fn outlet_pressure(
    inlets: &[InletStream],
    mode: PressureBehavior,
) -> Result<Pressure, MixerError> {
    if inlets.is_empty() {
        return Err(MixerError::NoInlets);
    }
    let pressures = inlets.iter().map(|s| s.pressure.get::<pascal>());
    let p = match mode {
        PressureBehavior::Minimum => pressures.fold(f64::INFINITY, f64::min),
        PressureBehavior::Maximum => pressures.fold(f64::NEG_INFINITY, f64::max),
        PressureBehavior::Average => {
            let sum: f64 = pressures.sum();
            sum / inlets.len() as f64
        }
    };
    Ok(Pressure::new::<pascal>(p))
}

/// Full adiabatic mixer balance: combine `inlets` into one [`MixerOutlet`]
/// `(pressure, specific_enthalpy, mass_flow)` using pressure mode `mode`
/// (Mixer.vb `Calculate`, :85-241).
///
/// A single inlet is handled as a passthrough (its mass flow, enthalpy, and
/// pressure carry straight through — min/max/mean of one value is that value),
/// mirroring DWSIM's single-source shortcut (Mixer.vb:149-162, `isSS`).
///
/// The returned state is the **input to a caller-side PH flash** — see the
/// module-level "Flash boundary" note — which recovers outlet temperature and
/// phase from `(pressure, specific_enthalpy)`.
///
/// # Errors
/// - [`MixerError::NoInlets`] if `inlets` is empty.
/// - [`MixerError::ZeroTotalMassFlow`] if `Σ w_i == 0`.
pub fn mix(inlets: &[InletStream], mode: PressureBehavior) -> Result<MixerOutlet, MixerError> {
    let specific_enthalpy = mixed_specific_enthalpy(inlets)?;
    let pressure = outlet_pressure(inlets, mode)?;
    let mass_flow = total_mass_flow(inlets);
    Ok(MixerOutlet {
        pressure,
        specific_enthalpy,
        mass_flow,
    })
}

/// Mixed mass fraction of **one compound** across the inlets:
/// `x_out = Σ(w_i · x_i) / Σ w_i` (DWSIM `Vw` accumulation, Mixer.vb:193, and
/// normalization at :219 `comp.MassFraction = Vw(comp.Name) / W`).
///
/// `inlet_flows[i]` is inlet `i`'s total mass flow `w_i`; `inlet_mass_fractions[i]`
/// is that compound's mass fraction `x_i` in inlet `i` (dimensionless \[0, 1\]).
/// The two slices must be the same length. This is composition mixing on a mass
/// basis; call it once per compound to build the full outlet composition vector.
///
/// # Errors
/// - [`MixerError::NoInlets`] if the slices are empty.
/// - [`MixerError::ZeroTotalMassFlow`] if `Σ w_i == 0`.
///
/// # Panics
/// Panics if `inlet_flows.len() != inlet_mass_fractions.len()`.
pub fn mixed_mass_fraction(
    inlet_flows: &[MassRate],
    inlet_mass_fractions: &[Ratio],
) -> Result<Ratio, MixerError> {
    assert_eq!(
        inlet_flows.len(),
        inlet_mass_fractions.len(),
        "flow and mass-fraction slices must have equal length"
    );
    if inlet_flows.is_empty() {
        return Err(MixerError::NoInlets);
    }
    let w: f64 = inlet_flows.iter().map(|f| f.get::<kilogram_per_second>()).sum();
    if w == 0.0 {
        return Err(MixerError::ZeroTotalMassFlow);
    }
    let weighted: f64 = inlet_flows
        .iter()
        .zip(inlet_mass_fractions.iter())
        .map(|(f, x)| f.get::<kilogram_per_second>() * x.get::<ratio>())
        .sum();
    Ok(Ratio::new::<ratio>(weighted / w))
}

/// Convert a mixed mass-fraction vector to mole fractions given each compound's
/// molar mass (DWSIM Mixer.vb:221-232):
/// `y_i = (x_i / M_i) / Σ_j (x_j / M_j)`.
///
/// `mass_fractions[i]` and `molar_weights[i]` describe the same compound `i`.
/// The `M_i` unit cancels in the ratio, so any consistent molar-mass unit is
/// fine (`uom` `MolarMass`, base kg/mol). Returns a mole-fraction vector that
/// sums to 1 (dimensionless \[0, 1\]).
///
/// # Panics
/// Panics if the two slices differ in length. Returns an empty vector for empty
/// input. If `Σ_j (x_j / M_j) == 0` (e.g. all-zero mass fractions), the result
/// entries are `NaN` — the caller should mix nonzero compositions.
pub fn mass_to_mole_fractions(
    mass_fractions: &[Ratio],
    molar_weights: &[MolarMass],
) -> Vec<Ratio> {
    assert_eq!(
        mass_fractions.len(),
        molar_weights.len(),
        "mass-fraction and molar-weight slices must have equal length"
    );
    use uom::si::molar_mass::kilogram_per_mole;
    // Σ_j x_j / M_j  (DWSIM `mass_div_mm`, Mixer.vb:221-225).
    let mass_div_mm: f64 = mass_fractions
        .iter()
        .zip(molar_weights.iter())
        .map(|(x, m)| x.get::<ratio>() / m.get::<kilogram_per_mole>())
        .sum();
    mass_fractions
        .iter()
        .zip(molar_weights.iter())
        .map(|(x, m)| {
            Ratio::new::<ratio>(x.get::<ratio>() / m.get::<kilogram_per_mole>() / mass_div_mm)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! # Verification tests (methodology + measured results)
    //!
    //! Verification only — these check the port implements DWSIM's algebra
    //! correctly against hand calculations, **not** validation against
    //! experiment or a physical benchmark (no flash/property package is invoked
    //! here). Measured numbers recorded 2026-07-24.
    use super::*;
    use approx::assert_relative_eq;
    use uom::si::molar_mass::kilogram_per_mole;

    /// Methodology: two inlets, `w1 = 2`, `w2 = 3` kg/s. Total = 5 kg/s by
    /// `w = Σ w_i` (Mixer.vb:143-144).
    /// Result (2026-07-24): `total_mass_flow = 5.000000 kg/s` (exact).
    #[test]
    fn two_stream_mass_sum() {
        let inlets = [
            InletStream::from_si(2.0, 100_000.0, 2.0e5),
            InletStream::from_si(3.0, 200_000.0, 5.0e5),
        ];
        assert_relative_eq!(
            total_mass_flow(&inlets).get::<kilogram_per_second>(),
            5.0,
            epsilon = 1e-12
        );
    }

    /// Methodology: flow-weighted enthalpy `h = Σ(w_i·h_i)/Σ w_i`
    /// (Mixer.vb:145, :165). Hand value: `(2·100000 + 3·200000)/5 =
    /// 800000/5 = 160000 J/kg`.
    /// Result (2026-07-24): `mixed_specific_enthalpy = 160000.000000 J/kg`.
    #[test]
    fn enthalpy_is_flow_weighted_average() {
        let inlets = [
            InletStream::from_si(2.0, 100_000.0, 2.0e5),
            InletStream::from_si(3.0, 200_000.0, 5.0e5),
        ];
        let h = mixed_specific_enthalpy(&inlets).unwrap();
        assert_relative_eq!(h.get::<joule_per_kilogram>(), 160_000.0, epsilon = 1e-9);
    }

    /// Methodology: three pressure modes over inlets at 2e5 and 5e5 Pa
    /// (Mixer.vb:124-139, :167). Hand values: Minimum = 2e5, Maximum = 5e5,
    /// Average = (2e5+5e5)/2 = 3.5e5 Pa.
    /// Result (2026-07-24): Minimum 200000, Maximum 500000, Average 350000 Pa.
    #[test]
    fn each_pressure_mode_picks_correct_outlet_pressure() {
        let inlets = [
            InletStream::from_si(2.0, 100_000.0, 2.0e5),
            InletStream::from_si(3.0, 200_000.0, 5.0e5),
        ];
        assert_relative_eq!(
            outlet_pressure(&inlets, PressureBehavior::Minimum)
                .unwrap()
                .get::<pascal>(),
            2.0e5,
            epsilon = 1e-6
        );
        assert_relative_eq!(
            outlet_pressure(&inlets, PressureBehavior::Maximum)
                .unwrap()
                .get::<pascal>(),
            5.0e5,
            epsilon = 1e-6
        );
        assert_relative_eq!(
            outlet_pressure(&inlets, PressureBehavior::Average)
                .unwrap()
                .get::<pascal>(),
            3.5e5,
            epsilon = 1e-6
        );
    }

    /// Methodology: a single inlet must pass through unchanged (DWSIM `isSS`
    /// single-source shortcut, Mixer.vb:149-162). Inlet: `w=4`, `h=150000`,
    /// `p=3e5`. Under any pressure mode min/max/mean of one value is itself.
    /// Result (2026-07-24): outlet = (300000 Pa, 150000 J/kg, 4 kg/s).
    #[test]
    fn single_stream_passthrough() {
        let inlets = [InletStream::from_si(4.0, 150_000.0, 3.0e5)];
        for mode in [
            PressureBehavior::Minimum,
            PressureBehavior::Maximum,
            PressureBehavior::Average,
        ] {
            let out = mix(&inlets, mode).unwrap();
            assert_relative_eq!(out.mass_flow.get::<kilogram_per_second>(), 4.0, epsilon = 1e-12);
            assert_relative_eq!(
                out.specific_enthalpy.get::<joule_per_kilogram>(),
                150_000.0,
                epsilon = 1e-9
            );
            assert_relative_eq!(out.pressure.get::<pascal>(), 3.0e5, epsilon = 1e-6);
        }
    }

    /// Methodology: empty input and zero-total-flow error handling (this port's
    /// honest divergence from DWSIM's silent `Hs=0` fallback, Mixer.vb:165).
    /// Result (2026-07-24): empty slice → `NoInlets`; two zero-flow inlets →
    /// `ZeroTotalMassFlow` from `mixed_specific_enthalpy`/`mix`; pressure alone
    /// is still well-defined for zero-flow inlets.
    #[test]
    fn empty_and_zero_flow_error_handling() {
        let empty: [InletStream; 0] = [];
        assert_eq!(mixed_specific_enthalpy(&empty), Err(MixerError::NoInlets));
        assert_eq!(
            outlet_pressure(&empty, PressureBehavior::Minimum),
            Err(MixerError::NoInlets)
        );
        assert_eq!(mix(&empty, PressureBehavior::Average), Err(MixerError::NoInlets));

        let zero_flow = [
            InletStream::from_si(0.0, 100_000.0, 2.0e5),
            InletStream::from_si(0.0, 200_000.0, 5.0e5),
        ];
        assert_eq!(
            mixed_specific_enthalpy(&zero_flow),
            Err(MixerError::ZeroTotalMassFlow)
        );
        assert_eq!(
            mix(&zero_flow, PressureBehavior::Average),
            Err(MixerError::ZeroTotalMassFlow)
        );
        // Pressure is still defined even at zero flow.
        assert_relative_eq!(
            outlet_pressure(&zero_flow, PressureBehavior::Minimum)
                .unwrap()
                .get::<pascal>(),
            2.0e5,
            epsilon = 1e-6
        );
    }

    /// Methodology: composition mixing on a mass basis (Mixer.vb:193, :219) and
    /// mass→mole conversion (Mixer.vb:221-232). Two inlets, one compound:
    /// `w1=2, x1=0.10`; `w2=3, x2=0.60`. Hand value:
    /// `x_out = (2·0.10 + 3·0.60)/5 = (0.2+1.8)/5 = 2.0/5 = 0.40`.
    /// Mass→mole for a mix `x = [0.40, 0.60]`, `M = [0.018, 0.032] kg/mol`:
    /// `d = 0.40/0.018 + 0.60/0.032 = 22.2222 + 18.7500 = 40.9722`;
    /// `y1 = 22.2222/40.9722 = 0.542370`, `y2 = 18.7500/40.9722 = 0.457630`.
    /// Result (2026-07-24): x_out 0.400000; y = (0.542370, 0.457630), Σy = 1.
    #[test]
    fn composition_mass_and_mole_mixing() {
        let flows = [
            MassRate::new::<kilogram_per_second>(2.0),
            MassRate::new::<kilogram_per_second>(3.0),
        ];
        let x = [Ratio::new::<ratio>(0.10), Ratio::new::<ratio>(0.60)];
        let x_out = mixed_mass_fraction(&flows, &x).unwrap();
        assert_relative_eq!(x_out.get::<ratio>(), 0.40, epsilon = 1e-12);

        let mass_fractions = [Ratio::new::<ratio>(0.40), Ratio::new::<ratio>(0.60)];
        let molar_weights = [
            MolarMass::new::<kilogram_per_mole>(0.018),
            MolarMass::new::<kilogram_per_mole>(0.032),
        ];
        let y = mass_to_mole_fractions(&mass_fractions, &molar_weights);
        assert_relative_eq!(y[0].get::<ratio>(), 0.542370, epsilon = 1e-5);
        assert_relative_eq!(y[1].get::<ratio>(), 0.457630, epsilon = 1e-5);
        assert_relative_eq!(
            y[0].get::<ratio>() + y[1].get::<ratio>(),
            1.0,
            epsilon = 1e-12
        );
    }
}
