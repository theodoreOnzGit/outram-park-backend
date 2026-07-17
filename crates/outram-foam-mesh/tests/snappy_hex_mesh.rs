// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; see the GNU General Public License for more details.

//! Verification tests for the `snappyHexMesh` Phase-1 slice (bead op-ax7.2):
//! STL input + castellation. Snapping / layer addition are scaffolded, so only
//! their implemented sub-pieces (projection, grading arithmetic) and their
//! honest `NotImplemented` returns are asserted.
//!
//! ## Methodology
//!
//! A unit-radius UV-sphere STL (`sphere_soup`, R = 1 m at the origin) is used as
//! the refinement surface inside an `[-2,2]³` background box meshed `8×8×8`
//! (level-0 cell = 0.5 m). Castellation refines to level 2 (finest cell 0.125 m)
//! and keeps the region OUTSIDE the sphere (external-flow domain). The output
//! `FvMesh` is checked for:
//!
//! - **Topological validity** — `FvMesh::validate()` passes (patch coverage,
//!   array lengths).
//! - **Watertightness** — the signed face-area vectors of every cell sum to
//!   ~0 (a closed control volume); this is the strong correctness gate for the
//!   split-hex hanging-node face emission.
//! - **Refinement actually happened** — cells exist at both level 0 (far field)
//!   and level 2 (surface shell), and the refined mesh has many more cells than
//!   the 512-cell uniform background.
//! - **Region removal is correct** — no kept cell centre lies deep inside the
//!   sphere; the retained volume ≈ box − sphere.
//!
//! ## Results (recorded 2026-07-17, first green run — see the assertions/prints)
//!
//! - Mesh validates; per-cell area-vector closure max |Σ Sf| = 0.0 m² exactly
//!   (axis-aligned split-hex faces cancel to the bit).
//! - Cells grow from 512 (uniform) to 2312 after refinement + removal, split
//!   `cells_by_level = [424, 224, 1664]` (mixed → refinement localised at the
//!   surface); 7020 internal faces, 8652 total, 1248 carved-surface faces.
//! - Retained volume = 59.75 m³ vs analytic box−sphere = 64 − 4.189 = 59.81 m³
//!   (staircase agreement ~0.1%). Keep-inside variant: 4.25 m³ vs sphere 4.19.
//! - `snap` / `add_layers` return `MeshError::NotImplemented`; the projection
//!   and grading sub-pieces they expose compute correctly.

use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_mesh::snappy_hex_mesh::{
    background::{BackgroundMesh, Bounds},
    castellation::{castellate, CastellationControls},
    layers::{add_layers, layer_thicknesses, total_layer_thickness, LayerControls},
    snapping::{raw_surface_displacements, snap, SnapControls},
    stl::{read_stl_ascii_str, read_stl_binary, Triangle, TriangleSoup},
};
use outram_foam_mesh::MeshError;

// ── Fixtures ────────────────────────────────────────────────────────────────

/// A closed UV sphere of radius `r` centred at `c`, as a triangle soup.
fn sphere_soup(c: Vector3, r: f64, n_lat: usize, n_lon: usize) -> TriangleSoup {
    let mut tris = Vec::new();
    let point = |ia: f64, io: f64| -> Vector3 {
        let theta = std::f64::consts::PI * ia; // 0..pi (lat)
        let phi = 2.0 * std::f64::consts::PI * io; // 0..2pi (lon)
        Vector3::new(
            c.x + r * theta.sin() * phi.cos(),
            c.y + r * theta.sin() * phi.sin(),
            c.z + r * theta.cos(),
        )
    };
    for i in 0..n_lat {
        for j in 0..n_lon {
            let a0 = i as f64 / n_lat as f64;
            let a1 = (i + 1) as f64 / n_lat as f64;
            let o0 = j as f64 / n_lon as f64;
            let o1 = (j + 1) as f64 / n_lon as f64;
            let p00 = point(a0, o0);
            let p01 = point(a0, o1);
            let p10 = point(a1, o0);
            let p11 = point(a1, o1);
            // Two triangles per quad (winding gives outward normals).
            tris.push(Triangle::new(p00, p11, p01));
            tris.push(Triangle::new(p00, p10, p11));
        }
    }
    TriangleSoup::new("sphere", tris)
}

/// An ASCII STL of a unit tetrahedron (4 facets) for reader coverage.
const TETRA_ASCII_STL: &str = "\
solid tetra
  facet normal 0 0 0
    outer loop
      vertex 0 0 0
      vertex 1 0 0
      vertex 0 1 0
    endloop
  endfacet
  facet normal 0 0 0
    outer loop
      vertex 0 0 0
      vertex 0 1 0
      vertex 0 0 1
    endloop
  endfacet
  facet normal 0 0 0
    outer loop
      vertex 0 0 0
      vertex 0 0 1
      vertex 1 0 0
    endloop
  endfacet
  facet normal 0 0 0
    outer loop
      vertex 1 0 0
      vertex 0 0 1
      vertex 0 1 0
    endloop
  endfacet
endsolid tetra
";

// ── STL reader tests ─────────────────────────────────────────────────────────

#[test]
fn ascii_stl_parses_all_facets() {
    let soup = read_stl_ascii_str(TETRA_ASCII_STL).expect("parse tetra");
    assert_eq!(soup.name, "tetra");
    assert_eq!(soup.len(), 4);
    let (lo, hi) = soup.bounding_box().unwrap();
    assert_eq!(lo, Vector3::new(0.0, 0.0, 0.0));
    assert_eq!(hi, Vector3::new(1.0, 1.0, 1.0));
}

#[test]
fn binary_stl_roundtrips_one_triangle() {
    // Assemble a minimal binary STL: 80-byte header, count=1, one facet.
    let mut bytes = vec![0u8; 84 + 50];
    bytes[80..84].copy_from_slice(&1u32.to_le_bytes());
    let verts = [
        [0.0f32, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        [0.0, 3.0, 0.0],
    ];
    let base = 84 + 12; // skip 3-float normal
    for (v, vert) in verts.iter().enumerate() {
        for (k, comp) in vert.iter().enumerate() {
            let off = base + v * 12 + k * 4;
            bytes[off..off + 4].copy_from_slice(&comp.to_le_bytes());
        }
    }
    let soup = read_stl_binary(&bytes).expect("parse binary");
    assert_eq!(soup.len(), 1);
    let t = soup.triangles[0];
    assert_eq!(t.a, Vector3::new(0.0, 0.0, 0.0));
    assert_eq!(t.b, Vector3::new(2.0, 0.0, 0.0));
    assert_eq!(t.c, Vector3::new(0.0, 3.0, 0.0));
    // Area = 1/2 |AB x AC| = 1/2 * (2*3) = 3.
    assert!((t.area() - 3.0).abs() < 1e-6);
}

// ── Surface geometry tests ───────────────────────────────────────────────────

#[test]
fn inside_outside_of_sphere() {
    let soup = sphere_soup(Vector3::ZERO, 1.0, 24, 24);
    assert!(soup.contains_point(Vector3::ZERO), "centre is inside");
    assert!(
        soup.contains_point(Vector3::new(0.5, 0.1, -0.3)),
        "near-centre inside"
    );
    assert!(
        !soup.contains_point(Vector3::new(2.0, 0.0, 0.0)),
        "far point outside"
    );
    assert!(
        !soup.contains_point(Vector3::new(-1.9, -1.9, -1.9)),
        "corner outside"
    );
}

#[test]
fn nearest_surface_point_lands_on_sphere() {
    let soup = sphere_soup(Vector3::ZERO, 1.0, 48, 48);
    // A point outside along +x: nearest surface point ~ (1,0,0).
    let p = Vector3::new(3.0, 0.0, 0.0);
    let q = soup.nearest_point(p).unwrap();
    // With finite tessellation, allow a small chordal error.
    assert!((q.mag() - 1.0).abs() < 0.02, "nearest is ~on the sphere: {q:?}");
    assert!(soup.distance_to(p) > 1.9 && soup.distance_to(p) < 2.05);
}

#[test]
fn closest_point_on_triangle_regions() {
    let t = Triangle::new(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(1.0, 0.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
    );
    // Above the interior → projects straight down to the z=0 plane.
    let q = t.closest_point(Vector3::new(0.25, 0.25, 5.0));
    assert!((q - Vector3::new(0.25, 0.25, 0.0)).mag() < 1e-12);
    // Beyond vertex A → clamps to A.
    let q = t.closest_point(Vector3::new(-1.0, -1.0, 0.0));
    assert!((q - Vector3::ZERO).mag() < 1e-12);
    // Off edge AB → clamps onto the edge.
    let q = t.closest_point(Vector3::new(0.5, -1.0, 0.0));
    assert!((q - Vector3::new(0.5, 0.0, 0.0)).mag() < 1e-12);
}

// ── Castellation: the Phase-1 deliverable ────────────────────────────────────

/// Sum the signed face-area vectors per cell; a watertight mesh has all ~0.
fn max_cell_closure(mesh: &outram_foam_basic_lib::mesh::FvMesh) -> f64 {
    let mut acc = vec![Vector3::ZERO; mesh.n_cells];
    for f in 0..mesh.n_faces {
        let sf = mesh.face_area_vectors[f];
        acc[mesh.owner[f]] += sf;
        if f < mesh.n_internal_faces {
            let nb = mesh.neighbour[f];
            acc[nb] -= sf;
        }
    }
    acc.iter().map(|v| v.mag()).fold(0.0, f64::max)
}

#[test]
fn castellate_box_around_sphere() {
    let surface = sphere_soup(Vector3::ZERO, 1.0, 32, 32);
    let domain = Bounds::new(
        Vector3::new(-2.0, -2.0, -2.0),
        Vector3::new(2.0, 2.0, 2.0),
    );
    let background = BackgroundMesh::uniform(domain, 8, 8, 8);
    // Keep the region OUTSIDE the sphere.
    let keep_point = Vector3::new(-1.9, -1.9, -1.9);
    let controls = CastellationControls::new(background, 2, keep_point);

    let cast = castellate(&surface, &controls).expect("castellation succeeds");
    let mesh = &cast.fv_mesh;

    // (1) Topological validity.
    mesh.validate().expect("output FvMesh is valid");

    // (2) Watertightness — the split-hex face-emission correctness gate.
    let closure = max_cell_closure(mesh);
    eprintln!("max per-cell |Σ Sf| = {closure:.3e} m^2");
    assert!(closure < 1e-9, "cells must be closed control volumes");

    // (3) Refinement actually localised at the surface.
    eprintln!("cells_by_level = {:?}", cast.cells_by_level);
    eprintln!(
        "n_cells = {}, n_internal_faces = {}, n_faces = {}, surface_faces = {}",
        mesh.n_cells,
        mesh.n_internal_faces,
        mesh.n_faces,
        cast.surface_faces.len()
    );
    assert_eq!(cast.max_level, 2);
    assert!(cast.cells_by_level[0] > 0, "coarse far-field cells remain");
    assert!(cast.cells_by_level[2] > 0, "fine surface-shell cells created");
    assert!(
        mesh.n_cells > 512,
        "refinement grows cell count past the 512-cell background"
    );
    assert!(!cast.surface_faces.is_empty(), "carved surface has faces");

    // (4) Region removal correct: no kept cell centre is deep inside the sphere.
    for (c, centre) in mesh.cell_centres.iter().enumerate() {
        assert!(
            centre.mag() > 0.85,
            "kept cell {c} centre {centre:?} lies inside the sphere"
        );
    }

    // Retained volume ≈ box (64) − sphere (4.18879) = 59.81 m^3.
    let total_vol: f64 = mesh.cell_volumes.iter().sum();
    eprintln!("total retained volume = {total_vol:.4} m^3 (analytic ~59.81)");
    assert!(
        (56.0..62.0).contains(&total_vol),
        "retained volume {total_vol} out of expected staircase band"
    );
}

#[test]
fn castellate_keep_inside_carves_external() {
    // Keeping the sphere interior should retain far fewer cells than keeping
    // the exterior, and stay watertight.
    let surface = sphere_soup(Vector3::ZERO, 1.0, 24, 24);
    let domain = Bounds::new(
        Vector3::new(-2.0, -2.0, -2.0),
        Vector3::new(2.0, 2.0, 2.0),
    );
    let bg = BackgroundMesh::uniform(domain, 8, 8, 8);
    let controls = CastellationControls::new(bg, 2, Vector3::ZERO); // keep_point inside

    let cast = castellate(&surface, &controls).expect("castellate inside");
    cast.fv_mesh.validate().expect("valid");
    assert!(max_cell_closure(&cast.fv_mesh) < 1e-9);
    let vol: f64 = cast.fv_mesh.cell_volumes.iter().sum();
    eprintln!("kept-inside volume = {vol:.4} m^3 (analytic sphere ~4.19)");
    // Staircase sphere: within a factor of the analytic 4.19 m^3.
    assert!((3.0..6.0).contains(&vol), "interior volume {vol}");
}

// ── Phase 2/3 scaffolds: implemented sub-pieces + honest stubs ────────────────

#[test]
fn snapping_projection_and_stub() {
    let surface = sphere_soup(Vector3::ZERO, 1.0, 24, 24);
    let domain = Bounds::new(
        Vector3::new(-2.0, -2.0, -2.0),
        Vector3::new(2.0, 2.0, 2.0),
    );
    let bg = BackgroundMesh::uniform(domain, 8, 8, 8);
    let controls = CastellationControls::new(bg, 2, Vector3::new(-1.9, -1.9, -1.9));
    let cast = castellate(&surface, &controls).unwrap();

    // The projection sub-piece is real: surface points get a nonzero pull.
    let disp = raw_surface_displacements(&cast, &surface);
    assert_eq!(disp.len(), cast.points.len());
    let moved = disp.iter().filter(|d| d.mag() > 1e-9).count();
    assert!(moved > 0, "some surface points project onto the STL");

    // The full morph is an honest stub.
    let err = snap(&cast, &surface, &SnapControls::default()).unwrap_err();
    assert!(matches!(err, MeshError::NotImplemented(_)));
}

#[test]
fn layer_grading_and_stub() {
    let c = LayerControls {
        n_surface_layers: 3,
        expansion_ratio: 2.0,
        first_layer_thickness: 1.0,
    };
    // Geometric grading 1, 2, 4 → total 7.
    assert_eq!(layer_thicknesses(&c), vec![1.0, 2.0, 4.0]);
    assert!((total_layer_thickness(&c) - 7.0).abs() < 1e-12);

    // add_layers is an honest stub.
    let surface = sphere_soup(Vector3::ZERO, 1.0, 16, 16);
    let domain = Bounds::new(Vector3::new(-2.0, -2.0, -2.0), Vector3::new(2.0, 2.0, 2.0));
    let bg = BackgroundMesh::uniform(domain, 8, 8, 8);
    let ctrl = CastellationControls::new(bg, 1, Vector3::new(-1.9, -1.9, -1.9));
    let cast = castellate(&surface, &ctrl).unwrap();
    let err = add_layers(&cast, &c).unwrap_err();
    assert!(matches!(err, MeshError::NotImplemented(_)));
}
