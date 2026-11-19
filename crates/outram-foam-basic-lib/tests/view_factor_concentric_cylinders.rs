//! Grey exchange between infinitely long concentric cylinders against the closed form
//! (gh:#1529, prediction 8 in `verification_and_validation/upstream_view_factor_gh1529/`).
//!
//! # Method
//! The two cylinders (inner radius 0.5 m at 1000 K, emissivity 0.8; outer radius 1.0 m at
//! 500 K, emissivity 0.5) are replaced by N-facet inscribed polygons per unit depth. The 2-D
//! view factor between facets is the double line integral
//! `F_ij = (1/L_i) int int cos(t_i) cos(t_j)/(2 d) dl_i dl_j` by the midpoint rule with 40
//! points per facet and a visibility test against the (convex) inner polygon for
//! outer-to-outer pairs. These F, one coarse face per facet, go to
//! `ViewFactorModel::solve` (the ported ESI direct solver; coarse patches inner/outer).
//!
//! # Reference
//! Siegel & Howell, *Thermal Radiation Heat Transfer*, 5th ed., Eq. for concentric
//! cylinders; Modest, *Radiative Heat Transfer*, 3rd ed., Eq. (5.28):
//! `q1 = sigma (T1^4 - T2^4) / (1/e1 + (r1/r2)(1/e2 - 1))` per unit inner area, with each
//! radius taken as perimeter/(2 pi) of its polygon, so the only error is the quadrature and
//! the polygon departure from a circle... which also moves the F values themselves.
//!
//! # Results
//! `verification_and_validation/upstream_view_factor_gh1529/README.md`.

use outram_foam_basic_lib::radiation::stefan_boltzmann_esi_v2506;
use outram_foam_basic_lib::view_factor_radiation::{
    GreyDiffusiveViewFactorPatch, ViewFactorModel, ViewFactorPatch,
};

type P = (f64, f64);

fn polygon(r: f64, n: usize) -> Vec<P> {
    (0..n)
        .map(|k| {
            let a = 2.0 * std::f64::consts::PI * (k as f64) / (n as f64);
            (r * a.cos(), r * a.sin())
        })
        .collect()
}

/// Segment `a-b` properly crosses segment `c-d`.
fn crosses(a: P, b: P, c: P, d: P) -> bool {
    let o = |p: P, q: P, r: P| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2, d3, d4) = (o(a, b, c), o(a, b, d), o(c, d, a), o(c, d, b));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

struct Facet {
    a: P,
    b: P,
    n: P, // unit normal pointing into the enclosed gas
    len: f64,
}

fn facets(poly: &[P], gas_outside: bool) -> Vec<Facet> {
    let n = poly.len();
    (0..n)
        .map(|k| {
            let (a, b) = (poly[k], poly[(k + 1) % n]);
            let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            // counter-clockwise polygon: the right-hand normal points outward
            let out = ((b.1 - a.1) / len, -(b.0 - a.0) / len);
            let nn = if gas_outside { out } else { (-out.0, -out.1) };
            Facet { a, b, n: nn, len }
        })
        .collect()
}

const M: usize = 40;

fn point(f: &Facet, k: usize) -> P {
    let s = (k as f64 + 0.5) / (M as f64);
    (f.a.0 + s * (f.b.0 - f.a.0), f.a.1 + s * (f.b.1 - f.a.1))
}

fn f_ij(fi: &Facet, fj: &Facet, blocker: &[P]) -> f64 {
    let mut sum = 0.0;
    for p in 0..M {
        let pp = point(fi, p);
        for q in 0..M {
            let qq = point(fj, q);
            let d = (qq.0 - pp.0, qq.1 - pp.1);
            let dist = (d.0 * d.0 + d.1 * d.1).sqrt();
            let ci = (fi.n.0 * d.0 + fi.n.1 * d.1) / dist;
            let cj = -(fj.n.0 * d.0 + fj.n.1 * d.1) / dist;
            if ci <= 0.0 || cj <= 0.0 {
                continue;
            }
            let nb = blocker.len();
            let blocked = (0..nb).any(|k| crosses(pp, qq, blocker[k], blocker[(k + 1) % nb]));
            if blocked {
                continue;
            }
            sum += ci * cj / (2.0 * dist);
        }
    }
    sum * fj.len / (M as f64) * fi.len / (M as f64) / fi.len
}

/// (relative error of the inner flux, energy balance, worst row-sum error)
fn run(n: usize) -> (f64, f64, f64) {
    let (r1, r2) = (0.5, 1.0);
    let pin = polygon(r1, n);
    let pout = polygon(r2, n);
    let fin = facets(&pin, true);
    let fout = facets(&pout, false);
    let all: Vec<&Facet> = fin.iter().chain(fout.iter()).collect();
    let nn = 2 * n;
    let mut gff = vec![Vec::new(); nn];
    let mut frows = vec![Vec::new(); nn];
    let mut rowsum = vec![0.0; nn];
    for i in 0..nn {
        for j in 0..nn {
            if i == j {
                continue;
            }
            // only outer-to-outer rays can be blocked by the (convex) inner polygon; an
            // inner point lies ON its polygon, so testing it would be rounding noise
            let blocker: &[P] = if i >= n && j >= n { &pin } else { &[] };
            let f = f_ij(all[i], all[j], blocker);
            if f > 0.0 {
                gff[i].push(j);
                frows[i].push(f);
                rowsum[i] += f;
            }
        }
    }
    let patches = vec![
        ViewFactorPatch { name: "inner".into(), fine_areas: fin.iter().map(|f| f.len).collect(), agglom: (0..n).collect() },
        ViewFactorPatch { name: "outer".into(), fine_areas: fout.iter().map(|f| f.len).collect(), agglom: (0..n).collect() },
    ];
    let mut m = ViewFactorModel::new(patches, &gff, &frows, false, false).expect("model");
    let mut bcs = vec![
        GreyDiffusiveViewFactorPatch::new(vec![0.0; n]),
        GreyDiffusiveViewFactorPatch::new(vec![0.0; n]),
    ];
    let sol = m
        .solve(
            &[vec![1000.0; n], vec![500.0; n]],
            &[vec![0.8; n], vec![0.5; n]],
            &mut bcs,
        )
        .expect("solve");
    let per1: f64 = fin.iter().map(|f| f.len).sum::<f64>();
    let per2: f64 = fout.iter().map(|f| f.len).sum::<f64>();
    let (re1, re2) = (per1 / (2.0 * std::f64::consts::PI), per2 / (2.0 * std::f64::consts::PI));
    let sg = stefan_boltzmann_esi_v2506();
    let q_exact = sg * (1000f64.powi(4) - 500f64.powi(4)) / (1.0 / 0.8 + (re1 / re2) * (1.0 / 0.5 - 1.0));
    let q_model = -sol.q_coarse[..n].iter().sum::<f64>() / (n as f64);
    let bal: f64 = (0..nn).map(|i| all[i].len * sol.q_coarse[i]).sum();
    let mag: f64 = (0..nn).map(|i| (all[i].len * sol.q_coarse[i]).abs()).sum();
    let worst_row = rowsum.iter().map(|s| (s - 1.0).abs()).fold(0.0, f64::max);
    println!("RESULT cylinders N={n} q_model {q_model:.2} q_exact {q_exact:.2} err {:.4}% balance {:e} worst rowsum err {:e}",
        100.0 * (q_model / q_exact - 1.0), bal / mag, worst_row);
    (q_model / q_exact - 1.0, bal / mag, worst_row)
}

#[test]
fn concentric_cylinders_converge_to_closed_form() {
    let e16 = run(16).0.abs();
    let e32 = run(32).0.abs();
    let e64 = run(64).0.abs();
    assert!(e32 < 0.02, "N=32 error {e32}");
    assert!(e16 > e32 && e32 > e64, "not monotone: {e16} {e32} {e64}");
    let (_, bal, _) = run(32);
    assert!(bal.abs() < 1e-3, "balance {bal}");
}
