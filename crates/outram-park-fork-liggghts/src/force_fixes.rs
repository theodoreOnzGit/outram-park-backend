// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Force fixes `fix addforce`, `fix viscous`, `fix freeze` (gh:#1418) and `fix wall/region` (gh:#1416).
//!
//! Translated from LIGGGHTS-PUBLIC `3d5c00f2`: `src/fix_addforce.cpp`
//! (`FixAddForce::post_force`), `src/fix_viscous.cpp`
//! (`FixViscous::post_force`) and `src/fix_freeze.cpp`
//! (`FixFreeze::post_force`). Each runs at `post_force`, after the pair, wall
//! and gravity forces, in the order the fixes are declared in the deck; this
//! crate applies them at the end of [`crate::granular_system::GranularSystem`]'s
//! force evaluation, after gravity, in the order they were added.
//!
//! `fix wall/region` is translated from `src/fix_wall_region.cpp` (LAMMPS
//! original, Sandia, via LIGGGHTS-PUBLIC `3d5c00f2`): `lj93`, `lj126`,
//! `colloid` and `harmonic` styles, the energy scalar and the three force
//! components on the wall, for an INTERIOR `block` or `cylinder` region
//! (`units box`, not dynamic): contacts from
//! [`InsertRegion::surface_interior`]. Not ported: exterior regions, other
//! region styles, `side out`, moving/rotating regions. `pow` and `log` are
//! the platform libm (`f64::powf`, `f64::ln`), the same glibc as upstream.
//!
//! ## Scope, stated plainly
//!
//!
//! * `addforce`: constant and equal-style (`v_name`) components, via
//!   [`crate::variable`]. NOT ported: atom-style variables, the `region`
//!   keyword and the `energy` keyword. The unwrapped-coordinate "potential
//!   energy" `foriginal[0]` is not computed (no periodic images here); the
//!   force sum `foriginal[1..3]` is.
//! * `viscous`: `gamma` per particle type (`gamma[type[i]]`).
//! * `freeze`: zeroes force and torque of the group. Upstream also zeroes the
//!   `Ksl` property of a tangential-stiffness fix when present; this crate has
//!   no such per-atom property, so there is nothing to zero.
//! * Groups are a per-particle mask (`group <name> id <= N` in the deck), in
//!   particle-vector order, which is atom-id order here.
//! * `end_step` of the [`VarContext`] is not known to the system (no `run`
//!   length is stored), so `ramp()` in an `addforce` variable is not faithful;
//!   use `step`/`time` based formulas.

use crate::insert::InsertRegion;
use crate::particle::Vec3;
use crate::variable::{Expr, VarContext, VariableTable};

/// One component of an `addforce` force: a constant or an equal-style variable.
#[derive(Debug, Clone, PartialEq)]
pub enum ForceComponent {
    /// A number (`fix addforce 0 0 -1`).
    Constant(f64),
    /// `v_name`, an equal-style variable evaluated every step
    /// (`input->variable->compute_equal`).
    Equal(Expr),
}

impl ForceComponent {
    /// Parse an `addforce` argument: a number or `v_<name>` of `table`.
    ///
    /// # Errors
    /// On an undefined variable or an unparsable number.
    pub fn parse(arg: &str, table: &VariableTable) -> Result<Self, String> {
        if let Some(name) = arg.strip_prefix("v_") {
            let e = table
                .get(name)
                .ok_or_else(|| format!("Variable name {name} for fix addforce does not exist"))?;
            Ok(Self::Equal(e.clone()))
        } else {
            arg.parse::<f64>()
                .map(Self::Constant)
                .map_err(|e| format!("fix addforce: bad value {arg}: {e}"))
        }
    }

    fn value(&self, ctx: &VarContext) -> Result<f64, String> {
        match self {
            Self::Constant(v) => Ok(*v),
            Self::Equal(e) => e.eval(ctx),
        }
    }
}

/// Potential of a `fix wall/region` (`enum{LJ93,LJ126,COLLOID,HARMONIC}`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallRegionStyle {
    /// Lennard-Jones 9/3.
    Lj93,
    /// Lennard-Jones 12/6.
    Lj126,
    /// Colloid (finite-size sphere against a flat wall).
    Colloid,
    /// Harmonic: `2 epsilon (cutoff - r)`.
    Harmonic,
}

/// `FixWallRegion::init` coefficients and `lj93`/`lj126`/`colloid`/`harmonic`:
/// returns `(fwall, eng)` at wall distance `r` (`rad` for colloid).
fn wall_region_pair(
    style: WallRegionStyle,
    epsilon: f64,
    sigma: f64,
    cutoff: f64,
    r: f64,
    rad: f64,
) -> (f64, f64) {
    // coefficients and offset exactly as FixWallRegion::init
    let (coeff1, coeff2, coeff3, coeff4, offset);
    match style {
        WallRegionStyle::Lj93 => {
            coeff1 = 6.0 / 5.0 * epsilon * sigma.powf(9.0);
            coeff2 = 3.0 * epsilon * sigma.powf(3.0);
            coeff3 = 2.0 / 15.0 * epsilon * sigma.powf(9.0);
            coeff4 = epsilon * sigma.powf(3.0);
            let rinv = 1.0 / cutoff;
            let r2inv = rinv * rinv;
            let r4inv = r2inv * r2inv;
            offset = coeff3 * r4inv * r4inv * rinv - coeff4 * r2inv * rinv;
        }
        WallRegionStyle::Lj126 => {
            coeff1 = 48.0 * epsilon * sigma.powf(12.0);
            coeff2 = 24.0 * epsilon * sigma.powf(6.0);
            coeff3 = 4.0 * epsilon * sigma.powf(12.0);
            coeff4 = 4.0 * epsilon * sigma.powf(6.0);
            let r2inv = 1.0 / (cutoff * cutoff);
            let r6inv = r2inv * r2inv * r2inv;
            offset = r6inv * (coeff3 * r6inv - coeff4);
        }
        WallRegionStyle::Colloid => {
            coeff1 = -4.0 / 315.0 * epsilon * sigma.powf(6.0);
            coeff2 = -2.0 / 3.0 * epsilon;
            coeff3 = epsilon * sigma.powf(6.0) / 7560.0;
            coeff4 = epsilon / 6.0;
            let rinv = 1.0 / cutoff;
            let r2inv = rinv * rinv;
            let r4inv = r2inv * r2inv;
            offset = coeff3 * r4inv * r4inv * rinv - coeff4 * r2inv * rinv;
        }
        WallRegionStyle::Harmonic => {
            let dr = cutoff - r;
            return (2.0 * epsilon * dr, epsilon * dr * dr);
        }
    }
    match style {
        WallRegionStyle::Lj93 => {
            let rinv = 1.0 / r;
            let r2inv = rinv * rinv;
            let r4inv = r2inv * r2inv;
            let r10inv = r4inv * r4inv * r2inv;
            (
                coeff1 * r10inv - coeff2 * r4inv,
                coeff3 * r4inv * r4inv * rinv - coeff4 * r2inv * rinv - offset,
            )
        }
        WallRegionStyle::Lj126 => {
            let rinv = 1.0 / r;
            let r2inv = rinv * rinv;
            let r6inv = r2inv * r2inv * r2inv;
            (
                r6inv * (coeff1 * r6inv - coeff2) * rinv,
                r6inv * (coeff3 * r6inv - coeff4) - offset,
            )
        }
        _ => {
            let new_coeff2 = coeff2 * rad * rad * rad;
            let diam = 2.0 * rad;
            let rad2 = rad * rad;
            let rad4 = rad2 * rad2;
            let rad8 = rad4 * rad4;
            let delta2 = rad2 - r * r;
            let rinv = 1.0 / delta2;
            let r2inv = rinv * rinv;
            let r4inv = r2inv * r2inv;
            let r8inv = r4inv * r4inv;
            let fwall = coeff1
                * (rad8 * rad
                    + 27.0 * rad4 * rad2 * rad * r.powf(2.0)
                    + 63.0 * rad4 * rad * r.powf(4.0)
                    + 21.0 * rad2 * rad * r.powf(6.0))
                * r8inv
                - new_coeff2 * r2inv;
            let r2 = 0.5 * diam - r;
            let rinv2 = 1.0 / r2;
            let r2inv2 = rinv2 * rinv2;
            let r4inv2 = r2inv2 * r2inv2;
            let r3 = r + 0.5 * diam;
            let rinv3 = 1.0 / r3;
            let r2inv3 = rinv3 * rinv3;
            let r4inv3 = r2inv3 * r2inv3;
            let eng = coeff3
                * ((-3.5 * diam + r) * r4inv2 * r2inv2 * rinv2
                    + (3.5 * diam + r) * r4inv3 * r2inv3 * rinv3)
                - coeff4
                    * ((-diam * r + r2 * r3 * ((-r2).ln() - r3.ln())) * (-rinv2) * rinv3)
                - offset;
            (fwall, eng)
        }
    }
}

/// A force fix applied at `post_force`.
#[derive(Debug, Clone, PartialEq)]
pub enum ForceFix {
    /// `fix ID group addforce fx fy fz`.
    AddForce {
        /// Group membership per particle.
        group: Vec<bool>,
        /// The three components.
        components: [ForceComponent; 3],
        /// `foriginal[1..3]`: sum of the group's forces before this fix added
        /// its own (upstream `compute_vector` source), updated each step.
        foriginal: [f64; 3],
    },
    /// `fix ID group viscous gamma` (per type: `gamma[type]`).
    Viscous {
        /// Group membership per particle.
        group: Vec<bool>,
        /// Drag coefficient indexed by particle type.
        gamma: Vec<f64>,
    },
    /// `fix ID group wall/region region style epsilon sigma cutoff`
    /// (`FixWallRegion::post_force`).
    WallRegion {
        /// Group membership per particle.
        group: Vec<bool>,
        /// The interior region.
        region: InsertRegion,
        /// Wall potential.
        style: WallRegionStyle,
        /// `epsilon`.
        epsilon: f64,
        /// `sigma`.
        sigma: f64,
        /// `cutoff` (> 0).
        cutoff: f64,
        /// `ewall[0..4]`: energy, then minus the summed force on the particles
        /// (the force on the wall), rebuilt every step.
        ewall: [f64; 4],
    },
    /// `fix ID group freeze`.
    Freeze {
        /// Group membership per particle.
        group: Vec<bool>,
        /// Sum of the group's forces before they were zeroed.
        foriginal: [f64; 3],
    },
}

impl ForceFix {
    /// `order` is the local atom order (sums over atoms, e.g. the wall
    /// energy, are accumulated in it; see `GranularSystem::local_order_indices`).
    /// `FixAddForce::post_force` / `FixViscous::post_force` /
    /// `FixFreeze::post_force`, applied to `forces`/`torques` of the particles
    /// with velocities `velocities` and types `types`.
    ///
    /// # Errors
    /// An equal-style variable that fails to evaluate (upstream `error->all`).
    pub fn apply(
        &mut self,
        ctx: &VarContext,
        order: &[usize],
        positions: &[Vec3],
        radii: &[f64],
        velocities: &[Vec3],
        types: &[usize],
        forces: &mut [Vec3],
        torques: &mut [Vec3],
    ) -> Result<(), String> {
        match self {
            Self::AddForce {
                group,
                components,
                foriginal,
            } => {
                *foriginal = [0.0; 3];
                // variables are evaluated once, before the atom loop
                let v = [
                    components[0].value(ctx)?,
                    components[1].value(ctx)?,
                    components[2].value(ctx)?,
                ];
                for (i, f) in forces.iter_mut().enumerate() {
                    if group[i] {
                        foriginal[0] += f.x;
                        foriginal[1] += f.y;
                        foriginal[2] += f.z;
                        f.x += v[0];
                        f.y += v[1];
                        f.z += v[2];
                    }
                }
            }
            Self::Viscous { group, gamma } => {
                for (i, f) in forces.iter_mut().enumerate() {
                    if group[i] {
                        let drag = gamma[types[i]];
                        f.x -= drag * velocities[i].x;
                        f.y -= drag * velocities[i].y;
                        f.z -= drag * velocities[i].z;
                    }
                }
            }
            Self::WallRegion {
                group,
                region,
                style,
                epsilon,
                sigma,
                cutoff,
                ewall,
            } => {
                *ewall = [0.0; 4];
                let mut onflag = false;
                for &i in order {
                    let f = &mut forces[i];
                    if !group[i] {
                        continue;
                    }
                    if !region.contains(positions[i]) {
                        onflag = true;
                        continue;
                    }
                    let tooclose = if *style == WallRegionStyle::Colloid {
                        radii[i]
                    } else {
                        0.0
                    };
                    for (r, del) in region.surface_interior(positions[i], *cutoff) {
                        if r <= tooclose {
                            onflag = true;
                            continue;
                        }
                        let rinv = 1.0 / r;
                        let (fwall, eng) =
                            wall_region_pair(*style, *epsilon, *sigma, *cutoff, r, radii[i]);
                        ewall[0] += eng;
                        let fx = fwall * del[0] * rinv;
                        let fy = fwall * del[1] * rinv;
                        let fz = fwall * del[2] * rinv;
                        f.x += fx;
                        f.y += fy;
                        f.z += fz;
                        ewall[1] -= fx;
                        ewall[2] -= fy;
                        ewall[3] -= fz;
                    }
                }
                if onflag {
                    return Err(
                        "Particle on or inside surface of region used in fix wall/region".into(),
                    );
                }
            }
            Self::Freeze { group, foriginal } => {
                *foriginal = [0.0; 3];
                for (i, f) in forces.iter_mut().enumerate() {
                    if group[i] {
                        foriginal[0] += f.x;
                        foriginal[1] += f.y;
                        foriginal[2] += f.z;
                        *f = Vec3::zero();
                        torques[i] = Vec3::zero();
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(step: i64) -> VarContext {
        VarContext {
            step,
            begin_step: 0,
            end_step: 100,
            dt: 1e-5,
        }
    }

    #[test]
    fn addforce_viscous_freeze_apply_in_order() {
        let mut t = VariableTable::new();
        t.define_equal("fz", "2*step").unwrap();
        let mut fixes = vec![
            ForceFix::AddForce {
                group: vec![true, false],
                components: [
                    ForceComponent::Constant(1.0),
                    ForceComponent::Constant(0.0),
                    ForceComponent::parse("v_fz", &t).unwrap(),
                ],
                foriginal: [0.0; 3],
            },
            ForceFix::Viscous {
                group: vec![true, true],
                gamma: vec![0.5],
            },
            ForceFix::Freeze {
                group: vec![false, true],
                foriginal: [0.0; 3],
            },
        ];
        let pos = [Vec3::zero(); 2];
        let rad = [0.005; 2];
        let v = [Vec3::new(2.0, 0.0, 0.0), Vec3::new(0.0, 4.0, 0.0)];
        let mut f = vec![Vec3::new(1.0, 1.0, 1.0); 2];
        let mut tq = vec![Vec3::new(1.0, 1.0, 1.0); 2];
        for fx in &mut fixes {
            fx.apply(&ctx(3), &[0, 1], &pos, &rad, &v, &[0, 0], &mut f, &mut tq).unwrap();
        }
        // particle 0: x 1+1-0.5*2, y 1, z 1+6
        assert_eq!(f[0], Vec3::new(1.0, 1.0, 7.0));
        // particle 1: frozen
        assert_eq!(f[1], Vec3::zero());
        assert_eq!(tq[1], Vec3::zero());
        assert_eq!(tq[0], Vec3::new(1.0, 1.0, 1.0));
        match &fixes[2] {
            ForceFix::Freeze { foriginal, .. } => assert_eq!(*foriginal, [1.0, -1.0, 1.0]),
            _ => unreachable!(),
        }
    }
}
