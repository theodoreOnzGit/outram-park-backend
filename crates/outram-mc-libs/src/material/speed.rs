//! [`SpeedTier`]: how much a nuclide may trade for speed.
//!
//! One selector covers both halves of a run: how a [`Nuclide`] is built from
//! ENDF (nuclear-data processing) and how its cross sections are looked up
//! during transport.
//!
//! | tier | results vs `Standard` | what changes |
//! |---|---|---|
//! | [`Standard`](SpeedTier::Standard) | the reference | nothing: the unoptimized lookup, kept to check `Fast` against |
//! | [`Fast`](SpeedTier::Fast) (**default**) | **identical**, to the last digit | exact optimizations of the transport lookup |
//! | [`VeryFast`](SpeedTier::VeryFast) | **approximate**, shift measured | `Fast`, plus a coarser RECONR/BROADR tolerance |
//!
//! `Fast` is the default because it is exact: this crate's rule is that the
//! cheapest *correct* path is the default, not an opt-in (`CLAUDE.md`,
//! maintainer direction 2026-09-25). `VeryFast` is never a default, because it
//! is not correct to NJOY's tolerance.
//!
//! Choose it when the nuclide is built, with
//! [`Nuclide::from_endf_file_with_speed`], or on an existing nuclide with
//! [`Nuclide::with_speed`] (transport lookup only; see its docs).
//!
//! Measured costs and speed-ups: `docs/profiling/speed_tiers_2026_09_27.md`.
//!
//! [`Nuclide`]: crate::material::nuclide::Nuclide
//! [`Nuclide::from_endf_file_with_speed`]: crate::material::nuclide::Nuclide::from_endf_file_with_speed
//! [`Nuclide::with_speed`]: crate::material::nuclide::Nuclide::with_speed

/// How much a nuclide may trade for speed: `Standard`, `Fast` or `VeryFast`.
///
/// `Fast` is the default. `Standard` is the unoptimized reference that `Fast`
/// must reproduce exactly. `VeryFast` is an approximation whose effect on `k`
/// is measured and recorded, never assumed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SpeedTier {
    /// The reference path: the code as it was before these optimizations,
    /// kept so that `Fast` can be checked against it.
    ///
    /// - **Nuclear data:** RECONR and BROADR at NJOY's tolerance, `0.001`.
    /// - **Transport:** each cross section is found by searching the reaction
    ///   list on every call, and choosing a collision nuclide evaluates each
    ///   nuclide's full cross-section set.
    Standard,

    /// **The default.** Exact optimizations: every number is the same as
    /// `Standard`'s, so a seeded run gives the same `k` to the last digit.
    ///
    /// - **Nuclear data:** identical to `Standard`.
    /// - **Transport:** reaction positions are resolved once when the nuclide
    ///   is built rather than searched for on every call, and choosing a
    ///   collision nuclide evaluates only each nuclide's total cross section
    ///   ([`Nuclide::total_at_energy`], which equals
    ///   `xs_at_energy(..).total` exactly). The interpolation arithmetic is
    ///   shared with `Standard`, not duplicated.
    ///
    /// [`Nuclide::total_at_energy`]: crate::material::nuclide::Nuclide::total_at_energy
    #[default]
    Fast,

    /// `Fast`, plus an **approximation** in the nuclear data.
    ///
    /// - **Nuclear data:** RECONR and BROADR at tolerance `0.01`, one decade
    ///   coarser than NJOY's `0.001`, so each table carries fewer points and is
    ///   built and searched faster. Cross sections between points are accurate
    ///   to about 1 % instead of 0.1 %.
    /// - **Transport:** as `Fast`.
    ///
    /// The value `0.01` was chosen before measuring, as "one decade coarser",
    /// and was not tuned to a benchmark. Its measured effect on the four ICSBEP
    /// benchmarks is recorded in `docs/profiling/speed_tiers_2026_09_27.md`;
    /// read it before quoting a `VeryFast` result as a benchmark comparison.
    VeryFast,
}

impl SpeedTier {
    /// Fractional reconstruction tolerance for RECONR and BROADR at this tier
    /// (dimensionless): `0.001` for `Standard` and `Fast`, `0.01` for
    /// `VeryFast`.
    pub fn data_tolerance(self) -> f64 {
        match self {
            SpeedTier::Standard | SpeedTier::Fast => 1.0e-3,
            SpeedTier::VeryFast => 1.0e-2,
        }
    }

    /// `true` if this tier gives the same numbers as `Standard` (`Standard`
    /// and `Fast`); `false` for `VeryFast`, which approximates.
    pub fn is_exact(self) -> bool {
        !matches!(self, SpeedTier::VeryFast)
    }

    /// `true` if cross sections are looked up by precomputed reaction
    /// positions (`Fast` and `VeryFast`) instead of by searching (`Standard`).
    pub(crate) fn indexed_lookup(self) -> bool {
        !matches!(self, SpeedTier::Standard)
    }
}

impl std::fmt::Display for SpeedTier {
    /// `standard`, `fast` or `very-fast`: the spelling [`str::parse`] accepts.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            SpeedTier::Standard => "standard",
            SpeedTier::Fast => "fast",
            SpeedTier::VeryFast => "very-fast",
        })
    }
}

impl std::str::FromStr for SpeedTier {
    type Err = String;

    /// Parse `standard`, `fast` or `very-fast`, ignoring case; `veryfast` and
    /// `very_fast` are accepted too.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "standard" => Ok(SpeedTier::Standard),
            "fast" => Ok(SpeedTier::Fast),
            "very-fast" | "veryfast" | "very_fast" => Ok(SpeedTier::VeryFast),
            other => Err(format!(
                "unknown speed tier `{other}`: expected standard, fast or very-fast"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default is the cheapest EXACT tier: a caller who does not choose
    /// gets the optimized path, and never an approximation (crate `CLAUDE.md`:
    /// "the cheapest correct path is the default").
    #[test]
    fn fast_is_the_default_and_it_is_exact() {
        assert_eq!(SpeedTier::default(), SpeedTier::Fast);
        assert!(SpeedTier::default().is_exact());
        assert_eq!(SpeedTier::default().data_tolerance(), 1.0e-3);
    }

    #[test]
    fn only_very_fast_approximates() {
        assert!(SpeedTier::Standard.is_exact());
        assert!(SpeedTier::Fast.is_exact());
        assert!(!SpeedTier::VeryFast.is_exact());
        assert_eq!(SpeedTier::Fast.data_tolerance(), SpeedTier::Standard.data_tolerance());
        assert!(SpeedTier::VeryFast.data_tolerance() > SpeedTier::Standard.data_tolerance());
    }

    #[test]
    fn parses_what_it_prints() {
        for t in [SpeedTier::Standard, SpeedTier::Fast, SpeedTier::VeryFast] {
            assert_eq!(t.to_string().parse::<SpeedTier>(), Ok(t));
        }
        assert_eq!("Very_Fast".parse::<SpeedTier>(), Ok(SpeedTier::VeryFast));
        assert!("turbo".parse::<SpeedTier>().is_err());
    }
}
