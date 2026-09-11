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
//
// ---------------------------------------------------------------------------
// Ported from:
//   Project:  PRISMS-Plasticity (prisms-center/plasticity)
//   Source:   applications/crystalPlasticity/fcc/*/slipNormals.txt
//             applications/crystalPlasticity/fcc/*/slipDirections.txt
//             applications/crystalPlasticity/fcc/*/LatentHardeningRatio.txt
//             applications/crystalPlasticity/bcc/simpleTension/slip*.txt
//             src/materialModels/crystalPlasticity/MaterialModels/
//                 RateDependentModel/calculatePlasticity.cc  (Schmid tensor,
//                 lines 199-230; latent-hardening product, lines 645-657)
//   Version:  commit ffdf4eb67b55b84f8b20cbb21407cf310ec3a7e4 (2026-08-27)
//   Copyright (c) 2016 The Regents of the University of Michigan, PRISMS Center
//   Licence:  LGPL-2.1-or-later upstream; relicensed to GPL-3.0-only here under
//             LGPL-2.1 section 3. See crates/farrer-park/NOTICE and
//             crates/farrer-park/docs/upstream-provenance.md.
// ---------------------------------------------------------------------------

//! Slip-system geometry: which planes slip, in which directions, and the
//! Schmid tensors that follow.
//!
//! # What belongs in this module
//!
//! The **crystal-frame** geometry of a slip family — unit plane normals `n`,
//! unit slip directions `m`, the symmetric Schmid tensor `P = sym(m (x) n)`,
//! the Schmid factor for a given loading axis, and the latent-hardening
//! `q`-matrix that says how much slip on one system hardens another.
//!
//! # What does NOT belong here
//!
//! Orientation (see [`crate::crystal::orient`]), elasticity (see
//! [`crate::crystal::elastic`]) and the flow rule (see
//! [`crate::crystal::flow`]). Everything in this module is fixed by the
//! crystal structure alone and carries no material constants and no history.
//!
//! # Provenance
//!
//! The slip directions, plane normals **and their ordering** are transcribed
//! from the PRISMS-Plasticity input decks named in the header above, so that a
//! system index here means the same physical system it does upstream. The
//! `q`-matrix values (1.0 coplanar, 1.4 non-coplanar) are likewise upstream's,
//! read from `LatentHardeningRatio.txt`; here they are *computed* from
//! coplanarity rather than transcribed, and a test checks the computed matrix
//! against the upstream file's pattern.
//!
//! # Units
//!
//! Everything in this module is **dimensionless**: `n` and `m` are unit
//! vectors in the crystal lattice frame, the Schmid tensor is dimensionless,
//! the Schmid factor is dimensionless and lies in `[0, 0.5]`, and the
//! `q`-matrix entries are dimensionless ratios.

use crate::tensor::Voigt6;

/// The largest number of slip systems any [`SlipFamily`] in this crate has.
///
/// Twelve. Both implemented families — FCC `{111}<110>` and BCC `{110}<111>` —
/// have exactly twelve systems, so every per-system array is a fixed
/// `[f64; 12]` and the crystal state stays `Copy`. BCC's additional
/// `{112}<111>` and `{123}<111>` families (another 12 and 24 systems) are
/// **not** implemented; see the crate `CLAUDE.md` and bead `op-q75c`.
///
/// Dimensionless count.
pub const MAX_SLIP_SYSTEMS: usize = 12;

/// One slip system: a slip plane and a slip direction lying in it, both in the
/// **crystal lattice frame**.
///
/// # Units
///
/// Both vectors are dimensionless and of unit length. `normal` and `direction`
/// are orthogonal: a dislocation glides *within* its slip plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlipSystem {
    /// Unit slip-plane normal `n` in the crystal frame, dimensionless.
    pub normal: [f64; 3],
    /// Unit slip direction `m` in the crystal frame, dimensionless, with
    /// `m . n = 0`.
    pub direction: [f64; 3],
}

impl SlipSystem {
    /// Build a slip system from unnormalised Miller indices, e.g.
    /// `SlipSystem::from_miller([1.0, 1.0, 1.0], [0.0, 1.0, -1.0])` for the
    /// FCC system `(111)[0 1 -1]`.
    ///
    /// Both arguments are normalised to unit length here, so the caller may
    /// pass raw integer indices. No orthogonality check is made — use
    /// [`SlipSystem::orthogonality_defect`] if you need one.
    ///
    /// # Units
    ///
    /// Both arguments and the result are dimensionless.
    #[must_use]
    pub fn from_miller(plane: [f64; 3], direction: [f64; 3]) -> Self {
        Self {
            normal: normalise(plane),
            direction: normalise(direction),
        }
    }

    /// `|m . n|`, which must be zero for a physically meaningful slip system.
    ///
    /// Dimensionless; used by the geometry-invariant verification case rather
    /// than by the constitutive path, which assumes the invariant holds.
    #[must_use]
    pub fn orthogonality_defect(&self) -> f64 {
        dot(self.direction, self.normal).abs()
    }

    /// The **symmetric Schmid tensor** `P = (m (x) n + n (x) m) / 2`, in
    /// stress-form Voigt order (see [`crate::tensor`]).
    ///
    /// This is the tensor that both extracts the resolved shear stress,
    /// `tau = sigma : P`, and gives the direction of the plastic strain rate
    /// contributed by this system, `eps_p_dot = gamma_dot P`. It is
    /// **deviatoric**: `tr P = m . n = 0`, so slip preserves volume exactly.
    ///
    /// The symmetric part is the correct object for a **small-strain** model
    /// with a symmetric Cauchy stress. PRISMS-Plasticity stores the
    /// *unsymmetrised* `m (x) n` because it is a finite-deformation code whose
    /// plastic velocity gradient `L_p = sum gamma_dot m (x) n` carries a spin;
    /// contracting either with a symmetric stress gives the same `tau`. The
    /// spin, and therefore lattice reorientation, is **not** modelled here —
    /// see the module-level note in [`crate::crystal`].
    ///
    /// # Units
    ///
    /// Dimensionless.
    #[must_use]
    pub fn schmid_tensor(&self) -> Voigt6 {
        let (m, n) = (self.direction, self.normal);
        let s = |i: usize, j: usize| 0.5 * (m[i] * n[j] + m[j] * n[i]);
        Voigt6::new(s(0, 0), s(1, 1), s(2, 2), s(1, 2), s(0, 2), s(0, 1))
    }

    /// The **Schmid factor** for a uniaxial load along `axis`, expressed in the
    /// same (crystal) frame as the slip system:
    /// `mu = |(m . t)(n . t)|` with `t` the unit load axis.
    ///
    /// For a uniaxial stress `sigma t (x) t` the resolved shear on this system
    /// is `tau = mu sigma`, so `mu` is the fraction of the applied stress that
    /// drives this system. It is dimensionless and bounded by `0.5`, attained
    /// when `m` and `n` both make 45 degrees with the axis.
    ///
    /// `axis` need not be normalised; it is normalised here. A zero-length
    /// axis returns `0.0`.
    ///
    /// # Units
    ///
    /// `axis` dimensionless (a direction), result dimensionless.
    #[must_use]
    pub fn schmid_factor(&self, axis: [f64; 3]) -> f64 {
        let t = normalise(axis);
        (dot(self.direction, t) * dot(self.normal, t)).abs()
    }
}

/// The closed set of slip families this crate implements.
///
/// An enum rather than a user-supplied table, deliberately: the geometry is
/// fixed by the crystal structure, the ordering must match
/// PRISMS-Plasticity's so that a system index means the same thing in both
/// codes, and an exhaustive `match` makes adding a family a compile error
/// everywhere it has to be handled.
///
/// # Units
///
/// Dimensionless — a selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SlipFamily {
    /// **FCC octahedral slip**, `{111}<110>`: four `{111}` planes with three
    /// `<110>` directions each, twelve systems.
    ///
    /// The family that operates in aluminium, copper, nickel, austenitic
    /// stainless steels and nickel-based superalloys at all temperatures of
    /// interest.
    #[default]
    FccOctahedral,
    /// **BCC `{110}<111>` slip**: six `{110}` planes with two `<111>`
    /// directions each, twelve systems.
    ///
    /// The easiest family in ferritic and martensitic steels. BCC also slips
    /// on `{112}` and `{123}`, and shows non-Schmid (twinning/anti-twinning
    /// asymmetry) behaviour at low temperature; **neither is implemented**, so
    /// a BCC result from this crate is the `{110}` contribution only.
    Bcc110,
}

impl SlipFamily {
    /// A short name for diagnostics and table headings.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            SlipFamily::FccOctahedral => "FCC {111}<110>",
            SlipFamily::Bcc110 => "BCC {110}<111>",
        }
    }

    /// How many systems this family has. Always `12` for both implemented
    /// families; the accessor exists so callers do not hard-code it.
    ///
    /// Dimensionless count.
    #[must_use]
    pub fn n_systems(self) -> usize {
        match self {
            SlipFamily::FccOctahedral | SlipFamily::Bcc110 => 12,
        }
    }

    /// The twelve slip systems, in **PRISMS-Plasticity's ordering**, in the
    /// crystal lattice frame.
    ///
    /// FCC, from `slipNormals.txt` / `slipDirections.txt`: the four `{111}`
    /// planes `(111)`, `(-1-11)`, `(-111)`, `(1-11)`, each followed by its
    /// three `<110>` directions.
    ///
    /// BCC, from the same files under `applications/crystalPlasticity/bcc/`:
    /// the six `{110}` planes `(011)`, `(101)`, `(110)`, `(0-11)`, `(10-1)`,
    /// `(-110)`, each followed by its two `<111>` directions.
    ///
    /// # Units
    ///
    /// Dimensionless unit vectors.
    #[must_use]
    pub fn systems(self) -> [SlipSystem; MAX_SLIP_SYSTEMS] {
        let miller: [([f64; 3], [f64; 3]); MAX_SLIP_SYSTEMS] = match self {
            SlipFamily::FccOctahedral => [
                ([1.0, 1.0, 1.0], [0.0, 1.0, -1.0]),
                ([1.0, 1.0, 1.0], [-1.0, 0.0, 1.0]),
                ([1.0, 1.0, 1.0], [1.0, -1.0, 0.0]),
                ([-1.0, -1.0, 1.0], [0.0, -1.0, -1.0]),
                ([-1.0, -1.0, 1.0], [1.0, 0.0, 1.0]),
                ([-1.0, -1.0, 1.0], [-1.0, 1.0, 0.0]),
                ([-1.0, 1.0, 1.0], [0.0, 1.0, -1.0]),
                ([-1.0, 1.0, 1.0], [1.0, 0.0, 1.0]),
                ([-1.0, 1.0, 1.0], [-1.0, -1.0, 0.0]),
                ([1.0, -1.0, 1.0], [0.0, -1.0, -1.0]),
                ([1.0, -1.0, 1.0], [-1.0, 0.0, 1.0]),
                ([1.0, -1.0, 1.0], [1.0, 1.0, 0.0]),
            ],
            SlipFamily::Bcc110 => [
                ([0.0, 1.0, 1.0], [1.0, -1.0, 1.0]),
                ([0.0, 1.0, 1.0], [1.0, 1.0, -1.0]),
                ([1.0, 0.0, 1.0], [-1.0, 1.0, 1.0]),
                ([1.0, 0.0, 1.0], [1.0, 1.0, -1.0]),
                ([1.0, 1.0, 0.0], [-1.0, 1.0, 1.0]),
                ([1.0, 1.0, 0.0], [1.0, -1.0, 1.0]),
                ([0.0, -1.0, 1.0], [1.0, 1.0, 1.0]),
                ([0.0, -1.0, 1.0], [-1.0, 1.0, 1.0]),
                ([1.0, 0.0, -1.0], [1.0, 1.0, 1.0]),
                ([1.0, 0.0, -1.0], [1.0, -1.0, 1.0]),
                ([-1.0, 1.0, 0.0], [1.0, 1.0, 1.0]),
                ([-1.0, 1.0, 0.0], [1.0, 1.0, -1.0]),
            ],
        };
        miller.map(|(p, d)| SlipSystem::from_miller(p, d))
    }

    /// The twelve symmetric Schmid tensors in the crystal frame, in the same
    /// order as [`SlipFamily::systems`]. Dimensionless.
    #[must_use]
    pub fn schmid_tensors(self) -> [Voigt6; MAX_SLIP_SYSTEMS] {
        self.systems().map(|s| s.schmid_tensor())
    }

    /// The **latent-hardening matrix** `q`, where `q[a][b]` multiplies the
    /// hardening that slip on system `b` induces on system `a`.
    ///
    /// Two values, exactly as in PRISMS-Plasticity's `LatentHardeningRatio.txt`:
    ///
    /// - `self_ratio` for a **coplanar** pair (`a` and `b` share a slip plane,
    ///   including `a == b`). Upstream uses `1.0`.
    /// - `latent_ratio` for a **non-coplanar** pair. Upstream uses `1.4`, i.e.
    ///   slip on one plane obstructs a system on another plane 40 % more than
    ///   it obstructs its own coplanar neighbours — forest dislocations cut by
    ///   a gliding dislocation are the stronger obstacle.
    ///
    /// Coplanarity is decided by `|n_a . n_b| > 1 - 1e-9`, which treats `n` and
    /// `-n` as the same plane, as it must.
    ///
    /// Setting both arguments equal gives pure isotropic (Taylor) hardening;
    /// `latent_ratio = 0` with `self_ratio = 1` gives pure self hardening.
    ///
    /// # Units
    ///
    /// Both arguments and every entry are dimensionless ratios.
    #[must_use]
    pub fn latent_hardening_matrix(
        self,
        self_ratio: f64,
        latent_ratio: f64,
    ) -> [[f64; MAX_SLIP_SYSTEMS]; MAX_SLIP_SYSTEMS] {
        let sys = self.systems();
        let n = self.n_systems();
        let mut q = [[0.0_f64; MAX_SLIP_SYSTEMS]; MAX_SLIP_SYSTEMS];
        for a in 0..n {
            for b in 0..n {
                let coplanar = dot(sys[a].normal, sys[b].normal).abs() > 1.0 - 1.0e-9;
                q[a][b] = if coplanar { self_ratio } else { latent_ratio };
            }
        }
        q
    }
}

/// Dot product of two three-vectors. Units are the product of the arguments'.
#[inline]
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Scale a three-vector to unit length; a zero vector is returned unchanged.
#[inline]
fn normalise(v: [f64; 3]) -> [f64; 3] {
    let n = dot(v, v).sqrt();
    if n == 0.0 {
        v
    } else {
        [v[0] / n, v[1] / n, v[2] / n]
    }
}
