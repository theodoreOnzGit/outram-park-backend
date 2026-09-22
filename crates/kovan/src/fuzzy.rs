//! Fuzzy matching for Kovan's finders (the PDF reader's literature finder).
//!
//! Ported from `fuzzy_score` in
//! `crates/njoy-outram-park-fork/src/bin/njoy-tui/nuclides.rs` (the njoy TUI's
//! nuclide finder), which lives in another crate's binary and so cannot be
//! depended on. Same algorithm, same constants; keep the two in step. Hand
//! rolled, as there, rather than a new fuzzy-matching dependency.

/// Score how well `query` matches `candidate`, or `None` if it does not.
/// Both are lowercased here, so the match is case-insensitive.
///
/// - **Substring** matches score highest (1000+), with a bonus for a prefix
///   match and for a tighter candidate.
/// - **Subsequence** matches (every query character in order, fzf-style)
///   score below any substring, penalised by how spread out they are.
pub fn fuzzy_score(query: &str, candidate: &str) -> Option<i32> {
    let query = query.trim().to_lowercase();
    let candidate = candidate.to_lowercase();
    if query.is_empty() {
        return Some(0);
    }
    const SUBSTRING_BASE: i32 = 1_000;
    const SUBSEQUENCE_BASE: i32 = 0;

    if let Some(pos) = candidate.find(&query) {
        let prefix_bonus = if pos == 0 { 50 } else { 0 };
        let tightness = 100 - (candidate.len() as i32 - query.len() as i32).min(100);
        return Some(SUBSTRING_BASE + prefix_bonus + tightness);
    }

    let cand: Vec<char> = candidate.chars().collect();
    let mut ci = 0usize;
    let mut first_match: Option<usize> = None;
    let mut last_match = 0usize;
    for qc in query.chars() {
        let mut found = false;
        while ci < cand.len() {
            if cand[ci] == qc {
                first_match.get_or_insert(ci);
                last_match = ci;
                ci += 1;
                found = true;
                break;
            }
            ci += 1;
        }
        if !found {
            return None;
        }
    }
    let span = last_match - first_match.unwrap_or(0) + 1;
    let spread_penalty = span as i32 - query.chars().count() as i32;
    Some(SUBSEQUENCE_BASE - spread_penalty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substring_beats_subsequence_and_order_matters() {
        let sub = fuzzy_score("pangu", "cc-by/she2021pangu.pdf").unwrap();
        let seq = fuzzy_score("shpg", "cc-by/she2021pangu.pdf").unwrap();
        assert!(sub > seq);
        assert_eq!(fuzzy_score("ugnap", "she2021pangu"), None);
        assert_eq!(fuzzy_score("PANGU", "She2021Pangu"), fuzzy_score("pangu", "she2021pangu"));
    }

    #[test]
    fn a_tighter_cluster_ranks_higher() {
        let tight = fuzzy_score("htr", "htr10-otto.pdf").unwrap();
        let loose = fuzzy_score("htr", "h-t-r.pdf").unwrap();
        assert!(tight > loose);
    }
}
