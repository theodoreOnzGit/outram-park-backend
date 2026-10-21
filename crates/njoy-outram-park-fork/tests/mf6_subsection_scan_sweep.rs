//! **V&V — the `skip6a` port, swept over every MF=6 section in the reference
//! library rather than the handful the continuum gate looks at.**
//!
//! # Why a sweep, and why this is the right oracle for `skip6a`
//!
//! `parse_mf6_law1_neutrons` used to stop at the first subsection that was not a
//! `ZAP=1 LAW=1` neutron, because it could not step over another law's body.
//! Porting NJOY's `skip6a` (acefc.f90:7949+) lets it scan the whole section — and
//! a subsection skip is the kind of code whose failure mode is *silent*: a wrong
//! stride leaves the cursor mid-record, and the next `TAB1` header read is then
//! whatever numbers happen to sit there. That does not throw; it produces a
//! plausible-looking subsection with a nonsense `ZAP`.
//!
//! So the oracle is **structural, and applied to every MF=6 section on every
//! reference tape**: a correctly-skipped scan yields only physical results — a
//! `ZAP` that is a real particle, a multiplicity in a physical range, ascending
//! `E'` grids, and normalised distributions — while a misaligned one produces
//! garbage almost immediately, across hundreds of sections.
//!
//! # A defect this sweep found immediately
//!
//! The first version of the `skip6` port was a port of **`skip6a`** instead —
//! and NJOY's own header on that routine says why that is wrong: *"Special
//! version of skip6 for special version of File 6 used in ACER. Law=7 has a TAB1
//! containing the angular distribution instead of the normal TAB2 for each
//! incident energy."* `skip6a` walks ACER's internal File-6 variant; a raw ENDF
//! tape needs `skip6` (endf.f90:1437-1476), whose LAW=7 per-energy record is a
//! **TAB2**. ENDF/B-VIII.0's Be-9 MF=6/MT=16 is LAW=7, so the wrong stride
//! desynchronised the cursor there on the first run of this sweep.
//!
//! It also found **`LCT = 3`** on C-12's MF=6/MT=5. Three is a legal reference
//! frame (first two particles in the centre of mass, the rest in the
//! laboratory), and both this crate's `cm_frame` flag and its ACE TYR sign
//! tested `lct == 2`, so a CM spectrum would have been used unrotated. NJOY
//! collapses it at `acefc.f90:7187` — `if (lct.gt.2) lct=2` — and signs on
//! `lct.ge.2` at 5869, with MT=18 forced back to the laboratory at 7277. Both
//! sites now follow that.
//!
//! Neither defect throws on the tapes the narrower continuum gate reads; both
//! needed breadth to surface.
//!
//! # What the library actually contains (measured 2026-09-13)
//!
//! A prior survey of `reference-data/endf/` found:
//!
//! ```text
//!   MF=5 with NK > 1                                    0 sections
//!   MF=6 with a ZAP=1 subsection whose LAW != 1        110 sections
//!   MF=6 LAW=1 with discrete lines (ND > 0)              0 tables
//!   MF=6 with a ZAP=1 subsection that is NOT leading     0 sections
//! ```
//!
//! Two consequences worth stating plainly rather than leaving implicit:
//!
//! - The `LAW != 1` neutron path is **common**, not exotic — Si-29's MT=51…57
//!   are `LAW=2` (two-body), U-238's MT=18 is `LAW=0`. This sweep therefore
//!   exercises the skip against real records in bulk.
//! - The `NK > 1`, `ND > 0` and non-leading-neutron ports are **not exercised by
//!   any tape here**. They are ported because the format allows them, and this
//!   test cannot claim otherwise; a synthetic fixture would only test the fixture.
//!   That gap is recorded rather than papered over.

use njoy_outram_park_fork::acer::energy::parse_mf6_law1_neutrons;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::NjoyError;

/// Every reference tape this sweep reads. Absent files are skipped, so a
/// crates.io consumer with no `reference-data/` tree sees an empty pass.
const TAPES: &[&str] = &[
    "n-092_U_238.endf",
    "n-092_U_235-ENDF8.0.endf",
    "n-092_U_234-ENDF8.0.endf",
    "n-009_F_019-ENDF8.0.endf",
    "n-006_C_012-ENDF8.0.endf",
    "n-008_O_016-ENDF8.0.endf",
    "n-014_Si_028-ENDF8.0.endf",
    "n-014_Si_029-ENDF8.0.endf",
    "n-014_Si_030-ENDF8.0.endf",
    "n-013_Al_027-ENDF8.0.endf",
    "n-005_B_010-ENDF8.0.endf",
    "n-004_Be_009-ENDF8.0.endf",
];

#[test]
fn mf6_subsection_scan_stays_aligned_across_the_whole_reference_library() {
    let mut sections_scanned = 0usize;
    let mut neutron_sections = 0usize;
    let mut branches_total = 0usize;
    let mut tapes_read = 0usize;

    for file in TAPES {
        let Some(path) = njoy_outram_park_fork::reference_data::reference_endf(file) else {
            continue;
        };
        let Ok(tape) = Tape::read_file(&path) else {
            continue;
        };
        tapes_read += 1;
        for mat in tape.materials() {
            for mt in 1..=200i32 {
                let Some(sec) = tape.section(mat, 6, mt) else {
                    continue;
                };
                sections_scanned += 1;
                match parse_mf6_law1_neutrons(sec) {
                    // An honest refusal is a correct outcome — LAW=0/2 neutron
                    // subsections are exactly what upstream `acelf6` rejects too.
                    Err(NjoyError::NotPorted(_)) => {}
                    Err(e) => panic!("{file} MF=6/MT={mt}: unexpected error {e:?}"),
                    Ok(neutrons) => {
                        neutron_sections += 1;
                        assert!(
                            !neutrons.is_empty(),
                            "{file} MF=6/MT={mt}: Ok with no branches"
                        );
                        branches_total += neutrons.len();
                        assert!(
                            neutrons.len() <= 8,
                            "{file} MF=6/MT={mt}: {} neutron subsections — a \
                             misaligned skip invents subsections",
                            neutrons.len()
                        );
                        for (bi, n) in neutrons.iter().enumerate() {
                            assert!(
                                (1..=3).contains(&n.lct),
                                "{file} MF=6/MT={mt} branch {bi}: LCT = {} is not 1, 2 or 3 \
                                 reference frame — the cursor is misaligned",
                                n.lct
                            );
                            for &(e, y) in &n.yield_pairs {
                                assert!(
                                    (0.0..=1.0e9).contains(&e) && (0.0..=20.0).contains(&y),
                                    "{file} MF=6/MT={mt} branch {bi}: yield point \
                                     ({e:e} eV, {y}) is not physical"
                                );
                            }
                            assert!(
                                !n.law4.incident.is_empty(),
                                "{file} MF=6/MT={mt} branch {bi}: no incident tables"
                            );
                            for t in &n.law4.incident {
                                assert!(
                                    t.e_in_mev > 0.0 && t.e_in_mev <= 1.0e3,
                                    "{file} MF=6/MT={mt} branch {bi}: incident energy \
                                     {} MeV is out of range",
                                    t.e_in_mev
                                );
                                assert!(
                                    t.e_out_mev.len() == t.pdf.len() && t.pdf.len() == t.cdf.len(),
                                    "{file} MF=6/MT={mt} branch {bi}: ragged table"
                                );
                                assert!(
                                    t.e_out_mev.windows(2).all(|w| w[1] > w[0]),
                                    "{file} MF=6/MT={mt} branch {bi} @ {} MeV: E' must \
                                     ascend — the classic signature of a wrong stride",
                                    t.e_in_mev
                                );
                                assert!(
                                    t.pdf.iter().all(|&p| p >= 0.0 && p.is_finite()),
                                    "{file} MF=6/MT={mt} branch {bi}: negative or \
                                     non-finite pdf"
                                );
                                let last = t.cdf[t.cdf.len() - 1];
                                assert!(
                                    (last - 1.0).abs() < 1.0e-6,
                                    "{file} MF=6/MT={mt} branch {bi} @ {} MeV: cdf ends \
                                     at {last}, want 1",
                                    t.e_in_mev
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    if tapes_read == 0 {
        return; // no reference data in this checkout
    }
    // Measured 2026-09-13: 12 tapes, 139 MF=6 sections, 50 of them with a
    // readable LAW=1 neutron emission, 51 branches — the extra branch is F-19's
    // second (n,2n) neutron subsection. The counts are asserted as a floor, not
    // an equality, so adding a tape does not fail the test — but a scan that
    // silently stopped early would fall below them.
    assert!(
        sections_scanned >= 100,
        "only {sections_scanned} MF=6 sections scanned across {tapes_read} tapes"
    );
    assert!(
        neutron_sections >= 40 && branches_total >= neutron_sections,
        "{neutron_sections} sections with a readable neutron emission, \
         {branches_total} branches — expected at least 40"
    );
    println!(
        "  swept {tapes_read} tapes, {sections_scanned} MF=6 sections, \
         {neutron_sections} with a readable neutron emission, {branches_total} branches"
    );
}
