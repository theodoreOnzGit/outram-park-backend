// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GeN-Foam heat exchangers (`structureProperties.heatExchangers`) in the `onePhase` region vs the real binary (gh:#1847, round 44)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Methodology, the comparison criteria and the predictions (committed BEFORE the first run of upstream or port of this round)
//! and the results: `verification_and_validation/genfoam_vs_upstream_gh1361/README.md`, "Round 44". Cases: `featureCases/1D_HX/onePhase`
//! AS SHIPPED (`tests/data/genfoam_1d_hx_onephase`, case A) and the same case with the secondary exchanger zone refined to 30 cells
//! (`tests/data/genfoam_1d_hx_nonconformal`, case B, non-conformal meshes, fractional `meshToMesh` weights). References are written by
//! `scripts/extract_hx_reference.sh` from runs of the unmodified `GeN-Foam` binary (652b3da on ESI v2506, serial) made with
//! `scripts/fc_protocol.sh` (`TIGHT=1 PSOLVER=pcg`, `END_TIME=20`: solvers 1e-14, `p_rgh` `GAMG -> PCG/DIC`, `writePrecision 17`,
//! every step written). The same edit is applied to a per-test copy by [`protocol_copy`].

// Reads OpenFOAM case files: `std` only (gh:#1633).
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::PressureSolve;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_fc::{
    OnePhaseFcCase, OnePhaseFcSolver,
};
use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;

/// The steps whose fields the references keep (README round 44, "Steps compared").
const STEPS: [usize; 15] = [1, 2, 3, 5, 10, 25, 50, 100, 200, 300, 500, 750, 1000, 1500, 2000];

struct Reference {
    n_steps: usize,
    dt: Vec<f64>,
    fields: HashMap<(usize, String), Vec<f64>>,
    solves: Vec<Vec<(f64, usize)>>,
}

fn load_reference(name: &str) -> Reference {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/genfoam_vs_upstream_gh1361/refs")
        .join(format!("{name}.ref.txt.gz"));
    let bytes = std::fs::read(&path).expect("reference present (git submodule update --init the refs submodule)");
    let text = String::from_utf8(decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")).expect("utf8");
    let mut r = Reference { n_steps: 0, dt: vec![], fields: HashMap::new(), solves: vec![] };
    for line in text.lines() {
        let mut w = line.split_whitespace();
        match w.next() {
            Some("N") => r.n_steps = w.next().expect("n").parse().expect("n"),
            Some("DT") => r.dt = w.map(|x| x.parse().expect("dt")).collect(),
            Some("F") => {
                let step: usize = w.next().expect("step").parse().expect("step");
                let f = w.next().expect("field").to_string();
                r.fields.insert((step, f), w.map(|x| x.parse().expect("value")).collect());
            }
            Some("P") => {
                let step: usize = w.next().expect("step").parse().expect("step");
                let v: Vec<f64> = w.map(|x| x.parse().expect("value")).collect();
                let pairs: Vec<(f64, usize)> = v.chunks(2).map(|c| (c[0], c[1] as usize)).collect();
                assert_eq!(r.solves.len() + 1, step, "P lines in step order");
                r.solves.push(pairs);
            }
            _ => {}
        }
    }
    r
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("mkdir");
    for e in std::fs::read_dir(from).expect("read_dir") {
        let e = e.expect("entry");
        let dest = to.join(e.file_name());
        if e.file_type().expect("type").is_dir() {
            copy_dir(&e.path(), &dest);
        } else {
            std::fs::copy(e.path(), &dest).expect("copy");
        }
    }
}

/// Copy of a vendored case with `scripts/fc_protocol.sh` (`TIGHT=1 PSOLVER=pcg`) applied (as round 38's test).
fn protocol_copy(case: &str, test: &str) -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(case);
    let dst = std::env::temp_dir().join(format!("gh1847_{test}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(&src, &dst);
    let cd = dst.join("system/controlDict");
    let text = std::fs::read_to_string(&cd).expect("controlDict");
    let text: Vec<String> = text
        .lines()
        .map(|l| {
            if l.starts_with("writePrecision ") {
                "writePrecision  17;".to_string()
            } else if l.starts_with("writeControl ") {
                "writeControl    timeStep;".to_string()
            } else if l.starts_with("writeInterval ") {
                "writeInterval   1;".to_string()
            } else {
                l.to_string()
            }
        })
        .collect();
    std::fs::write(&cd, text.join("\n") + "\n").expect("write controlDict");
    let f = dst.join("system/fluidRegion/fvSolution");
    let text = std::fs::read_to_string(&f).expect("fvSolution");
    let mut out = String::new();
    let mut in_solvers = false;
    let mut skip_smoother = false;
    for line in text.lines() {
        let t = line.trim_start();
        if line.starts_with("solvers") {
            in_solvers = true;
        }
        if line.starts_with("PIMPLE") {
            in_solvers = false;
        }
        if in_solvers && t.starts_with("tolerance") {
            out.push_str(&format!("        tolerance       {};\n", std::env::var("HX_TOL").unwrap_or("1e-14".into())));
        } else if in_solvers && t.starts_with("relTol") {
            out.push_str("        relTol          0;\n");
        } else if in_solvers && t.starts_with("maxIter") {
            // removed: PCG at 1e-14 needs more than the 25 sweeps shipped for GAMG
        } else if in_solvers && t.starts_with("solver") && t.contains("GAMG") {
            out.push_str("        solver          PCG;\n        preconditioner  DIC;\n");
            skip_smoother = true;
        } else if skip_smoother && t.starts_with("smoother") {
            skip_smoother = false;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    std::fs::write(&f, out).expect("write fvSolution");
    dst
}

/// `max |a - b| / max |b|` (absolute when the reference is identically zero).
fn worst_rel(port: &[f64], up: &[f64]) -> f64 {
    assert_eq!(port.len(), up.len(), "reference field has the wrong length");
    let scale = up.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
    port.iter().zip(up).map(|(a, b)| if scale == 0.0 { (a - b).abs() } else { (a - b).abs() / scale }).fold(0.0, f64::max)
}

fn worst_scaled(port: &[f64], up: &[f64], scale: f64) -> f64 {
    port.iter().zip(up).map(|(a, b)| (a - b).abs() / scale).fold(0.0, f64::max)
}

fn worst_abs(port: &[f64], up: &[f64]) -> f64 {
    port.iter().zip(up).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max)
}

/// The exchanged power density `iAHX*H*(THX - T)` [W/m^3] of one state (README round 44, "Exchanged heat").
fn exchanged(i_a_hx: &[f64], htc: &[f64], t_hx: &[f64], t: &[f64]) -> Vec<f64> {
    (0..t.len()).map(|c| i_a_hx[c] * htc[c] * (t_hx[c] - t[c])).collect()
}

/// The per-zone integral `sum V Q` over the primary cells of each exchanger.
fn zone_integrals(s: &OnePhaseFcSolver, vol: &[f64], q: &[f64]) -> Vec<f64> {
    s.structure
        .heat_exchangers
        .zones
        .iter()
        .map(|z| z.primary_cells.iter().map(|&c| vol[c] * q[c]).sum::<f64>())
        .collect()
}

type Row = Vec<(String, f64)>;

/// Per-field worst differences against the reference's step `step` (README round 44 criteria).
fn compare(s: &OnePhaseFcSolver, r: &Reference, step: usize, dt_used: f64, vol: &[f64]) -> Row {
    let rho_a = s.fluid.rho.c.iter().zip(&s.fluid.alpha.c).map(|(a, b)| a * b).fold(0.0_f64, f64::max);
    let ux: Vec<f64> = s.fluid.u.c.iter().map(|u| u.x).collect();
    let uy: Vec<f64> = s.fluid.u.c.iter().map(|u| u.y).collect();
    let uz: Vec<f64> = s.fluid.u.c.iter().map(|u| u.z).collect();
    let hx = &s.structure.heat_exchangers;
    let port: Vec<(&str, &Vec<f64>)> = vec![
        ("p", &s.p.c),
        ("p_rgh", &s.p_rgh.c),
        ("T", &s.fluid.t.c),
        ("Uz", &uz),
        ("T.fixedPower", &s.structure.t_act),
        ("htc", &s.htc),
        ("Re", &s.re),
        ("contErr", &s.fluid.cont_err),
        ("dgdt", &s.fluid.dgdt),
        ("T.HX", &hx.t_hx),
    ];
    let mut row: Row = Vec::new();
    for (name, v) in port {
        if name == "T.HX" && !hx.is_present() {
            continue; // the ablation (C1) has no `T.HX` field
        }
        if let Some(u) = r.fields.get(&(step, name.to_string())) {
            let w = match name {
                "contErr" => worst_scaled(v, u, rho_a / dt_used),
                "dgdt" => worst_scaled(v, u, 1.0 / dt_used),
                _ => worst_rel(v, u),
            };
            row.push((name.to_string(), w));
            if name == "T" {
                row.push(("dT_K_abs".to_string(), worst_abs(v, u)));
                // the cells of the exchanger zones only
                for (k, z) in hx.zones.iter().enumerate() {
                    let a: Vec<f64> = z.primary_cells.iter().map(|&c| v[c]).collect();
                    let b: Vec<f64> = z.primary_cells.iter().map(|&c| u[c]).collect();
                    row.push((format!("T[zone{k}]"), worst_rel(&a, &b)));
                }
            }
        }
    }
    for (name, v, tol) in [("Ux", &ux, 0.0), ("Uy", &uy, 0.0)] {
        if let Some(u) = r.fields.get(&(step, name.to_string())) {
            let _ = tol;
            row.push((format!("{name}_abs"), worst_abs(v, u)));
        }
    }
    // |U| scale (identical to Uz's own scale in a pencil mesh: Ux, Uy are zero)
    if let (Some(rx), Some(ry), Some(rz)) =
        (r.fields.get(&(step, "Ux".to_string())), r.fields.get(&(step, "Uy".to_string())), r.fields.get(&(step, "Uz".to_string())))
    {
        let umax = (0..rz.len()).map(|i| (rx[i] * rx[i] + ry[i] * ry[i] + rz[i] * rz[i]).sqrt()).fold(0.0_f64, f64::max);
        row.push(("Uz/|U|max".to_string(), worst_scaled(&uz, rz, umax)));
    }
    // exchanged heat, from each code's own htc, T, T.HX and the (asserted-equal) iAHX
    if !hx.is_present() {
        return row;
    }
    if let (Some(ut), Some(uh), Some(uthx)) = (
        r.fields.get(&(step, "T".to_string())),
        r.fields.get(&(step, "htc".to_string())),
        r.fields.get(&(step, "T.HX".to_string())),
    ) {
        let qp = exchanged(&hx.i_a_hx, &s.htc, &hx.t_hx, &s.fluid.t.c);
        let qu = exchanged(&hx.i_a_hx, uh, uthx, ut);
        row.push(("Q_hx".to_string(), worst_rel(&qp, &qu)));
        let ip = zone_integrals(s, vol, &qp);
        let iu = zone_integrals(s, vol, &qu);
        for (k, (a, b)) in ip.iter().zip(&iu).enumerate() {
            row.push((format!("intQ[zone{k}]"), if *b == 0.0 { (a - b).abs() } else { ((a - b) / b).abs() }));
        }
    }
    row
}

#[derive(Default, Debug)]
struct SolveTally {
    steps_count_mismatch: usize,
    straddle_steps: usize,
    solves: usize,
    n_tier: [usize; 3],
    violations_tier: [usize; 3],
    worst_iter_diff_tier: [usize; 3],
    first_dt_difference: usize,
    worst_dt_rel: f64,
    first_count_mismatch: usize,
}

/// Round 38's tiers (README round 44, criteria): returns `false` when the solve counts differ.
fn tally_solves(t: &mut SolveTally, step: usize, port: &[PressureSolve], up: &[(f64, usize)]) -> bool {
    for (k, (p, &(r_up, n_up))) in port.iter().zip(up).enumerate() {
        if (p.iterations == 0) != (n_up == 0) {
            if (5e-15..=2e-14).contains(&p.initial_residual) && (5e-15..=2e-14).contains(&r_up) {
                t.straddle_steps += 1;
                return true;
            }
            t.violations_tier[2] += 1;
            eprintln!("  unexplained convergence difference step {step} solve {k}: port ({:.3e}, {}) up ({r_up:.3e}, {n_up})", p.initial_residual, p.iterations);
            return false;
        }
        let dn = p.iterations.abs_diff(n_up);
        let tier = if r_up >= 1e-8 {
            0
        } else if r_up >= 1e-11 {
            1
        } else {
            2
        };
        t.solves += 1;
        t.n_tier[tier] += 1;
        t.worst_iter_diff_tier[tier] = t.worst_iter_diff_tier[tier].max(dn);
        let ratio = p.initial_residual / r_up;
        let bad = match tier {
            0 => dn > 1 || (p.initial_residual - r_up).abs() / r_up > 1e-3,
            1 => dn > 2 || !(0.1..=10.0).contains(&ratio),
            _ => !(p.initial_residual <= 10.0 * r_up || p.initial_residual <= 1e-10),
        };
        if bad {
            t.violations_tier[tier] += 1;
            eprintln!(
                "  violation tier {} step {step} solve {k}: port ({:.3e}, {}) upstream ({r_up:.3e}, {n_up})",
                ["i", "ii", "iii"][tier],
                p.initial_residual,
                p.iterations
            );
        }
    }
    port.len() == up.len()
}

struct Outcome {
    rows: Vec<(usize, Row)>,
    tally: SolveTally,
    solver: OnePhaseFcSolver,
    steps_run: usize,
}

fn run_and_compare(
    case: &str,
    tag: &str,
    refname: &str,
    ncells_check: usize,
    max_steps: usize,
    modify: fn(OnePhaseFcSolver) -> OnePhaseFcSolver,
) -> Outcome {
    let r = load_reference(refname);
    let dir = protocol_copy(case, tag);
    let vol = read_poly_mesh(&dir.join("constant/fluidRegion/polyMesh")).expect("mesh").cell_volumes.clone();
    assert_eq!(vol.len(), ncells_check);
    // upstream's stale `converged_` flag (round 4, README): the comparison must set it to compare like with like
    let mut s = modify(OnePhaseFcSolver::from_case_dir(&dir).expect("read the case").with_upstream_stale_pimple_converged_flag());
    let _ = std::fs::remove_dir_all(&dir);
    let mut rows = Vec::new();
    let mut tally = SolveTally::default();
    let mut steps_run = 0;
    for n in 1..=max_steps.min(r.n_steps) {
        let dt_used = s.dt;
        let dt_up = if n == 1 { 1e-3 } else { r.dt[n - 2] };
        tally.worst_dt_rel = tally.worst_dt_rel.max((dt_used - dt_up).abs() / dt_up);
        if dt_used != dt_up && tally.first_dt_difference == 0 {
            tally.first_dt_difference = n;
        }
        assert!((dt_used - dt_up).abs() <= 1e-12 * dt_up, "step {n}: deltaT {dt_used:e} differs from upstream's {dt_up:e}");
        s.step().expect("step");
        steps_run = n;
        if let Some(up) = r.solves.get(n - 1) {
            if !tally_solves(&mut tally, n, &s.last_step_solves, up) {
                tally.steps_count_mismatch += 1;
                if tally.first_count_mismatch == 0 {
                    tally.first_count_mismatch = n;
                }
                eprintln!("step {n}: port {} solves, upstream {}", s.last_step_solves.len(), up.len());
            }
        }
        if STEPS.contains(&n) || n == r.n_steps {
            rows.push((n, compare(&s, &r, n, dt_used, &vol)));
        }
        if !s.running() {
            break;
        }
    }
    Outcome { rows, tally, solver: s, steps_run }
}

fn print_rows(rows: &[(usize, Row)]) {
    for (n, w) in rows {
        let cells: Vec<String> = w.iter().map(|(k, v)| format!("{k}={v:.2e}")).collect();
        eprintln!("step {n:5} {}", cells.join(" "));
    }
}

fn worst_of(rows: &[(usize, Row)], name: &str) -> f64 {
    rows.iter().flat_map(|(_, w)| w.iter()).filter(|(k, _)| k == name).map(|(_, v)| *v).fold(0.0, f64::max)
}

/// The committed gate (README round 44, "Comparison criteria": every field 1e-9, `p_rgh`, `T` and the zone integrals 1e-10). Round 44 AMENDED it post-run
/// to 2e-7 (`p`, early steps), 1e-8 (`Uz`, `Re`, `htc`), 5e-7 (the exchanged heat), 5e-9 (`p`, steps 10..=200) at the measured size of a
/// deterministic pressure offset (gh:#1894). Round 45 found the cause (`fvMatrix::relax` on a `slip` wall, README round 45, fixed in
/// `OnePhaseFcSolver::relax_diag_boundary`) and RESTORED the committed gates: measured worst `p` 3.0e-12, `Uz` 1.6e-13, `Q` 5.3e-11,
/// `T` 3.5e-13 over all 2008 steps.
fn gate(_step: usize, name: &str) -> f64 {
    let strict = name == "T" || name.starts_with("T[") || name == "T.HX" || name == "p_rgh" || name.starts_with("intQ");
    if strict {
        1e-10
    } else {
        1e-9
    }
}

fn assert_gates(rows: &[(usize, Row)]) {
    for (n, w) in rows {
        for (k, v) in w {
            if k.ends_with("_abs") || k.starts_with("dT_K") {
                continue;
            }
            let g = gate(*n, k);
            assert!(*v <= g, "step {n}: {k} {v:.3e} above the {g:e} gate");
        }
    }
}

/// H1 (README round 44): the mapping of the shipped case is exact: every primary cell of both exchangers has one source entry of
/// weight exactly 1.0, and the translations equal the ones upstream logged (`Heat exchanger <name> mapping: applied translation`).
#[test]
fn shipped_mapping_is_one_to_one_and_the_translation_matches_the_upstream_log() {
    let s = OnePhaseFcCase::read(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_1d_hx_onephase"))
        .expect("read A");
    let hx = &s.structure.heat_exchangers;
    assert_eq!(hx.zones.len(), 2);
    let mut deltas: Vec<[f64; 3]> = Vec::new();
    for z in &hx.zones {
        assert_eq!(z.primary_cells.len(), 20);
        for &c in &z.primary_cells {
            assert_eq!(z.mapping.tgt_to_src_addr[c].len(), 1, "cell {c}");
            assert_eq!(z.mapping.tgt_to_src_wght[c][0], 1.0);
        }
        deltas.push([z.translation.x, z.translation.y, z.translation.z]);
        assert_eq!(z.i_a, 250.0);
        assert_eq!(z.hw, 2.5e4);
    }
    // upstream log (`logs/A.log`): sIHX1 (primary sHX) (0.19999999999999998 1.3877787807814457e-17 -4.4408920985006262e-16),
    // pIHX1 (primary pHX) the negative; the dictionary order of the port is the file's, so match by sign of x
    let (neg, pos): (Vec<&[f64; 3]>, Vec<&[f64; 3]>) = deltas.iter().partition(|d| d[0] < 0.0);
    assert_eq!((neg.len(), pos.len()), (1, 1));
    assert_eq!(pos[0][0], 0.19999999999999998);
    assert_eq!(pos[0][1], 1.3877787807814457e-17);
    assert_eq!(pos[0][2], -4.4408920985006262e-16);
    assert_eq!(neg[0][0], -0.19999999999999998);
    assert_eq!(neg[0][1], -1.3877787807814457e-17);
    assert_eq!(neg[0][2], 4.4408920985006262e-16);
    for c in 0..hx.i_a_hx.len() {
        let primary = hx.zones.iter().any(|z| z.primary_cells.contains(&c));
        assert_eq!(hx.i_a_hx[c], if primary { 250.0 } else { 0.0 });
    }
}

/// H2: the non-conformal variant: 2 or 3 positive weights summing to 1 on every primary cell of `pIHX1` (the 20-cell primary zone
/// against the 30-cell secondary zone).
#[test]
fn nonconformal_mapping_has_fractional_weights() {
    let s = OnePhaseFcCase::read(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_1d_hx_nonconformal"))
        .expect("read B");
    let hx = &s.structure.heat_exchangers;
    let mut fractional = 0;
    for z in &hx.zones {
        if z.primary_cells.len() != 20 {
            continue; // the 30-cell zone as primary: each of its cells overlaps one or two of the 20
        }
        for &c in &z.primary_cells {
            let w = &z.mapping.tgt_to_src_wght[c];
            assert!((2..=3).contains(&w.len()) || w.len() == 1, "cell {c}: {} entries", w.len());
            assert!(w.iter().all(|x| *x > 0.0));
            let sum: f64 = w.iter().sum();
            assert!((sum - 1.0).abs() <= 4.0 * f64::EPSILON, "cell {c}: weights sum to {sum}");
            if w.len() > 1 {
                fractional += 1;
            }
        }
    }
    assert!(fractional > 0, "the variant must exercise fractional weights");
}

/// The reader refuses what is not ported, by name.
#[test]
fn unported_heat_exchanger_keys_are_refused_by_name() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_1d_hx_onephase");
    let dst = std::env::temp_dir().join(format!("gh1847_refuse_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst);
    copy_dir(&src, &dst);
    let f = dst.join("constant/fluidRegion/phaseProperties");
    let text = std::fs::read_to_string(&f).expect("phaseProperties");
    let text = text.replacen("primary             \"pHX\";", "primary             \"pHX\";\n            secondaryRegion     otherRegion;", 1);
    std::fs::write(&f, text).expect("write");
    let err = format!("{}", OnePhaseFcCase::read(&dst).expect_err("must refuse"));
    let _ = std::fs::remove_dir_all(&dst);
    assert!(err.contains("secondaryRegion") && err.contains("not ported"), "{err}");
}

/// H3-H6, H8 (README round 44): case A, 2008 steps to 20 s.
#[test]
fn shipped_1d_hx_20s_tight_vs_upstream() {
    let o = run_and_compare("genfoam_1d_hx_onephase", "a", "hx_A_pcg", 120, usize::MAX, |s| s);
    print_rows(&o.rows);
    eprintln!("A: steps run {}, solve log: {:?}", o.steps_run, o.tally);
    assert_eq!(o.steps_run, 2008, "the same number of steps as upstream");
    assert_gates(&o.rows);
    assert_eq!(o.tally.steps_count_mismatch, 0);
    assert_eq!(o.tally.violations_tier.iter().sum::<usize>(), 0);
    let hx = &o.solver.structure.heat_exchangers;
    assert!(hx.is_active());
}

/// H7, H8: case B (non-conformal), 2008 steps.
#[test]
fn nonconformal_1d_hx_20s_tight_vs_upstream() {
    let o = run_and_compare("genfoam_1d_hx_nonconformal", "b", "hx_nc_pcg", 130, usize::MAX, |s| s);
    print_rows(&o.rows);
    eprintln!("B: steps run {}, solve log: {:?}", o.steps_run, o.tally);
    assert_eq!(o.steps_run, 2008);
    assert_gates(&o.rows);
    assert_eq!(o.tally.steps_count_mismatch, 0);
    assert_eq!(o.tally.violations_tier.iter().sum::<usize>(), 0);
}

fn worst_at(o: &Outcome, step: usize, name: &str) -> f64 {
    o.rows.iter().filter(|(n, _)| *n == step).flat_map(|(_, w)| w.iter()).filter(|(k, _)| k == name).map(|(_, v)| *v).fold(0.0, f64::max)
}

/// C1: heat exchangers OFF in the port: the zones' `T` is wrong by far more than the gate.
#[test]
fn control_without_heat_exchangers_differs() {
    let o = run_and_compare("genfoam_1d_hx_onephase", "c1", "hx_A_pcg", 120, 500, |s| s.without_heat_exchangers());
    print_rows(&o.rows);
    let on = worst_at(&o, 500, "T");
    eprintln!("C1: T at step 500 with the exchangers off: {on:.3e}");
    assert!(on >= 1e-4, "{on:e}");
}

/// C2: nearest-cell mapping instead of the overlap weights: on the non-conformal case it differs; on the conformal case it is
/// identical (a conformal mapping IS nearest-cell: the control cannot fail there).
#[test]
fn control_nearest_cell_mapping() {
    let b = run_and_compare("genfoam_1d_hx_nonconformal", "c2b", "hx_nc_pcg", 130, 500, |s| s.with_nearest_cell_heat_exchanger_mapping());
    print_rows(&b.rows);
    let wb = worst_of(&b.rows, "T");
    let a = run_and_compare("genfoam_1d_hx_onephase", "c2a", "hx_A_pcg", 120, 500, |s| s.with_nearest_cell_heat_exchanger_mapping());
    let wa = worst_of(&a.rows, "T");
    eprintln!("C2: T worst, nearest-cell mapping: non-conformal {wb:.3e}, conformal {wa:.3e}");
    assert!(wb >= 1e-7, "{wb:e}");
    assert!(wa <= 1e-10, "conformal mapping must be the same as nearest-cell: {wa:e}");
}

/// C3: the secondary `htc*T` with `T.prevIter()`.
#[test]
fn control_secondary_prev_iter_temperature() {
    let o = run_and_compare("genfoam_1d_hx_onephase", "c3", "hx_A_pcg", 120, 2000, |s| s.with_heat_exchanger_secondary_prev_iter_temperature());
    let w = worst_of(&o.rows, "T");
    eprintln!("C3: worst T difference with T.prevIter() on the secondary side: {w:.3e}");
    assert!(w >= 1e-9, "{w:e}");
}

/// DIAGNOSTIC (not a gate): per-cell differences of `p`, `p_rgh`, `Uz`, `T` after step 1 (`cargo test -- --ignored --nocapture`).
#[test]
#[ignore]
fn diagnostic_step1_cells() {
    let r = load_reference("hx_A_pcg");
    let dir = protocol_copy("genfoam_1d_hx_onephase", "diag");
    let mut s = OnePhaseFcSolver::from_case_dir(&dir).expect("read").with_upstream_stale_pimple_converged_flag();
    s.step().expect("step");
    let up = |f: &str| r.fields.get(&(1, f.to_string())).expect("field").clone();
    let uz: Vec<f64> = s.fluid.u.c.iter().map(|u| u.z).collect();
    for (name, v) in [("p", s.p.c.clone()), ("p_rgh", s.p_rgh.c.clone()), ("Uz", uz), ("T", s.fluid.t.c.clone())] {
        let u = up(name);
        eprint!("{name:6}");
        for c in 0..v.len() {
            eprint!(" {:.1e}", v[c] - u[c]);
        }
        eprintln!();
    }
    eprintln!("up p     {:?}", &up("p")[..8]);
    eprintln!("up Uz    {:?}", &up("Uz")[..8]);
}

/// DIAGNOSTIC (not a gate): step 1 with `nOuterCorrectors 1`, `nCorrectors 1` against `/home/user/upstream-build/gh1847/b1_*.txt`.
#[test]
#[ignore]
fn diagnostic_step1_one_corrector() {
    let dir = protocol_copy("genfoam_1d_hx_onephase", "diag1");
    if std::env::var("DIAG_P0").is_ok() {
        let pp = dir.join("constant/fluidRegion/phaseProperties");
        let t = std::fs::read_to_string(&pp).expect("pp").replace("powerDensity    1.1e9;", "powerDensity    0;");
        std::fs::write(&pp, t).expect("write");
    }
    let f = dir.join("system/fluidRegion/fvSolution");
    let text = std::fs::read_to_string(&f).expect("fvSolution");
    let text = text
        .lines()
        .map(|l| {
            if l.trim_start().starts_with("nOuterCorrectors") {
                format!("    nOuterCorrectors    {};", std::env::var("DIAG_NOUT").unwrap_or("1".into()))
            } else if l.trim_start().starts_with("nCorrectors") {
                format!("    nCorrectors    {};", std::env::var("DIAG_NCORR").unwrap_or("1".into()))
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&f, text).expect("write");
    let mut s = OnePhaseFcSolver::from_case_dir(&dir).expect("read").with_upstream_stale_pimple_converged_flag();
    if std::env::var("DIAG_NOHX").is_ok() {
        s = s.without_heat_exchangers();
    }
    s.step().expect("step");
    let up = |f: &str| -> Vec<f64> {
        std::fs::read_to_string(format!("/home/user/upstream-build/gh1847/{}_{f}.txt", std::env::var("DIAG_TAG").unwrap_or("b1".into())))
            .expect("dump")
            .lines()
            .map(|x| x.trim().parse().expect("num"))
            .collect()
    };
    let uz: Vec<f64> = s.fluid.u.c.iter().map(|u| u.z).collect();
    for (name, v) in [("p", s.p.c.clone()), ("p_rgh", s.p_rgh.c.clone()), ("Uz", uz), ("T", s.fluid.t.c.clone()), ("htc", s.htc.clone()), ("T.HX", s.structure.heat_exchangers.t_hx.clone())] {
        let u = up(name);
        let w = worst_rel(&v, &u);
        eprint!("{name:6} worst {w:.2e}:");
        for c in (0..v.len()).step_by(7) {
            eprint!(" {:.1e}", v[c] - u[c]);
        }
        eprintln!();
    }
}
