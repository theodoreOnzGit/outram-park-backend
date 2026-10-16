//! **The reaction channels of neutronics lesson pages 10 and 11** (gh:#906,
//! "What can happen when a neutron meets a nucleus?" and "Fission: fissile
//! vs fissionable"): every number those pages quote, read here from the
//! ENDF/B-VIII.0 tapes in `reference-data/endf/` and written to
//! `docs/site/science/neutronics/reaction-data.js`.
//!
//! ```bash
//! cargo run --release -p njoy-outram-park-fork --example lesson_reactions [out.js]
//! ```
//!
//! - **Q-values**: for C-12, B-10, U-235 and U-238, each channel's MF=3 TAB1
//!   head (`C1 = QM`, `C2 = QI`, ENDF-102 §3.2) for MT 2, 51 (the first
//!   excited level), 4, 16, 18, 102, 103 and 107, where the evaluation has
//!   them; and the AWR from the MF=3 HEAD.
//! - **Fission cross sections** of U-235 and U-238 (MT=18), reconstructed by
//!   this crate's RECONR port at 0 K, tolerance 0.001, thinned as page 9's
//!   (`lesson_total_xs`): the highest and lowest point of each of 900 log-E
//!   steps, nothing smoothed.
//! - **U-238's fission onset**, defined on the data rather than quoted: the
//!   energy above which σ_f STAYS at or above 10 % of its value at 2 MeV (the
//!   point after its last value below that, up to 2 MeV). "First reaches"
//!   would pick a fission resonance at about 1 keV.
//!
//! Driver glue (`main`): the ENDF reader and the RECONR port it calls are
//! tested in the crate; `docs/site/science/neutronics/check-physics.js`
//! checks the file this writes.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::reference_endf;
use std::fmt::Write as _;

const BINS: usize = 900;
const E_LO: f64 = 1.0e-5;
const E_HI: f64 = 2.0e7;
const MTS: [(i32, &str); 8] = [(2, "elastic"), (51, "first inelastic level"), (4, "inelastic, total"), (16, "(n,2n)"), (18, "fission"), (102, "(n,gamma)"), (103, "(n,p)"), (107, "(n,alpha)")];
const NUCLIDES: [(&str, &str); 4] = [("C-12", "n-006_C_012.endf"), ("B-10", "n-005_B_010.endf"), ("U-235", "n-092_U_235.endf"), ("U-238", "n-092_U_238.endf")];

/// The highest and lowest point of each log-E step, in energy order (as
/// `lesson_total_xs`).
fn thin(pairs: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let (l0, l1) = (E_LO.log10(), E_HI.log10());
    let mut best: Vec<Option<(usize, usize)>> = vec![None; BINS];
    for (i, &(e, s)) in pairs.iter().enumerate() {
        if !(E_LO..=E_HI).contains(&e) || s <= 0.0 {
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

/// σ at `e` by lin-lin interpolation on a sorted grid.
fn at(pairs: &[(f64, f64)], e: f64) -> f64 {
    let k = pairs.partition_point(|p| p.0 < e).clamp(1, pairs.len() - 1);
    let ((e0, s0), (e1, s1)) = (pairs[k - 1], pairs[k]);
    s0 + (s1 - s0) * (e - e0) / (e1 - e0)
}

fn main() -> Result<(), String> {
    let out = std::env::args().nth(1).unwrap_or_else(|| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/site/science/neutronics/reaction-data.js").to_string()
    });
    let mut js = String::new();
    let mut header = String::new();
    for (name, file) in NUCLIDES {
        let path = reference_endf(file).ok_or_else(|| format!("{file}: not in reference-data/endf"))?;
        let tape = Tape::read(std::fs::File::open(&path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let mat = *tape.materials().first().ok_or("no material")?;
        let mut awr = 0.0;
        let mut ch = Vec::new();
        let mut line = format!("//   {name} (MAT {mat}):");
        for (mt, what) in MTS {
            let Some(sec) = tape.section(mat, 3, mt) else { continue };
            if sec.rows.len() < 2 {
                continue;
            }
            awr = sec.rows[0][1];
            let (qm, qi) = (sec.rows[1][0], sec.rows[1][1]);
            ch.push(format!("{{\"mt\":{mt},\"what\":\"{what}\",\"qm\":{qm:.6e},\"qi\":{qi:.6e}}}"));
            let _ = write!(line, " MT{mt} QM {:.4} MeV QI {:.4} MeV;", qm / 1e6, qi / 1e6);
        }
        println!("{line}");
        let _ = writeln!(header, "{}", line.trim_end_matches(';'));
        let mut fission = String::new();
        if name.starts_with('U') {
            let r = reconr(&tape, &ReconrConfig { mat, tolerance: 0.001, temperature: 0.0 }).map_err(|e| e.to_string())?;
            let sf = r.sections.iter().find(|s| s.mt.number() == 18).ok_or("no MT=18")?;
            let th = thin(&sf.pairs);
            let at2 = at(&sf.pairs, 2.0e6);
            let below = sf.pairs.iter().rposition(|p| p.0 < 2.0e6 && p.1 < 0.1 * at2);
            let onset = below.and_then(|k| sf.pairs.get(k + 1)).map_or(sf.pairs[0].0, |p| p.0);
            let thermal = at(&sf.pairs, 0.0253);
            println!("  {name} sigma_f: 0.0253 eV {thermal:.4e} b, 2 MeV {at2:.4e} b, stays above 10 % of the 2 MeV value from {onset:.4e} eV; {} points, {} kept", sf.pairs.len(), th.len());
            let _ = writeln!(header, "//   {name} sigma_f: {thermal:.4} b at 0.0253 eV, {at2:.4} b at 2 MeV, stays above 10 % of that from {:.4} MeV", onset / 1e6);
            let e: Vec<String> = th.iter().map(|p| format!("{:.6e}", p.0)).collect();
            let s: Vec<String> = th.iter().map(|p| format!("{:.5e}", p.1)).collect();
            fission = format!(",\"sf\":{{\"thermal\":{thermal:.6e},\"at2MeV\":{at2:.6e},\"onset\":{onset:.6e},\"e\":[{}],\"s\":[{}]}}", e.join(","), s.join(","));
        }
        let _ = write!(js, "\"{name}\":{{\"mat\":{mat},\"awr\":{awr:.6},\"channels\":[{}]{fission}}},", ch.join(","));
    }
    js.pop();
    let file = format!(
        "// Reaction channels for the neutronics lessons, pages 10 and 11 (gh:#906):\n\
         // ENDF/B-VIII.0 tapes in reference-data/endf/, read by\n\
         // crates/njoy-outram-park-fork/examples/lesson_reactions.rs (regenerate with\n\
         // it; do not edit). qm, qi: each channel's MF=3 QM and QI [eV] (ENDF-102\n\
         // §3.2); awr: the MF=3 HEAD's mass ratio. sf: MT=18 reconstructed by OUTRAM\n\
         // PARK's RECONR port at 0 K, tolerance 0.001, thinned to the highest and\n\
         // lowest point of {BINS} log-E steps. Read from the tapes:\n\
         {header}\
         window.REACTION_DATA = {{{js}}};\n"
    );
    std::fs::write(&out, file).map_err(|e| e.to_string())?;
    println!("wrote {out}");
    Ok(())
}
