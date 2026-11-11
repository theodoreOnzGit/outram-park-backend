//! Operator-level differential test against upstream OpenFOAM-v2506 (gh:#1362).
//!
//! # Method
//! `verification_and_validation/upstream_ops_gh1362/driver/opsDump.C` was built
//! against the real OpenFOAM-v2506 (ESI) libraries and run on two meshes made by
//! upstream tools: `blockMesh` (3 blocks, skewed and graded, one collapsed block
//! giving prisms; 316 cells) and `polyDualMesh 45` of it (470 cells, 210
//! polyhedra). It prints IEEE-754 bit patterns of the geometry, the inputs
//! (polynomial T, phi, gamma: no libm anywhere) and the operator results. This
//! test reads those dumps back (`include_str!`), builds the Rust mesh and
//! fields from the **upstream** geometry and inputs, so each operator is
//! compared in isolation, then separately compares the geometry the Rust port
//! computes itself from points and faces.
//!
//! # Criterion
//! Bit-identity is the goal. For every row the test counts the bit-identical
//! values, the worst relative difference and the worst ulp distance, prints
//! them (`--nocapture`) and PINS them, so a change in either direction fails.
//! Rows with a difference carry the proven cause in the table in
//! `verification_and_validation/upstream_ops_gh1362/README.md`.
//!
//! # Results
//! See the README table (measured 2026-11-11, release build).

use outram_foam_basic_lib::fields::boundary::bc::{BoundaryCondition, PatchField};
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::surface_field::SurfaceScalarField;
use outram_foam_basic_lib::fields::vol_field::VolScalarField;
use outram_foam_basic_lib::fv_operators::fvc;
use outram_foam_basic_lib::fv_operators::fvc::Limiter;
use outram_foam_basic_lib::fv_operators::fvm::{laplacian_with_delta, DeltaCoeff};
use outram_foam_basic_lib::io::poly_mesh::{MeshFace, PolyMesh};
use outram_foam_basic_lib::limiters::FluxLimiter;
use outram_foam_basic_lib::mesh::fv_mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;
use std::collections::HashMap;
use std::sync::Arc;

const HEX_BLOCK: &str =
    include_str!("../verification_and_validation/upstream_ops_gh1362/reference/ref_hexblock.txt");
const POLY_DUAL: &str =
    include_str!("../verification_and_validation/upstream_ops_gh1362/reference/ref_polydual.txt");

// ── parsing ────────────────────────────────────────────────────────────────

struct Ref {
    cols: HashMap<String, Vec<Vec<f64>>>,
    points: Vec<Vector3>,
    faces: Vec<Vec<usize>>,
    own: Vec<usize>,
    nei: Vec<usize>,
    patches: Vec<(String, usize, usize)>,
    n_cells: usize,
    n_int: usize,
}

fn hex(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).expect("hex"))
}

fn parse(text: &str) -> Ref {
    let mut r = Ref {
        cols: HashMap::new(),
        points: vec![],
        faces: vec![],
        own: vec![],
        nei: vec![],
        patches: vec![],
        n_cells: 0,
        n_int: 0,
    };
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.is_empty() {
            continue;
        }
        if t[0] == "#" {
            r.n_cells = t[2].parse().unwrap();
            r.n_int = t[6].parse().unwrap();
            continue;
        }
        match t[0] {
            "face" => r.faces.push(t[3..].iter().map(|x| x.parse().unwrap()).collect()),
            "own" => r.own.push(t[2].parse().unwrap()),
            "nei" => r.nei.push(t[2].parse().unwrap()),
            "patch" => r
                .patches
                .push((t[2].to_string(), t[3].parse().unwrap(), t[4].parse().unwrap())),
            "points" => r.points.push(Vector3::new(hex(t[2]), hex(t[3]), hex(t[4]))),
            key => r
                .cols
                .entry(key.to_string())
                .or_default()
                .push(t[2..].iter().map(|x| hex(x)).collect()),
        }
    }
    r
}

impl Ref {
    fn s(&self, k: &str) -> Vec<f64> {
        self.cols[k].iter().map(|v| v[0]).collect()
    }
    fn v(&self, k: &str) -> Vec<Vector3> {
        self.cols[k]
            .iter()
            .map(|v| Vector3::new(v[0], v[1], v[2]))
            .collect()
    }
    fn n_faces(&self) -> usize {
        self.faces.len()
    }
    fn patch_list(&self) -> Vec<BoundaryPatch> {
        self.patches
            .iter()
            .map(|(n, s, z)| {
                let kind = if n == "walls" { PatchKind::Wall } else { PatchKind::Patch };
                BoundaryPatch::new(n.clone(), *s, *z, kind)
            })
            .collect()
    }
    /// Rust mesh built from the UPSTREAM geometry, so operators are isolated.
    fn fv_mesh(&self) -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(self.n_cells)
                .n_internal_faces(self.n_int)
                .owner(self.own.clone())
                .neighbour(self.nei.clone())
                .patches(self.patch_list())
                .cell_volumes(self.s("V"))
                .cell_centres(self.v("C"))
                .face_area_vectors(self.v("Sf"))
                .face_centres(self.v("Cf"))
                .build()
                .unwrap(),
        )
    }
    /// Boundary-face values (patch order = global face order after the internal ones).
    fn bfield(&self, mesh: &FvMesh, key: &str) -> Vec<PatchField<f64>> {
        let all = self.s(key);
        mesh.patches
            .iter()
            .map(|p| {
                let vals = Field::new(all[p.start..p.start + p.size].to_vec());
                PatchField {
                    bc: BoundaryCondition::FixedField(vals.clone()),
                    values: vals,
                }
            })
            .collect()
    }
    fn t_field(&self, mesh: &Arc<FvMesh>) -> VolScalarField {
        // T is dumped per cell; its fixedValue boundary values are f_T(Cf), i.e. the
        // polynomial evaluated on the upstream face centres (exact, same formula).
        let cf = self.v("Cf");
        let boundary = mesh
            .patches
            .iter()
            .map(|p| {
                let vals = Field::new((p.start..p.start + p.size).map(|f| f_t(cf[f])).collect());
                PatchField {
                    bc: BoundaryCondition::FixedField(vals.clone()),
                    values: vals,
                }
            })
            .collect();
        VolScalarField::new("T", mesh.clone(), Field::new(self.s("T")), boundary)
    }
    fn surf(&self, mesh: &Arc<FvMesh>, key: &str) -> SurfaceScalarField {
        let all = self.s(key);
        SurfaceScalarField::new(
            key,
            mesh.clone(),
            Field::new(all[..mesh.n_internal_faces].to_vec()),
            self.bfield(mesh, key),
        )
    }
}

/// Same polynomial as `fT` in the C++ driver (operation order preserved).
fn f_t(p: Vector3) -> f64 {
    let (x, y, z) = (p.x, p.y, p.z);
    1.0 + 2.0 * x - 3.0 * y + 0.5 * z + x * y + 0.25 * y * z * z + x * x * z
}

// ── statistics ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
struct Stat {
    n: usize,
    identical: usize,
    max_ulp: u64,
    max_rel: f64,
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
    let mut s = Stat { n: rust.len(), identical: 0, max_ulp: 0, max_rel: 0.0 };
    for (a, b) in rust.iter().zip(up) {
        if a.to_bits() == b.to_bits() || (*a == 0.0 && *b == 0.0) {
            s.identical += 1;
            continue;
        }
        let u = (ord(*a) - ord(*b)).unsigned_abs();
        s.max_ulp = s.max_ulp.max(u);
        let rel = (a - b).abs() / b.abs().max(f64::MIN_POSITIVE);
        if rel > s.max_rel {
            s.max_rel = rel;
        }
    }
    println!(
        "{label:<34} n={:<6} identical={:<6} max_ulp={:<3} max_rel={:.3e}",
        s.n, s.identical, s.max_ulp, s.max_rel
    );
    s
}

fn comps(v: &[Vector3]) -> Vec<f64> {
    v.iter().flat_map(|a| [a.x, a.y, a.z]).collect()
}

fn all_faces(mesh: &FvMesh, internal: &[f64], boundary: impl Fn(usize) -> Vec<f64>) -> Vec<f64> {
    let mut v = internal.to_vec();
    for pi in 0..mesh.patches.len() {
        v.extend(boundary(pi));
    }
    v
}

// ── geometry computed by the port ──────────────────────────────────────────

fn poly_from_ref(r: &Ref) -> PolyMesh {
    let faces = r
        .faces
        .iter()
        .enumerate()
        .map(|(i, v)| MeshFace {
            verts: v.clone(),
            owner: r.own[i],
            neighbour: if i < r.n_int { Some(r.nei[i]) } else { None },
        })
        .collect();
    PolyMesh {
        points: r.points.clone(),
        faces,
        n_internal_faces: r.n_int,
        n_cells: r.n_cells,
        patches: r.patch_list(),
    }
}

fn geometry(text: &str) -> Vec<(&'static str, Stat)> {
    let r = parse(text);
    let m = poly_from_ref(&r).to_fv_mesh().unwrap();
    let tri: Vec<bool> = r.faces.iter().map(|f| f.len() == 3).collect();
    let cf_up = r.v("Cf");
    let sel = |v: &[Vector3], want_tri: bool| -> Vec<f64> {
        comps(
            &v.iter()
                .zip(&tri)
                .filter(|(_, t)| **t == want_tri)
                .map(|(a, _)| *a)
                .collect::<Vec<_>>(),
        )
    };
    vec![
        ("Sf (all faces)", cmp("Sf (all faces)", &comps(&m.face_area_vectors), &comps(&r.v("Sf")))),
        ("magSf", cmp("magSf", &m.face_areas, &r.s("magSf"))),
        ("Cf (triangles)", cmp("Cf (triangles)", &sel(&m.face_centres, true), &sel(&cf_up, true))),
        ("Cf (non-triangles)", cmp("Cf (non-triangles)", &sel(&m.face_centres, false), &sel(&cf_up, false))),
        ("V", cmp("V", &m.cell_volumes, &r.s("V"))),
        ("C", cmp("C", &comps(&m.cell_centres), &comps(&r.v("C")))),
    ]
}

// ── operators on upstream geometry ─────────────────────────────────────────

fn operators(text: &str) -> Vec<(&'static str, Stat)> {
    let r = parse(text);
    let mesh = r.fv_mesh();
    let nf = r.n_faces();
    let t = r.t_field(&mesh);
    let mut out: Vec<(&'static str, Stat)> = vec![];
    let up_unit = |name: &'static str, rust: &[f64], key: &str, out: &mut Vec<(&'static str, Stat)>| {
        out.push((name, cmp(name, rust, &r.s(key))));
    };

    // face weights
    let w: Vec<f64> = (0..mesh.n_internal_faces).map(|f| fvc::linear_weight(&mesh, f)).collect();
    let up_w = r.s("weights");
    out.push(("weights (internal)", cmp("weights (internal)", &w, &up_w[..r.n_int])));

    // delta coefficients
    let dc = |mode: DeltaCoeff| -> Vec<f64> {
        (0..nf)
            .map(|f| {
                1.0 / if f < r.n_int {
                    mode.interior_delta(&mesh, f)
                } else {
                    mode.boundary_delta(&mesh, f)
                }
            })
            .collect()
    };
    up_unit("deltaCoeffs", &dc(DeltaCoeff::Orthogonal), "deltaCoeffs", &mut out);
    up_unit("nonOrthDeltaCoeffs", &dc(DeltaCoeff::NonOrthogonal), "nonOrthDeltaCoeffs", &mut out);

    // linear interpolation
    let lin = fvc::interpolate(&t);
    let lin_all = all_faces(&mesh, lin.internal.as_slice(), |pi| lin.boundary[pi].values.as_slice().to_vec());
    up_unit("interpolate linear", &lin_all, "interp_linear_pos", &mut out);

    // Gauss linear gradient
    let g = fvc::grad(&t);
    out.push((
        "grad Gauss linear",
        cmp("grad Gauss linear", &comps(g.internal.as_slice()), &comps(&r.v("gradT"))),
    ));

    // snGrad: orthogonal = deltaCoeffs; uncorrected = nonOrthDeltaCoeffs
    let sn = fvc::sn_grad(&t);
    let sn_all = all_faces(&mesh, sn.internal.as_slice(), |pi| sn.boundary[pi].values.as_slice().to_vec());
    up_unit("snGrad (orthogonal)", &sn_all, "snGrad_orthogonal", &mut out);
    let tb = |f: usize| -> f64 {
        let pi = mesh.patches.iter().position(|p| f >= p.start && f < p.start + p.size).unwrap();
        t.boundary[pi].values[f - mesh.patches[pi].start]
    };
    let sn_unc: Vec<f64> = (0..nf)
        .map(|f| {
            if f < r.n_int {
                // upstream `uncorrectedSnGrad`: `nonOrthDeltaCoeffs * (N - P)`
                // (gh:#1489: multiply by the stored coefficient, not divide)
                DeltaCoeff::NonOrthogonal.interior_delta_coeff(&mesh, f)
                    * (t.internal[mesh.neighbour[f]] - t.internal[mesh.owner[f]])
            } else {
                // non-coupled boundary: `pvf.snGrad()` uses the patch's own
                // `deltaCoeffs`, not the scheme's `nonOrthDeltaCoeffs`
                DeltaCoeff::Orthogonal.boundary_delta_coeff(&mesh, f) * (tb(f) - t.internal[mesh.owner[f]])
            }
        })
        .collect();
    up_unit("snGrad (uncorrected, via delta)", &sn_unc, "snGrad_uncorrected", &mut out);

    // divergence
    let phi = r.surf(&mesh, "phi");
    up_unit("div(phi)", fvc::div_flux(&phi).internal.as_slice(), "divPhi", &mut out);
    up_unit("div(phi,T) Gauss linear", fvc::div(&phi, &t).internal.as_slice(), "divPhiT", &mut out);

    // limited schemes, flux sign +1 and -1 (internal faces)
    for (name, lim, key_pos, key_neg) in [
        ("recon upwind", Limiter::Upwind, "interp_upwind_pos", "interp_upwind_neg"),
        ("recon linear", Limiter::Linear, "interp_linear_pos", "interp_linear_neg"),
        ("recon vanLeer", Limiter::VanLeer, "interp_vanLeer_pos", "interp_vanLeer_neg"),
        ("recon Minmod", Limiter::Minmod, "interp_Minmod_pos", "interp_Minmod_neg"),
    ] {
        let (pos, neg) = fvc::reconstruct_pos_neg(&t, lim);
        let mut both = pos.internal.as_slice().to_vec();
        both.extend_from_slice(neg.internal.as_slice());
        let mut up = r.s(key_pos)[..r.n_int].to_vec();
        up.extend_from_slice(&r.s(key_neg)[..r.n_int]);
        out.push((name, cmp(name, &both, &up)));
    }

    // laplacian, Gauss linear uncorrected
    let gam = r.surf(&mesh, "gam");
    let m = laplacian_with_delta(&gam, &t, DeltaCoeff::NonOrthogonal);
    let neg = |v: Vec<f64>| -> Vec<f64> { v.into_iter().map(|x| -x).collect() };
    // Rust matrix is -div(gamma grad T): exact sign flip of upstream.
    let up_upper = neg(r.s("lap_upper"));
    out.push(("laplacian upper", cmp("laplacian upper", m.ldu.upper.as_slice(), &up_upper)));
    // diag: upstream diag() + internalCoeffs (boundary faces added in patch order)
    let mut diag = r.s("lap_diag");
    let ic = r.s("lap_intCoeff");
    let mut k = 0;
    for p in &mesh.patches {
        for fi in 0..p.size {
            diag[mesh.owner[p.start + fi]] += ic[k];
            k += 1;
        }
    }
    out.push(("laplacian diag (+bnd)", cmp("laplacian diag (+bnd)", m.ldu.diag.as_slice(), &neg(diag))));
    let mut src = vec![0.0; r.n_cells];
    let bc = r.s("lap_bndCoeff");
    let mut k = 0;
    for p in &mesh.patches {
        for fi in 0..p.size {
            src[mesh.owner[p.start + fi]] += bc[k];
            k += 1;
        }
    }
    out.push(("laplacian source (bnd)", cmp("laplacian source (bnd)", m.source.as_slice(), &neg(src))));
    out
}

/// Every limiter function against upstream's, with the upstream-computed `r`.
/// Stats are split at `r < 0`, where the port clamps vanAlbada / OSPRE and
/// upstream does not.
fn limiters(text: &str) -> Vec<(String, Stat)> {
    let r = parse(text);
    let mut out = vec![];
    for (name, key, lim) in [
        ("limiter vanLeer", "lim_vanLeer", FluxLimiter::VanLeer),
        ("limiter vanAlbada", "lim_vanAlbada", FluxLimiter::VanAlbada),
        ("limiter Minmod", "lim_Minmod", FluxLimiter::Minmod),
        ("limiter SuperBee", "lim_SuperBee", FluxLimiter::SuperBee),
        ("limiter MUSCL", "lim_MUSCL", FluxLimiter::Muscl),
        ("limiter UMIST", "lim_UMIST", FluxLimiter::Umist),
        ("limiter OSPRE", "lim_OSPRE", FluxLimiter::Ospre),
        ("limiter limitedLinear 1", "lim_limitedLinear1", FluxLimiter::LimitedLinear(1.0)),
        ("limiter limitedLinear 0.5", "lim_limitedLinear05", FluxLimiter::LimitedLinear(0.5)),
    ] {
        let rs = r.s(&format!("{key}_r"));
        let up = r.s(key);
        for (suffix, neg) in [(" [r>=0]", false), (" [r<0]", true)] {
            let (a, b): (Vec<f64>, Vec<f64>) = rs
                .iter()
                .zip(&up)
                .filter(|(x, _)| (**x < 0.0) == neg)
                .map(|(x, u)| (lim.psi(*x), *u))
                .unzip();
            let label = format!("{name}{suffix}");
            let st = cmp(&label, &a, &b);
            out.push((label, st));
        }
    }
    // cause proofs for the two limiters whose r<0 / operation order differ
    let rs = r.s("lim_OSPRE_r");
    let o: Vec<f64> = rs.iter().map(|&x| { let rrp1 = x * (x + 1.0); 1.5 * rrp1 / (rrp1 + 1.0) }).collect();
    out.push(("proof OSPRE 1.5*rrp1/(rrp1+1)".to_string(), cmp("proof OSPRE 1.5*rrp1/(rrp1+1)", &o, &r.s("lim_OSPRE"))));
    let rs = r.s("lim_vanAlbada_r");
    let o: Vec<f64> = rs.iter().map(|&x| x * (x + 1.0) / (x * x + 1.0)).collect();
    out.push(("proof vanAlbada r(r+1)/(r^2+1), no clamp".to_string(), cmp("proof vanAlbada r(r+1)/(r^2+1), no clamp", &o, &r.s("lim_vanAlbada"))));
    out
}

// ── cause proofs: the upstream arithmetic, replayed from upstream inputs ───

fn sc(v: Vector3, k: f64) -> Vector3 {
    Vector3::new(v.x * k, v.y * k, v.z * k)
}
fn dv(v: Vector3, k: f64) -> Vector3 {
    Vector3::new(v.x / k, v.y / k, v.z / k)
}
fn dot(a: Vector3, b: Vector3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}
fn mag(a: Vector3) -> f64 {
    dot(a, a).sqrt()
}

/// `primitiveMeshTools::makeFaceCentresAndAreas` + `makeCellCentresAndVols`
/// (OpenFOAM-v2506), replayed operation for operation.
fn upstream_geometry(r: &Ref) -> (Vec<Vector3>, Vec<Vector3>, Vec<Vector3>, Vec<f64>) {
    let p = &r.points;
    let mut fc = vec![];
    let mut fa = vec![];
    for f in &r.faces {
        let n = f.len();
        if n == 3 {
            let (a, b, c) = (p[f[0]], p[f[1]], p[f[2]]);
            fc.push(sc(a + b + c, 1.0 / 3.0));
            fa.push(sc((b - a).cross(c - a), 0.5));
        } else {
            let mut ce = p[f[0]];
            for &i in &f[1..] {
                ce = ce + p[i];
            }
            let ce = dv(ce, n as f64);
            let (mut sn, mut sa, mut sac) = (Vector3::ZERO, 0.0, Vector3::ZERO);
            for i in 0..n {
                let this = p[f[i]];
                let next = p[f[(i + 1) % n]];
                let c = this + next + ce;
                let nn = (next - this).cross(ce - this);
                let a = mag(nn);
                sn = sn + nn;
                sa += a;
                sac = sac + sc(c, a);
            }
            fc.push(dv(sc(sac, 1.0 / 3.0), sa));
            fa.push(sc(sn, 0.5));
        }
    }
    let nc = r.n_cells;
    let mut ce = vec![Vector3::ZERO; nc];
    let mut cnt = vec![0.0; nc];
    for f in 0..r.n_faces() {
        ce[r.own[f]] = ce[r.own[f]] + fc[f];
        cnt[r.own[f]] += 1.0;
    }
    for f in 0..r.n_int {
        ce[r.nei[f]] = ce[r.nei[f]] + fc[f];
        cnt[r.nei[f]] += 1.0;
    }
    for c in 0..nc {
        ce[c] = dv(ce[c], cnt[c]);
    }
    let mut cc = vec![Vector3::ZERO; nc];
    let mut vol = vec![0.0; nc];
    for f in 0..r.n_faces() {
        let o = r.own[f];
        let pyr = dot(fa[f], fc[f] - ce[o]);
        let pc = sc(fc[f], 3.0 / 4.0) + sc(ce[o], 1.0 / 4.0);
        cc[o] = cc[o] + sc(pc, pyr);
        vol[o] += pyr;
    }
    for f in 0..r.n_int {
        let n = r.nei[f];
        let pyr = dot(fa[f], ce[n] - fc[f]);
        let pc = sc(fc[f], 3.0 / 4.0) + sc(ce[n], 1.0 / 4.0);
        cc[n] = cc[n] + sc(pc, pyr);
        vol[n] += pyr;
    }
    for c in 0..nc {
        cc[c] = dv(cc[c], vol[c]);
        vol[c] *= 1.0 / 3.0;
    }
    (fc, fa, cc, vol)
}

fn proofs(text: &str) -> Vec<(&'static str, Stat)> {
    let r = parse(text);
    let mut out: Vec<(&'static str, Stat)> = vec![];
    let nf = r.n_faces();

    let (fc, fa, cc, vol) = upstream_geometry(&r);
    out.push(("proof Cf (upstream order)", cmp("proof Cf (upstream order)", &comps(&fc), &comps(&r.v("Cf")))));
    out.push(("proof Sf (upstream order)", cmp("proof Sf (upstream order)", &comps(&fa), &comps(&r.v("Sf")))));
    out.push(("proof C (upstream order)", cmp("proof C (upstream order)", &comps(&cc), &comps(&r.v("C")))));
    out.push(("proof V (upstream order)", cmp("proof V (upstream order)", &vol, &r.s("V"))));

    let c = r.v("C");
    let cf = r.v("Cf");
    let sf = r.v("Sf");
    let msf = r.s("magSf");
    let mut dc = vec![];
    let mut nod = vec![];
    for f in 0..nf {
        let o = r.own[f];
        if f < r.n_int {
            let d = c[r.nei[f]] - c[o];
            dc.push(1.0 / mag(d));
            let unit = dv(sf[f], msf[f]);
            nod.push(1.0 / (dot(unit, d)).max(0.05 * mag(d)));
        } else {
            let n = dv(sf[f], msf[f]);
            let d = sc(n, dot(n, cf[f] - c[o]));
            dc.push(1.0 / mag(d));
            nod.push(1.0 / dot(n, d).max(0.05 * mag(d)));
        }
    }
    out.push(("proof deltaCoeffs", cmp("proof deltaCoeffs (patch-normal delta)", &dc, &r.s("deltaCoeffs"))));
    out.push(("proof nonOrthDeltaCoeffs", cmp("proof nonOrthDeltaCoeffs (Sf/magSf)", &nod, &r.s("nonOrthDeltaCoeffs"))));

    let w = r.s("weights");
    let t = r.s("T");
    let lin: Vec<f64> = (0..r.n_int)
        .map(|f| w[f] * (t[r.own[f]] - t[r.nei[f]]) + t[r.nei[f]])
        .collect();
    out.push(("proof linear", cmp("proof linear w*(P-N)+N", &lin, &r.s("interp_linear_pos")[..r.n_int])));
    let up_pos: Vec<f64> = (0..r.n_int).map(|f| 1.0 * (t[r.own[f]] - t[r.nei[f]]) + t[r.nei[f]]).collect();
    out.push(("proof upwind", cmp("proof upwind(+) 1*(P-N)+N", &up_pos, &r.s("interp_upwind_pos")[..r.n_int])));

    let tf = r.s("interp_linear_pos");
    let v = r.s("V");
    let mut g = vec![Vector3::ZERO; r.n_cells];
    for f in 0..nf {
        let contrib = sc(sf[f], tf[f]);
        g[r.own[f]] = g[r.own[f]] + contrib;
        if f < r.n_int {
            g[r.nei[f]] = g[r.nei[f]] - contrib;
        }
    }
    let g_div: Vec<Vector3> = (0..r.n_cells).map(|i| dv(g[i], v[i])).collect();
    let g_mul: Vec<Vector3> = (0..r.n_cells).map(|i| sc(g[i], 1.0 / v[i])).collect();
    out.push(("proof grad g/V", cmp("proof grad Gauss, g/V", &comps(&g_div), &comps(&r.v("gradT")))));
    out.push(("proof grad g*(1/V)", cmp("proof grad Gauss, g*(1/V) (port form)", &comps(&g_mul), &comps(&r.v("gradT")))));

    let dcu = r.s("deltaCoeffs");
    let cfv: Vec<f64> = (0..nf).map(|f| f_t(cf[f])).collect();
    let sn: Vec<f64> = (0..nf)
        .map(|f| {
            if f < r.n_int {
                dcu[f] * (t[r.nei[f]] - t[r.own[f]])
            } else {
                dcu[f] * (cfv[f] - t[r.own[f]])
            }
        })
        .collect();
    out.push(("proof snGrad", cmp("proof snGrad deltaCoeffs*(N-P)", &sn, &r.s("snGrad_orthogonal"))));

    let gam = r.s("gam");
    let nod_u = r.s("nonOrthDeltaCoeffs");
    let up: Vec<f64> = (0..r.n_int).map(|f| nod_u[f] * (gam[f] * msf[f])).collect();
    out.push(("proof laplacian upper", cmp("proof laplacian upper dC*(gamma*magSf)", &up, &r.s("lap_upper"))));
    out
}

fn rows(text: &str) -> Vec<(String, Stat)> {
    let mut v: Vec<(String, Stat)> = vec![];
    v.extend(geometry(text).into_iter().map(|(n, s)| (n.to_string(), s)));
    v.extend(operators(text).into_iter().map(|(n, s)| (n.to_string(), s)));
    v.extend(limiters(text));
    v.extend(proofs(text).into_iter().map(|(n, s)| (n.to_string(), s)));
    v
}

/// Measured 2026-11-11 (release): (mesh, row, n, bit-identical, worst ulp).
/// **gh:#1488 (same day):** `deltaCoeffs` and `snGrad (orthogonal)` re-pinned
/// after the boundary-`deltaCoeffs` fix (patch-normal `fvPatch::delta()`):
/// `deltaCoeffs` hex ~~789/1083, 6.8e15 ulp (rel 0.66)~~ **1083/1083, 0**,
/// poly ~~1236/1788, 1.4e16 ulp (rel 0.88)~~ **1788/1788, 0**; `snGrad
/// (orthogonal)` hex ~~533/1083~~ **737/1083, 1 ulp**, poly ~~890/1788~~
/// **1285/1788, 1 ulp**. The remaining 1-ulp snGrad differences are `/d`
/// against upstream's `deltaCoeffs*` (proof row `proof snGrad`), gh:#1489.
/// Pinned in BOTH directions: a change that improves a row fails this test too,
/// so the README table is updated with it. Rows named `proof ...` must be
/// 100 % bit-identical: they replay the upstream arithmetic and explain the
/// difference in the row above them.
const PINS: &[(&str, &str, usize, usize, u64)] = &[
    ("hex", "Sf (all faces)", 3249, 3249, 0),
    ("hex", "magSf", 1083, 1083, 0),
    ("hex", "Cf (triangles)", 90, 90, 0), // ~~58, 1 ulp~~ gh:#1489
    ("hex", "Cf (non-triangles)", 3159, 3159, 0), // ~~1737, 3 ulp~~ gh:#1489
    ("hex", "V", 316, 316, 0), // ~~42, 47 ulp~~ gh:#1489
    ("hex", "C", 948, 948, 0), // ~~342, 4 ulp~~ gh:#1489
    ("hex", "weights (internal)", 789, 789, 0),
    ("hex", "deltaCoeffs", 1083, 1083, 0), // ~~789, 6819937884107484~~ gh:#1488
    ("hex", "nonOrthDeltaCoeffs", 1083, 1083, 0), // ~~772, 6 ulp~~ gh:#1489
    ("hex", "interpolate linear", 1083, 1083, 0), // ~~888, 12 ulp~~ gh:#1489
    ("hex", "grad Gauss linear", 948, 948, 0), // ~~506, 62 ulp~~ gh:#1489
    ("hex", "snGrad (orthogonal)", 1083, 1083, 0), // ~~737, 1 ulp~~ gh:#1489 // ~~533, 7394036669051906~~ gh:#1488
    ("hex", "snGrad (uncorrected, via delta)", 1083, 1083, 0), // ~~627, 3 ulp~~ gh:#1489
    ("hex", "div(phi)", 316, 316, 0),
    ("hex", "div(phi,T) Gauss linear", 316, 316, 0), // ~~196, 34 ulp~~ gh:#1489
    ("hex", "recon upwind", 1578, 1578, 0), // ~~1546, 32 ulp~~ gh:#1489
    ("hex", "recon linear", 1578, 1578, 0), // ~~1199, 16 ulp~~ gh:#1489
    ("hex", "recon vanLeer", 1578, 1578, 0), // ~~813, 192 ulp~~ gh:#1489
    ("hex", "recon Minmod", 1578, 1578, 0), // ~~1010, 32 ulp~~ gh:#1489
    ("hex", "laplacian upper", 789, 789, 0), // ~~466, 3 ulp~~ gh:#1489
    ("hex", "laplacian diag (+bnd)", 316, 316, 0), // ~~224, 2 ulp~~ gh:#1489
    ("hex", "laplacian source (bnd)", 316, 316, 0), // ~~197, 3 ulp~~ gh:#1489
    ("hex", "limiter vanLeer [r>=0]", 232, 232, 0),
    ("hex", "limiter vanLeer [r<0]", 82, 82, 0),
    ("hex", "limiter vanAlbada [r>=0]", 232, 232, 0),
    ("hex", "limiter vanAlbada [r<0]", 82, 1, 4603579539098121011),
    ("hex", "limiter Minmod [r>=0]", 232, 232, 0),
    ("hex", "limiter Minmod [r<0]", 82, 82, 0),
    ("hex", "limiter SuperBee [r>=0]", 232, 232, 0),
    ("hex", "limiter SuperBee [r<0]", 82, 82, 0),
    ("hex", "limiter MUSCL [r>=0]", 232, 232, 0),
    ("hex", "limiter MUSCL [r<0]", 82, 82, 0),
    ("hex", "limiter UMIST [r>=0]", 232, 232, 0),
    ("hex", "limiter UMIST [r<0]", 82, 82, 0),
    ("hex", "limiter OSPRE [r>=0]", 232, 232, 0), // ~~151, 2 ulp~~ gh:#1489
    ("hex", "limiter OSPRE [r<0]", 82, 1, 4608469161550694693),
    ("hex", "limiter limitedLinear 1 [r>=0]", 232, 232, 0),
    ("hex", "limiter limitedLinear 1 [r<0]", 82, 82, 0),
    ("hex", "limiter limitedLinear 0.5 [r>=0]", 232, 232, 0),
    ("hex", "limiter limitedLinear 0.5 [r<0]", 82, 82, 0),
    ("hex", "proof OSPRE 1.5*rrp1/(rrp1+1)", 314, 314, 0),
    ("hex", "proof vanAlbada r(r+1)/(r^2+1), no clamp", 314, 314, 0),
    ("hex", "proof Cf (upstream order)", 3249, 3249, 0),
    ("hex", "proof Sf (upstream order)", 3249, 3249, 0),
    ("hex", "proof C (upstream order)", 948, 948, 0),
    ("hex", "proof V (upstream order)", 316, 316, 0),
    ("hex", "proof deltaCoeffs", 1083, 1083, 0),
    ("hex", "proof nonOrthDeltaCoeffs", 1083, 1083, 0),
    ("hex", "proof linear", 789, 789, 0),
    ("hex", "proof upwind", 789, 789, 0),
    ("hex", "proof grad g/V", 948, 948, 0),
    ("hex", "proof grad g*(1/V)", 948, 728, 1),
    ("hex", "proof snGrad", 1083, 1083, 0),
    ("hex", "proof laplacian upper", 789, 789, 0),
    ("poly", "Sf (all faces)", 5364, 5364, 0),
    ("poly", "magSf", 1788, 1788, 0),
    ("poly", "Cf (triangles)", 0, 0, 0),
    ("poly", "Cf (non-triangles)", 5364, 5364, 0), // ~~2933, 23 ulp~~ gh:#1489
    ("poly", "V", 470, 470, 0), // ~~70, 60 ulp~~ gh:#1489
    ("poly", "C", 1410, 1410, 0), // ~~496, 6 ulp~~ gh:#1489
    ("poly", "weights (internal)", 1236, 1236, 0),
    ("poly", "deltaCoeffs", 1788, 1788, 0), // ~~1236, 14045069578427769~~ gh:#1488
    ("poly", "nonOrthDeltaCoeffs", 1788, 1788, 0), // ~~1193, 4 ulp~~ gh:#1489
    ("poly", "interpolate linear", 1788, 1788, 0), // ~~1469, 16 ulp~~ gh:#1489
    ("poly", "grad Gauss linear", 1410, 1410, 0), // ~~696, 103 ulp~~ gh:#1489
    ("poly", "snGrad (orthogonal)", 1788, 1788, 0), // ~~1285, 1 ulp~~ gh:#1489 // ~~890, 13935367941715158~~ gh:#1488
    ("poly", "snGrad (uncorrected, via delta)", 1788, 1788, 0), // ~~947, 3 ulp~~ gh:#1489
    ("poly", "div(phi)", 470, 470, 0),
    ("poly", "div(phi,T) Gauss linear", 470, 470, 0), // ~~297, 49 ulp~~ gh:#1489
    ("poly", "recon upwind", 2472, 2472, 0), // ~~2407, 8 ulp~~ gh:#1489
    ("poly", "recon linear", 2472, 2472, 0), // ~~1858, 16 ulp~~ gh:#1489
    ("poly", "recon vanLeer", 2472, 2472, 0), // ~~1322, 1024 ulp~~ gh:#1489
    ("poly", "recon Minmod", 2472, 2472, 0), // ~~1537, 80 ulp~~ gh:#1489
    ("poly", "laplacian upper", 1236, 1236, 0), // ~~740, 2 ulp~~ gh:#1489
    ("poly", "laplacian diag (+bnd)", 470, 470, 0), // ~~312, 2 ulp~~ gh:#1489
    ("poly", "laplacian source (bnd)", 470, 470, 0), // ~~311, 8 ulp~~ gh:#1489
    ("poly", "limiter vanLeer [r>=0]", 232, 232, 0),
    ("poly", "limiter vanLeer [r<0]", 82, 82, 0),
    ("poly", "limiter vanAlbada [r>=0]", 232, 232, 0),
    ("poly", "limiter vanAlbada [r<0]", 82, 1, 4603579539098121011),
    ("poly", "limiter Minmod [r>=0]", 232, 232, 0),
    ("poly", "limiter Minmod [r<0]", 82, 82, 0),
    ("poly", "limiter SuperBee [r>=0]", 232, 232, 0),
    ("poly", "limiter SuperBee [r<0]", 82, 82, 0),
    ("poly", "limiter MUSCL [r>=0]", 232, 232, 0),
    ("poly", "limiter MUSCL [r<0]", 82, 82, 0),
    ("poly", "limiter UMIST [r>=0]", 232, 232, 0),
    ("poly", "limiter UMIST [r<0]", 82, 82, 0),
    ("poly", "limiter OSPRE [r>=0]", 232, 232, 0), // ~~151, 2 ulp~~ gh:#1489
    ("poly", "limiter OSPRE [r<0]", 82, 1, 4608469161550694693),
    ("poly", "limiter limitedLinear 1 [r>=0]", 232, 232, 0),
    ("poly", "limiter limitedLinear 1 [r<0]", 82, 82, 0),
    ("poly", "limiter limitedLinear 0.5 [r>=0]", 232, 232, 0),
    ("poly", "limiter limitedLinear 0.5 [r<0]", 82, 82, 0),
    ("poly", "proof OSPRE 1.5*rrp1/(rrp1+1)", 314, 314, 0),
    ("poly", "proof vanAlbada r(r+1)/(r^2+1), no clamp", 314, 314, 0),
    ("poly", "proof Cf (upstream order)", 5364, 5364, 0),
    ("poly", "proof Sf (upstream order)", 5364, 5364, 0),
    ("poly", "proof C (upstream order)", 1410, 1410, 0),
    ("poly", "proof V (upstream order)", 470, 470, 0),
    ("poly", "proof deltaCoeffs", 1788, 1788, 0),
    ("poly", "proof nonOrthDeltaCoeffs", 1788, 1788, 0),
    ("poly", "proof linear", 1236, 1236, 0),
    ("poly", "proof upwind", 1236, 1236, 0),
    ("poly", "proof grad g/V", 1410, 1410, 0),
    ("poly", "proof grad g*(1/V)", 1410, 1060, 1),
    ("poly", "proof snGrad", 1788, 1788, 0),
    ("poly", "proof laplacian upper", 1236, 1236, 0),
];

#[test]
fn operators_match_upstream_as_pinned() {
    let mut seen = 0;
    let mut mismatches: Vec<String> = vec![];
    for (label, text) in [("hex", HEX_BLOCK), ("poly", POLY_DUAL)] {
        for (name, st) in rows(text) {
            let pin = PINS
                .iter()
                .find(|p| p.0 == label && p.1 == name)
                .unwrap_or_else(|| panic!("no pin for {label}|{name}"));
            if (st.n, st.identical, st.max_ulp) != (pin.2, pin.3, pin.4) {
                // collect, so one run lists every moved row (PINS format)
                mismatches.push(format!(
                    "    (\"{label}\", \"{name}\", {}, {}, {}), // pinned ({}, {}, {})",
                    st.n, st.identical, st.max_ulp, pin.2, pin.3, pin.4
                ));
            }
            if name.starts_with("proof ") && !name.contains("port form") && !name.contains("g*(1/V)") {
                assert_eq!(st.identical, st.n, "{label}|{name}: proof row must be bit-identical");
            }
            seen += 1;
        }
    }
    assert!(mismatches.is_empty(), "moved rows:\n{}", mismatches.join("\n"));
    assert_eq!(seen, PINS.len());
}
