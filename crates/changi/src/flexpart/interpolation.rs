// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/interpol_mod.f90, src/interpol_all.f90,
//                    src/interpol_all_nests.f90, src/interpol_misslev.f90,
//                    src/interpol_misslev_nests.f90, src/interpol_wind.f90,
//                    src/interpol_wind_nests.f90, src/interpol_wind_short.f90,
//                    src/interpol_wind_short_nests.f90, src/interpol_vdep.f90,
//                    src/interpol_vdep_nests.f90
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Interpolation of the gridded meteorology to a particle position: bilinear
//! in the horizontal, linear in height, linear in time between the two wind
//! fields held in memory, plus the sub-grid standard deviation of the corner
//! values that FLEXPART uses for its mesoscale-turbulence term.
//!
//! # Two structs, mirroring upstream's two modules
//!
//! * [`MetFields`] is the slice of `com_mod` these routines read: one grid
//!   (the mother grid or one nest) at the two time levels in memory.
//! * [`Interpolator`] is `interpol_mod`: the per-particle interpolation state
//!   (corner indices, bilinear weights, time weights, vertical profiles and
//!   the "already interpolated" flags). Upstream shares it between
//!   `advance.f90` and the `interpol_*` routines through the module; the port
//!   passes it explicitly, and the routines read and write the same fields in
//!   the same order.
//!
//! # Nests and the polar grids
//!
//! Each `interpol_*_nests.f90` routine is the mother-grid routine reading the
//! nest arrays `uun`, `vvn`, ... instead of `uu`, `vv`, ... . The arithmetic
//! is the same, and only the order in which independent accumulators are
//! updated differs, so one Rust function serves both: pass the nest's
//! [`MetFields`]. The code-to-code test checks each port against **both**
//! upstream versions.
//!
//! On the polar grids (`ngrid < 0`), the horizontal wind is read from the
//! polar-stereographic components `uupol`, `vvpol` instead of `uu`, `vv`;
//! everything else is identical.
//!
//! # Index conventions
//!
//! Grid indices `ix`, `jy` are 0-based, as upstream's `0:nxmax-1` arrays are.
//! **Level indices are 0-based here** (upstream's level `k` is `k-1`), as are
//! the two memory slots (`memind`) and species.
//!
//! # Units
//!
//! FLEXPART's: winds m/s (`ww` in m/s after `verttransform`), heights m,
//! times s, density kg/m³.

/// `eps` of the standard-deviation guards (`eps=1.0e-30`).
const EPS: f64 = 1.0e-30;

/// The meteorological fields of one grid at the two time levels in memory.
///
/// Three-dimensional fields are stored `[slot][k][j][i]` (`i` fastest), two-
/// dimensional ones `[slot][j][i]`, and `vdep` `[slot][species][j][i]`; use
/// [`MetFields::idx3`] / [`MetFields::idx2`] / [`MetFields::idx_vdep`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MetFields {
    /// Grid points in x (`nx`).
    pub nx: usize,
    /// Grid points in y (`ny`).
    pub ny: usize,
    /// Model levels (`nz`).
    pub nz: usize,
    /// Number of species carried in `vdep`.
    pub nspec: usize,
    /// Height of each model level above ground, m (`height`).
    pub height: Vec<f64>,
    /// Validity times of the two fields, s (`memtime`).
    pub memtime: [i64; 2],
    /// Which storage slot holds the earlier and the later field (`memind`).
    pub memind: [usize; 2],
    /// Horizontal wind, m/s.
    pub uu: Vec<f64>,
    /// Horizontal wind, m/s.
    pub vv: Vec<f64>,
    /// Vertical wind, m/s.
    pub ww: Vec<f64>,
    /// Polar-stereographic wind components, m/s (mother grid only).
    pub uupol: Vec<f64>,
    /// Polar-stereographic wind components, m/s (mother grid only).
    pub vvpol: Vec<f64>,
    /// Air density, kg/m³.
    pub rho: Vec<f64>,
    /// Vertical density gradient, kg/m⁴.
    pub drhodz: Vec<f64>,
    /// Temperature, K (read by `get_settling` through `advance`).
    pub tt: Vec<f64>,
    /// Friction velocity, m/s.
    pub ustar: Vec<f64>,
    /// Convective velocity scale, m/s.
    pub wstar: Vec<f64>,
    /// Inverse Obukhov length, 1/m.
    pub oli: Vec<f64>,
    /// Mixing height, m.
    pub hmix: Vec<f64>,
    /// Tropopause height, m.
    pub tropopause: Vec<f64>,
    /// Dry deposition velocity per species, m/s.
    pub vdep: Vec<f64>,
}

impl MetFields {
    /// A grid of the given size with every field zero.
    #[must_use]
    pub fn zeros(nx: usize, ny: usize, nz: usize, nspec: usize) -> Self {
        let n3 = 2 * nz * ny * nx;
        let n2 = 2 * ny * nx;
        Self {
            nx,
            ny,
            nz,
            nspec,
            height: vec![0.0; nz],
            memtime: [0, 0],
            memind: [0, 1],
            uu: vec![0.0; n3],
            vv: vec![0.0; n3],
            ww: vec![0.0; n3],
            uupol: vec![0.0; n3],
            vvpol: vec![0.0; n3],
            rho: vec![0.0; n3],
            drhodz: vec![0.0; n3],
            tt: vec![0.0; n3],
            ustar: vec![0.0; n2],
            wstar: vec![0.0; n2],
            oli: vec![0.0; n2],
            hmix: vec![0.0; n2],
            tropopause: vec![0.0; n2],
            vdep: vec![0.0; 2 * nspec * ny * nx],
        }
    }

    /// Index of `(i, j, k, slot)` in a three-dimensional field.
    #[must_use]
    pub fn idx3(&self, i: usize, j: usize, k: usize, slot: usize) -> usize {
        ((slot * self.nz + k) * self.ny + j) * self.nx + i
    }

    /// Index of `(i, j, slot)` in a two-dimensional field.
    #[must_use]
    pub fn idx2(&self, i: usize, j: usize, slot: usize) -> usize {
        (slot * self.ny + j) * self.nx + i
    }

    /// Index of `(i, j, species, slot)` in `vdep`.
    #[must_use]
    pub fn idx_vdep(&self, i: usize, j: usize, species: usize, slot: usize) -> usize {
        ((slot * self.nspec + species) * self.ny + j) * self.nx + i
    }
}

/// `interpol_mod`: the per-particle interpolation state.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Interpolator {
    /// Lower-left corner of the cell, 0-based.
    pub ix: usize,
    /// Lower-left corner of the cell, 0-based.
    pub jy: usize,
    /// Upper-right corner of the cell.
    pub ixp: usize,
    /// Upper-right corner of the cell.
    pub jyp: usize,
    /// Which grid: `> 0` nest, `0` mother, `-1`/`-2` north/south polar.
    pub ngrid: i32,
    /// Bilinear weights of the four corners.
    pub p1: f64,
    /// Bilinear weights of the four corners.
    pub p2: f64,
    /// Bilinear weights of the four corners.
    pub p3: f64,
    /// Bilinear weights of the four corners.
    pub p4: f64,
    /// Fractional position in the cell.
    pub ddx: f64,
    /// Fractional position in the cell.
    pub ddy: f64,
    /// `1 - ddx`.
    pub rddx: f64,
    /// `1 - ddy`.
    pub rddy: f64,
    /// Time since the earlier field, s.
    pub dt1: f64,
    /// Time to the later field, s.
    pub dt2: f64,
    /// `1 / (dt1 + dt2)`.
    pub dtt: f64,
    /// Level below the particle, 0-based.
    pub indz: usize,
    /// Level above the particle, 0-based.
    pub indzp: usize,
    /// `true` while a species' deposition velocity is still to be
    /// interpolated in this time step.
    pub depoindicator: Vec<bool>,
    /// `true` while a level's profile is still to be interpolated.
    pub indzindicator: Vec<bool>,
    /// Interpolated profiles, per level.
    pub uprof: Vec<f64>,
    /// Interpolated profiles, per level.
    pub vprof: Vec<f64>,
    /// Interpolated profiles, per level.
    pub wprof: Vec<f64>,
    /// Sub-grid standard deviations, per level.
    pub usigprof: Vec<f64>,
    /// Sub-grid standard deviations, per level.
    pub vsigprof: Vec<f64>,
    /// Sub-grid standard deviations, per level.
    pub wsigprof: Vec<f64>,
    /// Density and its gradient, per level.
    pub rhoprof: Vec<f64>,
    /// Density and its gradient, per level.
    pub rhogradprof: Vec<f64>,
    /// Wind at the particle, m/s.
    pub u: f64,
    /// Wind at the particle, m/s.
    pub v: f64,
    /// Wind at the particle, m/s.
    pub w: f64,
    /// Sub-grid standard deviations at the particle, m/s.
    pub usig: f64,
    /// Sub-grid standard deviations at the particle, m/s.
    pub vsig: f64,
    /// Sub-grid standard deviations at the particle, m/s.
    pub wsig: f64,
}

/// Surface scales interpolated by [`Interpolator::interpol_all`] into
/// upstream's `hanna_mod`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceScales {
    /// Friction velocity, m/s.
    pub ust: f64,
    /// Convective velocity scale, m/s.
    pub wst: f64,
    /// Obukhov length, m (`99999` where the interpolated `1/L` is exactly 0).
    pub ol: f64,
}

/// The horizontal-wind arrays grid `ngrid` reads: polar-stereographic
/// components on the polar grids (`ngrid < 0`).
fn horizontal(ngrid: i32, met: &MetFields) -> (&[f64], &[f64]) {
    if ngrid < 0 {
        (&met.uupol, &met.vvpol)
    } else {
        (&met.uu, &met.vv)
    }
}

/// `sum_{4 corners} f` and `sum f^2`, accumulated in upstream's order.
fn corner_sums(f: &[f64], c: [usize; 4], sl: &mut f64, sq: &mut f64) {
    *sl = *sl + f[c[0]] + f[c[1]] + f[c[2]] + f[c[3]];
    *sq = *sq + f[c[0]] * f[c[0]] + f[c[1]] * f[c[1]] + f[c[2]] * f[c[2]] + f[c[3]] * f[c[3]];
}

/// Sub-grid standard deviation from a sum and a sum of squares over `n`
/// values: `sqrt((sq - sl^2/n) / (n-1))`, zero below `1e-30`.
fn subgrid_sigma(sl: f64, sq: f64, n: f64, nm1: f64) -> f64 {
    let xaux = sq - sl * sl / n;
    if xaux < EPS {
        0.0
    } else {
        (xaux / nm1).sqrt()
    }
}

impl Interpolator {
    /// A state for a grid with `nz` levels and `nspec` species, as upstream's
    /// module starts: everything zero and every flag **cleared**.
    ///
    /// `interpol_mod` has no initialiser, so upstream relies on static storage
    /// starting zeroed (`.false.`). That matters: `advance.f90` only raises
    /// `indzindicator` for levels `1..nmixz`, so a level above `nmixz` keeps
    /// whatever profile `interpol_all` last left there, possibly from another
    /// particle. Starting the flags raised would re-interpolate those levels
    /// and depart from upstream.
    #[must_use]
    pub fn new(nz: usize, nspec: usize) -> Self {
        Self {
            depoindicator: vec![false; nspec],
            indzindicator: vec![false; nz],
            uprof: vec![0.0; nz],
            vprof: vec![0.0; nz],
            wprof: vec![0.0; nz],
            usigprof: vec![0.0; nz],
            vsigprof: vec![0.0; nz],
            wsigprof: vec![0.0; nz],
            rhoprof: vec![0.0; nz],
            rhogradprof: vec![0.0; nz],
            ..Self::default()
        }
    }

    /// The four corner indices of a 3-D field at level `k`, slot `slot`, in
    /// upstream's order `(ix,jy) (ixp,jy) (ix,jyp) (ixp,jyp)`.
    fn corners3(&self, met: &MetFields, k: usize, slot: usize) -> [usize; 4] {
        [
            met.idx3(self.ix, self.jy, k, slot),
            met.idx3(self.ixp, self.jy, k, slot),
            met.idx3(self.ix, self.jyp, k, slot),
            met.idx3(self.ixp, self.jyp, k, slot),
        ]
    }

    fn corners2(&self, met: &MetFields, slot: usize) -> [usize; 4] {
        [
            met.idx2(self.ix, self.jy, slot),
            met.idx2(self.ixp, self.jy, slot),
            met.idx2(self.ix, self.jyp, slot),
            met.idx2(self.ixp, self.jyp, slot),
        ]
    }

    /// `p1 f(c1) + p2 f(c2) + p3 f(c3) + p4 f(c4)`.
    fn bilinear(&self, f: &[f64], c: [usize; 4]) -> f64 {
        self.p1 * f[c[0]] + self.p2 * f[c[1]] + self.p3 * f[c[2]] + self.p4 * f[c[3]]
    }

    /// `(y1 dt2 + y2 dt1) dtt`.
    fn in_time(&self, y: [f64; 2]) -> f64 {
        (y[0] * self.dt2 + y[1] * self.dt1) * self.dtt
    }

    /// The bilinear and time weights, as every `interpol_*` routine opens.
    fn set_weights(&mut self, itime: i64, xt: f64, yt: f64, met: &MetFields) {
        self.ddx = xt - self.ix as f64;
        self.ddy = yt - self.jy as f64;
        self.rddx = 1.0 - self.ddx;
        self.rddy = 1.0 - self.ddy;
        self.p1 = self.rddx * self.rddy;
        self.p2 = self.ddx * self.rddy;
        self.p3 = self.rddx * self.ddy;
        self.p4 = self.ddx * self.ddy;
        self.dt1 = (itime - met.memtime[0]) as f64;
        self.dt2 = (met.memtime[1] - itime) as f64;
        self.dtt = 1.0 / (self.dt1 + self.dt2);
    }

    /// Upstream's level search: the first level above `zt`, starting from the
    /// second. `None` if `zt` is not below the top level (upstream then keeps
    /// the stale `indz`).
    fn find_level(met: &MetFields, zt: f64) -> Option<usize> {
        (1..met.nz).find(|&i| met.height[i] > zt).map(|i| i - 1)
    }

    /// `interpol_all.f90` / `interpol_all_nests.f90`: everything at the two
    /// levels bracketing `zt`, plus the surface scales.
    ///
    /// Sets the weights, `indz`/`indzp`, and the profiles at those two levels
    /// (clearing their `indzindicator`). The corner indices `ix`, `jy`, `ixp`,
    /// `jyp` and `ngrid` must already be set, as `advance.f90` sets them.
    ///
    /// # Returns
    /// The time-interpolated `u*`, `w*` and `L`. `None` if `zt` is not below
    /// the top level: upstream would then use the previous particle's level.
    pub fn interpol_all(
        &mut self,
        met: &MetFields,
        itime: i64,
        xt: f64,
        yt: f64,
        zt: f64,
    ) -> Option<SurfaceScales> {
        self.set_weights(itime, xt, yt, met);
        let (mut ust1, mut wst1, mut oli1) = ([0.0; 2], [0.0; 2], [0.0; 2]);
        for m in 0..2 {
            let c = self.corners2(met, met.memind[m]);
            ust1[m] = self.bilinear(&met.ustar, c);
            wst1[m] = self.bilinear(&met.wstar, c);
            oli1[m] = self.bilinear(&met.oli, c);
        }
        let ust = self.in_time(ust1);
        let wst = self.in_time(wst1);
        let oliaux = self.in_time(oli1);
        let ol = if oliaux != 0.0 { 1.0 / oliaux } else { 99999.0 };

        let indz = Self::find_level(met, zt)?;
        self.indz = indz;
        self.indzp = indz + 1;
        for n in self.indz..=self.indzp {
            self.profile_at(met, n);
        }
        Some(SurfaceScales { ust, wst, ol })
    }

    /// `interpol_misslev.f90` / `interpol_misslev_nests.f90`: the profiles at
    /// one level (0-based `n`), using the weights already set.
    pub fn interpol_misslev(&mut self, met: &MetFields, n: usize) {
        self.profile_at(met, n);
    }

    /// The per-level body shared verbatim by `interpol_all` and
    /// `interpol_misslev` (8 values: 4 corners x 2 times).
    fn profile_at(&mut self, met: &MetFields, n: usize) {
        let (uf, vf) = horizontal(self.ngrid, met);
        let (mut usl, mut vsl, mut wsl, mut usq, mut vsq, mut wsq) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let (mut y1, mut y2, mut y3, mut rho1, mut rhograd1) =
            ([0.0; 2], [0.0; 2], [0.0; 2], [0.0; 2], [0.0; 2]);
        for m in 0..2 {
            let c = self.corners3(met, n, met.memind[m]);
            y1[m] = self.bilinear(uf, c);
            y2[m] = self.bilinear(vf, c);
            corner_sums(uf, c, &mut usl, &mut usq);
            corner_sums(vf, c, &mut vsl, &mut vsq);
            y3[m] = self.bilinear(&met.ww, c);
            rhograd1[m] = self.bilinear(&met.drhodz, c);
            rho1[m] = self.bilinear(&met.rho, c);
            corner_sums(&met.ww, c, &mut wsl, &mut wsq);
        }
        self.uprof[n] = self.in_time(y1);
        self.vprof[n] = self.in_time(y2);
        self.wprof[n] = self.in_time(y3);
        self.rhoprof[n] = self.in_time(rho1);
        self.rhogradprof[n] = self.in_time(rhograd1);
        self.indzindicator[n] = false;
        self.usigprof[n] = subgrid_sigma(usl, usq, 8.0, 7.0);
        self.vsigprof[n] = subgrid_sigma(vsl, vsq, 8.0, 7.0);
        self.wsigprof[n] = subgrid_sigma(wsl, wsq, 8.0, 7.0);
    }

    /// The wind at the particle, shared by `interpol_wind` and
    /// `interpol_wind_short`. With `sigma`, also the corner sums over the 16
    /// values (4 corners x 2 levels x 2 times).
    fn wind_at(
        &mut self,
        met: &MetFields,
        itime: i64,
        xt: f64,
        yt: f64,
        zt: f64,
        sigma: bool,
    ) -> Option<()> {
        self.set_weights(itime, xt, yt, met);
        let indz = Self::find_level(met, zt)?;
        self.indz = indz;
        let dz = 1.0 / (met.height[indz + 1] - met.height[indz]);
        let dz1 = (zt - met.height[indz]) * dz;
        let dz2 = (met.height[indz + 1] - zt) * dz;

        let (uf, vf) = horizontal(self.ngrid, met);
        let (mut usl, mut vsl, mut wsl, mut usq, mut vsq, mut wsq) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let (mut uh, mut vh, mut wh) = ([0.0; 2], [0.0; 2], [0.0; 2]);
        for m in 0..2 {
            let (mut u1, mut v1, mut w1) = ([0.0; 2], [0.0; 2], [0.0; 2]);
            for n in 0..2 {
                let c = self.corners3(met, indz + n, met.memind[m]);
                u1[n] = self.bilinear(uf, c);
                v1[n] = self.bilinear(vf, c);
                w1[n] = self.bilinear(&met.ww, c);
                if sigma {
                    corner_sums(uf, c, &mut usl, &mut usq);
                    corner_sums(vf, c, &mut vsl, &mut vsq);
                    corner_sums(&met.ww, c, &mut wsl, &mut wsq);
                }
            }
            uh[m] = dz2 * u1[0] + dz1 * u1[1];
            vh[m] = dz2 * v1[0] + dz1 * v1[1];
            wh[m] = dz2 * w1[0] + dz1 * w1[1];
        }
        self.u = self.in_time(uh);
        self.v = self.in_time(vh);
        self.w = self.in_time(wh);
        if sigma {
            self.usig = subgrid_sigma(usl, usq, 16.0, 15.0);
            self.vsig = subgrid_sigma(vsl, vsq, 16.0, 15.0);
            self.wsig = subgrid_sigma(wsl, wsq, 16.0, 15.0);
        }
        Some(())
    }

    /// `interpol_wind.f90` / `interpol_wind_nests.f90`: wind and its sub-grid
    /// standard deviation at the particle, for the step above the boundary
    /// layer. Sets `u`, `v`, `w`, `usig`, `vsig`, `wsig`. `None` if `zt` is
    /// not below the top level.
    pub fn interpol_wind(
        &mut self,
        met: &MetFields,
        itime: i64,
        xt: f64,
        yt: f64,
        zt: f64,
    ) -> Option<()> {
        self.wind_at(met, itime, xt, yt, zt, true)
    }

    /// `interpol_wind_short.f90` / `interpol_wind_short_nests.f90`: as
    /// [`Self::interpol_wind`] without the standard deviations (the
    /// Petterssen correction step). Leaves `usig`, `vsig`, `wsig` untouched.
    pub fn interpol_wind_short(
        &mut self,
        met: &MetFields,
        itime: i64,
        xt: f64,
        yt: f64,
        zt: f64,
    ) -> Option<()> {
        self.wind_at(met, itime, xt, yt, zt, false)
    }

    /// `interpol_vdep.f90` / `interpol_vdep_nests.f90`: deposition velocity
    /// of species `level` (0-based) at the particle, using the weights
    /// already set; clears its `depoindicator`.
    pub fn interpol_vdep(&mut self, met: &MetFields, level: usize) -> f64 {
        let mut y = [0.0; 2];
        for (m, ym) in y.iter_mut().enumerate() {
            let slot = met.memind[m];
            let c = [
                met.idx_vdep(self.ix, self.jy, level, slot),
                met.idx_vdep(self.ixp, self.jy, level, slot),
                met.idx_vdep(self.ix, self.jyp, level, slot),
                met.idx_vdep(self.ixp, self.jyp, level, slot),
            ];
            *ym = self.bilinear(&met.vdep, c);
        }
        self.depoindicator[level] = false;
        self.in_time(y)
    }
}
