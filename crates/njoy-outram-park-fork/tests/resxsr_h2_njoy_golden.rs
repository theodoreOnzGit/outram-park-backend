//! RESXSR verification against an NJOY2016 `resxsr` run (`resxsr.f90`).
//!
//! Oracle: upstream NJOY2016 (`ac5adf5`, gfortran 13.3.0) on the committed
//! 293.6 K H-2 PENDF (`reference-data/errorr/h2-ENDF8.0-293.6K.pendf`, MAT
//! 128): one material, one temperature, `efirst = 1e-5`, `elast = 1e3`,
//! `eps = 0.001` — `reference-data/resxsr/h2-293.6K-eps0.001.njoy-input`,
//! output `h2-293.6K-eps0.001.resxs` (2,124 bytes). NJOY's listing: MT=2 and
//! MT=102 on 314 points each, 165 after thinning. Note: NJOY's *binary* unit
//! path (`nout < 0`) aborts under this gfortran ("unit number is negative
//! and unit was not already opened with NEWUNIT", `resxsr.f90:445`), so the
//! oracle is the positive-unit stream, which resxsr still writes with
//! unformatted `write(nout)` statements.
//!
//! **Prediction** (stated before measuring): `run_resxs` with the same cards
//! reproduces the union grid (165 points) and the two reaction columns to
//! the PENDF's 7 figures; whether the byte stream is identical depends on
//! the record framing, which is reported rather than assumed.
//!
//! **Measured (2026-09-10).** First run: 2,100 bytes against NJOY's 2,124 and
//! NJOY's stream unreadable — the crate framed records with one leading
//! *word* count where gfortran writes a leading and trailing *byte* count;
//! after that fix the streams differed only at byte 96: `locm` (the crate
//! counted the four file-header records, upstream `resxsr.f90:234,253`
//! starts `irec` at 0). With both fixed the stream is byte-identical, which
//! is now asserted.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use njoy_outram_park_fork::resxsr::driver::run_resxs;
use njoy_outram_park_fork::resxsr::input::{MaterialSpec, ResxsrInput};
use njoy_outram_park_fork::resxsr::resxs::ResxsFile;

#[test]
fn resxsr_h2_matches_njoy() {
    let tag = "resxsr-h2";
    let Some(pendf) = reference_file_or_skip("errorr", "h2-ENDF8.0-293.6K.pendf", tag) else {
        return;
    };
    let Some(golden) = reference_file_or_skip("resxsr", "h2-293.6K-eps0.001.resxs", tag) else {
        return;
    };
    let njoy_bytes = std::fs::read(&golden).unwrap();
    let tape = Tape::read_file(&pendf).expect("PENDF parses");
    let input = ResxsrInput {
        nout: 25,
        maxt: 1,
        efirst: 1e-5,
        elast: 1e3,
        eps: 0.001,
        user_id: "w4 test".into(),
        ivers: 1,
        comments: vec!["resxsr oracle: h2 293.6 K pendf, eps 0.001".into()],
        materials: vec![MaterialSpec {
            hmat: "h2".into(),
            mat: 128,
            nin: 20,
        }],
    };
    let mut ours = Vec::new();
    run_resxs(&input, std::slice::from_ref(&tape), &mut ours).expect("run_resxs");
    let first_diff = ours
        .iter()
        .zip(&njoy_bytes)
        .position(|(a, b)| a != b)
        .or_else(|| (ours.len() != njoy_bytes.len()).then_some(ours.len().min(njoy_bytes.len())));
    println!(
        "[{tag}] bytes: ours {} njoy {}; first differing byte {:?}",
        ours.len(),
        njoy_bytes.len(),
        first_diff
    );
    assert_eq!(
        first_diff, None,
        "{tag}: RESXS stream byte-identical to NJOY"
    );

    let njoy = ResxsFile::read(njoy_bytes.as_slice());
    let ours_f = ResxsFile::read(ours.as_slice()).expect("our RESXS re-reads");
    match njoy {
        Err(e) => {
            println!("[{tag}] NJOY stream does not parse with ResxsFile::read: {e:?}");
            panic!("{tag}: NJOY RESXS stream not readable by the crate's reader");
        }
        Ok(njoy) => {
            let (m, o) = (&njoy.materials[0], &ours_f.materials[0]);
            println!(
                "[{tag}] njoy: nmat {} nener {} nreac {} temps {:?}; ours: nener {} nreac {} temps {:?}",
                njoy.control.nmat,
                m.control.nener,
                m.control.nreac,
                m.control.temps,
                o.control.nener,
                o.control.nreac,
                o.control.temps
            );
            assert_eq!(m.control.nener, o.control.nener, "{tag}: union grid size");
            assert_eq!(m.control.nreac, o.control.nreac, "{tag}: reaction count");
            let mut worst = (0.0f64, 0usize, 0usize, 0.0, 0.0);
            for (i, (pn, po)) in m.points.iter().zip(&o.points).enumerate() {
                let de = (pn.energy - po.energy).abs() / pn.energy.abs().max(1e-30);
                assert!(
                    de < 1e-6,
                    "{tag}: energy {i}: njoy {} ours {}",
                    pn.energy,
                    po.energy
                );
                for (k, (&a, &b)) in pn.values.iter().zip(&po.values).enumerate() {
                    let scale = a.abs().max(b.abs());
                    if scale <= 1e-30 {
                        continue;
                    }
                    let rel = (a - b).abs() / scale;
                    if rel > worst.0 {
                        worst = (rel, i, k, b, a);
                    }
                }
            }
            println!(
                "[{tag}] values: worst {:.3e} at point {} column {} (ours {:.6e}, njoy {:.6e})",
                worst.0, worst.1, worst.2, worst.3, worst.4
            );
            assert!(worst.0 < 2e-6, "{tag}: worst {:.3e}", worst.0);
        }
    }
}

/// The multi-temperature loop (`resxsr.f90:267-352`): the same H-2 tape
/// broadened to 293.6 K **and** 600 K (`reference-data/resxsr/
/// h2-293.6K-600K.pendf`, the deck alongside), `maxt = 2`. NJOY's listing:
/// 314 + 314 points at 293.6 K, 329 + 329 at 600 K, 168 after thinning;
/// `tape25` is 3,508 bytes. Upstream appends each temperature's reactions as
/// further columns (`jx` temperature-major) and thins over all of them.
///
/// **Prediction** (stated before measuring): byte-identical, with the
/// material control record carrying two temperatures, `nreac = 2`, 168
/// points of `1 + 2*2` words.
#[test]
fn resxsr_h2_two_temperatures_matches_njoy() {
    let tag = "resxsr-h2-2T";
    let Some(pendf) = reference_file_or_skip("resxsr", "h2-293.6K-600K.pendf", tag) else {
        return;
    };
    let Some(golden) = reference_file_or_skip("resxsr", "h2-293.6K-600K-eps0.001.resxs", tag)
    else {
        return;
    };
    let njoy_bytes = std::fs::read(&golden).unwrap();
    let tape = Tape::read_file(&pendf).expect("PENDF parses");
    let input = ResxsrInput {
        nout: 25,
        maxt: 2,
        efirst: 1e-5,
        elast: 1e3,
        eps: 0.001,
        user_id: "w4 test".into(),
        ivers: 1,
        comments: vec!["resxsr oracle: h2 293.6 + 600 K pendf, eps 0.001".into()],
        materials: vec![MaterialSpec {
            hmat: "h2".into(),
            mat: 128,
            nin: 22,
        }],
    };
    let mut ours = Vec::new();
    run_resxs(&input, std::slice::from_ref(&tape), &mut ours).expect("run_resxs");
    let first_diff = ours
        .iter()
        .zip(&njoy_bytes)
        .position(|(a, b)| a != b)
        .or_else(|| (ours.len() != njoy_bytes.len()).then_some(ours.len().min(njoy_bytes.len())));
    let njoy = ResxsFile::read(njoy_bytes.as_slice()).expect("NJOY RESXS parses");
    let ours_f = ResxsFile::read(ours.as_slice()).expect("our RESXS re-reads");
    let (m, o) = (&njoy.materials[0], &ours_f.materials[0]);
    println!(
        "[{tag}] bytes: ours {} njoy {}; first differing byte {:?}; njoy nener {} nreac {} temps {:?}; ours nener {} nreac {} temps {:?}",
        ours.len(),
        njoy_bytes.len(),
        first_diff,
        m.control.nener,
        m.control.nreac,
        m.control.temps,
        o.control.nener,
        o.control.nreac,
        o.control.temps
    );
    assert_eq!(
        m.control.temps.len(),
        2,
        "{tag}: oracle has two temperatures"
    );
    assert_eq!(
        first_diff, None,
        "{tag}: RESXS stream byte-identical to NJOY"
    );
}
