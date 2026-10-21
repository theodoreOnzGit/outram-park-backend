// SPDX-License-Identifier: GPL-3.0-only
//! The minimal CSV reader the caller-supplied tables go through: no quoted
//! fields, `#` comment lines and blank lines skipped, fields trimmed. Blank or
//! unparsable numbers read as NaN, which is how pandas reads an empty cell.

/// Split a simple CSV into a header and rows.
pub(crate) fn split_csv(text: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let mut lines = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'));
    let header = lines
        .next()
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();
    let rows = lines
        .map(|l| l.split(',').map(|s| s.trim().to_string()).collect())
        .collect();
    (header, rows)
}

/// Index of a named column.
pub(crate) fn col(header: &[String], name: &str) -> Result<usize, String> {
    header
        .iter()
        .position(|h| h == name)
        .ok_or_else(|| format!("missing column `{name}`"))
}

/// A field as `f64`, NaN if blank or unparsable.
pub(crate) fn num(field: Option<&String>) -> f64 {
    field
        .and_then(|f| f.parse::<f64>().ok())
        .unwrap_or(f64::NAN)
}
