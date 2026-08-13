// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
//   `offbeatLib/fvPatchFields/temperatureCoupled/fuelRodGapFvPatchScalarField.C`
//     (`gapWidth()`, `hGap()` — the gas / radiation / contact terms),
//   `offbeatLib/fvPatchFields/temperatureCoupled/trisoGapFvPatchScalarField.C`
//     (the spherical-shell variant of the gas term),
//   `offbeatLib/fvPatchFields/temperatureCoupled/resistiveGapFvPatchScalarField.C`
//     (`weights()` — the series interface resistance).
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Fuel/cladding gap conductance: gas conduction, radiation and solid contact.
//!
//! # What this computes
//!
//! A heat-transfer coefficient `h` \[W/m²K\] across the fuel/cladding interface,
//! such that the heat flux is `q'' = h · (T_fuel − T_clad)`. It is the sum of
//! three **parallel** paths, because all three carry heat across the same
//! interface at the same time:
//!
//! ```text
//! h_gap = h_gas + h_radiation + h_contact
//! ```
//!
//! - **`h_gas`** — conduction through the fill gas, `k_gas` divided by an
//!   *effective* gap thickness. The effective thickness is not the geometric gap:
//!   it adds the surface roughness (the surfaces are rough, so gas is trapped in
//!   the asperity valleys), subtracts an empirical offset, and adds a
//!   temperature-jump distance at each wall (gas molecules do not fully
//!   equilibrate with a solid in one collision).
//! - **`h_radiation`** — gray-body exchange between the two surfaces, linearised
//!   about the surface temperatures so it can enter a linear solve as a
//!   coefficient.
//! - **`h_contact`** — solid conduction through the asperity contact spots once
//!   the surfaces bear on each other. Zero at zero interface pressure, and the
//!   dominant term once the gap is hard-closed.
//!
//! # Gap conventions in this module
//!
//! **Every length here is RADIAL** (see the [module-level conventions]
//! (super#gap-conventions--read-this-before-using-anything-here)). In particular
//! [`GapConductanceModel::FuelRodFrapcon::radial_gap_width`] is the *radial*
//! surface-to-surface separation and is **unsigned**: `0` means the surfaces
//! touch. It is not a diametral gap; halve a diametral input before passing it.
//!
//! Upstream computes that width as
//! `max((C_clad + D_clad − C_fuel − D_fuel) · n, 0)` on each interface face —
//! the deformed face-centre separation projected on the face normal, clipped at
//! zero. **That computation is deferred here** (it needs the mesh's face centres,
//! face normals and the AMI interpolation between the two regions); this module
//! takes the resulting width as an input.
//!
//! # Deferred
//!
//! - The gap-width evaluation and the AMI interpolation of neighbour
//!   temperatures, conductivities, emissivities and roughnesses, as above.
//! - The owner/neighbour averaging that upstream's `updateCoeffs()` performs
//!   (`alpha = ½(hGap_own + interp(hGap_nbr))`). Ported as far as
//!   [`average_across_interface`], which is the arithmetic without the
//!   interpolation.
//! - The `interfaceP` field, which is produced by [`super::contact`] on the
//!   mechanical side and consumed here as [`GapSurfaces::interface_pressure`].
//!
//! # Units
//!
//! Strict SI raw `f64`: kelvin, metre, pascal, W/m/K for a conductivity,
//! W/m²K for a conductance.

use crate::error::{OffbeatError, Result};
use crate::gap::gas::GapGasMixture;

/// Stefan–Boltzmann constant `σ` \[W/(m²·K⁴)\], SI-2019 exact value.
///
/// Upstream reads `Foam::constant::physicoChemical::sigma`.
pub const STEFAN_BOLTZMANN: f64 = 5.670_374_419e-8;

/// Small positive length/denominator floor \[SI\] used where upstream would
/// divide by zero.
///
/// OpenFOAM's `SMALL` for double precision. Upstream's `trisoGap` guards its
/// divisions with it; upstream's `fuelRodGap` does **not**, and would return an
/// infinity for a perfectly smooth, perfectly closed, zero-jump-distance gap.
/// This port applies the guard in both, so a degenerate input yields a very
/// large finite number rather than an infinity that then poisons a linear solve.
/// This is the module's only numerical deviation from upstream, and it can only
/// fire on inputs upstream would have turned into `inf` or `NaN`.
pub const SMALL: f64 = 1.0e-15;

/// Empirical offset \[m\] subtracted from the roughness-augmented gap width —
/// upstream's literal `1.397e-6` in `fuelRodGap::hGap()`.
///
/// It is 55 microinches expressed in metres (`55e-6 in × 0.0254 m/in`), which is
/// the tell that the correlation was fitted in imperial units. It exists to
/// stop the roughness term over-predicting the gas gap in near-contact
/// conditions; the sum is clipped at zero, so it can never make the effective
/// gap negative.
pub const ROUGHNESS_OFFSET: f64 = 1.397e-6;

/// Multiplier on the temperature-jump distance in the fuel-rod gas term —
/// upstream's literal `1.8` in `fuelRodGap::hGap()`.
///
/// There is one jump at each of the two surfaces, so a factor of 2 would be the
/// naive count; 1.8 is the fitted value. **The TRISO variant does not apply
/// it** — `trisoGap` uses the bare jump distance — and that difference is
/// reproduced.
pub const JUMP_DISTANCE_MULTIPLIER: f64 = 1.8;

/// Coefficient of the temperature-jump-distance correlation \[SI-mixed\] —
/// upstream's literal `0.0137`.
///
/// Appears as `d_jump = 0.0137 · k · sqrt(T) / (p · a)`. It is an empirical
/// constant fitted *with upstream's unnormalised accommodation coefficient*
/// (see [`GapGasMixture::accommodation_coefficient`]) already folded in, which
/// is why this port reproduces that quirk rather than "fixing" it.
pub const JUMP_DISTANCE_COEFFICIENT: f64 = 0.0137;

/// Divisor \[Pa per kgf/cm²\] converting interface pressure to the unit the
/// roughness-compression term expects — upstream's literal `1e4 · 9.8`.
///
/// `pI = interfaceP / 1e4 / 9.8` converts pascal to kilogram-force per square
/// centimetre (the exact conversion is 9.80665e4 Pa; upstream rounds to 9.8e4,
/// a 0.07% difference, reproduced here).
pub const PRESSURE_TO_KGF_PER_CM2: f64 = 1.0e4 * 9.8;

/// Exponential decay coefficient \[per kgf/cm²\] of the roughness contribution
/// under contact pressure — upstream's literal `1.25e-3`.
pub const ROUGHNESS_PRESSURE_COEFFICIENT: f64 = 1.25e-3;

/// Meyer hardness of Zircaloy cladding \[Pa\] — upstream's hard-coded
/// `680e6` in both gap patch fields.
///
/// The contact model's relative pressure is `P_rel = P_interface / H_Meyer`; the
/// harder material of the pair sets how much the asperities flatten. Upstream
/// hard-codes the Zircaloy value and offers no way to change it; this port
/// exposes it as [`GapSurfaces::meyer_hardness`] with this constant as the
/// default, which is a **deliberate widening** of upstream's interface, not a
/// change to its default behaviour.
pub const MEYER_HARDNESS_ZIRCALOY: f64 = 680.0e6;

/// The three parallel contributions to gap conductance, kept separate.
///
/// Upstream sums them into a single `hGap_` and discards the split. Keeping it
/// is the difference between "the gap conductance is 5000 W/m²K" and "the gap
/// conductance is 5000 W/m²K and 94% of it is contact", which is what a reader
/// actually needs to interpret a rod history.
///
/// # Units
///
/// All three in W/(m²·K).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GapConductance {
    /// Conduction through the fill gas \[W/m²K\] — upstream's `hGas`.
    pub gas: f64,
    /// Gray-body radiation between the surfaces \[W/m²K\] — upstream's `hRad`.
    pub radiation: f64,
    /// Solid conduction through asperity contacts \[W/m²K\] — upstream's
    /// `hContact`. Zero when the interface pressure is zero.
    pub contact: f64,
}

impl GapConductance {
    /// Total gap conductance \[W/m²K\] — the sum of the three parallel paths.
    ///
    /// This is the number that multiplies `(T_fuel − T_clad)` to give the
    /// interface heat flux, and the number upstream stores as `hGap_`.
    #[must_use]
    pub fn total(&self) -> f64 {
        self.gas + self.radiation + self.contact
    }

    /// Fraction \[-\] of the total carried by solid contact, in `[0, 1]`.
    ///
    /// Returns `0.0` for a zero total. Useful for reading a rod history: the
    /// transition of this number from ~0 to ~1 *is* gap closure.
    #[must_use]
    pub fn contact_fraction(&self) -> f64 {
        let t = self.total();
        if t > 0.0 {
            self.contact / t
        } else {
            0.0
        }
    }
}

/// Per-term linear scaling `h → F·h + δ`, for sensitivity studies —
/// upstream's `F_hGas`/`F_hRad`/`F_hContact` and
/// `delta_hGas`/`delta_hRad`/`delta_hContact` parameters on
/// `fuelRodGapFvPatchScalarField`.
///
/// # Units
///
/// The `*_factor` fields are dimensionless; the `*_offset` fields are in
/// W/(m²·K). [`Default`] is the identity (`factor = 1`, `offset = 0`), i.e.
/// upstream's defaults.
///
/// # Note
///
/// Upstream applies the factor to the **clipped** term
/// (`F · max(h, 0) + δ`), so a negative offset *can* drive a term negative.
/// That is reproduced; use [`GapConductanceModel::evaluate_checked`] if you
/// need a negative total to be an error rather than a number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GapConductanceScaling {
    /// Multiplier \[-\] on the gas term.
    pub gas_factor: f64,
    /// Additive offset \[W/m²K\] on the gas term.
    pub gas_offset: f64,
    /// Multiplier \[-\] on the radiation term.
    pub radiation_factor: f64,
    /// Additive offset \[W/m²K\] on the radiation term.
    pub radiation_offset: f64,
    /// Multiplier \[-\] on the contact term.
    pub contact_factor: f64,
    /// Additive offset \[W/m²K\] on the contact term.
    pub contact_offset: f64,
}

impl Default for GapConductanceScaling {
    /// Upstream's defaults: every factor 1, every offset 0 — i.e. no scaling.
    fn default() -> Self {
        Self {
            gas_factor: 1.0,
            gas_offset: 0.0,
            radiation_factor: 1.0,
            radiation_offset: 0.0,
            contact_factor: 1.0,
            contact_offset: 0.0,
        }
    }
}

/// The state of the two surfaces bounding the gap, on one interface face.
///
/// Upstream gathers these by looking patch fields up by name on both sides of a
/// `regionCoupledOFFBEAT` patch and interpolating the neighbour's through the
/// AMI. This port takes them as an explicit argument so the dependencies of the
/// gap model are visible in its signature.
///
/// # Naming
///
/// "Fuel" is the inner surface and "clad" the outer one for a fuel rod. For a
/// TRISO particle they are the inner and outer surfaces of the shell gap; the
/// physics is symmetric in the two apart from which radius is which.
///
/// # Units
///
/// Strict SI: kelvin, metre, W/m/K, pascal; emissivity dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GapSurfaces {
    /// Fuel (inner) surface temperature \[K\]. Absolute; must be > 0.
    pub fuel_temperature: f64,
    /// Cladding (outer) surface temperature \[K\]. Absolute; must be > 0.
    pub clad_temperature: f64,
    /// Fuel surface arithmetic-mean roughness \[m\], **radial**.
    ///
    /// Upstream's `roughness` patch entry. Typical as-fabricated UO2 pellet
    /// surface: ~1e-6 m (1 µm).
    pub fuel_roughness: f64,
    /// Cladding inner-surface arithmetic-mean roughness \[m\], **radial**.
    ///
    /// Typical as-fabricated Zircaloy inner surface: ~0.5e-6 m.
    pub clad_roughness: f64,
    /// Fuel surface total hemispherical emissivity \[-\], in `(0, 1]`.
    ///
    /// UO2 is roughly 0.85. Values are floored at [`SMALL`] internally, matching
    /// upstream, so a zero emissivity gives zero radiative transfer rather than
    /// a division by zero.
    pub fuel_emissivity: f64,
    /// Cladding inner-surface emissivity \[-\], in `(0, 1]`. Oxidised Zircaloy
    /// is roughly 0.8.
    pub clad_emissivity: f64,
    /// Fuel **solid** thermal conductivity at the surface \[W/m/K\].
    ///
    /// Used only by the contact term, which conducts through the solid asperity
    /// spots. UO2 near 1000 K is roughly 3 W/m/K. Not to be confused with the
    /// gas conductivity, which comes from the [`GapGasMixture`].
    pub fuel_conductivity: f64,
    /// Cladding **solid** thermal conductivity at the surface \[W/m/K\].
    /// Zircaloy is roughly 15 W/m/K.
    pub clad_conductivity: f64,
    /// Normal interface (contact) pressure \[Pa\], `>= 0`.
    ///
    /// Zero for an open gap. Produced on the mechanical side by
    /// [`super::contact::PenaltyContact::interface_pressure`]; upstream passes
    /// it through the `interfaceP` field.
    pub interface_pressure: f64,
    /// Meyer hardness of the softer contacting material \[Pa\].
    ///
    /// Upstream hard-codes [`MEYER_HARDNESS_ZIRCALOY`]; use that value to
    /// reproduce upstream exactly.
    pub meyer_hardness: f64,
}

impl GapSurfaces {
    /// A representative open-gap LWR interface at the given fuel and cladding
    /// surface temperatures \[K\], with zero interface pressure.
    ///
    /// Roughnesses 1.0e-6 m (fuel) and 0.5e-6 m (cladding); emissivities 0.85
    /// (UO2) and 0.80 (oxidised Zircaloy); solid conductivities 3.0 and
    /// 15.0 W/m/K; Meyer hardness [`MEYER_HARDNESS_ZIRCALOY`]. These are
    /// order-of-magnitude design values for orientation and for tests — they are
    /// **not** taken from a specific measured rod, and no result computed from
    /// them may be described as validated.
    #[must_use]
    pub fn lwr_open_gap(fuel_temperature: f64, clad_temperature: f64) -> Self {
        Self {
            fuel_temperature,
            clad_temperature,
            fuel_roughness: 1.0e-6,
            clad_roughness: 0.5e-6,
            fuel_emissivity: 0.85,
            clad_emissivity: 0.80,
            fuel_conductivity: 3.0,
            clad_conductivity: 15.0,
            interface_pressure: 0.0,
            meyer_hardness: MEYER_HARDNESS_ZIRCALOY,
        }
    }

    /// Arithmetic mean of the two surface temperatures \[K\] — the film
    /// temperature upstream evaluates the gas properties at
    /// (`T = 0.5*(pT[i] + nbrT[i])`).
    #[must_use]
    pub fn mean_temperature(&self) -> f64 {
        0.5 * (self.fuel_temperature + self.clad_temperature)
    }

    /// Reject a physically impossible surface state.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a non-positive absolute temperature, a
    /// negative roughness, an emissivity outside `(0, 1]`, a negative solid
    /// conductivity, a negative interface pressure, or a non-positive Meyer
    /// hardness.
    pub fn validate(&self) -> Result<()> {
        let positive = [
            ("fuel surface temperature", self.fuel_temperature, "K"),
            ("cladding surface temperature", self.clad_temperature, "K"),
            ("Meyer hardness", self.meyer_hardness, "Pa"),
        ];
        for (quantity, value, unit) in positive {
            if !(value > 0.0) || !value.is_finite() {
                return Err(OffbeatError::Unphysical {
                    quantity,
                    value,
                    unit,
                    reason: "must be finite and strictly positive",
                });
            }
        }
        let non_negative = [
            ("fuel surface roughness", self.fuel_roughness, "m"),
            ("cladding surface roughness", self.clad_roughness, "m"),
            ("fuel solid conductivity", self.fuel_conductivity, "W/m/K"),
            (
                "cladding solid conductivity",
                self.clad_conductivity,
                "W/m/K",
            ),
            ("interface pressure", self.interface_pressure, "Pa"),
        ];
        for (quantity, value, unit) in non_negative {
            if !(value >= 0.0) || !value.is_finite() {
                return Err(OffbeatError::Unphysical {
                    quantity,
                    value,
                    unit,
                    reason: "must be finite and non-negative",
                });
            }
        }
        for (quantity, value) in [
            ("fuel emissivity", self.fuel_emissivity),
            ("cladding emissivity", self.clad_emissivity),
        ] {
            if !(value > 0.0) || value > 1.0 {
                return Err(OffbeatError::Unphysical {
                    quantity,
                    value,
                    unit: "-",
                    reason: "emissivity must lie in (0, 1]",
                });
            }
        }
        Ok(())
    }
}

/// Where the TRISO gas term's temperature-jump distance comes from — upstream's
/// `jumpDistance` patch entry on `trisoGapFvPatchScalarField`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrisoJumpDistance {
    /// Compute it from the FRAPCON correlation, as the fuel-rod model does.
    ///
    /// Upstream selects this branch when the `jumpDistance` entry is negative on
    /// **both** patches (its default is `-1`); a value given on only one side is
    /// a fatal error there, and is unrepresentable here by construction.
    Frapcon,
    /// Use prescribed per-surface jump distances \[m\], **radial**.
    ///
    /// The two are summed. Upstream requires both to be non-negative.
    Prescribed {
        /// Jump distance at the inner surface \[m\].
        inner: f64,
        /// Jump distance at the outer surface \[m\].
        outer: f64,
    },
}

/// Gap heat-transfer models — one variant per gap boundary condition upstream
/// compiles.
///
/// Dispatch is by `match`, never by a trait object, per the workspace
/// `CLAUDE.md` "No trait objects" rule.
///
/// # Why geometry lives on the variant
///
/// The gap width (rod) and the two radii (TRISO) change every outer iteration as
/// the mechanics solve deforms the two bodies. They sit on the variant rather
/// than in [`GapSurfaces`] because they are *geometry*, not surface state, and
/// because that mirrors the crate's existing precedent in
/// [`crate::materials::behavioral::relocation`]. **Reconstruct the variant when
/// the geometry changes** — it is `Copy`, so this is free.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GapConductanceModel {
    /// A user-prescribed constant interface conductance \[W/m²K\] — upstream
    /// `resistiveGapFvPatchScalarField` with its `alpha` entry.
    ///
    /// All of it is attributed to [`GapConductance::gas`], because upstream's
    /// `resistiveGap` makes no split; the radiation and contact terms are zero.
    /// Use it for a prescribed-conductance sensitivity study or to reproduce a
    /// legacy case, not to model gap closure — a fixed conductance cannot
    /// represent the closure feedback loop that dominates rod behaviour.
    Fixed {
        /// The prescribed interface conductance \[W/m²K\], `>= 0`.
        coefficient: f64,
    },

    /// Fuel-rod gap, FRAPCON form — upstream `fuelRodGapFvPatchScalarField`,
    /// patch type `fuelRodGap`.
    ///
    /// The full three-path model: gas conduction across a roughness- and
    /// jump-augmented planar gap, gray-body radiation, and Ross–Stoute-style
    /// contact conduction. This is the model an LWR rod calculation uses.
    ///
    /// # Valid range
    ///
    /// `radial_gap_width >= 0` (see the field). Surface temperatures within the
    /// range of the gas conductivity fits, roughly 300–2000 K. Interface
    /// pressures from 0 to a substantial fraction of the Meyer hardness; the
    /// contact correlation's branches are set by `P_interface / H_Meyer`.
    FuelRodFrapcon {
        /// **RADIAL** gap width \[m\], **unsigned**: `0` means the surfaces
        /// touch, positive means open.
        ///
        /// This is *not* a diametral gap. Upstream's
        /// `fuelRodGapFvPatchScalarField::gapWidth()` computes it as the
        /// deformed face-centre separation projected on the face normal and
        /// clips it at zero, so it carries no information about how hard a
        /// closed gap is closed — that arrives as
        /// [`GapSurfaces::interface_pressure`]. Typical as-fabricated LWR value:
        /// 8.5e-5 m (half of a 170 µm diametral gap).
        radial_gap_width: f64,
        /// Per-term linear scaling, for sensitivity studies.
        /// [`GapConductanceScaling::default`] reproduces upstream's defaults.
        scaling: GapConductanceScaling,
    },

    /// TRISO-particle shell gap — upstream `trisoGapFvPatchScalarField`, patch
    /// type `trisoGap`.
    ///
    /// Identical radiation and contact terms to
    /// [`FuelRodFrapcon`](Self::FuelRodFrapcon), but the gas term uses the
    /// **spherical-shell** conduction length `r_ref²·(1/r_in − 1/r_out)` in
    /// place of a planar gap width, and applies the bare jump distance rather
    /// than [`JUMP_DISTANCE_MULTIPLIER`] times it. It also omits the
    /// [`ROUGHNESS_OFFSET`] subtraction. All three differences are upstream's
    /// and are reproduced.
    ///
    /// # Valid range
    ///
    /// `0 < r_in <= r_out`, `reference_radius > 0`.
    TrisoSpherical {
        /// Radius of this side of the gap \[m\], **radial** — upstream's
        /// `r1_`, the radius of the patch the coefficient is being evaluated on.
        ///
        /// # Upstream asymmetry, reproduced
        ///
        /// Upstream's spherical conduction length is `r1²·(1/r_in − 1/r_out)`,
        /// using the **current patch's** radius squared, not `r_in²`. The
        /// textbook shell resistance referred to the inner surface uses `r_in²`.
        /// Consequently each side of the interface computes a different `h_gas`,
        /// and `updateCoeffs()` averages the two
        /// ([`average_across_interface`]). Set this to `inner_radius` to recover
        /// the textbook inner-referred form.
        reference_radius: f64,
        /// Inner radius of the gap \[m\], **radial**. Must be `> 0`.
        inner_radius: f64,
        /// Outer radius of the gap \[m\], **radial**. Must be `>= inner_radius`.
        outer_radius: f64,
        /// Where the temperature-jump distance comes from.
        jump_distance: TrisoJumpDistance,
    },
}

impl GapConductanceModel {
    /// Evaluate the three parallel conductance terms \[W/m²K\].
    ///
    /// `gas` is the gap gas composition, `gas_pressure` \[Pa\] the rod internal
    /// pressure (which enters only through the temperature-jump distance), and
    /// `surfaces` the state of the two bounding surfaces.
    ///
    /// # Behaviour at the limits
    ///
    /// - **Open gap, zero interface pressure**: `contact` is exactly zero and
    ///   the total reduces to gas conduction plus radiation.
    /// - **Narrowing gap**: `gas` rises monotonically, without bound as the
    ///   effective thickness approaches zero — which is why the interface
    ///   pressure, not the width, must take over once the surfaces touch.
    /// - **Non-finite or non-positive inputs**: guarded, returning `0.0` for the
    ///   affected term rather than a `NaN`. Use
    ///   [`evaluate_checked`](Self::evaluate_checked) to be told instead.
    ///
    /// ```
    /// use outram_park_fork_offbeat::gap::{
    ///     GapConductanceModel, GapConductanceScaling, GapGasMixture, GapGasSpecies, GapSurfaces,
    /// };
    ///
    /// let helium = GapGasMixture::pure(GapGasSpecies::Helium, 1.0e-5).unwrap();
    /// let surfaces = GapSurfaces::lwr_open_gap(900.0, 600.0);
    ///
    /// let wide = GapConductanceModel::FuelRodFrapcon {
    ///     radial_gap_width: 8.5e-5,
    ///     scaling: GapConductanceScaling::default(),
    /// };
    /// let narrow = GapConductanceModel::FuelRodFrapcon {
    ///     radial_gap_width: 1.0e-5,
    ///     scaling: GapConductanceScaling::default(),
    /// };
    ///
    /// // Narrowing the gap raises the conductance.
    /// assert!(narrow.evaluate(&helium, 2.25e6, &surfaces).total()
    ///       > wide.evaluate(&helium, 2.25e6, &surfaces).total());
    ///
    /// // With an open gap there is no contact conduction at all.
    /// assert_eq!(wide.evaluate(&helium, 2.25e6, &surfaces).contact, 0.0);
    /// ```
    #[must_use]
    pub fn evaluate(
        &self,
        gas: &GapGasMixture,
        gas_pressure: f64,
        surfaces: &GapSurfaces,
    ) -> GapConductance {
        match self {
            Self::Fixed { coefficient } => GapConductance {
                gas: coefficient.max(0.0),
                radiation: 0.0,
                contact: 0.0,
            },

            Self::FuelRodFrapcon {
                radial_gap_width,
                scaling,
            } => {
                let t = surfaces.mean_temperature();
                let k = gas.conductivity(t);
                let jump =
                    temperature_jump_distance(k, t, gas_pressure, gas.accommodation_coefficient(t));
                let roughness = effective_roughness_distance(
                    surfaces.fuel_roughness,
                    surfaces.clad_roughness,
                    surfaces.interface_pressure,
                );
                let effective = (radial_gap_width.max(0.0) + roughness - ROUGHNESS_OFFSET).max(0.0);
                let denominator = (effective + JUMP_DISTANCE_MULTIPLIER * jump).max(SMALL);
                let raw_gas = (k / denominator).max(0.0);

                GapConductance {
                    gas: scaling.gas_factor * raw_gas + scaling.gas_offset,
                    radiation: scaling.radiation_factor * radiative_conductance(surfaces)
                        + scaling.radiation_offset,
                    contact: scaling.contact_factor * contact_conductance(surfaces)
                        + scaling.contact_offset,
                }
            }

            Self::TrisoSpherical {
                reference_radius,
                inner_radius,
                outer_radius,
                jump_distance,
            } => {
                let t = surfaces.mean_temperature();
                let k = gas.conductivity(t);
                let jump = match jump_distance {
                    TrisoJumpDistance::Frapcon => temperature_jump_distance(
                        k,
                        t,
                        gas_pressure,
                        gas.accommodation_coefficient(t),
                    ),
                    TrisoJumpDistance::Prescribed { inner, outer } => {
                        inner.max(0.0) + outer.max(0.0)
                    }
                };
                let shell = spherical_gap_conduction_length(
                    *reference_radius,
                    *inner_radius,
                    *outer_radius,
                );
                let roughness = effective_roughness_distance(
                    surfaces.fuel_roughness,
                    surfaces.clad_roughness,
                    surfaces.interface_pressure,
                );
                let denominator = (shell + roughness + jump).max(SMALL);

                GapConductance {
                    gas: (k / denominator).max(0.0),
                    radiation: radiative_conductance(surfaces),
                    contact: contact_conductance(surfaces),
                }
            }
        }
    }

    /// [`Self::evaluate`], but rejecting inputs it would have had to guard.
    ///
    /// # Errors
    ///
    /// - [`OffbeatError::Unphysical`] from [`GapSurfaces::validate`], or for a
    ///   negative gap width / radius, a non-positive gas pressure, or a
    ///   `Fixed` coefficient below zero.
    /// - [`OffbeatError::Unphysical`] if `outer_radius < inner_radius`.
    ///
    /// A negative *result* (only reachable through a negative
    /// [`GapConductanceScaling`] offset) is also reported, because a negative
    /// interface conductance is not a physically meaningful boundary condition
    /// and will make the enclosing energy solve behave strangely rather than
    /// fail.
    pub fn evaluate_checked(
        &self,
        gas: &GapGasMixture,
        gas_pressure: f64,
        surfaces: &GapSurfaces,
    ) -> Result<GapConductance> {
        surfaces.validate()?;
        if !(gas_pressure > 0.0) || !gas_pressure.is_finite() {
            return Err(OffbeatError::Unphysical {
                quantity: "gap gas pressure",
                value: gas_pressure,
                unit: "Pa",
                reason: "must be finite and strictly positive; it divides the \
                         temperature-jump distance",
            });
        }
        match self {
            Self::Fixed { coefficient } => {
                if !(*coefficient >= 0.0) || !coefficient.is_finite() {
                    return Err(OffbeatError::Unphysical {
                        quantity: "prescribed gap conductance",
                        value: *coefficient,
                        unit: "W/m^2/K",
                        reason: "must be finite and non-negative",
                    });
                }
            }
            Self::FuelRodFrapcon {
                radial_gap_width, ..
            } => {
                if !(*radial_gap_width >= 0.0) || !radial_gap_width.is_finite() {
                    return Err(OffbeatError::Unphysical {
                        quantity: "radial gap width",
                        value: *radial_gap_width,
                        unit: "m",
                        reason: "must be finite and non-negative (this is the RADIAL, \
                                 open-only width; a closed gap is width 0 plus a \
                                 non-zero interface pressure)",
                    });
                }
            }
            Self::TrisoSpherical {
                reference_radius,
                inner_radius,
                outer_radius,
                jump_distance,
            } => {
                for (quantity, value) in [
                    ("TRISO gap reference radius", *reference_radius),
                    ("TRISO gap inner radius", *inner_radius),
                    ("TRISO gap outer radius", *outer_radius),
                ] {
                    if !(value > 0.0) || !value.is_finite() {
                        return Err(OffbeatError::Unphysical {
                            quantity,
                            value,
                            unit: "m",
                            reason: "must be finite and strictly positive",
                        });
                    }
                }
                if outer_radius < inner_radius {
                    return Err(OffbeatError::Unphysical {
                        quantity: "TRISO gap outer radius",
                        value: *outer_radius,
                        unit: "m",
                        reason: "must not be smaller than the inner radius",
                    });
                }
                if let TrisoJumpDistance::Prescribed { inner, outer } = jump_distance {
                    for (quantity, value) in [
                        ("TRISO inner-surface jump distance", *inner),
                        ("TRISO outer-surface jump distance", *outer),
                    ] {
                        if !(value >= 0.0) || !value.is_finite() {
                            return Err(OffbeatError::Unphysical {
                                quantity,
                                value,
                                unit: "m",
                                reason: "a prescribed jump distance must be finite and \
                                         non-negative; upstream aborts if it is given on \
                                         only one side of the interface",
                            });
                        }
                    }
                }
            }
        }

        let h = self.evaluate(gas, gas_pressure, surfaces);
        for (quantity, value) in [
            ("gas gap conductance", h.gas),
            ("radiative gap conductance", h.radiation),
            ("contact gap conductance", h.contact),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(OffbeatError::Unphysical {
                    quantity,
                    value,
                    unit: "W/m^2/K",
                    reason: "must be finite and non-negative; check the \
                             GapConductanceScaling offsets",
                });
            }
        }
        Ok(h)
    }
}

/// Temperature-jump distance \[m\], **radial** — upstream's `d_jump`.
///
/// ```text
/// d_jump = 0.0137 · k · sqrt(T) / (p · a)
/// ```
///
/// # What it represents
///
/// Gas molecules leaving a solid surface do not carry the surface's full
/// temperature, so a discontinuity sits at each wall. Its thermal effect is the
/// same as adding this much extra gas thickness to the gap. It grows with gas
/// conductivity and temperature and *falls* with pressure — which is why a
/// depressurised rod has a much worse gap than the geometry alone suggests.
///
/// # Arguments
///
/// - `conductivity` — gas mixture conductivity \[W/m/K\] at the film temperature.
/// - `temperature` — film temperature \[K\], the mean of the two surfaces.
/// - `gas_pressure` — rod internal gas pressure \[Pa\].
/// - `accommodation` — **upstream's** accommodation coefficient from
///   [`GapGasMixture::accommodation_coefficient`]. Note that it is not
///   dimensionless (see that method's documented upstream defect); the constant
///   `0.0137` absorbs the scaling.
///
/// Returns `0.0` if any input is non-positive or non-finite, rather than an
/// infinity.
#[must_use]
pub fn temperature_jump_distance(
    conductivity: f64,
    temperature: f64,
    gas_pressure: f64,
    accommodation: f64,
) -> f64 {
    if !(conductivity > 0.0)
        || !(temperature > 0.0)
        || !(gas_pressure > 0.0)
        || !(accommodation > 0.0)
        || !conductivity.is_finite()
        || !temperature.is_finite()
        || !gas_pressure.is_finite()
        || !accommodation.is_finite()
    {
        return 0.0;
    }
    JUMP_DISTANCE_COEFFICIENT * conductivity * temperature.sqrt() / (gas_pressure * accommodation)
}

/// Effective roughness thickness \[m\], **radial** — upstream's `d_eff`.
///
/// ```text
/// d_eff = exp(−1.25e−3 · P_kgf) · (R_fuel + R_clad)
/// ```
///
/// where `P_kgf` is the interface pressure in kilogram-force per square
/// centimetre. Gas trapped in the asperity valleys adds to the conduction path;
/// pressing the surfaces together squashes the asperities and removes it, hence
/// the exponential decay with contact pressure.
///
/// # Arguments
///
/// - `fuel_roughness`, `clad_roughness` — per-surface arithmetic-mean roughness
///   \[m\], **radial**. They are summed, in contrast to the root-sum-square
///   combination the contact term uses; both are upstream's.
/// - `interface_pressure` — contact pressure \[Pa\], `>= 0`.
///
/// Negative or non-finite inputs are treated as zero.
#[must_use]
pub fn effective_roughness_distance(
    fuel_roughness: f64,
    clad_roughness: f64,
    interface_pressure: f64,
) -> f64 {
    let sum = fuel_roughness.max(0.0) + clad_roughness.max(0.0);
    if !sum.is_finite() || sum <= 0.0 {
        return 0.0;
    }
    let p_kgf = if interface_pressure.is_finite() && interface_pressure > 0.0 {
        interface_pressure / PRESSURE_TO_KGF_PER_CM2
    } else {
        0.0
    };
    (-ROUGHNESS_PRESSURE_COEFFICIENT * p_kgf).exp() * sum
}

/// Spherical-shell gas conduction length \[m\], **radial** — upstream's
/// `gap_sphere` in `trisoGapFvPatchScalarField::hGap()`.
///
/// ```text
/// L = max( r_ref² · (1/r_in − 1/r_out), 0 )
/// ```
///
/// This is the planar-equivalent thickness of a spherical shell: dividing the
/// gas conductivity by it gives a conductance referred to the surface of radius
/// `r_ref`. For a thin shell it tends to `r_out − r_in`, the planar gap width;
/// for a thick one it does not, which is the whole point of the spherical form
/// in a TRISO particle where the coating thicknesses are a large fraction of the
/// radius.
///
/// See [`GapConductanceModel::TrisoSpherical::reference_radius`] for why
/// `r_ref` is a separate argument and not simply `r_in`.
///
/// Returns `0.0` for non-positive or non-finite radii, or if `r_out < r_in`.
#[must_use]
pub fn spherical_gap_conduction_length(
    reference_radius: f64,
    inner_radius: f64,
    outer_radius: f64,
) -> f64 {
    if !(inner_radius > 0.0)
        || !(outer_radius > 0.0)
        || !reference_radius.is_finite()
        || !inner_radius.is_finite()
        || !outer_radius.is_finite()
    {
        return 0.0;
    }
    let r_in = inner_radius.min(outer_radius);
    let r_out = inner_radius.max(outer_radius);
    (reference_radius * reference_radius * (1.0 / r_in - 1.0 / r_out)).max(0.0)
}

/// Linearised gray-body radiative conductance \[W/m²K\] — upstream's `hRad`.
///
/// ```text
/// h_rad = σ (T₁ + T₂)(T₁² + T₂²) / (1/ε₁ + 1/ε₂ − 1)
/// ```
///
/// # Why this form
///
/// The net exchange between two gray surfaces is
/// `q'' = σ (T₁⁴ − T₂⁴) / (1/ε₁ + 1/ε₂ − 1)`. Factoring
/// `T₁⁴ − T₂⁴ = (T₁ + T₂)(T₁² + T₂²)(T₁ − T₂)` leaves exactly the expression
/// above multiplied by `(T₁ − T₂)`, so `h_rad` is an *exact* linearisation, not
/// an approximation — it can be used as a conductance in a linear solve without
/// introducing error, provided it is re-evaluated as the temperatures change.
///
/// # Assumption, stated because it is not obviously right here
///
/// The denominator `1/ε₁ + 1/ε₂ − 1` is the **infinite-parallel-plate** view
/// factor. For concentric cylinders the correct form is
/// `1/ε₁ + (A₁/A₂)(1/ε₂ − 1)`. Upstream uses the plate form for both the rod and
/// the TRISO geometry; since a fuel/cladding gap is a few tens of microns across
/// a ~4 mm radius, `A₁/A₂ ≈ 1` and the two agree closely there. For a TRISO
/// shell, where the area ratio departs from 1, this is a genuine approximation.
/// It is reproduced rather than corrected.
///
/// Emissivities are floored at [`SMALL`], matching upstream, so a zero
/// emissivity gives zero radiative transfer. Returns `0.0` for non-positive
/// temperatures.
#[must_use]
pub fn radiative_conductance(surfaces: &GapSurfaces) -> f64 {
    let t1 = surfaces.fuel_temperature;
    let t2 = surfaces.clad_temperature;
    if !(t1 > 0.0) || !(t2 > 0.0) || !t1.is_finite() || !t2.is_finite() {
        return 0.0;
    }
    let e1 = surfaces.fuel_emissivity.max(SMALL);
    let e2 = surfaces.clad_emissivity.max(SMALL);
    let resistance = 1.0 / e1 + 1.0 / e2 - 1.0;
    if !(resistance > 0.0) || !resistance.is_finite() {
        return 0.0;
    }
    STEFAN_BOLTZMANN * (t1 + t2) * (t1 * t1 + t2 * t2) / resistance
}

/// Solid-contact conductance \[W/m²K\] — upstream's `hContact`, the
/// Ross–Stoute-style correlation used by both gap patch fields.
///
/// ```text
/// P_rel  = P_interface / H_Meyer
/// k_m    = 2 k₁ k₂ / (k₁ + k₂)                        (harmonic mean)
/// R      = sqrt(R₁² + R₂²)                            (root-sum-square roughness)
/// R_f    = ½ (R₁ + R₂)                                (mean roughness)
/// R_mult = 333.3 · P_rel     if P_rel ≤ 0.0087, else 2.9
/// E      = exp(5.738 − 0.528 · ln(3.937e7 · R_f))
///
/// h_contact = 0.4166 · k_m · P_rel · R_mult / (R · E)   for P_rel > 0.003
///           = 0.00125 · k_m / (R · E)                   for 9e−6 < P_rel ≤ 0.003
///           = 0.4166 · k_m · sqrt(P_rel) / (R · E)      for P_rel ≤ 9e−6
/// ```
///
/// # Reading the branches
///
/// They are the three asperity-deformation regimes: elastic at very low relative
/// pressure (`sqrt(P_rel)`), a plateau, then plastic flattening where the real
/// contact area grows in proportion to load. **The three branches join
/// continuously**: at `P_rel = 9e−6` the elastic branch gives
/// `0.4166·3e−3 = 1.2498e−3 ≈ 0.00125`, and at `P_rel = 0.003` the plastic
/// branch gives `0.4166·0.003·0.9999 = 1.2497e−3 ≈ 0.00125`. That continuity is
/// asserted in the tests, and it is the reason the constants look arbitrary.
///
/// The `3.937e7` inside `E` converts metres to microinches
/// (`1 m = 3.937e7 µin`) — another sign of an imperial-unit fit.
///
/// # Behaviour at the limits
///
/// - **Zero interface pressure**: returns exactly `0.0`. An open gap conducts no
///   heat through contact, by definition.
/// - **Hard contact**: grows linearly in interface pressure, and dominates the
///   gas and radiation terms by an order of magnitude or more.
///
/// Returns `0.0` for non-finite or degenerate inputs (zero combined roughness,
/// zero conductivities) rather than an infinity.
#[must_use]
pub fn contact_conductance(surfaces: &GapSurfaces) -> f64 {
    let p = surfaces.interface_pressure;
    if !(p > 0.0) || !p.is_finite() {
        return 0.0;
    }
    if !(surfaces.meyer_hardness > 0.0) || !surfaces.meyer_hardness.is_finite() {
        return 0.0;
    }
    let k1 = surfaces.fuel_conductivity;
    let k2 = surfaces.clad_conductivity;
    if !(k1 > 0.0) || !(k2 > 0.0) || !k1.is_finite() || !k2.is_finite() {
        return 0.0;
    }
    let r1 = surfaces.fuel_roughness.max(0.0);
    let r2 = surfaces.clad_roughness.max(0.0);

    let p_rel = p / surfaces.meyer_hardness;
    let k_m = 2.0 * k1 * k2 / (k1 + k2);
    let r_rss = (r1 * r1 + r2 * r2).sqrt().max(SMALL);
    let r_mean = (0.5 * (r1 + r2)).max(SMALL);

    let r_mult = if p_rel <= 0.0087 { 333.3 * p_rel } else { 2.9 };
    let e = (5.738 - 0.528 * (3.937e7 * r_mean).ln()).exp();
    if !(e > 0.0) || !e.is_finite() {
        return 0.0;
    }

    let h = if p_rel > 0.003 {
        0.4166 * k_m * p_rel * r_mult / r_rss / e
    } else if p_rel > 9.0e-6 {
        0.00125 * k_m / r_rss / e
    } else {
        0.4166 * k_m * p_rel.sqrt() / r_rss / e
    };

    if h.is_finite() && h > 0.0 {
        h
    } else {
        0.0
    }
}

/// Effective interface conductance \[W/m²K\] of three resistances in series —
/// upstream's `alphaEff` in `resistiveGapFvPatchScalarField::weights()`.
///
/// ```text
/// 1/h_eff = 1/h_fuel_wall + 1/h_clad_wall + 1/h_gap
/// ```
///
/// The gap resistance sits between the two half-cell wall resistances, so the
/// three add as resistances (reciprocals), **not** as conductances. This is the
/// counterpart to the three gap *paths*, which are parallel and do add directly
/// ([`GapConductance::total`]); confusing the two is the classic error here.
///
/// Use [`wall_conductance`] to build the two wall terms from a cell
/// conductivity and its wall distance.
///
/// A non-positive term is treated as an infinite resistance, so the result is
/// `0.0` if any of the three is zero.
#[must_use]
pub fn series_conductance(fuel_wall: f64, clad_wall: f64, gap: f64) -> f64 {
    let mut resistance = 0.0;
    for h in [fuel_wall, clad_wall, gap] {
        if !(h > 0.0) || !h.is_finite() {
            return 0.0;
        }
        resistance += 1.0 / h;
    }
    if resistance > 0.0 {
        1.0 / resistance
    } else {
        0.0
    }
}

/// Half-cell wall conductance \[W/m²K\] — upstream's `kappa()/deltas` in
/// `resistiveGapFvPatchScalarField::weights()`.
///
/// `h = k / δ`, where `δ` is the normal distance from the cell centre to the
/// boundary face (`patch.nf() & patch.delta()` upstream). Returns `0.0` for a
/// non-positive distance or conductivity.
///
/// # Deferred
///
/// `δ` itself comes from the mesh; this function does the arithmetic only.
#[must_use]
pub fn wall_conductance(conductivity: f64, wall_distance: f64) -> f64 {
    if !(conductivity > 0.0) || !(wall_distance > 0.0) {
        return 0.0;
    }
    conductivity / wall_distance
}

/// Arithmetic mean of the owner and neighbour values on an interface —
/// upstream's `0.5*(hGap() + interpolate(nbr.hGap()))` in `updateCoeffs()`.
///
/// Both gap patch fields evaluate the whole model twice, once from each side
/// (the two sides see different temperatures, different solid conductivities and
/// — for TRISO — different reference radii), then average. This function is that
/// average.
///
/// # Deferred
///
/// The AMI interpolation that brings the neighbour's value onto this patch's
/// faces. This function assumes the two values already refer to the same face.
#[must_use]
pub fn average_across_interface(owner: f64, neighbour: f64) -> f64 {
    0.5 * (owner + neighbour)
}

/// Explicit under-relaxation of a gap conductance between outer iterations —
/// upstream's `hGap_ = relax_*(...) + (1 - relax_)*hGap_`.
///
/// Gap conductance and temperature are strongly and non-linearly coupled: a
/// hotter pellet expands, narrows the gap, raises the conductance, and cools
/// again. Solved without relaxation that loop oscillates. `factor = 1` is no
/// relaxation (upstream's default); smaller values damp the loop.
///
/// # Arguments
///
/// - `new`, `previous` — conductances \[W/m²K\] from this and the last outer
///   iteration.
/// - `factor` — relaxation factor \[-\], clamped to `[0, 1]`.
#[must_use]
pub fn under_relax(new: f64, previous: f64, factor: f64) -> f64 {
    let f = factor.clamp(0.0, 1.0);
    f * new + (1.0 - f) * previous
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gap::gas::GapGasSpecies;

    fn helium() -> GapGasMixture {
        GapGasMixture::pure(GapGasSpecies::Helium, 1.0e-5).unwrap()
    }

    /// A helium/xenon mixture at a prescribed xenon **mole** fraction.
    ///
    /// Present because the mole and mass bases differ by the 32.8x molar-mass
    /// ratio, and the test below exists precisely to pin that difference.
    fn helium_xenon_by_mole(x_xe: f64) -> GapGasMixture {
        let mut y = [0.0; 6];
        y[GapGasSpecies::Helium.index()] = (1.0 - x_xe) * GapGasSpecies::Helium.molar_mass();
        y[GapGasSpecies::Xenon.index()] = x_xe * GapGasSpecies::Xenon.molar_mass();
        GapGasMixture::from_mass_fractions(y, 1.0e-4).unwrap()
    }

    fn xenon_rich() -> GapGasMixture {
        // 20% helium / 80% xenon by mass — a heavily-irradiated rod.
        let mut y = [0.0; 6];
        y[GapGasSpecies::Helium.index()] = 0.2;
        y[GapGasSpecies::Xenon.index()] = 0.8;
        GapGasMixture::from_mass_fractions(y, 1.0e-4).unwrap()
    }

    fn rod(width: f64) -> GapConductanceModel {
        GapConductanceModel::FuelRodFrapcon {
            radial_gap_width: width,
            scaling: GapConductanceScaling::default(),
        }
    }

    /// Reference-checked against upstream's own literal constants.
    ///
    /// **Methodology.** Every magic number in `fuelRodGapFvPatchScalarField.C`
    /// and `trisoGapFvPatchScalarField.C` is asserted here against the value in
    /// upstream's source (tolerance: bitwise equality). A silent typo in one of
    /// these would shift every gap temperature in a run while leaving all the
    /// qualitative behaviour intact, so it would not be caught by any of the
    /// self-consistency checks below.
    ///
    /// **Result** (2026-07-29): all constants match upstream verbatim.
    #[test]
    fn model_constants_match_upstream_literals() {
        assert_eq!(ROUGHNESS_OFFSET, 1.397e-6);
        assert_eq!(JUMP_DISTANCE_MULTIPLIER, 1.8);
        assert_eq!(JUMP_DISTANCE_COEFFICIENT, 0.0137);
        assert_eq!(PRESSURE_TO_KGF_PER_CM2, 1.0e4 * 9.8);
        assert_eq!(ROUGHNESS_PRESSURE_COEFFICIENT, 1.25e-3);
        assert_eq!(MEYER_HARDNESS_ZIRCALOY, 680.0e6);
        // The imperial tell: the roughness offset is 55 microinches.
        assert!((ROUGHNESS_OFFSET - 55.0e-6 * 0.0254).abs() < 1e-12);
    }

    /// Self-consistency check — conductance rises monotonically as the gap
    /// narrows.
    ///
    /// **Methodology.** Sweep the radial gap width from 2.0e-4 m down to 0 in
    /// 200 steps at fixed surface state and gas composition, and require the
    /// total conductance to increase strictly at every step. Pass criterion:
    /// strict monotonicity. This is the single most important qualitative
    /// property of the model — the closure feedback loop depends on it — and it
    /// is **an ordering property, not a validation.**
    ///
    /// **Result** (2026-07-29, measured, pure helium at 2.25 MPa, surfaces at
    /// 900 K / 600 K): strictly increasing over all 200 steps; the total rose
    /// from 1.4989e3 W/m²K at a 2.0e-4 m radial gap to 3.6541e5 W/m²K at a
    /// closed (zero-width, zero-pressure) gap — a factor of 244.
    #[test]
    fn conductance_rises_monotonically_as_the_gap_narrows() {
        let gas = helium();
        let surfaces = GapSurfaces::lwr_open_gap(900.0, 600.0);
        let steps = 200;
        let widest = 2.0e-4;

        let mut previous = f64::NEG_INFINITY;
        let mut first = 0.0;
        let mut last = 0.0;
        for i in 0..=steps {
            let width = widest * (1.0 - i as f64 / steps as f64);
            let h = rod(width).evaluate(&gas, 2.25e6, &surfaces).total();
            assert!(
                h > previous,
                "conductance fell at width {width}: {h} <= {previous}"
            );
            if i == 0 {
                first = h;
            }
            last = h;
            previous = h;
        }
        assert!((first - 1.498_916e3).abs() < 1.0, "widest-gap h = {first}");
        assert!((last - 3.654_123e5).abs() < 1.0e2, "closed-gap h = {last}");
    }

    /// Self-consistency check — the open-gap limit is gas conduction plus
    /// radiation, with no contact term at all.
    ///
    /// **Methodology.** With `interface_pressure = 0` the contact correlation
    /// must return exactly `0.0` (not merely a small number), so the total is
    /// `gas + radiation`. Tolerance: exact equality for the contact term, 0 for
    /// the sum residual.
    ///
    /// **Result** (2026-07-29, measured, 8.5e-5 m radial gap, pure helium,
    /// 2.25 MPa, 900 K / 600 K): `gas = 3.3450e3`, `radiation = 6.9763e1`,
    /// `contact = 0` W/m²K; radiation is 2.04% of the total, which is the
    /// expected order for an LWR gap at these temperatures.
    #[test]
    fn open_gap_reduces_to_gas_plus_radiation() {
        let gas = helium();
        let surfaces = GapSurfaces::lwr_open_gap(900.0, 600.0);
        let h = rod(8.5e-5).evaluate(&gas, 2.25e6, &surfaces);

        assert_eq!(h.contact, 0.0);
        assert_eq!(h.contact_fraction(), 0.0);
        assert!((h.total() - (h.gas + h.radiation)).abs() < 1e-12 * h.total());
        assert!((h.gas - 3.345_010e3).abs() < 1.0, "h_gas = {}", h.gas);
        assert!(
            (h.radiation - 6.976_314e1).abs() < 0.05,
            "h_rad = {}",
            h.radiation
        );
        assert!(h.radiation / h.total() < 0.05);
    }

    /// Self-consistency check — the closed-gap limit, and a **finding that
    /// contradicts the usual expectation**.
    ///
    /// **Methodology.** Set the radial gap width to zero and sweep the interface
    /// pressure from 10 MPa to 500 MPa (the last being close to the 680 MPa
    /// Zircaloy Meyer hardness, i.e. as hard as this correlation can be pushed).
    /// Record the gas/contact split. The hypothesis under test was the textbook
    /// one — that a hard-closed gap is *dominated by contact conduction*.
    ///
    /// **Result** (2026-07-29, measured, pure helium at 2.25 MPa, 900 K / 600 K,
    /// as-fabricated roughnesses 1.0 and 0.5 µm): **the hypothesis is false for
    /// upstream's model.** The gas term saturates at `4.2048e5 W/m²K` and the
    /// contact term never catches it:
    ///
    /// | `P_interface` \[MPa\] | `h_gas` \[W/m²K\] | `h_contact` \[W/m²K\] | contact fraction |
    /// |---|---|---|---|
    /// | 10 | 4.2048e5 | 1.5290e3 | 0.0036 |
    /// | 50 | 4.2048e5 | 7.6452e3 | 0.0179 |
    /// | 100 | 4.2048e5 | 1.5290e4 | 0.0351 |
    /// | 200 | 4.2048e5 | 3.0581e4 | 0.0678 |
    /// | 500 | 4.2048e5 | 7.6452e4 | 0.1538 |
    ///
    /// **Why.** Once `d_gap + d_eff` falls below [`ROUGHNESS_OFFSET`], the
    /// effective gas thickness clips to zero and the gas term becomes
    /// `k / (1.8 · d_jump)` — limited only by the temperature-jump distance,
    /// which for helium at a few MPa is a few hundred nanometres. That saturated
    /// value, 4.2e5 W/m²K here, is far above anything the Ross–Stoute contact
    /// correlation produces at reachable pressures. Making the surfaces rougher
    /// does not change the conclusion: at 5 µm roughness on both sides and
    /// 50 MPa the split is `h_gas = 6.2788e4`, `h_contact = 3.2914e3`, contact
    /// fraction 0.0498.
    ///
    /// **Interpretation.** This is a property of upstream's formulation, not a
    /// port error — the algebra is reproduced verbatim and pinned by
    /// `model_constants_match_upstream_literals`.
    /// Two consequences a user must know: (a) the saturated closed-gap
    /// conductance is set by the *gas* and its pressure, so a depressurised rod
    /// behaves very differently from a pressurised one even in hard contact; and
    /// (b) the widely-quoted closed-gap figure of order 1e4 W/m²K is **not** what
    /// this model gives at exactly zero width. Whether upstream intends the
    /// zero-width limit to be reached at all — in a finite-volume solve there is
    /// normally a small residual numerical gap — has not been established here,
    /// and is recorded as an open question rather than resolved.
    #[test]
    fn closed_gap_conductance_saturates_at_the_jump_distance_limit() {
        let gas = helium();
        let mut surfaces = GapSurfaces::lwr_open_gap(900.0, 600.0);

        let expected = [
            (10.0e6, 1.529_05e3, 0.003_62),
            (50.0e6, 7.645_24e3, 0.017_85),
            (100.0e6, 1.529_05e4, 0.035_08),
            (200.0e6, 3.058_10e4, 0.067_79),
            (500.0e6, 7.645_24e4, 0.153_83),
        ];
        for (pressure, h_contact, fraction) in expected {
            surfaces.interface_pressure = pressure;
            let h = rod(0.0).evaluate(&gas, 2.25e6, &surfaces);

            // The gas term is saturated: identical at every pressure.
            assert!(
                (h.gas - 4.204_84e5).abs() < 1.0e2,
                "P={pressure}: h_gas = {}",
                h.gas
            );
            assert!(
                (h.contact - h_contact).abs() < 1e-3 * h_contact,
                "P={pressure}: h_contact = {}",
                h.contact
            );
            assert!(
                (h.contact_fraction() - fraction).abs() < 1e-3,
                "P={pressure}: contact fraction = {}",
                h.contact_fraction()
            );
            // The finding: contact never dominates.
            assert!(h.contact < h.gas, "P={pressure}: contact overtook gas");
        }

        // The saturated value is exactly k / (1.8 * d_jump).
        surfaces.interface_pressure = 50.0e6;
        let t = surfaces.mean_temperature();
        let k = gas.conductivity(t);
        let jump = temperature_jump_distance(k, t, 2.25e6, gas.accommodation_coefficient(t));
        let saturated = k / (JUMP_DISTANCE_MULTIPLIER * jump);
        let h = rod(0.0).evaluate(&gas, 2.25e6, &surfaces);
        assert!(
            (h.gas - saturated).abs() < 1e-9 * saturated,
            "{} vs {saturated}",
            h.gas
        );

        // Rougher surfaces do not change the conclusion.
        let mut rough = GapSurfaces::lwr_open_gap(900.0, 600.0);
        rough.fuel_roughness = 5.0e-6;
        rough.clad_roughness = 5.0e-6;
        rough.interface_pressure = 50.0e6;
        let hr = rod(0.0).evaluate(&gas, 2.25e6, &rough);
        assert!(
            (hr.gas - 6.278_77e4).abs() < 1.0e2,
            "rough h_gas = {}",
            hr.gas
        );
        assert!(
            (hr.contact - 3.291_43e3).abs() < 1.0,
            "rough h_contact = {}",
            hr.contact
        );
        assert!(hr.contact < hr.gas);
    }

    /// Self-consistency check — contact conductance is **non-decreasing** in
    /// interface pressure, with a genuine plateau, and exactly zero at zero
    /// pressure.
    ///
    /// **Methodology.** Sweep the interface pressure from 0 to 200 MPa in 500
    /// steps. Pass criterion: exactly zero at zero pressure, and never
    /// decreasing thereafter. **Strict** monotonicity was the original
    /// hypothesis and it is false — see below.
    ///
    /// **Result** (2026-07-29, measured): zero at zero pressure; non-decreasing
    /// over all 500 steps, reaching 3.0581e4 W/m²K at 200 MPa. The sequence is
    /// **flat** for steps 1 through 5 (0.4–2.0 MPa), because that range is
    /// `9e−6 < P_rel ≤ 0.003`, where the correlation's middle branch is the
    /// pressure-independent constant `0.00125·k_m/(R·E)`. This is upstream's
    /// design, not a defect: the three branches are `sqrt(P_rel)` / constant /
    /// linear-in-`P_rel`, and the constant one is the plateau documented on
    /// [`contact_conductance`]. Recording it here so nobody "fixes" the test by
    /// smoothing the model.
    ///
    /// A strict increase is still asserted **across** the plateau — from the
    /// first non-zero pressure to the last — so a model that had gone flat
    /// everywhere would still fail.
    #[test]
    fn contact_conductance_is_non_decreasing_in_pressure_with_a_plateau() {
        let mut surfaces = GapSurfaces::lwr_open_gap(900.0, 600.0);
        surfaces.interface_pressure = 0.0;
        assert_eq!(contact_conductance(&surfaces), 0.0);

        let mut previous = 0.0;
        let mut first = 0.0;
        let mut last = 0.0;
        let mut plateau_steps = 0;
        for i in 1..=500 {
            surfaces.interface_pressure = 200.0e6 * i as f64 / 500.0;
            let h = contact_conductance(&surfaces);
            assert!(h >= previous, "contact conductance fell at step {i}");
            if i > 1 && h == previous {
                plateau_steps += 1;
            }
            if i == 1 {
                first = h;
            }
            previous = h;
            last = h;
        }
        assert!(last > first, "no net increase across the sweep");
        assert!(
            (last - 3.058_097e4).abs() < 1.0,
            "h_contact(200 MPa) = {last}"
        );

        // The plateau is real and lives in the middle branch.
        assert_eq!(
            plateau_steps, 4,
            "plateau spanned {plateau_steps} extra steps"
        );
        for i in 1..=5 {
            surfaces.interface_pressure = 200.0e6 * i as f64 / 500.0;
            let p_rel = surfaces.interface_pressure / surfaces.meyer_hardness;
            assert!(
                p_rel > 9.0e-6 && p_rel <= 0.003,
                "step {i}: P_rel = {p_rel}"
            );
            assert!((contact_conductance(&surfaces) - first).abs() < 1e-9 * first);
        }
    }

    /// Self-consistency check — the three contact branches join continuously.
    ///
    /// **Methodology.** The correlation switches formula at `P_rel = 9e−6` and
    /// `P_rel = 0.003`. Evaluate a hair either side of each breakpoint and
    /// require the relative jump to be below 1e-3. This tests that the port
    /// picked up upstream's branch conditions and constants exactly — a wrong
    /// constant would open a visible step.
    ///
    /// **Result** (2026-07-29, measured): relative jump 1.6003e-4 at
    /// `P_rel = 9e−6` and 2.5998e-4 at `P_rel = 0.003`. Both are the genuine
    /// residual of upstream's rounded constants (`0.4166`, `333.3`, `0.00125`),
    /// not a port error.
    #[test]
    fn contact_branches_join_continuously() {
        let mut surfaces = GapSurfaces::lwr_open_gap(900.0, 600.0);
        let h_meyer = surfaces.meyer_hardness;

        for p_rel in [9.0e-6, 0.003] {
            let eps = p_rel * 1.0e-9;
            surfaces.interface_pressure = (p_rel - eps) * h_meyer;
            let below = contact_conductance(&surfaces);
            surfaces.interface_pressure = (p_rel + eps) * h_meyer;
            let above = contact_conductance(&surfaces);
            let jump = (above - below).abs() / below;
            assert!(jump < 1.0e-3, "jump at P_rel={p_rel} is {jump}");
        }
    }

    /// Self-consistency check — xenon dilution degrades the gas term
    /// monotonically, and **by mole fraction, not by mass fraction**.
    ///
    /// **Methodology.** Same 8.5e-5 m radial gap, same surfaces, same gas
    /// pressure; only the He/Xe composition varies, specified by **xenon mole
    /// fraction**. Pass criterion: `h_gas` strictly decreasing in xenon content,
    /// with the pure-helium and pure-xenon values as the two endpoints. **This is
    /// an ordering property, not a validation against measured mixture data.**
    ///
    /// **Result** (2026-07-29, measured, film temperature 750 K):
    ///
    /// | `x_Xe` \[-\] | `k_mix` \[W/m/K\] | `h_gas` \[W/m²K\] |
    /// |---|---|---|
    /// | 0.00 | 0.286953 | 3.3450e3 |
    /// | 0.01 | 0.276402 | 3.2228e3 |
    /// | 0.05 | 0.239593 | 2.7960e3 |
    /// | 0.10 | 0.202986 | 2.3709e3 |
    /// | 0.25 | 0.130487 | 1.5267e3 |
    /// | 0.50 | 0.067516 | 7.9119e2 |
    /// | 0.90 | 0.020009 | 2.3482e2 |
    /// | 1.00 | 0.012615 | 1.4810e2 |
    ///
    /// **The mole/mass distinction matters and was nearly got wrong here.** A
    /// mixture that is 80% *xenon by mass* is only 10.9% xenon *by mole*,
    /// because xenon is 32.8x heavier per mole than helium — and it costs only
    /// 31% of the gas conductivity, not the factor of nine an 80% figure
    /// suggests. Fission-gas release is reported in moles; a composition
    /// converted through the wrong basis will under- or over-predict gap
    /// degradation by roughly an order of magnitude.
    #[test]
    fn xenon_dilution_degrades_the_gas_term_by_mole_fraction() {
        let surfaces = GapSurfaces::lwr_open_gap(900.0, 600.0);
        let model = rod(8.5e-5);

        let expected = [
            (0.00, 0.286_953, 3.345_010e3),
            (0.01, 0.276_402, 3.222_805e3),
            (0.05, 0.239_593, 2.796_016e3),
            (0.10, 0.202_986, 2.370_854e3),
            (0.25, 0.130_487, 1.526_708e3),
            (0.50, 0.067_516, 7.911_910e2),
            (0.90, 0.020_009, 2.348_224e2),
            (1.00, 0.012_615, 1.481_029e2),
        ];

        let mut previous = f64::INFINITY;
        for (x_xe, k_mix, h_gas) in expected {
            let mix = helium_xenon_by_mole(x_xe);
            assert!(
                (mix.mole_fraction(GapGasSpecies::Xenon) - x_xe).abs() < 1e-12,
                "composition helper is wrong at x_Xe = {x_xe}"
            );
            let k = mix.conductivity(750.0);
            assert!((k - k_mix).abs() < 1e-5, "x_Xe={x_xe}: k_mix = {k}");

            let h = model.evaluate(&mix, 2.25e6, &surfaces).gas;
            assert!((h - h_gas).abs() < 1.0, "x_Xe={x_xe}: h_gas = {h}");
            assert!(h < previous, "h_gas rose at x_Xe = {x_xe}");
            previous = h;
        }

        // Pure helium conducts 22.6x better than pure xenon at this gap.
        let ratio: f64 = 3.345_010e3 / 1.481_029e2;
        assert!((ratio - 22.585).abs() < 0.01, "ratio = {ratio}");

        // The mole/mass trap, pinned: 80% xenon BY MASS is 10.9% by mole.
        let by_mass = xenon_rich();
        let x_xe = by_mass.mole_fraction(GapGasSpecies::Xenon);
        assert!((x_xe - 0.108_685).abs() < 1e-5, "x_Xe = {x_xe}");
        let h = model.evaluate(&by_mass, 2.25e6, &surfaces).gas;
        assert!((h - 2.306_613e3).abs() < 1.0, "h_gas(80% Xe by mass) = {h}");
    }

    /// Self-consistency check — the radiative term is the exact linearisation of
    /// the fourth-power law.
    ///
    /// **Methodology.** For arbitrary surface temperatures,
    /// `h_rad · (T₁ − T₂)` must equal `σ (T₁⁴ − T₂⁴) / (1/ε₁ + 1/ε₂ − 1)` to
    /// within floating-point rounding. Tested at four temperature pairs;
    /// tolerance 1e-12 relative. This is an algebraic identity, so any failure
    /// is a transcription error, not a modelling disagreement.
    ///
    /// **Result** (2026-07-29): all four pairs agreed to better than 1e-15
    /// relative.
    #[test]
    fn radiative_conductance_is_the_exact_fourth_power_linearisation() {
        for (t1, t2) in [
            (900.0, 600.0),
            (1500.0, 700.0),
            (400.0, 390.0),
            (2000.0, 300.0),
        ] {
            let s = GapSurfaces::lwr_open_gap(t1, t2);
            let h = radiative_conductance(&s);
            let resistance = 1.0 / s.fuel_emissivity + 1.0 / s.clad_emissivity - 1.0;
            let direct = STEFAN_BOLTZMANN * (t1.powi(4) - t2.powi(4)) / resistance;
            let linearised = h * (t1 - t2);
            assert!(
                (linearised - direct).abs() < 1e-12 * direct.abs(),
                "T=({t1},{t2}): {linearised} vs {direct}"
            );
        }
    }

    /// Self-consistency check — radiation scales as the difference of fourth
    /// powers.
    ///
    /// **Methodology.** Doubling both surface temperatures must multiply the
    /// radiative *flux* by 16 (`T⁴` scaling) and the *conductance* by 8 (one
    /// factor of temperature is absorbed by the linearisation). Tolerance 1e-12
    /// relative.
    ///
    /// **Result** (2026-07-29): flux ratio 16.0 and conductance ratio 8.0, both
    /// to within 1e-15 relative.
    #[test]
    fn radiation_scales_as_the_fourth_power_of_temperature() {
        let cold = GapSurfaces::lwr_open_gap(600.0, 400.0);
        let hot = GapSurfaces::lwr_open_gap(1200.0, 800.0);

        let h_cold = radiative_conductance(&cold);
        let h_hot = radiative_conductance(&hot);
        assert!(
            (h_hot / h_cold - 8.0).abs() < 1e-12,
            "ratio {}",
            h_hot / h_cold
        );

        let q_cold = h_cold * (600.0 - 400.0);
        let q_hot = h_hot * (1200.0 - 800.0);
        assert!(
            (q_hot / q_cold - 16.0).abs() < 1e-12,
            "ratio {}",
            q_hot / q_cold
        );
    }

    /// Self-consistency check — the temperature-jump distance falls with
    /// pressure and vanishes on degenerate input.
    ///
    /// **Methodology.** `d_jump ∝ 1/p`, so halving the pressure must double it
    /// exactly (tolerance 1e-12 relative). Non-positive inputs must give 0
    /// rather than an infinity.
    ///
    /// **Result** (2026-07-29, measured): for pure helium at a 750 K film
    /// temperature and 2.25 MPa, `d_jump = 3.7913e-7` m. Multiplied by
    /// [`JUMP_DISTANCE_MULTIPLIER`] that is 6.82e-7 m — a quarter of the
    /// as-fabricated roughness sum and about 0.8% of a fresh 8.5e-5 m radial
    /// gap, but the *only* thing left in the denominator once the gap closes
    /// (see
    /// `closed_gap_conductance_saturates_at_the_jump_distance_limit`),
    /// which is why it cannot be neglected. Halving the pressure doubled it
    /// exactly.
    #[test]
    fn temperature_jump_distance_scales_inversely_with_pressure() {
        let gas = helium();
        let t = 750.0;
        let k = gas.conductivity(t);
        let a = gas.accommodation_coefficient(t);

        let d1 = temperature_jump_distance(k, t, 2.25e6, a);
        let d2 = temperature_jump_distance(k, t, 1.125e6, a);
        assert!((d2 / d1 - 2.0).abs() < 1e-12);
        assert!((d1 - 3.791_310e-7).abs() < 1e-12, "d_jump = {d1}");

        assert_eq!(temperature_jump_distance(k, t, 0.0, a), 0.0);
        assert_eq!(temperature_jump_distance(k, t, 2.25e6, 0.0), 0.0);
        assert_eq!(temperature_jump_distance(0.0, t, 2.25e6, a), 0.0);
    }

    /// Self-consistency check — pressing the surfaces together squeezes the
    /// roughness gas out.
    ///
    /// **Methodology.** `d_eff = exp(−1.25e−3 · P_kgf)(R₁ + R₂)` must equal
    /// `R₁ + R₂` at zero pressure and decay monotonically thereafter.
    ///
    /// **Result** (2026-07-29, measured): 1.5e-6 m at zero pressure, falling to
    /// 2.5489e-9 m at 500 MPa — a factor of 589. The decay is fast because
    /// 500 MPa is 5102 kgf/cm², and `exp(−1.25e−3 · 5102) = 1.70e−3`.
    #[test]
    fn roughness_distance_decays_with_contact_pressure() {
        let sum = 1.0e-6 + 0.5e-6;
        assert!((effective_roughness_distance(1.0e-6, 0.5e-6, 0.0) - sum).abs() < 1e-18);

        let mut previous = sum;
        let mut last = 0.0;
        for i in 1..=100 {
            let p = 500.0e6 * i as f64 / 100.0;
            let d = effective_roughness_distance(1.0e-6, 0.5e-6, p);
            assert!(d < previous, "roughness distance rose at {p} Pa");
            previous = d;
            last = d;
        }
        assert!(
            (last - 2.548_919e-9).abs() < 1e-12,
            "d_eff(500 MPa) = {last}"
        );
    }

    /// Self-consistency check — the spherical conduction length tends to the
    /// planar gap width for a thin shell.
    ///
    /// **Methodology.** For `r_ref = r_in` and a shell thickness `t << r_in`,
    /// `r_in²(1/r_in − 1/(r_in+t)) = r_in t/(r_in+t) → t`. Checked at
    /// `r_in = 1.0e-4 m` (a TRISO kernel scale) for `t/r_in` from 1e-1 down to
    /// 1e-4; pass criterion: the relative error falls below `t/r_in`.
    ///
    /// **Result** (2026-07-29, measured): relative error 9.09e-2, 9.90e-3,
    /// 9.99e-4 and 1.00e-4 for `t/r_in` of 1e-1, 1e-2, 1e-3 and 1e-4 — first
    /// order in the thickness ratio, exactly as the algebra requires.
    #[test]
    fn spherical_conduction_length_tends_to_the_planar_gap() {
        let r_in = 1.0e-4;
        for ratio in [1.0e-1, 1.0e-2, 1.0e-3, 1.0e-4] {
            let t = r_in * ratio;
            let l = spherical_gap_conduction_length(r_in, r_in, r_in + t);
            let error = (l - t).abs() / t;
            assert!(error <= ratio, "ratio {ratio}: relative error {error}");
        }
        // Degenerate inputs are guarded rather than producing infinities.
        assert_eq!(spherical_gap_conduction_length(1.0e-4, 0.0, 1.0e-4), 0.0);
        assert_eq!(spherical_gap_conduction_length(1.0e-4, 1.0e-4, 1.0e-4), 0.0);
    }

    /// Self-consistency check — the TRISO variant reproduces the fuel-rod gas
    /// term in the thin-shell, no-offset limit.
    ///
    /// **Methodology.** The two models differ in three documented ways: the
    /// spherical vs planar length, the missing [`ROUGHNESS_OFFSET`], and the
    /// missing [`JUMP_DISTANCE_MULTIPLIER`]. Construct a TRISO case with a thin
    /// shell and a prescribed jump distance of zero, and a rod case with the
    /// same width; then the *only* remaining difference is the roughness offset,
    /// so the rod's effective thickness is smaller by exactly
    /// [`ROUGHNESS_OFFSET`] and its `h_gas` correspondingly larger. Pass
    /// criterion: the two effective thicknesses, back-computed as `k/h_gas`,
    /// differ by `ROUGHNESS_OFFSET` to within 1e-12 m.
    ///
    /// **Result** (2026-07-29): the back-computed difference was
    /// 1.397e-6 m, matching [`ROUGHNESS_OFFSET`] to 1e-21 m.
    #[test]
    fn triso_and_rod_gas_terms_differ_only_by_the_documented_offsets() {
        let gas = helium();
        let surfaces = GapSurfaces::lwr_open_gap(900.0, 600.0);
        let r_in = 1.0e-3;
        let width = 1.0e-6 * 5.0;

        let triso = GapConductanceModel::TrisoSpherical {
            reference_radius: r_in,
            inner_radius: r_in,
            outer_radius: r_in + width,
            jump_distance: TrisoJumpDistance::Prescribed {
                inner: 0.0,
                outer: 0.0,
            },
        };
        let shell = spherical_gap_conduction_length(r_in, r_in, r_in + width);
        let rod_model = GapConductanceModel::FuelRodFrapcon {
            radial_gap_width: shell,
            scaling: GapConductanceScaling {
                // Cancel the jump term so only the offset differs.
                ..GapConductanceScaling::default()
            },
        };

        let t = surfaces.mean_temperature();
        let k = gas.conductivity(t);
        // Rod with an infinite pressure has zero jump distance; use a huge
        // pressure to drive the jump term to (numerically) zero.
        let huge_p = 1.0e30;
        let h_triso = triso.evaluate(&gas, huge_p, &surfaces).gas;
        let h_rod = rod_model.evaluate(&gas, huge_p, &surfaces).gas;

        let d_triso = k / h_triso;
        let d_rod = k / h_rod;
        assert!(
            (d_triso - d_rod - ROUGHNESS_OFFSET).abs() < 1e-12,
            "d_triso {d_triso} - d_rod {d_rod} = {}",
            d_triso - d_rod
        );
    }

    /// Self-consistency check — resistances in series, not in parallel.
    ///
    /// **Methodology.** Three equal conductances `h` in series must give `h/3`,
    /// exactly. A zero term must give zero (an infinite resistance blocks the
    /// path entirely). Contrast with [`GapConductance::total`], where the three
    /// *parallel* gap paths add directly.
    #[test]
    fn series_conductance_adds_reciprocals() {
        assert!((series_conductance(3000.0, 3000.0, 3000.0) - 1000.0).abs() < 1e-9);
        assert_eq!(series_conductance(3000.0, 0.0, 3000.0), 0.0);
        // Parallel, for contrast.
        let parallel = GapConductance {
            gas: 3000.0,
            radiation: 3000.0,
            contact: 3000.0,
        };
        assert_eq!(parallel.total(), 9000.0);
    }

    /// Self-consistency check — the scaling parameters do what upstream's do.
    #[test]
    fn scaling_parameters_apply_factor_then_offset() {
        let gas = helium();
        let surfaces = GapSurfaces::lwr_open_gap(900.0, 600.0);
        let plain = rod(8.5e-5).evaluate(&gas, 2.25e6, &surfaces);

        let scaled = GapConductanceModel::FuelRodFrapcon {
            radial_gap_width: 8.5e-5,
            scaling: GapConductanceScaling {
                gas_factor: 2.0,
                gas_offset: 100.0,
                radiation_factor: 0.0,
                radiation_offset: 0.0,
                contact_factor: 1.0,
                contact_offset: 7.0,
            },
        }
        .evaluate(&gas, 2.25e6, &surfaces);

        assert!((scaled.gas - (2.0 * plain.gas + 100.0)).abs() < 1e-9);
        assert_eq!(scaled.radiation, 0.0);
        assert!((scaled.contact - 7.0).abs() < 1e-12);
    }

    /// Self-consistency check — under-relaxation blends and clamps.
    #[test]
    fn under_relaxation_blends_between_iterations() {
        assert!((under_relax(2000.0, 1000.0, 1.0) - 2000.0).abs() < 1e-12);
        assert!((under_relax(2000.0, 1000.0, 0.0) - 1000.0).abs() < 1e-12);
        assert!((under_relax(2000.0, 1000.0, 0.5) - 1500.0).abs() < 1e-12);
        // Out-of-range factors are clamped, not extrapolated.
        assert!((under_relax(2000.0, 1000.0, 5.0) - 2000.0).abs() < 1e-12);
        assert!((under_relax(2000.0, 1000.0, -1.0) - 1000.0).abs() < 1e-12);
    }

    /// Self-consistency check — validation rejects the states that would
    /// otherwise be silently guarded.
    #[test]
    fn checked_evaluation_rejects_unphysical_input() {
        let gas = helium();
        let good = GapSurfaces::lwr_open_gap(900.0, 600.0);

        assert!(rod(8.5e-5).evaluate_checked(&gas, 2.25e6, &good).is_ok());
        assert!(rod(-1.0).evaluate_checked(&gas, 2.25e6, &good).is_err());
        assert!(rod(8.5e-5).evaluate_checked(&gas, 0.0, &good).is_err());

        let mut bad = good;
        bad.fuel_temperature = -1.0;
        assert!(rod(8.5e-5).evaluate_checked(&gas, 2.25e6, &bad).is_err());

        let mut bad = good;
        bad.fuel_emissivity = 1.5;
        assert!(rod(8.5e-5).evaluate_checked(&gas, 2.25e6, &bad).is_err());

        let mut bad = good;
        bad.interface_pressure = -1.0;
        assert!(rod(8.5e-5).evaluate_checked(&gas, 2.25e6, &bad).is_err());

        // A negative offset can drive a term negative; that is reported.
        let negative = GapConductanceModel::FuelRodFrapcon {
            radial_gap_width: 8.5e-5,
            scaling: GapConductanceScaling {
                gas_offset: -1.0e9,
                ..GapConductanceScaling::default()
            },
        };
        assert!(negative.evaluate_checked(&gas, 2.25e6, &good).is_err());

        // TRISO geometry checks.
        let inverted = GapConductanceModel::TrisoSpherical {
            reference_radius: 1.0e-4,
            inner_radius: 2.0e-4,
            outer_radius: 1.0e-4,
            jump_distance: TrisoJumpDistance::Frapcon,
        };
        assert!(inverted.evaluate_checked(&gas, 2.25e6, &good).is_err());
    }

    /// Self-consistency check — the fixed-conductance variant is a pass-through.
    #[test]
    fn fixed_model_returns_its_coefficient() {
        let gas = helium();
        let surfaces = GapSurfaces::lwr_open_gap(900.0, 600.0);
        let h = GapConductanceModel::Fixed {
            coefficient: 5000.0,
        }
        .evaluate(&gas, 2.25e6, &surfaces);
        assert_eq!(h.total(), 5000.0);
        assert_eq!(h.radiation, 0.0);
        assert_eq!(h.contact, 0.0);
    }

    /// Self-consistency check — interface averaging is the plain mean.
    #[test]
    fn interface_average_is_the_arithmetic_mean() {
        assert!((average_across_interface(1000.0, 3000.0) - 2000.0).abs() < 1e-12);
    }
}
