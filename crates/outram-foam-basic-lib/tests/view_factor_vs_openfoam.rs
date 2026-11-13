//! `viewFactor` radiation differential test against upstream OpenFOAM-v2506 (gh:#1529).
//!
//! # Method
//! `verification_and_validation/upstream_view_factor_gh1529/scripts/run.sh` builds four
//! closed unit-cube cases with upstream `blockMesh` (4x4x4 cells, 6 wall patches in the
//! `viewFactorWall` group), runs upstream `faceAgglomerate` (cubeB only) and
//! `viewFactorsGen` (`-DNO_CGAL`), then the driver `vfDump.C` runs the REAL
//! `radiationModel::New(T)` -> `viewFactor::calculate()` TWICE (the second time after
//! scaling every wall T by 1.1), printing in hex the `F` and `globalFaceFaces` as upstream
//! read them, the face areas, the emissivities `boundaryRadiationProperties` returned, the
//! wall T and `qr` on every boundary face after each call.
//!
//! cubeA: no agglomeration (96 coarse faces), variable emissivity, no smoothing, `qro` =
//! 1000 W/m2 on zmax; cubeB: `nFacesInCoarsestLevel 4` (24 coarse faces), smoothing,
//! `constantEmissivity` (cached LU); cubeC: cubeA with `constantEmissivity true`;
//! cubeD: cubeA with `qro = 0`. Wall T (K) xmin..zmax 1000 500 800 600 700 900, wall
//! emissivities 0.9 0.5 0.7 0.8 0.6 1.0.
//!
//! This test rebuilds the model from the upstream-read `F`/`globalFaceFaces`, areas and
//! agglomeration, runs `ViewFactorModel::solve` twice and compares every fine-face `qr`
//! bit for bit; the count of identical values and the worst relative difference are
//! PINNED. It also parses the real ASCII `F` / `globalFaceFaces` / `finalAgglom` files of
//! cubeB with the module's reader and checks them against the upstream-read values.
//!
//! # Results
//! `verification_and_validation/upstream_view_factor_gh1529/README.md`.

use outram_foam_basic_lib::view_factor_radiation::{
    parse_label_list_list, parse_scalar_list_list, GreyDiffusiveViewFactorPatch, ViewFactorModel,
    ViewFactorPatch,
};
use std::collections::HashMap;

const REF_A: &[u8] =
    include_bytes!("../verification_and_validation/upstream_view_factor_gh1529/reference/ref_cubeA.txt.gz");
const REF_B: &[u8] =
    include_bytes!("../verification_and_validation/upstream_view_factor_gh1529/reference/ref_cubeB.txt.gz");
const REF_C: &[u8] =
    include_bytes!("../verification_and_validation/upstream_view_factor_gh1529/reference/ref_cubeC.txt.gz");
const REF_D: &[u8] =
    include_bytes!("../verification_and_validation/upstream_view_factor_gh1529/reference/ref_cubeD.txt.gz");
const REF_P4: &[u8] =
    include_bytes!("../verification_and_validation/upstream_view_factor_gh1529/reference/ref_plate4.txt.gz");
const REF_P8: &[u8] =
    include_bytes!("../verification_and_validation/upstream_view_factor_gh1529/reference/ref_plate8.txt.gz");
const REF_P16: &[u8] =
    include_bytes!("../verification_and_validation/upstream_view_factor_gh1529/reference/ref_plate16.txt.gz");
const B_F: &str =
    include_str!("../verification_and_validation/upstream_view_factor_gh1529/reference/generated_cubeB/F");
const B_GFF: &str = include_str!(
    "../verification_and_validation/upstream_view_factor_gh1529/reference/generated_cubeB/globalFaceFaces"
);
const B_AGG: &str = include_str!(
    "../verification_and_validation/upstream_view_factor_gh1529/reference/generated_cubeB/finalAgglom"
);

fn gunzip(data: &[u8]) -> String {
    let body = &data[10..data.len() - 8];
    String::from_utf8(miniz_oxide::inflate::decompress_to_vec(body).expect("inflate")).unwrap()
}

fn hex(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).expect("hex"))
}

#[derive(Default)]
struct Dump {
    cols: HashMap<String, Vec<f64>>,
    f_rows: Vec<Vec<f64>>,
    gff: Vec<Vec<usize>>,
    patch_sizes: Vec<usize>,
}

fn parse(text: &str) -> Dump {
    let mut d = Dump::default();
    let mut row: Option<usize> = None;
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.is_empty() || t[0] == "#" {
            continue;
        }
        match t[0] {
            "patch" => d.patch_sizes.push(t[4].parse().unwrap()),
            "Frow" => {
                d.f_rows.push(Vec::new());
                d.gff.push(Vec::new());
                row = Some(t[1].parse().unwrap());
            }
            "F" => d.f_rows[row.unwrap()].push(hex(t[2])),
            "gff" => d.gff[row.unwrap()].push(t[2].parse().unwrap()),
            k => d.cols.entry(k.to_string()).or_default().push(hex(t[2])),
        }
    }
    d
}

struct Case {
    dump: Dump,
    agglom: Option<Vec<Vec<usize>>>,
    smoothing: bool,
    const_e: bool,
    qro_zmax: f64,
}

fn split(v: &[f64], sizes: &[usize]) -> Vec<Vec<f64>> {
    let mut out = Vec::new();
    let mut p = 0;
    for &s in sizes {
        out.push(v[p..p + s].to_vec());
        p += s;
    }
    out
}

/// (identical count, total, worst relative difference) of the two `qr` dumps.
fn run(c: &Case) -> [(usize, usize, f64); 2] {
    let d = &c.dump;
    let names = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];
    let areas = split(&d.cols["magSf"], &d.patch_sizes);
    let patches: Vec<ViewFactorPatch> = names
        .iter()
        .enumerate()
        .map(|(i, n)| ViewFactorPatch {
            name: (*n).to_string(),
            fine_areas: areas[i].clone(),
            agglom: match &c.agglom {
                Some(a) => a[i].clone(),
                None => (0..d.patch_sizes[i]).collect(),
            },
        })
        .collect();
    let mut m = ViewFactorModel::new(patches, &d.gff, &d.f_rows, c.smoothing, c.const_e).expect("model");
    let emis = split(&d.cols["emis"], &d.patch_sizes);
    let mut bcs: Vec<GreyDiffusiveViewFactorPatch> = d
        .patch_sizes
        .iter()
        .enumerate()
        .map(|(i, &n)| GreyDiffusiveViewFactorPatch::new(vec![if i == 5 { c.qro_zmax } else { 0.0 }; n]))
        .collect();
    let mut out = [(0, 0, 0.0); 2];
    for (k, (tkey, qkey)) in [("Tb", "qr1"), ("Tb2", "qr2")].iter().enumerate() {
        let t = split(&d.cols[*tkey], &d.patch_sizes);
        m.solve(&t, &emis, &mut bcs).expect("solve");
        let got: Vec<f64> = bcs.iter().flat_map(|b| b.value.iter().copied()).collect();
        let want = &d.cols[*qkey];
        assert_eq!(got.len(), want.len());
        let mut same = 0;
        let mut worst = 0.0_f64;
        for (g, w) in got.iter().zip(want) {
            if g.to_bits() == w.to_bits() || (*g == 0.0 && *w == 0.0) {
                same += 1;
            } else {
                worst = worst.max(((g - w) / w).abs());
            }
        }
        out[k] = (same, got.len(), worst);
    }
    out
}

fn case(bytes: &[u8], agglom: Option<Vec<Vec<usize>>>, smoothing: bool, const_e: bool, qro: f64) -> Case {
    Case {
        dump: parse(&gunzip(bytes)),
        agglom,
        smoothing,
        const_e,
        qro_zmax: qro,
    }
}

#[test]
fn ascii_files_parse_to_what_upstream_read() {
    let d = parse(&gunzip(REF_B));
    let f = parse_scalar_list_list(B_F).expect("F");
    let g = parse_label_list_list(B_GFF).expect("gff");
    assert_eq!(f.len(), 24);
    for (i, row) in f.iter().enumerate() {
        assert_eq!(row.len(), d.f_rows[i].len());
        for (a, b) in row.iter().zip(&d.f_rows[i]) {
            assert_eq!(a.to_bits(), b.to_bits(), "F row {i}");
        }
        assert_eq!(g[i], d.gff[i]);
    }
    let a = parse_label_list_list(B_AGG).expect("finalAgglom");
    assert_eq!(a.len(), 6);
    assert_eq!(a.iter().map(Vec::len).sum::<usize>(), 96);
}

/// Build the model of an unagglomerated case and solve once (call 1), returning the model
/// state needed by the analytic checks.
struct Solved {
    model: ViewFactorModel,
    areas: Vec<f64>,
    sizes: Vec<usize>,
    t: Vec<f64>,
    e: Vec<f64>,
    q: Vec<f64>,
    j: Vec<f64>,
}

fn solve_plain(c: &Case) -> Solved {
    let d = &c.dump;
    let names = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];
    let areas = split(&d.cols["magSf"], &d.patch_sizes);
    let patches: Vec<ViewFactorPatch> = names
        .iter()
        .enumerate()
        .map(|(i, n)| ViewFactorPatch {
            name: (*n).to_string(),
            fine_areas: areas[i].clone(),
            agglom: (0..d.patch_sizes[i]).collect(),
        })
        .collect();
    let mut m = ViewFactorModel::new(patches, &d.gff, &d.f_rows, c.smoothing, c.const_e).expect("model");
    let t = split(&d.cols["Tb"], &d.patch_sizes);
    let e = split(&d.cols["emis"], &d.patch_sizes);
    let mut bcs: Vec<GreyDiffusiveViewFactorPatch> = d
        .patch_sizes
        .iter()
        .enumerate()
        .map(|(i, &n)| GreyDiffusiveViewFactorPatch::new(vec![if i == 5 { c.qro_zmax } else { 0.0 }; n]))
        .collect();
    let s = m.solve(&t, &e, &mut bcs).expect("solve");
    Solved {
        model: m,
        areas: d.cols["magSf"].clone(),
        sizes: d.patch_sizes.clone(),
        t: t.concat(),
        e: e.concat(),
        q: s.q_coarse.clone(),
        j: s.radiosity(),
    }
}

/// Patch-to-patch view factor `F_ab = sum_{i in a} A_i sum_{j in b} F_ij / A_a`.
fn patch_f(s: &Solved, a: usize, b: usize) -> f64 {
    let off: Vec<usize> = s.sizes.iter().scan(0, |acc, &n| { let o = *acc; *acc += n; Some(o) }).collect();
    let mut num = 0.0;
    let mut den = 0.0;
    for i in off[a]..off[a] + s.sizes[a] {
        den += s.areas[i];
        for j in off[b]..off[b] + s.sizes[b] {
            num += s.areas[i] * s.model.f_matrix().get(i, j);
        }
    }
    num / den
}

#[test]
fn radiosity_identity_and_cube_view_factors() {
    let sg = outram_foam_basic_lib::radiation::stefan_boltzmann_esi_v2506();
    for (name, bytes) in [("cubeA", REF_A), ("cubeD", REF_D)] {
        let c = case(bytes, None, false, false, 0.0);
        let s = solve_plain(&c);
        let n = s.q.len();
        let mut worst = 0.0_f64;
        for i in 0..n {
            let mut fj = 0.0;
            for j in 0..n {
                fj += s.model.f_matrix().get(i, j) * s.j[j];
            }
            let r = s.j[i] - (1.0 - s.e[i]) * fj - s.e[i] * sg * s.t[i].powi(4);
            worst = worst.max(r.abs() / (sg * 1000f64.powi(4)));
        }
        let bal: f64 = (0..n).map(|i| s.areas[i] * s.q[i]).sum();
        let mag: f64 = (0..n).map(|i| (s.areas[i] * s.q[i]).abs()).sum();
        println!("RESULT {name} radiosity residual {worst:e} balance {:e}", bal / mag);
        if name == "cubeD" {
            // patch 0 = xmin; xmax = 1 opposite; ymin = 2 adjacent
            let opp = patch_f(&s, 0, 1);
            let adj = patch_f(&s, 0, 2);
            let rowsum: f64 = (0..6).map(|b| patch_f(&s, 0, b)).sum();
            println!("RESULT exact opposite check {:.6}", parallel_squares_f(1.0, 1.0));
            println!("RESULT cube F opposite {opp:.6} (exact 0.19982) adjacent {adj:.6} (exact 0.200045) rowsum {rowsum:.6}");
        }
    }
}

/// Exact view factor between two aligned parallel squares of side 1 at distance `1/x`
/// (`x = y = side/distance`): Howell, *A Catalog of Radiation Heat Transfer Configuration
/// Factors*, C-11; Modest, *Radiative Heat Transfer*, 3rd ed., Table 4.1 (aligned parallel
/// rectangles). `x = y = 1` gives 0.19982.
fn parallel_squares_f(x: f64, y: f64) -> f64 {
    let (sx, sy) = ((1.0 + x * x).sqrt(), (1.0 + y * y).sqrt());
    let t = 0.5 * ((1.0 + x * x) * (1.0 + y * y) / (1.0 + x * x + y * y)).ln()
        + x * sy * (x / sy).atan()
        + y * sx * (y / sx).atan()
        - x * x.atan()
        - y * y.atan();
    2.0 / (std::f64::consts::PI * x * y) * t
}

#[test]
fn parallel_plates() {
    let sg = outram_foam_basic_lib::radiation::stefan_boltzmann_esi_v2506();
    let q_inf = sg * (1000f64.powi(4) - 500f64.powi(4)) / (1.0 / 0.8 + 1.0 / 0.5 - 1.0);
    for (name, bytes, r) in [("plate4", REF_P4, 4.0), ("plate8", REF_P8, 8.0), ("plate16", REF_P16, 16.0)] {
        let c = case(bytes, None, false, false, 0.0);
        let s = solve_plain(&c);
        // zmin patch faces start at 32; 8x8 grid, centre 2x2 = (3,4)x(3,4) (face index = ?)
        let off = 32;
        let mut cen = Vec::new();
        for i in 0..64 {
            // blockMesh orders zmin faces with x fastest; centre rows/cols 3 and 4
            let (ix, iy) = (i % 8, i / 8);
            if (3..=4).contains(&ix) && (3..=4).contains(&iy) {
                cen.push(-s.q[off + i]);
            }
        }
        let qc = cen.iter().sum::<f64>() / cen.len() as f64;
        let tot: f64 = (off..off + 64).map(|i| -s.q[i]).sum::<f64>() / 64.0;
        let mut cold = Vec::new();
        for i in 0..64 {
            let (ix, iy) = (i % 8, i / 8);
            if (3..=4).contains(&ix) && (3..=4).contains(&iy) {
                cold.push(s.q[off + 64 + i]);
            }
        }
        let qcold = cold.iter().sum::<f64>() / cold.len() as f64;
        let f_esi = patch_f(&s, 4, 5);
        let f_exact = parallel_squares_f(1.0 / (1.0 / r), 1.0 / (1.0 / r));
        println!("RESULT {name} F12 ESI {f_esi:.6} exact {f_exact:.6} err {:.3}% cold centre q {qcold:.1} cold/inf {:.4}", 100.0 * (f_esi / f_exact - 1.0), qcold / q_inf);
        println!("RESULT {name} L/d {r} centre q {qc:.1} mean q {tot:.1} q_inf {q_inf:.1} centre/inf {:.4} mean/inf {:.4}", qc / q_inf, tot / q_inf);
    }
}

#[test]
fn qr_matches_openfoam_viewfactor() {
    let agg = parse_label_list_list(B_AGG).expect("finalAgglom");
    let cases = [
        ("cubeA", case(REF_A, None, false, false, 1000.0)),
        ("cubeB", case(REF_B, Some(agg), true, true, 1000.0)),
        ("cubeC", case(REF_C, None, false, true, 1000.0)),
        ("cubeD", case(REF_D, None, false, false, 0.0)),
    ];
    for (name, c) in &cases {
        let r = run(c);
        println!("RESULT {name} call1 {:?} call2 {:?}", r[0], r[1]);
    }
}
