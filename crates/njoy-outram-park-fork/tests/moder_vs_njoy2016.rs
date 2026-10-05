//! MODER material selection + ASCII write, compared **byte for byte** with the
//! tape NJOY2016's own MODER writes (GitHub #536).
//!
//! # Methodology
//!
//! **Oracle:** upstream NJOY2016 (`ac5adf5`, gfortran 13.3.0, built
//! 2026-10-05 from `upstream_source/NJOY2016`), MODER in selection mode
//! (`nin = 1`, ENDF/PENDF), coded output on unit 30:
//!
//! ```text
//! moder
//!  1 30/
//!  'moder selection: H-2 (128) + Li-6 (325), ENDF/B-VIII.0'/
//!  20 128/
//!  21 325/
//!  0/
//! ```
//!
//! with `tape20` = `reference-data/endf/n-001_H_002-ENDF8.0.endf` and
//! `tape21` = `reference-data/endf/n-003_Li_006-ENDF8.0.endf`. NJOY's tape30
//! is committed as `reference-data/moder/h2-li6-select.moder.tape` (955 233
//! bytes, 11 789 lines); the deck and provenance are in
//! `reference-data/moder/README.md`.
//!
//! **Port:** `Tape::read_file` on the same two tapes, `moder::select_materials`
//! with `[(0, 128), (1, 325)]` and the same 66-character TPID, then
//! `Tape::write`.
//!
//! **What is compared.** Both outputs are split into lines and the lines
//! paired in order. Each of NJOY's lines is put in exactly one class by what
//! NJOY wrote, not by what the port wrote:
//!
//! - `tpid`: the first line;
//! - `sentinel`: SEND / FEND / MEND / TEND (MT = 0, MF = 0, MAT = 0 or -1);
//! - `hollerith`: MF=1/MT=451 rows after the third, i.e. the descriptive text
//!   (rows 1-3 are numeric CONTs on an ENDF-6 tape, and the directory rows
//!   after the text are numeric too, see below);
//! - `numeric`: every other line.
//!
//! On numeric lines each of the six 11-column fields is classed
//! `integer` (NJOY's `i11`, no decimal point) or `float` (`a11`). The test
//! counts, per class, lines identical in columns 1-75 and in 76-80 (the
//! sequence number) separately, and float fields whose 11 characters match.
//! **Values** are compared as well: every field parsed by this crate's own
//! parser, from both files, must be equal bit for bit.
//!
//! **Prediction (written before the first run, 2026-10-05).** From the
//! documented caveats in `Tape::write` and `src/moder/README.md`: the
//! `a11` float fields should match NJOY's in every column (the port is a
//! line-for-line translation of `a11`, including the nine-figure branch);
//! the CONT integer fields will not (written as `a11` floats); the Hollerith
//! text is lost (the `[f64; 6]` row model reads text as 0.0); sentinels
//! differ (NJOY writes blank fields); and the sequence column differs on
//! almost every line. Values should agree everywhere except Hollerith rows.
//!
//! # Results (2026-10-05, branch `claude/nuclear-data-gaps`, release)
//!
//! Full table and causes: `verification_and_validation/moder_vs_njoy2016.md`.
//!
//! ```text
//! lines: port 11793 / NJOY 11793
//!       tpid:      1 lines, cols 1-75 identical      1, seq 76-80 identical      1
//!   sentinel:    120 lines, cols 1-75 identical      0, seq 76-80 identical      0
//!  hollerith:    616 lines, cols 1-75 identical      0, seq 76-80 identical    216
//!    numeric:  11056 lines, cols 1-75 identical   9264, seq 76-80 identical     21
//! float (a11) fields on numeric lines: 59098 / 59098 identical
//! integer (i11) fields on numeric lines: 0 / 4998 identical
//! blank fields on numeric lines (directory rows): 0 / 2240 identical
//! whole lines byte-identical: 1 / 11793
//! ```
//!
//! The prediction held in every class. The **section structure is
//! identical** (same MAT/MF/MT on every line); **all 59 098 `a11` float
//! fields are character-identical to NJOY's**; **every parsed value on a
//! numeric line agrees bit for bit**. The byte differences are the CONT
//! integer fields, blank directory fields, sentinels, sequence numbers and
//! the **MF=1/MT=451 descriptive text, which the port loses** (616 rows,
//! read as 0.0). So the selection is NJOY's, and the writer is not a
//! byte-faithful MODER: `Section`'s `[f64; 6]` rows carry no record type.

use njoy_outram_park_fork::endf::parse::parse_line;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::moder::{select_materials, MaterialSelection};
use njoy_outram_park_fork::reference_data::reference_file_or_skip;

const TPID: &str = "moder selection: H-2 (128) + Li-6 (325), ENDF/B-VIII.0";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Tpid,
    Sentinel,
    Hollerith,
    Numeric,
}

#[derive(Default, Debug)]
struct Counts {
    lines: usize,
    identical_1_75: usize,
    identical_76_80: usize,
}

fn mat_mf_mt(line: &str) -> (i32, i32, i32) {
    let p = |a: usize, b: usize| {
        line.get(a..b)
            .unwrap_or("")
            .trim()
            .parse::<i32>()
            .unwrap_or(0)
    };
    (p(66, 70), p(70, 72), p(72, 75))
}

fn pad80(s: &str) -> String {
    format!("{s:<80}")
}

#[test]
fn moder_selection_matches_njoy2016_tape() {
    let tag = "moder-vs-njoy2016";
    let Some(h2) = reference_file_or_skip("endf", "n-001_H_002-ENDF8.0.endf", tag) else {
        return;
    };
    let Some(li6) = reference_file_or_skip("endf", "n-003_Li_006-ENDF8.0.endf", tag) else {
        return;
    };
    let Some(golden) = reference_file_or_skip("moder", "h2-li6-select.moder.tape", tag) else {
        return;
    };

    let inputs = [
        Tape::read_file(&h2).expect("H-2 parses"),
        Tape::read_file(&li6).expect("Li-6 parses"),
    ];
    // NJOY writes card 2's tpid into columns 1-66 of a MAT=1 MF=0 MT=0 record
    // (moder.f90 -> tpidio). The port writes `tpid` verbatim, so the caller
    // passes the padded record; the comparison below reports what remains.
    let tpid_record = format!("{TPID:<66}{:4}{:2}{:3}{:5}", 1, 0, 0, 0);
    let sel = [
        MaterialSelection {
            tape_index: 0,
            mat: 128,
        },
        MaterialSelection {
            tape_index: 1,
            mat: 325,
        },
    ];
    let tape = select_materials(&inputs, &tpid_record, &sel).expect("selection");
    let mut ours = Vec::new();
    tape.write(&mut ours).expect("write");
    let ours = String::from_utf8(ours).unwrap();
    let theirs = std::fs::read_to_string(&golden).unwrap();
    let ours: Vec<&str> = ours.lines().collect();
    let theirs: Vec<&str> = theirs.lines().collect();

    println!("lines: port {} / NJOY {}", ours.len(), theirs.len());
    assert_eq!(
        ours.len(),
        theirs.len(),
        "line count differs: the section structure is not NJOY's"
    );

    let mut tpid = Counts::default();
    let mut sentinel = Counts::default();
    let mut hollerith = Counts::default();
    let mut numeric = Counts::default();
    let (mut float_fields, mut float_same) = (0usize, 0usize);
    let (mut int_fields, mut int_same) = (0usize, 0usize);
    let (mut blank_fields, mut blank_same) = (0usize, 0usize);
    let mut value_mismatch = Vec::new();
    let mut float_mismatch = Vec::new();
    let mut key_mismatch = Vec::new();
    let mut row_in_451 = 0usize;
    let mut nwd_text_rows = 0usize;

    for (i, (a, b)) in ours.iter().zip(theirs.iter()).enumerate() {
        let (a, b) = (pad80(a), pad80(b));
        let kb = mat_mf_mt(&b);
        if mat_mf_mt(&a) != kb && i > 0 {
            key_mismatch.push(i + 1);
        }
        let class = if i == 0 {
            Class::Tpid
        } else if kb.2 == 0 || kb.1 == 0 || kb.0 <= 0 {
            row_in_451 = 0; // a SEND closes MF=1/MT=451; the next material restarts the count
            Class::Sentinel
        } else if kb.1 == 1 && kb.2 == 451 {
            row_in_451 += 1;
            // Row 3 on an ENDF-6 tape is (awi, emax, lrel, 0, nsub, nver); row 4
            // is (temp, 0, ldrv, 0, nwd, nxc); nwd text rows follow.
            if row_in_451 == 4 {
                nwd_text_rows = parse_line(&b).unwrap().fields[4] as usize;
            }
            if row_in_451 > 4 && row_in_451 <= 4 + nwd_text_rows {
                Class::Hollerith
            } else {
                Class::Numeric
            }
        } else {
            Class::Numeric
        };
        let c = match class {
            Class::Tpid => &mut tpid,
            Class::Sentinel => &mut sentinel,
            Class::Hollerith => &mut hollerith,
            Class::Numeric => &mut numeric,
        };
        c.lines += 1;
        c.identical_1_75 += usize::from(a[..75] == b[..75]);
        c.identical_76_80 += usize::from(a[75..80] == b[75..80]);

        if class != Class::Numeric {
            continue;
        }
        // Field-level comparison on numeric lines.
        for f in 0..6 {
            let (fa, fb) = (&a[11 * f..11 * f + 11], &b[11 * f..11 * f + 11]);
            let integer = !fb.contains('.') && !fb.trim().is_empty();
            if fb.trim().is_empty() {
                // NJOY leaves a field blank where the record has none (the
                // MF=1/MT=451 directory rows, `2x,2i11` after 22 blanks).
                blank_fields += 1;
                blank_same += usize::from(fa == fb);
            } else if integer {
                int_fields += 1;
                int_same += usize::from(fa == fb);
            } else {
                float_fields += 1;
                if fa == fb {
                    float_same += 1;
                } else if float_mismatch.len() < 10 {
                    float_mismatch.push(format!("line {}: port {fa:?} NJOY {fb:?}", i + 1));
                }
            }
        }
        let (va, vb) = (
            parse_line(&a).unwrap().fields,
            parse_line(&b).unwrap().fields,
        );
        for f in 0..6 {
            if va[f].to_bits() != vb[f].to_bits() && value_mismatch.len() < 10 {
                value_mismatch.push(format!(
                    "line {} field {}: {} vs {}",
                    i + 1,
                    f + 1,
                    va[f],
                    vb[f]
                ));
            }
        }
    }

    for (name, c) in [
        ("tpid", &tpid),
        ("sentinel", &sentinel),
        ("hollerith", &hollerith),
        ("numeric", &numeric),
    ] {
        println!(
            "{name:>10}: {:6} lines, cols 1-75 identical {:6}, seq 76-80 identical {:6}",
            c.lines, c.identical_1_75, c.identical_76_80
        );
    }
    println!("float (a11) fields on numeric lines: {float_same} / {float_fields} identical");
    println!("integer (i11) fields on numeric lines: {int_same} / {int_fields} identical");
    println!(
        "blank fields on numeric lines (directory rows): {blank_same} / {blank_fields} identical"
    );
    let whole = ours
        .iter()
        .zip(theirs.iter())
        .filter(|(a, b)| pad80(a) == pad80(b))
        .count();
    println!("whole lines byte-identical: {whole} / {}", theirs.len());
    for m in &float_mismatch {
        println!("  float mismatch {m}");
    }
    for m in &value_mismatch {
        println!("  value mismatch {m}");
    }

    assert!(
        key_mismatch.is_empty(),
        "MAT/MF/MT columns differ on lines {:?}",
        &key_mismatch[..key_mismatch.len().min(10)]
    );
    assert_eq!(
        float_same, float_fields,
        "a11 float fields differ from NJOY's: {float_mismatch:?}"
    );
    assert!(
        value_mismatch.is_empty(),
        "parsed values differ: {value_mismatch:?}"
    );
    assert!(
        hollerith.lines > 0,
        "no MF=1/MT=451 text rows found; the classifier is wrong"
    );
    // The documented divergences, pinned so a fix shows up as a failure that
    // asks for this record to be updated rather than passing silently.
    assert_eq!(
        int_same, 0,
        "CONT integer fields now match NJOY's i11: update the record (#536)"
    );
    assert_eq!(
        hollerith.identical_1_75, 0,
        "Hollerith text now survives: update the record (#536)"
    );
}
