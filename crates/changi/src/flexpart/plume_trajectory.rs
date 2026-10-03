// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/plumetraj.f90, src/clustering.f90,
//                    src/centerofmass.f90, src/mean_mod.f90 (mean_sp / mean_dp)
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Plume-centroid trajectories and particle clustering (`iout = 4, 5`):
//! for each release point, the centre of mass of its particles on the
//! sphere, mean height, mean topography / mixing height / tropopause / PV,
//! the fractions of particles in the PBL, in the troposphere and with
//! |PV| < 2 pvu, and a five-cluster partition of the particle positions
//! (Dorling et al. 1992, *Atmos. Environ.* 26A, 2575).
//!
//! # Clustering is deterministic
//!
//! `clustering.f90` seeds cluster `j` with the particle `j*n/ncluster`
//! (integer division) — **no random numbers are drawn**, so the port takes
//! no random input. It is a k-means on the sphere with great-circle distance
//! ([`distance2`]) and centroids re-projected from the mean 3-D unit vector;
//! it is ported line for line rather than replaced by a generic k-means,
//! because code-to-code agreement depends on the exact seeds, tie-breaking
//! (strict `<`, so the lowest-index cluster wins a tie) and stopping rule.
//!
//! # Upstream quirks reproduced
//!
//! * `clustering` **returns immediately when `n < ncluster`**, leaving every
//!   output (`xclust`, ..., `rms`, `zrms`) unassigned; `plumetraj` then
//!   writes them anyway, so its record carries the previous release point's
//!   cluster values (observed in the fixture) or stack garbage. The port
//!   returns `None` for the cluster block in that case.
//! * `clustering` converts the caller's `xl`, `yl` to radians and back **in
//!   place**; the round trip `x*pi180/pi180` is not the identity in floating
//!   point, and `plumetraj` then passes the perturbed arrays to
//!   `centerofmass` and `distance`. The port takes `&mut` slices and
//!   reproduces the perturbation.
//! * The returned `rms` and `rmsclust` are distances to the centroids of the
//!   **previous** iteration (computed before the centroids are updated),
//!   while `xclust`, `yclust` are the updated centroids.
//! * The stopping test `abs(rms-rmsold)/rmsold < 0.005` divides by zero when
//!   the previous `rms` was 0 (all particles on their centroids); the NaN/inf
//!   comparison is false and the loop runs all 100 iterations. IEEE in Rust
//!   gives the same.
//! * An empty cluster keeps its previous centroid and gets `rmsclust = 0`,
//!   `zclust = 0`, `fclust = 0`.
//! * `ncl` (the nearest cluster) is a local that persists between particles:
//!   if no cluster is nearer than `10.**10.` km (only possible with NaN
//!   distances) the previous particle's cluster is reused, and the very first
//!   one is undefined. The port carries `ncl` the same way and returns `None`
//!   in the undefined case.
//! * `plumetraj`'s level search leaves `indz` unassigned for a particle at or
//!   above `height(nz)` (stale from the previous particle). The port refuses
//!   ([`PlumeError::AboveTopLevel`]). There is no north-pole clamp here,
//!   unlike `partpos_average`.
//! * `tropocenter = tropocenter + tri + topo` (evaluated left to right), the
//!   PV hemisphere test on `yl > 0` (degrees; the equator counts as south),
//!   `rmsdist = max(rmsdist, 0.)`, and the record's time
//!   `itime - (ireleasestart + ireleaseend)/2` with Fortran integer division
//!   (truncation toward zero; Rust's `/` on `i64` is the same).
//! * `mean` is the single-/double-precision `mean_sp`/`mean_dp` pair (the
//!   same algorithm): the one-pass `xq - xl*xl/n` variance, set to 0 when it
//!   is below `eps = 1e-30`; `n = 1` therefore gives `xs = 0` rather than a
//!   division by zero. The mixed-precision `mean_mixed_*` variants are not
//!   used by `plumetraj` and are not ported.
//!
//! # Units
//!
//! Longitudes/latitudes in degrees (inputs and outputs), heights in m,
//! distances (`rms`, `rmsclust`, `rmsdist`) in km, fractions in percent.

use crate::flexpart::constants::PI180;
use crate::flexpart::geodesy::{distance, distance2};
use crate::flexpart::particle_average::{
    surface_in_time, time_weights, Corners, GridGeometry, GriddedMet, Vertical,
};

/// Number of clusters (`ncluster` in `par_mod.f90`).
pub const NCLUSTER: usize = 5;

/// `centerofmass.f90`: centre of mass on the unit sphere of points given in
/// degrees, re-projected to (longitude, latitude) in degrees.
#[must_use]
pub fn centerofmass(xl: &[f64], yl: &[f64]) -> (f64, f64) {
    let n = xl.len();
    let mut xav = 0.0;
    let mut yav = 0.0;
    let mut zav = 0.0;
    for l in 0..n {
        let xll = xl[l] * PI180;
        let yll = yl[l] * PI180;
        let x = yll.cos() * xll.sin();
        let y = -yll.cos() * xll.cos();
        let z = yll.sin();
        xav += x;
        yav += y;
        zav += z;
    }
    xav /= n as f64;
    yav /= n as f64;
    zav /= n as f64;
    let xcenter = xav.atan2(-yav);
    let ycenter = zav.atan2((xav * xav + yav * yav).sqrt());
    (xcenter / PI180, ycenter / PI180)
}

/// `mean_mod`'s `mean` (`mean_sp` / `mean_dp`): mean and sample standard
/// deviation, one-pass formula, 0 below `eps = 1e-30`. Returns `(xm, xs)`.
#[must_use]
pub fn mean(x: &[f64]) -> (f64, f64) {
    const EPS: f64 = 1.0e-30;
    let number = x.len();
    let mut xl = 0.0;
    let mut xq = 0.0;
    for &v in x {
        xl += v;
        xq += v * v;
    }
    let xm = xl / number as f64;
    let xaux = xq - xl * xl / number as f64;
    let xs = if xaux < EPS {
        0.0
    } else {
        (xaux / (number - 1) as f64).sqrt()
    };
    (xm, xs)
}

/// The outputs of `clustering.f90`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clusters {
    /// Cluster centroid longitudes, degrees.
    pub xclust: [f64; NCLUSTER],
    /// Cluster centroid latitudes, degrees.
    pub yclust: [f64; NCLUSTER],
    /// Mean height of each cluster's members, m.
    pub zclust: [f64; NCLUSTER],
    /// Percentage of particles in each cluster.
    pub fclust: [f64; NCLUSTER],
    /// Total horizontal rms distance, km (to the previous iteration's centroids).
    pub rms: f64,
    /// Per-cluster horizontal rms distance, km.
    pub rmsclust: [f64; NCLUSTER],
    /// Total vertical rms deviation from the cluster mean heights, m.
    pub zrms: f64,
}

/// `clustering.f90`: partition `n` particle positions (degrees) into
/// [`NCLUSTER`] clusters. `xl`, `yl` are converted to radians and back in
/// place, as upstream does (see the module doc). Returns `None` when
/// `n < NCLUSTER` (upstream returns with its outputs unassigned and `xl`, `yl`
/// untouched) or where upstream's `ncl` would be undefined.
pub fn clustering(xl: &mut [f64], yl: &mut [f64], zl: &[f64]) -> Option<Clusters> {
    let n = xl.len();
    if n < NCLUSTER {
        return None;
    }
    let mut rmsold = -5.0;
    let mut nclust = vec![0usize; n];

    for i in 0..n {
        nclust[i] = i;
        xl[i] *= PI180;
        yl[i] *= PI180;
    }

    let mut xclust = [0.0; NCLUSTER];
    let mut yclust = [0.0; NCLUSTER];
    let mut zclust = [0.0; NCLUSTER];
    for j in 0..NCLUSTER {
        zclust[j] = 0.0;
        let seed = (j + 1) * n / NCLUSTER - 1;
        xclust[j] = xl[seed];
        yclust[j] = yl[seed];
    }

    let mut xav = [0.0; NCLUSTER];
    let mut yav = [0.0; NCLUSTER];
    let mut zav = [0.0; NCLUSTER];
    let mut rmsclust = [0.0; NCLUSTER];
    let mut numb = [0usize; NCLUSTER];
    let mut rms = 0.0;
    let mut ncl: Option<usize> = None;

    for l in 1..=100 {
        for i in 0..n {
            let mut distancemin = 1.0e10;
            for j in 0..NCLUSTER {
                let distances = distance2(yl[i], xl[i], yclust[j], xclust[j]);
                if distances < distancemin {
                    distancemin = distances;
                    ncl = Some(j);
                }
            }
            nclust[i] = ncl?;
        }

        for j in 0..NCLUSTER {
            xav[j] = 0.0;
            yav[j] = 0.0;
            zav[j] = 0.0;
            rmsclust[j] = 0.0;
            numb[j] = 0;
        }
        rms = 0.0;

        for i in 0..n {
            let c = nclust[i];
            numb[c] += 1;
            let distances = distance2(yl[i], xl[i], yclust[c], xclust[c]);
            rms += distances * distances;
            rmsclust[c] += distances * distances;
            let x = yl[i].cos() * xl[i].sin();
            let y = -yl[i].cos() * xl[i].cos();
            let z = yl[i].sin();
            xav[c] += x;
            yav[c] += y;
            zav[c] += z;
        }

        rms = (rms / n as f64).sqrt();

        for j in 0..NCLUSTER {
            if numb[j] > 0 {
                rmsclust[j] = (rmsclust[j] / numb[j] as f64).sqrt();
                xav[j] /= numb[j] as f64;
                yav[j] /= numb[j] as f64;
                zav[j] /= numb[j] as f64;
                xclust[j] = xav[j].atan2(-yav[j]);
                yclust[j] = zav[j].atan2((xav[j] * xav[j] + yav[j] * yav[j]).sqrt());
            }
        }

        if l > 1 && (rms - rmsold).abs() / rmsold < 0.005 {
            break;
        }
        rmsold = rms;
    }

    for i in 0..n {
        xl[i] /= PI180;
        yl[i] /= PI180;
        zclust[nclust[i]] += zl[i];
    }

    let mut fclust = [0.0; NCLUSTER];
    for j in 0..NCLUSTER {
        xclust[j] /= PI180;
        yclust[j] /= PI180;
        if numb[j] > 0 {
            zclust[j] /= numb[j] as f64;
        }
        fclust[j] = 100.0 * numb[j] as f64 / n as f64;
    }

    let mut zrms = 0.0;
    for i in 0..n {
        let zdist = zl[i] - zclust[nclust[i]];
        zrms += zdist * zdist;
    }
    if zrms > 0.0 {
        zrms = (zrms / n as f64).sqrt();
    }

    Some(Clusters {
        xclust,
        yclust,
        zclust,
        fclust,
        rms,
        rmsclust,
        zrms,
    })
}

/// One particle as `plumetraj` reads it from `com_mod`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlumeParticle {
    /// Time the particle is at, s (`itra1`); only particles at `itime` count.
    pub itra1: i64,
    /// Release point, 0-based (`npoint - 1`).
    pub release: usize,
    /// Position in grid units (`xtra1`, `ytra1`).
    pub x: f64,
    /// Position in grid units.
    pub y: f64,
    /// Height above ground, m (`ztra1`).
    pub z: f64,
}

/// Start and end of a release, s (`ireleasestart`, `ireleaseend`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReleaseWindow {
    /// `ireleasestart`.
    pub start: i64,
    /// `ireleaseend`.
    pub end: i64,
}

/// One record of `trajectories.txt`, as `plumetraj` writes it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlumeRecord {
    /// Release point, 0-based (upstream prints `j`, 1-based).
    pub release: usize,
    /// `itime - (ireleasestart + ireleaseend)/2`, s (integer division).
    pub time_offset: i64,
    /// Centre-of-mass longitude, degrees.
    pub xcenter: f64,
    /// Centre-of-mass latitude, degrees.
    pub ycenter: f64,
    /// Mean height above sea level, m.
    pub zcenter: f64,
    /// Mean topography, m.
    pub topocenter: f64,
    /// Mean mixing height above ground, m.
    pub hmixcenter: f64,
    /// Mean tropopause height above sea level, m.
    pub tropocenter: f64,
    /// Mean PV, pvu.
    pub pvcenter: f64,
    /// Horizontal rms distance from the centre of mass, km.
    pub rmsdist: f64,
    /// Standard deviation of the height above sea level, m.
    pub zrmsdist: f64,
    /// Percentage of particles below the mixing height.
    pub hmixfract: f64,
    /// Percentage with PV < 2 pvu (north) / PV > -2 pvu (south).
    pub pvfract: f64,
    /// Percentage below the tropopause.
    pub tropofract: f64,
    /// The cluster block (`rms`, `zrms` and the five clusters); `None` when
    /// fewer than [`NCLUSTER`] particles, where upstream writes values it
    /// never assigned.
    pub clusters: Option<Clusters>,
}

/// Why [`plumetraj`] refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlumeError {
    /// A particle is at or above the top model level, where upstream's level
    /// index is unassigned (stale). Index of the particle.
    AboveTopLevel(usize),
    /// A particle's interpolation corners fall outside the met arrays.
    OutsideGrid(usize),
    /// `clustering`'s nearest-cluster index would be undefined.
    UndefinedCluster(usize),
}

/// `plumetraj.f90`: the plume statistics for every release point, at time
/// `itime`. `lage_last` is `lage(nageclass)`: release points whose start is
/// more than that before (or after) `itime` are skipped, as are release
/// points with no particle at `itime` (no record is written for either).
pub fn plumetraj(
    itime: i64,
    particles: &[PlumeParticle],
    releases: &[ReleaseWindow],
    lage_last: i64,
    met: &GriddedMet,
    geo: &GridGeometry,
) -> Result<Vec<PlumeRecord>, PlumeError> {
    let (dt1, dt2, dtt) = time_weights(met, itime);
    let mut records = Vec::new();

    for (j, rel) in releases.iter().enumerate() {
        if (rel.start - itime).abs() > lage_last {
            continue;
        }
        let mut topocenter = 0.0;
        let mut hmixcenter = 0.0;
        let mut hmixfract = 0.0;
        let mut tropocenter = 0.0;
        let mut tropofract = 0.0;
        let mut pvfract = 0.0;
        let mut pvcenter = 0.0;
        let mut rmsdist = 0.0;

        let mut xl = Vec::new();
        let mut yl = Vec::new();
        let mut zl = Vec::new();
        for (i, p) in particles.iter().enumerate() {
            if p.itra1 != itime || p.release != j {
                continue;
            }
            let xln = geo.xlon0 + p.x * geo.dx;
            let yln = geo.ylat0 + p.y * geo.dy;
            let mut zln = p.z;

            let c = Corners::new(met, p.x, p.y, false).ok_or(PlumeError::OutsideGrid(i))?;
            let topo = c.bilinear(|ii, jj| met.oro[met.idx2(ii, jj)]);
            topocenter += topo;

            let v = Vertical::new(met, zln).ok_or(PlumeError::AboveTopLevel(i))?;
            let pvi = v.field(met, &c, &met.pv, dt1, dt2, dtt);
            pvcenter += pvi;
            if yln > 0.0 {
                if pvi < 2.0 {
                    pvfract += 1.0;
                }
            } else if pvi > -2.0 {
                pvfract += 1.0;
            }

            let tri = surface_in_time(met, &c, &met.tropopause, dt1, dt2, dtt);
            let hmixi = surface_in_time(met, &c, &met.hmix, dt1, dt2, dtt);
            if zln < tri {
                tropofract += 1.0;
            }
            tropocenter = tropocenter + tri + topo;
            if zln < hmixi {
                hmixfract += 1.0;
            }
            zln += topo;
            hmixcenter += hmixi;

            xl.push(xln);
            yl.push(yln);
            zl.push(zln);
        }

        let n = xl.len();
        if n == 0 {
            continue;
        }
        let nf = n as f64;
        topocenter /= nf;
        hmixcenter /= nf;
        pvcenter /= nf;
        tropocenter /= nf;
        hmixfract = 100.0 * hmixfract / nf;
        pvfract = 100.0 * pvfract / nf;
        tropofract = 100.0 * tropofract / nf;

        let clusters = if n < NCLUSTER {
            None
        } else {
            Some(clustering(&mut xl, &mut yl, &zl).ok_or(PlumeError::UndefinedCluster(j))?)
        };

        let (xcenter, ycenter) = centerofmass(&xl, &yl);
        let (zcenter, zrmsdist) = mean(&zl);

        for k in 0..n {
            let dist = distance(yl[k], xl[k], ycenter, xcenter);
            rmsdist += dist * dist;
        }
        if rmsdist > 0.0 {
            rmsdist = (rmsdist / nf).sqrt();
        }
        rmsdist = f64::max(rmsdist, 0.0);

        records.push(PlumeRecord {
            release: j,
            time_offset: itime - (rel.start + rel.end) / 2,
            xcenter,
            ycenter,
            zcenter,
            topocenter,
            hmixcenter,
            tropocenter,
            pvcenter,
            rmsdist,
            zrmsdist,
            hmixfract,
            pvfract,
            tropofract,
            clusters,
        });
    }
    Ok(records)
}
