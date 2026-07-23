//! Microbially-mediated (biodegradation) kinetic reactions — bead op-v6s.15.4.
//!
//! Adds **microbial** kinetic reactions to the reactive-transport stack: an
//! electron **donor** is oxidised using an electron **acceptor**, the reaction
//! catalysed by living **biomass** `B`, at a rate governed by a **Monod**
//! (dual-Monod) saturation law rather than by mass-action equilibrium. This is
//! the rate model used by PFLOTRAN's `MICROBIAL_REACTION` block and the
//! standard textbook description of substrate-limited microbial growth.
//!
//! # Physical model
//!
//! ## Monod / dual-Monod rate law
//!
//! A single microbial reaction `r` degrades a donor species `d` with an
//! acceptor species `a`, mediated by a biomass pool `B_r`. Its instantaneous
//! rate (mol of reaction turnover per litre per second) is
//!
//! ```text
//! R_r = k_max,r * B_r * [ S_d / (K_d + S_d) ] * [ S_a / (K_a + S_a) ]
//!                     * prod_j [ I_j / (I_j + C_j) ]
//! ```
//!
//! where
//!
//! - `k_max,r` is the maximum specific reaction rate, `1/s` (per unit biomass);
//! - `B_r` is the biomass concentration of the pool driving reaction `r`
//!   (mol/L, or any biomass unit consistent with `k_max` and `yield`);
//! - `S_d = C[donor]` and `S_a = C[acceptor]` are the donor / acceptor aqueous
//!   concentrations (mol/L);
//! - `K_d`, `K_a` are the donor / acceptor **half-saturation** constants
//!   (mol/L) — the substrate concentration at which the corresponding Monod
//!   factor reaches `1/2`;
//! - each optional **inhibition** factor `I_j / (I_j + C_j)` (non-competitive /
//!   Haldane-type inhibition) suppresses the rate as the concentration `C_j` of
//!   inhibitor species `j` rises above its inhibition constant `I_j` (mol/L).
//!
//! Each Monod factor `S / (K + S)` runs monotonically from `0` (at `S = 0`) to
//! `1` (as `S >> K`), so the product is bounded in `[0, 1]` and the rate is
//! bounded by `k_max * B`. The **dual** structure means the reaction is limited
//! by whichever substrate is scarce relative to its half-saturation constant.
//!
//! ## Effect on the state
//!
//! Aqueous species change by the reaction stoichiometry `nu_r[i]` (mol of
//! species `i` per mol of reaction turnover; consumed species carry `nu < 0`,
//! produced species `nu > 0`); biomass grows in proportion to turnover and
//! decays by first-order endogenous respiration:
//!
//! ```text
//! dC_i/dt = sum_r nu_r[i] * R_r          (aqueous species i)
//! dB_r/dt = yield_r * R_r - k_decay,r * B_r   (biomass pool r)
//! ```
//!
//! with `yield_r` the biomass yield (biomass produced per unit reaction
//! turnover) and `k_decay,r` the first-order biomass decay constant (`1/s`).
//! When substrate is ample the growth term `yield * R` dominates and biomass
//! rises; when substrate is exhausted `R -> 0` and biomass decays exponentially
//! as `B_r(t) = B_r(0) * exp(-k_decay,r * t)`.
//!
//! The aqueous-species update is stoichiometrically exact: for a single
//! reaction, `Delta C_i / nu_i` is the same integrated turnover for every
//! species `i`, so elemental balances implied by `nu` are conserved to solver
//! precision.
//!
//! # Numerics — coupled ODE over a timestep
//!
//! [`MicrobialSystem::react`] integrates the coupled state
//! `y = [C_0 .. C_{Ns-1}, B_0 .. B_{Nr-1}]` over a macro-timestep `dt` with the
//! foam-basic-lib **RKF45** adaptive explicit Runge-Kutta-Fehlberg 4(5) solver
//! ([`outram_foam_basic_lib::ode::Rkf45`]), the same integrator the mineral
//! [`crate::kinetics`] module uses. The right-hand side is closed-form
//! arithmetic (no inner equilibrium solve), so an explicit adaptive method is
//! appropriate; its controller shrinks the step automatically when fast growth
//! or decay stiffens the system. Concentrations and biomass are floored to
//! zero inside each Monod / inhibition factor so that a transient adaptive-stage
//! extrapolation slightly below zero cannot produce a spurious negative or
//! non-finite rate.
//!
//! # Simplifications (flagged for human review)
//!
//! - **One biomass pool per reaction.** Each reaction has its own scalar
//!   biomass concentration; there is no shared microbial community, no
//!   competition between reactions for a common biomass, and no mobile-vs-
//!   attached partitioning.
//! - **No thermodynamic factor.** The rate has no `(1 - Q/K)` /
//!   `(1 - exp(Delta G / RT))` far-from-equilibrium term — turnover does not
//!   stop when the reaction approaches thermodynamic equilibrium. Only substrate
//!   availability and inhibition limit the rate.
//! - **No pH, temperature, or nutrient dependence** beyond the explicit donor,
//!   acceptor, and inhibitor Monod factors listed on the reaction.
//! - **Ideal (concentration = activity)** aqueous phase, matching the rest of
//!   this crate's geochemistry.
//!
//! Per the workspace `RESPONSIBLE_USE.md`, this is untrusted AI-generated draft
//! material until a human reviews it; no validation against a published
//! biodegradation benchmark has been performed.
//!
//! # Provenance
//!
//! - J. Monod, "The growth of bacterial cultures", *Annual Review of
//!   Microbiology* **3**, 371–394 (1949) — the single-substrate saturation
//!   rate law `R = R_max * S / (K + S)`.
//! - The dual-Monod (multiplicative donor × acceptor) extension and the
//!   non-competitive inhibition factor `I / (I + C)` are the standard forms used
//!   in subsurface biodegradation reactive-transport codes.
//! - PFLOTRAN `MICROBIAL_REACTION` (Monod / inhibition / biomass) — the
//!   reference implementation this module mirrors. See P. C. Lichtner et al.,
//!   *PFLOTRAN User Manual*.

use std::sync::Arc;

use crate::error::PflotranError;
use outram_foam_basic_lib::ode::{OdeSystem, Rkf45};

/// Absolute per-equation error tolerance for the RKF45 integration.
const ODE_ABS_TOL: f64 = 1.0e-14;

/// Relative per-equation error tolerance for the RKF45 integration.
const ODE_REL_TOL: f64 = 1.0e-8;

/// Upper bound on the number of accepted macro-steps in one
/// [`MicrobialSystem::react`] call, guarding against a pathological (e.g.
/// extremely stiff) request that would otherwise loop indefinitely.
const MAX_MACRO_STEPS: usize = 1_000_000;

/// A single Monod (dual-Monod) microbial reaction with optional inhibition and
/// a biomass catalyst.
///
/// The reaction degrades the electron [`donor`](Self::donor) species using the
/// electron [`acceptor`](Self::acceptor) species, at the rate
///
/// ```text
/// R = k_max * B * [S_d/(K_d + S_d)] * [S_a/(K_a + S_a)] * prod_j I_j/(I_j + C_j)
/// ```
///
/// (see the [module header](crate::microbial) for the full physics and units).
/// Species indices refer to positions in the host [`MicrobialSystem`]'s aqueous
/// species vector.
#[derive(Debug, Clone)]
pub struct MonodReaction {
    /// Human-facing reaction label (e.g. `"aerobic acetate oxidation"`).
    pub name: String,
    /// Index of the electron-donor species, `0 <= donor < n_species`.
    pub donor: usize,
    /// Index of the electron-acceptor species, `0 <= acceptor < n_species`.
    pub acceptor: usize,
    /// Maximum specific reaction rate `k_max`, `1/s` (turnover per unit
    /// biomass). Must be finite and non-negative.
    pub k_max: f64,
    /// Donor half-saturation constant `K_d`, mol/L. Must be finite and strictly
    /// positive (it appears in the denominator `K_d + S_d`).
    pub half_sat_donor: f64,
    /// Acceptor half-saturation constant `K_a`, mol/L. Must be finite and
    /// strictly positive.
    pub half_sat_acceptor: f64,
    /// Reaction stoichiometry `nu_i` per aqueous species (length
    /// `n_species`): mol of species `i` per mol of reaction turnover. Consumed
    /// species carry `nu_i < 0`, produced species `nu_i > 0`. Every entry must
    /// be finite.
    pub stoichiometry: Vec<f64>,
    /// Optional non-competitive inhibition terms `(species index j,
    /// inhibition constant I_j)`. Each contributes a factor
    /// `I_j / (I_j + C_j)` to the rate, suppressing it as inhibitor `j`
    /// accumulates. Each `I_j` must be finite and strictly positive; each index
    /// must be a valid species.
    pub inhibition: Vec<(usize, f64)>,
    /// Biomass yield: biomass produced per unit reaction turnover (`dB/dt`
    /// growth term is `yield * R`). Must be finite. Typically non-negative;
    /// a negative yield is unusual but not forbidden.
    pub biomass_yield: f64,
    /// First-order biomass decay constant `k_decay`, `1/s` (endogenous
    /// respiration). Must be finite and non-negative.
    pub biomass_decay: f64,
}

/// A microbial reaction system: a set of [`MonodReaction`]s acting on `Ns`
/// aqueous species plus one biomass pool per reaction.
///
/// Build with [`MicrobialSystem::new`] (which validates every reaction's
/// indices, stoichiometry length, and parameter positivity) and advance a
/// state forward with [`MicrobialSystem::react`]. The instantaneous rate of any
/// reaction at a given state is available from [`MicrobialSystem::rate`].
///
/// The reaction set is shared via [`Arc`] (workspace rule: no lifetime
/// parameters, no `Box`) so the per-`react` ODE system can hold a cheap handle
/// without borrowing `self`.
#[derive(Debug, Clone)]
pub struct MicrobialSystem {
    /// The microbial reactions, length `Nr`. One biomass pool drives each.
    reactions: Arc<Vec<MonodReaction>>,
    /// Number of aqueous species `Ns` every reaction's stoichiometry spans.
    n_species: usize,
}

impl MicrobialSystem {
    /// Build a microbial system from an aqueous species count and a set of
    /// Monod reactions, validating every reaction.
    ///
    /// # Parameters
    ///
    /// - `n_species`: the number of aqueous species `Ns` the reactions act on.
    /// - `reactions`: the microbial reactions. May be empty (an inert system
    ///   whose [`MicrobialSystem::react`] is a no-op).
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if, for any reaction:
    /// `donor` or `acceptor` is out of range; `stoichiometry` does not have
    /// exactly `Ns` entries or contains a non-finite value; an inhibition
    /// species index is out of range; `k_max` or `biomass_decay` is non-finite
    /// or negative; `half_sat_donor`, `half_sat_acceptor`, or any inhibition
    /// constant `I_j` is non-finite or not strictly positive; or `biomass_yield`
    /// is non-finite.
    pub fn new(
        n_species: usize,
        reactions: Vec<MonodReaction>,
    ) -> Result<Self, PflotranError> {
        for (r, rx) in reactions.iter().enumerate() {
            if rx.donor >= n_species {
                return Err(PflotranError::InvalidInput(format!(
                    "reaction {r} (\"{}\") donor index {} out of range (n_species = {n_species})",
                    rx.name, rx.donor
                )));
            }
            if rx.acceptor >= n_species {
                return Err(PflotranError::InvalidInput(format!(
                    "reaction {r} (\"{}\") acceptor index {} out of range (n_species = {n_species})",
                    rx.name, rx.acceptor
                )));
            }
            if rx.stoichiometry.len() != n_species {
                return Err(PflotranError::InvalidInput(format!(
                    "reaction {r} (\"{}\") has {} stoichiometry entries but there are {n_species} species",
                    rx.name,
                    rx.stoichiometry.len()
                )));
            }
            for (i, v) in rx.stoichiometry.iter().enumerate() {
                if !v.is_finite() {
                    return Err(PflotranError::InvalidInput(format!(
                        "reaction {r} stoichiometry[{i}] is not finite"
                    )));
                }
            }
            if !rx.k_max.is_finite() || rx.k_max < 0.0 {
                return Err(PflotranError::InvalidInput(format!(
                    "reaction {r} k_max = {} must be finite and non-negative",
                    rx.k_max
                )));
            }
            if !rx.half_sat_donor.is_finite() || rx.half_sat_donor <= 0.0 {
                return Err(PflotranError::InvalidInput(format!(
                    "reaction {r} half_sat_donor = {} must be finite and strictly positive",
                    rx.half_sat_donor
                )));
            }
            if !rx.half_sat_acceptor.is_finite() || rx.half_sat_acceptor <= 0.0 {
                return Err(PflotranError::InvalidInput(format!(
                    "reaction {r} half_sat_acceptor = {} must be finite and strictly positive",
                    rx.half_sat_acceptor
                )));
            }
            for (k, (j, ic)) in rx.inhibition.iter().enumerate() {
                if *j >= n_species {
                    return Err(PflotranError::InvalidInput(format!(
                        "reaction {r} inhibition[{k}] species index {j} out of range (n_species = {n_species})"
                    )));
                }
                if !ic.is_finite() || *ic <= 0.0 {
                    return Err(PflotranError::InvalidInput(format!(
                        "reaction {r} inhibition[{k}] constant = {ic} must be finite and strictly positive"
                    )));
                }
            }
            if !rx.biomass_yield.is_finite() {
                return Err(PflotranError::InvalidInput(format!(
                    "reaction {r} biomass_yield is not finite"
                )));
            }
            if !rx.biomass_decay.is_finite() || rx.biomass_decay < 0.0 {
                return Err(PflotranError::InvalidInput(format!(
                    "reaction {r} biomass_decay = {} must be finite and non-negative",
                    rx.biomass_decay
                )));
            }
        }

        Ok(Self {
            reactions: Arc::new(reactions),
            n_species,
        })
    }

    /// The number of aqueous species `Ns`.
    pub fn n_species(&self) -> usize {
        self.n_species
    }

    /// The number of microbial reactions `Nr` (= the number of biomass pools).
    pub fn n_reactions(&self) -> usize {
        self.reactions.len()
    }

    /// The microbial reactions, in index order.
    pub fn reactions(&self) -> &[MonodReaction] {
        &self.reactions
    }

    /// Instantaneous rate `R_r` (mol/(L·s)) of reaction `reaction` at the given
    /// aqueous concentrations and biomass pools.
    ///
    /// Evaluates the dual-Monod rate law
    /// `R = k_max * B * [S_d/(K_d+S_d)] * [S_a/(K_a+S_a)] * prod_j I_j/(I_j+C_j)`
    /// (see the [module header](crate::microbial)). The driving biomass is
    /// `biomass[reaction]`. Concentrations are floored to zero inside each Monod
    /// and inhibition factor, so a slightly-negative transient input yields a
    /// zero factor rather than a non-finite rate.
    ///
    /// # Parameters
    ///
    /// - `reaction`: reaction index, `0 <= reaction < n_reactions()`.
    /// - `concentrations`: aqueous species concentrations in mol/L, length
    ///   `Ns`.
    /// - `biomass`: biomass pools, length `Nr` (one per reaction).
    ///
    /// # Panics
    ///
    /// Panics if `reaction >= n_reactions()`, `concentrations.len() != Ns`, or
    /// `biomass.len() != Nr` (a programming error, not a physical-input error).
    pub fn rate(&self, reaction: usize, concentrations: &[f64], biomass: &[f64]) -> f64 {
        assert!(
            reaction < self.reactions.len(),
            "reaction index {reaction} out of range (n_reactions = {})",
            self.reactions.len()
        );
        assert_eq!(
            concentrations.len(),
            self.n_species,
            "concentrations has length {} but there are {} species",
            concentrations.len(),
            self.n_species
        );
        assert_eq!(
            biomass.len(),
            self.reactions.len(),
            "biomass has length {} but there are {} reactions",
            biomass.len(),
            self.reactions.len()
        );
        reaction_rate(&self.reactions[reaction], concentrations, biomass[reaction])
    }

    /// Integrate the aqueous concentrations and biomass pools forward by `dt`
    /// seconds under the microbial (Monod) kinetics, updating both slices **in
    /// place**.
    ///
    /// The coupled ODE `y = [concentrations, biomass]` is advanced with the
    /// foam-basic-lib RKF45 adaptive solver. On each right-hand-side evaluation
    /// every reaction's rate `R_r` is formed from the current state and the
    /// derivatives `dC_i/dt = sum_r nu_r[i] R_r` and
    /// `dB_r/dt = yield_r R_r - k_decay,r B_r` are assembled — see the module
    /// header for the full rate law.
    ///
    /// # Parameters
    ///
    /// - `concentrations`: aqueous species concentrations `C_i` in mol/L, length
    ///   `Ns`. Updated in place to the post-reaction concentrations.
    /// - `biomass`: biomass pools `B_r`, length `Nr` (one per reaction). Updated
    ///   in place.
    /// - `dt`: reaction time in seconds, `dt >= 0`. `dt == 0` is a no-op that
    ///   returns `0` steps.
    ///
    /// # Returns
    ///
    /// The number of accepted RKF45 macro-steps taken.
    ///
    /// # Errors
    ///
    /// - [`PflotranError::InvalidInput`] if `concentrations.len() != Ns`,
    ///   `biomass.len() != Nr`, `dt` is negative or non-finite, or any input
    ///   value is non-finite.
    /// - [`PflotranError::Convergence`] if the ODE step size underflows or the
    ///   step budget is exhausted.
    pub fn react(
        &self,
        concentrations: &mut [f64],
        biomass: &mut [f64],
        dt: f64,
    ) -> Result<usize, PflotranError> {
        let ns = self.n_species;
        let nr = self.reactions.len();

        if concentrations.len() != ns {
            return Err(PflotranError::InvalidInput(format!(
                "concentrations has length {} but there are {ns} species",
                concentrations.len()
            )));
        }
        if biomass.len() != nr {
            return Err(PflotranError::InvalidInput(format!(
                "biomass has length {} but there are {nr} reactions",
                biomass.len()
            )));
        }
        if !dt.is_finite() || dt < 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "dt = {dt} must be finite and non-negative"
            )));
        }
        for (i, c) in concentrations.iter().enumerate() {
            if !c.is_finite() {
                return Err(PflotranError::InvalidInput(format!(
                    "concentrations[{i}] = {c} is not finite"
                )));
            }
        }
        for (r, b) in biomass.iter().enumerate() {
            if !b.is_finite() {
                return Err(PflotranError::InvalidInput(format!(
                    "biomass[{r}] = {b} is not finite"
                )));
            }
        }

        // dt == 0 or no reactions: nothing evolves.
        if dt == 0.0 || nr == 0 {
            return Ok(0);
        }

        // Assemble the ODE state y = [concentrations ; biomass].
        let n = ns + nr;
        let mut y = Vec::with_capacity(n);
        y.extend_from_slice(concentrations);
        y.extend_from_slice(biomass);

        let ode = MicrobialOde {
            reactions: Arc::clone(&self.reactions),
            ns,
            nr,
        };

        let mut solver = Rkf45::new(n, ODE_ABS_TOL, ODE_REL_TOL);
        let mut x = 0.0_f64;
        let mut dx = dt; // adaptive controller shrinks this on the first step if needed
        let mut n_steps = 0usize;

        while x < dt {
            let dx_target = (x + dx).min(dt) - x;
            let mut dx_step = dx_target;
            solver
                .solve_step(&ode, &mut x, &mut y, &mut dx_step)
                .map_err(|e| {
                    PflotranError::Convergence(format!("microbial ODE integration failed: {e}"))
                })?;
            dx = dx_step;
            n_steps += 1;
            if n_steps > MAX_MACRO_STEPS {
                return Err(PflotranError::Convergence(format!(
                    "microbial ODE exceeded {MAX_MACRO_STEPS} steps over dt = {dt}"
                )));
            }
        }

        // Write the evolved state back in place.
        concentrations.copy_from_slice(&y[..ns]);
        biomass.copy_from_slice(&y[ns..]);
        Ok(n_steps)
    }
}

/// One Monod saturation factor `S / (K + S)` with `S` floored to zero.
///
/// `K > 0` is guaranteed by the constructor, so the denominator is strictly
/// positive and the result is finite in `[0, 1)`. A non-positive `S` (which can
/// occur transiently during an adaptive RK stage) yields `0`.
#[inline]
fn monod_factor(s: f64, k: f64) -> f64 {
    let s = if s.is_finite() && s > 0.0 { s } else { 0.0 };
    s / (k + s)
}

/// The dual-Monod rate `R` of one reaction at concentrations `c` with driving
/// biomass `b`. Shared by [`MicrobialSystem::rate`] and the ODE right-hand
/// side so the two can never disagree.
///
/// Biomass is floored to zero (a transient RK stage may extrapolate slightly
/// negative). The full law is
/// `R = k_max * B * f(S_d, K_d) * f(S_a, K_a) * prod_j I_j/(I_j + C_j)`.
fn reaction_rate(rx: &MonodReaction, c: &[f64], b: f64) -> f64 {
    let b = if b.is_finite() && b > 0.0 { b } else { 0.0 };
    if b == 0.0 || rx.k_max == 0.0 {
        return 0.0;
    }
    let f_d = monod_factor(c[rx.donor], rx.half_sat_donor);
    let f_a = monod_factor(c[rx.acceptor], rx.half_sat_acceptor);
    let mut r = rx.k_max * b * f_d * f_a;
    for &(j, ic) in rx.inhibition.iter() {
        let cj = if c[j].is_finite() && c[j] > 0.0 { c[j] } else { 0.0 };
        // ic > 0 guaranteed by the constructor, so this factor is finite in (0, 1].
        r *= ic / (ic + cj);
    }
    r
}

/// Internal ODE system wrapping a [`MicrobialSystem`] for one
/// [`MicrobialSystem::react`] integration. Holds an [`Arc`] handle to the
/// reactions (no borrow of the caller); the right-hand side is closed-form
/// arithmetic and cannot fail, so no deferred-error scratch is needed.
struct MicrobialOde {
    reactions: Arc<Vec<MonodReaction>>,
    ns: usize,
    nr: usize,
}

impl OdeSystem for MicrobialOde {
    fn n_eqns(&self) -> usize {
        self.ns + self.nr
    }

    fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        let ns = self.ns;
        let nr = self.nr;

        let concentrations = &y[..ns];

        // Rate of each reaction, driven by its own biomass pool y[ns + r].
        let mut rate = vec![0.0_f64; nr];
        for r in 0..nr {
            rate[r] = reaction_rate(&self.reactions[r], concentrations, y[ns + r]);
        }

        // dC_i/dt = sum_r nu_r[i] * R_r.
        for i in 0..ns {
            let mut acc = 0.0;
            for r in 0..nr {
                acc += self.reactions[r].stoichiometry[i] * rate[r];
            }
            dydx[i] = acc;
        }
        // dB_r/dt = yield_r * R_r - k_decay,r * B_r.
        for r in 0..nr {
            let rx = &self.reactions[r];
            dydx[ns + r] = rx.biomass_yield * rate[r] - rx.biomass_decay * y[ns + r];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two species `{donor, acceptor}` (indices 0, 1), single reaction
    /// `donor + acceptor -> (nothing)` with `nu = [-1, -1]`, unit `k_max`, and
    /// the given half-saturation constants. No inhibition, no biomass growth/
    /// decay unless overridden by the caller after construction.
    fn one_reaction(k_d: f64, k_a: f64) -> MonodReaction {
        MonodReaction {
            name: "test".into(),
            donor: 0,
            acceptor: 1,
            k_max: 1.0,
            half_sat_donor: k_d,
            half_sat_acceptor: k_a,
            stoichiometry: vec![-1.0, -1.0],
            inhibition: vec![],
            biomass_yield: 0.0,
            biomass_decay: 0.0,
        }
    }

    /// **Monod saturation limits.** As substrate `>> K` the single-substrate
    /// factor saturates so `R -> k_max * B`; as substrate `-> 0` the factor and
    /// hence `R -> 0`.
    #[test]
    fn monod_saturation_and_starvation() {
        // Acceptor held hugely abundant so its factor is ~1; vary the donor.
        let rx = one_reaction(1.0e-3, 1.0e-9);
        let sys = MicrobialSystem::new(2, vec![rx]).unwrap();
        let biomass = vec![2.0];

        // Donor >> K_d (10 vs 1e-3): R -> k_max * B = 1 * 2 = 2.
        let sat = sys.rate(0, &[10.0, 1.0], &biomass);
        assert!(
            (sat - 2.0).abs() < 1e-3,
            "saturated rate {sat} should approach k_max*B = 2"
        );

        // Donor -> 0: R -> 0.
        let starved = sys.rate(0, &[1.0e-15, 1.0], &biomass);
        assert!(starved < 1e-6, "starved rate {starved} should approach 0");

        // Exactly at K_d the factor is 1/2, so R = 0.5 * k_max * B = 1.0.
        let half = sys.rate(0, &[1.0e-3, 1.0], &biomass);
        assert!(
            (half - 1.0).abs() < 1e-3,
            "rate at half-saturation {half} should be 0.5*k_max*B = 1"
        );
    }

    /// **Dual-Monod limitation.** The rate is set by whichever of donor /
    /// acceptor is scarce relative to its half-saturation constant.
    #[test]
    fn dual_monod_limited_by_scarce_substrate() {
        let rx = one_reaction(1.0e-3, 1.0e-3);
        let sys = MicrobialSystem::new(2, vec![rx]).unwrap();
        let biomass = vec![1.0];

        // Donor scarce (S_d = K_d/100 -> factor ~ 1/101), acceptor ample (~1).
        let s_scarce = 1.0e-5;
        let donor_limited = sys.rate(0, &[s_scarce, 1.0], &biomass);
        // Expected ~ k_max*B*f_d with the (near-unity) acceptor factor dropped;
        // the acceptor factor is 0.999, so allow ~1% tolerance.
        let expected = 1.0 * 1.0 * (s_scarce / (1.0e-3 + s_scarce)) * 1.0;
        assert!(
            (donor_limited - expected).abs() < 1e-2 * expected,
            "donor-limited rate {donor_limited} vs expected {expected}"
        );

        // Symmetric case: acceptor scarce, donor ample -> same rate.
        let acceptor_limited = sys.rate(0, &[1.0, s_scarce], &biomass);
        assert!(
            (donor_limited - acceptor_limited).abs() < 1e-9,
            "dual-Monod should be symmetric: {donor_limited} vs {acceptor_limited}"
        );

        // With BOTH substrates ample the rate is far larger — scarcity matters.
        let both_ample = sys.rate(0, &[1.0, 1.0], &biomass);
        assert!(
            both_ample > 50.0 * donor_limited,
            "ample rate {both_ample} should dwarf the substrate-limited {donor_limited}"
        );
    }

    /// **Inhibition reduces the rate.** Adding a non-competitive inhibitor whose
    /// concentration is comparable to its inhibition constant cuts the rate; the
    /// factor `I/(I+C)` is `1/2` at `C = I`.
    #[test]
    fn inhibition_reduces_rate() {
        // 3 species: donor, acceptor, inhibitor.
        let rx = MonodReaction {
            name: "inhibited".into(),
            donor: 0,
            acceptor: 1,
            k_max: 1.0,
            half_sat_donor: 1.0e-6,
            half_sat_acceptor: 1.0e-6,
            stoichiometry: vec![-1.0, -1.0, 0.0],
            inhibition: vec![(2, 1.0e-3)], // inhibitor species 2, I = 1e-3
            biomass_yield: 0.0,
            biomass_decay: 0.0,
        };
        let sys = MicrobialSystem::new(3, vec![rx]).unwrap();
        let biomass = vec![1.0];

        // Substrates ample so their factors ~ 1; base rate ~ k_max*B = 1.
        let no_inhibitor = sys.rate(0, &[1.0, 1.0, 0.0], &biomass);
        let with_inhibitor = sys.rate(0, &[1.0, 1.0, 1.0e-3], &biomass);

        assert!(
            with_inhibitor < no_inhibitor,
            "inhibitor should reduce rate: {with_inhibitor} vs {no_inhibitor}"
        );
        // At C = I the inhibition factor is exactly 1/2.
        assert!(
            (with_inhibitor - 0.5 * no_inhibitor).abs() < 1e-3 * no_inhibitor,
            "rate at C=I ({with_inhibitor}) should be half the uninhibited ({no_inhibitor})"
        );
    }

    /// **`react`: substrate consumption, product formation, biomass growth.**
    ///
    /// Methodology: 3 species `{donor(0), acceptor(1), product(2)}`, one
    /// reaction with `nu = [-1, -1, +1]` (one product formed per turnover),
    /// `k_max = 1/s`, small half-saturations so both substrates start ample,
    /// `yield = 1`, `k_decay = 0.1/s`. Start with ample donor / acceptor and a
    /// seed of biomass, integrate a short `dt`.
    ///
    /// Results (checked in-test): donor and acceptor fall, product rises, and
    /// because `yield*k_max = 1 > k_decay = 0.1` while substrate is ample the
    /// biomass grows.
    #[test]
    fn react_consumes_substrate_forms_product_grows_biomass() {
        let rx = MonodReaction {
            name: "growth".into(),
            donor: 0,
            acceptor: 1,
            k_max: 1.0,
            half_sat_donor: 1.0e-4,
            half_sat_acceptor: 1.0e-4,
            stoichiometry: vec![-1.0, -1.0, 1.0],
            inhibition: vec![],
            biomass_yield: 1.0,
            biomass_decay: 0.1,
        };
        let sys = MicrobialSystem::new(3, vec![rx]).unwrap();

        let c0 = [1.0e-2, 1.0e-2, 0.0];
        let b0 = [1.0e-3];
        let mut c = c0.to_vec();
        let mut b = b0.to_vec();

        let steps = sys.react(&mut c, &mut b, 0.1).unwrap();
        assert!(steps > 0, "expected at least one ODE step");

        assert!(c[0] < c0[0], "donor not consumed: {} -> {}", c0[0], c[0]);
        assert!(c[1] < c0[1], "acceptor not consumed: {} -> {}", c0[1], c[1]);
        assert!(c[2] > c0[2], "product not formed: {} -> {}", c0[2], c[2]);
        assert!(b[0] > b0[0], "biomass did not grow: {} -> {}", b0[0], b[0]);
    }

    /// **`react`: biomass decays when substrate is exhausted.**
    ///
    /// With the donor essentially absent (`S_d << K_d`) the rate is ~0, so the
    /// biomass ODE reduces to `dB/dt = -k_decay * B` and biomass decays. Over
    /// `dt` the closed-form solution is `B(dt) = B0 * exp(-k_decay*dt)`.
    #[test]
    fn react_biomass_decays_when_starved() {
        let rx = MonodReaction {
            name: "starving".into(),
            donor: 0,
            acceptor: 1,
            k_max: 1.0,
            half_sat_donor: 1.0e-3,
            half_sat_acceptor: 1.0e-3,
            stoichiometry: vec![-1.0, -1.0],
            inhibition: vec![],
            biomass_yield: 1.0,
            biomass_decay: 0.5,
        };
        let sys = MicrobialSystem::new(2, vec![rx]).unwrap();

        // Donor ~ 0 so turnover is negligible; acceptor ample.
        let mut c = vec![1.0e-15, 1.0e-2];
        let b0 = 1.0;
        let mut b = vec![b0];

        let dt = 2.0;
        sys.react(&mut c, &mut b, dt).unwrap();

        let expected = b0 * (-0.5 * dt).exp(); // ~ 0.3679
        assert!(b[0] < b0, "biomass did not decay: {b0} -> {}", b[0]);
        assert!(
            (b[0] - expected).abs() < 1e-4,
            "decayed biomass {} vs analytic {expected}",
            b[0]
        );
    }

    /// **Stoichiometric conservation.** For a single reaction the integrated
    /// species change satisfies `Delta C_i / nu_i = Delta C_k / nu_k` for every
    /// pair (both equal the integrated turnover). This is structural in the ODE
    /// (`dC_i/dt = nu_i R`) and must hold to solver precision.
    #[test]
    fn stoichiometric_conservation() {
        // nu = [-1, -2, +3]: acceptor consumed twice as fast, product 3x.
        let rx = MonodReaction {
            name: "stoich".into(),
            donor: 0,
            acceptor: 1,
            k_max: 1.0,
            half_sat_donor: 1.0e-4,
            half_sat_acceptor: 1.0e-4,
            stoichiometry: vec![-1.0, -2.0, 3.0],
            inhibition: vec![],
            biomass_yield: 0.0,
            biomass_decay: 0.0,
        };
        let sys = MicrobialSystem::new(3, vec![rx.clone()]).unwrap();

        let c0 = [1.0e-2, 1.0e-2, 0.0];
        let mut c = c0.to_vec();
        let mut b = vec![1.0e-3];

        // Partial step, well short of exhausting a substrate.
        sys.react(&mut c, &mut b, 1.0e-3).unwrap();

        let d0 = c[0] - c0[0];
        let d1 = c[1] - c0[1];
        let d2 = c[2] - c0[2];
        assert!(d0 < 0.0 && d1 < 0.0 && d2 > 0.0, "no reaction happened");

        // turnover = -d0/1 = -d1/2 = d2/3, all equal.
        let t0 = d0 / rx.stoichiometry[0];
        let t1 = d1 / rx.stoichiometry[1];
        let t2 = d2 / rx.stoichiometry[2];
        assert!(
            (t0 - t1).abs() < 1e-12 && (t0 - t2).abs() < 1e-12,
            "turnover mismatch: {t0}, {t1}, {t2}"
        );
    }

    /// **Constructor validation.** Bad indices, wrong stoichiometry length,
    /// non-positive half-saturation, non-positive inhibition constant, and
    /// negative `k_max` are all rejected; a valid system constructs and its
    /// `react` rejects mismatched slice lengths and negative `dt`.
    #[test]
    fn constructor_validation() {
        // donor index out of range
        let mut bad = one_reaction(1e-3, 1e-3);
        bad.donor = 5;
        assert!(MicrobialSystem::new(2, vec![bad]).is_err());

        // stoichiometry length mismatch
        let mut bad = one_reaction(1e-3, 1e-3);
        bad.stoichiometry = vec![-1.0, -1.0, 0.0];
        assert!(MicrobialSystem::new(2, vec![bad]).is_err());

        // non-positive half-saturation
        let mut bad = one_reaction(0.0, 1e-3);
        assert!(MicrobialSystem::new(2, vec![bad.clone()]).is_err());
        bad = one_reaction(-1.0, 1e-3);
        assert!(MicrobialSystem::new(2, vec![bad]).is_err());

        // negative k_max
        let mut bad = one_reaction(1e-3, 1e-3);
        bad.k_max = -1.0;
        assert!(MicrobialSystem::new(2, vec![bad]).is_err());

        // non-positive inhibition constant
        let mut bad = one_reaction(1e-3, 1e-3);
        bad.inhibition = vec![(0, 0.0)];
        assert!(MicrobialSystem::new(2, vec![bad]).is_err());

        // out-of-range inhibition species index
        let mut bad = one_reaction(1e-3, 1e-3);
        bad.inhibition = vec![(9, 1e-3)];
        assert!(MicrobialSystem::new(2, vec![bad]).is_err());

        // negative biomass decay
        let mut bad = one_reaction(1e-3, 1e-3);
        bad.biomass_decay = -0.1;
        assert!(MicrobialSystem::new(2, vec![bad]).is_err());

        // a valid single-reaction system constructs.
        let ok = MicrobialSystem::new(2, vec![one_reaction(1e-3, 1e-3)]).unwrap();
        assert_eq!(ok.n_reactions(), 1);
        assert_eq!(ok.n_species(), 2);

        // bad react inputs.
        let mut c = vec![1e-3, 1e-3];
        let mut b = vec![1.0];
        assert!(ok.react(&mut vec![1e-3], &mut b, 1.0).is_err()); // wrong concentrations len
        assert!(ok.react(&mut c, &mut vec![1.0, 2.0], 1.0).is_err()); // wrong biomass len
        assert!(ok.react(&mut c, &mut b, -1.0).is_err()); // negative dt

        // empty system: react is a no-op returning 0 steps.
        let empty = MicrobialSystem::new(2, vec![]).unwrap();
        let mut c = vec![1e-3, 1e-3];
        let mut b: Vec<f64> = vec![];
        assert_eq!(empty.react(&mut c, &mut b, 1.0).unwrap(), 0);
    }
}
