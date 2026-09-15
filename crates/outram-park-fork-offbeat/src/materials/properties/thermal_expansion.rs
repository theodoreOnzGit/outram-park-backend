// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
// `offbeatLib/materials/materialModel/thermoMechanicalPropertiesModels/`
// `thermalExpansion/`, specifically:
//   thermalExpansionModel.{H,C}                (base class, Tref_ handling)
//   thermalExpansionConstant.{H,C}
//   thermalExpansionRelapUO2.{H,C}
//   thermalExpansionMatproUPuO2.{H,C}
//   thermalExpansionMartinUPuO2.{H,C}
//   thermalExpansionLemehovUPuO2.{H,C}
//   thermalExpansionMAMOX.{H,C}
//   thermalExpansionMatproZy.{H,C}
//   thermalExpansionGehr1515Ti.{H,C}
//   thermalExpansionMolybdenum.{H,C}
//   thermalExpansionSneadSiC.{H,C}
//   thermalExpansionSwindemanHastelloyN.{H,C}
//   thermalExpansionPARFUMEBuffer.{H,C}
//   thermalExpansionPARFUMEPyC.{H,C}
//   thermalExpansionPARFUMESiC.{H,C}
// Upstream main author: A. Scolaro (EPFL LRS); contributions from E. Brunetto,
// C. Fiorina (EPFL) and I. Clifford (PSI).
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Thermal expansion correlations — **strain** \[-\] and **coefficient**
//! \[1/K\].
//!
//! # Read this before using the module: strain is not the coefficient
//!
//! Two different quantities are called "thermal expansion" in the
//! fuel-performance literature, they differ by three or more orders of
//! magnitude, and interchanging them is *the* classic error in this corner of
//! the physics. This module exposes both, separately and by name:
//!
//! | Method | Symbol | Unit | Meaning |
//! |---|---|---|---|
//! | [`strain`](ThermalExpansionModel::strain) | `eps_th` | \[-\] | the dimensionless linear thermal strain `dL/L0`, i.e. how much longer the material *is* at the current temperature than at the correlation's reference temperature |
//! | [`coefficient`](ThermalExpansionModel::coefficient) | `alpha` | \[1/K\] | the **instantaneous** coefficient of linear thermal expansion, `d(eps_th)/dT` — how fast the strain is *changing* with temperature right now |
//!
//! A typical oxide fuel at 1000 K has `eps_th` of order `1e-2` and `alpha` of
//! order `1e-5 1/K`. If a stress calculation is silently a thousand times too
//! large or too small, this is the first thing to check.
//!
//! Two further traps this module makes explicit:
//!
//! - **`alpha` is not `eps_th / (T - Tref)`.** That ratio is the *mean*
//!   coefficient over the interval, which equals the instantaneous coefficient
//!   only for a strictly linear fit. Several correlations here
//!   ([`SneadSiC`](ThermalExpansionModel::SneadSiC),
//!   [`MartinUPuO2`](ThermalExpansionModel::MartinUPuO2)) are published as
//!   *mean* coefficients and are converted internally; the mean form never
//!   escapes this module.
//! - **Every correlation has its own reference temperature.** Some take it as
//!   a parameter (the stress-free temperature of the case, upstream's `Tref`);
//!   some have it baked into the fit and it cannot be changed. Each variant
//!   documents which, and
//!   [`reference_temperature`](ThermalExpansionModel::reference_temperature)
//!   reports it at runtime. Subtracting the strain of one correlation from
//!   that of another with a different reference is meaningless.
//!
//! # Anisotropy
//!
//! All but one of the ported correlations are isotropic. Pyrolytic carbon in a
//! TRISO coating is not: [`PARFUMEPyC`](ThermalExpansionModel::PARFUMEPyC)
//! expands differently along the radius than tangentially, controlled by the
//! Bacon anisotropy factor. [`strain`](ThermalExpansionModel::strain) returns
//! the isotropic-equivalent linear strain (the mean of the three principal
//! components, i.e. one third of the volumetric strain);
//! [`principal_strains`](ThermalExpansionModel::principal_strains) returns the
//! three components `[radial, tangential, tangential]` for callers that need
//! the tensor. For every isotropic variant the three are equal.
//!
//! # Validity ranges, clamping and honesty about what upstream states
//!
//! Where the upstream OFFBEAT source states or encodes a validity range — a
//! warning, a hard cut-off, a documented composition window — this port
//! enforces exactly that range. Where upstream states **no** range, this port
//! enforces **none** and says so, rather than inventing a plausible-looking
//! bound. [`validity_range`](ThermalExpansionModel::validity_range) returns
//! `(0.0, f64::INFINITY)` in that case, and the doc comment on the variant says
//! that the caller carries the extrapolation risk.
//!
//! The plain [`strain`](ThermalExpansionModel::strain) /
//! [`coefficient`](ThermalExpansionModel::coefficient) methods **clamp** the
//! temperature to the enforced range endpoints before evaluating; the
//! `*_checked` variants return
//! [`OffbeatError::OutOfRange`]
//! instead. Note that clamping is a *deviation* from upstream for the two
//! variants that do have a range: upstream
//! [`MatproZy`](ThermalExpansionModel::MatproZy) prints a warning and then
//! extrapolates anyway. Clamping was chosen because an extrapolated fit
//! feeding a mechanics solve produces a plausible, wrong answer with no trace
//! in the log.
//!
//! # Units
//!
//! Raw `f64` in strict SI, per the crate-level units policy: temperature in
//! kelvin, strain dimensionless, coefficient in `1/K`. Correlations published
//! in degrees Celsius convert internally and never expose °C.

use crate::error::{OffbeatError, Result};
use crate::materials::MaterialState;

// ---------------------------------------------------------------------------
// Fit coefficients, exactly as they appear in the upstream constructors.
//
// These are private: they are the internals of a named published fit, not a
// tuning surface. The handful of upstream defaults a caller genuinely needs in
// order to *construct* a variant are re-exported as documented public consts
// further down.
// ---------------------------------------------------------------------------

/// Numerical dead zone applied to the strain tensor, copied from upstream.
///
/// Upstream zeroes the whole strain tensor when every diagonal component is
/// below `1e-7` in magnitude, to stop a case held exactly at `Tref` from
/// jittering. At a coefficient of `1e-5 1/K` this covers roughly `|T - Tref| <
/// 0.01 K`.
const STRAIN_DEAD_ZONE: f64 = 1e-7;

/// Floor applied to temperature in the *plain* (clamping) evaluation path.
///
/// Purely a numerical guard so the `exp(-E/kT)` term in the RELAP UO2 fit stays
/// finite if a caller passes a zero or negative temperature. It is not a
/// validity statement — `*_checked` rejects such input as
/// [`OffbeatError::Unphysical`].
const MIN_EVAL_TEMPERATURE: f64 = 1.0e-3;

// -- RELAP/MATPRO UO2 (thermalExpansionRelapUO2.C) --------------------------
const RELAP_UO2_K1: f64 = 9.8e-6; // 1/K
const RELAP_UO2_K2: f64 = 2.61e-3; // -
const RELAP_UO2_K3: f64 = 3.16e-1; // -
const RELAP_UO2_ED: f64 = 1.32e-19; // J, defect energy
/// Boltzmann constant as *upstream rounds it* (`1.38e-23`), not the CODATA
/// value. Kept as-is so the port reproduces upstream numbers bit-for-bit.
const RELAP_UO2_BOLTZMANN: f64 = 1.38e-23; // J/K

// -- MATPRO-v11 (U,Pu)O2 (thermalExpansionMatproUPuO2.C) --------------------
// par1..par4: PuO2 end member; par5..par8: UO2 end member. Polynomials in
// degrees Celsius.
const MATPRO_UPUO2_PUO2: [f64; 4] = [-3.9735e-4, 8.4955e-6, 2.15130e-9, 3.7143e-16];
const MATPRO_UPUO2_UO2: [f64; 4] = [-4.972e-4, 7.107e-6, 2.581e-9, 1.140e-13];

// -- Martin (U,Pu)O2 (thermalExpansionMartinUPuO2.C) ------------------------
// par1..par4 below 923 K, par5..par8 above. Polynomials in kelvin, giving a
// MEAN coefficient; the strain is `alpha(T)*T`.
const MARTIN_LOW: [f64; 4] = [9.828e-6, -6.39e-10, 1.33e-12, -1.757e-17];
const MARTIN_HIGH: [f64; 4] = [1.1833e-5, -5.013e-9, 3.756e-12, -6.125e-17];
const MARTIN_BRANCH_TEMPERATURE: f64 = 923.0; // K
const MARTIN_STOICHIOMETRY_FACTOR: f64 = 3.98; // multiplies (2 - O/M)

// -- Lemehov (U,Pu)O2 (thermalExpansionLemehovUPuO2.C) ----------------------
const LEMEHOV_B: [f64; 4] = [-0.3080, 3.4303, -1.9157, 3.4636]; // percent
const LEMEHOV_BY: f64 = 3.98; // multiplies (2 - O/M)
                              // Magni melting-temperature correlation, doi:10.1016/j.jnucmat.2021.153312
const LEMEHOV_TM_A: f64 = 3147.0;
const LEMEHOV_TM_PU: f64 = 364.85;
const LEMEHOV_TM_OM: f64 = 1014.15;
const LEMEHOV_TM_AM: f64 = 329.5;
const LEMEHOV_TM_ASYMPTOTE: f64 = 2964.94;
const LEMEHOV_TM_BURNUP_SCALE: f64 = 24.25; // GWd/tHM

// -- MA-MOX (thermalExpansionMAMOX.C) ---------------------------------------
// Each polynomial coefficient a_i is itself a quadratic response surface in
// (cPu, x), with the leading decade factored out.
const MAMOX_A0: ([f64; 6], f64) = ([-2.8809, 0.0301, -4.3954, 0.0156, -15.1759, 2.5642], 1e-3);
const MAMOX_A1: ([f64; 6], f64) = ([9.5024, -0.1864, 15.8173, -0.0229, 7.6258, -7.5789], 1e-6);
const MAMOX_A2: ([f64; 6], f64) = ([2.0894, 2.9483, -19.9227, -1.0355, 73.8931, 11.6442], 1e-10);
const MAMOX_A3: ([f64; 6], f64) = ([4.4096, -1.4263, 23.5638, 0.0251, -54.751, -14.418], 1e-13);

// -- MATPRO Zircaloy (thermalExpansionMatproZy.C) ---------------------------
// NOTE: upstream declares par1/par2 (a separate axial fit) and par6, then
// comments out every use of them and assumes isotropy. Only the four constants
// actually reached by `setAlphaT` are ported.
const MATPRO_ZY_P3: f64 = 6.721e-6; // 1/K, alpha phase
const MATPRO_ZY_P4: f64 = 2.073e-3; // -,   alpha phase offset
const MATPRO_ZY_P5: f64 = 9.7e-6; // 1/K, beta phase
                                  // NOTE: upstream is internally inconsistent here — the constructor initialiser
                                  // list sets par7 = 9.4e-3 while the dictionary default one line later is
                                  // 9.45e-3. 9.4e-3 is what a case without a `thermalExpansion` sub-dictionary
                                  // actually gets, so that is what this port uses.
const MATPRO_ZY_P7: f64 = 9.4e-3; // -, beta phase offset
const MATPRO_ZY_T_ALPHA: f64 = 1073.0; // K, end of the alpha phase
const MATPRO_ZY_T_BETA: f64 = 1273.0; // K, start of the beta phase

// -- Gehr 15-15Ti (thermalExpansionGehr1515Ti.C) ----------------------------
// Polynomial in degrees Celsius, referenced to 20 °C.
const GEHR_1515TI: [f64; 3] = [-3.101e-4, 1.545e-5, 2.75e-9];
/// Upstream zeroes the 15-15Ti strain at or below this temperature (note the
/// bare `293`, not `293.15`, in `thermalExpansionGehr1515Ti.C`).
const GEHR_1515TI_CUTOFF: f64 = 293.0; // K

// -- Molybdenum (thermalExpansionMolybdenum.C) ------------------------------
const MOLYBDENUM_P1: f64 = 4.985e-6;
const MOLYBDENUM_P2: f64 = 6.667e-10;

// -- Snead SiC (thermalExpansionSneadSiC.C) ---------------------------------
const SNEAD_SIC: [f64; 4] = [-1.8276, 0.0178, -1.5544e-5, 4.5246e-9]; // x 1e-6
const SNEAD_SIC_HIGH_ALPHA: f64 = 5e-6; // 1/K, constant above 1273.15 K
const SNEAD_SIC_BRANCH_TEMPERATURE: f64 = 1273.15; // K
/// Reference temperature of Snead's **mean** expansion coefficient (25 °C).
///
/// Distinct from the case's stress-free temperature; see
/// [`ThermalExpansionModel::SneadSiC`].
const SNEAD_SIC_MEAN_REFERENCE: f64 = 298.15; // K

// -- Swindeman Hastelloy N (thermalExpansionSwindemanHastelloyN.C) ----------
// Polynomial in degrees Celsius, scaled by 1e-6.
const SWINDEMAN_HN: [f64; 3] = [0.005291, 9.682, 107.8];

// -- PARFUME buffer (thermalExpansionPARFUMEBuffer.C) -----------------------
const PARFUME_BUFFER: [f64; 4] = [5.0, 0.11, 400.0, 700.0]; // par1 scaled by 1e-6

// -- PARFUME pyrolytic carbon (thermalExpansionPARFUMEPyC.C) ----------------
const PARFUME_PYC: [f64; 6] = [30.0, 37.5, 0.11, 673.0, 700.0, 36.0]; // x 1e-6

/// Upstream default instantaneous linear expansion coefficient of SiC from the
/// PARFUME code \[1/K\]: `4.9e-6`, quoted directly in the upstream class
/// description of `thermalExpansionPARFUMESiC.H`.
pub const PARFUME_SIC_ALPHA: f64 = 4.9e-6;

/// Upstream default as-fabricated Bacon anisotropy factor (BAF) of pyrolytic
/// carbon \[-\]: `1.0`, i.e. fully isotropic PyC.
///
/// See [`ThermalExpansionModel::PARFUMEPyC`] for what the factor does.
pub const PARFUME_PYC_DEFAULT_ANISOTROPY: f64 = 1.0;

/// Approximate conversion factor from Pu atom fraction of the heavy metal to
/// Pu **mass** fraction of the fuel \[-\]: `1.13`.
///
/// Upstream computes it as `(MM_HM + 2*MM_O) / MM_Pu` with `MM_Pu ~ 239`,
/// `MM_HM ~ 238.5` and `MM_O = 16` g/mol, and applies it as
/// `c_mass = c_atom / 1.13` in both the MATPRO and the Lemehov MOX
/// correlations. It is approximate by upstream's own admission.
pub const PU_ATOM_TO_MASS_FRACTION: f64 = 1.13;

// ---------------------------------------------------------------------------
// The model enum
// ---------------------------------------------------------------------------

/// A published correlation for the thermal expansion of a fuel, cladding or
/// structural material.
///
/// Evaluate it with [`strain`](Self::strain) for the dimensionless thermal
/// strain `eps_th = dL/L0` \[-\] or [`coefficient`](Self::coefficient) for the
/// instantaneous linear expansion coefficient `alpha = d(eps_th)/dT` \[1/K\].
/// Read the [module documentation](self) first — the two are not
/// interchangeable.
///
/// # Choosing a variant
///
/// The variant names the **author or data source of the fit** and the material,
/// as the fuel-performance literature does. Two correlations for "MOX thermal
/// expansion" can differ by several per cent in strain, which is several
/// hundred MPa of cladding stress after gap closure, so the provenance is part
/// of the model, not a footnote.
///
/// | Variant | Material | Reference temperature |
/// |---|---|---|
/// | [`Constant`](Self::Constant) | any | caller-supplied |
/// | [`RelapUO2`](Self::RelapUO2) | UO2 | caller-supplied |
/// | [`MatproUPuO2`](Self::MatproUPuO2) | (U,Pu)O2 | fixed by the fit (~341 K) |
/// | [`MartinUPuO2`](Self::MartinUPuO2) | (U,Pu)O2 | caller-supplied |
/// | [`LemehovUPuO2`](Self::LemehovUPuO2) | (U,Pu)O2 | fixed by the fit |
/// | [`MAMOX`](Self::MAMOX) | minor-actinide MOX | caller-supplied |
/// | [`MatproZy`](Self::MatproZy) | Zircaloy | caller-supplied |
/// | [`Gehr1515Ti`](Self::Gehr1515Ti) | 15-15Ti steel | 293.15 K, fixed |
/// | [`Molybdenum`](Self::Molybdenum) | Mo | 273.15 K, fixed |
/// | [`SneadSiC`](Self::SneadSiC) | SiC | caller-supplied stress-free T |
/// | [`SwindemanHastelloyN`](Self::SwindemanHastelloyN) | Hastelloy N | caller-supplied |
/// | [`PARFUMEBuffer`](Self::PARFUMEBuffer) | TRISO buffer | caller-supplied |
/// | [`PARFUMEPyC`](Self::PARFUMEPyC) | TRISO pyrolytic carbon | caller-supplied |
/// | [`PARFUMESiC`](Self::PARFUMESiC) | TRISO SiC | caller-supplied |
///
/// # Example
///
/// ```
/// use outram_park_fork_offbeat::materials::MaterialState;
/// use outram_park_fork_offbeat::materials::properties::thermal_expansion::
///     ThermalExpansionModel;
///
/// // Zircaloy cladding, stress-free at 300 K, now sitting at 600 K.
/// let model = ThermalExpansionModel::MatproZy { t_ref: 300.0 };
/// let state = MaterialState::fresh(600.0);
///
/// let eps = model.strain(&state);       // dimensionless, ~2e-3
/// let alpha = model.coefficient(&state); // 1/K, ~6.7e-6
///
/// assert!(eps > 1e-3 && eps < 3e-3);
/// assert!(alpha > 5e-6 && alpha < 8e-6);
/// // The strain is ~300x the coefficient here; they are different quantities.
/// assert!(eps / alpha > 100.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThermalExpansionModel {
    /// Temperature-independent expansion coefficient — `eps_th = alpha *
    /// (T - t_ref)`.
    ///
    /// Upstream `thermalExpansionConstant`. Use when a case supplies a single
    /// engineering `alpha` and no fit is wanted, or as the null model in a
    /// sensitivity study.
    ///
    /// # Validity
    ///
    /// None. A user-supplied constant carries no fitted range, so
    /// [`validity_range`](Self::validity_range) reports an unbounded range and
    /// `*_checked` only rejects a non-positive temperature. The caller owns the
    /// question of where this `alpha` is meaningful.
    Constant {
        /// Instantaneous linear expansion coefficient \[1/K\]. Physically
        /// positive for essentially all solids; typically `1e-6` to `2e-5`.
        alpha: f64,
        /// Reference (stress-free) temperature \[K\] at which the strain is
        /// zero.
        t_ref: f64,
    },

    /// UO2 fuel, RELAP-derived fit with a Frenkel-defect term.
    ///
    /// `f(T) = K1*T - K2 + K3*exp(-E_D / (k*T))`, and `eps_th = f(T) -
    /// f(t_ref)`, with `K1 = 9.8e-6 1/K`, `K2 = 2.61e-3`, `K3 = 3.16e-1`,
    /// `E_D = 1.32e-19 J`, `k = 1.38e-23 J/K` (upstream's rounded Boltzmann
    /// constant is retained so results match upstream exactly).
    ///
    /// The exponential term is negligible below about 1500 K and turns upward
    /// steeply above 2000 K, which is the physical signature of Frenkel-pair
    /// formation in the oxygen sub-lattice near melting.
    ///
    /// Upstream `thermalExpansionRelapUO2`.
    ///
    /// # Validity
    ///
    /// **Upstream states no validity range and this port enforces none.** The
    /// fit describes solid UO2 and has no meaning above the melting point
    /// (~3120 K), but no bound is imposed here because none could be sourced
    /// from upstream. The caller carries the extrapolation risk.
    RelapUO2 {
        /// Reference (stress-free) temperature \[K\].
        t_ref: f64,
    },

    /// (U,Pu)O2 MOX fuel, MATPRO-v11 — cubic polynomials in °C for the PuO2 and
    /// UO2 end members, blended by Pu **mass** fraction.
    ///
    /// `eps_th = c_Pu * P_PuO2(T_C) + (1 - c_Pu) * P_UO2(T_C)` with
    /// `T_C = T - 273.15` and `c_Pu = pu_fraction / 1.13` (see
    /// [`PU_ATOM_TO_MASS_FRACTION`]).
    ///
    /// # The reference temperature is baked into the fit
    ///
    /// Unlike most variants here, upstream subtracts **nothing**: the
    /// polynomial is returned as the strain directly, so the reference
    /// temperature is wherever the polynomial happens to vanish — about
    /// **341 K** for pure UO2, and composition-dependent in general.
    /// [`reference_temperature`](Self::reference_temperature) therefore returns
    /// `None` for this variant. Do not mix its strain with another
    /// correlation's without accounting for that.
    ///
    /// Upstream `thermalExpansionMatproUPuO2`. Note that upstream's dictionary
    /// reader has a copy-paste defect (it reads `par5..par8` from the keys
    /// `par1..par4`); this port hard-codes the intended MATPRO coefficients and
    /// does not reproduce the defect.
    ///
    /// # Inputs used from [`MaterialState`]
    ///
    /// [`temperature`](MaterialState::temperature),
    /// [`pu_fraction`](MaterialState::pu_fraction). Upstream additionally
    /// discounts the Pu atom fraction by any Am and Np present; this port has
    /// no minor-actinide fields in [`MaterialState`] and so assumes none, which
    /// makes `pu_fraction` the Pu/(U+Pu) atom ratio exactly as documented on
    /// the field.
    ///
    /// # Validity
    ///
    /// **Upstream states no validity range and this port enforces none.**
    MatproUPuO2,

    /// (U,Pu)O2 MOX fuel, Martin (1988) review.
    ///
    /// D. G. Martin, *"The thermal expansion of solid UO2 and (U,Pu) mixed
    /// oxides — a review and recommendations"*, J. Nucl. Mater. 152 (1988),
    /// [doi:10.1016/0022-3115(88)90315-7](https://doi.org/10.1016/0022-3115(88)90315-7).
    ///
    /// Martin publishes a **mean** coefficient `alpha_m(T)` as a cubic in `T`,
    /// with separate coefficient sets below and above 923 K, scaled by
    /// `(1 + 3.98*(2 - O/M))` for hypostoichiometry. Upstream forms the strain
    /// as `alpha_m(T)*T - alpha_m(t_ref)*t_ref`, and this port reproduces that
    /// — including upstream's detail that the reference term always uses the
    /// **low-temperature** coefficient set regardless of `t_ref`. The
    /// instantaneous coefficient returned by [`coefficient`](Self::coefficient)
    /// is the correct derivative `alpha_m(T) + T * d(alpha_m)/dT`, which is
    /// **not** `alpha_m(T)`.
    ///
    /// The two coefficient sets agree to six significant figures at the 923 K
    /// branch point, so the strain is continuous there; the derivative is not.
    ///
    /// # Inputs used from [`MaterialState`]
    ///
    /// [`temperature`](MaterialState::temperature),
    /// [`oxygen_deviation`](MaterialState::oxygen_deviation) (as
    /// `2 - O/M = -oxygen_deviation`).
    ///
    /// # Validity
    ///
    /// **Upstream states no validity range and this port enforces none.**
    MartinUPuO2 {
        /// Reference (stress-free) temperature \[K\].
        t_ref: f64,
    },

    /// (U,Pu)O2 MOX fuel, Lemehov (2020) — strain as a cubic in the homologous
    /// temperature `T/T_melt`, with a burnup-dependent melting point.
    ///
    /// INSPYRE technical report WP7-D7.2 (2020),
    /// <https://re.public.polimi.it/handle/11311/1172415>. The melting
    /// temperature follows Magni et al.,
    /// [doi:10.1016/j.jnucmat.2021.153312](https://doi.org/10.1016/j.jnucmat.2021.153312):
    /// `T_m0 = 3147 - 364.85*c_Pu - 1014.15*(2 - O/M) - 329.5*c_Am` and
    /// `T_m = 2964.94 + (T_m0 - 2964.94)*exp(-Bu/24.25)` with burnup in
    /// GWd/tHM, which is numerically identical to
    /// [`MaterialState::burnup`] in MWd/kgHM.
    ///
    /// Then `eps_th = 0.01*(b0 + b1*r + b2*r^2 + b3*r^3) * (1 + 3.98*(2 - O/M))`
    /// with `r = T/T_m`. The leading `0.01` converts upstream's per-cent fit.
    ///
    /// # The reference temperature is baked into the fit
    ///
    /// As with [`MatproUPuO2`](Self::MatproUPuO2), upstream subtracts nothing,
    /// so the strain is zero wherever the cubic vanishes and
    /// [`reference_temperature`](Self::reference_temperature) returns `None`.
    ///
    /// # Inputs used from [`MaterialState`]
    ///
    /// [`temperature`](MaterialState::temperature),
    /// [`burnup`](MaterialState::burnup),
    /// [`pu_fraction`](MaterialState::pu_fraction),
    /// [`oxygen_deviation`](MaterialState::oxygen_deviation). The Am term of
    /// the melting-point correlation is evaluated with zero americium, because
    /// [`MaterialState`] carries no minor-actinide inventory; for MA-bearing
    /// fuel this over-predicts `T_m` and therefore under-predicts the strain.
    ///
    /// # Validity — stated upstream, and enforced
    ///
    /// The upstream class description gives the composition window explicitly:
    /// **O/M between 1.94 and 2.0** (i.e.
    /// [`oxygen_deviation`](MaterialState::oxygen_deviation) in `[-0.06, 0.0]`)
    /// and **Pu/HM below 60 %**. Both are checked by
    /// [`strain_checked`](Self::strain_checked). No temperature range is
    /// stated, so none is enforced.
    LemehovUPuO2,

    /// Minor-actinide-bearing MOX (MA-MOX), isotropic.
    ///
    /// J. Nucl. Mater. 469 (2016) 223-227,
    /// [doi:10.1016/j.jnucmat.2015.11.048](https://doi.org/10.1016/j.jnucmat.2015.11.048).
    ///
    /// A cubic in `T`, `P(T) = a0 + a1*T + a2*T^2 + a3*T^3`, where each `a_i`
    /// is itself a quadratic response surface in the Pu content `c_Pu` and the
    /// hypostoichiometry `x = 2 - O/M`. The strain is `P(T) - P(t_ref)`.
    ///
    /// # Inputs used from [`MaterialState`]
    ///
    /// [`temperature`](MaterialState::temperature),
    /// [`pu_fraction`](MaterialState::pu_fraction) (used directly as the atom
    /// fraction `c_Pu`, matching upstream's `ratioPuMetal` dictionary entry),
    /// [`oxygen_deviation`](MaterialState::oxygen_deviation).
    ///
    /// # Validity
    ///
    /// The upstream class description says the fit is *"for Pu = 0.3"*, yet the
    /// implementation retains the full `c_Pu` dependence of the response
    /// surface. No numerical composition window and no temperature range are
    /// stated, so **this port enforces none**; treat compositions far from
    /// `pu_fraction = 0.3` with suspicion.
    MAMOX {
        /// Reference (stress-free) temperature \[K\].
        t_ref: f64,
    },

    /// Zircaloy cladding, MATPRO-v11, with the alpha → beta phase transition.
    ///
    /// Three regimes, all isotropic (upstream carries a separate axial fit but
    /// has it commented out and assumes isotropy):
    ///
    /// - `T < 1073 K` (alpha phase): `s(T) = 6.721e-6*T - 2.073e-3`
    /// - `1073 <= T < 1273 K`: linear interpolation between the two branch
    ///   values, giving a **negative** apparent coefficient of about
    ///   `-1.1e-5 1/K` — the material genuinely contracts through the
    ///   alpha → beta transformation
    /// - `T >= 1273 K` (beta phase): `s(T) = 9.7e-6*T - 9.4e-3`
    ///
    /// and `eps_th = s(T) - s(t_ref)`.
    ///
    /// Upstream `thermalExpansionMatproZy`. (Upstream's class description says
    /// "UO2 fuel"; the code is unambiguously Zircaloy, and the variant is named
    /// for what the code does.)
    ///
    /// # Validity — stated upstream, and enforced
    ///
    /// **273 K to 1800 K.** Upstream warns outside `273 < T < 1800 K` (checking
    /// `T < 272.9 || T > 1801` to absorb rounding) and notes that the
    /// literature lower bound of 290 K was relaxed to 273 K so that contraction
    /// below room temperature can be modelled. Upstream then extrapolates
    /// anyway; this port clamps in [`strain`](Self::strain) and errors in
    /// [`strain_checked`](Self::strain_checked).
    MatproZy {
        /// Reference (stress-free) temperature \[K\].
        t_ref: f64,
    },

    /// 15-15Ti austenitic stainless cladding, Gehr (1973).
    ///
    /// `eps_th = -3.101e-4 + 1.545e-5*T_C + 2.75e-9*T_C^2` with
    /// `T_C = T - 273.15`, referenced to **20 °C = 293.15 K** (the quadratic
    /// vanishes there to within `1e-8`). The reference is fixed by the fit and
    /// cannot be moved.
    ///
    /// Upstream additionally forces the strain to exactly zero at or below
    /// 293 K, which this port reproduces; the strain is therefore discontinuous
    /// in its derivative at that point.
    ///
    /// Upstream `thermalExpansionGehr1515Ti`.
    ///
    /// # Validity
    ///
    /// Lower bound **293 K**, taken from upstream's own hard cut-off. Upstream
    /// states no upper bound and this port enforces none.
    Gehr1515Ti,

    /// Molybdenum structural material.
    ///
    /// `eps_th = (4.985e-6 + 6.667e-10*T) * (T - 273.15)` — a mean coefficient
    /// linear in `T`, multiplied by the rise above 273.15 K, so the reference
    /// temperature is fixed at **273.15 K**. The instantaneous coefficient is
    /// `4.985e-6 + 6.667e-10*(2T - 273.15)`, not the bracketed term.
    ///
    /// Upstream `thermalExpansionMolybdenum`.
    ///
    /// # Validity
    ///
    /// **Upstream states no validity range and this port enforces none.**
    Molybdenum,

    /// Silicon carbide, Snead handbook fit converted to instantaneous form.
    ///
    /// - L. L. Snead, T. Nozawa, Y. Katoh, T.-S. Byun, S. Kondo, D. A. Petti,
    ///   *"Handbook of SiC properties for fuel performance modeling"*,
    ///   J. Nucl. Mater. 371 (2007) 329-377.
    /// - M. Niffenegger, K. Reichlin, *"The proper use of thermal expansion
    ///   coefficients in finite element calculations"*, Nucl. Eng. Des. 243
    ///   (2012) 356-359 — the mean → instantaneous conversion.
    /// - B. P. Collin, J. Nucl. Mater. 451 (2014) 65-77 — use of the Snead fit
    ///   for UN TRISO.
    ///
    /// The **mean** coefficient is `alpha_m(T) = 1e-6*(-1.8276 + 0.0178*T -
    /// 1.5544e-5*T^2 + 4.5246e-9*T^3)` below 1273.15 K and a constant `5e-6
    /// 1/K` above (the two agree to 0.5 % at the branch). Niffenegger's
    /// conversion to a strain referenced to the stress-free temperature `T_sf`
    /// is
    ///
    /// `eps_th = [alpha_m(T)*(T - T_r) - alpha_m(T_sf)*(T_sf - T_r)] /
    /// [1 + alpha_m(T_sf)*(T_sf - T_r)]`
    ///
    /// where `T_r = 298.15 K` is the reference of the **mean** coefficient
    /// itself — a different thing from `T_sf`, and the reason this variant is
    /// the easiest one in the module to get wrong.
    ///
    /// Upstream `thermalExpansionSneadSiC`.
    ///
    /// # Validity
    ///
    /// **Upstream states no validity range and this port enforces none.** The
    /// 1273.15 K branch point is a change of functional form, not a validity
    /// bound.
    SneadSiC {
        /// Stress-free temperature `T_sf` \[K\] of the case — upstream's
        /// `Tref`, i.e. the temperature at which the component carries no
        /// thermal strain. Not to be confused with the fit's own 298.15 K mean
        /// reference.
        t_stress_free: f64,
    },

    /// Hastelloy N, Swindeman correlation.
    ///
    /// `f(T) = 1e-6*(0.005291*T_C^2 + 9.682*T_C + 107.8)` with
    /// `T_C = T - 273.15`, and `eps_th = f(T) - f(t_ref)`. The mean coefficient
    /// is about `1.35e-5 1/K` at 970 K, in the expected range for a
    /// nickel-based alloy.
    ///
    /// Upstream `thermalExpansionSwindemanHastelloyN`.
    ///
    /// # Validity
    ///
    /// **Upstream states no validity range and this port enforces none.**
    SwindemanHastelloyN {
        /// Reference (stress-free) temperature \[K\].
        t_ref: f64,
    },

    /// TRISO buffer layer (porous pyrolytic carbon), PARFUME correlation.
    ///
    /// `alpha(T) = 5e-6 * (1 + 0.11*(T_C - 400)/700)` with `T_C = T - 273.15`,
    /// applied as a **mean** coefficient: `eps_th = alpha(T)*(T - t_ref)`. The
    /// instantaneous coefficient adds the `(T - t_ref) * d(alpha)/dT` term.
    ///
    /// Upstream `thermalExpansionPARFUMEBuffer`.
    ///
    /// # Deviation from upstream
    ///
    /// Upstream's numerical dead zone here tests `strain < 1e-7` rather than
    /// `|strain| < 1e-7`, so it silently discards **all** contraction below the
    /// reference temperature. This port applies the magnitude test used
    /// everywhere else in the family, treating the unsigned comparison as an
    /// upstream defect. A case cooling below `t_ref` will therefore differ from
    /// upstream — deliberately.
    ///
    /// # Validity
    ///
    /// **Upstream states no validity range and this port enforces none.**
    PARFUMEBuffer {
        /// Reference (stress-free) temperature \[K\].
        t_ref: f64,
    },

    /// TRISO pyrolytic carbon layer (IPyC/OPyC), PARFUME correlation —
    /// **transversely isotropic**.
    ///
    /// Deposited PyC has a preferred crystallite orientation measured by the
    /// Bacon anisotropy factor (BAF). With `R_r = 2/(2 + BAF)` and
    /// `R_t = (1 + BAF)/(2 + BAF)`, the mean coefficients are
    ///
    /// - radial: `alpha_r = (30 - 37.5*R_r) * (1 + 0.11*(T - 673)/700) * 1e-6`
    /// - tangential: `alpha_t = (36*(R_t - 1)^2 + 1) * (1 + 0.11*(T - 673)/700) * 1e-6`
    ///
    /// and `eps_r = alpha_r*(T - t_ref)`, `eps_t = alpha_t*(T - t_ref)`.
    ///
    /// At `BAF = 1` (isotropic as-fabricated PyC) both reduce to the same
    /// `5e-6`-scaled expression — a useful self-check, and the reason
    /// [`PARFUME_PYC_DEFAULT_ANISOTROPY`] is 1.0.
    ///
    /// [`strain`](Self::strain) returns the isotropic-equivalent
    /// `(eps_r + 2*eps_t)/3`; use
    /// [`principal_strains`](Self::principal_strains) to get
    /// `[eps_r, eps_t, eps_t]` separately. Upstream also offers a rotation of
    /// the spherical-coordinate tensor into Cartesian components for
    /// non-1D cases; that is a *mesh* operation and belongs with the mechanics
    /// assembly, not with the correlation, so it is not ported here.
    ///
    /// Upstream `thermalExpansionPARFUMEPyC`.
    ///
    /// # Deviation from upstream
    ///
    /// The same unsigned dead-zone defect described under
    /// [`PARFUMEBuffer`](Self::PARFUMEBuffer) applies, and is likewise not
    /// reproduced.
    ///
    /// # Validity
    ///
    /// **Upstream states no validity range and this port enforces none.**
    PARFUMEPyC {
        /// Reference (stress-free) temperature \[K\].
        t_ref: f64,
        /// As-fabricated Bacon anisotropy factor (BAF) \[-\]. `1.0` is
        /// isotropic; deposited PyC in TRISO particles is typically 1.0-1.1.
        /// Must be greater than `-2` for the orientation factors to be finite.
        anisotropy_factor: f64,
    },

    /// TRISO silicon-carbide layer, PARFUME constant coefficient.
    ///
    /// `eps_th = alpha * (T - t_ref)` with upstream's default
    /// `alpha = 4.9e-6 1/K` ([`PARFUME_SIC_ALPHA`]), quoted directly in the
    /// upstream class description.
    ///
    /// Kept distinct from [`Constant`](Self::Constant) despite the identical
    /// algebra, because the provenance of the number is part of the model.
    ///
    /// Upstream `thermalExpansionPARFUMESiC`.
    ///
    /// # Validity
    ///
    /// **Upstream states no validity range and this port enforces none.**
    PARFUMESiC {
        /// Instantaneous linear expansion coefficient \[1/K\]; upstream default
        /// [`PARFUME_SIC_ALPHA`].
        alpha: f64,
        /// Reference (stress-free) temperature \[K\].
        t_ref: f64,
    },
}

impl ThermalExpansionModel {
    /// Human-readable name of the correlation, used in error messages.
    ///
    /// Stable enough to match on in a log, but not a parsing surface — build
    /// the enum directly rather than round-tripping through this string.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Constant { .. } => "constant thermal expansion",
            Self::RelapUO2 { .. } => "RELAP UO2 thermal expansion",
            Self::MatproUPuO2 => "MATPRO (U,Pu)O2 thermal expansion",
            Self::MartinUPuO2 { .. } => "Martin (U,Pu)O2 thermal expansion",
            Self::LemehovUPuO2 => "Lemehov (U,Pu)O2 thermal expansion",
            Self::MAMOX { .. } => "MA-MOX thermal expansion",
            Self::MatproZy { .. } => "MATPRO Zircaloy thermal expansion",
            Self::Gehr1515Ti => "Gehr 15-15Ti thermal expansion",
            Self::Molybdenum => "molybdenum thermal expansion",
            Self::SneadSiC { .. } => "Snead SiC thermal expansion",
            Self::SwindemanHastelloyN { .. } => "Swindeman Hastelloy N thermal expansion",
            Self::PARFUMEBuffer { .. } => "PARFUME buffer thermal expansion",
            Self::PARFUMEPyC { .. } => "PARFUME PyC thermal expansion",
            Self::PARFUMESiC { .. } => "PARFUME SiC thermal expansion",
        }
    }

    /// Temperature range \[K\] over which this port *enforces* the correlation,
    /// as `(low, high)`.
    ///
    /// The bound is taken from the upstream OFFBEAT source — a warning, a hard
    /// cut-off, or an explicit statement in the class description. Where
    /// upstream states no bound, this returns `(0.0, f64::INFINITY)`, meaning
    /// **no range is enforced and the caller carries the extrapolation risk**.
    /// That is a deliberate refusal to invent a bound, not an assertion that
    /// the fit is valid everywhere. Each variant's doc comment says which case
    /// it is in.
    ///
    /// [`strain`](Self::strain) and [`coefficient`](Self::coefficient) clamp
    /// the temperature to this range; [`strain_checked`](Self::strain_checked)
    /// and [`coefficient_checked`](Self::coefficient_checked) return
    /// [`OffbeatError::OutOfRange`] instead.
    #[must_use]
    pub fn validity_range(&self) -> (f64, f64) {
        match self {
            // Stated upstream: "273 < T < 1800 K" (thermalExpansionMatproZy.C).
            Self::MatproZy { .. } => (273.0, 1800.0),
            // Upstream hard-zeroes at or below 293 K.
            Self::Gehr1515Ti => (GEHR_1515TI_CUTOFF, f64::INFINITY),
            // No bound stated upstream; none invented here.
            _ => (0.0, f64::INFINITY),
        }
    }

    /// Reference temperature \[K\] at which this correlation's strain is zero,
    /// or `None` when the fit sets it implicitly.
    ///
    /// `None` means the strain vanishes wherever the fitted polynomial happens
    /// to vanish — near 341 K for [`MatproUPuO2`](Self::MatproUPuO2) with pure
    /// UO2, composition- and burnup-dependent for
    /// [`LemehovUPuO2`](Self::LemehovUPuO2). Strains from a `None` variant and
    /// from a `Some` variant are referenced to different states and must not be
    /// differenced.
    ///
    /// For [`SneadSiC`](Self::SneadSiC) this returns the **stress-free**
    /// temperature, which is the temperature of zero strain; the fit's separate
    /// 298.15 K mean-coefficient reference is an internal detail.
    #[must_use]
    pub fn reference_temperature(&self) -> Option<f64> {
        match self {
            Self::Constant { t_ref, .. }
            | Self::RelapUO2 { t_ref }
            | Self::MartinUPuO2 { t_ref }
            | Self::MAMOX { t_ref }
            | Self::MatproZy { t_ref }
            | Self::SwindemanHastelloyN { t_ref }
            | Self::PARFUMEBuffer { t_ref }
            | Self::PARFUMEPyC { t_ref, .. }
            | Self::PARFUMESiC { t_ref, .. } => Some(*t_ref),
            Self::SneadSiC { t_stress_free } => Some(*t_stress_free),
            Self::Gehr1515Ti => Some(293.15),
            Self::Molybdenum => Some(273.15),
            Self::MatproUPuO2 | Self::LemehovUPuO2 => None,
        }
    }

    /// Linear thermal strain `eps_th = dL/L0` \[**dimensionless**\].
    ///
    /// This is the *strain*, not the coefficient — see the
    /// [module documentation](self). Order `1e-3` to `3e-2` for fuel and
    /// cladding at operating temperature.
    ///
    /// For the one anisotropic variant, [`PARFUMEPyC`](Self::PARFUMEPyC), this
    /// returns the isotropic-equivalent linear strain, i.e. the mean of the
    /// three principal components (one third of the volumetric strain). Use
    /// [`principal_strains`](Self::principal_strains) for the components.
    ///
    /// # Clamping
    ///
    /// **The temperature is clamped to
    /// [`validity_range`](Self::validity_range) before evaluation**, so a
    /// caller outside the range silently gets the endpoint value rather than an
    /// extrapolation. Use [`strain_checked`](Self::strain_checked) to be told
    /// instead. A non-positive temperature is floored at `1e-3 K` purely to
    /// keep `1/T` terms finite; that is a numerical guard, not a validity
    /// statement, and `strain_checked` rejects such input outright.
    ///
    /// Strains whose three principal components are all below `1e-7` in
    /// magnitude are set to exactly zero, reproducing upstream's dead zone
    /// around the reference temperature.
    #[must_use]
    pub fn strain(&self, state: &MaterialState) -> f64 {
        mean(self.principal_strains(state))
    }

    /// Linear thermal strain \[-\], or [`OffbeatError`] if the correlation is
    /// being evaluated outside the range it was fitted over.
    ///
    /// Returns [`OffbeatError::Unphysical`] for a non-positive temperature and
    /// [`OffbeatError::OutOfRange`] outside
    /// [`validity_range`](Self::validity_range) — or, for
    /// [`LemehovUPuO2`](Self::LemehovUPuO2), outside its stated composition
    /// window. Unlike [`strain`](Self::strain) this never clamps: on success
    /// the value is the fit evaluated at the temperature you supplied.
    ///
    /// # Errors
    ///
    /// See above.
    pub fn strain_checked(&self, state: &MaterialState) -> Result<f64> {
        self.check(state)?;
        Ok(mean(dead_zone(
            self.principal_strain_raw(state.temperature, state),
        )))
    }

    /// The three principal linear thermal strains \[-\], as
    /// `[radial, tangential, tangential]`.
    ///
    /// Equal in all three components for every variant except
    /// [`PARFUMEPyC`](Self::PARFUMEPyC), which is transversely isotropic. The
    /// ordering follows upstream's spherical convention for TRISO layers:
    /// component 0 is radial, components 1 and 2 are the two tangential
    /// directions.
    ///
    /// Clamps and dead-zones exactly as [`strain`](Self::strain) does.
    #[must_use]
    pub fn principal_strains(&self, state: &MaterialState) -> [f64; 3] {
        let t = self.clamp_temperature(state.temperature);
        dead_zone(self.principal_strain_raw(t, state))
    }

    /// **Instantaneous** coefficient of linear thermal expansion
    /// `alpha = d(eps_th)/dT` \[**1/K**\].
    ///
    /// This is the coefficient a constitutive law wants when it forms an
    /// incremental thermal strain `d(eps_th) = alpha * dT`. It is *not* the
    /// mean coefficient `eps_th/(T - Tref)`, and for the correlations published
    /// in mean form ([`SneadSiC`](Self::SneadSiC),
    /// [`MartinUPuO2`](Self::MartinUPuO2),
    /// [`PARFUMEBuffer`](Self::PARFUMEBuffer),
    /// [`PARFUMEPyC`](Self::PARFUMEPyC), [`Molybdenum`](Self::Molybdenum)) the
    /// difference is a real physical term, not rounding.
    ///
    /// Typical magnitude `1e-6` to `2e-5 1/K`. It is negative for
    /// [`MatproZy`](Self::MatproZy) between 1073 K and 1273 K, where the
    /// alpha → beta transformation contracts the metal.
    ///
    /// Returned as the analytic derivative of the same expression
    /// [`strain`](Self::strain) evaluates, so the two are consistent by
    /// construction wherever the correlation is smooth. It is **not**
    /// dead-zoned: the dead zone is a hack for the strain near the reference
    /// temperature and would put a spurious zero in the coefficient.
    ///
    /// # Clamping
    ///
    /// The temperature is clamped to [`validity_range`](Self::validity_range),
    /// as for [`strain`](Self::strain). Use
    /// [`coefficient_checked`](Self::coefficient_checked) to be told instead.
    #[must_use]
    pub fn coefficient(&self, state: &MaterialState) -> f64 {
        mean(self.principal_coefficients(state))
    }

    /// Instantaneous linear expansion coefficient \[1/K\], or [`OffbeatError`]
    /// outside the correlation's validity range.
    ///
    /// The checked counterpart of [`coefficient`](Self::coefficient); the same
    /// conditions apply as for [`strain_checked`](Self::strain_checked).
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a non-positive temperature;
    /// [`OffbeatError::OutOfRange`] outside the enforced temperature or
    /// composition range.
    pub fn coefficient_checked(&self, state: &MaterialState) -> Result<f64> {
        self.check(state)?;
        Ok(mean(
            self.principal_coefficient_raw(state.temperature, state),
        ))
    }

    /// The three principal instantaneous expansion coefficients \[1/K\], as
    /// `[radial, tangential, tangential]`.
    ///
    /// The anisotropic counterpart of [`coefficient`](Self::coefficient); see
    /// [`principal_strains`](Self::principal_strains) for the component
    /// convention.
    #[must_use]
    pub fn principal_coefficients(&self, state: &MaterialState) -> [f64; 3] {
        let t = self.clamp_temperature(state.temperature);
        self.principal_coefficient_raw(t, state)
    }

    // -- internals ----------------------------------------------------------

    /// Clamp to the enforced validity range, with a positive floor so `1/T`
    /// terms stay finite.
    fn clamp_temperature(&self, t: f64) -> f64 {
        let (low, high) = self.validity_range();
        t.clamp(low, high).max(MIN_EVAL_TEMPERATURE)
    }

    /// Validate temperature and, where upstream states one, composition.
    fn check(&self, state: &MaterialState) -> Result<()> {
        let t = state.temperature;
        if !(t > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: self.name(),
                value: t,
                unit: "K",
                reason: "absolute temperature must be strictly positive",
            });
        }

        let (low, high) = self.validity_range();
        if t < low || t > high {
            return Err(OffbeatError::OutOfRange {
                quantity: self.name(),
                value: t,
                low,
                high,
                unit: "K",
            });
        }

        // Lemehov is the only variant whose upstream description states a
        // composition window.
        if matches!(self, Self::LemehovUPuO2) {
            let om_ratio = 2.0 + state.oxygen_deviation;
            if !(1.94..=2.0).contains(&om_ratio) {
                return Err(OffbeatError::OutOfRange {
                    quantity: "Lemehov (U,Pu)O2 thermal expansion (O/M ratio)",
                    value: om_ratio,
                    low: 1.94,
                    high: 2.0,
                    unit: "-",
                });
            }
            if !(0.0..=0.6).contains(&state.pu_fraction) {
                return Err(OffbeatError::OutOfRange {
                    quantity: "Lemehov (U,Pu)O2 thermal expansion (Pu/HM ratio)",
                    value: state.pu_fraction,
                    low: 0.0,
                    high: 0.6,
                    unit: "-",
                });
            }
        }

        Ok(())
    }

    /// Principal strains before the dead zone is applied, at an explicit
    /// temperature.
    fn principal_strain_raw(&self, t: f64, state: &MaterialState) -> [f64; 3] {
        match self {
            Self::Constant { alpha, t_ref } => iso(alpha * (t - t_ref)),

            Self::RelapUO2 { t_ref } => iso(relap_uo2(t) - relap_uo2(*t_ref)),

            Self::MatproUPuO2 => {
                let c_pu = state.pu_fraction / PU_ATOM_TO_MASS_FRACTION;
                let tc = t - 273.15;
                let puo2 = cubic(&MATPRO_UPUO2_PUO2, tc);
                let uo2 = cubic(&MATPRO_UPUO2_UO2, tc);
                iso(c_pu * puo2 + (1.0 - c_pu) * uo2)
            }

            Self::MartinUPuO2 { t_ref } => {
                let x = -state.oxygen_deviation; // = 2 - O/M
                                                 // Faithful to upstream: the reference term always uses the
                                                 // low-temperature coefficient set, whatever t_ref is.
                let alpha_ref = cubic(&MARTIN_LOW, *t_ref) * martin_stoichiometry(x);
                iso(martin_mean_alpha(t, x) * t - alpha_ref * t_ref)
            }

            Self::LemehovUPuO2 => iso(lemehov_strain(t, state)),

            Self::MAMOX { t_ref } => {
                let c_pu = state.pu_fraction;
                let x = -state.oxygen_deviation;
                iso(mamox_poly(t, c_pu, x) - mamox_poly(*t_ref, c_pu, x))
            }

            Self::MatproZy { t_ref } => iso(matpro_zy(t) - matpro_zy(*t_ref)),

            Self::Gehr1515Ti => {
                if t <= GEHR_1515TI_CUTOFF {
                    iso(0.0)
                } else {
                    let tc = t - 273.15;
                    iso(GEHR_1515TI[0] + GEHR_1515TI[1] * tc + GEHR_1515TI[2] * tc * tc)
                }
            }

            Self::Molybdenum => iso((MOLYBDENUM_P1 + MOLYBDENUM_P2 * t) * (t - 273.15)),

            Self::SneadSiC { t_stress_free } => iso(snead_sic(t, *t_stress_free)),

            Self::SwindemanHastelloyN { t_ref } => iso(swindeman(t) - swindeman(*t_ref)),

            Self::PARFUMEBuffer { t_ref } => iso(parfume_buffer_alpha(t) * (t - t_ref)),

            Self::PARFUMEPyC {
                t_ref,
                anisotropy_factor,
            } => {
                let (alpha_r, alpha_t) = parfume_pyc_alphas(t, *anisotropy_factor);
                let dt = t - t_ref;
                [alpha_r * dt, alpha_t * dt, alpha_t * dt]
            }

            Self::PARFUMESiC { alpha, t_ref } => iso(alpha * (t - t_ref)),
        }
    }

    /// Analytic `d(eps_th)/dT` of [`Self::principal_strain_raw`].
    fn principal_coefficient_raw(&self, t: f64, state: &MaterialState) -> [f64; 3] {
        match self {
            Self::Constant { alpha, .. } => iso(*alpha),

            Self::RelapUO2 { .. } => iso(d_relap_uo2(t)),

            Self::MatproUPuO2 => {
                let c_pu = state.pu_fraction / PU_ATOM_TO_MASS_FRACTION;
                let tc = t - 273.15;
                let d_puo2 = d_cubic(&MATPRO_UPUO2_PUO2, tc);
                let d_uo2 = d_cubic(&MATPRO_UPUO2_UO2, tc);
                iso(c_pu * d_puo2 + (1.0 - c_pu) * d_uo2)
            }

            Self::MartinUPuO2 { .. } => {
                let x = -state.oxygen_deviation;
                // d/dT [alpha_m(T) * T] = alpha_m(T) + T * d(alpha_m)/dT
                let alpha = martin_mean_alpha(t, x);
                let coeffs = if t <= MARTIN_BRANCH_TEMPERATURE {
                    &MARTIN_LOW
                } else {
                    &MARTIN_HIGH
                };
                let d_alpha = d_cubic(coeffs, t) * martin_stoichiometry(x);
                iso(alpha + t * d_alpha)
            }

            Self::LemehovUPuO2 => iso(d_lemehov_strain(t, state)),

            Self::MAMOX { .. } => {
                let c_pu = state.pu_fraction;
                let x = -state.oxygen_deviation;
                iso(d_mamox_poly(t, c_pu, x))
            }

            Self::MatproZy { .. } => iso(d_matpro_zy(t)),

            Self::Gehr1515Ti => {
                if t <= GEHR_1515TI_CUTOFF {
                    iso(0.0)
                } else {
                    let tc = t - 273.15;
                    iso(GEHR_1515TI[1] + 2.0 * GEHR_1515TI[2] * tc)
                }
            }

            Self::Molybdenum => iso(MOLYBDENUM_P1 + MOLYBDENUM_P2 * (2.0 * t - 273.15)),

            Self::SneadSiC { t_stress_free } => iso(d_snead_sic(t, *t_stress_free)),

            Self::SwindemanHastelloyN { .. } => iso(d_swindeman(t)),

            Self::PARFUMEBuffer { t_ref } => {
                // d/dT [alpha(T) * (T - Tref)] = alpha(T) + (T - Tref)*dalpha/dT
                let d_alpha = PARFUME_BUFFER[0] * PARFUME_BUFFER[1] / PARFUME_BUFFER[3] * 1e-6;
                iso(parfume_buffer_alpha(t) + (t - t_ref) * d_alpha)
            }

            Self::PARFUMEPyC {
                t_ref,
                anisotropy_factor,
            } => {
                let (alpha_r, alpha_t) = parfume_pyc_alphas(t, *anisotropy_factor);
                let (d_r, d_t) = d_parfume_pyc_alphas(*anisotropy_factor);
                let dt = t - t_ref;
                [alpha_r + dt * d_r, alpha_t + dt * d_t, alpha_t + dt * d_t]
            }

            Self::PARFUMESiC { alpha, .. } => iso(*alpha),
        }
    }
}

// ---------------------------------------------------------------------------
// Free helpers — one per correlation, so each fit reads as its published form.
// ---------------------------------------------------------------------------

/// Three equal principal components.
fn iso(v: f64) -> [f64; 3] {
    [v, v, v]
}

/// Isotropic-equivalent scalar: the mean of the three principal components.
fn mean(v: [f64; 3]) -> f64 {
    (v[0] + v[1] + v[2]) / 3.0
}

/// Upstream's dead zone: zero the whole tensor when every component is tiny.
fn dead_zone(v: [f64; 3]) -> [f64; 3] {
    if v[0].abs() < STRAIN_DEAD_ZONE
        && v[1].abs() < STRAIN_DEAD_ZONE
        && v[2].abs() < STRAIN_DEAD_ZONE
    {
        [0.0; 3]
    } else {
        v
    }
}

/// `c0 + c1*x + c2*x^2 + c3*x^3`.
fn cubic(c: &[f64; 4], x: f64) -> f64 {
    c[0] + x * (c[1] + x * (c[2] + x * c[3]))
}

/// `d/dx` of [`cubic`].
fn d_cubic(c: &[f64; 4], x: f64) -> f64 {
    c[1] + x * (2.0 * c[2] + x * 3.0 * c[3])
}

/// RELAP UO2 expansion function `K1*T - K2 + K3*exp(-E_D/(k*T))` \[-\].
fn relap_uo2(t: f64) -> f64 {
    RELAP_UO2_K1 * t - RELAP_UO2_K2
        + RELAP_UO2_K3 * (-RELAP_UO2_ED / (RELAP_UO2_BOLTZMANN * t)).exp()
}

/// `d/dT` of [`relap_uo2`] \[1/K\].
fn d_relap_uo2(t: f64) -> f64 {
    let e_over_k = RELAP_UO2_ED / RELAP_UO2_BOLTZMANN;
    RELAP_UO2_K1 + RELAP_UO2_K3 * (-e_over_k / t).exp() * e_over_k / (t * t)
}

/// Martin's hypostoichiometry multiplier `1 + 3.98*(2 - O/M)`.
fn martin_stoichiometry(x: f64) -> f64 {
    1.0 + MARTIN_STOICHIOMETRY_FACTOR * x
}

/// Martin's **mean** linear expansion coefficient \[1/K\].
fn martin_mean_alpha(t: f64, x: f64) -> f64 {
    let coeffs = if t <= MARTIN_BRANCH_TEMPERATURE {
        &MARTIN_LOW
    } else {
        &MARTIN_HIGH
    };
    cubic(coeffs, t) * martin_stoichiometry(x)
}

/// Magni burnup- and composition-dependent melting temperature \[K\] used by
/// the Lemehov correlation.
fn lemehov_melting_temperature(state: &MaterialState) -> f64 {
    let x = -state.oxygen_deviation; // = 2 - O/M
    let c_pu = state.pu_fraction / PU_ATOM_TO_MASS_FRACTION;
    // MaterialState carries no minor-actinide inventory, so c_Am = 0.
    let c_am = 0.0;
    let tm0 = LEMEHOV_TM_A - LEMEHOV_TM_PU * c_pu - LEMEHOV_TM_OM * x - LEMEHOV_TM_AM * c_am;
    LEMEHOV_TM_ASYMPTOTE
        + (tm0 - LEMEHOV_TM_ASYMPTOTE) * (-state.burnup / LEMEHOV_TM_BURNUP_SCALE).exp()
}

/// Lemehov linear thermal strain \[-\].
fn lemehov_strain(t: f64, state: &MaterialState) -> f64 {
    let tm = lemehov_melting_temperature(state);
    let x = -state.oxygen_deviation;
    let r = t / tm;
    0.01 * cubic(&LEMEHOV_B, r) * (1.0 + LEMEHOV_BY * x)
}

/// `d/dT` of [`lemehov_strain`] \[1/K\].
fn d_lemehov_strain(t: f64, state: &MaterialState) -> f64 {
    let tm = lemehov_melting_temperature(state);
    let x = -state.oxygen_deviation;
    let r = t / tm;
    0.01 * d_cubic(&LEMEHOV_B, r) / tm * (1.0 + LEMEHOV_BY * x)
}

/// One MA-MOX response-surface coefficient `a_i(c_Pu, x)`.
fn mamox_coefficient(surface: &([f64; 6], f64), c_pu: f64, x: f64) -> f64 {
    let (b, scale) = surface;
    scale * (b[0] + b[1] * c_pu + b[2] * x + b[3] * c_pu * c_pu + b[4] * x * x + b[5] * c_pu * x)
}

/// MA-MOX expansion polynomial `a0 + a1*T + a2*T^2 + a3*T^3` \[-\].
fn mamox_poly(t: f64, c_pu: f64, x: f64) -> f64 {
    let a = [
        mamox_coefficient(&MAMOX_A0, c_pu, x),
        mamox_coefficient(&MAMOX_A1, c_pu, x),
        mamox_coefficient(&MAMOX_A2, c_pu, x),
        mamox_coefficient(&MAMOX_A3, c_pu, x),
    ];
    cubic(&a, t)
}

/// `d/dT` of [`mamox_poly`] \[1/K\].
fn d_mamox_poly(t: f64, c_pu: f64, x: f64) -> f64 {
    let a = [
        mamox_coefficient(&MAMOX_A0, c_pu, x),
        mamox_coefficient(&MAMOX_A1, c_pu, x),
        mamox_coefficient(&MAMOX_A2, c_pu, x),
        mamox_coefficient(&MAMOX_A3, c_pu, x),
    ];
    d_cubic(&a, t)
}

/// MATPRO Zircaloy expansion function \[-\], with the alpha/beta transition.
fn matpro_zy(t: f64) -> f64 {
    if t < MATPRO_ZY_T_ALPHA {
        MATPRO_ZY_P3 * t - MATPRO_ZY_P4
    } else if t < MATPRO_ZY_T_BETA {
        let s_alpha = MATPRO_ZY_P3 * MATPRO_ZY_T_ALPHA - MATPRO_ZY_P4;
        let s_beta = MATPRO_ZY_P5 * MATPRO_ZY_T_BETA - MATPRO_ZY_P7;
        let w = (t - MATPRO_ZY_T_ALPHA) / (MATPRO_ZY_T_BETA - MATPRO_ZY_T_ALPHA);
        s_alpha * (1.0 - w) + s_beta * w
    } else {
        MATPRO_ZY_P5 * t - MATPRO_ZY_P7
    }
}

/// `d/dT` of [`matpro_zy`] \[1/K\]. Negative through the phase transition.
fn d_matpro_zy(t: f64) -> f64 {
    if t < MATPRO_ZY_T_ALPHA {
        MATPRO_ZY_P3
    } else if t < MATPRO_ZY_T_BETA {
        let s_alpha = MATPRO_ZY_P3 * MATPRO_ZY_T_ALPHA - MATPRO_ZY_P4;
        let s_beta = MATPRO_ZY_P5 * MATPRO_ZY_T_BETA - MATPRO_ZY_P7;
        (s_beta - s_alpha) / (MATPRO_ZY_T_BETA - MATPRO_ZY_T_ALPHA)
    } else {
        MATPRO_ZY_P5
    }
}

/// Snead's **mean** SiC expansion coefficient \[1/K\], referenced to 298.15 K.
fn snead_mean_alpha(t: f64) -> f64 {
    if t < SNEAD_SIC_BRANCH_TEMPERATURE {
        1e-6 * cubic(&SNEAD_SIC, t)
    } else {
        SNEAD_SIC_HIGH_ALPHA
    }
}

/// `d/dT` of [`snead_mean_alpha`] \[1/K^2\].
fn d_snead_mean_alpha(t: f64) -> f64 {
    if t < SNEAD_SIC_BRANCH_TEMPERATURE {
        1e-6 * d_cubic(&SNEAD_SIC, t)
    } else {
        0.0
    }
}

/// Niffenegger conversion of Snead's mean coefficient to a strain referenced to
/// the stress-free temperature \[-\].
fn snead_sic(t: f64, t_sf: f64) -> f64 {
    let alpha = snead_mean_alpha(t);
    let alpha_sf = 1e-6 * cubic(&SNEAD_SIC, t_sf);
    let offset = alpha_sf * (t_sf - SNEAD_SIC_MEAN_REFERENCE);
    (alpha * (t - SNEAD_SIC_MEAN_REFERENCE) - offset) / (1.0 + offset)
}

/// `d/dT` of [`snead_sic`] \[1/K\].
fn d_snead_sic(t: f64, t_sf: f64) -> f64 {
    let alpha_sf = 1e-6 * cubic(&SNEAD_SIC, t_sf);
    let offset = alpha_sf * (t_sf - SNEAD_SIC_MEAN_REFERENCE);
    let numerator = snead_mean_alpha(t) + (t - SNEAD_SIC_MEAN_REFERENCE) * d_snead_mean_alpha(t);
    numerator / (1.0 + offset)
}

/// Swindeman Hastelloy N expansion function \[-\].
fn swindeman(t: f64) -> f64 {
    let tc = t - 273.15;
    1e-6 * (SWINDEMAN_HN[0] * tc * tc + SWINDEMAN_HN[1] * tc + SWINDEMAN_HN[2])
}

/// `d/dT` of [`swindeman`] \[1/K\].
fn d_swindeman(t: f64) -> f64 {
    let tc = t - 273.15;
    1e-6 * (2.0 * SWINDEMAN_HN[0] * tc + SWINDEMAN_HN[1])
}

/// PARFUME buffer **mean** expansion coefficient \[1/K\].
fn parfume_buffer_alpha(t: f64) -> f64 {
    let tc = t - 273.15;
    PARFUME_BUFFER[0]
        * (1.0 + PARFUME_BUFFER[1] * (tc - PARFUME_BUFFER[2]) / PARFUME_BUFFER[3])
        * 1e-6
}

/// PARFUME PyC **mean** radial and tangential expansion coefficients \[1/K\].
fn parfume_pyc_alphas(t: f64, baf: f64) -> (f64, f64) {
    let r_r = 2.0 / (2.0 + baf);
    let r_t = (1.0 + baf) / (2.0 + baf);
    let temperature_factor = 1.0 + PARFUME_PYC[2] * (t - PARFUME_PYC[3]) / PARFUME_PYC[4];
    let alpha_r = (PARFUME_PYC[0] - PARFUME_PYC[1] * r_r) * temperature_factor * 1e-6;
    let alpha_t = (PARFUME_PYC[5] * (r_t - 1.0).powi(2) + 1.0) * temperature_factor * 1e-6;
    (alpha_r, alpha_t)
}

/// `d/dT` of [`parfume_pyc_alphas`] \[1/K^2\] — independent of temperature.
fn d_parfume_pyc_alphas(baf: f64) -> (f64, f64) {
    let r_r = 2.0 / (2.0 + baf);
    let r_t = (1.0 + baf) / (2.0 + baf);
    let slope = PARFUME_PYC[2] / PARFUME_PYC[4];
    (
        (PARFUME_PYC[0] - PARFUME_PYC[1] * r_r) * slope * 1e-6,
        (PARFUME_PYC[5] * (r_t - 1.0).powi(2) + 1.0) * slope * 1e-6,
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant, built with representative parameters, for the sweeps
    /// below. `t_ref = 300 K` throughout so that test temperatures sit well
    /// outside upstream's `1e-7` dead zone.
    fn all_models() -> Vec<ThermalExpansionModel> {
        vec![
            ThermalExpansionModel::Constant {
                alpha: 1.0e-5,
                t_ref: 300.0,
            },
            ThermalExpansionModel::RelapUO2 { t_ref: 300.0 },
            ThermalExpansionModel::MatproUPuO2,
            ThermalExpansionModel::MartinUPuO2 { t_ref: 300.0 },
            ThermalExpansionModel::LemehovUPuO2,
            ThermalExpansionModel::MAMOX { t_ref: 300.0 },
            ThermalExpansionModel::MatproZy { t_ref: 300.0 },
            ThermalExpansionModel::Gehr1515Ti,
            ThermalExpansionModel::Molybdenum,
            ThermalExpansionModel::SneadSiC {
                t_stress_free: 300.0,
            },
            ThermalExpansionModel::SwindemanHastelloyN { t_ref: 300.0 },
            ThermalExpansionModel::PARFUMEBuffer { t_ref: 300.0 },
            ThermalExpansionModel::PARFUMEPyC {
                t_ref: 300.0,
                anisotropy_factor: PARFUME_PYC_DEFAULT_ANISOTROPY,
            },
            ThermalExpansionModel::PARFUMESiC {
                alpha: PARFUME_SIC_ALPHA,
                t_ref: 300.0,
            },
        ]
    }

    /// A MOX state inside every stated composition window (O/M = 1.98,
    /// Pu/HM = 0.20, 30 MWd/kgHM).
    fn mox_state(temperature: f64) -> MaterialState {
        let mut s = MaterialState::fresh(temperature);
        s.pu_fraction = 0.20;
        s.oxygen_deviation = -0.02; // O/M = 1.98
        s.burnup = 30.0;
        s
    }

    fn numerical_coefficient(model: &ThermalExpansionModel, state: &MaterialState, h: f64) -> f64 {
        let mut hot = *state;
        hot.temperature += h;
        let mut cold = *state;
        cold.temperature -= h;
        (model.strain(&hot) - model.strain(&cold)) / (2.0 * h)
    }

    // -- Verification against the upstream C++ expressions -------------------

    /// **Reference-checked** against the upstream C++ source.
    ///
    /// *Methodology.* `thermalExpansionMatproZy.C::setAlphaT` gives, in the
    /// alpha phase (`T < 1073 K`), `alphaT = par3*T - par4` with
    /// `par3 = 6.721e-6 1/K` and `par4 = 2.073e-3`, and `correct()` returns
    /// `setAlphaT(T) - setAlphaT(Tref)`. Hand-evaluating at `T = 500 K`,
    /// `Tref = 300 K`:
    /// `s(500) = 3.36050e-3 - 2.073e-3 = 1.28750e-3`;
    /// `s(300) = 2.01630e-3 - 2.073e-3 = -5.6700e-5`;
    /// `eps_th = 1.344200e-3`. Pass criterion: agreement to `1e-12`
    /// (round-off only — the port must reproduce the expression exactly, not
    /// approximately).
    ///
    /// *Result.* `eps_th = 1.3442000000e-3`, difference from the hand
    /// evaluation `< 1e-15`. Measured 2026-07-29 against upstream commit
    /// 80e84450a115b0c411e1bfa5d166379f6bf6c084.
    ///
    /// This verifies the port against the upstream implementation. It is
    /// **not** validation against experiment.
    #[test]
    fn matpro_zy_alpha_phase_matches_upstream_hand_evaluation() {
        let model = ThermalExpansionModel::MatproZy { t_ref: 300.0 };
        let eps = model.strain(&MaterialState::fresh(500.0));
        let expected = (6.721e-6 * 500.0 - 2.073e-3) - (6.721e-6 * 300.0 - 2.073e-3);
        assert!((eps - expected).abs() < 1e-12, "eps = {eps:e}");
        assert!((eps - 1.3442e-3).abs() < 1e-12, "eps = {eps:e}");
    }

    /// **Reference-checked** against the upstream C++ source.
    ///
    /// *Methodology.* The alpha-phase instantaneous coefficient is the slope
    /// `par3 = 6.721e-6 1/K` exactly, and the beta-phase slope is
    /// `par5 = 9.7e-6 1/K`. Through the 1073-1273 K transition the upstream
    /// interpolation gives the chord slope
    /// `(par5*1273 - par7 - (par3*1073 - par4)) / 200`, with `par7 = 9.4e-3`
    /// (the constructor's initialiser-list default) — i.e.
    /// `(2.948e-3 - 5.1386e-3)/200 = -1.09530e-5 1/K`. Pass criterion:
    /// `1e-15` absolute.
    ///
    /// *Result.* alpha phase `6.721000e-6 1/K`, transition
    /// `-1.095300e-5 1/K`, beta phase `9.700000e-6 1/K`; all within `1e-15` of
    /// the upstream expressions. The negative transition value is the physical
    /// contraction of Zr through the alpha → beta transformation, and is the
    /// clearest demonstration in this module that a "thermal expansion
    /// coefficient" may legitimately be negative.
    #[test]
    fn matpro_zy_coefficient_matches_upstream_branch_slopes() {
        let model = ThermalExpansionModel::MatproZy { t_ref: 300.0 };
        let alpha_phase = model.coefficient(&MaterialState::fresh(500.0));
        assert!((alpha_phase - 6.721e-6).abs() < 1e-15);

        let beta_phase = model.coefficient(&MaterialState::fresh(1400.0));
        assert!((beta_phase - 9.7e-6).abs() < 1e-15);

        let transition = model.coefficient(&MaterialState::fresh(1150.0));
        let expected = ((9.7e-6 * 1273.0 - 9.4e-3) - (6.721e-6 * 1073.0 - 2.073e-3)) / 200.0;
        assert!((transition - expected).abs() < 1e-15, "{transition:e}");
        assert!(transition < 0.0, "Zr contracts through alpha -> beta");
    }

    /// **Reference-checked** against the upstream class description.
    ///
    /// *Methodology.* `thermalExpansionPARFUMESiC.H` states the SiC
    /// coefficient as "a constant value 4.9x10^-6 /K from PARFUME", and the
    /// constructor's default for `thermalExpansionCoefficient` is `4.9e-6`.
    /// The ported [`PARFUME_SIC_ALPHA`] must equal that, and the model must
    /// return it as the instantaneous coefficient at any temperature. Pass
    /// criterion: exact equality.
    ///
    /// *Result.* `PARFUME_SIC_ALPHA = 4.9e-6 1/K`, returned unchanged at 300 K,
    /// 1200 K and 2000 K.
    #[test]
    fn parfume_sic_coefficient_matches_upstream_constant() {
        assert_eq!(PARFUME_SIC_ALPHA, 4.9e-6);
        let model = ThermalExpansionModel::PARFUMESiC {
            alpha: PARFUME_SIC_ALPHA,
            t_ref: 300.0,
        };
        for t in [300.0, 1200.0, 2000.0] {
            assert_eq!(model.coefficient(&MaterialState::fresh(t)), 4.9e-6);
        }
    }

    /// **Reference-checked** against the upstream C++ source.
    ///
    /// *Methodology.* `thermalExpansionRelapUO2.C` computes
    /// `K1*T - K2 + K3*exp(-ED/k/T)` with `K1 = 9.8e-6`, `K2 = 2.61e-3`,
    /// `K3 = 3.16e-1`, `ED = 1.32e-19 J`, `k = 1.38e-23 J/K`, and subtracts the
    /// same at `Tref`. Recomputed independently here at `T = 1000 K`,
    /// `Tref = 300 K`. Pass criterion: `1e-15` relative.
    ///
    /// *Result.* `eps_th = 6.882159805861141e-3` at 1000 K, matching the
    /// independent evaluation to `< 1e-18`.
    ///
    /// The Frenkel-defect term behaves as the physics requires: it contributes
    /// `4.49e-15` at 300 K, `2.22e-5` at 1000 K (0.3 % of the strain) and
    /// `1.04e-2` at 2800 K (comparable to the whole linear term), a factor of
    /// 468 rise over the last 1800 K. That steep upturn near melting is the
    /// signature of oxygen-sublattice Frenkel-pair formation, and it is the
    /// reason the fit is not simply linear.
    #[test]
    fn relap_uo2_matches_upstream_hand_evaluation() {
        let model = ThermalExpansionModel::RelapUO2 { t_ref: 300.0 };
        let eps = model.strain(&MaterialState::fresh(1000.0));

        let f = |t: f64| 9.8e-6 * t - 2.61e-3 + 3.16e-1 * (-1.32e-19 / (1.38e-23 * t)).exp();
        let expected = f(1000.0) - f(300.0);
        assert!((eps - expected).abs() < 1e-18, "eps = {eps:e}");
        assert!(
            (eps - 6.882_159_805_861_141e-3).abs() < 1e-15,
            "eps = {eps:e}"
        );

        // Shape of the defect term: negligible at 300 K, small but real at
        // 1000 K, dominant near melting.
        let defect = |t: f64| 3.16e-1 * (-1.32e-19 / (1.38e-23 * t)).exp();
        assert!(defect(300.0) < 1e-12);
        assert!((defect(1000.0) - 2.215_981e-5).abs() < 1e-10);
        assert!((defect(2800.0) - 1.037_701e-2).abs() < 1e-7);
        assert!(defect(2800.0) / defect(1000.0) > 100.0);
    }

    /// **Reference-checked** against the upstream stated validity range.
    ///
    /// *Methodology.* `thermalExpansionMatproZy.C::setAlphaT` warns when
    /// `T < 272.9 || T > 1801`, describing the range as `273 < T < 1800 K`.
    /// This port enforces `[273, 1800] K`. Pass criterion: `strain_checked`
    /// succeeds at both endpoints and returns
    /// [`OffbeatError::OutOfRange`] with those bounds just outside them; the
    /// plain `strain` clamps instead of extrapolating.
    ///
    /// *Result.* Ok at 273 K and 1800 K; `OutOfRange { low: 273, high: 1800,
    /// unit: "K" }` at 272 K and 1801 K; `strain(2000 K) == strain(1800 K)`
    /// exactly, confirming the clamp.
    #[test]
    fn matpro_zy_enforces_the_upstream_validity_range() {
        let model = ThermalExpansionModel::MatproZy { t_ref: 300.0 };

        assert!(model.strain_checked(&MaterialState::fresh(273.0)).is_ok());
        assert!(model.strain_checked(&MaterialState::fresh(1800.0)).is_ok());

        let too_cold = model.strain_checked(&MaterialState::fresh(272.0));
        assert!(matches!(
            too_cold,
            Err(OffbeatError::OutOfRange {
                low, high, unit: "K", ..
            }) if low == 273.0 && high == 1800.0
        ));
        assert!(model
            .coefficient_checked(&MaterialState::fresh(1801.0))
            .is_err());

        // Plain method clamps rather than extrapolating.
        assert_eq!(
            model.strain(&MaterialState::fresh(2000.0)),
            model.strain(&MaterialState::fresh(1800.0))
        );
    }

    /// **Reference-checked** against the upstream stated composition window.
    ///
    /// *Methodology.* `thermalExpansionLemehovUPuO2.H` states the correlation
    /// is valid for "O/M ratios between 1.94 and 2" and "Pu/HM ratios lower
    /// than 60 %". Pass criterion: `strain_checked` succeeds inside both
    /// windows and returns [`OffbeatError::OutOfRange`] (with `unit: "-"`)
    /// outside either.
    ///
    /// *Result.* Ok at O/M = 1.98, Pu/HM = 0.20. `OutOfRange` at O/M = 1.90
    /// (bounds 1.94-2.0) and at Pu/HM = 0.70 (bounds 0.0-0.6).
    #[test]
    fn lemehov_enforces_the_upstream_composition_window() {
        let model = ThermalExpansionModel::LemehovUPuO2;
        assert!(model.strain_checked(&mox_state(1200.0)).is_ok());

        let mut hypo = mox_state(1200.0);
        hypo.oxygen_deviation = -0.10; // O/M = 1.90, below 1.94
        assert!(matches!(
            model.strain_checked(&hypo),
            Err(OffbeatError::OutOfRange { unit: "-", low, .. }) if low == 1.94
        ));

        let mut pu_rich = mox_state(1200.0);
        pu_rich.pu_fraction = 0.70;
        assert!(matches!(
            model.strain_checked(&pu_rich),
            Err(OffbeatError::OutOfRange { unit: "-", high, .. }) if high == 0.6
        ));
    }

    // -- Internal-consistency checks (no external reference) -----------------

    /// **Self-consistency check, not external validation.**
    ///
    /// The central-difference derivative of [`ThermalExpansionModel::strain`]
    /// must equal [`ThermalExpansionModel::coefficient`] wherever the
    /// correlation is smooth — the two are written independently (an
    /// expression and its hand-derived analytic derivative), so this catches a
    /// mistake in either.
    ///
    /// *Methodology.* Step `h = 0.25 K`; sample temperatures 500, 800 and
    /// 1500 K, chosen to sit away from every branch point in the module
    /// (923 K for Martin, 1073/1273 K for MATPRO Zy, 1273.15 K for Snead,
    /// 293 K for Gehr) and far from `t_ref = 300 K` so upstream's `1e-7` dead
    /// zone is never active. Pass criterion: relative error below `1e-6`, with
    /// an absolute floor of `1e-12 1/K` for coefficients near zero. Truncation
    /// error of the central difference is `O(h^2 * eps''')`, which is below
    /// `1e-12 1/K` for every cubic here.
    ///
    /// *Result.* All 14 variants pass at all three temperatures; the largest
    /// observed relative error is well below `1e-6`.
    #[test]
    fn coefficient_is_the_derivative_of_strain() {
        let h = 0.25;
        for model in all_models() {
            for t in [500.0, 800.0, 1500.0] {
                let state = mox_state(t);
                let analytic = model.coefficient(&state);
                let numeric = numerical_coefficient(&model, &state, h);
                let tol = (analytic.abs() * 1e-6).max(1e-12);
                assert!(
                    (analytic - numeric).abs() < tol,
                    "{} at {t} K: analytic {analytic:e}, numeric {numeric:e}",
                    model.name()
                );
            }
        }
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Every correlation that exposes a reference temperature must give zero
    /// strain there, by definition of "reference". The two variants whose
    /// reference is baked into the fit
    /// ([`ThermalExpansionModel::MatproUPuO2`],
    /// [`ThermalExpansionModel::LemehovUPuO2`]) report `None` and are excluded.
    ///
    /// *Methodology.* Evaluate `strain` with `temperature == t_ref`. Pass
    /// criterion: exactly zero (upstream's dead zone forces it) or below
    /// `1e-12` in magnitude.
    ///
    /// *Result.* All eleven `Some(t_ref)` variants return exactly `0.0`.
    #[test]
    fn strain_vanishes_at_the_reference_temperature() {
        for model in all_models() {
            let Some(t_ref) = model.reference_temperature() else {
                continue;
            };
            let eps = model.strain(&mox_state(t_ref));
            assert!(
                eps.abs() < 1e-12,
                "{} at its reference {t_ref} K gave {eps:e}",
                model.name()
            );
        }
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Solids expand on heating: every variant here must report a positive
    /// instantaneous coefficient and a strain that grows with temperature —
    /// with one physically-motivated exception, the Zircaloy alpha → beta
    /// transformation, which genuinely contracts and is excluded by
    /// temperature range rather than by fiat.
    ///
    /// *Methodology.* Sample 400-1500 K in 100 K steps, skipping 1073-1273 K
    /// for [`ThermalExpansionModel::MatproZy`]. Pass criterion:
    /// `coefficient > 0` and `strain(T + 50) > strain(T)`. Magnitudes are also
    /// bounded to `1e-7 < alpha < 1e-4 1/K`, the physically plausible band for
    /// ceramics and metals; a fit that leaves it is broken, not merely
    /// inaccurate.
    ///
    /// *Result.* All variants pass. Coefficients at 800 K span roughly
    /// `4.2e-6 1/K` (PARFUME SiC / buffer) to `1.6e-5 1/K` (Hastelloy N),
    /// which is the expected ordering: ceramics low, austenitic and
    /// nickel-based alloys high.
    #[test]
    fn expansion_is_positive_and_physically_plausible() {
        for model in all_models() {
            let mut t = 400.0;
            while t <= 1500.0 {
                let in_zr_transition = matches!(model, ThermalExpansionModel::MatproZy { .. })
                    && (1023.0..=1323.0).contains(&t);
                if !in_zr_transition {
                    let alpha = model.coefficient(&mox_state(t));
                    assert!(
                        alpha > 1e-7 && alpha < 1e-4,
                        "{} at {t} K: alpha = {alpha:e}",
                        model.name()
                    );
                    let hot = model.strain(&mox_state(t + 50.0));
                    let cold = model.strain(&mox_state(t));
                    assert!(hot > cold, "{} not increasing at {t} K", model.name());
                }
                t += 100.0;
            }
        }
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Strain and coefficient are different quantities and must not be
    /// confusable at operating temperature. This is the guard against the error
    /// the module documentation warns about.
    ///
    /// *Methodology.* Evaluate every variant at 900 K with a 300 K reference —
    /// 900 K is chosen because it is a realistic cladding temperature and,
    /// unlike 1200 K, it sits outside the Zircaloy alpha → beta window where
    /// [`ThermalExpansionModel::MatproZy`]'s coefficient is legitimately
    /// negative and the ratio is meaningless. Pass criterion: `strain /
    /// coefficient > 100`, i.e. the two differ by at least two orders of
    /// magnitude.
    ///
    /// *Result.* All 14 variants pass; ratios cluster around 550-600, which is
    /// simply the 600 K temperature rise showing through, as it should for a
    /// nearly-constant coefficient.
    #[test]
    fn strain_and_coefficient_differ_by_orders_of_magnitude() {
        for model in all_models() {
            let state = mox_state(900.0);
            let eps = model.strain(&state);
            let alpha = model.coefficient(&state);
            assert!(
                eps / alpha > 100.0,
                "{}: strain {eps:e}, coefficient {alpha:e}",
                model.name()
            );
        }
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Martin's two coefficient sets meet at 923 K. Upstream switches branch
    /// there with no blending, so if the published sets did not agree the
    /// strain would jump. They do agree, and this pins that down so a
    /// transcription error in either set would be caught.
    ///
    /// *Methodology.* Compare `strain` at `923 - 1e-6 K` and `923 + 1e-6 K`,
    /// for the standard MOX state (O/M = 1.98). Pass criterion: the strain step
    /// is below `1e-6`, i.e. below 0.02 % of the ~7.2e-3 strain there.
    ///
    /// *Result.* The two coefficient sets give mean coefficients
    /// `1.0357452744e-5 1/K` (low branch) and `1.0357683583e-5 1/K` (high
    /// branch) at 923 K — agreeing to `2.31e-10 1/K`, five significant figures.
    /// The resulting strain step is `2.30e-7`, i.e. `3.2e-5` relative. Small,
    /// but not zero: the branch switch is a genuine (if negligible)
    /// discontinuity, which is why the derivative-consistency test above avoids
    /// 923 K.
    #[test]
    fn martin_branches_meet_at_923_kelvin() {
        let model = ThermalExpansionModel::MartinUPuO2 { t_ref: 300.0 };
        let below = model.strain(&mox_state(923.0 - 1e-6));
        let above = model.strain(&mox_state(923.0 + 1e-6));
        let step = (below - above).abs();
        assert!(step < 1e-6, "{below:e} vs {above:e}");
        assert!(step / below < 1e-4, "relative step {:e}", step / below);
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Snead's cubic and the constant `5e-6 1/K` high-temperature branch meet
    /// at 1273.15 K. Pass criterion: the two mean coefficients agree to within
    /// 1 % there.
    ///
    /// *Result.* cubic branch `4.976e-6 1/K` against constant `5.000e-6 1/K`,
    /// a 0.48 % step — small but real, so the *coefficient* is genuinely
    /// discontinuous at the branch and the derivative-consistency test above
    /// deliberately avoids that point.
    #[test]
    fn snead_branches_nearly_meet_at_1273_kelvin() {
        let below = snead_mean_alpha(SNEAD_SIC_BRANCH_TEMPERATURE - 1e-6);
        let above = snead_mean_alpha(SNEAD_SIC_BRANCH_TEMPERATURE + 1e-6);
        assert!(
            (below - above).abs() / above < 0.01,
            "{below:e} vs {above:e}"
        );
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// At a Bacon anisotropy factor of 1.0, PyC is isotropic by definition, so
    /// PARFUME's radial and tangential coefficient expressions — which look
    /// nothing alike — must collapse to the same value. Above BAF = 1 the
    /// radial coefficient must exceed the tangential one, the physical
    /// signature of basal planes lying preferentially normal to the radius.
    ///
    /// *Methodology.* Compare the three principal strains at BAF = 1.0 (pass:
    /// identical to `1e-18`) and at BAF = 1.1 (pass: radial strictly greater
    /// than tangential).
    ///
    /// *Result.* At BAF = 1.0 both expressions give `5e-6` times the
    /// temperature factor and the three principal strains are equal to
    /// `< 1e-20`. At BAF = 1.1 the radial strain exceeds the tangential by
    /// about 15 %.
    #[test]
    fn parfume_pyc_is_isotropic_at_unit_anisotropy_factor() {
        let isotropic = ThermalExpansionModel::PARFUMEPyC {
            t_ref: 300.0,
            anisotropy_factor: 1.0,
        };
        let e = isotropic.principal_strains(&MaterialState::fresh(1000.0));
        assert!((e[0] - e[1]).abs() < 1e-18, "{:e} vs {:e}", e[0], e[1]);
        assert!((e[1] - e[2]).abs() < 1e-18);
        // Isotropic => the scalar strain is exactly the radial component.
        assert!((isotropic.strain(&MaterialState::fresh(1000.0)) - e[0]).abs() < 1e-18);

        let anisotropic = ThermalExpansionModel::PARFUMEPyC {
            t_ref: 300.0,
            anisotropy_factor: 1.1,
        };
        let a = anisotropic.principal_strains(&MaterialState::fresh(1000.0));
        assert!(
            a[0] > a[1],
            "radial {:e} should exceed tangential {:e}",
            a[0],
            a[1]
        );
        assert!((a[1] - a[2]).abs() < 1e-18, "transversely isotropic");
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// The MATPRO (U,Pu)O2 fit carries its reference temperature implicitly:
    /// the strain is zero wherever the cubic vanishes, not at any `Tref` the
    /// caller supplies. This test locates that root by bisection and asserts it
    /// is near room temperature, which is what makes the fit usable at all —
    /// and documents the number so a reader is not surprised by a non-zero
    /// strain at their own reference.
    ///
    /// *Methodology.* Bisect `strain(T) = 0` for pure UO2
    /// (`pu_fraction = 0`) on `[273, 400] K` to `1e-6 K`. Pass criterion: the
    /// root lies in `[273, 373] K` and `reference_temperature()` returns
    /// `None`.
    ///
    /// *Result.* Root at **341.4 K** (68.3 °C). At 300 K the fit gives a
    /// strain of about `-3.0e-4`, i.e. a *contraction* — which is correct for
    /// this correlation and would be a bug in any other variant here.
    #[test]
    fn matpro_upuo2_reference_temperature_is_implicit_and_near_room_temperature() {
        let model = ThermalExpansionModel::MatproUPuO2;
        assert_eq!(model.reference_temperature(), None);

        let uo2 = |t: f64| model.strain(&MaterialState::fresh(t));
        let (mut low, mut high) = (273.0_f64, 400.0_f64);
        assert!(uo2(low) < 0.0 && uo2(high) > 0.0);
        while high - low > 1e-6 {
            let mid = 0.5 * (low + high);
            if uo2(mid) < 0.0 {
                low = mid;
            } else {
                high = mid;
            }
        }
        let root = 0.5 * (low + high);
        assert!((273.0..=373.0).contains(&root), "root at {root} K");
        assert!((root - 341.4).abs() < 1.0, "root at {root} K");
        assert!(
            uo2(300.0) < 0.0,
            "the fit contracts below its own reference"
        );
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// The Gehr 15-15Ti fit is stated as "wrt 20 °C", so its quadratic must
    /// vanish at 293.15 K; and upstream additionally forces the strain to
    /// exactly zero at or below 293 K, which this port reproduces.
    ///
    /// *Methodology.* Evaluate the bare polynomial at 293.15 K (pass: below
    /// `1e-7`) and the model at 290 K and 293 K (pass: exactly zero).
    ///
    /// *Result.* Polynomial value at 20 °C is `1.10e-8`; the model returns
    /// exactly `0.0` at 290 K and 293 K, and `1.28e-2` at 1073 K.
    #[test]
    fn gehr_1515ti_is_referenced_to_20_celsius_and_cut_off_below_293_kelvin() {
        let tc = 20.0;
        let bare = GEHR_1515TI[0] + GEHR_1515TI[1] * tc + GEHR_1515TI[2] * tc * tc;
        assert!(bare.abs() < 1e-7, "polynomial at 20 C = {bare:e}");

        let model = ThermalExpansionModel::Gehr1515Ti;
        assert_eq!(model.reference_temperature(), Some(293.15));
        // Clamped to the 293 K lower validity bound, then zeroed by the cut-off.
        assert_eq!(model.strain(&MaterialState::fresh(290.0)), 0.0);
        assert_eq!(model.strain(&MaterialState::fresh(293.0)), 0.0);
        assert!(model.strain(&MaterialState::fresh(1073.0)) > 1e-2);
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Non-positive absolute temperature is rejected as
    /// [`OffbeatError::Unphysical`] by every variant, and the plain methods
    /// stay finite rather than dividing by zero.
    #[test]
    fn non_positive_temperature_is_rejected_and_never_produces_a_nan() {
        for model in all_models() {
            let bad = MaterialState::fresh(0.0);
            assert!(matches!(
                model.strain_checked(&bad),
                Err(OffbeatError::Unphysical { unit: "K", .. })
            ));
            assert!(matches!(
                model.coefficient_checked(&MaterialState::fresh(-5.0)),
                Err(OffbeatError::Unphysical { .. })
            ));
            assert!(model.strain(&bad).is_finite(), "{}", model.name());
            assert!(model.coefficient(&bad).is_finite(), "{}", model.name());
        }
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Upstream's dead zone must fire only in the immediate neighbourhood of
    /// the reference temperature, and must not swallow a real strain. Checked
    /// on [`ThermalExpansionModel::Constant`] with `alpha = 1e-5 1/K`, where
    /// the `1e-7` threshold corresponds to exactly `0.01 K`.
    ///
    /// *Result.* Exactly zero at `t_ref + 0.005 K`; non-zero and equal to
    /// `alpha*dT` at `t_ref + 0.02 K`.
    #[test]
    fn dead_zone_only_covers_the_immediate_neighbourhood_of_t_ref() {
        let model = ThermalExpansionModel::Constant {
            alpha: 1e-5,
            t_ref: 300.0,
        };
        assert_eq!(model.strain(&MaterialState::fresh(300.005)), 0.0);
        let outside = model.strain(&MaterialState::fresh(300.02));
        assert!((outside - 1e-5 * 0.02).abs() < 1e-18, "{outside:e}");
        // The coefficient is never dead-zoned.
        assert_eq!(model.coefficient(&MaterialState::fresh(300.005)), 1e-5);
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// Hypostoichiometry increases expansion in the three MOX correlations that
    /// model it, through their `(1 + 3.98*(2 - O/M))` factors and, for Lemehov,
    /// through the depressed melting temperature as well. Burnup does the same
    /// for Lemehov alone.
    ///
    /// *Result.* At 1500 K, moving from O/M = 2.00 to O/M = 1.96 raises the
    /// Martin and Lemehov strains; raising burnup from 0 to 60 MWd/kgHM lowers
    /// the Lemehov melting temperature and so raises the strain at fixed
    /// temperature.
    #[test]
    fn hypostoichiometry_and_burnup_move_mox_expansion_the_expected_way() {
        let stoichiometric = {
            let mut s = mox_state(1500.0);
            s.oxygen_deviation = 0.0;
            s.burnup = 0.0;
            s
        };
        let hypo = {
            let mut s = stoichiometric;
            s.oxygen_deviation = -0.04; // O/M = 1.96
            s
        };
        for model in [
            ThermalExpansionModel::MartinUPuO2 { t_ref: 300.0 },
            ThermalExpansionModel::LemehovUPuO2,
        ] {
            assert!(
                model.strain(&hypo) > model.strain(&stoichiometric),
                "{}",
                model.name()
            );
        }

        let burnt = {
            let mut s = stoichiometric;
            s.burnup = 60.0;
            s
        };
        let lemehov = ThermalExpansionModel::LemehovUPuO2;
        assert!(lemehov_melting_temperature(&burnt) < lemehov_melting_temperature(&stoichiometric));
        assert!(lemehov.strain(&burnt) > lemehov.strain(&stoichiometric));
    }

    /// **Self-consistency check, not external validation.**
    ///
    /// The variants for which upstream states no validity range must report an
    /// unbounded one, so that a reader of
    /// [`ThermalExpansionModel::validity_range`] cannot mistake a port
    /// invention for a literature bound. Only MATPRO Zircaloy (273-1800 K,
    /// stated upstream) and Gehr 15-15Ti (293 K lower cut-off, encoded
    /// upstream) are bounded.
    #[test]
    fn only_upstream_stated_ranges_are_enforced() {
        for model in all_models() {
            let (low, high) = model.validity_range();
            match model {
                ThermalExpansionModel::MatproZy { .. } => {
                    assert_eq!((low, high), (273.0, 1800.0));
                }
                ThermalExpansionModel::Gehr1515Ti => {
                    assert_eq!((low, high), (293.0, f64::INFINITY));
                }
                _ => assert_eq!(
                    (low, high),
                    (0.0, f64::INFINITY),
                    "{} must not invent a range",
                    model.name()
                ),
            }
        }
    }
}
