//! **Where are the neutrons being absorbed?** Macroscopic Σ per material.
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::htr10::{fuel_pebble_materials, BoronReading, Htr10Nuclides};
use nee_soon::htr10_rmc::reflector::zone_composition;

const B10_OF_NATURAL: f64 = 0.199;

fn main() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");
    let l = |n: &str, f: &str| Nuclide::from_endf_file(&dir.join(f), n, 293.6, 1.0e-3).ok();
    let sab = ThermalScattering::from_endf_file(
        dir.join("tsl-crystalline-graphite.endf").to_str().unwrap(), 30, 293.6, "c_Graphite").ok();
    let (Some(u5), Some(u8), Some(o), Some(cf), Some(cg), Some(si), Some(b)) = (
        l("U235","n-092_U_235-ENDF8.0.endf"), l("U238","n-092_U_238.endf"),
        l("O16","n-008_O_016-ENDF8.0.endf"), l("C12","n-006_C_012-ENDF8.0.endf"),
        l("C12","n-006_C_012-ENDF8.0.endf"), l("Si28","n-014_Si_028-ENDF8.0.endf"),
        l("B10","n-005_B_010-ENDF8.0.endf")) else { println!("SKIP: no endf dir"); return; };
    let nucs = vec![u5, u8, o, cf, cg.with_thermal_scattering(sab.unwrap()), si, b];
    let idx = Htr10Nuclides { u235: 0, u238: 1, o16: 2, c_free: 3, c_graphite: 4, si28: 5, b10: 6 };
    let mut mats = fuel_pebble_materials(idx, BoronReading::Natural, 293.6);
    mats.truncate(6);
    mats.push(Material { id: 70, name: "helium".into(), components: vec![], temperature: 293.6 });
    let z = zone_composition(22).expect("zone 22");
    mats.push(Material { id: 71, name: "reflector".into(), components: vec![
        NuclideComponent { nuclide_idx: idx.c_graphite, atom_density: z.carbon },
        NuclideComponent { nuclide_idx: idx.b10, atom_density: z.natural_boron * B10_OF_NATURAL },
    ], temperature: 293.6 });
    println!("zone 22: carbon {:.6e}  natural_boron {:.6e}  -> B10 {:.6e}",
             z.carbon, z.natural_boron, z.natural_boron * B10_OF_NATURAL);
    println!("{:<26} {:>12} {:>12} {:>12} {:>9}", "material @ 0.0253 eV", "Sigma_t", "Sigma_a", "nuSigma_f", "a/t");
    for (i, m) in mats.iter().enumerate() {
        let st = m.macro_xs_total(0.0253, &nucs);
        let x = m.macro_xs(0.0253, &nucs);
        let (sa, sf) = (x.absorption, x.nu_fission);
        println!("{i} {:<24} {st:>12.5e} {sa:>12.5e} {sf:>12.5e} {:>9.4}", m.name, if st>0.0 {sa/st} else {0.0});
    }
    println!();
    println!("{:<26} {:>12} {:>12} {:>12} {:>9}", "material @ 1 MeV", "Sigma_t", "Sigma_a", "nuSigma_f", "a/t");
    for (i, m) in mats.iter().enumerate() {
        let st = m.macro_xs_total(1.0e6, &nucs);
        let x = m.macro_xs(1.0e6, &nucs);
        let (sa, sf) = (x.absorption, x.nu_fission);
        println!("{i} {:<24} {st:>12.5e} {sa:>12.5e} {sf:>12.5e} {:>9.4}", m.name, if st>0.0 {sa/st} else {0.0});
    }
}
