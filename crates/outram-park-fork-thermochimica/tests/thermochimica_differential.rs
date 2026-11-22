//! Differential comparison against a run of Thermochimica 0c35c8d (gh:#1407, epic #1336).
//!
//! Methodology, numbers and the defects found are in
//! `verification_and_validation/thermochimica_differential/` (`README.md`,
//! `PREDICTIONS.md` written before the runs, `RESULTS.md`). The reference
//! `thermochimica_reference.txt.gz` is the output of `tc_driver.F90` linked
//! against the unmodified upstream library (BSD-3-Clause, ORNL-CEES/thermochimica)
//! on 15 cases from the ChemSage databases shipped in upstream `data/`, once at
//! upstream's default tolerance (`dTolerance(1) = 1e-5`) and once tightened to 1e-10.
//!
//! The crate has no ChemSage parser and no Gibbs-polynomial evaluator, so this test
//! feeds the port what upstream itself evaluated (dimensionless `g0/RT`, atom matrix,
//! phase membership, RKMP parameters) and compares the converged equilibrium:
//! assemblage, species moles, element potentials and `G/RT`. Cases with a phase model
//! the port lacks (ternary/quaternary RKMP terms) are reported as out of reach, not
//! skipped silently.
//!
//! Set `TC_DIFF_PRINT=1` and pass `--nocapture` to print the tables.

use std::collections::BTreeSet;

use miniz_oxide::inflate::decompress_to_vec;
use outram_park_fork_thermochimica::gem::{
    BinaryInteraction, GemError, GemOptions, GemResult, GemSystem, PhaseInput, SolutionModel, R,
};

const REFERENCE_GZ: &[u8] = include_bytes!(
    "../verification_and_validation/thermochimica_differential/thermochimica_reference.txt.gz"
);

/// Upstream `dIdealConstant` (`setup/InitThermo.f90:64`), \[J/(mol K)\].
const R_UPSTREAM: f64 = 8.314_472;

const CASE_NAMES: [&str; 15] = [
    "C1", "C2", "C3", "C4", "H1", "H2", "H3", "H4", "W1", "W2", "W3", "W4", "S1", "S2", "S3",
];

fn print_enabled() -> bool {
    std::env::var("TC_DIFF_PRINT").is_ok()
}

fn gunzip(bytes: &[u8]) -> String {
    assert!(bytes.len() > 18 && bytes[0] == 0x1f && bytes[1] == 0x8b && bytes[3] == 0);
    let raw = decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate");
    String::from_utf8(raw).expect("utf8")
}

fn f(s: &str) -> f64 {
    s.parse::<f64>().expect("reference value parses as f64")
}

// ---------------------------------------------------------------- reference

#[derive(Debug, Clone)]
struct RefSpecies {
    name: String,
    particles_per_mole: i64,
    g0rt: f64,
    n: f64,
    stoich: Vec<f64>,
}

#[derive(Debug, Clone)]
struct RefPhase {
    name: String,
    ty: String,
    first: usize,
    last: usize,
}

#[derive(Debug, Clone)]
struct RefRkmp {
    phase: usize,
    ty: i64,
    i: usize,
    j: usize,
    exponent: usize,
    l_rt: f64,
}

#[derive(Debug, Clone, Default)]
struct RefCase {
    mode: String,
    id: usize,
    info: i64,
    iter_global: usize,
    t: f64,
    p_atm: f64,
    r_up: f64,
    gibbs_j: f64,
    elem_pi: Vec<f64>,
    elem_b: Vec<f64>,
    assem_con: Vec<(usize, f64)>,
    assem_soln: Vec<(usize, f64)>,
    phases: Vec<RefPhase>,
    con_first: usize,
    con_last: usize,
    species: Vec<RefSpecies>,
    rkmp: Vec<RefRkmp>,
}

fn after<'a>(line: &'a str, key: &str) -> &'a str {
    let at = line.find(key).unwrap_or_else(|| panic!("`{key}` in `{line}`"));
    &line[at + key.len()..]
}

fn parse_reference(text: &str) -> Vec<RefCase> {
    let mut cases: Vec<RefCase> = Vec::new();
    let mut mode = String::new();
    for line in text.lines() {
        if let Some(m) = line.strip_prefix("#mode ") {
            mode = m.trim().to_string();
            continue;
        }
        let tok: Vec<&str> = line.split_whitespace().collect();
        if tok.is_empty() {
            continue;
        }
        if tok[0] == "case" {
            cases.push(RefCase {
                mode: mode.clone(),
                id: tok[1].parse().unwrap(),
                info: tok[3].parse().unwrap(),
                iter_global: tok[5].parse().unwrap(),
                ..RefCase::default()
            });
            continue;
        }
        let c = cases.last_mut().expect("record before first case");
        match tok[0] {
            "cond" => {
                c.t = f(tok[2]);
                c.p_atm = f(tok[4]);
                c.r_up = f(tok[6]);
            }
            "gibbs_J" => c.gibbs_j = f(tok[1]),
            "elem" => {
                c.elem_b.push(f(tok[5]));
                c.elem_pi.push(f(tok[7]));
            }
            "assem_con" => c.assem_con.push((tok[3].parse().unwrap(), f(tok[5]))),
            "assem_soln" => c.assem_soln.push((tok[3].parse().unwrap(), f(tok[5]))),
            "phase" => {
                let name = after(line, " name ");
                let name_end = name.find(" type ").unwrap();
                let rest = after(line, " type ");
                let r: Vec<&str> = rest.split_whitespace().collect();
                c.phases.push(RefPhase {
                    name: name[..name_end].trim().to_string(),
                    ty: r[0].to_string(),
                    first: r[2].parse().unwrap(),
                    last: r[4].parse().unwrap(),
                });
            }
            "conspecies" => {
                c.con_first = tok[2].parse().unwrap();
                c.con_last = tok[4].parse().unwrap();
            }
            "species" => {
                let name = after(line, " name ");
                let name_end = name.find(" ppm ").unwrap();
                let r: Vec<&str> = after(line, " ppm ").split_whitespace().collect();
                c.species.push(RefSpecies {
                    name: name[..name_end].trim().to_string(),
                    particles_per_mole: r[0].parse().unwrap(),
                    g0rt: f(r[2]),
                    n: f(r[6]),
                    stoich: Vec::new(),
                });
            }
            "stoich" => {
                let sp = c.species.last_mut().expect("stoich after species");
                sp.stoich = tok[2..].iter().map(|v| f(v)).collect();
            }
            "rkmp" => {
                // rkmp phase P type T i I j J e4 E4 e5 E5 L value
                let ty: i64 = tok[4].parse().unwrap();
                c.rkmp.push(RefRkmp {
                    phase: tok[2].parse().unwrap(),
                    ty,
                    i: tok[6].parse().unwrap(),
                    j: tok[8].parse().unwrap(),
                    exponent: tok[10].parse().unwrap(),
                    l_rt: f(tok[14]),
                });
            }
            _ => {}
        }
    }
    cases
}

// ---------------------------------------------------------------- problem

/// A reference case translated into this crate's inputs.
struct Problem {
    system: GemSystem,
    /// `g0` per phase and species \[J/mol\].
    gibbs: Vec<Vec<f64>>,
    feed: Vec<Vec<f64>>,
    /// Upstream 1-based species index for each (phase, species).
    upstream_index: Vec<Vec<usize>>,
    /// Label of each phase, `soln:<upstream phase index>` or `con:<upstream species index>`.
    labels: Vec<String>,
}

fn is_dummy(name: &str) -> bool {
    name.ends_with('#')
}

/// Why a case cannot be expressed with this crate's phase models, or the problem.
///
/// `r_for_energy` is the gas constant used to turn the dimensionless upstream
/// energies into J/mol: this crate's own `R` (mode A, same dimensionless problem) or
/// upstream's (mode B, literal J/mol).
fn build_problem(
    c: &RefCase,
    r_for_energy: f64,
    restrict: Option<&BTreeSet<String>>,
) -> Result<Problem, String> {
    let n_el = c.elem_pi.len();
    let elements: Vec<String> = (0..n_el).map(|k| format!("e{k}")).collect();
    let el_refs: Vec<&str> = elements.iter().map(String::as_str).collect();
    // Upstream adds ln(P/atm) and R-normalised 0.10945 J/mol/K to the first solution
    // phase's species when it is IDMX (`setup/CompThermoData.f90:454-460`), and stores the
    // sum in the standard energy. The pressure term is removed here so the port's own
    // `ln(P/P0)` is exercised; the constant 0.10945/R (atm to bar standard state) stays
    // as part of the supplied data.
    let ln_p = c.p_atm.ln();

    let mut phases: Vec<PhaseInput> = Vec::new();
    let mut gibbs: Vec<Vec<f64>> = Vec::new();
    let mut upstream_index: Vec<Vec<usize>> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    let rt = r_for_energy * c.t;

    for (ip0, ph) in c.phases.iter().enumerate() {
        let ip = ip0 + 1;
        let ids: Vec<usize> = (ph.first..=ph.last).collect();
        let is_gas = ip == 1 && ph.ty == "IDMX";
        let model = match ph.ty.as_str() {
            "IDMX" if is_gas => SolutionModel::IdealGas,
            "IDMX" => SolutionModel::IdealSolution,
            "RKMP" => {
                let mine: Vec<&RefRkmp> = c.rkmp.iter().filter(|r| r.phase == ip).collect();
                if let Some(bad) = mine.iter().find(|r| r.ty != 2) {
                    return Err(format!(
                        "phase `{}` has RKMP parameters of type {} (ternary/quaternary Muggianu terms): not ported",
                        ph.name, bad.ty
                    ));
                }
                let mut pairs: Vec<(usize, usize)> = Vec::new();
                for r in &mine {
                    if !pairs.contains(&(r.i, r.j)) {
                        pairs.push((r.i, r.j));
                    }
                }
                let interactions = pairs
                    .iter()
                    .map(|&(i, j)| {
                        let mut l: Vec<f64> = Vec::new();
                        for r in mine.iter().filter(|r| (r.i, r.j) == (i, j)) {
                            if l.len() <= r.exponent {
                                l.resize(r.exponent + 1, 0.0);
                            }
                            l[r.exponent] = r.l_rt * rt;
                        }
                        BinaryInteraction {
                            species_i: i - 1,
                            species_j: j - 1,
                            l_coeffs: l,
                        }
                    })
                    .collect();
                SolutionModel::RedlichKister { interactions }
            }
            other => return Err(format!("phase type `{other}` not ported")),
        };
        let mut atoms = vec![Vec::new(); n_el];
        let mut g = Vec::new();
        let mut names = Vec::new();
        for &id in &ids {
            let sp = &c.species[id - 1];
            if sp.particles_per_mole != 1 {
                return Err(format!("species `{}` has {} particles per mole", sp.name, sp.particles_per_mole));
            }
            for (k, row) in atoms.iter_mut().enumerate() {
                row.push(sp.stoich[k]);
            }
            let g0rt = if is_gas {
                sp.g0rt - ln_p
            } else {
                sp.g0rt
            };
            g.push(g0rt * rt);
            names.push(sp.name.clone());
        }
        phases.push(PhaseInput {
            model,
            species_names: names,
            atom_matrix: atoms,
        });
        gibbs.push(g);
        upstream_index.push(ids);
        labels.push(format!("soln:{ip}"));
    }
    for id in c.con_first..=c.con_last {
        let sp = &c.species[id - 1];
        if is_dummy(&sp.name) {
            continue;
        }
        phases.push(PhaseInput {
            model: SolutionModel::IdealSolution,
            species_names: vec![sp.name.clone()],
            atom_matrix: (0..n_el).map(|k| vec![sp.stoich[k]]).collect(),
        });
        gibbs.push(vec![sp.g0rt * rt]);
        upstream_index.push(vec![id]);
        labels.push(format!("con:{id}"));
    }

    // Candidate reduction, from the supplied data only (never from the upstream answer):
    // (1) `restrict` keeps only the listed phases (the "given assemblage" mode);
    // (2) otherwise, of several pure condensed phases with the same atom vector
    // (graphite / diamond) only the one with the lowest g0 is kept, because the port has no
    // assemblage selection and two phases of identical composition make its saddle
    // system singular.
    let mut keep: Vec<bool> = labels.iter().map(|l| restrict.map_or(true, |r| r.contains(l))).collect();
    if restrict.is_none() {
        for a in 0..phases.len() {
            for b in 0..phases.len() {
                if a != b
                    && labels[a].starts_with("con:")
                    && labels[b].starts_with("con:")
                    && phases[a].atom_matrix == phases[b].atom_matrix
                    && (gibbs[b][0], b) < (gibbs[a][0], a)
                {
                    keep[a] = false;
                }
            }
        }
    }
    let mut it = keep.iter();
    phases.retain(|_| *it.next().unwrap());
    let mut it = keep.iter();
    gibbs.retain(|_| *it.next().unwrap());
    let mut it = keep.iter();
    upstream_index.retain(|_| *it.next().unwrap());
    let mut it = keep.iter();
    labels.retain(|_| *it.next().unwrap());

    // Feed: element moles (upstream `dMolesElement`, equal to the input feed), placed on the
    // single-element species with the lowest g0 per atom (a deterministic, data-only choice
    // of starting species; the port seeds its iterate from the feed).
    let b: Vec<f64> = c.elem_b.clone();
    let mut feed: Vec<Vec<f64>> = phases.iter().map(|p| vec![0.0; p.species_names.len()]).collect();
    for (k, bk) in b.iter().enumerate() {
        let mut best: Option<(f64, usize, usize)> = None;
        for (p, ph) in phases.iter().enumerate() {
            for s in 0..ph.species_names.len() {
                let a_k = ph.atom_matrix[k][s];
                let only_k = a_k > 0.0 && (0..n_el).all(|m| m == k || ph.atom_matrix[m][s] == 0.0);
                if only_k {
                    let g_per_atom = gibbs[p][s] / a_k;
                    if best.map_or(true, |(g, _, _)| g_per_atom < g) {
                        best = Some((g_per_atom, p, s));
                    }
                }
            }
        }
        match best {
            Some((_, p, s)) => feed[p][s] = *bk / phases[p].atom_matrix[k][s],
            None => return Err(format!("no single-element species to seed element {k}")),
        }
    }

    let system = GemSystem::new(&el_refs, &phases).map_err(|e| format!("system: {e}"))?;
    Ok(Problem {
        system,
        gibbs,
        feed,
        upstream_index,
        labels,
    })
}

fn solve(p: &Problem, c: &RefCase, opts: &GemOptions) -> Result<GemResult, GemError> {
    let g: Vec<&[f64]> = p.gibbs.iter().map(Vec::as_slice).collect();
    let feed: Vec<&[f64]> = p.feed.iter().map(Vec::as_slice).collect();
    p.system.minimize(&g, c.t, &feed, c.p_atm, 1.0, opts)
}

// ---------------------------------------------------------------- metrics

const PRESENT_MOLES: f64 = 1e-9;
const MAJOR_MOLES: f64 = 1e-3;

#[derive(Debug, Clone, Default)]
struct Metrics {
    assemblage_match: bool,
    rust_assemblage: BTreeSet<String>,
    upstream_assemblage: BTreeSet<String>,
    max_rel_pi: f64,
    max_rel_major_n: f64,
    max_abs_minor_n: f64,
    rel_g_rt: f64,
    compared: usize,
    bit_identical: usize,
    upstream_iterations: usize,
    rust_iterations: usize,
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(f64::MIN_POSITIVE)
}

fn compare(p: &Problem, c: &RefCase, r: &GemResult) -> Metrics {
    let mut m = Metrics {
        upstream_iterations: c.iter_global,
        rust_iterations: r.iterations,
        ..Metrics::default()
    };
    let tally = |a: f64, b: f64, m: &mut Metrics| {
        m.compared += 1;
        if a == b {
            m.bit_identical += 1;
        }
    };
    // assemblage
    for (idx, label) in p.labels.iter().enumerate() {
        if r.phase_totals[idx] > PRESENT_MOLES {
            m.rust_assemblage.insert(label.clone());
        }
    }
    m.upstream_assemblage = upstream_assemblage(c);
    m.assemblage_match = m.rust_assemblage == m.upstream_assemblage;
    // element potentials
    for (k, pi_up) in c.elem_pi.iter().enumerate() {
        m.max_rel_pi = m.max_rel_pi.max(rel(r.element_potentials[k], *pi_up));
        tally(r.element_potentials[k], *pi_up, &mut m);
    }
    // species moles
    for (ip, ids) in p.upstream_index.iter().enumerate() {
        for (is, &id) in ids.iter().enumerate() {
            let n_up = c.species[id - 1].n;
            let n_rs = r.moles[ip][is];
            if n_up >= MAJOR_MOLES {
                m.max_rel_major_n = m.max_rel_major_n.max(rel(n_rs, n_up));
                tally(n_rs, n_up, &mut m);
            } else {
                m.max_abs_minor_n = m.max_abs_minor_n.max((n_rs - n_up).abs());
            }
        }
    }
    let g_up = c.gibbs_j / (c.r_up * c.t);
    m.rel_g_rt = rel(r.gibbs_energy_rt, g_up);
    tally(r.gibbs_energy_rt, g_up, &mut m);
    m
}

#[derive(Debug)]
enum Outcome {
    OutOfReach(String),
    SolveFailed(String),
    /// Solved; the index into [`LADDER`] of the first option set that converged.
    Compared(usize, Metrics),
}

/// Option sets tried in order; the first that converges is the one compared. Fixed
/// before the final tables were read and applied identically to every case, and the
/// rung is always reported. Rung 0 is what a user gets from `GemOptions::default()`.
/// (`tol`, `mole_floor`, `phase_floor`)
const LADDER: [(f64, f64, f64); 4] = [
    (1e-10, 1e-12, 1e-11),
    (1e-8, 1e-12, 1e-11),
    (1e-10, 1e-50, 1e-50),
    (1e-8, 1e-50, 1e-50),
];

fn options_for(rung: usize) -> GemOptions {
    let (tol, mole_floor, phase_floor) = LADDER[rung];
    GemOptions {
        tol,
        mole_floor,
        phase_floor,
        ..GemOptions::default()
    }
}

fn upstream_assemblage(c: &RefCase) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for (sp, mol) in &c.assem_con {
        if *mol > PRESENT_MOLES {
            set.insert(format!("con:{sp}"));
        }
    }
    for (ph, mol) in &c.assem_soln {
        if *mol > PRESENT_MOLES {
            set.insert(format!("soln:{ph}"));
        }
    }
    set
}

fn run(c: &RefCase, r_for_energy: f64, given: bool) -> Outcome {
    if c.info != 0 {
        return Outcome::OutOfReach(format!("upstream INFO {}", c.info));
    }
    if c.iter_global >= 1000 {
        return Outcome::OutOfReach(format!(
            "reference itself did not converge cleanly (iterGlobal {}, upstream cap 1000+)",
            c.iter_global
        ));
    }
    let assembled = upstream_assemblage(c);
    let p = match build_problem(c, r_for_energy, if given { Some(&assembled) } else { None }) {
        Err(e) => return Outcome::OutOfReach(e),
        Ok(p) => p,
    };
    let mut last = String::new();
    for rung in 0..LADDER.len() {
        match solve(&p, c, &options_for(rung)) {
            Ok(r) => return Outcome::Compared(rung, compare(&p, c, &r)),
            Err(e) => last = e.to_string(),
        }
    }
    Outcome::SolveFailed(last)
}

fn describe(o: &Outcome) -> String {
    match o {
        Outcome::OutOfReach(why) => format!("OUT OF REACH: {why}"),
        Outcome::SolveFailed(why) => format!("PORT DID NOT SOLVE (all rungs): {why}"),
        Outcome::Compared(rung, m) => format!(
            "rung {rung} | assem {} | pi {:.2e} | n_major {:.2e} | n_minor_abs {:.2e} | G/RT {:.2e} | bit {}/{} | iters up {} port {}",
            if m.assemblage_match { "same" } else { "DIFF" },
            m.max_rel_pi,
            m.max_rel_major_n,
            m.max_abs_minor_n,
            m.rel_g_rt,
            m.bit_identical,
            m.compared,
            m.upstream_iterations,
            m.rust_iterations,
        ),
    }
}

fn reference() -> Vec<RefCase> {
    parse_reference(&gunzip(REFERENCE_GZ))
}

fn table(mode: &str, r_for_energy: f64, given: bool, label: &str) -> Vec<(String, Outcome)> {
    let rows: Vec<(String, Outcome)> = reference()
        .iter()
        .filter(|c| c.mode == mode)
        .map(|c| {
            let o = run(c, r_for_energy, given);
            if print_enabled() {
                println!("[{label}] {:>3} {}", CASE_NAMES[c.id - 1], describe(&o));
            }
            (CASE_NAMES[c.id - 1].to_string(), o)
        })
        .collect();
    if print_enabled() {
        let (mut bit, mut all, mut solved) = (0, 0, 0);
        for (_, o) in &rows {
            if let Outcome::Compared(_, m) = o {
                bit += m.bit_identical;
                all += m.compared;
                solved += 1;
            }
        }
        println!("[{label}] SUMMARY solved {solved}/{} bit-identical {bit}/{all}", rows.len());
    }
    rows
}

fn metrics<'a>(t: &'a [(String, Outcome)], name: &str) -> (usize, &'a Metrics) {
    match &t.iter().find(|(n, _)| n == name).expect("case present").1 {
        Outcome::Compared(rung, m) => (*rung, m),
        other => panic!("{name}: {}", describe(other)),
    }
}

fn reason<'a>(t: &'a [(String, Outcome)], name: &str) -> &'a Outcome {
    &t.iter().find(|(n, _)| n == name).expect("case present").1
}

// ---------------------------------------------------------------- tests

#[test]
fn reference_parses_with_every_case_in_both_modes() {
    let cases = reference();
    assert_eq!(cases.len(), 30, "15 cases x 2 tolerance modes");
    for c in &cases {
        assert_eq!(c.info, 0, "upstream INFO for {} {}", c.mode, CASE_NAMES[c.id - 1]);
        assert!(c.species.iter().all(|s| s.stoich.len() == c.elem_pi.len()));
    }
}

#[test]
fn print_tables() {
    // Always runs; prints only with TC_DIFF_PRINT.
    let _ = table("default", R, false, "default A free ");
    let _ = table("tight", R, false, "tight   A free ");
    let _ = table("tight", R_UPSTREAM, false, "tight   B free ");
    let _ = table("default", R, true, "default A given");
    let _ = table("tight", R, true, "tight   A given");
    let _ = table("tight", R_UPSTREAM, true, "tight   B given");
}

/// Cases of the ideal gas + pure condensed (`CO.dat`) and the one ideal gas
/// (`HO.dat`) type that both codes solve, against upstream tightened to 1e-10.
/// The pins are the measured worst values rounded up one decade (RESULTS.md).
#[test]
fn ideal_cases_match_tightened_upstream() {
    let t = table("tight", R, false, "pin tight A free");
    let mut worst = (0.0_f64, 0.0_f64, 0.0_f64);
    for name in ["C1", "C2", "C3", "C4", "H2", "H3", "H4"] {
        let (rung, m) = metrics(&t, name);
        assert!(m.assemblage_match, "{name} assemblage {:?} vs {:?}", m.rust_assemblage, m.upstream_assemblage);
        worst.0 = worst.0.max(m.max_rel_pi);
        worst.1 = worst.1.max(m.max_rel_major_n);
        worst.2 = worst.2.max(m.rel_g_rt);
        // H2 only converges on rung 1 (tol 1e-8): its potentials carry that tolerance.
        let pi_bound = if rung >= 1 { 1e-7 } else { 1e-9 };
        assert!(m.max_rel_pi <= pi_bound, "{name} pi {:e}", m.max_rel_pi);
        assert!(m.max_rel_major_n <= 1e-7, "{name} n {:e}", m.max_rel_major_n);
        assert!(m.rel_g_rt <= 1e-9, "{name} G/RT {:e}", m.rel_g_rt);
    }
    assert!(worst.2 <= 1e-9);
}

/// The one-phase four-species RKMP case (`WAuArO-1.dat`, binary W-Au term, T-dependent
/// `L`) and the CsI liquid plus solid case with exponent-0 and exponent-1 terms.
#[test]
fn rkmp_cases_match_tightened_upstream() {
    let t = table("tight", R, false, "pin tight A free");
    let (_, w1) = metrics(&t, "W1");
    assert!(w1.assemblage_match);
    assert!(w1.max_rel_pi <= 1e-9, "W1 pi {:e}", w1.max_rel_pi);
    assert!(w1.max_rel_major_n <= 1e-12, "W1 n {:e}", w1.max_rel_major_n);
    assert!(w1.rel_g_rt <= 1e-9, "W1 G/RT {:e}", w1.rel_g_rt);
    let g = table("tight", R, true, "pin tight A given");
    let (_, s3) = metrics(&g, "S3");
    assert!(s3.assemblage_match);
    assert!(s3.max_rel_pi <= 1e-9, "S3 pi {:e}", s3.max_rel_pi);
    assert!(s3.max_rel_major_n <= 1e-7, "S3 n {:e}", s3.max_rel_major_n);
    assert!(s3.rel_g_rt <= 1e-9, "S3 G/RT {:e}", s3.rel_g_rt);
}

/// Upstream at its default tolerance (`dTolerance(1) = 1e-5`) bounds the agreement:
/// the measured worst values, not the issue's 1e-8, are what these cases can show.
#[test]
fn default_upstream_tolerance_bounds_the_agreement() {
    let t = table("default", R, false, "pin default A free");
    for name in ["C1", "C2", "C3", "C4", "H3", "H4", "W1"] {
        let (_, m) = metrics(&t, name);
        assert!(m.assemblage_match, "{name}");
        assert!(m.max_rel_pi <= 1e-5, "{name} pi {:e}", m.max_rel_pi);
        assert!(m.max_rel_major_n <= 1e-4, "{name} n {:e}", m.max_rel_major_n);
        assert!(m.rel_g_rt <= 1e-6, "{name} G/RT {:e}", m.rel_g_rt);
    }
}

/// Mode B feeds the J/mol values upstream's `R = 8.314472` implies to a port whose
/// `R = 8.31446261815324`: the relative `R` difference, 1.1e-6, shows through
/// unchanged in the potentials and in `G/RT`.
#[test]
fn gas_constant_difference_shows_through_in_literal_mode() {
    let r_rel = (R_UPSTREAM - R).abs() / R;
    assert!((1.0e-6..1.2e-6).contains(&r_rel), "R relative difference {r_rel:e}");
    let a = table("tight", R, false, "A");
    let b = table("tight", R_UPSTREAM, false, "B");
    for name in ["H3", "H4"] {
        let (_, ma) = metrics(&a, name);
        let (_, mb) = metrics(&b, name);
        assert!(ma.max_rel_pi < 1e-9, "{name} A {:e}", ma.max_rel_pi);
        assert!(mb.max_rel_pi > 1e-7 && mb.max_rel_pi < 1e-5, "{name} B {:e}", mb.max_rel_pi);
        assert!(mb.rel_g_rt > 1e-7 && mb.rel_g_rt < 1e-5, "{name} B G {:e}", mb.rel_g_rt);
    }
}

/// What the port cannot reach, counted so a later port that closes a gap turns this red
/// and the pin is moved with the reason (never relaxed silently).
#[test]
fn out_of_reach_cases_are_the_known_gaps() {
    let t = table("tight", R, false, "gaps");
    for name in ["W2", "W3"] {
        match reason(&t, name) {
            Outcome::OutOfReach(why) => {
                assert!(why.contains("not ported"), "{name}: {why}");
            }
            other => panic!("{name}: {}", describe(other)),
        }
    }
    // W4 has ternary/quaternary RKMP terms too, and its tightened reference run also
    // stopped at upstream's iteration cap, so either reason is reported first.
    assert!(matches!(reason(&t, "W4"), Outcome::OutOfReach(_)));
    // H1: the reference run itself stopped at upstream's iteration cap.
    assert!(matches!(reason(&t, "H1"), Outcome::OutOfReach(_)));
    // With more candidate phases than elements the port's saddle system is singular or
    // stalls (no assemblage selection): the free CsI cases.
    for name in ["S1", "S2", "S3"] {
        assert!(
            matches!(reason(&t, name), Outcome::SolveFailed(_) | Outcome::OutOfReach(_)),
            "{name}: {}",
            describe(reason(&t, name))
        );
    }
}
