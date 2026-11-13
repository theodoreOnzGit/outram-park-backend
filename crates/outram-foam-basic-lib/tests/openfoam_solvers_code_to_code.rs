//! Linear-solver differential test against upstream OpenFOAM-v2506 (gh:#1362, row 3).
//!
//! # Method
//! `verification_and_validation/upstream_solvers_gh1362/scripts/run.sh` runs the REAL ESI
//! `pimpleFoam` on a 20x20x1 lid-driven cavity for one time step with a solver that dumps
//! the first pressure matrix (diag, upper, addressing), its source and the incoming psi,
//! then runs the real upstream `PCG`/`DIC`, `PBiCGStab`/`DILU`, `smoothSolver`/`GaussSeidel`
//! and `GAMG` on it for n = 1..30 iterations (`tolerance 0`, `minIter = maxIter = n`) and
//! prints the iterate after each n plus the normalised residuals. This test builds the matrix
//! from the dump (lower = upper, as upstream stores a symmetric matrix) and runs the port's
//! solvers for the same n, comparing the iterate bit for bit; the residual is compared too
//! where the port computes the same quantity (the initial residual, gh:#1363).
//!
//! # Criterion
//! Bit-identity per iterate; per solver the count of bit-identical iterates (out of 30
//! iterates x 400 cells) and the worst relative difference are PINNED.
//!
//! # Results
//! See `verification_and_validation/upstream_solvers_gh1362/README.md` (release, 2026-11-13).

use outram_foam_basic_lib::ldu_matrix::solvers::{
    conjugate_gradient, gauss_seidel, gauss_seidel_upstream, pbicgstab_dilu, PbicgstabDiluSettings,
};
use outram_foam_basic_lib::ldu_matrix::{LduMatrix, SolverSettings};
use std::collections::BTreeMap;

const REF_GZ: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_solvers_gh1362/reference/ref_cavity_p.txt.gz"
);

const N_ITER: usize = 30;

fn gunzip(data: &[u8]) -> String {
    // `gzip -n`: 10-byte header, deflate stream, 8-byte trailer.
    let body = &data[10..data.len() - 8];
    let raw = miniz_oxide::inflate::decompress_to_vec(body).expect("inflate");
    String::from_utf8(raw).expect("utf8")
}

fn hex(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).expect("hex"))
}

struct Dump {
    cols: BTreeMap<String, Vec<f64>>,
    lower_addr: Vec<usize>,
    upper_addr: Vec<usize>,
}

fn dump() -> Dump {
    let mut d = Dump { cols: BTreeMap::new(), lower_addr: vec![], upper_addr: vec![] };
    for line in gunzip(REF_GZ).lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        match t[0] {
            "addr_lower" => d.lower_addr.push(t[2].parse().unwrap()),
            "addr_upper" => d.upper_addr.push(t[2].parse().unwrap()),
            key => d.cols.entry(key.to_string()).or_default().push(hex(t[2])),
        }
    }
    d
}

fn matrix(d: &Dump) -> (LduMatrix, Vec<f64>, Vec<f64>) {
    let n = d.cols["eq_p_diag"].len();
    let mut m = LduMatrix::new(n, d.lower_addr.clone(), d.upper_addr.clone());
    m.diag = d.cols["eq_p_diag"].clone();
    m.upper = d.cols["eq_p_upper"].clone();
    m.lower = m.upper.clone();
    (m, d.cols["eq_p_source"].clone(), d.cols["eq_p_psi"].clone())
}

fn ord(x: f64) -> i64 {
    let b = x.to_bits() as i64;
    if b < 0 {
        i64::MIN - b
    } else {
        b
    }
}

/// (bit-identical values, total values, worst relative difference, first iterate that is
/// not bit-identical (0 = none)).
fn compare_iterates(
    name: &str,
    d: &Dump,
    run: impl Fn(usize) -> Vec<f64>,
) -> (usize, usize, f64, usize) {
    let (mut same, mut total, mut worst, mut first_bad) = (0, 0, 0.0_f64, 0);
    for n in 1..=N_ITER {
        let rust = run(n);
        let up = &d.cols[&format!("hist_{name}_psi{n}")];
        assert_eq!(rust.len(), up.len());
        let mut bad = false;
        for (a, b) in rust.iter().zip(up) {
            total += 1;
            if a.to_bits() == b.to_bits() {
                same += 1;
            } else {
                bad = true;
                let rel = (a - b).abs() / b.abs().max(f64::MIN_POSITIVE);
                worst = worst.max(rel);
                let _ = ord(*a);
            }
        }
        if bad && first_bad == 0 {
            first_bad = n;
        }
    }
    println!(
        "{name:<28} identical={same}/{total} worst_rel={worst:.3e} first_non_identical_iterate={first_bad}"
    );
    (same, total, worst, first_bad)
}

#[test]
fn initial_residual_is_upstream_one() {
    let d = dump();
    let (m, b, psi0) = matrix(&d);
    let r = m.openfoam_initial_residual(&psi0, &b);
    let up = d.cols["hist_PCG_DIC_init"][0];
    println!("initial residual port {r:e} upstream {up:e}");
    assert_eq!(r.to_bits(), up.to_bits());
}

#[test]
fn pcg_dic_vs_openfoam() {
    let d = dump();
    let (m, b, psi0) = matrix(&d);
    let run = |n: usize| {
        let s = SolverSettings { tolerance: 0.0, max_iter: n };
        conjugate_gradient(&m, &b, Some(&psi0), &s).0
    };
    let r = compare_iterates("PCG_DIC", &d, run);
    assert_eq!((r.0, r.1, r.3), PIN_PCG);
}

#[test]
fn smooth_solver_gauss_seidel_vs_openfoam() {
    let d = dump();
    let (m, b, psi0) = matrix(&d);
    let run = |n: usize| {
        let mut x = psi0.clone();
        gauss_seidel(&m, &b, &mut x, 0.0, n);
        x
    };
    let r = compare_iterates("smoothSolver_GaussSeidel", &d, run);
    assert_eq!((r.0, r.1, r.3), PIN_GS);
    assert!(r.2 < 1e-12, "the reference form differs from upstream only by rounding");
}

#[test]
fn smooth_solver_gauss_seidel_upstream_form_vs_openfoam() {
    let d = dump();
    let (m, b, psi0) = matrix(&d);
    let run = |n: usize| {
        let mut x = psi0.clone();
        gauss_seidel_upstream(&m, &b, &mut x, n);
        x
    };
    let r = compare_iterates("smoothSolver_GaussSeidel", &d, run);
    assert_eq!((r.0, r.1, r.3), (12000, 12000, 0));
}

#[test]
fn pbicgstab_dilu_vs_openfoam() {
    let d = dump();
    let (m, b, psi0) = matrix(&d);
    let run = |n: usize| {
        let s = PbicgstabDiluSettings { tolerance: 0.0, rel_tol: 0.0, min_iter: n, max_iter: n };
        pbicgstab_dilu(&m, &b, &psi0, &s).0
    };
    let r = compare_iterates("PBiCGStab_DILU", &d, run);
    assert_eq!((r.0, r.1, r.3), PIN_PBICGSTAB);
}

#[test]
fn krylov_bicgstab_ilu0_differs_from_upstream_pbicgstab_dilu() {
    use outram_foam_basic_lib::compute::ComputeBackend;
    use outram_foam_basic_lib::krylov::{bicgstab, KrylovSettings, Preconditioner};
    let _ = ComputeBackend::Serial;
    let d = dump();
    let (m, b, psi0) = matrix(&d);
    let pre = Preconditioner::ilu0(&m);
    let run = |n: usize| {
        let s = KrylovSettings { tolerance: 1e-300, max_iter: n, restart: 0 };
        bicgstab(&m, &b, Some(&psi0), &pre, &s).0
    };
    let r = compare_iterates("PBiCGStab_DILU", &d, run);
    println!("krylov::bicgstab with ILU(0) against upstream PBiCGStab/DILU: {r:?}");
    assert_eq!((r.0, r.1, r.3), PIN_KRYLOV_ILU0);
}

const PIN_KRYLOV_ILU0: (usize, usize, usize) = (132, 12000, 1);
const PIN_PCG:(usize, usize, usize) = (12000, 12000, 0);
const PIN_GS: (usize, usize, usize) = (4737, 12000, 1);
const PIN_PBICGSTAB: (usize, usize, usize) = (12000, 12000, 0);
