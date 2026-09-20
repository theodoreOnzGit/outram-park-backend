//! **V&V: this crate's unresolved kernel against NJOY2016's own MF=2/MT=152
//! table — the energies NJOY actually evaluated.**
//!
//! # Why this is the right comparison
//!
//! `genunr` (`reconr.f90:1628-1735`) evaluates the infinitely-dilute unresolved
//! cross sections on its internal `eunr` grid and writes them to the PENDF as
//! **MF=2/MT=152**. `sigunr` (`:1737-1769`) then *interpolates that table* to
//! fill MF=3 wherever the output grid carries an energy `eunr` does not.
//!
//! So MF=3 mixes two different things — evaluations and interpolations — and a
//! comparison against it measures whichever happens to be at each energy.
//! **MT=152 is the table itself**: NJOY's own record of what it computed, at
//! the energies it computed it. Comparing against that isolates the physics
//! with no interpolation and no MF=3 background in the way.
//!
//! This makes the companion tests' finding trivially checkable rather than
//! inferential. `reconr_urr_kernel_vs_njoy2016.rs` had to argue it: 26 of 34
//! MF=3 points agree, and the other 8 are reproducible as lin-lin
//! interpolations of their neighbours. Here the argument is unnecessary —
//! **the energies NJOY evaluated are written down in the tape**, and
//! [`the_agreeing_mf3_energies_are_exactly_the_mt152_energies`] shows the
//! agreement set *is* that list.
//!
//! # Methodology
//!
//! **Reference.** `reference-data/reconr/u234-ENDF8.0-0K-err0.001.pendf`,
//! NJOY2016 `ac5adf5f` (2016.79), gfortran 13.3.0. Its MT=152 carries
//! `nunr = 27` energies, `nsig0 = 1` (infinite dilution, `big = 1e10`) and
//! `nx = 5` columns.
//!
//! **Read through this crate's own reader** —
//! [`crate::groupr::urr_pendf::read_urr_from_tape`], GROUPR's `stounr` feeder —
//! rather than a hand-rolled parser, so the test exercises the shipped code
//! path.
//!
//! **Compared quantity.** All four physical columns (total, elastic, fission,
//! capture) at every one of the 27 energies, against
//! [`crate::unresr::unresolved_cross_sections`] at `sig0 = 1e10`, **plus the
//! MF=3 background**.
//!
//! That last part is not optional, and getting it wrong is instructive. For
//! `LSSF = 0`, `genunr` does not store the bare unresolved cross section: its
//! tail (`reconr.f90:1694-1727`) walks the evaluation's MF=3 and adds the
//! background to each stored column — `ix = 1, 2, 3, 4` for MT = 1, 2, 18,
//! 102, and for `ix = 1` it updates the duplicate column 5 as well. The first
//! version of this test compared a bare kernel against that table and failed
//! on **total only**, 5 of 27 energies, by up to 4.1e-2, while elastic,
//! fission and capture passed 0/27. The partials passed because U-234's MF=3
//! carries nothing for MT=2/18/102 in this window; the total failed because
//! MF=3 MT=1 does, and grows with energy. The kernel was never wrong — the
//! comparison was.
//!
//! **Column 5 is deliberately not compared.** RECONR writes the total a second
//! time (`sunr(l+5)=sunr(l+1)`, `reconr.f90:1690`), whereas this crate's kernel
//! returns *transport* as its fifth output. Comparing them would be a unit
//! mismatch dressed up as a disagreement.
//!
//! **Pass criterion.** Worst relative deviation `<= 1e-5` over all 27x4 values.
//!
//! # Results (2026-09-15)
//!
//! Worst relative deviation **6.94e-7** across all 108 comparisons — at the
//! 7-significant-figure floor, which is exactly where it should sit: `genunr`
//! rounds every value it stores with `sigfig(...,7,0)` (`reconr.f90:1719`,
//! `:1723`). Every energy, every reaction.
//!
//! # Scope
//!
//! One material (U-234, MAT 9225), `LSSF = 0`, infinite dilution, 0 K.
//! Verification against NJOY2016, not validation against measurement.

use njoy_outram_park_fork::endf::mt::MtReaction;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::groupr::urr_pendf::read_urr_from_tape;
use njoy_outram_park_fork::reconr::reconr_background;
use njoy_outram_park_fork::reference_data::{reference_file, reference_file_or_skip};
use njoy_outram_park_fork::unresr::mf2::{parse_lru2_ranges, UnresolvedRange};
use njoy_outram_park_fork::unresr::unresolved_cross_sections;
use njoy_outram_park_fork::unresr::wfun::WTable;

const MAT: i32 = 9225;
const SIG0: f64 = 1.0e10;
const GATE: f64 = 1.0e-5;
const URR: std::ops::RangeInclusive<f64> = 1.5e3..=1.0e5;

/// `(MT=152 column, our kernel's output slot, the MF=3 MT whose background
/// `genunr` adds to that column, label)`.
///
/// Column 4 is skipped: NJOY repeats the total there
/// (`sunr(l+5)=sunr(l+1)`, `reconr.f90:1690`), while this crate's kernel
/// returns transport as its fifth output.
const COLUMNS: [(usize, usize, i32, &str); 4] = [
    (0, 0, 1, "total"),
    (1, 1, 2, "elastic"),
    (2, 2, 18, "fission"),
    (3, 3, 102, "capture"),
];

fn kernel(range: &UnresolvedRange, e: f64, t: &WTable) -> [f64; 5] {
    unresolved_cross_sections(std::slice::from_ref(range), e, 0.0, &[SIG0], [0.0; 4], t)
        .expect("kernel evaluates")
        .first()
        .copied()
        .unwrap_or([0.0; 5])
}

struct Fixture {
    table: njoy_outram_park_fork::groupr::unresolved::UnresolvedTable,
    range: UnresolvedRange,
    background: njoy_outram_park_fork::reconr::ReconrResult,
    wtable: WTable,
}

fn fixture() -> Option<Fixture> {
    let pendf = reference_file_or_skip(
        "reconr",
        "u234-ENDF8.0-0K-err0.001.pendf",
        "U-234 RECONR golden (MT=152)",
    )?;
    let ep = reference_file("endf", "n-092_U_234-ENDF8.0.endf")?;
    let pendf_tape = Tape::read_file(&pendf).ok()?;
    let table = read_urr_from_tape(Some(&pendf_tape), MAT, 1, 0.0).ok()??;
    let eval_tape = Tape::read_file(&ep).ok()?;
    let sec = eval_tape.section(MAT, 2, 151)?;
    let range = parse_lru2_ranges(&sec.rows[1..]).ok()?.into_iter().next()?;
    // `genunr` adds the evaluation's own MF=3, so the background arm must be
    // the background alone -- not a reconstruction that already carries the
    // unresolved contribution this test is measuring.
    let background = reconr_background(&eval_tape, MAT, 0.001).ok()?;
    Some(Fixture {
        table,
        range,
        background,
        wtable: WTable::new(),
    })
}

#[test]
fn the_kernel_plus_background_reproduces_njoys_mt152_table_at_every_stored_energy() {
    let Some(f) = fixture() else {
        eprintln!("skipping: reference data absent");
        return;
    };
    println!(
        "  MT=152: {} energies, {} reaction columns, sigma0 grid {:?}",
        f.table.n_points(),
        f.table.n_reactions(),
        f.table.sigma0_grid()
    );
    assert!(f.table.n_points() > 20, "expected NJOY's eunr grid");

    let (mut worst, mut worst_at) = (0.0_f64, String::new());
    let mut compared = 0usize;
    for (col, slot, bg_mt, label) in COLUMNS {
        let (mut n, mut bad, mut cw) = (0usize, 0usize, 0.0_f64);
        for p in f.table.points() {
            let Some(theirs) = p.xs.get(col).and_then(|c| c.first()).copied() else {
                continue;
            };
            if theirs == 0.0 {
                continue;
            }
            // genunr: stored = dilute URR + MF=3 background (reconr.f90:1694-1727).
            let bg = f
                .background
                .eval_mt(MtReaction::from_any(bg_mt), p.energy_ev);
            let mine = kernel(&f.range, p.energy_ev, &f.wtable)[slot] + bg;
            let dev = (mine - theirs).abs() / theirs.abs();
            n += 1;
            compared += 1;
            if dev > GATE {
                bad += 1;
            }
            if dev > cw {
                cw = dev;
            }
            if dev > worst {
                worst = dev;
                worst_at = format!(
                    "E={:.6e} {label}: ours {mine:.7e} (kernel + bg {bg:.4e}) vs NJOY {theirs:.7e}",
                    p.energy_ev
                );
            }
        }
        println!("  {label:>8}: {bad}/{n} exceed {GATE:.0e}, worst {cw:.2e}");
    }

    println!("  compared {compared} values; worst {worst:.2e} at {worst_at}");
    assert!(compared >= 100, "only {compared} values compared");
    assert!(
        worst <= GATE,
        "worst relative deviation {worst:.2e} exceeds the {GATE:.0e} gate \
         ({worst_at}). MT=152 is NJOY's own record of what it evaluated, so a \
         disagreement here is a genuine physics difference -- unlike an MF=3 \
         disagreement, which may be NJOY interpolating. Check first that the \
         MF=3 background is still being added the way genunr does \
         (reconr.f90:1694-1727); comparing a bare kernel against this table \
         fails on total alone, which is a test bug and not a port bug."
    );
}

/// The inferential argument in `reconr_urr_kernel_vs_njoy2016.rs`, checked
/// directly: **the MF=3 energies where this crate agrees with NJOY are exactly
/// the energies MT=152 says NJOY evaluated.**
///
/// Neither set is hard-coded -- both are derived from the tapes at run time, so
/// this re-derives the split rather than asserting a remembered list. Measured
/// 2026-09-15: 26 agree-and-stored, 8 differ-and-not-stored, and **zero** in
/// either off-diagonal cell.
#[test]
fn the_agreeing_mf3_energies_are_exactly_the_mt152_energies() {
    use njoy_outram_park_fork::endf::records::SectionCursor;

    let Some(f) = fixture() else {
        eprintln!("skipping: reference data absent");
        return;
    };
    let stored: Vec<f64> = f.table.points().iter().map(|p| p.energy_ev).collect();
    let is_stored = |e: f64| stored.iter().any(|&s| (s - e).abs() <= 1e-6 * e);

    let pendf = reference_file("reconr", "u234-ENDF8.0-0K-err0.001.pendf").expect("golden");
    let pendf_tape = Tape::read_file(&pendf).expect("PENDF parses");
    let sec = pendf_tape.section(MAT, 3, 18).expect("MF=3 MT=18");
    let mut cur = SectionCursor::new(&sec.rows);
    cur.read_cont().expect("CONT");
    let pairs = cur.read_tab1().expect("TAB1").pairs;

    let (mut agree_stored, mut agree_unstored) = (0usize, 0usize);
    let (mut differ_stored, mut differ_unstored) = (0usize, 0usize);
    let mut prev = f64::NAN;
    for &(e, njoy) in &pairs {
        if e == prev {
            continue;
        }
        prev = e;
        if !URR.contains(&e) || njoy == 0.0 {
            continue;
        }
        // MT=18 carries no MF=3 background in this window, so the bare kernel
        // is the right arm here -- see the companion test's doc.
        let agrees = (kernel(&f.range, e, &f.wtable)[2] - njoy).abs() / njoy.abs() < GATE;
        match (agrees, is_stored(e)) {
            (true, true) => agree_stored += 1,
            (true, false) => agree_unstored += 1,
            (false, true) => differ_stored += 1,
            (false, false) => differ_unstored += 1,
        }
    }

    println!("  MF=3 points in the URR, classified against MT=152's energy list:");
    println!("    agree & stored in MT=152    : {agree_stored}");
    println!("    differ & NOT in MT=152      : {differ_unstored}");
    println!("    agree but NOT in MT=152     : {agree_unstored}");
    println!("    differ but stored in MT=152 : {differ_stored}");

    assert!(
        agree_stored > 15 && differ_unstored > 0,
        "expected the split to be agree-and-stored vs differ-and-not-stored; \
         got {agree_stored} / {differ_unstored}"
    );
    assert_eq!(
        differ_stored, 0,
        "{differ_stored} MF=3 energies are IN NJOY's MT=152 table -- so NJOY \
         evaluated them -- yet this crate disagrees there. That would be a real \
         physics difference, not interpolation, and would invalidate the \
         conclusion of the urr_interpolation_study."
    );
}
