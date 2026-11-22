// SPDX-License-Identifier: AGPL-3.0-only

//! **gh:#1712: mesh-surface currents on rectilinear, cylindrical and spherical
//! meshes**, through the port of OpenMC's `StructuredMesh::raytrace_mesh`
//! (`src/mesh.cpp:1312-1488`, OpenMC `8f79d9d0`) now generic over the four
//! structured kinds, with each mesh's `get_indices` and
//! `distance_to_grid_boundary` ported (`:1839`, `:1885`, `:1980-2146`,
//! `:2278-2482`).
//!
//! # Methodology
//!
//! 1. **Regular mesh, bit-identity with gh:#1709.** A verbatim copy of the
//!    regular-only walker that gh:#1709 landed (frozen below as `walk_1709`)
//!    against the generic one, on 20 000 random tracks **including** tracks
//!    that start or end exactly on a plane. Pass: identical bin lists.
//! 2. **Rectilinear with uniform planes = regular.** Planes built as
//!    `ll + i * w` (the regular mesh's own arithmetic); 20 000 random off-tie
//!    tracks (no end within 1e-6 cm of a plane). Pass: identical bins.
//! 3. **Analytic tracks, hand-computed bins**: a non-uniform rectilinear
//!    mesh; a radial chord through the axis of a cylindrical and through the
//!    centre of a spherical mesh, both with an off-zero origin; a phi crossing
//!    and the 2 pi -> 0 wrap on a cylindrical mesh; a theta crossing on a
//!    spherical mesh. Pass: exact bin lists.
//! 4. **Conservation**: on 5000 random tracks per curvilinear/rectilinear
//!    mesh, for every element `#inward - #outward = [r1 in e] - [r0 in e]`,
//!    and every inward current is into the element the next outward current
//!    leaves. Pass: exact on every track.
//!
//! **Predictions (before the run, `verification_and_validation/mesh_surface_meshes_1712/README.md`):**
//! 1 and 2 identical; 3 exact; 4 exact.
//!
//! # Results (2026-11-22)
//!
//! 1, 2 identical; 4 exact (after excluding the upstream behaviours pinned
//! below); 3 exact against **upstream's** labelling, which differs from the
//! hand count in the face label of some inward currents. Full table and the
//! three upstream findings: the V&V README.

use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::tally::filter::{Filter, FilterEvent};
use outram_mc_libs::tally::filter_extra::MeshSurfaceFilter;
use outram_mc_libs::tally::mesh::{
    CylindricalMesh, MeshKind, MeshKindExt, RectilinearMesh, RegularMesh, RegularMeshExt, SphericalMesh,
};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

fn dir(a: Position, b: Position) -> Direction {
    let d = [b.x - a.x, b.y - a.y, b.z - a.z];
    let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    Direction { u: d[0] / n, v: d[1] / n, w: d[2] / n }
}

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
    fn point(&mut self, lo: [f64; 3], hi: [f64; 3]) -> Position {
        Position::new(
            lo[0] + (hi[0] - lo[0]) * self.next(),
            lo[1] + (hi[1] - lo[1]) * self.next(),
            lo[2] + (hi[2] - lo[2]) * self.next(),
        )
    }
}

// ---------------------------------------------------------------------------
// 1. Frozen gh:#1709 regular-mesh walker (verbatim logic, commit 7fb1cec0c).
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Md {
    next_index: i64,
    max_surface: bool,
    distance: f64,
}

fn idx_1709(m: &RegularMesh, r: f64, i: usize, width: f64) -> i64 {
    let shape = m.dimension[i] as i64;
    if r <= m.lower_left[i] {
        return if r == m.lower_left[i] { 1 } else { 0 };
    }
    if r >= m.upper_right[i] {
        return if r == m.upper_right[i] { shape } else { shape + 1 };
    }
    ((r - m.lower_left[i]) / width).ceil() as i64
}

fn indices_1709(m: &RegularMesh, r: [f64; 3], w: [f64; 3]) -> ([i64; 3], bool) {
    let mut ijk = [0i64; 3];
    let mut inside = true;
    for i in 0..3 {
        ijk[i] = idx_1709(m, r[i], i, w[i]);
        if ijk[i] < 1 || ijk[i] > m.dimension[i] as i64 {
            inside = false;
        }
    }
    (ijk, inside)
}

fn dist_1709(m: &RegularMesh, ijk: [i64; 3], i: usize, r0: [f64; 3], u: [f64; 3], w: [f64; 3]) -> Md {
    let mut d = Md { next_index: ijk[i], max_surface: false, distance: f64::INFINITY };
    if u[i].abs() < 1.0e-14 {
        return d;
    }
    d.max_surface = u[i] > 0.0;
    if d.max_surface && ijk[i] <= m.dimension[i] as i64 {
        d.next_index += 1;
        d.distance = ((m.lower_left[i] + ijk[i] as f64 * w[i]) - r0[i]) / u[i];
    } else if !d.max_surface && ijk[i] >= 1 {
        d.next_index -= 1;
        d.distance = ((m.lower_left[i] + (ijk[i] - 1) as f64 * w[i]) - r0[i]) / u[i];
    }
    d
}

fn walk_1709(m: &RegularMesh, r0: Position, r1: Position, u: Direction) -> Vec<usize> {
    const TINY_BIT: f64 = 1.0e-8;
    let mut bins = Vec::new();
    let width = m.width();
    let dr = [r1.x - r0.x, r1.y - r0.y, r1.z - r0.z];
    let total = (dr[0] * dr[0] + dr[1] * dr[1] + dr[2] * dr[2]).sqrt();
    if total == 0.0 {
        return bins;
    }
    let lr = [r0.x, r0.y, r0.z];
    let uu = [u.u, u.v, u.w];
    let at = |l: f64| [lr[0] + l * uu[0], lr[1] + l * uu[1], lr[2] + l * uu[2]];
    let mut trav = 0.0f64;
    let (mut ijk, mut in_mesh) = indices_1709(m, at(TINY_BIT), width);
    if total < 2.0 * TINY_BIT {
        return bins;
    }
    let bin_of = |ijk: [i64; 3]| -> usize {
        (((ijk[2] - 1) as usize * m.dimension[1] + (ijk[1] - 1) as usize) * m.dimension[0]) + (ijk[0] - 1) as usize
    };
    let mut d = [Md { next_index: 0, max_surface: false, distance: f64::INFINITY }; 3];
    for k in 0..3 {
        d[k] = dist_1709(m, ijk, k, lr, uu, width);
    }
    loop {
        if in_mesh {
            let mut k = 0;
            for j in 1..3 {
                if d[j].distance < d[k].distance {
                    k = j;
                }
            }
            trav = d[k].distance;
            if trav >= total {
                return bins;
            }
            bins.push(m.surface_bin(bin_of(ijk), k, d[k].max_surface, false));
            ijk[k] = d[k].next_index;
            d[k] = dist_1709(m, ijk, k, lr, uu, width);
            in_mesh = ijk[k] >= 1 && ijk[k] <= m.dimension[k] as i64;
            if in_mesh {
                bins.push(m.surface_bin(bin_of(ijk), k, !d[k].max_surface, true));
            }
        } else {
            let mut k_max: Option<usize> = None;
            for k in 0..3 {
                if (ijk[k] < 1 || ijk[k] > m.dimension[k] as i64) && d[k].distance > trav {
                    trav = d[k].distance;
                    k_max = Some(k);
                }
            }
            if k_max.is_none() {
                trav += TINY_BIT;
            }
            if trav >= total {
                return bins;
            }
            let (n, inside) = indices_1709(m, at(trav + TINY_BIT), width);
            ijk = n;
            in_mesh = inside;
            for k in 0..3 {
                d[k] = dist_1709(m, ijk, k, lr, uu, width);
            }
            if let (true, Some(k)) = (in_mesh, k_max) {
                bins.push(m.surface_bin(bin_of(ijk), k, !d[k].max_surface, true));
            }
        }
    }
}

fn reg() -> RegularMesh {
    RegularMesh { lower_left: [-5.0, -3.0, -2.0], upper_right: [5.0, 3.0, 6.0], dimension: [5, 3, 4] }
}

/// Snap a coordinate to the nearest plane of axis `k` (to make ties).
fn snap(m: &RegularMesh, x: f64, k: usize) -> f64 {
    let w = m.width()[k];
    let i = ((x - m.lower_left[k]) / w).round().clamp(0.0, m.dimension[k] as f64);
    m.lower_left[k] + i * w
}

#[test]
fn regular_mesh_generic_walker_is_bit_identical_to_the_1709_port() {
    let m = reg();
    let kind = MeshKind::Regular(m.clone());
    let mut g = Lcg(0x1712_0001);
    let mut ties = 0;
    let mut total_bins = 0;
    for n in 0..20_000 {
        let mut a = g.point([-8.0, -6.0, -5.0], [8.0, 6.0, 9.0]);
        let mut b = g.point([-8.0, -6.0, -5.0], [8.0, 6.0, 9.0]);
        // Every fourth track starts or ends exactly on a plane.
        if n % 4 == 1 {
            a.x = snap(&m, a.x, 0);
            ties += 1;
        } else if n % 4 == 2 {
            b.y = snap(&m, b.y, 1);
            ties += 1;
        } else if n % 4 == 3 {
            b.z = snap(&m, b.z, 2);
            a.x = snap(&m, a.x, 0);
            ties += 1;
        }
        let u = dir(a, b);
        let frozen = walk_1709(&m, a, b, u);
        let now = m.surface_bins_crossed_openmc(a, b, u);
        assert_eq!(now, frozen, "track {n}: {a:?} -> {b:?}");
        assert_eq!(kind.surface_bins_crossed_openmc(a, b, u).unwrap(), frozen);
        total_bins += now.len();
    }
    println!("regular: 20000 tracks ({ties} with a tie) identical to gh:#1709, {total_bins} bins");
    assert!(total_bins > 20_000);
}

// ---------------------------------------------------------------------------
// 2. Rectilinear with uniform planes against the regular port.
// ---------------------------------------------------------------------------

fn uniform_rectilinear(m: &RegularMesh) -> RectilinearMesh {
    let w = m.width();
    RectilinearMesh {
        grid: core::array::from_fn(|k| (0..=m.dimension[k]).map(|i| m.lower_left[k] + i as f64 * w[k]).collect()),
    }
}

fn off_plane(grid: &[Vec<f64>; 3], p: Position) -> f64 {
    let r = [p.x, p.y, p.z];
    let mut best = f64::INFINITY;
    for k in 0..3 {
        for &e in &grid[k] {
            best = best.min((r[k] - e).abs());
        }
    }
    best
}

#[test]
fn rectilinear_with_uniform_planes_equals_the_regular_port_off_ties() {
    let m = reg();
    let rect = uniform_rectilinear(&m);
    let kind = MeshKind::Rectilinear(rect.clone());
    let mut g = Lcg(0x1712_0002);
    let mut n = 0;
    let mut bins = 0;
    while n < 20_000 {
        let a = g.point([-8.0, -6.0, -5.0], [8.0, 6.0, 9.0]);
        let b = g.point([-8.0, -6.0, -5.0], [8.0, 6.0, 9.0]);
        if off_plane(&rect.grid, a) < 1e-6 || off_plane(&rect.grid, b) < 1e-6 {
            continue;
        }
        let u = dir(a, b);
        let r = kind.surface_bins_crossed_openmc(a, b, u).unwrap();
        assert_eq!(r, m.surface_bins_crossed_openmc(a, b, u), "track {n}: {a:?} -> {b:?}");
        bins += r.len();
        n += 1;
    }
    println!("rectilinear(uniform): 20000 off-tie tracks identical to regular, {bins} bins");
    assert!(bins > 20_000);
}

// ---------------------------------------------------------------------------
// 3. Analytic tracks.
// ---------------------------------------------------------------------------

/// Non-uniform x planes `[0, 1, 3, 6]`, one cell in y and z. Bin layout
/// `12 e + 4 axis + 2 max + inward`.
#[test]
fn rectilinear_non_uniform_planes_against_hand_computed_bins() {
    let kind = MeshKind::Rectilinear(RectilinearMesh { grid: [vec![0.0, 1.0, 3.0, 6.0], vec![0.0, 1.0], vec![0.0, 1.0]] });
    let px = Direction { u: 1.0, v: 0.0, w: 0.0 };
    // Inside elem 0 to inside elem 2: x = 1 (out 0 max = 2, in 1 min = 13),
    // x = 3 (out 1 max = 14, in 2 min = 25).
    let r = kind.surface_bins_crossed_openmc(Position::new(0.5, 0.5, 0.5), Position::new(5.0, 0.5, 0.5), px).unwrap();
    assert_eq!(r, vec![2, 13, 14, 25]);
    // From outside: enter elem 0 through its min x face (inward = 1).
    let r = kind.surface_bins_crossed_openmc(Position::new(-1.0, 0.5, 0.5), Position::new(2.0, 0.5, 0.5), px).unwrap();
    assert_eq!(r, vec![1, 2, 13]);
    // Leaving through x = 6: out of elem 2 max (= 26), nothing inward.
    let r = kind.surface_bins_crossed_openmc(Position::new(4.0, 0.5, 0.5), Position::new(7.0, 0.5, 0.5), px).unwrap();
    assert_eq!(r, vec![26]);
}

/// A chord along x through the axis/centre, shells `r = 1, 2, 3`, one bin in
/// every angle, origin off zero so the `local_coords` / `get_indices`
/// distinction is exercised. Elements r1, r2, r3 are bins 0, 1, 2.
///
/// Entering r = 3: inward bin 27; pairs at r = 2 (24, 15) and r = 1 (12, **1**);
/// **no** crossing at r = 0 (`find_r_crossing` returns INFTY for a zero
/// radius); out through r = 1 (2, 13), r = 2 (14, 25), leaving through r = 3
/// (26).
///
/// **Upstream behaviour, found by this test (2026-11-22).** `raytrace_mesh`
/// labels an inward current with `!distances[k].max_surface`
/// (`src/mesh.cpp:1393`): the face is inferred from where the track will
/// **leave** the element on that axis, not from the face it came in through.
/// On a Cartesian axis the two are always opposite; on a radial axis they need
/// not be. Entering the core element (r < 1) from r = 1, its next r exit is r = 1
/// again (max), so upstream scores the inward current on the **min** r face
/// (bin `0 + 0 + 0 + 1 = 1`), a face of zero radius. The hand count before the
/// run expected the max face (bin 3); the port reproduces upstream, so the
/// expectation here is upstream's. Same on the spherical mesh. Whether OpenMC
/// itself gives 1 is the pending code-to-code check (V&V README).
const RADIAL_CHORD: [usize; 10] = [27, 24, 15, 12, 1, 2, 13, 14, 25, 26];

#[test]
fn cylindrical_radial_chord_crosses_each_shell_in_and_out() {
    let o = Position::new(10.0, -5.0, 2.0);
    let kind = MeshKind::Cylindrical(CylindricalMesh {
        r_grid: vec![0.0, 1.0, 2.0, 3.0],
        phi_grid: vec![0.0, TAU],
        z_grid: vec![-1.0, 1.0],
        origin: o,
    });
    let a = Position::new(o.x - 4.0, o.y, o.z + 0.25);
    let b = Position::new(o.x + 4.0, o.y, o.z + 0.25);
    let r = kind.surface_bins_crossed_openmc(a, b, dir(a, b)).unwrap();
    assert_eq!(r, RADIAL_CHORD.to_vec());
    // Through the filter, same answer and 12 bins per element.
    let f = MeshSurfaceFilter::new(kind).unwrap();
    assert_eq!(f.n_bins(), 36);
    let ev = FilterEvent { position_last: a, position: b, direction: dir(a, b), ..FilterEvent::default() };
    assert_eq!(f.matches(&ev), RADIAL_CHORD.to_vec());
    assert_eq!(f.get_bin(&ev), Some(27));
}

#[test]
fn spherical_radial_chord_crosses_each_shell_in_and_out() {
    let o = Position::new(-3.0, 7.0, 1.5);
    let kind = MeshKind::Spherical(SphericalMesh {
        r_grid: vec![0.0, 1.0, 2.0, 3.0],
        theta_grid: vec![0.0, PI],
        phi_grid: vec![0.0, TAU],
        origin: o,
    });
    // A diagonal chord through the centre, from |r| = 4 to |r| = 4.
    let s = 4.0 / 3f64.sqrt();
    let a = Position::new(o.x - s, o.y - s, o.z - s);
    let b = Position::new(o.x + s, o.y + s, o.z + s);
    let r = kind.surface_bins_crossed_openmc(a, b, dir(a, b)).unwrap();
    assert_eq!(r, RADIAL_CHORD.to_vec());
}

/// Four phi bins on a cylinder `r <= 2`, `z in [0, 1]`. Elements
/// `(r1, phi j, z1)` are bins `j - 1`.
#[test]
fn cylindrical_phi_crossings_including_the_two_pi_wrap() {
    let kind = MeshKind::Cylindrical(CylindricalMesh {
        r_grid: vec![0.0, 2.0],
        phi_grid: vec![0.0, FRAC_PI_2, PI, 1.5 * PI, TAU],
        z_grid: vec![0.0, 1.0],
        origin: Position::new(0.0, 0.0, 0.0),
    });
    // phi 0.46 (bin 0) -> 2.68 (bin 1), across phi = pi/2 at x = 0, phi
    // increasing: out of bin 0 through its max phi face (4 + 2 = 6), into
    // bin 1 through its min phi face (12 + 4 + 1 = 17).
    let (a, b) = (Position::new(1.0, 0.5, 0.5), Position::new(-1.0, 0.5, 0.5));
    assert_eq!(kind.surface_bins_crossed_openmc(a, b, dir(a, b)).unwrap(), vec![6, 17]);
    // phi 5.82 (bin 3) -> 0.46 (bin 0) across phi = 2 pi == 0: the full grid
    // wraps (sanitize_phi): out of bin 3 max (36 + 4 + 2 = 42), into bin 0
    // min (4 + 1 = 5).
    let (a, b) = (Position::new(1.0, -0.5, 0.5), Position::new(1.0, 0.5, 0.5));
    assert_eq!(kind.surface_bins_crossed_openmc(a, b, dir(a, b)).unwrap(), vec![42, 5]);
    // And back the other way: out of bin 0 through its min phi face (4), into
    // bin 3. Entering bin 3 the track has no further phi exit (both candidate
    // planes return INFTY, `std::min` keeps the first, `max_surface = true`),
    // so upstream labels the inward current `!true` = min face
    // (36 + 4 + 0 + 1 = 41), not the max face it came through (43, the hand
    // count before the run). The same `!distances[k].max_surface` labelling as
    // in RADIAL_CHORD.
    assert_eq!(kind.surface_bins_crossed_openmc(b, a, dir(b, a)).unwrap(), vec![4, 41]);
}

/// Two theta bins (upper and lower hemisphere) on a sphere `r <= 2`; a track
/// along -z at x = 0.5 crosses theta = pi/2 (the plane z = 0) once, theta
/// increasing.
#[test]
fn spherical_theta_crossing() {
    let kind = MeshKind::Spherical(SphericalMesh {
        r_grid: vec![0.0, 2.0],
        theta_grid: vec![0.0, FRAC_PI_2, PI],
        phi_grid: vec![0.0, TAU],
        origin: Position::new(0.0, 0.0, 0.0),
    });
    let (a, b) = (Position::new(0.5, 0.0, 0.5), Position::new(0.5, 0.0, -0.5));
    assert_eq!(kind.surface_bins_crossed_openmc(a, b, dir(a, b)).unwrap(), vec![6, 17]);
    // Back up: out of bin 1 through its min theta face (16), into bin 0.
    // There the track has no further theta exit (the theta = pi/2 cone is
    // behind it; `sanitize_theta(0)` wraps a full theta grid to the theta = pi
    // cone, which a track along +z never crosses), so upstream labels the
    // inward current min (4 + 1 = 5) rather than max (7, the hand count).
    assert_eq!(kind.surface_bins_crossed_openmc(b, a, dir(b, a)).unwrap(), vec![16, 5]);
}

// ---------------------------------------------------------------------------
// 4. Conservation on random tracks.
// ---------------------------------------------------------------------------

fn check_conservation(
    name: &str,
    kind: &MeshKind,
    lo: [f64; 3],
    hi: [f64; 3],
    seed: u64,
    keep_start: impl Fn(Position) -> bool,
) {
    let n_el = kind.n_bins();
    let mut g = Lcg(seed);
    let mut crossings = 0;
    let mut n = 0;
    let mut skipped = 0;
    while n < 5000 {
        let a = g.point(lo, hi);
        let b = g.point(lo, hi);
        if !keep_start(a) {
            skipped += 1;
            continue;
        }
        n += 1;
        let bins = kind.surface_bins_crossed_openmc(a, b, dir(a, b)).unwrap();
        crossings += bins.len();
        let mut net = vec![0i64; n_el];
        for &s in &bins {
            let e = s / 12;
            if s % 2 == 1 {
                net[e] += 1;
            } else {
                net[e] -= 1;
            }
        }
        let mut expect = vec![0i64; n_el];
        if let Some(e) = kind.bin(b) {
            expect[e] += 1;
        }
        if let Some(e) = kind.bin(a) {
            expect[e] -= 1;
        }
        assert_eq!(net, expect, "{name} track {n}: {a:?} -> {b:?}: {bins:?}");
        // Each inward current is into the element the next outward one leaves.
        let mut last_in: Option<usize> = None;
        for &s in &bins {
            if s % 2 == 1 {
                last_in = Some(s / 12);
            } else if let Some(e) = last_in.take() {
                assert_eq!(e, s / 12, "{name} track {n}: {bins:?}");
            }
        }
    }
    println!("{name}: 5000 random tracks conserve ({skipped} starts excluded), {crossings} surface bins");
    assert!(crossings > 5000, "{name}: the tracks cross the mesh");
}

#[test]
fn currents_are_conserved_element_by_element_on_random_tracks() {
    let o = Position::new(1.0, -2.0, 0.5);
    check_conservation(
        "rectilinear",
        &MeshKind::Rectilinear(RectilinearMesh {
            grid: [vec![-3.0, -1.0, 0.5, 4.0], vec![-2.0, 0.0, 0.7, 3.0], vec![-1.0, 2.0]],
        }),
        [-5.0, -4.0, -3.0],
        [6.0, 5.0, 4.0],
        0x1712_0004,
        |_| true,
    );
    check_conservation(
        "cylindrical (full phi, 4 bins)",
        &MeshKind::Cylindrical(CylindricalMesh {
            r_grid: vec![0.0, 0.8, 1.7, 3.0],
            phi_grid: vec![0.0, 1.0, 2.5, 4.0, TAU],
            z_grid: vec![-2.0, 0.0, 1.0, 2.5],
            origin: o,
        }),
        [o.x - 4.0, o.y - 4.0, o.z - 3.0],
        [o.x + 4.0, o.y + 4.0, o.z + 3.5],
        0x1712_0005,
        |_| true,
    );
    check_conservation(
        "cylindrical (partial phi, r_min > 0)",
        &MeshKind::Cylindrical(CylindricalMesh {
            r_grid: vec![0.5, 1.5, 3.0],
            phi_grid: vec![0.3, 1.2, 2.0],
            z_grid: vec![-1.0, 1.0],
            origin: o,
        }),
        [o.x - 4.0, o.y - 4.0, o.z - 2.0],
        [o.x + 4.0, o.y + 4.0, o.z + 2.0],
        0x1712_0006,
        // Upstream defect (see `partial_phi_track_starting_above_phi_max_is_lost`):
        // a start whose phi lies above the last phi plane is excluded here.
        |p| local_phi(p, o) <= 2.0,
    );
    check_conservation(
        "spherical (full theta and phi)",
        &MeshKind::Spherical(SphericalMesh {
            r_grid: vec![0.0, 1.0, 2.2, 3.0],
            // Not pi/2: see `theta_half_pi_cone_crossing_is_lost_to_round_off`.
            theta_grid: vec![0.0, 0.7, 1.45, 2.4, PI],
            phi_grid: vec![0.0, 2.0, 4.5, TAU],
            origin: o,
        }),
        [o.x - 4.0, o.y - 4.0, o.z - 4.0],
        [o.x + 4.0, o.y + 4.0, o.z + 4.0],
        0x1712_0007,
        |_| true,
    );
}

fn local_phi(p: Position, o: Position) -> f64 {
    let a = (p.y - o.y).atan2(p.x - o.x);
    if a < 0.0 {
        a + TAU
    } else {
        a
    }
}

/// **Upstream behaviour, found by the conservation check (2026-11-22).** On a
/// cylindrical mesh whose phi grid does not cover `[0, 2 pi]`, a track that
/// **starts** at a phi above the last phi plane and enters the mesh scores
/// **nothing**. `CylindricalMesh::get_indices` (`src/mesh.cpp:1999`) sanitises
/// the out-of-range phi index `N + 1` to 0, so `distance_to_grid_boundary`
/// looks for the plane `phi_grid[0]` instead of `phi_grid[N]`, finds no
/// crossing (INFTY), and the outside branch of `raytrace_mesh` then travels
/// the INFTY distance and returns. A track starting below the first plane
/// (index 0, sanitised to 0) is unaffected. Ported as is (porting rule), and
/// pinned here so a deliberate fix has to change this test; whether OpenMC
/// reproduces it is part of the pending code-to-code check.
#[test]
fn partial_phi_track_starting_above_phi_max_is_lost() {
    let o = Position::new(1.0, -2.0, 0.5);
    let kind = MeshKind::Cylindrical(CylindricalMesh {
        r_grid: vec![0.5, 1.5, 3.0],
        phi_grid: vec![0.3, 1.2, 2.0],
        z_grid: vec![-1.0, 1.0],
        origin: o,
    });
    let a = Position::new(-1.6363668610690913, 0.5994044702671486, 0.03602396035287869);
    let b = Position::new(1.5923890705485402, 0.7029171911665149, 0.47554377882648957);
    assert!(local_phi(a, o) > 2.0);
    assert_eq!(kind.bin(a), None);
    assert!(kind.bin(b).is_some(), "the track ends inside the mesh");
    assert_eq!(kind.surface_bins_crossed_openmc(a, b, dir(a, b)).unwrap(), Vec::<usize>::new());
    // Reversed (starting inside), the exit through phi = 2.0 is scored.
    assert!(!kind.surface_bins_crossed_openmc(b, a, dir(b, a)).unwrap().is_empty());
}

/// **Upstream behaviour, found by the conservation check (2026-11-22).** A
/// theta grid edge at exactly `pi/2` is the plane `z = 0`, a degenerate cone:
/// `find_theta_crossing` (`src/mesh.cpp:2363`) solves `a s^2 + 2 b s + c = 0`
/// with a **double** root, so its discriminant `D = p^2 - c/a` is zero in exact
/// arithmetic and its sign is decided by round-off. On this track
/// `D = -3.55e-15` (recomputed in Python with the same operations), so upstream
/// returns INFTY and the `z = 0` crossing at `s = 3.3521` is **not scored**:
/// the walker ends in theta bin 3 while the end point is in theta bin 2. Ported
/// as is (porting rule) and pinned so a deliberate fix has to change this test.
/// A pi/2 theta edge is common in practice (upper/lower hemisphere), so this
/// is worth raising upstream; the pending OpenMC comparison will show whether
/// OpenMC's `libm` rounds the same way.
#[test]
fn theta_half_pi_cone_crossing_is_lost_to_round_off() {
    let o = Position::new(1.0, -2.0, 0.5);
    let kind = MeshKind::Spherical(SphericalMesh {
        r_grid: vec![0.0, 1.0, 2.2, 3.0],
        theta_grid: vec![0.0, 0.7, FRAC_PI_2, 2.4, PI],
        phi_grid: vec![0.0, 2.0, 4.5, TAU],
        origin: o,
    });
    let a = Position::new(0.554283833126739, 0.49440571967109914, -1.7032555740322852);
    let b = Position::new(2.2844714158658492, -3.526205884796224, 2.1140405103515496);
    let bins = kind.surface_bins_crossed_openmc(a, b, dir(a, b)).unwrap();
    assert_eq!(bins, vec![99, 96, 87, 84, 73, 80, 369, 362, 373, 374, 385]);
    // No theta-axis (4..8) bin anywhere, although z changes sign along the track.
    assert!(bins.iter().all(|s| !(4..8).contains(&(s % 12))), "{bins:?}");
    assert_eq!(kind.bin(b), Some(29));
    assert_eq!(*bins.last().unwrap() / 12, 32, "the walker believes it ends in theta bin 3");
}
