// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam diffusion neutronics against a run of the real GeN-Foam binary (gh:#1361, step 3)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da0cc55` on OpenFOAM-ESI v2506. The binary is the
//! unmodified `GeN-Foam.C` linked against `libMultiRegion` and
//! `libNeutronicsSolvers` (`scripts/build_neutronics_stack.sh`). It ran the two
//! guided tutorials `1_reactorSlab_1D_1Gr_neutronicDiffusion` (151 cells) and
//! `2_reactorSlabReflected_1D_1Gr_neutronicDiffusion` (751 cells) on the
//! tutorials' own `polyMesh` and `nuclearData`, with `writePrecision 17`, every
//! linear solver at `tolerance 1e-14, relTol 0` and `endTime 60`
//! (`scripts/tight_protocol.sh`), serial. The reference
//! `refs/guided_slabs.ref.txt.gz` holds, per case, the keff, power and first
//! residual of each of the 230 pseudo-time steps, and the final `flux0` per cell.
//!
//! The port reads the same committed inputs (`cases/guided{1,2}`), solves the
//! eigenvalue problem with `DiffusionNeutronics::solve_eigenvalue` at
//! `k_tolerance 1e-13`, `flux_tolerance 1e-12`, linear `1e-13`, and is compared
//! on k_eff and the flux per cell (shape, normalised to unit sum, because
//! upstream normalises to `pTarget` and the port to unit fission production).
//!
//! A third, independent reference is computed here: a tridiagonal generalised
//! eigenproblem for the same discrete operator, by power iteration with the
//! Thomas algorithm, written from the equations alone.
//!
//! The iteration HISTORY is not compared: upstream updates keff once per
//! pseudo-time step (`calcKeff.H`), the port has no such driver.
//!
//! Results and predictions: the `README.md` beside the references.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::{
    DiffusionNeutronics, DiffusionSettings,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::sp3::{Sp3Neutronics, Sp3Settings};
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

fn gunzip(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")
}

/// Decompress a committed case's polyMesh into the test scratch directory.
fn stage_case(case: &str) -> PathBuf {
    let src = vv_dir().join("cases").join(case);
    let dst = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1361_{case}"));
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(dst.join("polyMesh")).expect("scratch dir");
    std::fs::copy(src.join("nuclearData"), dst.join("nuclearData")).expect("nuclearData");
    for f in ["points", "faces", "owner", "neighbour", "boundary", "cellZones"] {
        let data = gunzip(&src.join("polyMesh").join(format!("{f}.gz")));
        std::fs::write(dst.join("polyMesh").join(f), data).expect("write polyMesh file");
    }
    dst
}

struct UpstreamCase {
    keff_history: Vec<f64>,
    flux: Vec<f64>,
}

fn upstream(case_index: usize) -> UpstreamCase {
    upstream_from("guided_slabs", case_index)
}

fn upstream_from(file: &str, case_index: usize) -> UpstreamCase {
    let text = String::from_utf8(gunzip(&vv_dir().join(format!("refs/{file}.ref.txt.gz")))).unwrap();
    let mut cur = 0usize;
    let (mut keff_history, mut flux) = (Vec::new(), Vec::new());
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("CASE") => cur += 1,
            Some("KEFF") if cur == case_index => {
                keff_history = it.map(|w| w.parse().unwrap()).collect();
            }
            Some("FLUX0") if cur == case_index => {
                flux = it.map(|w| w.parse().unwrap()).collect();
            }
            _ => {}
        }
    }
    UpstreamCase { keff_history, flux }
}

/// Everything the comparison needs from one port solve.
struct PortSolve {
    k_eff: f64,
    outer_iterations: usize,
    converged: bool,
    flux: Vec<f64>,
    // geometry and cross sections per cell, in cell order, for the third reference
    mesh: Arc<FvMesh>,
    d: Vec<f64>,
    sigma_r: Vec<f64>,
    nu_sigma_f: Vec<f64>,
}

/// Mesh, cross sections, zone map and the tutorials' flux boundary conditions.
struct Setup {
    mesh: Arc<FvMesh>,
    xs: CrossSectionData,
    zone_map: Vec<usize>,
    flux_boundary: Vec<BoundaryCondition<f64>>,
}

fn setup(case: &str) -> Setup {
    let dir = stage_case(case);
    let mesh: Arc<FvMesh> = read_poly_mesh(&dir.join("polyMesh")).expect("polyMesh");
    let input = read_nuclear_data(&dir.join("nuclearData")).expect("nuclearData");
    let xs = CrossSectionData::from_input(&input).expect("cross sections");
    let zones = read_cell_zones(&dir.join("polyMesh")).expect("cellZones");
    let order: Vec<&str> = (0..xs.zone_count())
        .map(|i| xs.zone(i).expect("zone").name())
        .collect();
    let zone_map = zone_of_cell(&zones, &order, mesh.n_cells).expect("zone map");
    // 0.orig/defaultFlux and defaultFlux2: `walls` zeroGradient, `(top|bottom)` fixedValue 0.
    let flux_boundary: Vec<BoundaryCondition<f64>> = mesh
        .patches
        .iter()
        .map(|p| {
            if p.name == "walls" {
                BoundaryCondition::ZeroGradient
            } else {
                assert!(p.name == "top" || p.name == "bottom", "patch {}", p.name);
                BoundaryCondition::FixedValue(0.0)
            }
        })
        .collect();
    Setup { mesh, xs, zone_map, flux_boundary }
}

/// SP3 on the same inputs: `(k_eff, outer iterations, converged, flux per cell)`.
fn solve_sp3_port(case: &str) -> (f64, usize, bool, Vec<f64>) {
    let s = setup(case);
    let settings = Sp3Settings {
        k_tolerance: 1e-13,
        flux_tolerance: 1e-12,
        max_outer_iterations: 200_000,
        linear: SolverSettings {
            tolerance: 1e-13,
            max_iter: 100_000,
        },
        ..Sp3Settings::default()
    };
    let mut solver = Sp3Neutronics::with_cross_sections(
        s.mesh.clone(),
        &s.xs,
        &s.zone_map,
        &[],
        &s.flux_boundary,
        settings,
    )
    .expect("SP3 solver");
    let report = solver.solve_eigenvalue().expect("SP3 eigenvalue solve");
    let flux = solver.state().flux()[0].internal.as_slice().to_vec();
    (report.k_eff, report.outer_iterations, report.converged, flux)
}

fn solve_port(case: &str) -> PortSolve {
    let Setup { mesh, xs, zone_map, flux_boundary } = setup(case);
    let settings = DiffusionSettings {
        k_tolerance: 1e-13,
        flux_tolerance: 1e-12,
        max_outer_iterations: 200_000,
        linear: SolverSettings {
            tolerance: 1e-13,
            max_iter: 100_000,
        },
        ..DiffusionSettings::default()
    };
    let mut solver =
        DiffusionNeutronics::new(mesh.clone(), &xs, &zone_map, &[], &flux_boundary, settings)
            .expect("diffusion solver");
    let report = solver.solve_eigenvalue().expect("eigenvalue solve");
    let flux = solver.state().flux()[0].internal.as_slice().to_vec();

    let per_cell = |f: &dyn Fn(usize) -> f64| -> Vec<f64> { (0..mesh.n_cells).map(f).collect() };
    let xf = solver.xs_fields();
    let d = per_cell(&|c| xf.d[0].internal.as_slice()[c]);
    let sigma_r = per_cell(&|c| xf.sigma_removal[0].internal.as_slice()[c]);
    let nu_sigma_f = per_cell(&|c| xf.nu_sigma_f[0].internal.as_slice()[c]);
    PortSolve {
        k_eff: report.k_eff,
        outer_iterations: report.outer_iterations,
        converged: report.converged,
        flux,
        mesh,
        d,
        sigma_r,
        nu_sigma_f,
    }
}

/// Independent reference: the 1-D finite-volume operator written from the
/// equations alone, `-(D phi')' + Sigma_r phi = nuSigma_f phi / k`, zero flux at
/// both ends (half-cell distance), face D by linear interpolation at the face,
/// solved by power iteration with the Thomas algorithm. Cell area 0.1 x 0.1 m
/// (the blockMeshDict's `dx dy`); it cancels from the result.
fn tridiagonal_reference(p: &PortSolve) -> f64 {
    let area = 0.1 * 0.1;
    let mut cells: Vec<usize> = (0..p.mesh.n_cells).collect();
    cells.sort_by(|&a, &b| {
        p.mesh.cell_centres[a].z.partial_cmp(&p.mesh.cell_centres[b].z).unwrap()
    });
    let n = cells.len();
    let h: Vec<f64> = cells.iter().map(|&c| p.mesh.cell_volumes[c] / area).collect();
    let mut zf = vec![0.0; n + 1];
    for i in 0..n {
        zf[i + 1] = zf[i] + h[i];
    }
    let zc: Vec<f64> = (0..n).map(|i| zf[i] + 0.5 * h[i]).collect();
    let d: Vec<f64> = cells.iter().map(|&c| p.d[c]).collect();
    let sr: Vec<f64> = cells.iter().map(|&c| p.sigma_r[c]).collect();
    let nf: Vec<f64> = cells.iter().map(|&c| p.nu_sigma_f[c]).collect();

    // face coefficients a_i between cell i and i+1; boundary coefficients
    let mut a = vec![0.0; n - 1];
    for i in 0..n - 1 {
        let w = (zc[i + 1] - zf[i + 1]) / (zc[i + 1] - zc[i]);
        let df = w * d[i] + (1.0 - w) * d[i + 1];
        a[i] = df * area / (zc[i + 1] - zc[i]);
    }
    let a_lo = d[0] * area / (0.5 * h[0]);
    let a_hi = d[n - 1] * area / (0.5 * h[n - 1]);
    let mut diag = vec![0.0; n];
    for i in 0..n {
        let left = if i == 0 { a_lo } else { a[i - 1] };
        let right = if i == n - 1 { a_hi } else { a[i] };
        diag[i] = left + right + sr[i] * h[i] * area;
    }
    let vol: Vec<f64> = (0..n).map(|i| h[i] * area).collect();

    let thomas = |rhs: &[f64]| -> Vec<f64> {
        let mut c = vec![0.0; n];
        let mut dd = vec![0.0; n];
        c[0] = -a[0] / diag[0];
        dd[0] = rhs[0] / diag[0];
        for i in 1..n {
            let upper = if i < n - 1 { a[i] } else { 0.0 };
            let m = diag[i] + a[i - 1] * c[i - 1];
            c[i] = -upper / m;
            dd[i] = (rhs[i] + a[i - 1] * dd[i - 1]) / m;
        }
        let mut x = vec![0.0; n];
        x[n - 1] = dd[n - 1];
        for i in (0..n - 1).rev() {
            x[i] = dd[i] - c[i] * x[i + 1];
        }
        x
    };

    let mut phi = vec![1.0; n];
    let mut k = 1.0;
    let fission = |phi: &[f64]| -> f64 { (0..n).map(|i| nf[i] * vol[i] * phi[i]).sum() };
    let mut f_old = fission(&phi);
    for _ in 0..2_000_000 {
        let rhs: Vec<f64> = (0..n).map(|i| nf[i] * vol[i] * phi[i] / k).collect();
        let new = thomas(&rhs);
        let f_new = fission(&new);
        let k_new = k * f_new / f_old;
        let scale = 1.0 / f_new;
        phi = new.iter().map(|x| x * scale).collect();
        f_old = 1.0;
        let done = (k_new - k).abs() < 1e-16;
        k = k_new;
        if done {
            break;
        }
    }
    k
}

fn shape(v: &[f64]) -> Vec<f64> {
    let s: f64 = v.iter().sum();
    v.iter().map(|x| x / s).collect()
}

fn compare_case(case: &str, index: usize) -> (f64, f64, f64, f64, usize, bool, usize) {
    let up = upstream(index);
    let port = solve_port(case);
    let k_up = *up.keff_history.last().expect("history");
    let k_ref = tridiagonal_reference(&port);
    let (sp, su) = (shape(&port.flux), shape(&up.flux));
    assert_eq!(sp.len(), su.len());
    let flux_rel = sp
        .iter()
        .zip(&su)
        .map(|(a, b)| ((a - b) / b).abs())
        .fold(0.0_f64, f64::max);
    (
        port.k_eff - k_up,
        k_ref - k_up,
        port.k_eff - k_ref,
        flux_rel,
        port.outer_iterations,
        port.converged,
        up.keff_history.len(),
    )
}

#[test]
fn guided_slabs_against_upstream() {
    for (case, idx) in [("guided1", 1), ("guided2", 2)] {
        let up = upstream(idx);
        let r = compare_case(case, idx);
        println!(
            "{case}: upstream keff {:.17} ({} steps)\n  port-upstream {:+.3e}\n  tridiag-upstream {:+.3e}\n  port-tridiag {:+.3e}\n  flux shape max rel {:.3e}\n  port outer iterations {} converged {}",
            up.keff_history.last().unwrap(),
            r.6,
            r.0,
            r.1,
            r.2,
            r.3,
            r.4,
            r.5
        );
        // Predictions S1-S3 (README), pinned at the predicted bounds.
        assert!(r.5, "{case}: port eigenvalue solve converged");
        assert_eq!(r.6, 230, "{case}: upstream steps in the reference");
        assert!(r.0.abs() < 1e-12, "{case}: port - upstream keff {:e}", r.0);
        assert!(r.1.abs() < 1e-12, "{case}: tridiagonal - upstream keff {:e}", r.1);
        assert!(r.2.abs() < 1e-12, "{case}: port - tridiagonal keff {:e}", r.2);
        assert!(r.3 < 1e-9, "{case}: flux shape max relative difference {:e}", r.3);
    }
}

/// SP3 on the two slabs against the real binary with `SP3Neutronics` selected
/// (prediction S5 in the README).
#[test]
fn sp3_slabs_against_upstream() {
    for (case, idx) in [("guided1", 1usize), ("guided2", 2)] {
        let up = upstream_from("sp3_slabs", idx);
        let k_up = *up.keff_history.last().unwrap();
        let (k, iters, converged, flux) = solve_sp3_port(case);
        let (sp, su) = (shape(&flux), shape(&up.flux));
        let flux_rel = sp
            .iter()
            .zip(&su)
            .map(|(a, b)| ((a - b) / b).abs())
            .fold(0.0_f64, f64::max);
        println!(
            "SP3 {case}: upstream {k_up:.17} ({} steps)  port {k:.17}  dk {:+.3e}  flux shape max rel {flux_rel:.3e}  port iterations {iters} converged {converged}",
            up.keff_history.len(),
            k - k_up
        );
        // Prediction S5, pinned at the predicted bounds.
        assert!(converged, "{case}: port SP3 solve converged");
        assert!((k - k_up).abs() < 1e-9, "{case}: SP3 dk {:e}", k - k_up);
        assert!(flux_rel < 1e-7, "{case}: SP3 flux shape {flux_rel:e}");
    }
}

/// The upstream history is a converging sequence: keff rises from the seed,
/// the last 10 steps agree to 1e-14 (the reference is settled), and the
/// shipped-tolerance value of case 1 (`1.0000037`, from the tutorial run at its
/// own 1e-6 / 1e-3 solver tolerances) is below the converged value, as S0 says.
#[test]
fn upstream_history_is_settled_and_shipped_value_is_not_converged() {
    for idx in [1usize, 2] {
        let h = upstream(idx).keff_history;
        let last = h[h.len() - 1];
        assert!(h[0] < last);
        for k in &h[h.len() - 10..] {
            assert!((k - last).abs() < 1e-14, "{k} vs {last}");
        }
    }
    let converged = *upstream(1).keff_history.last().unwrap();
    let shipped = 1.0000037;
    assert!(shipped < converged && (converged - shipped) > 3.0e-6 && (converged - shipped) < 4.5e-6);
}
