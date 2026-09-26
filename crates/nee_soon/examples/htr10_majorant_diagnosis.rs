//! **Why delta-tracking a whole HTR-10 bed fails** — diagnosis for `bn:op-867c`.
//!
//! The first full-core run (2026-09-17) returned `k = 0`, zero entropy and
//! **2.82e9 virtual collisions** — 18,801 per history — with every history
//! exhausting its budget and leaking. This measures why, rather than guessing.

use nee_soon::htr10_rmc::reflector::zone_composition;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::pebble_beds::htr10::{fuel_pebble_materials, BoronReading, Htr10Nuclides};

const TEMP_K: f64 = 300.15;
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
    let load = |n: &str, f: &str| -> Option<Nuclide> {
        let p = base.join(f);
        p.exists().then_some(())?;
        Nuclide::from_endf_file(&p, n, TEMP_K, 1.0e-3).ok()
    };
    let Some(sab) = ThermalScattering::from_endf_file(
        base.join("tsl-crystalline-graphite.endf").to_str().unwrap(),
        30,
        TEMP_K,
        "c_Graphite",
    )
    .ok() else {
        println!("SKIP: no tapes");
        return;
    };
    // SiC's carbon and silicon are bound in a crystal, not a free gas.
    // ENDF/B-VIII.0 ships tsl-CinSiC (MAT 44) and tsl-SiinSiC (MAT 43) so
    // the coating need not be approximated; a missing law falls back to
    // free gas rather than dropping the nuclide.
    let sic_law = |mat_no: i32, file: &str, name: &'static str| -> Option<ThermalScattering> {
        ThermalScattering::from_endf_file(base.join(file).to_str()?, mat_no, TEMP_K, name).ok()
    };
    let c_in_sic = sic_law(44, "tsl-CinSiC.endf", "c_SiC");
    let si_in_sic = sic_law(43, "tsl-SiinSiC.endf", "Si_SiC");
    let bind_sic = |n: Nuclide, s: &Option<ThermalScattering>| match s {
        Some(t) => n.with_thermal_scattering(t.clone()),
        None => n,
    };
    let Some(nucs) = (|| {
        Some(vec![
            load("U235", "n-092_U_235-ENDF8.0.endf")?,
            load("U238", "n-092_U_238.endf")?,
            load("O16", "n-008_O_016-ENDF8.0.endf")?,
            load("C12", "n-006_C_012-ENDF8.0.endf")?,
            load("C12", "n-006_C_012-ENDF8.0.endf")?.with_thermal_scattering(sab),
            bind_sic(load("Si28", "n-014_Si_028-ENDF8.0.endf")?, &si_in_sic),
            load("B10", "n-005_B_010-ENDF8.0.endf")?,
            // 7, 8, 9: carbon bound in SiC, and silicon's other two natural
            // isotopes. The atom density was always built from silicon's natural
            // molar mass, so this splits a correct total rather than changing it.
            bind_sic(load("C12", "n-006_C_012-ENDF8.0.endf")?, &c_in_sic),
            bind_sic(load("Si29", "n-014_Si_029-ENDF8.0.endf")?, &si_in_sic),
            bind_sic(load("Si30", "n-014_Si_030-ENDF8.0.endf")?, &si_in_sic),
            load("B11", "n-005_B_011-ENDF8.0.endf")?, // 10: B-11 (gh:#311)
        ])
    })() else {
        println!("SKIP: no tapes");
        return;
    };

    let mut mats = fuel_pebble_materials(NUC, BoronReading::Natural, TEMP_K);
    mats.truncate(6);
    mats.push(Material {
        id: 70,
        name: "helium (empty)".into(),
        components: vec![],
        temperature: TEMP_K,
    });
    let z = zone_composition(22).unwrap();
    mats.push(Material {
        id: 71,
        name: "reflector".into(),
        components: vec![
            NuclideComponent {
                nuclide_idx: NUC.c_graphite,
                atom_density: z.carbon,
            },
            NuclideComponent {
                nuclide_idx: NUC.b10,
                atom_density: z.natural_boron * 0.199,
            },
        ],
        temperature: TEMP_K,
    });

    let grid: Vec<f64> = (0..4096)
        .map(|i| (1.0e-4_f64.ln() + (2.0e7_f64.ln() - 1.0e-4_f64.ln()) * i as f64 / 4095.0).exp())
        .collect();
    let maj = Majorant::over_indices(&mats, &(0..=6).collect::<Vec<_>>(), &nucs, &grid, 0.3);

    println!("Why a whole-bed delta region rejects 18,801 times per history");
    println!("=============================================================\n");
    println!(
        "{:<34} {:>14} {:>14} {:>12}",
        "material", "sigma_t(0.0253)", "majorant", "p_real"
    );
    println!("{:-<78}", "");
    let m = maj.at(0.0253);
    for (i, mat) in mats.iter().enumerate().take(7) {
        let st = mat.macro_xs_total(0.0253, &nucs);
        let p = if m > 0.0 { st / m } else { 0.0 };
        println!(
            "{:<34} {st:>14.5} {m:>14.5} {p:>12.2e}",
            format!("{i}: {}", mat.name)
        );
    }
    let helium_p: f64 = 0.0;
    println!("\n  The majorant is set by the UO2 KERNEL, which occupies about");
    println!("  0.05 x 0.578 = 2.9 % of a fuelled pebble by volume, and fuelled");
    println!("  pebbles are 57 % of a bed that is itself 61 % solid.");
    println!("  So the kernel is roughly 1 % of the delta region by volume, while");
    println!("  setting the bound for all of it.");
    println!("\n  Expected acceptance, volume-weighted:");
    let vols = [0.010_f64, 0.004, 0.002, 0.002, 0.003, 0.33, 0.39];
    let mut acc = 0.0;
    for (i, v) in vols.iter().enumerate() {
        acc += v * mats[i].macro_xs_total(0.0253, &nucs) / m;
    }
    println!(
        "    p_accept ~ {acc:.4}  ->  ~{:.0} rejections per real collision",
        1.0 / acc.max(1e-12) - 1.0
    );
    println!("    measured: 18,801 virtual collisions per history");
    let _ = helium_p;
}
