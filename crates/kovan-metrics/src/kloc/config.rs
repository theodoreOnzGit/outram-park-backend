//! The measurement's configuration: which repositories form the baseline, how
//! each crate is classified, and which commits are pinned.
//!
//! **This is the one editorial input to the whole accounting.** Everything else
//! is measured from git. The classification of a crate as translated, original
//! or an extension is a judgement, and it is carried here — beside each crate's
//! own `Cargo.toml` description in the output — precisely so that it can be
//! audited rather than taken on trust.
//!
//! Ported from the retired `scripts/kloc_accounting.py`. The reasoning comments
//! are reproduced rather than summarised: each one records why a number is what
//! it is, and losing them would leave the constants looking arbitrary.

/// GitHub account hosting the pre-agentic repositories.
pub const GITHUB_USER: &str = "theodoreOnzGit";

/// A checkout to measure.
#[derive(Clone, Debug)]
pub struct RepoSpec {
    /// Directory and GitHub repository name.
    pub key: &'static str,
    /// How the manuscript names it, as LaTeX.
    pub label: &'static str,
    /// Footnote text for the rate table's "AI?" column.
    pub note: &'static str,
    /// Footnote marker (a, b, c) in the table.
    pub marker: &'static str,
    /// Bib key, cited in the table row.
    pub cite: &'static str,
    /// Count lines at this commit instead of at the branch tip.
    ///
    /// Needed only where a repository was emptied after its code moved
    /// elsewhere — see [`BASELINE_REPOS`].
    pub measure_ref: Option<&'static str>,
}

/// The pre-agentic baseline, in the order the manuscript's table lists them.
///
/// # Why `thermal_hydraulics_rs` is pinned to a commit
///
/// On 2024-10-11 that repository was **emptied** — 260 `.rs` files down to 9 —
/// when its code moved into TUAS. Its tip therefore holds ~1.6 KLOC, which
/// would understate the predecessor by ~58 KLOC and, worse, would make the
/// "TUAS net of what it inherited" subtraction meaningless. `4d534af` is the
/// last commit at full extent (2024-10-08, 260 `.rs` files).
pub const BASELINE_REPOS: &[RepoSpec] = &[
    RepoSpec {
        key: "thermal_hydraulics_rs",
        label: r"\texttt{thermal\_hydraulics\_rs} (predecessor to TUAS)",
        note: "none",
        marker: "a",
        cite: "ong2024thermalhydraulicsrs",
        measure_ref: Some("4d534af51eca256f318462234c0c8592930f764b"),
    },
    RepoSpec {
        key: "chem-eng-real-time-process-control-simulator",
        label: r"\texttt{chem-eng-\ldots-simulator}",
        note: "none",
        marker: "a",
        cite: "ong2024chemengprocesscontrol",
        measure_ref: None,
    },
    RepoSpec {
        key: "teh-o-prke",
        label: r"\texttt{teh-o-prke}",
        note: "none",
        marker: "a",
        cite: "ong2025tehoprke",
        measure_ref: None,
    },
    RepoSpec {
        key: "tuas_boussinesq_solver",
        label: r"\texttt{tuas\_boussinesq\_solver} (net-new)",
        note: "none",
        marker: "a",
        cite: "ong2024tuasgithubrepo",
        measure_ref: None,
    },
    RepoSpec {
        key: "tampines-steam-tables",
        label: r"\texttt{tampines-steam-tables}",
        note: "root-finding solvers only",
        marker: "b",
        cite: "ong2026tampinessteamtables",
        measure_ref: None,
    },
    RepoSpec {
        key: "boon-lay",
        label: r"\texttt{boon-lay}",
        note: "code generation",
        marker: "c",
        cite: "ong2026boonlay",
        measure_ref: None,
    },
];

/// The lettered footnotes under the baseline tables.
pub const BASELINE_FOOTNOTES: &[(&str, &str)] = &[
    ("a", "No AI assistance of any kind."),
    (
        "b",
        "Minimal AI assistance, confined to root-finding solvers, via NUS AI-know.",
    ),
    (
        "c",
        "Non-agentic AI code generation for user interfaces and some Monte Carlo solvers, via NUS AI-know.",
    ),
];

/// TUAS, and the predecessor it is reported net of.
pub const TUAS_KEY: &str = "tuas_boussinesq_solver";
/// The predecessor whose imported tree is subtracted from TUAS.
pub const TUAS_PREDECESSOR_KEY: &str = "thermal_hydraulics_rs";

/// TUAS's second commit, which imported the predecessor wholesale 42 minutes
/// after the initial commit.
///
/// # What is subtracted, and why it is this and not the predecessor's extent
///
/// What is removed is **the code that actually came across** — the tree at this
/// commit — not the predecessor's own full extent. The two differ: the
/// predecessor held 260 `.rs` files at `4d534af`; 236 were imported.
/// Subtracting its full extent would remove 7,754 code lines that were written
/// in the predecessor and never carried forward, erasing real pre-agentic work
/// from the baseline and — because the baseline is the denominator of the
/// productivity claim — flattering the agentic figure. It would also contradict
/// the table caption, which says "net of the code it inherited": inherited
/// means what arrived, not what the predecessor happened to contain.
pub const TUAS_IMPORT_REF: &str = "c451c8e203d5772955c2c9f3c6739e92b8180c78";

/// The agentic repository is pinned to a commit, not to the branch tip.
///
/// `develop` is under active daily development — it moved four commits during a
/// single afternoon of preparing these tables, changing the translated subtotal
/// by 462 lines. A manuscript that quotes the tip quotes a number nobody can
/// reproduce afterwards. Set to `None` to measure the tip instead; the drift
/// check will then report how far the repository has moved since the pin.
pub const AGENTIC_MEASURE_REF: Option<&str> = Some("3130b38b65edc94288fb2715bbcab868e6846f79");

/// The agentic repository.
pub const AGENTIC_KEY: &str = "outram-park-backend";
/// How the manuscript names it.
pub const AGENTIC_LABEL: &str = r"\texttt{outram-park-backend}";

/// Start of the agentic window reported in the manuscript.
pub const AGENTIC_SINCE: &str = "2026-06-19";
/// End of the agentic window reported in the manuscript.
pub const AGENTIC_UNTIL: &str = "2026-07-23";

/// How a crate's lines are attributed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Provenance {
    /// A pure-Rust fork or port of a named upstream project.
    Translated,
    /// Newly written for Outram Park.
    Original,
    /// A vendored pre-agentic crate: only the excess over the standalone
    /// original is agentic.
    Extension,
}

impl Provenance {
    /// The stable display order used by every table and total.
    pub const ORDER: &'static [Provenance] = &[Self::Translated, Self::Original, Self::Extension];

    /// Short machine-readable name, as written to CSV.
    pub fn key(self) -> &'static str {
        match self {
            Self::Translated => "translated",
            Self::Original => "original",
            Self::Extension => "extension",
        }
    }

    /// Section heading used in the console report.
    pub fn label(self) -> &'static str {
        match self {
            Self::Translated => "Translated or ported",
            Self::Original => "Originally written",
            Self::Extension => "Agentic extensions to vendored crates",
        }
    }

    /// Section heading used in the LaTeX table.
    pub fn tex_section(self) -> &'static str {
        match self {
            Self::Translated => "Translated or ported from an existing codebase",
            Self::Original => "Originally written",
            Self::Extension => "Agentic extensions to vendored pre-agentic crates",
        }
    }

    /// Subtotal row label used in the LaTeX table.
    pub fn tex_subtotal(self) -> &'static str {
        match self {
            Self::Translated => r"\textbf{Subtotal, translated}",
            Self::Original => r"\textbf{Subtotal, original}",
            Self::Extension => r"\textbf{Subtotal, extensions}",
        }
    }
}

/// Provenance of each crate in `outram-park-backend/crates`, with the upstream
/// it derives from (for [`Provenance::Translated`]) or the pre-agentic
/// repository it extends (for [`Provenance::Extension`]).
pub const CRATE_PROVENANCE: &[(&str, Provenance, Option<&str>)] = &[
    (
        "njoy-outram-park-fork",
        Provenance::Translated,
        Some("NJOY2016"),
    ),
    (
        "outram-park-fork-coolprop",
        Provenance::Translated,
        Some("CoolProp"),
    ),
    (
        "outram-foam-appbuilder-lib",
        Provenance::Translated,
        Some("OpenFOAM"),
    ),
    (
        "outram-foam-basic-lib",
        Provenance::Translated,
        Some("OpenFOAM"),
    ),
    ("outram-mc-libs", Provenance::Translated, Some("OpenMC")),
    (
        "outram-park-fork-pflotran",
        Provenance::Translated,
        Some("PFLOTRAN"),
    ),
    ("outram-foam-mesh", Provenance::Translated, Some("OpenFOAM")),
    ("outram-blender", Provenance::Translated, Some("Blender")),
    (
        "outram-park-fork-dwsim-libs",
        Provenance::Translated,
        Some("DWSIM"),
    ),
    (
        "outram-foam-turbulence-lib",
        Provenance::Translated,
        Some("OpenFOAM"),
    ),
    ("outram-foam-cli", Provenance::Translated, Some("OpenFOAM")),
    (
        "outram-park-digital-twin-engine",
        Provenance::Original,
        None,
    ),
    ("tampines", Provenance::Original, None),
    ("kovan-tui", Provenance::Original, None),
    ("kovan-cli", Provenance::Original, None),
    ("kovan-codegen", Provenance::Original, None),
    ("kovan-semantics", Provenance::Original, None),
    ("kovan-literature", Provenance::Original, None),
    ("kovan-discovery", Provenance::Original, None),
    ("kovan-common", Provenance::Original, None),
    ("nee_soon", Provenance::Original, None),
    (
        "tampines-steam-tables",
        Provenance::Extension,
        Some("tampines-steam-tables"),
    ),
    ("boon-lay", Provenance::Extension, Some("boon-lay")),
    (
        "tuas_boussinesq_solver",
        Provenance::Extension,
        Some("tuas_boussinesq_solver"),
    ),
    ("teh-o-prke", Provenance::Extension, Some("teh-o-prke")),
    (
        "chem-eng-real-time-process-control-simulator",
        Provenance::Extension,
        Some("chem-eng-real-time-process-control-simulator"),
    ),
];

/// Look up a crate's classification.
pub fn provenance_of(crate_name: &str) -> Option<(Provenance, Option<&'static str>)> {
    CRATE_PROVENANCE
        .iter()
        .find(|(name, _, _)| *name == crate_name)
        .map(|(_, klass, upstream)| (*klass, *upstream))
}

/// Parts of a crate classified differently from the crate as a whole.
///
/// On 2026-07-23 the two terminal interfaces stopped being workspace crates and
/// became feature-gated binaries inside the libraries they drive, so a library
/// consumer no longer sees them as separate packages. Their code is newly
/// written, but it now lives inside crates that are ports of an existing
/// upstream. Counting them with their host crate would credit ~2.5 KLOC of
/// original interface work as translation and overstate the translated share.
/// They are therefore split out at the path boundary and reported separately.
pub const CRATE_SUBPATH_PROVENANCE: &[(&str, &str, Provenance, &str)] = &[
    (
        "njoy-outram-park-fork",
        "src/bin/njoy-tui",
        Provenance::Original,
        "njoy-tui",
    ),
    (
        "outram-mc-libs",
        "src/bin/outram-mc-tui",
        Provenance::Original,
        "outram-mc-tui",
    ),
];

/// Repositories grouped by how much non-agentic AI help each had.
///
/// The discussion compares repositories written with no AI help at all against
/// those that had some non-agentic help from NUS AI-know. Grouped here so the
/// rates quoted in the prose come out of the same run as the tables.
pub const ASSISTANCE_GROUPS: &[(&str, &[&str])] = &[
    (
        "no AI assistance at all",
        &[
            "thermal_hydraulics_rs",
            "chem-eng-real-time-process-control-simulator",
            "teh-o-prke",
            "tuas_boussinesq_solver",
        ],
    ),
    (
        "some non-agentic NUS AI-know help",
        &["tampines-steam-tables", "boon-lay"],
    ),
];

/// Values as printed in the manuscript, for the drift check only.
///
/// **These are not used in any computation.** They exist so the run can report
/// drift. The tables themselves are emitted by this code, so they cannot drift
/// through transcription error any more; what these still catch is the case
/// that matters — the repositories moving after the manuscript's *prose*
/// figures, percentages and headline ratio were written against them. Recorded
/// from the run of 2026-07-23 on `develop`.
pub const MANUSCRIPT: &[(&str, i64)] = &[
    ("baseline_total_lines", 303_463),
    ("baseline_code_lines", 181_298),
    ("baseline_active_days", 367),
    ("agentic_total_rust", 349_541),
    ("agentic_vendored_preagentic", 173_544),
    ("agentic_code_lines", 175_997),
    ("subtotal_translated", 136_462),
    ("subtotal_original", 27_177),
    ("subtotal_extension", 12_358),
    ("n_crates", 26),
];

/// Crates whose names need something other than a plain `\texttt{}` rendering.
pub const CRATE_TEX_NAME: &[(&str, &str)] = &[
    (
        "chem-eng-real-time-process-control-simulator",
        r"\texttt{chem-eng-\ldots-simulator}",
    ),
    ("nee_soon", r"\texttt{nee\_soon}$^{\dagger}$"),
];

/// Header stamped onto every generated LaTeX file.
pub const GENERATED_BY: &str = "% Generated by kovan kloc -- do not edit by hand.\n\
                                % Re-run the command to update; edits here will be overwritten.\n";

/// Abbreviated month names, for the table's period column.
pub const MONTHS: &[&str] = &[
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every crate the extension class names must be a real baseline
    /// repository, or its pre-agentic original silently counts as agentic.
    #[test]
    fn every_extension_points_at_a_baseline_repository() {
        for (name, klass, upstream) in CRATE_PROVENANCE {
            if *klass != Provenance::Extension {
                continue;
            }
            let upstream = upstream.expect("an extension must name what it extends");
            assert!(
                BASELINE_REPOS.iter().any(|r| r.key == upstream),
                "{name} extends {upstream}, which is not a baseline repository -- \
                 its pre-agentic lines would be counted as agentic"
            );
        }
    }

    #[test]
    fn crate_classifications_are_unique() {
        let mut seen: Vec<&str> = CRATE_PROVENANCE.iter().map(|(n, _, _)| *n).collect();
        let before = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(before, seen.len(), "a crate is classified twice");
    }

    #[test]
    fn baseline_repository_keys_are_unique() {
        let mut keys: Vec<&str> = BASELINE_REPOS.iter().map(|r| r.key).collect();
        let before = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(before, keys.len());
    }

    /// The assistance groups exist to be quoted in prose; a key that names no
    /// repository would silently drop from the comparison.
    #[test]
    fn assistance_groups_name_real_repositories() {
        for (group, keys) in ASSISTANCE_GROUPS {
            for key in *keys {
                assert!(
                    BASELINE_REPOS.iter().any(|r| r.key == *key),
                    "{group} names {key}, which is not a baseline repository"
                );
            }
        }
    }

    /// Every baseline repository must carry a footnote marker that exists.
    #[test]
    fn footnote_markers_resolve() {
        for repo in BASELINE_REPOS {
            if repo.marker.is_empty() {
                continue;
            }
            assert!(
                BASELINE_FOOTNOTES.iter().any(|(m, _)| *m == repo.marker),
                "{} carries marker {} with no footnote",
                repo.key,
                repo.marker
            );
        }
    }

    #[test]
    fn the_subpath_overrides_name_real_crates() {
        for (host, _, _, _) in CRATE_SUBPATH_PROVENANCE {
            assert!(
                provenance_of(host).is_some(),
                "{host} has a subpath override but no classification"
            );
        }
    }

    #[test]
    fn the_manuscript_crate_count_matches_the_classification_table() {
        let want = MANUSCRIPT
            .iter()
            .find(|(k, _)| *k == "n_crates")
            .map(|(_, v)| *v)
            .unwrap();
        assert_eq!(
            CRATE_PROVENANCE.len() as i64,
            want,
            "the manuscript reports {want} crates; the classification table has {}",
            CRATE_PROVENANCE.len()
        );
    }
}
