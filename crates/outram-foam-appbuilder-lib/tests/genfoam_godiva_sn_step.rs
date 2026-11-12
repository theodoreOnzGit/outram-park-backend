// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam `Godiva_SN`, ONE pseudo-time step against the real binary (gh:#1474, gh:#1361)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Ignored by default: it needs the tutorial's case directory (78 MB, not committed)
//! and runs for many minutes. Run with
//! `GENFOAM_GODIVA_CASE=<copy of Tutorials/reactorCases/Godiva_SN> cargo test --release
//! -p outram-foam-appbuilder-lib --test genfoam_godiva_sn_step -- --ignored --nocapture`.
//!
//! ## Methodology
//!
//! The upstream run is `scripts/run_tutorial.sh reactorCases/Godiva_SN godiva1` with
//! `scripts/godiva_one_step.sh` as the protocol hook: `endTime 1` (one step, nothing
//! written), `writePrecision 17`, and only the `angularFlux.*` solver tightened to
//! `tolerance 1e-10, relTol 0` (as shipped it is `1e-7 / 0.1`, where the answer depends
//! on the solver). The case is the shipped one: 23 groups, `legendreMoments 3` (P0 to P3
//! used), 4 directions per octant, `inletOutlet` on all patches, `keff 0.980106`,
//! `pTarget 1e6`, `maxInnerIter 3`, `innToOutTolerance 0.01`. The port reads the same
//! `polyMesh`, `nuclearData` and `quadratureSet` and takes the same one step.
//! The reference numbers below are the upstream log's `keff_` and `pow:` of step 1.
//! Results: README, "SN results".

use std::path::PathBuf;
use std::sync::Arc;

use outram_foam_appbuilder_lib::genfoam::neutronics::pseudo_time::{
    run_pseudo_time_eigenvalue, PseudoTimeControls,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::sn::pseudo_time::{
    SnPseudoTimeSettings, SnPseudoTimeSolver,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::sn::quadrature_set::QuadratureSet;
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData;
use outram_foam_appbuilder_lib::io::nuclear_data::read_nuclear_data;
use outram_foam_appbuilder_lib::io::poly_mesh::{read_cell_zones, read_poly_mesh, zone_of_cell};
use outram_foam_basic_lib::fields::boundary::bc::BoundaryCondition;
use outram_foam_basic_lib::ldu_matrix::SolverSettings;
use outram_foam_basic_lib::mesh::FvMesh;

/// Upstream `keff_` after step 1 (filled in from the log by the README step that records it).
const UPSTREAM_KEFF: Option<f64> = None;
/// Upstream `pow:` of step 1.
const UPSTREAM_POWER: Option<f64> = None;

#[test]
#[ignore = "needs the Godiva_SN tutorial directory (GENFOAM_GODIVA_CASE) and runs for many minutes"]
fn godiva_sn_first_step_against_upstream() {
    let Some(case) = std::env::var_os("GENFOAM_GODIVA_CASE") else {
        println!("GENFOAM_GODIVA_CASE not set: nothing to compare against");
        return;
    };
    let region = PathBuf::from(case).join("constant/neutroRegion");
    let mesh: Arc<FvMesh> = read_poly_mesh(&region.join("polyMesh")).expect("polyMesh");
    let input = read_nuclear_data(&region.join("nuclearData")).expect("nuclearData");
    let xs = CrossSectionData::from_input(&input).expect("cross sections");
    let zones = read_cell_zones(&region.join("polyMesh")).expect("cellZones");
    let order: Vec<&str> = (0..xs.zone_count()).map(|i| xs.zone(i).expect("zone").name()).collect();
    let zone_map = zone_of_cell(&zones, &order, mesh.n_cells).expect("zone map");
    let bcs: Vec<BoundaryCondition<f64>> = mesh
        .patches
        .iter()
        .map(|_| BoundaryCondition::InletOutlet { inlet_value: 0.0 })
        .collect();
    let quadrature = QuadratureSet::read(&region.join("quadratureSet")).expect("quadratureSet");
    let settings = SnPseudoTimeSettings {
        legendre_moments: 1 + 3,
        linear: SolverSettings { tolerance: 1e-10, max_iter: 100_000 },
        ..SnPseudoTimeSettings::default()
    };
    println!("Godiva_SN: {} cells, {} directions, {} groups", mesh.n_cells, quadrature.direction_count(), xs.energy_groups());
    let mut solver =
        SnPseudoTimeSolver::new(mesh, &xs, &zone_map, quadrature, &bcs, 0.980106, settings)
            .expect("SN solver");
    let mut controls = PseudoTimeControls::guided(1.0, 1.0, 20.0, 1.0, 0.025, 1e6);
    controls.adjust_time_step = false;
    let h = run_pseudo_time_eigenvalue(&mut solver, &controls).expect("one step");
    assert_eq!(h.len(), 1);
    let s = h[0].step;
    println!("Godiva_SN step 1: keff {:.16} pow {:.10} residual {:.6e}", s.k_eff, s.power, s.first_residual);
    if let (Some(k), Some(p)) = (UPSTREAM_KEFF, UPSTREAM_POWER) {
        println!("  dk {:+.3e}  dP rel {:.3e}", s.k_eff - k, ((s.power - p) / p).abs());
    }
}
