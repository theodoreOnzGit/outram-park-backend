#![cfg(feature = "std")]
//! **`collapse_rate` against libopenmc's own** (gh:#1625).
//!
//! Reference: OpenMC `e4febe9`'s `Nuclide::collapse_rate` (`nuclide.cpp:1045`,
//! through `openmc.lib`), run 2026-11-17 by `verification_and_validation/
//! spectrum_depletion/openmc_inputs/ref_collapse_rate.py` on the gh:#1626
//! pincell library (HDF5 converted from the port-made ACE tables by
//! `IncidentNeutron.from_ace`), CCFE-709 bounds and a fixed made-up flux
//! written to `openmc_outputs/collapse_rate.tsv`. Nine chain_simple nuclides
//! x MT 102, 18, 16.
//!
//! The port: `depletion::microxs::nuclide_collapse_rate` on
//! `depletion::coupled_operator::reaction_from_ace` of the same ACE tables
//! (the library is in a scratch directory, not the repository; set
//! `OUTRAM_PINCELL_ACE_LIB` to its root, else the test says so and passes
//! without checking).
//!
//! **Prediction, written before the port side was run:** bit-identical, or
//! a few ulp where the summation order differs; the HDF5 holds the ACE
//! grid and SIG values unchanged, so the inputs are the same doubles.
//!
//! **Results, measured 2026-11-17:** **27 of 27 bit-identical** (prediction
//! held), after one defect in this crate was fixed on the way. On the
//! first run U-234's MT=18 was 0 here and 0.934 in OpenMC: the table carries
//! only MT 19/20/21/38, and OpenMC's `from_ace` builds a redundant MT=18
//! from them because the photon block names it. `reaction_from_ace` now
//! ports that rule (and the MT 16, 103..107 one). The pincell port runs of
//! gh:#1626 made before the fix had U-234 fission at zero and were re-run.
//!
//! **An upstream defect, reproduced on purpose:** U-238's fission rate comes
//! out **negative** (-19.955) in both codes. `Reaction::collapse_rate`
//! (`reaction.cpp:146-187`) never stops at the top of the nuclide's grid
//! (30 MeV): `i_high` is capped at `size - 2`, so the end-of-grid `break`
//! cannot fire, and for every group above the grid `E_low = E_group_low >
//! E_high = E_r` makes `dE < 0`, adding a negative, linearly extrapolated
//! term. Here the made-up flux's 1/E tail runs to CCFE-709's 1 GeV top: 48
//! groups contribute -20.1. A reactor transport flux is zero above ~20 MeV,
//! so the pincell is unaffected, but any group structure above the data's
//! grid with flux in it is. The port keeps upstream's arithmetic (the port
//! is the oracle); a guarded route belongs beside it.

use njoy_outram_park_fork::acer::ce_decode::decode_ce;
use outram_mc_libs::depletion::coupled_operator::reaction_from_ace;
use outram_mc_libs::depletion::microxs::nuclide_collapse_rate;
use std::path::PathBuf;

#[test]
fn collapse_rate_matches_libopenmc() {
    let Some(lib) = std::env::var_os("OUTRAM_PINCELL_ACE_LIB").map(PathBuf::from) else {
        println!("SKIP: OUTRAM_PINCELL_ACE_LIB not set (the gh:#1626 pincell ACE library)");
        return;
    };
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/spectrum_depletion/openmc_outputs/collapse_rate.tsv");
    let text = std::fs::read_to_string(p).unwrap();
    let (mut edges, mut flux, mut refs) = (Vec::new(), Vec::new(), Vec::new());
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        match f.as_slice() {
            ["edge", _, e] => edges.push(e.parse::<f64>().unwrap()),
            ["flux", _, x] => flux.push(x.parse::<f64>().unwrap()),
            ["rate", n, mt, x] => refs.push(((*n).to_string(), mt.parse::<i32>().unwrap(), x.parse::<f64>().unwrap())),
            _ => panic!("bad line {line}"),
        }
    }
    let (mut identical, mut worst) = (0, 0.0f64);
    for (n, mt, r) in &refs {
        let raw = njoy_outram_park_fork::acer::read::read(lib.join("293.6K").join(format!("{n}.ace.gz"))).unwrap();
        let ace = decode_ce(&raw).unwrap();
        let ours = reaction_from_ace(&raw, &ace, *mt).map_or(0.0, |rx| nuclide_collapse_rate(&rx, &edges, &flux));
        let rel = if *r == 0.0 { ours.abs() } else { ((ours - r) / r).abs() };
        identical += usize::from(ours.to_bits() == r.to_bits());
        worst = worst.max(rel);
        println!("{n:6} MT{mt:<4} ours {ours:+.17e}  OpenMC {r:+.17e}  rel {rel:.2e}");
    }
    println!("{identical} of {} bit-identical, worst relative {worst:.2e}", refs.len());
    assert!(worst <= 1e-12, "{worst}");
}
