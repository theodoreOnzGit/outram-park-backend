//! GAMINR photon-interaction group constants and scattering matrices vs an
//! NJOY2016 GAM-out tape — the first oracle on `gtff` (`gaminr.f90:1162-1514`),
//! the photon `gpanel` (`:874-1011`), `dspla` (`:1013-1131`) and the
//! total-heating edit (`:400-419`).
//!
//! # Oracle
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

const MAT: i32 = 600;
const LABEL: &str = "gaminr-synthetic-z6";
const ENDF: &str = "photoat-synthetic-Z6.endf";
const GOLDEN: &str = "photoat-synthetic-Z6-lanl12-iwt3-lord3.gendf";
const LORD: usize = 3;
const TOL: f64 = 1e-5;

fn tab1(tape: &Tape, mf: i32, mt: i32) -> (f64, f64, PhotonTab1) {
    let sec = tape.section(MAT, mf, mt).expect("section present");
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

#[test]
fn njoy_gaminr_synthetic_z6_matches_gamout() {
    let Some(endf_path) = reference_endf_or_skip(ENDF, LABEL) else {
        return;
    };
    let Some(golden_path) = reference_file_or_skip("gendf", GOLDEN, LABEL) else {
        return;
    };
    let endf = Tape::read_file(&endf_path).expect("synthetic tape parses");
    let golden = Tape::read_file(&golden_path).expect("GAM-out tape parses");
    let egg = photon_group_structure(3).unwrap();
    let ngg = egg.len() - 1;
    let flux = PhotonFlux::one_over_e_rolloffs();
    let (_, _, form_factor) = tab1(&endf, 27, 502);
    let (_, _, scattering_function) = tab1(&endf, 27, 504);
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
        let sec = golden.section(MAT, mf, mt).expect("golden section");
        let theirs = GendfSection::from_rows(mf, mt, &sec.rows).expect("golden decodes");
        assert_eq!(
            (ours.nl, ours.nz),
            (theirs.nl, theirs.nz),
            "MF={mf}/MT={mt}: NL/NZ"
        );
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
            "[{LABEL}] MF={mf}/MT={mt}: {} records, {n} words, worst rel dev {:.3e} (ig {}, word \
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
        let (za, awr, xs) = tab1(&endf, 23, *mt);
        let sigma = PointwiseXs::LinLin(Arc::new(xs.pairs));
        let ours =
            gaminr_reaction(reaction, &sigma, &flux, &egg, LORD, za, awr, &mut heating).unwrap();
        compare(*mf, *mt, &ours);
    }
    let (za, awr, _) = tab1(&endf, 23, 501);
    let total = heating.section(525, za, awr);
    compare(23, 525, &total);
    println!(
        "[{LABEL}] worst overall {:.3e} at MF={}/MT={} ig {} word {}",
        worst_all.0, worst_all.1, worst_all.2, worst_all.3, worst_all.4
    );
    assert!(
        worst_all.0 < TOL,
        "GAMINR deviates from NJOY: {worst_all:?}"
    );
}
