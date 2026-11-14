// SPDX-License-Identifier: GPL-3.0

//! **A cell's temperature selects the temperature its material is looked up
//! at, as OpenMC does** (GitHub #313).
//!
//! # What upstream does
//!
//! OpenMC resolves temperature at the *cell*: `Cell::sqrtkT_` is set from the
//! cell's `temperature` attribute, and a cell with none takes its material's
//! (`assign_temperatures`, `src/geometry_aux.cpp:170-198`; void gets 0 K).
//! On entering a cell `Particle::sqrtkT()` becomes `cell.sqrtkT(instance)`
//! (`src/geometry.cpp:203-204`), and every cross-section lookup and the
//! free-gas kinematics read that value (`src/nuclide.cpp:633,675`,
//! `src/physics.cpp:738`). So two cells filled with the *same* material may
//! sit at different temperatures.
//!
//! # What this port does, and why it is a lowering
//!
//! Here the lookup temperature is `Material::temperature` and the collision
//! sites that read it (`transport_history_vr` and its drivers) are
//! kovan-reviewed and closed to modification. [`bind_cell_temperatures`]
//! therefore applies the cell temperature **beside** them, the same way
//! [`crate::material::material_sab::assign_material_sab`] applies #462: it
//! returns a geometry and material array in which every material cell whose
//! declared temperature differs from its material's points at a **copy of
//! that material at the cell's temperature** (one copy per
//! `(material, temperature)`, appended after the originals, so existing
//! indices stay valid). Transport then reads the cell temperature through the
//! material, with no change to the kernels.
//!
//! # What it can and cannot do
//!
//! The temperature moves what depends on it: windowed-multipole data (the LOW
//! `Core` tier, and a pointwise nuclide with WMP attached) and the
//! free-gas kT of a multipole nuclide. A pointwise table processed at one
//! temperature stays at that temperature, as in OpenMC when the library holds
//! one temperature (`temperature_method = nearest` then picks that table or
//! refuses outside `temperature_tolerance`; this port does not refuse).
//! Selecting among several tabulated temperatures is
//! [`crate::material::temperature_interpolation`] (GitHub #947), which is a
//! per-collision draw and is not on a transport path.
//!
//! # The default of every `run_keff_csg*` driver (GitHub #313, 2026-11-14)
//!
//! `Cell::temperature` is `Option<f64>` since 2026-11-14: `None` is "unset,
//! take the material's" (OpenMC's rule), `Some(t)` a declaration. The k
//! drivers (`run_keff_csg_seq_progress`, `run_keff_csg_par_progress` and the
//! `CsgPowerIteration` steps) call [`bind_cell_temperatures`] themselves, so
//! the cell temperature is applied unless the caller ablates it with
//! `Geometry::without_cell_temperatures`. ~~Not the default, because
//! `Cell::temperature` was a plain `f64` that every constructor filled with a
//! literal `293.6`.~~ A cell whose `Some(t)` equals its material's, and every
//! `None` cell, is unchanged by a bit (pinned by the test); the model layer
//! (`model::lower`) writes each cell's material temperature into the cell, so a
//! lowered model is unchanged. Pinned by `tests/cell_temperature_is_default.rs`.
//! The binding clones the geometry and material array only when some cell
//! disagrees with its material.

use crate::geometry::cell::CellFill;
use crate::geometry::geometry::Geometry;
use crate::material::material::Material;

/// One material cell whose declared temperature differs from its material's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellTemperatureMismatch {
    /// Index of the cell in `Geometry::cells`.
    pub cell: usize,
    /// Index of its material.
    pub material: usize,
    /// The cell's declared temperature \[K\].
    pub cell_k: f64,
    /// The material's temperature \[K\].
    pub material_k: f64,
}

/// Every material cell whose temperature differs (bitwise) from its
/// material's. Empty when the geometry and materials agree, which is the case
/// for any lowered model.
#[must_use]
pub fn cell_temperature_mismatches(
    geometry: &Geometry,
    materials: &[Material],
) -> Vec<CellTemperatureMismatch> {
    let mut out = Vec::new();
    for (i, c) in geometry.cells.iter().enumerate() {
        if let CellFill::Material(m) = c.fill {
            if let Some(mat) = materials.get(m) {
                // `None` is "unset": the cell takes its material's temperature.
                if let Some(t) = c.temperature {
                    if t.to_bits() != mat.temperature.to_bits() {
                        out.push(CellTemperatureMismatch {
                            cell: i,
                            material: m,
                            cell_k: t,
                            material_k: mat.temperature,
                        });
                    }
                }
            }
        }
    }
    out
}

/// Apply every material cell's temperature (see the module docs). Returns
/// `Ok(None)` when no cell disagrees with its material, so the caller keeps
/// its originals and nothing changes by a bit; otherwise the new geometry and
/// material arrays.
///
/// # Errors
///
/// A material cell whose temperature is not finite and `>= 0`, or whose
/// material index is out of range. OpenMC rejects a negative cell temperature
/// (`src/cell.cpp`, "Cell temperature must be non-negative").
pub fn bind_cell_temperatures(
    geometry: &Geometry,
    materials: &[Material],
) -> Result<Option<(Geometry, Vec<Material>)>, String> {
    let mismatches = cell_temperature_mismatches(geometry, materials);
    for (i, c) in geometry.cells.iter().enumerate() {
        if let CellFill::Material(m) = c.fill {
            if m >= materials.len() {
                return Err(format!("cell {i} references material {m}, out of range"));
            }
            if let Some(t) = c.temperature {
                if !(t.is_finite() && t >= 0.0) {
                    return Err(format!(
                        "cell {i} (id {}) has temperature {} K; it must be finite and >= 0",
                        c.id, t
                    ));
                }
            }
        }
    }
    if mismatches.is_empty() {
        return Ok(None);
    }
    let mut geom = geometry.clone();
    let mut mats = materials.to_vec();
    // (material, temperature bits) -> index of its copy.
    let mut copies: Vec<((usize, u64), usize)> = Vec::new();
    for mm in &mismatches {
        let key = (mm.material, mm.cell_k.to_bits());
        let idx = match copies.iter().find(|(k, _)| *k == key) {
            Some((_, i)) => *i,
            None => {
                let i = mats.len();
                let mut copy = materials[mm.material].clone();
                copy.temperature = mm.cell_k;
                mats.push(copy);
                copies.push((key, i));
                i
            }
        };
        geom.cells[mm.cell].fill = CellFill::Material(idx);
    }
    Ok(Some((geom, mats)))
}

/// The geometry and materials a driver should transport through: the cell
/// temperatures applied ([`bind_cell_temperatures`]), or the caller's own
/// when nothing disagrees. Panics with upstream's message on an invalid cell
/// temperature (OpenMC `fatal_error`s there).
#[must_use]
pub fn with_cell_temperatures(
    geometry: &Geometry,
    materials: &[Material],
) -> Option<(Geometry, Vec<Material>)> {
    bind_cell_temperatures(geometry, materials).unwrap_or_else(|e| panic!("{e}"))
}
