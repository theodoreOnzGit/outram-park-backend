//! **Nuclear data through the beginner `NuclearData` handle, every way in**
//! (gh #935, #936; epic #925).
//!
//! # Methodology
//!
//! 1. **A library folder and its zip are the reference tapes.** Godiva
//!    (ICSBEP HEU-MET-FAST-001, `vv::godiva`: one bare sphere, U-234/235/238 at
//!    the spec's atom densities, 293.6 K) is built through `model::Model`
//!    three times, differing only in the data handle:
//!    `NuclearData::workspace_reference()` (the recorded path),
//!    `NuclearData::from_folder(reference-data/endf)` (#935: the mixed
//!    reference folder, read as the library most of its tapes belong to by
//!    their `HSUB` line, ENDF/B-VIII.0), and
//!    `NuclearData::from_zip_bytes(..)` on a zip of that same folder made in
//!    memory by `njoy_outram_park_fork::archive::write_zip` (every file,
//!    README and generator scripts included). Each processes its three
//!    nuclides (RECONR, BROADR, PURR, the 0 K elastic) and runs
//!    `MCSettings::Custom` (400 neutrons × [3 + 6] cycles, seed 11).
//!    **Pass:** the three `k` traces are equal **to the bit**, every cycle,
//!    and the three handles resolve each nuclide to a tape of the same name
//!    (the folder and zip index paths, `workspace_reference` file names).
//! 2. **The ACE way in records what it used.** Godiva from NJOY2016's own
//!    ENDF/B-VIII.0 ACE tables (`reference-data/ace/reference-njoy/`,
//!    `293.6K/U234,U235,U238.ace.gz` and the `0K/U235.ace.gz` companion; the
//!    submodule's `MANIFEST.tsv` has their provenance) through
//!    `NuclearData::from_ace_files`, nothing processed. **Pass:** it runs;
//!    the run's `data_library` names every table with its file and
//!    temperature; U-235 carries DBRC (0 K companion found by header), U-234
//!    and U-238 carry it **off with a stated reason** (the files hold no 0 K
//!    table for them); a model at 600 K is refused with the temperatures the
//!    files have. The `k` is printed, not judged: this is a tiny run and the
//!    two routes differ in physics (DBRC on U-234/238), so no agreement with
//!    test 1 is expected or claimed.
//!
//! Both are `long-tests` (processing three uranium tapes three times, and
//! reading ~580 MB of ACE text).
//!
//! # Results (2026-10-17, i9-13900K, 3 threads, release)
//!
//! 1. **Pass.** The three `k` traces are identical in all 9 cycles
//!    (k = 0.996506 +- 0.018180 each; a tiny run, not a benchmark number). The
//!    folder and zip read the reference folder as ENDF/B-VIII.0: 52
//!    incident-neutron tapes used, 18 of other libraries left out.
//! 2. **Pass.** Four tables indexed in 3.5 s; DBRC on U-235 only; 600 K
//!    refused; k = 1.0208 +- 0.0192 (tiny run).
//!
//! Full record, with the headless-Chromium run:
//! `verification_and_validation/nuclear_data_loading_web/README.md`.

use std::path::{Path, PathBuf};
use std::time::Instant;

use njoy_outram_park_fork::reference_data::reference_endf_dir;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::vv::godiva;
use uom::si::f64::{Length, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::thermodynamic_temperature::kelvin;

fn godiva_model(data: NuclearData, t_k: f64) -> Model {
    let mut m = Model::new(data);
    let core = m.component("Core");
    let heu = Material::from_atom_densities(
        godiva::NUCLIDES
            .iter()
            .map(|(n, _, d)| (Isotope::from_name(n).unwrap(), atoms_per_barn_cm(*d))),
    )
    .named("Godiva HEU");
    m.add(
        Sphere::radius(Length::new::<centimeter>(godiva::RADIUS_CM))
            .filled_with(heu)
            .in_component(core),
    );
    m.set_temperature(Temperature::Uniform(
        ThermodynamicTemperature::new::<kelvin>(t_k),
    ));
    m
}

fn small_run() -> KeffSettings {
    KeffSettings::new(
        MCSettings::Custom(CustomRun {
            particles: 400,
            inactive: 3,
            active: 6,
            seed: 11,
        }),
        None,
    )
}

/// Every file under `dir`, paths relative to it, sorted.
fn read_tree(dir: &Path, rel: &str, out: &mut Vec<(String, Vec<u8>)>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        let name = p.file_name().unwrap().to_str().unwrap().to_string();
        let r = if rel.is_empty() {
            name
        } else {
            format!("{rel}/{name}")
        };
        if p.is_dir() {
            read_tree(&p, &r, out);
        } else {
            out.push((r, std::fs::read(&p).unwrap()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
}

#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "processes U-234/235/238 three times (~6 min); runs by default"
)]
fn godiva_from_folder_and_zip_matches_the_reference_bit_for_bit() {
    let dir = reference_endf_dir();
    if !dir.join("n-092_U_235-ENDF8.0.endf").exists() {
        eprintln!("SKIP: reference tapes not present");
        return;
    }
    let mut files = Vec::new();
    read_tree(&dir, "", &mut files);
    let t0 = Instant::now();
    let zip = njoy_outram_park_fork::archive::write_zip(&files).unwrap();
    println!(
        "zipped {} files ({:.1} MB) to {:.1} MB in {:.1} s",
        files.len(),
        files.iter().map(|f| f.1.len()).sum::<usize>() as f64 / 1e6,
        zip.len() as f64 / 1e6,
        t0.elapsed().as_secs_f64()
    );
    let handles = [
        (
            "workspace_reference",
            NuclearData::workspace_reference().unwrap(),
        ),
        ("from_folder", NuclearData::from_folder(&dir).unwrap()),
        (
            "from_zip_bytes",
            NuclearData::from_zip_bytes("reference-data-endf.zip", zip).unwrap(),
        ),
    ];
    let mut traces: Vec<Vec<u64>> = Vec::new();
    for (name, data) in handles {
        for (n, file, _) in godiva::NUCLIDES {
            let got = data.tape_file(Isotope::from_name(n).unwrap()).unwrap();
            assert_eq!(
                got.rsplit('/').next().unwrap(),
                file.to_string(),
                "{name}: {n}"
            );
        }
        println!("{name}: {}", data.label());
        let t = Instant::now();
        let run = godiva_model(data, godiva::TEMPERATURE_K)
            .keff(small_run())
            .unwrap();
        println!(
            "{name}: k = {:.6} +- {:.6} ({:.1} s, processing included)",
            run.keff().mean,
            run.keff().std,
            t.elapsed().as_secs_f64()
        );
        for p in &run.provenance.physics {
            assert!(p.dbrc.is_ok(), "{name} {}: {:?}", p.nuclide, p.dbrc);
        }
        traces.push(run.k_by_cycle.iter().map(|k| k.to_bits()).collect());
    }
    assert_eq!(
        traces[0], traces[1],
        "from_folder differs from workspace_reference"
    );
    assert_eq!(
        traces[0], traces[2],
        "from_zip_bytes differs from workspace_reference"
    );
    println!("k trace fingerprint (bits of every cycle's k) equal across all three");
}

fn ace_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/ace/reference-njoy/endf-b-viii.0")
}

#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "reads ~580 MB of ACE text; runs by default"
)]
fn godiva_from_ace_files_runs_and_records_its_data() {
    let dir = ace_dir();
    if !dir.join("293.6K/U235.ace.gz").exists() {
        eprintln!("SKIP: reference-data/ace submodule not checked out");
        return;
    }
    let names = [
        "293.6K/U234.ace.gz",
        "293.6K/U235.ace.gz",
        "293.6K/U238.ace.gz",
        "0K/U235.ace.gz",
    ];
    let files: Vec<(String, Vec<u8>)> = names
        .iter()
        .map(|n| (n.to_string(), std::fs::read(dir.join(n)).unwrap()))
        .collect();
    let t = Instant::now();
    let data = NuclearData::from_ace_files("reference-njoy ENDF/B-VIII.0", files).unwrap();
    println!(
        "indexed in {:.1} s: {}",
        t.elapsed().as_secs_f64(),
        data.label()
    );
    for n in names {
        assert!(
            data.label().contains(n),
            "{n} missing from {}",
            data.label()
        );
    }
    let refused = godiva_model(data.clone(), 600.0).keff(small_run());
    match refused {
        Err(RunError::Data(DataError::Ace { reason, .. })) => {
            assert!(reason.contains("293"), "{reason}")
        }
        other => panic!("600 K must be refused: {:?}", other.map(|r| r.keff())),
    }
    let t = Instant::now();
    let run = godiva_model(data, godiva::TEMPERATURE_K)
        .keff(small_run())
        .unwrap();
    println!(
        "ACE Godiva: k = {:.6} +- {:.6} ({:.1} s)\n{run}",
        run.keff().mean,
        run.keff().std,
        t.elapsed().as_secs_f64()
    );
    assert!(run.provenance.data_library.contains("92235.00c"));
    for p in &run.provenance.physics {
        match p.nuclide.as_str() {
            "U235" => assert!(p.dbrc.is_ok(), "U-235 has its 0 K companion: {:?}", p.dbrc),
            _ => assert!(
                p.dbrc.is_err(),
                "{} has no 0 K table, so DBRC must say it is off",
                p.nuclide
            ),
        }
    }
    assert!(run.keff().mean > 0.9 && run.keff().mean < 1.1);
}
