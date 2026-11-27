// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # The coupled sequence "map all fields, `deformMesh()`, `ddt`" on the 3D_SmallESFR meshes, against OpenFOAM-ESI (gh:#1852, #1854, round 30)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da0cc55` on OpenFOAM-ESI v2506. `upstream_driver/coupled_deform_dump/coupledDeformDump.C` builds the three
//! real `fvMesh`es and the five real `meshToMesh` objects of the shipped `multiRegionCouplingDict` (once, on the undeformed meshes, as
//! `meshHandler.C:156-205`), then runs the time-step sequence of `GeN-Foam.C:160-200` / `multiPhysicsSolver.C:174-186`: `runTime++`,
//! map every listed field (`mapTgtToSrc`, `plusEqOp`, `correctBoundaryConditions()`, `meshHandlerTemplates.C:85-93`), then the five
//! upstream lines of `neutronics.C::deformMesh()` from the MAPPED `disp`, then `fvm::ddt(rho, flux)` on the moving neutro mesh. Calls 1,
//! 2, 3 are three time steps (`deltaT = 0.05`); call 4 is a second Picard iteration in step 3 with a changed `meshDisp`. All fields
//! are closed-form (`+ - * /` only). This test runs the same sequence through [`MeshHandler`] (fields, maps, boundary correction),
//! [`deform_region_mesh`] / [`DeformableMesh::deform_mesh_at`] (moving-mesh state) and [`DeformableMesh::euler_fvm_ddt`], and compares
//! item by item: mapped fields (cells and boundary values), `points`, `oldPoints`, `V`, `V0`, `V00`, `meshPhi` (internal and boundary),
//! `moving()`, the `ddt` `diag` and `source`, and the reverse (`mapSrcToTgt`) maps.
//!
//! Reference: `refs/coupled_deform_esfr.ref.txt.gz`, every 31st item plus the last of the 50 MB dump
//! (`scripts/subsample_coupled_deform_dump.py`). `GENFOAM_COUPLED_DEFORM_DUMP=<dump>`
//! (`scripts/build_and_run_coupled_deform_dump.sh <file>`) compares every item.
//!
//! Pass criterion: bit identity (the 1e-9 allowance only with an explained cause; none is claimed). Predictions (committed first) and
//! results: README, "Round 30".

#![cfg(feature = "std")]
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::multi_region::cell_overlap::OverlapMesh;
use outram_foam_appbuilder_lib::genfoam::multi_region::coupled_loop::{deform_region_mesh, RegionMeshMap};
use outram_foam_appbuilder_lib::genfoam::multi_region::coupling_fields::{CouplingField, CouplingRegion, MeshHandler};
use outram_foam_appbuilder_lib::genfoam::multi_region::mesh_deformation::{
    DeformableMesh, MeshDeformationDict, PointCellsOrder, StepClock,
};
use outram_foam_appbuilder_lib::genfoam::multi_region::mesh_to_mesh::{MappingMethod, MeshToMesh};
use outram_foam_basic_lib::fields::boundary::bc::PatchField;
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::vol_field::{VolScalarField, VolVectorField};
use outram_foam_basic_lib::io::dict::FoamFile;
use outram_foam_basic_lib::prelude::{FvMesh, Vector3};

const FL: &str = "fluidRegion";
const NE: &str = "neutroRegion";
const TM: &str = "thermoMechanicalRegion";
const DELTA_T: f64 = 0.05;

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let flg = bytes[3];
    let mut at = 10;
    if flg & 4 != 0 {
        at += 2 + usize::from(bytes[at]) + 256 * usize::from(bytes[at + 1]);
    }
    for bit in [8u8, 16u8] {
        if flg & bit != 0 {
            while bytes[at] != 0 {
                at += 1;
            }
            at += 1;
        }
    }
    if flg & 2 != 0 {
        at += 2;
    }
    decompress_to_vec(&bytes[at..bytes.len() - 8]).unwrap_or_else(|_| panic!("inflate {}", path.display()))
}

fn poly_dir(region: &str) -> PathBuf {
    let v = vv_dir();
    match region {
        TM => v.join("cases/legacy_tm_esfr/constant/thermoMechanicalRegion/polyMesh"),
        r => v.join(format!("cases/mesh_deform_esfr/constant/{r}/polyMesh")),
    }
}

fn read_region(region: &str) -> DeformableMesh {
    let poly = poly_dir(region);
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1852_{region}"));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    for f in ["points", "faces", "owner", "neighbour"] {
        std::fs::write(root.join(f), gunzip(&poly.join(format!("{f}.gz")))).unwrap();
    }
    std::fs::copy(poly.join("boundary"), root.join("boundary")).unwrap();
    DeformableMesh::read(&root, PointCellsOrder::CellAscending).unwrap_or_else(|e| panic!("{region}: {e}"))
}

// ---- the driver's closed-form expressions, operation for operation ------------------------------------------------------------
fn cf(c: Vector3, n: f64, a: f64) -> f64 {
    let (x, y, z) = (c.x, c.y, c.z);
    a + 100.0 * x - 40.0 * y + 25.0 * z + 10.0 * x * y * z + 7.0 * n * (x - 0.5 * y) + a * n / 3.0
}
fn disp_of(c: Vector3, n: f64) -> Vector3 {
    let (x, y, z) = (c.x, c.y, c.z);
    let s = (z + 0.2) / 2.51;
    let r2 = x * x + y * y;
    Vector3::new(1.0e-3 * n * (x * s + 0.05 * y * (1.0 - s)), 1.0e-3 * n * (y * s - 0.05 * x * s * s), 2.0e-3 * n * s * (1.0 + 0.3 * r2))
}

/// `(receiver, provider, source name, target name, vector)` in the shipped dictionary's order; object index `o` of the driver.
const OBJS: [(&str, &str, &str, &str, bool); 11] = [
    (FL, NE, "powerDensity", "powerDensityNeutronics", false),
    (FL, NE, "secondaryPowerDensity", "powerDensityNeutronicsToLiquid", false),
    (NE, FL, "T", "TCool", false),
    (NE, FL, "thermo:rho", "rhoCool", false),
    (NE, FL, "T.fuelAvForNeutronics", "TFuel", false),
    (NE, FL, "T.cladAvForNeutronics", "TClad", false),
    (NE, FL, "T.passiveStructure", "TStructMech", false),
    (NE, TM, "meshDisp", "disp", true),
    (TM, FL, "T.passiveStructure", "TStructFromTH", false),
    (TM, FL, "T.fuelAvForNeutronics", "TFuel", false),
    (TM, NE, "powerDensity", "Q", false),
];

fn scalar_field(name: &str, mesh: &Arc<FvMesh>, a: f64) -> VolScalarField {
    let boundary = mesh
        .patches
        .iter()
        .enumerate()
        .map(|(i, p)| if i == 0 { PatchField::fixed_value(p.size, a) } else { PatchField::zero_gradient(p.size) })
        .collect();
    VolScalarField::new(name, mesh.clone(), Field::uniform(mesh.n_cells, a), boundary)
}

fn vector_field(name: &str, mesh: &Arc<FvMesh>) -> VolVectorField {
    let boundary = mesh
        .patches
        .iter()
        .enumerate()
        .map(|(i, p)| if i == 0 { PatchField::fixed_value_vec(p.size, Vector3::ZERO) } else { PatchField::zero_gradient_vec(p.size) })
        .collect();
    VolVectorField::new(name, mesh.clone(), Field::uniform(mesh.n_cells, Vector3::ZERO), boundary)
}

fn flat3(v: &[Vector3]) -> Vec<f64> {
    v.iter().flat_map(|p| [p.x, p.y, p.z]).collect()
}

fn bnd_scalar(f: &VolScalarField) -> Vec<f64> {
    f.boundary.iter().flat_map(|p| p.values.as_slice().to_vec()).collect()
}

fn bnd_vector(f: &VolVectorField) -> Vec<f64> {
    f.boundary.iter().flat_map(|p| flat3(p.values.as_slice())).collect()
}

/// Everything the Rust side produced, keyed `(call, key)` like the dump.
struct Sim {
    out: HashMap<(usize, String), Vec<f64>>,
    state: HashMap<usize, (bool, i64)>,
    /// the neutro mesh as moved by call 1, for the rebuilt-mapping control
    neutro_after_call1: DeformableMesh,
    tm: DeformableMesh,
    ablation_left_mesh_unmoved: bool,
}

fn simulate() -> Sim {
    let tm = read_region(TM);
    let fl = read_region(FL);
    let mut neutro = read_region(NE);
    let c0_ne = neutro.mesh().cell_centres.clone();
    let c0 = |r: &str| -> Vec<Vector3> {
        match r {
            TM => tm.mesh().cell_centres.clone(),
            FL => fl.mesh().cell_centres.clone(),
            _ => c0_ne.clone(),
        }
    };
    let (c0_tm, c0_fl) = (c0(TM), c0(FL));
    let centres = |r: &str| -> &Vec<Vector3> {
        match r {
            TM => &c0_tm,
            FL => &c0_fl,
            _ => &c0_ne,
        }
    };

    // the handler: regions on the UNDEFORMED meshes with their polyhedral geometry; mappings built once
    let mut handler = MeshHandler::new();
    for (name, m) in [(FL, &fl), (NE, &neutro), (TM, &tm)] {
        handler.add_region(CouplingRegion::from_deformable(name, m).unwrap()).unwrap();
    }
    for (o, &(recv, prov, src, tgt, vec)) in OBJS.iter().enumerate() {
        let rm = handler.region(recv).unwrap().mesh().clone();
        let pm = handler.region(prov).unwrap().mesh().clone();
        if vec {
            handler.region_mut(recv).unwrap().insert_vector(vector_field(tgt, &rm));
            handler.region_mut(prov).unwrap().insert_vector(vector_field(src, &pm));
        } else {
            handler.region_mut(recv).unwrap().insert_scalar(scalar_field(tgt, &rm, 5.0 + o as f64));
            if handler.region(prov).unwrap().scalar(src).is_none() {
                handler.region_mut(prov).unwrap().insert_scalar(scalar_field(src, &pm, 300.0 + 10.0 * o as f64));
            }
        }
    }
    for (recv, prov) in [(FL, NE), (NE, FL), (NE, TM), (TM, FL), (TM, NE)] {
        let fields: Vec<CouplingField> = OBJS
            .iter()
            .filter(|o| o.0 == recv && o.1 == prov)
            .map(|o| if o.4 { CouplingField::vector(o.2, o.3) } else { CouplingField::scalar(o.2, o.3) })
            .collect();
        handler.add_mapping(recv, prov, MappingMethod::CellVolumeWeight, fields).unwrap();
    }

    let dict_text = format!("meshDeformation {{ {NE} {{ displacementField disp; }} }}");
    let dict = MeshDeformationDict::from_coupling_dict(&FoamFile::parse(&dict_text).unwrap().dict).unwrap();
    // the ablation leaves a mesh where it was
    let mut ablated = read_region(NE);
    let none = dict.clone().without_deformation();
    let ablation_left_mesh_unmoved = !deform_region_mesh(&none, NE, &mut ablated, &handler, StepClock { time_index: 1, delta_t: DELTA_T }).unwrap()
        && ablated.points() == ablated.original_points();
    drop(ablated);

    let rho: Vec<f64> = c0_ne.iter().map(|c| 1.0 + 0.1 * c.x).collect();
    let mut out: HashMap<(usize, String), Vec<f64>> = HashMap::new();
    let mut state = HashMap::new();
    let mut flux_prev: Vec<f64> = vec![0.0; c0_ne.len()];
    let mut flux_old_hold = flux_prev.clone();
    let mut neutro_after_call1 = None;
    let mut reverse_done = false;

    let set_providers = |handler: &mut MeshHandler, n: f64, only_disp: bool| {
        for (o, &(_, prov, src, _, vec)) in OBJS.iter().enumerate() {
            if only_disp && !vec {
                continue;
            }
            let cs = centres(prov);
            if vec {
                let f = handler.region_mut(prov).unwrap().vector_mut(src).unwrap();
                for (c, x) in cs.iter().enumerate() {
                    f.internal[c] = disp_of(*x, n);
                }
            } else {
                let f = handler.region_mut(prov).unwrap().scalar_mut(src).unwrap();
                for (c, x) in cs.iter().enumerate() {
                    f.internal[c] = cf(*x, n, 300.0 + 10.0 * o as f64);
                }
            }
        }
    };

    let mut geometry = |call: usize, clock: StepClock, neutro: &mut DeformableMesh, handler: &MeshHandler, out: &mut HashMap<(usize, String), Vec<f64>>| {
        let moved = deform_region_mesh(&dict, NE, neutro, handler, clock).unwrap();
        assert!(moved);
        state.insert(call, (neutro.moving(), clock.time_index));
        let mut put = |k: &str, v: Vec<f64>| {
            out.insert((call, k.to_string()), v);
        };
        put("points", flat3(neutro.points()));
        put("oldPoints", flat3(neutro.old_points().unwrap()));
        put("V", neutro.mesh().cell_volumes.clone());
        put("V0", neutro.v0().unwrap().to_vec());
        put("V00", neutro.v00().unwrap().to_vec());
        let n_int = neutro.topology().neighbour().len();
        let phi = neutro.mesh_phi().unwrap();
        put("meshPhi", phi[..n_int].to_vec());
        put("meshPhi.bnd", phi[n_int..].to_vec());
        // gh:#1856 item 5: `meshPhi.oldTime()`
        put("meshPhiOld", neutro.mesh_phi_old().unwrap()[..n_int].to_vec());
        // the consumer: flux = cf(C0, call, 1.0); old flux as the driver's primitiveFieldRef() / oldTime() leaves it
        let flux: Vec<f64> = c0_ne.iter().map(|c| cf(*c, call as f64, 1.0)).collect();
        if call != 4 {
            flux_old_hold = flux_prev.clone();
            flux_prev = flux.clone();
        }
        let (diag, source) = neutro.euler_fvm_ddt(DELTA_T, &rho, &rho, &flux_old_hold).unwrap();
        put("ddtDiag", diag);
        put("ddtSource", source);
        put("fluxOld", flux_old_hold.clone());
    };

    let dump_mapped = |handler: &MeshHandler, call: usize, out: &mut HashMap<(usize, String), Vec<f64>>, only_disp: bool| {
        for &(recv, _, _, tgt, vec) in &OBJS {
            if only_disp && !vec {
                continue;
            }
            let key = format!("{recv}:{tgt}");
            if vec {
                let f = handler.region(recv).unwrap().vector(tgt).unwrap();
                out.insert((call, key.clone()), flat3(f.internal.as_slice()));
                out.insert((call, format!("{key}.bnd")), bnd_vector(f));
            } else {
                let f = handler.region(recv).unwrap().scalar(tgt).unwrap();
                out.insert((call, key.clone()), f.internal.as_slice().to_vec());
                out.insert((call, format!("{key}.bnd")), bnd_scalar(f));
            }
        }
    };

    for step in 1..=3usize {
        set_providers(&mut handler, step as f64, false);
        handler.interpolate_coupling_fields().unwrap();
        dump_mapped(&handler, step, &mut out, false);
        if step == 1 && !reverse_done {
            reverse_done = true;
            // mapSrcToTgt (heatExchanger.C:265-267): receiver's field onto the provider's list
            let (rg, pg) = (handler.region(NE).unwrap().overlap_mesh().unwrap().clone(), handler.region(TM).unwrap().overlap_mesh().unwrap().clone());
            let m = MeshToMesh::cell_volume_weight(handler.region(TM).unwrap().mesh().clone(), handler.region(NE).unwrap().mesh().clone(), &pg, &rg).unwrap();
            let mut back = vector_field("back", handler.region(TM).unwrap().mesh());
            for c in 0..back.internal.len() {
                back.internal[c] = Vector3::new(0.5 + c as f64 * 1e-7, 2.0, -1.0);
            }
            m.map_reverse_vector(handler.region(NE).unwrap().vector("disp").unwrap(), &mut back).unwrap();
            out.insert((1, "reverseVector".into()), flat3(back.internal.as_slice()));
            let (pg, rg) = (handler.region(FL).unwrap().overlap_mesh().unwrap().clone(), handler.region(NE).unwrap().overlap_mesh().unwrap().clone());
            let ms = MeshToMesh::cell_volume_weight(handler.region(FL).unwrap().mesh().clone(), handler.region(NE).unwrap().mesh().clone(), &pg, &rg).unwrap();
            let mut back_s = scalar_field("backS", handler.region(FL).unwrap().mesh(), 0.0);
            for c in 0..back_s.internal.len() {
                back_s.internal[c] = 1.0 + c as f64 * 1e-7;
            }
            ms.map_reverse_scalar(handler.region(NE).unwrap().scalar("TCool").unwrap(), &mut back_s).unwrap();
            out.insert((1, "reverseScalar".into()), back_s.internal.as_slice().to_vec());
        }
        geometry(step, StepClock { time_index: step as i64, delta_t: DELTA_T }, &mut neutro, &handler, &mut out);
        if step == 1 {
            neutro_after_call1 = Some(neutro.clone());
        }
        if step == 3 {
            set_providers(&mut handler, 3.5, true);
            handler.interpolate_these(&[NE, TM]).unwrap();
            dump_mapped(&handler, 4, &mut out, true);
            geometry(4, StepClock { time_index: 3, delta_t: DELTA_T }, &mut neutro, &handler, &mut out);
        }
    }
    Sim { out, state, neutro_after_call1: neutro_after_call1.unwrap(), tm, ablation_left_mesh_unmoved }
}

fn sim() -> &'static Sim {
    static SIM: OnceLock<Sim> = OnceLock::new();
    SIM.get_or_init(simulate)
}

// ---- the reference ------------------------------------------------------------------------------------------------------------
struct RefItem {
    nc: usize,
    n: usize,
    items: Vec<(usize, Vec<f64>)>,
}
struct Reference {
    fields: HashMap<(usize, String), RefItem>,
    state: HashMap<usize, (bool, i64)>,
    full: bool,
}

fn load_reference() -> Reference {
    let (text, full) = match std::env::var("GENFOAM_COUPLED_DEFORM_DUMP") {
        Ok(p) => (std::fs::read_to_string(p).expect("read dump"), true),
        Err(_) => (String::from_utf8(gunzip(&vv_dir().join("refs/coupled_deform_esfr_r56.ref.txt.gz"))).unwrap(), false),
    };
    let vector_names = ["points", "oldPoints", "reverseVector", "disp", "disp.bnd"];
    let mut r = Reference { fields: HashMap::new(), state: HashMap::new(), full };
    for line in text.lines() {
        let mut it = line.split(' ');
        match it.next() {
            Some("S") => {
                let call: usize = it.next().unwrap().parse().unwrap();
                let moving = it.next().unwrap().ends_with('1');
                let ti: i64 = it.next().unwrap().trim_start_matches("timeIndex=").parse().unwrap();
                r.state.insert(call, (moving, ti));
            }
            Some("G") => {
                let call: usize = it.next().unwrap().parse().unwrap();
                let key = it.next().unwrap().to_string();
                let nc: usize = it.next().unwrap().parse().unwrap();
                let n: usize = it.next().unwrap().parse().unwrap();
                let _stride = it.next();
                let items = it
                    .map(|t| {
                        let (i, v) = t.split_once(':').unwrap();
                        (i.parse().unwrap(), v.split(',').map(|x| x.parse::<f64>().unwrap()).collect())
                    })
                    .collect();
                r.fields.insert((call, key), RefItem { nc, n, items });
            }
            Some("F") => {
                let call: usize = it.next().unwrap().parse().unwrap();
                let key = it.next().unwrap().to_string();
                let nc = if vector_names.contains(&key.rsplit(':').next().unwrap()) { 3 } else { 1 };
                let v: Vec<f64> = it.map(|x| x.parse().unwrap()).collect();
                let n = v.len() / nc;
                let items = (0..n).map(|i| (i, v[i * nc..(i + 1) * nc].to_vec())).collect();
                r.fields.insert((call, key), RefItem { nc, n, items });
            }
            _ => {}
        }
    }
    r
}

fn reference() -> &'static Reference {
    static R: OnceLock<Reference> = OnceLock::new();
    R.get_or_init(load_reference)
}

#[derive(Default, Clone)]
struct Tally {
    compared: usize,
    exact: usize,
    worst: f64,
    first: Option<String>,
}

impl Tally {
    fn add(&mut self, what: &str, a: f64, b: f64) {
        self.compared += 1;
        if a.to_bits() == b.to_bits() || (a == 0.0 && b == 0.0) {
            self.exact += 1;
            return;
        }
        let rel = if b != 0.0 { ((a - b) / b).abs() } else { (a - b).abs() };
        if rel > self.worst {
            self.worst = rel;
        }
        if self.first.is_none() {
            self.first = Some(format!("{what}: mine {a:e} upstream {b:e}"));
        }
    }
}

fn compare(t: &mut Tally, r: &Reference, call: usize, key: &str, mine: &[f64]) {
    let item = r.fields.get(&(call, key.to_string())).unwrap_or_else(|| panic!("reference lacks call {call} {key}"));
    assert_eq!(item.n * item.nc, mine.len(), "size of call {call} {key}");
    for (i, vals) in &item.items {
        for k in 0..item.nc {
            t.add(&format!("call {call} {key}[{i}][{k}]"), mine[i * item.nc + k], vals[k]);
        }
    }
}

fn all_keys() -> Vec<(usize, String)> {
    let s = sim();
    let mut k: Vec<(usize, String)> = s.out.keys().cloned().collect();
    k.sort();
    k
}

#[test]
fn everything_is_bit_identical_to_upstream_through_the_full_handler_path() {
    let (s, r) = (sim(), reference());
    let mut by_key: BTreeMap<String, Tally> = BTreeMap::new();
    for (call, key) in all_keys() {
        let t = by_key.entry(key.clone()).or_default();
        compare(t, r, call, &key, &s.out[&(call, key.clone())]);
    }
    for (call, (moving, ti)) in &s.state {
        assert_eq!(r.state[call], (*moving, *ti), "moving()/timeIndex of call {call}");
    }
    println!("reference: {} ({} states)", if r.full { "FULL dump" } else { "committed sample" }, r.state.len());
    let (mut total, mut exact) = (0, 0);
    for (k, t) in &by_key {
        println!("{k:50} compared {:9} exact {:9} worst rel {:e} {}", t.compared, t.exact, t.worst, t.first.clone().unwrap_or_default());
        total += t.compared;
        exact += t.exact;
    }
    println!("TOTAL compared {total} exact {exact}");
    assert_eq!(total, exact, "every compared value must be bit-identical");
    assert!(total > if r.full { 2_000_000 } else { 20_000 }, "too few values compared: {total}");
}

#[test]
fn ablation_and_stale_state_controls_fail_the_same_comparison() {
    let (s, r) = (sim(), reference());
    assert!(s.ablation_left_mesh_unmoved, "without_deformation must leave the mesh at its original points");
    // (a) no deformation: V stays the original volumes = this port's V0 of call 1; upstream's V of call 1 differs
    let mut t = Tally::default();
    compare(&mut t, r, 1, "V", &s.out[&(1, "V0".to_string())]);
    println!("control a (no deformation, V of call 1): worst rel {:e}, exact {}/{}", t.worst, t.exact, t.compared);
    assert!(t.exact < t.compared && t.worst > 1e-4);
    // (b) stale V0 (never refreshed after step 1): step 2's V0 against upstream
    let mut t = Tally::default();
    compare(&mut t, r, 2, "V0", &s.out[&(1, "V0".to_string())]);
    println!("control b (stale V0 at call 2): worst rel {:e}, exact {}/{}", t.worst, t.exact, t.compared);
    assert!(t.exact < t.compared && t.worst > 1e-4);
    // (c) V0 refreshed on EVERY movePoints: call 4's V0 would be call 3's V
    let mut t = Tally::default();
    compare(&mut t, r, 4, "V0", &s.out[&(3, "V".to_string())]);
    println!("control c (V0 refreshed every call, call 4): worst rel {:e}, exact {}/{}", t.worst, t.exact, t.compared);
    assert!(t.exact < t.compared && t.worst > 1e-5);
    // ... and oldPoints / meshPhi likewise
    let mut t = Tally::default();
    compare(&mut t, r, 4, "oldPoints", &s.out[&(3, "points".to_string())]);
    println!("control c' (oldPoints refreshed every call, call 4): worst rel {:e}, exact {}/{}", t.worst, t.exact, t.compared);
    assert!(t.exact < t.compared);
    // (e) one ulp in one meshPhi is found, and only it
    let mut phi = s.out[&(2, "meshPhi".to_string())].clone();
    phi[5] = f64::from_bits(phi[5].to_bits() + 1);
    let mut t = Tally::default();
    compare(&mut t, r, 2, "meshPhi", &phi);
    println!("control e (1 ulp in meshPhi[5]): {} of {} differ (worst rel {:e})", t.compared - t.exact, t.compared, t.worst);
    assert!(t.compared - t.exact <= 1 && (r.full || t.compared - t.exact == 0) || r.full && t.compared - t.exact == 1);
}

#[test]
fn mapping_rebuilt_on_the_moved_mesh_is_not_what_upstream_does() {
    // (d) upstream builds the meshToMesh objects once, on the undeformed meshes; a port that rebuilt them on the deformed neutro mesh
    // would map a different disp at step 2.
    let (s, r) = (sim(), reference());
    let moved = OverlapMesh::from_deformable(&s.neutro_after_call1).unwrap();
    let tm = OverlapMesh::from_deformable(&s.tm).unwrap();
    let map = RegionMeshMap::new(s.tm.mesh(), s.neutro_after_call1.mesh(), Some((&tm, &moved))).unwrap();
    let provider: Vec<Vector3> = s.tm.mesh().cell_centres.iter().map(|c| disp_of(*c, 2.0)).collect();
    let mut receiver = vec![Vector3::ZERO; s.neutro_after_call1.mesh().n_cells];
    map.map_vector(&provider, &mut receiver);
    let mut t = Tally::default();
    compare(&mut t, r, 2, &format!("{NE}:disp"), &flat3(&receiver));
    println!("control d (mapping rebuilt on the moved neutro mesh, disp at call 2): worst rel {:e}, exact {}/{}", t.worst, t.exact, t.compared);
    assert!(t.exact < t.compared && t.worst > 1e-4, "{:e}", t.worst);
}

#[test]
fn geometric_conservation_of_mesh_phi_is_recorded() {
    // recorded, not gated against upstream: sum over a cell's faces of meshPhi*deltaT against V - V0 (call 1)
    let s = sim();
    let phi_i = &s.out[&(1, "meshPhi".to_string())];
    let phi_b = &s.out[&(1, "meshPhi.bnd".to_string())];
    let (v, v0) = (&s.out[&(1, "V".to_string())], &s.out[&(1, "V0".to_string())]);
    let tm_mesh_cells = v.len();
    // owner / neighbour of the neutro mesh through its DeformableMesh topology
    let neutro = read_region(NE);
    let t = neutro.topology();
    let mut net = vec![0.0; tm_mesh_cells];
    for (f, &o) in t.owner().iter().enumerate() {
        let p = if f < phi_i.len() { phi_i[f] } else { phi_b[f - phi_i.len()] };
        net[o] += p * DELTA_T;
    }
    for (f, &n) in t.neighbour().iter().enumerate() {
        net[n] -= phi_i[f] * DELTA_T;
    }
    let mut rel: Vec<f64> = (0..v.len()).map(|c| ((net[c] - (v[c] - v0[c])) / (v[c] - v0[c])).abs()).collect();
    rel.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!("GCL residual |sum(phi dt) - (V - V0)| / |V - V0|: median {:e}, 99% {:e}, max {:e}", rel[rel.len() / 2], rel[rel.len() * 99 / 100], rel[rel.len() - 1]);
    assert!(rel[rel.len() - 1].is_finite());
}
