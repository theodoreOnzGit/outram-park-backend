//! V&V of the anchoring on kovan's three intended uses (GitHub #754), on
//! realistic text written for these tests (no third-party text).
//!
//! **Methodology.** Each test describes a span in an "old" text with the
//! public describe functions, edits the text the way the use case does,
//! anchors the stored selectors in the "new" text, and checks (i) the text
//! anchored to, (ii) the strategy that found it and (iii) the score. Each
//! test's doc comment states the prediction, written **before the first
//! run**, and then the measured result. Pass criterion: the anchored text
//! is the edited counterpart of the old span, by the predicted strategy.
//!
//! **Results** (2026-10-07, `cargo test --release -p kovan-common`): see
//! each test. Scores are upstream's normalised score, printed with
//! `--nocapture`.

use crate::anchoring::{
    anchor, anchor_in_pages, describe, describe_in_pages, lines_to_range, range_to_lines,
    AnchorStrategy, Selector,
};

fn slice(text: &str, a: usize, b: usize) -> String {
    text.chars().skip(a).take(b - a).collect()
}

fn char_index(text: &str, needle: &str) -> usize {
    text[..text.find(needle).unwrap()].chars().count()
}

// ---------------------------------------------------------------- (a) PDF

/// Page texts as a first extractor produced them: one line per text line.
fn pdf_old() -> Vec<String> {
    vec![
        "Pebble-bed reactor physics: lecture notes\nChapter 4. Reactivity feedback\n".into(),
        "4.2 Temperature feedback\nThe fuel temperature coefficient is negative in a\n\
         graphite-moderated pebble bed because Doppler broadening of the\nU-238 \
         capture resonances increases resonance absorption as the\nfuel heats up. The \
         moderator temperature coefficient is smaller\nand its sign depends on the \
         plutonium content of the fuel.\n"
            .into(),
    ]
}

/// The same pages from a different extractor: a longer running header on
/// page 1 (every document offset after it moves), lines joined with spaces
/// or not at all, doubled spaces, and the word "temperature" hyphenated
/// across a line break inside the quote.
fn pdf_new() -> Vec<String> {
    vec![
        "Pebble-bed reactor physics:  lecture notes   (draft 2) Chapter 4.  Reactivity feedback "
            .into(),
        "4.2 Temperature feedback The fuel tempera-\nture coefficient is negative in a graphite-\
         moderated pebble bed because Doppler  broadening of the U-238 capture resonances \
         increases resonance absorption as the fuel heats up. The moderator temperature \
         coefficient is smaller and its sign depends on the plutonium content of the fuel. "
            .into(),
    ]
}

/// **(a) A quote in a page of extracted PDF text, after re-extraction
/// shifts whitespace and hyphenation.**
///
/// Quote: "fuel temperature coefficient is negative" on page 1 (index 1).
///
/// Prediction (before running): the position fails (page 0 grew, so the
/// document offset points elsewhere); whitespace is ignored in paged
/// matching, so the only difference left is the hyphen in "tempera-ture":
/// one edit in a 36-character stripped quote. Expected: `FuzzyQuote`, page
/// 1, anchored text "fuel tempera-\nture coefficient is negative", score
/// about (50·(1 − 1/36) + 20 + 20 + 2·~1)/92 ≈ 0.98. Prefix: the old prefix
/// "4.2 Temperature feedback\nThe " strips to the same characters as the new
/// one, so it scores 1.0.
///
/// Result (2026-10-07, first run): as predicted: `FuzzyQuote`, page 1,
/// the anchored text is "fuel tempera-\nture coefficient is negative",
/// score 0.98405 (predicted ≈ 0.98).
#[test]
fn a_pdf_quote_survives_reextraction_with_hyphenation() {
    let old = pdf_old();
    let quote = "fuel temperature coefficient is negative";
    let start = char_index(&old[1], quote);
    let sel = describe_in_pages(&old, 1, start, start + quote.len(), None).unwrap();

    let new = pdf_new();
    let a = anchor_in_pages(&new, &sel);
    let a = a.anchor().expect("orphaned");
    eprintln!("(a) {a:?}");
    assert_eq!(a.page, Some(1));
    assert_eq!(
        slice(&new[1], a.start, a.end),
        "fuel tempera-\nture coefficient is negative"
    );
    assert_eq!(a.strategy, AnchorStrategy::FuzzyQuote);
    assert!(a.score > 0.97, "score {}", a.score);
}

/// **(a′) The same re-extraction, a quote with whitespace changes only.**
///
/// Quote: "Doppler broadening of the\nU-238 capture resonances" (it spans a
/// line break in the old extraction, a double space and a plain space in
/// the new one).
///
/// Prediction (before running): exact once whitespace is stripped, so
/// `ExactQuote` on page 1, the anchored text "Doppler  broadening of the
/// U-238 capture resonances", score 1.0 minus only the position term
/// (prefix and suffix also strip identically), i.e. > 0.99.
///
/// Result (2026-10-07, first run): as predicted: `ExactQuote`, page 1,
/// score 0.99899.
#[test]
fn a_pdf_quote_survives_whitespace_only_changes() {
    let old = pdf_old();
    let quote = "Doppler broadening of the\nU-238 capture resonances";
    let start = char_index(&old[1], quote);
    let sel = describe_in_pages(&old, 1, start, start + quote.chars().count(), None).unwrap();

    let new = pdf_new();
    let a = anchor_in_pages(&new, &sel);
    let a = a.anchor().expect("orphaned");
    eprintln!("(a') {a:?}");
    assert_eq!(a.page, Some(1));
    assert_eq!(
        slice(&new[1], a.start, a.end),
        "Doppler  broadening of the U-238 capture resonances"
    );
    assert_eq!(a.strategy, AnchorStrategy::ExactQuote);
    assert!(a.score > 0.99, "score {}", a.score);
}

// --------------------------------------------------------------- (b) note

const NOTE_OLD: &str = "# Reading notes: decay heat\n\n\
The ANS-5.1 curve is a fit to summation calculations. \
It is conservative at short cooling times.\n\n\
For HTR-10 the passive path removes decay heat by conduction and radiation. \
It is conservative at short cooling times, but the radial gap conductance is uncertain.\n";

const NOTE_NEW: &str = "# Reading notes: decay heat\n\n\
Added 2026-10: see also the PSA notes in chapter 7, which use the same curve.\n\n\
The ANS-5.1 standard curve is a fit to summation calculations. \
It is conservative at short cooling times.\n\n\
For the HTR-10 benchmark the passive path removes decay heat by conduction and thermal radiation. \
It is conservative at short cooling times, but the radial gap conductance is uncertain.\n";

/// **(b) A quote anchor whose surrounding note text was edited.**
///
/// The quote "It is conservative at short cooling times" occurs **twice**
/// in the note; the anchor is on the second (the HTR-10 paragraph). Edits:
/// a paragraph inserted above (every offset moves), "standard" inserted
/// near the first occurrence, and the second occurrence's prefix edited
/// ("For the HTR-10 benchmark … thermal radiation."). The quote itself is
/// unchanged.
///
/// Prediction (before running): the position fails (text shifted); both
/// occurrences are exact, so the context decides. Second occurrence: the
/// prefix (32 chars, "…conduction and radiation. ") now reads "…conduction
/// and thermal radiation. ": ~8 edits, prefix score ≈ 0.75; suffix exact.
/// First occurrence: prefix "…summation calculations. " vs the stored one,
/// mostly different, and suffix ".\n\nFor HTR-10…" vs ", but the radial…",
/// mostly different. Expected: `ExactQuote` at the **second** occurrence,
/// score ≈ (50 + 20·0.75 + 20 + 2·~1)/92 ≈ 0.95.
///
/// Result (2026-10-07, first run): the anchor is right (`ExactQuote`, the
/// second occurrence), but the **score prediction was refuted**: measured
/// 0.89893, predicted ≈ 0.95, and the test's original bound `> 0.9`
/// failed. Back-solving, the prefix scored ≈ 0.54, not 0.75: upstream
/// compares the stored 32-character prefix with the **32 characters** now
/// before the match, so an 8-character insertion costs about 8 edits for
/// the insertion and about 8 more for the old prefix's start falling out
/// of the window (≈ 15 edits of 32). That is upstream's scoring, ported as
/// is; an insertion of k characters in the context costs ≈ 2k.
///
/// ~~The bound is now `> 0.85`, derived from that mechanism.~~ **CORRECTED
/// 2026-10-07** (main session): loosening a bound after it failed is
/// moving a threshold to make a test pass (root CLAUDE.md), so the bound is
/// gone. The pass criterion is the anchored occurrence and strategy; the
/// score is pinned at its measured value (the algorithm is deterministic)
/// so any change to the scoring shows up, and the refuted prediction stays
/// recorded above.
#[test]
fn b_note_quote_survives_edits_around_it() {
    let quote = "It is conservative at short cooling times";
    let second_old = NOTE_OLD.rfind(quote).unwrap();
    let sel = describe(NOTE_OLD, second_old, second_old + quote.len());

    let a = anchor(NOTE_NEW, &sel);
    let a = a.anchor().expect("orphaned");
    eprintln!("(b) {a:?}");
    assert_eq!(a.start, NOTE_NEW.rfind(quote).unwrap());
    assert_eq!(&NOTE_NEW[a.start..a.end], quote);
    assert_eq!(a.strategy, AnchorStrategy::ExactQuote);
    // The measured score, pinned; the prediction (≈ 0.95, bound > 0.9) was
    // refuted, see above.
    assert!((a.score - 0.89893).abs() < 1e-5, "score {}", a.score);
}

/// **(b′) The quote itself lightly edited.**
///
/// The anchor is on "the passive path removes decay heat by conduction and
/// radiation" (60 chars); the new note reads "… by conduction and thermal
/// radiation".
///
/// Prediction (before running): no exact occurrence, so the fuzzy search,
/// with 8 edits ("thermal ") inside a 30-edit budget. Expected:
/// `FuzzyQuote`, anchored text "the passive path removes decay heat by
/// conduction and thermal radiation", score ≈ (50·(1 − 8/60) + 20·~0.8 +
/// 20 + 2)/92 ≈ 0.86.
///
/// Result (2026-10-07, first run): anchor as predicted (`FuzzyQuote`, that
/// text) but the score was lower than predicted: 0.80317 against ≈ 0.86,
/// passing the `> 0.8` bound set before the run by a small margin. The
/// quote is 63 characters, not 60 (quote score 1 − 8/63 = 0.873), and the
/// prefix scored ≈ 0.41, not 0.8: the 14 characters inserted before the
/// quote ("the ", "benchmark ") cost ≈ 2 × 14 edits in upstream's
/// fixed-length window, the same mechanism as (b).
#[test]
fn b_note_quote_survives_an_edit_inside_it() {
    let quote = "the passive path removes decay heat by conduction and radiation";
    let start = NOTE_OLD.find(quote).unwrap();
    let sel = describe(NOTE_OLD, start, start + quote.len());

    let a = anchor(NOTE_NEW, &sel);
    let a = a.anchor().expect("orphaned");
    eprintln!("(b') {a:?}");
    assert_eq!(
        &NOTE_NEW[a.start..a.end],
        "the passive path removes decay heat by conduction and thermal radiation"
    );
    assert_eq!(a.strategy, AnchorStrategy::FuzzyQuote);
    assert!(a.score > 0.8, "score {}", a.score);
}

// --------------------------------------------------------------- (c) code

const CODE_OLD: &str = "use std::f64::consts::PI;

/// Area of a pebble's surface.
pub fn pebble_area(radius: f64) -> f64 {
    4.0 * PI * radius * radius
}

/// Volumetric heat source in a pebble, W/m^3.
pub fn pebble_power_density(power_w: f64, radius: f64) -> f64 {
    let volume = 4.0 / 3.0 * PI * radius.powi(3);
    power_w / volume
}

pub fn unrelated() {}
";

const CODE_NEW: &str = "use std::f64::consts::PI;

/// Pebble radius of HTR-10 fuel, m.
pub const HTR10_PEBBLE_RADIUS: f64 = 0.03;

/// Area of a pebble's surface.
pub fn pebble_area(radius: f64) -> f64 {
    4.0 * PI * radius * radius
}

/// Volumetric heat source in a pebble, W/m^3.
pub fn pebble_power_density(power_w: f64, radius: f64) -> f64 {
    // Sphere volume.
    let vol = 4.0 / 3.0 * PI * radius.powi(3);
    power_w / vol
}

pub fn unrelated() {}
";

/// **(c) A function's lines, after lines were inserted above it and the
/// function itself was lightly edited.** The review-stamp case: a stamp
/// records 1-based inclusive `lines`.
///
/// Old: `pebble_power_density` with its doc comment is lines 8-12. New:
/// three lines inserted above (a blank, a doc comment, a const), a comment
/// line added inside, and `volume` renamed to `vol` twice.
///
/// Prediction (before running): the position fails (shifted by the
/// inserted lines); no exact occurrence; the fuzzy search finds it with
/// about 25 edits ("    // Sphere volume.\n" is 22 characters, plus 3 for
/// the two renames) in a 180-character quote (budget 90). Expected:
/// `FuzzyQuote`, lines 11-16 in the new file (8+3 to 12+3+1), score about
/// (50·(1 − 25/180) + 20 + 20 + 2)/92 ≈ 0.92 (prefix and suffix
/// unchanged: the function after it and the one before it are untouched).
///
/// Result (2026-10-07, first run): as predicted: `FuzzyQuote`, lines
/// 11-16, the anchored range is exactly the new function with its doc
/// comment; score 0.92470 (predicted ≈ 0.92).
#[test]
fn c_function_lines_survive_insertions_above_and_light_edits() {
    let (start, end) = lines_to_range(CODE_OLD, 8, 12).unwrap();
    assert!(slice(CODE_OLD, start, end).starts_with("/// Volumetric heat source"));
    let sel = describe(CODE_OLD, start, end);

    let a = anchor(CODE_NEW, &sel);
    let a = a.anchor().expect("orphaned");
    eprintln!("(c) {a:?}");
    assert_eq!(a.strategy, AnchorStrategy::FuzzyQuote);
    assert_eq!(range_to_lines(CODE_NEW, a.start, a.end), (11, 16));
    let (ns, ne) = lines_to_range(CODE_NEW, 11, 16).unwrap();
    assert_eq!((a.start, a.end), (ns, ne));
    assert!(a.score > 0.85, "score {}", a.score);
}

// ----------------------------------------------------- serde and helpers

/// The W3C JSON shape, and that a stored selector list round-trips through
/// JSON and TOML (the format kovan's records use).
#[test]
fn selectors_serialise_in_w3c_shape() {
    let sel = describe("abc def ghi", 4, 7);
    let json = serde_json::to_value(&sel).unwrap();
    assert_eq!(
        json,
        serde_json::json!([
            {"type": "TextPositionSelector", "start": 4, "end": 7},
            {"type": "TextQuoteSelector", "exact": "def", "prefix": "abc ", "suffix": " ghi"}
        ])
    );
    let back: Vec<Selector> = serde_json::from_value(json).unwrap();
    assert_eq!(back, sel);

    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct Rec {
        selector: Vec<Selector>,
    }
    let rec = Rec {
        selector: describe_in_pages(&["ab", "cd"], 1, 0, 1, None).unwrap(),
    };
    let toml_text = toml::to_string(&rec).unwrap();
    assert_eq!(toml::from_str::<Rec>(&toml_text).unwrap(), rec);
    // A quote without context omits the optional fields.
    let q: Selector = serde_json::from_str(r#"{"type":"TextQuoteSelector","exact":"x"}"#).unwrap();
    assert_eq!(
        serde_json::to_string(&q).unwrap(),
        r#"{"type":"TextQuoteSelector","exact":"x"}"#
    );
}

#[test]
fn line_helpers() {
    let t = "a\nbc\n\nd";
    assert_eq!(lines_to_range(t, 1, 1), Some((0, 1)));
    assert_eq!(lines_to_range(t, 2, 2), Some((2, 4)));
    assert_eq!(lines_to_range(t, 3, 3), Some((5, 5)));
    assert_eq!(lines_to_range(t, 2, 4), Some((2, 7)));
    assert_eq!(lines_to_range(t, 4, 5), None);
    assert_eq!(lines_to_range(t, 0, 1), None);
    assert_eq!(range_to_lines(t, 2, 4), (2, 2));
    assert_eq!(range_to_lines(t, 2, 5), (2, 2)); // ends just after a newline
    assert_eq!(range_to_lines(t, 0, 7), (1, 4));
    assert_eq!(range_to_lines(t, 6, 6), (4, 4));
}

/// The position strategy and the unverified position.
#[test]
fn position_is_tried_first_and_checked_against_the_quote() {
    let t = "alpha beta gamma beta";
    let sel = describe(t, 17, 21);
    let a = anchor(t, &sel);
    assert_eq!(a.anchor().unwrap().strategy, AnchorStrategy::Position);
    assert_eq!(a.anchor().unwrap().start, 17);
    let only_pos = [sel[0].clone()];
    assert_eq!(
        anchor("xxxxxxxxxxxxxxxxxxxxxxxxx", &only_pos)
            .anchor()
            .unwrap()
            .strategy,
        AnchorStrategy::PositionUnverified
    );
}
