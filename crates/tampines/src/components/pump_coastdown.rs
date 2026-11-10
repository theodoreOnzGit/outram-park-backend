//! Inertial pump coast-down: the shaft speed of a tripped pump that runs
//! down against its own hydraulic torque and a constant friction torque
//! (gh:#1322; EBR-II groundwork, epic gh:#1312).
//!
//! # Model
//!
//! After the trip at `t = 0` no motor torque acts. The rotor (impeller,
//! shaft, motor rotor, flywheel) of moment of inertia `I` obeys
//!
//! ```text
//! I dω/dt = −T_h(ω) − T_f,     ω(0) = ω₀
//! ```
//!
//! with the **hydraulic torque at homologous similarity**,
//! `T_h = T₀ (ω/ω₀)²`. That is the affinity-law torque of a pump whose
//! operating point slides along one homologous point as it slows: the flow
//! scales as `ω`, the head as `ω²`, the hydraulic torque as `ω²`. It holds
//! when the loop's resistance is quadratic in flow (turbulent friction and
//! form losses) and the loop's own fluid inertia and buoyancy are small
//! against the rotor's. `T_f` is a constant bearing/seal friction torque,
//! zero by default.
//!
//! **Closed forms** (derived here; both checked against the ODE integrator
//! below):
//!
//! - `T_f = 0`: `ω(t) = ω₀ / (1 + t/τ)`, `τ = I ω₀ / T₀ = I ω₀² / P₀`
//!   (`P₀ = T₀ ω₀` the pre-trip hydraulic shaft power). `τ` is the time to
//!   half speed. The speed never reaches zero.
//! - `T_f > 0`: with `a = T₀/ω₀²`, `b = T_f`, `s = √(b/a)`,
//!   `ω(t) = s · tan(atan(ω₀/s) − t √(ab)/I)` until it reaches zero at
//!   `t_stop = I/√(ab) · atan(ω₀/s)`, and `ω = 0` after (friction holds a
//!   stopped rotor; no reverse rotation is modelled).
//!
//! # Where this sits in the workspace (searched 2026-11-10)
//!
//! The same `T_f = 0` solution already exists in two places, which this
//! module generalises rather than replaces:
//!
//! - `crate::htr10_primary_loop::CoastdownModel::Inertial` (HTR-10 helium
//!   circulator, relative speed only, `τ` supplied by the caller);
//! - GeN-Foam's `velocityRundown` boundary condition, ported as
//!   `outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::boundary_conditions::VelocityRundownBc`,
//!   `(A + B t)^C`, which is this solution at `A = 1`, `B = 1/τ`, `C = −1`.
//!
//! Both are cross-checked in the tests below. Neither carries the inertia,
//! rated torque, friction or an ODE form. `crate::components::Pump` is a
//! steady operating-point stub (`evaluate` is `NotYetImplemented`), and the
//! HEM network's `PumpHead` takes a head table: [`InertialPumpCoastdown::head_ratio`]
//! is the `(ω/ω₀)²` factor to build that table from.
//!
//! # Not modelled (yet)
//!
//! A **homologous pump** in the RELAP5/SAM sense (head and torque from
//! homologous curves in all four quadrants, the flow from the loop momentum
//! equation, so that flow and speed are coupled and can reverse) is not
//! here. The open upstream candidate is MOOSE thermal-hydraulics'
//! `ShaftConnectedPump1Phase` (LGPL-2.1); it is follow-up work on gh:#1322.
//! This module is the special case in which the pump stays at its rated
//! homologous point.
//!
//! AI-drafted (Claude Opus 5.5 in Claude Code), untrusted until reviewed.
//! Research, education and V&V only; not for plant operation or safety
//! analysis.

use petir::mathf::RealMath;
use uom::si::angular_velocity::radian_per_second;
use uom::si::f64::{AngularVelocity, MomentOfInertia, Ratio, Time, Torque};
use uom::si::moment_of_inertia::kilogram_square_meter;
use uom::si::ratio::ratio;
use uom::si::time::second;
use uom::si::torque::newton_meter;

use crate::TampinesError;

/// A tripped pump's rotor running down against a hydraulic torque
/// `T₀ (ω/ω₀)²` and a constant friction torque (see the module doc).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InertialPumpCoastdown {
    /// Rotor moment of inertia `I` (pump, shaft, motor, flywheel), `> 0`.
    pub moment_of_inertia: MomentOfInertia,
    /// Pre-trip shaft speed `ω₀`, `> 0`.
    pub rated_speed: AngularVelocity,
    /// Pre-trip hydraulic torque `T₀`, `> 0`.
    pub rated_hydraulic_torque: Torque,
    /// Constant friction torque `T_f`, `≥ 0`.
    pub friction_torque: Torque,
}

fn positive_finite(name: &str, value: f64) -> Result<f64, TampinesError> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(TampinesError::InvalidInput(format!("{name} must be finite and > 0, got {value}")))
    }
}

impl InertialPumpCoastdown {
    /// A coast-down with no friction torque.
    ///
    /// # Errors
    /// [`TampinesError::InvalidInput`] unless `I`, `ω₀` and `T₀` are finite
    /// and positive.
    pub fn new(
        moment_of_inertia: MomentOfInertia,
        rated_speed: AngularVelocity,
        rated_hydraulic_torque: Torque,
    ) -> Result<Self, TampinesError> {
        positive_finite("moment of inertia", moment_of_inertia.get::<kilogram_square_meter>())?;
        positive_finite("rated speed", rated_speed.get::<radian_per_second>())?;
        positive_finite("rated hydraulic torque", rated_hydraulic_torque.get::<newton_meter>())?;
        Ok(Self {
            moment_of_inertia,
            rated_speed,
            rated_hydraulic_torque,
            friction_torque: Torque::new::<newton_meter>(0.0),
        })
    }

    /// The same coast-down with a constant friction torque `T_f ≥ 0`.
    ///
    /// # Errors
    /// [`TampinesError::InvalidInput`] if `T_f` is negative or not finite.
    pub fn with_friction_torque(self, friction_torque: Torque) -> Result<Self, TampinesError> {
        let tf = friction_torque.get::<newton_meter>();
        if !(tf.is_finite() && tf >= 0.0) {
            return Err(TampinesError::InvalidInput(format!(
                "friction torque must be finite and >= 0, got {tf}"
            )));
        }
        Ok(Self { friction_torque, ..self })
    }

    fn si(&self) -> (f64, f64, f64, f64) {
        (
            self.moment_of_inertia.get::<kilogram_square_meter>(),
            self.rated_speed.get::<radian_per_second>(),
            self.rated_hydraulic_torque.get::<newton_meter>(),
            self.friction_torque.get::<newton_meter>(),
        )
    }

    /// `τ = I ω₀ / T₀`, the time to half speed without friction.
    pub fn time_constant(&self) -> Time {
        let (i, w0, t0, _) = self.si();
        Time::new::<second>(i * w0 / t0)
    }

    /// The time the rotor stops, `I/√(ab) · atan(ω₀/s)`; `None` without
    /// friction (the speed then only tends to zero).
    pub fn stop_time(&self) -> Option<Time> {
        let (i, w0, t0, tf) = self.si();
        if tf <= 0.0 {
            return None;
        }
        let a = t0 / (w0 * w0);
        let s = (tf / a).sqrt();
        Some(Time::new::<second>(i / (a * tf).sqrt() * (w0 / s).r_atan()))
    }

    /// Shaft speed at time `t` after the trip, from the closed form. Before
    /// the trip (`t < 0`) it is `ω₀`.
    pub fn speed(&self, t: Time) -> AngularVelocity {
        let (i, w0, t0, tf) = self.si();
        let t = t.get::<second>();
        if t <= 0.0 {
            return self.rated_speed;
        }
        let w = if tf <= 0.0 {
            w0 / (1.0 + t / (i * w0 / t0))
        } else {
            let a = t0 / (w0 * w0);
            let s = (tf / a).sqrt();
            let phase = (w0 / s).r_atan() - t * (a * tf).sqrt() / i;
            if phase <= 0.0 {
                0.0
            } else {
                s * phase.r_tan()
            }
        };
        AngularVelocity::new::<radian_per_second>(w)
    }

    /// `ω(t)/ω₀`.
    pub fn speed_ratio(&self, t: Time) -> Ratio {
        self.speed(t) / self.rated_speed
    }

    /// `Q(t)/Q₀ = ω/ω₀`: the affinity-law flow at the rated homologous point.
    pub fn flow_ratio(&self, t: Time) -> Ratio {
        self.speed_ratio(t)
    }

    /// `H(t)/H₀ = (ω/ω₀)²`: the affinity-law head.
    pub fn head_ratio(&self, t: Time) -> Ratio {
        let r = self.speed_ratio(t);
        r * r
    }

    /// `dω/dt = −(T₀ (ω/ω₀)² + T_f)/I` in rad/s², and zero once a rotor
    /// with friction has stopped (`ω ≤ 0`).
    fn deceleration_si(&self, w: f64) -> f64 {
        let (i, w0, t0, tf) = self.si();
        if w <= 0.0 {
            return 0.0;
        }
        -(t0 * (w / w0) * (w / w0) + tf) / i
    }

    /// Shaft speed at `t_end` by integrating the ODE with classical
    /// fourth-order Runge-Kutta at a fixed step `dt` (the last step is
    /// shortened to land on `t_end`). An independent route to
    /// [`Self::speed`], for checking it and as the template for a coupled
    /// solve; the speed is floored at zero.
    ///
    /// # Errors
    /// [`TampinesError::InvalidInput`] unless `dt > 0` and `t_end ≥ 0` are
    /// finite.
    pub fn integrate_speed(&self, t_end: Time, dt: Time) -> Result<AngularVelocity, TampinesError> {
        let h = positive_finite("time step", dt.get::<second>())?;
        let t_end = t_end.get::<second>();
        if !(t_end.is_finite() && t_end >= 0.0) {
            return Err(TampinesError::InvalidInput(format!(
                "end time must be finite and >= 0, got {t_end}"
            )));
        }
        let mut w = self.rated_speed.get::<radian_per_second>();
        let mut t = 0.0;
        while t < t_end {
            let step = h.min(t_end - t);
            let k1 = self.deceleration_si(w);
            let k2 = self.deceleration_si(w + 0.5 * step * k1);
            let k3 = self.deceleration_si(w + 0.5 * step * k2);
            let k4 = self.deceleration_si(w + step * k3);
            w = (w + step / 6.0 * (k1 + 2.0 * k2 + 2.0 * k3 + k4)).max(0.0);
            t += step;
        }
        Ok(AngularVelocity::new::<radian_per_second>(w))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::htr10_primary_loop::CoastdownModel;
    use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::boundary_conditions::VelocityRundownBc;
    use uom::si::angular_velocity::revolution_per_minute;
    use uom::si::frequency::hertz;
    use uom::si::f64::Frequency;

    /// A generic sodium primary pump, round numbers (not any real pump):
    /// `I = 50 kg m²`, `ω₀ = 1000 rpm`, `T₀ = 2000 N m` (`P₀ ≈ 209 kW`).
    /// `τ = 50 · 104.7198 / 2000 = 2.61799 s`.
    fn pump() -> InertialPumpCoastdown {
        InertialPumpCoastdown::new(
            MomentOfInertia::new::<kilogram_square_meter>(50.0),
            AngularVelocity::new::<revolution_per_minute>(1000.0),
            Torque::new::<newton_meter>(2000.0),
        )
        .unwrap()
    }

    fn s(t: f64) -> Time {
        Time::new::<second>(t)
    }

    fn assert_rel(got: f64, want: f64, tol: f64) {
        assert!((got / want - 1.0).abs() <= tol, "got {got}, want {want}, tol {tol}");
    }

    /// Verification of the frictionless closed form against its defining
    /// properties: `ω(0) = ω₀`, `ω(τ) = ω₀/2`, `ω(3τ) = ω₀/4`, flow and head
    /// follow the affinity laws, and `τ = I ω₀/T₀` by hand
    /// (`50 · 1000·2π/60 / 2000 = 2.617994 s`).
    #[test]
    fn frictionless_closed_form_halves_speed_at_tau() {
        let p = pump();
        let tau = p.time_constant().get::<second>();
        assert_rel(tau, 2.617_993_877_991_494, 1e-12);
        assert_eq!(p.speed(s(0.0)), p.rated_speed);
        assert_eq!(p.speed(s(-1.0)), p.rated_speed);
        assert_rel(p.speed_ratio(s(tau)).get::<ratio>(), 0.5, 1e-12);
        assert_rel(p.speed_ratio(s(3.0 * tau)).get::<ratio>(), 0.25, 1e-12);
        assert_rel(p.flow_ratio(s(tau)).get::<ratio>(), 0.5, 1e-12);
        assert_rel(p.head_ratio(s(tau)).get::<ratio>(), 0.25, 1e-12);
        assert!(p.stop_time().is_none());
    }

    /// Verification: the RK4 ODE integrator against the closed form,
    /// without and with friction (`T_f = 200 N m`, 10 % of `T₀`).
    ///
    /// Prediction before the run: RK4 at `dt = τ/100` has a global error of
    /// order `(dt/τ)^4 ≈ 1e-8` relative, so agreement below 1e-6 at 1, 5 and
    /// 20 `τ`. With friction the closed form stops the rotor at `t_stop`,
    /// evaluated independently in Python 3: `a = 2000/104.7198² = 0.182378`,
    /// `s = √(200/a) = 33.1153`, `√(ab) = 6.03951`, `atan(ω₀/s) =
    /// atan(3.16228) = 1.26452`, `t_stop = 50/6.03951 · 1.26452 = 10.4687 s`;
    /// the integrator must give zero after it.
    ///
    /// Result 2026-11-10 (release build, x86-64): without friction the
    /// relative error is 6.1e-11, 2.3e-11, 6.6e-12 at 1, 5, 20 `τ`; with
    /// friction 4.9e-11, 4.4e-11, 1.7e-10 at 0.1, 0.5, 0.9 `t_stop`;
    /// `t_stop = 10.468729 s`; zero after it. All below the predicted 1e-8,
    /// asserted at 1e-6. The closed forms are therefore consistent with the
    /// ODE they claim to solve.
    #[test]
    fn rk4_integrator_matches_closed_forms() {
        let p = pump();
        let tau = p.time_constant().get::<second>();
        let dt = s(tau / 100.0);
        for k in [1.0, 5.0, 20.0] {
            let exact = p.speed(s(k * tau)).get::<radian_per_second>();
            let numeric = p.integrate_speed(s(k * tau), dt).unwrap().get::<radian_per_second>();
            println!("no friction t = {k} tau: exact {exact:.9}, RK4 {numeric:.9}, rel {:.2e}", (numeric / exact - 1.0).abs());
            assert_rel(numeric, exact, 1e-6);
        }

        let pf = pump().with_friction_torque(Torque::new::<newton_meter>(200.0)).unwrap();
        let t_stop = pf.stop_time().unwrap().get::<second>();
        println!("friction stop time {t_stop:.6} s");
        assert_rel(t_stop, 10.4687, 1e-4);
        for frac in [0.1, 0.5, 0.9] {
            let t = frac * t_stop;
            let exact = pf.speed(s(t)).get::<radian_per_second>();
            let numeric = pf.integrate_speed(s(t), dt).unwrap().get::<radian_per_second>();
            println!("friction t = {frac} t_stop: exact {exact:.9}, RK4 {numeric:.9}, rel {:.2e}", (numeric / exact - 1.0).abs());
            assert_rel(numeric, exact, 1e-6);
        }
        assert_eq!(pf.speed(s(1.01 * t_stop)).get::<radian_per_second>(), 0.0);
        assert_eq!(pf.integrate_speed(s(1.5 * t_stop), dt).unwrap().get::<radian_per_second>(), 0.0);
        // Friction only shortens the coast-down.
        assert!(pf.speed(s(tau)) < pump().speed(s(tau)));
    }

    /// Cross-check against the two existing forms of the same solution:
    /// tampines' HTR-10 `CoastdownModel::Inertial` and GeN-Foam's
    /// `velocityRundown` at `A = 1, B = 1/τ, C = −1`. Agreement to 1e-12.
    #[test]
    fn agrees_with_htr10_inertial_and_genfoam_velocity_rundown() {
        let p = pump();
        let tau = p.time_constant();
        let htr10 = CoastdownModel::Inertial { time_constant: tau };
        let rundown = VelocityRundownBc::new(
            Ratio::new::<ratio>(1.0),
            Frequency::new::<hertz>(1.0 / tau.get::<second>()),
            -1.0,
            s(0.0),
        );
        for t in [0.5, 2.0, 10.0, 60.0] {
            let ours = p.speed_ratio(s(t)).get::<ratio>();
            assert_rel(ours, htr10.speed(t), 1e-12);
            assert_rel(ours, rundown.rundown_factor(s(t)).get::<ratio>(), 1e-12);
        }
    }

    /// Invalid inputs are errors, not panics.
    #[test]
    fn invalid_inputs_are_rejected() {
        let zero_inertia = InertialPumpCoastdown::new(
            MomentOfInertia::new::<kilogram_square_meter>(0.0),
            AngularVelocity::new::<revolution_per_minute>(1000.0),
            Torque::new::<newton_meter>(2000.0),
        );
        assert!(zero_inertia.is_err());
        assert!(pump().with_friction_torque(Torque::new::<newton_meter>(-1.0)).is_err());
        assert!(pump().integrate_speed(s(1.0), s(0.0)).is_err());
        assert!(pump().integrate_speed(s(f64::NAN), s(0.1)).is_err());
    }
}
