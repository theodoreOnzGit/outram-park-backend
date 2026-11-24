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
//!
//! # Round 2 (2026-11-24): strict everywhere NJOY reads a number
//!
//! The text fallback is now confined to the cards that are not `e11.0` records, found by
//! walking the section as MODER does:
//!
//! - **MF=1/MT=451:** only the description lines (`moder::layout`'s `file1`) are text; a
//!   corrupt HEAD/CONT/dictionary field is refused. Case: C-12 line 2 (the HEAD), field 2.
//! - **MF=32:** only `LCOMP = 2` INTG cards (`file32`'s walk) are not `e11.0`, and each is
//!   checked as `intgio` reads it (`2i5,1x,18i3` for `NDIGIT = 2`, ...). Cases on Cu-65
//!   ENDF/B-VIII.0 (MAT 2931): a corrupt INTG integer (line 50331, `-2` to `-x`) and a corrupt
//!   field of a numeric LIST card in the same section (line 50326).
//! - **MAT/MF/MT and sequence columns** are Fortran integers on every card (`i4,i2,i3,i5`,
//!   blanks ignored), a text card included. Case: C-12 line 603's MAT `625` to `6x5`.
//! - **GENDF readers** (`errorr::gendf::parse_gendf_text`, GROUPR's input GENDF, POWR's
//!   reader, ACER's `gamout`) check their cards with `check_gendf_text`. Case: a GENDF with a
//!   corrupt LIST field.
//!
//! Prediction (before the sweep): every ENDF tape in `reference-data/endf/` and in NJOY2016's
//! `tests/resources/` still reads; only the non-ENDF files there are refused. **Result: as
//! predicted** (`tests/endf_strict_read_held_tapes.rs`: 152 tapes read; the 7 refused files
//! are `README.md`, four `.py` generators, a `.readme` and `thermalscattering.ace`). The
//! MF=32 walk accounted for every MF=32 section read (22 MATs; it never fell back to the
//! lenient read), and checked 28 342 INTG cards (MATs 1725, 2925, 2931, 4234, 7825, two of
//! them on two tapes each); the MF=1/MT=451 walk never fell back where a card was refused.

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

#[test]
fn strict_outside_text_and_intg_cards() {
    let Some(path) = reference_endf_or_skip(TAPE, "gh:#660 round 2") else { return };
    let original = std::fs::read_to_string(&path).unwrap();

    // MF=1/MT=451 HEAD (not a text line): refused.
    let err = Tape::from_bytes(&edited(&original, 2, 11, " 1.1x93650")).unwrap_err().to_string();
    println!("MF=1 HEAD: {err}");
    assert!(err.contains("line 2") && err.contains("MF 1 MT 451"), "{err}");

    // MAT column garbage: refused, even though the fields are fine.
    let err = Tape::from_bytes(&edited(&original, 603, 66, "6x5")).unwrap_err().to_string();
    println!("MAT column: {err}");
    assert!(err.contains("line 603") && err.contains("MAT field"), "{err}");
}

#[test]
fn mf32_intg_cards_are_read_as_intgio_reads_them() {
    let Some(path) = reference_endf_or_skip("n-029_Cu_065-ENDF8.0.endf", "gh:#660 MF=32") else {
        return;
    };
    let original = std::fs::read_to_string(&path).unwrap();
    assert_eq!(&original.lines().nth(50330).unwrap()[..23], "    5    1   1 -2    -2", "line 50331 moved");
    assert!(Tape::from_bytes(original.as_bytes()).is_ok(), "the held tape reads");

    let err = Tape::from_bytes(&edited(&original, 50331, 20, " -x")).unwrap_err().to_string();
    println!("INTG card: {err}");
    assert!(err.contains("line 50331") && err.contains("INTG (NDIGIT=2) integer 6"), "{err}");

    let err = Tape::from_bytes(&edited(&original, 50326, 11, " 3.7x00000")).unwrap_err().to_string();
    println!("MF=32 LIST card: {err}");
    assert!(err.contains("line 50326") && err.contains("MF 32 MT 151"), "{err}");
}

#[test]
fn gendf_readers_refuse_a_corrupt_card() {
    use njoy_outram_park_fork::endf::parse::check_gendf_text;
    use njoy_outram_park_fork::errorr::gendf::parse_gendf_text;
    let good = [
        "label                                                                1 0  0    0",
        " 6.000000+3 1.189690+1          0          1         -1          11306 1451    1",
        " 2.960000+2 0.000000+0          1          1          2          01306 1451    2",
        " 1.000000+5 2.000000+7                                            1306 1451    3",
        "                                                                  1306 1  0    4",
        "",
    ]
    .join("\n");
    let good = good.as_str();
    assert!(parse_gendf_text(good).is_ok());
    let bad = good.replace(" 1.000000+5 2.000000+7", " 1.0000x0+5 2.000000+7");
    let err = parse_gendf_text(&bad).unwrap_err().to_string();
    println!("GENDF: {err}");
    assert!(err.contains("GENDF line 4"), "{err}");
    // Another material's cards are read as `tosend` reads them: identifier columns only.
    assert!(check_gendf_text(&bad, Some(9999)).is_ok());
    assert!(check_gendf_text(&bad, Some(1306)).is_err());
}

/// Cu-65's MF=32 compact block rewritten at `NDIGIT = 6`: each 18-integer `NDIGIT = 2` card
/// becomes one to three 8-integer cards in `intgio`'s `(2i5,1x,8i7,i4,i2,i3,i5)` layout (81
/// columns, MAT in columns 68-71), and the block's CONT gets `NDIGIT = 6` and the new count.
/// Returns the tape and the 1-based line of the first INTG card.
fn cu65_at_ndigit6(original: &str) -> (String, usize) {
    let lines: Vec<&str> = original.split('\n').collect();
    let ci = 50327; // the compact block's CONT, line 50328
    let c = lines[ci];
    assert_eq!(&c[70..75], "32151");
    assert_eq!(c[22..33].trim(), "2", "NDIGIT");
    let nn: usize = c[44..55].trim().parse().unwrap();
    let int = |s: &str| -> i64 {
        let t: String = s.chars().filter(|&ch| ch != ' ').collect();
        if t.is_empty() { 0 } else { t.parse().unwrap() }
    };
    let mut cards = Vec::new();
    for card in &lines[ci + 1..ci + 1 + nn] {
        let (ii, jj) = (int(&card[0..5]), int(&card[5..10]));
        let vals: Vec<i64> = (0..18).map(|k| int(card.get(11 + 3 * k..14 + 3 * k).unwrap_or(""))).collect();
        for m in (0..18).step_by(8) {
            let mut chunk: Vec<i64> = vals[m..(m + 8).min(18)].to_vec();
            if m > 0 && chunk.iter().all(|&v| v == 0) {
                continue;
            }
            chunk.resize(8, 0);
            let body: String = chunk.iter().map(|v| format!("{v:7}")).collect();
            cards.push(format!("{ii:5}{:5} {body}{:4}{:2}{:3}{:5}", jj + m as i64, 2931, 32, 151, 1));
        }
    }
    let cont = format!("{}{:11}{}{:11}{}", &c[..22], 6, &c[33..44], cards.len(), &c[55..]);
    let mut out: Vec<String> = lines[..ci].iter().map(|s| s.to_string()).collect();
    out.push(cont);
    out.extend(cards);
    out.extend(lines[ci + 1 + nn..].iter().map(|s| s.to_string()));
    (out.join("\n"), ci + 2)
}

/// **`NDIGIT = 6` INTG cards follow `intgio`'s 81-column layout** (gh:#660).
///
/// NJOY2016.80 (`build2/njoy`) run on the tape [`cu65_at_ndigit6`] builds (the same
/// construction in Python), with `moder 20 -21 / moder -21 22`, ran to the end (rc 0,
/// 2026-11-24) and wrote all 4 003 INTG cards back unchanged: 81 columns, MAT in 68-71.
/// Prediction, written before that run: exactly that. Here the same tape must read, keyed to
/// MAT 2931 MF 32 MT 151 throughout; a corrupt `i7` integer, the card shifted back to the
/// 80-column layout, and a corrupt MAT in columns 68-71 must each be refused.
#[test]
fn mf32_ndigit6_cards_follow_intgio() {
    let Some(path) = reference_endf_or_skip("n-029_Cu_065-ENDF8.0.endf", "gh:#660 NDIGIT=6") else {
        return;
    };
    let original = std::fs::read_to_string(&path).unwrap();
    let rows0 = Tape::from_bytes(original.as_bytes()).unwrap().section(2931, 32, 151).unwrap().rows.len();
    let (nd6, first) = cu65_at_ndigit6(&original);
    let first_card = nd6.lines().nth(first - 1).unwrap().to_string();
    assert_eq!(first_card.len(), 81, "{first_card:?}");

    let t = Tape::from_bytes(nd6.as_bytes()).expect("the NDIGIT=6 tape reads");
    let rows = t.section(2931, 32, 151).unwrap().rows.len();
    println!("MF=32 rows: {rows0} at NDIGIT=2, {rows} at NDIGIT=6");
    assert_eq!(rows, rows0 - 2943 + 4003, "every NDIGIT=6 card stays in MAT 2931 MF 32 MT 151");
    assert_eq!(t.materials(), vec![2931]);

    let with_line = |text: &str, lineno: usize, new: &str| -> String {
        text.split('\n')
            .enumerate()
            .map(|(i, l)| if i + 1 == lineno { new.to_string() } else { l.to_string() })
            .collect::<Vec<_>>()
            .join("\n")
    };
    // A corrupt i7 integer (the first, columns 12-18).
    let bad = format!("{}{}{}", &first_card[..11], "    -7x", &first_card[18..]);
    let err = Tape::from_bytes(with_line(&nd6, first, &bad).as_bytes()).unwrap_err().to_string();
    println!("i7: {err}");
    assert!(err.contains("INTG (NDIGIT=6) integer 3"), "{err}");
    // The same card in the 80-column layout (MAT in 67-70): intgio would misread it.
    let left = format!("{}{}", &first_card[..66], &first_card[67..]);
    let err = Tape::from_bytes(with_line(&nd6, first, &left).as_bytes()).unwrap_err().to_string();
    println!("80-column: {err}");
    assert!(err.contains("80-column layout"), "{err}");
    // A corrupt MAT in columns 68-71.
    let mat = format!("{}29x1{}", &first_card[..67], &first_card[71..]);
    assert!(Tape::from_bytes(with_line(&nd6, first, &mat).as_bytes()).is_err());
}
