//! Dump this crate's unresolved-range MT=18 cross section at full precision,
//! at the energies where NJOY2016 interpolates rather than evaluates.
//!
//! The comparison arm of the study in
//! `verification_and_validation/urr_interpolation_study/`. It exists because
//! the analysis scripts must not compare against values rounded to the seven
//! figures NJOY prints -- doing so reports several deviations as exactly zero
//! when they are 4e-10 to 4e-7, which flatters this crate and is not true.
//!
//! ```bash
//! cargo run --release -p njoy-outram-park-fork --example urr_kernel_dump \
//!   > crates/njoy-outram-park-fork/verification_and_validation/urr_interpolation_study/ours_mt18.csv
//! ```

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_file;
use njoy_outram_park_fork::unresr::mf2::parse_lru2_ranges;
use njoy_outram_park_fork::unresr::unresolved_cross_sections;
use njoy_outram_park_fork::unresr::wfun::WTable;

/// Infinite dilution -- `genunr`'s `big = 1.e10_kr` (`reconr.f90:1644`).
const SIG0: f64 = 1.0e10;

fn main() {
    let Some(ep) = reference_file("endf", "n-092_U_234-ENDF8.0.endf") else {
        eprintln!("reference-data/endf/n-092_U_234-ENDF8.0.endf absent");
        std::process::exit(1);
    };
    let tape = Tape::read_file(&ep).expect("evaluation parses");
    let sec = tape.section(9225, 2, 151).expect("MF=2/151");
    let ranges = parse_lru2_ranges(&sec.rows[1..]).expect("LRU=2 ranges");
    let range = ranges.first().expect("one unresolved range");
    let table = WTable::new();

    println!("# U-234 MAT 9225, MF=3 MT=18, infinitely dilute (sig0=1e10), 0 K");
    println!("# njoy-outram-park-fork, unresr::unresolved_cross_sections");
    println!("energy_ev,sigma_b");
    for e in [
        7.00000e3_f64,
        4.368748e4,
        4.368749e4,
        4.51800e4,
        5.25000e4,
        7.00000e4,
        8.00000e4,
        9.00000e4,
    ] {
        let out = unresolved_cross_sections(
            std::slice::from_ref(range),
            e,
            0.0,
            &[SIG0],
            [0.0; 4],
            &table,
        )
        .expect("kernel evaluates");
        println!("{:.10e},{:.12e}", e, out[0][2]);
    }
}
