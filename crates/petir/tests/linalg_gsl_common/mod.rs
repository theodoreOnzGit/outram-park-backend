// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! Shared reader and scorer for the `*-gsl-2.8-reference.txt` files that the
//! linear-algebra drivers in `reference-data/gsl/` write (gh:#993).
//!
//! Every driver writes the same line shape: `CASE <name> <dims...>`, then
//! `<TAG> <index...> <value>` lines, then `END <name>`. The inputs are written
//! too (tags `A`, `B`), so a replaying test rebuilds them from the file and
//! never has to keep an input formula in step with the C by hand.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use petir::linalg::Matrix;

/// One `CASE ... END` block.
#[derive(Default, Debug)]
pub struct Case {
    /// Integers after the case name.
    pub dims: Vec<usize>,
    /// Tag -> rows of (indices, value), in file order.
    pub rows: BTreeMap<String, Vec<(Vec<usize>, f64)>>,
}

impl Case {
    /// Every `(indices, value)` under `tag`, or an empty slice.
    pub fn tag(&self, tag: &str) -> &[(Vec<usize>, f64)] {
        self.rows.get(tag).map(Vec::as_slice).unwrap_or(&[])
    }

    /// The single scalar under `tag` (e.g. `STATUS`), if present.
    pub fn scalar(&self, tag: &str) -> Option<f64> {
        self.tag(tag).first().map(|r| r.1)
    }

    /// A vector recorded as `<tag> <i> <v>`.
    pub fn vector(&self, tag: &str) -> Vec<f64> {
        let rows = self.tag(tag);
        let mut v = vec![0.0; rows.len()];
        for (idx, val) in rows {
            v[idx[0]] = *val;
        }
        v
    }

    /// A matrix recorded as `<tag> <i> <j> <v>`, sized by its largest indices.
    pub fn matrix(&self, tag: &str) -> Option<Matrix> {
        let rows = self.tag(tag);
        let m = rows.iter().map(|r| r.0[0]).max()? + 1;
        let n = rows.iter().map(|r| r.0[1]).max()? + 1;
        let mut out = Matrix::zeros(m, n).ok()?;
        for (idx, val) in rows {
            out.set(idx[0], idx[1], *val);
        }
        Some(out)
    }
}

/// Load `reference-data/gsl/<file>`, or `None` if it is absent.
pub fn load(file: &str) -> Option<Vec<(String, Case)>> {
    load_in("gsl", file)
}

/// Load `reference-data/<dir>/<file>` (same line format), or `None`.
pub fn load_in(dir: &str, file: &str) -> Option<Vec<(String, Case)>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data")
        .join(dir)
        .join(file);
    let text = fs::read_to_string(path).ok()?;
    let mut out: Vec<(String, Case)> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f.first().copied() {
            None | Some("END") => {}
            Some("CASE") => {
                let dims = f[2..].iter().filter_map(|v| v.parse().ok()).collect();
                out.push((
                    f[1].to_string(),
                    Case {
                        dims,
                        ..Default::default()
                    },
                ));
            }
            Some(tag) => {
                let Some((_, case)) = out.last_mut() else {
                    continue;
                };
                let n = f.len();
                let Ok(v) = f[n - 1].parse::<f64>() else {
                    continue;
                };
                let idx = f[1..n - 1].iter().filter_map(|s| s.parse().ok()).collect();
                case.rows.entry(tag.to_string()).or_default().push((idx, v));
            }
        }
    }
    Some(out)
}

/// Relative difference, falling back to absolute below `1e-300`.
pub fn rel_diff(a: f64, b: f64) -> f64 {
    if a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()) {
        return 0.0;
    }
    let scale = a.abs().max(b.abs());
    if scale < 1e-300 {
        (a - b).abs()
    } else {
        (a - b).abs() / scale
    }
}

/// Running bit-identity count and worst relative difference.
#[derive(Default, Debug)]
pub struct Score {
    pub compared: usize,
    pub identical: usize,
    pub worst: f64,
    pub worst_where: String,
}

impl Score {
    /// Score one value.
    pub fn add(&mut self, what: impl FnOnce() -> String, gsl: f64, petir: f64) {
        self.compared += 1;
        if gsl.to_bits() == petir.to_bits() {
            self.identical += 1;
        }
        let d = rel_diff(gsl, petir);
        if d > self.worst || d.is_nan() {
            self.worst = d;
            self.worst_where = format!("{} gsl={gsl:e} petir={petir:e}", what());
        }
    }

    /// One-line summary for the test log.
    pub fn summary(&self, label: &str) -> String {
        format!(
            "{label}: {} values, {} bit-identical ({:.1} %), worst relative difference {:e} at {}",
            self.compared,
            self.identical,
            100.0 * self.identical as f64 / self.compared.max(1) as f64,
            self.worst,
            self.worst_where
        )
    }
}
