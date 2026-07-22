//! Kinetic mineral precipitation / dissolution (v1) — bead op-v6s.12.
//!
//! Extends the equilibrium [`crate::geochemistry`] speciation core toward
//! reactive-transport geochemistry (GIRT) by adding *kinetic* mineral phases:
//! solids that dissolve into, or precipitate out of, the aqueous solution at a
//! finite rate governed by **transition-state theory (TST)** rather than
//! instantaneous equilibrium. The aqueous complexation itself is still solved
//! at equilibrium (via [`ReactionNetwork::speciate`]); only the mineral–water
//! exchange is rate-limited.
//!
//! # Physical model
//!
//! A kinetic mineral `M_i` exchanges with the aqueous primary (component)
//! species by the dissolution reaction
//!
//! ```text
//! M_i  <->  sum_j nu[i][j] * primary_j
//! ```
//!
//! where `nu[i][j]` (`stoich[j]` on [`KineticMineral`]) is the number of moles
//! of primary component `j` **released into solution per mole of mineral
//! dissolved**, and the mineral has a base-10 solubility product
//! `log10_ksp[i]`.
//!
//! Given the free primary concentrations `C_prim` obtained from a speciation
//! solve (ideal activities inherited from [`crate::geochemistry`], so activity
//! equals concentration in mol/L), define:
//!
//! - **Ion activity product** `Q_i = prod_j C_prim[j]^{nu[i][j]}`.
//! - **Solubility product** `Ksp_i = 10^{log10_ksp[i]}`.
//! - **Saturation ratio** `Omega_i = Q_i / Ksp_i`.
//! - **Saturation index** `SI_i = log10(Omega_i) = log10(Q_i) - log10_ksp[i]`.
//!
//! The transition-state-theory rate (moles of mineral reacting per unit time)
//! is
//!
//! ```text
//! R_i = k_i * A_i * (1 - Omega_i)
//! ```
//!
//! with `k_i` the rate constant (mol/(m^2 s)) and `A_i` the reactive surface
//! area (m^2). The sign convention:
//!
//! - `Omega_i < 1` (undersaturated) => `R_i > 0` => **dissolution** (mineral
//!   moles fall, aqueous totals rise).
//! - `Omega_i > 1` (supersaturated) => `R_i < 0` => **precipitation** (mineral
//!   moles rise, aqueous totals fall).
//! - `Omega_i = 1` (saturated) => `R_i = 0` (equilibrium; the rate law relaxes
//!   the system toward `Omega -> 1`).
//!
//! The effect on the state is
//!
//! ```text
//! d(total_j)/dt = sum_i nu[i][j] * R_i        (aqueous component totals)
//! d(m_i)/dt     = -R_i                        (mineral moles)
//! ```
//!
//! which conserves mass exactly: every mole released into the totals is a mole
//! removed from a mineral, weighted by `nu`.
//!
//! ## No-mineral clamp (documented behaviour)
//!
//! A mineral that is **absent** (`m_i <= 0`) cannot dissolve — there is no
//! solid to consume. So whenever `m_i <= 0` and the raw TST rate is a
//! *dissolution* rate (`R_i > 0`, i.e. `Omega_i < 1`), the rate is clamped to
//! `0`. Precipitation from an absent phase (`R_i < 0`, `Omega_i > 1`) is still
//! permitted, so a supersaturated solution can nucleate a new mineral from
//! zero. (This is the physically-correct reading of the clamp; a bare "cannot
//! dissolve what isn't there" rule, consistent with the `no_mineral_clamp`
//! test.) No secondary-nucleation kinetics or induction time is modelled —
//! precipitation begins immediately once `Omega > 1`.
//!
//! # Numerics — coupled ODE over a timestep
//!
//! The coupled system for the state vector
//! `y = [total_0 .. total_{Nc-1}, m_0 .. m_{Nm-1}]` is integrated over a
//! macro-timestep `dt` with the foam-basic-lib **RKF45** adaptive explicit
//! Runge-Kutta-Fehlberg 4(5) solver ([`outram_foam_basic_lib::ode::Rkf45`]).
//!
//! **Why RKF45 (explicit) and not Rosenbrock23 (stiff):** the right-hand side
//! is evaluated by running a full Newton **speciation** each call, so the
//! Jacobian `d(dy/dt)/dy` that a Rosenbrock/W-method needs would have to
//! differentiate through the implicit speciation solve — no closed form is
//! available, and a finite-difference Jacobian would cost `Nc+1` speciations
//! per Jacobian. RKF45 needs no Jacobian and its adaptive controller shrinks
//! the step automatically when the kinetics stiffen near equilibrium. For
//! very fast rate constants (relaxation time `<< dt`) a stiff integrator with
//! a hand-coded or frozen-activity Jacobian would be more efficient; that is a
//! documented follow-up, not needed for the moderate rates modelled here.
//!
//! **Live vs frozen activities:** this implementation **re-speciates on every
//! RHS evaluation** (live activities), warm-started from the previous free
//! primary vector. This is the accurate choice — the ion activity product
//! `Q_i` tracks the true aqueous state as it evolves — at the cost of one
//! Newton speciation per RHS call. A cheaper *frozen-activity* alternative
//! (speciate once per macro-step and hold `C_prim` fixed inside the ODE) is a
//! valid simplification but is **not** used here.
//!
//! # Simplifications (flagged for human review)
//!
//! - **Ideal activities** (`gamma = 1`) inherited from
//!   [`crate::geochemistry`]; no Debye–Hückel / Davies correction.
//! - **Single lumped surface-area model** — `A_i` is a constant per mineral; no
//!   evolving grain-size / specific-surface-area or reactive-site model, and no
//!   dependence of `A_i` on the current mineral amount.
//! - **No nucleation kinetics** — precipitation is governed by the same TST law
//!   with no induction time or critical supersaturation.
//! - **No pH- or catalysis-dependent rate laws, no far-from-equilibrium
//!   (`|1 - Omega|^n`) exponents** — the affinity term is the linear
//!   `(1 - Omega)`.
//!
//! Per the workspace `RESPONSIBLE_USE.md`, this is untrusted AI-generated draft
//! material until a human reviews it; no validation against published
//! reactive-transport benchmarks has been performed.

use std::cell::RefCell;
use std::sync::Arc;

use crate::error::PflotranError;
use crate::geochemistry::ReactionNetwork;
use outram_foam_basic_lib::ode::{OdeSystem, Rkf45};

/// Absolute per-equation error tolerance for the RKF45 integration.
const ODE_ABS_TOL: f64 = 1.0e-14;

/// Relative per-equation error tolerance for the RKF45 integration.
const ODE_REL_TOL: f64 = 1.0e-8;

/// Positive floor applied to a component total before it is handed to the
/// equilibrium speciation solve, which requires strictly-positive totals. An
/// adaptive RK stage can transiently extrapolate a total to (or below) zero;
/// flooring keeps the speciation well-posed without treating that transient as
/// a fatal error. `1e-30 mol/L` is far below any physically meaningful
/// concentration.
const TOTAL_FLOOR: f64 = 1.0e-30;

/// Upper bound on the number of accepted macro-steps in one [`KineticSystem::react`]
/// call, guarding against a pathological (e.g. extremely stiff) request that
/// would otherwise loop indefinitely.
const MAX_MACRO_STEPS: usize = 1_000_000;

/// A kinetic mineral: its dissolution stoichiometry into the aqueous primary
/// components, its solubility product, its rate constant, and its reactive
/// surface area.
///
/// The dissolution reaction is `M <-> sum_j stoich[j] * primary_j`, so
/// `stoich[j]` is the moles of primary component `j` released per mole of
/// mineral dissolved (length `Nc`, the number of primary species in the
/// associated [`ReactionNetwork`]). Values may be non-integer; a negative entry
/// would mean a component is *consumed* on dissolution, which is unusual but
/// not forbidden.
#[derive(Debug, Clone)]
pub struct KineticMineral {
    /// Human-facing mineral label (e.g. `"Calcite"`).
    pub name: String,
    /// Dissolution stoichiometry, length `Nc`: moles of primary `j` released
    /// per mole of mineral dissolved.
    pub stoich: Vec<f64>,
    /// Base-10 solubility product, `log10(Ksp)` (dimensionless, ideal
    /// activities). `Ksp = 10^{log10_ksp}`.
    pub log10_ksp: f64,
    /// Rate constant `k`, mol/(m^2 s). Must be finite and non-negative.
    pub rate_constant: f64,
    /// Reactive surface area `A`, m^2. Must be finite and non-negative.
    pub surface_area: f64,
}

/// A set of kinetic minerals reacting with an aqueous [`ReactionNetwork`].
///
/// Build with [`KineticSystem::new`] (which validates every mineral's
/// stoichiometry length against the network's primary-species count) and drive
/// a state forward in time with [`KineticSystem::react`]. The saturation index
/// of any mineral at a given free-primary state is available from
/// [`KineticSystem::saturation_index`].
///
/// The network and mineral set are shared via [`Arc`] (workspace rule: no
/// lifetime parameters, no `Box`), so the per-`react` ODE system can hold cheap
/// handles to them without borrowing `self`.
#[derive(Debug, Clone)]
pub struct KineticSystem {
    /// Equilibrium aqueous network supplying free primary concentrations.
    network: Arc<ReactionNetwork>,
    /// Kinetic mineral phases, length `Nm`.
    minerals: Arc<Vec<KineticMineral>>,
    /// Cached number of primary components `Nc = network.n_primary()`.
    n_primary: usize,
}

impl KineticSystem {
    /// Build a kinetic system from an aqueous reaction network and a set of
    /// kinetic minerals, validating each mineral against the network.
    ///
    /// # Parameters
    ///
    /// - `network`: the equilibrium [`ReactionNetwork`] whose `Nc` primary
    ///   components the minerals exchange with.
    /// - `minerals`: the kinetic mineral phases. May be empty (an inert system
    ///   whose [`KineticSystem::react`] is a no-op).
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if any mineral's `stoich` does
    /// not have exactly `Nc` entries, or if any `stoich` value, `log10_ksp`,
    /// `rate_constant`, or `surface_area` is non-finite, or if `rate_constant`
    /// or `surface_area` is negative.
    pub fn new(
        network: ReactionNetwork,
        minerals: Vec<KineticMineral>,
    ) -> Result<Self, PflotranError> {
        let n_primary = network.n_primary();

        for (i, m) in minerals.iter().enumerate() {
            if m.stoich.len() != n_primary {
                return Err(PflotranError::InvalidInput(format!(
                    "mineral {} (\"{}\") has {} stoichiometry entries but there are {} primary species",
                    i,
                    m.name,
                    m.stoich.len(),
                    n_primary
                )));
            }
            for (j, v) in m.stoich.iter().enumerate() {
                if !v.is_finite() {
                    return Err(PflotranError::InvalidInput(format!(
                        "mineral {i} stoichiometry[{j}] is not finite"
                    )));
                }
            }
            if !m.log10_ksp.is_finite() {
                return Err(PflotranError::InvalidInput(format!(
                    "mineral {i} log10_ksp is not finite"
                )));
            }
            if !m.rate_constant.is_finite() || m.rate_constant < 0.0 {
                return Err(PflotranError::InvalidInput(format!(
                    "mineral {i} rate_constant = {} must be finite and non-negative",
                    m.rate_constant
                )));
            }
            if !m.surface_area.is_finite() || m.surface_area < 0.0 {
                return Err(PflotranError::InvalidInput(format!(
                    "mineral {i} surface_area = {} must be finite and non-negative",
                    m.surface_area
                )));
            }
        }

        Ok(Self {
            network: Arc::new(network),
            minerals: Arc::new(minerals),
            n_primary,
        })
    }

    /// The number of kinetic minerals, `Nm`.
    pub fn n_minerals(&self) -> usize {
        self.minerals.len()
    }

    /// The number of aqueous primary components, `Nc`.
    pub fn n_primary(&self) -> usize {
        self.n_primary
    }

    /// The underlying equilibrium aqueous reaction network.
    pub fn network(&self) -> &ReactionNetwork {
        &self.network
    }

    /// The kinetic mineral phases, in index order.
    pub fn minerals(&self) -> &[KineticMineral] {
        &self.minerals
    }

    /// Saturation index `SI = log10(Q / Ksp)` of mineral `mineral` at the given
    /// free primary concentrations.
    ///
    /// Computed in log space as `SI = sum_j nu[mineral][j] * log10(primary[j]) -
    /// log10_ksp[mineral]`, which is numerically better-behaved than forming the
    /// product `Q` directly. `SI < 0` is undersaturated (dissolution favoured),
    /// `SI > 0` supersaturated (precipitation favoured), `SI = 0` at
    /// equilibrium.
    ///
    /// # Parameters
    ///
    /// - `mineral`: index of the mineral, `0 <= mineral < n_minerals()`.
    /// - `primary`: free primary (component) concentrations in mol/L, length
    ///   `Nc`, all strictly positive.
    ///
    /// # Panics
    ///
    /// Panics if `mineral >= n_minerals()` or if `primary.len() != Nc` (a
    /// programming error, not a physical-input error). A non-positive
    /// `primary[j]` yields a non-finite result rather than panicking.
    pub fn saturation_index(&self, mineral: usize, primary: &[f64]) -> f64 {
        assert!(
            mineral < self.minerals.len(),
            "mineral index {mineral} out of range (n_minerals = {})",
            self.minerals.len()
        );
        assert_eq!(
            primary.len(),
            self.n_primary,
            "primary has length {} but there are {} primary species",
            primary.len(),
            self.n_primary
        );
        let m = &self.minerals[mineral];
        let mut log10_q = 0.0;
        for j in 0..self.n_primary {
            log10_q += m.stoich[j] * primary[j].log10();
        }
        log10_q - m.log10_ksp
    }

    /// Integrate the aqueous component totals and mineral amounts forward by
    /// `dt` seconds under transition-state-theory kinetics, updating both slices
    /// **in place**.
    ///
    /// The coupled ODE `y = [totals, mineral_moles]` is advanced with the
    /// foam-basic-lib RKF45 adaptive solver. On each right-hand-side evaluation
    /// the current totals are re-speciated (live activities, warm-started from
    /// the previous free-primary vector) to obtain `C_prim`, from which each
    /// mineral's `Omega_i`, TST rate `R_i` (with the no-mineral clamp), and the
    /// derivatives `d(total)/dt` / `d(m)/dt` are assembled — see the module
    /// header for the full rate law and clamp definition.
    ///
    /// # Parameters
    ///
    /// - `totals`: aqueous component totals `T[j]` in mol/L, length `Nc`.
    ///   Updated in place to the post-reaction totals.
    /// - `mineral_moles`: mineral amounts `m_i` in mol, length `Nm`. Updated in
    ///   place.
    /// - `dt`: reaction time in seconds, `dt >= 0`. `dt == 0` is a no-op that
    ///   returns `0` steps.
    ///
    /// # Returns
    ///
    /// The number of accepted RKF45 macro-steps taken.
    ///
    /// # Errors
    ///
    /// - [`PflotranError::InvalidInput`] if `totals.len() != Nc`,
    ///   `mineral_moles.len() != Nm`, `dt` is negative or non-finite, or any
    ///   input value is non-finite or (for the initial totals) non-positive.
    /// - [`PflotranError::Convergence`] if the initial or an in-loop speciation
    ///   fails to converge, if the ODE step size underflows, or if the step
    ///   budget is exhausted.
    pub fn react(
        &self,
        totals: &mut Vec<f64>,
        mineral_moles: &mut Vec<f64>,
        dt: f64,
    ) -> Result<usize, PflotranError> {
        let nc = self.n_primary;
        let nm = self.minerals.len();

        if totals.len() != nc {
            return Err(PflotranError::InvalidInput(format!(
                "totals has length {} but there are {nc} primary species",
                totals.len()
            )));
        }
        if mineral_moles.len() != nm {
            return Err(PflotranError::InvalidInput(format!(
                "mineral_moles has length {} but there are {nm} minerals",
                mineral_moles.len()
            )));
        }
        if !dt.is_finite() || dt < 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "dt = {dt} must be finite and non-negative"
            )));
        }
        for (j, t) in totals.iter().enumerate() {
            if !t.is_finite() || *t <= 0.0 {
                return Err(PflotranError::InvalidInput(format!(
                    "totals[{j}] = {t} must be finite and strictly positive"
                )));
            }
        }
        for (i, m) in mineral_moles.iter().enumerate() {
            if !m.is_finite() {
                return Err(PflotranError::InvalidInput(format!(
                    "mineral_moles[{i}] = {m} is not finite"
                )));
            }
        }

        // dt == 0 or no minerals: nothing evolves.
        if dt == 0.0 || nm == 0 {
            return Ok(0);
        }

        // Warm-start the RHS speciation from the initial equilibrium state; this
        // also validates that the starting totals speciate at all.
        let initial = self.network.speciate(totals.as_slice(), None)?;

        // Assemble the ODE state y = [totals ; mineral_moles].
        let n = nc + nm;
        let mut y = Vec::with_capacity(n);
        y.extend_from_slice(totals.as_slice());
        y.extend_from_slice(mineral_moles.as_slice());

        let ode = KineticOde {
            network: Arc::clone(&self.network),
            minerals: Arc::clone(&self.minerals),
            nc,
            nm,
            last_primary: RefCell::new(initial.primary),
            error: RefCell::new(None),
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
                    PflotranError::Convergence(format!("kinetic ODE integration failed: {e}"))
                })?;
            // Surface any speciation failure recorded inside the RHS.
            if let Some(err) = ode.error.borrow_mut().take() {
                return Err(err);
            }
            dx = dx_step;
            n_steps += 1;
            if n_steps > MAX_MACRO_STEPS {
                return Err(PflotranError::Convergence(format!(
                    "kinetic ODE exceeded {MAX_MACRO_STEPS} steps over dt = {dt}"
                )));
            }
        }

        // Write the evolved state back in place.
        totals.copy_from_slice(&y[..nc]);
        mineral_moles.copy_from_slice(&y[nc..]);
        Ok(n_steps)
    }
}

/// Internal ODE system wrapping a [`KineticSystem`] for one [`KineticSystem::react`]
/// integration. Holds `Arc` handles (no borrow of the caller) plus interior-mutable
/// scratch for the warm-start primary vector and a deferred speciation error, since
/// [`OdeSystem::derivatives`] takes `&self` and cannot return an error.
struct KineticOde {
    network: Arc<ReactionNetwork>,
    minerals: Arc<Vec<KineticMineral>>,
    nc: usize,
    nm: usize,
    /// Free primary concentrations from the previous RHS speciation, reused as
    /// the Newton warm start for the next one.
    last_primary: RefCell<Vec<f64>>,
    /// First speciation error encountered during the integration, surfaced by
    /// the caller after each step. `None` while healthy.
    error: RefCell<Option<PflotranError>>,
}

impl OdeSystem for KineticOde {
    fn n_eqns(&self) -> usize {
        self.nc + self.nm
    }

    fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        let nc = self.nc;
        let nm = self.nm;

        // Once a speciation error is recorded, freeze the system (zero
        // derivatives) so the outer loop can stop cleanly.
        if self.error.borrow().is_some() {
            for d in dydx.iter_mut() {
                *d = 0.0;
            }
            return;
        }

        // Floor totals to a strictly-positive value for the speciation solve;
        // a transient adaptive-stage extrapolation can dip to/below zero.
        let mut floored = vec![0.0_f64; nc];
        for j in 0..nc {
            let v = y[j];
            floored[j] = if v.is_finite() && v > TOTAL_FLOOR {
                v
            } else {
                TOTAL_FLOOR
            };
        }

        let warm = self.last_primary.borrow().clone();
        let spec = match self.network.speciate(floored.as_slice(), Some(warm.as_slice())) {
            Ok(s) => s,
            Err(e) => {
                *self.error.borrow_mut() = Some(e);
                for d in dydx.iter_mut() {
                    *d = 0.0;
                }
                return;
            }
        };
        let c_prim = &spec.primary;
        *self.last_primary.borrow_mut() = c_prim.clone();

        // TST rate for each mineral, with the no-mineral clamp.
        let mut rate = vec![0.0_f64; nm];
        for i in 0..nm {
            let m = &self.minerals[i];
            let mut log10_q = 0.0;
            for j in 0..nc {
                log10_q += m.stoich[j] * c_prim[j].log10();
            }
            let si = log10_q - m.log10_ksp;
            let omega = 10f64.powf(si);
            let mut r = m.rate_constant * m.surface_area * (1.0 - omega);
            // Absent mineral cannot dissolve: clamp a dissolution rate to zero.
            if y[nc + i] <= 0.0 && r > 0.0 {
                r = 0.0;
            }
            rate[i] = r;
        }

        // d(total_j)/dt = sum_i nu[i][j] * R_i.
        for j in 0..nc {
            let mut acc = 0.0;
            for i in 0..nm {
                acc += self.minerals[i].stoich[j] * rate[i];
            }
            dydx[j] = acc;
        }
        // d(m_i)/dt = -R_i.
        for i in 0..nm {
            dydx[nc + i] = -rate[i];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2-primary network `{A, B}` with **no** secondary species, so a
    /// speciation returns the totals unchanged (free primary == total). This
    /// isolates the mineral kinetics from aqueous complexation.
    fn ab_network() -> ReactionNetwork {
        ReactionNetwork::new(vec!["A".into(), "B".into()], vec![], vec![], vec![]).unwrap()
    }

    /// Single mineral `M <-> A + B`, `stoich = [1, 1]`, with the given
    /// solubility product and unit rate constant / surface area.
    fn ab_mineral(log10_ksp: f64) -> KineticMineral {
        KineticMineral {
            name: "AB".into(),
            stoich: vec![1.0, 1.0],
            log10_ksp,
            rate_constant: 1.0,
            surface_area: 1.0,
        }
    }

    /// **Approach to equilibrium.**
    ///
    /// Methodology: one mineral `M <-> A + B`, `Ksp = 1e-6` (`log10_ksp = -6`).
    /// Start undersaturated with `T_A = T_B = 1e-4` mol/L, so
    /// `Q = 1e-8`, `Omega = 1e-2`, `SI = -2`. With `m0 = 1.0` mol of solid
    /// available, `react` should dissolve the mineral, driving the totals up so
    /// the saturation index climbs toward 0 and `Omega -> 1`. The symmetric
    /// equilibrium is `C_A = C_B = sqrt(Ksp) = 1e-3` mol/L.
    ///
    /// Results (checked in-test): after `dt = 1.0 s` the saturation index is
    /// within `1e-4` of 0 (i.e. `|Omega - 1| < ~1e-3`), and its magnitude has
    /// strictly decreased from the initial `|SI| = 2`.
    #[test]
    fn approach_to_equilibrium() {
        let sys = KineticSystem::new(ab_network(), vec![ab_mineral(-6.0)]).unwrap();

        let mut totals = vec![1.0e-4, 1.0e-4];
        let mut minerals = vec![1.0];

        // Ns = 0, so free primary == totals.
        let si0 = sys.saturation_index(0, &totals);
        assert!((si0 - (-2.0)).abs() < 1e-9, "initial SI = {si0}, expected -2");

        let steps = sys.react(&mut totals, &mut minerals, 1.0).unwrap();
        assert!(steps > 0, "expected at least one ODE step");

        let si1 = sys.saturation_index(0, &totals);
        assert!(
            si1.abs() < 1e-4,
            "final SI = {si1} not within 1e-4 of equilibrium"
        );
        assert!(
            si1.abs() < si0.abs(),
            "SI magnitude did not decrease: {} -> {}",
            si0.abs(),
            si1.abs()
        );
        // Totals moved toward the equilibrium value 1e-3.
        assert!(
            (totals[0] - 1.0e-3).abs() < 1e-5 && (totals[1] - 1.0e-3).abs() < 1e-5,
            "totals {totals:?} did not approach 1e-3"
        );
        // Some mineral was consumed but not all.
        assert!(minerals[0] < 1.0 && minerals[0] > 0.0, "m = {}", minerals[0]);
    }

    /// **Elemental / mole conservation.**
    ///
    /// Methodology: a 3-primary network `{A, B, C}` (no secondaries) with two
    /// minerals sharing component `B` — `M1 <-> A + B` and `M2 <-> B + C`, both
    /// undersaturated so both dissolve. Integrate a *partial* step
    /// (`dt = 1e-2 s`, well short of equilibrium) and check the conservation
    /// identity `Δtotal_j == sum_i nu[i][j] * (-Δm_i)` for every component.
    ///
    /// Results (checked in-test): every component balances to `< 1e-12` (the
    /// identity is structural in the ODE — `d(total)/dt = sum nu R` while
    /// `d(m)/dt = -R` — so it holds to solver arithmetic precision, comfortably
    /// inside the required `1e-8`).
    #[test]
    fn mole_conservation() {
        let net = ReactionNetwork::new(
            vec!["A".into(), "B".into(), "C".into()],
            vec![],
            vec![],
            vec![],
        )
        .unwrap();
        let m1 = KineticMineral {
            name: "M1".into(),
            stoich: vec![1.0, 1.0, 0.0],
            log10_ksp: -6.0,
            rate_constant: 1.0,
            surface_area: 1.0,
        };
        let m2 = KineticMineral {
            name: "M2".into(),
            stoich: vec![0.0, 1.0, 1.0],
            log10_ksp: -6.0,
            rate_constant: 1.0,
            surface_area: 1.0,
        };
        let sys = KineticSystem::new(net, vec![m1.clone(), m2.clone()]).unwrap();

        let t0 = vec![1.0e-4, 1.0e-4, 1.0e-4];
        let m0 = vec![1.0, 1.0];
        let mut totals = t0.clone();
        let mut minerals = m0.clone();

        sys.react(&mut totals, &mut minerals, 1.0e-2).unwrap();

        let dm = [minerals[0] - m0[0], minerals[1] - m0[1]];
        // Real dissolution happened (both minerals shrank).
        assert!(dm[0] < 0.0 && dm[1] < 0.0, "no dissolution: dm = {dm:?}");

        let stoich = [&m1.stoich, &m2.stoich];
        for j in 0..3 {
            let d_total = totals[j] - t0[j];
            let released: f64 = (0..2).map(|i| stoich[i][j] * (-dm[i])).sum();
            assert!(
                (d_total - released).abs() < 1e-12,
                "conservation violated at component {j}: Δtotal = {d_total}, sum nu*(-Δm) = {released}"
            );
        }
    }

    /// **No-mineral clamp.**
    ///
    /// Methodology: an undersaturated solution (`T_A = T_B = 1e-4`, `Omega =
    /// 1e-2 < 1`) whose single mineral is **absent** (`m0 = 0`). Dissolution is
    /// the only reaction the affinity favours, but there is no solid to
    /// dissolve, so the clamp must hold the rate at zero: the mineral stays at
    /// 0 and the totals are unchanged.
    ///
    /// Results (checked in-test): after `dt = 1.0 s`, `m == 0` exactly and each
    /// total is unchanged to `< 1e-12`.
    #[test]
    fn no_mineral_clamp() {
        let sys = KineticSystem::new(ab_network(), vec![ab_mineral(-6.0)]).unwrap();

        let t0 = vec![1.0e-4, 1.0e-4];
        let mut totals = t0.clone();
        let mut minerals = vec![0.0];

        // Confirm the state is genuinely undersaturated (would dissolve if solid present).
        assert!(sys.saturation_index(0, &totals) < 0.0);

        sys.react(&mut totals, &mut minerals, 1.0).unwrap();

        assert_eq!(minerals[0], 0.0, "absent mineral changed: {}", minerals[0]);
        for j in 0..2 {
            assert!(
                (totals[j] - t0[j]).abs() < 1e-12,
                "total {j} changed: {} -> {}",
                t0[j],
                totals[j]
            );
        }
    }

    /// **Precipitation from zero is still allowed** (the clamp only blocks
    /// dissolution of an absent phase). A supersaturated solution with no solid
    /// present must nucleate mineral and draw the totals down.
    #[test]
    fn supersaturated_precipitates_from_zero() {
        // Ksp = 1e-6 but start with C_A = C_B = 1e-2 => Omega = 1e-4/1e-6 = 100 > 1.
        let sys = KineticSystem::new(ab_network(), vec![ab_mineral(-6.0)]).unwrap();
        let mut totals = vec![1.0e-2, 1.0e-2];
        let mut minerals = vec![0.0];

        assert!(sys.saturation_index(0, &totals) > 0.0);
        sys.react(&mut totals, &mut minerals, 1.0e-2).unwrap();

        assert!(
            minerals[0] > 0.0,
            "supersaturated solution did not precipitate: m = {}",
            minerals[0]
        );
        assert!(totals[0] < 1.0e-2 && totals[1] < 1.0e-2, "totals {totals:?} did not fall");
    }

    /// **Constructor validation:** a mineral whose stoichiometry length does not
    /// match the network's primary count is rejected, as are negative rate /
    /// area and non-finite entries.
    #[test]
    fn constructor_validation() {
        let net = ab_network(); // Nc = 2

        // stoich length 3 != Nc = 2
        let bad_len = KineticMineral {
            name: "bad".into(),
            stoich: vec![1.0, 1.0, 1.0],
            log10_ksp: -6.0,
            rate_constant: 1.0,
            surface_area: 1.0,
        };
        assert!(KineticSystem::new(net.clone(), vec![bad_len]).is_err());

        // negative rate constant
        let bad_k = KineticMineral {
            name: "bad".into(),
            stoich: vec![1.0, 1.0],
            log10_ksp: -6.0,
            rate_constant: -1.0,
            surface_area: 1.0,
        };
        assert!(KineticSystem::new(net.clone(), vec![bad_k]).is_err());

        // negative surface area
        let bad_a = KineticMineral {
            name: "bad".into(),
            stoich: vec![1.0, 1.0],
            log10_ksp: -6.0,
            rate_constant: 1.0,
            surface_area: -1.0,
        };
        assert!(KineticSystem::new(net.clone(), vec![bad_a]).is_err());

        // non-finite log10_ksp
        let bad_ksp = KineticMineral {
            name: "bad".into(),
            stoich: vec![1.0, 1.0],
            log10_ksp: f64::NAN,
            rate_constant: 1.0,
            surface_area: 1.0,
        };
        assert!(KineticSystem::new(net.clone(), vec![bad_ksp]).is_err());

        // a valid single-mineral system constructs.
        let ok = KineticSystem::new(net, vec![ab_mineral(-6.0)]).unwrap();
        assert_eq!(ok.n_minerals(), 1);
        assert_eq!(ok.n_primary(), 2);

        // bad react inputs
        let mut t = vec![1e-4, 1e-4];
        let mut m = vec![1.0];
        assert!(ok.react(&mut vec![1e-4], &mut m, 1.0).is_err()); // wrong totals len
        assert!(ok.react(&mut t, &mut vec![1.0, 2.0], 1.0).is_err()); // wrong minerals len
        assert!(ok.react(&mut t, &mut m, -1.0).is_err()); // negative dt
    }
}
