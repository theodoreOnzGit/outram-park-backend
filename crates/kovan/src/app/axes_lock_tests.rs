//! GH issue #421: the graph digitiser's "Lock axes" gate, at the state level
//! (the image panel's gestures all read the one `axes_locked` flag).
//!
//! What these pin down:
//!
//! 1. A new figure starts unlocked, and locking is refused while the
//!    calibration is incomplete.
//! 2. Locking with points already placed under a different calibration (the
//!    axes were unlocked and moved) re-reads every point from its pixel, so
//!    the numbers match the lines on screen.
//! 3. Loading a new image unlocks again; restoring a saved digitisation
//!    comes back locked, because its points were placed through those axes.

use super::*;

/// An app with a 500 x 400 blank image and a complete linear calibration:
/// x = 0..100 over columns 50..450, y = 0..50 over rows 350..50.
fn calibrated_app() -> DigitiseApp {
    let mut app = DigitiseApp::default();
    app.set_raster(
        PlotRaster::from_rgb_fn(500, 400, |_, _| [255, 255, 255]),
        String::new(),
    );
    app.figure = "Fig. 1".into();
    app.ref_px = [Some(50.0), Some(450.0), Some(350.0), Some(50.0)];
    app.ref_val = ["0", "100", "0", "50"].map(String::from);
    app
}

#[test]
fn a_new_figure_starts_unlocked_and_incomplete_axes_cannot_be_locked() {
    let mut app = calibrated_app();
    assert!(!app.axes_locked, "a new image must start unlocked");
    app.ref_val[1].clear();
    app.lock_axes();
    assert!(!app.axes_locked, "locked with an X2 value missing");
    assert!(app.message_is_error);
    assert!(app.message.contains("cannot lock"), "{}", app.message);

    app.ref_val[1] = "100".into();
    app.lock_axes();
    assert!(app.axes_locked, "{}", app.message);
    assert!(!app.message_is_error, "{}", app.message);
}

#[test]
fn relocking_moved_axes_rereads_every_point_through_them() {
    let mut app = calibrated_app();
    app.lock_axes();
    app.start_empty();
    app.add_point(250.0, 200.0); // mid-plot: (50, 25)
    app.series_name = "first".into();
    app.finish_series();
    app.add_point(450.0, 50.0); // top right: (100, 50)
    let x_of = |d: &DigitisedDataset| d.points[0].x;
    assert!((x_of(&app.completed_series[0]) - 50.0).abs() < 1e-9);
    assert!((x_of(app.dataset.as_ref().unwrap()) - 100.0).abs() < 1e-9);

    // The X2 line was on the wrong tick: unlock, move it, lock again.
    app.unlock_axes();
    assert!(!app.axes_locked);
    assert!(app.message.contains("re-read"), "{}", app.message);
    app.ref_px[1] = Some(250.0); // column 250 is now x = 100
    app.lock_axes();
    assert!(app.axes_locked);
    assert!(
        app.message.contains("2 existing point(s)"),
        "{}",
        app.message
    );
    // Every series, banked and live, now reads through the moved axes.
    assert!((x_of(&app.completed_series[0]) - 100.0).abs() < 1e-9);
    assert!((x_of(app.dataset.as_ref().unwrap()) - 200.0).abs() < 1e-9);
    // y did not move, and neither did any pixel.
    assert!((app.completed_series[0].points[0].y - 25.0).abs() < 1e-9);
    assert_eq!(app.dataset.as_ref().unwrap().points[0].x_px, Some(450.0));

    // Locking again with nothing moved re-reads nothing.
    app.unlock_axes();
    app.lock_axes();
    assert!(!app.message.contains("re-read"), "{}", app.message);
}

#[test]
fn a_new_image_unlocks_and_a_restored_digitisation_comes_back_locked() {
    let mut app = calibrated_app();
    app.lock_axes();
    app.set_raster(
        PlotRaster::from_rgb_fn(300, 200, |_, _| [255, 255, 255]),
        String::new(),
    );
    assert!(
        !app.axes_locked,
        "the old figure's lock followed a new image"
    );

    let (_dir, _root, mut app, id) = super::tests::app_with_a_saved_two_series_graph();
    let artifact = {
        let session = &app.active_paper.as_ref().unwrap().session;
        crate::research_record::ResearchRecordIndex::from_session(session)
            .artifacts()
            .iter()
            .find(|a| a.id() == id)
            .expect("the saved graph")
            .clone()
    };
    let saved = saved_digitisation::SavedDigitisation::from_artifact(&artifact, None, None);
    app.axes_locked = false;
    assert!(app.restore_saved_series(&saved), "{}", app.message);
    assert!(app.axes_locked);
}
