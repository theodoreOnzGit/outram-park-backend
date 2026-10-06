//! Port of the Hypothesis client's
//! `src/annotator/anchoring/test/match-quote-test.js` (commit
//! b4d085a2f893aa6de3b61d8b8bc3ae4d0f24fc1a, BSD-2-Clause). Same fixtures,
//! inputs and assertions.

use crate::anchoring::{match_quote, QuoteContext};

/// Upstream's `normalize`: collapse every whitespace run to one space.
fn normalize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn solitude() -> String {
    normalize(
        "Many years later, as he faced the firing squad,
    Colonel Aureliano Buendía was to remember that distant afternoon
    when his father took him to discover ice",
    )
}

fn two_cities() -> String {
    normalize(
        "It was the best of times, it was the worst of times,
    it was the age of wisdom, it was the age of foolishness, it was the epoch of belief,
    it was the epoch of incredulity, it was the season of Light, it was the
    season of Darkness, it was the spring of hope, it was the winter of despair, we had
    everything before us, we had nothing before us, we were all going direct to Heaven,
    we were all going direct the other way.",
    )
}

fn slice(text: &str, a: usize, b: usize) -> String {
    text.chars().skip(a).take(b - a).collect()
}

/// `char` index of the first occurrence (the fixtures are ASCII except
/// "Buendía", so convert from bytes explicitly).
fn index_of(text: &str, needle: &str) -> usize {
    text[..text.find(needle).unwrap()].chars().count()
}

fn ctx(prefix: Option<&str>, suffix: Option<&str>, hint: Option<usize>) -> QuoteContext {
    QuoteContext {
        prefix: prefix.map(str::to_string),
        suffix: suffix.map(str::to_string),
        hint,
    }
}

#[test]
fn finds_exact_match() {
    let t = solitude();
    let m = match_quote(&t, "discover ice", &QuoteContext::default()).unwrap();
    assert_eq!(m.score, 1.0);
    assert_eq!(slice(&t, m.start, m.end), "discover ice");
}

#[test]
fn finds_best_approximate_match_if_no_exact_match() {
    let t = solitude();
    let m = match_quote(&t, "some years later", &QuoteContext::default()).unwrap();
    assert!(m.score > 0.0 && m.score < 1.0);
    assert_eq!(slice(&t, m.start, m.end), "Many years later");
}

fn assert_descending(scores: &[f64]) {
    for i in 1..scores.len() {
        assert!(scores[i] < scores[i - 1], "{scores:?}");
    }
}

#[test]
fn scores_matches_based_on_quote_similarity() {
    let t = solitude();
    let quotes = [
        "Many years later",
        "Many yers later",
        "Some years later",
        "Some years after",
    ];
    let scores: Vec<f64> = quotes
        .iter()
        .map(|q| match_quote(&t, q, &QuoteContext::default()).unwrap().score)
        .collect();
    assert_descending(&scores);
}

#[test]
fn scores_matches_based_on_prefix_similarity() {
    let t = solitude();
    let prefixes = [
        "Many years later",
        "Many yers later",
        "Some years later",
        "Some years after",
    ];
    let quote = ", as he faced the firing squad";
    let scores: Vec<f64> = prefixes
        .iter()
        .map(|p| {
            match_quote(&t, quote, &ctx(Some(p), None, None))
                .unwrap()
                .score
        })
        .collect();
    assert_descending(&scores);
}

#[test]
fn scores_matches_based_on_suffix_similarity() {
    let t = solitude();
    let suffixes = [
        ", as he faced the firing squad",
        ", as she faced the firing squad",
        ", as he awaited the firing squad",
        ", as he awaited his death",
    ];
    let quote = "Many years later";
    let scores: Vec<f64> = suffixes
        .iter()
        .map(|s| {
            match_quote(&t, quote, &ctx(None, Some(s), None))
                .unwrap()
                .score
        })
        .collect();
    assert_descending(&scores);
}

#[test]
fn returns_none_if_no_acceptable_approximate_match() {
    assert_eq!(
        match_quote(&two_cities(), &solitude(), &QuoteContext::default()),
        None
    );
}

#[test]
fn returns_none_if_quote_or_text_empty() {
    assert_eq!(match_quote("foobar", "", &QuoteContext::default()), None);
    assert_eq!(match_quote("", "foobar", &QuoteContext::default()), None);
}

#[test]
fn finds_match_with_best_context_match() {
    let cases: [(&str, Option<&str>, Option<&str>, &str); 8] = [
        // Exact prefix matches.
        (
            "before us",
            Some("we had everything"),
            None,
            "before us, we had nothing",
        ),
        (
            "before us",
            Some("we had nothing"),
            None,
            "before us, we were all going",
        ),
        // Approximate prefix matches.
        (
            "before us",
            Some("we had every-thing"),
            None,
            "before us, we had nothing",
        ),
        (
            "before us",
            Some("we had nout"),
            None,
            "before us, we were all going",
        ),
        // Exact suffix matches.
        ("we had", None, Some("everything"), "we had everything"),
        ("we had", None, Some("nothing"), "we had nothing"),
        // Approximate suffix matches.
        ("we had", None, Some("ever ting"), "we had everything"),
        ("we had", None, Some("nutting"), "we had nothing"),
    ];
    let text = two_cities();
    for (i, (quote, prefix, suffix, expected)) in cases.into_iter().enumerate() {
        let m = match_quote(&text, quote, &ctx(prefix, suffix, None)).unwrap();
        assert_eq!(slice(&text, m.start, m.end), quote, "case {i}");
        assert_eq!(m.start, index_of(&text, expected), "case {i}");
    }
}

#[test]
fn uses_hint_as_tie_breaker() {
    let text = two_cities();
    let pos_a = index_of(&text, "everything before us") + "everything ".len();
    let pos_b = index_of(&text, "nothing before us") + "nothing ".len();
    let a = match_quote(&text, "befor us", &ctx(None, None, Some(pos_a))).unwrap();
    let b = match_quote(&text, "befor us", &ctx(None, None, Some(pos_b))).unwrap();
    let none = match_quote(&text, "befor us", &QuoteContext::default()).unwrap();
    assert_eq!(a.start, pos_a, "Wrong match for hint `posA`");
    assert_eq!(b.start, pos_b, "Wrong match for hint `posB`");
    assert_eq!(none.start, pos_a, "Wrong match with no hint");
}

#[test]
fn matches_when_prefix_longer_than_match_start() {
    let t = solitude();
    let prefix = "It used to be many";
    let m = match_quote(&t, "years later", &ctx(Some(prefix), None, None)).unwrap();
    assert!(prefix.len() > m.start);
    assert_eq!(slice(&t, m.start, m.end), "years later");
}

#[test]
fn matches_at_end_of_text_with_nonempty_suffix() {
    let text = "Some document text";
    let m = match_quote(text, "text", &ctx(None, Some("missing"), None)).unwrap();
    assert_eq!(slice(text, m.start, m.end), "text");
}
