//! Progress of a list of data-processing jobs (tapes to process, files to
//! parse), shown as a card on the main view, so it is visible with the panel
//! folded (mobile-first rule, item 6), and as a table in the panel.

use crate::web_demo::platform::now_s;
use egui::{Color32, Pos2, Rect, Stroke, StrokeKind, Vec2};

pub struct Loading {
    /// `(label, anything)` per job, in processing order; the label is shown.
    pub labels: Vec<&'static str>,
    /// Rough cost of each job, used ONLY to weight the progress bar (jobs
    /// can differ by orders of magnitude, so counting them makes the bar
    /// jump). Any unit; only ratios matter.
    pub weights: Vec<f64>,
    pub started: f64,
    pub job_started: Vec<Option<f64>>,
    pub job_secs: Vec<Option<f64>>,
}

impl Loading {
    pub fn new(labels: Vec<&'static str>, weights: Vec<f64>) -> Self {
        let n = labels.len();
        let weights = if weights.len() == n { weights } else { vec![1.0; n] };
        Self { labels, weights, started: now_s(), job_started: vec![None; n], job_secs: vec![None; n] }
    }
    pub fn job_started(&mut self, index: usize) {
        if let Some(s) = self.job_started.get_mut(index) {
            *s = Some(now_s());
        }
    }
    pub fn job_done(&mut self, index: usize, secs: f64) {
        if let Some(s) = self.job_secs.get_mut(index) {
            *s = Some(secs);
        }
    }
    pub fn done(&self) -> usize {
        self.job_secs.iter().filter(|s| s.is_some()).count()
    }
    /// The job running now, if any.
    pub fn current(&self) -> Option<usize> {
        (0..self.labels.len()).find(|&i| self.job_started[i].is_some() && self.job_secs[i].is_none())
    }
    /// Finished jobs by weight, plus the running job's elapsed share of its
    /// expected time, capped short of done.
    pub fn fraction(&self, now: f64) -> f32 {
        let total: f64 = self.weights.iter().sum::<f64>().max(1e-9);
        let mut f: f64 = (0..self.labels.len()).filter(|&i| self.job_secs[i].is_some()).map(|i| self.weights[i]).sum();
        if let Some(i) = self.current() {
            let t = now - self.job_started[i].unwrap_or(now);
            f += self.weights[i] * (t / self.weights[i].max(0.05)).min(0.95);
        }
        (f / total).clamp(0.0, 1.0) as f32
    }
    pub fn status_line(&self) -> String {
        match self.current() {
            Some(i) => format!("{} ({} of {})", self.labels[i], i + 1, self.labels.len()),
            None if self.done() == 0 => "Downloading".into(),
            None => "Assembling the model".into(),
        }
    }
    /// `(label, seconds)` per finished job.
    pub fn timings(&self) -> Vec<(&'static str, f64)> {
        self.labels.iter().zip(&self.job_secs).map(|(l, s)| (*l, s.unwrap_or(0.0))).collect()
    }

    /// Test hook: the fraction with a job started at a known time.
    #[cfg(test)]
    fn started_at(&mut self, index: usize, t: f64) {
        self.job_started[index] = Some(t);
    }

    /// The per-job table for the side panel.
    pub fn grid(&self, ui: &mut egui::Ui) {
        let now = now_s();
        egui::Grid::new("loading_jobs").num_columns(2).spacing([16.0, 2.0]).show(ui, |ui| {
            for (i, label) in self.labels.iter().enumerate() {
                ui.label(*label);
                if let Some(s) = self.job_secs[i] {
                    ui.label(format!("done in {s:.1} s"));
                } else if let Some(t0) = self.job_started[i] {
                    ui.colored_label(Color32::from_rgb(250, 200, 80), format!("processing… {:.0} s", now - t0));
                } else {
                    ui.weak("queued");
                }
                ui.end_row();
            }
        });
    }

    /// The card on the main view: title, status, progress bar, elapsed time,
    /// and an optional warning line (wrapped). Skipped when the view is too
    /// narrow to hold it (the side panel is then open over it and shows the
    /// same).
    pub fn card(&self, painter: &egui::Painter, rect: Rect, title: &str, note: Option<&str>) {
        if rect.width() < 260.0 {
            return;
        }
        let now = now_s();
        let w = (rect.width() - 32.0).min(440.0);
        let f = |s: f32| egui::FontId::proportional(s);
        let note_galley = note.map(|n| painter.layout(n.to_string(), f(12.0), Color32::from_rgb(250, 200, 80), w - 32.0));
        let extra = note_galley.as_ref().map_or(0.0, |g| g.size().y + 10.0);
        let card = Rect::from_center_size(rect.center(), Vec2::new(w, 112.0 + extra));
        painter.rect_filled(card, 8.0, Color32::from_rgba_unmultiplied(14, 16, 22, 235));
        painter.rect_stroke(card, 8.0, Stroke::new(1.0, Color32::from_rgb(70, 80, 100)), StrokeKind::Inside);
        let left = card.left() + 16.0;
        painter.text(Pos2::new(left, card.top() + 14.0), egui::Align2::LEFT_TOP, title, f(17.0), Color32::WHITE);
        painter.text(Pos2::new(left, card.top() + 40.0), egui::Align2::LEFT_TOP, self.status_line(), f(13.0), Color32::from_rgb(200, 205, 215));
        let bar = Rect::from_min_size(Pos2::new(left, card.top() + 66.0), Vec2::new(w - 32.0, 12.0));
        painter.rect_filled(bar, 6.0, Color32::from_rgb(40, 46, 58));
        let mut fill = bar;
        fill.set_width(bar.width() * self.fraction(now));
        painter.rect_filled(fill, 6.0, Color32::from_rgb(110, 160, 255));
        painter.text(
            Pos2::new(left, card.top() + 86.0),
            egui::Align2::LEFT_TOP,
            format!("rough · {:.0} % · {:.0} s elapsed", 100.0 * self.fraction(now), now - self.started),
            f(12.0),
            Color32::from_rgb(150, 158, 175),
        );
        if let Some(g) = note_galley {
            painter.galley(Pos2::new(left, card.top() + 108.0), g, Color32::from_rgb(250, 200, 80));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bar moves by weight, not by job count, and never reaches 100 %
    /// while a job is still running.
    #[test]
    fn progress_is_weighted_and_capped_while_running() {
        let mut l = Loading::new(vec!["small", "big"], vec![1.0, 99.0]);
        l.job_done(0, 0.1);
        assert!((l.fraction(0.0) - 0.01).abs() < 1e-6);
        l.started_at(1, 0.0);
        assert!(l.fraction(1.0e6) < 1.0, "a running job never completes the bar");
        assert_eq!(l.current(), Some(1));
        l.job_done(1, 2.0);
        assert_eq!(l.done(), 2);
        assert!((l.fraction(0.0) - 1.0).abs() < 1e-6);
    }
}
