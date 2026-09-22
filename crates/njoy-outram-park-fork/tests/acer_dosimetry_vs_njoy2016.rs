//! **V&V gate** — the dosimetry ACE writer (`acer iopt = 3`) against NJOY2016.
//!
//! ## Methodology
//!
//! Both codes read the **same PENDF**, so the comparison isolates `acedos`
//! and folds in neither RECONR nor BROADR. The pass criterion is **byte
//! equality of the written Type-1 file**: every XSS word agrees to 12
//! significant figures, and the header formatting, NXS/JXS layout, the
//! integer/real split that `dosout` applies word by word, and the line
//! breaking are all pinned at the same time.
//!
//! References were produced with NJOY2016 2016.79:
//!
//! ```text
//! reconr / 20 21 / '<nuclide> pendf' / <mat> 0 / .001 / 0
//! broadr / 20 21 22 / <mat> 1 / .001 1.e6 / 293.6 / 0
//! acer   / 20 22 0 24 25 / 3 1 1 .00 / '<comment>' / <mat> 293.6
//! ```
//!
//! ## Results (2026-09-21)
//!
//! | case | reactions | XSS words | bytes | result |
//! |---|---|---|---|---|
//! | H-1 ENDF/B-VIII.0-β6, MAT 125, 293.6 K | 2 | 2 532 | 52 130 | **byte-identical** |
//! | Mn-55 ENDF/B-VIII.0, MAT 2525, 293.6 K | 119 | 70 440 | 1 427 267 | **byte-identical** |
//!
//! Mn-55 is the load-bearing one: its PENDF carries **MF=10**, so the run
//! exercises the isomeric-production branch — the synthetic MT
//! `MT + 1000 (10 + LFS)` (reactions 10030, 12030, 10037, 11037 in the
//! table) and the interleaved region convention that differs from MF=3's.
//!
//! **NJOY's own test suite never runs `iopt = 3`** (checked over all 40
//! cases in `upstream_source/NJOY2016/tests`), so this gate covers a path
//! upstream does not cover itself.

use njoy_outram_park_fork::acer::dosimetry::{dosimetry_ace, DosimetryOptions};
use njoy_outram_park_fork::acer::read::AceFileType;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_file_or_skip;

fn check(pendf: &str, ace: &str, mat: i32, comment: &str, ntr: usize, len2: i32) {
    check_with(pendf, ace, mat, comment, ntr, len2, false, "09/21/26")
}

#[allow(clippy::too_many_arguments)]
fn check_with(
    pendf: &str,
    ace: &str,
    mat: i32,
    comment: &str,
    ntr: usize,
    len2: i32,
    mcnpx: bool,
    date: &str,
) {
    let label = format!("acer dosimetry {mat}");
    let Some(pendf) = reference_file_or_skip("acer", pendf, &label) else {
        return;
    };
    let Some(ace) = reference_file_or_skip("acer", ace, &label) else {
        return;
    };
    let tape = Tape::read_file(&pendf).expect("read the PENDF");
    let opts = DosimetryOptions {
        suffix: 0.0,
        comment: comment.to_string(),
        // `hd` is whatever `dater()` returned when the reference was made.
        date: date.to_string(),
        mcnpx,
        ..Default::default()
    };
    let built = dosimetry_ace(&tape, mat, 293.6, &opts).expect("build the dosimetry table");
    assert_eq!(built.ntr, ntr, "reaction count");
    assert_eq!(built.nxs[0], len2, "NXS(1) = len2");
    assert_eq!(built.jxs[0], 1, "JXS(1) = lone");
    assert_eq!(built.jxs[2], 1, "JXS(3) = mtr");
    assert_eq!(built.jxs[5], 1 + ntr as i32, "JXS(6) = lsig");
    assert_eq!(built.jxs[6], 1 + 2 * ntr as i32, "JXS(7) = sigd");
    assert_eq!(built.jxs[21], len2, "JXS(22) = end");

    if mcnpx {
        assert_eq!(
            built.za as f64,
            1001.0,
            "the mcnpx variant must not change the table, only its header"
        );
    }
    let table = built.into_raw(&opts, AceFileType::Type1Ascii);
    if mcnpx {
        assert_eq!(
            table.header.raw_text[0].len(),
            13,
            "the mcnpx ZAID field is 13 characters wide (acedo.f90:270-272)"
        );
    }
    let want = std::fs::read_to_string(&ace).expect("read NJOY's dosimetry ACE");
    let got = table.to_type1_string();
    if got != want {
        let n = got
            .bytes()
            .zip(want.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        panic!(
            "dosimetry Type-1 output differs from NJOY at byte {n} of {} (NJOY {} \
             bytes)\n  ours: {:?}\n  njoy: {:?}",
            got.len(),
            want.len(),
            &got[n.saturating_sub(40)..(n + 40).min(got.len())],
            &want[n.saturating_sub(40)..(n + 40).min(want.len())],
        );
    }
    eprintln!("[dosimetry {mat}] reproduced NJOY's {} bytes exactly", want.len());
}

#[test]
fn h1_dosimetry_is_byte_identical_to_njoy2016() {
    check(
        "h1_293k_pendf_njoy2016.pendf",
        "h1_293k_dosimetry_njoy2016.ace",
        125,
        "h1 dosimetry",
        2,
        2532,
    );
}

/// The MF=10 case. Its four synthetic MTs are the only exercise the
/// isomeric-production branch gets anywhere in this workspace.
#[test]
fn mn55_dosimetry_is_byte_identical_to_njoy2016() {
    check(
        "mn55_293k_pendf_njoy2016.pendf",
        "mn55_293k_dosimetry_njoy2016.ace",
        2525,
        "mn55 dosimetry",
        119,
        70440,
    );
    let Some(pendf) = reference_file_or_skip(
        "acer",
        "mn55_293k_pendf_njoy2016.pendf",
        "acer dosimetry MF=10",
    ) else {
        return;
    };
    let tape = Tape::read_file(&pendf).expect("read the PENDF");
    let opts = DosimetryOptions::default();
    let built = dosimetry_ace(&tape, 2525, 293.6, &opts).expect("build");
    let mts: Vec<i32> = (0..built.ntr).map(|i| built.mt(i)).collect();
    for want in [10030, 12030, 10037, 11037] {
        assert!(
            mts.contains(&want),
            "MF=10 must contribute the synthetic MT {want} (MT + 1000*(10 + LFS)); \
             got {mts:?}"
        );
    }
    // Every reaction must read back through its own LSIG offset with matching
    // grid and cross-section lengths -- the locator squeeze is the one place
    // an off-by-one would hide.
    for i in 0..built.ntr {
        let (e, s) = built.reaction(i);
        assert_eq!(e.len(), s.len(), "reaction {i} grid/sigma length");
        assert!(!e.is_empty(), "reaction {i} is empty");
        assert!(
            e.windows(2).all(|w| w[1] >= w[0]),
            "reaction {i} energies must ascend"
        );
    }
}

/// Upstream cannot produce a 0 K dosimetry table — its temperature search
/// never runs, so ZA and AWR are never read and the file comes out with ZAID
/// `0.00y`. Verified against NJOY2016 on 2026-09-21. This port refuses
/// instead, and that refusal is pinned so nobody "fixes" it into silence.
#[test]
fn zero_kelvin_is_refused_rather_than_written_with_no_zaid() {
    let Some(pendf) = reference_file_or_skip(
        "acer",
        "h1_293k_pendf_njoy2016.pendf",
        "acer dosimetry 0 K",
    ) else {
        return;
    };
    let tape = Tape::read_file(&pendf).expect("read the PENDF");
    let err = dosimetry_ace(&tape, 125, 0.0, &DosimetryOptions::default())
        .expect_err("a 0 K dosimetry table must be refused");
    let msg = format!("{err}");
    assert!(
        msg.contains("0 K dosimetry"),
        "the refusal must say why: {msg}"
    );
}


/// The **mcnpx** variant: `f10.3` then `"ny "` in a 13-character field
/// (`acedo.f90:270-272`), selected by a negative `iopt`. Implemented and
/// unexercised until a reference was made for it on 2026-09-22.
#[test]
fn h1_dosimetry_mcnpx_is_byte_identical_to_njoy2016() {
    check_with(
        "h1_293k_pendf_njoy2016.pendf",
        "h1_293k_dosimetry_mcnpx_njoy2016.ace",
        125,
        "h1 dosimetry mcnpx",
        2,
        2532,
        true,
        "09/22/26",
    );
}
