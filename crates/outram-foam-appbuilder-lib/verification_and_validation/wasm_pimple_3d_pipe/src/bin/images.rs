//! Draws the meshes the solver gets (the neutral mesh that `to_fv` converts),
//! for the human check the geometry-drawing rule asks for. `main` glue (file
//! output); the meshes come from the library functions the tests reach.
//!
//!   images OUT_DIR
use foam_wasm_pipe::*;
use outram_blender::csg::plot::slice::PlotBasis;
use outram_blender::unstructured::{render_mesh_slice_annotated, MeshColourBy, MeshSlice, UnstructuredMesh};
use std::path::{Path, PathBuf};

/// One slice; `origin` and `width` in cm.
fn save(m: &UnstructuredMesh, basis: PlotBasis, origin: [f64; 3], width: [f64; 2], px: usize, title: &str, out: &Path, name: &str) {
    let py = ((px as f64) * width[1] / width[0]).round() as usize;
    let sl = MeshSlice { basis, origin, width, pixels: [px, py], colour_by: MeshColourBy::Cell };
    let img = render_mesh_slice_annotated(m, &sl, title);
    img.write_png(&out.join(name)).expect("write png");
    println!("wrote {name}: {} cells, volume {:.6e} m3", m.n_cells(), m.total_volume());
}

fn draw(m: &UnstructuredMesh, tag: &str, label: &str, out: &Path) {
    // Cut planes sit off the grids' own planes (a cut in a face plane is degenerate).
    save(m, PlotBasis::Xy, [0.0, 0.0, 10.13], [2.2, 2.2], 900, &format!("{label}, X-Y AT Z = 10.13 CM"), out, &format!("{tag}_xy.png"));
    save(m, PlotBasis::Xz, [0.0, 0.013, 2.0], [2.2, 4.4], 500, &format!("{label}, X-Z AT Y = 0.013 CM, INLET END"), out, &format!("{tag}_xz_inlet.png"));
    save(m, PlotBasis::Xz, [0.0, 0.013, 10.0], [2.4, 21.0], 240, &format!("{label}, X-Z, FULL LENGTH"), out, &format!("{tag}_xz_full.png"));
}

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).expect("output dir"));
    std::fs::create_dir_all(&out).expect("create output dir");
    for (g, tag) in [(OGrid { n: 6, nr: 4, nz: 40 }, "ogrid_5k"), (OGrid { n: 8, nr: 6, nz: 80 }, "ogrid_20k")] {
        let m = mesh_ogrid(&g);
        draw(&m, tag, &format!("O-GRID {} CELLS", m.n_cells()), &out);
    }
    for (cs, tag) in [(0.004, "cfmesh_5k"), (0.0026, "cfmesh_20k")] {
        let (m, _) = mesh_cfmesh(cs, 0, 48);
        draw(&m, tag, &format!("CFMESH TET-DUAL {} CELLS", m.n_cells()), &out);
    }
    let (m, _) = mesh_cfmesh(0.008, 3, 48);
    draw(&m, "cfmesh_layers_8k", &format!("CFMESH + 3 LAYERS {} CELLS", m.n_cells()), &out);
}
