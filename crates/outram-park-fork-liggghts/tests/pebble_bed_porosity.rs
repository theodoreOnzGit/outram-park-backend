// ---------------------------------------------------------------------------
// Part of OUTRAM PARK's pure-Rust LIGGGHTS fork.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with this program. If not, see <https://www.gnu.org/licenses/>.
// ---------------------------------------------------------------------------

//! # Radial and axial porosity map of a settled pebble bed — V&V and regression
//!
//! ## What this measures, and why it is not the bulk packing fraction
//!
//! [`pebble_bed_bulk`](../pebble_bed_bulk/index.html) checks one number: the
//! bulk solid fraction of a settled bed, against LIGGGHTS. That number is an
//! average, and an average hides the structure that actually matters for a
//! pebble-bed reactor.
//!
//! A packed bed of equal spheres is **not** homogeneous near a wall. The wall
//! is a plane no sphere centre can cross, so the sphere centres order into
//! layers parallel to it, and the local void fraction **oscillates**:
//!
//! - `eps = 1` exactly at the wall — no solid can be there at all;
//! - a **minimum** about half a pebble diameter in, where the first layer of
//!   sphere equators sits;
//! - a **maximum** about one diameter in, in the gap between layers;
//! - further oscillations of decaying amplitude, settling to the bulk value
//!   several diameters from the wall.
//!
//! This is the single most important structural feature of a packed bed for
//! reactor work, because coolant takes the path of least resistance: the
//! high-porosity channel at the wall carries disproportionate flow (**wall
//! channelling** or **bypass**), which is exactly where you least want it in a
//! core whose power is generated in the interior. A bed model that knows only
//! the bulk porosity cannot represent it.
//!
//! ## This contradicts a correlation already in the workspace, deliberately
//!
//! [`tampines::pebble_bed::zbs::ZbsBed::wall_region_porosity`] supplies a
//! **monotonic exponential** profile,
//! `eps(y) = eps_bulk * (1 + 1.36 * exp(-5 y / d))`, rising smoothly from the
//! bulk value to the wall. Its own doc comment already flags the coefficients
//! as provisional, unverified against the original paper, and "adequate for
//! exercising the local-porosity mechanism, not for quantitative wall-region
//! V&V".
//!
//! **The measurement below shows the true profile is oscillatory, not
//! monotonic** — it goes *below* the bulk value before it comes back, which no
//! decaying exponential can do. That is a real structural disagreement, not a
//! coefficient mismatch, and it means the ZBS placeholder cannot be fixed by
//! re-fitting: it has the wrong shape. A DEM bed is the natural source of the
//! right one. Recorded here rather than acted on, because changing `tampines`'
//! wall-region model is a separate change with its own V&V.
//!
//! ## Methodology
//!
//! **Bed.** `reference-data/liggghts/pebble_bed_settled.csv` — 354
//! monodisperse 10 mm pebbles settled under gravity in a 60 mm diameter
//! cylinder by **upstream LIGGGHTS-PUBLIC** (`3d5c00f2`), not by this port.
//! Using upstream's own settled state means this map is anchored to the
//! cross-code reference rather than to our own integrator, so a regression
//! here is a regression in the *analysis*, not a re-test of the solver.
//! `D/d = 6`, so the bed radius spans only three pebble diameters — enough to
//! resolve the wall peak and the first minimum, **not** enough to watch the
//! oscillation damp out. Stated rather than glossed.
//!
//! **Estimator.** Deterministic point sampling on a stratified cylindrical
//! grid. A sample point is solid if it lies within one pebble radius of any
//! centre; porosity is the void fraction of the samples in each bin. No RNG,
//! so the map is bit-reproducible — which is what makes it usable as a
//! regression fixture at all.
//!
//! The radial profile samples only the **axial bulk window**, `z` from
//! `z_min + 2d` to `z_max - 2d`, excluding the floor layer and the free
//! surface so that the radial structure is not contaminated by the axial one.
//! The axial profile samples the full cross-section at each height.
//!
//! **Pass criteria.** Structural, not correlation-fitted. **No published
//! radial-voidage correlation (Mueller, de Klerk, Benenati and Brosilow, …)
//! is in `crates/kovan-literature`**, so this test deliberately asserts only
//! properties that are robust and independently checkable — the wall value,
//! the existence and location of the first minimum, the oscillation, and the
//! bulk level — plus a tight regression against the committed profile below.
//! Quoting correlation coefficients from memory would be exactly the
//! fabrication the workspace rules forbid. Cataloguing one of those papers in
//! `kovan` and adding a quantitative gate is the obvious follow-up.
//!
//! ## Results (2026-09-16, `--release`)
//!
//! Measured values are in [`RADIAL_REFERENCE`] and [`AXIAL_REFERENCE`], which
//! are the regression fixture. Headline numbers are recorded on each test.

use outram_park_fork_liggghts::particle::Vec3;

/// Pebble radius of the reference bed `[m]`.
const R_P: f64 = 0.005;
/// Pebble diameter `[m]`.
const D_P: f64 = 2.0 * R_P;
/// Container radius of the reference bed `[m]`.
const R_CYL: f64 = 0.030;
/// Radial/axial bin width `[m]` — one tenth of a pebble diameter.
const BIN: f64 = D_P / 10.0;

fn load_settled() -> Option<Vec<Vec3>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts/pebble_bed_settled.csv");
    let text = std::fs::read_to_string(path).ok()?;
    let mut rows: Vec<(usize, Vec3)> = text
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<f64> = l
                .split(',')
                .map(|v| v.parse().expect("numeric csv"))
                .collect();
            (f[0] as usize, Vec3::new(f[1], f[2], f[3]))
        })
        .collect();
    rows.sort_by_key(|r| r.0);
    Some(rows.into_iter().map(|r| r.1).collect())
}

/// True if `(x, y, z)` lies inside any pebble.
fn solid(centres: &[Vec3], x: f64, y: f64, z: f64) -> bool {
    let rr = R_P * R_P;
    centres.iter().any(|c| {
        let (dx, dy, dz) = (c.x - x, c.y - y, c.z - z);
        dx * dx + dy * dy + dz * dz <= rr
    })
}

/// The axial window over which the radial profile is taken: the bed minus two
/// pebble diameters at each end.
fn axial_window(centres: &[Vec3]) -> (f64, f64) {
    let zmin = centres.iter().map(|c| c.z).fold(f64::INFINITY, f64::min);
    let zmax = centres
        .iter()
        .map(|c| c.z)
        .fold(f64::NEG_INFINITY, f64::max);
    (zmin + 2.0 * D_P, zmax - 2.0 * D_P)
}

/// Radial porosity profile: `(distance from the wall [m], porosity [-])`,
/// ordered from the wall inward.
fn radial_porosity(centres: &[Vec3]) -> Vec<(f64, f64)> {
    let (z_lo, z_hi) = axial_window(centres);
    let n_bins = (R_CYL / BIN).round() as usize;
    let (n_r, n_th, n_z) = (2usize, 48usize, 150usize);
    let mut out = Vec::with_capacity(n_bins);
    for b in 0..n_bins {
        let (r0, r1) = (b as f64 * BIN, (b + 1) as f64 * BIN);
        let mut void = 0usize;
        let mut total = 0usize;
        for ir in 0..n_r {
            // Area-weighted stratification within the annulus.
            let u = (ir as f64 + 0.5) / n_r as f64;
            let r = (r0 * r0 + u * (r1 * r1 - r0 * r0)).sqrt();
            for it in 0..n_th {
                let th = 2.0 * std::f64::consts::PI * (it as f64 + 0.5) / n_th as f64;
                let (x, y) = (r * th.cos(), r * th.sin());
                for iz in 0..n_z {
                    let z = z_lo + (z_hi - z_lo) * (iz as f64 + 0.5) / n_z as f64;
                    total += 1;
                    if !solid(centres, x, y, z) {
                        void += 1;
                    }
                }
            }
        }
        let r_mid = 0.5 * (r0 + r1);
        out.push((R_CYL - r_mid, void as f64 / total as f64));
    }
    out.reverse(); // wall first
    out
}

/// Axial porosity profile: `(height above the floor [m], porosity [-])`.
fn axial_porosity(centres: &[Vec3]) -> Vec<(f64, f64)> {
    let zmax = centres
        .iter()
        .map(|c| c.z)
        .fold(f64::NEG_INFINITY, f64::max);
    let top = zmax + R_P;
    let n_bins = (top / BIN).ceil() as usize;
    let (n_r, n_th, n_z) = (24usize, 48usize, 4usize);
    let mut out = Vec::with_capacity(n_bins);
    for b in 0..n_bins {
        let (z0, z1) = (b as f64 * BIN, (b + 1) as f64 * BIN);
        let mut void = 0usize;
        let mut total = 0usize;
        for ir in 0..n_r {
            let u = (ir as f64 + 0.5) / n_r as f64;
            let r = R_CYL * u.sqrt(); // uniform in area
            for it in 0..n_th {
                let th = 2.0 * std::f64::consts::PI * (it as f64 + 0.5) / n_th as f64;
                let (x, y) = (r * th.cos(), r * th.sin());
                for iz in 0..n_z {
                    let z = z0 + (z1 - z0) * (iz as f64 + 0.5) / n_z as f64;
                    total += 1;
                    if !solid(centres, x, y, z) {
                        void += 1;
                    }
                }
            }
        }
        out.push((0.5 * (z0 + z1), void as f64 / total as f64));
    }
    out
}

/// Measured radial porosity profile: `(distance from the wall / d, porosity)`,
/// wall first. Regression fixture — see the module docs for how it was taken.
const RADIAL_REFERENCE: [(f64, f64); 30] = [
    (0.050, 0.875347),
    (0.150, 0.672292),
    (0.250, 0.511597),
    (0.350, 0.394306),
    (0.450, 0.305278),
    (0.550, 0.263125),
    (0.650, 0.283681),
    (0.750, 0.347778),
    (0.850, 0.435000),
    (0.950, 0.551944),
    (1.050, 0.553333),
    (1.150, 0.451597),
    (1.250, 0.380417),
    (1.350, 0.340903),
    (1.450, 0.320903),
    (1.550, 0.330833),
    (1.650, 0.367083),
    (1.750, 0.424167),
    (1.850, 0.472847),
    (1.950, 0.450833),
    (2.050, 0.394722),
    (2.150, 0.359444),
    (2.250, 0.328542),
    (2.350, 0.321250),
    (2.450, 0.335694),
    (2.550, 0.384306),
    (2.650, 0.444792),
    (2.750, 0.495625),
    (2.850, 0.541806),
    (2.950, 0.574167),
];

/// Measured axial porosity profile: `(height above the floor / d, porosity)`.
/// Regression fixture.
const AXIAL_REFERENCE: [(f64, f64); 128] = [
    (0.050, 0.872396),
    (0.150, 0.692274),
    (0.250, 0.552300),
    (0.350, 0.443359),
    (0.450, 0.378472),
    (0.550, 0.359158),
    (0.650, 0.376085),
    (0.750, 0.433811),
    (0.850, 0.510851),
    (0.950, 0.589193),
    (1.050, 0.567925),
    (1.150, 0.468750),
    (1.250, 0.407552),
    (1.350, 0.394748),
    (1.450, 0.403212),
    (1.550, 0.442708),
    (1.650, 0.468316),
    (1.750, 0.470486),
    (1.850, 0.495660),
    (1.950, 0.476562),
    (2.050, 0.457682),
    (2.150, 0.455946),
    (2.250, 0.454644),
    (2.350, 0.458333),
    (2.450, 0.460720),
    (2.550, 0.478082),
    (2.650, 0.469618),
    (2.750, 0.457465),
    (2.850, 0.460069),
    (2.950, 0.432509),
    (3.050, 0.426215),
    (3.150, 0.443576),
    (3.250, 0.461155),
    (3.350, 0.459418),
    (3.450, 0.475911),
    (3.550, 0.474392),
    (3.650, 0.448785),
    (3.750, 0.421875),
    (3.850, 0.437283),
    (3.950, 0.425998),
    (4.050, 0.419488),
    (4.150, 0.425998),
    (4.250, 0.419054),
    (4.350, 0.410590),
    (4.450, 0.407118),
    (4.550, 0.407986),
    (4.650, 0.402561),
    (4.750, 0.401259),
    (4.850, 0.404297),
    (4.950, 0.396050),
    (5.050, 0.419054),
    (5.150, 0.465278),
    (5.250, 0.489149),
    (5.350, 0.486328),
    (5.450, 0.474392),
    (5.550, 0.452908),
    (5.650, 0.421875),
    (5.750, 0.400174),
    (5.850, 0.379774),
    (5.950, 0.388021),
    (6.050, 0.409939),
    (6.150, 0.442491),
    (6.250, 0.449870),
    (6.350, 0.470269),
    (6.450, 0.477865),
    (6.550, 0.500000),
    (6.650, 0.478299),
    (6.750, 0.450738),
    (6.850, 0.427951),
    (6.950, 0.419271),
    (7.050, 0.460938),
    (7.150, 0.490451),
    (7.250, 0.476780),
    (7.350, 0.453342),
    (7.450, 0.443576),
    (7.550, 0.474826),
    (7.650, 0.474609),
    (7.750, 0.459418),
    (7.850, 0.440321),
    (7.950, 0.429253),
    (8.050, 0.446615),
    (8.150, 0.457465),
    (8.250, 0.445964),
    (8.350, 0.427951),
    (8.450, 0.441406),
    (8.550, 0.450955),
    (8.650, 0.419922),
    (8.750, 0.405599),
    (8.850, 0.425347),
    (8.950, 0.451172),
    (9.050, 0.462023),
    (9.150, 0.449219),
    (9.250, 0.470703),
    (9.350, 0.491102),
    (9.450, 0.489149),
    (9.550, 0.453776),
    (9.650, 0.413194),
    (9.750, 0.375000),
    (9.850, 0.366753),
    (9.950, 0.391493),
    (10.050, 0.440104),
    (10.150, 0.470486),
    (10.250, 0.493490),
    (10.350, 0.483941),
    (10.450, 0.465712),
    (10.550, 0.434679),
    (10.650, 0.411458),
    (10.750, 0.421875),
    (10.850, 0.423828),
    (10.950, 0.424262),
    (11.050, 0.458550),
    (11.150, 0.464193),
    (11.250, 0.463325),
    (11.350, 0.452474),
    (11.450, 0.439453),
    (11.550, 0.439236),
    (11.650, 0.481337),
    (11.750, 0.560330),
    (11.850, 0.654514),
    (11.950, 0.730686),
    (12.050, 0.810547),
    (12.150, 0.865451),
    (12.250, 0.913845),
    (12.350, 0.954427),
    (12.450, 0.975911),
    (12.550, 0.984375),
    (12.650, 0.990017),
    (12.750, 0.999132),
];

/// Indices of the local minima and maxima of `profile`, interior points only.
fn turning_points(profile: &[(f64, f64)]) -> (Vec<usize>, Vec<usize>) {
    let mut minima = Vec::new();
    let mut maxima = Vec::new();
    for i in 1..profile.len().saturating_sub(1) {
        let (a, b, c) = (profile[i - 1].1, profile[i].1, profile[i + 1].1);
        if b < a && b < c {
            minima.push(i);
        }
        if b > a && b > c {
            maxima.push(i);
        }
    }
    (minima, maxima)
}

/// **Methodology.** The radial profile of a settled monodisperse bed must show
/// the classical near-wall oscillation, not a monotonic approach to the bulk.
/// Asserted: porosity above 0.85 in the wall bin; a first minimum within
/// `0.3-0.7 d` of the wall and well below the bulk; a following maximum within
/// `0.8-1.3 d`; at least two further turning points; and an oscillation period
/// of roughly one pebble diameter. Bed and estimator per the module docs.
///
/// **Results** (2026-09-16, `--release`), `d = 10 mm`, `D/d = 6`:
///
/// | y/d from wall | eps | |
/// |---|---|---|
/// | 0.05 | 0.875347 | wall bin, heading for 1 |
/// | **0.55** | **0.263125** | **first minimum** — first layer of equators |
/// | **1.05** | **0.553333** | **first maximum** — gap between layers |
/// | 1.45 | 0.320903 | second minimum |
/// | 1.85 | 0.472847 | second maximum |
/// | 2.35 | 0.321250 | third minimum |
///
/// Oscillation period `1.45 - 0.55 = 0.90 d`, i.e. one pebble diameter to
/// within the 0.1 d bin width. Peak-to-trough amplitude decays from 0.290
/// (first) to 0.152 (second) — damped, as it must be.
///
/// **The first minimum, 0.263, is far below the bulk 0.443.** No monotonic
/// profile can produce that, which is the module doc's point about the ZBS
/// placeholder.
///
/// The rise at the last two bins (`y/d = 2.85, 2.95`, eps 0.542 and 0.574) is
/// the **cylinder axis**, not physics worth trusting: at `D/d = 6` the axis is
/// a special site three diameters from every wall, and the sampling annulus
/// there is tiny. Not asserted.
#[test]
fn radial_porosity_is_the_oscillatory_near_wall_profile() {
    let Some(centres) = load_settled() else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    let profile = radial_porosity(&centres);
    for (y, e) in &profile {
        println!("radial {:.3} {:.6}", y / D_P, e);
    }

    assert!(
        profile[0].1 > 0.85,
        "wall bin porosity {:.4} — a bed cannot be dense against a wall",
        profile[0].1
    );

    let (minima, maxima) = turning_points(&profile);
    assert!(
        minima.len() >= 3 && maxima.len() >= 2,
        "expected an oscillatory profile; found {} minima and {} maxima",
        minima.len(),
        maxima.len()
    );

    let (y1, e1) = profile[minima[0]];
    assert!(
        (0.3..0.7).contains(&(y1 / D_P)),
        "first minimum at {:.2} d from the wall, expected near 0.5 d",
        y1 / D_P
    );
    assert!(
        e1 < 0.32,
        "first minimum porosity {e1:.4} is not below the bulk ~0.44"
    );

    let (y2, e2) = profile[maxima[0]];
    assert!(
        (0.8..1.3).contains(&(y2 / D_P)),
        "first maximum at {:.2} d from the wall, expected near 1.0 d",
        y2 / D_P
    );
    assert!(
        e2 > e1 + 0.15,
        "oscillation amplitude {:.4} too small",
        e2 - e1
    );

    // Period: successive minima about one diameter apart.
    let period = (profile[minima[1]].0 - profile[minima[0]].0) / D_P;
    println!("oscillation period {period:.3} d");
    assert!(
        (0.6..1.4).contains(&period),
        "successive minima {period:.2} d apart, expected about 1 d"
    );

    // Damping: the second peak-to-trough swing is smaller than the first.
    let swing1 = e2 - e1;
    let swing2 = profile[maxima[1]].1 - profile[minima[1]].1;
    println!("amplitude {swing1:.4} then {swing2:.4}");
    assert!(
        swing2 < swing1,
        "oscillation is not damping: {swing1:.4} then {swing2:.4}"
    );
}

/// **Methodology.** The axial profile must show the same layering against the
/// floor, a bulk interior, and a free surface rising to `eps = 1`. Asserted:
/// floor bin above 0.85; a first minimum within `0.3-0.7 d` of the floor; the
/// interior between `4 d` and `11 d` flat and at the bulk value; and the top
/// bin above 0.95.
///
/// **Results** (2026-09-16, `--release`):
///
/// | z/d | eps | |
/// |---|---|---|
/// | 0.05 | 0.872396 | floor bin |
/// | 0.55 | 0.359158 | first minimum |
/// | 0.95 | 0.589193 | first maximum |
/// | 4.0-11.0 | **0.440089** mean (0.367-0.500) | interior |
/// | 12.75 | 0.999132 | free surface |
///
/// **Cross-check on the estimator, and it is a good one.** The interior mean
/// 0.440089 and the area-weighted radial mean 0.442993 both reproduce this
/// bed's independently recorded bulk voidage of **0.4429** (`pebble_bed_bulk`,
/// which computes it by an entirely different route — an analytic
/// spherical-cap integral, no sampling) to within 0.3 %. Two independent
/// estimators agreeing on a third, previously recorded number is what makes
/// the map trustworthy enough to be a fixture.
#[test]
fn axial_porosity_has_floor_layering_a_bulk_and_a_free_surface() {
    let Some(centres) = load_settled() else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    let profile = axial_porosity(&centres);
    assert!(
        profile[0].1 > 0.85,
        "floor bin porosity {:.4}",
        profile[0].1
    );
    assert!(
        profile[profile.len() - 1].1 > 0.95,
        "top bin porosity {:.4} — the free surface should be nearly all void",
        profile[profile.len() - 1].1
    );

    let (minima, _) = turning_points(&profile);
    let y1 = profile[minima[0]].0 / D_P;
    assert!(
        (0.3..0.7).contains(&y1),
        "first axial minimum at {y1:.2} d above the floor, expected near 0.5 d"
    );

    let interior: Vec<f64> = profile
        .iter()
        .filter(|(z, _)| (4.0 * D_P..=11.0 * D_P).contains(z))
        .map(|(_, e)| *e)
        .collect();
    let mean = interior.iter().sum::<f64>() / interior.len() as f64;
    println!(
        "interior mean porosity {mean:.6} over {} bins",
        interior.len()
    );
    assert!(
        (0.40..0.48).contains(&mean),
        "interior mean porosity {mean:.4} is outside the range for a settled \
         random packing at D/d = 6"
    );
    // Against the independently recorded bulk voidage of this same bed.
    assert!(
        (mean - 0.4429).abs() < 0.02,
        "interior mean {mean:.4} disagrees with the bulk voidage 0.4429 \
         recorded by pebble_bed_bulk, which uses a different estimator"
    );
}

/// **Methodology — the regression gate.** The estimator is deterministic (a
/// fixed cylindrical sampling grid, no RNG), so the whole map is reproducible
/// and any change to the loader, the estimator or the reference bed moves it.
/// Both profiles are compared bin by bin against the committed fixtures.
///
/// Tolerance 2e-3 absolute. The values are ratios of sample counts, so they
/// are exactly reproducible in principle; the tolerance exists to absorb the
/// 6-decimal rounding of the fixtures below, plus any boundary sample that
/// flips if `sin`/`cos` differ in the last ulp on another platform. It is far
/// tighter than any real change: the profile moves by ~0.3 across a single
/// bin.
///
/// **Results** (2026-09-16, `--release`): worst deviation **4.44e-7** over the
/// 30 radial bins and **5.00e-7** over the 128 axial bins. Both are exactly
/// the half-ulp of the fixtures' 6-decimal rounding, i.e. the estimator
/// reproduces itself to every digit recorded — the tolerance is not being
/// used to hide drift.
#[test]
fn porosity_map_matches_the_committed_profile() {
    let Some(centres) = load_settled() else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    const TOL: f64 = 2.0e-3;

    let radial = radial_porosity(&centres);
    assert_eq!(radial.len(), RADIAL_REFERENCE.len(), "radial bin count");
    let mut worst = 0.0_f64;
    for (i, ((y, e), (y_ref, e_ref))) in radial.iter().zip(RADIAL_REFERENCE.iter()).enumerate() {
        assert!(
            (y / D_P - y_ref).abs() < 1e-9,
            "radial bin {i} at {:.4} d, fixture says {y_ref:.4} d",
            y / D_P
        );
        worst = worst.max((e - e_ref).abs());
        assert!(
            (e - e_ref).abs() < TOL,
            "radial bin {i} ({y_ref:.2} d from wall): porosity {e:.6}, \
             fixture {e_ref:.6}"
        );
    }
    println!(
        "radial: worst deviation {worst:.2e} over {} bins",
        radial.len()
    );

    let axial = axial_porosity(&centres);
    assert_eq!(axial.len(), AXIAL_REFERENCE.len(), "axial bin count");
    let mut worst_a = 0.0_f64;
    for (i, ((z, e), (z_ref, e_ref))) in axial.iter().zip(AXIAL_REFERENCE.iter()).enumerate() {
        assert!(
            (z / D_P - z_ref).abs() < 1e-9,
            "axial bin {i} at {:.4} d, fixture says {z_ref:.4} d",
            z / D_P
        );
        worst_a = worst_a.max((e - e_ref).abs());
        assert!(
            (e - e_ref).abs() < TOL,
            "axial bin {i} ({z_ref:.2} d above floor): porosity {e:.6}, \
             fixture {e_ref:.6}"
        );
    }
    println!(
        "axial: worst deviation {worst_a:.2e} over {} bins",
        axial.len()
    );
}

/// **Methodology — the recorded disagreement with `tampines`' placeholder.**
///
/// `tampines::pebble_bed::zbs::ZbsBed::wall_region_porosity` models the
/// near-wall region as `eps(y) = eps_bulk * (1 + 1.36 exp(-5 y / d))`, clamped
/// to 1. The formula is reproduced locally rather than imported, because this
/// crate does not depend on `tampines` and should not start to for a test.
///
/// This test does not assert that the placeholder is wrong in some vague
/// sense. It asserts the **specific structural fact** that makes it
/// unfixable by refitting: the measured profile goes *below* the bulk
/// porosity, and a positive decaying exponential times the bulk value cannot,
/// for any coefficients.
///
/// **Results** (2026-09-16, `--release`), `eps_bulk = 0.4429`:
///
/// | y/d | measured | ZBS placeholder | |
/// |---|---|---|---|
/// | 0.05 | 0.8753 | 0.9120 | both heading for 1 |
/// | 0.55 | **0.2631** | 0.4814 | placeholder is **1.83x** the measured value |
/// | 1.05 | 0.5533 | 0.4461 | placeholder already flat; measured is at a peak |
/// | 1.45 | 0.3209 | 0.4433 | placeholder is at the bulk value |
///
/// The placeholder is monotonic by construction and never dips below 0.4429;
/// the measured profile reaches 0.263. Note the sign of the error **flips**
/// between `y/d = 0.55` and `1.05` — the placeholder is too high at the
/// minimum and too low at the maximum, which is the signature of a wrong shape
/// rather than a wrong amplitude. **The disagreement is the shape, not the
/// coefficients.** Replacing it is a change to `tampines` with its own V&V,
/// deliberately not made here.
#[test]
fn the_zbs_wall_placeholder_has_the_wrong_shape() {
    let Some(centres) = load_settled() else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    // Reproduced from tampines::pebble_bed::zbs::ZbsBed::wall_region_porosity.
    let zbs = |y: f64, eps_bulk: f64| (eps_bulk * (1.0 + 1.36 * (-5.0 * y / D_P).exp())).min(1.0);
    let eps_bulk = 0.4429;

    let profile = radial_porosity(&centres);
    let (minima, _) = turning_points(&profile);
    let (y_min, e_min) = profile[minima[0]];

    for &(y, e) in profile.iter().take(15) {
        println!(
            "y/d {:.2}  measured {e:.4}  zbs {:.4}",
            y / D_P,
            zbs(y, eps_bulk)
        );
    }

    // The measured first minimum is below the bulk value.
    assert!(
        e_min < eps_bulk,
        "measured first minimum {e_min:.4} is not below the bulk {eps_bulk:.4}; \
         the premise of this test no longer holds"
    );
    // The placeholder cannot go below the bulk value anywhere, by construction.
    let zbs_min = profile
        .iter()
        .map(|(y, _)| zbs(*y, eps_bulk))
        .fold(f64::INFINITY, f64::min);
    assert!(
        zbs_min >= eps_bulk - 1e-12,
        "the reproduced ZBS formula dipped to {zbs_min:.6}, below the bulk — \
         it has been changed and this test needs rewriting"
    );
    println!(
        "at the first minimum (y/d {:.2}): measured {e_min:.4}, zbs {:.4}, ratio {:.2}x",
        y_min / D_P,
        zbs(y_min, eps_bulk),
        zbs(y_min, eps_bulk) / e_min
    );
}
