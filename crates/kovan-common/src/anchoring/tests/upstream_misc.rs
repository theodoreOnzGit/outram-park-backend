//! Ports of the text-level cases in three more Hypothesis client test files
//! (commit b4d085a2f893aa6de3b61d8b8bc3ae4d0f24fc1a, BSD-2-Clause):
//!
//! - `src/annotator/anchoring/test/types-test.js`, `TextQuoteAnchor`
//!   (`fromRange`, `toSelector`, the range round trip) on its Gettysburg
//!   fixture. The `RangeAnchor`/`MediaTimeAnchor` and DOM-node cases have no
//!   plain-text counterpart and are not ported.
//! - `src/annotator/anchoring/test/html-test.js`, "When anchoring fails".
//!   Its fixture is an HTML page; the cases only need a text that contains
//!   "Lorem ipsum" and is shorter than 1000 characters, so a short Lorem
//!   ipsum paragraph stands in for it.
//! - `src/annotator/util/test/normalize-test.js`, `translateOffsets`, with
//!   upstream's `/\S/` filter (`char::is_whitespace` here). The two
//!   ligature cases need NFKD normalisation, which is not ported
//!   (`offsets` module doc), and are left out.

use crate::anchoring::offsets::translate_offsets;
use crate::anchoring::{
    anchor, describe, describe_quote, AnchorStrategy, Anchoring, OrphanReason, Selector,
    TextPositionSelector, TextQuoteSelector,
};

fn gettysburg() -> String {
    [
        "Four score and seven years ago our fathers brought forth on this continent,",
        "a new nation, conceived in Liberty, and dedicated to the proposition that",
        "all men are created equal.",
    ]
    .join(" ")
}

#[test]
fn quote_from_range_has_32_chars_of_context() {
    let text = gettysburg();
    let quote = "our fathers";
    let pos = text.find(quote).unwrap();
    let q = describe_quote(&text, pos, pos + quote.len());
    assert_eq!(q.exact, quote);
    assert_eq!(
        q.prefix.as_deref(),
        Some(&text[pos.saturating_sub(32)..pos])
    );
    assert_eq!(
        q.suffix.as_deref(),
        Some(&text[pos + quote.len()..pos + quote.len() + 32])
    );
}

#[test]
fn quote_round_trip_at_start_of_text() {
    let text = gettysburg();
    let q = describe_quote(&text, 0, 4);
    assert_eq!(
        q,
        TextQuoteSelector {
            exact: "Four".into(),
            prefix: Some(String::new()),
            suffix: Some(" score and seven years ago our f".into()),
        }
    );
    let a = anchor(&text, &[Selector::TextQuoteSelector(q)]);
    let a = a.anchor().unwrap();
    assert_eq!(&text[a.start..a.end], "Four");
}

#[test]
fn position_round_trip() {
    let text = gettysburg();
    let pos = text.find("Liberty").unwrap();
    let s = describe(&text, pos, pos + 7);
    for one in &s {
        let a = anchor(&text, std::slice::from_ref(one));
        let a = a.anchor().unwrap();
        assert_eq!(&text[a.start..a.end], "Liberty", "{one:?}");
    }
}

const LOREM: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod \
                     tempor incididunt ut labore et dolore magna aliqua.";

fn valid_quote() -> Selector {
    Selector::TextQuoteSelector(TextQuoteSelector {
        exact: "Lorem ipsum".into(),
        prefix: None,
        suffix: None,
    })
}

#[test]
fn quote_that_does_not_appear_is_orphaned() {
    let q = Selector::TextQuoteSelector(TextQuoteSelector {
        exact: "This text does not appear in the web page".into(),
        prefix: None,
        suffix: None,
    });
    assert_eq!(
        anchor(LOREM, &[q]),
        Anchoring::Orphaned(OrphanReason::NotFound)
    );
}

#[test]
fn failed_position_falls_back_to_quote() {
    let pos = Selector::TextPositionSelector(TextPositionSelector {
        start: 1000,
        end: 1010,
    });
    let a = anchor(LOREM, &[pos, valid_quote()]);
    let a = a.anchor().unwrap();
    assert_eq!(
        (a.start, a.end, a.strategy),
        (0, 11, AnchorStrategy::ExactQuote)
    );
}

#[test]
fn unsupported_selector_is_ignored() {
    // Upstream: a RangeSelector that fails must not stop the quote anchoring.
    let range: Selector = serde_json::from_str(
        r#"{"type":"RangeSelector","startContainer":"/main","startOffset":1,"endContainer":"/main","endOffset":5}"#,
    )
    .unwrap();
    assert_eq!(range, Selector::Unsupported);
    let a = anchor(LOREM, &[range, valid_quote()]);
    assert_eq!(a.anchor().unwrap().start, 0);
}

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

fn not_ws(c: char) -> bool {
    !c.is_whitespace()
}

#[test]
fn translate_offsets_cases() {
    let cases = [
        ("abcd", "abcd", "abcd", "abcd"),
        (
            "   ab    cd  ",
            " a   b  c   d ",
            "ab    cd",
            "a   b  c   d",
        ),
        (" foo   bar\nbaz", "foob  arbaz", "bar", "b  ar"),
    ];
    for (in_str, out_str, in_match, out_match) in cases {
        let start = in_str.find(in_match).unwrap();
        let end = start + in_match.len();
        let exp = out_str.find(out_match).unwrap();
        let got = translate_offsets(
            &chars(in_str),
            &chars(out_str),
            start as i64,
            end as i64,
            not_ws,
        );
        assert_eq!(
            got,
            (exp, exp + out_match.len()),
            "{in_str:?} -> {out_str:?}"
        );
    }
}

#[test]
fn translate_offsets_edge_cases() {
    // Only ignored characters: offsets at the end of the string.
    assert_eq!(
        translate_offsets(&chars("     "), &chars("     "), 0, 5, not_ws),
        (5, 5)
    );
    // Start < 0.
    let out = "  a      bcd";
    assert_eq!(
        translate_offsets(&chars("abcd"), &chars(out), -3, 4, not_ws),
        (out.find('a').unwrap(), out.find('d').unwrap() + 1)
    );
    // End past the input.
    assert_eq!(
        translate_offsets(&chars("abcd"), &chars(out), 2, 9, not_ws),
        (out.find('c').unwrap(), out.find('d').unwrap() + 1)
    );
    // Equal start and end.
    let out = "a b c d";
    let c = out.find('c').unwrap();
    assert_eq!(
        translate_offsets(&chars("abcd"), &chars(out), 2, 2, not_ws),
        (c, c)
    );
    // Never beyond the output.
    let input = "foo bar baz";
    let start = input.find("bar").unwrap();
    for out in ["", "foo   b", "fooba"] {
        let (s, e) = translate_offsets(
            &chars(input),
            &chars(out),
            start as i64,
            (start + 3) as i64,
            not_ws,
        );
        let sub = &out[s..e];
        assert_eq!(sub, &input[start..start + sub.len()]);
        assert!(s <= out.len() && e <= out.len());
    }
}
