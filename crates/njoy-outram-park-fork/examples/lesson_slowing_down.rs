//! **The data of neutronics lesson page 17** (gh:#906, "Why are
//! cross-section graphs logarithmic?"): written to
//! `docs/site/science/neutronics/slowing-data.js`.
//!
//! ```bash
//! cargo run --release -p njoy-outram-park-fork --example lesson_slowing_down [out.js]
//! ```
//!
//! - **Mass ratios.** Each target's AWR, read from MF=1 of its ENDF/B-VIII.0
//!   evaluation by this crate's RECONR port (`ReconrResult::material`). The
//!   page computes α, ξ and the collision counts from these, not from integer
//!   mass numbers. H-1's comes from the only local H-1 tape, a **beta** of
//!   ENDF/B-VIII.0 (gh:#922), and is marked as such.
//! - **Overlay curves**, reconstructed by the RECONR port at 0 K and NJOY's
//!   tolerance 0.001: U-238 capture (MT 102), C-12 elastic (MT 2) and B-10
//!   (n,α) (MT 107). Thinned for a phone by `lesson_common/thin.rs`.
//! - **U-238's resonances.** Every positive resonance energy in the resolved
//!   range, per l (read with `errorr`'s port of `rdumrd2`,
//!   `Mf2Resonances::read`), and the limits of the resolved (`LRU = 1`) and
//!   unresolved (`LRU = 2`) ranges.
//!
//! Driver glue (`main`): the RECONR port and the MF=2 reader it calls are
//! tested in the crate; the lesson's check (`check-physics.js`) reads the
//! file this writes.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::errorr::resprx::mf2::Mf2Resonances;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig, ReconrResult};
use njoy_outram_park_fork::reference_data::reference_endf;
use std::fmt::Write as _;

#[path = "lesson_common/thin.rs"]
mod thin;
use thin::{thin, BINS};

fn run(file: &str) -> Result<(Tape, i32, ReconrResult), String> {
    let path = reference_endf(file).ok_or_else(|| format!("{file}: not in reference-data/endf"))?;
    let tape = Tape::read(std::fs::File::open(&path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let mat = *tape.materials().first().ok_or("no material")?;
    let r = reconr(&tape, &ReconrConfig { mat, tolerance: 0.001, temperature: 0.0 }).map_err(|e| e.to_string())?;
    Ok((tape, mat, r))
}

fn curve(r: &ReconrResult, mt: i32) -> Result<String, String> {
    let s = r.sections.iter().find(|s| s.mt.number() == mt).ok_or(format!("no MT={mt}"))?;
    let th = thin(&s.pairs);
    let e: Vec<String> = th.iter().map(|p| format!("{:.6e}", p.0)).collect();
    let v: Vec<String> = th.iter().map(|p| format!("{:.5e}", p.1)).collect();
    println!("  MT {mt}: {} reconstructed points, {} kept", s.pairs.len(), th.len());
    Ok(format!("{{\"n\":{},\"e\":[{}],\"s\":[{}]}}", s.pairs.len(), e.join(","), v.join(",")))
}

fn main() -> Result<(), String> {
    let out = std::env::args().nth(1).unwrap_or_else(|| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/site/science/neutronics/slowing-data.js").to_string()
    });
    let mut awr = String::new();
    let mut curves = String::new();
    let targets = [
        ("H-1", "n-001_H_001-ENDF8.0-Beta6.endf", None),
        ("He-4", "n-002_He_004.endf", None),
        ("B-10", "n-005_B_010-ENDF8.0.endf", Some(("B-10 (n,alpha)", 107))),
        ("C-12", "n-006_C_012-ENDF8.0.endf", Some(("C-12 elastic", 2))),
        ("U-238", "n-092_U_238.endf", Some(("U-238 capture", 102))),
    ];
    let mut u238 = None;
    for (name, file, cv) in targets {
        let (tape, mat, r) = run(file)?;
        println!("{name}: MAT {mat}, AWR {}", r.material.awr);
        let _ = write!(awr, "\"{name}\":{},", r.material.awr);
        if let Some((label, mt)) = cv {
            let _ = write!(curves, "\"{label}\":{},", curve(&r, mt)?);
        }
        if name == "U-238" {
            u238 = Some((tape, mat));
        }
    }
    awr.pop();
    curves.pop();
    // U-238's resolved resonances and range limits.
    let (tape, mat) = u238.ok_or("no U-238")?;
    let m = Mf2Resonances::read(&tape, mat).map_err(|e| e.to_string())?;
    let mut ranges = String::new();
    let mut levels = String::new();
    for rg in &m.ranges {
        let (el, eh, lru, lrf) = (rg.range.c1, rg.range.c2, rg.range.l1, rg.range.l2);
        println!("U-238 range LRU={lru} LRF={lrf}: {el:e} to {eh:e} eV");
        let _ = write!(ranges, "{{\"lru\":{lru},\"lrf\":{lrf},\"el\":{el},\"eh\":{eh}}},");
        if lru == 1 {
            for l in &rg.lists {
                let mut er: Vec<f64> = l.data.chunks(6).map(|c| c[0]).filter(|&e| e > 0.0).collect();
                er.sort_by(f64::total_cmp);
                println!("  l={}: {} positive levels, lowest {:?}", l.head.l1, er.len(), &er[..3.min(er.len())]);
                let v: Vec<String> = er.iter().map(|e| format!("{e}")).collect();
                let _ = write!(levels, "\"{}\":[{}],", l.head.l1, v.join(","));
            }
        }
    }
    ranges.pop();
    levels.pop();
    let file = format!(
        "// Data for the neutronics lessons, page 17 (gh:#906), written by\n\
         // crates/njoy-outram-park-fork/examples/lesson_slowing_down.rs (regenerate\n\
         // with it; do not edit). awr: each target's mass ratio from MF=1 of its\n\
         // ENDF/B-VIII.0 tape (H-1: the local tape is a beta, gh:#922). curves:\n\
         // reconstructed by OUTRAM PARK's RECONR port at 0 K, tolerance 0.001,\n\
         // thinned to the highest and lowest point of {BINS} log-E steps (e [eV],\n\
         // s [b], n: points before thinning). u238: its MF=2 energy ranges and every\n\
         // positive resolved resonance energy [eV] per l.\n\
         window.SLOWING_DATA = {{\"awr\":{{{awr}}},\"awrBeta\":[\"H-1\"],\"curves\":{{{curves}}},\"u238\":{{\"ranges\":[{ranges}],\"levels\":{{{levels}}}}}}};\n"
    );
    std::fs::write(&out, file).map_err(|e| e.to_string())?;
    println!("wrote {out}");
    Ok(())
}
