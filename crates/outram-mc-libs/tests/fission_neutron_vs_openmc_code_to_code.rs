// SPDX-License-Identifier: GPL-3.0

//! **`Nuclide::sample_fission_neutron_openmc` against the COMPILED OpenMC
//! `sample_fission_neutron`** (gh:#1386, #1444 d). Upstream `8f79d9d0`
//! (`src/physics.cpp:1081`, with `Nuclide::nu`, `src/nuclide.cpp:532`, and
//! `rotate_angle`), extracted verbatim and compiled with g++ 13.3 `-O2
//! -ffp-contract=off` by `verification_and_validation/upstream_openmc_samplers/
//! run.sh`; the real `UncorrelatedAngleEnergy`, `Tabulated1D` and LCG run unmodified.
//!
//! # Methodology
//!
//! One SYNTHETIC fission nuclide, built identically on both sides (the driver
//! prints every table as hex bits): `nu_total` and `nu_d(E)` tabulated lin-lin on
//! {1e-5, 1e6, 2e7} eV; three precursor groups with yield `nu_d(E) * p_k`
//! (p = 0.1, 0.25, 0.65; nu_d is inflated to beta ~ 0.12 so the delayed branch is
//! exercised, which is irrelevant to the sampling logic); prompt = isotropic Watt
//! (energy-dependent a, b), delayed = isotropic Maxwell with a distinct theta per
//! group; `energy_max` = 1.5 MeV so the resampling loop fires. 6 incident
//! energies x 3 incident directions x 120 seeds = 2160 births. Compared: delayed
//! group, `E'`, the three direction components and the FINAL RNG STATE (which
//! includes the emission-time draw, whose value the port does not keep).
//! Pass criterion: delayed group and final RNG state bit-identical for all
//! births; `E'` < 1e-14 relative and direction components < 1e-14 absolute (last-bit
//! libm differences, below).
//!
//! # Results
//!
//! 2026-11-11, rustc 1.99.1 release: 2160 births (226 delayed): delayed group
//! 2160/2160, final RNG state 2160/2160 (so no extra, missing or reordered draw),
//! `E'` bit-identical 2137/2160 (worst 6.5e-16 relative), direction bit-identical
//! 2034/2160 (all components within 1e-14). Cause: last-bit libm (PETIR vs glibc).

use njoy_outram_park_fork::endf::records::{Cont, Tab1};
use njoy_outram_park_fork::nuclear_data::secondary::{FissionSpectrum, NuBar};
use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::material::nuclide::{DelayedData, Nuclide};

const REF: &str =
    include_str!("../verification_and_validation/upstream_openmc_samplers/reference_fission.txt");

fn f(h: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(h, 16).unwrap())
}
fn key<'a>(line: &'a str, k: &str) -> &'a str {
    let pat = format!(" {k}=");
    let i = line.find(&pat).unwrap_or_else(|| panic!("no {k} in {line}")) + pat.len();
    let rest = &line[i..];
    &rest[..rest.find(' ').unwrap_or(rest.len())]
}
fn list(line: &str, k: &str) -> Vec<f64> {
    key(line, k).split(',').map(f).collect()
}
fn tab(name: &str) -> (Vec<f64>, Vec<f64>) {
    let l = REF
        .lines()
        .find(|l| l.split_whitespace().take(2).collect::<Vec<_>>() == ["tab", name])
        .unwrap_or_else(|| panic!("no tab {name}"));
    (list(l, "x"), list(l, "y"))
}
fn tab1(name: &str) -> Tab1 {
    let (x, y) = tab(name);
    Tab1 {
        head: Cont { c1: 0.0, c2: 0.0, l1: 0, l2: 0, n1: 1, n2: x.len() as i32 },
        interp: vec![(x.len() as u32, 2)],
        pairs: x.into_iter().zip(y).collect(),
    }
}
fn param(line_start: &str) -> &'static str {
    REF.lines().find(|l| l.starts_with(line_start)).unwrap()
}

fn nuclide() -> (Nuclide, f64) {
    let (eg, nu_total) = tab("nu_total");
    let (_, nud) = tab("nu_delayed");
    let n = key(param("param n_groups"), "n_groups").parse::<usize>().unwrap();
    let emax = f(key(param("param n_groups"), "emax"));
    let mut lambda = Vec::new();
    let mut frac = Vec::new();
    let mut spectra = Vec::new();
    for g in 0..n {
        let l = param(&format!("param lam{g}="));
        lambda.push(f(key(&format!(" {l}"), &format!("lam{g}"))));
        let p = f(key(&format!(" {l}"), &format!("p{g}")));
        frac.push(vec![(eg[0], p), (*eg.last().unwrap(), p)]);
        spectra.push(FissionSpectrum::Maxwell { theta: tab1(&format!("theta_g{}", g + 1)), u: 1.0e4 });
    }
    let delayed = DelayedData {
        lambda,
        energy: eg.clone(),
        nu_delayed: nud,
        group_fraction: frac,
        nu_delayed_interp: Vec::new(),
        group_fraction_interp: vec![Vec::new(); n],
        lambda_is_lowest_energy_only: false,
        spectra,
    };
    let chi = FissionSpectrum::WattEnergyDependent { a: tab1("watt_a"), b: tab1("watt_b"), u: 1.0e4 };
    let nu = NuBar { energy: eg, nu_total, poly: None, interp: Vec::new() };
    let nuc = Nuclide::from_core("U235").expect("core").with_fission_data(nu, chi, Some(delayed));
    (nuc, emax)
}

#[derive(Default, Debug)]
struct Tally {
    n: usize,
    group_ok: usize,
    e_ok: usize,
    dir_ok: usize,
    end_ok: usize,
    delayed: usize,
    worst_e_rel: f64,
    worst_dir_abs: f64,
}

/// Compiled-upstream differential, whole routine: group, E', direction, end state.
#[test]
fn sample_fission_neutron_matches_openmc() {
    let (nuc, emax) = nuclide();
    let mut t = Tally::default();
    for l in REF.lines().filter(|l| l.starts_with("F ")) {
        let seed0 = u64::from_str_radix(key(l, "seed0"), 16).unwrap();
        let u0 = match key(l, "id") {
            "0" => Direction::new(0.0, 0.0, 1.0),
            "1" => Direction::new(0.6, 0.0, 0.8),
            _ => Direction::new(-0.48, 0.64, -0.6),
        };
        let e_in = f(key(l, "E"));
        let mut seed = seed0;
        let (g, e, d) = nuc.sample_fission_neutron_openmc(e_in, emax, u0, &mut seed);
        let g_ref: i64 = key(l, "grp").parse().unwrap();
        let e_ref = f(key(l, "Eo"));
        let uref = list(&format!("{l} "), "u");
        let end_ref = u64::from_str_radix(key(l, "end"), 16).unwrap();
        t.n += 1;
        t.delayed += (g_ref > 0) as usize;
        t.group_ok += (g.map_or(0, |k| k as i64 + 1) == g_ref) as usize;
        t.e_ok += (e.to_bits() == e_ref.to_bits()) as usize;
        t.dir_ok += (d.u.to_bits() == uref[0].to_bits()
            && d.v.to_bits() == uref[1].to_bits()
            && d.w.to_bits() == uref[2].to_bits()) as usize;
        t.end_ok += (seed == end_ref) as usize;
        t.worst_e_rel = t.worst_e_rel.max(((e - e_ref) / e_ref).abs());
        for (a, b) in [(d.u, uref[0]), (d.v, uref[1]), (d.w, uref[2])] {
            t.worst_dir_abs = t.worst_dir_abs.max((a - b).abs());
        }
    }
    println!("fission birth vs OpenMC: {t:?}");
    assert_eq!(t.n, 2160);
    assert!(t.delayed > 100, "delayed branch not exercised: {t:?}");
    assert_eq!(t.group_ok, t.n, "{t:?}");
    assert_eq!(t.end_ok, t.n, "{t:?}");
    // E' and the direction are bit-identical except last-bit libm differences
    // (PETIR libm vs glibc `ln`/`cos`/`sin` in the Watt/Maxwell draws and
    // `rotate_angle`; accepted by the 2026-11-11 PETIR-maths decision, gh:#1455).
    // The same cause is A/B-proven for the Maxwell energy draw in
    // `secondary_samplers_vs_openmc_code_to_code::maxwell_energy_differences_are_libm_roundoff`;
    // for the direction it is inferred (every sample's RNG end state is identical,
    // so no draw differs) and NOT separately A/B-tested here.
    assert!(t.worst_e_rel < 1.0e-14, "{t:?}");
    assert!(t.worst_dir_abs < 1.0e-14, "{t:?}");
}
