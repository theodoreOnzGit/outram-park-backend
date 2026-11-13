//! Circulating-fuel point kinetics: delayed-neutron precursors drift out of
//! the core with the fuel and return after a loop transit (MSRE T-C1,
//! gh:#1131, epic gh:#1125).
//!
//! New physics beside [`super::n_group_prke::PointKinetics`], which it does
//! not modify. AI-drafted (Claude Opus 5.5 in Claude Code), untrusted until
//! reviewed; research, education and V&V only.
//!
//! # Model
//!
//! The standard lumped circulating-fuel point kinetics: one well-mixed core
//! with transit time `τ_c`, and an external loop with transit time `τ_L` in
//! which precursors decay but produce no neutrons that matter.
//!
//! ```text
//! dn/dt   = (ρ − β)/Λ n + Σ λ_i C_i + S
//! dC_i/dt = β_i/Λ n − λ_i C_i − C_i/τ_c + C_i(t − τ_L) e^{−λ_i τ_L} / τ_c
//! ```
//!
//! The last term is the core outlet concentration `τ_L` earlier, decayed for
//! the loop transit and mixed back in. `τ_c → ∞` recovers the stationary-fuel
//! equations of [`super::n_group_prke::PointKinetics`].
//!
//! # Analytic results the solver is checked against
//!
//! - **Steady-state circulation loss** ([`CirculatingPointKinetics::circulation_reactivity_loss`]):
//!   the reactivity that keeps the circulating core critical,
//!   `Δρ = Σ β_i [1 − λ_i / (λ_i + (1 − e^{−λ_i τ_L})/τ_c)]`.
//! - **Circulating in-hour equation** ([`CirculatingPointKinetics::inhour_reactivity`]):
//!   with `n, C_i ~ e^{ωt}`,
//!   `ρ(ω) = ωΛ + β − Σ λ_i β_i / (ω + λ_i + (1 − e^{−(ω+λ_i) τ_L})/τ_c)`.
//!
//! # Time stepping
//!
//! Backward Euler (first order, unconditionally stable for the prompt term).
//! The delayed return `C_i(t_{k+1} − τ_L)` is already in the past for any
//! step `dt ≤ τ_L`, so it is a known source. Each step is then closed form:
//! eliminating the `C_i` leaves one scalar equation for `n`. `τ_L` must be a
//! whole number of steps; [`CirculatingPointKinetics::new`] refuses
//! otherwise, rather than interpolating the history.
//!
//! # V&V (criteria pre-registered on gh:#1131; results 2026-11-09)
//!
//! **MSRE inputs**, ORNL-TM-0728 Table 5.4 (PDF pp. 137-138): core transit
//! 9.4 s, fuel-loop total 25.2 s (so τ_L = 15.8 s), Λ = 2.40e-4 s (35 %
//! U-235 fuel). Group data: the crate's U-235 six-group set, ~~β = 650 pcm~~
//! **since 2026-11-10 (gh:#1319) the ENDF/B-VIII.0 U-235 default, β =
//! 652.30 pcm**. Struck values are the 2026-11-09 results with the textbook
//! set; the bold ones were re-measured 2026-11-10 with
//! `cargo test --release -p teh-o-prke --lib circulating -- --nocapture`.
//!
//! | Check | Criterion | Result |
//! |---|---|---|
//! | V1 equilibrium at ρ = Δρ, 300 s at dt = 0.01 s | \|n/n0 − 1\| ≤ 1e-9 | ~~2.2e-16~~ **0** |
//! | V2 analytic Δρ | reported | ~~224.3505 pcm (β_circ = 425.6495 pcm)~~ **207.1463 pcm (β_circ = 445.1573 pcm)** |
//! | V3 asymptotic ω vs the circulating in-hour root, Δρ ± 50 pcm | 1 % at dt = 0.01 s, falling with dt | ~~+50 pcm: +0.0363 / +0.0181 / +0.0091 %; −50 pcm: −0.0079 / −0.0040 / −0.0020 %~~ **+50 pcm: +0.0447 / +0.0223 / +0.0112 %; −50 pcm: −0.0086 / −0.0043 / −0.0022 %** at dt = 0.04 / 0.02 / 0.01 s: first order |
//! | V4 τ_c = 1e12 s vs `PointKinetics::inhour_growth_rate` | 1e-9 relative | ~~8e-11~~ **7e-11** |
//!
//! **The V2 move was predicted with the wrong sign.** gh:#1319 predicted
//! the loss would grow 5-15 %; it fell 7.7 % (224.35 to 207.15 pcm). The
//! loss is carried by the long-lived groups, which decay mostly out of the
//! core; ENDF/B-VIII.0 moves abundance from group 2 (p = 0.181 against
//! the textbook 0.218) into the short-lived groups 5 and 6 (0.159 and 0.066
//! against 0.115 and 0.042), which barely leave the core before decaying.
//!
//! **Protocol correction (not a criterion change).** V3 first measured the
//! slope over the last 100 s of a 600 s run. At −50 pcm that read 0.53 % off
//! and did not fall with dt: the circulating loop's delay-induced modes had
//! not died away. The protocol was lengthened to 2400 s (last 400 s), and
//! the criterion was not moved.
//!
//! **What this does not show.** It verifies the solver against its own
//! equations. Whether the lumped model's ~~224~~ **207** pcm describes the MSRE (TM-0728
//! implies 310 pcm, importance-weighted, from its static and circulating
//! β_eff; the measured loss is MSRE N-4) is validation, gh:#1130.

use std::collections::VecDeque;

use petir::mathf::RealMath;
use uom::si::f64::*;
use uom::si::frequency::hertz;
use uom::si::ratio::ratio;
use uom::si::time::second;
use uom::si::volumetric_number_density::per_cubic_meter;
use uom::si::volumetric_number_rate::per_cubic_meter_second;

use crate::teh_o_prke_error::TehOPrkeError;
use crate::zero_power_prke::six_group_precursor_prke::DecayConstant;
use crate::zero_power_prke::six_group_precursor_prke::six_group_constants::FissioningNuclideType;

/// Circulating-fuel point kinetics with a fixed time step.
///
/// State: neutron density `n`, the core precursor densities `C_i`, and the
/// history of `C_i` over the last `τ_L` (one entry per step), which is what
/// returns from the loop.
#[derive(Debug, Clone, PartialEq)]
pub struct CirculatingPointKinetics {
    decay_constants: Vec<f64>,
    delayed_fractions: Vec<f64>,
    /// Core transit time `τ_c` \[s\].
    core_transit: f64,
    /// Loop transit time `τ_L` \[s\].
    loop_transit: f64,
    /// Fixed step `dt` \[s\]; `τ_L = loop_steps · dt`.
    dt: f64,
    loop_steps: usize,
    /// Neutron generation time `Λ` \[s\].
    generation_time: f64,
    n: f64,
    c: Vec<f64>,
    /// Core `C_i` at past steps, oldest first; `history[0]` left the core
    /// `τ_L` before the step being taken.
    history: VecDeque<Vec<f64>>,
}

impl CirculatingPointKinetics {
    /// Kinetics for `nuclide`'s six-group data (the crate's tables), with
    /// core transit `core_transit`, loop transit `loop_transit`, generation
    /// time `generation_time` and fixed step `timestep`.
    ///
    /// Errors if a time is not strictly positive, if `timestep > loop_transit`,
    /// or if `loop_transit` is not a whole number of steps (to 1e-9 relative).
    /// The state starts at `n = 1 /m³` in the circulating equilibrium
    /// ([`Self::set_circulating_equilibrium`]).
    pub fn new(
        nuclide: FissioningNuclideType,
        core_transit: Time,
        loop_transit: Time,
        generation_time: Time,
        timestep: Time,
    ) -> Result<Self, TehOPrkeError> {
        Self::with_groups(
            nuclide.get_delayed_fraction_array().to_vec(),
            nuclide.get_decay_constant_array().to_vec(),
            core_transit,
            loop_transit,
            generation_time,
            timestep,
        )
    }

    /// As [`Self::new`], with explicit group data: delayed fractions `β_i`
    /// and decay constants `λ_i` (e.g. a report's own table). Errors as
    /// `new`, and if the two lists differ in length or are empty, or a decay
    /// constant is not positive or a fraction negative.
    pub fn with_groups(
        delayed_fractions: Vec<Ratio>,
        decay_constants: Vec<DecayConstant>,
        core_transit: Time,
        loop_transit: Time,
        generation_time: Time,
        timestep: Time,
    ) -> Result<Self, TehOPrkeError> {
        if delayed_fractions.len() != decay_constants.len() || delayed_fractions.is_empty() {
            return Err(TehOPrkeError::ShapeMismatch(format!(
                "{} delayed fractions but {} decay constants",
                delayed_fractions.len(),
                decay_constants.len()
            )));
        }
        let lambdas: Vec<f64> = decay_constants.iter().map(|l: &DecayConstant| l.get::<hertz>()).collect();
        let betas: Vec<f64> = delayed_fractions.iter().map(|b| b.get::<ratio>()).collect();
        if lambdas.iter().any(|&l| !(l > 0.0)) || betas.iter().any(|&b| !(b >= 0.0)) {
            return Err(TehOPrkeError::GenericStringError(
                "decay constants must be > 0 and delayed fractions >= 0".to_string(),
            ));
        }
        let groups = lambdas.len();
        let tc = core_transit.get::<second>();
        let tl = loop_transit.get::<second>();
        let dt = timestep.get::<second>();
        let lam = generation_time.get::<second>();
        for (name, v) in [("core transit", tc), ("loop transit", tl), ("timestep", dt), ("generation time", lam)] {
            if !(v > 0.0) {
                return Err(TehOPrkeError::GenericStringError(format!("{name} must be > 0, got {v}")));
            }
        }
        if dt > tl {
            return Err(TehOPrkeError::GenericStringError(format!(
                "timestep {dt} s exceeds the loop transit {tl} s: the returning precursors would not be known"
            )));
        }
        let steps = (tl / dt).round();
        if ((steps * dt - tl) / tl).abs() > 1.0e-9 {
            return Err(TehOPrkeError::GenericStringError(format!(
                "loop transit {tl} s is not a whole number of {dt} s steps"
            )));
        }
        let mut pk = Self {
            decay_constants: lambdas,
            delayed_fractions: betas,
            core_transit: tc,
            loop_transit: tl,
            dt,
            loop_steps: steps as usize,
            generation_time: lam,
            n: 1.0,
            c: vec![0.0; groups],
            history: VecDeque::new(),
        };
        pk.set_circulating_equilibrium(VolumetricNumberDensity::new::<per_cubic_meter>(1.0));
        Ok(pk)
    }

    /// `β = Σ β_i`.
    pub fn total_delayed_fraction(&self) -> Ratio {
        Ratio::new::<ratio>(self.delayed_fractions.iter().sum())
    }

    /// Neutron density `n`.
    pub fn neutron_density(&self) -> VolumetricNumberDensity {
        VolumetricNumberDensity::new::<per_cubic_meter>(self.n)
    }

    /// Core precursor densities `C_i` \[1/m³\].
    pub fn precursor_densities(&self) -> Vec<VolumetricNumberDensity> {
        self.c.iter().map(|&c| VolumetricNumberDensity::new::<per_cubic_meter>(c)).collect()
    }

    /// The effective removal from the core of group `i` at steady state,
    /// `(1 − e^{−λ_i τ_L}) / τ_c` \[1/s\].
    fn drift_rate(&self, i: usize) -> f64 {
        (1.0 - (-self.decay_constants[i] * self.loop_transit).r_exp()) / self.core_transit
    }

    /// The analytic steady-state circulation reactivity loss,
    /// `Δρ = Σ β_i [1 − λ_i / (λ_i + (1 − e^{−λ_i τ_L})/τ_c)]`: the reactivity
    /// that holds the circulating core critical with no source.
    pub fn circulation_reactivity_loss(&self) -> Ratio {
        let mut loss = 0.0;
        for i in 0..self.decay_constants.len() {
            let l = self.decay_constants[i];
            loss += self.delayed_fractions[i] * (1.0 - l / (l + self.drift_rate(i)));
        }
        Ratio::new::<ratio>(loss)
    }

    /// Set `n` and the core `C_i` to the circulating steady state at
    /// `ρ = Δρ` with no source, `C_i = β_i n / (Λ (λ_i + (1 − e^{−λ_i τ_L})/τ_c))`,
    /// and fill the loop history with that steady `C_i`.
    pub fn set_circulating_equilibrium(&mut self, neutron_density: VolumetricNumberDensity) {
        self.n = neutron_density.get::<per_cubic_meter>();
        self.c = (0..self.decay_constants.len())
            .map(|i| {
                self.delayed_fractions[i] * self.n
                    / (self.generation_time * (self.decay_constants[i] + self.drift_rate(i)))
            })
            .collect();
        self.history = std::iter::repeat(self.c.clone()).take(self.loop_steps).collect();
    }

    /// The circulating in-hour equation, `ρ(ω) = ωΛ + β − Σ λ_i β_i / (ω +
    /// λ_i + (1 − e^{−(ω+λ_i) τ_L})/τ_c)`: the reactivity for which the
    /// equations have the solution `n ~ e^{ωt}`.
    pub fn inhour_reactivity(&self, omega: Frequency) -> Ratio {
        let w = omega.get::<hertz>();
        let mut rho = w * self.generation_time;
        for i in 0..self.decay_constants.len() {
            let l = self.decay_constants[i];
            let b = self.delayed_fractions[i];
            let a = w + l + (1.0 - (-(w + l) * self.loop_transit).r_exp()) / self.core_transit;
            rho += b - l * b / a;
        }
        Ratio::new::<ratio>(rho)
    }

    /// The largest real root `ω` of [`Self::inhour_reactivity`] for a step
    /// reactivity `rho`, by bisection to round-off (200 halvings) on
    /// `(−λ_min, (ρ + β)/Λ + 1)`, where the right-hand side is monotonic.
    pub fn inhour_growth_rate(&self, reactivity: Ratio) -> Frequency {
        let rho = reactivity.get::<ratio>();
        let lambda_min = self.decay_constants.iter().cloned().fold(f64::INFINITY, f64::min);
        let f = |w: f64| self.inhour_reactivity(Frequency::new::<hertz>(w)).get::<ratio>() - rho;
        let mut lo = -lambda_min * (1.0 - 1.0e-12);
        let mut hi = (rho.abs() + self.total_delayed_fraction().get::<ratio>()) / self.generation_time + 1.0;
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if f(mid) > 0.0 {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        Frequency::new::<hertz>(0.5 * (lo + hi))
    }

    /// Advance one fixed step by backward Euler at reactivity `reactivity`
    /// and source `source`.
    pub fn step_implicit(&mut self, reactivity: Ratio, source: VolumetricNumberRate) -> Result<(), TehOPrkeError> {
        let dt = self.dt;
        let lam = self.generation_time;
        let rho = reactivity.get::<ratio>();
        let s = source.get::<per_cubic_meter_second>();
        let beta: f64 = self.delayed_fractions.iter().sum();
        let returning = self.history.front().cloned().unwrap_or_else(|| self.c.clone());
        // C_i' = (C_i/dt + D_i + β_i n'/Λ) / a_i, a_i = 1/dt + λ_i + 1/τ_c.
        let mut lhs = 1.0 / dt - (rho - beta) / lam;
        let mut rhs = self.n / dt + s;
        let mut partial = Vec::with_capacity(self.c.len());
        for i in 0..self.c.len() {
            let l = self.decay_constants[i];
            let a = 1.0 / dt + l + 1.0 / self.core_transit;
            let d = returning[i] * (-l * self.loop_transit).r_exp() / self.core_transit;
            let known = (self.c[i] / dt + d) / a;
            let per_n = self.delayed_fractions[i] / lam / a;
            lhs -= l * per_n;
            rhs += l * known;
            partial.push((known, per_n));
        }
        if !(lhs.is_finite() && lhs != 0.0) {
            return Err(TehOPrkeError::GenericStringError(format!("singular step: lhs = {lhs}")));
        }
        let n_new = rhs / lhs;
        let c_new: Vec<f64> = partial.iter().map(|(known, per_n)| known + per_n * n_new).collect();
        self.history.push_back(c_new.clone());
        if self.history.len() > self.loop_steps {
            self.history.pop_front();
        }
        self.n = n_new;
        self.c = c_new;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zero_power_prke::n_group_prke::PointKinetics;

    /// MSRE inputs, ORNL-TM-0728 Table 5.4 (PDF pp. 137-138): core transit
    /// 9.4 s, fuel-loop total 25.2 s (so loop 15.8 s), Λ = 2.40e-4 s for the
    /// 35 % U-235 fuel.
    fn msre(dt: f64) -> CirculatingPointKinetics {
        CirculatingPointKinetics::new(
            FissioningNuclideType::U235,
            Time::new::<second>(9.4),
            Time::new::<second>(25.2 - 9.4),
            Time::new::<second>(2.40e-4),
            Time::new::<second>(dt),
        )
        .expect("MSRE kinetics")
    }

    /// V2: the analytic MSRE circulation loss with the crate's U-235 data;
    /// V1: started at the circulating equilibrium with ρ = Δρ, n holds over
    /// 300 s at dt = 0.01 s (|n/n0 − 1| ≤ 1e-9).
    #[test]
    fn v1_v2_the_circulating_equilibrium_holds_at_the_analytic_loss() {
        let mut pk = msre(0.01);
        let loss = pk.circulation_reactivity_loss();
        println!(
            "T-C1 V2: beta = {:.2} pcm, circulation loss = {:.4} pcm, beta_circ = {:.4} pcm",
            1e5 * pk.total_delayed_fraction().get::<ratio>(),
            1e5 * loss.get::<ratio>(),
            1e5 * (pk.total_delayed_fraction() - loss).get::<ratio>()
        );
        // ~~2.2435045e-3~~ with the textbook U-235 set; CHANGED 2026-11-10
        // (gh:#1319) to the ENDF/B-VIII.0 U-235 default: 207.1463 pcm.
        approx::assert_relative_eq!(loss.get::<ratio>(), 2.071463e-3, max_relative = 1e-6);
        let zero = VolumetricNumberRate::new::<per_cubic_meter_second>(0.0);
        let mut worst: f64 = 0.0;
        for _ in 0..30_000 {
            pk.step_implicit(loss, zero).unwrap();
            worst = worst.max((pk.neutron_density().get::<per_cubic_meter>() - 1.0).abs());
        }
        println!("T-C1 V1: worst |n/n0 - 1| over 300 s = {worst:.3e}");
        assert!(worst <= 1.0e-9);
    }

    /// Measured asymptotic growth rate: the slope of ln n over the last
    /// `window` seconds of a `duration` run at a step reactivity from the
    /// circulating equilibrium.
    fn measured_omega(dt: f64, rho: f64, duration: f64, window: f64) -> f64 {
        let mut pk = msre(dt);
        let zero = VolumetricNumberRate::new::<per_cubic_meter_second>(0.0);
        let steps = (duration / dt).round() as usize;
        let w_steps = (window / dt).round() as usize;
        let mut n_start = 0.0;
        for k in 1..=steps {
            pk.step_implicit(Ratio::new::<ratio>(rho), zero).unwrap();
            if k == steps - w_steps {
                n_start = pk.neutron_density().get::<per_cubic_meter>();
            }
        }
        (pk.neutron_density().get::<per_cubic_meter>() / n_start).r_ln() / window
    }

    /// V3: a step of Δρ ± 50 pcm; the growth rate over the last 400 s of a
/// 2400 s run against the
    /// circulating in-hour root, within 1 % at dt = 0.01 s, with the error
    /// falling roughly linearly in dt.
    #[test]
    fn v3_the_asymptotic_period_matches_the_circulating_inhour_root() {
        let pk = msre(0.01);
        let loss = pk.circulation_reactivity_loss().get::<ratio>();
        for dr in [5.0e-4, -5.0e-4] {
            let rho = loss + dr;
            let exact = pk.inhour_growth_rate(Ratio::new::<ratio>(rho)).get::<hertz>();
            let mut errs = Vec::new();
            for dt in [0.04, 0.02, 0.01] {
                // 2400 s, slope over the last 400 s. A 600 s run (last
                // 100 s), the first protocol tried, had not reached the
                // asymptotic regime at -50 pcm: 0.53 % off, constant in dt
                // (the delay-induced modes had not died away). Protocol
                // fixed, criterion unchanged (gh:#1131).
                let w = measured_omega(dt, rho, 2400.0, 400.0);
                errs.push((dt, w, w / exact - 1.0));
            }
            for (dt, w, e) in &errs {
                println!("T-C1 V3: rho = loss {:+.0} pcm, dt = {dt}: omega = {w:.6e} /s vs root {exact:.6e} /s ({:+.4} %)", dr * 1e5, 100.0 * e);
            }
            assert!(errs[2].2.abs() <= 0.01, "dt = 0.01 s: {:+.4} %", 100.0 * errs[2].2);
            assert!(errs[0].2.abs() > errs[2].2.abs(), "error must fall with dt");
        }
    }

    /// V4: τ_c → ∞ (1e12 s) recovers the stationary-fuel in-hour root of
    /// `PointKinetics` to 1e-9 relative, for ±50 pcm.
    #[test]
    fn v4_without_circulation_the_inhour_root_is_the_stationary_one() {
        let pk = CirculatingPointKinetics::new(
            FissioningNuclideType::U235,
            Time::new::<second>(1.0e12),
            Time::new::<second>(15.8),
            Time::new::<second>(2.40e-4),
            Time::new::<second>(0.01),
        )
        .unwrap();
        let stationary = PointKinetics::from_nuclide(FissioningNuclideType::U235);
        for rho in [5.0e-4, -5.0e-4] {
            let a = pk.inhour_growth_rate(Ratio::new::<ratio>(rho)).get::<hertz>();
            let b = stationary
                .inhour_growth_rate(Ratio::new::<ratio>(rho), Time::new::<second>(2.40e-4))
                .get::<hertz>();
            println!("T-C1 V4: rho = {:+.0} pcm: circulating(tau_c = 1e12 s) {a:.12e} /s, stationary {b:.12e} /s", rho * 1e5);
            approx::assert_relative_eq!(a, b, max_relative = 1e-9);
        }
    }

    /// A loop transit that is not a whole number of steps is refused.
    #[test]
    fn a_loop_transit_off_the_step_grid_is_refused() {
        let r = CirculatingPointKinetics::new(
            FissioningNuclideType::U235,
            Time::new::<second>(9.4),
            Time::new::<second>(15.805),
            Time::new::<second>(2.40e-4),
            Time::new::<second>(0.01),
        );
        assert!(r.is_err());
    }
}
