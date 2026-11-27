// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. Written for this workspace.

//! The GeN-Foam port's number readers round a decimal as OpenFOAM-ESI does
//! (gh:#1909, README round 53 of `genfoam_vs_upstream_gh1361`).
//!
//! # Methodology
//!
//! `0.8417393185` is read by ESI `readDouble` (`strtold` narrowed to `double`)
//! as `0.84173931849999994`, one ulp below the correctly rounded
//! `0.8417393185` (`foam_number_vs_strtold`, round 49). A temporary case is
//! written with that text in a `0/` field (uniform and nonuniform, scalar and
//! vector); each `field_reader` internal-field reader must return the ESI
//! double, never the correctly rounded one. Pass criterion: bit equality with
//! `parse_foam_f64`. The dictionary entry points (`FoamFile::*_esi`) and
//! basic-lib's `read_vol_*_field_esi` are pinned in basic-lib's
//! `tests/io_roundtrip.rs`; the boundary-value payload site
//! (`parse_uniform_scalar_payload`) needs a mesh and is covered only by the
//! whole-suite re-run (README round 53), not by a dedicated assertion.
//!
//! # Results (2026-11-27, release)
//!
//! The four `field_reader` internal-field forms return the ESI double; the
//! correctly rounded value differs from it by exactly one ulp, so the test
//! fails if any site goes back to `str::parse`.

use std::path::PathBuf;

use outram_foam_appbuilder_lib::io::field_reader::{read_vol_scalar_field, read_vol_vector_field};
use outram_foam_basic_lib::io::parse_foam_f64;

const SPLIT: &str = "0.8417393185";

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("op_r53_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn header(class: &str) -> String {
    format!("FoamFile {{ version 2.0; format ascii; class {class}; object f; }}\n")
}

#[test]
fn field_reader_internal_fields_are_read_as_esi_does() {
    let esi = parse_foam_f64(SPLIT).unwrap();
    let right: f64 = SPLIT.parse().unwrap();
    assert_ne!(esi, right, "the probe number must split the two roundings");
    let d = tmp("field");

    let p = d.join("s_uniform");
    std::fs::write(&p, format!("{}internalField uniform {SPLIT};\n", header("volScalarField"))).unwrap();
    assert_eq!(read_vol_scalar_field(&p, 2).unwrap(), vec![esi, esi]);

    let p = d.join("s_nonuniform");
    std::fs::write(&p, format!("{}internalField nonuniform List<scalar> 2({SPLIT} 1);\n", header("volScalarField"))).unwrap();
    assert_eq!(read_vol_scalar_field(&p, 2).unwrap(), vec![esi, 1.0]);

    let p = d.join("v_uniform");
    std::fs::write(&p, format!("{}internalField uniform ({SPLIT} 0 0);\n", header("volVectorField"))).unwrap();
    let v = read_vol_vector_field(&p, 1).unwrap();
    assert_eq!(v[0].x, esi);

    let p = d.join("v_nonuniform");
    std::fs::write(&p, format!("{}internalField nonuniform List<vector> 1(({SPLIT} 0 0));\n", header("volVectorField"))).unwrap();
    let v = read_vol_vector_field(&p, 1).unwrap();
    assert_eq!(v[0].x, esi);
    let _ = std::fs::remove_dir_all(&d);
}
