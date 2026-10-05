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
//! after one warm-up render; since the second run below, the fastest of
//! three). Skips, with a printed message, when no GPU adapter is found.
//!
//! Since 2026-10-05 (second run) every view is drawn on the GPU twice: with
//! the grid index (`flatten`, what the workbench uses) and without it
//! (`flatten_with(.., false)`, the tracer as it was in the first run), and
//! the same views are drawn for a **DEM-poured bed**
//! ([`dem_bed_views_gpu_match_cpu`]).
//!
//! # Results, first run (2026-10-05, NVIDIA RTX A5000 (Vulkan) against 16
//! CPU threads of the same machine, shared with other jobs)
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
//! resolution while the camera moves. ~~The cost per crossing is high (~90 ns
//! of whole-GPU time) because a step in the reflector evaluates root cells
//! of up to 124 region tokens; a per-cell spatial index is not built.~~
//! **CORRECTED 2026-10-05 (second run, below):** the diagnosis was wrong.
//! Counting the work per pixel showed ~10 full scans of the 128-cell root
//! universe per pixel, made almost all by rays **outside the model**: a ray
//! that has not reached the model steps from one root-cell surface to the
//! next, and the root cells' planes, cones and borings extend to infinity,
//! so a ray crossed ~9 (half-section) to ~22 (whole view) of those
//! extensions before it reached the reflector, scanning all 1 675 tokens at
//! each. The "crossings per pixel" above are mostly those entry steps.
//! Folding the reflector's 124-token cells alone (the index without a
//! closed grid) made the frame *slower* (0.32 s against 0.21 s).
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
//! # Results, second run (2026-10-05, same machine and GPU, the GPU shared with
//! the desktop; GPU s = fastest of three runs)
//!
//! The grid index (`outram_blender::csg::gpu::index`) on the root universe:
//! 30 x 30 x 47 voxels, 1 966 distinct lists, 2.07 candidate cells and 3.7
//! folded tokens per voxel (1 675 unfolded). The grid is **closed** (no root
//! cell outside it) once eight cells whose OpenMC bounding box is unbounded
//! (the seven small-absorber-sphere channels, whose slot section lies
//! between oblique planes, and the core cell, whose conus is cut from a
//! cone) are bounded by `index`'s tightened box; outside a closed grid a
//! ray jumps straight to the grid. Flattened: 4.1 MB (3.6 MB without the index)
//! in 0.7 s.
//!
//! Şeker lattice bed (`htr10_views_gpu_match_cpu`):
//!
//! | view | pixels | mismatch, indexed / unindexed | CPU s | GPU s indexed | GPU s unindexed | crossings / pixel, indexed (unindexed) |
//! |---|---|---|---|---|---|---|
//! | bed, half-section | 504 000 | 0.371 % / 0.363 % | 4.27 | 0.024-0.060 | 0.356-0.386 | mean 0.2, max 13 (9.0, 35); voxel steps mean 1.9 |
//! | reflector, half-section | 504 000 | 0.0006 % / 0.0006 % | 5.32 | 0.019-0.021 | 0.356-0.359 | mean 0.2, max 26 (9.1, 35); voxel steps mean 3.8 |
//! | one pebble, cut through a particle | 504 000 | 0.006 % / 0.006 % | 0.070 | 0.0058 | 0.0063 | mean 0.2 |
//! | x-z slice, whole model | 900 000 | 0 % / 0 % | 0.362 | 0.0014 | 0.0176 | — |
//! | x-y slice, mid-bed | 810 000 | 0 % / 0 % | 0.326 | 0.0021 | 0.0166 | — |
//! | x-z slice, one fuel pebble | 810 000 | 0.0036 % / 0.0036 % | 0.048 | 0.0045 | 0.0053 | — |
//!
//! DEM bed (`dem_bed_views_gpu_match_cpu`; 125 620 cells, 248 066 surfaces,
//! 3 034 universes; flattened to 28.7 MB, 28.1 MB without the index; a
//! second universe indexed, a 72-cell, 516-token tile):
//!
//! | view | pixels | mismatch, indexed / unindexed | CPU s | GPU s indexed | GPU s unindexed |
//! |---|---|---|---|---|---|
//! | bed, half-section | 504 000 | 0.306 % / 0.297 % | 4.28 | 0.033 | 0.388 |
//! | reflector, half-section | 504 000 | 0.0008 % / 0.0008 % | 6.83 | 0.020 | 0.353 |
//! | one pebble, cut through a particle | 504 000 | 0.0042 % / 0.0042 % | 0.245 | 0.018 | 0.018 |
//! | x-z slice, whole model | 900 000 | 0 % / 0 % | 0.391 | 0.0024 | 0.018 |
//! | x-y slice, mid-bed | 810 000 | 0.0004 % / 0.0004 % | 0.325 | 0.0047 | 0.017 |
//! | x-z slice, one fuel pebble | 810 000 | 0.0017 % / 0.0017 % | 0.105 | 0.0054 | 0.0055 |
//!
//! Interpretation. The index changes almost no pixel: the indexed and
//! unindexed GPU pictures differ from the CPU's by the same fraction to
//! within 0.01 % of pixels (the few that differ between them lie on the
//! same grazing boundaries). The half-sections drop from ~0.36 s to 20-60
//! ms a frame at full resolution, the target of < 50 ms met for the
//! reflector view and, in a dedicated five-run benchmark (min 17 ms), for
//! the bed view; the spread is the shared GPU (a single run varies 2-4x).
//! In the running workbench (Xvfb, Step 3) the status line read "GPU
//! ray-traced in 27 ms" (was 643 ms) and the x-ray 117 ms (was 2.2 s). The
//! CPU plotter takes the same entry walk; it is not changed here (it is the
//! reference, and the OpenMC port).
//!
//! Re-run with
//! `cargo test --release -p dhoby-ghaut --test gpu_csg_parity -- --nocapture --test-threads=1`
//! (`GPU_PARITY_DUMP=<dir>` writes every CPU and GPU picture).

#![cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]

use std::time::Instant;

use nee_soon::htr10_rmc::core_model::{assemble_explicit_triso, mat};
use outram_blender::csg::gpu::flat::{flatten, flatten_with};
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
    // Warm-up (pipeline caches, first-touch of buffers, GPU clocks out of
    // idle), then the fastest of three timed runs: the GPU is shared with
    // the desktop, and a single run varies by 2-4x (2026-10-05).
    let f = gpu.render(plot, s).expect("renders");
    gpu.wait().expect("runs");
    drop(f);
    let mut best = f64::INFINITY;
    let mut last = None;
    for _ in 0..3 {
        let t = Instant::now();
        let f2 = gpu.render(plot, s).expect("renders");
        gpu.wait().expect("runs");
        best = best.min(t.elapsed().as_secs_f64());
        last = Some(f2);
    }
    let (img, ids) = gpu
        .read_back(&last.expect("three runs"))
        .expect("reads back");
    (best, img, ids)
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
    let (Some(gpu), Some(gpu_u)) = (probe_renderer(), probe_renderer()) else {
        println!("SKIP htr10_views_gpu_match_cpu: no GPU adapter");
        return;
    };
    let t = Instant::now();
    let core = assemble_explicit_triso(14, 12, 0);
    println!(
        "Seker lattice bed assembled in {:.1} s",
        t.elapsed().as_secs_f64()
    );
    let worst = compare_views("htr10", &core, gpu, gpu_u);
    assert!(
        worst <= 0.01,
        "worst view: {:.3} % of pixels differ",
        100.0 * worst
    );
}

/// **A DEM-poured bed through the same GPU path** (Step 1 with a finished
/// pour, `Req::AssembleFromCentres` in the workbench): the 27 554 pebble
/// centres of `reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv`
/// (an `outram-park-fork-liggghts` pour, DEM frame, metres), fuel assigned
/// by `fuel_assignment_at` at the default design's 0.57, assembled by
/// `nee_soon::htr10_rmc::explicit_bed::assemble_explicit_triso_from_centres_with`
/// exactly as the workbench's engine does, then the same views, the same
/// comparison and the same 1 % criterion as the lattice bed.
///
/// Results: see the module docs.
#[test]
fn dem_bed_views_gpu_match_cpu() {
    use nee_soon::htr10_rmc::core_design::Htr10CoreDesign;
    use nee_soon::htr10_rmc::explicit_bed::{
        assemble_explicit_triso_from_centres_with, fuel_assignment_at,
    };
    use uom::si::f64::Length;
    use uom::si::length::meter;
    let (Some(gpu), Some(gpu_u)) = (probe_renderer(), probe_renderer()) else {
        println!("SKIP dem_bed_views_gpu_match_cpu: no GPU adapter");
        return;
    };
    let csv = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv");
    let text = std::fs::read_to_string(&csv).expect("the pour CSV is in the repository");
    let centres: Vec<[Length; 3]> = text
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<f64> = l.split(',').filter_map(|v| v.trim().parse().ok()).collect();
            [f[1], f[2], f[3]].map(Length::new::<meter>)
        })
        .collect();
    let t = Instant::now();
    let design = Htr10CoreDesign::default();
    let fuel = fuel_assignment_at(&centres, design.fuel_ball_fraction);
    let core = assemble_explicit_triso_from_centres_with(&centres, &fuel, 14, 0, &design);
    println!(
        "DEM bed: {} pebbles assembled in {:.1} s",
        centres.len(),
        t.elapsed().as_secs_f64()
    );
    let worst = compare_views("dem_bed", &core, gpu, gpu_u);
    assert!(
        worst <= 0.01,
        "worst view: {:.3} % of pixels differ",
        100.0 * worst
    );
}

/// Every view, CPU against the GPU with and without the grid index; the
/// worst mismatch fraction.
fn compare_views(
    label: &str,
    core: &nee_soon::htr10_rmc::core_model::AssembledCore,
    mut gpu: CsgGpuRenderer,
    mut gpu_u: CsgGpuRenderer,
) -> f64 {
    let g = &core.geometry;
    println!(
        "geometry: {} cells, {} surfaces, {} universes, {} lattices",
        g.cells.len(),
        g.surfaces.len(),
        g.universes.len(),
        g.lattices.len(),
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
    gpu.set_geometry(&flat).expect("fits the device");
    gpu.wait().expect("upload");
    println!(
        "flattened: {:.1} MB, depth {}, in {:.3} s; uploaded in {:.3} s",
        flat.bytes() as f64 / 1e6,
        flat.depth,
        t_flat,
        t.elapsed().as_secs_f64()
    );
    for st in &flat.grids {
        println!(
            "  grid index on universe {} ({} cells): {:?} voxels, {} lists, \
             {:.2} candidates and {:.1} tokens per voxel (full: {})",
            st.universe,
            g.universes[st.universe].cell_indices.len(),
            st.n,
            st.lists,
            st.mean_candidates,
            st.mean_tokens,
            st.full_tokens
        );
    }
    // The ablation: every universe scanned whole (the tracer before the
    // grid index, 2026-10-05).
    let t = Instant::now();
    let flat_u = flatten_with(g, false).expect("HTR-10 flattens");
    println!(
        "flattened without the index: {:.1} MB in {:.3} s",
        flat_u.bytes() as f64 / 1e6,
        t.elapsed().as_secs_f64()
    );
    gpu_u.set_geometry(&flat_u).expect("fits the device");
    gpu_u.wait().expect("upload");

    let s3d = scheme(g, Rgb::new(48, 48, 52));
    let n = s3d.colours.len();
    let mid = [0.0, 0.0, 0.5 * (core.refl_bottom + core.refl_top)];
    let half = 0.5 * (core.refl_top - core.refl_bottom);
    let (pebble, particle) = fuel_pebble(g);
    let is_pebble = |i: usize| i <= mat::GRAPHITE || i == mat::HOMOG_DUMMY;

    println!(
        "view | pixels | index mismatch (indexed / unindexed) | CPU s | GPU s indexed | GPU s unindexed"
    );
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
        let (t_gpu_u, _, gpu_ids_u) = gpu_time(&gpu_u, &gplot, &s3d);
        for (label, r) in [("indexed", &gpu), ("unindexed", &gpu_u)] {
            let work = r
                .read_work(&r.render(&gplot, &s3d).expect("renders"))
                .expect("reads back");
            let n = work.len() as f64;
            let mean_c = work.iter().map(|w| f64::from(w.0)).sum::<f64>() / n;
            let mean_v = work.iter().map(|w| f64::from(w.1)).sum::<f64>() / n;
            println!(
                "  {name}, {label}: per pixel, boundary crossings mean {mean_c:.1} max {}, \
                 voxel steps mean {mean_v:.1} max {}",
                work.iter().map(|w| w.0).max().unwrap_or(0),
                work.iter().map(|w| w.1).max().unwrap_or(0)
            );
        }
        dump(&format!("{label} {name}"), &cpu_img, &gpu_img);
        let m = mismatch(&cpu_ids, &gpu_ids);
        let mu = mismatch(&cpu_ids, &gpu_ids_u);
        worst = worst.max(m).max(mu);
        println!(
            "{name} | {} | {:.4} % / {:.4} % | {t_cpu:.3} | {t_gpu:.4} | {t_gpu_u:.4}",
            cpu_ids.len(),
            100.0 * m,
            100.0 * mu
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
        let gplot = GpuPlot::Slice(plot);
        let (t_gpu, gpu_img, gpu_ids) = gpu_time(&gpu, &gplot, &s2d);
        let (t_gpu_u, _, gpu_ids_u) = gpu_time(&gpu_u, &gplot, &s2d);
        dump(&format!("{label} {name}"), &cpu_img, &gpu_img);
        let m = mismatch(&cpu, &gpu_ids);
        let mu = mismatch(&cpu, &gpu_ids_u);
        worst = worst.max(m).max(mu);
        println!(
            "{name} | {} | {:.4} % / {:.4} % | {t_cpu:.3} | {t_gpu:.4} | {t_gpu_u:.4}",
            cpu.len(),
            100.0 * m,
            100.0 * mu
        );
    }
    worst
}
