// Reads reference files: needs `std` (gh:#1592).
#![cfg(feature = "std")]

// SPDX-License-Identifier: AGPL-3.0-only
//! **A corrupt numeric field stops the read, as it stops NJOY2016** (gh:#660).
//!
//! # Methodology
//!
//! ENDF/B-VIII.0 C-12 (`reference-data/endf/n-006_C_012-ENDF8.0.endf`, MAT 625) is edited
//! in memory at line 603, the first data line of MF=3/MT=2, field 2 (`4.74816700`):
//!
//! - **corrupt**: `4.7x81670`. NJOY2016.80 (`ab27c64`, gfortran, `build2/njoy`) running
//!   `reconr 20 21 / 'c12' / 625 0 / 0.001 / 0` on that tape stops with
//!   `At line 852 of file .../endf.f90 (unit = 20, file = 'tape20') Fortran runtime error:
//!   Bad value during floating point read`, exit status 2 (measured 2026-11-24): `contio`
//!   reads `(6e11.0,i4,i2,i3,i5)` with no `iostat`.
//! - **internal blank**: `4.74 8167`. The same NJOY run completes; gfortran's default
//!   `BLANK='NULL'` drops the blank, so the field is 4.748167.
//! - **text in MF=1/MT=451**: the description line 6 overwritten with letters, which NJOY reads
//!   as text (`a66`), so it is not a numeric record there.
//!
//! Pass criterion: [`Tape::from_bytes`] refuses the first with an `EndfParse` error naming the
//! line, MAT/MF/MT and field; reads the second as 4.748167; reads the third, with the MF=3
//! data unchanged.
//!
//! # Prediction (2026-11-24, before the change) and result
//!
//! Before the change all three read, the first silently as 0.0 b (`parse_endf_float`'s text
//! fallback, applied to every record). Predicted after: refusal, 4.748167, unchanged; and no
//! other test in the crate moves, since every held tape is one NJOY reads. **Result
//! (2026-11-24): the three cases as predicted, but the "no other test moves" half was wrong
//! on the first run**: Cu-65 (`ace_five_route_parity`) stopped at line 50331, an MF=32
//! compact (`LCOMP = 2`) INTG record whose columns pack integers (`  1 -2    -`), which NJOY
//! does not read with `e11.0`. MF=32 now keeps the text fallback as MF=1/MT=451 does (the two
//! sections `Tape` keeps raw), and the whole crate suite passes (counts in the commit).

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;

const TAPE: &str = "n-006_C_012-ENDF8.0.endf";

/// The tape with 1-based line `lineno`'s columns `[from, from + text.len())` replaced.
fn edited(original: &str, lineno: usize, from: usize, text: &str) -> Vec<u8> {
    let mut out = String::with_capacity(original.len());
    for (i, line) in original.split_inclusive('\n').enumerate() {
        if i + 1 == lineno {
            let mut l = line.to_string();
            l.replace_range(from..from + text.len(), text);
            out.push_str(&l);
        } else {
            out.push_str(line);
        }
    }
    out.into_bytes()
}

#[test]
fn corrupt_numeric_field_is_refused_and_internal_blank_reads_as_fortran_does() {
    let Some(path) = reference_endf_or_skip(TAPE, "gh:#660 corrupt field") else { return };
    let original = std::fs::read_to_string(&path).unwrap();
    assert_eq!(&original.lines().nth(602).unwrap()[11..22], " 4.74816700", "line 603 moved");

    let good = Tape::from_bytes(original.as_bytes()).unwrap();
    let value = |t: &Tape| t.section(625, 3, 2).unwrap().rows[3][1];
    assert_eq!(value(&good), 4.748167);

    // Corrupt: refused by name.
    let err = Tape::from_bytes(&edited(&original, 603, 11, " 4.7x81670 ")).unwrap_err();
    let msg = err.to_string();
    println!("corrupt field: {msg}");
    for part in ["line 603", "field 2", "MAT 625 MF 3 MT 2", "4.7x81670"] {
        assert!(msg.contains(part), "error {msg:?} should name {part:?}");
    }

    // Internal blank: Fortran BN.
    let blank = Tape::from_bytes(&edited(&original, 603, 11, " 4.74 8167 ")).unwrap();
    assert_eq!(value(&blank), 4.748167);

    // Text in an MF=1/MT=451 description line stays legal.
    let line6 = original.lines().nth(5).unwrap();
    assert!(&line6[70..75] == " 1451", "line 6 should be MF=1/MT=451: {line6:?}");
    let text = Tape::from_bytes(&edited(&original, 6, 0, "free text, not numbers: x.yz-q ")).unwrap();
    assert_eq!(value(&text), 4.748167);
}
