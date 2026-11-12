//! `RhoPimpleFoamEsi` against upstream OpenFOAM-v2506 `rhoPimpleFoam` on a 1-D
//! h-form tube, per cell (gh:#1363, row 3a).
//!
//! # Method
//! `verification_and_validation/upstream_openfoam_gh1363/rhopimple` holds the
//! case (the sonicFoam laminar shockTube mesh and states, 100 cells, `mu = 0`,
//! `sensibleEnthalpy`, fixed dt = 1e-5 s, 700 steps), the blockMesh polyMesh the
//! port reads unchanged, and `shock/reference/rhopimple_v2506_shockTube.txt.gz`:
//! the IEEE-754 bit patterns written by `driver/rhoPimpleDump.C`
//! (`rhoPimpleFoam.C` and its `.H` files on the unmodified ESI libraries, see
//! `verification_and_validation/rho_pimple_foam_vs_openfoam.md` for the
//! substitutions): geometry, rho, Ux, T, p, h, psi and the face flux at steps
//! 1-5, 10, 100, 700, the continuity errors of every pressure corrector, and the
//! linear-solver log of the first ten steps.
//!
//! # Results
//! See `verification_and_validation/rho_pimple_foam_vs_openfoam.md`.

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::io::field_reader::{read_vol_scalar_field_full, read_vol_vector_field_full};
use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_appbuilder_lib::solvers::rho_central_foam::PerfectGas;
use outram_foam_appbuilder_lib::solvers::rho_pimple_foam::esi::RhoPimpleFoamEsi;
use std::collections::BTreeMap;
use std::path::PathBuf;

const DT: f64 = 1e-5;
const N_STEPS: usize = 700;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/upstream_openfoam_gh1363")
}

type Fields = BTreeMap<String, Vec<f64>>;

/// Upstream output: geometry, per-step end fields, continuity errors per step,
/// solver log lines per step.
struct Reference {
    geom: Fields,
    steps: BTreeMap<usize, Fields>,
    cont: BTreeMap<usize, Vec<(String, f64)>>,
    solves: BTreeMap<usize, Vec<(String, f64, f64, usize)>>,
}

fn read_reference() -> Reference {
    let bytes = std::fs::read(root().join("shock/reference/rhopimple_v2506_shockTube.txt.gz")).expect("reference");
    let text = String::from_utf8(decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")).expect("utf8");
    let mut r = Reference { geom: Fields::new(), steps: BTreeMap::new(), cont: BTreeMap::new(), solves: BTreeMap::new() };
    for line in text.lines() {
        let tok: Vec<&str> = line.split_whitespace().collect();
        if line.starts_with("# STEP") || line.starts_with("# GEOMETRY") {
            continue;
        }
        let bits = |h: &str| f64::from_bits(u64::from_str_radix(h, 16).unwrap());
        match tok[0] {
            "S" => {
                let (step, tag, name, hex) = (tok[1].parse::<usize>().unwrap(), tok[2], tok[3], tok[5]);
                let v = bits(hex);
                if tag == "geom" {
                    r.geom.entry(name.to_string()).or_default().push(v);
                } else {
                    r.steps.entry(step).or_default().entry(name.to_string()).or_default().push(v);
                }
            }
            "C" => {
                r.cont.entry(tok[1].parse().unwrap()).or_default().push((tok[2].to_string(), bits(tok[3])));
            }
            "L" => {
                // L <step> <field> <initial bits> <final bits> <iterations>
                let step: usize = tok[1].parse().unwrap();
                r.solves.entry(step).or_default().push((
                    tok[2].to_string(),
                    bits(tok[3]),
                    bits(tok[4]),
                    tok[5].parse().unwrap(),
                ));
            }
            _ => {}
        }
    }
    r
}

#[derive(Debug, Clone, Copy)]
struct Cmp {
    identical: usize,
    worst_vs_max: f64,
    worst_cell_rel: f64,
}

fn compare(port: &[f64], up: &[f64]) -> Cmp {
    assert_eq!(port.len(), up.len(), "length");
    let max = up.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let mut c = Cmp { identical: 0, worst_vs_max: 0.0, worst_cell_rel: 0.0 };
    for (a, b) in port.iter().zip(up) {
        if a.to_bits() == b.to_bits() {
            c.identical += 1;
        }
        let d = (a - b).abs();
        if max > 0.0 {
            c.worst_vs_max = c.worst_vs_max.max(d / max);
        }
        if b.abs() > 1e-6 * max {
            c.worst_cell_rel = c.worst_cell_rel.max(d / b.abs());
        }
    }
    c
}

fn build_port() -> RhoPimpleFoamEsi {
    let case = root().join("rhopimple");
    let mesh = read_poly_mesh(&case.join("constant/polyMesh")).expect("polyMesh");
    let p0 = read_vol_scalar_field_full(&case.join("0/p"), &mesh).expect("0/p");
    let t0 = read_vol_scalar_field_full(&case.join("0/T"), &mesh).expect("0/T");
    let u0 = read_vol_vector_field_full(&case.join("0/U"), &mesh).expect("0/U");
    // thermophysicalProperties: molWeight 28.9, Cp 1005 (hConst, perfectGas), mu 0
    let gas = PerfectGas::from_molar_weight_and_cp(28.9, 1005.0);
    let p: Vec<f64> = p0.internal.iter().copied().collect();
    let t: Vec<f64> = t0.internal.iter().copied().collect();
    let ux: Vec<f64> = u0.internal.iter().map(|v| v.x).collect();
    RhoPimpleFoamEsi::new(&mesh, gas, 0.0, DT, &p, &t, &ux).expect("port")
}

#[test]
fn shock_tube_matches_upstream_rhopimplefoam() {
    let reference = read_reference();
    let mut s = build_port();

    // geometry the port derives
    let topo = s.topo().clone();
    let mesh = read_poly_mesh(&root().join("rhopimple/constant/polyMesh")).unwrap();
    let v: Vec<f64> = mesh.cell_volumes.to_vec();
    for (name, port_vals) in [
        ("V", v.as_slice()),
        ("Sfx", topo.sf_x()),
        ("magSf", topo.mag_sf()),
        ("w", topo.weights()),
        ("deltaCoeffs", topo.delta_coeffs()),
    ] {
        let c = compare(port_vals, &reference.geom[name]);
        eprintln!("geometry {name}: {c:?}");
        assert_eq!(c.identical, port_vals.len(), "geometry {name}: {c:?}");
    }

    let mut worst: BTreeMap<String, (usize, usize, f64)> = BTreeMap::new();
    let mut solve_mismatch = 0usize;
    let mut cont_mismatch = 0usize;
    for step in 1..=N_STEPS {
        let log = match s.step() {
            Ok(l) => l,
            Err(e) => panic!("step {step}: {e}; end-cell Ux {:e} {:e}", s.ux[0], s.ux[s.ux.len() - 1]),
        };
        if let Some(up) = reference.cont.get(&step) {
            for (i, (name, v)) in up.iter().enumerate() {
                let pair = log.continuity[i / 2];
                let got = if name == "sumLocal" { pair.0 } else { pair.1 };
                if got.to_bits() != v.to_bits() {
                    cont_mismatch += 1;
                }
            }
        }
        if let Some(up) = reference.solves.get(&step) {
            for (i, (name, init, fin, it)) in up.iter().enumerate() {
                let (pn, pl) = &log.solves[i];
                let same = *pn == name.as_str()
                    && pl.initial.to_bits() == init.to_bits()
                    && pl.final_.to_bits() == fin.to_bits()
                    && pl.iterations == *it;
                if !same {
                    solve_mismatch += 1;
                    eprintln!("step {step} solve {i}: port {pn} {pl:?} vs upstream {name} {init:e} {fin:e} {it}");
                }
            }
        }
        if let Some(up) = reference.steps.get(&step) {
            let phi_up = &up["phi"];
            let fields: [(&str, Vec<f64>); 7] = [
                ("rho", s.rho.clone()),
                ("Ux", s.ux.clone()),
                ("T", s.t.clone()),
                ("p", s.p.clone()),
                ("h", s.h.clone()),
                ("psi", s.psi.clone()),
                ("phi", s.phi.clone()),
            ];
            for (name, vals) in fields {
                // upstream names the compressibility field "thermo:psi"
                let key = if name == "psi" { "thermo:psi" } else { name };
                let c = compare(&vals, if name == "phi" { phi_up } else { &up[key] });
                eprintln!("step {step} {name}: {c:?} of {}", vals.len());
                let e = worst.entry(name.to_string()).or_insert((0, 0, 0.0));
                e.0 += c.identical;
                e.1 += vals.len();
                e.2 = e.2.max(c.worst_vs_max);
            }
        }
    }
    eprintln!("per-field (identical, total, worst vs max): {worst:?}");
    eprintln!("solver-log mismatches {solve_mismatch}, continuity mismatches {cont_mismatch}");
    for (name, (ident, total, w)) in &worst {
        assert_eq!(ident, total, "{name}: {ident}/{total} bit-identical, worst {w:e}");
    }
    assert_eq!(solve_mismatch, 0);
    assert_eq!(cont_mismatch, 0);
}

/// Diagnostic, not a gate: first step (up to `RHOPIMPLE_PROBE_STEPS`, default 120)
/// at which any dumped end-of-step field differs from the driver log written
/// with `dumpSteps` covering every step (see `scripts/run_rhopimple.sh`, `NOREF=1`).
#[test]
fn first_divergent_step_against_upstream_driver_log() {
    let path = std::env::var("RHOPIMPLE_STAGE_LOG")
        .unwrap_or_else(|_| "/home/user/upstream-build/gh1363/rhopimple/rhopimple.log".to_string());
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("no stage log at {path}; skipped");
        return;
    };
    let last: usize = std::env::var("RHOPIMPLE_PROBE_STEPS").ok().and_then(|v| v.parse().ok()).unwrap_or(120);
    let mut ends: BTreeMap<(usize, String), Vec<f64>> = BTreeMap::new();
    for line in text.lines() {
        let tok: Vec<&str> = line.split_whitespace().collect();
        if tok.len() == 6 && tok[0] == "S" && tok[2] == "end" {
            let v = f64::from_bits(u64::from_str_radix(tok[5], 16).unwrap());
            ends.entry((tok[1].parse().unwrap(), tok[3].to_string())).or_default().push(v);
        }
    }
    if !ends.contains_key(&(last, "p".to_string())) {
        eprintln!("log has no dump at step {last}; skipped");
        return;
    }
    let mut s = build_port();
    for step in 1..=last {
        s.step().expect("step");
        let fields: [(&str, Vec<f64>); 7] = [
            ("rho", s.rho.clone()),
            ("Ux", s.ux.clone()),
            ("T", s.t.clone()),
            ("p", s.p.clone()),
            ("h", s.h.clone()),
            ("thermo:psi", s.psi.clone()),
            ("phi", s.phi.clone()),
        ];
        let mut bad = Vec::new();
        for (name, vals) in &fields {
            let c = compare(vals, &ends[&(step, name.to_string())]);
            if c.identical != vals.len() {
                bad.push(format!("{name}: {}/{} identical, worst vs max {:e}", c.identical, vals.len(), c.worst_vs_max));
            }
        }
        if !bad.is_empty() {
            eprintln!("first divergent step {step}: {}", bad.join("; "));
            return;
        }
    }
    eprintln!("no divergence in {last} steps");
}

/// Diagnostic, not a gate: compares the port's intermediate fields and matrix
/// coefficients of the first two steps with the upstream driver's
/// `dumpStages` output, stage by stage, to find the first divergent operation.
/// Reads the full driver log named by `RHOPIMPLE_STAGE_LOG` (default
/// `/home/user/upstream-build/gh1363/rhopimple/rhopimple.log`); does nothing when the
/// log is absent.
#[test]
fn stage_probe_against_upstream_driver_log() {
    let path = std::env::var("RHOPIMPLE_STAGE_LOG")
        .unwrap_or_else(|_| "/home/user/upstream-build/gh1363/rhopimple/rhopimple.log".to_string());
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("no stage log at {path}; skipped");
        return;
    };
    type Key = (usize, String, String);
    let mut up: BTreeMap<Key, Vec<Vec<f64>>> = BTreeMap::new();
    for line in text.lines() {
        let tok: Vec<&str> = line.split_whitespace().collect();
        if tok.len() != 6 || tok[0] != "S" || tok[2] == "end" || tok[2] == "geom" {
            continue;
        }
        let key = (tok[1].parse::<usize>().unwrap(), tok[2].to_string(), tok[3].to_string());
        let idx: usize = tok[4].parse().unwrap();
        let v = f64::from_bits(u64::from_str_radix(tok[5], 16).unwrap());
        let occ = up.entry(key).or_default();
        if idx == 0 {
            occ.push(Vec::new());
        }
        occ.last_mut().unwrap().push(v);
    }
    let probe: usize = std::env::var("RHOPIMPLE_PROBE_STAGE_STEP").ok().and_then(|v| v.parse().ok()).unwrap_or(2);
    let mut s = build_port();
    s.dump_stage_from = probe.saturating_sub(1).max(1);
    s.dump_stage_steps = probe;
    for _ in 0..probe {
        s.step().expect("step");
    }
    // the port records in execution order; a new step starts at every "rhoEqn"
    let mut mine: BTreeMap<Key, Vec<Vec<f64>>> = BTreeMap::new();
    let mut step = s.dump_stage_from - 1;
    for (tag, name, v) in &s.stages {
        if *tag == "rhoEqn" {
            step += 1;
        }
        mine.entry((step, tag.to_string(), name.to_string())).or_default().push(v.clone());
    }
    let mut bad = 0usize;
    for (key, occs) in &up {
        let Some(m) = mine.get(key) else {
            eprintln!("step {} {} {}: not recorded by the port", key.0, key.1, key.2);
            continue;
        };
        for (i, o) in occs.iter().enumerate() {
            let Some(mv) = m.get(i) else { continue };
            if mv.is_empty() && o.is_empty() {
                continue;
            }
            let c = compare(mv, o);
            if c.identical != o.len() {
                bad += 1;
                eprintln!(
                    "step {} {} {} [{}]: {}/{} identical, worst vs max {:e}, worst cell rel {:e}",
                    key.0, key.1, key.2, i, c.identical, o.len(), c.worst_vs_max, c.worst_cell_rel
                );
                let shown: Vec<String> = mv
                    .iter()
                    .zip(o)
                    .enumerate()
                    .filter(|(_, (a, b))| a.to_bits() != b.to_bits())
                    .take(6)
                    .map(|(k, (a, b))| format!("[{k}] port {a:e} up {b:e}"))
                    .collect();
                eprintln!("    {}", shown.join("; "));
            }
        }
    }
    eprintln!("stages with a difference: {bad}");
}
