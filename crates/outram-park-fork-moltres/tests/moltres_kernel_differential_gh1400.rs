// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
//! # Kernel-level differential against Moltres 3dd2ce7 (gh:#1400)
//!
//! **Methodology.** `verification_and_validation/moltres_differential_gh1400/
//! dump_moltres_kernels.cpp` transcribes the Moltres kernels
//! (`InScatter`, `CoupledFissionKernel`, `PrecursorSource`,
//! `DelayedNeutronSource`, `PrecursorDecay`) and the MOOSE
//! `LinearInterpolation` used by `GenericMoltresMaterial` into standalone C++
//! (lines marked `UPSTREAM_*` are grepped against the pinned trees by
//! `check_verbatim.sh`). It evaluates each at one quadrature point on
//! deterministic pseudo-random inputs and prints inputs and outputs as u64
//! bit patterns to `reference_kernels.txt`. This test replays every input
//! through the crate's own functions and compares **bits**.
//!
//! Not reproduced by the C++ shim: MOOSE's quadrature and shape functions,
//! libMesh, PETSc; `_test` is 1. Details and results:
//! `verification_and_validation/moltres_differential_gh1400/RESULTS.md`.
//!
//! **Pass criteria.** 100 % bit-identical for table interpolation (value and
//! slope), `InScatter`, `DelayedNeutronSource`, `PrecursorDecay` and the
//! prompt-fission term; for `PrecursorSource` (association differs, see
//! `terms::precursor_production`) the worst difference must be <= 8 ulp
//! and is printed.

use std::collections::BTreeMap;
use std::sync::Arc;

use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
use outram_foam_basic_lib::prelude::{FvMesh, VolScalarField};
use outram_park_fork_moltres::diffusion::{fission_source, scattering_source};
use outram_park_fork_moltres::materials::{DelayedFamily, MsrMaterial, XsFields};
use outram_park_fork_moltres::precursors::PrecursorDrift;
use outram_park_fork_moltres::ring_mesh::RingMesh;
use outram_park_fork_moltres::tables::{InterpType, MoltresTables, TableLayout};
use outram_park_fork_moltres::terms::{precursor_production, prompt_fission_source};
use uom::si::area::square_meter;
use uom::si::f64::{Area, Length};
use uom::si::length::meter;

const REF: &str = include_str!(
    "../verification_and_validation/moltres_differential_gh1400/reference_kernels.txt"
);
const DATA: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/verification_and_validation/moltres_differential_gh1400/data/newt_fuel_"
);

fn f(h: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(h, 16).unwrap())
}

/// Count of bit-identical results and the worst ulp distance.
#[derive(Default, Debug)]
struct Stat {
    n: usize,
    identical: usize,
    worst_ulp: u64,
}

impl Stat {
    fn add(&mut self, port: f64, up: f64) {
        self.n += 1;
        if port.to_bits() == up.to_bits() || (port == 0.0 && up == 0.0) {
            self.identical += 1;
            return;
        }
        let a = port.to_bits() as i64;
        let b = up.to_bits() as i64;
        self.worst_ulp = self.worst_ulp.max((a - b).unsigned_abs());
    }
}

fn lines(tag: &str) -> Vec<Vec<&'static str>> {
    REF.lines()
        .filter(|l| l.starts_with(tag))
        .map(|l| l.split_whitespace().skip(1).collect())
        .collect()
}

fn mesh(n: usize) -> Arc<FvMesh> {
    Arc::new(
        create_one_d_mesh(
            Length::new::<meter>(1.0),
            Area::new::<square_meter>(1.0),
            n as i64,
        )
        .unwrap(),
    )
}

fn set(field: &mut VolScalarField, vals: &[f64]) {
    for (d, s) in field.internal.as_mut_slice().iter_mut().zip(vals) {
        *d = *s;
    }
}

/// K1: table interpolation, value and slope, 12 files x columns x 200 T.
#[test]
fn k1_table_interpolation_is_bit_identical() {
    let tabs = MoltresTables::load(DATA, 2, 6, TableLayout::Legacy, InterpType::Linear).unwrap();
    let mut stat = Stat::default();
    let mut slope = Stat::default();
    let mut seen = 0usize;
    for l in lines("TBL") {
        let name = match l[0] {
            "CHI" => "CHI_T",
            "RECIPVEL" => continue, // not carried by the port (transient only)
            n => n,
        };
        let k: usize = l[1].parse().unwrap();
        let t = f(l[2]);
        let (v, s) = tabs.raw_sample(name, k, t).unwrap();
        stat.add(v, f(l[3]));
        slope.add(s, f(l[4]));
        seen += 1;
    }
    println!("[K1] value {stat:?} slope {slope:?}");
    assert!(seen > 1500, "only {seen} samples");
    assert_eq!(stat.identical, stat.n);
    assert_eq!(slope.identical, slope.n);
}

fn scatter_materials(g: usize, rows: &[(usize, Vec<f64>)], layout_sss2: bool) -> Vec<MsrMaterial> {
    rows.iter()
        .map(|(_, gt)| {
            let mut sc = vec![vec![0.0; g]; g];
            for from in 0..g {
                for to in 0..g {
                    if from != to {
                        let idx = if layout_sss2 {
                            from * g + to
                        } else {
                            from + to * g
                        };
                        sc[from][to] = gt[idx];
                    }
                }
            }
            let mut m = MsrMaterial::non_fuel("s", vec![0.01; g], vec![1.0; g]);
            m.scattering = sc;
            m
        })
        .collect()
}

/// K2: `InScatter`, G = 3, both index layouts.
#[test]
fn k2_inscatter_is_bit_identical() {
    for sss2 in [0usize, 1] {
        let recs: Vec<Vec<&str>> = lines("INSC")
            .into_iter()
            .filter(|l| l[0] == sss2.to_string())
            .collect();
        let n = recs.len();
        let rows: Vec<(usize, Vec<f64>)> = recs
            .iter()
            .map(|l| {
                (
                    l[1].parse().unwrap(),
                    l[2..11].iter().map(|h| f(h)).collect(),
                )
            })
            .collect();
        let mats = scatter_materials(3, &rows, sss2 == 1);
        let zones: Vec<usize> = (0..n).collect();
        let m = mesh(n);
        let xs = XsFields::materialize(m.clone(), &zones, &mats).unwrap();
        let mut flux: Vec<VolScalarField> = (0..3)
            .map(|g| VolScalarField::uniform(format!("p{g}"), m.clone(), 1.0))
            .collect();
        for g in 0..3 {
            let v: Vec<f64> = recs.iter().map(|l| f(l[11 + g])).collect();
            set(&mut flux[g], &v);
        }
        let by_group: Vec<Vec<f64>> = (0..3).map(|g| scattering_source(&xs, &flux, g)).collect();
        let mut stat = Stat::default();
        for (c, l) in recs.iter().enumerate() {
            let g = rows[c].0;
            let up = f(l[14]);
            stat.add(by_group[g][c], -up);
        }
        println!("[K2 sss2={sss2}] {stat:?}");
        assert_eq!(stat.identical, stat.n);
    }
}

fn fissile(nsf: Vec<f64>) -> MsrMaterial {
    let g = nsf.len();
    let mut m = MsrMaterial::non_fuel("f", vec![0.01; g], vec![1.0; g]);
    m.nu_sigma_f = nsf;
    m.chi_prompt = (0..g).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect();
    m.chi_delayed = m.chi_prompt.clone();
    m
}

/// K5: `CoupledFissionKernel` prompt term (`account_delayed = true`).
/// Also prints the pre-fix figure for the legacy expression
/// `chi_p * (1 - beta) / k * S`.
#[test]
fn k5_prompt_fission_is_bit_identical_after_translation() {
    for g in [2usize, 3] {
        let recs: Vec<Vec<&str>> = lines("FISS")
            .into_iter()
            .filter(|l| l[0] == g.to_string())
            .collect();
        let n = recs.len();
        let mats: Vec<MsrMaterial> = recs
            .iter()
            .map(|l| fissile(l[2..2 + g].iter().map(|h| f(h)).collect()))
            .collect();
        let zones: Vec<usize> = (0..n).collect();
        let m = mesh(n);
        let xs = XsFields::materialize(m.clone(), &zones, &mats).unwrap();
        let mut flux: Vec<VolScalarField> = (0..g)
            .map(|i| VolScalarField::uniform(format!("p{i}"), m.clone(), 1.0))
            .collect();
        for i in 0..g {
            let v: Vec<f64> = recs.iter().map(|l| f(l[2 + g + i])).collect();
            set(&mut flux[i], &v);
        }
        let sf = fission_source(&xs, &flux);
        let mut now = Stat::default();
        let mut legacy = Stat::default();
        for (c, l) in recs.iter().enumerate() {
            let chi = f(l[2 + 2 * g]);
            let beta = f(l[3 + 2 * g]);
            let k = f(l[4 + 2 * g]);
            let up = -f(l[5 + 2 * g]);
            let s = sf.internal.as_slice()[c];
            now.add(prompt_fission_source(s, beta, chi, k), up);
            legacy.add(chi * (1.0 - beta) / k * s, up);
        }
        println!("[K5 G={g}] translated {now:?}; legacy expression {legacy:?}");
        assert_eq!(now.identical, now.n);
    }
}

/// K6: `PrecursorSource` -- association differs for G >= 2 (documented).
#[test]
fn k6_precursor_source_is_within_a_few_ulp() {
    for g in [2usize, 3] {
        let recs: Vec<Vec<&str>> = lines("PSRC")
            .into_iter()
            .filter(|l| l[0] == g.to_string())
            .collect();
        let n = recs.len();
        let mats: Vec<MsrMaterial> = recs
            .iter()
            .map(|l| fissile(l[2..2 + g].iter().map(|h| f(h)).collect()))
            .collect();
        let zones: Vec<usize> = (0..n).collect();
        let m = mesh(n);
        let xs = XsFields::materialize(m.clone(), &zones, &mats).unwrap();
        let mut flux: Vec<VolScalarField> = (0..g)
            .map(|i| VolScalarField::uniform(format!("p{i}"), m.clone(), 1.0))
            .collect();
        for i in 0..g {
            let v: Vec<f64> = recs.iter().map(|l| f(l[2 + g + i])).collect();
            set(&mut flux[i], &v);
        }
        let sf = fission_source(&xs, &flux);
        let mut stat = Stat::default();
        for (c, l) in recs.iter().enumerate() {
            let beta = f(l[1]);
            let k = f(l[2 + 2 * g]);
            let up = -f(l[3 + 2 * g]);
            stat.add(precursor_production(beta, k, sf.internal.as_slice()[c]), up);
        }
        println!("[K6 G={g}] {stat:?}");
        assert!(stat.worst_ulp <= 8, "{stat:?}");
    }
}

fn tiny_drift(families: Vec<DelayedFamily>) -> (RingMesh, PrecursorDrift) {
    let ring = RingMesh::new(1.0, 1.0, 3).unwrap();
    let flow = ring.uniform_flux(0.0);
    let drift = PrecursorDrift::new(
        ring.mesh.clone(),
        families,
        flow,
        0.0,
        outram_foam_basic_lib::prelude::SolverSettings {
            tolerance: 1e-10,
            max_iter: 100,
        },
    )
    .unwrap();
    (ring, drift)
}

/// K3 and K4: `DelayedNeutronSource` (6 families) and `PrecursorDecay`.
#[test]
fn k3_k4_delayed_source_and_decay_are_bit_identical() {
    let mut dns = Stat::default();
    for l in lines("DNS") {
        let chi = f(l[0]);
        let lam: Vec<f64> = l[1..7].iter().map(|h| f(h)).collect();
        let conc: Vec<f64> = l[7..13].iter().map(|h| f(h)).collect();
        let up = f(l[13]);
        let fams: Vec<DelayedFamily> = lam
            .iter()
            .map(|lambda| DelayedFamily {
                beta: 0.001,
                lambda: *lambda,
            })
            .collect();
        let (ring, drift) = tiny_drift(fams);
        let fields: Vec<VolScalarField> = conc
            .iter()
            .map(|c| VolScalarField::uniform("c", ring.mesh.clone(), *c))
            .collect();
        let sd = drift.delayed_source(&fields);
        dns.add(chi * sd.internal.as_slice()[0], -up);
    }
    let mut dec = Stat::default();
    for l in lines("DEC") {
        let lam = f(l[0]);
        let c = f(l[1]);
        let up = f(l[2]);
        let (ring, drift) = tiny_drift(vec![DelayedFamily {
            beta: 0.001,
            lambda: lam,
        }]);
        let fields = vec![VolScalarField::uniform("c", ring.mesh.clone(), c)];
        dec.add(drift.delayed_source(&fields).internal.as_slice()[0], up);
    }
    println!("[K3] {dns:?} [K4] {dec:?}");
    assert_eq!(dns.identical, dns.n);
    assert_eq!(dec.identical, dec.n);
}

/// Sanity: the reference file holds every record kind the tests expect.
#[test]
fn reference_file_has_expected_record_counts() {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for l in REF.lines() {
        *counts
            .entry(l.split_whitespace().next().unwrap())
            .or_default() += 1;
    }
    assert_eq!(counts["INSC"], 2 * 4096);
    assert_eq!(counts["FISS"], 2 * 4096);
    assert_eq!(counts["PSRC"], 2 * 4096);
    assert_eq!(counts["DNS"], 4096);
    assert_eq!(counts["DEC"], 4096);
    assert_eq!(counts["TBL"], 30 * 200);
}
