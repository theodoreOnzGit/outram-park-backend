//! **Where are the neutrons being absorbed?** Macroscopic Σ per material.
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::htr10::{fuel_pebble_materials, BoronReading, Htr10Nuclides};
use nee_soon::htr10_rmc::reflector::zone_composition;

const B10_OF_NATURAL: f64 = 0.199;

fn main() {
    let dir =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");
    let l = |n: &str, f: &str| Nuclide::from_endf_file(&dir.join(f), n, 293.6, 1.0e-3).ok();
    let sab = ThermalScattering::from_endf_file(
        dir.join("tsl-crystalline-graphite.endf").to_str().unwrap(),
        30,
        293.6,
        "c_Graphite",
    )
    .ok();
    // SiC's carbon and silicon are bound in a crystal, not a free gas.
    // ENDF/B-VIII.0 ships tsl-CinSiC (MAT 44) and tsl-SiinSiC (MAT 43) so the
    // coating need not be approximated; a missing law falls back to free gas
    // rather than dropping the nuclide.
    let sic_law = |mat_no: i32, file: &str, name: &'static str| -> Option<ThermalScattering> {
        ThermalScattering::from_endf_file(dir.join(file).to_str()?, mat_no, 293.6, name).ok()
    };
    let c_in_sic = sic_law(44, "tsl-CinSiC.endf", "c_SiC");
    let si_in_sic = sic_law(43, "tsl-SiinSiC.endf", "Si_SiC");
    let bind_sic = |n: Nuclide, s: &Option<ThermalScattering>| match s {
        Some(t) => n.with_thermal_scattering(t.clone()),
        None => n,
    };
    let (
        Some(u5),
        Some(u8),
        Some(o),
        Some(cf),
        Some(cg),
        Some(si),
        Some(b),
        Some(c_sic),
        Some(si29),
        Some(si30),
    ) = (
        l("U235", "n-092_U_235-ENDF8.0.endf"),
        l("U238", "n-092_U_238.endf"),
        l("O16", "n-008_O_016-ENDF8.0.endf"),
        l("C12", "n-006_C_012-ENDF8.0.endf"),
        l("C12", "n-006_C_012-ENDF8.0.endf"),
        l("Si28", "n-014_Si_028-ENDF8.0.endf"),
        l("B10", "n-005_B_010-ENDF8.0.endf"),
        l("C12", "n-006_C_012-ENDF8.0.endf"),
        l("Si29", "n-014_Si_029-ENDF8.0.endf"),
        l("Si30", "n-014_Si_030-ENDF8.0.endf"),
    ) else {
        println!("SKIP: no endf dir");
        return;
    };
    let nucs = vec![
        u5,
        u8,
        o,
        cf,
        cg.with_thermal_scattering(sab.unwrap()),
        bind_sic(si, &si_in_sic),
        b,
        // 7, 8, 9: carbon bound in SiC, and silicon's other two natural
        // isotopes. The atom density was always built from silicon's natural
        // molar mass, so this splits a correct total rather than changing it.
        bind_sic(c_sic, &c_in_sic),
        bind_sic(si29, &si_in_sic),
        bind_sic(si30, &si_in_sic),
    ];
    let idx = Htr10Nuclides {
        u235: 0,
        u238: 1,
        o16: 2,
        c_free: 3,
        c_graphite: 4,
        si28: 5,
        b10: 6,
        // Appended 2026-09-23: slots 0..=6 keep their indices.
        c_sic: 7,
        si29: 8,
        si30: 9,
    };
    let mut mats = fuel_pebble_materials(idx, BoronReading::Natural, 293.6);
    mats.truncate(6);
    mats.push(Material {
        id: 70,
        name: "helium".into(),
        components: vec![],
        temperature: 293.6,
    });
    let z = zone_composition(22).expect("zone 22");
    mats.push(Material {
        id: 71,
        name: "reflector".into(),
        components: vec![
            NuclideComponent {
                nuclide_idx: idx.c_graphite,
                atom_density: z.carbon,
            },
            NuclideComponent {
                nuclide_idx: idx.b10,
                atom_density: z.natural_boron * B10_OF_NATURAL,
            },
        ],
        temperature: 293.6,
    });
    println!(
        "zone 22: carbon {:.6e}  natural_boron {:.6e}  -> B10 {:.6e}",
        z.carbon,
        z.natural_boron,
        z.natural_boron * B10_OF_NATURAL
    );
    println!(
        "{:<26} {:>12} {:>12} {:>12} {:>9}",
        "material @ 0.0253 eV", "Sigma_t", "Sigma_a", "nuSigma_f", "a/t"
    );
    for (i, m) in mats.iter().enumerate() {
        let st = m.macro_xs_total(0.0253, &nucs);
        let x = m.macro_xs(0.0253, &nucs);
        let (sa, sf) = (x.absorption, x.nu_fission);
        println!(
            "{i} {:<24} {st:>12.5e} {sa:>12.5e} {sf:>12.5e} {:>9.4}",
            m.name,
            if st > 0.0 { sa / st } else { 0.0 }
        );
    }
    println!();
    println!(
        "{:<26} {:>12} {:>12} {:>12} {:>9}",
        "material @ 1 MeV", "Sigma_t", "Sigma_a", "nuSigma_f", "a/t"
    );
    for (i, m) in mats.iter().enumerate() {
        let st = m.macro_xs_total(1.0e6, &nucs);
        let x = m.macro_xs(1.0e6, &nucs);
        let (sa, sf) = (x.absorption, x.nu_fission);
        println!(
            "{i} {:<24} {st:>12.5e} {sa:>12.5e} {sf:>12.5e} {:>9.4}",
            m.name,
            if st > 0.0 { sa / st } else { 0.0 }
        );
    }
}
