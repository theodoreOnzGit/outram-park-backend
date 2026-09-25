//! **Per-nuclide radioactive decay, as a transfer function on the puff.**
//!
//! Each nuclide gets its own scalar gain applied on top of one shared
//! transport field (maintainer, 2026-09-24: "each nuclide should have its own
//! attenuation factor slapped on top of the puff model ... this feels a little
//! like a transfer function"). It does, and the analogy is exact rather than
//! a convenience.
//!
//! # Why this is EXACT, not an approximation
//!
//! Transport with decay obeys
//!
//! ```text
//!   dC/dt = K grad^2 C  -  u . grad C  -  lambda C
//! ```
//!
//! Substitute `C = exp(-lambda t) C_0`. The first two terms are linear in `C`
//! and carry the factor straight through, while the time derivative produces
//! `-lambda exp(-lambda t) C_0` which cancels the decay term exactly, leaving
//!
//! ```text
//!   dC_0/dt = K grad^2 C_0  -  u . grad C_0
//! ```
//!
//! i.e. the **decay-free** equation. So `C = exp(-lambda t) C_0` solves the
//! full problem whenever `C_0` solves the transport problem: decay separates
//! completely and contributes a multiplier that depends **only on travel
//! time**, never on position within the puff.
//!
//! Two consequences worth stating plainly:
//!
//! - **The spatial field is computed once and shared.** Every nuclide sees
//!   the same Gaussian; they differ only in a scalar. That is the transfer
//!   function — the puff is the plant, decay is a first-order gain on its
//!   output, and `N` nuclides cost one transport solve and `N` multiplies.
//! - **A Monte-Carlo decay simulation here would buy nothing.** It would
//!   sample a distribution whose mean is this closed form and whose variance
//!   is an artefact of the sampling, not of the physics. `boon-lay`'s
//!   Lagrangian decay engine earns its place where the answer is *not* a
//!   single exponential — see "Chains" below — not here.
//!
//! # Where the shared field stops being shared
//!
//! Decay is uniform; **depletion is not**. Dry deposition, wet scavenging and
//! gravitational settling all depend on the nuclide's chemical and physical
//! form, so a noble gas, an iodine and a caesium aerosol released together do
//! **not** stay the same shape as they travel — the depleted ones lose mass
//! from the bottom of the plume first.
//!
//! So the rule is: **one transport field per depletion class, one scalar per
//! nuclide within it.** For an HTR-10 source term that is three fields, not
//! thirty:
//!
//! | class | depletion | members |
//! |---|---|---|
//! | noble gases | none | Kr, Xe |
//! | halogens | dry + wet, reactive | I |
//! | particulate / metallic | dry + wet, aerosol | Cs, Sr, Ag, Te |
//!
//! [`crate::activity::deposition`] owns that side. This module owns only the
//! part that is genuinely uniform.
//!
//! # Chains, and when `boon-lay` becomes the right tool
//!
//! For a nuclide with an ingrowing parent the gain is **not** a single
//! exponential — it is the Bateman solution, and a daughter's airborne
//! activity can *rise* during transport while its parent falls. The spatial
//! shape is still shared (same puff, same transport), so it is still a scalar
//! per nuclide; it is just a different scalar. [`bateman_two_step`] covers the
//! common parent-daughter case in closed form.
//!
//! Beyond two steps, or where a daughter changes depletion class mid-flight
//! (a noble-gas parent decaying to a particulate daughter genuinely does),
//! the closed form stops being convenient and `boon_lay`'s decay engine is
//! the honest tool. That case is **not** implemented here.
//!
//! # Does it matter at all? Usually not, and the table says when
//!
//! Travel time to 10 km at 5 m/s is about 2000 s. Against that:
//!
//! | half-life | `exp(-lambda t)` at 2000 s | verdict |
//! |---|---|---|
//! | Cs-137, 30 y | 1.0000 | neglect |
//! | Kr-85, 10.7 y | 1.0000 | neglect |
//! | I-131, 8.02 d | 0.9980 | neglect |
//! | Xe-133, 5.24 d | 0.9969 | neglect |
//! | I-133, 20.8 h | 0.9817 | marginal |
//! | Xe-135, 9.14 h | 0.9587 | keep |
//! | Kr-88, 2.84 h | 0.8732 | keep |
//! | Kr-87, 76.3 min | 0.7387 | dominant |
//! | Xe-138, 14.1 min | 0.1942 | dominant |
//!
//! So the maintainer's instinct is right twice over: the long-lived nuclides
//! really can be neglected over transport, and the short-lived ones really do
//! need the factor. Both follow from one line of arithmetic, which is why
//! this module exposes [`decay_is_negligible`] rather than leaving each
//! caller to guess.

use uom::si::f64::{Ratio, Time};
use uom::si::ratio::ratio;
use uom::si::time::second;

use crate::flexpart::decay::{decay_constant_exact, surviving_fraction};

/// One nuclide's decay gain: the scalar this nuclide contributes on top of
/// the shared transport field.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecayTransfer {
    /// `lambda = ln2 / t_half`, s^-1.
    lambda: f64,
}

impl DecayTransfer {
    /// From a half-life. A non-positive or non-finite half-life gives a
    /// **stable** nuclide (gain 1) rather than a NaN — "no half-life" and
    /// "does not decay" are the same statement for this purpose.
    pub fn from_half_life(half_life: Time) -> Self {
        let t = half_life.get::<second>();
        if !(t > 0.0) || !t.is_finite() {
            return Self { lambda: 0.0 };
        }
        Self {
            // `decay_constant_exact`, NOT `decay_constant`. The latter keeps
            // FLEXPART's truncated literal `0.693147` so that port matches
            // upstream term for term; this module is not a port and has no
            // upstream to agree with, so it takes the exact `ln 2`. The
            // difference is ~2.6e-7 relative -- immaterial to a consequence
            // figure, but it is the difference between a half-life halving
            // the activity and very nearly halving it, and there is no reason
            // to inherit an approximation that buys nothing here.
            lambda: decay_constant_exact(t),
        }
    }

    /// A nuclide that does not decay on any transport timescale.
    pub fn stable() -> Self {
        Self { lambda: 0.0 }
    }

    /// `lambda`, s^-1.
    pub fn decay_constant(&self) -> f64 {
        self.lambda
    }

    /// The gain, `exp(-lambda t)`, for a given travel time.
    ///
    /// This is the whole transfer function: multiply any transport quantity
    /// that is linear in released activity — `chi/Q`, air concentration,
    /// time-integrated concentration — by this.
    pub fn gain(&self, travel_time: Time) -> Ratio {
        Ratio::new::<ratio>(surviving_fraction(
            self.lambda,
            travel_time.get::<second>().max(0.0),
        ))
    }

    /// Whether decay may be neglected over `travel_time` at the given
    /// relative tolerance.
    ///
    /// Exists so "long-lived, ignore it" is a computed statement rather than
    /// a habit: `true` means `1 - exp(-lambda t) <= tolerance`.
    pub fn decay_is_negligible(&self, travel_time: Time, tolerance: f64) -> bool {
        1.0 - self.gain(travel_time).get::<ratio>() <= tolerance
    }
}

/// The airborne activity of a **daughter** fed by a decaying parent, per unit
/// parent activity released, after `travel_time` (the two-step Bateman
/// solution).
///
/// ```text
///   A_d(t) / A_p(0) = lambda_d/(lambda_d - lambda_p)
///                     * ( exp(-lambda_p t) - exp(-lambda_d t) )
/// ```
///
/// Still a scalar on the same shared puff, because parent and daughter travel
/// together — **provided they share a depletion class**. A noble-gas parent
/// decaying to a particulate daughter does not, and this function is the
/// wrong tool for that case; see the module docs.
///
/// Returns zero for a stable parent (nothing to feed the daughter) and
/// handles `lambda_p == lambda_d` by its limit, `lambda t exp(-lambda t)`,
/// rather than dividing by zero.
pub fn bateman_two_step(
    parent: DecayTransfer,
    daughter: DecayTransfer,
    travel_time: Time,
) -> Ratio {
    let t = travel_time.get::<second>().max(0.0);
    let (lp, ld) = (parent.lambda, daughter.lambda);
    if lp <= 0.0 {
        // A stable parent does not feed the daughter on this timescale.
        return Ratio::new::<ratio>(0.0);
    }
    let value = if (ld - lp).abs() <= f64::EPSILON * ld.max(lp).max(1.0) {
        // Degenerate limit, taken analytically rather than numerically.
        lp * t * (-lp * t).exp()
    } else {
        ld / (ld - lp) * ((-lp * t).exp() - (-ld * t).exp())
    };
    Ratio::new::<ratio>(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::time::{day, hour, minute, second, year};

    fn t(s: f64) -> Time {
        Time::new::<second>(s)
    }

    /// The gain is `exp(-lambda t)`, against the closed form computed here.
    #[test]
    fn the_gain_is_the_exponential() {
        let d = DecayTransfer::from_half_life(Time::new::<hour>(2.84)); // Kr-88
        let lambda = 2f64.ln() / (2.84 * 3600.0);
        for secs in [0.0, 100.0, 2000.0, 20_000.0] {
            let want = (-lambda * secs).exp();
            let got = d.gain(t(secs)).get::<ratio>();
            assert!((got - want).abs() < 1e-12, "at {secs} s: {got} vs {want}");
        }
    }

    /// One half-life is exactly one half — the definition, and the cheapest
    /// check that `lambda` was built from `ln2` and not something else.
    #[test]
    fn one_half_life_halves_it() {
        for hours in [0.25, 2.84, 9.14, 200.0] {
            let d = DecayTransfer::from_half_life(Time::new::<hour>(hours));
            let got = d.gain(Time::new::<hour>(hours)).get::<ratio>();
            assert!((got - 0.5).abs() < 1e-12, "{hours} h: {got}");
            // And two half-lives is a quarter.
            let two = d.gain(Time::new::<hour>(2.0 * hours)).get::<ratio>();
            assert!((two - 0.25).abs() < 1e-12, "{hours} h doubled: {two}");
        }
    }

    /// A stable nuclide, and a nonsense half-life, both give unit gain rather
    /// than a NaN: "no half-life" and "does not decay" are the same statement
    /// for a transport calculation.
    #[test]
    fn stable_and_degenerate_inputs_give_unit_gain() {
        assert_eq!(DecayTransfer::stable().gain(t(1e9)).get::<ratio>(), 1.0);
        for bad in [0.0, -5.0, f64::NAN, f64::INFINITY] {
            let d = DecayTransfer::from_half_life(t(bad));
            let g = d.gain(t(2000.0)).get::<ratio>();
            assert_eq!(g, 1.0, "half-life {bad} should be treated as stable");
        }
        // A negative travel time cannot manufacture activity.
        let d = DecayTransfer::from_half_life(Time::new::<hour>(1.0));
        assert_eq!(d.gain(t(-100.0)).get::<ratio>(), 1.0);
    }

    /// **The module doc's own table, asserted.**
    ///
    /// A documentation table of "which nuclides can be neglected" is exactly
    /// the sort of claim that rots. These are the values it quotes at a
    /// 2000 s travel time (10 km at 5 m/s), so the doc cannot drift from the
    /// arithmetic without this failing.
    #[test]
    fn the_documented_neglect_table_is_correct() {
        let travel = t(2000.0);
        // (half-life, quoted gain in the doc)
        let rows: [(Time, f64); 9] = [
            (Time::new::<year>(30.17), 1.0000),  // Cs-137
            (Time::new::<year>(10.76), 1.0000),  // Kr-85
            (Time::new::<day>(8.02), 0.9980),    // I-131
            (Time::new::<day>(5.24), 0.9969),    // Xe-133
            (Time::new::<hour>(20.8), 0.9817),   // I-133
            (Time::new::<hour>(9.14), 0.9587),   // Xe-135
            (Time::new::<hour>(2.84), 0.8732),   // Kr-88
            (Time::new::<minute>(76.3), 0.7387), // Kr-87
            (Time::new::<minute>(14.1), 0.1942), // Xe-138
        ];
        for (half_life, quoted) in rows {
            let got = DecayTransfer::from_half_life(half_life)
                .gain(travel)
                .get::<ratio>();
            assert!(
                (got - quoted).abs() < 5e-4,
                "half-life {:?}: computed {got:.4}, doc says {quoted:.4}",
                half_life.get::<second>()
            );
        }
    }

    /// `decay_is_negligible` sorts the table the way the doc claims: the
    /// long-lived are droppable at 1 %, the short-lived are not.
    #[test]
    fn negligibility_is_computed_not_assumed() {
        let travel = t(2000.0);
        let one_percent = 0.01;

        for half_life in [
            Time::new::<year>(30.17),
            Time::new::<year>(10.76),
            Time::new::<day>(8.02),
            Time::new::<day>(5.24),
        ] {
            assert!(
                DecayTransfer::from_half_life(half_life).decay_is_negligible(travel, one_percent),
                "should be droppable at 1 %"
            );
        }
        for half_life in [
            Time::new::<hour>(9.14),
            Time::new::<hour>(2.84),
            Time::new::<minute>(14.1),
        ] {
            assert!(
                !DecayTransfer::from_half_life(half_life).decay_is_negligible(travel, one_percent),
                "should NOT be droppable at 1 %"
            );
        }
    }

    /// A daughter's airborne activity **rises** from zero, peaks, then falls —
    /// the behaviour a single exponential cannot produce, and the reason
    /// ingrowth needs its own treatment rather than another gain.
    #[test]
    fn an_ingrowing_daughter_rises_before_it_falls() {
        let parent = DecayTransfer::from_half_life(Time::new::<hour>(2.84)); // Kr-88
        let daughter = DecayTransfer::from_half_life(Time::new::<minute>(17.8)); // Rb-88

        let at = |secs: f64| bateman_two_step(parent, daughter, t(secs)).get::<ratio>();
        assert_eq!(at(0.0), 0.0, "no daughter at release");

        let early = at(600.0);
        let peak = at(2400.0);
        let late = at(60_000.0);
        assert!(early > 0.0 && peak > early, "must rise: {early} -> {peak}");
        assert!(late < peak, "and then fall: {peak} -> {late}");
        // It never exceeds the parent's initial activity here.
        assert!(
            peak < 1.0,
            "transient equilibrium, not amplification: {peak}"
        );
    }

    /// The degenerate `lambda_p == lambda_d` case is taken as its analytic
    /// limit rather than dividing by zero.
    #[test]
    fn equal_decay_constants_use_the_analytic_limit() {
        let d = DecayTransfer::from_half_life(Time::new::<hour>(3.0));
        let lambda = d.decay_constant();
        for secs in [600.0, 5000.0, 30_000.0] {
            let got = bateman_two_step(d, d, t(secs)).get::<ratio>();
            let want = lambda * secs * (-lambda * secs).exp();
            assert!(got.is_finite(), "must not blow up at {secs} s");
            assert!((got - want).abs() < 1e-12, "at {secs} s: {got} vs {want}");
        }
    }

    /// A stable parent feeds nothing.
    #[test]
    fn a_stable_parent_produces_no_daughter() {
        let daughter = DecayTransfer::from_half_life(Time::new::<hour>(1.0));
        assert_eq!(
            bateman_two_step(DecayTransfer::stable(), daughter, t(5000.0)).get::<ratio>(),
            0.0
        );
    }
}
