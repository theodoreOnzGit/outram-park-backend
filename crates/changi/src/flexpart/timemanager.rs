// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/timemanager.f90 (subroutine timemanager), lines
//                    118-121, 151-504 (output clock, deposition decay,
//                    particle splitting), 509-513 (ldeltat), 532-705 (the
//                    per-particle step)
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! The per-synchronisation-step bookkeeping of `timemanager.f90`, as
//! deterministic functions. File I/O, `getfields`/`readwind`, particle
//! release and domain filling are out of scope; the routines `timemanager`
//! calls are ported in their own modules and are named below.
//!
//! # One synchronisation step, in upstream's order
//!
//! `do itime = 0, ideltas, lsynctime` (line 151):
//!
//! 1. `wetdepo` if `WETDEP`, `itime /= 0` and there are particles
//!    ([`crate::flexpart::wet_deposition::wetdepo`]);
//! 2. `ohreaction` under the same condition
//!    ([`crate::flexpart::oh_chemistry::ohreaction`]);
//! 3. backward runs with convection: `convmix` for `itime < 0`
//!    ([`crate::flexpart::convmix`]);
//! 4. `getfields` (not ported, I/O), `gethourlyOH` if `OHREA`;
//! 5. release (`releaseparticles`, or `init_domainfill` /
//!    `boundcond_domainfill`; not ported);
//! 6. forward runs with convection: `convmix`;
//! 7. **decay of the deposition grids** at the middle of the averaging
//!    interval ([`ClockEvents::decay_deposition`],
//!    [`decay_deposition_grid`]);
//! 8. inside the averaging window: **sampling** (`conccalc`,
//!    [`crate::flexpart::concentration::conccalc`]) with weight ½ at the
//!    window ends; at the window end, **output** (`concoutput*`,
//!    [`crate::flexpart::concoutput::concoutput`]), the clock moves on one
//!    output step, the new window's first sample, and **particle splitting**
//!    ([`split_particles`]);
//! 9. `exit` at `itime == ideltas`;
//! 10. `ldeltat`, the time since the deposition decay was last applied;
//! 11. for every particle due at `itime` ([`particle_due`]):
//!     [`pre_advance`] (release slot, age class, `initialize` for new
//!     particles, backward scavenging initialisation), then `advance`
//!     ([`crate::flexpart::advance::advance`]), then `partpos_average` /
//!     `calcfluxes` (optional output), then [`post_advance`] (termination,
//!     radioactive decay and dry-deposition mass removal, the `minmass`
//!     test, `drydepokernel` calls, the age limit).
//!
//! Termination by **height or leaving the domain** happens inside `advance`
//! (it returns `nstop = 3`); [`post_advance`] then only marks the particle.
//!
//! # Upstream quirks reproduced (and documented, not fixed)
//!
//! * **Particles are split only at output times.** The split test (line
//!   468) sits inside `if (itime == loutend .and. outnum > 0)`, so a
//!   particle whose `itrasplit` passes between outputs waits for the next
//!   output.
//! * **With `iout = 4` (plume trajectories only) `outnum` is never reset**
//!   (the reset, line 431, is inside `if (iout <= 3 .or. iout == 5)`), so it
//!   grows for the whole run. Nothing reads it then.
//! * **The clock stalls if `loutend` is never sampled with a positive
//!   `outnum`**: the clock only advances inside the output branch. Upstream
//!   reaches `loutend` with `outnum > 0` whenever any sample fell in the
//!   window; the sweep includes `loutsample` not dividing the window, where
//!   `loutend` itself is not sampled but output still happens.
//! * **A particle whose release mass `xmass(npoint, ks)` is 0 for every
//!   species is terminated by the `minmass` test** (`xmassfract` stays 0),
//!   unless domain filling or quasi-Lagrangian mode skips the test.
//! * **The dry deposit is passed to `drydepokernel` for a particle that has
//!   just been terminated for small mass**: the `minmass` termination comes
//!   first and does not skip the deposition call.
//! * **`drydeposit` is a timemanager local kept between particles**: for a
//!   species without dry deposition it is not assigned, and
//!   `drydepokernel` receives the previous particle's value (it ignores it,
//!   since it tests `DRYDEPSPEC` itself). The port takes the buffer as an
//!   argument so the stale value is visible.
//! * **The age termination calls `initial_cond_calc` again** after a
//!   `minmass` termination when `linit_cond >= 1`: `itra1 = -999999999`
//!   then makes `|itra1 - itramem| >= lage(nageclass)` true.
//! * **The decay back-dating of the dry deposit uses `|ldeltat|`**, and
//!   `ldeltat` is computed with `itime < loutnext`, a test that is
//!   direction-blind; dry deposition onto the output grid only happens in
//!   forward runs, so backward runs never use it.
//! * **The age class of a particle older than the last class boundary is
//!   `nageclass + 1`** (the `do ... exit` loop runs out), out of the bounds of
//!   every age-classed grid. [`pre_advance`] returns `None`.
//! * **`get_wetscav`'s counters are 64-bit arrays, `timemanager` passes a
//!   default-integer scalar** (`call get_wetscav(...,idummy,idummy,wetscav)`,
//!   line 583, against `integer(selected_int_kind(16)), dimension(nspec)
//!   :: blc_count, inc_count` in `get_wetscav.f90:56`): a `WETBKDEP` run
//!   increments 8 bytes at `idummy + 8*(ks-1)`, out of bounds. Found by
//!   reading; not exercised (the counters do not affect the result).
//!
//! # Precision
//!
//! Default `real` is `f64` here (upstream's `-fdefault-real-8` build). The
//! `real(xtra1(j))` conversion of the `drydepokernel` call is therefore an
//! identity here and an `f32` rounding in the shipped build.
//!
//! # Units
//!
//! Times s (signed with `ldirect` in backward runs, as `readcommand.f90`
//! stores them), masses kg, decay constants s⁻¹, deposition probability
//! dimensionless.

use crate::flexpart::concentration::DepositionGrid;
use crate::flexpart::decay::surviving_fraction;

/// `minmass = 0.0001` (`par_mod.f90`): particles carrying less than this
/// fraction of their initial mass (all species) are terminated.
pub const MINMASS: f64 = 0.0001;

/// The value upstream stores in `itra1` of a terminated particle.
pub const TERMINATED: i64 = -999_999_999;

// ---------------------------------------------------------------------------
// Output clock
// ---------------------------------------------------------------------------

/// The `com_mod` settings the clock reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClockSettings {
    /// `+1` forward, `-1` backward.
    pub ldirect: i64,
    /// Output interval, s (`loutstep`, negative backward).
    pub loutstep: i64,
    /// Averaging time, s (`loutaver`).
    pub loutaver: i64,
    /// Sampling interval, s (`loutsample`).
    pub loutsample: i64,
    /// Time at which particles start to be split, s (`itsplit`).
    pub itsplit: i64,
    /// Output selector (`iout`).
    pub iout: i32,
    /// Any deposition switched on (`DEP`).
    pub dep: bool,
}

/// What happens at one `itime` (all `false`/`None` when nothing does).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClockEvents {
    /// Decay the deposition grids now (line 264).
    pub decay_deposition: bool,
    /// `conccalc` is called with this weight (lines 353-359).
    pub sample: Option<f64>,
    /// The output routines are called with this `outnum` (lines 371-430).
    pub output: Option<f64>,
    /// The new window starts now: `conccalc` with weight 0.5 (lines
    /// 455-459).
    pub resample: bool,
    /// The particle-split test runs (line 468).
    pub split: bool,
    /// `ldeltat` (lines 509-513), `None` on the last step (upstream exits
    /// first, line 504).
    pub ldeltat: Option<i64>,
}

/// The clock's state: `loutnext`, `loutstart`, `loutend`, `outnum`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputClock {
    /// Settings.
    pub settings: ClockSettings,
    /// Middle of the current averaging window, s.
    pub loutnext: i64,
    /// Window start, s.
    pub loutstart: i64,
    /// Window end, s.
    pub loutend: i64,
    /// Accumulated sample weight (`outnum`).
    pub outnum: f64,
}

impl OutputClock {
    /// Lines 118-121: the first window is centred on `loutstep/2` (integer
    /// division, truncating toward zero), half-width `loutaver/2`.
    #[must_use]
    pub fn new(settings: ClockSettings) -> Self {
        let loutnext = settings.loutstep / 2;
        Self {
            settings,
            loutnext,
            loutstart: loutnext - settings.loutaver / 2,
            loutend: loutnext + settings.loutaver / 2,
            outnum: 0.0,
        }
    }

    /// The events at `itime`, updating the clock. `last` is
    /// `itime == ideltas` (upstream exits before computing `ldeltat`).
    pub fn step(&mut self, itime: i64, last: bool) -> ClockEvents {
        let s = self.settings;
        let mut ev = ClockEvents {
            decay_deposition: deposition_decay_due(s.dep, itime, self.loutnext, s.ldirect),
            sample: None,
            output: None,
            resample: false,
            split: false,
            ldeltat: None,
        };
        if s.ldirect * itime >= s.ldirect * self.loutstart
            && s.ldirect * itime <= s.ldirect * self.loutend
        {
            // Fortran `mod` takes the dividend's sign, as Rust `%` does.
            if (itime - self.loutstart) % s.loutsample == 0 {
                let weight = if itime == self.loutstart || itime == self.loutend {
                    0.5
                } else {
                    1.0
                };
                self.outnum += weight;
                ev.sample = Some(weight);
            }
            if itime == self.loutend && self.outnum > 0.0 {
                if s.iout <= 3 || s.iout == 5 {
                    ev.output = Some(self.outnum);
                    self.outnum = 0.0;
                }
                self.loutnext += s.loutstep;
                self.loutstart = self.loutnext - s.loutaver / 2;
                self.loutend = self.loutnext + s.loutaver / 2;
                if itime == self.loutstart {
                    self.outnum += 0.5;
                    ev.resample = true;
                }
                ev.split = s.ldirect * itime >= s.ldirect * s.itsplit;
            }
        }
        if !last {
            ev.ldeltat = Some(if itime < self.loutnext {
                itime - (self.loutnext - s.loutstep)
            } else {
                itime - self.loutnext
            });
        }
        ev
    }
}

// ---------------------------------------------------------------------------
// Deposition decay at loutnext
// ---------------------------------------------------------------------------

/// Line 264: the deposition grids decay when deposition is on, the run is
/// forward and `itime` is the middle of the averaging interval.
#[must_use]
pub fn deposition_decay_due(dep: bool, itime: i64, loutnext: i64, ldirect: i64) -> bool {
    dep && itime == loutnext && ldirect > 0
}

/// Lines 264-299: when [`ClockEvents::decay_deposition`] fires, every
/// deposition grid value of a species with `decay(ks) > 0` is multiplied by
/// `exp(-outstep*decay(ks))` (a full output step: the deposits were
/// back-dated to the previous decay epoch, see [`post_advance`]). Apply to
/// the mother grids and, with nested output, to the nest grids.
///
/// `decay[ks]` in s⁻¹; `outstep = |loutstep|` s. Species with a
/// non-positive constant are left unchanged.
pub fn decay_deposition_grid(grid: &mut DepositionGrid, decay: &[f64], outstep: f64) {
    let per_species = grid.nx * grid.ny;
    for (block, chunk) in grid.values.chunks_mut(per_species).enumerate() {
        let ks = block % grid.nspec;
        if ks < decay.len() && decay[ks] > 0.0 {
            // exp(-1.*outstep*decay(ks)): (-1*outstep)*decay == -(outstep*decay).
            let f = surviving_fraction(decay[ks], outstep);
            for v in chunk {
                *v *= f;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Particle splitting
// ---------------------------------------------------------------------------

/// The particle fields the split copies (`com_mod` arrays at one index).
#[derive(Debug, Clone, PartialEq)]
pub struct SplitParticle {
    /// Current time, s (`itra1`).
    pub itra1: i64,
    /// Release time, s (`itramem`).
    pub itramem: i64,
    /// Next split time, s (`itrasplit`).
    pub itrasplit: i64,
    /// Time step, s (`idt`).
    pub idt: i64,
    /// Release point, 1-based (`npoint`).
    pub npoint: usize,
    /// Uncertainty class, 1-based (`nclass`).
    pub nclass: usize,
    /// Position, grid units (`xtra1`, `real(kind=dp)`).
    pub xtra1: f64,
    /// See [`SplitParticle::xtra1`].
    pub ytra1: f64,
    /// Height, m.
    pub ztra1: f64,
    /// Turbulent velocities, m/s (`uap`, `ucp`, `uzp`).
    pub uap: f64,
    /// See [`SplitParticle::uap`].
    pub ucp: f64,
    /// See [`SplitParticle::uap`].
    pub uzp: f64,
    /// Previous turbulent sigmas, m/s (`us`, `vs`, `ws`).
    pub us: f64,
    /// See [`SplitParticle::us`].
    pub vs: f64,
    /// See [`SplitParticle::us`].
    pub ws: f64,
    /// CBL flag (`cbt`, `integer(kind=2)`).
    pub cbt: i16,
    /// Mass per species, kg (`xmass1`).
    pub xmass1: Vec<f64>,
}

/// Lines 468-499: when `ldirect*itime >= ldirect*itsplit`, every particle
/// (of those present on entry) with `ldirect*itime >= ldirect*itrasplit` is
/// split in two while fewer than `maxpart` exist: its next split time
/// doubles its age at split (`itrasplit = 2*(itrasplit - itramem) +
/// itramem`), both halves carry half the mass, and the copy is appended.
/// Only called at output times (see the module doc).
pub fn split_particles(
    parts: &mut Vec<SplitParticle>,
    itime: i64,
    ldirect: i64,
    itsplit: i64,
    maxpart: usize,
) {
    if ldirect * itime < ldirect * itsplit {
        return;
    }
    let numpart = parts.len();
    for j in 0..numpart {
        if ldirect * itime >= ldirect * parts[j].itrasplit && parts.len() < maxpart {
            let p = &mut parts[j];
            p.itrasplit = 2 * (p.itrasplit - p.itramem) + p.itramem;
            for m in &mut p.xmass1 {
                *m /= 2.0;
            }
            let copy = p.clone();
            parts.push(copy);
        }
    }
}

// ---------------------------------------------------------------------------
// The per-particle step
// ---------------------------------------------------------------------------

/// Line 532: a particle is integrated at `itime` iff `itra1 == itime`
/// (terminated particles carry [`TERMINATED`]).
#[must_use]
pub fn particle_due(itra1: i64, itime: i64) -> bool {
    itra1 == itime
}

/// Settings [`pre_advance`] reads.
#[derive(Debug, Clone, PartialEq)]
pub struct PreAdvanceSettings {
    /// `ioutputforeachrelease == 1`: one output slot per release point.
    pub output_each_release: bool,
    /// Age class upper bounds, s (`lage(1:nageclass)`).
    pub lage: Vec<i64>,
    /// Backward dry-deposition receptor mode (`DRYBKDEP`).
    pub drybkdep: bool,
    /// Backward wet-deposition receptor mode (`WETBKDEP`).
    pub wetbkdep: bool,
    /// Per-species dry deposition (`DRYDEPSPEC`).
    pub drydepspec: Vec<bool>,
}

/// What [`pre_advance`] decided.
#[derive(Debug, Clone, PartialEq)]
pub struct PreAdvance {
    /// Output release slot, 0-based (`kp - 1`).
    pub kp: usize,
    /// Age class, 0-based; `None` when the particle is older than the last
    /// class boundary (upstream's `nage = nageclass + 1`).
    pub nage: Option<usize>,
    /// `initialize` is to be called (`itramem == itime` or `itime == 0`).
    pub initialize: bool,
    /// How many times `get_vdep_prob` was called (once per species still
    /// unset, `DRYBKDEP`).
    pub vdep_calls: usize,
    /// How many times `get_wetscav` was called (`WETBKDEP`).
    pub wetscav_calls: usize,
}

/// One species' `get_wetscav` result for this particle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WetScavResult {
    /// Scavenging coefficient, s⁻¹ (`wetscav`).
    pub wetscav: f64,
    /// `grfraction(1)`, the precipitating fraction of the grid cell.
    pub grfraction1: f64,
}

/// Lines 534-593, before `advance`. `itra1`/`itramem` and `npoint`
/// (1-based) are the particle's; `xmass1` and `xscav_frac1` its per-species
/// arrays (updated in place). `prob_rec` is what `get_vdep_prob` returns at
/// the particle ([`crate::flexpart::advance::get_vdep_prob`]), `wetscav` per
/// species what `get_wetscav` returns
/// ([`crate::flexpart::wet_deposition::get_wetscav`]); `zpoint1`/`zpoint2`
/// the release's bottom and top, m.
///
/// Backward receptor modes: a species whose `xscav_frac1` is still negative
/// gets its scavenged fraction (dry: `prob_rec`; wet: `wetscav * (zpoint2 -
/// zpoint1) * grfraction(1)`), or, with no dry deposition for it / no wet
/// scavenging, its mass set to 0.
#[allow(clippy::too_many_arguments)]
pub fn pre_advance(
    set: &PreAdvanceSettings,
    itime: i64,
    itra1: i64,
    itramem: i64,
    npoint: usize,
    xmass1: &mut [f64],
    xscav_frac1: &mut [f64],
    prob_rec: &[f64],
    wetscav: &[WetScavResult],
    zpoint1: f64,
    zpoint2: f64,
) -> PreAdvance {
    let kp = if set.output_each_release {
        npoint - 1
    } else {
        0
    };
    let itage = (itra1 - itramem).abs();
    let nage = set.lage.iter().position(|&l| itage < l);
    let mut out = PreAdvance {
        kp,
        nage,
        initialize: itramem == itime || itime == 0,
        vdep_calls: 0,
        wetscav_calls: 0,
    };
    let nspec = xmass1.len();
    if set.drybkdep {
        for ks in 0..nspec {
            if xscav_frac1[ks] < 0.0 {
                out.vdep_calls += 1;
                if set.drydepspec[ks] {
                    xscav_frac1[ks] = prob_rec[ks];
                } else {
                    xmass1[ks] = 0.0;
                    xscav_frac1[ks] = 0.0;
                }
            }
        }
    }
    if set.wetbkdep {
        for ks in 0..nspec {
            if xscav_frac1[ks] < 0.0 {
                out.wetscav_calls += 1;
                let w = wetscav[ks];
                if w.wetscav > 0.0 {
                    xscav_frac1[ks] = w.wetscav * (zpoint2 - zpoint1) * w.grfraction1;
                } else {
                    xmass1[ks] = 0.0;
                    xscav_frac1[ks] = 0.0;
                }
            }
        }
    }
    out
}

/// Settings [`post_advance`] reads.
#[derive(Debug, Clone, PartialEq)]
pub struct PostAdvanceSettings {
    /// `+1` forward, `-1` backward.
    pub ldirect: i64,
    /// Synchronisation interval, s (signed).
    pub lsynctime: i64,
    /// Decay constant per species, s⁻¹ (`decay`, 0 = stable).
    pub decay: Vec<f64>,
    /// Dry deposition switched on (`DRYDEP`).
    pub drydep: bool,
    /// Per-species dry deposition (`DRYDEPSPEC`).
    pub drydepspec: Vec<bool>,
    /// Domain-filling mode (`mdomainfill`, 0 = off).
    pub mdomainfill: i32,
    /// Quasi-Lagrangian mode (`mquasilag`, 0 = off).
    pub mquasilag: i32,
    /// Nested output on (`nested_output == 1`).
    pub nested_output: bool,
    /// Initial-condition output (`linit_cond`, backward runs).
    pub linit_cond: i32,
    /// The last age-class bound, s (`lage(nageclass)`).
    pub lage_max: i64,
}

/// The particle state [`post_advance`] reads and updates.
#[derive(Debug, Clone, PartialEq)]
pub struct StepParticle {
    /// Time, s (`itra1`).
    pub itra1: i64,
    /// Release time, s (`itramem`).
    pub itramem: i64,
    /// Uncertainty class, 1-based (`nclass`).
    pub nclass: usize,
    /// Position, grid units (`xtra1`).
    pub xtra1: f64,
    /// See [`StepParticle::xtra1`].
    pub ytra1: f64,
    /// Mass per species, kg (`xmass1`).
    pub xmass1: Vec<f64>,
}

/// One `drydepokernel` (or `drydepokernel_nest`) call.
#[derive(Debug, Clone, PartialEq)]
pub struct KernelCall {
    /// `drydepokernel_nest` rather than `drydepokernel`.
    pub nest: bool,
    /// Uncertainty class, 1-based (`nclass(j)`).
    pub nunc: usize,
    /// The whole `drydeposit` buffer at the call, kg per species.
    pub deposit: Vec<f64>,
    /// `real(xtra1(j))`.
    pub x: f64,
    /// `real(ytra1(j))`.
    pub y: f64,
    /// Age class as passed (1-based, `nage`).
    pub nage: usize,
    /// Release slot as passed (1-based, `kp`).
    pub kp: usize,
}

/// Why a particle stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termination {
    /// Still active; next due at `itime + lsynctime`.
    Active,
    /// `advance` returned `nstop > 1` (left the domain or the top).
    Stopped,
    /// Carries less than [`MINMASS`] of its initial mass.
    SmallMass,
    /// Reached `lage(nageclass)`.
    Age,
}

/// What [`post_advance`] did.
#[derive(Debug, Clone, PartialEq)]
pub struct PostAdvance {
    /// The termination decided last (`Age` overrides `SmallMass`, as the
    /// age test runs after it).
    pub termination: Termination,
    /// `xmassfract`, `None` when `nstop > 1` (not computed).
    pub xmassfract: Option<f64>,
    /// Which species had `drydeposit` assigned this call.
    pub drydeposit_assigned: Vec<bool>,
    /// `drydepokernel` calls, in order.
    pub kernel_calls: Vec<KernelCall>,
    /// `initial_cond_calc` calls (their time argument), when `linit_cond >=
    /// 1`.
    pub initial_cond_calls: Vec<i64>,
}

/// Lines 625-703, after `advance`.
///
/// `nstop` and `prob` (per species dry-deposition probability) come from
/// `advance`; `ldeltat` from [`ClockEvents::ldeltat`]; `nage` and `kp`
/// (1-based) from [`pre_advance`]; `xmass_release` is `xmass(npoint(j),
/// :)` and `npart` the release's particle count. `drydeposit` is
/// timemanager's per-species buffer, kept by the caller between particles
/// (see the module doc).
///
/// Per species: `decfact = exp(-|lsynctime| decay)` for `decay > 0`; with
/// dry deposition `drydeposit = xmass1*prob*decfact`, `xmass1 =
/// xmass1*(1-prob)*decfact`, and for a decaying species the deposit is
/// multiplied by `exp(|ldeltat| decay)` (back-dated to the last decay of the
/// deposition grids, which [`decay_deposition_grid`] then applies in full).
#[allow(clippy::too_many_arguments)]
pub fn post_advance(
    set: &PostAdvanceSettings,
    p: &mut StepParticle,
    itime: i64,
    nstop: i32,
    prob: &[f64],
    ldeltat: i64,
    nage: usize,
    kp: usize,
    xmass_release: &[f64],
    npart: i64,
    drydeposit: &mut [f64],
) -> PostAdvance {
    let nspec = p.xmass1.len();
    let mut out = PostAdvance {
        termination: Termination::Active,
        xmassfract: None,
        drydeposit_assigned: vec![false; nspec],
        kernel_calls: Vec::new(),
        initial_cond_calls: Vec::new(),
    };
    if nstop > 1 {
        if set.linit_cond >= 1 {
            out.initial_cond_calls.push(itime);
        }
        p.itra1 = TERMINATED;
        out.termination = Termination::Stopped;
        return out;
    }
    p.itra1 = itime + set.lsynctime;

    let mut xmassfract = 0.0_f64;
    for ks in 0..nspec {
        let decfact = if set.decay[ks] > 0.0 {
            surviving_fraction(set.decay[ks], set.lsynctime.abs() as f64)
        } else {
            1.0
        };
        if set.drydepspec[ks] {
            drydeposit[ks] = p.xmass1[ks] * prob[ks] * decfact;
            p.xmass1[ks] = p.xmass1[ks] * (1.0 - prob[ks]) * decfact;
            out.drydeposit_assigned[ks] = true;
            if set.decay[ks] > 0.0 {
                // exp(real(abs(ldeltat))*decay(ks))
                drydeposit[ks] *= (ldeltat.abs() as f64 * set.decay[ks]).exp();
            }
        } else {
            p.xmass1[ks] *= decfact;
        }
        if set.mdomainfill == 0 && set.mquasilag == 0 {
            if xmass_release[ks] > 0.0 {
                xmassfract = xmassfract.max(npart as f64 * p.xmass1[ks] / xmass_release[ks]);
            }
        } else {
            xmassfract = 1.0;
        }
    }
    out.xmassfract = Some(xmassfract);
    if xmassfract < MINMASS {
        p.itra1 = TERMINATED;
        out.termination = Termination::SmallMass;
    }
    if set.drydep && set.ldirect == 1 {
        let call = |nest: bool| KernelCall {
            nest,
            nunc: p.nclass,
            deposit: drydeposit.to_vec(),
            x: p.xtra1,
            y: p.ytra1,
            nage,
            kp,
        };
        out.kernel_calls.push(call(false));
        if set.nested_output {
            out.kernel_calls.push(call(true));
        }
    }
    if (p.itra1 - p.itramem).abs() >= set.lage_max {
        if set.linit_cond >= 1 {
            out.initial_cond_calls.push(itime + set.lsynctime);
        }
        p.itra1 = TERMINATED;
        out.termination = Termination::Age;
    }
    out
}
