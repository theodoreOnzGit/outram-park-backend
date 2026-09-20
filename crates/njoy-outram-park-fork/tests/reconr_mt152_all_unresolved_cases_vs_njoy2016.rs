//! **V&V: MF=2/MT=152 on every path `reconr_mt152_writer_vs_njoy2016.rs`
//! could not reach — the Case A and Case B energy grids, and the `LSSF = 1`
//! early return.**
//!
//! # What this closes
//!
//! The U-234 gate next door verifies the writer on a Case C (`LRF=2`),
//! `LSSF = 0` material. Two paths stayed dark behind it, both recorded on
//! `bn:op-12lu` as "no held evaluation reaches it":
//!
//! 1. **The Case A / Case B energy grid.** `unresolved_grid` treated "these
//!    parameters tabulate no energies of their own" as a fallback and filled
//!    40 log-spaced points, flagged in its own doc comment as the one thing in
//!    the module that was not a literal translation.
//! 2. **The `LSSF = 1` early return** at `reconr.f90:1694`, which stores the
//!    bare kernel values and skips the MF=3 background.
//!
//! Neither was unreachable. Screening the held tapes for their MF=2 range
//! flags (`reference-data/endf/screen_endf_flags.py`) finds **Fe-58**
//! (ENDF/B-VIII.0 Beta4, MAT 2637) with `LRU=2, LRF=1, LFW=0, LSSF=1` over
//! `3.5e5 .. 3.0e6 eV` — Case A *and* `LSSF = 1` together — and **U-238**
//! (ENDF/B-VIII.0, MAT 9237) with Case C and `LSSF = 1`, which isolates the
//! early return from the grid change.
//!
//! # What the Case A grid actually is
//!
//! `rdf2u0` (`reconr.f90:1288-1308`) does not interpolate a fallback. It walks
//! the built-in 78-node `egridu` from `el` to `eh`, stepping to the first node
//! at or above `E + E/100`, taking every node strictly below `eh`. For Fe-58
//! that is eleven nodes, `4.0e5 … 2.5e6`, plus the two shaded bounds — **13
//! energies**, against the 42 the log-spaced fallback produced. The fallback
//! was not merely unverified; it was the wrong grid.
//!
//! Two further divergences surfaced from reading the three `rdf2u*`
//! subroutines side by side, both fixed in the same change and both gated
//! here: the gap-fill step is `E + E/100` in `rdf2u0`/`rdf2u1` but
//! `E + E/1000` in `rdf2u2`; and `intunr` is **5** (log-log) by default
//! (`rdfil2:809`), overridden only by Case C (`rdf2u2:1487`). It had been
//! hard-coded to 2, which is U-234's value and no one else's.
//!
//! # Results (2026-09-15)
//!
//! | material | case | L values | `LSSF` | `intunr` | energies | worst value deviation |
//! |---|---|---|---|---|---|---|
//! | U-238 (MAT 9237) | C (`LRF=2`) | 0,1,2 | 1 | 5 | 84, exact | **1.00e-13** |
//! | synthetic (MAT 9998) | B (`LRF=1`, `LFW=1`) | 0,1 | 0 | 5 | 16, exact | **4.22e-7** |
//! | Fe-58 (MAT 2637) | A (`LRF=1`, `LFW=0`) | 0,1,2,**3** | 1 | 5 | 13, exact | 3113x … 139210x — **upstream defect, see below** |
//!
//! U-238 agrees with NJOY on all 84 energies and all 336 stored values to
//! **1e-13** — every value carries the same seven printed digits, and the
//! worst case differs only in the last bit of the decoded `f64`. That is the
//! `LSSF = 1` closure: with no MF=3 background to add, every stored value is
//! one `sigfig(...,7,0)` of a kernel result, and the kernel already matched
//! NJOY to all seven printed figures (`reconr_urr_kernel_vs_njoy2016.rs`).
//! The U-234 gate's residual 9.00e-7 comes entirely from the background
//! addition that `LSSF = 1` skips.
//!
//! # Fe-58: the grid matches, the values must not
//!
//! **`unfac` (`reconr.f90:4473-4496`) has no `l >= 3` branch.** It is
//! `if (l.eq.0) … else if (l.eq.1) … else if (l.eq.2) … endif`, with no
//! `else`, so for `l >= 3` the `intent(out)` arguments `vl` and `ps` are never
//! assigned and keep their previous values. And `csunr1` writes
//! `vl = vl*e2` **inside its J loop** (`:4010`), so at `l >= 3` that
//! multiplies `vl` by another `sqrt(E)` for every J-state.
//!
//! Fe-58 has `NLS = 4`, so `L = 3` exists with two J-states, and its neutron
//! width is inflated by `E`. NJOY's stored total runs `1.42e4 b` at 350 keV to
//! `6.62e5 b` at 3 MeV — about 1900x the unitarity limit `4π/k²` (~7.5 b at
//! 350 keV), and rising with energy, which no cross section does there.
//!
//! This is not inferred. [`reproduce_csunr1_with_upstream_fall_through`] below
//! implements `csunr1` literally, fall-through included, and reproduces NJOY's
//! own numbers to better than 1e-3 — see
//! `fe58_disagreement_is_explained_by_upstreams_unfac_fall_through`. Our
//! values track the evaluation's own MF=3 instead, which for `LSSF = 1` is the
//! authoritative infinitely-dilute cross section.
//!
//! **We must not reproduce it, and the port already does not.** Our
//! [`penetrability_factor`] uses a `_ =>` arm that clamps `l >= 2` to the
//! `l = 2` formula. That is not a local fix — it is exactly what NJOY's
//! *UNRESR* does: `uunfac` (`unresr.f90:1213-1239`) writes a bare `else`
//! where RECONR's `unfac` and PURR's `unfac2` (`purr.f90:1487-1511`) write
//! `else if (l.eq.2)`. Our kernel ports `uunfac`, so it is faithful to the
//! module it comes from, and the defect is confined to the two that fall
//! through. Filed as `bn:op-12lu`'s child; upstream is not patched from here
//! (`upstream_source/` is read-only).
//!
//! # Case B has no evaluation, so the tape is synthetic
//!
//! Screening every held tape finds exactly one `LRU=2/LRF=1` range (Fe-58),
//! and it is `LFW=0`. Rather than leave Case B untested,
//! `reference-data/endf/synthetic-caseb-lfw1.generator.py` builds a minimal
//! format-legal tape that NJOY2016 processes cleanly, so RECONR's own output
//! is still the oracle — the same technique as
//! `reference-data/endf/photoat-synthetic-Z6.endf`. Its parameters are
//! invented and carry no physical claim; what they are chosen for is branch
//! coverage. One gap in the fission grid (`1.1e4 -> 3.0e4 eV`) exceeds
//! `wide = 1.26` so the `egridu` fill must fire, and one (`1.0e4 -> 1.1e4`)
//! does not so it must not; the grid comes out at NJOY's 16 energies exactly,
//! which is what tells us both branches went the right way.
//!
//! It is also the only `LSSF = 0` material here, so it exercises the MF=3
//! background addition on a Case B range.
//!
//! # Scope
//!
//! Three materials, 0 K, infinite dilution only (`nsig0 = 1`). Verification
//! against NJOY2016, not validation: nothing here is compared to measurement,
//! and the synthetic material does not exist.

use njoy_outram_park_fork::common::phys::{AMASSN_AMU, PI};
use njoy_outram_park_fork::endf::mt::MtReaction;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::groupr::urr_pendf::{read_urr_from_tape, read_urr_table};
use njoy_outram_park_fork::reconr::slbw::WAVE_K;
use njoy_outram_park_fork::reconr::urr::build_mt152;
use njoy_outram_park_fork::reconr::{mf1, reconr_background};
use njoy_outram_park_fork::reference_data::{reference_file, reference_file_or_skip};
use njoy_outram_park_fork::unresr::mf2::{parse_lru2_ranges, UnresolvedCase, UnresolvedRange};
use njoy_outram_park_fork::unresr::penetrability_factor;

/// One material to check, and what NJOY stores for it.
struct Case {
    label: &'static str,
    mat: i32,
    evaluation: &'static str,
    oracle: &'static str,
    /// Expected `LSSF` (`sunr(3)`, `reconr.f90:1652`).
    lssf: i32,
    /// Expected stored interpolation law (`sunr(6)`, `:1656`).
    intunr: i32,
    /// Expected `NUNR` — the size of `eunr`.
    n_energies: usize,
}

const U238: Case = Case {
    label: "U-238 (Case C, LRF=2, LSSF=1, L<=2)",
    mat: 9237,
    evaluation: "n-092_U_238.endf",
    oracle: "u238-ENDF8.0-0K-err0.001.mt152.pendf",
    lssf: 1,
    intunr: 5,
    n_energies: 84,
};

/// Case B has no evaluation anywhere in `reference-data/endf/` — screening
/// every held tape finds exactly one `LRU=2/LRF=1` range (Fe-58) and it is
/// `LFW=0`. This is a synthetic tape built to exercise the path, with
/// NJOY2016's own RECONR output as the oracle; see
/// `reference-data/endf/synthetic-caseb-lfw1.generator.py` for what it
/// contains and why each choice was made. `LSSF = 0` here, so unlike the two
/// real materials it also exercises the MF=3 background addition.
const CASEB: Case = Case {
    label: "synthetic (Case B, LRF=1, LFW=1, LSSF=0, L<=1)",
    mat: 9998,
    evaluation: "synthetic-caseb-lfw1.endf",
    oracle: "synthetic-caseb-lfw1-0K-err0.001.mt152.pendf",
    lssf: 0,
    // Case B has no INT of its own, so `rdfil2:809`'s default stands -- the
    // same value Case A takes, reached by a different route.
    intunr: 5,
    n_energies: 16,
};

const FE58: Case = Case {
    label: "Fe-58 Beta4 (Case A, LFW=0, LSSF=1, L up to 3)",
    mat: 2637,
    evaluation: "n-026_Fe_058-ENDF8.0-Beta4.endf",
    oracle: "fe58-ENDF8.0-Beta4-0K-err0.001.mt152.pendf",
    lssf: 1,
    intunr: 5,
    n_energies: 13,
};

/// The **released** ENDF/B-VIII.0 Fe-58, added 2026-09-15.
///
/// It is here because the obvious escape from the `unfac` defect — "our copy
/// is a pre-release, the shipping evaluation must have fixed it" — is false,
/// and that needs to be gated rather than asserted in prose. See
/// `released_and_beta_fe58_carry_the_same_unresolved_parameters`.
const FE58_RELEASED: Case = Case {
    label: "Fe-58 released VIII.0 (Case A, LFW=0, LSSF=1, L up to 3)",
    mat: 2637,
    evaluation: "n-026_Fe_058-ENDF8.0.endf",
    oracle: "fe58-ENDF8.0-0K-err0.001.mt152.pendf",
    lssf: 1,
    intunr: 5,
    n_energies: 13,
};

/// Everything needed to compare one material, or `None` when a tape is absent.
struct Loaded {
    rows: Vec<[f64; 6]>,
    ranges: Vec<UnresolvedRange>,
    njoy_tape: Tape,
    mf3_mt1: Vec<(f64, f64)>,
}

fn load(case: &Case) -> Option<Loaded> {
    let oracle = reference_file_or_skip("reconr", case.oracle, case.label)?;
    let Some(ep) = reference_file("endf", case.evaluation) else {
        eprintln!("skipping {}: evaluation absent", case.label);
        return None;
    };
    let eval_tape = Tape::read_file(&ep).expect("evaluation parses");
    let material = mf1::parse_material_info(eval_tape.section(case.mat, 1, 451).expect("MF=1/451"))
        .expect("material info");
    // `parse_lru2_ranges` wants the records AFTER the per-isotope CONT.
    let sec = eval_tape.section(case.mat, 2, 151).expect("MF=2/151");
    let ranges = parse_lru2_ranges(&sec.rows[1..]).expect("LRU=2 ranges");
    assert!(
        !ranges.is_empty(),
        "{} must carry an LRU=2 range",
        case.label
    );
    let background = reconr_background(&eval_tape, case.mat, 0.001).expect("background");
    let rows = build_mt152(
        material.za,
        material.awr,
        &ranges,
        &background.sections,
        0.0,
        0.001,
    )
    .expect("writer runs")
    .expect("an unresolved range is present, so a section is written");
    let mf3_mt1 = background
        .sections
        .iter()
        .find(|s| s.mt == MtReaction::from_any(1))
        .map(|s| s.pairs.clone())
        .unwrap_or_default();
    Some(Loaded {
        rows,
        ranges,
        njoy_tape: Tape::read_file(&oracle).expect("oracle parses"),
        mf3_mt1,
    })
}

/// The grid and the header flags — the two things `op-12lu` left open — must
/// match NJOY exactly for **both** materials, whatever the stored values do.
#[test]
fn case_a_and_lssf1_grid_and_flags_match_njoys_own() {
    for case in [&U238, &CASEB, &FE58, &FE58_RELEASED] {
        println!("\n=== {} ===", case.label);
        let Some(l) = load(case) else { continue };

        let njoy_sec = l
            .njoy_tape
            .section(case.mat, 2, 152)
            .expect("oracle carries MF=2/MT=152");
        let (mine_cont, theirs_cont) = (l.rows[0], njoy_sec.rows[0]);
        println!(
            "  CONT  ours [LSSF={} INT={}]   NJOY [LSSF={} INT={}]",
            mine_cont[2], mine_cont[5], theirs_cont[2], theirs_cont[5]
        );
        assert_eq!(
            theirs_cont[2] as i32, case.lssf,
            "{}: the ORACLE's LSSF is {} not the {} recorded here -- the \
             expectation in this test is stale, not the code",
            case.label, theirs_cont[2], case.lssf
        );
        assert_eq!(
            mine_cont[2] as i32, case.lssf,
            "{}: we store LSSF={} not {}",
            case.label, mine_cont[2], case.lssf
        );
        assert_eq!(
            mine_cont[5] as i32, case.intunr,
            "{}: stored interpolation law is {} not {}. intunr defaults to 5 \
             (rdfil2:809) and only rdf2u2 overrides it (:1487) -- hard-coding \
             lin-lin gives U-234's value and no one else's.",
            case.label, mine_cont[5], case.intunr
        );
        assert_eq!(
            mine_cont[5] as i32, theirs_cont[5] as i32,
            "{}: stored interpolation law differs from NJOY's",
            case.label
        );

        let mine =
            read_urr_table(&l.rows, 0.0).expect("our section round-trips through our reader");
        let theirs = read_urr_from_tape(Some(&l.njoy_tape), case.mat, 1, 0.0)
            .expect("NJOY MT=152 parses")
            .expect("NJOY MT=152 present");
        println!(
            "  ours {} energies / {} columns    NJOY {} energies / {} columns",
            mine.n_points(),
            mine.n_reactions(),
            theirs.n_points(),
            theirs.n_reactions()
        );

        assert_eq!(
            theirs.n_points(),
            case.n_energies,
            "{}: the oracle has {} energies, not the {} recorded here",
            case.label,
            theirs.n_points(),
            case.n_energies
        );
        assert_eq!(
            mine.n_points(),
            theirs.n_points(),
            "{}: energy-grid size differs: ours {} vs NJOY {}. The stored grid \
             is `eunr`; for Case A that is rdf2u0's unconditional egridu walk \
             (reconr.f90:1288-1308), NOT a log-spaced fill.",
            case.label,
            mine.n_points(),
            theirs.n_points()
        );
        assert_eq!(
            mine.n_reactions(),
            theirs.n_reactions(),
            "{}: column count differs",
            case.label
        );
        assert_eq!(
            mine.sigma0_grid(),
            theirs.sigma0_grid(),
            "{}: dilution grid differs",
            case.label
        );
        for (a, b) in mine.points().iter().zip(theirs.points()) {
            let de = (a.energy_ev - b.energy_ev).abs() / b.energy_ev;
            assert!(
                de < 1.0e-9,
                "{}: energy grids diverge: ours {:.7e} vs NJOY {:.7e}",
                case.label,
                a.energy_ev,
                b.energy_ev
            );
        }
        println!("  grid and flags match exactly");
    }
}

/// U-238 (`L <= 2`, so `unfac`'s gap never fires) must match NJOY's stored
/// values to the precision NJOY prints them — this is the `LSSF = 1` closure.
///
/// The gate is `1e-12`, not zero: NJOY writes seven significant figures, and
/// decoding the same seven digits on both sides can still leave the two `f64`s
/// differing in the last bit. Measured worst: **1.00e-13** over 336 values.
#[test]
fn u238_lssf1_stored_values_match_njoys_own_exactly() {
    compare_stored_values(&U238, 1.0e-12);
}

/// Case B's stored values, against NJOY's on the synthetic tape.
///
/// A looser gate than U-238's on purpose: this material is `LSSF = 0`, so
/// `genunr` adds the MF=3 background and re-quantises with
/// `sigfig(...,7,0)` (`:1719`), which puts the floor at NJOY's seven printed
/// figures rather than at the last bit. Same reason the U-234 gate next door
/// sits at 9.00e-7. Measured worst: **4.22e-7** over 64 values.
#[test]
fn case_b_stored_values_match_njoys_own() {
    compare_stored_values(&CASEB, 1.0e-5);
}

/// Compare every stored cross section for `case` against NJOY's.
fn compare_stored_values(case: &Case, gate: f64) {
    let Some(l) = load(case) else { return };
    let mine = read_urr_table(&l.rows, 0.0).expect("round-trips");
    let theirs = read_urr_from_tape(Some(&l.njoy_tape), case.mat, 1, 0.0)
        .expect("parses")
        .expect("present");

    let (mut worst, mut worst_at) = (0.0_f64, String::new());
    let mut compared = 0usize;
    for (a, b) in mine.points().iter().zip(theirs.points()) {
        for (col, name) in [(0, "total"), (1, "elastic"), (2, "fission"), (3, "capture")] {
            let (Some(x), Some(y)) = (
                a.xs.get(col).and_then(|c| c.first()).copied(),
                b.xs.get(col).and_then(|c| c.first()).copied(),
            ) else {
                continue;
            };
            compared += 1;
            if y == 0.0 {
                // A relative test cannot see a hard zero; assert it directly.
                assert_eq!(
                    x, 0.0,
                    "at E={:.6e} NJOY stores {name}=0 but we store {x:.7e}",
                    a.energy_ev
                );
                continue;
            }
            let dev = (x - y).abs() / y.abs();
            if dev > worst {
                worst = dev;
                worst_at = format!("E={:.6e} {name}: ours {x:.7e} vs NJOY {y:.7e}", a.energy_ev);
            }
        }
    }
    println!(
        "  {}: compared {compared} stored values; worst deviation {worst:.2e} {worst_at}",
        case.label
    );
    assert!(
        compared >= 4 * case.n_energies,
        "{}: only {compared} values compared -- the comparison is not covering \
         the table",
        case.label
    );
    assert!(
        worst < gate,
        "{}: worst stored-value deviation {worst:.2e} exceeds {gate:.0e} at {worst_at}",
        case.label
    );
}

/// `LSSF = 1` must skip the MF=3 background addition.
///
/// The structural half of the gate above: `genunr` returns at
/// `reconr.f90:1694` before "add on unresolved background from mf3". Asserted
/// separately because a value comparison would also pass if we added a
/// background that happened to be zero — U-238 carries a real, non-zero MF=3
/// across its unresolved window, so adding it would be plainly visible.
#[test]
fn lssf1_stores_the_bare_kernel_with_no_mf3_background() {
    let Some(ep) = reference_file("endf", U238.evaluation) else {
        eprintln!("skipping: U-238 evaluation absent");
        return;
    };
    let eval_tape = Tape::read_file(&ep).expect("evaluation parses");
    let material = mf1::parse_material_info(eval_tape.section(U238.mat, 1, 451).expect("MF=1/451"))
        .expect("material info");
    let ranges =
        parse_lru2_ranges(&eval_tape.section(U238.mat, 2, 151).expect("MF=2/151").rows[1..])
            .expect("LRU=2 ranges");
    assert_eq!(ranges[0].lssf, 1, "U-238's unresolved range is LSSF=1");
    let background = reconr_background(&eval_tape, U238.mat, 0.001).expect("background");

    let with_background = build_mt152(
        material.za,
        material.awr,
        &ranges,
        &background.sections,
        0.0,
        0.001,
    )
    .expect("writer runs")
    .expect("section written");
    // The same call with nothing to add. If the LSSF gate works these are
    // identical; if it does not, the MF=3 total alone shifts every row.
    let without_background = build_mt152(material.za, material.awr, &ranges, &[], 0.0, 0.001)
        .expect("writer runs")
        .expect("section written");
    assert_eq!(
        with_background, without_background,
        "LSSF=1 must take genunr's early return (reconr.f90:1694): passing a \
         real MF=3 background changed the stored table"
    );

    // ... and confirm the background offered was genuinely non-zero, so that
    // the comparison above is not vacuous.
    let mt1 = background
        .sections
        .iter()
        .find(|s| s.mt == MtReaction::from_any(1))
        .expect("U-238 has MF=3 MT=1");
    let (el, eh) = (ranges[0].el, ranges[0].eh);
    let positive = mt1
        .pairs
        .iter()
        .filter(|(e, x)| *e >= el && *e <= eh && *x > 0.0)
        .count();
    assert!(
        positive > 0,
        "the comparison is vacuous: MF=3 MT=1 has no positive point inside \
         {el:.3e}..{eh:.3e}"
    );
    println!(
        "  U-238 MF=3 MT=1 carries {positive} positive points across the URR, \
         and none of them reached MT=152"
    );
}

/// Our `l >= 3` penetrability must stay clamped, not fall through.
///
/// This is the one-line guard against "fixing" the port to match RECONR:
/// `uunfac` (`unresr.f90:1213-1239`) uses a bare `else`, so `l >= 2` all take
/// the `l = 2` formula. If this ever becomes a fall-through, Fe-58's stored
/// values run away exactly as NJOY's do.
#[test]
fn our_penetrability_clamps_l_at_two_like_uunfac() {
    for &(rho, rho_c) in &[(0.71, 0.715), (2.08, 2.094), (0.1, 0.1)] {
        let two = penetrability_factor(2, rho, rho_c);
        for l in 3..=6 {
            let hi = penetrability_factor(l, rho, rho_c);
            assert_eq!(
                hi, two,
                "penetrability_factor({l}, {rho}, {rho_c}) = {hi:?} but uunfac's \
                 `else` clamps every l >= 2 to the l=2 value {two:?}"
            );
        }
    }
    println!("  l >= 3 clamps to the l = 2 formula, as uunfac does");
}

/// Fe-58's value disagreement is upstream's `unfac` fall-through, quantitatively.
///
/// Reproduces `csunr1` literally — including the missing `l >= 3` branch and
/// the `vl = vl*e2` that sits inside the J loop — and shows it lands on NJOY's
/// published numbers. Without this, "we differ from the oracle" is an
/// assertion; with it, it is a measurement with a named cause.
#[test]
fn fe58_disagreement_is_explained_by_upstreams_unfac_fall_through() {
    let Some(l) = load(&FE58) else { return };
    let mine = read_urr_table(&l.rows, 0.0).expect("round-trips");
    let theirs = read_urr_from_tape(Some(&l.njoy_tape), FE58.mat, 1, 0.0)
        .expect("parses")
        .expect("present");
    let range = &l.ranges[0];

    // The defect needs an L >= 3 state to bite; assert Fe-58 supplies one, so
    // this test cannot pass vacuously on an evaluation that stops at l = 2.
    let UnresolvedCase::CaseA { l_states, .. } = &range.case_ else {
        panic!("Fe-58's unresolved range should parse as Case A");
    };
    let max_l = l_states.iter().map(|s| s.l).max().unwrap_or(0);
    assert!(
        max_l >= 3,
        "Fe-58 should carry an L >= 3 unresolved state (found max L = {max_l}); \
         without one, unfac's missing branch never fires and this test proves \
         nothing"
    );

    let mf3_at = |e: f64| -> f64 {
        for w in l.mf3_mt1.windows(2) {
            if e >= w[0].0 && e <= w[1].0 {
                let f = if w[1].0 > w[0].0 {
                    (e - w[0].0) / (w[1].0 - w[0].0)
                } else {
                    0.0
                };
                return w[0].1 + f * (w[1].1 - w[0].1);
            }
        }
        f64::NAN
    };

    println!(
        "{:>13} {:>13} {:>13} {:>11} {:>13} {:>10}",
        "E (eV)", "ours", "NJOY", "NJOY/ours", "reproduced", "repro dev"
    );
    let (mut worst_repro, mut min_ratio) = (0.0_f64, f64::INFINITY);
    for (a, b) in mine.points().iter().zip(theirs.points()) {
        let (ours, njoy) = (a.xs[0][0], b.xs[0][0]);
        let repro = reproduce_csunr1_with_upstream_fall_through(range, a.energy_ev);
        let dev = (repro - njoy).abs() / njoy;
        let ratio = njoy / ours;
        worst_repro = worst_repro.max(dev);
        min_ratio = min_ratio.min(ratio);
        println!(
            "{:13.6e} {:13.6e} {:13.6e} {:11.1} {:13.6e} {:10.2e}",
            a.energy_ev, ours, njoy, ratio, repro, dev
        );

        // Ours must stay near the evaluation's own MF=3, which for LSSF=1 IS
        // the infinitely-dilute cross section. Loose on purpose: MT=1 picks up
        // inelastic above ~0.8 MeV, which the URR formalism does not carry, so
        // this is an order-of-magnitude sanity check and nothing more.
        let mf3 = mf3_at(a.energy_ev);
        assert!(
            mf3.is_finite() && ours > 0.5 * mf3 && ours < 2.0 * mf3,
            "at E={:.6e} our stored total {ours:.6e} b is not within a factor 2 \
             of the evaluation's own MF=3 MT=1 {mf3:.6e} b",
            a.energy_ev
        );
    }

    assert!(
        worst_repro < 1.0e-3,
        "reproducing csunr1 with unfac's fall-through should land on NJOY's \
         published values; worst deviation {worst_repro:.2e}. If this fails, \
         the diagnosis in this file's header is wrong and must be revisited \
         before anything is concluded about Fe-58."
    );
    assert!(
        min_ratio > 100.0,
        "NJOY/ours should be at least 100x everywhere (measured 3113x rising to \
         139210x); the smallest seen was {min_ratio:.1}x. If this shrinks, \
         either the port has started reproducing the defect or the oracle changed."
    );
    println!(
        "  reproduced NJOY's Fe-58 totals to {worst_repro:.2e} using the \
         fall-through; NJOY exceeds ours by {min_ratio:.0}x at minimum"
    );
}

/// `csunr1` (`reconr.f90:3826-4077`) for Case A, reproduced **including its
/// defect**, to prove the Fe-58 diagnosis rather than assert it.
///
/// Differs from production in exactly one way, and deliberately: `unfac`
/// (`:4473-4496`) has no `l >= 3` branch, so `vl` and `ps` are carried over
/// from the previous call, and `vl = vl*e2` inside the J loop (`:4010`)
/// multiplies `vl` by another `sqrt(E)` per J-state. **Never call this from
/// anything but this test.**
///
/// Returns the stored total (elastic + fission + capture) in barns.
fn reproduce_csunr1_with_upstream_fall_through(range: &UnresolvedRange, e: f64) -> f64 {
    // Porter-Thomas quadrature for mu = 1 (`gnrl`, reconr.f90:4508-4530).
    const QW: [f64; 10] = [
        1.1120413e-1,
        2.3546798e-1,
        2.8440987e-1,
        2.2419127e-1,
        0.10967668,
        0.030493789,
        0.0042930874,
        2.5827047e-4,
        4.9031965e-6,
        1.4079206e-8,
    ];
    const QP: [f64; 10] = [
        3.0013465e-3,
        7.8592886e-2,
        4.3282415e-1,
        1.3345267,
        3.0481846,
        5.8263198,
        9.9452656,
        15.782128,
        23.996824,
        36.216208,
    ];

    let UnresolvedCase::CaseA {
        awri,
        ap,
        spi,
        l_states,
    } = &range.case_
    else {
        panic!("this reproduction covers Case A only");
    };
    let (awri, ap, spi) = (*awri, *ap, *spi);
    let rat = awri / (awri + 1.0);
    let aw = awri * AMASSN_AMU;
    // `naps` handling, `csunr1:3950-3958`.
    let aa = match range.naps {
        0 => 0.123 * aw.cbrt() + 0.08,
        _ => ap,
    };
    let konst = (2.0 * PI * PI) / (WAVE_K * rat).powi(2);

    let (mut elastic, mut capture) = (0.0_f64, 0.0_f64);
    // `vl`/`ps` are Fortran locals of `csunr1`, so they persist across l and j.
    // That persistence IS the defect being reproduced.
    let (mut vl, mut ps) = (0.0_f64, 0.0_f64);
    for state in l_states {
        let ll = state.l;
        let mut spot = 0.0;
        for (jdx, j) in state.j_states.iter().enumerate() {
            let gj = (2.0 * j.aj + 1.0) / (4.0 * spi + 2.0);
            let e2 = e.sqrt();
            let k = rat * e2 * WAVE_K;
            let (rho, rho_c) = (k * aa, k * ap);
            assert!(
                (j.amun - 1.0).abs() < 1e-9,
                "this reproduction hard-codes the mu = 1 quadrature; AMUN = {}",
                j.amun
            );
            // `call unfac(ll,rho,rhoc,amun,vl,ps)` -- for ll >= 3 it assigns
            // NEITHER output, so both keep their previous values.
            if ll <= 2 {
                let (v, p) = penetrability_factor(ll, rho, rho_c);
                // `unfac` folds AMUN into vl (`vl=amun*...`); ours does not.
                vl = v * j.amun;
                ps = p;
            }
            vl *= e2; // `:4010`, inside the j loop.
            if jdx == 0 {
                spot = 4.0 * PI * (2.0 * ll as f64 + 1.0) * (ps.sin() / k).powi(2);
            }
            let gnx = j.gno * vl;
            let den = e * j.d;
            let temp = konst * gj * gnx / den;
            let (mut s, mut c) = (0.0, 0.0);
            for i in 0..10 {
                s += QW[i] * QP[i] * QP[i] / (gnx * QP[i] + j.gg);
                c += QW[i] * QP[i] / (gnx * QP[i] + j.gg);
            }
            let add = konst * gj * 2.0 * gnx * ps.sin().powi(2) / den;
            elastic += s * temp * gnx - add;
            capture += c * temp * j.gg;
        }
        elastic += spot;
    }
    elastic + capture
}

/// The `unfac` defect reaches a **shipping** library, not just a pre-release.
///
/// # Why this test exists
///
/// The natural first objection to the Fe-58 evidence is that our copy is
/// `ENDF/B-VIII.0 Beta4` — a pre-release — so whatever is wrong with it was
/// presumably fixed before distribution. That objection is wrong, and this
/// gate is what makes it checkable instead of arguable.
///
/// The released ENDF/B-VIII.0 Fe-58 (`----ENDF/B-VIII.0 MATERIAL 2637`,
/// `DIST-FEB18`, eval Oct-2016, NDS **148**, 214 (2018)) carries an unresolved
/// parameter block **character-for-character identical** to Beta4's: the same
/// `3.5e5 .. 3.0e6 eV` range, `LRU=2, LRF=1, LFW=0, LSSF=1`, and the same
/// `NLS = 4` with the same widths on all four L-states. The two therefore
/// produce byte-identical MF=2/MT=152 through NJOY, superluminal values and
/// all.
///
/// # Methodology
///
/// Compare the MF=2/MT=151 records of the two evaluations from the start of
/// the `LRU=2` range header onward, field by field as decoded numbers. Then
/// assert both reach `L = 3`, since that is the precondition for the defect.
///
/// # Results (2026-09-15)
///
/// All 13 records of the unresolved block match exactly. `NLS = 4` on both.
/// NJOY's stored total is `1.422225e4 b` at 350 keV and `6.620656e5 b` at
/// 3 MeV on **both** tapes — identical to seven figures.
///
/// # What this does and does not establish
///
/// It establishes that one shipping evaluation in ENDF/B-VIII.0 trips the
/// defect. It says **nothing** about how many others do; that survey is open
/// (`gh:#211`). It also does not make the defect dangerous here: Fe-58 is
/// `LSSF = 1`, so `genunr` returns at `reconr.f90:1694` and the bad table
/// never enters MF=3. The combination that would reach a transport
/// calculation is `NLS > 3` **and** `LSSF = 0`, which no evaluation held here
/// exhibits.
#[test]
fn released_and_beta_fe58_carry_the_same_unresolved_parameters() {
    let (Some(beta), Some(rel)) = (
        reference_file("endf", FE58.evaluation),
        reference_file("endf", FE58_RELEASED.evaluation),
    ) else {
        eprintln!("skipping: one of the two Fe-58 evaluations is absent");
        return;
    };

    let load = |p: &std::path::Path| -> Vec<[f64; 6]> {
        let t = Tape::read_file(p).expect("evaluation parses");
        let sec = t.section(FE58.mat, 2, 151).expect("MF=2/151");
        // Take everything from the LRU=2 range header onward. That header is
        // the row whose L1 is 2 with a legal LRF in L2.
        let start = sec
            .rows
            .iter()
            .position(|r| r[2] as i32 == 2 && matches!(r[3] as i32, 1 | 2))
            .expect("an LRU=2 range header");
        sec.rows[start..].to_vec()
    };

    let (b, r) = (load(&beta), load(&rel));
    assert!(
        !b.is_empty(),
        "no LRU=2 range found in the Beta4 tape -- the premise of this test is gone"
    );
    assert_eq!(
        b.len(),
        r.len(),
        "the two Fe-58 evaluations have different record counts from the \
         unresolved range onward: Beta4 {} vs released {}",
        b.len(),
        r.len()
    );
    for (i, (x, y)) in b.iter().zip(&r).enumerate() {
        assert_eq!(
            x, y,
            "unresolved record {i} differs between Beta4 and released VIII.0:\n  \
             beta     {x:?}\n  released {y:?}\n\
             If this ever fires, the released evaluation HAS changed its \
             unresolved parameters and the 'shipping library is affected' \
             claim in gh:#210 must be re-measured, not assumed."
        );
    }

    // And the precondition for the defect: L must reach 3 on both.
    for (label, p) in [("Beta4", &beta), ("released", &rel)] {
        let t = Tape::read_file(p).expect("parses");
        let ranges = parse_lru2_ranges(&t.section(FE58.mat, 2, 151).expect("MF=2/151").rows[1..])
            .expect("LRU=2 ranges");
        let UnresolvedCase::CaseA { l_states, .. } = &ranges[0].case_ else {
            panic!("{label} Fe-58 should parse as Case A");
        };
        let max_l = l_states.iter().map(|s| s.l).max().unwrap_or(0);
        assert_eq!(
            max_l, 3,
            "{label} Fe-58 should reach L=3 (NLS=4); found max L = {max_l}. \
             Without an L>=3 state unfac's missing branch never fires."
        );
    }

    println!(
        "  Beta4 and released ENDF/B-VIII.0 Fe-58 share {} identical unresolved \
         records, both NLS=4 -- the defect reaches a shipping library",
        b.len()
    );
}
