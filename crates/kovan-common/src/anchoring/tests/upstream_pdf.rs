//! Port of the Hypothesis client's
//! `src/annotator/anchoring/test/pdf-test.js` (commit
//! b4d085a2f893aa6de3b61d8b8bc3ae4d0f24fc1a, BSD-2-Clause): the `describe`
//! and `anchor` cases that concern text, with the fake PDF.js viewer
//! replaced by the page texts it serves. Upstream's fake viewer joins a
//! page's lines with nothing between them ("the new lines in fixtures don't
//! appear in the extracted PDF text"), so the fixtures drop their `\n`.
//!
//! Not ported, as they test the viewer and not the anchoring: rendering
//! state, placeholders, text-layer/text-API mismatches, the session cache,
//! `canDescribe`, shapes, and the `matchQuote` spy (its substance, that
//! quote, prefix, suffix and hint are whitespace-stripped, is checked by
//! `ignores_spaces_when_searching`). The `offset: -500` variant of "anchors
//! using a quote if the position selector fails" cannot be written: a
//! W3C/our `TextPositionSelector` offset is unsigned.

use crate::anchoring::{
    anchor_in_pages, describe_in_pages, AnchorStrategy, Anchoring, OrphanReason, PageSelector,
    Selector, TextPositionSelector, TextQuoteSelector,
};

fn pages() -> Vec<String> {
    [
        "Pride And Prejudice And Zombies\n       \nBy Jane Austen and Seth Grahame-Smith ",
        "IT IS A TRUTH universally acknowledged that a zombie in possession of\n\
         brains must be in want of more brains. Never was this truth more plain\n\
         than during the recent attacks at Netherfield Park, in which a household\n\
         of eighteen was slaughtered and consumed by a horde of the living dead.",
        "\"My dear Mr. Bennet,\" said his lady to him one day, \"have you heard that\n\
         Netherfield Park is occupied again?\" ",
        "NODE A\nNODE B\nNODE C",
    ]
    .iter()
    .map(|p| p.replace('\n', ""))
    .collect()
}

/// Describe the first occurrence of `text` on `page`.
fn describe_text(pages: &[String], page: usize, text: &str) -> Vec<Selector> {
    let p = &pages[page];
    let start = p[..p.find(text).unwrap()].chars().count();
    describe_in_pages(pages, page, start, start + text.chars().count(), None).unwrap()
}

fn anchored_text(pages: &[String], a: &Anchoring) -> (usize, String) {
    let a = a.anchor().unwrap_or_else(|| panic!("orphaned: {a:?}"));
    let page = a.page.unwrap();
    (
        page,
        pages[page]
            .chars()
            .skip(a.start)
            .take(a.end - a.start)
            .collect(),
    )
}

fn quote(exact: &str, prefix: Option<&str>, suffix: Option<&str>) -> Selector {
    Selector::TextQuoteSelector(TextQuoteSelector {
        exact: exact.into(),
        prefix: prefix.map(Into::into),
        suffix: suffix.map(Into::into),
    })
}

#[test]
fn describe_returns_position_quote_and_page_selectors() {
    let p = pages();
    let s = describe_text(&p, 2, "Netherfield Park");
    let mut types: Vec<&str> = s
        .iter()
        .map(|s| match s {
            Selector::PageSelector(_) => "PageSelector",
            Selector::TextPositionSelector(_) => "TextPositionSelector",
            Selector::TextQuoteSelector(_) => "TextQuoteSelector",
            Selector::Unsupported => "?",
        })
        .collect();
    types.sort();
    assert_eq!(
        types,
        ["PageSelector", "TextPositionSelector", "TextQuoteSelector"]
    );
}

#[test]
fn describe_position_offsets_are_document_wide() {
    let p = pages();
    let quote = "Netherfield Park";
    let s = describe_text(&p, 2, quote);
    let content = p.join("");
    let expected = content.rfind(quote).unwrap();
    assert_eq!(
        Selector::position(&s),
        Some(&TextPositionSelector {
            start: expected,
            end: expected + quote.len()
        })
    );
}

#[test]
fn describe_quote_has_32_chars_of_page_context() {
    let p = pages();
    let s = describe_text(&p, 2, "Netherfield Park");
    assert_eq!(
        Selector::quote(&s),
        Some(&TextQuoteSelector {
            exact: "Netherfield Park".into(),
            prefix: Some("im one day, \"have you heard that".into()),
            suffix: Some(" is occupied again?\" ".into()),
        })
    );
}

#[test]
fn describe_page_selector_labels() {
    let p = pages();
    let s = describe_text(&p, 2, "Netherfield Park");
    assert_eq!(
        Selector::page(&s),
        Some(&PageSelector {
            index: 2,
            label: Some("3".into())
        })
    );
    let start = p[2].find("Netherfield Park").unwrap();
    let s = describe_in_pages(&p, 2, start, start + 16, Some("iv".into())).unwrap();
    assert_eq!(
        Selector::page(&s),
        Some(&PageSelector {
            index: 2,
            label: Some("iv".into())
        })
    );
}

#[test]
fn anchors_previously_created_selectors() {
    let p = pages();
    let s = describe_text(&p, 2, "My dear Mr. Bennet");
    let position = *Selector::position(&s).unwrap();
    let q = Selector::quote(&s).unwrap().clone();
    let both = [
        Selector::TextPositionSelector(position),
        Selector::TextQuoteSelector(q.clone()),
    ];
    let a = anchor_in_pages(&p, &both);
    assert_eq!(anchored_text(&p, &a), (2, "My dear Mr. Bennet".into()));
    assert_eq!(a.anchor().unwrap().strategy, AnchorStrategy::Position);
    let a = anchor_in_pages(&p, &[Selector::TextQuoteSelector(q)]);
    assert_eq!(anchored_text(&p, &a), (2, "My dear Mr. Bennet".into()));
}

#[test]
fn fails_without_a_quote_selector() {
    let p = pages();
    assert_eq!(
        anchor_in_pages(&p, &[]),
        Anchoring::Orphaned(OrphanReason::NoUsableSelector)
    );
    let pos = [Selector::TextPositionSelector(TextPositionSelector {
        start: 0,
        end: 200,
    })];
    assert_eq!(
        anchor_in_pages(&p, &pos),
        Anchoring::Orphaned(OrphanReason::NoUsableSelector)
    );
}

#[test]
fn anchors_quote_on_page_with_blank_items() {
    let p = pages();
    let a = anchor_in_pages(&p, &[quote("Jane Austen", None, None)]);
    assert_eq!(anchored_text(&p, &a).1, "Jane Austen");
}

/// Upstream issue hypothesis/client#3705: an exact match on page 3 beats a
/// close match on page 2, whitespace differences ignored.
#[test]
fn anchors_quotes_to_best_match_across_all_pages() {
    let p = pages();
    for q in [
        "Netherfield Park is",
        "Netherfield  Park  is",
        "NetherfieldParkis",
        "Netherfield Park as",
    ] {
        let a = anchor_in_pages(&p, &[quote(q, None, None)]);
        assert_eq!(
            anchored_text(&p, &a),
            (2, "Netherfield Park is".into()),
            "quote {q:?}"
        );
    }
}

#[test]
fn prefers_a_context_match() {
    let p = pages();
    let cases: [(&str, Option<&str>, Option<&str>, &str); 5] = [
        (
            "prefix-only",
            Some("that"),
            None,
            "Netherfield Park is occupied again?",
        ),
        (
            "suffix-only",
            None,
            Some(" Park is occupied"),
            "Netherfield Park is occupied again?",
        ),
        (
            "suffix-match",
            Some("DOES NOT MATCH"),
            Some(" Park is occupied"),
            "Netherfield Park is occupied again?",
        ),
        (
            "prefix-match",
            Some("that"),
            Some("DOES NOT MATCH"),
            "Netherfield Park is occupied again?",
        ),
        (
            "no-context",
            None,
            None,
            "recent attacks at Netherfield Park",
        ),
    ];
    for (name, prefix, suffix, expected) in cases {
        let expected_page = p.iter().position(|pg| pg.contains(expected)).unwrap();
        let a = anchor_in_pages(&p, &[quote("Netherfield", prefix, suffix)]);
        assert_eq!(
            anchored_text(&p, &a),
            (expected_page, "Netherfield".into()),
            "{name}"
        );
    }
}

/// The substance of upstream's `matchQuote` spy test: with a deliberately
/// wrong `end` the position fails, and the whitespace-stripped quote search
/// finds "Mr. Bennet" on page 2, exactly.
#[test]
fn ignores_spaces_when_searching() {
    let p = pages();
    let q = TextQuoteSelector {
        exact: "Mr. Bennet".into(),
        prefix: Some("My dear".into()),
        suffix: Some(",\" said his lady".into()),
    };
    let off = p[0].len() + p[1].len() + p[2].find(&q.exact).unwrap();
    let sel = [
        Selector::TextPositionSelector(TextPositionSelector {
            start: off,
            end: off + 1,
        }),
        Selector::TextQuoteSelector(q),
    ];
    let a = anchor_in_pages(&p, &sel);
    assert_eq!(anchored_text(&p, &a), (2, "Mr. Bennet".into()));
    assert_eq!(a.anchor().unwrap().strategy, AnchorStrategy::ExactQuote);
}

/// Upstream issue hypothesis/client#1329.
#[test]
fn anchors_the_last_text_on_a_page() {
    let p = pages();
    let a = anchor_in_pages(&p, &[quote("horde of the living dead.", None, None)]);
    assert_eq!(
        anchored_text(&p, &a),
        (1, "horde of the living dead.".into())
    );
}

#[test]
fn anchors_using_quote_when_position_fails() {
    let p = pages();
    let selection = "zombie in possession";
    let s = describe_text(&p, 1, selection);
    let q = Selector::quote(&s).unwrap().clone();
    for offset in [5, p[0].len() + 1, p[0].len() + p[1].len() + 5, 100_000] {
        let sel = [
            Selector::TextPositionSelector(TextPositionSelector {
                start: offset,
                end: offset + selection.len(),
            }),
            Selector::TextQuoteSelector(q.clone()),
        ];
        let a = anchor_in_pages(&p, &sel);
        assert_eq!(
            anchored_text(&p, &a),
            (1, selection.into()),
            "offset {offset}"
        );
    }
}

/// Upstream's "rejects if quote cannot be anchored" asserts inside a
/// `.catch` only, so it passes whether or not the promise rejects. Here the
/// outcome is asserted. Prediction (before running): the stripped quote is
/// 30 characters, so up to 15 edits are allowed, and the four fixture
/// pages have no window within 15 edits of "phrasethatdoesnotexistinthePDF"
/// -> orphaned. Result (2026-10-07, first run): orphaned (`NotFound`), as
/// predicted.
#[test]
fn quote_not_in_document_is_orphaned() {
    let p = pages();
    let a = anchor_in_pages(
        &p,
        &[quote("phrase that does not exist in the PDF", None, None)],
    );
    assert_eq!(a, Anchoring::Orphaned(OrphanReason::NotFound));
}
