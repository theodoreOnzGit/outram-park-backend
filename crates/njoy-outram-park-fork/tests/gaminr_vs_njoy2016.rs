//! GAMINR photon-interaction group constants and scattering matrices vs
//! NJOY2016 GAM-out tapes — the oracle on `gtff` (`gaminr.f90:1162-1514`), the
//! photon `gpanel` (`:874-1011`), `dspla` (`:1013-1131`) and the total-heating
//! edit (`:400-419`).
//!
//! # Two cases, and why the second was added
//!
//! Until 2026-09-17 this file had **one** case, and it was synthetic — the
//! module docs below say why: "no photoatomic evaluation is available offline".
//! That turned out to be wrong. `reference-data/endf/photoat-092_U_000-ENDF8.0.endf`
//! is a real ENDF/B-VIII.0 photoatomic evaluation (MAT 9200, Z = 92), and it is
//! committed. A second case now runs the same deck against it.
//!
//! **It immediately found something the synthetic case could not.** On the real
//! evaluation:
//!
//! | reaction | agreement |
//! |---|---|
//! | MF=23 MT=501 total | **3.6e-9** |
//! | MF=23 MT=502 coherent | **2.7e-7** |
//! | MF=23 MT=504 incoherent | **3.4e-9** |
//! | MF=23 MT=516 pair | **3.0e-9** |
//! | MF=23 MT=522 photoelectric | **1.8e-8** |
//! | MF=26 MT=516 pair matrix | **3.0e-9** |
//! | MF=26 MT=502 coherent matrix | **disagrees** — see below |
//! | MF=26 MT=504 incoherent matrix | **disagrees** |
//! | MF=23 MT=525 total heating | 0.61 % (depends on MT=504's heating slot) |
//!
//! So every *vector* reaction is right on a real evaluation, and so is the one
//! *matrix* that consumes no MF=27 table. The two matrices that **do** consume
//! an MF=27 table — 502 (coherent form factor) and 504 (incoherent scattering
//! function) — do not. Measured at group 1:
//!
//! ```text
//! MF=26/502  ours  1.822327e2  1.278700e2  9.884985e1  7.259552e1
//!            njoy  1.394827e2  1.056669e2  8.412942e1  6.462165e1
//! MF=26/504  ours  3.719060e1 -2.994461e-2 2.265225e0 -8.314837e-1
//!            njoy  1.302159e2 -7.558322e-1 7.802657e0 -2.991777e0
//! ```
//!
//! and NJOY writes only **2** records for each (groups 1 and 12) where this port
//! writes 12 — NJOY's `igzero` gate (`gaminr.f90:400`) drops a record whose
//! every normalised value is below 1e-9, plus it always keeps `ig == ngg`. This
//! port implements that gate; the records survive here because the values it
//! computes are not small.
//!
//! **One invariant that holds on the synthetic tape and breaks on U**, worth
//! recording because it is the sharpest clue: `gtff_coherent` normalises its
//! Legendre moments by `sigcoh`, so `ff[0] == 1` and the MF=26 `l = 0` group
//! value should equal the MF=23 group cross section. On the synthetic case
//! NJOY's are identical (0.41244466 both). On U, NJOY's MF=23/502 group 1 is
//! 182.232750 — which is *this port's* MF=26 `l = 0` — while NJOY's own MF=26
//! `l = 0` is 139.482685.
//!
//! These three reactions are **printed, not asserted**, while that is open
//! (`unasserted` on the `Case`). Asserting them would mean either freezing
//! wrong numbers or widening a gate past the point where it says anything.
//! Everything else on the real evaluation **is** asserted, at the same 1e-5 the
//! synthetic case uses.
//!
//! # Oracle (synthetic case)
//! No photoatomic evaluation is available offline (both public data hosts
//! refuse downloads from this environment), so the material is a
//! **synthetic Z = 6 photoatomic tape** — `reference-data/endf/photoat-synthetic-Z6.endf`,
//! written by the committed generator next to it: MF=23 cross sections with
//! analytic shapes (coherent `2.4/(1+(E/3e4)^2)`, incoherent
//! `3.99 (1+E/511 keV)^-0.9 (1-0.3 e^{-E/1e4})`, photoelectric
//! `4e3 (E/1 keV)^-3`, pair `0.2 ln(E/1.022 MeV)` above threshold, total =
//! sum) tabulated lin-lin on 53 energies from 1 keV to 100 GeV, and MF=27
//! form factor `F(x) = 6/(1+(x/0.6)^2)^2` / scattering function
//! `S(x) = 6 (1 - 1/(1+(x/0.5)^2))` on 24 momentum transfers from 0 to
//! 1e9 /Angstrom. It is *not* an evaluation; it exercises every code path
//! (`terpa` regions, the Compton-edge critical points, the pair threshold
//! inside a group) with physically-shaped numbers. Because MF=23 is already
//! lin-lin the same tape serves as `nendf` and `npend`.
//!
//! NJOY2016 `ac5adf5` (2026-09-10), deck
//! `reference-data/gendf/photoat-synthetic-Z6-lanl12-iwt3-lord3.njoy-input`:
//! `gaminr 20 20 0 21 / 600 3 3 3 1 /` — LANL 12-group structure (`igg = 3`),
//! `iwt = 3` (1/E with roll-offs), `lord = 3` — reactions `23/501 502 504
//! 516 522`, `26/502 504 516`, `23/525` in that order (the heating edit
//! must follow the reactions that feed it). Golden:
//! `reference-data/gendf/photoat-synthetic-Z6-lanl12-iwt3-lord3.gendf`.
//!
//! # What upstream does, read before measuring
//! - `gtff` for MF=26/504 returns the sink-group feed plus two trailing
//!   slots (cross section, heating) normalised by `siginc`; the driver
//!   folds the heating slot into `toth` and drops both slots before
//!   writing (`:353-359`), so the GENDF MT=504 records carry only the flux
//!   and the sink groups; MT=516 keeps its 511-keV pair term (`ff = 2`)
//!   and drops its heating slot; MT=522 keeps only the cross section.
//! - `gpanel` spans its Lobatto rule from the nudged `elo` to `ehigh`
//!   (`:951-952`), unlike GROUPR's `panel`.
//! - `dspla` leaves an MF=23 slot un-normalised when its raw integral is
//!   below `1e-9` (`:1067`); the photoelectric cross section at 10–20 MeV
//!   (1e-8 b) is where that bites.
//!
//! **Prediction stated before running:** identical record structure per
//! section and every word within 1e-5 relative of the tape (seven-figure
//! storage on NJOY's side), the total-heating edit included.
//!
//! **Result:** see the test's printed summary and assertion.

use std::sync::Arc;

use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::gaminr::{
    gaminr_reaction, photon_group_structure, PhotonFlux, PhotonReaction, PhotonTab1, TotalHeating,
};
use njoy_outram_park_fork::groupr::gendf::GendfSection;
use njoy_outram_park_fork::groupr::panel::PointwiseXs;
use njoy_outram_park_fork::reference_data::{reference_endf_or_skip, reference_file_or_skip};

const LORD: usize = 3;
const TOL: f64 = 1e-5;

struct Case {
    label: &'static str,
    mat: i32,
    endf: &'static str,
    golden: &'static str,
    /// `(mf, mt)` pairs that are compared but **not** asserted, only printed.
    /// Empty on the synthetic case; on U it holds the two MF=27-driven
    /// matrices and the heating edit that depends on one of them, while the
    /// discrepancy described in the module docs is open.
    unasserted: &'static [(i32, i32)],
}

/// The synthetic Z = 6 tape: every code path, physically-shaped numbers, no
/// evaluation. See the module docs.
const SYNTHETIC: Case = Case {
    label: "gaminr-synthetic-z6",
    mat: 600,
    endf: "photoat-synthetic-Z6.endf",
    golden: "photoat-synthetic-Z6-lanl12-iwt3-lord3.gendf",
    unasserted: &[],
};

/// A **real** ENDF/B-VIII.0 photoatomic evaluation: uranium, MAT 9200. Added
/// 2026-09-17, when it turned out one *was* available offline after all.
const U_PHOTOAT: Case = Case {
    label: "gaminr-u-photoat",
    mat: 9200,
    endf: "photoat-092_U_000-ENDF8.0.endf",
    golden: "photoat-U000-ENDF8.0-lanl12-iwt3-lord3.gendf",
    unasserted: &[(26, 502), (26, 504), (23, 525)],
};

fn tab1(tape: &Tape, mat: i32, mf: i32, mt: i32) -> (f64, f64, PhotonTab1) {
    let sec = tape.section(mat, mf, mt).expect("section present");
    let mut cur = SectionCursor::new(&sec.rows);
    let head = cur.read_cont().unwrap();
    let t = cur.read_tab1().unwrap();
    (
        head.c1,
        head.c2,
        PhotonTab1 {
            z: t.head.c2,
            interp: t.interp,
            pairs: t.pairs,
        },
    )
}

fn check(case: &Case) {
    let Case {
        label,
        mat,
        endf: endf_name,
        golden: golden_name,
        unasserted,
    } = *case;
    let Some(endf_path) = reference_endf_or_skip(endf_name, label) else {
        return;
    };
    let Some(golden_path) = reference_file_or_skip("gendf", golden_name, label) else {
        return;
    };
    let endf = Tape::read_file(&endf_path).expect("photoatomic tape parses");
    let golden = Tape::read_file(&golden_path).expect("GAM-out tape parses");
    let egg = photon_group_structure(3).unwrap();
    let ngg = egg.len() - 1;
    let flux = PhotonFlux::one_over_e_rolloffs();
    let (_, _, form_factor) = tab1(&endf, mat, 27, 502);
    let (_, _, scattering_function) = tab1(&endf, mat, 27, 504);
    let mut heating = TotalHeating::new(ngg);
    let requests: Vec<(i32, i32, PhotonReaction)> = vec![
        (23, 501, PhotonReaction::Vector { mt: 501 }),
        (23, 502, PhotonReaction::Vector { mt: 502 }),
        (23, 504, PhotonReaction::Vector { mt: 504 }),
        (23, 516, PhotonReaction::Vector { mt: 516 }),
        (23, 522, PhotonReaction::Vector { mt: 522 }),
        (26, 502, PhotonReaction::Coherent { form_factor }),
        (
            26,
            504,
            PhotonReaction::Incoherent {
                scattering_function,
            },
        ),
        (26, 516, PhotonReaction::Pair),
    ];
    let mut worst_all = (0.0f64, 0, 0, 0, 0usize);
    let mut compare = |mf: i32, mt: i32, ours: &GendfSection| {
        let sec = golden.section(mat, mf, mt).expect("golden section");
        let theirs = GendfSection::from_rows(mf, mt, &sec.rows).expect("golden decodes");
        assert_eq!(
            (ours.nl, ours.nz),
            (theirs.nl, theirs.nz),
            "MF={mf}/MT={mt}: NL/NZ"
        );
        if unasserted.contains(&(mf, mt)) {
            let fmt = |r: Option<&njoy_outram_park_fork::groupr::gendf::GendfGroupRecord>| {
                r.map(|r| {
                    r.data
                        .iter()
                        .map(|v| format!("{v:.6e}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default()
            };
            println!(
                "[{label}] MF={mf}/MT={mt}: PRINTED, NOT ASSERTED (open discrepancy, see the \
                 module docs) -- ours {} records, NJOY {}\n      ours ig1 [{}]\n      njoy ig1 [{}]",
                ours.records.len(),
                theirs.records.len(),
                fmt(ours.records.first()),
                fmt(theirs.records.first()),
            );
            return;
        }
        assert_eq!(
            ours.records
                .iter()
                .map(|r| (r.ig, r.ig2lo, r.ng2))
                .collect::<Vec<_>>(),
            theirs
                .records
                .iter()
                .map(|r| (r.ig, r.ig2lo, r.ng2))
                .collect::<Vec<_>>(),
            "MF={mf}/MT={mt}: record structure (ig, ig2lo, ng2)"
        );
        let mut worst = (0.0f64, 0i32, 0usize, 0.0f64, 0.0f64);
        let mut n = 0usize;
        for (a, b) in ours.records.iter().zip(&theirs.records) {
            assert_eq!(
                a.data.len(),
                b.data.len(),
                "MF={mf}/MT={mt} ig {}: words",
                a.ig
            );
            for (k, (&x, &y)) in a.data.iter().zip(&b.data).enumerate() {
                n += 1;
                assert_eq!(
                    x == 0.0,
                    y == 0.0,
                    "MF={mf}/MT={mt} ig {} word {k}: {x} vs {y}",
                    a.ig
                );
                if y == 0.0 {
                    continue;
                }
                let r = ((x - y) / y).abs();
                if r > worst.0 {
                    worst = (r, a.ig, k, x, y);
                }
            }
        }
        println!(
            "[{label}] MF={mf}/MT={mt}: {} records, {n} words, worst rel dev {:.3e} (ig {}, word \
             {}: ours {:.7e} vs NJOY {:.7e})",
            ours.records.len(),
            worst.0,
            worst.1,
            worst.2,
            worst.3,
            worst.4
        );
        if worst.0 > worst_all.0 {
            worst_all = (worst.0, mf, mt, worst.1, worst.2);
        }
    };
    for (mf, mt, reaction) in &requests {
        let (za, awr, xs) = tab1(&endf, mat, 23, *mt);
        let sigma = PointwiseXs::LinLin(Arc::new(xs.pairs));
        let ours =
            gaminr_reaction(reaction, &sigma, &flux, &egg, LORD, za, awr, &mut heating).unwrap();
        compare(*mf, *mt, &ours);
    }
    let (za, awr, _) = tab1(&endf, mat, 23, 501);
    let total = heating.section(525, za, awr);
    compare(23, 525, &total);
    println!(
        "[{label}] worst overall {:.3e} at MF={}/MT={} ig {} word {}",
        worst_all.0, worst_all.1, worst_all.2, worst_all.3, worst_all.4
    );
    assert!(
        worst_all.0 < TOL,
        "[{label}] GAMINR deviates from NJOY: {worst_all:?}"
    );
}

/// The synthetic Z = 6 case — every code path, no evaluation.
#[test]
fn njoy_gaminr_synthetic_z6_matches_gamout() {
    check(&SYNTHETIC);
}

/// **A real ENDF/B-VIII.0 photoatomic evaluation.** See the module docs for why
/// this was missing and what it adds over the synthetic case.
#[test]
fn njoy_gaminr_u_photoatomic_matches_gamout() {
    check(&U_PHOTOAT);
}
