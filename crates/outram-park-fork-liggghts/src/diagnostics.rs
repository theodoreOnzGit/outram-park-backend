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
        let mut w = AveTime::new(1, 1, 1, 1, AveMode::Window(2), 0).unwrap();
        let mut r = AveTime::new(1, 1, 1, 1, AveMode::Running, 0).unwrap();
        for (s, x) in [(1, 1.0), (2, 3.0), (3, 5.0)] {
            w.end_of_step(s, &[x]);
            r.end_of_step(s, &[x]);
        }
        assert_eq!(w.scalar(), 4.0); // (3 + 5) / 2
        assert_eq!(r.scalar(), 3.0); // (1 + 3 + 5) / 3
        assert_eq!(AveTime::new(1, 1, 1, 1, AveMode::One, 0).unwrap().scalar(), 0.0);
    }
}
