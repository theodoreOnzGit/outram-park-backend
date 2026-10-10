//! **Rung 5, IV&V, in the review bar** (GitHub #810): one shared section
//! for kovan-web and desktop kovan (which embeds this UI). For the selected
//! function it shows whether independent V&V is reached, every review's
//! plain-English reasons it is not, the separation attestation's audit
//! record as a link labelled "audit record (not verified by kovan)" (never
//! fetched), the "independent V&V not counted" flag and the registry
//! records that do not verify.
//!
//! The data is [`IvvSummary`] on the function's
//! [`kovan_common::call_graph::split::StampState::ivv`], filled by desktop
//! kovan's `stamping::stamp_states` and by `kovan-cli call-graph
//! --split-dir`; absent (older data, or no valid review) the section is not
//! drawn. Folded by default, so the bar stays one line at phone width.

use kovan_common::review::ivv_view::{headline_tone, ivv_lines, IvvSummary};

use super::bar::tone_colour;

/// Draw the section for function `id`. GUI drawing code (exempt from the
/// test rule): the text is `kovan_common`'s [`ivv_lines`] (shared with
/// `kovan-cli review ivv`); the test below draws it headless.
/// `open`: unfolded the first time it is drawn (the bar passes `false`).
pub fn show(ui: &mut egui::Ui, id: &str, s: &IvvSummary, open: bool) {
    let head = egui::RichText::new(s.headline()).color(tone_colour(headline_tone(s)));
    egui::CollapsingHeader::new(head)
        .id_salt(("ivv", id))
        .default_open(open)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt(("ivv-scroll", id))
                .max_height(160.0)
                .show(ui, |ui| {
                    for l in ivv_lines(s) {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new(&l.text).color(tone_colour(l.tone)));
                            if let Some(u) = &l.link {
                                ui.hyperlink_to(u, u);
                            }
                        });
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use kovan_common::review::ivv_view::{IvvCandidateSummary, IvvPassSummary};
    use kovan_common::review::state::Tone;

    const URL: &str = "https://github.com/o/r/issues/1";

    /// Methodology: a pass, a flagged miss and a warning, as lines. Pass:
    /// the audit record is a link with the "not verified by kovan" label;
    /// every reason appears as its own line; tones are good for the pass
    /// and attention for the flag and warning; an ordinary miss is neutral.
    ///
    /// Result (2026-10-10): passes.
    #[test]
    fn lines_show_pass_reasons_links_flag_and_warnings() {
        let pass = IvvSummary {
            pass: Some(IvvPassSummary {
                reviewer: "github:v".into(),
                organisation: "A".into(),
                developing_organisation: "B".into(),
                attestation: "sep-1".into(),
                audit_record: URL.into(),
            }),
            candidates: vec![IvvCandidateSummary {
                reviewer: "github:v".into(),
                date: "2026-10-10".into(),
                review: "review-1".into(),
                attestation: Some("sep-1".into()),
                audit_record: Some(URL.into()),
                reasons: vec![],
            }],
            ..IvvSummary::default()
        };
        assert_eq!(headline_tone(&pass), Tone::Good);
        let l = ivv_lines(&pass);
        assert!(l[0]
            .text
            .starts_with("passed: github:v (A) is separate from B"));
        assert_eq!(l[1].text, "audit record (not verified by kovan):");
        assert_eq!(l[1].link.as_deref(), Some(URL));
        assert!(l.iter().any(|x| x.text.contains("gives rung 5")));

        let miss = IvvSummary {
            candidates: vec![IvvCandidateSummary {
                reviewer: "github:v".into(),
                attestation: Some("sep-1".into()),
                audit_record: Some("https://github.com/o/r/pull/1".into()),
                reasons: vec!["the function has no known concept area".into(), "x".into()],
                ..IvvCandidateSummary::default()
            }],
            not_counted: vec!["independent V&V not counted: review r names …".into()],
            warnings: vec!["developing organisation record 1: it is not signed".into()],
            ..IvvSummary::default()
        };
        assert_eq!(headline_tone(&miss), Tone::Attention);
        assert_eq!(
            miss.headline(),
            "IV&V (rung 5): independent V&V not counted"
        );
        let l = ivv_lines(&miss);
        assert_eq!(l[0].tone, Tone::Attention);
        assert!(l
            .iter()
            .any(|x| x.text == "  - the function has no known concept area"));
        assert!(l
            .iter()
            .any(|x| x.link.as_deref() == Some("https://github.com/o/r/pull/1")));
        assert!(l
            .last()
            .unwrap()
            .text
            .starts_with("record does not verify: "));
        let ordinary = IvvSummary {
            candidates: vec![IvvCandidateSummary {
                reviewer: "github:m".into(),
                reasons: vec!["the reviewer gave the first review".into()],
                ..IvvCandidateSummary::default()
            }],
            ..IvvSummary::default()
        };
        assert_eq!(headline_tone(&ordinary), Tone::Neutral);
        assert_eq!(ordinary.headline(), "IV&V (rung 5): not reached");

        // Headless: the section draws folded and open (links included)
        // without a display or a panic.
        let ctx = egui::Context::default();
        for open in [false, true] {
            for s in [&pass, &miss, &ordinary] {
                let id = if open { "f-open" } else { "f" };
                let _ = ctx.run_ui(Default::default(), |ui| show(ui, id, s, open));
            }
        }
    }
}
