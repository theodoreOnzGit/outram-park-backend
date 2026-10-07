//! **The two HTR-10 pebble beds of rung 5's liberties toggle**, baked for the
//! web (gh:#787). Shared by `monte_carlo_web` (the `htr10` rung's `beds` view)
//! and `dem_web` (which also writes the bake, `--bake-beds`).
//!
//! # The two beds
//!
//! - **lattice**: the ball list of `nee_soon`'s
//!   `assemble_explicit_triso(14, N, 0)` at N = [`LATTICE_LAYERS`], i.e. the
//!   `PebbleBed` the geometry every recorded HTR-10 k run used is built from
//!   (Şeker & Çolak 2003's 13-ball cell, every ball whole). Read from the
//!   ASSEMBLED core's `bed`, not from constants.
//! - **random (DEM)**: the 27 554-pebble bed the LIGGGHTS port's
//!   `GranularSystem` settled into the published conus in the gh:#216
//!   friction study at µ = 0.1, µ_r = 0 (E = 5e8 Pa, ν = 0.2, e = 0.5,
//!   dt = 35 µs; `outram-park-fork-liggghts` V&V § 4.9):
//!   `reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv`.
//!   Re-run on 2026-10-07 for this bake (gh:#787) and found byte-identical;
//!   provenance, cost and the filling fraction are in
//!   `crates/dhoby-ghaut/verification_and_validation/htr10_dem_bed_bake/README.md`.
//!   ~~**No k_eff of this bed is shown anywhere it is drawn.**~~ **Since
//!   2026-10-08 (gh:#787)** the beds view shows its one recorded native k,
//!   cut to the lattice's 16 681 balls
//!   (`nee_soon/verification_and_validation/htr10_dem_bed_keff_2026_10_08/`).
//!
//! Both are in the DEM frame: metres, `z = 0` at the conus inlet (the bed
//! floor), the conus below it. Lattice balls centred below [`CLIP_Z_M`] (the
//! DEM run's valve; Şeker's tube runs on 2.2 m further) are not kept.
//!
//! # The wire format (`htr10_beds.zz`)
//!
//! zlib (`miniz_oxide`, level 9: the codec the demo's ENDF tapes use) of:
//! the 8-byte magic [`MAGIC`], `u32` lattice layers, `u32` bed count, then per
//! bed `u32 n`, `f32 lo[3]`, `f32 hi[3]` and the centres as `u16` per axis
//! (all x, then all y, then all z), each `lo + q (hi − lo) / 65535`. Pebbles
//! are sorted by height. The quantisation step is under 0.05 mm, against a
//! 30 mm radius and a soft-sphere overlap of up to 0.5 mm.

// Each demo uses part of this module.
#![allow(dead_code)]

use outram_park_fork_liggghts::htr10_fill::{
    surface_height_m, whole_core_fraction, PEBBLE_RADIUS_M, VALVE_Z_M,
};
use outram_park_fork_liggghts::particle::Vec3;

/// The baked beds, as committed.
pub const BAKED: &[u8] = include_bytes!("htr10_beds.zz");

/// First bytes of the inflated file.
pub const MAGIC: &[u8; 8] = b"HTRBED01";

/// Şeker layers of the lattice bed: the zoom ladder's N, Şeker's critical
/// row (123.576 cm).
pub const LATTICE_LAYERS: usize = 12;

/// Lowest centre kept \[m\]: the DEM run's valve.
pub const CLIP_Z_M: f64 = VALVE_Z_M;

/// Pebble radius \[m\] (both beds: the 6 cm HTR-10 ball).
pub const RADIUS_M: f64 = PEBBLE_RADIUS_M;

/// One bed: pebble centres \[m\], DEM frame, sorted by height.
#[derive(Debug, Clone, PartialEq)]
pub struct Bed {
    pub centres: Vec<[f32; 3]>,
}

/// Both beds of the toggle.
#[derive(Debug, Clone, PartialEq)]
pub struct Beds {
    pub lattice_layers: u32,
    pub lattice: Bed,
    pub dem: Bed,
}

/// What the toggle reports for a bed, by the gh:#216 instruments.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BedStats {
    /// Pebbles drawn (core, conus and the kept tube).
    pub total: usize,
    /// Pebbles centred above the bed floor (the count the reference
    /// tabulates and the beds are matched by).
    pub core: usize,
    /// Robust surface: 99th-percentile core centre height + r \[m\].
    pub surface_m: f64,
    /// Whole-core filling fraction `N V / (π R² h)` (gh:#216's like-for-like
    /// measure against the published 0.61).
    pub phi_whole_core: f64,
}

/// One pebble cut by a slice plane: in-plane centre \[m\] and the radius of
/// the circle the plane cuts \[m\].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cut {
    pub u: f32,
    pub v: f32,
    pub r: f32,
}

fn sort_by_height(c: &mut [[f64; 3]]) {
    c.sort_by(|a, b| {
        a[2].total_cmp(&b[2])
            .then(a[0].total_cmp(&b[0]))
            .then(a[1].total_cmp(&b[1]))
    });
}

/// Encode beds (centres in metres) into the wire format, zlib-compressed.
/// Each bed is sorted by height first.
pub fn encode(lattice_layers: u32, beds: &[Vec<[f64; 3]>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&lattice_layers.to_le_bytes());
    out.extend_from_slice(&(beds.len() as u32).to_le_bytes());
    for bed in beds {
        let mut c = bed.clone();
        sort_by_height(&mut c);
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for p in &c {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k] as f32);
                hi[k] = hi[k].max(p[k] as f32);
            }
        }
        out.extend_from_slice(&(c.len() as u32).to_le_bytes());
        for v in lo.iter().chain(hi.iter()) {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for k in 0..3 {
            let span = f64::from(hi[k]) - f64::from(lo[k]);
            for p in &c {
                let q = if span > 0.0 {
                    ((p[k] - f64::from(lo[k])) / span * 65535.0)
                        .round()
                        .clamp(0.0, 65535.0)
                } else {
                    0.0
                };
                out.extend_from_slice(&(q as u16).to_le_bytes());
            }
        }
    }
    miniz_oxide::deflate::compress_to_vec_zlib(&out, 9)
}

/// Decode the wire format. Needs at least two beds (lattice, then DEM).
pub fn decode(zz: &[u8]) -> Result<Beds, String> {
    let raw =
        miniz_oxide::inflate::decompress_to_vec_zlib(zz).map_err(|e| format!("inflate: {e:?}"))?;
    let mut at = 0usize;
    let mut take = |n: usize| -> Result<&[u8], String> {
        let s = raw
            .get(at..at + n)
            .ok_or_else(|| format!("bed file truncated at byte {at}"))?;
        at += n;
        Ok(s)
    };
    if take(8)? != MAGIC {
        return Err("not an HTR-10 bed file (bad magic)".into());
    }
    let u32_of = |s: &[u8]| u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
    let layers = u32_of(take(4)?);
    let n_beds = u32_of(take(4)?) as usize;
    if n_beds < 2 {
        return Err(format!("bed file holds {n_beds} beds, need 2"));
    }
    let mut beds = Vec::with_capacity(n_beds);
    for _ in 0..n_beds {
        let n = u32_of(take(4)?) as usize;
        let mut box_ = [0f32; 6];
        for b in &mut box_ {
            let s = take(4)?;
            *b = f32::from_le_bytes([s[0], s[1], s[2], s[3]]);
        }
        let mut centres = vec![[0f32; 3]; n];
        for k in 0..3 {
            let (lo, hi) = (f64::from(box_[k]), f64::from(box_[k + 3]));
            let s = take(2 * n)?;
            for (i, c) in centres.iter_mut().enumerate() {
                let q = u16::from_le_bytes([s[2 * i], s[2 * i + 1]]);
                c[k] = (lo + f64::from(q) * (hi - lo) / 65535.0) as f32;
            }
        }
        beds.push(Bed { centres });
    }
    let mut it = beds.into_iter();
    let lattice = it.next().ok_or("no lattice bed")?;
    let dem = it.next().ok_or("no DEM bed")?;
    Ok(Beds {
        lattice_layers: layers,
        lattice,
        dem,
    })
}

impl Bed {
    fn vec3s(&self) -> Vec<Vec3> {
        self.centres
            .iter()
            .map(|c| Vec3::new(f64::from(c[0]), f64::from(c[1]), f64::from(c[2])))
            .collect()
    }

    /// Counts, surface and whole-core φ, with `htr10_fill`'s instruments (the
    /// gh:#216 ones).
    pub fn stats(&self) -> BedStats {
        let v = self.vec3s();
        BedStats {
            total: v.len(),
            core: v.iter().filter(|c| c.z > 0.0).count(),
            surface_m: surface_height_m(&v).unwrap_or(0.0),
            phi_whole_core: whole_core_fraction(&v),
        }
    }

    /// The bed with only its lowest `core` pebbles above the floor kept (and
    /// everything in the conus and tube): how a bed of a given ball count is
    /// cut from a poured one, as `nee_soon`'s DEM-bed builder does.
    pub fn trimmed_to_core(&self, core: usize) -> Bed {
        let mut kept = 0usize;
        let centres = self
            .centres
            .iter()
            .filter(|c| {
                if c[2] <= 0.0 {
                    return true;
                }
                kept += 1;
                kept <= core
            })
            .copied()
            .collect();
        Bed { centres }
    }

    /// Pebbles cut by the vertical plane `y = y0` \[m\]: `u = x`, `v = z`.
    pub fn side_cut(&self, y0: f32) -> Vec<Cut> {
        cut(&self.centres, 1, y0, 0, 2)
    }

    /// Pebbles cut by the horizontal plane `z = z0` \[m\]: `u = x`, `v = y`.
    pub fn plan_cut(&self, z0: f32) -> Vec<Cut> {
        cut(&self.centres, 2, z0, 0, 1)
    }
}

fn cut(centres: &[[f32; 3]], axis: usize, at: f32, u: usize, v: usize) -> Vec<Cut> {
    let r = RADIUS_M as f32;
    centres
        .iter()
        .filter_map(|c| {
            let d = c[axis] - at;
            (d.abs() < r).then(|| Cut {
                u: c[u],
                v: c[v],
                r: (r * r - d * d).sqrt(),
            })
        })
        .collect()
}

/// The vessel's inner wall radius \[m\] at height `z` (the DEM run's: barrel,
/// conus, discharge tube; `htr10_fill`).
pub fn wall_radius_at(z: f64) -> f64 {
    outram_park_fork_liggghts::htr10_fill::wall_radius_at(z)
}

/// The pebbles a cut-away shows: centres within `depth` \[m\] behind the
/// plane `y = 0` (viewer on `+y`), farthest first, as `(x, z, y)`.
pub fn cut_away(centres: &[[f32; 3]], depth: f32) -> Vec<[f32; 3]> {
    let mut kept: Vec<[f32; 3]> = centres
        .iter()
        .filter(|c| c[1] <= 0.0 && c[1] >= -depth)
        .map(|c| [c[0], c[2], c[1]])
        .collect();
    kept.sort_by(|a, b| a[2].total_cmp(&b[2]));
    kept
}

/// Drawing, in a `View` whose world unit is the **centimetre** (x across,
/// z up). GUI drawing code: exempt from the reaching-test rule (what it
/// draws is computed by the tested functions above: [`Bed::side_cut`],
/// [`Bed::plan_cut`], [`cut_away`], [`wall_radius_at`]).
pub mod draw {
    use super::{wall_radius_at, Cut, RADIUS_M};
    use dhoby_ghaut::web_demo::view::View;
    use egui::{Color32, Painter, Pos2, Rect, Stroke};
    use outram_park_fork_liggghts::htr10_fill::{
        CONE_HEIGHT_M, CORE_RADIUS_M, TUBE_RADIUS_M, VALVE_Z_M,
    };

    pub const WALL: Color32 = Color32::from_rgb(150, 156, 170);
    pub const PEBBLE: Color32 = Color32::from_rgb(120, 128, 140);

    /// The vessel's inner wall in a side cut (barrel up to `top_m`, conus,
    /// tube to the valve), shifted `dx_cm` across. GUI drawing (exempt).
    pub fn vessel_side(p: &Painter, rect: Rect, view: &View, dx_cm: f64, top_m: f64) {
        let pts = [
            (CORE_RADIUS_M, top_m),
            (CORE_RADIUS_M, 0.0),
            (TUBE_RADIUS_M, -CONE_HEIGHT_M),
            (TUBE_RADIUS_M, VALVE_Z_M),
        ];
        for side in [-1.0, 1.0] {
            let line: Vec<Pos2> = pts
                .iter()
                .map(|&(r, z)| view.to_screen(rect, dx_cm + side * 100.0 * r, 100.0 * z))
                .collect();
            p.add(egui::Shape::line(line, Stroke::new(1.5, WALL)));
        }
        let a = view.to_screen(rect, dx_cm - 100.0 * TUBE_RADIUS_M, 100.0 * VALVE_Z_M);
        let b = view.to_screen(rect, dx_cm + 100.0 * TUBE_RADIUS_M, 100.0 * VALVE_Z_M);
        p.line_segment([a, b], Stroke::new(1.5, Color32::from_rgb(200, 120, 90)));
    }

    /// The wall in a plan cut at height `z_m`. GUI drawing (exempt).
    pub fn vessel_plan(p: &Painter, rect: Rect, view: &View, dx_cm: f64, z_m: f64) {
        let c = view.to_screen(rect, dx_cm, 0.0);
        p.circle_stroke(
            c,
            (100.0 * wall_radius_at(z_m) * view.scale) as f32,
            Stroke::new(1.5, WALL),
        );
    }

    /// Cut circles, shifted `dx_cm` across. GUI drawing (exempt).
    pub fn cuts(p: &Painter, rect: Rect, view: &View, dx_cm: f64, cuts: &[Cut], fill: Color32) {
        for c in cuts {
            let s = view.to_screen(rect, dx_cm + 100.0 * f64::from(c.u), 100.0 * f64::from(c.v));
            let r = (100.0 * f64::from(c.r) * view.scale) as f32;
            if rect.expand(r).contains(s) {
                p.circle(
                    s,
                    r.max(0.6),
                    fill,
                    Stroke::new(0.5, Color32::from_rgb(30, 32, 38)),
                );
            }
        }
    }

    /// A cut-away ([`super::cut_away`] output), shaded darker with depth.
    /// GUI drawing (exempt).
    pub fn cut_away(
        p: &Painter,
        rect: Rect,
        view: &View,
        kept: &[[f32; 3]],
        depth: f32,
        base: Color32,
    ) {
        let r = (100.0 * RADIUS_M * view.scale) as f32;
        for c in kept {
            let s = view.to_screen(rect, 100.0 * f64::from(c[0]), 100.0 * f64::from(c[1]));
            if !rect.expand(r).contains(s) {
                continue;
            }
            let k = 1.0 - 0.6 * (-c[2] / depth.max(1e-6)).clamp(0.0, 1.0);
            let f = Color32::from_rgb(
                (f32::from(base.r()) * k) as u8,
                (f32::from(base.g()) * k) as u8,
                (f32::from(base.b()) * k) as u8,
            );
            p.circle(
                s,
                r.max(0.6),
                f,
                Stroke::new(0.5, Color32::from_rgb(25, 27, 32)),
            );
        }
    }
}

/// The bake (native only): the lattice from the assembled core, the DEM bed
/// from its CSV. Returns the compressed bytes and a report.
#[cfg(not(target_arch = "wasm32"))]
pub mod bake {
    use super::*;

    /// The committed #216 bed, relative to the workspace root.
    pub const DEM_CSV: &str = "reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv";

    /// The lattice bed's centres \[m\], DEM frame, from the assembled core.
    pub fn lattice_centres(layers: usize) -> Result<Vec<[f64; 3]>, String> {
        let core = nee_soon::htr10_rmc::core_model::assemble_explicit_triso(14, layers, 0);
        let bed = core.bed.ok_or("the assembled core carries no ball list")?;
        let floor = bed.bed_bottom();
        Ok(bed
            .all_balls()
            .into_iter()
            .filter(|b| bed.is_present(*b))
            .map(|b| {
                let c = bed.centre(b);
                [c[0] / 100.0, c[1] / 100.0, (c[2] - floor) / 100.0]
            })
            .filter(|c| c[2] >= CLIP_Z_M)
            .collect())
    }

    /// Centres \[m\] from an `id,x,y,z,vx,vy,vz` CSV (the
    /// `reference-data/liggghts/` format), in id order.
    pub fn read_csv(text: &str) -> Result<Vec<[f64; 3]>, String> {
        let mut rows: Vec<(u64, [f64; 3])> = Vec::new();
        for (i, line) in text.lines().enumerate().skip(1) {
            if line.trim().is_empty() {
                continue;
            }
            let f: Vec<f64> = line
                .split(',')
                .map(|v| v.trim().parse::<f64>())
                .collect::<Result<_, _>>()
                .map_err(|e| format!("line {}: {e}", i + 1))?;
            if f.len() < 4 {
                return Err(format!("line {}: {} fields, need 4", i + 1, f.len()));
            }
            rows.push((f[0] as u64, [f[1], f[2], f[3]]));
        }
        rows.sort_by_key(|r| r.0);
        Ok(rows.into_iter().map(|r| r.1).collect())
    }

    /// Whole-core φ of full-precision centres (the gh:#216 instrument).
    pub fn phi(c: &[[f64; 3]]) -> f64 {
        let v: Vec<Vec3> = c.iter().map(|p| Vec3::new(p[0], p[1], p[2])).collect();
        whole_core_fraction(&v)
    }

    /// Bake from the DEM CSV's text. The report is one line per fact.
    pub fn bake(dem_csv: &str) -> Result<(Vec<u8>, Vec<String>), String> {
        let lattice = lattice_centres(LATTICE_LAYERS)?;
        let dem = read_csv(dem_csv)?;
        let zz = encode(LATTICE_LAYERS as u32, &[lattice.clone(), dem.clone()]);
        let beds = decode(&zz)?;
        let (ls, ds) = (beds.lattice.stats(), beds.dem.stats());
        let trimmed = beds.dem.trimmed_to_core(ls.core).stats();
        let report = vec![
            format!("lattice: assemble_explicit_triso(14, {LATTICE_LAYERS}, 0), {} balls kept ({} in the core), surface {:.4} m, whole-core phi {:.4}", ls.total, ls.core, ls.surface_m, ls.phi_whole_core),
            format!("dem: {DEM_CSV}, {} pebbles ({} in the core), surface {:.4} m", ds.total, ds.core, ds.surface_m),
            format!("dem whole-core phi: {:.6} full precision, {:.6} decoded", phi(&dem), ds.phi_whole_core),
            format!("dem trimmed to the lattice's {} core balls: surface {:.4} m, whole-core phi {:.4}", ls.core, trimmed.surface_m, trimmed.phi_whole_core),
            format!("bytes: {} compressed", zz.len()),
        ];
        Ok((zz, report))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding_round_trips_within_the_quantisation_step() {
        let a = vec![[0.1, -0.2, 0.5], [-0.89, 0.3, -0.6], [0.0, 0.0, 1.9]];
        let b = vec![[0.5, 0.5, 0.06], [0.2, -0.1, 0.03]];
        let beds = decode(&encode(7, &[a.clone(), b])).expect("decode");
        assert_eq!(beds.lattice_layers, 7);
        assert_eq!(beds.lattice.centres.len(), 3);
        // Sorted by height: the -0.6 ball comes first.
        let c = beds.lattice.centres[0];
        for k in 0..3 {
            assert!(
                (f64::from(c[k]) - a[1][k]).abs() < 3e-3 / 65535.0 * 1000.0,
                "axis {k}: {c:?}"
            );
        }
        assert!(decode(b"junk").is_err());
        assert!(decode(&miniz_oxide::deflate::compress_to_vec_zlib(b"HTRBED01", 9)).is_err());
    }

    #[test]
    fn a_cut_shows_the_chord_circle() {
        let bed = Bed {
            centres: vec![[0.0, 0.0, 0.1], [0.2, 0.02, 0.1], [0.4, 0.05, 0.1]],
        };
        let cuts = bed.side_cut(0.0);
        assert_eq!(cuts.len(), 2, "the pebble 5 cm off the plane is not cut");
        assert!((cuts[0].r - 0.03).abs() < 1e-6);
        assert!((cuts[1].r - (0.03f32 * 0.03 - 0.02 * 0.02).sqrt()).abs() < 1e-6);
        assert_eq!(bed.plan_cut(0.1).len(), 3);
        assert_eq!(bed.plan_cut(0.2).len(), 0);
        let away = cut_away(
            &[
                [0.0, -0.05, 1.0],
                [0.1, 0.01, 2.0],
                [0.2, -0.01, 3.0],
                [0.3, -0.2, 4.0],
            ],
            0.06,
        );
        assert_eq!(
            away,
            vec![[0.0, 1.0, -0.05], [0.2, 3.0, -0.01]],
            "behind the plane, within the depth, farthest first"
        );
    }

    #[test]
    fn trimming_keeps_the_conus_and_the_lowest_core_balls() {
        let bed = Bed {
            centres: vec![
                [0.0, 0.0, -0.3],
                [0.0, 0.0, -0.1],
                [0.0, 0.0, 0.1],
                [0.0, 0.0, 0.2],
                [0.0, 0.0, 0.3],
            ],
        };
        let t = bed.trimmed_to_core(2);
        assert_eq!(t.centres.len(), 4);
        assert_eq!(t.centres.last().map(|c| c[2]), Some(0.2));
    }

    /// The committed bake is what its sources give now (native: needs the
    /// committed CSV and builds the lattice core, ~1 s), and its DEM bed keeps
    /// the gh:#216 whole-core φ: **0.6047** against the published 0.61.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn the_baked_beds_regenerate_from_their_sources() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv"
        );
        let csv = std::fs::read_to_string(path).expect("the committed #216 bed");
        let (zz, report) = bake::bake(&csv).expect("bake");
        assert!(
            zz == BAKED,
            "htr10_beds.zz is stale; re-run `dem_web -- --bake-beds`:\n{}",
            report.join("\n")
        );
        let beds = decode(BAKED).expect("decode");
        let s = beds.dem.stats();
        assert_eq!(s.total, 27_554);
        assert!(
            (s.phi_whole_core - 0.6047).abs() < 1e-4,
            "dem phi {}",
            s.phi_whole_core
        );
        let full = bake::phi(&bake::read_csv(&csv).expect("csv"));
        assert!(
            (s.phi_whole_core - full).abs() < 1e-4,
            "quantisation moved phi: {full} -> {}",
            s.phi_whole_core
        );
        assert_eq!(beds.lattice_layers as usize, LATTICE_LAYERS);
        // Every lattice ball inside the DEM vessel's wall (the two vessels
        // are the same published one).
        for c in &beds.lattice.centres {
            let r = (f64::from(c[0]).powi(2) + f64::from(c[1]).powi(2)).sqrt();
            assert!(
                r + RADIUS_M <= wall_radius_at(f64::from(c[2])) + 0.002,
                "lattice ball outside the DEM wall at {c:?}"
            );
        }
    }
}
