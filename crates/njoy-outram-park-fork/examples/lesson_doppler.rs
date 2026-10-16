//! **The numbers of neutronics lesson page 22** (gh:#906, "Thermal effects for
//! lone atoms: Doppler broadening"): written to
//! `docs/site/science/neutronics/doppler-data.js`.
//!
//! ```bash
//! cargo run --release -p njoy-outram-park-fork --example lesson_doppler [out.js]
//! ```
//!
//! - **U-238's 6.674 eV capture resonance** (ENDF/B-VIII.0) reconstructed by
//!   the RECONR port at 0 K and broadened by the BROADR port
//!   (`broadr::doppler_broaden`, NJOY's SIGMA1 kernel) at 293.6 K and 1200 K,
//!   on the stretch 4-10 eV: the peak, its full width at half maximum, and the
//!   area under the curve between 5.5 and 8 eV.
//! - **The manual's invariance check**: a pure 1/v cross section, broadened
//!   at the same temperatures with the same kernel, and its largest relative
//!   change between 0.01 and 10 eV.
//!
//! Driver glue (`main`): RECONR and BROADR are tested in the crate.

use njoy_outram_park_fork::broadr::doppler_broaden;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::reference_endf;
use std::fmt::Write as _;

/// The exact area of a lin-lin table over [a, b].
fn area(p: &[(f64, f64)], a: f64, b: f64) -> f64 {
    let mut s = 0.0;
    for q in p.windows(2) {
        let ((x0, y0), (x1, y1)) = (q[0], q[1]);
        let (lo, hi) = (x0.max(a), x1.min(b));
        if hi <= lo || x1 <= x0 {
            continue;
        }
        let f = |x: f64| y0 + (y1 - y0) * (x - x0) / (x1 - x0);
        s += 0.5 * (f(lo) + f(hi)) * (hi - lo);
    }
    s
}

/// Peak (E, sigma) and full width at half maximum, by linear interpolation of
/// the half-maximum crossings on the table's own points.
fn peak_fwhm(p: &[(f64, f64)]) -> (f64, f64, f64) {
    let (ip, &(ep, sp)) = p.iter().enumerate().max_by(|a, b| a.1 .1.total_cmp(&b.1 .1)).expect("points");
    let hm = sp / 2.0;
    let cross = |i: usize, j: usize| {
        let ((x0, y0), (x1, y1)) = (p[i], p[j]);
        x0 + (hm - y0) * (x1 - x0) / (y1 - y0)
    };
    let mut lo = ep;
    for i in (1..=ip).rev() {
        if p[i - 1].1 < hm {
            lo = cross(i - 1, i);
            break;
        }
    }
    let mut hi = ep;
    for i in ip..p.len() - 1 {
        if p[i + 1].1 < hm {
            hi = cross(i, i + 1);
            break;
        }
    }
    (ep, sp, hi - lo)
}

fn main() -> Result<(), String> {
    let out = std::env::args().nth(1).unwrap_or_else(|| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/site/science/neutronics/doppler-data.js").to_string()
    });
    let path = reference_endf("n-092_U_238.endf").ok_or("n-092_U_238.endf missing")?;
    let tape = Tape::read(std::fs::File::open(&path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let mat = *tape.materials().first().ok_or("no material")?;
    let r = reconr(&tape, &ReconrConfig { mat, tolerance: 0.001, temperature: 0.0 }).map_err(|e| e.to_string())?;
    let awr = r.material.awr;
    let mut cap = r.sections.iter().find(|s| s.mt.number() == 102).ok_or("no MT=102")?.clone();
    cap.pairs.retain(|p| (4.0..=10.0).contains(&p.0));
    let mut rows = String::new();
    for t in [0.0, 293.6, 1200.0] {
        let b = if t > 0.0 { doppler_broaden(std::slice::from_ref(&cap), awr, t).remove(0) } else { cap.clone() };
        let (ep, sp, w) = peak_fwhm(&b.pairs);
        let a = area(&b.pairs, 5.5, 8.0);
        println!("U-238 capture at {t} K: peak {sp:.1} b at {ep:.4} eV, FWHM {:.2} meV, area 5.5-8 eV {a:.3} b eV", w * 1e3);
        let _ = write!(rows, "\"{t}\":{{\"ep\":{ep},\"peak\":{sp},\"fwhm\":{w},\"area\":{a}}},");
    }
    rows.pop();
    // A pure 1/v cross section through the same kernel.
    let mut onev = cap.clone();
    onev.pairs = (0..=400).map(|i| 1e-3 * 1e5f64.powf(i as f64 / 400.0)).map(|e| (e, 10.0 / e.sqrt())).collect();
    let mut dev = String::new();
    for t in [293.6, 1200.0] {
        let b = doppler_broaden(std::slice::from_ref(&onev), awr, t).remove(0);
        let worst = b.pairs.iter().filter(|p| (0.01..=10.0).contains(&p.0)).map(|&(e, s)| (s / (10.0 / e.sqrt()) - 1.0).abs()).fold(0.0, f64::max);
        println!("1/v broadened at {t} K: worst relative change 0.01-10 eV {worst:.2e}");
        let _ = write!(dev, "\"{t}\":{worst},");
    }
    dev.pop();
    let file = format!(
        "// Numbers for the neutronics lessons, page 22 (gh:#906), written by\n\
         // crates/njoy-outram-park-fork/examples/lesson_doppler.rs (regenerate with it;\n\
         // do not edit). U-238 (ENDF/B-VIII.0, AWR {awr}) capture near its 6.674 eV\n\
         // resonance: RECONR at 0 K, BROADR's kernel at 293.6 and 1200 K; peak [b] at\n\
         // ep [eV], fwhm [eV], area 5.5-8 eV [b eV]. onev: the largest relative change\n\
         // of a pure 1/v cross section broadened the same way, 0.01-10 eV.\n\
         window.DOPPLER_DATA = {{\"awr\":{awr},\"u238\":{{{rows}}},\"onev\":{{{dev}}}}};\n"
    );
    std::fs::write(&out, file).map_err(|e| e.to_string())?;
    println!("wrote {out}");
    Ok(())
}
