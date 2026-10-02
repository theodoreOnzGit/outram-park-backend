// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/verttransform_ecmwf.f90, src/verttransform_gfs.f90,
//                    src/verttransform_nests.f90
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
//                    (A. Stohl, G. Wotawa; GFS version C. Forster; clouds
//                    S. Eckhardt, H. Grythe; unified build M. Harustak)
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Transformation of the raw model-level meteorology to FLEXPART's internal
//! Cartesian `z` grid: the `verttransform_*` routines that `getfields.f90`
//! calls once per wind field read.
//!
//! For every column the routines integrate the hypsometric equation over the
//! model levels (virtual temperature, `const = r_air/ga`), interpolate the
//! horizontal wind, temperature, humidity, potential vorticity, density (and
//! cloud water) linearly in height onto the fixed reference heights
//! `height(1..nz)`, convert the vertical velocity from pressure units to m/s
//! with `pinmconv = dz/dp` and add the terrain-slope correction
//! `dz/dx u + dz/dy v`, compute `drhodz`, rotate the winds into the
//! polar-stereographic grids near the poles (via [`cmapf::cc2gll`]), and
//! diagnose the cloud / precipitation-type field `clouds` that the wet
//! scavenging reads.
//!
//! | Rust | Upstream |
//! |---|---|
//! | [`verttransform_ecmwf`] | `verttransform_ecmwf.f90` (hybrid eta levels, `eta-dot` in Pa/s) |
//! | [`verttransform_gfs`] | `verttransform_gfs.f90` (NCEP pressure levels, `omega` in Pa/s) |
//! | [`verttransform_nests`] | `verttransform_nests.f90` (ECMWF nests, no polar part) |
//!
//! # State, explicitly
//!
//! Upstream reads and writes `com_mod` and keeps state between calls; the
//! port passes all of it explicitly:
//!
//! * [`RawFields`] — the raw level fields `readwind` fills (`uuh`, `vvh`,
//!   `pvh`, `wwh` arguments; `tth`, `qvh`, `clwch`, `ciwch`, `ps`, `tt2`,
//!   `td2`, `lsprec`, `convprec` from `com_mod` at time slot `n`).
//! * [`ZFields`] — the `com_mod` output arrays of one time slot (`uu`, `vv`,
//!   `ww`, `tt`, `qv`, `pv`, `rho`, `drhodz`, `prs`, `pplev`, `uupol`,
//!   `vvpol`, `clwc`, `ciwc`, `clw`, `clouds`, `cloudsh`, `ctwc`). It is
//!   **in/out**: several outputs are only partly rewritten (see the quirks),
//!   so the caller must pass the previous contents of that slot, as upstream's
//!   `com_mod` would hold them. `interpolation::MetFields` has no `qv`, `pv`,
//!   `prs`, cloud fields, so it cannot hold everything; [`ZFields::copy_into_met`]
//!   copies the fields `MetFields` does have.
//! * [`ZGrid`] — `com_mod`'s `height` and `nmixz`, which the ECMWF and GFS
//!   routines set **on their first call only** and every routine reads.
//! * [`SavedInit`] (ECMWF) and [`GfsSaved`] (GFS) — the routines' own `SAVE`d
//!   state: the `init` flag, and for GFS the static array `uvwzlev`, whose
//!   levels below a column's first above-ground level are never rewritten.
//!
//! Field storage reuses [`Field2`]/[`Field3`] (`ix` fastest, levels 0-based:
//! upstream level `k` is index `k-1`). A field may be **larger** than the grid
//! (like upstream's `0:nxmax-1` arrays): elements are addressed by
//! `(ix, jy, k)`, so values outside the active grid persist untouched between
//! calls on grids of different size, exactly as in `com_mod`.
//!
//! # Units
//!
//! FLEXPART's: Pa, K, kg/kg, m, m/s, kg/m³, kg/m⁴ (`drhodz`); `wwh` in Pa/s
//! (ECMWF `eta-dot` is already multiplied by `dp/deta` in `readwind`); the
//! output `ww` in m/s. Precipitation in mm/h. `cloudsh` in whole metres
//! (upstream `integer`).
//!
//! # Translation notes
//!
//! The arithmetic follows upstream left to right (`a*b*c/d` is
//! `((a*b)*c)/d`), so a `-fdefault-real-8` build agrees bit for bit. The
//! virtual-temperature factor of the reference column, `tt2*(1+0.378*ew(td2)/ps)`,
//! calls [`thermo::ew_kelvin`]; the old cloud scheme calls
//! [`boundary_layer::f_qvsat`]; both are the verified ports.
//!
//! # Upstream quirks reproduced (each pinned by the code-to-code fixture)
//!
//! 1. **Heights are set once, from the first field ever read.** `init` is a
//!    `SAVE`d logical. The reference profile `height(1..nuvz)` is integrated at
//!    the first column (`jy` outer, `ix` inner) with `ps > 100000 Pa`, and
//!    `nmixz` is the first level above `hmixmax = 4500 m`. Every later call, of
//!    either routine's *other* fields too, reuses them. The ECMWF and GFS
//!    routines each have their **own** `init`, but write the **same**
//!    `com_mod` `height`: the second routine to be called for the first time
//!    overwrites the first's heights. If no level exceeds `hmixmax`, `nmixz`
//!    keeps whatever it held. If no column has `ps > 100000 Pa`, upstream
//!    reads undefined `ixm, jym`; the port returns
//!    [`VertError::NoReferenceColumn`].
//! 2. **`cloudsh` is an `integer`**: `cloudsh = cloudsh + height(kz) -
//!    height(kz-1)` is evaluated in `real` and **truncated** on assignment,
//!    at every layer, so the sum loses up to 1 m per layer.
//! 3. **The cloud-water scheme (`readclouds`) never resets `cloudsh`**: it
//!    accumulates on whatever the slot held (the old scheme resets it to 0
//!    per column). The nest version accumulates on the never-initialised
//!    `allocate`d `cloudshn`.
//! 4. **`cloudh_min` is the bottom of the *highest* cloud layer, and is stale
//!    across columns.** It is assigned at every cloudy level scanning
//!    *upwards*, so the last assignment wins; a column with no cloud water but
//!    with precipitation reuses the previous column's (or previous nest's)
//!    value. It is a non-`SAVE` local, undefined before the first assignment
//!    in a call: the port returns [`VertError::UndefinedCloudBase`] if it
//!    would be read then.
//! 5. **The old cloud scheme never writes `clouds` at level 1**, and the
//!    cloud-water scheme leaves level 1 at the 0 it reset everything to; the
//!    nest cloud-water scheme instead runs to level 1 and reads `height(0)`,
//!    **outside the array**, when level 1 is cloudy in a precipitating column
//!    ([`VertError::NestHeightZero`]).
//! 6. **The south-pole wind is rotated with the NORTH-pole map**:
//!    `call cc2gll(northpolemap, -90., 180., ...)` in both ECMWF and GFS. On a
//!    grid that holds the South Pole but not the North Pole, `northpolemap` is
//!    never set (`gridcheck` only sets it for `nglobal`) and is all zeros, so
//!    the pole row's `uupol`/`vvpol` are 0.
//! 7. **GFS south pole, `vv > 0`: `ddpol = pi + atan(u/v) - xlonr`**, where
//!    the ECMWF routine (and both other GFS branches) have `+ xlonr`.
//! 8. **`uupol`/`vvpol` are only written in the polar bands** (`jy >=
//!    int(switchnorthg)-2`, `jy <= int(switchsouthg)+3`); every other row keeps
//!    the slot's previous values. Where the two bands overlap, the south pass
//!    (run second) wins.
//! 9. **Linear extrapolation with a stale bracket.** ECMWF/nests: when a
//!    reference height lies above the column's top `w` level (or, in the slope
//!    correction, above its top `u` level), the bracket search fails and the
//!    previous level's bracket index is reused, so the value is extrapolated.
//!    GFS: when the slope-correction search fails, `kl`, `klp`, `dz1`, `dz2`,
//!    `dz` keep their last values (from the previous level, the previous
//!    column, or the `w` interpolation of the last column); the port carries
//!    them exactly, and returns [`VertError::UndefinedSlopeBracket`] if `kl`
//!    was never assigned in the call.
//! 10. **GFS `ww` levels the bracket search misses keep the slot's previous
//!     `ww`** (no extrapolation in GFS).
//! 11. **GFS reads stale `uvwzlev` at neighbours.** `uvwzlev` is a static
//!     local, rewritten only from each column's first above-ground level
//!     `llev` upward; the slope correction reads neighbouring columns at the
//!     centre column's levels, which may lie below the neighbour's `llev`, so
//!     it reads that neighbour's values from an earlier call (0 before the
//!     first). [`GfsSaved`] carries the array.
//! 12. **GFS `llev` is capped at `nuvz-2`** even when more levels are below
//!     ground; the heights then start from that level's pressure `akz(llev)`,
//!     not from `ps`.
//! 13. **`pinmconv` at level 1 divides `uvzlev(2)` by `p(2)-p(1)`** (one-sided)
//!     and the `ww` of level `nz` set before the loop is overwritten by the
//!     `iz = nz` interpolation (ECMWF/nests).
//! 14. **ECMWF/nests: with `readclouds` and not `sumclouds`, `clwc` is
//!     overwritten in place by `clwc + ciwc`** (all levels), so the stored
//!     `clwc` is total condensate.
//! 15. **The `virr` test counter** and its never-executed file output
//!     (`if (1.eq.2)`) have no effect and are not ported.

use super::boundary_layer::f_qvsat;
use super::cmapf::{cc2gll, Strcmp};
use super::constants::{GA, HMIXMAX, PI, PI180, R_AIR};
use super::interpolation::MetFields;
use super::met_fields::{Field2, Field3, GridGeometry};
use super::thermo::ew_kelvin;

/// `const = r_air/ga` of the three routines, m/K.
const CONST: f64 = R_AIR / GA;

/// Why a `verttransform_*` port refused. Each is an input on which upstream
/// reads an undefined value or writes outside an array.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VertError {
    /// A field is smaller than the grid it is used on, or `nz != nuvz`, or
    /// `nwz > nuvz` / `nwz < 2`, or `nuvz < 3`.
    ShapeMismatch,
    /// First call, and no column has `ps > 100000 Pa`: upstream integrates the
    /// reference profile at undefined indices `ixm, jym`.
    NoReferenceColumn,
    /// A polar band starts below row 0 (`int(switchnorthg)-2 < 0`) or ends
    /// above row `ny-1` (`int(switchsouthg)+3 > ny-1`): upstream indexes
    /// outside the grid.
    PolarBandOutOfGrid,
    /// The precipitation-type diagnostic needs `cloudh_min` before any
    /// cloudy level has assigned it in this call (undefined upstream).
    UndefinedCloudBase,
    /// GFS slope correction: the first bracket search of the call failed, so
    /// upstream uses never-assigned `kl`, `klp`.
    UndefinedSlopeBracket,
    /// Nest cloud-water scheme: level 1 is cloudy in a precipitating column,
    /// and upstream reads `height(0)`, outside the array.
    NestHeightZero,
}

/// `com_mod`'s reference heights and PBL level cap.
#[derive(Debug, Clone, PartialEq)]
pub struct ZGrid {
    /// `height(1..)`, m above ground, index `k-1`. At least `nz` long.
    pub height: Vec<f64>,
    /// `nmixz`: the first level (1-based) whose height exceeds `hmixmax`.
    pub nmixz: usize,
}

/// A routine's `SAVE`d `init` flag (`logical :: init = .true.`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavedInit {
    /// `true` until the routine's first call has set `height` and `nmixz`.
    pub init: bool,
}

impl Default for SavedInit {
    fn default() -> Self {
        Self { init: true }
    }
}

/// `verttransform_gfs`'s state between calls: its `init` flag and its static
/// local `uvwzlev(0:nxmax-1,0:nymax-1,nzmax)` (zero before the first call;
/// see quirk 11).
#[derive(Debug, Clone, PartialEq)]
pub struct GfsSaved {
    /// The `init` flag.
    pub init: bool,
    /// `uvwzlev`, m; capacity fixed at construction (like `nxmax`, ...).
    pub uvwzlev: Field3,
}

impl GfsSaved {
    /// Fresh state, with room for grids up to `nxmax x nymax x nzmax`.
    #[must_use]
    pub fn new(nxmax: usize, nymax: usize, nzmax: usize) -> Self {
        Self {
            init: true,
            uvwzlev: Field3::filled(nxmax, nymax, nzmax, 0.0),
        }
    }
}

/// Hybrid coefficients from `gridcheck`: `akz`, `bkz` (length `>= nuvz`) and
/// `aknew`, `bknew` (length `>= nz`), index `k-1`. Level pressure
/// `p = a + b*ps`, Pa. For GFS `akz` holds the pressure levels and `bkz` is 0.
#[derive(Debug, Clone, PartialEq)]
pub struct HybridCoefficients {
    /// `akz`, Pa.
    pub akz: Vec<f64>,
    /// `bkz`.
    pub bkz: Vec<f64>,
    /// `aknew`, Pa.
    pub aknew: Vec<f64>,
    /// `bknew`.
    pub bknew: Vec<f64>,
}

/// The grid a routine works on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VertGrid {
    /// Horizontal geometry (`nx`, `ny`, `dx`, `dy`, `xlon0`, `ylat0`).
    pub geom: GridGeometry,
    /// `dxconst = 180/(dx r_earth pi)` (rad/m per degree), from `gridcheck`.
    pub dxconst: f64,
    /// `dyconst`.
    pub dyconst: f64,
    /// Levels of `u`, `v`, `T`, `q` (`nuvz`).
    pub nuvz: usize,
    /// Levels of `w` (`nwz`); `gridcheck` gives `nwz <= nuvz`.
    pub nwz: usize,
    /// Output levels (`nz`); `gridcheck` sets `nz = nuvz`, which the port
    /// requires (upstream would read unset levels otherwise).
    pub nz: usize,
}

/// `com_mod`'s polar-grid settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PolarCaps {
    /// Row `ny-1` is the North Pole.
    pub nglobal: bool,
    /// Row 0 is the South Pole.
    pub sglobal: bool,
    /// `switchnorthg`, grid units.
    pub switchnorthg: f64,
    /// `switchsouthg`, grid units.
    pub switchsouthg: f64,
    /// `northpolemap` (all zeros unless `gridcheck` set it; see quirk 6).
    pub northpolemap: Strcmp,
    /// `southpolemap`.
    pub southpolemap: Strcmp,
}

impl PolarCaps {
    /// A limited-area grid: no polar processing.
    pub const LIMITED: Self = Self {
        nglobal: false,
        sglobal: false,
        switchnorthg: 999_999.0,
        switchsouthg: 999_999.0,
        northpolemap: Strcmp([0.0; 9]),
        southpolemap: Strcmp([0.0; 9]),
    };
}

/// Cloud diagnostics selected by `readwind` (`readclouds`, `sumclouds`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CloudScheme {
    /// Cloud water was read from the GRIB file (`readclouds`); otherwise the
    /// relative-humidity parameterisation is used.
    pub readclouds: bool,
    /// Liquid and ice were read already summed (`sumclouds`).
    pub sumclouds: bool,
}

/// The raw level fields of one time slot (what `readwind` provides).
/// 3-d fields have `nuvz` levels (`wwh`: `nwz`), index `k-1`.
#[derive(Debug, Clone, PartialEq)]
pub struct RawFields {
    /// `uuh`, m/s (level 1: 10-m wind).
    pub uuh: Field3,
    /// `vvh`, m/s.
    pub vvh: Field3,
    /// `pvh`, pvu.
    pub pvh: Field3,
    /// `wwh`, Pa/s.
    pub wwh: Field3,
    /// `tth`, K (level 1: 2-m temperature).
    pub tth: Field3,
    /// `qvh`, kg/kg.
    pub qvh: Field3,
    /// `clwch`, kg/kg (read only with `readclouds`).
    pub clwch: Field3,
    /// `ciwch`, kg/kg (read only with `readclouds` and not `sumclouds`).
    pub ciwch: Field3,
    /// `ps`, Pa.
    pub ps: Field2,
    /// `tt2`, K.
    pub tt2: Field2,
    /// `td2`, K.
    pub td2: Field2,
    /// `lsprec`, mm/h.
    pub lsprec: Field2,
    /// `convprec`, mm/h.
    pub convprec: Field2,
}

/// The `com_mod` output arrays of one time slot. In/out: see the module docs.
/// `clouds` and `cloudsh` are upstream `integer(kind=1)` / `integer` arrays,
/// stored with the [`Field3`]/[`Field2`] index layout of `nx_cap`, `ny_cap`.
#[derive(Debug, Clone, PartialEq)]
pub struct ZFields {
    /// `uu`, m/s.
    pub uu: Field3,
    /// `vv`, m/s.
    pub vv: Field3,
    /// `ww`, m/s.
    pub ww: Field3,
    /// `tt`, K.
    pub tt: Field3,
    /// `qv`, kg/kg.
    pub qv: Field3,
    /// `pv`, pvu.
    pub pv: Field3,
    /// `rho`, kg/m³.
    pub rho: Field3,
    /// `drhodz`, kg/m⁴.
    pub drhodz: Field3,
    /// `prs`, Pa (ECMWF only).
    pub prs: Field3,
    /// `pplev`, Pa (GFS only).
    pub pplev: Field3,
    /// `uupol`, m/s (polar-stereographic grid component).
    pub uupol: Field3,
    /// `vvpol`, m/s.
    pub vvpol: Field3,
    /// `clwc`, kg/kg.
    pub clwc: Field3,
    /// `ciwc`, kg/kg.
    pub ciwc: Field3,
    /// `clw`, m (column-integrated cloud water per layer, upstream "m3/m3").
    pub clw: Field3,
    /// `clouds`: 0 none, 1 cloud, 2/3 convective/large-scale in-cloud,
    /// 4/5 convective/large-scale washout.
    pub clouds: Vec<i8>,
    /// `cloudsh`, m (integer).
    pub cloudsh: Vec<i32>,
    /// `ctwc`, total column cloud water.
    pub ctwc: Field2,
    nx_cap: usize,
    ny_cap: usize,
    nz_cap: usize,
}

impl ZFields {
    /// Arrays of capacity `nx x ny x nz`, every real set to `real`, `clouds`
    /// to `cloud`, `cloudsh` to `cloudsh`.
    #[must_use]
    pub fn filled(nx: usize, ny: usize, nz: usize, real: f64, cloud: i8, cloudsh: i32) -> Self {
        let f3 = || Field3::filled(nx, ny, nz, real);
        Self {
            uu: f3(),
            vv: f3(),
            ww: f3(),
            tt: f3(),
            qv: f3(),
            pv: f3(),
            rho: f3(),
            drhodz: f3(),
            prs: f3(),
            pplev: f3(),
            uupol: f3(),
            vvpol: f3(),
            clwc: f3(),
            ciwc: f3(),
            clw: f3(),
            clouds: vec![cloud; nx * ny * nz],
            cloudsh: vec![cloudsh; nx * ny],
            ctwc: Field2::filled(nx, ny, real),
            nx_cap: nx,
            ny_cap: ny,
            nz_cap: nz,
        }
    }

    /// `clouds` at `(ix, jy, k)`, `k` 0-based.
    #[must_use]
    pub fn cloud(&self, ix: usize, jy: usize, k: usize) -> i8 {
        self.clouds[ix + self.nx_cap * (jy + self.ny_cap * k)]
    }

    fn set_cloud(&mut self, ix: usize, jy: usize, k: usize, v: i8) {
        self.clouds[ix + self.nx_cap * (jy + self.ny_cap * k)] = v;
    }

    /// `cloudsh` at `(ix, jy)`.
    #[must_use]
    pub fn cloud_height(&self, ix: usize, jy: usize) -> i32 {
        self.cloudsh[ix + self.nx_cap * jy]
    }

    fn set_cloud_height(&mut self, ix: usize, jy: usize, v: i32) {
        self.cloudsh[ix + self.nx_cap * jy] = v;
    }

    fn fits(&self, nx: usize, ny: usize, nz: usize) -> bool {
        nx <= self.nx_cap && ny <= self.ny_cap && nz <= self.nz_cap
    }

    /// Copy the fields [`MetFields`] holds (`uu`, `vv`, `ww`, `uupol`,
    /// `vvpol`, `rho`, `drhodz`, `tt`) of the `nx x ny x nz` grid into
    /// `met`'s storage slot `slot`.
    pub fn copy_into_met(&self, met: &mut MetFields, slot: usize) {
        for k in 0..met.nz {
            for j in 0..met.ny {
                for i in 0..met.nx {
                    let d = met.idx3(i, j, k, slot);
                    met.uu[d] = self.uu.at(i, j, k);
                    met.vv[d] = self.vv.at(i, j, k);
                    met.ww[d] = self.ww.at(i, j, k);
                    met.uupol[d] = self.uupol.at(i, j, k);
                    met.vvpol[d] = self.vvpol.at(i, j, k);
                    met.rho[d] = self.rho.at(i, j, k);
                    met.drhodz[d] = self.drhodz.at(i, j, k);
                    met.tt[d] = self.tt.at(i, j, k);
                }
            }
        }
    }
}

// ── shared pieces ────────────────────────────────────────────────────────────

fn f3_fits(f: &Field3, nx: usize, ny: usize, nz: usize) -> bool {
    nx <= f.nx && ny <= f.ny && nz <= f.nz && f.data.len() >= f.nx * f.ny * f.nz
}

fn f2_fits(f: &Field2, nx: usize, ny: usize) -> bool {
    nx <= f.nx && ny <= f.ny && f.data.len() >= f.nx * f.ny
}

fn check_shapes(
    grid: &VertGrid,
    hyb: &HybridCoefficients,
    zgrid: &ZGrid,
    raw: &RawFields,
    out: &ZFields,
) -> Result<(), VertError> {
    let (nx, ny) = (grid.geom.nx, grid.geom.ny);
    let (nuvz, nwz, nz) = (grid.nuvz, grid.nwz, grid.nz);
    let ok = nz == nuvz
        && nuvz >= 3
        && (2..=nuvz).contains(&nwz)
        && nx >= 1
        && ny >= 1
        && hyb.akz.len() >= nuvz
        && hyb.bkz.len() >= nuvz
        && hyb.aknew.len() >= nz
        && hyb.bknew.len() >= nz
        && zgrid.height.len() >= nz
        && [&raw.uuh, &raw.vvh, &raw.pvh, &raw.tth, &raw.qvh, &raw.clwch, &raw.ciwch]
            .iter()
            .all(|f| f3_fits(f, nx, ny, nuvz))
        && f3_fits(&raw.wwh, nx, ny, nwz)
        && [&raw.ps, &raw.tt2, &raw.td2, &raw.lsprec, &raw.convprec]
            .iter()
            .all(|f| f2_fits(f, nx, ny))
        && out.fits(nx, ny, nz);
    if ok {
        Ok(())
    } else {
        Err(VertError::ShapeMismatch)
    }
}

/// `tvold` of a column's surface: `tt2*(1.+0.378*ew(td2)/ps)`.
fn surface_tv(raw: &RawFields, ix: usize, jy: usize) -> f64 {
    raw.tt2.at(ix, jy) * (1.0 + 0.378 * ew_kelvin(raw.td2.at(ix, jy)) / raw.ps.at(ix, jy))
}

/// One step of the hypsometric integration, shared by every routine:
/// `z(kz-1) + const*log(pold/pint)*(tv-tvold)/log(tv/tvold)` for a layer
/// whose virtual temperature changes by more than 0.2 K, else
/// `z(kz-1) + const*log(pold/pint)*tv`.
fn hypsometric(zlow: f64, pold: f64, pint: f64, tv: f64, tvold: f64) -> f64 {
    if (tv - tvold).abs() > 0.2 {
        zlow + CONST * (pold / pint).ln() * (tv - tvold) / (tv / tvold).ln()
    } else {
        zlow + CONST * (pold / pint).ln() * tv
    }
}

/// The `if (init)` block (identical arithmetic in ECMWF and GFS): reference
/// heights from the first column with `ps > 100000 Pa`, then `nmixz`.
fn init_heights(
    grid: &VertGrid,
    hyb: &HybridCoefficients,
    raw: &RawFields,
    zgrid: &mut ZGrid,
) -> Result<(), VertError> {
    let (nx, ny) = (grid.geom.nx, grid.geom.ny);
    let mut found = None;
    'search: for jy in 0..ny {
        for ix in 0..nx {
            if raw.ps.at(ix, jy) > 100000.0 {
                found = Some((ix, jy));
                break 'search;
            }
        }
    }
    let (ixm, jym) = found.ok_or(VertError::NoReferenceColumn)?;
    let psm = raw.ps.at(ixm, jym);
    let mut tvold = surface_tv(raw, ixm, jym);
    let mut pold = psm;
    zgrid.height[0] = 0.0;
    for kz in 2..=grid.nuvz {
        let pint = hyb.akz[kz - 1] + hyb.bkz[kz - 1] * psm;
        let tv = raw.tth.at(ixm, jym, kz - 1) * (1.0 + 0.608 * raw.qvh.at(ixm, jym, kz - 1));
        zgrid.height[kz - 1] = hypsometric(zgrid.height[kz - 2], pold, pint, tv, tvold);
        tvold = tv;
        pold = pint;
    }
    for kz in 1..=grid.nz {
        if zgrid.height[kz - 1] > HMIXMAX {
            zgrid.nmixz = kz;
            break;
        }
    }
    Ok(())
}

/// `int(x)` of a Fortran `real` (truncation toward zero).
fn fortran_int(x: f64) -> i64 {
    x.trunc() as i64
}

/// The polar-stereographic winds and the pole `w` fix (ECMWF and GFS; the
/// one arithmetic difference, quirk 7, is `gfs`).
fn polar_caps(grid: &VertGrid, polar: &PolarCaps, gfs: bool, out: &mut ZFields) {
    let g = &grid.geom;
    let (nx, ny, nz) = (g.nx, g.ny, grid.nz);
    let nymin1 = ny - 1;
    let ixc = nx / 2 - 1;
    if polar.nglobal {
        let j0 = (fortran_int(polar.switchnorthg) - 2) as usize;
        for iz in 0..nz {
            for jy in j0..=nymin1 {
                let ylat = g.ylat0 + jy as f64 * g.dy;
                for ix in 0..nx {
                    let xlon = g.xlon0 + ix as f64 * g.dx;
                    let (ug, vg) = cc2gll(
                        &polar.northpolemap,
                        ylat,
                        xlon,
                        out.uu.at(ix, jy, iz),
                        out.vv.at(ix, jy, iz),
                    );
                    out.uupol.set(ix, jy, iz, ug);
                    out.vvpol.set(ix, jy, iz, vg);
                }
            }
        }
        for iz in 0..nz {
            let xlon = g.xlon0 + ixc as f64 * g.dx;
            let xlonr = xlon * PI / 180.0;
            let u = out.uu.at(ixc, nymin1, iz);
            let v = out.vv.at(ixc, nymin1, iz);
            let ffpol = (u * u + v * v).sqrt();
            let mut ddpol = if v < 0.0 {
                (u / v).atan() - xlonr
            } else if v > 0.0 {
                PI + (u / v).atan() - xlonr
            } else {
                PI / 2.0 - xlonr
            };
            if ddpol < 0.0 {
                ddpol = 2.0 * PI + ddpol;
            }
            if ddpol > 2.0 * PI {
                ddpol -= 2.0 * PI;
            }
            let xlon = 180.0;
            let xlonr = xlon * PI / 180.0;
            let ylat = 90.0;
            let uuaux = -(ffpol * (xlonr + ddpol).sin());
            let vvaux = -(ffpol * (xlonr + ddpol).cos());
            let (up, vp) = cc2gll(&polar.northpolemap, ylat, xlon, uuaux, vvaux);
            for ix in 0..nx {
                out.uupol.set(ix, nymin1, iz, up);
                out.vvpol.set(ix, nymin1, iz, vp);
            }
        }
        for iz in 0..nz {
            let mut wdummy = 0.0;
            for ix in 0..nx {
                wdummy += out.ww.at(ix, ny - 2, iz);
            }
            wdummy /= nx as f64;
            for ix in 0..nx {
                out.ww.set(ix, nymin1, iz, wdummy);
            }
        }
    }
    if polar.sglobal {
        let j1 = (fortran_int(polar.switchsouthg) + 3) as usize;
        for iz in 0..nz {
            for jy in 0..=j1 {
                let ylat = g.ylat0 + jy as f64 * g.dy;
                for ix in 0..nx {
                    let xlon = g.xlon0 + ix as f64 * g.dx;
                    let (ug, vg) = cc2gll(
                        &polar.southpolemap,
                        ylat,
                        xlon,
                        out.uu.at(ix, jy, iz),
                        out.vv.at(ix, jy, iz),
                    );
                    out.uupol.set(ix, jy, iz, ug);
                    out.vvpol.set(ix, jy, iz, vg);
                }
            }
        }
        for iz in 0..nz {
            let xlon = g.xlon0 + ixc as f64 * g.dx;
            let xlonr = xlon * PI / 180.0;
            let u = out.uu.at(ixc, 0, iz);
            let v = out.vv.at(ixc, 0, iz);
            let ffpol = (u * u + v * v).sqrt();
            let mut ddpol = if v < 0.0 {
                (u / v).atan() + xlonr
            } else if v > 0.0 {
                if gfs {
                    // quirk 7: verttransform_gfs.f90 line 456
                    PI + (u / v).atan() - xlonr
                } else {
                    PI + (u / v).atan() + xlonr
                }
            } else {
                PI / 2.0 - xlonr
            };
            if ddpol < 0.0 {
                ddpol = 2.0 * PI + ddpol;
            }
            if ddpol > 2.0 * PI {
                ddpol -= 2.0 * PI;
            }
            let xlon = 180.0;
            let xlonr = xlon * PI / 180.0;
            let ylat = -90.0;
            let uuaux = ffpol * (xlonr - ddpol).sin();
            let vvaux = -(ffpol * (xlonr - ddpol).cos());
            // quirk 6: the NORTH-pole map, as upstream
            let (up, vp) = cc2gll(&polar.northpolemap, ylat, xlon, uuaux, vvaux);
            for ix in 0..nx {
                out.uupol.set(ix, 0, iz, up);
                out.vvpol.set(ix, 0, iz, vp);
            }
        }
        for iz in 0..nz {
            let mut wdummy = 0.0;
            for ix in 0..nx {
                wdummy += out.ww.at(ix, 1, iz);
            }
            wdummy /= nx as f64;
            for ix in 0..nx {
                out.ww.set(ix, 0, iz, wdummy);
            }
        }
    }
}

fn check_polar(grid: &VertGrid, polar: &PolarCaps) -> Result<(), VertError> {
    let ny = grid.geom.ny as i64;
    if polar.nglobal && (fortran_int(polar.switchnorthg) - 2 < 0 || ny < 2) {
        return Err(VertError::PolarBandOutOfGrid);
    }
    if polar.sglobal && (fortran_int(polar.switchsouthg) + 3 > ny - 1 || ny < 2) {
        return Err(VertError::PolarBandOutOfGrid);
    }
    if (polar.nglobal || polar.sglobal) && grid.geom.nx < 2 {
        return Err(VertError::PolarBandOutOfGrid);
    }
    Ok(())
}

/// Cloud-water scheme for one column (`readclouds`), all three routines.
/// `bottom` is the lowest level of the precipitation-type loop (2 for the
/// mother grid, 1 for nests, quirk 5). `cloudh_min` carries across columns.
#[allow(clippy::too_many_arguments)]
fn cloud_water_column(
    ix: usize,
    jy: usize,
    nz: usize,
    height: &[f64],
    lsp: f64,
    convp: f64,
    bottom: usize,
    cloudh_min: &mut Option<f64>,
    out: &mut ZFields,
) -> Result<(), VertError> {
    for kz in 1..nz {
        if out.clwc.at(ix, jy, kz - 1) > 0.0 {
            let clw =
                (out.clwc.at(ix, jy, kz - 1) * out.rho.at(ix, jy, kz - 1)) * (height[kz] - height[kz - 1]);
            out.clw.set(ix, jy, kz - 1, clw);
            let c = out.ctwc.at(ix, jy) + clw;
            out.ctwc.set(ix, jy, c);
            *cloudh_min = Some(height[kz].min(height[kz - 1]));
        }
    }
    if lsp > 0.01 || convp > 0.01 {
        for kz in (bottom..=nz).rev() {
            let clw = out.clw.at(ix, jy, kz - 1);
            if clw > 0.0 {
                if kz == 1 {
                    return Err(VertError::NestHeightZero);
                }
                let sh = ((out.cloud_height(ix, jy) as f64 + height[kz - 1]) - height[kz - 2]).trunc();
                out.set_cloud_height(ix, jy, sh as i32);
                out.set_cloud(ix, jy, kz - 1, 1);
                out.set_cloud(ix, jy, kz - 1, if lsp >= convp { 3 } else { 2 });
            } else if clw <= 0.0 {
                let cmin = cloudh_min.ok_or(VertError::UndefinedCloudBase)?;
                if cmin >= height[kz - 1] {
                    out.set_cloud(ix, jy, kz - 1, if lsp >= convp { 5 } else { 4 });
                }
            }
            if height[kz - 1] >= 19000.0 {
                out.set_cloud(ix, jy, kz - 1, 0);
            }
        }
    }
    Ok(())
}

/// Old (relative-humidity) cloud scheme for one column, all three routines.
fn rh_cloud_column(ix: usize, jy: usize, nz: usize, height: &[f64], lsp: f64, convp: f64, out: &mut ZFields) {
    let mut rain_cloud_above = false;
    out.set_cloud_height(ix, jy, 0);
    for kz_inv in 1..nz {
        let kz = nz - kz_inv + 1;
        let tt = out.tt.at(ix, jy, kz - 1);
        let pressure = out.rho.at(ix, jy, kz - 1) * R_AIR * tt;
        let rh = out.qv.at(ix, jy, kz - 1) / f_qvsat(pressure, tt);
        out.set_cloud(ix, jy, kz - 1, 0);
        if rh > 0.8 {
            if lsp > 0.01 || convp > 0.01 {
                rain_cloud_above = true;
                let sh = ((out.cloud_height(ix, jy) as f64 + height[kz - 1]) - height[kz - 2]).trunc();
                out.set_cloud_height(ix, jy, sh as i32);
                out.set_cloud(ix, jy, kz - 1, if lsp >= convp { 3 } else { 2 });
            } else {
                out.set_cloud(ix, jy, kz - 1, 1);
            }
        } else if rain_cloud_above {
            out.set_cloud(ix, jy, kz - 1, if lsp >= convp { 5 } else { 4 });
        }
    }
}

/// Per-column profiles of the hybrid-level routines (ECMWF and nests).
struct EtaColumns {
    nx: usize,
    ny: usize,
    /// `uvzlev`, `[k][jy][ix]`.
    uvzlev: Vec<f64>,
    /// `wzlev`.
    wzlev: Vec<f64>,
    /// `rhoh`.
    rhoh: Vec<f64>,
    /// `prsh`.
    prsh: Vec<f64>,
    /// `pinmconv`.
    pinmconv: Vec<f64>,
}

impl EtaColumns {
    fn at(&self, v: &[f64], ix: usize, jy: usize, k: usize) -> f64 {
        v[ix + self.nx * (jy + self.ny * k)]
    }
    fn i(&self, ix: usize, jy: usize, k: usize) -> usize {
        ix + self.nx * (jy + self.ny * k)
    }
}

/// Heights of the eta levels, `w` levels, `rhoh`, `prsh` and `pinmconv`
/// (ECMWF lines 204–258; nests 91–143).
fn eta_columns(grid: &VertGrid, hyb: &HybridCoefficients, raw: &RawFields) -> EtaColumns {
    let (nx, ny) = (grid.geom.nx, grid.geom.ny);
    let (nuvz, nwz, nz) = (grid.nuvz, grid.nwz, grid.nz);
    let n3 = nx * ny * nuvz.max(nz);
    let mut c = EtaColumns {
        nx,
        ny,
        uvzlev: vec![0.0; n3],
        wzlev: vec![0.0; n3],
        rhoh: vec![0.0; n3],
        prsh: vec![0.0; n3],
        pinmconv: vec![0.0; n3],
    };
    for jy in 0..ny {
        for ix in 0..nx {
            let ps = raw.ps.at(ix, jy);
            let mut tvold = surface_tv(raw, ix, jy);
            let mut pold = ps;
            let i1 = c.i(ix, jy, 0);
            c.uvzlev[i1] = 0.0;
            c.wzlev[i1] = 0.0;
            c.rhoh[i1] = pold / (R_AIR * tvold);
            c.prsh[i1] = ps;
            for kz in 2..=nuvz {
                let pint = hyb.akz[kz - 1] + hyb.bkz[kz - 1] * ps;
                let ik = c.i(ix, jy, kz - 1);
                c.prsh[ik] = pint;
                let tv = raw.tth.at(ix, jy, kz - 1) * (1.0 + 0.608 * raw.qvh.at(ix, jy, kz - 1));
                c.rhoh[ik] = pint / (R_AIR * tv);
                let zlow = c.uvzlev[c.i(ix, jy, kz - 2)];
                c.uvzlev[ik] = hypsometric(zlow, pold, pint, tv, tvold);
                tvold = tv;
                pold = pint;
            }
            for kz in 2..nwz {
                let v = (c.uvzlev[c.i(ix, jy, kz)] + c.uvzlev[c.i(ix, jy, kz - 1)]) / 2.0;
                let ik = c.i(ix, jy, kz - 1);
                c.wzlev[ik] = v;
            }
            let v = c.wzlev[c.i(ix, jy, nwz - 2)] + c.uvzlev[c.i(ix, jy, nuvz - 1)]
                - c.uvzlev[c.i(ix, jy, nuvz - 2)];
            let ik = c.i(ix, jy, nwz - 1);
            c.wzlev[ik] = v;

            let dp = |k: usize| hyb.aknew[k - 1] + hyb.bknew[k - 1] * ps;
            let v = c.uvzlev[c.i(ix, jy, 1)] / (dp(2) - dp(1));
            let ik = c.i(ix, jy, 0);
            c.pinmconv[ik] = v;
            for kz in 2..nz {
                let v = (c.uvzlev[c.i(ix, jy, kz)] - c.uvzlev[c.i(ix, jy, kz - 2)]) / (dp(kz + 1) - dp(kz - 1));
                let ik = c.i(ix, jy, kz - 1);
                c.pinmconv[ik] = v;
            }
            let v = (c.uvzlev[c.i(ix, jy, nz - 1)] - c.uvzlev[c.i(ix, jy, nz - 2)]) / (dp(nz) - dp(nz - 1));
            let ik = c.i(ix, jy, nz - 1);
            c.pinmconv[ik] = v;
        }
    }
    c
}

/// Which hybrid-level routine [`eta_transform`] is running.
#[derive(Clone, Copy, PartialEq)]
enum EtaKind {
    /// Mother grid: writes `prs`; slope factor `dxconst*cosf`.
    Ecmwf,
    /// Nest: no `prs`; slope factor `dxconst*xresoln*cosf`.
    Nest { xresoln: f64, yresoln: f64 },
}

/// The body shared by `verttransform_ecmwf` and `verttransform_nests`, up to
/// (not including) the polar part and the clouds.
fn eta_transform(
    kind: EtaKind,
    grid: &VertGrid,
    hyb: &HybridCoefficients,
    height: &[f64],
    clouds: CloudScheme,
    raw: &RawFields,
    out: &mut ZFields,
) -> EtaColumns {
    let g = &grid.geom;
    let (nx, ny) = (g.nx, g.ny);
    let (nuvz, nwz, nz) = (grid.nuvz, grid.nwz, grid.nz);
    let c = eta_columns(grid, hyb, raw);
    let ecmwf = kind == EtaKind::Ecmwf;

    for jy in 0..ny {
        for ix in 0..nx {
            // Levels where u, v, t and q are given.
            out.uu.set(ix, jy, 0, raw.uuh.at(ix, jy, 0));
            out.vv.set(ix, jy, 0, raw.vvh.at(ix, jy, 0));
            out.tt.set(ix, jy, 0, raw.tth.at(ix, jy, 0));
            out.qv.set(ix, jy, 0, raw.qvh.at(ix, jy, 0));
            if clouds.readclouds {
                out.clwc.set(ix, jy, 0, raw.clwch.at(ix, jy, 0));
                if !clouds.sumclouds {
                    out.ciwc.set(ix, jy, 0, raw.ciwch.at(ix, jy, 0));
                }
            }
            out.pv.set(ix, jy, 0, raw.pvh.at(ix, jy, 0));
            out.rho.set(ix, jy, 0, c.at(&c.rhoh, ix, jy, 0));
            if ecmwf {
                out.prs.set(ix, jy, 0, c.at(&c.prsh, ix, jy, 0));
            }
            let t = nz - 1;
            let u = nuvz - 1;
            out.uu.set(ix, jy, t, raw.uuh.at(ix, jy, u));
            out.vv.set(ix, jy, t, raw.vvh.at(ix, jy, u));
            out.tt.set(ix, jy, t, raw.tth.at(ix, jy, u));
            out.qv.set(ix, jy, t, raw.qvh.at(ix, jy, u));
            if clouds.readclouds {
                out.clwc.set(ix, jy, t, raw.clwch.at(ix, jy, u));
                if !clouds.sumclouds {
                    out.ciwc.set(ix, jy, t, raw.ciwch.at(ix, jy, u));
                }
            }
            out.pv.set(ix, jy, t, raw.pvh.at(ix, jy, u));
            out.rho.set(ix, jy, t, c.at(&c.rhoh, ix, jy, u));
            if ecmwf {
                out.prs.set(ix, jy, t, c.at(&c.prsh, ix, jy, u));
            }

            let top = c.at(&c.uvzlev, ix, jy, nuvz - 1);
            let mut idx = 2usize;
            for iz in 2..nz {
                let h = height[iz - 1];
                if h > top {
                    out.uu.set(ix, jy, iz - 1, out.uu.at(ix, jy, t));
                    out.vv.set(ix, jy, iz - 1, out.vv.at(ix, jy, t));
                    out.tt.set(ix, jy, iz - 1, out.tt.at(ix, jy, t));
                    out.qv.set(ix, jy, iz - 1, out.qv.at(ix, jy, t));
                    if clouds.readclouds {
                        out.clwc.set(ix, jy, iz - 1, out.clwc.at(ix, jy, t));
                        if !clouds.sumclouds {
                            out.ciwc.set(ix, jy, iz - 1, out.ciwc.at(ix, jy, t));
                        }
                    }
                    out.pv.set(ix, jy, iz - 1, out.pv.at(ix, jy, t));
                    out.rho.set(ix, jy, iz - 1, out.rho.at(ix, jy, t));
                    if ecmwf {
                        out.prs.set(ix, jy, iz - 1, out.prs.at(ix, jy, t));
                    }
                } else {
                    for kz in idx..=nuvz {
                        if idx <= kz
                            && h > c.at(&c.uvzlev, ix, jy, kz - 2)
                            && h <= c.at(&c.uvzlev, ix, jy, kz - 1)
                        {
                            idx = kz;
                            break;
                        }
                    }
                }
                if h <= top {
                    let kz = idx;
                    let dz1 = h - c.at(&c.uvzlev, ix, jy, kz - 2);
                    let dz2 = c.at(&c.uvzlev, ix, jy, kz - 1) - h;
                    let dz = dz1 + dz2;
                    let lin = |f: &Field3| (f.at(ix, jy, kz - 2) * dz2 + f.at(ix, jy, kz - 1) * dz1) / dz;
                    out.uu.set(ix, jy, iz - 1, lin(&raw.uuh));
                    out.vv.set(ix, jy, iz - 1, lin(&raw.vvh));
                    out.tt.set(ix, jy, iz - 1, lin(&raw.tth));
                    out.qv.set(ix, jy, iz - 1, lin(&raw.qvh));
                    if clouds.readclouds {
                        out.clwc.set(ix, jy, iz - 1, lin(&raw.clwch));
                        if !clouds.sumclouds {
                            out.ciwc.set(ix, jy, iz - 1, lin(&raw.ciwch));
                        }
                    }
                    out.pv.set(ix, jy, iz - 1, lin(&raw.pvh));
                    let r = (c.at(&c.rhoh, ix, jy, kz - 2) * dz2 + c.at(&c.rhoh, ix, jy, kz - 1) * dz1) / dz;
                    out.rho.set(ix, jy, iz - 1, r);
                    if ecmwf {
                        let p = (c.at(&c.prsh, ix, jy, kz - 2) * dz2 + c.at(&c.prsh, ix, jy, kz - 1) * dz1) / dz;
                        out.prs.set(ix, jy, iz - 1, p);
                    }
                }
            }

            // Levels where w is given.
            out.ww.set(ix, jy, 0, raw.wwh.at(ix, jy, 0) * c.at(&c.pinmconv, ix, jy, 0));
            out.ww.set(ix, jy, t, raw.wwh.at(ix, jy, nwz - 1) * c.at(&c.pinmconv, ix, jy, nz - 1));
            let mut idx = 2usize;
            for iz in 2..=nz {
                let h = height[iz - 1];
                for kz in idx..=nwz {
                    if idx <= kz && h > c.at(&c.wzlev, ix, jy, kz - 2) && h <= c.at(&c.wzlev, ix, jy, kz - 1) {
                        idx = kz;
                        break;
                    }
                }
                let kz = idx;
                let dz1 = h - c.at(&c.wzlev, ix, jy, kz - 2);
                let dz2 = c.at(&c.wzlev, ix, jy, kz - 1) - h;
                let dz = dz1 + dz2;
                let w = (raw.wwh.at(ix, jy, kz - 2) * c.at(&c.pinmconv, ix, jy, kz - 2) * dz2
                    + raw.wwh.at(ix, jy, kz - 1) * c.at(&c.pinmconv, ix, jy, kz - 1) * dz1)
                    / dz;
                out.ww.set(ix, jy, iz - 1, w);
            }
        }
    }

    // Density gradients.
    for jy in 0..ny {
        for ix in 0..nx {
            let d = (out.rho.at(ix, jy, 1) - out.rho.at(ix, jy, 0)) / (height[1] - height[0]);
            out.drhodz.set(ix, jy, 0, d);
            for kz in 2..nz {
                let d = (out.rho.at(ix, jy, kz) - out.rho.at(ix, jy, kz - 2)) / (height[kz] - height[kz - 2]);
                out.drhodz.set(ix, jy, kz - 1, d);
            }
            out.drhodz.set(ix, jy, nz - 1, out.drhodz.at(ix, jy, nz - 2));
        }
    }

    // Slope of the eta levels and the vertical-wind correction.
    if ny >= 3 && nx >= 3 {
        for jy in 1..ny - 1 {
            let cosf = 1.0 / ((jy as f64 * g.dy + g.ylat0) * PI180).cos();
            for ix in 1..nx - 1 {
                let mut idx = 2usize;
                for iz in 2..nz {
                    let h = height[iz - 1];
                    for kz in idx..=nz {
                        if idx <= kz
                            && h > c.at(&c.uvzlev, ix, jy, kz - 2)
                            && h <= c.at(&c.uvzlev, ix, jy, kz - 1)
                        {
                            idx = kz;
                            break;
                        }
                    }
                    let kz = idx;
                    let dz1 = h - c.at(&c.uvzlev, ix, jy, kz - 2);
                    let dz2 = c.at(&c.uvzlev, ix, jy, kz - 1) - h;
                    let dz = dz1 + dz2;
                    let zl = |i: usize, j: usize, k: usize| c.at(&c.uvzlev, i, j, k - 1);
                    let dzdx1 = (zl(ix + 1, jy, kz - 1) - zl(ix - 1, jy, kz - 1)) / 2.0;
                    let dzdx2 = (zl(ix + 1, jy, kz) - zl(ix - 1, jy, kz)) / 2.0;
                    let dzdx = (dzdx1 * dz2 + dzdx2 * dz1) / dz;
                    let dzdy1 = (zl(ix, jy + 1, kz - 1) - zl(ix, jy - 1, kz - 1)) / 2.0;
                    let dzdy2 = (zl(ix, jy + 1, kz) - zl(ix, jy - 1, kz)) / 2.0;
                    let dzdy = (dzdy1 * dz2 + dzdy2 * dz1) / dz;
                    let u = out.uu.at(ix, jy, iz - 1);
                    let v = out.vv.at(ix, jy, iz - 1);
                    let corr = match kind {
                        EtaKind::Ecmwf => dzdx * u * grid.dxconst * cosf + dzdy * v * grid.dyconst,
                        EtaKind::Nest { xresoln, yresoln } => {
                            dzdx * u * grid.dxconst * xresoln * cosf + dzdy * v * grid.dyconst * yresoln
                        }
                    };
                    out.ww.set(ix, jy, iz - 1, out.ww.at(ix, jy, iz - 1) + corr);
                }
            }
        }
    }
    let _ = nuvz;
    c
}

/// Cloud diagnostics of the hybrid-level routines (ECMWF lines 603–721;
/// nests 440–549). `bottom`: 2 (mother) or 1 (nest).
fn eta_clouds(
    grid: &VertGrid,
    height: &[f64],
    clouds: CloudScheme,
    raw: &RawFields,
    bottom: usize,
    cloudh_min: &mut Option<f64>,
    out: &mut ZFields,
) -> Result<(), VertError> {
    let (nx, ny, nz) = (grid.geom.nx, grid.geom.ny, grid.nz);
    if clouds.readclouds {
        for k in 0..nz {
            for jy in 0..ny {
                for ix in 0..nx {
                    out.clw.set(ix, jy, k, 0.0);
                    out.set_cloud(ix, jy, k, 0);
                    if !clouds.sumclouds {
                        let s = out.clwc.at(ix, jy, k) + out.ciwc.at(ix, jy, k);
                        out.clwc.set(ix, jy, k, s);
                    }
                }
            }
        }
        for jy in 0..ny {
            for ix in 0..nx {
                out.ctwc.set(ix, jy, 0.0);
            }
        }
        for jy in 0..ny {
            for ix in 0..nx {
                let lsp = raw.lsprec.at(ix, jy);
                let convp = raw.convprec.at(ix, jy);
                cloud_water_column(ix, jy, nz, height, lsp, convp, bottom, cloudh_min, out)?;
            }
        }
    } else {
        for jy in 0..ny {
            for ix in 0..nx {
                let lsp = raw.lsprec.at(ix, jy);
                let convp = raw.convprec.at(ix, jy);
                rh_cloud_column(ix, jy, nz, height, lsp, convp, out);
            }
        }
    }
    Ok(())
}

/// `verttransform_ecmwf(n, uuh, vvh, wwh, pvh)`: ECMWF hybrid levels to the
/// `z` grid, for one time slot.
///
/// * `saved` — the routine's `SAVE`d `init` (start with
///   [`SavedInit::default`]).
/// * `zgrid` — `com_mod` `height`/`nmixz`: written on the first call, read
///   always.
/// * `grid`, `hyb`, `polar` — the `gridcheck` settings.
/// * `clouds` — `readclouds`, `sumclouds`.
/// * `raw` — the raw fields of slot `n`.
/// * `out` — slot `n` of the `com_mod` output arrays, in/out.
///
/// On `Err` nothing upstream would have computed is defined; `out` may be
/// partly written.
///
/// # Errors
///
/// See [`VertError`].
#[allow(clippy::too_many_arguments)]
pub fn verttransform_ecmwf(
    saved: &mut SavedInit,
    zgrid: &mut ZGrid,
    grid: &VertGrid,
    hyb: &HybridCoefficients,
    polar: &PolarCaps,
    clouds: CloudScheme,
    raw: &RawFields,
    out: &mut ZFields,
) -> Result<(), VertError> {
    check_shapes(grid, hyb, zgrid, raw, out)?;
    check_polar(grid, polar)?;
    if saved.init {
        init_heights(grid, hyb, raw, zgrid)?;
        saved.init = false;
    }
    let height = zgrid.height.clone();
    eta_transform(EtaKind::Ecmwf, grid, hyb, &height, clouds, raw, out);
    polar_caps(grid, polar, false, out);
    let mut cloudh_min = None;
    eta_clouds(grid, &height, clouds, raw, 2, &mut cloudh_min, out)
}

/// One nest for [`verttransform_nests`].
#[derive(Debug, Clone, PartialEq)]
pub struct NestInput {
    /// The nest grid (`nxn`, `nyn`, `dxn`, `dyn`, `xlon0n`, `ylat0n`) with the
    /// **mother** grid's `dxconst`, `dyconst`, `nuvz`, `nwz`, `nz`.
    pub grid: VertGrid,
    /// `xresoln(l)`.
    pub xresoln: f64,
    /// `yresoln(l)`.
    pub yresoln: f64,
    /// `readclouds_nest(l)`, `sumclouds_nest(l)`.
    pub clouds: CloudScheme,
    /// Raw fields of the nest at slot `n`.
    pub raw: RawFields,
}

/// `verttransform_nests(n, uuhn, vvhn, wwhn, pvhn)`: every nest `l =
/// 1..numbnests` in order (`cloudh_min` carries from one nest to the next,
/// quirk 4). `outs[l]` is nest `l`'s slot `n`; its `prs`, `pplev`, `uupol`,
/// `vvpol` are not touched. Reads `height` (set by the mother-grid routine);
/// no `init`, no polar part.
///
/// # Errors
///
/// See [`VertError`]; also [`VertError::ShapeMismatch`] if `outs` is
/// shorter than `nests`.
pub fn verttransform_nests(
    height: &[f64],
    hyb: &HybridCoefficients,
    nests: &[NestInput],
    outs: &mut [ZFields],
) -> Result<(), VertError> {
    if outs.len() < nests.len() {
        return Err(VertError::ShapeMismatch);
    }
    let zgrid_view = ZGrid {
        height: height.to_vec(),
        nmixz: 0,
    };
    let mut cloudh_min = None;
    for (nest, out) in nests.iter().zip(outs.iter_mut()) {
        check_shapes(&nest.grid, hyb, &zgrid_view, &nest.raw, out)?;
        eta_transform(
            EtaKind::Nest {
                xresoln: nest.xresoln,
                yresoln: nest.yresoln,
            },
            &nest.grid,
            hyb,
            height,
            nest.clouds,
            &nest.raw,
            out,
        );
        eta_clouds(&nest.grid, height, nest.clouds, &nest.raw, 1, &mut cloudh_min, out)?;
    }
    Ok(())
}

/// GFS: the first level above ground, `llev` (lines 149–154).
fn gfs_llev(ps: f64, akz: &[f64], nuvz: usize) -> usize {
    let mut llev = 0;
    for i in 1..=nuvz {
        if ps < akz[i - 1] {
            llev = i;
        }
    }
    llev += 1;
    if llev > nuvz - 2 {
        llev = nuvz - 2;
    }
    llev
}

/// `verttransform_gfs(n, uuh, vvh, wwh, pvh)`: NCEP pressure levels to the
/// `z` grid, for one time slot. Arguments as [`verttransform_ecmwf`], with
/// [`GfsSaved`] for the `SAVE`d state and `pplev` written instead of `prs`.
///
/// # Errors
///
/// See [`VertError`].
#[allow(clippy::too_many_arguments)]
pub fn verttransform_gfs(
    saved: &mut GfsSaved,
    zgrid: &mut ZGrid,
    grid: &VertGrid,
    hyb: &HybridCoefficients,
    polar: &PolarCaps,
    clouds: CloudScheme,
    raw: &RawFields,
    out: &mut ZFields,
) -> Result<(), VertError> {
    check_shapes(grid, hyb, zgrid, raw, out)?;
    check_polar(grid, polar)?;
    let g = grid.geom;
    let (nx, ny) = (g.nx, g.ny);
    let (nuvz, nwz, nz) = (grid.nuvz, grid.nwz, grid.nz);
    if !f3_fits(&saved.uvwzlev, nx, ny, nz) {
        return Err(VertError::ShapeMismatch);
    }
    if saved.init {
        init_heights(grid, hyb, raw, zgrid)?;
        saved.init = false;
    }
    let height = zgrid.height.clone();
    let uvw = &mut saved.uvwzlev;

    // The scalars dz1, dz2, dz are shared by every loop of the routine and
    // can be read stale by the slope correction (quirk 9).
    let mut dz1 = 0.0_f64;
    let mut dz2 = 0.0_f64;
    let mut dz = 0.0_f64;
    let mut dz_set = false;
    let mut rhoh = vec![0.0; nuvz];
    let mut wzlev = vec![0.0; nuvz.max(nwz)];
    let mut pinmconv = vec![0.0; nz];

    for jy in 0..ny {
        for ix in 0..nx {
            let ps = raw.ps.at(ix, jy);
            let llev = gfs_llev(ps, &hyb.akz, nuvz);

            let mut tvold = raw.tth.at(ix, jy, llev - 1) * (1.0 + 0.608 * raw.qvh.at(ix, jy, llev - 1));
            let mut pold = hyb.akz[llev - 1];
            wzlev[llev - 1] = 0.0;
            uvw.set(ix, jy, llev - 1, 0.0);
            rhoh[llev - 1] = pold / (R_AIR * tvold);
            for kz in llev + 1..=nuvz {
                let pint = hyb.akz[kz - 1] + hyb.bkz[kz - 1] * ps;
                let tv = raw.tth.at(ix, jy, kz - 1) * (1.0 + 0.608 * raw.qvh.at(ix, jy, kz - 1));
                rhoh[kz - 1] = pint / (R_AIR * tv);
                let z = hypsometric(uvw.at(ix, jy, kz - 2), pold, pint, tv, tvold);
                uvw.set(ix, jy, kz - 1, z);
                wzlev[kz - 1] = z;
                tvold = tv;
                pold = pint;
            }

            let dp = |k: usize| hyb.aknew[k - 1] + hyb.bknew[k - 1] * ps;
            pinmconv[llev - 1] =
                (uvw.at(ix, jy, llev) - uvw.at(ix, jy, llev - 1)) / (dp(llev + 1) - dp(llev));
            for kz in llev + 1..nz {
                pinmconv[kz - 1] = (uvw.at(ix, jy, kz) - uvw.at(ix, jy, kz - 2)) / (dp(kz + 1) - dp(kz - 1));
            }
            pinmconv[nz - 1] = (uvw.at(ix, jy, nz - 1) - uvw.at(ix, jy, nz - 2)) / (dp(nz) - dp(nz - 1));

            let l = llev - 1;
            let t = nz - 1;
            let u = nuvz - 1;
            out.uu.set(ix, jy, 0, raw.uuh.at(ix, jy, l));
            out.vv.set(ix, jy, 0, raw.vvh.at(ix, jy, l));
            out.tt.set(ix, jy, 0, raw.tth.at(ix, jy, l));
            out.qv.set(ix, jy, 0, raw.qvh.at(ix, jy, l));
            if clouds.readclouds {
                out.clwc.set(ix, jy, 0, raw.clwch.at(ix, jy, l));
            }
            out.pv.set(ix, jy, 0, raw.pvh.at(ix, jy, l));
            out.rho.set(ix, jy, 0, rhoh[l]);
            out.pplev.set(ix, jy, 0, hyb.akz[l]);
            out.uu.set(ix, jy, t, raw.uuh.at(ix, jy, u));
            out.vv.set(ix, jy, t, raw.vvh.at(ix, jy, u));
            out.tt.set(ix, jy, t, raw.tth.at(ix, jy, u));
            out.qv.set(ix, jy, t, raw.qvh.at(ix, jy, u));
            if clouds.readclouds {
                out.clwc.set(ix, jy, t, raw.clwch.at(ix, jy, u));
            }
            out.pv.set(ix, jy, t, raw.pvh.at(ix, jy, u));
            out.rho.set(ix, jy, t, rhoh[u]);
            out.pplev.set(ix, jy, t, hyb.akz[u]);

            let kmin = llev + 1;
            for iz in 2..nz {
                let h = height[iz - 1];
                for kz in kmin..=nuvz {
                    if h > uvw.at(ix, jy, nuvz - 1) {
                        out.uu.set(ix, jy, iz - 1, out.uu.at(ix, jy, t));
                        out.vv.set(ix, jy, iz - 1, out.vv.at(ix, jy, t));
                        out.tt.set(ix, jy, iz - 1, out.tt.at(ix, jy, t));
                        out.qv.set(ix, jy, iz - 1, out.qv.at(ix, jy, t));
                        if clouds.readclouds {
                            out.clwc.set(ix, jy, iz - 1, out.clwc.at(ix, jy, t));
                        }
                        out.pv.set(ix, jy, iz - 1, out.pv.at(ix, jy, t));
                        out.rho.set(ix, jy, iz - 1, out.rho.at(ix, jy, t));
                        out.pplev.set(ix, jy, iz - 1, out.pplev.at(ix, jy, t));
                        break; // goto 30
                    }
                    if h > uvw.at(ix, jy, kz - 2) && h <= uvw.at(ix, jy, kz - 1) {
                        dz1 = h - uvw.at(ix, jy, kz - 2);
                        dz2 = uvw.at(ix, jy, kz - 1) - h;
                        dz = dz1 + dz2;
                        dz_set = true;
                        let (a, b, d) = (dz1, dz2, dz);
                        let lin = |f: &Field3| (f.at(ix, jy, kz - 2) * b + f.at(ix, jy, kz - 1) * a) / d;
                        out.uu.set(ix, jy, iz - 1, lin(&raw.uuh));
                        out.vv.set(ix, jy, iz - 1, lin(&raw.vvh));
                        out.tt.set(ix, jy, iz - 1, lin(&raw.tth));
                        out.qv.set(ix, jy, iz - 1, lin(&raw.qvh));
                        if clouds.readclouds {
                            out.clwc.set(ix, jy, iz - 1, lin(&raw.clwch));
                        }
                        out.pv.set(ix, jy, iz - 1, lin(&raw.pvh));
                        out.rho.set(ix, jy, iz - 1, (rhoh[kz - 2] * b + rhoh[kz - 1] * a) / d);
                        out.pplev
                            .set(ix, jy, iz - 1, (hyb.akz[kz - 2] * b + hyb.akz[kz - 1] * a) / d);
                    }
                }
            }

            out.ww.set(ix, jy, 0, raw.wwh.at(ix, jy, l) * pinmconv[l]);
            out.ww.set(ix, jy, t, raw.wwh.at(ix, jy, nwz - 1) * pinmconv[nz - 1]);
            for iz in 2..=nz {
                let h = height[iz - 1];
                for kz in kmin..=nwz {
                    if h > wzlev[kz - 2] && h <= wzlev[kz - 1] {
                        dz1 = h - wzlev[kz - 2];
                        dz2 = wzlev[kz - 1] - h;
                        dz = dz1 + dz2;
                        dz_set = true;
                        let w = (raw.wwh.at(ix, jy, kz - 2) * pinmconv[kz - 2] * dz2
                            + raw.wwh.at(ix, jy, kz - 1) * pinmconv[kz - 1] * dz1)
                            / dz;
                        out.ww.set(ix, jy, iz - 1, w);
                    }
                }
            }

            let d = (out.rho.at(ix, jy, 1) - out.rho.at(ix, jy, 0)) / (height[1] - height[0]);
            out.drhodz.set(ix, jy, 0, d);
            for kz in 2..nz {
                let d = (out.rho.at(ix, jy, kz) - out.rho.at(ix, jy, kz - 2)) / (height[kz] - height[kz - 2]);
                out.drhodz.set(ix, jy, kz - 1, d);
            }
            out.drhodz.set(ix, jy, nz - 1, out.drhodz.at(ix, jy, nz - 2));
        }
    }

    // Slope correction.
    let mut kl: Option<usize> = None;
    let mut klp = 0usize;
    if ny >= 3 && nx >= 3 {
        for jy in 1..ny - 1 {
            let cosf = ((jy as f64 * g.dy + g.ylat0) * PI180).cos();
            for ix in 1..nx - 1 {
                let llev = gfs_llev(raw.ps.at(ix, jy), &hyb.akz, nuvz);
                let kmin = llev + 1;
                for iz in 2..nz {
                    let h = height[iz - 1];
                    let ui = out.uu.at(ix, jy, iz - 1) * grid.dxconst / cosf;
                    let vi = out.vv.at(ix, jy, iz - 1) * grid.dyconst;
                    for kz in kmin..=nz {
                        if h > uvw.at(ix, jy, kz - 2) && h <= uvw.at(ix, jy, kz - 1) {
                            dz1 = h - uvw.at(ix, jy, kz - 2);
                            dz2 = uvw.at(ix, jy, kz - 1) - h;
                            dz = dz1 + dz2;
                            dz_set = true;
                            kl = Some(kz - 1);
                            klp = kz;
                            break; // goto 47
                        }
                    }
                    let k0 = kl.ok_or(VertError::UndefinedSlopeBracket)?;
                    if !dz_set {
                        return Err(VertError::UndefinedSlopeBracket);
                    }
                    let zl = |i: usize, j: usize, k: usize| uvw.at(i, j, k - 1);
                    let dzdx1 = (zl(ix + 1, jy, k0) - zl(ix - 1, jy, k0)) / 2.0;
                    let dzdx2 = (zl(ix + 1, jy, klp) - zl(ix - 1, jy, klp)) / 2.0;
                    let dzdx = (dzdx1 * dz2 + dzdx2 * dz1) / dz;
                    let dzdy1 = (zl(ix, jy + 1, k0) - zl(ix, jy - 1, k0)) / 2.0;
                    let dzdy2 = (zl(ix, jy + 1, klp) - zl(ix, jy - 1, klp)) / 2.0;
                    let dzdy = (dzdy1 * dz2 + dzdy2 * dz1) / dz;
                    out.ww.set(ix, jy, iz - 1, out.ww.at(ix, jy, iz - 1) + (dzdx * ui + dzdy * vi));
                }
            }
        }
    }

    polar_caps(grid, polar, true, out);

    // Clouds.
    if clouds.readclouds {
        for k in 0..nz {
            for jy in 0..ny {
                for ix in 0..nx {
                    out.clw.set(ix, jy, k, 0.0);
                    out.set_cloud(ix, jy, k, 0);
                }
            }
        }
        for jy in 0..ny {
            for ix in 0..nx {
                out.ctwc.set(ix, jy, 0.0);
            }
        }
        let mut cloudh_min = None;
        for jy in 0..ny {
            for ix in 0..nx {
                let lsp = raw.lsprec.at(ix, jy);
                let convp = raw.convprec.at(ix, jy);
                cloud_water_column(ix, jy, nz, &height, lsp, convp, 2, &mut cloudh_min, out)?;
            }
        }
    } else {
        for jy in 0..ny {
            for ix in 0..nx {
                let lsp = raw.lsprec.at(ix, jy);
                let convp = raw.convprec.at(ix, jy);
                rh_cloud_column(ix, jy, nz, &height, lsp, convp, out);
            }
        }
    }
    Ok(())
}
