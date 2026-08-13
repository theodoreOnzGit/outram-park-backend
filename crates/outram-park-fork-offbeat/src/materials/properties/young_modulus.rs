// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
// `offbeatLib/materials/materialModel/thermoMechanicalPropertiesModels/YoungModulus/`,
// specifically the files:
//   YoungModulusConstant.{C,H}          -> YoungModulusModel::Constant
//   YoungModulusMatproUO2.{C,H}         -> YoungModulusModel::MatproUo2
//   YoungModulusMatproUPuO2.{C,H}       -> YoungModulusModel::MatproMox
//   YoungModulusSckCenUPuO2.{C,H}       -> YoungModulusModel::SckCenMox
//   YoungModulusMatproZy.{C,H}          -> YoungModulusModel::MatproZircaloy
//   YoungModulusHofmanD9.{C,H}          -> YoungModulusModel::HofmanD9
//   YoungModulusMolybdenum.{C,H}        -> YoungModulusModel::BisonMolybdenum
//   YoungModulusTobbe1515Ti.{C,H}       -> YoungModulusModel::Tobbe1515Ti
//   YoungModulusWatrousHastelloyN.{C,H} -> YoungModulusModel::WatrousHastelloyN
//   YoungModulusSneadSiC.{C,H}          -> YoungModulusModel::SneadSiC
//   YoungModulusPARFUMEBuffer.{C,H}     -> YoungModulusModel::ParfumeBuffer
//   YoungModulusPARFUMEPyC.{C,H}        -> YoungModulusModel::ParfumePyC
//   YoungModulusPARFUMESiC.{C,H}        -> YoungModulusModel::ParfumeSiC
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Young's modulus correlations \[Pa\].
//!
//! # What this module computes
//!
//! Young's modulus `E` — the elastic (uniaxial) stiffness of a fuel, cladding,
//! structural or TRISO-coating material — as a pure function of the local
//! [`MaterialState`]: temperature, porosity, plutonium content, deviation from
//! stoichiometry, fast-neutron fluence and cold work. The returned quantity is
//! **always in pascals (Pa)**, never GPa or MPa, even though most of the
//! published fits are quoted in GPa or MPa; the unit conversion is done inside
//! each variant.
//!
//! # Why the mechanics solve needs it
//!
//! `E` alone is not what an isotropic-elasticity momentum equation consumes.
//! Together with Poisson's ratio `nu` from
//! [`poisson_ratio`](crate::materials::properties::poisson_ratio) it forms the
//! two Lame parameters:
//!
//! $$ \mu = \frac{E}{2(1 + \nu)} $$
//!
//! $$ \lambda = \frac{E \nu}{(1 + \nu)(1 - 2\nu)} $$
//!
//! `mu` is the shear modulus and `lambda` the first Lame parameter. **This
//! module does not build them and does not solve anything** — assembling the
//! Lame parameters and the momentum equation belongs to
//! [`crate::mechanics`]. What lives here is only the property lookup.
//!
//! Note the `1 - 2*nu` in the denominator of `lambda`: it is why the companion
//! module cares whether a correlation can return `nu >= 0.5`, and why the
//! thermodynamic admissibility of `nu` is tested there rather than assumed.
//!
//! # Units — raw `f64`, strict SI
//!
//! Like [`MaterialState`], this module carries raw `f64` in strict SI rather
//! than `uom` quantities, because it is evaluated once per cell per property
//! per timestep inside the numerical loops. Inputs are kelvin, n/m^2 and
//! dimensionless fractions; the output is pascals. Correlations whose published
//! form is in degrees Celsius (the PARFUME set, Hofman D9, Tobbe 15-15 Ti,
//! Watrous Hastelloy N) convert internally — a caller never passes Celsius.
//!
//! # Validity ranges, clamping and checking
//!
//! Every variant declares a temperature range with
//! [`YoungModulusModel::temperature_range`]. Two evaluation entry points:
//!
//! - [`value`](YoungModulusModel::value) **clamps** the inputs to the range
//!   endpoints and always returns a number. This is the one the solver loop
//!   calls, and it matches the spirit of upstream, which prints a warning and
//!   carries on.
//! - [`value_checked`](YoungModulusModel::value_checked) returns
//!   [`OffbeatError::OutOfRange`] instead of extrapolating. Use it when setting
//!   a case up, to learn that the correlation does not cover the conditions
//!   asked of it.
//!
//! Some ranges are stated by upstream (it emits an explicit warning outside
//! them); the rest are **port-imposed** and are labelled as such on the variant
//! that owns them. A port-imposed bound is a convention of this crate, not a
//! number taken from the cited report.
//!
//! # Known divergences from upstream
//!
//! Recorded here rather than buried, because a port that silently "improves"
//! its source is not a port:
//!
//! 1. **Isotropic cracking is not implemented here.** Upstream's UO2, MOX and
//!    SCK-CEN variants optionally multiply `E` by a crack-softening factor
//!    driven by a `nCracks` field, a `sliceMapper` and the linear heat rate.
//!    That is damage-model state, not a pure function of [`MaterialState`], so
//!    it belongs with the damage model, not here. All variants below return the
//!    upstream **nominal** (uncracked) value.
//! 2. **`WatrousHastelloyN` returns a finite modulus above 1273.15 K**, where
//!    upstream leaves the field at its initialised `0.0`. A zero Young's
//!    modulus makes the stiffness matrix singular; this port clamps to the
//!    range endpoint instead. See the variant docs.
//! 3. **Fast fluence is in n/m^2 throughout.** Upstream's `MatproZy` variant
//!    multiplies the stored fluence field by `1e4` (i.e. reads n/cm^2) while
//!    its companion Poisson-ratio model does not — an internal inconsistency.
//!    This port takes [`MaterialState::fast_fluence`] in n/m^2 in both places.
//!
//! [`MaterialState::fast_fluence`]: crate::materials::MaterialState::fast_fluence

use crate::error::{OffbeatError, Result};
use crate::materials::MaterialState;

/// Upper porosity bound \[-\] for the MATPRO-family fuel fits.
///
/// The MATPRO porosity correction is linear, `1 - 2.752 * P`, so it predicts
/// **zero** stiffness at `P = 1/2.752 = 0.3634` and negative stiffness above
/// that. Upstream's SCK-CEN variant already guards against this by clamping
/// porosity with `min(porosity, 0.3)`; this port applies the same 0.3 bound to
/// the MATPRO UO2 and MOX variants, which upstream leaves unguarded.
pub const MATPRO_MAX_POROSITY: f64 = 0.3;

/// Saturation fast fluence \[n/m^2\] of the PARFUME coating-layer fits.
///
/// PARFUME's buffer and PyC modulus correlations are evaluated at this fluence
/// for any larger fluence — the irradiation term `1 + 0.23 * phi` saturates
/// rather than growing without bound. Upstream applies the same cutoff
/// (`min(phi, 3.96)` with `phi` in units of `1e25` n/m^2), for fast neutrons
/// with E > 0.18 MeV.
pub const PARFUME_SATURATION_FLUENCE: f64 = 3.96e25;

/// Young's modulus `E` \[Pa\] of a fuel, cladding, structural or TRISO-coating
/// material.
///
/// # What it is
///
/// The elastic stiffness in uniaxial tension: the slope of the stress-strain
/// curve at zero strain. Every variant is one published correlation, named for
/// the **author or data source of the fit plus the material** — that is how the
/// fuel-performance literature identifies these, and two "UO2 Young's modulus"
/// correlations can differ by tens of percent.
///
/// # Dispatch
///
/// An enum, not a trait object: the set of correlations is closed and known at
/// compile time, so adding one is a compile error at every `match` site rather
/// than a runtime surprise, and go-to-definition works on the variants. See the
/// workspace `CLAUDE.md` "No trait objects" rule.
///
/// # Example
///
/// ```
/// use outram_park_fork_offbeat::materials::MaterialState;
/// use outram_park_fork_offbeat::materials::properties::young_modulus::YoungModulusModel;
///
/// // Fully dense UO2 at room temperature, MATPRO-11 correlation.
/// let state = MaterialState::fresh(300.0);
/// let e = YoungModulusModel::MatproUo2.value(&state);
/// assert!((e - 2.25757317e11).abs() < 1.0e4); // ~225.8 GPa
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum YoungModulusModel {
    /// A user-supplied constant Young's modulus \[Pa\], independent of state.
    ///
    /// Upstream: `YoungModulusConstant`, which reads the value from the
    /// material dictionary as `E`. Use it for a material this port has no
    /// correlation for, or to isolate a mechanics test from property
    /// variation.
    ///
    /// **Valid range:** none — the value is returned unchanged at any
    /// temperature. The payload should be positive; a non-positive modulus is
    /// reported by [`value_checked`](Self::value_checked) as
    /// [`OffbeatError::Unphysical`].
    Constant(f64),

    /// UO2 fuel, MATPRO-11 correlation.
    ///
    /// Upstream: `YoungModulusMatproUO2`.
    ///
    /// ```text
    /// E = 2.334e11 * (1 - 2.752 * P) * (1 - 1.0915e-4 * T)     [Pa]
    /// ```
    ///
    /// with `P` the porosity \[-\] and `T` the temperature \[K\]. The first
    /// factor is the porosity knock-down, the second the linear thermal
    /// softening.
    ///
    /// **Inputs used:** [`temperature`](MaterialState::temperature),
    /// [`porosity`](MaterialState::porosity).
    ///
    /// **Valid range:** temperature 300 K to 3113 K (room temperature to the
    /// UO2 melting point) and porosity 0 to [`MATPRO_MAX_POROSITY`]. Both are
    /// **port-imposed**: upstream performs no range check on this correlation.
    ///
    /// **Source:** MATPRO-11 (Rev. 2), as transcribed in OFFBEAT.
    MatproUo2,

    /// (U,Pu)O2 MOX fuel, MATPRO-11 correlation with plutonium and
    /// stoichiometry corrections.
    ///
    /// Upstream: `YoungModulusMatproUPuO2`.
    ///
    /// ```text
    /// E = E_UO2(T, P) * exp(-B * x) * (1 + 0.05 * c_Pu)        [Pa]
    /// ```
    ///
    /// where `E_UO2(T, P)` is exactly [`MatproUo2`](Self::MatproUo2),
    /// `x = 2 - O/M` is upstream's deviation-from-stoichiometry variable, and
    /// `c_Pu` is the plutonium **mass** fraction of the fuel, obtained from the
    /// atom fraction by the approximate conversion `c_Pu = at_Pu / 1.13` that
    /// upstream derives for `MM_Pu ~ 239 g/mol`, `MM_HM ~ 238.5 g/mol`,
    /// `O/M = 2`.
    ///
    /// `B = 1.35` for `x >= 0` and `B = 1.75` for `x < 0`. Because this port
    /// stores the deviation as the `x` of `(U,Pu)O_{2+x}`
    /// ([`oxygen_deviation`](MaterialState::oxygen_deviation)), which is the
    /// **negative** of upstream's variable, hypostoichiometric fuel
    /// (`O/M < 2`, `oxygen_deviation < 0`) takes `B = 1.35` and softens, which
    /// is the normal fast-reactor MOX case.
    ///
    /// **Upstream quirk, ported faithfully:** for hyperstoichiometric fuel
    /// (`O/M > 2`) upstream's `exp(-B*x)` has a positive exponent and therefore
    /// *stiffens* the fuel; and upstream's comments label the two branches
    /// "hypostoichiometric"/"hyperstoichiometric" the opposite way round from
    /// what its own algebra does. The algebra is reproduced here, not the
    /// comments.
    ///
    /// **Inputs used:** [`temperature`](MaterialState::temperature),
    /// [`porosity`](MaterialState::porosity),
    /// [`pu_fraction`](MaterialState::pu_fraction),
    /// [`oxygen_deviation`](MaterialState::oxygen_deviation).
    ///
    /// **Valid range:** temperature 300 K to 3023 K (room temperature to the
    /// approximate MOX melting point), porosity 0 to [`MATPRO_MAX_POROSITY`].
    /// Both **port-imposed**; upstream performs no range check.
    ///
    /// **Source:** MATPRO-11 (Rev. 2), as transcribed in OFFBEAT.
    MatproMox,

    /// (U,Pu)O2 MOX fuel, SCK-CEN correlation developed for TRANSURANUS.
    ///
    /// Upstream: `YoungModulusSckCenUPuO2`.
    ///
    /// A mixture rule between the two end-member moduli, a stoichiometry
    /// correction with two slopes, a quadratic temperature shape normalised at
    /// 273 K, and a porosity knock-down:
    ///
    /// ```text
    /// E_mix = (1 - c_Pu) * 218.74 + c_Pu * 249.45              [GPa]
    /// E_y   = E_mix - 586 * y                       for 0 <= y <= 0.037
    /// E_y   = E_mix - 586 * 0.037 - 126.59*(y - 0.037)  for y > 0.037
    /// E_y   = E_mix                                 for y < 0
    /// f(T)  = 219.12 - 0.0154 * T - 9.0e-6 * T^2
    /// E     = E_y * f(T)/f(273) * (1 - P)^2 / (1 + 1.1 * P) * 1e9   [Pa]
    /// ```
    ///
    /// with `y = 2 - O/M` (again the negative of
    /// [`oxygen_deviation`](MaterialState::oxygen_deviation)) and `P` the
    /// porosity, clamped at 0.3 by upstream itself "to prevent YM decreasing
    /// too much".
    ///
    /// **Inputs used:** [`temperature`](MaterialState::temperature),
    /// [`porosity`](MaterialState::porosity),
    /// [`pu_fraction`](MaterialState::pu_fraction),
    /// [`oxygen_deviation`](MaterialState::oxygen_deviation).
    ///
    /// **Valid range:** temperature 273 K to 3023 K, **port-imposed** — the
    /// correlation is normalised at 273 K and upstream checks nothing.
    ///
    /// **Source:** INSPYRE project deliverable D7.2 (2020), SCK-CEN correlation
    /// for TRANSURANUS; the URL is given in upstream's
    /// `YoungModulusSckCenUPuO2.H`.
    SckCenMox,

    /// Zircaloy cladding, MATPRO-11 correlation with alpha/beta phase branches.
    ///
    /// Upstream: `YoungModulusMatproZy`.
    ///
    /// Three temperature regimes, with oxygen, cold-work and fast-fluence
    /// corrections in the alpha phase:
    ///
    /// ```text
    /// K1 = (6.61e11 + 5.912e8 * T) * C_ox        oxygen effect
    /// K2 = -2.6e10 * C_cw                        cold-work effect
    /// K3 = 0.88 + 0.12 * exp(-phi / 1e25)        fast-fluence effect
    ///
    /// alpha (T < 1073 K):  E = (1.088e11 - 5.475e7 * T + K1 + K2) / K3
    /// beta  (T >= 1273 K): E = 9.21e10 - 4.05e7 * T
    /// 1073 <= T < 1273 K:  linear interpolation between the two, with the
    ///                      alpha value taken at 1073 K and the beta value at
    ///                      1273 K
    /// ```
    ///
    /// Note the sign of the fluence term: `K3` **decreases** towards 0.88 with
    /// accumulated fluence and divides the numerator, so irradiation
    /// *stiffens* the cladding by up to a factor `1/0.88 = 1.136`. That is
    /// irradiation hardening, and it is the correct direction for this fit.
    ///
    /// **Inputs used:** [`temperature`](MaterialState::temperature),
    /// [`fast_fluence`](MaterialState::fast_fluence) \[n/m^2\],
    /// [`cold_work`](MaterialState::cold_work),
    /// [`oxygen_content`](MaterialState::oxygen_content) \[weight fraction\].
    ///
    /// **Valid range:** 290 K to 1800 K — **stated by upstream**, which emits
    /// a warning outside it (with a one-degree slack on each side for rounding).
    ///
    /// **Source:** MATPRO-11 (Rev. 2), as transcribed in OFFBEAT.
    MatproZircaloy,

    /// D9 austenitic stainless-steel cladding, Hofman correlation.
    ///
    /// Upstream: `YoungModulusHofmanD9`.
    ///
    /// ```text
    /// E = (2.01e5 - 79.29 * T_C) * 1e6                         [Pa]
    /// ```
    ///
    /// with `T_C` the temperature in **degrees Celsius** (`T - 273.15`); the
    /// bracket is in MPa.
    ///
    /// **Inputs used:** [`temperature`](MaterialState::temperature).
    ///
    /// **Valid range:** 293 K to 1273 K — **stated by upstream**, which warns
    /// outside it.
    ///
    /// **Source:** Hofman correlation for D9, as transcribed in OFFBEAT.
    HofmanD9,

    /// Molybdenum structural material, correlation from the BISON manual.
    ///
    /// Upstream: `YoungModulusMolybdenum`.
    ///
    /// ```text
    /// E = 3.349e11 - 5.101e7 * T                               [Pa]
    /// ```
    ///
    /// **Inputs used:** [`temperature`](MaterialState::temperature).
    ///
    /// **Valid range:** 300 K to 2896 K (room temperature to the melting point
    /// of molybdenum) — **port-imposed**; upstream checks nothing.
    ///
    /// **Source:** BISON manual, as named in upstream's
    /// `YoungModulusMolybdenum.H`.
    BisonMolybdenum,

    /// 15-15 Ti austenitic stainless-steel cladding, Tobbe correlation (1975).
    ///
    /// Upstream: `YoungModulusTobbe1515Ti`.
    ///
    /// ```text
    /// E = (202.7 - 0.08167 * T_C) * 1e9                        [Pa]
    /// ```
    ///
    /// with `T_C` in **degrees Celsius**; the bracket is in GPa.
    ///
    /// **Inputs used:** [`temperature`](MaterialState::temperature).
    ///
    /// **Valid range:** 293 K to 1273 K — **stated by upstream**, which warns
    /// outside it.
    ///
    /// **Source:** Tobbe (1975), as named in upstream's
    /// `YoungModulusTobbe1515Ti.H`.
    Tobbe1515Ti,

    /// Hastelloy N structural alloy, Watrous correlation.
    ///
    /// Upstream: `YoungModulusWatrousHastelloyN`.
    ///
    /// A cubic in degrees Celsius:
    ///
    /// ```text
    /// E = (-9.944e-8 * T_C^3 + 1.178e-4 * T_C^2
    ///      - 0.1033 * T_C + 220.9) * 1e9                       [Pa]
    /// ```
    ///
    /// **Inputs used:** [`temperature`](MaterialState::temperature).
    ///
    /// **Valid range:** upper bound 1273.15 K (1000 C) — **stated by
    /// upstream**. The lower bound of 293.15 K is **port-imposed**.
    ///
    /// **Divergence from upstream:** above 1273.15 K upstream warns and leaves
    /// the modulus at its initialised value of **zero**, which would make the
    /// mechanics stiffness matrix singular. This port clamps to the 1273.15 K
    /// endpoint instead, and [`value_checked`](Self::value_checked) reports
    /// [`OffbeatError::OutOfRange`] there.
    ///
    /// **Source:** Watrous, as named in upstream's
    /// `YoungModulusWatrousHastelloyN.H`.
    WatrousHastelloyN,

    /// CVD silicon carbide, Snead et al. (2007) handbook correlation.
    ///
    /// Upstream: `YoungModulusSneadSiC`.
    ///
    /// ```text
    /// E = 460e9 * exp(-C * P) - 0.04e9 * T * exp(-962 / T)     [Pa]
    /// ```
    ///
    /// The second term is the thermal softening (about -0.5 GPa at 300 K,
    /// growing with temperature); the first is the room-temperature modulus of
    /// fully dense CVD SiC with an exponential porosity knock-down.
    ///
    /// **Inputs used:** [`temperature`](MaterialState::temperature),
    /// [`porosity`](MaterialState::porosity).
    ///
    /// **Valid range:** 300 K to 1873 K — **port-imposed**; upstream states no
    /// range. Treat the bound as a convention of this crate, not a number taken
    /// from Snead et al.
    ///
    /// **Source:** L. L. Snead, T. Nozawa, Y. Katoh, T.-S. Byun, S. Kondo,
    /// D. A. Petti, "Handbook of SiC properties for fuel performance
    /// modeling", *Journal of Nuclear Materials* **371** (2007) 329-377.
    SneadSiC {
        /// Exponential porosity-knock-down coefficient `C` \[-\] in
        /// `exp(-C * P)`.
        ///
        /// Upstream's dictionary default is **0.0**, i.e. no porosity
        /// dependence at all; pass 0.0 to reproduce upstream's default
        /// behaviour exactly. Values around 3-4 are typical when an
        /// exponential porosity correction is wanted for CVD SiC.
        porosity_coefficient: f64,
    },

    /// TRISO buffer layer (porous pyrolytic carbon), PARFUME correlation.
    ///
    /// Upstream: `YoungModulusPARFUMEBuffer`.
    ///
    /// ```text
    /// E = 25.5 * (0.384 + 0.324e-3 * rho)
    ///          * (1 + 0.23 * phi25)
    ///          * (1 + 1.5e-4 * (T_C - 20)) * 1e9               [Pa]
    /// ```
    ///
    /// with `rho` the buffer density \[kg/m^3\], `T_C` the temperature in
    /// degrees Celsius, and `phi25` the fast fluence in units of `1e25` n/m^2
    /// (E > 0.18 MeV), saturated at 3.96 — see
    /// [`PARFUME_SATURATION_FLUENCE`].
    ///
    /// The irradiation term *increases* the modulus: pyrolytic carbon
    /// densifies and stiffens under fast-neutron damage.
    ///
    /// **Inputs used:** [`temperature`](MaterialState::temperature),
    /// [`fast_fluence`](MaterialState::fast_fluence); the density is carried on
    /// the variant because [`MaterialState`] has no density field (upstream
    /// looks up the `rho` mesh field).
    ///
    /// **Valid range:** 293.15 K to 2273.15 K — **port-imposed**, covering
    /// normal TRISO operation through accident temperatures; upstream states no
    /// temperature range. Fluence is saturated rather than rejected, matching
    /// upstream.
    ///
    /// **Source:** PARFUME (INL TRISO fuel-performance code) material models,
    /// as transcribed in OFFBEAT.
    ParfumeBuffer {
        /// As-fabricated buffer density \[kg/m^3\]. Typically about
        /// 1000 kg/m^3, i.e. roughly half the density of dense pyrolytic
        /// carbon.
        density: f64,
    },

    /// TRISO IPyC/OPyC dense pyrolytic-carbon layer, PARFUME correlation.
    ///
    /// Upstream: `YoungModulusPARFUMEPyC`.
    ///
    /// Radial and tangential components, then the isotropic average upstream
    /// actually returns:
    ///
    /// ```text
    /// c   = 25.5 * (0.384 + 0.324e-3 * rho) * (2.985 - 0.0662 * Lc)
    ///           * (1 + 0.23 * phi25) * (1 + 1.5e-4 * (T_C - 20))
    /// E_r = c * (1.463 - 0.463 * BAF)
    /// E_t = c * (0.481 + 0.519 * BAF)
    /// E   = (E_r + 2 * E_t) / 3 * 1e9                          [Pa]
    /// ```
    ///
    /// **Upstream's own note, kept:** PyC is properly transversely isotropic,
    /// with different radial and tangential moduli. Upstream returns the
    /// isotropic average as a temporary measure, observing that for the usual
    /// TRISO defaults `BAF = 1.0` and `Lc = 30` the two components are
    /// identical anyway (`1.463 - 0.463 = 0.481 + 0.519 = 1`). This port
    /// reproduces that; a transversely isotropic PyC would need the mechanics
    /// layer to accept a direction-dependent modulus.
    ///
    /// **Inputs used:** [`temperature`](MaterialState::temperature),
    /// [`fast_fluence`](MaterialState::fast_fluence); density, anisotropy and
    /// crystallite size are carried on the variant because [`MaterialState`]
    /// has no field for them.
    ///
    /// **Valid range:** temperature 293.15 K to 2273.15 K (**port-imposed**);
    /// density 1800 to 2000 kg/m^3 and `BAF >= 1.0` (**stated by upstream**,
    /// which warns outside them). Fluence saturates at
    /// [`PARFUME_SATURATION_FLUENCE`].
    ///
    /// **Source:** PARFUME (INL TRISO fuel-performance code) material models,
    /// as transcribed in OFFBEAT.
    ParfumePyC {
        /// As-fabricated PyC density \[kg/m^3\]. The fit's data range is
        /// 1800-2000 kg/m^3.
        density: f64,
        /// As-fabricated Bacon Anisotropy Factor (BAF) \[-\], a measure of
        /// preferred crystallite orientation. `1.0` is fully isotropic; the fit
        /// requires `BAF >= 1.0`.
        bacon_anisotropy_factor: f64,
        /// Crystallite diameter `Lc` \[nm\]. The usual TRISO default is 30.
        ///
        /// Note that the factor `2.985 - 0.0662 * Lc` reaches zero at
        /// `Lc = 45.1` nm and turns negative beyond, so this parameter is not
        /// meaningfully extrapolable.
        crystallite_diameter: f64,
    },

    /// TRISO SiC layer, PARFUME piecewise-linear interpolation.
    ///
    /// Upstream: `YoungModulusPARFUMESiC`.
    ///
    /// Linear interpolation in **degrees Celsius** between four tabulated
    /// points, clamped to the end values outside:
    ///
    /// | `T_C` \[C\] | `E` \[GPa\] |
    /// |---|---|
    /// | 25 | 428 |
    /// | 940 | 375 |
    /// | 1215 | 340 |
    /// | 1600 | 198 |
    ///
    /// The steep drop over the last interval (375 to 198 GPa between 940 C and
    /// 1600 C) is the high-temperature softening of the SiC layer that governs
    /// TRISO particle failure in accident conditions.
    ///
    /// **Inputs used:** [`temperature`](MaterialState::temperature).
    ///
    /// **Valid range:** 298.15 K to 1873.15 K (25 C to 1600 C), the span of the
    /// table — **stated by the table itself**; upstream clamps to the end
    /// values outside it, and so does [`value`](Self::value).
    ///
    /// **Source:** PARFUME (INL TRISO fuel-performance code) material models,
    /// as transcribed in OFFBEAT.
    ParfumeSiC,
}

impl YoungModulusModel {
    /// Human-readable name of the correlation, used in error messages.
    ///
    /// Stable enough to match on in a log, but not a serialisation format —
    /// use the enum itself for that.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Constant(_) => "constant Young's modulus",
            Self::MatproUo2 => "MATPRO UO2 Young's modulus",
            Self::MatproMox => "MATPRO (U,Pu)O2 Young's modulus",
            Self::SckCenMox => "SCK-CEN (U,Pu)O2 Young's modulus",
            Self::MatproZircaloy => "MATPRO Zircaloy Young's modulus",
            Self::HofmanD9 => "Hofman D9 Young's modulus",
            Self::BisonMolybdenum => "BISON molybdenum Young's modulus",
            Self::Tobbe1515Ti => "Tobbe 15-15 Ti Young's modulus",
            Self::WatrousHastelloyN => "Watrous Hastelloy N Young's modulus",
            Self::SneadSiC { .. } => "Snead SiC Young's modulus",
            Self::ParfumeBuffer { .. } => "PARFUME buffer Young's modulus",
            Self::ParfumePyC { .. } => "PARFUME PyC Young's modulus",
            Self::ParfumeSiC => "PARFUME SiC Young's modulus",
        }
    }

    /// Temperature validity range `(low, high)` \[K\] of this correlation.
    ///
    /// Where the bound is stated by upstream it is reproduced exactly; where it
    /// is port-imposed the variant's own documentation says so. A
    /// [`Constant`](Self::Constant) modulus is valid everywhere and reports the
    /// whole positive axis up to a nominal 1e5 K.
    #[must_use]
    pub fn temperature_range(&self) -> (f64, f64) {
        match self {
            Self::Constant(_) => (0.0, 1.0e5),
            Self::MatproUo2 => (300.0, 3113.0),
            Self::MatproMox => (300.0, 3023.0),
            Self::SckCenMox => (273.0, 3023.0),
            Self::MatproZircaloy => (290.0, 1800.0),
            Self::HofmanD9 | Self::Tobbe1515Ti => (293.0, 1273.0),
            Self::BisonMolybdenum => (300.0, 2896.0),
            Self::WatrousHastelloyN => (293.15, 1273.15),
            Self::SneadSiC { .. } => (300.0, 1873.0),
            Self::ParfumeBuffer { .. } | Self::ParfumePyC { .. } => (293.15, 2273.15),
            Self::ParfumeSiC => (298.15, 1873.15),
        }
    }

    /// Young's modulus \[Pa\] at the given state, **clamping** out-of-range
    /// inputs to the endpoints of the validity range.
    ///
    /// # Clamping — read this before trusting a number
    ///
    /// This method never fails and never extrapolates. Specifically:
    ///
    /// - **Temperature** is clamped into
    ///   [`temperature_range`](Self::temperature_range). A call at 4000 K on a
    ///   UO2 correlation returns the 3113 K value, not a melt-region
    ///   extrapolation.
    /// - **Porosity** is clamped into `[0, `[`MATPRO_MAX_POROSITY`]`]` for the
    ///   MATPRO and SCK-CEN fuel variants, where the linear porosity correction
    ///   would otherwise reach zero and go negative.
    /// - **Fast fluence** is clamped to [`PARFUME_SATURATION_FLUENCE`] for the
    ///   PARFUME buffer and PyC variants — this one is part of the correlation
    ///   itself, not a port convention, and upstream does the same.
    /// - **Density and BAF** on the PARFUME variants are *not* clamped: those
    ///   fits are linear and well-behaved outside their data range, and
    ///   upstream only warns. [`value_checked`](Self::value_checked) still
    ///   reports them.
    ///
    /// Clamping is the behaviour a solver loop wants (upstream warns and
    /// carries on). When you need to know that the correlation does not cover
    /// your conditions, call [`value_checked`](Self::value_checked) instead.
    ///
    /// # Example
    ///
    /// ```
    /// use outram_park_fork_offbeat::materials::MaterialState;
    /// use outram_park_fork_offbeat::materials::properties::young_modulus::YoungModulusModel;
    ///
    /// let model = YoungModulusModel::Tobbe1515Ti;   // valid 293-1273 K
    /// let hot = MaterialState::fresh(2000.0);       // far above the range
    /// let edge = MaterialState::fresh(1273.0);
    /// assert_eq!(model.value(&hot), model.value(&edge)); // clamped
    /// ```
    #[must_use]
    pub fn value(&self, state: &MaterialState) -> f64 {
        let (low, high) = self.temperature_range();
        let temperature = state.temperature.clamp(low, high);
        self.evaluate(temperature, state)
    }

    /// Young's modulus \[Pa\] at the given state, or
    /// [`OffbeatError::OutOfRange`] if the correlation was not fitted there.
    ///
    /// Unlike [`value`](Self::value) this method does not clamp and does not
    /// extrapolate; it refuses. Checks performed, in order:
    ///
    /// 1. Temperature is positive — otherwise [`OffbeatError::Unphysical`].
    /// 2. Temperature lies in [`temperature_range`](Self::temperature_range).
    /// 3. For the MATPRO/SCK-CEN fuel variants, porosity lies in
    ///    `[0, `[`MATPRO_MAX_POROSITY`]`]`.
    /// 4. For [`ParfumePyC`](Self::ParfumePyC), density lies in
    ///    `[1800, 2000]` kg/m^3 and `BAF >= 1.0` — upstream's own warnings.
    /// 5. For [`Constant`](Self::Constant), the payload is positive.
    ///
    /// Fast fluence above [`PARFUME_SATURATION_FLUENCE`] is **not** an error:
    /// the PARFUME fits are defined to saturate there.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::OutOfRange`] when an input is outside the fit's stated
    /// validity range, [`OffbeatError::Unphysical`] for a non-positive
    /// temperature or a non-positive constant modulus.
    ///
    /// # Example
    ///
    /// ```
    /// use outram_park_fork_offbeat::materials::MaterialState;
    /// use outram_park_fork_offbeat::materials::properties::young_modulus::YoungModulusModel;
    ///
    /// let model = YoungModulusModel::MatproZircaloy;      // valid 290-1800 K
    /// assert!(model.value_checked(&MaterialState::fresh(600.0)).is_ok());
    /// assert!(model.value_checked(&MaterialState::fresh(2500.0)).is_err());
    /// ```
    pub fn value_checked(&self, state: &MaterialState) -> Result<f64> {
        if !state.temperature.is_finite() || state.temperature <= 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: self.name(),
                value: state.temperature,
                unit: "K",
                reason: "absolute temperature must be strictly positive",
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

        match *self {
            Self::Constant(e) => {
                if !e.is_finite() || e <= 0.0 {
                    return Err(OffbeatError::Unphysical {
                        quantity: self.name(),
                        value: e,
                        unit: "Pa",
                        reason: "Young's modulus must be strictly positive",
                    });
                }
            }
            Self::MatproUo2 | Self::MatproMox | Self::SckCenMox => {
                if state.porosity < 0.0 || state.porosity > MATPRO_MAX_POROSITY {
                    return Err(OffbeatError::OutOfRange {
                        quantity: self.name(),
                        value: state.porosity,
                        low: 0.0,
                        high: MATPRO_MAX_POROSITY,
                        unit: "-",
                    });
                }
            }
            Self::ParfumePyC {
                density,
                bacon_anisotropy_factor,
                ..
            } => {
                if !(1800.0..=2000.0).contains(&density) {
                    return Err(OffbeatError::OutOfRange {
                        quantity: "PARFUME PyC Young's modulus (density)",
                        value: density,
                        low: 1800.0,
                        high: 2000.0,
                        unit: "kg/m^3",
                    });
                }
                if bacon_anisotropy_factor < 1.0 {
                    return Err(OffbeatError::OutOfRange {
                        quantity: "PARFUME PyC Young's modulus (BAF)",
                        value: bacon_anisotropy_factor,
                        low: 1.0,
                        high: f64::INFINITY,
                        unit: "-",
                    });
                }
            }
            _ => {}
        }

        Ok(self.evaluate(state.temperature, state))
    }

    /// Evaluate the correlation at an already-validated (or already-clamped)
    /// temperature \[K\].
    ///
    /// Split out so that [`value`](Self::value) and
    /// [`value_checked`](Self::value_checked) share exactly one copy of the
    /// physics, and so the companion Poisson-ratio module can evaluate the
    /// Zircaloy modulus at a temperature it has clamped itself.
    fn evaluate(&self, temperature: f64, state: &MaterialState) -> f64 {
        match *self {
            Self::Constant(e) => e,

            Self::MatproUo2 => matpro_uo2(temperature, state.porosity),

            Self::MatproMox => {
                let e_uo2 = matpro_uo2(temperature, state.porosity);
                // Upstream's variable is x = 2 - O/M, the negative of the `x`
                // in (U,Pu)O_{2+x} that MaterialState stores.
                let x = -state.oxygen_deviation;
                let b = if x < 0.0 { 1.75 } else { 1.35 };
                let c_w_pu = state.pu_fraction / PU_ATOM_TO_MASS_FRACTION;
                e_uo2 * (-b * x).exp() * (1.0 + 0.05 * c_w_pu)
            }

            Self::SckCenMox => {
                const E_UO2_GPA: f64 = 218.74;
                const E_PUO2_GPA: f64 = 249.45;
                const F1: f64 = 219.12;
                const F2: f64 = -0.0154;
                const F3: f64 = -9.0e-6;
                const REFERENCE_T: f64 = 273.0;

                let c_w_pu = state.pu_fraction / PU_ATOM_TO_MASS_FRACTION;
                let e_mix = (1.0 - c_w_pu) * E_UO2_GPA + c_w_pu * E_PUO2_GPA;

                // Upstream's y = 2 - O/M, again the negative of oxygen_deviation.
                let y = -state.oxygen_deviation;
                let e_y = if (0.0..=0.037).contains(&y) {
                    e_mix - 586.0 * y
                } else if y > 0.037 {
                    (e_mix - 0.037 * 586.0) + (y - 0.037) * -126.59
                } else {
                    e_mix
                };

                let f = |t: f64| F1 + F2 * t + F3 * t * t;
                let porosity = state.porosity.clamp(0.0, MATPRO_MAX_POROSITY);

                e_y * f(temperature) / f(REFERENCE_T) * 1.0e9 * (1.0 - porosity).powi(2)
                    / (1.0 + 1.1 * porosity)
            }

            Self::MatproZircaloy => matpro_zircaloy_young(temperature, state),

            Self::HofmanD9 => {
                let t_c = temperature - CELSIUS_OFFSET;
                (2.01e5 - 79.29 * t_c) * 1.0e6
            }

            Self::BisonMolybdenum => 3.349e11 - 5.101e7 * temperature,

            Self::Tobbe1515Ti => {
                let t_c = temperature - CELSIUS_OFFSET;
                (202.7 - 81.67e-3 * t_c) * 1.0e9
            }

            Self::WatrousHastelloyN => {
                let t_c = temperature - CELSIUS_OFFSET;
                (-9.944e-8 * t_c * t_c * t_c + 1.178e-4 * t_c * t_c - 1.033e-1 * t_c + 220.9)
                    * 1.0e9
            }

            Self::SneadSiC {
                porosity_coefficient,
            } => {
                const E0: f64 = 460.0e9;
                const T0: f64 = 962.0;
                const B: f64 = 0.04e9;
                E0 * (-porosity_coefficient * state.porosity).exp()
                    - B * temperature * (-T0 / temperature).exp()
            }

            Self::ParfumeBuffer { density } => {
                let t_c = temperature - CELSIUS_OFFSET;
                let phi = parfume_fluence_units(state.fast_fluence);
                25.5 * (0.384 + 0.324e-3 * density)
                    * (1.0 + 0.23 * phi)
                    * (1.0 + 1.5e-4 * (t_c - 20.0))
                    * 1.0e9
            }

            Self::ParfumePyC {
                density,
                bacon_anisotropy_factor: baf,
                crystallite_diameter: lc,
            } => {
                let t_c = temperature - CELSIUS_OFFSET;
                let phi = parfume_fluence_units(state.fast_fluence);
                let common = 25.5
                    * (0.384 + 0.324e-3 * density)
                    * (2.985 - 0.0662 * lc)
                    * (1.0 + 0.23 * phi)
                    * (1.0 + 0.00015 * (t_c - 20.0));
                let e_radial = common * (1.463 - 0.463 * baf);
                let e_tangential = common * (0.481 + 0.519 * baf);
                (e_radial + 2.0 * e_tangential) / 3.0 * 1.0e9
            }

            Self::ParfumeSiC => {
                // Tabulated in degrees Celsius, clamped to the end values.
                const T_TABLE: [f64; 4] = [25.0, 940.0, 1215.0, 1600.0];
                const E_TABLE: [f64; 4] = [428.0, 375.0, 340.0, 198.0];

                let t_c = temperature - CELSIUS_OFFSET;
                let e_gpa = if t_c <= T_TABLE[0] {
                    E_TABLE[0]
                } else if t_c > T_TABLE[3] {
                    E_TABLE[3]
                } else {
                    let mut out = E_TABLE[3];
                    for i in 0..3 {
                        if t_c > T_TABLE[i] && t_c <= T_TABLE[i + 1] {
                            let slope =
                                (E_TABLE[i + 1] - E_TABLE[i]) / (T_TABLE[i + 1] - T_TABLE[i]);
                            out = slope * (t_c - T_TABLE[i]) + E_TABLE[i];
                            break;
                        }
                    }
                    out
                };
                e_gpa * 1.0e9
            }
        }
    }
}

/// Offset \[K\] between the kelvin and Celsius scales.
///
/// Several fits below are published in degrees Celsius; this is the only place
/// the conversion is written down.
const CELSIUS_OFFSET: f64 = 273.15;

/// Approximate conversion factor \[-\] from plutonium **atom** fraction of the
/// heavy metal to plutonium **mass** fraction of the fuel: `c_mass = c_atom / k`.
///
/// Upstream derives `k = 1.13` from `k_Pu = (MM_HM + (O/M) * MM_O) / MM_Pu`
/// with `MM_Pu ~ 239 g/mol`, `MM_HM ~ 238.5 g/mol`, `MM_O = 16 g/mol`,
/// `O/M = 2`. It is an approximation, used by both MOX correlations.
const PU_ATOM_TO_MASS_FRACTION: f64 = 1.13;

/// MATPRO UO2 Young's modulus \[Pa\] at temperature `t` \[K\] and porosity `p`
/// \[-\].
///
/// Shared by [`YoungModulusModel::MatproUo2`] and
/// [`YoungModulusModel::MatproMox`], which builds on it. Porosity is clamped
/// into `[0, MATPRO_MAX_POROSITY]` so the linear knock-down cannot drive the
/// modulus to zero or negative.
fn matpro_uo2(t: f64, p: f64) -> f64 {
    const PAR1: f64 = 2.334e11;
    const PAR2: f64 = 2.752;
    const PAR3: f64 = 1.0915e-4;
    let porosity = p.clamp(0.0, MATPRO_MAX_POROSITY);
    PAR1 * (1.0 - PAR2 * porosity) * (1.0 - PAR3 * t)
}

/// MATPRO Zircaloy Young's modulus \[Pa\] at temperature `t` \[K\].
///
/// Alpha phase below 1073 K, beta phase at and above 1273 K, linear
/// interpolation between. Only the alpha phase carries the oxygen, cold-work
/// and fluence corrections; the beta-phase fit is a bare line in temperature.
///
/// Kept as a free function (rather than inlined into the `match`) because the
/// Poisson-ratio module needs the identical expression to form `nu = E/(2G)-1`.
pub(crate) fn matpro_zircaloy_young(t: f64, state: &MaterialState) -> f64 {
    const PAR1: f64 = 6.61e11;
    const PAR2: f64 = 5.912e8;
    const PAR3: f64 = 2.6e10;
    const PAR4: f64 = 0.88;
    const PAR5: f64 = 0.12;
    const PAR6: f64 = 1.0e25;
    const PAR7: f64 = 1.088e11;
    const PAR8: f64 = 5.475e7;
    const PAR9: f64 = 9.21e10;
    const PAR10: f64 = 4.05e7;
    const T_ALPHA: f64 = 1073.0;
    const T_BETA: f64 = 1273.0;

    let alpha = |temp: f64| {
        let k1 = (PAR1 + PAR2 * temp) * state.oxygen_content;
        let k2 = -PAR3 * state.cold_work;
        let k3 = PAR4 + PAR5 * (-state.fast_fluence / PAR6).exp();
        (PAR7 - PAR8 * temp + k1 + k2) / k3
    };
    let beta = |temp: f64| PAR9 - PAR10 * temp;

    if t < T_ALPHA {
        alpha(t)
    } else if t < T_BETA {
        // Upstream evaluates the beta endpoint at the fixed temperature T_BETA.
        let e_alpha = alpha(T_ALPHA);
        let e_beta = beta(T_BETA);
        e_alpha * (T_BETA - t) / (T_BETA - T_ALPHA) + e_beta * (t - T_ALPHA) / (T_BETA - T_ALPHA)
    } else {
        beta(t)
    }
}

/// Fast fluence expressed in the PARFUME fits' own unit of `1e25` n/m^2,
/// saturated at [`PARFUME_SATURATION_FLUENCE`].
fn parfume_fluence_units(fast_fluence: f64) -> f64 {
    (fast_fluence.max(0.0) / 1.0e25).min(PARFUME_SATURATION_FLUENCE / 1.0e25)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Relative-difference helper for the reference checks below.
    fn rel_diff(a: f64, b: f64) -> f64 {
        (a - b).abs() / b.abs()
    }

    /// Every variant, at a temperature inside its own validity range, for
    /// sweep-style tests.
    fn all_variants() -> Vec<YoungModulusModel> {
        vec![
            YoungModulusModel::Constant(2.0e11),
            YoungModulusModel::MatproUo2,
            YoungModulusModel::MatproMox,
            YoungModulusModel::SckCenMox,
            YoungModulusModel::MatproZircaloy,
            YoungModulusModel::HofmanD9,
            YoungModulusModel::BisonMolybdenum,
            YoungModulusModel::Tobbe1515Ti,
            YoungModulusModel::WatrousHastelloyN,
            YoungModulusModel::SneadSiC {
                porosity_coefficient: 0.0,
            },
            YoungModulusModel::ParfumeBuffer { density: 1000.0 },
            YoungModulusModel::ParfumePyC {
                density: 1900.0,
                bacon_anisotropy_factor: 1.0,
                crystallite_diameter: 30.0,
            },
            YoungModulusModel::ParfumeSiC,
        ]
    }

    // ---------------------------------------------------------------------
    // Constant
    // ---------------------------------------------------------------------

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: the constant model must return its payload unchanged at any
    /// temperature, since upstream `YoungModulusConstant::correct` simply
    /// assigns the dictionary value. Inputs: payload 1.5e11 Pa evaluated at
    /// 300 K and 2000 K. Pass criterion: exact equality.
    ///
    /// Result: both evaluations returned exactly 1.5e11 Pa.
    #[test]
    fn constant_returns_its_payload_at_any_temperature() {
        let model = YoungModulusModel::Constant(1.5e11);
        assert_eq!(model.value(&MaterialState::fresh(300.0)), 1.5e11);
        assert_eq!(model.value(&MaterialState::fresh(2000.0)), 1.5e11);
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: a non-positive constant modulus is physically impossible;
    /// `value_checked` must report it as `Unphysical`. Input: `Constant(-1.0)`
    /// at 300 K. Pass criterion: `Err(Unphysical { .. })`.
    ///
    /// Result: the error fired as expected.
    #[test]
    fn constant_rejects_a_non_positive_modulus() {
        let err = YoungModulusModel::Constant(-1.0)
            .value_checked(&MaterialState::fresh(300.0))
            .unwrap_err();
        assert!(matches!(err, OffbeatError::Unphysical { .. }));
    }

    // ---------------------------------------------------------------------
    // MATPRO UO2
    // ---------------------------------------------------------------------

    /// Reference-checked against the upstream expression (port verification).
    ///
    /// Methodology: `YoungModulusMatproUO2.C` computes
    /// `E = par1 * (1 - par2 * (1 - densityFrac)) * (1 - par3 * T)` with
    /// `par1 = 2.334e11`, `par2 = 2.752`, `par3 = 1.0915e-4`. Evaluated by hand
    /// at `T = 300 K` and zero porosity:
    /// `2.334e11 * 1 * (1 - 0.032745) = 2.25757317e11 Pa`. Tolerance: 1e-9
    /// relative. Pass criterion: the port reproduces that number.
    ///
    /// Result: `E = 2.25757317e11 Pa` (225.76 GPa), relative difference below
    /// 1e-15. This is **verification** of the transcription, not validation:
    /// 225.8 GPa is the same order as the commonly quoted room-temperature
    /// modulus of fully dense UO2 (roughly 220-230 GPa), but no experimental
    /// dataset was consulted here.
    #[test]
    fn matpro_uo2_matches_the_hand_evaluated_upstream_expression() {
        let e = YoungModulusModel::MatproUo2.value(&MaterialState::fresh(300.0));
        assert!(rel_diff(e, 2.25757317e11) < 1.0e-9, "got {e:e}");
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: the MATPRO UO2 fit is linear and decreasing in both
    /// temperature and porosity, so raising either must lower the modulus, and
    /// the porosity knock-down must match `1 - 2.752 * P` exactly. Inputs:
    /// 300/1000/2000 K at zero porosity, and `P = 0.05` at 300 K. Pass
    /// criterion: strict monotonic decrease, and the porosity ratio equal to
    /// `1 - 2.752 * 0.05 = 0.8624` to 1e-12 relative.
    ///
    /// Result: E(300) = 2.25757e11 > E(1000) = 2.07924e11 > E(2000) = 1.82449e11
    /// Pa; porosity ratio 0.8624 as predicted.
    #[test]
    fn matpro_uo2_softens_with_temperature_and_porosity() {
        let m = YoungModulusModel::MatproUo2;
        let e300 = m.value(&MaterialState::fresh(300.0));
        let e1000 = m.value(&MaterialState::fresh(1000.0));
        let e2000 = m.value(&MaterialState::fresh(2000.0));
        assert!(e300 > e1000 && e1000 > e2000);

        let mut porous = MaterialState::fresh(300.0);
        porous.porosity = 0.05;
        let ratio = m.value(&porous) / e300;
        assert!(rel_diff(ratio, 1.0 - 2.752 * 0.05) < 1.0e-12);
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: the port clamps porosity at `MATPRO_MAX_POROSITY` because
    /// the linear knock-down `1 - 2.752 * P` reaches zero at `P = 0.3634`.
    /// Inputs: porosity 0.30, 0.50 and 0.90 at 800 K. Pass criterion: all three
    /// give the same, strictly positive modulus.
    ///
    /// Result: all three returned 3.71506e10 Pa, positive as required. Without
    /// the clamp, `P = 0.9` would give a negative modulus.
    #[test]
    fn matpro_uo2_clamps_porosity_and_stays_positive() {
        let m = YoungModulusModel::MatproUo2;
        let at = |p: f64| {
            let mut s = MaterialState::fresh(800.0);
            s.porosity = p;
            m.value(&s)
        };
        let reference = at(MATPRO_MAX_POROSITY);
        assert!(reference > 0.0);
        assert_eq!(at(0.5), reference);
        assert_eq!(at(0.9), reference);
    }

    // ---------------------------------------------------------------------
    // MATPRO MOX
    // ---------------------------------------------------------------------

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: with zero plutonium and exact stoichiometry, upstream's
    /// MOX expression `ES * exp(-B*x) * (1 + 0.05*c_Pu)` collapses to `ES`,
    /// which is the MATPRO UO2 expression. Inputs: `pu_fraction = 0`,
    /// `oxygen_deviation = 0`, sweep 400-2800 K. Pass criterion: bitwise-equal
    /// to `MatproUo2`.
    ///
    /// Result: identical at every sampled temperature.
    #[test]
    fn matpro_mox_reduces_to_matpro_uo2_for_stoichiometric_zero_pu_fuel() {
        for t in [400.0, 1000.0, 1800.0, 2800.0] {
            let s = MaterialState::fresh(t);
            assert_eq!(
                YoungModulusModel::MatproMox.value(&s),
                YoungModulusModel::MatproUo2.value(&s)
            );
        }
    }

    /// Reference-checked against the upstream expression (port verification).
    ///
    /// Methodology: for hypostoichiometric MOX with `O/M = 1.97`, upstream's
    /// variable is `x = 2 - O/M = 0.03 >= 0`, so `B = 1.35`; with a plutonium
    /// atom fraction of 0.25 the mass fraction is `0.25/1.13 = 0.221239`. The
    /// expected ratio to the UO2 value is therefore
    /// `exp(-1.35*0.03) * (1 + 0.05*0.221239) = 0.960268 * 1.011062`.
    /// Tolerance: 1e-12 relative. Pass criterion: the port reproduces the
    /// product.
    ///
    /// Result: measured ratio 0.970932, matching the hand evaluation to better
    /// than 1e-15 relative. Note the direction: hypostoichiometry softens the
    /// fuel (factor < 1) while plutonium stiffens it (factor > 1), which is the
    /// sign behaviour the MATPRO fit intends.
    #[test]
    fn matpro_mox_hypostoichiometry_softens_and_plutonium_stiffens() {
        let mut s = MaterialState::fresh(1200.0);
        s.pu_fraction = 0.25;
        s.oxygen_deviation = -0.03; // O/M = 1.97

        let ratio = YoungModulusModel::MatproMox.value(&s) / YoungModulusModel::MatproUo2.value(&s);
        let expected = (-1.35f64 * 0.03).exp() * (1.0 + 0.05 * 0.25 / 1.13);
        assert!(rel_diff(ratio, expected) < 1.0e-12, "got {ratio}");
        assert!(
            (-1.35f64 * 0.03).exp() < 1.0,
            "hypostoichiometry must soften"
        );
    }

    // ---------------------------------------------------------------------
    // SCK-CEN MOX
    // ---------------------------------------------------------------------

    /// Reference-checked against a value stated in the cited source.
    ///
    /// Methodology: the SCK-CEN correlation normalises its temperature shape at
    /// 273 K, so at exactly 273 K with zero porosity, zero plutonium and exact
    /// stoichiometry it must return the UO2 end-member modulus `EUO2 = 218.74`
    /// GPa quoted in the INSPYRE D7.2 report and hard-coded upstream as
    /// `EUO2_(218.74)`. Tolerance: 1e-12 relative. Pass criterion: 2.1874e11 Pa.
    ///
    /// Result: `E = 2.1874e11 Pa` exactly (relative difference 0.0).
    #[test]
    fn sck_cen_mox_returns_the_uo2_end_member_at_the_normalisation_temperature() {
        let e = YoungModulusModel::SckCenMox.value(&MaterialState::fresh(273.0));
        assert!(rel_diff(e, 218.74e9) < 1.0e-12, "got {e:e}");
    }

    /// Reference-checked against a value stated in the cited source.
    ///
    /// Methodology: with the plutonium **mass** fraction driven to 1 (atom
    /// fraction 1.13, the conversion factor) the mixture rule
    /// `(1 - c) * 218.74 + c * 249.45` must return the PuO2 end member
    /// `EPuO2 = 249.45` GPa from the INSPYRE report. Evaluated at 273 K with
    /// exact stoichiometry and zero porosity. Tolerance: 1e-12 relative.
    ///
    /// Result: `E = 2.4945e11 Pa`, matching the quoted end member exactly.
    #[test]
    fn sck_cen_mox_returns_the_puo2_end_member_at_full_plutonium() {
        let mut s = MaterialState::fresh(273.0);
        s.pu_fraction = 1.13; // -> mass fraction 1.0
        let e = YoungModulusModel::SckCenMox.value(&s);
        assert!(rel_diff(e, 249.45e9) < 1.0e-12, "got {e:e}");
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: three structural properties of the SCK-CEN fit must hold —
    /// the quadratic `f(T)` decreases with temperature, hypostoichiometry
    /// (`y > 0`) reduces the modulus through the `-586 * y` slope, and the
    /// porosity factor `(1-P)^2/(1+1.1P)` decreases with porosity. Inputs:
    /// 600 vs 1600 K; `O/M = 1.98` vs 2.00; porosity 0 vs 0.1, all at 1000 K.
    /// Pass criterion: strict decrease in each case.
    ///
    /// Result: E(600 K) = 2.10975e11 > E(1600 K) = 1.75037e11 Pa; the
    /// `O/M = 1.98` case is 5.36 % below the stoichiometric one; porosity 0.1
    /// removes 27.03 % of the modulus. All three signs as expected.
    #[test]
    fn sck_cen_mox_softens_with_temperature_hypostoichiometry_and_porosity() {
        let m = YoungModulusModel::SckCenMox;
        assert!(m.value(&MaterialState::fresh(600.0)) > m.value(&MaterialState::fresh(1600.0)));

        let base = MaterialState::fresh(1000.0);
        let mut hypo = base;
        hypo.oxygen_deviation = -0.02; // y = +0.02
        assert!(m.value(&hypo) < m.value(&base));

        let mut porous = base;
        porous.porosity = 0.1;
        assert!(m.value(&porous) < m.value(&base));
    }

    // ---------------------------------------------------------------------
    // MATPRO Zircaloy
    // ---------------------------------------------------------------------

    /// Reference-checked against the upstream expression (port verification).
    ///
    /// Methodology: in the alpha phase with no oxygen, cold work or fluence,
    /// `YoungModulusMatproZy.C` reduces to `E = (1.088e11 - 5.475e7*T)/1.0`.
    /// Hand-evaluated at 300 K: `1.088e11 - 1.6425e10 = 9.2375e10 Pa`.
    /// Tolerance: 1e-12 relative.
    ///
    /// Result: `E = 9.2375e10 Pa` (92.4 GPa), matching exactly. For context,
    /// the commonly quoted room-temperature Young's modulus of Zircaloy-4 is
    /// about 99 GPa, so the MATPRO fit sits roughly 7 % low there — that is a
    /// property of the correlation, and this test verifies the transcription,
    /// not the correlation.
    #[test]
    fn matpro_zircaloy_alpha_phase_matches_the_hand_evaluated_expression() {
        let e = YoungModulusModel::MatproZircaloy.value(&MaterialState::fresh(300.0));
        assert!(rel_diff(e, 9.2375e10) < 1.0e-12, "got {e:e}");
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: the three-branch phase structure must be continuous, or the
    /// mechanics solve would see a stiffness jump at a phase boundary. The
    /// interpolation branch is built from the alpha value at 1073 K and the
    /// beta value at 1273 K, so both boundaries should join smoothly. Inputs:
    /// pairs straddling 1073 K and 1273 K by 1e-6 K. Pass criterion: relative
    /// jump below 1e-6 (the finite offset alone produces a jump of order
    /// 1e-9, so a genuine discontinuity would be orders of magnitude larger).
    ///
    /// Result: relative jumps of 2.04e-9 at 1073 K and 2.17e-9 at 1273 K —
    /// both consistent with the 1e-6 K sampling offset, i.e. continuous.
    #[test]
    fn matpro_zircaloy_is_continuous_across_both_phase_boundaries() {
        let m = YoungModulusModel::MatproZircaloy;
        for boundary in [1073.0, 1273.0] {
            let below = m.value(&MaterialState::fresh(boundary - 1.0e-6));
            let above = m.value(&MaterialState::fresh(boundary + 1.0e-6));
            assert!(rel_diff(below, above) < 1.0e-6, "jump at {boundary} K");
        }
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: the fluence term is `K3 = 0.88 + 0.12*exp(-phi/1e25)` and
    /// divides the alpha-phase numerator, so accumulated fast fluence must
    /// *raise* the modulus (irradiation hardening) and saturate at a factor
    /// `1/0.88 = 1.13636`. Cold work enters as `K2 = -2.6e10 * C_cw` in the
    /// numerator and must *lower* it. Inputs at 600 K: fluence 0, 1e25 and
    /// 1e27 n/m^2; cold work 0 vs 0.2. Pass criterion: monotone increase with
    /// fluence, saturation ratio within 1e-6 of 1/0.88, decrease with cold work.
    ///
    /// Result: E(0) = 7.5950e10, E(1e25) = 8.2184e10, E(1e27) = 8.6307e10 Pa;
    /// saturation ratio 1.136364 as predicted; 20 % cold work removed 6.85 % of
    /// the modulus.
    #[test]
    fn matpro_zircaloy_hardens_with_fluence_and_softens_with_cold_work() {
        let m = YoungModulusModel::MatproZircaloy;
        let base = MaterialState::fresh(600.0);

        let mut low = base;
        low.fast_fluence = 1.0e25;
        let mut high = base;
        high.fast_fluence = 1.0e27;
        assert!(m.value(&base) < m.value(&low));
        assert!(m.value(&low) < m.value(&high));
        assert!(rel_diff(m.value(&high) / m.value(&base), 1.0 / 0.88) < 1.0e-6);

        let mut worked = base;
        worked.cold_work = 0.2;
        assert!(m.value(&worked) < m.value(&base));
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: upstream states the validity range 290-1800 K explicitly
    /// (it warns outside). `value_checked` must reject both ends while `value`
    /// clamps. Inputs: 200 K and 2500 K. Pass criterion: `Err(OutOfRange)` from
    /// `value_checked`, and `value` equal to the corresponding endpoint value.
    ///
    /// Result: both rejected; clamped values matched the 290 K and 1800 K
    /// evaluations exactly.
    #[test]
    fn matpro_zircaloy_range_check_fires_and_value_clamps() {
        let m = YoungModulusModel::MatproZircaloy;
        assert!(matches!(
            m.value_checked(&MaterialState::fresh(200.0)),
            Err(OffbeatError::OutOfRange { .. })
        ));
        assert!(matches!(
            m.value_checked(&MaterialState::fresh(2500.0)),
            Err(OffbeatError::OutOfRange { .. })
        ));
        assert_eq!(
            m.value(&MaterialState::fresh(200.0)),
            m.value(&MaterialState::fresh(290.0))
        );
        assert_eq!(
            m.value(&MaterialState::fresh(2500.0)),
            m.value(&MaterialState::fresh(1800.0))
        );
    }

    // ---------------------------------------------------------------------
    // Metallic claddings and structures
    // ---------------------------------------------------------------------

    /// Reference-checked against the upstream expression (port verification).
    ///
    /// Methodology: `YoungModulusHofmanD9.C` computes
    /// `E = (2.01e5 - 79.29 * T_C) * 1e6` with `T_C = T - 273.15`.
    /// Hand-evaluated at 20 C: `(201000 - 1585.8) * 1e6 = 1.994142e11 Pa`.
    /// Tolerance: 1e-12 relative.
    ///
    /// Result: `E = 1.994142e11 Pa` (199.4 GPa) at 293.15 K, matching exactly.
    /// That is the expected order for an austenitic stainless steel; no
    /// experimental dataset was consulted.
    #[test]
    fn hofman_d9_matches_the_hand_evaluated_expression_at_20_celsius() {
        let e = YoungModulusModel::HofmanD9.value(&MaterialState::fresh(293.15));
        assert!(rel_diff(e, 1.994142e11) < 1.0e-12, "got {e:e}");
    }

    /// Reference-checked against the upstream expression (port verification).
    ///
    /// Methodology: `YoungModulusTobbe1515Ti.C` computes
    /// `E = (202.7 - 0.08167 * T_C) * 1e9`. Hand-evaluated at 20 C:
    /// `(202.7 - 1.6334) * 1e9 = 2.010666e11 Pa`. Tolerance: 1e-12 relative.
    ///
    /// Result: `E = 2.010666e11 Pa` (201.1 GPa) at 293.15 K, matching exactly.
    #[test]
    fn tobbe_15_15_ti_matches_the_hand_evaluated_expression_at_20_celsius() {
        let e = YoungModulusModel::Tobbe1515Ti.value(&MaterialState::fresh(293.15));
        assert!(rel_diff(e, 2.010666e11) < 1.0e-12, "got {e:e}");
    }

    /// Reference-checked against the upstream expression (port verification).
    ///
    /// Methodology: `YoungModulusMolybdenum.C` computes
    /// `E = 3.349e11 - 5.101e7 * T`. Hand-evaluated at 300 K:
    /// `3.349e11 - 1.5303e10 = 3.19597e11 Pa`. Tolerance: 1e-12 relative.
    ///
    /// Result: `E = 3.19597e11 Pa` (319.6 GPa), matching exactly — the same
    /// order as the commonly quoted room-temperature modulus of molybdenum
    /// (about 320 GPa), though no experimental dataset was consulted.
    #[test]
    fn bison_molybdenum_matches_the_hand_evaluated_expression_at_300_k() {
        let e = YoungModulusModel::BisonMolybdenum.value(&MaterialState::fresh(300.0));
        assert!(rel_diff(e, 3.19597e11) < 1.0e-12, "got {e:e}");
    }

    /// Reference-checked against the upstream expression (port verification).
    ///
    /// Methodology: `YoungModulusWatrousHastelloyN.C` computes the cubic
    /// `E = (-9.944e-8*T_C^3 + 1.178e-4*T_C^2 - 0.1033*T_C + 220.9) * 1e9`.
    /// Hand-evaluated at 20 C: `220.9 + 0.047120 - 2.066 - 0.00079552`
    /// `= 218.880324 GPa`. Tolerance: 1e-10 relative.
    ///
    /// Result: `E = 2.18880324e11 Pa` (218.88 GPa) at 293.15 K, matching to
    /// better than 1e-15 relative.
    #[test]
    fn watrous_hastelloy_n_matches_the_hand_evaluated_expression_at_20_celsius() {
        let e = YoungModulusModel::WatrousHastelloyN.value(&MaterialState::fresh(293.15));
        assert!(rel_diff(e, 2.1888032448e11) < 1.0e-10, "got {e:e}");
    }

    /// Self-consistency check documenting a deliberate divergence from upstream.
    ///
    /// Methodology: upstream leaves the Hastelloy N modulus at its initialised
    /// **zero** above 1273.15 K, which would make the stiffness matrix
    /// singular; this port clamps to the 1273.15 K endpoint instead. Inputs:
    /// 1500 K and 1273.15 K. Pass criterion: equal, strictly positive, and
    /// `value_checked` reports `OutOfRange` at 1500 K.
    ///
    /// Result: both evaluations returned 1.35960e11 Pa (135.96 GPa) and the
    /// range check fired. Upstream would have returned 0 Pa.
    #[test]
    fn watrous_hastelloy_n_clamps_instead_of_returning_zero_above_the_limit() {
        let m = YoungModulusModel::WatrousHastelloyN;
        let clamped = m.value(&MaterialState::fresh(1500.0));
        assert!(clamped > 0.0);
        assert_eq!(clamped, m.value(&MaterialState::fresh(1273.15)));
        assert!(matches!(
            m.value_checked(&MaterialState::fresh(1500.0)),
            Err(OffbeatError::OutOfRange { .. })
        ));
    }

    // ---------------------------------------------------------------------
    // Silicon carbide
    // ---------------------------------------------------------------------

    /// Reference-checked against a value stated in the cited source.
    ///
    /// Methodology: Snead et al. (2007) give the CVD-SiC modulus as
    /// `E = E0 - B*T*exp(-T0/T)` with `E0 = 460 GPa`, `B = 0.04 GPa/K`,
    /// `T0 = 962 K`; `E0` is the handbook's room-temperature value. At 300 K
    /// the thermal term removes only 0.486 GPa, so the prediction must sit
    /// within 0.2 % of 460 GPa. Tolerance: 0.2 % relative to 460 GPa, plus an
    /// exact 1e-12 check against the hand-evaluated 4.59514104e11 Pa.
    ///
    /// Result: `E = 4.5951410e11 Pa` (459.51 GPa), 0.106 % below the handbook
    /// `E0` — consistent with the small thermal term at 300 K.
    #[test]
    fn snead_sic_is_within_a_fraction_of_a_percent_of_the_handbook_value_at_300_k() {
        let m = YoungModulusModel::SneadSiC {
            porosity_coefficient: 0.0,
        };
        let e = m.value(&MaterialState::fresh(300.0));
        assert!(rel_diff(e, 460.0e9) < 2.0e-3, "got {e:e}");
        assert!(rel_diff(e, 4.59514103682772e11) < 1.0e-12, "got {e:e}");
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: the thermal term `-B*T*exp(-T0/T)` grows monotonically with
    /// temperature, and the porosity term `exp(-C*P)` reduces the modulus when
    /// `C > 0`. Inputs: 300/900/1800 K at `C = 0`; `P = 0.1` with `C = 3.0` at
    /// 900 K. Pass criterion: strict decrease in both directions.
    ///
    /// Result: E(300) = 4.59514e11 > E(900) = 4.47638e11 > E(1800) = 4.17808e11
    /// Pa; with `C = 3`, 10 % porosity removed 26.63 % of the modulus.
    #[test]
    fn snead_sic_softens_with_temperature_and_with_porosity() {
        let dense = YoungModulusModel::SneadSiC {
            porosity_coefficient: 0.0,
        };
        let e300 = dense.value(&MaterialState::fresh(300.0));
        let e900 = dense.value(&MaterialState::fresh(900.0));
        let e1800 = dense.value(&MaterialState::fresh(1800.0));
        assert!(e300 > e900 && e900 > e1800);

        let porous_model = YoungModulusModel::SneadSiC {
            porosity_coefficient: 3.0,
        };
        let mut porous = MaterialState::fresh(900.0);
        porous.porosity = 0.1;
        assert!(porous_model.value(&porous) < porous_model.value(&MaterialState::fresh(900.0)));
    }

    /// Reference-checked against values stated in the cited source.
    ///
    /// Methodology: the PARFUME SiC model is a piecewise-linear interpolation
    /// through four tabulated points, transcribed upstream as
    /// `(temp1..temp4) = (25, 940, 1215, 1600) C` and
    /// `(E1..E4) = (428, 375, 340, 198) GPa`. Evaluating at exactly those
    /// temperatures must return exactly those moduli, and outside the table the
    /// model clamps to the end values. Tolerance: 1e-12 relative.
    ///
    /// Result: 428.0, 375.0, 340.0 and 198.0 GPa recovered at 25, 940, 1215 and
    /// 1600 C respectively; 0 C returned 428 GPa and 2000 C returned 198 GPa,
    /// confirming the clamping.
    #[test]
    fn parfume_sic_recovers_its_tabulated_points() {
        let m = YoungModulusModel::ParfumeSiC;
        for (t_c, e_gpa) in [
            (25.0, 428.0),
            (940.0, 375.0),
            (1215.0, 340.0),
            (1600.0, 198.0),
        ] {
            let e = m.value(&MaterialState::fresh(t_c + 273.15));
            assert!(rel_diff(e, e_gpa * 1.0e9) < 1.0e-12, "at {t_c} C got {e:e}");
        }
        // Clamped below and above the table.
        assert!(rel_diff(m.value(&MaterialState::fresh(273.15)), 428.0e9) < 1.0e-12);
        assert!(rel_diff(m.value(&MaterialState::fresh(2273.15)), 198.0e9) < 1.0e-12);
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: the tabulated points decrease monotonically, so the
    /// interpolant must too; and a midpoint must land halfway between its
    /// bracketing table entries. Inputs: a 25 C sweep across the table, plus
    /// the midpoint of the first interval, `T_C = 482.5`. Pass criterion:
    /// non-increasing sequence, midpoint equal to `(428 + 375)/2 = 401.5` GPa
    /// to 1e-12 relative.
    ///
    /// Result: monotone throughout; midpoint returned 401.5 GPa exactly.
    #[test]
    fn parfume_sic_interpolates_linearly_and_monotonically() {
        let m = YoungModulusModel::ParfumeSiC;
        let mut previous = f64::INFINITY;
        let mut t_c = 25.0;
        while t_c <= 1600.0 {
            let e = m.value(&MaterialState::fresh(t_c + 273.15));
            assert!(e <= previous, "not monotone at {t_c} C");
            previous = e;
            t_c += 25.0;
        }
        let mid = m.value(&MaterialState::fresh(482.5 + 273.15));
        assert!(rel_diff(mid, 401.5e9) < 1.0e-12, "got {mid:e}");
    }

    // ---------------------------------------------------------------------
    // PARFUME coating layers
    // ---------------------------------------------------------------------

    /// Reference-checked against the upstream expression (port verification).
    ///
    /// Methodology: `YoungModulusPARFUMEBuffer.C` computes
    /// `E = 25.5*(0.384 + 0.324e-3*rho)*(1 + 0.23*phi25)*(1 + 1.5e-4*(T_C-20))`
    /// in GPa. At the reference point `rho = 1000 kg/m^3`, `T_C = 20 C`,
    /// `phi = 0` the last two factors are unity, leaving
    /// `25.5 * 0.708 = 18.054 GPa`. Tolerance: 1e-12 relative.
    ///
    /// Result: `E = 1.8054e10 Pa` (18.05 GPa), matching exactly. The buffer is
    /// deliberately the most compliant TRISO layer, an order of magnitude below
    /// the SiC layer's 428 GPa.
    #[test]
    fn parfume_buffer_matches_the_hand_evaluated_expression_at_the_reference_point() {
        let m = YoungModulusModel::ParfumeBuffer { density: 1000.0 };
        let e = m.value(&MaterialState::fresh(293.15));
        assert!(rel_diff(e, 1.8054e10) < 1.0e-12, "got {e:e}");
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: the PARFUME irradiation term `1 + 0.23*phi25` saturates at
    /// `phi = 3.96e25 n/m^2` (`PARFUME_SATURATION_FLUENCE`) by upstream's own
    /// `min(phi, 3.96)`. Inputs: fluence 3.96e25, 1e26 and 1e27 n/m^2 at
    /// 293.15 K. Pass criterion: all equal, and equal to `1 + 0.23*3.96 = 1.9108`
    /// times the zero-fluence value.
    ///
    /// Result: all three returned 3.44976e10 Pa; ratio to the unirradiated
    /// value 1.9108, exactly the saturated factor.
    #[test]
    fn parfume_buffer_fluence_term_saturates_at_the_documented_cutoff() {
        let m = YoungModulusModel::ParfumeBuffer { density: 1000.0 };
        let at = |phi: f64| {
            let mut s = MaterialState::fresh(293.15);
            s.fast_fluence = phi;
            m.value(&s)
        };
        let saturated = at(PARFUME_SATURATION_FLUENCE);
        assert_eq!(at(1.0e26), saturated);
        assert_eq!(at(1.0e27), saturated);
        assert!(rel_diff(saturated / at(0.0), 1.0 + 0.23 * 3.96) < 1.0e-12);
    }

    /// Reference-checked against the upstream expression (port verification).
    ///
    /// Methodology: `YoungModulusPARFUMEPyC.C` at the standard TRISO defaults
    /// `rho = 1900 kg/m^3`, `BAF = 1.0`, `Lc = 30 nm`, `T_C = 20 C`, `phi = 0`
    /// gives `25.5 * 0.9996 * 1.0 * 0.999 = 25.4643102 GPa` for both the radial
    /// and tangential components, hence the same for their average.
    /// Tolerance: 1e-12 relative.
    ///
    /// Result: `E = 2.54643102e10 Pa` (25.46 GPa), matching exactly.
    #[test]
    fn parfume_pyc_matches_the_hand_evaluated_expression_at_triso_defaults() {
        let m = YoungModulusModel::ParfumePyC {
            density: 1900.0,
            bacon_anisotropy_factor: 1.0,
            crystallite_diameter: 30.0,
        };
        let e = m.value(&MaterialState::fresh(293.15));
        assert!(rel_diff(e, 2.54643102e10) < 1.0e-12, "got {e:e}");
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: upstream notes that at `BAF = 1.0` the radial factor
    /// `1.463 - 0.463*BAF` and the tangential factor `0.481 + 0.519*BAF` are
    /// both exactly 1, so the transversely isotropic PyC degenerates to an
    /// isotropic material and the `(E_r + 2*E_t)/3` average is exact rather
    /// than approximate. Checked by comparing the two factors symbolically at
    /// `BAF = 1` and confirming a `BAF = 1.06` case moves the average.
    /// Pass criterion: the factors agree at `BAF = 1`, and the average shifts
    /// for `BAF > 1`.
    ///
    /// Result: the factors agree exactly at `BAF = 1`; at `BAF = 1.06` the
    /// average rose by 1.15 % (the tangential term grows faster than the radial
    /// term shrinks), so the isotropic-average simplification is only exact at
    /// `BAF = 1`.
    #[test]
    fn parfume_pyc_is_isotropic_only_at_unit_anisotropy_factor() {
        let radial = |baf: f64| 1.463 - 0.463 * baf;
        let tangential = |baf: f64| 0.481 + 0.519 * baf;
        assert_eq!(radial(1.0), tangential(1.0));

        let isotropic = YoungModulusModel::ParfumePyC {
            density: 1900.0,
            bacon_anisotropy_factor: 1.0,
            crystallite_diameter: 30.0,
        };
        let anisotropic = YoungModulusModel::ParfumePyC {
            density: 1900.0,
            bacon_anisotropy_factor: 1.06,
            crystallite_diameter: 30.0,
        };
        let s = MaterialState::fresh(293.15);
        assert!(anisotropic.value(&s) > isotropic.value(&s));
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: upstream warns when the PyC density leaves 1800-2000 kg/m^3
    /// or when `BAF < 1`; this port reports both through `value_checked` while
    /// leaving `value` unclamped, because the fit is linear in density and
    /// well-behaved outside its data range. Inputs: density 1500 kg/m^3, and
    /// `BAF = 0.9`. Pass criterion: `Err(OutOfRange)` in both cases, and a
    /// finite positive `value`.
    ///
    /// Result: both checks fired; the unclamped `value` calls returned finite
    /// positive moduli (2.21628e10 Pa and 2.49762e10 Pa respectively).
    #[test]
    fn parfume_pyc_reports_density_and_anisotropy_outside_the_fit_range() {
        let s = MaterialState::fresh(293.15);

        let thin = YoungModulusModel::ParfumePyC {
            density: 1500.0,
            bacon_anisotropy_factor: 1.0,
            crystallite_diameter: 30.0,
        };
        assert!(matches!(
            thin.value_checked(&s),
            Err(OffbeatError::OutOfRange { .. })
        ));
        assert!(thin.value(&s).is_finite() && thin.value(&s) > 0.0);

        let low_baf = YoungModulusModel::ParfumePyC {
            density: 1900.0,
            bacon_anisotropy_factor: 0.9,
            crystallite_diameter: 30.0,
        };
        assert!(matches!(
            low_baf.value_checked(&s),
            Err(OffbeatError::OutOfRange { .. })
        ));
        assert!(low_baf.value(&s) > 0.0);
    }

    // ---------------------------------------------------------------------
    // Cross-variant sweeps
    // ---------------------------------------------------------------------

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: a Young's modulus that is zero, negative, infinite or NaN
    /// anywhere inside a correlation's own validity range would make the
    /// mechanics stiffness matrix singular or poison it with NaN. Every variant
    /// is swept over 64 points spanning its declared temperature range at a
    /// representative irradiated state (5 % porosity, 1e25 n/m^2 fluence, 10 %
    /// Pu, `O/M = 1.98`, 10 % cold work, 0.1 wt% oxygen). Pass criterion:
    /// finite and strictly positive everywhere.
    ///
    /// Result: all 13 variants passed at all 64 sample points.
    #[test]
    fn every_variant_is_finite_and_positive_across_its_validity_range() {
        for model in all_variants() {
            let (low, high) = model.temperature_range();
            // Constant reports the whole positive axis; sample a sane subrange.
            let (low, high) = if matches!(model, YoungModulusModel::Constant(_)) {
                (300.0, 3000.0)
            } else {
                (low, high)
            };
            for i in 0..64 {
                let t = low + (high - low) * f64::from(i) / 63.0;
                let mut s = MaterialState::fresh(t);
                s.porosity = 0.05;
                s.fast_fluence = 1.0e25;
                s.pu_fraction = 0.10;
                s.oxygen_deviation = -0.02;
                s.cold_work = 0.10;
                s.oxygen_content = 0.001;
                let e = model.value(&s);
                assert!(
                    e.is_finite() && e > 0.0,
                    "{} gave {e:e} at {t} K",
                    model.name()
                );
            }
        }
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: the documented contract is that `value` clamps and
    /// `value_checked` refuses. For every variant with a finite range, evaluate
    /// 1 K below the low bound and 1 K above the high bound. Pass criterion:
    /// `value` equals the endpoint evaluation, `value_checked` returns
    /// `OutOfRange`.
    ///
    /// Result: all 12 range-limited variants clamped and reported correctly
    /// (the `Constant` variant is exempt: it declares no meaningful range).
    #[test]
    fn value_clamps_where_value_checked_reports_out_of_range() {
        for model in all_variants() {
            if matches!(model, YoungModulusModel::Constant(_)) {
                continue;
            }
            let (low, high) = model.temperature_range();
            for (outside, endpoint) in [(low - 1.0, low), (high + 1.0, high)] {
                let s_out = MaterialState::fresh(outside);
                let s_end = MaterialState::fresh(endpoint);
                assert_eq!(
                    model.value(&s_out),
                    model.value(&s_end),
                    "{} did not clamp at {outside} K",
                    model.name()
                );
                assert!(
                    matches!(
                        model.value_checked(&s_out),
                        Err(OffbeatError::OutOfRange { .. })
                    ),
                    "{} did not report {outside} K",
                    model.name()
                );
            }
        }
    }

    /// Self-consistency check (no external reference).
    ///
    /// Methodology: a non-positive absolute temperature is impossible, and must
    /// be reported as `Unphysical` rather than `OutOfRange`, for every variant.
    /// Inputs: 0 K and -10 K. Pass criterion: `Err(Unphysical { .. })`.
    ///
    /// Result: all 13 variants reported `Unphysical` at both inputs.
    #[test]
    fn non_positive_temperature_is_unphysical_for_every_variant() {
        for model in all_variants() {
            for t in [0.0, -10.0] {
                assert!(
                    matches!(
                        model.value_checked(&MaterialState::fresh(t)),
                        Err(OffbeatError::Unphysical { .. })
                    ),
                    "{} accepted {t} K",
                    model.name()
                );
            }
        }
    }
}
