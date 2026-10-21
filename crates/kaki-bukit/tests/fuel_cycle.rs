// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! An end-to-end fuel-cycle simulation, run through the whole kernel.
//!
//! This file is the crate's **worked example** as much as it is a test. The
//! workspace rule is that "examples are the primary entry point, not the API
//! docs" and that a reader must be able to follow one top to bottom without
//! jumping between files. A `no_std` crate cannot ship a runnable
//! `examples/` binary that prints anything, so — following `petir`, this
//! workspace's other `no_std` crate — the worked example is an integration
//! test instead. Read `mine_to_repository` first.
//!
//! It exercises the full path: build a context, register a recipe, add agents
//! under a proper Region > Institution > Facility hierarchy, and run the six
//! phases for twelve time steps. Every trade goes through the real dynamic
//! resource exchange — portfolios, translation into an exchange graph, the
//! greedy solver, and back-translation into trades.

use kaki_bukit::agent::AgentRole;
use kaki_bukit::agents::{AgentKind, Facility, Simulation, Sink, Source};
use kaki_bukit::comp_math::CompMap;
use kaki_bukit::composition::{AtomicMasses, Composition};
use kaki_bukit::context::Context;
use kaki_bukit::nuclide::nuc;
use kaki_bukit::sim::SimInfo;

/// Natural uranium: 0.711 % U-235 by mass, balance U-238.
fn natural_u() -> Composition {
    let m: CompMap = [(nuc::U235, 0.00711), (nuc::U238, 0.99289)]
        .into_iter()
        .collect();
    Composition::from_mass(m, &AtomicMasses::MassNumber).unwrap()
}

/// A uranium mine feeding a repository for twelve months.
///
/// # What this demonstrates
///
/// The mine supplies at most 100 kg/month and holds 1000 kg over its life.
/// The repository accepts at most 80 kg/month. Neither is told about the
/// other: the mine offers a commodity, the repository asks for one, and the
/// exchange matches them every step.
///
/// # Expected result, and why
///
/// The repository's 80 kg/month is the binding constraint, so 12 steps move
/// `12 x 80 = 960` kg — under the mine's 1000 kg lifetime inventory, so the
/// mine never runs dry. Mass is conserved exactly: what the mine supplied is
/// what the repository holds.
#[test]
fn mine_to_repository() {
    let mut ctx = Context::new(SimInfo::new(12).with_handle("mine-to-repository")).unwrap();
    ctx.add_recipe("natural_u", natural_u()).unwrap();

    let mut sim = Simulation::new(ctx, AtomicMasses::MassNumber);

    // Region > Institution > Facility, which the driver enforces.
    let region = sim
        .add_agent(
            AgentRole::Region,
            "Earth",
            ":cyclus:Region",
            None,
            AgentKind::Inert,
        )
        .unwrap();
    let inst = sim
        .add_agent(
            AgentRole::Institution,
            "FuelCo",
            ":cyclus:Inst",
            Some(region),
            AgentKind::Inert,
        )
        .unwrap();

    let mine = sim
        .add_agent(
            AgentRole::Facility,
            "Mine",
            ":cycamore:Source",
            Some(inst),
            AgentKind::Source(
                Source::new("natu")
                    .unwrap()
                    .with_recipe("natural_u")
                    .with_throughput(100.0)
                    .unwrap()
                    .with_inventory(1000.0)
                    .unwrap(),
            ),
        )
        .unwrap();

    let repo = sim
        .add_agent(
            AgentRole::Facility,
            "Repository",
            ":cycamore:Sink",
            Some(inst),
            AgentKind::Sink(
                Sink::new(&["natu"])
                    .unwrap()
                    .with_recipe("natural_u")
                    .with_capacity(80.0)
                    .unwrap(),
            ),
        )
        .unwrap();

    sim.run().unwrap();

    // One trade per time step.
    assert_eq!(sim.trade_log().len(), 12, "expected one trade per step");

    let moved: f64 = sim.trade_log().iter().map(|(_, t)| t.amt).sum();
    assert!(
        (moved - 960.0).abs() < 1e-9,
        "expected 12 x 80 = 960 kg moved, got {moved}"
    );

    // The sink's capacity bound, not the source's throughput.
    for (t, trade) in sim.trade_log() {
        assert!(
            (trade.amt - 80.0).abs() < 1e-9,
            "step {t}: expected 80 kg, got {}",
            trade.amt
        );
    }

    // Mass is conserved end to end.
    let AgentKind::Source(m) = sim.agent(mine).unwrap() else {
        panic!("mine should be a Source")
    };
    let AgentKind::Sink(r) = sim.agent(repo).unwrap() else {
        panic!("repository should be a Sink")
    };
    assert!((m.supplied() - 960.0).abs() < 1e-9);
    assert!((r.quantity() - 960.0).abs() < 1e-9);
    assert!(
        (m.supplied() - r.quantity()).abs() < 1e-12,
        "mass must be conserved: mine supplied {}, repository holds {}",
        m.supplied(),
        r.quantity()
    );

    // The delivered material really is natural uranium, not a placeholder.
    let held = r.inventory().resources()[0].as_material().unwrap();
    assert!((held.comp().mass_frac(nuc::U235) - 0.00711).abs() < 1e-12);
}

/// The source's throughput binds when it is the smaller of the two.
#[test]
fn the_tighter_constraint_is_the_one_that_binds() {
    let mut ctx = Context::new(SimInfo::new(5)).unwrap();
    ctx.add_recipe("natural_u", natural_u()).unwrap();
    let mut sim = Simulation::new(ctx, AtomicMasses::MassNumber);

    let region = sim
        .add_agent(
            AgentRole::Region,
            "Earth",
            ":cyclus:Region",
            None,
            AgentKind::Inert,
        )
        .unwrap();
    let inst = sim
        .add_agent(
            AgentRole::Institution,
            "FuelCo",
            ":cyclus:Inst",
            Some(region),
            AgentKind::Inert,
        )
        .unwrap();

    // Mine 25 kg/month, repository 80 kg/month: the mine binds.
    sim.add_agent(
        AgentRole::Facility,
        "Mine",
        ":cycamore:Source",
        Some(inst),
        AgentKind::Source(
            Source::new("natu")
                .unwrap()
                .with_recipe("natural_u")
                .with_throughput(25.0)
                .unwrap(),
        ),
    )
    .unwrap();
    sim.add_agent(
        AgentRole::Facility,
        "Repository",
        ":cycamore:Sink",
        Some(inst),
        AgentKind::Sink(Sink::new(&["natu"]).unwrap().with_capacity(80.0).unwrap()),
    )
    .unwrap();

    sim.run().unwrap();
    let moved: f64 = sim.trade_log().iter().map(|(_, t)| t.amt).sum();
    assert!((moved - 125.0).abs() < 1e-9, "expected 5 x 25 = 125, got {moved}");
}

/// A mine whose lifetime inventory runs out stops trading and is decommissioned.
#[test]
fn an_exhausted_source_stops_trading_and_retires() {
    let mut ctx = Context::new(SimInfo::new(10)).unwrap();
    ctx.add_recipe("natural_u", natural_u()).unwrap();
    let mut sim = Simulation::new(ctx, AtomicMasses::MassNumber);

    let region = sim
        .add_agent(
            AgentRole::Region,
            "Earth",
            ":cyclus:Region",
            None,
            AgentKind::Inert,
        )
        .unwrap();
    let inst = sim
        .add_agent(
            AgentRole::Institution,
            "FuelCo",
            ":cyclus:Inst",
            Some(region),
            AgentKind::Inert,
        )
        .unwrap();

    // 100 kg total, 40 kg/month: exhausted partway through step 3.
    let mine = sim
        .add_agent(
            AgentRole::Facility,
            "Mine",
            ":cycamore:Source",
            Some(inst),
            AgentKind::Source(
                Source::new("natu")
                    .unwrap()
                    .with_recipe("natural_u")
                    .with_throughput(40.0)
                    .unwrap()
                    .with_inventory(100.0)
                    .unwrap(),
            ),
        )
        .unwrap();
    sim.add_agent(
        AgentRole::Facility,
        "Repository",
        ":cycamore:Sink",
        Some(inst),
        AgentKind::Sink(Sink::new(&["natu"]).unwrap().with_capacity(40.0).unwrap()),
    )
    .unwrap();

    sim.run().unwrap();

    let moved: f64 = sim.trade_log().iter().map(|(_, t)| t.amt).sum();
    assert!(
        (moved - 100.0).abs() < 1e-9,
        "a source must never supply more than its lifetime inventory, got {moved}"
    );
    // 40 + 40 + 20, then nothing.
    assert_eq!(sim.trade_log().len(), 3);
    assert!(sim.info(mine).unwrap().decommissioned);
}

/// The Region > Institution > Facility nesting is enforced, where upstream
/// silently accepts any parent.
#[test]
fn a_malformed_hierarchy_is_rejected() {
    let ctx = Context::new(SimInfo::new(1)).unwrap();
    let mut sim = Simulation::new(ctx, AtomicMasses::MassNumber);

    let region = sim
        .add_agent(
            AgentRole::Region,
            "Earth",
            ":cyclus:Region",
            None,
            AgentKind::Inert,
        )
        .unwrap();

    // A facility directly under a region: upstream allows it, this does not.
    let bad = sim.add_agent(
        AgentRole::Facility,
        "Mine",
        ":cycamore:Source",
        Some(region),
        AgentKind::Source(Source::new("natu").unwrap()),
    );
    assert!(bad.is_err());

    // A second region under the first is equally wrong.
    assert!(sim
        .add_agent(
            AgentRole::Region,
            "Mars",
            ":cyclus:Region",
            Some(region),
            AgentKind::Inert,
        )
        .is_err());
}

/// A simulation with nothing to trade runs cleanly rather than failing.
#[test]
fn a_simulation_with_no_requests_is_a_no_op() {
    let mut ctx = Context::new(SimInfo::new(3)).unwrap();
    ctx.add_recipe("natural_u", natural_u()).unwrap();
    let mut sim = Simulation::new(ctx, AtomicMasses::MassNumber);

    let region = sim
        .add_agent(
            AgentRole::Region,
            "Earth",
            ":cyclus:Region",
            None,
            AgentKind::Inert,
        )
        .unwrap();
    let inst = sim
        .add_agent(
            AgentRole::Institution,
            "FuelCo",
            ":cyclus:Inst",
            Some(region),
            AgentKind::Inert,
        )
        .unwrap();
    // A mine with nobody to sell to.
    sim.add_agent(
        AgentRole::Facility,
        "Mine",
        ":cycamore:Source",
        Some(inst),
        AgentKind::Source(Source::new("natu").unwrap().with_recipe("natural_u")),
    )
    .unwrap();

    sim.run().unwrap();
    assert!(sim.trade_log().is_empty());
}

/// Agent summaries are reachable through the enum, so a driver can report
/// state without knowing the archetype.
#[test]
fn summaries_dispatch_through_the_enum() {
    let src = AgentKind::Source(Source::new("natu").unwrap());
    let snk = AgentKind::Sink(Sink::new(&["natu"]).unwrap());
    assert!(src.summary().contains("natu"));
    assert!(snk.summary().contains("natu"));
}
