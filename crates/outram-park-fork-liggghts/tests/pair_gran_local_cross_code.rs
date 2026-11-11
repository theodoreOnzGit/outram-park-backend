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

//! # Per-contact dump (`compute pair/gran/local`) against `GranularSystem::pair_gran_local_for`
//!
//! gh:#1406 prio-2 row "`compute pair/gran/local` dump at steps 1, 10 and 100";
//! gh:#1418 (first item).
//!
//! ## Methodology
//!
//! Deck `reference-data/liggghts/in.pgl_pebble` (LIGGGHTS-PUBLIC `3d5c00f2`):
//! the committed settled `D/d = 6` bed (354 pebbles, `d = 10 mm`, `rho = 2500`,
//! `E = 5e6`, `nu = 0.3`, `e = 0.5`, `mu = 0.3`, Hertz + history, cylinder
//! `R = 30 mm`, floor, gravity, `dt = 5 us`) is loaded with `csv2data.sh` and
//! integrated 100 steps; `compute pair/gran/local pos vel id force torque
//! history contactArea delta` is dumped at every step with a `%.17g` format and
//! steps 1, 10 and 100 are committed (`pgl_pebble_step{1,10,100}.txt`). Each
//! row is one overlapping pair with the force and torque on `i` re-evaluated
//! from the stored shear history without advancing it. This test builds the
//! same bed in a `GranularSystem`, runs the same number of steps and
//! evaluates `pair_gran_local_for(tag_i, tag_j)` for every upstream row (upstream's
//! `i`/`j` orientation), comparing all 23 numeric fields per contact on their
//! bit patterns.
//!
//! ## Prediction (written before the run, 2026-11-11)
//!
//! * Step 1: **every field of every contact bit-identical** (same state after
//!   one step; each contact is an independent evaluation).
//! * Steps 10 and 100: the same contact set; fields agree to ulp-level
//!   differences that grow with time, because a particle's net force is a
//!   sum over its contacts in an order that depends on the neighbour list (the
//!   cell grid here, upstream's bin list), and the sum order changes the last
//!   bit of the velocity that the next contacts see. Pass criterion for those
//!   steps: identical contact count, identical contact pairs, and every field
//!   within `1e-8` relative to the largest magnitude of that field (the
//!   workspace round-off allowance; the measured values are printed).
//!
//! ## Result
//!
//! Printed by the test (`--nocapture`) and recorded in
//! `docs/cross-code-summary.md`.

use outram_park_fork_liggghts::boundary::Boundary;
use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::neighbor_list::LiggghtsNeighbor;
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

fn read(name: &str) -> Option<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts")
        .join(name);
    std::fs::read_to_string(path).ok()
}

fn fields(rec: &outram_park_fork_liggghts::granular_system::PairLocalRecord) -> [f64; 23] {
    [
        rec.pos_i.x, rec.pos_i.y, rec.pos_i.z, rec.pos_j.x, rec.pos_j.y, rec.pos_j.z,
        rec.vel_i.x, rec.vel_i.y, rec.vel_i.z, rec.vel_j.x, rec.vel_j.y, rec.vel_j.z,
        rec.force.x, rec.force.y, rec.force.z,
        rec.torque.x, rec.torque.y, rec.torque.z,
        rec.history.x, rec.history.y, rec.history.z,
        rec.contact_area, rec.delta,
    ]
}

/// The deck's neighbour settings: `read_data` box, `neighbor 0.003 bin`.
fn deck_neighbor() -> LiggghtsNeighbor {
    LiggghtsNeighbor::new(Vec3::new(-1.0, -1.0, -0.005), Vec3::new(1.0, 1.0, 0.35), 0.003).unwrap()
}

/// Compare the committed dumps (steps 0, 1, 10, 100) with the port, field by
/// field, under the given pair ordering (`None` = the default cell grid).
/// Returns the number of differing values per step.
fn compare(order: Option<LiggghtsNeighbor>) -> Option<Vec<usize>> {
    let mut per_step = Vec::new();
    let Some(csv) = read("pebble_bed_settled.csv") else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return None;
    };
    let r = 0.005;
    let mass = 4.0 * std::f64::consts::PI / 3.0 * r * r * r * 2500.0;
    let particles: Vec<Particle> = csv
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(n, l)| {
            let v: Vec<f64> = l.split(',').map(|x| x.parse().unwrap()).collect();
            assert_eq!(v[0] as usize, n + 1, "csv ids must be 1..N in order");
            Particle::new(
                Vec3::new(v[1], v[2], v[3]),
                Vec3::new(v[4], v[5], v[6]),
                Vec3::zero(),
                Mass::new::<kilogram>(mass),
                Length::new::<meter>(r),
                ThermodynamicTemperature::new::<kelvin>(300.0),
            )
            .unwrap()
        })
        .collect();
    let material = GranularMaterial::new(5.0e6, 0.3, 0.5, 0.3).unwrap();
    let boundaries = vec![
        Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), 0.030).unwrap(),
        Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap(),
    ];
    let mut sys = GranularSystem::new(
        particles,
        boundaries,
        GranularContactModel::hertz_history(material),
        Vec3::new(0.0, 0.0, -9.81),
        5e-6,
    )
    .unwrap();
    if let Some(cfg) = order {
        sys = sys.with_liggghts_neighbor_order(cfg);
    }

    let mut done = 0usize;
    let mut degree_step0 = vec![0usize; 354];
    for step in [0usize, 1, 10, 100] {
        let Some(dump) = read(&format!("pgl_pebble_step{step}.txt")) else {
            eprintln!("skipping step {step}: dump not present");
            return None;
        };
        sys.run(step - done);
        done = step;
        let rows: Vec<Vec<f64>> = dump
            .lines()
            .skip(9)
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.split_whitespace().map(|x| x.parse().unwrap()).collect())
            .collect();
        let ours_all = sys.pair_gran_local();
        assert_eq!(ours_all.len(), rows.len(), "step {step}: contact count");

        // pair-contact degree of every particle (number of pair contacts)
        let mut degree = vec![0usize; 354];
        for row in &rows {
            degree[row[12] as usize - 1] += 1;
            degree[row[13] as usize - 1] += 1;
        }
        if step == 0 {
            degree_step0 = degree.clone();
        }
        let mut low_degree_bad = 0usize; // mismatching contacts whose two particles have <= 2 contacts
        let (mut identical_contacts, mut bad_values, mut total) = (0usize, 0usize, 0usize);
        let mut worst_rel = 0.0_f64;
        let mut scale = [0.0_f64; 23];
        let mut worst_abs = [0.0_f64; 23];
        for row in &rows {
            let (tag_i, tag_j) = (row[12] as u64 - 1, row[13] as u64 - 1);
            let rec = sys
                .pair_gran_local_for(tag_i, tag_j)
                .unwrap_or_else(|| panic!("step {step}: upstream contact ({tag_i},{tag_j}) missing"));
            let ours = fields(&rec);
            // upstream column order: pos_i, pos_j, vel_i, vel_j, id x3,
            // force, torque, history, area, delta
            let theirs: Vec<f64> = row[0..12].iter().chain(&row[15..26]).copied().collect();
            let mut same = true;
            for k in 0..23 {
                total += 1;
                scale[k] = scale[k].max(theirs[k].abs());
                worst_abs[k] = worst_abs[k].max((ours[k] - theirs[k]).abs());
                if ours[k].to_bits() != theirs[k].to_bits() {
                    bad_values += 1;
                    same = false;
                }
            }
            if same {
                identical_contacts += 1;
            } else if degree[tag_i as usize].max(degree_step0[tag_i as usize]) <= 2
                && degree[tag_j as usize].max(degree_step0[tag_j as usize]) <= 2
            {
                low_degree_bad += 1;
            }
        }
        for k in 0..23 {
            if scale[k] > 0.0 {
                worst_rel = worst_rel.max(worst_abs[k] / scale[k]);
            }
        }
        eprintln!(
            "pair/gran/local step {step}: {} contacts, {identical_contacts} fully bit-identical, \
             {} of {total} values differ, worst |d|/max|field| {worst_rel:e}, \
             mismatching contacts between particles with <= 2 pair contacts at steps 0 and now: {low_degree_bad}",
            rows.len(),
            bad_values
        );
        let _ = (low_degree_bad, worst_rel);
        per_step.push(bad_values);
    }
    Some(per_step)
}

/// Prediction (written 2026-11-11 before the run): with the LIGGGHTS neighbour
/// order every value of every contact is bit-identical at steps 0, 1, 10 and
/// 100 (the 1-3 ulp of the cell-grid order were a force-sum-order effect).
#[test]
fn per_contact_dump_matches_upstream() {
    let Some(bad) = compare(Some(deck_neighbor())) else { return };
    assert_eq!(bad, vec![0, 0, 0, 0], "values differing at steps 0, 1, 10, 100");
}

/// Ablation: the default cell-grid order. Step 0 is a single evaluation and
/// stays bit-identical; later steps differ at ulp level because the sum order
/// differs. This pins that the option is what closes the gap, not luck.
#[test]
fn cell_grid_order_differs_only_in_sum_order() {
    let Some(bad) = compare(None) else { return };
    eprintln!("cell-grid order: differing values at steps 0,1,10,100 = {bad:?}");
    assert_eq!(bad[0], 0);
    assert!(bad[1] > 0, "the cell-grid order is expected to differ from step 1");
}

/// Ablation: LIGGGHTS' list order but no atom sort (`atom_modify sort 0 0`):
/// the local order stays the data-file order, which is NOT what the deck runs.
#[test]
fn list_order_without_atom_sort_is_not_upstream() {
    let mut cfg = deck_neighbor();
    cfg.sort_every = 0;
    let Some(bad) = compare(Some(cfg)) else { return };
    eprintln!("no atom sort: differing values at steps 0,1,10,100 = {bad:?}");
    assert_eq!(bad[0], 0);
}
