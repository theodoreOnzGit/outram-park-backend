//! **Foam Studio** (gh:#1155, stage 1 of epic gh:#1153; the double-pipe heat
//! exchanger is stage 2, gh:#1156; the helical coil section, with its Dean
//! vortices and friction against Mishra & Gupta (1979), is gh:#1157): put a
//! cylinder, a box, a double pipe or a helical coil
//! in a 3-D viewport, move and scale it, tag its faces inlet, outlet or
//! wall, mesh it, and run OUTRAM PARK's pimpleFoam port on it in the
//! reader's browser, with velocity and pressure on two slices, the residual
//! history, a probe and the velocity profile against Hagen–Poiseuille.
//! Built on `bugis::web_demo` (panel, dock, worker link, field and residual
//! painting); published at `demos/foam-studio/` (`web/foam_studio/`).
//!
//! ```text
//! cargo run  -p dhoby-ghaut --example foam_studio_web --release
//! cargo run  -p dhoby-ghaut --example foam_studio_web --release -- --headless [cells] [re] [steady|transient]
//! cargo run  -p dhoby-ghaut --example foam_studio_web --release -- --render-mesh [out-dir]
//! cargo run  -p dhoby-ghaut --example foam_studio_web --release -- --headless-exchanger [cells] [re]
//! cargo run  -p dhoby-ghaut --example foam_studio_web --release -- --render-exchanger [out-dir]
//! cargo run  -p dhoby-ghaut --example foam_studio_web --release -- --render-coil [out-dir]
//! cargo run  -p dhoby-ghaut --example foam_studio_web --release -- --headless-coil [cells] [re] [turns]
//! cargo run  -p dhoby-ghaut --example foam_studio_web --release -- --coil-run <m> <re> [linear|upwind] [straight]
//! cargo run  -p dhoby-ghaut --example foam_studio_web --release -- --headless-unstructured [h_mm] [layers] [first_mm] [expansion] [re] [length_mm]
//! cargo run  -p dhoby-ghaut --example foam_studio_web --release -- --mesh-sweep
//! cargo run  -p dhoby-ghaut --example foam_studio_web --release -- --render-unstructured [out-dir]
//! cargo run  -p dhoby-ghaut --example foam_studio_web --release -- --render-default [out-dir]
//! cargo run  -p dhoby-ghaut --example foam_studio_web --release -- --render-layer-strategies [out-dir]
//! cargo test -p dhoby-ghaut --example foam_studio_web --release
//! ```
//!
//! - **Physics**: none written here. Meshes from
//!   `outram_foam_mesh::block_primitives` (the pipe O-grid, the box block),
//!   and, for the cylinder's **Unstructured** choice (gh:#1202), the cfMesh
//!   port through `outram_blender::foam_mesh` (tet → polyhedral dual →
//!   feature-edge capture, wall-only prism layers, `linearUpwind grad(U)`
//!   with `cellLimited Gauss linear 1` convection; ~~at the default u_c/2U
//!   0.89 and dp/dz 1.03 × Hagen–Poiseuille, an open gap, see the V&V
//!   record's gh:#1202 section~~ **since gh:#1298 the default mesher is the
//!   cell-centre dual with cfMesh's surface untangling and
//!   `optimiseFinalMesh` (upstream's sequence without the Laplace pass):
//!   u_c/2U 0.883 and dp/dz 1.065 × Hagen–Poiseuille at the default, an
//!   open gap, see the V&V record's gh:#1298 section; the old mesher, 0.8925
//!   / 1.0326, is the ablation `route=legacy`);
//!   the solve is `outram_foam_appbuilder_lib`'s `PimpleFoam` with
//!   `cases::laminar_pipe`'s boundary conditions. V&V record:
//!   `crates/outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155.md`.
//! - **Cells**: 20k by default (interactive), 50k at most as a batch run with
//!   a progress bar (maintainer decision, 2026-11-03).
//! - **Solve modes**: steady by pseudo-transient PISO at Co ≈ 4 (the
//!   default), or Co ≈ 1 to watch the profile develop.
//! - **No lag**: each worker message is a mesh or one chunk of time steps
//!   ([`engine`]); the page only draws.
//! - `--headless` runs the same engine natively and prints its progress
//!   (the engine CLAUDE.md headless rule; regression test in [`engine`]).
//! - `--render-mesh` writes the drawing rule's mesh images, from the
//!   `PolyMesh` the solver is built from (`--render-exchanger`,
//!   `--render-coil` for the other cases).
//! - The coil ([`coil`]): `outram_foam_mesh::block_primitives::HelicalPipe`,
//!   `cases::helical_coil`, Mishra & Gupta from `tampines`; `--coil-run`
//!   is the V&V command, record
//!   `crates/outram-foam-appbuilder-lib/verification_and_validation/helical_coil_dean_gh1157.md`.
//!
//! Research, education and V&V only (`RESPONSIBLE_USE.md`): not for reactor
//! operation, licensing or safety decisions.

#[cfg(not(target_os = "android"))]
mod app;
#[cfg(not(target_os = "android"))]
mod coil;
#[cfg(not(target_os = "android"))]
mod engine;
#[cfg(not(target_os = "android"))]
mod render;

/// Android stub: windowing GUIs are out of scope on Termux (the workspace
/// example rule: a blanked file gives "main function not found").
#[cfg(target_os = "android")]
fn main() {
    eprintln!("foam_studio_web is a windowing GUI and is not built for Android.");
}

/// The mesh images of the drawing rule: for each design, a cross-section and
/// an axial slice of the mesh the solver gets, drawn through
/// `outram_blender`'s mesh plotter. CLI glue (file output); the meshes come
/// from the tested `engine::mesh_design`.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn render_mesh(out: &std::path::Path) -> Result<(), String> {
    use outram_blender::csg::plot::slice::PlotBasis;
    use outram_blender::unstructured::convert::foam::from_poly_mesh;
    use outram_blender::unstructured::{render_mesh_slice_annotated, MeshColourBy, MeshSlice};
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let pipe = |cells: usize, length: f64| engine::Design {
        shape: engine::Shape::Cylinder { radius: 0.01, length, centre: [0.0, 0.0, 0.5 * length] },
        target_cells: cells,
        ..engine::Design::default()
    };
    let cases = [
        ("demo_20k", pipe(engine::DEFAULT_CELLS, 0.2), "DEMO PIPE"),
        ("vv_m3", pipe(17_280, 0.4), "VV PIPE L/D=20, M=3"),
        ("vv_m5", pipe(80_000, 0.4), "VV PIPE L/D=20, M=5"),
    ];
    for (tag, d, label) in cases {
        let (poly, _, _) = if tag.starts_with("vv_") {
            // The V&V grids are the similar family `PipeOGrid::level`, not the
            // demo's; m = 3 and 5.
            let lvl = if tag == "vv_m3" { 3 } else { 5 };
            let len = uom::si::f64::Length::new::<uom::si::length::meter>;
            let g = outram_foam_mesh::block_primitives::PipeOGrid::level(len(0.01), len(0.4), lvl);
            (g.mesh().map_err(|e| e.to_string())?, 2, 0.0)
        } else {
            engine::mesh_design(&d)?
        };
        let m = from_poly_mesh(&poly.to_foam_poly_mesh()).map_err(|e| format!("{e:?}"))?;
        let len_cm = match d.shape {
            engine::Shape::Cylinder { length, .. } | engine::Shape::DoublePipe { length, .. } => length * 100.0,
            engine::Shape::Box { size, .. } => size[2] * 100.0,
            // Not drawn here (render_mesh draws the pipes; `--render-coil` the coil).
            engine::Shape::Coil { .. } => 0.0,
        };
        let shots = [
            (PlotBasis::Xy, [0.0, 0.0, 0.5 * len_cm + 0.013], [2.2, 2.2], 900, format!("{label}, {} CELLS, X-Y AT Z = {:.2} CM", m.n_cells(), 0.5 * len_cm + 0.013), "xy"),
            (PlotBasis::Xz, [0.0, 0.013, 2.0], [2.2, 4.4], 500, format!("{label}, X-Z AT Y = 0.013 CM, INLET END"), "xz_inlet"),
        ];
        for (basis, origin, width, px, title, view) in shots {
            let py = ((px as f64) * width[1] / width[0]).round() as usize;
            let sl = MeshSlice { basis, origin, width, pixels: [px, py], colour_by: MeshColourBy::Cell };
            let img = render_mesh_slice_annotated(&m, &sl, &title);
            let name = format!("{tag}_{view}.png");
            img.write_png(&out.join(&name)).map_err(|e| format!("{e:?}"))?;
            println!("wrote {name}: {} cells, volume {:.6e} m3", m.n_cells(), m.total_volume());
        }
    }
    Ok(())
}

/// `--mesh-sweep`: mesh the demo pipe (R 10 mm, L 200 mm) unstructured over a
/// grid of cell sizes and layer counts and print cells, wall faces, quality
/// and the meshing time, to set the demo's defaults and limits (gh:#1202);
/// on the demo's default mesher (`TetDualSpec::BASE`, since gh:#1298 the
/// cell-centre dual with upstream's sequence; `FOAM_CFMESH_ROUTE=legacy`
/// with `FOAM_DUAL_KIND=corner FOAM_DUAL_FP=edges FOAM_LAYER_STRATEGY=backoff`
/// for the pre-gh:#1298 one).
/// CLI glue; the meshing is the tested `engine::mesh_unstructured`.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn mesh_sweep() -> Result<(), String> {
    use engine::{Tag, TetDualSpec};
    println!("h_mm layers cells wall_faces est_cells max_nonortho_deg max_skew mesh_s notes");
    for h_mm in [6.0, 5.0, 4.0, 3.3, 2.5, 2.0] {
        for layers in [0, 1, 2, 3] {
            let spec = TetDualSpec { cell_size: h_mm * 1e-3, n_layers: layers, first_layer: 3e-4, expansion: 1.2, ..engine::TetDualSpec::BASE };
            let t0 = std::time::Instant::now();
            let (poly, notes) = engine::mesh_unstructured(0.01, 0.2, [0.0, 0.0, 0.1], [Tag::Inlet, Tag::Outlet, Tag::Wall], &spec)?;
            let s = t0.elapsed().as_secs_f64();
            let q = outram_foam_mesh::mesh_quality::assess_quality(&poly.to_foam_poly_mesh());
            let wall = poly.patches.iter().find(|p| p.name == "walls").map_or(0, |p| p.size);
            println!(
                "{h_mm} {layers} {} {wall} {:.0} {:.1} {:.2} {s:.2} [{}]",
                poly.n_cells,
                spec.estimated_cells(0.01, 0.2),
                q.max_non_ortho_deg,
                q.max_skewness,
                notes.join("; ")
            );
        }
    }
    Ok(())
}

/// `--render-unstructured`: the drawing rule's images of the unstructured
/// pipe (gh:#1202), from the `PolyMesh` the solver gets: the demo default
/// with wall-only layers, and the same settings with layers on the whole
/// boundary (the old behaviour, through the cfMesh pipeline with
/// `layer_patches` empty), each as a cross-section at mid-length and an x-z
/// slice of the inlet end and of the inlet corner, coloured by cell; plus
/// the V&V meshes. CLI glue (file output).
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn render_unstructured(out: &std::path::Path) -> Result<(), String> {
    use engine::{Tag, TetDualSpec};
    use outram_blender::unstructured::convert::foam::from_poly_mesh;
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let tags = [Tag::Inlet, Tag::Outlet, Tag::Wall];
    // The before/after pair at h 5 mm, 1 layer (the default when drawn, 2026-11-04).
    // gh:#1298: the gh:#1202 record's mesher, the named ablation.
    let def = TetDualSpec { cell_size: 0.005, n_layers: 1, first_layer: 3e-4, expansion: 1.2, ..engine::TetDualSpec::PRE_GH1298 };
    // Whole-boundary layers (the pre-gh:#1202 behaviour), for the before picture.
    let whole = {
        use outram_blender::foam_mesh::{mesh_to_tet_dual_multipatch, TetDualOptions, Vec3};
        let s = def.clamped(0.01, 0.2);
        let cyl = outram_blender::primitives::cylinder(engine::CYL_SEGMENTS, 0.01, 0.2);
        let label = |t: [Vec3; 3]| -> String {
            let flat = (t[0].z - t[1].z).abs() < 1e-12 && (t[1].z - t[2].z).abs() < 1e-12;
            match (flat, t[0].z < 0.0) {
                (true, true) => "inlet",
                (true, false) => "outlet",
                _ => "walls",
            }
            .into()
        };
        let opts = TetDualOptions { cell_size: s.cell_size, n_layers: s.n_layers, first_layer_thickness: s.first_layer, expansion: s.expansion, ..TetDualOptions::default() };
        let (mut vm, _) = mesh_to_tet_dual_multipatch(&cyl, label, &opts)?;
        for p in &mut vm.points {
            p.z += 0.1;
        }
        let n = outram_blender::unstructured::convert::cfmesh::from_volume_mesh(&vm).map_err(|e| format!("{e:?}"))?;
        n
    };
    let wall_only = engine::mesh_unstructured(0.01, 0.2, [0.0, 0.0, 0.1], tags, &def)?.0;
    let mut meshes = vec![
        ("unstructured_h5_1layer_wall_only", from_poly_mesh(&wall_only.to_foam_poly_mesh()).map_err(|e| format!("{e:?}"))?, "UNSTRUCTURED H 5 MM, 1 LAYER, ON THE WALL ONLY (AFTER)".to_string()),
        ("unstructured_h5_1layer_whole_boundary", whole, "SAME SETTINGS, LAYERS ON THE WHOLE BOUNDARY (BEFORE)".to_string()),
    ];
    for (tag, spec) in vv_unstructured_specs() {
        let p = engine::mesh_unstructured(0.01, 0.2, [0.0, 0.0, 0.1], tags, &spec)?.0;
        meshes.push((tag, from_poly_mesh(&p.to_foam_poly_mesh()).map_err(|e| format!("{e:?}"))?, format!("V&V {tag}")));
    }
    write_unstructured_shots(out, &meshes)
}

/// `--render-default` (gh:#1298): the drawing rule's images of the demo's
/// unstructured mesher (`TetDualSpec::BASE`) bare at h 4 mm (~~the
/// default~~ the default until the maintainer's gh:#1298 decision) and with
/// 3 wall layers at h 5 mm (`TetDualSpec::default_for`, the default since
/// that decision, 2026-11-05), beside the pre-gh:#1298
/// default (`TetDualSpec::PRE_GH1298`), from the `PolyMesh` the solver
/// gets, with the shots of `--render-unstructured`. CLI glue (file output);
/// reached by no test (drawing only).
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn render_default(out: &std::path::Path) -> Result<(), String> {
    use engine::{Tag, TetDualSpec};
    use outram_blender::unstructured::convert::foam::from_poly_mesh;
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let tags = [Tag::Inlet, Tag::Outlet, Tag::Wall];
    let def = TetDualSpec::default_for(0.01);
    let cases = [
        ("default_gh1298_h4_bare", TetDualSpec { cell_size: 0.004, n_layers: 0, ..def }, "GH1298 MESHER, H 4 MM, NO LAYER (DEFAULT UNTIL THE GH1298 DECISION)"),
        ("default_gh1298_h5_3layers", def, "GH1298 DEFAULT: H 5 MM, 3 LAYERS (CFMESH OPTIMISE)"),
        ("pre_gh1298_h4_bare", TetDualSpec { cell_size: 0.004, n_layers: 0, first_layer: def.first_layer, expansion: def.expansion, ..TetDualSpec::PRE_GH1298 }, "PRE-GH1298 DEFAULT (ABLATION): PER-CORNER DUAL, H 4 MM, NO LAYER"),
    ];
    let mut meshes = Vec::new();
    for (tag, spec, label) in cases {
        let p = engine::mesh_unstructured(0.01, 0.2, [0.0, 0.0, 0.1], tags, &spec)?.0;
        meshes.push((tag, from_poly_mesh(&p.to_foam_poly_mesh()).map_err(|e| format!("{e:?}"))?, label.to_string()));
    }
    write_unstructured_shots(out, &meshes)
}

/// The four views of `--render-unstructured` (cross-section at mid-length,
/// x-z inlet end, x-z inlet corner, wall zoom), coloured by cell, for each
/// `(tag, mesh, label)`. CLI glue (file output).
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn write_unstructured_shots(out: &std::path::Path, meshes: &[(&str, outram_blender::unstructured::UnstructuredMesh, String)]) -> Result<(), String> {
    use outram_blender::csg::plot::slice::PlotBasis;
    use outram_blender::unstructured::{render_mesh_slice_annotated, MeshColourBy, MeshSlice};
    for (tag, m, label) in meshes {
        let shots = [
            (PlotBasis::Xy, [0.0, 0.0, 10.013], [2.2, 2.2], 900, format!("{label}, {} CELLS, X-Y AT Z = 10.01 CM", m.n_cells()), "xy"),
            (PlotBasis::Xz, [0.0, 0.013, 2.0], [2.2, 4.4], 500, format!("{label}, X-Z AT Y = 0.013 CM, INLET END"), "xz_inlet"),
            (PlotBasis::Xz, [0.75, 0.013, 0.25], [0.6, 0.6], 700, format!("{label}, X-Z, INLET CORNER (X 0.45-1.05, Z 0-0.55 CM)"), "xz_corner"),
            (PlotBasis::Xy, [0.85, 0.0, 10.013], [0.35, 0.35], 800, format!("{label}, X-Y WALL ZOOM (X 0.675-1.025 CM) AT Z = 10.01 CM"), "xy_wall"),
        ];
        for (basis, origin, width, px, title, view) in shots {
            let py = ((px as f64) * width[1] / width[0]).round() as usize;
            let sl = MeshSlice { basis, origin, width, pixels: [px, py], colour_by: MeshColourBy::Cell };
            let img = render_mesh_slice_annotated(m, &sl, &title);
            let name = format!("{tag}_{view}.png");
            img.write_png(&out.join(&name)).map_err(|e| format!("{e:?}"))?;
            println!("wrote {name}: {} cells, volume {:.6e} m3", m.n_cells(), m.total_volume());
        }
    }
    Ok(())
}

/// `--render-layer-strategies`: the gh:#1215 pictures. For each layer
/// strategy on the demo pipe at h 5 mm with 3 wall layers: the inlet rim
/// (x-z zoom), a wall zoom of the cross-section at mid-length, and an
/// unrolled map of the wall (angle against z) coloured by the layers each
/// wall point got and how thick its column is. CLI glue: the
/// meshes come from `engine::mesh_unstructured_with`, the slices from
/// `render_mesh_slice_annotated`; reached by no test (drawing only).
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn render_layer_strategies(out: &std::path::Path) -> Result<(), String> {
    use engine::{Tag, TetDualSpec};
    use outram_blender::csg::plot::slice::PlotBasis;
    use outram_blender::csg::plot::{annotate::draw_text, ImageData};
    use outram_blender::unstructured::convert::foam::from_poly_mesh;
    use outram_blender::unstructured::{render_mesh_slice_annotated, MeshColourBy, MeshSlice};
    use outram_blender::foam_mesh::LayerQualityStrategy as S;
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let tags = [Tag::Inlet, Tag::Outlet, Tag::Wall];
    let h_mm: f64 = std::env::var("FOAM_RENDER_H_MM").ok().and_then(|v| v.parse().ok()).unwrap_or(5.0);
    // FOAM_RENDER_N (gh:#1241): the layer count, 3 unless set.
    let n_layers: usize = std::env::var("FOAM_RENDER_N").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
    // gh:#1298: the records these images belong to were drawn on the
    // pre-gh:#1298 mesher; FOAM_CFMESH_ROUTE=upstream draws the new route.
    let spec = TetDualSpec { cell_size: h_mm * 1e-3, n_layers, first_layer: 3e-4, expansion: 1.2, ..engine::TetDualSpec::PRE_GH1298 };
    for (strategy, tag) in [(S::BackOffOnly, "backoff"), (S::CfMeshOptimise, "cfmesh"), (S::SnappyCollapse, "snappy")] {
        let (poly, _, outcome) = engine::mesh_unstructured_with(0.01, 0.2, [0.0, 0.0, 0.1], tags, &spec, strategy)?;
        let m = from_poly_mesh(&poly.to_foam_poly_mesh()).map_err(|e| format!("{e:?}"))?;
        let label = format!("{tag} h {h_mm} mm {n_layers} layers");
        let shots = [
            (PlotBasis::Xz, [0.75, 0.013, 0.25], [0.6, 0.6], 700, format!("{label}, {} cells, x-z inlet rim (x 0.45-1.05, z 0-0.55 cm)", m.n_cells()).to_uppercase(), "xz_rim"),
            (PlotBasis::Xy, [0.0, 0.0, 10.013], [2.2, 2.2], 900, format!("{label}, x-y at z = 10.01 cm").to_uppercase(), "xy"),
            (PlotBasis::Xy, [0.85, 0.0, 10.013], [0.35, 0.35], 800, format!("{label}, x-y wall zoom (x 0.675-1.025 cm)").to_uppercase(), "xy_wall"),
        ];
        for (basis, origin, width, px, title, view) in shots {
            let py = ((px as f64) * width[1] / width[0]).round() as usize;
            let sl = MeshSlice { basis, origin, width, pixels: [px, py], colour_by: MeshColourBy::Cell };
            let img = render_mesh_slice_annotated(&m, &sl, &title);
            let name = format!("layers_{tag}_h{h_mm}_{n_layers}_{view}.png");
            img.write_png(&out.join(&name)).map_err(|e| format!("{e:?}"))?;
            println!("wrote {name}");
        }
        // The wall map: angle (x) against z (y), one dot per wall point.
        let Some(o) = outcome else { continue };
        let (w, hgt, margin) = (960usize, 520usize, 40usize);
        let mut img = ImageData::filled(w, hgt + 2 * margin, outram_blender::csg::plot::Rgb { r: 255, g: 255, b: 255 });
        for (p, n, t, cap) in &o.points {
            let theta = p.y.atan2(p.x); // -pi..pi
            let z = p.z + 0.1; // the pipeline's frame is z in [-0.1, 0.1] m
            let x = ((theta + std::f64::consts::PI) / (2.0 * std::f64::consts::PI) * (w - 1) as f64) as i64;
            let y = margin as i64 + ((z / 0.2) * (hgt - 1) as f64) as i64;
            // Colour: no layer red; fewer than requested orange; the full
            // count blue, darker the thicker the column (0 to 1 mm).
            let c = if *n == 0 {
                outram_blender::csg::plot::Rgb { r: 220, g: 30, b: 30 }
            } else if *n < o.n_layers_requested {
                outram_blender::csg::plot::Rgb { r: 240, g: 150, b: 20 }
            } else {
                let _ = cap;
                let f = (t / 1e-3).clamp(0.0, 1.0);
                outram_blender::csg::plot::Rgb { r: (40.0 + 160.0 * (1.0 - f)) as u8, g: (90.0 + 120.0 * (1.0 - f)) as u8, b: 230 }
            };
            for dx in -2..=2 {
                for dy in -2..=2 {
                    let (xx, yy) = (x + dx, y + dy);
                    if xx >= 0 && yy >= 0 && (xx as usize) < w && (yy as usize) < hgt + 2 * margin {
                        img.set(xx as usize, yy as usize, c);
                    }
                }
            }
        }
        let black = outram_blender::csg::plot::Rgb { r: 0, g: 0, b: 0 };
        draw_text(&mut img, 6, 6, &format!("{label}: WALL MAP, ANGLE (X) AGAINST Z (Y, INLET AT TOP)").to_uppercase(), black);
        draw_text(
            &mut img,
            6,
            (hgt + margin + 12) as i64,
            &format!(
                "RED NONE, ORANGE FEWER, BLUE ALL (DARKER = THICKER, 0-1 MM). FULL {:.1}%, NONE {:.1}%, MEAN {:.3} MM",
                100.0 * o.full_coverage(),
                100.0 * o.collapsed_point_fraction(),
                1e3 * o.mean_total_thickness()
            ),
            black,
        );
        let name = format!("layers_{tag}_h{h_mm}_{n_layers}_wallmap.png");
        img.write_png(&out.join(&name)).map_err(|e| format!("{e:?}"))?;
        println!("wrote {name}: full {:.3}, none {:.3}, mean total {:.4} mm; {:?}", o.full_coverage(), o.collapsed_point_fraction(), 1e3 * o.mean_total_thickness(), o.notes);
    }
    Ok(())
}

/// `--render-dual-kinds`: the gh:#1240 pictures. For each dual kind on the
/// demo pipe at h 5 mm: the wall faces of one wall cell at mid-length,
/// unrolled (R θ against z, every face outlined in its own colour, with the
/// face-to-owner angle of each); the cross-section at mid-length and its
/// wall zoom, bare; and the x-z inlet rim with 3 wall layers (`BackOffOnly`).
/// CLI glue (drawing and file output); reached by no test.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn render_dual_kinds(out: &std::path::Path) -> Result<(), String> {
    use engine::{Tag, TetDualSpec};
    use outram_blender::csg::plot::slice::PlotBasis;
    use outram_blender::csg::plot::{annotate::draw_text, ImageData, Rgb};
    use outram_blender::foam_mesh::{mesh_to_tet_dual_multipatch, LayerQualityStrategy as S, TetDualOptions, Vec3};
    use outram_blender::unstructured::convert::foam::from_poly_mesh;
    use outram_blender::unstructured::{render_mesh_slice_annotated, MeshColourBy, MeshSlice};
    use outram_park_fork_cfmesh::dual::DualKind as D;
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let tags = [Tag::Inlet, Tag::Outlet, Tag::Wall];
    let h = 0.005;
    // FOAM_DUAL_FP=corners: the polyDualMesh class's feature points, file suffix `_corners`.
    let sfx = if std::env::var("FOAM_DUAL_FP").as_deref() == Ok("corners") { "_corners" } else { "" };
    // FOAM_CORRECT_EDGES=1 (gh:#1241): correctEdgesBetweenPatches, file suffix `_ce`.
    let ce = std::env::var("FOAM_CORRECT_EDGES").as_deref() == Ok("1");
    let sfx = if ce { format!("{sfx}_ce") } else { sfx.to_string() };
    for (kind, tag) in [(D::MedianPerCorner, "corner"), (D::MedianPerPoint, "point"), (D::CellCentre, "centre")] {
        // 1. One wall cell's wall faces, from the VolumeMesh itself.
        let cyl = outram_blender::primitives::cylinder(engine::CYL_SEGMENTS, 0.01, 0.2);
        let label = |t: [Vec3; 3]| -> String {
            let flat = (t[0].z - t[1].z).abs() < 1e-12 && (t[1].z - t[2].z).abs() < 1e-12;
            match (flat, t[0].z < 0.0) {
                (true, true) => "inlet",
                (true, false) => "outlet",
                _ => "walls",
            }
            .into()
        };
        let opts = TetDualOptions { cell_size: h, n_layers: 0, layer_patches: vec!["walls".into()], dual_kind: kind, dual_feature_points: if std::env::var("FOAM_DUAL_FP").as_deref() == Ok("corners") { outram_park_fork_cfmesh::dual::FeaturePointRule::Corners } else { outram_park_fork_cfmesh::dual::FeaturePointRule::OnFeatureEdge }, correct_edges: ce, ..TetDualOptions::default() };
        let (vm, _) = mesh_to_tet_dual_multipatch(&cyl, label, &opts)?;
        let walls = vm.patches.iter().find(|p| p.name == "walls").ok_or("no walls patch")?;
        let wf: Vec<usize> = (walls.start_face..walls.start_face + walls.n_faces).collect();
        // The wall cell whose first wall face is nearest (R, 0, 0) (mid-length).
        let target = Vec3::new(0.01, 0.0, 0.0);
        let f0 = *wf.iter().min_by(|&&a, &&b| vm.face_centroid(a).sub(target).length().partial_cmp(&vm.face_centroid(b).sub(target).length()).unwrap()).unwrap();
        let cell = vm.owner[f0];
        let mine: Vec<usize> = wf.iter().copied().filter(|&f| vm.owner[f] == cell).collect();
        // Owner centre: volume-weighted, from the cell's faces (fan about the face-centroid mean).
        let cf: Vec<(usize, bool)> = (0..vm.face_count()).filter_map(|f| if vm.owner[f] == cell { Some((f, true)) } else if vm.neighbour[f] == Some(cell) { Some((f, false)) } else { None }).collect();
        let apex = cf.iter().fold(Vec3::ZERO, |a, &(f, _)| a.add(vm.face_centroid(f))).scale(1.0 / cf.len() as f64);
        let (mut vol, mut cen) = (0.0, Vec3::ZERO);
        for &(f, own) in &cf {
            let (c, s) = (vm.face_centroid(f), vm.face_area_vector(f));
            let v = s.dot(c.sub(apex)) / 3.0 * if own { 1.0 } else { -1.0 };
            vol += v;
            cen = cen.add(apex.scale(0.25).add(c.scale(0.75)).scale(v));
        }
        let cc = cen.scale(1.0 / vol);
        let (w, hgt) = (800usize, 800usize);
        let mut img = ImageData::filled(w, hgt, Rgb { r: 255, g: 255, b: 255 });
        let span = 0.006; // 6 mm window, unrolled
        let (u0, z0) = (0.01 * vm.face_centroid(f0).y.atan2(vm.face_centroid(f0).x), vm.face_centroid(f0).z);
        let px = |q: Vec3| -> (f64, f64) {
            let u = 0.01 * q.y.atan2(q.x) - u0;
            let z = q.z - z0;
            ((u / span + 0.5) * w as f64, (0.5 - z / span) * hgt as f64)
        };
        let line = |img: &mut ImageData, a: (f64, f64), b: (f64, f64), c: Rgb| {
            let n = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) as usize).max(1);
            for i in 0..=n {
                let t = i as f64 / n as f64;
                let (x, y) = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
                for d in 0..2 {
                    let (xx, yy) = (x as i64 + d, y as i64);
                    if xx >= 0 && yy >= 0 && (xx as usize) < w && (yy as usize) < hgt {
                        img.set(xx as usize, yy as usize, c);
                    }
                }
            }
        };
        let palette = [Rgb { r: 200, g: 30, b: 30 }, Rgb { r: 30, g: 120, b: 200 }, Rgb { r: 20, g: 150, b: 60 }, Rgb { r: 200, g: 120, b: 0 }, Rgb { r: 130, g: 40, b: 170 }, Rgb { r: 0, g: 150, b: 150 }, Rgb { r: 120, g: 120, b: 0 }, Rgb { r: 90, g: 90, b: 90 }];
        let black = Rgb { r: 0, g: 0, b: 0 };
        // The other wall faces nearby, light grey, for context.
        for &f in &wf {
            if vm.face_centroid(f).sub(vm.face_centroid(f0)).length() < span && !mine.contains(&f) {
                let r = &vm.faces[f];
                for i in 0..r.len() {
                    line(&mut img, px(vm.points[r[i]]), px(vm.points[r[(i + 1) % r.len()]]), Rgb { r: 205, g: 205, b: 205 });
                }
            }
        }
        let mut angles = Vec::new();
        for (k, &f) in mine.iter().enumerate() {
            let r = &vm.faces[f];
            let c = palette[k % palette.len()];
            for i in 0..r.len() {
                line(&mut img, px(vm.points[r[i]]), px(vm.points[r[(i + 1) % r.len()]]), c);
            }
            let (d, sv) = (vm.face_centroid(f).sub(cc), vm.face_area_vector(f));
            let a = (d.dot(sv) / (d.length() * sv.length())).clamp(-1.0, 1.0).acos().to_degrees();
            angles.push(format!("{a:.0}"));
            let (x, y) = px(vm.face_centroid(f));
            draw_text(&mut img, x as i64 - 6, y as i64 - 4, &format!("{a:.0}"), c);
        }
        let (x, y) = px(cc);
        for d in -4i64..=4 {
            for (xx, yy) in [(x as i64 + d, y as i64), (x as i64, y as i64 + d)] {
                if xx >= 0 && yy >= 0 && (xx as usize) < w && (yy as usize) < hgt {
                    img.set(xx as usize, yy as usize, black);
                }
            }
        }
        draw_text(&mut img, 6, 6, &format!("{tag} dual, h 5 mm: wall faces of one wall cell, unrolled (R theta x z), 6 mm window").to_uppercase(), black);
        draw_text(&mut img, 6, 20, &format!("{} wall faces; face-to-owner angle (deg) at each centre: {}; + owner centre", mine.len(), angles.join(" ")).to_uppercase(), black);
        let name = format!("dual_{tag}{sfx}_h5_wall_cell.png");
        img.write_png(&out.join(&name)).map_err(|e| format!("{e:?}"))?;
        println!("wrote {name}: cell {cell}, {} wall faces, angles {:?}", mine.len(), angles);

        // 2. Slices of the solver's PolyMesh: bare cross-section, 3 layers at the rim.
        for (n, view) in [(0usize, "bare"), (3, "3layers")] {
            let spec = TetDualSpec { cell_size: h, n_layers: n, first_layer: 3e-4, expansion: 1.2, ..engine::TetDualSpec::PRE_GH1298 };
            let (poly, notes, _) = engine::mesh_unstructured_dual(0.01, 0.2, [0.0, 0.0, 0.1], tags, &spec, S::BackOffOnly, kind)?;
            let m = from_poly_mesh(&poly.to_foam_poly_mesh()).map_err(|e| format!("{e:?}"))?;
            let label = format!("{tag} dual h 5 mm {view}");
            let shots: Vec<(PlotBasis, [f64; 3], [f64; 2], usize, String, &str)> = if n == 0 {
                vec![
                    (PlotBasis::Xy, [0.0, 0.0, 10.013], [2.2, 2.2], 900, format!("{label}, {} cells, x-y at z = 10.01 cm", m.n_cells()).to_uppercase(), "xy"),
                    (PlotBasis::Xy, [0.85, 0.0, 10.013], [0.35, 0.35], 800, format!("{label}, x-y wall zoom (x 0.675-1.025 cm)").to_uppercase(), "xy_wall"),
                ]
            } else {
                vec![(PlotBasis::Xz, [0.75, 0.013, 0.25], [0.6, 0.6], 700, format!("{label} backoff, {} cells, x-z inlet rim (x 0.45-1.05, z 0-0.55 cm)", m.n_cells()).to_uppercase(), "xz_rim")]
            };
            for (basis, origin, width, pxl, title, v) in shots {
                let py = ((pxl as f64) * width[1] / width[0]).round() as usize;
                let sl = MeshSlice { basis, origin, width, pixels: [pxl, py], colour_by: MeshColourBy::Cell };
                let img = render_mesh_slice_annotated(&m, &sl, &title);
                let name = format!("dual_{tag}{sfx}_h5_{view}_{v}.png");
                img.write_png(&out.join(&name)).map_err(|e| format!("{e:?}"))?;
                println!("wrote {name}; notes {notes:?}");
            }
        }
    }
    Ok(())
}

/// The unstructured meshes of the V&V record (gh:#1202): tags and specs.
/// CLI glue shared by `--render-unstructured`.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn vv_unstructured_specs() -> Vec<(&'static str, engine::TetDualSpec)> {
    // The ~7k member (the ~20k member is the demo default, drawn above).
    // 2026-11-04 (gh:#1202): the mapped (feature-edge) meshes, the demo
    // default bare and with 2 and 3 wall layers.
    // gh:#1298: these are the gh:#1202 record's meshes, on its mesher.
    let s = |h: f64, n: usize| engine::TetDualSpec { cell_size: h, n_layers: n, first_layer: 3e-4, expansion: 1.2, ..engine::TetDualSpec::PRE_GH1298 };
    vec![
        ("unstructured_default_7k", s(0.006, 1)),
        ("mapped_h4_bare", s(0.004, 0)),
        ("mapped_h4_2layers", s(0.004, 2)),
        ("mapped_h4_3layers", s(0.004, 3)),
    ]
}

/// The drawing rule's images of the double-pipe exchanger (gh:#1156): the
/// three region meshes the solver gets (`engine::exchanger_grid`, the
/// demo's 20k grid and the V&V record's 8k test grid), concatenated and
/// coloured by region (one zone per region), cross-sections at mid-length,
/// zoomed onto the tube wall, an x-z slice at the inlet end, and one
/// cell-coloured cross-section. CLI glue (file output); the meshes come from
/// the tested `engine::exchanger_grid` and `DoublePipe::meshes`.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn render_exchanger(out: &std::path::Path) -> Result<(), String> {
    use outram_blender::csg::plot::slice::PlotBasis;
    use outram_blender::unstructured::convert::foam::from_poly_mesh;
    use outram_blender::unstructured::{render_mesh_slice_annotated, MeshColourBy, MeshSlice, UnstructuredMesh, Zone};
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let d = engine::Design::exchanger();
    let engine::Shape::DoublePipe { r_tube, wall, r_shell, length, centre } = d.shape else { unreachable!() };
    let len = uom::si::f64::Length::new::<uom::si::length::meter>;
    let vv = outram_foam_mesh::block_primitives::DoublePipe {
        r_tube_inner: len(0.005),
        r_tube_outer: len(0.006),
        r_shell: len(0.01),
        length: len(0.2),
        origin: [len(0.0); 3],
        n: 4,
        nr: [3, 2, 4],
        nz: 50,
    };
    let grids = [("exch_demo_20k", engine::exchanger_grid(r_tube, wall, r_shell, length, centre, engine::DEFAULT_CELLS), "DEMO EXCHANGER"), ("exch_vv_8k", vv, "VV EXCHANGER")];
    for (tag, g, label) in grids {
        let [a, b, c] = g.meshes().map_err(|e| e.to_string())?;
        let all = outram_foam_mesh::block_mesh::PolyMesh::concatenate(&[("tube_fluid", &a), ("tube_wall", &b), ("annulus_fluid", &c)]);
        let plain = from_poly_mesh(&all.to_foam_poly_mesh()).map_err(|e| format!("{e:?}"))?;
        // The same mesh with one zone per region, for the region colours.
        let (n0, n1) = (a.n_cells, a.n_cells + b.n_cells);
        let zones = vec![
            Zone { name: "TUBE FLUID (HOT)".into(), cells: (0..n0).collect() },
            Zone { name: "TUBE WALL (SOLID)".into(), cells: (n0..n1).collect() },
            Zone { name: "ANNULUS FLUID (COLD)".into(), cells: (n1..all.n_cells).collect() },
        ];
        let zoned = rebuild_with_zones(&all, zones)?;
        let z0 = g.origin[2].get::<uom::si::length::meter>() * 100.0;
        let lcm = g.length.get::<uom::si::length::meter>() * 100.0;
        let zmid = z0 + 0.5 * lcm + 0.013;
        let cells = g.n_cells();
        let shots: [(&UnstructuredMesh, MeshColourBy, PlotBasis, [f64; 3], [f64; 2], usize, String, &str); 4] = [
            (&zoned, MeshColourBy::Zone, PlotBasis::Xy, [0.0, 0.0, zmid], [2.2, 2.2], 900, format!("{label} {:?} CELLS, REGIONS, X-Y AT Z = {zmid:.2} CM", cells), "xy_regions"),
            (&zoned, MeshColourBy::Zone, PlotBasis::Xy, [0.55, 0.0, zmid], [0.5, 0.5], 800, format!("{label}, REGIONS AT THE TUBE WALL (R 0.5-0.6 CM), X-Y ZOOM"), "xy_wall_zoom"),
            (&zoned, MeshColourBy::Zone, PlotBasis::Xz, [0.0, 0.013, z0 + 2.0], [2.2, 4.4], 500, format!("{label}, REGIONS, X-Z AT Y = 0.013 CM, INLET END"), "xz_regions"),
            (&plain, MeshColourBy::Cell, PlotBasis::Xy, [0.0, 0.0, zmid], [2.2, 2.2], 900, format!("{label}, CELLS, X-Y AT Z = {zmid:.2} CM"), "xy_cells"),
        ];
        for (mesh, colour_by, basis, origin, width, px, title, view) in shots {
            let py = ((px as f64) * width[1] / width[0]).round() as usize;
            let sl = MeshSlice { basis, origin, width, pixels: [px, py], colour_by };
            let img = render_mesh_slice_annotated(mesh, &sl, &title);
            let name = format!("{tag}_{view}.png");
            img.write_png(&out.join(&name)).map_err(|e| format!("{e:?}"))?;
            println!("wrote {name}: {} cells, volume {:.6e} m3", mesh.n_cells(), mesh.total_volume());
        }
    }
    Ok(())
}

/// `from_poly_mesh` with cell zones (it builds none). CLI glue for
/// [`render_exchanger`].
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn rebuild_with_zones(all: &outram_foam_mesh::block_mesh::PolyMesh, zones: Vec<outram_blender::unstructured::Zone>) -> Result<outram_blender::unstructured::UnstructuredMesh, String> {
    use outram_blender::unstructured::{LengthUnit, Patch, PatchKind, PolyFace, UnstructuredMesh};
    let points = all.points.iter().map(|p| [p.x, p.y, p.z]).collect();
    let faces = all.faces.iter().map(|f| PolyFace { verts: f.verts.clone(), owner: f.owner, neighbour: f.neighbour }).collect();
    let patches = all.patches.iter().map(|p| Patch { name: p.name.clone(), kind: PatchKind::Wall, faces: (p.start..p.start + p.size).collect() }).collect();
    UnstructuredMesh::from_polyhedral(LengthUnit::Metre, points, faces, all.n_cells, patches, zones).map_err(|e| format!("{e:?}"))
}

/// `--coil-run <m> <re> [linear|upwind] [straight] [max_steps] [co=<Courant, 4>] [tol=<steady tolerance>] [svg=<path>]`: one run of the
/// coil V&V (`helical_coil_dean_gh1157.md`) on the level-`m` grid, printing
/// progress and one result row. CLI glue; the run is the tested
/// `coil::run_to_steady`.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn coil_run_cli(args: &[String]) -> Result<(), String> {
    use outram_foam_appbuilder_lib::io::fv_schemes::DivScheme;
    let lvl: usize = args.first().and_then(|v| v.parse().ok()).unwrap_or(3);
    let re: f64 = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(200.0);
    let has = |w: &str| args.iter().any(|a| a == w);
    let scheme = if has("upwind") { DivScheme::GaussUpwind } else { DivScheme::GaussLinearDeferred };
    let spec = if has("straight") { coil::CoilSpec::straight_limit() } else { coil::CoilSpec::vv() };
    let max_steps = args.iter().filter_map(|a| a.parse::<usize>().ok()).nth(2).unwrap_or(20_000);
    let co: f64 = args.iter().find_map(|a| a.strip_prefix("co=")).and_then(|v| v.parse().ok()).unwrap_or(4.0);
    let tol = args.iter().find_map(|a| a.strip_prefix("tol=")).and_then(|v| v.parse().ok()).unwrap_or(engine::STEADY);
    let g = spec.level(lvl);
    let cells = g.n_cells();
    let t0 = bugis::web_demo::platform::now_s();
    // Every 50 steps: f on the two windows, and where the largest one-step
    // change of U is (arc position s, and r/R), from a snapshot one step
    // earlier.
    let mut snap: Option<Vec<outram_foam_basic_lib::primitives::Vector3>> = None;
    let (r, run, solve_s) = coil::run_to_steady(g, re, scheme.clone(), co, tol, max_steps, |run, c| {
        if let Some(prev) = snap.take() {
            let (mut worst, mut at) = (0.0, 0);
            for (i, (a, b)) in prev.iter().zip(run.solver.u.internal.as_slice()).enumerate() {
                let d = (*b - *a).mag();
                if d > worst {
                    worst = d;
                    at = i;
                }
            }
            let rr = (run.cells.n[at].powi(2) + run.cells.b[at].powi(2)).sqrt() / run.grid.radius.get::<uom::si::length::meter>();
            eprintln!("  largest change at s = {:.4} m of {:.4} (r/R {:.2}), |dU| {:.3e}", run.cells.s[at], run.grid.arc_length().get::<uom::si::length::meter>(), rr, worst);
        }
        if run.steps % 50 == 0 {
            let r = run.report();
            eprintln!("step {}: change {c:.3e} t {:.0} s fRe64 {:.5} before {:.5}", run.steps, bugis::web_demo::platform::now_s() - t0, r.f_re_64, r.f_re_64_before);
        }
        if run.steps % 50 == 49 {
            snap = Some(run.solver.u.internal.as_slice().to_vec());
        }
    })?;
    let steps = run.steps;
    if let Some(path) = args.iter().find_map(|a| a.strip_prefix("svg=")) {
        let l = run.grid.arc_length().get::<uom::si::length::meter>();
        write_dean_svg(&run, r.section_s / l, &format!("De = {:.1} (Re {:.0}, d/D_c {:.4}), s = {:.3} m of {:.3} m, {cells} cells, {scheme:?}", r.dean, r.reynolds, run.flow.curvature_ratio(), r.section_s, l), std::path::Path::new(path))?;
        eprintln!("wrote {path}");
    }
    let s = r.section;
    println!(
        "ROW m={lvl} cells={cells} scheme={scheme:?} co={co} tol={tol:e} straight={} Re={:.0} De={:.2} steps={steps} wall_s={:.1} (solve {:.1}) fRe64={:.5} fRe64_before={:.5} dev_ratio={:.5} MG={:.5} cfd/MG={:.5} peak_n={:.3} peak={:.4} max_sec={:.4} core_un={:.4} pair={:.4} sec_cells={} sec_s={:.4}",
        has("straight"),
        r.reynolds,
        r.dean,
        bugis::web_demo::platform::now_s() - t0,
        solve_s,
        r.f_re_64,
        r.f_re_64_before,
        r.f_re_64_before / r.f_re_64,
        r.mishra_gupta,
        r.f_re_64 / r.mishra_gupta,
        s.axial_peak_n,
        s.axial_peak,
        s.max_secondary,
        s.core_u_n,
        s.vortex_pair,
        s.cells,
        r.section_s
    );
    Ok(())
}

/// The Dean cross-section of a solved coil as an SVG (for the V&V record):
/// the layer at arc fraction `f` from the assembled solver mesh's cells
/// (`engine::Engine::coil_cross`), coil axis to the left, outer wall to the
/// right, B up; fill u_t/U (grey ramp, labelled), arrows the in-plane
/// velocity (u_n, u_b), scaled to the largest. CLI glue (file output).
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn write_dean_svg(run: &coil::CoilRun, f: f64, title: &str, path: &std::path::Path) -> Result<(), String> {
    use outram_foam_appbuilder_lib::cases::helical_coil::frenet_components;
    let straight = run.grid.straight_grid().mesh().map_err(|e| e.to_string())?;
    let (mut cross, mut probe) = (Vec::new(), 0);
    let _ = engine::Engine::coil_cross(&run.grid, &straight, &run.solver.mesh, f, &mut cross, &mut probe);
    let r = run.grid.radius.get::<uom::si::length::meter>();
    let u = coil::U_COIL;
    let (w, h, k) = (640.0, 700.0, 280.0 / r);
    let (cx, cy) = (320.0, 330.0);
    let comps: Vec<[f64; 3]> = cross.iter().map(|c| frenet_components(run.solver.u.internal[c.cell], &run.cells.frenet[c.cell])).collect();
    let umax_t = comps.iter().map(|v| v[0]).fold(0.0_f64, f64::max).max(1e-300);
    let umax_s = comps.iter().map(|v| (v[1] * v[1] + v[2] * v[2]).sqrt()).fold(0.0_f64, f64::max).max(1e-300);
    let mut s = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" font-family=\"sans-serif\">\n<rect width=\"{w}\" height=\"{h}\" fill=\"white\"/>\n");
    s.push_str(&format!("<text x=\"10\" y=\"20\" font-size=\"13\">{title}</text>\n"));
    s.push_str("<text x=\"10\" y=\"40\" font-size=\"12\">left: coil axis side (inner, +N) · right: outer wall · up: +B · fill: u_axial/U · arrows: secondary (u_n, u_b)</text>\n");
    let to = |n: f64, b: f64| (cx - n * k, cy - b * k);
    let cell_px = 2.0 * 280.0 / (cross.len() as f64).sqrt();
    for (c, v) in cross.iter().zip(&comps) {
        let g = (255.0 * (1.0 - 0.85 * v[0].max(0.0) / umax_t)).round() as u8;
        let pts: Vec<String> = c.polygon.iter().map(|p| { let (x, y) = to(p[0], p[1]); format!("{x:.1},{y:.1}") }).collect();
        s.push_str(&format!("<polygon points=\"{}\" fill=\"rgb({g},{g},{})\" stroke=\"#888\" stroke-width=\"0.4\"/>\n", pts.join(" "), g.saturating_add(20)));
    }
    for (c, v) in cross.iter().zip(&comps) {
        let n = c.polygon.len() as f64;
        let (pn, pb) = c.polygon.iter().fold((0.0, 0.0), |a, p| (a.0 + p[0] / n, a.1 + p[1] / n));
        let (x0, y0) = to(pn, pb);
        let sc = 0.9 * cell_px / umax_s;
        let (dx, dy) = (-v[1] * sc, -v[2] * sc);
        if (dx * dx + dy * dy).sqrt() < 1.0 {
            continue;
        }
        s.push_str(&format!("<line x1=\"{x0:.1}\" y1=\"{y0:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" stroke=\"#c00\" stroke-width=\"1.3\"/>\n", x0 + dx, y0 + dy));
        let (l, ux, uy) = ((dx * dx + dy * dy).sqrt(), dx, dy);
        let (hx, hy) = (ux / l * 5.0, uy / l * 5.0);
        s.push_str(&format!("<polygon points=\"{:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" fill=\"#c00\"/>\n", x0 + dx, y0 + dy, x0 + dx - hx - 0.5 * hy, y0 + dy - hy + 0.5 * hx, x0 + dx - hx + 0.5 * hy, y0 + dy - hy - 0.5 * hx));
    }
    s.push_str(&format!("<text x=\"10\" y=\"{}\" font-size=\"12\">max u_axial = {:.3} U (white 0, dark = max) · longest arrow = {:.3} U</text>\n", h - 40.0, umax_t / u, umax_s / u));
    s.push_str(&format!("<text x=\"10\" y=\"{}\" font-size=\"12\">solver mesh cells of one layer, drawn in the Frenet (N, B) plane at s; tube radius {:.1} mm</text>\n</svg>\n", h - 20.0, r * 1e3));
    std::fs::write(path, s).map_err(|e| e.to_string())
}

/// The drawing rule's images of the coil meshes (gh:#1157): the V&V grid
/// (m = 3, 3 turns) and the demo's default (20k, 2 turns), from the
/// assembled `PolyMesh` the solver gets, through `outram_blender`'s mesh
/// plotter: a vertical slice through the coil axis (every half-turn cut
/// across), a horizontal plan slice, and zooms onto single tube sections at
/// several stations. CLI glue (file output); the meshes come from the tested
/// `coil::CoilSpec::level` and `engine::coil_grid`.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn render_coil(out: &std::path::Path) -> Result<(), String> {
    use outram_blender::csg::plot::slice::PlotBasis;
    use outram_blender::unstructured::convert::foam::from_poly_mesh;
    use outram_blender::unstructured::{render_mesh_slice_annotated, MeshColourBy, MeshSlice};
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let mut demo = engine::coil_grid(&engine::Design::coil()).ok_or("no coil")?;
    demo.axis_origin = [uom::si::f64::Length::new::<uom::si::length::meter>(0.0); 3];
    for (tag, g) in [("vv_m3", coil::CoilSpec::vv().level(3)), ("demo_20k", demo)] {
        let poly = g.mesh().map_err(|e| e.to_string())?;
        let mesh = from_poly_mesh(&poly.to_foam_poly_mesh()).map_err(|e| format!("{e:?}"))?;
        let cm = |v: uom::si::f64::Length| v.get::<uom::si::length::meter>() * 100.0;
        let (rw, rt, h) = (0.5 * cm(g.coil_diameter), cm(g.radius), cm(g.pitch));
        let height = h * g.turns;
        let label = format!("{} CELLS ({}x{}x{} LAYERS), D_W {:.0} MM, PITCH {:.0} MM, {} TURNS", g.n_cells(), g.n, g.nr, g.n_layers, 10.0 * 2.0 * rw, 10.0 * h, g.turns);
        let wide = 2.0 * (rw + rt) + 1.0;
        let mut shots: Vec<(PlotBasis, [f64; 3], [f64; 2], usize, String, String)> = vec![
            (PlotBasis::Xz, [0.0, 0.0, 0.5 * height], [wide, height + 2.0 * rt + 1.0], 1000, format!("COIL {label}, X-Z AT Y = 0 (THROUGH THE AXIS)"), "xz_axis".into()),
            (PlotBasis::Xy, [0.0, 0.0, 0.5 * h + 0.013], [wide, wide], 900, format!("COIL, X-Y PLAN AT Z = {:.2} CM", 0.5 * h + 0.013), "xy_plan".into()),
        ];
        // Single tube sections on the plane y = 0 at phi = pi, 2pi, ... (x = -R, +R alternately).
        let half_turns = (2.0 * g.turns).floor() as usize;
        for k in 1..=half_turns {
            let phi = std::f64::consts::PI * k as f64;
            let x = if k % 2 == 1 { -rw } else { rw };
            let z = h * phi / (2.0 * std::f64::consts::PI);
            let s_m = phi * (g.arc_length().get::<uom::si::length::meter>() / (2.0 * std::f64::consts::PI * g.turns));
            if k == 1 || k == half_turns || k == half_turns / 2 + 1 {
                shots.push((PlotBasis::Xz, [x, 0.0, z], [2.6 * rt, 2.6 * rt], 700, format!("TUBE SECTION AT PHI = {k} PI (S = {:.3} M), X-Z AT Y = 0 (TILTED BY THE PITCH ANGLE)", s_m), format!("xz_section_{k}pi")));
            }
        }
        for (basis, origin, width, px, title, view) in shots {
            let py = ((px as f64) * width[1] / width[0]).round() as usize;
            let sl = MeshSlice { basis, origin, width, pixels: [px, py], colour_by: MeshColourBy::Cell };
            let img = render_mesh_slice_annotated(&mesh, &sl, &title);
            let name = format!("{tag}_{view}.png");
            img.write_png(&out.join(&name)).map_err(|e| format!("{e:?}"))?;
            println!("wrote {name}: {} cells, volume {:.6e} m3 (A L = {:.6e})", mesh.n_cells(), mesh.total_volume(), g.straight_grid().polygon_area() * g.arc_length().get::<uom::si::length::meter>());
        }
    }
    Ok(())
}

/// Natively: the GUI, or a command-line mode. `main` and CLI glue: exempt
/// from the reaching-test rule; each mode calls tested code
/// ([`engine::headless`], [`engine::mesh_design`]).
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--headless") => {
            let cells = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(engine::DEFAULT_CELLS);
            let re = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(100.0);
            let mode = if args.get(3).map(String::as_str) == Some("transient") { engine::Mode::Transient } else { engine::Mode::Steady };
            let d = engine::Design { target_cells: cells, ..engine::Design::default() };
            for l in engine::headless(d, re, mode, 5000)? {
                println!("{l}");
            }
            Ok(())
        }
        Some("--headless-unstructured") => {
            // [cell_mm] [layers] [first_mm] [expansion] [re] [length_mm]
            let f = |i: usize, d: f64| args.get(i).and_then(|v| v.parse::<f64>().ok()).unwrap_or(d);
            let def = engine::TetDualSpec::default_for(0.01);
            let spec = engine::TetDualSpec { cell_size: f(1, def.cell_size * 1e3) * 1e-3, n_layers: f(2, def.n_layers as f64) as usize, first_layer: f(3, def.first_layer * 1e3) * 1e-3, expansion: f(4, def.expansion), ..def };
            let length = f(6, 200.0) * 1e-3;
            let d = engine::Design { shape: engine::Shape::Cylinder { radius: 0.01, length, centre: [0.0, 0.0, 0.5 * length] }, mesh: engine::PipeMesh::Unstructured(spec), ..engine::Design::default() };
            for l in engine::headless(d, f(5, 100.0), engine::Mode::Steady, 5000)? {
                println!("{l}");
            }
            Ok(())
        }
        Some("--mesh-sweep") => mesh_sweep(),
        Some("--render-layer-strategies") => {
            let out = args.get(1).cloned().unwrap_or_else(|| {
                concat!(env!("CARGO_MANIFEST_DIR"), "/../outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155/layer_strategies_gh1215").to_string()
            });
            render_layer_strategies(std::path::Path::new(&out))
        }
        Some("--render-dual-kinds") => {
            let out = args.get(1).cloned().unwrap_or_else(|| {
                concat!(env!("CARGO_MANIFEST_DIR"), "/../outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155/dual_kinds_gh1240").to_string()
            });
            render_dual_kinds(std::path::Path::new(&out))
        }
        Some("--render-default") => {
            let out = args.get(1).cloned().unwrap_or_else(|| {
                concat!(env!("CARGO_MANIFEST_DIR"), "/../outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155/default_gh1298").to_string()
            });
            render_default(std::path::Path::new(&out))
        }
        Some("--render-unstructured") => {
            let out = args.get(1).cloned().unwrap_or_else(|| {
                concat!(env!("CARGO_MANIFEST_DIR"), "/../outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155/unstructured_gh1202").to_string()
            });
            render_unstructured(std::path::Path::new(&out))
        }
        Some("--coil-run") => coil_run_cli(&args[1..]),
        Some("--render-coil") => {
            let out = args.get(1).cloned().unwrap_or_else(|| {
                concat!(env!("CARGO_MANIFEST_DIR"), "/../outram-foam-appbuilder-lib/verification_and_validation/helical_coil_dean_gh1157").to_string()
            });
            render_coil(std::path::Path::new(&out))
        }
        Some("--headless-coil") => {
            let cells = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(engine::DEFAULT_CELLS);
            let re = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(200.0);
            let mut d = engine::Design { target_cells: cells, ..engine::Design::coil() };
            if let (Some(t), engine::Shape::Coil { turns, .. }) = (args.get(3).and_then(|v| v.parse().ok()), &mut d.shape) {
                *turns = t;
            }
            for l in engine::headless(d, re, engine::Mode::Steady, 20_000)? {
                println!("{l}");
            }
            Ok(())
        }
        Some("--render-exchanger") => {
            let out = args.get(1).cloned().unwrap_or_else(|| {
                concat!(env!("CARGO_MANIFEST_DIR"), "/../outram-foam-appbuilder-lib/verification_and_validation/cht_double_pipe_gh1156").to_string()
            });
            render_exchanger(std::path::Path::new(&out))
        }
        Some("--headless-exchanger") => {
            let cells = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(engine::DEFAULT_CELLS);
            let re = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(30.0);
            let d = engine::Design { target_cells: cells, ..engine::Design::exchanger() };
            for l in engine::headless(d, re, engine::Mode::Steady, 20_000)? {
                println!("{l}");
            }
            Ok(())
        }
        Some("--render-mesh") => {
            let out = args.get(1).cloned().unwrap_or_else(|| {
                concat!(env!("CARGO_MANIFEST_DIR"), "/../outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155").to_string()
            });
            render_mesh(std::path::Path::new(&out))
        }
        _ => eframe::run_native(
            "Foam Studio",
            eframe::NativeOptions::default(),
            Box::new(move |cc| Ok(Box::new(app::FoamApp::new(cc)))),
        )
        .map_err(|e| e.to_string()),
    }
}

/// In the browser this module runs twice: on the page, where it starts the
/// egui app, and in the Web Worker (`web/foam_studio/worker.js`), where there
/// is no `window` and it serves the engine instead. Start-up glue (exempt).
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    if js_sys::global().dyn_ref::<web_sys::DedicatedWorkerGlobalScope>().is_some() {
        bugis::web_demo::link::worker_main::<engine::Engine>();
        return;
    }
    let web_options = eframe::WebOptions { renderer: eframe::Renderer::Wgpu, ..Default::default() };
    wasm_bindgen_futures::spawn_local(async move {
        let document = web_sys::window().expect("no window").document().expect("no document");
        let canvas = document
            .get_element_by_id("foam_canvas")
            .expect("no #foam_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#foam_canvas is not a canvas");
        let started = eframe::WebRunner::new().start(canvas, web_options, Box::new(move |cc| Ok(Box::new(app::FoamApp::new(cc))))).await;
        if let Some(el) = document.get_element_by_id("loading") {
            match started {
                Ok(()) => el.remove(),
                Err(e) => el.set_inner_html(&format!("<p>Failed to start: {e:?}</p>")),
            }
        }
    });
}
