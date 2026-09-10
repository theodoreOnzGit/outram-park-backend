//! SAMM (`LRF=7` R-matrix limited) oracle: the crate's Reich-Moore-limited
//! R-matrix cross sections for ENDF/B-VII.1 Cl-35 (MAT 1725) against
//! NJOY2016's own RECONR output (`reference-data/reconr/cl35-ENDF7.1-0K-err0.01.pendf`,
//! `tape22` of the committed test-20 deck, `reconr / 1725 0 0 / 0.01 /`).
//!
//! # Why this test exists (bead `op-cjw.2`)
//!
//! Every phase of `samm.f90` was translated in July 2026 but never run
//! against a real `LRF=7` evaluation: no committed tape carried one, and
//! the public data hosts refuse this environment. The NJOY2016 upstream
//! test suite ships `tests/resources/cl35rml` — the ENDF/B-VII.1 Cl-35
//! evaluation (ORNL, Sayer/Guber/Leal/Larson/Young; `$Rev:: 532`,
//! 2011-12-05), `LRU=1/LRF=7/KRM=3/IFG=0`, 8 spin groups, particle pairs
//! `(γ, n, p)` so `MT=2/102/600` all come out of the R-matrix — and it is
//! now committed as `reference-data/endf/n-017_Cl_035-ENDF7.1.endf`.
//!
//! # Tiers
//!
//! **Tier 1 — the R-matrix kernel at NJOY's own nodes.** RECONR writes the
//! exact 0 K cross section at every node of its grid, so evaluating
//! [`njoy_outram_park_fork::samm::xsformula::cssammy`] at those same
//! energies and adding the ENDF MF=3 background isolates
//! `setup`/`abpart`/`setr`/`yinvrs`/`setxqx`/`sectio` from any grid
//! question. Prediction (before the first run): the tape's printing floor,
//! 5e-7 relative, for **elastic** (its MF=3 background is exactly zero
//! below 1.2 MeV); for **capture** and the **(n,p)** channel the MF=3
//! background is a `1/v` log-log TAB1 that RECONR linearises to `err =
//! 0.01`, so those carry an extra `err × background` allowance. The
//! **total** is not compared against MF=3/MT=1's own background: on the
//! oracle tape `MT=1` is the sum of every partial (checked numerically,
//! 2026-09-11: `MT1 − (MT2+MT102+MT600) = MT107` to 4e-8 at 1.15 MeV,
//! where the evaluation's own `MT=1` TAB1 is `1e-20` but `MT=600`'s is
//! `7e-3` b), so the kernel's total is asserted equal to the sum of its
//! own three channels instead.
//!
//! **Tier 2 — the crate's RECONR end to end** (`reconr`, `err = 0.001`):
//! the PENDF the crate would hand BROADR, interpolated at a sample of
//! NJOY's nodes. Reported, asserted loosely; the `MT=600` section is
//! checked too because upstream RECONR carries the R-matrix (n,p) channel
//! into its own MF=3 section (10 730 points on the oracle tape).

use njoy_outram_park_fork::endf::interp::eval_tab1;
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::mf2::parse_resonance_info;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::{reference_endf_or_skip, reference_file_or_skip};
use njoy_outram_park_fork::samm::setup::setup;
use njoy_outram_park_fork::samm::xsformula::cssammy;

const MAT: i32 = 1725;
const LABEL: &str = "reconr-cl35-rml";
const ENDF: &str = "n-017_Cl_035-ENDF7.1.endf";
const GOLDEN: &str = "cl35-ENDF7.1-0K-err0.01.pendf";
/// The oracle deck's RECONR tolerance (background linearisation).
const ORACLE_ERR: f64 = 0.01;
/// 7 printed figures -> half-ulp 5e-7.
const PRINT_FLOOR: f64 = 6e-7;

struct Tab {
    interp: Vec<(u32, u32)>,
    pairs: Vec<(f64, f64)>,
}

fn tab1(tape: &Tape, mt: i32) -> Tab {
    let sec = tape.section(MAT, 3, mt).expect("MF=3 section");
    let mut cur = SectionCursor::new(&sec.rows);
    let _ = cur.read_cont().unwrap();
    let t = cur.read_tab1().unwrap();
    Tab {
        interp: t.interp,
        pairs: t.pairs,
    }
}

fn eval(t: &Tab, e: f64) -> f64 {
    eval_tab1(e, &t.interp, &t.pairs).unwrap_or(0.0)
}

struct Worst {
    rel: f64,
    e: f64,
    ours: f64,
    njoy: f64,
}

impl Worst {
    fn new() -> Self {
        Worst {
            rel: 0.0,
            e: 0.0,
            ours: 0.0,
            njoy: 0.0,
        }
    }
    fn note(&mut self, e: f64, ours: f64, njoy: f64, allow: f64) {
        // deviation in units of the allowance; > 1 fails
        let d = (ours - njoy).abs() / allow;
        if d > self.rel {
            self.rel = d;
            self.e = e;
            self.ours = ours;
            self.njoy = njoy;
        }
    }
}

#[test]
fn cl35_rml_kernel_matches_njoy_reconr_at_its_nodes() {
    let Some(endf_path) = reference_endf_or_skip(ENDF, LABEL) else {
        return;
    };
    let Some(golden) = reference_file_or_skip("reconr", GOLDEN, LABEL) else {
        return;
    };
    let endf = Tape::read_file(&endf_path).expect("ENDF parses");
    let njoy = Tape::read_file(&golden).expect("NJOY tape22 parses");

    let mf2 = endf.section(MAT, 2, 151).expect("MF=2");
    let info = parse_resonance_info(mf2).expect("MF=2 parses");
    let range = info
        .resolved_rml_ranges()
        .next()
        .expect("one LRF=7 range");
    let rml = range.rml.as_ref().unwrap();
    assert_eq!(rml.section.spin_groups.len(), 8);
    assert_eq!(rml.section.particle_pairs.len(), 3);
    assert_eq!(rml.section.particle_pairs[2].mt, 600);
    let mut section = rml.section.clone();
    let st = setup(&mut section, rml.awr).expect("samm setup");

    // ENDF backgrounds (MT=2 is identically zero below 1.2 MeV; MT=1 and
    // MT=102 share a 1/v log-log curve; MT=600 is 1e-20 below 1 MeV).
    let bg = [tab1(&endf, 1), tab1(&endf, 2), tab1(&endf, 102), tab1(&endf, 600)];
    let nj = [tab1(&njoy, 1), tab1(&njoy, 2), tab1(&njoy, 102), tab1(&njoy, 600)];
    for t in &nj {
        assert_eq!(t.pairs.len(), 10730, "NJOY grid is unionised");
    }

    let mut worst = [Worst::new(), Worst::new(), Worst::new(), Worst::new()];
    let mut n = 0usize;
    for (i, &(e, _)) in nj[1].pairs.iter().enumerate() {
        if e >= range.eh {
            break;
        }
        // NJOY writes duplicate energies at MF=3 discontinuities; keep the
        // first.
        if i > 0 && nj[1].pairs[i - 1].0 == e {
            continue;
        }
        let r = cssammy(
            &section,
            &st.kinematics,
            &st.amplitudes,
            &st.quantum_info,
            e,
        );
        let sig_p = r.other.iter().find(|(mt, _)| *mt == 600).map_or(0.0, |x| x.1);
        // the kernel's total is elastic + the whole non-elastic bucket
        assert!(
            (r.total - (r.elastic + r.capture + sig_p)).abs() <= 1e-12 * r.total.abs(),
            "total = elastic + capture + (n,p) at E={e:e}"
        );
        let ours = [
            r.total + eval(&bg[0], e),
            r.elastic + eval(&bg[1], e),
            r.capture + eval(&bg[2], e),
            sig_p + eval(&bg[3], e),
        ];
        for k in 1..4 {
            let njv = nj[k].pairs[i].1;
            let allow = PRINT_FLOOR * njv.abs() + 1.5 * ORACLE_ERR * eval(&bg[k], e).abs() + 1e-12;
            worst[k].note(e, ours[k], njv, allow);
        }
        // total: NJOY's MT=1 is the sum of all partials; the three R-matrix
        // channels plus MF=3 MT=107's background account for it (reported)
        let njv = nj[0].pairs[i].1;
        worst[0].note(e, ours[0], njv, 1e-2 * njv.abs() + 1e-12);
        n += 1;
    }
    let names = ["total (reported)", "elastic", "capture", "(n,p) MT=600"];
    println!("{LABEL}: {n} NJOY nodes below {} eV", range.eh);
    for k in 0..4 {
        let w = &worst[k];
        println!(
            "  {:<13} worst {:.3e} x allowance at E={:.6e}: ours {:.7e} njoy {:.7e} (rel {:.2e})",
            names[k],
            w.rel,
            w.e,
            w.ours,
            w.njoy,
            (w.ours - w.njoy).abs() / w.njoy.abs().max(1e-300)
        );
    }
    for k in 1..4 {
        assert!(
            worst[k].rel <= 1.0,
            "{}: {:.3e} x allowance at E={:.6e} (ours {:.7e}, njoy {:.7e})",
            names[k],
            worst[k].rel,
            worst[k].e,
            worst[k].ours,
            worst[k].njoy
        );
    }
}

/// Tier 2: the crate's own RECONR (`err = 0.001`) against the oracle at a
/// sample of NJOY's nodes. Interpolation of two independently thinned
/// grids bounds this at the tolerance; asserted at 3e-3 for elastic and
/// capture, reported for the total and the (n,p) channel.
#[test]
fn cl35_crate_reconr_end_to_end_reported() {
    let Some(endf_path) = reference_endf_or_skip(ENDF, LABEL) else {
        return;
    };
    let Some(golden) = reference_file_or_skip("reconr", GOLDEN, LABEL) else {
        return;
    };
    let endf = Tape::read_file(&endf_path).expect("ENDF parses");
    let njoy = Tape::read_file(&golden).expect("NJOY tape22 parses");
    let r = reconr(
        &endf,
        &ReconrConfig {
            mat: MAT,
            tolerance: 0.001,
            temperature: 0.0,
        },
    )
    .expect("RECONR");
    let sample = [
        1e-5, 1e-3, 0.0253, 1.0, 100.0, 1000.0, 5000.0, 2.5e4, 1e5, 3e5, 6e5, 8e5, 1.1e6,
    ];
    for mt in [1, 2, 102, 600] {
        let nj = tab1(&njoy, mt);
        let ours = r
            .sections
            .iter()
            .find(|s| i32::from(s.mt) == mt)
            .expect("crate section");
        let interp = [(ours.pairs.len() as u32, 2)];
        let mut worst = (0.0f64, 0.0f64);
        for &e in &sample {
            let a = eval_tab1(e, &interp, &ours.pairs).unwrap();
            let b = eval(&nj, e);
            let rel = (a - b).abs() / b.abs().max(1e-30);
            if rel > worst.0 {
                worst = (rel, e);
            }
        }
        println!(
            "{LABEL} tier 2 MT={mt}: {} points (NJOY {}), worst {:.3e} at {:.3e} eV",
            ours.pairs.len(),
            nj.pairs.len(),
            worst.0,
            worst.1
        );
        if mt == 2 || mt == 102 {
            assert!(worst.0 < 3e-3, "MT={mt} worst {:.3e} at {:.3e} eV", worst.0, worst.1);
        }
    }
}
