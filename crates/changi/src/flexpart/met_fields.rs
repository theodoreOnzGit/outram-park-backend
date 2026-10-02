// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/calcpar.f90, src/calcpar_nests.f90,
//                    src/calcpv.f90, src/calcpv_nests.f90
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
//                    (calcpar: A. Stohl, P. Seibert, C. Forster, B. C. Krueger,
//                    M. Harustak; calcpv: P. James, A. Stohl)
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Per-grid-point boundary-layer parameters (`calcpar`) and potential
//! vorticity on the 3-d grid (`calcpv`).
//!
//! | Function | Upstream | Computes |
//! |---|---|---|
//! | [`calcpar`] | `calcpar.f90`, `calcpar_nests.f90` | `u*`, `1/L`, mixing height, `w*`, dry deposition velocity, thermal tropopause, then PV |
//! | [`calcpv`] | `calcpv.f90`, `calcpv_nests.f90` | Ertel potential vorticity on the model levels, PVU |
//!
//! # One function per routine pair
//!
//! `calcpar_nests.f90` and `calcpv_nests.f90` are copies of the mother-grid
//! routines on the nest arrays. The port has one function each that takes the
//! grid as an argument, and the code-to-code test drives it against **both**
//! upstream versions. The copies differ in exactly these ways, all
//! reproduced through arguments:
//!
//! - `calcpar_nests` has **no NCEP branch**: it always calls `obukhov` with
//!   `tthn(.,.,2)` and a never-assigned `dummyakzllev`, always lets
//!   `richardson` write the mixing height and always starts the level-height
//!   loop at 2. For ECMWF input that is identical to `calcpar`'s ECMWF branch.
//!   For NCEP input `obukhov` would read the undefined `dummyakzllev`, so
//!   [`calcpar`] **refuses** [`GridKind::Nest`] with
//!   [`MetDataFormat::Ncep`] ([`MetFieldsError::NestWithNcep`]).
//! - `calcpar_nests` calls `getvdep_nests`, which computes its latitude as
//!   `jy*dy + ylat0` — the **mother grid's** `dy` and `ylat0` with the
//!   **nest's** row index. That is an upstream defect (a nest at 30–32°N on a
//!   mother grid starting at 60°S is given southern-hemisphere seasons); the
//!   port reproduces it by taking the season latitude origin and spacing as
//!   separate arguments ([`DryDepositionInput::season_lat0`],
//!   [`DryDepositionInput::season_dy`]). A caller porting a nest passes the
//!   mother grid's values, as upstream does.
//! - `calcpv_nests` has no global-domain branches: pass
//!   [`GlobalDomain::LIMITED`].
//!
//! # Upstream quirks reproduced (and documented)
//!
//! - **`L` passed to `getvdep` is `1/(1/L)`**, not `L`: upstream stores
//!   `oli = 1/ol` and calls `getvdep(..., 1./oli, ...)`. The double reciprocal
//!   is kept (it can differ from `ol` in the last bit).
//! - **`z0(7)` is overwritten in `com_mod`** at every grid point with the
//!   Charnock water roughness `0.016 u*^2 / g`; the port passes the modified
//!   table to `getvdep` and reports the last value written
//!   ([`CalcparOutput::z0_water_last`]).
//! - **`ustar` is floored at `1e-8`** before everything else.
//! - **Tropopause left unassigned.** If no layer satisfies the thermal
//!   criterion, `tropopause(ix,jy)` keeps whatever it held before the call
//!   (the previous meteorological field). The port returns `None` there.
//! - **Stale `kzmin`.** `kzmin` (the lowest level searched for the
//!   tropopause) is assigned only when some level reaches the minimum
//!   altitude `altmin`. Otherwise upstream reuses the value from the
//!   **previous column** of the same call. The port carries it across columns
//!   in upstream's loop order (`jy` outer, `ix` inner); if the first column
//!   has none, the value is undefined and the port returns `None` for that
//!   column's tropopause.
//! - **`zlev(1)` is never assigned in the ECMWF branch** (the height loop
//!   starts at level 2) but the `kzmin` search starts at level 1, so upstream
//!   compares an uninitialised stack value with `altmin`. The port starts the
//!   search at level 2. That equals upstream whenever the stale value is below
//!   `altmin` (>= 2500 m) or NaN — e.g. zero; it was so in the reference run,
//!   where the port matches upstream on every tropopause. A different stack
//!   history could make upstream pick level 1; nothing in FLEXPART guards it.
//! - **NCEP `llev`.** `llev` is one above the last level whose pressure
//!   exceeds `ps`, set to `nuvz-1` if that is above the top.
//!
//! # Units
//!
//! Bare `f64` in FLEXPART units: Pa, K, kg/kg, m/s, W/m², mm/h, m; PV in PVU
//! (`1e-6 K m² kg⁻¹ s⁻¹`), sign as upstream (positive in the northern
//! hemisphere for a stable atmosphere).

use super::boundary_layer::richardson;
use super::constants::{GA, HMIXMAX, HMIXMIN, KAPPA, NUMCLASS, PI, R_AIR, R_EARTH};
use super::dry_deposition::{getvdep, DepositionSpecies, SurfaceMet, SurfaceResistances, NSEASON};
use super::surface_layer::{obukhov, scalev, MetDataFormat};
use super::thermo::ew_kelvin;

/// A 2-d field on the horizontal grid, stored as upstream does (`ix`
/// fastest): element `(ix, jy)` is `data[ix + nx*jy]`, both 0-based like
/// upstream's `0:nxmax-1` bounds.
#[derive(Debug, Clone, PartialEq)]
pub struct Field2 {
    /// Points in x.
    pub nx: usize,
    /// Points in y.
    pub ny: usize,
    /// Values, `ix` fastest.
    pub data: Vec<f64>,
}

impl Field2 {
    /// A field filled with `v`.
    #[must_use]
    pub fn filled(nx: usize, ny: usize, v: f64) -> Self {
        Self {
            nx,
            ny,
            data: vec![v; nx * ny],
        }
    }

    /// Value at `(ix, jy)`.
    #[must_use]
    pub fn at(&self, ix: usize, jy: usize) -> f64 {
        self.data[ix + self.nx * jy]
    }

    /// Set the value at `(ix, jy)`.
    pub fn set(&mut self, ix: usize, jy: usize, v: f64) {
        self.data[ix + self.nx * jy] = v;
    }
}

/// A 3-d field: element `(ix, jy, k)` is `data[ix + nx*(jy + ny*k)]`, with
/// `k` 0-based (upstream's level `k` is index `k-1`).
#[derive(Debug, Clone, PartialEq)]
pub struct Field3 {
    /// Points in x.
    pub nx: usize,
    /// Points in y.
    pub ny: usize,
    /// Levels.
    pub nz: usize,
    /// Values, `ix` fastest, then `jy`, then `k`.
    pub data: Vec<f64>,
}

impl Field3 {
    /// A field filled with `v`.
    #[must_use]
    pub fn filled(nx: usize, ny: usize, nz: usize, v: f64) -> Self {
        Self {
            nx,
            ny,
            nz,
            data: vec![v; nx * ny * nz],
        }
    }

    /// Value at `(ix, jy, k)`, `k` 0-based.
    #[must_use]
    pub fn at(&self, ix: usize, jy: usize, k: usize) -> f64 {
        self.data[ix + self.nx * (jy + self.ny * k)]
    }

    /// Set the value at `(ix, jy, k)`, `k` 0-based.
    pub fn set(&mut self, ix: usize, jy: usize, k: usize, v: f64) {
        self.data[ix + self.nx * (jy + self.ny * k)] = v;
    }

    /// The column at `(ix, jy)`, levels 0-based.
    #[must_use]
    pub fn column(&self, ix: usize, jy: usize) -> Vec<f64> {
        (0..self.nz).map(|k| self.at(ix, jy, k)).collect()
    }
}

/// Horizontal geometry of a meteorological grid (mother or nest).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridGeometry {
    /// Points in x (`nx` / `nxn(l)`).
    pub nx: usize,
    /// Points in y (`ny` / `nyn(l)`).
    pub ny: usize,
    /// Grid spacing in x, degrees.
    pub dx: f64,
    /// Grid spacing in y, degrees.
    pub dy: f64,
    /// Longitude of the lower-left point, degrees.
    pub xlon0: f64,
    /// Latitude of the lower-left point, degrees.
    pub ylat0: f64,
}

/// Which global-domain special cases [`calcpv`] applies (`xglobal`,
/// `sglobal`, `nglobal` in `com_mod`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalDomain {
    /// The grid wraps in longitude; column `nx-1` duplicates column 0, so the
    /// west neighbour of column 0 is column `nx-2`.
    pub xglobal: bool,
    /// Row 0 is the South Pole.
    pub sglobal: bool,
    /// Row `ny-1` is the North Pole.
    pub nglobal: bool,
}

impl GlobalDomain {
    /// A limited-area grid (and every nest: `calcpv_nests` has no global
    /// branches).
    pub const LIMITED: Self = Self {
        xglobal: false,
        sglobal: false,
        nglobal: false,
    };
}

/// Mother grid or nest (selects `calcpar` or `calcpar_nests` behaviour).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridKind {
    /// `calcpar.f90`.
    Mother,
    /// `calcpar_nests.f90` (ECMWF only, see the module docs).
    Nest,
}

/// Why [`calcpar`] / [`calcpv`] refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetFieldsError {
    /// `calcpar_nests` with NCEP input reads the never-assigned
    /// `dummyakzllev`; upstream's result is undefined.
    NestWithNcep,
    /// A finite-difference stencil in [`calcpv`] has fewer than two points
    /// (`nx < 2`, `ny < 2`, or a 3-row grid that is both `sglobal` and
    /// `nglobal`); upstream then reads an unassigned `vx(2)` / `uy(2)`.
    DegenerateStencil,
    /// Field dimensions disagree with the grid geometry.
    ShapeMismatch,
}

/// The inputs `calcpar` reads from `com_mod` at one time index `n`.
#[derive(Debug, Clone, PartialEq)]
pub struct CalcparInput {
    /// Surface pressure, Pa (`ps`).
    pub ps: Field2,
    /// 2 m temperature, K (`tt2`).
    pub tt2: Field2,
    /// 2 m dew point, K (`td2`).
    pub td2: Field2,
    /// Surface stress, N/m² (`surfstr`).
    pub surfstr: Field2,
    /// Surface sensible heat flux, W/m² (`sshf`; negative = upward).
    pub sshf: Field2,
    /// Surface solar radiation, W/m² (`ssr`).
    pub ssr: Field2,
    /// Large-scale precipitation, mm/h (`lsprec`).
    pub lsprec: Field2,
    /// Convective precipitation, mm/h (`convprec`).
    pub convprec: Field2,
    /// Snow depth, m water equivalent (`sd`).
    pub sd: Field2,
    /// Sub-grid orography excess, m (`excessoro`).
    pub excessoro: Field2,
    /// Mixing height read by `readwind` for NCEP input, m (`hmix` on entry).
    /// Ignored for ECMWF, where `richardson` computes it.
    pub hmix_in: Field2,
    /// Temperature on the model levels, K (`tth`, `nuvz` levels).
    pub tth: Field3,
    /// Specific humidity on the model levels, kg/kg (`qvh`).
    pub qvh: Field3,
    /// Wind components on the model levels, m/s (`uuh`, `vvh`).
    pub uuh: Field3,
    /// See [`CalcparInput::uuh`].
    pub vvh: Field3,
    /// Hybrid coefficient `A` of each model level, Pa (`akz`).
    pub akz: Vec<f64>,
    /// Hybrid coefficient `B` of each model level (`bkz`).
    pub bkz: Vec<f64>,
}

/// Dry deposition inputs (`DRYDEP` true). Upstream reads all of it from
/// `com_mod`; the Wesely tables are data, never shipped here.
#[derive(Debug, Clone, PartialEq)]
pub struct DryDepositionInput {
    /// Julian date of the field, `bdate + wftime(n)/86400`.
    pub jul: f64,
    /// Latitude origin `getvdep` uses for the season, degrees: `ylat0` of the
    /// **mother** grid, also for a nest (upstream defect, module docs).
    pub season_lat0: f64,
    /// Latitude spacing `getvdep` uses for the season, degrees: `dy` of the
    /// mother grid, also for a nest.
    pub season_dy: f64,
    /// Landuse fractions of each column (`xlanduse(ix,jy,:)`), index
    /// `ix + nx*jy`.
    pub landuse: Vec<[f64; NUMCLASS]>,
    /// Roughness length of each class, m (`z0`); class 7 (index 6) is
    /// overwritten at every point.
    pub z0: [f64; NUMCLASS],
    /// Species, each with its Wesely table `[season][class]`.
    pub species: Vec<(DepositionSpecies, [[SurfaceResistances; NUMCLASS]; NSEASON])>,
}

/// What [`calcpar`] computes.
#[derive(Debug, Clone, PartialEq)]
pub struct CalcparOutput {
    /// Friction velocity, m/s (`ustar`).
    pub ustar: Field2,
    /// Inverse Obukhov length, 1/m (`oli`; `99999` when `L == 0`).
    pub oli: Field2,
    /// Mixing height, m (`hmix`), clamped to `[hmixmin, hmixmax]`.
    pub hmix: Field2,
    /// Convective velocity scale, m/s (`wstar`).
    pub wstar: Field2,
    /// Dry deposition velocity per species, m/s (`vdep`); empty without
    /// dry deposition.
    pub vdep: Vec<Field2>,
    /// Thermal tropopause height, m, index `ix + nx*jy`; `None` where
    /// upstream leaves it unassigned (see the module docs).
    pub tropopause: Vec<Option<f64>>,
    /// The last value written to `z0(7)` (`None` without dry deposition).
    pub z0_water_last: Option<f64>,
    /// Potential vorticity from [`calcpv`], PVU.
    pub pv: Field3,
}

/// Options selecting upstream's branches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CalcparOptions {
    /// ECMWF (with `akm`, `bkm` of the lowest two half levels) or NCEP.
    pub format: MetDataFormat,
    /// Mother grid or nest.
    pub kind: GridKind,
    /// `lsubgrid == 1`: add `min(excessoro, hmixplus)` to the mixing height.
    pub lsubgrid: bool,
    /// Global-domain flags for [`calcpv`] (nests: [`GlobalDomain::LIMITED`]).
    pub domain: GlobalDomain,
}

fn shape_ok2(f: &Field2, g: &GridGeometry) -> bool {
    f.nx == g.nx && f.ny == g.ny && f.data.len() == g.nx * g.ny
}

fn shape_ok3(f: &Field3, g: &GridGeometry, nuvz: usize) -> bool {
    f.nx == g.nx && f.ny == g.ny && f.nz == nuvz && f.data.len() == g.nx * g.ny * nuvz
}

/// `calcpar.f90` / `calcpar_nests.f90`: boundary-layer parameters over the
/// whole grid at one time, followed by [`calcpv`].
///
/// Per column, in upstream's order: `u* = scalev(...)` floored at `1e-8`;
/// `L = obukhov(...)` and `oli = 1/L` (or `99999` for `L == 0`);
/// `richardson` for `h`, `w*` and `hmixplus` (for NCEP `h` is the value read
/// in, [`CalcparInput::hmix_in`]); `h += min(excessoro, hmixplus)` when
/// `lsubgrid`; `h` clamped to `[100, 4500]` m; the dry deposition velocity
/// with `z0(7) = 0.016 u*^2/g` and `rh = e(Td)/e(T)`; and the thermal
/// tropopause of Hoinka (1997): the first level above `altmin` (5000 m in
/// the tropics, 2500 m poleward of 40°, linear between) from which the
/// temperature falls by less than 2 K/km over the next 2000 m.
///
/// See the module docs for every upstream quirk this reproduces.
///
/// # Errors
/// [`MetFieldsError::NestWithNcep`], [`MetFieldsError::ShapeMismatch`], and
/// those of [`calcpv`].
pub fn calcpar(
    geom: &GridGeometry,
    opts: &CalcparOptions,
    input: &CalcparInput,
    drydep: Option<&DryDepositionInput>,
) -> Result<CalcparOutput, MetFieldsError> {
    let ncep = matches!(opts.format, MetDataFormat::Ncep);
    if ncep && opts.kind == GridKind::Nest {
        return Err(MetFieldsError::NestWithNcep);
    }
    let nuvz = input.akz.len();
    let (nx, ny) = (geom.nx, geom.ny);
    let two_d = [
        &input.ps,
        &input.tt2,
        &input.td2,
        &input.surfstr,
        &input.sshf,
        &input.ssr,
        &input.lsprec,
        &input.convprec,
        &input.sd,
        &input.excessoro,
        &input.hmix_in,
    ];
    let three_d = [&input.tth, &input.qvh, &input.uuh, &input.vvh];
    if input.bkz.len() != nuvz
        || nuvz < 2
        || !two_d.iter().all(|f| shape_ok2(f, geom))
        || !three_d.iter().all(|f| shape_ok3(f, geom, nuvz))
        || drydep.is_some_and(|d| d.landuse.len() != nx * ny)
    {
        return Err(MetFieldsError::ShapeMismatch);
    }

    let konst = R_AIR / GA;
    let mut out = CalcparOutput {
        ustar: Field2::filled(nx, ny, 0.0),
        oli: Field2::filled(nx, ny, 0.0),
        hmix: Field2::filled(nx, ny, 0.0),
        wstar: Field2::filled(nx, ny, 0.0),
        vdep: drydep.map_or_else(Vec::new, |d| {
            vec![Field2::filled(nx, ny, 0.0); d.species.len()]
        }),
        tropopause: vec![None; nx * ny],
        z0_water_last: None,
        pv: Field3::filled(nx, ny, nuvz, 0.0),
    };
    let mut z0 = drydep.map_or([0.0; NUMCLASS], |d| d.z0);
    // Carried across columns exactly as upstream's uninitialised local is.
    let mut kzmin: Option<usize> = None;
    let mut zlev = vec![0.0_f64; nuvz + 1]; // 1-based

    for jy in 0..ny {
        // Minimum height for the tropopause.
        let ylat = geom.ylat0 + jy as f64 * geom.dy;
        let altmin = if (-20.0..=20.0).contains(&ylat) {
            5000.0
        } else if ylat > 20.0 && ylat < 40.0 {
            2500.0 + (40.0 - ylat) * 125.0
        } else if ylat > -40.0 && ylat < -20.0 {
            2500.0 + (40.0 + ylat) * 125.0
        } else {
            2500.0
        };

        for ix in 0..nx {
            let ps = input.ps.at(ix, jy);
            let tt2 = input.tt2.at(ix, jy);
            let td2 = input.td2.at(ix, jy);
            let sshf = input.sshf.at(ix, jy);
            let tth = |k: usize| input.tth.at(ix, jy, k - 1);
            let qvh = |k: usize| input.qvh.at(ix, jy, k - 1);

            // 1) friction velocity
            let mut ustar = scalev(ps, tt2, td2, input.surfstr.at(ix, jy));
            if ustar <= 1.0e-8 {
                ustar = 1.0e-8;
            }
            out.ustar.set(ix, jy, ustar);

            // 2) inverse Obukhov length
            let mut llev = 0usize;
            let ol = if ncep {
                for i in 1..=nuvz {
                    if ps < input.akz[i - 1] {
                        llev = i;
                    }
                }
                llev += 1;
                if llev > nuvz {
                    llev = nuvz - 1;
                }
                obukhov(
                    ps,
                    tt2,
                    td2,
                    tth(llev),
                    ustar,
                    sshf,
                    input.akz[llev - 1],
                    opts.format,
                )
            } else {
                // level pressure unused for ECMWF (rebuilt from akm/bkm)
                obukhov(ps, tt2, td2, tth(2), ustar, sshf, 0.0, opts.format)
            };
            let oli = if ol != 0.0 { 1.0 / ol } else { 99999.0 };
            out.oli.set(ix, jy, oli);

            // 3) convective velocity scale and mixing height
            let ulev = input.uuh.column(ix, jy);
            let vlev = input.vvh.column(ix, jy);
            let ttlev = input.tth.column(ix, jy);
            let qvlev = input.qvh.column(ix, jy);
            let m = richardson(
                ps,
                ustar,
                &input.akz,
                &input.bkz,
                &ttlev,
                &qvlev,
                &ulev,
                &vlev,
                sshf,
                tt2,
                td2,
                &opts.format,
            );
            let mut hmix = if ncep { input.hmix_in.at(ix, jy) } else { m.h };
            out.wstar.set(ix, jy, m.wst);
            let subsceff = if opts.lsubgrid {
                input.excessoro.at(ix, jy).min(m.hmixplus)
            } else {
                0.0
            };
            hmix += subsceff;
            hmix = HMIXMIN.max(hmix);
            hmix = HMIXMAX.min(hmix);
            out.hmix.set(ix, jy, hmix);

            // 4) dry deposition velocities
            if let Some(d) = drydep {
                z0[6] = 0.016 * ustar * ustar / GA;
                out.z0_water_last = Some(z0[6]);
                let rh = ew_kelvin(td2) / ew_kelvin(tt2);
                let met = SurfaceMet {
                    ust: ustar,
                    temp: tt2,
                    pa: ps,
                    ol: 1.0 / oli,
                    gr: input.ssr.at(ix, jy),
                    rh,
                    rr: input.lsprec.at(ix, jy) + input.convprec.at(ix, jy),
                    snow: input.sd.at(ix, jy),
                };
                // getvdep: ylat = jy*dy + ylat0 (mother grid's, see docs)
                let season_lat = jy as f64 * d.season_dy + d.season_lat0;
                let landuse = &d.landuse[ix + nx * jy];
                for (i, (species, table)) in d.species.iter().enumerate() {
                    let v = getvdep(d.jul, season_lat, &met, landuse, &z0, table, species);
                    out.vdep[i].set(ix, jy, v);
                }
            }

            // Thermal tropopause (Hoinka, 1997)
            // 1) altitudes of the model levels
            let mut tvold = tt2 * (1.0 + 0.378 * ew_kelvin(td2) / ps);
            let mut pold = ps;
            let mut zold = 0.0;
            let loop_start = if ncep { llev } else { 2 };
            // 1-based level loop kept index-for-index with upstream
            #[allow(clippy::needless_range_loop)]
            for kz in loop_start..=nuvz {
                let pint = input.akz[kz - 1] + input.bkz[kz - 1] * ps;
                let tv = tth(kz) * (1.0 + 0.608 * qvh(kz));
                zlev[kz] = if (tv - tvold).abs() > 0.2 {
                    zold + konst * (pold / pint).ln() * (tv - tvold) / (tv / tvold).ln()
                } else {
                    zold + konst * (pold / pint).ln() * tv
                };
                tvold = tv;
                pold = pint;
                zold = zlev[kz];
            }

            // 2) lowest level searched (upstream starts the ECMWF search at
            //    the never-assigned zlev(1); see the module docs)
            if let Some(kz) = (loop_start..=nuvz).find(|&kz| zlev[kz] >= altmin) {
                kzmin = Some(kz);
            }

            // 3) first stable layer above kzmin
            let mut trop = None;
            if let Some(kzmin) = kzmin {
                'search: for kz in kzmin..=nuvz {
                    for lz in kz + 1..=nuvz {
                        if zlev[lz] - zlev[kz] > 2000.0 {
                            if (tth(kz) - tth(lz)) / (zlev[lz] - zlev[kz]) < 0.002 {
                                trop = Some(zlev[kz]);
                                break 'search;
                            }
                            continue 'search;
                        }
                    }
                }
            }
            out.tropopause[ix + nx * jy] = trop;
        }
    }

    out.pv = calcpv(
        geom,
        opts.domain,
        &input.akz,
        &input.bkz,
        &input.ps,
        &input.tth,
        &input.uuh,
        &input.vvh,
    )?;
    Ok(out)
}

/// Spiral search of upstream's `calcpv`: from level `klpt`, alternately one
/// level up and one level down, for the layer `[k, k+1]` of a neighbouring
/// column whose potential temperature brackets `theta`; returns the wind
/// interpolated to that `theta`, or `None` after `nlck` layers.
///
/// `th(k)` and `w(k)` are 1-based level lookups in the neighbouring column.
/// The control flow is upstream's `goto 40 / 41` loop transcribed; it
/// terminates because `nlck = nuvz/3 <= nuvz - 1` layers exist to check.
fn theta_search<T: Fn(usize) -> f64, W: Fn(usize) -> f64>(
    theta: f64,
    klpt: usize,
    nuvz: usize,
    nlck: usize,
    th: T,
    w: W,
) -> Option<f64> {
    const EPS: f64 = 1.0e-5;
    let bracket = |k: usize| -> Option<f64> {
        let thdn = th(k);
        let thup = th(k + 1);
        if (thdn >= theta && thup <= theta) || (thdn <= theta && thup >= theta) {
            let mut dt1 = (theta - thdn).abs();
            let mut dt2 = (theta - thup).abs();
            let mut dt = dt1 + dt2;
            if dt < EPS {
                // "Avoid division by zero error" (G.W., 10.4.1996)
                dt1 = 0.5;
                dt2 = 0.5;
                dt = 1.0;
            }
            Some((w(k) * dt2 + w(k + 1) * dt1) / dt)
        } else {
            None
        }
    };
    let mut kup = klpt as i64 - 1;
    let mut kdn = klpt as i64;
    let mut kch = 0usize;
    loop {
        // label 40: upward branch
        kup += 1;
        if kch >= nlck {
            return None;
        }
        if kup < nuvz as i64 {
            kch += 1;
            if let Some(v) = bracket(kup as usize) {
                return Some(v);
            }
        }
        // label 41: downward branch
        kdn -= 1;
        if kdn < 1 {
            continue;
        }
        kch += 1;
        if let Some(v) = bracket(kdn as usize) {
            return Some(v);
        }
    }
}

/// `calcpv.f90` / `calcpv_nests.f90`: Ertel potential vorticity on the model
/// levels, PVU,
///
/// ```text
///   PV = -g dtheta/dp (f + (dv/dx / cos(phi) - du/dy + u tan(phi)) / R) * 1e6
/// ```
///
/// with the horizontal derivatives taken **along the potential-temperature
/// surface**: in each neighbouring column the wind is interpolated to the
/// level where `theta` equals the current point's (a spiral search of at most
/// `nuvz/3` layers, see `theta_search`). Where no such level is found the
/// current point's own wind is used and the gap shrinks by one grid length;
/// where both sides fail the derivative falls back to the same-level
/// difference.
///
/// # Upstream behaviour reproduced
/// - `theta = T (100000/p)^kappa` with `kappa = 0.286`; `dtheta/dp` is
///   centred, one-sided at the bottom and top level.
/// - Coriolis `f = 0.00014585 sin(phi)` (upstream's literal), `pi` is
///   `par_mod`'s `3.14159265`, `R = 6.371e6 m`, `g = 9.81` (literal).
/// - A bracket whose two levels and `theta` all agree within `1e-5` K takes
///   the plain average of the two winds ("avoid division by zero").
/// - `xglobal`: the west neighbour of column 0 is column `nx-2` (column
///   `nx-1` duplicates column 0), the east neighbour of `nx-1` is column 1.
/// - `sglobal` / `nglobal`: the pole rows are skipped, the next row in
///   uses a one-sided difference, and the pole row is then filled with the
///   mean of that next row (summed in `ix` order, divided by `nx`).
///
/// # Errors
/// [`MetFieldsError::DegenerateStencil`] where upstream would read an
/// unassigned stencil value; [`MetFieldsError::ShapeMismatch`].
#[allow(clippy::too_many_arguments)]
pub fn calcpv(
    geom: &GridGeometry,
    domain: GlobalDomain,
    akz: &[f64],
    bkz: &[f64],
    ps: &Field2,
    tth: &Field3,
    uuh: &Field3,
    vvh: &Field3,
) -> Result<Field3, MetFieldsError> {
    let nuvz = akz.len();
    let (nx, ny) = (geom.nx, geom.ny);
    if bkz.len() != nuvz
        || nuvz < 2
        || !shape_ok2(ps, geom)
        || !shape_ok3(tth, geom, nuvz)
        || !shape_ok3(uuh, geom, nuvz)
        || !shape_ok3(vvh, geom, nuvz)
    {
        return Err(MetFieldsError::ShapeMismatch);
    }
    let (nxi, nyi) = (nx as i64, ny as i64);
    let (nxmin1, nymin1) = (nxi - 1, nyi - 1);
    let nlck = nuvz / 3;

    // ppml, ppmk (1-based levels)
    let mut ppml = Field3::filled(nx, ny, nuvz, 0.0);
    let mut ppmk = Field3::filled(nx, ny, nuvz, 0.0);
    for kl in 0..nuvz {
        for jy in 0..ny {
            for ix in 0..nx {
                let p = akz[kl] + bkz[kl] * ps.at(ix, jy);
                ppml.set(ix, jy, kl, p);
                ppmk.set(ix, jy, kl, (100_000.0 / p).powf(KAPPA));
            }
        }
    }
    let theta_at = |ix: usize, jy: usize, k: usize| tth.at(ix, jy, k - 1) * ppmk.at(ix, jy, k - 1);

    let mut pvh = Field3::filled(nx, ny, nuvz, 0.0);
    for jy in 0..nyi {
        if domain.sglobal && jy == 0 {
            continue;
        }
        if domain.nglobal && jy == nymin1 {
            continue;
        }
        let ju = jy as usize;
        let phi = (geom.ylat0 + jy as f64 * geom.dy) * PI / 180.0;
        let f = 0.00014585 * phi.sin();
        let tanphi = phi.tan();
        let cosphi = phi.cos();
        // virtual jy+1 and jy-1 at the domain edge
        let mut jyvp = jy + 1;
        let mut jyvm = jy - 1;
        if jy == 0 {
            jyvm = 0;
        }
        if jy == nymin1 {
            jyvp = nymin1;
        }
        let mut jumpy = 2;
        if jy == 0 || jy == nymin1 {
            jumpy = 1;
        }
        if domain.sglobal && jy == 1 {
            jyvm = 1;
            jumpy = 1;
        }
        if domain.nglobal && jy == nyi - 2 {
            jyvp = nyi - 2;
            jumpy = 1;
        }
        let ystencil: Vec<i64> = (jyvm..=jyvp).step_by(jumpy as usize).collect();
        if ystencil.len() != 2 {
            return Err(MetFieldsError::DegenerateStencil);
        }

        for ix in 0..nxi {
            let iu = ix as usize;
            let mut ixvp = ix + 1;
            let mut ixvm = ix - 1;
            let mut jumpx = 2;
            let (ivrp, ivrm);
            if domain.xglobal {
                let mut p = ixvp;
                let mut m = ixvm;
                if ixvm < 0 {
                    m = ixvm + nxmin1;
                }
                if ixvp >= nxi {
                    p = ixvp - nxi + 1;
                }
                ivrp = p;
                ivrm = m;
            } else {
                if ix == 0 {
                    ixvm = 0;
                }
                if ix == nxmin1 {
                    ixvp = nxmin1;
                }
                ivrp = ixvp;
                ivrm = ixvm;
                if ix == 0 || ix == nxmin1 {
                    jumpx = 1;
                }
            }
            let xstencil: Vec<i64> = (ixvm..=ixvp).step_by(jumpx as usize).collect();
            if xstencil.len() != 2 {
                return Err(MetFieldsError::DegenerateStencil);
            }

            for kl in 1..=nuvz {
                let theta = theta_at(iu, ju, kl);
                let klvrp = if kl + 1 > nuvz { nuvz } else { kl + 1 };
                let klvrm = if kl < 2 { 1 } else { kl - 1 };
                let thetap = theta_at(iu, ju, klvrp);
                let thetam = theta_at(iu, ju, klvrm);
                let dthetadp =
                    (thetap - thetam) / (ppml.at(iu, ju, klvrp - 1) - ppml.at(iu, ju, klvrm - 1));

                // a) x direction
                let mut jux: i64 = jumpx;
                let mut vx = [0.0_f64; 2];
                for (ii, &i) in xstencil.iter().enumerate() {
                    let mut ivr = i;
                    if domain.xglobal {
                        if i < 0 {
                            ivr += nxmin1;
                        }
                        if i >= nxi {
                            ivr = ivr - nxi + 1;
                        }
                    }
                    let c = ivr as usize;
                    match theta_search(
                        theta,
                        kl,
                        nuvz,
                        nlck,
                        |k| theta_at(c, ju, k),
                        |k| vvh.at(c, ju, k - 1),
                    ) {
                        Some(v) => vx[ii] = v,
                        None => {
                            vx[ii] = vvh.at(iu, ju, kl - 1);
                            jux -= 1;
                        }
                    }
                }
                let dvdx = if jux > 0 {
                    (vx[1] - vx[0]) / jux as f64 / (geom.dx * PI / 180.0)
                } else {
                    let d = vvh.at(ivrp as usize, ju, kl - 1) - vvh.at(ivrm as usize, ju, kl - 1);
                    d / jumpx as f64 / (geom.dx * PI / 180.0)
                };

                // b) y direction
                let mut juy: i64 = jumpy;
                let mut uy = [0.0_f64; 2];
                for (jj, &j) in ystencil.iter().enumerate() {
                    let c = j as usize;
                    match theta_search(
                        theta,
                        kl,
                        nuvz,
                        nlck,
                        |k| theta_at(iu, c, k),
                        |k| uuh.at(iu, c, k - 1),
                    ) {
                        Some(v) => uy[jj] = v,
                        None => {
                            uy[jj] = uuh.at(iu, ju, kl - 1);
                            juy -= 1;
                        }
                    }
                }
                let dudy = if juy > 0 {
                    (uy[1] - uy[0]) / juy as f64 / (geom.dy * PI / 180.0)
                } else {
                    let d = uuh.at(iu, jyvp as usize, kl - 1) - uuh.at(iu, jyvm as usize, kl - 1);
                    d / jumpy as f64 / (geom.dy * PI / 180.0)
                };

                let pv = dthetadp
                    * (f + (dvdx / cosphi - dudy + uuh.at(iu, ju, kl - 1) * tanphi) / R_EARTH)
                    * (-1.0e6)
                    * 9.81;
                pvh.set(iu, ju, kl - 1, pv);
            }
        }
    }

    // Poles: mean PV of the adjacent latitude ring.
    let ring_mean = |pvh: &Field3, row: usize, k: usize| {
        let mut s = 0.0;
        for ix in 0..nx {
            s += pvh.at(ix, row, k);
        }
        s / nx as f64
    };
    if domain.sglobal {
        for k in 0..nuvz {
            let avg = ring_mean(&pvh, 1, k);
            for ix in 0..nx {
                pvh.set(ix, 0, k, avg);
            }
        }
    }
    if domain.nglobal {
        for k in 0..nuvz {
            let avg = ring_mean(&pvh, ny - 2, k);
            for ix in 0..nx {
                pvh.set(ix, ny - 1, k, avg);
            }
        }
    }
    Ok(pvh)
}
