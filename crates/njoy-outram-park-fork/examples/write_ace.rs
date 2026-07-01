//! Build a continuous-energy ACE table from an ENDF evaluation and write it.
//!
//! Pipeline: read ENDF tape → RECONR (reconstruct pointwise cross sections) →
//! parse the elastic angular distribution (MF=4) and the secondary-neutron energy
//! distributions (MF=5/MF=6) → [`AceTable::from_reconr_full`] → write a Type-1
//! ASCII ACE file.
//!
//! The table carries the ESZ cross-section block, the MTR/LQR/TYR/LSIG/SIG
//! reaction blocks, the elastic LAND/AND angular block, and the LDLW/DLW
//! secondary-energy blocks (Law 3 for discrete inelastic levels, Law 4 for the
//! continuum / (n,xn) reactions). Still outstanding: fission ν̄ (NU), non-elastic
//! angular, and heating — see the `njoy_outram_park_fork::ace` module docs.
//!
//! Run with:
//! ```bash
//! cargo run -p njoy-outram-park-fork --release --example write_ace
//! ```

use std::fs::File;

use njoy_outram_park_fork::{
    ace::{angular::parse_elastic_angular, energy::build_emissions, jxs, nxs, AceTable},
    endf::tape::Tape,
    reconr::{reconr, ReconrConfig},
};

fn main() {
    // U-235 exercises the full path: resonances, elastic angular, discrete
    // inelastic levels (Law 3), and continuum / (n,xn) emission (Law 4).
    const MAT: i32 = 9228;
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/resources/n-092_U_235-ENDF8.0.endf");

    let tape = Tape::read(File::open(&path).expect("open ENDF fixture")).expect("parse ENDF");
    let cfg = ReconrConfig { mat: MAT, tolerance: 0.001, temperature: 0.0 };
    let result = reconr(&tape, &cfg).expect("RECONR");

    // Elastic angular distribution (MF=4/MT=2).
    let angular = tape.section(MAT, 4, 2).map(|s| parse_elastic_angular(s).expect("parse MF=4"));

    // Secondary-neutron energy distributions for the producing reactions.
    let partials: Vec<(i32, f64)> =
        result.sections.iter().map(|s| (i32::from(s.mt), s.qi)).collect();
    let emissions = build_emissions(&tape, MAT, result.material.awr, &partials);

    // kT = 0 → 0 K table; suffix 0 → ZAID "92235.00c".
    let ace = AceTable::from_reconr_full(&result, 0.0, 0, angular.as_ref(), &emissions);

    println!("ZAID          : {}", ace.zaid.trim());
    println!("AWR           : {:.6}", ace.awr);
    println!("NES (grid)    : {}", ace.nxs[nxs::NES]);
    println!("NTR (rxns)    : {}", ace.nxs[nxs::NTR]);
    println!("NR (producers): {}", ace.nxs[nxs::NR]);
    println!("XSS length    : {}", ace.nxs[nxs::LEN_XSS]);
    println!("JXS SIG       : {}", ace.jxs[jxs::SIG]);
    println!("JXS LAND/AND  : {} / {}", ace.jxs[jxs::LAND], ace.jxs[jxs::AND]);
    println!("JXS LDLW/DLW  : {} / {}", ace.jxs[jxs::LDLW], ace.jxs[jxs::DLW]);
    let (law3, law4) = dlw_law_counts(&ace);
    println!("DLW laws      : {law3} × Law 3 (discrete), {law4} × Law 4 (continuum)");

    let out = std::env::temp_dir().join("92235.00c.ace");
    ace.write_type1(&out).expect("write ACE");
    println!("wrote         : {}", out.display());
}

/// Count how many DLW producers use Law 3 vs Law 4 (for the summary line).
fn dlw_law_counts(ace: &AceTable) -> (usize, usize) {
    let (nr, ldlw, dlw) = (ace.nxs[nxs::NR] as usize, ace.jxs[jxs::LDLW], ace.jxs[jxs::DLW]);
    if nr == 0 {
        return (0, 0);
    }
    let dlw0 = (dlw - 1) as usize;
    let (mut law3, mut law4) = (0, 0);
    for i in 0..nr {
        let loc = ace.xss[(ldlw - 1) as usize + i] as usize;
        match ace.xss[dlw0 + loc - 1 + 1] as i64 {
            3 => law3 += 1,
            4 => law4 += 1,
            _ => {}
        }
    }
    (law3, law4)
}
