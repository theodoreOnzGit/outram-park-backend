// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/conccalc.f90 (subroutine conccalc),
//                    src/drydepokernel.f90, src/drydepokernel_nest.f90
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Gridding of particle mass onto the output grids: concentrations
//! (`conccalc.f90`) and dry deposition (`drydepokernel.f90`,
//! `drydepokernel_nest.f90`).
//!
//! # The two kernels
//!
//! * **Uniform kernel** on the output grids. A particle's mass is shared
//!   between the cell it is in and the three neighbours towards its nearest
//!   corner, with bilinear "tent" weights `wx = 1.5 - ddx` or `0.5 + ddx`
//!   (and the same in y) — a box of one cell width centred on the particle.
//!   `conccalc` falls back to **direct attribution** (whole mass to one cell)
//!   for particles younger than 10 800 s, for particles within half a cell
//!   of the grid edge, and when the kernel is switched off
//!   (`lusekerneloutput = .false.`).
//! * **Parabolic (Epanechnikov) kernel** at receptor points:
//!   `K = 0.596831 (1 - r²)` on the ellipsoid
//!   `r² = (Δx/hx)² + (Δy/hy)² + (z/hz)²`, with age-dependent bandwidths
//!   `hz = min(50 + 0.3 sqrt(age), 150) m`,
//!   `hx = min((0.29 + 2.222e-3 sqrt(age)) dx + 1.2e-5 age, 6)`,
//!   `hy = min((0.18 + 1.389e-3 sqrt(age)) dy + 7.5e-6 age, 4)` grid units.
//!   `0.596831 = 15/(8π)` normalises the kernel over a half ellipsoid; the
//!   final `2 * weight * c / receptorarea` accounts for the ground.
//!
//! # One function per domain, not one per upstream copy
//!
//! The nested-output block of `conccalc` repeats the mother-grid block on the
//! nest arrays; [`conccalc`] runs one Rust routine on each domain. The two
//! copies are **not** textually identical: in the backward-deposition
//! (`DRYBKDEP`/`WETBKDEP`) kernel branch the mother grid multiplies
//! `xmass/rhoi*w*weight` at the `(ix,jy)` and `(ixp,jyp)` corners but
//! `xmass/rhoi*weight*w` at the other two, while the nest uses `*weight*w`
//! at all four. With upstream's weights (0.5 or 1.0, exact powers of two)
//! the order is invisible; for any other weight it changes the last bit. The
//! port reproduces both orders ([`ProductOrder`]).
//!
//! `drydepokernel_nest` is `drydepokernel` on the nest grid **without the
//! `lusekerneloutput` switch**: the nest always uses the kernel. The port is
//! one function, [`drydepokernel`], with an [`Attribution`] argument;
//! [`drydepokernel_nest`] calls it with [`Attribution::UniformKernel`].
//!
//! # Upstream quirks reproduced
//!
//! * `conccalc` computes the cell as `ix = int(xl); if (xl < 0) ix = ix - 1`,
//!   a floor that is wrong at negative integers (`xl = -1.0` gives `-2`); it
//!   only affects cells off the grid. `drydepokernel` has **no** correction:
//!   `int()` truncates toward zero, so `-1 < xl < 0` lands in column 0 with
//!   `ddx < 0` and a kernel weight `wx = 0.5 + ddx < 0.5`.
//! * `drydepokernel` uses the kernel right up to the grid edge (no half-cell
//!   fallback, no age threshold), so mass whose kernel corners fall off the
//!   grid is lost; `conccalc` instead attributes such particles directly.
//! * The kernel branch takes the `else` side when `ddx` is exactly 0.5
//!   (`ixp = ix - 1`, `wx = 1`): all of the x weight stays in the cell.
//! * With `ind_samp = -1` (mass mixing ratio) the density at the particle is
//!   interpolated with the first corner from slot `memind(2)` and the other
//!   three from **slot 2 literally** (`rho(ix,jy,ind,memind(2)) +
//!   p2*rho(ixp,jy,ind,2) + ...`); after the fields swap (`memind(2) = 1`)
//!   the four corners come from different times. Reproduced.
//! * The density branch's north-pole fix compares `jyp` with the
//!   compile-time `nymax`, not `ny` (see [`GriddedMet::ny`]), and has no
//!   check on `ixp`.
//! * The age class loop `do nage=1,nageclass; if (itage < lage(nage)) exit`
//!   leaves `nage = nageclass + 1` for a particle older than the last class
//!   boundary, and upstream then writes **past the end** of `gridunc`.
//!   `timemanager` terminates such particles first, so in a normal run it
//!   does not happen; the port refuses ([`ConcCalcError::AgeBeyondLastClass`]).
//! * The density branch's level search leaves `indz` unassigned (stale) for
//!   `ztra1 >= height(nz)`; it runs for **every** particle at `itime`, even
//!   one above the output grid. The port refuses
//!   ([`ConcCalcError::AboveTopLevel`]).
//! * Any `ind_samp` other than 0 / -1 would leave `rhoi` stale; it cannot be
//!   expressed through [`SamplingUnit`].
//! * **Upstream defect: particle-count output with the kernel on mixes
//!   units.** `lparticlecountoutput` is tested only in the direct-attribution
//!   branch; the kernel branch adds `xmass/rhoi*weight*w` regardless. With
//!   `lparticlecountoutput = .true.` and `lusekerneloutput = .true.`, young
//!   particles and particles near the edge add 1 per species, older ones add
//!   mass. Observed in the code-to-code fixture (variant V3: 34 count cells
//!   and 88 mass cells in one grid) and reproduced by the port.
//! * The receptor estimate ignores the particle's age class and release
//!   point, uses `zd = ztra1/hz` (one-sided: no `zd < -1` test), and only
//!   counts particles with `r2 < 1` strictly.
//! * **Storage precision.** Upstream accumulates `gridunc`/`creceptor` in
//!   default `real` (single precision as shipped) and the deposition grids in
//!   `real(dep_prec)`, where `dep_prec = sp` is `real(4)` **even in a
//!   `-fdefault-real-8` build**. The port accumulates in `f64`; a caller
//!   wanting upstream's storage rounds to `f32` itself.
//!
//! # Index conventions
//!
//! Output-grid columns/rows `ix`, `jy` are 0-based, as upstream's
//! `0:numxgrid-1`; output level, species, release point, uncertainty class
//! and age class are **0-based here** (upstream's are 1-based).
//!
//! # Units
//!
//! Positions in meteorological grid units (`xtra1`, `ytra1`), heights m,
//! times s; `dx`, `dy`, `dxout`, `dyout`, `xoutshift`, `youtshift` in
//! degrees; masses kg; receptor areas m². `gridunc` accumulates
//! `mass/rhoi * weight` (kg, or kg/(kg/m³) for a mixing ratio).

use crate::flexpart::particle_average::{Corners, GriddedMet, Vertical};

/// `factor = .596831` of `conccalc.f90` (15/(8 pi), Epanechnikov kernel).
const KERNEL_FACTOR: f64 = 0.596_831;
/// `hxmax = 6.0`, grid units.
const HXMAX: f64 = 6.0;
/// `hymax = 4.0`, grid units.
const HYMAX: f64 = 4.0;
/// `hzmax = 150.`, m.
const HZMAX: f64 = 150.0;
/// Age below which `conccalc` never uses the kernel, s.
const KERNEL_MIN_AGE: i64 = 10_800;

/// Geometry of one output grid (`numxgrid`, `numygrid`, `dxout`, `dyout`,
/// `xoutshift`, `youtshift`, or their nest counterparts with suffix `n`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputDomain {
    /// Number of output columns.
    pub numxgrid: usize,
    /// Number of output rows.
    pub numygrid: usize,
    /// Output cell width, degrees.
    pub dxout: f64,
    /// Output cell height, degrees.
    pub dyout: f64,
    /// `xlon0 - outlon0`, degrees.
    pub xoutshift: f64,
    /// `ylat0 - outlat0`, degrees.
    pub youtshift: f64,
}

impl OutputDomain {
    /// `xl = (x*dx + xoutshift)/dxout`, `yl` likewise: the particle position
    /// in output-cell units.
    fn to_cells(self, x: f64, y: f64, dx: f64, dy: f64) -> (f64, f64) {
        (
            (x * dx + self.xoutshift) / self.dxout,
            (y * dy + self.youtshift) / self.dyout,
        )
    }

    fn contains(&self, ix: i64, jy: i64) -> bool {
        // `ix.le.numxgrid-1` etc. upstream; the same on integers.
        ix >= 0 && jy >= 0 && ix < self.numxgrid as i64 && jy < self.numygrid as i64
    }
}

/// A 7-D concentration accumulator, `gridunc(0:numxgrid-1, 0:numygrid-1,
/// numzgrid, nspec, npointspec, nclassunc, nageclass)` (or `griduncn`).
#[derive(Debug, Clone, PartialEq)]
pub struct ConcentrationGrid {
    /// Columns.
    pub nx: usize,
    /// Rows.
    pub ny: usize,
    /// Output levels.
    pub nz: usize,
    /// Species.
    pub nspec: usize,
    /// Release points with separate output (`maxpointspec_act`).
    pub npointspec: usize,
    /// Uncertainty classes (`nclassunc`).
    pub nclassunc: usize,
    /// Age classes.
    pub nageclass: usize,
    /// Values, `ix` fastest, then `jy`, `kz`, `ks`, release, class, age.
    pub values: Vec<f64>,
}

impl ConcentrationGrid {
    /// All-zero grid.
    #[must_use]
    pub fn zeros(
        nx: usize,
        ny: usize,
        nz: usize,
        nspec: usize,
        npointspec: usize,
        nclassunc: usize,
        nageclass: usize,
    ) -> Self {
        Self {
            nx,
            ny,
            nz,
            nspec,
            npointspec,
            nclassunc,
            nageclass,
            values: vec![0.0; nx * ny * nz * nspec * npointspec * nclassunc * nageclass],
        }
    }

    /// Flat index; all indices 0-based.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn index(
        &self,
        ix: usize,
        jy: usize,
        kz: usize,
        ks: usize,
        kp: usize,
        nc: usize,
        na: usize,
    ) -> usize {
        ix + self.nx
            * (jy
                + self.ny
                    * (kz
                        + self.nz
                            * (ks
                                + self.nspec
                                    * (kp + self.npointspec * (nc + self.nclassunc * na)))))
    }
}

/// A 6-D deposition accumulator, `drygridunc(0:numxgrid-1, 0:numygrid-1,
/// nspec, npointspec, nclassunc, nageclass)` (or `drygriduncn`).
#[derive(Debug, Clone, PartialEq)]
pub struct DepositionGrid {
    /// Columns.
    pub nx: usize,
    /// Rows.
    pub ny: usize,
    /// Species.
    pub nspec: usize,
    /// Release points with separate output.
    pub npointspec: usize,
    /// Uncertainty classes.
    pub nclassunc: usize,
    /// Age classes.
    pub nageclass: usize,
    /// Values, `ix` fastest, then `jy`, `ks`, release, class, age.
    pub values: Vec<f64>,
}

impl DepositionGrid {
    /// All-zero grid.
    #[must_use]
    pub fn zeros(
        nx: usize,
        ny: usize,
        nspec: usize,
        npointspec: usize,
        nclassunc: usize,
        nageclass: usize,
    ) -> Self {
        Self {
            nx,
            ny,
            nspec,
            npointspec,
            nclassunc,
            nageclass,
            values: vec![0.0; nx * ny * nspec * npointspec * nclassunc * nageclass],
        }
    }

    /// Flat index; all indices 0-based.
    #[must_use]
    pub fn index(&self, ix: usize, jy: usize, ks: usize, kp: usize, nc: usize, na: usize) -> usize {
        ix + self.nx
            * (jy
                + self.ny * (ks + self.nspec * (kp + self.npointspec * (nc + self.nclassunc * na))))
    }
}

/// `ind_samp`: what a sampled particle contributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SamplingUnit {
    /// `ind_samp = 0`: mass (`rhoi = 1`).
    Mass,
    /// `ind_samp = -1`: mass divided by the air density at the particle.
    MassMixingRatio,
}

/// The particle arrays `conccalc` reads from `com_mod`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Particles {
    /// Time each particle is at, s (`itra1`).
    pub itra1: Vec<i64>,
    /// Release time, s (`itramem`).
    pub itramem: Vec<i64>,
    /// x position, grid units (`xtra1`).
    pub xtra1: Vec<f64>,
    /// y position, grid units (`ytra1`).
    pub ytra1: Vec<f64>,
    /// Height above ground, m (`ztra1`).
    pub ztra1: Vec<f64>,
    /// Release point, 0-based (`npoint - 1`).
    pub npoint: Vec<usize>,
    /// Uncertainty class, 0-based (`nclass - 1`).
    pub nclass: Vec<usize>,
    /// Number of species stored per particle in `xmass1`/`xscav_frac1`.
    pub nspec: usize,
    /// Mass per species, kg, `[i*nspec + ks]` (`xmass1`).
    pub xmass1: Vec<f64>,
    /// Scavenged fraction per species, `[i*nspec + ks]` (`xscav_frac1`).
    pub xscav_frac1: Vec<f64>,
}

impl Particles {
    /// Number of particles (`numpart`).
    #[must_use]
    pub fn len(&self) -> usize {
        self.itra1.len()
    }

    /// `true` when there are no particles.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.itra1.is_empty()
    }
}

/// The `com_mod` / `par_mod` switches and tables `conccalc` reads.
#[derive(Debug, Clone, PartialEq)]
pub struct ConcCalcSettings {
    /// `ind_samp`.
    pub sampling: SamplingUnit,
    /// `ioutputforeachrelease = 1`.
    pub output_for_each_release: bool,
    /// `mdomainfill = 1` (then `npoint` is a particle number and all mass
    /// goes to release slot 0).
    pub domain_filling: bool,
    /// `lusekerneloutput` (a `par_mod` parameter upstream).
    pub use_kernel: bool,
    /// `DRYBKDEP .or. WETBKDEP`: weight mass by `max(xscav_frac1, 0)`.
    pub backward_deposition: bool,
    /// `lparticlecountoutput` (a `par_mod` parameter): count particles
    /// instead of mass (direct attribution only; the kernel still grids mass).
    pub particle_count_output: bool,
    /// Upper age of each age class, s (`lage(1:nageclass)`).
    pub lage: Vec<i64>,
    /// Upper height of each output level, m (`outheight`).
    pub outheight: Vec<f64>,
    /// Meteorological grid spacing in x, degrees (`dx`).
    pub dx: f64,
    /// Meteorological grid spacing in y, degrees (`dy`).
    pub dy: f64,
    /// Species to grid (`nspec`).
    pub nspec: usize,
}

/// Receptor points (`xreceptor`, `yreceptor` in grid units, `receptorarea`
/// in m²).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Receptors {
    /// x, grid units.
    pub x: Vec<f64>,
    /// y, grid units.
    pub y: Vec<f64>,
    /// Area of a 1 x 1 grid cell at the receptor, m².
    pub area: Vec<f64>,
}

/// Why [`conccalc`] refused. The index is the particle's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcCalcError {
    /// Age at or beyond `lage(nageclass)`: upstream writes past `gridunc`.
    AgeBeyondLastClass(usize),
    /// `ztra1 >= height(nz)` with `ind_samp = -1`: upstream's level index is
    /// stale.
    AboveTopLevel(usize),
    /// The density corners fall outside the supplied arrays.
    DensityOutsideGrid(usize),
    /// `ind_samp = -1` but no density field supplied.
    MissingDensity,
    /// Release point or uncertainty class outside the concentration grid.
    IndexOutsideGrid(usize),
}

/// Multiplication order of the backward-deposition kernel products (see the
/// module doc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductOrder {
    /// Mother grid: `*w*weight` at `(ix,jy)` and `(ixp,jyp)`, `*weight*w` at
    /// the other corners.
    Mother,
    /// Nest: `*weight*w` at every corner.
    Nest,
}

/// Per-particle values shared by both domains.
struct Sample {
    i: usize,
    itage: i64,
    kz: usize,
    nrel: usize,
    ncl: usize,
    nage: usize,
    rhoi: f64,
}

/// Grid one particle onto one domain (`conccalc.f90:156-295` for the mother
/// grid, `:301-441` for the nest).
fn grid_on_domain(
    s: &Sample,
    weight: f64,
    p: &Particles,
    set: &ConcCalcSettings,
    dom: &OutputDomain,
    grid: &mut ConcentrationGrid,
    order: ProductOrder,
) {
    let i = s.i;
    let (xl, yl) = dom.to_cells(p.xtra1[i], p.ytra1[i], set.dx, set.dy);
    let mut ix = xl.trunc() as i64;
    if xl < 0.0 {
        ix -= 1;
    }
    let mut jy = yl.trunc() as i64;
    if yl < 0.0 {
        jy -= 1;
    }
    let mass = |ks: usize| p.xmass1[i * p.nspec + ks];
    let scav = |ks: usize| f64::max(p.xscav_frac1[i * p.nspec + ks], 0.0);
    let add = |gx: i64, gy: i64, ks: usize, v: f64, grid: &mut ConcentrationGrid| {
        let k = grid.index(gx as usize, gy as usize, s.kz, ks, s.nrel, s.ncl, s.nage);
        grid.values[k] += v;
    };

    if !set.use_kernel
        || s.itage < KERNEL_MIN_AGE
        || xl < 0.5
        || yl < 0.5
        || xl > (dom.numxgrid as i64 - 1) as f64 - 0.5
        || yl > (dom.numygrid as i64 - 1) as f64 - 0.5
    {
        if dom.contains(ix, jy) {
            for ks in 0..set.nspec {
                let v = if set.backward_deposition {
                    mass(ks) / s.rhoi * weight * scav(ks)
                } else if set.particle_count_output {
                    1.0
                } else {
                    mass(ks) / s.rhoi * weight
                };
                add(ix, jy, ks, v, grid);
            }
        }
        return;
    }

    let ddx = xl - ix as f64;
    let ddy = yl - jy as f64;
    let (ixp, wx) = if ddx > 0.5 {
        (ix + 1, 1.5 - ddx)
    } else {
        (ix - 1, 0.5 + ddx)
    };
    let (jyp, wy) = if ddy > 0.5 {
        (jy + 1, 1.5 - ddy)
    } else {
        (jy - 1, 0.5 + ddy)
    };

    // Backward-deposition product: `w*weight` (true) or `weight*w` (false).
    let corner = |ks: usize, w: f64, w_first: bool| -> f64 {
        if set.backward_deposition {
            if w_first {
                mass(ks) / s.rhoi * w * weight * scav(ks)
            } else {
                mass(ks) / s.rhoi * weight * w * scav(ks)
            }
        } else {
            mass(ks) / s.rhoi * weight * w
        }
    };
    let diag_w_first = order == ProductOrder::Mother;

    let xin = |x: i64| x >= 0 && x < dom.numxgrid as i64;
    let yin = |y: i64| y >= 0 && y < dom.numygrid as i64;

    if xin(ix) {
        if yin(jy) {
            let w = wx * wy;
            for ks in 0..set.nspec {
                add(ix, jy, ks, corner(ks, w, diag_w_first), grid);
            }
        }
        if yin(jyp) {
            let w = wx * (1.0 - wy);
            for ks in 0..set.nspec {
                add(ix, jyp, ks, corner(ks, w, false), grid);
            }
        }
    }
    if xin(ixp) {
        if yin(jyp) {
            let w = (1.0 - wx) * (1.0 - wy);
            for ks in 0..set.nspec {
                add(ixp, jyp, ks, corner(ks, w, diag_w_first), grid);
            }
        }
        if yin(jy) {
            let w = (1.0 - wx) * wy;
            for ks in 0..set.nspec {
                add(ixp, jy, ks, corner(ks, w, false), grid);
            }
        }
    }
}

/// `conccalc.f90`: add the particles at time `itime` to the concentration
/// grids (mother, and nest when given) with sampling weight `weight`, and add
/// the receptor concentrations to `creceptor` (`[n*nspec + ks]`).
///
/// `density` is required for [`SamplingUnit::MassMixingRatio`] (its `rho`,
/// `height`, `nz`, `memind` and `ny` — read as upstream's `nymax` — are
/// used). On error nothing has been modified.
#[allow(clippy::too_many_arguments)]
pub fn conccalc(
    itime: i64,
    weight: f64,
    particles: &Particles,
    settings: &ConcCalcSettings,
    density: Option<&GriddedMet>,
    mother: &OutputDomain,
    gridunc: &mut ConcentrationGrid,
    nest: Option<(&OutputDomain, &mut ConcentrationGrid)>,
    receptors: &Receptors,
    creceptor: &mut [f64],
) -> Result<(), ConcCalcError> {
    let p = particles;
    let set = settings;

    // Pass 1: everything upstream computes per particle before gridding,
    // validated before anything is written.
    let mut samples = Vec::new();
    for i in 0..p.len() {
        if p.itra1[i] != itime {
            continue;
        }
        let itage = (p.itra1[i] - p.itramem[i]).abs();
        let nage = set
            .lage
            .iter()
            .position(|&l| itage < l)
            .ok_or(ConcCalcError::AgeBeyondLastClass(i))?;

        let rhoi = match set.sampling {
            SamplingUnit::Mass => 1.0,
            SamplingUnit::MassMixingRatio => {
                let met = density.ok_or(ConcCalcError::MissingDensity)?;
                let c = Corners::new(met, p.xtra1[i], p.ytra1[i], true)
                    .ok_or(ConcCalcError::DensityOutsideGrid(i))?;
                let v = Vertical::new(met, p.ztra1[i]).ok_or(ConcCalcError::AboveTopLevel(i))?;
                let mut rhoprof = [0.0; 2];
                for (n, ind) in [v.indz, v.indz + 1].into_iter().enumerate() {
                    let at = |x: usize, y: usize, slot: usize| met.rho[met.idx3(x, y, ind, slot)];
                    rhoprof[n] = c.p1 * at(c.ix, c.jy, met.memind[1])
                        + c.p2 * at(c.ixp, c.jy, 1)
                        + c.p3 * at(c.ix, c.jyp, 1)
                        + c.p4 * at(c.ixp, c.jyp, 1);
                }
                (v.dz1 * rhoprof[1] + v.dz2 * rhoprof[0]) * v.dz
            }
        };

        let nrel = if !set.output_for_each_release || set.domain_filling {
            0
        } else {
            p.npoint[i]
        };

        let Some(kz) = set.outheight.iter().position(|&h| h > p.ztra1[i]) else {
            continue; // above the output domain
        };
        let ncl = p.nclass[i];
        let fits = |g: &ConcentrationGrid| {
            nrel < g.npointspec
                && ncl < g.nclassunc
                && nage < g.nageclass
                && kz < g.nz
                && set.nspec <= g.nspec
        };
        if !fits(gridunc) || nest.as_ref().is_some_and(|(_, g)| !fits(g)) {
            return Err(ConcCalcError::IndexOutsideGrid(i));
        }
        samples.push(Sample {
            i,
            itage,
            kz,
            nrel,
            ncl,
            nage,
            rhoi,
        });
    }

    // Pass 2: section 1, the output grids.
    let mut nest = nest;
    for s in &samples {
        grid_on_domain(s, weight, p, set, mother, gridunc, ProductOrder::Mother);
        if let Some((dom, grid)) = nest.as_mut() {
            grid_on_domain(s, weight, p, set, dom, grid, ProductOrder::Nest);
        }
    }

    // Section 2: receptors, parabolic kernel.
    let mut c = vec![0.0; set.nspec];
    for n in 0..receptors.x.len() {
        c.iter_mut().for_each(|v| *v = 0.0);
        for i in 0..p.len() {
            if p.itra1[i] != itime {
                continue;
            }
            let itage = (p.itra1[i] - p.itramem[i]).abs();
            let age = itage as f64;

            let hz = f64::min(50.0 + 0.3 * age.sqrt(), HZMAX);
            let zd = p.ztra1[i] / hz;
            if zd > 1.0 {
                continue;
            }
            let hx = f64::min(
                (0.29 + 2.222e-3 * age.sqrt()) * set.dx + age * 1.2e-5,
                HXMAX,
            );
            let xd = (p.xtra1[i] - receptors.x[n]) / hx;
            if xd * xd > 1.0 {
                continue;
            }
            let hy = f64::min(
                (0.18 + 1.389e-3 * age.sqrt()) * set.dy + age * 7.5e-6,
                HYMAX,
            );
            let yd = (p.ytra1[i] - receptors.y[n]) / hy;
            if yd * yd > 1.0 {
                continue;
            }
            let h = hx * hy * hz;
            let r2 = xd * xd + yd * yd + zd * zd;
            if r2 < 1.0 {
                let xkern = KERNEL_FACTOR * (1.0 - r2);
                for (ks, cv) in c.iter_mut().enumerate() {
                    *cv += p.xmass1[i * p.nspec + ks] * xkern / h;
                }
            }
        }
        for (ks, &cv) in c.iter().enumerate() {
            creceptor[n * set.nspec + ks] += 2.0 * weight * cv / receptors.area[n];
        }
    }
    Ok(())
}

/// How `drydepokernel` attributes a deposit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attribution {
    /// `lusekerneloutput = .false.`: the whole deposit to the particle's cell.
    DirectCell,
    /// The uniform kernel (four cells).
    UniformKernel,
}

/// Why a deposition call refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepositionError {
    /// `nunc`, `nage`, `kp` or the species count outside the grid.
    IndexOutsideGrid,
}

/// `drydepokernel.f90`: add one particle's dry deposit (`deposit[ks]`, kg)
/// at grid position `(x, y)` (meteorological grid units) to `grid`, for the
/// species with `drydepspec[ks]` and a non-zero deposit. `nunc`, `nage`, `kp`
/// are the 0-based uncertainty class, age class and release slot.
///
/// The cell is `int(xl)` (truncation toward zero, see the module doc). With
/// [`Attribution::UniformKernel`] the four corners are tested and filled in
/// upstream's order `(ix,jy)`, `(ixp,jyp)`, `(ixp,jy)`, `(ix,jyp)`.
#[allow(clippy::too_many_arguments)]
pub fn drydepokernel(
    domain: &OutputDomain,
    dx: f64,
    dy: f64,
    attribution: Attribution,
    deposit: &[f64],
    drydepspec: &[bool],
    x: f64,
    y: f64,
    nunc: usize,
    nage: usize,
    kp: usize,
    grid: &mut DepositionGrid,
) -> Result<(), DepositionError> {
    let nspec = deposit.len();
    if nunc >= grid.nclassunc
        || nage >= grid.nageclass
        || kp >= grid.npointspec
        || nspec > grid.nspec
        || drydepspec.len() < nspec
    {
        return Err(DepositionError::IndexOutsideGrid);
    }
    let (xl, yl) = domain.to_cells(x, y, dx, dy);
    let ix = xl.trunc() as i64;
    let jy = yl.trunc() as i64;
    let ddx = xl - ix as f64;
    let ddy = yl - jy as f64;
    let (ixp, wx) = if ddx > 0.5 {
        (ix + 1, 1.5 - ddx)
    } else {
        (ix - 1, 0.5 + ddx)
    };
    let (jyp, wy) = if ddy > 0.5 {
        (jy + 1, 1.5 - ddy)
    } else {
        (jy - 1, 0.5 + ddy)
    };

    let mut add = |gx: i64, gy: i64, ks: usize, v: f64| {
        if domain.contains(gx, gy) {
            let k = grid.index(gx as usize, gy as usize, ks, kp, nunc, nage);
            grid.values[k] += v;
        }
    };

    for ks in 0..nspec {
        if !(deposit[ks].abs() > 0.0 && drydepspec[ks]) {
            continue;
        }
        match attribution {
            Attribution::DirectCell => add(ix, jy, ks, deposit[ks]),
            Attribution::UniformKernel => {
                add(ix, jy, ks, deposit[ks] * (wx * wy));
                add(ixp, jyp, ks, deposit[ks] * ((1.0 - wx) * (1.0 - wy)));
                add(ixp, jy, ks, deposit[ks] * ((1.0 - wx) * wy));
                add(ix, jyp, ks, deposit[ks] * (wx * (1.0 - wy)));
            }
        }
    }
    Ok(())
}

/// `drydepokernel_nest.f90`: [`drydepokernel`] on a nested output grid. The
/// nest version has no `lusekerneloutput` switch, so it **always** uses the
/// uniform kernel, even when the mother grid attributes directly.
#[allow(clippy::too_many_arguments)]
pub fn drydepokernel_nest(
    nest: &OutputDomain,
    dx: f64,
    dy: f64,
    deposit: &[f64],
    drydepspec: &[bool],
    x: f64,
    y: f64,
    nunc: usize,
    nage: usize,
    kp: usize,
    grid: &mut DepositionGrid,
) -> Result<(), DepositionError> {
    drydepokernel(
        nest,
        dx,
        dy,
        Attribution::UniformKernel,
        deposit,
        drydepspec,
        x,
        y,
        nunc,
        nage,
        kp,
        grid,
    )
}
