//! ACER for an incident charged particle (gh:#1485): function-level gates
//! against NJOY2016.80's reference tapes (`tests/14`). Skips when the
//! upstream clone is absent (`NJOY2016_ROOT`, else `upstream_source/NJOY2016`).

use njoy_outram_park_fork::acer::{charged, read::parse_type1};
use njoy_outram_park_fork::endf::tape::Tape;
use std::path::PathBuf;

fn root() -> PathBuf {
    std::env::var("NJOY2016_ROOT").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

/// `unionx`'s charged-particle union grid (deck 14, p + N-14) against the ESZ
/// energy column of NJOY2016's reference ACE (`referenceTape31`), MeV at 9
/// figures. Pass criterion: same length, every energy equal after `sigfig(E/1e6, 9)`.
#[test]
fn deck14_union_grid_matches_reference_esz() {
    let r = root();
    let (Ok(tape), Ok(ace)) = (Tape::read_file(&r.join("tests/resources/epn14")), std::fs::read_to_string(r.join("tests/14/referenceTape31"))) else {
        eprintln!("skip: upstream not present");
        return;
    };
    assert_eq!(charged::incident_particle(&tape, 725), 1001);
    let grid = charged::union_grid(&tape, 725).unwrap();
    let t = parse_type1(&ace).unwrap();
    let nes = t.nxs[2] as usize;
    let ours: Vec<f64> = grid.iter().map(|&e| njoy_outram_park_fork::mixr::mix::sigfig(e / 1.0e6, 9, 0)).collect();
    let refe = &t.xss[..nes];
    let bad = ours.iter().zip(refe).filter(|(a, b)| ((*a - *b) / *b).abs() > 1e-11).count();
    println!("grid {} vs ESZ {}; differing {bad}", ours.len(), nes);
    assert_eq!(ours.len(), nes);
    assert_eq!(bad, 0);
}
