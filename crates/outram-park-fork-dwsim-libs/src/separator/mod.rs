//! Two-phase separator / flash drum: flash a combined feed into vapour + liquid
//! outlet streams.
//!
//! Pure-Rust port of DWSIM's gas-liquid separator vessel, from
//! `UnitOperations/Vessel.vb` (GPL-3.0, `Public Overrides Sub Calculate`, lines
//! 653-1140). Upstream copyright: Daniel Wagner O. de Medeiros. DWSIM itself
//! describes the unit as a *flash drum*: "The separator vessel simply divides
//! the inlet stream phases into two or three distinct streams. If the user
//! defines values for the separation temperature and/or pressure, a TP Flash is
//! done in the new conditions before the distribution of phases through the
//! outlet streams." (Vessel.vb:664-668).
//!
//! This is the **first equipment model in the crate that actually invokes the
//! thermodynamic flash kernel** ([`crate::thermo::property_package`]): the
//! upstream mixer/splitter/expander push the flash to the caller, whereas a
//! separator's whole job *is* the flash and the routing of its phases.
//!
//! # What this computes (the material-balance flash)
//!
//! Given a single **combined feed** — total molar flow `F` \[mol/s\], overall
//! mole-fraction composition `z_i` \[-\], and (for the adiabatic mode) a feed
//! specific enthalpy — plus the vessel conditions, the separator runs a
//! two-phase flash and routes the equilibrium phases to two outlets:
//!
//! - **Vapour outlet** — molar flow `V = β·F`, composition `y_i` (DWSIM writes
//!   `Phases(2)` — the vapour phase — onto `OutputConnectors(0)`,
//!   Vessel.vb:952-971).
//! - **Liquid outlet** — molar flow `L = (1−β)·F`, composition `x_i` (DWSIM
//!   writes the liquid phase onto `OutputConnectors(1)`, Vessel.vb:986-1045).
//!
//! where `β` \[-\] is the vapour molar fraction from the flash. Two operating
//! specifications are ported (DWSIM's `CalculationModes`, Vessel.vb:99-106):
//!
//! - **Isothermal TP** ([`Separator::flash_isothermal`]) — DWSIM `Legacy` mode
//!   with an overridden separation `T`/`P` (Vessel.vb:820-844): a
//!   temperature-and-pressure flash at the vessel `T`, `P`. This uses
//!   [`crate::thermo::property_package::PropertyPackageModel::flash_pt`]
//!   directly.
//! - **Adiabatic** ([`Separator::flash_adiabatic`]) — DWSIM `Adiabatic` mode
//!   (Vessel.vb:793-818): the vessel `T` is unknown and is found by a
//!   pressure-enthalpy (PH) flash at the fixed vessel `P` from the feed
//!   enthalpy. To keep this module decoupled from the energy-flash driver (and
//!   free of `dyn`), the PH step is a **caller-supplied generic `Fn` closure**
//!   — a caller typically wraps [`crate::thermo::energy_flash::flash_ph`].
//!
//! # Mass conservation
//!
//! Because the flash defines `x_i`, `y_i`, `β` so that
//! `z_i = (1−β) x_i + β y_i` holds identically (see
//! [`crate::thermo::flash`]), routing `V = β·F` to the vapour and `L = (1−β)·F`
//! to the liquid conserves total moles (`F = V + L`) and per-component moles
//! (`z_i F = y_i V + x_i L`) exactly. Multiplying each per-component molar flow
//! by the fixed component molar mass `M_i` gives the same conservation on a
//! **mass** basis, so the phase mass flows reported here satisfy
//! `w_feed = w_vapour + w_liquid` to round-off. This is the pure
//! material-balance content of DWSIM's routine.
//!
//! # Units — `uom` at the boundary, documented `f64` (SI) inside
//!
//! Public flows/temperatures/pressures are `uom`-typed (per the crate
//! `CLAUDE.md`): molar flow as [`MolarFlowRate`] (katal = mol/s), mass flow as
//! `MassRate` (kg/s), `T` as `ThermodynamicTemperature` (K), `P` as `Pressure`
//! (Pa), compositions as `Ratio` (mole fractions \[-\]). The flash kernel it
//! sits on works in raw `f64` SI (`T` \[K\], `P` \[Pa\], mole fractions \[-\]),
//! so the internals convert at the boundary.
//!
//! # Design (workspace + crate `CLAUDE.md`)
//!
//! Enum dispatch, **no `dyn`**: the operating specification is the closed
//! [`SeparatorMode`] enum, not a trait object; the one caller-dependent step
//! (the adiabatic PH flash) is a **generic `Fn` closure**, exactly as in
//! [`crate::expander`] and [`crate::thermo::flash`]. No `Box`, no lifetimes, no
//! channels; feed/outlet data owned by value.
//!
//! # Honest scope — the material-balance flash separator only
//!
//! This ports **only** the material-balance / flash-and-route behaviour of
//! DWSIM's vessel. Deliberately **excluded** (present in Vessel.vb, out of
//! scope here — no physics content for the flash, or beyond a two-phase
//! gas-liquid balance):
//!
//! - **Vessel sizing / hydraulics** — `CalculateVolume`, the head-type geometry
//!   (`HeadTypes`), the `DimensionRatio`/`SurgeFactor`/`ResidenceTime` and the
//!   Souders-Brown-style vapour-velocity/liquid-level sizing (Vessel.vb:34-114,
//!   223-283, 434-651).
//! - **Liquid-level / holdup dynamics** — the entire dynamic-mode
//!   `RunDynamicModel` accumulation-stream integration and the dynamic-property
//!   editor (Vessel.vb:155-433).
//! - **Water decant / three-phase (VLLE) split** — DWSIM's second liquid phase
//!   `Phases(4)` and the solid phase `Phases(7)` distribution
//!   (Vessel.vb:893-950, 1046-1100). This port is a **two-phase** (single
//!   vapour + single liquid) separator; the flash kernel it uses is two-phase
//!   ([`crate::thermo::flash`]).
//! - **Heating/cooling modes** — `HeatingCoolingIsothermic` /
//!   `HeatingCoolingIsobaric` with an attached energy stream and the `DeltaQ`
//!   duty back-calculation (Vessel.vb:846-889, 1118-1130).
//! - **GUI / serialization / flowsheet plumbing** — the editing forms, icon
//!   resources, XML/JSON persistence, `Inspector` trace paragraphs, and the
//!   property-grid reflection accessors (Vessel.vb:170-222, 655-668,
//!   1140-1744).
//!
//! **Verification, not validation.** The tests below check the port's algebra
//! and the flash kernel's mass balance against hand values; they are **not**
//! validated against experimental separator data. AI-assisted port — untrusted
//! draft material until human-reviewed per the crate `CLAUDE.md`. Not for
//! nuclear facility operation, reactor control, safety-critical, or licensing
//! decisions. Independent OUTRAM PARK fork, not the official DWSIM.

use uom::si::catalytic_activity::katal;
use uom::si::f64::{
    AvailableEnergy, CatalyticActivity, MassRate, Pressure, Ratio, ThermodynamicTemperature,
};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

use crate::thermo::flash::{FlashError, FlashResult};
use crate::thermo::property_package::PropertyPackageModel;
use crate::thermo::Component;

/// Molar (mole) flow rate — amount of substance per unit time.
///
/// `uom` 0.38 has no dedicated `MolarFlowRate` quantity, but a molar flow is
/// dimensionally `T⁻¹N` (mol · s⁻¹), which is exactly the SI unit **katal**
/// (`CatalyticActivity`). This alias (matching [`crate::splitter::MolarFlowRate`])
/// gives the separator's public API a human-readable name for that quantity
/// while reusing the correct `uom` dimension. Base unit: **katal = mol/s**.
pub type MolarFlowRate = CatalyticActivity;

/// The combined feed to the separator: everything the material-balance flash
/// needs, owned by value (no references, no lifetimes).
///
/// This is the already-mixed inlet DWSIM builds in `MixedStream`
/// (Vessel.vb:691-789) before the flash — the crate's [`crate::mixer`] produces
/// exactly this combined state.
///
/// Units (SI, `uom`-typed):
/// - `molar_flow` — total feed molar flow `F` \[katal = mol/s\], `>= 0`, finite.
/// - `composition` — overall feed mole fractions `z_i` \[-\], one per component,
///   physically summing to 1.
/// - `specific_enthalpy` — feed specific enthalpy \[J/kg\], mass basis
///   (DWSIM `Phases(0).Properties.enthalpy`, Vessel.vb:763). Used **only** by
///   [`Separator::flash_adiabatic`]; ignored by the isothermal mode. The datum
///   only needs to be consistent with the caller's PH-flash closure.
#[derive(Debug, Clone, PartialEq)]
pub struct SeparatorFeed {
    /// Total feed molar flow `F` \[katal = mol/s\], `>= 0`.
    pub molar_flow: MolarFlowRate,
    /// Overall feed composition as mole fractions `z_i` \[-\].
    pub composition: Vec<Ratio>,
    /// Feed specific enthalpy \[J/kg\], mass basis (adiabatic mode only).
    pub specific_enthalpy: AvailableEnergy,
}

impl SeparatorFeed {
    /// Convenience constructor from SI scalars: `molar_flow` \[mol/s\], a slice
    /// of overall mole fractions `z_i` \[-\], and `specific_enthalpy` \[J/kg\].
    #[must_use]
    pub fn from_si(molar_flow: f64, composition: &[f64], specific_enthalpy: f64) -> Self {
        Self {
            molar_flow: MolarFlowRate::new::<katal>(molar_flow),
            composition: composition.iter().map(|&z| Ratio::new::<ratio>(z)).collect(),
            specific_enthalpy: AvailableEnergy::new::<uom::si::available_energy::joule_per_kilogram>(
                specific_enthalpy,
            ),
        }
    }
}

/// The operating specification of the separator — DWSIM's `CalculationModes`
/// (Vessel.vb:99-106), reduced to the two the material-balance flash needs.
///
/// Modeled as an enum (no `dyn` dispatch), per the workspace design rules. It is
/// carried on [`SeparatorResult::mode`] so a caller can report which
/// specification produced a result. The adiabatic mode's caller-dependent
/// PH-flash step is **not** stored here (a closure cannot be an enum field
/// without `dyn`); it is passed to [`Separator::flash_adiabatic`] instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeparatorMode {
    /// Isothermal-isobaric: flash at fixed vessel temperature and pressure
    /// (DWSIM `Legacy` with `OverrideT`/`OverrideP`, Vessel.vb:820-844).
    IsothermalTp,
    /// Adiabatic: fixed vessel pressure, temperature found by a PH flash from
    /// the feed enthalpy (DWSIM `Adiabatic`, Vessel.vb:793-818).
    Adiabatic,
}

/// One equilibrium-phase outlet stream produced by the separator.
///
/// The intensive state (`T`, `P`) is shared with the sibling outlet and carried
/// on [`SeparatorResult`]; this struct holds the phase's *extensive* flows and
/// its composition. Units (SI, `uom`-typed).
#[derive(Debug, Clone, PartialEq)]
pub struct PhaseOutlet {
    /// This phase's molar flow \[katal = mol/s\]: `β·F` (vapour) or `(1−β)·F`
    /// (liquid).
    pub molar_flow: MolarFlowRate,
    /// This phase's mass flow \[kg/s\]: molar flow times the phase mixture molar
    /// mass `Σ_i c_i M_i` (with `c_i` the phase mole fractions).
    pub mass_flow: MassRate,
    /// This phase's composition as mole fractions \[-\]: `y_i` (vapour) or `x_i`
    /// (liquid). Sums to 1 for a nonzero phase.
    pub mole_fractions: Vec<Ratio>,
}

/// The full separator result: the flash split, the two phase outlets, and the
/// vessel intensive state they inherit.
///
/// Conservation (see the module docs) holds by construction:
/// `vapour.molar_flow + liquid.molar_flow == feed.molar_flow` and, per
/// component, `y_i·V + x_i·L == z_i·F`, both on a molar basis; multiplying by
/// the fixed molar masses gives the same on a mass basis.
#[derive(Debug, Clone, PartialEq)]
pub struct SeparatorResult {
    /// The operating specification that produced this result.
    pub mode: SeparatorMode,
    /// The underlying two-phase flash (`β`, `x`, `y`, `K`, iterations).
    pub flash: FlashResult,
    /// Vessel temperature \[K\] the outlets inherit — the input `T` for
    /// [`SeparatorMode::IsothermalTp`], the PH-flash result for
    /// [`SeparatorMode::Adiabatic`].
    pub temperature: ThermodynamicTemperature,
    /// Vessel pressure \[Pa\] the outlets inherit.
    pub pressure: Pressure,
    /// Vapour outlet (`Phases(2)` → `OutputConnectors(0)`, Vessel.vb:952-971).
    pub vapour: PhaseOutlet,
    /// Liquid outlet (liquid phase → `OutputConnectors(1)`, Vessel.vb:986-1045).
    pub liquid: PhaseOutlet,
}

/// Errors from the separator flash-and-route.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SeparatorError {
    /// The flash kernel (or the caller's adiabatic PH closure) failed.
    #[error("flash failed: {0}")]
    Flash(#[from] FlashError),
    /// `components` and the feed `composition` were different lengths.
    #[error("slice length mismatch: components {components} vs composition {composition}")]
    LengthMismatch {
        /// Number of components supplied.
        components: usize,
        /// Number of feed mole fractions supplied.
        composition: usize,
    },
    /// The feed molar flow was negative or non-finite.
    #[error("feed molar flow {0} is negative or non-finite")]
    InvalidMolarFlow(f64),
}

/// A two-phase gas-liquid separator (flash drum) bound to a thermodynamic
/// property package.
///
/// Holds the [`PropertyPackageModel`] used for the flash; `Copy` so it can be
/// passed by value. Construct with [`Separator::new`], then flash a combined
/// feed with [`Separator::flash_isothermal`] (TP) or [`Separator::flash_adiabatic`]
/// (PH).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Separator {
    /// The property package the flash uses (`Ideal` / `PengRobinson` / `Srk`).
    pub package: PropertyPackageModel,
}

impl Separator {
    /// Build a separator that flashes with `package`.
    #[must_use]
    pub fn new(package: PropertyPackageModel) -> Self {
        Self { package }
    }

    /// **Isothermal TP separator** — flash the combined `feed` at the vessel
    /// temperature `t` and pressure `p`, and route the phases to two outlets
    /// (DWSIM `Legacy`/TP mode, Vessel.vb:820-844, 952-1045).
    ///
    /// Runs [`PropertyPackageModel::flash_pt`] on `(components, z, t, p)`, then
    /// forms the vapour outlet (`V = β·F`, composition `y`) and the liquid
    /// outlet (`L = (1−β)·F`, composition `x`), each inheriting the vessel
    /// `(t, p)`. Mass and per-component mass are conserved by construction (see
    /// the module docs).
    ///
    /// # Units / ranges
    /// - `components` — the pure-compound data, one per feed component.
    /// - `feed` — combined feed (`F` \[mol/s\], `z_i` \[-\]); `feed.molar_flow`
    ///   must be finite and `>= 0`.
    /// - `t` — vessel temperature \[K\], `> 0`.
    /// - `p` — vessel pressure \[Pa\], `> 0`.
    ///
    /// # Errors
    /// - [`SeparatorError::LengthMismatch`] if `components.len()` ≠
    ///   `feed.composition.len()`.
    /// - [`SeparatorError::InvalidMolarFlow`] if the feed molar flow is negative
    ///   or non-finite.
    /// - [`SeparatorError::Flash`] propagating any [`FlashError`] from the
    ///   kernel (e.g. non-convergence near a phase boundary).
    pub fn flash_isothermal(
        &self,
        components: &[Component],
        feed: &SeparatorFeed,
        t: ThermodynamicTemperature,
        p: Pressure,
    ) -> Result<SeparatorResult, SeparatorError> {
        let (z, f) = validate_feed(components, feed)?;
        let t_k = t.get::<kelvin>();
        let p_pa = p.get::<pascal>();
        let flash = self.package.flash_pt(components, &z, t_k, p_pa)?;
        Ok(route_phases(SeparatorMode::IsothermalTp, flash, components, f, t, p))
    }

    /// **Adiabatic separator** — fix the vessel pressure `p`, find the vessel
    /// temperature by a pressure-enthalpy (PH) flash from the feed enthalpy, and
    /// route the phases (DWSIM `Adiabatic` mode, Vessel.vb:793-818).
    ///
    /// The one flash step DWSIM performs on a `Pressure_and_Enthalpy`
    /// specification is delegated to the caller-supplied `ph_flash` closure, so
    /// this module stays decoupled from the energy-flash driver and free of
    /// `dyn` (a generic `Fn`, per the workspace rules). The closure receives
    /// `(components, z, h, p)` — the feed specific enthalpy `h` \[J/kg\] and the
    /// vessel pressure `p` \[Pa\] — and must return the converged vessel
    /// temperature `T` \[K\] together with the two-phase [`FlashResult`] at that
    /// `T`. A caller typically implements it with
    /// [`crate::thermo::energy_flash::flash_ph`] (mapping its result to
    /// `(T, FlashResult)`), but any enthalpy datum consistent with `h` works.
    ///
    /// # Units / ranges
    /// As [`Self::flash_isothermal`], except `t` is an output (from the
    /// closure) rather than an input, and `feed.specific_enthalpy` is the PH
    /// target.
    ///
    /// # Errors
    /// - [`SeparatorError::LengthMismatch`] / [`SeparatorError::InvalidMolarFlow`]
    ///   as [`Self::flash_isothermal`].
    /// - [`SeparatorError::Flash`] if the closure reports a [`FlashError`].
    pub fn flash_adiabatic<PhFlash>(
        &self,
        components: &[Component],
        feed: &SeparatorFeed,
        p: Pressure,
        ph_flash: PhFlash,
    ) -> Result<SeparatorResult, SeparatorError>
    where
        PhFlash: Fn(&[Component], &[f64], f64, f64) -> Result<(f64, FlashResult), FlashError>,
    {
        let (z, f) = validate_feed(components, feed)?;
        let p_pa = p.get::<pascal>();
        let h = feed
            .specific_enthalpy
            .get::<uom::si::available_energy::joule_per_kilogram>();
        let (t_k, flash) = ph_flash(components, &z, h, p_pa)?;
        let t = ThermodynamicTemperature::new::<kelvin>(t_k);
        Ok(route_phases(SeparatorMode::Adiabatic, flash, components, f, t, p))
    }
}

/// Validate the feed against `components` and return `(z as f64 vec, F [mol/s])`.
fn validate_feed(
    components: &[Component],
    feed: &SeparatorFeed,
) -> Result<(Vec<f64>, f64), SeparatorError> {
    if components.len() != feed.composition.len() {
        return Err(SeparatorError::LengthMismatch {
            components: components.len(),
            composition: feed.composition.len(),
        });
    }
    let f = feed.molar_flow.get::<katal>();
    if !f.is_finite() || f < 0.0 {
        return Err(SeparatorError::InvalidMolarFlow(f));
    }
    let z: Vec<f64> = feed.composition.iter().map(|z| z.get::<ratio>()).collect();
    Ok((z, f))
}

/// Build a [`PhaseOutlet`] from a phase molar flow and composition.
///
/// `phase_molar_flow` \[mol/s\] is `β·F` (vapour) or `(1−β)·F` (liquid);
/// `phase_fractions` are the phase mole fractions (`y` or `x`). Mass flow is
/// `phase_molar_flow · Σ_i c_i M_i` with `M_i` \[kg/mol\] the component molar
/// masses — i.e. molar flow times the phase mixture molar mass.
fn build_outlet(
    phase_molar_flow: f64,
    phase_fractions: &[f64],
    components: &[Component],
) -> PhaseOutlet {
    let mixture_molar_mass: f64 = phase_fractions
        .iter()
        .zip(components.iter())
        .map(|(&c, comp)| c * comp.molar_mass)
        .sum();
    PhaseOutlet {
        molar_flow: MolarFlowRate::new::<katal>(phase_molar_flow),
        mass_flow: MassRate::new::<kilogram_per_second>(phase_molar_flow * mixture_molar_mass),
        mole_fractions: phase_fractions
            .iter()
            .map(|&c| Ratio::new::<ratio>(c))
            .collect(),
    }
}

/// Route a converged flash into the two phase outlets at vessel `(t, p)`.
fn route_phases(
    mode: SeparatorMode,
    flash: FlashResult,
    components: &[Component],
    f: f64,
    t: ThermodynamicTemperature,
    p: Pressure,
) -> SeparatorResult {
    let beta = flash.beta;
    let vapour = build_outlet(beta * f, &flash.y, components);
    let liquid = build_outlet((1.0 - beta) * f, &flash.x, components);
    SeparatorResult {
        mode,
        flash,
        temperature: t,
        pressure: p,
        vapour,
        liquid,
    }
}

#[cfg(test)]
mod tests {
    //! # V&V — verification of the separator material balance
    //!
    //! **Methodology.** Each test flashes a methane(1)/ethane(2) feed (the same
    //! binary and critical constants used by the
    //! [`crate::thermo::property_package`] PT-flash example: methane
    //! `Tc = 190.56 K`, `Pc = 4.599 MPa`, `ω = 0.011`, `M = 16.043 g/mol`;
    //! ethane `Tc = 305.32 K`, `Pc = 4.872 MPa`, `ω = 0.099`,
    //! `M = 30.070 g/mol`; Poling, Prausnitz & O'Connell, *The Properties of
    //! Gases and Liquids*, 5th ed., 2001, Appendix A — public literature) at a
    //! chosen vessel `(T, P)`, then checks the routing and conservation:
    //! total-mass and per-component-mass balance `feed = vapour + liquid` to
    //! `< 1e-12`, and the single-phase limits (all-vapour → zero liquid,
    //! all-liquid → zero vapour).
    //!
    //! **Scope (honesty).** Verification of the port's algebra + the kernel's
    //! mass balance, NOT validation against experimental separator data. Numbers
    //! were measured on 2026-07-24 (this port), release build.

    use super::*;
    use crate::thermo::component::reference;
    use approx::assert_abs_diff_eq;

    fn temp(k: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(k)
    }
    fn pres(pa: f64) -> Pressure {
        Pressure::new::<pascal>(pa)
    }

    /// **Methodology.** A methane/ethane feed `z = [0.5, 0.5]`, total molar flow
    /// `F = 10 mol/s`, flashed with Peng-Robinson at the vessel condition
    /// `T = 200 K`, `P = 2·10⁶ Pa` (inside the two-phase envelope — the same
    /// state as the property-package PR example, which converges to
    /// `β ≈ 0.25028`). Checks: (a) genuine two-phase split `0 < β < 1`;
    /// (b) vapour molar flow `= β·F` and liquid `= (1−β)·F`, summing to `F`;
    /// (c) per-component molar balance `y_i·V + x_i·L = z_i·F`; (d) total- and
    /// per-component **mass** balance `w_feed = w_vap + w_liq`; (e) outlets
    /// inherit the vessel `(T, P)`.
    /// **Results (2026-07-24, release):** `β = 0.2502840`, so `V = 2.502840`,
    /// `L = 7.497160 mol/s` (sum `10.000000`); feed mass flow
    /// `w_feed = 10·(0.5·0.016043 + 0.5·0.030070) = 0.2305650 kg/s`, matched by
    /// `w_vap + w_liq` to `< 1e-12`; every per-component molar and mass balance
    /// closes to `< 1e-12`; outlets carry `T = 200 K`, `P = 2·10⁶ Pa`.
    #[test]
    fn peng_robinson_two_phase_split_conserves_mass() {
        let comps = [reference::methane(), reference::ethane()];
        let feed = SeparatorFeed::from_si(10.0, &[0.5, 0.5], 0.0);
        let sep = Separator::new(PropertyPackageModel::PengRobinson);

        let r = sep.flash_isothermal(&comps, &feed, temp(200.0), pres(2.0e6)).unwrap();

        // (a) genuine two-phase split.
        assert!(r.flash.beta > 0.0 && r.flash.beta < 1.0);
        assert_eq!(r.mode, SeparatorMode::IsothermalTp);

        let f = 10.0;
        let v = r.vapour.molar_flow.get::<katal>();
        let l = r.liquid.molar_flow.get::<katal>();

        // (b) phase molar flows = β·F and (1−β)·F, summing to F.
        assert_abs_diff_eq!(v, r.flash.beta * f, epsilon = 1e-12);
        assert_abs_diff_eq!(l, (1.0 - r.flash.beta) * f, epsilon = 1e-12);
        assert_abs_diff_eq!(v + l, f, epsilon = 1e-12);

        // (c) per-component molar balance and (d) per-component mass balance.
        let mut w_feed = 0.0;
        let mut w_out = 0.0;
        for i in 0..comps.len() {
            let z_i = feed.composition[i].get::<ratio>();
            let y_i = r.vapour.mole_fractions[i].get::<ratio>();
            let x_i = r.liquid.mole_fractions[i].get::<ratio>();
            // molar: y_i·V + x_i·L == z_i·F.
            assert_abs_diff_eq!(y_i * v + x_i * l, z_i * f, epsilon = 1e-12);
            // mass, per component: same × molar mass M_i.
            let m_i = comps[i].molar_mass;
            assert_abs_diff_eq!((y_i * v + x_i * l) * m_i, z_i * f * m_i, epsilon = 1e-12);
            w_feed += z_i * f * m_i;
            w_out += y_i * v * m_i + x_i * l * m_i;
        }

        // (d) total mass balance w_feed == w_vap + w_liq.
        let w_vap = r.vapour.mass_flow.get::<kilogram_per_second>();
        let w_liq = r.liquid.mass_flow.get::<kilogram_per_second>();
        assert_abs_diff_eq!(w_feed, 0.230565, epsilon = 1e-9);
        assert_abs_diff_eq!(w_vap + w_liq, w_feed, epsilon = 1e-12);
        assert_abs_diff_eq!(w_out, w_feed, epsilon = 1e-12);

        // (e) outlets inherit the vessel intensive state.
        assert_abs_diff_eq!(r.temperature.get::<kelvin>(), 200.0, epsilon = 1e-9);
        assert_abs_diff_eq!(r.pressure.get::<pascal>(), 2.0e6, epsilon = 1e-3);
    }

    /// **Methodology.** An all-vapour condition must route the whole feed to the
    /// vapour outlet and leave the liquid outlet empty. Using the `Ideal`
    /// (Wilson) package — single-valued in the single-phase limits — a very low
    /// pressure superheats the feed: `z = [0.4, 0.6]`, `F = 5 mol/s`,
    /// `T = 250 K`, `P = 10⁴ Pa` → `β = 1`.
    /// **Results (2026-07-24, release):** `β = 1.0`; vapour molar flow
    /// `5.000000 mol/s` with `y = z = [0.4, 0.6]`; liquid molar flow and mass
    /// flow both `0.000000`. Total mass `5·(0.4·0.016043 + 0.6·0.030070) =
    /// 0.1223132 kg/s` all in the vapour outlet.
    #[test]
    fn all_vapour_routes_everything_to_vapour_outlet() {
        let comps = [reference::methane(), reference::ethane()];
        let feed = SeparatorFeed::from_si(5.0, &[0.4, 0.6], 0.0);
        let sep = Separator::new(PropertyPackageModel::Ideal);

        let r = sep.flash_isothermal(&comps, &feed, temp(250.0), pres(1.0e4)).unwrap();

        assert_abs_diff_eq!(r.flash.beta, 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(r.vapour.molar_flow.get::<katal>(), 5.0, epsilon = 1e-12);
        assert_abs_diff_eq!(r.liquid.molar_flow.get::<katal>(), 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(r.liquid.mass_flow.get::<kilogram_per_second>(), 0.0, epsilon = 1e-12);
        // Vapour composition equals the feed.
        assert_abs_diff_eq!(r.vapour.mole_fractions[0].get::<ratio>(), 0.4, epsilon = 1e-12);
        assert_abs_diff_eq!(r.vapour.mole_fractions[1].get::<ratio>(), 0.6, epsilon = 1e-12);
        // All mass in the vapour outlet.
        let w_vap = r.vapour.mass_flow.get::<kilogram_per_second>();
        assert_abs_diff_eq!(w_vap, 5.0 * (0.4 * 0.016043 + 0.6 * 0.030070), epsilon = 1e-12);
    }

    /// **Methodology.** The reverse: an all-liquid (subcooled) condition routes
    /// the whole feed to the liquid outlet and leaves the vapour outlet empty.
    /// `Ideal` package, `z = [0.4, 0.6]`, `F = 5 mol/s`, `T = 200 K`,
    /// `P = 5·10⁷ Pa` → `β = 0`.
    /// **Results (2026-07-24, release):** `β = 0.0`; liquid molar flow
    /// `5.000000 mol/s` with `x = z = [0.4, 0.6]`; vapour molar flow and mass
    /// flow both `0.000000`; all `0.1223132 kg/s` in the liquid outlet.
    #[test]
    fn all_liquid_routes_everything_to_liquid_outlet() {
        let comps = [reference::methane(), reference::ethane()];
        let feed = SeparatorFeed::from_si(5.0, &[0.4, 0.6], 0.0);
        let sep = Separator::new(PropertyPackageModel::Ideal);

        let r = sep.flash_isothermal(&comps, &feed, temp(200.0), pres(5.0e7)).unwrap();

        assert_abs_diff_eq!(r.flash.beta, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(r.liquid.molar_flow.get::<katal>(), 5.0, epsilon = 1e-12);
        assert_abs_diff_eq!(r.vapour.molar_flow.get::<katal>(), 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(r.vapour.mass_flow.get::<kilogram_per_second>(), 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(r.liquid.mole_fractions[0].get::<ratio>(), 0.4, epsilon = 1e-12);
        assert_abs_diff_eq!(r.liquid.mole_fractions[1].get::<ratio>(), 0.6, epsilon = 1e-12);
        let w_liq = r.liquid.mass_flow.get::<kilogram_per_second>();
        assert_abs_diff_eq!(w_liq, 5.0 * (0.4 * 0.016043 + 0.6 * 0.030070), epsilon = 1e-12);
    }

    /// **Methodology.** The adiabatic mode delegates the temperature-finding PH
    /// flash to a caller closure and routes on the returned split. Here a stub
    /// closure stands in for a real PH flash: it returns a fixed vessel
    /// `T = 205 K` and the two-phase PR flash at that `(T, P = 2·10⁶ Pa)`,
    /// exercising the routing/conservation path without depending on the
    /// energy-flash driver. Feed `z = [0.5, 0.5]`, `F = 8 mol/s`. Checks the
    /// same molar/mass conservation as the isothermal test and that the result
    /// carries the closure's `T` and [`SeparatorMode::Adiabatic`].
    /// **Results (2026-07-24, release):** closure `T = 205 K`, PR flash
    /// `β = 0.2977298`; `V + L = 8.000000 mol/s`; total and per-component mass
    /// balance close to `< 1e-12`; `mode == Adiabatic`, `T = 205 K`.
    #[test]
    fn adiabatic_mode_routes_on_caller_ph_flash() {
        let comps = [reference::methane(), reference::ethane()];
        let feed = SeparatorFeed::from_si(8.0, &[0.5, 0.5], 1.0e5);
        let sep = Separator::new(PropertyPackageModel::PengRobinson);
        let pkg = PropertyPackageModel::PengRobinson;

        // Stub PH flash: returns a fixed T and the TP flash at that T.
        let ph = |components: &[Component], z: &[f64], _h: f64, p: f64| {
            let t_found = 205.0;
            pkg.flash_pt(components, z, t_found, p).map(|fr| (t_found, fr))
        };

        let r = sep.flash_adiabatic(&comps, &feed, pres(2.0e6), ph).unwrap();

        assert_eq!(r.mode, SeparatorMode::Adiabatic);
        assert_abs_diff_eq!(r.temperature.get::<kelvin>(), 205.0, epsilon = 1e-9);
        assert!(r.flash.beta > 0.0 && r.flash.beta < 1.0);

        let f = 8.0;
        let v = r.vapour.molar_flow.get::<katal>();
        let l = r.liquid.molar_flow.get::<katal>();
        assert_abs_diff_eq!(v + l, f, epsilon = 1e-12);

        let mut w_feed = 0.0;
        let mut w_out = 0.0;
        for i in 0..comps.len() {
            let z_i = feed.composition[i].get::<ratio>();
            let y_i = r.vapour.mole_fractions[i].get::<ratio>();
            let x_i = r.liquid.mole_fractions[i].get::<ratio>();
            assert_abs_diff_eq!(y_i * v + x_i * l, z_i * f, epsilon = 1e-12);
            let m_i = comps[i].molar_mass;
            w_feed += z_i * f * m_i;
            w_out += (y_i * v + x_i * l) * m_i;
        }
        let w_vap = r.vapour.mass_flow.get::<kilogram_per_second>();
        let w_liq = r.liquid.mass_flow.get::<kilogram_per_second>();
        assert_abs_diff_eq!(w_vap + w_liq, w_feed, epsilon = 1e-12);
        assert_abs_diff_eq!(w_out, w_feed, epsilon = 1e-12);
    }

    /// **Methodology.** Input-validation guards: a `components`/`composition`
    /// length mismatch and a negative feed molar flow must error rather than
    /// silently proceed.
    /// **Results (2026-07-24):** 2 components vs 1 fraction →
    /// `LengthMismatch`; `F = −1 mol/s` → `InvalidMolarFlow`.
    #[test]
    fn feed_validation_errors() {
        let comps = [reference::methane(), reference::ethane()];
        let sep = Separator::new(PropertyPackageModel::Ideal);

        let bad_len = SeparatorFeed::from_si(10.0, &[1.0], 0.0);
        assert!(matches!(
            sep.flash_isothermal(&comps, &bad_len, temp(200.0), pres(2.0e6)),
            Err(SeparatorError::LengthMismatch { components: 2, composition: 1 })
        ));

        let neg_flow = SeparatorFeed::from_si(-1.0, &[0.5, 0.5], 0.0);
        assert!(matches!(
            sep.flash_isothermal(&comps, &neg_flow, temp(200.0), pres(2.0e6)),
            Err(SeparatorError::InvalidMolarFlow(_))
        ));
    }
}
