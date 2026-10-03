// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/partpos_average.f90 (subroutine partpos_average)
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Particle-dump averaging (`partpos_average.f90`), and [`GriddedMet`], the
//! slice of `com_mod`'s meteorology that this routine, `plumetraj.f90` and
//! the mixing-ratio branch of `conccalc.f90` read.
//!
//! When `ipout = 3`, FLEXPART dumps each particle's position and a set of
//! met variables **averaged over the output interval**. `partpos_average` is
//! called once per particle per time step and adds the particle's current
//! values to running sums (`part_av_*`, `npart_av` in `com_mod`); the dump
//! divides by the count. The port keeps the sums in a [`ParticleAverages`].
//!
//! # What is interpolated, and how
//!
//! Bilinear in the horizontal on the corner weights `p1..p4`, linear in time
//! between the two fields in memory (`memtime`, `memind`), linear in height
//! between the two model levels bracketing the particle: PV, specific
//! humidity, temperature, `u`, `v` and density on model levels; tropopause
//! height and mixing height on the surface; topography without time
//! interpolation. The arithmetic is transcribed operation for operation.
//!
//! # Upstream quirks reproduced (and where the port refuses)
//!
//! * `ix = xtra1(j)` is an implicit `real(dp) -> integer` conversion, i.e.
//!   truncation toward zero (a position in `(-1, 0)` gives `ix = 0` and a
//!   negative weight, which the port reproduces). There is **no** bounds
//!   check on `ixp = ix + 1`.
//!   The port returns `None` when a corner falls outside the supplied arrays
//!   (upstream would read whatever lies there).
//! * The "north pole fix" `if (jyp >= nymax) jyp = jyp - 1` compares against
//!   the compile-time array bound `nymax`, not the actual grid size `ny`. The
//!   port compares against [`GriddedMet::ny`], which is therefore to be read
//!   as upstream's `nymax`: pass the arrays at their full upstream extent.
//! * The level search `do il=2,nz; if (height(il) > z)` leaves `indz`
//!   **unassigned** when the particle is at or above the top level
//!   `height(nz)`; upstream then uses the value left from the previous call
//!   (or garbage). The port returns `None` instead of guessing.
//! * The energy uses the literal `9.81`, not `par_mod`'s `ga` (equal value),
//!   and `par_mod`'s `cpa`; `(uui**2+vvi**2)/2.` is an integer power, i.e. a
//!   product.
//!
//! # Units
//!
//! FLEXPART's: positions in grid units (`xtra1`, `ytra1`) and m above ground
//! (`ztra1`); `xlon0`, `ylat0`, `dx`, `dy` in degrees; times in s; met fields
//! in their `com_mod` units (PV in pvu, `qv` kg/kg, `tt` K, winds m/s,
//! density kg/m³, heights m).

use crate::flexpart::constants::{CPA, PI180};

/// The meteorological arrays of one grid at the two time levels in memory,
/// as `com_mod` holds them: `oro(0:nxmax-1,0:nymax-1)`,
/// `tropopause`/`hmix(0:nxmax-1,0:nymax-1,1,numwfmem)`, and `pv`, `qv`, `tt`,
/// `uu`, `vv`, `rho(0:nxmax-1,0:nymax-1,nzmax,numwfmem)`.
///
/// Storage: 2-D `[j][i]`, 2-D per slot `[slot][j][i]`, 3-D `[slot][k][j][i]`
/// (`i` fastest); level `k` and memory slot are **0-based** here (upstream's
/// level `k` is `k-1`, slot `m` is `m-1`). Use the `idx*` helpers.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GriddedMet {
    /// Array extent in x (upstream's `nxmax` when passed at full size).
    pub nx: usize,
    /// Array extent in y. The north-pole clamp compares against this, as
    /// upstream compares against `nymax`.
    pub ny: usize,
    /// Number of model levels in use (`nz`).
    pub nz: usize,
    /// Height of each model level above ground, m (`height`), length `nz`.
    pub height: Vec<f64>,
    /// Validity times of the two fields in memory, s (`memtime`).
    pub memtime: [i64; 2],
    /// Storage slot of the earlier and of the later field, 0-based (`memind`).
    pub memind: [usize; 2],
    /// Topography, m (`oro`), `[j][i]`.
    pub oro: Vec<f64>,
    /// Tropopause height, m (`tropopause`), `[slot][j][i]`.
    pub tropopause: Vec<f64>,
    /// Mixing height, m (`hmix`), `[slot][j][i]`.
    pub hmix: Vec<f64>,
    /// Potential vorticity, pvu (`pv`), `[slot][k][j][i]`.
    pub pv: Vec<f64>,
    /// Specific humidity, kg/kg (`qv`), `[slot][k][j][i]`.
    pub qv: Vec<f64>,
    /// Temperature, K (`tt`), `[slot][k][j][i]`.
    pub tt: Vec<f64>,
    /// Zonal wind, m/s (`uu`), `[slot][k][j][i]`.
    pub uu: Vec<f64>,
    /// Meridional wind, m/s (`vv`), `[slot][k][j][i]`.
    pub vv: Vec<f64>,
    /// Air density, kg/m³ (`rho`), `[slot][k][j][i]`.
    pub rho: Vec<f64>,
}

impl GriddedMet {
    /// All-zero fields of the given extent (two memory slots).
    #[must_use]
    pub fn zeros(nx: usize, ny: usize, nz: usize) -> Self {
        let n2 = nx * ny;
        let n3 = 2 * nz * n2;
        Self {
            nx,
            ny,
            nz,
            height: vec![0.0; nz],
            memtime: [0, 0],
            memind: [0, 1],
            oro: vec![0.0; n2],
            tropopause: vec![0.0; 2 * n2],
            hmix: vec![0.0; 2 * n2],
            pv: vec![0.0; n3],
            qv: vec![0.0; n3],
            tt: vec![0.0; n3],
            uu: vec![0.0; n3],
            vv: vec![0.0; n3],
            rho: vec![0.0; n3],
        }
    }

    /// Index into `oro`.
    #[must_use]
    pub fn idx2(&self, i: usize, j: usize) -> usize {
        j * self.nx + i
    }

    /// Index into `tropopause` / `hmix`.
    #[must_use]
    pub fn idx2s(&self, i: usize, j: usize, slot: usize) -> usize {
        (slot * self.ny + j) * self.nx + i
    }

    /// Index into the 3-D fields; `k` and `slot` 0-based.
    #[must_use]
    pub fn idx3(&self, i: usize, j: usize, k: usize, slot: usize) -> usize {
        ((slot * self.nz + k) * self.ny + j) * self.nx + i
    }

    /// The level-bracketing search FLEXPART repeats inline:
    /// `do il=2,nz; if (height(il) > z) then indz=il-1; indzp=il`. Returns the
    /// 0-based `indz` (`indzp = indz + 1`), or `None` where upstream leaves
    /// `indz` unassigned (`z >= height(nz)`).
    #[must_use]
    pub fn level_below(&self, z: f64) -> Option<usize> {
        (1..self.nz)
            .find(|&il| self.height[il] > z)
            .map(|il| il - 1)
    }
}

/// Origin and spacing of the meteorological grid (`xlon0`, `ylat0`, `dx`,
/// `dy` in `com_mod`), degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridGeometry {
    /// Longitude of grid point `ix = 0`, degrees.
    pub xlon0: f64,
    /// Latitude of grid point `jy = 0`, degrees.
    pub ylat0: f64,
    /// Grid spacing in x, degrees.
    pub dx: f64,
    /// Grid spacing in y, degrees.
    pub dy: f64,
}

/// Horizontal corner indices and bilinear weights of a particle, as
/// `plumetraj.f90` and `partpos_average.f90` compute them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Corners {
    pub ix: usize,
    pub jy: usize,
    pub ixp: usize,
    pub jyp: usize,
    pub p1: f64,
    pub p2: f64,
    pub p3: f64,
    pub p4: f64,
}

impl Corners {
    /// `ix=int(xtra1); ... p1=rddx*rddy ...`. `pole_clamp` applies upstream's
    /// `if (jyp >= nymax) jyp=jyp-1` (present in `partpos_average` and
    /// `conccalc`, absent from `plumetraj`). `None` when a corner lies outside
    /// the supplied arrays.
    pub fn new(met: &GriddedMet, xtra1: f64, ytra1: f64, pole_clamp: bool) -> Option<Self> {
        // Fortran int(): truncation toward zero, so -1 < x < 0 gives 0.
        let (ixi, jyi) = (xtra1.trunc(), ytra1.trunc());
        if !(ixi >= 0.0 && jyi >= 0.0) {
            return None;
        }
        let ix = ixi as usize;
        let jy = jyi as usize;
        let ixp = ix + 1;
        let mut jyp = jy + 1;
        let ddx = xtra1 - ix as f64;
        let ddy = ytra1 - jy as f64;
        let rddx = 1.0 - ddx;
        let rddy = 1.0 - ddy;
        let p1 = rddx * rddy;
        let p2 = ddx * rddy;
        let p3 = rddx * ddy;
        let p4 = ddx * ddy;
        if pole_clamp && jyp >= met.ny {
            jyp -= 1;
        }
        if ixp >= met.nx || jyp >= met.ny {
            return None;
        }
        Some(Self {
            ix,
            jy,
            ixp,
            jyp,
            p1,
            p2,
            p3,
            p4,
        })
    }

    /// `p1*f(ix,jy) + p2*f(ixp,jy) + p3*f(ix,jyp) + p4*f(ixp,jyp)` over a
    /// 2-D slice addressed by `at(i, j)`.
    pub fn bilinear<F: Fn(usize, usize) -> f64>(&self, at: F) -> f64 {
        self.p1 * at(self.ix, self.jy)
            + self.p2 * at(self.ixp, self.jy)
            + self.p3 * at(self.ix, self.jyp)
            + self.p4 * at(self.ixp, self.jyp)
    }
}

/// Interpolates a 3-D field in space and time: for both bracketing levels,
/// bilinear at each memory time, then `(f1*dt2 + f2*dt1)*dtt`, then linear in
/// height `(dz1*prof(2) + dz2*prof(1))*dz` — `partpos_average.f90:83-130`,
/// `plumetraj.f90:131-141`.
pub(crate) struct Vertical {
    pub indz: usize,
    pub dz1: f64,
    pub dz2: f64,
    pub dz: f64,
}

impl Vertical {
    pub fn new(met: &GriddedMet, z: f64) -> Option<Self> {
        let indz = met.level_below(z)?;
        let dz1 = z - met.height[indz];
        let dz2 = met.height[indz + 1] - z;
        let dz = 1.0 / (dz1 + dz2);
        Some(Self { indz, dz1, dz2, dz })
    }

    pub fn field(
        &self,
        met: &GriddedMet,
        c: &Corners,
        f: &[f64],
        dt1: f64,
        dt2: f64,
        dtt: f64,
    ) -> f64 {
        let mut prof = [0.0; 2];
        for (n, ind) in [self.indz, self.indz + 1].into_iter().enumerate() {
            let mut v1 = [0.0; 2];
            for (m, v) in v1.iter_mut().enumerate() {
                let slot = met.memind[m];
                *v = c.bilinear(|i, j| f[met.idx3(i, j, ind, slot)]);
            }
            prof[n] = (v1[0] * dt2 + v1[1] * dt1) * dtt;
        }
        (self.dz1 * prof[1] + self.dz2 * prof[0]) * self.dz
    }
}

/// Time interpolation of a surface field (`tropopause`, `hmix`):
/// bilinear at both memory times, then `(f1*dt2 + f2*dt1)*dtt`.
pub(crate) fn surface_in_time(
    met: &GriddedMet,
    c: &Corners,
    f: &[f64],
    dt1: f64,
    dt2: f64,
    dtt: f64,
) -> f64 {
    let mut v = [0.0; 2];
    for (m, vm) in v.iter_mut().enumerate() {
        let slot = met.memind[m];
        *vm = c.bilinear(|i, j| f[met.idx2s(i, j, slot)]);
    }
    (v[0] * dt2 + v[1] * dt1) * dtt
}

/// `dt1`, `dt2`, `dtt` of the time interpolation:
/// `dt1=real(itime-memtime(1)); dt2=real(memtime(2)-itime); dtt=1./(dt1+dt2)`.
pub(crate) fn time_weights(met: &GriddedMet, itime: i64) -> (f64, f64, f64) {
    let dt1 = (itime - met.memtime[0]) as f64;
    let dt2 = (met.memtime[1] - itime) as f64;
    let dtt = 1.0 / (dt1 + dt2);
    (dt1, dt2, dtt)
}

/// One particle's running sums for the averaged particle dump
/// (`npart_av(j)`, `part_av_*(j)` in `com_mod`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ParticleAverages {
    /// Number of samples added (`npart_av`).
    pub count: i64,
    /// Sum of the Cartesian unit-sphere x coordinate (`part_av_cartx`).
    pub cartx: f64,
    /// Sum of the y coordinate (`part_av_carty`).
    pub carty: f64,
    /// Sum of the z coordinate (`part_av_cartz`).
    pub cartz: f64,
    /// Sum of the height above ground, m (`part_av_z`).
    pub z: f64,
    /// Sum of the topography, m (`part_av_topo`).
    pub topo: f64,
    /// Sum of PV, pvu (`part_av_pv`).
    pub pv: f64,
    /// Sum of specific humidity, kg/kg (`part_av_qv`).
    pub qv: f64,
    /// Sum of temperature, K (`part_av_tt`).
    pub tt: f64,
    /// Sum of `u`, m/s (`part_av_uu`).
    pub uu: f64,
    /// Sum of `v`, m/s (`part_av_vv`).
    pub vv: f64,
    /// Sum of density, kg/m³ (`part_av_rho`).
    pub rho: f64,
    /// Sum of tropopause height, m (`part_av_tro`).
    pub tro: f64,
    /// Sum of mixing height, m (`part_av_hmix`).
    pub hmix: f64,
    /// Sum of the specific energy `cp T + g z + L q + (u²+v²)/2`, J/kg
    /// (`part_av_energy`).
    pub energy: f64,
}

/// `partpos_average.f90`: add one particle's current values to its running
/// sums.
///
/// * `itime` — current time, s; `memtime` must bracket it.
/// * `xtra1`, `ytra1` — position in grid units; `ztra1` — height above
///   ground, m.
///
/// Returns `None`, leaving `acc` untouched, where upstream would read an
/// undefined level index (`ztra1 >= height(nz)`) or outside the arrays (see
/// the module doc).
pub fn partpos_average(
    acc: &mut ParticleAverages,
    itime: i64,
    xtra1: f64,
    ytra1: f64,
    ztra1: f64,
    met: &GriddedMet,
    geo: &GridGeometry,
) -> Option<()> {
    let (dt1, dt2, dtt) = time_weights(met, itime);

    let xlon = geo.xlon0 + xtra1 * geo.dx;
    let ylat = geo.ylat0 + ytra1 * geo.dy;

    let c = Corners::new(met, xtra1, ytra1, true)?;
    let topo = c.bilinear(|i, j| met.oro[met.idx2(i, j)]);

    let v = Vertical::new(met, ztra1)?;
    let pvi = v.field(met, &c, &met.pv, dt1, dt2, dtt);
    let qvi = v.field(met, &c, &met.qv, dt1, dt2, dtt);
    let tti = v.field(met, &c, &met.tt, dt1, dt2, dtt);
    let uui = v.field(met, &c, &met.uu, dt1, dt2, dtt);
    let vvi = v.field(met, &c, &met.vv, dt1, dt2, dtt);
    let rhoi = v.field(met, &c, &met.rho, dt1, dt2, dtt);

    let tri = surface_in_time(met, &c, &met.tropopause, dt1, dt2, dtt);
    let hmixi = surface_in_time(met, &c, &met.hmix, dt1, dt2, dtt);

    let energy =
        tti * CPA + (ztra1 + topo) * 9.81 + qvi * 2_501_000.0 + (uui * uui + vvi * vvi) / 2.0;

    acc.count += 1;

    let xlon = xlon * PI180;
    let ylat = ylat * PI180;
    let x = ylat.cos() * xlon.sin();
    let y = -ylat.cos() * xlon.cos();
    let z = ylat.sin();

    acc.cartx += x;
    acc.carty += y;
    acc.cartz += z;
    acc.z += ztra1;
    acc.topo += topo;
    acc.pv += pvi;
    acc.qv += qvi;
    acc.tt += tti;
    acc.uu += uui;
    acc.vv += vvi;
    acc.rho += rhoi;
    acc.tro += tri;
    acc.hmix += hmixi;
    acc.energy += energy;
    Some(())
}
