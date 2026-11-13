// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. OUTRAM PARK is free software: you can
// redistribute it and/or modify it under the terms of the GNU General Public
// License as published by the Free Software Foundation, either version 3 of
// the License, or (at your option) any later version. It is distributed
// WITHOUT ANY WARRANTY; see <https://www.gnu.org/licenses/>.

//! `fix property/atom/tracer` and `compute nparticles/tracer/region`
//! (gh:#1418).
//!
//! Translated from LIGGGHTS-PUBLIC `3d5c00f2`: `src/fix_property_atom_tracer.cpp`
//! (`FixPropertyAtomTracer::{end_of_step, compute_scalar}`) and
//! `src/compute_nparticles_tracer_region.cpp`
//! (`ComputeNparticlesTracerRegion::{compute_vector, compute_vector_eval}`).
//! Copyright DCS Computing GmbH, Linz / JKU Linz, GPL-3.0.
//!
//! ## Scope, stated plainly
//!
//! * Group `all`, one process, regions as [`InsertRegion`] (`block`/`cylinder`,
//!   interior, `units box`, not dynamic) -- `Region::match` of those is
//!   `inside` (boundary inclusive).
//! * The `compute`'s `periodic` (image-flag) keyword is NOT ported (the
//!   decks are non-periodic). `fix property/atom/regiontracer*`
//!   (residence-time tracers) is not ported.
//! * The marker is stored per particle by tag (index into
//!   [`GranularSystem::particles`](crate::granular_system::GranularSystem::particles));
//!   particles inserted later start at `0.0`, like `fix property/atom`'s
//!   default. Sums over particles take the local order as an argument so the
//!   mass sum is accumulated in upstream's order.

use crate::insert::InsertRegion;
use crate::particle::Particle;

/// `MathExtraLiggghts::compDouble(a, b, prec)` (`|a - b| < prec`).
fn comp_double(a: f64, b: f64, prec: f64) -> bool {
    (a - b).abs() < prec
}

/// `marker_style` of `fix property/atom/tracer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerStyle {
    /// Mark every unmarked particle found in the region on every check.
    Heaviside,
    /// Mark only at the first check on or after `mark_step`.
    Dirac,
    /// Never mark.
    None,
}

/// `fix ID all property/atom/tracer region_mark R mark_step N marker_style S
/// check_mark_every K`.
#[derive(Debug, Clone)]
pub struct PropertyAtomTracer {
    region: InsertRegion,
    step: i64,
    style: MarkerStyle,
    check_every: i64,
    first_mark: bool,
    nmarked_last: i64,
    nmarked: i64,
    marker: Vec<f64>,
}

impl PropertyAtomTracer {
    /// Create the fix at time step `now` (`update->ntimestep` when the deck
    /// line is read). `mark_step < now` clears `first_mark_` like upstream.
    ///
    /// # Errors
    ///
    /// A message for `mark_step < 0` or `check_mark_every < 0` (upstream
    /// errors). `check_mark_every == 0` is rejected too: LIGGGHTS accepts it
    /// but then evaluates `ntimestep % 0` (undefined).
    pub fn new(
        region: InsertRegion,
        mark_step: i64,
        style: MarkerStyle,
        check_mark_every: i64,
        now: i64,
    ) -> Result<Self, String> {
        if mark_step < 0 {
            return Err("mark_step > 0 required".into());
        }
        if check_mark_every <= 0 {
            return Err("check_mark_every > 0 required".into());
        }
        Ok(Self {
            region,
            step: mark_step,
            style,
            check_every: check_mark_every,
            first_mark: mark_step >= now,
            nmarked_last: 0,
            nmarked: 0,
            marker: Vec::new(),
        })
    }

    /// `end_of_step` at time step `step` (`Modify` calls it only on multiples
    /// of `nevery = check_mark_every`). `particles` in tag order.
    pub fn end_of_step(&mut self, step: i64, particles: &[Particle]) {
        if step % self.check_every != 0 {
            return;
        }
        if self.marker.len() < particles.len() {
            self.marker.resize(particles.len(), 0.0);
        }
        if step < self.step
            || self.style == MarkerStyle::None
            || (self.style == MarkerStyle::Dirac && !self.first_mark)
        {
            return;
        }
        let mut nmarked_this = 0;
        for (i, p) in particles.iter().enumerate() {
            if !comp_double(self.marker[i], 1.0, 1e-5) && self.region.contains(p.position) {
                self.marker[i] = 1.0;
                nmarked_this += 1;
            }
        }
        self.nmarked += nmarked_this;
        self.first_mark = false;
    }

    /// `compute_scalar`: markers added since the last call.
    pub fn compute_scalar(&mut self) -> f64 {
        let r = self.nmarked - self.nmarked_last;
        self.nmarked_last = self.nmarked;
        r as f64
    }

    /// The per-atom marker (`vector_atom`), by tag, as the dump shows it.
    #[must_use]
    pub fn marker(&self, i: usize) -> f64 {
        self.marker.get(i).copied().unwrap_or(0.0)
    }
}

/// `compute ID all nparticles/tracer/region region_count R tracer T
/// reset_marker yes|no`.
#[derive(Debug, Clone)]
pub struct NparticlesTracerRegion {
    region: InsertRegion,
    reset_marker: bool,
}

impl NparticlesTracerRegion {
    /// Create the compute (`reset_marker` defaults to `yes` upstream).
    #[must_use]
    pub fn new(region_count: InsertRegion, reset_marker: bool) -> Self {
        Self { region: region_count, reset_marker }
    }

    /// `compute_vector`: `[n_in_region, n_marked, mass_in_region,
    /// mass_marked]`. `particles` in tag order, `order` the local-order
    /// indices ([`GranularSystem::local_order_indices`](crate::granular_system::GranularSystem::local_order_indices)).
    /// Like upstream the mass pass (the second one) clears the markers of
    /// marked particles in the region when `reset_marker` is on.
    pub fn compute_vector(
        &self,
        tracer: &mut PropertyAtomTracer,
        particles: &[Particle],
        order: &[usize],
    ) -> [f64; 4] {
        if tracer.marker.len() < particles.len() {
            tracer.marker.resize(particles.len(), 0.0);
        }
        let (n_tot, n_marked) = self.eval(false, tracer, particles, order);
        let (m_tot, m_marked) = self.eval(true, tracer, particles, order);
        [n_tot, n_marked, m_tot, m_marked]
    }

    fn eval(
        &self,
        count_mass: bool,
        tracer: &mut PropertyAtomTracer,
        particles: &[Particle],
        order: &[usize],
    ) -> (f64, f64) {
        let (mut tot, mut marked) = (0.0, 0.0);
        for &i in order {
            let p = &particles[i];
            if !self.region.contains(p.position) {
                continue;
            }
            tot += if count_mass { p.mass } else { 1.0 };
            if comp_double(tracer.marker[i], 1.0, 1e-5) {
                marked += if count_mass { p.mass } else { 1.0 };
                if self.reset_marker && count_mass {
                    tracer.marker[i] = 0.0;
                }
            }
        }
        (tot, marked)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::particle::Vec3;
    use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
    use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

    fn at(z: f64) -> Particle {
        Particle::new(
            Vec3::new(0.0, 0.0, z),
            Vec3::zero(),
            Vec3::zero(),
            Mass::new::<kilogram>(2.0),
            Length::new::<meter>(0.003),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .unwrap()
    }

    #[test]
    fn heaviside_dirac_and_reset() {
        let reg = InsertRegion::block(Vec3::new(-1.0, -1.0, 0.0), Vec3::new(1.0, 1.0, 1.0)).unwrap();
        let mut h = PropertyAtomTracer::new(reg, 3, MarkerStyle::Heaviside, 5, 0).unwrap();
        let mut d = PropertyAtomTracer::new(reg, 3, MarkerStyle::Dirac, 5, 0).unwrap();
        let ps = vec![at(0.5), at(2.0)];
        h.end_of_step(3, &ps); // not a multiple of 5
        h.end_of_step(5, &ps);
        d.end_of_step(5, &ps);
        assert_eq!((h.marker(0), h.marker(1)), (1.0, 0.0));
        let ps2 = vec![at(0.5), at(0.6)];
        h.end_of_step(10, &ps2);
        d.end_of_step(10, &ps2); // dirac: only once
        assert_eq!(h.marker(1), 1.0);
        assert_eq!(d.marker(1), 0.0);
        assert_eq!(h.compute_scalar(), 2.0);
        assert_eq!(h.compute_scalar(), 0.0);
        let c = NparticlesTracerRegion::new(reg, true);
        let v = c.compute_vector(&mut h, &ps2, &[1, 0]);
        assert_eq!(v, [2.0, 2.0, 4.0, 4.0]);
        assert_eq!(h.marker(0), 0.0); // reset by the mass pass
        assert!(PropertyAtomTracer::new(reg, -1, MarkerStyle::None, 5, 0).is_err());
        let mut n = PropertyAtomTracer::new(reg, 0, MarkerStyle::None, 1, 0).unwrap();
        n.end_of_step(1, &ps);
        assert_eq!(n.marker(0), 0.0);
    }
}
