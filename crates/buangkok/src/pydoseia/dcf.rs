// SPDX-License-Identifier: GPL-3.0-only
//! Dose-coefficient (DCF) tables and pyDOSEIA's lookups into them: the age
//! brackets, the lung-absorption-type selection for inhalation, and the
//! short-lived-progeny correction for the external pathways.
//!
//! # Provenance
//!
//! Ported from pyDOSEIA `raddcffunc.py` (`RaddcfFunc.inhalation_dcf_list`,
//! `dcf_list_ecerman_ground_shine_include_progeny`,
//! `dcf_list_ecerman_submersion_include_progeny`,
//! `find_progeny_name_and_yield_f`), upstream
//! <https://github.com/BiswajitSadhu/pyDOSEIA> at commit
//! `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024 Dr. Biswajit
//! Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`).
//!
//! # No coefficient data ships with this crate — the caller supplies it
//!
//! Upstream bundles its coefficients. Their sources, and why none is copied
//! here (details in `crates/buangkok/docs/pydoseia-port-scoping.md`):
//!
//! | Upstream file / sheet | Content | Source | Here |
//! |---|---|---|---|
//! | `RadioToxicityMaster.xls` / `Inhalation CED Sv per Bq Public` | inhalation e(g), six ages, types F/M/S/V | ICRP (Publ. 72 values, as reproduced in the IAEA BSS); ICRP data are copyrighted | **not copied**; load your own via [`InhalationDcfTable::from_csv`] |
//! | `Dose_ecerman_final.xlsx` / `surface_dose`, `submersion_dose` | external dose-rate coefficients, six ages | US EPA **FGR-15** (EPA-402/R-19/002, 2019), Table 4-1 and the submersion table; a US federal report | not copied in this pass; loadable via [`ExternalDcfTable::from_csv`] |
//! | `dcf_corr.xlsx` | decay chains and branching | upstream says "SRS 19 based on ICRP 107" (IAEA / ICRP) | **not copied**; load your own via [`ProgenyChains::from_csv`] |
//!
//! **Note (2026-09-29):** the FGR-15 edition named above is upstream's, the
//! 2019 EPA-402/R-19/002, which EPA has since **withdrawn** ("contained errors
//! in the dose coefficient tables"). The coefficients buangkok does ship, in
//! [`crate::coefficients`], are from the **July 2025 revision, EPA
//! 402-R-25-001**, for five nuclides only.
//!
//! The tables are read from CSVs in upstream's column layout (see each
//! `from_csv`). The code-to-code test uses **synthetic** tables in that layout
//! (`tests/data/pydoseia_synthetic_*.csv`), so no copyrighted coefficient is in
//! the repository.

/// The six age brackets every pyDOSEIA DCF lookup uses, selected from an age in
/// years with upstream's boundaries (`age <= 1`, `1 < age <= 2`, `2 < age <= 7`,
/// `7 < age <= 12`, `12 < age <= 17`, `age > 17`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgeBracket {
    /// `age <= 1` (ICRP "< 1 a" / FGR-15 "Newborn").
    Infant,
    /// `1 < age <= 2` ("1-2 a" / "1-yr-old").
    OneToTwo,
    /// `2 < age <= 7` ("2-7 a" / "5-yr-old").
    TwoToSeven,
    /// `7 < age <= 12` ("7-12 a" / "10-yr-old").
    SevenToTwelve,
    /// `12 < age <= 17` ("12-17 a" / "15-yr-old").
    TwelveToSeventeen,
    /// `age > 17` ("> 17 a" / "Adult").
    Adult,
}

impl AgeBracket {
    /// Upstream's bracket for an age in years; `None` for NaN (upstream raises
    /// `ValueError`).
    #[must_use]
    pub fn from_age_years(age: f64) -> Option<Self> {
        if age > 17.0 {
            Some(Self::Adult)
        } else if 12.0 < age && age <= 17.0 {
            Some(Self::TwelveToSeventeen)
        } else if 7.0 < age && age <= 12.0 {
            Some(Self::SevenToTwelve)
        } else if 2.0 < age && age <= 7.0 {
            Some(Self::TwoToSeven)
        } else if 1.0 < age && age <= 2.0 {
            Some(Self::OneToTwo)
        } else if age <= 1.0 {
            Some(Self::Infant)
        } else {
            None
        }
    }

    /// Column index, 0 (infant) to 5 (adult), in upstream's table order.
    #[must_use]
    pub const fn column(self) -> usize {
        self as usize
    }
}

/// Lung absorption type used to pick inhalation rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LungAbsorptionType {
    /// Fast.
    F,
    /// Moderate.
    M,
    /// Slow.
    S,
    /// Vapour / gas.
    V,
    /// Upstream's `'Max'`: every row whose type contains F, M, S or V, taking
    /// the largest coefficient.
    Max,
}

impl LungAbsorptionType {
    fn matches(self, type_field: &str) -> bool {
        match self {
            Self::F => type_field.contains('F'),
            Self::M => type_field.contains('M'),
            Self::S => type_field.contains('S'),
            Self::V => type_field.contains('V'),
            Self::Max => ['F', 'M', 'S', 'V'].iter().any(|c| type_field.contains(*c)),
        }
    }
}

use super::csv::{col, num, split_csv};

/// pandas' `Series.max()`: the maximum ignoring NaN; NaN if nothing remains.
pub(crate) fn nan_max(values: impl Iterator<Item = f64>) -> f64 {
    values
        .filter(|v| !v.is_nan())
        .fold(f64::NAN, |m, v| if m.is_nan() || v > m { v } else { m })
}

/// One inhalation-coefficient row: nuclide, absorption type, e(g) in Sv/Bq for
/// the six [`AgeBracket`]s (NaN where blank).
#[derive(Debug, Clone, PartialEq)]
pub struct InhalationDcfRow {
    /// Nuclide name as written in the table.
    pub nuclide: String,
    /// Absorption type field (`None` if blank; such rows never match).
    pub absorption_type: Option<String>,
    /// Committed effective dose per unit intake, Sv/Bq, by age bracket.
    pub sv_per_bq: [f64; 6],
}

/// An inhalation coefficient table in upstream's layout.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct InhalationDcfTable {
    /// The rows, in file order.
    pub rows: Vec<InhalationDcfRow>,
}

impl InhalationDcfTable {
    /// The six coefficient columns of upstream's sheet, in [`AgeBracket`] order.
    pub const AGE_COLUMNS: [&'static str; 6] = [
        "e_g_age_g_lt_1a_Sv/Bq",
        "e_g_age_g_1_2a_Sv/Bq",
        "e_g_age_g_2_7a_Sv/Bq",
        "e_g_age_g_7_12a_Sv/Bq",
        "e_g_age_g_12_17a_Sv/Bq",
        "e_g_age_g_gt_17a_Sv/Bq",
    ];

    /// Read a CSV export of upstream's `Inhalation CED Sv per Bq Public` sheet:
    /// columns `Nuclide`, `Type` and [`Self::AGE_COLUMNS`] (others ignored).
    ///
    /// # Errors
    /// A missing column.
    pub fn from_csv(text: &str) -> Result<Self, String> {
        let (h, rows) = split_csv(text);
        let n = col(&h, "Nuclide")?;
        let t = col(&h, "Type")?;
        let ages: Vec<usize> = Self::AGE_COLUMNS
            .iter()
            .map(|c| col(&h, c))
            .collect::<Result<_, _>>()?;
        let rows = rows
            .into_iter()
            .map(|r| InhalationDcfRow {
                nuclide: r.get(n).cloned().unwrap_or_default(),
                absorption_type: r.get(t).filter(|s| !s.is_empty()).cloned(),
                sv_per_bq: core::array::from_fn(|k| num(r.get(ages[k]))),
            })
            .collect();
        Ok(Self { rows })
    }

    /// Upstream's `inhalation_dcf_list` for one nuclide: the largest
    /// coefficient among rows with exactly this nuclide name and a matching
    /// absorption type, Sv/Bq. NaN if no row matches (upstream: pandas'
    /// max of an empty column).
    #[must_use]
    pub fn lookup(&self, nuclide: &str, absorption: LungAbsorptionType, age: AgeBracket) -> f64 {
        nan_max(
            self.rows
                .iter()
                .filter(|r| r.nuclide == nuclide)
                .filter(|r| {
                    r.absorption_type
                        .as_deref()
                        .is_some_and(|t| absorption.matches(t))
                })
                .map(|r| r.sv_per_bq[age.column()]),
        )
    }
}

/// One external dose-rate coefficient row: nuclide and the six age columns
/// (Sv m^2/(Bq s) for ground surface; Sv m^3/(Bq s) for submersion).
#[derive(Debug, Clone, PartialEq)]
pub struct ExternalDcfRow {
    /// Nuclide name as written in the table.
    pub nuclide: String,
    /// Coefficients by age bracket (NaN where blank).
    pub coefficients: [f64; 6],
}

/// An external-exposure coefficient table (ground surface or submersion) in
/// upstream's FGR-15-derived layout.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExternalDcfTable {
    /// The rows, in file order.
    pub rows: Vec<ExternalDcfRow>,
}

impl ExternalDcfTable {
    /// Upstream's column names, in [`AgeBracket`] order.
    pub const AGE_COLUMNS: [&'static str; 6] = [
        "Newborn",
        "1-yr-old",
        "5-yr-old",
        "10-yr-old",
        "15-yr-old",
        "Adult",
    ];

    /// Read a CSV export of upstream's `surface_dose` or `submersion_dose`
    /// sheet: columns `Nuclide` and [`Self::AGE_COLUMNS`].
    ///
    /// # Errors
    /// A missing column.
    pub fn from_csv(text: &str) -> Result<Self, String> {
        let (h, rows) = split_csv(text);
        let n = col(&h, "Nuclide")?;
        let ages: Vec<usize> = Self::AGE_COLUMNS
            .iter()
            .map(|c| col(&h, c))
            .collect::<Result<_, _>>()?;
        let rows = rows
            .into_iter()
            .map(|r| ExternalDcfRow {
                nuclide: r.get(n).cloned().unwrap_or_default(),
                coefficients: core::array::from_fn(|k| num(r.get(ages[k]))),
            })
            .collect();
        Ok(Self { rows })
    }

    /// Largest coefficient over rows whose name **equals** `nuclide` (upstream's
    /// parent lookup).
    #[must_use]
    pub fn lookup_exact(&self, nuclide: &str, age: AgeBracket) -> f64 {
        nan_max(
            self.rows
                .iter()
                .filter(|r| r.nuclide == nuclide)
                .map(|r| r.coefficients[age.column()]),
        )
    }

    /// Largest coefficient over rows whose name **contains** `nuclide`
    /// (upstream's daughter lookup, `str.contains`).
    ///
    /// **Upstream defect D4, reproduced:** a substring match, so daughter
    /// `"Y-90"` also matches a `"Y-90m"` row (or any longer name containing
    /// it) and the larger coefficient wins.
    #[must_use]
    pub fn lookup_contains(&self, nuclide: &str, age: AgeBracket) -> f64 {
        nan_max(
            self.rows
                .iter()
                .filter(|r| r.nuclide.contains(nuclide))
                .map(|r| r.coefficients[age.column()]),
        )
    }
}

/// Decay chains for the progeny correction: each parent's daughters with
/// their yields, and each daughter's half-life text.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProgenyChains {
    /// `(parent, daughter, yield)`, in file order.
    pub links: Vec<(String, String, f64)>,
    /// `(nuclide, half-life text)` in upstream's progeny format (parsed by
    /// [`super::nuclide::parse_progeny_half_life`]).
    pub half_lives: Vec<(String, String)>,
}

impl ProgenyChains {
    /// Read two CSVs: `parent,daughter,yield` and `nuclide,half_life`. This is
    /// a flattened form of upstream's `dcf_corr.xlsx` (parent row followed by
    /// continuation rows naming daughter and yield).
    ///
    /// # Errors
    /// A missing column.
    pub fn from_csv(links_csv: &str, half_lives_csv: &str) -> Result<Self, String> {
        let (h, rows) = split_csv(links_csv);
        let (p, d, y) = (col(&h, "parent")?, col(&h, "daughter")?, col(&h, "yield")?);
        let links = rows
            .into_iter()
            .map(|r| (r[p].clone(), r[d].clone(), num(r.get(y))))
            .collect();
        let (h, rows) = split_csv(half_lives_csv);
        let (n, t) = (col(&h, "nuclide")?, col(&h, "half_life")?);
        let half_lives = rows
            .into_iter()
            .map(|r| (r[n].clone(), r[t].clone()))
            .collect();
        Ok(Self { links, half_lives })
    }

    /// Upstream's `find_progeny_name_and_yield_f`: the daughters of `parent`
    /// whose half-life is **at most** `ignore_half_life_s` (short-lived
    /// progeny, taken to be in equilibrium), with their yields, in file order.
    ///
    /// A daughter with no half-life entry is skipped, as upstream skips it.
    /// A half-life text upstream cannot parse raises there; here it skips.
    #[must_use]
    pub fn short_lived_daughters(
        &self,
        parent: &str,
        ignore_half_life_s: f64,
    ) -> Vec<(String, f64)> {
        self.links
            .iter()
            .filter(|(p, _, _)| p == parent)
            .filter_map(|(_, d, y)| {
                let text = &self.half_lives.iter().find(|(n, _)| n == d)?.1;
                let x = super::nuclide::parse_progeny_half_life(text)?;
                (x <= ignore_half_life_s).then(|| (d.clone(), *y))
            })
            .collect()
    }
}

/// Whether to add short-lived progeny to an external coefficient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProgenyCorrection {
    /// Parent only (upstream `consider_progeny: False`).
    ParentOnly,
    /// Add `yield x coefficient` for each daughter with half-life at most this
    /// many seconds (upstream `consider_progeny: True`, `ignore_half_life`,
    /// default 1800 s).
    IncludeShortLived {
        /// Half-life threshold, s.
        ignore_half_life_s: f64,
    },
}

/// A parent coefficient with and without the progeny correction (upstream
/// returns this pair; the pathways pick one according to the same flag).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExternalDcfPair {
    /// Parent plus short-lived progeny (equal to `uncorrected` for
    /// [`ProgenyCorrection::ParentOnly`]).
    pub corrected: f64,
    /// Parent only.
    pub uncorrected: f64,
}

impl ExternalDcfPair {
    /// The coefficient a pathway uses: `corrected` when progeny are included.
    #[must_use]
    pub fn selected(self, progeny: ProgenyCorrection) -> f64 {
        match progeny {
            ProgenyCorrection::ParentOnly => self.uncorrected,
            ProgenyCorrection::IncludeShortLived { .. } => self.corrected,
        }
    }
}

/// Upstream's `dcf_list_ecerman_*_include_progeny` for one nuclide.
#[must_use]
pub fn external_dcf(
    table: &ExternalDcfTable,
    chains: &ProgenyChains,
    nuclide: &str,
    age: AgeBracket,
    progeny: ProgenyCorrection,
) -> ExternalDcfPair {
    let parent = table.lookup_exact(nuclide, age);
    let mut corrected = parent;
    if let ProgenyCorrection::IncludeShortLived { ignore_half_life_s } = progeny {
        for (d, y) in chains.short_lived_daughters(nuclide, ignore_half_life_s) {
            corrected += table.lookup_contains(&d, age) * y;
        }
    }
    ExternalDcfPair {
        corrected,
        uncorrected: parent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_bracket_boundaries_follow_upstream() {
        assert_eq!(AgeBracket::from_age_years(1.0), Some(AgeBracket::Infant));
        assert_eq!(AgeBracket::from_age_years(1.5), Some(AgeBracket::OneToTwo));
        assert_eq!(
            AgeBracket::from_age_years(7.0),
            Some(AgeBracket::TwoToSeven)
        );
        assert_eq!(
            AgeBracket::from_age_years(17.0),
            Some(AgeBracket::TwelveToSeventeen)
        );
        assert_eq!(AgeBracket::from_age_years(17.5), Some(AgeBracket::Adult));
        assert_eq!(AgeBracket::from_age_years(f64::NAN), None);
    }

    #[test]
    fn substring_daughter_lookup_takes_the_larger_match() {
        let t = ExternalDcfTable::from_csv("Nuclide,Newborn,1-yr-old,5-yr-old,10-yr-old,15-yr-old,Adult\nY-90,1,1,1,1,1,1\nY-90m,2,2,2,2,2,2\n").unwrap();
        assert_eq!(t.lookup_exact("Y-90", AgeBracket::Adult), 1.0);
        assert_eq!(t.lookup_contains("Y-90", AgeBracket::Adult), 2.0);
    }
}
