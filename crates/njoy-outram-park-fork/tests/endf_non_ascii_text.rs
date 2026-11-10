// SPDX-License-Identifier: AGPL-3.0-only
//
// gh:#660: `Tape::read` on a tape whose MF=1/MT=451 text holds a non-ASCII
// character.

//! # A non-ASCII character in MF=1 text reads, and changes nothing else (gh:#660)
//!
//! **Methodology.** Take the He-4 alpha-sublibrary tape
//! (`reference-data/endf/a-002_He_004-ENDF8.0.endf`, ENDF/B-VIII.0, MAT 228)
//! and replace one character of two MF=1/MT=451 text lines with `é`:
//!
//! - line 6, column 11 (`  2-He-  4 ` becomes `  2-He-  4é`): in UTF-8 the
//!   two bytes straddle the first field boundary, the issue's reproducer;
//! - line 7, column 2 (`NDS` becomes `NéS`).
//!
//! Each modified tape is written twice, as UTF-8 (`é` = `C3 A9`) and as
//! Latin-1 (`é` = `E9`). NJOY2016 reads a record as fixed-width columns
//! (`endf.f90:55`), so for Latin-1 one byte is one column and the result must
//! equal the unmodified tape's parse: the same section keys in the same
//! order, and the same rows (text fields read as `0.0` either way). For UTF-8
//! the port counts characters as columns (the issue's guidance), so the same
//! equality is required.
//!
//! **Pass criterion:** both reads succeed and are row-for-row identical to
//! the original.
//!
//! **Results (2026-11-10, x86_64-linux, release):** pass, both encodings.
//! Before the fix (develop `a836e2f07`) the UTF-8 tape panicked in
//! `parse_line` ("byte index 11 is not a char boundary") and the Latin-1 tape
//! failed with `Io(InvalidData, "stream did not contain valid UTF-8")`.

use njoy_outram_park_fork::endf::parse::parse_line;
use njoy_outram_park_fork::endf::tape::Tape;

fn he4_bytes() -> Vec<u8> {
    let p = njoy_outram_park_fork::reference_data::reference_endf_dir()
        .join("a-002_He_004-ENDF8.0.endf");
    std::fs::read(p).expect("He-4 alpha tape present in reference-data/endf")
}

/// Replace the character at 0-based `col` of 1-based line `line_no` with
/// `replacement` (raw bytes), leaving every other byte as it was.
fn replace_char(tape: &[u8], line_no: usize, col: usize, replacement: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(tape.len() + 4);
    for (i, line) in tape.split_inclusive(|&b| b == b'\n').enumerate() {
        if i + 1 == line_no {
            assert!(line[col].is_ascii(), "the original column is ASCII");
            out.extend_from_slice(&line[..col]);
            out.extend_from_slice(replacement);
            out.extend_from_slice(&line[col + 1..]);
        } else {
            out.extend_from_slice(line);
        }
    }
    out
}

fn with_e_acute(encoding_bytes: &[u8]) -> Vec<u8> {
    let t = he4_bytes();
    let t = replace_char(&t, 6, 10, encoding_bytes);
    replace_char(&t, 7, 1, encoding_bytes)
}

fn assert_same_parse(a: &Tape, b: &Tape) {
    assert_eq!(a.len(), b.len());
    for (sa, sb) in a.sections().iter().zip(b.sections()) {
        assert_eq!(sa.key, sb.key);
        assert_eq!(sa.rows, sb.rows, "section {:?}", sa.key);
    }
    assert_eq!(a.materials(), vec![228]);
}

#[test]
fn utf8_e_acute_in_mf1_text_reads_like_the_original() {
    let original = Tape::read(&he4_bytes()[..]).unwrap();
    let modified = Tape::read(&with_e_acute("é".as_bytes())[..])
        .expect("a UTF-8 accent in MF=1 text must not fail the read");
    assert_same_parse(&original, &modified);
}

#[test]
fn latin1_e_acute_in_mf1_text_reads_like_the_original() {
    let original = Tape::read(&he4_bytes()[..]).unwrap();
    let modified = Tape::read(&with_e_acute(&[0xE9])[..])
        .expect("a Latin-1 accent in MF=1 text must not fail the read");
    assert_same_parse(&original, &modified);
}

/// The line parser on its own: a multibyte character straddling column 11
/// keeps MAT/MF/MT in columns 67-75 and reads the text field as 0.0.
#[test]
fn parse_line_counts_characters_not_bytes() {
    let line = "  2-He-  4éLLNL       EVAL-DEC99 R.M.White,D.A.Resler,S.I.Warshaw  228 1451";
    let rl = parse_line(line).expect("no panic, no error");
    assert_eq!((rl.mat, rl.mf, rl.mt), (228, 1, 451));
    assert_eq!(rl.fields, [0.0; 6]);
    // An ASCII data line is unchanged by the column rule.
    let data = " 2.004000+3 3.968219+0         -1          0          0          0 228 1451";
    let rl = parse_line(data).unwrap();
    assert_eq!(rl.fields[0], 2004.0);
    assert_eq!(rl.fields[2], -1.0);
    assert_eq!((rl.mat, rl.mf, rl.mt), (228, 1, 451));
}
