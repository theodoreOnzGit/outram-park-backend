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

//! Force fixes `fix addforce`, `fix viscous` and `fix freeze` (gh:#1418).
//!
//! Translated from LIGGGHTS-PUBLIC `3d5c00f2`: `src/fix_addforce.cpp`
//! (`FixAddForce::post_force`), `src/fix_viscous.cpp`
//! (`FixViscous::post_force`) and `src/fix_freeze.cpp`
//! (`FixFreeze::post_force`). Each runs at `post_force`, after the pair, wall
//! and gravity forces, in the order the fixes are declared in the deck; this
//! crate applies them at the end of [`crate::granular_system::GranularSystem`]'s
//! force evaluation, after gravity, in the order they were added.
//!
//! ## Scope, stated plainly
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
    /// `fix ID group freeze`.
    Freeze {
        /// Group membership per particle.
        group: Vec<bool>,
        /// Sum of the group's forces before they were zeroed.
        foriginal: [f64; 3],
    },
}

impl ForceFix {
    /// `FixAddForce::post_force` / `FixViscous::post_force` /
    /// `FixFreeze::post_force`, applied to `forces`/`torques` of the particles
    /// with velocities `velocities` and types `types`.
    ///
    /// # Errors
    /// An equal-style variable that fails to evaluate (upstream `error->all`).
    pub fn apply(
        &mut self,
        ctx: &VarContext,
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
        let v = [Vec3::new(2.0, 0.0, 0.0), Vec3::new(0.0, 4.0, 0.0)];
        let mut f = vec![Vec3::new(1.0, 1.0, 1.0); 2];
        let mut tq = vec![Vec3::new(1.0, 1.0, 1.0); 2];
        for fx in &mut fixes {
            fx.apply(&ctx(3), &v, &[0, 0], &mut f, &mut tq).unwrap();
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
