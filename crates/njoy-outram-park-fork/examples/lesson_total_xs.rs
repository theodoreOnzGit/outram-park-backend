//! **The total cross sections of the neutronics lesson page 9** (gh:#906,
//! "Why are nuclear energy levels so complicated?"): He-4, Mn-55 and U-238,
//! σ_tot(E) from their ENDF/B-VIII.0 evaluations, reconstructed by this
//! crate's own RECONR port at 0 K and NJOY's tolerance 0.001, and written to
//! `docs/site/science/neutronics/xs-data.js`.
//!
//! ```bash
//! cargo run --release -p njoy-outram-park-fork --example lesson_total_xs [out.js]
//! ```
//!
//! - **Thinning, stated on the page.** A reconstructed curve has up to ~10⁵
//!   points, too many for a phone. The energy axis 10⁻⁵ eV to 20 MeV is cut
//!   into [`BINS`] equal steps in log E, and in each step the point with the
//!   highest and the point with the lowest σ are kept, in energy order. Every
//!   peak and trough survives to the plot's resolution; nothing is smoothed
//!   or averaged.
//! - **Numbers the page quotes are printed here, from the tape**: each MF=2
//!   energy range (`LRU`, `LRF`, limits) and, for a resolved range, its
//!   resonances per orbital angular momentum l (read with `errorr`'s port of
//!   `rdumrd2`, `Mf2Resonances::read`); and He-4's broad maximum between 0.3
//!   and 3 MeV, which the page calls its resonance near 1 MeV.
//! - H-1 is not included: the maintainer dropped it (gh:#906, 2026-10-16)
//!   because the only local tape is a beta of ENDF/B-VIII.0.
//!
//! Driver glue (`main`): the RECONR port and the MF=2 reader it calls are
//! tested in the crate; the lesson's own check (`docs/site/check-physics.js`)
//! reads the file this writes.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::errorr::resprx::mf2::Mf2Resonances;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::reference_endf;
use std::fmt::Write as _;

/// Steps in log E across 1e-5 eV .. 2e7 eV.
const BINS: usize = 900;
const E_LO: f64 = 1.0e-5;
const E_HI: f64 = 2.0e7;

const NUCLIDES: [(&str, &str); 3] = [("He-4", "n-002_He_004.endf"), ("Mn-55", "n-025_Mn_055.endf"), ("U-238", "n-092_U_238.endf")];

/// Keep the highest and lowest point of each log-E step, in energy order.
fn thin(pairs: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let (l0, l1) = (E_LO.log10(), E_HI.log10());
    let mut best: Vec<Option<(usize, usize)>> = vec![None; BINS];
    for (i, &(e, s)) in pairs.iter().enumerate() {
        if !(E_LO..=E_HI).contains(&e) {
            continue;
        }
        let b = (((e.log10() - l0) / (l1 - l0)) * BINS as f64).floor().clamp(0.0, (BINS - 1) as f64) as usize;
        best[b] = Some(match best[b] {
            None => (i, i),
            Some((lo, hi)) => (if s < pairs[lo].1 { i } else { lo }, if s > pairs[hi].1 { i } else { hi }),
        });
    }
    let mut keep: Vec<usize> = best.into_iter().flatten().flat_map(|(a, b)| [a, b]).collect();
    keep.sort_unstable();
    keep.dedup();
    keep.into_iter().map(|i| pairs[i]).collect()
}

fn main() -> Result<(), String> {
    let out = std::env::args().nth(1).unwrap_or_else(|| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/site/science/neutronics/xs-data.js").to_string()
    });
    let mut js = String::new();
    let mut header = String::new();
    for (name, file) in NUCLIDES {
        let path = reference_endf(file).ok_or_else(|| format!("{file}: not in reference-data/endf"))?;
        let tape = Tape::read(std::fs::File::open(&path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let mat = *tape.materials().first().ok_or("no material")?;
        let t = std::time::Instant::now();
        let r = reconr(&tape, &ReconrConfig { mat, tolerance: 0.001, temperature: 0.0 }).map_err(|e| e.to_string())?;
        let total = r.sections.iter().find(|s| s.mt.number() == 1).ok_or("no MT=1")?;
        let thinned = thin(&total.pairs);
        let secs = t.elapsed().as_secs_f64();
        let mut mf2 = String::new();
        match Mf2Resonances::read(&tape, mat) {
            Ok(m) => {
                for rg in &m.ranges {
                    let (el, eh, lru, lrf) = (rg.range.c1, rg.range.c2, rg.range.l1, rg.range.l2);
                    let _ = write!(mf2, "LRU={lru} LRF={lrf} {el:.4e}-{eh:.4e} eV");
                    if lru == 1 {
                        let per_l: Vec<String> = rg.lists.iter().map(|l| format!("l={}: {}", l.head.l1, l.head.n2)).collect();
                        let _ = write!(mf2, " ({})", per_l.join(", "));
                    }
                    mf2.push_str("; ");
                }
            }
            // `rdumrd2` has no coding for LRU = 0: the evaluation gives no
            // resonance parameters, only its scattering radius, and its
            // cross sections are all in MF=3.
            Err(e) if e.to_string().contains("lru=0") => mf2 = "no resonance parameters (LRU=0): all of sigma(E) is in MF=3".to_string(),
            Err(e) => mf2 = format!("no MF=2 resonances read ({e})"),
        }
        println!("{name}: MAT {mat}, {} reconstructed points, {} kept, {secs:.1} s; MF=2: {mf2}", total.pairs.len(), thinned.len());
        if name == "He-4" {
            let peak = total.pairs.iter().filter(|p| (3.0e5..3.0e6).contains(&p.0)).fold((0.0, 0.0), |m, &p| if p.1 > m.1 { p } else { m });
            println!("  He-4 broad maximum between 0.3 and 3 MeV: {:.4e} b at {:.4e} eV", peak.1, peak.0);
            let _ = write!(header, "//   He-4 broad maximum between 0.3 and 3 MeV: {:.3} b at {:.3} MeV\n", peak.1, peak.0 / 1e6);
        }
        let _ = write!(header, "//   {name}: MAT {mat}, {} points reconstructed, {} kept; MF=2: {mf2}\n", total.pairs.len(), thinned.len());
        let e: Vec<String> = thinned.iter().map(|p| format!("{:.6e}", p.0)).collect();
        let s: Vec<String> = thinned.iter().map(|p| format!("{:.5e}", p.1)).collect();
        let mf2_js = mf2.trim_end_matches("; ").replace('"', "'");
        let _ = write!(js, "\"{name}\":{{\"mat\":{mat},\"n\":{},\"mf2\":\"{mf2_js}\",\"e\":[{}],\"s\":[{}]}},", total.pairs.len(), e.join(","), s.join(","));
    }
    js.pop();
    let file = format!(
        "// Total cross sections for the neutronics lessons, page 9 (gh:#906): He-4,\n\
         // Mn-55 and U-238, MT=1 of their ENDF/B-VIII.0 evaluations in\n\
         // reference-data/endf/, reconstructed by OUTRAM PARK's RECONR port at 0 K,\n\
         // tolerance 0.001, and written by\n\
         // crates/njoy-outram-park-fork/examples/lesson_total_xs.rs (regenerate with\n\
         // it; do not edit). Thinned for a phone: 1e-5 eV to 20 MeV in {BINS} equal\n\
         // steps of log E, keeping the highest and lowest point of each step, so\n\
         // peaks and troughs survive and nothing is smoothed. Read from the tapes:\n\
         {header}\
         // e: energy [eV]; s: sigma_total [b]; n: points before thinning.\n\
         window.XS_DATA = {{{js}}};\n"
    );
    std::fs::write(&out, file).map_err(|e| e.to_string())?;
    println!("wrote {out}");
    Ok(())
}
