//! **V&V: the MF=2/MT=152 section this crate writes, against the one NJOY2016
//! writes for the same material.**
//!
//! # What this closes
//!
//! `genunr` (`reconr.f90:1628-1735`) stores the infinitely-dilute unresolved
//! cross sections as MF=2/MT=152, and GROUPR's `stounr` reads them back. This
//! crate had the **reader** ([`crate::groupr::urr_pendf::read_urr_from_tape`])
//! but not the writer, so its own RECONR output could not feed its own GROUPR.
//! [`crate::reconr::urr::build_mt152`] is the missing half.
//!
//! # Methodology
//!
//! Build the section for U-234 (ENDF/B-VIII.0, MAT 9225, `LSSF = 0`) and
//! compare against NJOY2016 `ac5adf5f`'s own, from
//! `reference-data/reconr/u234-ENDF8.0-0K-err0.001.pendf`.
//!
//! Both sides are read through **this crate's reader**, so the comparison is of
//! decoded tables rather than of text — a formatting difference is not a
//! physics difference, and conflating them would make this test fail for the
//! wrong reason.
//!
//! **Pass criteria.** Identical `LSSF`, interpolation law, dilution grid and
//! energy grid; and every stored cross section within `1e-5` relative.
//!
//! # Results (2026-09-15)
//!
//! Energy grid: **27 points, matching NJOY's exactly**. Worst relative
//! deviation over all 27x4 stored values: **9.00e-7**, at the seven-significant-
//! figure floor `genunr`'s own `sigfig(...,7,0)` (`:1719`) imposes.
//!
//! # Scope
//!
//! One material, `LSSF = 0`, one temperature (0 K), infinite dilution only
//! (`nsig0 = 1`). An `LSSF = 1` material takes the early return at `:1694` and
//! stores bare cross sections; no such case is exercised here because U-234 is
//! the only `LSSF = 0` material held, and the `LSSF = 1` ones (U-235, U-238)
//! are the ones whose MT=152 this crate does not yet need to write.
//! Verification against NJOY2016, not validation.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::groupr::urr_pendf::{read_urr_from_tape, read_urr_table};
use njoy_outram_park_fork::reconr::urr::build_mt152;
use njoy_outram_park_fork::reconr::{mf1, reconr_background};
use njoy_outram_park_fork::reference_data::{reference_file, reference_file_or_skip};
use njoy_outram_park_fork::unresr::mf2::parse_lru2_ranges;

const MAT: i32 = 9225;
const GATE: f64 = 1.0e-5;

#[test]
fn the_written_mt152_matches_njoys_own() {
    let Some(pendf) = reference_file_or_skip(
        "reconr",
        "u234-ENDF8.0-0K-err0.001.pendf",
        "U-234 RECONR golden (MT=152)",
    ) else {
        return;
    };
    let Some(ep) = reference_file("endf", "n-092_U_234-ENDF8.0.endf") else {
        eprintln!("skipping: U-234 evaluation absent");
        return;
    };

    let eval_tape = Tape::read_file(&ep).expect("evaluation parses");
    let material = mf1::parse_material_info(
        eval_tape.section(MAT, 1, 451).expect("MF=1/451"),
    )
    .expect("material info");
    let sec = eval_tape.section(MAT, 2, 151).expect("MF=2/151");
    let ranges = parse_lru2_ranges(&sec.rows[1..]).expect("LRU=2 ranges");
    let background = reconr_background(&eval_tape, MAT, 0.001).expect("background");

    let rows = build_mt152(
        material.za,
        material.awr,
        &ranges,
        &background.sections,
        0.0,
        0.001,
    )
    .expect("writer runs")
    .expect("U-234 has an unresolved range, so a section is written");

    let mine = read_urr_table(&rows, 0.0).expect("our section round-trips through our reader");
    let theirs = read_urr_from_tape(
        Some(&Tape::read_file(&pendf).expect("PENDF parses")),
        MAT,
        1,
        0.0,
    )
    .expect("NJOY MT=152 parses")
    .expect("NJOY MT=152 present");

    println!(
        "  ours : {} energies, {} columns, sigma0 {:?}",
        mine.n_points(),
        mine.n_reactions(),
        mine.sigma0_grid()
    );
    println!(
        "  NJOY : {} energies, {} columns, sigma0 {:?}",
        theirs.n_points(),
        theirs.n_reactions(),
        theirs.sigma0_grid()
    );

    assert_eq!(
        mine.n_reactions(),
        theirs.n_reactions(),
        "column count differs -- genunr writes five (the fifth is the total repeated)"
    );
    assert_eq!(
        mine.sigma0_grid(),
        theirs.sigma0_grid(),
        "dilution grid differs; RECONR stores a single infinite dilution"
    );
    assert_eq!(
        mine.n_points(),
        theirs.n_points(),
        "energy-grid size differs: ours {} vs NJOY {}. The stored grid is `eunr`, \
         built from the unresolved parameter energies plus the shaded range \
         bounds -- if these differ, the grid construction has diverged from \
         rdf2u0/rdf2u1, not the physics.",
        mine.n_points(),
        theirs.n_points()
    );

    let (mut worst, mut worst_at) = (0.0_f64, String::new());
    for (a, b) in mine.points().iter().zip(theirs.points()) {
        let de = (a.energy_ev - b.energy_ev).abs() / b.energy_ev;
        assert!(
            de < 1.0e-9,
            "energy grids diverge: ours {:.7e} vs NJOY {:.7e}",
            a.energy_ev,
            b.energy_ev
        );
        for (col, label) in [(0, "total"), (1, "elastic"), (2, "fission"), (3, "capture")] {
            let (Some(x), Some(y)) = (
                a.xs.get(col).and_then(|c| c.first()).copied(),
                b.xs.get(col).and_then(|c| c.first()).copied(),
            ) else {
                continue;
            };
            if y == 0.0 {
                continue;
            }
            let dev = (x - y).abs() / y.abs();
            if dev > worst {
                worst = dev;
                worst_at =
                    format!("E={:.6e} {label}: ours {x:.7e} vs NJOY {y:.7e}", a.energy_ev);
            }
        }
    }
    println!("  worst stored-value deviation {worst:.2e} at {worst_at}");
    assert!(
        worst <= GATE,
        "worst stored-value deviation {worst:.2e} exceeds {GATE:.0e} ({worst_at})"
    );
}

/// The fifth column is the total repeated, not transport.
///
/// `sunr(l+5) = sunr(l+1)` (`reconr.f90:1690`), and for `LSSF = 0` the
/// background is added to both (`:1721-1724`). Asserted because this crate's
/// kernel returns *transport* as its fifth output, so the obvious wiring is
/// wrong and would go unnoticed — nothing else reads column 5.
#[test]
fn the_fifth_column_repeats_the_total() {
    let Some(ep) = reference_file("endf", "n-092_U_234-ENDF8.0.endf") else {
        eprintln!("skipping: U-234 evaluation absent");
        return;
    };
    let eval_tape = Tape::read_file(&ep).expect("evaluation parses");
    let material =
        mf1::parse_material_info(eval_tape.section(MAT, 1, 451).expect("MF=1/451")).expect("mf1");
    let ranges = parse_lru2_ranges(&eval_tape.section(MAT, 2, 151).expect("MF=2/151").rows[1..])
        .expect("LRU=2");
    let background = reconr_background(&eval_tape, MAT, 0.001).expect("background");
    let rows = build_mt152(material.za, material.awr, &ranges, &background.sections, 0.0, 0.001)
        .expect("writer runs")
        .expect("section written");
    let table = read_urr_table(&rows, 0.0).expect("round-trips");

    for p in table.points() {
        let total = p.xs[0][0];
        let fifth = p.xs[4][0];
        assert_eq!(
            total, fifth,
            "at E={:.6e} column 5 is {fifth:.7e} but the total is {total:.7e}; \
             genunr repeats the total there (reconr.f90:1690)",
            p.energy_ev
        );
    }
    println!("  column 5 repeats the total at all {} points", table.n_points());
}
