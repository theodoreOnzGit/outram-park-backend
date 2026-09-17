//! Shortcut distillation column — Fenske / Underwood / Gilliland (FUG) design.
//!
//! Pure-Rust port of DWSIM's shortcut column (GPL-3.0). Upstream project:
//! **DWSIM** (<https://github.com/DanWBR/dwsim>, branch `windows`), source file
//! `DWSIM.UnitOperations/UnitOperations/ShortcutColumn.vb` (1006 lines), pinned
//! commit `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (2026-07-17). Upstream
//! copyright: 2008-2013 Daniel Wagner O. de Medeiros. The Brent scalar
//! minimiser is ported from the same project's `DWSIM.Math/BrentMinimize.vb`
//! (`brentoptimize2`), copyright 2008 Daniel Wagner O. de Medeiros.
//!
//! > **⚠️ Untrusted AI-assisted draft — no human V&V.** Verified against hand
//! > arithmetic and cross-checked against this crate's rigorous MESH solvers
//! > (see the tests below), **not** validated against experimental data or
//! > DWSIM's own output. Not for nuclear facility operation, reactor control,
//! > safety-critical, or licensing decisions. Independent OUTRAM PARK fork,
//! > not the official DWSIM.
//!
//! # What belongs here
//!
//! The *shortcut* (approximate, correlation-based) sizing of a simple
//! distillation column: one feed, a top and a bottom product, a condenser
//! (total or partial) and a reboiler. Its outputs — stage count, feed stage,
//! minimum reflux — are exactly the design inputs the **rigorous** MESH column
//! in the sibling modules ([`crate::columns::initial_estimates::RigorousColumn`])
//! needs before it can be solved, which is why this module lives in
//! `columns/` rather than at the crate top level. What does **not** belong
//! here: rigorous stage-by-stage solving (the sibling solvers do that), and
//! any of DWSIM's GUI/serialization plumbing (see "Excluded upstream
//! behavior").
//!
//! # Correlations the upstream file genuinely implements
//!
//! Verified against the source, not assumed from the textbook:
//!
//! - **Fenske** minimum stages at total reflux (`ShortcutColumn.vb:238-243`):
//!   `Nmin = ln[(xd_lk/xd_hk)(xb_hk/xb_lk)] / ln(alpha_lk/alpha_hk)`, with
//!   relative volatilities `alpha_i = K_i/K_hk` evaluated **once** at the feed
//!   temperature and pressure.
//! - **Hengstebeck-Geddes-style non-key distribution** (`:245-272`): a
//!   constant `C` from the two key splits, then
//!   `xd_i/xb_i = 10^(Nmin*log10(alpha_i) + C)` for every non-key, iterated on
//!   the distillate rate `D` to a relative tolerance of `1e-4`.
//! - **Underwood** minimum reflux (`:284-403`), in two modes:
//!   [`UnderwoodMode::SingleRoot`] — one root `theta` of
//!   `Sum_i alpha_i z_i/(alpha_i - theta) = 1 - q` between `alpha_hk` and
//!   `alpha_lk`, found by Brent *minimisation* of the squared residual
//!   (`rminfunc`, `:567-580`), then `Rmin = Sum_i alpha_i xd_i/(alpha_i -
//!   theta) - 1`; or [`UnderwoodMode::DistributedKeys`] — when the Shiras-type
//!   criterion (`Dr`, `:294`) flags non-keys distributing between the
//!   products, one root per inter-volatility gap and a dense linear solve for
//!   `Rmin` and the distributed overhead fractions (`:325-392`).
//! - **Gilliland** actual stages in the **Eduljee analytic fit** (`:409-414`):
//!   `X = (R - Rmin)/(R + 1)`, `Y = 0.75(1 - X^0.5668)`,
//!   `N = (Y + Nmin)/(1 - Y)`. Note this fit is **finite at `R = Rmin`**
//!   (`X = 0` gives `Y = 0.75`, so `N -> 4 Nmin + 3`): the ported correlation
//!   does *not* diverge at minimum reflux, unlike the conceptual limit.
//! - **Feed stage by the Fenske-ratio method** (`:512-519`), **not**
//!   Kirkbride: `Ns = N * [ln((z_lk/z_hk)(xb_hk/xb_lk))/ln(alpha_lk)] / Nmin`.
//!   `(z_lk/z_hk)(xb_hk/xb_lk)` is the feed-to-bottoms key separation, so
//!   `Ns` is the **stripping-section** stage count — the feed sits `Ns`
//!   theoretical stages above the reboiler. Upstream exposes this number as
//!   "Optimal Feed Stage" (`ofs`).
//! - **Feed thermal quality `q` from enthalpy** (`:170-187`): when the
//!   supplied liquid fraction is exactly 0 or 1 (a flash cannot distinguish
//!   saturated from subcooled/superheated), `q` is recomputed as
//!   `q = 1 + (Hbub - H)/(Hdew - Hbub)`, which gives `q > 1` for subcooled
//!   liquid and `q < 0` for superheated vapour.
//! - **Condenser/reboiler duties from bubble/dew flashes** of the products
//!   (`:446-510`), and **column sizing** (`:537-561`): height
//!   `(N + 2) * stage_height`; diameter from a Souders-Brown-type maximum
//!   vapour velocity `uv = (-0.17 lt^2 + 0.27 lt - 0.047) *
//!   sqrt((rho_l - rho_v)/rho_v)` (a Fair-style capacity quadratic in the tray
//!   spacing `lt` \[m\]) with ideal-gas vapour density and Rackett liquid
//!   density.
//!
//! # Units
//!
//! `uom` at the public boundary with the sibling module's named aliases
//! ([`MolarFlowRate`], [`StagePressure`], [`StageTemperature`]) plus
//! [`ShortcutHeatDuty`]; documented raw `f64` SI internally (crate
//! `CLAUDE.md`): \[K\], \[Pa\], \[mol/s\], \[J/mol\], \[W\], \[m\], \[-\].
//! Mole fractions, relative volatilities, and stage counts are dimensionless
//! and carried as bare `f64` — a stage count wrapped in a `uom` type would be
//! dishonest precision. Reflux ratios use [`uom::si::f64::Ratio`].
//!
//! Upstream works in kJ (duty) and g/mol (molar mass); this port folds those
//! scalings away and works in W and kg/mol throughout, so no `/1000` factors
//! appear.
//!
//! # Excluded upstream behavior (with `ShortcutColumn.vb` line ranges)
//!
//! - WinForms editor plumbing: the `EditingForm_ShortcutColumn` field (`:40`)
//!   and `DisplayEditForm` / `UpdateEditForm` / `CloseEditForm` (`:864-917`).
//! - Base-class constructors and flowsheet-object identity (`:65-75`),
//!   equipment-type and dimension lists (`:77-98`).
//! - XML/JSON serialization and cloning: `CloneXML` / `CloneJSON` /
//!   `LoadData` / `SaveData` (`:100-120`).
//! - `Inspector` trace paragraphs (`:124-137` and scattered `IObj?` calls).
//! - Flowsheet stream-graph wiring: inlet/outlet stream fetching and
//!   validation (`:142-153`), writing compositions/flows back into the
//!   product `MaterialStream`s (`:416-444`, `:453-457`, `:476-480`), the
//!   energy-stream duty updates (`:521-535`), and the temporary
//!   `MaterialStream` created for sizing (`:546-549`). Feeds arrive here as a
//!   plain [`ShortcutFeed`]; products leave as a plain
//!   [`ShortcutColumnResult`].
//! - `DeCalculate` (`:582-607`), the property-grid reflection layer
//!   `GetPropertyValue` / `GetProperties` / `SetPropertyValue` /
//!   `GetPropertyUnit` (`:609-862`), icons/localisation (`:892-908`),
//!   `MobileCompatible` (`:919-923`), the text report `GetReport`
//!   (`:925-977`), and `GetPropertyDescription` (`:979-999`).
//! - The `lnk`/`dnk`/`hnk` volatility classification (`:207-218`): only the
//!   lighter-than-light-key list affects any result (the first distillate
//!   estimate, `:220-228`); the `dnk`/`hnk` lists are dead stores upstream and
//!   are not carried.
//! - `EstimatedDiameter`/`EstimatedHeight` as mutable object state — they are
//!   returned in the result instead.
//!
//! # Deliberate deviations from upstream (all documented at the site)
//!
//! - **Typed errors** ([`ShortcutColumnError`]) instead of bare .NET
//!   exceptions with UI strings.
//! - **An iteration cap (1000) on the distillate-rate loop** — upstream's
//!   `GoTo restart` loop (`:230-272`) is unbounded and can spin forever on a
//!   non-convergent distribution; the cap turns that into
//!   [`ShortcutColumnError::DistributionNotConverged`].
//! - **Input validation up front** (keys distinct and present in the feed,
//!   specs in `(0, 1)`, `alpha_lk > alpha_hk`) where upstream would emit NaN.
//! - **K-value sanitisation** is delegated to
//!   [`ColumnThermo::k_values`] (non-finite K replaced by the ideal
//!   `P_sat/P`), which subsumes upstream's `Double.MaxValue` substitution
//!   (`:203-205`).
//! - **The single-composition K-value call** `DW_CalcKvalue(z, T, P)` (`:201`)
//!   is approximated by the bridge's two-composition form with `x = y = z`.
//!   For the `Ideal` package the two are identical; for a cubic package this
//!   is a first-order approximation, consistent with the shortcut method's own
//!   accuracy.
//! - **Mixture liquid density for sizing** uses the Rackett route of
//!   `PropertyPackage.vb:7674` (Kay's-rule pseudo-criticals into
//!   [`liquid_density_rackett`], with the mixture vapour pressure approximated
//!   by the Raoult sum of Wilson vapour pressures) rather than dispatching on
//!   a per-package density mode.
//! - The result reports which [`UnderwoodMode`] was taken — upstream computes
//!   the same branch but does not expose it.

use ndarray::Array2;
use uom::si::catalytic_activity::katal;
use uom::si::f64::{Length, MolarMass, Power, Pressure, Ratio, ThermodynamicTemperature};
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::molar_mass::kilogram_per_mole;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

use crate::columns::linalg::lu_solve;
use crate::columns::model::{MolarFlowRate, StagePressure, StageTemperature};
use crate::columns::thermo_bridge::ColumnThermo;
use crate::petroleum::aux_props::liquid_density_rackett;
use crate::thermo::property_package::PropertyPackageModel;
use crate::thermo::saturation::dew_temperature_with;
use crate::thermo::saturation::SaturationOptions;
use crate::thermo::Component;

/// Heat duty \[W\] of the shortcut column's condenser or reboiler.
///
/// Sign convention (upstream's, `ShortcutColumn.vb:499-510`): for a normal
/// column **both duties come out positive** — the condenser duty is the heat
/// *removed* overhead and the reboiler duty the heat *added* at the bottom,
/// related by the overall balance
/// `F*HF + Qb = D*HD + B*HB + Qc`. This differs from the rigorous sibling's
/// [`crate::columns::model::StageHeatDuty`] (positive *into* the stage), hence
/// the separate alias.
pub type ShortcutHeatDuty = Power;

/// Condenser type of the shortcut column.
///
/// Ports upstream's own two-variant `CondenserType` enum
/// (`ShortcutColumn.vb:42-45`) — the shortcut method has no full-reflux mode,
/// so this is deliberately narrower than the rigorous sibling's
/// [`crate::columns::model::CondenserType`]. Closed set, enum dispatch, per
/// the workspace design rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShortcutCondenserType {
    /// All overhead vapour condensed; the distillate leaves as a saturated
    /// liquid at its bubble point (upstream `TotalCond = 0`, the default,
    /// `:63`).
    #[default]
    TotalCondenser,
    /// Overhead vapour partially condensed; the distillate leaves as a
    /// saturated vapour at its dew point (upstream `PartialCond = 1`).
    PartialCondenser,
}

/// Which branch of Underwood's method produced the minimum reflux.
///
/// Upstream computes this distinction (`mode2`, `ShortcutColumn.vb:288-303`)
/// but does not expose it; reporting it is a deliberate addition of this port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnderwoodMode {
    /// No non-key distributes between the products: one Underwood root
    /// between `alpha_hk` and `alpha_lk` (`:303-321`).
    SingleRoot,
    /// One or more non-keys distribute (the `Dr` criterion of `:294` lies in
    /// `(0, 1)`): one root per volatility gap and a linear solve for `Rmin`
    /// (`:323-403`).
    DistributedKeys,
}

/// Errors raised by the shortcut column.
///
/// Upstream throws bare `Exception`s with UI strings; this port returns a
/// typed error so a caller can distinguish a bad specification from a
/// numerical failure without string matching.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ShortcutColumnError {
    /// The column or feed description is malformed (bad key indices, spec
    /// fractions outside `(0, 1)`, composition length mismatch, non-positive
    /// flow, keys absent from the feed, or light key not more volatile than
    /// the heavy key). Upstream has no such guard and would emit NaN.
    #[error("invalid shortcut column configuration: {detail}")]
    InvalidConfiguration {
        /// What was wrong.
        detail: String,
    },
    /// The non-key distribution loop produced a NaN or zero distillate rate.
    /// Ports upstream's `ArgumentOutOfRangeException` ("Invalid value for
    /// Distillate Rate", `ShortcutColumn.vb:270`).
    #[error("invalid distillate rate: {value} mol/s")]
    InvalidDistillateRate {
        /// The offending rate \[mol/s\].
        value: f64,
    },
    /// The distillate-rate iteration (`:230-272`) failed to reach its `1e-4`
    /// relative tolerance. **Port deviation:** upstream's `GoTo restart` loop
    /// is unbounded; this port caps it at 1000 passes.
    #[error("non-key distribution did not converge in {iterations} iterations")]
    DistributionNotConverged {
        /// Iterations taken before giving up.
        iterations: usize,
    },
    /// The internal flows implied by the reflux ratio are negative
    /// (`L_strip < 0` or `V_strip < 0`). Ports upstream's "Invalid Reflux
    /// Ratio" exception (`:280-282`).
    #[error(
        "invalid reflux ratio: stripping liquid = {stripping_liquid} mol/s, \
         stripping vapor = {stripping_vapor} mol/s"
    )]
    InvalidRefluxRatio {
        /// Stripping-section liquid flow `L + q F` \[mol/s\].
        stripping_liquid: f64,
        /// Stripping-section vapour flow `L + q F - B` \[mol/s\].
        stripping_vapor: f64,
    },
    /// The requested reflux ratio is below the Underwood minimum. Ports
    /// upstream's "Defined Reflux Ratio ({0}) lower than calculated minimum
    /// ({1})" exception (`:405-407`).
    #[error("reflux ratio {specified} is below the Underwood minimum {minimum}")]
    RefluxBelowMinimum {
        /// The reflux ratio that was asked for \[-\].
        specified: f64,
        /// The computed minimum reflux ratio \[-\].
        minimum: f64,
    },
    /// The distributed-key Underwood linear system (`:369-392`) is singular.
    #[error("the distributed-key Underwood linear system is singular")]
    UnderwoodSingular,
    /// A bubble- or dew-point calculation needed for the product temperatures
    /// and duties failed. Wraps the underlying saturation error text.
    #[error("saturation calculation failed for {what}: {detail}")]
    SaturationFailed {
        /// Which flash failed (e.g. "distillate bubble point").
        what: &'static str,
        /// Underlying error text.
        detail: String,
    },
}

/// The single feed stream of the shortcut column.
///
/// Ports the slice of the upstream `MaterialStream` the algorithm actually
/// reads (`ShortcutColumn.vb:165-196`): molar flow, overall composition,
/// temperature, pressure, and the liquid molar fraction.
#[derive(Debug, Clone, PartialEq)]
pub struct ShortcutFeed {
    /// Total feed molar flow `F` \[mol/s\]. Must be > 0.
    pub molar_flow: MolarFlowRate,
    /// Overall feed mole fractions `z_i` \[-\], one per component, normalised
    /// internally.
    pub composition: Vec<f64>,
    /// Feed temperature \[K\]. Relative volatilities, the sizing densities,
    /// and the feed enthalpy are evaluated here. Must be > 0.
    pub temperature: StageTemperature,
    /// Feed pressure \[Pa\]. Must be > 0.
    pub pressure: StagePressure,
    /// Liquid molar fraction `q` of the feed \[-\], in `[0, 1]` (ports
    /// `feed.Phases(3).Properties.molarfraction`, `:166`). A value of exactly
    /// 0 or 1 triggers the enthalpy-based recomputation of the thermal
    /// quality (`:170-187`), which can legitimately push the *effective* `q`
    /// above 1 (subcooled liquid) or below 0 (superheated vapour).
    pub liquid_fraction: Ratio,
}

impl ShortcutFeed {
    /// Build a feed, validating flow, temperature and pressure are positive
    /// and the composition is non-empty with a positive sum.
    ///
    /// `liquid_fraction` is clamped to `[0, 1]` (it is a phase fraction; the
    /// enthalpy-recomputed thermal quality may later leave that range, but the
    /// *input* cannot).
    pub fn new(
        molar_flow: MolarFlowRate,
        composition: Vec<f64>,
        temperature: StageTemperature,
        pressure: StagePressure,
        liquid_fraction: Ratio,
    ) -> Result<Self, ShortcutColumnError> {
        let f = molar_flow.get::<katal>();
        let t = temperature.get::<kelvin>();
        let p = pressure.get::<pascal>();
        if !(f.is_finite() && f > 0.0) {
            return Err(ShortcutColumnError::InvalidConfiguration {
                detail: format!("feed molar flow must be finite and > 0 mol/s, got {f}"),
            });
        }
        if !(t.is_finite() && t > 0.0) || !(p.is_finite() && p > 0.0) {
            return Err(ShortcutColumnError::InvalidConfiguration {
                detail: format!("feed T and P must be finite and > 0, got T = {t} K, P = {p} Pa"),
            });
        }
        let s: f64 = composition.iter().sum();
        if composition.is_empty() || !s.is_finite() || s <= 0.0 {
            return Err(ShortcutColumnError::InvalidConfiguration {
                detail: "feed composition must be non-empty with a positive sum".into(),
            });
        }
        let q = liquid_fraction.get::<ratio>().clamp(0.0, 1.0);
        Ok(Self {
            molar_flow,
            composition,
            temperature,
            pressure,
            liquid_fraction: Ratio::new::<ratio>(q),
        })
    }
}

/// The shortcut (Fenske-Underwood-Gilliland) distillation column.
///
/// Mirrors the specification fields of upstream's `ShortcutColumn` class
/// (`ShortcutColumn.vb:47-63`): the two key components, the key impurity
/// specs, the operating reflux ratio, the end pressures, the condenser type,
/// and the tray spacing used for sizing. Solve with
/// [`ShortcutColumn::solve`].
///
/// The two "key" components define the split: everything more volatile than
/// the light key should leave overhead, everything less volatile than the
/// heavy key should leave in the bottoms, and the two specs pin how much of
/// each key is allowed to leak the wrong way.
#[derive(Debug, Clone, PartialEq)]
pub struct ShortcutColumn {
    /// Pure-component constants, shared with the thermo bridge.
    pub components: Vec<Component>,
    /// K-value / enthalpy model (enum dispatch, no `dyn`).
    pub package: PropertyPackageModel,
    /// Index of the **light key** component in `components` (upstream
    /// `m_lightkey`, matched by name at `:193`).
    pub light_key: usize,
    /// Index of the **heavy key** component (upstream `m_heavykey`).
    pub heavy_key: usize,
    /// Specified mole fraction of the **light key in the bottoms** \[-\],
    /// in `(0, 1)` (upstream `m_lightkeymolarfrac`, default 0.01).
    pub light_key_bottoms_fraction: f64,
    /// Specified mole fraction of the **heavy key in the distillate** \[-\],
    /// in `(0, 1)` (upstream `m_heavykeymolarfrac`, default 0.01).
    pub heavy_key_distillate_fraction: f64,
    /// Operating reflux ratio `R = L/D` \[-\] (upstream `m_refluxratio`,
    /// default 1.5). Must exceed the Underwood minimum or
    /// [`ShortcutColumnError::RefluxBelowMinimum`] is returned.
    pub reflux_ratio: Ratio,
    /// Condenser pressure \[Pa\] (upstream `m_condenserpressure`, default
    /// 101 325 Pa).
    pub condenser_pressure: StagePressure,
    /// Reboiler pressure \[Pa\] (upstream `m_boilerpressure`, default
    /// 101 325 Pa).
    pub reboiler_pressure: StagePressure,
    /// Total or partial condenser (upstream `condtype`, default total).
    pub condenser_type: ShortcutCondenserType,
    /// Stage (tray) spacing \[m\] used by the height and diameter estimates
    /// (upstream `StageHeight`, default 0.5 m). The Fair-style capacity
    /// quadratic was fitted for spacings roughly in `0.15-0.9 m`.
    pub stage_height: Length,
}

impl ShortcutColumn {
    /// Build a shortcut column over `components` with the given key pair,
    /// using upstream's defaults for everything else (specs 0.01/0.01, reflux
    /// ratio 1.5, both pressures 101 325 Pa, total condenser, 0.5 m stage
    /// height — `ShortcutColumn.vb:51-63`).
    ///
    /// # Errors
    ///
    /// [`ShortcutColumnError::InvalidConfiguration`] if fewer than two
    /// components are given, a key index is out of range, or the keys
    /// coincide.
    pub fn new(
        components: Vec<Component>,
        package: PropertyPackageModel,
        light_key: usize,
        heavy_key: usize,
    ) -> Result<Self, ShortcutColumnError> {
        let n = components.len();
        if n < 2 {
            return Err(ShortcutColumnError::InvalidConfiguration {
                detail: format!("a shortcut column needs at least 2 components, got {n}"),
            });
        }
        if light_key >= n || heavy_key >= n {
            return Err(ShortcutColumnError::InvalidConfiguration {
                detail: format!(
                    "key indices ({light_key}, {heavy_key}) out of range for {n} components"
                ),
            });
        }
        if light_key == heavy_key {
            return Err(ShortcutColumnError::InvalidConfiguration {
                detail: "light and heavy key must be different components".into(),
            });
        }
        Ok(Self {
            components,
            package,
            light_key,
            heavy_key,
            light_key_bottoms_fraction: 0.01,
            heavy_key_distillate_fraction: 0.01,
            reflux_ratio: Ratio::new::<ratio>(1.5),
            condenser_pressure: StagePressure::new::<pascal>(101_325.0),
            reboiler_pressure: StagePressure::new::<pascal>(101_325.0),
            condenser_type: ShortcutCondenserType::default(),
            stage_height: Length::new::<meter>(0.5),
        })
    }

    /// Set the key impurity specs: light key in bottoms, heavy key in
    /// distillate (both mole fractions in `(0, 1)`, checked at solve time).
    #[must_use]
    pub fn with_key_specs(
        mut self,
        light_key_in_bottoms: f64,
        heavy_key_in_distillate: f64,
    ) -> Self {
        self.light_key_bottoms_fraction = light_key_in_bottoms;
        self.heavy_key_distillate_fraction = heavy_key_in_distillate;
        self
    }

    /// Set the operating reflux ratio `R = L/D` \[-\].
    #[must_use]
    pub fn with_reflux_ratio(mut self, r: Ratio) -> Self {
        self.reflux_ratio = r;
        self
    }

    /// Set the condenser and reboiler pressures \[Pa\].
    #[must_use]
    pub fn with_pressures(mut self, condenser: StagePressure, reboiler: StagePressure) -> Self {
        self.condenser_pressure = condenser;
        self.reboiler_pressure = reboiler;
        self
    }

    /// Set the condenser type (total or partial).
    #[must_use]
    pub fn with_condenser_type(mut self, ct: ShortcutCondenserType) -> Self {
        self.condenser_type = ct;
        self
    }

    /// Set the stage (tray) spacing \[m\] used for sizing.
    #[must_use]
    pub fn with_stage_height(mut self, h: Length) -> Self {
        self.stage_height = h;
        self
    }

    /// Run the Fenske-Underwood-Gilliland calculation.
    ///
    /// Ports the algorithmic core of `ShortcutColumn.Calculate`
    /// (`ShortcutColumn.vb:122-565`); see the module doc for the correlation
    /// list and the deliberate deviations.
    ///
    /// # Errors
    ///
    /// Every variant of [`ShortcutColumnError`]; in particular
    /// [`ShortcutColumnError::RefluxBelowMinimum`] when the requested reflux
    /// ratio is infeasible.
    pub fn solve(&self, feed: &ShortcutFeed) -> Result<ShortcutColumnResult, ShortcutColumnError> {
        let n = self.components.len();
        let lk = self.light_key;
        let hk = self.heavy_key;
        if feed.composition.len() != n {
            return Err(ShortcutColumnError::InvalidConfiguration {
                detail: format!(
                    "feed composition has {} entries for {n} components",
                    feed.composition.len()
                ),
            });
        }
        for (name, v) in [
            ("light key in bottoms", self.light_key_bottoms_fraction),
            (
                "heavy key in distillate",
                self.heavy_key_distillate_fraction,
            ),
        ] {
            if !(v.is_finite() && v > 0.0 && v < 1.0) {
                return Err(ShortcutColumnError::InvalidConfiguration {
                    detail: format!("spec `{name}` must lie in (0, 1), got {v}"),
                });
            }
        }
        let r_spec = self.reflux_ratio.get::<ratio>();
        if !(r_spec.is_finite() && r_spec > 0.0) {
            return Err(ShortcutColumnError::InvalidConfiguration {
                detail: format!("reflux ratio must be finite and > 0, got {r_spec}"),
            });
        }

        // Normalised feed composition; keys must actually be present.
        let zsum: f64 = feed.composition.iter().sum();
        let z: Vec<f64> = feed.composition.iter().map(|v| v / zsum).collect();
        if z[lk] <= 0.0 || z[hk] <= 0.0 {
            return Err(ShortcutColumnError::InvalidConfiguration {
                detail: format!(
                    "both keys must be present in the feed (z_lk = {}, z_hk = {})",
                    z[lk], z[hk]
                ),
            });
        }

        let f_flow = feed.molar_flow.get::<katal>(); // F [mol/s]
        let t_feed = feed.temperature.get::<kelvin>(); // T [K]
        let p_feed = feed.pressure.get::<pascal>(); // P [Pa]
        let thermo = ColumnThermo::new(self.components.clone(), self.package);

        // ---- Feed enthalpy and thermal quality q (ShortcutColumn.vb:165-187).
        //
        // The feed enthalpy H [J/mol] is evaluated at the supplied phase split
        // (upstream reads the stream's own enthalpy, `:173`); it doubles as HF
        // in the duty balance (`:451`).
        let q_input = feed.liquid_fraction.get::<ratio>();
        let h_feed = thermo.feed_molar_enthalpy(&z, t_feed, p_feed, 1.0 - q_input);
        let mut q = q_input;
        if q == 0.0 || q == 1.0 {
            // Saturated-or-beyond feed: recompute the thermal quality from
            // enthalpies, q = 1 + (Hbub - H)/(Hdew - Hbub) (`:170-187`).
            // Upstream wraps this in Try/Catch and keeps the old q on failure;
            // this port does the same via if-let.
            if let (Ok((t_bub, _)), Ok(t_dew)) = (
                thermo.bubble_temperature(&z, p_feed, t_feed, 0),
                dew_temperature_of(&self.components, self.package, &z, p_feed),
            ) {
                let h_bub = thermo.liquid_molar_enthalpy(&z, t_bub, p_feed);
                let h_dew = thermo.vapor_molar_enthalpy(&z, t_dew, p_feed);
                let q_new = 1.0 + (h_bub - h_feed) / (h_dew - h_bub);
                if q_new.is_finite() {
                    q = q_new;
                }
            }
        }

        // ---- Relative volatilities at feed conditions (`:198-218`).
        //
        // Upstream calls the one-composition form DW_CalcKvalue(z, T, P); the
        // bridge's (x, y) form is fed x = y = z (see module doc, deviations).
        // The bridge also subsumes the Double.MaxValue guard of `:203-205`.
        let k = thermo.k_values(&z, &z, t_feed, p_feed);
        let alpha: Vec<f64> = k.iter().map(|ki| ki / k[hk]).collect();
        if !(alpha[lk] > alpha[hk]) {
            return Err(ShortcutColumnError::InvalidConfiguration {
                detail: format!(
                    "light key must be more volatile than heavy key at feed conditions \
                     (alpha_lk = {}, alpha_hk = {})",
                    alpha[lk], alpha[hk]
                ),
            });
        }

        // First distillate estimate: light key plus everything lighter
        // (`:220-228`; the lnk list built at `:210-218`).
        let mut d_flow = f_flow * z[lk];
        for i in 0..n {
            if k[i] > k[lk] {
                d_flow += f_flow * z[i];
            }
        }

        // ---- Fenske + Hengstebeck-Geddes distribution, iterated on D
        // (`:230-272`, upstream's `restart:` GoTo loop).
        let xd_hk_spec = self.heavy_key_distillate_fraction;
        let xb_lk_spec = self.light_key_bottoms_fraction;
        let mut xd = vec![0.0; n];
        let mut xb = vec![0.0; n];
        let mut b_flow;
        let mut n_min;
        let mut iterations = 0usize;
        loop {
            iterations += 1;
            if iterations > 1000 {
                // Port deviation: upstream's GoTo loop is unbounded.
                return Err(ShortcutColumnError::DistributionNotConverged { iterations });
            }
            b_flow = f_flow - d_flow;

            xd[hk] = xd_hk_spec;
            xb[lk] = xb_lk_spec;
            xb[hk] = (f_flow * z[hk] - d_flow * xd[hk]) / (f_flow - d_flow);
            xd[lk] = (f_flow * z[lk] - (f_flow - d_flow) * xb[lk]) / d_flow;

            // Fenske minimum stages (`:238-243`). alpha_hk == 1 by
            // construction, but the ratio is kept as upstream writes it.
            let s = (xd[lk] / xd[hk]) * (xb[hk] / xb[lk]);
            n_min = s.ln() / (alpha[lk] / alpha[hk]).ln();

            // Non-key distribution constant C and per-component split
            // (`:245-259`).
            let c = (alpha[lk].log10() * (xd[hk] / xb[hk]).log10()
                - alpha[hk].log10() * (xd[lk] / xb[lk]).log10())
                / (alpha[lk].log10() - alpha[hk].log10());
            for i in 0..n {
                if i != lk && i != hk {
                    let cte = 10.0_f64.powf(n_min * alpha[i].log10() + c);
                    xb[i] = f_flow * z[i] / (b_flow + d_flow * cte);
                    xd[i] = xb[i] * cte;
                }
            }

            // Distillate-rate update (`:261-272`): D_new = D_old * sum(xd).
            let d_ant = d_flow;
            d_flow = (0..n).filter(|&i| z[i] != 0.0).map(|i| d_ant * xd[i]).sum();
            if d_flow.is_nan() || d_flow == 0.0 {
                return Err(ShortcutColumnError::InvalidDistillateRate { value: d_flow });
            }
            if ((d_flow - d_ant) / d_flow).abs() < 1.0e-4 {
                // Note (faithful quirk): B, xd, xb keep the values computed
                // with the *pre-update* D of this pass, while D itself is the
                // updated value — exactly as upstream falls through at `:272`.
                break;
            }
        }

        // ---- Internal flows (`:274-282`).
        let r = r_spec;
        let l_rect = r * d_flow; // L  [mol/s]
        let l_strip = l_rect + q * f_flow; // L_ [mol/s]
        let v_strip = l_strip - b_flow; // V_ [mol/s]
        let v_rect = d_flow + l_rect; // V  [mol/s]
        if l_strip < 0.0 || v_strip < 0.0 {
            return Err(ShortcutColumnError::InvalidRefluxRatio {
                stripping_liquid: l_strip,
                stripping_vapor: v_strip,
            });
        }

        // ---- Underwood minimum reflux (`:284-403`).
        //
        // Distribution criterion Dr (Shiras-type, `:294`): a non-key with
        // 0 < Dr < 1 distributes between the products.
        let mut indexes: Vec<usize> = Vec::new();
        for i in 0..n {
            let dr = (alpha[i] - 1.0) / (alpha[lk] - 1.0) * d_flow * xd[lk] / (f_flow * z[lk])
                + (alpha[lk] - alpha[i]) / (alpha[lk] - 1.0) * d_flow * xd[hk] / (f_flow * z[hk]);
            if dr > 0.0 && dr < 1.0 && z[i] != 0.0 && i != lk && i != hk {
                indexes.push(i);
            }
        }
        let underwood_mode = if indexes.is_empty() {
            UnderwoodMode::SingleRoot
        } else {
            UnderwoodMode::DistributedKeys
        };

        let r_min = match underwood_mode {
            UnderwoodMode::SingleRoot => {
                // One root between alpha_hk and alpha_lk (`:305-321`).
                let theta = brent_minimize(alpha[hk] * 1.01, alpha[lk], 1.0e-7, |x| {
                    underwood_objective(x, &alpha, &z, q)
                });
                let mut sum = 0.0;
                for i in 0..n {
                    if z[i] != 0.0 {
                        sum += alpha[i] * xd[i] / (alpha[i] - theta);
                    }
                }
                sum - 1.0
            }
            UnderwoodMode::DistributedKeys => {
                // One root per volatility gap (`:325-367`), then a dense
                // linear solve for [L/D_min, xd of distributed comps]
                // (`:369-392`).
                let count = indexes.len();
                let mut theta = vec![0.0; count + 1];
                for (i, th) in theta.iter_mut().enumerate() {
                    let (a_lo, b_hi) = if i == 0 {
                        if alpha[lk] < alpha[indexes[0]] {
                            (alpha[lk] * 1.01, alpha[indexes[0]] * 0.99)
                        } else {
                            (alpha[indexes[0]] * 1.01, alpha[lk] * 0.99)
                        }
                    } else if i == count {
                        if alpha[indexes[i - 1]] < alpha[hk] {
                            (alpha[indexes[i - 1]] * 1.01, alpha[hk] * 0.99)
                        } else {
                            (alpha[hk] * 1.01, alpha[indexes[i - 1]] * 0.99)
                        }
                    } else {
                        // Upstream's If and Else bodies are identical for the
                        // middle gaps (`:354-364`) — ported as written.
                        (alpha[indexes[i - 1]] * 1.01, alpha[indexes[i]] * 0.99)
                    };
                    *th = brent_minimize(a_lo, b_hi, 1.0e-7, |x| {
                        underwood_objective(x, &alpha, &z, q)
                    });
                }

                let m = count + 1;
                let mut ma = Array2::<f64>::zeros((m, m));
                let mut mb = vec![0.0; m];
                for i in 0..m {
                    mb[i] = -1.0;
                    ma[[i, 0]] = 1.0; // coefficient of L/D_min
                    let mut j2 = 0usize;
                    for j in 0..n {
                        if !indexes.contains(&j) {
                            mb[i] += alpha[j] * xd[j] / (alpha[j] - theta[i]);
                        } else {
                            ma[[i, j2 + 1]] = -alpha[j] / (alpha[j] - theta[i]);
                            j2 += 1;
                        }
                    }
                }
                let x = lu_solve(ma, &mb).ok_or(ShortcutColumnError::UnderwoodSingular)?;
                x[0]
            }
        };

        if r_min > r {
            return Err(ShortcutColumnError::RefluxBelowMinimum {
                specified: r,
                minimum: r_min,
            });
        }

        // ---- Gilliland actual stages, Eduljee analytic fit (`:409-414`).
        let xx = (r - r_min) / (r + 1.0);
        let yy = 0.75 * (1.0 - xx.powf(0.5668));
        let n_actual = (yy + n_min) / (1.0 - yy);

        // ---- Product temperatures and duties (`:446-510`).
        let p_cond = self.condenser_pressure.get::<pascal>();
        let p_reb = self.reboiler_pressure.get::<pascal>();
        let xd_n = normalized_positive(&xd);
        let xb_n = normalized_positive(&xb);

        // Distillate temperature: dew point for a partial condenser (VF = 1
        // flash, `:460-462`), bubble point for a total condenser (VF = 0,
        // `:463-466`).
        let (t_dist, h_dist) = match self.condenser_type {
            ShortcutCondenserType::PartialCondenser => {
                let td = dew_temperature_of(&self.components, self.package, &xd_n, p_cond)
                    .map_err(|detail| ShortcutColumnError::SaturationFailed {
                        what: "distillate dew point",
                        detail,
                    })?;
                (td, thermo.vapor_molar_enthalpy(&xd_n, td, p_cond))
            }
            ShortcutCondenserType::TotalCondenser => {
                let (td, _) = thermo
                    .bubble_temperature(&xd_n, p_cond, t_feed, 0)
                    .map_err(|e| ShortcutColumnError::SaturationFailed {
                        what: "distillate bubble point",
                        detail: e.to_string(),
                    })?;
                (td, thermo.liquid_molar_enthalpy(&xd_n, td, p_cond))
            }
        };

        // Bottoms temperature: bubble point at reboiler pressure (`:483-492`).
        let (t_bot, _) = thermo
            .bubble_temperature(&xb_n, p_reb, t_feed, 0)
            .map_err(|e| ShortcutColumnError::SaturationFailed {
                what: "bottoms bubble point",
                detail: e.to_string(),
            })?;
        let h_bot = thermo.liquid_molar_enthalpy(&xb_n, t_bot, p_reb);

        // Condenser duty (`:494-508`). H in J/mol and flows in mol/s, so the
        // duties are in W (upstream's /1000 to kW is folded away).
        let q_cond = match self.condenser_type {
            ShortcutCondenserType::PartialCondenser => {
                // HL = liquid enthalpy at the distillate bubble point.
                let (t_bub_d, _) = thermo
                    .bubble_temperature(&xd_n, p_cond, t_dist, 0)
                    .map_err(|e| ShortcutColumnError::SaturationFailed {
                        what: "distillate bubble point (partial-condenser duty)",
                        detail: e.to_string(),
                    })?;
                let h_l = thermo.liquid_molar_enthalpy(&xd_n, t_bub_d, p_cond);
                -(h_l - h_dist) * l_rect
            }
            ShortcutCondenserType::TotalCondenser => {
                // HD0 = vapour enthalpy at the distillate dew point,
                // HL = liquid enthalpy at its bubble point (= h_dist here).
                let t_dew_d = dew_temperature_of(&self.components, self.package, &xd_n, p_cond)
                    .map_err(|detail| ShortcutColumnError::SaturationFailed {
                        what: "distillate dew point (total-condenser duty)",
                        detail,
                    })?;
                let h_d0 = thermo.vapor_molar_enthalpy(&xd_n, t_dew_d, p_cond);
                let h_l = thermo.liquid_molar_enthalpy(&xd_n, t_dist, p_cond);
                -(h_l - h_d0) * (l_rect + d_flow)
            }
        };

        // Reboiler duty from the overall energy balance (`:510`).
        let q_reb = d_flow * h_dist + b_flow * h_bot + q_cond - f_flow * h_feed;

        // ---- Optimum feed stage, Fenske-ratio method (`:512-519`).
        //
        // (z_lk/z_hk)(xb_hk/xb_lk) is the feed-to-bottoms key separation, so
        // Ns is the stripping-section stage count: the feed sits Ns
        // theoretical stages above the reboiler.
        let ss = z[lk] / z[hk] * xb[hk] / xb[lk];
        let n_min_strip = ss.ln() / alpha[lk].ln();
        let feed_stage = n_min_strip * n_actual / n_min;

        // ---- Sizing (`:537-561`).
        let lt = self.stage_height.get::<meter>();
        let height = (n_actual + 2.0) * lt;

        let max_v = v_rect.max(v_strip); // [mol/s], vapour composition ~ xd
        let m_vap = thermo.mixture_molar_mass(&xd_n); // [kg/mol]
        let max_vw = max_v * m_vap; // [kg/s]

        // Ideal-gas vapour density at feed conditions (`:556`): upstream's
        // MW/(8.314*T/P*1000) with MW in g/mol is M*P/(R*T) in kg/m3.
        let rho_v = m_vap * p_feed / (8.314 * t_feed);
        // Rackett liquid density of the bottoms-like liquid at feed T
        // (`:557`, AUX_LIQDENS): Kay's-rule pseudo-criticals
        // (PropertyPackage.vb:7674), mixture vapour pressure from the Raoult
        // sum of Wilson vapour pressures (see module doc, deviations).
        let rho_l = {
            let tcm: f64 = self
                .components
                .iter()
                .zip(xb_n.iter())
                .map(|(c, &x)| x * c.critical_temperature)
                .sum();
            let pcm: f64 = self
                .components
                .iter()
                .zip(xb_n.iter())
                .map(|(c, &x)| x * c.critical_pressure)
                .sum();
            let wm: f64 = self
                .components
                .iter()
                .zip(xb_n.iter())
                .map(|(c, &x)| x * c.acentric_factor)
                .sum();
            let mm = thermo.mixture_molar_mass(&xb_n);
            let pvap: f64 = (0..n)
                .map(|i| xb_n[i] * thermo.vapor_pressure(i, t_feed))
                .sum();
            liquid_density_rackett(
                ThermodynamicTemperature::new::<kelvin>(t_feed),
                ThermodynamicTemperature::new::<kelvin>(tcm),
                Pressure::new::<pascal>(pcm),
                Ratio::new::<ratio>(wm),
                MolarMass::new::<kilogram_per_mole>(mm),
                None,
                Some(Pressure::new::<pascal>(p_feed)),
                Some(Pressure::new::<pascal>(pvap)),
            )
            .get::<kilogram_per_cubic_meter>()
        };
        // Souders-Brown-type maximum vapour velocity, Fair-style capacity
        // quadratic in the tray spacing (`:558`).
        let uv = (-0.17 * lt * lt + 0.27 * lt - 0.047) * ((rho_l - rho_v) / rho_v).sqrt();
        let diameter = (4.0 * max_vw / (std::f64::consts::PI * rho_v * uv)).sqrt();

        Ok(ShortcutColumnResult {
            minimum_stages: n_min,
            actual_stages: n_actual,
            minimum_reflux_ratio: Ratio::new::<ratio>(r_min),
            underwood_mode,
            optimal_feed_stage: feed_stage,
            feed_quality: Ratio::new::<ratio>(q),
            distillate_flow: MolarFlowRate::new::<katal>(d_flow),
            bottoms_flow: MolarFlowRate::new::<katal>(b_flow),
            distillate_composition: xd,
            bottoms_composition: xb,
            rectifying_liquid: MolarFlowRate::new::<katal>(l_rect),
            rectifying_vapor: MolarFlowRate::new::<katal>(v_rect),
            stripping_liquid: MolarFlowRate::new::<katal>(l_strip),
            stripping_vapor: MolarFlowRate::new::<katal>(v_strip),
            distillate_temperature: StageTemperature::new::<kelvin>(t_dist),
            bottoms_temperature: StageTemperature::new::<kelvin>(t_bot),
            condenser_duty: ShortcutHeatDuty::new::<watt>(q_cond),
            reboiler_duty: ShortcutHeatDuty::new::<watt>(q_reb),
            estimated_height: Length::new::<meter>(height),
            estimated_diameter: Length::new::<meter>(diameter),
        })
    }
}

/// Everything the shortcut calculation produces.
///
/// Mirrors upstream's result fields `m_Nmin`, `m_N`, `m_Rmin`, `ofs`, `L`,
/// `V`, `L_`, `V_`, `m_Qc`, `m_Qb`, the product streams' compositions and
/// flows, and `EstimatedHeight` / `EstimatedDiameter`
/// (`ShortcutColumn.vb:61`, `:416-561`).
#[derive(Debug, Clone, PartialEq)]
pub struct ShortcutColumnResult {
    /// Fenske minimum number of theoretical stages at total reflux `Nmin`
    /// \[-\] (fractional — the correlation does not round).
    pub minimum_stages: f64,
    /// Gilliland (Eduljee fit) actual number of theoretical stages `N` \[-\]
    /// at the specified reflux ratio (fractional; round up for a real
    /// column). Always exceeds `minimum_stages` for any finite reflux.
    pub actual_stages: f64,
    /// Underwood minimum reflux ratio `Rmin = (L/D)_min` \[-\].
    pub minimum_reflux_ratio: Ratio,
    /// Which Underwood branch produced `minimum_reflux_ratio`.
    pub underwood_mode: UnderwoodMode,
    /// Optimum feed stage by the Fenske-ratio method \[-\]: the number of
    /// theoretical stages in the **stripping section**, i.e. the feed sits
    /// this many stages above the reboiler. Upstream reports this same number
    /// as "Optimal Feed Stage" (`ofs`, `ShortcutColumn.vb:519`). Fractional.
    pub optimal_feed_stage: f64,
    /// The feed thermal quality `q` actually used \[-\] (moles of
    /// stripping-section liquid added per mole of feed): 1 = saturated
    /// liquid, 0 = saturated vapour, `> 1` subcooled, `< 0` superheated.
    pub feed_quality: Ratio,
    /// Distillate molar flow `D` \[mol/s\].
    pub distillate_flow: MolarFlowRate,
    /// Bottoms molar flow `B` \[mol/s\].
    pub bottoms_flow: MolarFlowRate,
    /// Distillate mole fractions \[-\], one per component.
    pub distillate_composition: Vec<f64>,
    /// Bottoms mole fractions \[-\], one per component.
    pub bottoms_composition: Vec<f64>,
    /// Rectifying-section liquid flow `L = R D` \[mol/s\].
    pub rectifying_liquid: MolarFlowRate,
    /// Rectifying-section vapour flow `V = L + D` \[mol/s\].
    pub rectifying_vapor: MolarFlowRate,
    /// Stripping-section liquid flow `L' = L + q F` \[mol/s\].
    pub stripping_liquid: MolarFlowRate,
    /// Stripping-section vapour flow `V' = L' - B` \[mol/s\].
    pub stripping_vapor: MolarFlowRate,
    /// Distillate temperature \[K\]: bubble point (total condenser) or dew
    /// point (partial condenser) at the condenser pressure.
    pub distillate_temperature: StageTemperature,
    /// Bottoms temperature \[K\]: bubble point at the reboiler pressure.
    pub bottoms_temperature: StageTemperature,
    /// Condenser duty \[W\], positive = heat removed (see
    /// [`ShortcutHeatDuty`]).
    pub condenser_duty: ShortcutHeatDuty,
    /// Reboiler duty \[W\], positive = heat added, closed from the overall
    /// energy balance `Qb = D HD + B HB + Qc - F HF`.
    pub reboiler_duty: ShortcutHeatDuty,
    /// Estimated column height `(N + 2) * stage_height` \[m\].
    pub estimated_height: Length,
    /// Estimated column diameter \[m\] from the Souders-Brown-type maximum
    /// vapour velocity. A rough sizing number, not a hydraulic design.
    pub estimated_diameter: Length,
}

/// Underwood's squared residual `(Sum_j alpha_j z_j/(alpha_j - x) - 1 + q)^2`.
///
/// Ports `rminfunc` (`ShortcutColumn.vb:567-580`) exactly, including the
/// `z_j != 0` skip. Minimised (not root-found) by [`brent_minimize`], exactly
/// as upstream drives it through `BrentOpt.BrentMinimize.brentoptimize2`.
fn underwood_objective(x: f64, alpha: &[f64], z: &[f64], q: f64) -> f64 {
    let mut value = 0.0;
    for j in 0..z.len() {
        if z[j] != 0.0 {
            value += alpha[j] * z[j] / (alpha[j] - x);
        }
    }
    (value - 1.0 + q).powi(2)
}

/// Brent scalar minimisation on `[a, b]` to absolute tolerance `epsilon`,
/// returning the abscissa of the minimum.
///
/// Faithful port of DWSIM's `BrentOpt.BrentMinimize.brentoptimize2`
/// (`DWSIM.Math/BrentMinimize.vb:176-295`): golden-section constant 0.381966,
/// 100-iteration cap, the same parabolic-step acceptance test, the same
/// NaN-bail (`:262`), and the same stopping rule — including upstream's
/// literal `v = 2` clause in the bookkeeping (`:287`), which looks like an
/// inherited typo but is reproduced as written.
fn brent_minimize(a: f64, b: f64, epsilon: f64, mut func: impl FnMut(f64) -> f64) -> f64 {
    const CGOLD: f64 = 0.381_966;
    let bx = 0.5 * (a + b);
    let mut ia = a.min(b);
    let mut ib = a.max(b);
    let mut v = bx;
    let mut w = v;
    let mut x = v;
    let mut d = 0.0_f64;
    let mut e = 0.0_f64;
    let mut fx = func(x);
    let mut fv = fx;
    let mut fw = fx;
    for _iter in 1..=100 {
        let xm = 0.5 * (ia + ib);
        if (x - xm).abs() <= epsilon * 2.0 - 0.5 * (ib - ia) {
            break;
        }
        if e.abs() > epsilon {
            let r = (x - w) * (fx - fv);
            let mut q = (x - v) * (fx - fw);
            let mut p = (x - v) * q - (x - w) * r;
            q = 2.0 * (q - r);
            if q > 0.0 {
                p = -p;
            }
            q = q.abs();
            let etemp = e;
            e = d;
            if !(p.abs() >= (0.5 * q * etemp).abs() || p <= q * (ia - x) || p >= q * (ib - x)) {
                d = p / q;
                let u = x + d;
                if u - ia < epsilon * 2.0 || ib - u < epsilon * 2.0 {
                    d = mysign(epsilon, xm - x);
                }
            } else {
                e = if x >= xm { ia - x } else { ib - x };
                d = CGOLD * e;
            }
        } else {
            e = if x >= xm { ia - x } else { ib - x };
            d = CGOLD * e;
        }
        let u = if d.abs() >= epsilon {
            x + d
        } else {
            x + mysign(epsilon, d)
        };
        let fu = func(u);
        if fu.is_nan() {
            break;
        }
        if fu <= fx {
            if u >= x {
                ia = x;
            } else {
                ib = x;
            }
            v = w;
            fv = fw;
            w = x;
            fw = fx;
            x = u;
            fx = fu;
        } else {
            if u < x {
                ia = u;
            } else {
                ib = u;
            }
            if fu <= fw || w == x {
                v = w;
                fv = fw;
                w = u;
                fw = fu;
            } else if fu <= fv || v == x || v == 2.0 {
                // `v == 2.0` is upstream's own literal (`BrentMinimize.vb:287`),
                // reproduced as written.
                v = u;
                fv = fu;
            }
        }
    }
    x
}

/// Upstream's `mysign` helper (`BrentMinimize.vb:298-307`): `|a|` with the
/// sign of `b` (`b <= 0` gives the negative sign, matching the VB original).
fn mysign(a: f64, b: f64) -> f64 {
    if b > 0.0 {
        a.abs()
    } else {
        -a.abs()
    }
}

/// Dew-point temperature \[K\] of composition `z` at pressure `p` \[Pa\],
/// bridging [`crate::thermo::saturation::dew_temperature_with`] with the
/// package's K-value closure. Errors are returned as their display text (the
/// caller wraps them in [`ShortcutColumnError::SaturationFailed`]).
fn dew_temperature_of(
    components: &[Component],
    package: PropertyPackageModel,
    z: &[f64],
    p: f64,
) -> Result<f64, String> {
    dew_temperature_with(
        components,
        z,
        p,
        |x, y, t, pp| package.k_values(components, x, y, t, pp),
        SaturationOptions::default(),
    )
    .map(|st| st.temperature)
    .map_err(|e| e.to_string())
}

/// Clamp negatives/NaNs to zero and normalise to sum 1 (uniform if the sum is
/// not positive). Guards the product-composition flashes against the NaN
/// entries upstream zeroes at `ShortcutColumn.vb:423`/`:436`.
fn normalized_positive(x: &[f64]) -> Vec<f64> {
    let cleaned: Vec<f64> = x
        .iter()
        .map(|v| if v.is_finite() && *v > 0.0 { *v } else { 0.0 })
        .collect();
    let s: f64 = cleaned.iter().sum();
    if s > 0.0 {
        cleaned.iter().map(|v| v / s).collect()
    } else {
        vec![1.0 / x.len().max(1) as f64; x.len()]
    }
}

#[cfg(test)]
mod tests {
    //! # V&V — the shortcut column against hand arithmetic and the rigorous
    //! # MESH solver
    //!
    //! ## Methodology
    //!
    //! Two tiers of checks, per the workspace V&V rule:
    //!
    //! 1. **Hand arithmetic on a binary.** For a binary column the ported
    //!    algorithm collapses to closed forms that can be verified by hand
    //!    (recorded in each test): the distillate iteration converges in one
    //!    pass to `D` from the overall balance, Fenske is
    //!    `ln[(xd_lk/xd_hk)(xb_hk/xb_lk)]/ln(alpha)`, and for an equimolar
    //!    saturated-liquid feed the Underwood root is exactly
    //!    `theta = 2 alpha/(alpha + 1)`.
    //! 2. **Cross-check against the rigorous Wang-Henke MESH solver** in
    //!    [`crate::columns::bubble_point`] — an independent method sharing
    //!    only the thermo bridge. The shortcut design (stages, feed stage,
    //!    reflux, bottoms rate) is fed into the rigorous column and the
    //!    achieved separation compared against the shortcut's prediction.
    //!
    //! Test system: benzene/toluene (constants from Poling, Prausnitz &
    //! O'Connell, *The Properties of Gases and Liquids*, 5th ed., 2001,
    //! Appendix A — public literature), `Ideal` package, 101.325 kPa,
    //! matching the rigorous module's own test column.
    //!
    //! ## Results
    //!
    //! Recorded per test below; all numbers measured **2026-08-11** with
    //! `cargo test --release -p outram-park-fork-dwsim-libs` on this port.
    //!
    //! ## Honest scope
    //!
    //! Verification, not validation: nothing here is compared against
    //! experimental distillation data or DWSIM's own output. The K-values are
    //! Wilson/`k_ij = 0` estimates. AI-assisted draft, no human V&V.

    use super::*;
    use crate::columns::bubble_point::WangHenkeSolver;
    use crate::columns::initial_estimates::RigorousColumn;
    use crate::columns::model::{ColumnSpec, Stage};
    use crate::columns::thermo_bridge::tests::{benzene, toluene};
    use uom::si::f64::MolarEnergy;
    use uom::si::molar_energy::joule_per_mole;
    use uom::si::power::watt as watt_unit;
    use uom::si::thermodynamic_temperature::kelvin as kelvin_unit;

    const P_ATM: f64 = 101_325.0;

    /// Equimolar benzene/toluene feed, saturated liquid at its bubble point
    /// (so the enthalpy-recomputed `q` stays exactly 1 — `H = Hbub` by
    /// construction), 1 mol/s at 101.325 kPa.
    pub(super) fn saturated_liquid_feed(comps: &[Component]) -> ShortcutFeed {
        let thermo = ColumnThermo::new(comps.to_vec(), PropertyPackageModel::Ideal);
        let z = vec![0.5, 0.5];
        let (t_bub, _) = thermo
            .bubble_temperature(&z, P_ATM, 365.0, 0)
            .expect("feed bubble point must converge");
        ShortcutFeed::new(
            MolarFlowRate::new::<katal>(1.0),
            z,
            StageTemperature::new::<kelvin>(t_bub),
            StagePressure::new::<pascal>(P_ATM),
            Ratio::new::<ratio>(1.0),
        )
        .expect("feed must validate")
    }

    /// Symmetric benzene/toluene column: 5 % heavy key overhead, 5 % light
    /// key in the bottoms, R = 2, total condenser.
    pub(super) fn benzene_toluene_column() -> ShortcutColumn {
        ShortcutColumn::new(
            vec![benzene(), toluene()],
            PropertyPackageModel::Ideal,
            0,
            1,
        )
        .expect("column must validate")
        .with_key_specs(0.05, 0.05)
        .with_reflux_ratio(Ratio::new::<ratio>(2.0))
    }

    /// **Methodology.** Binary benzene/toluene, equimolar saturated-liquid
    /// feed, symmetric 5 %/5 % key specs, `R = 2`. Every closed form is
    /// checked against independent hand arithmetic computed in the test from
    /// the same measured `alpha`:
    ///
    /// - **D**: for a binary the non-key loop is empty, and the fixed point of
    ///   `D_new = D_old (xd_lk + xd_hk)` with the balance lines gives
    ///   `0.95 D = 0.5 - 0.05 (1 - D)`, i.e. `D = 0.5` exactly; the first
    ///   estimate `D = F z_lk = 0.5` already satisfies it, so the loop
    ///   converges on pass 1 with `xd = (0.95, 0.05)`, `xb = (0.05, 0.95)`.
    /// - **Fenske**: `Nmin = ln[(0.95/0.05)(0.95/0.05)]/ln(alpha)
    ///   = ln(361)/ln(alpha)`.
    /// - **Underwood** (single root, equimolar, `q = 1`):
    ///   `alpha z/( alpha - theta) + z/(1 - theta) = 0` gives
    ///   `theta = 2 alpha/(alpha + 1)` exactly, then
    ///   `Rmin = 0.95 alpha/(alpha - theta) + 0.05/(1 - theta) - 1`.
    /// - **Gilliland/Eduljee**: `X = (2 - Rmin)/3`,
    ///   `Y = 0.75 (1 - X^0.5668)`, `N = (Y + Nmin)/(1 - Y)`.
    /// - **Feed stage**: symmetric specs make the stripping Fenske ratio
    ///   exactly half (`ln 19 = ln 361 / 2`), so `Ns = N/2`.
    ///
    /// **Results (2026-08-11, `cargo test --release`):** feed bubble point
    /// `T = 364.2819 K`, `K = [1.41634, 0.58366]`, measured
    /// `alpha = 2.426637`; `D = B = 0.500000000` mol/s (loop converged on
    /// pass 1); `Nmin = 6.642794` vs hand `ln(361)/ln(alpha) = 6.642794`
    /// (diff `< 1e-9`); Underwood root `theta_hand = 1.416337` gives
    /// `Rmin = 1.16170902` vs the closed form `1.16170892` (diff `1.012e-7`,
    /// Brent tolerance 1e-7 on theta); `N = 11.445717` vs hand `11.445717`
    /// (diff `5.05e-7`); feed stage `Ns = 5.722859 = N/2` (diff `8.9e-16`);
    /// mode = `SingleRoot`. Internal flows `L = 1.0`, `V = 1.5`, `L' = 2.0`,
    /// `V' = 1.5 mol/s`. Duties/temperatures: `TD = 353.10 K` (distillate
    /// bubble point), `TB = 380.65 K`, `Qc = 46 258 W` removed,
    /// `Qb = 46 887 W` added.
    ///
    /// **Interpretation.** Every ported correlation reproduces its closed
    /// form on the binary to solver tolerance; the energy-balance closure and
    /// temperature ordering are physically sane.
    #[test]
    fn binary_fenske_underwood_gilliland_matches_hand_arithmetic() {
        let comps = vec![benzene(), toluene()];
        let column = benzene_toluene_column();
        let feed = saturated_liquid_feed(&comps);
        let out = column.solve(&feed).expect("binary case must solve");

        // Reproduce alpha independently from the same bridge call.
        let thermo = ColumnThermo::new(comps.clone(), PropertyPackageModel::Ideal);
        let z = [0.5, 0.5];
        let t = feed.temperature.get::<kelvin>();
        let k = thermo.k_values(&z, &z, t, P_ATM);
        let alpha = k[0] / k[1];
        assert!(alpha > 1.0, "benzene must be the more volatile: {alpha}");

        // D = B = 0.5 exactly (hand arithmetic above).
        let d = out.distillate_flow.get::<katal>();
        let b = out.bottoms_flow.get::<katal>();
        assert!((d - 0.5).abs() < 1e-12, "D = {d}");
        assert!((b - 0.5).abs() < 1e-12, "B = {b}");
        assert!((out.distillate_composition[0] - 0.95).abs() < 1e-12);
        assert!((out.bottoms_composition[0] - 0.05).abs() < 1e-12);

        // Fenske closed form.
        let nmin_hand = 361.0_f64.ln() / alpha.ln();
        assert!(
            (out.minimum_stages - nmin_hand).abs() < 1e-9,
            "Nmin = {} vs hand {nmin_hand}",
            out.minimum_stages
        );

        // Underwood closed form (equimolar, q = 1).
        let theta_hand = 2.0 * alpha / (alpha + 1.0);
        let rmin_hand = 0.95 * alpha / (alpha - theta_hand) + 0.05 / (1.0 - theta_hand) - 1.0;
        let rmin = out.minimum_reflux_ratio.get::<ratio>();
        assert_eq!(out.underwood_mode, UnderwoodMode::SingleRoot);
        assert!(
            (rmin - rmin_hand).abs() < 1e-5,
            "Rmin = {rmin} vs hand {rmin_hand}"
        );

        // Gilliland/Eduljee closed form from the hand Rmin.
        let xx = (2.0 - rmin_hand) / 3.0;
        let yy = 0.75 * (1.0 - xx.powf(0.5668));
        let n_hand = (yy + nmin_hand) / (1.0 - yy);
        assert!(
            (out.actual_stages - n_hand).abs() < 1e-4,
            "N = {} vs hand {n_hand}",
            out.actual_stages
        );

        // Symmetric specs put the feed exactly mid-column.
        assert!(
            (out.optimal_feed_stage - out.actual_stages / 2.0).abs() < 1e-6,
            "Ns = {} vs N/2 = {}",
            out.optimal_feed_stage,
            out.actual_stages / 2.0
        );

        // Physical sanity of the temperature/duty block.
        let td = out.distillate_temperature.get::<kelvin>();
        let tb = out.bottoms_temperature.get::<kelvin>();
        assert!(td < tb, "TD = {td} K must be below TB = {tb} K");
        assert!((300.0..450.0).contains(&td) && (300.0..450.0).contains(&tb));
        let qc = out.condenser_duty.get::<watt>();
        let qb = out.reboiler_duty.get::<watt>();
        assert!(qc > 0.0, "condenser duty (heat removed) = {qc} W");
        assert!(qb > 0.0, "reboiler duty (heat added) = {qb} W");
        assert!((out.feed_quality.get::<ratio>() - 1.0).abs() < 1e-9);
    }

    /// **Methodology.** The stage count must (a) always exceed the Fenske
    /// minimum at finite reflux, (b) **strictly decrease** as the reflux
    /// ratio increases, (c) approach `Nmin` as `R -> infinity`
    /// (`X -> 1, Y -> 0`), and (d) approach the Eduljee fit's **finite**
    /// `R -> Rmin` limit `N -> 4 Nmin + 3` (`X = 0, Y = 0.75`) — the ported
    /// correlation does *not* diverge at minimum reflux, and this test
    /// documents that honestly rather than asserting a divergence the code
    /// does not have.
    ///
    /// **Results (2026-08-11, `cargo test --release`):** with
    /// `Rmin = 1.161709`, `Nmin = 6.642794`: `N(1.001 Rmin) = 28.3377`,
    /// then `N = 11.4457` (R = 2), `9.4309` (R = 3), `7.9042` (R = 6),
    /// `7.0011` (R = 20), `6.7133` (R = 100) — strictly decreasing, all above
    /// `Nmin`, with `N(100)/Nmin = 1.0106`. At `R = (1 + 1e-6) Rmin`,
    /// `N = 29.5456` vs the Eduljee limit `4 Nmin + 3 = 29.5712` (relative
    /// difference `8.7e-4`; the limit is approached slowly because
    /// `X^0.5668` has infinite slope at `X = 0`, which is why the limit is
    /// probed at `1e-6` rather than `1e-3` above `Rmin`).
    #[test]
    fn gilliland_stage_count_decreases_with_reflux_toward_the_fenske_minimum() {
        let comps = vec![benzene(), toluene()];
        let feed = saturated_liquid_feed(&comps);
        let base = benzene_toluene_column();

        let rmin = base
            .solve(&feed)
            .expect("base case must solve")
            .minimum_reflux_ratio
            .get::<ratio>();

        let ratios = [1.001 * rmin, 2.0, 3.0, 6.0, 20.0, 100.0];
        let mut prev_n = f64::INFINITY;
        let mut nmin = 0.0;
        for r in ratios {
            let out = base
                .clone()
                .with_reflux_ratio(Ratio::new::<ratio>(r))
                .solve(&feed)
                .unwrap_or_else(|e| panic!("R = {r} must solve: {e}"));
            nmin = out.minimum_stages;
            assert!(
                out.actual_stages > out.minimum_stages,
                "R = {r}: N = {} must exceed Nmin = {}",
                out.actual_stages,
                out.minimum_stages
            );
            assert!(
                out.actual_stages < prev_n,
                "R = {r}: N = {} must be below the previous {prev_n}",
                out.actual_stages
            );
            prev_n = out.actual_stages;
        }
        // R -> infinity limit: N -> Nmin.
        assert!(
            prev_n < 1.05 * nmin,
            "N(R = 100) = {prev_n} should be within 5 % of Nmin = {nmin}"
        );
        // R -> Rmin limit is FINITE for the Eduljee fit: N -> 4 Nmin + 3.
        // Probed very close to Rmin because X^0.5668 leaves X = 0 with
        // infinite slope, so the limit is approached slowly.
        let near_min = base
            .with_reflux_ratio(Ratio::new::<ratio>((1.0 + 1e-6) * rmin))
            .solve(&feed)
            .expect("R just above Rmin must solve");
        let limit = 4.0 * near_min.minimum_stages + 3.0;
        assert!(
            (near_min.actual_stages - limit).abs() / limit < 0.02,
            "N(R -> Rmin) = {} vs Eduljee limit {limit}",
            near_min.actual_stages
        );
    }

    /// **Methodology.** A reflux ratio below the Underwood minimum must be
    /// rejected with [`ShortcutColumnError::RefluxBelowMinimum`] carrying the
    /// computed minimum — porting the guard at `ShortcutColumn.vb:405-407`.
    ///
    /// **Results (2026-08-11):** requesting `R = 1.0` against
    /// `Rmin = 1.161709` returns `RefluxBelowMinimum { specified: 1.0,
    /// minimum: 1.161709 }`; the reported minimum matches the feasible run's
    /// `Rmin` to `< 1e-9`.
    #[test]
    fn reflux_below_the_underwood_minimum_is_rejected() {
        let comps = vec![benzene(), toluene()];
        let feed = saturated_liquid_feed(&comps);
        let rmin_ref = benzene_toluene_column()
            .solve(&feed)
            .expect("base case must solve")
            .minimum_reflux_ratio
            .get::<ratio>();

        let err = benzene_toluene_column()
            .with_reflux_ratio(Ratio::new::<ratio>(1.0))
            .solve(&feed)
            .expect_err("R = 1.0 < Rmin must be rejected");
        match err {
            ShortcutColumnError::RefluxBelowMinimum { specified, minimum } => {
                assert_eq!(specified, 1.0);
                assert!(
                    (minimum - rmin_ref).abs() < 1e-9,
                    "reported minimum {minimum} vs reference {rmin_ref}"
                );
            }
            other => panic!("wrong error variant: {other:?}"),
        }
    }

    /// **Methodology.** THE independent check: feed the shortcut design into
    /// the rigorous Wang-Henke MESH solver and compare the achieved
    /// separation. Shortcut specs are chosen to mirror the rigorous module's
    /// own base case (its `R = 2` solve measured
    /// `xd_benzene = 0.8828`, `xb_benzene = 0.1173` on 2026-08-11): here
    /// `xd_hk = 0.1172`, `xb_lk = 0.1173`, `R = 2`. The shortcut's stage
    /// count is rounded up, a total condenser stage added, the feed placed
    /// `round(Ns)` stages above the reboiler, and the rigorous column solved
    /// with the same `R` and the shortcut's bottoms rate. Pass criteria: the
    /// rigorous distillate benzene fraction within **0.05** of the shortcut's
    /// prediction, and both solvers' D and B in agreement to `1e-6` mol/s
    /// (the bottoms rate is imposed). No tuning of either model.
    ///
    /// **Results (2026-08-11, `cargo test --release`):** shortcut design:
    /// `Nmin = 4.5544`, `Rmin = 0.8388`, `N = 7.0740`, feed `3.5362` stages
    /// above the reboiler, `D = 0.499938 mol/s`, `B = 0.500050 mol/s`,
    /// predicted `xd_benzene = 0.88278`, `xb_benzene = 0.11730`. Rigorous
    /// column built from it: 9 stages (8 theoretical, rounded up from 7.07,
    /// plus a total condenser), feed on stage index 4, `R = 2`,
    /// `B = 0.500050 mol/s`. Wang-Henke converged in 17 iterations and
    /// achieved `xd_benzene = 0.904301` vs the shortcut's 0.882777 —
    /// **difference 2.15e-2**, within the 0.05 gate — and
    /// `xb_benzene = 0.095776` vs predicted 0.117300 (also `2.15e-2`).
    ///
    /// **Interpretation.** The two methods agree to about 0.02 mole fraction,
    /// and the *direction* of the disagreement is the expected one: rounding
    /// 7.07 theoretical stages up to 8 and adding the condenser stage gives
    /// the rigorous column slightly more separating power than the shortcut
    /// asked for, so it over-delivers purity on both ends symmetrically.
    /// Neither model was tuned toward the other. This is a *consistency*
    /// result between two models sharing one thermo bridge, not validation
    /// against measured data.
    #[test]
    fn shortcut_design_reproduces_requested_separation_in_rigorous_solver() {
        let comps = vec![benzene(), toluene()];
        let feed = saturated_liquid_feed(&comps);

        // Specs mirroring the rigorous base case's achieved separation.
        let column = ShortcutColumn::new(comps.clone(), PropertyPackageModel::Ideal, 0, 1)
            .unwrap()
            .with_key_specs(0.1173, 0.1172)
            .with_reflux_ratio(Ratio::new::<ratio>(2.0));
        let sc = column.solve(&feed).expect("shortcut design must solve");

        // Build the rigorous column from the shortcut design.
        let n_theoretical = sc.actual_stages.ceil() as usize; // incl. reboiler
        let n_stages = n_theoretical + 1; // + total condenser
        let ns = sc.optimal_feed_stage.round() as usize; // stages above reboiler
        let feed_stage = (n_stages - 1).saturating_sub(ns).clamp(1, n_stages - 2);

        let thermo = ColumnThermo::new(comps.clone(), PropertyPackageModel::Ideal);
        let feed_z = [0.5, 0.5];
        let t_feed = feed.temperature.get::<kelvin>();
        let h_feed = thermo.feed_molar_enthalpy(&feed_z, t_feed, P_ATM, 0.0);

        let p = StagePressure::new::<pascal>(P_ATM);
        let mut stages: Vec<Stage> = (0..n_stages)
            .map(|i| {
                let t = StageTemperature::new::<kelvin>(
                    355.0 + 25.0 * i as f64 / (n_stages - 1) as f64,
                );
                Stage::new(format!("stage {i}"), p, t, 2)
            })
            .collect();
        stages[feed_stage] = stages[feed_stage].clone().with_feed(
            MolarFlowRate::new::<katal>(1.0),
            feed_z.to_vec(),
            MolarEnergy::new::<joule_per_mole>(h_feed),
        );

        let rigorous = RigorousColumn::distillation(
            comps,
            PropertyPackageModel::Ideal,
            stages,
            ColumnSpec::reflux_ratio(2.0),
            ColumnSpec::product_molar_flow(sc.bottoms_flow),
        )
        .with_distillate_estimate(sc.distillate_flow)
        .with_reflux_ratio_estimate(2.0);

        let input = rigorous.solver_input().expect("estimates must generate");
        let out = WangHenkeSolver::default()
            .solve_column(&input)
            .expect("rigorous solver must converge on the shortcut design");

        let xd_rigorous = out.liquid_compositions[0][0];
        let xd_shortcut = sc.distillate_composition[0];
        let xb_rigorous = out.liquid_compositions[n_stages - 1][0];
        let xb_shortcut = sc.bottoms_composition[0];
        assert!(
            (xd_rigorous - xd_shortcut).abs() < 0.05,
            "distillate benzene: rigorous {xd_rigorous} vs shortcut {xd_shortcut}"
        );
        assert!(
            (xb_rigorous - xb_shortcut).abs() < 0.05,
            "bottoms benzene: rigorous {xb_rigorous} vs shortcut {xb_shortcut}"
        );
        let b_rig = out.bottoms_molar_flow().get::<katal>();
        assert!(
            (b_rig - sc.bottoms_flow.get::<katal>()).abs() < 1e-6,
            "bottoms rate: rigorous {b_rig} vs shortcut {}",
            sc.bottoms_flow.get::<katal>()
        );
    }

    /// **Methodology.** Exercise the **distributed-key** Underwood branch
    /// (`ShortcutColumn.vb:323-403`): a ternary with a *synthetic* middle
    /// component ("intermediate surrogate", volatility between the keys —
    /// synthetic constants, no literature provenance claimed, chosen only to
    /// make the Shiras `Dr` criterion land in `(0, 1)`). Keys: benzene
    /// (light) and a toluene-like heavy; pass criteria: the solver reports
    /// [`UnderwoodMode::DistributedKeys`], a finite `Rmin > 0` below the
    /// operating `R`, stage counts finite with `N > Nmin`, and the
    /// distributed component's overhead + bottoms split conserving its feed
    /// moles to `1e-4` — the residual is bounded by the distillate loop's own
    /// (faithfully ported) `1e-4` relative convergence tolerance, so a
    /// tighter gate would test the tolerance, not the physics.
    ///
    /// **Results (2026-08-11, `cargo test --release`):** with the surrogate
    /// (`Tc = 575 K`, `Pc = 45 bar`, `omega = 0.24`, `Tb = 368 K`, benzene
    /// Cp) at `z = (0.4, 0.2, 0.4)`, specs 2 %/2 %, `R = 3`: feed bubble
    /// point `364.697 K`, `alpha = [2.4239, 1.6119, 1.0]`; the middle
    /// component's `Dr = 0.434129` (in `(0, 1)`, so mode 2 fired);
    /// `Rmin = 1.185307` from the 2-theta linear system; `Nmin = 8.277052`,
    /// `N = 11.722493`, `Ns = 5.912896`; `D = 0.516602`, `B = 0.483437`
    /// mol/s with `xd = [0.7556, 0.2244, 0.02]`,
    /// `xb = [0.02, 0.1739, 0.8060]`; distributed-component recovery
    /// `D xd + B xb = 0.20000883 mol/s` vs fed `0.2` (residual `8.83e-6`);
    /// per-component balance residuals `<= 2.97e-5 mol/s`.
    ///
    /// **Interpretation.** The mode-2 path (multi-root Brent + dense solve)
    /// runs end-to-end and is mass-consistent. A code-path exercise on a
    /// synthetic system, not physics validation.
    #[test]
    fn distributed_key_underwood_branch_is_exercised_and_mass_consistent() {
        // Synthetic middle-volatility surrogate; Cp coefficients copied from
        // benzene (they only affect enthalpies, not Rmin).
        let bz = benzene();
        let mid = Component::new(
            "intermediate surrogate (synthetic)",
            0.085,
            575.0,
            45.0e5,
            f64::NAN,
            0.24,
            368.0,
            [bz.cp_ig_a, bz.cp_ig_b, bz.cp_ig_c, bz.cp_ig_d, bz.cp_ig_e],
            f64::NAN,
        )
        .expect("surrogate constants are valid");
        let comps = vec![benzene(), mid, toluene()];

        let column = ShortcutColumn::new(
            comps.clone(),
            PropertyPackageModel::Ideal,
            0, // light key: benzene
            2, // heavy key: toluene
        )
        .unwrap()
        .with_key_specs(0.02, 0.02)
        .with_reflux_ratio(Ratio::new::<ratio>(3.0));

        let thermo = ColumnThermo::new(comps.clone(), PropertyPackageModel::Ideal);
        let z = vec![0.4, 0.2, 0.4];
        let (t_bub, _) = thermo
            .bubble_temperature(&z, P_ATM, 370.0, 0)
            .expect("ternary bubble point must converge");
        let feed = ShortcutFeed::new(
            MolarFlowRate::new::<katal>(1.0),
            z,
            StageTemperature::new::<kelvin>(t_bub),
            StagePressure::new::<pascal>(P_ATM),
            Ratio::new::<ratio>(1.0),
        )
        .unwrap();

        let out = column.solve(&feed).expect("ternary case must solve");
        assert_eq!(
            out.underwood_mode,
            UnderwoodMode::DistributedKeys,
            "the synthetic middle component must distribute"
        );
        let rmin = out.minimum_reflux_ratio.get::<ratio>();
        assert!(
            rmin.is_finite() && rmin > 0.0 && rmin < 3.0,
            "Rmin = {rmin}"
        );
        assert!(out.actual_stages > out.minimum_stages);
        assert!(out.actual_stages.is_finite() && out.minimum_stages > 0.0);

        // Middle-component mass conservation across the split.
        let d = out.distillate_flow.get::<katal>();
        let b = out.bottoms_flow.get::<katal>();
        // Gate 1e-4: the residual is bounded by the D-loop's faithfully
        // ported 1e-4 relative tolerance (measured residual 8.83e-6).
        let recovered = d * out.distillate_composition[1] + b * out.bottoms_composition[1];
        assert!(
            (recovered - 0.2).abs() < 1e-4,
            "distributed component: D xd + B xb = {recovered} mol/s vs fed 0.2"
        );
        // Overall mass balance of every component.
        for i in 0..3 {
            let res = d * out.distillate_composition[i] + b * out.bottoms_composition[i]
                - feed.composition[i];
            assert!(
                res.abs() < 1e-3,
                "component {i} balance residual = {res} mol/s"
            );
        }
    }

    /// **Methodology.** The enthalpy-based feed-quality recomputation
    /// (`ShortcutColumn.vb:170-187`): a feed supplied as `q = 1` but **10 K
    /// below** its bubble point must yield an effective `q > 1` (subcooled
    /// liquid condenses extra vapour, adding internal reflux), and the
    /// saturated feed must keep `q = 1` exactly (its enthalpy equals the
    /// bubble enthalpy by construction). Because subcooling supplies internal
    /// reflux for free, the **external** minimum reflux ratio `Rmin` must
    /// **decrease**; that direction is asserted, not assumed.
    ///
    /// **Results (2026-08-11, `cargo test --release`):** saturated feed
    /// `q = 1.000000000` (exact); subcooled feed (`T = Tbub - 10 K`)
    /// `q = 1.050863`; `Rmin` fell from `1.161709` (saturated) to `1.072571`
    /// (subcooled) — the asserted direction. Stage count at `R = 2` fell from
    /// `11.4457` to `10.7025`.
    #[test]
    fn subcooled_feed_raises_thermal_quality_above_one() {
        let comps = vec![benzene(), toluene()];
        let column = benzene_toluene_column();

        let saturated = saturated_liquid_feed(&comps);
        let out_sat = column.solve(&saturated).expect("saturated feed must solve");
        assert!(
            (out_sat.feed_quality.get::<ratio>() - 1.0).abs() < 1e-9,
            "saturated q = {}",
            out_sat.feed_quality.get::<ratio>()
        );

        let t_sub = saturated.temperature.get::<kelvin>() - 10.0;
        let subcooled = ShortcutFeed::new(
            saturated.molar_flow,
            saturated.composition.clone(),
            StageTemperature::new::<kelvin>(t_sub),
            saturated.pressure,
            Ratio::new::<ratio>(1.0),
        )
        .unwrap();
        let out_sub = column.solve(&subcooled).expect("subcooled feed must solve");
        let q_sub = out_sub.feed_quality.get::<ratio>();
        assert!(q_sub > 1.0, "subcooled q = {q_sub} must exceed 1");
        assert!(
            out_sub.minimum_reflux_ratio.get::<ratio>()
                < out_sat.minimum_reflux_ratio.get::<ratio>(),
            "subcooling must lower Rmin: subcooled {} vs saturated {}",
            out_sub.minimum_reflux_ratio.get::<ratio>(),
            out_sat.minimum_reflux_ratio.get::<ratio>()
        );
    }

    /// **Methodology.** The sizing block (`ShortcutColumn.vb:537-561`): the
    /// height must equal `(N + 2) * stage_height` **exactly** (it is that
    /// formula), and the Souders-Brown diameter must land in a plausible band
    /// (0.05-5 m) for a ~1 mol/s atmospheric benzene/toluene column — wide
    /// gates, because the test's job is to catch unit blunders (a factor of
    /// 1000 from a stray g/kg slip), not to certify hydraulics.
    ///
    /// **Results (2026-08-11, `cargo test --release`):** `N = 11.4457`,
    /// height `= 6.722859 m = (11.4457 + 2) x 0.5 m` (exact to 1e-12);
    /// vapour density `rho_v = 2.6368 kg/m3` (ideal gas, distillate-like
    /// vapour of 78.8 g/mol at the 364.28 K / 101.325 kPa feed conditions),
    /// liquid density `rho_l = 786.44 kg/m3` (Rackett, Kay's rule), maximum
    /// vapour velocity `uv = 0.784468 m/s`, diameter `= 0.269761 m`.
    #[test]
    fn sizing_estimates_are_dimensionally_sane() {
        let comps = vec![benzene(), toluene()];
        let feed = saturated_liquid_feed(&comps);
        let out = benzene_toluene_column()
            .solve(&feed)
            .expect("base case must solve");

        let height = out.estimated_height.get::<meter>();
        let expected_height = (out.actual_stages + 2.0) * 0.5;
        assert!(
            (height - expected_height).abs() < 1e-12,
            "height = {height} m vs (N + 2) lt = {expected_height} m"
        );

        let dia = out.estimated_diameter.get::<meter>();
        assert!(
            dia.is_finite() && (0.05..5.0).contains(&dia),
            "diameter = {dia} m is outside the plausible band"
        );
    }

    /// **Methodology.** The partial-condenser branch
    /// (`ShortcutColumn.vb:460-462`, `:495-499`): with a partial condenser
    /// the distillate leaves as a saturated **vapour** at its dew point, so
    /// (a) its temperature must **exceed** the total-condenser distillate
    /// temperature (dew point above bubble point for any non-pure mixture at
    /// one pressure), and (b) the condenser duty must be **smaller**, since
    /// only the reflux `L` is condensed rather than `L + D`. Both directions
    /// are asserted; the split itself (D, B, N, Rmin) must be identical, as
    /// the condenser type only enters the temperature/duty block.
    ///
    /// **Results (2026-08-11, `cargo test --release`):**
    /// `TD = 353.1028 K` (total, bubble point) vs `354.5267 K` (partial, dew
    /// point); `Qc = 46 257.93 W` (total, condensing `L + D = 1.5 mol/s`) vs
    /// `30 838.62 W` (partial, condensing `L = 1.0 mol/s`) — exactly the
    /// 2/3 ratio the flow split implies. `Qb = 46 886.62 W` in **both** cases,
    /// which the ported balance predicts analytically: expanding upstream's
    /// two `Qc` branches, `Qb = D*H_dew + L*(H_dew - H_bub) + B*HB - F*HF`
    /// either way, so the condenser type cancels out of the reboiler duty.
    #[test]
    fn partial_condenser_gives_dew_point_distillate_and_smaller_duty() {
        let comps = vec![benzene(), toluene()];
        let feed = saturated_liquid_feed(&comps);
        let total = benzene_toluene_column()
            .solve(&feed)
            .expect("total-condenser case must solve");
        let partial = benzene_toluene_column()
            .with_condenser_type(ShortcutCondenserType::PartialCondenser)
            .solve(&feed)
            .expect("partial-condenser case must solve");

        // Same split and stage counts: condenser type only affects T/Q.
        assert_eq!(partial.distillate_flow, total.distillate_flow);
        assert_eq!(partial.minimum_stages, total.minimum_stages);
        assert_eq!(partial.minimum_reflux_ratio, total.minimum_reflux_ratio);

        let td_total = total.distillate_temperature.get::<kelvin_unit>();
        let td_partial = partial.distillate_temperature.get::<kelvin_unit>();
        assert!(
            td_partial > td_total,
            "dew-point TD ({td_partial} K) must exceed bubble-point TD ({td_total} K)"
        );
        let qc_total = total.condenser_duty.get::<watt_unit>();
        let qc_partial = partial.condenser_duty.get::<watt_unit>();
        assert!(
            qc_partial > 0.0 && qc_partial < qc_total,
            "partial-condenser duty ({qc_partial} W) must be positive and below \
             the total-condenser duty ({qc_total} W)"
        );
    }

    /// **Methodology.** Configuration guards this port adds where upstream
    /// would emit NaN: coincident keys, out-of-range key index, composition
    /// length mismatch, spec fractions outside `(0, 1)`, and a key pair whose
    /// volatilities are inverted at feed conditions (toluene as "light" key
    /// over benzene). Each must return
    /// [`ShortcutColumnError::InvalidConfiguration`].
    ///
    /// **Results (2026-08-11):** all five malformed inputs are rejected with
    /// the expected variant.
    #[test]
    fn malformed_configurations_are_rejected() {
        let comps = vec![benzene(), toluene()];

        assert!(matches!(
            ShortcutColumn::new(comps.clone(), PropertyPackageModel::Ideal, 0, 0),
            Err(ShortcutColumnError::InvalidConfiguration { .. })
        ));
        assert!(matches!(
            ShortcutColumn::new(comps.clone(), PropertyPackageModel::Ideal, 0, 5),
            Err(ShortcutColumnError::InvalidConfiguration { .. })
        ));

        let feed = saturated_liquid_feed(&comps);

        // Composition length mismatch.
        let bad_feed = ShortcutFeed::new(
            MolarFlowRate::new::<katal>(1.0),
            vec![0.3, 0.3, 0.4],
            feed.temperature,
            feed.pressure,
            Ratio::new::<ratio>(1.0),
        )
        .unwrap();
        assert!(matches!(
            benzene_toluene_column().solve(&bad_feed),
            Err(ShortcutColumnError::InvalidConfiguration { .. })
        ));

        // Spec outside (0, 1).
        assert!(matches!(
            benzene_toluene_column()
                .with_key_specs(0.0, 0.05)
                .solve(&feed),
            Err(ShortcutColumnError::InvalidConfiguration { .. })
        ));

        // Inverted keys: toluene as "light" over benzene.
        let inverted = ShortcutColumn::new(comps, PropertyPackageModel::Ideal, 1, 0)
            .unwrap()
            .with_key_specs(0.05, 0.05);
        assert!(matches!(
            inverted.solve(&feed),
            Err(ShortcutColumnError::InvalidConfiguration { .. })
        ));
    }
}
