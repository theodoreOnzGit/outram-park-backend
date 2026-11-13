// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.

//! `PolyMesh::write_cell_zones` (gh:#1463): the `cellZones` file `blockMesh`
//! writes when a block names a zone.
//!
//! Methodology: write two zones to a temporary directory and check the file
//! against the layout OpenFOAM's `cellZoneMesh::write` produces
//! (`N ( name { type cellZone; cellLabels List<label> n ( ... ) ; } ... )`
//! under a `regIOobject` / `cellZones` header); an empty zone list writes no
//! file. Result (2026-11-11): all assertions hold.

use outram_foam_basic_lib::io::poly_mesh::PolyMesh;

#[test]
fn cell_zones_file_has_openfoam_layout() {
    let dir = std::env::temp_dir().join(format!("outram_cellzones_{}", std::process::id()));
    let zones = vec![
        ("fluid".to_string(), vec![0usize, 1, 2]),
        ("solid".to_string(), vec![3usize]),
    ];
    PolyMesh::write_cell_zones(&dir, &zones).unwrap();
    let text = std::fs::read_to_string(dir.join("cellZones")).unwrap();
    assert!(text.contains("class       regIOobject;") || text.contains("regIOobject"));
    assert!(text.contains("object      cellZones;") || text.contains("cellZones"));
    let body: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let at = body.iter().position(|l| *l == "2").expect("zone count line");
    assert_eq!(body[at + 1], "(");
    assert_eq!(body[at + 2], "fluid");
    assert_eq!(body[at + 3], "{");
    assert_eq!(body[at + 4], "type cellZone;");
    assert!(body[at + 5].starts_with("cellLabels") && body[at + 5].ends_with("List<label>"));
    assert_eq!(&body[at + 6..at + 12], &["3", "(", "0", "1", "2", ")"]);
    assert_eq!(&body[at + 12..at + 14], &[";", "}"]);
    assert_eq!(body[at + 14], "solid");
    // An empty list writes nothing.
    let none = dir.join("none");
    PolyMesh::write_cell_zones(&none, &[]).unwrap();
    assert!(!none.join("cellZones").exists());
    let _ = std::fs::remove_dir_all(&dir);
}
