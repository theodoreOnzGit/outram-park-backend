//! Which stage loses the U-238 20 keV discontinuity: RECONR or BROADR?
use njoy_outram_park_fork::broadr::doppler_broaden;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};

fn dump(tag: &str, pairs: &[(f64, f64)]) {
    println!("\n-- {tag} -- MT=102 points in [1.999e4, 2.45e4]:");
    let mut n = 0;
    for &(e, s) in pairs {
        if (1.99999e4..=2.45e4).contains(&e) {
            println!("   E = {e:>14.8e}   sigma = {s:>10.5} b");
            n += 1;
            if n > 14 { println!("   ..."); break; }
        }
    }
    if n == 0 { println!("   (none)"); }
}

fn main() {
    let tape = Tape::read_file(std::path::Path::new("reference-data/endf/n-092_U_238.endf")).unwrap();
    let mat = tape.materials()[0];
    let r = reconr(&tape, &ReconrConfig { mat, tolerance: 1e-3, temperature: 0.0 }).unwrap();
    let cap = r.sections.iter().find(|s| i32::from(s.mt) == 102).expect("no MT=102");
    dump("AFTER RECONR (0 K)", &cap.pairs);

    let b = doppler_broaden(&r.sections, r.material.awr, 600.0);
    let capb = b.iter().find(|s| i32::from(s.mt) == 102).expect("no MT=102");
    dump("AFTER BROADR (600 K)", &capb.pairs);
}
