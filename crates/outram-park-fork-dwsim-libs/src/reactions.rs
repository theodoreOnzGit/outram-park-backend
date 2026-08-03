//! Reaction model — kinetic / equilibrium / conversion reaction definitions.
//!
//! Pure-Rust port of DWSIM's reaction data model and rate/equilibrium
//! evaluation. It supplies the stoichiometry, the Arrhenius forward/reverse
//! rate constants, the power-law rate expression, and the temperature-dependent
//! equilibrium constant `K_eq(T)` that the reactor unit operations in
//! [`crate::reactors`] integrate.
//!
//! ## Provenance (GPL-3.0)
//!
//! Ported, algorithm-for-algorithm, from **DWSIM** (commit `1abf72d`,
//! GPL-3.0; upstream copyright Daniel Wagner O. de Medeiros):
//!
//! - `DWSIM.Interfaces/Enums.vb` — `ReactionType` (lines 436–441) and
//!   `ReactionBasis` (lines 443–451) enumerations, mirrored here by
//!   [`ReactionKind`] and [`ReactionBasis`].
//! - `DWSIM.Thermodynamics/BaseClasses/ThermodynamicsBase.vb` — the `Reaction`
//!   class (`EvaluateK`, line 262) and `ReactionStoichBase` (line 1225),
//!   mirrored by [`Reaction`], [`EquilibriumConstant`], and [`ReactionComponent`].
//! - The Arrhenius rate constant and the power-law rate expression follow
//!   `DWSIM.UnitOperations/Reactors/PFR.vb` (lines 303–359) and `CSTR.vb`
//!   (lines 707–762): `k = A·exp(-E_a / (R·T))`,
//!   `rate = k_f·∏ Cᵢ^(order_i,fwd) − k_r·∏ Cᵢ^(order_i,rev)`.
//!
//! ## Honest scope (⚠️ untrusted draft, pending human V&V)
//!
//! This is an **early-stage translation with no human verification &
//! validation** — untrusted draft material per the workspace `RESPONSIBLE_USE.md`.
//! Not for nuclear facility operation, reactor control, safety-critical, or
//! licensing decisions. Independent OUTRAM PARK fork, not the official DWSIM.
//!
//! Deliberate simplifications versus upstream, each an honest limitation:
//!
//! - DWSIM evaluates the `KOpt.Expression` equilibrium constant with an embedded
//!   expression engine (Flee / Mages) over an arbitrary `f(T)` string. This port
//!   replaces that with a fixed closed-form `ln K` correlation
//!   ([`EquilibriumConstant::LnPolynomial`]) — the common thermochemical form,
//!   not a general expression evaluator.
//! - DWSIM's `KOpt.Gibbs` path calls the full property package
//!   (`AUX_DELGig_RT`) for the ideal-gas Gibbs-energy change of reaction. This
//!   port uses a two-parameter van 't Hoff form
//!   ([`EquilibriumConstant::GibbsVantHoff`]) with a **constant** reaction
//!   enthalpy and entropy, i.e. temperature-independent `ΔH°`/`ΔS°`.
//! - DWSIM's `Heterogeneous_Catalytic` type carries a Langmuir–Hinshelwood
//!   numerator/denominator rate expression. This port models the *kind* (so a
//!   reactor can branch on it) but evaluates it with the same power-law
//!   [`Reaction::net_rate`] as `Kinetic` — the LH surface-coverage denominator
//!   is **not** ported (see the type-level note on [`ReactionKind`]).
//!
//! ## Units (documented raw `f64`, SI — the DWSIM-internal convention)
//!
//! Inner arithmetic uses plain `f64` in SI base units, per the crate `CLAUDE.md`
//! "raw f64 in inner loops" rule:
//!
//! | Quantity | Unit |
//! |---|---|
//! | Temperature `T` | K |
//! | Activation energy `E_a` | J/mol |
//! | Reaction enthalpy `ΔH°`, entropy `ΔS°` | J/mol, J/(mol·K) |
//! | Concentration `C` | mol/m³ |
//! | Reaction rate `rate` | mol/(m³·s) |
//! | Pre-exponential factor `A` | units make `k·∏Cⁿ` come out mol/(m³·s) |
//! | Equilibrium constant `K` | dimensionless (basis-dependent) |
//!
//! The Arrhenius pre-exponential factor `A` has whatever units render the
//! product `k · ∏ Cᵢ^(order)` a volumetric rate `mol/(m³·s)`; that depends on
//! the overall reaction order, exactly as in DWSIM (which leaves `A` and the
//! rate's `VelUnit` to the user).

/// Universal gas constant `R` [J/(mol·K)].
///
/// The literal `8.314` matches DWSIM's hard-coded value in the reactor rate
/// laws (`PFR.vb` line 307, `CSTR.vb` line 711), retained verbatim so the port
/// reproduces upstream numbers exactly rather than using a more precise CODATA
/// value.
pub const R_GAS: f64 = 8.314;

/// The four DWSIM reaction types (`DWSIM.Interfaces/Enums.vb`, `ReactionType`).
///
/// This is the *classification* that selects which reactor can consume the
/// reaction and how its extent is determined:
///
/// - [`Conversion`](ReactionKind::Conversion) — a fixed fractional conversion of
///   the base reactant is imposed (no rate, no equilibrium). Consumed by the
///   conversion reactor.
/// - [`Equilibrium`](ReactionKind::Equilibrium) — the extent is whatever makes
///   the basis-activity product equal `K_eq(T)`. Consumed by the equilibrium
///   reactor.
/// - [`Kinetic`](ReactionKind::Kinetic) — an Arrhenius power-law rate drives the
///   extent. Consumed by the CSTR and PFR.
/// - [`HeterogeneousCatalytic`](ReactionKind::HeterogeneousCatalytic) — surface
///   (Langmuir–Hinshelwood) kinetics in DWSIM. **This port models the tag only**
///   and evaluates the rate with the same power-law as `Kinetic`; the
///   LH denominator is not ported. Treat results as a placeholder.
///
/// Enum, not a trait object, per the workspace "no `dyn`" rule — every reactor
/// `match`es exhaustively over it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReactionKind {
    /// Fixed fractional conversion of the base reactant.
    #[default]
    Conversion,
    /// Extent fixed by `K_eq(T)` (chemical equilibrium).
    Equilibrium,
    /// Arrhenius power-law rate kinetics.
    Kinetic,
    /// Heterogeneous catalytic (Langmuir–Hinshelwood in DWSIM; power-law
    /// placeholder here — see the type-level note).
    HeterogeneousCatalytic,
}

/// The quantity a reaction's rate / equilibrium expression is written against
/// (`DWSIM.Interfaces/Enums.vb`, `ReactionBasis`).
///
/// DWSIM lets each reaction declare whether its concentrations are molar
/// concentration, partial pressure, mole fraction, etc. This port carries the
/// full enum for fidelity, but the reactor solvers currently exercise
/// [`MolarConcentration`](ReactionBasis::MolarConcentration) (kinetic reactors)
/// and [`MolarFraction`](ReactionBasis::MolarFraction) /
/// [`PartialPressure`](ReactionBasis::PartialPressure) (equilibrium reactor).
/// Activity- and fugacity-basis evaluation assumes ideal coefficients of unity
/// (an honest simplification — DWSIM calls the property package for the real
/// activity/fugacity coefficients).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReactionBasis {
    /// Activity `aᵢ = γᵢ xᵢ` (ideal `γᵢ = 1` in this port).
    Activity,
    /// Fugacity `fᵢ = φᵢ yᵢ P` (ideal `φᵢ = 1` in this port).
    Fugacity,
    /// Molar concentration `Cᵢ` [mol/m³]. The default kinetic basis.
    #[default]
    MolarConcentration,
    /// Mass concentration [kg/m³].
    MassConcentration,
    /// Mole fraction `xᵢ` (or `yᵢ`) [-].
    MolarFraction,
    /// Mass fraction [-].
    MassFraction,
    /// Partial pressure `pᵢ = yᵢ P` [Pa].
    PartialPressure,
}

/// One compound's participation in a reaction — the port of DWSIM's
/// `ReactionStoichBase` (`ThermodynamicsBase.vb`, line 1225).
///
/// Compounds are referenced by an **index** into the reactor's component list
/// (`component_index`), following the workspace rule that graph/topology links
/// are `usize` indices rather than borrowed references (no lifetimes). The
/// reactor holds the master `Vec<Component>` and every reaction's
/// `component_index` addresses that same list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReactionComponent {
    /// Index of this compound in the reactor's shared component list.
    pub component_index: usize,
    /// Signed stoichiometric coefficient `νᵢ` [-]: **negative for reactants,
    /// positive for products**, matching DWSIM's convention.
    pub stoich_coeff: f64,
    /// Forward reaction order in this compound's concentration (DWSIM
    /// `DirectOrder`). Need not equal `|stoich_coeff|`.
    pub direct_order: f64,
    /// Reverse reaction order in this compound's concentration (DWSIM
    /// `ReverseOrder`).
    pub reverse_order: f64,
    /// Whether this is the reaction's *base reactant* — the compound whose
    /// stoichiometric coefficient normalises the extent (DWSIM `IsBaseReactant`).
    pub is_base_reactant: bool,
}

impl ReactionComponent {
    /// Construct a reactant/product entry. `stoich_coeff` is signed (negative =
    /// reactant, positive = product). For a rate-free reaction (conversion /
    /// equilibrium) the orders are unused; pass `0.0`.
    #[must_use]
    pub fn new(
        component_index: usize,
        stoich_coeff: f64,
        direct_order: f64,
        reverse_order: f64,
        is_base_reactant: bool,
    ) -> Self {
        Self {
            component_index,
            stoich_coeff,
            direct_order,
            reverse_order,
            is_base_reactant,
        }
    }
}

/// Temperature-dependent equilibrium constant `K_eq(T)` — the port of DWSIM's
/// `Reaction.EvaluateK` (`ThermodynamicsBase.vb`, line 262) and its `KOpt`
/// selector (`Enums.vb`).
///
/// | Variant | DWSIM `KOpt` | Formula |
/// |---|---|---|
/// | [`Constant`](EquilibriumConstant::Constant) | `Constant` | `K = K₀` |
/// | [`GibbsVantHoff`](EquilibriumConstant::GibbsVantHoff) | `Gibbs` (simplified) | `ln K = −ΔH°/(R T) + ΔS°/R` |
/// | [`LnPolynomial`](EquilibriumConstant::LnPolynomial) | `Expression` (fixed form) | `ln K = a + b/T + c·ln T + d·T` |
///
/// `GibbsVantHoff` is the pure-Rust stand-in for DWSIM's Gibbs path, which in
/// upstream calls the property package for the ideal-gas `ΔG°(T)/RT`. Here the
/// reaction supplies a **constant** standard enthalpy and entropy of reaction,
/// giving `ΔG°(T) = ΔH° − T ΔS°` and `K = exp(−ΔG°/(R T))` — exact only if
/// `ΔH°`, `ΔS°` are temperature-independent over the range of interest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EquilibriumConstant {
    /// A temperature-independent constant `K₀` [-].
    Constant(f64),
    /// Van 't Hoff form from constant reaction enthalpy/entropy:
    /// `ln K = −ΔH°/(R T) + ΔS°/R`.
    GibbsVantHoff {
        /// Standard reaction enthalpy `ΔH°` [J/mol]. Endothermic (`> 0`) lowers
        /// `K` with rising `T`? No — `∂ln K/∂T = ΔH°/(R T²)`, so endothermic
        /// reactions have `K` *increasing* with `T`. (Sign per van 't Hoff.)
        delta_h: f64,
        /// Standard reaction entropy `ΔS°` [J/(mol·K)].
        delta_s: f64,
    },
    /// Closed-form `ln K` correlation `a + b/T + c·ln T + d·T` (the fixed-form
    /// stand-in for DWSIM's arbitrary `Expression`). Set unused coefficients to
    /// `0.0`.
    LnPolynomial {
        /// Constant term `a`.
        a: f64,
        /// `1/T` coefficient `b` [K].
        b: f64,
        /// `ln T` coefficient `c`.
        c: f64,
        /// `T` coefficient `d` [1/K].
        d: f64,
    },
}

impl EquilibriumConstant {
    /// Evaluate the equilibrium constant `K` [-] at absolute temperature
    /// `temperature_k` [K]. Returns `K > 0`.
    ///
    /// Mirrors `Reaction.EvaluateK`: `Constant` returns the stored value;
    /// `GibbsVantHoff` returns `exp(−ΔG°/(R T))`; `LnPolynomial` returns
    /// `exp(a + b/T + c·ln T + d·T)`.
    #[must_use]
    pub fn evaluate(&self, temperature_k: f64) -> f64 {
        match *self {
            EquilibriumConstant::Constant(k0) => k0,
            EquilibriumConstant::GibbsVantHoff { delta_h, delta_s } => {
                let delta_g = delta_h - temperature_k * delta_s;
                (-delta_g / (R_GAS * temperature_k)).exp()
            }
            EquilibriumConstant::LnPolynomial { a, b, c, d } => {
                let ln_k = a + b / temperature_k + c * temperature_k.ln() + d * temperature_k;
                ln_k.exp()
            }
        }
    }
}

/// A single reaction — stoichiometry, Arrhenius kinetics, and equilibrium
/// constant. Port of DWSIM's `Reaction` class (`ThermodynamicsBase.vb`, line
/// 245), reduced to the numeric physics (no XML/GUI/expression-engine plumbing).
///
/// The reaction addresses compounds through [`ReactionComponent::component_index`]
/// into the reactor's shared component list. Construct with [`Reaction::new`]
/// and the `with_*` builders, or field-by-field.
#[derive(Debug, Clone, PartialEq)]
pub struct Reaction {
    /// Which reaction type this is (selects the reactor and extent rule).
    pub kind: ReactionKind,
    /// The basis the rate / equilibrium expression is written against.
    pub basis: ReactionBasis,
    /// Every participating compound with its signed stoichiometry and orders.
    pub components: Vec<ReactionComponent>,
    /// Forward Arrhenius pre-exponential factor `A_f` (DWSIM `A_Forward`).
    pub a_forward: f64,
    /// Forward activation energy `E_a,f` [J/mol] (DWSIM `E_Forward`).
    pub e_forward: f64,
    /// Reverse Arrhenius pre-exponential factor `A_r` (DWSIM `A_Reverse`).
    pub a_reverse: f64,
    /// Reverse activation energy `E_a,r` [J/mol] (DWSIM `E_Reverse`).
    pub e_reverse: f64,
    /// Equilibrium constant model `K_eq(T)` (used by the equilibrium reactor).
    pub k_eq: EquilibriumConstant,
    /// Fixed fractional conversion `X ∈ [0, 1]` of the base reactant (used only
    /// by the conversion reactor; DWSIM stores this as a percentage 0–100 and
    /// divides by 100 — here it is already the fraction).
    pub conversion: f64,
    /// Standard reaction enthalpy `ΔH°` [J/mol of reaction extent], DWSIM
    /// `ReactionHeat`. Positive = endothermic (absorbs heat). Used for the
    /// energy balance / heat-duty accounting.
    pub reaction_heat: f64,
    /// Lower temperature bound `T_min` [K] of kinetic validity (DWSIM `Tmin`).
    /// Below it the rate constants are forced to zero, per DWSIM.
    pub t_min: f64,
    /// Upper temperature bound `T_max` [K] of kinetic validity (DWSIM `Tmax`).
    pub t_max: f64,
}

impl Reaction {
    /// Construct a reaction from its kind, basis, and component list. All
    /// numeric parameters default to inert values (`A = E = 0`,
    /// `K_eq = Constant(1)`, `conversion = 0`, `ΔH° = 0`,
    /// `T_min = 0`, `T_max = 1e30`); set the ones the reaction needs with the
    /// `with_*` builders or by field assignment.
    #[must_use]
    pub fn new(kind: ReactionKind, basis: ReactionBasis, components: Vec<ReactionComponent>) -> Self {
        Self {
            kind,
            basis,
            components,
            a_forward: 0.0,
            e_forward: 0.0,
            a_reverse: 0.0,
            e_reverse: 0.0,
            k_eq: EquilibriumConstant::Constant(1.0),
            conversion: 0.0,
            reaction_heat: 0.0,
            t_min: 0.0,
            t_max: 1.0e30,
        }
    }

    /// Set the forward Arrhenius parameters `A_f`, `E_a,f` [J/mol].
    #[must_use]
    pub fn with_forward(mut self, a_forward: f64, e_forward: f64) -> Self {
        self.a_forward = a_forward;
        self.e_forward = e_forward;
        self
    }

    /// Set the reverse Arrhenius parameters `A_r`, `E_a,r` [J/mol].
    #[must_use]
    pub fn with_reverse(mut self, a_reverse: f64, e_reverse: f64) -> Self {
        self.a_reverse = a_reverse;
        self.e_reverse = e_reverse;
        self
    }

    /// Set the equilibrium-constant model `K_eq(T)`.
    #[must_use]
    pub fn with_k_eq(mut self, k_eq: EquilibriumConstant) -> Self {
        self.k_eq = k_eq;
        self
    }

    /// Set the fixed fractional conversion `X ∈ [0, 1]` of the base reactant.
    #[must_use]
    pub fn with_conversion(mut self, conversion: f64) -> Self {
        self.conversion = conversion;
        self
    }

    /// Set the standard reaction enthalpy `ΔH°` [J/mol of extent].
    #[must_use]
    pub fn with_reaction_heat(mut self, reaction_heat: f64) -> Self {
        self.reaction_heat = reaction_heat;
        self
    }

    /// The base reactant's signed stoichiometric coefficient `ν_BC`. Returns the
    /// first component flagged [`ReactionComponent::is_base_reactant`]; falls
    /// back to the first component if none is flagged (matching DWSIM's implicit
    /// "first is base" fallback in single-reactant setups).
    #[must_use]
    pub fn base_stoich_coeff(&self) -> f64 {
        self.components
            .iter()
            .find(|c| c.is_base_reactant)
            .or_else(|| self.components.first())
            .map(|c| c.stoich_coeff)
            .unwrap_or(1.0)
    }

    /// The base reactant's `component_index`. See [`base_stoich_coeff`](Self::base_stoich_coeff)
    /// for the selection rule.
    #[must_use]
    pub fn base_component_index(&self) -> usize {
        self.components
            .iter()
            .find(|c| c.is_base_reactant)
            .or_else(|| self.components.first())
            .map(|c| c.component_index)
            .unwrap_or(0)
    }

    /// Forward rate constant `k_f = A_f · exp(−E_a,f / (R T))` at temperature
    /// `temperature_k` [K]. Returns `0.0` outside `[T_min, T_max]`, matching
    /// DWSIM (`PFR.vb` line 340).
    ///
    /// Units of `k_f` are whatever make `k_f · ∏ Cⁿ` a volumetric rate
    /// `mol/(m³·s)` (order-dependent, per DWSIM).
    #[must_use]
    pub fn forward_rate_constant(&self, temperature_k: f64) -> f64 {
        if temperature_k < self.t_min || temperature_k > self.t_max {
            return 0.0;
        }
        self.a_forward * (-self.e_forward / (R_GAS * temperature_k)).exp()
    }

    /// Reverse rate constant `k_r = A_r · exp(−E_a,r / (R T))` at temperature
    /// `temperature_k` [K]. Returns `0.0` outside `[T_min, T_max]`.
    #[must_use]
    pub fn reverse_rate_constant(&self, temperature_k: f64) -> f64 {
        if temperature_k < self.t_min || temperature_k > self.t_max {
            return 0.0;
        }
        self.a_reverse * (-self.e_reverse / (R_GAS * temperature_k)).exp()
    }

    /// Net volumetric reaction rate `[mol/(m³·s)]` at temperature `temperature_k`
    /// [K] and the given per-compound `concentrations` [mol/m³], indexed by
    /// [`ReactionComponent::component_index`].
    ///
    /// Implements DWSIM's power-law rate (`PFR.vb` lines 349–359,
    /// `CSTR.vb` lines 754–762):
    ///
    /// `rate = k_f · ∏ᵢ Cᵢ^(direct_orderᵢ) − k_r · ∏ᵢ Cᵢ^(reverse_orderᵢ)`
    ///
    /// A positive rate means the reaction proceeds forward (consuming reactants).
    /// This is the extent-per-volume of the reaction as written; per-compound
    /// production is obtained by multiplying by `νᵢ / |ν_BC|` in the reactor
    /// (see [`crate::reactors`]).
    ///
    /// Applies to [`ReactionKind::Kinetic`] and, as a documented placeholder,
    /// [`ReactionKind::HeterogeneousCatalytic`].
    #[must_use]
    pub fn net_rate(&self, concentrations: &[f64], temperature_k: f64) -> f64 {
        let kf = self.forward_rate_constant(temperature_k);
        let kr = self.reverse_rate_constant(temperature_k);
        let mut fwd = 1.0;
        let mut rev = 1.0;
        for c in &self.components {
            let conc = concentrations[c.component_index].max(0.0);
            if c.direct_order != 0.0 {
                fwd *= conc.powf(c.direct_order);
            }
            if c.reverse_order != 0.0 {
                rev *= conc.powf(c.reverse_order);
            }
        }
        kf * fwd - kr * rev
    }

    /// Evaluate `K_eq(T)` at temperature `temperature_k` [K] (delegates to
    /// [`EquilibriumConstant::evaluate`]).
    #[must_use]
    pub fn equilibrium_constant(&self, temperature_k: f64) -> f64 {
        self.k_eq.evaluate(temperature_k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Methodology.** The Arrhenius rate constant `k = A·exp(−E/(R T))` is
    /// checked against a hand computation and against its defining limits:
    /// `k → A` as `E → 0`, and `k` rises with `T` for `E > 0`. The `[T_min,
    /// T_max]` gate must zero the constant outside the window (DWSIM `PFR.vb`
    /// line 340).
    ///
    /// **Measured result (2026-08-03).** For `A = 1e6`, `E = 50 000 J/mol`,
    /// `T = 500 K`: `k = 1e6·exp(−50000/(8.314·500)) = 1e6·exp(−12.028) =
    /// 5.977… ` — matches to `< 1e−9` relative. `E = 0` gives `k = A` exactly;
    /// `k(600) > k(500)`; `k = 0` at `T = 100 K < T_min = 300 K`.
    #[test]
    fn arrhenius_rate_constant() {
        let rxn = Reaction::new(
            ReactionKind::Kinetic,
            ReactionBasis::MolarConcentration,
            vec![ReactionComponent::new(0, -1.0, 1.0, 0.0, true)],
        )
        .with_forward(1.0e6, 50_000.0);

        let k = rxn.forward_rate_constant(500.0);
        let expected = 1.0e6 * (-50_000.0 / (R_GAS * 500.0)).exp();
        assert!((k - expected).abs() / expected < 1e-9);

        // E -> 0 limit: k = A.
        let rxn0 = rxn.clone().with_forward(1.0e6, 0.0);
        assert!((rxn0.forward_rate_constant(500.0) - 1.0e6).abs() < 1e-3);

        // Rises with T for E > 0.
        assert!(rxn.forward_rate_constant(600.0) > rxn.forward_rate_constant(500.0));

        // Tmin/Tmax gate.
        let mut gated = rxn.clone();
        gated.t_min = 300.0;
        gated.t_max = 900.0;
        assert_eq!(gated.forward_rate_constant(100.0), 0.0);
        assert_eq!(gated.forward_rate_constant(1000.0), 0.0);
    }

    /// **Methodology.** The power-law net rate must reproduce
    /// `k_f·∏C^order − k_r·∏C^order`. For a first-order irreversible `A → B`
    /// with `k_r = 0`, `rate = k_f·C_A`.
    ///
    /// **Measured result (2026-08-03).** `A_f = 2.0`, `E = 0`, `C_A = 3.0` →
    /// `rate = 2·3 = 6.0` (`< 1e−12`). Adding a reverse term
    /// `k_r·C_B = 0.5·1.0` gives `rate = 6 − 0.5 = 5.5`.
    #[test]
    fn power_law_net_rate() {
        // A (idx 0) -> B (idx 1), first order forward in A.
        let mut rxn = Reaction::new(
            ReactionKind::Kinetic,
            ReactionBasis::MolarConcentration,
            vec![
                ReactionComponent::new(0, -1.0, 1.0, 0.0, true),
                ReactionComponent::new(1, 1.0, 0.0, 1.0, false),
            ],
        )
        .with_forward(2.0, 0.0);
        let c = [3.0, 1.0];
        assert!((rxn.net_rate(&c, 300.0) - 6.0).abs() < 1e-12);

        rxn = rxn.with_reverse(0.5, 0.0);
        assert!((rxn.net_rate(&c, 300.0) - 5.5).abs() < 1e-12);
    }

    /// **Methodology.** `K_eq(T)` variants are checked against closed forms:
    /// `Constant` is temperature-flat; van 't Hoff satisfies
    /// `ln K = −ΔH/(R T) + ΔS/R` and (for endothermic `ΔH > 0`) rises with `T`;
    /// `LnPolynomial` matches `exp(a + b/T + c·ln T + d·T)`.
    ///
    /// **Measured result (2026-08-03).** `Constant(3.5)` returns `3.5` at 300 K
    /// and 900 K. Van 't Hoff with `ΔH = 40 000`, `ΔS = 30`: `K(1000) > K(500)`
    /// and `ln K(1000) = −40000/(8.314·1000) + 30/8.314` to `< 1e−12`.
    /// `LnPolynomial{a=1,b=−1000,c=0,d=0}` at 500 K = `exp(1 − 2) = 0.3679`.
    #[test]
    fn equilibrium_constant_forms() {
        let kc = EquilibriumConstant::Constant(3.5);
        assert_eq!(kc.evaluate(300.0), 3.5);
        assert_eq!(kc.evaluate(900.0), 3.5);

        let kv = EquilibriumConstant::GibbsVantHoff {
            delta_h: 40_000.0,
            delta_s: 30.0,
        };
        assert!(kv.evaluate(1000.0) > kv.evaluate(500.0));
        let ln_expected = -40_000.0 / (R_GAS * 1000.0) + 30.0 / R_GAS;
        assert!((kv.evaluate(1000.0).ln() - ln_expected).abs() < 1e-12);

        let kp = EquilibriumConstant::LnPolynomial {
            a: 1.0,
            b: -1000.0,
            c: 0.0,
            d: 0.0,
        };
        assert!((kp.evaluate(500.0) - (1.0f64 - 2.0).exp()).abs() < 1e-12);
    }
}
