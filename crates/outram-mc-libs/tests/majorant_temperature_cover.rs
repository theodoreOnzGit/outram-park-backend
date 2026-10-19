//! **A majorant that covers a temperature range** (gh #945, stage 3):
//! `Σ_maj(E) ≥ Σ_t(E, T)` for every `T` in `[T_min, T_max]`. Full record,
//! methodology and results, with the predictions posted beforehand:
//! `verification_and_validation/region_temperature_945/README.md`, section of
//! 2026-10-19. Construction: `physics/delta_tracking/temperature_cover.rs`.
//!
//! # Methodology
//!
//! Two region contents, each processed from ENDF/B-VIII.0 (RECONR + BROADR +
//! URR tables, and LEAPR crystalline graphite for the bound carbon) **exactly**
//! at nine temperatures, 293.6, 450, 600, 750, 900, 1050, 1200, 1350 and
//! 1500 K:
//!
//! - `U238`: U-238 metal, 0.04784 /b-cm (worst Doppler case);
//! - `MIX`: graphite (C-12 at 0.0800, crystalline S(α,β)) with UO2 kernels
//!   smeared at 5 % by volume (U-235 5.285e-5, U-238 1.10735e-3, O-16
//!   2.3204e-3 /b-cm).
//!
//! The **tables** a bound is built from are a subset of the nine
//! (`{293.6, 900, 1500}` and `{293.6, 600, 900, 1200, 1500}`, and the ends
//! alone `{293.6, 1500}` as a control); the **audit** runs
//! `Majorant::audit_over_temperatures` against data processed exactly at the
//! temperatures that are *not* tables, plus those that are. Each table's
//! bound is `Majorant::bounding(4096 bins x 32, margin m0)`, as stage 1;
//! tables are merged by `Majorant::over_temperature_tables(t_margin)`.
//! `m0 = 0` isolates the between-table excess; `m0 = 0.3` is stage 1's
//! configuration. The Chebyshev cover (`chebyshev_cover`) is checked
//! against its own table at every node and midpoint (the proof says it
//! cannot be below), then audited against the data.
//!
//! Cost is reported against stage 1's per-temperature rebuild: the mean
//! `Σ_maj / Σ_t` (the expected virtual collisions per real one, plus 1) over
//! 20 000 log-spaced energies, flat in lethargy and in three windows, for the
//! material at a temperature between the tables, and the build time of each.
//!
//! Hardware: see the README section.

use njoy_outram_park_fork::leapr::decks::SabMaterial;
use njoy_outram_park_fork::reference_data::reference_endf_dir;
use outram_mc_libs::material::material::{Material as LowMaterial, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::speed::SpeedTier;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::physics::delta_tracking::{CoverSettings, Majorant, DEFAULT_T_MARGIN};
use uom::si::f64::{Length, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::thermodynamic_temperature::kelvin;
use rayon::prelude::*;
use std::time::Instant;

/// Every temperature processed, in K.
const ALL_T: [f64; 9] = [
    293.6, 450.0, 600.0, 750.0, 900.0, 1050.0, 1200.0, 1350.0, 1500.0,
];
const U238_TAPE: &str = "n-092_U_238.endf";
const U235_TAPE: &str = "n-092_U_235-ENDF8.0.endf";
const O16_TAPE: &str = "n-008_O_016-ENDF8.0.endf";
const C12_TAPE: &str = "n-006_C_012-ENDF8.0.endf";

/// (tape, name, law, atom density /b-cm) per nuclide of a case.
type Spec = (&'static str, &'static str, Option<SabMaterial>, f64);

fn specs(case: &str) -> Vec<Spec> {
    match case {
        "U238" => vec![(U238_TAPE, "U238", None, 0.04784)],
        _ => vec![
            (
                C12_TAPE,
                "C12",
                Some(SabMaterial::CrystallineGraphite),
                0.0800,
            ),
            (U235_TAPE, "U235", None, 5.285e-5),
            (U238_TAPE, "U238", None, 1.10735e-3),
            (O16_TAPE, "O16", None, 2.3204e-3),
        ],
    }
}

fn have_tapes() -> bool {
    let ok = [U238_TAPE, U235_TAPE, O16_TAPE, C12_TAPE]
        .iter()
        .all(|f| reference_endf_dir().join(f).exists());
    if !ok {
        eprintln!("SKIP: reference ENDF tapes not present");
    }
    ok
}

/// One region content at temperature `t`: its material and nuclides.
struct Set {
    t: f64,
    materials: Vec<LowMaterial>,
    nuclides: Vec<Nuclide>,
}

fn load(case: &str, t: f64) -> Set {
    let sp = specs(case);
    let nuclides: Vec<Nuclide> = sp
        .par_iter()
        .map(|&(file, name, law, _)| {
            let n = Nuclide::from_endf_file_with_speed(
                &reference_endf_dir().join(file),
                name,
                t,
                SpeedTier::Fast,
            )
            .unwrap();
            match law {
                None => n,
                Some(m) => n.with_thermal_scattering(
                    ThermalScattering::from_embedded_leapr_deck(m, t, &format!("{m:?}")).unwrap(),
                ),
            }
        })
        .collect();
    let components = sp
        .iter()
        .enumerate()
        .map(|(i, s)| NuclideComponent {
            nuclide_idx: i,
            atom_density: s.3,
        })
        .collect();
    Set {
        t,
        materials: vec![LowMaterial {
            id: 1,
            name: case.into(),
            components,
            temperature: t,
        }],
        nuclides,
    }
}

fn bounding(s: &Set, margin: f64) -> Majorant {
    Majorant::bounding(&s.materials, &s.nuclides, 1.0e-4, 2.0e7, 4096, 32, margin)
}

fn pick<'a>(sets: &'a [Set], ts: &[f64]) -> Vec<&'a Set> {
    ts.iter()
        .map(|t| sets.iter().find(|s| s.t == *t).expect("processed"))
        .collect()
}

fn audit(m: &Majorant, sets: &[&Set]) -> (f64, f64, f64) {
    let v: Vec<(&[LowMaterial], &[Nuclide])> = sets
        .iter()
        .map(|s| (s.materials.as_slice(), s.nuclides.as_slice()))
        .collect();
    let a = m.audit_over_temperatures(&v, 1.0e-5, 2.0e7, 50_000);
    (a.worst_ratio, a.energy_ev, a.temperature_k)
}

/// Mean Σ_maj/Σ_t of `m` over `s`: flat in lethargy over 1e-3 eV..20 MeV,
/// and over thermal (<1 eV), epithermal (1 eV..10 keV) and fast (>10 keV).
fn overhead(m: &Majorant, s: &Set) -> [f64; 4] {
    let n = 20_000;
    let es: Vec<f64> = (0..n)
        .map(|i| 1.0e-3 * (2.0e10_f64).powf((i as f64 + 0.5) / n as f64))
        .collect();
    let w = |f: &dyn Fn(f64) -> bool| -> Vec<f64> {
        es.iter().map(|&e| if f(e) { 1.0 } else { 0.0 }).collect()
    };
    let flat = vec![1.0; n];
    let th = w(&|e| e < 1.0);
    let ep = w(&|e| (1.0..1.0e4).contains(&e));
    let fa = w(&|e| e >= 1.0e4);
    let o = |w: &[f64]| m.mean_overhead(&s.materials, &s.nuclides, &es, w);
    [o(&flat), o(&th), o(&ep), o(&fa)]
}

/// The proof's claim, checked on the data: the cover is not below its table
/// at any node, any midpoint, or 20 000 independent log energies.
fn assert_cover_above_table(cover: &Majorant, table: &Majorant) {
    // Nodes are private; probe densely instead: 400 000 log energies, plus
    // 1e-6 relative neighbours of each.
    let n = 400_000;
    for i in 0..=n {
        let e = 1.0e-5 * (2.0e12_f64).powf(i as f64 / n as f64);
        for f in [1.0 - 1e-6, 1.0, 1.0 + 1e-6] {
            let x = e * f;
            assert!(
                cover.at(x) >= table.at(x) * (1.0 - 1e-12),
                "cover {} below table {} at {x}",
                cover.at(x),
                table.at(x)
            );
        }
    }
}

fn run_case(case: &str) {
    if !have_tapes() {
        return;
    }
    let clock = Instant::now();
    let sets: Vec<Set> = ALL_T.iter().map(|&t| load(case, t)).collect();
    println!(
        "[{case}] data at {} temperatures: {:.0} s",
        ALL_T.len(),
        clock.elapsed().as_secs_f64()
    );

    // ---- A. between-table excess vs table spacing and margin ---------------
    let configs: [(&str, Vec<f64>); 4] = [
        ("1 table {293.6}", vec![293.6]),
        ("ends {293.6,1500}", vec![293.6, 1500.0]),
        ("3 tables {293.6,900,1500}", vec![293.6, 900.0, 1500.0]),
        (
            "5 tables {293.6,600,900,1200,1500}",
            vec![293.6, 600.0, 900.0, 1200.0, 1500.0],
        ),
    ];
    let mut best_5_m03: Option<Majorant> = None;
    for (label, ts) in &configs {
        let tables = pick(&sets, ts);
        let others: Vec<&Set> = sets.iter().filter(|s| !ts.contains(&s.t)).collect();
        for m0 in [0.0, 0.3] {
            let clock = Instant::now();
            let each: Vec<Majorant> = tables.iter().map(|s| bounding(s, m0)).collect();
            let merged = Majorant::over_temperature_tables(&each, 0.0);
            let build = clock.elapsed().as_secs_f64();
            let (w_tab, e_tab, t_tab) = audit(&merged, &tables);
            let (w_mid, e_mid, t_mid) = audit(&merged, &others);
            println!(
                "[{case}] {label}, per-table margin {m0}: build {build:.1} s, {} nodes; \
                 at the tables worst {w_tab:.7} ({e_tab:.4e} eV, {t_tab} K); \
                 between worst {w_mid:.7} ({e_mid:.4e} eV, {t_mid} K) over {} unprocessed-as-table Ts",
                merged.len(),
                others.len()
            );
            if label.starts_with('5') && m0 == 0.3 {
                assert!(w_tab <= 1.0, "bound broken at a table");
                best_5_m03 = Some(merged);
            }
        }
    }
    let table = best_5_m03.unwrap();

    // ---- B. cost against the per-temperature rebuild (stage 1) -------------
    let mid = &sets[5]; // 1050 K, not a table in the 5-table set
    let clock = Instant::now();
    let rebuilt = bounding(mid, 0.3);
    let t_rebuild = clock.elapsed().as_secs_f64();
    let (w_re, _, _) = audit(&rebuilt, &[mid]);
    let o_re = overhead(&rebuilt, mid);
    let o_tab = overhead(&table, mid);
    println!(
        "[{case}] COST at {} K: per-T rebuild (stage 1): build {t_rebuild:.1} s, worst {w_re:.3}, \
         mean Smaj/St flat/thermal/epi/fast = {:.2}/{:.2}/{:.2}/{:.2}",
        mid.t, o_re[0], o_re[1], o_re[2], o_re[3]
    );
    println!(
        "[{case}] COST at {} K: 5-table max over [293.6,1500]: mean Smaj/St = {:.2}/{:.2}/{:.2}/{:.2} \
         (x{:.2} of rebuild, flat)",
        mid.t,
        o_tab[0],
        o_tab[1],
        o_tab[2],
        o_tab[3],
        o_tab[0] / o_re[0]
    );

    // ---- C. the Chebyshev cover: regions x degree --------------------------
    let mut default_cover: Option<Majorant> = None;
    for regions in [1usize, 4, 16, 64, 256, 1024] {
        for degree in [0usize, 2, 4, 8] {
            let clock = Instant::now();
            let cover = table.chebyshev_cover(CoverSettings { regions, degree });
            let fit = clock.elapsed().as_secs_f64();
            let o = overhead(&cover, mid);
            println!(
                "[{case}] COVER regions {regions:>3} degree {degree}: fit {fit:.2} s, \
                 mean Smaj/St = {:.1}/{:.1}/{:.1}/{:.1} (x{:.1} of table, flat)",
                o[0],
                o[1],
                o[2],
                o[3],
                o[0] / o_tab[0]
            );
            if regions == CoverSettings::default().regions
                && degree == CoverSettings::default().degree
            {
                default_cover = Some(cover);
            }
        }
    }
    let cover = default_cover.expect("the default is in the sweep");
    assert_cover_above_table(&cover, &table);
    let all: Vec<&Set> = sets.iter().collect();
    let (w_c, e_c, t_c) = audit(&cover, &all);
    println!(
        "[{case}] DEFAULT cover {:?}: audited against all {} temperatures, worst Sigma_t/Sigma_maj \
         {w_c:.4} at {e_c:.4e} eV, {t_c} K",
        CoverSettings::default(),
        all.len()
    );
    assert!(
        w_c <= 1.0,
        "the default cover is under the data: {w_c} at {e_c} eV, {t_c} K"
    );
}

/// **U-238 (worst Doppler case) over 293.6-1500 K.** *Results:* README of
/// 2026-10-19.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "processes U-238 at nine temperatures from ENDF (~minutes); runs by default"
)]
fn u238_majorant_covers_the_temperature_range() {
    run_case("U238");
}

/// **Graphite + UO2 kernels (S(α,β) on) over 293.6-1500 K.**
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "processes four nuclides at nine temperatures from ENDF (~tens of minutes); runs by default"
)]
fn graphite_uo2_majorant_covers_the_temperature_range() {
    run_case("MIX");
}

/// **The shipped default, audited between its tables.** Tables 293.6 and
/// 1500 K only (a 1207 K span, the widest spacing measured), stage 1's 0.3
/// per-table margin and `DEFAULT_T_MARGIN`, covered with
/// `CoverSettings::default()`; audited at 293.6, 750 and 1500 K (750 K is
/// between the tables). The cover must not be below its table and nothing may
/// exceed it. Run on its own because the sweep above audits (64, 4).
fn default_audit(case: &str) {
    if !have_tapes() {
        return;
    }
    let sets: Vec<Set> = [293.6, 750.0, 1500.0]
        .iter()
        .map(|&t| load(case, t))
        .collect();
    let tables = pick(&sets, &[293.6, 1500.0]);
    let clock = Instant::now();
    let each: Vec<Majorant> = tables.iter().map(|s| bounding(s, 0.3)).collect();
    let table = Majorant::over_temperature_tables(&each, DEFAULT_T_MARGIN);
    let t_table = clock.elapsed().as_secs_f64();
    let clock = Instant::now();
    let cover = table.chebyshev_cover(CoverSettings::default());
    let t_cover = clock.elapsed().as_secs_f64();
    assert_cover_above_table(&cover, &table);
    let all: Vec<&Set> = sets.iter().collect();
    let (w_t, e_t, k_t) = audit(&table, &all);
    let (w_c, e_c, k_c) = audit(&cover, &all);
    let o_t = overhead(&table, &sets[1]);
    let o_c = overhead(&cover, &sets[1]);
    println!(
        "[{case}] DEFAULT {:?}, t_margin {DEFAULT_T_MARGIN}: table build {t_table:.1} s, cover fit {t_cover:.2} s; \
         table worst {w_t:.4} ({e_t:.3e} eV, {k_t} K), cover worst {w_c:.4} ({e_c:.3e} eV, {k_c} K); \
         at 750 K mean Smaj/St table {:.2}/{:.2}/{:.2}/{:.2}, cover {:.2}/{:.2}/{:.2}/{:.2}",
        CoverSettings::default(),
        o_t[0], o_t[1], o_t[2], o_t[3], o_c[0], o_c[1], o_c[2], o_c[3]
    );
    assert!(
        w_t <= 1.0 && w_c <= 1.0,
        "under-bound: table {w_t}, cover {w_c}"
    );
}

/// The shipped default on U-238, audited between its tables. *Results:* README.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "processes U-238 at three temperatures from ENDF (~minutes); runs by default"
)]
fn u238_default_cover_bounds_between_its_tables() {
    default_audit("U238");
}

/// The shipped default on graphite + UO2, audited between its tables.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "processes four nuclides at three temperatures from ENDF (~minutes); runs by default"
)]
fn graphite_uo2_default_cover_bounds_between_its_tables() {
    default_audit("MIX");
}

/// A library of tape headers (masses only): enough to assemble.
fn header_only_library() -> NuclearData {
    let tape = |za: &str, awr: &str| {
        format!(
            " TPID\n{za:>11}{awr:>11}{:>11}{:>11}{:>11}{:>11}\n{:>11}{:>11}{:>11}{:>11}{:>11}{:>11}\n{:>11}{:>11}{:>11}{:>11}{:>11}{:>11}\n",
            0, 0, 0, 0, "0.0", "0.0", 0, 0, 0, 6, "1.0", "2.0+7", 0, 0, 10, 8
        )
        .into_bytes()
    };
    NuclearData::from_endf_bytes(
        "headers only",
        [
            ("u235".to_string(), tape("9.223500+4", "2.330248+2")),
            ("o16".to_string(), tape("8.016000+3", "1.585751+1")),
        ],
    )
    .expect("two incident-neutron headers")
}

/// **The beginner path.** A delta-tracked region (HEU kernels in oxygen) on
/// CORE (LOW-tier) nuclides: `advanced().with_majorant_cover` gives the
/// region a Chebyshev cover that is at or above the default table at every
/// probed energy; without the call the majorant is the table (no bands).
/// Runs on built-in data, no ENDF needed.
#[test]
fn advanced_cover_reaches_the_region_majorant_and_bounds_the_table() {
    let mut m = Model::new(header_only_library());
    m.set_temperature(Temperature::Uniform(
        ThermodynamicTemperature::new::<kelvin>(293.6),
    ));
    let heu = Material::from_atom_densities([(U235, atoms_per_barn_cm(4.4994e-2))]);
    let oxygen = Material::from_atom_densities([(O16, atoms_per_barn_cm(0.05))]);
    let kernel = Sphere::radius(Length::new::<centimeter>(0.02)).filled_with(heu);
    let particle = Sphere::radius(Length::new::<centimeter>(0.04))
        .filled_with(oxygen.clone())
        .containing(kernel);
    let zone = DhRegion::new(
        Sphere::radius(Length::new::<centimeter>(0.5)),
        oxygen.clone(),
    )
    .with_particles(
        particle,
        Packing::Lattice {
            pitch: Length::new::<centimeter>(0.2),
            offset: [0.5; 3],
        },
    )
    .delta_tracking();
    m.add(
        Sphere::radius(Length::new::<centimeter>(0.6))
            .filled_with(oxygen)
            .containing(zone),
    );
    let plain = m.assemble().unwrap();
    let covered = m
        .clone()
        .advanced()
        .with_majorant_cover(CoverSettings {
            regions: 16,
            degree: 4,
        })
        .assemble()
        .unwrap();
    let nuclides: Vec<Nuclide> = plain
        .nuclides
        .iter()
        .map(|k| Nuclide::from_core(&k.isotope.name()).expect("CORE nuclide"))
        .collect();
    let table = plain.majorants(&nuclides).remove(0);
    let cover = covered.majorants(&nuclides).remove(0);
    assert_eq!(table.cover_bands(), 0, "the default is the table");
    assert!(cover.cover_bands() >= 1);
    for i in 0..=20_000 {
        let e = 1.0e-5 * (2.0e12_f64).powf(i as f64 / 20_000.0);
        assert!(cover.at(e) >= table.at(e) * (1.0 - 1e-12), "at {e}");
    }
}
