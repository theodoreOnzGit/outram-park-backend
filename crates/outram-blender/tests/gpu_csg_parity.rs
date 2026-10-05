//! **GPU CSG ray tracer against the CPU plotter, pixel by pixel** (gh:#587).
//!
//! # Methodology
//!
//! A small nested geometry with every structure the HTR-10 bed uses — a root
//! universe with a complemented region (the reflector), a cell filled with a
//! 3-D hexagonal lattice of pebble tiles, a pebble's fuel zone filled with a
//! translated-in-place 3-D rectangular TRISO lattice whose `outer` is matrix
//! graphite, and a two-shell particle — is drawn by the trusted `f64` CPU
//! plotter (`csg::plot`, an OpenMC port verified against `openmc --plot`)
//! and by the `f32` GPU tracer (`csg::gpu::render`), from the same plot
//! descriptions. The quantity compared is **the colour index each pixel
//! shows** (`SolidRayTracePlot::create_image_with_ids`, `SlicePlot::id_map`),
//! which lighting cannot change; colour is also compared for the solid and
//! x-ray pictures, as the fraction of pixels whose channels differ by more
//! than 2 (truncation of an `f32` versus an `f64` shade).
//!
//! Views: a perspective and an orthographic solid view (helium hidden, as
//! the workbench draws by default), the same perspective view with an x
//! section plane through the bed, an x-z slice through the whole model and a
//! slice zoomed into one pebble's TRISO lattice, and an x-ray view.
//!
//! **Pass criterion:** at most 1 % of pixels may show a different index
//! (boundary pixels, where `f32` and `f64` round a grazing ray to different
//! sides). Skips, with a printed message, when no GPU adapter is found.
//!
//! # Results (2026-10-05, NVIDIA RTX A5000, Vulkan)
//!
//! | view | pixels | index mismatch | colour mismatch (> 2 levels) |
//! |---|---|---|---|
//! | solid perspective | 43 200 | 0 % | 0 % |
//! | solid orthographic | 43 200 | 0 % | 0 % |
//! | solid, x section (camera side cut away) | 43 200 | 0 % | 0 % |
//! | x-z slice | 105 600 | 0 % | — |
//! | x-y slice in one pebble (TRISO) | 90 000 | 0 % | — |
//! | x-ray | 43 200 | — | 0 % |
//!
//! Interpretation: on this geometry the `f32` tracer reproduces the `f64`
//! plotter exactly. That needed the CPU's "a crossing closer than
//! 10 x TINY_BIT is not reported" rule scaled to the GPU's larger nudge
//! (first run: 36-55 % of solid pixels wrongly in shadow, ids still 0 %).
//! It is a small, axis-aligned model; the HTR-10 numbers, where boundary
//! pixels do differ, are in `crates/dhoby-ghaut/tests/gpu_csg_parity.rs`.
//! Re-run: `cargo test --release -p outram-blender --test gpu_csg_parity --
//! --nocapture` (`GPU_PARITY_DUMP=<dir>` writes both pictures as PNGs).

#![cfg(all(
    feature = "gpu",
    not(target_os = "android"),
    not(target_arch = "wasm32")
))]

use outram_blender::csg::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_blender::csg::geometry::Geometry;
use outram_blender::csg::gpu::flat::flatten;
use outram_blender::csg::gpu::render::{probe_renderer, GpuPlot};
use outram_blender::csg::lattice::{HexLattice, HexOrientation, Lattice, RectLattice};
use outram_blender::csg::plot::{
    Camera, ClipPlane, ColourScheme, ImageData, PlotBasis, PlotColourBy, Projection, Rgb, SliceHit,
    SlicePlot, SolidRayTracePlot, WireframeRayTracePlot,
};
use outram_blender::csg::position::{Direction, Position};
use outram_blender::csg::surface::{
    BoundaryType, Sphere, SurfaceKind, XPlane, YPlane, ZCylinder, ZPlane,
};
use outram_blender::csg::universe::Universe;

const T: BoundaryType = BoundaryType::Transmissive;

fn hs(i: usize, inside: bool) -> RegionToken {
    RegionToken::HalfSpace {
        surface_idx: i,
        sense: if inside {
            HalfSpaceSense::Inside
        } else {
            HalfSpaceSense::Outside
        },
    }
}

fn and(mut v: Vec<RegionToken>) -> Vec<RegionToken> {
    let n = v.len();
    for _ in 1..n {
        v.push(RegionToken::Intersection);
    }
    v
}

fn sphere(r: f64) -> SurfaceKind {
    SurfaceKind::Sphere(Sphere {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        r,
        bc: T,
    })
}

/// Materials: 0 kernel, 1 helium, 2 pebble shell, 3 matrix, 4 reflector,
/// 5 coating.
fn model() -> Geometry {
    let surfaces = vec![
        SurfaceKind::XPlane(XPlane { x0: -20.0, bc: T }),
        SurfaceKind::XPlane(XPlane { x0: 20.0, bc: T }),
        SurfaceKind::YPlane(YPlane { y0: -20.0, bc: T }),
        SurfaceKind::YPlane(YPlane { y0: 20.0, bc: T }),
        SurfaceKind::ZPlane(ZPlane { z0: -10.0, bc: T }),
        SurfaceKind::ZPlane(ZPlane { z0: 10.0, bc: T }),
        SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: 12.0,
            bc: T,
        }),
        sphere(2.8),
        sphere(2.0),
        sphere(0.25),
        sphere(0.4),
    ];
    let core = and(vec![hs(6, true), hs(4, false), hs(5, true)]);
    let mut refl = and(vec![
        hs(0, false),
        hs(1, true),
        hs(2, false),
        hs(3, true),
        hs(4, false),
        hs(5, true),
    ]);
    refl.extend(core.clone());
    refl.push(RegionToken::Complement);
    refl.push(RegionToken::Intersection);
    let cells = vec![
        Cell::fill(1, core, CellFill::Lattice(0), Position::ZERO),
        Cell::material(2, refl, 4, 293.6),
        Cell::fill(3, vec![hs(8, true)], CellFill::Lattice(1), Position::ZERO),
        Cell::material(4, and(vec![hs(8, false), hs(7, true)]), 2, 293.6),
        Cell::material(5, vec![hs(7, false)], 1, 293.6),
        Cell::material(6, vec![hs(9, true)], 0, 293.6),
        Cell::material(7, and(vec![hs(9, false), hs(10, true)]), 5, 293.6),
        Cell::material(8, vec![hs(10, false)], 3, 293.6),
        Cell::material(9, vec![], 3, 293.6),
        Cell::material(10, vec![], 1, 293.6),
    ];
    let universes = vec![
        Universe {
            id: 0,
            cell_indices: vec![0, 1],
        },
        Universe {
            id: 1,
            cell_indices: vec![2, 3, 4],
        },
        Universe {
            id: 2,
            cell_indices: vec![5, 6, 7],
        },
        Universe {
            id: 3,
            cell_indices: vec![8],
        },
        Universe {
            id: 4,
            cell_indices: vec![9],
        },
    ];
    let n_rings = 2;
    let side = 2 * n_rings - 1;
    let n_axial = 3;
    let mut map = vec![-1i32; side * side * n_axial];
    for iz in 0..n_axial {
        for iy in 0..side {
            for ix in 0..side {
                let s = (ix + iy) as i32;
                if s > n_rings as i32 - 2 && s < 3 * n_rings as i32 - 2 {
                    map[side * side * iz + side * iy + ix] = 1;
                }
            }
        }
    }
    let hex = HexLattice {
        id: 1,
        orientation: HexOrientation::Y,
        n_rings,
        n_axial,
        center: Position::ZERO,
        pitch: [6.0, 6.0],
        universes: map,
        outer: Some(4),
    };
    let rect = RectLattice {
        id: 2,
        n: [5, 5, 5],
        lower_left: Position::new(-2.25, -2.25, -2.25),
        pitch: [0.9, 0.9, 0.9],
        universes: vec![2; 125],
        outer: Some(3),
    };
    Geometry {
        surfaces,
        cells,
        universes,
        lattices: vec![Lattice::Hex(hex), Lattice::Rect(rect)],
        root_universe: 0,
    }
}

fn scheme() -> ColourScheme {
    let mut seed = 1;
    let pal = [
        Rgb::new(200, 40, 40),
        Rgb::new(230, 230, 250),
        Rgb::new(90, 90, 90),
        Rgb::new(60, 60, 60),
        Rgb::new(150, 120, 80),
        Rgb::new(240, 200, 60),
    ];
    let mut s = ColourScheme::new(PlotColourBy::Material, pal.len(), &mut seed)
        .with_background(Rgb::new(48, 48, 52));
    for (i, c) in pal.iter().enumerate() {
        s = s.with_colour(i, *c);
    }
    s
}

fn camera(eye: [f64; 3], projection: Projection) -> Camera {
    Camera {
        position: Position::new(eye[0], eye[1], eye[2]),
        look_at: Position::ZERO,
        up: Direction::new(0.0, 0.0, 1.0),
        pixels: [240, 180],
        projection,
    }
}

fn solid(cam: Camera, clip: Option<ClipPlane>) -> SolidRayTracePlot {
    let mut p = SolidRayTracePlot::new(cam, 6);
    for i in [0, 2, 3, 4, 5] {
        p = p.with_opaque(i);
    }
    p.diffuse_fraction = 0.35;
    p.clip = clip;
    p
}

/// With `GPU_PARITY_DUMP=<dir>`, write both pictures as PNGs for a human.
fn dump(name: &str, cpu: &ImageData, gpu: &ImageData) {
    if let Ok(dir) = std::env::var("GPU_PARITY_DUMP") {
        let stem = name.replace([' ', ',', '.'], "_");
        let dir = std::path::Path::new(&dir);
        let _ = std::fs::create_dir_all(dir);
        let _ = cpu.write_png(&dir.join(format!("{stem}_cpu.png")));
        let _ = gpu.write_png(&dir.join(format!("{stem}_gpu.png")));
    }
}

fn mismatch(a: &[i32], b: &[i32]) -> f64 {
    let n = a.iter().zip(b).filter(|(x, y)| x != y).count();
    n as f64 / a.len() as f64
}

fn colour_mismatch(a: &[Rgb], b: &[Rgb]) -> f64 {
    let d = |x: u8, y: u8| (i32::from(x) - i32::from(y)).abs() > 2;
    let n = a
        .iter()
        .zip(b)
        .filter(|(p, q)| d(p.r, q.r) || d(p.g, q.g) || d(p.b, q.b))
        .count();
    n as f64 / a.len() as f64
}

/// The WGSL parses and validates with naga, no adapter needed (runs on CI).
#[test]
fn shader_validates_without_a_gpu() {
    let module = naga::front::wgsl::parse_str(outram_blender::csg::gpu::render::SHADER)
        .unwrap_or_else(|e| panic!("WGSL parse: {e:?}"));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("WGSL validation: {e:?}"));
}

#[test]
fn gpu_trace_matches_the_cpu_plotter() {
    let Some(mut gpu) = probe_renderer() else {
        println!("SKIP gpu_trace_matches_the_cpu_plotter: no GPU adapter");
        return;
    };
    let g = model();
    gpu.set_geometry(&flatten(&g).expect("flattens"));
    let sch = scheme();
    let persp = Projection::Perspective {
        horizontal_fov_deg: 50.0,
    };
    let views: Vec<(&str, SolidRayTracePlot)> = vec![
        (
            "solid perspective",
            solid(camera([60.0, -45.0, 35.0], persp), None),
        ),
        (
            "solid orthographic",
            solid(
                camera(
                    [60.0, -45.0, 35.0],
                    Projection::Orthographic { width: 50.0 },
                ),
                None,
            ),
        ),
        (
            "solid, x section at 0.3",
            solid(
                camera([60.0, -45.0, 35.0], persp),
                Some(ClipPlane {
                    normal: [-1.0, 0.0, 0.0],
                    offset: -0.3,
                }),
            ),
        ),
    ];
    println!("view | pixels | index mismatch | colour mismatch");
    for (name, plot) in views {
        let (cpu_img, cpu_ids) = plot.create_image_with_ids(&g, &sch);
        let frame = gpu.render(&GpuPlot::Solid(plot), &sch).expect("renders");
        let (gpu_img, gpu_ids) = gpu.read_back(&frame).expect("reads back");
        let m = mismatch(&cpu_ids, &gpu_ids);
        let c = colour_mismatch(&cpu_img.pixels, &gpu_img.pixels);
        dump(name, &cpu_img, &gpu_img);
        println!(
            "{name} | {} | {:.4} % | {:.4} %",
            cpu_ids.len(),
            100.0 * m,
            100.0 * c
        );
        assert!(m <= 0.01, "{name}: {:.3} % of pixels differ", 100.0 * m);
        let shown = cpu_ids.iter().filter(|&&i| i >= 0).count();
        assert!(
            shown > cpu_ids.len() / 10,
            "{name}: the model fills the view"
        );
    }

    for (name, plot) in [
        (
            "x-z slice",
            SlicePlot::new(
                PlotBasis::Xz,
                Position::new(0.0, 0.1, 0.0),
                [44.0, 24.0],
                [440, 240],
            ),
        ),
        (
            "x-y slice in one pebble",
            SlicePlot::new(
                PlotBasis::Xy,
                Position::new(0.0, 0.0, 0.1),
                [6.0, 6.0],
                [300, 300],
            ),
        ),
    ] {
        let cpu: Vec<i32> = plot
            .id_map(&g)
            .hits
            .iter()
            .map(|h| match *h {
                SliceHit::NotFound => -1,
                SliceHit::Overlap => -2,
                SliceHit::Found { material: None, .. } => -3,
                SliceHit::Found {
                    material: Some(m), ..
                } => m as i32,
            })
            .collect();
        let frame = gpu.render(&GpuPlot::Slice(plot), &sch).expect("renders");
        let (_, gpu_ids) = gpu.read_back(&frame).expect("reads back");
        let m = mismatch(&cpu, &gpu_ids);
        println!("{name} | {} | {:.4} % | -", cpu.len(), 100.0 * m);
        assert!(m <= 0.01, "{name}: {:.3} % of pixels differ", 100.0 * m);
        assert!(cpu.contains(&0), "{name}: kernels are in view");
    }

    let mut xray = WireframeRayTracePlot::new(camera([60.0, -45.0, 35.0], persp), 6);
    for i in 0..6 {
        xray = xray.with_xs(i, if i == 1 { 0.0 } else { 0.02 });
    }
    xray.wireframe_ids = vec![0, 2, 3, 4, 5];
    let cpu_img = xray.create_image(&g, &sch);
    let frame = gpu
        .render(&GpuPlot::Wireframe(xray), &sch)
        .expect("renders");
    let (gpu_img, _) = gpu.read_back(&frame).expect("reads back");
    let c = colour_mismatch(&cpu_img.pixels, &gpu_img.pixels);
    dump("x-ray", &cpu_img, &gpu_img);
    println!("x-ray | {} | - | {:.4} %", cpu_img.pixels.len(), 100.0 * c);
    assert!(c <= 0.05, "x-ray: {:.3} % of pixels differ", 100.0 * c);
}
