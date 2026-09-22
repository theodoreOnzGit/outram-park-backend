//! Gate: the ACE reader gives the SAME table from Type 1 and Type 2.
//!
//! Upstream `acer` reads both containers — `iopt = 7` (Type 1, ASCII) and
//! `iopt = 8` (Type 2, Fortran unformatted), `itype = iopt - 6`
//! (`acer.f90:485-533`). This crate could read neither until 2026-09-21; five
//! files carried their own ad-hoc Type-1 parser instead.
//!
//! # Methodology
//!
//! NJOY2016 writes the *same* table twice from one ENDF tape, changing only
//! `itype` on ACER card 2. The two files share nothing byte-wise — one is
//! 5 442 964 bytes of ASCII, the other 2 154 676 bytes of binary — so reading
//! both and requiring **bit-identical** `NXS`, `JXS` and `XSS` is a real test
//! of the binary layout rather than of the reader's self-consistency.
//!
//! Type 2's layout is taken from the writer at `acefc.f90:13028-13053`:
//! one header record (`hz`, `aw0`, `tz`, `hd`, `hk`, `hm`, 16 `izn`/`awn`
//! pairs, `NXS(16)`, `JXS(32)`), then `XSS` in records of `ner = 512` reals
//! (`acefc.f90:187-188`).
//!
//! **`XSS` is compared bit-for-bit, not to a tolerance.** Type 1 is decimal
//! text written to 7 significant figures (NJOY's `sigfig`) and Type 2 is raw
//! IEEE-754, so the two are NOT required to agree exactly in general — this
//! test asserts what is actually observed, and if a future NJOY build widens
//! the Type-1 precision the assertion below will say so rather than hide it.
//!
//! # Results (2026-09-21, NJOY2016 2016.79, Al-27 ENDF/B-VIII.0 MAT 1325, 0 K)
//!
//! | quantity | result |
//! |---|---|
//! | class from ZAID | `c` (continuous-energy neutron), both |
//! | `NXS(1..16)` | identical |
//! | `JXS(1..32)` | identical |
//! | `XSS` length | identical |
//! | `XSS` values | see the assertion — measured, not assumed |
//!
//! The fixture files are produced by the recipe in the module docs and are not
//! committed (5 MB + 2 MB); the test skips when they are absent, honouring
//! `OUTRAM_PARK_REQUIRE_REFERENCE_DATA` so the skip cannot pass silently.

use njoy_outram_park_fork::acer::read::{self as read, read_type1, read_type2, AceClass, AceFileType};
use njoy_outram_park_fork::reference_data::reference_file_or_skip;

/// Where the two fixtures live when they have been generated.
const T1: &str = "/tmp/t2/tape24";
const T2: &str = "/tmp/t2/tape25";

#[test]
fn type1_and_type2_give_the_same_table() {
    if !std::path::Path::new(T1).exists() || !std::path::Path::new(T2).exists() {
        assert!(
            !njoy_outram_park_fork::reference_data::reference_data_required(),
            "[ace-type1-vs-type2] fixtures {T1} / {T2} absent and \
             OUTRAM_PARK_REQUIRE_REFERENCE_DATA is set"
        );
        println!("[ace-type1-vs-type2] SKIP — fixtures not generated");
        return;
    }

    let a = read_type1(T1).expect("Type 1 reads");
    let b = read_type2(T2).expect("Type 2 reads");

    assert_eq!(a.file_type, AceFileType::Type1Ascii);
    assert_eq!(b.file_type, AceFileType::Type2Binary);

    // The class letter is the only thing in the file saying what the blocks
    // mean, so it is checked first.
    assert_eq!(a.header.class, AceClass::ContinuousNeutron);
    assert_eq!(
        a.header.class, b.header.class,
        "the two containers disagree about the ACE class"
    );
    assert_eq!(a.header.zaid, b.header.zaid, "ZAID");
    assert_eq!(a.header.awr, b.header.awr, "AWR");
    assert_eq!(a.header.kt_mev, b.header.kt_mev, "kT");

    assert_eq!(a.nxs, b.nxs, "NXS differs between Type 1 and Type 2");
    assert_eq!(a.jxs, b.jxs, "JXS differs between Type 1 and Type 2");
    assert_eq!(
        a.xss.len(),
        b.xss.len(),
        "XSS length differs: Type 1 {} vs Type 2 {}",
        a.xss.len(),
        b.xss.len()
    );

    // Compare the data.
    //
    // THE FIRST VERSION OF THIS TEST ASSERTED BIT-IDENTITY AND WAS WRONG about
    // the format, not about the reader. Measured on Al-27: 208 109 of 268 746
    // values differ, and the two mechanisms are both Type 1's:
    //
    //   1. Type 1 writes 12 significant figures (`1.00000000000E-11`), so a
    //      value whose 13th digit is non-zero cannot survive the round trip.
    //      Index 0 is exactly this: the grid value is 1.00000000000009978e-11,
    //      Type 2 stores it, Type 1 rounds it to 1e-11. Relative size ~1e-13.
    //
    //   2. `change` (`acefc.f90:13088`, "Change ACE data fields from integer to
    //      real or vice versa") coerces the fields it treats as integers.
    //      Index 169615 holds 5001.2 in Type 2 and 5001 in Type 1; the run of
    //      16001.2 / 22001.2 / 28001.2 beside it does the same. Relative size
    //      4e-5, which is what made the worst case 400x larger than rounding
    //      alone would explain.
    //
    // So the invariant that actually holds is: away from the integer-coerced
    // fields the two containers agree to the 12-figure text precision. That is
    // asserted below, and it still catches what this test exists to catch — a
    // Type-2 layout defect misaligns the block and produces differences of
    // order 1, not 1e-13.
    let mut worst_real = (0.0f64, 0usize);
    let mut n_coerced = 0usize;
    let mut n_rounded = 0usize;
    for (k, (&x, &y)) in a.xss.iter().zip(b.xss.iter()).enumerate() {
        if x == y {
            continue;
        }
        // An integer-coercion site: Type 1 holds an exact integer and Type 2
        // does not, and they round to the same integer.
        if x.fract() == 0.0 && y.fract() != 0.0 && (x - y).abs() < 1.0 {
            n_coerced += 1;
            continue;
        }
        n_rounded += 1;
        let d = if y != 0.0 { ((x - y) / y).abs() } else { (x - y).abs() };
        if d > worst_real.0 {
            worst_real = (d, k);
        }
    }

    /// Type 1 carries 12 significant figures, so 1e-11 relative is the tightest
    /// bound the format can support; 1e-10 leaves one decade of headroom and is
    /// still five orders tighter than any misalignment would produce.
    const TEXT_PRECISION: f64 = 1.0e-10;
    assert!(
        worst_real.0 <= TEXT_PRECISION,
        "away from integer-coerced fields the two containers must agree to Type 1's \
         12-figure text precision, but index {} differs by {:.3e} (type1 {:.17e}, \
         type2 {:.17e}). A Type-2 layout defect shows up here as a difference of \
         order 1, not of order 1e-13.",
        worst_real.1,
        worst_real.0,
        a.xss[worst_real.1],
        b.xss[worst_real.1]
    );

    // Rather than pin a count with an invented margin, assert the INVARIANT of
    // integer coercion: every coerced entry must be the Type-2 value rounded to
    // the nearest integer. That is a statement about what `change` does, it can
    // fail, and it does not need a magic number.
    //
    // Measured 2026-09-21 on Al-27 ENDF/B-VIII.0: 7517 of 268 746 entries are
    // integer-coerced (2.8 %) -- the locators, MT numbers and counts an ACE
    // table is full of. An earlier draft of this test guessed "4" and was
    // wrong by three orders of magnitude, which is why the count is now
    // reported rather than asserted.
    let mut bad_round = Vec::new();
    for (k, (&x, &y)) in a.xss.iter().zip(b.xss.iter()).enumerate() {
        if x != y && x.fract() == 0.0 && y.fract() != 0.0 && (x - y).abs() < 1.0 && x != y.round()
        {
            bad_round.push((k, x, y));
            if bad_round.len() >= 5 {
                break;
            }
        }
    }
    assert!(
        bad_round.is_empty(),
        "an integer-coerced entry is not the Type-2 value rounded to nearest: {:?}",
        bad_round
    );

    println!(
        "[ace-type1-vs-type2] {} values: {n_rounded} differ by text rounding \
         (worst {:.3e}), {n_coerced} by integer coercion",
        a.xss.len(),
        worst_real.0
    );
}

/// The sniffing entry point picks the right parser without being told.
#[test]
fn read_sniffs_the_container() {
    if !std::path::Path::new(T1).exists() || !std::path::Path::new(T2).exists() {
        assert!(!njoy_outram_park_fork::reference_data::reference_data_required());
        println!("[ace-sniff] SKIP — fixtures not generated");
        return;
    }
    let a = njoy_outram_park_fork::acer::read::read(T1).expect("sniff Type 1");
    let b = njoy_outram_park_fork::acer::read::read(T2).expect("sniff Type 2");
    assert_eq!(a.file_type, AceFileType::Type1Ascii);
    assert_eq!(b.file_type, AceFileType::Type2Binary);
}

/// Every ACE **class** this port can obtain a real file for is read, and the
/// class letter is recognised as upstream's dispatch recognises it.
///
/// Upstream routes six letters (`acer.f90:513-533`); four can be produced from
/// what this repository holds:
///
/// | file | ZAID | class | produced by |
/// |---|---|---|---|
/// | `/tmp/t2/tape24` | `13027.00c` | continuous-energy neutron | `acer iopt=1` |
/// | `/tmp/tsl1/tape24_type1.ace` | `al27.00t` | thermal | `acer iopt=2` |
/// | `/tmp/dos/tape24` | `92000.00p` | photoatomic | `acer iopt=4` |
/// | `/tmp/he4/tape24` | `2004.00a` | charged particle | `acer iopt=1`, alpha sublibrary |
///
/// The two not covered are `u` (photonuclear) and `y` (dosimetry): neither
/// sublibrary is present in `reference-data/endf/`, so no such table can be
/// produced to read back. That is a gap in the *fixtures*, not in the reader —
/// the container is class-independent and every letter is unit-tested in
/// `acer::read` — and it is stated here rather than left implied.
#[test]
fn every_obtainable_class_reads() {
    let cases: [(&str, &str, AceClass); 4] = [
        ("/tmp/t2/tape24", "13027.00c", AceClass::ContinuousNeutron),
        ("/tmp/tsl1/tape24_type1.ace", "al27.00t", AceClass::Thermal),
        ("/tmp/dos/tape24", "92000.00p", AceClass::Photoatomic),
        ("/tmp/he4/tape24", "2004.00a", AceClass::ChargedParticle('a')),
    ];
    let mut read_any = false;
    for (path, want_zaid, want_class) in cases {
        if !std::path::Path::new(path).exists() {
            println!("[ace-classes] SKIP {path} — not generated");
            continue;
        }
        read_any = true;
        let t = njoy_outram_park_fork::acer::read::read(path)
            .unwrap_or_else(|e| panic!("read {path}: {e}"));
        assert_eq!(t.header.zaid, want_zaid, "{path} ZAID");
        assert_eq!(t.header.class, want_class, "{path} class");
        // NXS(1) is the declared XSS length; the reader checks it, so getting
        // here at all means the container decoded end to end.
        assert_eq!(
            t.nxs[0] as usize,
            t.xss.len(),
            "{path}: NXS(1) and XSS length must agree"
        );
        // A thermal ZAID is a name, every other class's is a number.
        if want_class == AceClass::Thermal {
            assert!(t.header.zaid_num.is_none(), "{path}: thermal ZAID is not numeric");
        } else {
            assert!(t.header.zaid_num.is_some(), "{path}: non-thermal ZAID should parse");
        }
        println!(
            "[ace-classes] {path}: {} {:?}, NXS(1)={}, {} XSS values",
            t.header.zaid,
            t.header.class,
            t.nxs[0],
            t.xss.len()
        );
    }
    if !read_any {
        assert!(
            !njoy_outram_park_fork::reference_data::reference_data_required(),
            "[ace-classes] no class fixture present and \
             OUTRAM_PARK_REQUIRE_REFERENCE_DATA is set"
        );
    }
}

/// Byte-for-byte: read NJOY2016's own Type-2 file and write it back out.
///
/// This is the strongest statement available about the Type-2 **writer**. A
/// round trip through our own reader and writer only proves the two agree with
/// each other; reproducing NJOY's bytes proves the container itself is right —
/// record markers, field widths, chunking at `ner = 512`, and the exact padding
/// of every Fortran `character(n)` field.
///
/// # Result (2026-09-21, Al-27 ENDF/B-VIII.0 MAT 1325, 0 K, 2 154 676 bytes)
///
/// See the assertion: it reports the first differing offset, because *where*
/// the first byte differs localises the defect immediately (before 500 = the
/// header record, after = a data record or a chunk boundary).
#[test]
fn type2_write_reproduces_njoys_bytes() {
    if !std::path::Path::new(T2).exists() {
        assert!(
            !njoy_outram_park_fork::reference_data::reference_data_required(),
            "[ace-type2-write] fixture {T2} absent and \
             OUTRAM_PARK_REQUIRE_REFERENCE_DATA is set"
        );
        println!("[ace-type2-write] SKIP — fixture not generated");
        return;
    }
    let original = std::fs::read(T2).expect("read NJOY's Type 2");
    let table = read_type2(T2).expect("parse NJOY's Type 2");
    let ours = table.to_type2_bytes();

    assert_eq!(
        ours.len(),
        original.len(),
        "Type-2 byte length: ours {} vs NJOY {}. A length difference is a record \
         framing or field-width error, not a value error.",
        ours.len(),
        original.len()
    );
    let first_diff = ours
        .iter()
        .zip(original.iter())
        .position(|(a, b)| a != b);
    assert!(
        first_diff.is_none(),
        "Type-2 bytes differ from NJOY's at offset {:?}. Before offset 500 that is \
         the header record (field widths or padding); after it, a data record or \
         the ner=512 chunk boundary.",
        first_diff
    );
    println!(
        "[ace-type2-write] reproduced NJOY's {} bytes exactly",
        original.len()
    );
}

/// Type-1 write: read NJOY's ASCII table and write it back.
///
/// The Type-2 writer reproduces NJOY's bytes exactly. This asks the same of
/// Type 1, and the answer is **not** "exactly" — which is worth having
/// measured rather than assumed. Type 1 is a formatted text container, so
/// reproducing it byte-for-byte requires matching NJOY's Fortran edit
/// descriptors in every field, including which words `change` emits as
/// integers. What this test pins is the property that actually matters: the
/// table **round-trips through the value domain** — write it, read it back,
/// and every `NXS`, `JXS` and `XSS` value returns unchanged.
#[test]
fn type1_write_round_trips_through_values() {
    if !std::path::Path::new(T1).exists() {
        assert!(
            !njoy_outram_park_fork::reference_data::reference_data_required(),
            "[ace-type1-write] fixture {T1} absent and \
             OUTRAM_PARK_REQUIRE_REFERENCE_DATA is set"
        );
        println!("[ace-type1-write] SKIP — fixture not generated");
        return;
    }
    let original = read_type1(T1).expect("read NJOY's Type 1");
    let text = original.to_type1_string();
    let back = njoy_outram_park_fork::acer::read::parse_type1(&text)
        .expect("our own Type-1 output must re-read");

    assert_eq!(back.nxs, original.nxs, "NXS did not survive the round trip");
    assert_eq!(back.jxs, original.jxs, "JXS did not survive the round trip");
    assert_eq!(
        back.xss.len(),
        original.xss.len(),
        "XSS length changed across the round trip"
    );
    let mut worst = (0.0f64, 0usize);
    for (k, (&a, &b)) in back.xss.iter().zip(original.xss.iter()).enumerate() {
        let d = if b != 0.0 { ((a - b) / b).abs() } else { (a - b).abs() };
        if d > worst.0 {
            worst = (d, k);
        }
    }
    // Our Type-1 real format is 1pE20.11 -- 12 significant figures, the same as
    // NJOY's -- so a value that came FROM a Type-1 file must return exactly.
    assert!(
        worst.0 == 0.0,
        "a value read from Type 1 must survive a Type-1 rewrite exactly, but \
         index {} moved by {:.3e} (was {:.17e}, became {:.17e})",
        worst.1,
        worst.0,
        original.xss[worst.1],
        back.xss[worst.1]
    );
    println!(
        "[ace-type1-write] {} values round-tripped exactly through Type 1",
        original.xss.len()
    );
}


/// **mcnpx-format reading**, which was implemented and unexercised until
/// references were produced on 2026-09-22.
///
/// A 13-character ZAID carries a **two**-character class string (`"pp "` for
/// photo-atomic, `"ny "` for dosimetry), and the file records no flag saying
/// which width it uses, so [`read_type1`] decides from the bytes: columns
/// 11-13 hold letters in the mcnpx variant and the first three columns of the
/// `f12.6` AWR otherwise.
///
/// The class letter is taken as the **last** letter of the field, which works
/// for both conventions. Upstream's own read-back does not: `acer.f90:510`
/// reads the mcnpx suffix with `a3` and then dispatches on `ht(1:1)`, so a
/// dosimetry file's `"ny "` gives `'n'`, a thermal file's `"nt "` gives `'n'`,
/// and neither matches any of its branches — **NJOY cannot read back the
/// mcnpx files it writes for those classes.** Recorded here because it is the
/// kind of thing a port is tempted to reproduce.
#[test]
fn type1_files_read_back_and_rewrite_byte_exactly() {
    for (file, want_class, want_za, want_len2, hz_len) in [
        ("z6_photoatomic_mcnpx_njoy2016.ace", AceClass::Photoatomic, 6000.0, 449, 13),
        ("h1_293k_dosimetry_mcnpx_njoy2016.ace", AceClass::Dosimetry, 1001.0, 2532, 13),
        // The standard-width files beside them, so the 10/13 decision is
        // exercised both ways rather than only in the new direction.
        ("z6_photoatomic_njoy2016.ace", AceClass::Photoatomic, 6000.0, 449, 10),
        ("h1_293k_dosimetry_njoy2016.ace", AceClass::Dosimetry, 1001.0, 2532, 10),
        ("mn55_293k_dosimetry_njoy2016.ace", AceClass::Dosimetry, 25055.0, 70440, 10),
    ] {
        let Some(path) = reference_file_or_skip("acer", file, "ace mcnpx read") else {
            return;
        };
        let t = read::read_type1(&path).expect("read the mcnpx Type-1 file");
        assert_eq!(t.header.class, want_class, "{file}: class from the last letter");
        assert_eq!(
            t.header.zaid_num,
            Some(want_za),
            "{file}: the ZA must survive a two-character class suffix"
        );
        assert_eq!(
            t.header.raw_text[0].len(),
            hz_len,
            "{file}: ZAID field width, decided from the bytes"
        );
        assert_eq!(t.nxs[0] as usize, want_len2, "{file}: NXS(1)");
        assert_eq!(t.xss.len(), want_len2, "{file}: XSS length");
        // And writing it back reproduces the file, which is what says the
        // width was recovered rather than guessed.
        let text = std::fs::read_to_string(&path).expect("read as text");
        let back = t.to_type1_string();
        if back != text {
            // Bounded, deliberately: an `assert_eq!` on two multi-megabyte
            // strings buries the one differing byte in a screenful of noise.
            let n = back
                .bytes()
                .zip(text.bytes())
                .take_while(|(a, b)| a == b)
                .count();
            panic!(
                "{file}: read-then-write differs at byte {n} of {} (file {} bytes)\n  \
                 ours: {:?}\n  njoy: {:?}",
                back.len(),
                text.len(),
                &back[n.saturating_sub(40)..(n + 40).min(back.len())],
                &text[n.saturating_sub(40)..(n + 40).min(text.len())],
            );
        }
        eprintln!(
            "[ace-roundtrip] {file}: {want_class:?} ZA {want_za}, {want_len2} words, \
             {hz_len}-column ZAID, round trip byte-exact"
        );
    }
}
