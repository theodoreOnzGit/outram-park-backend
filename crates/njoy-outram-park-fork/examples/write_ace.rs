//! Build a continuous-energy ACE table from an ENDF evaluation and write it.
//!
//! Pipeline: read ENDF tape → RECONR (reconstruct pointwise cross sections) →
//! [`AceTable::from_reconr`] → write a Type-1 ASCII ACE file.
//!
//! This first ACER increment writes the cross-section core of the table (the ESZ
//! block plus the MTR/LQR/TYR/LSIG/SIG reaction blocks). The angular/energy
//! secondary-distribution blocks and the heating column are not yet filled, so
//! the file is a valid cross-section ACE table but not yet a complete transport
//! library — see the `njoy_outram_park_fork::ace` module docs.
//!
//! Run with:
//! ```bash
//! cargo run -p njoy-outram-park-fork --release --example write_ace
//! ```

use std::fs::File;

use njoy_outram_park_fork::{
    ace::{jxs, nxs, AceTable},
    endf::tape::Tape,
    reconr::{reconr, ReconrConfig},
};

fn main() {
    // H-2 is the smallest fixture: elastic + capture, no resonances.
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/resources/n-001_H_002-ENDF8.0.endf");

    let tape = Tape::read(File::open(&path).expect("open ENDF fixture")).expect("parse ENDF");
    let cfg = ReconrConfig { mat: 128, tolerance: 0.001, temperature: 0.0 };
    let result = reconr(&tape, &cfg).expect("RECONR");

    // kT = 0 → 0 K table; suffix 0 → ZAID "1002.00c".
    let ace = AceTable::from_reconr(&result, 0.0, 0);

    println!("ZAID         : {}", ace.zaid.trim());
    println!("AWR          : {:.6}", ace.awr);
    println!("NES (grid)   : {}", ace.nxs[nxs::NES]);
    println!("NTR (rxns)   : {}", ace.nxs[nxs::NTR]);
    println!("XSS length   : {}", ace.nxs[nxs::LEN_XSS]);
    println!("JXS ESZ/SIG  : {} / {}", ace.jxs[jxs::ESZ], ace.jxs[jxs::SIG]);

    let out = std::env::temp_dir().join("1002.00c.ace");
    ace.write_type1(&out).expect("write ACE");
    println!("wrote        : {}", out.display());
}
