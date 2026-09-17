//! Does the HTR-10 kernel material actually fission? Diagnostic for op-867c.
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::htr10::{fuel_pebble_materials, BoronReading, Htr10Nuclides};

const T: f64 = 300.15;
const NUC: Htr10Nuclides = Htr10Nuclides { u235:0,u238:1,o16:2,c_free:3,c_graphite:4,si28:5,b10:6 };

fn main() {
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");
    let ld = |n:&str,f:&str| -> Option<Nuclide> {
        let p=base.join(f); p.exists().then_some(())?;
        Nuclide::from_endf_file(&p,n,T,1.0e-3).ok() };
    let Some(sab)=ThermalScattering::from_endf_file(
        base.join("tsl-crystalline-graphite.endf").to_str().unwrap(),30,T,"c_Graphite").ok()
        else { println!("SKIP"); return };
    let Some(n)=(||Some(vec![
        ld("U235","n-092_U_235-ENDF8.0.endf")?, ld("U238","n-092_U_238.endf")?,
        ld("O16","n-008_O_016-ENDF8.0.endf")?,  ld("C12","n-006_C_012-ENDF8.0.endf")?,
        ld("C12","n-006_C_012-ENDF8.0.endf")?.with_thermal_scattering(sab),
        ld("Si28","n-014_Si_028-ENDF8.0.endf")?, ld("B10","n-005_B_010-ENDF8.0.endf")?,
    ]))() else { println!("SKIP"); return };

    let mats = fuel_pebble_materials(NUC, BoronReading::Natural, T);
    println!("fuel_pebble_materials returned {} materials", mats.len());
    for e in [0.0253_f64, 1.0, 1.0e5, 2.0e6] {
        println!("\n--- E = {e:e} eV ---");
        for (i, m) in mats.iter().enumerate() {
            let x = m.macro_xs(e, &n);
            println!("  {i}: {:<24} total {:>10.5}  fission {:>10.3e}  nu-fis {:>10.3e}",
                     m.name, x.total, x.fission, x.nu_fission);
        }
    }
}
