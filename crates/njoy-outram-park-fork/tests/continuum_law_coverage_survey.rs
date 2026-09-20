//! **Where does the Weisskopf evaporation stand-in still get used?**
//!
//! # Why this exists
//!
//! `outram-mc-libs` samples continuum inelastic and multiplying reactions from
//! the evaluation's own `f0(E->E')` where `ContinuumEmission::from_endf_mf6`
//! produces one, and from a **Weisskopf evaporation stand-in** where it does
//! not. The stand-in is a physically reasonable shape with no connection to the
//! evaluation, so every place it fires is a silent approximation.
//!
//! Three such places were found and closed on 2026-09-16 — H-2 MT=16 (MF=6
//! LAW=6), Be-9 MT=16 (MF=6 LAW=7), and the dropped `LANG` coefficients
//! (`op-og56`) — and each was found by accident rather than by looking. This
//! test looks: it enumerates every `(tape, MT)` in `reference-data/endf/` that a
//! transport run would reach, and reports which get an evaluated law and which
//! get the stand-in.
//!
//! # Methodology
//!
//! For each neutron tape and each of MT = 16 ((n,2n)), 17 ((n,3n)), 91
//! (continuum inelastic), classify into one of:
//!
//! - **evaluated** — `from_endf_mf6` returns a law
//! - **MF=4/5 instead** — no MF=6, but the evaluation supplies the outgoing
//!   energy law as **MF=5** and the emission cosine as **MF=4**, the pre-ENDF-6
//!   way of saying the same thing. Read since 2026-09-16 via
//!   `UncorrelatedEmission::from_endf`, so these are served by the evaluation's
//!   own law, not by the stand-in. The count is pinned so a new tape in this
//!   category cannot slip in unnoticed.
//! - **nothing anywhere** — no MF=6, no MF=5. The stand-in is then the only
//!   option the evaluation leaves, which is a property of the evaluation rather
//!   than of this port.
//! - **MF=6 present but no law** — an MF=6 section exists and we produce nothing
//!   from it. **This is the port gap**, and it is what the assertion below
//!   forbids.
//!
//! Pass criteria: the **MF=6-present-but-no-law** category is empty, and every
//! section in the **MF=4/5** category actually produces an `UncorrelatedEmission`
//! — a category count alone would not notice the law failing to build. Both gaps
//! were closed on 2026-09-16 and neither may reopen.
//!
//! # Results
//!
//! | date | tapes | from MF=6 | from MF=4/5 | MF=6-but-no-law | no law anywhere |
//! |---|---|---|---|---|---|
//! | 2026-09-16 | 27 | 42 | 11 | 0 | 0 |
//! | **2026-09-20** | **57** | **98** | **20** | **0** | **0** |
//!
//! The 2026-09-20 row is the same survey over every incident-neutron tape in
//! `reference-data/endf/`, run alongside the 57-tape ACE parity sweep. The
//! MF=4/5 population grew by nine sections — C-nat MT=91, Na-23 MT=16/91 and
//! Mg-24/25/26 MT=16/91 — because those tapes were added after the first run,
//! not because anything regressed. **Both pass criteria still hold at 57
//! tapes: zero MF=6-but-no-law, and all 20 MF=4/5 sections build a law.**
//!
//! # The MF=4/5 gap, found by this survey
//!
//! (The table below was written for the original 11 and is left as it stood;
//! the nine added in 2026-09-20 are listed in the pin's own comment near the
//! assertion, with the same MT=16/91 shape.)
//!
//! The 11 were first counted as benign — "the evaluation carries no MF=6, so the
//! stand-in is all there is". That was wrong, and checking rather than assuming
//! is what showed it: **every one of the 11 carries both MF=4 and MF=5** for the
//! same MT.
//!
//! | tape | MTs | MF=4 `LTT` | MF=5 `LF` |
//! |---|---|---|---|
//! | Li-7 ENDF/B-VIII.0 | 16 | 2 (tabulated) | 1 (tabulated) |
//! | C-12 ENDF/B-VIII.0 | 91 | 0 (isotropic) | 9 (evaporation, `theta(E)`) |
//! | Sr-88 ENDF/B-VIII.1 | 16, 17, 91 | — | — |
//! | U-238 JENDL-3.3 | 16, 17, 91 | 2 / 1 | 1 |
//! | Pu-239 JENDL-3.3 | 16, 17, 91 | — | — |
//!
//! Both `LF` values found were already ported (`LF=1`, `LF=9`) and MF=4
//! linearisation already existed for the discrete levels (`op-tm9f`), so this
//! was a **wiring** gap, not a missing representation — the same shape as LAW=6
//! and LAW=7 before it. **Closed 2026-09-16** by `UncorrelatedEmission` +
//! `Nuclide::sample_inelastic_emission`.
//!
//! **The frame question, settled by measurement rather than argument.** ENDF-102
//! puts MF=5 secondary energies in the laboratory system while MF=4 carries its
//! own `LCT`, so a single frame flag looked unable to express the combination.
//! Reading upstream settled it: `acefc.f90:5825-5869` takes `lct` from MF=4's
//! own CONT record and sets the ACE `TY` sign from it — one flag per reaction,
//! from MF=4. And all **11** of these sections are `LCT = 1` (laboratory)
//! anyway, so the CM branch is unexercised; `from_endf` refuses it rather than
//! shipping untested frame-transform code.
//!
//! **Scope, stated honestly:** none of this workspace's criticality cases is
//! affected by the change. Godiva and the thermal cases run on ENDF/B-VIII.0 U-234/U-235/U-238
//! and Fe/Si/O, all of which carry MF=6 and land in the *evaluated* column. The
//! affected tapes are the two JENDL-3.3 evaluations (kept for cross-library
//! comparison), Sr-88 (the LRF=7 test case), and two light-nuclide thresholds.
//! So this changes no `k` that has been reported — but it is a silent
//! substitution and it is now counted rather than assumed away.
//!
//! # What this does NOT claim
//!
//! That the evaluated laws are *right* — only that where an evaluation supplies
//! one, this port reads it rather than substituting its own shape. Correctness
//! of each law is the business of the per-law tests
//! (`mf6_law6_phase_space.rs`, `mf6_law7_conversion.rs`,
//! `mf6_continuum_angular_vs_endf.rs`, and `outram-mc-libs`'s
//! `mt91_transfer_vs_openmc.rs`).

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::nuclear_data::secondary::ContinuumEmission;
use njoy_outram_park_fork::reference_data::reference_data_dir;

#[derive(Debug, PartialEq)]
enum Coverage {
    /// An MF=6 law was read and converted.
    Evaluated,
    /// No MF=6, but MF=5 (and usually MF=4) carry the same physics the older
    /// way. Served by `UncorrelatedEmission` since 2026-09-16.
    Mf45,
    /// No MF=6 and no MF=5: the evaluation supplies no secondary-energy law at
    /// all, so the stand-in is the only option. Not a port gap.
    NothingAnywhere,
    /// The port gap this test asserts against: MF=6 is present and we build
    /// nothing from it.
    Mf6ButNoLaw,
}

#[test]
fn every_mf6_continuum_section_yields_an_evaluated_law() {
    let dir = reference_data_dir("endf");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        eprintln!("SKIP: reference-data/endf/ not present");
        return;
    };
    let mut files: Vec<_> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|x| x == "endf")
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("n-"))
        })
        .collect();
    files.sort();
    if files.is_empty() {
        eprintln!("SKIP: no neutron tapes");
        return;
    }

    let (mut evaluated, mut nothing) = (0usize, 0usize);
    let mut mf45: Vec<String> = Vec::new();
    let mut gaps: Vec<String> = Vec::new();

    for f in &files {
        let Ok(tape) = Tape::read_file(f) else {
            continue;
        };
        let Some(&mat) = tape.materials().first() else {
            continue;
        };
        let name = f.file_name().unwrap().to_string_lossy().into_owned();
        let mut line = Vec::new();
        for mt in [16i32, 17, 91] {
            // Only reactions the evaluation actually has: no MF=3 means the
            // channel does not exist and nothing ever samples it.
            if tape.section(mat, 3, mt).is_none() {
                continue;
            }
            let has_mf6 = tape.section(mat, 6, mt).is_some();
            let has_mf5 = tape.section(mat, 5, mt).is_some();
            let law =
                ContinuumEmission::from_endf_mf6(&tape, mat, mt).expect("MF=6 read must not error");
            let cov = match (has_mf6, law.is_some(), has_mf5) {
                (_, true, _) => Coverage::Evaluated,
                (true, false, _) => Coverage::Mf6ButNoLaw,
                (false, false, true) => Coverage::Mf45,
                (false, false, false) => Coverage::NothingAnywhere,
            };
            match cov {
                Coverage::Evaluated => evaluated += 1,
                Coverage::NothingAnywhere => nothing += 1,
                Coverage::Mf45 => mf45.push(format!("{name} MT={mt}")),
                Coverage::Mf6ButNoLaw => gaps.push(format!("{name} MT={mt}")),
            }
            line.push(format!("MT={mt}:{cov:?}"));
        }
        if !line.is_empty() {
            println!("   {name}: {}", line.join("  "));
        }
    }

    println!(
        "continuum-law coverage over {} neutron tapes: {evaluated} from MF=6, {} from MF=4/5, \
         {nothing} with no law anywhere (stand-in is all the evaluation leaves), {} \
         MF=6-but-no-law",
        files.len(),
        mf45.len(),
        gaps.len()
    );
    if !mf45.is_empty() {
        println!("   MF=4/5 sections served by UncorrelatedEmission: {mf45:?}");
    }

    assert!(
        evaluated > 0,
        "no evaluated continuum law was built at all; the survey measured nothing."
    );
    // ORDER MATTERS, and it was wrong until 2026-09-20. The service check below
    // is the real gate; the inventory pin that follows it is a tripwire. With
    // the pin first, adding a tape made the pin fire and the gate never ran at
    // all -- so the run that was supposed to confirm "the new sections are
    // served" could not, and the only way to clear it was to bump a number on
    // faith. Gate first, then pin.
    //
    // A count is not enough on its own: every one of them must actually BUILD a
    // law. This is what would catch `UncorrelatedEmission::from_endf` starting
    // to return None -- the category count would be unchanged and the Weisskopf
    // stand-in would be quietly back.
    for entry in &mf45 {
        let (name, mt) = entry.split_once(" MT=").expect("survey label format");
        let mt: i32 = mt.parse().expect("MT parses");
        let path = dir.join(name);
        let tape = Tape::read_file(&path).expect("tape re-reads");
        let mat = tape.materials()[0];
        let law = njoy_outram_park_fork::nuclear_data::secondary::UncorrelatedEmission::from_endf(
            &tape, mat, mt,
        )
        .expect("MF=4/5 read does not error");
        assert!(
            law.is_some(),
            "{name} MT={mt} has MF=5 but builds no UncorrelatedEmission, so transport is back \
             on the Weisskopf stand-in over data that is present."
        );
    }

    // The MF=4/5 count is PINNED so a newly added tape in this category shows up
    // as a test to update rather than as silence. Every section counted here has
    // just been shown to build a law by the loop above, so updating this number
    // records a confirmed inventory rather than asserting one.
    //
    // 2026-09-16: 11, over 27 neutron tapes.
    // 2026-09-20: 20, over 57 neutron tapes -- C-nat MT=91, Na-23 MT=16/91 and
    //   Mg-24/25/26 MT=16/91 were added to `reference-data/endf/` after the
    //   first pin. All nine build a law. Found by the 57-tape ACE parity sweep,
    //   which independently identified this same MF=4/5 population as the
    //   dominant cause of this port's NXS(5) undercount against NJOY2016.
    const MF45_SECTIONS: usize = 20;
    assert_eq!(
        mf45.len(),
        MF45_SECTIONS,
        "the number of MF=4/5 sections changed from {MF45_SECTIONS} to {}. A tape was added or \
         removed; the loop above has already confirmed each one builds a law, so update this \
         constant. Sections: {mf45:?}",
        mf45.len()
    );

    assert!(
        gaps.is_empty(),
        "{} section(s) carry an MF=6 this port reads nothing from, so a transport run \
         substitutes a Weisskopf evaporation shape with nothing recording it. That is the \
         exact silent gap H-2 MT=16 (LAW=6) and Be-9 MT=16 (LAW=7) sat in until 2026-09-16. \
         Sections: {:?}",
        gaps.len(),
        gaps
    );
}
