//! The `th` struct: core power, coolant inlet conditions and channel geometry
//! counts.
//!
//! # Provenance
//!
//! | | |
//! |---|---|
//! | Original author | Than Yan Ren, Singapore Nuclear Research and Safety Institute (SNRSI) |
//! | Source files | the `T-H input` block of `neacrpa2.m` / `neacrpa2t.m` / `neacrpa1t.m` / `neacrpd1.m`, and the inlet forcing of `neacrpd1t.m` |
//! | Snapshot | `BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…` |
//! | Permission | translation and open-source publication approved by the author and the project lead; see `docs/bedok-port-scoping.md` §6 |
//!
//! # Units
//!
//! Power \[W\], pressure \[MPa\], temperature \[K\], mass flux \[g/s/cm²\],
//! specific enthalpy \[kJ/kg\] (numerically equal to the code's J/g).

/// Direction of coolant flow through the core.
///
/// MATLAB `th.flowdir`, `+1` for upwards and `-1` for downwards. Both NEACRP
/// cases are upflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowDirection {
    /// Inlet at the bottom of the model. MATLAB `th.flowdir = 1`.
    Upward,
    /// Inlet at the top of the model. MATLAB `th.flowdir = -1`.
    Downward,
}

impl FlowDirection {
    /// The MATLAB sign, `+1` or `-1`.
    #[must_use]
    pub const fn sign(self) -> f64 {
        match self {
            Self::Upward => 1.0,
            Self::Downward => -1.0,
        }
    }
}

/// How the coolant inlet temperature is specified.
///
/// MATLAB writes `th.coolant.inlettemp` as a number in the PWR cases, but the
/// BWR case computes it from a **subcooling below saturation**:
///
/// ```text
/// tsat = IAPWS_IF97('Tsat_p', 6.7);
/// hsat = IAPWS_IF97('h1_pT',  6.7, tsat);
/// th.coolant.inlettemp = IAPWS_IF97('T_ph', 6.7, hsat - 46.52);
/// ```
///
/// # Why the specification is kept rather than only its value
///
/// `docs/bedok-port-scoping.md` §3 decides that `IAPWS_IF97.m` is **not**
/// ported and that steam properties come from `tampines-steam-tables`, through
/// `reference::th::steam`. The flash *is* evaluated — see
/// [`evaluate_kelvin`](Self::evaluate_kelvin) — but the pressure and enthalpy
/// deficit are what the benchmark actually specifies, so they are what is
/// stored. That also makes the one allowed substitution visible at the point
/// it happens instead of baked into a literal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoolantInletTemperature {
    /// A temperature stated directly \[K\]. MATLAB
    /// `th.coolant.inlettemp = 559.15`.
    Fixed(f64),
    /// Subcooled by a stated enthalpy below the saturated-liquid enthalpy at
    /// the system pressure.
    SubcooledBelowSaturation {
        /// System pressure \[MPa\].
        pressure_mpa: f64,
        /// Enthalpy below saturated liquid \[kJ/kg\]; `46.52` for NEACRP D1.
        enthalpy_deficit_kj_per_kg: f64,
    },
}

impl CoolantInletTemperature {
    /// The inlet temperature \[K\].
    ///
    /// For [`Fixed`](Self::Fixed) this is the stored number. For
    /// [`SubcooledBelowSaturation`](Self::SubcooledBelowSaturation) it is the
    /// three-step evaluation of `neacrpd1.m`:
    ///
    /// ```text
    /// tsat = IAPWS_IF97('Tsat_p', p);
    /// hsat = IAPWS_IF97('h1_pT',  p, tsat);
    ///        IAPWS_IF97('T_ph',   p, hsat - deficit);
    /// ```
    ///
    /// carried out by [`crate::reference::th::steam`], i.e. by
    /// `tampines-steam-tables` rather than by the unported `IAPWS_IF97.m`.
    /// This is the single substitution §3 permits inside the reference path;
    /// its parity gate is agreement of `tampines-steam-tables` with the
    /// published IF97 verification values over the benchmarks' operating
    /// envelope, not agreement with Mifofski's implementation.
    ///
    /// Returns `NaN` if the state falls outside the IF97 backward-equation
    /// envelope, matching what the MATLAB returns there.
    #[must_use]
    pub fn evaluate_kelvin(self) -> f64 {
        match self {
            Self::Fixed(t) => t,
            Self::SubcooledBelowSaturation {
                pressure_mpa,
                enthalpy_deficit_kj_per_kg,
            } => {
                let h_saturated =
                    crate::reference::th::steam::saturated_liquid_enthalpy(pressure_mpa);
                crate::reference::th::steam::temperature_ph(
                    pressure_mpa,
                    h_saturated - enthalpy_deficit_kj_per_kg,
                )
            }
        }
    }
}

/// The NEACRP D1 inlet cold-water-injection forcing.
///
/// Rust translation of `th.inlettemp_t` in `neacrpd1t.m`. NEACRP-L-335
/// Figure 6.1 doubles the inlet subcooling with a 2.5 s time constant:
///
/// ```text
/// dH(t) = 46.52 * (2 - exp(-0.4 t))   kJ/kg
/// ```
///
/// so `dH(0) = 46.52`, exactly the steady inlet of `neacrpd1.m` — the forcing
/// is continuous at `t = 0` — rising to `93.04 kJ/kg`. The core pressure and
/// the inlet mass flow are constant throughout, and there is no rod motion.
///
/// This type supplies the enthalpy history the MATLAB feeds to
/// `IAPWS_IF97('T_ph', …)`; [`inlet_at`](Self::inlet_at) turns a time into the
/// [`CoolantInletTemperature`] whose
/// [`evaluate_kelvin`](CoolantInletTemperature::evaluate_kelvin) performs the
/// flash.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColdWaterInjection {
    /// Core pressure, constant through the transient \[MPa\].
    pub pressure_mpa: f64,
    /// Steady-state subcooling \[kJ/kg\]; the `46.52` of Figure 6.1.
    pub steady_deficit_kj_per_kg: f64,
    /// Asymptotic multiple of the steady subcooling \[dimensionless\]; `2`.
    pub asymptotic_multiple: f64,
    /// Exponential rate \[1/s\]; `0.4`, i.e. a 2.5 s time constant.
    pub rate_per_second: f64,
}

impl ColdWaterInjection {
    /// The Figure 6.1 forcing of NEACRP D1.
    #[must_use]
    pub const fn neacrp_d1() -> Self {
        Self {
            pressure_mpa: 6.7,
            steady_deficit_kj_per_kg: 46.52,
            asymptotic_multiple: 2.0,
            rate_per_second: 0.4,
        }
    }

    /// Inlet enthalpy deficit below saturated liquid at time `t` \[s\], in
    /// \[kJ/kg\].
    ///
    /// `dH(t) = steady * (multiple - exp(-rate*t))`.
    #[must_use]
    pub fn enthalpy_deficit_kj_per_kg(&self, t: f64) -> f64 {
        self.steady_deficit_kj_per_kg
            * (self.asymptotic_multiple - (-self.rate_per_second * t).exp())
    }

    /// The inlet condition at time `t` \[s\], in the form the steam-table
    /// layer can flash to a temperature.
    #[must_use]
    pub fn inlet_at(&self, t: f64) -> CoolantInletTemperature {
        CoolantInletTemperature::SubcooledBelowSaturation {
            pressure_mpa: self.pressure_mpa,
            enthalpy_deficit_kj_per_kg: self.enthalpy_deficit_kj_per_kg(t),
        }
    }
}

/// Coolant inlet state.
///
/// MATLAB `th.coolant`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoolantInlet {
    /// System pressure \[MPa\]. MATLAB `th.coolant.inletpress`; 15.5 MPa for
    /// the PWR cases, 6.7 MPa for the BWR case.
    pub pressure_mpa: f64,
    /// Inlet temperature, stated or specified as a subcooling.
    /// MATLAB `th.coolant.inlettemp`.
    pub temperature: CoolantInletTemperature,
    /// Inlet volumetric gas fraction \[dimensionless\]. MATLAB
    /// `th.coolant.inletvoid`.
    ///
    /// Both cases set `1e-14` rather than zero: the two-phase closures divide
    /// by the void fraction, so a hard zero would be a division by zero. That
    /// is a numerical guard in the reference, not a physical statement.
    pub inlet_void: f64,
}

/// The `th` struct: everything the thermal-hydraulic solver needs that is not
/// geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct ThermalHydraulics {
    /// Rated thermal power of the modelled sector \[W\]. MATLAB `th.maxpow`.
    ///
    /// The PWR cases model a core quarter and give `693.75e6 W`; the BWR case
    /// gives `1800e6/4 W`.
    pub max_power_watt: f64,
    /// Fraction of rated power the case runs at \[dimensionless\]. MATLAB
    /// `th.powratio`; `1` at full power, `1e-6` for the hot-zero-power case A1.
    pub power_ratio: f64,
    /// Fraction of the fission energy deposited directly in the coolant
    /// \[dimensionless\]. MATLAB `th.coolheatfrac`; `0.019` in every case.
    pub coolant_heat_fraction: f64,
    /// Inlet state.
    pub coolant: CoolantInlet,
    /// Area-averaged coolant mass flux \[g/s/cm²\]. MATLAB `th.flowrate`.
    pub mass_flux_g_per_s_cm2: f64,
    /// Flow direction. MATLAB `th.flowdir`.
    pub flow_direction: FlowDirection,
    /// Fuel pins per radial node, after dividing by the radial refinement
    /// factors \[dimensionless count\]. MATLAB `th.nfuelpin`, which is
    /// assigned as the per-assembly count and then divided by
    /// `xscale*yscale`.
    pub fuel_pins_per_node: f64,
    /// Guide tubes per radial node \[dimensionless count\]. MATLAB
    /// `th.gtube`; `25` in both cases.
    ///
    /// # Unfinished in the reference
    ///
    /// Unlike `th.nfuelpin`, this is **not** divided by the refinement
    /// factors, so on a refined grid the guide-tube count per node stays at
    /// the whole-assembly value while the pin count shrinks. Recorded, not
    /// fixed. It has no effect at the native 17 × 17 mesh, where both scales
    /// are 1.
    pub guide_tubes_per_node: f64,
    /// Time-dependent inlet forcing, if the case has one. MATLAB
    /// `th.inlettemp_t`, set only by `neacrpd1t.m`.
    pub inlet_forcing: Option<ColdWaterInjection>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_direction_signs_match_matlab() {
        assert_eq!(FlowDirection::Upward.sign(), 1.0);
        assert_eq!(FlowDirection::Downward.sign(), -1.0);
    }

    /// The Figure 6.1 forcing is continuous with the steady inlet at t=0 and
    /// doubles the subcooling asymptotically.
    #[test]
    fn cold_water_forcing_starts_at_the_steady_subcooling() {
        let f = ColdWaterInjection::neacrp_d1();
        assert!((f.enthalpy_deficit_kj_per_kg(0.0) - 46.52).abs() < 1e-12);
        assert!((f.enthalpy_deficit_kj_per_kg(1000.0) - 93.04).abs() < 1e-9);
        // 2.5 s time constant: one e-folding at t = 2.5 s.
        let at_tau = f.enthalpy_deficit_kj_per_kg(2.5);
        let expected = 46.52 * (2.0 - (-1.0f64).exp());
        assert!((at_tau - expected).abs() < 1e-12);
    }

    #[test]
    fn inlet_at_carries_the_pressure_through() {
        let f = ColdWaterInjection::neacrp_d1();
        match f.inlet_at(0.0) {
            CoolantInletTemperature::SubcooledBelowSaturation {
                pressure_mpa,
                enthalpy_deficit_kj_per_kg,
            } => {
                assert_eq!(pressure_mpa, 6.7);
                assert!((enthalpy_deficit_kj_per_kg - 46.52).abs() < 1e-12);
            }
            CoolantInletTemperature::Fixed(_) => panic!("expected a subcooling specification"),
        }
    }
}
