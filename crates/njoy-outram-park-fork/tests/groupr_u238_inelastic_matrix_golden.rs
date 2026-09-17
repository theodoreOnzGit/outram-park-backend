//! GROUPR **discrete-level inelastic** vectors and transfer matrices
//! (MF=3 and MF=6, MT=51/52/60/89) vs an NJOY2016 GENDF tape — the first
//! oracle on the `q < 0` (threshold) path of `getdis`.
//!
//! # Why this test exists
//! Every port defect found in this session came from an oracle run, not
//! from reading the port. The discrete-level path of `getdis` — the
//! threshold `thresh = (awr+1)(-q)/awr` (`getsig:6751`), the CM speed
//! ratio `ast = sqrt(awr2) sqrt(1 - thresh/e)` (`:9440`), the critical
//! energies `ecl/ech = thresh/(1 - af^2/awr2)` (`:9637,9665`), and the
//! lab-cosine/energy map with `ast < 1` near threshold — existed with no
//! oracle at all; only elastic (`q = 0`, `ast` constant) had been pinned.
//!
//! # Oracles
//! Two GENDF tapes from NJOY2016 `ac5adf5` (2026-09-10; decks committed
//! next to them), both GROUPR alone on NJOY's own 293.6 K U-238 PENDF, 29
//! groups, `iwt = 3`, `lord = 3`, reactions `3/51 3/52 3/60 3/89 6/51 6/52
//! 6/60 6/89`:
//! - `…-6sigz-lord3-inelastic-mf3-mf6.gendf`: six sigma-zero values. Per
//!   `groupr.f90:5816-5823` only `MF=3/MT=51` is self-shielded (`nz = 6`);
//!   the rest carry `nz = 1` — but because the deck has `nsigz > 1`,
//!   `getflx` serves *every* reaction from the tabulated `genflx` flux,
//!   dilution 1 (= 1e10 b) for the `nz = 1` ones (`:6478-6510`), whose
//!   break points are the whole total-cross-section grid.
//! - `…-1sigz-lord3-inelastic-mf3-mf6.gendf`: one sigma-zero value, so
//!   `getflx` hands `panel` the bare `getwtf` weight on its own 1 % ladder
//!   (`:6512-6516`).
//!
//! The levels: MT=51 (`QI = -44.916 keV`, first record group 22), MT=52
//! (`-148.38 keV`, group 24), MT=60 (`-930.55 keV`, group 25), MT=89
//! (`-1.2857 MeV`, group 26). Near threshold the outgoing lab energy is
//! `~E/(A+1)^2`, so the matrices reach down to secondary group 4 (1–4 eV)
//! with elements of order `1e-9` — the seven-decimal rounding of the feed
//! (`getdis:9573-9580`) matters there and the elastic test's `1e-7 sigma_g`
//! floor rule is reused.
//!
//! # Methodology
//! Pointwise input = the same NJOY PENDF (`OUTRAM_PARK_NJOY_U238_PENDF`),
//! `QI`/`LR` from its MF=3 TAB1 heads via [`read_pendf_cross_section`];
//! File 4 (`LTT = 1`, `LCT = 2`) from the committed ENDF tape; feed =
//! [`TwoBodyFeed`] with that `q`; quadrature = [`two_body_matrix`]; the
//! vectors = `group_integral`. Flux per the deck rule above: the Bondarenko
//! components on NJOY's greedy grid (six-sigma-zero deck) or the bare 1/E
//! weight (one-sigma-zero deck).
//!
//! **Prediction stated before running:** identical record structure, every
//! transfer element within 1e-5 relative or within two units of
//! `1e-7 sigma_g`, every vector within 1e-5 — unless the threshold path has
//! a defect, in which case the near-threshold group (22 for MT=51) and the
//! low secondary groups are where it will show.
//!
//! # What the oracle found (2026-09-10)
//! Three things, in the order they surfaced; none was visible by reading.
//! 1. **`GroupFlux::analytic` stepped at GAMINR's 1.05, not `getwtf`'s
//!    1.01** (`groupr.f90:5146`). On the one-sigma-zero deck that put the
//!    threshold-group vectors 3.5e-4 … 5.0e-3 off and every group flux
//!    2.7e-4 … 3.4e-4 off (1/E trapezoid over 5 % panels). Fixed:
//!    `GETWTF_STEP = 1.01`; the same deck is now within 3.1e-6 / 1e-13.
//! 2. **The flux for `nz = 1` reactions in an `nsigz > 1` deck is the
//!    tabulated `genflx` flux, not the bare weight** — a wrong assumption in
//!    the first draft of this test, not a port defect (the port's
//!    `FluxComponents` doc already states the rule). With the bare weight
//!    the six-sigma-zero threshold groups were 4.6e-5 low in P0 row sum and
//!    1.5e-6 off in flux because the total-cross-section grid below the
//!    reaction's own threshold was missing from the panel set.
//! 3. **`getfle`'s slide keeps stale high-order coefficients**
//!    (`:9789-9793`, `do i=1,nhi: flo(i)=fhi(i)`). The MT=52 P3 transfer
//!    from group 26 to group 25 was `-9.469e-5` against NJOY's `-8.760e-5`
//!    (8 %), with P0–P2 of the same slot at the floor. An independent
//!    converged quadrature gives `-9.48e-5`, i.e. NJOY is the one carrying a
//!    quirk; replaying the partial copy (a stale `a3 = +8.1e-4` inherited
//!    from 640 keV enters the 1.04–1.06 MeV bracket where `a3` reappears)
//!    reproduces it: `-8.7605e-5`. Ported in `File4Angular::from_tape`.
//!
//! **Result after the three:** both decks, all four levels: every vector
//! within 3.2e-6, every group flux within 1.5e-7 (identical arithmetic on
//! the six-sigma-zero deck: 1e-13), every transfer element within 1e-5 or
//! under 1.0 unit of `1e-7 sigma_g` (worst 0.954, MT=60 group 27). The
//! P0 row sums match NJOY's own to 3e-7 — including the threshold groups,
//! where NJOY's matrix row and its MF=3 vector legitimately differ by 1e-4
//! because the matrix path breaks its panels at every critical energy.

use std::collections::BTreeMap;
use std::sync::Arc;

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::groupr::file4::{File4Angular, NLD};
use njoy_outram_park_fork::groupr::gendf::GendfSection;
use njoy_outram_park_fork::groupr::matrix_panel::{two_body_matrix, FluxComponents, MatrixHeader};
use njoy_outram_park_fork::groupr::panel::{group_integral, GroupFlux, PointwiseXs};
use njoy_outram_park_fork::groupr::pendf_feed::read_pendf_cross_section;
use njoy_outram_park_fork::groupr::two_body::TwoBodyFeed;
use njoy_outram_park_fork::groupr::unresolved::genflx_bondarenko_components;
use njoy_outram_park_fork::groupr::weights::AnalyticWeight;
use njoy_outram_park_fork::reference_data::{reference_endf_or_skip, reference_file_or_skip};

const MAT: i32 = 9237;
const TEMP_K: f64 = 293.6;
/// Six-sigma-zero deck (`nsigz = 6`): `getflx` serves every reaction from the
/// tabulated `genflx` flux (`groupr.f90:6478-6510`).
const GOLDEN_6SIGZ: &str = "u238-ENDF8.0-293.6K-29g-iwt3-6sigz-lord3-inelastic-mf3-mf6.gendf";
/// One-sigma-zero deck (`nsigz = 1`): `getflx` serves the bare `getwtf`
/// weight on its own 1 % ladder (`:6512-6516`).
const GOLDEN_1SIGZ: &str = "u238-ENDF8.0-293.6K-29g-iwt3-1sigz-lord3-inelastic-mf3-mf6.gendf";
const NJOY_PENDF_ENV: &str = "OUTRAM_PARK_NJOY_U238_PENDF";
const ENDF: &str = "n-092_U_238.endf";
const MTS: [i32; 4] = [51, 52, 60, 89];
const NL: usize = 4;

const BOUNDS: [f64; 30] = [
    1e-5, 0.1, 0.625, 1.0, 4.0, 6.5, 8.0, 10.0, 15.0, 20.0, 25.0, 30.0, 40.0, 50.0, 100.0, 200.0,
    500.0, 1e3, 2e3, 5e3, 1e4, 2e4, 5e4, 1e5, 5e5, 1e6, 2e6, 5e6, 1e7, 2e7,
];
const SIGZ: [f64; 6] = [1e10, 1e4, 1e3, 1e2, 10.0, 1.0];
const N_SIGZ: usize = SIGZ.len();

const TOL: f64 = 1e-5;
const FLOOR_UNITS: f64 = 2.0;

/// NJOY's flux grid for `iwt = 3`, `nsigz > 1` (as in the elastic test).
fn njoy_flux_grid(sigt: &[(f64, f64)]) -> Vec<f64> {
    let etop = sigt.last().unwrap().0;
    let mut grid = Vec::with_capacity(sigt.len() * 2);
    let mut e = sigt[0].0;
    grid.push(e);
    let mut ip = 1;
    while e < etop {
        while ip < sigt.len() && sigt[ip].0 <= e {
            ip += 1;
        }
        let en = if ip < sigt.len() { sigt[ip].0 } else { etop };
        let enext = (1.01 * e).min(en);
        grid.push(enext);
        e = enext;
    }
    grid
}

/// Which `getflx` branch the deck exercises.
#[derive(Clone, Copy)]
enum Deck {
    /// `nsigz = 6`: tabulated `genflx` flux for every reaction; `nz = 6`
    /// for MF=3/MT=51, `nz = 1` (dilution 1 = 1e10 b) for the rest.
    SixSigz,
    /// `nsigz = 1`: the bare 1/E weight on `getwtf`'s ladder for everything.
    OneSigz,
}

struct Inputs {
    golden: Tape,
    pendf: Tape,
    endf: Tape,
    /// Per dilution, the `NL` Legendre flux components `wtf*fac^(il+1)`
    /// on NJOY's greedy grid (`SixSigz`), or the bare weight (`OneSigz`).
    components: Vec<Vec<GroupFlux>>,
}

fn load(deck: Deck, label: &str) -> Option<Inputs> {
    let golden = match deck {
        Deck::SixSigz => GOLDEN_6SIGZ,
        Deck::OneSigz => GOLDEN_1SIGZ,
    };
    let golden_path = reference_file_or_skip("gendf", golden, label)?;
    let endf_path = reference_endf_or_skip(ENDF, label)?;
    let Ok(pendf_path) = std::env::var(NJOY_PENDF_ENV) else {
        println!("[{label}] SKIP: set {NJOY_PENDF_ENV} to the NJOY 293.6 K PENDF");
        return None;
    };
    let pendf = Tape::read_file(std::path::Path::new(&pendf_path)).expect("NJOY PENDF parses");
    let weight = GroupFlux::analytic(AnalyticWeight::OneOverE, TEMP_K);
    let components = match deck {
        Deck::SixSigz => {
            let total = read_pendf_cross_section(&pendf, MAT, 1).expect("PENDF MF=3/MT=1");
            let PointwiseXs::LinLin(sigt_pairs) = &total.xs else {
                panic!("non-tabulated total")
            };
            let grid = njoy_flux_grid(sigt_pairs);
            let sig_t = PointwiseXs::LinLin(Arc::new((**sigt_pairs).clone()));
            genflx_bondarenko_components(&sig_t, None, &weight, 0.0, &SIGZ, &grid, NL).unwrap()
        }
        Deck::OneSigz => vec![vec![weight]],
    };
    Some(Inputs {
        golden: Tape::read_file(&golden_path).expect("golden GENDF parses"),
        pendf,
        endf: Tape::read_file(&endf_path).expect("ENDF tape parses"),
        components,
    })
}

/// **Vectors (MF=3).** In the six-sigma-zero deck MT=51 is self-shielded
/// over six dilutions and MT=52/60/89 carry `nz = 1`, served from the same
/// tabulated flux at dilution 1 (`groupr.f90:6478-6510`); in the
/// one-sigma-zero deck every reaction uses the bare 1/E weight on the 1 %
/// ladder (`:6512-6516`). Compared group by group against the GENDF P0 words.
fn vectors(deck: Deck, label: &str) {
    let Some(inp) = load(deck, label) else { return };
    let mut worst: BTreeMap<i32, (f64, usize, usize, f64, f64)> = BTreeMap::new();
    for mt in MTS {
        let xs = read_pendf_cross_section(&inp.pendf, MAT, mt).expect("PENDF MF=3 section");
        let sec = inp.golden.section(MAT, 3, mt).expect("GENDF MF=3 section");
        let golden = GendfSection::from_rows(3, mt, &sec.rows).expect("MF=3 decodes");
        let nz = golden.nz as usize;
        let nl = golden.nl as usize;
        let want_nz = match (deck, mt) {
            (Deck::SixSigz, 51) => N_SIGZ,
            _ => 1,
        };
        assert_eq!(nz, want_nz, "MT={mt}: oracle NZ");
        println!(
            "[{label}] MF=3/MT={mt}: QI = {} eV, LR = {}, NL = {nl}, NZ = {nz}, {} records",
            xs.qi,
            xs.lr,
            golden.records.len()
        );
        let mut w = (0.0f64, 0usize, 0usize, 0.0, 0.0);
        for rec in &golden.records {
            let g = rec.ig as usize;
            assert_eq!(rec.ng2, 2, "MT={mt} ig {g}: vector record");
            for iz in 0..nz {
                let phi = &inp.components[iz][0];
                let integ = group_integral(&xs.xs, phi, BOUNDS[g - 1], BOUNDS[g]);
                // Words: flux (it=0) then sigma (it=1), il fastest then iz.
                let njoy_flux = rec.data[iz * nl];
                let njoy_sig = rec.data[nl * nz + iz * nl];
                for (ours, theirs) in [(integ.flux, njoy_flux), (integ.average(), njoy_sig)] {
                    let r = ((ours - theirs) / theirs).abs();
                    if r > w.0 {
                        w = (r, g, iz, ours, theirs);
                    }
                }
            }
        }
        println!(
            "[{label}] MF=3/MT={mt}: worst rel dev {:.3e} (group {}, iz {}: ours {:.7e} vs NJOY \
             {:.7e})",
            w.0, w.1, w.2, w.3, w.4
        );
        worst.insert(mt, w);
    }
    for (mt, w) in worst {
        assert!(w.0 < TOL, "MF=3/MT={mt} deviates from NJOY: {w:?}");
    }
}

/// **Matrices (MF=6, `lord = 3`, `nz = 1`).** The `getdis` threshold path
/// through `two_body_matrix`, word for word against the GENDF with the
/// elastic test's floor rule for elements below the feed's own rounding.
/// The flux components are dilution 1 of the tabulated flux (`SixSigz`) or
/// the bare weight (`OneSigz`).
fn matrices(deck: Deck, label: &str) {
    let Some(inp) = load(deck, label) else { return };
    let fluxes = FluxComponents {
        per_dilution: vec![inp.components[0].clone()],
    };
    let mut worst_all = 0.0f64;
    let mut violations: Vec<(i32, i32, usize, f64)> = Vec::new();
    for mt in MTS {
        let xs = read_pendf_cross_section(&inp.pendf, MAT, mt).expect("PENDF MF=3 section");
        let sec = inp.golden.section(MAT, 6, mt).expect("GENDF MF=6 section");
        let golden = GendfSection::from_rows(6, mt, &sec.rows).expect("MF=6 decodes");
        assert_eq!(golden.nl, NL as i32, "MT={mt}: oracle NL");
        assert_eq!(golden.nz, 1, "MT={mt}: oracle NZ");
        let angular = File4Angular::from_tape(&inp.endf, MAT, mt, NLD).expect("MF=4 section");
        let mut feed = TwoBodyFeed::new(angular, &BOUNDS, xs.awr, xs.qi, xs.lr).unwrap();
        let header = MatrixHeader {
            mf: 6,
            mt,
            za: 92238.0,
            zam: 0.0,
            lrflag: xs.lr,
            temperature_k: TEMP_K,
            emaxx: 2.0e7,
        };
        let ours = two_body_matrix(&xs.xs, &fluxes, &mut feed, NL, &header).unwrap();
        assert_eq!(
            ours.records.iter().map(|r| r.ig).collect::<Vec<_>>(),
            golden.records.iter().map(|r| r.ig).collect::<Vec<_>>(),
            "MT={mt}: initial groups written"
        );
        let nlz = NL;
        let mut worst = (0.0f64, 0i32, 0usize, 0.0f64, 0.0f64);
        let mut worst_flux = 0.0f64;
        let mut worst_units = (0.0f64, 0i32, 0usize, 0.0f64, 0.0f64);
        let mut n_words = 0usize;
        let mut n_floor = 0usize;
        for (a, b) in ours.records.iter().zip(&golden.records) {
            assert_eq!(
                (a.ig2lo, a.ng2),
                (b.ig2lo, b.ng2),
                "MT={mt} ig {}: (ig2lo, ng2) ours vs NJOY",
                a.ig
            );
            assert_eq!(
                a.data.len(),
                b.data.len(),
                "MT={mt} ig {}: word count",
                a.ig
            );
            let sig_g: f64 = (1..b.ng2 as usize).map(|it| b.data[it * nlz]).sum();
            for (k, (&x, &y)) in a.data.iter().zip(&b.data).enumerate() {
                n_words += 1;
                if k < nlz {
                    worst_flux = worst_flux.max(((x - y) / y).abs());
                    continue;
                }
                assert_eq!(
                    x == 0.0,
                    y == 0.0,
                    "MT={mt} ig {} word {k}: zero pattern {x} vs {y}",
                    a.ig
                );
                if y == 0.0 {
                    continue;
                }
                let r = ((x - y) / y).abs();
                if r < TOL {
                    if r > worst.0 {
                        worst = (r, a.ig, k, x, y);
                    }
                    continue;
                }
                let units = (x - y).abs() / (1.0e-7 * sig_g);
                n_floor += 1;
                if units > worst_units.0 {
                    worst_units = (units, a.ig, k, x, y);
                }
                if units > FLOOR_UNITS {
                    println!(
                        "[{label}] MF=6/MT={mt} ig {} word {k} (it {}, il {}): ours {x:.7e} vs \
                         NJOY {y:.7e} ({r:.2e} rel, {units:.2} units of 1e-7*sigma_g = {sig_g:.4e})",
                        a.ig,
                        k / nlz,
                        k % nlz
                    );
                    violations.push((mt, a.ig, k, units));
                }
            }
        }
        println!(
            "[{label}] MF=6/MT={mt}: {} records, {n_words} words; {n_floor} outside {TOL:e} rel, \
             worst {:.3} units of 1e-7*sigma_g (ig {}, word {}: ours {:.7e} vs NJOY {:.7e}); \
             worst rel dev among the rest {:.3e} (ig {}, word {}: ours {:.7e} vs NJOY {:.7e}); \
             worst flux dev {:.3e}",
            ours.records.len(),
            worst_units.0,
            worst_units.1,
            worst_units.2,
            worst_units.3,
            worst_units.4,
            worst.0,
            worst.1,
            worst.2,
            worst.3,
            worst.4,
            worst_flux
        );
        worst_all = worst_all.max(worst.0).max(worst_flux);
    }
    assert!(
        violations.is_empty(),
        "{} elements beyond {FLOOR_UNITS} units of 1e-7*sigma_g: {violations:?}",
        violations.len()
    );
    assert!(worst_all < TOL, "inelastic matrix deviates: {worst_all:e}");
}

#[test]
fn njoy_pendf_inelastic_vectors_6sigz() {
    vectors(Deck::SixSigz, "groupr-u238-inelastic-6sigz");
}

#[test]
fn njoy_pendf_inelastic_matrices_6sigz() {
    matrices(Deck::SixSigz, "groupr-u238-inelastic-6sigz");
}

#[test]
fn njoy_pendf_inelastic_vectors_1sigz() {
    vectors(Deck::OneSigz, "groupr-u238-inelastic-1sigz");
}

#[test]
fn njoy_pendf_inelastic_matrices_1sigz() {
    matrices(Deck::OneSigz, "groupr-u238-inelastic-1sigz");
}
