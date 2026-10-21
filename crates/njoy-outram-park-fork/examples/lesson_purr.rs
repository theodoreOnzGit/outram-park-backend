//! **The data of neutronics lesson page 19** (gh:#906, "PURR: make up
//! resonances that obey the statistics"): written to
//! `docs/site/science/neutronics/purr-data.js`.
//!
//! ```bash
//! cargo run --release -p njoy-outram-park-fork --example lesson_purr [out.js]
//! ```
//!
//! Every sampled number is OUTRAM PARK's PURR port, at U-238's
//! ENDF/B-VIII.0 unresolved parameters at [`E`] and [`T`], seeded with
//! NJOY's own `PURR_SEED`:
//! - **One ladder** (`purr::sample_ladder`, one iteration of
//!   `probability_table`'s loop): each spin sequence's resonances, and σ at
//!   every one of [`NSAMP`] random sample energies. The page draws the stretch
//!   [`WIN`] wide at the start of the sampled range, and bins all samples.
//! - **Many ladders**: the mean total and capture of each of [`NLADR`]
//!   ladders in turn (the same random stream `probability_table` would use),
//!   against the infinitely dilute values computed from the same parameters
//!   (`infinite_dilution_reference`, the manual's "infd").
//! - **The probability table** after all of them (`probability_table`,
//!   [`NBIN`] bins): each bin's probability and its total and capture.
//!
//! No File-3 background is added (`bkg = 0`): the numbers are the resonance
//! part plus potential scattering, the quantity "infd" is.
//!
//! Driver glue (`main`): `sample_ladder`, `probability_table` and
//! `infinite_dilution_reference` are tested in the crate.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::purr::wfun::DopplerTable;
use njoy_outram_park_fork::purr::{infinite_dilution_reference, probability_table, sample_ladder, Rng, PURR_SEED};
use njoy_outram_park_fork::reference_data::reference_endf;
use njoy_outram_park_fork::unresr::mf2::parse_lru2_ranges;
use std::fmt::Write as _;

/// Energy in U-238's unresolved range at which the ladders are built \[eV\].
const E: f64 = 50_000.0;
/// Temperature \[K\].
const T: f64 = 293.6;
/// Samples per ladder (NJOY's default).
const NSAMP: usize = 10_000;
/// Ladders (NJOY2016's ACE decks here use 64).
const NLADR: usize = 64;
/// Probability-table bins (NJOY's decks here use 20).
const NBIN: usize = 20;
/// Width of the drawn stretch of the first ladder \[eV\].
const WIN: f64 = 300.0;

fn main() -> Result<(), String> {
    let out = std::env::args().nth(1).unwrap_or_else(|| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/site/science/neutronics/purr-data.js").to_string()
    });
    let path = reference_endf("n-092_U_238.endf").ok_or("n-092_U_238.endf missing")?;
    let tape = Tape::read(std::fs::File::open(&path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let mat = *tape.materials().first().ok_or("no material")?;
    let sec = tape.section(mat, 2, 151).ok_or("no MF=2")?;
    let ranges = parse_lru2_ranges(&sec.rows[1..]).map_err(|e| e.to_string())?;
    let inf = infinite_dilution_reference(&ranges, E).map_err(|e| e.to_string())?;
    let seqs = &inf.sequences;
    let infd_total = inf.sigma_elastic_inf + inf.sigma_fission_inf + inf.sigma_capture_inf + inf.potential_scattering;
    let infd_cap = inf.sigma_capture_inf;
    println!("U-238 at {E} eV, {T} K: {} sequences, D = {:?} eV; infd total {infd_total:.4} b, capture {infd_cap:.4} b", seqs.len(), seqs.iter().map(|s| s.dbar).collect::<Vec<_>>());
    let bkg = [0.0; 4];
    let dop = DopplerTable::new();
    // The ladders, one at a time, on the random stream probability_table uses.
    let mut rng = Rng::new(PURR_SEED);
    let _warmup = rng.next(); // purr.f90:167, as UrrProbabilityTables::from_endf
    let mut means = Vec::new();
    let mut first = None;
    for k in 0..NLADR {
        let s = sample_ladder(seqs, &inf, bkg, T, NSAMP, &mut rng, &dop).map_err(|e| e.to_string())?;
        let n = s.xs.len() as f64;
        means.push((s.xs.iter().map(|x| x[0]).sum::<f64>() / n, s.xs.iter().map(|x| x[3]).sum::<f64>() / n));
        if k == 0 {
            first = Some(s);
        }
    }
    let first = first.ok_or("no ladder")?;
    let (lo, hi) = (first.energies[0], first.energies[0] + WIN);
    // The drawn stretch: resonances per sequence, and the samples in it.
    let mut res = String::new();
    for (i, lad) in first.ladders.iter().enumerate() {
        let v: Vec<String> = lad.iter().filter(|r| r.energy >= lo && r.energy <= hi).map(|r| format!("[{:.4},{:.4e},{:.4}]", r.energy, r.total_width, r.gn_frac)).collect();
        let _ = write!(res, "{{\"j\":{},\"d\":{},\"n\":{},\"r\":[{}]}},", i, seqs[i].dbar, lad.len(), v.join(","));
    }
    res.pop();
    let win: Vec<usize> = (0..first.energies.len()).filter(|&i| first.energies[i] <= hi).collect();
    let we: Vec<String> = win.iter().map(|&i| format!("{:.4}", first.energies[i])).collect();
    let wt: Vec<String> = win.iter().map(|&i| format!("{:.4}", first.xs[i][0])).collect();
    let all_t: Vec<String> = first.xs.iter().map(|x| format!("{:.4}", x[0])).collect();
    let all_c: Vec<String> = first.xs.iter().map(|x| format!("{:.4}", x[3])).collect();
    let mt: Vec<String> = means.iter().map(|m| format!("{:.5}", m.0)).collect();
    let mc: Vec<String> = means.iter().map(|m| format!("{:.5}", m.1)).collect();
    // The table after all the ladders, from a fresh stream (same as above).
    let mut rng2 = Rng::new(PURR_SEED);
    let _ = rng2.next();
    let pt = probability_table(seqs, &inf, bkg, &[1e10], &[T], NBIN, NLADR, NSAMP, &mut rng2, &dop).map_err(|e| e.to_string())?;
    let t = &pt.tables[0];
    let p: Vec<String> = t.bin_probability.iter().map(|x| format!("{x:.5e}")).collect();
    let bt: Vec<String> = t.bin_xs.iter().map(|x| format!("{:.5}", x[0])).collect();
    let bc: Vec<String> = t.bin_xs.iter().map(|x| format!("{:.5}", x[3])).collect();
    let c = pt.convergence;
    println!("first ladder: {} samples, {} in the drawn {WIN} eV; mean total over {NLADR} ladders {:.4} b ({:.2} %), capture {:.4} b; table bins {}", first.xs.len(), win.len(), c.mean_total, c.pct_std_total, c.mean_capture, t.bin_probability.len());
    let file = format!(
        "// Data for the neutronics lessons, page 19 (gh:#906), written by\n\
         // crates/njoy-outram-park-fork/examples/lesson_purr.rs (regenerate with it;\n\
         // do not edit). OUTRAM PARK's PURR port on U-238's ENDF/B-VIII.0 unresolved\n\
         // parameters at {E} eV and {T} K, seeded with NJOY's PURR_SEED, no File-3\n\
         // background. ladder: the first ladder's resonances in a {WIN} eV stretch\n\
         // ([E, Gamma_total, Gamma_n/Gamma] per sequence; energies on the ladder's own\n\
         // scale) and the samples in it; samples: sigma_total and capture at all\n\
         // {NSAMP} samples; means: each of {NLADR} ladders' mean total and capture;\n\
         // infd: the dilute values from the same parameters; table: the {NBIN}-bin\n\
         // probability table after all ladders.\n\
         window.PURR_DATA = {{\"e\":{E},\"t\":{T},\"nsamp\":{NSAMP},\"nladr\":{NLADR},\"infd\":{{\"total\":{infd_total},\"cap\":{infd_cap}}},\
         \"ladder\":{{\"lo\":{lo},\"hi\":{hi},\"seqs\":[{res}],\"e\":[{}],\"t\":[{}]}},\"samples\":{{\"t\":[{}],\"c\":[{}]}},\
         \"means\":{{\"t\":[{}],\"c\":[{}]}},\"table\":{{\"p\":[{}],\"t\":[{}],\"c\":[{}]}},\"conv\":{{\"total\":{},\"pct\":{},\"cap\":{}}}}};\n",
        we.join(","), wt.join(","), all_t.join(","), all_c.join(","), mt.join(","), mc.join(","), p.join(","), bt.join(","), bc.join(","), c.mean_total, c.pct_std_total, c.mean_capture
    );
    std::fs::write(&out, file).map_err(|e| e.to_string())?;
    println!("wrote {out}");
    Ok(())
}
