// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam `disp`-driven `radExp` / `axExp` feedback variables, against upstream's own `XS` class (gh:#1898; README
//! "Round 47", item 2c)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da0cc55` on OpenFOAM-ESI v2506. `upstream_driver/dump_xs_exp` derives from the real `XS` class,
//! reads `0/neutroRegion/disp` (a closed-form displacement on the 451-cell df_slab mesh, `scripts/make_r47_expansion_case.sh`),
//! calls `correct()` and dumps the displacement it read, the two expansion fields and the interpolated cross sections of the
//! `nuclearData.x0` feedback states (`axExp`, `radExp` in {0, 0.02}^2). The port takes upstream's own dumped `disp` as input,
//! computes `radExp`/`axExp` with `expansion_from_displacement`, builds the parameter matrix, materialises through `XsEvolution`
//! and is compared value by value. Cases: `ea` (disp A, axis z), `eb` (disp B, axis z), `ec` (disp A, `axialOrientation (1 0 0)`).
//! Criteria fixed before the runs (README "Item (2c)"): bit-identical (`==`), else the worst relative difference of the field
//! maximum, an unexplained difference above 1e-9 being a miss. MEASURED bit-identical in every array of the three cases (README "Round 47 results"), so the gates are `==`. References: `refs/r47_expansion.ref.txt.gz`.

#![cfg(feature = "std")]
use std::path::{Path, PathBuf};
use std::sync::Arc;

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::expansion::{
    cell_parameters_with_expansion, expansion_from_displacement,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::XsEvolution;
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::disc_factors::DiscFactorControls;
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData;
use outram_foam_appbuilder_lib::io::nuclear_data::{read_disc_factor_controls, read_nuclear_data};
use outram_foam_appbuilder_lib::io::poly_mesh::{read_cell_zones, read_poly_mesh, zone_of_cell};
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")
}

struct Case {
    mesh: Arc<FvMesh>,
    xs: CrossSectionData,
    mesh_zone: Vec<usize>,
    cell_zones: Vec<(String, Vec<usize>)>,
    controls: DiscFactorControls,
}

fn load(tag: &str, axis_line: Option<&str>) -> Case {
    let vv = vv_dir();
    let thread = format!("{:?}", std::thread::current().id()).replace(['(', ')'], "");
    let dst = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1898r47_{tag}_{thread}"));
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(dst.join("polyMesh")).expect("scratch dir");
    for f in ["points", "faces", "owner", "neighbour", "boundary"] {
        std::fs::write(dst.join("polyMesh").join(f), gunzip(&vv.join("cases/guided2/polyMesh").join(format!("{f}.gz")))).expect("write");
    }
    std::fs::write(dst.join("polyMesh/cellZones"), gunzip(&vv.join("cases/df_slab/cellZones.gz"))).expect("zones");
    let mut nd = std::fs::read_to_string(vv.join("cases/df_slab/nuclearData.x0")).expect("nuclearData.x0");
    if let Some(line) = axis_line {
        nd = nd.replacen("fastNeutrons    true;", &format!("fastNeutrons    true;\n{line}"), 1);
    }
    std::fs::write(dst.join("nuclearData"), nd).expect("nuclearData");
    let mesh = read_poly_mesh(&dst.join("polyMesh")).expect("polyMesh");
    let input = read_nuclear_data(&dst.join("nuclearData")).expect("nuclearData");
    let xs = CrossSectionData::from_input(&input).expect("cross sections");
    let zones = read_cell_zones(&dst.join("polyMesh")).expect("cellZones");
    let order: Vec<&str> = (0..xs.zone_count()).map(|i| xs.zone(i).expect("zone").name()).collect();
    let mesh_zone = zone_of_cell(&zones, &order, mesh.n_cells).expect("zone map");
    let cell_zones = zones.iter().map(|z| (z.name.clone(), z.cells.clone())).collect();
    let controls = read_disc_factor_controls(&dst.join("nuclearData")).expect("controls");
    Case { mesh, xs, mesh_zone, cell_zones, controls }
}

struct Up {
    arrays: Vec<(String, Vec<f64>)>,
}

impl Up {
    fn get(&self, name: &str) -> &[f64] {
        &self.arrays.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("array {name}")).1
    }
}

fn reference(case: &str) -> Up {
    let text = String::from_utf8(gunzip(&vv_dir().join("refs/r47_expansion.ref.txt.gz"))).unwrap();
    let mut on = false;
    let mut arrays = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("CASE") => on = it.next() == Some(case),
            Some("ARR") if on => {
                let name = it.next().unwrap().to_string();
                arrays.push((name, it.map(|w| w.parse().unwrap()).collect()));
            }
            _ => {}
        }
    }
    assert!(!arrays.is_empty(), "no reference for {case}");
    Up { arrays }
}

/// `(count of values that are not ==, worst relative difference of the field maximum, field maximum)`.
fn compare(port: &[f64], up: &[f64]) -> (usize, f64, f64) {
    assert_eq!(port.len(), up.len());
    let max = up.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let mut n = 0;
    let mut worst = 0.0_f64;
    for (a, b) in port.iter().zip(up) {
        if a != b {
            n += 1;
            worst = worst.max((a - b).abs() / max.max(f64::MIN_POSITIVE));
        }
    }
    (n, worst, max)
}

struct Outcome {
    rows: Vec<(String, usize, f64, f64)>,
}

fn run_case(case: &str, axis: Vector3, axis_line: Option<&str>, zero_disp: bool) -> Outcome {
    let c = load(case, axis_line);
    let up = reference(case);
    let disp: Vec<Vector3> = if zero_disp {
        vec![Vector3::ZERO; c.mesh.n_cells]
    } else {
        (0..c.mesh.n_cells).map(|i| Vector3::new(up.get("dispX")[i], up.get("dispY")[i], up.get("dispZ")[i])).collect()
    };
    let e = expansion_from_displacement(&c.mesh, &disp, axis);
    let names: Vec<String> = c.xs.variable_names().iter().map(|s| (*s).to_string()).collect();
    let params = cell_parameters_with_expansion(&names, c.mesh.n_cells, &e, &[]);
    let ev = XsEvolution::new(&c.xs, &c.mesh, c.mesh_zone.clone(), &c.cell_zones, params, Vec::new(), c.controls.clone())
        .expect("evolution");
    let f = ev.materialize(&c.mesh, 1.0).expect("materialize");
    let vals = |v: &[outram_foam_basic_lib::fields::vol_field::VolScalarField], g: usize| v[g].internal.as_slice().to_vec();
    let mut rows = Vec::new();
    let mut push = |name: &str, port: Vec<f64>| {
        let (n, w, m) = compare(&port, up.get(name));
        rows.push((name.to_string(), n, w, m));
    };
    push("axExp", e.ax_exp.clone());
    push("radExp", e.rad_exp.clone());
    for g in 0..2 {
        push(&format!("D{g}"), vals(&f.d, g));
        push(&format!("nuSigmaEff{g}"), vals(&f.nu_sigma_f, g));
        push(&format!("sigmaRemoval{g}"), vals(&f.sigma_removal, g));
        push(&format!("discFactor{g}"), vals(&f.disc_factor, g));
    }
    push("sigmaFromTo_0_1", f.scattering[0][1].internal.as_slice().to_vec());
    Outcome { rows }
}

fn print(case: &str, o: &Outcome) {
    for (name, n, w, m) in &o.rows {
        println!("{case} {name}: {} values differ, worst relative to field max {w:.3e} (field max {m:.3e})", n);
    }
}

#[test]
fn ea_axial_displacement_a_matches_upstream() {
    let o = run_case("ea", Vector3::new(0.0, 0.0, 1.0), None, false);
    print("ea", &o);
    for (name, n, w, _) in &o.rows {
        assert!(*n == 0, "ea {name}: {n} differ, worst {w:e}");
    }
}

#[test]
fn eb_axial_displacement_b_matches_upstream() {
    let o = run_case("eb", Vector3::new(0.0, 0.0, 1.0), None, false);
    print("eb", &o);
    for (name, n, w, _) in &o.rows {
        assert!(*n == 0, "eb {name}: {n} differ, worst {w:e}");
    }
}

#[test]
fn ec_x_axial_orientation_matches_upstream() {
    let o = run_case("ec", Vector3::new(1.0, 0.0, 0.0), Some("axialOrientation (1 0 0);"), false);
    print("ec", &o);
    for (name, n, w, _) in &o.rows {
        assert!(*n == 0, "ec {name}: {n} differ, worst {w:e}");
    }
}

/// E4 control: with the displacement zeroed the cross sections of `ea` are missed (the expansion matters).
#[test]
fn e4_control_zero_displacement_misses_the_cross_sections() {
    let o = run_case("ea", Vector3::new(0.0, 0.0, 1.0), None, true);
    let d0 = o.rows.iter().find(|r| r.0 == "D0").unwrap();
    println!("E4 control: D0 worst relative difference with zero displacement {:.3e}", d0.2);
    assert!(d0.2 > 1e-3, "{:e}", d0.2);
}
