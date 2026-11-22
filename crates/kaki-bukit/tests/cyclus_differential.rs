// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! Differential comparison of `kaki-bukit` against a CYCLUS run (gh:#1431).
//!
//! # Methodology
//!
//! Five scenarios (`verification_and_validation/cyclus_differential_gh1431/`,
//! inputs under `inputs/`, predictions in `PREDICTIONS.md` committed before
//! any run) are run in CYCLUS 1.6.0 (conda-forge `cyclus 1.6.0
//! py315he15e396_10` with `cycamore 1.6.0`) and in this crate. CYCLUS's
//! `Transactions` table is dumped to `cyclus_output/<case>.csv`
//! (`time,sender,receiver,commodity,qty`, quantity at 17 significant digits so
//! binary64 round-trips) and compared with [`Simulation::transactions`]: same
//! time step, same sender and receiver prototype, quantity bit for bit.
//!
//! Pass criterion: same row count and every row bit-identical.
//!
//! The rows are compared after sorting both tables by (time, sender,
//! receiver). CYCLUS lists a step's rows in the iteration order of
//! `std::map<std::pair<Trader*, Trader*>, ...>` (`trade_executor.h`,
//! `RecordTrades` over `all_trades`), i.e. by heap address of the two
//! facility objects, which carries no meaning; the count of rows that also
//! agree in emitted order is printed but not asserted.
//!
//! # Results (2026-11-13)
//!
//! See `README.md` beside the data for the table and interpretation.

use kaki_bukit::agent::AgentRole;
use kaki_bukit::agents::{AgentKind, Simulation, Sink, Source};
use kaki_bukit::composition::AtomicMasses;
use kaki_bukit::context::Context;
use kaki_bukit::sim::SimInfo;

/// One table row: time, sender, receiver, quantity.
type Row = (i64, String, String, f64);

fn parse(csv: &str) -> Vec<Row> {
    csv.lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            (
                f[0].parse().unwrap(),
                f[1].to_string(),
                f[2].to_string(),
                f[4].parse().unwrap(),
            )
        })
        .collect()
}

fn build(facilities: Vec<(&str, AgentKind)>) -> Simulation {
    let ctx = Context::new(SimInfo::new(24)).unwrap();
    let mut sim = Simulation::new(ctx, AtomicMasses::MassNumber);
    let r = sim
        .add_agent(
            AgentRole::Region,
            "R",
            ":agents:NullRegion",
            None,
            AgentKind::Inert,
        )
        .unwrap();
    let i = sim
        .add_agent(
            AgentRole::Institution,
            "I",
            ":agents:NullInst",
            Some(r),
            AgentKind::Inert,
        )
        .unwrap();
    for (name, kind) in facilities {
        let spec = match kind {
            AgentKind::Source(_) => ":cycamore:Source",
            _ => ":cycamore:Sink",
        };
        sim.add_agent(AgentRole::Facility, name, spec, Some(i), kind)
            .unwrap();
    }
    sim.run().unwrap();
    sim
}

/// Port rows in the order the exchange returned them.
fn port_rows(sim: &Simulation) -> Vec<Row> {
    sim.transactions()
        .iter()
        .map(|t| {
            (
                t.time,
                sim.info(t.sender).unwrap().prototype.clone(),
                sim.info(t.receiver).unwrap().prototype.clone(),
                t.amt,
            )
        })
        .collect()
}

/// Result of comparing one case.
struct Outcome {
    port_rows: usize,
    cyclus_rows: usize,
    /// Rows bit-identical after sorting both sides by (time, sender,
    /// receiver).
    identical: usize,
    /// Worst relative quantity difference over rows with equal endpoints.
    worst_rel: f64,
    /// Rows bit-identical in the order the tables list them, unsorted.
    identical_in_emitted_order: usize,
}

fn same_row(p: &Row, c: &Row) -> bool {
    p.0 == c.0 && p.1 == c.1 && p.2 == c.2 && p.3.to_bits() == c.3.to_bits()
}

fn key(r: &Row) -> (i64, String, String) {
    (r.0, r.1.clone(), r.2.clone())
}

fn compare(port: &[Row], cy: &[Row]) -> Outcome {
    let identical_in_emitted_order = port
        .iter()
        .zip(cy)
        .filter(|(p, c)| same_row(p, c))
        .count();
    let mut p = port.to_vec();
    let mut c = cy.to_vec();
    p.sort_by_key(key);
    c.sort_by_key(key);
    let mut identical = 0;
    let mut worst_rel = 0.0_f64;
    for (a, b) in p.iter().zip(&c) {
        if same_row(a, b) {
            identical += 1;
        } else if key(a) == key(b) {
            worst_rel = worst_rel.max(((a.3 - b.3) / b.3).abs());
        }
    }
    Outcome {
        port_rows: port.len(),
        cyclus_rows: cy.len(),
        identical,
        worst_rel,
        identical_in_emitted_order,
    }
}

fn src(thr: f64, inv: Option<f64>) -> AgentKind {
    let s = Source::new("u").unwrap().with_throughput(thr).unwrap();
    AgentKind::Source(match inv {
        Some(v) => s.with_inventory(v).unwrap(),
        None => s,
    })
}

fn snk(cap: f64, maxinv: Option<f64>, pref: Option<f64>) -> AgentKind {
    let mut s = Sink::new(&["u"]).unwrap().with_capacity(cap).unwrap();
    if let Some(m) = maxinv {
        s = s.with_max_inventory(m).unwrap();
    }
    if let Some(p) = pref {
        s = s.with_preferences(&[p]).unwrap();
    }
    AgentKind::Sink(s)
}

/// Runs one case against its CYCLUS table and asserts the pass criterion.
fn run_case(csv: &str, facilities: Vec<(&str, AgentKind)>) {
    let sim = build(facilities);
    let o = compare(&port_rows(&sim), &parse(csv));
    std::eprintln!(
        "rows port={} cyclus={} bit-identical(sorted)={} worst_rel={:e} identical-in-emitted-order={}",
        o.port_rows,
        o.cyclus_rows,
        o.identical,
        o.worst_rel,
        o.identical_in_emitted_order
    );
    assert_eq!(o.port_rows, o.cyclus_rows, "row count differs");
    assert_eq!(
        o.identical, o.cyclus_rows,
        "not every row is bit-identical; worst rel {:e}",
        o.worst_rel
    );
}

#[test]
fn c1a_source_to_sink_inventory_runs_dry() {
    let csv = include_str!(
        "../verification_and_validation/cyclus_differential_gh1431/cyclus_output/c1a.csv"
    );
    run_case(
        csv,
        vec![("S", src(100.0, Some(1000.0))), ("K", snk(80.0, None, None))],
    );
}

#[test]
fn c1b_sink_inventory_fills_with_inexact_sums() {
    let csv = include_str!(
        "../verification_and_validation/cyclus_differential_gh1431/cyclus_output/c1b.csv"
    );
    run_case(
        csv,
        vec![("S", src(33.3, None)), ("K", snk(100.0, Some(500.0), None))],
    );
}

#[test]
fn c2a_two_sinks_equal_preference() {
    let csv = include_str!(
        "../verification_and_validation/cyclus_differential_gh1431/cyclus_output/c2a.csv"
    );
    run_case(
        csv,
        vec![
            ("S", src(100.0, None)),
            ("A", snk(60.0, None, None)),
            ("B", snk(70.0, None, None)),
        ],
    );
}

#[test]
fn c2b_two_sinks_unequal_preference() {
    let csv = include_str!(
        "../verification_and_validation/cyclus_differential_gh1431/cyclus_output/c2b.csv"
    );
    run_case(
        csv,
        vec![
            ("S", src(100.0, None)),
            ("A", snk(60.0, None, Some(1.0))),
            ("B", snk(70.0, None, Some(2.0))),
        ],
    );
}

#[test]
fn c2c_two_sources_one_sink() {
    let csv = include_str!(
        "../verification_and_validation/cyclus_differential_gh1431/cyclus_output/c2c.csv"
    );
    run_case(
        csv,
        vec![
            ("P", src(60.0, None)),
            ("Q", src(70.0, None)),
            ("K", snk(100.0, None, None)),
        ],
    );
}
