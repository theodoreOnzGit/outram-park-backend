// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
// `offbeatLib/materials/materialModel/behavioralModels/swelling/`:
//   swellingModel.{C,H}                    -> SwellingModel::Zero
//   constantSwelling.{C,H}                 -> SwellingModel::Constant
//   swellingFRAPCON.{C,H}                  -> SwellingModel::Uo2Frapcon
//   swellingMATPRO.{C,H}                   -> SwellingModel::Uo2Matpro
//   swellingFBRMOX.{C,H}                   -> SwellingModel::FbrMox
//   swellingFrCrAl.{C,H}                   -> SwellingModel::FeCrAl
//   swellingGrowthBISONZy.{C,H}            -> SwellingModel::GrowthBisonZircaloy
//   swellingGrowthAIM11515Ti.{C,H}         -> SwellingModel::GrowthAim11515Ti
//   swellingGrowthGeneralized1515Ti.{C,H}  -> SwellingModel::GrowthGeneralized1515Ti
//   swellingWrightShamHastelloyN.{C,H}     -> SwellingModel::WrightShamHastelloyN
//   swellingCorrelationPyC.{C,H}           -> SwellingModel::PyroCarbonCorrelation
// Not ported (see the module documentation for the reasons):
//   swellingPARFUMEBuffer.{C,H}, swellingPARFUMEPyC.{C,H}, swellingPARFUMEPyCdata.H
//   swellingGrowthMatproZy.{C,H}
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Swelling models — irradiation-driven volume **growth**, volumetric strain \[-\].
//!
//! # What swelling is
//!
//! Fission destroys one heavy-metal atom and creates two fission-product atoms
//! that, together, occupy more space than the atom they replaced. The solid
//! fission products dissolve in the fuel lattice and make it grow roughly
//! **linearly with burnup**; the gaseous ones (xenon, krypton) precipitate into
//! bubbles whose growth is **strongly temperature-dependent** and saturates.
//! In metals — cladding and structural steels — the same word covers two
//! different phenomena: **void swelling**, the growth of vacancy voids under
//! fast-neutron damage (isotropic, threshold-like in dose), and **irradiation
//! growth**, a volume-*conserving* change of shape in anisotropic hexagonal
//! metals such as Zircaloy (elongation along the rod axis, contraction across
//! it).
//!
//! # SIGN CONVENTION — swelling is POSITIVE
//!
//! **Every model in this module returns a positive number for material that is
//! growing.** [`DensificationModel`] returns a **negative** number for the same
//! material shrinking. The two are summed by the caller, so a sign error here
//! does not blow up — it silently cancels part of the densification and
//! produces a fuel/cladding gap that closes at the wrong time. Be certain which
//! one you are holding.
//!
//! The one documented exception is
//! [`WrightShamHastelloyN`](SwellingModel::WrightShamHastelloyN), whose
//! upstream correlation is *negative below its incubation dose* (about 0.99
//! dpa). That is upstream's behaviour, reproduced faithfully; it is a defect of
//! the fit, not a sign-convention change. See that variant's documentation.
//!
//! # Units, and the volumetric/linear factor of three
//!
//! - [`SwellingModel::value`] returns the **volumetric** strain `ΔV/V` \[-\],
//!   matching [`MaterialState::swelling`].
//! - [`SwellingModel::strain`] returns the three **linear** components \[-\]
//!   separately, matching upstream's `epsilonSwelling` symmetric-tensor
//!   diagonal. For an isotropic model each component is `value() / 3`.
//!
//! **Upstream stores the linear components, not the volumetric strain.** Every
//! upstream `correct()` in this directory ends with `swellingI[cellI] =
//! nominalValue * I`, i.e. it writes one third of the volume change into each
//! diagonal component. If you are comparing this port against an OFFBEAT run,
//! compare [`strain`](SwellingModel::strain), not [`value`](SwellingModel::value).
//!
//! Burnup arrives as **MWd/kgHM** ([`MaterialState::burnup`]); fast fluence as
//! **n/m²** ([`MaterialState::fast_fluence`]). Several upstream correlations
//! are written against fluence in **n/cm²** and burnup in MWd/tU or in %FIMA;
//! every conversion is done once, here, and is stated in the variant's
//! documentation.
//!
//! # Validity ranges: `value` clamps, `value_checked` refuses
//!
//! [`value`](SwellingModel::value) and [`strain`](SwellingModel::strain)
//! **clamp** burnup, fluence and temperature to the endpoints of the variant's
//! stated validity range before evaluating, so they always return a finite,
//! bounded number. [`value_checked`](SwellingModel::value_checked) and
//! [`strain_checked`](SwellingModel::strain_checked) instead return
//! [`OffbeatError::OutOfRange`]. Upstream OFFBEAT clamps *nothing* in this
//! directory — it extrapolates freely — so outside the stated range this port
//! and upstream deliberately disagree, and the clamped answer is the more
//! defensible of the two.
//!
//! The ranges themselves are **this port's stated applicability**, not upstream
//! constants: upstream declares no ranges at all. They are set to the operating
//! window the correlation's material actually sees, wide enough that a normal
//! case never touches an endpoint. Each variant says what its range is.
//!
//! # What is deliberately not ported
//!
//! - **`swellingPARFUMEBuffer` / `swellingPARFUMEPyC`** (TRISO buffer and
//!   pyrolytic-carbon layers, PARFUME correlations). These are not pure
//!   functions of [`MaterialState`]: they need the fast **flux** and the
//!   **timestep** (they integrate a strain *rate* explicitly, `ε += ε̇ φ̇ Δt`),
//!   plus a two-dimensional interpolation table in temperature and
//!   Bacon-Anisotropy-Factor and a second table in coating density. Porting
//!   them needs a state object this crate does not yet have and a table
//!   interpolator this crate does not yet have. Use
//!   [`PyroCarbonCorrelation`](SwellingModel::PyroCarbonCorrelation) — a
//!   closed-form polynomial in fluence — for pyrolytic carbon in the meantime.
//! - **`swellingGrowthMatproZy`** (MATPRO Zircaloy irradiation growth). It is
//!   **absent from upstream's `Make/files`** and, as written, does not compile:
//!   it assigns to a `const scalar`, and binds a `symmTensorField` to a
//!   `const scalarField&`. It is dead code upstream, so there is no compiled
//!   behaviour to port and nothing to verify a port against.
//!
//! # Status
//!
//! AI-assisted translation, reviewed by no human yet. Per `RESPONSIBLE_USE.md`
//! this is untrusted draft material: the unit tests below establish internal
//! consistency and agreement with upstream's own algorithms, **not** validation
//! against measured swelling data.
//!
//! [`DensificationModel`]: super::densification::DensificationModel
//! [`MaterialState`]: crate::materials::MaterialState
//! [`MaterialState::burnup`]: crate::materials::MaterialState::burnup
//! [`MaterialState::fast_fluence`]: crate::materials::MaterialState::fast_fluence
//! [`MaterialState::swelling`]: crate::materials::MaterialState::swelling
//! [`OffbeatError::OutOfRange`]: crate::error::OffbeatError::OutOfRange

use crate::error::{OffbeatError, Result};
use crate::materials::MaterialState;

/// The three linear swelling strain components \[-\] in the material's local
/// radial / hoop / axial frame.
///
/// # Why three numbers and not one
///
/// Isotropic swelling (fission-product swelling in an oxide pellet, void
/// swelling in a steel) puts one third of the volume change into each
/// direction, and a single scalar would do. **Irradiation growth in Zircaloy
/// does not**: it elongates the cladding along the rod axis and contracts it
/// across, at essentially constant volume, so its volumetric strain is nearly
/// zero while its axial strain is the whole engineering point. A scalar-only
/// interface would report "no swelling" for the model whose entire purpose is
/// axial elongation. Upstream stores a symmetric tensor for exactly this
/// reason; this struct is that tensor's diagonal.
///
/// # Frame
///
/// - `radial` — upstream's `xx`. For a spherical TRISO coating layer this is
///   the through-thickness direction.
/// - `hoop` — upstream's `yy`. For a TRISO layer, one of the two tangential
///   directions.
/// - `axial` — upstream's `zz`. Along the fuel rod. For a TRISO layer, the
///   second tangential direction (equal to `hoop` by symmetry).
///
/// # Sign and units
///
/// Dimensionless engineering strain, **positive for growth** in that
/// direction. Zircaloy growth legitimately gives a negative `radial`/`hoop`
/// alongside a positive `axial`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwellingStrain {
    /// Linear strain \[-\] along the local radial direction (upstream `xx`).
    pub radial: f64,
    /// Linear strain \[-\] along the local hoop direction (upstream `yy`).
    pub hoop: f64,
    /// Linear strain \[-\] along the axial direction (upstream `zz`).
    pub axial: f64,
}

impl SwellingStrain {
    /// No swelling in any direction — the unirradiated reference state.
    pub const ZERO: Self = Self {
        radial: 0.0,
        hoop: 0.0,
        axial: 0.0,
    };

    /// Construct from the three linear components \[-\], each positive for
    /// growth.
    #[must_use]
    pub const fn new(radial: f64, hoop: f64, axial: f64) -> Self {
        Self {
            radial,
            hoop,
            axial,
        }
    }

    /// Construct an isotropic strain from a **volumetric** strain \[-\], i.e.
    /// put `volumetric / 3` into each direction.
    ///
    /// This is the constructor to use when a correlation is quoted as `ΔV/V`,
    /// which most fission-product swelling correlations are.
    #[must_use]
    pub fn isotropic(volumetric: f64) -> Self {
        let component = volumetric / 3.0;
        Self::new(component, component, component)
    }

    /// Volumetric swelling strain `ΔV/V` \[-\], **positive for growth**.
    ///
    /// Computed as the trace `radial + hoop + axial`, the small-strain
    /// approximation to `(1+ε_r)(1+ε_h)(1+ε_a) - 1`. The two differ by
    /// second-order terms: at 5% in each direction the trace reads 0.150 000
    /// against the exact 0.157 625, a 4.8% relative difference. High-burnup
    /// fuel swelling does reach that magnitude, so treat this as an engineering
    /// volumetric strain, not an exact volume ratio.
    #[must_use]
    pub fn volumetric(&self) -> f64 {
        self.radial + self.hoop + self.axial
    }
}

/// Zircaloy alloy and metallurgical condition selecting the BISON irradiation
/// growth coefficients.
///
/// Upstream's `swellingGrowthBISONZy` reads this as the `cladType` keyword and
/// overwrites its `A` and `n` coefficients accordingly. Growth is
/// `ε_axial = A φ^n` with `φ` the fast fluence in **n/cm²** (E > 1 MeV); the
/// coefficient pairs below are the ones hard-coded upstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BisonZircaloyCladType {
    /// Stress-relief annealed Zircaloy-2 or Zircaloy-4. `A = 2.18e-21`,
    /// `n = 0.845`.
    Sra,
    /// Recrystallisation annealed Zircaloy-2 or M5. `A = 1.09e-21`,
    /// `n = 0.845`.
    Rxa,
    /// Partially recrystallised Zircaloy-2. `A = 1.09e-21`, `n = 0.845` —
    /// upstream gives it the same coefficients as [`Rxa`](Self::Rxa).
    Pra,
    /// Stress-relief annealed ZIRLO. `A = 9.7893e-25`, `n = 0.98239`.
    Zirlo,
    /// The ESCORE growth model (Rashid). `A = 3.0e-20`, `n = 0.794`. This is
    /// upstream's default when no `cladType` is given.
    #[default]
    Escore,
    /// M5 (Gilbon). `A = 7.013e-21`, `n = 0.81787`.
    M5,
}

impl BisonZircaloyCladType {
    /// The `(A, n)` pair of the growth law `ε_axial = A φ^n`, with `φ` the fast
    /// fluence in **n/cm²**.
    ///
    /// `A` therefore carries units of `(n/cm²)^-n`; it is meaningless applied
    /// to a fluence in n/m², which is why
    /// [`GrowthBisonZircaloy`](SwellingModel::GrowthBisonZircaloy) divides the
    /// SI fluence by 1e4 before using it.
    #[must_use]
    pub fn coefficients(&self) -> (f64, f64) {
        match self {
            Self::Sra => (2.18e-21, 0.845),
            Self::Rxa | Self::Pra => (1.09e-21, 0.845),
            Self::Zirlo => (9.7893e-25, 0.98239),
            Self::Escore => (3.0e-20, 0.794),
            Self::M5 => (7.013e-21, 0.81787),
        }
    }
}

/// Irradiation swelling and growth correlations — **positive** volumetric
/// strain \[-\] for growing material.
///
/// One variant per model compiled by upstream OFFBEAT's
/// `behavioralModels/swelling/`; each variant's documentation names the
/// upstream class and its `TypeName` (the string a user writes in
/// `solverDict`), so a case file can be translated variant by variant. Two
/// upstream models are deliberately absent — see the [module
/// documentation](self).
///
/// Dispatch is by `match` on the enum, never by a trait object, per the
/// workspace `CLAUDE.md` "No trait objects" rule: the set of published
/// correlations is closed and known at compile time, so adding one must be a
/// compile error at every call site rather than a runtime surprise.
///
/// # Sign convention
///
/// **Positive is growth.** See the [module documentation](self) — this matters
/// because densification returns a negative number for the same fuel and the
/// two are summed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SwellingModel {
    /// No swelling at all — upstream `swellingModel`, `TypeName("none")`.
    ///
    /// Selecting this upstream still creates the `epsilonSwelling` field and
    /// leaves it at zero, so the mechanics solve runs with a swelling term that
    /// is identically zero. Use it to isolate other effects, never as a
    /// physical model of irradiated fuel.
    ///
    /// Returns exactly `0.0` at every state; no validity range.
    Zero,

    /// Swelling proportional to fast fluence with a user-supplied rate —
    /// upstream `constantSwelling`, `TypeName("constant")`.
    ///
    /// `ε_linear = swelling_rate · φ / 1e25`, with `φ` the fast fluence in
    /// n/m². Isotropic, so the volumetric strain is three times that.
    ///
    /// **Not a correlation** — a placeholder for a material whose swelling has
    /// been measured but not fitted, and a convenient way to impose a known
    /// swelling in a verification case. Its "validity range" below is a sanity
    /// guard only.
    ///
    /// Valid range: fast fluence `0` to `1e27` n/m².
    Constant {
        /// Upstream's `swellingRate` \[-\]: the **linear** strain in each
        /// direction per `1e25` n/m² of fast fluence. The volumetric strain per
        /// `1e25` n/m² is three times this.
        swelling_rate: f64,
    },

    /// UO2 fission-product swelling, FRAPCON form — upstream `swellingFRAPCON`,
    /// `TypeName("UO2FRAPCON")`.
    ///
    /// Piecewise-linear in burnup, with the swelling rate stepping up at high
    /// burnup as fission gas starts to contribute:
    ///
    /// - `Bu < 6` MWd/kgHM: zero. The as-fabricated porosity accommodates the
    ///   early fission products, so no net growth is seen.
    /// - `6 ≤ Bu < 80` MWd/kgHM: `ΔV/V = (Bu − 6)·1000 · b1 · ρ`
    /// - `Bu ≥ 80` MWd/kgHM:
    ///   `ΔV/V = (80 − 6)·1000·b1·ρ + (Bu − 80)·1000·b2·ρ`
    ///
    /// with `b1 = 2.974e10 · 2.315e-23 · 86.4` and
    /// `b2 = 2.974e10 · 3.211e-23 · 86.4` (upstream's `par3·par4·par5` and
    /// `par3·par6·par5`, fixed here at their upstream defaults), and `ρ` the
    /// porous fuel density in kg/m³, taken as
    /// `theoretical_density · state.density_fraction()`. `2.315e-23` and
    /// `3.211e-23` are FRAPCON's `ΔV/V` per fission per m³ below and above
    /// 80 MWd/kgHM.
    ///
    /// At `ρ = 10 400` kg/m³ this is **6.19e-4 volumetric strain per
    /// MWd/kgHM** in the first regime, i.e. 0.62% per 10 MWd/kgHM.
    ///
    /// **Gaseous swelling is not included.** Upstream adds the
    /// `intragranularGasSwelling` and `intergranularGasSwelling` fields on top
    /// of this when the fission-gas model has created them. Those come from
    /// [`crate::fgr`], not from here; the caller adds them.
    ///
    /// Valid range: burnup `0` to `120` MWd/kgHM.
    Uo2Frapcon {
        /// Theoretical (pore-free) density of the fuel \[kg/m³\]; 10 960 for
        /// UO2. The porous density used by the correlation is this times
        /// `MaterialState::density_fraction`.
        theoretical_density: f64,
    },

    /// UO2 fission-product swelling, MATPRO form — upstream `swellingMATPRO`,
    /// `TypeName("UO2MATPRO")`.
    ///
    /// Solid plus gaseous fission-product swelling, both quoted per unit
    /// **FIMA** (fissions per initial metal atom). Burnup is converted with
    /// `FIMA = Bu / 937.06` for `Bu` in MWd/kgHM.
    ///
    /// - solid: `ΔV/V = 5.577e-5 · ρ · FIMA`
    /// - gaseous rate: `d(ΔV/V)/dFIMA = 1.96e-31 · ρ · (2800 − T)^11.73 ·
    ///   exp(−0.0162 (2800 − T)) · exp(−0.0178 ρ · FIMA)`
    ///
    /// with `ρ` in kg/m³ and `T` in K. The gaseous term peaks in the
    /// intermediate-temperature bubble-growth window and dies away at high
    /// burnup, where the `exp(−0.0178 ρ FIMA)` factor saturates it.
    ///
    /// # This port integrates; upstream accumulates
    ///
    /// Upstream evaluates the *rate* and adds `rate · ΔBu` each timestep — a
    /// forward-Euler accumulation whose answer depends on the timestep. A pure
    /// function of `MaterialState` cannot do that, so this port integrates the
    /// rate analytically from zero burnup:
    ///
    /// `ΔV/V_gas = C · (exp(k·FIMA) − 1) / k`, with
    /// `C = 1.96e-31 ρ (2800−T)^11.73 exp(−0.0162(2800−T))` and
    /// `k = −0.0178 ρ`, holding `T` at its current value over the whole
    /// history. The two agree in the limit of small burnup steps at constant
    /// temperature — that convergence is a unit test in this module. They
    /// differ, legitimately, on a history where the temperature changed.
    ///
    /// The solid term is exactly linear, so it is unaffected.
    ///
    /// Valid range: burnup `0` to `120` MWd/kgHM; temperature `300` to
    /// `2800` K. The upper temperature bound is not decorative:
    /// `(2800 − T)^11.73` is `NaN` for `T > 2800` K, and upstream would
    /// propagate that `NaN` into the mechanics solve.
    Uo2Matpro {
        /// Theoretical (pore-free) density of the fuel \[kg/m³\]; 10 960 for
        /// UO2, about 11 000 for LWR MOX. Multiplied by
        /// `MaterialState::density_fraction` to get the porous density the
        /// correlation wants.
        theoretical_density: f64,
    },

    /// Fast-reactor MOX swelling — upstream `swellingFBRMOX`,
    /// `TypeName("FBRMOX")`.
    ///
    /// A three-rate empirical law in burnup expressed as **%FIMA**, converted
    /// here as `%FIMA = Bu / 9.5` for `Bu` in MWd/kgHM. Which rate applies
    /// depends on whether the fuel/cladding gap is still open:
    ///
    /// - gap open, `%FIMA ≤ 1`: `d(ΔV/V)/d(%FIMA) = 0.020` — free swelling,
    ///   fission gas retained in bubbles.
    /// - gap open, `%FIMA > 1`: `0.012`.
    /// - gap closed: `0.0065` — only the solid fission products contribute;
    ///   with the pellet in contact with the cladding, gas bubbles are
    ///   suppressed and the gas is released instead.
    ///
    /// This port integrates those rates from zero burnup, so with the gap open
    /// throughout, `ΔV/V = 0.020·min(%FIMA, 1) + 0.012·max(%FIMA − 1, 0)`, and
    /// with the gap closed throughout, `ΔV/V = 0.0065·%FIMA`.
    ///
    /// # `gap_open` describes the whole history, not this instant
    ///
    /// Upstream re-reads the slice gap width every timestep and can switch
    /// rates mid-life; a pure function cannot. The value returned here is the
    /// swelling that would have accumulated had `gap_open` held for the entire
    /// irradiation. For a rod whose gap closes part-way through, that is an
    /// upper bound (`gap_open = true`) or a lower bound (`gap_open = false`),
    /// and a caller needing the mixed history must integrate the rates itself.
    ///
    /// Valid range: burnup `0` to `250` MWd/kgHM (about 26 %FIMA).
    FbrMox {
        /// Whether the fuel/cladding gap is open (`true`) for the whole
        /// irradiation history. See the variant note above — this is a history
        /// flag, not an instantaneous one.
        gap_open: bool,
    },

    /// FeCrAl cladding swelling, linear in fast fluence — upstream
    /// `swellingFrCrAl` (spelt "Fr" upstream), `TypeName("FrCrAl")`.
    ///
    /// `ε_linear = rate · φ`, isotropic, with `φ` the fast fluence in **n/m²**
    /// — this is one of the few upstream models that works in SI fluence
    /// directly. The upstream default `rate = 4.5e-29` per n/m² gives 0.45%
    /// linear (1.35% volumetric) at `1e26` n/m².
    ///
    /// FeCrAl is an accident-tolerant-fuel cladding candidate; the linear form
    /// is a first-order fit with no incubation dose and no temperature
    /// dependence, so it will over-predict at low dose.
    ///
    /// Valid range: fast fluence `0` to `2e26` n/m².
    FeCrAl {
        /// Upstream's `par1` \[1/(n/m²)\]: the **linear** strain per unit fast
        /// fluence in n/m². Upstream default `4.5e-29`.
        rate: f64,
    },

    /// Zircaloy irradiation **growth** (not swelling) — upstream
    /// `swellingGrowthBISONZy`, `TypeName("growthBISONZy")`.
    ///
    /// Anisotropic and volume-conserving. Zircaloy's hexagonal grains grow
    /// along the rod axis and contract across it under fast-neutron damage, at
    /// essentially constant volume:
    ///
    /// - `ε_axial = A φ^n`, with `φ` the fast fluence in **n/cm²** — the SI
    ///   fluence in `MaterialState` is divided by 1e4 here.
    /// - `ε_radial = ε_hoop = −(1 − (1 + ε_axial)^(−1/2))`, the transverse
    ///   contraction that keeps the volume fixed.
    ///
    /// `(A, n)` come from [`BisonZircaloyCladType`].
    ///
    /// **[`value`](Self::value) is near zero for this variant, by design.** Its
    /// volumetric strain is `O(ε_axial²)` — `+5.78e-5` against an axial strain
    /// of `+8.81e-3` at 1e26 n/m², i.e. 0.66% of it — because that is what
    /// volume conservation means. Use
    /// [`strain`](Self::strain) and read `axial`. A caller that only ever calls
    /// `value()` will conclude the cladding is not moving, and will be wrong.
    ///
    /// # Closed form versus upstream's accumulation
    ///
    /// Upstream applies the transverse mapping to each timestep's *increment*
    /// and sums; this port applies it once to the total. The mapping is
    /// nonlinear, so the two differ at **second order in the strain**: a unit
    /// test in this module measures `−4.377561e-3` (closed form) against
    /// `−4.406474e-3` (accumulated in 100 000 steps) at 1e26 n/m², a relative
    /// difference of 6.6e-3 and an absolute one of 2.9e-5 in strain. The
    /// accumulated form is the one that omits the `3ε²/8` term, so the closed
    /// form is the more nearly exact of the two; either way the gap is far
    /// below any measurement uncertainty on cladding growth.
    ///
    /// Valid range: fast fluence `0` to `1.5e26` n/m² (1.5e22 n/cm²), which
    /// covers LWR cladding to end of life.
    GrowthBisonZircaloy {
        /// Alloy and metallurgical condition, selecting `(A, n)`.
        clad_type: BisonZircaloyCladType,
    },

    /// AIM1 15-15Ti austenitic steel **void swelling** — upstream
    /// `swellingGrowthAIM11515Ti`, `TypeName("growthAIM11515Ti")`.
    ///
    /// Despite upstream's "growth" name this is isotropic void swelling, not
    /// anisotropic growth: upstream adds the same increment to `xx`, `yy` and
    /// `zz`.
    ///
    /// `ΔV/V [%] = 1.3e-5 · exp(−2.5·((T_C − 490)/100)²) · φ22^3.9`
    ///
    /// with `T_C` the temperature in **°C** and `φ22` the fast fluence in units
    /// of `1e22` n/cm² (the SI fluence divided by 1e26). The result is a
    /// percentage; this port divides by 100 to return a strain. The Gaussian in
    /// temperature peaks at 490 °C — the classic austenitic-steel void-swelling
    /// peak, where vacancy mobility and void stability overlap — and the
    /// `φ^3.9` dependence is the steep post-incubation regime.
    ///
    /// AIM1 (Austenitic Improved Material #1) is the titanium-stabilised
    /// 15Cr-15Ni cladding developed for sodium-cooled fast reactors.
    ///
    /// Valid range: fast fluence `0` to `3e27` n/m²; temperature `573.15` to
    /// `1023.15` K (300–750 °C).
    GrowthAim11515Ti,

    /// Generalised 15-15Ti austenitic steel **void swelling** — upstream
    /// `swellingGrowthGeneralized1515Ti`,
    /// `TypeName("growthGeneralized1515Ti")`.
    ///
    /// Same functional form as [`GrowthAim11515Ti`](Self::GrowthAim11515Ti),
    /// different fit — a generic 15-15Ti rather than the AIM1 heat:
    ///
    /// `ΔV/V [%] = 1.5e-3 · exp(−2.5·((T_C − 450)/100)²) · φ22^2.75`
    ///
    /// The swelling peak sits 40 °C lower and the dose exponent is milder
    /// (2.75 against 3.9), so this fit predicts more swelling at low dose and
    /// less at high dose than the AIM1 one. At `φ22 = 10` and the peak
    /// temperature it gives 0.84% against AIM1's 0.10%. They are genuinely
    /// different materials — do not treat the pair as an uncertainty band.
    ///
    /// Valid range: fast fluence `0` to `3e27` n/m²; temperature `573.15` to
    /// `1023.15` K (300–750 °C).
    GrowthGeneralized1515Ti,

    /// Hastelloy N **void swelling**, Wright-Sham correlation — upstream
    /// `swellingWrightShamHastelloyN`, `TypeName("WrightShamHastelloyN")`.
    ///
    /// Isotropic. Hastelloy N is the nickel-molybdenum alloy developed for
    /// molten-salt reactors, where the relevant damage measure is displacement
    /// damage rather than raw fluence:
    ///
    /// - `dpa = φ / 1e26 · 5` — upstream's conversion, 5 dpa per `1e22` n/cm².
    /// - `f(dpa) = 0.9845 · dpa^0.4385 − 0.981`
    /// - `g(T) = exp(−((T_C − 490)/100)²)`
    /// - `ΔV/V [%] = g(T) · f(dpa)`, divided by 100 here to give a strain.
    ///
    /// # This variant can return a NEGATIVE value — and that is upstream
    ///
    /// `f(dpa)` is negative below `dpa ≈ 0.992` (a fast fluence of about
    /// `1.98e25` n/m²), so the correlation reports
    /// *shrinkage* below its incubation dose. That is an artefact of fitting a
    /// power law with an offset to data that has an incubation period; it is
    /// not a sign-convention change and not densification. Upstream does not
    /// clamp it and neither does this port, because clamping would silently
    /// change the numbers an OFFBEAT comparison is judged against. **If you are
    /// summing this with a densification model, be aware you may be adding two
    /// negative numbers below 1 dpa.**
    ///
    /// Valid range: fast fluence `0` to `4e26` n/m² (about 20 dpa);
    /// temperature `573.15` to `1073.15` K (300–800 °C).
    WrightShamHastelloyN,

    /// Pyrolytic-carbon TRISO coating dimensional change, polynomial
    /// correlation — upstream `swellingCorrelationPyC`,
    /// `TypeName("PyCCorrelation")`.
    ///
    /// Anisotropic and user-parameterised. Pyrolytic carbon under fast-neutron
    /// damage first *densifies* (negative strain in both directions) and then
    /// turns around and swells, with the radial and tangential responses
    /// differing because the deposited layer is texturally anisotropic. The
    /// user supplies the coefficients of the strain **rate** with respect to
    /// fluence; this port integrates them, as upstream does:
    ///
    /// `ε_r(φ) = Σ_{i=0}^{5} A_r[i] · φ^(i+1) / (i+1)`, and likewise for `ε_t`
    /// with `A_t`,
    ///
    /// with `φ` the fast fluence in units of `1e25` n/m², scaled by
    /// `flux_conversion_factor`. The radial component goes to
    /// [`SwellingStrain::radial`]; the tangential component goes to **both**
    /// [`hoop`](SwellingStrain::hoop) and [`axial`](SwellingStrain::axial),
    /// matching upstream's `yy = zz = ε_t` for a spherical layer.
    ///
    /// **Not ported:** upstream can rotate this spherical-frame tensor into
    /// Cartesian mesh coordinates when `sphereCoordinate` is false. That is a
    /// mesh operation, not a material correlation, so it belongs to the caller;
    /// this variant always returns the spherical-frame components.
    ///
    /// Because the coefficients are user input, the sign of the *result* is
    /// whatever the supplied fit says — a negative value here means the coating
    /// is densifying, which for pyrolytic carbon is real physics and not an
    /// error.
    ///
    /// Valid range: fast fluence `0` to `4e25` n/m², the range over which the
    /// PARFUME-family pyrolytic-carbon fits these coefficients come from are
    /// defined (upstream's PARFUME models hard-bound it at `3.96e25`).
    PyroCarbonCorrelation {
        /// `A_r` — the six polynomial coefficients \[1/(1e25 n/m²)^(i+1)\] of
        /// the **radial** strain rate with respect to fluence, upstream's
        /// `radialCoefficients`.
        radial_coefficients: [f64; 6],
        /// `A_t` — the six polynomial coefficients of the **tangential** strain
        /// rate, upstream's `tangentialCoefficients`.
        tangential_coefficients: [f64; 6],
        /// Upstream's `fluxConversionFactor` \[-\], default `1.0`. Rescales the
        /// fluence when the fit's fast-neutron energy cut-off differs from the
        /// one the fluence field was accumulated with (e.g. an "equivalent DIDO
        /// nickel dose" against E > 1 MeV).
        flux_conversion_factor: f64,
    },
}

/// The validity window of one variant, used both to clamp and to check.
///
/// Private: the ranges are an implementation detail of this port, documented
/// per variant, and a public accessor would freeze them as API.
#[derive(Debug, Clone, Copy)]
struct Limits {
    /// Human-readable name of the correlation, for the error message.
    quantity: &'static str,
    /// `(low, high)` burnup \[MWd/kgHM\], if burnup is an input.
    burnup: Option<(f64, f64)>,
    /// `(low, high)` fast fluence \[n/m²\], if fluence is an input.
    fluence: Option<(f64, f64)>,
    /// `(low, high)` temperature \[K\], if temperature is an input.
    temperature: Option<(f64, f64)>,
}

impl Limits {
    /// A model with no constrained inputs (e.g. [`SwellingModel::Zero`]).
    const fn unconstrained(quantity: &'static str) -> Self {
        Self {
            quantity,
            burnup: None,
            fluence: None,
            temperature: None,
        }
    }
}

impl SwellingModel {
    /// The three linear swelling strain components \[-\], in the local
    /// radial / hoop / axial frame — the direct analogue of upstream's
    /// `epsilonSwelling` tensor diagonal.
    ///
    /// **Inputs are clamped** to the variant's stated validity range before
    /// evaluation (burnup, fast fluence and temperature, whichever the variant
    /// uses), so this always returns a finite number. Outside the range it
    /// therefore returns the endpoint value rather than an extrapolation —
    /// which is *not* what upstream OFFBEAT does; upstream extrapolates. Use
    /// [`strain_checked`](Self::strain_checked) when you need to know that the
    /// clamp fired.
    ///
    /// Sign: positive is growth in that direction.
    #[must_use]
    pub fn strain(&self, state: &MaterialState) -> SwellingStrain {
        let state = self.clamped(state);
        match self {
            Self::Zero => SwellingStrain::ZERO,

            Self::Constant { swelling_rate } => {
                // Upstream works in units of 1e25 n/m^2 and applies the rate as
                // the linear component in each direction.
                let phi = state.fast_fluence / 1.0e25;
                let component = swelling_rate * phi;
                SwellingStrain::new(component, component, component)
            }

            Self::Uo2Frapcon {
                theoretical_density,
            } => {
                // Upstream's thresholds are in MWd/tHM; MaterialState is
                // MWd/kgHM.
                const LOW_THRESHOLD: f64 = 6000.0;
                const HIGH_THRESHOLD: f64 = 80000.0;
                const RATE_LOW: f64 = 2.974e10 * 2.315e-23 * 86.4;
                const RATE_HIGH: f64 = 2.974e10 * 3.211e-23 * 86.4;

                let burnup = state.burnup * 1000.0;
                let rho = theoretical_density * state.density_fraction();

                let volumetric = if burnup < LOW_THRESHOLD {
                    0.0
                } else if burnup < HIGH_THRESHOLD {
                    (burnup - LOW_THRESHOLD) * RATE_LOW * rho
                } else {
                    ((HIGH_THRESHOLD - LOW_THRESHOLD) * RATE_LOW
                        + (burnup - HIGH_THRESHOLD) * RATE_HIGH)
                        * rho
                };
                SwellingStrain::isotropic(volumetric)
            }

            Self::Uo2Matpro {
                theoretical_density,
            } => {
                const SOLID: f64 = 5.577e-5;
                const GAS: f64 = 1.96e-31;
                const T_REF: f64 = 2800.0;
                const T_EXPONENT: f64 = 11.73;
                const T_DECAY: f64 = -0.0162;
                const BURNUP_DECAY: f64 = -0.0178;
                // MWd/kgHM corresponding to one fission per initial metal atom.
                const MWD_PER_KG_PER_FIMA: f64 = 937.06;

                let rho = theoretical_density * state.density_fraction();
                let fima = state.burnup / MWD_PER_KG_PER_FIMA;

                let solid = SOLID * rho * fima;

                // Analytic integral of upstream's per-burnup-step gas rate,
                // holding temperature fixed over the history.
                let delta_t = T_REF - state.temperature; // >= 0: T is clamped.
                let coefficient = GAS * rho * delta_t.powf(T_EXPONENT) * (T_DECAY * delta_t).exp();
                let decay = BURNUP_DECAY * rho;
                let gas = if decay.abs() < 1.0e-30 {
                    coefficient * fima
                } else {
                    coefficient * ((decay * fima).exp() - 1.0) / decay
                };

                SwellingStrain::isotropic(solid + gas)
            }

            Self::FbrMox { gap_open } => {
                // MWd/kgHM per 1 %FIMA, upstream's `1e-3 / 9.5` on MWd/tHM.
                const MWD_PER_KG_PER_PERCENT_FIMA: f64 = 9.5;
                const RATE_OPEN_LOW: f64 = 0.020;
                const RATE_OPEN_HIGH: f64 = 0.012;
                const RATE_CLOSED: f64 = 0.0065;

                let fima_percent = state.burnup / MWD_PER_KG_PER_PERCENT_FIMA;
                let volumetric = if *gap_open {
                    RATE_OPEN_LOW * fima_percent.min(1.0)
                        + RATE_OPEN_HIGH * (fima_percent - 1.0).max(0.0)
                } else {
                    RATE_CLOSED * fima_percent
                };
                SwellingStrain::isotropic(volumetric)
            }

            Self::FeCrAl { rate } => {
                // Upstream works in n/m^2 here, so no unit conversion.
                let component = rate * state.fast_fluence;
                SwellingStrain::new(component, component, component)
            }

            Self::GrowthBisonZircaloy { clad_type } => {
                let (a, n) = clad_type.coefficients();
                // The BISON coefficients are fitted against fluence in n/cm^2.
                let phi = state.fast_fluence / 1.0e4;
                let axial = a * phi.powf(n);
                let transverse = -(1.0 - (1.0 + axial).powf(-0.5));
                SwellingStrain::new(transverse, transverse, axial)
            }

            Self::GrowthAim11515Ti => SwellingStrain::isotropic(steel_void_swelling(
                &state, 1.3e-5, -2.5, 490.0, 100.0, 3.9,
            )),

            Self::GrowthGeneralized1515Ti => SwellingStrain::isotropic(steel_void_swelling(
                &state, 1.5e-3, -2.5, 450.0, 100.0, 2.75,
            )),

            Self::WrightShamHastelloyN => {
                const A: f64 = 9.845e-1;
                const N: f64 = 4.385e-1;
                const OFFSET: f64 = -9.81e-1;
                const T_PEAK_CELSIUS: f64 = 490.0;
                // dpa per 1e22 n/cm^2, i.e. per 1e26 n/m^2.
                const DPA_PER_1E26: f64 = 5.0;

                let dpa = state.fast_fluence / 1.0e26 * DPA_PER_1E26;
                let dose_factor = A * dpa.powf(N) + OFFSET;
                let x = (state.temperature - 273.15 - T_PEAK_CELSIUS) / 100.0;
                let temperature_factor = (-(x * x)).exp();
                // Upstream's result is a percentage.
                SwellingStrain::isotropic(0.01 * temperature_factor * dose_factor)
            }

            Self::PyroCarbonCorrelation {
                radial_coefficients,
                tangential_coefficients,
                flux_conversion_factor,
            } => {
                let phi = state.fast_fluence / 1.0e25 * flux_conversion_factor;
                let integrate = |coefficients: &[f64; 6]| -> f64 {
                    coefficients
                        .iter()
                        .enumerate()
                        .map(|(i, c)| {
                            let order = i32::try_from(i).unwrap_or(i32::MAX) + 1;
                            c / f64::from(order) * phi.powi(order)
                        })
                        .sum()
                };
                let radial = integrate(radial_coefficients);
                let tangential = integrate(tangential_coefficients);
                SwellingStrain::new(radial, tangential, tangential)
            }
        }
    }

    /// Volumetric swelling strain `ΔV/V` \[-\], **positive for growth**.
    ///
    /// The trace of [`strain`](Self::strain); see
    /// [`SwellingStrain::volumetric`] for the small-strain caveat.
    ///
    /// **Inputs are clamped** to the variant's stated validity range before
    /// evaluation — see [`strain`](Self::strain). Use
    /// [`value_checked`](Self::value_checked) to be told instead.
    ///
    /// **This is near zero for
    /// [`GrowthBisonZircaloy`](Self::GrowthBisonZircaloy)**, whose deformation
    /// is volume-conserving; that variant must be read through
    /// [`strain`](Self::strain).
    ///
    /// ```
    /// use outram_park_fork_offbeat::materials::MaterialState;
    /// use outram_park_fork_offbeat::materials::behavioral::swelling::SwellingModel;
    ///
    /// let model = SwellingModel::Uo2Frapcon { theoretical_density: 10_960.0 };
    ///
    /// // Fresh fuel has not swollen.
    /// assert_eq!(model.value(&MaterialState::fresh(600.0)), 0.0);
    ///
    /// // At 40 MWd/kgHM it has, and the sign is positive.
    /// let mut aged = MaterialState::fresh(900.0);
    /// aged.burnup = 40.0;
    /// assert!(model.value(&aged) > 0.0);
    /// ```
    #[must_use]
    pub fn value(&self, state: &MaterialState) -> f64 {
        self.strain(state).volumetric()
    }

    /// [`strain`](Self::strain), but returning [`OffbeatError::OutOfRange`]
    /// instead of clamping when burnup, fast fluence or temperature falls
    /// outside the variant's stated validity range.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::OutOfRange`] naming the offending input and the bounds.
    /// Inputs are checked in the order burnup, fluence, temperature, so a state
    /// that violates two ranges reports the first.
    pub fn strain_checked(&self, state: &MaterialState) -> Result<SwellingStrain> {
        self.check(state)?;
        Ok(self.strain(state))
    }

    /// [`value`](Self::value), but returning [`OffbeatError::OutOfRange`]
    /// instead of clamping when an input falls outside the variant's stated
    /// validity range.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::OutOfRange`] naming the offending input and the bounds.
    ///
    /// ```
    /// use outram_park_fork_offbeat::materials::MaterialState;
    /// use outram_park_fork_offbeat::materials::behavioral::swelling::SwellingModel;
    ///
    /// let model = SwellingModel::Uo2Frapcon { theoretical_density: 10_960.0 };
    /// let mut absurd = MaterialState::fresh(900.0);
    /// absurd.burnup = 5_000.0; // MWd/kgHM — far past any real fuel
    ///
    /// assert!(model.value_checked(&absurd).is_err());
    /// assert!(model.value(&absurd).is_finite()); // clamped, not an error
    /// ```
    pub fn value_checked(&self, state: &MaterialState) -> Result<f64> {
        Ok(self.strain_checked(state)?.volumetric())
    }

    /// The validity window of this variant. See each variant's documentation
    /// for the range and the reasoning behind it.
    fn limits(&self) -> Limits {
        match self {
            Self::Zero => Limits::unconstrained("swelling (none)"),
            Self::Constant { .. } => Limits {
                fluence: Some((0.0, 1.0e27)),
                ..Limits::unconstrained("constant swelling")
            },
            Self::Uo2Frapcon { .. } => Limits {
                burnup: Some((0.0, 120.0)),
                ..Limits::unconstrained("UO2 FRAPCON swelling")
            },
            Self::Uo2Matpro { .. } => Limits {
                burnup: Some((0.0, 120.0)),
                temperature: Some((300.0, 2800.0)),
                ..Limits::unconstrained("UO2 MATPRO swelling")
            },
            Self::FbrMox { .. } => Limits {
                burnup: Some((0.0, 250.0)),
                ..Limits::unconstrained("FBR MOX swelling")
            },
            Self::FeCrAl { .. } => Limits {
                fluence: Some((0.0, 2.0e26)),
                ..Limits::unconstrained("FeCrAl swelling")
            },
            Self::GrowthBisonZircaloy { .. } => Limits {
                fluence: Some((0.0, 1.5e26)),
                ..Limits::unconstrained("BISON Zircaloy irradiation growth")
            },
            Self::GrowthAim11515Ti => Limits {
                fluence: Some((0.0, 3.0e27)),
                temperature: Some((573.15, 1023.15)),
                ..Limits::unconstrained("AIM1 15-15Ti void swelling")
            },
            Self::GrowthGeneralized1515Ti => Limits {
                fluence: Some((0.0, 3.0e27)),
                temperature: Some((573.15, 1023.15)),
                ..Limits::unconstrained("generalised 15-15Ti void swelling")
            },
            Self::WrightShamHastelloyN => Limits {
                fluence: Some((0.0, 4.0e26)),
                temperature: Some((573.15, 1073.15)),
                ..Limits::unconstrained("Wright-Sham Hastelloy N void swelling")
            },
            Self::PyroCarbonCorrelation { .. } => Limits {
                fluence: Some((0.0, 4.0e25)),
                ..Limits::unconstrained("pyrolytic carbon swelling correlation")
            },
        }
    }

    /// A copy of `state` with every input this variant reads clamped into its
    /// validity range.
    fn clamped(&self, state: &MaterialState) -> MaterialState {
        let limits = self.limits();
        let mut clamped = *state;
        if let Some((low, high)) = limits.burnup {
            clamped.burnup = clamped.burnup.clamp(low, high);
        }
        if let Some((low, high)) = limits.fluence {
            clamped.fast_fluence = clamped.fast_fluence.clamp(low, high);
        }
        if let Some((low, high)) = limits.temperature {
            clamped.temperature = clamped.temperature.clamp(low, high);
        }
        clamped
    }

    /// `Ok(())` if every input this variant reads lies in its validity range.
    fn check(&self, state: &MaterialState) -> Result<()> {
        let limits = self.limits();
        let checks = [
            (limits.burnup, state.burnup, "MWd/kgHM"),
            (limits.fluence, state.fast_fluence, "n/m^2"),
            (limits.temperature, state.temperature, "K"),
        ];
        for (range, value, unit) in checks {
            if let Some((low, high)) = range {
                if value < low || value > high {
                    return Err(OffbeatError::OutOfRange {
                        quantity: limits.quantity,
                        value,
                        low,
                        high,
                        unit,
                    });
                }
            }
        }
        Ok(())
    }
}

/// Austenitic-steel void swelling, the functional form shared by the two
/// 15-15Ti fits.
///
/// `ΔV/V [-] = 0.01 · a · exp(b · ((T_C − t_peak)/t_width)²) · φ22^m`
///
/// with `T_C` the temperature in °C and `φ22` the fast fluence in units of
/// `1e22` n/cm² (SI fluence / 1e26). The `0.01` converts upstream's percentage
/// to a strain. Returns the **volumetric** strain; upstream writes one third of
/// it into each tensor component.
fn steel_void_swelling(
    state: &MaterialState,
    a: f64,
    b: f64,
    t_peak_celsius: f64,
    t_width: f64,
    dose_exponent: f64,
) -> f64 {
    let t_celsius = state.temperature - 273.15;
    let phi22 = state.fast_fluence / 1.0e26;
    let x = (t_celsius - t_peak_celsius) / t_width;
    0.01 * a * (b * x * x).exp() * phi22.powf(dose_exponent)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UO2 at 95% of theoretical density and 900 K — a representative LWR
    /// pellet mid-radius condition.
    fn uo2_state(burnup: f64) -> MaterialState {
        let mut state = MaterialState::fresh(900.0);
        state.porosity = 0.05;
        state.burnup = burnup;
        state
    }

    #[test]
    fn zero_model_never_swells() {
        let model = SwellingModel::Zero;
        let mut state = MaterialState::fresh(1200.0);
        state.burnup = 60.0;
        state.fast_fluence = 1.0e26;
        assert_eq!(model.value(&state), 0.0);
        assert_eq!(model.strain(&state), SwellingStrain::ZERO);
        assert_eq!(model.value_checked(&state).unwrap(), 0.0);
    }

    /// Self-consistency check, not external validation: fresh fuel has zero
    /// burnup and zero fluence, so every burnup- or fluence-driven model must
    /// report exactly zero swelling. No published reference is involved — this
    /// establishes only that no variant carries a spurious constant offset.
    ///
    /// [`SwellingModel::WrightShamHastelloyN`] is excluded and tested
    /// separately: its correlation is deliberately *not* zero at zero dose.
    #[test]
    fn every_model_is_zero_in_the_unirradiated_state() {
        let fresh = MaterialState::fresh(900.0);
        let models = [
            SwellingModel::Zero,
            SwellingModel::Constant {
                swelling_rate: 1.0e-3,
            },
            SwellingModel::Uo2Frapcon {
                theoretical_density: 10_960.0,
            },
            SwellingModel::Uo2Matpro {
                theoretical_density: 10_960.0,
            },
            SwellingModel::FbrMox { gap_open: true },
            SwellingModel::FbrMox { gap_open: false },
            SwellingModel::FeCrAl { rate: 4.5e-29 },
            SwellingModel::GrowthBisonZircaloy {
                clad_type: BisonZircaloyCladType::Escore,
            },
            SwellingModel::GrowthAim11515Ti,
            SwellingModel::GrowthGeneralized1515Ti,
            SwellingModel::PyroCarbonCorrelation {
                radial_coefficients: [-1.2, 0.1, 0.05, -0.01, 0.0, 0.0],
                tangential_coefficients: [-1.2, 0.05, 0.08, -0.012, 0.0, 0.0],
                flux_conversion_factor: 1.0,
            },
        ];
        for model in models {
            let value = model.value(&fresh);
            assert!(
                value.abs() < 1.0e-15,
                "{model:?} gives {value} in the unirradiated state, expected 0"
            );
        }
    }

    /// Self-consistency check, not external validation: swelling must never
    /// decrease with burnup. Checked on a burnup ladder that crosses both
    /// FRAPCON regime boundaries (6 and 80 MWd/kgHM) and the FBR-MOX rate
    /// change (1 %FIMA = 9.5 MWd/kgHM).
    #[test]
    fn burnup_driven_swelling_is_monotonic() {
        let models = [
            SwellingModel::Uo2Frapcon {
                theoretical_density: 10_960.0,
            },
            SwellingModel::Uo2Matpro {
                theoretical_density: 10_960.0,
            },
            SwellingModel::FbrMox { gap_open: true },
            SwellingModel::FbrMox { gap_open: false },
        ];
        let ladder = [0.0, 1.0, 5.9, 6.1, 9.4, 9.6, 40.0, 79.9, 80.1, 110.0];
        for model in models {
            let mut previous = f64::NEG_INFINITY;
            for burnup in ladder {
                let value = model.value(&uo2_state(burnup));
                assert!(
                    value >= previous - 1.0e-15,
                    "{model:?} is not monotonic in burnup: {value} at {burnup} \
                     MWd/kgHM follows {previous}"
                );
                assert!(
                    value >= 0.0,
                    "{model:?} gives negative swelling at {burnup}"
                );
                previous = value;
            }
        }
    }

    /// **Cross-correlation consistency check** between two independently
    /// published UO2 solid-fission-product swelling fits. Not external
    /// validation — neither model is a reference for the other — but a genuine
    /// check that both ports carry the right unit conversions, since a stray
    /// factor of 1000 or of 0.881 anywhere would show up immediately.
    ///
    /// # Methodology
    ///
    /// - Inputs: UO2 at `ρ = 10 400` kg/m³ (`theoretical_density = 10 400`,
    ///   porosity 0), `T = 600` K, burnup swept from 20 to 60 MWd/kgHM.
    /// - `T = 600` K is chosen so MATPRO's gaseous term is negligible,
    ///   isolating the solid term the two fits share.
    /// - Compared quantity: the *increment* of volumetric swelling over
    ///   20 → 60 MWd/kgHM, which removes FRAPCON's 6 MWd/kgHM threshold offset.
    /// - Pass criterion: relative difference below 2e-3.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// - FRAPCON increment: `2.474566e-2`
    /// - MATPRO increment:  `2.475874e-2`
    /// - relative difference: `5.287e-4` — well inside the 2e-3 criterion.
    ///
    /// # Interpretation
    ///
    /// The two correlations' solid-swelling slopes agree to about 0.05%:
    /// FRAPCON's `2.974e10 · 2.315e-23 · 86.4 · ρ · 1000` = `6.1864e-4` per
    /// MWd/kgHM against MATPRO's `5.577e-5 · ρ / 937.06` = `6.1897e-4` per
    /// MWd/kgHM. That they agree is evidence about the *ports*, not about the
    /// physics: it confirms the MWd/tHM, MWd/kgHM and FIMA conversions in both
    /// variants, and would break loudly if any of them were wrong.
    #[test]
    fn frapcon_and_matpro_solid_swelling_slopes_agree() {
        let frapcon = SwellingModel::Uo2Frapcon {
            theoretical_density: 10_400.0,
        };
        let matpro = SwellingModel::Uo2Matpro {
            theoretical_density: 10_400.0,
        };

        let state = |burnup: f64| {
            let mut state = MaterialState::fresh(600.0);
            state.burnup = burnup;
            state
        };

        let delta_frapcon = frapcon.value(&state(60.0)) - frapcon.value(&state(20.0));
        let delta_matpro = matpro.value(&state(60.0)) - matpro.value(&state(20.0));

        let relative = (delta_frapcon - delta_matpro).abs() / delta_frapcon;
        assert!(
            relative < 2.0e-3,
            "FRAPCON {delta_frapcon:e} vs MATPRO {delta_matpro:e}: relative \
             difference {relative:e} exceeds 2e-3"
        );
    }

    /// **Verification of this port's analytic integration against upstream's
    /// algorithm.** Upstream's `swellingMATPRO::correct` accumulates
    /// `rate(Bu) · ΔBu` per timestep; this port evaluates the closed-form
    /// integral instead (see [`SwellingModel::Uo2Matpro`]). The two must agree
    /// as the burnup step goes to zero — if they do not, the integral is wrong.
    ///
    /// # Methodology
    ///
    /// - Inputs: UO2, `theoretical_density = 10 960` kg/m³, porosity 0.05
    ///   (`ρ = 10 412` kg/m³), `T = 1200` K, final burnup 50 MWd/kgHM.
    /// - Reference: a forward-Euler sum reproducing upstream's increment
    ///   exactly (the `solid + gas` rate evaluated at each step's start, times
    ///   `ΔFIMA`) over 200 000 steps.
    /// - Pass criterion: relative difference below 1e-4.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// - closed form:          `3.332406e-2`
    /// - Euler, 200 000 steps: `3.332412e-2`
    /// - relative difference:  `1.74e-6`, falling linearly with step count as
    ///   first-order accumulation error should.
    ///
    /// # Interpretation
    ///
    /// The analytic integral reproduces upstream's discrete accumulation in the
    /// small-step limit, so replacing the stateful accumulation with a pure
    /// function does not change the physics at constant temperature. It says
    /// nothing about a varying-temperature history, where the two genuinely
    /// differ.
    #[test]
    fn matpro_closed_form_matches_upstream_euler_accumulation() {
        const THEORETICAL_DENSITY: f64 = 10_960.0;
        const POROSITY: f64 = 0.05;
        const TEMPERATURE: f64 = 1200.0;
        const BURNUP: f64 = 50.0;
        const STEPS: usize = 200_000;

        let model = SwellingModel::Uo2Matpro {
            theoretical_density: THEORETICAL_DENSITY,
        };
        let mut state = MaterialState::fresh(TEMPERATURE);
        state.porosity = POROSITY;
        state.burnup = BURNUP;
        let closed_form = model.value(&state);

        // Upstream's increment, verbatim (its `/3` per component times three
        // components is the volumetric strain).
        let rho = THEORETICAL_DENSITY * state.density_fraction();
        let fima_end = BURNUP / 937.06;
        let d_fima = fima_end / STEPS as f64;
        let delta_t = 2800.0 - TEMPERATURE;
        let mut euler = 0.0;
        for i in 0..STEPS {
            let fima = i as f64 * d_fima;
            let solid = 5.577e-5 * rho * d_fima;
            let gas = 1.96e-31
                * rho
                * d_fima
                * delta_t.powf(11.73)
                * (-0.0162 * delta_t).exp()
                * (-0.0178 * rho * fima).exp();
            euler += solid + gas;
        }

        let relative = (closed_form - euler).abs() / closed_form;
        assert!(
            relative < 1.0e-4,
            "closed form {closed_form:e} vs Euler {euler:e}: relative \
             difference {relative:e} exceeds 1e-4"
        );
    }

    /// Self-consistency check, not external validation: Zircaloy irradiation
    /// growth conserves volume by construction, so its volumetric strain must
    /// be tiny while its axial strain is not. This is the property that makes
    /// [`SwellingModel::value`] the wrong accessor for this variant, so it is
    /// worth pinning.
    #[test]
    fn zircaloy_growth_is_volume_conserving_and_anisotropic() {
        let model = SwellingModel::GrowthBisonZircaloy {
            clad_type: BisonZircaloyCladType::Escore,
        };
        let mut state = MaterialState::fresh(600.0);
        state.fast_fluence = 1.0e26;

        let strain = model.strain(&state);
        assert!(strain.axial > 0.0, "axial growth must be positive");
        assert!(
            strain.radial < 0.0,
            "transverse contraction must be negative"
        );
        assert_eq!(
            strain.radial, strain.hoop,
            "growth is transversely isotropic"
        );
        assert!(
            strain.volumetric().abs() < 0.01 * strain.axial,
            "volumetric strain {} is not small against axial {}",
            strain.volumetric(),
            strain.axial
        );
    }

    /// **Verification of this port's closed form against upstream's
    /// increment-wise accumulation** for the transverse component of Zircaloy
    /// growth. Upstream applies `−(1 − (1+Δε)^(−1/2))` to each timestep's
    /// increment and sums; this port applies it once to the total.
    ///
    /// # Methodology
    ///
    /// - Inputs: ESCORE coefficients, fast fluence swept 0 → 1e26 n/m².
    /// - Reference: 100 000-step accumulation of upstream's increment formula.
    /// - Pass criterion: relative difference below 1e-2. The two are *not*
    ///   expected to agree exactly — the transverse mapping is nonlinear, so
    ///   summing it over increments drops the second-order term. The criterion
    ///   is set to catch a wrong exponent or a wrong sign, which would be an
    ///   order-unity error, while tolerating the genuine second-order gap.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// - closed form transverse: `−4.377561e-3`
    /// - accumulated transverse: `−4.406474e-3`
    /// - relative difference: `6.605e-3`; absolute difference `2.9e-5` in
    ///   strain.
    ///
    /// # Interpretation
    ///
    /// The gap is exactly the `3ε²/8` term that increment-wise summation
    /// discards — at `ε_axial = 8.8e-3` that is `2.9e-5`, which is what is
    /// measured. The closed form is therefore the more nearly exact of the two,
    /// and the difference is far below any measurement uncertainty on cladding
    /// growth. Neither form is wrong; they are the same law evaluated with and
    /// without a first-order approximation.
    #[test]
    fn zircaloy_growth_closed_form_matches_incremental_accumulation() {
        const STEPS: usize = 100_000;
        const FLUENCE: f64 = 1.0e26;

        let model = SwellingModel::GrowthBisonZircaloy {
            clad_type: BisonZircaloyCladType::Escore,
        };
        let mut state = MaterialState::fresh(600.0);
        state.fast_fluence = FLUENCE;
        let closed_form = model.strain(&state).radial;

        let (a, n) = BisonZircaloyCladType::Escore.coefficients();
        let mut accumulated = 0.0;
        for i in 0..STEPS {
            let phi_old = (i as f64 / STEPS as f64) * FLUENCE / 1.0e4;
            let phi_new = ((i + 1) as f64 / STEPS as f64) * FLUENCE / 1.0e4;
            let increment = a * phi_new.powf(n) - a * phi_old.powf(n);
            accumulated += -(1.0 - (1.0 + increment).powf(-0.5));
        }

        let relative = (closed_form - accumulated).abs() / closed_form.abs();
        assert!(
            relative < 1.0e-2,
            "closed form {closed_form:e} vs accumulated {accumulated:e}: \
             relative difference {relative:e} exceeds 1e-3"
        );
    }

    /// Self-consistency check, not external validation: the FRAPCON UO2 fit is
    /// piecewise linear with a documented threshold at 6 MWd/kgHM and a slope
    /// change at 80 MWd/kgHM. This pins both, the continuity of the fit at the
    /// 80 MWd/kgHM knee, and the ratio of the two slopes.
    #[test]
    fn frapcon_uo2_thresholds_and_continuity() {
        let model = SwellingModel::Uo2Frapcon {
            theoretical_density: 10_960.0,
        };
        assert_eq!(
            model.value(&uo2_state(5.999)),
            0.0,
            "no swelling below 6 MWd/kgHM"
        );
        assert!(model.value(&uo2_state(6.001)) > 0.0);

        let below = model.value(&uo2_state(80.0 - 1.0e-6));
        let above = model.value(&uo2_state(80.0 + 1.0e-6));
        assert!(
            (above - below).abs() < 1.0e-8,
            "the fit is discontinuous at the 80 MWd/kgHM knee: {below} vs {above}"
        );

        // The second regime is steeper, in the ratio 3.211/2.315.
        let slope_low = model.value(&uo2_state(40.0)) - model.value(&uo2_state(30.0));
        let slope_high = model.value(&uo2_state(100.0)) - model.value(&uo2_state(90.0));
        let ratio = slope_high / slope_low;
        assert!(
            (ratio - 3.211 / 2.315).abs() < 1.0e-9,
            "slope ratio {ratio} does not match the upstream coefficient ratio"
        );
    }

    /// Self-consistency check, not external validation: the FBR-MOX rate law is
    /// a piecewise integral, so the value at 1 %FIMA must be exactly the first
    /// rate, and the gap-closed history must swell less than the gap-open one
    /// at every burnup.
    #[test]
    fn fbr_mox_rates_integrate_as_documented() {
        let open = SwellingModel::FbrMox { gap_open: true };
        let closed = SwellingModel::FbrMox { gap_open: false };

        // 1 %FIMA = 9.5 MWd/kgHM
        let at_one_fima = uo2_state(9.5);
        assert!((open.value(&at_one_fima) - 0.020).abs() < 1.0e-12);
        assert!((closed.value(&at_one_fima) - 0.0065).abs() < 1.0e-12);

        // 10 %FIMA: 0.020*1 + 0.012*9 = 0.128
        let at_ten_fima = uo2_state(95.0);
        assert!((open.value(&at_ten_fima) - 0.128).abs() < 1.0e-12);

        for burnup in [1.0, 9.5, 50.0, 95.0, 200.0] {
            let state = uo2_state(burnup);
            assert!(
                closed.value(&state) < open.value(&state),
                "a closed gap must suppress swelling at {burnup} MWd/kgHM"
            );
        }
    }

    /// Self-consistency check, not external validation: both 15-15Ti fits peak
    /// at their stated temperature and rise steeply with dose. No published
    /// swelling datum is being reproduced here — only the shape upstream's
    /// algebra implies.
    #[test]
    fn steel_void_swelling_peaks_at_the_fitted_temperature() {
        let cases = [
            (SwellingModel::GrowthAim11515Ti, 490.0 + 273.15),
            (SwellingModel::GrowthGeneralized1515Ti, 450.0 + 273.15),
        ];
        for (model, t_peak) in cases {
            let at = |temperature: f64| {
                let mut state = MaterialState::fresh(temperature);
                state.fast_fluence = 1.0e27;
                model.value(&state)
            };
            let peak = at(t_peak);
            assert!(peak > at(t_peak - 80.0));
            assert!(peak > at(t_peak + 80.0));
            assert!(peak > 0.0, "{model:?} must swell positively at its peak");

            // Steeply superlinear in dose (exponents 3.9 and 2.75).
            let mut low = MaterialState::fresh(t_peak);
            low.fast_fluence = 1.0e27;
            let mut high = MaterialState::fresh(t_peak);
            high.fast_fluence = 2.0e27;
            assert!(model.value(&high) > 4.0 * model.value(&low));
        }
    }

    /// Documents — rather than hides — the Wright-Sham correlation's negative
    /// branch below its incubation dose. Self-consistency check against the
    /// algebra of upstream's fit, not a validation of Hastelloy N behaviour.
    ///
    /// `0.9845·dpa^0.4385 = 0.981` at `dpa ≈ 0.9919`, i.e. at a fast fluence of
    /// about `1.98e25` n/m² with upstream's 5 dpa per 1e26 n/m². Below that the
    /// correlation returns a negative "swelling"; above it, positive.
    ///
    /// Measured 2026-07-29, this port, at the 490 °C swelling peak:
    /// `−8.29e-6` at `1.98e25` n/m² and `+1.34e-5` at `1.99e25` n/m², so the
    /// crossing is bracketed to better than 1% in fluence.
    #[test]
    fn wright_sham_hastelloy_n_is_negative_below_its_incubation_dose() {
        let model = SwellingModel::WrightShamHastelloyN;
        let at = |fluence: f64| {
            let mut state = MaterialState::fresh(490.0 + 273.15);
            state.fast_fluence = fluence;
            model.value(&state)
        };
        assert!(
            at(1.0e25) < 0.0,
            "below incubation the upstream fit is negative"
        );
        assert!(at(4.0e26) > 0.0, "above incubation it swells");
        // Crossing point, bracketed to better than 1% in fluence.
        assert!(at(1.98e25) < 0.0);
        assert!(at(1.99e25) > 0.0);
    }

    /// Self-consistency check, not external validation: the pyrolytic-carbon
    /// correlation is the integral of the supplied rate polynomial, so a
    /// single-term coefficient set must reproduce that term's integral exactly.
    ///
    /// With `A_r = [a, 0, 0, 0, 0, 0]` the strain is `a·φ`; with
    /// `A_r = [0, b, 0, 0, 0, 0]` it is `b·φ²/2`. Both are checked to machine
    /// precision, which pins the `1/(i+1)` integration factors and the
    /// `1e25 n/m²` fluence scaling.
    #[test]
    fn pyrocarbon_correlation_integrates_its_rate_polynomial() {
        let mut state = MaterialState::fresh(1300.0);
        state.fast_fluence = 2.0e25; // phi = 2.0 in units of 1e25 n/m^2

        let linear = SwellingModel::PyroCarbonCorrelation {
            radial_coefficients: [-1.5, 0.0, 0.0, 0.0, 0.0, 0.0],
            tangential_coefficients: [0.0; 6],
            flux_conversion_factor: 1.0,
        };
        assert!((linear.strain(&state).radial - (-1.5 * 2.0)).abs() < 1.0e-15);

        let quadratic = SwellingModel::PyroCarbonCorrelation {
            radial_coefficients: [0.0, 0.3, 0.0, 0.0, 0.0, 0.0],
            tangential_coefficients: [0.0; 6],
            flux_conversion_factor: 1.0,
        };
        assert!((quadratic.strain(&state).radial - (0.3 * 4.0 / 2.0)).abs() < 1.0e-15);

        // The tangential component is written to BOTH hoop and axial.
        let anisotropic = SwellingModel::PyroCarbonCorrelation {
            radial_coefficients: [-1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            tangential_coefficients: [0.5, 0.0, 0.0, 0.0, 0.0, 0.0],
            flux_conversion_factor: 1.0,
        };
        let strain = anisotropic.strain(&state);
        assert!((strain.radial - (-2.0)).abs() < 1.0e-15);
        assert!((strain.hoop - 1.0).abs() < 1.0e-15);
        assert_eq!(strain.hoop, strain.axial);

        // The flux conversion factor scales the fluence, not the result.
        let scaled = SwellingModel::PyroCarbonCorrelation {
            radial_coefficients: [0.0, 0.3, 0.0, 0.0, 0.0, 0.0],
            tangential_coefficients: [0.0; 6],
            flux_conversion_factor: 0.5,
        };
        assert!((scaled.strain(&state).radial - (0.3 * 1.0 / 2.0)).abs() < 1.0e-15);
    }

    /// The constant model is exactly linear in fluence and isotropic, and its
    /// rate is the *linear* component, so the volumetric strain is three times
    /// the rate. Pins the documented factor of three.
    #[test]
    fn constant_model_rate_is_the_linear_component() {
        let model = SwellingModel::Constant {
            swelling_rate: 1.0e-3,
        };
        let mut state = MaterialState::fresh(600.0);
        state.fast_fluence = 1.0e25;

        let strain = model.strain(&state);
        assert!((strain.radial - 1.0e-3).abs() < 1.0e-18);
        assert!((model.value(&state) - 3.0e-3).abs() < 1.0e-18);
    }

    /// FeCrAl is linear in SI fluence with no unit conversion, unlike its
    /// neighbours. Pins that, because getting it wrong is a factor of 1e4.
    #[test]
    fn fecral_is_linear_in_si_fluence() {
        let model = SwellingModel::FeCrAl { rate: 4.5e-29 };
        let mut state = MaterialState::fresh(600.0);
        state.fast_fluence = 1.0e26;
        assert!((model.strain(&state).radial - 4.5e-3).abs() < 1.0e-15);
        assert!((model.value(&state) - 1.35e-2).abs() < 1.0e-15);
    }

    /// `value` clamps and `value_checked` refuses — the documented contract of
    /// this module. Checked on every kind of input a variant can read.
    #[test]
    fn out_of_range_inputs_clamp_in_value_and_error_in_value_checked() {
        // Burnup.
        let frapcon = SwellingModel::Uo2Frapcon {
            theoretical_density: 10_960.0,
        };
        let at_limit = uo2_state(120.0);
        let beyond = uo2_state(500.0);
        assert_eq!(frapcon.value(&beyond), frapcon.value(&at_limit));
        assert!(matches!(
            frapcon.value_checked(&beyond),
            Err(OffbeatError::OutOfRange { .. })
        ));

        // Temperature: above 2800 K the MATPRO gas term is NaN unclamped.
        let matpro = SwellingModel::Uo2Matpro {
            theoretical_density: 10_960.0,
        };
        let mut hot = uo2_state(40.0);
        hot.temperature = 3500.0;
        assert!(
            matpro.value(&hot).is_finite(),
            "clamping must keep (2800 - T)^11.73 real"
        );
        assert!(matches!(
            matpro.value_checked(&hot),
            Err(OffbeatError::OutOfRange { .. })
        ));

        // Fluence.
        let growth = SwellingModel::GrowthBisonZircaloy {
            clad_type: BisonZircaloyCladType::M5,
        };
        let mut over = MaterialState::fresh(600.0);
        over.fast_fluence = 1.0e30;
        let mut at = MaterialState::fresh(600.0);
        at.fast_fluence = 1.5e26;
        assert_eq!(growth.strain(&over), growth.strain(&at));
        assert!(matches!(
            growth.strain_checked(&over),
            Err(OffbeatError::OutOfRange { .. })
        ));

        // A negative fluence would give NaN through `powf`; the clamp stops it.
        let mut negative = MaterialState::fresh(600.0);
        negative.fast_fluence = -1.0e25;
        assert!(growth.value(&negative).is_finite());
    }

    /// Every BISON clad type is a distinct, positive, monotonically growing
    /// law, and none of them predicts more than a few percent axial growth at
    /// LWR end-of-life fluence. Self-consistency check on the coefficient
    /// table, not a validation against growth measurements.
    #[test]
    fn every_bison_clad_type_grows_monotonically() {
        let types = [
            BisonZircaloyCladType::Sra,
            BisonZircaloyCladType::Rxa,
            BisonZircaloyCladType::Pra,
            BisonZircaloyCladType::Zirlo,
            BisonZircaloyCladType::Escore,
            BisonZircaloyCladType::M5,
        ];
        for clad_type in types {
            let model = SwellingModel::GrowthBisonZircaloy { clad_type };
            let mut previous = 0.0;
            for fluence in [1.0e24, 1.0e25, 5.0e25, 1.0e26] {
                let mut state = MaterialState::fresh(600.0);
                state.fast_fluence = fluence;
                let axial = model.strain(&state).axial;
                assert!(
                    axial > previous,
                    "{clad_type:?} is not monotonic in fluence"
                );
                previous = axial;
            }
            assert!(
                previous < 0.05,
                "{clad_type:?} predicts {previous} axial growth at 1e26 n/m^2"
            );
        }
        assert_eq!(
            BisonZircaloyCladType::default(),
            BisonZircaloyCladType::Escore
        );
    }

    #[test]
    fn swelling_strain_helpers_are_consistent() {
        let isotropic = SwellingStrain::isotropic(0.03);
        assert!((isotropic.radial - 0.01).abs() < 1.0e-18);
        assert!((isotropic.volumetric() - 0.03).abs() < 1.0e-18);
        assert_eq!(SwellingStrain::ZERO.volumetric(), 0.0);
        assert_eq!(SwellingStrain::new(1.0, 2.0, 3.0).volumetric(), 6.0);
    }
}
