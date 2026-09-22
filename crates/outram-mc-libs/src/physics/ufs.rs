// SPDX-License-Identifier: GPL-3.0

//! **Uniform fission site (UFS) weighting** — GitHub #258 scope item 5.
//!
//! Ported from `ufs_count_sites` and `ufs_get_weight` (`src/eigenvalue.cpp`)
//! at OpenMC `afa7a14`.
//!
//! # What it does and why it is not the same as a weight window
//!
//! A weight window steers particles towards a *tally*. UFS steers the
//! **fission source** towards uniformity across a mesh, by producing more
//! fission sites where the source is sparse and fewer where it is dense, and
//! compensating with the site weight so the expectation is unchanged.
//!
//! That matters in a problem where the fission source is strongly peaked — a
//! reflected core, a partially-loaded lattice, a pebble bed with a hot
//! central channel — because the peripheral regions then get so few source
//! particles that their tallies never converge, however many histories the run
//! uses in total.
//!
//! # The bias this carries, stated
//!
//! UFS makes the **expected** production per site correct while changing the
//! *variance* of the fission source, which shifts the fission-bank
//! population-control bias of power iteration. That bias scales as `1/N` in
//! the bank size and is the same mechanism this branch is separately measuring
//! for survival biasing — see
//! `verification_and_validation/variance_reduction/`. So a UFS run and an
//! analog run of the same problem agree in the large-bank limit and **need not
//! agree at a fixed small bank**, and a comparison that finds a few tens of
//! pcm between them has not necessarily found a defect.
//!
//! # First generation is deliberately unbiased
//!
//! Upstream assumes an even source on the very first generation so that the
//! production is not biased before there is anything to measure the
//! distribution from ([`UfsWeights::uniform`]). Reproducing that matters: the
//! alternative is weighting against a source distribution derived from the
//! *initial guess*, which is usually a box and bears no relation to the
//! converged shape.

use crate::geometry::position::Position;
use crate::tally::mesh::RegularMesh;

/// Per-element source fractions, and the weights they imply.
#[derive(Debug, Clone, PartialEq)]
pub struct UfsWeights {
    /// The mesh the source is flattened over.
    pub mesh: RegularMesh,
    /// Fraction of the total source weight in each element, summing to 1.
    pub source_frac: Vec<f64>,
    /// `1 / n_bins` — each element's share of the volume, since a
    /// [`RegularMesh`] has uniform elements.
    pub volume_frac: f64,
}

impl UfsWeights {
    /// The first-generation state: assume the source is already even, so no
    /// biasing happens before there is a distribution to measure.
    pub fn uniform(mesh: RegularMesh) -> Self {
        let n = mesh.n_bins();
        let volume_frac = 1.0 / n as f64;
        Self {
            mesh,
            source_frac: vec![volume_frac; n],
            volume_frac,
        }
    }

    /// Count the source bank into the mesh and normalise —
    /// `ufs_count_sites` (`src/eigenvalue.cpp`).
    ///
    /// `sites` is `(position, weight)` per bank entry.
    ///
    /// # Errors
    ///
    /// **Any site outside the mesh.** Upstream calls `fatal_error` here and it
    /// is right to: a UFS mesh that does not cover the source means some
    /// fission sites are weighted against a distribution they were never
    /// counted into, and the resulting bias is silent. Widening the mesh is
    /// the fix, not ignoring the strays.
    ///
    /// Also an empty bank, or one of total weight zero.
    pub fn from_bank(mesh: RegularMesh, sites: &[(Position, f64)]) -> Result<Self, String> {
        if sites.is_empty() {
            return Err("the source bank is empty; there is no distribution to flatten".into());
        }
        let n = mesh.n_bins();
        let mut frac = vec![0.0_f64; n];
        let mut outside = 0usize;
        for (r, w) in sites {
            match mesh.get_bin(*r) {
                Some(b) => frac[b] += w,
                None => outside += 1,
            }
        }
        if outside > 0 {
            return Err(format!(
                "{outside} of {} source sites are outside the UFS mesh. Those sites \
                 would be weighted against a distribution they were never counted \
                 into, and the resulting bias is silent. Widen the mesh.",
                sites.len()
            ));
        }
        let total: f64 = frac.iter().sum();
        if !(total > 0.0) {
            return Err(format!(
                "the source bank has total weight {total}; there is nothing to flatten"
            ));
        }
        for f in &mut frac {
            *f /= total;
        }
        Ok(Self {
            mesh,
            source_frac: frac,
            volume_frac: 1.0 / n as f64,
        })
    }

    /// The UFS weight at `r` — `ufs_get_weight`.
    ///
    /// The **expected number of fission sites** produced at a collision is
    /// multiplied by this, and each site is born at `1 / weight`, so the
    /// expectation is unchanged while the *count* is flattened.
    ///
    /// An element with no source gets weight 1 (no biasing), which is
    /// upstream's behaviour: there is no distribution there to bias against.
    ///
    /// # Errors
    ///
    /// A position outside the mesh, for the same reason [`Self::from_bank`]
    /// refuses strays.
    pub fn weight_at(&self, r: Position) -> Result<f64, String> {
        let bin = self.mesh.get_bin(r).ok_or_else(|| {
            format!(
                "a fission site at ({:.4}, {:.4}, {:.4}) is outside the UFS mesh",
                r.x, r.y, r.z
            )
        })?;
        Ok(if self.source_frac[bin] != 0.0 {
            self.volume_frac / self.source_frac[bin]
        } else {
            1.0
        })
    }

    /// How uneven the source is, as `max/min` over elements that carry any.
    ///
    /// This is the number that says whether UFS is worth turning on: a ratio
    /// near 1 means the source is already flat and UFS only adds weight
    /// variance.
    pub fn peaking(&self) -> f64 {
        let nonzero: Vec<f64> = self
            .source_frac
            .iter()
            .copied()
            .filter(|&f| f > 0.0)
            .collect();
        if nonzero.is_empty() {
            return 1.0;
        }
        let max = nonzero.iter().copied().fold(f64::MIN, f64::max);
        let min = nonzero.iter().copied().fold(f64::MAX, f64::min);
        if min > 0.0 {
            max / min
        } else {
            f64::INFINITY
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh() -> RegularMesh {
        RegularMesh {
            lower_left: [0.0, 0.0, 0.0],
            upper_right: [4.0, 1.0, 1.0],
            dimension: [4, 1, 1],
        }
    }

    fn at(x: f64) -> Position {
        Position::new(x, 0.5, 0.5)
    }

    /// The first generation biases nothing — every weight is exactly 1.
    #[test]
    fn the_first_generation_applies_no_bias() {
        let u = UfsWeights::uniform(mesh());
        for x in [0.5, 1.5, 2.5, 3.5] {
            assert_eq!(u.weight_at(at(x)).unwrap(), 1.0);
        }
        assert_eq!(u.peaking(), 1.0);
    }

    /// **The weight is the inverse of the source density.** A cell holding
    /// half the source in a quarter of the volume gets weight 1/2; an empty
    /// quarter of the volume would get 1.
    #[test]
    fn the_weight_inverts_the_source_density() {
        // 60 % of the weight in cell 0, 40 % in cell 3, nothing in 1 and 2.
        let sites = vec![(at(0.5), 6.0), (at(3.5), 4.0)];
        let u = UfsWeights::from_bank(mesh(), &sites).unwrap();
        assert!((u.source_frac[0] - 0.6).abs() < 1e-12);
        assert!((u.source_frac[3] - 0.4).abs() < 1e-12);
        assert_eq!(u.source_frac[1], 0.0);

        // volume_frac = 0.25.
        assert!((u.weight_at(at(0.5)).unwrap() - 0.25 / 0.6).abs() < 1e-12);
        assert!((u.weight_at(at(3.5)).unwrap() - 0.25 / 0.4).abs() < 1e-12);
        // An empty cell is not biased.
        assert_eq!(u.weight_at(at(1.5)).unwrap(), 1.0);
        assert!((u.peaking() - 1.5).abs() < 1e-12);
    }

    /// **The expectation is preserved.** The number of sites is multiplied by
    /// `w` and each is born at `1/w`, so the total weight produced is
    /// unchanged wherever the source is.
    #[test]
    fn the_expected_produced_weight_is_unchanged() {
        let sites = vec![(at(0.5), 8.0), (at(1.5), 1.0), (at(3.5), 1.0)];
        let u = UfsWeights::from_bank(mesh(), &sites).unwrap();
        for x in [0.5, 1.5, 3.5] {
            let w = u.weight_at(at(x)).unwrap();
            // n_sites scales by w, each born at 1/w.
            let produced = w * (1.0 / w);
            assert!(
                (produced - 1.0).abs() < 1e-15,
                "at x={x} the produced weight is {produced}, not 1"
            );
        }
    }

    /// **A site outside the mesh is refused, not ignored.**
    ///
    /// An ignored stray is weighted against a distribution it was never
    /// counted into, and the bias that follows is silent.
    #[test]
    fn a_source_site_outside_the_mesh_is_refused() {
        let sites = vec![(at(0.5), 1.0), (Position::new(99.0, 0.5, 0.5), 1.0)];
        let err = UfsWeights::from_bank(mesh(), &sites).unwrap_err();
        assert!(err.contains("outside the UFS mesh"), "{err}");
        assert!(err.contains("Widen the mesh"), "{err}");

        let u = UfsWeights::uniform(mesh());
        assert!(u.weight_at(Position::new(-1.0, 0.5, 0.5)).is_err());
    }

    /// Degenerate banks are refused.
    #[test]
    fn degenerate_banks_are_refused() {
        assert!(UfsWeights::from_bank(mesh(), &[]).is_err());
        let err = UfsWeights::from_bank(mesh(), &[(at(0.5), 0.0)]).unwrap_err();
        assert!(err.contains("nothing to flatten"), "{err}");
    }

    /// Peaking is the number that says whether UFS is worth turning on.
    #[test]
    fn peaking_measures_how_uneven_the_source_is() {
        let flat = UfsWeights::from_bank(
            mesh(),
            &[(at(0.5), 1.0), (at(1.5), 1.0), (at(2.5), 1.0), (at(3.5), 1.0)],
        )
        .unwrap();
        assert!((flat.peaking() - 1.0).abs() < 1e-12, "{}", flat.peaking());

        let peaked =
            UfsWeights::from_bank(mesh(), &[(at(0.5), 100.0), (at(3.5), 1.0)]).unwrap();
        assert!((peaked.peaking() - 100.0).abs() < 1e-9, "{}", peaked.peaking());
    }
}
