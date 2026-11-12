// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.

//! **Secondary-distribution samplers against OpenMC `8f79d9d0` with a fixed
//! random-number sequence** (gh:#1386, prio-1 row 4).
//!
//! # Methodology
//!
//! `verification_and_validation/upstream_openmc_samplers/run.sh` builds OpenMC's
//! real `distribution_energy.cpp`, `distribution_angle.cpp`,
//! `secondary_{uncorrelated,kalbach,correlated,nbody}.cpp`, `random_dist.cpp`
//! and `random_lcg.cpp` UNMODIFIED, plus verbatim, SHA-checked extracts of
//! `Distribution`/`Tabular` (`distribution.cpp`), `Tabulated1D` (`endf.cpp`)
//! and `exprel`/`log1prel`/`get_energy_index` (`math_functions.cpp`), against an
//! in-memory replacement of the HDF5 layer, so upstream's own constructors run
//! on tables the driver builds in `openmc.data`'s layout. For each family it
//! prints the table (hex bits) and then `(E', mu)` and the final RNG state for
//! 6 incident energies x 40 seeds. A "fixed xi sequence" is a fixed seed: both
//! codes draw from the same `prn`, so a mismatch in the sampled value, or in
//! the **final seed** (number and order of draws), is a difference in the
//! routine. This test builds the identical table in the port's types and
//! compares.
//!
//! **Pass criterion:** bit identity of `E'`, `mu` and the end state, no
//! tolerance.
//!
//! # Prediction (written before the run)
//!
//! `random_dist` `maxwell`/`watt` and `ContinuousTabular` energy: bit-identical
//! (ported line by line and statistically checked before). `Maxwell`/
//! `Evaporation`/`Watt`: bit-identical `E'`. **Expected to differ**: the
//! uncorrelated law's draw order (OpenMC draws `mu` first, the port `E'`
//! first), Kalbach-Mann's cosine (OpenMC spends two or three variates, the
//! port one), the correlated law's angular bin choice, `normal_variate`
//! (OpenMC Marsaglia polar, the port Box-Muller) and N-body phase space (no
//! port at all). **CORRECTED 2026-11-11:** `NBodyPhaseSpace` is now ported
//! (`physics::nbody`) and compared here (357 of 360 bit-identical, the rest
//! 1 ulp of libm `ln`/`cos`, explained by `nbody_differences_are_libm_roundoff`),
//! and the thermal S(alpha,beta) laws of `secondary_thermal.cpp` are compared
//! below (all five bit-identical on the first run; no port change needed beyond
//! `ThermalScattering::from_ace_thermal`).
//!
//! # Results
//!
//! See `verification_and_validation/upstream_openmc_samplers/README.md`.

use njoy_outram_park_fork::endf::records::{Cont, Tab1};
use njoy_outram_park_fork::nuclear_data::secondary::{
    ChiEout, ChiTabular, ContinuumAngular, ContinuumAngularRow, ContinuumAngularTable,
    ContinuumBranch, ContinuumKalbachRow, ContinuumKalbachTable, FissionSpectrum,
    UncorrelatedEmission,
};
use outram_mc_libs::material::nuclide::sample_uncorrelated_emission;
use outram_mc_libs::physics::scatter::{sample_continuum_branch, ContinuumAngularMode};
use outram_mc_libs::rng::distributions::{maxwell, normal_variate, watt};
use std::collections::BTreeMap;

const REFERENCE: &str =
    include_str!("../verification_and_validation/upstream_openmc_samplers/reference.txt");

fn f(h: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(h, 16).unwrap())
}
fn list(line: &str, key: &str) -> Vec<f64> {
    let pat = format!(" {key}=");
    let i = line.find(&pat).unwrap_or_else(|| panic!("no {key} in {line}")) + pat.len();
    let rest = &line[i..];
    let end = rest.find(' ').unwrap_or(rest.len());
    rest[..end].split(',').map(f).collect()
}
fn int(line: &str, key: &str) -> i64 {
    let pat = format!(" {key}=");
    let i = line.find(&pat).unwrap_or_else(|| panic!("no {key} in {line}")) + pat.len();
    let rest = &line[i..];
    let end = rest.find(' ').unwrap_or(rest.len());
    rest[..end].parse().unwrap()
}
fn hexu(line: &str, key: &str) -> u64 {
    let pat = format!(" {key}=");
    let i = line.find(&pat).unwrap_or_else(|| panic!("no {key} in {line}")) + pat.len();
    let rest = &line[i..];
    let end = rest.find(' ').unwrap_or(rest.len());
    u64::from_str_radix(&rest[..end], 16).unwrap()
}
fn lines(prefix: &str, name: &str) -> Vec<&'static str> {
    REFERENCE
        .lines()
        .filter(|l| {
            let mut t = l.split_whitespace();
            t.next() == Some(prefix) && t.next() == Some(name)
        })
        .collect()
}

#[derive(Clone)]
struct Row {
    interp: i64,
    nd: usize,
    eout: Vec<f64>,
    p: Vec<f64>,
    c: Vec<f64>,
    r: Vec<f64>,
    a: Vec<f64>,
}
struct Tbl {
    egrid: Vec<f64>,
    rows: Vec<Row>,
    /// `mu[i][k]` = (x, p, c) of the angle table at outgoing point k of incident i.
    mu: Vec<Vec<(Vec<f64>, Vec<f64>, Vec<f64>)>>,
}
fn table(name: &str) -> Tbl {
    let t = lines("table", name)[0];
    let egrid = list(t, "egrid");
    let mut rows = Vec::new();
    let mut mu: Vec<Vec<(Vec<f64>, Vec<f64>, Vec<f64>)>> = vec![Vec::new(); egrid.len()];
    for l in lines("row", name) {
        let has = l.contains(" r=");
        rows.push(Row {
            interp: int(l, "interp"),
            nd: int(l, "nd") as usize,
            eout: list(l, "eout"),
            p: list(l, "p"),
            c: list(l, "c"),
            r: if has { list(l, "r") } else { vec![] },
            a: if has { list(l, "a") } else { vec![] },
        });
    }
    for l in lines("mu", name) {
        let i = int(l, "i") as usize;
        mu[i].push((list(l, "x"), list(l, "p"), list(l, "c")));
    }
    Tbl { egrid, rows, mu }
}
fn chi(t: &Tbl) -> ChiTabular {
    ChiTabular {
        incident: t.egrid.clone(),
        tables: t
            .rows
            .iter()
            .map(|r| ChiEout {
                e_out: r.eout.clone(),
                pdf: r.p.clone(),
                cdf: r.c.clone(),
                linlin: r.interp == 2,
                n_discrete: r.nd,
            })
            .collect(),
        incident_interp: vec![(t.egrid.len() as u32, 2)],
    }
}
fn tab1(name: &str, which: usize) -> Tab1 {
    let l = lines("tab1", name)[which];
    let (x, y) = (list(l, "x"), list(l, "y"));
    Tab1 {
        head: Cont { c1: 0.0, c2: 0.0, l1: 0, l2: 0, n1: 1, n2: x.len() as i32 },
        interp: vec![(x.len() as u32, 2)],
        pairs: x.into_iter().zip(y).collect(),
    }
}

/// One sampled line: (E_in, seed index, e_out, mu, end state).
struct Sample {
    e_in: f64,
    seed: u64,
    eout: f64,
    mu: Option<f64>,
    end: u64,
}
fn samples(name: &str) -> Vec<Sample> {
    lines("S", name)
        .into_iter()
        .map(|l| {
            let s = int(l, "seed") as u64;
            Sample {
                e_in: f64::from_bits(hexu(l, "E")),
                seed: s * 7919 + 11,
                eout: f64::from_bits(hexu(l, "eout")),
                mu: if l.contains(" mu=") { Some(f64::from_bits(hexu(l, "mu"))) } else { None },
                end: hexu(l, "end"),
            }
        })
        .collect()
}

/// Tally of one family: bit-identical counts of E', mu and end state.
#[derive(Default, Debug)]
struct Tally {
    n: usize,
    e_ok: usize,
    mu_ok: usize,
    mu_n: usize,
    end_ok: usize,
    worst_e_rel: f64,
}
impl Tally {
    fn add(&mut self, e: f64, e_ref: f64, mu: Option<(f64, f64)>, end: u64, end_ref: u64) {
        self.n += 1;
        if e.to_bits() == e_ref.to_bits() {
            self.e_ok += 1;
        }
        let rel = ((e - e_ref) / e_ref).abs();
        if rel > self.worst_e_rel {
            self.worst_e_rel = rel;
        }
        if let Some((m, m_ref)) = mu {
            self.mu_n += 1;
            if m.to_bits() == m_ref.to_bits() {
                self.mu_ok += 1;
            }
        }
        if end == end_ref {
            self.end_ok += 1;
        }
    }
    fn report(&self, name: &str) {
        println!(
            "{name:22} n={:4}  E' bit-identical {:4}  mu {:4}/{:4}  end-state {:4}  worst rel(E') {:.3e}",
            self.n, self.e_ok, self.mu_ok, self.mu_n, self.end_ok, self.worst_e_rel
        );
    }
    fn all_identical(&self) -> bool {
        self.e_ok == self.n && self.mu_ok == self.mu_n && self.end_ok == self.n
    }
}

/// `random_dist.cpp`: `maxwell_spectrum`, `watt_spectrum`, `normal_variate` (value and end state),
/// 64 seeds each.
#[test]
fn maxwell_and_watt_spectra_match_openmc() {
    let mut t = Tally::default();
    for l in REFERENCE.lines().filter(|l| l.starts_with("D ")) {
        let s = int(l, "seed") as u64 * 104729 + 3;
        let mut seed = s;
        let m = maxwell(&mut seed, 1.2e6);
        assert_eq!(m.to_bits(), hexu(l, "maxwell"), "{l}");
        assert_eq!(seed, hexu(l, "m_end"), "{l}");
        let w = watt(&mut seed, 0.988e6, 2.249e-6);
        assert_eq!(w.to_bits(), hexu(l, "watt"), "{l}");
        assert_eq!(seed, hexu(l, "w_end"), "{l}");
        let n = normal_variate(2.0, 0.5, &mut seed);
        assert_eq!(n.to_bits(), hexu(l, "normal"), "{l}");
        assert_eq!(seed, hexu(l, "n_end"), "{l}");
        t.n += 1;
    }
    assert_eq!(t.n, 64);
}

fn run_ct(name: &str) -> Tally {
    let t = table(name);
    let branch = ContinuumBranch {
        spectrum: chi(&t),
        yield_pairs: vec![],
        yield_interp: vec![],
        applicability: None,
        angular: ContinuumAngular::EvaluatedIsotropic,
    };
    let mut tally = Tally::default();
    for s in samples(name) {
        let mut seed = s.seed;
        let (e, _mu) = sample_continuum_branch(
            &branch,
            s.e_in,
            ContinuumAngularMode::IsotropicAblation,
            &mut seed,
        );
        tally.add(e, s.eout, None, seed, seed);
    }
    tally
}

/// `ContinuousTabular::sample`: histogram, lin-lin, and lin-lin with two
/// discrete lines. Compared on `E'` (the port's `sample_continuum_branch`
/// spends one extra variate on the cosine, so its end state is not compared
/// here; `end` is compared in the uncorrelated test below).
#[test]
fn continuous_tabular_energy_matches_openmc() {
    for name in ["ct_hist", "ct_linlin", "ct_disc"] {
        let t = run_ct(name);
        t.report(name);
        assert_eq!(t.e_ok, t.n, "{name}: {t:?}");
    }
}

fn run_kalbach(name: &str) -> Tally {
    let t = table(name);
    let angular = ContinuumAngular::KalbachMann(
        t.rows
            .iter()
            .map(|r| ContinuumKalbachTable {
                rows: r.r.iter().zip(&r.a).map(|(&r, &a)| ContinuumKalbachRow { r, a }).collect(),
            })
            .collect(),
    );
    let branch = ContinuumBranch {
        spectrum: chi(&t),
        yield_pairs: vec![],
        yield_interp: vec![],
        applicability: None,
        angular,
    };
    let mut tally = Tally::default();
    for s in samples(name) {
        let mut seed = s.seed;
        let (e, mu) =
            sample_continuum_branch(&branch, s.e_in, ContinuumAngularMode::Evaluated, &mut seed);
        tally.add(e, s.eout, Some((mu, s.mu.unwrap())), seed, s.end);
    }
    tally
}

/// `KalbachMann::sample` (`sample_params` + the two-branch cosine).
#[test]
fn kalbach_mann_matches_openmc() {
    for name in ["km_hist", "km_linlin", "kmd_hist", "kmd_linlin"] {
        let t = run_kalbach(name);
        t.report(name);
        assert!(t.all_identical(), "{name}: {t:?}");
    }
}

fn run_correlated(name: &str) -> Tally {
    let t = table(name);
    let tables: Vec<ContinuumAngularTable> = t
        .mu
        .iter()
        .map(|rows| ContinuumAngularTable {
            rows: rows
                .iter()
                .map(|(x, p, c)| ContinuumAngularRow {
                    cosines: x.clone(),
                    cdf: c.clone(),
                    pdf: p.clone(),
                    histogram: false,
                    mubar: 0.0,
                })
                .collect(),
        })
        .collect();
    let branch = ContinuumBranch {
        spectrum: chi(&t),
        yield_pairs: vec![],
        yield_interp: vec![],
        applicability: None,
        angular: ContinuumAngular::Legendre(tables),
    };
    let mut tally = Tally::default();
    for s in samples(name) {
        let mut seed = s.seed;
        let (e, mu) =
            sample_continuum_branch(&branch, s.e_in, ContinuumAngularMode::Evaluated, &mut seed);
        tally.add(e, s.eout, Some((mu, s.mu.unwrap())), seed, s.end);
    }
    tally
}

/// `CorrelatedAngleEnergy::sample` (energy by `ContinuousTabular`'s scheme,
/// then the angle table of the chosen outgoing point).
#[test]
fn correlated_angle_energy_matches_openmc() {
    for name in ["corr_hist", "corr_linlin", "corrd_hist", "corrd_linlin"] {
        let t = run_correlated(name);
        t.report(name);
        assert!(t.all_identical(), "{name}: {t:?}");
    }
}

/// `UncorrelatedAngleEnergy::sample` (cosine FIRST, then energy) with the
/// Maxwell, Evaporation and Watt energy laws on `Tabulated1D` parameters and
/// either a tabulated angle distribution or none (isotropic).
///
/// `mu` and the end state must be bit-identical. `E'` is accepted within 1e-14
/// relative ONLY because the difference is explained and checked by
/// `maxwell_energy_differences_are_libm_roundoff`: the port calls PETIR's
/// `RealMath` `ln`/`cos`/`exp` (gh:#985), which differ from glibc in the last
/// bit for a few percent of arguments.
#[test]
fn uncorrelated_angle_energy_matches_openmc() {
    use njoy_outram_park_fork::acer::angular::{ElasticAngular, EnergyAngular};
    let theta = tab1("theta", 0);
    let maxwell_law = FissionSpectrum::Maxwell { theta: theta.clone(), u: 1.0e4 };
    let evap_law = FissionSpectrum::Evaporation { theta, u: 1.0e4 };
    let watt_law =
        FissionSpectrum::WattEnergyDependent { a: tab1("a", 0), b: tab1("b", 0), u: 1.0e4 };
    let grid_line = REFERENCE.lines().find(|l| l.starts_with("angle uncorr_grid")).unwrap();
    let egrid = list(grid_line, "egrid");
    let energies: Vec<EnergyAngular> = REFERENCE
        .lines()
        .filter(|l| l.starts_with("angle uncorr i="))
        .zip(egrid)
        .map(|(l, e)| EnergyAngular {
            e_mev: e * 1.0e-6,
            cosines: list(l, "x"),
            pdf: list(l, "p"),
            cdf: list(l, "c"),
            histogram: false,
        })
        .collect();
    let iso = || ElasticAngular { energies: vec![], lct: 1 };
    let arms = [
        ("uncorr_maxwell", maxwell_law.clone(), ElasticAngular { energies, lct: 1 }),
        ("uncorr_iso_maxwell", maxwell_law, iso()),
        ("uncorr_iso_evaporation", evap_law, iso()),
        ("uncorr_iso_watt", watt_law, iso()),
    ];
    for (name, energy, angular) in arms {
        let law = UncorrelatedEmission { energy, angular, lct: 1, yield_n: 1 };
        let mut tally = Tally::default();
        for s in samples(name) {
            let mut seed = s.seed;
            let (e, mu) = sample_uncorrelated_emission(&law, s.e_in, &mut seed);
            tally.add(e, s.eout, Some((mu, s.mu.unwrap())), seed, s.end);
        }
        tally.report(name);
        assert_eq!(tally.mu_ok, tally.mu_n, "{name}: mu {tally:?}");
        assert_eq!(tally.end_ok, tally.n, "{name}: end state {tally:?}");
        assert!(tally.worst_e_rel < 1.0e-14, "{name}: E' {tally:?}");
    }
}

/// The E' differences above are glibc-vs-PETIR `ln`/`cos` last-bit differences:
/// the Maxwell draw recomputed with `std` math reproduces OpenMC's bits for
/// EVERY sample, including those where the port's `E'` differs.
#[test]
fn maxwell_energy_differences_are_libm_roundoff() {
    let theta = tab1("theta", 0);
    let th = |e: f64| {
        let p = &theta.pairs;
        if e > p[2].0 {
            return p[2].1;
        }
        let i = if e <= p[1].0 { 0 } else { 1 };
        let (x0, y0) = p[i];
        let (x1, y1) = p[i + 1];
        let r = (e - x0) / (x1 - x0);
        y0 + r * (y1 - y0)
    };
    let mut n = 0;
    // (arm, variates OpenMC spends on the cosine before the energy): isotropic 1;
    // tabulated angle 2 (neighbour pick + table inversion).
    for (arm, mu_draws) in [("uncorr_iso_maxwell", 1), ("uncorr_maxwell", 2)] {
        for s in samples(arm) {
            let mut seed = s.seed;
            for _ in 0..mu_draws {
                let _ = outram_mc_libs::rng::lcg::prn(&mut seed);
            }
            let t = th(s.e_in);
            let e = loop {
                let r1 = outram_mc_libs::rng::lcg::prn(&mut seed);
                let r2 = outram_mc_libs::rng::lcg::prn(&mut seed);
                let r3 = outram_mc_libs::rng::lcg::prn(&mut seed);
                let c = (std::f64::consts::PI / 2.0 * r3).cos();
                let e = -t * (r1.ln() + r2.ln() * c * c);
                if e <= s.e_in - 1.0e4 {
                    break e;
                }
            };
            assert_eq!(e.to_bits(), s.eout.to_bits(), "{arm} E_in {} seed {}", s.e_in, s.seed);
            n += 1;
        }
    }
    assert_eq!(n, 320);
}

/// Reference-table sanity: every family's table lines are present (guards the
/// parser against a driver change).
#[test]
fn reference_has_every_family() {
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for l in REFERENCE.lines().filter(|l| l.starts_with("S ")) {
        *count.entry(l.split_whitespace().nth(1).unwrap()).or_default() += 1;
    }
    for fam in [
        "ct_hist", "ct_linlin", "ct_disc", "level", "nbody3",
        "nbody4", "nbody5", "km_hist", "km_linlin", "corr_hist", "corr_linlin", "uncorr_maxwell",
        "uncorr_iso_maxwell", "uncorr_iso_evaporation", "uncorr_iso_watt",
    ] {
        assert!(count.get(fam).copied().unwrap_or(0) > 0, "missing {fam}");
    }
}

/// `NBodyPhaseSpace::sample` (3, 4, 5 bodies; ENDF/ACE LAW 66), ported as
/// `outram_mc_libs::physics::nbody::NBodyPhaseSpace` (gh:#1444).
///
/// **Methodology:** the reference lines `S nbodyN` come from OpenMC's real
/// `secondary_nbody.cpp` (unmodified, `-ffp-contract=off`); the parameters are in
/// the `param nbodyN` lines; 3 incident energies x 40 seeds per body count.
/// **Pass criterion:** bit identity of `E'`, `mu` and the end state (a missing or
/// extra draw changes the end state). Result: see the harness README.
#[test]
fn nbody_phase_space_matches_openmc() {
    use outram_mc_libs::physics::nbody::NBodyPhaseSpace;
    for nb in 3..=5 {
        let name = format!("nbody{nb}");
        let p = lines("param", &name)[0];
        let d = NBodyPhaseSpace {
            n_bodies: int(p, "n") as i32,
            mass_ratio: f64::from_bits(hexu(p, "total_mass")),
            awr: f64::from_bits(hexu(p, "awr")),
            q_value: f64::from_bits(hexu(p, "q")),
        };
        let mut tally = Tally::default();
        for s in samples(&name) {
            let mut seed = s.seed;
            let (e, mu) = d.sample(s.e_in, &mut seed);
            tally.add(e, s.eout, Some((mu, s.mu.unwrap())), seed, s.end);
        }
        tally.report(&name);
        // E' is accepted within 1e-14 ONLY because `nbody_differences_are_libm_roundoff`
        // shows the difference is glibc-vs-PETIR ln/cos (gh:#985), bit-identical with std math.
        assert!(tally.worst_e_rel < 1.0e-14, "{name}: E' {tally:?}");
        assert_eq!(tally.mu_ok, tally.mu_n, "{name}: mu {tally:?}");
        assert_eq!(tally.end_ok, tally.n, "{name}: end state {tally:?}");
    }
}

// ---------------------------------------------------------------------------
// Thermal S(alpha,beta) samplers (`secondary_thermal.cpp`), gh:#1444.
//
// Methodology: the driver builds each law in OpenMC's own in-memory layout and
// runs the REAL constructors of `CoherentElasticAE`, `IncoherentElasticAEDiscrete`,
// `IncoherentInelasticAEDiscrete` (equiprobable and skewed) and
// `IncoherentInelasticAE` (continuous), prints the arrays (`tdata ...`,
// `table thcont`) and samples each at 6-7 incident energies x 40 seeds. This side
// hands the same arrays to `ThermalScattering::from_ace_thermal` (the arrays an
// ACE thermal table decodes to) and samples through the public elastic /
// inelastic entry points. Pass criterion: bit identity of E', mu and end state.
// Not compared: the analytic `IncoherentElasticAE` (ENDF-HDF5 form; the port
// always holds the discrete cosine form NJOY/ACER writes) and `MixedElasticAE`
// selection.
// ---------------------------------------------------------------------------

use njoy_outram_park_fork::acer::thermal_read::{
    AceThermal, AceThermalContinuous, AceThermalElastic, AceThermalEmission,
};
use outram_mc_libs::material::thermal::ThermalScattering;

fn dummy_emission() -> Vec<AceThermalEmission> {
    vec![
        AceThermalEmission { e_out: vec![0.1], cosines: vec![0.0], n_mu: 1 },
        AceThermalEmission { e_out: vec![0.2], cosines: vec![0.0], n_mu: 1 },
    ]
}

fn run_thermal(name: &str, th: &ThermalScattering, elastic: bool) -> Tally {
    let mut tally = Tally::default();
    for s in samples(name) {
        let mut seed = s.seed;
        let (e, mu) = if elastic {
            th.elastic().sample(s.e_in, &mut seed).expect("elastic sample")
        } else {
            th.sample_inelastic(s.e_in, &mut seed).expect("inelastic sample")
        };
        tally.add(e, s.eout, Some((mu, s.mu.unwrap())), seed, s.end);
    }
    tally.report(name);
    tally
}

fn thermal_with_elastic(elastic: AceThermalElastic) -> ThermalScattering {
    ThermalScattering::from_ace_thermal(
        AceThermal {
            kt_ev: 0.0253,
            inel_energy: vec![1.0e-5, 4.0],
            inel_xs: vec![1.0, 1.0],
            emission: dummy_emission(),
            continuous: vec![],
            elastic,
            ifeng: 0,
        },
        "synthetic",
    )
}

/// `CoherentElasticAE::sample` against OpenMC (bit identity).
#[test]
fn coherent_elastic_matches_openmc() {
    let l = lines("tdata", "coh")[0];
    let th = thermal_with_elastic(AceThermalElastic::Coherent {
        energy: list(l, "edges"),
        cumulative: list(l, "factors"),
    });
    let t = run_thermal("tcoh", &th, true);
    assert!(t.all_identical() && t.e_ok == t.n, "{t:?}");
}

/// `IncoherentElasticAEDiscrete::sample` against OpenMC (bit identity).
#[test]
fn incoherent_elastic_discrete_matches_openmc() {
    let l = lines("tdata", "incoh")[0];
    let eg = list(l, "g");
    let th = thermal_with_elastic(AceThermalElastic::Incoherent {
        energy: eg.clone(),
        xs: vec![1.0; eg.len()],
        cosines: list(l, "mu"),
        n_mu: int(l, "nmu") as usize,
    });
    let t = run_thermal("tincoh", &th, true);
    assert!(t.all_identical() && t.e_ok == t.n, "{t:?}");
}

fn inelastic_discrete(name: &str) -> Tally {
    let l = lines("tdata", name)[0];
    let eg = list(l, "g");
    let (eo, mu) = (list(l, "eo"), list(l, "mu"));
    let (nout, nmu) = (int(l, "nout") as usize, int(l, "nmu") as usize);
    let emission = (0..eg.len())
        .map(|i| AceThermalEmission {
            e_out: eo[i * nout..(i + 1) * nout].to_vec(),
            cosines: mu[i * nout * nmu..(i + 1) * nout * nmu].to_vec(),
            n_mu: nmu,
        })
        .collect();
    let th = ThermalScattering::from_ace_thermal(
        AceThermal {
            kt_ev: 0.0253,
            inel_energy: eg.clone(),
            inel_xs: vec![1.0; eg.len()],
            emission,
            continuous: vec![],
            elastic: AceThermalElastic::None,
            ifeng: int(l, "skewed") as i32,
        },
        "synthetic",
    );
    run_thermal(name, &th, false)
}

/// `IncoherentInelasticAEDiscrete::sample`, equiprobable bins (IFENG 0).
#[test]
fn inelastic_discrete_matches_openmc() {
    let t = inelastic_discrete("tinel_disc");
    assert!(t.all_identical() && t.e_ok == t.n, "{t:?}");
}

/// `IncoherentInelasticAEDiscrete::sample`, skewed bins (IFENG 1).
#[test]
fn inelastic_skewed_matches_openmc() {
    let t = inelastic_discrete("tinel_skew");
    assert!(t.all_identical() && t.e_ok == t.n, "{t:?}");
}

/// `IncoherentInelasticAE::sample` (continuous, IFENG 2) on a correlated table.
#[test]
fn inelastic_continuous_matches_openmc() {
    let tb = table("thcont");
    let continuous = tb
        .rows
        .iter()
        .enumerate()
        .map(|(i, r)| AceThermalContinuous {
            e_out: r.eout.clone(),
            pdf: r.p.clone(),
            cdf: r.c.clone(),
            cosines: tb.mu[i].iter().flat_map(|(x, _, _)| x.iter().copied()).collect(),
            n_mu: 5,
        })
        .collect();
    let th = ThermalScattering::from_ace_thermal(
        AceThermal {
            kt_ev: 0.0253,
            inel_energy: tb.egrid.clone(),
            inel_xs: vec![1.0; tb.egrid.len()],
            emission: vec![],
            continuous,
            elastic: AceThermalElastic::None,
            ifeng: 2,
        },
        "synthetic",
    );
    let t = run_thermal("tinel_cont", &th, false);
    assert!(t.all_identical() && t.e_ok == t.n, "{t:?}");
}

/// The N-body `E'` differences above are glibc-vs-PETIR `ln`/`cos` last-bit
/// differences: the same algorithm written with `std` (glibc) math reproduces
/// OpenMC's bits for EVERY sample of 3, 4 and 5 bodies.
#[test]
fn nbody_differences_are_libm_roundoff() {
    use std::f64::consts::PI;
    let (mut n, mut ok) = (0, 0);
    for nb in 3..=5 {
        let name = format!("nbody{nb}");
        let p = lines("param", &name)[0];
        let (ap, awr, q) = (
            f64::from_bits(hexu(p, "total_mass")),
            f64::from_bits(hexu(p, "awr")),
            f64::from_bits(hexu(p, "q")),
        );
        for s in samples(&name) {
            let mut seed = s.seed;
            let _mu = -1.0 + 2.0 * outram_mc_libs::rng::lcg::prn(&mut seed);
            let e_max = (ap - 1.0) / ap * (awr / (awr + 1.0) * s.e_in + q);
            let mut p3 = || outram_mc_libs::rng::lcg::prn(&mut seed);
            let maxw = |p3: &mut dyn FnMut() -> f64| {
                let (r1, r2, r3) = (p3(), p3(), p3());
                let c = (PI / 2.0 * r3).cos();
                -1.0 * (r1.ln() + r2.ln() * c * c)
            };
            let x = maxw(&mut p3);
            let y = match nb {
                3 => maxw(&mut p3),
                4 => {
                    let (r1, r2, r3) = (p3(), p3(), p3());
                    -(r1 * r2 * r3).ln()
                }
                _ => {
                    let (r1, r2, r3, r4, r5, r6) = (p3(), p3(), p3(), p3(), p3(), p3());
                    let c = (PI / 2.0 * r6).cos();
                    -(r1 * r2 * r3 * r4).ln() - r5.ln() * (c * c)
                }
            };
            let e = e_max * (x / (x + y));
            n += 1;
            ok += (e.to_bits() == s.eout.to_bits()) as usize;
        }
    }
    println!("N-body with std math: {ok}/{n} bit-identical");
    assert_eq!(ok, n);
}
