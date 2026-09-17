// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
//   `offbeatLib/sliceMapper/sliceMapper.{H,C}`              (the `none` base),
//   `offbeatLib/sliceMapper/sliceMapperByMaterial.{H,C}`    (`calcAddressing`),
//   `offbeatLib/sliceMapper/sliceMapperByPellets.{H,C}`     (`calcAddressing`),
//   `offbeatLib/sliceMapper/sliceMapperAutoAxialSlices.{H,C}` (`calcAddressing`),
//   `offbeatLib/sliceMapper/sliceMapperTemplates.H`         (`sliceAverage`).
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Axial slicing — the 1.5D / 2D / 3D mapping layer.
//!
//! # What a slice mapper is for
//!
//! A fuel rod is a long thin object, and much of fuel-performance physics is
//! **one-dimensional per axial level**: fission-gas release, relocation, the
//! FRAPCON gap-closure correlation and the axial power profile are all stated as
//! functions of a *slice-averaged* burnup, temperature or linear power. But the
//! mesh a case is solved on may be 1.5D (a stack of independent radial columns),
//! 2D (an r–z axisymmetric mesh) or full 3D.
//!
//! A slice mapper is what lets one implementation of those correlations serve
//! all three. It **partitions the cells into axial slices** and provides
//! volume-weighted averages over each. In 1.5D the partition is trivial — one
//! slice per column — and in 2D or 3D it collapses a whole ring or disc of cells
//! onto one number. The correlation itself never learns which mesh it is on.
//!
//! That is the whole concept, and it is why this module exists in the [`gap`
//! module](super): the gap models consume slice-averaged quantities
//! (relocation strain per pellet, gas temperature per axial level), and a
//! difference in slicing changes the gap history.
//!
//! # The three strategies
//!
//! [`AxialSlicing`] has one variant per upstream `sliceMapper`:
//!
//! | Variant | Upstream | How it slices |
//! |---|---|---|
//! | [`None`](AxialSlicing::None) | `sliceMapper` (`"none"`) | No slices at all. Upstream warns that models depending on a mapper — relocation among them — will conflict with this. |
//! | [`ByMaterial`](AxialSlicing::ByMaterial) | `sliceMapperByMaterial` | A fixed number of slices per material, of equal height or of explicitly-listed heights. |
//! | [`ByPellets`](AxialSlicing::ByPellets) | `sliceMapperByPellets` | One slice per **pellet** — heights derived by dividing the material height by the pellet count. |
//! | [`AutoAxial`](AxialSlicing::AutoAxial) | `sliceMapperAutoAxialSlices` | One slice per **distinct mesh axial level**, found by rounding cell-centre coordinates to a precision and grouping equal values. |
//!
//! # Gap conventions
//!
//! Nothing in this module is a gap width, so the radial/diametral question does
//! not arise. The one convention that does: **`axial_coordinate` is a length
//! \[m\] measured along the pin direction**, and `height_above_bottom` is that
//! coordinate minus the material's lowest point, so it is always `>= 0` for a
//! cell inside the material.
//!
//! # Deferred
//!
//! Upstream's slice mappers are half arithmetic and half mesh traversal. The
//! arithmetic is ported; the traversal is not:
//!
//! - **Cell-to-material addressing** (`mat_.matAddrList()`) — which cells belong
//!   to which material. Taken as an input: the caller passes the coordinates of
//!   the cells it wants sliced.
//! - **The material extent** `h_min`, `h_max`, which upstream finds by walking
//!   every *point* of every cell (`mesh_.cellPoints()`), not the cell centres.
//!   [`AxialSlicing::assign`] takes `h_min` as an argument for that reason — it
//!   is deliberately *not* the minimum of the coordinates passed in, because the
//!   bottom cell's centre is above the material's bottom face.
//! - **The `isFuel` tagging** (`isA<fuelMaterial>`), the `sliceID` debug
//!   `volScalarField`, and the `topoChanging()` re-addressing trigger.
//! - **The parallel `Pstream` gather/scatter** that reconciles slice
//!   identities across processors, and the `reduce(sizeI, sumOp)` empty-slice
//!   check that depends on it. [`SliceAverage`] reports empty slices instead of
//!   aborting.
//!
//! # Units
//!
//! Strict SI raw `f64`: metre for coordinates and heights, m³ for cell volumes.
//! Averaged quantities carry whatever unit the caller's values carry.

use outram_foam_basic_lib::primitives::Vector3;

use crate::error::{OffbeatError, Result};

/// Absolute tolerance \[m\] in the slice-boundary test — upstream's literal
/// `1e-6` in `sliceMapperByMaterial::calcAddressing` and
/// `sliceMapperByPellets::calcAddressing`.
///
/// The test is `cumulative_height >= height_above_bottom + 1e-6`, so a cell
/// centre lying **exactly** on a slice boundary is assigned to the slice
/// **above** it, and a cell centre within 1 µm below a boundary is too. It is an
/// absolute length, not a relative one, so its effect depends on the rod's
/// absolute dimensions — for a 1 cm pellet it is a 0.01% band.
pub const SLICE_BOUNDARY_TOLERANCE: f64 = 1.0e-6;

/// Axial coordinate \[m\] of a point along the pin direction — upstream's
/// `mesh_.C() & pinDirection_`.
///
/// A plain dot product. `pin_direction` is expected to be a **unit** vector
/// along the rod axis (upstream reads it from its `globalOptions`); if it is
/// not, every coordinate is scaled by its magnitude and the slice heights no
/// longer mean metres. This function does not normalise, matching upstream — use
/// [`axial_coordinate_checked`] to be told.
#[must_use]
pub fn axial_coordinate(position: Vector3, pin_direction: Vector3) -> f64 {
    position.dot(pin_direction)
}

/// [`axial_coordinate`], rejecting a non-unit pin direction.
///
/// # Errors
///
/// [`OffbeatError::Unphysical`] if `|pin_direction| − 1` exceeds 1e-9. A pin
/// direction that is not a unit vector silently rescales every axial coordinate,
/// which produces slices of the wrong height rather than an obvious failure.
pub fn axial_coordinate_checked(position: Vector3, pin_direction: Vector3) -> Result<f64> {
    let magnitude = pin_direction.dot(pin_direction).sqrt();
    if (magnitude - 1.0).abs() > 1.0e-9 {
        return Err(OffbeatError::Unphysical {
            quantity: "pin direction magnitude",
            value: magnitude,
            unit: "-",
            reason: "must be a unit vector; otherwise every axial coordinate is \
                     rescaled and the slice heights stop meaning metres",
        });
    }
    Ok(position.dot(pin_direction))
}

/// How the cells of one material are partitioned into axial slices — one variant
/// per upstream `sliceMapper` implementation.
///
/// Dispatch is by `match`, never by a trait object, per the workspace
/// `CLAUDE.md` "No trait objects" rule.
#[derive(Debug, Clone, PartialEq)]
pub enum AxialSlicing {
    /// No slicing — upstream `sliceMapper`, `TypeName("none")`.
    ///
    /// [`assign`](Self::assign) returns `None` for every cell and
    /// [`n_slices`](Self::n_slices) returns `Some(0)`.
    ///
    /// Selecting this is a real modelling decision, not a null one: upstream
    /// warns that models depending on a mapper will conflict with it. The
    /// relocation model in particular is stated per axial slice, so without a
    /// mapper it has no linear power to branch on.
    None,

    /// A fixed set of slices per material — upstream `sliceMapperByMaterial`,
    /// `TypeName("byMaterial")`.
    ///
    /// Built either from a slice count (equal heights, upstream's `nSlices`) or
    /// from an explicit list (upstream's `heightSlices`). Use
    /// [`by_material_uniform`](Self::by_material_uniform) for the former.
    ByMaterial {
        /// Height \[m\] of each slice, bottom to top. Must all be `> 0`, and
        /// upstream additionally requires their sum to equal the material height
        /// to within 1e-6 m — checked by [`validate`](Self::validate).
        slice_heights: Vec<f64>,
    },

    /// One slice per pellet — upstream `sliceMapperByPellets`,
    /// `TypeName("byPellets")`.
    ///
    /// Structurally identical to [`ByMaterial`](Self::ByMaterial) — upstream
    /// derives equal pellet heights as `material_height / nPellets` — but it
    /// **differs in what happens to a cell that falls in no bin**; see
    /// [`assign`](Self::assign). Kept as a separate variant so that difference
    /// cannot be lost.
    ByPellets {
        /// Height \[m\] of each pellet, bottom to top.
        pellet_heights: Vec<f64>,
    },

    /// One slice per distinct mesh axial level — upstream
    /// `sliceMapperAutoAxialSlices`, `TypeName("autoAxialSlices")`.
    ///
    /// Cell-centre axial coordinates are rounded to `precision` and cells
    /// sharing a rounded value form a slice; slices are ordered by increasing
    /// coordinate. The slice count is therefore a property of the **mesh**, not
    /// of this configuration, which is why [`n_slices`](Self::n_slices) returns
    /// `None` for this variant and why [`assign`](Self::assign) must be used to
    /// discover it.
    ///
    /// This is the natural choice for a 1.5D mesh, where the axial levels are
    /// already exactly the slices wanted.
    AutoAxial {
        /// Rounding precision \[m\] — upstream's `precision`, default `1e-6`.
        ///
        /// Coordinates are grouped by `round(z / precision) · precision`. Too
        /// coarse and distinct mesh levels merge; too fine and floating-point
        /// noise splits one level into several.
        precision: f64,
    },
}

impl AxialSlicing {
    /// Equal-height slices over a material of total height `material_height`
    /// \[m\] — upstream `byMaterial` with only `nSlices` given.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a zero slice count or a non-positive
    /// material height.
    pub fn by_material_uniform(n_slices: usize, material_height: f64) -> Result<Self> {
        Ok(Self::ByMaterial {
            slice_heights: uniform_heights(n_slices, material_height, "slices")?,
        })
    }

    /// Equal-height pellets over a material of total height `material_height`
    /// \[m\] — upstream `byPellets`, which always divides equally.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a zero pellet count or a non-positive
    /// material height.
    pub fn by_pellets_uniform(n_pellets: usize, material_height: f64) -> Result<Self> {
        Ok(Self::ByPellets {
            pellet_heights: uniform_heights(n_pellets, material_height, "pellets")?,
        })
    }

    /// Number of slices, when it is a property of the configuration.
    ///
    /// `Some(0)` for [`None`](Self::None), `Some(n)` for the two height-list
    /// variants, and **`None` for [`AutoAxial`](Self::AutoAxial)** — whose count
    /// depends on the mesh and is only known after [`assign`](Self::assign) has
    /// run.
    #[must_use]
    pub fn n_slices(&self) -> Option<usize> {
        match self {
            Self::None => Some(0),
            Self::ByMaterial { slice_heights } => Some(slice_heights.len()),
            Self::ByPellets { pellet_heights } => Some(pellet_heights.len()),
            Self::AutoAxial { .. } => Option::None,
        }
    }

    /// Total height \[m\] the slice list spans, or `None` for the variants that
    /// do not know it.
    #[must_use]
    pub fn total_height(&self) -> Option<f64> {
        match self {
            Self::None | Self::AutoAxial { .. } => Option::None,
            Self::ByMaterial { slice_heights } => Some(slice_heights.iter().sum()),
            Self::ByPellets { pellet_heights } => Some(pellet_heights.iter().sum()),
        }
    }

    /// Slice index of a single point, given its height above the bottom of the
    /// material \[m\].
    ///
    /// Reproduces upstream's cumulative-height search exactly:
    ///
    /// ```text
    /// total = 0
    /// for m in 0..n:
    ///     total += height[m]
    ///     if total >= height_above_bottom + 1e-6:  return m
    /// ```
    ///
    /// # Boundary behaviour
    ///
    /// A point lying **exactly on** a slice boundary, or within
    /// [`SLICE_BOUNDARY_TOLERANCE`] below it, is assigned to the slice
    /// **above**. See that constant.
    ///
    /// # Returns `None` when no bin matches
    ///
    /// That happens only for a point at or above the top of the last slice — in
    /// practice a degenerate input, because a cell *centre* is always strictly
    /// inside the material. **[`assign`](Self::assign) is where the two
    /// upstream fallbacks for this case differ**; this function reports the
    /// no-match honestly and lets the caller choose.
    #[must_use]
    pub fn slice_index(&self, height_above_bottom: f64) -> Option<usize> {
        let heights = match self {
            Self::None | Self::AutoAxial { .. } => return Option::None,
            Self::ByMaterial { slice_heights } => slice_heights,
            Self::ByPellets { pellet_heights } => pellet_heights,
        };
        if !height_above_bottom.is_finite() {
            return Option::None;
        }
        let mut total = 0.0;
        for (m, h) in heights.iter().enumerate() {
            total += h;
            if total >= height_above_bottom + SLICE_BOUNDARY_TOLERANCE {
                return Some(m);
            }
        }
        Option::None
    }

    /// Assign every cell to a slice.
    ///
    /// # Arguments
    ///
    /// - `axial_coordinates` — the cells' axial coordinates \[m\] along the pin
    ///   direction, from [`axial_coordinate`]. One entry per cell of the
    ///   material being sliced.
    /// - `material_bottom` — the material's **lowest point** `h_min` \[m\].
    ///   Upstream finds it by walking every cell *vertex*, not the cell centres,
    ///   so it is genuinely lower than `min(axial_coordinates)` — which is why
    ///   it is an argument and not derived here. See [Deferred](self#deferred).
    ///
    /// # Returns
    ///
    /// One `Option<usize>` per cell: `Some(slice)` or `None` where no slice
    /// matched.
    ///
    /// # Upstream defect reproduced deliberately: the two fallbacks disagree
    ///
    /// When the cumulative-height search finds no bin, upstream's two mappers do
    /// **different** things:
    ///
    /// - `sliceMapperByMaterial` leaves the cell's local slice ID at its
    ///   initialiser `-1`, and `invertOneToMany` then silently **drops** the
    ///   cell from every slice's addressing. It contributes to no average.
    /// - `sliceMapperByPellets` leaves `currentSliceID` at its initialiser `0`
    ///   and appends the cell to slice **0 — the bottom pellet**, however high up
    ///   the rod it actually is.
    ///
    /// The second is the more damaging: a cell silently teleported to the bottom
    /// of the rod corrupts that slice's average rather than merely being absent
    /// from it. Both are reproduced — [`ByMaterial`](Self::ByMaterial) yields
    /// `None` and [`ByPellets`](Self::ByPellets) yields `Some(0)` — and both are
    /// pinned in this module's tests.
    ///
    /// # AutoAxial
    ///
    /// For [`AutoAxial`](Self::AutoAxial) the slices *are* the distinct rounded
    /// coordinates, so every cell always matches and `material_bottom` is
    /// unused.
    #[must_use]
    pub fn assign(&self, axial_coordinates: &[f64], material_bottom: f64) -> Vec<Option<usize>> {
        match self {
            Self::None => vec![Option::None; axial_coordinates.len()],

            Self::ByMaterial { .. } => axial_coordinates
                .iter()
                .map(|z| self.slice_index(z - material_bottom))
                .collect(),

            Self::ByPellets { .. } => axial_coordinates
                .iter()
                .map(|z| {
                    // Upstream's `currentSliceID` is initialised to 0 and the
                    // loop simply never overwrites it when nothing matches.
                    Some(self.slice_index(z - material_bottom).unwrap_or(0))
                })
                .collect(),

            Self::AutoAxial { precision } => {
                let levels = auto_axial_levels(axial_coordinates, *precision);
                axial_coordinates
                    .iter()
                    .map(|z| {
                        let rounded = round_to(*z, *precision);
                        levels
                            .iter()
                            .position(|l| *l == rounded)
                            .map(Some)
                            .unwrap_or(Option::None)
                    })
                    .collect()
            }
        }
    }

    /// Reject a configuration upstream would have aborted on.
    ///
    /// `material_height` \[m\] is the extent the slices must cover; pass `None`
    /// to skip that check (upstream only makes it when `heightSlices` is given
    /// explicitly).
    ///
    /// # Errors
    ///
    /// - [`OffbeatError::Unphysical`] for an empty height list, a non-positive
    ///   slice height, or a non-positive `AutoAxial` precision.
    /// - [`OffbeatError::Mesh`] if the heights do not sum to `material_height`
    ///   within 1e-6 m — upstream's own tolerance and its own fatal error
    ///   ("The sum of slice heights … differs from calculated material height").
    pub fn validate(&self, material_height: Option<f64>) -> Result<()> {
        let (heights, what) = match self {
            Self::None => return Ok(()),
            Self::AutoAxial { precision } => {
                if !(*precision > 0.0) || !precision.is_finite() {
                    return Err(OffbeatError::Unphysical {
                        quantity: "auto-axial slicing precision",
                        value: *precision,
                        unit: "m",
                        reason: "must be finite and strictly positive",
                    });
                }
                return Ok(());
            }
            Self::ByMaterial { slice_heights } => (slice_heights, "slice"),
            Self::ByPellets { pellet_heights } => (pellet_heights, "pellet"),
        };

        if heights.is_empty() {
            return Err(OffbeatError::Unphysical {
                quantity: "number of axial slices",
                value: 0.0,
                unit: "-",
                reason: "must be at least one; upstream aborts on an empty slice",
            });
        }
        for h in heights {
            if !(*h > 0.0) || !h.is_finite() {
                return Err(OffbeatError::Unphysical {
                    quantity: "axial slice height",
                    value: *h,
                    unit: "m",
                    reason: "must be finite and strictly positive",
                });
            }
        }
        if let Some(expected) = material_height {
            let sum: f64 = heights.iter().sum();
            if (sum - expected).abs() > 1.0e-6 {
                return Err(OffbeatError::Mesh(format!(
                    "sum of {what} heights ({sum} m) differs from the material height \
                     ({expected} m) by more than 1e-6 m"
                )));
            }
        }
        Ok(())
    }
}

/// Build `n` equal heights spanning `total`, or report why not.
fn uniform_heights(n: usize, total: f64, what: &'static str) -> Result<Vec<f64>> {
    if n == 0 {
        return Err(OffbeatError::Unphysical {
            quantity: "number of axial slices",
            value: 0.0,
            unit: "-",
            reason: "must be at least one",
        });
    }
    if !(total > 0.0) || !total.is_finite() {
        let _ = what;
        return Err(OffbeatError::Unphysical {
            quantity: "material height",
            value: total,
            unit: "m",
            reason: "must be finite and strictly positive",
        });
    }
    Ok(vec![total / n as f64; n])
}

/// Round `z` \[m\] to the nearest multiple of `precision` \[m\] — upstream's
/// `round(Cz/precision_)*precision_`.
///
/// Rust's `f64::round` and C++'s `std::round` both round halves away from zero,
/// so this matches upstream bit-for-bit. Returns `z` unchanged for a
/// non-positive or non-finite precision.
#[must_use]
pub fn round_to(z: f64, precision: f64) -> f64 {
    if !(precision > 0.0) || !precision.is_finite() || !z.is_finite() {
        return z;
    }
    (z / precision).round() * precision
}

/// The distinct rounded axial levels \[m\] present in `axial_coordinates`,
/// sorted ascending — upstream's `sortedAxialLocationList` in
/// `sliceMapperAutoAxialSlices::calcAddressing`.
///
/// Each level becomes one slice, so the length of the result is the slice count
/// for [`AxialSlicing::AutoAxial`]. Non-finite coordinates are dropped.
///
/// # Deferred
///
/// Upstream builds this list per processor and reconciles it with a
/// `Pstream::gatherList`/`scatterList` pair, so that a level present on only one
/// processor still exists (empty) on the others. This function is the
/// single-processor case.
#[must_use]
pub fn auto_axial_levels(axial_coordinates: &[f64], precision: f64) -> Vec<f64> {
    let mut levels: Vec<f64> = axial_coordinates
        .iter()
        .filter(|z| z.is_finite())
        .map(|z| round_to(*z, precision))
        .collect();
    levels.sort_by(|a, b| {
        a.partial_cmp(b)
            .expect("non-finite coordinates were filtered out")
    });
    levels.dedup();
    levels
}

/// Volume-weighted averages of one field over the axial slices — upstream's
/// `sliceMapper::sliceAverage<Type>()`.
///
/// ```text
/// avg[s] = Σ_{i ∈ s} value_i · V_i  /  Σ_{i ∈ s} V_i
/// ```
///
/// Volume weighting, not cell counting: a 2D or 3D mesh has cells of very
/// different sizes in one slice (an outer ring holds far more material than the
/// central one), and a cell-count average would over-weight the centre of the
/// pellet, where the temperature is highest.
///
/// # Units
///
/// [`means`](Self::means) carry whatever unit the input values carried;
/// [`slice_volume`](Self::slice_volume) is m³.
#[derive(Debug, Clone, PartialEq)]
pub struct SliceAverage {
    means: Vec<f64>,
    volumes: Vec<f64>,
}

impl SliceAverage {
    /// Compute the per-slice volume-weighted averages.
    ///
    /// # Arguments
    ///
    /// - `values` — the quantity to average, one entry per cell, in any unit.
    /// - `volumes` — cell volumes \[m³\], one entry per cell, all `>= 0`.
    /// - `slice_ids` — the assignment from [`AxialSlicing::assign`]. A `None`
    ///   entry means the cell belongs to no slice and is **excluded** from every
    ///   average, which is upstream `byMaterial`'s behaviour for an unmatched
    ///   cell.
    /// - `n_slices` — the number of slices, which sets the length of the result.
    ///
    /// # Errors
    ///
    /// - [`OffbeatError::Mesh`] if the three input slices differ in length, or
    ///   if a slice ID is `>= n_slices`.
    /// - [`OffbeatError::Unphysical`] for a negative or non-finite cell volume.
    ///
    /// # Empty slices
    ///
    /// A slice containing no cells (or only zero-volume ones) gets a mean of
    /// `0.0` and a volume of `0.0`. Upstream instead aborts with *"Found empty
    /// slice … Check the keyword \"nSlices\""*; this port reports it through
    /// [`empty_slices`](Self::empty_slices) so a caller can decide, because an
    /// empty slice is a configuration error in a solver run but a perfectly
    /// ordinary condition in a unit test.
    pub fn compute(
        values: &[f64],
        volumes: &[f64],
        slice_ids: &[Option<usize>],
        n_slices: usize,
    ) -> Result<Self> {
        if values.len() != volumes.len() || values.len() != slice_ids.len() {
            return Err(OffbeatError::Mesh(format!(
                "slice averaging needs matching lengths: {} values, {} volumes, {} slice ids",
                values.len(),
                volumes.len(),
                slice_ids.len()
            )));
        }

        let mut weighted = vec![0.0; n_slices];
        let mut totals = vec![0.0; n_slices];

        for (i, id) in slice_ids.iter().enumerate() {
            let Some(s) = id else { continue };
            if *s >= n_slices {
                return Err(OffbeatError::Mesh(format!(
                    "cell {i} is assigned to slice {s}, but only {n_slices} slices exist"
                )));
            }
            let v = volumes[i];
            if !(v >= 0.0) || !v.is_finite() {
                return Err(OffbeatError::Unphysical {
                    quantity: "cell volume",
                    value: v,
                    unit: "m^3",
                    reason: "must be finite and non-negative",
                });
            }
            weighted[*s] += values[i] * v;
            totals[*s] += v;
        }

        let means = weighted
            .iter()
            .zip(totals.iter())
            .map(|(w, v)| if *v > 0.0 { w / v } else { 0.0 })
            .collect();

        Ok(Self {
            means,
            volumes: totals,
        })
    }

    /// The per-slice averages, bottom to top.
    #[must_use]
    pub fn means(&self) -> &[f64] {
        &self.means
    }

    /// The per-slice total volumes \[m³\], bottom to top.
    #[must_use]
    pub fn slice_volumes(&self) -> &[f64] {
        &self.volumes
    }

    /// Number of slices.
    #[must_use]
    pub fn n_slices(&self) -> usize {
        self.means.len()
    }

    /// Average over slice `s`, or `None` if `s` is out of range.
    #[must_use]
    pub fn mean(&self, s: usize) -> Option<f64> {
        self.means.get(s).copied()
    }

    /// Total volume \[m³\] of slice `s`, or `None` if `s` is out of range.
    #[must_use]
    pub fn slice_volume(&self, s: usize) -> Option<f64> {
        self.volumes.get(s).copied()
    }

    /// Indices of slices that received no volume — the condition upstream treats
    /// as a fatal configuration error.
    ///
    /// An empty result means every slice has cells. A non-empty one almost
    /// always means the slice count exceeds the mesh's axial divisions.
    #[must_use]
    pub fn empty_slices(&self) -> Vec<usize> {
        self.volumes
            .iter()
            .enumerate()
            .filter(|(_, v)| **v <= 0.0)
            .map(|(i, _)| i)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference-checked against upstream's own literal tolerance and default.
    ///
    /// **Methodology.** `sliceMapperByMaterial.C` line 198 and
    /// `sliceMapperByPellets.C` line 133 both test
    /// `totalHeight >= deltaH + 1e-6`; `sliceMapperAutoAxialSlices.C` line 182
    /// defaults `precision` to `1e-6`. Asserted bitwise.
    ///
    /// **Result** (2026-07-29): both match upstream.
    #[test]
    fn constants_match_upstream_literals() {
        assert_eq!(SLICE_BOUNDARY_TOLERANCE, 1.0e-6);
        // The auto-axial default, for the record.
        let auto = AxialSlicing::AutoAxial { precision: 1.0e-6 };
        assert!(auto.validate(Option::None).is_ok());
    }

    /// Self-consistency check — uniform slicing bins cells in order.
    ///
    /// **Methodology.** A 0.1 m material cut into 10 equal 0.01 m slices. Ten
    /// cell centres at the slice midpoints (0.005, 0.015, …) must map to slices
    /// 0…9 in order.
    ///
    /// **Result** (2026-07-29): all ten mapped to their own slice.
    #[test]
    fn uniform_slicing_bins_midpoints_in_order() {
        let slicing = AxialSlicing::by_material_uniform(10, 0.1).unwrap();
        slicing.validate(Some(0.1)).unwrap();
        assert_eq!(slicing.n_slices(), Some(10));

        let centres: Vec<f64> = (0..10).map(|i| 0.005 + 0.01 * i as f64).collect();
        let ids = slicing.assign(&centres, 0.0);
        for (i, id) in ids.iter().enumerate() {
            assert_eq!(*id, Some(i), "cell {i} landed in {id:?}");
        }
    }

    /// Self-consistency check — a point exactly on a boundary goes to the slice
    /// above.
    ///
    /// **Methodology.** With 0.01 m slices, the boundary between slices 0 and 1
    /// is at 0.01 m. Upstream's `total >= deltaH + 1e-6` gives `0.01 >= 0.01001`
    /// = false for slice 0, so the point falls through to slice 1. Also checked
    /// 0.5 µm below the boundary (still slice 1, inside the tolerance band) and
    /// 2 µm below (slice 0).
    ///
    /// **Result** (2026-07-29, measured): exactly 0.01 m → slice 1;
    /// 0.0099995 m → slice 1; 0.009998 m → slice 0. The 1 µm tolerance band sits
    /// **below** each boundary, as the inequality requires.
    #[test]
    fn boundary_points_go_to_the_slice_above() {
        let slicing = AxialSlicing::by_material_uniform(10, 0.1).unwrap();
        assert_eq!(slicing.slice_index(0.01), Some(1));
        assert_eq!(slicing.slice_index(0.01 - 0.5e-6), Some(1));
        assert_eq!(slicing.slice_index(0.01 - 2.0e-6), Some(0));
        assert_eq!(slicing.slice_index(0.0), Some(0));
    }

    /// Reproduced upstream defect — the two mappers disagree about an unmatched
    /// cell.
    ///
    /// **Methodology.** Place a point above the top of the last slice, so the
    /// cumulative-height search finds no bin. `sliceMapperByMaterial` leaves the
    /// local ID at `-1` and `invertOneToMany` drops the cell;
    /// `sliceMapperByPellets` leaves `currentSliceID` at `0` and appends the cell
    /// to the **bottom** pellet. Assert this port reproduces both.
    ///
    /// **Result** (2026-07-29, measured): for a point at 0.2 m above the bottom
    /// of a 0.1 m material, `ByMaterial` yielded `None` (cell dropped) and
    /// `ByPellets` yielded `Some(0)` — a cell at the top of the rod attributed
    /// to the bottom pellet. The second is a genuine upstream bug, reproduced
    /// deliberately so that a comparison against OFFBEAT matches.
    #[test]
    fn unmatched_cells_are_dropped_by_material_but_sent_to_slice_zero_by_pellets() {
        let by_material = AxialSlicing::by_material_uniform(10, 0.1).unwrap();
        let by_pellets = AxialSlicing::by_pellets_uniform(10, 0.1).unwrap();

        // A point above the top of the material: no bin matches.
        let above = [0.2];
        assert_eq!(by_material.assign(&above, 0.0), vec![Option::None]);
        assert_eq!(by_pellets.assign(&above, 0.0), vec![Some(0)]);

        // The shared, honest primitive reports the no-match for both.
        assert_eq!(by_material.slice_index(0.2), Option::None);
        assert_eq!(by_pellets.slice_index(0.2), Option::None);
    }

    /// Self-consistency check — explicit slice heights are respected and their
    /// sum is checked against the material height.
    ///
    /// **Methodology.** Three unequal slices (0.02, 0.05, 0.03 m) spanning
    /// 0.10 m. A point at 0.06 m must land in slice 1 (which spans 0.02–0.07 m).
    /// A height list summing to something else must be rejected with upstream's
    /// own 1e-6 m tolerance.
    ///
    /// **Result** (2026-07-29): 0.06 m → slice 1; a list summing to 0.11 m was
    /// rejected; a list off by 5e-7 m was accepted, matching upstream's
    /// tolerance.
    #[test]
    fn explicit_slice_heights_are_respected_and_checked() {
        let slicing = AxialSlicing::ByMaterial {
            slice_heights: vec![0.02, 0.05, 0.03],
        };
        slicing.validate(Some(0.10)).unwrap();
        assert_eq!(slicing.slice_index(0.01), Some(0));
        assert_eq!(slicing.slice_index(0.06), Some(1));
        assert_eq!(slicing.slice_index(0.09), Some(2));

        let wrong = AxialSlicing::ByMaterial {
            slice_heights: vec![0.02, 0.05, 0.04],
        };
        assert!(wrong.validate(Some(0.10)).is_err());

        // Inside upstream's 1e-6 m tolerance.
        let close = AxialSlicing::ByMaterial {
            slice_heights: vec![0.02, 0.05, 0.03 + 5.0e-7],
        };
        assert!(close.validate(Some(0.10)).is_ok());
    }

    /// Self-consistency check — auto-axial slicing groups cells by rounded
    /// coordinate and orders the slices bottom-up.
    ///
    /// **Methodology.** Four axial levels with three cells each, jittered by
    /// ±1e-9 m — far below the 1e-6 m precision, so each level must collapse to
    /// one slice. Pass criterion: exactly four levels, ordered ascending, and
    /// all three cells of each level sharing a slice ID.
    ///
    /// **Result** (2026-07-29): four slices; every level's three cells shared an
    /// ID; slice order matched coordinate order.
    #[test]
    fn auto_axial_groups_jittered_levels() {
        let mut coords = Vec::new();
        for level in 0..4 {
            let z = 0.01 + 0.02 * level as f64;
            coords.push(z - 1.0e-9);
            coords.push(z);
            coords.push(z + 1.0e-9);
        }
        let slicing = AxialSlicing::AutoAxial { precision: 1.0e-6 };
        let levels = auto_axial_levels(&coords, 1.0e-6);
        assert_eq!(levels.len(), 4, "levels = {levels:?}");
        assert!(levels.windows(2).all(|w| w[0] < w[1]));

        let ids = slicing.assign(&coords, 0.0);
        for level in 0..4 {
            let base = level * 3;
            assert_eq!(ids[base], Some(level));
            assert_eq!(ids[base + 1], Some(level));
            assert_eq!(ids[base + 2], Some(level));
        }
        // The slice count is a property of the mesh, not the configuration.
        assert_eq!(slicing.n_slices(), Option::None);
    }

    /// Self-consistency check — a coarser precision merges levels.
    ///
    /// **Methodology.** The same four levels at 0.02 m spacing, rounded to a
    /// 0.05 m precision, must collapse to fewer distinct values. This is the
    /// failure mode the `precision` entry invites.
    ///
    /// **Result** (2026-07-29, measured): 4 levels at 1e-6 m precision collapse
    /// to 2 at 0.05 m precision.
    #[test]
    fn coarse_precision_merges_axial_levels() {
        let coords: Vec<f64> = (0..4).map(|i| 0.01 + 0.02 * i as f64).collect();
        assert_eq!(auto_axial_levels(&coords, 1.0e-6).len(), 4);
        assert_eq!(auto_axial_levels(&coords, 0.05).len(), 2);
    }

    /// Self-consistency check — averaging is volume-weighted, not cell-count
    /// weighted.
    ///
    /// **Methodology.** One slice, two cells: value 1000 in a 1e-9 m³ cell and
    /// value 500 in a 9e-9 m³ cell. The cell-count mean is 750; the correct
    /// volume-weighted mean is `(1000·1 + 500·9)/10 = 550`. Pass criterion: 550,
    /// tolerance 1e-9. This is the distinction that matters on a 2D or 3D mesh,
    /// where an outer ring holds far more material than an inner one.
    ///
    /// **Result** (2026-07-29): 550.0 exactly.
    #[test]
    fn slice_average_is_volume_weighted() {
        let values = [1000.0, 500.0];
        let volumes = [1.0e-9, 9.0e-9];
        let ids = [Some(0), Some(0)];
        let avg = SliceAverage::compute(&values, &volumes, &ids, 1).unwrap();
        assert!(
            (avg.mean(0).unwrap() - 550.0).abs() < 1e-9,
            "{:?}",
            avg.means()
        );
        assert!((avg.slice_volume(0).unwrap() - 1.0e-8).abs() < 1e-20);
        assert!(avg.empty_slices().is_empty());
    }

    /// Self-consistency check — averaging over a realistic 1.5D stack.
    ///
    /// **Methodology.** Ten axial slices, four radial cells each, with a linear
    /// axial temperature ramp from 600 K to 1500 K applied uniformly across each
    /// slice's radial cells. Each slice's average must then equal its own ramp
    /// value exactly, whatever the radial volumes — because averaging a constant
    /// gives the constant.
    ///
    /// **Result** (2026-07-29): all ten slice averages matched their ramp value
    /// to within 1e-12.
    #[test]
    fn slice_average_recovers_a_constant_within_each_slice() {
        let n_slices = 10;
        let mut values = Vec::new();
        let mut volumes = Vec::new();
        let mut ids = Vec::new();
        for s in 0..n_slices {
            let t = 600.0 + 900.0 * s as f64 / (n_slices - 1) as f64;
            for r in 0..4 {
                values.push(t);
                // Deliberately very unequal radial volumes.
                volumes.push(1.0e-9 * (r + 1) as f64 * (r + 1) as f64);
                ids.push(Some(s));
            }
        }
        let avg = SliceAverage::compute(&values, &volumes, &ids, n_slices).unwrap();
        for s in 0..n_slices {
            let expected = 600.0 + 900.0 * s as f64 / (n_slices - 1) as f64;
            assert!(
                (avg.mean(s).unwrap() - expected).abs() < 1e-12,
                "slice {s}: {:?} vs {expected}",
                avg.mean(s)
            );
        }
    }

    /// Self-consistency check — unmatched cells are excluded, and empty slices
    /// are reported rather than aborted on.
    ///
    /// **Methodology.** Three cells, one of which has slice ID `None`. It must
    /// contribute to neither the mean nor the slice volume. A slice with no
    /// cells must appear in [`SliceAverage::empty_slices`] with a mean of 0.
    ///
    /// **Result** (2026-07-29): the `None` cell was excluded (mean 1000, not
    /// 750); slice 1 was reported empty with a mean of 0.
    #[test]
    fn unassigned_cells_are_excluded_and_empty_slices_reported() {
        let values = [1000.0, 1000.0, 250.0];
        let volumes = [1.0e-9, 1.0e-9, 1.0e-9];
        let ids = [Some(0), Some(0), Option::None];
        let avg = SliceAverage::compute(&values, &volumes, &ids, 2).unwrap();

        assert!((avg.mean(0).unwrap() - 1000.0).abs() < 1e-12);
        assert!((avg.slice_volume(0).unwrap() - 2.0e-9).abs() < 1e-20);
        assert_eq!(avg.mean(1), Some(0.0));
        assert_eq!(avg.empty_slices(), vec![1]);
        assert_eq!(avg.n_slices(), 2);
    }

    /// Self-consistency check — mismatched or out-of-range input is rejected.
    #[test]
    fn slice_average_rejects_bad_input() {
        assert!(SliceAverage::compute(&[1.0], &[1.0, 2.0], &[Some(0)], 1).is_err());
        assert!(SliceAverage::compute(&[1.0], &[1.0], &[Some(5)], 1).is_err());
        assert!(SliceAverage::compute(&[1.0], &[-1.0], &[Some(0)], 1).is_err());
    }

    /// Self-consistency check — the axial projection is a dot product, and a
    /// non-unit pin direction is caught.
    ///
    /// **Methodology.** A point at (0.003, 0.004, 0.5) projected on the z axis
    /// must give 0.5 exactly, and on a 45° direction in the x–z plane must give
    /// `(0.003 + 0.5)/sqrt(2)`. A pin direction of magnitude 2 must be rejected
    /// by the checked form and silently doubled by the unchecked one, matching
    /// upstream.
    ///
    /// **Result** (2026-07-29, measured): 0.5 exactly on z; 0.355675 on the 45°
    /// direction; a magnitude-2 direction gave 1.0 from the unchecked form and
    /// an error from the checked one.
    #[test]
    fn axial_projection_is_a_dot_product() {
        let p = Vector3 {
            x: 0.003,
            y: 0.004,
            z: 0.5,
        };
        let z_axis = Vector3 {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        };
        assert!((axial_coordinate(p, z_axis) - 0.5).abs() < 1e-15);
        assert!((axial_coordinate_checked(p, z_axis).unwrap() - 0.5).abs() < 1e-15);

        let diagonal = Vector3 {
            x: std::f64::consts::FRAC_1_SQRT_2,
            y: 0.0,
            z: std::f64::consts::FRAC_1_SQRT_2,
        };
        let expected = (0.003 + 0.5) * std::f64::consts::FRAC_1_SQRT_2;
        assert!((axial_coordinate(p, diagonal) - expected).abs() < 1e-15);
        assert!((expected - 0.355_675).abs() < 1e-5, "expected = {expected}");

        let doubled = Vector3 {
            x: 0.0,
            y: 0.0,
            z: 2.0,
        };
        assert!((axial_coordinate(p, doubled) - 1.0).abs() < 1e-15);
        assert!(axial_coordinate_checked(p, doubled).is_err());
    }

    /// Self-consistency check — the `none` mapper slices nothing.
    #[test]
    fn none_mapper_assigns_no_slices() {
        let slicing = AxialSlicing::None;
        assert_eq!(slicing.n_slices(), Some(0));
        assert_eq!(slicing.total_height(), Option::None);
        assert_eq!(slicing.slice_index(0.05), Option::None);
        assert_eq!(
            slicing.assign(&[0.01, 0.02], 0.0),
            vec![Option::None, Option::None]
        );
        assert!(slicing.validate(Option::None).is_ok());
    }

    /// Self-consistency check — configuration validation.
    #[test]
    fn validation_rejects_bad_configurations() {
        assert!(AxialSlicing::by_material_uniform(0, 0.1).is_err());
        assert!(AxialSlicing::by_material_uniform(10, 0.0).is_err());
        assert!(AxialSlicing::by_pellets_uniform(0, 0.1).is_err());

        assert!(AxialSlicing::ByMaterial {
            slice_heights: vec![]
        }
        .validate(Option::None)
        .is_err());
        assert!(AxialSlicing::ByMaterial {
            slice_heights: vec![0.01, -0.01]
        }
        .validate(Option::None)
        .is_err());
        assert!(AxialSlicing::AutoAxial { precision: 0.0 }
            .validate(Option::None)
            .is_err());
    }

    /// Self-consistency check — `material_bottom` shifts the whole assignment.
    ///
    /// **Methodology.** The same cell coordinates offset by +1 m, with
    /// `material_bottom` offset by the same amount, must produce identical slice
    /// assignments — the mapping depends only on the height above the material's
    /// bottom, never on the absolute coordinate.
    #[test]
    fn assignment_depends_only_on_height_above_the_material_bottom() {
        let slicing = AxialSlicing::by_material_uniform(10, 0.1).unwrap();
        let centres: Vec<f64> = (0..10).map(|i| 0.005 + 0.01 * i as f64).collect();
        let shifted: Vec<f64> = centres.iter().map(|z| z + 1.0).collect();
        assert_eq!(slicing.assign(&centres, 0.0), slicing.assign(&shifted, 1.0));
    }
}
