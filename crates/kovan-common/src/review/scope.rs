//! Reviewer **scope** globs (maintainer, #739 "many maintainers",
//! 2026-10-07): a stamp outside its reviewer's scope shows "outside scope"
//! and does not count.
//!
//! The glob language is deliberately small, over `/`-separated
//! workspace-relative paths: `*` matches within one path segment, `**`
//! matches any number of whole segments (including none), `?` one character
//! other than `/`. Everything else is literal. No external glob crate: the
//! rule has to run in web-kovan (wasm) and is a few lines.

/// Whether `path` matches `glob`.
pub fn glob_match(glob: &str, path: &str) -> bool {
    let g: Vec<&str> = glob.split('/').filter(|s| !s.is_empty()).collect();
    let p: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match_segments(&g, &p)
}

fn match_segments(g: &[&str], p: &[&str]) -> bool {
    match g.split_first() {
        None => p.is_empty(),
        Some((&"**", rest)) => (0..=p.len()).any(|i| match_segments(rest, &p[i..])),
        Some((first, rest)) => match p.split_first() {
            Some((seg, prest)) => match_one(first, seg) && match_segments(rest, prest),
            None => false,
        },
    }
}

fn match_one(glob: &str, seg: &str) -> bool {
    let g: Vec<char> = glob.chars().collect();
    let s: Vec<char> = seg.chars().collect();
    fn go(g: &[char], s: &[char]) -> bool {
        match g.split_first() {
            None => s.is_empty(),
            Some(('*', rest)) => (0..=s.len()).any(|i| go(rest, &s[i..])),
            Some(('?', rest)) => !s.is_empty() && go(rest, &s[1..]),
            Some((c, rest)) => s.first() == Some(c) && go(rest, &s[1..]),
        }
    }
    go(&g, &s)
}

/// Whether `path` is inside any of `globs`.
pub fn in_scope(globs: &[String], path: &str) -> bool {
    globs.iter().any(|g| glob_match(g, path))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: `*`, `**` and `?` against workspace paths.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn globs() {
        assert!(glob_match("crates/tampines/**", "crates/tampines/src/steam.rs"));
        assert!(glob_match("crates/**/steam.rs", "crates/tampines/src/steam.rs"));
        assert!(glob_match("crates/*/src/*.rs", "crates/a/src/lib.rs"));
        assert!(!glob_match("crates/*/src/*.rs", "crates/a/src/x/lib.rs"));
        assert!(glob_match("crates/tampines/**", "crates/tampines"));
        assert!(!glob_match("crates/tampines/**", "crates/tampines-steam-tables/src/a.rs"));
        assert!(glob_match("crates/a/src/lib.r?", "crates/a/src/lib.rs"));
        assert!(!in_scope(&[], "crates/a/src/lib.rs"));
    }
}
