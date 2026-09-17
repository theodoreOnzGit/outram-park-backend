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

//! # Sphere in a box — geometry to OpenFOAM mesh, end to end
//!
//! Run it:
//!
//! ```text
//! cargo run --release -p outram-foam-mesh --example sphere_in_box [output_dir]
//! ```
//!
//! What it does, in order:
//!
//! 1. Builds a **unit sphere** as a closed triangulated surface (no STL file
//!    needed — the geometry is generated in code).
//! 2. Meshes the region **outside** the sphere and inside a `[-2,2]³` box —
//!    i.e. a box with a spherical hole, the classic external-flow domain — by
//!    calling one function, `mesh_from_surface`.
//! 3. Prints a `checkMesh`-style **quality report**: cell count, max/mean
//!    non-orthogonality in degrees, skewness, aspect ratio, inverted cells.
//! 4. Writes `constant/polyMesh/{points,faces,owner,neighbour,boundary}` to
//!    disk, readable by OpenFOAM and by this workspace's own case reader.
//! 5. Runs the pipeline a second time **with boundary layers** and reports,
//!    honestly, which of the two layer placements was actually used — the
//!    layer phase in this crate is restricted (see the printout).
//!
//! Everything the example needs is in this file; you do not have to open any
//! other source file to follow it.
//!
//! ## Measured output (2026-08-07, release, x86_64)
//!
//! Stage 1 — castellate + snap, 12×12×12 background, refinement level 2:
//! **6128 cells**, 9736 points, 21 960 faces; max non-orthogonality
//! **41.80°** (mean 9.58°, 0 faces above 70°); max skewness **0.668**; max
//! aspect ratio **1.965**; **0 inverted cells**; mesh volume **59.8594 m³**
//! against the analytic `64 − 4π/3 = 59.8112 m³`, i.e. **+0.081 %**. Verdict
//! **GOOD**.
//!
//! Stage 2 — same plus 3 boundary layers: 6128 → **14 192 cells**, outcome
//! **`InsertedInterior`** (total volume unchanged at 59.8594 m³, so the prisms
//! really were carved out of the near-wall cells rather than extruded into new
//! space). Quality degrades though: max non-orthogonality **77.86°** with 32
//! faces above 70°, max skewness **2.61**, max aspect ratio **4.96**, smallest
//! cell **8.2e-7 m³** — verdict **MARGINAL**. On a coarser background
//! (8×8×8) the same request instead returns `ExtrudedOutward`, which grows the
//! domain and is not the mesh you asked for. Always read the reported outcome.

use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_mesh::snappy_hex_mesh::{Bounds, LayerControls, TriangleSoup};
use outram_foam_mesh::{mesh_from_surface, LayerOutcome, MeshingControls, MeshingPhases};

fn main() {
    // Where to write the case. `target/` keeps generated output out of the way.
    let out_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/outram-foam-mesh-example/sphere_in_box".to_string());

    // ── 1. The geometry ─────────────────────────────────────────────────────
    // A closed, outward-wound unit sphere at the origin, tessellated with
    // 32 latitude bands × 32 longitude sectors (2048 facets). This is the
    // "hole": the mesher will carve it out of the box.
    let sphere = TriangleSoup::uv_sphere(Vector3::ZERO, 1.0, 32, 32);
    println!("Geometry: unit sphere, {} triangles", sphere.len());

    // ── 2. The meshing controls ─────────────────────────────────────────────
    // External flow: keep everything OUTSIDE the sphere. The background box is
    // [-2,2]³ split 12×12×12, so a far-field cell is 1/3 m across; refinement
    // level 2 makes the cells touching the sphere 4× finer (1/12 m).
    //
    // Phases are picked by one enum. `CastellateSnap` = octree-refine and carve
    // the box, then morph the carved staircase onto the true sphere.
    let domain = Bounds::new(Vector3::new(-2.0, -2.0, -2.0), Vector3::new(2.0, 2.0, 2.0));
    let controls = MeshingControls::external_flow([12, 12, 12], 2)
        .with_domain(domain)
        .with_phases(MeshingPhases::CastellateSnap);

    println!(
        "Domain:   {:?} .. {:?} m, background {}x{}x{}, refinement level {}",
        domain.min,
        domain.max,
        controls.background_divisions[0],
        controls.background_divisions[1],
        controls.background_divisions[2],
        controls.refinement_level,
    );

    // ── 3. Mesh it — the one call ───────────────────────────────────────────
    println!("\nRunning castellate -> snap ...");
    let result = match mesh_from_surface(&sphere, &controls) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("meshing failed: {e}");
            std::process::exit(1);
        }
    };

    // ── 4. Quality report ───────────────────────────────────────────────────
    println!("\nMesh quality (OpenFOAM checkMesh definitions):");
    print!("{}", result.quality.summary());

    // Independent sanity check: the meshed volume is the box minus the sphere.
    let analytic = 4.0 * 4.0 * 4.0 - 4.0 / 3.0 * std::f64::consts::PI * 1.0f64.powi(3);
    let err_pct = 100.0 * (result.quality.total_volume - analytic) / analytic;
    println!(
        "\nVolume check: meshed {:.4} m^3 vs analytic box-sphere {:.4} m^3  ({err_pct:+.3} %)",
        result.quality.total_volume, analytic
    );

    // What the numbers mean, in one line each.
    println!(
        "\n  Non-orthogonality above {:.0} deg is what OpenFOAM warns about; this mesh peaks at {:.1} deg.",
        outram_foam_mesh::mesh_quality::NON_ORTHO_SEVERE_DEG,
        result.quality.max_non_ortho_deg
    );
    println!(
        "  Skewness above {:.0} fails checkMesh; this mesh peaks at {:.3}.",
        outram_foam_mesh::mesh_quality::SKEWNESS_LIMIT,
        result.quality.max_skewness
    );
    println!(
        "  Any inverted cell is fatal; this mesh has {}.",
        result.quality.n_negative_volume_cells
    );

    // ── 5. Write the OpenFOAM case ──────────────────────────────────────────
    if let Err(e) = result.write_case(&out_dir) {
        eprintln!("could not write the case: {e}");
        std::process::exit(1);
    }
    println!("\nWrote {out_dir}/constant/polyMesh/{{points,faces,owner,neighbour,boundary}}");
    println!("(mesh only — add system/ dictionaries and 0/ fields before running a solver)");

    // Read it straight back to prove the files are a real, parseable polyMesh.
    let poly_dir = std::path::Path::new(&out_dir)
        .join("constant")
        .join("polyMesh");
    match outram_foam_basic_lib::io::PolyMesh::read(&poly_dir) {
        Ok(reread) => println!(
            "Read back OK: {} points, {} faces, {} cells, {} patch(es)",
            reread.points.len(),
            reread.faces.len(),
            reread.n_cells,
            reread.patches.len()
        ),
        Err(e) => eprintln!("WARNING: could not read the mesh back: {e}"),
    }

    // ── 6. The layer phase, reported honestly ───────────────────────────────
    // Boundary layers are Phase 3, and this crate implements them in a
    // restricted form with two possible placements:
    //   - InsertedInterior: prisms are carved out of the near-wall cells, the
    //     wall stays put, and the meshed volume is conserved. Correct.
    //   - ExtrudedOutward:  prisms grow outward along the wall normal, so the
    //     meshed domain gets bigger. Watertight, but not the mesh you asked
    //     for. This is the fallback where displacing a wall point would open
    //     an octree hanging-node gap.
    // Which one you get is decided inside the layer phase, not by you. The
    // driver detects it from the change in total meshed volume and reports it.
    println!("\nRunning castellate -> snap -> layers (3 layers, ratio 1.2, first 0.02 m) ...");
    let layer_controls = LayerControls {
        n_surface_layers: 3,
        expansion_ratio: 1.2,
        first_layer_thickness: 0.02,
        ..LayerControls::default()
    };
    let layered_controls = controls
        .clone()
        .with_phases(MeshingPhases::CastellateSnapLayers)
        .with_layers(layer_controls);

    match mesh_from_surface(&sphere, &layered_controls) {
        Ok(layered) => {
            println!(
                "  cells {} -> {} (castellated {})",
                result.quality.n_cells, layered.quality.n_cells, layered.n_cells_castellated
            );
            match layered.layers {
                LayerOutcome::InsertedInterior => println!(
                    "  outcome: InsertedInterior — the prisms were carved out of the near-wall\n  \
                     cells, the wall did not move, and the meshed volume is conserved.\n  \
                     This is the correct placement. Note the quality cost below, though:\n  \
                     thin near-wall prisms push non-orthogonality and aspect ratio up."
                ),
                LayerOutcome::ExtrudedOutward => println!(
                    "  outcome: ExtrudedOutward — the prism block grew OUTWARD and the domain got bigger.\n  \
                     This is the crate's documented fallback for octree hanging-node regions.\n  \
                     The mesh is watertight and inversion-free, but it is NOT the external-flow\n  \
                     domain you asked for. Do not treat it as validated."
                ),
                LayerOutcome::NoneAdded => println!(
                    "  outcome: NoneAdded — every candidate layer count failed the quality gate,\n  \
                     so the unlayered mesh was returned unchanged."
                ),
                LayerOutcome::NotRequested => unreachable!("layers were requested"),
            }
            print!("{}", layered.quality.summary());
        }
        Err(e) => println!("  layer phase failed: {e}"),
    }
}
