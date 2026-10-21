//! # Auto watcher thresholds: the experiments of gh #927
//!
//! Re-implements the control flow of `MCSettings::Auto`'s watcher
//! (`model/run.rs`) with every threshold a parameter, because the constants
//! there are `const`. Methodology, predictions and results are in
//! `verification_and_validation/auto_watcher_927/README.md`; this file is the
//! instrument. Example and CLI glue, so no unit tests; its one fidelity gate
//! is the `check` command (emulator against the library's own Auto run, bit
//! for bit), whose result is recorded in that README.
//!
//! ```text
//! auto_watcher_927 check   <case>                                 # emulator vs MCSettings::Auto, seed 1, bit for bit
//! auto_watcher_927 record  <case> <seed0> <seed1> <dir> x [cycles] # low-population traces and one production trace (default 250 cycles), CSV
//! auto_watcher_927 analyse <case> <dir> [d_ref]                   # replay the grid, print markdown tables
//! auto_watcher_927 windows <case> <dir> [d_ref] [low_from]        # pass rate of each part of the criterion, stationary vs transient windows
//! auto_watcher_927 live    <case> <seed0> <seed1> [default]       # the same grid, run live (new seeds)
//! ```
//! Run with `RAYON_NUM_THREADS=4 taskset -c 0-3`; the ENDF tapes of
//! `reference-data/endf` are processed on every start (about 75 s).
//! Cases: `godiva`, `pairA` (r_B = 8.2 cm), `pairB` (r_B = 8.6 cm), `hst009`.

use petir::mathf::RealMath;
use std::collections::BTreeMap;
use std::path::PathBuf;

use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::model::run::{
    Progress, AUTO_PARTICLES, ENTROPY_MESH_DIVISIONS, MAX_INACTIVE_CYCLES, SETTLE_CYCLES,
    WATCH_EVERY_CYCLES, WATCH_NORMALITY_ALPHA, WATCH_PARTICLES, WATCH_WINDOW_CYCLES,
};
use outram_mc_libs::model::{AssembledModel, Model};
use outram_mc_libs::physics::keff::KeffSettings as LowLevelKeffSettings;
use outram_mc_libs::physics::transport_csg::CsgPowerIteration;
use outram_mc_libs::stats::convergence::TraceDiagnosis;
use outram_mc_libs::tally::mesh::RegularMesh;
use outram_mc_libs::vv::{godiva, hst009};
use raffles::estimators::mean_and_stderr;
use raffles::estimators::normality::dagostino_pearson;
use uom::si::f64::{Length, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::thermodynamic_temperature::kelvin;

/// Meshes recorded per cycle (divisions per side).
const MESHES: [usize; 4] = [3, 5, 8, 12];
/// Low-population sizes recorded.
const PARTS: [usize; 4] = [500, 1000, 2000, 5000];
/// Cycles recorded at low population: the longest low phase any grid point
/// can use is `MAX_INACTIVE_CYCLES - S - W` = 100 - 0 - 20.
const LOW_CYCLES: usize = 80;
/// Active cycles averaged for the bias.
const ACTIVE: usize = 100;

// ───────────────────────────── the cases ─────────────────────────────────

fn cm(x: f64) -> Length {
    Length::new::<centimeter>(x)
}

fn godiva_material() -> Material {
    Material::from_atom_densities(
        godiva::NUCLIDES
            .iter()
            .map(|(n, _, d)| (Isotope::from_name(n).unwrap(), atoms_per_barn_cm(*d))),
    )
    .named("Godiva HEU")
}

fn build(case: &str, data: NuclearData) -> Model {
    let mut m = Model::new(data);
    match case {
        "godiva" => {
            let c = m.component("Core");
            m.add(
                Sphere::radius(cm(godiva::RADIUS_CM))
                    .filled_with(godiva_material())
                    .in_component(c),
            );
            m.set_temperature(Temperature::Uniform(
                ThermodynamicTemperature::new::<kelvin>(godiva::TEMPERATURE_K),
            ));
        }
        "pairA" | "pairB" => {
            let r_b = if case == "pairA" { 8.2 } else { 8.6 };
            let a = m.component("Sphere A");
            let b = m.component("Sphere B");
            m.add(
                Sphere::radius(cm(godiva::RADIUS_CM))
                    .centred_at(cm(0.0), cm(0.0), cm(0.0))
                    .filled_with(godiva_material())
                    .in_component(a),
            );
            m.add(
                Sphere::radius(cm(r_b))
                    .centred_at(cm(40.0), cm(0.0), cm(0.0))
                    .filled_with(godiva_material())
                    .in_component(b),
            );
            m.set_temperature(Temperature::Uniform(
                ThermodynamicTemperature::new::<kelvin>(godiva::TEMPERATURE_K),
            ));
        }
        "hst009" => {
            let sol_c = m.component("Fuel solution");
            let tank_c = m.component("Tank");
            let wat_c = m.component("Reflector");
            let spec = |mat: usize, name: &str| {
                atoms_per_barn_cm(
                    hst009::MATERIALS[mat]
                        .1
                        .iter()
                        .find(|(n, _)| *n == name)
                        .map_or(0.0, |(_, d)| *d),
                )
            };
            let solution = Material::from_atom_densities([
                (U234, spec(hst009::MAT_SOLUTION, "U234")),
                (U235, spec(hst009::MAT_SOLUTION, "U235")),
                (U236, spec(hst009::MAT_SOLUTION, "U236")),
                (U238, spec(hst009::MAT_SOLUTION, "U238")),
                (F19, spec(hst009::MAT_SOLUTION, "F19")),
                (O16, spec(hst009::MAT_SOLUTION, "O16")),
                (H1, spec(hst009::MAT_SOLUTION, "H1")),
            ])
            .bound_in(Moderator::LightWater)
            .named("uranium oxyfluoride solution");
            let tank = Material::aluminium_1100();
            let water = Material::from_atom_densities([
                (H1, spec(hst009::MAT_WATER, "H1")),
                (O16, spec(hst009::MAT_WATER, "O16")),
            ])
            .bound_in(Moderator::LightWater)
            .named("water reflector");
            let sol = Sphere::radius(cm(hst009::R_SOLUTION))
                .filled_with(solution)
                .in_component(sol_c);
            let tank_ball = Sphere::radius(cm(hst009::R_TANK))
                .filled_with(tank)
                .in_component(tank_c)
                .containing(sol);
            let refl = Sphere::radius(cm(hst009::R_REFLECTOR))
                .filled_with(water)
                .in_component(wat_c)
                .containing(tank_ball);
            m.add(refl);
            m.set_temperature(Temperature::Uniform(
                ThermodynamicTemperature::new::<kelvin>(hst009::TEMPERATURE_K),
            ));
        }
        other => panic!("unknown case {other}"),
    }
    m
}

// ───────────────────────────── the parameters ────────────────────────────

/// The watcher's thresholds.
#[derive(Clone, Copy, Debug, PartialEq)]
struct P {
    w: usize,
    e: usize,
    s: usize,
    mesh: usize,
    part: usize,
    /// 0.0 switches the normality gate off.
    alpha: f64,
}

const DEFAULT: P = P {
    w: WATCH_WINDOW_CYCLES,
    e: WATCH_EVERY_CYCLES,
    s: SETTLE_CYCLES,
    mesh: ENTROPY_MESH_DIVISIONS,
    part: WATCH_PARTICLES,
    alpha: WATCH_NORMALITY_ALPHA,
};

/// The one-at-a-time grid around the default: (label, parameters).
fn grid() -> Vec<(String, P)> {
    let mut g = vec![("default".to_string(), DEFAULT)];
    for w in [20, 50] {
        g.push((format!("W={w}"), P { w, ..DEFAULT }));
    }
    for e in [1, 10] {
        g.push((format!("E={e}"), P { e, ..DEFAULT }));
    }
    for s in [0, 5, 20] {
        g.push((format!("S={s}"), P { s, ..DEFAULT }));
    }
    for mesh in [3, 8, 12] {
        g.push((format!("mesh={mesh}"), P { mesh, ..DEFAULT }));
    }
    for part in [500, 2000, 5000] {
        g.push((format!("P={part}"), P { part, ..DEFAULT }));
    }
    for alpha in [0.0, 0.01, 0.10] {
        g.push((format!("alpha={alpha}"), P { alpha, ..DEFAULT }));
    }
    g
}

// ───────────────────────────── the watcher ───────────────────────────────

/// `run.rs::judge_window` with alpha a parameter: k and H stationary
/// (MSER-5 and Geweke) and k not rejected as normal.
fn passes(k: &[f64], h: &[f64], alpha: f64) -> bool {
    let stationary = TraceDiagnosis::analyse("k", k).converged()
        && TraceDiagnosis::analyse("entropy", h).converged();
    stationary
        && dagostino_pearson(k)
            .ok()
            .is_some_and(|t| t.p_value >= alpha)
}

/// What a watched inactive phase decided.
#[derive(Clone, Debug)]
struct Decision {
    k: Vec<f64>,
    h: Vec<Option<f64>>,
    pops: Vec<usize>,
    low_passed: Option<usize>,
    prod_passed: Option<usize>,
    n_inactive: usize,
}

/// `run.rs::keff_with_ablations`' three phases, step for step. `step(cycle,
/// production)` returns k and H of the next cycle (`cycle` is the zero-based
/// index of the cycle about to run); `switch` raises the population.
fn watch(
    p: &P,
    mut step: impl FnMut(usize, bool) -> (f64, Option<f64>),
    mut switch: impl FnMut(),
) -> Decision {
    let mut d = Decision {
        k: Vec::new(),
        h: Vec::new(),
        pops: Vec::new(),
        low_passed: None,
        prod_passed: None,
        n_inactive: 0,
    };
    let window = |d: &Decision, from: usize| -> Option<(Vec<f64>, Vec<f64>)> {
        let k = d.k.get(from..)?.to_vec();
        let h: Option<Vec<f64>> = d.h.get(from..)?.iter().copied().collect();
        Some((k, h?))
    };
    let low_cap = MAX_INACTIVE_CYCLES.saturating_sub(p.s + p.w);
    while d.k.len() < low_cap {
        let (k, h) = step(d.k.len(), false);
        d.k.push(k);
        d.h.push(h);
        d.pops.push(p.part);
        let c = d.k.len();
        if c >= p.w && c % p.e == 0 {
            if let Some((k, h)) = window(&d, c - p.w) {
                if passes(&k, &h, p.alpha) {
                    d.low_passed = Some(c);
                    break;
                }
            }
        }
    }
    switch();
    for _ in 0..p.s {
        let (k, h) = step(d.k.len(), true);
        d.k.push(k);
        d.h.push(h);
        d.pops.push(AUTO_PARTICLES);
    }
    let start = d.k.len();
    while d.k.len() < MAX_INACTIVE_CYCLES {
        let (k, h) = step(d.k.len(), true);
        d.k.push(k);
        d.h.push(h);
        d.pops.push(AUTO_PARTICLES);
        let c = d.k.len() - start;
        if c >= p.w && c % p.e == 0 {
            if let Some((k, h)) = window(&d, d.k.len() - p.w) {
                if passes(&k, &h, p.alpha) {
                    d.prod_passed = Some(d.k.len());
                    break;
                }
            }
        }
    }
    d.n_inactive = d.k.len();
    d
}

// ───────────────────────────── the transport ─────────────────────────────

fn mesh_of(a: &AssembledModel, n: usize) -> RegularMesh {
    let (l, u) = (a.source_box.lower, a.source_box.upper);
    RegularMesh {
        lower_left: [l.x, l.y, l.z],
        upper_right: [u.x, u.y, u.z],
        dimension: [n; 3],
    }
}

/// A power iteration at `part` per cycle, as the Auto run builds it.
fn iteration(a: &AssembledModel, nuc: &[Nuclide], part: usize, seed: u64) -> CsgPowerIteration {
    let low = LowLevelKeffSettings {
        n_particles: part,
        n_inactive: 0,
        // Only a generation budget: the iteration stops being steppable once
        // `n_inactive + n_active` generations are done. Large enough for the
        // longest recorded production trace.
        n_active: 2_000,
        temperature_k: a.temperature_k,
        seed,
        ..LowLevelKeffSettings::default()
    };
    CsgPowerIteration::new(&a.geometry, &a.materials, nuc, a.source_box, &low)
        .with_majorants(a.majorants(nuc))
}

fn step_once(it: &mut CsgPowerIteration, a: &AssembledModel, nuc: &[Nuclide]) -> f64 {
    let (r, _) = it
        .step_par(&a.geometry, &a.materials, nuc, None)
        .expect("population died");
    assert!(r.bank_size > 0, "population died");
    r.k
}

/// A fixed-population trace: k and the entropy on each of [`MESHES`].
fn trace(
    a: &AssembledModel,
    nuc: &[Nuclide],
    part: usize,
    cycles: usize,
    seed: u64,
) -> (Vec<f64>, Vec<Vec<f64>>) {
    let mut it = iteration(a, nuc, part, seed);
    let meshes: Vec<RegularMesh> = MESHES.iter().map(|&n| mesh_of(a, n)).collect();
    let mut k = Vec::new();
    let mut h = vec![Vec::new(); MESHES.len()];
    for _ in 0..cycles {
        k.push(step_once(&mut it, a, nuc));
        for (i, m) in meshes.iter().enumerate() {
            h[i].push(it.source_entropy(m).unwrap_or(f64::NAN));
        }
    }
    (k, h)
}

/// The whole Auto run, live, at thresholds `p`, with `active` active cycles.
fn live_run(
    a: &AssembledModel,
    nuc: &[Nuclide],
    p: &P,
    seed: u64,
    active: usize,
) -> (Decision, Vec<f64>) {
    let mut it = iteration(a, nuc, p.part, seed);
    let mesh = mesh_of(a, p.mesh);
    let cell = std::cell::RefCell::new(&mut it);
    let d = watch(
        p,
        |_, _| {
            let mut it = cell.borrow_mut();
            let k = step_once(&mut it, a, nuc);
            (k, it.source_entropy(&mesh))
        },
        || cell.borrow_mut().set_population(AUTO_PARTICLES),
    );
    let mut act = Vec::new();
    for _ in 0..active {
        act.push(step_once(&mut cell.borrow_mut(), a, nuc));
    }
    (d, act)
}

// ───────────────────────────── replay ────────────────────────────────────

/// A recorded trace.
#[derive(Clone, Default)]
struct Rec {
    k: Vec<f64>,
    h: BTreeMap<usize, Vec<f64>>,
}

struct Outcome {
    n_inactive: usize,
    low_passed: Option<usize>,
    prod_passed: Option<usize>,
    cost: f64,
    k_active: f64,
}

fn replay(low: &Rec, prod: &Rec, p: &P) -> Outcome {
    let h_of = |r: &Rec, c: usize| {
        let x = r.h[&p.mesh][c];
        x.is_finite().then_some(x)
    };
    let d = watch(
        p,
        |c, production| {
            let r = if production { prod } else { low };
            (r.k[c], h_of(r, c))
        },
        || {},
    );
    let n = d.n_inactive;
    let (k_active, _) = mean_and_stderr(&prod.k[n..n + ACTIVE]);
    Outcome {
        n_inactive: n,
        low_passed: d.low_passed,
        prod_passed: d.prod_passed,
        cost: d.pops.iter().sum::<usize>() as f64,
        k_active,
    }
}

fn read_csv(path: &std::path::Path) -> (BTreeMap<usize, Rec>, Rec) {
    let text = std::fs::read_to_string(path).unwrap();
    let mut low: BTreeMap<usize, Rec> = BTreeMap::new();
    let mut prod = Rec::default();
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        let part: usize = f[1].parse().unwrap();
        let k: f64 = f[3].parse().unwrap();
        let rec = if f[0] == "prod" {
            &mut prod
        } else {
            low.entry(part).or_default()
        };
        rec.k.push(k);
        for (i, m) in MESHES.iter().enumerate() {
            rec.h.entry(*m).or_default().push(f[4 + i].parse().unwrap());
        }
    }
    (low, prod)
}

fn write_csv(
    path: &std::path::Path,
    low: &BTreeMap<usize, (Vec<f64>, Vec<Vec<f64>>)>,
    prod: &(Vec<f64>, Vec<Vec<f64>>),
) {
    let mut s = String::from("kind,part,cycle,k,h3,h5,h8,h12\n");
    let mut put = |kind: &str, part: usize, t: &(Vec<f64>, Vec<Vec<f64>>)| {
        for c in 0..t.0.len() {
            s += &format!("{kind},{part},{c},{:.17e}", t.0[c]);
            for h in &t.1 {
                s += &format!(",{:.17e}", h[c]);
            }
            s += "\n";
        }
    };
    for (part, t) in low {
        put("low", *part, t);
    }
    put("prod", AUTO_PARTICLES, prod);
    std::fs::write(path, s).unwrap();
}

// ───────────────────────────── statistics ────────────────────────────────

fn mean(x: &[f64]) -> f64 {
    x.iter().sum::<f64>() / x.len() as f64
}
fn sd(x: &[f64]) -> f64 {
    if x.len() < 2 {
        return f64::NAN;
    }
    let m = mean(x);
    (x.iter().map(|v| (v - m).r_powi(2)).sum::<f64>() / (x.len() - 1) as f64).sqrt()
}
fn median(x: &[f64]) -> f64 {
    let mut v = x.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// The reference: converged levels and the first cycle after which the
/// ensemble-mean trace stays within `tol` of `level`, in 5-cycle blocks.
fn c_true(ens_mean: &[f64], level: f64, tol: f64, d_ref: usize) -> usize {
    let blocks = d_ref / 5;
    let mut first_ok = 0;
    for j in 0..blocks {
        let b = mean(&ens_mean[5 * j..5 * j + 5]);
        if (b - level).abs() >= tol {
            first_ok = 5 * (j + 1);
        }
    }
    first_ok
}

struct Reference {
    d_ref: usize,
    k_ref: f64,
    /// sd across seeds of the ACTIVE-cycle mean taken from d_ref.
    sigma_run: f64,
    /// Per mesh: (H_inf, tau, c_true, slope of H-bar over the reference
    /// range in tau per 100 cycles).
    h: BTreeMap<usize, (f64, f64, usize, f64)>,
    c_true_k: usize,
}

fn reference(prods: &[Rec], d_ref: usize) -> Reference {
    let n = prods[0].k.len();
    let s = prods.len();
    let kbar: Vec<f64> = (0..n)
        .map(|c| mean(&prods.iter().map(|r| r.k[c]).collect::<Vec<_>>()))
        .collect();
    let k_ref = mean(&kbar[d_ref..]);
    let seed_means: Vec<f64> = prods
        .iter()
        .map(|r| mean(&r.k[d_ref..d_ref + ACTIVE]))
        .collect();
    let sigma_run = sd(&seed_means);
    let mut h = BTreeMap::new();
    for &m in &MESHES {
        let hbar: Vec<f64> = (0..n)
            .map(|c| mean(&prods.iter().map(|r| r.h[&m][c]).collect::<Vec<_>>()))
            .collect();
        let level = mean(&hbar[d_ref..]);
        // within-run per-cycle sd, pooled over seeds
        let var: f64 = prods
            .iter()
            .map(|r| sd(&r.h[&m][d_ref..]).r_powi(2))
            .sum::<f64>()
            / s as f64;
        let tau = var.sqrt();
        // slope of hbar over the reference range, least squares
        let xs: Vec<f64> = (d_ref..n).map(|c| c as f64).collect();
        let (mx, my) = (mean(&xs), mean(&hbar[d_ref..]));
        let sxy: f64 = xs
            .iter()
            .zip(&hbar[d_ref..])
            .map(|(x, y)| (x - mx) * (y - my))
            .sum();
        let sxx: f64 = xs.iter().map(|x| (x - mx).r_powi(2)).sum();
        let slope = sxy / sxx * 100.0 / tau;
        h.insert(m, (level, tau, c_true(&hbar, level, tau, d_ref), slope));
    }
    let c_true_k = c_true(&kbar, k_ref, sigma_run, d_ref);
    Reference {
        d_ref,
        k_ref,
        sigma_run,
        h,
        c_true_k,
    }
}

// ───────────────────────────── commands ──────────────────────────────────

fn load(case: &str) -> (AssembledModel, Vec<Nuclide>) {
    let data = NuclearData::workspace_reference().expect("reference-data/endf");
    let model = build(case, data);
    let a = model.assemble().expect("assemble");
    let nuc = a
        .load_nuclides_with(model.data(), Progress::Silent)
        .expect("nuclides");
    (a, nuc)
}

fn cmd_check(case: &str) {
    let (a, nuc) = load(case);
    let run = a
        .keff_with(&nuc, &KeffSettings::new(MCSettings::Auto, None).silent())
        .expect("auto run");
    let (d, act) = live_run(&a, &nuc, &DEFAULT, 1, 250);
    let ours_k: Vec<f64> = d.k.iter().chain(act.iter()).copied().collect();
    let same_k = ours_k == run.k_by_cycle;
    let n = d.n_inactive;
    let ours_h: Vec<Option<f64>> = d.h.clone();
    let same_h = ours_h[..] == run.entropy_by_cycle[..n];
    println!(
        "check {case}: Auto inactive {} (low {:?}, production {:?}); emulator inactive {} (low {:?}, production {:?}); k_by_cycle identical: {same_k}; entropy identical over the inactive cycles: {same_h}; Auto k {} ; emulator mean {:.5}",
        run.provenance.inactive_cycles,
        match &run.provenance.convergence {
            outram_mc_libs::model::run::Convergence::Watched { low_population_passed_at, .. } => *low_population_passed_at,
            _ => None,
        },
        match &run.provenance.convergence {
            outram_mc_libs::model::run::Convergence::Watched { production_passed_at, .. } => *production_passed_at,
            _ => None,
        },
        d.n_inactive,
        d.low_passed,
        d.prod_passed,
        run.keff(),
        mean(&act)
    );
    assert!(same_k && same_h, "emulator differs from MCSettings::Auto");
}

fn cmd_record(case: &str, s0: u64, s1: u64, dir: &str, n_prod: usize) {
    let (a, nuc) = load(case);
    std::fs::create_dir_all(dir).unwrap();
    for seed in s0..s1 {
        let t = std::time::Instant::now();
        let mut low = BTreeMap::new();
        for &part in &PARTS {
            low.insert(part, trace(&a, &nuc, part, LOW_CYCLES, seed));
        }
        let prod = trace(&a, &nuc, AUTO_PARTICLES, n_prod, 100_000 + seed);
        let path = PathBuf::from(dir).join(format!("{case}_s{seed}.csv"));
        write_csv(&path, &low, &prod);
        eprintln!("{case} seed {seed}: {:.1}s", t.elapsed().as_secs_f64());
    }
}

fn load_dir(case: &str, dir: &str) -> Vec<(BTreeMap<usize, Rec>, Rec)> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&format!("{case}_s")) && n.ends_with(".csv"))
        })
        .collect();
    files.sort();
    files.iter().map(|p| read_csv(p)).collect()
}

fn pct(k: usize, n: usize) -> String {
    // Wilson 95 % interval
    let (kf, nf) = (k as f64, n as f64);
    if n == 0 {
        return "-".into();
    }
    let z = 1.96_f64;
    let p = kf / nf;
    let den = 1.0 + z * z / nf;
    let c = (p + z * z / (2.0 * nf)) / den;
    let hw = z * (p * (1.0 - p) / nf + z * z / (4.0 * nf * nf)).sqrt() / den;
    format!(
        "{k}/{n} [{:.0}-{:.0}%]",
        100.0 * (c - hw).max(0.0),
        100.0 * (c + hw).min(1.0)
    )
}

fn cmd_analyse(case: &str, dir: &str, d_ref: usize) {
    let recs = load_dir(case, dir);
    let prods: Vec<Rec> = recs.iter().map(|r| r.1.clone()).collect();
    let s = prods.len();
    let r = reference(&prods, d_ref);
    println!(
        "### {case}: reference from {s} seeds, production traces of {} cycles, D_ref = {}",
        prods[0].k.len(),
        r.d_ref
    );
    println!(
        "k_ref = {:.5} (sd of a {ACTIVE}-cycle active mean across seeds, sigma_run = {:.0} pcm); c_true(k) = {} cycles",
        r.k_ref,
        r.sigma_run * 1e5,
        r.c_true_k
    );
    println!("\n| mesh | H_inf [bits] | tau (per-cycle sd) | slope of H-bar over reference range [tau/100 cycles] | c_true(H) [cycles] |");
    println!("|---|---|---|---|---|");
    for (m, (lvl, tau, ct, slope)) in &r.h {
        println!("| {m}^3 | {lvl:.4} | {tau:.4} | {slope:+.3} | {ct} |");
    }
    let ct_default = r.h[&DEFAULT.mesh].2;
    println!("\nOne threshold at a time (default row first). Bias = mean of k over {ACTIVE} active cycles from n_in, minus k_ref.");
    println!("false pass = production confirmation declared AND n_in < c_true(H) of the judged mesh; never passes = hit the {MAX_INACTIVE_CYCLES}-cycle cap.\n");
    println!("| setting | n_in median [min-max] | mean n_in | never passes | false pass | inactive cost [Mhist] | baseline [Mhist] | bias [pcm] | bias/sigma_run |");
    println!("|---|---|---|---|---|---|---|---|---|");
    for (label, p) in grid() {
        let ct = r.h[&p.mesh].2;
        let mut n_in = Vec::new();
        let mut never = 0;
        let mut fp = 0;
        let mut declared = 0;
        let mut cost = Vec::new();
        let mut bias = Vec::new();
        for (low, prod) in &recs {
            let o = replay(&low[&p.part], prod, &p);
            n_in.push(o.n_inactive as f64);
            cost.push(o.cost);
            bias.push((o.k_active - r.k_ref) * 1e5);
            if o.prod_passed.is_none() {
                never += 1;
            } else {
                declared += 1;
                if o.n_inactive < ct {
                    fp += 1;
                }
            }
            let _ = o.low_passed;
        }
        let (bm, bs) = (mean(&bias), sd(&bias) / (s as f64).sqrt());
        println!(
            "| {label} | {:.0} [{:.0}-{:.0}] | {:.1} | {} | {} | {:.3} | {:.3} | {:+.1} ± {:.1} | {:+.2} |",
            median(&n_in),
            n_in.iter().cloned().fold(f64::MAX, f64::min),
            n_in.iter().cloned().fold(f64::MIN, f64::max),
            mean(&n_in),
            pct(never, s),
            pct(fp, declared),
            mean(&cost) / 1e6,
            (AUTO_PARTICLES * ct) as f64 / 1e6,
            bm,
            bs,
            bm / (r.sigma_run * 1e5)
        );
    }
    println!("\n(default mesh c_true = {ct_default})");
}

/// Per-window pass rate of each part of the criterion on windows taken from
/// cycles where the source is stationary by construction (`d_ref` onward of
/// the production traces, `low_from` onward of the low-population traces):
/// the watcher's false-rejection rate (type I error). Windows start every
/// 5 cycles, so they overlap and are correlated; the count is not the
/// number of independent windows.
fn cmd_windows(case: &str, dir: &str, d_ref: usize, low_from: usize) {
    let recs = load_dir(case, dir);
    println!(
        "### {case}: pass rate of each part of the window criterion on stationary windows ({} seeds)\n",
        recs.len()
    );
    println!("| trace | start | W | windows | k MSER in first half | k Geweke no drift | k both | H(5^3) both | k and H stationary | normality p>=.05 | normality p>=.01 | all (alpha .05) |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|");
    let row = |label: String, from: usize, until: usize, w: usize, traces: Vec<&Rec>| {
        let (mut n, mut m1, mut g1, mut kb, mut hb, mut st, mut n05, mut n01, mut all) =
            (0, 0, 0, 0, 0, 0, 0, 0, 0);
        for r in traces {
            let len = r.k.len();
            let mut a = from;
            while a + w <= len.min(until) {
                let k = &r.k[a..a + w];
                let h = &r.h[&DEFAULT.mesh][a..a + w];
                let dk = TraceDiagnosis::analyse("k", k);
                let dh = TraceDiagnosis::analyse("entropy", h);
                n += 1;
                m1 += dk.mser.as_ref().is_some_and(|m| m.in_first_half) as usize;
                g1 += dk
                    .geweke_after_truncation
                    .as_ref()
                    .is_some_and(|g| !g.drift) as usize;
                kb += dk.converged() as usize;
                hb += dh.converged() as usize;
                let stat = dk.converged() && dh.converged();
                st += stat as usize;
                let p = dagostino_pearson(k).ok().map(|t| t.p_value);
                n05 += p.is_some_and(|p| p >= 0.05) as usize;
                n01 += p.is_some_and(|p| p >= 0.01) as usize;
                all += (stat && p.is_some_and(|p| p >= 0.05)) as usize;
                a += 5;
            }
        }
        let f = |x: usize| format!("{:.2}", x as f64 / n.max(1) as f64);
        println!(
            "| {label} | {from} | {w} | {n} | {} | {} | {} | {} | {} | {} | {} | {} |",
            f(m1),
            f(g1),
            f(kb),
            f(hb),
            f(st),
            f(n05),
            f(n01),
            f(all)
        );
    };
    // c_true of the default mesh: windows starting at or after it are
    // stationary (should pass), windows ending at or before it are still in
    // the transient (should fail).
    let prods: Vec<Rec> = recs.iter().map(|r| r.1.clone()).collect();
    let ct = reference(&prods, d_ref).h[&DEFAULT.mesh].2;
    println!("(c_true of the default mesh = {ct})\n");
    for w in [20, 30, 50] {
        row(
            format!("production {AUTO_PARTICLES}, start >= c_true (should pass)"),
            ct.max(5),
            usize::MAX,
            w,
            recs.iter().map(|r| &r.1).collect(),
        );
        if ct >= w {
            row(
                format!("production {AUTO_PARTICLES}, end <= c_true (should FAIL)"),
                0,
                ct,
                w,
                recs.iter().map(|r| &r.1).collect(),
            );
        }
    }
    // The entropy mesh: pass rate of the entropy half alone (MSER-5 and
    // Geweke on H) on stationary windows against windows still in the
    // transient, per mesh, at the default window.
    if ct >= DEFAULT.w {
        println!("\nEntropy half alone, W = {}, by mesh:\n", DEFAULT.w);
        println!("| mesh | stationary windows pass | transient windows pass |");
        println!("|---|---|---|");
        for m in MESHES {
            let rate = |from: usize, until: usize| {
                let (mut n, mut ok) = (0, 0);
                for r in recs.iter().map(|r| &r.1) {
                    let mut a = from;
                    while a + DEFAULT.w <= r.k.len().min(until) {
                        n += 1;
                        ok += TraceDiagnosis::analyse("entropy", &r.h[&m][a..a + DEFAULT.w])
                            .converged() as usize;
                        a += 5;
                    }
                }
                format!("{:.2} ({n})", ok as f64 / n.max(1) as f64)
            };
            println!("| {m}^3 | {} | {} |", rate(ct, usize::MAX), rate(0, ct));
        }
    }
    if ct > low_from {
        return;
    }
    for part in PARTS {
        for w in [20, 30, 50] {
            row(
                format!("low {part}"),
                low_from,
                usize::MAX,
                w,
                recs.iter().map(|r| &r.0[&part]).collect(),
            );
        }
    }
}

fn cmd_live(case: &str, s0: u64, s1: u64, default_only: bool) {
    let (a, nuc) = load(case);
    println!("| setting | n_in median [min-max] | mean n_in | never passes | inactive cost [Mhist] | k active mean (100 cycles) |");
    println!("|---|---|---|---|---|---|");
    for (label, p) in grid()
        .into_iter()
        .take(if default_only { 1 } else { usize::MAX })
    {
        let mut n_in = Vec::new();
        let mut cost = Vec::new();
        let mut ks = Vec::new();
        let mut never = 0;
        for seed in s0..s1 {
            let (d, act) = live_run(&a, &nuc, &p, 200_000 + seed, ACTIVE);
            n_in.push(d.n_inactive as f64);
            cost.push(d.pops.iter().sum::<usize>() as f64);
            ks.push(mean(&act));
            if d.prod_passed.is_none() {
                never += 1;
            }
        }
        println!(
            "| {label} | {:.0} [{:.0}-{:.0}] | {:.1} | {} | {:.3} | {:.5} ± {:.5} |",
            median(&n_in),
            n_in.iter().cloned().fold(f64::MAX, f64::min),
            n_in.iter().cloned().fold(f64::MIN, f64::max),
            mean(&n_in),
            pct(never, n_in.len()),
            mean(&cost) / 1e6,
            mean(&ks),
            sd(&ks) / (ks.len() as f64).sqrt()
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let a = |i: usize| args.get(i).cloned().unwrap_or_default();
    let num = |i: usize| a(i).parse::<u64>().unwrap();
    match a(1).as_str() {
        "check" => cmd_check(&a(2)),
        "record" => {
            let n_prod = args.get(7).and_then(|s| s.parse().ok()).unwrap_or(250);
            cmd_record(&a(2), num(3), num(4), &a(5), n_prod)
        }
        "analyse" => {
            let d_ref = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(150);
            cmd_analyse(&a(2), &a(3), d_ref)
        }
        "windows" => {
            let d_ref = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(150);
            let low_from = args.get(5).and_then(|s| s.parse().ok()).unwrap_or(20);
            cmd_windows(&a(2), &a(3), d_ref, low_from)
        }
        "live" => cmd_live(&a(2), num(3), num(4), a(5) == "default"),
        _ => eprintln!("see the module docs"),
    }
}
