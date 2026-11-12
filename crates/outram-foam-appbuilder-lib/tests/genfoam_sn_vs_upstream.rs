// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam S_N (quadratureSet, anisotropic scattering) against the real binary (gh:#1474, gh:#1361)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da0cc55` on OpenFOAM-ESI v2506, the unmodified
//! `GeN-Foam.C` with `SNNeutronics`. `Godiva_SN` (23 groups, 9 min and 3.2 GB per
//! step) is too heavy for an identical-input comparison here, so the case is built
//! from the same kind of dictionaries on a small mesh (`scripts/sn_slab_protocol.sh`):
//! guided tutorial 1's slab (151 cells, `cases/guided1/polyMesh`), Godiva's own
//! `quadratureSet` (4 directions per octant, 32 in all), `legendreMoments 3`
//! (P0 to P3; P1 = 0.03, P2 = 0.012, P3 = 0.005 are synthetic inputs of both
//! codes), `inletOutlet` on the vacuum faces, linear solvers at 1e-14 / relTol 0,
//! `adjustTimeStep false`, `deltaT 1`, 6 pseudo-time steps. The committed
//! reference `refs/sn_slab.ref.txt.gz` holds `keff_`, `pow:` and the largest
//! initial residual per step, and the final `flux0`.
//!
//! The port reads the same committed `nuclearData`, `quadratureSet` and polyMesh
//! with `QuadratureSet::read`, builds `SnPseudoTimeSolver` and runs
//! `run_pseudo_time_eigenvalue`. Predictions N1 to N7 of the README were
//! committed before this test was first run. Results: README, "SN results".

use std::path::{Path, PathBuf};
use std::sync::Arc;

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::neutronics::pseudo_time::{
    run_pseudo_time_eigenvalue, PseudoTimeControls, PseudoTimeRecord,
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

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

/// A scratch-directory suffix unique to the calling test (tests run in parallel).
fn scratch_id() -> String {
    format!("{:?}", std::thread::current().id()).replace(['(', ')'], "")
}

fn gunzip(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")
}

struct Upstream {
    keff: Vec<f64>,
    power: Vec<f64>,
    resid: Vec<f64>,
    flux: Vec<f64>,
}

fn upstream() -> Upstream {
    let text = String::from_utf8(gunzip(&vv_dir().join("refs/sn_slab.ref.txt.gz"))).unwrap();
    let mut u = Upstream { keff: vec![], power: vec![], resid: vec![], flux: vec![] };
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let key = it.next();
        let vals: Vec<f64> = if matches!(key, Some("KEFF" | "POWER" | "RESID" | "FLUX0")) {
            it.map(|w| w.parse().unwrap()).collect()
        } else {
            Vec::new()
        };
        match key {
            Some("KEFF") => u.keff = vals,
            Some("POWER") => u.power = vals,
            Some("RESID") => u.resid = vals,
            Some("FLUX0") => u.flux = vals,
            _ => {}
        }
    }
    u
}

/// Stage the case: guided1's polyMesh (committed gzipped) and the SN dictionaries.
fn stage() -> PathBuf {
    let src = vv_dir().join("cases");
    let dst = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1361_sn_slab_{}", scratch_id()));
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(dst.join("polyMesh")).expect("scratch dir");
    std::fs::copy(src.join("sn_slab/nuclearData"), dst.join("nuclearData")).expect("nuclearData");
    std::fs::copy(src.join("sn_slab/quadratureSet"), dst.join("quadratureSet")).expect("quadratureSet");
    for f in ["points", "faces", "owner", "neighbour", "boundary", "cellZones"] {
        let data = gunzip(&src.join("guided1/polyMesh").join(format!("{f}.gz")));
        std::fs::write(dst.join("polyMesh").join(f), data).expect("write polyMesh file");
    }
    dst
}

fn run_port(legendre_moments: Option<usize>) -> Vec<PseudoTimeRecord> {
    run_port_with_flux(legendre_moments).0
}

fn run_port_with_flux(legendre_moments: Option<usize>) -> (Vec<PseudoTimeRecord>, Vec<f64>) {
    let dir = stage();
    let mesh: Arc<FvMesh> = read_poly_mesh(&dir.join("polyMesh")).expect("polyMesh");
    let input = read_nuclear_data(&dir.join("nuclearData")).expect("nuclearData");
    let xs = CrossSectionData::from_input(&input).expect("cross sections");
    assert_eq!(xs.legendre_moments(), 4, "P0 to P3 stored");
    let zones = read_cell_zones(&dir.join("polyMesh")).expect("cellZones");
    let order: Vec<&str> = (0..xs.zone_count()).map(|i| xs.zone(i).expect("zone").name()).collect();
    let zone_map = zone_of_cell(&zones, &order, mesh.n_cells).expect("zone map");
    let bcs: Vec<BoundaryCondition<f64>> = mesh
        .patches
        .iter()
        .map(|p| {
            if p.name == "walls" {
                BoundaryCondition::ZeroGradient
            } else {
                BoundaryCondition::InletOutlet { inlet_value: 0.0 }
            }
        })
        .collect();
    let quadrature = QuadratureSet::read(&dir.join("quadratureSet")).expect("quadratureSet");
    assert_eq!(quadrature.direction_count(), 32);
    // `legendreMoments 3` in the dictionary = 4 matrices used (XS.C:75).
    let settings = SnPseudoTimeSettings {
        legendre_moments: legendre_moments.unwrap_or(4),
        linear: SolverSettings { tolerance: 1e-13, max_iter: 100_000 },
        ..SnPseudoTimeSettings::default()
    };
    let mut solver =
        SnPseudoTimeSolver::new(mesh.clone(), &xs, &zone_map, quadrature, &bcs, 1.0, settings)
            .expect("SN solver");
    let mut controls = PseudoTimeControls::guided(1.0, 6.0, 6.0, 1.0, 0.025, 1e6);
    controls.adjust_time_step = false;
    let h = run_pseudo_time_eigenvalue(&mut solver, &controls).expect("loop");
    assert_eq!(solver.k_eff(), h.last().expect("history").step.k_eff);
    assert_eq!(solver.quadrature().direction_count(), 32);
    let flux = solver.flux(0).to_vec();
    (h, flux)
}

fn shape(v: &[f64]) -> Vec<f64> {
    let s: f64 = v.iter().sum();
    v.iter().map(|x| x / s).collect()
}

#[test]
fn sn_slab_with_godiva_quadrature_and_p3_scattering_matches_upstream() {
    let up = upstream();
    assert_eq!(up.keff.len(), 6);
    let (h, flux) = run_port_with_flux(None);
    assert_eq!(h.len(), 6, "six pseudo-time steps");
    let mut dks = Vec::new();
    for (i, r) in h.iter().enumerate() {
        let dk = (r.step.k_eff - up.keff[i]).abs();
        let dp = ((r.step.power - up.power[i]) / up.power[i]).abs();
        let dr = ((r.step.first_residual - up.resid[i]) / up.resid[i]).abs();
        println!(
            "SN slab step {}: upstream k {:.16} port k {:.16} dk {:+.3e} | dP rel {:.3e} | residual up {:.6e} port {:.6e} rel {:.3e}",
            i + 1,
            up.keff[i],
            r.step.k_eff,
            r.step.k_eff - up.keff[i],
            dp,
            up.resid[i],
            r.step.first_residual,
            dr
        );
        dks.push((dk, dp, dr));
    }
    let (sp, su) = (shape(&flux), shape(&up.flux));
    let flux_rel = sp.iter().zip(&su).map(|(a, b)| ((a - b) / b).abs()).fold(0.0_f64, f64::max);
    println!("SN slab step 6 scalar flux shape, max relative difference {flux_rel:.3e}");
    // N1
    assert!(dks[0].0 < 1e-11, "N1 dk {:e}", dks[0].0);
    assert!(dks[0].1 < 1e-11, "N1 dP {:e}", dks[0].1);
    assert!(dks[0].2 < 1e-9, "N1 residual {:e}", dks[0].2);
    // N2
    for (i, d) in dks.iter().enumerate().take(4).skip(1) {
        assert!(d.0 < 1e-9, "N2 step {} dk {:e}", i + 1, d.0);
        assert!(d.2 < 1e-5, "N4 step {} residual {:e}", i + 1, d.2);
    }
    // N3, N4
    for (i, d) in dks.iter().enumerate().skip(4) {
        assert!(d.0 < 1e-6, "N3 step {} dk {:e}", i + 1, d.0);
        assert!(d.2 < 1e-3, "N4 step {} residual {:e}", i + 1, d.2);
    }
    // N5
    assert!(flux_rel < 1e-5, "N5 flux shape {flux_rel:e}");
}

/// N6, the ablation: with P0 only on the same quadrature the first step differs
/// from upstream, so the case does exercise the anisotropic source.
#[test]
fn dropping_the_anisotropic_moments_is_visible_against_upstream() {
    let up = upstream();
    let h = run_port(Some(1));
    let rel = ((h[0].step.k_eff - up.keff[0]) / up.keff[0]).abs();
    println!("SN slab P0 only: step-1 keff {} vs upstream {} (relative {rel:.3e})", h[0].step.k_eff, up.keff[0]);
    assert!(rel > 1e-4, "N6: P1 to P3 must matter, got {rel:e}");
}
