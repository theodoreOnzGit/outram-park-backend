//! gh:#1191's pilot: tracking a drifting wall resistance r_f(t) in the
//! double pipe from noisy synthetic outlet temperatures, with FilterPy's
//! filters ported into `raffles::filtering`, through
//! `raffles::fidelity::StateSpaceModel`.
//!
//! **Synthetic truth only** (`RESPONSIBLE_USE.md`): a twin experiment for
//! teaching and V&V, never an estimate of a real exchanger.
//!
//! **Construction** (V&V record
//! `verification_and_validation/double_pipe_filtering_gh1191.md`, written
//! before this code):
//! - the truth takes the CFD's own mean Nusselt numbers at a training design
//!   point ([`Closures::from_cfd`] on the committed gh:#1178 dataset), folds
//!   r_f(k) into the tube side ([`one_d::fold_fouling`]), takes UA from TUAS's
//!   nodal conductances ([`one_d::ua_with_nusselt`]) and the outlets from
//!   counter-current ε-NTU ([`one_d::eps_ntu`]);
//! - r_f(k) follows the Kern–Seaton form `r_∞ (1 − e^{−k/τ})`, a
//!   **synthetic** scenario ([`R_INF`], [`TAU_STEPS`]), not a measurement:
//!   the CFD has r_f = 0;
//! - the noise is N(0, [`SIGMA_T_K`]²) per outlet, from the workspace LCG.
//!
//! **No new physics:** every relation evaluated is TUAS's (conductances,
//! Gnielinski), dwsim-libs' (ε-NTU) or an existing function of [`one_d`];
//! this module composes them and runs the filters.
use petir::mathf::RealMath;
use std::marker::PhantomData;

use raffles::distributions::seeded::sample_normal;
use raffles::fidelity::{Assimilator, Quantities, StateSpaceModel};
use raffles::filtering::{nees, Matrix};
use raffles::{RafflesError, Result};
use uom::si::f64::{Ratio, ThermodynamicTemperature};
use uom::si::ratio::ratio;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

use super::closure::Closures;
use super::dataset::load_committed;
use super::design::Split;
use super::{fixed, fouling_to_si, one_d, AreaSpecificThermalResistance, DoublePipeInput};

/// Asymptotic fouling resistance of the synthetic truth \[m² K/W\], inside
/// Incropera & DeWitt's Table 11.1 water-service range (2e-4 – 1e-3).
pub const R_INF: f64 = 4.0e-4;
/// Kern–Seaton time constant of the synthetic truth \[steps\].
pub const TAU_STEPS: f64 = 60.0;
/// Number of assimilation steps.
pub const STEPS: usize = 200;
/// Outlet thermometer noise, one standard deviation \[K\] (synthetic).
pub const SIGMA_T_K: f64 = 0.05;
/// Steps excluded from the "after burn-in" metrics.
pub const BURN_IN: usize = 30;

/// The true r_f at step `s` (k = s + 1).
#[must_use]
pub fn truth_r_f(step: usize) -> f64 {
    R_INF * (1.0 - (-((step + 1) as f64) / TAU_STEPS).r_exp())
}

raffles::quantities! {
    /// The two measured outlet temperatures.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct OutletTemperatures {
        /// Hot (tube) outlet.
        pub t_hot_out: ThermodynamicTemperature,
        /// Cold (annulus) outlet.
        pub t_cold_out: ThermodynamicTemperature,
    }
}

raffles::quantities! {
    /// State of P-A / P-B: the wall resistance alone.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct FoulingState {
        /// Wall (fouling) resistance on the tube's outer area.
        pub r_f: AreaSpecificThermalResistance,
    }
}

raffles::quantities! {
    /// State of P-D: ln θ_h and r_f.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct FoulingHotState {
        /// ln of the tube-side Nusselt multiplier.
        pub ln_nu_mult_hot: Ratio,
        /// Wall resistance.
        pub r_f: AreaSpecificThermalResistance,
    }
}

raffles::quantities! {
    /// State of P-C: ln θ_h, ln θ_c and r_f.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct FoulingBothState {
        /// ln of the tube-side Nusselt multiplier.
        pub ln_nu_mult_hot: Ratio,
        /// ln of the annulus-side Nusselt multiplier.
        pub ln_nu_mult_cold: Ratio,
        /// Wall resistance.
        pub r_f: AreaSpecificThermalResistance,
    }
}

/// A state the fouling model can read: `(ln θ_h, ln θ_c, r_f)` in SI.
pub trait FoulingStateLike: Quantities {
    /// The three, with absent components 0.
    fn parts(&self) -> (f64, f64, f64);
    /// Which SI channel is r_f.
    fn r_f_index() -> usize;
    /// Prior standard deviations, per channel (V&V record).
    fn prior_sd() -> Vec<f64>;
    /// Process-noise standard deviations per step, per channel.
    fn process_sd() -> Vec<f64>;
}

const PRIOR_SD_R_F: f64 = 5.0e-4;
const PRIOR_SD_LN_H: f64 = 0.2;
const PRIOR_SD_LN_C: f64 = 0.3;
const PROCESS_SD_LN: f64 = 1.0e-3;

fn process_sd_r_f() -> f64 {
    R_INF / TAU_STEPS
}

impl FoulingStateLike for FoulingState {
    fn parts(&self) -> (f64, f64, f64) {
        (0.0, 0.0, fouling_to_si(self.r_f))
    }
    fn r_f_index() -> usize {
        0
    }
    fn prior_sd() -> Vec<f64> {
        vec![PRIOR_SD_R_F]
    }
    fn process_sd() -> Vec<f64> {
        vec![process_sd_r_f()]
    }
}

impl FoulingStateLike for FoulingHotState {
    fn parts(&self) -> (f64, f64, f64) {
        (self.ln_nu_mult_hot.get::<ratio>(), 0.0, fouling_to_si(self.r_f))
    }
    fn r_f_index() -> usize {
        1
    }
    fn prior_sd() -> Vec<f64> {
        vec![PRIOR_SD_LN_H, PRIOR_SD_R_F]
    }
    fn process_sd() -> Vec<f64> {
        vec![PROCESS_SD_LN, process_sd_r_f()]
    }
}

impl FoulingStateLike for FoulingBothState {
    fn parts(&self) -> (f64, f64, f64) {
        (self.ln_nu_mult_hot.get::<ratio>(), self.ln_nu_mult_cold.get::<ratio>(), fouling_to_si(self.r_f))
    }
    fn r_f_index() -> usize {
        2
    }
    fn prior_sd() -> Vec<f64> {
        vec![PRIOR_SD_LN_H, PRIOR_SD_LN_C, PRIOR_SD_R_F]
    }
    fn process_sd() -> Vec<f64> {
        vec![PROCESS_SD_LN, PROCESS_SD_LN, process_sd_r_f()]
    }
}

/// Where the (unfouled, θ = 1) Nusselt numbers come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosureBase {
    /// The CFD's own, at the point (it must be a training point).
    Cfd,
    /// TUAS's textbook correlations (the uncalibrated 1-D model).
    Textbook,
}

/// The operating points, one per step.
#[derive(Debug, Clone, PartialEq)]
pub enum Schedule {
    /// One point throughout.
    Fixed(DoublePipeInput),
    /// Cycle through the points, one per step.
    Cycling(Vec<DoublePipeInput>),
}

impl Schedule {
    /// The point at `step`.
    ///
    /// # Panics
    /// On an empty cycle (a construction bug).
    #[must_use]
    pub fn at(&self, step: usize) -> DoublePipeInput {
        match self {
            Self::Fixed(x) => *x,
            Self::Cycling(v) => v[step % v.len()],
        }
    }

    fn points(&self) -> Vec<DoublePipeInput> {
        match self {
            Self::Fixed(x) => vec![*x],
            Self::Cycling(v) => v.clone(),
        }
    }
}

fn model_err(e: impl std::fmt::Display) -> RafflesError {
    RafflesError::ModelEvaluation { model: "double-pipe fouling twin".into(), reason: e.to_string() }
}

/// The base (unfouled, θ = 1) Nusselt numbers at `x`.
///
/// # Errors
/// If `x` is not a training point of the committed dataset (for
/// [`ClosureBase::Cfd`]), or TUAS fails.
pub fn base_nusselt(x: &DoublePipeInput, base: ClosureBase) -> Result<(f64, f64)> {
    match base {
        ClosureBase::Cfd => {
            let row = load_committed().into_iter().find(|r| r.split == Split::Training && r.input == *x).ok_or_else(|| model_err(format!("{x:?} is not a training point")))?;
            let c = Closures::from_cfd(&row.input, &row.output);
            Ok((c.nu_hot.get::<ratio>(), c.nu_cold.get::<ratio>()))
        }
        ClosureBase::Textbook => {
            let (h, c) = one_d::textbook_nusselt(x).map_err(model_err)?;
            Ok((h.get::<ratio>(), c.get::<ratio>()))
        }
    }
}

/// The outlet temperatures at `x` for base Nusselt numbers, multipliers
/// `e^{ln θ}` and wall resistance `r_f` (module docs: TUAS UA, then ε-NTU,
/// then the energy balance on each stream).
///
/// # Errors
/// TUAS errors (e.g. a non-positive folded Nusselt number).
pub fn outlets(x: &DoublePipeInput, base: (f64, f64), ln_h: f64, ln_c: f64, r_f: f64) -> Result<[f64; 2]> {
    let nu_hot = one_d::fold_fouling(base.0 * ln_h.r_exp(), r_f);
    let nu_cold = base.1 * ln_c.r_exp();
    if !(nu_hot > 0.0 && nu_cold > 0.0) {
        return Err(model_err(format!("non-positive Nusselt number ({nu_hot}, {nu_cold})")));
    }
    let ua = one_d::ua_with_nusselt(x, Ratio::new::<ratio>(nu_hot), Ratio::new::<ratio>(nu_cold)).map_err(model_err)?;
    let eps = one_d::eps_ntu(x, ua);
    let (ch, cc) = x.capacity_rates();
    let q = eps * ch.min(cc) * (fixed::T_HOT_IN - fixed::T_COLD_IN);
    debug_assert!(ua.get::<watt_per_kelvin>() > 0.0);
    Ok([fixed::T_HOT_IN - q / ch, fixed::T_COLD_IN + q / cc])
}

/// The filter's model: a random-walk state observed through [`outlets`].
#[derive(Debug, Clone, PartialEq)]
pub struct FoulingModel<S> {
    schedule: Schedule,
    bases: Vec<(f64, f64)>,
    state: PhantomData<fn() -> S>,
}

impl<S: FoulingStateLike> FoulingModel<S> {
    /// Over `schedule`, with base Nusselt numbers from `base`.
    ///
    /// # Errors
    /// As [`base_nusselt`].
    pub fn new(schedule: Schedule, base: ClosureBase) -> Result<Self> {
        let bases = schedule.points().iter().map(|x| base_nusselt(x, base)).collect::<Result<Vec<_>>>()?;
        Ok(Self { schedule, bases, state: PhantomData })
    }

    fn base_at(&self, step: usize) -> (f64, f64) {
        self.bases[step % self.bases.len()]
    }
}

fn temps(v: [f64; 2]) -> OutletTemperatures {
    OutletTemperatures { t_hot_out: ThermodynamicTemperature::new::<kelvin>(v[0]), t_cold_out: ThermodynamicTemperature::new::<kelvin>(v[1]) }
}

impl<S: FoulingStateLike> StateSpaceModel for FoulingModel<S> {
    type State = S;
    type Observation = OutletTemperatures;
    /// A random walk: the noise-free step is the identity.
    fn propagate(&self, x: &S, _step: usize) -> Result<S> {
        Ok(x.clone())
    }
    fn observe(&self, x: &S, step: usize) -> Result<OutletTemperatures> {
        let (h, c, r) = x.parts();
        Ok(temps(outlets(&self.schedule.at(step), self.base_at(step), h, c, r)?))
    }
}

/// The synthetic measurements: the truth (CFD closures, θ = 1, r_f(k))
/// plus N(0, σ_T²) per outlet from the LCG at `seed`.
///
/// # Errors
/// As [`base_nusselt`] and [`outlets`].
pub fn synthetic_measurements(schedule: &Schedule, seed: u64) -> Result<Vec<OutletTemperatures>> {
    let pts = schedule.points();
    let bases = pts.iter().map(|x| base_nusselt(x, ClosureBase::Cfd)).collect::<Result<Vec<_>>>()?;
    let mut s = seed;
    (0..STEPS)
        .map(|k| {
            let t = outlets(&schedule.at(k), bases[k % bases.len()], 0.0, 0.0, truth_r_f(k))?;
            Ok(temps([t[0] + SIGMA_T_K * sample_normal(&mut s), t[1] + SIGMA_T_K * sample_normal(&mut s)]))
        })
        .collect()
}

/// Which filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKind {
    /// UKF, Merwe α 0.5, β 2, κ 3 − n.
    Unscented,
    /// EKF, central differences, relative step 1e-3.
    Extended,
    /// EnKF, N 50, seed 1191.
    Ensemble,
    /// OpenDA's EnKF (gh:#1192, `raffles::openda`), N 50, seed 1192: the
    /// ensemble mean and ensemble covariance as the estimate.
    OpenDaEnsemble,
}

/// One run's per-step trace and summary.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    /// The filter.
    pub filter: FilterKind,
    /// Per step: (truth r_f, estimated r_f, its σ, NEES, NIS).
    pub trace: Vec<[f64; 5]>,
    /// RMSE of r_f after burn-in \[m² K/W\].
    pub rmse_after_burn_in: f64,
    /// RMSE of r_f over all steps.
    pub rmse_all: f64,
    /// Mean error of r_f after burn-in.
    pub bias_after_burn_in: f64,
    /// Mean NEES (all steps).
    pub nees_mean: f64,
    /// Mean NIS (all steps).
    pub nis_mean: f64,
    /// Fraction of steps with the true r_f inside the 95 % interval.
    pub coverage_95: f64,
    /// Final posterior correlation of ln θ_h with r_f (NaN without θ_h).
    pub final_corr_h_rf: f64,
    /// Final posterior means, SI, in state order.
    pub final_mean: Vec<f64>,
}

/// Run one filter on `model` against `zs`. NEES is against the truth
/// `(0, …, 0, r_f(k))`; with `r_f_only_nees`, on the r_f channel alone
/// (P-B, whose θ has no true value: its closures are not the truth's).
///
/// # Errors
/// From the model or the filter.
pub fn run<S: FoulingStateLike>(model: &FoulingModel<S>, zs: &[OutletTemperatures], filter: FilterKind, r_f_only_nees: bool) -> Result<Run> {
    let n = S::width();
    let ir = S::r_f_index();
    let p0 = Matrix::diagonal(&S::prior_sd().iter().map(|v| v * v).collect::<Vec<f64>>());
    let q = Matrix::diagonal(&S::process_sd().iter().map(|v| v * v).collect::<Vec<f64>>());
    let r = Matrix::diagonal(&[SIGMA_T_K * SIGMA_T_K; 2]);
    let x0 = S::from_si(&vec![0.0; n])?;
    let mut a = match filter {
        FilterKind::Unscented => Assimilator::unscented::<S, OutletTemperatures>(&x0, p0, q, r, 0.5, 2.0, 3.0 - n as f64)?,
        FilterKind::Extended => Assimilator::extended::<S, OutletTemperatures>(&x0, p0, q, r, 1e-3)?,
        FilterKind::Ensemble => Assimilator::ensemble::<S, OutletTemperatures>(&x0, p0, q, r, 50, 1191)?,
        FilterKind::OpenDaEnsemble => Assimilator::openda_ensemble::<S, OutletTemperatures>(&x0, p0, q, r, 50, 1192)?,
    };
    let mut trace = Vec::with_capacity(zs.len());
    for (k, z) in zs.iter().enumerate() {
        let st = a.step(model, k, z)?;
        let m = st.posterior.mean.to_si();
        let p = &st.posterior.covariance_si;
        let tr = truth_r_f(k);
        let e = if r_f_only_nees {
            (m[ir] - tr).r_powi(2) / p.get(ir, ir)
        } else {
            let mut truth = vec![0.0; n];
            truth[ir] = tr;
            nees(&m, &truth, p)?
        };
        trace.push([tr, m[ir], p.get(ir, ir).sqrt(), e, st.nis]);
    }
    let after = &trace[BURN_IN..];
    let rms = |t: &[[f64; 5]]| (t.iter().map(|v| (v[1] - v[0]).r_powi(2)).sum::<f64>() / t.len() as f64).sqrt();
    let mean = |t: &[[f64; 5]], i: usize| t.iter().map(|v| v[i]).sum::<f64>() / t.len() as f64;
    let fin: raffles::fidelity::Gaussian<S> = a.estimate()?;
    let pf = &fin.covariance_si;
    let final_corr_h_rf = if ir > 0 { pf.get(0, ir) / (pf.get(0, 0) * pf.get(ir, ir)).sqrt() } else { f64::NAN };
    Ok(Run {
        filter,
        rmse_after_burn_in: rms(after),
        rmse_all: rms(&trace),
        bias_after_burn_in: after.iter().map(|v| v[1] - v[0]).sum::<f64>() / after.len() as f64,
        nees_mean: mean(&trace, 3),
        nis_mean: mean(&trace, 4),
        coverage_95: trace.iter().filter(|v| (v[1] - v[0]).abs() <= 1.96 * v[2]).count() as f64 / trace.len() as f64,
        final_corr_h_rf,
        final_mean: fin.mean.to_si(),
        trace,
    })
}

/// x_A of the V&V record: (Re_hot 30, Re_cold 15, k_wall 100, L 0.1), a
/// training point.
#[must_use]
pub fn x_a() -> DoublePipeInput {
    DoublePipeInput::si(30.0, 15.0, 100.0, 0.1)
}

/// P-C's cycle: the 9 training (Re_hot, Re_cold) pairs at k_wall 100,
/// L 0.1, Re_hot fastest.
#[must_use]
pub fn cycle_c() -> Vec<DoublePipeInput> {
    let mut v = Vec::new();
    for rc in super::design::RE_COLD_LEVELS {
        for rh in super::design::RE_HOT_LEVELS {
            v.push(DoublePipeInput::si(rh, rc, 100.0, 0.1));
        }
    }
    v
}

/// The experiments of the V&V record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Experiment {
    /// CFD closures known, r_f only, fixed x_A (identical twin).
    PA,
    /// Textbook closures, r_f only, fixed x_A (model-form error).
    PB,
    /// CFD closures × unknown θ_h, (ln θ_h, r_f), fixed x_A.
    PD,
    /// CFD closures × unknown θ_h, θ_c, (ln θ_h, ln θ_c, r_f), cycling.
    PC,
}

impl Experiment {
    /// All four, in the record's order.
    pub const ALL: [Self; 4] = [Self::PA, Self::PB, Self::PD, Self::PC];

    /// The record's label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::PA => "P-A",
            Self::PB => "P-B",
            Self::PD => "P-D",
            Self::PC => "P-C",
        }
    }
}

/// Seeds of the synthetic measurements: the fixed point, the cycle.
pub const MEASUREMENT_SEEDS: [u64; 2] = [11_910, 11_911];

/// Run `filter` on `experiment` (fresh synthetic measurements, same seed for
/// every filter, and the same series for P-A, P-B and P-D).
///
/// # Errors
/// From the model or the filter.
pub fn run_experiment(experiment: Experiment, filter: FilterKind) -> Result<Run> {
    let fixed = Schedule::Fixed(x_a());
    match experiment {
        Experiment::PA => run(&FoulingModel::<FoulingState>::new(fixed.clone(), ClosureBase::Cfd)?, &synthetic_measurements(&fixed, MEASUREMENT_SEEDS[0])?, filter, false),
        Experiment::PB => run(&FoulingModel::<FoulingState>::new(fixed.clone(), ClosureBase::Textbook)?, &synthetic_measurements(&fixed, MEASUREMENT_SEEDS[0])?, filter, true),
        Experiment::PD => run(&FoulingModel::<FoulingHotState>::new(fixed.clone(), ClosureBase::Cfd)?, &synthetic_measurements(&fixed, MEASUREMENT_SEEDS[0])?, filter, false),
        Experiment::PC => {
            let c = Schedule::Cycling(cycle_c());
            run(&FoulingModel::<FoulingBothState>::new(c.clone(), ClosureBase::Cfd)?, &synthetic_measurements(&c, MEASUREMENT_SEEDS[1])?, filter, false)
        }
    }
}

/// Every run as CSV: experiment, filter, step, truth, estimate, σ, NEES, NIS.
#[must_use]
pub fn traces_csv(runs: &[(Experiment, Run)]) -> String {
    let mut out = String::from("experiment,filter,step,r_f_truth_m2K_per_W,r_f_estimate_m2K_per_W,r_f_sigma_m2K_per_W,nees,nis\n");
    for (e, r) in runs {
        for (k, v) in r.trace.iter().enumerate() {
            out.push_str(&format!("{},{:?},{k},{:.6e},{:.6e},{:.6e},{:.4},{:.4}\n", e.label(), r.filter, v[0], v[1], v[2], v[3], v[4]));
        }
    }
    out
}

/// One panel per experiment: the true r_f (black), the UKF's estimate
/// (blue) and its 95 % band (shaded), in 1e-4 m² K/W against step.
#[must_use]
pub fn tracking_svg(runs: &[(Experiment, Run)]) -> String {
    let (pw, ph, pad) = (420.0, 220.0, 40.0);
    let ukf: Vec<&(Experiment, Run)> = runs.iter().filter(|(_, r)| r.filter == FilterKind::Unscented).collect();
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" font-family=\"sans-serif\" font-size=\"11\">\n<rect width=\"100%\" height=\"100%\" fill=\"white\"/>\n",
        2.0 * pw,
        ph * ukf.len().div_ceil(2) as f64
    );
    for (i, (e, r)) in ukf.iter().enumerate() {
        let (ox, oy) = ((i % 2) as f64 * pw, (i / 2) as f64 * ph);
        let lo = r.trace.iter().map(|v| v[1] - 1.96 * v[2]).fold(-1.0e-4, f64::min).max(-12.0e-4);
        let hi = r.trace.iter().map(|v| v[1] + 1.96 * v[2]).fold(6.0e-4, f64::max).min(16.0e-4);
        let sx = |k: usize| ox + pad + (pw - 1.5 * pad) * k as f64 / (STEPS - 1) as f64;
        let sy = |v: f64| oy + ph - pad + (ph - 1.6 * pad) * -((v.clamp(lo, hi) - lo) / (hi - lo));
        let mut band = String::new();
        for (k, v) in r.trace.iter().enumerate() {
            band.push_str(&format!("{:.1},{:.1} ", sx(k), sy(v[1] + 1.96 * v[2])));
        }
        for (k, v) in r.trace.iter().enumerate().rev() {
            band.push_str(&format!("{:.1},{:.1} ", sx(k), sy(v[1] - 1.96 * v[2])));
        }
        let line = |idx: usize| r.trace.iter().enumerate().map(|(k, v)| format!("{:.1},{:.1}", sx(k), sy(v[idx]))).collect::<Vec<_>>().join(" ");
        svg.push_str(&format!("<polygon points=\"{band}\" fill=\"#9ecae1\" opacity=\"0.6\"/>\n"));
        svg.push_str(&format!("<polyline points=\"{}\" fill=\"none\" stroke=\"black\" stroke-width=\"1.5\"/>\n", line(0)));
        svg.push_str(&format!("<polyline points=\"{}\" fill=\"none\" stroke=\"#2171b5\" stroke-width=\"1.2\"/>\n", line(1)));
        svg.push_str(&format!("<line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"grey\"/>\n", sx(0), sy(0.0), sx(STEPS - 1), sy(0.0)));
        svg.push_str(&format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" font-weight=\"bold\">{} (UKF): r_f in 1e-4 m2K/W, axis {:.1} to {:.1}, 200 steps</text>\n<text x=\"{:.1}\" y=\"{:.1}\">black truth, blue estimate, shaded 95 %; RMSE(31-200) {:.2e}; grey r_f = 0</text>\n",
            ox + 8.0,
            oy + 14.0,
            e.label(),
            lo * 1e4,
            hi * 1e4,
            ox + 8.0,
            oy + 28.0,
            r.rmse_after_burn_in
        ));
    }
    svg.push_str("</svg>\n");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pieces: the truth curve, the closures at a training point (the
    /// CFD's) and a held-out point (refused), the outlets' energy balance
    /// and their response to r_f (signal ≈ 1 K at r_∞, V&V record), and the
    /// model through the trait.
    #[test]
    fn construction_pieces() {
        assert!((truth_r_f(10_000) - R_INF).abs() < 1e-12 && truth_r_f(0) > 0.0 && truth_r_f(0) < truth_r_f(1));
        let x = x_a();
        let cfd = base_nusselt(&x, ClosureBase::Cfd).unwrap();
        let tb = base_nusselt(&x, ClosureBase::Textbook).unwrap();
        println!("x_A Nu CFD {cfd:?}, textbook {tb:?}");
        assert!(base_nusselt(&DoublePipeInput::si(30.0, 15.0, 10.0, 0.2), ClosureBase::Cfd).is_err());
        let clean = outlets(&x, cfd, 0.0, 0.0, 0.0).unwrap();
        let fouled = outlets(&x, cfd, 0.0, 0.0, R_INF).unwrap();
        let (ch, cc) = x.capacity_rates();
        assert!((ch * (fixed::T_HOT_IN - clean[0]) - cc * (clean[1] - fixed::T_COLD_IN)).abs() < 1e-9);
        println!("outlets clean {clean:?}, fouled {fouled:?}: signal {:.3} K / {:.3} K", fouled[0] - clean[0], clean[1] - fouled[1]);
        assert!(fouled[0] > clean[0] && fouled[1] < clean[1]);
        assert!(outlets(&x, cfd, 0.0, 0.0, -1.0).is_err());
        let m: FoulingModel<FoulingState> = FoulingModel::new(Schedule::Fixed(x), ClosureBase::Cfd).unwrap();
        let st = FoulingState::from_si(&[R_INF]).unwrap();
        assert_eq!(m.propagate(&st, 3).unwrap(), st);
        assert_eq!(m.observe(&st, 0).unwrap().to_si(), fouled.to_vec());
        assert_eq!(Schedule::Cycling(cycle_c()).at(10), cycle_c()[1]);
        assert_eq!(cycle_c().len(), 9);
        let zs = synthetic_measurements(&Schedule::Fixed(x), 7).unwrap();
        assert_eq!(zs.len(), STEPS);
        assert_eq!(zs, synthetic_measurements(&Schedule::Fixed(x), 7).unwrap(), "deterministic");
    }

    /// **The pilot (gh:#1191).** Four experiments × three filters on 200
    /// steps of synthetic outlet temperatures (V&V record
    /// `double_pipe_filtering_gh1191.md`, predictions F1–F10 written before
    /// this ran). Asserts only what holds regardless of the predictions
    /// (finite numbers, deterministic reruns); the predictions are judged in
    /// the record, misses included. Writes the traces and the figure when
    /// `GH1191_WRITE=1`.
    ///
    /// **Results (2026-11-04), UKF, r_f RMSE over steps 31–200, NEES, NIS:**
    /// - P-A: 8.65e-6 m² K/W, 1.14, 1.81. The EKF and EnKF are within 1 %.
    /// - P-B: biased by −4.37e-4, NEES ≈ 1900, NIS 1.81. The innovations
    ///   cannot see the closure error.
    /// - P-D: 9.05e-4, NEES 99, NIS 1.61. Inconsistent on the ridge; the EKF
    ///   is consistent at NEES 1.06, but r_f is unidentified (σ 2.4e-4).
    /// - P-C: 1.42e-5, 1.45, 1.85.
    ///
    /// All 12 runs take 2.1 s. Predictions F1, F3–F6, F8 (UKF and EKF), F9
    /// and F10 were met; F2 half missed; F7 mostly missed; F8 missed for the
    /// EnKF.
    #[test]
    fn pilot_four_experiments_three_filters() {
        let t0 = std::time::Instant::now();
        let mut runs = Vec::new();
        for e in Experiment::ALL {
            for f in [FilterKind::Unscented, FilterKind::Extended, FilterKind::Ensemble] {
                let t = std::time::Instant::now();
                let r = run_experiment(e, f).unwrap();
                println!(
                    "{} {:?}: RMSE(31-200) {:.3e} all {:.3e} bias {:+.3e} NEES {:.3} NIS {:.3} cov95 {:.3} corr(h,rf) {:.3} final {:?} ({:.1} s)",
                    e.label(),
                    f,
                    r.rmse_after_burn_in,
                    r.rmse_all,
                    r.bias_after_burn_in,
                    r.nees_mean,
                    r.nis_mean,
                    r.coverage_95,
                    r.final_corr_h_rf,
                    r.final_mean,
                    t.elapsed().as_secs_f64()
                );
                assert!(r.rmse_all.is_finite() && r.nis_mean.is_finite() && r.nees_mean.is_finite());
                runs.push((e, r));
            }
        }
        println!("pilot total {:.1} s", t0.elapsed().as_secs_f64());
        // Deterministic rerun (traces and final means; `Run` holds a NaN
        // correlation for one-parameter states, so not `==` on the whole).
        let again = run_experiment(Experiment::PA, FilterKind::Ensemble).unwrap();
        assert_eq!((again.trace, again.final_mean), (runs[2].1.trace.clone(), runs[2].1.final_mean.clone()), "rerun is identical");
        for (e, r) in &runs {
            let last = r.trace.last().unwrap();
            println!("{} {:?}: final sigma(r_f) {:.3e}", e.label(), r.filter, last[2]);
        }
        let csv = traces_csv(&runs);
        let svg = tracking_svg(&runs);
        assert_eq!(csv.lines().count(), 1 + 12 * STEPS);
        assert!(svg.contains("P-C (UKF)"));
        if std::env::var("GH1191_WRITE").as_deref() == Ok("1") {
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/double_pipe_filtering_gh1191");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("traces.csv"), csv).unwrap();
            std::fs::write(dir.join("r_f_tracking.svg"), svg).unwrap();
        }
    }
}
