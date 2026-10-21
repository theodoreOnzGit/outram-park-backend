//! Hop between the curves on one figure (maintainer, 2026-09-28: "I want to
//! be able to select data in previous banks, like through a dropdown menu,
//! or as clickable buttons").
//!
//! Every edit path in the digitiser (add, drag, erase, right-drag erase in
//! Draw trace, review) acts on the one live `dataset`. Rather than teach each
//! of them about "which series", selecting a banked series **swaps** it with
//! the live one: the live curve is banked under its name (if it has points),
//! and the chosen one becomes live, name and all. Nothing is copied twice and
//! nothing is dropped.
//!
//! **Order.** The saved artifact lists the series in the order they were
//! traced. A naive swap (bank the live curve at the end, pull the selected
//! one out) would reorder them on every hop, so the live curve's place in
//! that order is kept in `live_position`: `all_series()` puts it back there,
//! and banking it re-inserts it there.

use super::DigitiseApp;
use crate::digitiser::dataset::DigitisedDataset;
use eframe::egui;

/// With more series than this the picker is a dropdown rather than a row of
/// buttons: a wrapped row of a dozen names stops being scannable.
const MAX_SERIES_BUTTONS: usize = 6;

/// One entry in the series picker, in saved order.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SeriesEntry {
    /// Index into `completed_series`, or `None` for the live curve.
    pub banked_index: Option<usize>,
    pub name: String,
    pub points: usize,
}

impl DigitiseApp {
    /// Where the live curve sits in the saved order: before
    /// `completed_series[k]`, or after all of them. Clamped, so a banked
    /// series removed from under it cannot push it out of range.
    pub(super) fn live_index(&self) -> usize {
        self.live_position
            .unwrap_or(self.completed_series.len())
            .min(self.completed_series.len())
    }

    /// The name the live curve is saved under: the name box, which is where
    /// a rename happens.
    fn live_series_name(&self) -> String {
        self.series_name.trim().to_string()
    }

    /// Every series on the figure in saved order, the live one included
    /// (even while empty, so the picker can show it).
    pub(super) fn series_entries(&self) -> Vec<SeriesEntry> {
        let mut out: Vec<SeriesEntry> = self
            .completed_series
            .iter()
            .enumerate()
            .map(|(i, d)| SeriesEntry {
                banked_index: Some(i),
                name: d.series.clone().unwrap_or_else(|| "(unnamed)".into()),
                points: d.points.len(),
            })
            .collect();
        if let Some(d) = &self.dataset {
            let name = self.live_series_name();
            out.insert(
                self.live_index(),
                SeriesEntry {
                    banked_index: None,
                    name: if name.is_empty() {
                        "(live, unnamed)".into()
                    } else {
                        name
                    },
                    points: d.points.len(),
                },
            );
        }
        out
    }

    /// Make banked series `i` the live one.
    ///
    /// The current live curve is banked under the name in the name box first
    /// if it has points, at its own place in the saved order; an empty live
    /// curve is simply discarded (it would save as a named series with no
    /// rows). Refused, with nothing changed, if the live curve has points but
    /// no name, or a name another series already has -- the same rules as
    /// "Bank & start next". Returns whether the swap happened.
    pub(super) fn select_banked_series(&mut self, i: usize) -> bool {
        if i >= self.completed_series.len() {
            self.set_error(format!("no banked series #{}", i + 1));
            return false;
        }
        let live_has_points = self.dataset.as_ref().is_some_and(|d| !d.points.is_empty());
        let live_name = self.live_series_name();
        if live_has_points {
            if live_name.is_empty() {
                self.set_error("name the current series before switching to another");
                return false;
            }
            if self
                .completed_series
                .iter()
                .any(|d| d.series.as_deref() == Some(live_name.as_str()))
            {
                self.set_error(format!(
                    "a series called {live_name:?} is already on this figure — rename the \
                     current one before switching"
                ));
                return false;
            }
        }

        // The full saved order, live included when it counts, then take the
        // selected one out: the ones left are the new bank, and the selected
        // curve's place is where the live one now sits.
        let live_at = self.live_index();
        let mut full: Vec<DigitisedDataset> = std::mem::take(&mut self.completed_series);
        let mut global = i;
        if let (true, Some(mut live)) = (live_has_points, self.dataset.take()) {
            live.series = Some(live_name.clone());
            full.insert(live_at, live);
            if live_at <= i {
                global += 1;
            }
        }
        let selected = full.remove(global);
        self.completed_series = full;
        self.live_position = Some(global);

        let name = selected.series.clone().unwrap_or_default();
        let n = selected.points.len();
        self.series_name = name.clone();
        self.dataset = Some(selected);
        self.selected = None;
        self.dragging = None;
        self.hovered_series = None;
        let mut msg = format!("editing series {name:?} ({n} point(s))");
        if live_has_points {
            msg.push_str(&format!("; {live_name:?} banked"));
        }
        self.set_status(msg);
        true
    }

    /// The `(name, csv)` of every series in saved order, as the
    /// multi-series artifact body writes them. The live curve takes the name
    /// in the name box (a rename there applies to it), else its own
    /// `series`, else `series-N`.
    pub(super) fn named_series_csv(&self) -> Vec<(String, String)> {
        let live = self.dataset.as_ref();
        let live_name = self.live_series_name();
        self.all_series()
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let is_live = live.is_some_and(|l| std::ptr::eq(l, *d));
                let name = (is_live && !live_name.is_empty())
                    .then(|| live_name.clone())
                    .or_else(|| d.series.clone().filter(|n| !n.trim().is_empty()))
                    .unwrap_or_else(|| format!("series-{}", i + 1));
                (name, d.to_csv_data_only())
            })
            .collect()
    }

    /// The picker: a row of buttons (one per series, live one highlighted),
    /// or a dropdown once there are more than [`MAX_SERIES_BUTTONS`].
    /// Hovering a button highlights that series on the figure.
    pub(super) fn series_picker_ui(&mut self, ui: &mut egui::Ui) {
        self.hovered_series = None;
        let entries = self.series_entries();
        if entries.len() < 2 {
            return;
        }
        let label = |e: &SeriesEntry| format!("{} ({})", e.name, e.points);
        let mut pick: Option<usize> = None;
        if entries.len() <= MAX_SERIES_BUTTONS {
            ui.horizontal_wrapped(|ui| {
                ui.weak("edit:");
                for e in &entries {
                    let live = e.banked_index.is_none();
                    let text = if live {
                        format!("\u{270E} {}", label(e))
                    } else {
                        label(e)
                    };
                    let r = ui.selectable_label(live, text);
                    let r = if live {
                        r.on_hover_text("the series being edited now")
                    } else {
                        r.on_hover_text(
                            "make this the series being edited; the current one is banked",
                        )
                    };
                    if r.hovered() {
                        self.hovered_series = e.banked_index;
                    }
                    if r.clicked() {
                        pick = e.banked_index;
                    }
                }
            });
        } else {
            let current = entries
                .iter()
                .find(|e| e.banked_index.is_none())
                .map(label)
                .unwrap_or_else(|| "(none)".into());
            ui.horizontal(|ui| {
                ui.weak("edit:");
                egui::ComboBox::from_id_salt("digitiser_series_picker")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        for e in &entries {
                            let r = ui.selectable_label(e.banked_index.is_none(), label(e));
                            if r.hovered() {
                                self.hovered_series = e.banked_index;
                            }
                            if r.clicked() {
                                pick = e.banked_index;
                            }
                        }
                    });
            });
        }
        if let Some(i) = pick {
            self.select_banked_series(i);
        }
    }

    /// Draw the banked (not live) series faintly, so every curve on the
    /// figure is visible while one is edited; the hovered one prominently.
    pub(super) fn paint_banked_series(
        &self,
        painter: &egui::Painter,
        to_screen: impl Fn(f64, f64) -> egui::Pos2,
    ) {
        use egui::{Color32, Stroke};
        for (i, d) in self.completed_series.iter().enumerate() {
            let hovered = self.hovered_series == Some(i);
            for p in &d.points {
                let (Some(x), Some(y)) = (p.x_px, p.y_px) else {
                    continue;
                };
                let pos = to_screen(x, y);
                if hovered {
                    painter.circle_filled(pos, 3.5, Color32::from_rgb(40, 120, 230));
                    painter.circle_stroke(pos, 5.5, Stroke::new(1.5_f32, Color32::WHITE));
                } else {
                    painter.circle_filled(
                        pos,
                        2.0,
                        Color32::from_rgba_unmultiplied(110, 110, 170, 120),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digitiser::dataset::{DigitisedPoint, PointOrigin};

    fn point(x: f64) -> DigitisedPoint {
        DigitisedPoint {
            x,
            y: x * 10.0,
            x_px: Some(x),
            y_px: Some(x),
            x_minus: 0.0,
            x_plus: 0.0,
            y_minus: 0.0,
            y_plus: 0.0,
            origin: PointOrigin::HandPlaced {
                by: "unit test".into(),
            },
        }
    }

    /// Three banked-then-live curves A (1 pt), B (2 pts), C (3 pts, live).
    fn three_series_app() -> DigitiseApp {
        let mut app = DigitiseApp::default();
        let mut d = super::super::tests::sample_dataset_for_series();
        d.points.clear();
        app.dataset = Some(d);
        for (name, n) in [("A", 1), ("B", 2)] {
            let live = app.dataset.as_mut().unwrap();
            live.points = (0..n).map(|k| point(k as f64)).collect();
            app.series_name = name.into();
            app.finish_series();
        }
        app.dataset.as_mut().unwrap().points = (0..3).map(|k| point(k as f64)).collect();
        app.series_name = "C".into();
        app
    }

    fn order(app: &DigitiseApp) -> Vec<(String, usize)> {
        app.named_series_csv()
            .into_iter()
            .zip(app.all_series())
            .map(|((n, _), d)| (n, d.points.len()))
            .collect()
    }

    fn total_points(app: &DigitiseApp) -> usize {
        app.all_series().iter().map(|d| d.points.len()).sum()
    }

    #[test]
    fn selecting_a_banked_series_makes_it_live_and_banks_the_old_live_one() {
        let mut app = three_series_app();
        let before = order(&app);
        let total = total_points(&app);

        assert!(app.select_banked_series(0));
        assert_eq!(app.series_name, "A");
        assert_eq!(app.dataset.as_ref().unwrap().points.len(), 1);
        let banked: Vec<_> = app
            .completed_series
            .iter()
            .map(|d| d.series.clone().unwrap())
            .collect();
        assert_eq!(banked, vec!["B", "C"], "old live C banked under its name");
        assert_eq!(total_points(&app), total, "points conserved");
        assert_eq!(order(&app), before, "saved order unchanged");
    }

    #[test]
    fn hopping_repeatedly_never_scrambles_the_order_or_loses_points() {
        let mut app = three_series_app();
        let before = order(&app);
        let total = total_points(&app);
        // live C -> B -> A -> C -> B(index of B among banked each time).
        for want in ["B", "A", "C", "B", "A"] {
            let i = app
                .completed_series
                .iter()
                .position(|d| d.series.as_deref() == Some(want))
                .unwrap();
            assert!(app.select_banked_series(i), "{}", app.message);
            assert_eq!(app.series_name, want);
            assert_eq!(order(&app), before, "after selecting {want}");
            assert_eq!(total_points(&app), total);
        }
    }

    #[test]
    fn selecting_the_live_series_is_a_no_op() {
        let mut app = three_series_app();
        let before = order(&app);
        let entries = app.series_entries();
        let live = entries.iter().find(|e| e.banked_index.is_none()).unwrap();
        assert_eq!(live.name, "C");
        // The picker only ever calls select_banked_series for a banked entry;
        // the live one carries no index, so clicking it does nothing.
        assert_eq!(live.banked_index, None);
        assert_eq!(order(&app), before);
        // Out-of-range is refused without change.
        assert!(!app.select_banked_series(9));
        assert_eq!(order(&app), before);
    }

    #[test]
    fn an_empty_live_curve_is_not_banked() {
        let mut app = three_series_app();
        app.finish_series(); // bank C, live now empty
        assert_eq!(app.completed_series.len(), 3);
        assert!(app.select_banked_series(1));
        assert_eq!(app.series_name, "B");
        let banked: Vec<_> = app
            .completed_series
            .iter()
            .map(|d| d.series.clone().unwrap())
            .collect();
        assert_eq!(banked, vec!["A", "C"], "no empty series appeared");
        assert_eq!(
            order(&app).into_iter().map(|(n, _)| n).collect::<Vec<_>>(),
            vec!["A", "B", "C"]
        );
    }

    #[test]
    fn an_unnamed_or_duplicate_live_curve_blocks_the_switch() {
        let mut app = three_series_app();
        let before = order(&app);
        app.series_name = "  ".into();
        assert!(!app.select_banked_series(0));
        assert!(app.message_is_error);
        app.series_name = "B".into();
        assert!(!app.select_banked_series(0));
        assert!(app.message_is_error);
        app.series_name = "C".into();
        assert_eq!(order(&app), before, "nothing changed");
    }

    #[test]
    fn a_rename_in_the_name_box_applies_to_the_live_series() {
        let mut app = three_series_app();
        assert!(app.select_banked_series(0)); // A live, carrying series=Some("A")
        app.series_name = "A (renamed)".into();
        let names: Vec<_> = app.named_series_csv().into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, vec!["A (renamed)", "B", "C"]);
        // And hopping away banks it under the new name.
        assert!(app.select_banked_series(1)); // C
        assert_eq!(
            app.completed_series[0].series.as_deref(),
            Some("A (renamed)")
        );
    }

    #[test]
    fn banking_a_mid_order_live_series_keeps_its_place() {
        let mut app = three_series_app();
        assert!(app.select_banked_series(1)); // B live, in the middle
        app.dataset.as_mut().unwrap().points.push(point(7.0));
        app.finish_series();
        let names: Vec<_> = app
            .completed_series
            .iter()
            .map(|d| d.series.clone().unwrap())
            .collect();
        assert_eq!(names, vec!["A", "B", "C"], "B banked back in its place");
        assert_eq!(app.completed_series[1].points.len(), 3);
        assert_eq!(app.live_index(), 3, "the next curve goes at the end");
    }

    /// Saving after hopping writes every series, in the order traced, under
    /// the right names -- through the real save path into a paper session.
    #[test]
    fn saving_after_hopping_writes_every_series_in_original_order() {
        let (_dir, _root, mut app, _id) = super::super::tests::app_with_a_saved_two_series_graph();
        // "1600 degC" banked, "1700 degC" live (1 point). Hop back to 1600.
        assert!(app.select_banked_series(0), "{}", app.message);
        assert_eq!(app.series_name, "1600 degC");
        app.add_point(300.0, 200.0); // a third point on 1600
        app.save_into_project_then_read();
        assert!(!app.message_is_error, "{}", app.message);
        let bodies = super::super::tests::graph_bodies(&app);
        assert_eq!(bodies.len(), 1, "replaced, not appended");
        let body = &bodies[0].1;
        let a = body.find("1600 degC").expect("1600 saved");
        let b = body.find("1700 degC").expect("1700 saved");
        assert!(a < b, "saved order kept:\n{body}");
        let names: Vec<_> = app.named_series_csv().into_iter().map(|(n, _)| n).collect();
        assert_eq!(names, vec!["1600 degC", "1700 degC"]);
        let counts: Vec<_> = app.all_series().iter().map(|d| d.points.len()).collect();
        assert_eq!(counts, vec![3, 1]);
    }

    /// Headless: the button row renders one button per series and clicking
    /// nothing changes nothing (a frame through the real UI code).
    #[test]
    fn the_picker_renders_headlessly() {
        let mut app = three_series_app();
        let before = order(&app);
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run_ui(Default::default(), |ui| app.series_picker_ui(ui));
        }
        assert_eq!(order(&app), before);
        assert_eq!(app.hovered_series, None);
    }
}
