// SPDX-License-Identifier: GPL-3.0-only
//! # V&V — shortcut column (Fenske-Underwood-Gilliland) against upstream DWSIM (gh:#1356)
//!
//! **Methodology.** Upstream `ShortcutColumn.Calculate` (DWSIM
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`,
//! `DWSIM.UnitOperations/UnitOperations/ShortcutColumn.vb:149-515`, with
//! `BrentOpt.BrentMinimize.brentoptimize2` and Mapack `Matrix.Solve`) is
//! transcribed verbatim into
//! `verification_and_validation/upstream_differential_gh1356/fug_driver.py`
//! (route and limits: that folder's README). Upstream's own K-values need its
//! flash package, which is not reproduced, so the driver is **seeded with this
//! port's K-values** (`ColumnThermo::k_values`, stored as bit patterns in
//! `fug_inputs.txt` and asserted equal on every run): this compares the FUG
//! *algorithm*, not the K-values. C3/C4/C5/C6 (Poling constants), 2 packages
//! (Ideal, Peng-Robinson), 2 key pairs (C4/C5 gives the single-root Underwood
//! branch, C3/C6 the distributed-key branch), feed q = 0.5 and 0.8 (never 0 or 1,
//! so the enthalpy re-computation of q, which needs the flash package, is not
//! entered). Compared: D, B, xd, xb, Nmin, Rmin, N, optimal feed stage.
//!
//! **Bounds, derived before measuring** (README): 1e-12 relative on everything
//! except Rmin, which carries the Brent tolerance; fallback bound 1e-6 absolute
//! on Rmin if a Brent comparison flips.
//!
//! **Results (2026-11-13, release; 8 cases x 14 quantities = 112 comparisons).**
//! 110 bit-identical (`D`, `B`, all `xd`/`xb`, `Nmin`, `N`, optimal feed stage, and
//! `Rmin` in the 4 single-root and 2 of the 4 distributed-key cases). The two
//! that differ are `Rmin` of `pr_c3c6_q50` (1.3e-16 relative) and `pr_c3c6_q80`
//! (3.3e-15 relative, absolute 5.6e-17 on a value of -6.4e-3). Predictions held:
//! 1e-12 on everything but Rmin (measured 0), Rmin within 1e-6 (measured 5.6e-17).
//! **Localisation:** the Brent path is identical (single-root Rmin is bit-exact),
//! and re-running the upstream transcription with this port's `lu_solve`
//! (right-looking Gaussian elimination) in place of Mapack's `LuDecomposition`
//! (Crout, dot-product order) reproduces the port's two values bit for bit
//! (`fug_port_lu_control.py`). So the first divergent operation is the dense
//! solve in the distributed-key branch (`ShortcutColumn.vb:380`), an accepted
//! round-off-level difference (< 1e-8), flagged for the maintainer. **Upstream
//! behaviour noted, not a port defect:** both codes return a NEGATIVE `Rmin`
//! (-6.4e-3, -2.8e-2... see the q = 0.8 distributed rows) for a subcooled-liquid
//! feed in the distributed-key branch, and upstream's `Rmin > R` guard cannot
//! catch it.

use outram_park_fork_dwsim_libs::columns::shortcut::{ShortcutColumn, ShortcutFeed};
use outram_park_fork_dwsim_libs::columns::thermo_bridge::ColumnThermo;
use outram_park_fork_dwsim_libs::thermo::component::Component;
use outram_park_fork_dwsim_libs::thermo::property_package::PropertyPackageModel;
use outram_park_fork_dwsim_libs::columns::model::MolarFlowRate;
use uom::si::f64::{Pressure, Ratio, ThermodynamicTemperature};
use uom::si::catalytic_activity::katal as mole_per_second;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

const R: f64 = 8.314_462_618;

/// n-alkane from Poling, Prausnitz & O'Connell (2001), Appendix A (public
/// literature): Tc, Pc, omega, Tb. `cp_ig` is a flat 100 J/(mol K): the
/// duties are not compared here.
fn alkane(name: &str, m: f64, tc: f64, pc_bar: f64, w: f64, tb: f64) -> Component {
    Component::new(name, m, tc, pc_bar * 1e5, f64::NAN, w, tb, [100.0, 0.0, 0.0, 0.0, 0.0], f64::NAN)
        .expect("valid constants")
}

fn components() -> Vec<Component> {
    let _ = R;
    vec![
        alkane("propane", 0.044_096, 369.83, 42.48, 0.152, 231.11),
        alkane("n-butane", 0.058_123, 425.12, 37.96, 0.200, 272.65),
        alkane("n-pentane", 0.072_149, 469.7, 33.70, 0.251, 309.22),
        alkane("n-hexane", 0.086_175, 507.6, 30.25, 0.301, 341.88),
    ]
}

struct Case {
    name: String,
    package: PropertyPackageModel,
    lk: usize,
    hk: usize,
    q: f64,
}

fn cases() -> Vec<Case> {
    let mut v = Vec::new();
    for (pn, package) in [("ideal", PropertyPackageModel::Ideal), ("pr", PropertyPackageModel::PengRobinson)] {
        for (kn, lk, hk) in [("c4c5", 1, 2), ("c3c6", 0, 3)] {
            for (qn, q) in [("q50", 0.5), ("q80", 0.8)] {
                let name = format!("{pn}_{kn}_{qn}");
                v.push(Case { name, package, lk, hk, q });
            }
        }
    }
    v
}

const Z: [f64; 4] = [0.2, 0.3, 0.25, 0.25];
const T_FEED: f64 = 320.0;
const P_FEED: f64 = 101_325.0;
const F_FEED: f64 = 100.0;
const REFLUX: f64 = 8.0;
const XD_HK: f64 = 0.01; // heavy key in distillate
const XB_LK: f64 = 0.02; // light key in bottoms

fn column(c: &Case) -> ShortcutColumn {
    ShortcutColumn::new(components(), c.package, c.lk, c.hk)
        .expect("column")
        .with_key_specs(XB_LK, XD_HK)
        .with_reflux_ratio(Ratio::new::<ratio>(REFLUX))
}

fn feed(c: &Case) -> ShortcutFeed {
    ShortcutFeed::new(
        MolarFlowRate::new::<mole_per_second>(F_FEED),
        Z.to_vec(),
        ThermodynamicTemperature::new::<kelvin>(T_FEED),
        Pressure::new::<pascal>(P_FEED),
        Ratio::new::<ratio>(c.q),
    )
    .expect("feed")
}

fn hex(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

/// Prints the seed lines for `fug_inputs.txt`:
/// `cargo test ... emit_fug_inputs -- --ignored --nocapture | grep '^CASE '`.
#[test]
#[ignore = "generator for the driver's seed file, not a check"]
fn emit_fug_inputs() {
    for c in cases() {
        let th = ColumnThermo::new(components(), c.package);
        let k = th.k_values(&Z, &Z, T_FEED, P_FEED);
        println!(
            "CASE {} {} {} {} {} {} {} {} {} | {} | {}",
            c.name,
            c.lk,
            c.hk,
            hex(c.q),
            hex(F_FEED),
            hex(REFLUX),
            hex(XD_HK),
            hex(XB_LK),
            4,
            Z.iter().map(|v| hex(*v)).collect::<Vec<_>>().join(" "),
            k.iter().map(|v| hex(*v)).collect::<Vec<_>>().join(" ")
        );
    }
}

fn f(h: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(h, 16).expect("hex"))
}

#[test]
fn shortcut_column_matches_upstream_fug() {
    let inputs = include_str!("fixtures/upstream_gh1356/fug_inputs.txt");
    let outputs = include_str!("fixtures/upstream_gh1356/fug_upstream_output.txt");
    let mut worst_rel = 0.0_f64;
    let mut worst_rmin_abs = 0.0_f64;
    let (mut ncmp, mut nbit) = (0, 0);
    for c in cases() {
        // seed check: the K-values the upstream driver was given are still this port's.
        let line = inputs.lines().find(|l| l.starts_with(&format!("CASE {} ", c.name))).expect("seed line");
        let k_seed: Vec<f64> = line.split('|').nth(2).expect("K").split_whitespace().map(f).collect();
        let th = ColumnThermo::new(components(), c.package);
        let k = th.k_values(&Z, &Z, T_FEED, P_FEED);
        for (a, b) in k.iter().zip(&k_seed) {
            assert_eq!(a.to_bits(), b.to_bits(), "{}: K-values drifted from the seed", c.name);
        }
        // upstream results
        let out = outputs.lines().find(|l| l.starts_with(&format!("RESULT {} ", c.name))).expect("result line");
        let mut up = std::collections::HashMap::new();
        let mut it = out.split_whitespace().skip(2);
        while let (Some(key), Some(val)) = (it.next(), it.next()) {
            up.insert(key.to_string(), val.to_string());
        }
        let r = column(&c).solve(&feed(&c)).expect("port solve");
        let mut check = |key: &str, mine: f64| {
            let theirs = f(&up[key]);
            ncmp += 1;
            if mine.to_bits() == theirs.to_bits() {
                nbit += 1;
            }
            let rel = ((mine - theirs) / theirs).abs();
            if key == "Rmin" {
                worst_rmin_abs = worst_rmin_abs.max((mine - theirs).abs());
            } else {
                worst_rel = worst_rel.max(rel);
            }
            println!("{:<14} {:<7} port {:.17e} upstream {:.17e} rel {:.2e}", c.name, key, mine, theirs, rel);
        };
        check("D", r.distillate_flow.get::<mole_per_second>());
        check("B", r.bottoms_flow.get::<mole_per_second>());
        for i in 0..4 {
            check(&format!("xd{i}"), r.distillate_composition[i]);
            check(&format!("xb{i}"), r.bottoms_composition[i]);
        }
        check("Nmin", r.minimum_stages);
        check("Rmin", r.minimum_reflux_ratio.get::<ratio>());
        check("N", r.actual_stages);
        check("ofs", r.optimal_feed_stage);
        assert_eq!(up["mode"], if matches!(r.underwood_mode, outram_park_fork_dwsim_libs::columns::shortcut::UnderwoodMode::SingleRoot) { "single" } else { "distributed" }, "{}: Underwood branch", c.name);
    }
    println!("FUG: {nbit}/{ncmp} bit-identical, worst rel (non-Rmin) {worst_rel:e}, worst |dRmin| {worst_rmin_abs:e}");
    assert!(worst_rel < 1e-12, "gate 1e-12 breached: {worst_rel:e}");
    assert!(worst_rmin_abs < 1e-6, "Rmin fallback bound breached: {worst_rmin_abs:e}");
    // Pinned 2026-11-13 (see RESULTS in the module docs).
    assert_eq!((nbit, ncmp), (110, 112), "bit-identical count moved");
}
