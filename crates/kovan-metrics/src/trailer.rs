//! The `API-Usage-*` commit trailers — the workspace's durable token record.
//!
//! Every commit carries its own accounting in its message:
//!
//! ```text
//! API-Usage-Since-Last-Commit: total=1234 in=10 out=20 cache_read=1200 cache_write=4 source=session-transcript
//! API-Usage-Session-Cumulative: total=98765
//! ```
//!
//! The **trailer is the source of truth**, not the generated
//! `docs/token-usage.md` ledger (which is gitignored and regenerable). Anything
//! that reports usage over a window reads these lines back out of git.
//!
//! **Honesty rule.** `total = in + out + cache_read + cache_write`. A commit made
//! outside a Claude session legitimately reads `total=0 source=none`; that is a
//! correct measurement of zero, not missing data, and must never be replaced
//! with an estimate.

/// Trailer key for the per-commit delta.
pub const TRAILER_KEY: &str = "API-Usage-Since-Last-Commit";
/// Trailer key for the running session total.
pub const CUMULATIVE_KEY: &str = "API-Usage-Session-Cumulative";

/// The four token components the API bills separately.
///
/// Kept as a struct rather than a map so a missing component is a compile
/// error rather than a silent zero. Cache-read normally dominates by an order
/// of magnitude and is always reported separately — never fold it into a
/// single headline number.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TokenCounts {
    /// Uncached input tokens.
    pub input: u64,
    /// Generated output tokens — the closest proxy for net produced content.
    pub output: u64,
    /// Tokens read back from the prompt cache.
    pub cache_read: u64,
    /// Tokens written into the prompt cache.
    pub cache_write: u64,
}

impl TokenCounts {
    /// `input + output + cache_read + cache_write`.
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_read + self.cache_write
    }

    /// Component-wise sum.
    pub fn add(&mut self, other: &TokenCounts) {
        self.input += other.input;
        self.output += other.output;
        self.cache_read += other.cache_read;
        self.cache_write += other.cache_write;
    }

    /// Component-wise `self - baseline`, clamped at zero.
    ///
    /// The clamp matters: transcripts can be rotated or pruned between commits,
    /// which would otherwise yield a negative delta and a nonsense trailer.
    pub fn saturating_sub(&self, baseline: &TokenCounts) -> TokenCounts {
        TokenCounts {
            input: self.input.saturating_sub(baseline.input),
            output: self.output.saturating_sub(baseline.output),
            cache_read: self.cache_read.saturating_sub(baseline.cache_read),
            cache_write: self.cache_write.saturating_sub(baseline.cache_write),
        }
    }
}

/// A parsed `API-Usage-Since-Last-Commit` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTrailer {
    /// The four components as recorded.
    pub counts: TokenCounts,
    /// The `total=` field as recorded, which is trusted over the recomputed sum
    /// so a report reproduces exactly what the commit claimed.
    pub total: u64,
    /// The `source=` field, e.g. `session-transcript` or `none`.
    pub source: String,
}

impl ParsedTrailer {
    /// Whether this trailer carries real measured usage.
    ///
    /// `source=none` means the commit was made outside a Claude session — a
    /// true zero that should be rendered as "no token data" rather than `0`.
    pub fn has_data(&self) -> bool {
        self.source != "none"
    }
}

/// Extract the `API-Usage-Since-Last-Commit` trailer from a commit message
/// body, or `None` when the commit carries no trailer.
///
/// Scans line by line for the key (the Python used a `MULTILINE` regex; this is
/// the same match without the dependency). Unparseable numeric fields read as
/// `0` rather than failing the whole record — a mangled trailer should degrade
/// one row, not abort a report.
pub fn parse(body: &str) -> Option<ParsedTrailer> {
    let prefix = format!("{TRAILER_KEY}:");
    let rest = body
        .lines()
        .find_map(|line| line.trim_start().strip_prefix(&prefix))?;

    let mut counts = TokenCounts::default();
    let mut total: Option<u64> = None;
    let mut source = "none".to_string();

    for token in rest.split_whitespace() {
        let Some((key, value)) = token.split_once('=') else {
            continue;
        };
        let n = || value.parse::<u64>().unwrap_or(0);
        match key {
            "in" => counts.input = n(),
            "out" => counts.output = n(),
            "cache_read" => counts.cache_read = n(),
            "cache_write" => counts.cache_write = n(),
            "total" => total = Some(n()),
            "source" => source = value.to_string(),
            _ => {}
        }
    }

    Some(ParsedTrailer {
        total: total.unwrap_or_else(|| counts.total()),
        counts,
        source,
    })
}

/// Render the two trailer lines appended to a commit message.
pub fn format(delta: &TokenCounts, cumulative: &TokenCounts, source: &str) -> String {
    format!(
        "{TRAILER_KEY}: total={} in={} out={} cache_read={} cache_write={} source={}\n\
         {CUMULATIVE_KEY}: total={}",
        delta.total(),
        delta.input,
        delta.output,
        delta.cache_read,
        delta.cache_write,
        source,
        cumulative.total(),
    )
}

/// Format an integer with thousands separators, e.g. `1234567` -> `1,234,567`.
pub fn group(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_formatted_trailer() {
        let delta = TokenCounts {
            input: 10,
            output: 20,
            cache_read: 1200,
            cache_write: 4,
        };
        let cum = TokenCounts {
            input: 100,
            output: 200,
            cache_read: 98_000,
            cache_write: 465,
        };
        let text = format(&delta, &cum, "session-transcript");
        let parsed = parse(&text).expect("its own output must parse");
        assert_eq!(parsed.counts, delta);
        assert_eq!(parsed.total, delta.total());
        assert_eq!(parsed.total, 1234);
        assert_eq!(parsed.source, "session-transcript");
        assert!(parsed.has_data());
        assert!(text.contains(&format!("{CUMULATIVE_KEY}: total={}", cum.total())));
    }

    #[test]
    fn a_commit_with_no_trailer_yields_none() {
        assert!(parse("just a subject\n\nand a body\n").is_none());
    }

    #[test]
    fn source_none_is_a_true_zero_not_missing_data() {
        let body = format!("{TRAILER_KEY}: total=0 in=0 out=0 cache_read=0 cache_write=0 source=none");
        let p = parse(&body).unwrap();
        assert_eq!(p.total, 0);
        assert!(!p.has_data(), "source=none must render as 'no data', not 0");
    }

    #[test]
    fn finds_the_trailer_among_surrounding_prose() {
        let body = format!(
            "some commit prose\n\nmore prose\n\n{TRAILER_KEY}: total=7 in=1 out=2 cache_read=3 cache_write=1 source=session-transcript\n{CUMULATIVE_KEY}: total=99\n"
        );
        let p = parse(&body).unwrap();
        assert_eq!(p.total, 7);
        assert_eq!(p.counts.cache_read, 3);
    }

    #[test]
    fn a_mangled_field_degrades_to_zero_rather_than_failing() {
        let body = format!("{TRAILER_KEY}: total=oops in=5 out=2 cache_read=1 cache_write=0 source=session-transcript");
        let p = parse(&body).unwrap();
        assert_eq!(p.total, 0);
        assert_eq!(p.counts.input, 5);
    }

    #[test]
    fn delta_clamps_at_zero_when_transcripts_rotate() {
        let now = TokenCounts { input: 5, output: 0, cache_read: 0, cache_write: 0 };
        let baseline = TokenCounts { input: 100, output: 0, cache_read: 0, cache_write: 0 };
        assert_eq!(now.saturating_sub(&baseline).input, 0);
    }

    #[test]
    fn total_is_the_sum_of_all_four_components() {
        let c = TokenCounts { input: 1, output: 2, cache_read: 4, cache_write: 8 };
        assert_eq!(c.total(), 15);
    }

    #[test]
    fn groups_thousands() {
        assert_eq!(group(0), "0");
        assert_eq!(group(999), "999");
        assert_eq!(group(1_000), "1,000");
        assert_eq!(group(1_234_567), "1,234,567");
    }
}
