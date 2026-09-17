// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Radial distribution function `g(r)` — the **structure** of a packed bed, as
//! opposed to its bulk packing fraction.
//!
//! # What this measures and why a packing fraction is not enough
//!
//! A solid fraction `φ` is one number: it says how much of the bed is pebble.
//! Two beds can share a `φ` and be structurally different — one crystallising
//! into ordered layers, one genuinely random — and that difference governs flow
//! resistance, effective conductivity and, for a pebble-bed reactor, the
//! neutron streaming paths. `g(r)` resolves it.
//!
//! `g(r)` is the probability of finding another pebble centre at separation `r`
//! from a given centre, **relative to a uniform random arrangement of the same
//! mean density**. So:
//!
//! - `g(r) = 1` means "as likely as random" — the value `g` tends to at large
//!   `r` in any disordered packing, by construction of the normalisation;
//! - `g(r) = 0` for `r < d` — hard spheres cannot interpenetrate, so the
//!   function is identically zero below one pebble diameter (a nonzero value
//!   there is a bug or an overlap, and this module's tests check exactly that);
//! - a **sharp first peak at `r = d`** is the contact shell: its area is the
//!   mean number of touching neighbours;
//! - the **split second peak**, at `r = √3 d ≈ 1.732 d` and `r = 2 d`, is the
//!   signature of a *random* close packing and is the single most-used
//!   diagnostic for distinguishing it from a crystalline one. In an FCC or HCP
//!   crystal the second-neighbour structure sits at `√2 d ≈ 1.414 d` instead.
//!
//! That last point is why this module exists for the HTR-10 work: the question
//! is whether slow recirculation densifies the bed toward the published filling
//! fraction of 0.61, and *how*. A bed that densifies by crystallising is a
//! different physical claim from one that densifies while staying random, and
//! `φ` alone cannot tell them apart.
//!
//! # The estimator, and why the domain is eroded
//!
//! The normalisation is the whole difficulty. Naively,
//!
//! ```text
//! g(r) = <n(r)> / (rho * 4 * pi * r^2 * dr)
//! ```
//!
//! where `<n(r)>` is the mean number of neighbours in a shell. That is only
//! correct while the shell lies **entirely inside the bed**. Near a wall or the
//! free surface part of the shell is outside, no pebble can be there, and `g`
//! sags below 1 for a purely geometric reason that has nothing to do with
//! structure. In a bed 30 pebbles across, that artefact is large.
//!
//! This module avoids it rather than correcting for it: **only pebbles at least
//! `r_max` from every boundary are used as shell centres** (an erosion of the
//! domain), while *every* pebble remains available as a neighbour. Every shell
//! is then fully enclosed, the normalisation above is exact, and no boundary
//! correction is needed or assumed. The price is fewer centres — for the HTR-10
//! bed at `r_max = 5 d` roughly a third of them — which costs statistics, not
//! correctness. [`Rdf::n_centres`] reports how many were used so a caller can
//! see what it paid.
//!
//! The mean number density `rho` is measured **over the eroded region itself**,
//! so it is the local bulk density the shells actually sample, not a whole-bed
//! average contaminated by the loose free surface.
//!
//! # Backends
//!
//! The kernel is an all-pairs distance histogram — stateless, with no
//! dependence between pairs — which makes it the one part of this crate that
//! genuinely suits a GPU, and the reason [`ComputeType::Gpu`] exists here at
//! all. See [`crate::compute`] for the contrast with the DEM timestep, which
//! does not.
//!
//! The two CPU backends are held to the **identical histogram**: binning is
//! integer counting and integer addition is associative, so a per-thread
//! histogram reduced in any order gives exactly the serial counts. No ordered
//! accumulation is needed here, unlike the DEM force loop.
//!
//! **The GPU backend is not bit-identical, and cannot be.** WGSL has no `f64`,
//! so that kernel computes separations in `f32` while the CPU path uses `f64`.
//! A pair whose separation falls within `f32` rounding of a bin edge may land
//! in a neighbouring bin. The effect is bounded and small — `f32` resolves a
//! 2 m coordinate to ~0.2 µm against a bin width of `d/50 = 1.2 mm` — but it is
//! real, and it is measured rather than asserted in `tests/htr10_rdf.rs`.
//! **Any number quoted in a V&V document comes from the CPU path.**

use crate::compute::ComputeType;
use crate::particle::Vec3;

#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;
#[cfg(target_arch = "wasm32")]
use crate::wasm_par as rayon;
#[cfg(target_arch = "wasm32")]
use crate::wasm_par::prelude::*;

/// The region a bed occupies: a right circular cylinder with its axis along
/// `z`, which is the shape of every bed in this crate (core barrel, laboratory
/// cylinder, lifting cylinder).
///
/// Used only to decide which pebbles are far enough from a boundary to serve as
/// shell centres — see the module docs on erosion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RdfDomain {
    /// Cylinder radius `[m]`, about the `z` axis at `x = y = 0`.
    pub radius: f64,
    /// Lower bound of the occupied height `[m]`.
    pub z_min: f64,
    /// Upper bound of the occupied height `[m]`.
    pub z_max: f64,
}

impl RdfDomain {
    /// A cylinder of `radius` spanning `z_min..z_max`, about the `z` axis.
    #[must_use]
    pub fn cylinder(radius: f64, z_min: f64, z_max: f64) -> Self {
        Self {
            radius,
            z_min,
            z_max,
        }
    }

    /// Infer the domain from the pebbles themselves: the largest cylindrical
    /// radius any centre reaches, and the span of `z`.
    ///
    /// Convenient for a settled bed whose free surface is wherever it ended up.
    /// Prefer [`RdfDomain::cylinder`] with the **known** confining radius when
    /// there is one — a bed that has not quite reached the wall would otherwise
    /// report a radius slightly under the real barrel and erode too little.
    #[must_use]
    pub fn from_positions(centres: &[Vec3]) -> Self {
        let mut radius: f64 = 0.0;
        let (mut z_min, mut z_max) = (f64::INFINITY, f64::NEG_INFINITY);
        for c in centres {
            radius = radius.max((c.x * c.x + c.y * c.y).sqrt());
            z_min = z_min.min(c.z);
            z_max = z_max.max(c.z);
        }
        Self {
            radius,
            z_min,
            z_max,
        }
    }

    /// Whether `p` is at least `margin` `[m]` from the curved wall and from
    /// both ends — i.e. whether a shell of radius `margin` about `p` lies
    /// entirely inside the domain.
    #[must_use]
    pub fn is_interior(&self, p: Vec3, margin: f64) -> bool {
        let r = (p.x * p.x + p.y * p.y).sqrt();
        r <= self.radius - margin && p.z >= self.z_min + margin && p.z <= self.z_max - margin
    }

    /// Volume `[m³]` of the region eroded by `margin` — the region
    /// [`RdfDomain::is_interior`] accepts. Zero if the erosion consumes the
    /// domain.
    #[must_use]
    pub fn eroded_volume(&self, margin: f64) -> f64 {
        let r = (self.radius - margin).max(0.0);
        let h = (self.z_max - self.z_min - 2.0 * margin).max(0.0);
        std::f64::consts::PI * r * r * h
    }
}

/// What to compute: how far out, and at what resolution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RdfSettings {
    /// Largest separation `[m]` to histogram. Also the erosion margin, so
    /// raising it costs shell centres quadratically — `5 d` is a good default
    /// for a packed bed, comfortably past the split second peak at `2 d`.
    pub r_max: f64,
    /// Number of equal-width bins spanning `0..r_max`.
    pub n_bins: usize,
    /// The region the bed occupies.
    pub domain: RdfDomain,
}

impl RdfSettings {
    /// Default settings for a bed of pebble **diameter** `d`: out to `5 d` in
    /// bins of `d / 50`.
    ///
    /// The bin width matters: too coarse and the split second peak merges into
    /// one bump, which is exactly the feature being looked for. `d / 50` puts
    /// ~16 bins between the `√3 d` and `2 d` sub-peaks.
    #[must_use]
    pub fn for_diameter(d: f64, domain: RdfDomain) -> Self {
        Self {
            r_max: 5.0 * d,
            n_bins: 250,
            domain,
        }
    }
}

/// A computed radial distribution function.
#[derive(Debug, Clone, PartialEq)]
pub struct Rdf {
    /// Bin centre separations `[m]`, ascending, `n_bins` long.
    pub r: Vec<f64>,
    /// `g(r)`, dimensionless, `n_bins` long. Tends to 1 at large `r`.
    pub g: Vec<f64>,
    /// Running mean number of neighbours within each bin's outer edge — the
    /// **cumulative coordination number**. `coordination[k]` counts every
    /// neighbour out to `r[k] + dr/2`.
    ///
    /// Read at the first minimum of `g` (just past the contact peak) this is
    /// the contact coordination number, ~6 for a random loose packing and
    /// ~9–10 for a dense one.
    pub coordination: Vec<f64>,
    /// Raw pair counts per bin, before normalisation. Backends must agree on
    /// these **exactly** — see the module docs.
    pub counts: Vec<u64>,
    /// How many pebbles survived the erosion to serve as shell centres.
    pub n_centres: usize,
    /// Mean number density `[1/m³]` over the eroded region, the `rho` of the
    /// normalisation.
    pub number_density: f64,
    /// Bin width `[m]`.
    pub dr: f64,
}

impl Rdf {
    /// The separation `[m]` at which `g` is largest — the contact peak, which
    /// for non-overlapping spheres of diameter `d` must land in the first bin
    /// at or above `d`.
    #[must_use]
    pub fn peak_r(&self) -> f64 {
        let mut best = 0usize;
        for k in 0..self.g.len() {
            if self.g[k] > self.g[best] {
                best = k;
            }
        }
        self.r[best]
    }

    /// Coordination number at the first minimum of `g` after the contact peak
    /// — the conventional definition of "how many neighbours is each pebble
    /// touching".
    ///
    /// Returns `None` if no minimum follows the peak within `r_max`.
    #[must_use]
    pub fn contact_coordination(&self) -> Option<f64> {
        let peak = self.g.iter().enumerate().fold(
            (0usize, f64::NEG_INFINITY),
            |acc, (k, &v)| if v > acc.1 { (k, v) } else { acc },
        ).0;
        let mut k = peak + 1;
        while k + 1 < self.g.len() {
            if self.g[k] <= self.g[k + 1] {
                return Some(self.coordination[k]);
            }
            k += 1;
        }
        None
    }

    /// Render as CSV rows (no header) for the V&V dataset:
    /// `r_m,r_over_d,g,coordination,counts`.
    #[must_use]
    pub fn to_csv_rows(&self, diameter: f64) -> String {
        let mut s = String::with_capacity(self.r.len() * 48);
        for k in 0..self.r.len() {
            s.push_str(&format!(
                "{:.6},{:.4},{:.6},{:.6},{}\n",
                self.r[k],
                self.r[k] / diameter,
                self.g[k],
                self.coordination[k],
                self.counts[k]
            ));
        }
        s
    }
}

/// Compute `g(r)` for a set of pebble centres.
///
/// # Methodology
///
/// See the module docs. In short: pebbles at least `r_max` from every boundary
/// serve as shell centres; every pebble is available as a neighbour; the number
/// density is measured over the eroded region; no boundary correction is
/// applied because none is needed.
///
/// # Arguments
///
/// `centres` are pebble centre positions `[m]`. `settings` fixes the range,
/// resolution and domain. `compute` selects the backend — all of which produce
/// the **same bin counts**, so this choice affects speed only.
///
/// # Panics
///
/// If `settings.n_bins` is zero, or `settings.r_max` is not finite and
/// positive.
///
/// # Returns
///
/// An [`Rdf`] with `n_bins` entries. If the erosion leaves no centres (a bed
/// smaller than `2 r_max` across) every `g` is `NaN` and `n_centres` is 0 —
/// deliberately, rather than silently returning a meaningless curve from an
/// un-eroded domain.
#[must_use]
pub fn radial_distribution(centres: &[Vec3], settings: RdfSettings, compute: ComputeType) -> Rdf {
    assert!(settings.n_bins > 0, "n_bins must be non-zero");
    assert!(
        settings.r_max.is_finite() && settings.r_max > 0.0,
        "r_max must be finite and positive"
    );
    let n_bins = settings.n_bins;
    let dr = settings.r_max / n_bins as f64;

    // --- erosion: which pebbles may serve as shell centres ---
    let margin = settings.r_max;
    let centre_idx: Vec<u32> = centres
        .iter()
        .enumerate()
        .filter(|(_, p)| settings.domain.is_interior(**p, margin))
        .map(|(i, _)| i as u32)
        .collect();

    let eroded_volume = settings.domain.eroded_volume(margin);
    let number_density = if eroded_volume > 0.0 {
        centre_idx.len() as f64 / eroded_volume
    } else {
        0.0
    };

    let counts = match compute {
        ComputeType::CpuSingleThread => histogram_serial(centres, &centre_idx, settings.r_max, dr, n_bins),
        ComputeType::CpuMultiThread(_) => {
            histogram_parallel(centres, &centre_idx, settings.r_max, dr, n_bins, compute.threads())
        }
        ComputeType::Gpu => {
            #[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
            {
                match crate::gpu::rdf_histogram(centres, &centre_idx, settings.r_max, dr, n_bins) {
                    Some(c) => c,
                    // No adapter (headless CI, no Vulkan loader): run the CPU
                    // path rather than erroring, exactly as the MC pillar does.
                    None => histogram_parallel(
                        centres,
                        &centre_idx,
                        settings.r_max,
                        dr,
                        n_bins,
                        ComputeType::CpuMultiThread(crate::compute::ThreadCount::Auto).threads(),
                    ),
                }
            }
            #[cfg(any(target_os = "android", target_arch = "wasm32"))]
            {
                histogram_serial(centres, &centre_idx, settings.r_max, dr, n_bins)
            }
        }
    };

    // --- normalise ---
    let mut r = Vec::with_capacity(n_bins);
    let mut g = Vec::with_capacity(n_bins);
    let mut coordination = Vec::with_capacity(n_bins);
    let mut cumulative = 0u64;
    for k in 0..n_bins {
        let r_lo = k as f64 * dr;
        let r_hi = r_lo + dr;
        r.push(0.5 * (r_lo + r_hi));
        cumulative += counts[k];
        if centre_idx.is_empty() {
            g.push(f64::NAN);
            coordination.push(f64::NAN);
            continue;
        }
        // Exact shell volume, not 4*pi*r^2*dr — at d/50 resolution the
        // difference is small but it is free to be exact, and it removes a
        // systematic bias in the first bins where r^2 varies fastest.
        let shell = 4.0 / 3.0 * std::f64::consts::PI * (r_hi.powi(3) - r_lo.powi(3));
        let expected = number_density * shell * centre_idx.len() as f64;
        g.push(if expected > 0.0 {
            counts[k] as f64 / expected
        } else {
            f64::NAN
        });
        coordination.push(cumulative as f64 / centre_idx.len() as f64);
    }

    Rdf {
        r,
        g,
        coordination,
        counts,
        n_centres: centre_idx.len(),
        number_density,
        dr,
    }
}

/// Scalar all-pairs histogram — the trusted reference the other backends are
/// checked against.
fn histogram_serial(
    centres: &[Vec3],
    centre_idx: &[u32],
    r_max: f64,
    dr: f64,
    n_bins: usize,
) -> Vec<u64> {
    let mut counts = vec![0u64; n_bins];
    let r_max_sq = r_max * r_max;
    for &ci in centre_idx {
        let a = centres[ci as usize];
        for (j, b) in centres.iter().enumerate() {
            if j == ci as usize {
                continue;
            }
            let (dx, dy, dz) = (b.x - a.x, b.y - a.y, b.z - a.z);
            let d2 = dx * dx + dy * dy + dz * dz;
            if d2 >= r_max_sq {
                continue;
            }
            let bin = (d2.sqrt() / dr) as usize;
            if bin < n_bins {
                counts[bin] += 1;
            }
        }
    }
    counts
}

/// Rayon-parallel all-pairs histogram.
///
/// Counting is exact and order-independent — integer addition is associative —
/// so per-thread histograms reduced in any order give **identical** counts to
/// [`histogram_serial`]. This kernel therefore needs none of the ordered-
/// accumulation machinery the DEM force loop does.
fn histogram_parallel(
    centres: &[Vec3],
    centre_idx: &[u32],
    r_max: f64,
    dr: f64,
    n_bins: usize,
    threads: usize,
) -> Vec<u64> {
    let r_max_sq = r_max * r_max;
    let chunk = (centre_idx.len() / (threads * 4)).max(64);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("rayon thread pool");
    let partials: Vec<Vec<u64>> = pool.install(|| {
        centre_idx
            .par_chunks(chunk)
            .map(|chunk| {
                let mut local = vec![0u64; n_bins];
                for &ci in chunk {
                    let a = centres[ci as usize];
                    for (j, b) in centres.iter().enumerate() {
                        if j == ci as usize {
                            continue;
                        }
                        let (dx, dy, dz) = (b.x - a.x, b.y - a.y, b.z - a.z);
                        let d2 = dx * dx + dy * dy + dz * dz;
                        if d2 >= r_max_sq {
                            continue;
                        }
                        let bin = (d2.sqrt() / dr) as usize;
                        if bin < n_bins {
                            local[bin] += 1;
                        }
                    }
                }
                local
            })
            .collect()
    });
    let mut counts = vec![0u64; n_bins];
    for p in partials {
        for (c, v) in counts.iter_mut().zip(p.iter()) {
            *c += v;
        }
    }
    counts
}
