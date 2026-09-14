//! RECONR MLBW (`LRF=2`) oracle: the crate's 0 K reconstruction of
//! TENDL-2023 Ar-37 against NJOY2016's own `tape21`
//! (`reference-data/reconr/ar37-tendl2023-0K.pendf`, deck
//! `reconr 20 21 / 1828 0 / 0.001 / 0 /`).
//!
//! # Why this test exists
//!
//! Until 2026-09-10 the crate evaluated `LRF=2` ranges with the SLBW
//! elastic formula (`csslbw`), on the assumption that multilevel
//! interference is negligible for widely-spaced resonances. The ERRORR
//! MF=32 tier-2 oracle measured the cost on Ar-37 (three bound levels, a
//! 52 eV neutron width at −3236 eV): elastic +6.33 % from 1e-5 eV to 1 eV,
//! −8.23 % at 1 keV, −5.53 % at 4 keV, while capture matched to 1e-7
//! (bead `op-cral`). `reconr::slbw::eval_mlbw_lstate` now ports
//! `csmlbw`'s per-`J` assembly; this test pins the result.
//!
//! # Predictions (measured 2026-09-10 after the fix)
//!
//! Elastic within **1e-5** of NJOY below 100 eV (measured ≤ 4.1e-7);
//! capture within **1e-4** there (measured 5.6e-5 at 1e-2 eV — lin-lin
//! interpolation of a 1/v curve between two independently thinned grids,
//! identical before and after the fix); both within **1e-3** on the
//! resonance wings (measured worst 4.5e-4 elastic and 3.0e-4 capture at
//! 1400 eV, again grid interpolation). The total is reported, not asserted: it carries the
//! MF=3 background, which differs by 1.45 % at 4 keV and 0.34 % at
//! 4268 eV with elastic and capture agreeing there.

use njoy_outram_park_fork::endf::interp::eval_tab1;
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::{reference_endf_or_skip, reference_file_or_skip};

const MAT: i32 = 1828;
const LABEL: &str = "reconr-ar37-mlbw";
const LOW_TOL_ELASTIC: f64 = 1e-5;
const LOW_TOL_CAPTURE: f64 = 1e-4;
const WING_TOL: f64 = 1e-3;

fn njoy_xs(njoy: &Tape, mt: i32, e: f64) -> f64 {
    let sec = njoy.section(MAT, 3, mt).expect("NJOY PENDF section");
    let mut cur = SectionCursor::new(&sec.rows);
    let _ = cur.read_cont().unwrap();
    let tab = cur.read_tab1().unwrap();
    eval_tab1(e, &tab.interp, &tab.pairs).unwrap()
}

#[test]
fn ar37_mlbw_elastic_and_capture_match_njoy_tape21() {
    let Some(endf_path) = reference_endf_or_skip("n-018_Ar_37-tendl2023.endf", LABEL) else {
        return;
    };
    let Some(golden) = reference_file_or_skip("reconr", "ar37-tendl2023-0K.pendf", LABEL) else {
        return;
    };
    let endf = Tape::read_file(&endf_path).expect("ENDF parses");
    let njoy = Tape::read_file(&golden).expect("NJOY tape21 parses");
    let r = reconr(
        &endf,
        &ReconrConfig {
            mat: MAT,
            tolerance: 0.001,
            temperature: 0.0,
        },
    )
    .expect("RECONR");
    let low = [1e-5, 1e-4, 1e-3, 1e-2, 0.0253, 0.1, 1.0, 10.0, 100.0];
    let wings = [
        500.0, 1000.0, 1400.0, 1540.0, 1700.0, 2100.0, 2630.0, 3000.0, 4000.0, 4268.0,
    ];
    for mt in [1, 2, 102] {
        let ours = r
            .sections
            .iter()
            .find(|s| i32::from(s.mt) == mt)
            .expect("crate section");
        let interp = [(ours.pairs.len() as u32, 2)];
        let mut worst_low = (0.0f64, 0.0f64);
        let mut worst_wing = (0.0f64, 0.0f64);
        for (set, worst) in [(&low[..], &mut worst_low), (&wings[..], &mut worst_wing)] {
            for &e in set {
                let a = njoy_xs(&njoy, mt, e);
                let b = eval_tab1(e, &interp, &ours.pairs).unwrap();
                let rel = (b - a) / a;
                if rel.abs() > worst.0.abs() {
                    *worst = (rel, e);
                }
            }
        }
        println!(
            "[{LABEL}] MT={mt}: worst below 100 eV {:+.3e} at {:.3e} eV | worst on the wings {:+.3e} at {:.4e} eV",
            worst_low.0, worst_low.1, worst_wing.0, worst_wing.1
        );
        if mt == 1 {
            continue; // MF=3 background differences near the RRR/URR edge: reported only
        }
        let low_tol = if mt == 2 {
            LOW_TOL_ELASTIC
        } else {
            LOW_TOL_CAPTURE
        };
        assert!(
            worst_low.0.abs() < low_tol,
            "MT={mt} below 100 eV: {:+.3e} at {:.3e} eV (was +6.3e-2 with the SLBW formula)",
            worst_low.0,
            worst_low.1
        );
        assert!(
            worst_wing.0.abs() < WING_TOL,
            "MT={mt} on the wings: {:+.3e} at {:.4e} eV",
            worst_wing.0,
            worst_wing.1
        );
    }
}
