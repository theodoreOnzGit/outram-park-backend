// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. OUTRAM PARK is free software: you can
// redistribute it and/or modify it under the terms of the GNU General Public
// License as published by the Free Software Foundation, either version 3 of
// the License, or (at your option) any later version. It is distributed
// WITHOUT ANY WARRANTY; see <https://www.gnu.org/licenses/>.

//! `compute ke`, `compute erotate/sphere` and `fix ave/time` (gh:#1418).
//!
//! Translated from LIGGGHTS-PUBLIC `3d5c00f2`: `src/compute_ke.cpp`
//! (`ComputeKE::compute_scalar`), `src/compute_erotate_sphere.cpp`
//! (`ComputeERotateSphere::compute_scalar`) and `src/fix_ave_time.cpp`
//! (`FixAveTime::{nextvalid, invoke_scalar}`).
//!
//! ## Scope, stated plainly
//!
//! * `compute ke` / `erotate/sphere` for the group of all particles, in SI
//!   units (`mvv2e = 1`), end-of-step (`halfstep = 0`); the multisphere branch
//!   is not ported.
//! * `fix ave/time` in `mode scalar` only, with `ave one`, `running` and
//!   `window N`. NOT ported: `mode vector`, `off`, `start`, `overwrite`, file
//!   output (upstream writes `%g`; the value, not the text, is what is
//!   compared).

use crate::particle::Particle;

/// `INERTIA` of a solid sphere in LIGGGHTS (`math_const.h`).
const INERTIA: f64 = 0.4;

/// `compute ke` of the group of all particles: `0.5 * Σ m (vx² + vy² + vz²)`,
/// summed in particle order, scaled once at the end like upstream.
#[must_use]
pub fn compute_ke(particles: &[Particle]) -> f64 {
    let mut ke = 0.0;
    for p in particles {
        let v = p.velocity;
        ke += p.mass * (v.x * v.x + v.y * v.y + v.z * v.z);
    }
    ke * (0.5 * 1.0)
}

/// `compute erotate/sphere`: `0.5 * 0.4 * Σ (ωx² + ωy² + ωz²) r² m`.
#[must_use]
pub fn compute_erotate_sphere(particles: &[Particle]) -> f64 {
    let mut e = 0.0;
    for p in particles {
        let w = p.angular_velocity;
        e += (w.x * w.x + w.y * w.y + w.z * w.z) * p.radius * p.radius * p.mass;
    }
    e * (0.5 * 1.0 * INERTIA)
}

/// The `ave` keyword of `fix ave/time`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AveMode {
    /// `ave one`: only the latest `Nfreq` average.
    One,
    /// `ave running`: all `Nfreq` averages so far.
    Running,
    /// `ave window N`: the last `N` `Nfreq` averages.
    Window(usize),
}

/// `fix ave/time Nevery Nrepeat Nfreq <scalars> ave <mode>` (scalar mode).
#[derive(Debug, Clone, PartialEq)]
pub struct AveTime {
    nevery: i64,
    nrepeat: i64,
    nfreq: i64,
    mode: AveMode,
    nvalid: i64,
    irepeat: i64,
    vector: Vec<f64>,
    vector_total: Vec<f64>,
    norm: i64,
    window_list: Vec<Vec<f64>>,
    iwindow: usize,
    window_limit: bool,
}

impl AveTime {
    /// Create the fix at timestep `step` with `nvalues` columns
    /// (`FixAveTime::FixAveTime` + `nextvalid`, `startstep = 0`).
    ///
    /// # Errors
    /// The upstream argument checks: all of `nevery`, `nrepeat`, `nfreq`
    /// positive, `nfreq % nevery == 0` and `(nrepeat-1)*nevery < nfreq`.
    pub fn new(
        nevery: i64,
        nrepeat: i64,
        nfreq: i64,
        nvalues: usize,
        mode: AveMode,
        step: i64,
    ) -> Result<Self, String> {
        if nevery <= 0 || nrepeat <= 0 || nfreq <= 0 {
            return Err("Illegal fix ave/time command".into());
        }
        if nfreq % nevery != 0 || (nrepeat - 1) * nevery >= nfreq {
            return Err("Illegal fix ave/time command".into());
        }
        if mode == AveMode::Window(0) {
            return Err("Illegal fix ave/time command".into());
        }
        let nwin = if let AveMode::Window(n) = mode { n } else { 0 };
        let mut s = Self {
            nevery,
            nrepeat,
            nfreq,
            mode,
            nvalid: 0,
            irepeat: 0,
            vector: vec![0.0; nvalues],
            vector_total: vec![0.0; nvalues],
            norm: 0,
            window_list: vec![vec![0.0; nvalues]; nwin],
            iwindow: 0,
            window_limit: false,
        };
        s.nvalid = s.nextvalid(step);
        Ok(s)
    }

    /// `FixAveTime::nextvalid` with `startstep = 0`.
    fn nextvalid(&self, step: i64) -> i64 {
        let mut nvalid = (step / self.nfreq) * self.nfreq + self.nfreq;
        if nvalid - self.nfreq == step && self.nrepeat == 1 {
            nvalid = step;
        } else {
            nvalid -= (self.nrepeat - 1) * self.nevery;
        }
        if nvalid < step {
            nvalid += self.nfreq;
        }
        nvalid
    }

    /// `FixAveTime::end_of_step` + `invoke_scalar`: call at the end of every
    /// step with the current values of the columns. Does nothing unless
    /// `step` is the next valid step.
    pub fn end_of_step(&mut self, step: i64, values: &[f64]) {
        if step != self.nvalid {
            return;
        }
        if self.irepeat == 0 {
            self.vector.iter_mut().for_each(|v| *v = 0.0);
        }
        for (v, x) in self.vector.iter_mut().zip(values) {
            *v += *x;
        }
        self.irepeat += 1;
        if self.irepeat < self.nrepeat {
            self.nvalid += self.nevery;
            return;
        }
        self.irepeat = 0;
        self.nvalid = step + self.nfreq - (self.nrepeat - 1) * self.nevery;
        let repeat = self.nrepeat as f64;
        for v in &mut self.vector {
            *v /= repeat;
        }
        match self.mode {
            AveMode::One => {
                self.vector_total.clone_from(&self.vector);
                self.norm = 1;
            }
            AveMode::Running => {
                for (t, v) in self.vector_total.iter_mut().zip(&self.vector) {
                    *t += *v;
                }
                self.norm += 1;
            }
            AveMode::Window(nwindow) => {
                for i in 0..self.vector.len() {
                    self.vector_total[i] += self.vector[i];
                    if self.window_limit {
                        self.vector_total[i] -= self.window_list[self.iwindow][i];
                    }
                    self.window_list[self.iwindow][i] = self.vector[i];
                }
                self.iwindow += 1;
                if self.iwindow == nwindow {
                    self.iwindow = 0;
                    self.window_limit = true;
                }
                self.norm = if self.window_limit { nwindow as i64 } else { self.iwindow as i64 };
            }
        }
    }

    /// `compute_vector(i)`: the averaged column `i`, `0` before the first
    /// output (upstream returns `0` while `norm == 0`).
    #[must_use]
    pub fn value(&self, i: usize) -> f64 {
        if self.norm != 0 {
            self.vector_total[i] / self.norm as f64
        } else {
            0.0
        }
    }

    /// `compute_scalar()`: column 0.
    #[must_use]
    pub fn scalar(&self) -> f64 {
        self.value(0)
    }
}

/// `fix ave/euler nevery N cell_size_relative C parallel no` on a serial,
/// fixed orthogonal box: Eulerian cell averages of volume fraction, mass-
/// averaged velocity and number-averaged radius.
///
/// Translated from `FixAveEuler::{setup_bins, bin_atoms, coord2bin,
/// calculate_eu}` (LIGGGHTS-PUBLIC `3d5c00f2`, `fix_ave_euler.cpp`; gh:#1418).
///
/// **Not ported (stated):** the per-cell stress columns (they need
/// `compute stress/atom pair`, the per-atom pair virial, which
/// [`crate::granular_system::GranularSystem`] does not expose),
/// `basevolume_region` (Monte-Carlo weights; every cell has weight 1 here),
/// `parallel yes`, triclinic boxes and the multisphere/superquadric branches.
/// Reproduced: centre (cols 1-3), volume fraction (4), mass-averaged velocity
/// (5-7) and the number-averaged radius.
///
/// Upstream defect, recorded: `FixAveEuler` never sets `global_freq` (left at 0
/// by `Fix`), so reading `f_ID[i][j]` from `thermo` divides by zero
/// (`Thermo::init`: `thermo_every % global_freq`) and `lmp_serial` dies with a
/// floating-point exception. The reference binary used for the comparison is
/// relinked with the one-line `global_freq = nevery;` (see
/// `reference-data/liggghts/make_aveeuler_deck.sh`); the averaging is untouched.
/// The port does not copy the crash.
#[derive(Debug, Clone, PartialEq)]
pub struct AveEuler {
    nevery: i64,
    lo: [f64; 3],
    hi: [f64; 3],
    ncells_dim: [usize; 3],
    cell_size_inv: [f64; 3],
    cell_volume: f64,
    center: Vec<[f64; 3]>,
    v_av: Vec<[f64; 3]>,
    vol_fr: Vec<f64>,
    radius: Vec<f64>,
    ncount: Vec<i64>,
    mass: Vec<f64>,
}

impl AveEuler {
    /// `setup_bins`. `cut_minus_skin` is `neighbor->cutneighmax - neighbor->skin`
    /// (for the granular pair style the largest contact distance, `2 r_max`).
    ///
    /// # Errors
    /// `nevery < 1` or `cell_size_relative < 1` (upstream's checks).
    pub fn new(nevery: i64, cell_size_relative: f64, cut_minus_skin: f64, lo: [f64; 3], hi: [f64; 3]) -> Result<Self, String> {
        if nevery < 1 {
            return Err("'nevery' > 0 required".into());
        }
        if cell_size_relative < 1.0 {
            return Err("'cell_size_relative' > 1 required".into());
        }
        let ideal = cell_size_relative * cut_minus_skin;
        let mut dim = [1usize; 3];
        let mut cell_size = [0.0; 3];
        let mut inv = [0.0; 3];
        for d in 0..3 {
            let n = ((hi[d] - lo[d]) / ideal) as i64;
            dim[d] = if n < 1 { 1 } else { n as usize };
            cell_size[d] = (hi[d] - lo[d]) / dim[d] as f64;
            inv[d] = 1. / cell_size[d];
        }
        let ncells = dim[0] * dim[1] * dim[2];
        let mut center = vec![[0.0; 3]; ncells];
        for ix in 0..dim[0] {
            for iy in 0..dim[1] {
                for iz in 0..dim[2] {
                    let b = iz * dim[1] * dim[0] + iy * dim[0] + ix;
                    center[b] = [
                        lo[0] + (ix as f64 + 0.5) * cell_size[0],
                        lo[1] + (iy as f64 + 0.5) * cell_size[1],
                        lo[2] + (iz as f64 + 0.5) * cell_size[2],
                    ];
                }
            }
        }
        Ok(Self {
            nevery,
            lo,
            hi,
            ncells_dim: dim,
            cell_size_inv: inv,
            cell_volume: cell_size[0] * cell_size[1] * cell_size[2],
            center,
            v_av: vec![[0.0; 3]; ncells],
            vol_fr: vec![0.0; ncells],
            radius: vec![0.0; ncells],
            ncount: vec![0; ncells],
            mass: vec![0.0; ncells],
        })
    }

    /// Number of cells.
    #[must_use]
    pub fn ncells(&self) -> usize {
        self.center.len()
    }

    /// `coord2bin` (serial): `None` for a particle on or outside the box.
    fn coord2bin(&self, x: [f64; 3]) -> Option<usize> {
        let mut c = [0usize; 3];
        for d in 0..3 {
            if x[d] <= self.lo[d] || x[d] >= self.hi[d] {
                return None;
            }
            c[d] = ((x[d] - self.lo[d]) * self.cell_size_inv[d]) as usize;
        }
        Some(c[2] * self.ncells_dim[1] * self.ncells_dim[0] + c[1] * self.ncells_dim[0] + c[0])
    }

    /// `end_of_step`: bin and average. Does nothing unless `step % nevery == 0`.
    /// `particles` must be in LIGGGHTS' local atom order
    /// ([`crate::granular_system::GranularSystem::particles_in_local_order`]),
    /// the order upstream's cell linked lists are walked in.
    pub fn end_of_step(&mut self, step: i64, particles: &[Particle]) {
        if step % self.nevery != 0 {
            return;
        }
        let prefactor_vol_fr = 4. / 3. * std::f64::consts::PI / self.cell_volume;
        for c in 0..self.ncells() {
            self.ncount[c] = 0;
            self.v_av[c] = [0.0; 3];
            self.vol_fr[c] = 0.0;
            self.radius[c] = 0.0;
            self.mass[c] = 0.0;
        }
        for p in particles {
            let x = [p.position.x, p.position.y, p.position.z];
            let Some(c) = self.coord2bin(x) else { continue };
            let v = [p.velocity.x, p.velocity.y, p.velocity.z];
            for d in 0..3 {
                self.v_av[c][d] += v[d] * p.mass;
            }
            let r = p.radius;
            self.vol_fr[c] += r * r * r;
            self.radius[c] += r;
            self.mass[c] += p.mass;
            self.ncount[c] += 1;
        }
        for c in 0..self.ncells() {
            if self.ncount[c] != 0 {
                let sinv = 1. / self.mass[c];
                for d in 0..3 {
                    self.v_av[c][d] *= sinv;
                }
                self.radius[c] /= self.ncount[c] as f64;
            }
            // weight_ == 1 everywhere (no basevolume_region); eps_ntry = 1/50.
            let weight = 1.0;
            let eps_ntry = 1. / 50.;
            if weight < eps_ntry {
                self.vol_fr[c] = 0.0;
            } else {
                self.vol_fr[c] *= prefactor_vol_fr / weight;
            }
        }
    }

    /// `compute_array(i, j)` for `j` in `0..7`: centre, volume fraction,
    /// mass-averaged velocity. Rows past the grid return `0`, like upstream.
    #[must_use]
    pub fn array(&self, i: usize, j: usize) -> f64 {
        if i >= self.ncells() {
            0.0
        } else if j < 3 {
            self.center[i][j]
        } else if j == 3 {
            self.vol_fr[i]
        } else if j < 7 {
            self.v_av[i][j - 4]
        } else {
            0.0
        }
    }

    /// Number-averaged radius of cell `i` (upstream column 15).
    #[must_use]
    pub fn radius(&self, i: usize) -> f64 {
        self.radius[i]
    }
}

/// `fix ave/atom Nevery Nrepeat Nfreq <columns>`: the per-atom time average of
/// per-atom columns (atom attributes `x y z vx vy vz fx fy fz`, or the
/// `vector_atom`/`array_atom` of a compute/fix/atom-style variable, which the
/// caller supplies as a column slice each step).
///
/// Translated from `FixAveAtom::end_of_step` and `nextvalid` (LIGGGHTS-PUBLIC
/// `3d5c00f2`, `fix_ave_atom.cpp`; gh:#1418): zero at `irepeat == 0`, add the
/// columns on each valid step, divide by `Nrepeat` on the last. As upstream's
/// `array`, it is only rewritten on a valid step, so between outputs it holds
/// mid-accumulation state; read it on an output step.
#[derive(Debug, Clone, PartialEq)]
pub struct AveAtom {
    nevery: i64,
    nrepeat: i64,
    nfreq: i64,
    nvalid: i64,
    irepeat: i64,
    nvalues: usize,
    array: Vec<Vec<f64>>,
}

impl AveAtom {
    /// Create the fix at timestep `step` for `natoms` atoms and `nvalues` columns.
    ///
    /// # Errors
    /// The upstream argument checks (`nevery`, `nrepeat`, `nfreq` positive,
    /// `nfreq % nevery == 0`, `(nrepeat-1)*nevery < nfreq`).
    pub fn new(nevery: i64, nrepeat: i64, nfreq: i64, natoms: usize, nvalues: usize, step: i64) -> Result<Self, String> {
        if nevery <= 0 || nrepeat <= 0 || nfreq <= 0 || nfreq % nevery != 0 || (nrepeat - 1) * nevery >= nfreq {
            return Err("Illegal fix ave/atom command".into());
        }
        let mut s = Self {
            nevery,
            nrepeat,
            nfreq,
            nvalid: 0,
            irepeat: 0,
            nvalues,
            array: vec![vec![0.0; nvalues]; natoms],
        };
        s.nvalid = s.nextvalid(step);
        Ok(s)
    }

    /// `FixAveAtom::nextvalid`.
    fn nextvalid(&self, step: i64) -> i64 {
        let mut nvalid = (step / self.nfreq) * self.nfreq + self.nfreq;
        if nvalid - self.nfreq == step && self.nrepeat == 1 {
            nvalid = step;
        } else {
            nvalid -= (self.nrepeat - 1) * self.nevery;
        }
        if nvalid < step {
            nvalid += self.nfreq;
        }
        nvalid
    }

    /// Call at the end of every step. `columns[m][i]` is column `m` of atom `i`.
    /// Does nothing unless `step` is the next valid step.
    pub fn end_of_step(&mut self, step: i64, columns: &[&[f64]]) {
        if step != self.nvalid {
            return;
        }
        if self.irepeat == 0 {
            for row in &mut self.array {
                row.iter_mut().for_each(|v| *v = 0.0);
            }
        }
        for (m, col) in columns.iter().enumerate().take(self.nvalues) {
            for (row, x) in self.array.iter_mut().zip(col.iter()) {
                row[m] += *x;
            }
        }
        self.irepeat += 1;
        if self.irepeat < self.nrepeat {
            self.nvalid += self.nevery;
            return;
        }
        self.irepeat = 0;
        self.nvalid = step + self.nfreq - (self.nrepeat - 1) * self.nevery;
        let repeat = self.nrepeat as f64;
        for row in &mut self.array {
            for v in row.iter_mut() {
                *v /= repeat;
            }
        }
    }

    /// Column `m` of atom `i` (`array_atom[i][m]`).
    #[must_use]
    pub fn value(&self, i: usize, m: usize) -> f64 {
        self.array[i][m]
    }
}

/// `origin` argument of one `fix ave/spatial` dimension (`lower|center|upper|<coord>`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpatialOrigin {
    /// `lower`: the box lower bound along the dimension.
    Lower,
    /// `center`: the box centre.
    Center,
    /// `upper`: the box upper bound.
    Upper,
    /// An explicit coordinate (`units box`).
    Coord(f64),
}

/// One binned dimension of `fix ave/spatial`: `x|y|z origin delta`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialDim {
    /// 0, 1, 2 for x, y, z.
    pub axis: usize,
    /// Bin origin.
    pub origin: SpatialOrigin,
    /// Bin width, SI metres (`units box`).
    pub delta: f64,
}

/// A per-atom value column of `fix ave/spatial`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialValue {
    /// `vx|vy|vz` (axis 0..3).
    V(usize),
    /// `fx|fy|fz` (axis 0..3).
    F(usize),
    /// `density/number`.
    DensityNumber,
    /// `density/mass`.
    DensityMass,
    /// `c_ID`, `f_ID`, `v_name`: index into the caller's per-atom `columns`.
    Column(usize),
}

/// The `norm` keyword of `fix ave/spatial`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialNorm {
    /// `norm all`: total value / total count.
    All,
    /// `norm sample`: mean of per-sample means (densities un-normalised per sample).
    Sample,
}

/// The simulation box seen by `fix ave/spatial` (`units box`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpatialBox {
    /// `domain->boxlo`.
    pub lo: [f64; 3],
    /// `domain->boxhi`.
    pub hi: [f64; 3],
    /// `domain->periodicity`.
    pub periodic: [bool; 3],
}

/// `fix ave/spatial Nevery Nrepeat Nfreq <1-3 dims> <values> norm .. ave ..` (`units box`).
///
/// Translated from `FixAveSpatial` (LIGGGHTS-PUBLIC `3d5c00f2`,
/// `fix_ave_spatial.cpp`; gh:#1418): `setup_bins` (origin, `lo`/`hi` bin edges,
/// `nlayers`, `bin_volume`), `atom2bin1d/2d/3d` (PBC remap, clamp, including the
/// `region` branch's `nlayer-1` clamp quirk in 2-d), `end_of_step` (both
/// `norm` branches, `ave one/running/window`, density normalisation by repeat
/// and bin volume with `mv2d = 1`) and `compute_array`. Not translated:
/// `units lattice|reduced`, file output, `std`. Per-atom sums use the caller's
/// array order, which must be upstream's local order.
#[derive(Debug, Clone, PartialEq)]
pub struct AveSpatial {
    nevery: i64,
    nrepeat: i64,
    nfreq: i64,
    nvalid: i64,
    irepeat: i64,
    dims: Vec<SpatialDim>,
    values: Vec<SpatialValue>,
    norm_flag: SpatialNorm,
    ave: AveMode,
    bx: SpatialBox,
    offset: [f64; 3],
    invdelta: [f64; 3],
    delta: [f64; 3],
    nlayers: [i32; 3],
    bin_volume: f64,
    nbins: usize,
    coord: Vec<[f64; 3]>,
    count_many: Vec<f64>,
    count_sum: Vec<f64>,
    count_total: Vec<f64>,
    values_many: Vec<Vec<f64>>,
    values_sum: Vec<Vec<f64>>,
    values_total: Vec<Vec<f64>>,
    count_list: Vec<Vec<f64>>,
    values_list: Vec<Vec<Vec<f64>>>,
    iwindow: usize,
    window_limit: bool,
    norm: i64,
    region: bool,
}

impl AveSpatial {
    /// Create the fix at timestep `step` (the constructor plus `setup_bins`).
    ///
    /// # Errors
    /// The upstream argument checks (`nevery`, `nrepeat`, `nfreq`, `delta`
    /// positive, `nfreq % nevery == 0`, `(nrepeat-1)*nevery < nfreq`, 1-3
    /// distinct dimensions, `window` size positive).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        nevery: i64,
        nrepeat: i64,
        nfreq: i64,
        dims: Vec<SpatialDim>,
        values: Vec<SpatialValue>,
        norm_flag: SpatialNorm,
        ave: AveMode,
        bx: SpatialBox,
        step: i64,
    ) -> Result<Self, String> {
        let bad = || "Illegal fix ave/spatial command".to_string();
        let nd = dims.len();
        if nd == 0 || nd > 3 || nevery <= 0 || nrepeat <= 0 || nfreq <= 0 || nfreq % nevery != 0 || (nrepeat - 1) * nevery >= nfreq {
            return Err(bad());
        }
        for (a, d) in dims.iter().enumerate() {
            if d.axis > 2 || d.delta <= 0.0 || dims[..a].iter().any(|e| e.axis == d.axis) {
                return Err(bad());
            }
        }
        if let AveMode::Window(0) = ave {
            return Err(bad());
        }
        let mut s = Self {
            nevery,
            nrepeat,
            nfreq,
            nvalid: 0,
            irepeat: 0,
            dims,
            values,
            norm_flag,
            ave,
            bx,
            offset: [0.0; 3],
            invdelta: [0.0; 3],
            delta: [0.0; 3],
            nlayers: [1; 3],
            bin_volume: 0.0,
            nbins: 0,
            coord: Vec::new(),
            count_many: Vec::new(),
            count_sum: Vec::new(),
            count_total: Vec::new(),
            values_many: Vec::new(),
            values_sum: Vec::new(),
            values_total: Vec::new(),
            count_list: Vec::new(),
            values_list: Vec::new(),
            iwindow: 0,
            window_limit: false,
            norm: 0,
            region: false,
        };
        for (m, d) in s.dims.iter().enumerate() {
            s.delta[m] = d.delta;
            s.invdelta[m] = 1.0 / d.delta;
        }
        s.setup_bins();
        s.nvalid = s.nextvalid(step);
        Ok(s)
    }

    /// Mark the fix as having a `region` keyword: the 2-d bin clamp then
    /// follows upstream's region branch (`nlayer - 1 - 1`).
    #[must_use]
    pub fn with_region(mut self) -> Self {
        self.region = true;
        self
    }

    /// `FixAveSpatial::setup_bins`.
    fn setup_bins(&mut self) {
        let nd = self.dims.len();
        let (boxlo, boxhi) = (self.bx.lo, self.bx.hi);
        let prd = [boxhi[0] - boxlo[0], boxhi[1] - boxlo[1], boxhi[2] - boxlo[2]];
        self.bin_volume = prd[0] * prd[1] * prd[2];
        let mut nbins: usize = 1;
        for m in 0..nd {
            let d = self.dims[m];
            let a = d.axis;
            let origin = match d.origin {
                SpatialOrigin::Lower => boxlo[a],
                SpatialOrigin::Upper => boxhi[a],
                SpatialOrigin::Center => 0.5 * (boxlo[a] + boxhi[a]),
                SpatialOrigin::Coord(c) => c,
            };
            let delta = self.delta[m];
            let lo;
            if origin < boxlo[a] {
                let n = ((boxlo[a] - origin) * self.invdelta[m]) as i32;
                lo = origin + f64::from(n) * delta;
            } else {
                let n = ((origin - boxlo[a]) * self.invdelta[m]) as i32;
                let mut l = origin - f64::from(n) * delta;
                if l > boxlo[a] {
                    l -= delta;
                }
                lo = l;
            }
            let hi;
            if origin < boxhi[a] {
                let n = ((boxhi[a] - origin) * self.invdelta[m]) as i32;
                let mut h = origin + f64::from(n) * delta;
                if h < boxhi[a] {
                    h += delta;
                }
                hi = h;
            } else {
                let n = ((origin - boxhi[a]) * self.invdelta[m]) as i32;
                hi = origin - f64::from(n) * delta;
            }
            self.offset[m] = lo;
            self.nlayers[m] = ((hi - lo) * self.invdelta[m] + 0.5) as i32;
            nbins *= self.nlayers[m].max(0) as usize;
            self.bin_volume *= delta / prd[a];
        }
        self.nbins = nbins;
        let nv = self.values.len();
        self.count_many = vec![0.0; nbins];
        self.count_sum = vec![0.0; nbins];
        self.count_total = vec![0.0; nbins];
        self.values_many = vec![vec![0.0; nv]; nbins];
        self.values_sum = vec![vec![0.0; nv]; nbins];
        self.values_total = vec![vec![0.0; nv]; nbins];
        if let AveMode::Window(nw) = self.ave {
            self.count_list = vec![vec![0.0; nbins]; nw];
            self.values_list = vec![vec![vec![0.0; nv]; nbins]; nw];
        }
        let c = |i: i32, m: usize| self.offset[m] + (f64::from(i) + 0.5) * self.delta[m];
        let mut coord = Vec::with_capacity(nbins);
        match nd {
            1 => {
                for i in 0..self.nlayers[0] {
                    coord.push([c(i, 0), 0.0, 0.0]);
                }
            }
            2 => {
                for i in 0..self.nlayers[0] {
                    let c1 = c(i, 0);
                    for j in 0..self.nlayers[1] {
                        coord.push([c1, c(j, 1), 0.0]);
                    }
                }
            }
            _ => {
                for i in 0..self.nlayers[0] {
                    let c1 = c(i, 0);
                    for j in 0..self.nlayers[1] {
                        let c2 = c(j, 1);
                        for k in 0..self.nlayers[2] {
                            coord.push([c1, c2, c(k, 2)]);
                        }
                    }
                }
            }
        }
        self.coord = coord;
    }

    /// `FixAveSpatial::nextvalid`.
    fn nextvalid(&self, step: i64) -> i64 {
        let mut nvalid = (step / self.nfreq) * self.nfreq + self.nfreq;
        if nvalid - self.nfreq == step && self.nrepeat == 1 {
            nvalid = step;
        } else {
            nvalid -= (self.nrepeat - 1) * self.nevery;
        }
        if nvalid < step {
            nvalid += self.nfreq;
        }
        nvalid
    }

    /// Number of bins.
    #[must_use]
    pub fn nbins(&self) -> usize {
        self.nbins
    }

    /// Bin volume used for the density normalisation (`bin_volume`).
    #[must_use]
    pub fn bin_volume(&self) -> f64 {
        self.bin_volume
    }

    /// `atom2bin1d/2d/3d` for one atom (PBC remap, truncate, clamp).
    fn bin_of(&self, x: &[f64; 3]) -> usize {
        let nd = self.dims.len();
        let mut ibin = 0usize;
        for m in 0..nd {
            let a = self.dims[m].axis;
            let mut r = x[a];
            if self.bx.periodic[a] {
                let prd = self.bx.hi[a] - self.bx.lo[a];
                if r < self.bx.lo[a] {
                    r += prd;
                }
                if r >= self.bx.hi[a] {
                    r -= prd;
                }
            }
            let mut i = ((r - self.offset[m]) * self.invdelta[m]) as i32;
            i = i.max(0);
            // upstream's 2-d region branch clamps to nlayer - 2 (a quirk kept for parity)
            let top = if self.region && nd == 2 { self.nlayers[m] - 2 } else { self.nlayers[m] - 1 };
            i = i.min(top).max(0);
            ibin = ibin * self.nlayers[m] as usize + i as usize;
        }
        ibin
    }

    /// `FixAveSpatial::end_of_step`. Call at the end of every step (and at
    /// setup of a run); does nothing unless `step` is the next valid step.
    /// `x`, `v`, `f`, `rmass`, `columns[c][i]` are per atom in upstream's local
    /// order; `region[i]` is the `region->match` result when the fix has one.
    #[allow(clippy::too_many_arguments)]
    pub fn end_of_step(
        &mut self,
        step: i64,
        x: &[[f64; 3]],
        v: &[[f64; 3]],
        f: &[[f64; 3]],
        rmass: &[f64],
        columns: &[&[f64]],
        region: Option<&[bool]>,
    ) {
        if step != self.nvalid {
            return;
        }
        let (nbins, nv) = (self.nbins, self.values.len());
        if self.irepeat == 0 {
            for m in 0..nbins {
                self.count_many[m] = 0.0;
                self.count_sum[m] = 0.0;
                self.values_many[m].iter_mut().for_each(|e| *e = 0.0);
            }
        }
        let mut count_one = vec![0.0; nbins];
        let mut values_one = vec![vec![0.0; nv]; nbins];
        let bin: Vec<Option<usize>> = x
            .iter()
            .enumerate()
            .map(|(i, xi)| {
                if region.is_none_or(|r| r[i]) {
                    let b = self.bin_of(xi);
                    count_one[b] += 1.0;
                    Some(b)
                } else {
                    None
                }
            })
            .collect();
        for (m, val) in self.values.iter().enumerate() {
            for (i, b) in bin.iter().enumerate() {
                let Some(b) = *b else { continue };
                values_one[b][m] += match *val {
                    SpatialValue::V(j) => v[i][j],
                    SpatialValue::F(j) => f[i][j],
                    SpatialValue::DensityNumber => 1.0,
                    SpatialValue::DensityMass => rmass[i],
                    SpatialValue::Column(c) => columns[c][i],
                };
            }
        }
        let is_density = |m: usize| matches!(self.values[m], SpatialValue::DensityNumber | SpatialValue::DensityMass);
        if self.norm_flag == SpatialNorm::All {
            for m in 0..nbins {
                self.count_many[m] += count_one[m];
                for j in 0..nv {
                    self.values_many[m][j] += values_one[m][j];
                }
            }
        } else {
            self.count_many.copy_from_slice(&count_one);
            for m in 0..nbins {
                if self.count_many[m] > 0.0 {
                    for j in 0..nv {
                        if is_density(j) {
                            self.values_many[m][j] += values_one[m][j];
                        } else {
                            self.values_many[m][j] += values_one[m][j] / self.count_many[m];
                        }
                    }
                }
                self.count_sum[m] += self.count_many[m];
            }
        }
        self.irepeat += 1;
        if self.irepeat < self.nrepeat {
            self.nvalid += self.nevery;
            return;
        }
        self.irepeat = 0;
        self.nvalid = step + self.nfreq - (self.nrepeat - 1) * self.nevery;
        let repeat = self.nrepeat as f64;
        let mv2d = 1.0_f64;
        if self.norm_flag == SpatialNorm::All {
            self.count_sum.copy_from_slice(&self.count_many);
            for m in 0..nbins {
                self.values_sum[m].copy_from_slice(&self.values_many[m]);
                if self.count_sum[m] > 0.0 {
                    for j in 0..nv {
                        match self.values[j] {
                            SpatialValue::DensityNumber => self.values_sum[m][j] /= repeat,
                            SpatialValue::DensityMass => self.values_sum[m][j] *= mv2d / repeat,
                            _ => self.values_sum[m][j] /= self.count_sum[m],
                        }
                    }
                }
                self.count_sum[m] /= repeat;
            }
        } else {
            for m in 0..nbins {
                self.values_sum[m].copy_from_slice(&self.values_many[m]);
                for j in 0..nv {
                    self.values_sum[m][j] /= repeat;
                }
                self.count_sum[m] /= repeat;
            }
        }
        for j in 0..nv {
            if is_density(j) {
                for m in 0..nbins {
                    self.values_sum[m][j] /= self.bin_volume;
                }
            }
        }
        match self.ave {
            AveMode::One => {
                for m in 0..nbins {
                    self.values_total[m].copy_from_slice(&self.values_sum[m]);
                    self.count_total[m] = self.count_sum[m];
                }
                self.norm = 1;
            }
            AveMode::Running => {
                for m in 0..nbins {
                    for i in 0..nv {
                        self.values_total[m][i] += self.values_sum[m][i];
                    }
                    self.count_total[m] += self.count_sum[m];
                }
                self.norm += 1;
            }
            AveMode::Window(nwindow) => {
                for m in 0..nbins {
                    for i in 0..nv {
                        self.values_total[m][i] += self.values_sum[m][i];
                        if self.window_limit {
                            self.values_total[m][i] -= self.values_list[self.iwindow][m][i];
                        }
                        self.values_list[self.iwindow][m][i] = self.values_sum[m][i];
                    }
                    self.count_total[m] += self.count_sum[m];
                    if self.window_limit {
                        self.count_total[m] -= self.count_list[self.iwindow][m];
                    }
                    self.count_list[self.iwindow][m] = self.count_sum[m];
                }
                self.iwindow += 1;
                if self.iwindow == nwindow {
                    self.iwindow = 0;
                    self.window_limit = true;
                }
                self.norm = if self.window_limit { nwindow as i64 } else { self.iwindow as i64 };
            }
        }
    }

    /// `compute_array(i, j)`: columns `0..ndim` are bin coordinates, then the
    /// count, then the values (0-based `j`). Out-of-range or not yet averaged
    /// gives `0.0`, as upstream.
    #[must_use]
    pub fn array(&self, i: usize, j: usize) -> f64 {
        let nd = self.dims.len();
        if i >= self.nbins {
            return 0.0;
        }
        if j < nd {
            return self.coord[i][j];
        }
        if self.norm == 0 {
            return 0.0;
        }
        let norm = self.norm as f64;
        if j == nd {
            return self.count_total[i] / norm;
        }
        self.values_total[i][j - nd - 1] / norm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_arguments() {
        assert!(AveTime::new(7, 3, 50, 1, AveMode::One, 0).is_err());
        assert!(AveTime::new(10, 11, 100, 1, AveMode::One, 0).is_err());
        assert!(AveTime::new(0, 1, 10, 1, AveMode::One, 0).is_err());
        assert!(AveTime::new(1, 1, 10, 1, AveMode::Window(0), 0).is_err());
    }

    #[test]
    fn window_and_running_means() {
        // nevery=1, nrepeat=1, nfreq=1: every step is an output
        let mut w = AveTime::new(1, 1, 1, 1, AveMode::Window(2), 1).unwrap();
        let mut r = AveTime::new(1, 1, 1, 1, AveMode::Running, 1).unwrap();
        for (s, x) in [(1, 1.0), (2, 3.0), (3, 5.0)] {
            w.end_of_step(s, &[x]);
            r.end_of_step(s, &[x]);
        }
        assert_eq!(w.scalar(), 4.0); // (3 + 5) / 2
        assert_eq!(r.scalar(), 3.0); // (1 + 3 + 5) / 3
        assert_eq!(AveTime::new(1, 1, 1, 1, AveMode::One, 0).unwrap().scalar(), 0.0);
    }

    #[test]
    fn ave_atom_averages_and_rejects() {
        assert!(AveAtom::new(7, 3, 50, 1, 1, 0).is_err());
        let mut a = AveAtom::new(1, 2, 2, 2, 1, 0).unwrap();
        a.end_of_step(1, &[&[1.0, 10.0]]);
        a.end_of_step(2, &[&[3.0, 20.0]]);
        assert_eq!(a.value(0, 0), 2.0);
        assert_eq!(a.value(1, 0), 15.0);
    }

    #[test]
    fn ave_spatial_bins_and_averages() {
        let bx = SpatialBox { lo: [0.0; 3], hi: [1.0, 1.0, 2.0], periodic: [false, false, true] };
        let dz = SpatialDim { axis: 2, origin: SpatialOrigin::Lower, delta: 1.0 };
        let dx = SpatialDim { axis: 0, origin: SpatialOrigin::Center, delta: 0.5 };
        let dy = SpatialDim { axis: 1, origin: SpatialOrigin::Coord(0.25), delta: 0.5 };
        let vals = vec![
            SpatialValue::V(2),
            SpatialValue::DensityNumber,
            SpatialValue::DensityMass,
            SpatialValue::Column(0),
            SpatialValue::F(0),
        ];
        let mk = |d: Vec<SpatialDim>, n, a| AveSpatial::new(1, 1, 1, d, vals.clone(), n, a, bx, 1);
        assert!(mk(vec![], SpatialNorm::All, AveMode::One).is_err());
        assert!(mk(vec![dz, dz], SpatialNorm::All, AveMode::One).is_err());
        assert!(mk(vec![dz], SpatialNorm::All, AveMode::Window(0)).is_err());
        for norm in [SpatialNorm::All, SpatialNorm::Sample] {
            for ave in [AveMode::One, AveMode::Running, AveMode::Window(2)] {
                for dims in [vec![dz], vec![dz, dx], vec![dz, dx, dy]] {
                    let nd = dims.len();
                    let mut a = mk(dims, norm, ave).unwrap().with_region();
                    let x = [[0.2, 0.1, 0.5], [0.7, 0.9, 1.5], [0.1, 0.1, -0.5]];
                    let v = [[0.0, 0.0, 1.0], [0.0, 0.0, 3.0], [0.0, 0.0, 5.0]];
                    let f = [[1.0, 0.0, 0.0]; 3];
                    let col = [1.0, 2.0, 4.0];
                    for s in 1..=3 {
                        a.end_of_step(s, &x, &v, &f, &[2.0, 2.0, 2.0], &[&col], Some(&[true, true, false]));
                    }
                    assert!(a.nbins() >= 2 && a.bin_volume() > 0.0);
                    assert!(a.array(0, 0) > 0.0 && a.array(a.nbins(), 0) == 0.0);
                    let _ = a.array(0, nd + 1);
                }
            }
        }
        // 1-d, norm all, ave one: z = -0.5 wraps to 1.5, so bin 1 holds atoms 1 and 2
        let mut a = AveSpatial::new(1, 1, 1, vec![dz], vec![SpatialValue::V(2)], SpatialNorm::All, AveMode::One, bx, 1).unwrap();
        assert_eq!(a.array(0, 1), 0.0); // not yet averaged
        let v = [[0.0; 3], [0.0, 0.0, 3.0], [0.0, 0.0, 5.0]];
        a.end_of_step(1, &[[0.0, 0.0, 0.5], [0.0, 0.0, 1.5], [0.0, 0.0, -0.5]], &v, &[[0.0; 3]; 3], &[1.0; 3], &[], None);
        assert_eq!(a.array(0, 1), 1.0);
        assert_eq!(a.array(1, 1), 2.0);
        assert_eq!(a.array(1, 2), 4.0);
    }
}
