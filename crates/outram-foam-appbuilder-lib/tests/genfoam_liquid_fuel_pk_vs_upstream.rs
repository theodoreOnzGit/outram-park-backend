// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # Liquid-fuel `pointKineticNeutronics` against the real class (gh:#1882, gh:#1361 round 48)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da` on OpenFOAM-ESI v2506, the unmodified `solvers::pointKineticNeutronics`
//! (`libNeutronicsSolvers.so`) with `liquidFuel true`, driven by `upstream_driver/pkRunLF1882/pkRunLF.C`
//! on the `neutroRegion` mesh of `Tutorials/reactorCases/1D_MSR_pointKinetics` (640 cells, cyclic
//! top/bottom): 120 calls of the real `correctPhysics()` (`solvePointKineticsLiquidFuel.H`), `dt` cycling
//! 1e-3..0.2, with PRESCRIBED fluid fields (`U`, `alpha`, `alphat`, `mu`) and feedback fields from integer
//! formulas that this test repeats (`scripts/run_pk_lf_1882.sh`, `refs/pk1882_<variant>.ref.txt.gz`). The
//! reference holds, per step, the class' power, old power, reactivity, `precEquilibriumReactivity_`,
//! `liquidFuelBeta_`, `maxDeltaT()`, references, sums of the rescaled fields, per-group precursor sums and
//! cells, the whole `precPKStar_` at steps 0 1 2 3 10 40 119, and the real class' own `Info` lines (neutron
//! iterations, `neutroResidual`, intermediate power, `PBiCG` iterations and initial/final residual of
//! every solve) at 17 digits; at steps 0 1 2 7 40 the matrix of the first neutron iteration (groups 0, 7)
//! with `phi`, `diffCoeffPrec`, `diag`, `upper`, `lower`, `source` and the patch coefficients.
//!
//! The port runs its OWN loop (free run: nothing of upstream's per-step state is an input after the
//! constructor). Results and predictions: README "Round 48". Run with `--nocapture` to print every tally.

#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::neutronics::point_kinetics::cyclic_transport::{
    assemble_precursor_matrix, parse_cyclic_neighbours, PrecursorEquationInputs, TransportMesh, TransportPatchKind,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::point_kinetics::model::liquid_fuel::{
    AlphaUpdate, FluidFields, LiquidFuelConfig, LiquidFuelPk,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::point_kinetics::model::{
    feedback_cell_field, FeedbackCoefficients, PkConfig, PkFeedbackFields, PkMeshData,
};
use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_basic_lib::prelude::Vector3;

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    String::from_utf8(decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")).expect("utf8")
}

fn nums(parts: &[&str]) -> Vec<f64> {
    parts.iter().map(|w| w.parse::<f64>().unwrap_or_else(|_| panic!("number {w}"))).collect()
}

/// The integer-formula field of the driver: `double((a*i + b*k) % m) / m`.
fn frac(i: usize, k: usize, a: usize, b: usize, m: usize) -> f64 {
    ((a * i + b * k) % m) as f64 / m as f64
}

const BETA: [f64; 8] = [2.114580e-04, 4.231390e-04, 3.737000e-04, 5.789020e-04, 8.929690e-04, 1.426090e-04, 1.926010e-04, 3.738980e-05];
const LAMBDA: [f64; 8] = [1.246670e-02, 2.829170e-02, 4.252440e-02, 1.330420e-01, 2.924670e-01, 6.664880e-01, 1.634780e+00, 3.099610e+00];

#[derive(Default, Clone)]
struct MatrixRef {
    diag: Vec<f64>,
    upper: Vec<f64>,
    lower: Vec<f64>,
    source: Vec<f64>,
    ints: HashMap<usize, Vec<f64>>,
    bous: HashMap<usize, Vec<f64>>,
}

#[derive(Default, Clone)]
struct StepRef {
    t: f64,
    dt: f64,
    k: Vec<f64>,
    p: HashMap<usize, Vec<f64>>,
    ps: HashMap<usize, Vec<f64>>,
    f: Vec<f64>,
    solves: Vec<(usize, f64, f64)>,
    powers: Vec<f64>,
    residuals: Vec<f64>,
    phi: Vec<f64>,
    phib: HashMap<usize, Vec<f64>>,
    diff: Vec<f64>,
    matrices: HashMap<usize, MatrixRef>,
    sv: HashMap<usize, Vec<f64>>,
}

#[derive(Default)]
struct Reference {
    cfg: Vec<Vec<String>>,
    vol: Vec<f64>,
    zones: HashMap<String, Vec<usize>>,
    geom: HashMap<String, Vec<f64>>,
    gp: Vec<(String, HashMap<String, Vec<f64>>)>,
    steps: Vec<StepRef>,
}

fn read_reference(variant: &str) -> Reference {
    let text = gunzip(&vv_dir().join(format!("refs/pk1882_{variant}.ref.txt.gz")));
    let mut r = Reference::default();
    let mut cur: Option<usize> = None; // step index of the Info lines
    let mut mat_k = 0usize;
    let mut mat_g = 0usize;
    let mut in_patch = false;
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        if w.is_empty() {
            continue;
        }
        if r.steps.len() < 200 && w[0] == "STEP" {
            let k: usize = w[1].parse().unwrap();
            while r.steps.len() <= k {
                r.steps.push(StepRef::default());
            }
            cur = Some(k);
            continue;
        }
        match w[0] {
            "CFG" => r.cfg.push(w[1..].iter().map(|s| s.to_string()).collect()),
            "VOL" => r.vol = nums(&w[1..]),
            "ZONE" => {
                let n: usize = w[2].parse().unwrap();
                r.zones.insert(w[1].to_string(), w[3..3 + n].iter().map(|s| s.parse().unwrap()).collect());
            }
            "G" => {
                r.geom.insert(w[1].to_string(), nums(&w[2..]));
            }
            "GP" => {
                if w[1].parse::<usize>().is_ok() {
                    r.gp.push((w[2].to_string(), HashMap::new()));
                    in_patch = true;
                } else if in_patch {
                    r.gp.last_mut().unwrap().1.insert(w[1].to_string(), nums(&w[2..]));
                }
            }
            "PHI" => {
                let k: usize = w[1].parse().unwrap();
                while r.steps.len() <= k {
                    r.steps.push(StepRef::default());
                }
                mat_k = k;
                r.steps[k].phi = nums(&w[2..]);
            }
            "PHIB" => {
                let p: usize = w[2].parse().unwrap();
                r.steps[mat_k].phib.insert(p, nums(&w[3..]));
            }
            "DIFF" => r.steps[mat_k].diff = nums(&w[1..]),
            "M" => {
                mat_k = w[1].parse().unwrap();
                mat_g = w[2].parse().unwrap();
                r.steps[mat_k].matrices.insert(mat_g, MatrixRef::default());
            }
            "MD" => r.steps[mat_k].matrices.get_mut(&mat_g).unwrap().diag = nums(&w[1..]),
            "MU" => r.steps[mat_k].matrices.get_mut(&mat_g).unwrap().upper = nums(&w[1..]),
            "ML" => r.steps[mat_k].matrices.get_mut(&mat_g).unwrap().lower = nums(&w[1..]),
            "MS" => r.steps[mat_k].matrices.get_mut(&mat_g).unwrap().source = nums(&w[1..]),
            "SV" => {
                let k: usize = w[1].parse().unwrap();
                let g: usize = w[2].parse().unwrap();
                let it: f64 = w[3].parse().unwrap();
                let init: f64 = w[4].parse().unwrap();
                let fin: f64 = w[5].parse().unwrap();
                let mut v = vec![it, init, fin];
                v.extend(nums(&w[6..]));
                r.steps[k].sv.insert(g, v);
            }
            "MI" => {
                let p: usize = w[1].parse().unwrap();
                r.steps[mat_k].matrices.get_mut(&mat_g).unwrap().ints.insert(p, nums(&w[3..]));
            }
            "MB" => {
                let p: usize = w[1].parse().unwrap();
                r.steps[mat_k].matrices.get_mut(&mat_g).unwrap().bous.insert(p, nums(&w[3..]));
            }
            "K" => {
                let k: usize = w[1].parse().unwrap();
                let flat: Vec<f64> = w[2..].iter().filter(|s| **s != "|").map(|s| s.parse().unwrap()).collect();
                r.steps[k].t = flat[0];
                r.steps[k].dt = flat[1];
                r.steps[k].k = flat[2..].to_vec();
            }
            "P" => {
                let k: usize = w[1].parse().unwrap();
                let g: usize = w[2].parse().unwrap();
                let flat: Vec<f64> = w[3..].iter().filter(|s| **s != "|").map(|s| s.parse().unwrap()).collect();
                r.steps[k].p.insert(g, flat);
            }
            "PS" => {
                let k: usize = w[1].parse().unwrap();
                let g: usize = w[2].parse().unwrap();
                r.steps[k].ps.insert(g, nums(&w[3..]));
            }
            "F" => {
                let k: usize = w[1].parse().unwrap();
                r.steps[k].f = nums(&w[2..]);
            }
            _ => {
                let k = cur.unwrap_or(0);
                if line.contains("Solving for precPKStar") {
                    // DILUPBiCG:  Solving for precPKStar0, Initial residual = X, Final residual = Y, No Iterations N
                    let parts: Vec<&str> = line.split(',').collect();
                    let init: f64 = parts[1].split('=').nth(1).unwrap().trim().parse().unwrap();
                    let fin: f64 = parts[2].split('=').nth(1).unwrap().trim().parse().unwrap();
                    let it: usize = parts[3].trim().rsplit(' ').next().unwrap().parse().unwrap();
                    r.steps[k].solves.push((it, init, fin));
                } else if line.starts_with("Intermediate Power") {
                    r.steps[k].powers.push(line.split('=').nth(1).unwrap().trim().parse().unwrap());
                } else if line.starts_with("neutroResidual") {
                    r.steps[k].residuals.push(line.split(':').nth(2).unwrap().trim().parse().unwrap());
                }
            }
        }
    }
    r
}

fn cfg_value(r: &Reference, tag: &str, key: &str) -> String {
    for l in &r.cfg {
        if l[0] == tag {
            if let Some(p) = l.iter().position(|w| w == key) {
                return l[p + 1].clone();
            }
        }
    }
    panic!("no CFG {tag} {key}")
}

/// A tally of comparisons: how many are bit-identical, and the worst relative difference.
#[derive(Default, Clone, Copy)]
struct Tally {
    n: usize,
    equal: usize,
    worst: f64,
}

impl Tally {
    fn add(&mut self, port: f64, up: f64) {
        self.n += 1;
        if port == up || (port.is_nan() && up.is_nan()) {
            self.equal += 1;
        } else {
            let rel = (port - up).abs() / port.abs().max(up.abs()).max(1e-300);
            if rel > self.worst {
                self.worst = rel;
            }
        }
    }
    fn add_all(&mut self, port: &[f64], up: &[f64]) {
        assert_eq!(port.len(), up.len(), "length");
        for i in 0..port.len() {
            self.add(port[i], up[i]);
        }
    }
    fn show(&self, name: &str) {
        println!("  {name}: {}/{} bit-identical, worst rel {:.3e}", self.equal, self.n, self.worst);
    }
}

struct Setup {
    case: Reference,
    tm: TransportMesh,
    n: usize,
    mesh_data: PkMeshData,
    flux0: Vec<f64>,
    flow: String,
    alpha_mode: String,
}

fn cells_of(r: &Reference, names: &[&str], n: usize) -> Vec<Vec<usize>> {
    let _ = n;
    names.iter().map(|z| r.zones[*z].clone()).collect()
}

fn setup(variant: &str) -> Setup {
    let case = read_reference(variant);
    let mesh = read_poly_mesh(&vv_dir().join("cases/msr1d_neutro/polyMesh")).expect("polyMesh");
    let n = mesh.n_cells;
    assert_eq!(n, 640);
    let boundary = std::fs::read_to_string(vv_dir().join("cases/msr1d_neutro/polyMesh/boundary")).expect("boundary");
    let tm = TransportMesh::from_fv_mesh(&mesh, &parse_cyclic_neighbours(&boundary)).expect("transport mesh");
    let mesh_data = PkMeshData {
        volume: mesh.cell_volumes.clone(),
        fuel: feedback_cell_field(n, &cells_of(&case, &["core", "HotLeg"], n)),
        // upstream reads `coolFeedbackZones`
        cool: feedback_cell_field(n, &cells_of(&case, &["core", "HotLeg", "hx"], n)),
        structure: feedback_cell_field(n, &[]),
        structure_mech: feedback_cell_field(n, &cells_of(&case, &["hx"], n)),
        driveline: feedback_cell_field(n, &[]),
    };
    let mut flux0 = vec![0.0; n];
    for (z, v) in [("core", 1.0), ("HotLeg", 0.7), ("pump", 0.3), ("hx", 0.1), ("ColdLeg", 0.4)] {
        for &c in &case.zones[z] {
            flux0[c] = v;
        }
    }
    let flow = cfg_value(&case, "flow", "flow");
    let alpha_mode = cfg_value(&case, "flow", "alpha");
    Setup { case, tm, n, mesh_data, flux0, flow, alpha_mode }
}

fn pk_config() -> PkConfig {
    PkConfig {
        prompt_generation_time: 1.26e-6,
        betas: BETA.to_vec(),
        lambdas: LAMBDA.to_vec(),
        fast_neutrons: true,
        coefficients: FeedbackCoefficients {
            doppler: 4.0e-3,
            t_fuel: -3.54253e-05,
            t_clad: -1.0e-05,
            t_cool: -2.0e-05,
            rho_cool: 1.5e-07,
            t_struct: -4.0e-05,
            t_struct_mech: 3.0e-05,
            driveline_expansion: 0.0,
        },
        control_rod_map: vec![],
        gem_map: vec![],
        external_reactivity: None,
        boron_reactivity: None,
        decay_power: None,
        p_target: 1.0e9,
        subcritical_index: None,
        ksrc: None,
        precursor_powers: None,
        external_source: None,
        energy_groups: 6,
        start_time: 0.0,
    }
}

fn pk_fields(n: usize, k: usize) -> PkFeedbackFields {
    PkFeedbackFields {
        t_fuel: (0..n).map(|i| 900.0 + 150.0 * frac(i, k, 3, 5, 97)).collect(),
        t_clad: (0..n).map(|i| 850.0 + 60.0 * frac(i, k, 5, 3, 89)).collect(),
        t_cool: (0..n).map(|i| 800.0 + 50.0 * frac(i, k, 7, 2, 83)).collect(),
        rho_cool: (0..n).map(|i| 3300.0 - 80.0 * frac(i, k, 2, 9, 79)).collect(),
        t_struct: (0..n).map(|i| 780.0 + 30.0 * frac(i, k, 11, 4, 73)).collect(),
        t_struct_mech: (0..n).map(|i| 700.0 + 20.0 * frac(i, k, 13, 6, 71)).collect(),
    }
}

fn fluid_fields(n: usize, k: usize, flow: &str, alpha_mode: &str) -> FluidFields {
    let u = (0..n)
        .map(|i| {
            let uz = match flow {
                "flow" => 0.4 + 0.3 * frac(i, k, 7, 3, 53),
                "rev" => frac(i, k, 7, 3, 53) - 0.4,
                "slow" => 0.02 + 0.01 * frac(i, k, 7, 3, 53),
                _ => 0.0,
            };
            Vector3::new(0.0, 0.0, uz)
        })
        .collect();
    let alpha = if alpha_mode == "const" { vec![1.0; n] } else { (0..n).map(|i| 0.5 + 0.5 * frac(i, k, 5, 2, 47)).collect() };
    FluidFields {
        u,
        alpha,
        alphat: (0..n).map(|i| 2.0 + 3.0 * frac(i, k, 3, 11, 41)).collect(),
        mu: (0..n).map(|i| 4.0 + 2.0 * frac(i, k, 5, 7, 43)).collect(),
    }
}

fn lf_config(s: &Setup, alpha_update: AlphaUpdate) -> LiquidFuelConfig {
    LiquidFuelConfig {
        sc_no: cfg_value(&s.case, "liquid", "ScNo").parse().unwrap(),
        init_precursors: cfg_value(&s.case, "liquid", "initPrec") == "1",
        neutron_iteration_residual: 1.0e-6,
        max_neutron_iterations: 50,
        solver_tolerance: 1.0e-6,
        solver_rel_tol: 1.0e-3,
        solver_max_iter: 1000,
        solver_min_iter: 0,
        alpha_update,
    }
}

/// The named ablations (controls). `None` is upstream's form.
#[derive(Clone, Copy, PartialEq)]
enum Ablation {
    None,
    NoNonOrthCorrection,
    FrozenPrecursors,
}

fn new_pk(s: &Setup, alpha_update: AlphaUpdate) -> LiquidFuelPk {
    new_pk_with(s, alpha_update, Ablation::None)
}

fn new_pk_with(s: &Setup, alpha_update: AlphaUpdate, ablation: Ablation) -> LiquidFuelPk {
    let n = s.n;
    let pd: Vec<f64> = (0..n).map(|i| 1.0e6 + 3.0e5 * frac(i, 1, 17, 1, 61)).collect();
    let spd: Vec<f64> = (0..n).map(|i| 2.0e5 + 1.0e5 * frac(i, 1, 19, 1, 59)).collect();
    let pk = LiquidFuelPk::new(pk_config(), s.mesh_data.clone(), s.flux0.clone(), pd, spd, s.tm.clone(), lf_config(s, alpha_update))
        .expect("class");
    match ablation {
        Ablation::None => pk,
        Ablation::NoNonOrthCorrection => pk.without_non_orth_correction(),
        Ablation::FrozenPrecursors => pk.with_frozen_precursors(),
    }
}

fn sum(v: &[f64]) -> f64 {
    let mut s = 0.0;
    for x in v {
        s += x;
    }
    s
}

/// What a variant run measured.
struct Outcome {
    power: Tally,
    scalars: Tally,
    iters: Tally,
    residuals: Tally,
    inter: Tally,
    solve_it: Tally,
    solve_res: Tally,
    prec_sums: Tally,
    prec_cells: Tally,
    prec_full: Tally,
    fields: Tally,
    first_diverging_step: Option<usize>,
}

fn run_variant(variant: &str, alpha_update: AlphaUpdate, matrices: bool) -> (Outcome, Option<Tally>) {
    run_variant_with(variant, alpha_update, matrices, Ablation::None)
}

fn run_variant_with(variant: &str, alpha_update: AlphaUpdate, matrices: bool, ablation: Ablation) -> (Outcome, Option<Tally>) {
    let s = setup(variant);
    let mut pk = new_pk_with(&s, alpha_update, ablation);
    let mut o = Outcome {
        power: Tally::default(),
        scalars: Tally::default(),
        iters: Tally::default(),
        residuals: Tally::default(),
        inter: Tally::default(),
        solve_it: Tally::default(),
        solve_res: Tally::default(),
        prec_sums: Tally::default(),
        prec_cells: Tally::default(),
        prec_full: Tally::default(),
        fields: Tally::default(),
        first_diverging_step: None,
    };
    let mut mat = Tally::default();
    let steps = s.case.steps.len();
    for k in 0..steps {
        let r = &s.case.steps[k];
        let fields = pk_fields(s.n, k);
        let fluid = fluid_fields(s.n, k, &s.flow, &s.alpha_mode);
        if matrices && !r.matrices.is_empty() {
            for (&g, m) in &r.matrices {
                let (eqn, phi, diff) = pk.first_iteration_matrix(k + 1, r.dt, &fields, &fluid, g, k == 0).expect("matrix");
                mat.add_all(&phi.internal, &r.phi);
                mat.add_all(&diff, &r.diff);
                for (&p, v) in &r.phib {
                    mat.add_all(&phi.patches[p][..v.len()], v);
                }
                mat.add_all(&eqn.diag, &m.diag);
                mat.add_all(&eqn.upper, &m.upper);
                mat.add_all(&eqn.lower, &m.lower);
                mat.add_all(&eqn.source, &m.source);
                for (&p, v) in &m.ints {
                    mat.add_all(&eqn.internal_coeffs[p][..v.len()], v);
                }
                for (&p, v) in &m.bous {
                    mat.add_all(&eqn.boundary_coeffs[p][..v.len()], v);
                }
            }
        }
        let rep = pk.step(k + 1, r.t, r.dt, &fields, &fluid).expect("step");
        let before = (o.power.n - o.power.equal) + (o.scalars.n - o.scalars.equal) + (o.solve_it.n - o.solve_it.equal)
            + (o.iters.n - o.iters.equal) + (o.inter.n - o.inter.equal);
        // Info lines
        o.iters.add(rep.neutron_iterations as f64, r.residuals.len() as f64);
        let ncmp = rep.neutro_residuals.len().min(r.residuals.len());
        o.residuals.add_all(&rep.neutro_residuals[..ncmp], &r.residuals[..ncmp]);
        let ncmp = rep.intermediate_powers.len().min(r.powers.len());
        o.inter.add_all(&rep.intermediate_powers[..ncmp], &r.powers[..ncmp]);
        o.solve_it.add(rep.solves.len() as f64, r.solves.len() as f64);
        let ncmp = rep.solves.len().min(r.solves.len());
        for i in 0..ncmp {
            o.solve_it.add(rep.solves[i].0 as f64, r.solves[i].0 as f64);
            o.solve_res.add(rep.solves[i].1, r.solves[i].1);
            o.solve_res.add(rep.solves[i].2, r.solves[i].2);
        }
        // K line: power powerOld totalReact precEq lfBeta maxDeltaT | sums | refs..., pOld | refs
        let b = pk.base();
        let mdt = pk.max_delta_t(r.dt, None, None);
        o.power.add(b.power(), r.k[0]);
        o.scalars.add(b.total_reactivity(), r.k[2]);
        o.scalars.add(pk.prec_equilibrium_reactivity(), r.k[3]);
        o.scalars.add(pk.liquid_fuel_beta(), r.k[4]);
        o.scalars.add(mdt, r.k[5]);
        o.fields.add(sum(&b.one_group_flux), r.k[6]);
        o.fields.add(sum(&b.fluxes[5]), r.k[7]);
        o.fields.add(sum(&b.power_density), r.k[8]);
        o.fields.add(sum(&b.secondary_power_density), r.k[9]);
        if let Some(refs) = b.references() {
            o.scalars.add(refs.t_fuel, r.k[10]);
            o.scalars.add(refs.t_clad, r.k[11]);
            o.scalars.add(refs.t_cool, r.k[12]);
        } else {
            o.scalars.add(0.0, r.k[10]);
        }
        for g in 0..8 {
            let pr = &r.p[&g];
            o.prec_sums.add(sum(pk.prec_pk_star(g)), pr[0]);
            o.prec_sums.add(sum(pk.prec_pk(g)), pr[1]);
            let st = pk.prec_pk_star(g);
            o.prec_cells.add(st[0], pr[2]);
            o.prec_cells.add(st[s.n / 2], pr[3]);
            o.prec_cells.add(st[s.n - 1], pr[4]);
            o.prec_cells.add(pk.prec_pk(g)[0], pr[5]);
            o.prec_cells.add(pk.prec_pk(g)[s.n - 1], pr[6]);
            if let Some(full) = r.ps.get(&g) {
                o.prec_full.add_all(st, full);
            }
        }
        o.fields.add(b.one_group_flux[0], r.f[0]);
        o.fields.add(b.one_group_flux[s.n / 2], r.f[1]);
        o.fields.add(b.one_group_flux[s.n - 1], r.f[2]);
        let after = (o.power.n - o.power.equal) + (o.scalars.n - o.scalars.equal) + (o.solve_it.n - o.solve_it.equal)
            + (o.iters.n - o.iters.equal) + (o.inter.n - o.inter.equal);
        if after > before && o.first_diverging_step.is_none() {
            o.first_diverging_step = Some(k);
        }
    }
    (o, if matrices { Some(mat) } else { None })
}

fn print_outcome(name: &str, o: &Outcome, mat: Option<&Tally>) {
    println!("{name}: first diverging step {:?}", o.first_diverging_step);
    o.power.show("power");
    o.scalars.show("reactivity, precEq, liquidFuelBeta, maxDeltaT, references");
    o.iters.show("neutron iterations per step");
    o.residuals.show("neutroResidual");
    o.inter.show("intermediate power");
    o.solve_it.show("PBiCG iteration counts");
    o.solve_res.show("PBiCG initial/final residual");
    o.prec_sums.show("precursor sums");
    o.prec_cells.show("precursor sample cells");
    o.prec_full.show("precPKStar_ whole field (dumped steps)");
    o.fields.show("rescaled field sums and samples");
    if let Some(m) = mat {
        m.show("first-iteration matrix, phi, diffCoeffPrec, patch coefficients");
    }
}

/// Every tally is bit-identical (the gate; set post-run at the measured size: all equal, README Round 48).
fn assert_identical(name: &str, o: &Outcome, mat: Option<&Tally>) {
    let all: [(&str, &Tally); 11] = [
        ("power", &o.power),
        ("scalars", &o.scalars),
        ("neutron iterations", &o.iters),
        ("neutroResidual", &o.residuals),
        ("intermediate power", &o.inter),
        ("PBiCG iteration counts", &o.solve_it),
        ("PBiCG residuals", &o.solve_res),
        ("precursor sums", &o.prec_sums),
        ("precursor cells", &o.prec_cells),
        ("precPKStar_", &o.prec_full),
        ("rescaled fields", &o.fields),
    ];
    for (label, t) in all {
        assert!(t.n > 0, "{name}: {label} compared nothing");
        assert_eq!(t.equal, t.n, "{name}: {label} not bit-identical (worst rel {:e})", t.worst);
    }
    assert_eq!(o.first_diverging_step, None, "{name}");
    if let Some(m) = mat {
        assert!(m.n > 30000, "{name}: the matrix comparison did not run");
        assert_eq!(m.equal, m.n, "{name}: matrix not bit-identical (worst rel {:e})", m.worst);
    }
}

fn worst_of(o: &Outcome) -> f64 {
    [&o.power, &o.scalars, &o.residuals, &o.inter, &o.prec_sums, &o.prec_cells, &o.prec_full, &o.fields]
        .iter()
        .fold(0.0_f64, |a, t| a.max(t.worst))
}

#[test]
fn the_controls_fail_as_predicted() {
    // P6: precursors not transported after the equilibrium initialisation
    let (o, _) = run_variant_with("flow", AlphaUpdate::Assign, false, Ablation::FrozenPrecursors);
    println!("frozen precursors: power {}/{} equal, worst rel {:e}, precPKStar_ {}/{} equal, worst {:e}", o.power.equal, o.power.n, o.power.worst, o.prec_full.equal, o.prec_full.n, o.prec_full.worst);
    assert!(o.power.worst > 1.0e-4, "frozen precursors must change the power");
    // the explicit non-orthogonal correction: left out, the matrix source and then the field differ
    let (o, m) = run_variant_with("rev", AlphaUpdate::Assign, true, Ablation::NoNonOrthCorrection);
    let m = m.expect("matrices");
    println!("no non-orth correction (rev): matrix {}/{} equal, worst {:e}; precPKStar_ {}/{} equal, worst {:e}", m.equal, m.n, m.worst, o.prec_full.equal, o.prec_full.n, o.prec_full.worst);
    assert!(m.equal < m.n, "the correction must matter for the rev variant");
    assert!(o.prec_full.equal < o.prec_full.n);
    // P4: alpha.oldTime() of the assignment form against the element-write form
    let (o, _) = run_variant_with("alpha_assign", AlphaUpdate::Direct, false, Ablation::None);
    println!("alpha_assign run with the Direct old-time form: worst rel {:e}, power {}/{} equal", worst_of(&o), o.power.equal, o.power.n);
    assert!(worst_of(&o) > 1.0e-6, "the old-time alpha must matter");
    // and the converse: the direct reference against the assignment form
    let (o, _) = run_variant_with("alpha_direct", AlphaUpdate::Assign, false, Ablation::None);
    println!("alpha_direct run with the Assign old-time form: worst rel {:e}", worst_of(&o));
    assert!(worst_of(&o) > 1.0e-6);
}

#[test]
fn the_seam_as_an_ldu_face_is_not_the_binarys_solve() {
    // S2: the same matrices (bit-identical to upstream's, round 48) solved with the seam as one more LDU face
    // (basic-lib's FvMesh form): the preconditioner sees the seam and Amul has no interface part.
    let s = setup("flowall");
    let mut pk = new_pk(&s, AlphaUpdate::Assign);
    let (mut n_solves, mut diff_iters, mut diff_sol, mut worst) = (0, 0, 0, 0.0_f64);
    for k in 0..s.case.steps.len() {
        let r = &s.case.steps[k];
        let fields = pk_fields(s.n, k);
        let fluid = fluid_fields(s.n, k, &s.flow, &s.alpha_mode);
        for (&g, _m) in &r.matrices {
            let (eqn, _phi, _diff) = pk.first_iteration_matrix(k + 1, r.dt, &fields, &fluid, g, k == 0).expect("matrix");
            let sv = &r.sv[&g];
            let mut psi = pk.prec_pk_star(g).to_vec();
            let perf = eqn.solve_seam_as_ldu_face(&s.tm, &mut psi, 1.0e-6, 1.0e-3, 1000, 0);
            n_solves += 1;
            if perf.iterations as f64 != sv[0] {
                diff_iters += 1;
            }
            let mut t = Tally::default();
            t.add_all(&psi, &sv[3..]);
            if t.equal < t.n {
                diff_sol += 1;
            }
            worst = worst.max(t.worst);
        }
        let _ = pk.step(k + 1, r.t, r.dt, &fields, &fluid).expect("step");
    }
    println!("seam as an LDU face: {n_solves} single solves, {diff_iters} with a different iteration count, {diff_sol} with a different solution, worst rel {worst:e}");
    assert!(diff_sol > n_solves / 2, "the seam form must differ from the binary in most solves");
}

#[test]
fn unsupported_patches_and_the_cyclic_parser_are_refused_or_read_by_name() {
    use outram_foam_appbuilder_lib::genfoam::neutronics::point_kinetics::cyclic_transport::TransportError;
    let text = std::fs::read_to_string(vv_dir().join("cases/msr1d_neutro/polyMesh/boundary")).expect("boundary");
    assert_eq!(
        parse_cyclic_neighbours(&text),
        vec![("top".to_string(), "bottom".to_string()), ("bottom".to_string(), "top".to_string())]
    );
    // a wedge patch is refused by name
    let mesh = read_poly_mesh(&vv_dir().join("cases/msr1d_neutro/polyMesh")).expect("polyMesh");
    let mut m = (*mesh).clone();
    m.patches[2].kind = outram_foam_basic_lib::mesh::PatchKind::Wedge;
    match TransportMesh::from_fv_mesh(&m, &parse_cyclic_neighbours(&text)) {
        Err(TransportError::UnsupportedPatch(name, _)) => assert_eq!(name, "walls"),
        other => panic!("expected the wedge patch to be refused, got {:?}", other.map(|_| ())),
    }
    // a cyclic patch with no resolvable partner is refused
    match TransportMesh::from_fv_mesh(&mesh, &[]) {
        Err(TransportError::CyclicPartner(name)) => assert_eq!(name, "top"),
        other => panic!("expected the unresolved cyclic patch to be refused, got {:?}", other.map(|_| ())),
    }
}

#[test]
fn the_class_accessors_report_the_constructor_quantities() {
    let s = setup("flow");
    let pk = new_pk(&s, AlphaUpdate::Assign);
    let sum_init_n = sum(pk.init_one_group_flux_n());
    let up: f64 = cfg_value(&s.case, "sumFlux0", "sumInitN").parse().unwrap();
    let dom: f64 = cfg_value(&s.case, "sumFlux0", "domInt").parse().unwrap();
    assert_eq!(sum_init_n, up);
    assert_eq!(pk.domain_integrated_init_n(), dom);
    assert_eq!(pk.transport_mesh().n_cells, 640);
    assert_eq!(pk.liquid_fuel_beta(), 0.0);
    assert_eq!(pk.prec_equilibrium_reactivity(), 0.0);
}

#[test]
fn the_neutro_mesh_geometry_matches_the_real_mesh() {
    let s = setup("flow");
    let g = &s.case.geom;
    let mut t = Tally::default();
    let n_int = s.tm.n_internal_faces;
    assert_eq!(n_int, 639);
    for f in 0..n_int {
        t.add(s.tm.weights[f], g["w"][f]);
        t.add(s.tm.non_orth_delta[f], g["nonOrthDelta"][f]);
        t.add(s.tm.mag_sf[f], g["magSf"][f]);
        t.add(s.tm.sf[f].x, g["Sf"][3 * f]);
        t.add(s.tm.sf[f].y, g["Sf"][3 * f + 1]);
        t.add(s.tm.sf[f].z, g["Sf"][3 * f + 2]);
    }
    t.show("internal faces: weights, nonOrthDelta, magSf, Sf");
    let mut vol = Tally::default();
    vol.add_all(&s.tm.volume, &s.case.vol);
    vol.show("cell volumes");
    let mut seam = Tally::default();
    for (pi, p) in s.tm.patches.iter().enumerate() {
        let (name, d) = &s.case.gp[pi];
        assert_eq!(&p.name, name);
        if p.kind == TransportPatchKind::Empty {
            continue;
        }
        let m = d["w"].len();
        seam.add_all(&p.weights[..m], &d["w"]);
        seam.add_all(&p.non_orth_delta[..m], &d["nonOrthDelta"]);
        seam.add_all(&p.mag_sf[..m], &d["magSf"]);
        for i in 0..m {
            seam.add(p.sf[i].x, d["Sf"][3 * i]);
            seam.add(p.sf[i].y, d["Sf"][3 * i + 1]);
            seam.add(p.sf[i].z, d["Sf"][3 * i + 2]);
            seam.add(p.face_cells[i] as f64, d["faceCells"][i]);
        }
    }
    seam.show("patches (cyclic seam and first wall faces): weights, nonOrthDelta, magSf, Sf, faceCells");
    let mut corr = Tally::default();
    for f in 0..n_int {
        corr.add(s.tm.non_orth_corr[f].x, g["nonOrthCorr"][3 * f]);
        corr.add(s.tm.non_orth_corr[f].y, g["nonOrthCorr"][3 * f + 1]);
        corr.add(s.tm.non_orth_corr[f].z, g["nonOrthCorr"][3 * f + 2]);
    }
    corr.show("internal faces: nonOrthCorrectionVectors");
    assert_eq!(corr.equal, corr.n);
    // nonOrthCorrectionVectors are NOT exactly zero (prediction G3 missed): their size is recorded
    let corr_max = g["nonOrthCorr"].iter().fold(0.0_f64, |a, b| a.max(b.abs()));
    println!("  max |nonOrthCorrectionVectors| (internal faces) = {corr_max:e}");
    assert_eq!(t.equal, t.n);
    assert_eq!(vol.equal, vol.n);
    assert_eq!(seam.equal, seam.n);
}

#[test]
fn the_liquid_fuel_class_matches_the_real_class_on_every_variant() {
    let variants: [(&str, AlphaUpdate); 8] = [
        ("flow", AlphaUpdate::Assign),
        ("rev", AlphaUpdate::Assign),
        ("still", AlphaUpdate::Assign),
        ("slow", AlphaUpdate::Assign),
        ("alpha_assign", AlphaUpdate::Assign),
        ("alpha_direct", AlphaUpdate::Direct),
        ("scno", AlphaUpdate::Assign),
        ("noinit", AlphaUpdate::Assign),
    ];
    for (v, upd) in variants {
        let with_matrices = matches!(v, "flow" | "rev" | "still" | "slow" | "alpha_assign");
        let (o, m) = run_variant(v, upd, with_matrices);
        print_outcome(v, &o, m.as_ref());
        assert_identical(v, &o, m.as_ref());
    }
}

#[test]
fn the_matrix_assembler_refuses_a_wrong_length() {
    let s = setup("flow");
    let n = s.n;
    let inp = PrecursorEquationInputs {
        alpha: vec![1.0; n - 1],
        lambda: 0.1,
        su: vec![0.0; n],
        gamma: vec![0.0; n],
        phi: s.tm.flux_of_u_alpha(&vec![Vector3::new(0.0, 0.0, 1.0); n], &vec![1.0; n]).unwrap(),
        ddt: None,
        psi: vec![0.0; n],
        non_orth_correction: true,
    };
    assert!(assemble_precursor_matrix(&s.tm, &inp).is_err());
}

/// Per-step account of the first steps (diagnostic: prints, asserts nothing).
#[test]
#[ignore = "diagnostic: prints, asserts nothing"]
fn the_first_steps_in_detail() {
    for variant in ["flow", "rev", "still"] {
        let s = setup(variant);
        let mut pk = new_pk(&s, AlphaUpdate::Assign);
        for k in 0..4 {
            let r = &s.case.steps[k];
            let fields = pk_fields(s.n, k);
            let fluid = fluid_fields(s.n, k, &s.flow, &s.alpha_mode);
            let rep = pk.step(k + 1, r.t, r.dt, &fields, &fluid).expect("step");
            println!("{variant} step {k}: power {:e} vs {:e} (equal {}); iterations {} vs {}", pk.base().power(), r.k[0], pk.base().power() == r.k[0], rep.neutron_iterations, r.residuals.len());
            for (i, sv) in rep.solves.iter().enumerate() {
                let u = r.solves[i];
                println!("   solve {i}: it {} vs {}; init {:e} vs {:e} (equal {}); final {:e} vs {:e} (equal {})", sv.0, u.0, sv.1, u.1, sv.1 == u.1, sv.2, u.2, sv.2 == u.2);
            }
            for g in 0..8 {
                if let Some(full) = r.ps.get(&g) {
                    let mut t = Tally::default();
                    t.add_all(pk.prec_pk_star(g), full);
                    println!("   precPKStar group {g}: {}/{} equal, worst {:.2e}", t.equal, t.n, t.worst);
                    let bad: Vec<usize> = (0..full.len()).filter(|&i| pk.prec_pk_star(g)[i] != full[i]).collect();
                    if !bad.is_empty() {
                        println!("      differing cells: {bad:?}");
                    }
                }
            }
        }
    }
}

/// Per-array, per-step account of the first-iteration matrix (diagnostic: prints, asserts nothing).
#[test]
#[ignore = "diagnostic: prints, asserts nothing"]
fn the_matrix_arrays_in_detail() {
    for variant in ["flowall", "rev"] {
        let s = setup(variant);
        let mut pk = new_pk(&s, AlphaUpdate::Assign);
        for k in 0..s.case.steps.len() {
            let r = &s.case.steps[k];
            let fields = pk_fields(s.n, k);
            let fluid = fluid_fields(s.n, k, &s.flow, &s.alpha_mode);
            for (&g, m) in &r.matrices {
                let (eqn, phi, diff) = pk.first_iteration_matrix(k + 1, r.dt, &fields, &fluid, g, k == 0).expect("matrix");
                let mut line = format!("{variant} step {k} group {g}:");
                let mut one = |name: &str, a: &[f64], b: &[f64]| {
                    let mut t = Tally::default();
                    t.add_all(a, b);
                    line.push_str(&format!(" {name} {}/{} ({:.1e})", t.equal, t.n, t.worst));
                };
                one("phi", &phi.internal, &r.phi);
                one("diff", &diff, &r.diff);
                one("diag", &eqn.diag, &m.diag);
                one("upper", &eqn.upper, &m.upper);
                one("lower", &eqn.lower, &m.lower);
                one("source", &eqn.source, &m.source);
                for (&p, v) in &m.ints {
                    one(&format!("int{p}"), &eqn.internal_coeffs[p][..v.len()], v);
                }
                for (&p, v) in &m.bous {
                    one(&format!("bou{p}"), &eqn.boundary_coeffs[p][..v.len()], v);
                }
                println!("{line}");
                if let Some(sv) = r.sv.get(&g) {
                    let mut psi = pk.prec_pk_star(g).to_vec();
                    let perf = eqn.solve(&s.tm, &mut psi, 1.0e-6, 1.0e-3, 1000, 0);
                    let mut t = Tally::default();
                    t.add_all(&psi, &sv[3..]);
                    let bad: Vec<usize> = (0..psi.len()).filter(|&i| psi[i] != sv[3 + i]).collect();
                    println!(
                        "     solve alone: iterations {} vs {}, init {:e} vs {:e} (equal {}), final equal {}, solution {}/{} equal, differing cells {:?}",
                        perf.iterations, sv[0], perf.initial_residual, sv[1], perf.initial_residual == sv[1], perf.final_residual == sv[2], t.equal, t.n, bad
                    );
                }
                let bad: Vec<usize> = (0..eqn.source.len()).filter(|&i| eqn.source[i] != m.source[i]).collect();
                for &i in bad.iter().take(6) {
                    println!(
                        "     source cell {i}: port {:e} upstream {:e} (diff {:e}); init_n {:e}; psi_old {:e}",
                        eqn.source[i],
                        m.source[i],
                        eqn.source[i] - m.source[i],
                        pk.init_one_group_flux_n()[i],
                        pk.prec_pk_star(g)[i]
                    );
                }
            }
            let _ = pk.step(k + 1, r.t, r.dt, &fields, &fluid).expect("step");
            if k >= 8 {
                break;
            }
        }
    }
}
