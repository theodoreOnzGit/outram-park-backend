//! **The resonance shapes of neutronics lesson pages 12 to 16** (gh:#906,
//! SLBW, full R-matrix, Reich–Moore, MLBW and Adler–Adler, in the order of
//! the maintainer's amendment of 2026-10-16): written to
//! `docs/site/science/neutronics/resonance-data.js`.
//!
//! ```bash
//! cargo run --release -p njoy-outram-park-fork --example lesson_resonances [out.js]
//! ```
//!
//! - **U-238's lowest positive-energy s-wave level**, read from MF=2 of
//!   `n-092_U_238.endf` (ENDF/B-VIII.0, MAT 9237): E_r, J, Γn, Γγ, and the
//!   range's SPI, AP and NAPS. They are **Reich–Moore** parameters; page 12
//!   draws them with the SLBW formula and says so. The scattering radius is
//!   the l = 0 list's APL (U-238 gives AP = 0 at range level).
//! - **Reference cross sections from this crate's RECONR port**
//!   (`reconr::slbw::eval_slbw_lstate`, `eval_mlbw_lstate` and
//!   `reconr::rm::eval_rm_lstate`: NJOY2016's `csslbw`, `csmlbw` and
//!   `csrmat`), at fixed energies, for the U-238 level (SLBW) and for an
//!   illustrative pair of levels (SLBW sum, MLBW and Reich–Moore, same J and
//!   different J, without and with a fission width). The page's JavaScript
//!   reimplements the same formulas; `check-physics.js` compares it with
//!   these numbers.
//! - **How many resolved levels U-238 lists** for each l (the length of each
//!   l-list in its first resolved range).
//! - **Which formalism the local evaluations use**: the first resolved range
//!   (`LRU = 1`, its `LRF`) of every `n-*.endf` tape in
//!   `reference-data/endf/` whose MF=1 text says "ENDF/B-VIII.0", leaving
//!   out betas and other libraries by file name.
//!
//! Driver glue (`main`): the MF=2 reader and the SLBW/MLBW routines it calls
//! are tested in the crate.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::errorr::resprx::mf2::Mf2Resonances;
use njoy_outram_park_fork::reconr::mf2::{RmLState, RmResonance};
use njoy_outram_park_fork::reconr::rm::eval_rm_lstate;
use njoy_outram_park_fork::reconr::slbw::{channel_radius, eval_mlbw_lstate, eval_slbw_lstate, WAVE_K};
use njoy_outram_park_fork::reference_data::{reference_endf, reference_endf_dir};
use std::fmt::Write as _;

fn nums(v: &[f64]) -> String {
    v.iter().map(|x| format!("{x:.15e}")).collect::<Vec<_>>().join(",")
}

fn main() -> Result<(), String> {
    let out = std::env::args().nth(1).unwrap_or_else(|| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/site/science/neutronics/resonance-data.js").to_string()
    });
    // U-238's lowest positive-energy s-wave level.
    let path = reference_endf("n-092_U_238.endf").ok_or("n-092_U_238.endf missing")?;
    let tape = Tape::read(std::fs::File::open(&path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let mat = *tape.materials().first().ok_or("no material")?;
    let m = Mf2Resonances::read(&tape, mat).map_err(|e| e.to_string())?;
    let rg = m.ranges.iter().find(|r| r.range.l1 == 1).ok_or("no resolved range")?;
    let (lrf, naps) = (rg.range.l2, rg.range.n2);
    let sc = rg.spi_cont.as_ref().ok_or("no SPI/AP record")?;
    let l0 = rg.lists.iter().find(|l| l.head.l1 == 0).ok_or("no l = 0 list")?;
    let awri = l0.head.c1;
    // Reich-Moore carries an l-dependent radius APL on each l-list (ENDF-102
    // §2.2.1.3); U-238 gives AP = 0 at range level and APL = 0.948 for l = 0.
    let (spi, ap) = (sc.c1, if l0.head.c2 != 0.0 { l0.head.c2 } else { sc.c2 });
    let lvl = l0.data.chunks(6).filter(|c| c[0] > 0.0).min_by(|a, b| a[0].total_cmp(&b[0])).ok_or("no positive level")?;
    let (er, aj, gn, gg) = (lvl[0], lvl[1], lvl[2], lvl[3]);
    let ra = channel_radius(awri, naps, ap);
    println!("U-238 MAT {mat}: LRF {lrf}, NAPS {naps}, SPI {spi}, AP {ap}, AWRI {awri}, ra {ra}; lowest l=0 level E_r {er} eV, J {aj}, GN {gn}, GG {gg}");
    let gt = gn + gg;
    let res = [(er, aj, gt, gn, gg, 0.0)];
    let es: Vec<f64> = [-10.0, -2.0, -0.5, 0.0, 0.5, 2.0, 10.0].iter().map(|k| er + k * gt).chain([1.0, 3.0, 20.0]).collect();
    let (mut el, mut cap) = (Vec::new(), Vec::new());
    for &e in &es {
        let s = eval_slbw_lstate(e, &res, 0, spi, ap, awri, ra);
        el.push(s.elastic);
        cap.push(s.capture);
    }
    // An illustrative pair of s-wave levels (stated on the page as such).
    let (pa, pspi, pap) = (100.0, 0.5, 0.6);
    let pra = channel_radius(pa, 0, pap);
    let pair = |j2: f64| [(10.0, 1.0, 1.03, 1.0, 0.03, 0.0), (12.0, j2, 1.03, 1.0, 0.03, 0.0)];
    let pe: Vec<f64> = (0..41).map(|i| 6.0 + 0.2 * i as f64).collect();
    let (mut s_same, mut m_same, mut s_diff, mut m_diff) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for &e in &pe {
        s_same.push(eval_slbw_lstate(e, &pair(1.0), 0, pspi, pap, pa, pra).elastic);
        m_same.push(eval_mlbw_lstate(e, &pair(1.0), 0, pspi, pap, pa, pra).elastic);
        s_diff.push(eval_slbw_lstate(e, &pair(0.0), 0, pspi, pap, pa, pra).elastic);
        m_diff.push(eval_mlbw_lstate(e, &pair(0.0), 0, pspi, pap, pa, pra).elastic);
    }
    // The same pair through Reich-Moore (no APL, NAPS 0), and every pair
    // formalism again with a fission width Γf on both levels.
    let gfp = 0.2;
    let rm_pair = |j2: f64, gf: f64| RmLState {
        awri: pa,
        apl: 0.0,
        l: 0,
        resonances: [(10.0, 1.0), (12.0, j2)].iter().map(|&(er, aj)| RmResonance { er, aj, gn: 1.0, gg: 0.03, gfa: gf, gfb: 0.0 }).collect(),
    };
    let fpair = [(10.0, 1.0, 1.03 + gfp, 1.0, 0.03, gfp), (12.0, 1.0, 1.03 + gfp, 1.0, 0.03, gfp)];
    let mut rmv: [Vec<f64>; 7] = Default::default();
    let mut fv: [Vec<f64>; 4] = Default::default();
    for &e in &pe {
        let (a, b, c) = (eval_rm_lstate(e, &rm_pair(1.0, 0.0), pap, pspi, 0), eval_rm_lstate(e, &rm_pair(0.0, 0.0), pap, pspi, 0), eval_rm_lstate(e, &rm_pair(1.0, gfp), pap, pspi, 0));
        for (v, x) in rmv.iter_mut().zip([a.elastic, a.capture, b.elastic, b.capture, c.elastic, c.capture, c.fission]) {
            v.push(x);
        }
        let (m, s) = (eval_mlbw_lstate(e, &fpair, 0, pspi, pap, pa, pra), eval_slbw_lstate(e, &fpair, 0, pspi, pap, pa, pra));
        for (v, x) in fv.iter_mut().zip([m.capture, m.fission, s.capture, s.fission]) {
            v.push(x);
        }
    }
    // Resolved levels per l in U-238's first resolved range.
    let counts: Vec<String> = rg.lists.iter().map(|l| format!("\"{}\":{}", l.head.l1, l.data.len() / 6)).collect();
    println!("U-238 resolved levels per l: {}", counts.join(", "));
    // The formalism census.
    let mut census: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    let mut files: Vec<_> = std::fs::read_dir(reference_endf_dir()).map_err(|e| e.to_string())?.flatten().map(|e| e.path()).collect();
    files.sort();
    for f in files {
        let name = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if !name.starts_with("n-") || !name.ends_with(".endf") {
            continue;
        }
        let text = std::fs::read_to_string(&f).unwrap_or_default();
        // Released ENDF/B-VIII.0 only: no beta, no VIII.1 or other library.
        let other = ["8.1", "Beta", "ENDF7", "JENDL", "JEFF", "TENDL"].iter().any(|t| name.contains(t));
        if other || !text.lines().take(60).any(|l| l.contains("ENDF/B-VIII.0")) {
            continue;
        }
        let Ok(t) = Tape::read(std::io::Cursor::new(text.as_bytes())) else { continue };
        let Some(&mat) = t.materials().first() else { continue };
        let kind = match Mf2Resonances::read(&t, mat) {
            Ok(r) => match r.ranges.iter().find(|x| x.range.l1 == 1) {
                Some(x) => match x.range.l2 { 1 => "SLBW (LRF 1)", 2 => "MLBW (LRF 2)", 3 => "Reich-Moore (LRF 3)", 7 => "R-Matrix Limited (LRF 7)", _ => "other" },
                None => "no resolved range",
            },
            Err(_) => "no resonance parameters",
        };
        census.entry(kind.to_string()).or_default().push(name.trim_end_matches(".endf").to_string());
    }
    let mut cjs = String::new();
    for (k, v) in &census {
        println!("  {k}: {} ({})", v.len(), v.join(", "));
        let _ = write!(cjs, "\"{k}\":{},", v.len());
    }
    cjs.pop();
    let file = format!(
        "// Resonance shapes for the neutronics lessons, pages 12 to 16 (gh:#906),\n\
         // written by crates/njoy-outram-park-fork/examples/lesson_resonances.rs\n\
         // (regenerate with it; do not edit). u238: the lowest positive-energy\n\
         // s-wave level of n-092_U_238.endf MF=2 (ENDF/B-VIII.0, MAT 9237; Reich-Moore\n\
         // parameters, LRF {lrf}). ref: cross sections [b] from OUTRAM PARK's RECONR\n\
         // port (reconr::slbw, reconr::rm: NJOY2016 csslbw, csmlbw, csrmat), against which\n\
         // check-physics.js tests the page's JavaScript. census: the first resolved\n\
         // range's formalism on the local ENDF/B-VIII.0 neutron tapes.\n\
         window.RES_DATA = {{\"waveK\":{WAVE_K:.15e},\"u238\":{{\"awri\":{awri},\"spi\":{spi},\"ap\":{ap},\"naps\":{naps},\"ra\":{ra:.15e},\"er\":{er},\"aj\":{aj},\"gn\":{gn},\"gg\":{gg}}},\
         \"ref\":{{\"slbw\":{{\"e\":[{}],\"el\":[{}],\"cap\":[{}]}},\"pair\":{{\"awri\":{pa},\"spi\":{pspi},\"ap\":{pap},\"ra\":{pra:.15e},\"e\":[{}],\"slbwSame\":[{}],\"mlbwSame\":[{}],\"slbwDiff\":[{}],\"mlbwDiff\":[{}],\
         \"gf\":{gfp},\"rmSame\":{{\"el\":[{}],\"cap\":[{}]}},\"rmDiff\":{{\"el\":[{}],\"cap\":[{}]}},\"rmFis\":{{\"el\":[{}],\"cap\":[{}],\"fis\":[{}]}},\
         \"mlbwFis\":{{\"cap\":[{}],\"fis\":[{}]}},\"slbwFis\":{{\"cap\":[{}],\"fis\":[{}]}}}}}},\"u238Levels\":{{{}}},\"census\":{{{cjs}}}}};\n",
        nums(&es), nums(&el), nums(&cap), nums(&pe), nums(&s_same), nums(&m_same), nums(&s_diff), nums(&m_diff),
        nums(&rmv[0]), nums(&rmv[1]), nums(&rmv[2]), nums(&rmv[3]), nums(&rmv[4]), nums(&rmv[5]), nums(&rmv[6]),
        nums(&fv[0]), nums(&fv[1]), nums(&fv[2]), nums(&fv[3]), counts.join(",")
    );
    std::fs::write(&out, file).map_err(|e| e.to_string())?;
    println!("wrote {out}");
    Ok(())
}
