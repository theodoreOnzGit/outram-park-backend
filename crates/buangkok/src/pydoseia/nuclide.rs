// SPDX-License-Identifier: GPL-3.0-only
//! Half-life text parsing and decay constants, as pyDOSEIA does them.
//!
//! # Provenance
//!
//! Ported from pyDOSEIA `raddcffunc.py`: the nested
//! `convert_half_life_to_seconds` of `RaddcfFunc.get_nuclide_info` (primary
//! half-life table) and the half-life branch of
//! `RaddcfFunc.find_progeny_name_and_yield_f` (progeny table). Upstream
//! <https://github.com/BiswajitSadhu/pyDOSEIA> at commit
//! `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024 Dr. Biswajit
//! Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`).
//!
//! # No half-life data here
//!
//! Upstream bundles half-life tables (its `library/half_life/`, derived from
//! ICRP Publication 107 and JAERI-Data/Code 2002-013). **None is copied into
//! this crate.** The workspace's half-life source is `boon-lay`
//! (`boon_lay::nuclide_reaction_and_decay_data`, `try_get_half_life`); a
//! caller supplies decay constants to [`super::dose`]. This module only
//! parses upstream's text formats, so a user-supplied table in upstream's
//! format can be read and the port can be verified against upstream.
//!
//! # Two different year lengths and `ln 2 = 0.693`
//!
//! Upstream is not internally consistent, and the port keeps both behaviours:
//! the primary-table parser uses a Gregorian year of **31 556 952 s**, the
//! progeny parser uses **365 days**, and the decay constant is
//! `0.693 / T_half` (not `ln 2 = 0.693147...`, a relative difference of
//! 2.1e-4).

/// Upstream's value of `ln 2`, as written: `0.693`.
pub const UPSTREAM_LN2: f64 = 0.693;

/// Upstream's decay constant, `0.693 / T_half`, 1/s, for a half-life in s.
#[must_use]
pub fn upstream_decay_constant(half_life_s: f64) -> f64 {
    UPSTREAM_LN2 / half_life_s
}

/// Parse a primary-table half-life string such as `"30.0 y"`, `"8.0 d"`,
/// `"3.0 ms"` to seconds (upstream `convert_half_life_to_seconds`).
///
/// Units are tried in upstream's order, `ls` (1e-6), `ms`, `s`, `m`, `h`,
/// `d`, `y` (31 556 952 s), by *suffix*; the unit is then removed and the
/// rest parsed. Returns `None` if no unit matches or the number does not
/// parse, as upstream does.
#[must_use]
pub fn parse_primary_half_life(text: &str) -> Option<f64> {
    const UNITS: [(&str, f64); 7] = [
        ("ls", 1e-6),
        ("ms", 1e-3),
        ("s", 1.0),
        ("m", 60.0),
        ("h", 3600.0),
        ("d", 86400.0),
        ("y", 31_556_952.0),
    ];
    let t = text.trim();
    for (unit, factor) in UNITS {
        if t.ends_with(unit) {
            let value: f64 = t.replace(unit, "").trim().parse().ok()?;
            return Some(value * factor);
        }
    }
    None
}

/// Parse a progeny-table half-life string such as `"2.5 m"` or `"12.32 y"` to
/// seconds, exactly as upstream's `find_progeny_name_and_yield_f` does.
///
/// The unit tests are **substring** tests in upstream's order: `'s'`, then
/// `'m'`, `'d'`, `'a'` or `'y'` (365 days), `'h'`; the number is everything
/// but the last character. A string containing none of these letters is an
/// error upstream (`ValueError`); here it is `None`.
#[must_use]
pub fn parse_progeny_half_life(text: &str) -> Option<f64> {
    let number = |t: &str| -> Option<f64> {
        let mut chars = t.chars();
        chars.next_back()?;
        chars.as_str().trim().parse::<f64>().ok()
    };
    if text.contains('s') {
        number(text)
    } else if text.contains('m') {
        Some(number(text)? * 60.0)
    } else if text.contains('d') {
        Some(number(text)? * 3600.0 * 24.0)
    } else if text.contains('a') || text.contains('y') {
        Some(number(text)? * 3600.0 * 24.0 * 365.0)
    } else if text.contains('h') {
        Some(number(text)? * 3600.0)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_units() {
        assert_eq!(parse_primary_half_life("250.0 s"), Some(250.0));
        assert_eq!(parse_primary_half_life("3.0 ms"), Some(3.0e-3));
        assert_eq!(parse_primary_half_life("1.0 y"), Some(31_556_952.0));
        assert_eq!(parse_primary_half_life("1.0 w"), None);
    }

    #[test]
    fn progeny_units_use_a_365_day_year() {
        assert_eq!(parse_progeny_half_life("1.0 y"), Some(31_536_000.0));
        assert_eq!(parse_progeny_half_life("2.5 m"), Some(150.0));
        assert_eq!(parse_progeny_half_life("5.0 h"), Some(18_000.0));
        // "stable" contains an 's', so upstream tries float("stabl") and fails.
        assert_eq!(parse_progeny_half_life("stable"), None);
    }
}
