//! P1 radiation differential test against upstream OpenFOAM-v2506 (gh:#1379).
//!
//! # Method
//! `verification_and_validation/upstream_radiation_gh1379/scripts/run.sh` runs the REAL
//! ESI `radiationModel::New(T)` -> `P1::correct()` (driver `radDump.C`) on four cases:
//! `hexA` and `hexB` on the 316-cell skewed hex+prism blockMesh of gh:#1362 (patches
//! inlet `MarshakRadiation` eps 0.9, outlet `MarshakRadiationFixedTemperature` eps 0.6
//! Trad 900 K, walls `MarshakRadiation` eps 0.8; hexA a = e = 2 /m, E = 0; hexB a = 1.5,
//! e = 1.2 /m, E = 500 W/m3; T a polynomial of position, no libm), and a 40-cell slab
//! (xmin eps 0.8 at 1500 K, xmax eps 0.5 at 500 K, medium 1000 K, sides `zeroGradient`;
//! thick a = e = 2 /m, thin a = e = 0.5 /m). Solver: PCG/DIC, tolerance 1e-10, G from 0.
//! The dump holds the geometry, T, sigma, G (cells and boundary), qr, the Marshak
//! valueFraction/refValue/refGrad, deltaCoeffs, `Ru`, `Rp`, and the matrices of
//! `radiationModel::ST(T)` and `radiationModel::Sh` (with polynomial Cpv, he).
//! This test builds the port's mesh from the UPSTREAM geometry and T, runs
//! `P1Model::solve` for exactly the iteration count ESI reports (tolerance 0), and
//! compares every quantity bit for bit. Separately it records how many iterations the
//! port needs with ESI's tolerance (its residual normalisation differs, gh:#1362).
//!
//! # Criterion
//! Bit-identity; per quantity the count of bit-identical values (+0.0 and -0.0 count
//! as equal) and the worst relative difference are PINNED.
//!
//! # Results
//! `verification_and_validation/upstream_radiation_gh1379/README.md`.

use outram_foam_basic_lib::fields::boundary::bc::{BoundaryCondition, PatchField};
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::vol_field::VolScalarField;
use outram_foam_basic_lib::ldu_matrix::SolverSettings;
use outram_foam_basic_lib::mesh::fv_mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_basic_lib::radiation::{
    stefan_boltzmann_esi_v2506, ConstantAbsorptionEmission, P1Model, P1PatchModel,
};
use std::collections::HashMap;
use std::sync::Arc;

const HEX_A: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_radiation_gh1379/reference/ref_hexA.txt.gz"
);
const HEX_B: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_radiation_gh1379/reference/ref_hexB.txt.gz"
);
const SLAB_THICK: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_radiation_gh1379/reference/ref_slab_thick.txt.gz"
);
const SLAB_THIN: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_radiation_gh1379/reference/ref_slab_thin.txt.gz"
);

fn gunzip(data: &[u8]) -> String {
    // `gzip -n`: 10-byte header, deflate stream, 8-byte trailer.
    let body = &data[10..data.len() - 8];
    String::from_utf8(miniz_oxide::inflate::decompress_to_vec(body).expect("inflate")).unwrap()
}

fn hex(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).expect("hex"))
}

#[derive(Default)]
struct Ref {
    cols: HashMap<String, Vec<Vec<f64>>>,
    points: Vec<Vector3>,
    faces: Vec<Vec<usize>>,
    own: Vec<usize>,
    nei: Vec<usize>,
    patches: Vec<(String, usize, usize)>,
    n_cells: usize,
    n_int: usize,
    esi_iterations: usize,
}

fn parse(text: &str) -> Ref {
    let mut r = Ref::default();
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.is_empty() {
            continue;
        }
        if t[0] == "#" {
            if t[1] == "nCells" {
                r.n_cells = t[2].parse().unwrap();
                r.n_int = t[6].parse().unwrap();
            } else if t[1] == "solve" {
                // "... No Iterations 15"
                r.esi_iterations = t.last().unwrap().parse().unwrap();
            }
            continue;
        }
        match t[0] {
            "face" => r.faces.push(t[3..].iter().map(|x| x.parse().unwrap()).collect()),
            "own" => r.own.push(t[2].parse().unwrap()),
            "nei" => r.nei.push(t[2].parse().unwrap()),
            "patch" => r.patches.push((t[2].to_string(), t[3].parse().unwrap(), t[4].parse().unwrap())),
            "points" => r.points.push(Vector3::new(hex(t[2]), hex(t[3]), hex(t[4]))),
            key => r.cols.entry(key.to_string()).or_default().push(t[2..].iter().map(|x| hex(x)).collect()),
        }
    }
    r
}

impl Ref {
    fn s(&self, k: &str) -> Vec<f64> {
        self.cols[k].iter().map(|v| v[0]).collect()
    }
    fn v(&self, k: &str) -> Vec<Vector3> {
        self.cols[k].iter().map(|v| Vector3::new(v[0], v[1], v[2])).collect()
    }
    fn fv_mesh(&self) -> Arc<FvMesh> {
        let patches = self
            .patches
            .iter()
            .map(|(n, s, z)| {
                let kind = if n == "walls" { PatchKind::Wall } else { PatchKind::Patch };
                BoundaryPatch::new(n.clone(), *s, *z, kind)
            })
            .collect();
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(self.n_cells)
                .n_internal_faces(self.n_int)
                .owner(self.own.clone())
                .neighbour(self.nei.clone())
                .patches(patches)
                .cell_volumes(self.s("V"))
                .cell_centres(self.v("C"))
                .face_area_vectors(self.v("Sf"))
                .face_centres(self.v("Cf"))
                .build()
                .unwrap(),
        )
    }
    /// T from the dump: cells, and `Tb` per boundary face (patch order).
    fn t_field(&self, mesh: &Arc<FvMesh>) -> VolScalarField {
        let tb = self.s("Tb");
        let boundary = mesh
            .patches
            .iter()
            .map(|p| {
                let off = p.start - mesh.n_internal_faces;
                let vals = Field::new(tb[off..off + p.size].to_vec());
                PatchField { bc: BoundaryCondition::FixedField(vals.clone()), values: vals }
            })
            .collect();
        VolScalarField::new("T", mesh.clone(), Field::new(self.s("T")), boundary)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Stat {
    n: usize,
    identical: usize,
}

fn ord(x: f64) -> i64 {
    let b = x.to_bits() as i64;
    if b < 0 {
        i64::MIN - b
    } else {
        b
    }
}

fn cmp(label: &str, rust: &[f64], up: &[f64]) -> Stat {
    assert_eq!(rust.len(), up.len(), "{label}: length");
    let (mut same, mut max_ulp, mut max_rel) = (0, 0u64, 0.0_f64);
    for (a, b) in rust.iter().zip(up) {
        if a.to_bits() == b.to_bits() || (*a == 0.0 && *b == 0.0) {
            same += 1;
            continue;
        }
        max_ulp = max_ulp.max((ord(*a) - ord(*b)).unsigned_abs());
        max_rel = max_rel.max((a - b).abs() / b.abs().max(f64::MIN_POSITIVE));
    }
    println!(
        "{label:<22} n={:<6} identical={:<6} max_ulp={:<3} max_rel={:.3e}",
        rust.len(),
        same,
        max_ulp,
        max_rel
    );
    Stat { n: rust.len(), identical: same }
}

/// Boundary values of one per-patch quantity, in global boundary-face order.
fn flat(per_patch: impl Iterator<Item = Vec<f64>>) -> Vec<f64> {
    per_patch.flatten().collect()
}

struct CaseSpec {
    label: &'static str,
    props: ConstantAbsorptionEmission,
    patch_model: fn(&str) -> P1PatchModel,
}

fn hex_patch(name: &str) -> P1PatchModel {
    match name {
        "inlet" => P1PatchModel::Marshak { emissivity: 0.9 },
        "outlet" => P1PatchModel::MarshakFixedTemperature { emissivity: 0.6, t_rad: 900.0 },
        "walls" => P1PatchModel::Marshak { emissivity: 0.8 },
        other => panic!("unexpected patch {other}"),
    }
}

fn slab_patch(name: &str) -> P1PatchModel {
    match name {
        "xmin" => P1PatchModel::Marshak { emissivity: 0.8 },
        "xmax" => P1PatchModel::Marshak { emissivity: 0.5 },
        "sides" => P1PatchModel::ZeroGradient,
        other => panic!("unexpected patch {other}"),
    }
}

/// Run one case; returns the stats of every compared quantity, in a fixed order.
fn run_case(geom: &Ref, r: &Ref, spec: &CaseSpec) -> Vec<(&'static str, Stat)> {
    let mesh = geom.fv_mesh();
    let t = r.t_field(&mesh);
    let model = P1Model::new(
        spec.props,
        geom.patches.iter().map(|(n, _, _)| (spec.patch_model)(n)).collect(),
    );
    println!("--- {}: ESI PCG iterations {}", spec.label, r.esi_iterations);
    // sigma
    assert_eq!(stefan_boltzmann_esi_v2506().to_bits(), r.s("sigma")[0].to_bits(), "sigma");

    // Same iteration count as ESI, tolerance 0: separates arithmetic from the residual stop.
    let sol = model.solve(&t, SolverSettings { tolerance: 0.0, max_iter: r.esi_iterations });
    let mut out = Vec::new();
    out.push(("G", cmp("G cells", &sol.g.internal.iter().copied().collect::<Vec<_>>(), &r.s("G"))));
    out.push((
        "Gb",
        cmp("G boundary", &flat(sol.g.boundary.iter().map(|p| p.values.iter().copied().collect())), &r.s("Gb")),
    ));
    out.push((
        "qr",
        cmp("qr boundary", &flat(sol.qr.iter().map(|q| q.iter().copied().collect())), &r.s("qrb")),
    ));
    // Marshak coefficients (only mixed patches; the dump holds -1 elsewhere).
    let mut vf = Vec::new();
    let mut rv = Vec::new();
    for p in &sol.g.boundary {
        match &p.bc {
            BoundaryCondition::MixedField { value_fraction, ref_value, .. } => {
                vf.extend(value_fraction.iter().copied());
                rv.extend(ref_value.iter().copied());
            }
            _ => {
                vf.extend(std::iter::repeat(-1.0).take(p.values.len()));
                rv.extend(std::iter::repeat(-1.0).take(p.values.len()));
            }
        }
    }
    out.push(("vf", cmp("valueFraction", &vf, &r.s("vf"))));
    out.push(("refv", cmp("refValue", &rv, &r.s("refv"))));
    out.push(("Ru", cmp("Ru", &sol.ru(), &r.s("Ru"))));
    out.push(("Rp", cmp("Rp", &sol.rp(), &r.s("Rp"))));

    // ST and Sh: the port's matrices are upstream's negated.
    let neg = |v: &[f64]| v.iter().map(|x| -x).collect::<Vec<_>>();
    let st = sol.st_matrix();
    out.push(("st_diag", cmp("ST diag", &neg(&st.ldu.diag), &r.s("st_diag"))));
    out.push(("st_source", cmp("ST source", &neg(&st.source.iter().copied().collect::<Vec<_>>()), &r.s("st_source"))));
    let sh = sol.sh_matrix(&r.s("Cpv"), &r.s("he"));
    out.push(("sh_diag", cmp("Sh diag", &neg(&sh.ldu.diag), &r.s("sh_diag"))));
    out.push(("sh_source", cmp("Sh source", &neg(&sh.source.iter().copied().collect::<Vec<_>>()), &r.s("sh_source"))));

    // The port's own stopping rule at ESI's tolerance (not gated, recorded).
    let own = model.solve(&t, SolverSettings { tolerance: 1e-10, max_iter: 1000 });
    println!(
        "port PCG at tolerance 1e-10: {} iterations (ESI {}), residual {:.3e}",
        own.performance.n_iterations, r.esi_iterations, own.performance.final_residual
    );
    out
}

fn assert_all_identical(res: &[(&str, Stat)]) {
    for (k, s) in res {
        assert_eq!(s.identical, s.n, "{k}: {} of {} bit-identical", s.identical, s.n);
    }
}

#[test]
fn hex_a_matches_openfoam() {
    let r = parse(&gunzip(HEX_A));
    let res = run_case(
        &r,
        &r,
        &CaseSpec {
            label: "hexA",
            props: ConstantAbsorptionEmission { absorptivity: 2.0, emissivity: 2.0, emission_source: 0.0 },
            patch_model: hex_patch,
        },
    );
    assert_all_identical(&res);
}

#[test]
fn hex_b_matches_openfoam() {
    let g = parse(&gunzip(HEX_A));
    let r = parse(&gunzip(HEX_B));
    let res = run_case(
        &g,
        &r,
        &CaseSpec {
            label: "hexB",
            props: ConstantAbsorptionEmission { absorptivity: 1.5, emissivity: 1.2, emission_source: 500.0 },
            patch_model: hex_patch,
        },
    );
    assert_all_identical(&res);
}

#[test]
fn slab_thick_matches_openfoam() {
    let r = parse(&gunzip(SLAB_THICK));
    let res = run_case(
        &r,
        &r,
        &CaseSpec {
            label: "slabThick",
            props: ConstantAbsorptionEmission { absorptivity: 2.0, emissivity: 2.0, emission_source: 0.0 },
            patch_model: slab_patch,
        },
    );
    assert_all_identical(&res);
}

#[test]
fn slab_thin_matches_openfoam() {
    let g = parse(&gunzip(SLAB_THICK));
    let r = parse(&gunzip(SLAB_THIN));
    let res = run_case(
        &g,
        &r,
        &CaseSpec {
            label: "slabThin",
            props: ConstantAbsorptionEmission { absorptivity: 0.5, emissivity: 0.5, emission_source: 0.0 },
            patch_model: slab_patch,
        },
    );
    assert_all_identical(&res);
}
