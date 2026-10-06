//! Port of approx-string-match's own tests, `test/index-test.ts`
//! (https://github.com/robertknight/approx-string-match-js, commit
//! fe814eba4d6b6daf88d38179331a14d156a0b5a0, MIT, (c) 2020 Robert Knight).
//! Same inputs and expected outputs, except the `unicode` fixtures, whose
//! expectations change because this port counts `char`s, not UTF-16 units
//! (see `approx_match` module doc).

use crate::anchoring::approx_match::{search_str, Match};

fn repeat(s: &str, n: usize) -> String {
    s.repeat(n)
}

fn m(start: usize, end: usize, errors: usize) -> Match {
    Match { start, end, errors }
}

/// Upstream's `check`: search with `max_errors = pattern.len()`, keep the
/// matches with the fewest errors, and compare. If the search returns one
/// match where several are expected, it must be one of them.
fn check(text: &str, pattern: &str, expected: &[Match]) {
    let max_errors = pattern.chars().count();
    let mut actual = search_str(text, pattern, max_errors);
    let min = actual.iter().map(|m| m.errors).min().unwrap();
    actual.retain(|m| m.errors == min);
    if actual.len() == 1 && expected.len() > 1 {
        assert!(
            expected.contains(&actual[0]),
            "{actual:?} not in {expected:?}"
        );
    } else {
        assert_eq!(actual, expected, "text {text:?} pattern {pattern:?}");
    }
}

#[test]
fn exact_match() {
    check("three blind mice", "blind", &[m(6, 11, 0)]);
    check("three blind mice", "three", &[m(0, 5, 0)]);
}

#[test]
fn one_error() {
    check("three blind mice", "bliind", &[m(6, 11, 1)]);
    check("three blind mice", "blnd", &[m(6, 11, 1)]);
    check("three blind mice", "thrae", &[m(0, 4, 1), m(0, 5, 1)]);
    check("facebook", "fccebook", &[m(0, 8, 1)]);
    check("facebook", "fcebook", &[m(1, 8, 1), m(0, 8, 1)]);
}

#[test]
fn many_errors() {
    check(
        "foursquare andseven",
        "four square and seven",
        &[m(0, 19, 2)],
    );
    check("four squareand seven", "square  and", &[m(5, 14, 2)]);
}

/// Upstream expects `[0, 6)` with 2 errors for "smile" in "sm😊le", since
/// the emoji is two UTF-16 units. In `char`s it is one substitution in a
/// five-character text: `[0, 5)`, 1 error. The second fixture, an exact
/// match, is `[0, 5)` instead of `[0, 6)` for the same reason.
#[test]
fn unicode_counts_chars_not_utf16_units() {
    check("sm😊le", "smile", &[m(0, 5, 1)]);
    check("sm😊le", "sm😊le", &[m(0, 5, 0)]);
}

#[test]
fn long_pattern() {
    let text = repeat("foo", 5) + &repeat("bar", 20) + &repeat("baz", 5);
    check(&text, &repeat("bar", 20), &[m(15, 75, 0)]);
    let text =
        repeat("foo", 5) + &repeat("bar", 10) + "zog" + &repeat("bar", 10) + &repeat("baz", 5);
    check(&text, &repeat("bar", 20), &[m(15, 75, 3), m(15, 78, 3)]);
}

#[test]
fn no_match_returns_empty() {
    assert_eq!(search_str("four candles", "foouur", 1), vec![]);
}

#[test]
fn long_text_short_pattern() {
    let text = "\n  A great discovery solves a great problem but there is a grain of discovery in\n  the solution of any problem.\n  ";
    let matches = search_str(text, "discvery", 2);
    let chars: Vec<char> = text.chars().collect();
    assert!(!matches.is_empty());
    for mt in matches {
        assert_eq!(
            chars[mt.start..mt.end].iter().collect::<String>(),
            "discovery"
        );
    }
}

#[test]
fn max_errors_exceeding_pattern_length_by_word_size() {
    assert_eq!(search_str("four score", "score", 50), vec![m(5, 10, 0)]);
}

#[test]
fn returns_all_matches() {
    assert_eq!(search_str(&repeat("foo bar ", 5), "foo", 0).len(), 5);
}

#[test]
fn empty_text_and_empty_pattern() {
    assert_eq!(search_str("", "foo", 0), vec![]);
    assert_eq!(search_str("foo", "", 0), vec![]);
}

const WORD_SIZE_TEXT: &str = "This is a string which exceeds the \"word size\" of the JS language.";

#[test]
fn pattern_length_equals_block_size() {
    let pat = &WORD_SIZE_TEXT[1..33];
    assert_eq!(search_str(WORD_SIZE_TEXT, pat, 0), vec![m(1, 33, 0)]);
}

#[test]
fn pattern_length_exceeds_block_size() {
    let pat = &WORD_SIZE_TEXT[23..56];
    assert_eq!(search_str(WORD_SIZE_TEXT, pat, 0), vec![m(23, 56, 0)]);
    let text = repeat(WORD_SIZE_TEXT, 5);
    assert_eq!(search_str(&text, WORD_SIZE_TEXT, 0).len(), 5);
}

#[test]
fn pattern_length_multiple_of_block_size() {
    let text = "for Native Americans as well.\nThe last global ice age trapped much";
    let pattern = " well.\n \n2. The First Americans\nThe last global ice age trapped ";
    assert_eq!(pattern.chars().count(), 64);
    let matches = search_str(text, pattern, 32);
    assert_eq!(matches, vec![m(23, 62, 25)]);
    let chars: Vec<char> = text.chars().collect();
    assert_eq!(
        chars[23..62].iter().collect::<String>(),
        " well.\nThe last global ice age trapped "
    );
}
