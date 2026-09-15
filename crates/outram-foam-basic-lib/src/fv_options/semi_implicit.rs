// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

// Upstream source: src/fvModels/general/semiImplicitSource/.

//! A general explicit/implicit source on one field.

use super::{CellSelection, SourceContribution};

/// A constant source term with an explicit and an implicit part.
///
/// Upstream's `semiImplicitSource`, the simplest `fvModel` there is, and the
/// one that exercises the mechanism end to end without any physics of its own.
///
/// # Units
///
/// Both parts are **per unit volume**, in the units of the equation's residual;
/// see [`SourceContribution`]. The cell volume is applied when the contribution
/// is placed into the matrix.
#[derive(Debug, Clone, PartialEq)]
pub struct SemiImplicitSource {
    name: String,
    field: String,
    selection: CellSelection,
    contribution: SourceContribution,
}

impl SemiImplicitSource {
    /// A source named `name` on the equation for `field`, over `selection`.
    ///
    /// `explicit` is added to the right-hand side; `implicit` is the
    /// coefficient of the solved variable, where a **negative** value is a sink
    /// and stabilises the matrix.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        field: impl Into<String>,
        selection: CellSelection,
        explicit: f64,
        implicit: f64,
    ) -> Self {
        Self {
            name: name.into(),
            field: field.into(),
            selection,
            contribution: SourceContribution { explicit, implicit },
        }
    }

    /// The model's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The field whose equation this contributes to.
    #[must_use]
    pub fn field_name(&self) -> &str {
        &self.field
    }

    /// The cells acted on.
    #[must_use]
    pub fn selection(&self) -> &CellSelection {
        &self.selection
    }

    /// The per-unit-volume contribution.
    #[must_use]
    pub fn contribution(&self) -> SourceContribution {
        self.contribution
    }
}
