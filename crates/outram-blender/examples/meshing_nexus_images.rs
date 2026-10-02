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
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! **Draw the meshes the meshing nexus produces** (GitHub #492; the
//! geometry-drawing HARD RULE in this crate's `CLAUDE.md`).
//!
//! Generates one mesh through each orchestrated mesher, converts it to the
//! neutral `UnstructuredMesh`, and draws slices of the neutral mesh itself
//! (cell cuts filled by zone, crossed faces in black) into
//! `crates/outram-blender/docs/meshing_nexus/`:
//!
//! | file | mesher | view |
//! |---|---|---|
//! | `fv_cfmesh_cylinder_xy.png` | cfMesh tet -> dual -> layers (`foam-mesh`) | x-y slice at mid-height |
//! | `fv_cfmesh_cylinder_xz.png` | same | x-z slice through the axis |
//! | `fv_blockmesh_box_xz.png` | blockMesh (`block-mesh`) | x-z slice |
//! | `fe_farrer_quarter_annulus.png` | farrer-park `quarter_annulus_quad4` (`fem-export`) | the 2-D mesh |
//! | `fe_farrer_box_tet4_xz.png` | farrer-park `box_tet4` | x-z slice |
//! | `one_d_column_xz.png` | the 1-D mesher (core) | x-z slice |
//!
//! Zones are assigned from cell centres (e.g. `inner` / `outer` by radius) so
//! the colouring shows the zone map the solver would see.
//!
//! ```text
//! cargo run --release -p outram-blender --example meshing_nexus_images \
//!     --features "block-mesh foam-mesh fem-export"
//! ```

use std::path::PathBuf;

use outram_blender::csg::plot::slice::PlotBasis;
use outram_blender::foam_mesh::{TetDualOptions, Vec3};
use outram_blender::unstructured::convert::{block_mesh, cfmesh, fem};
use outram_blender::unstructured::one_d::one_d_column;
use outram_blender::unstructured::{
    render_mesh_slice_annotated, LengthUnit, MeshColourBy, MeshSlice, UnstructuredMesh, Zone,
};

/// Split cells into two zones by a predicate on the cell centre.
fn two_zones(m: UnstructuredMesh, a: &str, b: &str, in_a: impl Fn([f64; 3]) -> bool) -> UnstructuredMesh {
    let (mut za, mut zb) = (Vec::new(), Vec::new());
    for c in 0..m.n_cells() {
        if in_a(m.cell_centre(c)) {
            za.push(c);
        } else {
            zb.push(c);
        }
    }
    m.with_zones(vec![Zone { name: a.into(), cells: za }, Zone { name: b.into(), cells: zb }])
        .expect("zones index existing cells")
}

fn save(m: &UnstructuredMesh, basis: PlotBasis, origin_cut: Option<f64>, title: &str, out: &PathBuf, name: &str) {
    let mut sl = MeshSlice::framing(m, basis, 700, MeshColourBy::Zone);
    if let Some(cut) = origin_cut {
        let (a, b) = basis.axes();
        sl.origin[3 - a - b] = cut;
    }
    let img = render_mesh_slice_annotated(m, &sl, title);
    let path = out.join(name);
    img.write_png(&path).expect("write png");
    println!(
        "wrote {} ({} cells, {} not star-shaped about their centroid, total volume {:.6e} {:?}^3)",
        path.display(),
        m.n_cells(),
        m.n_non_star_cells(),
        m.total_volume(),
        m.unit()
    );
}

fn main() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/meshing_nexus");
    std::fs::create_dir_all(&out).expect("create output dir");

    // ── FV: cfMesh tet-dual of a cylinder, r = 0.5 m, h = 1 m ──────────────
    let opts = TetDualOptions { cell_size: 0.1, first_layer_thickness: 0.01, ..Default::default() };
    let (vm, report) =
        outram_blender::foam_mesh::cylinder_tet_dual(Vec3::new(0.0, 0.0, 0.0), 0.5, 1.0, 32, &opts)
            .expect("cfMesh cylinder");
    println!("cfMesh report: valid = {}, total volume = {:.6} m^3", report.valid, report.total_volume);
    let fv = cfmesh::from_volume_mesh(&vm).expect("neutral from cfMesh");
    let fv = two_zones(fv, "inner r<0.3 m", "outer", |c| c[0].hypot(c[1]) < 0.3);
    save(&fv, PlotBasis::Xy, Some(52.0), "FV CFMESH TET-DUAL CYLINDER, Z = 52 CM", &out, "fv_cfmesh_cylinder_xy.png");
    save(&fv, PlotBasis::Xz, Some(1.3), "FV CFMESH TET-DUAL CYLINDER, Y = 1.3 CM", &out, "fv_cfmesh_cylinder_xz.png");

    // ── FV: blockMesh, two blocks with grading ──────────────────────────────
    let dict = r#"
convertToMeters 0.1;
vertices ( (0 0 0) (4 0 0) (4 1 0) (0 1 0) (0 0 2) (4 0 2) (4 1 2) (0 1 2)
           (6 0 0) (6 1 0) (6 0 2) (6 1 2) );
blocks ( hex (0 1 2 3 4 5 6 7) (8 2 6) simpleGrading (2 1 1)
         hex (1 8 9 2 5 10 11 6) (4 2 6) simpleGrading (0.5 1 1) );
edges ();
boundary ( inlet { type patch; faces ( (0 4 7 3) ); } outlet { type patch; faces ( (8 9 11 10) ); } );
"#;
    let bm = block_mesh::block_mesh(dict).expect("blockMesh");
    let bm = two_zones(bm, "block 1 (x<0.4 m)", "block 2", |c| c[0] < 0.4);
    save(&bm, PlotBasis::Xz, Some(3.0), "FV BLOCKMESH, TWO GRADED BLOCKS, Y = 3 CM", &out, "fv_blockmesh_box_xz.png");

    // ── FE: farrer-park generators (they stay in farrer-park) ───────────────
    let (qa, _) = fem::generate(fem::FemGenerator::QuarterAnnulusQuad4 { r_inner: 0.1, r_outer: 0.2, n_r: 6, n_theta: 12 })
        .expect("quarter annulus");
    let qa = two_zones(qa, "inner half r<0.15 m", "outer half", |c| c[0].hypot(c[1]) < 0.15);
    save(&qa, PlotBasis::Xy, None, "FE FARRER-PARK QUARTER ANNULUS QUAD4 (2-D)", &out, "fe_farrer_quarter_annulus.png");

    let (bt, _) = fem::generate(fem::FemGenerator::BoxTet4 { size: [0.3, 0.2, 0.2], divisions: [3, 2, 2] }).expect("box tet4");
    let bt = two_zones(bt, "x<0.15 m", "x>0.15 m", |c| c[0] < 0.15);
    save(&bt, PlotBasis::Xz, Some(4.3), "FE FARRER-PARK BOX TET4 (KUHN), Y = 4.3 CM", &out, "fe_farrer_box_tet4_xz.png");

    // ── The 1-D mesher ──────────────────────────────────────────────────────
    let col = one_d_column(LengthUnit::Metre, 1.0, 0.01, 10).expect("1-D column");
    let col = two_zones(col, "x<0.5 m", "x>0.5 m", |c| c[0] < 0.5);
    save(&col, PlotBasis::Xz, Some(0.1), "1-D COLUMN, 10 HEX8, Y = 0.1 CM", &out, "one_d_column_xz.png");
}
