// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Original OUTRAM PARK verification code (gh:#1439). It compares this crate's
// port of GeN-Foam's point-kinetics feedback sum against GeN-Foam 652b3da
// (https://gitlab.com/foam-for-nuclear/GeN-Foam, GPL-3.0, EPFL), run on ESI
// OpenFOAM v2506. Only recorded upstream OUTPUT is read here.

//! # V&V: `fast_reactor` feedback and the PRKE backward-Euler step vs GeN-Foam
//!
//! ## Methodology
//!
//! The reference file `verification_and_validation/genfoam_differential/refs/
//! pkdump.ref.txt` is written by the driver `upstream_driver/pkDump/pkDump.C`,
//! which subclasses GeN-Foam's real `solvers::pointKineticNeutronics` and
//!
//! * **F**: runs upstream's unmodified `include/correctReactivity.H` on fixed
//!   inputs (coefficient set of `Tutorials/reactorCases/2D_FFTF`; a second,
//!   dyadic set that lands exactly on rod-worth map knots and ends), every
//!   input printed with 17 significant digits so it round-trips;
//! * **K**: runs the real `correctPhysics()` (`solvePointKinetics.H`) for 300
//!   steps of varying `dt` (1e-3 to 1e-1 s), 8 delayed groups, and prints the
//!   state before and after each step and the total reactivity it used.
//!
//! Each F row is re-evaluated with [`FastReactorFeedback`]; each K row is
//! replayed as ONE [`PointKinetics::step_implicit`] from upstream's state
//! before the step, so error does not accumulate across steps. Pass criterion
//! and results: `verification_and_validation/genfoam_differential/README.md`.
//! Predictions were committed before any run (`PREDICTIONS.md`).

use teh_o_prke::feedback_mechanisms::fast_reactor::{
    CoolantDensityTerm, DrivelineExpansionTerm, FastDopplerTerm, FastReactorFeedback,
    FastReactorState, LinearTemperatureTerm, RodWorthMap, RodWorthOutOfRange,
};
use teh_o_prke::matrix::SquareMatrix;
use teh_o_prke::zero_power_prke::n_group_prke::PointKinetics;
use uom::si::f64::*;
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::ratio::ratio;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volumetric_number_density::per_cubic_meter;
use uom::si::volumetric_number_rate::per_cubic_meter_second;
use uom::ConstZero;

const REF: &str = include_str!(
    "../verification_and_validation/genfoam_differential/refs/pkdump.ref.txt"
);

fn nums(fields: &[&str]) -> Vec<f64> {
    fields.iter().map(|f| f.parse::<f64>().unwrap()).collect()
}

/// Distance in units in the last place (0 when the values are equal as numbers).
fn ulps(a: f64, b: f64) -> u64 {
    if a == b {
        return 0;
    }
    let key = |x: f64| -> i64 {
        let bits = x.to_bits() as i64;
        if bits < 0 {
            i64::MIN - bits
        } else {
            bits
        }
    };
    key(a).abs_diff(key(b))
}

fn rel(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        a.abs()
    } else {
        ((a - b) / b).abs()
    }
}

struct FRow {
    inputs: [f64; 7],
    terms: [f64; 9],
}

struct FCase {
    coef: Vec<(String, f64)>,
    reference: Vec<f64>,
    map: Vec<(f64, f64)>,
    rows: Vec<FRow>,
}

struct KRow {
    idx: usize,
    dt: f64,
    rho: f64,
    old: Vec<f64>,
    new: Vec<f64>,
}

struct KCase {
    lambda_prompt: f64,
    betas: Vec<f64>,
    lambdas: Vec<f64>,
    rows: Vec<KRow>,
}

fn parse() -> (Vec<(String, FCase)>, KCase) {
    let mut cases: Vec<(String, FCase)> = Vec::new();
    let mut k = KCase {
        lambda_prompt: 0.0,
        betas: vec![],
        lambdas: vec![],
        rows: vec![],
    };
    for line in REF.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f[0] {
            "COEF" | "REF" | "MAP" => {
                let tag = f[1].to_string();
                if tag == "K" {
                    continue;
                }
                if !cases.iter().any(|(t, _)| *t == tag) {
                    cases.push((
                        tag.clone(),
                        FCase {
                            coef: vec![],
                            reference: vec![],
                            map: vec![],
                            rows: vec![],
                        },
                    ));
                }
                let c = &mut cases.iter_mut().find(|(t, _)| *t == tag).unwrap().1;
                match f[0] {
                    "COEF" => c.coef.push((f[2].to_string(), f[3].parse().unwrap())),
                    "REF" => c.reference = nums(&f[2..]),
                    _ => c.map.push((f[2].parse().unwrap(), f[3].parse().unwrap())),
                }
            }
            "KIN" => {
                if f[1] != "K" {
                    continue;
                }
                k.lambda_prompt = f[2].parse().unwrap();
                let n: usize = f[3].parse().unwrap();
                k.betas = nums(&f[4..4 + n]);
                k.lambdas = nums(&f[4 + n..4 + 2 * n]);
            }
            "FB" => {
                let c = &mut cases.iter_mut().find(|(t, _)| *t == f[1]).unwrap().1;
                let bar = f.iter().position(|s| *s == "|").unwrap();
                let inp = nums(&f[3..bar]);
                let out = nums(&f[bar + 1..]);
                c.rows.push(FRow {
                    inputs: inp.try_into().unwrap(),
                    terms: out.try_into().unwrap(),
                });
            }
            "K" => {
                let bar = f.iter().position(|s| *s == "|").unwrap();
                let head = nums(&f[2..bar]);
                let n = k.betas.len();
                k.rows.push(KRow {
                    idx: f[1].parse().unwrap(),
                    dt: head[0],
                    rho: head[1],
                    old: head[2..2 + 1 + n].to_vec(),
                    new: nums(&f[bar + 1..]),
                });
            }
            _ => panic!("unknown ref line {line}"),
        }
    }
    (cases, k)
}

fn coef(c: &FCase, name: &str) -> f64 {
    c.coef.iter().find(|(n, _)| n == name).unwrap().1
}

fn lin(a: f64, t_ref: f64) -> Option<LinearTemperatureTerm> {
    Some(LinearTemperatureTerm {
        coefficient: TemperatureCoefficient::new::<per_kelvin>(a),
        reference_temperature: ThermodynamicTemperature::new::<kelvin>(t_ref),
    })
}

/// This crate's feedback set built from the coefficients the driver printed.
/// Driveline: `L = 1 m`, `alpha = c_dl`, so `L alpha` is upstream's `c_dl` exactly.
fn build(c: &FCase, policy: RodWorthOutOfRange) -> FastReactorFeedback {
    let r = &c.reference;
    FastReactorFeedback {
        doppler: Some(FastDopplerTerm {
            doppler_constant: Ratio::new::<ratio>(coef(c, "doppler")),
            reference_temperature: ThermodynamicTemperature::new::<kelvin>(r[0]),
        }),
        axial_fuel_expansion: lin(coef(c, "fuel"), r[0]),
        cladding: lin(coef(c, "clad"), r[1]),
        coolant_temperature: lin(coef(c, "cool"), r[2]),
        coolant_density: Some(CoolantDensityTerm {
            coefficient: SpecificVolume::new::<cubic_meter_per_kilogram>(coef(c, "rho")),
            reference_density: MassDensity::new::<kilogram_per_cubic_meter>(r[3]),
        }),
        radial_expansion: vec![LinearTemperatureTerm {
            coefficient: TemperatureCoefficient::new::<per_kelvin>(coef(c, "struct")),
            reference_temperature: ThermodynamicTemperature::new::<kelvin>(r[4]),
        }],
        driveline: Some(DrivelineExpansionTerm {
            length: Length::new::<meter>(1.0),
            linear_expansion_coefficient: TemperatureCoefficient::new::<per_kelvin>(coef(
                c, "driveline",
            )),
            reference_temperature: ThermodynamicTemperature::new::<kelvin>(r[6]),
            worth: RodWorthMap::new(
                c.map.iter().map(|p| Length::new::<meter>(p.0)).collect(),
                c.map.iter().map(|p| Ratio::new::<ratio>(p.1)).collect(),
            )
            .unwrap()
            .with_out_of_range(policy),
        }),
    }
}

fn state(inp: &[f64; 7]) -> FastReactorState {
    FastReactorState {
        fuel_temperature: ThermodynamicTemperature::new::<kelvin>(inp[0]),
        cladding_temperature: ThermodynamicTemperature::new::<kelvin>(inp[1]),
        coolant_temperature: ThermodynamicTemperature::new::<kelvin>(inp[2]),
        coolant_density: MassDensity::new::<kilogram_per_cubic_meter>(inp[3]),
        radial_structure_temperatures: vec![ThermodynamicTemperature::new::<kelvin>(inp[4])],
        driveline_temperature: ThermodynamicTemperature::new::<kelvin>(inp[6]),
    }
}

const TERM_NAMES: [&str; 8] = [
    "doppler",
    "axial fuel",
    "clad",
    "coolant T",
    "coolant rho",
    "structure",
    "driveline",
    "total",
];

/// Per case and term: (rows, bit-identical, worst ulp) with `UpstreamZero`.
fn feedback_stats() -> Vec<(String, [(usize, usize, u64, usize); 8])> {
    let (cases, _) = parse();
    let mut out = Vec::new();
    for (tag, c) in &cases {
        let fb = build(c, RodWorthOutOfRange::UpstreamZero);
        let mut st = [(0usize, 0usize, 0u64, 0usize); 8];
        for row in &c.rows {
            let e = fb.evaluate(&state(&row.inputs)).unwrap();
            // upstream terms: [doppler, fuel, clad, cool, rho, struct, structMech, driveline, total]
            let mine = [
                e.doppler.get::<ratio>(),
                e.axial_fuel_expansion.get::<ratio>(),
                e.cladding.get::<ratio>(),
                e.coolant_temperature.get::<ratio>(),
                e.coolant_density.get::<ratio>(),
                e.radial_expansion.get::<ratio>(),
                e.driveline.get::<ratio>(),
                e.total().get::<ratio>(),
            ];
            let up = [
                row.terms[0],
                row.terms[1],
                row.terms[2],
                row.terms[3],
                row.terms[4],
                row.terms[5],
                row.terms[7],
                row.terms[8],
            ];
            for i in 0..8 {
                st[i].0 += 1;
                if mine[i].to_bits() == up[i].to_bits() {
                    st[i].1 += 1;
                } else if mine[i] == up[i] {
                    st[i].3 += 1; // numerically equal, differs in the sign of zero
                }
                if i == 0 && ulps(mine[i], up[i]) > 0 {
                    eprintln!("DOPPLER-MISMATCH {tag} T={} mine={:e} upstream={:e}", row.inputs[0], mine[i], up[i]);
                }
                st[i].2 = st[i].2.max(ulps(mine[i], up[i]));
            }
        }
        out.push((tag.clone(), st));
    }
    out
}

#[test]
fn feedback_terms_against_upstream_correct_reactivity() {
    for (tag, st) in feedback_stats() {
        for (i, (n, same, worst, zero_sign)) in st.iter().enumerate() {
            eprintln!(
                "FEEDBACK {tag:4} {:12} rows {n:3} bit-identical {same:3} sign-of-zero-only {zero_sign} worst {worst} ulp",
                TERM_NAMES[i]
            );
            if i == 0 && (tag == "F1" || tag == "F1D") {
                // 1 of 400 Doppler rows differs by 2 ulp (PETIR r_ln, see
                // `petir_ln_is_the_doppler_mismatch`); 2 ulp exceeds the 1 ulp accepted
                assert_eq!((*same, *worst), (399, 2), "{tag} {}", TERM_NAMES[i]);
            } else {
                // every other term equal as a number; the only bit difference is the sign
                // of a zero (the radial-structure fold starts from +0, upstream's term is -0)
                assert_eq!(same + zero_sign, *n, "{tag} {}", TERM_NAMES[i]);
                assert_eq!(*worst, 0, "{tag} {}", TERM_NAMES[i]);
            }
        }
    }
}

/// Localises the Doppler mismatch: this crate's term is `-K ln(T/T_ref)` with
/// PETIR's `r_ln`; upstream's is glibc `log`. Compares the two on every F1
/// ratio and checks that exactly the mismatching row is where they disagree.
#[test]
fn petir_ln_is_the_doppler_mismatch() {
    use petir::mathf::RealMath;
    let (cases, _) = parse();
    let c = &cases.iter().find(|(t, _)| t == "F1").unwrap().1;
    let k = coef(c, "doppler");
    let t_ref = c.reference[0];
    let (mut differ, mut worst, mut std_matches_upstream) = (0, 0u64, 0);
    for row in &c.rows {
        let x = row.inputs[0].max(1.0e-15) / t_ref.max(1.0e-15);
        let (p, g) = (x.r_ln(), x.ln());
        if p.to_bits() != g.to_bits() {
            differ += 1;
            worst = worst.max(ulps(p, g));
            // glibc log reproduces upstream's Doppler term at this row
            if (-k * g).to_bits() == row.terms[0].to_bits() {
                std_matches_upstream += 1;
            }
        }
    }
    eprintln!("PETIR-LN rows {} differ from std ln {differ} worst {worst} ulp; std ln reproduces upstream on {std_matches_upstream}", c.rows.len());
    assert_eq!((differ, std_matches_upstream), (1, 1));
}

#[test]
fn default_error_policy_differs_from_upstream_only_where_documented() {
    let (cases, _) = parse();
    let (mut interpolated, mut at_excluded_end, mut outside) = (0, 0, 0);
    for (tag, c) in &cases {
        let fb_up = build(c, RodWorthOutOfRange::UpstreamZero);
        let fb_err = build(c, RodWorthOutOfRange::Error);
        for row in &c.rows {
            let s = state(&row.inputs);
            let up = fb_up.evaluate(&s).unwrap().driveline.get::<ratio>();
            match fb_err.evaluate(&s) {
                Ok(e) => {
                    let mine = e.driveline.get::<ratio>();
                    if up.to_bits() == mine.to_bits() {
                        interpolated += 1;
                    } else {
                        // the one end upstream excludes: upstream 0, here the tabulated value
                        assert_eq!(up, 0.0, "{tag}: only an upstream zero may differ");
                        assert!(c.map.iter().any(|p| p.1 == mine), "{tag}: not a map value");
                        at_excluded_end += 1;
                    }
                }
                Err(_) => {
                    assert_eq!(up, 0.0, "{tag}: an error is where upstream gives zero");
                    outside += 1;
                }
            }
        }
    }
    eprintln!("ERRPOLICY identical {interpolated} excluded-end {at_excluded_end} error {outside}");
    assert_eq!((interpolated, at_excluded_end, outside), (742, 2, 86));
}

struct StepStats {
    steps: usize,
    p_same: usize,
    p_worst: f64,
    p_worst_step: usize,
    c_same: usize,
    c_total: usize,
    c_worst: f64,
}

fn prke(k: &KCase) -> PointKinetics {
    PointKinetics::new(
        k.betas.iter().map(|b| Ratio::new::<ratio>(*b)).collect(),
        k.lambdas.iter().map(|l| Frequency::new::<hertz>(*l)).collect(),
    )
    .unwrap()
}

fn density(x: f64) -> VolumetricNumberDensity {
    VolumetricNumberDensity::new::<per_cubic_meter>(x)
}

/// One `step_implicit` per upstream step, from upstream's state before it.
fn crate_step(k: &KCase, row: &KRow) -> Vec<f64> {
    let mut pk = prke(k);
    let c_old: Vec<VolumetricNumberDensity> = row.old[1..].iter().map(|x| density(*x)).collect();
    pk.set_state(density(row.old[0]), &c_old).unwrap();
    pk.step_implicit(
        Time::new::<second>(row.dt),
        Ratio::new::<ratio>(row.rho),
        Time::new::<second>(k.lambda_prompt),
        VolumetricNumberRate::ZERO,
    )
    .unwrap();
    let mut v = vec![pk.neutron_density().get::<per_cubic_meter>()];
    v.extend(
        pk.precursor_densities()
            .iter()
            .map(|c| c.get::<per_cubic_meter>()),
    );
    v
}

/// The same step assembled the way `solvePointKinetics.H` does (unscaled
/// rows: `1/dt - (rho - beta)/Lambda`, `-lambda_i`, `-beta_i/Lambda`,
/// `1/dt + lambda_i`, right-hand side `old/dt`) and solved with this crate's
/// LU, to separate matrix assembly from the LU solve.
fn upstream_form_assembly(k: &KCase, row: &KRow) -> (SquareMatrix, Vec<f64>) {
    let g = k.betas.len();
    let n = g + 2; // upstream carries the external-source row too
    let beta: f64 = k.betas.iter().sum();
    let mut a = SquareMatrix::new(n);
    let mut b = vec![0.0; n];
    a.set(0, 0, 1.0 / row.dt - (row.rho - 0.0 - beta) / k.lambda_prompt);
    for i in 1..n - 1 {
        a.set(0, i, -k.lambdas[i - 1]);
        b[0] = row.old[0] / row.dt;
    }
    a.set(0, n - 1, -1.0 / k.lambda_prompt);
    a.set(n - 1, n - 1, 1.0 / row.dt);
    b[n - 1] = 0.0 / row.dt;
    for i in 1..n - 1 {
        a.set(i, 0, -k.betas[i - 1] / k.lambda_prompt);
        a.set(i, i, 1.0 / row.dt + k.lambdas[i - 1]);
        b[i] = row.old[i] / row.dt;
    }
    (a, b)
}

fn upstream_form_lu(k: &KCase, row: &KRow) -> Vec<f64> {
    let (a, b) = upstream_form_assembly(k, row);
    a.solve(&b).unwrap()[..=k.betas.len()].to_vec()
}

fn upstream_form_openfoam_solve(k: &KCase, row: &KRow) -> Vec<f64> {
    let (a, b) = upstream_form_assembly(k, row);
    a.solve_gaussian_openfoam(&b).unwrap()[..=k.betas.len()].to_vec()
}

/// The public GeN-Foam-form step of this crate (zeta = 0, S = 0).
fn crate_genfoam_form_step(k: &KCase, row: &KRow) -> Vec<f64> {
    let mut pk = prke(k);
    let c_old: Vec<VolumetricNumberDensity> = row.old[1..].iter().map(|x| density(*x)).collect();
    pk.set_state(density(row.old[0]), &c_old).unwrap();
    pk.step_implicit_genfoam_form(
        Time::new::<second>(row.dt),
        Ratio::new::<ratio>(row.rho),
        Ratio::ZERO,
        Time::new::<second>(k.lambda_prompt),
        density(0.0),
    )
    .unwrap();
    let mut v = vec![pk.neutron_density().get::<per_cubic_meter>()];
    v.extend(pk.precursor_densities().iter().map(|c| c.get::<per_cubic_meter>()));
    v
}

fn step_stats(step: &dyn Fn(&KCase, &KRow) -> Vec<f64>) -> StepStats {
    let (_, k) = parse();
    let mut s = StepStats {
        steps: 0,
        p_same: 0,
        p_worst: 0.0,
        p_worst_step: 0,
        c_same: 0,
        c_total: 0,
        c_worst: 0.0,
    };
    for row in &k.rows {
        let mine = step(&k, row);
        s.steps += 1;
        if mine[0].to_bits() == row.new[0].to_bits() {
            s.p_same += 1;
        }
        let rp = rel(mine[0], row.new[0]);
        if rp > s.p_worst {
            s.p_worst = rp;
            s.p_worst_step = row.idx;
        }
        for i in 1..mine.len() {
            s.c_total += 1;
            if mine[i].to_bits() == row.new[i].to_bits() {
                s.c_same += 1;
            }
            s.c_worst = s.c_worst.max(rel(mine[i], row.new[i]));
        }
    }
    s
}

#[test]
fn prke_step_against_upstream_solve_point_kinetics() {
    type Step = dyn Fn(&KCase, &KRow) -> Vec<f64>;
    let variants: [(&str, &Step); 4] = [
        ("crate step_implicit", &crate_step),
        ("upstream-form assembly + crate LU", &upstream_form_lu),
        ("upstream-form assembly + OpenFOAM solve()", &upstream_form_openfoam_solve),
        ("step_implicit_genfoam_form", &crate_genfoam_form_step),
    ];
    let mut all = Vec::new();
    for (name, f) in variants {
        let s = step_stats(f);
        eprintln!(
            "KINETICS {name}: steps {} P bit-identical {} worst rel {:e} (step {}) \
             C bit-identical {}/{} worst rel {:e}",
            s.steps, s.p_same, s.p_worst, s.p_worst_step, s.c_same, s.c_total, s.c_worst
        );
        all.push(s);
    }
    // default step_implicit: round-off apart (predicted P <= 1e-10, C <= 1e-11, below half identical)
    assert_eq!((all[0].p_same, all[0].c_same), (6, 761));
    assert!(all[0].p_worst < 1.0e-10 && all[0].c_worst < 1.0e-11);
    // upstream's rows with this crate's LU: still not identical, so the LU is the cause
    assert_eq!((all[1].p_same, all[1].c_same), (5, 896));
    // upstream's rows with OpenFOAM's solve(), and the public GeN-Foam-form step: all identical
    for s in &all[2..] {
        assert_eq!((s.steps, s.p_same, s.c_same, s.c_total), (300, 300, 2400, 2400));
    }
}

#[test]
fn initial_precursor_state_matches_upstream_equilibrium() {
    // upstream: (beta_i P) / (lambda_i Lambda) at the start of the run
    let (_, k) = parse();
    let row = &k.rows[0];
    let mut pk = prke(&k);
    pk.set_critical_equilibrium(density(row.old[0]), Time::new::<second>(k.lambda_prompt));
    let mine = pk.precursor_densities();
    let same = mine
        .iter()
        .zip(&row.old[1..])
        .filter(|(m, u)| m.get::<per_cubic_meter>().to_bits() == u.to_bits())
        .count();
    eprintln!("EQUILIBRIUM bit-identical {same}/{}", k.betas.len());
    assert_eq!(same, k.betas.len());
}

#[test]
fn set_state_rejects_wrong_group_count() {
    let (_, k) = parse();
    let mut pk = prke(&k);
    assert!(pk.set_state(density(1.0), &[density(1.0)]).is_err());
    let _ = VolumetricNumberRate::new::<per_cubic_meter_second>(0.0);
}
