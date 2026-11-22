// SPDX-License-Identifier: AGPL-3.0-only

//! **LEU-COMP-THERM-008 case 1 lattice from an OpenMC HDF5 library, against the
//! straight-from-ENDF route** -- GitHub #303 acceptance row 2.
//!
//! The model, nuclide tier and settings are `lct008_ace_roundtrip.rs`'s (the
//! parity bar the ACE route is held to): the case-1 lattice of
//! `common/lct008_model.rs`, 11 nuclides, 10000 histories x [250 inactive + 400
//! active] by default. Two arms, same seeds:
//!
//! ```text
//! A.  ENDF tape -> RECONR -> BROADR (293.6 K), URR + DBRC by default  -> Nuclide
//! B.  OpenMC .h5 (NJOY2016 ACER, 0K + 294K, PURR) -> read_nuclide_selected -> Nuclide::from_hdf5
//! ```
//!
//! S(a,b) H in H2O is built once from `tsl-HinH2O.endf` and attached on both
//! arms (the library's `c_H_in_H2O.h5` is not read), so this tests the
//! continuous-energy tables. Both arms carry URR and DBRC (the library's `0K`
//! elastic feeds B's DBRC).
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example lct008_hdf5_parity -- \
//!     --hdf5-dir DIR --seeds 8 --threads 2 [--particles N --inactive N --active N]
//! ```
//!
//! Results and the predictions they are held to:
//! `verification_and_validation/hdf5_library_303/hdf5_library_2026_11_18.md`.

use njoy_outram_park_fork::hdf5::nuclide_read::read_nuclide_selected;
use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::physics::compute::{ComputeType, ThreadCount};
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use std::collections::BTreeMap;
use std::time::Instant;

#[path = "common/lct008_model.rs"]
mod lct008_model;

const TEMP_K: f64 = 293.6;

/// `(name, ENDF file)` -- the 11-nuclide tier, as `lct008_ace_roundtrip.rs`.
const NUCLIDES: [(&str, &str); 11] = [
    ("H1", "n-001_H_001-ENDF8.0-Beta6.endf"),
    ("B10", "n-005_B_010-ENDF8.0.endf"),
    ("O16", "n-008_O_016-ENDF8.0.endf"),
    ("U234", "n-092_U_234-ENDF8.0.endf"),
    ("U235", "n-092_U_235-ENDF8.0.endf"),
    ("U238", "n-092_U_238.endf"),
    ("Al27", "n-013_Al_027-ENDF8.0.endf"),
    ("Si28", "n-014_Si_028-ENDF8.0.endf"),
    ("Si29", "n-014_Si_029-ENDF8.0.endf"),
    ("Si30", "n-014_Si_030-ENDF8.0.endf"),
    ("Mn55", "n-025_Mn_055-ENDF8.0.endf"),
];

fn arg_usize(args: &[String], flag: &str) -> Option<usize> {
    let i = args.iter().position(|a| a == flag)?;
    args.get(i + 1)?.parse().ok()
}

/// Mean, sd and standard error of a mean over seeds.
fn summarise(v: &[f64]) -> (f64, f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    let var = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0).max(1.0);
    (m, var.sqrt(), (var / n).sqrt())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args
        .iter()
        .position(|a| a == "--hdf5-dir")
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from)
        .expect("--hdf5-dir DIR is required");
    let n_particles = arg_usize(&args, "--particles").unwrap_or(10_000);
    let n_inactive = arg_usize(&args, "--inactive").unwrap_or(250);
    let n_active = arg_usize(&args, "--active").unwrap_or(400);
    let threads = arg_usize(&args, "--threads").unwrap_or(2);
    let n_seeds = arg_usize(&args, "--seeds").unwrap_or(8).max(2);
    let seed0 = arg_usize(&args, "--seed").map_or(KeffSettings::default().seed, |v| v as u64);
    let skip_endf = args.iter().any(|a| a == "--h5-only");

    println!("LCT-008 case-1 lattice, 11 nuclides: ENDF route (A) vs OpenMC HDF5 route (B)");
    let mut via_endf: Vec<Nuclide> = Vec::new();
    let mut via_h5: Vec<Nuclide> = Vec::new();
    let (mut t_endf, mut t_read, mut t_conv) = (0.0, 0.0, 0.0);
    for (name, file) in NUCLIDES {
        let t = Instant::now();
        let read = read_nuclide_selected(dir.join(format!("{name}.h5")), Some("294K"), &["0K"])
            .unwrap_or_else(|e| panic!("read {name}.h5: {e}"));
        t_read += t.elapsed().as_secs_f64();
        let t = Instant::now();
        via_h5.push(Nuclide::from_hdf5(&read, name).unwrap_or_else(|e| panic!("{name}: {e}")));
        t_conv += t.elapsed().as_secs_f64();
        if !skip_endf {
            let path = reference_endf(file).unwrap_or_else(|| panic!("missing tape {file}"));
            let t = Instant::now();
            via_endf.push(
                Nuclide::from_endf_file(
                    &path,
                    name,
                    uom::si::f64::ThermodynamicTemperature::new::<
                        uom::si::thermodynamic_temperature::kelvin,
                    >(TEMP_K),
                    1.0e-3,
                )
                .unwrap_or_else(|e| panic!("from_endf_file({name}): {e}")),
            );
            t_endf += t.elapsed().as_secs_f64();
        }
    }
    println!(
        "data: HDF5 read {t_read:.1} s + from_hdf5 {t_conv:.1} s; ENDF route {t_endf:.1} s \
         ({threads} transport threads; Intel Xeon 2.10 GHz, 4 logical cores, shared, CPU only)"
    );
    for (a, b) in via_endf.iter().zip(&via_h5) {
        println!(
            "  {:<5} URR endf/h5 {}/{}  DBRC {}/{}",
            a.name,
            a.has_urr_probability_tables(),
            b.has_urr_probability_tables(),
            a.has_dbrc(),
            b.has_dbrc()
        );
    }

    let _ = lct008_model::ACTIVE_CASE.set(1);
    let spec = lct008_model::parse_materials(lct008_model::materials_xml());
    let slots: BTreeMap<String, usize> = NUCLIDES
        .iter()
        .enumerate()
        .map(|(i, (n, _))| (n.to_string(), i))
        .collect();
    let mut omitted: BTreeMap<String, f64> = BTreeMap::new();
    for m in &spec {
        for (n, ao) in &m.nuclides {
            if !slots.contains_key(n) {
                *omitted.entry(n.clone()).or_insert(0.0) += ao;
            }
        }
    }
    let (materials, _) = lct008_model::build_materials(&spec, &slots, &omitted, false);
    lct008_model::report_omissions(&spec, &omitted);
    let geom = lct008_model::build_geometry(&materials, true);
    let _ = lct008_model::check_geometry(&geom, &materials);
    let src = SourceBox {
        lower: Position::new(-lct008_model::R_CORE, -lct008_model::R_CORE, lct008_model::Z_LO),
        upper: Position::new(lct008_model::R_CORE, lct008_model::R_CORE, lct008_model::Z_HI),
    };
    let sab = ThermalScattering::from_endf_file(
        reference_endf("tsl-HinH2O.endf").expect("H(H2O) tape").to_str().expect("path"),
        1,
        TEMP_K,
        "c_H_in_H2O",
    )
    .expect("H(H2O) S(a,b)");
    let bound = |nuc: &[Nuclide]| -> (Vec<Material>, Vec<Nuclide>) {
        lct008_model::assign_spec_sab(&spec, &materials, nuc, &slots, Some(&sab))
    };
    let (mats_h5, via_h5) = bound(&via_h5);
    let endf_bound = (!skip_endf).then(|| bound(&via_endf));

    let base = KeffSettings {
        n_particles,
        n_inactive,
        n_active,
        temperature_k: TEMP_K,
        compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(threads)),
        ..KeffSettings::default()
    };
    println!(
        "\n{n_particles} histories x [{n_inactive} + {n_active}], {n_seeds} seeds from {seed0}, {threads} threads"
    );
    let (mut ka, mut kb) = (Vec::new(), Vec::new());
    let (mut tr_a, mut tr_b) = (0.0, 0.0);
    for s in 0..n_seeds {
        let mut set = base.clone();
        set.seed = seed0 + s as u64;
        if let Some((mats, nuc)) = &endf_bound {
            let t = Instant::now();
            let r = run_keff_csg(&geom, mats, nuc, src, &set, None);
            tr_a += t.elapsed().as_secs_f64();
            ka.push(r.k_mean);
        }
        let t = Instant::now();
        let r = run_keff_csg(&geom, &mats_h5, &via_h5, src, &set, None);
        tr_b += t.elapsed().as_secs_f64();
        kb.push(r.k_mean);
        println!(
            "  seed {:>3}: ENDF {}   HDF5 {:.5}",
            set.seed,
            ka.get(s).map_or("-".to_string(), |k| format!("{k:.5}")),
            kb[s]
        );
    }
    let (mb, sdb, seb) = summarise(&kb);
    println!("\n  HDF5 route: k = {mb:.5} +/- {seb:.5} (sem), seed sd {:.0} pcm, transport {tr_b:.0} s", 1.0e5 * sdb);
    if !ka.is_empty() {
        let (ma, sda, sea) = summarise(&ka);
        println!("  ENDF route: k = {ma:.5} +/- {sea:.5} (sem), seed sd {:.0} pcm, transport {tr_a:.0} s", 1.0e5 * sda);
        let d = 1.0e5 * (mb - ma);
        let sig = 1.0e5 * (sea * sea + seb * seb).sqrt();
        println!("  HDF5 - ENDF: {d:+.1} +/- {sig:.1} pcm  (z = {:+.2})", d / sig);
    }
}
