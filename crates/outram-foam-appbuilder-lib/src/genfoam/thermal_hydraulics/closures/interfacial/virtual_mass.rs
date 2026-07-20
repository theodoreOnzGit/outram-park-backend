// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/virtualMassModels/
//             virtualMassCoefficientModels/{virtualMassCoefficientModel.{H,C}}
//             and src/classes/thermalHydraulics/src/physicsModels/templatedModels/
//             constant/{constantModel.{H,C}, constantModels.C} (the
//             `constantVirtualMassCoefficient` instantiation)
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `interfacial::virtual_mass` — virtual (added) mass coefficient
//!
//! Port of GeN-Foam's `virtualMassCoefficientModel` run-time-selectable family.
//!
//! **Why this is a one-variant enum.** `virtualMassCoefficientModel.H`/`.C`
//! declare only the abstract base and its `runTimeSelectionTable` — no concrete
//! subclass ships in `virtualMassModels/`. The only instantiation that actually
//! exists in the upstream tree is the generic `constantModel<scalar,
//! virtualMassCoefficientModel>` template, registered under the name
//! `"constant"` in `templatedModels/constant/constantModels.C` and used by both
//! shipped tutorials (`Tutorials/featureCases/{1D_boiling,2D_KNS37-L22}/constant/
//! fluidRegion/phaseProperties`, e.g. `virtualMassCoefficientModel { type
//! constant; value 0.1; }`). `constantModel::value()` is trivial — it returns
//! the dictionary-read constant unchanged for every cell — so this port is that
//! one real, dictionary-driven model, not a fabricated placeholder. A future
//! non-constant `Cvm` correlation (e.g. Zuber & Findlay's slip-flow form) would
//! be a new closed-enum variant, not a change to this one.
//!
//! `virtualMass::correct()` itself (upstream `virtualMassModels/virtualMass.{H,C}`)
//! assembles the two-fluid added-mass **force** as an `fvVectorMatrix` from
//! `Vm`, the phase velocities, and their fluxes — that is porous-momentum-solver
//! wiring (mesh fields, `fvm::ddt`/`fvm::div`), not a pure closure, and belongs
//! to the solver bead (op-p6p.7.11) once it exists. Only the scalar `Cvm(alpha)`
//! coefficient closure is ported here.

use uom::si::f64::Ratio;
use uom::si::ratio::ratio;

/// Virtual (added) mass coefficient `Cvm` — dimensionless.
///
/// Closed enum port of GeN-Foam's `virtualMassCoefficientModel` family.
/// Evaluate with [`VirtualMassCoefficient::coefficient`]. See the module docs
/// for why [`VirtualMassCoefficient::Constant`] is currently the only variant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VirtualMassCoefficient {
    /// A fixed coefficient read once from the case dictionary (upstream
    /// `virtualMassCoefficientModel { type constant; value <Cvm>; }`; a
    /// theoretical sphere in inviscid potential flow has `Cvm = 0.5`).
    Constant {
        /// The fixed coefficient value (dimensionless).
        cvm: f64,
    },
}

impl VirtualMassCoefficient {
    /// Build a [`VirtualMassCoefficient::Constant`] from a dimensionless `Cvm`.
    #[must_use]
    pub fn constant(cvm: Ratio) -> Self {
        Self::Constant {
            cvm: cvm.get::<ratio>(),
        }
    }

    /// Evaluate the coefficient. Takes no per-cell field arguments: every
    /// currently-shipped upstream model (see module docs) is spatially
    /// uniform, so this is a pure accessor, not yet a correlation.
    #[must_use]
    pub fn coefficient(&self) -> Ratio {
        let Self::Constant { cvm } = *self;
        Ratio::new::<ratio>(cvm)
    }
}
