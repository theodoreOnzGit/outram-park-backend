//! MIXR verification against an NJOY2016 `mixr` run (`mixr.f90`).
//!
//! Oracle: upstream NJOY2016 (`ac5adf5`, gfortran 13.3.0) mixing the two
//! committed 293.6 K PENDFs `reference-data/errorr/h2-ENDF8.0-293.6K.pendf`
//! (MAT 128) and `be9-ENDF8.0-293.6K.pendf` (MAT 425) with weights 0.5/0.5
//! for MT 1, 2, 102 into MAT 9999 (`za = 9999`, `awr = 5`) — the deck in
//! `reference-data/mixr/h2-be9-0.5-0.5-293.6K.njoy-input`, output
//! `h2-be9-0.5-0.5-293.6K.pendf` (814 lines).
//!
//! **Prediction** (stated before measuring): `mixr` builds the union energy
//! grid of the inputs per reaction and sums `w_i * sigma_i(E)` by lin-lin
//! interpolation (`mixr.f90:196-378`); the crate's `mix` is a port of that,
//! so MAT 9999's three sections should have the same point counts and agree
//! to the tape's 7 printed figures. A wrong union (dropped or duplicated
//! energies) shows as a count mismatch; a wrong weighting as a uniform
//! factor.

use njoy_outram_park_fork::endf::interp::eval_tab1;
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::mixr::driver::run_mix;
use njoy_outram_park_fork::mixr::MixrInput;
use njoy_outram_park_fork::reference_data::reference_file_or_skip;

const OUT_MAT: i32 = 9999;
const TOL: f64 = 2e-6;

fn section_tab1(tape: &Tape, mat: i32, mt: i32) -> (Vec<(u32, u32)>, Vec<(f64, f64)>) {
    let sec = tape
        .section(mat, 3, mt)
        .unwrap_or_else(|| panic!("MF=3/MT={mt} of MAT {mat}"));
    let mut cur = SectionCursor::new(&sec.rows);
    let _ = cur.read_cont().unwrap();
    let t = cur.read_tab1().unwrap();
    (t.interp, t.pairs)
}

#[test]
fn mixr_h2_be9_matches_njoy() {
    let tag = "mixr-h2-be9";
    let Some(h2) = reference_file_or_skip("errorr", "h2-ENDF8.0-293.6K.pendf", tag) else {
        return;
    };
    let Some(be9) = reference_file_or_skip("errorr", "be9-ENDF8.0-293.6K.pendf", tag) else {
        return;
    };
    let Some(golden) = reference_file_or_skip("mixr", "h2-be9-0.5-0.5-293.6K.pendf", tag) else {
        return;
    };
    let input = MixrInput::from_cards(
        24,
        &[0, 1],
        vec![1, 2, 102],
        &[(128, 0.5), (425, 0.5)],
        293.6,
        OUT_MAT,
        9999.0,
        5.0,
        "w4 mixr oracle: 0.5 h2 + 0.5 be9 at 293.6 K",
    );
    let mut out = Vec::new();
    run_mix(
        &input,
        vec![
            std::fs::File::open(&h2).unwrap(),
            std::fs::File::open(&be9).unwrap(),
        ],
        &mut out,
    )
    .expect("run_mix");
    let ours = Tape::read(out.as_slice()).expect("our MIXR tape parses");
    let njoy = Tape::read_file(&golden).expect("NJOY MIXR tape parses");

    // MF=1/451 head: za, awr.
    let head = SectionCursor::new(&njoy.section(OUT_MAT, 1, 451).unwrap().rows)
        .read_cont()
        .unwrap();
    let ours_head = SectionCursor::new(&ours.section(OUT_MAT, 1, 451).unwrap().rows)
        .read_cont()
        .unwrap();
    assert_eq!(
        (head.c1, head.c2),
        (ours_head.c1, ours_head.c2),
        "MF=1/451 ZA, AWR"
    );

    let mut worst_all = (0.0f64, 0, 0.0, 0.0, 0.0);
    for mt in [1, 2, 102] {
        let (ni, np) = section_tab1(&njoy, OUT_MAT, mt);
        let (oi, op) = section_tab1(&ours, OUT_MAT, mt);
        let mut worst = (0.0f64, 0.0, 0.0, 0.0);
        for &(e, want) in &np {
            let got = eval_tab1(e, &oi, &op).unwrap();
            let scale = got.abs().max(want.abs());
            if scale == 0.0 {
                continue;
            }
            let rel = (got - want).abs() / scale;
            if rel > worst.0 {
                worst = (rel, e, got, want);
            }
        }
        let sum_njoy: f64 = np.iter().map(|p| p.1).sum();
        let sum_ours: f64 = op.iter().map(|p| p.1).sum();
        assert!(sum_njoy > 0.0, "MT={mt}: oracle section carries data");
        println!(
            "[{tag}] MT={mt:3}: {} pts (njoy {}), interp regions {} (njoy {}); sum sigma {:.6e} (njoy {:.6e}); worst {:.3e} at {:.4e} eV (crate {:.6e}, njoy {:.6e})",
            op.len(),
            np.len(),
            oi.len(),
            ni.len(),
            sum_ours,
            sum_njoy,
            worst.0,
            worst.1,
            worst.2,
            worst.3
        );
        assert_eq!(op.len(), np.len(), "MT={mt}: union grid point count");
        if worst.0 > worst_all.0 {
            worst_all = (worst.0, mt, worst.1, worst.2, worst.3);
        }
    }
    assert!(
        worst_all.0 < TOL,
        "{tag}: worst {:.3e} for MT={} at {:.4e} eV (crate {:.6e}, njoy {:.6e})",
        worst_all.0,
        worst_all.1,
        worst_all.2,
        worst_all.3,
        worst_all.4
    );
}
