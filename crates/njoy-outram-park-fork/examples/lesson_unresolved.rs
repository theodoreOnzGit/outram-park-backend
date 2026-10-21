//! **The data of neutronics lesson page 18** (gh:#906, "The double whammy":
//! the unresolved resonance range and its smeared, infinitely dilute
//! average): written to `docs/site/science/neutronics/unresolved-data.js`.
//!
//! ```bash
//! cargo run --release -p njoy-outram-park-fork --example lesson_unresolved [out.js]
//! ```
//!
//! For U-238 (`LSSF = 1`) and U-234 (`LSSF = 0`), ENDF/B-VIII.0:
//! - **The unresolved range's average parameters**, read with UNRESR's port
//!   of `rdunf2` (`unresr::mf2::parse_lru2_ranges`): each spin sequence's
//!   (l, J) and its tabulated mean spacing D(E), as the tape gives them.
//! - **Capture, reconstructed by the RECONR port** at 0 K and NJOY's
//!   tolerance 0.001: the resolved forest (thinned by
//!   `lesson_common/thin.rs`) and, in the unresolved range, the infinitely
//!   dilute average. For `LSSF = 1` that average is the evaluation's own
//!   File 3; for `LSSF = 0` RECONR computes it from the parameters.
//! - **A running average of the forest** ("smearing"): at each of [`NAVG`]
//!   energies, the plain average of σ over the window E/(1 + W) .. E(1 + W),
//!   integrated exactly on the reconstructed (lin-lin) points.
//!
//! Driver glue (`main`): the RECONR port and the MF=2 reader it calls are
//! tested in the crate; the lesson's check (`check-physics.js`) reads the
//! file this writes.

use petir::mathf::RealMath;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::reference_endf;
use njoy_outram_park_fork::unresr::mf2::{parse_lru2_ranges, UnresolvedCase};
use std::fmt::Write as _;

#[path = "lesson_common/thin.rs"]
mod thin;
use thin::{thin, BINS};

/// Half-width of the smearing window, as a fraction of E.
const W: f64 = 0.05;
/// Energies of the running average, log-spaced from 100 eV to 200 keV.
const NAVG: usize = 240;

/// The exact average of a lin-lin tabulated σ over [a, b].
fn average(p: &[(f64, f64)], a: f64, b: f64) -> f64 {
    let (mut s, mut w) = (0.0, 0.0);
    for q in p.windows(2) {
        let ((x0, y0), (x1, y1)) = (q[0], q[1]);
        let (lo, hi) = (x0.max(a), x1.min(b));
        if hi <= lo || x1 <= x0 {
            continue;
        }
        let f = |x: f64| y0 + (y1 - y0) * (x - x0) / (x1 - x0);
        s += 0.5 * (f(lo) + f(hi)) * (hi - lo);
        w += hi - lo;
    }
    if w > 0.0 {
        s / w
    } else {
        0.0
    }
}

fn nums(v: impl Iterator<Item = f64>, prec: usize) -> String {
    v.map(|x| format!("{x:.prec$e}")).collect::<Vec<_>>().join(",")
}

fn main() -> Result<(), String> {
    let out = std::env::args().nth(1).unwrap_or_else(|| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/site/science/neutronics/unresolved-data.js").to_string()
    });
    let mut js = String::new();
    for (name, file) in [("U-238", "n-092_U_238.endf"), ("U-234", "n-092_U_234-ENDF8.0.endf")] {
        let path = reference_endf(file).ok_or_else(|| format!("{file}: not in reference-data/endf"))?;
        let tape = Tape::read(std::fs::File::open(&path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let mat = *tape.materials().first().ok_or("no material")?;
        let sec = tape.section(mat, 2, 151).ok_or("no MF=2")?;
        let ranges = parse_lru2_ranges(&sec.rows[1..]).map_err(|e| e.to_string())?;
        let rg = ranges.first().ok_or("no unresolved range")?;
        println!("{name}: MAT {mat}, unresolved {} .. {} eV, LSSF {}", rg.el, rg.eh, rg.lssf);
        let mut seqs = String::new();
        match &rg.case_ {
            UnresolvedCase::CaseC { l_states, spi, .. } => {
                for ls in l_states {
                    for j in &ls.j_states {
                        let pts: Vec<String> = j.points.iter().map(|p| format!("[{},{}]", p.e, p.d)).collect();
                        println!("  l={} J={} INT={} D: {}", ls.l, j.aj, j.int_, pts.join(" "));
                        let _ = write!(seqs, "{{\"l\":{},\"j\":{},\"int\":{},\"d\":[{}]}},", ls.l, j.aj, j.int_, pts.join(","));
                    }
                }
                println!("  SPI {spi}");
            }
            _ => return Err(format!("{name}: expected LRF=2 (Case C) unresolved parameters")),
        }
        seqs.pop();
        let r = reconr(&tape, &ReconrConfig { mat, tolerance: 0.001, temperature: 0.0 }).map_err(|e| e.to_string())?;
        let cap = r.sections.iter().find(|s| s.mt.number() == 102).ok_or("no MT=102")?;
        let th = thin(&cap.pairs);
        let ea: Vec<f64> = (0..NAVG).map(|i| 1e2 * (2e5f64 / 1e2).r_powf(i as f64 / (NAVG - 1) as f64)).collect();
        let av: Vec<f64> = ea.iter().map(|&e| average(&cap.pairs, e / (1.0 + W), e * (1.0 + W))).collect();
        println!("  MT 102: {} points, {} kept; average at 10 keV {:.4} b, at 30 keV {:.4} b", cap.pairs.len(), th.len(), average(&cap.pairs, 1e4 / (1.0 + W), 1e4 * (1.0 + W)), average(&cap.pairs, 3e4 / (1.0 + W), 3e4 * (1.0 + W)));
        let _ = write!(
            js,
            "\"{name}\":{{\"mat\":{mat},\"el\":{},\"eh\":{},\"lssf\":{},\"seqs\":[{seqs}],\"cap\":{{\"n\":{},\"e\":[{}],\"s\":[{}]}},\"avg\":{{\"e\":[{}],\"s\":[{}]}}}},",
            rg.el,
            rg.eh,
            rg.lssf,
            cap.pairs.len(),
            nums(th.iter().map(|p| p.0), 6),
            nums(th.iter().map(|p| p.1), 5),
            nums(ea.iter().copied(), 5),
            nums(av.iter().copied(), 5)
        );
    }
    js.pop();
    let file = format!(
        "// Data for the neutronics lessons, page 18 (gh:#906), written by\n\
         // crates/njoy-outram-park-fork/examples/lesson_unresolved.rs (regenerate with\n\
         // it; do not edit). For U-238 (LSSF 1) and U-234 (LSSF 0), ENDF/B-VIII.0: the\n\
         // unresolved range [el, eh] eV, each spin sequence's tabulated mean spacing\n\
         // D(E) [[E eV, D eV], ...] with its INT, capture (MT 102) from OUTRAM PARK's\n\
         // RECONR port at 0 K (thinned to the highest and lowest point of {BINS} log-E\n\
         // steps), and avg: the plain average of that capture over E/(1+{W})..E(1+{W}).\n\
         window.UNRESOLVED_DATA = {{{js}}};\n"
    );
    std::fs::write(&out, file).map_err(|e| e.to_string())?;
    println!("wrote {out}");
    Ok(())
}
