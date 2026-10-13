//! Drawing [`crate::syntax`]'s runs with egui (GitHub #857). GUI drawing
//! code: no test reaches the painting itself; the cutting into runs and the
//! colour table are tested in `crate::syntax`, and `job_keeps_the_text`
//! below checks the text survives.

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId};

use crate::syntax::{self, HighlightKind, HighlightSpan};

/// The colour of `kind` under `ui`'s light or dark visuals.
pub fn colour(kind: HighlightKind, dark: bool) -> Color32 {
    let (r, g, b) = syntax::rgb(kind, dark);
    Color32::from_rgb(r, g, b)
}

/// `line` as one piece of laid-out text, each run in its colour. Plain runs
/// take `plain`; `background` (a selection or an annotation tint) lies
/// behind the whole line.
pub fn job(line: &str, spans: &[HighlightSpan], font: FontId, plain: Color32, dark: bool, background: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    for (text, kind) in syntax::runs(line, spans) {
        let color = kind.map_or(plain, |k| colour(k, dark));
        job.append(text, 0.0, TextFormat { font_id: font.clone(), color, background, ..Default::default() });
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_keeps_the_text() {
        let line = "    let σ = stress(r, p); // Pa";
        let spans = &syntax::highlight(line, syntax::Language::Rust)[0];
        let j = job(line, spans, FontId::monospace(14.0), Color32::WHITE, true, Color32::TRANSPARENT);
        assert_eq!(j.text, line);
        assert!(j.sections.len() > 3, "several coloured runs");
        assert!(j.sections.iter().any(|s| s.format.color == colour(HighlightKind::Keyword, true)));
    }
}
