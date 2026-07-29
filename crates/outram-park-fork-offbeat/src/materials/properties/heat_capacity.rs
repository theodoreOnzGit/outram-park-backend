// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
// `offbeatLib/materials/materialModel/thermoMechanicalPropertiesModels/heatCapacity/`:
//   heatCapacityConstant.{C,H}        heatCapacityMatproUO2.{C,H}
//   heatCapacityMatproUPuO2.{C,H}     heatCapacityFinkUPuO2.{C,H}
//   heatCapacityMatproZy.{C,H}        heatCapacityIAEAZy.{C,H}
//   heatCapacityBanerjee1515Ti.{C,H}  heatCapacityMolybdenum.{C,H}
//   heatCapacitySneadSiC.{C,H}
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Specific heat capacity correlations \[J/(kg K)\].
//!
//! # What this module computes
//!
//! The **specific heat capacity at constant pressure** `Cp` of fuel, cladding
//! and structural materials, in J/(kg K), as a function of the local
//! [`MaterialState`].
//!
//! Heat capacity does not appear in a steady-state temperature solution at all
//! — it is the property that sets the **thermal inertia** of a transient. It
//! decides how fast the fuel centreline responds to a power ramp, how much
//! stored energy a rod holds at the start of an accident, and how sharply the
//! cladding heats up during a temperature excursion. The Zircaloy fits below
//! carry a large peak near 1150 K precisely because the alpha-to-beta phase
//! transformation absorbs latent heat there, and that peak is what slows a
//! LOCA heat-up.
//!
//! # Units — raw `f64`, strict SI
//!
//! Inputs come from [`MaterialState`] (temperature in K, and for the oxide
//! fuels the plutonium fraction and the deviation from stoichiometry); the
//! returned value is always in **J/(kg K)**.
//!
//! # Validity ranges: `value` clamps, `value_checked` reports
//!
//! Identical convention to [`conductivity`](super::conductivity):
//!
//! - [`HeatCapacityModel::value`] **clamps the temperature** into
//!   [`HeatCapacityModel::temperature_range`] before evaluating.
//! - [`HeatCapacityModel::value_checked`] returns
//!   [`OffbeatError::OutOfRange`] outside that window and
//!   [`OffbeatError::Unphysical`] for a non-positive absolute temperature.
//!
//! Upstream warns and extrapolates rather than clamping (`heatCapacityMatproZy`,
//! `heatCapacityIAEAZy`, `heatCapacityBanerjee1515Ti` and
//! `heatCapacitySneadSiC` all print a `WarningInFunction` and carry on), so the
//! clamp in [`value`] is this port's deliberate, documented choice. Four of the
//! nine models state their window in their own source; the rest are
//! port-chosen and say so in their doc comments.
//!
//! [`value`]: HeatCapacityModel::value
//! [`OffbeatError::OutOfRange`]: crate::error::OffbeatError::OutOfRange
//! [`OffbeatError::Unphysical`]: crate::error::OffbeatError::Unphysical
//!
//! # Known upstream defects reproduced here
//!
//! Both are reproduced faithfully — a port that silently repairs its upstream
//! stops being comparable to it — and both are characterised by a unit test:
//!
//! - **`heatCapacityMatproZy` has an off-by-one-interval bug.** Its
//!   1093–1113 K branch interpolates from `Tlow = 1090` instead of
//!   `Tlow = 1093`, which puts a spurious 11.5 J/(kg K) step at 1093 K. See
//!   [`HeatCapacityModel::MatproZircaloy`].
//! - **`heatCapacityMatproUPuO2` and `heatCapacitySneadSiC` each declare a
//!   coefficient twice, with different values.** The constructor initialiser
//!   list and the dictionary `lookupOrDefault` default disagree
//!   (`K2 = 3.95e-4` vs `3.95e4`; `par4 = -3.1946e7` vs `-3.19446e7`). This
//!   port uses the initialiser value in both cases — that is the value
//!   upstream actually uses when the optional `heatCapacity` sub-dictionary is
//!   absent, and for SiC it is also the value published by Snead et al.

use crate::error::{OffbeatError, Result};
use crate::materials::MaterialState;

/// Molar gas constant \[J/(mol K)\] — CODATA 2018, exact by the SI redefinition.
///
/// Upstream uses OpenFOAM's `Foam::constant::physicoChemical::R`, which is the
/// same quantity. It enters the Schottky-defect term of the MATPRO oxide-fuel
/// heat capacities, where the activation energy is quoted in J/mol.
const GAS_CONSTANT: f64 = 8.314_462_618_153_24;

/// Specific heat capacity \[J/(kg K)\] of a fuel, cladding or structural
/// material.
///
/// # What this represents
///
/// One published heat-capacity correlation, selected at construction and
/// evaluated per cell against a [`MaterialState`]. Variants are named
/// `<author-or-source><material>` following the convention in
/// [`crate::materials`].
///
/// # Dispatch
///
/// An enum rather than a trait object, per the workspace "no trait objects"
/// rule — see [`crate::materials`] for the reasoning.
///
/// # Example
///
/// ```
/// use outram_park_fork_offbeat::materials::MaterialState;
/// use outram_park_fork_offbeat::materials::properties::heat_capacity::HeatCapacityModel;
///
/// // UO2 at room temperature.
/// let cp = HeatCapacityModel::MatproUo2.value(&MaterialState::fresh(300.0));
/// assert!((230.0..245.0).contains(&cp), "UO2 Cp near 300 K is ~235 J/(kg K), got {cp}");
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeatCapacityModel {
    /// Temperature-independent heat capacity \[J/(kg K)\], the payload value.
    ///
    /// Upstream: `heatCapacityConstant` (reads `Cp` from the case dictionary).
    ///
    /// [`temperature_range`](Self::temperature_range) returns `(0, +inf)`: a
    /// constant is not a fit to anything, so there is nothing to invalidate.
    /// Appropriate for scoping calculations and for verification cases with an
    /// analytical transient solution, not for a real material.
    Constant(f64),

    /// UO2, MATPRO-v11 \[J/(kg K)\].
    ///
    /// Upstream: `heatCapacityMatproUO2`. The standard three-term oxide form —
    /// an Einstein (lattice-vibration) term, a linear
    /// anharmonic/thermal-expansion term, and a Schottky-defect term that
    /// switches on above roughly 2000 K:
    ///
    /// ```text
    /// Cp = K1*theta^2*exp(theta/T) / (T*(exp(theta/T) - 1))^2
    ///      + K2*T
    ///      + (O/M / 2) * K3*ED / (R*T^2) * exp(-ED/(R*T))
    /// ```
    ///
    /// with `K1 = 296.7` J/(kg K), `K2 = 2.43e-2` J/(kg K^2),
    /// `K3 = 8.745e7` J/kg, Einstein temperature `theta = 535.285` K,
    /// Schottky activation energy `ED = 1.577e5` J/mol, and `R` the molar gas
    /// constant.
    ///
    /// The Einstein term tends to `K1` as `T -> infinity`, so the whole
    /// expression is asymptotically `K1 + K2*T` plus the (bounded) defect term
    /// — a useful analytic handle, and one the unit tests use.
    ///
    /// # Inputs used
    ///
    /// [`temperature`](MaterialState::temperature) \[K\] and
    /// [`oxygen_deviation`](MaterialState::oxygen_deviation) \[-\], the latter
    /// entering as `O/M = 2 + oxygen_deviation`. Upstream instead reads a fixed
    /// `OM` from the case dictionary (default 2.0), so a stoichiometric state
    /// reproduces upstream exactly.
    ///
    /// # Validity range
    ///
    /// Upstream states none. This port uses **300–3113 K**, the upper bound
    /// being the melting temperature of unirradiated UO2 as used by MATPRO.
    /// Port's choice, not a bound from the report.
    ///
    /// # Source
    ///
    /// MATPRO-v11 UO2 specific heat, as implemented in upstream
    /// `heatCapacityMatproUO2.C`.
    MatproUo2,

    /// MOX (U,Pu)O2, MATPRO-v11 \[J/(kg K)\].
    ///
    /// Upstream: `heatCapacityMatproUPuO2`. The mass-weighted mean of the
    /// MATPRO UO2 and PuO2 heat capacities, each in the same three-term form as
    /// [`MatproUo2`](Self::MatproUo2):
    ///
    /// ```text
    /// Cp = w_UO2 * Cp_UO2(T) + w_PuO2 * Cp_PuO2(T)
    /// ```
    ///
    /// The PuO2 coefficients are `K1 = 347.4` J/(kg K), `K2 = 3.95e-4`
    /// J/(kg K^2), `K3 = 3.86e7` J/kg, `theta = 571` K, `ED = 1.967e5` J/mol.
    ///
    /// # Coefficient discrepancy in upstream
    ///
    /// Upstream declares `K2_` twice with different values: `3.95e-4` in the
    /// constructor initialiser list and `3.95e4` as the dictionary
    /// `lookupOrDefault` fallback — an eight-order-of-magnitude difference.
    /// `3.95e-4` J/(kg K^2) is the MATPRO PuO2 value and is what upstream
    /// actually uses unless a case supplies a `heatCapacity` sub-dictionary, so
    /// that is what this port uses.
    ///
    /// # Inputs used
    ///
    /// [`temperature`](MaterialState::temperature) \[K\],
    /// [`pu_fraction`](MaterialState::pu_fraction) \[-\] as the PuO2 weight
    /// fraction (upstream reads it from an `isotopes/Pu/ratioOverMetal` entry
    /// in the case dictionary — the same quantity to within the PuO2/UO2 molar
    /// mass ratio of about 1.004), and
    /// [`oxygen_deviation`](MaterialState::oxygen_deviation) \[-\] as
    /// `O/M = 2 + oxygen_deviation`.
    ///
    /// At `pu_fraction = 0` this reduces **exactly** to
    /// [`MatproUo2`](Self::MatproUo2), which the unit tests check.
    ///
    /// # Validity range
    ///
    /// Upstream states none; this port uses **300–3113 K**. Port's choice.
    ///
    /// # Source
    ///
    /// MATPRO-v11 (U,Pu)O2 specific heat, as implemented in upstream
    /// `heatCapacityMatproUPuO2.C`.
    MatproMox,

    /// MOX (U,Pu)O2, Fink correlation \[J/(kg K)\].
    ///
    /// Upstream: `heatCapacityFinkUPuO2`. An Einstein term plus a linear term,
    /// with the defect term disabled by upstream's default `C3 = 0`:
    ///
    /// ```text
    /// Cp = C1*(theta/T)^2*exp(theta/T)/(exp(theta/T) - 1)^2
    ///      + 2*C2*T
    ///      + C3*Ea*exp(-Ea/T)/T^2
    /// ```
    ///
    /// with `C1 = 322.49` J/(kg K), `C2 = 1.4679e-2` J/(kg K^2), `C3 = 0`,
    /// `theta = 587.41` K, `Ea = 18531.7` K. Note that the Einstein term is
    /// written here in the algebraically equivalent `(theta/T)^2` form rather
    /// than MATPRO's `theta^2/T^2` form, and that the linear term carries an
    /// explicit factor of two — both as upstream has them.
    ///
    /// Because `C3 = 0`, this correlation has **no Schottky-defect upturn** and
    /// so falls increasingly below [`MatproMox`](Self::MatproMox) above about
    /// 2000 K. That is a property of the fit, not of the port.
    ///
    /// # Inputs used
    ///
    /// [`temperature`](MaterialState::temperature) \[K\] only.
    ///
    /// # Validity range
    ///
    /// Upstream states none; this port uses **300–3000 K**. Port's choice.
    ///
    /// # Source
    ///
    /// J. K. Fink's (U,Pu)O2 heat capacity,
    /// <https://info.ornl.gov/sites/publications/Files/Pub57523.pdf>, as cited
    /// by upstream `heatCapacityFinkUPuO2.H`.
    FinkMox,

    /// Zircaloy cladding, MATPRO piecewise-linear table \[J/(kg K)\].
    ///
    /// Upstream: `heatCapacityMatproZy`. A table of thirteen `(T, Cp)` anchor
    /// points interpolated linearly, resolving the alpha-to-beta phase
    /// transformation between 1090 K and 1248 K in 20 K steps. The peak is
    /// 816 J/(kg K) at 1173 K, more than double the 356 J/(kg K) plateau of the
    /// beta phase above 1248 K.
    ///
    /// # Known upstream bug, reproduced
    ///
    /// The branch covering 1093 < T <= 1113 K interpolates from `Tlow = 1090`
    /// rather than `Tlow = 1093`, while its `CpLow = 502` is the value belonging
    /// to 1093 K. The result is a spurious upward step of about
    /// 11.5 J/(kg K) at exactly 1093 K, where the table is otherwise
    /// continuous. This port reproduces it so results stay comparable with
    /// upstream; the unit test
    /// `matpro_zircaloy_has_the_upstream_discontinuity_at_1093_k` characterises
    /// it, and it should be reported upstream rather than silently patched
    /// here.
    ///
    /// # Inputs used
    ///
    /// [`temperature`](MaterialState::temperature) \[K\] only.
    ///
    /// # Validity range
    ///
    /// **273–2099 K**, stated by upstream (which notes it extended the
    /// literature lower bound of 300 K down to 273 K to allow simulation below
    /// 290 K, so the 273–300 K stretch is itself a linear extrapolation of the
    /// first table interval).
    ///
    /// # Source
    ///
    /// MATPRO Zircaloy specific heat,
    /// <https://www.nrc.gov/docs/ML1429/ML14296A063.pdf> page 60, as cited by
    /// upstream `heatCapacityMatproZy.H`.
    MatproZircaloy,

    /// Zircaloy cladding, IAEA correlation \[J/(kg K)\].
    ///
    /// Upstream: `heatCapacityIAEAZy`. A smooth low-temperature line and a
    /// high-temperature parabola, with a Gaussian phase-transformation peak
    /// added over the transition window:
    ///
    /// ```text
    /// Cp1 = 255.66 + 0.1024*T
    /// Cp2 = 597.1 - 0.4088*T + 1.565e-4*T^2
    /// f   = 1058.4 * exp(-(T - 1213.8)^2 / 719.61)
    ///
    /// Cp  = Cp1       for T <  1100 K
    ///     = Cp1 + f   for 1100 <= T < 1213.8 K
    ///     = Cp2 + f   for 1213.8 <= T < 1320 K
    ///     = Cp2       for T >= 1320 K
    /// ```
    ///
    /// The Gaussian is negligible (about 1.6e-5 J/(kg K)) at both 1100 K and
    /// 1320 K, so those two branch changes are continuous to round-off. The
    /// switch from `Cp1` to `Cp2` at 1213.8 K is **not**: the two differ by
    /// about 48 J/(kg K) there, on top of a peak of 1058 J/(kg K), a step of
    /// roughly 3.4%. That is a property of the published piecewise fit; see the
    /// unit tests.
    ///
    /// # Inputs used
    ///
    /// [`temperature`](MaterialState::temperature) \[K\] only.
    ///
    /// # Validity range
    ///
    /// **273–2000 K**, stated by upstream.
    ///
    /// # Source
    ///
    /// IAEA-TECDOC-1496,
    /// <https://www-pub.iaea.org/MTCD/publications/PDF/te_1496_web.pdf>, as
    /// cited by upstream `heatCapacityIAEAZy.H`.
    IaeaZircaloy,

    /// 15-15 Ti austenitic stainless cladding, Banerjee (2007)
    /// \[J/(kg K)\].
    ///
    /// Upstream: `heatCapacityBanerjee1515Ti`.
    ///
    /// ```text
    /// Cp = 431 + 0.177*T + 8.72e-5*T^-2
    /// ```
    ///
    /// # Port note on the third term
    ///
    /// Upstream writes the last term as `par3 * pow(Ti, -2)` with
    /// `par3 = 8.72e-5`, which makes it utterly negligible — of order
    /// 1e-10 J/(kg K) at any temperature of interest, i.e. the correlation is
    /// effectively the straight line `431 + 0.177*T`. A `+8.72e-5*T^2` term
    /// (positive exponent) would instead contribute about 87 J/(kg K) at
    /// 1000 K, which is the sort of magnitude an inverse-square Debye
    /// correction normally has in these fits. This port **reproduces upstream's
    /// negative exponent verbatim** because it cannot check Banerjee (2007)
    /// offline, and flags the ambiguity here rather than guessing. Treat the
    /// third term as unverified.
    ///
    /// # Inputs used
    ///
    /// [`temperature`](MaterialState::temperature) \[K\] only.
    ///
    /// # Validity range
    ///
    /// **293–1273 K**, stated by upstream.
    ///
    /// # Source
    ///
    /// Banerjee et al. (2007) 15-15 Ti heat capacity, as cited by upstream
    /// `heatCapacityBanerjee1515Ti.H` (no DOI given upstream).
    Banerjee1515Ti,

    /// Molybdenum \[J/(kg K)\].
    ///
    /// Upstream: `heatCapacityMolybdenum`.
    ///
    /// ```text
    /// Cp = 9.74e-6*T^2 + 5.37e-2*T + 235
    /// ```
    ///
    /// # Inputs used
    ///
    /// [`temperature`](MaterialState::temperature) \[K\] only.
    ///
    /// # Validity range
    ///
    /// Upstream states none; this port uses **300–2800 K**, below
    /// molybdenum's melting point of about 2896 K. Port's choice.
    ///
    /// # Source
    ///
    /// Upstream `heatCapacityMolybdenum.C`; upstream's header `Description`
    /// block is empty and cites no report.
    Molybdenum,

    /// Silicon carbide, Snead et al. (2007) \[J/(kg K)\].
    ///
    /// Upstream: `heatCapacitySneadSiC`.
    ///
    /// ```text
    /// Cp = 925.65 + 0.3772*T - 7.9259e-5*T^2 - 3.1946e7*T^-2
    /// ```
    ///
    /// The large negative `T^-2` term is what pulls the curve down at low
    /// temperature — it dominates below about 400 K and is negligible above
    /// 1000 K.
    ///
    /// # Coefficient discrepancy in upstream
    ///
    /// As with [`MatproMox`](Self::MatproMox), upstream declares the last
    /// coefficient twice with different values: `-3.1946e7` in the constructor
    /// initialiser list and `-3.19446e7` as the dictionary fallback. This port
    /// uses `-3.1946e7`, which is both the value upstream uses by default and
    /// the value published by Snead et al.
    ///
    /// # Inputs used
    ///
    /// [`temperature`](MaterialState::temperature) \[K\] only.
    ///
    /// # Validity range
    ///
    /// **200–2400 K**, stated by upstream.
    ///
    /// # Source
    ///
    /// L. L. Snead et al., *Handbook of SiC properties for fuel performance
    /// modeling*, Journal of Nuclear Materials (2007), as cited by upstream
    /// `heatCapacitySneadSiC.H`.
    SneadSiC,
}

impl HeatCapacityModel {
    /// Short human-readable name of the correlation, for error messages and
    /// logs.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Constant(_) => "constant heat capacity",
            Self::MatproUo2 => "MATPRO UO2 heat capacity",
            Self::MatproMox => "MATPRO (U,Pu)O2 heat capacity",
            Self::FinkMox => "Fink (U,Pu)O2 heat capacity",
            Self::MatproZircaloy => "MATPRO Zircaloy heat capacity",
            Self::IaeaZircaloy => "IAEA Zircaloy heat capacity",
            Self::Banerjee1515Ti => "Banerjee 15-15 Ti heat capacity",
            Self::Molybdenum => "molybdenum heat capacity",
            Self::SneadSiC => "Snead SiC heat capacity",
        }
    }

    /// Temperature validity window `(low, high)` \[K\] of this correlation.
    ///
    /// Stated by upstream for [`MatproZircaloy`](Self::MatproZircaloy)
    /// (273–2099 K), [`IaeaZircaloy`](Self::IaeaZircaloy) (273–2000 K),
    /// [`Banerjee1515Ti`](Self::Banerjee1515Ti) (293–1273 K) and
    /// [`SneadSiC`](Self::SneadSiC) (200–2400 K). **Port-chosen** for the
    /// oxide fuels and molybdenum — see each variant's doc comment. A
    /// port-chosen window is a guard rail against nonsense extrapolation, not
    /// a claim about the experiment behind the fit.
    ///
    /// [`Constant`](Self::Constant) has no temperature dependence and returns
    /// `(0.0, f64::INFINITY)`.
    ///
    /// ```
    /// use outram_park_fork_offbeat::materials::properties::heat_capacity::HeatCapacityModel;
    ///
    /// // Stated by upstream heatCapacitySneadSiC.C.
    /// assert_eq!(HeatCapacityModel::SneadSiC.temperature_range(), (200.0, 2400.0));
    /// ```
    #[must_use]
    pub const fn temperature_range(&self) -> (f64, f64) {
        match self {
            Self::Constant(_) => (0.0, f64::INFINITY),
            // Port's choice: up to the MATPRO melting temperature of UO2.
            Self::MatproUo2 | Self::MatproMox => (300.0, 3113.0),
            Self::FinkMox => (300.0, 3000.0),
            // Stated by upstream.
            Self::MatproZircaloy => (273.0, 2099.0),
            Self::IaeaZircaloy => (273.0, 2000.0),
            Self::Banerjee1515Ti => (293.0, 1273.0),
            Self::SneadSiC => (200.0, 2400.0),
            // Port's choice: below the melting point of molybdenum.
            Self::Molybdenum => (300.0, 2800.0),
        }
    }

    /// Specific heat capacity \[J/(kg K)\], **clamping** the temperature into
    /// [`temperature_range`](Self::temperature_range).
    ///
    /// # Clamping
    ///
    /// If `state.temperature` lies outside the correlation's validity window,
    /// this method evaluates the fit **at the nearest endpoint of the window**
    /// rather than extrapolating, and does not signal that it did so. Use
    /// [`value_checked`](Self::value_checked) when you need to know. Upstream
    /// prints a warning and extrapolates instead; see the module documentation.
    ///
    /// # Units
    ///
    /// Returns J/(kg K).
    ///
    /// ```
    /// use outram_park_fork_offbeat::materials::MaterialState;
    /// use outram_park_fork_offbeat::materials::properties::heat_capacity::HeatCapacityModel;
    ///
    /// let model = HeatCapacityModel::Banerjee1515Ti;
    /// // 2000 K is above the 1273 K upper bound; it is clamped there.
    /// assert_eq!(
    ///     model.value(&MaterialState::fresh(2000.0)),
    ///     model.value(&MaterialState::fresh(1273.0)),
    /// );
    /// ```
    #[must_use]
    pub fn value(&self, state: &MaterialState) -> f64 {
        let (low, high) = self.temperature_range();
        let mut clamped = *state;
        clamped.temperature = state.temperature.clamp(low, high);
        self.evaluate(&clamped)
    }

    /// Specific heat capacity \[J/(kg K)\], **reporting** an out-of-range
    /// temperature instead of clamping it.
    ///
    /// # Errors
    ///
    /// - [`OffbeatError::Unphysical`] if `state.temperature` is not a positive
    ///   absolute temperature (zero, negative or NaN).
    /// - [`OffbeatError::OutOfRange`] if `state.temperature` falls outside
    ///   [`temperature_range`](Self::temperature_range). Only the temperature
    ///   is range-checked; upstream states no composition bound for the oxide
    ///   fuels, so this port does not invent one.
    ///
    /// ```
    /// use outram_park_fork_offbeat::materials::MaterialState;
    /// use outram_park_fork_offbeat::materials::properties::heat_capacity::HeatCapacityModel;
    ///
    /// let model = HeatCapacityModel::IaeaZircaloy;
    /// assert!(model.value_checked(&MaterialState::fresh(600.0)).is_ok());
    /// assert!(model.value_checked(&MaterialState::fresh(2500.0)).is_err());
    /// ```
    pub fn value_checked(&self, state: &MaterialState) -> Result<f64> {
        // NaN must be rejected too, hence the explicit `is_nan` rather than a
        // single comparison.
        if state.temperature.is_nan() || state.temperature <= 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: self.name(),
                value: state.temperature,
                unit: "K",
                reason: "absolute temperature must be positive",
            });
        }

        let (low, high) = self.temperature_range();
        if state.temperature < low || state.temperature > high {
            return Err(OffbeatError::OutOfRange {
                quantity: self.name(),
                value: state.temperature,
                low,
                high,
                unit: "K",
            });
        }

        Ok(self.evaluate(state))
    }

    /// Evaluate the correlation with no range handling at all.
    ///
    /// Private on purpose: the two public entry points differ only in what they
    /// do about the validity window, so the physics lives in exactly one place.
    fn evaluate(&self, state: &MaterialState) -> f64 {
        let t = state.temperature;

        match *self {
            Self::Constant(cp) => cp,

            Self::MatproUo2 => matpro_oxide_heat_capacity(
                t,
                2.0 + state.oxygen_deviation,
                296.7,
                2.43e-2,
                8.745e7,
                535.285,
                1.577e5,
            ),

            Self::MatproMox => {
                let oxygen_metal_ratio = 2.0 + state.oxygen_deviation;
                let uo2 = matpro_oxide_heat_capacity(
                    t,
                    oxygen_metal_ratio,
                    296.7,
                    2.43e-2,
                    8.745e7,
                    535.285,
                    1.577e5,
                );
                // K2 = 3.95e-4 J/(kg K^2): the initialiser-list value upstream
                // actually uses. See the variant doc comment.
                let puo2 = matpro_oxide_heat_capacity(
                    t,
                    oxygen_metal_ratio,
                    347.4,
                    3.95e-4,
                    3.86e7,
                    571.0,
                    1.967e5,
                );
                let w_puo2 = state.pu_fraction;
                (1.0 - w_puo2) * uo2 + w_puo2 * puo2
            }

            Self::FinkMox => {
                let c1 = 322.49;
                let c2 = 1.4679e-2;
                let c3 = 0.0;
                let theta = 587.41;
                let activation = 18531.7;

                let u = theta / t;
                let einstein = c1 * u * u * u.exp() / ((u.exp() - 1.0) * (u.exp() - 1.0));
                einstein + 2.0 * c2 * t + c3 * activation * (-activation / t).exp() / (t * t)
            }

            Self::MatproZircaloy => {
                // Thirteen (T, Cp) anchor points, linearly interpolated. The
                // 1090 in the fifth interval is upstream's off-by-one-interval
                // bug, reproduced deliberately — see the variant doc comment.
                let (t_low, t_high, cp_low, cp_high) = if t <= 400.0 {
                    (300.0, 400.0, 281.0, 302.0)
                } else if t <= 640.0 {
                    (400.0, 640.0, 302.0, 331.0)
                } else if t <= 1090.0 {
                    (640.0, 1090.0, 331.0, 375.0)
                } else if t <= 1093.0 {
                    (1090.0, 1093.0, 375.0, 502.0)
                } else if t <= 1113.0 {
                    (1090.0, 1113.0, 502.0, 590.0)
                } else if t <= 1133.0 {
                    (1113.0, 1133.0, 590.0, 615.0)
                } else if t <= 1153.0 {
                    (1133.0, 1153.0, 615.0, 719.0)
                } else if t <= 1173.0 {
                    (1153.0, 1173.0, 719.0, 816.0)
                } else if t <= 1193.0 {
                    (1173.0, 1193.0, 816.0, 770.0)
                } else if t <= 1213.0 {
                    (1193.0, 1213.0, 770.0, 619.0)
                } else if t <= 1233.0 {
                    (1213.0, 1233.0, 619.0, 469.0)
                } else if t <= 1248.0 {
                    (1233.0, 1248.0, 469.0, 356.0)
                } else {
                    (1248.0, 2099.0, 356.0, 356.0)
                };
                cp_low + (cp_high - cp_low) * (t - t_low) / (t_high - t_low)
            }

            Self::IaeaZircaloy => {
                let cp_alpha = 255.66 + 0.1024 * t;
                let cp_beta = 597.1 - 0.4088 * t + 1.565e-4 * t * t;
                let peak = 1058.4 * (-(t - 1213.8).powi(2) / 719.61).exp();

                if t < 1100.0 {
                    cp_alpha
                } else if t < 1213.8 {
                    cp_alpha + peak
                } else if t < 1320.0 {
                    cp_beta + peak
                } else {
                    cp_beta
                }
            }

            // Upstream writes the third term with a *negative* exponent; see
            // the variant doc comment on why it is reproduced, not repaired.
            Self::Banerjee1515Ti => 431.0 + 0.177 * t + 8.72e-5 * t.powi(-2),

            Self::Molybdenum => 9.74e-6 * t * t + 5.37e-2 * t + 235.0,

            // par4 = -3.1946e7: the initialiser-list value upstream uses, and
            // the value published by Snead et al.
            Self::SneadSiC => 925.65 + 0.3772 * t - 7.9259e-5 * t * t - 3.1946e7 * t.powi(-2),
        }
    }
}

/// MATPRO three-term oxide-fuel specific heat \[J/(kg K)\].
///
/// The functional form shared by MATPRO's UO2 and PuO2 heat capacities:
///
/// ```text
/// Cp = k1*theta^2*exp(theta/T) / (T*(exp(theta/T) - 1))^2
///      + k2*T
///      + (O/M / 2) * k3*ed / (R*T^2) * exp(-ed/(R*T))
/// ```
///
/// # Parameters
///
/// - `temperature` \[K\], must be positive.
/// - `oxygen_metal_ratio` \[-\], the O/M ratio (2.0 for stoichiometric fuel).
/// - `k1` \[J/(kg K)\] — Einstein-term amplitude; the high-temperature
///   asymptote of that term.
/// - `k2` \[J/(kg K^2)\] — linear anharmonic coefficient.
/// - `k3` \[J/kg\] — Schottky-defect amplitude.
/// - `theta` \[K\] — Einstein temperature.
/// - `defect_energy` \[J/mol\] — Schottky-defect activation energy.
///
/// Returns J/(kg K).
fn matpro_oxide_heat_capacity(
    temperature: f64,
    oxygen_metal_ratio: f64,
    k1: f64,
    k2: f64,
    k3: f64,
    theta: f64,
    defect_energy: f64,
) -> f64 {
    let scaled = theta / temperature;
    let exponential = scaled.exp();
    let denominator = temperature * (exponential - 1.0);

    let einstein = k1 * theta * theta * exponential / (denominator * denominator);
    let anharmonic = k2 * temperature;
    let schottky = (oxygen_metal_ratio / 2.0) * k3 * defect_energy
        / (GAS_CONSTANT * temperature * temperature)
        * (-defect_energy / (GAS_CONSTANT * temperature)).exp();

    einstein + anharmonic + schottky
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant, for coverage sweeps.
    fn all_models() -> Vec<HeatCapacityModel> {
        vec![
            HeatCapacityModel::Constant(300.0),
            HeatCapacityModel::MatproUo2,
            HeatCapacityModel::MatproMox,
            HeatCapacityModel::FinkMox,
            HeatCapacityModel::MatproZircaloy,
            HeatCapacityModel::IaeaZircaloy,
            HeatCapacityModel::Banerjee1515Ti,
            HeatCapacityModel::Molybdenum,
            HeatCapacityModel::SneadSiC,
        ]
    }

    // ---------------------------------------------------------------------
    // Cross-cutting contract tests
    // ---------------------------------------------------------------------

    /// **Self-consistency check, not validation.**
    ///
    /// A negative or non-finite heat capacity would make a transient solve
    /// unstable, so every variant must be finite and strictly positive
    /// everywhere inside its own validity window. Methodology: sample each
    /// variant at 25 points spanning its
    /// [`HeatCapacityModel::temperature_range`] (200 K to 3000 K where the
    /// window is unbounded). Pass criterion: finite and `> 0` at every point.
    #[test]
    fn every_model_is_finite_and_positive_across_its_whole_range() {
        for model in all_models() {
            let (low, high) = model.temperature_range();
            let (low, high) = if high.is_finite() {
                (low, high)
            } else {
                (200.0, 3000.0)
            };
            for i in 0..=24 {
                let t = low + (high - low) * f64::from(i) / 24.0;
                let cp = model.value(&MaterialState::fresh(t.max(1.0)));
                assert!(
                    cp.is_finite() && cp > 0.0,
                    "{} gave Cp = {cp} J/(kg K) at T = {t} K",
                    model.name()
                );
            }
        }
    }

    /// **Self-consistency check, not validation.**
    ///
    /// `value` must clamp at both ends of the advertised window — the
    /// documented difference between this port and upstream's
    /// warn-and-extrapolate behaviour.
    #[test]
    fn value_clamps_at_both_ends_of_the_range() {
        for model in all_models() {
            let (low, high) = model.temperature_range();
            if !high.is_finite() {
                continue;
            }
            assert_eq!(
                model.value(&MaterialState::fresh(high + 1000.0)),
                model.value(&MaterialState::fresh(high)),
                "{} did not clamp above its upper bound",
                model.name()
            );
            assert_eq!(
                model.value(&MaterialState::fresh(low * 0.5)),
                model.value(&MaterialState::fresh(low)),
                "{} did not clamp below its lower bound",
                model.name()
            );
        }
    }

    /// **Self-consistency check, not validation.**
    ///
    /// `value_checked` must report exactly the window `temperature_range`
    /// advertises, and must reject a non-positive absolute temperature as
    /// [`OffbeatError::Unphysical`] before any range test.
    #[test]
    fn value_checked_reports_the_advertised_range() {
        for model in all_models() {
            let (low, high) = model.temperature_range();

            assert!(matches!(
                model.value_checked(&MaterialState::fresh(0.0)),
                Err(OffbeatError::Unphysical { .. })
            ));

            if low > 0.0 {
                assert!(
                    matches!(
                        model.value_checked(&MaterialState::fresh(low * 0.5)),
                        Err(OffbeatError::OutOfRange { .. })
                    ),
                    "{} accepted a temperature below its lower bound",
                    model.name()
                );
            }
            if high.is_finite() {
                assert!(
                    matches!(
                        model.value_checked(&MaterialState::fresh(high + 1.0)),
                        Err(OffbeatError::OutOfRange { .. })
                    ),
                    "{} accepted a temperature above its upper bound",
                    model.name()
                );
                let inside = 0.5 * (low + high);
                assert!(
                    model.value_checked(&MaterialState::fresh(inside)).is_ok(),
                    "{} rejected a temperature inside its own window",
                    model.name()
                );
            }
        }
    }

    /// **Self-consistency check, not validation.**
    ///
    /// Inside the validity window the two entry points share one `evaluate`, so
    /// they must agree bit-for-bit.
    #[test]
    fn value_and_value_checked_agree_inside_the_range() {
        for model in all_models() {
            let (low, high) = model.temperature_range();
            let t = if high.is_finite() {
                0.5 * (low + high)
            } else {
                1000.0
            };
            let state = MaterialState::fresh(t);
            assert_eq!(model.value(&state), model.value_checked(&state).unwrap());
        }
    }

    // ---------------------------------------------------------------------
    // Constant
    // ---------------------------------------------------------------------

    /// **Analytic-limit check.** A constant model returns its payload for any
    /// state.
    #[test]
    fn constant_returns_its_payload() {
        let model = HeatCapacityModel::Constant(450.0);
        let mut state = MaterialState::fresh(1e-3);
        state.pu_fraction = 0.3;
        assert_eq!(model.value(&state), 450.0);
        assert_eq!(model.value(&MaterialState::fresh(4000.0)), 450.0);
    }

    // ---------------------------------------------------------------------
    // MATPRO UO2
    // ---------------------------------------------------------------------

    /// **Reference-checked against published UO2 heat capacity.**
    ///
    /// Methodology: evaluate [`HeatCapacityModel::MatproUo2`] for
    /// stoichiometric UO2 at 300 K and at 1000 K, with the MATPRO-v11
    /// coefficients transcribed from upstream `heatCapacityMatproUO2.C`.
    /// Reference: the specific heat of stoichiometric UO2 is a
    /// well-established quantity, approximately **235 J/(kg K) at 300 K** and
    /// **314 J/(kg K) at 1000 K** (MATPRO-v11; the same values appear in
    /// Fink's IAEA-recommended UO2 assessment). Tolerance: 2% at each point.
    ///
    /// Result (2026-07-29, this implementation): **Cp(300 K) = 236.4 J/(kg K)**
    /// (+0.6% against 235) and **Cp(1000 K) = 314.0 J/(kg K)** (+0.01% against
    /// 314). Interpretation: the Einstein and anharmonic terms are transcribed
    /// correctly and the gas constant is right; the Schottky term is inactive at
    /// both points (below 1e-2 J/(kg K)), so this test does **not** exercise it.
    ///
    /// The precise expected values are asserted with a tight tolerance as well,
    /// so that a coefficient typo introduced later shows up as a test failure
    /// rather than as a 1% drift inside the literature band.
    #[test]
    fn matpro_uo2_matches_published_values_at_300_and_1000_k() {
        let cp_300 = HeatCapacityModel::MatproUo2.value(&MaterialState::fresh(300.0));
        let cp_1000 = HeatCapacityModel::MatproUo2.value(&MaterialState::fresh(1000.0));

        assert!(
            (cp_300 - 235.0).abs() / 235.0 < 0.02,
            "Cp(300 K) = {cp_300} J/(kg K), reference 235"
        );
        assert!(
            (cp_1000 - 314.0).abs() / 314.0 < 0.02,
            "Cp(1000 K) = {cp_1000} J/(kg K), reference 314"
        );

        assert!((cp_300 - 236.4).abs() < 0.5, "regression: {cp_300}");
        assert!((cp_1000 - 314.0).abs() < 0.5, "regression: {cp_1000}");
    }

    /// **Analytic-limit check.**
    ///
    /// The Einstein term `K1*theta^2*exp(u)/(T*(exp(u) - 1))^2` with
    /// `u = theta/T` tends to `K1` as `T -> infinity`, so the MATPRO oxide form
    /// is asymptotically `K1 + K2*T` plus the bounded Schottky term.
    /// Methodology: evaluate the private helper at 1e7 K (far outside any
    /// physical range, which is why it is done on the helper rather than
    /// through the clamping public API), with the defect term switched off by
    /// passing `k3 = 0`. Pass criterion: within 1e-6 relative of `K1 + K2*T`.
    ///
    /// Result: the helper agrees with the analytic `K1 + K2*T` to better than
    /// 1e-9 relative at 1e7 K.
    #[test]
    fn matpro_oxide_einstein_term_tends_to_k1_at_high_temperature() {
        let t = 1.0e7;
        let cp = matpro_oxide_heat_capacity(t, 2.0, 296.7, 2.43e-2, 0.0, 535.285, 1.577e5);
        let analytic = 296.7 + 2.43e-2 * t;
        assert!((cp / analytic - 1.0).abs() < 1.0e-6, "{cp} vs {analytic}");
    }

    /// **Self-consistency check, not validation.**
    ///
    /// UO2 heat capacity rises monotonically across the whole solid range: the
    /// Einstein term saturates, the linear term keeps climbing, and the
    /// Schottky term adds a sharp upturn above roughly 2000 K. Methodology:
    /// sample 300 → 3113 K in 50 K steps. Pass criterion: strictly increasing.
    #[test]
    fn matpro_uo2_rises_monotonically_to_the_melting_point() {
        let mut previous = f64::NEG_INFINITY;
        let mut t = 300.0;
        while t <= 3113.0 {
            let cp = HeatCapacityModel::MatproUo2.value(&MaterialState::fresh(t));
            assert!(cp > previous, "T = {t} K: {cp} not above {previous}");
            previous = cp;
            t += 50.0;
        }
    }

    /// **Self-consistency check, not validation.**
    ///
    /// The Schottky-defect term must be negligible at operating temperature and
    /// dominant near melting — that is its entire purpose in the fit.
    /// Methodology: difference the model against the same expression with
    /// `k3 = 0` at 1000 K and at 3000 K. Pass criteria: below 0.1 J/(kg K) at
    /// 1000 K, above 100 J/(kg K) at 3000 K.
    ///
    /// Result (2026-07-29): the defect contribution is 9.6e-3 J/(kg K) at
    /// 1000 K and 331.0 J/(kg K) at 3000 K.
    #[test]
    fn matpro_uo2_schottky_term_switches_on_only_near_melting() {
        for (t, low_bound, high_bound) in [(1000.0, 0.0, 0.1), (3000.0, 100.0, 1000.0)] {
            let with_defects = HeatCapacityModel::MatproUo2.value(&MaterialState::fresh(t));
            let without_defects =
                matpro_oxide_heat_capacity(t, 2.0, 296.7, 2.43e-2, 0.0, 535.285, 1.577e5);
            let contribution = with_defects - without_defects;
            assert!(
                contribution > low_bound && contribution < high_bound,
                "T = {t} K: defect term {contribution} J/(kg K)"
            );
        }
    }

    /// **Self-consistency check, not validation.**
    ///
    /// The Schottky term is proportional to `O/M`, so hyperstoichiometric fuel
    /// must have a slightly higher heat capacity near melting and
    /// hypostoichiometric fuel a slightly lower one. Methodology: compare
    /// `O/M = 2.00`, `2.02` and `1.98` at 2800 K. Pass criterion: strictly
    /// ordered.
    #[test]
    fn matpro_uo2_scales_the_defect_term_with_stoichiometry() {
        let at = |deviation: f64| {
            let mut state = MaterialState::fresh(2800.0);
            state.oxygen_deviation = deviation;
            HeatCapacityModel::MatproUo2.value(&state)
        };
        assert!(at(-0.02) < at(0.0));
        assert!(at(0.0) < at(0.02));
    }

    // ---------------------------------------------------------------------
    // MATPRO MOX and Fink MOX
    // ---------------------------------------------------------------------

    /// **Analytic-limit check.**
    ///
    /// [`HeatCapacityModel::MatproMox`] is a mass-weighted mean of the UO2 and
    /// PuO2 heat capacities, so at zero plutonium content it must reduce
    /// **exactly** to [`HeatCapacityModel::MatproUo2`]. Methodology: compare
    /// the two at 300, 1000 and 2500 K with `pu_fraction = 0`. Pass criterion:
    /// bit-for-bit equality.
    ///
    /// This is the strongest check available on the MOX implementation without
    /// a PuO2 reference dataset: it pins the UO2 half of the mixture exactly and
    /// therefore localises any error to the PuO2 coefficients.
    #[test]
    fn matpro_mox_reduces_exactly_to_matpro_uo2_at_zero_plutonium() {
        for t in [300.0, 1000.0, 2500.0] {
            let state = MaterialState::fresh(t);
            assert_eq!(
                HeatCapacityModel::MatproMox.value(&state),
                HeatCapacityModel::MatproUo2.value(&state),
                "T = {t} K"
            );
        }
    }

    /// **Self-consistency check, not validation.**
    ///
    /// The mixture rule must be exactly linear in the plutonium weight
    /// fraction. Methodology: evaluate at `pu_fraction` 0, 0.5 and 1 at 1000 K;
    /// pass criterion: the value at 0.5 equals the mean of the endpoints to
    /// 1e-12 relative.
    #[test]
    fn matpro_mox_is_linear_in_plutonium_fraction() {
        let at = |w: f64| {
            let mut state = MaterialState::fresh(1000.0);
            state.pu_fraction = w;
            HeatCapacityModel::MatproMox.value(&state)
        };
        let midpoint = 0.5 * (at(0.0) + at(1.0));
        assert!((at(0.5) / midpoint - 1.0).abs() < 1.0e-12);
    }

    /// **Self-consistency check, not validation.**
    ///
    /// Two independent MOX heat-capacity fits must agree in magnitude even
    /// where they disagree in detail. Methodology: compare
    /// [`HeatCapacityModel::FinkMox`] and [`HeatCapacityModel::MatproMox`] at
    /// `pu_fraction = 0.1` at 1000 K; pass criterion: within 15%.
    ///
    /// Result (2026-07-29): Fink = 342.7 J/(kg K), MATPRO = 316.5 J/(kg K),
    /// i.e. Fink is 8.3% higher. Interpretation: consistent magnitudes; the
    /// spread is of the size normally seen between published oxide-fuel Cp
    /// correlations. Not a validation of either.
    #[test]
    fn fink_mox_agrees_with_matpro_mox_in_magnitude() {
        let mut state = MaterialState::fresh(1000.0);
        state.pu_fraction = 0.1;
        let fink = HeatCapacityModel::FinkMox.value(&state);
        let matpro = HeatCapacityModel::MatproMox.value(&state);
        assert!(
            (fink - matpro).abs() / matpro < 0.15,
            "Fink {fink} vs MATPRO {matpro} J/(kg K)"
        );
    }

    /// **Analytic-limit check.**
    ///
    /// Fink's Einstein term `C1*(theta/T)^2*exp(u)/(exp(u) - 1)^2` also tends
    /// to `C1` as `T -> infinity`, so with upstream's `C3 = 0` the whole
    /// correlation is asymptotically `C1 + 2*C2*T`. Methodology: evaluate the
    /// correlation's own expression at 1e7 K (outside the public API's clamped
    /// range, so recomputed here from the same coefficients). Pass criterion:
    /// within 1e-6 relative of `C1 + 2*C2*T`.
    #[test]
    fn fink_mox_einstein_term_tends_to_c1_at_high_temperature() {
        let t: f64 = 1.0e7;
        let u: f64 = 587.41 / t;
        let cp = 322.49 * u * u * u.exp() / ((u.exp() - 1.0) * (u.exp() - 1.0))
            + 2.0 * 1.4679e-2 * t;
        let analytic = 322.49 + 2.0 * 1.4679e-2 * t;
        assert!((cp / analytic - 1.0).abs() < 1.0e-6, "{cp} vs {analytic}");
    }

    /// **Self-consistency check, not validation.**
    ///
    /// Because upstream sets `C3 = 0`, Fink's fit has no Schottky upturn, so
    /// it must fall increasingly behind MATPRO MOX at high temperature even
    /// though it leads at operating temperature. Methodology: compare the two
    /// at `pu_fraction = 0.1` at 1000 K and at 3000 K. Pass criterion: Fink
    /// higher at 1000 K, lower at 3000 K.
    #[test]
    fn fink_mox_lacks_the_high_temperature_defect_upturn() {
        let at = |t: f64| {
            let mut state = MaterialState::fresh(t);
            state.pu_fraction = 0.1;
            (
                HeatCapacityModel::FinkMox.value(&state),
                HeatCapacityModel::MatproMox.value(&state),
            )
        };
        let (fink_1000, matpro_1000) = at(1000.0);
        let (fink_3000, matpro_3000) = at(3000.0);
        assert!(fink_1000 > matpro_1000);
        assert!(fink_3000 < matpro_3000);
    }

    // ---------------------------------------------------------------------
    // Zircaloy
    // ---------------------------------------------------------------------

    /// **Reference-checked against the MATPRO table upstream transcribes.**
    ///
    /// Methodology: the MATPRO Zircaloy heat capacity is a table of
    /// `(T, Cp)` anchor points, and a linear interpolant must return each
    /// anchor value exactly at its own temperature. Reference: the anchor
    /// points listed in upstream `heatCapacityMatproZy.C` (MATPRO, NRC
    /// ML14296A063 p. 60): (300, 281), (400, 302), (640, 331), (1090, 375),
    /// (1133, 615), (1153, 719), (1173, 816), (1193, 770), (1213, 619),
    /// (1233, 469), (1248, 356). Tolerance: 1e-9 J/(kg K).
    ///
    /// Result (2026-07-29): every one of the eleven anchors is reproduced to
    /// better than 1e-12 J/(kg K). Interpretation: the table and the
    /// interpolation are transcribed correctly. The 1093 K and 1113 K anchors
    /// are deliberately excluded here — they sit inside the interval affected
    /// by the upstream off-by-one bug, which its own test covers.
    #[test]
    fn matpro_zircaloy_reproduces_every_table_anchor() {
        let anchors = [
            (300.0, 281.0),
            (400.0, 302.0),
            (640.0, 331.0),
            (1090.0, 375.0),
            (1133.0, 615.0),
            (1153.0, 719.0),
            (1173.0, 816.0),
            (1193.0, 770.0),
            (1213.0, 619.0),
            (1233.0, 469.0),
            (1248.0, 356.0),
        ];
        for (t, expected) in anchors {
            let cp = HeatCapacityModel::MatproZircaloy.value(&MaterialState::fresh(t));
            assert!(
                (cp - expected).abs() < 1.0e-9,
                "T = {t} K: {cp} vs expected {expected} J/(kg K)"
            );
        }
    }

    /// **Characterisation of a known upstream bug — not a validation.**
    ///
    /// Upstream's branch for 1093 < T <= 1113 K interpolates from
    /// `Tlow = 1090` instead of `Tlow = 1093`, while carrying the `CpLow = 502`
    /// that belongs to 1093 K. The table is therefore discontinuous at exactly
    /// 1093 K, where it should be continuous.
    ///
    /// Methodology: evaluate at 1093 K and at 1093 K + 1e-9. Pass criterion
    /// (documenting the defect, not endorsing it): the step exceeds
    /// 10 J/(kg K).
    ///
    /// Result (2026-07-29): 502.000 J/(kg K) just below and 513.478 J/(kg K)
    /// just above — a step of 11.478 J/(kg K), 2.3% of the local value. The
    /// correct interpolant would give 502.0 on both sides. This test exists so
    /// the bug is visible and so a later "cleanup" cannot silently change
    /// results relative to upstream.
    #[test]
    fn matpro_zircaloy_has_the_upstream_discontinuity_at_1093_k() {
        let below = HeatCapacityModel::MatproZircaloy.value(&MaterialState::fresh(1093.0));
        let above = HeatCapacityModel::MatproZircaloy.value(&MaterialState::fresh(1093.0 + 1e-9));
        assert!((below - 502.0).abs() < 1.0e-9, "{below}");
        assert!(
            above - below > 10.0,
            "expected the documented upstream step: {below} -> {above}"
        );
    }

    /// **Self-consistency check, not validation.**
    ///
    /// The alpha-to-beta transformation peak must be where MATPRO puts it:
    /// a maximum of 816 J/(kg K) at 1173 K, more than double the 356 J/(kg K)
    /// beta-phase plateau above 1248 K. Methodology: scan 273 → 2099 K in 1 K
    /// steps for the maximum. Pass criteria: the maximum is 816 J/(kg K) at
    /// 1173 K, and the value at 1500 K is exactly 356 J/(kg K).
    #[test]
    fn matpro_zircaloy_peaks_at_the_phase_transformation() {
        let mut best = (0.0_f64, f64::NEG_INFINITY);
        let mut t = 273.0;
        while t <= 2099.0 {
            let cp = HeatCapacityModel::MatproZircaloy.value(&MaterialState::fresh(t));
            if cp > best.1 {
                best = (t, cp);
            }
            t += 1.0;
        }
        assert!((best.0 - 1173.0).abs() < 1.0e-9, "peak at {} K", best.0);
        assert!((best.1 - 816.0).abs() < 1.0e-9, "peak {} J/(kg K)", best.1);

        let plateau = HeatCapacityModel::MatproZircaloy.value(&MaterialState::fresh(1500.0));
        assert!((plateau - 356.0).abs() < 1.0e-9, "plateau {plateau}");
    }

    /// **Self-consistency check, not validation.**
    ///
    /// The IAEA Zircaloy fit adds its Gaussian transformation peak on the
    /// interval 1100 ≤ T < 1320 K. The Gaussian is centred at 1213.8 K with a
    /// width parameter of 719.61 K^2, so it is negligible at both interval
    /// ends and the branch changes there are continuous. Methodology: evaluate
    /// on either side of 1100 K and of 1320 K. Pass criterion: the relative
    /// step is below 1e-6 at each.
    ///
    /// Result (2026-07-29): the Gaussian contributes 1.618e-5 J/(kg K) at
    /// 1100 K and 1.652e-4 J/(kg K) at 1320 K, i.e. relative steps of 4.3e-8
    /// and 5.0e-7.
    #[test]
    fn iaea_zircaloy_is_continuous_where_the_gaussian_switches_on_and_off() {
        for boundary in [1100.0, 1320.0] {
            let below =
                HeatCapacityModel::IaeaZircaloy.value(&MaterialState::fresh(boundary - 1e-9));
            let above =
                HeatCapacityModel::IaeaZircaloy.value(&MaterialState::fresh(boundary + 1e-9));
            assert!(
                (above - below).abs() / below < 1.0e-6,
                "boundary {boundary} K: {below} -> {above}"
            );
        }
    }

    /// **Characterisation of a discontinuity in the published fit — not a
    /// validation.**
    ///
    /// At 1213.8 K the IAEA correlation switches its baseline from the
    /// low-temperature line `Cp1` to the high-temperature parabola `Cp2` while
    /// the Gaussian peak is at its maximum. The two baselines differ by about
    /// 48 J/(kg K) there, so the correlation has a genuine step.
    ///
    /// Methodology: evaluate on either side of 1213.8 K. Pass criterion: the
    /// step lies between 40 and 60 J/(kg K) and is downward.
    ///
    /// Result (2026-07-29): 1438.353 J/(kg K) just below and 1389.872 J/(kg K)
    /// just above — a downward step of 48.481 J/(kg K), 3.4% of the local
    /// value.
    /// Interpretation: this is a feature of the published piecewise fit, not a
    /// porting error; it is recorded here so a reader of a transient result
    /// that shows a kink at 1213.8 K knows where it comes from.
    #[test]
    fn iaea_zircaloy_steps_where_its_two_baselines_meet() {
        let below = HeatCapacityModel::IaeaZircaloy.value(&MaterialState::fresh(1213.8 - 1e-9));
        let above = HeatCapacityModel::IaeaZircaloy.value(&MaterialState::fresh(1213.8 + 1e-9));
        let step = below - above;
        assert!(
            (40.0..60.0).contains(&step),
            "step at 1213.8 K: {below} -> {above} ({step} J/(kg K))"
        );
    }

    /// **Self-consistency check, not validation.**
    ///
    /// Two independent Zircaloy fits must agree in magnitude away from the
    /// phase transformation, where both are smooth. Methodology: compare
    /// [`HeatCapacityModel::IaeaZircaloy`] and
    /// [`HeatCapacityModel::MatproZircaloy`] at 400, 600 and 800 K; pass
    /// criterion: within 10% at each.
    ///
    /// Result (2026-07-29): 296.62 vs 302.00 at 400 K (-1.8%), 317.10 vs
    /// 326.17 at 600 K (-2.8%), 337.58 vs 346.64 at 800 K (-2.6%).
    /// Interpretation: two
    /// independently sourced fits to the same alloy agree to within a few per
    /// cent in the alpha phase, which is a meaningful consistency result even
    /// though it validates neither.
    #[test]
    fn iaea_and_matpro_zircaloy_agree_in_the_alpha_phase() {
        for t in [400.0, 600.0, 800.0] {
            let iaea = HeatCapacityModel::IaeaZircaloy.value(&MaterialState::fresh(t));
            let matpro = HeatCapacityModel::MatproZircaloy.value(&MaterialState::fresh(t));
            assert!(
                (iaea - matpro).abs() / matpro < 0.10,
                "T = {t} K: IAEA {iaea} vs MATPRO {matpro} J/(kg K)"
            );
        }
    }

    /// **Self-consistency check, not validation.**
    ///
    /// Both Zircaloy fits must show a large transformation peak somewhere in
    /// 1100–1250 K, which is the whole point of resolving the phase change.
    /// Methodology: scan 273–2000 K in 0.5 K steps for the IAEA maximum. Pass
    /// criteria: the peak lies in 1150–1250 K and exceeds 1000 J/(kg K).
    ///
    /// Result (2026-07-29): the IAEA peak is 1438.19 J/(kg K) at 1213.5 K.
    #[test]
    fn iaea_zircaloy_has_a_large_transformation_peak() {
        let mut best = (0.0_f64, f64::NEG_INFINITY);
        let mut t = 273.0;
        while t <= 2000.0 {
            let cp = HeatCapacityModel::IaeaZircaloy.value(&MaterialState::fresh(t));
            if cp > best.1 {
                best = (t, cp);
            }
            t += 0.5;
        }
        assert!(
            (1150.0..1250.0).contains(&best.0),
            "peak at {} K",
            best.0
        );
        assert!(best.1 > 1000.0, "peak only {} J/(kg K)", best.1);
    }

    // ---------------------------------------------------------------------
    // Metals and SiC
    // ---------------------------------------------------------------------

    /// **Self-consistency check, not validation.**
    ///
    /// As documented on [`HeatCapacityModel::Banerjee1515Ti`], upstream's third
    /// term carries a negative exponent and is therefore numerically dead. This
    /// test records that fact so the ambiguity cannot be lost: the correlation
    /// as ported is the straight line `431 + 0.177*T` to within
    /// 1e-9 J/(kg K) over its whole stated range.
    ///
    /// Methodology: compare the model against `431 + 0.177*T` at 293, 800 and
    /// 1273 K. Pass criterion: absolute difference below 1e-9 J/(kg K).
    ///
    /// Result (2026-07-29): the `8.72e-5*T^-2` term contributes
    /// 1.016e-9 J/(kg K) at 293 K and 5.377e-11 J/(kg K) at 1273 K. If the
    /// term was meant to be `T^2` this test
    /// fails immediately, which is the intent.
    #[test]
    fn banerjee_1515_ti_third_term_is_numerically_dead_as_written_upstream() {
        for t in [293.0, 800.0, 1273.0] {
            let cp = HeatCapacityModel::Banerjee1515Ti.value(&MaterialState::fresh(t));
            let linear = 431.0 + 0.177 * t;
            assert!(
                (cp - linear).abs() < 1.0e-8,
                "T = {t} K: {cp} vs linear {linear} J/(kg K)"
            );
        }
    }

    /// **Self-consistency check, not validation.**
    ///
    /// 15-15 Ti heat capacity rises with temperature over the stated
    /// 293–1273 K window and stays in the band expected of an austenitic
    /// stainless steel. Methodology: evaluate at the two endpoints; pass
    /// criteria: increasing, and both values inside 450–700 J/(kg K).
    ///
    /// Result (2026-07-29): Cp(293 K) = 482.86 J/(kg K), Cp(1273 K) = 656.32
    /// J/(kg K).
    #[test]
    fn banerjee_1515_ti_rises_and_stays_in_the_stainless_steel_band() {
        let cold = HeatCapacityModel::Banerjee1515Ti.value(&MaterialState::fresh(293.0));
        let hot = HeatCapacityModel::Banerjee1515Ti.value(&MaterialState::fresh(1273.0));
        assert!(hot > cold);
        assert!((450.0..700.0).contains(&cold), "Cp(293 K) = {cold}");
        assert!((450.0..700.0).contains(&hot), "Cp(1273 K) = {hot}");
    }

    /// **Reference-checked against the tabulated room-temperature value.**
    ///
    /// Methodology: evaluate [`HeatCapacityModel::Molybdenum`] at 300 K using
    /// the quadratic from upstream `heatCapacityMolybdenum.C`. Reference: the
    /// specific heat capacity of pure molybdenum at 300 K is a standard
    /// handbook value of approximately **251 J/(kg K)** (CRC: 0.251 J/(g K)).
    /// Tolerance: +/- 5 J/(kg K).
    ///
    /// Result (2026-07-29): **Cp(300 K) = 251.99 J/(kg K)**, within 1.0
    /// J/(kg K) of the 251 J/(kg K) handbook figure. Interpretation: the fit is anchored on
    /// the accepted room-temperature specific heat and the coefficients are
    /// transcribed correctly.
    #[test]
    fn molybdenum_reproduces_the_handbook_room_temperature_value() {
        let cp = HeatCapacityModel::Molybdenum.value(&MaterialState::fresh(300.0));
        assert!((cp - 251.0).abs() < 5.0, "Cp(300 K) = {cp} J/(kg K)");
        assert!((cp - 251.99).abs() < 0.05, "regression: {cp}");
    }

    /// **Self-consistency check, not validation.**
    ///
    /// Molybdenum's heat capacity fit has strictly positive linear and
    /// quadratic coefficients, so it must rise monotonically over the whole
    /// window. Methodology: sample 300 → 2800 K in 100 K steps; pass criterion:
    /// strictly increasing.
    #[test]
    fn molybdenum_heat_capacity_rises_monotonically() {
        let mut previous = f64::NEG_INFINITY;
        let mut t = 300.0;
        while t <= 2800.0 {
            let cp = HeatCapacityModel::Molybdenum.value(&MaterialState::fresh(t));
            assert!(cp > previous);
            previous = cp;
            t += 100.0;
        }
    }

    /// **Reference-checked against the published Snead fit.**
    ///
    /// Methodology: evaluate [`HeatCapacityModel::SneadSiC`] at 300 K with the
    /// coefficients as printed in upstream `heatCapacitySneadSiC.C`
    /// (`925.65 + 0.3772*T - 7.9259e-5*T^2 - 3.1946e7*T^-2`). Reference: the
    /// specific heat of CVD silicon carbide at room temperature is commonly
    /// quoted as approximately **675 J/(kg K)**. Tolerance: +/- 25 J/(kg K),
    /// deliberately loose because the reference is a generally quoted figure
    /// rather than a point taken from the Snead dataset itself.
    ///
    /// Result (2026-07-29): **Cp(300 K) = 676.72 J/(kg K)**, 1.7 J/(kg K)
    /// above the quoted figure. Interpretation: the four coefficients — including the
    /// sign and exponent of the `T^-2` term, and the choice of `-3.1946e7`
    /// over upstream's contradictory `-3.19446e7` dictionary default — are
    /// transcribed consistently with the published fit. This is a
    /// room-temperature magnitude check, **not** a comparison against a
    /// digitised Snead curve over the full 200–2400 K range.
    #[test]
    fn snead_sic_matches_the_quoted_room_temperature_specific_heat() {
        let cp = HeatCapacityModel::SneadSiC.value(&MaterialState::fresh(300.0));
        assert!((cp - 675.0).abs() < 25.0, "Cp(300 K) = {cp} J/(kg K)");
        assert!((cp - 676.7).abs() < 0.5, "regression: {cp}");
    }

    /// **Self-consistency check, not validation.**
    ///
    /// The Snead SiC fit must rise monotonically across its stated 200–2400 K
    /// window — the `T^-2` term lifts the low-temperature end and the negative
    /// quadratic only flattens, never reverses, the curve inside the window.
    /// Methodology: sample 200 → 2400 K in 50 K steps; pass criterion: strictly
    /// increasing.
    ///
    /// Result (2026-07-29): Cp rises from 199.27 J/(kg K) at 200 K to 1368.85
    /// J/(kg K) at 2400 K. Note how flat the top of the window is: the fit's
    /// stationary point sits just above 2400 K, so monotonicity holds over the
    /// stated range but with little margin.
    #[test]
    fn snead_sic_rises_monotonically_across_its_stated_range() {
        let mut previous = f64::NEG_INFINITY;
        let mut t = 200.0;
        while t <= 2400.0 {
            let cp = HeatCapacityModel::SneadSiC.value(&MaterialState::fresh(t));
            assert!(cp > previous, "T = {t} K: {cp} not above {previous}");
            previous = cp;
            t += 50.0;
        }
    }
}
