//! # N-group point reactor kinetics (gh:#1082)
//!
//! The one point-kinetics core in this crate. [`PointKinetics`] carries any
//! number `N_d >= 0` of delayed-neutron precursor groups;
//! [`SixGroupPRKE`](super::six_group_precursor_prke::SixGroupPRKE) is now a
//! fixed-size front end that delegates its arithmetic to the functions in
//! this module, and is pinned **bit-identical** to its pre-gh:#1082 self by
//! `zero_power_prke::six_group_bit_identity_pin`.
//!
//! ## Equations
//!
//! RELAP-7 Theory Manual (INL/EXT-14-31366), §7.1, Eqs (412)-(413), the
//! standard point-kinetics system in neutron density:
//!
//! ```text
//! dn/dt   = (rho - beta)/Lambda n + sum_i lambda_i C_i + S
//! dC_i/dt = beta_i/Lambda n - lambda_i C_i            i = 1..N_d
//! ```
//!
//! with `n` and `C_i` in /m³, `S` in /m³/s, `beta = sum_i beta_i`. RELAP-7
//! writes `beta_i = beta f_i`; [`PointKinetics::with_total_delayed_fraction`]
//! uses exactly that split to rescale a group set to a `beta_eff` computed
//! elsewhere (for instance `outram_mc_libs::physics::kinetics`, whose
//! `KineticsParameters` gives a total `beta_eff` and `lambda_generation`
//! but no group split; this crate does not depend on it, the caller passes
//! the two numbers).
//!
//! ## Schemes
//!
//! - **Implicit** ([`PointKinetics::step_implicit`]): backward Euler, one
//!   dense `(N_d+1)x(N_d+1)` solve per step with this crate's
//!   [`SquareMatrix`](crate::matrix::SquareMatrix) (LU with partial
//!   pivoting). This is the same scheme as GeN-Foam's point kinetics
//!   (`src/classes/neutronics/pointKinetics/include/solvePointKinetics.H`,
//!   GeN-Foam commit 652b3da, ported in
//!   `outram-foam-appbuilder-lib::genfoam::neutronics::point_kinetics`),
//!   which carries `[P, C_1..C_N, S]` in power units and an extra source row;
//!   here the source enters the right-hand side and the unknowns are
//!   densities. GeN-Foam's subcritical index `zeta` is not carried: pass
//!   `rho - zeta` as the reactivity. The two cores are therefore the same
//!   method in different units; teh-o-prke does not depend on the appbuilder
//!   crate.
//! - **Explicit** ([`PointKinetics::step_explicit`]): one fixed RKF45 step
//!   (this crate's OpenFOAM-derived [`RKF45`]). Stiff: use only with
//!   `dt << Lambda/beta`.
//!
//! > Research, education and V&V only. AI draft, unreviewed.

use uom::si::f64::*;
use uom::si::frequency::hertz;
use uom::si::ratio::ratio;
use uom::si::time::second;
use uom::si::volumetric_number_density::per_cubic_meter;
use uom::si::volumetric_number_rate::per_cubic_meter_second;
use uom::ConstZero;

use super::six_group_precursor_prke::six_group_constants::FissioningNuclideType;
use super::six_group_precursor_prke::DecayConstant;
use crate::matrix::SquareMatrix;
use crate::teh_o_prke_error::TehOPrkeError;
use crate::time_stepping::openfoam_rfk45::RKF45;

/// Backward-Euler step of the N-group PRKE, in place.
///
/// `state` is `[n, C_1, .., C_N]` (length `N+1`), `decay_constants` and
/// `delayed_fractions` have length `N`. The arithmetic (operation order,
/// `uom` operations, matrix layout) is exactly that of the pre-gh:#1082
/// hand-unrolled six-group solver, so six groups reproduce it bit for bit.
pub(crate) fn backward_euler_step(
    decay_constants: &[DecayConstant],
    delayed_fractions: &[Ratio],
    state: &mut [VolumetricNumberDensity],
    timestep: Time,
    reactivity: Ratio,
    neutron_generation_time: Time,
    background_source_rate: VolumetricNumberRate,
) -> Result<(), TehOPrkeError> {
    let groups = decay_constants.len();
    check_shapes(groups, delayed_fractions.len(), state.len())?;
    let size = groups + 1;

    let timestep_to_neutron_generation_time_ratio: Ratio = timestep / neutron_generation_time;
    let total_delayed_fraction: Ratio = delayed_fractions.iter().copied().sum();
    let top_left_coefficient = Ratio::new::<ratio>(1.0)
        - timestep_to_neutron_generation_time_ratio * (reactivity - total_delayed_fraction);

    let mut matrix = SquareMatrix::new(size);
    matrix.set(0, 0, top_left_coefficient.get::<ratio>());
    for i in 0..groups {
        let delta_t_lambda: Ratio = timestep * decay_constants[i];
        matrix.set(0, i + 1, (-delta_t_lambda).get::<ratio>());
        matrix.set(
            i + 1,
            0,
            (-timestep_to_neutron_generation_time_ratio * delayed_fractions[i]).get::<ratio>(),
        );
        matrix.set(
            i + 1,
            i + 1,
            (Ratio::new::<ratio>(1.0) + delta_t_lambda).get::<ratio>(),
        );
    }

    let background_source_term: VolumetricNumberDensity =
        (background_source_rate * timestep).into();
    let mut rhs: Vec<f64> = Vec::with_capacity(size);
    rhs.push((background_source_term + state[0]).get::<per_cubic_meter>());
    for c in state.iter().skip(1) {
        rhs.push(c.get::<per_cubic_meter>());
    }

    let solution = matrix
        .solve(&rhs)
        .map_err(|e| TehOPrkeError::ShapeMismatch(e.to_string()))?;
    for (slot, value) in state.iter_mut().zip(solution) {
        *slot = VolumetricNumberDensity::new::<per_cubic_meter>(value);
    }
    Ok(())
}

/// One fixed-size RKF45 step of the N-group PRKE, in place. Same layout as
/// [`backward_euler_step`]; same arithmetic as the pre-gh:#1082 six-group
/// explicit solver (the decay-rate sum starts from group 1, not from zero).
pub(crate) fn rkf45_step(
    decay_constants: &[DecayConstant],
    delayed_fractions: &[Ratio],
    state: &mut [VolumetricNumberDensity],
    timestep: Time,
    reactivity: Ratio,
    neutron_generation_time: Time,
    background_source_rate: VolumetricNumberRate,
) -> Result<(), TehOPrkeError> {
    let groups = decay_constants.len();
    check_shapes(groups, delayed_fractions.len(), state.len())?;

    let total_delayed_fraction: Ratio = delayed_fractions.iter().copied().sum();
    let rho_minus_beta_over_big_lambda: Frequency =
        (reactivity - total_delayed_fraction) / neutron_generation_time;
    let beta_by_big_lambda: Vec<Frequency> = delayed_fractions
        .iter()
        .map(|beta| *beta / neutron_generation_time)
        .collect();

    let ode = |_t: f64, y: &Vec<f64>| -> Vec<f64> {
        let n = VolumetricNumberDensity::new::<per_cubic_meter>(y[0]);
        let decay_rates: Vec<VolumetricNumberRate> = (0..groups)
            .map(|i| {
                (decay_constants[i] * VolumetricNumberDensity::new::<per_cubic_meter>(y[i + 1]))
                    .into()
            })
            .collect();
        let mut dydt = vec![0.0; groups + 1];
        let mut dndt: VolumetricNumberRate = (rho_minus_beta_over_big_lambda * n).into();
        if let Some((first, rest)) = decay_rates.split_first() {
            let total_decay_rate = rest.iter().fold(*first, |acc, r| acc + *r);
            dndt += total_decay_rate;
        }
        dndt += background_source_rate;
        dydt[0] = dndt.get::<per_cubic_meter_second>();
        for i in 0..groups {
            let mut dcdt: VolumetricNumberRate = (beta_by_big_lambda[i] * n).into();
            dcdt -= decay_rates[i];
            dydt[i + 1] = dcdt.get::<per_cubic_meter_second>();
        }
        dydt
    };

    let y0: Vec<f64> = state.iter().map(|v| v.get::<per_cubic_meter>()).collect();
    let y1 = RKF45::solve_functional_prog_single_stepsize_no_stepsize_adjust(
        timestep.get::<second>(),
        y0,
        timestep.get::<second>(),
        ode,
    );
    for (slot, value) in state.iter_mut().zip(y1) {
        *slot = VolumetricNumberDensity::new::<per_cubic_meter>(value);
    }
    Ok(())
}

fn check_shapes(groups: usize, fractions: usize, state: usize) -> Result<(), TehOPrkeError> {
    if fractions != groups || state != groups + 1 {
        return Err(TehOPrkeError::ShapeMismatch(format!(
            "point kinetics: {groups} decay constants, {fractions} delayed fractions, \
             state length {state} (expected {})",
            groups + 1
        )));
    }
    Ok(())
}

/// N-group point reactor kinetics state (RELAP-7 §7.1, Eqs 412-413).
///
/// Holds the delayed-neutron data and the state `[n, C_1..C_N]`. Reactivity,
/// generation time and source are per-step inputs, so feedback and a
/// time-varying `Lambda` are the caller's to supply.
///
/// ```
/// use teh_o_prke::prelude::*;
/// use uom::si::f64::*;
/// use uom::si::ratio::ratio;
/// use uom::si::time::second;
/// use uom::si::volumetric_number_density::per_cubic_meter;
/// use uom::ConstZero;
///
/// let lambda_gen = Time::new::<second>(1.0e-4);
/// let mut pk = PointKinetics::from_nuclide(FissioningNuclideType::U235);
/// pk.set_critical_equilibrium(VolumetricNumberDensity::new::<per_cubic_meter>(1.0e12), lambda_gen);
/// pk.step_implicit(
///     Time::new::<second>(0.01), Ratio::new::<ratio>(0.0), lambda_gen, VolumetricNumberRate::ZERO,
/// )?;
/// assert!((pk.neutron_density().get::<per_cubic_meter>() / 1.0e12 - 1.0).abs() < 1e-12);
/// # Ok::<(), TehOPrkeError>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct PointKinetics {
    decay_constants: Vec<DecayConstant>,
    delayed_fractions: Vec<Ratio>,
    /// `[n, C_1, .., C_N]`
    state: Vec<VolumetricNumberDensity>,
}

impl PointKinetics {
    /// A state with the given group data, `n = 1 /m³` and no precursors (the
    /// same start as `SixGroupPRKE::default()`).
    ///
    /// Errors if the two vectors differ in length, a decay constant is not
    /// strictly positive, or a delayed fraction is negative. Zero groups is
    /// allowed: prompt-only kinetics, the Nordheim-Fuchs assumption.
    pub fn new(
        delayed_fractions: Vec<Ratio>,
        decay_constants: Vec<DecayConstant>,
    ) -> Result<Self, TehOPrkeError> {
        if delayed_fractions.len() != decay_constants.len() {
            return Err(TehOPrkeError::ShapeMismatch(format!(
                "{} delayed fractions but {} decay constants",
                delayed_fractions.len(),
                decay_constants.len()
            )));
        }
        for lambda in &decay_constants {
            let v = lambda.get::<hertz>();
            if !(v > 0.0) {
                return Err(TehOPrkeError::NonPositiveDelayedDecayConstant(v));
            }
        }
        for beta in &delayed_fractions {
            if !(beta.get::<ratio>() >= 0.0) {
                return Err(TehOPrkeError::GenericStringError(format!(
                    "delayed fraction must be >= 0, got {}",
                    beta.get::<ratio>()
                )));
            }
        }
        let mut state = vec![VolumetricNumberDensity::ZERO; decay_constants.len() + 1];
        state[0] = VolumetricNumberDensity::new::<per_cubic_meter>(1.0);
        Ok(Self {
            decay_constants,
            delayed_fractions,
            state,
        })
    }

    /// The six-group data this crate tabulates for `nuclide` (the same data
    /// `SixGroupPRKE` uses), `n = 1 /m³`, no precursors.
    pub fn from_nuclide(nuclide: FissioningNuclideType) -> Self {
        Self {
            decay_constants: nuclide.get_decay_constant_array().to_vec(),
            delayed_fractions: nuclide.get_delayed_fraction_array().to_vec(),
            state: {
                let mut s = vec![VolumetricNumberDensity::ZERO; 7];
                s[0] = VolumetricNumberDensity::new::<per_cubic_meter>(1.0);
                s
            },
        }
    }

    /// Rescale the group fractions so they sum to `beta_eff`, keeping each
    /// group's share `f_i = beta_i / beta` (RELAP-7 Eq 413 writes
    /// `beta_i = beta f_i`). Use it to apply a computed `beta_eff`, e.g. from
    /// `outram_mc_libs::physics::kinetics::KineticsParameters::beta_eff`.
    /// Precursor densities are left as they are; call
    /// [`Self::set_critical_equilibrium`] afterwards if needed.
    pub fn with_total_delayed_fraction(mut self, beta_eff: Ratio) -> Self {
        let total = self.total_delayed_fraction();
        if total.get::<ratio>() > 0.0 {
            for beta in self.delayed_fractions.iter_mut() {
                *beta = beta_eff * (*beta / total);
            }
        }
        self
    }

    /// Number of delayed groups `N_d`.
    pub fn group_count(&self) -> usize {
        self.decay_constants.len()
    }

    /// Group decay constants `lambda_i`.
    pub fn decay_constants(&self) -> &[DecayConstant] {
        &self.decay_constants
    }

    /// Group delayed fractions `beta_i`.
    pub fn delayed_fractions(&self) -> &[Ratio] {
        &self.delayed_fractions
    }

    /// `beta = sum_i beta_i`.
    pub fn total_delayed_fraction(&self) -> Ratio {
        self.delayed_fractions.iter().copied().sum()
    }

    /// Neutron density `n`.
    pub fn neutron_density(&self) -> VolumetricNumberDensity {
        self.state[0]
    }

    /// Precursor densities `C_1..C_N`.
    pub fn precursor_densities(&self) -> &[VolumetricNumberDensity] {
        &self.state[1..]
    }

    /// Set `n` and every `C_i` to the zero-reactivity, zero-source steady
    /// state, `C_i = beta_i n / (Lambda lambda_i)` (Eq 413 with `dC_i/dt = 0`).
    pub fn set_critical_equilibrium(
        &mut self,
        neutron_density: VolumetricNumberDensity,
        neutron_generation_time: Time,
    ) {
        self.state[0] = neutron_density;
        for i in 0..self.group_count() {
            let lambda_big_lambda: Ratio = neutron_generation_time * self.decay_constants[i];
            self.state[i + 1] =
                (self.delayed_fractions[i] * neutron_density / lambda_big_lambda).into();
        }
    }

    /// Advance one step by backward Euler (unconditionally stable).
    pub fn step_implicit(
        &mut self,
        timestep: Time,
        reactivity: Ratio,
        neutron_generation_time: Time,
        background_source_rate: VolumetricNumberRate,
    ) -> Result<(), TehOPrkeError> {
        backward_euler_step(
            &self.decay_constants,
            &self.delayed_fractions,
            &mut self.state,
            timestep,
            reactivity,
            neutron_generation_time,
            background_source_rate,
        )
    }

    /// Advance one step with a single fixed RKF45 step. Stiff: unstable
    /// unless `timestep` is well below `Lambda / beta`.
    pub fn step_explicit(
        &mut self,
        timestep: Time,
        reactivity: Ratio,
        neutron_generation_time: Time,
        background_source_rate: VolumetricNumberRate,
    ) -> Result<(), TehOPrkeError> {
        rkf45_step(
            &self.decay_constants,
            &self.delayed_fractions,
            &mut self.state,
            timestep,
            reactivity,
            neutron_generation_time,
            background_source_rate,
        )
    }

    /// The in-hour equation: the reactivity for which the PRKE has the
    /// asymptotic solution `n ~ exp(omega t)`,
    ///
    /// ```text
    /// rho(omega) = omega Lambda + sum_i beta_i omega / (omega + lambda_i)
    /// ```
    ///
    /// (substitute `n, C_i ~ exp(omega t)` into Eqs 412-413 with `S = 0`).
    pub fn inhour_reactivity(&self, omega: Frequency, neutron_generation_time: Time) -> Ratio {
        let w = omega.get::<hertz>();
        let mut rho = w * neutron_generation_time.get::<second>();
        for i in 0..self.group_count() {
            rho += self.delayed_fractions[i].get::<ratio>() * w
                / (w + self.decay_constants[i].get::<hertz>());
        }
        Ratio::new::<ratio>(rho)
    }

    /// The asymptotic growth rate `omega` (the largest root of the in-hour
    /// equation) for a step reactivity `rho`; the stable period is `1/omega`.
    ///
    /// The right-hand side of the in-hour equation increases monotonically
    /// for `omega > -lambda_min`, so the largest root is unique there and is
    /// found by bisection to round-off (200 halvings). Zero groups gives the
    /// prompt-only root `rho/Lambda` exactly.
    pub fn inhour_growth_rate(
        &self,
        reactivity: Ratio,
        neutron_generation_time: Time,
    ) -> Frequency {
        let rho = reactivity.get::<ratio>();
        let big_lambda = neutron_generation_time.get::<second>();
        if self.group_count() == 0 {
            return Frequency::new::<hertz>(rho / big_lambda);
        }
        let lambda_min = self
            .decay_constants
            .iter()
            .map(|l| l.get::<hertz>())
            .fold(f64::INFINITY, f64::min);
        let f = |w: f64| {
            self.inhour_reactivity(Frequency::new::<hertz>(w), neutron_generation_time)
                .get::<ratio>()
                - rho
        };
        let mut lo = -lambda_min;
        let mut hi = rho.abs() / big_lambda + 1.0;
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if mid <= lo || mid >= hi {
                break;
            }
            // just above -lambda_min the sum term tends to -infinity
            if f(mid) > 0.0 {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        Frequency::new::<hertz>(0.5 * (lo + hi))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use petir::mathf::RealMath;

    fn s(x: f64) -> Time {
        Time::new::<second>(x)
    }
    fn r(x: f64) -> Ratio {
        Ratio::new::<ratio>(x)
    }

    /// Shapes and validation: mismatched lengths and non-positive decay
    /// constants are refused; zero groups is accepted.
    #[test]
    fn construction_validates_inputs() {
        assert!(PointKinetics::new(vec![r(0.001)], vec![]).is_err());
        assert!(PointKinetics::new(vec![r(0.001)], vec![Frequency::new::<hertz>(0.0)]).is_err());
        assert!(PointKinetics::new(vec![r(-0.001)], vec![Frequency::new::<hertz>(0.1)]).is_err());
        let pk = PointKinetics::new(vec![], vec![]).unwrap();
        assert_eq!(pk.group_count(), 0);
        let mut bad = PointKinetics::new(vec![], vec![]).unwrap();
        bad.state.push(VolumetricNumberDensity::ZERO);
        assert!(bad
            .step_implicit(s(1e-3), r(0.0), s(1e-4), VolumetricNumberRate::ZERO)
            .is_err());
        assert!(bad
            .step_explicit(s(1e-3), r(0.0), s(1e-4), VolumetricNumberRate::ZERO)
            .is_err());
    }

    /// `PointKinetics::from_nuclide` stepped through the bit-identity script
    /// gives exactly the `SixGroupPRKE` numbers (both front ends share one
    /// core). Prediction: zero bits differ.
    #[test]
    fn six_group_point_kinetics_matches_six_group_prke_bitwise() {
        use super::super::six_group_precursor_prke::SixGroupPRKE;
        let mut six = SixGroupPRKE::default();
        let mut pk = PointKinetics::from_nuclide(FissioningNuclideType::U235);
        let src = VolumetricNumberRate::new::<per_cubic_meter_second>(3.0);
        for (k, rho) in [0.0, 0.002, -0.004, 0.007, 0.0].iter().enumerate() {
            let dt = s(0.01 * (k as f64 + 1.0));
            six.step_implicit(dt, r(*rho), s(1e-4), src).unwrap();
            pk.step_implicit(dt, r(*rho), s(1e-4), src).unwrap();
            assert_eq!(
                six.precursor_and_neutron_pop_and_source_array.to_vec(),
                pk.state
            );
        }
    }

    /// Rescaling to a computed `beta_eff` keeps the group shares.
    /// ~~Hand evaluation: U-235 shares are 0.00021/0.0065 etc; rescaled to
    /// 0.0070 the first group is 0.00021*0.0070/0.0065 = 2.26154e-4.~~
    /// ~~**Result (2026-11-01):** total 0.0070 to 1e-18, group 1 2.26154e-4.~~
    /// **CHANGED 2026-11-10 (gh:#1319):** U-235's default is now
    /// ENDF/B-VIII.0, whose shares are the abundances `p_i` over their sum
    /// (0.99999985). Hand evaluation: group 1 is
    /// 0.03500755 * 0.0070 / 0.99999985 = 2.450529e-4.
    /// **Result (2026-11-10):** total 0.0070 to 1e-18, group 1 2.450529e-4.
    #[test]
    fn rescale_to_beta_eff_keeps_group_shares() {
        let pk = PointKinetics::from_nuclide(FissioningNuclideType::U235)
            .with_total_delayed_fraction(r(0.0070));
        assert!((pk.total_delayed_fraction().get::<ratio>() - 0.0070).abs() < 1e-18);
        let p_sum = 0.03500755 + 0.1806982 + 0.1725102 + 0.3867821 + 0.1585752 + 0.0664266;
        assert!(
            (pk.delayed_fractions()[0].get::<ratio>() - 0.03500755 * 0.0070 / p_sum).abs() < 1e-18
        );
        assert!((pk.delayed_fractions()[0].get::<ratio>() - 2.450529e-4).abs() < 1e-10);
        assert_eq!(pk.decay_constants().len(), 6);
        assert_eq!(pk.precursor_densities().len(), 6);
    }

    /// Critical equilibrium holds exactly-ish at zero reactivity: 1000 steps
    /// of 0.1 s move n by less than 1e-12 relative.
    /// **Result (2026-11-01):** passes.
    #[test]
    fn critical_equilibrium_holds() {
        let lam = s(1e-4);
        let mut pk = PointKinetics::from_nuclide(FissioningNuclideType::Pu239);
        let n0 = VolumetricNumberDensity::new::<per_cubic_meter>(5.0e13);
        pk.set_critical_equilibrium(n0, lam);
        for _ in 0..1000 {
            pk.step_implicit(s(0.1), r(0.0), lam, VolumetricNumberRate::ZERO)
                .unwrap();
        }
        assert!((pk.neutron_density() / n0).get::<ratio>().r_ln().abs() < 1e-12);
    }

    /// Measured asymptotic `omega` from a long implicit transient, from
    /// `ln n` between two late times.
    fn measured_omega(
        pk: &mut PointKinetics,
        rho: f64,
        lam: f64,
        dt: f64,
        t1: f64,
        t2: f64,
    ) -> f64 {
        let n_steps_1 = (t1 / dt).round() as usize;
        let n_steps_2 = (t2 / dt).round() as usize;
        let mut ln1 = 0.0;
        for k in 1..=n_steps_2 {
            pk.step_implicit(s(dt), r(rho), s(lam), VolumetricNumberRate::ZERO)
                .unwrap();
            if k == n_steps_1 {
                ln1 = pk.neutron_density().get::<per_cubic_meter>().r_ln();
            }
        }
        let ln2 = pk.neutron_density().get::<per_cubic_meter>().r_ln();
        (ln2 - ln1) / (t2 - t1)
    }

    /// **V&V: in-hour equation, six groups (gh:#1082).**
    ///
    /// *Methodology.* U-235 six-group data (~~beta = 0.0065~~ **since
    /// 2026-11-10, gh:#1319: the ENDF/B-VIII.0 default, beta = 0.0065230**),
    /// `Lambda = 1e-4 s`,
    /// start at critical equilibrium, apply a step reactivity and integrate by
    /// backward Euler at `dt = 1 ms` to 500 s. The asymptotic growth rate is
    /// measured as `d ln n / dt` between 400 s and 500 s, when every other
    /// root of the in-hour equation has decayed by at least `e^-12`. Reference:
    /// the largest root of `rho = omega Lambda + sum beta_i omega/(omega +
    /// lambda_i)` by bisection ([`PointKinetics::inhour_growth_rate`]); the
    /// in-hour equation is derived from the same Eqs 412-413, so this verifies
    /// the time integration, not the data.
    ///
    /// *Prediction (written before running).* Backward Euler's discrete growth
    /// rate is `ln(1/(1 - omega dt))/dt ~ omega (1 + omega dt / 2)`, so the
    /// relative error should be about `omega dt / 2 ~ 1e-5` for the positive
    /// cases; pass criterion 1e-3 relative. For `rho = -0.001` the root lies
    /// between `-lambda_1` and 0.
    ///
    /// *Results, first run (2026-11-01).* Positive steps agreed as predicted
    /// (rho = 0.001: 9.1e-6; rho = 0.003: 7.0e-5). **The negative step missed:
    /// 6.7e-3 relative**, against the 1e-3 criterion. The fault was in the
    /// protocol, not the solver: the "every other root decayed by e^-12"
    /// claim holds for positive steps (the gap to the next root exceeds
    /// 0.03 /s) but not for rho = -0.001, where the largest root is
    /// -7.69e-3 /s and the second root sits between -lambda_2 and -lambda_1,
    /// only ~0.01 /s below it, so at 400 s its term had decayed by about e^-4.
    /// The criterion was kept; the window was moved.
    ///
    /// *Protocol, revised.* For each step the second root is computed too, and
    /// the window starts where the gap times `t_1` is at least 15
    /// (`t_1 = max(400 s, 15 / gap)`, `t_2 = t_1 + 100 s`). Prediction for the
    /// negative step under the revised protocol: ~|omega| dt / 2 ~ 4e-6.
    ///
    /// *Results, revised protocol (2026-11-01):* rho = +0.001: omega
    /// 1.81870702e-2 /s (period 54.98 s), PRKE 1.81872357e-2, rel 9.1e-6;
    /// rho = +0.003: 1.40406076e-1 /s (7.122 s), PRKE 1.40415934e-1, rel
    /// 7.0e-5; rho = -0.001: -7.69269389e-3 /s (-130.0 s), window from
    /// 1831 s, PRKE -7.69266472e-3, rel -3.8e-6 (predicted ~4e-6). Also in
    /// `verification_and_validation/prke_analytic_checks.md`.
    ///
    /// *Re-measured 2026-11-10 (gh:#1319), U-235 data now ENDF/B-VIII.0*
    /// (`cargo test --release -p teh-o-prke --lib inhour_equation_six_group
    /// -- --nocapture`). The data moved, so the roots moved; the integration
    /// error stayed of the same order, as predicted on the issue:
    /// rho = +0.001: omega ~~1.81870702e-2~~ **2.22678814e-2** /s (period
    /// ~~54.98~~ **44.91** s, +22.4 % in omega), PRKE 2.22681294e-2, rel
    /// 1.11e-5; rho = +0.003: ~~1.40406076e-1~~ **1.72959426e-1** /s
    /// (~~7.122~~ **5.782** s, +23.2 %), PRKE 1.72974385e-1, rel 8.6e-5;
    /// rho = -0.001: ~~-7.69269389e-3~~ **-8.56679171e-3** /s (~~-130.0~~
    /// **-116.7** s), window from 1562 s, PRKE -8.56675560e-3, rel -4.2e-6.
    /// The predicted omega rise for small rho was +15 to +18 %; the measured
    /// +22 % is a miss in size (at omega ~ 0.02 /s the in-hour equation is
    /// not in its small-omega limit, where only `sum beta_i/lambda_i`
    /// counts: group 1's lambda is 0.013 /s, the same order as omega).
    #[test]
    fn inhour_equation_six_group() {
        let lam = 1.0e-4;
        for rho in [0.001, 0.003, -0.001] {
            let mut pk = PointKinetics::from_nuclide(FissioningNuclideType::U235);
            pk.set_critical_equilibrium(
                VolumetricNumberDensity::new::<per_cubic_meter>(1.0),
                s(lam),
            );
            let reference = pk.inhour_growth_rate(r(rho), s(lam)).get::<hertz>();
            // second root: bisection between the two smallest poles
            let l: Vec<f64> = pk
                .decay_constants()
                .iter()
                .map(|x| x.get::<hertz>())
                .collect();
            let (mut lo, mut hi) = (-l[1], -l[0]);
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                let f = pk
                    .inhour_reactivity(Frequency::new::<hertz>(mid), s(lam))
                    .get::<ratio>()
                    - rho;
                if f > 0.0 {
                    hi = mid
                } else {
                    lo = mid
                }
            }
            let gap = reference - 0.5 * (lo + hi);
            let t1 = (15.0 / gap).max(400.0);
            let measured = measured_omega(&mut pk, rho, lam, 1.0e-3, t1, t1 + 100.0);
            let rel = (measured - reference) / reference;
            println!(
                "inhour 6g rho={rho}: omega_ref={reference:.8e} /s (period {:.4} s), \
                 gap={gap:.4e} /s, window from {t1:.0} s, omega_PRKE={measured:.8e} /s, rel={rel:.3e}",
                1.0 / reference
            );
            assert!(rel.abs() < 1.0e-3, "rho={rho}: rel {rel:e}");
        }
    }

    /// **V&V: in-hour equation, one group, closed form.**
    ///
    /// *Methodology.* With one group the in-hour equation is the quadratic
    /// `Lambda w^2 + (Lambda lambda + beta - rho) w - rho lambda = 0`, largest
    /// root by formula. Checks (a) the bisection solver against the formula
    /// and (b) the PRKE (beta = 0.0065, lambda = 0.08 /s, Lambda = 1e-4 s,
    /// rho = 0.002, dt = 1 ms, measured 200-300 s) against it.
    ///
    /// *Prediction.* (a) agree to ~1e-12 relative; (b) to ~1e-5 relative.
    ///
    /// *Results (2026-11-01).* (a) closed form 3.5464557789e-2 /s, bisection
    /// equal to 1e-12; (b) PRKE 3.5465186672e-2 /s, rel 1.8e-5.
    #[test]
    fn inhour_equation_one_group_closed_form() {
        let (beta, lambda, big_l, rho): (f64, f64, f64, f64) = (0.0065, 0.08, 1.0e-4, 0.002);
        let b = big_l * lambda + beta - rho;
        let closed = (-b + (b * b + 4.0 * big_l * rho * lambda).sqrt()) / (2.0 * big_l);
        let mut pk =
            PointKinetics::new(vec![r(beta)], vec![Frequency::new::<hertz>(lambda)]).unwrap();
        let bisect = pk.inhour_growth_rate(r(rho), s(big_l)).get::<hertz>();
        assert!(
            ((bisect - closed) / closed).abs() < 1e-12,
            "{bisect} vs {closed}"
        );
        pk.set_critical_equilibrium(
            VolumetricNumberDensity::new::<per_cubic_meter>(1.0),
            s(big_l),
        );
        let measured = measured_omega(&mut pk, rho, big_l, 1.0e-3, 200.0, 300.0);
        let rel = (measured - closed) / closed;
        println!("inhour 1g: closed={closed:.10e} bisect={bisect:.10e} PRKE={measured:.10e} rel={rel:.3e}");
        assert!(rel.abs() < 1.0e-3);
        // zero groups: prompt-only root
        let p0 = PointKinetics::new(vec![], vec![]).unwrap();
        assert_eq!(p0.inhour_growth_rate(r(0.001), s(1e-3)).get::<hertz>(), 1.0);
    }

    /// The explicit RKF45 path at a stable step agrees with the implicit
    /// path on a gentle transient (rho = 0.001, dt = 1e-5 s, 1 s): both are
    /// consistent integrators of the same system.
    /// Prediction: agree to ~1e-4 relative (implicit error ~ omega dt / 2 on
    /// the prompt jump). **Result (2026-11-01):** passes at 1e-3.
    #[test]
    fn explicit_and_implicit_agree_at_small_steps() {
        let lam = s(1e-4);
        let mut a = PointKinetics::from_nuclide(FissioningNuclideType::U235);
        a.set_critical_equilibrium(VolumetricNumberDensity::new::<per_cubic_meter>(1.0), lam);
        let mut b = a.clone();
        for _ in 0..100_000 {
            a.step_implicit(s(1e-5), r(0.001), lam, VolumetricNumberRate::ZERO)
                .unwrap();
            b.step_explicit(s(1e-5), r(0.001), lam, VolumetricNumberRate::ZERO)
                .unwrap();
        }
        let rel = (a.neutron_density() / b.neutron_density()).get::<ratio>() - 1.0;
        assert!(rel.abs() < 1e-3, "{rel:e}");
    }
}
