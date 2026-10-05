//! **The workbench's views of HTR-10, GPU against CPU** (gh:#587): pixel
//! parity of the GPU ray tracer with the CPU plotter on the assembled
//! explicit-TRISO bed, and the time each takes.
//!
//! # Methodology
//!
//! The preset's geometry is assembled exactly as the workbench's Step 1 does
//! (`nee_soon::htr10_rmc::core_model::assemble_explicit_triso(14, 12, 0)`:
//! Şeker's hexagonal bed lattice of 13-ball tiles, every fuel pebble's fuel
//! zone a rectangular TRISO lattice, explicit reflector with its borings).
//! Each view is drawn by the CPU plotter (`outram_blender::csg::plot`, the
//! OpenMC port verified against `openmc --plot`; `f64`; rows in parallel on
//! every core) and by the GPU tracer (`outram_blender::csg::gpu`; `f32`),
//! from the same plot description, with the workbench's palette and settings
//! (diffuse fraction 0.35, helium hidden, 50 degree field of view):
//!
//! - **bed, half-section**: Step 1's view (cut at y = 0, the near half
//!   removed, yaw -1.1, pitch 0.3, framed on the model);
//! - **reflector, half-section**: Step 4's view (the pebbles hidden too);
//! - **one pebble, cut through a TRISO particle**: Step 2's view;
//! - **slices**: the x-z slice through the axis (whole model), an x-y slice
//!   at mid-bed, and the review gate's zoom on one fuel pebble.
//!
//! Compared: **the material index each pixel shows** (lighting cannot change
//! it). Pass criterion: at most 1 % of pixels differ. Timing: wall time of
//! one full-resolution render, CPU (`create_image_with_ids` / `id_map`) and
//! GPU (submit to completion, `CsgGpuRenderer::wait`, read-back excluded,
//! after one warm-up render). Skips, with a printed message, when no GPU
//! adapter is found.
//!
//! # Results (2026-10-05, NVIDIA RTX A5000 (Vulkan) against 16 CPU threads
//! of the same machine, shared with other jobs)
//!
//! Geometry: 43 445 cells, 174 surfaces, 1 502 universes, 2 lattices (root
//! universe 128 cells, 1 675 region tokens); flattened to 3.6 MB in 0.5-0.7 s
//! (off the UI thread in the workbench), uploaded in 2 ms.
//!
//! | view | pixels | index mismatch | CPU s | GPU s | GPU crossings / pixel |
//! |---|---|---|---|---|---|
//! | bed, half-section | 504 000 | 0.363 % | 6.95 | 0.411 | mean 9.0, max 35 |
//! | reflector, half-section | 504 000 | 0.0006 % | 9.08 | 0.408 | mean 9.1, max 35 |
//! | one pebble, cut through a particle | 504 000 | 0.006 % | 0.168 | 0.0135 | mean 0.2 |
//! | x-z slice, whole model | 900 000 | 0 % | 0.844 | 0.034 | — |
//! | x-y slice, mid-bed | 810 000 | 0 % | 0.642 | 0.048 | — |
//! | x-z slice, one fuel pebble | 810 000 | 0.004 % | 0.102 | 0.0083 | — |
//!
//! CPU times are with the machine shared (load average ~28 on 16 threads);
//! an earlier, less loaded run gave 5.2 s and 6.2 s for the two half-sections,
//! and GPU times vary by about 2x between runs (0.32-0.44 s, bed). So: the
//! GPU is 10-50x faster here, **not** interactive at full resolution on the
//! half-sections (~2.5 frames/s), which is why the workbench traces at half
//! resolution while the camera moves. The cost per crossing is high (~90 ns
//! of whole-GPU time) because a step in the reflector evaluates root cells
//! of up to 124 region tokens; a per-cell spatial index is not built.
//!
//! The bed view's 0.36 % of differing pixels sit along columns of pebbles
//! cut by the section plane y = 0 and on the reflector's silhouette edge
//! (`verification_and_validation/gpu_csg_parity/htr10_bed_half_section_diff.png`):
//! places where the plane or the ray grazes a boundary and `f32` and `f64`
//! round to different sides. Before the tracer skipped fully hidden subtrees
//! the reflector view (pebbles hidden) differed in 0.59 % of pixels, rays
//! having walked every hidden TRISO lattice in `f32`; skipping them took it
//! to 0.0006 % and its time from 0.74 s to 0.41 s.
//!
//! Re-run with
//! `cargo test --release -p dhoby-ghaut --test gpu_csg_parity -- --nocapture`
//! (`GPU_PARITY_DUMP=<dir>` writes every CPU and GPU picture).

#![cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]

use std::time::Instant;

use nee_soon::htr10_rmc::core_model::{assemble_explicit_triso, mat};
use outram_blender::csg::gpu::flat::flatten;
use outram_blender::csg::gpu::render::{probe_renderer, CsgGpuRenderer, GpuPlot};
use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::plot::{
    material_count, Camera, ClipPlane, ColourScheme, ImageData, PlotBasis, PlotColourBy,
    Projection, Rgb, SliceHit, SlicePlot, SolidRayTracePlot, DEFAULT_PLOTTER_SEED,
};
use outram_mc_libs::geometry::position::{Direction, Position};

fn scheme(g: &Geometry, background: Rgb) -> ColourScheme {
    let pal = nee_soon::htr10_rmc::plots::palette();
    let n = material_count(g).max(pal.len());
    let mut seed = DEFAULT_PLOTTER_SEED;
    let mut s = ColourScheme::new(PlotColourBy::Material, n, &mut seed).with_background(background);
    for (i, (c, _)) in pal.iter().enumerate() {
        s = s.with_colour(i, *c);
    }
    s
}

/// `View3d::eye` of the workbench.
fn eye(target: [f64; 3], yaw: f64, pitch: f64, distance: f64) -> Position {
    Position::new(
        target[0] + distance * pitch.cos() * yaw.cos(),
        target[1] + distance * pitch.cos() * yaw.sin(),
        target[2] + distance * pitch.sin(),
    )
}

fn view(
    n: usize,
    target: [f64; 3],
    extent: f64,
    yaw: f64,
    pitch: f64,
    cut_y: f64,
    shown: impl Fn(usize) -> bool,
) -> SolidRayTracePlot {
    let camera = Camera {
        position: eye(target, yaw, pitch, 2.6 * extent),
        look_at: Position::new(target[0], target[1], target[2]),
        up: Direction::new(0.0, 0.0, 1.0),
        pixels: [900, 560],
        projection: Projection::Perspective {
            horizontal_fov_deg: 50.0,
        },
    };
    let mut p = SolidRayTracePlot::new(camera, n);
    for i in (0..n).filter(|&i| shown(i)) {
        p = p.with_opaque(i);
    }
    p.diffuse_fraction = 0.35;
    p.with_clip(ClipPlane {
        normal: [0.0, 1.0, 0.0],
        offset: cut_y,
    })
}

fn mismatch(a: &[i32], b: &[i32]) -> f64 {
    a.iter().zip(b).filter(|(x, y)| x != y).count() as f64 / a.len() as f64
}

fn dump(name: &str, cpu: &ImageData, gpu: &ImageData) {
    if let Ok(dir) = std::env::var("GPU_PARITY_DUMP") {
        let stem = name.replace([' ', ',', '.', '-'], "_");
        let dir = std::path::Path::new(&dir);
        let _ = std::fs::create_dir_all(dir);
        let _ = cpu.write_png(&dir.join(format!("{stem}_cpu.png")));
        let _ = gpu.write_png(&dir.join(format!("{stem}_gpu.png")));
    }
}

fn gpu_time(gpu: &CsgGpuRenderer, plot: &GpuPlot, s: &ColourScheme) -> (f64, ImageData, Vec<i32>) {
    // Warm-up (pipeline caches, first-touch of buffers), then the timed run.
    let f = gpu.render(plot, s).expect("renders");
    gpu.wait().expect("runs");
    let t = Instant::now();
    let f2 = gpu.render(plot, s).expect("renders");
    gpu.wait().expect("runs");
    let secs = t.elapsed().as_secs_f64();
    drop(f);
    let (img, ids) = gpu.read_back(&f2).expect("reads back");
    (secs, img, ids)
}

/// A fuel pebble's centre and one TRISO particle's centre (the workbench's
/// `engine::find_fuel_pebble`, a 2-D scan stepped through one TRISO pitch).
fn fuel_pebble(g: &Geometry) -> ([f64; 3], [f64; 3]) {
    let u = Direction::new(0.0, 0.0, 1.0);
    for (kz, j) in (0..20).flat_map(|kz| (0..40).map(move |j| (kz, j))) {
        for i in 0..4000 {
            let p = Position::new(0.01 * i as f64, 0.01 * j as f64, 0.01 * kz as f64);
            if let Some(path) = g.locate(p, u, SurfaceToken::NONE) {
                let n = path.levels.len();
                if path.material == Some(mat::KERNEL) && n >= 3 {
                    let tile = &path.levels[n - 2];
                    let peb = tile.offset + g.cells[tile.cell].translation;
                    let part = path.levels[n - 1].offset;
                    return ([peb.x, peb.y, peb.z], [part.x, part.y, part.z]);
                }
            }
        }
    }
    panic!("no fuel pebble found");
}

#[test]
fn htr10_views_gpu_match_cpu() {
    let Some(mut gpu) = probe_renderer() else {
        println!("SKIP htr10_views_gpu_match_cpu: no GPU adapter");
        return;
    };
    let t = Instant::now();
    let core = assemble_explicit_triso(14, 12, 0);
    let g = &core.geometry;
    println!(
        "assembled: {} cells, {} surfaces, {} universes, {} lattices in {:.1} s",
        g.cells.len(),
        g.surfaces.len(),
        g.universes.len(),
        g.lattices.len(),
        t.elapsed().as_secs_f64()
    );
    let root = &g.universes[g.root_universe].cell_indices;
    let toks: Vec<usize> = root.iter().map(|&c| g.cells[c].region.len()).collect();
    println!(
        "root universe: {} cells, {} region tokens in all, largest {}",
        root.len(),
        toks.iter().sum::<usize>(),
        toks.iter().max().copied().unwrap_or(0)
    );
    let t = Instant::now();
    let flat = flatten(g).expect("HTR-10 flattens");
    let t_flat = t.elapsed().as_secs_f64();
    let t = Instant::now();
    gpu.set_geometry(&flat);
    gpu.wait().expect("upload");
    println!(
        "flattened: {:.1} MB, depth {}, in {:.3} s; uploaded in {:.3} s",
        flat.bytes() as f64 / 1e6,
        flat.depth,
        t_flat,
        t.elapsed().as_secs_f64()
    );

    let s3d = scheme(g, Rgb::new(48, 48, 52));
    let n = s3d.colours.len();
    let mid = [0.0, 0.0, 0.5 * (core.refl_bottom + core.refl_top)];
    let half = 0.5 * (core.refl_top - core.refl_bottom);
    let (pebble, particle) = fuel_pebble(g);
    let is_pebble = |i: usize| i <= mat::GRAPHITE || i == mat::HOMOG_DUMMY;

    println!("view | pixels | index mismatch | CPU s | GPU s | speed-up");
    let mut worst: f64 = 0.0;
    let solids = [
        (
            "bed, half-section",
            view(n, mid, half, -1.1, 0.3, 0.0, |i| i != mat::HELIUM),
        ),
        (
            "reflector, half-section",
            view(n, mid, half, -1.1, 0.3, 0.0, |i| {
                i != mat::HELIUM && !is_pebble(i)
            }),
        ),
        (
            "one pebble, cut through a particle",
            view(n, pebble, 4.0, -1.3, 0.25, particle[1], |i| {
                i != mat::HELIUM
            }),
        ),
    ];
    for (name, plot) in solids {
        let t = Instant::now();
        let (cpu_img, cpu_ids) = plot.create_image_with_ids(g, &s3d);
        let t_cpu = t.elapsed().as_secs_f64();
        let gplot = GpuPlot::Solid(plot);
        let (t_gpu, gpu_img, gpu_ids) = gpu_time(&gpu, &gplot, &s3d);
        let steps = gpu
            .read_steps(&gpu.render(&gplot, &s3d).expect("renders"))
            .expect("reads back");
        let mean = steps.iter().map(|&s| f64::from(s)).sum::<f64>() / steps.len() as f64;
        println!(
            "  {name}: boundary crossings per pixel, mean {mean:.1}, max {}",
            steps.iter().max().copied().unwrap_or(0)
        );
        dump(name, &cpu_img, &gpu_img);
        let m = mismatch(&cpu_ids, &gpu_ids);
        worst = worst.max(m);
        println!(
            "{name} | {} | {:.4} % | {t_cpu:.3} | {t_gpu:.4} | {:.0}x",
            cpu_ids.len(),
            100.0 * m,
            t_cpu / t_gpu
        );
        assert!(
            cpu_ids.iter().filter(|&&i| i >= 0).count() > cpu_ids.len() / 20,
            "{name}: the model is in view"
        );
    }

    let s2d = scheme(g, Rgb::new(235, 235, 235));
    let slices = [
        (
            "x-z slice, whole model",
            SlicePlot::new(
                PlotBasis::Xz,
                Position::new(0.0, 0.0, mid[2]),
                [2.2 * half * 900.0 / 1000.0, 2.2 * half],
                [900, 1000],
            ),
        ),
        (
            "x-y slice, mid-bed",
            SlicePlot::new(
                PlotBasis::Xy,
                Position::new(0.0, 0.0, 0.0),
                [400.0, 400.0],
                [900, 900],
            ),
        ),
        (
            "x-z slice, one fuel pebble",
            SlicePlot::new(
                PlotBasis::Xz,
                Position::new(pebble[0], particle[1], pebble[2]),
                [7.0, 7.0],
                [900, 900],
            ),
        ),
    ];
    for (name, plot) in slices {
        let t = Instant::now();
        let cpu: Vec<i32> = plot
            .id_map(g)
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
        let t_cpu = t.elapsed().as_secs_f64();
        let cpu_img = plot.create_image(g, &s2d);
        let (t_gpu, gpu_img, gpu_ids) = gpu_time(&gpu, &GpuPlot::Slice(plot), &s2d);
        dump(name, &cpu_img, &gpu_img);
        let m = mismatch(&cpu, &gpu_ids);
        worst = worst.max(m);
        println!(
            "{name} | {} | {:.4} % | {t_cpu:.3} | {t_gpu:.4} | {:.0}x",
            cpu.len(),
            100.0 * m,
            t_cpu / t_gpu
        );
    }
    assert!(
        worst <= 0.01,
        "worst view: {:.3} % of pixels differ",
        100.0 * worst
    );
}
