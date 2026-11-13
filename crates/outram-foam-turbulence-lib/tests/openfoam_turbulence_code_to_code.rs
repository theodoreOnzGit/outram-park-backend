//! Turbulence-model differential test against upstream OpenFOAM-v2506 (gh:#1362, row 5).
//!
//! # Method
//! `verification_and_validation/upstream_turb_gh1362/driver/turbDump.C` runs the REAL
//! ESI model (`kEpsilon`, `kOmega`, `kOmegaSST`, `SpalartAllmaras`, `Smagorinsky`) on a
//! frozen polynomial field on a 12x10x3 graded duct, prints the real `nut`, the real
//! `fvc::grad(U)` / `div(phi)` / scalar gradients, the model's own registered `G`,
//! `GbyNu`, every assembled scalar matrix, and the source terms TRANSCRIBED from the
//! upstream `.C` (audited bit for bit against the real matrices: `audit` lines, re-checked
//! here). This test builds the port's mesh and fields from the UPSTREAM geometry and inputs
//! and feeds the port's `source_terms` the UPSTREAM gradients, so the model formulas are
//! isolated from the Gauss-gradient ulp orderings of gh:#1489.
//!
//! Per model: `SourceForm::EsiV2506` (the option) must be bit-identical to upstream
//! (except where a libm function is involved: Smagorinsky's `cbrt`, pinned); the default
//! `SourceForm::Port` rows are PINNED (bit-identical count per row, both directions) so a
//! change in either direction fails. "(esi y)" feeds upstream's `meshWave` wall distance,
//! "(own y)" the port's nearest-wall-face-centre distance.
//!
//! # Results
//! See `verification_and_validation/upstream_turb_gh1362/README.md` (release, 2026-11-13).

use outram_foam_basic_lib::fields::boundary::bc::{BoundaryCondition, PatchField};
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::surface_field::SurfaceScalarField;
use outram_foam_basic_lib::fields::vol_field::{VolScalarField, VolVectorField};
use outram_foam_basic_lib::mesh::fv_mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
use outram_foam_basic_lib::primitives::{Tensor, Vector3};
use outram_foam_turbulence_lib::k_epsilon::KEpsilon;
use outram_foam_turbulence_lib::k_omega::KOmega;
use outram_foam_turbulence_lib::k_omega_sst::KOmegaSST;
use outram_foam_turbulence_lib::les::wale::Wale;
use outram_foam_turbulence_lib::realizable_ke::RealizableKE;
use outram_foam_turbulence_lib::rng_k_epsilon::RngKEpsilon;
use outram_foam_turbulence_lib::les::smagorinsky::Smagorinsky;
use outram_foam_turbulence_lib::source_terms::{FrozenState, SourceForm, SourceTerms};
use outram_foam_turbulence_lib::spalart_allmaras::SpalartAllmaras;
use std::collections::HashMap;
use std::sync::Arc;

const REF_K_EPSILON: &[u8] =
    include_bytes!("../verification_and_validation/upstream_turb_gh1362/reference/ref_kEpsilon.txt.gz");
const REF_K_OMEGA: &[u8] =
    include_bytes!("../verification_and_validation/upstream_turb_gh1362/reference/ref_kOmega.txt.gz");
const REF_SST: &[u8] =
    include_bytes!("../verification_and_validation/upstream_turb_gh1362/reference/ref_kOmegaSST.txt.gz");
const REF_SA: &[u8] =
    include_bytes!("../verification_and_validation/upstream_turb_gh1362/reference/ref_SpalartAllmaras.txt.gz");
const REF_RNG: &[u8] =
    include_bytes!("../verification_and_validation/upstream_turb_gh1362/reference/ref_RNGkEpsilon.txt.gz");
const REF_REALIZABLE: &[u8] =
    include_bytes!("../verification_and_validation/upstream_turb_gh1362/reference/ref_realizableKE.txt.gz");
const REF_WALE: &[u8] =
    include_bytes!("../verification_and_validation/upstream_turb_gh1362/reference/ref_WALE.txt.gz");
const REF_SMAG: &[u8] =
    include_bytes!("../verification_and_validation/upstream_turb_gh1362/reference/ref_Smagorinsky.txt.gz");

fn gunzip(data: &[u8]) -> String {
    // `gzip -n`: 10-byte header, deflate stream, 8-byte trailer.
    let body = &data[10..data.len() - 8];
    let raw = miniz_oxide::inflate::decompress_to_vec(body).expect("inflate");
    String::from_utf8(raw).expect("utf8")
}

fn hex(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).expect("hex"))
}

struct Ref {
    cols: HashMap<String, Vec<Vec<f64>>>,
    own: Vec<usize>,
    nei: Vec<usize>,
    patches: Vec<(String, usize, usize)>,
    audits: Vec<String>,
    n_cells: usize,
    n_int: usize,
}

fn parse(gz: &[u8]) -> Ref {
    let text = gunzip(gz);
    let mut r = Ref {
        cols: HashMap::new(),
        own: vec![],
        nei: vec![],
        patches: vec![],
        audits: vec![],
        n_cells: 0,
        n_int: 0,
    };
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.is_empty() {
            continue;
        }
        match t[0] {
            "#" => {
                r.n_cells = t[2].parse().unwrap();
                r.n_int = t[6].parse().unwrap();
            }
            "audit" => r.audits.push(line.to_string()),
            "face" | "points" => {}
            "own" => r.own.push(t[2].parse().unwrap()),
            "nei" => r.nei.push(t[2].parse().unwrap()),
            "patch" => r
                .patches
                .push((t[2].to_string(), t[3].parse().unwrap(), t[4].parse().unwrap())),
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
    fn has(&self, k: &str) -> bool {
        self.cols.contains_key(k)
    }
    fn s(&self, k: &str) -> Vec<f64> {
        self.cols[k].iter().map(|v| v[0]).collect()
    }
    fn v(&self, k: &str) -> Vec<Vector3> {
        self.cols[k]
            .iter()
            .map(|v| Vector3::new(v[0], v[1], v[2]))
            .collect()
    }
    fn t(&self, k: &str) -> Vec<Tensor> {
        self.cols[k]
            .iter()
            .map(|v| Tensor::new(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8]))
            .collect()
    }
    fn mesh(&self) -> Arc<FvMesh> {
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
    /// Scalar zero-gradient field: boundary value = owner cell value.
    fn zg(&self, mesh: &Arc<FvMesh>, name: &str, key: &str) -> VolScalarField {
        let cells = self.s(key);
        let boundary = mesh
            .patches
            .iter()
            .map(|p| {
                let vals = Field::new(
                    (p.start..p.start + p.size).map(|f| cells[mesh.owner[f]]).collect(),
                );
                PatchField {
                    bc: BoundaryCondition::FixedField(vals.clone()),
                    values: vals,
                }
            })
            .collect();
        VolScalarField::new(name, mesh.clone(), Field::new(cells), boundary)
    }
    fn velocity(&self, mesh: &Arc<FvMesh>) -> VolVectorField {
        let ub = self.v("Ub");
        let boundary = mesh
            .patches
            .iter()
            .map(|p| {
                let b0 = p.start - mesh.n_internal_faces;
                let vals = Field::new(ub[b0..b0 + p.size].to_vec());
                PatchField {
                    bc: BoundaryCondition::FixedField(vals.clone()),
                    values: vals,
                }
            })
            .collect();
        VolVectorField::new("U", mesh.clone(), Field::new(self.v("U")), boundary)
    }
    fn flux(&self, mesh: &Arc<FvMesh>) -> SurfaceScalarField {
        let all = self.s("phi");
        let boundary = mesh
            .patches
            .iter()
            .map(|p| {
                let vals = Field::new(all[p.start..p.start + p.size].to_vec());
                PatchField {
                    bc: BoundaryCondition::FixedField(vals.clone()),
                    values: vals,
                }
            })
            .collect();
        SurfaceScalarField::new(
            "phi",
            mesh.clone(),
            Field::new(all[..mesh.n_internal_faces].to_vec()),
            boundary,
        )
    }
    /// The frozen state with the UPSTREAM gradients.
    fn state(&self, with_y: bool, with_delta: bool) -> FrozenState {
        FrozenState {
            grad_u: self.t("gradU"),
            div_u: self.s("divU"),
            grad_k: self.v("gradk"),
            grad_omega: self.v("gradomega"),
            grad_nu_tilda: self.v("gradnuTilda"),
            wall_distance: if with_y { Some(self.s("y")) } else { None },
            cell_length: if with_delta { Some(self.s("T_delta")) } else { None },
        }
    }
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
        s.max_ulp = s.max_ulp.max((ord(*a) - ord(*b)).unsigned_abs());
        let rel = (a - b).abs() / b.abs().max(f64::MIN_POSITIVE);
        if rel > s.max_rel {
            s.max_rel = rel;
        }
    }
    println!(
        "{label:<56} n={:<5} identical={:<5} max_ulp={:<12} max_rel={:.3e}",
        s.n, s.identical, s.max_ulp, s.max_rel
    );
    s
}

/// (reference key, term name) pairs per model.
type Pairs = &'static [(&'static str, &'static str)];

type Rows = Vec<(String, Stat)>;

fn compare(label: &str, r: &Ref, st: &SourceTerms, pairs: Pairs) -> Rows {
    pairs
        .iter()
        .map(|(rk, term)| {
            let rust = st.get(term).unwrap_or_else(|| panic!("term {term}"));
            let l = format!("{label} {term}");
            (l.clone(), cmp(&l, rust, &r.s(rk)))
        })
        .collect()
}

/// Port-versus-ESI comparison of a term the dump does not carry (both are Rust).
fn compare_forms(label: &str, a: &SourceTerms, b: &SourceTerms, term: &str) -> (String, Stat) {
    let l = format!("{label} {term}");
    (l.clone(), cmp(&l, a.get(term).unwrap(), b.get(term).unwrap()))
}

fn check_audits(r: &Ref, n_eq_parts: usize) {
    assert_eq!(r.audits.len(), n_eq_parts, "audit line count");
    for a in &r.audits {
        // "audit eq part n=360 identical=360 worst_rel=0.000e+00"
        let n = a.split("n=").nth(1).unwrap().split_whitespace().next().unwrap();
        let id = a.split("identical=").nth(1).unwrap().split_whitespace().next().unwrap();
        assert_eq!(n, id, "transcription of the upstream equation is not bit-identical: {a}");
    }
}

/// Real registered field vs transcribed field (a second audit of the transcription).
fn check_registered(r: &Ref, real_key: &str, transcribed_key: &str) {
    let real = r.s(real_key);
    let tr = r.s(transcribed_key);
    assert_eq!(real.len(), tr.len());
    for (a, b) in real.iter().zip(&tr) {
        assert_eq!(a.to_bits(), b.to_bits(), "{real_key} != {transcribed_key}");
    }
}

/// Pin the bit-identical count of every row (both directions). With an empty pin
/// list the rows are printed as ready-to-paste pins and the test fails.
fn pin(rows: &Rows, pins: &[(&str, usize)]) {
    if pins.is_empty() {
        for (l, s) in rows {
            println!("PIN (\"{l}\", {}),", s.identical);
        }
        panic!("no pins recorded");
    }
    assert_eq!(rows.len(), pins.len(), "row count");
    for ((l, s), (pl, pc)) in rows.iter().zip(pins) {
        assert_eq!(l, pl, "row order");
        assert_eq!(s.identical, *pc, "{l}: bit-identical count moved");
    }
}

fn all_identical(rows: &Rows) {
    for (l, s) in rows {
        assert_eq!(s.identical, s.n, "{l}");
    }
}

#[test]
fn k_epsilon_vs_openfoam() {
    let r = parse(REF_K_EPSILON);
    check_audits(&r, 8);
    check_registered(&r, "reg_epsilon_kEpsilon_G", "T_G");
    check_registered(&r, "reg_epsilon_kEpsilon_GbyNu", "T_GbyNu");
    let mesh = r.mesh();
    let mut m = KEpsilon::new(mesh.clone());
    m.k = r.zg(&mesh, "k", "k");
    m.epsilon = r.zg(&mesh, "epsilon", "epsilon");
    m.nu = r.zg(&mesh, "nu", "nu");
    m.u = r.velocity(&mesh);
    m.phi = r.flux(&mesh);
    let pairs: Pairs = &[
        ("nut_real", "nut"),
        ("T_GbyNu", "GbyNu"),
        ("T_G", "G"),
        ("T_eps_E", "eps_E"),
        ("T_eps_SuSp", "eps_SuSp"),
        ("T_eps_Sp", "eps_Sp"),
        ("T_k_E", "k_E"),
        ("T_k_SuSp", "k_SuSp"),
        ("T_k_Sp", "k_Sp"),
    ];
    let st = r.state(false, false);
    let esi = compare("kEpsilon esi ", &r, &m.source_terms(SourceForm::EsiV2506, &st), pairs);
    all_identical(&esi);
    let port = compare("kEpsilon port", &r, &m.source_terms(SourceForm::Port, &st), pairs);
    pin(&port, PINS_K_EPSILON_PORT);
}

#[test]
fn k_omega_vs_openfoam() {
    let r = parse(REF_K_OMEGA);
    check_audits(&r, 8);
    check_registered(&r, "reg_omega_kOmega_G", "T_G");
    let mesh = r.mesh();
    let mut m = KOmega::new(mesh.clone());
    m.k = r.zg(&mesh, "k", "k");
    m.omega = r.zg(&mesh, "omega", "omega");
    m.nu = r.zg(&mesh, "nu", "nu");
    m.u = r.velocity(&mesh);
    m.phi = r.flux(&mesh);
    let pairs: Pairs = &[
        ("nut_real", "nut"),
        ("T_GbyNu", "GbyNu"),
        ("T_G", "G"),
        ("T_omega_E", "omega_E"),
        ("T_omega_SuSp", "omega_SuSp"),
        ("T_omega_Sp", "omega_Sp"),
        ("T_k_E", "k_E"),
        ("T_k_SuSp", "k_SuSp"),
        ("T_k_Sp", "k_Sp"),
    ];
    let st = r.state(false, false);
    let esi = compare("kOmega esi ", &r, &m.source_terms(SourceForm::EsiV2506, &st), pairs);
    all_identical(&esi);
    let port = compare("kOmega port", &r, &m.source_terms(SourceForm::Port, &st), pairs);
    pin(&port, PINS_K_OMEGA_PORT);
}

#[test]
fn k_omega_sst_vs_openfoam() {
    let r = parse(REF_SST);
    check_audits(&r, 8);
    check_registered(&r, "reg_omega_kOmegaSST_G", "T_G");
    check_registered(&r, "reg_omega_kOmegaSST_gamma", "T_gamma");
    check_registered(&r, "reg_omega_kOmegaSST_beta", "T_beta");
    check_registered(&r, "reg_omega_kOmegaSST_GbyNu", "T_GbyNu");
    let mesh = r.mesh();
    let mut m = KOmegaSST::new(mesh.clone());
    m.k = r.zg(&mesh, "k", "k");
    m.omega = r.zg(&mesh, "omega", "omega");
    m.nu = r.zg(&mesh, "nu", "nu");
    m.u = r.velocity(&mesh);
    m.phi = r.flux(&mesh);
    let y_rows = vec![(
        "kOmegaSST port wall distance vs meshWave".to_string(),
        cmp("kOmegaSST port wall distance vs meshWave", m.wall_distance(), &r.s("y")),
    )];
    pin(&y_rows, PINS_SST_Y);
    let pairs: Pairs = &[
        ("y", "y"),
        ("nut_real", "nut"),
        ("T_S2", "S2"),
        ("T_CDkOmega", "CDkOmega"),
        ("T_F1", "F1"),
        ("T_F23", "F23"),
        ("T_GbyNu0", "GbyNu0"),
        ("T_G", "G"),
        ("T_gamma", "gamma"),
        ("T_beta", "beta"),
        ("T_GbyNu", "GbyNu"),
        ("T_Pk", "Pk"),
        ("T_omega_E", "omega_E"),
        ("T_omega_SuSp1", "omega_SuSp1"),
        ("T_omega_Sp", "omega_Sp"),
        ("T_omega_SuSp2", "omega_SuSp2"),
        ("T_k_E", "k_E"),
        ("T_k_SuSp", "k_SuSp"),
        ("T_k_Sp", "k_Sp"),
    ];
    let st_y = r.state(true, false);
    let st_own = r.state(false, false);
    let esi = compare("kOmegaSST esi  (esi y)", &r, &m.source_terms(SourceForm::EsiV2506, &st_y), pairs);
    // the arithmetic is upstream's; the only difference is PETIR `tanh` against glibc `tanh`
    // (a python glibc replay reproduces upstream F1 and F23 360/360, see the README)
    assert!(esi.iter().all(|(_, s)| s.max_ulp <= 3), "libm-level differences only");
    pin(&esi, PINS_SST_ESI);
    let mut rows = compare("kOmegaSST port (esi y)", &r, &m.source_terms(SourceForm::Port, &st_y), pairs);
    rows.extend(compare("kOmegaSST port (own y)", &r, &m.source_terms(SourceForm::Port, &st_own), pairs));
    rows.extend(compare("kOmegaSST esi  (own y)", &r, &m.source_terms(SourceForm::EsiV2506, &st_own), pairs));
    // explicit cross-diffusion source: (1-F1) CDkw against upstream -SuSp2*omega
    let omega = r.s("omega");
    let susp2 = r.s("T_omega_SuSp2");
    let up_cross: Vec<f64> = susp2.iter().zip(&omega).map(|(s, w)| -s * w).collect();
    let pt = m.source_terms(SourceForm::Port, &st_y);
    let l1 = "kOmegaSST port (esi y) omega_cross vs -SuSp2*omega".to_string();
    rows.push((l1.clone(), cmp(&l1, pt.get("omega_cross").unwrap(), &up_cross)));
    let et = m.source_terms(SourceForm::EsiV2506, &st_y);
    let l2 = "kOmegaSST esi  (esi y) omega_cross vs -SuSp2*omega".to_string();
    rows.push((l2.clone(), cmp(&l2, et.get("omega_cross").unwrap(), &up_cross)));
    pin(&rows, PINS_SST);
}

#[test]
fn spalart_allmaras_vs_openfoam() {
    let r = parse(REF_SA);
    check_audits(&r, 4);
    let mesh = r.mesh();
    let mut m = SpalartAllmaras::new(mesh.clone());
    m.nu_tilde = r.zg(&mesh, "nuTilda", "nuTilda");
    m.nu = r.zg(&mesh, "nu", "nu");
    m.u = r.velocity(&mesh);
    m.phi = r.flux(&mesh);
    let y_rows = vec![(
        "SA port wall distance vs meshWave".to_string(),
        cmp("SA port wall distance vs meshWave", m.wall_distance(), &r.s("y")),
    )];
    pin(&y_rows, PINS_SA_Y);
    let pairs: Pairs = &[
        ("nut_real", "nut"),
        ("T_chi", "chi"),
        ("T_fv1", "fv1"),
        ("T_fv2", "fv2"),
        ("T_Omega", "Omega"),
        ("T_Stilda", "Stilda"),
        ("T_r", "r"),
        ("T_fw", "fw"),
        ("T_gradSqr", "gradSqr"),
        ("T_nuTilda_E", "nuTilda_E"),
        ("T_nuTilda_Sp", "nuTilda_Sp"),
    ];
    let st_y = r.state(true, false);
    let st_own = r.state(false, false);
    let esi = compare("SA esi  (esi y)", &r, &m.source_terms(SourceForm::EsiV2506, &st_y), pairs);
    all_identical(&esi);
    let mut rows = compare("SA port (esi y)", &r, &m.source_terms(SourceForm::Port, &st_y), pairs);
    rows.extend(compare("SA port (own y)", &r, &m.source_terms(SourceForm::Port, &st_own), pairs));
    rows.extend(compare("SA esi  (own y)", &r, &m.source_terms(SourceForm::EsiV2506, &st_own), pairs));
    // Cb2/sigma |grad nuTilda|^2 is not dumped: port (sigma 2/3) against the ESI form (0.66666)
    let (pt, et) = (
        m.source_terms(SourceForm::Port, &st_y),
        m.source_terms(SourceForm::EsiV2506, &st_y),
    );
    rows.push(compare_forms("SA port-vs-esi (esi y)", &pt, &et, "cb2_term"));
    pin(&rows, PINS_SA);
}

#[test]
fn smagorinsky_vs_openfoam() {
    let r = parse(REF_SMAG);
    assert!(r.has("T_delta") && r.audits.is_empty());
    let mesh = r.mesh();
    let mut m = Smagorinsky::new(mesh.clone());
    m.nu = r.zg(&mesh, "nu", "nu");
    m.u = r.velocity(&mesh);
    let pairs: Pairs = &[
        ("nut_real", "nut"),
        ("T_delta", "delta"),
        ("T_k", "k"),
        ("T_nut", "nut"),
    ];
    // own delta (cube root by PETIR) and upstream's delta fed in (isolates the formula)
    let own = r.state(false, false);
    let up = r.state(false, true);
    let mut rows = compare("Smagorinsky esi  (own delta)", &r, &m.source_terms(SourceForm::EsiV2506, &own), pairs);
    rows.extend(compare("Smagorinsky port (own delta)", &r, &m.source_terms(SourceForm::Port, &own), pairs));
    let esi_up = compare("Smagorinsky esi  (esi delta)", &r, &m.source_terms(SourceForm::EsiV2506, &up), pairs);
    all_identical(&esi_up);
    rows.extend(esi_up);
    rows.extend(compare("Smagorinsky port (esi delta)", &r, &m.source_terms(SourceForm::Port, &up), pairs));
    pin(&rows, PINS_SMAG);
}

const PINS_K_EPSILON_PORT: &[(&str, usize)] = &[
    ("kEpsilon port nut", 267),
    ("kEpsilon port GbyNu", 0),
    ("kEpsilon port G", 0),
    ("kEpsilon port eps_E", 0),
    ("kEpsilon port eps_SuSp", 0),
    ("kEpsilon port eps_Sp", 231),
    ("kEpsilon port k_E", 0),
    ("kEpsilon port k_SuSp", 0),
    ("kEpsilon port k_Sp", 360),
];
const PINS_K_OMEGA_PORT: &[(&str, usize)] = &[
    ("kOmega port nut", 360),
    ("kOmega port GbyNu", 0),
    ("kOmega port G", 0),
    ("kOmega port omega_E", 0),
    ("kOmega port omega_SuSp", 0),
    ("kOmega port omega_Sp", 360),
    ("kOmega port k_E", 0),
    ("kOmega port k_SuSp", 0),
    ("kOmega port k_Sp", 360),
];
const PINS_SST_ESI: &[(&str, usize)] = &[
    ("kOmegaSST esi  (esi y) y", 360),
    ("kOmegaSST esi  (esi y) nut", 360),
    ("kOmegaSST esi  (esi y) S2", 360),
    ("kOmegaSST esi  (esi y) CDkOmega", 360),
    ("kOmegaSST esi  (esi y) F1", 334),
    ("kOmegaSST esi  (esi y) F23", 339),
    ("kOmegaSST esi  (esi y) GbyNu0", 360),
    ("kOmegaSST esi  (esi y) G", 360),
    ("kOmegaSST esi  (esi y) gamma", 356),
    ("kOmegaSST esi  (esi y) beta", 358),
    ("kOmegaSST esi  (esi y) GbyNu", 360),
    ("kOmegaSST esi  (esi y) Pk", 360),
    ("kOmegaSST esi  (esi y) omega_E", 357),
    ("kOmegaSST esi  (esi y) omega_SuSp1", 358),
    ("kOmegaSST esi  (esi y) omega_Sp", 358),
    ("kOmegaSST esi  (esi y) omega_SuSp2", 343),
    ("kOmegaSST esi  (esi y) k_E", 360),
    ("kOmegaSST esi  (esi y) k_SuSp", 360),
    ("kOmegaSST esi  (esi y) k_Sp", 360),
];
const PINS_SST_Y: &[(&str, usize)] = &[
    ("kOmegaSST port wall distance vs meshWave", 265),
];
const PINS_SST: &[(&str, usize)] = &[
    ("kOmegaSST port (esi y) y", 360),
    ("kOmegaSST port (esi y) nut", 360),
    ("kOmegaSST port (esi y) S2", 360),
    ("kOmegaSST port (esi y) CDkOmega", 23),
    ("kOmegaSST port (esi y) F1", 268),
    ("kOmegaSST port (esi y) F23", 316),
    ("kOmegaSST port (esi y) GbyNu0", 0),
    ("kOmegaSST port (esi y) G", 0),
    ("kOmegaSST port (esi y) gamma", 0),
    ("kOmegaSST port (esi y) beta", 294),
    ("kOmegaSST port (esi y) GbyNu", 0),
    ("kOmegaSST port (esi y) Pk", 2),
    ("kOmegaSST port (esi y) omega_E", 0),
    ("kOmegaSST port (esi y) omega_SuSp1", 0),
    ("kOmegaSST port (esi y) omega_Sp", 294),
    ("kOmegaSST port (esi y) omega_SuSp2", 148),
    ("kOmegaSST port (esi y) k_E", 2),
    ("kOmegaSST port (esi y) k_SuSp", 0),
    ("kOmegaSST port (esi y) k_Sp", 360),
    ("kOmegaSST port (own y) y", 265),
    ("kOmegaSST port (own y) nut", 360),
    ("kOmegaSST port (own y) S2", 360),
    ("kOmegaSST port (own y) CDkOmega", 23),
    ("kOmegaSST port (own y) F1", 247),
    ("kOmegaSST port (own y) F23", 281),
    ("kOmegaSST port (own y) GbyNu0", 0),
    ("kOmegaSST port (own y) G", 0),
    ("kOmegaSST port (own y) gamma", 0),
    ("kOmegaSST port (own y) beta", 267),
    ("kOmegaSST port (own y) GbyNu", 0),
    ("kOmegaSST port (own y) Pk", 2),
    ("kOmegaSST port (own y) omega_E", 0),
    ("kOmegaSST port (own y) omega_SuSp1", 0),
    ("kOmegaSST port (own y) omega_Sp", 267),
    ("kOmegaSST port (own y) omega_SuSp2", 148),
    ("kOmegaSST port (own y) k_E", 2),
    ("kOmegaSST port (own y) k_SuSp", 0),
    ("kOmegaSST port (own y) k_Sp", 360),
    ("kOmegaSST esi  (own y) y", 265),
    ("kOmegaSST esi  (own y) nut", 360),
    ("kOmegaSST esi  (own y) S2", 360),
    ("kOmegaSST esi  (own y) CDkOmega", 360),
    ("kOmegaSST esi  (own y) F1", 288),
    ("kOmegaSST esi  (own y) F23", 300),
    ("kOmegaSST esi  (own y) GbyNu0", 360),
    ("kOmegaSST esi  (own y) G", 360),
    ("kOmegaSST esi  (own y) gamma", 310),
    ("kOmegaSST esi  (own y) beta", 312),
    ("kOmegaSST esi  (own y) GbyNu", 360),
    ("kOmegaSST esi  (own y) Pk", 360),
    ("kOmegaSST esi  (own y) omega_E", 312),
    ("kOmegaSST esi  (own y) omega_SuSp1", 310),
    ("kOmegaSST esi  (own y) omega_Sp", 312),
    ("kOmegaSST esi  (own y) omega_SuSp2", 296),
    ("kOmegaSST esi  (own y) k_E", 360),
    ("kOmegaSST esi  (own y) k_SuSp", 360),
    ("kOmegaSST esi  (own y) k_Sp", 360),
    ("kOmegaSST port (esi y) omega_cross vs -SuSp2*omega", 164),
    ("kOmegaSST esi  (esi y) omega_cross vs -SuSp2*omega", 343),
];
const PINS_SA_Y: &[(&str, usize)] = &[
    ("SA port wall distance vs meshWave", 265),
];
const PINS_SA: &[(&str, usize)] = &[
    ("SA port (esi y) nut", 360),
    ("SA port (esi y) chi", 360),
    ("SA port (esi y) fv1", 360),
    ("SA port (esi y) fv2", 360),
    ("SA port (esi y) Omega", 360),
    ("SA port (esi y) Stilda", 360),
    ("SA port (esi y) r", 360),
    ("SA port (esi y) fw", 360),
    ("SA port (esi y) gradSqr", 192),
    ("SA port (esi y) nuTilda_E", 360),
    ("SA port (esi y) nuTilda_Sp", 0),
    ("SA port (own y) nut", 360),
    ("SA port (own y) chi", 360),
    ("SA port (own y) fv1", 360),
    ("SA port (own y) fv2", 360),
    ("SA port (own y) Omega", 360),
    ("SA port (own y) Stilda", 303),
    ("SA port (own y) r", 265),
    ("SA port (own y) fw", 265),
    ("SA port (own y) gradSqr", 192),
    ("SA port (own y) nuTilda_E", 303),
    ("SA port (own y) nuTilda_Sp", 0),
    ("SA esi  (own y) nut", 360),
    ("SA esi  (own y) chi", 360),
    ("SA esi  (own y) fv1", 360),
    ("SA esi  (own y) fv2", 360),
    ("SA esi  (own y) Omega", 360),
    ("SA esi  (own y) Stilda", 303),
    ("SA esi  (own y) r", 265),
    ("SA esi  (own y) fw", 265),
    ("SA esi  (own y) gradSqr", 360),
    ("SA esi  (own y) nuTilda_E", 303),
    ("SA esi  (own y) nuTilda_Sp", 265),
    ("SA port-vs-esi (esi y) cb2_term", 0),
];
const PINS_SMAG: &[(&str, usize)] = &[
    ("Smagorinsky esi  (own delta) nut", 170),
    ("Smagorinsky esi  (own delta) delta", 156),
    ("Smagorinsky esi  (own delta) k", 189),
    ("Smagorinsky esi  (own delta) nut", 170),
    ("Smagorinsky port (own delta) nut", 170),
    ("Smagorinsky port (own delta) delta", 156),
    ("Smagorinsky port (own delta) k", 189),
    ("Smagorinsky port (own delta) nut", 170),
    ("Smagorinsky esi  (esi delta) nut", 360),
    ("Smagorinsky esi  (esi delta) delta", 360),
    ("Smagorinsky esi  (esi delta) k", 360),
    ("Smagorinsky esi  (esi delta) nut", 360),
    ("Smagorinsky port (esi delta) nut", 360),
    ("Smagorinsky port (esi delta) delta", 360),
    ("Smagorinsky port (esi delta) k", 360),
    ("Smagorinsky port (esi delta) nut", 360),
];

// ── gh:#1377: RNGkEpsilon, realizableKE, WALE (ESI arithmetic only) ─────────

/// Largest ulp distance over the rows.
fn max_ulp(rows: &Rows) -> u64 {
    rows.iter().map(|(_, s)| s.max_ulp).max().unwrap_or(0)
}

#[test]
fn rng_k_epsilon_vs_openfoam() {
    let r = parse(REF_RNG);
    check_audits(&r, 8);
    let mesh = r.mesh();
    let mut m = RngKEpsilon::new(mesh.clone());
    m.k = r.zg(&mesh, "k", "k");
    m.epsilon = r.zg(&mesh, "epsilon", "epsilon");
    m.nu = r.zg(&mesh, "nu", "nu");
    m.nu_t = r.zg(&mesh, "nut", "nut_real");
    m.u = r.velocity(&mesh);
    m.phi = r.flux(&mesh);
    let pairs: Pairs = &[
        ("nut_real", "nut"),
        ("T_nut", "nut"),
        ("T_GbyNu", "GbyNu"),
        ("T_G", "G"),
        ("T_eta", "eta"),
        ("T_R", "R"),
        ("T_eps_E", "eps_E"),
        ("T_eps_SuSp", "eps_SuSp"),
        ("T_eps_Sp", "eps_Sp"),
        ("T_k_E", "k_E"),
        ("T_k_SuSp", "k_SuSp"),
        ("T_k_Sp", "k_Sp"),
    ];
    let rows = compare("RNGkEpsilon", &r, &m.source_terms(&r.state(false, false)), pairs);
    all_identical(&rows);
}

#[test]
fn realizable_ke_vs_openfoam() {
    let r = parse(REF_REALIZABLE);
    check_audits(&r, 8);
    let mesh = r.mesh();
    let mut m = RealizableKE::new(mesh.clone());
    m.k = r.zg(&mesh, "k", "k");
    m.epsilon = r.zg(&mesh, "epsilon", "epsilon");
    m.nu = r.zg(&mesh, "nu", "nu");
    m.nu_t = r.zg(&mesh, "nut", "nut_real");
    m.u = r.velocity(&mesh);
    m.phi = r.flux(&mesh);
    let pairs: Pairs = &[
        ("nut_real", "nut"),
        ("T_S2", "S2"),
        ("T_magS", "magS"),
        ("T_W", "W"),
        ("T_phis", "phis"),
        ("T_As", "As"),
        ("T_Us", "Us"),
        ("T_rCmu", "rCmu"),
        ("T_nut", "nut"),
        ("T_eta", "eta"),
        ("T_C1", "C1"),
        ("T_G", "G"),
        ("T_eps_E", "eps_E"),
        ("T_eps_Sp", "eps_Sp"),
        ("T_k_E", "k_E"),
        ("T_k_SuSp", "k_SuSp"),
        ("T_k_Sp", "k_Sp"),
    ];
    let rows = compare("realizableKE", &r, &m.source_terms(&r.state(false, false)), pairs);
    // No libm before phis: these must be bit-identical.
    for (l, s) in &rows {
        if ["S2", "magS", " W"].iter().any(|t| l.ends_with(t)) {
            assert_eq!(s.identical, s.n, "{l}");
        }
    }
    assert!(max_ulp(&rows) <= REALIZABLE_MAX_ULP, "max ulp {}", max_ulp(&rows));
    pin(&rows, PINS_REALIZABLE);
}

#[test]
fn wale_vs_openfoam() {
    let r = parse(REF_WALE);
    assert!(r.has("T_delta") && r.audits.is_empty());
    let mesh = r.mesh();
    let mut m = Wale::new(mesh.clone());
    m.nu = r.zg(&mesh, "nu", "nu");
    m.u = r.velocity(&mesh);
    let pairs: Pairs = &[
        ("nut_real", "nut"),
        ("T_delta", "delta"),
        ("T_magSqrSd", "magSqrSd"),
        ("T_magSqrS", "magSqrS"),
        ("T_pow52", "pow52"),
        ("T_pow54", "pow54"),
        ("T_k", "k"),
        ("T_nut", "nut"),
    ];
    let own = r.state(false, false);
    let up = r.state(false, true);
    let mut rows = compare("WALE (own delta)", &r, &m.source_terms(&own), pairs);
    let esi_delta = compare("WALE (esi delta)", &r, &m.source_terms(&up), pairs);
    for (l, s) in &esi_delta {
        if l.ends_with("magSqrSd") || l.ends_with("magSqrS") {
            assert_eq!(s.identical, s.n, "{l}");
        }
    }
    assert!(max_ulp(&esi_delta) <= WALE_MAX_ULP, "max ulp {}", max_ulp(&esi_delta));
    rows.extend(esi_delta);
    assert!(max_ulp(&rows) <= 7, "own-delta max ulp {}", max_ulp(&rows));
    pin(&rows, PINS_WALE);
}

const REALIZABLE_MAX_ULP: u64 = 4;
const WALE_MAX_ULP: u64 = 1;
const PINS_REALIZABLE: &[(&str, usize)] = &[
    ("realizableKE nut", 346),
    ("realizableKE S2", 360),
    ("realizableKE magS", 360),
    ("realizableKE W", 360),
    ("realizableKE phis", 353),
    ("realizableKE As", 344),
    ("realizableKE Us", 360),
    ("realizableKE rCmu", 346),
    ("realizableKE nut", 346),
    ("realizableKE eta", 360),
    ("realizableKE C1", 360),
    ("realizableKE G", 360),
    ("realizableKE eps_E", 360),
    ("realizableKE eps_Sp", 360),
    ("realizableKE k_E", 360),
    ("realizableKE k_SuSp", 360),
    ("realizableKE k_Sp", 360),
];
const PINS_WALE: &[(&str, usize)] = &[
    ("WALE (own delta) nut", 173),
    ("WALE (own delta) delta", 156),
    ("WALE (own delta) magSqrSd", 360),
    ("WALE (own delta) magSqrS", 360),
    ("WALE (own delta) pow52", 360),
    ("WALE (own delta) pow54", 358),
    ("WALE (own delta) k", 185),
    ("WALE (own delta) nut", 173),
    ("WALE (esi delta) nut", 360),
    ("WALE (esi delta) delta", 360),
    ("WALE (esi delta) magSqrSd", 360),
    ("WALE (esi delta) magSqrS", 360),
    ("WALE (esi delta) pow52", 360),
    ("WALE (esi delta) pow54", 358),
    ("WALE (esi delta) k", 360),
    ("WALE (esi delta) nut", 360),
];
