// SPDX-License-Identifier: GPL-3.0-only
//! **V&V: `petir::interp2d` (bilinear, bicubic) against compiled GSL 2.8**
//! (gh:#1341). Methodology, reference and prediction P9:
//! `verification_and_validation/interp_piecewise_gh1341.md`.
//!
//! Replays `reference-data/gsl/interp2d-gsl-2.8-reference.txt` (driver
//! `interp2d_reference_driver.c`): four grids, both methods, value and the five
//! partial derivatives at a 17 x 17 grid of points plus every node, 12
//! extrapolation points, the 12 out-of-range refusals, and whether
//! `gsl_interp2d_alloc` accepts the grid. Every value is compared as a bit
//! pattern. The grids come from the file's own `G` rows (GSL's inputs).

use petir::interp2d::{Interp2d, Interp2dKind};
use petir::PetirError;
use std::collections::BTreeMap;

const REFERENCE: &str = include_str!("../../../reference-data/gsl/interp2d-gsl-2.8-reference.txt");

fn kind_of(m: &str) -> Interp2dKind {
    match m {
        "bilinear" => Interp2dKind::Bilinear,
        "bicubic" => Interp2dKind::Bicubic,
        other => panic!("unknown method {other}"),
    }
}

#[test]
fn bilinear_and_bicubic_match_compiled_gsl() {
    let mut grids: BTreeMap<String, (Vec<f64>, Vec<f64>, Vec<f64>)> = BTreeMap::new();
    let mut models: BTreeMap<(String, String), Interp2d> = BTreeMap::new();
    let mut tally: BTreeMap<(String, String), (usize, usize)> = BTreeMap::new();
    let (mut allocs, mut oors) = (0, 0);
    for line in REFERENCE.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f[0] {
            "G" => {
                let nx: usize = f[2].parse().unwrap();
                let ny: usize = f[3].parse().unwrap();
                let v: Vec<f64> = f[4..].iter().map(|s| s.parse().unwrap()).collect();
                assert_eq!(v.len(), nx + ny + nx * ny);
                let xs = v[..nx].to_vec();
                let ys = v[nx..nx + ny].to_vec();
                let zs = v[nx + ny..].to_vec();
                grids.insert(f[1].to_string(), (xs, ys, zs));
            }
            "ALLOC" => {
                let (m, g) = (f[1], f[2]);
                let ok = f[3] == "1";
                let (xs, ys, zs) = grids.get(g).expect("grid row first");
                let built = Interp2d::new(kind_of(m), xs, ys, zs);
                assert_eq!(built.is_ok(), ok, "{m} {g} alloc");
                if let Ok(model) = built {
                    models.insert((m.to_string(), g.to_string()), model);
                }
                allocs += 1;
            }
            "OOR" => {
                let (m, g) = (f[1], f[2]);
                let (x, y): (f64, f64) = (f[3].parse().unwrap(), f[4].parse().unwrap());
                let status: i32 = f[5].parse().unwrap();
                assert_eq!(status, 1, "GSL_EDOM");
                if let Some(model) = models.get(&(m.to_string(), g.to_string())) {
                    assert_eq!(model.eval(x, y), Err(PetirError::Domain));
                    oors += 1;
                }
            }
            "#" => panic!("driver reported a problem: {line}"),
            _ => {
                let (m, g, op) = (f[0], f[1], f[2]);
                let (x, y, want): (f64, f64, f64) = (
                    f[3].parse().unwrap(),
                    f[4].parse().unwrap(),
                    f[5].parse().unwrap(),
                );
                let model = models.get(&(m.to_string(), g.to_string())).expect("model");
                let got = match op {
                    "E" => model.eval(x, y).unwrap(),
                    "DX" => model.eval_deriv_x(x, y).unwrap(),
                    "DY" => model.eval_deriv_y(x, y).unwrap(),
                    "DXX" => model.eval_deriv_xx(x, y).unwrap(),
                    "DYY" => model.eval_deriv_yy(x, y).unwrap(),
                    "DXY" => model.eval_deriv_xy(x, y).unwrap(),
                    "X" => model.eval_extrap(x, y),
                    other => panic!("unknown op {other}"),
                };
                let e = tally.entry((m.to_string(), op.to_string())).or_default();
                e.0 += 1;
                if got.to_bits() == want.to_bits() {
                    e.1 += 1;
                } else if std::env::var_os("SPECFUNC_SHOW_DIFFS").is_some() {
                    println!("DIFF {line}: ours {got:e}");
                }
            }
        }
    }
    let (mut rows, mut exact) = (0, 0);
    for ((m, op), (r, x)) in &tally {
        println!("  {m} {op}: {x}/{r} bit-identical");
        rows += r;
        exact += x;
    }
    println!("  interp2d vs GSL 2.8: {exact}/{rows} bit-identical; {allocs} ALLOC rows; {oors} OOR refusals");
    assert_eq!(allocs, 8);
    assert_eq!(
        exact, rows,
        "every row must be bit-identical (prediction P9)"
    );
}
