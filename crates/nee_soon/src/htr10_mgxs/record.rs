// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! What a case run leaves behind, and how it is written and read back.
//!
//! A [`CaseRecord`] is plain data so it can be serialised: temperatures and
//! times are `f64` in the unit their field name states (`_k`, `_s`), the
//! file-format exception to the crate's `uom` rule. The run API itself takes
//! and returns `uom` quantities ([`super::MgxsCase::temperature`]).

use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::mgxs::MgxsLibrary;

/// Where the run came from, supplied by the caller (the record cannot
/// discover its own commit).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// `git rev-parse --short=10 HEAD` of this repository, `-dirty` if a
    /// tracked file differed.
    pub commit: String,
    /// The same for the `reference-data/ace` submodule.
    pub ace_commit: String,
}

/// One pass of a case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PassRecord {
    /// `"scalar"` or `"matrix"`.
    pub pass: String,
    /// `k_eff`, mean over active generations.
    pub k: f64,
    /// Its within-run 1σ.
    pub k_sigma: f64,
    /// Histories simulated, inactive generations included.
    pub histories: u64,
    /// Histories whose position no cell claimed.
    pub lost_locate: u64,
    /// Neutrons that left through the vacuum boundary.
    pub leak_vacuum: u64,
    /// Shannon entropy of every generation's source \[bits\] (4 × 4 × 4 mesh
    /// over the fissile region, ceiling 6 bits).
    pub entropy: Vec<f64>,
    /// Wall-clock time of the transport call \[s\].
    pub transport_s: f64,
}

/// The fine library collapsed onto one structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollapsedSummary {
    /// [`super::Collapse::label`].
    pub structure: String,
    /// Groups.
    pub n_groups: usize,
    /// Algebraic `k_inf` of the constants (production over absorption,
    /// every zone, no leakage). Collapse conserves it, so it must be the same
    /// on every structure; it is NOT the core's `k_eff`, which has leakage.
    pub k_inf: f64,
    /// Largest `|Sigma_a + Sigma_s,row - Sigma_t| / Sigma_t` over every zone
    /// and visited group: the scalar/matrix consistency (#682's ν-scatter
    /// excess shows up here). Dominated by the most sparsely visited bin,
    /// so it reads large on a short run; quote the flux-weighted one.
    pub max_balance_residual: f64,
    /// The same residual weighted by `Sigma_t phi` (the collision rate) over
    /// every zone and group: what the bulk of the collisions see.
    pub weighted_balance_residual: f64,
}

/// One pass/fail check, recorded whichever way it went.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Check {
    /// What was checked.
    pub name: String,
    /// Whether it held.
    pub passed: bool,
    /// The numbers behind it.
    pub detail: String,
}

/// Everything a case run produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseRecord {
    /// Case name ([`super::MgxsCase::name`]).
    pub case: String,
    /// Why the case exists.
    pub purpose: String,
    /// Issue the result feeds.
    pub issue: String,
    /// Model description.
    pub model: String,
    /// Every material's temperature \[K\].
    pub temperature_k: f64,
    /// Particles per generation.
    pub particles: usize,
    /// Inactive generations.
    pub inactive: usize,
    /// Active generations.
    pub active: usize,
    /// Seed of both passes.
    pub seed: u64,
    /// Threads the transport ran on (changes the wall clock only).
    pub threads: usize,
    /// Nuclear data library and treatment notes.
    pub data_notes: Vec<String>,
    /// Commit of the code and of the ACE submodule.
    pub provenance: Provenance,
    /// Hardware headline (CPU, memory).
    pub hardware: String,
    /// Wall-clock time of nuclear-data processing \[s\].
    pub data_s: f64,
    /// The scalar and matrix passes.
    pub passes: Vec<PassRecord>,
    /// The fine (tally-structure) library, ascending energy, one zone per
    /// material in material-index order.
    pub library: MgxsLibrary,
    /// The library collapsed onto each reported structure.
    pub collapsed: Vec<CollapsedSummary>,
    /// Checks, passed or not.
    pub checks: Vec<Check>,
    /// Wall-clock time of the whole case \[s\].
    pub total_s: f64,
}

impl CaseRecord {
    /// `record.json` of a case folder.
    pub const JSON: &'static str = "record.json";
    /// `summary.csv` of a case folder.
    pub const CSV: &'static str = "summary.csv";
    /// `README.md` of a case folder.
    pub const README: &'static str = "README.md";

    /// `k_eff` of the scalar pass, the case's eigenvalue.
    #[must_use]
    pub fn k(&self) -> (f64, f64) {
        self.passes
            .first()
            .map_or((f64::NAN, f64::NAN), |p| (p.k, p.k_sigma))
    }

    /// Whether every check passed.
    #[must_use]
    pub fn all_checks_passed(&self) -> bool {
        self.checks.iter().all(|c| c.passed)
    }

    /// Header of [`Self::summary_csv_row`].
    pub const SUMMARY_HEADER: &'static str = "case,temperature_k,k,k_sigma,k_inf_fine,histories_per_pass,threads,transport_s,checks_passed,commit";

    /// One CSV row: the numbers to quote.
    #[must_use]
    pub fn summary_csv_row(&self) -> String {
        let (k, s) = self.k();
        let fine = self.collapsed.last().map_or(f64::NAN, |c| c.k_inf);
        let t: f64 = self.passes.iter().map(|p| p.transport_s).sum();
        let h = self.passes.first().map_or(0, |p| p.histories);
        format!(
            "{},{},{k:.6},{s:.6},{fine:.6},{h},{},{t:.0},{},{}",
            self.case,
            self.temperature_k,
            self.threads,
            self.all_checks_passed(),
            self.provenance.commit
        )
    }

    /// The case's `README.md`: methodology and results, generated from the
    /// record so the two cannot disagree.
    #[must_use]
    pub fn readme_md(&self) -> String {
        let mut m = String::new();
        let (k, s) = self.k();
        let _ = writeln!(m, "# HTR-10 MGXS case `{}`\n", self.case);
        let _ = writeln!(
            m,
            "> **Research, education and V&V only.** Not for reactor operation, licensing or safety decisions (`RESPONSIBLE_USE.md`).\n"
        );
        let _ = writeln!(
            m,
            "Generated by `cargo run --release -p nee_soon --example htr10_mgxs_case -- --case {}` (`nee_soon::htr10_mgxs`). Do not edit by hand; rerun instead.\n",
            self.case
        );
        let _ = writeln!(m, "**Purpose:** {} Feeds {}.\n", self.purpose, self.issue);
        let _ = writeln!(m, "## Methodology\n");
        let _ = writeln!(m, "- **Model:** {}.", self.model);
        let _ = writeln!(
            m,
            "- **Temperature:** every material at {} K (isothermal state point).",
            self.temperature_k
        );
        for n in &self.data_notes {
            let _ = writeln!(m, "- **Data:** {n}");
        }
        let _ = writeln!(
            m,
            "- **Transport:** `outram_mc_libs` hybrid tracker (delta tracking in the bed, surface tracking elsewhere), {} particles × \\[{} inactive + {} active\\] generations, seed {}, {} threads.",
            self.particles, self.inactive, self.active, self.seed, self.threads
        );
        let _ = writeln!(
            m,
            "- **Tallies:** two passes at the same seed (byte-identical histories): `[Material, Energy]` with `{}`, and `[Material, Energy, EnergyOut]` with the analog scatter matrix and the fission births. In the delta-tracked bed the flux and reaction rates come from the tentative-collision estimator (gh:#598).",
            "Flux, Total, Absorption, NuFission, KappaFission, Fission"
        );
        let _ = writeln!(
            m,
            "- **Condensation:** one zone per material, flux-weighted, on the 70-group tally structure (NJOY WIMS-69 plus 10 to 20 MeV), then collapsed exactly onto each structure below. Nothing is rebalanced or adjusted; the library in `record.json` is the raw condensation."
        );
        let _ = writeln!(
            m,
            "- **Not done here:** no multigroup `k_eff` (the #850 gate runs it, through `MgxsLibrary::to_physics_mg`), no σ on collapsed constants (the replica case measures spread), P0 scattering only, no delayed-neutron data.\n"
        );
        let _ = writeln!(m, "## Results\n");
        // Whether the model leaks is read off the run, not assumed from the
        // case: the full core has vacuum boundaries, a cell is reflective.
        let leaked: u64 = self.passes.iter().map(|p| p.leak_vacuum).sum();
        let leakage = if leaked == 0 {
            "No history leaked: the model is reflective, so `k_eff` is this model's `k_inf`"
        } else {
            "Histories leaked through vacuum boundaries: this is `k_eff` WITH leakage"
        };
        let _ = writeln!(
            m,
            "**`k_eff` = {k:.6} ± {s:.6}** (scalar pass, within-run 1σ, one seed). {leakage}.\n"
        );
        let _ = writeln!(m, "| pass | k | σ | histories | lost | vacuum leaks | entropy first → last \\[bits\\] | transport \\[s\\] |");
        let _ = writeln!(m, "|---|---|---|---|---|---|---|---|");
        for p in &self.passes {
            let ent = match (p.entropy.first(), p.entropy.last()) {
                (Some(a), Some(b)) => format!("{a:.3} → {b:.3}"),
                _ => "n/a".to_string(),
            };
            let _ = writeln!(
                m,
                "| {} | {:.6} | {:.6} | {} | {} | {} | {ent} | {:.0} |",
                p.pass, p.k, p.k_sigma, p.histories, p.lost_locate, p.leak_vacuum, p.transport_s
            );
        }
        let _ = writeln!(m, "\n| structure | groups | algebraic `k_inf` of the constants | balance residual, collision-weighted | balance residual, worst bin |");
        let _ = writeln!(m, "|---|---|---|---|---|");
        for c in &self.collapsed {
            let _ = writeln!(
                m,
                "| {} | {} | {:.6} | {:.2e} | {:.2e} |",
                c.structure, c.n_groups, c.k_inf, c.weighted_balance_residual, c.max_balance_residual
            );
        }
        let _ = writeln!(
            m,
            "\n`k_inf` is production over absorption summed over every zone, with no leakage, so it sits above `k_eff` by the model's leakage (none in a reflective cell, where the two agree within statistics). It must be identical on every structure (collapse is exact). The balance residual is `|Sigma_a + Sigma_s,row - Sigma_t| / Sigma_t`; the collision-weighted one is the number to quote, the worst bin is whichever (zone, group) saw fewest collisions. A weighted residual well above statistics is the ν-scatter excess of #682 or a condensation defect.\n"
        );
        let _ = writeln!(m, "### Checks\n");
        let _ = writeln!(m, "| check | result | detail |");
        let _ = writeln!(m, "|---|---|---|");
        for c in &self.checks {
            let _ = writeln!(
                m,
                "| {} | {} | {} |",
                c.name,
                if c.passed { "pass" } else { "**FAIL**" },
                c.detail
            );
        }
        let _ = writeln!(m, "\n### Run\n");
        let _ = writeln!(
            m,
            "- commit `{}`, ACE submodule `{}`",
            self.provenance.commit, self.provenance.ace_commit
        );
        let _ = writeln!(m, "- hardware: {}", self.hardware);
        let _ = writeln!(
            m,
            "- nuclear data {:.0} s, whole case {:.0} s",
            self.data_s, self.total_s
        );
        let _ = writeln!(m, "\n## Files\n");
        let _ = writeln!(
            m,
            "- `record.json`: everything above plus the 70-group library per material (`library.zones[i]`, ascending energy, cm^-1). Reload with `serde_json::from_str::<nee_soon::htr10_mgxs::CaseRecord>`, then `record.library.collapse(&Collapse::Eight.structure())`."
        );
        let _ = writeln!(m, "- `summary.csv`: one row, the numbers to quote.");
        m
    }

    /// Write `record.json`, `summary.csv` and `README.md` into `dir`,
    /// creating it.
    ///
    /// # Errors
    ///
    /// Any I/O or serialisation error, as a message.
    pub fn write(&self, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        // Compact, not pretty: the 70 x 70 matrices of ~30 materials are
        // ~150 000 numbers, one line each when pretty-printed. The README and
        // CSV are the human-readable parts.
        let json = serde_json::to_string(self).map_err(|e| e.to_string())?;
        let put = |name: &str, text: &str| {
            std::fs::write(dir.join(name), text).map_err(|e| format!("{name}: {e}"))
        };
        put(Self::JSON, &json)?;
        put(
            Self::CSV,
            &format!("{}\n{}\n", Self::SUMMARY_HEADER, self.summary_csv_row()),
        )?;
        put(Self::README, &self.readme_md())
    }

    /// Read `record.json` back from a case folder.
    ///
    /// # Errors
    ///
    /// A missing file, or JSON that is not a record (including a library
    /// whose edges do not ascend).
    pub fn read(dir: &Path) -> Result<Self, String> {
        let p = dir.join(Self::JSON);
        let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", p.display()))
    }
}

/// Rewrite `INDEX.md` in `root` from every `<root>/<case>/record.json`:
/// one table row per case, in case-name order, so the folder's state can be
/// read in one file. Folders without a record (a run in progress) are listed
/// as such.
///
/// # Errors
///
/// An unreadable `root`, or a `record.json` that does not parse (named in the
/// message, since a broken record should not be silently skipped).
pub fn write_index(root: &Path) -> Result<(), String> {
    let mut dirs: Vec<_> = std::fs::read_dir(root)
        .map_err(|e| format!("{}: {e}", root.display()))?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .map(|e| e.path())
        .collect();
    dirs.sort();
    let mut m = String::new();
    let _ = writeln!(m, "# HTR-10 MGXS case records\n");
    let _ = writeln!(
        m,
        "Written by `nee_soon::htr10_mgxs::record::write_index` after every case; do not edit by hand. Each folder's `README.md` has the methodology and results.\n"
    );
    let _ = writeln!(m, "| case | T \\[K\\] | k_eff | σ | k_inf (constants) | checks | transport \\[s\\] | threads | commit |");
    let _ = writeln!(m, "|---|---|---|---|---|---|---|---|---|");
    for d in dirs {
        let name = d.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        if !d.join(CaseRecord::JSON).exists() {
            let _ = writeln!(m, "| `{name}` | | no record yet | | | | | | |");
            continue;
        }
        let r = CaseRecord::read(&d)?;
        let (k, s) = r.k();
        let fine = r.collapsed.last().map_or(f64::NAN, |c| c.k_inf);
        let t: f64 = r.passes.iter().map(|p| p.transport_s).sum();
        let _ = writeln!(
            m,
            "| [`{name}`]({name}/README.md) | {} | {k:.6} | {s:.6} | {fine:.6} | {} | {t:.0} | {} | `{}` |",
            r.temperature_k,
            if r.all_checks_passed() { "all pass" } else { "**some FAIL**" },
            r.threads,
            r.provenance.commit
        );
    }
    std::fs::write(root.join("INDEX.md"), m).map_err(|e| format!("INDEX.md: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mgxs::{GroupStructure, ZoneMgxs};

    fn fixture() -> CaseRecord {
        CaseRecord {
            case: "unit".into(),
            purpose: "a test fixture".into(),
            issue: "#845".into(),
            model: "none".into(),
            temperature_k: 300.15,
            particles: 10,
            inactive: 1,
            active: 2,
            seed: 1,
            threads: 1,
            data_notes: vec!["none".into()],
            provenance: Provenance {
                commit: "abc".into(),
                ace_commit: "def".into(),
            },
            hardware: "test".into(),
            data_s: 1.0,
            passes: vec![PassRecord {
                pass: "scalar".into(),
                k: 1.01,
                k_sigma: 0.002,
                histories: 30,
                lost_locate: 0,
                leak_vacuum: 3,
                entropy: vec![5.0, 5.5],
                transport_s: 2.0,
            }],
            library: MgxsLibrary {
                groups: GroupStructure::new(vec![1.0e-5, 2.0e7]).expect("ascending"),
                zones: vec![ZoneMgxs {
                    name: "fuel".into(),
                    flux: vec![1.0],
                    total: vec![1.0],
                    absorption: vec![0.1],
                    fission: vec![0.05],
                    nu_fission: vec![0.12],
                    kappa_fission: vec![0.0],
                    scatter: vec![vec![0.9]],
                    chi: vec![1.0],
                    sigma: None,
                    scatter_only: Vec::new(),
                }],
            },
            collapsed: vec![CollapsedSummary {
                structure: "70g".into(),
                n_groups: 1,
                k_inf: 1.2,
                max_balance_residual: 0.0,
                weighted_balance_residual: 0.0,
            }],
            checks: vec![Check {
                name: "a check".into(),
                passed: false,
                detail: "x".into(),
            }],
            total_s: 4.0,
        }
    }

    /// A record written to a folder reads back identical, and its README and
    /// CSV carry the numbers (a failed check is shown as a failure).
    #[test]
    fn a_record_round_trips_through_its_folder() {
        let r = fixture();
        let dir = std::env::temp_dir().join(format!("nee_soon_mgxs_record_{}", std::process::id()));
        r.write(&dir).expect("write");
        assert_eq!(CaseRecord::read(&dir).expect("read"), r);
        let md = std::fs::read_to_string(dir.join(CaseRecord::README)).expect("readme");
        assert!(md.contains("1.010000 ± 0.002000"));
        assert!(md.contains("**FAIL**"));
        assert!(md.contains("Research, education and V&V only"));
        let csv = std::fs::read_to_string(dir.join(CaseRecord::CSV)).expect("csv");
        assert!(csv.starts_with(CaseRecord::SUMMARY_HEADER));
        assert!(csv.contains("unit,300.15,1.010000,0.002000,1.200000,30,1,2,false,abc"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The index lists a finished case with its numbers and a folder without
    /// a record as unfinished.
    #[test]
    fn the_index_lists_finished_and_unfinished_cases() {
        let root = std::env::temp_dir().join(format!("nee_soon_mgxs_index_{}", std::process::id()));
        fixture().write(&root.join("unit")).expect("write");
        std::fs::create_dir_all(root.join("running")).expect("mkdir");
        write_index(&root).expect("index");
        let md = std::fs::read_to_string(root.join("INDEX.md")).expect("read");
        assert!(md.contains("[`unit`](unit/README.md) | 300.15 | 1.010000 | 0.002000 | 1.200000 | **some FAIL**"));
        assert!(md.contains("| `running` | | no record yet |"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
