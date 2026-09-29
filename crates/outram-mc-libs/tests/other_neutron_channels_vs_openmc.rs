// SPDX-License-Identifier: GPL-3.0

//! **The other neutron-emitting reactions are transported, on both routes, as
//! OpenMC transports them** — GitHub #365 audit.
//!
//! # What was wrong
//!
//! Every non-redundant reaction that emits a neutron but has no arm of its own
//! in the collision kernels sat inside MT=1 and was sampled as **elastic**, and
//! its extra neutrons were lost. That covered (n,n alpha) MT=22, (n,np) MT=28,
//! (n,nd) MT=32, (n,4n) MT=37, (n,2np) MT=41, (n,2n alpha) MT=24 and the rest.
//! OpenMC samples each one with its own product law, frame and multiplicity
//! (`src/physics.cpp`, `sample_scatter` → `inelastic_scatter`). They now have
//! an arm in every kernel (`physics::keff` ×3, `transport_csg`, `keff_delta`),
//! through `Nuclide::sample_other_emission`. It is on by default;
//! `without_other_neutron_channels()` is the ablation.
//!
//! # Methodology
//!
//! Reference: OpenMC 0.16.1.dev25's readers, via
//! `verification_and_validation/ace_route_physics/openmc_inputs/other_channels_reference.py`
//! → `data/other_channels_openmc.csv`. It lists OpenMC's non-redundant
//! reactions with a neutron product, minus those with their own arm here, with
//! each reaction's cross section, neutron yield and frame at 14 and 19 MeV:
//! - **ACE rows** come from the NJOY2016 tables of O-16, F-19, Al-27, Si-28,
//!   Mn-55 and U-234 (five-route study) and Li-7 (`target/ace_extra`);
//! - **ENDF rows** come from OpenMC's ENDF reader of O-16, F-19 and Li-7.
//!
//! 1. **Channel set**: identical to OpenMC's, per nuclide and route.
//! 2. **Cross section** (ACE rows): equal to 1e-10 relative.
//! 3. **Multiplicity and frame**: equal to OpenMC's yield (1e-12), except that an
//!    ENDF MF=4 + MF=5 reaction is compared with the ACE route, because OpenMC's
//!    ENDF reader leaves such a reaction's yield at the default 1; the CM flag
//!    equals OpenMC's on the ACE route and the ACE route's on the ENDF route
//!    (OpenMC's ENDF reader keeps an MF=6 frame on the product, not the
//!    reaction).
//! 4. **Selection**: on F-19 at 18 MeV, `sample_other_emission` picks MT=22 at
//!    the rate `sigma_22 / (sigma_22 + sigma_28)`, within 5 binomial sigma
//!    (`N = 2e5`).
//! 5. **Law, ACE route vs ENDF route**: two-sample KS on `E'` of MT=22 draws on
//!    F-19 and O-16 at 18 MeV, and of MT=24 draws on Li-7 at 14 MeV.
//!    Critical value `1.95 sqrt(1/n1 + 1/n2)` (alpha = 0.001).
//!
//! 6. **Partition closure**: on O-16 and Al-27 (ACE), `elastic + inelastic +
//!    n2n + n3n + mt5 + other + absorption` equals `total` to 1e-6 relative at
//!    10–20 MeV. With `without_other_neutron_channels` it does not, which is
//!    what the kernel used to see.
//!
//! The ACE files are regenerable scratch; the test skips when they are absent.
//!
//! # Results (2026-09-29)
//!
//! Printed by the test and recorded in the #365 thread.

use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::speed::SpeedTier;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

fn ws() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn ace_path(n: &str) -> PathBuf {
    if n == "Li7" {
        ws().join("target/ace_extra/Li7/tape24")
    } else {
        ws().join(format!("target/five_route_keff/njoy/293.6K/{n}.ace"))
    }
}

fn endf_file(n: &str) -> &'static str {
    match n {
        "O16" => "n-008_O_016-ENDF8.0.endf",
        "F19" => "n-009_F_019-ENDF8.0.endf",
        "Li7" => "n-003_Li_007-ENDF8.0.endf",
        _ => unreachable!(),
    }
}

fn ks_two(a: &mut [f64], b: &mut [f64]) -> f64 {
    a.sort_by(|x, y| x.partial_cmp(y).unwrap());
    b.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let (na, nb) = (a.len() as f64, b.len() as f64);
    let (mut i, mut j, mut d) = (0usize, 0usize, 0.0f64);
    while i < a.len() && j < b.len() {
        let x = a[i].min(b[j]);
        while i < a.len() && a[i] <= x {
            i += 1;
        }
        while j < b.len() && b[j] <= x {
            j += 1;
        }
        d = d.max((i as f64 / na - j as f64 / nb).abs());
    }
    d
}

fn draws(n: &Nuclide, mt: i32, e: f64, count: usize, seed0: u64) -> Vec<f64> {
    let mut seed = seed0;
    let u = Direction::new(0.0, 0.0, 1.0);
    let mut out = Vec::new();
    while out.len() < count {
        let o = n.sample_other_emission(e, u, &mut seed);
        if o.mt == mt {
            out.push(o.e);
        }
    }
    out
}

#[test]
fn other_channels_match_openmc_on_both_routes() {
    if !ace_path("O16").is_file() || !ace_path("Li7").is_file() {
        println!("ACE tables absent: skipping (see the module doc)");
        return;
    }
    let csv = ws().join(
        "crates/outram-mc-libs/verification_and_validation/ace_route_physics/data/other_channels_openmc.csv",
    );
    let text = std::fs::read_to_string(csv).expect("reference");
    let mut ace: BTreeMap<String, Nuclide> = BTreeMap::new();
    let mut endf: BTreeMap<String, Nuclide> = BTreeMap::new();
    let mut want_sets: BTreeMap<(String, String), BTreeSet<i32>> = BTreeMap::new();
    let mut rows = 0;
    let mut endf_mf45: Vec<(String, i32, f64, f64)> = Vec::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = line.split(',').collect();
        let (route, name, mt) = (f[0], f[1].to_string(), f[2].parse::<i32>().unwrap());
        let e: f64 = f[3].parse().unwrap();
        let (sig, y, cm): (f64, f64, bool) =
            (f[4].parse().unwrap(), f[5].parse().unwrap(), f[6] == "1");
        want_sets
            .entry((route.into(), name.clone()))
            .or_default()
            .insert(mt);
        let nuc = if route == "ace" {
            ace.entry(name.clone())
                .or_insert_with(|| Nuclide::from_ace_file(ace_path(&name), &name).expect("ace"))
        } else {
            endf.entry(name.clone()).or_insert_with(|| {
                let t = reference_endf(endf_file(&name)).expect("tape");
                Nuclide::from_endf_file_with_speed(&t, &name, 293.6, SpeedTier::VeryFast)
                    .expect("endf")
            })
        };
        let got_y = nuc
            .other_channel_yield(mt, e)
            .unwrap_or_else(|| panic!("{route} {name}: MT={mt} missing"));
        let kind = nuc
            .other_neutron_channels()
            .into_iter()
            .find(|c| c.0 == mt)
            .unwrap()
            .1;
        // OpenMC's ENDF reader gives an MF=4 + MF=5 reaction no multiplicity at
        // all -- its `Product.yield_` keeps the default `Polynomial((1,))`
        // (reaction.py:1189-1203), 1 even for Li-7's (n,2n alpha). For those
        // reactions the ENDF route is checked against the ACE route instead
        // (ACER sets TY from the same MT table), after this loop.
        if route == "endf" && kind == "uncorrelated" {
            endf_mf45.push((name.clone(), mt, e, got_y));
        } else {
            assert!(
                (got_y - y).abs() < 1e-12,
                "{route} {name} MT={mt} E={e}: yield {got_y} vs {y}"
            );
        }
        // Frame against OpenMC for ACE rows only: on the ENDF route OpenMC
        // carries an MF=6 frame on the *product* (LCT), and its
        // `Reaction.center_of_mass` keeps the default `True` (reaction.py:849)
        // for those reactions, so it is not the frame. The ENDF route's frame
        // is checked against the ACE route's instead (ACER derives TY's sign
        // from the same LCT), below.
        if route == "ace" {
            assert_eq!(
                nuc.other_channel_cm(mt),
                Some(cm),
                "{route} {name} MT={mt}: frame"
            );
        }
        if route == "ace" {
            let got = nuc.reaction_xs(mt, e).unwrap();
            let rel = if sig > 0.0 {
                ((got - sig) / sig).abs()
            } else {
                got.abs()
            };
            assert!(
                rel < 1e-10,
                "{route} {name} MT={mt} E={e}: sigma {got} vs {sig}"
            );
        }
        rows += 1;
    }
    for ((route, name), want) in &want_sets {
        let nuc = if route == "ace" {
            &ace[name]
        } else {
            &endf[name]
        };
        let got: BTreeSet<i32> = nuc
            .other_neutron_channels()
            .into_iter()
            .map(|c| c.0)
            .collect();
        assert_eq!(&got, want, "{route} {name}: channel set");
        assert!(nuc.applies_other_neutron_channels());
        println!("{route} {name}: channels {got:?} equal to OpenMC's");
    }
    println!("{rows} (route, nuclide, MT, E) rows: sigma, yield and frame equal to OpenMC");
    for (name, mt, e, y) in &endf_mf45 {
        let ace_y = ace[name].other_channel_yield(*mt, *e).unwrap();
        assert!(
            (y - ace_y).abs() < 1e-12,
            "endf {name} MT={mt}: yield {y} vs ACE route {ace_y}"
        );
        println!("endf {name} MT={mt} E={e:.1e}: yield {y} (MF=4+5; equal to the ACE route's)");
    }
    for (name, e_nuc) in &endf {
        for (mt, _) in e_nuc.other_neutron_channels() {
            assert_eq!(
                e_nuc.other_channel_cm(mt),
                ace[name].other_channel_cm(mt),
                "{name} MT={mt}: ENDF-route frame differs from the ACE route's"
            );
        }
    }

    // 4. Selection frequency on F-19 at 18 MeV.
    let f19 = &ace["F19"];
    let (s22, s28) = (
        f19.reaction_xs(22, 1.8e7).unwrap(),
        f19.reaction_xs(28, 1.8e7).unwrap(),
    );
    let p = s22 / (s22 + s28);
    let n = 200_000;
    let mut seed = 0x0773_0000u64;
    let u = Direction::new(0.0, 0.0, 1.0);
    let hits = (0..n)
        .filter(|_| f19.sample_other_emission(1.8e7, u, &mut seed).mt == 22)
        .count();
    let (fr, sg) = (hits as f64 / n as f64, (p * (1.0 - p) / n as f64).sqrt());
    println!(
        "F-19 18 MeV: MT=22 chosen {fr:.5} vs sigma share {p:.5} (z {:+.2})",
        (fr - p) / sg
    );
    assert!(((fr - p) / sg).abs() < 5.0);

    // 6. Partition closure with the other channels, and without.
    for name in ["O16", "Al27"] {
        let n = &ace[name];
        let off = n.clone().without_other_neutron_channels();
        let (mut worst_on, mut worst_off) = (0.0f64, 0.0f64);
        for k in 0..11 {
            let e = 1.0e7 + 1.0e6 * k as f64;
            for (nuc, w) in [(n, &mut worst_on), (&off, &mut worst_off)] {
                let x = nuc.xs_at_energy(e, 293.6);
                let parts =
                    x.elastic + x.inelastic + x.n2n + x.n3n + x.mt5 + x.other + x.absorption;
                *w = w.max(((x.total - parts) / x.total).abs());
            }
        }
        println!("{name}: partition closure 10-20 MeV {worst_on:.2e} (ablated: {worst_off:.2e})");
        assert!(
            worst_on < 1e-6,
            "{name}: partition does not close ({worst_on:.2e})"
        );
        assert!(
            worst_off > 1e-3,
            "{name}: the other channels must be visible in the partition"
        );
    }

    // 5. Laws, ACE vs ENDF.
    for (name, mt, e) in [("F19", 22, 1.8e7), ("O16", 22, 1.8e7), ("Li7", 24, 1.4e7)] {
        let mut a = draws(&ace[name], mt, e, 60_000, 0xACE0_0000 + mt as u64);
        let mut b = draws(&endf[name], mt, e, 60_000, 0xE0F0_0000 + mt as u64);
        let (ma, mb) = (
            a.iter().sum::<f64>() / a.len() as f64,
            b.iter().sum::<f64>() / b.len() as f64,
        );
        let crit = 1.95 * (1.0 / a.len() as f64 + 1.0 / b.len() as f64).sqrt();
        let d = ks_two(&mut a, &mut b);
        println!(
            "{name} MT={mt} {:.0} MeV: <E'> ACE {:.4} / ENDF {:.4} MeV, KS D {d:.2e} (crit {crit:.2e})",
            e / 1e6,
            ma / 1e6,
            mb / 1e6
        );
        assert!(d < crit, "{name} MT={mt}: ACE vs ENDF KS {d}");
    }
}
