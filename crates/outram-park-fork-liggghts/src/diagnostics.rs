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
}
