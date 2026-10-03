// SPDX-License-Identifier: GPL-3.0

//! **The nu and yield tables are evaluated on their own interpolation regions,
//! as OpenMC's `Tabulated1D` evaluates them.** This is part of the GitHub #365
//! audit.
//!
//! # What was missing
//!
//! | table | before | now |
//! |---|---|---|
//! | ACE NU (total, and prompt+total) | non-lin-lin region refused | carried |
//! | ACE DNU | regions dropped silently, read lin-lin | carried |
//! | ACE BDD group probabilities | regions dropped silently | carried |
//! | ACE `\|TY\| > 100` yields (MT=5, other channels) | non-lin-lin refused | carried |
//! | ENDF MF=1/452, 455, MF=5/455 `p_k`, MF=6 yields | regions dropped silently | carried |
//!
//! Carried regions are honoured by one evaluator,
//! `njoy_outram_park_fork::nuclear_data::secondary::tabulated1d_at`, which is
//! OpenMC's `Tabulated1D::operator()` (`src/endf.cpp`): the regions, `INT` 1–5,
//! clamping outside the table, and the `lower_bound_index` bin. A lin-lin table
//! keeps its old arithmetic, so it evaluates bit-identically.
//!
//! # Methodology
//!
//! Every nu table in ENDF/B-VIII.0 is lin-lin. The census on 2026-09-29 found
//! 86 MT=452, 86 MT=456 and 84 MT=455 tables. The one non-lin-lin neutron
//! yield is F-19's histogram, and it is constant. So
//! `verification_and_validation/ace_route_physics/openmc_inputs/interp_regions_reference.py`
//! **constructs** `target/ace_extra/U235_regions.ace` from NJOY2016 U-235:
//! - NU as prompt+total with five regions, `INT` 1–5;
//! - DNU with `INT` 4, 1;
//! - BDD group 1 energy-dependent with `INT` 5, 3;
//! - the MT=5 yield with `INT` 1, 5, 2.
//!
//! OpenMC's reader of the file must see those regions (the script asserts it).
//! The reference values follow OpenMC's C++ rule. The script re-implements it,
//! and it agrees with Python's `Tabulated1D` to 0.0 at every interior probe.
//! Probes: 12 midpoints spread over each table, plus one point below and one
//! above it.
//!
//! Pass: `|rel| <= 1e-12` on all 56 values, through the transport path:
//! `Nuclide::nu_bar`, `DelayedData::nu_delayed_at`, `group_fraction`,
//! `Nuclide::mt5_yield`. **Power:** in every quantity, at least one probe must
//! differ from the lin-lin reading (the old behaviour) by more than 1e-4
//! relative.
//!
//! **Group shares.** The reference is `p_g(E) / sum_g p_g(E_first)`, the
//! evaluated table as renormalised here. For an energy-dependent `p_g`,
//! OpenMC's ACE reader instead forms `nu_d(E) p_g(E)` on the union grid and
//! re-tabulates it lin-lin (`reaction.py:338-347`). That is an approximation of
//! the product, not a reading of the table, and it is deliberately not
//! reproduced. No held table has an energy-dependent `p_g` (U-235's six are
//! constant).
//!
//! # Results (2026-09-29)
//!
//! All 56 values agree with OpenMC's rule to ≤ 1e-12. Worst relative difference
//! from the lin-lin reading, per quantity, is printed by the test and recorded
//! in `ace_route_physics/interp_regions_2026-09-29.md`.

use outram_mc_libs::material::nuclide::Nuclide;
use std::path::PathBuf;

#[test]
fn nu_and_yield_regions_evaluate_as_openmc() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ace = root.join("../../target/ace_extra/U235_regions.ace");
    if !ace.is_file() {
        println!("{} absent: skipping", ace.display());
        return;
    }
    let nuc = Nuclide::from_ace_file(&ace, "U235").expect("a table with regions must load");
    let d = nuc.delayed().expect("delayed data");
    let csv =
        root.join("verification_and_validation/ace_route_physics/data/interp_regions_openmc.csv");
    let text = std::fs::read_to_string(csv).expect("reference");
    let mut worst: f64 = 0.0;
    let mut power: Vec<(String, f64)> = Vec::new();
    let mut n = 0;
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let f: Vec<&str> = line.split(',').collect();
        let (q, e, want, lin): (&str, f64, f64, f64) = (
            f[0],
            f[1].parse().unwrap(),
            f[2].parse().unwrap(),
            f[3].parse().unwrap(),
        );
        let got = match q {
            "nu_total" => nuc.nu_bar(e),
            "nu_delayed" => d.nu_delayed_at(e),
            "group1_share" => d.group_fraction(0, e),
            "mt5_yield" => nuc.mt5_yield(e),
            other => panic!("unknown quantity {other}"),
        };
        // A zero reference (MT=5's yield below its threshold) is compared
        // absolutely: a relative difference of 0/0 is not a number.
        let scale = if want == 0.0 { 1.0 } else { want.abs() };
        let rel = (got - want).abs() / scale;
        let sep = (lin - want).abs() / scale;
        println!("{q:13} E = {e:.4e}: {got:.12} (OpenMC rule {want:.12}) rel {rel:.1e}; lin-lin off by {sep:.1e}");
        assert!(rel <= 1e-12, "{q} at {e}: {got} vs {want}");
        worst = worst.max(rel);
        match power.iter_mut().find(|(k, _)| k == q) {
            Some(p) => p.1 = p.1.max(sep),
            None => power.push((q.to_string(), sep)),
        }
        n += 1;
    }
    println!("{n} values, worst rel {worst:.1e}; largest lin-lin error per quantity {power:?}");
    assert_eq!(n, 56);
    for (q, sep) in &power {
        assert!(
            *sep > 1e-4,
            "{q}: the probes cannot tell the regions from lin-lin"
        );
    }
}

/// The ENDF route carries MF=6 yield regions: F-19's MT=16/22/28/91 yields are
/// histograms (the only non-lin-lin neutron yields in ENDF/B-VIII.0). A
/// histogram on a constant table evaluates the same as lin-lin, so this pins
/// the carrying, not a number.
#[test]
fn endf_route_carries_f19_histogram_yield() {
    let Some(p) = njoy_outram_park_fork::reference_data::reference_endf("n-009_F_019.endf") else {
        println!("F-19 tape absent: skipping");
        return;
    };
    let nuc = Nuclide::from_endf_file(&p, "F19", 293.6, 1.0e-3).expect("F-19 loads");
    let law = nuc.continuum_law(91).expect("MT=91 law");
    let r = &law.branches[0].yield_interp;
    println!("F-19 MT=91 yield regions {r:?}");
    assert!(!r.is_empty() && r.iter().all(|&(_, int)| int == 1), "{r:?}");
}
