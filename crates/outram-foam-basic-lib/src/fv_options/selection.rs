// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

// Upstream source: src/finiteVolume/cfdTools/general/fvCellZone/ and the
// `cellSetOption` base of src/fvModels/.

//! Which cells a source term applies over.

use std::sync::Arc;

/// The set of cells a source term acts on.
///
/// Upstream's `fvCellZone` / `cellSetOption`. A source is almost never global:
/// a heat source occupies one component, a porous drag one region, a melting
/// model one material. This is how that restriction is expressed.
///
/// # Sharing
///
/// The cell list is held behind an `Arc`, per the workspace rule against
/// lifetime parameters, so several models can share one selection without
/// copying it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CellSelection {
    /// Every cell in the mesh.
    All,
    /// An explicit list of cell indices.
    ///
    /// Indices are not validated against a mesh here, because a selection is
    /// built before it meets one. An out-of-range index will panic on use,
    /// which is the correct outcome for what is a case-setup error.
    Zone(Arc<Vec<usize>>),
}

impl CellSelection {
    /// A selection from an explicit list of cell indices.
    #[must_use]
    pub fn zone(cells: Vec<usize>) -> Self {
        Self::Zone(Arc::new(cells))
    }

    /// The selected cells, given a mesh cell count.
    ///
    /// `n_cells` is used only to expand [`CellSelection::All`].
    #[must_use]
    pub fn cells(&self, n_cells: usize) -> Vec<usize> {
        match self {
            Self::All => (0..n_cells).collect(),
            Self::Zone(c) => c.as_ref().clone(),
        }
    }

    /// Number of selected cells, given a mesh cell count.
    #[must_use]
    pub fn len(&self, n_cells: usize) -> usize {
        match self {
            Self::All => n_cells,
            Self::Zone(c) => c.len(),
        }
    }

    /// Whether the selection is empty.
    ///
    /// An empty selection is legal — it contributes nothing — but is usually a
    /// case-setup mistake worth reporting.
    #[must_use]
    pub fn is_empty(&self, n_cells: usize) -> bool {
        self.len(n_cells) == 0
    }
}
