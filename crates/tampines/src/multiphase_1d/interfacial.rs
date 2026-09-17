// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! The 1-D **interfacial exchange** layer — the three source terms that couple
//! the two phases of a six-equation two-fluid model.
//!
//! # What belongs in here
//!
//! Exactly the *interfacial closures* a six-equation model needs, evaluated
//! **one cell at a time** from that cell's local two-phase state:
//!
//! | Term | Symbol | Units | Closed by |
//! |---|---|---|---|
//! | Interfacial drag | `F^i` | N/m³ | [`outram_foam_multiphase::two_fluid::DragModel`] |
//! | Interfacial heat transfer | `Q_g^i`, `Q_l^i` | W/m³ | [`outram_foam_multiphase::heat_transfer::InterfacialHeatTransfer`] |
//! | Interfacial mass transfer | `Γ` | kg/(m³·s) | the energy jump across the interface (below) |
//!
//! # What does NOT belong in here
//!
//! - **The field equations.** This module computes source *terms*; the phase
//!   mass/momentum/energy equations, their discretisation, and the pressure
//!   solve live in [`super::two_fluid`].
//! - **The well-posedness regularisation.** The naive six-equation system has
//!   complex characteristics; whichever term fixes that (interfacial pressure,
//!   virtual mass, artificial viscosity) is a *field-equation* decision and is
//!   made in the solver, not here.
//! - **Wall heat transfer and wall friction.** Those couple a phase to the
//!   *wall*, not to the interface.
//! - **Interfacial-area transport and the flow-regime map.** See "Honest
//!   scope" below — both are absent, and their absence is the largest
//!   modelling limitation of this layer.
//! - **A second property path.** Every thermodynamic and transport number comes
//!   from [`super::properties`]; nothing is re-derived here.
//!
//! # Trace-back: what is consumed rather than reinvented
//!
//! Beads `op-dt3.12` / `op-dt3.13` require 1-D closures to trace back to the
//! OUTRAM-FOAM 3-D reference rather than be invented. Concretely:
//!
//! - **Drag** — [`outram_foam_multiphase::two_fluid::DragModel`] (Schiller-
//!   Naumann, Wen-Yu, and a constant, all ported from OpenFOAM's
//!   `dispersedDragModel` chain) is called through its own public
//!   [`k_d`](outram_foam_multiphase::two_fluid::DragModel::k_d), on a
//!   single-cell [`outram_foam_multiphase::two_fluid::TwoFluidSystem`] packed
//!   from the local state. The slip Reynolds number is likewise taken from
//!   [`TwoFluidSystem::reynolds_number`], not recomputed. No drag correlation
//!   is written down in this file.
//! - **Heat transfer** —
//!   [`outram_foam_multiphase::heat_transfer::InterfacialHeatTransfer`]
//!   (Ranz-Marshall, spherical conduction, Gunn — ported from OpenFOAM's
//!   `multiphaseEuler` heat-transfer models) supplies both volumetric
//!   coefficients. No Nusselt correlation is written down in this file either.
//!
//! Note for readers of [`super::two_fluid`]'s module documentation: its
//! "what still has to be decided" point 2 says the 3-D reference has no
//! interfacial heat-transfer closure at all. That was true when it was
//! written; `outram_foam_multiphase::heat_transfer` has since been ported, and
//! this module consumes it. The mass-transfer closure below is the remaining
//! piece, and it is a *thermodynamic identity* rather than a correlation — see
//! the next section.
//!
//! # The two-resistance framework, and the error it exists to prevent
//!
//! The interface is taken to sit at the **saturation temperature** `T_sat(p)`.
//! Each phase then exchanges heat with that interface through its own
//! resistance:
//!
//! `Q_k^i = K_k · (T_sat − T_k)`  \[W/m³\], `k ∈ {g, l}`
//!
//! and the two `K_k` come from **different** correlations, because the two
//! resistances are physically different:
//!
//! - the **continuous** phase sees a convective boundary layer *outside* the
//!   inclusion — closed by Ranz-Marshall or Gunn, which read the
//!   **continuous** phase's thermal conductivity `λ_c`;
//! - the **dispersed** phase sees conduction *inside* the inclusion — closed by
//!   the spherical-conduction model, which reads the **dispersed** phase's
//!   conductivity `λ_d`.
//!
//! Swapping those two conductivities is a **silent** error: the code runs and
//! the answer is wrong by the conductivity ratio, which for steam/water is
//! close to a factor of ten (measured `λ_f/λ_g = 9.210375` at 7 MPa on
//! 2026-08-12 — see `two_resistance_pair_reads_one_conductivity_from_each_side`,
//! which shows the swapped liquid-side flux coming out 9.21× low). Three
//! things guard against it here, deliberately and redundantly:
//!
//! 1. [`InterfacialExchange::new`] **rejects** a pair whose
//!    [`ResistanceSide`]s are not one `Continuous` and one `Dispersed`.
//! 2. The conductivities are passed to the upstream closure **by role**
//!    (`λ_continuous`, `λ_dispersed`) in fixed argument positions; the variant
//!    itself decides which to read. This file never picks one.
//! 3. [`DispersedPhase`] is an explicit constructor argument, so "which phase
//!    is dispersed" is a stated modelling choice rather than an accident of
//!    how the caller ordered its arguments.
//!
//! # Interfacial mass transfer — derived, not invented
//!
//! With the interface at `T_sat(p)` and no thermal capacity of its own, the
//! energy that arrives at the interface from both sides has nowhere to go
//! except into phase change. That closes `Γ` with no new correlation:
//!
//! `Γ · h_fg = −(Q_g^i + Q_l^i)`, so `Γ = −(Q_g^i + Q_l^i) / (h_g^sat − h_l^sat)`.
//!
//! ## Sign convention — stated plainly
//!
//! - `Q_k^i` is the heat flowing **into phase `k` from the interface**
//!   \[W/m³\]. This is the convention of
//!   [`InterfacialHeatTransfer`](outram_foam_multiphase::heat_transfer::InterfacialHeatTransfer)
//!   (`Q = K (T_interface − T_phase)`) and it is the sign with which `Q_k^i`
//!   enters the phase energy equation as written in [`super::two_fluid`]
//!   (`… = α_k ∂p/∂t + Q_k^i + Γ_k h_k^i + q_k^wall`). A **positive** `Q_g^i`
//!   heats the vapour.
//! - **`Γ > 0` means evaporation** — liquid becoming vapour, mass leaving the
//!   liquid equation and entering the vapour equation (`Γ_g = +Γ`,
//!   `Γ_l = −Γ`). `Γ < 0` is condensation.
//! - The minus sign in `Γ = −(Q_g^i + Q_l^i)/h_fg` follows from those two
//!   choices together: superheated vapour (`T_g > T_sat`) gives `Q_g^i < 0` —
//!   the vapour *loses* heat to the interface — and that delivered heat
//!   evaporates liquid, so `Γ > 0`. Written instead with interface-directed
//!   fluxes `q_{k→i} = −Q_k^i`, the same relation reads
//!   `Γ = (q_{g→i} + q_{l→i})/h_fg` with a plus sign. Both are the same
//!   physics; this module reports the *into-phase* fluxes, so the minus sign
//!   is the one that belongs in the code.
//!
//! ## The degenerate case: `h_fg → 0` at the critical point
//!
//! Physically `h_fg = h_g^sat − h_l^sat` vanishes at the critical point
//! (`p_c = 22.064 MPa`), so `Γ` becomes a `0/0` limit there and is not
//! computable from this energy balance. Above `p_c` there is no saturation
//! line at all, and [`SaturatedProperties::at`] does **not** itself refuse
//! supercritical pressures — its `P_MAX_IF97` is 100 MPa — so an unguarded
//! division would return a finite, entirely fictitious `Γ`.
//!
//! **This module refuses, and it refuses on pressure rather than on `h_fg`.**
//! That is not the obvious choice, so here is why. Measured on 2026-08-12 (see
//! `latent_heat_guard_fires_only_next_to_the_critical_point`), the latent heat
//! this property layer reports on the approach to `p_c` is:
//!
//! ```text
//! p = 20.000 MPa   h_fg = 6.0058e5 J/kg
//! p = 22.000 MPa   h_fg = 3.8700e5 J/kg
//! p = 22.060 MPa   h_fg = 3.7988e5 J/kg
//! p = 22.064 MPa   h_fg = 3.7940e5 J/kg   <- p_c; the true value here is 0
//! ```
//!
//! It does not collapse. [`SaturatedProperties::at`] evaluates `h_f` and `h_g`
//! from the IF97 **Region 1 and Region 2** equations at `T_sat(p)`, and both
//! regions stop at `T = 623.15 K` while `T_sat(p_c) ≈ 647 K`; reproducing the
//! critical collapse needs Region 3, which that property layer does not
//! implement. So a threshold on `h_fg` alone would **never fire for water** and
//! would be a guard in name only — precisely the kind of reassuring-looking
//! check this project's V&V rules exist to prevent.
//!
//! What actually guards the closure is therefore an explicit refusal at
//! `p ≥ ` [`P_CRITICAL_WATER`], with the
//! [`MIN_LATENT_HEAT_FOR_MASS_TRANSFER`] threshold kept behind it as a
//! backstop for a property set that degenerates for some other reason (a
//! non-finite or negative `h_fg`, or a future Region-3 implementation that
//! *does* reproduce the collapse just below `p_c`). Both return
//! [`TampinesError::Unphysical`]. Nothing is clamped, nothing is floored, and
//! no "large but finite" `Γ` is returned, because a plausible-looking number
//! here would propagate into the phase mass equations and be
//! indistinguishable from an answer. A caller at these pressures needs a
//! different model (single-phase supercritical), not a rescued division.
//!
//! Note what this does *not* fix: at 22.0 MPa, just below the refusal, the
//! closure returns a number built on a `h_fg` that is already leaving IF97's
//! Region 1/2 validity. Nothing here has checked that number against
//! anything.
//!
//! # Honest scope — what this layer does NOT do
//!
//! - **No interfacial-area transport.** The inclusion diameter `d` is
//!   prescribed once at construction and never responds to breakup,
//!   coalescence, or the local flow. Every closure here scales as `1/d²`, so
//!   `d` is the single most influential number a caller supplies and the least
//!   justified.
//! - **No flow-regime map.** [`DispersedPhase`] is fixed for the whole
//!   calculation. A real transient runs bubbly at low void and droplet/mist at
//!   high void; this layer will happily evaluate bubbly closures at `α_g =
//!   0.99`, which is wrong and will not complain.
//! - **No lift, virtual mass, wall lubrication, or turbulent dispersion.**
//!   Upstream carries those as unported scaffolds
//!   ([`outram_foam_multiphase::two_fluid::InterfacialForce`]); only drag is
//!   available, so only drag is offered.
//! - **No condensation/evaporation rate limiter.** `Γ` is whatever the energy
//!   balance says. At small `α_g` with a large subcooling the implied `Γ` can
//!   remove more vapour than the cell contains within a timestep; bounding
//!   that is the *solver's* job (it needs `Δt` and the cell inventory, neither
//!   of which this layer sees) and it is not done here.
//! - **Transport properties are taken on the saturation line**, even for a
//!   metastable phase — see [`SaturatedTransport`] for why that is both safer
//!   and more accurate than the alternative over the bounded metastable
//!   departures [`PhaseState`] admits.
//! - **Not validated against any experiment.** Every test in this file is
//!   *verification* against a closed form or an invariant. No interfacial
//!   closure here has been compared with measured phase-change data.
//!
//! # Units
//!
//! Constructors and the `uom` accessors on [`InterfacialSources`] are
//! dimension-checked. [`InterfacialCellState`] and the raw fields of
//! [`InterfacialSources`] carry **raw `f64` in strict SI** — pascal, `J/kg`,
//! `m/s`, kelvin, `W/m³`, `N/m³`, `kg/(m³·s)` — because a 1-D system code
//! evaluates them once per cell per timestep, and every one of them is
//! documented individually with its unit spelled out.

use std::sync::Arc;

use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_multiphase::heat_transfer::{InterfacialHeatTransfer, ResistanceSide};
use outram_foam_multiphase::two_fluid::{DragModel, Phase, TwoFluidSystem, RESIDUAL_ALPHA};

use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{
    Area, AvailableEnergy, DynamicViscosity, Length, MassDensity, Pressure,
    ThermodynamicTemperature, Velocity, VolumetricPowerDensity,
};
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::velocity::meter_per_second;
use uom::si::volumetric_power_density::watt_per_cubic_meter;

use super::properties::{PhaseState, SaturatedProperties, SaturatedTransport};
use crate::TampinesError;

/// The critical pressure of water `p_c` \[Pa\], IAPWS-95 / IF97 value
/// `22.064 MPa`.
///
/// # What it guards
///
/// At and above `p_c` there is no saturation line, so the interface cannot be
/// "at `T_sat`", `h_fg` has no meaning, and `Γ` is not defined by the energy
/// jump this module uses.
/// [`InterfacialExchange::sources_with_properties`] refuses at `p ≥ p_c` with
/// [`TampinesError::Unphysical`].
///
/// This is a **pressure** guard rather than a latent-heat guard for a measured
/// reason: [`SaturatedProperties::at`] reports `h_fg = 3.7940e5 J/kg` at
/// exactly `p_c` (2026-08-12), where the physical value is zero, because its
/// `h_f`/`h_g` come from IF97 Regions 1 and 2, whose validity stops at
/// `T = 623.15 K` while `T_sat(p_c) ≈ 647 K`. The module documentation gives
/// the full sweep and the reasoning. Guarding on `h_fg` alone would have been
/// a check that never fires.
pub const P_CRITICAL_WATER: f64 = 22.064e6;

/// Smallest latent heat `h_fg` \[J/kg\] at which the interfacial mass-transfer
/// balance is still considered meaningful — the **backstop** behind
/// [`P_CRITICAL_WATER`].
///
/// # What it guards
///
/// `Γ = −(Q_g^i + Q_l^i)/h_fg` divides by the latent heat. The pressure guard
/// already excludes the region where that latent heat physically vanishes, so
/// this threshold exists to catch a property set that has gone wrong some
/// *other* way — a non-finite or negative `h_fg`, a mismatched saturation set,
/// or a future Region-3 property implementation which, unlike the present one,
/// would correctly drive `h_fg` to zero in the last few kPa below `p_c`. It is
/// checked after the pressure guard, so for water as the property layer stands
/// today it does not fire; that is stated rather than hidden.
///
/// # Why `1e4` J/kg specifically
///
/// Far enough above zero that the division is not dominated by the difference
/// of two large, nearly equal enthalpies, and far below any subcritical value
/// the property layer produces — the measured minimum over the sweep in
/// `latent_heat_guard_fires_only_next_to_the_critical_point` (2026-08-12) is
/// `3.7940e5 J/kg` at `p_c`, thirty-eight times this threshold.
pub const MIN_LATENT_HEAT_FOR_MASS_TRANSFER: f64 = 1.0e4;

/// Which phase is treated as **dispersed** (bubbles or droplets) and which as
/// **continuous** (the carrier).
///
/// Every closure in this module is a *dispersed-phase* closure: it assumes
/// inclusions of one phase carried in the other, with the interfacial area set
/// by the inclusion diameter. Which phase plays which role therefore changes
/// which conductivity closes which resistance, and it is an explicit modelling
/// choice, not an inference.
///
/// **There is no flow-regime map.** Whichever variant is chosen at
/// construction is used at every void fraction for the whole calculation. The
/// physical choice is `Vapour` for bubbly flow (low `α_g`) and `Liquid` for
/// droplet/mist flow (high `α_g`); nothing here checks that `α_g` is in the
/// matching range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispersedPhase {
    /// Vapour bubbles carried in continuous liquid — bubbly flow. The
    /// continuous-side resistance then reads the **liquid** conductivity
    /// `λ_f`, the dispersed-side resistance the **vapour** conductivity `λ_g`.
    Vapour,
    /// Liquid droplets carried in continuous vapour — droplet / mist flow. The
    /// roles above are exchanged: continuous side reads `λ_g`, dispersed side
    /// reads `λ_f`.
    Liquid,
}

impl DispersedPhase {
    /// `true` when the vapour is the dispersed phase (bubbly flow).
    #[must_use]
    pub fn is_vapour_dispersed(self) -> bool {
        matches!(self, Self::Vapour)
    }
}

/// The local two-phase state of **one** cell, as a six-equation solver carries
/// it.
///
/// # Why six numbers
///
/// A six-equation model transports `α_g`, and for each phase a velocity and an
/// enthalpy. That — plus the pressure — is exactly the state the interfacial
/// closures need. Nothing here is a mixture quantity: the whole point of the
/// six-equation model is that the phases may differ in both velocity and
/// temperature, so `h_g` and `h_l` are independent and neither is required to
/// sit on the saturation line.
///
/// # Units and ranges (raw `f64`, strict SI — this is a marching-loop boundary)
///
/// Every field is a raw `f64`; use [`InterfacialCellState::new`] for a
/// dimension-checked construction from `uom` quantities.
///
/// - `pressure` — \[Pa\], inside the IAPWS-IF97 range
///   `[611.657, 1.0e8]`, and (for a meaningful `Γ`) below the critical
///   pressure — see [`MIN_LATENT_HEAT_FOR_MASS_TRANSFER`].
/// - `void_fraction` — `α_g` \[-\], in `[0, 1]`. Outside that it is refused,
///   not clipped.
/// - `vapour_enthalpy`, `liquid_enthalpy` — \[J/kg\], within the bounded
///   metastable brackets [`PhaseState`] admits (30 K of vapour subcooling,
///   50 K of liquid superheat as of writing); further than that is refused by
///   the property layer, not extrapolated.
/// - `vapour_velocity`, `liquid_velocity` — \[m/s\], axial, signed, finite.
///   Only their **difference** enters any closure here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InterfacialCellState {
    /// Pressure `p` \[Pa\], shared by both phases (single-pressure model).
    pub pressure: f64,
    /// Vapour volume fraction `α_g` \[-\], in `[0, 1]`.
    pub void_fraction: f64,
    /// Vapour specific enthalpy `h_g` \[J/kg\].
    pub vapour_enthalpy: f64,
    /// Liquid specific enthalpy `h_l` \[J/kg\].
    pub liquid_enthalpy: f64,
    /// Vapour axial velocity `u_g` \[m/s\], signed.
    pub vapour_velocity: f64,
    /// Liquid axial velocity `u_l` \[m/s\], signed.
    pub liquid_velocity: f64,
}

impl InterfacialCellState {
    /// Build a cell state from `uom` quantities, so the dimensions are checked
    /// at the API boundary.
    ///
    /// `void_fraction` stays a bare `f64` because it is dimensionless by
    /// definition; it is range-checked when the state is used, not here.
    #[must_use]
    pub fn new(
        pressure: Pressure,
        void_fraction: f64,
        vapour_enthalpy: AvailableEnergy,
        liquid_enthalpy: AvailableEnergy,
        vapour_velocity: Velocity,
        liquid_velocity: Velocity,
    ) -> Self {
        Self {
            pressure: pressure.get::<pascal>(),
            void_fraction,
            vapour_enthalpy: vapour_enthalpy.get::<joule_per_kilogram>(),
            liquid_enthalpy: liquid_enthalpy.get::<joule_per_kilogram>(),
            vapour_velocity: vapour_velocity.get::<meter_per_second>(),
            liquid_velocity: liquid_velocity.get::<meter_per_second>(),
        }
    }

    /// Slip velocity `u_g − u_l` \[m/s\], signed. Positive means the vapour
    /// runs ahead of the liquid, the usual sense in an upward or a
    /// depressurising flow.
    #[must_use]
    pub fn slip_velocity(self) -> f64 {
        self.vapour_velocity - self.liquid_velocity
    }

    /// Reject a state no closure can be evaluated on.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] for a non-finite entry;
    /// [`TampinesError::Unphysical`] for a void fraction outside `[0, 1]`.
    /// Both are refusals — nothing is clipped into range.
    fn check(self) -> Result<(), TampinesError> {
        for (name, value) in [
            ("pressure", self.pressure),
            ("void_fraction", self.void_fraction),
            ("vapour_enthalpy", self.vapour_enthalpy),
            ("liquid_enthalpy", self.liquid_enthalpy),
            ("vapour_velocity", self.vapour_velocity),
            ("liquid_velocity", self.liquid_velocity),
        ] {
            if !value.is_finite() {
                return Err(TampinesError::InvalidInput(format!(
                    "interfacial cell state field `{name}` is not finite (got {value})"
                )));
            }
        }
        if !(0.0..=1.0).contains(&self.void_fraction) {
            return Err(TampinesError::Unphysical(format!(
                "void fraction {} is outside [0, 1]; refused rather than clipped",
                self.void_fraction
            )));
        }
        Ok(())
    }
}

/// The three interfacial source terms for one cell, plus the intermediate
/// quantities a caller needs to check them.
///
/// # Units (raw `f64`, strict SI — this is a marching-loop boundary)
///
/// See each field. `uom`-typed accessors are provided for the quantities that
/// have an SI dimension in `uom`'s quantity set; `kg/(m³·s)` and `N/m³` do
/// not, and are documented in prose instead — the same compromise
/// [`InterfacialHeatTransfer::volumetric_coefficient`] makes upstream.
///
/// # Signs, in one place
///
/// - `drag_force_on_vapour > 0` accelerates the vapour in `+x`.
/// - `vapour_heat > 0` heats the vapour (heat flows interface → vapour).
/// - `mass_transfer > 0` is **evaporation**.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InterfacialSources {
    /// Volumetric drag coefficient `K_d` \[kg/(m³·s)\] from the traced-back
    /// [`DragModel`]. Always `≥ 0`.
    pub volumetric_drag_coefficient: f64,
    /// Interfacial drag force on the **vapour** `F_g^i = K_d (u_l − u_g)`
    /// \[N/m³\]. The force on the liquid is exactly its negative — interfacial
    /// momentum exchange conserves momentum by construction, which is why only
    /// one of the pair is stored.
    ///
    /// This expression is independent of which phase is dispersed: the drag
    /// force on the dispersed phase is `K_d (u_continuous − u_dispersed)`, and
    /// substituting either role assignment gives `K_d (u_l − u_g)` for the
    /// vapour.
    pub drag_force_on_vapour: f64,
    /// Interfacial heat transfer **into the vapour** `Q_g^i = K_g (T_sat − T_g)`
    /// \[W/m³\]. Negative when the vapour is superheated.
    pub vapour_heat: f64,
    /// Interfacial heat transfer **into the liquid** `Q_l^i = K_l (T_sat − T_l)`
    /// \[W/m³\]. Negative when the liquid is metastably superheated, positive
    /// when it is subcooled.
    pub liquid_heat: f64,
    /// Interfacial mass transfer `Γ` \[kg/(m³·s)\], **positive for
    /// evaporation** (liquid → vapour). See the module docs for the derivation
    /// and the full sign argument.
    pub mass_transfer: f64,
    /// Interface temperature \[K\] — the saturation temperature `T_sat(p)`, by
    /// assumption. Reported so a caller can see the driving temperature
    /// differences rather than infer them.
    pub interface_temperature: f64,
    /// Vapour bulk temperature `T_g` \[K\], from
    /// [`PhaseState::vapour_at`] (metastable branch included).
    pub vapour_temperature: f64,
    /// Liquid bulk temperature `T_l` \[K\], from
    /// [`PhaseState::liquid_at`] (metastable branch included).
    pub liquid_temperature: f64,
    /// Slip Reynolds number `Re = |u_d − u_c| d / ν_c` \[-\], taken from
    /// [`TwoFluidSystem::reynolds_number`] rather than recomputed here. Both
    /// the drag and the continuous-side heat-transfer correlation are
    /// evaluated at this `Re`.
    pub slip_reynolds_number: f64,
}

impl InterfacialSources {
    /// Interfacial drag force on the **liquid** \[N/m³\] — the negative of
    /// [`drag_force_on_vapour`](Self::drag_force_on_vapour).
    #[must_use]
    pub fn drag_force_on_liquid(self) -> f64 {
        -self.drag_force_on_vapour
    }

    /// [`vapour_heat`](Self::vapour_heat) as a `uom` quantity \[W/m³\].
    #[must_use]
    pub fn vapour_heat_density(self) -> VolumetricPowerDensity {
        VolumetricPowerDensity::new::<watt_per_cubic_meter>(self.vapour_heat)
    }

    /// [`liquid_heat`](Self::liquid_heat) as a `uom` quantity \[W/m³\].
    #[must_use]
    pub fn liquid_heat_density(self) -> VolumetricPowerDensity {
        VolumetricPowerDensity::new::<watt_per_cubic_meter>(self.liquid_heat)
    }

    /// [`interface_temperature`](Self::interface_temperature) as a `uom`
    /// quantity \[K\].
    #[must_use]
    pub fn saturation_temperature(self) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(self.interface_temperature)
    }

    /// Net heat delivered **to the interface** from both phases \[W/m³\],
    /// `−(Q_g^i + Q_l^i)`.
    ///
    /// This is the quantity `Γ h_fg` equals, so it is the single number to
    /// look at when checking a mass-transfer sign: positive means heat is
    /// arriving at the interface and driving evaporation.
    #[must_use]
    pub fn net_heat_to_interface(self) -> f64 {
        -(self.vapour_heat + self.liquid_heat)
    }

    /// `true` when the cell is evaporating (`Γ > 0`).
    #[must_use]
    pub fn is_evaporating(self) -> bool {
        self.mass_transfer > 0.0
    }
}

/// One phase in its **role** (dispersed or continuous), with every property the
/// closures need already resolved.
///
/// Private, and deliberately role-keyed rather than phase-keyed: once the
/// caller's `(vapour, liquid)` pair has been mapped onto `(dispersed,
/// continuous)` exactly once, no downstream code can pick the wrong
/// conductivity, because no downstream code knows which phase it is holding.
#[derive(Debug, Clone, Copy)]
struct PhaseRole {
    /// Volume fraction `α` \[-\].
    alpha: f64,
    /// Density `ρ` \[kg/m³\], from the (possibly metastable) [`PhaseState`].
    rho: f64,
    /// Dynamic viscosity `μ` \[Pa·s\], on the saturation line.
    mu: f64,
    /// Thermal conductivity `λ` \[W/(m·K)\], on the saturation line.
    lambda: f64,
    /// Prandtl number `Pr` \[-\], on the saturation line.
    prandtl: f64,
    /// Axial velocity `u` \[m/s\].
    velocity: f64,
    /// Bulk temperature `T` \[K\].
    temperature: f64,
    /// Phase label for the upstream [`Phase`] and for error messages.
    name: &'static str,
}

/// The interfacial exchange closure set for a 1-D six-equation two-fluid model.
///
/// # What it is
///
/// A fixed choice of *three* closures — one drag model and a two-resistance
/// heat-transfer **pair** — plus the inclusion diameter they all scale with.
/// Given one cell's state it returns the drag, the two interfacial heat
/// fluxes, and the mass transfer they imply. It holds no field data and no
/// time state: it is a pure function of the cell state, evaluated once per
/// cell per timestep by the solver that owns the fields.
///
/// # Why it owns a mesh
///
/// The traced-back [`DragModel::k_d`] takes a
/// [`TwoFluidSystem`], which is field-based. Rather than re-implement its
/// arithmetic — which would be exactly the independent invention the trace-back
/// rule forbids — this type keeps a **single-cell** finite-volume mesh and
/// packs the local state into it for each call, the same round-trip
/// [`super::drift_flux`] performs for `SlipModel`. The mesh's geometry (a 1 m
/// long, 1 m² cell) is never read by any closure: only `n_cells == 1` matters.
///
/// The cost is a handful of small allocations per cell per timestep, because
/// [`Phase`]'s scalar density and viscosity are per-phase constants upstream
/// with no setters, so the phases must be rebuilt when the local density
/// changes — which in a blowdown is every cell and every step. That is a real
/// cost and it is the price of not duplicating the correlation. It is noted
/// here so a future profiler is not surprised.
///
/// # No derives
///
/// Neither [`DragModel`] nor this type derives `Debug`, `Clone`, or `Copy`:
/// the upstream enum derives nothing, and adding a wrapper that pretends
/// otherwise would mean cloning the mesh `Arc` under a `Clone` that looks
/// free.
pub struct InterfacialExchange {
    /// Single-cell scratch mesh — a carrier for the upstream field API only.
    mesh: Arc<FvMesh>,
    /// The traced-back drag closure.
    drag: DragModel,
    /// The closure resolving the **continuous**-side resistance. Asserted at
    /// construction to report [`ResistanceSide::Continuous`].
    continuous_side: InterfacialHeatTransfer,
    /// The closure resolving the **dispersed**-side resistance. Asserted at
    /// construction to report [`ResistanceSide::Dispersed`].
    dispersed_side: InterfacialHeatTransfer,
    /// Which phase is dispersed.
    dispersed: DispersedPhase,
    /// Inclusion diameter `d` \[m\], strictly positive.
    diameter: f64,
    /// Residual volume-fraction floor `α_res` \[-\] handed to the heat-transfer
    /// closures.
    residual_alpha: f64,
}

impl InterfacialExchange {
    /// Assemble an interfacial closure set.
    ///
    /// # Arguments
    ///
    /// - `drag` — the traced-back momentum closure; see [`DragModel`].
    /// - `continuous_side` — the heat-transfer closure for the resistance
    ///   *outside* the inclusion. Must report
    ///   [`ResistanceSide::Continuous`], i.e. be
    ///   [`RanzMarshall`](InterfacialHeatTransfer::RanzMarshall) or
    ///   [`Gunn`](InterfacialHeatTransfer::Gunn).
    /// - `dispersed_side` — the heat-transfer closure for conduction *inside*
    ///   the inclusion. Must report [`ResistanceSide::Dispersed`], i.e. be
    ///   [`Spherical`](InterfacialHeatTransfer::Spherical).
    /// - `dispersed` — which phase is dispersed; see [`DispersedPhase`].
    /// - `diameter` — inclusion diameter `d` \[m\] (`uom`), strictly positive
    ///   and finite. Physically 1e-4 m to 1e-2 m for steam bubbles; nothing
    ///   here enforces that upper end, but every closure scales as `1/d²`, so
    ///   an unphysical `d` is silently influential.
    /// - `residual_alpha` — the volume-fraction floor `α_res` \[-\] the
    ///   heat-transfer closures apply as `max(α_d, α_res)`, in `[0, 1)`.
    ///   [`RESIDUAL_ALPHA`] (`1e-6`) is the upstream default and what
    ///   [`bubbly`](Self::bubbly) uses. Note the **drag** closure hard-codes
    ///   [`RESIDUAL_ALPHA`] upstream and ignores this value — that asymmetry
    ///   is upstream's, not this module's.
    ///
    /// # Errors
    ///
    /// [`TampinesError::InvalidInput`] if the two heat-transfer models do not
    /// form one continuous-side and one dispersed-side resistance (the silent
    /// factor-of-ten error described in the module docs), if `diameter` is not
    /// strictly positive and finite, or if `residual_alpha` is outside
    /// `[0, 1)`. [`TampinesError::Numerical`] if the single-cell scratch mesh
    /// cannot be built, which would mean a defect in
    /// [`create_one_d_mesh`], not a caller error.
    pub fn new(
        drag: DragModel,
        continuous_side: InterfacialHeatTransfer,
        dispersed_side: InterfacialHeatTransfer,
        dispersed: DispersedPhase,
        diameter: Length,
        residual_alpha: f64,
    ) -> Result<Self, TampinesError> {
        if continuous_side.resistance_side() != ResistanceSide::Continuous {
            return Err(TampinesError::InvalidInput(format!(
                "the continuous-side heat-transfer model must resolve the continuous \
                 resistance, but {continuous_side:?} reports {:?}; using two models from \
                 the same side is wrong by the phase conductivity ratio (about 10x for \
                 steam/water) and would not otherwise be visible",
                continuous_side.resistance_side()
            )));
        }
        if dispersed_side.resistance_side() != ResistanceSide::Dispersed {
            return Err(TampinesError::InvalidInput(format!(
                "the dispersed-side heat-transfer model must resolve the dispersed \
                 resistance, but {dispersed_side:?} reports {:?}; using two models from \
                 the same side is wrong by the phase conductivity ratio (about 10x for \
                 steam/water) and would not otherwise be visible",
                dispersed_side.resistance_side()
            )));
        }

        let d = diameter.get::<meter>();
        if !(d > 0.0) || !d.is_finite() {
            return Err(TampinesError::InvalidInput(format!(
                "inclusion diameter must be finite and > 0 m (got {d})"
            )));
        }
        if !(0.0..1.0).contains(&residual_alpha) || !residual_alpha.is_finite() {
            return Err(TampinesError::InvalidInput(format!(
                "residual alpha must be in [0, 1) (got {residual_alpha})"
            )));
        }

        let mesh = Arc::new(
            create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), 1)
                .map_err(|e| {
                    TampinesError::Numerical(format!(
                        "single-cell scratch mesh construction failed: {e}"
                    ))
                })?,
        );

        Ok(Self {
            mesh,
            drag,
            continuous_side,
            dispersed_side,
            dispersed,
            diameter: d,
            residual_alpha,
        })
    }

    /// The conventional **bubbly-flow** closure set: Schiller-Naumann drag,
    /// Ranz-Marshall on the continuous (liquid) side, spherical conduction on
    /// the dispersed (vapour) side, with the upstream [`RESIDUAL_ALPHA`] floor.
    ///
    /// This is the set a subcooled-boiling or low-void blowdown cell wants.
    /// It is a *convenience*, not a recommendation: it inherits every
    /// limitation in the module's "Honest scope", in particular that it will
    /// keep being used at void fractions where bubbly flow no longer exists.
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new) — in practice only a non-positive `diameter`.
    pub fn bubbly(diameter: Length) -> Result<Self, TampinesError> {
        Self::new(
            DragModel::SchillerNaumann,
            InterfacialHeatTransfer::RanzMarshall,
            InterfacialHeatTransfer::Spherical,
            DispersedPhase::Vapour,
            diameter,
            RESIDUAL_ALPHA,
        )
    }

    /// The conventional **droplet / mist-flow** closure set: the same three
    /// correlations with the phase roles exchanged — liquid droplets dispersed
    /// in continuous vapour, so Ranz-Marshall now reads the **vapour**
    /// conductivity and spherical conduction the **liquid** conductivity.
    ///
    /// # Errors
    ///
    /// As [`new`](Self::new).
    pub fn droplet(diameter: Length) -> Result<Self, TampinesError> {
        Self::new(
            DragModel::SchillerNaumann,
            InterfacialHeatTransfer::RanzMarshall,
            InterfacialHeatTransfer::Spherical,
            DispersedPhase::Liquid,
            diameter,
            RESIDUAL_ALPHA,
        )
    }

    /// Which phase this closure set treats as dispersed.
    #[must_use]
    pub fn dispersed_phase(&self) -> DispersedPhase {
        self.dispersed
    }

    /// The continuous-side heat-transfer closure.
    #[must_use]
    pub fn continuous_side_model(&self) -> InterfacialHeatTransfer {
        self.continuous_side
    }

    /// The dispersed-side heat-transfer closure.
    #[must_use]
    pub fn dispersed_side_model(&self) -> InterfacialHeatTransfer {
        self.dispersed_side
    }

    /// The prescribed inclusion diameter `d` \[m\].
    #[must_use]
    pub fn diameter(&self) -> Length {
        Length::new::<meter>(self.diameter)
    }

    /// The residual volume-fraction floor `α_res` \[-\] used by the
    /// heat-transfer closures.
    #[must_use]
    pub fn residual_alpha(&self) -> f64 {
        self.residual_alpha
    }

    /// Evaluate all three interfacial source terms for one cell, evaluating the
    /// saturation and transport property sets at `cell.pressure` on the way.
    ///
    /// Convenience entry point for one-off evaluations and for tests. A
    /// marching loop should call
    /// [`sources_with_properties`](Self::sources_with_properties) with its own
    /// cached sets instead: this method costs a full IF97 saturation call plus
    /// four more IF97 evaluations (including the R15-11 conductivity with its
    /// critical enhancement) **every call**, which is the most expensive
    /// property work in [`super::properties`].
    ///
    /// # Errors
    ///
    /// As [`sources_with_properties`](Self::sources_with_properties), plus
    /// [`TampinesError::Unphysical`] if `cell.pressure` is outside the IF97
    /// range.
    pub fn sources_si(
        &self,
        cell: InterfacialCellState,
    ) -> Result<InterfacialSources, TampinesError> {
        cell.check()?;
        let saturated = SaturatedProperties::at(cell.pressure)?;
        let transport = SaturatedTransport::at(cell.pressure)?;
        self.sources_with_properties(cell, saturated, transport)
    }

    /// Evaluate all three interfacial source terms for one cell against
    /// **caller-supplied** property sets.
    ///
    /// This is the marching-loop entry point: a 1-D solver already holds a
    /// [`SaturatedProperties`] and a [`SaturatedTransport`] per cell and
    /// refreshes them only when
    /// [`is_stale_for`](SaturatedProperties::is_stale_for) says so, so making
    /// them arguments avoids re-evaluating IF97 for every closure call.
    ///
    /// # What is computed, in order
    ///
    /// 1. Per-phase temperature and density from [`PhaseState::vapour_at`] /
    ///    [`PhaseState::liquid_at`], so a metastable phase keeps its own
    ///    temperature rather than being flashed back onto the saturation line.
    /// 2. The phases are mapped onto their dispersed/continuous **roles** once,
    ///    and everything downstream is role-keyed.
    /// 3. A single-cell [`TwoFluidSystem`] is packed and handed to
    ///    [`DragModel::k_d`] and [`TwoFluidSystem::reynolds_number`].
    /// 4. Both volumetric heat-transfer coefficients come from
    ///    [`InterfacialHeatTransfer::volumetric_coefficient_si`], each given
    ///    its own phase's Prandtl number, with the two conductivities passed
    ///    by role.
    /// 5. `Γ` closes the energy jump — see the module docs.
    ///
    /// # Arguments
    ///
    /// - `cell` — the local state; raw `f64` SI, see [`InterfacialCellState`].
    /// - `saturated` — saturation set, which must be at `cell.pressure` (the
    ///   property layer checks this to a relative `1e-9`).
    /// - `transport` — conduction set, which must be at `cell.pressure` to
    ///   within [`super::properties::TRANSPORT_CACHE_TOLERANCE`].
    ///
    /// # Errors
    ///
    /// - [`TampinesError::InvalidInput`] for a non-finite state entry.
    /// - [`TampinesError::Unphysical`] for a void fraction outside `[0, 1]`; a
    ///   stale `transport` or `saturated` set; a phase enthalpy beyond the
    ///   metastable bracket; a non-positive continuous-phase viscosity (which
    ///   would make the slip Reynolds number undefined); a pressure at or above
    ///   [`P_CRITICAL_WATER`]; or `h_fg` below
    ///   [`MIN_LATENT_HEAT_FOR_MASS_TRANSFER`]. The last two are the
    ///   critical-point degeneracy described in the module docs. **None of
    ///   these is clamped.**
    /// - [`TampinesError::Closure`] if the traced-back drag or heat-transfer
    ///   closure rejects its own inputs.
    /// - [`TampinesError::Numerical`] if an IF97 inversion cannot bracket its
    ///   root.
    pub fn sources_with_properties(
        &self,
        cell: InterfacialCellState,
        saturated: SaturatedProperties,
        transport: SaturatedTransport,
    ) -> Result<InterfacialSources, TampinesError> {
        cell.check()?;
        if cell.pressure >= P_CRITICAL_WATER {
            return Err(TampinesError::Unphysical(format!(
                "pressure {} Pa is at or above the critical pressure {P_CRITICAL_WATER} Pa: \
                 there is no saturation line, so the interface cannot be placed at T_sat and \
                 the interfacial mass transfer is not defined by the energy jump this closure \
                 uses. Refused rather than evaluated against a fictitious saturation set \
                 (SaturatedProperties::at accepts pressures to 100 MPa and reports a \
                 non-zero h_fg here)",
                cell.pressure
            )));
        }
        if transport.is_stale_for(cell.pressure) {
            return Err(TampinesError::Unphysical(format!(
                "conduction property set is at {} Pa but the cell is at {} Pa, beyond the \
                 transport cache tolerance; re-evaluate SaturatedTransport::at rather than \
                 using a stale set",
                transport.pressure, cell.pressure
            )));
        }

        // 1. Per-phase state, metastable branches included.
        let vapour = PhaseState::vapour_at(cell.pressure, cell.vapour_enthalpy, saturated)?;
        let liquid = PhaseState::liquid_at(cell.pressure, cell.liquid_enthalpy, saturated)?;

        // 2. Map phases onto roles, exactly once.
        let vapour_role = PhaseRole {
            alpha: cell.void_fraction,
            rho: vapour.density,
            mu: saturated.mu_g,
            lambda: transport.lambda_g,
            prandtl: transport.vapour_prandtl(saturated)?,
            velocity: cell.vapour_velocity,
            temperature: vapour.temperature,
            name: "vapour",
        };
        let liquid_role = PhaseRole {
            alpha: 1.0 - cell.void_fraction,
            rho: liquid.density,
            mu: saturated.mu_f,
            lambda: transport.lambda_f,
            prandtl: transport.liquid_prandtl(saturated)?,
            velocity: cell.liquid_velocity,
            temperature: liquid.temperature,
            name: "liquid",
        };
        let (dispersed, continuous) = match self.dispersed {
            DispersedPhase::Vapour => (vapour_role, liquid_role),
            DispersedPhase::Liquid => (liquid_role, vapour_role),
        };

        if !(continuous.mu > 0.0) {
            return Err(TampinesError::Unphysical(format!(
                "continuous-phase ({}) dynamic viscosity is {} Pa s at p = {} Pa; the slip \
                 Reynolds number the drag and heat-transfer closures need is undefined",
                continuous.name, continuous.mu, cell.pressure
            )));
        }

        // 3. Drag and Reynolds number, from the traced-back closure.
        let (k_d, reynolds) = self.drag_and_reynolds(dispersed, continuous)?;
        let drag_force_on_vapour = k_d * (cell.liquid_velocity - cell.vapour_velocity);

        // 4. Two-resistance heat transfer. Both calls receive the SAME
        //    (kappa_continuous, kappa_dispersed) pair in the same argument
        //    positions; the variant picks the side it closes. Only the Prandtl
        //    number differs, because it belongs to the phase whose boundary
        //    layer is being resolved.
        let k_continuous_side = self.continuous_side.volumetric_coefficient_si(
            dispersed.alpha,
            continuous.alpha,
            continuous.lambda,
            dispersed.lambda,
            self.diameter,
            reynolds,
            continuous.prandtl,
            self.residual_alpha,
        )?;
        let k_dispersed_side = self.dispersed_side.volumetric_coefficient_si(
            dispersed.alpha,
            continuous.alpha,
            continuous.lambda,
            dispersed.lambda,
            self.diameter,
            reynolds,
            dispersed.prandtl,
            self.residual_alpha,
        )?;

        let q_continuous = k_continuous_side * (saturated.t_sat - continuous.temperature);
        let q_dispersed = k_dispersed_side * (saturated.t_sat - dispersed.temperature);
        let (vapour_heat, liquid_heat) = match self.dispersed {
            DispersedPhase::Vapour => (q_dispersed, q_continuous),
            DispersedPhase::Liquid => (q_continuous, q_dispersed),
        };

        // 5. Mass transfer from the interfacial energy jump.
        let h_fg = saturated.h_fg();
        if !(h_fg >= MIN_LATENT_HEAT_FOR_MASS_TRANSFER) {
            return Err(TampinesError::Unphysical(format!(
                "latent heat h_fg = {h_fg} J/kg at p = {} Pa is below the \
                 {MIN_LATENT_HEAT_FOR_MASS_TRANSFER} J/kg floor: the interface is at or \
                 above the critical point, where the energy jump no longer closes the \
                 mass-transfer rate. Refused rather than returning a large finite Gamma \
                 that would look like an answer",
                cell.pressure
            )));
        }
        let mass_transfer = -(vapour_heat + liquid_heat) / h_fg;

        Ok(InterfacialSources {
            volumetric_drag_coefficient: k_d,
            drag_force_on_vapour,
            vapour_heat,
            liquid_heat,
            mass_transfer,
            interface_temperature: saturated.t_sat,
            vapour_temperature: vapour.temperature,
            liquid_temperature: liquid.temperature,
            slip_reynolds_number: reynolds,
        })
    }

    /// Pack the two roles into a single-cell [`TwoFluidSystem`] and read the
    /// traced-back drag coefficient `K_d` \[kg/(m³·s)\] and slip Reynolds
    /// number `Re` \[-\] out of it.
    ///
    /// Both numbers are produced by the reference crate's own code, so an
    /// upstream correction propagates here instead of needing to be mirrored.
    fn drag_and_reynolds(
        &self,
        dispersed: PhaseRole,
        continuous: PhaseRole,
    ) -> Result<(f64, f64), TampinesError> {
        let dispersed_phase = Phase::new(
            self.mesh.clone(),
            dispersed.name,
            MassDensity::new::<kilogram_per_cubic_meter>(dispersed.rho),
            DynamicViscosity::new::<pascal_second>(dispersed.mu),
            Length::new::<meter>(self.diameter),
            dispersed.alpha,
        )?;
        let continuous_phase = Phase::new(
            self.mesh.clone(),
            continuous.name,
            MassDensity::new::<kilogram_per_cubic_meter>(continuous.rho),
            DynamicViscosity::new::<pascal_second>(continuous.mu),
            Length::new::<meter>(self.diameter),
            continuous.alpha,
        )?;

        let mut system = TwoFluidSystem::new(dispersed_phase, continuous_phase)?;
        system.dispersed.u_mut().internal.as_mut_slice()[0] =
            Vector3::new(dispersed.velocity, 0.0, 0.0);
        system.continuous.u_mut().internal.as_mut_slice()[0] =
            Vector3::new(continuous.velocity, 0.0, 0.0);

        let reynolds = system.reynolds_number()?.internal.as_slice()[0];
        let k_d = self.drag.k_d(&system)?.internal.as_slice()[0];
        Ok((k_d, reynolds))
    }
}

#[cfg(test)]
mod tests;
