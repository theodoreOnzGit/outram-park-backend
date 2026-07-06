//! PROBE (temporary): confirm RECONR + BROADR run on U-238 capture.

use njoy_outram_park_fork::interface::NuclearDataLibrary;
use std::path::PathBuf;
use uom::si::area::barn;
use uom::si::energy::electronvolt;
use uom::si::f64::{Energy, ThermodynamicTemperature};
use uom::si::thermodynamic_temperature::kelvin;

const U238_MAT: i32 = 9237;

fn endf_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/n-092_U_238.endf")
}

#[test]
fn probe_reconr_broadr_capture() {
    let ev = |e: f64| Energy::new::<electronvolt>(e);
    let t = std::time::Instant::now();
    let lib = NuclearDataLibrary::from_file(endf_path(), U238_MAT)
        .expect("load U238 ENDF")
        .reconstruct(0.001)
        .expect("RECONR U238");
    eprintln!("reconstruct 0K took {:?}", t.elapsed());

    for e in [0.0253_f64, 6.673, 20.87, 36.68, 66.03] {
        eprintln!("cap 0K @ {e:>8} eV = {:.4} b", lib.capture_xs(ev(e)).get::<barn>());
    }

    let t = std::time::Instant::now();
    let lib900 = lib
        .broaden(ThermodynamicTemperature::new::<kelvin>(900.0))
        .expect("BROADR 900K");
    eprintln!("broaden 900K took {:?}", t.elapsed());
    for e in [6.673_f64, 20.87] {
        eprintln!("cap 900K @ {e:>8} eV = {:.4} b", lib900.capture_xs(ev(e)).get::<barn>());
    }
}
