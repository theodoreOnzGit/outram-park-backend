//! RECONR resonance-grid convergence against NJOY2016's own 0 K PENDF
//! (`resxs`, `reconr.f90:2240-2570`) — the Si-30 inter-resonance capture
//! case of bead `op-yr43`.
//!
//! Oracle: upstream NJOY2016 (`ac5adf5`, gfortran 13.3.0) `reconr` on
//! ENDF/B-VIII.0 Si-30 (MAT 1431, `LRP=1`, RRR 1e-5 eV – 1.5 MeV, `LRF=3`),
//! `err = 0.001`, 0 K — `tape21` of the deck in
//! `reference-data/reconr/si30-ENDF8.0-0K.njoy-input`.
//!
//! **What was wrong** (measured 2026-09-10 by
//! `broadr_light_nuclide_pendf_golden.rs`): between the 2.2 and 4.9 keV
//! resonances the crate kept 8 grid points where NJOY has 47, and at
//! 3324.8 eV MT=102 was 6.276e-4 b against NJOY's 5.453e-4 b (+13 %). The
//! resonance kernel agreed at the crate's own points to <1e-3, so it was the
//! grid: the crate measured a panel's midpoint error against
//! `max(sigma, 0.1 b)`, a floor upstream does not have — a 13 % sag of a
//! 5e-4 b cross section is 6.5e-5 b, under `err` of the floor. Upstream's
//! only relaxation for small cross sections is the resonance-integral check
//! (`errmax = 10 err`, `errint = err/20000 = 5e-8 b` per point), and
//! `tsti = 2 errint xm / dx ~ 5e-7 b` is far below 6.5e-5 b, so NJOY splits.
//!
//! **Prediction** (stated before measuring): with upstream's test ported —
//! per-reaction relative error on elastic/fission/capture, the
//! `errmax`/`errint` integral check, the `err/5` rule below 0.5 eV, the
//! 7-figure midpoint rounding and significant-figure termination, and the
//! `estp = 4.1` step-increase rule — MT=102 at 3324.8 eV moves to within
//! 2e-3 of 5.453e-4 b, the 2.5–4.5 keV grid holds ~47 points, and every
//! reaction agrees with NJOY's tape to within a few times `err` over the
//! whole resolved range (two independent to-tolerance grids differ by up to
//! ~2 err where they are relaxed to `errmax` only on sub-`errint` panels).

use njoy_outram_park_fork::endf::interp::eval_tab1;
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::{reference_endf_or_skip, reference_file_or_skip};

const MAT: i32 = 1431;
const ERR: f64 = 0.001;
/// Bead `op-yr43`'s measured point: NJOY MT=102 at 3324.8 eV.
const PROBE_EV: f64 = 3324.8;
const PROBE_NJOY_B: f64 = 5.453e-4;
const PROBE_TOL: f64 = 2e-3;
/// Whole-range tolerance: `2 x errmax`. Both grids are to-tolerance, but on
/// panels whose contribution to the resonance integral is below `errint`
/// upstream (and now the port) only guarantees `errmax = 10 err`, so two
/// independently chosen grids can differ by up to about twice that on
/// 1e-5..1e-2 b cross sections (measured: 5-9e-3 on Si-30 MT=1/2 at 172 keV
/// and MT=102 at 1.3 MeV). Everywhere else the agreement is ~err.
const RANGE_TOL: f64 = 2.0 * 10.0 * ERR;
const N_SAMPLES: usize = 20_000;
const ABS_FLOOR_B: f64 = 1e-6;

#[test]
fn si30_reconr_grid_matches_njoy_between_resonances() {
    let tag = "reconr-si30-0K";
    let Some(endf_path) = reference_endf_or_skip("n-014_Si_030-ENDF8.0.endf", tag) else {
        return;
    };
    let Some(pendf_path) = reference_file_or_skip("reconr", "si30-ENDF8.0-0K.pendf", tag) else {
        return;
    };
    let endf = Tape::read_file(&endf_path).expect("ENDF parses");
    let njoy = Tape::read_file(&pendf_path).expect("NJOY 0 K PENDF parses");
    let t0 = std::time::Instant::now();
    let ours = reconr(
        &endf,
        &ReconrConfig {
            mat: MAT,
            tolerance: ERR,
            temperature: 0.0,
        },
    )
    .expect("RECONR");
    println!("[{tag}] RECONR in {:.2?}", t0.elapsed());

    let mut worst_all = (0.0f64, 0, 0.0, 0.0, 0.0);
    for sec in &ours.sections {
        let mt = i32::from(sec.mt);
        if ![1, 2, 18, 102].contains(&mt) {
            continue;
        }
        let nj = njoy
            .section(MAT, 3, mt)
            .unwrap_or_else(|| panic!("{tag}: NJOY PENDF has MF=3/MT={mt}"));
        let mut cur = SectionCursor::new(&nj.rows);
        let _ = cur.read_cont().unwrap();
        let tab = cur.read_tab1().unwrap();
        let our_interp = [(sec.pairs.len() as u32, 2u32)];

        // Grid density in the op-yr43 window (2.5-4.5 keV).
        let window = |pairs: &[(f64, f64)]| {
            pairs
                .iter()
                .filter(|(e, _)| (2500.0..=4500.0).contains(e))
                .count()
        };
        let (n_ours, n_njoy) = (window(&sec.pairs), window(&tab.pairs));

        // Sample the whole resolved range on NJOY's grid energies (the
        // strictest comparison: NJOY's own points are exact there) and on a
        // log-uniform sample.
        let e_lo = sec.pairs[0].0.max(tab.pairs[0].0).max(1e-5);
        let e_hi = 1.5e6_f64
            .min(sec.pairs.last().unwrap().0)
            .min(tab.pairs.last().unwrap().0);
        let mut worst = (0.0f64, 0.0, 0.0, 0.0);
        let mut probe = |e: f64| {
            if e < e_lo || e > e_hi {
                return;
            }
            let want = eval_tab1(e, &tab.interp, &tab.pairs).unwrap();
            let got = eval_tab1(e, &our_interp, &sec.pairs).unwrap();
            let scale = got.abs().max(want.abs());
            if scale <= ABS_FLOOR_B {
                return;
            }
            let rel = (got - want).abs() / scale;
            if rel > worst.0 {
                worst = (rel, e, got, want);
            }
        };
        for &(e, _) in &tab.pairs {
            probe(e);
        }
        for i in 0..=N_SAMPLES {
            probe(e_lo * (e_hi / e_lo).powf(i as f64 / N_SAMPLES as f64));
        }
        println!(
            "[{tag}] MT={mt:3} {:6} pts (njoy {:6}); 2.5-4.5 keV: {n_ours} pts (njoy {n_njoy}); worst {:.3e} at {:.4e} eV (crate {:.6e}, njoy {:.6e})",
            sec.pairs.len(),
            tab.pairs.len(),
            worst.0,
            worst.1,
            worst.2,
            worst.3
        );
        if worst.0 > worst_all.0 {
            worst_all = (worst.0, mt, worst.1, worst.2, worst.3);
        }

        if mt == 102 {
            let got = eval_tab1(PROBE_EV, &our_interp, &sec.pairs).unwrap();
            let want = eval_tab1(PROBE_EV, &tab.interp, &tab.pairs).unwrap();
            println!(
                "[{tag}] MT=102 at {PROBE_EV} eV: crate {got:.4e} njoy {want:.4e} (bead: {PROBE_NJOY_B:.3e}; crate was 6.276e-4)"
            );
            assert!(
                (want - PROBE_NJOY_B).abs() / PROBE_NJOY_B < 1e-3,
                "oracle tape at the probe: {want:.4e} vs the bead's {PROBE_NJOY_B:.3e}"
            );
            assert!(
                (got - want).abs() / want < PROBE_TOL,
                "MT=102 at {PROBE_EV} eV: crate {got:.4e} vs njoy {want:.4e}"
            );
            assert!(
                n_ours >= n_njoy / 2,
                "2.5-4.5 keV grid: crate {n_ours} points vs njoy {n_njoy}"
            );
        }
    }
    assert!(
        worst_all.0 < RANGE_TOL,
        "{tag}: worst {:.3e} for MT={} at {:.4e} eV (crate {:.6e}, njoy {:.6e})",
        worst_all.0,
        worst_all.1,
        worst_all.2,
        worst_all.3,
        worst_all.4
    );
}
