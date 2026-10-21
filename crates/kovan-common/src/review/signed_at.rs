//! **`signed_at`**: the full signing time of a stamp, and its plausibility
//! checks (GitHub #783; maintainer, 2026-10-07, follow-up to #762 and #739
//! D6).
//!
//! The signed `date` is day-level only. `signed_at` is an RFC 3339
//! timestamp **to the second, with its UTC offset**
//! (`2026-10-07T14:03:09+08:00`, or `Z` for UTC), written into the signed
//! bytes ([`super::signing`]; ~~the `kovan-review-signature-v2` header~~
//! the one `kovan-review-signature-v3` header since #825). Every signing
//! writes it. ~~A stamp signed before #783 has none (v1) and is judged
//! exactly as before.~~ **CORRECTED 2026-10-10** (#825): v1 stamps no longer
//! verify; an entry without `signed_at` (field kept optional) gets no
//! plausibility flag.
//!
//! # The plausibility flags
//!
//! Git timestamps are forgeable, so these are **tamper evidence, not
//! proof** (Leak Before Break, `docs/kovan.md`): each failure is a visible
//! flag on the function ([`super::engine::FunctionFlag::ImplausibleSignedAt`]),
//! never a rejection and never a void. The ed25519 key is what stops
//! forgery. A stamp is flagged when
//!
//! 1. `signed_at` is earlier than the committer time of the reviewed
//!    `commit` ([`SignedAtProblem::BeforeReviewedCommit`]): the review
//!    claims to predate the code it certifies;
//! 2. `signed_at` is later than the committer time of the commit that
//!    introduced the stamp ([`SignedAtProblem::AfterStampCommit`]): the
//!    stamp was committed before it was signed;
//! 3. the calendar date of `signed_at`, **in the offset it was written
//!    with**, is not `date` ([`SignedAtProblem::DateMismatch`]);
//! 4. `signed_at` is present but not RFC 3339 to the second
//!    ([`SignedAtProblem::Unparseable`]): nothing else can be judged.
//!
//! **Clock skew.** Every comparison allows [`SKEW_SECONDS`] (5 minutes)
//! either way: two machines' clocks, or a commit made seconds after signing
//! on a slightly slow clock, must not raise a flag. For the date check, a
//! `date` that is the date of either `signed_at - 5 min` or `signed_at +
//! 5 min` (in the written offset) passes.
//!
//! A check whose git time the caller did not supply (`None`) is skipped,
//! not flagged: no fact, no judgement.
//!
//! ~~**No local time zone.** `std` exposes no local zone and the workspace
//! carries no date crate (the reasoning in `kovan-metrics`' `date` module),
//! so [`now_utc`], which [`super::signing::keystore::UnlockedKey::sign_review`]
//! uses, writes UTC (`+00:00`).~~ **CORRECTED 2026-10-07** (maintainer:
//! add `chrono`): `sign_review` signs [`now_local`], the reviewer's local
//! time with its offset. The reviewer's `date` must be the date in that same
//! offset ([`date_of`] gives it), or flag 3 shows. `sign_review_at` still
//! takes an explicit timestamp, for tests.

use crate::zotero::date::{civil_from_days, days_from_civil};

/// The clock skew every check allows, either way (5 minutes).
pub const SKEW_SECONDS: i64 = 300;

/// A parsed `signed_at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rfc3339 {
    /// Seconds since 1970-01-01T00:00:00Z.
    pub unix: i64,
    /// The written UTC offset, in minutes (`+08:00` is 480; `Z` is 0).
    pub offset_minutes: i32,
}

impl Rfc3339 {
    /// The calendar date (`YYYY-MM-DD`) of `unix + shift` seconds, in the
    /// written offset.
    pub fn local_date(&self, shift: i64) -> String {
        let local = self.unix + shift + i64::from(self.offset_minutes) * 60;
        let (y, m, d) = civil_from_days(local.div_euclid(86_400));
        format!("{y:04}-{m:02}-{d:02}")
    }
}

fn digits(s: &str) -> Option<i64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        _ => 0,
    }
}

/// Parse `YYYY-MM-DDTHH:MM:SS` followed by `Z` or `±HH:MM` (RFC 3339 to
/// the second; no fraction, `T` and `Z` upper case as kovan writes them).
/// `None` for anything else, including an impossible date or time.
pub fn parse_rfc3339(s: &str) -> Option<Rfc3339> {
    let b = s.as_bytes();
    if b.len() < 20
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    let (y, mo, d) = (digits(&s[0..4])?, digits(&s[5..7])?, digits(&s[8..10])?);
    let (h, mi, se) = (
        digits(&s[11..13])?,
        digits(&s[14..16])?,
        digits(&s[17..19])?,
    );
    if !(1..=12).contains(&mo) || d < 1 || d > days_in_month(y, mo) || h > 23 || mi > 59 || se > 59
    {
        return None;
    }
    let offset_minutes = match &s[19..] {
        "Z" => 0,
        tz if tz.len() == 6 && tz.as_bytes()[3] == b':' => {
            let sign = match tz.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            let (oh, om) = (digits(&tz[1..3])?, digits(&tz[4..6])?);
            if oh > 23 || om > 59 {
                return None;
            }
            sign * (oh * 60 + om) as i32
        }
        _ => return None,
    };
    let local = days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + se;
    Some(Rfc3339 {
        unix: local - i64::from(offset_minutes) * 60,
        offset_minutes,
    })
}

/// `unix` as RFC 3339 in UTC, to the second: `YYYY-MM-DDTHH:MM:SS+00:00`.
pub fn format_utc(unix: i64) -> String {
    let (y, m, d) = civil_from_days(unix.div_euclid(86_400));
    let r = unix.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}+00:00",
        r / 3600,
        r % 3600 / 60,
        r % 60
    )
}

/// The clock now, as [`format_utc`]. Native only (`SystemTime::now` panics
/// on wasm32-unknown-unknown); a clock before 1970 reads as the epoch.
#[cfg(not(target_arch = "wasm32"))]
pub fn now_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    format_utc(secs)
}

/// The clock now in the machine's **local** time zone, RFC 3339 to the
/// second with its offset (`2026-10-08T07:30:00+08:00`). What
/// [`super::signing::keystore::UnlockedKey::sign_review`] signs (maintainer,
/// 2026-10-07: sign in local time). The zone comes from `chrono`'s `Local`
/// (iana-time-zone); where the zone cannot be read chrono uses UTC, which
/// the written `+00:00` makes visible. Native only, like [`now_utc`].
#[cfg(not(target_arch = "wasm32"))]
pub fn now_local() -> String {
    chrono::Local::now()
        .format("%Y-%m-%dT%H:%M:%S%:z")
        .to_string()
}

/// The local calendar date (`YYYY-MM-DD`) of a `signed_at`, in the offset it
/// was written with: the `date` a stamp signed at that moment should carry.
/// `None` when `signed_at` does not parse.
pub fn date_of(signed_at: &str) -> Option<String> {
    parse_rfc3339(signed_at).map(|t| t.local_date(0))
}

/// One implausibility of a stamp's `signed_at` (module doc). Times are
/// seconds since the epoch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignedAtProblem {
    /// Not RFC 3339 to the second.
    Unparseable { signed_at: String },
    /// Signed more than [`SKEW_SECONDS`] before the reviewed commit was
    /// committed.
    BeforeReviewedCommit { signed_at: String, commit_time: i64 },
    /// Signed more than [`SKEW_SECONDS`] after the commit that introduced
    /// the stamp was committed.
    AfterStampCommit { signed_at: String, commit_time: i64 },
    /// `signed_at`'s date, in its own offset, is not `date`.
    DateMismatch { signed_at: String, date: String },
}

impl SignedAtProblem {
    /// Plain-English text for a flag tooltip or a CLI line.
    pub fn describe(&self) -> String {
        match self {
            Self::Unparseable { signed_at } => format!("signed_at {signed_at:?} is not RFC 3339"),
            Self::BeforeReviewedCommit {
                signed_at,
                commit_time,
            } => format!(
                "signed at {signed_at}, before the reviewed commit ({})",
                format_utc(*commit_time)
            ),
            Self::AfterStampCommit {
                signed_at,
                commit_time,
            } => format!(
                "signed at {signed_at}, after the commit that added the stamp ({})",
                format_utc(*commit_time)
            ),
            Self::DateMismatch { signed_at, date } => {
                format!("signed at {signed_at}, but the stamp is dated {date}")
            }
        }
    }
}

/// Judge a stamp's `signed_at` (module doc). `reviewed_commit_time` is the
/// committer time of the stamp's `commit`; `stamp_commit_time` that of the
/// commit that introduced the stamp. A stamp without one (`signed_at =
/// None`) gets no flag.
pub fn plausibility(
    signed_at: Option<&str>,
    date: &str,
    reviewed_commit_time: Option<i64>,
    stamp_commit_time: Option<i64>,
) -> Vec<SignedAtProblem> {
    let Some(raw) = signed_at else {
        return Vec::new();
    };
    let Some(t) = parse_rfc3339(raw) else {
        return vec![SignedAtProblem::Unparseable {
            signed_at: raw.to_string(),
        }];
    };
    let mut out = Vec::new();
    if let Some(c) = reviewed_commit_time.filter(|c| t.unix < c - SKEW_SECONDS) {
        out.push(SignedAtProblem::BeforeReviewedCommit {
            signed_at: raw.to_string(),
            commit_time: c,
        });
    }
    if let Some(c) = stamp_commit_time.filter(|c| t.unix > c + SKEW_SECONDS) {
        out.push(SignedAtProblem::AfterStampCommit {
            signed_at: raw.to_string(),
            commit_time: c,
        });
    }
    if t.local_date(-SKEW_SECONDS) != date && t.local_date(SKEW_SECONDS) != date {
        out.push(SignedAtProblem::DateMismatch {
            signed_at: raw.to_string(),
            date: date.to_string(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: parse and format known instants; reference values from
    /// `date -u -d @<secs>` (GNU coreutils). Pass: exact equality.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn parse_and_format_known_instants() {
        // 2026-10-07T06:03:09Z = 1791352989 (date -u -d @1791352989).
        let t = parse_rfc3339("2026-10-07T14:03:09+08:00").unwrap();
        assert_eq!(t.unix, 1_791_352_989);
        assert_eq!(t.offset_minutes, 480);
        assert_eq!(
            parse_rfc3339("2026-10-07T06:03:09Z").unwrap().unix,
            1_791_352_989
        );
        assert_eq!(
            parse_rfc3339("2026-10-06T21:33:09-08:30").unwrap().unix,
            1_791_352_989
        );
        assert_eq!(format_utc(1_791_352_989), "2026-10-07T06:03:09+00:00");
        assert_eq!(
            parse_rfc3339(&format_utc(1_791_352_989)).unwrap().unix,
            1_791_352_989
        );
        assert_eq!(format_utc(0), "1970-01-01T00:00:00+00:00");
        assert_eq!(t.local_date(0), "2026-10-07");
        // 23:58 local: +5 min crosses midnight.
        let late = parse_rfc3339("2026-10-07T23:58:00+08:00").unwrap();
        assert_eq!(
            (late.local_date(0), late.local_date(SKEW_SECONDS)),
            ("2026-10-07".into(), "2026-10-08".into())
        );
        for bad in [
            "2026-10-07",
            "2026-10-07 14:03:09+08:00",
            "2026-10-07T14:03:09",
            "2026-10-07T14:03:09.5+08:00",
            "2026-02-30T00:00:00Z",
            "2026-13-01T00:00:00Z",
            "2026-10-07T24:00:00Z",
            "2026-10-07T14:03:09+0800",
            "2026-10-07T14:03:09*08:00",
            "2026-10-07T14:03:09+24:00",
            "2026-1a-07T14:03:09Z",
            "2026-10-07t14:03:09z",
        ] {
            assert_eq!(parse_rfc3339(bad), None, "{bad}");
        }
        assert!(parse_rfc3339("2024-02-29T00:00:00Z").is_some());
        assert!(parse_rfc3339("2025-02-29T00:00:00Z").is_none());
    }

    /// The clock reads as a parseable UTC timestamp.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn now_is_rfc3339_utc() {
        let now = now_utc();
        let t = parse_rfc3339(&now).unwrap();
        assert_eq!(t.offset_minutes, 0);
        assert!(t.unix > 1_700_000_000, "{now}");
    }

    /// The local clock parses, names the machine's own offset, agrees with
    /// UTC to within a second or two, and its `date_of` is the local date.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn now_local_is_rfc3339_with_the_local_offset() {
        let now = now_local();
        let t = parse_rfc3339(&now).unwrap();
        let utc = parse_rfc3339(&now_utc()).unwrap();
        assert!((t.unix - utc.unix).abs() <= 2, "{now}");
        let offset = chrono::Local::now().offset().local_minus_utc() / 60;
        assert_eq!(t.offset_minutes, offset, "{now}");
        assert_eq!(date_of(&now).unwrap(), now[..10]);
        assert_eq!(date_of("2026-10-07T23:30:00-02:00").unwrap(), "2026-10-07");
        assert_eq!(date_of("not a time"), None);
    }

    /// Methodology: each flag of the module doc, on both sides of the 5-min
    /// skew, plus absent and unparseable. Pass: exactly the predicted flags.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn plausibility_flags_and_skew() {
        let at = "2026-10-07T14:03:09+08:00";
        let t = 1_791_352_989;
        // Absent: nothing, whatever the git times.
        assert!(plausibility(None, "1999-01-01", Some(t + 9999), Some(t - 9999)).is_empty());
        // Plausible: reviewed commit before, stamp commit after.
        assert!(plausibility(Some(at), "2026-10-07", Some(t - 3600), Some(t + 60)).is_empty());
        // Unknown git times: only the date is judged.
        assert!(plausibility(Some(at), "2026-10-07", None, None).is_empty());
        // Within skew either way: no flag.
        assert!(plausibility(
            Some(at),
            "2026-10-07",
            Some(t + SKEW_SECONDS),
            Some(t - SKEW_SECONDS)
        )
        .is_empty());
        // One second past the skew: flagged.
        assert_eq!(
            plausibility(Some(at), "2026-10-07", Some(t + SKEW_SECONDS + 1), None),
            vec![SignedAtProblem::BeforeReviewedCommit {
                signed_at: at.into(),
                commit_time: t + SKEW_SECONDS + 1
            }]
        );
        assert_eq!(
            plausibility(Some(at), "2026-10-07", None, Some(t - SKEW_SECONDS - 1)),
            vec![SignedAtProblem::AfterStampCommit {
                signed_at: at.into(),
                commit_time: t - SKEW_SECONDS - 1
            }]
        );
        // The date in the written offset: 14:03 at +08:00 is the 7th; the
        // UTC date (also the 7th) is not what is compared.
        assert_eq!(
            plausibility(Some(at), "2026-10-06", None, None),
            vec![SignedAtProblem::DateMismatch {
                signed_at: at.into(),
                date: "2026-10-06".into()
            }]
        );
        let early = "2026-10-08T01:00:00+08:00"; // 2026-10-07T17:00Z
        assert!(plausibility(Some(early), "2026-10-08", None, None).is_empty());
        assert_eq!(plausibility(Some(early), "2026-10-07", None, None).len(), 1);
        // Near midnight, the skew lets either neighbouring date pass.
        assert!(
            plausibility(Some("2026-10-07T23:57:00+08:00"), "2026-10-08", None, None).is_empty()
        );
        assert!(
            plausibility(Some("2026-10-08T00:03:00+08:00"), "2026-10-07", None, None).is_empty()
        );
        assert_eq!(
            plausibility(Some("2026-10-08T00:06:00+08:00"), "2026-10-07", None, None).len(),
            1
        );
        // All three at once.
        assert_eq!(
            plausibility(Some(at), "2026-10-01", Some(t + 3600), Some(t - 3600)).len(),
            3
        );
        // Unparseable: one flag, nothing else judged.
        assert_eq!(
            plausibility(Some("yesterday"), "2026-10-07", Some(t), Some(t)),
            vec![SignedAtProblem::Unparseable {
                signed_at: "yesterday".into()
            }]
        );
        // Every problem describes itself.
        let all = plausibility(Some(at), "2026-10-01", Some(t + 3600), Some(t - 3600));
        let text: Vec<String> = all.iter().map(SignedAtProblem::describe).collect();
        assert!(
            text[0].contains("before the reviewed commit (2026-10-07T07:03:09+00:00)"),
            "{text:?}"
        );
        assert!(
            text[1].contains("after the commit that added the stamp"),
            "{text:?}"
        );
        assert!(text[2].contains("dated 2026-10-01"), "{text:?}");
        assert!(SignedAtProblem::Unparseable {
            signed_at: "x".into()
        }
        .describe()
        .contains("not RFC 3339"));
    }
}
