//! **V&V gate** — the photo-atomic ACE writer against NJOY2016's own output.
//!
//! ## Methodology
//!
//! `reference-data/endf/photoat-synthetic-Z6.endf` is processed twice: once by
//! NJOY2016 (`acer`, `iopt = 4`, `itype = 1` and `itype = 2`, MAT 600, suffix
//! `.00`), whose output is committed as
//! `reference-data/acer/z6_photoatomic_njoy2016.ace` and
//! `…type2.ace`; and once by this crate, through
//! [`photoatomic_ace`] and the Type-1/Type-2 writers.
//!
//! The pass criterion is **byte equality of the written file**, not a
//! tolerance on the values. That is the strongest statement available and it
//! is the right one here: a photo-atomic table is written entirely as
//! `1pe20.11`, so byte equality means every one of the 449 words agrees to 12
//! significant figures, and it additionally pins the header formatting, the
//! NXS/JXS layout, the natural-log convention on ESZG and — for Type 2 — the
//! one-real-per-record blocking (`ner = 1`, `acepa.f90:297`), none of which a
//! value comparison would catch.
//!
//! ## Results (2026-09-21)
//!
//! | check | result |
//! |---|---|
//! | Type-1 ASCII, 9,950 bytes | **byte-identical** |
//! | Type-2 binary, 7,692 bytes | header byte-identical; 93 of 449 data words differ, worst **3 ulps / 6.7e-16** |
//! | Type-2 ESZG is linear, Type-1 is logs | reproduced (upstream asymmetry) |
//!
//! The Type-2 residual is the one place in this comparison where full doubles
//! are compared rather than 12 printed digits, and it is 6.7e-16 — below
//! anything Type 1 can express, which is why that container is byte-exact.
//! Four candidate causes were tried and none removes it: the three groupings
//! of `terp1`'s lin-lin expression, and FMA contraction in either `terp1` or
//! `sigfig`. See `verification_and_validation/acer_photoatomic_vs_njoy2016.md`.
//! | NXS, JXS | identical (len2 449, Z 6, nes 53, nflo 0) |
//!
//! On the real evaluation `photoat-092_U_000-ENDF8.0.endf` (MAT 9200, Z = 92,
//! 11,942 energies, 71,807 words) the same comparison leaves **26 words**
//! outside the file's print precision, all in the heating block and all above
//! 14 MeV, worst 8.7e-8 relative. That residual is upstream's own quadrature
//! truncation, not a translation defect; see
//! `verification_and_validation/acer_photoatomic_vs_njoy2016.md` for the
//! measurement and the three hypotheses it killed.

use njoy_outram_park_fork::acer::photoatomic::{photoatomic_ace, PhotoatomicOptions};
use njoy_outram_park_fork::acer::read::AceFileType;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::{reference_endf_or_skip, reference_file_or_skip};

/// The comment and date NJOY was given, so the header is reproducible.
const COMMENT: &str = "synthetic Z6 photoatomic";
const DATE: &str = "09/21/26";

fn build(
    file_type: AceFileType,
) -> Option<(njoy_outram_park_fork::acer::read::RawAceTable, usize, usize)> {
    let tape_path = reference_endf_or_skip("photoat-synthetic-Z6.endf", "acer photoatomic")?;
    let tape = Tape::read_file(&tape_path).expect("read the synthetic Z=6 photoatomic tape");
    let opts = PhotoatomicOptions {
        suffix: 0.0,
        comment: COMMENT.to_string(),
        date: DATE.to_string(),
        ..Default::default()
    };
    let built = photoatomic_ace(&tape, 600, &opts, None).expect("build the photoatomic table");
    assert!(
        built.fluorescence_missing,
        "no relaxation tape was supplied, so upstream's nlax = 0 path must be taken"
    );
    let (nes, nflo) = (built.nes, built.nflo);
    Some((built.into_raw(&opts, file_type), nes, nflo))
}

#[test]
fn type1_output_is_byte_identical_to_njoy2016() {
    let Some(reference) = reference_file_or_skip(
        "acer",
        "z6_photoatomic_njoy2016.ace",
        "acer photoatomic type 1",
    ) else {
        return;
    };
    let Some((table, nes, nflo)) = build(AceFileType::Type1Ascii) else {
        return;
    };
    assert_eq!((nes, nflo), (53, 0), "grid length / fluorescence row count");
    assert_eq!(table.nxs[0], 449, "NXS(1) = len2");
    assert_eq!(table.nxs[1], 6, "NXS(2) = Z");
    assert_eq!(
        &table.jxs[..5],
        &[1, 266, 287, 397, 397],
        "JXS(1..5) = eszg, jinc, jcoh, jflo, lhnm"
    );

    let want = std::fs::read_to_string(&reference).expect("read NJOY's photoatomic ACE");
    let got = table.to_type1_string();
    if got != want {
        let n = got
            .bytes()
            .zip(want.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        panic!(
            "photoatomic Type-1 output differs from NJOY at byte {n} of {} \
             (NJOY {} bytes)\n  ours: {:?}\n  njoy: {:?}",
            got.len(),
            want.len(),
            &got[n.saturating_sub(40)..(n + 40).min(got.len())],
            &want[n.saturating_sub(40)..(n + 40).min(want.len())],
        );
    }
    eprintln!("[photoatomic-type1] reproduced NJOY's {} bytes exactly", want.len());
}

#[test]
fn type2_output_matches_njoy2016_to_machine_precision() {
    let Some(reference) = reference_file_or_skip(
        "acer",
        "z6_photoatomic_njoy2016.type2.ace",
        "acer photoatomic type 2",
    ) else {
        return;
    };
    let Some((table, _, _)) = build(AceFileType::Type2Binary) else {
        return;
    };
    let want = std::fs::read(&reference).expect("read NJOY's Type-2 photoatomic ACE");
    let got = table.to_type2_bytes();
    assert_eq!(
        got.len(),
        want.len(),
        "Type-2 length: a photo-atomic table blocks XSS one real per record \
         (ner = 1, acepa.f90:297), not 512 as the fast path does"
    );
    // The header record must be byte-exact -- it carries no arithmetic.
    assert_eq!(
        &got[..508],
        &want[..508],
        "the Type-2 header record (ZAID, AWR, kT, date, comment, MAT, IZ/AW, \
         NXS, JXS) must reproduce NJOY byte for byte"
    );
    // The data words are full doubles here, unlike Type 1's 12 printed digits,
    // so this is the only comparison in the suite that can see a single ulp.
    // It does: see the V&V record for what carries it and what does not.
    let mut differing = 0usize;
    let mut worst_ulps = 0i64;
    let mut worst_rel = 0.0f64;
    for i in 0..table.xss.len() {
        let o = 512 + 16 * i;
        let njoy = f64::from_le_bytes(want[o..o + 8].try_into().expect("8 bytes"));
        let ulps = (table.xss[i].to_bits() as i64 - njoy.to_bits() as i64).abs();
        if ulps != 0 {
            differing += 1;
            worst_ulps = worst_ulps.max(ulps);
            let rel = if njoy != 0.0 {
                (table.xss[i] - njoy).abs() / njoy.abs()
            } else {
                (table.xss[i] - njoy).abs()
            };
            worst_rel = worst_rel.max(rel);
        }
    }
    // Measured 2026-09-21: 93 of 449 words differ, worst 3 ulps / 6.7e-16
    // relative. The bound is set just above that, not at it -- a gate that a
    // recompilation could trip says nothing about the port.
    assert!(
        worst_rel <= 1.0e-14,
        "Type-2 data words must agree with NJOY to machine precision; worst was \
         {worst_rel:.3e} relative ({worst_ulps} ulps) over {differing} of {} words",
        table.xss.len()
    );
    assert!(
        differing <= 150,
        "{differing} of {} Type-2 words differ from NJOY at the last bit; 93 were \
         measured on 2026-09-21, so a large rise means something changed",
        table.xss.len()
    );
    eprintln!(
        "[photoatomic-type2] header byte-exact; {differing} of {} data words differ, \
         worst {worst_ulps} ulps ({worst_rel:.3e} relative)",
        table.xss.len()
    );
}

/// The ESZG block is stored as natural logs, and a zero must stay a zero
/// rather than becoming `-inf` (`acepa.f90:969`). Pinning it here because it
/// is the one transformation the builder does not apply and the writer does.
#[test]
fn eszg_is_logged_and_zeros_survive() {
    let Some((table, nes, _)) = build(AceFileType::Type1Ascii) else {
        return;
    };
    for (i, v) in table.xss.iter().take(5 * nes).enumerate() {
        assert!(
            v.is_finite(),
            "ESZG word {i} is {v}; a zero cross section must stay 0, not become -inf"
        );
    }
    // The energy column runs 1 keV to 1e5 MeV, so its logs bracket ln(1e-3)
    // and ln(1e5) in MeV.
    let e0 = table.xss[0];
    let e1 = table.xss[nes - 1];
    assert!(
        (e0 - (1.0e-3f64).ln()).abs() < 1.0e-6,
        "first grid point should be 1 keV in MeV, log is {e0}"
    );
    assert!(e1 > e0, "the energy column must ascend");
}
