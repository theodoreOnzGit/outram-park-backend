//! **What `process` would change in route 4's data: the precision question of
//! GitHub #929** (epic #925 §5). A measurement, not a gate; the decision is
//! the maintainer's.
//!
//! # Methodology
//!
//! For each nuclide, RECONR runs once at 0 K (tolerance 0.001) and both
//! routes are built from that one result, so RECONR cannot differ:
//!
//! - **route 4** as transport built it on 2026-10-17 before gh:#931 (since
//!   then route 4 *is* `process`; this chain is `process_evaluation_pre_931`):
//!   `Nuclide::process_from_stages(&recon0, None, 293.6, 0.001)`, i.e.
//!   `broaden_result_with_tolerance` plus the raw 0 K MT=2 below
//!   `DBRC_GRID_MAX_EV` (25 keV) for DBRC;
//! - **`process`**: `process::cross_sections_at` on a `Reconstruction`
//!   holding the same `recon0`, at 293.6 K, PURR and HEATR off (neither
//!   changes a PENDF section).
//!
//! Compared bit for bit: (1) every broadened section, point by point; (2) the
//! 0 K elastic, `process`'s cut at the same 25 keV. Reported per nuclide: the
//! points compared, the points that differ, and the largest relative
//! difference in energy and in cross section.
//!
//! Run: `cargo run --release -p outram-mc-libs --example
//! process_precision_vs_route4 [NAME:TAPE ...]` (default: the 13 five-route
//! nuclides). Tapes from `reference-data/endf/`.
//!
//! No test reaches `main`: it is CLI glue around two library calls that are
//! tested in their own crates.
//!
//! Then, per nuclide, 2 000 000 DBRC free-gas draws (1e-2 to 1e3 eV,
//! 293.6 K) on a table built from each 0 K elastic with the same seeds, and
//! a control with DBRC off.
//!
//! # Results (2026-10-17)
//!
//! Broadened sections: 0 of 5 067 756 points differ. 0 K elastic: all
//! 780 022 points differ, by 1.0e-13 relative (one O-16 energy by 3.7e-7).
//! DBRC draws: 0 of 26 000 000 differ; the control differs on all. Full
//! table and interpretation:
//! `verification_and_validation/process_precision_929/README.md`.

use petir::mathf::RealMath;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::process::{self, CrossSectionSettings, Reconstruction};
use njoy_outram_park_fork::reference_data::reference_endf;
use njoy_outram_park_fork::units::Temperature;
use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::material::nuclide::{Nuclide, DBRC_GRID_MAX_EV};
use outram_mc_libs::physics::scatter::{free_gas_elastic_scatter_dbrc, DbrcTable};
use std::sync::Arc;
use uom::si::thermodynamic_temperature::kelvin;

/// Free-gas DBRC draws per nuclide in the flip count.
const DRAWS: usize = 2_000_000;

const DEFAULT: [(&str, &str); 13] = [
    ("U234", "n-092_U_234-ENDF8.0.endf"),
    ("U235", "n-092_U_235-ENDF8.0.endf"),
    ("U238", "n-092_U_238.endf"),
    ("U236", "n-092_U_236-ENDF8.0.endf"),
    ("F19", "n-009_F_019-ENDF8.0.endf"),
    ("O16", "n-008_O_016-ENDF8.0.endf"),
    ("H1", "n-001_H_001-ENDF8.0-Beta6.endf"),
    ("Al27", "n-013_Al_027-ENDF8.0.endf"),
    ("Si28", "n-014_Si_028-ENDF8.0.endf"),
    ("Si29", "n-014_Si_029-ENDF8.0.endf"),
    ("Si30", "n-014_Si_030-ENDF8.0.endf"),
    ("Mn55", "n-025_Mn_055-ENDF8.0.endf"),
    ("B10", "n-005_B_010-ENDF8.0.endf"),
];

/// (points compared, points differing, max |ΔE|/E, max |Δσ|/|σ|) over the
/// two tables' common length; the caller reports any length difference.
fn diff(a: &[(f64, f64)], b: &[(f64, f64)]) -> (usize, usize, f64, f64) {
    let rel = |x: f64, y: f64| {
        if x == y {
            0.0
        } else {
            ((x - y) / x.abs().max(y.abs())).abs()
        }
    };
    let (mut n, mut de, mut ds) = (0, 0.0_f64, 0.0_f64);
    for (&(e1, s1), &(e2, s2)) in a.iter().zip(b) {
        if e1.to_bits() != e2.to_bits() || s1.to_bits() != s2.to_bits() {
            n += 1;
            de = de.max(rel(e1, e2));
            ds = ds.max(rel(s1, s2));
        }
    }
    (a.len().min(b.len()), n, de, ds)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let list: Vec<(String, String)> = if args.is_empty() {
        DEFAULT
            .iter()
            .map(|(n, f)| (n.to_string(), f.to_string()))
            .collect()
    } else {
        args.iter()
            .map(|a| {
                let (n, f) = a.split_once(':').expect("NAME:TAPE");
                (n.to_string(), f.to_string())
            })
            .collect()
    };
    let t_k = 293.6;
    let settings = CrossSectionSettings::default()
        .without_purr()
        .without_heatr();
    println!("| nuclide | sections | points | differ | elastic_0k pts (<= 25 keV) | differ | max dE/E | max dsigma/sigma |");
    println!("|---|---|---|---|---|---|---|---|");
    let mut dbrc_rows = Vec::new();
    for (name, file) in list {
        let p = reference_endf(&file).unwrap_or_else(|| panic!("missing tape {file}"));
        let tape = Tape::read_file(&p).expect("tape");
        let mat = tape.materials()[0];
        let recon0 = Nuclide::reconstruct_0k(&tape, mat, 1.0e-3).expect("RECONR");
        let r4 = Nuclide::process_from_stages(&recon0, None, t_k, 1.0e-3);
        let recon = Reconstruction {
            tape: Arc::new(tape),
            mat,
            reconr: recon0,
            reconr_tolerance: 1.0e-3,
        };
        let pr = process::cross_sections_at(&recon, Temperature::new::<kelvin>(t_k), &settings)
            .expect("process");
        let (mut pts, mut n_diff) = (0, 0);
        assert_eq!(
            r4.recon.sections.len(),
            pr.pendf.sections.len(),
            "{name}: section count"
        );
        for (a, b) in r4.recon.sections.iter().zip(&pr.pendf.sections) {
            assert_eq!(a.mt, b.mt, "{name}: section order");
            assert_eq!(a.pairs.len(), b.pairs.len(), "{name}: MT {:?} length", a.mt);
            let (n, d, _, _) = diff(&a.pairs, &b.pairs);
            pts += n;
            n_diff += d;
        }
        let el_pr: Vec<(f64, f64)> = pr
            .elastic_0k
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .copied()
            .filter(|(e, _)| *e <= DBRC_GRID_MAX_EV)
            .collect();
        // DBRC on both tables, identical RNG streams: how many sampled
        // outcomes change? A flip needs a uniform within the tables'
        // relative difference of the acceptance threshold.
        let awr = pr.awr();
        let (ta, tb) = (
            DbrcTable::from_pairs(&r4.elastic_0k, 1.0e3),
            DbrcTable::from_pairs(&el_pr, 1.0e3),
        );
        let kt = 8.617_333_262e-5 * t_k;
        let u = Direction::new(0.0, 0.0, 1.0);
        let (mut flips, mut control) = (0usize, 0usize);
        for i in 0..DRAWS {
            // log-uniform over the DBRC window, 1e-2 .. 1e3 eV
            let e = 1.0e-2 * 1.0e5_f64.r_powf((i as f64 + 0.5) / DRAWS as f64);
            let s0 = 0xDB4C_0929u64.wrapping_add((i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let (mut sa, mut sb) = (s0, s0);
            let a = free_gas_elastic_scatter_dbrc(e, u, awr, kt, 0.3, &mut sa, ta.as_ref());
            let b = free_gas_elastic_scatter_dbrc(e, u, awr, kt, 0.3, &mut sb, tb.as_ref());
            if a.0.to_bits() != b.0.to_bits() || sa != sb {
                flips += 1;
            }
            // Control: the same draw with DBRC off must often differ, or the
            // flip count above could not have been anything but zero.
            let mut sc = s0;
            let c = free_gas_elastic_scatter_dbrc(e, u, awr, kt, 0.3, &mut sc, None);
            if c.0.to_bits() != a.0.to_bits() || sc != sa {
                control += 1;
            }
        }
        assert!(
            ta.is_some() && tb.is_some(),
            "{name}: both DBRC tables built"
        );
        dbrc_rows.push(format!("| {name} | {DRAWS} | {flips} | {control} |"));
        let (n0, d0, de, ds) = diff(&r4.elastic_0k, &el_pr);
        let (l4, lp) = (r4.elastic_0k.len(), el_pr.len());
        let last = |v: &[(f64, f64)]| v.last().map_or(0.0, |p| p.0);
        println!(
            "| {name} | {} | {pts} | {n_diff} | {n0} (route 4 {l4}, process {lp}; last E {:.12e} / {:.12e}) | {d0} | {de:.2e} | {ds:.2e} |",
            pr.pendf.sections.len(),
            last(&r4.elastic_0k),
            last(&el_pr)
        );
    }
    println!("\nDBRC free-gas draws at 293.6 K, 1e-2..1e3 eV, same seeds, raw vs PENDF-rounded 0 K elastic:\n");
    println!("| nuclide | draws | outcomes that differ | control: differ from DBRC off |\n|---|---|---|---|");
    for r in dbrc_rows {
        println!("{r}");
    }
}
