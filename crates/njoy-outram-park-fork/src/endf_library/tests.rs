// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

use std::path::PathBuf;

use super::*;
use crate::acer::ace_set::AceSet;
use crate::acquire::EndfLibrary;
use crate::archive::write_zip;
use crate::leapr::decks::SabMaterial;
use crate::process::ThermalSettings;
use crate::units::Temperature;
use uom::si::thermodynamic_temperature::kelvin;

fn reference() -> Option<PathBuf> {
    let d = crate::reference_data::reference_endf_dir();
    d.join("n-092_U_235-ENDF8.0.endf").is_file().then_some(d)
}

fn head(file: &str) -> EndfHeader {
    let d = reference().expect("reference tapes");
    let bytes = std::fs::read(d.join(file)).unwrap();
    EndfHeader::parse(&String::from_utf8_lossy(&bytes[..4096.min(bytes.len())])).unwrap()
}

/// **Identity and library come from the header**, checked on the reference
/// tapes of five libraries: the `HSUB` line separates releases the `LREL`
/// field does not (VII.0's B-10 and VII.1's Cl-35 both carry `LREL = 1`).
#[test]
fn headers_name_their_library_and_identity() {
    if reference().is_none() {
        eprintln!("SKIP: no reference-data/endf");
        return;
    }
    let u = head("n-092_U_235-ENDF8.0.endf");
    assert_eq!(
        (u.za, u.z(), u.a(), u.mat, u.liso, u.nlib),
        (92235, 92, 235, Some(9228), 0, 0)
    );
    assert_eq!((u.nsub, u.nver, u.nfor), (Some(10), Some(8), Some(6)));
    assert_eq!(u.sub_library(), SubLibrary::IncidentNeutron);
    assert!(u.is_endf6());
    assert_eq!(u.library(), "ENDF/B-VIII.0");
    assert!(u.symbol.contains("235"));
    for (file, lib, variant) in [
        (
            "n-038_Sr_088-ENDF8.1.endf",
            "ENDF/B-VIII.1",
            Some(EndfLibrary::EndfBVIII1),
        ),
        (
            "n-005_B_010-ENDF7.0.endf",
            "ENDF/B-VII",
            Some(EndfLibrary::EndfBVII0),
        ),
        (
            "n-017_Cl_035-ENDF7.1.endf",
            "ENDF/B-VII.1",
            Some(EndfLibrary::EndfBVII1),
        ),
        ("n-092_U_238-JENDL3.3.endf", "JENDL-3.3", None),
        ("n-018_Ar_37-tendl2023.endf", "TENDL-2023", None),
    ] {
        let h = head(file);
        assert_eq!(h.library(), lib, "{file}");
        assert_eq!(EndfLibrary::from_hsub(&h.library()), variant, "{file}");
    }
    assert_eq!(
        head("n-005_B_010-ENDF7.0.endf").lrel,
        head("n-017_Cl_035-ENDF7.1.endf").lrel
    );
    let t = head("n-018_Ar_37-tendl2023.endf");
    assert_eq!((t.nlib, library_family(t.nlib)), (17, "TENDL"));
    let w = head("tsl-HinH2O.endf");
    assert_eq!(
        (w.sub_library(), w.mat),
        (SubLibrary::ThermalScattering, Some(1))
    );
    // Without the HSUB line the library is the NLIB family and NVER.
    let mut bare = u.clone();
    bare.library_text = None;
    assert_eq!(bare.library(), "ENDF/B (NLIB 0) version 8");
    bare.nver = None;
    assert_eq!(bare.library(), "ENDF/B (NLIB 0)");
    assert_eq!(library_family(99), "unlisted library");
    for n in [Some(0), None] {
        let s = EndfHeader {
            nsub: n,
            ..u.clone()
        }
        .sub_library();
        assert!(matches!(s, SubLibrary::Other(0) | SubLibrary::Unknown));
    }
    // A fixture with no MF/MT columns is read by position; junk is not a tape.
    let fake = " TPID\n 9.223500+4 2.330248+2          0          0          0          0\n";
    assert_eq!(EndfHeader::parse(fake).unwrap().za, 92235);
    assert!(EndfHeader::parse("hello\nworld\n").is_none());
}

/// **The layout from the names**, and which files each layout opens.
#[test]
fn the_layout_is_read_from_the_names() {
    let nndc = [
        "ENDF-B-VIII.0_neutrons/CHANGELOG.txt",
        "ENDF-B-VIII.0_neutrons/n-001_H_001.endf",
    ];
    assert_eq!(
        LibraryLayout::detect(nndc),
        LibraryLayout::NndcSubLibraryArchives
    );
    assert_eq!(library_named_in(nndc), Some(EndfLibrary::EndfBVIII0));
    let sub = ["ENDF-B-VII.1/neutrons/n.endf", "ENDF-B-VII.1/decay/d.endf"];
    assert_eq!(LibraryLayout::detect(sub), LibraryLayout::SubLibraryFolders);
    assert_eq!(library_named_in(sub), Some(EndfLibrary::EndfBVII1));
    assert_eq!(
        library_named_in(["ENDF-B-VII.1/a", "ENDF-B-VIII.0/b"]),
        None
    );
    let iaea = ["n/n_9228_92-U-235.zip", "tsl/tsl-HinH2O.zip"];
    assert_eq!(LibraryLayout::detect(iaea), LibraryLayout::IaeaPerNuclide);
    assert_eq!(
        LibraryLayout::detect(["a.endf", "b.endf"]),
        LibraryLayout::Flat
    );
    let s = LibraryLayout::SubLibraryFolders;
    assert_eq!(s.role("L/neutrons/n.endf"), PathRole::Neutrons);
    assert_eq!(s.role("L/thermal_scatt/t.endf"), PathRole::Thermal);
    assert_eq!(s.role("L/decay/d.endf"), PathRole::Skip);
    assert_eq!(s.role("loose.endf"), PathRole::Unknown);
    let n = LibraryLayout::NndcSubLibraryArchives;
    assert_eq!(
        n.role("ENDF-B-VIII.0_neutrons/CHANGELOG.txt"),
        PathRole::Skip
    );
    assert_eq!(
        n.role("ENDF-B-VIII.0_thermal_scatt/tsl-HinH2O.endf"),
        PathRole::Thermal
    );
    let i = LibraryLayout::IaeaPerNuclide;
    assert_eq!(
        i.role("n/n_9228_92-U-235.zip/n_9228.dat"),
        PathRole::Neutrons
    );
    assert_eq!(i.role("tsl/x.zip/x.dat"), PathRole::Thermal);
    assert_eq!(i.role("other.dat"), PathRole::Unknown);
    assert_eq!(LibraryLayout::Flat.role(".hidden"), PathRole::Skip);
    assert_eq!(LibraryLayout::Flat.role("x.endf"), PathRole::Unknown);
}

/// **The workspace's mixed reference folder** reads as ENDF/B-VIII.0 (most
/// of its tapes), and each Godiva nuclide resolves to the tape `vv::godiva`
/// names; another library's tape is named in the error, never used.
#[test]
fn a_mixed_folder_reads_as_its_main_library() {
    let Some(dir) = reference() else {
        eprintln!("SKIP: no reference-data/endf");
        return;
    };
    let ix = EndfLibraryIndex::from_folder(&dir).unwrap();
    assert_eq!(ix.layout(), LibraryLayout::Flat);
    assert_eq!(ix.source(), &LibrarySource::Folder(dir.clone()));
    let main = ix.main_library().unwrap();
    assert_eq!(main, "ENDF/B-VIII.0");
    for (a, file) in [
        (234, "n-092_U_234-ENDF8.0.endf"),
        (235, "n-092_U_235-ENDF8.0.endf"),
        (238, "n-092_U_238.endf"),
    ] {
        assert_eq!(ix.incident_neutron(&main, 92, a, 0).unwrap().path, file);
    }
    // Fe-58: the VIII.0 tape, not the "ENDF/B-VIII.beta" one beside it.
    assert_eq!(
        ix.incident_neutron(&main, 26, 58, 0).unwrap().path,
        "n-026_Fe_058-ENDF8.0.endf"
    );
    match ix.incident_neutron(&main, 17, 35, 0) {
        Err(LookupError::Missing { elsewhere }) => {
            assert!(elsewhere[0].contains("ENDF/B-VII.1"), "{elsewhere:?}")
        }
        other => panic!("{other:?}"),
    }
    assert!(
        matches!(ix.incident_neutron(&main, 99, 1, 0), Err(LookupError::Missing { elsewhere }) if elsewhere.is_empty())
    );
    let water = ix.thermal(&main, 1).unwrap();
    assert_eq!(water.path, "tsl-HinH2O.endf");
    assert!(ix.skipped().iter().any(|s| s.path.ends_with(".py")));
    assert!(ix.summary().contains("ENDF/B-VIII.0"));
    let hits = ix.search(&main, "u235");
    assert_eq!(hits[0].display_name(), "U-235");
    assert!(ix.search(&main, "").len() > 50);
    assert!(ix.search(&main, "zzzzqq").is_empty());
    assert!(LookupError::Ambiguous(vec!["a".into()])
        .to_string()
        .contains("several"));
    assert!(EndfLibraryIndex::from_folder(&dir.join("no-such-dir")).is_err());
}

/// **A zip and an upload index like the folder they came from**: an NNDC
/// sub-library archive built in memory from reference tapes (renamed as NNDC
/// names them, with a changelog), the same as dropped files, and the IAEA
/// per-nuclide tree (zips inside a zip). Tapes read back byte for byte.
#[test]
fn zips_and_uploads_index_like_folders() {
    let Some(dir) = reference() else {
        eprintln!("SKIP: no reference-data/endf");
        return;
    };
    let f19 = std::fs::read(dir.join("n-009_F_019-ENDF8.0.endf")).unwrap();
    let o16 = std::fs::read(dir.join("n-008_O_016-ENDF8.0.endf")).unwrap();
    let water = std::fs::read(dir.join("tsl-HinH2O.endf")).unwrap();
    let files = vec![
        (
            "ENDF-B-VIII.0_neutrons/CHANGELOG.txt".to_string(),
            b"changes".to_vec(),
        ),
        (
            "ENDF-B-VIII.0_neutrons/n-009_F_019.endf".to_string(),
            f19.clone(),
        ),
        ("ENDF-B-VIII.0_neutrons/n-008_O_016.endf".to_string(), o16),
        (
            "ENDF-B-VIII.0_thermal_scatt/tsl-HinH2O.endf".to_string(),
            water,
        ),
        (
            "ENDF-B-VIII.0_neutrons/junk.endf".to_string(),
            b"not a tape".to_vec(),
        ),
    ];
    let zip = write_zip(&files).unwrap();
    let ix = EndfLibraryIndex::from_zip_bytes(zip.clone()).unwrap();
    assert_eq!(ix.layout(), LibraryLayout::NndcSubLibraryArchives);
    assert_eq!(ix.named_library(), Some(EndfLibrary::EndfBVIII0));
    assert_eq!(ix.tapes().len(), 3);
    assert_eq!(ix.skipped().len(), 2);
    let main = ix.main_library().unwrap();
    let t = ix.incident_neutron(&main, 9, 19, 0).unwrap();
    assert_eq!(ix.read_bytes(t).unwrap(), f19);
    assert_eq!(ix.read_tape(t).unwrap().materials(), vec![925]);
    assert_eq!(
        ix.thermal(&main, 1).unwrap().display_name(),
        ix.thermal(&main, 1).unwrap().header.symbol
    );
    // The same files dropped one by one, and the zip dropped as one file.
    let up = EndfLibraryIndex::from_files(files.clone()).unwrap();
    assert_eq!(up.tapes().len(), 3);
    assert!(matches!(up.source(), LibrarySource::Files { count: 5 }));
    let one = EndfLibraryIndex::from_files(vec![("lib.zip".into(), zip)]).unwrap();
    assert_eq!(one.tapes().len(), 3);
    assert_eq!(one.layout(), LibraryLayout::NndcSubLibraryArchives);
    // The IAEA tree: one zip per tape, inside a zip.
    let inner = write_zip(&[("n_0925_9-F-19.dat".into(), f19.clone())]).unwrap();
    let tree = write_zip(&[
        ("n/n_0925_9-F-19.zip".into(), inner),
        ("n/broken.zip".into(), b"PK\x03\x04".to_vec()),
    ])
    .unwrap();
    let iaea = EndfLibraryIndex::from_zip_bytes(tree).unwrap();
    assert_eq!(iaea.layout(), LibraryLayout::IaeaPerNuclide);
    let t = iaea
        .incident_neutron(&iaea.main_library().unwrap(), 9, 19, 0)
        .unwrap();
    assert_eq!(iaea.read_bytes(t).unwrap(), f19);
    assert!(iaea
        .skipped()
        .iter()
        .any(|s| s.reason.contains("unreadable zip")));
    // Malformed: an error, never a panic.
    assert!(EndfLibraryIndex::from_zip_bytes(b"PK\x03\x04 nothing".to_vec()).is_err());
    let empty = EndfLibraryIndex::from_files(vec![("x.dat".into(), vec![0xff; 10])]).unwrap();
    assert!(empty.main_library().is_none() && empty.tapes().is_empty());
}

/// **The fuzzy matcher**: substring above subsequence, out-of-order refused,
/// case-insensitive.
#[test]
fn fuzzy_matching_prefers_substrings() {
    assert!(fuzzy_score("235", "u235").unwrap() > fuzzy_score("u35", "u235").unwrap());
    assert_eq!(fuzzy_score("53u", "u235"), None);
    assert_eq!(fuzzy_score("U235", "u235"), fuzzy_score("u235", "U235"));
    assert_eq!(fuzzy_score("  ", "anything"), Some(0));
}

/// **Browser generation (#936)**: F-19 at 293.6 K with its 0 K companion,
/// read back by header through [`AceSet`]; the companion can be ablated; the
/// H-in-H2O thermal table from the compiled-in LEAPR deck on the library's
/// own H-1.
#[test]
fn generated_ace_reads_back_by_header() {
    let Some(dir) = reference() else {
        eprintln!("SKIP: no reference-data/endf");
        return;
    };
    let ix = EndfLibraryIndex::from_folder(&dir).unwrap();
    let main = ix.main_library().unwrap();
    let f19 = ix.incident_neutron(&main, 9, 19, 0).unwrap();
    let t = Temperature::new::<kelvin>(293.6);
    let out = generate_ace(&ix, f19, &AceRequest::new(t)).unwrap();
    let paths: Vec<&str> = out.iter().map(|g| g.path.as_str()).collect();
    assert_eq!(paths, ["293.6K/F-19.ace", "0K/F-19.ace"]);
    assert_eq!(out[0].zaid, "9019.00c");
    let set = AceSet::from_files(
        out.iter()
            .map(|g| (g.path.clone(), g.bytes.clone()))
            .collect(),
    );
    assert_eq!(set.neutron(9019, 293.6).unwrap().file, "293.6K/F-19.ace");
    assert_eq!(set.zero_kelvin(9019).unwrap().file, "0K/F-19.ace");
    let alone = generate_ace(
        &ix,
        f19,
        &AceRequest::new(t).without_zero_kelvin_companion(),
    )
    .unwrap();
    assert_eq!(alone.len(), 1);
    assert_eq!(
        alone[0].bytes, out[0].bytes,
        "the table does not depend on the companion"
    );
    let custom = AceRequest::new(t).with_settings(crate::process::CrossSectionSettings::default());
    assert_eq!(
        generate_ace(&ix, f19, &custom.without_zero_kelvin_companion()).unwrap()[0].bytes,
        out[0].bytes
    );
    assert!(generate_ace(&ix, ix.thermal(&main, 1).unwrap(), &AceRequest::new(t)).is_err());
    assert_eq!(
        temperature_folder(Temperature::new::<kelvin>(600.0)),
        "600K"
    );
    let h1 = suggest_thermal_partner(&ix, &main, SabMaterial::HInH2O).unwrap();
    assert_eq!((h1.header.za, h1.display_name().as_str()), (1001, "H-1"));
    let law =
        generate_thermal_ace(&ix, SabMaterial::HInH2O, h1, t, &ThermalSettings::default()).unwrap();
    assert_eq!(
        law.path,
        format!("293.6K/{}.ace", SabMaterial::HInH2O.label())
    );
    let back = AceSet::from_files(vec![(law.path.clone(), law.bytes)]);
    let stem = law.zaid.split('.').next().unwrap().to_string();
    assert_eq!(back.thermal(&stem, 293.6).unwrap().file, law.path);
    assert!(generate_thermal_ace(
        &ix,
        SabMaterial::HInH2O,
        ix.thermal(&main, 1).unwrap(),
        t,
        &ThermalSettings::default()
    )
    .is_err());
}
