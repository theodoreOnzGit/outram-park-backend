// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream `offbeatLib/rheology/rheology.C/H` and
// `offbeatLib/rheology/rheologyByMaterial.C/H`.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Choosing a constitutive law per material zone.
//!
//! A fuel rod is at least two materials — a ceramic pellet and a metal tube —
//! and they do not obey the same constitutive law. The pellet creeps by
//! diffusion at 1500 K; the tube creeps under neutron flux at 600 K and yields
//! during a power ramp. Upstream handles this by holding one `constitutiveLaw`
//! per entry of its `materials` list, plus a cell-to-material address list, and
//! looping materials on the outside and cells on the inside.
//!
//! This module does the same with one law per material zone and an index per
//! cell.

use std::sync::Arc;

use crate::error::{OffbeatError, Result};

use super::law::ConstitutiveLaw;
use super::state::{RheologyInputs, RheologyState, StressCorrection};

/// One constitutive law per material zone, plus the cell-to-zone map.
///
/// # Threading
///
/// The cell-to-zone map is shared as `Arc<Vec<usize>>` and never mutated, per
/// the workspace rule preferring `Arc<T>` to lifetimes for read-only shared
/// data. Clone this struct freely across threads; the map is not copied.
///
/// # Example
///
/// ```
/// use std::sync::Arc;
/// use outram_park_fork_offbeat::rheology::{
///     ConstitutiveLaw, CreepModel, RheologyByMaterial, YieldStressModel,
///     ZircaloyCladType,
/// };
///
/// // Cells 0 and 1 are fuel, cells 2 and 3 are cladding.
/// let laws = vec![
///     ConstitutiveLaw::MisesPlasticCreep {
///         yield_stress: YieldStressModel::Constant { sigma_y: 1.0e9 },
///         creep: CreepModel::Matpro { sakai_correction: true },
///     },
///     ConstitutiveLaw::MisesPlasticCreep {
///         yield_stress: YieldStressModel::Fraptran,
///         creep: CreepModel::Limback { clad_type: ZircaloyCladType::Sra },
///     },
/// ];
/// let rheology = RheologyByMaterial::new(laws, Arc::new(vec![0, 0, 1, 1])).unwrap();
///
/// assert_eq!(rheology.n_materials(), 2);
/// assert_eq!(rheology.n_cells(), 4);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct RheologyByMaterial {
    laws: Vec<ConstitutiveLaw>,
    cell_material: Arc<Vec<usize>>,
}

impl RheologyByMaterial {
    /// Build from one law per material zone and a per-cell zone index.
    ///
    /// `cell_material[c]` is the index into `laws` for cell `c`.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Mesh`] if `laws` is empty or if any cell names a
    /// material zone that does not exist. Both are case-setup errors,
    /// detectable once at construction rather than per cell per timestep.
    pub fn new(laws: Vec<ConstitutiveLaw>, cell_material: Arc<Vec<usize>>) -> Result<Self> {
        if laws.is_empty() {
            return Err(OffbeatError::Mesh(
                "rheology needs at least one constitutive law".to_string(),
            ));
        }
        if let Some((cell, &material)) = cell_material
            .iter()
            .enumerate()
            .find(|(_, &m)| m >= laws.len())
        {
            return Err(OffbeatError::Mesh(format!(
                "cell {cell} names material zone {material}, but only {} law(s) were supplied",
                laws.len()
            )));
        }
        Ok(Self {
            laws,
            cell_material,
        })
    }

    /// A single law applied uniformly to `n_cells` cells.
    ///
    /// The right constructor for a single-material verification case.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Mesh`] if `n_cells` is zero.
    pub fn uniform(law: ConstitutiveLaw, n_cells: usize) -> Result<Self> {
        if n_cells == 0 {
            return Err(OffbeatError::Mesh(
                "a uniform rheology needs at least one cell".to_string(),
            ));
        }
        Self::new(vec![law], Arc::new(vec![0; n_cells]))
    }

    /// Number of material zones.
    #[must_use]
    pub fn n_materials(&self) -> usize {
        self.laws.len()
    }

    /// Number of cells the zone map covers.
    #[must_use]
    pub fn n_cells(&self) -> usize {
        self.cell_material.len()
    }

    /// The constitutive law governing a cell.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Mesh`] if `cell` is outside the zone map.
    pub fn law(&self, cell: usize) -> Result<&ConstitutiveLaw> {
        let material = *self.cell_material.get(cell).ok_or_else(|| {
            OffbeatError::Mesh(format!(
                "cell {cell} is outside the rheology zone map ({} cells)",
                self.cell_material.len()
            ))
        })?;
        Ok(&self.laws[material])
    }

    /// Integrate the law governing `cell` over one timestep.
    ///
    /// A thin dispatch over [`ConstitutiveLaw::correct`]; see there for the
    /// algorithm, the arguments and the error conditions.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Mesh`] if `cell` is outside the zone map, plus every
    /// error [`ConstitutiveLaw::correct`] can raise.
    pub fn correct(
        &self,
        cell: usize,
        inputs: &RheologyInputs,
        state: &RheologyState,
    ) -> Result<StressCorrection> {
        self.law(cell)?.correct(cell, inputs, state)
    }
}

/// Which rheology driver a case uses.
///
/// Upstream makes `rheology` runtime-selectable but registers exactly one
/// implementation, `byMaterial` — the base class's own `TypeName` is `"none"`
/// and it is abstract. This enum therefore has one variant today. It exists
/// rather than being collapsed to a bare struct so that a second driver (a
/// homogenised or a per-region rheology, say) becomes a compile error at every
/// match site instead of a silent runtime branch, which is the whole point of
/// the workspace's enum-dispatch rule.
#[derive(Debug, Clone, PartialEq)]
pub enum Rheology {
    /// One constitutive law per material zone. Upstream `rheologyByMaterial`.
    ByMaterial(RheologyByMaterial),
}

impl Rheology {
    /// Integrate the law governing `cell` over one timestep.
    ///
    /// # Errors
    ///
    /// See [`RheologyByMaterial::correct`].
    pub fn correct(
        &self,
        cell: usize,
        inputs: &RheologyInputs,
        state: &RheologyState,
    ) -> Result<StressCorrection> {
        match self {
            Self::ByMaterial(r) => r.correct(cell, inputs, state),
        }
    }

    /// Number of cells the driver covers.
    #[must_use]
    pub fn n_cells(&self) -> usize {
        match self {
            Self::ByMaterial(r) => r.n_cells(),
        }
    }
}
