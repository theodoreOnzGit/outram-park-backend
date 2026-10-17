// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.
//
// ---------------------------------------------------------------------------
// PORTED from OpenMC (NOT from GSL, which has no depletion integrators).
//
//   Project : OpenMC, https://github.com/openmc-dev/openmc
//   Files   : openmc/deplete/integrators.py (all eight integrators, :16-562)
//             openmc/deplete/abc.py (`Integrator._timed_deplete` :758-769,
//             `Integrator.integrate` :902-988, `SIIntegrator` :1202-1440)
//             openmc/deplete/pool.py (`deplete`, :67-245, the single-material
//             path without transfer or external-source rates)
//   Commit  : d7d3284a1b13d7cae020e0d8fea9f1e60d91b18b (2026-09-05)
//   Licence : MIT. "Copyright (c) 2011-2026 Massachusetts Institute of
//             Technology, UChicago Argonne LLC, and OpenMC contributors" --
//             full notice in NOTICE (section "DERIVED WORK -- OpenMC").
//
// Ported 2026-10-17 (gh:#950). Maintainer direction, recorded on #950:
// "Integrators can go in PETIR"; "All math moved to petir"; "match upstream as
// much as possible, and flag every simplification made for compute speed".
// ---------------------------------------------------------------------------

//! OpenMC's eight depletion time integrators, generic over the operator.
//!
//! See the [module docs](super) for the method table, the predicted orders
//! and the full list of deviations.

use alloc::vec::Vec;

use super::cram::DepSystemSolver;
use super::matrix::DepletionMatrix;
use super::matrix_funcs::{
    celi_f1, celi_f2, cf4_f1, cf4_f2, cf4_f3, cf4_f4, leqi_f1, leqi_f2, leqi_f3, leqi_f4, rk4_f1,
    rk4_f4,
};
use crate::PetirError;

/// An eigenvalue with its one-standard-deviation uncertainty — upstream
/// carries `k` as an `uncertainties.ufloat` (`OperatorResult.k`,
/// `abc.py:32-34`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Keff {
    /// Nominal value.
    pub value: f64,
    /// One standard deviation.
    pub std_dev: f64,
}

/// What one operator evaluation returns — upstream's
/// `OperatorResult = namedtuple('OperatorResult', ['k', 'rates'])`
/// (`abc.py:32`).
#[derive(Debug, Clone, PartialEq)]
pub struct OperatorResult<R> {
    /// Eigenvalue (or whatever scalar the operator reports) and uncertainty.
    pub k: Keff,
    /// Reaction rates, in whatever form the operator's
    /// [`TransportOperator::form_matrix`] reads.
    pub rates: R,
}

/// The weighted average the stochastic-implicit integrators form between
/// iterations: `rates = 1/j * inter_res.rates + (1 - 1/j) * res_bar.rates`
/// (`integrators.py:463`, `:547`).
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be averaged as reaction rates",
    note = "implement `AverageRates::weighted_sum` as `a * x + b * y` element by element, as numpy does for OpenMC's `ReactionRates` array"
)]
pub trait AverageRates: Sized {
    /// `a * x + b * y`, element by element, each element evaluated as
    /// `(a * x_i) + (b * y_i)`.
    fn weighted_sum(a: f64, x: &Self, b: f64, y: &Self) -> Self;
}

impl AverageRates for Vec<f64> {
    fn weighted_sum(a: f64, x: &Self, b: f64, y: &Self) -> Self {
        x.iter().zip(y).map(|(xi, yi)| a * xi + b * yi).collect()
    }
}

impl AverageRates for DepletionMatrix {
    /// The matrices themselves as "rates", for an operator whose rates are
    /// already a burnup matrix. `form_matrix` is linear in the rates, so this
    /// averages the same quantity, but the rounding differs from averaging the
    /// rates and then forming the matrix.
    fn weighted_sum(a: f64, x: &Self, b: f64, y: &Self) -> Self {
        let mut out = x.scaled(a);
        for (o, v) in out.as_mut_slice().iter_mut().zip(y.as_slice()) {
            *o = *o + b * v;
        }
        out
    }
}

/// The transport side of a depletion calculation — upstream's
/// `TransportOperator` (`abc.py:119-231`) together with `Chain.form_matrix`.
///
/// The integrators call [`evaluate`](Self::evaluate) where upstream calls
/// `self.operator(n, source_rate)`, and [`form_matrix`](Self::form_matrix)
/// where upstream's `deplete` calls `chain.form_matrix(rates[i],
/// fission_yields)` for material `i` (`pool.py:122-126`). Fission yields are
/// the operator's business.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a depletion operator",
    note = "implement `evaluate` (reaction rates at a composition) and `form_matrix` (the burnup matrix for one material from those rates)"
)]
pub trait TransportOperator {
    /// Reaction rates for every material.
    type Rates: Clone + AverageRates;
    /// The operator's own failure type.
    type Error;

    /// Transport (or a fixed-rate lookup) at composition `n` — one vector per
    /// material — and source rate `source_rate` (power in W or neutrons/s).
    ///
    /// # Errors
    ///
    /// Whatever the operator reports.
    fn evaluate(
        &mut self,
        n: &[Vec<f64>],
        source_rate: f64,
    ) -> core::result::Result<OperatorResult<Self::Rates>, Self::Error>;

    /// The burnup matrix (`1/s`) of material `material` from `rates`.
    ///
    /// # Errors
    ///
    /// Whatever the operator reports.
    fn form_matrix(
        &self,
        rates: &Self::Rates,
        material: usize,
    ) -> core::result::Result<DepletionMatrix, Self::Error>;

    /// Called before the stochastic-implicit integrators' first
    /// beginning-of-step evaluation, where upstream multiplies
    /// `operator.settings.particles` by `n_steps` (`abc.py:1330-1334`).
    /// Default: nothing.
    fn begin_si_initial_solve(&mut self, _n_steps: usize) {}

    /// Called after it, where upstream divides the particle count back
    /// (`abc.py:1337-1339`). Default: nothing.
    fn end_si_initial_solve(&mut self, _n_steps: usize) {}
}

/// What went wrong in a depletion step.
#[derive(Debug, Clone, PartialEq)]
pub enum DepletionError<E> {
    /// The numerics failed — a solve, a dimension mismatch or a bad argument.
    Numeric(PetirError),
    /// The operator failed.
    Operator(E),
}

impl<E> From<PetirError> for DepletionError<E> {
    fn from(e: PetirError) -> Self {
        DepletionError::Numeric(e)
    }
}

/// The eight integrators — upstream's `integrator_by_name`
/// (`integrators.py:565-574`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Integrator {
    /// `PredictorIntegrator` (`integrators.py:16-54`).
    Predictor,
    /// `CECMIntegrator` (`integrators.py:57-109`).
    CeCm,
    /// `CF4Integrator` (`integrators.py:112-185`).
    Cf4,
    /// `CELIIntegrator` (`integrators.py:188-247`).
    CeLi,
    /// `EPCRK4Integrator` (`integrators.py:250-313`).
    EpcRk4,
    /// `LEQIIntegrator` (`integrators.py:316-410`).
    LeQi,
    /// `SICELIIntegrator` (`integrators.py:413-475`).
    SiCeLi,
    /// `SILEQIIntegrator` (`integrators.py:478-562`).
    SiLeQi,
}

impl Integrator {
    /// All eight, in upstream's `__all__` order (`integrators.py:10-13`).
    pub const ALL: [Integrator; 8] = [
        Integrator::Predictor,
        Integrator::CeCm,
        Integrator::Cf4,
        Integrator::CeLi,
        Integrator::EpcRk4,
        Integrator::LeQi,
        Integrator::SiCeLi,
        Integrator::SiLeQi,
    ];

    /// Upstream's key in `integrator_by_name` (`integrators.py:565-574`).
    pub fn name(self) -> &'static str {
        match self {
            Integrator::Predictor => "predictor",
            Integrator::CeCm => "cecm",
            Integrator::Cf4 => "cf4",
            Integrator::CeLi => "celi",
            Integrator::EpcRk4 => "epc_rk4",
            Integrator::LeQi => "leqi",
            Integrator::SiCeLi => "si_celi",
            Integrator::SiLeQi => "si_leqi",
        }
    }

    /// The integrator with upstream's name `name`, if there is one.
    pub fn by_name(name: &str) -> Option<Integrator> {
        Integrator::ALL.into_iter().find(|i| i.name() == name)
    }

    /// Upstream's `_num_stages` — "number of intermediate transport
    /// solutions", used upstream for restarts (`abc.py:804-809`).
    ///
    /// **Not the number of operator calls per step.** LE/QI declares 2 but
    /// calls the operator three times per step after the first (see
    /// [`Self::operator_calls`]).
    pub fn num_stages(self) -> usize {
        match self {
            Integrator::Predictor => 1,
            Integrator::CeCm | Integrator::CeLi | Integrator::LeQi => 2,
            Integrator::SiCeLi | Integrator::SiLeQi => 2,
            Integrator::Cf4 | Integrator::EpcRk4 => 4,
        }
    }

    /// Whether this is a stochastic-implicit integrator (`SIIntegrator`
    /// subclass, `abc.py:1202`), which has its own driver loop.
    pub fn is_stochastic_implicit(self) -> bool {
        matches!(self, Integrator::SiCeLi | Integrator::SiLeQi)
    }

    /// Operator evaluations [`DepletionIntegrator::integrate`] makes for step
    /// `step_index`, counting the beginning-of-step one — what a transport-
    /// coupled run pays. Pinned by counting in `tests::operator_call_counts`.
    ///
    /// For the stochastic-implicit methods `si_n_steps` is the number of
    /// iterations (upstream default 10); the beginning-of-step evaluation
    /// happens only on step 0 (`abc.py:1407-1411`). LE/QI's first step is
    /// CE/LI (`integrators.py:374-378`); later steps re-evaluate the operator
    /// at the beginning of step although the driver already did
    /// (`integrators.py:386`) — upstream's code, kept.
    pub fn operator_calls(self, step_index: usize, si_n_steps: usize) -> usize {
        let si_bos = usize::from(step_index == 0);
        match self {
            Integrator::Predictor => 1,
            Integrator::CeCm | Integrator::CeLi => 2,
            Integrator::Cf4 | Integrator::EpcRk4 => 4,
            Integrator::LeQi => {
                if step_index == 0 {
                    2
                } else {
                    3
                }
            }
            Integrator::SiCeLi | Integrator::SiLeQi => si_bos + si_n_steps + 1,
        }
    }

    /// The order of convergence this port PREDICTS for the method on a
    /// burnup matrix that depends on the composition and does not commute
    /// with itself at different compositions — stated before it was measured
    /// (`verification_and_validation/depletion/petir_integrators_2026_10_17.md`).
    ///
    /// Predictor 1, CE/CM 2 and CF4 4 are upstream's own docstring claims
    /// (`integrators.py:18-20`, `:61`, `:116`). The other five are this port's
    /// local-error argument, written down in that record: CE/LI and SI-CE/LI
    /// interpolate `A` linearly (error `O(h^2)` in `A`, local `O(h^3)`, so 2);
    /// EPC-RK4 exponentiates a single averaged matrix and so drops the
    /// commutator term of the Magnus expansion (local `O(h^3)`, so 2); LE/QI
    /// and SI-LE/QI interpolate quadratically (3).
    pub fn predicted_order(self) -> u32 {
        match self {
            Integrator::Predictor => 1,
            Integrator::CeCm | Integrator::CeLi | Integrator::EpcRk4 | Integrator::SiCeLi => 2,
            Integrator::LeQi | Integrator::SiLeQi => 3,
            Integrator::Cf4 => 4,
        }
    }
}

/// One step's outcome — upstream's `__call__` return value without the
/// timing (`proc_time`).
#[derive(Debug, Clone, PartialEq)]
pub struct StepOutcome<R> {
    /// End-of-step compositions, one vector per material.
    pub n_end: Vec<Vec<f64>>,
    /// The stochastic-implicit integrators' averaged end-of-step operator
    /// result `res_bar` (`integrators.py:475`, `:562`); `None` otherwise.
    pub res_end: Option<OperatorResult<R>>,
}

/// One saved step — the content of upstream's `StepResult.save` call in
/// `integrate` (`abc.py:957-968`, `:979-990`), held in memory.
#[derive(Debug, Clone, PartialEq)]
pub struct StepRecord<R> {
    /// Step index (`self._i_res + i`, with no restart `_i_res == 0`).
    pub index: usize,
    /// Beginning-of-step compositions.
    pub n: Vec<Vec<f64>>,
    /// The operator result saved with them.
    pub k: Keff,
    /// The reaction rates saved with them.
    pub rates: R,
    /// `[t, t + dt]` in seconds (`[t, t]` for the final record).
    pub time: [f64; 2],
    /// Source rate of the step.
    pub source_rate: f64,
}

/// A configured integrator — upstream's `Integrator` instance: the method,
/// the solver, the substeps, the time grid and (for LE/QI) the rates it
/// carries between steps.
#[derive(Debug, Clone, PartialEq)]
pub struct DepletionIntegrator<S, R> {
    /// Which of the eight.
    pub method: Integrator,
    /// The Bateman solver (`abc.py:705-716`; upstream default `cram48`).
    pub solver: S,
    /// Substeps per depletion interval (`abc.py:691`, default 1).
    pub substeps: usize,
    /// Stochastic-implicit iterations per step (`abc.py:1328`, default 10).
    pub n_steps: usize,
    /// Size of each interval in seconds (`self.timesteps`).
    pub timesteps: Vec<f64>,
    /// Source rate of each interval (`self.source_rates`).
    pub source_rates: Vec<f64>,
    /// LE/QI's `self._prev_rates`.
    prev_rates: Option<R>,
}

/// Clip negative entries to zero in place — `r.clip(min=0.0, out=r)`
/// (`abc.py:765-766`).
///
/// **DEVIATION (signed zero only):** `ndarray.clip(min=0)` is `np.maximum(r,
/// 0.0)`, which keeps `-0.0`; this also keeps it (`-0.0 < 0.0` is false). NaN
/// is kept by both.
fn clip(v: &mut [f64]) {
    for d in v.iter_mut() {
        if *d < 0.0 {
            *d = 0.0;
        }
    }
}

impl<S: DepSystemSolver, R: Clone + AverageRates> DepletionIntegrator<S, R> {
    /// A configured integrator with upstream's defaults for `substeps` (1)
    /// and `n_steps` (10).
    ///
    /// # Errors
    ///
    /// [`PetirError::LengthMismatch`] if `timesteps` and `source_rates`
    /// differ in length — upstream's `_normalize_timesteps` broadcasts a
    /// scalar source rate; pass one per step here.
    pub fn new(
        method: Integrator,
        solver: S,
        timesteps: Vec<f64>,
        source_rates: Vec<f64>,
    ) -> Result<Self, PetirError> {
        if timesteps.len() != source_rates.len() {
            return Err(PetirError::LengthMismatch {
                expected: timesteps.len(),
                found: source_rates.len(),
            });
        }
        Ok(Self {
            method,
            solver,
            substeps: 1,
            n_steps: 10,
            timesteps,
            source_rates,
            prev_rates: None,
        })
    }

    /// `self._timed_deplete(n, rates, dt, i, matrix_func)` without the timer
    /// (`abc.py:758-769`), through `pool.deplete`'s per-material loop
    /// (`pool.py:122-126`, `:233-239`): form material `m`'s matrix with
    /// `matrix(op, m)`, solve, clip.
    fn deplete<O, F>(
        &self,
        op: &O,
        n: &[Vec<f64>],
        dt: f64,
        mut matrix: F,
    ) -> Result<Vec<Vec<f64>>, DepletionError<O::Error>>
    where
        O: TransportOperator<Rates = R>,
        F: FnMut(&O, usize) -> Result<DepletionMatrix, DepletionError<O::Error>>,
    {
        let mut out = Vec::with_capacity(n.len());
        for (mat, n_mat) in n.iter().enumerate() {
            let a = matrix(op, mat)?;
            let mut r = self.solver.solve(&a, n_mat, dt, self.substeps)?;
            clip(&mut r);
            out.push(r);
        }
        Ok(out)
    }

    /// Advance one step — the integrator's `__call__(n, rates, dt,
    /// source_rate, i)`.
    ///
    /// `rates` are the beginning-of-step rates; `i` is the step index into
    /// [`Self::timesteps`] (LE/QI and SI-LE/QI read `timesteps[i - 1]`).
    ///
    /// # Errors
    ///
    /// [`DepletionError::Operator`] from the operator;
    /// [`DepletionError::Numeric`] from the solver, or
    /// [`PetirError::Invalid`] for LE/QI or SI-LE/QI called at `i > 0` with
    /// no previous rates or no `timesteps[i - 1]`.
    pub fn step<O>(
        &mut self,
        op: &mut O,
        n: &[Vec<f64>],
        rates: &R,
        dt: f64,
        source_rate: f64,
        i: usize,
    ) -> Result<StepOutcome<R>, DepletionError<O::Error>>
    where
        O: TransportOperator<Rates = R>,
    {
        let plain = |n_end| StepOutcome {
            n_end,
            res_end: None,
        };
        match self.method {
            Integrator::Predictor => self.predictor(op, n, rates, dt).map(plain),
            Integrator::CeCm => self.cecm(op, n, rates, dt, source_rate).map(plain),
            Integrator::Cf4 => self.cf4(op, n, rates, dt, source_rate).map(plain),
            Integrator::CeLi => self.celi(op, n, rates, dt, source_rate).map(plain),
            Integrator::EpcRk4 => self.epc_rk4(op, n, rates, dt, source_rate).map(plain),
            Integrator::LeQi => self.leqi(op, n, rates, dt, source_rate, i).map(plain),
            Integrator::SiCeLi => self.si_celi(op, n, rates, dt, source_rate),
            Integrator::SiLeQi => self.si_leqi(op, n, rates, dt, source_rate, i),
        }
    }

    /// `PredictorIntegrator.__call__` (`integrators.py:28-54`).
    fn predictor<O>(
        &self,
        op: &O,
        n: &[Vec<f64>],
        rates: &R,
        dt: f64,
    ) -> Result<Vec<Vec<f64>>, DepletionError<O::Error>>
    where
        O: TransportOperator<Rates = R>,
    {
        // :53
        self.deplete(op, n, dt, |op, m| {
            op.form_matrix(rates, m).map_err(DepletionError::Operator)
        })
    }

    /// `CECMIntegrator.__call__` (`integrators.py:77-109`).
    fn cecm<O>(
        &self,
        op: &mut O,
        n: &[Vec<f64>],
        rates: &R,
        dt: f64,
        source_rate: f64,
    ) -> Result<Vec<Vec<f64>>, DepletionError<O::Error>>
    where
        O: TransportOperator<Rates = R>,
    {
        // :102-103 -- deplete across first half of interval
        let n_middle = self.deplete(op, n, dt / 2.0, |op, m| {
            op.form_matrix(rates, m).map_err(DepletionError::Operator)
        })?;
        let res_middle = op
            .evaluate(&n_middle, source_rate)
            .map_err(DepletionError::Operator)?;
        // :107 -- entire interval with BOS concentrations, MOS reaction rates
        self.deplete(op, n, dt, |op, m| {
            op.form_matrix(&res_middle.rates, m)
                .map_err(DepletionError::Operator)
        })
    }

    /// `CF4Integrator.__call__` (`integrators.py:138-185`).
    ///
    /// Follows upstream's CODE: the final two exponentials are applied
    /// `cf4_f3` first, then `cf4_f4` (`:180-183`). Upstream's docstring writes
    /// the product as `exp(f3-matrix) exp(f4-matrix) n`, which read
    /// right-to-left applies `f4` first; the pre-move `outram-mc-libs` CF4
    /// followed the docstring. See the module docs and the V&V record for
    /// the measured consequence.
    fn cf4<O>(
        &self,
        op: &mut O,
        n_bos: &[Vec<f64>],
        bos_rates: &R,
        dt: f64,
        source_rate: f64,
    ) -> Result<Vec<Vec<f64>>, DepletionError<O::Error>>
    where
        O: TransportOperator<Rates = R>,
    {
        let fm = |op: &O, r: &R, m| op.form_matrix(r, m).map_err(DepletionError::Operator);
        // :163-165 -- Step 1: deplete with matrix 1/2*A(y0)
        let n_eos1 = self.deplete(op, n_bos, dt, |op, m| Ok(cf4_f1(&fm(op, bos_rates, m)?)))?;
        let res1 = op
            .evaluate(&n_eos1, source_rate)
            .map_err(DepletionError::Operator)?;
        // :168-170 -- Step 2: deplete with matrix 1/2*A(y1)
        let n_eos2 = self.deplete(op, n_bos, dt, |op, m| Ok(cf4_f1(&fm(op, &res1.rates, m)?)))?;
        let res2 = op
            .evaluate(&n_eos2, source_rate)
            .map_err(DepletionError::Operator)?;
        // :173-176 -- Step 3: deplete with matrix -1/2*A(y0)+A(y2)
        let n_eos3 = self.deplete(op, &n_eos1, dt, |op, m| {
            Ok(cf4_f2(&fm(op, bos_rates, m)?, &fm(op, &res2.rates, m)?)?)
        })?;
        let res3 = op
            .evaluate(&n_eos3, source_rate)
            .map_err(DepletionError::Operator)?;
        // :179-183 -- Step 4: deplete with two matrix exponentials
        let four = |op: &O, m| -> Result<[DepletionMatrix; 4], DepletionError<O::Error>> {
            Ok([
                fm(op, bos_rates, m)?,
                fm(op, &res1.rates, m)?,
                fm(op, &res2.rates, m)?,
                fm(op, &res3.rates, m)?,
            ])
        };
        let n_inter = self.deplete(op, n_bos, dt, |op, m| {
            let [a, b, c, d] = four(op, m)?;
            Ok(cf4_f3(&a, &b, &c, &d)?)
        })?;
        self.deplete(op, &n_inter, dt, |op, m| {
            let [a, b, c, d] = four(op, m)?;
            Ok(cf4_f4(&a, &b, &c, &d)?)
        })
    }

    /// `CELIIntegrator.__call__` (`integrators.py:210-247`).
    fn celi<O>(
        &self,
        op: &mut O,
        n_bos: &[Vec<f64>],
        rates: &R,
        dt: f64,
        source_rate: f64,
    ) -> Result<Vec<Vec<f64>>, DepletionError<O::Error>>
    where
        O: TransportOperator<Rates = R>,
    {
        let fm = |op: &O, r: &R, m| op.form_matrix(r, m).map_err(DepletionError::Operator);
        // :235-236 -- deplete to end using BOS rates
        let n_ce = self.deplete(op, n_bos, dt, |op, m| fm(op, rates, m))?;
        let res_ce = op
            .evaluate(&n_ce, source_rate)
            .map_err(DepletionError::Operator)?;
        // :239-245 -- deplete using two matrix exponentials
        self.celi_corrector(op, n_bos, rates, &res_ce.rates, dt)
    }

    /// The two CE/LI corrector exponentials, `celi_f1` then `celi_f2`
    /// (`integrators.py:241-245`, `:467-471`).
    fn celi_corrector<O>(
        &self,
        op: &O,
        n_bos: &[Vec<f64>],
        r0: &R,
        r1: &R,
        dt: f64,
    ) -> Result<Vec<Vec<f64>>, DepletionError<O::Error>>
    where
        O: TransportOperator<Rates = R>,
    {
        let fm = |op: &O, r: &R, m| op.form_matrix(r, m).map_err(DepletionError::Operator);
        let n_inter = self.deplete(op, n_bos, dt, |op, m| {
            Ok(celi_f1(&fm(op, r0, m)?, &fm(op, r1, m)?)?)
        })?;
        self.deplete(op, &n_inter, dt, |op, m| {
            Ok(celi_f2(&fm(op, r0, m)?, &fm(op, r1, m)?)?)
        })
    }

    /// `EPCRK4Integrator.__call__` (`integrators.py:272-313`).
    fn epc_rk4<O>(
        &self,
        op: &mut O,
        n: &[Vec<f64>],
        rates: &R,
        dt: f64,
        source_rate: f64,
    ) -> Result<Vec<Vec<f64>>, DepletionError<O::Error>>
    where
        O: TransportOperator<Rates = R>,
    {
        let fm = |op: &O, r: &R, m| op.form_matrix(r, m).map_err(DepletionError::Operator);
        // :298-299 -- Step 1: deplete with matrix A(y0) / 2
        let n1 = self.deplete(op, n, dt, |op, m| Ok(rk4_f1(&fm(op, rates, m)?)))?;
        let res1 = op
            .evaluate(&n1, source_rate)
            .map_err(DepletionError::Operator)?;
        // :302-303 -- Step 2: deplete with matrix A(y1) / 2
        let n2 = self.deplete(op, n, dt, |op, m| Ok(rk4_f1(&fm(op, &res1.rates, m)?)))?;
        let res2 = op
            .evaluate(&n2, source_rate)
            .map_err(DepletionError::Operator)?;
        // :306-307 -- Step 3: deplete with matrix A(y2)
        let n3 = self.deplete(op, n, dt, |op, m| fm(op, &res2.rates, m))?;
        let res3 = op
            .evaluate(&n3, source_rate)
            .map_err(DepletionError::Operator)?;
        // :310-311 -- Step 4: deplete with matrix built from weighted rates
        self.deplete(op, n, dt, |op, m| {
            Ok(rk4_f4(
                &fm(op, rates, m)?,
                &fm(op, &res1.rates, m)?,
                &fm(op, &res2.rates, m)?,
                &fm(op, &res3.rates, m)?,
            )?)
        })
    }

    /// `prev_dt` and the stored `_prev_rates` for LE/QI step `i >= 1`
    /// (`integrators.py:382-383`, `:527-528`).
    fn leqi_history<E>(&self, i: usize) -> Result<(f64, R), DepletionError<E>> {
        let prev_dt = i
            .checked_sub(1)
            .and_then(|j| self.timesteps.get(j))
            .copied()
            .ok_or(DepletionError::Numeric(PetirError::Invalid))?;
        let prev = self
            .prev_rates
            .clone()
            .ok_or(DepletionError::Numeric(PetirError::Invalid))?;
        Ok((prev_dt, prev))
    }

    /// `LEQIIntegrator.__call__` (`integrators.py:350-410`).
    ///
    /// **DEVIATION (restart):** the `i == 0` branch that resumes from previous
    /// results (`:379-381`, `self._i_res >= 1`) is not ported: this crate
    /// keeps no results file, so `_i_res` is always 0 and step 0 is always
    /// CE/LI (`:375-378`).
    fn leqi<O>(
        &mut self,
        op: &mut O,
        n_bos: &[Vec<f64>],
        bos_rates: &R,
        dt: f64,
        source_rate: f64,
        i: usize,
    ) -> Result<Vec<Vec<f64>>, DepletionError<O::Error>>
    where
        O: TransportOperator<Rates = R>,
    {
        if i == 0 {
            // :375-378 -- need at least previous transport solution
            self.prev_rates = Some(bos_rates.clone());
            return self.celi(op, n_bos, bos_rates, dt, source_rate);
        }
        let (prev_dt, prev) = self.leqi_history(i)?;

        // :386 -- Remaining LE/QI (re-evaluates the operator at BOS)
        let bos_res = op
            .evaluate(n_bos, source_rate)
            .map_err(DepletionError::Operator)?;

        let fm = |op: &O, r: &R, m| op.form_matrix(r, m).map_err(DepletionError::Operator);
        // :388-394
        let n_inter = self.deplete(op, n_bos, dt, |op, m| {
            Ok(leqi_f1(
                &fm(op, &prev, m)?,
                &fm(op, &bos_res.rates, m)?,
                prev_dt,
                dt,
            )?)
        })?;
        let n_eos0 = self.deplete(op, &n_inter, dt, |op, m| {
            Ok(leqi_f2(
                &fm(op, &prev, m)?,
                &fm(op, &bos_res.rates, m)?,
                prev_dt,
                dt,
            )?)
        })?;
        // :396
        let res_inter = op
            .evaluate(&n_eos0, source_rate)
            .map_err(DepletionError::Operator)?;
        // :398-405
        let n_eos1 = self.leqi_corrector(
            op,
            n_bos,
            &prev,
            &bos_res.rates,
            &res_inter.rates,
            prev_dt,
            dt,
        )?;
        // :408 -- store updated rates
        self.prev_rates = Some(bos_res.rates);
        Ok(n_eos1)
    }

    /// The two LE/QI corrector exponentials, `leqi_f3` then `leqi_f4`
    /// (`integrators.py:398-405`, `:551-556`).
    #[allow(clippy::too_many_arguments)]
    fn leqi_corrector<O>(
        &self,
        op: &O,
        n_bos: &[Vec<f64>],
        r_prev: &R,
        r_bos: &R,
        r_eos: &R,
        prev_dt: f64,
        dt: f64,
    ) -> Result<Vec<Vec<f64>>, DepletionError<O::Error>>
    where
        O: TransportOperator<Rates = R>,
    {
        let fm = |op: &O, r: &R, m| op.form_matrix(r, m).map_err(DepletionError::Operator);
        let n_inter = self.deplete(op, n_bos, dt, |op, m| {
            Ok(leqi_f3(
                &fm(op, r_prev, m)?,
                &fm(op, r_bos, m)?,
                &fm(op, r_eos, m)?,
                prev_dt,
                dt,
            )?)
        })?;
        self.deplete(op, &n_inter, dt, |op, m| {
            Ok(leqi_f4(
                &fm(op, r_prev, m)?,
                &fm(op, r_bos, m)?,
                &fm(op, r_eos, m)?,
                prev_dt,
                dt,
            )?)
        })
    }

    /// The stochastic-implicit running average (`integrators.py:460-465`,
    /// `:544-549`): `res_bar` is a copy of `inter_res` for `j <= 1`, then
    /// `1/j * inter_res + (1 - 1/j) * res_bar`.
    ///
    /// **DEVIATION (uncertainty of k):** upstream's `k` is an
    /// `uncertainties.ufloat`, whose `std_dev` is propagated with correlation
    /// tracking. Each iteration's `k` is a fresh independent variable, so the
    /// exact propagation is `sqrt((a*s_new)^2 + (b*s_bar)^2)`, which this
    /// computes; the nominal value is upstream's arithmetic exactly. The
    /// standard deviation may differ from `uncertainties`' in the last bits.
    fn si_average(
        j: usize,
        inter: OperatorResult<R>,
        bar: Option<OperatorResult<R>>,
    ) -> OperatorResult<R> {
        match bar {
            Some(bar) if j > 1 => {
                let jf = j as f64;
                let a = 1.0 / jf;
                let b = 1.0 - 1.0 / jf;
                let rates = R::weighted_sum(a, &inter.rates, b, &bar.rates);
                let value = a * inter.k.value + b * bar.k.value;
                let (sa, sb) = (a * inter.k.std_dev, b * bar.k.std_dev);
                #[allow(unused_imports)]
                use crate::real::Real;
                let std_dev = Real::sqrt(sa * sa + sb * sb);
                OperatorResult {
                    k: Keff { value, std_dev },
                    rates,
                }
            }
            _ => inter,
        }
    }

    /// `SICELIIntegrator.__call__` (`integrators.py:426-475`).
    fn si_celi<O>(
        &self,
        op: &mut O,
        n_bos: &[Vec<f64>],
        bos_rates: &R,
        dt: f64,
        source_rate: f64,
    ) -> Result<StepOutcome<R>, DepletionError<O::Error>>
    where
        O: TransportOperator<Rates = R>,
    {
        let fm = |op: &O, r: &R, m| op.form_matrix(r, m).map_err(DepletionError::Operator);
        // :453-454
        let mut n_inter = self.deplete(op, n_bos, dt, |op, m| fm(op, bos_rates, m))?;
        let mut res_bar: Option<OperatorResult<R>> = None;
        // :457-472 -- Begin iteration
        for j in 0..=self.n_steps {
            let inter_res = op
                .evaluate(&n_inter, source_rate)
                .map_err(DepletionError::Operator)?;
            let bar = Self::si_average(j, inter_res, res_bar.take());
            n_inter = self.celi_corrector(op, n_bos, bos_rates, &bar.rates, dt)?;
            res_bar = Some(bar);
        }
        Ok(StepOutcome {
            n_end: n_inter,
            res_end: res_bar,
        })
    }

    /// `SILEQIIntegrator.__call__` (`integrators.py:491-562`).
    ///
    /// **DEVIATION (restart):** as [`Self::leqi`]; step 0 is always SI-CE/LI.
    fn si_leqi<O>(
        &mut self,
        op: &mut O,
        n_bos: &[Vec<f64>],
        bos_rates: &R,
        dt: f64,
        source_rate: f64,
        i: usize,
    ) -> Result<StepOutcome<R>, DepletionError<O::Error>>
    where
        O: TransportOperator<Rates = R>,
    {
        if i == 0 {
            // :519-523 -- Perform CELI for initial steps
            self.prev_rates = Some(bos_rates.clone());
            return self.si_celi(op, n_bos, bos_rates, dt, source_rate);
        }
        let (prev_dt, prev) = self.leqi_history(i)?;
        let fm = |op: &O, r: &R, m| op.form_matrix(r, m).map_err(DepletionError::Operator);
        // :531-536 -- Perform remaining LE/QI
        let n_inter = self.deplete(op, n_bos, dt, |op, m| {
            Ok(leqi_f1(
                &fm(op, &prev, m)?,
                &fm(op, bos_rates, m)?,
                prev_dt,
                dt,
            )?)
        })?;
        let mut n_inter = self.deplete(op, &n_inter, dt, |op, m| {
            Ok(leqi_f2(
                &fm(op, &prev, m)?,
                &fm(op, bos_rates, m)?,
                prev_dt,
                dt,
            )?)
        })?;
        let mut res_bar: Option<OperatorResult<R>> = None;
        // :541-557
        for j in 0..=self.n_steps {
            let inter_res = op
                .evaluate(&n_inter, source_rate)
                .map_err(DepletionError::Operator)?;
            let bar = Self::si_average(j, inter_res, res_bar.take());
            n_inter = self.leqi_corrector(op, n_bos, &prev, bos_rates, &bar.rates, prev_dt, dt)?;
            res_bar = Some(bar);
        }
        // :560 -- Store updated rates for next step
        self.prev_rates = Some(bos_rates.clone());
        Ok(StepOutcome {
            n_end: n_inter,
            res_end: res_bar,
        })
    }

    /// Perform the entire depletion — `Integrator.integrate`
    /// (`abc.py:902-988`), or `SIIntegrator.integrate` (`abc.py:1370-1440`)
    /// for the stochastic-implicit methods.
    ///
    /// Returns the saved records in memory: one per step (beginning-of-step
    /// compositions with the operator result at them) and a final one at the
    /// end time. `final_step == false` evaluates the final operator call at
    /// zero source rate (`abc.py:981`); the SI methods ignore it (no final
    /// solve, `abc.py:1427`).
    ///
    /// **DEVIATIONS (scope):** no results file, `write_bos_data` or
    /// `finalize` (records are returned instead); no restart from previous
    /// results; no transfer rates, external sources, redox or keff search
    /// control (`pool.py:128-231`); no MPI or multiprocessing
    /// (`pool.py:235-239`); timesteps in seconds and one source rate per
    /// step (upstream's `_normalize_timesteps` unit conversion is not
    /// ported). `proc_time` is not measured.
    ///
    /// # Errors
    ///
    /// [`PetirError::Invalid`] for no timesteps (upstream would fail on the
    /// unbound loop variable at `abc.py:981`); otherwise as [`Self::step`].
    pub fn integrate<O>(
        &mut self,
        op: &mut O,
        n0: Vec<Vec<f64>>,
        final_step: bool,
    ) -> Result<Vec<StepRecord<R>>, DepletionError<O::Error>>
    where
        O: TransportOperator<Rates = R>,
    {
        if self.timesteps.is_empty() {
            return Err(DepletionError::Numeric(PetirError::Invalid));
        }
        if self.method.is_stochastic_implicit() {
            return self.integrate_si(op, n0);
        }
        let mut records = Vec::with_capacity(self.timesteps.len() + 1);
        let mut n = n0;
        let mut t = 0.0;
        let mut source_rate = 0.0;
        let grid: Vec<(f64, f64)> = self
            .timesteps
            .iter()
            .copied()
            .zip(self.source_rates.iter().copied())
            .collect();
        for (i, (dt, sr)) in grid.into_iter().enumerate() {
            source_rate = sr;
            // abc.py:950 -> _get_bos_data_from_operator (:819-825)
            let res = op.evaluate(&n, sr).map_err(DepletionError::Operator)?;
            // :953
            let out = self.step(op, &n, &res.rates, dt, sr, i)?;
            // :955-966
            records.push(StepRecord {
                index: i,
                n,
                k: res.k,
                rates: res.rates,
                time: [t, t + dt],
                source_rate: sr,
            });
            // :969-970
            n = out.n_end;
            t += dt;
        }
        // :976-990 -- final simulation
        let res_final = op
            .evaluate(&n, if final_step { source_rate } else { 0.0 })
            .map_err(DepletionError::Operator)?;
        records.push(StepRecord {
            index: self.timesteps.len(),
            n,
            k: res_final.k,
            rates: res_final.rates,
            time: [t, t],
            source_rate,
        });
        Ok(records)
    }

    /// `SIIntegrator.integrate` (`abc.py:1370-1440`).
    fn integrate_si<O>(
        &mut self,
        op: &mut O,
        n0: Vec<Vec<f64>>,
    ) -> Result<Vec<StepRecord<R>>, DepletionError<O::Error>>
    where
        O: TransportOperator<Rates = R>,
    {
        let mut records = Vec::with_capacity(self.timesteps.len() + 1);
        let mut n = n0;
        let mut t = 0.0;
        let mut p = 0.0;
        let mut res: Option<OperatorResult<R>> = None;
        let grid: Vec<(f64, f64)> = self
            .timesteps
            .iter()
            .copied()
            .zip(self.source_rates.iter().copied())
            .collect();
        for (i, (dt, sr)) in grid.into_iter().enumerate() {
            p = sr;
            if i == 0 {
                // :1406-1409 -> _get_bos_data_from_operator (:1330-1340)
                op.begin_si_initial_solve(self.n_steps);
                let r = op.evaluate(&n, sr);
                op.end_si_initial_solve(self.n_steps);
                res = Some(r.map_err(DepletionError::Operator)?);
            }
            let bos = res
                .take()
                .ok_or(DepletionError::Numeric(PetirError::Invalid))?;
            // :1413
            let out = self.step(op, &n, &bos.rates, dt, sr, i)?;
            // :1415-1424
            records.push(StepRecord {
                index: i,
                n,
                k: bos.k,
                rates: bos.rates,
                time: [t, t + dt],
                source_rate: sr,
            });
            // :1427-1429
            n = out.n_end;
            res = out.res_end;
            t += dt;
        }
        // :1432-1442 -- No final simulation for SIE, use last iteration results
        let last = res.ok_or(DepletionError::Numeric(PetirError::Invalid))?;
        records.push(StepRecord {
            index: self.timesteps.len(),
            n,
            k: last.k,
            rates: last.rates,
            time: [t, t],
            source_rate: p,
        });
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::depletion::cram::CramSolver;
    use std::vec;

    /// Single-nuclide materials whose removal rate depends on their own
    /// density (`l0 * (1 + n)`), or is constant.
    struct Toy {
        calls: usize,
        constant: bool,
        fail_at: Option<usize>,
    }

    impl TransportOperator for Toy {
        type Rates = Vec<f64>;
        type Error = &'static str;

        fn evaluate(
            &mut self,
            n: &[Vec<f64>],
            _s: f64,
        ) -> core::result::Result<OperatorResult<Vec<f64>>, &'static str> {
            self.calls += 1;
            if self.fail_at == Some(self.calls) {
                return Err("transport failed");
            }
            let rates = n
                .iter()
                .map(|m| {
                    if self.constant {
                        1.0e-2
                    } else {
                        1.0e-2 * (1.0 + m[0])
                    }
                })
                .collect();
            Ok(OperatorResult {
                k: Keff {
                    value: 1.0,
                    std_dev: 0.0,
                },
                rates,
            })
        }

        fn form_matrix(
            &self,
            rates: &Vec<f64>,
            material: usize,
        ) -> core::result::Result<DepletionMatrix, &'static str> {
            let mut a = DepletionMatrix::zeros(1);
            a.set(0, 0, -*rates.get(material).ok_or("no such material")?)
                .map_err(|_| "set")?;
            Ok(a)
        }
    }

    fn run(method: Integrator, constant: bool, steps: usize) -> (Vec<StepRecord<Vec<f64>>>, usize) {
        let mut op = Toy {
            calls: 0,
            constant,
            fail_at: None,
        };
        let mut integ = DepletionIntegrator::new(
            method,
            CramSolver::Cram48,
            vec![10.0; steps],
            vec![1.0; steps],
        )
        .unwrap();
        integ.n_steps = 3;
        let recs = integ
            .integrate(&mut op, vec![vec![1.0], vec![0.5]], true)
            .unwrap();
        (recs, op.calls)
    }

    /// `operator_calls` is what `integrate` actually does, step by step.
    #[test]
    fn operator_call_counts() {
        for method in Integrator::ALL {
            let mut expected = 0;
            for steps in 1..=3 {
                expected += method.operator_calls(steps - 1, 3);
                let (_, calls) = run(method, false, steps);
                let final_call = usize::from(!method.is_stochastic_implicit());
                assert_eq!(calls, expected + final_call, "{method:?}, {steps} steps");
            }
        }
    }

    /// Every method is exact (to CRAM48) when the matrix does not depend on
    /// the composition, for every material.
    #[test]
    fn exact_on_a_constant_matrix() {
        for method in Integrator::ALL {
            let (recs, _) = run(method, true, 3);
            let last = recs.last().unwrap();
            assert_eq!(recs.len(), 4);
            assert_eq!(last.time, [30.0, 30.0]);
            for (n, n0) in last.n.iter().zip([1.0, 0.5]) {
                let want = n0 * (-0.3_f64).exp();
                assert!(
                    ((n[0] - want) / want).abs() < 1e-13,
                    "{method:?}: {} vs {want}",
                    n[0]
                );
            }
        }
    }

    #[test]
    fn names_round_trip() {
        for m in Integrator::ALL {
            assert_eq!(Integrator::by_name(m.name()), Some(m));
            assert!(m.num_stages() >= 1 && m.predicted_order() >= 1);
        }
        assert_eq!(Integrator::by_name("rk45"), None);
    }

    /// Failures are returned, never panicked.
    #[test]
    fn errors_are_reported() {
        assert!(DepletionIntegrator::<CramSolver, Vec<f64>>::new(
            Integrator::Cf4,
            CramSolver::Cram16,
            vec![1.0],
            vec![]
        )
        .is_err());
        let mut empty = DepletionIntegrator::<CramSolver, Vec<f64>>::new(
            Integrator::Predictor,
            CramSolver::Cram48,
            vec![],
            vec![],
        )
        .unwrap();
        let mut op = Toy {
            calls: 0,
            constant: true,
            fail_at: None,
        };
        assert_eq!(
            empty.integrate(&mut op, vec![vec![1.0]], true),
            Err(DepletionError::Numeric(PetirError::Invalid))
        );
        // LE/QI at i > 0 with no history.
        for method in [Integrator::LeQi, Integrator::SiLeQi] {
            let mut integ = DepletionIntegrator::new(
                method,
                CramSolver::Cram48,
                vec![1.0, 1.0],
                vec![1.0, 1.0],
            )
            .unwrap();
            let r = integ.step(&mut op, &[vec![1.0]], &vec![0.1], 1.0, 1.0, 1);
            assert_eq!(r, Err(DepletionError::Numeric(PetirError::Invalid)));
        }
        // An operator failure mid-step propagates.
        for method in Integrator::ALL {
            let mut bad = Toy {
                calls: 0,
                constant: false,
                fail_at: Some(2),
            };
            let mut integ =
                DepletionIntegrator::new(method, CramSolver::Cram48, vec![1.0; 2], vec![1.0; 2])
                    .unwrap();
            integ.n_steps = 1;
            assert_eq!(
                integ.integrate(&mut bad, vec![vec![1.0]], true),
                Err(DepletionError::Operator("transport failed")),
                "{method:?}"
            );
        }
        // form_matrix failing (material 1 has no rate) propagates too.
        let mut integ = DepletionIntegrator::new(
            Integrator::Predictor,
            CramSolver::Cram48,
            vec![1.0],
            vec![1.0],
        )
        .unwrap();
        let r = integ.step(&mut op, &[vec![1.0], vec![1.0]], &vec![0.1], 1.0, 1.0, 0);
        assert_eq!(r, Err(DepletionError::Operator("no such material")));
        assert_eq!(
            DepletionError::<()>::from(PetirError::Domain),
            DepletionError::Numeric(PetirError::Domain)
        );
    }

    /// The SI running average follows upstream's weights and propagates k's
    /// standard deviation as an independent sum.
    #[test]
    fn si_average_weights() {
        let r = |v: f64, s: f64| OperatorResult {
            k: Keff {
                value: v,
                std_dev: s,
            },
            rates: vec![v],
        };
        type I = DepletionIntegrator<CramSolver, Vec<f64>>;
        assert_eq!(
            I::si_average(0, r(2.0, 0.1), Some(r(5.0, 0.1))).rates,
            vec![2.0]
        );
        assert_eq!(
            I::si_average(1, r(2.0, 0.1), Some(r(5.0, 0.1))).rates,
            vec![2.0]
        );
        let avg = I::si_average(4, r(2.0, 0.4), Some(r(6.0, 0.0)));
        assert_eq!(avg.rates, vec![0.25 * 2.0 + 0.75 * 6.0]);
        assert_eq!(avg.k.value, 0.25 * 2.0 + 0.75 * 6.0);
        assert!((avg.k.std_dev - 0.1).abs() < 1e-15);
        let m = DepletionMatrix::zeros(1);
        let mut one = DepletionMatrix::zeros(1);
        one.set(0, 0, 4.0).unwrap();
        assert_eq!(
            DepletionMatrix::weighted_sum(0.5, &one, 0.5, &m).get(0, 0),
            2.0
        );
    }
}
