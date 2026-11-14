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
use outram_foam_turbulence_lib::des::spalart_allmaras_des::{SaDesVariant, SpalartAllmarasDes};
use outram_foam_turbulence_lib::des::KOmegaSstDes;
use outram_foam_turbulence_lib::k_epsilon::KEpsilon;
use outram_foam_turbulence_lib::k_omega::KOmega;
use outram_foam_turbulence_lib::k_omega_sst::KOmegaSST;
use outram_foam_turbulence_lib::k_omega_sst_lm::KOmegaSstLm;
use outram_foam_turbulence_lib::launder_sharma_ke::LaunderSharmaKe;
use outram_foam_turbulence_lib::les::k_eqn::KEqn;
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
            mag_sqr_grad_grad_u: if self.has("magSqrGradGradU") {
                self.s("magSqrGradGradU")
            } else {
                vec![]
            },
            grad_sqrt_k: if self.has("gradSqrtk") { self.v("gradSqrtk") } else { vec![] },
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

fn compare(label: &str, r: &Ref, st: &SourceTerms, pairs: &[(&str, &str)]) -> Rows {
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

// ── gh:#1545: DES family (ESI arithmetic only; upstream y, delta, hmax fed in) ─────────

const REF_SA_DES: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_turb_gh1362/reference/ref_SpalartAllmarasDES.txt.gz"
);
const REF_SA_DDES: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_turb_gh1362/reference/ref_SpalartAllmarasDDES.txt.gz"
);
const REF_SA_IDDES: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_turb_gh1362/reference/ref_SpalartAllmarasIDDES.txt.gz"
);
const REF_SST_DES: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_turb_gh1362/reference/ref_kOmegaSSTDES.txt.gz"
);

const SA_DES_COMMON: Pairs = &[
    ("nut_real", "nut"),
    ("T_nut", "nut"),
    ("T_delta", "delta"),
    ("T_chi", "chi"),
    ("T_fv1", "fv1"),
    ("T_fv2", "fv2"),
    ("T_psi", "psi"),
    ("T_lLES", "lLES"),
    ("T_fd", "fd"),
    ("T_dTilda", "dTilda"),
    ("T_Omega", "Omega"),
    ("T_Stilda", "Stilda"),
    ("T_r", "r"),
    ("T_fw", "fw"),
    ("T_gradSqr", "gradSqr"),
    ("T_nuTilda_E", "nuTilda_E"),
    ("T_nuTilda_Sp", "nuTilda_Sp"),
    ("T_LESRegion", "LESRegion"),
];
const SA_DDES_EXTRA: Pairs = &[("T_r_fd", "r_fd")];
const SA_IDDES_EXTRA: Pairs = &[
    ("T_alpha", "alpha"),
    ("T_fB", "fB"),
    ("T_fdt", "fdt"),
    ("T_ft", "ft"),
    ("T_fl", "fl"),
    ("T_fe1", "fe1"),
    ("T_fe", "fe"),
];

fn sa_des_case(
    name: &str,
    gz: &[u8],
    variant: SaDesVariant,
    extra: Pairs,
    own_delta: bool,
) -> Rows {
    let r = parse(gz);
    check_audits(&r, 4);
    assert!(r.has("delta_real") && (variant != SaDesVariant::Iddes || r.has("hmax_real")));
    let mesh = r.mesh();
    let mut m = SpalartAllmarasDes::new(mesh.clone(), variant);
    m.nu_tilde = r.zg(&mesh, "nuTilda", "nuTilda");
    m.nu = r.zg(&mesh, "nu", "nu");
    m.nu_t = r.zg(&mesh, "nut", "nut_real");
    if variant == SaDesVariant::Iddes {
        m.h_max = Some(r.s("hmax_real"));
    }
    let mut pairs: Vec<(&str, &str)> = SA_DES_COMMON.to_vec();
    pairs.extend_from_slice(extra);
    let mut rows = compare(
        &format!("{name} (esi y, esi delta)"),
        &r,
        &m.source_terms(&r.state(true, true)),
        &pairs,
    );
    if own_delta {
        rows.extend(compare(
            &format!("{name} (esi y, own delta)"),
            &r,
            &m.source_terms(&r.state(true, false)),
            &pairs,
        ));
    }
    rows
}

#[test]
fn sa_des_vs_openfoam() {
    let rows = sa_des_case("SA-DES", REF_SA_DES, SaDesVariant::Des, &[], true);
    // no libm before fw's pow(x, 1/6): every term is identical with upstream y and delta
    for (l, st) in rows.iter().filter(|(l, _)| l.contains("esi delta")) {
        assert_eq!(st.identical, st.n, "{l}");
    }
    assert!(max_ulp(&rows) <= 10, "max ulp {}", max_ulp(&rows)); // own cbrt delta
    pin(&rows, PINS_SA_DES);
}

#[test]
fn sa_ddes_vs_openfoam() {
    let rows = sa_des_case("SA-DDES", REF_SA_DDES, SaDesVariant::Ddes, SA_DDES_EXTRA, true);
    // pow and tanh in fd (a glibc replay reproduces upstream fd 360/360, README)
    assert!(max_ulp(&rows) <= 14, "max ulp {}", max_ulp(&rows));
    pin(&rows, PINS_SA_DDES);
}

#[test]
fn sa_iddes_vs_openfoam() {
    let rows = sa_des_case("SA-IDDES", REF_SA_IDDES, SaDesVariant::Iddes, SA_IDDES_EXTRA, false);
    assert!(max_ulp(&rows) <= 2, "max ulp {}", max_ulp(&rows));
    pin(&rows, PINS_SA_IDDES);
}

#[test]
fn k_omega_sst_des_vs_openfoam() {
    let r = parse(REF_SST_DES);
    check_audits(&r, 8);
    let mesh = r.mesh();
    let mut m = KOmegaSstDes::new(mesh.clone());
    m.sst.k = r.zg(&mesh, "k", "k");
    m.sst.omega = r.zg(&mesh, "omega", "omega");
    m.sst.nu = r.zg(&mesh, "nu", "nu");
    m.sst.nu_t = r.zg(&mesh, "nut", "nut_real");
    m.sst.u = r.velocity(&mesh);
    m.sst.phi = r.flux(&mesh);
    let pairs: Pairs = &[
        ("nut_real", "nut"),
        ("T_delta", "delta"),
        ("T_S2", "S2"),
        ("T_CDkOmega", "CDkOmega"),
        ("T_F1", "F1"),
        ("T_F23", "F23"),
        ("T_nut", "nut"),
        ("T_CDES", "CDES"),
        ("T_lRAS", "lRAS"),
        ("T_lLES", "lLES"),
        ("T_dTilda", "dTilda"),
        ("T_epsByk", "epsByk"),
        ("T_LESRegion", "LESRegion"),
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
    let rows = compare("kOmegaSSTDES (esi y, esi delta)", &r, &m.source_terms(&r.state(true, true)), pairs);
    assert!(max_ulp(&rows) <= 3, "max ulp {}", max_ulp(&rows));
    pin(&rows, PINS_SST_DES);
}

const PINS_SA_DES: &[(&str, usize)] = &[
    ("SA-DES (esi y, esi delta) nut", 360),
    ("SA-DES (esi y, esi delta) nut", 360),
    ("SA-DES (esi y, esi delta) delta", 360),
    ("SA-DES (esi y, esi delta) chi", 360),
    ("SA-DES (esi y, esi delta) fv1", 360),
    ("SA-DES (esi y, esi delta) fv2", 360),
    ("SA-DES (esi y, esi delta) psi", 360),
    ("SA-DES (esi y, esi delta) lLES", 360),
    ("SA-DES (esi y, esi delta) fd", 360),
    ("SA-DES (esi y, esi delta) dTilda", 360),
    ("SA-DES (esi y, esi delta) Omega", 360),
    ("SA-DES (esi y, esi delta) Stilda", 360),
    ("SA-DES (esi y, esi delta) r", 360),
    ("SA-DES (esi y, esi delta) fw", 360),
    ("SA-DES (esi y, esi delta) gradSqr", 360),
    ("SA-DES (esi y, esi delta) nuTilda_E", 360),
    ("SA-DES (esi y, esi delta) nuTilda_Sp", 360),
    ("SA-DES (esi y, esi delta) LESRegion", 360),
    ("SA-DES (esi y, own delta) nut", 360),
    ("SA-DES (esi y, own delta) nut", 360),
    ("SA-DES (esi y, own delta) delta", 156),
    ("SA-DES (esi y, own delta) chi", 360),
    ("SA-DES (esi y, own delta) fv1", 360),
    ("SA-DES (esi y, own delta) fv2", 360),
    ("SA-DES (esi y, own delta) psi", 360),
    ("SA-DES (esi y, own delta) lLES", 186),
    ("SA-DES (esi y, own delta) fd", 360),
    ("SA-DES (esi y, own delta) dTilda", 335),
    ("SA-DES (esi y, own delta) Omega", 360),
    ("SA-DES (esi y, own delta) Stilda", 360),
    ("SA-DES (esi y, own delta) r", 336),
    ("SA-DES (esi y, own delta) fw", 337),
    ("SA-DES (esi y, own delta) gradSqr", 360),
    ("SA-DES (esi y, own delta) nuTilda_E", 360),
    ("SA-DES (esi y, own delta) nuTilda_Sp", 335),
    ("SA-DES (esi y, own delta) LESRegion", 360),
];
const PINS_SA_DDES: &[(&str, usize)] = &[
    ("SA-DDES (esi y, esi delta) nut", 360),
    ("SA-DDES (esi y, esi delta) nut", 360),
    ("SA-DDES (esi y, esi delta) delta", 360),
    ("SA-DDES (esi y, esi delta) chi", 360),
    ("SA-DDES (esi y, esi delta) fv1", 360),
    ("SA-DDES (esi y, esi delta) fv2", 360),
    ("SA-DDES (esi y, esi delta) psi", 360),
    ("SA-DDES (esi y, esi delta) lLES", 360),
    ("SA-DDES (esi y, esi delta) fd", 355),
    ("SA-DDES (esi y, esi delta) dTilda", 360),
    ("SA-DDES (esi y, esi delta) Omega", 360),
    ("SA-DDES (esi y, esi delta) Stilda", 360),
    ("SA-DDES (esi y, esi delta) r", 360),
    ("SA-DDES (esi y, esi delta) fw", 360),
    ("SA-DDES (esi y, esi delta) gradSqr", 360),
    ("SA-DDES (esi y, esi delta) nuTilda_E", 360),
    ("SA-DDES (esi y, esi delta) nuTilda_Sp", 360),
    ("SA-DDES (esi y, esi delta) LESRegion", 360),
    ("SA-DDES (esi y, esi delta) r_fd", 360),
    ("SA-DDES (esi y, own delta) nut", 360),
    ("SA-DDES (esi y, own delta) nut", 360),
    ("SA-DDES (esi y, own delta) delta", 156),
    ("SA-DDES (esi y, own delta) chi", 360),
    ("SA-DDES (esi y, own delta) fv1", 360),
    ("SA-DDES (esi y, own delta) fv2", 360),
    ("SA-DDES (esi y, own delta) psi", 360),
    ("SA-DDES (esi y, own delta) lLES", 186),
    ("SA-DDES (esi y, own delta) fd", 355),
    ("SA-DDES (esi y, own delta) dTilda", 341),
    ("SA-DDES (esi y, own delta) Omega", 360),
    ("SA-DDES (esi y, own delta) Stilda", 360),
    ("SA-DDES (esi y, own delta) r", 341),
    ("SA-DDES (esi y, own delta) fw", 341),
    ("SA-DDES (esi y, own delta) gradSqr", 360),
    ("SA-DDES (esi y, own delta) nuTilda_E", 360),
    ("SA-DDES (esi y, own delta) nuTilda_Sp", 341),
    ("SA-DDES (esi y, own delta) LESRegion", 360),
    ("SA-DDES (esi y, own delta) r_fd", 360),
];
const PINS_SA_IDDES: &[(&str, usize)] = &[
    ("SA-IDDES (esi y, esi delta) nut", 360),
    ("SA-IDDES (esi y, esi delta) nut", 360),
    ("SA-IDDES (esi y, esi delta) delta", 360),
    ("SA-IDDES (esi y, esi delta) chi", 360),
    ("SA-IDDES (esi y, esi delta) fv1", 360),
    ("SA-IDDES (esi y, esi delta) fv2", 360),
    ("SA-IDDES (esi y, esi delta) psi", 360),
    ("SA-IDDES (esi y, esi delta) lLES", 360),
    ("SA-IDDES (esi y, esi delta) fd", 360),
    ("SA-IDDES (esi y, esi delta) dTilda", 360),
    ("SA-IDDES (esi y, esi delta) Omega", 360),
    ("SA-IDDES (esi y, esi delta) Stilda", 360),
    ("SA-IDDES (esi y, esi delta) r", 360),
    ("SA-IDDES (esi y, esi delta) fw", 360),
    ("SA-IDDES (esi y, esi delta) gradSqr", 360),
    ("SA-IDDES (esi y, esi delta) nuTilda_E", 360),
    ("SA-IDDES (esi y, esi delta) nuTilda_Sp", 360),
    ("SA-IDDES (esi y, esi delta) LESRegion", 360),
    ("SA-IDDES (esi y, esi delta) alpha", 360),
    ("SA-IDDES (esi y, esi delta) fB", 360),
    ("SA-IDDES (esi y, esi delta) fdt", 343),
    ("SA-IDDES (esi y, esi delta) ft", 360),
    ("SA-IDDES (esi y, esi delta) fl", 360),
    ("SA-IDDES (esi y, esi delta) fe1", 360),
    ("SA-IDDES (esi y, esi delta) fe", 360),
];
const PINS_SST_DES: &[(&str, usize)] = &[
    ("kOmegaSSTDES (esi y, esi delta) nut", 360),
    ("kOmegaSSTDES (esi y, esi delta) delta", 360),
    ("kOmegaSSTDES (esi y, esi delta) S2", 360),
    ("kOmegaSSTDES (esi y, esi delta) CDkOmega", 360),
    ("kOmegaSSTDES (esi y, esi delta) F1", 334),
    ("kOmegaSSTDES (esi y, esi delta) F23", 339),
    ("kOmegaSSTDES (esi y, esi delta) nut", 360),
    ("kOmegaSSTDES (esi y, esi delta) CDES", 356),
    ("kOmegaSSTDES (esi y, esi delta) lRAS", 360),
    ("kOmegaSSTDES (esi y, esi delta) lLES", 357),
    ("kOmegaSSTDES (esi y, esi delta) dTilda", 359),
    ("kOmegaSSTDES (esi y, esi delta) epsByk", 359),
    ("kOmegaSSTDES (esi y, esi delta) LESRegion", 360),
    ("kOmegaSSTDES (esi y, esi delta) GbyNu0", 360),
    ("kOmegaSSTDES (esi y, esi delta) G", 360),
    ("kOmegaSSTDES (esi y, esi delta) gamma", 356),
    ("kOmegaSSTDES (esi y, esi delta) beta", 358),
    ("kOmegaSSTDES (esi y, esi delta) GbyNu", 360),
    ("kOmegaSSTDES (esi y, esi delta) Pk", 360),
    ("kOmegaSSTDES (esi y, esi delta) omega_E", 357),
    ("kOmegaSSTDES (esi y, esi delta) omega_SuSp1", 358),
    ("kOmegaSSTDES (esi y, esi delta) omega_Sp", 358),
    ("kOmegaSSTDES (esi y, esi delta) omega_SuSp2", 343),
    ("kOmegaSSTDES (esi y, esi delta) k_E", 360),
    ("kOmegaSSTDES (esi y, esi delta) k_SuSp", 360),
    ("kOmegaSSTDES (esi y, esi delta) k_Sp", 359),
];

// ── gh:#1544: kEqn (ESI arithmetic only; upstream delta fed in for the bit-identical rows) ──

const REF_KEQN: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_turb_gh1362/reference/ref_kEqn.txt.gz"
);

#[test]
fn k_eqn_vs_openfoam() {
    let r = parse(REF_KEQN);
    check_audits(&r, 4);
    let mesh = r.mesh();
    let mut m = KEqn::new(mesh.clone());
    m.k = r.zg(&mesh, "k", "k");
    m.nu = r.zg(&mesh, "nu", "nu");
    m.nu_t = r.zg(&mesh, "nut", "nut_real");
    m.u = r.velocity(&mesh);
    m.phi = r.flux(&mesh);
    let pairs: Pairs = &[
        ("nut_real", "nut"),
        ("T_delta", "delta"),
        ("T_nut", "nut"),
        ("T_GbyNu", "GbyNu"),
        ("T_G", "G"),
        ("T_k_SuSp", "k_SuSp"),
        ("T_k_Sp", "k_Sp"),
    ];
    let esi = compare("kEqn (esi delta)", &r, &m.source_terms(&r.state(false, true)), pairs);
    all_identical(&esi);
    let mut rows = esi;
    rows.extend(compare("kEqn (own delta)", &r, &m.source_terms(&r.state(false, false)), pairs));
    assert!(max_ulp(&rows) <= 7, "max ulp {}", max_ulp(&rows));
    pin(&rows, PINS_KEQN);
}

const PINS_KEQN: &[(&str, usize)] = &[
    ("kEqn (esi delta) nut", 360),
    ("kEqn (esi delta) delta", 360),
    ("kEqn (esi delta) nut", 360),
    ("kEqn (esi delta) GbyNu", 360),
    ("kEqn (esi delta) G", 360),
    ("kEqn (esi delta) k_SuSp", 360),
    ("kEqn (esi delta) k_Sp", 360),
    ("kEqn (own delta) nut", 158),
    ("kEqn (own delta) delta", 156),
    ("kEqn (own delta) nut", 158),
    ("kEqn (own delta) GbyNu", 360),
    ("kEqn (own delta) G", 360),
    ("kEqn (own delta) k_SuSp", 360),
    ("kEqn (own delta) k_Sp", 156),
];

// ── gh:#1544: LaunderSharmaKE (ESI arithmetic only; upstream gradients fed in) ─────────

const REF_LS: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_turb_gh1362/reference/ref_LaunderSharmaKE.txt.gz"
);

const REF_LS_LOW: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_turb_gh1362/reference/ref_LaunderSharmaKE_lowRe.txt.gz"
);

#[test]
fn launder_sharma_ke_vs_openfoam() {
    let rows = launder_sharma_case("LaunderSharmaKE", REF_LS);
    pin(&rows, PINS_LS);
}

/// nu = 1e-2 (README slice 6): fMu in (0.03, 0.13) and f2 in (0.76, 1), so both `exp`
/// calls see varied arguments (at nu = 1e-5 fMu >= 0.97 and f2 is exactly 1 everywhere).
#[test]
fn launder_sharma_ke_low_re_vs_openfoam() {
    let rows = launder_sharma_case("LaunderSharmaKE lowRe", REF_LS_LOW);
    pin(&rows, PINS_LS_LOW);
}

fn launder_sharma_case(name: &str, gz: &[u8]) -> Rows {
    let r = parse(gz);
    check_audits(&r, 8);
    let mesh = r.mesh();
    let mut m = LaunderSharmaKe::new(mesh.clone());
    m.k = r.zg(&mesh, "k", "k");
    m.epsilon = r.zg(&mesh, "epsilon", "epsilon");
    m.nu = r.zg(&mesh, "nu", "nu");
    m.nu_t = r.zg(&mesh, "nut", "nut_real");
    let pairs: Pairs = &[
        ("nut_real", "nut"),
        ("T_fMu", "fMu"),
        ("T_f2", "f2"),
        ("T_nut", "nut"),
        ("T_E", "E"),
        ("T_D", "D"),
        ("T_G", "G"),
        ("T_eps_E1", "eps_E1"),
        ("T_eps_SuSp", "eps_SuSp"),
        ("T_eps_Sp", "eps_Sp"),
        ("T_k_E", "k_E"),
        ("T_k_SuSp", "k_SuSp"),
        ("T_k_Sp", "k_Sp"),
    ];
    let rows = compare(name, &r, &m.source_terms(&r.state(false, false)), pairs);
    assert!(max_ulp(&rows) <= 3, "max ulp {}", max_ulp(&rows));
    rows
}

const PINS_LS: &[(&str, usize)] = &[
    ("LaunderSharmaKE nut", 360),
    ("LaunderSharmaKE fMu", 360),
    ("LaunderSharmaKE f2", 360),
    ("LaunderSharmaKE nut", 360),
    ("LaunderSharmaKE E", 360),
    ("LaunderSharmaKE D", 360),
    ("LaunderSharmaKE G", 360),
    ("LaunderSharmaKE eps_E1", 360),
    ("LaunderSharmaKE eps_SuSp", 360),
    ("LaunderSharmaKE eps_Sp", 360),
    ("LaunderSharmaKE k_E", 360),
    ("LaunderSharmaKE k_SuSp", 360),
    ("LaunderSharmaKE k_Sp", 360),
];
const PINS_LS_LOW: &[(&str, usize)] = &[
    ("LaunderSharmaKE lowRe nut", 360),
    ("LaunderSharmaKE lowRe fMu", 360),
    ("LaunderSharmaKE lowRe f2", 360),
    ("LaunderSharmaKE lowRe nut", 360),
    ("LaunderSharmaKE lowRe E", 360),
    ("LaunderSharmaKE lowRe D", 360),
    ("LaunderSharmaKE lowRe G", 360),
    ("LaunderSharmaKE lowRe eps_E1", 360),
    ("LaunderSharmaKE lowRe eps_SuSp", 360),
    ("LaunderSharmaKE lowRe eps_Sp", 360),
    ("LaunderSharmaKE lowRe k_E", 360),
    ("LaunderSharmaKE lowRe k_SuSp", 360),
    ("LaunderSharmaKE lowRe k_Sp", 360),
];

// ── gh:#1544 slice 7: the LESfilter family (basic-lib) against the real ESI classes ───

use outram_foam_basic_lib::fields::vol_field::VolField;
use outram_foam_basic_lib::fv_operators::les_filter::{
    AnisotropicFilter, FilterValue, LaplaceFilter, LesFilter,
};
use outram_foam_basic_lib::primitives::SymmTensor;

const REF_LESFILTERS: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_turb_gh1362/reference/ref_LESfilters.txt.gz"
);

/// Component access for flattening and for building fields from the dump.
trait Comps: FilterValue {
    fn comps(&self) -> Vec<f64>;
    fn make(a: &[f64]) -> Self;
}

impl Comps for f64 {
    fn comps(&self) -> Vec<f64> {
        vec![*self]
    }
    fn make(a: &[f64]) -> Self {
        a[0]
    }
}

impl Comps for Vector3 {
    fn comps(&self) -> Vec<f64> {
        vec![self.x, self.y, self.z]
    }
    fn make(a: &[f64]) -> Self {
        Vector3::new(a[0], a[1], a[2])
    }
}

impl Comps for SymmTensor {
    fn comps(&self) -> Vec<f64> {
        vec![self.xx, self.xy, self.xz, self.yy, self.yz, self.zz]
    }
    fn make(a: &[f64]) -> Self {
        SymmTensor::new(a[0], a[1], a[2], a[3], a[4], a[5])
    }
}

impl Comps for Tensor {
    fn comps(&self) -> Vec<f64> {
        vec![self.xx, self.xy, self.xz, self.yx, self.yy, self.yz, self.zx, self.zy, self.zz]
    }
    fn make(a: &[f64]) -> Self {
        Tensor::new(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8])
    }
}

/// A field from the dump: cell values under `cell_key`, boundary faces (patch order) under
/// `bnd_key`; `zero_gradient` gives the patches the ZeroGradient BC (upstream's `k`).
fn ref_field<T: Comps>(
    r: &Ref,
    mesh: &Arc<FvMesh>,
    cell_key: &str,
    bnd_key: Option<&str>,
    zero_gradient: bool,
) -> VolField<T> {
    let cells: Vec<T> = r.cols[cell_key].iter().map(|v| T::make(v)).collect();
    let faces: Vec<T> = match bnd_key {
        Some(k) => r.cols[k].iter().map(|v| T::make(v)).collect(),
        None => mesh
            .patches
            .iter()
            .flat_map(|p| (p.start..p.start + p.size).map(|f| cells[mesh.owner[f]].clone()))
            .collect(),
    };
    let boundary = mesh
        .patches
        .iter()
        .map(|p| {
            let b0 = p.start - mesh.n_internal_faces;
            let vals = Field::new(faces[b0..b0 + p.size].to_vec());
            PatchField {
                bc: if zero_gradient {
                    BoundaryCondition::ZeroGradient
                } else {
                    BoundaryCondition::Calculated(vals.clone())
                },
                values: vals,
            }
        })
        .collect();
    VolField::new(cell_key, mesh.clone(), Field::new(cells), boundary)
}

fn flat<T: Comps>(v: &[T]) -> Vec<f64> {
    v.iter().flat_map(|x| x.comps()).collect()
}

fn ref_flat(r: &Ref, key: &str) -> Vec<f64> {
    r.cols[key].iter().flatten().copied().collect()
}

/// Cell and boundary-face rows of one filtered field against the dump.
fn compare_filtered<T: Comps>(label: &str, r: &Ref, out: &VolField<T>, key: &str) -> Rows {
    let l = format!("{label} {key} cells");
    let a = cmp(&l, &flat(out.internal.as_slice()), &ref_flat(r, key));
    let bnd: Vec<T> = out.boundary.iter().flat_map(|p| p.values.iter().cloned()).collect();
    let lb = format!("{label} {key} faces");
    let b = cmp(&lb, &flat(&bnd), &ref_flat(r, &format!("{key}_b")));
    vec![(l, a), (lb, b)]
}

fn filter_rows(label: &str, r: &Ref, mesh: &Arc<FvMesh>, f: &LesFilter, tag: &str) -> Rows {
    let k: VolScalarField = ref_field(r, mesh, "k", None, true);
    let mag: VolScalarField = ref_field(r, mesh, "in_magSqrU", Some("in_magSqrU_b"), false);
    let u: VolVectorField = ref_field(r, mesh, "U", Some("Ub"), false);
    let sqr_u: VolField<SymmTensor> = ref_field(r, mesh, "in_sqrU", Some("in_sqrU_b"), false);
    let dsym: VolField<SymmTensor> = ref_field(r, mesh, "in_Dsym", Some("in_Dsym_b"), false);
    // `gradU` is extrapolatedCalculated: the filter's `correctBoundaryConditions` re-extrapolates
    // its boundary to the owner-cell value, so THAT is what the real filter sees (the dumped
    // `gradU_b` is the corrected-normal boundary before that call; using it was this test's
    // first-run error, README slice 7).
    let gu: VolField<Tensor> = ref_field(r, mesh, "gradU", None, false);
    let mut rows = Rows::new();
    rows.extend(compare_filtered(label, r, &f.apply(&k), &format!("F_{tag}_k")));
    rows.extend(compare_filtered(label, r, &f.apply(&mag), &format!("F_{tag}_magSqrU")));
    rows.extend(compare_filtered(label, r, &f.apply(&u), &format!("F_{tag}_U")));
    rows.extend(compare_filtered(label, r, &f.apply(&sqr_u), &format!("F_{tag}_sqrU")));
    rows.extend(compare_filtered(label, r, &f.apply(&dsym), &format!("F_{tag}_Dsym")));
    rows.extend(compare_filtered(label, r, &f.apply(&gu), &format!("F_{tag}_gradU")));
    rows
}

#[test]
fn les_filters_vs_openfoam() {
    let r = parse(REF_LESFILTERS);
    let mesh = r.mesh();
    let mut rows = Rows::new();
    rows.extend(filter_rows("simple", &r, &mesh, &LesFilter::Simple, "simple"));
    let lap = LaplaceFilter::new(mesh.clone(), 2.0);
    let c = cmp("laplace coeff (PETIR pow)", lap.coeff(), &r.s("T_laplaceCoeff"));
    rows.push(("laplace coeff (PETIR pow)".to_string(), c));
    rows.extend(filter_rows("laplace (own coeff)", &r, &mesh, &LesFilter::Laplace(lap), "laplace"));
    let lap_up = LaplaceFilter::with_coeff(mesh.clone(), 2.0, r.s("T_laplaceCoeff"));
    rows.extend(filter_rows(
        "laplace (esi coeff)",
        &r,
        &mesh,
        &LesFilter::Laplace(lap_up),
        "laplace",
    ));
    let an = AnisotropicFilter::new(mesh.clone(), 2.0);
    let c = cmp("aniso coeff", &flat(an.coeff()), &ref_flat(&r, "T_anisoCoeff"));
    rows.push(("aniso coeff".to_string(), c));
    rows.extend(filter_rows("aniso", &r, &mesh, &LesFilter::Anisotropic(an), "aniso"));
    all_identical(&rows);
    pin(&rows, PINS_LES_FILTERS);
}

const PINS_LES_FILTERS: &[(&str, usize)] = &[
    ("simple F_simple_k cells", 360),
    ("simple F_simple_k faces", 372),
    ("simple F_simple_magSqrU cells", 360),
    ("simple F_simple_magSqrU faces", 372),
    ("simple F_simple_U cells", 1080),
    ("simple F_simple_U faces", 1116),
    ("simple F_simple_sqrU cells", 2160),
    ("simple F_simple_sqrU faces", 2232),
    ("simple F_simple_Dsym cells", 2160),
    ("simple F_simple_Dsym faces", 2232),
    ("simple F_simple_gradU cells", 3240),
    ("simple F_simple_gradU faces", 3348),
    ("laplace coeff (PETIR pow)", 360),
    ("laplace (own coeff) F_laplace_k cells", 360),
    ("laplace (own coeff) F_laplace_k faces", 372),
    ("laplace (own coeff) F_laplace_magSqrU cells", 360),
    ("laplace (own coeff) F_laplace_magSqrU faces", 372),
    ("laplace (own coeff) F_laplace_U cells", 1080),
    ("laplace (own coeff) F_laplace_U faces", 1116),
    ("laplace (own coeff) F_laplace_sqrU cells", 2160),
    ("laplace (own coeff) F_laplace_sqrU faces", 2232),
    ("laplace (own coeff) F_laplace_Dsym cells", 2160),
    ("laplace (own coeff) F_laplace_Dsym faces", 2232),
    ("laplace (own coeff) F_laplace_gradU cells", 3240),
    ("laplace (own coeff) F_laplace_gradU faces", 3348),
    ("laplace (esi coeff) F_laplace_k cells", 360),
    ("laplace (esi coeff) F_laplace_k faces", 372),
    ("laplace (esi coeff) F_laplace_magSqrU cells", 360),
    ("laplace (esi coeff) F_laplace_magSqrU faces", 372),
    ("laplace (esi coeff) F_laplace_U cells", 1080),
    ("laplace (esi coeff) F_laplace_U faces", 1116),
    ("laplace (esi coeff) F_laplace_sqrU cells", 2160),
    ("laplace (esi coeff) F_laplace_sqrU faces", 2232),
    ("laplace (esi coeff) F_laplace_Dsym cells", 2160),
    ("laplace (esi coeff) F_laplace_Dsym faces", 2232),
    ("laplace (esi coeff) F_laplace_gradU cells", 3240),
    ("laplace (esi coeff) F_laplace_gradU faces", 3348),
    ("aniso coeff", 1080),
    ("aniso F_aniso_k cells", 360),
    ("aniso F_aniso_k faces", 372),
    ("aniso F_aniso_magSqrU cells", 360),
    ("aniso F_aniso_magSqrU faces", 372),
    ("aniso F_aniso_U cells", 1080),
    ("aniso F_aniso_U faces", 1116),
    ("aniso F_aniso_sqrU cells", 2160),
    ("aniso F_aniso_sqrU faces", 2232),
    ("aniso F_aniso_Dsym cells", 2160),
    ("aniso F_aniso_Dsym faces", 2232),
    ("aniso F_aniso_gradU cells", 3240),
    ("aniso F_aniso_gradU faces", 3348),
];

// ── gh:#1544 slice 7: dynamicKEqn terms against the real ESI model ───────────────────

use outram_foam_turbulence_lib::les::dynamic_k_eqn::DynamicKEqn;

const REF_DYN_SIMPLE: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_turb_gh1362/reference/ref_dynamicKEqn_simple.txt.gz"
);
const REF_DYN_LAPLACE: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_turb_gh1362/reference/ref_dynamicKEqn_laplace.txt.gz"
);
const REF_DYN_ANISO: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_turb_gh1362/reference/ref_dynamicKEqn_anisotropic.txt.gz"
);

fn dyn_filter(kind: &str, mesh: &Arc<FvMesh>) -> LesFilter {
    match kind {
        "simple" => LesFilter::Simple,
        "laplace" => LesFilter::Laplace(LaplaceFilter::new(mesh.clone(), 2.0)),
        _ => LesFilter::Anisotropic(AnisotropicFilter::new(mesh.clone(), 2.0)),
    }
}

/// The model on the dumped state: `k`, `nut` (with boundary, as `validate()` left it), `nu`, `U`.
fn dyn_model(r: &Ref, mesh: &Arc<FvMesh>, kind: &str) -> DynamicKEqn {
    let mut m = DynamicKEqn::new(mesh.clone(), dyn_filter(kind, mesh));
    m.k = r.zg(mesh, "k", "k");
    m.nu_t = ref_field(r, mesh, "nutpre", Some("nutpre_b"), false);
    m.nu = ref_field(r, mesh, "nu_f", Some("nu_f_b"), false);
    m.u = ref_field(r, mesh, "U", Some("Ub"), false);
    m
}

fn dyn_state(r: &Ref, own_delta: bool) -> FrozenState {
    FrozenState {
        grad_u: r.t("gradU"),
        div_u: r.s("divU"),
        cell_length: if own_delta { None } else { Some(r.s("T_delta")) },
        ..FrozenState::default()
    }
}

fn dyn_rows(label: &str, gz: &[u8], kind: &str) -> Rows {
    let r = parse(gz);
    check_audits(&r, 4);
    let mesh = r.mesh();
    let m = dyn_model(&r, &mesh, kind);
    let grad_faces: Vec<Tensor> = r.cols["gradU_b"].iter().map(|v| Tensor::make(v)).collect();
    let st = dyn_state(&r, false);
    let t = m.terms(&st, &grad_faces);
    let mut rows = Rows::new();
    rows.extend(compare_filtered(label, &r, &t.d_symm, "T_D_symm"));
    rows.extend(compare_filtered(label, &r, &t.d_dev, "T_D_dev"));
    rows.extend(compare_filtered(label, &r, &t.kk_nut, "T_KK_nut"));
    rows.extend(compare_filtered(label, &r, &t.kk, "T_KK"));
    rows.extend(compare_filtered(label, &r, &t.ll, "T_LL"));
    rows.extend(compare_filtered(label, &r, &t.mm_symm, "T_MM_symm"));
    rows.extend(compare_filtered(label, &r, &t.mm_dev, "T_MM_dev"));
    // the REAL protected Ck and Ce of the model object (cells)
    let cells = |f: &VolScalarField| -> Vec<f64> { f.internal.iter().cloned().collect() };
    for (name, key, vals) in [
        ("Ck_symm vs real Ck", "R_Ck_symm", cells(&t.ck_symm)),
        ("Ck_dev vs real Ck", "R_Ck_dev", cells(&t.ck_dev)),
        ("Ce vs real Ce", "R_Ce", cells(&t.ce)),
        ("Ck_symm vs transcription", "T_Ck_symm", cells(&t.ck_symm)),
        ("Ck_dev vs transcription", "T_Ck_dev", cells(&t.ck_dev)),
        ("Ce vs transcription", "T_Ce", cells(&t.ce)),
        ("nut after validate() vs real nut", "nut_real", t.nut_validate.clone()),
        ("nut after correct() vs real nut", "nut_after", t.nut_after.clone()),
    ] {
        let l = format!("{label} {name}");
        let s = cmp(&l, &vals, &r.s(key));
        rows.push((l, s));
    }
    // the other D: the dev form against the validate()-time nut (prediction 5: they differ)
    let l = format!("{label} nut(devSymm form) vs nut after validate()");
    let s = cmp(&l, &t.nut_after, &r.s("nut_real"));
    rows.push((l, s));
    let terms = m.source_terms(&st, &grad_faces);
    for (term, key) in [("G", "T_G"), ("k_SuSp", "T_k_SuSp"), ("k_Sp", "T_k_Sp")] {
        let l = format!("{label} {term}");
        let s = cmp(&l, terms.get(term).unwrap(), &r.s(key));
        rows.push((l, s));
    }
    check_registered(&r, "reg_k_dynamicKEqn_G", "T_G");
    // prediction 6: the port's own gradient boundary (extrapolated to the owner cell)
    let owner_faces: Vec<Tensor> = mesh
        .patches
        .iter()
        .flat_map(|p| (p.start..p.start + p.size).map(|f| st.grad_u[mesh.owner[f]]))
        .collect();
    let te = m.terms(&st, &owner_faces);
    let l = format!("{label} Ck_dev with EXTRAPOLATED grad(U) boundary vs real Ck");
    let s = cmp(&l, &cells(&te.ck_dev), &r.s("R_Ck_dev"));
    rows.push((l, s));
    // every row is bit-identical except the two negative controls (the wrong `D` and the
    // extrapolated gradient boundary), which are expected to differ and are pinned below
    for (l, st) in &rows {
        if !l.contains("EXTRAPOLATED") && !l.contains("devSymm form") {
            assert_eq!(st.identical, st.n, "{l}");
        }
    }
    rows
}

#[test]
fn dynamic_k_eqn_simple_vs_openfoam() {
    let rows = dyn_rows("dynamicKEqn[simple]", REF_DYN_SIMPLE, "simple");
    pin(&rows, PINS_DYN_SIMPLE);
}

#[test]
fn dynamic_k_eqn_laplace_vs_openfoam() {
    let rows = dyn_rows("dynamicKEqn[laplace]", REF_DYN_LAPLACE, "laplace");
    pin(&rows, PINS_DYN_LAPLACE);
}

#[test]
fn dynamic_k_eqn_anisotropic_vs_openfoam() {
    let rows = dyn_rows("dynamicKEqn[aniso]", REF_DYN_ANISO, "anisotropic");
    pin(&rows, PINS_DYN_ANISO);
}

const PINS_DYN_SIMPLE: &[(&str, usize)] = &[
    ("dynamicKEqn[simple] T_D_symm cells", 2160),
    ("dynamicKEqn[simple] T_D_symm faces", 2232),
    ("dynamicKEqn[simple] T_D_dev cells", 2160),
    ("dynamicKEqn[simple] T_D_dev faces", 2232),
    ("dynamicKEqn[simple] T_KK_nut cells", 360),
    ("dynamicKEqn[simple] T_KK_nut faces", 372),
    ("dynamicKEqn[simple] T_KK cells", 360),
    ("dynamicKEqn[simple] T_KK faces", 372),
    ("dynamicKEqn[simple] T_LL cells", 2160),
    ("dynamicKEqn[simple] T_LL faces", 2232),
    ("dynamicKEqn[simple] T_MM_symm cells", 2160),
    ("dynamicKEqn[simple] T_MM_symm faces", 2232),
    ("dynamicKEqn[simple] T_MM_dev cells", 2160),
    ("dynamicKEqn[simple] T_MM_dev faces", 2232),
    ("dynamicKEqn[simple] Ck_symm vs real Ck", 360),
    ("dynamicKEqn[simple] Ck_dev vs real Ck", 360),
    ("dynamicKEqn[simple] Ce vs real Ce", 360),
    ("dynamicKEqn[simple] Ck_symm vs transcription", 360),
    ("dynamicKEqn[simple] Ck_dev vs transcription", 360),
    ("dynamicKEqn[simple] Ce vs transcription", 360),
    ("dynamicKEqn[simple] nut after validate() vs real nut", 360),
    ("dynamicKEqn[simple] nut after correct() vs real nut", 360),
    ("dynamicKEqn[simple] nut(devSymm form) vs nut after validate()", 280),
    ("dynamicKEqn[simple] G", 360),
    ("dynamicKEqn[simple] k_SuSp", 360),
    ("dynamicKEqn[simple] k_Sp", 360),
    ("dynamicKEqn[simple] Ck_dev with EXTRAPOLATED grad(U) boundary vs real Ck", 280),
];
const PINS_DYN_LAPLACE: &[(&str, usize)] = &[
    ("dynamicKEqn[laplace] T_D_symm cells", 2160),
    ("dynamicKEqn[laplace] T_D_symm faces", 2232),
    ("dynamicKEqn[laplace] T_D_dev cells", 2160),
    ("dynamicKEqn[laplace] T_D_dev faces", 2232),
    ("dynamicKEqn[laplace] T_KK_nut cells", 360),
    ("dynamicKEqn[laplace] T_KK_nut faces", 372),
    ("dynamicKEqn[laplace] T_KK cells", 360),
    ("dynamicKEqn[laplace] T_KK faces", 372),
    ("dynamicKEqn[laplace] T_LL cells", 2160),
    ("dynamicKEqn[laplace] T_LL faces", 2232),
    ("dynamicKEqn[laplace] T_MM_symm cells", 2160),
    ("dynamicKEqn[laplace] T_MM_symm faces", 2232),
    ("dynamicKEqn[laplace] T_MM_dev cells", 2160),
    ("dynamicKEqn[laplace] T_MM_dev faces", 2232),
    ("dynamicKEqn[laplace] Ck_symm vs real Ck", 360),
    ("dynamicKEqn[laplace] Ck_dev vs real Ck", 360),
    ("dynamicKEqn[laplace] Ce vs real Ce", 360),
    ("dynamicKEqn[laplace] Ck_symm vs transcription", 360),
    ("dynamicKEqn[laplace] Ck_dev vs transcription", 360),
    ("dynamicKEqn[laplace] Ce vs transcription", 360),
    ("dynamicKEqn[laplace] nut after validate() vs real nut", 360),
    ("dynamicKEqn[laplace] nut after correct() vs real nut", 360),
    ("dynamicKEqn[laplace] nut(devSymm form) vs nut after validate()", 248),
    ("dynamicKEqn[laplace] G", 360),
    ("dynamicKEqn[laplace] k_SuSp", 360),
    ("dynamicKEqn[laplace] k_Sp", 360),
    ("dynamicKEqn[laplace] Ck_dev with EXTRAPOLATED grad(U) boundary vs real Ck", 360),
];
const PINS_DYN_ANISO: &[(&str, usize)] = &[
    ("dynamicKEqn[aniso] T_D_symm cells", 2160),
    ("dynamicKEqn[aniso] T_D_symm faces", 2232),
    ("dynamicKEqn[aniso] T_D_dev cells", 2160),
    ("dynamicKEqn[aniso] T_D_dev faces", 2232),
    ("dynamicKEqn[aniso] T_KK_nut cells", 360),
    ("dynamicKEqn[aniso] T_KK_nut faces", 372),
    ("dynamicKEqn[aniso] T_KK cells", 360),
    ("dynamicKEqn[aniso] T_KK faces", 372),
    ("dynamicKEqn[aniso] T_LL cells", 2160),
    ("dynamicKEqn[aniso] T_LL faces", 2232),
    ("dynamicKEqn[aniso] T_MM_symm cells", 2160),
    ("dynamicKEqn[aniso] T_MM_symm faces", 2232),
    ("dynamicKEqn[aniso] T_MM_dev cells", 2160),
    ("dynamicKEqn[aniso] T_MM_dev faces", 2232),
    ("dynamicKEqn[aniso] Ck_symm vs real Ck", 360),
    ("dynamicKEqn[aniso] Ck_dev vs real Ck", 360),
    ("dynamicKEqn[aniso] Ce vs real Ce", 360),
    ("dynamicKEqn[aniso] Ck_symm vs transcription", 360),
    ("dynamicKEqn[aniso] Ck_dev vs transcription", 360),
    ("dynamicKEqn[aniso] Ce vs transcription", 360),
    ("dynamicKEqn[aniso] nut after validate() vs real nut", 360),
    ("dynamicKEqn[aniso] nut after correct() vs real nut", 360),
    ("dynamicKEqn[aniso] nut(devSymm form) vs nut after validate()", 203),
    ("dynamicKEqn[aniso] G", 360),
    ("dynamicKEqn[aniso] k_SuSp", 360),
    ("dynamicKEqn[aniso] k_Sp", 360),
    ("dynamicKEqn[aniso] Ck_dev with EXTRAPOLATED grad(U) boundary vs real Ck", 203),
];

// ── gh:#1544 slice 7: dynamicLagrangian terms against the real ESI model ─────────────

use outram_foam_basic_lib::fv_operators::fvc::grad_vec_corrected;
use outram_foam_turbulence_lib::les::dynamic_lagrangian::DynamicLagrangian;

const REF_LAG_SIMPLE: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_turb_gh1362/reference/ref_dynamicLagrangian_simple.txt.gz"
);
const REF_LAG_LAPLACE: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_turb_gh1362/reference/ref_dynamicLagrangian_laplace.txt.gz"
);
const REF_LAG_ANISO: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_turb_gh1362/reference/ref_dynamicLagrangian_anisotropic.txt.gz"
);

fn lag_rows(label: &str, gz: &[u8], kind: &str) -> Rows {
    let r = parse(gz);
    check_audits(&r, 8);
    let mesh = r.mesh();
    let mut m = DynamicLagrangian::new(mesh.clone(), dyn_filter(kind, &mesh));
    m.flm = ref_field(&r, &mesh, "T_flm", None, false);
    m.fmm = ref_field(&r, &mesh, "T_fmm", None, false);
    m.u = ref_field(&r, &mesh, "U", Some("Ub"), false);
    let mut rows = Rows::new();
    // the port's own Gauss gradient WITH upstream's boundary correction, against the real one
    let g = grad_vec_corrected(&m.u);
    rows.extend(compare_filtered(label, &r, &g, "gradU"));
    let t = m.terms(&Some(r.s("T_delta")));
    rows.extend(compare_filtered(label, &r, &t.s, "T_S"));
    rows.extend(compare_filtered(label, &r, &t.mag_s, "T_magS"));
    rows.extend(compare_filtered(label, &r, &t.uf, "T_Uf"));
    rows.extend(compare_filtered(label, &r, &t.sf, "T_Sf"));
    rows.extend(compare_filtered(label, &r, &t.mag_sf, "T_magSf"));
    rows.extend(compare_filtered(label, &r, &t.l, "T_L"));
    rows.extend(compare_filtered(label, &r, &t.m, "T_M"));
    rows.extend(compare_filtered(label, &r, &t.inv_t, "T_invT"));
    rows.extend(compare_filtered(label, &r, &t.lm, "T_LM"));
    rows.extend(compare_filtered(label, &r, &t.mm, "T_MM"));
    for (name, key) in [("nut vs transcription", "T_nut"), ("nut vs real nut after validate()", "nut_real"),
                        ("nut vs real nut after correct()", "nut_after")] {
        let l = format!("{label} {name}");
        let s = cmp(&l, &t.nut, &r.s(key));
        rows.push((l, s));
    }
    // PETIR cbrt against the upstream `pow(V, 1/3)` delta: separate row, libm only
    let own = m.terms(&None);
    let l = format!("{label} nut with the port's own cbrt delta vs real nut");
    let s = cmp(&l, &own.nut, &r.s("nut_after"));
    rows.push((l, s));
    for (l, st) in &rows {
        if !l.contains("own cbrt") {
            assert_eq!(st.identical, st.n, "{l}");
        }
    }
    rows
}

#[test]
fn dynamic_lagrangian_simple_vs_openfoam() {
    let rows = lag_rows("dynamicLagrangian[simple]", REF_LAG_SIMPLE, "simple");
    pin(&rows, PINS_LAG_SIMPLE);
}

#[test]
fn dynamic_lagrangian_laplace_vs_openfoam() {
    let rows = lag_rows("dynamicLagrangian[laplace]", REF_LAG_LAPLACE, "laplace");
    pin(&rows, PINS_LAG_LAPLACE);
}

#[test]
fn dynamic_lagrangian_anisotropic_vs_openfoam() {
    let rows = lag_rows("dynamicLagrangian[aniso]", REF_LAG_ANISO, "anisotropic");
    pin(&rows, PINS_LAG_ANISO);
}

const PINS_LAG_SIMPLE: &[(&str, usize)] = &[
    ("dynamicLagrangian[simple] gradU cells", 3240),
    ("dynamicLagrangian[simple] gradU faces", 3348),
    ("dynamicLagrangian[simple] T_S cells", 2160),
    ("dynamicLagrangian[simple] T_S faces", 2232),
    ("dynamicLagrangian[simple] T_magS cells", 360),
    ("dynamicLagrangian[simple] T_magS faces", 372),
    ("dynamicLagrangian[simple] T_Uf cells", 1080),
    ("dynamicLagrangian[simple] T_Uf faces", 1116),
    ("dynamicLagrangian[simple] T_Sf cells", 2160),
    ("dynamicLagrangian[simple] T_Sf faces", 2232),
    ("dynamicLagrangian[simple] T_magSf cells", 360),
    ("dynamicLagrangian[simple] T_magSf faces", 372),
    ("dynamicLagrangian[simple] T_L cells", 2160),
    ("dynamicLagrangian[simple] T_L faces", 2232),
    ("dynamicLagrangian[simple] T_M cells", 2160),
    ("dynamicLagrangian[simple] T_M faces", 2232),
    ("dynamicLagrangian[simple] T_invT cells", 360),
    ("dynamicLagrangian[simple] T_invT faces", 372),
    ("dynamicLagrangian[simple] T_LM cells", 360),
    ("dynamicLagrangian[simple] T_LM faces", 372),
    ("dynamicLagrangian[simple] T_MM cells", 360),
    ("dynamicLagrangian[simple] T_MM faces", 372),
    ("dynamicLagrangian[simple] nut vs transcription", 360),
    ("dynamicLagrangian[simple] nut vs real nut after validate()", 360),
    ("dynamicLagrangian[simple] nut vs real nut after correct()", 360),
    ("dynamicLagrangian[simple] nut with the port's own cbrt delta vs real nut", 157),
];
const PINS_LAG_LAPLACE: &[(&str, usize)] = &[
    ("dynamicLagrangian[laplace] gradU cells", 3240),
    ("dynamicLagrangian[laplace] gradU faces", 3348),
    ("dynamicLagrangian[laplace] T_S cells", 2160),
    ("dynamicLagrangian[laplace] T_S faces", 2232),
    ("dynamicLagrangian[laplace] T_magS cells", 360),
    ("dynamicLagrangian[laplace] T_magS faces", 372),
    ("dynamicLagrangian[laplace] T_Uf cells", 1080),
    ("dynamicLagrangian[laplace] T_Uf faces", 1116),
    ("dynamicLagrangian[laplace] T_Sf cells", 2160),
    ("dynamicLagrangian[laplace] T_Sf faces", 2232),
    ("dynamicLagrangian[laplace] T_magSf cells", 360),
    ("dynamicLagrangian[laplace] T_magSf faces", 372),
    ("dynamicLagrangian[laplace] T_L cells", 2160),
    ("dynamicLagrangian[laplace] T_L faces", 2232),
    ("dynamicLagrangian[laplace] T_M cells", 2160),
    ("dynamicLagrangian[laplace] T_M faces", 2232),
    ("dynamicLagrangian[laplace] T_invT cells", 360),
    ("dynamicLagrangian[laplace] T_invT faces", 372),
    ("dynamicLagrangian[laplace] T_LM cells", 360),
    ("dynamicLagrangian[laplace] T_LM faces", 372),
    ("dynamicLagrangian[laplace] T_MM cells", 360),
    ("dynamicLagrangian[laplace] T_MM faces", 372),
    ("dynamicLagrangian[laplace] nut vs transcription", 360),
    ("dynamicLagrangian[laplace] nut vs real nut after validate()", 360),
    ("dynamicLagrangian[laplace] nut vs real nut after correct()", 360),
    ("dynamicLagrangian[laplace] nut with the port's own cbrt delta vs real nut", 157),
];
const PINS_LAG_ANISO: &[(&str, usize)] = &[
    ("dynamicLagrangian[aniso] gradU cells", 3240),
    ("dynamicLagrangian[aniso] gradU faces", 3348),
    ("dynamicLagrangian[aniso] T_S cells", 2160),
    ("dynamicLagrangian[aniso] T_S faces", 2232),
    ("dynamicLagrangian[aniso] T_magS cells", 360),
    ("dynamicLagrangian[aniso] T_magS faces", 372),
    ("dynamicLagrangian[aniso] T_Uf cells", 1080),
    ("dynamicLagrangian[aniso] T_Uf faces", 1116),
    ("dynamicLagrangian[aniso] T_Sf cells", 2160),
    ("dynamicLagrangian[aniso] T_Sf faces", 2232),
    ("dynamicLagrangian[aniso] T_magSf cells", 360),
    ("dynamicLagrangian[aniso] T_magSf faces", 372),
    ("dynamicLagrangian[aniso] T_L cells", 2160),
    ("dynamicLagrangian[aniso] T_L faces", 2232),
    ("dynamicLagrangian[aniso] T_M cells", 2160),
    ("dynamicLagrangian[aniso] T_M faces", 2232),
    ("dynamicLagrangian[aniso] T_invT cells", 360),
    ("dynamicLagrangian[aniso] T_invT faces", 372),
    ("dynamicLagrangian[aniso] T_LM cells", 360),
    ("dynamicLagrangian[aniso] T_LM faces", 372),
    ("dynamicLagrangian[aniso] T_MM cells", 360),
    ("dynamicLagrangian[aniso] T_MM faces", 372),
    ("dynamicLagrangian[aniso] nut vs transcription", 360),
    ("dynamicLagrangian[aniso] nut vs real nut after validate()", 360),
    ("dynamicLagrangian[aniso] nut vs real nut after correct()", 360),
    ("dynamicLagrangian[aniso] nut with the port's own cbrt delta vs real nut", 157),
];

// ── kOmegaSSTLM (gh:#1544, README slice 8) ─────────────────────────────────

const REF_SST_LM: &[u8] = include_bytes!(
    "../verification_and_validation/upstream_turb_gh1362/reference/ref_kOmegaSSTLM.txt.gz"
);

/// `kOmegaSSTLM` on the frozen duct with its own transition-relevant fields (README slice 8),
/// ESI `y` and upstream gradients fed in. 16 audit parts (omega, k, ReThetat, gammaInt matrices).
#[test]
fn k_omega_sst_lm_vs_openfoam() {
    let r = parse(REF_SST_LM);
    check_audits(&r, 16);
    let mesh = r.mesh();
    let mut m = KOmegaSstLm::new(mesh.clone());
    m.sst.k = r.zg(&mesh, "k", "k");
    m.sst.omega = r.zg(&mesh, "omega", "omega");
    m.sst.nu = r.zg(&mesh, "nu", "nu");
    m.sst.nu_t = r.zg(&mesh, "nut", "nut_real");
    m.sst.u = r.velocity(&mesh);
    m.sst.phi = r.flux(&mesh);
    m.re_thetat = r.s("ReThetat");
    m.gamma_int = r.s("gammaInt");
    m.gamma_int_eff = r.s("gammaIntEff_in");
    let pairs: Pairs = &[
        ("nut_real", "nut"),
        ("T_S2", "S2"),
        ("T_CDkOmega", "CDkOmega"),
        ("T_F1sst", "F1sst"),
        ("T_F3", "F3"),
        ("T_F1", "F1"),
        ("T_F23", "F23"),
        ("T_GbyNu0", "GbyNu0"),
        ("T_G", "G"),
        ("T_gamma", "gamma"),
        ("T_beta", "beta"),
        ("T_GbyNu", "GbyNu"),
        ("T_PkSst", "PkSst"),
        ("T_Pk", "Pk"),
        ("T_omega_E", "omega_E"),
        ("T_omega_SuSp1", "omega_SuSp1"),
        ("T_omega_Sp", "omega_Sp"),
        ("T_omega_SuSp2", "omega_SuSp2"),
        ("T_k_E", "k_E"),
        ("T_k_SuSp", "k_SuSp"),
        ("T_k_Sp", "k_Sp"),
        ("T_Omega", "Omega"),
        ("T_S", "S"),
        ("T_Us", "Us"),
        ("T_dUsds", "dUsds"),
        ("T_delta", "delta"),
        ("T_Fwake", "Fwake"),
        ("T_Fthetat", "Fthetat"),
        ("T_ReThetac", "ReThetac"),
        ("T_Flength", "Flength"),
        ("T_ReThetat0", "ReThetat0"),
        ("T_lambdaIters", "lambdaIters"),
        ("T_Pthetat", "Pthetat"),
        ("T_ReThetat_E", "ReThetat_E"),
        ("T_Rev", "Rev"),
        ("T_RT", "RT"),
        ("T_Fonset", "Fonset"),
        ("T_Pgamma", "Pgamma"),
        ("T_Fturb", "Fturb"),
        ("T_Egamma", "Egamma"),
        ("T_Freattach", "Freattach"),
        ("T_gammaSep", "gammaSep"),
        ("T_gammaIntEff", "gammaIntEff"),
        ("gammaIntEff_real", "gammaIntEff"),
    ];
    let rows = compare("kOmegaSSTLM (esi y)", &r, &m.source_terms(&r.state(true, false)), pairs);
    assert!(max_ulp(&rows) <= LM_MAX_ULP, "max ulp {}", max_ulp(&rows));
    pin(&rows, PINS_SST_LM);
}

/// PETIR `tanh` against glibc (inherited from the SST blending functions, README slice 2).
const LM_MAX_ULP: u64 = 3;

const PINS_SST_LM: &[(&str, usize)] = &[
    ("kOmegaSSTLM (esi y) nut", 354),
    ("kOmegaSSTLM (esi y) S2", 360),
    ("kOmegaSSTLM (esi y) CDkOmega", 360),
    ("kOmegaSSTLM (esi y) F1sst", 351),
    ("kOmegaSSTLM (esi y) F3", 360),
    ("kOmegaSSTLM (esi y) F1", 360),
    ("kOmegaSSTLM (esi y) F23", 343),
    ("kOmegaSSTLM (esi y) GbyNu0", 360),
    ("kOmegaSSTLM (esi y) G", 354),
    ("kOmegaSSTLM (esi y) gamma", 360),
    ("kOmegaSSTLM (esi y) beta", 360),
    ("kOmegaSSTLM (esi y) GbyNu", 359),
    ("kOmegaSSTLM (esi y) PkSst", 355),
    ("kOmegaSSTLM (esi y) Pk", 355),
    ("kOmegaSSTLM (esi y) omega_E", 359),
    ("kOmegaSSTLM (esi y) omega_SuSp1", 360),
    ("kOmegaSSTLM (esi y) omega_Sp", 360),
    ("kOmegaSSTLM (esi y) omega_SuSp2", 360),
    ("kOmegaSSTLM (esi y) k_E", 355),
    ("kOmegaSSTLM (esi y) k_SuSp", 360),
    ("kOmegaSSTLM (esi y) k_Sp", 360),
    ("kOmegaSSTLM (esi y) Omega", 360),
    ("kOmegaSSTLM (esi y) S", 360),
    ("kOmegaSSTLM (esi y) Us", 360),
    ("kOmegaSSTLM (esi y) dUsds", 360),
    ("kOmegaSSTLM (esi y) delta", 360),
    ("kOmegaSSTLM (esi y) Fwake", 360),
    ("kOmegaSSTLM (esi y) Fthetat", 360),
    ("kOmegaSSTLM (esi y) ReThetac", 360),
    ("kOmegaSSTLM (esi y) Flength", 360),
    ("kOmegaSSTLM (esi y) ReThetat0", 360),
    ("kOmegaSSTLM (esi y) lambdaIters", 360),
    ("kOmegaSSTLM (esi y) Pthetat", 360),
    ("kOmegaSSTLM (esi y) ReThetat_E", 360),
    ("kOmegaSSTLM (esi y) Rev", 360),
    ("kOmegaSSTLM (esi y) RT", 360),
    ("kOmegaSSTLM (esi y) Fonset", 360),
    ("kOmegaSSTLM (esi y) Pgamma", 360),
    ("kOmegaSSTLM (esi y) Fturb", 360),
    ("kOmegaSSTLM (esi y) Egamma", 360),
    ("kOmegaSSTLM (esi y) Freattach", 360),
    ("kOmegaSSTLM (esi y) gammaSep", 360),
    ("kOmegaSSTLM (esi y) gammaIntEff", 360),
    ("kOmegaSSTLM (esi y) gammaIntEff", 360),
];
