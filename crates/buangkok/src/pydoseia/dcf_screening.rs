// SPDX-License-Identifier: GPL-3.0-only
//! Multi-source dose-coefficient screening: pyDOSEIA's `compute_max_dcf`,
//! which looks a nuclide up in five inhalation and two ingestion coefficient
//! compilations and reports either the coefficient of the requested lung
//! absorption type or the largest one found.
//!
//! # Provenance
//!
//! Ported from pyDOSEIA `raddcffunc.py` (`get_dcfs_for_radionuclides`,
//! `compute_max_dcf` — the second definition, the one Python binds —
//! `merge_dataframes_with_source_hc2` and the seven `screen_*` readers),
//! upstream <https://github.com/BiswajitSadhu/pyDOSEIA> at commit
//! `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024 Dr. Biswajit
//! Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`).
//!
//! # What upstream uses it for (defect D7)
//!
//! Only the **report**: `outputfunc.agewise_dcfs_inh_gs_submersion` prints
//! these coefficients, while the inhalation dose itself uses
//! [`super::dcf::InhalationDcfTable::lookup`] on a different table. The two
//! need not agree.
//!
//! # No coefficient data ships with this crate
//!
//! | Upstream file | Source | Here |
//! |---|---|---|
//! | `inhalation_HC2/Annex_G_ICRP119_dcf_inh_public.xlsx` | ICRP Publication 119, Annex G (copyright ICRP) | not copied |
//! | `inhalation_HC2/Annex_H_ICRP119_...csv` | ICRP 119 Annex H | not copied; **never read** upstream (defect D18) |
//! | `inhalation_HC2/Table_A2-DOE-STD-1196-2011_dcf_inhal.csv` | US DOE-STD-1196-2011, Table A-2 (a US government standard) | not copied in this pass |
//! | `inhalation_HC2/Table_5_JAERI_...csv`, `Table_7_JAERI_...csv`, `ingestion_public/table_4_jaeri_ingestion_public.csv` | JAERI-Data/Code 2002-013 (JAEA; terms not established) | not copied |
//! | `ingestion_public/AnnexF_ICRP119_dcf_ingestion_public.csv` | ICRP 119 Annex F | not copied |
//!
//! Tables are read with [`ScreeningTable::from_csv`] from CSVs carrying
//! upstream's **renamed** column headers (upstream reassigns the headers by
//! position after reading).

use crate::pydoseia::csv::{col, num, split_csv};
use crate::pydoseia::dcf::nan_max;

/// The seven compilations `compute_max_dcf` reads, in the order it merges
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreeningSource {
    /// JAERI-Data/Code 2002-013 Table 7 (soluble/reactive gases), inhalation.
    Table7Jaeri,
    /// JAERI-Data/Code 2002-013 Table 5 (particulates), inhalation.
    Table5Jaeri,
    /// DOE-STD-1196-2011 Table A-2, inhalation.
    TableA2Doe,
    /// ICRP 119 Annex G, inhalation.
    AnnexGIcrp119,
    /// ICRP 119 Annex F, ingestion.
    AnnexFIcrp119,
    /// JAERI-Data/Code 2002-013 Table 4, ingestion.
    Table4Jaeri,
}

impl ScreeningSource {
    /// The file name upstream opens (`get_corrected_nuclide` matches words of
    /// the alternate-name keys against it). The ingestion tables are not
    /// passed through that function.
    #[must_use]
    pub const fn upstream_file_name(self) -> &'static str {
        match self {
            Self::Table7Jaeri => "Table_7_JAERI_dcf_inh_Public_Soluble_Reactive_Gases_Vapours.csv",
            Self::Table5Jaeri => "Table_5_JAERI_dcf_inh_particulates_public.csv",
            Self::TableA2Doe => "Table_A2-DOE-STD-1196-2011_dcf_inhal.csv",
            Self::AnnexGIcrp119 => "Annex_G_ICRP119_dcf_inh_public.xlsx",
            Self::AnnexFIcrp119 => "AnnexF_ICRP119_dcf_ingestion_public.csv",
            Self::Table4Jaeri => "table_4_jaeri_ingestion_public.csv",
        }
    }

    /// The six age columns after upstream's renaming. DOE's three middle ones
    /// are **singular** (`inh_5_year`, ...), so the merged frame has no
    /// `inh_5_years` value for DOE rows (defect D19).
    #[must_use]
    pub const fn age_columns(self) -> [&'static str; 6] {
        match self {
            Self::TableA2Doe => [
                "inh_infant",
                "inh_1_year",
                "inh_5_year",
                "inh_10_year",
                "inh_15_year",
                "inh_adult",
            ],
            _ => MERGED_AGE_COLUMNS,
        }
    }

    const fn has_type(self) -> bool {
        !matches!(self, Self::AnnexFIcrp119 | Self::Table4Jaeri)
    }
}

/// The age columns `compute_max_dcf` selects from, by upstream's brackets
/// (`<= 1`, `<= 2`, `<= 7`, `<= 12`, `<= 17`, otherwise adult).
pub const MERGED_AGE_COLUMNS: [&str; 6] = [
    "inh_infant",
    "inh_1_year",
    "inh_5_years",
    "inh_10_years",
    "inh_15_years",
    "inh_adult",
];

/// One row of a screening table.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreeningRow {
    /// Nuclide field as written.
    pub nuclide: String,
    /// Absorption type field (`None` for the ingestion tables or a blank).
    pub absorption_type: Option<String>,
    /// Coefficients, Sv/Bq, in the source's [`ScreeningSource::age_columns`] order.
    pub coefficients: [f64; 6],
}

/// One screening table.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreeningTable {
    /// Which compilation.
    pub source: ScreeningSource,
    /// Rows in file order.
    pub rows: Vec<ScreeningRow>,
}

impl ScreeningTable {
    /// Read a CSV with upstream's renamed headers: `Nuclide`, `Type` (the
    /// inhalation tables) and [`ScreeningSource::age_columns`].
    ///
    /// # Errors
    /// A missing column.
    pub fn from_csv(source: ScreeningSource, text: &str) -> Result<Self, String> {
        let (h, rows) = split_csv(text);
        let n = col(&h, "Nuclide")?;
        let t = if source.has_type() {
            Some(col(&h, "Type")?)
        } else {
            None
        };
        let ages: Vec<usize> = source
            .age_columns()
            .iter()
            .map(|c| col(&h, c))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            source,
            rows: rows
                .into_iter()
                .map(|r| ScreeningRow {
                    nuclide: r.get(n).cloned().unwrap_or_default(),
                    absorption_type: t.and_then(|t| r.get(t).filter(|s| !s.is_empty()).cloned()),
                    coefficients: core::array::from_fn(|k| num(r.get(ages[k]))),
                })
                .collect(),
        })
    }

    /// Upstream's `screen_*` filter: rows whose trimmed, upper-cased nuclide
    /// field contains the upper-cased name delimited by the string ends or
    /// `_` (regex `(?:^|_)(?:NAME)(?:_|$)`), so `I-131` also selects
    /// `I-131_ELEMENTAL`.
    #[must_use]
    pub fn screen(&self, name: &str) -> Vec<&ScreeningRow> {
        let target = name.to_uppercase();
        self.rows
            .iter()
            .filter(|r| delimited_match(&r.nuclide.trim().to_uppercase(), &target))
            .collect()
    }

    /// The value in the merged frame's column for `bracket`: NaN where this
    /// source has no column of that name (DOE's middle brackets, D19).
    fn merged_value(&self, row: &ScreeningRow, bracket: usize) -> f64 {
        if self.source.age_columns()[bracket] == MERGED_AGE_COLUMNS[bracket] {
            row.coefficients[bracket]
        } else {
            f64::NAN
        }
    }

    fn has_merged_column(&self, bracket: usize) -> bool {
        self.source.age_columns()[bracket] == MERGED_AGE_COLUMNS[bracket]
    }
}

fn delimited_match(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let bytes = hay.as_bytes();
    let mut start = 0;
    while let Some(pos) = hay[start..].find(needle) {
        let i = start + pos;
        let j = i + needle.len();
        let before_ok = i == 0 || bytes[i - 1] == b'_';
        let after_ok = j == hay.len() || bytes[j] == b'_';
        if before_ok && after_ok {
            return true;
        }
        start = i + 1;
        while !hay.is_char_boundary(start) {
            start += 1;
        }
    }
    false
}

/// Upstream's alternate nuclide names (from its nomenclature file), in its
/// key order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AlternateNames {
    /// `DOE_STD_1196_name`.
    pub doe_std_1196: Option<String>,
    /// `FGR_12_name`.
    pub fgr_12: Option<String>,
    /// `ICRP119_107_name`.
    pub icrp119_107: Option<String>,
    /// `ICRP_38_name`.
    pub icrp_38: Option<String>,
}

impl AlternateNames {
    /// `get_corrected_nuclide`: the first alternate name whose key shares a
    /// `_`-separated word with the (lower-cased) file name, else the name
    /// itself. Note the word `name` is in every key; upstream's file names
    /// happen not to contain it.
    #[must_use]
    pub fn corrected(&self, file_name: &str, nuclide: &str) -> String {
        let f = file_name.to_lowercase();
        let keys = [
            ("DOE_STD_1196_name", &self.doe_std_1196),
            ("FGR_12_name", &self.fgr_12),
            ("ICRP119_107_name", &self.icrp119_107),
            ("ICRP_38_name", &self.icrp_38),
        ];
        for (key, alt) in keys {
            if let Some(alt) = alt.as_ref().filter(|a| !a.is_empty()) {
                if key.to_lowercase().split('_').any(|w| f.contains(w)) {
                    return alt.clone();
                }
            }
        }
        nuclide.to_string()
    }
}

/// The screening tables available (any may be empty or absent).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScreeningTables {
    /// Inhalation tables, in any order (merged in upstream's order).
    pub inhalation: Vec<ScreeningTable>,
    /// Ingestion tables.
    pub ingestion: Vec<ScreeningTable>,
}

/// `compute_max_dcf`'s result: `max_dcf_inh_public` and `max_dcf_ing_public`
/// (`None` where upstream stores `None`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenedDcf {
    /// Inhalation, Sv/Bq.
    pub inhalation: Option<f64>,
    /// Ingestion, Sv/Bq.
    pub ingestion: Option<f64>,
}

/// Upstream's age-to-column bracket for this function.
#[must_use]
pub fn screening_bracket(age: f64) -> usize {
    if age <= 1.0 {
        0
    } else if age <= 2.0 {
        1
    } else if age <= 7.0 {
        2
    } else if age <= 12.0 {
        3
    } else if age <= 17.0 {
        4
    } else {
        5
    }
}

fn merged(
    tables: &[ScreeningTable],
    order: &[ScreeningSource],
    name_for: impl Fn(ScreeningSource) -> String,
) -> Vec<(ScreeningTable, Vec<ScreeningRow>)> {
    let mut out = Vec::new();
    for src in order {
        for t in tables.iter().filter(|t| t.source == *src) {
            let rows: Vec<ScreeningRow> = t.screen(&name_for(*src)).into_iter().cloned().collect();
            if !rows.is_empty() {
                out.push((t.clone(), rows));
            }
        }
    }
    out
}

fn select(
    merged: &[(ScreeningTable, Vec<ScreeningRow>)],
    bracket: usize,
    nuclide: &str,
    user_type: Option<&str>,
) -> (Option<f64>, Option<f64>) {
    if !merged.iter().any(|(t, _)| t.has_merged_column(bracket)) {
        return (None, None);
    }
    let value = merged.iter().find_map(|(t, rows)| {
        rows.iter()
            .find(|r| {
                r.nuclide == nuclide
                    && user_type.is_none_or(|u| r.absorption_type.as_deref() == Some(u))
            })
            .map(|r| t.merged_value(r, bracket))
    });
    let max = nan_max(
        merged
            .iter()
            .flat_map(|(t, rows)| rows.iter().map(move |r| t.merged_value(r, bracket))),
    );
    (value, Some(max))
}

/// `compute_max_dcf(radionuclide, user_type, age)`. `None` when neither
/// merged table has the age column (upstream returns `None`).
///
/// For `user_type == "Max"` both results are the largest coefficient among
/// the screened rows (of any nuclide the delimited match selected, and any
/// type). Otherwise the first row, in merge order, whose nuclide field
/// **equals** the name (not the alternate name used to screen) and whose
/// type equals `user_type` (ingestion: name only), falling back to the
/// maximum. The Annex H reader always fails upstream (D18) and contributes
/// nothing, so it has no input here.
#[must_use]
pub fn compute_max_dcf(
    tables: &ScreeningTables,
    radionuclide: &str,
    user_type: &str,
    alternate: &AlternateNames,
    age: f64,
) -> Option<ScreenedDcf> {
    use ScreeningSource as S;
    let inh = merged(
        &tables.inhalation,
        &[
            S::Table7Jaeri,
            S::Table5Jaeri,
            S::TableA2Doe,
            S::AnnexGIcrp119,
        ],
        |s| alternate.corrected(s.upstream_file_name(), radionuclide),
    );
    let ing = merged(
        &tables.ingestion,
        &[S::AnnexFIcrp119, S::Table4Jaeri],
        |_| radionuclide.to_string(),
    );
    let bracket = screening_bracket(age);
    let inh_has = inh.iter().any(|(t, _)| t.has_merged_column(bracket));
    let ing_has = ing.iter().any(|(t, _)| t.has_merged_column(bracket));
    if !inh_has && !ing_has {
        return None;
    }
    let (inh_v, inh_max) = select(&inh, bracket, radionuclide, Some(user_type));
    let (ing_v, ing_max) = select(&ing, bracket, radionuclide, None);
    Some(if user_type == "Max" {
        ScreenedDcf {
            inhalation: inh_max,
            ingestion: ing_max,
        }
    } else {
        ScreenedDcf {
            inhalation: inh_v.or(inh_max),
            ingestion: ing_v.or(ing_max),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delimited_match_follows_the_regex() {
        assert!(delimited_match("I-131", "I-131"));
        assert!(delimited_match("I-131_ELEMENTAL", "I-131"));
        assert!(delimited_match("X_I-131", "I-131"));
        assert!(!delimited_match("I-1311", "I-131"));
        assert!(!delimited_match("TI-131", "I-131"));
    }

    #[test]
    fn alternate_names_pick_by_file_words() {
        let a = AlternateNames {
            doe_std_1196: Some("doe".into()),
            icrp119_107: Some("icrp".into()),
            ..AlternateNames::default()
        };
        assert_eq!(
            a.corrected(ScreeningSource::TableA2Doe.upstream_file_name(), "X"),
            "doe"
        );
        assert_eq!(
            a.corrected(ScreeningSource::AnnexGIcrp119.upstream_file_name(), "X"),
            "icrp"
        );
        assert_eq!(
            a.corrected(ScreeningSource::Table5Jaeri.upstream_file_name(), "X"),
            "X"
        );
    }
}
