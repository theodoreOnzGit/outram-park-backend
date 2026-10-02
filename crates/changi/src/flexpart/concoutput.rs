// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/concoutput.f90 (subroutine concoutput),
//                    src/concoutput_nest.f90 (subroutine concoutput_nest),
//                    src/concoutput_surf.f90 (subroutine concoutput_surf),
//                    src/mean_mod.f90 (mean_dp, mean_mixed_dsd)
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Conversion of the accumulated particle mass on the output grids into
//! FLEXPART's gridded output: concentrations (or, backward, residence
//! times), mixing ratios, wet and dry deposition, the uncertainty statistics
//! over the `nclassunc` particle classes and the **sparse packing** in which
//! FLEXPART writes them (`concoutput.f90`, `concoutput_nest.f90`,
//! `concoutput_surf.f90`).
//!
//! The numerics are ported; the file I/O is not. [`concoutput`] returns, in
//! upstream's record order, every record the routine writes (see
//! [`ConcOutput`]); a caller wanting FLEXPART's binary files writes them
//! from that.
//!
//! # What upstream computes
//!
//! * **Air density at each output cell** (`densityoutgrid`), from the
//!   coarse met grid: the met column nearest the output cell centre
//!   (`nint` of the cell's position in met grid units, clamped to the grid),
//!   interpolated linearly in height to the middle of the output layer,
//!   `halfheight = (outheight(kz) + outheight(kz-1))/2`. The same for the
//!   dry-air density, and `factor_drygrid = densityoutgrid/densitydrygrid`.
//! * **The unit factor** `factor3d = 1e12/volume/outnum` forward (kg → ng
//!   per m³, averaged over the `outnum` samples), `|loutaver|/outnum`
//!   backward (residence time, s).
//! * **The uncertainty statistics.** For every cell, the `nclassunc`
//!   per-class values go through `mean_mod`'s one-pass mean and sample
//!   standard deviation; the mean times `nclassunc` is the cell's total, the
//!   standard deviation times `sqrt(nclassunc)` its uncertainty. The three
//!   numbers `concoutput` returns are `sum(sigma)/sum(total)` over all
//!   species, release slots, age classes and cells.
//! * **The sparse packing.** A field is written as four records: the number
//!   of runs, the flat index of the first cell of every run of consecutive
//!   non-zero (`> tiny(0.0)`) cells, the number of values, and the values,
//!   whose **sign alternates from run to run** (`+` for the first run) — the
//!   run boundaries are recoverable from the sign changes. Runs continue
//!   across row and level boundaries: the scan is one linear sweep, `ix`
//!   fastest. Deposition fields are `1e12*value/area`, concentrations
//!   `value*factor3d/tot_mu` (or the particle count), mixing ratios
//!   `1e12*value/volume/outnum*28.97/weightmolar/density`; `factor_drygrid`
//!   is packed with "`!= 1`" in place of "non-zero".
//!
//! # The three routines ([`Routine`])
//!
//! | | `concoutput` | `concoutput_nest` | `concoutput_surf` |
//! |---|---|---|---|
//! | grids | `gridunc`, `area`, `volume` | `griduncn`, `arean`, `volumen` | as `concoutput` |
//! | density time slot | `memind(2)` | `memind(2)` | **literally 2** |
//! | levels written | all | all | **`kz = 1` only** (means and totals still over all levels) |
//! | uncertainty totals | yes | no | yes |
//! | receptor records | yes | no | yes |
//!
//! # Upstream quirks reproduced (and documented, not fixed)
//!
//! * **Level search.** The density level is found by
//!   `height(kzz-1) < halfheight < height(kzz)`, both strict. If no level
//!   satisfies it — `halfheight` above the top level, below `height(1)`, or
//!   **exactly equal to a model level** — the loop runs out and `kzz` is
//!   clamped to `nz`, so the density is *extrapolated* from the top two
//!   levels. The code-to-code sweep includes an output layer whose
//!   half-height is exactly `height(2) = 50 m`: upstream (and the port)
//!   then return `(rho(6)*(-950) + rho(5)*2450)/1500` instead of `rho(2)`.
//! * **`concoutput_surf` reads time slot 2, not `memind(2)`**, for every
//!   density (`rho(...,2)`; the other two use `mind = memind(2)`, with the
//!   comment "added to ensure identical results between 2&3-fields
//!   versions"). After the fields swap, the surface output uses the other
//!   time's density.
//! * **`mean_mod`'s `eps = 1e-30` is absolute.** The sample variance is set
//!   to 0 when `sum(x^2) - sum(x)^2/n < 1e-30`. Grid values are masses (kg)
//!   divided by the density; for masses below about `1e-15` the uncertainty
//!   is therefore always reported as exactly 0, whatever the spread. The
//!   sweep's case 3 (masses ~1e-17 kg, classes spread by a factor 50) shows
//!   it: every `gridsigma` is 0.
//! * **The uncertainty totals mix species, release points and age classes**,
//!   and `gridtotal`/`gridsigmatotal`/`gridtotalunc` are declared `real(sp)`
//!   — single precision even in a double-precision build (reproduced: the
//!   port accumulates them in `f32`). Backward runs and runs without
//!   deposition return 0.
//! * **The concentration index carries `kz` 1-based**: the first level's
//!   cells are numbered from `numxgrid*numygrid`, not 0.
//! * **`concoutput_nest` writes no receptor records** (it computes the
//!   receptor densities and discards them) and **zeroes `creceptor`** as
//!   well as `griduncn`; `concoutput` zeroes `creceptor` and `gridunc`. The
//!   port leaves resetting the accumulators to the caller.
//! * **Receptor concentrations are written whenever `numreceptor > 0`**,
//!   also for `iout = 2`, when `openreceptors.f90` has not opened the unit
//!   (gfortran then writes `fort.91`). Not reproduced (file I/O); the record
//!   is returned.
//! * **Mixing ratios ignore `ldirect`**: in a backward run with `iout = 2/3`
//!   they are still `1e12*grid/volume/outnum*...` — a residence time
//!   divided by a volume and an air density. Reproduced.
//! * **`wetgrid`/`drygrid` are only recomputed for forward runs with the
//!   switch on**; otherwise their (stale) contents are never written, since
//!   the sparse loops test the same switches. Reproduced by returning
//!   `None`.
//! * **Upstream does not compile in double precision as shipped.** With
//!   `-fdefault-real-8` and `dep_prec = sp`, `call mean(auxgrid, grid(...),
//!   gridsigma(...), nclassunc)` has a `real(4)` sample and `real(8)`
//!   results, and `mean_mod` has no matching specific (gfortran: "There is no
//!   specific subroutine for the generic 'mean'", `concoutput.f90:322`,
//!   `concoutput_nest.f90:272`, `concoutput_surf.f90:294`). The real(8)
//!   reference therefore sets `dep_prec = dp`, the alternative `par_mod.f90`
//!   documents.
//!
//! # Precision model
//!
//! The port reproduces upstream compiled with `-fdefault-real-8` and
//! `dep_prec = dp`: default `real` and `real(dep_prec)` are `f64`; the
//! quantities upstream declares **`real(sp)` explicitly** stay `f32` in
//! every build and are rounded here as there — `wetgrid`, `drygrid` (whose
//! mean comes from `mean_mixed_dsd`, `xm = real(xl,sp)/real(n,sp)`), and
//! `gridtotal`, `gridsigmatotal`, `gridtotalunc`. Against the shipped
//! single-precision build the residual is upstream's `real(4)` arithmetic.
//!
//! # Units and indices
//!
//! Grid inputs are what `conccalc`/`wetdepokernel`/`drydepokernel`
//! accumulate (kg, or kg/(kg m⁻³)). Areas m², volumes m³, heights m,
//! densities kg/m³, `outnum` dimensionless, `loutaver` s, molar weights
//! g/mol. Concentration output ng/m³ (forward) or s (backward), deposition
//! ng/m², mixing ratio pptv. Output and met cell indices are 0-based as
//! upstream's `0:numxgrid-1`; levels, species, release slots, classes and age
//! classes are 0-based here (1-based upstream) — except the packed flat
//! index, which is upstream's number.

use crate::flexpart::concentration::{ConcentrationGrid, DepositionGrid};
use crate::flexpart::plume_trajectory::mean;

/// `smallnum = tiny(0.0)` in the double-precision build.
const SMALLNUM: f64 = f64::MIN_POSITIVE;
/// `weightair = 28.97`, g/mol.
const WEIGHTAIR: f64 = 28.97;

/// Which upstream routine to reproduce. See the module table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routine {
    /// `concoutput.f90`: mother output grid, all levels.
    Concoutput,
    /// `concoutput_nest.f90`: nested output grid, no totals, no receptors.
    Nest,
    /// `concoutput_surf.f90`: mother grid, first level written only,
    /// density from time slot 2.
    Surface,
}

/// The meteorological fields `concoutput` reads: the coarse grid's density
/// and dry-air density columns (`rho`, `rho_dry`) for every time slot in
/// memory, and the level heights.
#[derive(Debug, Clone, PartialEq)]
pub struct DensityFields {
    /// `nxmin1` (the met grid has `nxmin1 + 1` columns).
    pub nxmin1: usize,
    /// `nymin1`.
    pub nymin1: usize,
    /// Model levels `nz`.
    pub nz: usize,
    /// Met grid origin, degrees (`xlon0`).
    pub xlon0: f64,
    /// Met grid origin, degrees (`ylat0`).
    pub ylat0: f64,
    /// Met grid spacing, degrees (`dx`).
    pub dx: f64,
    /// Met grid spacing, degrees (`dy`).
    pub dy: f64,
    /// Level heights, m (`height(1:nz)`).
    pub height: Vec<f64>,
    /// Air density, kg/m³: `ix` fastest, then `jy`, level, time slot.
    pub rho: Vec<f64>,
    /// Dry-air density, kg/m³, laid out as [`DensityFields::rho`].
    pub rho_dry: Vec<f64>,
    /// Time slots held (`numwfmem`).
    pub nslots: usize,
}

impl DensityFields {
    fn index(&self, ix: usize, jy: usize, k: usize, slot: usize) -> usize {
        let nx = self.nxmin1 + 1;
        let ny = self.nymin1 + 1;
        ix + nx * (jy + ny * (k + self.nz * slot))
    }

    fn expected_len(&self) -> usize {
        (self.nxmin1 + 1) * (self.nymin1 + 1) * self.nz * self.nslots
    }
}

/// One output grid (`numxgrid`, `outlon0`, `dxout`, `area`, `volume`, or
/// the nest's `numxgridn`, `outlon0n`, ..., `arean`, `volumen`).
#[derive(Debug, Clone, PartialEq)]
pub struct OutputGrid {
    /// Columns.
    pub numxgrid: usize,
    /// Rows.
    pub numygrid: usize,
    /// Lower-left cell longitude, degrees.
    pub outlon0: f64,
    /// Lower-left cell latitude, degrees.
    pub outlat0: f64,
    /// Cell width, degrees.
    pub dxout: f64,
    /// Cell height, degrees.
    pub dyout: f64,
    /// Cell areas, m², `ix` fastest.
    pub area: Vec<f64>,
    /// Cell volumes, m³, `ix` fastest, then `jy`, level.
    pub volume: Vec<f64>,
}

/// Run settings `concoutput` reads from `com_mod` / `par_mod`.
#[derive(Debug, Clone, PartialEq)]
pub struct ConcOutputSettings {
    /// Which routine.
    pub routine: Routine,
    /// `+1` forward, `-1` backward (`ldirect`).
    pub ldirect: i64,
    /// Output selector `iout`: concentrations for 1, 3, 5; mixing ratios
    /// for 2, 3.
    pub iout: i32,
    /// Wet deposition switched on (`WETDEP`).
    pub wetdep: bool,
    /// Dry deposition switched on (`DRYDEP`).
    pub drydep: bool,
    /// Number of samples in the averaging interval (`outnum`).
    pub outnum: f64,
    /// Averaging time, s (`loutaver`, negative in backward runs).
    pub loutaver: i64,
    /// `memind(2)`, 1-based time slot (ignored by [`Routine::Surface`]).
    pub memind2: usize,
    /// `lparticlecountoutput` (a `par_mod` parameter): write grid values
    /// unconverted.
    pub particle_count_output: bool,
    /// Output level tops, m (`outheight(1:numzgrid)`).
    pub outheight: Vec<f64>,
    /// Molar weight per species, g/mol (`weightmolar`).
    pub weightmolar: Vec<f64>,
    /// Released mass per release slot and species, kg, `xmass(kp, ks)` laid
    /// out `kp` fastest (backward runs divide by it).
    pub xmass: Vec<f64>,
}

/// Receptor points (`xreceptor`, `yreceptor` in met grid units) and their
/// accumulated concentrations `creceptor(i, ks)`, `i` fastest.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReceptorInput {
    /// x, met grid units.
    pub x: Vec<f64>,
    /// y, met grid units.
    pub y: Vec<f64>,
    /// `creceptor(i, ks)`, receptor fastest.
    pub creceptor: Vec<f64>,
}

/// One sparse-packed field: the four records upstream writes
/// (`sp_count_i`, `sparse_dump_i(1:sp_count_i)`, `sp_count_r`,
/// `sparse_dump_r(1:sp_count_r)`); the counts are the lengths.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SparseField {
    /// Flat index of the first cell of every run.
    pub indices: Vec<i64>,
    /// Values, the sign alternating from run to run.
    pub values: Vec<f64>,
}

impl SparseField {
    /// Upstream's run-length packing: scan `(flat index, value)` in order,
    /// open a run at each cell passing `keep` after one that did not, flip
    /// the sign for each new run, and store `sign * convert(cell)`.
    fn pack<I, K, C>(cells: I, keep: K, convert: C) -> Self
    where
        I: Iterator<Item = (i64, usize)>,
        K: Fn(usize) -> bool,
        C: Fn(f64, usize) -> f64,
    {
        let mut out = SparseField::default();
        let mut sp_fact = -1.0_f64;
        let mut sp_zer = true;
        for (flat, cell) in cells {
            if keep(cell) {
                if sp_zer {
                    out.indices.push(flat);
                    sp_zer = false;
                    sp_fact *= -1.0;
                }
                out.values.push(convert(sp_fact, cell));
            } else {
                sp_zer = true;
            }
        }
        out
    }
}

/// The three packed fields of one file for one (species, release slot, age
/// class): wet deposition, dry deposition, then the 3-D field. Wet and dry
/// are empty unless the run is forward with the switch on.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SliceRecords {
    /// Wet deposition, ng/m².
    pub wet: SparseField,
    /// Dry deposition, ng/m².
    pub dry: SparseField,
    /// Concentration (ng/m³; backward: s; or particle counts), or mixing
    /// ratio (pptv).
    pub field: SparseField,
}

/// The `nclassunc` statistics of one slice (upstream's `grid`, `gridsigma`,
/// `wetgrid`, ... work arrays after the slice).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SliceStats {
    /// Total over classes, `ix` fastest, then `jy`, level.
    pub grid: Vec<f64>,
    /// Its uncertainty (`sigma * sqrt(nclassunc)`).
    pub gridsigma: Vec<f64>,
    /// Wet deposition total, `real(sp)` upstream; `None` unless forward
    /// with `WETDEP`.
    pub wetgrid: Option<Vec<f32>>,
    /// Its uncertainty.
    pub wetgridsigma: Option<Vec<f64>>,
    /// Dry deposition total, `real(sp)` upstream.
    pub drygrid: Option<Vec<f32>>,
    /// Its uncertainty.
    pub drygridsigma: Option<Vec<f64>>,
}

/// Everything concerning one (species, release slot, age class).
#[derive(Debug, Clone, PartialEq)]
pub struct SliceOutput {
    /// Species, 0-based.
    pub ks: usize,
    /// Release slot, 0-based.
    pub kp: usize,
    /// Age class, 0-based.
    pub nage: usize,
    /// Records of the concentration file (`grid_conc_*` / `grid_time_*`),
    /// present for `iout` 1, 3, 5.
    pub conc: Option<SliceRecords>,
    /// Records of the mixing-ratio file (`grid_pptv_*`), present for `iout`
    /// 2, 3.
    pub pptv: Option<SliceRecords>,
    /// The class statistics.
    pub stats: SliceStats,
}

/// `gridtotalunc`, `wetgridtotalunc`, `drygridtotalunc`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Uncertainty {
    /// `sum(gridsigma)/sum(grid)`, `real(sp)` upstream.
    pub gridtotalunc: f32,
    /// The same for wet deposition.
    pub wetgridtotalunc: f64,
    /// The same for dry deposition.
    pub drygridtotalunc: f64,
}

/// Receptor records (`concoutput`, `concoutput_surf` only).
#[derive(Debug, Clone, PartialEq)]
pub struct ReceptorOutput {
    /// `receptor_conc`: one record per species, `1e12*creceptor/outnum`.
    pub conc: Vec<Vec<f64>>,
    /// `receptor_pptv` (`iout` 2, 3): per species, mixing ratio.
    pub pptv: Option<Vec<Vec<f64>>>,
    /// `factor_dryreceptor`: `rho/rho_dry` at each receptor.
    pub factor_dry: Vec<f64>,
    /// The receptor air density used (`densityoutrecept`).
    pub density: Vec<f64>,
}

/// What one call of the routine produces.
#[derive(Debug, Clone, PartialEq)]
pub struct ConcOutput {
    /// `densityoutgrid`, kg/m³, `ix` fastest, then `jy`, level.
    pub density: Vec<f64>,
    /// `densitydrygrid`.
    pub density_dry: Vec<f64>,
    /// `factor_drygrid = density/density_dry`.
    pub factor_drygrid: Vec<f64>,
    /// `factor3d`.
    pub factor3d: Vec<f64>,
    /// Slices in upstream's loop order: species, then release slot, then
    /// age class (age fastest).
    pub slices: Vec<SliceOutput>,
    /// The `factor_drygrid` file's packed field (no time record).
    pub factor_drygrid_sparse: SparseField,
    /// `None` for [`Routine::Nest`].
    pub uncertainty: Option<Uncertainty>,
    /// `None` for [`Routine::Nest`] or without receptors.
    pub receptors: Option<ReceptorOutput>,
}

/// Why [`concoutput`] refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcOutputError {
    /// An input array does not match the stated dimensions.
    DimensionMismatch,
    /// `memind2` (or slot 2 for the surface routine) is not a held slot.
    SlotOutOfRange,
    /// `nz < 2`: the density interpolation needs two levels.
    TooFewLevels,
}

/// `mean_dp`'s one-pass mean and standard deviation, but returning the sum
/// `xl` too (the mixed-precision variant needs it unrounded).
fn class_sum(x: &[f64]) -> f64 {
    let mut xl = 0.0;
    for &v in x {
        xl += v;
    }
    xl
}

/// Run `concoutput` / `concoutput_nest` / `concoutput_surf`.
///
/// `gridunc` (or `griduncn`) and, for forward runs with deposition, the
/// deposition grids are the accumulated inputs; their `nx`/`ny` must match
/// `out`, and their level count `set.outheight`. `wetgridunc`/`drygridunc`
/// may be `None` when the switch is off or the run is backward (upstream
/// does not allocate them then). `receptors` is ignored by
/// [`Routine::Nest`].
///
/// # Errors
/// [`ConcOutputError`] for inconsistent dimensions or a time slot out of
/// range; upstream would read out of bounds.
#[allow(clippy::too_many_lines)]
pub fn concoutput(
    set: &ConcOutputSettings,
    met: &DensityFields,
    out: &OutputGrid,
    gridunc: &ConcentrationGrid,
    wetgridunc: Option<&DepositionGrid>,
    drygridunc: Option<&DepositionGrid>,
    receptors: &ReceptorInput,
) -> Result<ConcOutput, ConcOutputError> {
    let nx = out.numxgrid;
    let ny = out.numygrid;
    let nzg = set.outheight.len();
    let nspec = gridunc.nspec;
    let npt = gridunc.npointspec;
    let nclassunc = gridunc.nclassunc;
    let nageclass = gridunc.nageclass;
    let forward = set.ldirect > 0;
    let do_wet = set.wetdep && forward;
    let do_dry = set.drydep && forward;

    if met.height.len() != met.nz
        || met.rho.len() != met.expected_len()
        || met.rho_dry.len() != met.expected_len()
        || out.area.len() != nx * ny
        || out.volume.len() != nx * ny * nzg
        || gridunc.nx != nx
        || gridunc.ny != ny
        || gridunc.nz != nzg
        || set.weightmolar.len() < nspec
        || (!forward && set.xmass.len() < npt * nspec)
        || receptors.x.len() != receptors.y.len()
    {
        return Err(ConcOutputError::DimensionMismatch);
    }
    for g in [wetgridunc, drygridunc].into_iter().flatten() {
        if g.nx != nx
            || g.ny != ny
            || g.nspec != nspec
            || g.npointspec != npt
            || g.nclassunc != nclassunc
            || g.nageclass != nageclass
        {
            return Err(ConcOutputError::DimensionMismatch);
        }
    }
    if (do_wet && wetgridunc.is_none()) || (do_dry && drygridunc.is_none()) {
        return Err(ConcOutputError::DimensionMismatch);
    }
    if met.nz < 2 {
        return Err(ConcOutputError::TooFewLevels);
    }
    // `mind = memind(2)`, or literally 2 in concoutput_surf.f90.
    let slot1 = match set.routine {
        Routine::Surface => 2,
        _ => set.memind2,
    };
    if slot1 == 0 || slot1 > met.nslots {
        return Err(ConcOutputError::SlotOutOfRange);
    }
    let mind = slot1 - 1;
    let nreceptor = receptors.x.len();
    if receptors.creceptor.len() < nreceptor * nspec {
        return Err(ConcOutputError::DimensionMismatch);
    }

    // tot_mu: 1 forward, xmass(kp,ks) backward.
    let tot_mu = |ks: usize, kp: usize| -> f64 {
        if set.ldirect == 1 {
            1.0
        } else {
            set.xmass[kp + npt * ks]
        }
    };

    // Air density at the output cells.
    let ncell2 = nx * ny;
    let mut density = vec![0.0; ncell2 * nzg];
    let mut density_dry = vec![0.0; ncell2 * nzg];
    let nint_clamp = |v: f64, max: usize| -> usize {
        // max(min(nint(v), max), 0); nint rounds half away from zero.
        let n = v.round() as i64;
        n.min(max as i64).max(0) as usize
    };
    for kz in 0..nzg {
        let halfheight = if kz == 0 {
            set.outheight[0] / 2.0
        } else {
            (set.outheight[kz] + set.outheight[kz - 1]) / 2.0
        };
        // do kzz=2,nz ... goto 46; on exhaustion kzz = nz+1. 1-based here.
        let mut kzz = met.nz + 1;
        for k in 2..=met.nz {
            if met.height[k - 2] < halfheight && met.height[k - 1] > halfheight {
                kzz = k;
                break;
            }
        }
        let kzz = kzz.min(met.nz).max(2);
        let dz1 = halfheight - met.height[kzz - 2];
        let dz2 = met.height[kzz - 1] - halfheight;
        let dz = dz1 + dz2;
        for jy in 0..ny {
            for ix in 0..nx {
                let xl = out.outlon0 + ix as f64 * out.dxout;
                let yl = out.outlat0 + jy as f64 * out.dyout;
                let xl = (xl - met.xlon0) / met.dx;
                let yl = (yl - met.ylat0) / met.dy;
                let iix = nint_clamp(xl, met.nxmin1);
                let jjy = nint_clamp(yl, met.nymin1);
                let up = met.index(iix, jjy, kzz - 1, mind);
                let lo = met.index(iix, jjy, kzz - 2, mind);
                let c = ix + nx * (jy + ny * kz);
                density[c] = (met.rho[up] * dz1 + met.rho[lo] * dz2) / dz;
                density_dry[c] = (met.rho_dry[up] * dz1 + met.rho_dry[lo] * dz2) / dz;
            }
        }
    }
    let mut rec_density = Vec::with_capacity(nreceptor);
    let mut rec_density_dry = Vec::with_capacity(nreceptor);
    for i in 0..nreceptor {
        let iix = nint_clamp(receptors.x[i], met.nxmin1);
        let jjy = nint_clamp(receptors.y[i], met.nymin1);
        let k = met.index(iix, jjy, 0, mind);
        rec_density.push(met.rho[k]);
        rec_density_dry.push(met.rho_dry[k]);
    }
    let factor_drygrid: Vec<f64> = density
        .iter()
        .zip(&density_dry)
        .map(|(a, b)| a / b)
        .collect();
    let factor_dryrecept: Vec<f64> = rec_density
        .iter()
        .zip(&rec_density_dry)
        .map(|(a, b)| a / b)
        .collect();

    let mut factor3d = vec![0.0; ncell2 * nzg];
    for (c, f) in factor3d.iter_mut().enumerate() {
        *f = if set.ldirect == 1 {
            1.0e12 / out.volume[c] / set.outnum
        } else {
            set.loutaver.abs() as f64 / set.outnum
        };
    }

    // Uncertainty accumulators: gridtotal etc. are real(sp).
    let mut gridtotal = 0.0_f32;
    let mut gridsigmatotal = 0.0_f32;
    let mut wetgridtotal = 0.0_f64;
    let mut wetgridsigmatotal = 0.0_f64;
    let mut drygridtotal = 0.0_f64;
    let mut drygridsigmatotal = 0.0_f64;
    let sqrt_n = (nclassunc as f64).sqrt();
    let n_f32 = nclassunc as f32;

    let write_conc = matches!(set.iout, 1 | 3 | 5);
    let write_ppt = matches!(set.iout, 2 | 3);
    // concoutput_surf writes only the first level.
    let nz_written = match set.routine {
        Routine::Surface => 1.min(nzg),
        _ => nzg,
    };

    let mut slices = Vec::with_capacity(nspec * npt * nageclass);
    let mut aux = vec![0.0; nclassunc];
    for ks in 0..nspec {
        for kp in 0..npt {
            for nage in 0..nageclass {
                let mut st = SliceStats {
                    grid: vec![0.0; ncell2 * nzg],
                    gridsigma: vec![0.0; ncell2 * nzg],
                    wetgrid: do_wet.then(|| vec![0.0; ncell2]),
                    wetgridsigma: do_wet.then(|| vec![0.0; ncell2]),
                    drygrid: do_dry.then(|| vec![0.0; ncell2]),
                    drygridsigma: do_dry.then(|| vec![0.0; ncell2]),
                };
                for jy in 0..ny {
                    for ix in 0..nx {
                        let c2 = ix + nx * jy;
                        // Deposition: mean_mixed_dsd (real(dep_prec) sample,
                        // real(sp) mean, real(dep_prec) sigma).
                        for (on, g, tot, sigtot, isw) in [
                            (do_wet, wetgridunc, &mut wetgridtotal, &mut wetgridsigmatotal, true),
                            (do_dry, drygridunc, &mut drygridtotal, &mut drygridsigmatotal, false),
                        ] {
                            if !on {
                                continue;
                            }
                            let g = g.expect("checked above");
                            for (l, a) in aux.iter_mut().enumerate() {
                                *a = g.values[g.index(ix, jy, ks, kp, l, nage)];
                            }
                            let xl = class_sum(&aux);
                            let (_, xs) = mean(&aux);
                            let xm = (xl as f32) / n_f32;
                            let total = xm * n_f32;
                            *tot += f64::from(total);
                            let sigma = xs * sqrt_n;
                            *sigtot += sigma;
                            if isw {
                                st.wetgrid.as_mut().expect("wet")[c2] = total;
                                st.wetgridsigma.as_mut().expect("wet")[c2] = sigma;
                            } else {
                                st.drygrid.as_mut().expect("dry")[c2] = total;
                                st.drygridsigma.as_mut().expect("dry")[c2] = sigma;
                            }
                        }
                        // Concentration: mean_dp.
                        for kz in 0..nzg {
                            for (l, a) in aux.iter_mut().enumerate() {
                                *a = gridunc.values[gridunc.index(ix, jy, kz, ks, kp, l, nage)];
                            }
                            let (xm, xs) = mean(&aux);
                            let total = xm * nclassunc as f64;
                            gridtotal = (f64::from(gridtotal) + total) as f32;
                            let sigma = xs * sqrt_n;
                            gridsigmatotal = (f64::from(gridsigmatotal) + sigma) as f32;
                            let c = c2 + ncell2 * kz;
                            st.grid[c] = total;
                            st.gridsigma[c] = sigma;
                        }
                    }
                }

                let cells2 = || (0..ncell2).map(|c| (c as i64, c));
                let cells3 = || {
                    (0..nz_written).flat_map(move |kz| {
                        (0..ncell2).map(move |c2| {
                            // ix + jy*numx + kz*numx*numy with kz 1-based.
                            (c2 as i64 + ((kz + 1) * ncell2) as i64, c2 + ncell2 * kz)
                        })
                    })
                };
                let dep = |grid: &Option<Vec<f32>>| -> SparseField {
                    match grid {
                        Some(gv) if set.ldirect == 1 => SparseField::pack(
                            cells2(),
                            |c| f64::from(gv[c]) > SMALLNUM,
                            |f, c| f * 1.0e12 * f64::from(gv[c]) / out.area[c],
                        ),
                        _ => SparseField::default(),
                    }
                };
                let conc = write_conc.then(|| SliceRecords {
                    wet: dep(&st.wetgrid),
                    dry: dep(&st.drygrid),
                    field: SparseField::pack(
                        cells3(),
                        |c| st.grid[c] > SMALLNUM,
                        |f, c| {
                            if set.particle_count_output {
                                f * st.grid[c]
                            } else {
                                f * st.grid[c] * factor3d[c] / tot_mu(ks, kp)
                            }
                        },
                    ),
                });
                let pptv = write_ppt.then(|| SliceRecords {
                    wet: dep(&st.wetgrid),
                    dry: dep(&st.drygrid),
                    field: SparseField::pack(
                        cells3(),
                        |c| st.grid[c] > SMALLNUM,
                        |f, c| {
                            f * 1.0e12 * st.grid[c] / out.volume[c] / set.outnum * WEIGHTAIR
                                / set.weightmolar[ks]
                                / density[c]
                        },
                    ),
                });
                slices.push(SliceOutput {
                    ks,
                    kp,
                    nage,
                    conc,
                    pptv,
                    stats: st,
                });
            }
        }
    }

    let factor_drygrid_sparse = SparseField::pack(
        (0..nz_written).flat_map(|kz| {
            (0..ncell2).map(move |c2| (c2 as i64 + ((kz + 1) * ncell2) as i64, c2 + ncell2 * kz))
        }),
        |c| factor_drygrid[c] > 1.0 + SMALLNUM || factor_drygrid[c] < 1.0 - SMALLNUM,
        |f, c| f * factor_drygrid[c],
    );

    let uncertainty = match set.routine {
        Routine::Nest => None,
        _ => {
            let mut u = Uncertainty {
                gridtotalunc: 0.0,
                wetgridtotalunc: 0.0,
                drygridtotalunc: 0.0,
            };
            if gridtotal > 0.0 {
                u.gridtotalunc = gridsigmatotal / gridtotal;
            }
            if wetgridtotal > 0.0 {
                u.wetgridtotalunc = wetgridsigmatotal / wetgridtotal;
            }
            if drygridtotal > 0.0 {
                u.drygridtotalunc = drygridsigmatotal / drygridtotal;
            }
            Some(u)
        }
    };

    let receptors_out = match set.routine {
        Routine::Nest => None,
        _ if nreceptor == 0 => None,
        _ => {
            let c = |i: usize, ks: usize| receptors.creceptor[i + nreceptor * ks];
            let pptv = write_ppt.then(|| {
                (0..nspec)
                    .map(|ks| {
                        (0..nreceptor)
                            .map(|i| {
                                1.0e12 * c(i, ks) / set.outnum * WEIGHTAIR
                                    / set.weightmolar[ks]
                                    / rec_density[i]
                            })
                            .collect()
                    })
                    .collect()
            });
            let conc = (0..nspec)
                .map(|ks| {
                    (0..nreceptor)
                        .map(|i| 1.0e12 * c(i, ks) / set.outnum)
                        .collect()
                })
                .collect();
            Some(ReceptorOutput {
                conc,
                pptv,
                factor_dry: factor_dryrecept,
                density: rec_density,
            })
        }
    };

    Ok(ConcOutput {
        density,
        density_dry,
        factor_drygrid,
        factor3d,
        slices,
        factor_drygrid_sparse,
        uncertainty,
        receptors: receptors_out,
    })
}
