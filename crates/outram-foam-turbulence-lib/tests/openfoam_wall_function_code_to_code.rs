//! Wall-function differential test against upstream OpenFOAM-v2506 (gh:#1362, row 5, wall-function slice).
//!
//! # Method
//! `verification_and_validation/upstream_wallfn_gh1362/driver/wallFnDump.C` builds the
//! real `wallFunctionCoefficients` (yPlusLam) from `libturbulenceModels` and
//! transcribes the STEPWISE branch of `nutkWallFunction::calcNut()` for one face
//! over 3 (kappa, E) x 3 nu x 12 y+ points, printing bit patterns. This test
//! compares `y_plus_lam` and `nu_t_wall_esi_v2506`, and records how the older
//! `nu_t_wall` differs. Bit-identity is the criterion; counts are pinned.

use outram_foam_turbulence_lib::wall_functions::{nu_t_wall, nu_t_wall_esi_v2506, y_plus_lam};

const REF: &str =
    include_str!("../verification_and_validation/upstream_wallfn_gh1362/reference/wallfn_esi_v2506.txt");

fn vals(line: &str) -> Vec<f64> {
    line.split_whitespace()
        .skip(1)
        .map(|h| f64::from_bits(u64::from_str_radix(h, 16).unwrap()))
        .collect()
}

fn ulp(a: f64, b: f64) -> u64 {
    if a.to_bits() == b.to_bits() {
        0
    } else if a.is_sign_negative() != b.is_sign_negative() {
        u64::MAX
    } else {
        a.to_bits().abs_diff(b.to_bits())
    }
}

#[test]
fn wall_function_vs_openfoam() {
    let (mut lam_same, mut lam_n) = (0, 0);
    let (mut esi_same, mut esi_n, mut esi_worst) = (0, 0, 0u64);
    let (mut old_same, mut old_n, mut old_diff_values) = (0, 0, 0);
    for line in REF.lines() {
        let v = vals(line);
        if line.starts_with("yplus_lam") {
            lam_n += 1;
            if y_plus_lam(v[0], v[1]).to_bits() == v[2].to_bits() {
                lam_same += 1;
            }
        } else {
            let (kappa, e, nu, yp, nutw) = (v[0], v[1], v[2], v[3], v[5]);
            esi_n += 1;
            let d = ulp(nu_t_wall_esi_v2506(yp, nu, kappa, e), nutw);
            if d == 0 {
                esi_same += 1;
            }
            esi_worst = esi_worst.max(d);
            old_n += 1;
            let o = nu_t_wall(yp, nu);
            if o.to_bits() == nutw.to_bits() {
                old_same += 1;
            }
            if (o - nutw).abs() > 1e-12 * nutw.abs().max(1e-30) {
                old_diff_values += 1;
            }
        }
    }
    println!(
        "yPlusLam {lam_same}/{lam_n}; ESI form {esi_same}/{esi_n} worst ulp {esi_worst}; old nu_t_wall {old_same}/{old_n} bit-identical, {old_diff_values} beyond 1e-12"
    );
    assert_eq!((lam_same, lam_n), (3, 3));
    assert_eq!((esi_same, esi_n, esi_worst), (108, 108, 0));
    // the older form hard-codes kappa 0.41, E 9.8 and a crossover of 11
    assert_eq!((old_same, old_n, old_diff_values), (42, 108, 48));
}
