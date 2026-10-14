//! **Two ACE tables, compared block by block and word for word** (GitHub
//! #863).
//!
//! The comparison behind `examples/ace_blocks_vs_reference.rs`, moved into the
//! library so that example, the tests and the nuclear data demo's comparator
//! rung share one implementation (maintainer, 2026-10-14). Typically the first
//! table is this port's and the second NJOY2016's for the same evaluation and
//! deck, but nothing here assumes which is which.
//!
//! # What is compared
//!
//! - **Header:** ZAID, atomic weight ratio and temperature (`kT`). The date,
//!   comment and material strings are shown, not judged: they legitimately
//!   differ between two codes.
//! - **NXS and JXS**, entry by entry.
//! - **Each block, word for word**, from its JXS locator to the next block
//!   present in the same table. The block names come from the table's class:
//!   continuous-energy neutron (`c`) and thermal S(α,β) (`t`). Other classes
//!   are compared as a whole table only.
//! - **The whole XSS array, word for word**, so no block can differ without
//!   this row failing.
//!
//! "Same" means the two parsed values are equal. Read from Type-1 text, that
//! is equality of the printed value (12 significant figures); there is no
//! tolerance anywhere.

use super::read::{AceClass, RawAceTable};

/// What a row of the comparison found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Equal in every word.
    Same,
    /// Present in both, and some word differs (or the lengths differ).
    Different,
    /// Absent in both tables: nothing to compare, nothing wrong.
    AbsentInBoth,
    /// Present only in the second table.
    MissingInFirst,
    /// Present only in the first table.
    MissingInSecond,
}

impl Verdict {
    /// The verdict in words, as the report prints it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Same => "SAME",
            Self::Different => "DIFFERENT",
            Self::AbsentInBoth => "absent in both",
            Self::MissingInFirst => "MISSING in the first table",
            Self::MissingInSecond => "MISSING in the second table",
        }
    }

    /// Whether this row is acceptable for a claim of identity.
    pub fn agrees(self) -> bool {
        matches!(self, Self::Same | Self::AbsentInBoth)
    }
}

/// The first word that differs within a span.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FirstDifference {
    /// Word number within the span, 1-based.
    pub word: usize,
    /// The same word's position in the first table's XSS, 1-based (the
    /// numbering JXS locators use).
    pub xss_index: usize,
    /// The value in the first table, if the span is that long.
    pub first: Option<f64>,
    /// The value in the second table, if the span is that long.
    pub second: Option<f64>,
}

/// One block (or the whole XSS) compared.
#[derive(Debug, Clone, PartialEq)]
pub struct SpanComparison {
    /// The block's ACE name (`ESZ`, `ITXE`, …), or `XSS` for the whole array.
    pub name: &'static str,
    pub verdict: Verdict,
    /// Words in the first and second table's span (0 when absent).
    pub words: (usize, usize),
    /// Positions, over the common length, where the values differ.
    pub n_different: usize,
    pub first_difference: Option<FirstDifference>,
}

/// One header field compared.
#[derive(Debug, Clone, PartialEq)]
pub struct HeaderField {
    pub name: &'static str,
    pub first: String,
    pub second: String,
    /// `None` for fields that are shown and not judged (date, comment).
    pub same: Option<bool>,
}

/// An integer array (NXS or JXS) compared entry by entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayComparison {
    pub name: &'static str,
    /// The 1-based entries that differ, with both values.
    pub differences: Vec<(usize, i32, i32)>,
}

/// The whole comparison of two tables.
#[derive(Debug, Clone, PartialEq)]
pub struct AceComparison {
    pub header: Vec<HeaderField>,
    pub nxs: ArrayComparison,
    pub jxs: ArrayComparison,
    /// Per block, in JXS order; empty for a class with no block table here.
    pub blocks: Vec<SpanComparison>,
    pub whole_xss: SpanComparison,
}

impl AceComparison {
    /// True when the two tables agree in every judged header field, every
    /// NXS and JXS entry, and every XSS word.
    pub fn identical(&self) -> bool {
        self.header.iter().all(|h| h.same != Some(false))
            && self.nxs.differences.is_empty()
            && self.jxs.differences.is_empty()
            && self.whole_xss.verdict == Verdict::Same
            && self.blocks.iter().all(|b| b.verdict.agrees())
    }

    /// The comparison as plain lines, the form the example prints and the demo
    /// shows.
    pub fn report_lines(&self) -> Vec<String> {
        let mut v = Vec::new();
        for h in &self.header {
            let tag = match h.same {
                Some(true) => "SAME",
                Some(false) => "DIFFERENT",
                None => "shown only",
            };
            v.push(format!("{:<10} {:<10} first {:<24} second {}", h.name, tag, h.first, h.second));
        }
        for a in [&self.nxs, &self.jxs] {
            if a.differences.is_empty() {
                v.push(format!("{:<10} SAME", a.name));
            } else {
                let d: Vec<String> = a.differences.iter().map(|(i, x, y)| format!("{}({i}): {x} vs {y}", a.name)).collect();
                v.push(format!("{:<10} DIFFERENT: {}", a.name, d.join(", ")));
            }
        }
        for s in self.blocks.iter().chain(std::iter::once(&self.whole_xss)) {
            v.push(span_line(s));
        }
        v.push(if self.identical() {
            "VERDICT: the two tables are identical in every compared word".to_string()
        } else {
            "VERDICT: the two tables differ (see the rows above)".to_string()
        });
        v
    }
}

/// One row of the report.
pub fn span_line(s: &SpanComparison) -> String {
    let name = if s.name == "XSS" { "XSS, whole table" } else { s.name };
    match s.verdict {
        Verdict::Same => format!("{name:<16} SAME ({} words, every word)", s.words.0),
        Verdict::Different => {
            let mut t = format!("{name:<16} DIFFERENT (first {} words, second {}; {} differ", s.words.0, s.words.1, s.n_different);
            if let Some(d) = s.first_difference {
                let f = |x: Option<f64>| x.map_or("(none)".to_string(), |x| format!("{x:e}"));
                t.push_str(&format!(
                    "; first at word {} (XSS {}): {} vs {}",
                    d.word,
                    d.xss_index,
                    f(d.first),
                    f(d.second)
                ));
            }
            t.push(')');
            t
        }
        v => format!("{name:<16} {}", v.label()),
    }
}

/// The continuous-energy neutron (`c`) blocks, in JXS order, with their
/// 0-based JXS index. `FIS` (JXS 21) points into SIG and `END` (JXS 22) is the
/// last word, so neither starts a block.
pub const CE_BLOCKS: &[(&str, usize)] = &[
    ("ESZ", 0),
    ("NU", 1),
    ("MTR", 2),
    ("LQR", 3),
    ("TYR", 4),
    ("LSIG", 5),
    ("SIG", 6),
    ("LAND", 7),
    ("AND", 8),
    ("LDLW", 9),
    ("DLW", 10),
    ("GPD", 11),
    ("MTRP", 12),
    ("LSIGP", 13),
    ("SIGP", 14),
    ("LANDP", 15),
    ("ANDP", 16),
    ("LDLWP", 17),
    ("DLWP", 18),
    ("YP", 19),
    ("LUNR", 22),
    ("DNU", 23),
    ("BDD", 24),
    ("DNEDL", 25),
    ("DNED", 26),
];

/// The thermal S(α,β) (`t`) blocks, in JXS order (`acer::thermal::jxs`).
pub const THERMAL_BLOCKS: &[(&str, usize)] = &[
    ("ITIE", 0),
    ("ITIX", 1),
    ("ITXE", 2),
    ("ITCE", 3),
    ("ITCX", 4),
    ("ITCA", 5),
    ("ITCEI", 6),
    ("ITCXI", 7),
    ("ITCAI", 8),
];

/// The block table for a class, or an empty one when this module does not
/// name its blocks (the whole XSS is still compared).
pub fn blocks_for(class: AceClass) -> &'static [(&'static str, usize)] {
    match class {
        AceClass::ContinuousNeutron => CE_BLOCKS,
        AceClass::Thermal => THERMAL_BLOCKS,
        _ => &[],
    }
}

/// Block `j`'s words in `t`, as the 1-based XSS range `[start, end)`: from its
/// locator to the next locator of any block in `layout` that lies beyond it,
/// else to the end of XSS. `None` when the block is absent (locator 0) or the
/// locator is outside XSS.
fn span(t: &RawAceTable, layout: &[(&str, usize)], j: usize) -> Option<(usize, usize)> {
    let a = t.jxs[j];
    if a <= 0 || a as usize > t.xss.len() {
        return None;
    }
    let end = layout
        .iter()
        .map(|&(_, k)| t.jxs[k])
        .filter(|&l| l > a)
        .min()
        .map_or(t.xss.len() + 1, |l| (l as usize).min(t.xss.len() + 1));
    Some((a as usize, end))
}

/// Compare the 1-based XSS range `first` of table `a` with `second` of `b`.
fn compare_spans(
    name: &'static str,
    ta: &RawAceTable,
    first: Option<(usize, usize)>,
    tb: &RawAceTable,
    second: Option<(usize, usize)>,
) -> SpanComparison {
    let fa = first.map(|(s, e)| (s, &ta.xss[s - 1..e - 1]));
    let fb = second.map(|(s, e)| (s, &tb.xss[s - 1..e - 1]));
    let (verdict, words, n_different, first_difference) = match (fa, fb) {
        (None, None) => (Verdict::AbsentInBoth, (0, 0), 0, None),
        (None, Some((_, b))) => (Verdict::MissingInFirst, (0, b.len()), 0, None),
        (Some((_, a)), None) => (Verdict::MissingInSecond, (a.len(), 0), 0, None),
        (Some((start, a)), Some((_, b))) => {
            let n = a.iter().zip(b).filter(|(x, y)| x != y).count();
            let first_at = a.iter().zip(b).position(|(x, y)| x != y).or((a.len() != b.len()).then(|| a.len().min(b.len())));
            let d = first_at.map(|i| FirstDifference {
                word: i + 1,
                xss_index: start + i,
                first: a.get(i).copied(),
                second: b.get(i).copied(),
            });
            let v = if d.is_none() { Verdict::Same } else { Verdict::Different };
            (v, (a.len(), b.len()), n, d)
        }
    };
    SpanComparison { name, verdict, words, n_different, first_difference }
}

fn compare_array(name: &'static str, a: &[i32], b: &[i32]) -> ArrayComparison {
    ArrayComparison {
        name,
        differences: a.iter().zip(b).enumerate().filter(|(_, (x, y))| x != y).map(|(i, (x, y))| (i + 1, *x, *y)).collect(),
    }
}

/// Compare two ACE tables. The block names follow the first table's class;
/// when the classes differ, the header row for the ZAID says so and only the
/// whole XSS is compared.
pub fn compare(first: &RawAceTable, second: &RawAceTable) -> AceComparison {
    let (h1, h2) = (&first.header, &second.header);
    let same_class = h1.class == h2.class;
    let header = vec![
        HeaderField { name: "ZAID", first: h1.zaid.clone(), second: h2.zaid.clone(), same: Some(h1.zaid == h2.zaid) },
        HeaderField { name: "AWR", first: format!("{:.6}", h1.awr), second: format!("{:.6}", h2.awr), same: Some(h1.awr == h2.awr) },
        HeaderField {
            name: "kT (MeV)",
            first: format!("{:e}", h1.kt_mev),
            second: format!("{:e}", h2.kt_mev),
            same: Some(h1.kt_mev == h2.kt_mev),
        },
        HeaderField { name: "date", first: h1.date.clone(), second: h2.date.clone(), same: None },
        HeaderField { name: "comment", first: h1.comment.trim().to_string(), second: h2.comment.trim().to_string(), same: None },
    ];
    let layout = if same_class { blocks_for(h1.class) } else { &[] };
    let blocks = layout
        .iter()
        .map(|&(name, j)| compare_spans(name, first, span(first, layout, j), second, span(second, layout, j)))
        .collect();
    let whole = |t: &RawAceTable| (!t.xss.is_empty()).then_some((1usize, t.xss.len() + 1));
    AceComparison {
        header,
        nxs: compare_array("NXS", &first.nxs, &second.nxs),
        jxs: compare_array("JXS", &first.jxs, &second.jxs),
        blocks,
        whole_xss: compare_spans("XSS", first, whole(first), second, whole(second)),
    }
}
