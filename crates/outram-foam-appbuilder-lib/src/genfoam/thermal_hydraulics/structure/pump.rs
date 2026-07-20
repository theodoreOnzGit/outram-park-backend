// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/phaseModels/structureModels/
//             pump/pump.{C,H}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream authors: Stefan Radman, Carlo Fiorina (FMU part) (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `pump` — structure-borne momentum source
//!
//! Port of GeN-Foam's `pump`. A pump is a body-force term the structure adds to
//! the porous momentum equation over the cells it occupies: a fixed source
//! **vector**, optionally scaled in time by a [`TimeProfile`] (e.g. a coast-down
//! curve on loss of power).
//!
//! ```text
//! momentum_source(t) = base_source * profile(t)
//! ```
//!
//! The source vector has the dimension of the momentum-equation source density,
//! **N / m^3** (equivalently Pa/m — a force per unit volume). It is carried as a
//! basic-lib [`Vector3`] to match the raw-`f64` convention of the finite-volume
//! vector fields it is ultimately written into (`Foam::volVectorField`); the
//! unit is documented rather than type-enforced because those fields are
//! themselves untyped. Assembling this source into the momentum equation is the
//! porous-solver bead's job (op-p6p.7.11); this type only reproduces the pump's
//! value-and-time-scaling behaviour, which is self-contained and verifiable.
//!
//! The upstream FMU-multiplier coupling (`pumpMultiplierNameFromFMU`) is not
//! ported — it reads a scalar from an external functional-mock-up unit, out of
//! scope for this crate.

use uom::si::f64::Time;

use outram_foam_basic_lib::prelude::Vector3;

use crate::genfoam::common::TimeProfile;

/// A structure momentum source (pump).
///
/// Construct with [`Pump::new`]; optionally attach a time scaling with
/// [`Pump::with_time_profile`]. Query the instantaneous source with
/// [`Pump::momentum_source`].
#[derive(Debug, Clone, PartialEq)]
pub struct Pump {
    /// Constant momentum-source vector [N/m^3] at the profile reference time.
    base_source: Vector3,
    /// Optional time scaling factor applied to `base_source`.
    time_profile: Option<TimeProfile>,
}

impl Pump {
    /// Build a pump with a constant momentum-source vector [N/m^3].
    #[must_use]
    pub fn new(base_source: Vector3) -> Self {
        Self {
            base_source,
            time_profile: None,
        }
    }

    /// Attach a momentum-source time-scaling profile (a factor relative to the
    /// base source). Consumes and returns `self` for chaining.
    #[must_use]
    pub fn with_time_profile(mut self, profile: TimeProfile) -> Self {
        self.time_profile = Some(profile);
        self
    }

    /// Whether this pump's source varies in time.
    #[must_use]
    pub fn time_dependent(&self) -> bool {
        self.time_profile.is_some()
    }

    /// The momentum-source vector [N/m^3] at time `t`.
    ///
    /// Port of `pump::correct`: `base_source * profile(t)`, or the constant
    /// `base_source` if no profile is attached.
    #[must_use]
    pub fn momentum_source(&self, t: Time) -> Vector3 {
        match &self.time_profile {
            Some(p) => self.base_source * p.value(t),
            None => self.base_source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::time::second;

    fn s(x: f64) -> Time {
        Time::new::<second>(x)
    }

    #[test]
    fn constant_pump_is_time_invariant() {
        let p = Pump::new(Vector3::new(0.0, 0.0, 1000.0));
        assert!(!p.time_dependent());
        let v = p.momentum_source(s(50.0));
        assert_eq!((v.x, v.y, v.z), (0.0, 0.0, 1000.0));
    }

    /// V&V — pump coast-down scales the base source by the profile.
    ///
    /// Methodology: base source (0,0,1000) N/m^3 with a linear coast-down from
    /// factor 1.0 at t=0 to 0.1 at t=1 s. At t=0.5 s the factor is 0.55, so the
    /// source must be (0,0,550) N/m^3.
    ///
    /// Result (2026-07-15): z-component = 550.0 N/m^3, |error| < 1e-9. Pass.
    #[test]
    fn coast_down_scales_source() {
        let p = Pump::new(Vector3::new(0.0, 0.0, 1000.0))
            .with_time_profile(TimeProfile::table(vec![s(0.0), s(1.0)], vec![1.0, 0.1]).unwrap());
        assert!(p.time_dependent());
        let v = p.momentum_source(s(0.5));
        assert!((v.z - 550.0).abs() < 1e-9);
        assert_eq!(v.x, 0.0);
    }
}
