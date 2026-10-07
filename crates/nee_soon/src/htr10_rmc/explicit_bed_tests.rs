// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// This file is part of OUTRAM PARK. See the module header of `htr10_rmc` for
// licence terms.

//! Tests of [`super::assemble_explicit_triso_from_centres`]. Every check reads
//! the ASSEMBLED geometry (`Geometry::locate`, `Cell::contains`), not the bed
//! description it was built from.

use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::{Direction, Position};
use uom::si::f64::Length;
use uom::si::length::{centimeter, meter};

use super::*;
use crate::htr10_rmc::core_model::{assemble_explicit_triso, tile_cell_role, TileCellRole};
use crate::htr10_rmc::tests::{built_ball_pieces, built_balls};

/// A small deterministic generator (SplitMix64) for sample points.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }
}

const DIR: Direction = Direction {
    u: 0.0,
    v: 0.0,
    w: 1.0,
};

/// The bed-tile level of the located path at `p`: `(lattice index, tile
/// universe, tile-local position, leaf cell index)`.
fn tile_level(g: &Geometry, p: Position) -> Option<([i32; 3], usize, Position, usize)> {
    let path = g.locate(p, DIR, SurfaceToken::NONE)?;
    let lvl = path.levels.iter().find(|c| c.lattice == Some(0))?;
    Some((lvl.lattice_index, lvl.universe, lvl.r, lvl.cell))
}

/// Cells of universe `u` whose region contains the tile-local point `r`.
fn containing_cells(g: &Geometry, u: usize, r: Position) -> Vec<usize> {
    g.universes[u]
        .cell_indices
        .iter()
        .copied()
        .filter(|&ci| g.cells[ci].contains(r, DIR, &g.surfaces, SurfaceToken::NONE))
        .collect()
}

fn m(x: f64) -> Length {
    Length::new::<meter>(x)
}

/// **A Şeker bed rebuilt from its own centres is the same bed.**
///
/// Methodology: build Şeker's bed (`assemble_explicit_triso`, 14 rings,
/// N = 2), take every present ball's centre and identity, move them to the
/// DEM frame (floor at z = 0, metres), and rebuild with
/// `assemble_explicit_triso_from_centres`. Pass: the same `core_balls`, the
/// same set of built balls read from the assembled lattice (centres to 1e-6
/// cm, identity), no overlap planes, no tube filler (every tube ball is
/// already in the list), and the same material at 20 000 random points of the
/// container (bed cylinder, conus and the top 60 cm of the tube) located in
/// both geometries.
///
/// Results (2026-10-05): see the printed line; 0 material mismatches,
/// identical ball sets.
#[test]
fn a_seker_bed_rebuilt_from_its_centres_is_the_same_bed() {
    let seker = assemble_explicit_triso(14, 2, 0);
    let Some(PebbleBed::Seker(sb)) = seker.bed.as_ref() else {
        panic!("the default bed is Şeker's")
    };
    let hh = seker.bed_half_height;
    let mut centres = Vec::new();
    let mut fuel = Vec::new();
    for id in sb.all_balls() {
        if sb.is_present(id) {
            let c = sb.centre(id);
            centres.push([m(c[0] / 100.0), m(c[1] / 100.0), m((c[2] + hh) / 100.0)]);
            fuel.push(sb.is_fuel(id));
        }
    }
    let ex = assemble_explicit_triso_from_centres(&centres, &fuel, 14, 0);
    let Some(PebbleBed::Explicit(eb)) = ex.bed.as_ref() else {
        panic!("an explicit bed")
    };
    assert!(
        (ex.bed_half_height - hh).abs() < 1e-9,
        "{} vs {hh}",
        ex.bed_half_height
    );
    assert_eq!(
        ex.bed.as_ref().unwrap().core_balls(),
        seker.bed.as_ref().unwrap().core_balls()
    );
    assert!(
        eb.overlaps.is_empty(),
        "Şeker's balls touch, they do not overlap"
    );
    assert!(
        eb.source
            .iter()
            .all(|s| matches!(s, ExplicitBallSource::Dem(_))),
        "no filler: the tube balls are in the list"
    );
    let a = built_balls(&built_ball_pieces(&seker));
    let b = built_balls(&built_ball_pieces(&ex));
    assert_eq!(a.len(), b.len(), "built ball count");
    for ((ca, fa), (cb, fb)) in a.iter().zip(&b) {
        for k in 0..3 {
            assert!((ca[k] - cb[k]).abs() < 1e-6, "{ca:?} vs {cb:?}");
        }
        assert!(
            fa.iter().chain(fb).all(|&f| f == fa[0]),
            "identity at {ca:?}"
        );
    }
    // The same material everywhere in the container.
    let mut rng = Rng(7);
    let (mut n, mut bad) = (0, 0);
    let z_lo = ex.conus_floor - 60.0;
    while n < 20_000 {
        let p = Position::new(
            (2.0 * rng.next() - 1.0) * 90.0,
            (2.0 * rng.next() - 1.0) * 90.0,
            z_lo + rng.next() * (hh - z_lo),
        );
        if p.x.hypot(p.y) > 90.0 {
            continue;
        }
        n += 1;
        let ma = seker
            .geometry
            .locate(p, DIR, SurfaceToken::NONE)
            .map(|q| q.material);
        let mb = ex
            .geometry
            .locate(p, DIR, SurfaceToken::NONE)
            .map(|q| q.material);
        bad += usize::from(ma != mb);
    }
    println!(
        "Şeker N=2 rebuilt: {} balls listed, {} built, core balls {:?}, {} mismatched materials in {n} points",
        centres.len(),
        b.len(),
        ex.bed.as_ref().unwrap().core_balls(),
        bad
    );
    assert_eq!(bad, 0, "materials differ at {bad} of {n} points");
}

/// **An overlapping pair is split by its bisector plane, and no point is
/// claimed by two cells.**
///
/// Methodology: three balls on the bed floor near the axis, pairwise 5.95,
/// 5.90 and 5.90 cm apart (overlaps 0.05, 0.10, 0.10 cm, at and above the
/// DEM's ~0.05 cm), one of them fuel, plus one separate fuel ball 12 cm away.
/// Below the floor the conus is empty and the tube holds Şeker's filler.
/// Sample points inside each lens and around the balls; for each, find the
/// bed tile it is located in and count the cells of that tile's universe
/// whose region contains it. Pass: exactly one, always; a lens point on ball
/// i's side of the bisector is in ball i's cell; the lens volume fraction
/// equals the analytic lens volume over the ball volume.
#[test]
fn an_overlapping_pair_is_split_by_its_bisector() {
    let r = PEBBLE_RADIUS_CM;
    // DEM frame, cm then metres.
    let cm = [
        [0.0, 0.0, r],
        [5.95, 0.0, r],
        [
            5.95 * 0.5,
            (5.90_f64.powi(2) - (5.95_f64 * 0.5).powi(2)).sqrt(),
            r,
        ],
        // A separate ball, 14.6 cm from the cluster.
        [-12.0, 8.196, r],
    ];
    let centres: Vec<[Length; 3]> = cm.iter().map(|c| c.map(|v| m(v / 100.0))).collect();
    let fuel = [true, false, false, true];
    let core = assemble_explicit_triso_from_centres(&centres, &fuel, 1, 0);
    let Some(PebbleBed::Explicit(eb)) = core.bed.as_ref() else {
        panic!("an explicit bed")
    };
    let mc = |i: usize| eb.centres[i];
    let pairs: Vec<(usize, usize)> = eb.overlaps.clone();
    assert_eq!(pairs.len(), 3, "three overlapping pairs: {pairs:?}");
    let g = &core.geometry;
    let mut rng = Rng(11);
    let mut checked = 0;
    for &(i, j) in &pairs {
        let (ci, cj) = (mc(i), mc(j));
        let e = [cj[0] - ci[0], cj[1] - ci[1], cj[2] - ci[2]];
        let d = (e[0] * e[0] + e[1] * e[1] + e[2] * e[2]).sqrt();
        let n = e.map(|x| x / d);
        let mut lens_pts = 0;
        while lens_pts < 2000 {
            // A point in the lens's bounding box round the midpoint.
            let p = [0, 1, 2].map(|k| 0.5 * (ci[k] + cj[k]) + (2.0 * rng.next() - 1.0) * 0.8);
            let di =
                ((p[0] - ci[0]).powi(2) + (p[1] - ci[1]).powi(2) + (p[2] - ci[2]).powi(2)).sqrt();
            let dj =
                ((p[0] - cj[0]).powi(2) + (p[1] - cj[1]).powi(2) + (p[2] - cj[2]).powi(2)).sqrt();
            if di >= r || dj >= r {
                continue;
            }
            lens_pts += 1;
            let pos = Position::new(p[0], p[1], p[2]);
            let (li, u, local, _) = tile_level(g, pos).expect("a lens point is in the bed lattice");
            let hits = containing_cells(g, u, local);
            assert_eq!(hits.len(), 1, "lens point {p:?} is in {} cells", hits.len());
            let cell = &g.cells[hits[0]];
            let role = tile_cell_role(cell.id).expect("a tile cell");
            assert!(
                matches!(role, TileCellRole::FuelShell | TileCellRole::DummyBall),
                "a lens point is graphite of one ball, got {role:?}"
            );
            let nr = eb.n_rings as i32;
            let key = (li[0] - nr + 1, li[1] - nr + 1, li[2]);
            let site = (cell.id.rem_euclid(1_000_000) % TILE_SITE_STRIDE) as usize;
            let owner = eb.tile_balls[&key][site];
            let side = (p[0] - 0.5 * (ci[0] + cj[0])) * n[0]
                + (p[1] - 0.5 * (ci[1] + cj[1])) * n[1]
                + (p[2] - 0.5 * (ci[2] + cj[2])) * n[2];
            let expect = if side < 0.0 { i } else { j };
            assert_eq!(
                owner, expect,
                "lens point {p:?} (side {side:.3e}) given to ball {owner}"
            );
            checked += 1;
        }
    }
    // Everywhere round the balls (and the straddling ball): one cell.
    let mut around = 0;
    while around < 20_000 {
        let k = (rng.next() * cm.len() as f64) as usize % cm.len();
        let c = mc(k);
        let p = Position::new(
            c[0] + (2.0 * rng.next() - 1.0) * 4.0,
            c[1] + (2.0 * rng.next() - 1.0) * 4.0,
            c[2] + (2.0 * rng.next() - 1.0) * 3.0,
        );
        if p.z < eb.bed_bottom || p.z > eb.bed_top || p.x.hypot(p.y) > 90.0 {
            continue;
        }
        let Some((_, u, local, _)) = tile_level(g, p) else {
            continue;
        };
        let hits = containing_cells(g, u, local);
        // A fuel zone's TRISO fill cell and nothing else, or one material cell.
        assert_eq!(hits.len(), 1, "point {p:?} is in {} cells", hits.len());
        around += 1;
    }
    let v_ball = 4.0 / 3.0 * std::f64::consts::PI * r.powi(3);
    let lens: f64 = [5.95, 5.90, 5.90].iter().map(|&d| lens_volume(r, d)).sum();
    let want = lens / (v_ball * eb.centres.len() as f64);
    println!(
        "{checked} lens points, {around} points round the balls; max overlap {:.4} cm, \
         lens fraction {:.3e} (analytic {want:.3e}); {} filler balls",
        eb.max_overlap,
        eb.lens_volume_fraction,
        eb.source
            .iter()
            .filter(|s| matches!(s, ExplicitBallSource::TubeFiller))
            .count()
    );
    assert!((eb.max_overlap - 0.10).abs() < 1e-9);
    assert!((eb.lens_volume_fraction - want).abs() < 1e-12 * want.max(1.0));
}

/// The lens volume formula against its closed-form ends: zero at contact,
/// the whole sphere at coincidence.
#[test]
fn the_lens_volume_has_the_right_limits() {
    let r = 3.0;
    assert!(lens_volume(r, 2.0 * r).abs() < 1e-12);
    let v = 4.0 / 3.0 * std::f64::consts::PI * r.powi(3);
    assert!((lens_volume(r, 0.0) - v).abs() < 1e-9 * v);
}

/// The 57:43 split goes to balls centred above the floor only.
#[test]
fn the_paper_assignment_leaves_the_conus_dummy() {
    let centres: Vec<[Length; 3]> = (0..1000)
        .map(|i| {
            let z = -0.3 + 0.002 * f64::from(i);
            [m(0.0), m(0.0), m(z)]
        })
        .collect();
    let f = paper_fuel_assignment(&centres);
    let above: Vec<bool> = centres
        .iter()
        .zip(&f)
        .filter(|(c, _)| c[2].get::<centimeter>() > 0.0)
        .map(|(_, &x)| x)
        .collect();
    assert!(centres
        .iter()
        .zip(&f)
        .all(|(c, &x)| c[2].get::<centimeter>() > 0.0 || !x));
    let frac = above.iter().filter(|&&x| x).count() as f64 / above.len() as f64;
    assert!((frac - 0.57).abs() < 1.0 / above.len() as f64, "{frac}");
}

/// A centre outside the container is a frame error, not a ball to clip.
#[test]
#[should_panic(expected = "outside the HTR-10 container")]
fn a_centre_outside_the_container_is_refused() {
    let centres = [[m(0.95), m(0.0), m(0.5)]];
    let _ = assemble_explicit_triso_from_centres(&centres, &[false], 1, 0);
}

/// Two balls closer than 5 cm would have a fuel zone cut by the bisector.
#[test]
#[should_panic(expected = "Refusing the bed")]
fn a_fuel_zone_cutting_overlap_is_refused() {
    let centres = [[m(0.0), m(0.0), m(0.03)], [m(0.049), m(0.0), m(0.03)]];
    let _ = assemble_explicit_triso_from_centres(&centres, &[true, true], 1, 0);
}

/// **A real LIGGGHTS-port bed builds, and no point of it is claimed by two
/// cells.**
///
/// Methodology: `reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv`
/// (27 554 pebbles, metres, DEM frame), trimmed to every pebble centred at or
/// below the floor plus the lowest 16 890 above it, identity by
/// [`paper_fuel_assignment`]. Pass: 16 890 core balls, fuel fraction 0.57 to
/// one ball, at least one overlap split, max overlap below 2 % of r, and
/// 20 000 random points of the bed cylinder all located with OpenMC's
/// `check_cell_overlap` false (no cell of any universe on the path shares the
/// point).
///
/// Results (2026-10-05): 19 347 DEM balls + 1 721 tube filler; 52 356 pairs
/// split, max overlap 0.04723 cm (1.574 % of r), lens volume fraction
/// 1.27e-5; max wall penetration 0.0317 cm; 0 ambiguous points.
#[test]
fn a_liggghts_bed_builds_with_one_cell_per_point() {
    use outram_mc_libs::geometry::plot::slice::check_cell_overlap;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv");
    let text = std::fs::read_to_string(&path).expect("the committed DEM bed");
    let all: Vec<[f64; 3]> = text
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<f64> = l
                .split(',')
                .skip(1)
                .take(3)
                .map(|v| v.trim().parse().unwrap())
                .collect();
            [f[0], f[1], f[2]]
        })
        .collect();
    let mut keep: Vec<[f64; 3]> = all.iter().copied().filter(|c| c[2] <= 0.0).collect();
    let mut above: Vec<[f64; 3]> = all.iter().copied().filter(|c| c[2] > 0.0).collect();
    above.sort_by(|p, q| p[2].total_cmp(&q[2]));
    above.truncate(16_890);
    keep.extend(above);
    let centres: Vec<[Length; 3]> = keep.iter().map(|c| c.map(m)).collect();
    let fuel = paper_fuel_assignment(&centres);
    let core = assemble_explicit_triso_from_centres(&centres, &fuel, 14, 0);
    let Some(PebbleBed::Explicit(eb)) = core.bed.as_ref() else {
        panic!("an explicit bed")
    };
    assert_eq!(eb.core_balls(), 16_890);
    let (e, f) = eb.eligible_and_fuel_balls();
    assert!((f as f64 - 0.57 * e as f64).abs() <= 1.0, "{f} of {e}");
    assert!(!eb.overlaps.is_empty());
    assert!(eb.max_overlap < 0.02 * eb.radius, "{}", eb.max_overlap);
    let g = &core.geometry;
    let mut rng = Rng(3);
    let (mut n, mut ambiguous, mut lost) = (0, 0, 0);
    while n < 20_000 {
        let p = Position::new(
            (2.0 * rng.next() - 1.0) * 90.0,
            (2.0 * rng.next() - 1.0) * 90.0,
            eb.bed_bottom + rng.next() * (eb.bed_top - eb.bed_bottom),
        );
        if p.x.hypot(p.y) >= 90.0 {
            continue;
        }
        n += 1;
        match g.locate(p, DIR, SurfaceToken::NONE) {
            None => lost += 1,
            Some(path) => ambiguous += usize::from(check_cell_overlap(g, &path)),
        }
    }
    println!(
        "DEM bed: {} balls, {} pairs, max overlap {:.5} cm, lens {:.3e}; {n} points, {lost} lost, {ambiguous} ambiguous",
        eb.centres.len(),
        eb.overlaps.len(),
        eb.max_overlap,
        eb.lens_volume_fraction
    );
    assert_eq!((lost, ambiguous), (0, 0));
}

#[test]
fn a_dem_csv_is_read_and_cut_to_a_ball_count() {
    let text = "id,x,y,z,vx,vy,vz\n\
                3,0.0,0.0,0.30,0,0,0\n\
                1,0.1,0.0,-0.20,0,0,0\n\
                \n\
                2,0.2,0.0,0.10,0,0,0\n\
                4,-0.1,0.0,0.10,0,0,0\n\
                5,0.0,0.3,0.0,0,0,0\n";
    let c = read_dem_centres_csv(text).expect("a well-formed CSV");
    assert_eq!(c.len(), 5);
    assert!((c[1][2].get::<meter>() + 0.20).abs() < 1e-15);
    // The floor (z <= 0) is kept whatever the count, in input order; above
    // it the lowest two, ties in z broken by x.
    let cut = trim_to_core_balls(&c, 2).expect("enough balls");
    let z: Vec<[f64; 3]> = cut.iter().map(|p| p.map(|v| v.get::<meter>())).collect();
    assert_eq!(
        z,
        vec![
            [0.1, 0.0, -0.20],
            [0.0, 0.3, 0.0],
            [-0.1, 0.0, 0.10],
            [0.2, 0.0, 0.10]
        ]
    );
    assert_eq!(
        trim_to_core_balls(&c, 4),
        Err(DemBedError::TooFewBalls {
            available: 3,
            wanted: 4
        })
    );
    assert!(matches!(
        read_dem_centres_csv("x,y,z\n1,2,3\n"),
        Err(DemBedError::Header(_))
    ));
    assert!(matches!(
        read_dem_centres_csv("id,x,y,z\n1,2,oops,3\n"),
        Err(DemBedError::Row { line: 2, .. })
    ));
    assert!(matches!(
        read_dem_centres_csv("id,x,y,z\n1,2,3\n"),
        Err(DemBedError::Row { line: 2, .. })
    ));
}

/// gh:#787: the committed gh:#216 bed cut to the lattice's ball count at
/// N = 12 builds a core with that many balls above the floor, and the cut is
/// the one the web beds view draws. That view cuts the bed after quantising
/// every coordinate to u16 over the bed's extent, which cannot change WHICH
/// balls are kept as long as no centre sits within one step of the floor and
/// the last kept and first dropped heights are more than one step apart.
/// Prints the conus and tube ball counts of both beds (the record's list of
/// differences besides the packing).
#[test]
fn the_dem_bed_cut_to_the_lattice_ball_count_is_the_beds_view_cut() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv");
    let text = std::fs::read_to_string(&path).expect("the committed DEM bed");
    let all = read_dem_centres_csv(&text).expect("a well-formed bed");
    assert_eq!(all.len(), 27_554);

    let lattice = assemble_explicit_triso(14, 12, 0);
    let lb = lattice.bed.as_ref().expect("a ball list");
    let n_core = lb.core_balls().expect("Şeker's bed counts its balls");
    assert_eq!(n_core, 16_681, "the lattice's ball count at N = 12");

    let cut = trim_to_core_balls(&all, n_core).expect("enough balls");
    let fuel = paper_fuel_assignment(&cut);
    let core = assemble_explicit_triso_from_centres(&cut, &fuel, 14, 0);
    let Some(PebbleBed::Explicit(eb)) = core.bed.as_ref() else {
        panic!("an explicit bed")
    };
    assert_eq!(eb.core_balls(), n_core);
    let (e, f) = eb.eligible_and_fuel_balls();

    // Robust to the view's quantisation.
    let z: Vec<f64> = all.iter().map(|c| c[2].get::<meter>()).collect();
    let (lo, hi) = z
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| {
            (a.min(v), b.max(v))
        });
    let step = (hi - lo) / 65_535.0;
    let mut above: Vec<f64> = z.iter().copied().filter(|&v| v > 0.0).collect();
    above.sort_by(f64::total_cmp);
    let gap = above[n_core] - above[n_core - 1];
    let near_floor = z.iter().map(|v| v.abs()).fold(f64::INFINITY, f64::min);
    assert!(
        gap > step,
        "cut gap {gap:.3e} m within one u16 step {step:.3e} m"
    );
    assert!(
        near_floor > step,
        "a centre {near_floor:.3e} m from the floor"
    );

    // Conus (floor to conus floor) and tube balls of both beds.
    let (l_floor, l_conus) = (lb.bed_bottom(), lattice.conus_floor);
    let (mut l_cone, mut l_tube) = (0, 0);
    for b in lb.all_balls().into_iter().filter(|b| lb.is_present(*b)) {
        let zc = lb.centre(b)[2];
        if zc <= l_floor && zc > l_conus {
            l_cone += 1;
        } else if zc <= l_conus {
            l_tube += 1;
        }
    }
    let (mut d_cone, mut d_tube_dem, mut d_filler) = (0, 0, 0);
    for (c, s) in eb.centres.iter().zip(&eb.source) {
        match s {
            ExplicitBallSource::TubeFiller => d_filler += 1,
            ExplicitBallSource::Dem(_) if c[2] <= eb.bed_bottom && c[2] > eb.conus_floor => {
                d_cone += 1;
            }
            ExplicitBallSource::Dem(_) if c[2] <= eb.conus_floor => d_tube_dem += 1,
            ExplicitBallSource::Dem(_) => {}
        }
    }
    println!(
        "lattice N = 12: {n_core} core balls, bed {:.3} cm, conus {l_cone}, tube {l_tube}",
        2.0 * lattice.bed_half_height
    );
    println!(
        "DEM cut: {} core balls (fuel {f} of {e}), bed {:.3} cm (p99 surface {:.3} cm), \
         conus {d_cone}, tube {d_tube_dem} DEM + {d_filler} filler (gap {:.3} cm), \
         {} lens pairs (fraction {:.3e}), max wall penetration {:.4} cm; \
         u16 step {step:.3e} m, cut gap {gap:.3e} m, nearest to floor {near_floor:.3e} m",
        eb.core_balls(),
        2.0 * core.bed_half_height,
        eb.surface_height,
        eb.tube_gap,
        eb.overlaps.len(),
        eb.lens_volume_fraction,
        eb.max_wall_penetration
    );
}
