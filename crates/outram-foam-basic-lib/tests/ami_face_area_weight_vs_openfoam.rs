// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! `face_area_weight_ami` differential test against upstream OpenFOAM ESI
//! (gh:#1859).
//!
//! # Method
//! `verification_and_validation/upstream_ami_gh1859/driver/amiDump.C` builds
//! each case's two patches, runs the UNMODIFIED ESI `faceAreaWeightAMI`
//! (commit `6e2595746a396d688edd9890f4aebaaa7ff4ecfd`, serial, dummy Pstream)
//! constructed from a dictionary, and writes the inputs (point coordinates as
//! IEEE-754 bit patterns, face lists, options) and the outputs (`srcAddress`,
//! `srcWeights`, `srcWeightsSum`, `srcMagSf`, `srcCentroids`,
//! `srcNonOverlap`, and the `tgt*` lists) to
//! `reference/ami_esi_6e25957.txt`. This test reads the same input bits, runs
//! the port with the same options, and compares every output.
//!
//! # Criterion
//! Bit-identity, address lists in the same order. The test counts, per case
//! and overall, the bit-identical values and the worst relative difference,
//! prints them (`--nocapture`), and fails on any address mismatch, on any
//! value off by more than 1e-9 relative, and on any value that is not
//! bit-identical.
//!
//! # Results
//! Measured 2026-11-27: 8579 of 8579 values bit-identical in 17 cases,
//! worst relative difference 0, every address list equal in order. Per-case
//! table: `verification_and_validation/upstream_ami_gh1859/README.md`.

use outram_foam_basic_lib::mesh::face_area_weight_ami::{
    face_area_weight_ami, AmiAreaNormalisation, AmiPatchGeometry, AmiTriangulationMode,
    FaceAreaWeightAmiOptions, FaceAreaWeightAmiResult,
};
use outram_foam_basic_lib::primitives::Vector3;

const REFERENCE: &str = include_str!(
    "../verification_and_validation/upstream_ami_gh1859/reference/ami_esi_6e25957.txt"
);

/// One upstream case: inputs and expected outputs.
struct Case {
    name: String,
    opts: FaceAreaWeightAmiOptions,
    src: AmiPatchGeometry,
    tgt: AmiPatchGeometry,
    src_address: Vec<Vec<usize>>,
    src_weights: Vec<Vec<f64>>,
    src_weights_sum: Vec<f64>,
    src_mag_sf: Vec<f64>,
    src_centroids: Vec<Vec<Vector3>>,
    src_non_overlap: Vec<usize>,
    tgt_address: Vec<Vec<usize>>,
    tgt_weights: Vec<Vec<f64>>,
    tgt_weights_sum: Vec<f64>,
    tgt_mag_sf: Vec<f64>,
}

fn f(tok: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(tok, 16).expect("hex bit pattern"))
}

struct Lines<'a> {
    it: std::iter::Peekable<std::str::Lines<'a>>,
}

impl<'a> Lines<'a> {
    fn next(&mut self) -> &'a str {
        loop {
            let l = self.it.next().expect("unexpected end of reference");
            if !l.starts_with('#') && !l.trim().is_empty() {
                return l;
            }
        }
    }
    /// A `"<tag> <n>"` header; returns `n`.
    fn header(&mut self, tag: &str) -> usize {
        let l = self.next();
        let mut w = l.split_whitespace();
        assert_eq!(w.next(), Some(tag), "expected {tag}, got {l}");
        w.next().unwrap().parse().unwrap()
    }
    fn geom(&mut self, tag: &str) -> AmiPatchGeometry {
        let n = self.header(&format!("{tag}_points"));
        let points = (0..n)
            .map(|_| {
                let v: Vec<f64> = self.next().split_whitespace().map(f).collect();
                Vector3::new(v[0], v[1], v[2])
            })
            .collect();
        let m = self.header(&format!("{tag}_faces"));
        let faces = (0..m).map(|_| self.counted_usize()).collect();
        AmiPatchGeometry { points, faces }
    }
    fn counted_usize(&mut self) -> Vec<usize> {
        let mut w = self.next().split_whitespace();
        let k: usize = w.next().unwrap().parse().unwrap();
        let v: Vec<usize> = w.map(|t| t.parse().unwrap()).collect();
        assert_eq!(v.len(), k);
        v
    }
    fn counted_f64(&mut self) -> Vec<f64> {
        let mut w = self.next().split_whitespace();
        let k: usize = w.next().unwrap().parse().unwrap();
        let v: Vec<f64> = w.map(f).collect();
        assert_eq!(v.len(), k);
        v
    }
    fn labels(&mut self, tag: &str) -> Vec<Vec<usize>> {
        let n = self.header(tag);
        (0..n).map(|_| self.counted_usize()).collect()
    }
    fn scalars(&mut self, tag: &str) -> Vec<Vec<f64>> {
        let n = self.header(tag);
        (0..n).map(|_| self.counted_f64()).collect()
    }
    fn list(&mut self, tag: &str) -> Vec<f64> {
        let n = self.header(tag);
        (0..n).map(|_| f(self.next().trim())).collect()
    }
    fn opt(&mut self, name: &str) -> &'a str {
        let l = self.next();
        let w: Vec<&str> = l.split_whitespace().collect();
        assert_eq!((w[0], w[1]), ("opt", name), "{l}");
        w[2]
    }
}

fn parse() -> Vec<Case> {
    let mut lines = Lines {
        it: REFERENCE.lines().peekable(),
    };
    let mut cases = Vec::new();
    while lines.it.peek().is_some() {
        if lines
            .it
            .peek()
            .is_some_and(|l| l.starts_with('#') || l.trim().is_empty())
        {
            lines.it.next();
            continue;
        }
        let l = lines.next();
        let name = l.strip_prefix("case ").expect("case header").to_string();
        let opts = FaceAreaWeightAmiOptions {
            require_match: lines.opt("require_match") == "1",
            reverse_target: lines.opt("reverse_target") == "1",
            low_weight_correction: f(lines.opt("low_weight_correction")),
            triangulation: match lines.opt("tri_mode") {
                "mesh" => AmiTriangulationMode::Mesh,
                "fan" => AmiTriangulationMode::Fan,
                other => panic!("tri_mode {other}"),
            },
            restart_uncovered_source_face: lines.opt("restart") == "1",
            max_distance2: f(lines.opt("max_distance2")),
            min_cos_angle: f(lines.opt("min_cos_angle")),
            area_normalisation: match lines.opt("area_norm") {
                "project" => AmiAreaNormalisation::Project,
                "mag" => AmiAreaNormalisation::Mag,
                other => panic!("area_norm {other}"),
            },
        };
        let src = lines.geom("src");
        let tgt = lines.geom("tgt");
        let src_address = lines.labels("src_address");
        let src_weights = lines.scalars("src_weights");
        let src_weights_sum = lines.list("src_weights_sum");
        let src_mag_sf = lines.list("src_mag_sf");
        let n = lines.header("src_centroids");
        let src_centroids = (0..n).map(|_| lines.counted_f64_triples()).collect();
        let k = lines.header("src_non_overlap");
        let src_non_overlap: Vec<usize> = if k == 0 {
            // the (empty) list line
            let _ = lines.it.next();
            Vec::new()
        } else {
            lines
                .next()
                .split_whitespace()
                .map(|t| t.parse().unwrap())
                .collect()
        };
        assert_eq!(src_non_overlap.len(), k);
        let tgt_address = lines.labels("tgt_address");
        let tgt_weights = lines.scalars("tgt_weights");
        let tgt_weights_sum = lines.list("tgt_weights_sum");
        let tgt_mag_sf = lines.list("tgt_mag_sf");
        assert_eq!(lines.next(), "end");
        cases.push(Case {
            name,
            opts,
            src,
            tgt,
            src_address,
            src_weights,
            src_weights_sum,
            src_mag_sf,
            src_centroids,
            src_non_overlap,
            tgt_address,
            tgt_weights,
            tgt_weights_sum,
            tgt_mag_sf,
        });
    }
    cases
}

impl<'a> Lines<'a> {
    fn counted_f64_triples(&mut self) -> Vec<Vector3> {
        let mut w = self.next().split_whitespace();
        let k: usize = w.next().unwrap().parse().unwrap();
        let v: Vec<f64> = w.map(f).collect();
        assert_eq!(v.len(), 3 * k);
        v.chunks(3)
            .map(|c| Vector3::new(c[0], c[1], c[2]))
            .collect()
    }
}

/// Running tally of one comparison.
#[derive(Default)]
struct Tally {
    n: usize,
    identical: usize,
    worst_rel: f64,
}

impl Tally {
    fn add(&mut self, got: f64, want: f64) {
        self.n += 1;
        if got.to_bits() == want.to_bits() {
            self.identical += 1;
        } else {
            let rel = (got - want).abs() / want.abs().max(f64::MIN_POSITIVE);
            self.worst_rel = self.worst_rel.max(rel);
        }
    }
    fn lists(&mut self, got: &[Vec<f64>], want: &[Vec<f64>]) {
        assert_eq!(got.len(), want.len());
        for (g, w) in got.iter().zip(want) {
            assert_eq!(g.len(), w.len());
            for (a, b) in g.iter().zip(w) {
                self.add(*a, *b);
            }
        }
    }
    fn flat(&mut self, got: &[f64], want: &[f64]) {
        assert_eq!(got.len(), want.len());
        for (a, b) in got.iter().zip(want) {
            self.add(*a, *b);
        }
    }
}

fn compare(c: &Case, r: &FaceAreaWeightAmiResult, t: &mut Tally) {
    assert_eq!(r.src_address, c.src_address, "{}: srcAddress", c.name);
    assert_eq!(r.tgt_address, c.tgt_address, "{}: tgtAddress", c.name);
    assert_eq!(
        r.src_non_overlap, c.src_non_overlap,
        "{}: srcNonOverlap",
        c.name
    );
    t.lists(&r.src_weights, &c.src_weights);
    t.flat(&r.src_weights_sum, &c.src_weights_sum);
    t.flat(&r.src_mag_sf, &c.src_mag_sf);
    t.lists(&r.tgt_weights, &c.tgt_weights);
    t.flat(&r.tgt_weights_sum, &c.tgt_weights_sum);
    t.flat(&r.tgt_mag_sf, &c.tgt_mag_sf);
    assert_eq!(r.src_centroids.len(), c.src_centroids.len());
    for (g, w) in r.src_centroids.iter().zip(&c.src_centroids) {
        assert_eq!(g.len(), w.len());
        for (a, b) in g.iter().zip(w) {
            t.add(a.x, b.x);
            t.add(a.y, b.y);
            t.add(a.z, b.z);
        }
    }
}

/// V&V (code-to-code, 2026-11-27): every case of the reference, every output
/// value. See the module docs for method and criterion; results in the
/// README.
#[test]
fn face_area_weight_ami_matches_openfoam_esi() {
    let cases = parse();
    assert!(!cases.is_empty());
    let mut total = Tally::default();
    for c in &cases {
        let r = face_area_weight_ami(&c.src, &c.tgt, &c.opts)
            .unwrap_or_else(|e| panic!("{}: {e}", c.name));
        let mut t = Tally::default();
        compare(c, &r, &mut t);
        println!(
            "{:40} {:5}/{:5} bit-identical, worst rel {:.3e}",
            c.name, t.identical, t.n, t.worst_rel
        );
        assert!(
            t.worst_rel <= 1e-9,
            "{}: worst rel {:e}",
            c.name,
            t.worst_rel
        );
        total.n += t.n;
        total.identical += t.identical;
        total.worst_rel = total.worst_rel.max(t.worst_rel);
    }
    println!(
        "TOTAL {} cases: {}/{} bit-identical, worst rel {:.3e}",
        cases.len(),
        total.identical,
        total.n,
        total.worst_rel
    );
    assert_eq!(
        total.identical, total.n,
        "non-identical values: see the per-case lines"
    );
}
