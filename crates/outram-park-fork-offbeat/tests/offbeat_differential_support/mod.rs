// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
//! Shared helpers for the OFFBEAT differential tests (gh:#1393): fixture
//! parsing, ulp distance and per-model statistics.

use std::collections::BTreeMap;

/// One fixture row: model name, numeric input columns, the upstream value.
pub struct Row {
    pub name: String,
    pub col: Vec<f64>,
    pub reference: f64,
}

/// Parse a whitespace-separated fixture: `name c0 c1 ... cN-1 value`.
pub fn parse_rows(text: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let name = match parts.next() {
            Some(n) => n.to_string(),
            None => continue,
        };
        let nums: Vec<f64> = parts.map(|p| p.parse::<f64>().expect("fixture number")).collect();
        let reference = *nums.last().expect("value column");
        let col = nums[..nums.len() - 1].to_vec();
        rows.push(Row { name, col, reference });
    }
    rows
}

/// Distance in units in the last place between two doubles; `u64::MAX` for
/// NaN mismatches or sign changes.
pub fn ulp_distance(a: f64, b: f64) -> u64 {
    if a.to_bits() == b.to_bits() {
        return 0;
    }
    if a.is_nan() || b.is_nan() {
        return if a.is_nan() && b.is_nan() { 0 } else { u64::MAX };
    }
    if (a < 0.0) != (b < 0.0) {
        return if a == 0.0 && b == 0.0 { 0 } else { u64::MAX };
    }
    a.to_bits().abs_diff(b.to_bits())
}

/// Per-model statistics.
#[derive(Default, Clone)]
pub struct Stats {
    pub compared: usize,
    pub skipped_outside_window: usize,
    pub bit_identical: usize,
    pub within_1_ulp: usize,
    pub within_4_ulp: usize,
    pub worst_rel: f64,
    pub worst_ulp: u64,
    pub worst_row: String,
}

impl Stats {
    /// Record one comparison of the port value against the upstream value.
    pub fn add(&mut self, port: f64, upstream: f64, row_desc: &str) {
        self.compared += 1;
        let u = ulp_distance(port, upstream);
        if u == 0 {
            self.bit_identical += 1;
        }
        if u <= 1 {
            self.within_1_ulp += 1;
        }
        if u <= 4 {
            self.within_4_ulp += 1;
        }
        let rel = if upstream == 0.0 {
            (port - upstream).abs()
        } else {
            ((port - upstream) / upstream).abs()
        };
        let rel = if rel.is_nan() { f64::INFINITY } else { rel };
        if rel > self.worst_rel || (u != 0 && self.worst_row.is_empty()) {
            self.worst_rel = rel.max(self.worst_rel);
            self.worst_row = row_desc.to_string();
        }
        self.worst_ulp = self.worst_ulp.max(u);
    }
}

/// Print a table and return the worst relative difference over all models.
pub fn report(stats: &BTreeMap<String, Stats>) -> f64 {
    println!(
        "{:<24} {:>7} {:>7} {:>8} {:>8} {:>8} {:>11} {:>8}",
        "model", "n", "skipped", "bit-id", "<=1ulp", "<=4ulp", "worst rel", "worstulp"
    );
    let mut worst = 0.0_f64;
    for (name, s) in stats {
        println!(
            "{:<24} {:>7} {:>7} {:>8} {:>8} {:>8} {:>11.3e} {:>8}   {}",
            name, s.compared, s.skipped_outside_window, s.bit_identical, s.within_1_ulp,
            s.within_4_ulp, s.worst_rel, s.worst_ulp, s.worst_row
        );
        worst = worst.max(s.worst_rel);
    }
    let n: usize = stats.values().map(|s| s.compared).sum();
    let b: usize = stats.values().map(|s| s.bit_identical).sum();
    println!(
        "TOTAL compared {n}, bit-identical {b} ({:.2} %), worst rel {worst:.3e}",
        100.0 * b as f64 / n.max(1) as f64
    );
    worst
}

/// Gates G1 and G2 of PREDICTIONS_round2.md: `|port - up| <= 8 eps * scale`
/// and relative < 1e-14 wherever `|up| >= 0.2 * scale`.
#[derive(Default, Clone)]
pub struct Gates {
    pub g1_fail: usize,
    pub g2_checked: usize,
    pub g2_fail: usize,
    pub worst_cond_rel: f64,
    pub worst_g1_ratio: f64,
}

impl Gates {
    /// Record one comparison.
    pub fn add(&mut self, port: f64, up: f64, scale: f64) {
        let d = (port - up).abs();
        if scale > 0.0 {
            self.worst_g1_ratio = self.worst_g1_ratio.max(d / (8.0 * f64::EPSILON * scale));
        }
        if d > 8.0 * f64::EPSILON * scale {
            self.g1_fail += 1;
        }
        if up != 0.0 && up.abs() >= 0.2 * scale {
            self.g2_checked += 1;
            if d / up.abs() >= 1e-14 {
                self.g2_fail += 1;
            }
        } else if up != 0.0 {
            self.worst_cond_rel = self.worst_cond_rel.max(d / up.abs());
        }
    }
}
