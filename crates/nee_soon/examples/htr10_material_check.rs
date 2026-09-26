//! Does the HTR-10 kernel material actually fission? Diagnostic for op-867c.
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::htr10::{fuel_pebble_materials, BoronReading, Htr10Nuclides};

const T: f64 = 300.15;
const NUC: Htr10Nuclides = Htr10Nuclides {
    u235: 0,
    u238: 1,
    o16: 2,
    c_free: 3,
    c_graphite: 4,
    si28: 5,
    b10: 6,
    // Appended 2026-09-23: slots 0..=6 keep their indices so no
    // existing material silently repoints at a different nuclide.
    c_sic: 7,
    si29: 8,
    si30: 9,
    b11: 10,
};

fn main() {
    let base =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");
    let ld = |n: &str, f: &str| -> Option<Nuclide> {
        let p = base.join(f);
        p.exists().then_some(())?;
        Nuclide::from_endf_file(&p, n, T, 1.0e-3).ok()
    };
    let Some(sab) = ThermalScattering::from_endf_file(
        base.join("tsl-crystalline-graphite.endf").to_str().unwrap(),
        30,
        T,
        "c_Graphite",
    )
    .ok() else {
        println!("SKIP");
        return;
    };
    // SiC's carbon and silicon are bound in a crystal, not a free gas.
    // ENDF/B-VIII.0 ships tsl-CinSiC (MAT 44) and tsl-SiinSiC (MAT 43) so
    // the coating need not be approximated; a missing law falls back to
    // free gas rather than dropping the nuclide.
    let sic_law = |mat_no: i32, file: &str, name: &'static str| -> Option<ThermalScattering> {
        ThermalScattering::from_endf_file(base.join(file).to_str()?, mat_no, T, name).ok()
    };
    let c_in_sic = sic_law(44, "tsl-CinSiC.endf", "c_SiC");
    let si_in_sic = sic_law(43, "tsl-SiinSiC.endf", "Si_SiC");
    let bind_sic = |n: Nuclide, s: &Option<ThermalScattering>| match s {
        Some(t) => n.with_thermal_scattering(t.clone()),
        None => n,
    };
    let Some(n) = (|| {
        Some(vec![
            ld("U235", "n-092_U_235-ENDF8.0.endf")?,
            ld("U238", "n-092_U_238.endf")?,
            ld("O16", "n-008_O_016-ENDF8.0.endf")?,
            ld("C12", "n-006_C_012-ENDF8.0.endf")?,
            ld("C12", "n-006_C_012-ENDF8.0.endf")?.with_thermal_scattering(sab),
            bind_sic(ld("Si28", "n-014_Si_028-ENDF8.0.endf")?, &si_in_sic),
            ld("B10", "n-005_B_010-ENDF8.0.endf")?,
            // 7, 8, 9: carbon bound in SiC, and silicon's other two natural
            // isotopes. The atom density was always built from silicon's natural
            // molar mass, so this splits a correct total rather than changing it.
            bind_sic(ld("C12", "n-006_C_012-ENDF8.0.endf")?, &c_in_sic),
            bind_sic(ld("Si29", "n-014_Si_029-ENDF8.0.endf")?, &si_in_sic),
            bind_sic(ld("Si30", "n-014_Si_030-ENDF8.0.endf")?, &si_in_sic),
            ld("B11", "n-005_B_011-ENDF8.0.endf")?, // 10: B-11 (gh:#311)
        ])
    })() else {
        println!("SKIP");
        return;
    };

    let mats = fuel_pebble_materials(NUC, BoronReading::Natural, T);
    println!("fuel_pebble_materials returned {} materials", mats.len());
    for e in [0.0253_f64, 1.0, 1.0e5, 2.0e6] {
        println!("\n--- E = {e:e} eV ---");
        for (i, m) in mats.iter().enumerate() {
            let x = m.macro_xs(e, &n);
            println!(
                "  {i}: {:<24} total {:>10.5}  fission {:>10.3e}  nu-fis {:>10.3e}",
                m.name, x.total, x.fission, x.nu_fission
            );
        }
    }
}
