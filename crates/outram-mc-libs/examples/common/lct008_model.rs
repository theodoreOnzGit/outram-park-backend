// SPDX-License-Identifier: AGPL-3.0-only

//! **The std glue around the LEU-COMP-THERM-008 model.** The model itself
//! (cards, materials, nested-lattice CSG, self-check) is in the library since
//! 2026-11-17: [`outram_mc_libs::vv::lct008`], `no_std`, re-exported here
//! whole. What stays is what needs `std`: the process-wide `--case` switch
//! ([`ACTIVE_CASE`]), the stashed lattice bookkeeping ([`MODEL`]) and the
//! lines the examples print. The call sites of `lct008_keff.rs`,
//! `lct008_ace_roundtrip.rs`, `lct008_pitch_sweep.rs`,
//! `icsbep_five_route_keff.rs`, `tutorial_lct008.rs` and `dhoby-ghaut`'s web
//! rung are unchanged.
//!
//! Each pulls it in with `#[path = "common/lct008_model.rs"] mod
//! lct008_model;`. A subdirectory without `main.rs` is not an example target,
//! so Cargo does not build this file on its own.
//!
//! Main/CLI glue: printing and process state, reached by the examples above
//! and by `tests/model_lct008_tutorial.rs`'s V&V model build.

#![allow(dead_code)]

pub use outram_mc_libs::vv::lct008::*;
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::vv::lct008 as lib;
use std::collections::BTreeMap;

/// The committed case, selected by `--case` (1 when unset). See [`Case`].
pub static ACTIVE_CASE: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

/// The active case.
pub fn active_case() -> Case {
    Case::from_number(ACTIVE_CASE.get().copied().unwrap_or(1))
}

/// The active case's `materials.xml`.
pub fn materials_xml() -> &'static str {
    active_case().materials_xml()
}

/// The active case's `geometry.xml`.
pub fn geometry_xml() -> &'static str {
    active_case().geometry_xml()
}

/// The lattice bookkeeping of the first build, for [`check_geometry`].
pub static MODEL: std::sync::OnceLock<Model> = std::sync::OnceLock::new();

/// [`lib::build_geometry_and_model`] for the active case. The first build's
/// [`Model`] is kept for [`check_geometry`]; the canonical build
/// (`reverse_rows`) prints its summary line.
pub fn build_geometry(materials: &[Material], reverse_rows: bool) -> Geometry {
    let (geom, model) = lib::build_geometry_and_model(active_case(), materials, reverse_rows);
    let _ = MODEL.set(model);
    if reverse_rows {
        eprintln!("{}", geometry_summary(&geom));
    }
    geom
}

/// [`lib::check_geometry`] on the active case, its two lines printed;
/// returns the material volume shares.
pub fn check_geometry(geom: &Geometry, materials: &[Material]) -> Vec<f64> {
    let check = lib::check_geometry(active_case(), geom, MODEL.get().expect("model"), materials);
    for line in check.report(materials) {
        eprintln!("{line}");
    }
    check.shares
}

/// [`omissions_report`], printed.
pub fn report_omissions(spec: &[MaterialSpec], omitted: &BTreeMap<String, f64>) {
    for line in omissions_report(spec, omitted) {
        eprintln!("{line}");
    }
}
