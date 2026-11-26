// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/regimeMapModels/
//             twoParameters/{twoParameters,regimeDomain2D,regimeBoundary2D}.{H,C}
//             regimeMapModel.{H,C}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//   Also translated (public-domain / OpenFOAM-ESI v2506): `Foam::Random` /
//   `Foam::Rand48` (the ray of a concave domain), `Foam::Hasher` (Bob Jenkins'
//   lookup3 `hashlittle`) and `HashTable` iteration order (the order in which
//   upstream's `regimeDomains_` table is walked).
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `interfacial::regime_map_2d` — the `twoParameters` flow-regime map (gh:#1225)
//!
//! A translation of GeN-Foam's `regimeMapModels::twoParameters` and its two
//! helper classes `regimeDomain2D` / `regimeBoundary2D`. A point
//! `(parameter1, parameter2)` is classified against named polygons in a 2-D
//! parameter space; within `interpolationWidth` (in the space normalised by the
//! overall extent) of a boundary shared with another polygon the cell is a
//! blend of the two regimes, weighted `c` and `1 - c` with `c = 0.5 (d + 1)`
//! (linear) or `1 - 0.5 (d - 1)^2` (quadratic), `d = distance / width`.
//!
//! ## What is reproduced exactly, and why it matters
//!
//! * **Walk order of `regimeDomains_`.** Upstream stores the polygons in a
//!   `HashTable<regimeDomain2D, word>` and finds a point's regime by walking
//!   that table and taking the FIRST polygon that contains the point
//!   (`containsPoint` is boundary-inclusive for convex polygons, so a point on
//!   a shared edge is claimed by whichever polygon the walk meets first). The
//!   walk order is the bucket order of Jenkins' `hashlittle` of the regime
//!   name in a 128-bucket table, newest-first within a bucket
//!   ([`hash_table_walk_order`]). Insertion order is NOT the walk order.
//! * **Polygon sign and ray taken from the UNNORMALISED points.** `sign_` and
//!   `rayNorm_` are computed in the constructor, before `p[0] /= DX_` and
//!   `p[1] /= DY_`, and `correct()` afterwards recomputes only `v_`, `vNorm_`,
//!   `vNormPerp_` and `len_`.
//! * **The ray of a concave polygon** is drawn from `Foam::Random(69 (id+1917))`
//!   (a `drand48` generator through libstdc++'s `generate_canonical`), a fresh
//!   ray per boundary, and the LAST ray drawn is the one kept
//!   ([`Rand48`]).
//!
//! `Info <<` debug output of upstream is not reproduced.
//!
//! Evaluated per cell with [`RegimeMap2D::regime_weights`]; a point outside
//! every polygon is upstream's `FatalError` and is returned as `Err`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use super::regime_map::RegimeInterpolationMode;
#[allow(unused_imports)]
use petir::real::Real;

/// Bob Jenkins' `lookup3` `hashlittle` over bytes, as `Foam::Hasher(data, len, seed)`
/// (`Hasher.C`, little-endian byte-wise branch).
#[must_use]
pub fn hash_little(key: &[u8], seed: u32) -> u32 {
    fn rot(x: u32, n: u32) -> u32 {
        x.rotate_left(n)
    }
    fn mix(a: &mut u32, b: &mut u32, c: &mut u32) {
        *a = a.wrapping_sub(*c);
        *a ^= rot(*c, 4);
        *c = c.wrapping_add(*b);
        *b = b.wrapping_sub(*a);
        *b ^= rot(*a, 6);
        *a = a.wrapping_add(*c);
        *c = c.wrapping_sub(*b);
        *c ^= rot(*b, 8);
        *b = b.wrapping_add(*a);
        *a = a.wrapping_sub(*c);
        *a ^= rot(*c, 16);
        *c = c.wrapping_add(*b);
        *b = b.wrapping_sub(*a);
        *b ^= rot(*a, 19);
        *a = a.wrapping_add(*c);
        *c = c.wrapping_sub(*b);
        *c ^= rot(*b, 4);
        *b = b.wrapping_add(*a);
    }
    fn fin(a: &mut u32, b: &mut u32, c: &mut u32) {
        *c ^= *b;
        *c = c.wrapping_sub(rot(*b, 14));
        *a ^= *c;
        *a = a.wrapping_sub(rot(*c, 11));
        *b ^= *a;
        *b = b.wrapping_sub(rot(*a, 25));
        *c ^= *b;
        *c = c.wrapping_sub(rot(*b, 16));
        *a ^= *c;
        *a = a.wrapping_sub(rot(*c, 4));
        *b ^= *a;
        *b = b.wrapping_sub(rot(*a, 14));
        *c ^= *b;
        *c = c.wrapping_sub(rot(*b, 24));
    }
    let mut length = key.len();
    let init = 0xdead_beef_u32.wrapping_add(length as u32).wrapping_add(seed);
    let (mut a, mut b, mut c) = (init, init, init);
    let mut k = key;
    let w = |k: &[u8], i: usize| -> u32 {
        u32::from(k[i]) | (u32::from(k[i + 1]) << 8) | (u32::from(k[i + 2]) << 16) | (u32::from(k[i + 3]) << 24)
    };
    while length > 12 {
        a = a.wrapping_add(w(k, 0));
        b = b.wrapping_add(w(k, 4));
        c = c.wrapping_add(w(k, 8));
        mix(&mut a, &mut b, &mut c);
        length -= 12;
        k = &k[12..];
    }
    if length == 0 {
        return c;
    }
    for i in (0..length).rev() {
        let byte = u32::from(k[i]);
        let sh = ((i % 4) * 8) as u32;
        if i >= 8 {
            c = c.wrapping_add(byte << sh);
        } else if i >= 4 {
            b = b.wrapping_add(byte << sh);
        } else {
            a = a.wrapping_add(byte << sh);
        }
    }
    fin(&mut a, &mut b, &mut c);
    c
}

/// The order in which upstream's `HashTable<_, word>` (default capacity 128, doubling when
/// `0.8 capacity < size`, insertion at the head of a bucket) yields `keys` inserted in the given
/// order: a permutation of `0..keys.len()` (`HashTable.C` `setEntry`, `setCapacity`).
#[must_use]
pub fn hash_table_walk_order(keys: &[String]) -> Vec<usize> {
    let mut capacity: usize = 128;
    let mut table: Vec<Vec<usize>> = vec![Vec::new(); capacity];
    let mut size = 0usize;
    for (n, key) in keys.iter().enumerate() {
        let index = (hash_little(key.as_bytes(), 0) as usize) & (capacity - 1);
        // insert at the head: the head is index 0 of the chain
        table[index].insert(0, n);
        size += 1;
        if 0.8 * (capacity as f64) < size as f64 {
            let new_capacity = 2 * capacity;
            let mut new_table: Vec<Vec<usize>> = vec![Vec::new(); new_capacity];
            for chain in &table {
                for &m in chain {
                    let j = (hash_little(keys[m].as_bytes(), 0) as usize) & (new_capacity - 1);
                    new_table[j].insert(0, m);
                }
            }
            table = new_table;
            capacity = new_capacity;
        }
    }
    table.into_iter().flatten().collect()
}

/// `Foam::Rand48` (a `drand48`-compatible `std::linear_congruential_engine`) with `Foam::Random`'s
/// `std::uniform_real_distribution<double>` on top (libstdc++ `generate_canonical<double, 53>`).
#[derive(Debug, Clone)]
pub struct Rand48 {
    x: u64,
}

impl Rand48 {
    /// `Random(seed)`: `Rand48(uint32 seed)` seeds the engine with `(seed << 16) | 0x330e`.
    #[must_use]
    pub fn new(seed: u32) -> Self {
        Self { x: ((u64::from(seed)) << 16) | 0x330e }
    }

    fn next_u31(&mut self) -> u64 {
        self.x = (0x5DEE_CE66D_u64.wrapping_mul(self.x).wrapping_add(0xB)) & ((1u64 << 48) - 1);
        self.x >> 17
    }

    /// `Random::sample01<scalar>()`: two 31-bit draws, `(u1 + u2 2^31) / 2^62` rounded once
    /// (`k = ceil(53 / 31) = 2` in `generate_canonical`).
    pub fn sample01(&mut self) -> f64 {
        let u1 = self.next_u31() as f64;
        let u2 = self.next_u31() as f64;
        let sum = 0.0 + u1 * 1.0;
        let sum = sum + u2 * 2_147_483_648.0;
        let tmp = 2_147_483_648.0 * 2_147_483_648.0;
        let ret = sum / tmp;
        if ret >= 1.0 {
            return 1.0 - f64::EPSILON / 2.0;
        }
        ret
    }
}

type V2 = [f64; 2];

fn sub(a: V2, b: V2) -> V2 {
    [a[0] - b[0], a[1] - b[1]]
}

fn dot(a: V2, b: V2) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn mag(a: V2) -> f64 {
    (a[0] * a[0] + a[1] * a[1]).sqrt()
}

/// `Vector2D::normalise(tol = ROOTVSMALL = 1e-150)`.
fn normalise(a: V2) -> V2 {
    let s = mag(a);
    if s < 1e-150 {
        [0.0, 0.0]
    } else {
        [a[0] / s, a[1] / s]
    }
}

/// `regimeBoundary2D`: the segment `p0 -> p1` of a polygon and the polygon on its other side.
#[derive(Debug, Clone)]
struct Boundary {
    p0: V2,
    p1: V2,
    v: V2,
    v_norm: V2,
    len: f64,
    nbr: Option<usize>,
}

impl Boundary {
    fn new(p0: V2, p1: V2) -> Self {
        let mut b = Self { p0, p1, v: [0.0; 2], v_norm: [0.0; 2], len: 0.0, nbr: None };
        b.correct();
        b
    }

    /// `regimeBoundary2D::correct`.
    fn correct(&mut self) {
        self.v = sub(self.p1, self.p0);
        self.v_norm = normalise(sub(self.p1, self.p0));
        self.len = mag(self.v);
    }

    /// `regimeBoundary2D::distanceTo`.
    fn distance_to(&self, p: V2) -> f64 {
        let v0p = sub(p, self.p0);
        let par = dot(v0p, self.v_norm);
        if par < 0.0 {
            return mag(v0p);
        }
        let v1p = sub(p, self.p1);
        if dot(v1p, self.v_norm) > 0.0 {
            return mag(v1p);
        }
        mag([v0p[0] - par * self.v_norm[0], v0p[1] - par * self.v_norm[1]])
    }

    /// `regimeBoundary2D::operator==`: same segment, in either direction.
    fn same_segment(&self, o: &Self) -> bool {
        (self.p0 == o.p0 && self.p1 == o.p1) || (self.p0 == o.p1 && self.p1 == o.p0)
    }
}

/// `regimeDomain2D`.
#[derive(Debug, Clone)]
pub(crate) struct Domain {
    points: Vec<V2>,
    boundaries: Vec<Boundary>,
    /// Convex counter-clockwise `1`, clockwise `-1`, concave `0`.
    sign: i32,
    ray: V2,
}

impl Domain {
    /// The constructor of `regimeDomain2D(id, pointPtrs)`, on the points as given (unnormalised).
    pub(crate) fn new(id: usize, points: Vec<V2>) -> Self {
        let n = points.len();
        let mut boundaries = Vec::new();
        for i in 0..n {
            let (p0, p1) = if i < n - 1 { (points[i], points[i + 1]) } else { (points[n - 1], points[0]) };
            boundaries.push(Boundary::new(p0, p1));
        }
        let mut sign_ = 0i32;
        for i in 0..boundaries.len() {
            let b0 = &boundaries[i];
            let b1 = if i == boundaries.len() - 1 { &boundaries[0] } else { &boundaries[i + 1] };
            let (v0, v1) = (b0.v_norm, b1.v_norm);
            let x = v0[0] * v1[1] - v0[1] * v1[0];
            // C++ `int(x/max(mag(x), 1e-9))`: truncation towards zero.
            let sign = (x / x.abs().max(1e-9)) as i32;
            if i == 0 {
                sign_ = if sign >= 0 { 1 } else { -1 };
            } else if sign_ * sign < 0 {
                sign_ = 0;
                break;
            }
        }
        // `Random rng(-(1922-1991)*(id+1917))`
        let mut rng = Rand48::new((69 * (id as i64 + 1917)) as u32);
        let mut ray = [0.0, 0.0];
        let mut flag = true;
        while flag {
            for b in &boundaries {
                // `Vector2D<scalar>(rng.sample01(), rng.sample01())`: GCC evaluates the arguments
                // right to left, so the FIRST draw is `y` (measured with a g++ -O3 probe of the same
                // constructor shape: draws 0.0443788.. then 0.7036560.. gave x = 0.7036.., y = 0.0443..).
                let y = rng.sample01();
                let x = rng.sample01();
                ray = normalise([x, y]);
                let d = dot(ray, b.v_norm).abs();
                let tol = 1e-2;
                if d < tol || d > 1.0 - tol {
                    flag = true;
                    break;
                }
                flag = false;
            }
        }
        Self { points, boundaries, sign: sign_, ray }
    }

    /// Convex counter-clockwise `1`, clockwise `-1`, concave `0`.
    pub(crate) fn sign(&self) -> i32 {
        self.sign
    }

    /// `regimeDomain2D::boundingBox`: `(min, max)`.
    fn bounding_box(&self) -> (V2, V2) {
        let p0 = self.points[0];
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (p0[0], p0[1], p0[0], p0[1]);
        for p in &self.points {
            min_x = min_x.min(p[0]);
            max_x = max_x.max(p[0]);
            min_y = min_y.min(p[1]);
            max_y = max_y.max(p[1]);
        }
        ([min_x, min_y], [max_x, max_y])
    }

    /// `regimeDomain2D::containsPoint`.
    pub(crate) fn contains_point(&self, p: V2) -> bool {
        if self.sign != 0 {
            self.convex_contains_point(p)
        } else {
            self.concave_contains_point(p)
        }
    }

    fn convex_contains_point(&self, p: V2) -> bool {
        for b in &self.boundaries {
            let v01 = b.v;
            let v0p = sub(p, b.p0);
            let x = v01[0] * v0p[1] - v01[1] * v0p[0];
            if f64::from(self.sign) * x < 0.0 {
                return false;
            }
        }
        true
    }

    fn concave_contains_point(&self, p: V2) -> bool {
        let mut n = 0i64;
        for b in &self.boundaries {
            let v01 = b.v;
            let vp0 = sub(b.p0, p);
            let vp1 = sub(b.p1, p);
            let vxvp0 = self.ray[0] * vp0[1] - self.ray[1] * vp0[0];
            let vxvp1 = self.ray[0] * vp1[1] - self.ray[1] * vp1[0];
            let vp0xv01 = vp0[0] * v01[1] - vp0[1] * v01[0];
            if (vp0xv01 > 0.0 && vxvp0 <= 0.0 && vxvp1 > 0.0) || (vp0xv01 < 0.0 && vxvp0 >= 0.0 && vxvp1 < 0.0) {
                n += 1;
            }
        }
        n % 2 == 1
    }
}

/// The `twoParameters` regime map.
#[derive(Debug, Clone)]
pub struct RegimeMap2D {
    /// Regime names in label order (the order of the `regimes` dictionary).
    names: Vec<String>,
    domains: Vec<Domain>,
    /// Indices into `domains` in the order upstream walks `regimeDomains_`.
    walk_order: Vec<usize>,
    dx: f64,
    dy: f64,
    interpolation_width: f64,
    mode: RegimeInterpolationMode,
}

impl RegimeMap2D {
    /// Build the map (`twoParameters::twoParameters`). `points` are the `regimePoints` entries and
    /// `regimes` the `regimes` entries (name and the names of its points), both in dictionary order.
    ///
    /// # Errors
    /// A regime that names an undefined point, or has fewer than three points.
    pub fn new(
        points: &[(String, [f64; 2])],
        regimes: &[(String, Vec<String>)],
        interpolation_width: f64,
        mode: RegimeInterpolationMode,
    ) -> Result<Self, String> {
        let lookup = |name: &str| -> Result<V2, String> {
            points
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, p)| *p)
                .ok_or_else(|| format!("regime point `{name}` is not defined in regimePoints"))
        };
        let mut names = Vec::new();
        let mut domains = Vec::new();
        for (id, (name, pts)) in regimes.iter().enumerate() {
            if pts.len() < 3 {
                return Err(format!("regime `{name}` has fewer than three points"));
            }
            let mut v = Vec::new();
            for p in pts {
                v.push(lookup(p)?);
            }
            names.push(name.clone());
            domains.push(Domain::new(id, v));
        }
        let walk_order = hash_table_walk_order(&names);
        // boundary owners and neighbours: loops in walk order, as upstream
        for &i0 in &walk_order {
            for &i1 in &walk_order {
                if i0 == i1 {
                    continue;
                }
                for i in 0..domains[i0].boundaries.len() {
                    for j in 0..domains[i1].boundaries.len() {
                        if domains[i0].boundaries[i].same_segment(&domains[i1].boundaries[j]) {
                            domains[i0].boundaries[i].nbr = Some(i1);
                            domains[i1].boundaries[j].nbr = Some(i0);
                            break;
                        }
                    }
                }
            }
        }
        // scale factors from the bounding boxes
        let (bmin, bmax) = domains[0].bounding_box();
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (bmin[0], bmin[1], bmax[0], bmax[1]);
        for &i in &walk_order {
            let (lo, hi) = domains[i].bounding_box();
            min_x = min_x.min(lo[0]);
            min_y = min_y.min(lo[1]);
            max_x = max_x.max(hi[0]);
            max_y = max_y.max(hi[1]);
        }
        let dx = max_x - min_x;
        let dy = max_y - min_y;
        // normalise the points, then re-calculate the boundary members
        for d in &mut domains {
            for p in &mut d.points {
                p[0] /= dx;
                p[1] /= dy;
            }
            for b in &mut d.boundaries {
                b.p0[0] /= dx;
                b.p0[1] /= dy;
                b.p1[0] /= dx;
                b.p1[1] /= dy;
                b.correct();
            }
        }
        Ok(Self { names, domains, walk_order, dx, dy, interpolation_width, mode })
    }

    /// Regime names in label order.
    #[must_use]
    pub fn regime_names(&self) -> &[String] {
        &self.names
    }

    /// The names in the order upstream walks its regime table (see the module doc).
    #[must_use]
    pub fn walk_order_names(&self) -> Vec<&str> {
        self.walk_order.iter().map(|&i| self.names[i].as_str()).collect()
    }

    /// `twoParameters::correct` for one cell: the `(regime name, coefficient)` list of
    /// `regimeLabelCoeffs_[i]` (one entry, or two inside an interpolation band).
    ///
    /// # Errors
    /// The point lies outside every regime (upstream's `FatalError`).
    pub fn regime_weights(&self, x: f64, y: f64) -> Result<Vec<(String, f64)>, String> {
        let p = [x / self.dx, y / self.dy];
        let mut found: Option<usize> = None;
        for &i in &self.walk_order {
            if self.domains[i].contains_point(p) {
                found = Some(i);
                break;
            }
        }
        let Some(r) = found else {
            return Err(format!("point ({x}, {y}) in parameter space is outside the bounds of the regime map"));
        };
        let mut coeffs = vec![(self.names[r].clone(), 1.0)];
        if self.interpolation_width != 0.0 {
            let d = &self.domains[r];
            let mut distance = 2.0;
            let mut nbr: Option<usize> = None;
            for b in d.boundaries.iter().filter(|b| b.nbr.is_some()) {
                let dj = b.distance_to(p);
                if dj < distance {
                    distance = dj;
                    nbr = b.nbr;
                }
            }
            if let Some(nb) = nbr {
                let dd = distance / self.interpolation_width;
                if dd < 1.0 {
                    let c = match self.mode {
                        RegimeInterpolationMode::Linear => 0.5 * (dd + 1.0),
                        RegimeInterpolationMode::Quadratic => {
                            let s = dd - 1.0;
                            1.0 - 0.5 * (s * s)
                        }
                    };
                    coeffs[0].1 = c;
                    coeffs.push((self.names[nb].clone(), 1.0 - c));
                }
            }
        }
        Ok(coeffs)
    }
}
