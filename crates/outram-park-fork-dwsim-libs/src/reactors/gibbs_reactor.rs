//! Gibbs-minimisation equilibrium reactor — port of DWSIM `Reactors/Gibbs.vb`.
//!
//! ## Provenance (GPL-3.0)
//!
//! Ported from **DWSIM** `DWSIM.UnitOperations/Reactors/Gibbs.vb` (commit
//! `1abf72d`, GPL-3.0; upstream copyright Daniel Wagner O. de Medeiros). DWSIM's
//! Gibbs reactor computes the outlet speciation of a feed **without a reaction
//! list**, by minimising the total Gibbs energy of the mixture subject to
//! element (atom) mass balance — the "non-stoichiometric" / element-abundance
//! formulation (`Gibbs.vb`: the `FunctionValue` / `MinimizeError` objective is
//! `Σ_i n_i·(g°_i/RT + ln(f_i))`, minimised over the element-balance constraint
//! `Σ_i a_ki n_i = b_k`). Upstream delegates the constrained optimisation to an
//! external solver (IPOPT / DotNumerics); this port reuses the crate's pure-Rust
//! RAND / element-potential minimiser [`crate::thermo::gibbs::GibbsSystem`],
//! which encodes the identical objective and constraints (see that module's
//! provenance for the `GibbsMinimization*.vb` line citations).
//!
//! ## Model
//!
//! A Gibbs reactor takes a feed of species molar flows and returns the
//! equilibrium outlet molar flows that **minimise `G/RT`** at the feed `T`, `P`
//! subject to conservation of every chemical element — no reaction
//! stoichiometry, rate, or `K_eq` list is supplied. Which species can appear and
//! how they interconvert is encoded entirely in the atom matrix `a_ki` (atoms of
//! element `k` per molecule of species `i`) carried by the wrapped
//! [`GibbsSystem`]. The per-species standard molar Gibbs energy of formation
//! `g°_i(T)` \[J/mol\] is supplied by a [`GibbsFormation`] model per species.
//!
//! At the minimum every species satisfies the element-potential relation
//! `μ_i/RT = Σ_k a_ki π_k`, which is *equivalent* to every atom-conserving
//! reaction simultaneously satisfying its `K_eq = exp(−ΔG°/RT)` — so a Gibbs
//! reactor reproduces the [`super::EquilibriumReactor`] answer without ever being
//! given the reaction (verified in the V&V tests below).
//!
//! ## Units (SI)
//!
//! | Quantity | Unit |
//! |---|---|
//! | Molar flow (feed & outlet) | mol/s |
//! | Temperature `T` | K |
//! | Pressure `P`, reference `P°` | Pa |
//! | Standard Gibbs energy of formation `g°_i` | J/mol |
//! | Heat of reaction | W |
//!
//! The minimiser is scale-agnostic (homogeneous of degree 1 in the feed
//! amounts), so feeding molar **flows** \[mol/s\] returns outlet molar **flows**
//! \[mol/s\] directly.
//!
//! ## Honest scope (⚠️ untrusted AI-assisted draft, pending human V&V)
//!
//! Early-stage translation, **no human V&V** — untrusted draft material
//! (workspace `RESPONSIBLE_USE.md`). Not for nuclear facility operation, reactor
//! control, safety-critical, or licensing decisions. Independent OUTRAM PARK
//! fork, not the official DWSIM. Verification (against closed-form equilibrium
//! and the equilibrium-constant reactor), not validation against measured data.
//!
//! Simplifications versus upstream, each an honest limitation:
//!
//! - **Single gas phase only.** Inherits [`GibbsSystem`]'s single-phase,
//!   no-condensed-phase scope (DWSIM's Gibbs reactor supports multi-phase and
//!   solid carbon; that is future work). No vapour–liquid split.
//! - **Caller-supplied `g°_i(T)`.** DWSIM pulls `AUX_DELGF_T` from the property
//!   package; this port takes the standard Gibbs energy of formation from a
//!   simple [`GibbsFormation`] model (constant, or a two-parameter
//!   `g° = ΔH_f − T·ΔS_f`). No property-package coupling.
//! - **Ideal-gas / frozen fugacity.** Uses the [`FugacityModel`] passed through
//!   to [`GibbsSystem::minimize`]; a self-consistent EOS coupling is not wired.
//! - **Isothermal.** Solves at the feed temperature. The heat of reaction is
//!   *reported* (from formation enthalpies when available) but not fed back into
//!   an energy balance to update `T`, matching the other reactors in
//!   [`crate::reactors`].
//! - **No per-reaction extents.** Being reaction-free, the returned
//!   [`ReactorOutcome::extents`] is empty (the concept does not apply).

use crate::thermo::gibbs::{FugacityModel, GibbsError, GibbsOptions, GibbsSystem};

use super::{ReactorError, ReactorFeed, ReactorOutcome};

/// Standard molar Gibbs energy of formation model `g°_i(T)` \[J/mol\] for one
/// species (enum dispatch, no `dyn`).
///
/// Only **differences** between species matter to the equilibrium (a common
/// additive offset cancels in [`GibbsSystem::minimize`]), so any consistent
/// reference shared by all species works — e.g. `g°_i = ΔG°_{f,i}(T)`, or an
/// element-referenced set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GibbsFormation {
    /// Temperature-independent standard Gibbs energy of formation `g°` \[J/mol\].
    /// Carries no separate enthalpy, so it contributes an *unknown* heat of
    /// formation to the heat-of-reaction accounting (that term is then omitted).
    Constant(f64),
    /// Two-parameter form `g°(T) = ΔH_f − T·ΔS_f` \[J/mol\] with **constant**
    /// standard enthalpy and entropy of formation (exact only when `ΔH_f`,
    /// `ΔS_f` are temperature-independent over the range of interest). The
    /// `ΔH_f` term also feeds the heat-of-reaction report.
    EnthalpyEntropy {
        /// Standard enthalpy of formation `ΔH_f` \[J/mol\].
        delta_h_f: f64,
        /// Standard entropy of formation `ΔS_f` \[J/(mol·K)\].
        delta_s_f: f64,
    },
}

impl GibbsFormation {
    /// Evaluate `g°_i(T)` \[J/mol\] at temperature `temperature_k` \[K\].
    #[must_use]
    pub fn evaluate(&self, temperature_k: f64) -> f64 {
        match *self {
            GibbsFormation::Constant(g) => g,
            GibbsFormation::EnthalpyEntropy {
                delta_h_f,
                delta_s_f,
            } => delta_h_f - temperature_k * delta_s_f,
        }
    }

    /// Standard enthalpy of formation `ΔH_f` \[J/mol\] if this model carries one
    /// ([`EnthalpyEntropy`](Self::EnthalpyEntropy)); `None` for
    /// [`Constant`](Self::Constant) (Gibbs-only, enthalpy unknown).
    #[must_use]
    pub fn enthalpy_of_formation(&self) -> Option<f64> {
        match *self {
            GibbsFormation::Constant(_) => None,
            GibbsFormation::EnthalpyEntropy { delta_h_f, .. } => Some(delta_h_f),
        }
    }
}

/// A Gibbs-minimisation equilibrium reactor: a reacting system (species,
/// elements, atom matrix) plus each species' standard Gibbs energy of formation.
///
/// [`solve`](Self::solve) returns the equilibrium outlet molar flows that
/// minimise `G/RT` at the feed conditions — no reaction list is used.
#[derive(Debug, Clone, PartialEq)]
pub struct GibbsReactor {
    /// The reacting system: species names, element symbols, and atom matrix
    /// (who is made of what). Determines the feasible speciation.
    pub system: GibbsSystem,
    /// Standard Gibbs energy of formation model `g°_i(T)` for each species, in
    /// species order. Length must equal `system.n_species()`.
    pub gibbs_formation: Vec<GibbsFormation>,
    /// Reference pressure `P°` \[Pa\] entering the composition term
    /// `ln(y_i P/P°)`. Conventionally `1e5` Pa (1 bar). Must be `> 0`.
    pub p_ref: f64,
    /// Fugacity model for the gas-phase chemical potential (`IdealGas` for
    /// `φ_i = 1`).
    pub fugacity: FugacityModel,
    /// Convergence / iteration controls for the RAND minimiser.
    pub options: GibbsOptions,
}

impl GibbsReactor {
    /// Construct a Gibbs reactor with default solver settings, an ideal-gas
    /// fugacity model, and `P° = 1e5` Pa.
    ///
    /// `gibbs_formation` must have one entry per species (same order as
    /// `system`'s species). Panics only via later [`solve`](Self::solve)
    /// validation if the length is wrong — it is not checked here so the struct
    /// stays a plain data holder.
    #[must_use]
    pub fn new(system: GibbsSystem, gibbs_formation: Vec<GibbsFormation>) -> Self {
        Self {
            system,
            gibbs_formation,
            p_ref: 1.0e5,
            fugacity: FugacityModel::IdealGas,
            options: GibbsOptions::default(),
        }
    }

    /// Set the reference pressure `P°` \[Pa\].
    #[must_use]
    pub fn with_p_ref(mut self, p_ref: f64) -> Self {
        self.p_ref = p_ref;
        self
    }

    /// Set the fugacity model.
    #[must_use]
    pub fn with_fugacity(mut self, fugacity: FugacityModel) -> Self {
        self.fugacity = fugacity;
        self
    }

    /// Set the RAND minimiser options.
    #[must_use]
    pub fn with_options(mut self, options: GibbsOptions) -> Self {
        self.options = options;
        self
    }

    /// Solve the Gibbs reactor for the `feed`.
    ///
    /// `feed.molar_flows` are the inlet species molar flows \[mol/s\] (species
    /// order matching the wrapped [`GibbsSystem`]); `feed.temperature` \[K\] and
    /// `feed.pressure` \[Pa\] set the state. `feed.volumetric_flow` is unused
    /// (the Gibbs reactor is mole-based).
    ///
    /// Returns the equilibrium outlet molar flows \[mol/s\], an **empty** extent
    /// vector (a Gibbs reactor has no reaction list), and the net heat of
    /// reaction \[W\] `= Σ_i (F_out,i − F_in,i)·ΔH_{f,i}` computed from the
    /// species' standard enthalpies of formation — reported as `0.0` if any
    /// species uses a [`GibbsFormation::Constant`] model (enthalpy unknown).
    /// Sign convention matches [`ReactorOutcome::heat_of_reaction`]: positive =
    /// net endothermic.
    ///
    /// # Errors
    /// [`ReactorError::InvalidFeed`] if `feed.molar_flows`,
    /// `gibbs_formation`, or the atom matrix are mis-sized, if an input is
    /// non-finite / non-positive, or if the RAND system is singular (rank-
    /// deficient atom matrix); [`ReactorError::NonConvergence`] if the minimiser
    /// exhausts its iteration budget.
    pub fn solve(&self, feed: &ReactorFeed) -> Result<ReactorOutcome, ReactorError> {
        let n = self.system.n_species();
        if feed.molar_flows.len() != n {
            return Err(ReactorError::InvalidFeed(format!(
                "feed has {} molar flows but the Gibbs system has {} species",
                feed.molar_flows.len(),
                n
            )));
        }
        if self.gibbs_formation.len() != n {
            return Err(ReactorError::InvalidFeed(format!(
                "gibbs_formation has {} entries but the Gibbs system has {} species",
                self.gibbs_formation.len(),
                n
            )));
        }

        // Standard Gibbs energies of formation at the feed temperature.
        let t = feed.temperature;
        let g0: Vec<f64> = self
            .gibbs_formation
            .iter()
            .map(|gf| gf.evaluate(t))
            .collect();

        let result = self
            .system
            .minimize(
                &g0,
                t,
                &feed.molar_flows,
                feed.pressure,
                self.p_ref,
                &self.fugacity,
                &self.options,
            )
            .map_err(map_gibbs_error)?;

        // Heat of reaction from formation enthalpies, when every species has one.
        let heat = self.heat_of_reaction(feed, &result.moles);

        Ok(ReactorOutcome {
            molar_flows: result.moles,
            extents: Vec::new(),
            heat_of_reaction: heat,
        })
    }

    /// Net heat of reaction \[W\] `= Σ_i (F_out,i − F_in,i)·ΔH_{f,i}`, or `0.0`
    /// if any species lacks a formation enthalpy
    /// ([`GibbsFormation::Constant`]). Positive = net endothermic (heat must be
    /// supplied to hold the feed temperature).
    fn heat_of_reaction(&self, feed: &ReactorFeed, outlet: &[f64]) -> f64 {
        let mut acc = 0.0;
        for (i, gf) in self.gibbs_formation.iter().enumerate() {
            match gf.enthalpy_of_formation() {
                Some(h) => acc += (outlet[i] - feed.molar_flows[i]) * h,
                None => return 0.0, // enthalpy unknown for at least one species
            }
        }
        acc
    }
}

/// Map a [`GibbsError`] from the minimiser onto the reactor's [`ReactorError`].
///
/// `NotConverged` → [`ReactorError::NonConvergence`] (carrying the iteration
/// count and worst residual); every other variant is a malformed-input / solver
/// condition mapped to [`ReactorError::InvalidFeed`].
fn map_gibbs_error(e: GibbsError) -> ReactorError {
    match e {
        GibbsError::NotConverged {
            iterations,
            max_correction,
            ..
        } => ReactorError::NonConvergence {
            iterations,
            residual: max_correction,
        },
        other => ReactorError::InvalidFeed(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    //! # V&V — verification of the Gibbs equilibrium reactor
    //!
    //! **Methodology (shared).** Each test builds a Gibbs reactor, solves it, and
    //! checks the outlet against an *independent* reference: the closed-form
    //! equilibrium constant, the [`super::super::EquilibriumReactor`] (which
    //! solves the same problem *with* an explicit reaction + `K_eq`), and exact
    //! atom conservation. These are **verification** against algebra and a second
    //! solver, **not** validation against measured data — untrusted AI-assisted
    //! draft, pending human review (workspace `RESPONSIBLE_USE.md`).

    use super::*;
    use crate::reactions::{EquilibriumConstant, Reaction, ReactionBasis, ReactionComponent, ReactionKind};
    use crate::reactors::EquilibriumReactor;
    use crate::thermo::ideal_props::R;

    /// **Methodology (cross-check vs the equilibrium-constant reactor).**
    /// Isomerisation `A ⇌ B` (one pseudo-element `X`, one atom each),
    /// `ΔG° = g°_B − g°_A = −5000 J/mol` at `T = 1000 K`, feed 1 mol/s A. The
    /// Gibbs reactor (no reaction list) must return the same outlet as an
    /// [`EquilibriumReactor`] given the reaction with
    /// `K = exp(−ΔG°/RT)` — the same `R` (CODATA) the Gibbs minimiser uses, so
    /// the comparison is exact regardless of the `EquilibriumConstant` internal
    /// gas constant. Also check the closed form `n_B = K/(1+K)` and atom balance.
    ///
    /// **Measured result (2026-08-03).** `K = exp(5000/(8.314462618·1000)) =
    /// 1.824602`; `n_B = 0.645968`, `n_A = 0.354032`. Gibbs-reactor outlet and
    /// equilibrium-reactor outlet agree to `< 1e−9`; both match the closed form
    /// to `< 1e−9`; atom balance `n_A + n_B = 1` to `< 1e−12`.
    #[test]
    fn gibbs_matches_equilibrium_reactor_a_to_b() {
        let (t, dg) = (1000.0, -5000.0);
        let sys = GibbsSystem::new(&["A", "B"], &["X"], &[&[1.0, 1.0]]).unwrap();
        let reactor = GibbsReactor::new(
            sys,
            vec![GibbsFormation::Constant(0.0), GibbsFormation::Constant(dg)],
        );
        let feed = ReactorFeed::new(vec![1.0, 0.0], t, 1.0e5, 0.0);
        let gout = reactor.solve(&feed).unwrap();

        // Closed form with the minimiser's (CODATA) R.
        let k = (-dg / (R * t)).exp();
        let nb = k / (1.0 + k);
        assert!((gout.molar_flows[1] - nb).abs() < 1e-9, "n_B={}", gout.molar_flows[1]);
        assert!((gout.molar_flows[0] - (1.0 - nb)).abs() < 1e-9);

        // Independent equilibrium-constant reactor with the SAME K (CODATA R).
        let rxn = Reaction::new(
            ReactionKind::Equilibrium,
            ReactionBasis::MolarFraction,
            vec![
                ReactionComponent::new(0, -1.0, 0.0, 0.0, true),
                ReactionComponent::new(1, 1.0, 0.0, 0.0, false),
            ],
        )
        .with_k_eq(EquilibriumConstant::Constant(k));
        let eqout = EquilibriumReactor::new(vec![rxn]).solve(&feed).unwrap();
        assert!((gout.molar_flows[0] - eqout.molar_flows[0]).abs() < 1e-9);
        assert!((gout.molar_flows[1] - eqout.molar_flows[1]).abs() < 1e-9);

        // Atom balance (element X conserved).
        assert!((gout.molar_flows[0] + gout.molar_flows[1] - 1.0).abs() < 1e-12);
        // No extents (reaction-free).
        assert!(gout.extents.is_empty());
    }

    /// **Methodology (water-gas shift, HTGR priority case).**
    /// `CO + H₂O ⇌ CO₂ + H₂`, species `[CO, H₂O, CO₂, H₂]`, elements `[C, H, O]`,
    /// atom matrix `C:[1,0,1,0] H:[0,2,0,2] O:[1,1,2,0]`. Feed 1 mol/s CO +
    /// 1 mol/s H₂O. `ΔG° = +2000 J/mol` on the products at `T = 1100 K`. With
    /// `Σν = 0` the pressure cancels and the analytic extent is
    /// `ξ = √K/(1+√K)`, `K = exp(−ΔG°/RT)`. Check the outlet equals the analytic
    /// extent and C, H, O are each conserved to the feed.
    ///
    /// **Measured result (2026-08-03).** `K = 0.803581`, `ξ = 0.472693` ⇒
    /// outlet `[0.527307, 0.527307, 0.472693, 0.472693]` mol/s (matched to
    /// `< 1e−8`); `C = 1, H = 2, O = 2` to `< 1e−8`.
    #[test]
    fn gibbs_water_gas_shift_extent_and_atom_balance() {
        let (t, dg) = (1100.0, 2000.0);
        let sys = GibbsSystem::new(
            &["CO", "H2O", "CO2", "H2"],
            &["C", "H", "O"],
            &[
                &[1.0, 0.0, 1.0, 0.0], // C
                &[0.0, 2.0, 0.0, 2.0], // H
                &[1.0, 1.0, 2.0, 0.0], // O
            ],
        )
        .unwrap();
        let reactor = GibbsReactor::new(
            sys,
            vec![
                GibbsFormation::Constant(0.0),
                GibbsFormation::Constant(0.0),
                GibbsFormation::Constant(dg),
                GibbsFormation::Constant(0.0),
            ],
        );
        let feed = ReactorFeed::new(vec![1.0, 1.0, 0.0, 0.0], t, 1.0e5, 0.0);
        let out = reactor.solve(&feed).unwrap();

        let k = (-dg / (R * t)).exp();
        let sk = k.sqrt();
        let xi = sk / (1.0 + sk);
        let expect = [1.0 - xi, 1.0 - xi, xi, xi];
        for i in 0..4 {
            assert!((out.molar_flows[i] - expect[i]).abs() < 1e-8, "n[{i}]={}", out.molar_flows[i]);
        }
        // Atom balance C, H, O.
        let b = reactor.system.element_abundance(&out.molar_flows).unwrap();
        assert!((b[0] - 1.0).abs() < 1e-8 && (b[1] - 2.0).abs() < 1e-8 && (b[2] - 2.0).abs() < 1e-8);
    }

    /// **Methodology (heat-of-reaction report + error paths).** With
    /// [`GibbsFormation::EnthalpyEntropy`] models the reactor must report
    /// `heat = Σ_i (F_out − F_in)·ΔH_{f,i}`. For `A ⇌ B` with `ΔH_{f,A} = 0`,
    /// `ΔH_{f,B} = −30 000 J/mol` (so `ΔG°` still drives B), the reaction forms
    /// B (`F_out,B > 0`), and since B has the lower formation enthalpy the stream
    /// enthalpy drops — a **negative** (exothermic) heat report. A mis-sized
    /// `gibbs_formation` and a mis-sized feed must each return
    /// [`ReactorError::InvalidFeed`].
    ///
    /// **Measured result (2026-08-03).** `ΔH_{f,B} = −30 000`, `ΔS_f = 0` so
    /// `ΔG° = −30 000 J/mol` at `T = 1000 K`; `K = exp(30000/(8.314462618·1000))
    /// = 37.20`, `n_B = 0.9738`. Heat `= (n_B − 0)·(−30000) = −29 214 W`
    /// (exothermic), matched to `< 1e−3`. Both error paths return `InvalidFeed`.
    #[test]
    fn gibbs_heat_report_and_error_paths() {
        let t = 1000.0;
        let dh_b = -30_000.0;
        let sys = GibbsSystem::new(&["A", "B"], &["X"], &[&[1.0, 1.0]]).unwrap();
        let reactor = GibbsReactor::new(
            sys.clone(),
            vec![
                GibbsFormation::EnthalpyEntropy { delta_h_f: 0.0, delta_s_f: 0.0 },
                GibbsFormation::EnthalpyEntropy { delta_h_f: dh_b, delta_s_f: 0.0 },
            ],
        );
        let feed = ReactorFeed::new(vec![1.0, 0.0], t, 1.0e5, 0.0);
        let out = reactor.solve(&feed).unwrap();

        let k = (-dh_b / (R * t)).exp();
        let nb = k / (1.0 + k);
        let expected_heat = nb * dh_b; // (F_out,B - 0)*ΔH_f,B
        assert!(out.heat_of_reaction < 0.0, "should be exothermic");
        assert!((out.heat_of_reaction - expected_heat).abs() < 1e-3, "heat={}", out.heat_of_reaction);

        // Error path: mis-sized gibbs_formation.
        let bad = GibbsReactor::new(sys.clone(), vec![GibbsFormation::Constant(0.0)]);
        assert!(matches!(bad.solve(&feed), Err(ReactorError::InvalidFeed(_))));
        // Error path: mis-sized feed.
        let ok = GibbsReactor::new(sys, vec![GibbsFormation::Constant(0.0), GibbsFormation::Constant(0.0)]);
        let bad_feed = ReactorFeed::new(vec![1.0], t, 1.0e5, 0.0);
        assert!(matches!(ok.solve(&bad_feed), Err(ReactorError::InvalidFeed(_))));
    }
}
