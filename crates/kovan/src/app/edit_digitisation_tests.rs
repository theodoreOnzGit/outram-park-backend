//! Headless reproductions for the "Edit digitisation" round trip when the
//! saved calibration header does not describe the saved values (maintainer
//! report 2026-09-28, PANAMA-I Fig. 7: header hand-edited `= 10` -> `= 1`,
//! then Edit -> prefilled wizard -> Start digitising -> Save, and every value
//! changed).
//!
//! What these pin down (see `DECISIONS.md`, 2026-09-28 "Edit digitisation:
//! the saved values are the data"):
//!
//! 1. An untouched wizard never rewrites a value, **even when the header was
//!    hand-edited**: the prefill and the restore both read the same (edited)
//!    header, so they agree and the values are kept byte-for-byte. The
//!    markers are drawn where the edited header puts those values, which is
//!    off the curve when the header is wrong -- that is the visible symptom,
//!    and the values are still untouched.
//! 2. A range changed in the wizard re-reads every restored value from its
//!    pixel, so it is refused until the operator ticks the explicit
//!    confirmation that names how many points will be recomputed.
//! 3. A restore that fails does not throw the saved points away.

use super::tests::{app_with_a_saved_two_series_graph, graph_bodies};
use super::*;

fn provenance(id: &str) -> CropProvenance {
    CropProvenance {
        page_index: 0,
        min: Pos2::ZERO,
        max: Pos2::new(500.0, 400.0),
        page_px: [0.0, 0.0],
        created_at: "2026-09-28T00:00:00Z".to_string(),
        author: "tester".to_string(),
        figure: "Fig. 7".to_string(),
        source_artifact_id: Some(id.to_string()),
    }
}

fn reopen(app: &mut DigitiseApp, id: &str) {
    app.route_crop(pdf_reader::CropResult::Plot(
        PlotRaster::from_rgb_fn(500, 400, |_, _| [255, 255, 255]),
        provenance(id),
    ));
    assert_eq!(app.view, View::PlotSetup);
}

fn values(app: &DigitiseApp) -> Vec<Vec<(f64, f64)>> {
    app.all_series()
        .iter()
        .map(|d| d.points.iter().map(|p| (p.x, p.y)).collect())
        .collect()
}

/// Rewrite the saved artifact's y-axis header the way the maintainer did by
/// hand: the top VALUE changed, both pixels left alone.
fn hand_edit_y_top(app: &mut DigitiseApp, from: &str, to: &str) {
    let session = &mut app.active_paper.as_mut().unwrap().session;
    let md = session.markdown().to_string();
    let old = format!("px 20 = {from}\"");
    assert!(md.contains(&old), "fixture header not found:\n{md}");
    session.set_markdown(md.replace(&old, &format!("px 20 = {to}\"")));
}

fn y_axis_header(app: &DigitiseApp) -> String {
    let session = &app.active_paper.as_ref().unwrap().session;
    let index = crate::research_record::ResearchRecordIndex::from_session(session);
    let g: Vec<_> = index
        .artifacts()
        .iter()
        .filter(|a| a.kind() == crate::artifact::ArtifactKind::DigitisedGraph)
        .collect();
    assert_eq!(g.len(), 1);
    g[0].toml
        .extraction
        .as_ref()
        .unwrap()
        .y_axis
        .clone()
        .unwrap()
}

/// Hypothesis (a1), predicted by reading the code: the prefill and the
/// restore both parse the same (hand-edited) header, so an untouched wizard
/// gives `saved_calibration == live calibration` and the values are kept
/// exactly; only the marker positions move. If instead the edit made the
/// restore re-read every value, the y values would be compressed in log about
/// the bottom reference by (log 100 - log 1e-3)/(log 1000 - log 1e-3) = 5/6
/// -- the Fig. 7 fingerprint (6/7) in this fixture's numbers. Result: kept
/// exactly (hypothesis (a1) refuted as the cause of the Fig. 7 change).
#[test]
fn an_untouched_edit_after_a_hand_edited_header_keeps_every_value() {
    let (_dir, _root, mut app, id) = app_with_a_saved_two_series_graph();
    let before = values(&app);
    let body_before = graph_bodies(&app);
    hand_edit_y_top(&mut app, "1000", "100");

    reopen(&mut app, &id);
    assert_eq!(
        app.plot_setup.y_max, "100",
        "prefilled from the edited header"
    );
    assert!(app.plot_setup.recalibration_warning().is_none());
    app.finish_plot_setup();
    assert_eq!(app.view, View::Digitiser);
    assert_eq!(values(&app), before, "no value re-read");
    // The marker is where the EDITED header puts the value -- not on the
    // pixel it was read from (y = 300). This is how a wrong header shows.
    let p0 = &app.completed_series[0].points[0];
    assert!((p0.x_px.unwrap() - 100.0).abs() < 1e-9);
    assert!((p0.y_px.unwrap() - 300.0).abs() > 1.0, "{:?}", p0.y_px);

    app.save_into_project_then_read();
    assert!(!app.message_is_error, "{}", app.message);
    assert_eq!(graph_bodies(&app), body_before, "byte-identical body");
    assert_eq!(
        y_axis_header(&app),
        "log scale, px 380 = 0.001 , px 20 = 100"
    );
}

/// A prefilled range the operator retypes to the same number in another
/// spelling is not a change: nothing is re-read and nothing asks.
#[test]
fn respelling_a_prefilled_range_is_not_a_change() {
    let (_dir, _root, mut app, id) = app_with_a_saved_two_series_graph();
    let before = values(&app);
    reopen(&mut app, &id);
    app.plot_setup.y_min = "1e-3".into();
    app.plot_setup.y_max = " 1000.0 ".into();
    assert!(app.plot_setup.recalibration_warning().is_none());
    app.finish_plot_setup();
    assert_eq!(values(&app), before);
}

/// Hypothesis (a2): a range changed in the wizard re-reads every restored
/// value from its pixel. That must be an explicit act: Start digitising is
/// refused until the operator confirms, and the warning names the count.
#[test]
fn a_changed_range_recomputes_only_after_explicit_confirmation() {
    let (_dir, _root, mut app, id) = app_with_a_saved_two_series_graph();
    let before = values(&app);
    reopen(&mut app, &id);
    app.plot_setup.y_max = "100".into();
    let warn = app.plot_setup.recalibration_warning().expect("warned");
    assert!(warn.contains("3 saved point"), "{warn}");
    app.plot_setup.stage = plot_setup::Stage::Ranges;
    assert!(app.plot_setup.blocking_error().is_some(), "Next is blocked");

    // Pressed anyway (e.g. a stale button): refused, nothing restored.
    app.finish_plot_setup();
    assert_eq!(app.view, View::PlotSetup, "stays in the wizard");
    assert!(app.message_is_error, "{}", app.message);
    assert!(app.all_series().is_empty());

    // Revert: the saved ranges come back and nothing is re-read.
    app.plot_setup.revert_to_saved_ranges();
    assert_eq!(app.plot_setup.y_max, "1000");
    assert!(app.plot_setup.recalibration_warning().is_none());

    // Change again and confirm: now the values are re-read from pixels.
    app.plot_setup.y_max = "100".into();
    app.plot_setup.recalibration_confirmed = true;
    assert!(app.plot_setup.blocking_error().is_none());
    app.finish_plot_setup();
    assert_eq!(app.view, View::Digitiser);
    let after = values(&app);
    assert_ne!(after, before);
    assert!(app.message.contains("re-read"), "{}", app.message);
    // Same pixel, new calibration: log y compressed by 5/6 about 1e-3.
    let (y0, y1) = (before[0][0].1, after[0][0].1);
    let expect = 10f64.powf((y0.log10() + 3.0) * 5.0 / 6.0 - 3.0);
    assert!((y1 - expect).abs() < 1e-9 * expect, "{y1} vs {expect}");
}

/// Latent defect found on the way: `finish_plot_setup` took the pending
/// restore and ignored a failed `restore_saved_series`, so a failure lost the
/// saved points for the session. They must stay pending, with the retry
/// button, instead.
#[test]
fn a_failed_restore_keeps_the_saved_points_pending() {
    let (_dir, _root, mut app, id) = app_with_a_saved_two_series_graph();
    reopen(&mut app, &id);
    app.plot_setup.figure = String::new(); // FigureSource::new refuses it
    app.finish_plot_setup();
    assert!(app.message_is_error, "{}", app.message);
    assert!(app.all_series().is_empty());
    assert!(
        app.pending_restore.is_some(),
        "the saved points are still there to restore"
    );
}

/// Hypothesis (b): the edit path drops points or series -- a fourth series,
/// a series named like Fig. 7's "Level of Heavy Metal Contamination", or a
/// lone early point like its 99.8 h measurement. Prediction: nothing in
/// `saved_series`/`restore_saved_series` filters by name, count or position
/// (points outside the crop are counted and warned about, not dropped), so
/// every series and point should survive an untouched round trip. Result:
/// all four series and every point survive, byte-identical.
#[test]
fn four_series_and_a_lone_early_point_survive_an_untouched_round_trip() {
    let (_dir, _root, mut app, id) = app_with_a_saved_two_series_graph();
    // Two more curves on the same figure, then re-save over the artifact.
    reopen(&mut app, &id);
    app.finish_plot_setup();
    app.finish_series(); // bank "1700 degC"
    app.add_point(45.0, 390.0); // one lone point near the axes' corner
    app.series_name = "FRJ2-K11/03 9.0% FIMA".into();
    app.finish_series();
    for (px, py) in [(60.0, 200.0), (300.0, 200.0), (450.0, 205.0)] {
        app.add_point(px, py);
    }
    app.series_name = "Level of Heavy Metal Contamination".into();
    app.save_into_project_then_read();
    assert!(!app.message_is_error, "{}", app.message);
    let before = values(&app);
    let body_before = graph_bodies(&app);
    assert_eq!(before.len(), 4);

    reopen(&mut app, &id);
    app.finish_plot_setup();
    assert_eq!(values(&app), before, "every series and point restored");
    app.save_into_project_then_read();
    assert_eq!(graph_bodies(&app), body_before);
}
