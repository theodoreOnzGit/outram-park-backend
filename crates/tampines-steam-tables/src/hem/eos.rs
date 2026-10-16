//! The equation-of-state contract a [`super::HemPipe`] is closed with, and the
//! calorically perfect gas that runs the Sod limit through the same solver.
//!
//! # What belongs here
//!
//! - [`HemEquationOfState`]: a **compile-time** trait (no `dyn`, per the
//!   workspace rule). The solver is generic over it; dispatch over several
//!   backends at run time is done with an enum one level up (`tampines`'
//!   `HemEos`, which adds CoolProp).
//! - [`HemThermoState`]: what every closure call returns.
//! - [`BreakFlow`]: what a critical-flow model returns for a choked break.
//! - [`IdealGasHem`]: `p = (γ − 1)ρe`, for verification.
//!
//! The IAPWS-IF97 implementation is [`super::Iapws97Hem`].
//!
//! # The sound speed is the EQUILIBRIUM one
//!
//! Every implementation must return the **homogeneous-equilibrium** sound
//! speed `c² = (∂p/∂ρ)_s` along the equilibrium isentrope, in which liquid
//! flashes and vapour condenses inside the wave. Inside the two-phase dome it
//! is far below the frozen (Wood–Wallis) speed. The solver places its KNP
//! characteristics `u ± c` and its Courant number with it, so a frozen speed
//! would put the waves of a flashing flow in the wrong place.

use petir::roots::{BracketingMethod, BracketingSolver};
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    AvailableEnergy, MassDensity, MassFlux, Pressure, Ratio, SpecificHeatCapacity, SpecificVolume,
    ThermodynamicTemperature, Velocity,
};
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::velocity::meter_per_second;

use super::HemError;

/// One thermodynamic state as an equation of state closes it: everything the
/// solver and a caller need, in `uom` units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HemThermoState {
    /// Absolute static pressure.
    pub pressure: Pressure,
    /// Temperature. Inside the dome, the saturation temperature at `pressure`.
    pub temperature: ThermodynamicTemperature,
    /// Mixture mass density `ρ`.
    pub density: MassDensity,
    /// Specific internal energy `e`, J/kg.
    pub specific_internal_energy: AvailableEnergy,
    /// Specific enthalpy `h = e + p/ρ`, J/kg.
    pub specific_enthalpy: AvailableEnergy,
    /// Homogeneous-**equilibrium** sound speed (never the frozen Wood–Wallis
    /// speed); see the module doc.
    pub equilibrium_sound_speed: Velocity,
    /// Equilibrium vapour mass fraction inside the two-phase dome; `None` for a
    /// single-phase state.
    pub quality: Option<Ratio>,
    /// Homogeneous (no-slip) vapour volume fraction, `x·ρ/ρ_g` in the dome,
    /// `0` for liquid, `1` for vapour or gas.
    pub void_fraction: Ratio,
}

/// What a critical-flow model returns for a break discharging from a
/// stagnation state to a back pressure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BreakFlow {
    /// Mass flux through the break area, kg/(m²·s). Zero when the stagnation
    /// pressure does not exceed the back pressure (no backflow is modelled).
    pub mass_flux: MassFlux,
    /// `true` when the flow is choked (throat pressure above the back
    /// pressure), so the mass flux is the critical one.
    pub choked: bool,
}

/// The equation of state that closes the HEM system `W = [ρ, ρu, ρE]`.
///
/// Implemented in this crate by [`super::Iapws97Hem`] (water/steam) and
/// [`IdealGasHem`]; `tampines` adds CoolProp and an enum that dispatches over
/// all three. A compile-time contract only: never use it as `dyn`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a HEM equation of state",
    note = "implemented by `Iapws97Hem` and `IdealGasHem` here, and by `tampines::hem::HemEos` (which adds CoolProp)"
)]
pub trait HemEquationOfState: Clone + core::fmt::Debug {
    /// A short name for messages, e.g. `"IAPWS-IF97"`.
    fn name(&self) -> &'static str;

    /// Close a state from the solver's conserved variables: mass density `ρ`
    /// and specific internal energy `e = E − ½u²`.
    ///
    /// `pressure_hint` is the cell's last pressure, if known; an iterative
    /// backend uses it to start its bracket. It never changes the answer, only
    /// the cost.
    ///
    /// # Errors
    ///
    /// [`HemError::EquationOfState`] when the state cannot be closed;
    /// [`HemError::Unphysical`] (with `cell = 0`, which the solver overwrites)
    /// is not raised here, non-positive inputs are an `EquationOfState` error.
    fn state_rho_e(
        &self,
        density: MassDensity,
        specific_internal_energy: AvailableEnergy,
        pressure_hint: Option<Pressure>,
    ) -> Result<HemThermoState, HemError>;

    /// Close a single-phase state from pressure and temperature. Inside the
    /// dome `(p, T)` does not fix the state; use [`Self::state_p_h`].
    ///
    /// # Errors
    ///
    /// [`HemError::EquationOfState`] outside the backend's range.
    fn state_p_t(
        &self,
        pressure: Pressure,
        temperature: ThermodynamicTemperature,
    ) -> Result<HemThermoState, HemError>;

    /// Close a state from pressure and specific enthalpy (valid in the dome).
    ///
    /// # Errors
    ///
    /// [`HemError::EquationOfState`] outside the backend's range.
    fn state_p_h(
        &self,
        pressure: Pressure,
        specific_enthalpy: AvailableEnergy,
    ) -> Result<HemThermoState, HemError>;

    /// Close a state from pressure and density (the natural input of the
    /// ideal-gas Sod problem).
    ///
    /// # Errors
    ///
    /// [`HemError::Unsupported`] for a backend without this inversion;
    /// [`HemError::EquationOfState`] outside its range.
    fn state_p_rho(
        &self,
        pressure: Pressure,
        density: MassDensity,
    ) -> Result<HemThermoState, HemError>;

    /// Mass flux through a break from stagnation `(p0, h0)` to `back_pressure`
    /// (critical if choked, isentropic-subcritical otherwise). Used by
    /// [`super::HemBoundary::ChokedBreak`].
    ///
    /// The default returns [`HemError::Unsupported`]; only
    /// [`super::Iapws97Hem`] provides it, through this crate's Moody- and
    /// Zaloudek-verified HEM critical-flow dispatcher.
    ///
    /// # Errors
    ///
    /// [`HemError::Unsupported`] unless overridden.
    fn break_flow(
        &self,
        stagnation_pressure: Pressure,
        stagnation_enthalpy: AvailableEnergy,
        back_pressure: Pressure,
    ) -> Result<BreakFlow, HemError> {
        let _ = (stagnation_pressure, stagnation_enthalpy, back_pressure);
        Err(HemError::Unsupported {
            eos: self.name(),
            what: "a choked-break critical-flow model",
        })
    }
}

/// Calorically perfect gas, `p = (γ − 1)ρe`, `T = p/(ρR)`, `c = √(γp/ρ)`.
///
/// For verification only: it runs the classical Sod and Toro problems through
/// the same KNP solver as water. Single phase by construction, so the
/// equilibrium and frozen sound speeds coincide. `γ > 1`, `R > 0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IdealGasHem {
    /// Ratio of specific heats `γ = c_p/c_v`, dimensionless, `> 1`.
    pub gamma: f64,
    /// Specific gas constant `R`, J/(kg·K), `> 0`.
    pub specific_gas_constant: SpecificHeatCapacity,
}

impl IdealGasHem {
    /// Build from `γ` (dimensionless, `> 1`) and `R` (J/(kg·K), `> 0`).
    ///
    /// # Errors
    ///
    /// [`HemError::InvalidInput`] if `γ ≤ 1` or `R ≤ 0` (or either is not
    /// finite).
    pub fn new(
        gamma: Ratio,
        specific_gas_constant: SpecificHeatCapacity,
    ) -> Result<Self, HemError> {
        let g = gamma.get::<ratio>();
        let r = specific_gas_constant.get::<joule_per_kilogram_kelvin>();
        if !(g.is_finite() && g > 1.0) {
            return Err(HemError::InvalidInput(format!(
                "ideal gas gamma = {g} must be > 1"
            )));
        }
        if !(r.is_finite() && r > 0.0) {
            return Err(HemError::InvalidInput(format!(
                "ideal gas R = {r} J/(kg K) must be > 0"
            )));
        }
        Ok(Self {
            gamma: g,
            specific_gas_constant,
        })
    }

    /// Dry air, `γ = 1.4`, `R = 8314.46261815324/28.96 ≈ 287.1 J/(kg·K)`: the
    /// constants of `outram-foam-appbuilder-lib`'s Sod validation case, so the
    /// two solvers see the same gas.
    pub fn air() -> Self {
        Self {
            gamma: 1.4,
            specific_gas_constant: SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(
                8314.462_618_153_24 / 28.96,
            ),
        }
    }

    fn r(&self) -> f64 {
        self.specific_gas_constant
            .get::<joule_per_kilogram_kelvin>()
    }

    /// Assemble a state from `(ρ, p)`, both already checked positive.
    fn from_rho_p(&self, rho: f64, p: f64) -> HemThermoState {
        let e = p / ((self.gamma - 1.0) * rho);
        HemThermoState {
            pressure: Pressure::new::<pascal>(p),
            temperature: ThermodynamicTemperature::new::<kelvin>(p / (rho * self.r())),
            density: MassDensity::new::<kilogram_per_cubic_meter>(rho),
            specific_internal_energy: AvailableEnergy::new::<joule_per_kilogram>(e),
            specific_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(e + p / rho),
            equilibrium_sound_speed: Velocity::new::<meter_per_second>(
                (self.gamma * p / rho).sqrt(),
            ),
            quality: None,
            void_fraction: Ratio::new::<ratio>(1.0),
        }
    }
}

/// Recover pressure from density `ρ` and specific internal energy `e` for any
/// closure that has a `(p, h)` flash: the bracketed root of
///
/// ```text
/// g(p) = ρ · v(p, h = e + p/ρ) − 1
/// ```
///
/// by `petir::roots` Brent. Any root is an equilibrium state with the
/// requested `(v, e)`, since `e = h − p·v`; for a fluid in stable equilibrium
/// that state is unique, so is the sign change.
///
/// The bracket grows geometrically from `pressure_hint` (or the geometric
/// middle of `pressure_range`), `[p/f, p·f]` with `f = 1 + 1e-4` squared at
/// each try, clipped to `pressure_range`. Every probe the flash accepts is
/// kept, and the bracket is the first pair of pressure-adjacent accepted
/// probes whose residuals change sign; probes the flash rejects
/// (`specific_volume_ph` returns `None`) are skipped. A bracket wider than a factor 2 is then narrowed on a 48-point log grid to
/// the first adjacent pair of accepted probes that changes sign, so Brent only
/// probes between two pressures the flash accepted. Brent then converges to
/// `relative_tolerance` on the bracket width.
///
/// Used by [`super::Iapws97Hem`] and by `tampines`' CoolProp closure, so both
/// invert the same way. `specific_volume_ph(p, h)` must return `v(p, h)` in
/// m³/kg, or `None` outside its domain; it must not panic there.
///
/// # Errors
///
/// [`HemError::EquationOfState`] if no sign change is found within the range,
/// or the Brent solve fails (a probe inside the bracket left the domain, or
/// the iteration limit was hit).
pub fn pressure_from_rho_e<F>(
    density: MassDensity,
    specific_internal_energy: AvailableEnergy,
    pressure_hint: Option<Pressure>,
    pressure_range: (Pressure, Pressure),
    relative_tolerance: f64,
    specific_volume_ph: F,
) -> Result<Pressure, HemError>
where
    F: Fn(Pressure, AvailableEnergy) -> Option<SpecificVolume>,
{
    const MAX_BRACKET_EXPANSIONS: usize = 40;
    const MAX_BRENT_ITER: usize = 200;
    let rho = require_positive(
        "density",
        density.get::<kilogram_per_cubic_meter>(),
        "kg/m3",
    )?;
    let e = specific_internal_energy.get::<joule_per_kilogram>();
    let (p_min, p_max) = (
        pressure_range.0.get::<pascal>(),
        pressure_range.1.get::<pascal>(),
    );
    if !e.is_finite() || !(p_min > 0.0 && p_max > p_min) {
        return Err(HemError::EquationOfState {
            cell: None,
            detail: format!("(rho, e) inversion given e = {e} J/kg, range [{p_min}, {p_max}] Pa"),
        });
    }
    let g = |p: f64| {
        specific_volume_ph(
            Pressure::new::<pascal>(p),
            AvailableEnergy::new::<joule_per_kilogram>(e + p / rho),
        )
        .map(|v| rho * v.get::<cubic_meter_per_kilogram>() - 1.0)
    };
    let p0 = pressure_hint
        .map(|p| p.get::<pascal>())
        .filter(|p| p.is_finite() && *p > 0.0)
        .unwrap_or_else(|| (p_min * p_max).sqrt())
        .clamp(p_min, p_max);
    let straddles = |a: f64, b: f64| (a <= 0.0 && b >= 0.0) || (a >= 0.0 && b <= 0.0);
    // Every accepted probe is kept; the bracket is any two pressure-adjacent
    // accepted probes that change sign. So a side whose probes the flash
    // rejects does not stop a sign change on the other side being found.
    let mut probes: Vec<(f64, f64)> = Vec::new();
    let mut factor: f64 = 1.0 + 1.0e-4;
    let mut bracket = None;
    for _ in 0..MAX_BRACKET_EXPANSIONS {
        let lo_new = (p0 / factor).max(p_min);
        let hi_new = (p0 * factor).min(p_max);
        for p in [lo_new, hi_new] {
            if let Some(v) = g(p) {
                probes.push((p, v));
            }
        }
        probes.sort_by(|a, b| a.0.total_cmp(&b.0));
        bracket = probes
            .windows(2)
            .find(|w| straddles(w[0].1, w[1].1))
            .map(|w| (w[0].0, w[1].0));
        if bracket.is_some() || (lo_new <= p_min && hi_new >= p_max) {
            break;
        }
        factor *= factor;
    }
    let Some((lo, hi)) = bracket else {
        return Err(HemError::EquationOfState {
            cell: None,
            detail: format!(
                "(rho, e) inversion found no pressure bracket for rho = {rho} kg/m3, e = {e} J/kg \
                 (searched from {p0} Pa over [{p_min}, {p_max}] Pa; {} probes accepted by the flash)",
                probes.len()
            ),
        });
    };
    // A wide bracket (a far-off or missing hint) may contain pressures where
    // the flash fails. Narrow it first on a log grid, keeping the first
    // adjacent pair of valid probes that changes sign, so Brent only ever
    // probes between two points the flash accepted.
    let (lo, hi) = if hi / lo > 2.0 {
        const N_GRID: usize = 48;
        let r = (hi / lo).powf(1.0 / N_GRID as f64);
        let mut prev = (lo, g(lo));
        let mut found = None;
        for k in 1..=N_GRID {
            let pk = if k == N_GRID {
                hi
            } else {
                lo * r.powi(k as i32)
            };
            let gk = g(pk);
            if let (Some(a), Some(b)) = (prev.1, gk) {
                if straddles(a, b) {
                    found = Some((prev.0, pk));
                    break;
                }
            }
            if gk.is_some() {
                prev = (pk, gk);
            }
        }
        found.unwrap_or((lo, hi))
    } else {
        (lo, hi)
    };
    let f = |p: f64| g(p).unwrap_or(f64::NAN);
    let p = BracketingSolver::new(BracketingMethod::Brent, f, lo, hi)
        .and_then(|mut s| s.solve(0.0, relative_tolerance, MAX_BRENT_ITER))
        .map_err(|err| HemError::EquationOfState {
            cell: None,
            detail: format!(
                "(rho, e) Brent solve failed on [{lo}, {hi}] Pa for rho = {rho} kg/m3, e = {e} J/kg: {err:?}"
            ),
        })?;
    Ok(Pressure::new::<pascal>(p))
}

/// `Err(EquationOfState)` unless `value` is finite and `> 0`.
pub(crate) fn require_positive(name: &str, value: f64, unit: &str) -> Result<f64, HemError> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(HemError::EquationOfState {
            cell: None,
            detail: format!("{name} = {value} {unit} must be finite and positive"),
        })
    }
}

impl HemEquationOfState for IdealGasHem {
    fn name(&self) -> &'static str {
        "ideal gas"
    }

    fn state_rho_e(
        &self,
        density: MassDensity,
        specific_internal_energy: AvailableEnergy,
        _pressure_hint: Option<Pressure>,
    ) -> Result<HemThermoState, HemError> {
        let rho = require_positive(
            "density",
            density.get::<kilogram_per_cubic_meter>(),
            "kg/m3",
        )?;
        let e = require_positive(
            "specific internal energy",
            specific_internal_energy.get::<joule_per_kilogram>(),
            "J/kg",
        )?;
        Ok(self.from_rho_p(rho, (self.gamma - 1.0) * rho * e))
    }

    fn state_p_t(
        &self,
        pressure: Pressure,
        temperature: ThermodynamicTemperature,
    ) -> Result<HemThermoState, HemError> {
        let p = require_positive("pressure", pressure.get::<pascal>(), "Pa")?;
        let t = require_positive("temperature", temperature.get::<kelvin>(), "K")?;
        Ok(self.from_rho_p(p / (self.r() * t), p))
    }

    fn state_p_h(
        &self,
        pressure: Pressure,
        specific_enthalpy: AvailableEnergy,
    ) -> Result<HemThermoState, HemError> {
        let p = require_positive("pressure", pressure.get::<pascal>(), "Pa")?;
        let h = require_positive(
            "specific enthalpy",
            specific_enthalpy.get::<joule_per_kilogram>(),
            "J/kg",
        )?;
        // h = γ e = γ/(γ−1) · p/ρ
        let rho = self.gamma / (self.gamma - 1.0) * p / h;
        Ok(self.from_rho_p(rho, p))
    }

    fn state_p_rho(
        &self,
        pressure: Pressure,
        density: MassDensity,
    ) -> Result<HemThermoState, HemError> {
        let p = require_positive("pressure", pressure.get::<pascal>(), "Pa")?;
        let rho = require_positive(
            "density",
            density.get::<kilogram_per_cubic_meter>(),
            "kg/m3",
        )?;
        Ok(self.from_rho_p(rho, p))
    }
}
