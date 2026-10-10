//! **Maturity in plain English** (GitHub #740 U1/U2, maintainer
//! 2026-10-07): "Maturity is shown in plain English, explained in fewer
//! than 3 sentences. It is never written as notation such as `3 ↓2`."
//!
//! The wording follows the U2 decision for a crate card whose human
//! reviews have lapsed:
//!
//! > "Maturity 2: AI V&V (was 3: human reviewed)". The bottom bar adds:
//! > "N reviewed functions have changed since you reviewed them. Re-review
//! > them to bring this crate back to maturity 3."
//!
//! **Which level a lapsed crate falls to.** A crate tagged 3 or 4 rests on
//! human reviews; while any of them is stale it is shown at the level
//! below human review, **2 (AI V&V)**, as U2's example does. That is the
//! highest level the tag can still claim without a person; whether the AI
//! V&V itself holds is not judged here (CI's in-memory recomputation,
//! #739, is the authority for the true level). Pure, no I/O: desktop
//! kovan's ⚑ queue and, later, web-kovan's bar can share it.

use super::maturity_label;

/// One sentence saying what a maturity level means (#729's labels).
pub fn level_meaning(level: u8) -> &'static str {
    match level {
        0 => "Nothing here is implemented or checked yet.",
        1 => "Written with AI help; no test or reference case checks it yet.",
        2 => "AI-written tests or comparisons check it, but no person has reviewed the code.",
        3 => "A person has read the code and stamped it.",
        4 => "A person has reviewed the code and checked it against a reference case by hand.",
        _ => "This level is not defined.",
    }
}

/// `"Maturity 3: human reviewed"`.
pub fn maturity_title(level: u8) -> String {
    format!("Maturity {level}: {}", maturity_label(level))
}

/// The plain-English maturity of a crate tagged `recorded` with `stale`
/// reviewed functions changed since their review: a title and at most two
/// sentences (module doc). Never notation.
pub fn plain_maturity(recorded: u8, stale: usize) -> String {
    if recorded >= 3 && stale > 0 {
        let now = 2;
        let functions = if stale == 1 {
            "1 reviewed function has changed since you reviewed it. Re-review it".to_string()
        } else {
            format!(
                "{stale} reviewed functions have changed since you reviewed them. Re-review them"
            )
        };
        return format!(
            "{} (was {recorded}: {}). {functions} to bring this crate back to maturity {recorded}.",
            maturity_title(now),
            maturity_label(recorded)
        );
    }
    format!("{}. {}", maturity_title(recorded), level_meaning(recorded))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sentences, counted at ". " and the final full stop.
    fn sentences(s: &str) -> usize {
        s.split(". ").count()
    }

    /// Methodology: every level, with and without stale reviews. Pass: the
    /// U2 wording exactly for a lapsed level-3 crate; fewer than 3 sentences
    /// after the title; never the arrow notation.
    #[test]
    fn maturity_is_plain_english_in_fewer_than_three_sentences() {
        assert_eq!(
            plain_maturity(3, 4),
            "Maturity 2: AI V&V (was 3: human reviewed). 4 reviewed functions have changed \
             since you reviewed them. Re-review them to bring this crate back to maturity 3."
        );
        assert!(plain_maturity(4, 1)
            .starts_with("Maturity 2: AI V&V (was 4: human V&V). 1 reviewed function has"));
        assert_eq!(
            plain_maturity(1, 3),
            "Maturity 1: AI draft. Written with AI help; no test or reference case checks it yet."
        );
        for level in 0..=5u8 {
            for stale in [0, 1, 7] {
                let s = plain_maturity(level, stale);
                assert!(!s.contains('\u{2193}') && !s.contains("M3"), "{s}");
                // The title, then at most two sentences.
                assert!(
                    sentences(s.split_once(". ").map_or("", |(_, r)| r)) <= 2,
                    "{s}"
                );
            }
        }
    }
}
