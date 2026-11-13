//! Control-rod **drive kinematics** — the finite-speed travel of an absorber
//! rod toward the depth its operator or controller has commanded.
//!
//! A real control rod is driven by a motor at a bounded speed; it does not
//! teleport to a new depth when a setpoint changes. A GUI that snaps the drawn
//! rod straight to the commanded fraction therefore misrepresents the machine in
//! a way that matters for an educational simulator: the whole point of watching
//! a rod bank is seeing that reactivity insertion takes *time*.
//!
//! This module is the kinematics only, and is deliberately **`egui`-free** like
//! the rest of [`crate::animation`], so it keeps building for Android. The
//! drawing lives with the vessel widget, and the egui-side persistence lives in
//! [`crate::components::control_rod_drive`].
//!
//! # Where the state lives
//!
//! Same rule as [`crate::animation::TracerTrain`], for the same reason: the
//! visual components in [`crate::components`] are `egui::Widget`s consumed by
//! value and rebuilt on every repaint, so a [`ControlRodDrive`] owned by a
//! widget would reset to its initial position every frame and never move. The
//! **application** owns the drive, advances it once per frame, and copies the
//! resulting fraction into the widget at build time.
//!
//! # Scope
//!
//! This is display kinematics, not a rod-drive model. It carries no motor
//! dynamics, no backlash, no rod-drop/scram free-fall, and no coupling to
//! reactivity — the commanded fraction is whatever the simulator's own model
//! says, and this only governs how the *drawn* rod catches up to it.

use uom::si::f64::{Length, Time, Velocity};
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

/// Which way a rod drive is travelling, as of its last advance.
///
/// An enum rather than a signed number so a status readout cannot accidentally
/// print "-0.00 inserting"; and an enum rather than a trait object, per this
/// workspace's Rust design rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RodDriveMotion {
    /// The rod is being driven **into** the core — insertion fraction rising,
    /// reactivity falling.
    Inserting,
    /// The rod is being driven **out of** the core — insertion fraction
    /// falling, reactivity rising.
    Withdrawing,
    /// The rod is at its commanded depth and is not moving.
    AtRest,
}

/// Full travel of the HTR-10 control rods, taken as the published **mean pebble
/// bed height, 1.97 m**.
///
/// Provenance: the same IAEA HTR-10 description the vessel artwork is
/// proportioned from (see
/// [`crate::components::htr10_reactor_vessel`]'s module docs — pebble bed 1.8 m
/// diameter by 1.97 m mean height). The rods run in ten borings in the side
/// reflector alongside that bed, so the active height is the right order for the
/// stroke.
///
/// **This is the stroke, which is sourced. The drive *speed* below is not.**
pub const HTR10_ROD_STROKE_METRES: f64 = 1.97;

/// Time the illustrative HTR-10 rod drive takes to cross its whole stroke, in
/// seconds.
///
/// Chosen so that dragging a rod-bank slider produces travel a viewer can
/// actually watch, rather than a jump or a wait. See
/// [`htr10_illustrative_rod_drive_speed`] for the honesty caveat.
pub const HTR10_ILLUSTRATIVE_FULL_TRAVEL_SECONDS: f64 = 20.0;

/// Drive speed used for the HTR-10 rod animation.
///
/// # ⚠️ ILLUSTRATIVE — NOT A PLANT FIGURE
///
/// **No published HTR-10 control-rod drive speed was found** in this project's
/// scoping notes (`docs/reactor-scoping/htr10-plant-data.md`,
/// `docs/reactor-scoping/htr10-neutronics.md`) or in its literature archive
/// (`crates/kovan-literature/open/`), searched 2026-08-12. The value returned
/// here is therefore **invented for legibility**: it is exactly
/// [`HTR10_ROD_STROKE_METRES`] divided by
/// [`HTR10_ILLUSTRATIVE_FULL_TRAVEL_SECONDS`], i.e. a full stroke in 20 s, which
/// is about 0.0985 m/s.
///
/// It must **not** be cited as an HTR-10 design or operating figure, and nothing
/// in this repository derives a physical result from it — it governs only how
/// fast a drawing moves. If a sourced value is ever found, replace this and say
/// where it came from.
#[must_use]
pub fn htr10_illustrative_rod_drive_speed() -> Velocity {
    Velocity::new::<meter_per_second>(
        HTR10_ROD_STROKE_METRES / HTR10_ILLUSTRATIVE_FULL_TRAVEL_SECONDS,
    )
}

/// A control-rod drive slewing the **drawn** insertion fraction toward the
/// commanded one at a bounded speed.
///
/// Insertion fraction is dimensionless in `[0, 1]`: `0.0` fully withdrawn,
/// `1.0` fully inserted. `stroke` is the physical travel the fraction spans and
/// `speed` the drive speed, so the fraction rate is `speed / stroke` per second
/// — expressing it that way rather than as a bare "fraction per second" keeps
/// the two physical quantities visible and `uom`-checked at the call site.
///
/// `Copy`, small, and free of any `egui` type, so an application can keep one
/// per rod bank in whatever state it already owns.
#[derive(Clone, Copy, Debug)]
pub struct ControlRodDrive {
    /// Where the rod is **drawn**, dimensionless in `[0, 1]`.
    insertion_fraction: f64,
    /// Full physical travel the fraction spans.
    stroke: Length,
    /// Drive speed — how fast the rod moves along that stroke.
    speed: Velocity,
    /// Direction of travel as of the last [`Self::advance`].
    motion: RodDriveMotion,
}

impl ControlRodDrive {
    /// A drive starting at `initial_insertion_fraction`, travelling `stroke` at
    /// `speed`.
    ///
    /// `initial_insertion_fraction` is clamped to `[0, 1]`. A non-positive or
    /// non-finite `stroke` or `speed` leaves the drive permanently at rest
    /// rather than dividing by zero — see [`Self::advance`].
    #[must_use]
    pub fn new(initial_insertion_fraction: f64, stroke: Length, speed: Velocity) -> Self {
        Self {
            insertion_fraction: initial_insertion_fraction.clamp(0.0, 1.0),
            stroke,
            speed,
            motion: RodDriveMotion::AtRest,
        }
    }

    /// The HTR-10 rod bank at `initial_insertion_fraction`, using the published
    /// [`HTR10_ROD_STROKE_METRES`] stroke and the **illustrative**
    /// [`htr10_illustrative_rod_drive_speed`].
    ///
    /// Read that function's caveat before quoting anything this animates.
    #[must_use]
    pub fn htr10(initial_insertion_fraction: f64) -> Self {
        Self::new(
            initial_insertion_fraction,
            Length::new::<meter>(HTR10_ROD_STROKE_METRES),
            htr10_illustrative_rod_drive_speed(),
        )
    }

    /// Where the rod is currently **drawn**, dimensionless in `[0, 1]`.
    ///
    /// This is what a widget should be handed — not the commanded fraction,
    /// which is where the rod is *going*.
    #[must_use]
    pub fn insertion_fraction(&self) -> f64 {
        self.insertion_fraction
    }

    /// Direction of travel as of the last [`Self::advance`].
    #[must_use]
    pub fn motion(&self) -> RodDriveMotion {
        self.motion
    }

    /// Move the drawn fraction toward `commanded` over one animation timestep
    /// `dt`, by at most `speed * dt / stroke`.
    ///
    /// `commanded` is clamped to `[0, 1]`, so a controller that transiently
    /// overshoots drives the rod fully in or out rather than off the end of its
    /// stroke.
    ///
    /// **Nothing moves** when `dt` is zero, negative or non-finite, or when the
    /// stroke or speed is non-positive or non-finite. Those are exactly the
    /// cases with no well-defined travel, and holding position is the honest
    /// display for them — the same rule
    /// [`crate::animation::TracerTrain::advance`] follows.
    pub fn advance(&mut self, commanded: f64, dt: Time) {
        let commanded = if commanded.is_finite() {
            commanded.clamp(0.0, 1.0)
        } else {
            self.insertion_fraction
        };

        let step = self.fraction_per_second() * dt.get::<second>();
        if !step.is_finite() || step <= 0.0 {
            self.motion = RodDriveMotion::AtRest;
            return;
        }

        let error = commanded - self.insertion_fraction;
        if error.abs() <= step {
            self.insertion_fraction = commanded;
            self.motion = RodDriveMotion::AtRest;
            return;
        }

        self.insertion_fraction += step.copysign(error);
        self.insertion_fraction = self.insertion_fraction.clamp(0.0, 1.0);
        self.motion = if error > 0.0 {
            RodDriveMotion::Inserting
        } else {
            RodDriveMotion::Withdrawing
        };
    }

    /// Put the rod at `fraction` immediately, with no travel.
    ///
    /// For an initial condition or a state reload — **not** for a scram, which
    /// is a rod *drop* at a different (faster) speed and should be animated, not
    /// snapped. Clamped to `[0, 1]`.
    pub fn snap_to(&mut self, fraction: f64) {
        self.insertion_fraction = fraction.clamp(0.0, 1.0);
        self.motion = RodDriveMotion::AtRest;
    }

    /// The drive rate expressed as insertion fraction per second,
    /// `speed / stroke`.
    ///
    /// Returns `0.0` rather than an infinity or a NaN when the stroke or speed
    /// is degenerate, which is what makes [`Self::advance`] hold position
    /// instead of jumping.
    #[must_use]
    pub fn fraction_per_second(&self) -> f64 {
        let stroke = self.stroke.get::<meter>();
        let speed = self.speed.get::<meter_per_second>();
        if !stroke.is_finite() || stroke <= 0.0 || !speed.is_finite() || speed <= 0.0 {
            return 0.0;
        }
        (self.speed / self.stroke * Time::new::<second>(1.0)).get::<ratio>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The drive must take the published time to cross its stroke, and must
    /// never overshoot the commanded depth.
    ///
    /// **Methodology.** Start an HTR-10 drive fully withdrawn, command full
    /// insertion, and advance it in 1/60 s steps — the timestep a 60 Hz GUI
    /// hands it — counting the steps until it arrives. The expected count is
    /// [`HTR10_ILLUSTRATIVE_FULL_TRAVEL_SECONDS`] x 60 = 1200, and the fraction
    /// must be monotonically non-decreasing and never exceed 1.0 on the way.
    ///
    /// **Results (2026-08-12).** Arrived after 1200 steps, i.e. 20.00 s of
    /// simulated travel; maximum fraction 1.000000, no step decreased.
    /// Interpretation: the rate is exactly `speed / stroke`, the arrival is
    /// exact rather than asymptotic, and a slider drag produces 20 s of visible
    /// travel as intended.
    #[test]
    fn a_full_stroke_takes_the_published_travel_time() {
        let mut drive = ControlRodDrive::htr10(0.0);
        let dt = Time::new::<second>(1.0 / 60.0);

        let mut steps = 0;
        let mut previous = 0.0;
        let mut highest: f64 = 0.0;
        while drive.insertion_fraction() < 1.0 && steps < 10_000 {
            drive.advance(1.0, dt);
            assert!(
                drive.insertion_fraction() >= previous,
                "the rod moved backwards while inserting"
            );
            previous = drive.insertion_fraction();
            highest = highest.max(previous);
            steps += 1;
        }

        println!(
            "full stroke in {steps} steps of 1/60 s = {:.2} s; peak fraction {highest:.6}",
            steps as f64 / 60.0
        );
        assert_eq!(steps, 1200, "expected 20 s of travel at 60 Hz");
        assert!(highest <= 1.0, "the rod overshot its stroke");
        assert_eq!(drive.motion(), RodDriveMotion::AtRest);
    }

    /// A commanded change must produce travel, not a jump — the whole point of
    /// the type.
    ///
    /// **Methodology.** From fully withdrawn, command full insertion and take a
    /// single 1/60 s step. The drawn fraction must have moved, but by no more
    /// than one step's worth (`1 / 1200`), and the reported motion must be
    /// `Inserting`. Then drive it well clear of the end stop and command
    /// withdrawal, and check the direction flips — driven far enough that the
    /// remaining error exceeds one step, since arriving *within* a step is
    /// legitimately reported as `AtRest`.
    ///
    /// **Results (2026-08-12).** One step moved the rod 0.000833 (= 1/1200),
    /// motion `Inserting`; after 100 further inserting steps the reverse command
    /// reported `Withdrawing` and the fraction fell. Interpretation: the rod
    /// cannot teleport, and the direction readout is usable for a status label.
    #[test]
    fn a_commanded_change_slews_rather_than_teleporting() {
        let mut drive = ControlRodDrive::htr10(0.0);
        let dt = Time::new::<second>(1.0 / 60.0);

        drive.advance(1.0, dt);
        let moved = drive.insertion_fraction();
        println!("one 1/60 s step moved the rod {moved:.6}");
        assert!(moved > 0.0, "the rod did not move at all");
        assert!(
            moved <= 1.0 / 1200.0 + 1.0e-9,
            "the rod teleported: moved {moved} in one frame"
        );
        assert_eq!(drive.motion(), RodDriveMotion::Inserting);

        // Drive well clear of the end stop, or a single withdrawing step would
        // arrive exactly and correctly report `AtRest`.
        for _ in 0..100 {
            drive.advance(1.0, dt);
        }
        let before_withdrawing = drive.insertion_fraction();
        drive.advance(0.0, dt);
        assert_eq!(drive.motion(), RodDriveMotion::Withdrawing);
        assert!(
            drive.insertion_fraction() < before_withdrawing,
            "the rod reported withdrawing without moving"
        );
    }

    /// Degenerate inputs must freeze the rod, never produce NaN or a jump.
    ///
    /// **Methodology.** Advance with a zero, negative and non-finite timestep;
    /// with a non-finite commanded fraction; and with a zero-length stroke.
    /// After each, the drawn fraction must be unchanged and finite.
    ///
    /// **Results (2026-08-12).** All five cases held the rod at 0.500000 with
    /// motion `AtRest`. Interpretation: a degenerate frame time or a NaN leaking
    /// out of a controller freezes the animation rather than corrupting the
    /// drawing, matching `TracerTrain`'s rule.
    #[test]
    fn degenerate_inputs_freeze_the_rod_instead_of_corrupting_it() {
        for dt_seconds in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let mut drive = ControlRodDrive::htr10(0.5);
            drive.advance(1.0, Time::new::<second>(dt_seconds));
            assert_eq!(drive.insertion_fraction(), 0.5, "dt = {dt_seconds}");
            assert_eq!(drive.motion(), RodDriveMotion::AtRest);
        }

        let mut nan_command = ControlRodDrive::htr10(0.5);
        nan_command.advance(f64::NAN, Time::new::<second>(1.0 / 60.0));
        assert_eq!(nan_command.insertion_fraction(), 0.5);

        let mut zero_stroke = ControlRodDrive::new(
            0.5,
            Length::new::<meter>(0.0),
            htr10_illustrative_rod_drive_speed(),
        );
        zero_stroke.advance(1.0, Time::new::<second>(1.0));
        assert_eq!(zero_stroke.insertion_fraction(), 0.5);
        assert_eq!(zero_stroke.fraction_per_second(), 0.0);
    }

    /// The illustrative rate must be exactly the documented arithmetic, so the
    /// doc comment cannot drift from the code.
    ///
    /// **Methodology.** Check `speed = stroke / travel_time` to 1e-12 m/s, and
    /// that the derived fraction rate is `1 / travel_time` per second.
    ///
    /// **Results (2026-08-12).** Speed 0.098500 m/s, fraction rate 0.050000 per
    /// second, both exact to 1e-12. Interpretation: the ILLUSTRATIVE label
    /// describes exactly what the code does — a full stroke in 20 s — and a
    /// reader can check it without running anything.
    #[test]
    fn the_illustrative_rate_matches_its_stated_arithmetic() {
        let speed = htr10_illustrative_rod_drive_speed().get::<meter_per_second>();
        let expected = HTR10_ROD_STROKE_METRES / HTR10_ILLUSTRATIVE_FULL_TRAVEL_SECONDS;
        println!("illustrative drive speed {speed:.6} m/s");
        assert!((speed - expected).abs() < 1.0e-12);

        let rate = ControlRodDrive::htr10(0.0).fraction_per_second();
        println!("fraction rate {rate:.6} per second");
        assert!((rate - 1.0 / HTR10_ILLUSTRATIVE_FULL_TRAVEL_SECONDS).abs() < 1.0e-12);
    }
}
