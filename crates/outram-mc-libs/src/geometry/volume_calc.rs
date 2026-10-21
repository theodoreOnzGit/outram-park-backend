// SPDX-License-Identifier: GPL-3.0

//! **Stochastic volume calculation** for CSG regions — GitHub #267.
//!
//! Ported from `VolumeCalculation::execute` (`src/volume_calc.cpp:95`) at
//! OpenMC `afa7a14`.
//!
//! # Why this exists
//!
//! This workspace's standing correction on R-Z geometry is: **assert region
//! volumes match before comparing `k`**. Until now there was no instrument in
//! this crate that could perform that assertion for a general CSG region —
//! volumes were analytic (simple shapes only), hand-computed, or trusted.
//!
//! A wrong volume is the signature of a wrong region definition: a mis-signed
//! half-space, a surface that does not close. It is the cheapest general check
//! that a CSG model is the model that was *intended*, and it catches the class
//! of defect GitHub #187 and #185 both describe.
//!
//! # The estimator
//!
//! Sample points uniformly in a bounding box, locate each one, count hits per
//! domain. With `N` samples, `h` hits and a box of volume `V_box`:
//!
//! ```text
//! V     = h / N * V_box
//! sigma = sqrt( V * (V_box - V) / N )
//! ```
//!
//! which is the binomial variance `V_box^2 * f(1-f) / N` with `f = h/N`,
//! rearranged into volume units exactly as upstream writes it
//! (`src/volume_calc.cpp:308`).
//!
//! # Two details that are upstream's and are easy to get wrong
//!
//! 1. **The sampling direction is fixed at `(1,1,1)/sqrt(3)`**, not random
//!    (`:161`). A direction is needed only because `locate` takes one to break
//!    surface-sense ties; randomising it would consume variates and change
//!    nothing. Kept identical so a sample that lands exactly on a surface
//!    breaks the same way in both codes.
//! 2. **A cell is counted at every coordinate level** (`:179`), not just the
//!    leaf. A cell that is filled with a universe still has a volume, and it is
//!    the sum of what is nested inside it.
//!
//! # RNG discipline
//!
//! Each sample gets its **own stream by jump-ahead**:
//! `init_seed(id, STREAM_VOLUME, master)`, upstream `:156`. That is this
//! crate's required discipline, not a stylistic choice — `CLAUDE.md` records
//! (defect `op-rbo`) that per-particle stream independence is what makes a
//! quoted sigma mean anything. Sampling this loop from one running seed would
//! make the estimate irreproducible under any reordering and correlate it with
//! whatever else drew from that seed.
//!
//! # Scope
//!
//! Cell and material domains. **Universe domains are not implemented** and are
//! rejected rather than silently treated as cells. Per-nuclide atom counts
//! (upstream `:295`) are not ported: this crate keeps nuclide bookkeeping in
//! the material layer and nothing here needs it yet.

use crate::geometry::geometry::Geometry;
use crate::geometry::position::{Direction, Position};
use crate::geometry::cell::SurfaceToken;
use crate::rng::lcg::{init_seed, prn};

/// Upstream's volume stream offset (`include/openmc/random_lcg.h:16`).
pub const STREAM_VOLUME: i64 = 3;

/// Which kind of region a volume is wanted for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeDomain {
    /// A cell, counted at **every** coordinate level it appears on.
    Cell,
    /// A material, counted at the leaf.
    Material,
}

/// The box points are sampled in. Must contain the domain; anything outside it
/// is invisible to the estimate, which is why [`VolumeCalculation::execute`]
/// reports the hit fraction so a caller can see a box that is far too large.
#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub lower: Position,
    pub upper: Position,
}

impl BoundingBox {
    /// Volume of the box itself.
    pub fn volume(&self) -> f64 {
        (self.upper.x - self.lower.x)
            * (self.upper.y - self.lower.y)
            * (self.upper.z - self.lower.z)
    }
}

/// One domain's estimated volume.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumeResult {
    /// Domain id as given in the request.
    pub id: usize,
    /// Estimated volume \[cm^3\].
    pub volume: f64,
    /// 1 sigma on [`Self::volume`] \[cm^3\], binomial.
    pub std_dev: f64,
    /// Raw hit count, so a caller can see a domain that was never hit (volume
    /// and sigma are then both exactly zero, which is otherwise
    /// indistinguishable from a correct answer for an empty region).
    pub hits: u64,
}

impl VolumeResult {
    /// Relative error, or `INFINITY` for a domain that was never hit.
    pub fn relative_error(&self) -> f64 {
        if self.volume == 0.0 {
            f64::INFINITY
        } else {
            self.std_dev / self.volume
        }
    }
}

/// A stochastic volume calculation request.
#[derive(Debug, Clone)]
pub struct VolumeCalculation {
    pub domain: VolumeDomain,
    pub ids: Vec<usize>,
    pub box_: BoundingBox,
    pub n_samples: u64,
    pub master_seed: i64,
}

impl VolumeCalculation {
    /// Run the calculation.
    ///
    /// Returns one [`VolumeResult`] per requested id, in the order requested,
    /// plus the overall fraction of samples that landed anywhere in the
    /// geometry — a number close to zero means the bounding box is mostly
    /// empty and the estimate is being paid for in wasted samples.
    pub fn execute(&self, geom: &Geometry) -> (Vec<VolumeResult>, f64) {
        // Upstream's fixed direction (`src/volume_calc.cpp:161`). See the module
        // docs for why it is not random.
        let inv_sqrt3 = 1.0 / 3.0_f64.sqrt();
        let u = Direction::new(inv_sqrt3, inv_sqrt3, inv_sqrt3);

        let d = Position::new(
            self.box_.upper.x - self.box_.lower.x,
            self.box_.upper.y - self.box_.lower.y,
            self.box_.upper.z - self.box_.lower.z,
        );
        let mut hits = vec![0_u64; self.ids.len()];
        let mut located = 0_u64;

        for i in 0..self.n_samples {
            // One independent stream per sample, by jump-ahead.
            let mut seed = init_seed(i as i64, STREAM_VOLUME, self.master_seed);
            let r = Position::new(
                self.box_.lower.x + d.x * prn(&mut seed),
                self.box_.lower.y + d.y * prn(&mut seed),
                self.box_.lower.z + d.z * prn(&mut seed),
            );
            let Some(path) = geom.locate(r, u, SurfaceToken::NONE) else {
                continue; // not in the geometry at all
            };
            located += 1;

            match self.domain {
                VolumeDomain::Material => {
                    if let Some(m) = path.material {
                        if let Some(k) = self.ids.iter().position(|&id| id == m) {
                            hits[k] += 1;
                        }
                    }
                }
                VolumeDomain::Cell => {
                    // Every level, not just the leaf (upstream `:179`). A cell
                    // filled with a universe still has a volume.
                    for level in &path.levels {
                        if let Some(k) = self.ids.iter().position(|&id| id == level.cell) {
                            hits[k] += 1;
                            break;
                        }
                    }
                }
            }
        }

        let v_box = self.box_.volume();
        let n = self.n_samples as f64;
        let results = self
            .ids
            .iter()
            .zip(hits.iter())
            .map(|(&id, &h)| {
                let volume = h as f64 / n * v_box;
                VolumeResult {
                    id,
                    volume,
                    // Upstream `:311`: sqrt(V (V_box - V) / N). Identical to the
                    // binomial V_box^2 f(1-f)/N; kept in upstream's arrangement
                    // so the two can be compared line for line.
                    std_dev: (volume * (v_box - volume) / n).sqrt(),
                    hits: h,
                }
            })
            .collect();
        (results, located as f64 / n)
    }
}
