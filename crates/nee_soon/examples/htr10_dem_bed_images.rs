// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK. See `crates/nee_soon/src/htr10_rmc/mod.rs`
// for licence terms.

//! **Draw the HTR-10 core built on a DEM-settled bed** — the geometry-drawing
//! HARD RULE of this crate's `CLAUDE.md`, applied to
//! [`assemble_explicit_triso_from_centres`].
//!
//! ```bash
//! taskset -c 0-11 cargo run --release -p nee_soon --example htr10_dem_bed_images
//! # options: --csv <file> --core-balls <n> --out <dir> --keff --threads <n>
//! ```
//!
//! # Input
//!
//! `reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv` (header
//! `id,x,y,z,vx,vy,vz`, SI metres, 27 554 pebbles settled by this workspace's
//! LIGGGHTS port into the published conus + 25 cm discharge tube; provenance in
//! `reference-data/liggghts/README.md`). It is **trimmed by height** to the
//! first-criticality loading: every pebble centred at or below the bed floor
//! (`z <= 0`, the conus and tube) is kept, and of those above it the lowest
//! `--core-balls` (default 16 890, IAEA-TECDOC-1382 p. 251). The top of the
//! trimmed bed is therefore a flat cut through a random packing, not a poured
//! surface. Identity: [`paper_fuel_assignment`] (57:43 above the floor, conus
//! and tube all dummy).
//!
//! # Output (default `crates/nee_soon/verification_and_validation/htr10_dem_bed_images/`)
//!
//! Every pixel is a `Geometry::locate` on the ASSEMBLED geometry, with
//! OpenMC's overlap check on (`SlicePlot::showing_overlaps`): a pixel claimed
//! by two cells of one universe is drawn RED and counted. The printed log
//! states the counts. `dem_lens_cells.png` is coloured by bed-tile CELL, so the
//! two halves of a split lens show in two colours.
//!
//! With `--keff` it also runs a SMOKE `k_eff` (500 x \[10 + 20\]) on the
//! core: evidence that it transports without lost particles, not a V&V
//! result.

use std::path::PathBuf;
use std::time::Instant;

use nee_soon::htr10_rmc::bed::PebbleBed;
use nee_soon::htr10_rmc::core_model::{mat, HTR10_DISCHARGE_TUBE_RADIUS_CM, HTR10_REFLECTOR_OUTER_CM};
use nee_soon::htr10_rmc::explicit_bed::{
    assemble_explicit_triso_from_centres, paper_fuel_assignment, ExplicitBallSource,
};
use nee_soon::htr10_rmc::keff_vs_height::{RINGS, TEMPERATURE_K};
use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::plot::{
    annotate_slice, render_material_slice, ColourScheme, LegendEntry, PlotBasis, PlotColourBy, Rgb,
    SliceHit, SlicePlot, DEFAULT_PLOTTER_SEED,
};
use outram_mc_libs::geometry::plot::slice::check_cell_overlap;
use outram_mc_libs::geometry::position::{Direction, Position};
use uom::si::f64::Length;
use uom::si::length::meter;

struct Args {
    csv: PathBuf,
    core_balls: usize,
    out: PathBuf,
    keff: bool,
    threads: usize,
}

fn parse_args() -> Args {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut a = Args {
        csv: manifest.join("../../reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv"),
        core_balls: 16_890,
        out: manifest.join("verification_and_validation/htr10_dem_bed_images"),
        keff: false,
        threads: 12,
    };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        match k.as_str() {
            "--csv" => a.csv = PathBuf::from(it.next().expect("--csv <file>")),
            "--core-balls" => {
                a.core_balls = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .expect("--core-balls <n>")
            }
            "--out" => a.out = PathBuf::from(it.next().expect("--out <dir>")),
            "--keff" => a.keff = true,
            "--threads" => {
                a.threads = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .expect("--threads <n>")
            }
            o => panic!("unknown argument {o:?}"),
        }
    }
    a
}

/// Centres \[m\] from a `id,x,y,z,...` CSV.
fn load_csv(path: &PathBuf) -> Vec<[f64; 3]> {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let mut lines = text.lines();
    let header = lines.next().expect("a header");
    assert!(
        header.starts_with("id,x,y,z"),
        "unexpected header {header:?}"
    );
    lines
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<f64> = l
                .split(',')
                .skip(1)
                .take(3)
                .map(|v| v.trim().parse().expect("a number"))
                .collect();
            [f[0], f[1], f[2]]
        })
        .collect()
}

/// A fuel pebble's centre near `near` and one of its TRISO particles' centre,
/// read off the located path (as `htr10_geometry_images`).
fn find_kernel(g: &Geometry, near: [f64; 3]) -> Option<Position> {
    let u = Direction::new(0.0, 0.0, 1.0);
    for kz in 0..20 {
        for j in 0..40 {
            for i in 0..400 {
                let p = Position::new(
                    near[0] - 2.0 + 0.01 * f64::from(i),
                    near[1] + 0.01 * f64::from(j),
                    near[2] + 0.01 * f64::from(kz),
                );
                if let Some(path) = g.locate(p, u, SurfaceToken::NONE) {
                    if path.material == Some(mat::KERNEL) {
                        return path.levels.last().map(|c| c.offset);
                    }
                }
            }
        }
    }
    None
}

#[allow(clippy::too_many_lines)]
fn main() {
    let args = parse_args();
    std::fs::create_dir_all(&args.out).expect("create output directory");

    let all = load_csv(&args.csv);
    let mut below: Vec<[f64; 3]> = all.iter().copied().filter(|c| c[2] <= 0.0).collect();
    let mut above: Vec<[f64; 3]> = all.iter().copied().filter(|c| c[2] > 0.0).collect();
    above.sort_by(|p, q| p[2].total_cmp(&q[2]));
    let n_above_csv = above.len();
    above.truncate(args.core_balls);
    let n_below = below.len();
    below.extend(above);
    let centres: Vec<[Length; 3]> = below.iter().map(|c| c.map(Length::new::<meter>)).collect();
    let fuel = paper_fuel_assignment(&centres);
    println!(
        "DEM bed {}: {} pebbles; kept {} at or below the floor and the lowest {} of {} above it",
        args.csv.display(),
        all.len(),
        n_below,
        centres.len() - n_below,
        n_above_csv
    );

    let t = Instant::now();
    let core = assemble_explicit_triso_from_centres(&centres, &fuel, RINGS, 0);
    let build_s = t.elapsed().as_secs_f64();
    let Some(PebbleBed::Explicit(eb)) = core.bed.as_ref() else {
        unreachable!("an explicit bed")
    };
    let g = &core.geometry;
    let fillers = eb
        .source
        .iter()
        .filter(|s| matches!(s, ExplicitBallSource::TubeFiller))
        .count();
    let (elig, nfuel) = eb.eligible_and_fuel_balls();
    println!(
        "assembled in {build_s:.1} s: {} tiles ({} with balls), {} cells, {} universes, {} surfaces",
        core.tiles,
        eb.tile_balls.len(),
        core.cells,
        core.universes,
        g.surfaces.len()
    );
    println!(
        "balls: {} DEM + {fillers} tube filler; core balls (centre above floor) {}; fuel {nfuel} of {elig} ({:.4})",
        eb.centres.len() - fillers,
        eb.core_balls(),
        nfuel as f64 / elig as f64
    );
    println!(
        "frame: bed half-height {:.4} cm (highest ball top {:.4} cm above the floor), \
         p99 surface {:.4} cm, z_MC = 100 z_DEM {:+.4}",
        core.bed_half_height,
        2.0 * core.bed_half_height,
        eb.surface_height,
        eb.dem_to_mc_dz
    );
    println!(
        "axial: model {:.3} .. {:.3}, conus floor {:.3}, bed {:.3} .. {:.3}, cavity top {:.3} cm",
        core.refl_bottom,
        core.refl_top,
        core.conus_floor,
        -core.bed_half_height,
        core.bed_half_height,
        core.cavity_top
    );
    println!(
        "overlap: {} pairs split, max {:.5} cm ({:.3} % of r), lens volume fraction {:.3e}; \
         max wall penetration {:.5} cm; dropped {}",
        eb.overlaps.len(),
        eb.max_overlap,
        100.0 * eb.max_overlap / eb.radius,
        eb.lens_volume_fraction,
        eb.max_wall_penetration,
        eb.dropped_dem_balls
    );
    if let Some(cut) = eb.tube_cut {
        println!(
            "tube: DEM column down to {cut:.4} cm ({:.4} cm below the conus floor); filler below, \
             gap {:.4} cm; tube bottom {:.3} cm",
            core.conus_floor - cut,
            eb.tube_gap,
            eb.container_bottom
        );
    }

    // Point check: random points of the container, every located one tested
    // with OpenMC's overlap check at every level.
    let dir = Direction::new(0.0, 0.0, 1.0);
    let mut s: u64 = 12_345;
    let mut rnd = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 11) as f64 / (1u64 << 53) as f64
    };
    let (mut pts, mut lost, mut ovl) = (0usize, 0usize, 0usize);
    let z_lo = eb.container_bottom;
    let t = Instant::now();
    while pts < 200_000 {
        let p = Position::new(
            (2.0 * rnd() - 1.0) * 90.0,
            (2.0 * rnd() - 1.0) * 90.0,
            z_lo + rnd() * (core.bed_half_height - z_lo),
        );
        let rho = p.x.hypot(p.y);
        let r_wall = if p.z >= -core.bed_half_height {
            90.0
        } else if p.z >= core.conus_floor {
            HTR10_DISCHARGE_TUBE_RADIUS_CM
                + (90.0 - HTR10_DISCHARGE_TUBE_RADIUS_CM) * (p.z - core.conus_floor)
                    / (-core.bed_half_height - core.conus_floor)
        } else {
            HTR10_DISCHARGE_TUBE_RADIUS_CM
        };
        if rho >= r_wall {
            continue;
        }
        pts += 1;
        match g.locate(p, dir, SurfaceToken::NONE) {
            None => lost += 1,
            Some(path) => ovl += usize::from(check_cell_overlap(g, &path)),
        }
    }
    println!(
        "point check: {pts} points in the container, {lost} not located, {ovl} claimed by two cells ({:.1} s)",
        t.elapsed().as_secs_f64()
    );

    let pal = nee_soon::htr10_rmc::plots::palette();
    let tag = format!("HTR-10 DEM BED, {} CORE BALLS", eb.core_balls());
    let render = |name: &str, plot: SlicePlot, what: &str| {
        let t = Instant::now();
        let plot = plot.showing_overlaps();
        let (_raw, img) = render_material_slice(g, &plot, &pal, &format!("{tag}: {what}"));
        let n_ovl = plot
            .id_map(g)
            .hits
            .iter()
            .filter(|h| matches!(h, SliceHit::Overlap))
            .count();
        let path = args.out.join(name);
        img.write_png(&path).expect("write png");
        println!(
            "wrote {} ({}x{} px, {n_ovl} overlap pixels, {:.1} s)",
            path.display(),
            plot.pixels[0],
            plot.pixels[1],
            t.elapsed().as_secs_f64()
        );
    };
    let px = |w: f64, cm: f64| (w / cm).round() as usize;

    // Whole model, R-Z through the axis.
    let half_w = HTR10_REFLECTOR_OUTER_CM + 5.0;
    let (zl, zh) = (core.refl_bottom - 5.0, core.refl_top + 5.0);
    let w = 2.0 * half_w;
    render(
        "dem_rz_full.png",
        SlicePlot::new(
            PlotBasis::Xz,
            Position::new(0.0, 0.0, 0.5 * (zl + zh)),
            [w, zh - zl],
            [px(w, 0.4), px(zh - zl, 0.4)],
        ),
        "R-Z (X-Z) SLICE, Y = 0",
    );
    // The bed and conus, R-Z, finer.
    let (bl, bh) = (core.conus_floor - 40.0, core.bed_half_height + 15.0);
    render(
        "dem_rz_bed.png",
        SlicePlot::new(
            PlotBasis::Xz,
            Position::new(0.0, 0.0, 0.5 * (bl + bh)),
            [200.0, bh - bl],
            [px(200.0, 0.1), px(bh - bl, 0.1)],
        ),
        "X-Z, Y = 0: BED, CONUS AND TUBE TOP",
    );
    let xy = |z: f64| {
        SlicePlot::new(
            PlotBasis::Xy,
            Position::new(0.0, 0.0, z),
            [200.0, 200.0],
            [1000, 1000],
        )
    };
    let floor = -core.bed_half_height;
    render(
        "dem_xy_bed_mid.png",
        xy(floor + 0.5 * eb.surface_height),
        "X-Y AT HALF THE LOADING HEIGHT",
    );
    render(
        "dem_xy_conus.png",
        xy(0.5 * (floor + core.conus_floor)),
        "X-Y THROUGH THE CONUS",
    );
    if let Some(cut) = eb.tube_cut {
        render(
            "dem_xy_tube_dem.png",
            SlicePlot::new(
                PlotBasis::Xy,
                Position::new(0.0, 0.0, 0.5 * (cut + core.conus_floor)),
                [60.0, 60.0],
                [800, 800],
            ),
            "X-Y, DISCHARGE TUBE, DEM BALLS",
        );
        render(
            "dem_xy_tube_filler.png",
            SlicePlot::new(
                PlotBasis::Xy,
                Position::new(0.0, 0.0, cut - 30.0),
                [60.0, 60.0],
                [800, 800],
            ),
            "X-Y, DISCHARGE TUBE, SEKER FILLER BALLS",
        );
        render(
            "dem_xz_tube_junction.png",
            SlicePlot::new(
                PlotBasis::Xz,
                Position::new(0.0, 0.0, cut),
                [60.0, 60.0],
                [900, 900],
            ),
            "X-Z, Y = 0: DEM BALLS ABOVE, FILLER BELOW",
        );
    }
    render(
        "dem_xz_bed_top.png",
        SlicePlot::new(
            PlotBasis::Xz,
            Position::new(0.0, 0.0, core.bed_half_height - 15.0),
            [80.0, 40.0],
            [1200, 600],
        ),
        "X-Z, Y = 0: TOP OF THE TRIMMED BED",
    );
    let zc = 0.5 * (floor + core.conus_floor);
    let rc = 0.5 * (90.0 + HTR10_DISCHARGE_TUBE_RADIUS_CM);
    render(
        "dem_xz_conus_wall.png",
        SlicePlot::new(
            PlotBasis::Xz,
            Position::new(rc, 0.0, zc),
            [30.0, 30.0],
            [900, 900],
        ),
        "X-Z, Y = 0: ON THE CONUS SLOPE",
    );
    render(
        "dem_xy_bed_wall.png",
        SlicePlot::new(
            PlotBasis::Xy,
            Position::new(84.0, 0.0, floor + 30.0),
            [24.0, 24.0],
            [800, 800],
        ),
        "X-Y AT THE BED WALL",
    );

    // One fuel pebble near the axis at half the loading height, and its TRISO.
    let target = floor + 0.5 * eb.surface_height;
    let (ib, cb) = eb
        .centres
        .iter()
        .enumerate()
        .filter(|(i, _)| eb.fuel[*i])
        .min_by(|(_, a), (_, b)| {
            let da = a[0].hypot(a[1]) + (a[2] - target).abs();
            let db = b[0].hypot(b[1]) + (b[2] - target).abs();
            da.total_cmp(&db)
        })
        .map(|(i, c)| (i, *c))
        .expect("a fuel ball");
    println!(
        "zoom pebble: ball {ib} at ({:.4}, {:.4}, {:.4}) cm",
        cb[0], cb[1], cb[2]
    );
    if let Some(part) = find_kernel(g, cb) {
        println!(
            "a TRISO particle at ({:.4}, {:.4}, {:.4})",
            part.x, part.y, part.z
        );
        render(
            "dem_xz_pebbles.png",
            SlicePlot::new(
                PlotBasis::Xz,
                Position::new(cb[0], part.y, cb[2]),
                [30.0, 30.0],
                [1000, 1000],
            ),
            "X-Z, 30 CM ROUND ONE FUEL PEBBLE",
        );
        render(
            "dem_xz_one_pebble.png",
            SlicePlot::new(
                PlotBasis::Xz,
                Position::new(cb[0], part.y, cb[2]),
                [7.0, 7.0],
                [1000, 1000],
            ),
            "X-Z THROUGH ONE FUEL PEBBLE",
        );
        render(
            "dem_xy_triso.png",
            SlicePlot::new(PlotBasis::Xy, part, [0.8, 0.8], [800, 800]),
            "X-Y, 0.8 CM ROUND ONE TRISO PARTICLE",
        );
    } else {
        eprintln!("no kernel found near ball {ib}; pebble zooms skipped");
    }

    // The most-overlapping pair: a CELL-coloured slice at the bed-tile level
    // through both centres, overlaps shown.
    let d2 = |a: [f64; 3], b: [f64; 3]| {
        (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
    };
    if let Some(&(i, j)) = eb
        .overlaps
        .iter()
        .filter(|&&(i, j)| eb.centres[i][2].min(eb.centres[j][2]) > floor)
        .min_by(|&&(a, b), &&(c, d)| {
            d2(eb.centres[a], eb.centres[b]).total_cmp(&d2(eb.centres[c], eb.centres[d]))
        })
    {
        let (ci, cj) = (eb.centres[i], eb.centres[j]);
        let mid = Position::new(
            0.5 * (ci[0] + cj[0]),
            0.5 * (ci[1] + cj[1]),
            0.5 * (ci[2] + cj[2]),
        );
        let e = [cj[0] - ci[0], cj[1] - ci[1], cj[2] - ci[2]];
        // The basis whose normal is most nearly perpendicular to the centre line.
        let (basis, name) = [(PlotBasis::Xy, 2), (PlotBasis::Xz, 1), (PlotBasis::Yz, 0)]
            .into_iter()
            .min_by(|a, b| e[a.1].abs().total_cmp(&e[b.1].abs()))
            .expect("three bases");
        let _ = name;
        let d = d2(ci, cj).sqrt();
        println!(
            "lens zoom: balls {i} and {j}, {d:.5} cm apart (overlap {:.5} cm), basis {basis:?}",
            6.0 - d
        );
        let plot = SlicePlot::new(basis, mid, [1.2, 1.2], [1000, 1000])
            .at_level(1)
            .showing_overlaps();
        let ids = plot.id_map(g);
        let mut seed = DEFAULT_PLOTTER_SEED;
        let scheme = ColourScheme::new(PlotColourBy::Cell, g.cells.len(), &mut seed)
            .with_background(Rgb::new(235, 235, 235));
        let raw = plot.colour_id_map(&ids, &scheme);
        let mut seen: Vec<usize> = Vec::new();
        let mut n_ovl = 0;
        for h in &ids.hits {
            match *h {
                SliceHit::Found { cell: Some(c), .. } if !seen.contains(&c) => seen.push(c),
                SliceHit::Overlap => n_ovl += 1,
                _ => {}
            }
        }
        let legend: Vec<LegendEntry> = seen
            .iter()
            .map(|&c| LegendEntry::new(scheme.colours[c], format!("CELL ID {}", g.cells[c].id)))
            .collect();
        let img = annotate_slice(
            &raw,
            &plot,
            &format!(
                "{tag}: LENS OF THE MOST-OVERLAPPING PAIR, BY TILE CELL ({:.4} CM)",
                6.0 - d
            ),
            &legend,
        );
        let path = args.out.join("dem_lens_cells.png");
        img.write_png(&path).expect("write png");
        println!(
            "wrote {} ({n_ovl} overlap pixels, {} cells shown)",
            path.display(),
            seen.len()
        );
    }

    if args.keff {
        smoke_keff(&core, args.threads);
    }
}

/// A SMOKE run: 500 x [10 + 20], ENDF/B-VIII.0, the correct-physics default
/// data. Shows transport completes and how many histories are lost; it is not
/// a V&V number.
fn smoke_keff(core: &nee_soon::htr10_rmc::core_model::AssembledCore, threads: usize) {
    use nee_soon::htr10_rmc::data::{load_htr10_nuclides, Htr10DataConfig, Htr10NuclideLayout};
    use nee_soon::htr10_rmc::keff_vs_height::{bed_majorant, run_core, SweepStatistics};
    use nee_soon::htr10_rmc::materials::{htr10_material_set, Htr10MaterialConfig};
    use outram_mc_libs::run_diagnostics::RunDiagnostics;
    use uom::si::f64::ThermodynamicTemperature;
    use uom::si::thermodynamic_temperature::kelvin;

    let cfg = Htr10DataConfig {
        temperature: ThermodynamicTemperature::new::<kelvin>(TEMPERATURE_K),
        ..Htr10DataConfig::default()
    };
    let layout = Htr10NuclideLayout::plan(&cfg).expect("the default data configuration is valid");
    let mut diag = RunDiagnostics::new("htr10_dem_bed_images");
    let nucs = load_htr10_nuclides(&cfg, &layout, &mut diag).expect("nuclear data");
    let mats = htr10_material_set(
        &layout,
        Htr10MaterialConfig::benchmark_default(TEMPERATURE_K),
    );
    let majorant = bed_majorant(&mats, &nucs);
    let stats = SweepStatistics {
        name: "smoke",
        particles: 500,
        inactive: 10,
        active: 20,
        seed: 20_260_917,
    };
    let (p, _res) = run_core(0, core, &mats, &nucs, &majorant, &stats, threads);
    println!(
        "SMOKE k_eff (500 x [10 + 20], {threads} threads, seed {}): {:.5} +/- {:.5}; {} histories, \
         {} lost (not located); entropy {:?}; transport {:.1} s. Ball count {:?} -> Şeker-equivalent \
         height {:.2} cm, RMC there {:?}. NOT a V&V result.",
        stats.seed,
        p.k,
        p.sigma,
        p.histories,
        p.lost_locate,
        p.entropy_first_last,
        p.transport_time.value,
        p.balls,
        p.reference_height_cm(),
        p.rmc
    );
}
