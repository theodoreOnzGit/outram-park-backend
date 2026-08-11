//! Thermodynamic bridge between the column solvers and the crate's
//! [`crate::thermo`] kernel.
//!
//! Maps the property-package calls DWSIM's rigorous-column solvers make onto
//! this crate's existing thermodynamics. Ported from the call sites in
//! `DWSIM.UnitOperations/UnitOperations/RigorousColumnSolvers/*.vb` (GPL-3.0),
//! upstream commit `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream
//! copyright: 2008-2022 Daniel Wagner O. de Medeiros et al.
//!
//! | DWSIM call | Where it appears | This module |
//! |---|---|---|
//! | `PP.DW_CalcKvalue(x, y, T, P)` | `BubblePoint.vb:1404`, `SumRates.vb:641`, `NewtonRaphson.vb:292` | [`ColumnThermo::k_values`] |
//! | `PP.DW_CalcEnthalpy(x, T, P, State.Liquid) * AUX_MMM(x) / 1000` | `BubblePoint.vb:1480`, `SumRates.vb:520`, `NewtonRaphson.vb:347` | [`ColumnThermo::liquid_molar_enthalpy`] |
//! | `PP.DW_CalcEnthalpy(y, T, P, State.Vapor) * AUX_MMM(y) / 1000` | `BubblePoint.vb:1482`, `SumRates.vb:531`, `NewtonRaphson.vb:340` | [`ColumnThermo::vapor_molar_enthalpy`] |
//! | `FlashAlg.Flash_PV(x, P, 0.0, T, ...)` (bubble point) | `BubblePoint.vb:1281`, `BubblePoint2.vb:1616` | [`ColumnThermo::bubble_temperature`] |
//! | `PP.AUX_PVAPi(j, T) / P` (K fallback) | `BubblePoint.vb:1133`, `SumRates.vb:217` | [`ColumnThermo::ideal_k_value`] |
//! | `PP.AUX_MMM(x)` | `BubblePoint.vb:980` | [`ColumnThermo::mixture_molar_mass`] |
//! | `PP.AUX_CONVERT_MOL_TO_MASS(x)` | `BubblePoint.vb:222` | [`ColumnThermo::mole_to_mass_fractions`] |
//! | `PP.AUX_CheckTrivial(K)` | `SumRates.vb:791` | [`ColumnThermo::is_trivial_solution`] |
//!
//! # Units
//!
//! Documented raw `f64` in SI throughout (crate `CLAUDE.md`): `T` \[K\], `P`
//! \[Pa\], mole fractions and K-values \[-\], molar enthalpy **\[J/mol\]**,
//! molar mass \[kg/mol\].
//!
//! DWSIM's `DW_CalcEnthalpy` returns a *mass-basis* enthalpy \[J/kg\] and the
//! solvers convert with `* AUX_MMM(x) / 1000` (mixture molar mass in g/mol
//! divided by 1000 gives kg/mol, so the product is J/mol). This port skips the
//! round trip and works in J/mol directly.
//!
//! # Enthalpy reference state
//!
//! Enthalpies are **sensible** ideal-gas enthalpies relative to
//! [`ColumnThermo::reference_temperature`] (default 298.15 K), plus a
//! phase-dependent residual — see [`ColumnEnthalpyModel`]. No enthalpy of
//! formation is added, exactly as [`crate::thermo::ideal_props`] documents.
//! Only enthalpy *differences* enter the MESH energy balance, so the reference
//! cancels — **provided the caller computes feed enthalpies on the same
//! reference**, which is what [`ColumnThermo::feed_molar_enthalpy`] is for.
//!
//! # Thermo capabilities implemented locally (candidates to move into `thermo`)
//!
//! Two routines below have no equivalent in [`crate::thermo`] today and are
//! implemented here, minimally but correctly:
//!
//! 1. [`ColumnThermo::enthalpy_of_vaporization`] — Vetere's `ΔH_vb` correlation
//!    at the normal boiling point plus the Watson exponent-0.375 extrapolation.
//!    This is a faithful port of DWSIM's own generic fallback route
//!    (`PropertyPackage.vb:6797` `AUX_HVAPi`, whose `HVap_A` is filled by
//!    `Hypotheticals.vb:906` `DHvb_Vetere` when the compound database has no
//!    fitted coefficients). It is needed because an ideal (Raoult) package has
//!    zero enthalpy departure, so without it `H_liquid == H_vapor`, the latent
//!    heat vanishes, and the energy balance degenerates.
//! 2. [`ColumnThermo::liquid_molar_enthalpy`] / [`ColumnThermo::vapor_molar_enthalpy`]
//!    — the ideal-gas + residual composition itself. `thermo` exposes the two
//!    halves ([`crate::thermo::ideal_props::mixture_ideal_gas_enthalpy`] and
//!    [`crate::thermo::cubic_eos::CubicEos::enthalpy_departure`]) but no
//!    package-level "give me the molar enthalpy of this phase" entry point.
//!
//! Both are flagged in the port hand-off as candidates to promote into
//! [`crate::thermo`]; nothing under `src/thermo/` was edited by this
//! workstream.
//!
//! # Excluded DWSIM behavior
//!
//! - **Parallel evaluation.** Every solver wraps its per-stage property calls
//!   in `Parallel.For` under a `Settings.EnableParallelProcessing` flag
//!   (`BubblePoint.vb:949-969`, `:1461-1484`; `SumRates.vb:455-494`;
//!   `NewtonRaphson.vb:230-330`). This port evaluates serially. The arithmetic
//!   is identical — upstream's parallel and serial branches compute the same
//!   numbers — so nothing physical is lost; the parallelism is a performance
//!   choice deferred to a later pass.
//! - **`ShouldUseKvalueMethod2` / `...Method3`** (`BubblePoint.vb:1399-1412`,
//!   `NewtonRaphson.vb:275-294`). These switch the K-value call to a
//!   *feed-composition* form `K(z, T, P)` (method 2) or a two-argument
//!   mole-number form (method 3) for property packages whose K-values need the
//!   overall composition (electrolytes, some activity models). The crate's
//!   [`crate::thermo::property_package::PropertyPackageModel`] has only the
//!   `(x, y, T, P)` form, so this port always takes upstream's `Else` branch —
//!   which is what every cubic/ideal package uses anyway.
//! - **`DW_CalcEnthalpyOfReaction`** (`SumRates.vb:486`,
//!   `NewtonRaphson.vb:351`) — reactive-distillation heat of reaction, guarded
//!   upstream by `pp.HasReactivePhase`. This port has no reactive column mode,
//!   so the term is identically zero. Reactive distillation is proposed as a
//!   follow-up.
//! - **The `"LL"` liquid-liquid K-value flavour** (`SumRates.vb:634`,
//!   `NewtonRaphson.vb:273`) used by DWSIM's liquid-liquid extractor mode.
//!   Out of scope here.

use crate::thermo::cubic_eos::{CubicEos, Phase};
use crate::thermo::flash::wilson_k_values;
use crate::thermo::ideal_props::mixture_ideal_gas_enthalpy;
use crate::thermo::property_package::PropertyPackageModel;
use crate::thermo::saturation::{bubble_temperature_with, SaturationOptions};
use crate::thermo::Component;

use crate::columns::model::ColumnError;

/// Universal gas constant \[J/(mol·K)\] — the value DWSIM uses in
/// `Hypotheticals.vb:908`.
const R_VETERE: f64 = 8.314;

/// How a phase's molar enthalpy is built from the ideal-gas value.
///
/// Enum dispatch, no `dyn`, per the workspace design rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnEnthalpyModel {
    /// `H = H_ideal_gas(T) + H_residual(T, P, phase)` from the cubic EOS.
    ///
    /// The physically consistent route for a cubic package: the vapour-liquid
    /// enthalpy difference *is* the EOS latent heat. Falls back to
    /// [`Self::IdealWithLatentHeat`] for a phase whose `Z`-root the EOS cannot
    /// find (which happens for a liquid root at very low reduced pressure), so
    /// the solver never sees a `NaN` enthalpy.
    EosDeparture,
    /// `H_vapor = H_ideal_gas(T)` and
    /// `H_liquid = H_ideal_gas(T) - Σ x_i ΔH_vap,i(T)`.
    ///
    /// The route an ideal (Raoult) package must take, because its enthalpy
    /// departure is identically zero. `ΔH_vap` comes from
    /// [`ColumnThermo::enthalpy_of_vaporization`].
    IdealWithLatentHeat,
}

/// The property-package façade the column solvers call.
///
/// Owns its component list by value (no lifetimes, per the workspace design
/// rules) and is cheap to clone.
///
/// # Valid ranges
///
/// All methods assume `T > 0` \[K\], `P > 0` \[Pa\], and mole-fraction slices
/// of length `components.len()`. Compositions need not be normalised on entry —
/// every method that is composition-sensitive normalises internally, matching
/// DWSIM's own behaviour of feeding un-normalised trial compositions into the
/// property package.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnThermo {
    /// Pure-component constants.
    components: Vec<Component>,
    /// The K-value / fugacity model.
    package: PropertyPackageModel,
    /// How phase enthalpies are assembled.
    enthalpy_model: ColumnEnthalpyModel,
    /// Reference temperature for the sensible ideal-gas enthalpy \[K\].
    reference_temperature: f64,
}

impl ColumnThermo {
    /// Build a bridge over `components` using `package`.
    ///
    /// The enthalpy model is chosen automatically:
    /// [`ColumnEnthalpyModel::IdealWithLatentHeat`] for
    /// [`PropertyPackageModel::Ideal`] (whose departure is identically zero),
    /// [`ColumnEnthalpyModel::EosDeparture`] for the cubic packages. The
    /// reference temperature is 298.15 K.
    ///
    /// # Panics
    ///
    /// Never. Component validity was already enforced by
    /// [`Component::new`](crate::thermo::Component::new).
    #[must_use]
    pub fn new(components: Vec<Component>, package: PropertyPackageModel) -> Self {
        let enthalpy_model = match package {
            PropertyPackageModel::Ideal => ColumnEnthalpyModel::IdealWithLatentHeat,
            _ => ColumnEnthalpyModel::EosDeparture,
        };
        Self {
            components,
            package,
            enthalpy_model,
            reference_temperature: 298.15,
        }
    }

    /// Override the enthalpy model (e.g. to force the latent-heat route on a
    /// cubic package for a low-pressure column where the liquid `Z`-root is
    /// poorly conditioned).
    #[must_use]
    pub fn with_enthalpy_model(mut self, model: ColumnEnthalpyModel) -> Self {
        self.enthalpy_model = model;
        self
    }

    /// Override the ideal-gas enthalpy reference temperature \[K\], > 0.
    #[must_use]
    pub fn with_reference_temperature(mut self, t_ref: f64) -> Self {
        self.reference_temperature = t_ref;
        self
    }

    /// The component list this bridge was built over.
    #[must_use]
    pub fn components(&self) -> &[Component] {
        &self.components
    }

    /// Number of components.
    #[must_use]
    pub fn n_components(&self) -> usize {
        self.components.len()
    }

    /// The underlying property-package model.
    #[must_use]
    pub fn package(&self) -> PropertyPackageModel {
        self.package
    }

    /// The ideal-gas enthalpy reference temperature \[K\].
    #[must_use]
    pub fn reference_temperature(&self) -> f64 {
        self.reference_temperature
    }

    /// Equilibrium K-values `K_i = y_i / x_i` \[-\] for a trial split.
    ///
    /// Ports `PP.DW_CalcKvalue(x, y, T, P)` (`BubblePoint.vb:1404`). `x`, `y`
    /// are mole fractions \[-\] (normalised internally), `t` \[K\], `p` \[Pa\].
    ///
    /// Any non-finite or non-positive K that the package returns is replaced by
    /// the ideal estimate `P_sat,i(T)/P` — this is upstream's own guard, applied
    /// at `BubblePoint.vb:1133`, `:1426`, and `SumRates.vb:648-655`.
    #[must_use]
    pub fn k_values(&self, x: &[f64], y: &[f64], t: f64, p: f64) -> Vec<f64> {
        let xn = normalized(x);
        let yn = normalized(y);
        let mut k = self.package.k_values(&self.components, &xn, &yn, t, p);
        for (i, ki) in k.iter_mut().enumerate() {
            if !ki.is_finite() || *ki <= 0.0 {
                *ki = self.ideal_k_value(i, t, p);
            }
        }
        k
    }

    /// Ideal (Raoult/Wilson) K-value for one component, `K_i = P_sat,i(T) / P`.
    ///
    /// Ports `PP.AUX_PVAPi(j, T) / P(i)` (`BubblePoint.vb:1133`). The saturation
    /// pressure is the Wilson estimate
    /// `P_sat,i = Pc_i · exp[5.373 (1 + ω_i)(1 − Tc_i/T)]`, i.e. exactly the
    /// correlation behind [`crate::thermo::flash::wilson_k_values`], so this
    /// fallback and the `Ideal` package agree by construction.
    ///
    /// `component_index` must be `< n_components()`; `t` \[K\] > 0, `p` \[Pa\] > 0.
    #[must_use]
    pub fn ideal_k_value(&self, component_index: usize, t: f64, p: f64) -> f64 {
        let c = &self.components[component_index];
        let psat = self.vapor_pressure_of(c, t);
        let k = psat / p;
        if k.is_finite() && k > 0.0 {
            k
        } else {
            1.0
        }
    }

    /// Wilson-correlation vapour pressure \[Pa\] of one component at `t` \[K\].
    ///
    /// Ports `AUX_PVAPi`'s role as the column solvers' K-value fallback (they
    /// only ever use it as `P_sat/P`). Note this is the *Wilson* estimate, not a
    /// fitted Antoine/Wagner curve: the crate's [`Component`] carries no
    /// vapour-pressure coefficients, so a corresponding-states estimate is the
    /// honest best available. This is only ever a fallback — the converged
    /// K-values come from the property package.
    #[must_use]
    pub fn vapor_pressure(&self, component_index: usize, t: f64) -> f64 {
        self.vapor_pressure_of(&self.components[component_index], t)
    }

    fn vapor_pressure_of(&self, c: &Component, t: f64) -> f64 {
        c.critical_pressure
            * (5.373 * (1.0 + c.acentric_factor) * (1.0 - c.critical_temperature / t)).exp()
    }

    /// Mixture molar mass \[kg/mol\] — `Σ x_i M_i`.
    ///
    /// Ports `PP.AUX_MMM(x)` (`BubblePoint.vb:980`), which returns g/mol; this
    /// returns **kg/mol**, so it composes directly with molar flows \[mol/s\] to
    /// give kg/s (upstream's `/1000` scalings are folded in here).
    /// `x` are mole fractions \[-\], normalised internally.
    #[must_use]
    pub fn mixture_molar_mass(&self, x: &[f64]) -> f64 {
        let xn = normalized(x);
        self.components
            .iter()
            .zip(xn.iter())
            .map(|(c, &xi)| xi * c.molar_mass)
            .sum()
    }

    /// Convert mole fractions to mass fractions \[-\].
    ///
    /// Ports `PP.AUX_CONVERT_MOL_TO_MASS(x)` (`BubblePoint.vb:222`), used by the
    /// mass-basis [`crate::columns::model::SpecType::ComponentFraction`] specs.
    #[must_use]
    pub fn mole_to_mass_fractions(&self, x: &[f64]) -> Vec<f64> {
        let xn = normalized(x);
        let total: f64 = self
            .components
            .iter()
            .zip(xn.iter())
            .map(|(c, &xi)| xi * c.molar_mass)
            .sum();
        if total <= 0.0 {
            return vec![0.0; xn.len()];
        }
        self.components
            .iter()
            .zip(xn.iter())
            .map(|(c, &xi)| xi * c.molar_mass / total)
            .collect()
    }

    /// `true` if the K-value vector has collapsed to the trivial solution
    /// `K_i ≈ 1` for every component.
    ///
    /// Ports `PP.AUX_CheckTrivial(Ki)` (`SumRates.vb:791`). Upstream's threshold
    /// is not exposed in the file; this port uses `|K_i − 1| < 1e-4` for all
    /// `i`, which is tight enough that a genuinely converged near-azeotropic
    /// stage is not flagged.
    #[must_use]
    pub fn is_trivial_solution(k: &[f64]) -> bool {
        !k.is_empty() && k.iter().all(|ki| (ki - 1.0).abs() < 1e-4)
    }

    /// Molar enthalpy of a **vapour** phase \[J/mol\] at `t` \[K\], `p` \[Pa\].
    ///
    /// Ports `PP.DW_CalcEnthalpy(y, T, P, State.Vapor) * AUX_MMM(y) / 1000`
    /// (`BubblePoint.vb:1482`). `y` are mole fractions \[-\], normalised
    /// internally.
    #[must_use]
    pub fn vapor_molar_enthalpy(&self, y: &[f64], t: f64, p: f64) -> f64 {
        let yn = normalized(y);
        let h_ig = mixture_ideal_gas_enthalpy(&self.components, &yn, t, self.reference_temperature);
        match (self.enthalpy_model, self.eos()) {
            (ColumnEnthalpyModel::EosDeparture, Some(eos)) => {
                match eos.enthalpy_departure(&self.components, &yn, t, p, Phase::Vapor, None) {
                    Some(dep) if dep.is_finite() => h_ig + dep,
                    _ => h_ig,
                }
            }
            _ => h_ig,
        }
    }

    /// Molar enthalpy of a **liquid** phase \[J/mol\] at `t` \[K\], `p` \[Pa\].
    ///
    /// Ports `PP.DW_CalcEnthalpy(x, T, P, State.Liquid) * AUX_MMM(x) / 1000`
    /// (`BubblePoint.vb:1480`). `x` are mole fractions \[-\], normalised
    /// internally.
    ///
    /// Under [`ColumnEnthalpyModel::EosDeparture`] the liquid residual comes
    /// from the cubic EOS's liquid `Z`-root; if that root does not exist the
    /// method falls back to `H_ig − Σ x_i ΔH_vap,i(T)` rather than returning a
    /// non-finite value.
    #[must_use]
    pub fn liquid_molar_enthalpy(&self, x: &[f64], t: f64, p: f64) -> f64 {
        let xn = normalized(x);
        let h_ig = mixture_ideal_gas_enthalpy(&self.components, &xn, t, self.reference_temperature);
        if let (ColumnEnthalpyModel::EosDeparture, Some(eos)) = (self.enthalpy_model, self.eos()) {
            if let Some(dep) =
                eos.enthalpy_departure(&self.components, &xn, t, p, Phase::Liquid, None)
            {
                if dep.is_finite() {
                    return h_ig + dep;
                }
            }
        }
        h_ig - self.mixture_enthalpy_of_vaporization(&xn, t)
    }

    /// Molar enthalpy \[J/mol\] of a stream of overall composition `z` at `t`
    /// \[K\], `p` \[Pa\] and vapour molar fraction `beta` \[-\] in `[0, 1]`.
    ///
    /// A convenience for building [`crate::columns::model::ColumnSolverInput::feed_enthalpies`]
    /// on the **same reference state** the solver's internal enthalpies use.
    /// `beta = 0` gives a saturated/subcooled liquid feed, `beta = 1` a
    /// superheated/saturated vapour feed; intermediate values interpolate the
    /// two phase enthalpies at the same composition, which is the two-phase
    /// feed enthalpy when the split is not resolved.
    #[must_use]
    pub fn feed_molar_enthalpy(&self, z: &[f64], t: f64, p: f64, beta: f64) -> f64 {
        let b = beta.clamp(0.0, 1.0);
        (1.0 - b) * self.liquid_molar_enthalpy(z, t, p) + b * self.vapor_molar_enthalpy(z, t, p)
    }

    /// Enthalpy of vaporization \[J/mol\] of one component at `t` \[K\].
    ///
    /// Vetere's correlation for `ΔH_vb` at the normal boiling point (DWSIM
    /// `Hypotheticals.vb:906-916`, `DHvb_Vetere`),
    ///
    /// `ΔH_vb = R Tc Tbr (0.4343 ln Pc − 0.69431 + 0.8954 Tbr) / (0.37691 − 0.37306 Tbr + 0.15075 Pc^{-1} Tbr^{-2})`
    ///
    /// with `Pc` in **bar** and `Tbr = Tb/Tc`, extrapolated to `T` by the Watson
    /// relation with DWSIM's exponent 0.375 (`PropertyPackage.vb:6820`,
    /// `HVap_A · ((1 − Tr)/(1 − Tbr))^0.375`).
    ///
    /// Returns `0.0` for `Tr >= 1` (supercritical — upstream's own early return,
    /// `PropertyPackage.vb:6812`) and for a component whose normal boiling point
    /// is missing or >= `Tc`.
    ///
    /// # Valid range
    ///
    /// The Watson extrapolation is reliable for `0.4 < Tr < 0.95`; near the
    /// critical point it under-predicts, and it is exactly zero at `Tr = 1`.
    #[must_use]
    pub fn enthalpy_of_vaporization(&self, component_index: usize, t: f64) -> f64 {
        let c = &self.components[component_index];
        let tc = c.critical_temperature;
        let tb = c.normal_boiling_point;
        let tr = t / tc;
        if !(tr < 1.0) || !tb.is_finite() || tb <= 0.0 || tb >= tc {
            return 0.0;
        }
        let tbr = tb / tc;
        let pc_bar = c.critical_pressure / 1.0e5;
        let num = R_VETERE * tc * tbr * (0.4343 * pc_bar.ln() - 0.69431 + 0.8954 * tbr);
        let den = 0.37691 - 0.37306 * tbr + 0.15075 / (pc_bar * tbr * tbr);
        if den == 0.0 || !den.is_finite() {
            return 0.0;
        }
        let dh_vb = num / den; // J/mol at Tb
        let watson = ((1.0 - tr) / (1.0 - tbr)).powf(0.375);
        let dh = dh_vb * watson;
        if dh.is_finite() && dh > 0.0 {
            dh
        } else {
            0.0
        }
    }

    /// Mole-fraction-weighted enthalpy of vaporization \[J/mol\] of a mixture.
    #[must_use]
    pub fn mixture_enthalpy_of_vaporization(&self, x: &[f64], t: f64) -> f64 {
        let xn = normalized(x);
        (0..self.components.len())
            .map(|i| xn[i] * self.enthalpy_of_vaporization(i, t))
            .sum()
    }

    /// Bubble-point temperature \[K\] of liquid `x` at `p` \[Pa\], plus the
    /// K-values there.
    ///
    /// Ports the `Flash_PV(x, P, VaporFraction = 0.0, T_guess, ...)` call the
    /// bubble-point solvers make once per stage per iteration
    /// (`BubblePoint.vb:1281`, `BubblePoint2.vb:1616`) — a PV flash at zero
    /// vapour fraction *is* a bubble-point calculation. Implemented on
    /// [`crate::thermo::saturation::bubble_temperature_with`], seeded from
    /// `t_guess`.
    ///
    /// # Parameters
    ///
    /// - `x` — liquid mole fractions \[-\], normalised internally.
    /// - `p` — stage pressure \[Pa\] > 0.
    /// - `t_guess` — starting temperature \[K\]; used to bracket the search
    ///   around the current profile so a stage does not jump to an unrelated
    ///   root.
    /// - `stage` — stage index, for error reporting only.
    ///
    /// # Returns
    ///
    /// `(T_bubble [K], K [-])`.
    ///
    /// # Errors
    ///
    /// [`ColumnError::BubblePointFailed`] carrying the underlying saturation
    /// error, mirroring upstream's "Error calculating bubble point temperature
    /// for stage {0} with P = {1} Pa" exception (`BubblePoint.vb:1283`).
    pub fn bubble_temperature(
        &self,
        x: &[f64],
        p: f64,
        t_guess: f64,
        stage: usize,
    ) -> Result<(f64, Vec<f64>), ColumnError> {
        let xn = normalized(x);
        let mut opts = SaturationOptions::default();
        // Bracket generously around the current estimate but stay physical.
        if t_guess.is_finite() && t_guess > 0.0 {
            opts.t_min = opts.t_min.max(0.2 * t_guess).max(1.0);
            opts.t_max = opts.t_max.min(5.0 * t_guess).max(opts.t_min * 1.5);
        }
        let comps = &self.components;
        let pkg = self.package;
        bubble_temperature_with(
            comps,
            &xn,
            p,
            |xx, yy, t, pp| pkg.k_values(comps, xx, yy, t, pp),
            opts,
        )
        .map(|st| {
            let k = st.k.clone();
            (st.temperature, k)
        })
        .map_err(|e| ColumnError::BubblePointFailed {
            stage,
            pressure: p,
            detail: e.to_string(),
        })
    }

    /// The cubic EOS behind the package, or `None` for
    /// [`PropertyPackageModel::Ideal`].
    fn eos(&self) -> Option<CubicEos> {
        match self.package {
            PropertyPackageModel::Ideal => None,
            PropertyPackageModel::PengRobinson => Some(CubicEos::PengRobinson),
            PropertyPackageModel::Srk => Some(CubicEos::Srk),
        }
    }

    /// Wilson K-value estimates \[-\] for the whole component set — the seed
    /// every solver's `k_values` profile starts from when the caller supplies
    /// none.
    #[must_use]
    pub fn wilson_k(&self, t: f64, p: f64) -> Vec<f64> {
        wilson_k_values(&self.components, t, p)
    }
}

/// Normalise a composition to sum 1, returning a uniform composition if the sum
/// is not positive.
///
/// DWSIM feeds un-normalised trial compositions into the property package all
/// through the column solvers (`BubblePoint.vb:1222` divides by `sumx` only
/// when it is positive); normalising at the bridge keeps the thermo kernel's
/// preconditions satisfied without changing any physics.
fn normalized(x: &[f64]) -> Vec<f64> {
    let s: f64 = x.iter().filter(|v| v.is_finite()).sum();
    if s > 0.0 {
        x.iter()
            .map(|v| if v.is_finite() { (v / s).max(0.0) } else { 0.0 })
            .collect()
    } else {
        vec![1.0 / x.len().max(1) as f64; x.len()]
    }
}

#[cfg(test)]
pub(crate) mod tests {
    //! # V&V — verification of the column thermo bridge
    //!
    //! **Methodology.** Each test checks an internal-consistency or
    //! literature-qualitative property, **not** an experimental data set:
    //!
    //! - Vetere+Watson `ΔH_vap` for benzene at its normal boiling point must
    //!   land near the literature value 30.7 kJ/mol (Poling, Prausnitz &
    //!   O'Connell, *The Properties of Gases and Liquids*, 5th ed., 2001,
    //!   Appendix A — public literature), and must fall monotonically to zero at
    //!   `Tc`.
    //! - The liquid molar enthalpy must be **below** the vapour molar enthalpy
    //!   at the same `(x, T, P)` by roughly the mixture latent heat — the
    //!   property the MESH energy balance depends on.
    //! - The bubble-point temperature of pure benzene at 101.325 kPa must be
    //!   near its normal boiling point (353.24 K), which is the definition of
    //!   the normal boiling point.
    //! - K-values must be finite and correctly ordered for a
    //!   benzene(light)/toluene(heavy) binary.
    //!
    //! **Scope (honesty).** Verification, NOT validation: the K-values use
    //! `k_ij = 0`, the `ΔH_vap` route is a corresponding-states correlation, and
    //! no comparison against a measured VLE or calorimetric data set is made.
    //! AI-assisted draft, no human V&V.
    //!
    //! **Results** are recorded per test below, measured 2026-08-11 on a release
    //! build of this port.

    use super::*;

    /// Benzene constants. Poling, Prausnitz & O'Connell, *The Properties of
    /// Gases and Liquids*, 5th ed. (2001), Appendix A — public literature.
    /// `Cp0/R = a0 + a1 T + a2 T² + a3 T³ + a4 T⁴` coefficients converted to
    /// J/(mol·K) by multiplying through by `R = 8.314462618`.
    #[must_use]
    pub(crate) fn benzene() -> Component {
        const R: f64 = 8.314_462_618;
        Component::new(
            "Benzene",
            0.078_114,
            562.05,
            48.95e5,
            256.0e-6,
            0.210,
            353.24,
            [
                3.551 * R,
                -6.184e-3 * R,
                1.4365e-4 * R,
                -1.9807e-7 * R,
                8.234e-11 * R,
            ],
            f64::NAN,
        )
        .expect("benzene reference constants are valid")
    }

    /// Toluene constants. Same source and conversion as [`benzene`].
    #[must_use]
    pub(crate) fn toluene() -> Component {
        const R: f64 = 8.314_462_618;
        Component::new(
            "Toluene",
            0.092_141,
            591.75,
            41.08e5,
            316.0e-6,
            0.264,
            383.79,
            [
                3.866 * R,
                3.558e-3 * R,
                1.3356e-4 * R,
                -1.9463e-7 * R,
                8.363e-11 * R,
            ],
            f64::NAN,
        )
        .expect("toluene reference constants are valid")
    }

    /// **Methodology.** Vetere `ΔH_vb` + Watson extrapolation for benzene,
    /// evaluated at `T = Tb = 353.24 K` (where the Watson factor is exactly 1,
    /// so this tests Vetere alone) and at `T = Tc`. Reference value:
    /// `ΔH_vb(benzene) = 30.72 kJ/mol` (Poling et al., 5th ed., Appendix A).
    /// Pass criterion: within 15 % of 30.72 kJ/mol at `Tb` (Vetere's own stated
    /// accuracy for hydrocarbons is ~2 %, but the correlation is being fed
    /// only Tc/Pc/Tb, so a looser gate is honest), and exactly 0 at `Tr >= 1`.
    /// **Result (2026-08-11, release):** `ΔH_vap(353.24 K) = 30 458.5 J/mol`,
    /// −0.85 % from the 30 720 J/mol literature value; `ΔH_vap(562.05 K) = 0`
    /// and `ΔH_vap(700 K) = 0`; the profile decreases monotonically over
    /// 300-560 K.
    /// **Interpretation:** the Vetere+Watson fallback reproduces benzene's
    /// latent heat well enough for the MESH energy balance, which is its only
    /// job here.
    #[test]
    fn vetere_watson_hvap_matches_benzene_literature() {
        let th = ColumnThermo::new(vec![benzene()], PropertyPackageModel::Ideal);
        let at_tb = th.enthalpy_of_vaporization(0, 353.24);
        let literature = 30_720.0_f64;
        let rel = (at_tb - literature).abs() / literature;
        assert!(
            rel < 0.15,
            "ΔH_vap(Tb) = {at_tb} J/mol, relative deviation {rel} from 30720 J/mol"
        );
        assert_eq!(th.enthalpy_of_vaporization(0, 562.05), 0.0);
        assert_eq!(th.enthalpy_of_vaporization(0, 700.0), 0.0);

        // Monotone decrease with temperature.
        let mut prev = f64::INFINITY;
        for k in 0..27 {
            let t = 300.0 + 10.0 * k as f64;
            let h = th.enthalpy_of_vaporization(0, t);
            assert!(h <= prev + 1e-9, "ΔH_vap not monotone at T = {t}");
            prev = h;
        }
    }

    /// **Methodology.** For an equimolar benzene/toluene liquid at 101.325 kPa
    /// and 365 K, the liquid molar enthalpy must sit below the vapour molar
    /// enthalpy by approximately the mixture latent heat, under **both**
    /// enthalpy models. Pass criterion: `H_v − H_l` positive and within a
    /// factor of 2 of `Σ x_i ΔH_vap,i`.
    /// **Result (2026-08-11, release):** ideal+latent model
    /// `H_l = −25 239.8 J/mol`, `H_v = +6 867.9 J/mol`,
    /// `H_v − H_l = 32 107.7 J/mol` versus `Σ x_i ΔH_vap,i = 32 107.7 J/mol`
    /// (identical by construction); Peng-Robinson EOS-departure model
    /// `H_l = −25 360.2 J/mol`, `H_v = +6 608.9 J/mol`,
    /// `H_v − H_l = 31 969.1 J/mol`, i.e. **0.9957x** the correlation value.
    /// **Interpretation:** both enthalpy routes give a physically sane latent
    /// heat, and the two independent routes agree to ~2 %, which is the
    /// cross-check this test exists for.
    #[test]
    fn liquid_enthalpy_is_below_vapor_by_the_latent_heat() {
        let comps = vec![benzene(), toluene()];
        let x = [0.5, 0.5];
        let (t, p) = (365.0, 101_325.0);

        for pkg in [
            PropertyPackageModel::Ideal,
            PropertyPackageModel::PengRobinson,
        ] {
            let th = ColumnThermo::new(comps.clone(), pkg);
            let hl = th.liquid_molar_enthalpy(&x, t, p);
            let hv = th.vapor_molar_enthalpy(&x, t, p);
            let latent = th.mixture_enthalpy_of_vaporization(&x, t);
            assert!(hl.is_finite() && hv.is_finite(), "{pkg:?}: non-finite H");
            assert!(hv > hl, "{pkg:?}: H_v = {hv} must exceed H_l = {hl}");
            let dh = hv - hl;
            assert!(
                dh > 0.5 * latent && dh < 2.0 * latent,
                "{pkg:?}: H_v − H_l = {dh} J/mol vs latent {latent} J/mol"
            );
        }
    }

    /// **Methodology.** The bubble-point temperature of (essentially) pure
    /// benzene at 101.325 kPa must equal its normal boiling point, 353.24 K,
    /// by definition. Computed with the `Ideal` (Wilson) package, whose
    /// vapour-pressure correlation is a corresponding-states estimate rather
    /// than a fitted Antoine curve, so a 10 K gate is the honest tolerance.
    /// **Result (2026-08-11, release):** `T_bubble = 352.0856 K`, −1.154 K
    /// from the 353.24 K literature normal boiling point (−0.327 %), with
    /// `K = [1.00060, 0.39828]`.
    /// **Interpretation:** the Wilson vapour-pressure estimate carries a few
    /// kelvin of error at atmospheric pressure, as expected for a two-parameter
    /// corresponding-states correlation; the bubble-point *solver* itself is
    /// converging on the right equation.
    #[test]
    fn bubble_temperature_of_benzene_is_near_its_boiling_point() {
        let th = ColumnThermo::new(vec![benzene(), toluene()], PropertyPackageModel::Ideal);
        let (tb, k) = th
            .bubble_temperature(&[0.999, 0.001], 101_325.0, 350.0, 0)
            .expect("bubble point must converge for near-pure benzene");
        assert!(
            (tb - 353.24).abs() < 10.0,
            "T_bubble = {tb} K, expected near 353.24 K"
        );
        assert!(k.iter().all(|v| v.is_finite() && *v > 0.0));
        assert!(k[0] > k[1], "benzene must be the more volatile component");
    }

    /// **Methodology.** K-values from the bridge must be finite, positive, and
    /// ordered light > heavy for benzene/toluene at 365 K, 101.325 kPa, under
    /// both the Ideal and Peng-Robinson packages. Also checks the non-finite
    /// guard: an empty/zero composition must not produce a `NaN` K.
    /// **Result (2026-08-11, release):** Ideal `K = [1.4446, 0.5965]`;
    /// Peng-Robinson `K = [1.4069, 0.5805]`; both ordered
    /// `K_benzene > K_toluene` and both bracketing 1, which is what a mixture
    /// near its bubble point requires. The zero-composition guard returned
    /// finite positive K-values in both cases.
    #[test]
    fn k_values_are_finite_and_correctly_ordered() {
        let comps = vec![benzene(), toluene()];
        for pkg in [
            PropertyPackageModel::Ideal,
            PropertyPackageModel::PengRobinson,
        ] {
            let th = ColumnThermo::new(comps.clone(), pkg);
            let k = th.k_values(&[0.5, 0.5], &[0.7, 0.3], 365.0, 101_325.0);
            assert!(
                k.iter().all(|v| v.is_finite() && *v > 0.0),
                "{pkg:?}: {k:?}"
            );
            assert!(k[0] > k[1], "{pkg:?}: benzene must be more volatile: {k:?}");

            // Zero composition must not poison the result.
            let k0 = th.k_values(&[0.0, 0.0], &[0.0, 0.0], 365.0, 101_325.0);
            assert!(
                k0.iter().all(|v| v.is_finite() && *v > 0.0),
                "{pkg:?}: {k0:?}"
            );
        }
    }

    /// **Methodology.** Mass-fraction conversion and mixture molar mass must be
    /// consistent: `Σ w_i = 1` and `M_mix = Σ x_i M_i`. Checked on an equimolar
    /// benzene/toluene mixture, where `M_mix = (78.114 + 92.141)/2 = 85.128`
    /// g/mol and `w_benzene = 78.114/170.255 = 0.4588`.
    /// **Result (2026-08-11):** `M_mix = 0.085127500 kg/mol`,
    /// `w = [0.4588059, 0.5411941]`, both matching the hand calculation to
    /// < 1e-12.
    #[test]
    fn molar_mass_and_mass_fraction_conversion_are_consistent() {
        let th = ColumnThermo::new(vec![benzene(), toluene()], PropertyPackageModel::Ideal);
        let m = th.mixture_molar_mass(&[0.5, 0.5]);
        assert!((m - 0.5 * (0.078_114 + 0.092_141)).abs() < 1e-12);
        let w = th.mole_to_mass_fractions(&[0.5, 0.5]);
        assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!((w[0] - 0.078_114 / (0.078_114 + 0.092_141)).abs() < 1e-12);
    }

    /// **Methodology.** The trivial-solution detector must fire on `K = 1` and
    /// not on a genuine split. Ports `AUX_CheckTrivial` (`SumRates.vb:791`).
    /// **Result (2026-08-11):** fires on `[1.0, 1.0]`, silent on `[2.0, 0.5]`.
    #[test]
    fn trivial_solution_detector_works() {
        assert!(ColumnThermo::is_trivial_solution(&[1.0, 1.0, 1.0]));
        assert!(!ColumnThermo::is_trivial_solution(&[2.0, 0.5]));
        assert!(!ColumnThermo::is_trivial_solution(&[]));
    }
}
