//! `egui`-side plumbing for the control-rod drive animation.
//!
//! The kinematics live in [`crate::animation::control_rod_drive`] and are
//! deliberately `egui`-free. This module is the thin layer that keeps a
//! [`ControlRodDrive`] alive **across repaints** and advances it once per frame,
//! so a vessel widget can be handed a *drawn* insertion fraction that travels
//! toward the commanded one instead of jumping to it.
//!
//! # Where the state lives, and why it is here
//!
//! Visual components are `egui::Widget`s consumed by value and rebuilt every
//! repaint (see the crate `CLAUDE.md`), so animation state inside a widget would
//! reset every frame and never move. The established pattern for
//! [`crate::animation::TracerTrain`] is that the **application** owns the state
//! on its `eframe::App` struct and copies it into the widget at build time.
//!
//! [`slewed_control_rod_insertion`] keeps that ownership rule — the state is
//! outside the widget and survives repaints — but parks it in `egui`'s own
//! per-context store, keyed by an [`Id`] the caller chooses, rather than on the
//! app struct. Two consequences worth knowing:
//!
//! - **It is still not widget-owned.** The widget receives a plain `f32` and has
//!   no memory of its own, exactly as before.
//! - **It costs the caller nothing to adopt.** A simulator that already owns its
//!   animation state on the app struct can drive [`ControlRodDrive`] directly
//!   and skip this helper entirely; that remains the more explicit option and is
//!   preferable when the drive's state needs to be read elsewhere (a status
//!   readout, a recorded trace, a saved session).
//!
//! # Repaint
//!
//! While a rod is travelling the helper calls `Context::request_repaint`, so the
//! animation runs to completion even in an application that only repaints on
//! demand. The example simulators repaint continuously anyway; this makes the
//! helper correct for one that does not.

use egui::{Context, Id};
use uom::si::f64::Time;
use uom::si::time::second;

use crate::animation::control_rod_drive::{ControlRodDrive, RodDriveMotion};

/// Longest animation timestep this helper will take in one frame, in seconds.
///
/// `egui`'s frame time can be huge after the window was hidden, minimised, or
/// the process was suspended; feeding that straight in would let a rod cross its
/// whole stroke in one frame, which is the teleport the animation exists to
/// avoid. Clamping to 100 ms means the worst a stall can do is advance the rod
/// by a tenth of a second's travel and then carry on smoothly.
pub const MAX_ANIMATION_TIMESTEP_SECONDS: f64 = 0.1;

/// Advance a persisted control-rod drive one frame toward `commanded` and return
/// where the rod should be **drawn**.
///
/// `commanded` and the return value are both dimensionless insertion fractions
/// in `[0, 1]` — `0.0` fully withdrawn, `1.0` fully inserted. `commanded` is the
/// simulator's own setpoint (for the HTGR example, `HtgrSnapshot`'s
/// `control_rod_insertion_fraction`); the return value lags it by the travel the
/// drive has not yet completed.
///
/// `id` names this rod bank's animation state. It must be **stable across
/// frames** and **distinct per rod bank**, or two banks will fight over one
/// drive. Deriving it from the vessel widget's own response id is the easiest
/// way to get both.
///
/// `drive_for_first_frame` is used only when there is no stored state yet — on
/// the very first frame the rod starts wherever that drive says, so a simulator
/// that begins at 60 % insertion does not animate up from zero on load.
///
/// The timestep is `egui`'s smoothed `stable_dt`, clamped to
/// [`MAX_ANIMATION_TIMESTEP_SECONDS`].
pub fn slewed_control_rod_insertion(
    ctx: &Context,
    id: Id,
    commanded: f64,
    drive_for_first_frame: ControlRodDrive,
) -> f32 {
    let mut drive: ControlRodDrive = ctx
        .data_mut(|d| d.get_temp::<ControlRodDrive>(id))
        .unwrap_or(drive_for_first_frame);

    let dt_seconds =
        f64::from(ctx.input(|i| i.stable_dt)).clamp(0.0, MAX_ANIMATION_TIMESTEP_SECONDS);
    drive.advance(commanded, Time::new::<second>(dt_seconds));

    if drive.motion() != RodDriveMotion::AtRest {
        // Keep the frames coming until the rod arrives, for an application that
        // repaints on demand rather than continuously.
        ctx.request_repaint();
    }

    let drawn = drive.insertion_fraction();
    ctx.data_mut(|d| d.insert_temp(id, drive));
    drawn as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The helper must persist the drive across frames, or the rod would restart
    /// from its initial position every repaint and never arrive.
    ///
    /// **Methodology.** Run a headless `egui` context for a sequence of frames,
    /// each commanding full insertion from a fully withdrawn start, and record
    /// the returned drawn fraction. It must increase over frames (proving state
    /// survived the repaint — a widget-owned drive would return the same value
    /// every frame), must never exceed 1.0, and must still be short of the
    /// setpoint after several frames (proving it did not teleport).
    ///
    /// **Results (2026-08-12).** Over 40 frames at the context's default
    /// `stable_dt` the drawn fraction rose monotonically from 0.0 and was still
    /// travelling on all 40 — as expected, since a full stroke takes 20 s and
    /// 40 frames is two thirds of a second. Interpretation: state is genuinely
    /// persisted outside the widget, and the animation is finite-speed rather
    /// than instantaneous.
    #[test]
    fn the_drive_persists_across_repaints_and_does_not_teleport() {
        let ctx = Context::default();
        let id = Id::new("rod_drive_test");

        let mut previous = -1.0f32;
        let mut frames_below_one = 0;
        for frame in 0..40 {
            let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
            let drawn = slewed_control_rod_insertion(&ctx, id, 1.0, ControlRodDrive::htr10(0.0));
            assert!(
                drawn >= previous,
                "frame {frame}: rod went backwards ({previous} -> {drawn})"
            );
            assert!(drawn <= 1.0, "frame {frame}: rod overshot ({drawn})");
            if drawn < 1.0 {
                frames_below_one += 1;
            }
            previous = drawn;
        }
        println!("{frames_below_one} of 40 frames were still travelling");
        assert!(
            frames_below_one >= 5,
            "the rod arrived almost immediately ({frames_below_one} travelling frames) — \
             is the state being persisted?"
        );
        assert!(previous > 0.0, "the rod never moved at all");
    }

    /// Two rod banks under different ids must not share one drive.
    ///
    /// **Methodology.** Drive two ids from the same start toward opposite
    /// setpoints for several frames, then require their drawn fractions to have
    /// diverged.
    ///
    /// **Results (2026-08-12).** After 10 frames the inserting bank read above
    /// its start and the withdrawing bank below it, with a clear gap between
    /// them. Interpretation: the id keys the state as intended, so a vessel with
    /// two independent banks (the FHR has left and right) can use this helper
    /// without the banks interfering.
    #[test]
    fn separate_ids_keep_separate_drives() {
        let ctx = Context::default();
        let inserting = Id::new("bank_a");
        let withdrawing = Id::new("bank_b");

        let (mut a, mut b) = (0.5f32, 0.5f32);
        for _ in 0..10 {
            let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
            a = slewed_control_rod_insertion(&ctx, inserting, 1.0, ControlRodDrive::htr10(0.5));
            b = slewed_control_rod_insertion(&ctx, withdrawing, 0.0, ControlRodDrive::htr10(0.5));
        }
        println!("bank A {a:.4}, bank B {b:.4}");
        assert!(a > 0.5, "the inserting bank did not insert");
        assert!(b < 0.5, "the withdrawing bank did not withdraw");
    }
}
