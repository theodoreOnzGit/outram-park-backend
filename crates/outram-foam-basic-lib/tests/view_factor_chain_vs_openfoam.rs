//! End-to-end `viewFactor` chain against ESI OpenFOAM-v2506: Rust `viewFactorsGen` -> Rust
//! `viewFactor::calculate` -> `qr` (gh:#1541, under gh:#1529 / gh:#1540).
//!
//! # Method
//! For each closed case the chain starts from the upstream polyMesh only
//! (`reference/vfgen/<case>/{points,faces,boundary}.gz`): `generate_view_factors` builds `F` and
//! `globalFaceFaces`, `|Sf|` is the Rust face area, `ViewFactorModel::new` + `solve` (direct LU,
//! variable or `constantEmissivity`, no smoothing, no agglomeration) gives `qr`. Wall T, emissivity
//! and `qro` are case inputs (T and emissivity read from the upstream dump `ref_<case>.txt.gz`, qro
//! from the case `0/qr`: 1000 W/m2 on zmax for cubeA/cubeC, else 0). Two `solve` calls (second after
//! T x 1.1), exactly as `vfDump.C`. The chain is NOT given upstream's F, globalFaceFaces or magSf.
//! The reference is upstream's `qr1` / `qr2`. cubeC and cubeD share cubeA's mesh.
//! Pass criterion: bit-identical on every face for the convex cases (pinned counts); the non-convex
//! lshape (F differs at 8 pairs, one of them -1.4e-4, gh:#1564) is a pinned known residual; the
//! identical count and worst difference are printed.
//!
//! # Results
//! `verification_and_validation/upstream_view_factor_gh1529/README.md`, section "viewFactor chain
//! (gh:#1541)"; predictions in `PREDICTIONS_chain.md` (committed before the first run).

use outram_foam_basic_lib::io::poly_mesh::face_centre_and_area;
use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_basic_lib::view_factor_generation::{
    generate_view_factors, ViewFactorGenInput, ViewFactorGenSettings,
};
use outram_foam_basic_lib::view_factor_radiation::{
    GreyDiffusiveViewFactorPatch, ViewFactorModel, ViewFactorPatch,
};
use std::collections::HashMap;

const DIR: &str = "verification_and_validation/upstream_view_factor_gh1529";

fn gunzip_bytes(path: &str) -> String {
    let data = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let body = &data[10..data.len() - 8];
    String::from_utf8(miniz_oxide::inflate::decompress_to_vec(body).expect("inflate")).unwrap()
}

/// Text after the `FoamFile { }` header with parentheses separated out.
fn body_tokens(text: &str) -> Vec<String> {
    let p = text.find("}\n").expect("FoamFile end");
    let body = &text[p + 2..];
    let mut clean = String::new();
    for line in body.lines() {
        if line.starts_with("//") {
            continue;
        }
        clean.push_str(line);
        clean.push('\n');
    }
    clean
        .replace('(', " ( ")
        .replace(')', " ) ")
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

fn parse_points(text: &str) -> Vec<Vector3> {
    let t = body_tokens(text);
    let n: usize = t[0].parse().unwrap();
    let nums: Vec<f64> = t[1..]
        .iter()
        .filter_map(|w| w.parse::<f64>().ok())
        .collect();
    assert_eq!(nums.len(), 3 * n);
    (0..n)
        .map(|i| Vector3::new(nums[3 * i], nums[3 * i + 1], nums[3 * i + 2]))
        .collect()
}

fn parse_faces(text: &str) -> Vec<Vec<usize>> {
    let t = body_tokens(text);
    let n: usize = t[0].parse().unwrap();
    // tokens: n ( k ( a b c d ) k ( ... ) )
    let nums: Vec<usize> = t[1..]
        .iter()
        .filter_map(|w| w.parse::<usize>().ok())
        .collect();
    let mut out = Vec::with_capacity(n);
    let mut p = 0;
    for _ in 0..n {
        let k = nums[p];
        out.push(nums[p + 1..p + 1 + k].to_vec());
        p += 1 + k;
    }
    out
}

/// `(startFace, nFaces)` per patch, in file order.
fn parse_patches(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut n = 0;
    for line in text.lines() {
        let t: Vec<&str> = line
            .split(|c: char| c.is_whitespace() || c == ';')
            .filter(|s| !s.is_empty())
            .collect();
        if t.len() >= 2 && t[0] == "nFaces" {
            n = t[1].parse().unwrap();
        }
        if t.len() >= 2 && t[0] == "startFace" {
            out.push((t[1].parse().unwrap(), n));
        }
    }
    out
}


fn hex(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).expect("hex"))
}

/// Columns of an upstream dump (`key index hex`), plus the patch sizes.
fn parse_dump(text: &str) -> (HashMap<String, Vec<f64>>, Vec<usize>) {
    let mut cols: HashMap<String, Vec<f64>> = HashMap::new();
    let mut sizes = Vec::new();
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.is_empty() || t[0] == "#" {
            continue;
        }
        match t[0] {
            "patch" => sizes.push(t[4].parse().unwrap()),
            "Frow" | "gff" | "F" => {}
            k => cols.entry(k.to_string()).or_default().push(hex(t[2])),
        }
    }
    (cols, sizes)
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

/// (identical, total, worst relative difference) for call 1 and call 2.
fn run_chain(mesh_case: &str, ref_case: &str, const_e: bool, qro_last: f64) -> [(usize, usize, f64); 2] {
    let root = env!("CARGO_MANIFEST_DIR");
    let mesh = |n: &str| gunzip_bytes(&format!("{root}/{DIR}/reference/vfgen/{mesh_case}/{n}.gz"));
    let points = parse_points(&mesh("points"));
    let faces = parse_faces(&mesh("faces"));
    let patches = parse_patches(&mesh("boundary"));
    let patch_faces: Vec<Vec<Vec<usize>>> = patches
        .iter()
        .map(|&(start, n)| faces[start..start + n].to_vec())
        .collect();
    let areas: Vec<Vec<f64>> = patch_faces
        .iter()
        .map(|p| p.iter().map(|f| face_centre_and_area(&points, f).1.mag()).collect())
        .collect();
    let g = generate_view_factors(
        &ViewFactorGenInput { points, patch_faces },
        ViewFactorGenSettings::default(),
    )
    .expect("generate");

    let (cols, sizes) = parse_dump(&gunzip_bytes(&format!("{root}/{DIR}/reference/ref_{ref_case}.txt.gz")));
    assert_eq!(sizes, areas.iter().map(Vec::len).collect::<Vec<_>>(), "patch sizes");
    let vp: Vec<ViewFactorPatch> = areas
        .iter()
        .enumerate()
        .map(|(i, a)| ViewFactorPatch {
            name: format!("p{i}"),
            fine_areas: a.clone(),
            agglom: (0..a.len()).collect(),
        })
        .collect();
    let mut m = ViewFactorModel::new(vp, &g.global_face_faces, &g.f, false, const_e).expect("model");
    let emis = split(&cols["emis"], &sizes);
    let last = sizes.len() - 1;
    let mut bcs: Vec<GreyDiffusiveViewFactorPatch> = sizes
        .iter()
        .enumerate()
        .map(|(i, &n)| GreyDiffusiveViewFactorPatch::new(vec![if i == last { qro_last } else { 0.0 }; n]))
        .collect();
    let mut out = [(0, 0, 0.0); 2];
    for (k, (tkey, qkey)) in [("Tb", "qr1"), ("Tb2", "qr2")].iter().enumerate() {
        let t = split(&cols[*tkey], &sizes);
        m.solve(&t, &emis, &mut bcs).expect("solve");
        let got: Vec<f64> = bcs.iter().flat_map(|b| b.value.iter().copied()).collect();
        let want = &cols[*qkey];
        assert_eq!(got.len(), want.len());
        let (mut same, mut worst) = (0, 0.0_f64);
        for (a, b) in got.iter().zip(want) {
            if a.to_bits() == b.to_bits() || (*a == 0.0 && *b == 0.0) {
                same += 1;
            } else {
                worst = worst.max(((a - b) / b).abs());
            }
        }
        out[k] = (same, got.len(), worst);
    }
    out
}

/// Convex enclosures: the whole chain is bit-identical to ESI.
#[test]
fn convex_chain_is_bit_identical_to_esi_qr() {
    let mut total = 0;
    for (mesh, rf, ce, qro) in [
        ("cubeA", "cubeA", false, 1000.0),
        ("cubeA", "cubeC", true, 1000.0),
        ("cubeA", "cubeD", false, 0.0),
        ("plate4", "plate4", false, 0.0),
        ("plate8", "plate8", false, 0.0),
        ("plate16", "plate16", false, 0.0),
        ("boxE", "boxE", false, 0.0),
    ] {
        let r = run_chain(mesh, rf, ce, qro);
        println!("{rf}: call1 {}/{} worst {:e}; call2 {}/{} worst {:e}", r[0].0, r[0].1, r[0].2, r[1].0, r[1].1, r[1].2);
        for c in r {
            assert_eq!(c.0, c.1, "{rf}: not bit-identical (worst {:e})", c.2);
            total += c.1;
        }
    }
    assert_eq!(total, 1660);
}

/// Non-convex lshape: KNOWN RESIDUAL, pinned (not a pass). Upstream lists the coplanar pair (106, 108)
/// with `F = -1.3878e-4` (a quadrature artefact, NOT rounding noise as gh:#1540 first recorded); the
/// Rust generator does not list it (gh:#1564), so `qr` differs on every face by up to ~2.1e-5 relative
/// (measured 2026-11-14: 0/126 bit-identical in both calls, worst 2.0902e-5). The test asserts the
/// residual is present and bounded; when gh:#1564 is fixed it fails and must be changed to the
/// bit-identity gate of the convex cases. The threshold was NOT chosen to make a gate pass: the
/// prediction (worst < 1e-10) was wrong and is recorded as wrong.
#[test]
fn lshape_chain_known_residual_is_pinned() {
    let r = run_chain("lshape", "lshape", false, 0.0);
    println!("lshape: call1 {}/{} worst {:e}; call2 {}/{} worst {:e}", r[0].0, r[0].1, r[0].2, r[1].0, r[1].1, r[1].2);
    for c in r {
        assert_eq!(c.0, 0, "lshape now has bit-identical faces: gh:#1564 progress, update this pin");
        assert!(c.2 > 1e-6 && c.2 < 1e-4, "lshape worst {:e}", c.2);
    }
}
