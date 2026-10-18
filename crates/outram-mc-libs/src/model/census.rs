//! **What the assembled geometry holds: copies and volumes** (gh #926).
//!
//! Counted from the **assembled** CSG, never from the numbers the model was
//! built with: the HTR-10 history had a TRISO lattice holding 8240
//! particles while the model reported 8340 (gh:#316, +353 pcm), because the
//! count came from a constant. Here the copies of each cell are the
//! distributed-cell instance count of the geometry itself
//! ([`DistribcellOffsets`], a walk of the universe and lattice graph, the
//! numbering OpenMC's `distribcell` uses), and each copy's volume is exact
//! (a shape's solid minus the solids it contains, from the lowering).

use uom::si::f64::{Ratio, Volume};
use uom::si::volume::cubic_centimeter;

use super::model::{AssembledModel, ModelError};
use super::shape::Component;
use crate::geometry::cell::CellFill;
use crate::geometry::distribcell::DistribcellOffsets;

/// One material cell of the assembled geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct CensusRow {
    /// The cell's index in the assembled geometry.
    pub cell: usize,
    /// Its material's name.
    pub material: String,
    /// Its component, if its shape had one.
    pub component: Option<Component>,
    /// That component's label.
    pub component_label: Option<String>,
    /// How many copies of the cell the geometry places (every lattice tile
    /// and nested universe it is reached through).
    pub copies: usize,
    /// The exact volume of one copy.
    pub volume_each: Volume,
}

impl CensusRow {
    /// `copies × volume_each`.
    pub fn volume(&self) -> Volume {
        self.volume_each * self.copies as f64
    }
}

/// **Every material cell, how many copies the geometry places, and their
/// volumes.** Made by [`AssembledModel::census`].
#[derive(Debug, Clone, PartialEq)]
pub struct Census {
    /// One row per material cell, in cell order.
    pub rows: Vec<CensusRow>,
}

impl Census {
    /// **How many shapes of `component` the geometry places**: the copies of
    /// every cell in it, summed. For a component holding one shape per
    /// particle (a TRISO kernel), this is the particle count.
    pub fn copies(&self, component: Component) -> usize {
        self.rows
            .iter()
            .filter(|r| r.component == Some(component))
            .map(|r| r.copies)
            .sum()
    }

    /// The total volume of `component`'s cells.
    pub fn volume(&self, component: Component) -> Volume {
        self.rows
            .iter()
            .filter(|r| r.component == Some(component))
            .map(CensusRow::volume)
            .fold(Volume::new::<cubic_centimeter>(0.0), |a, b| a + b)
    }

    /// The volume of every material cell together (void excluded).
    pub fn total_volume(&self) -> Volume {
        self.rows
            .iter()
            .map(CensusRow::volume)
            .fold(Volume::new::<cubic_centimeter>(0.0), |a, b| a + b)
    }

    /// `component`'s share of [`Self::total_volume`].
    pub fn volume_fraction(&self, component: Component) -> Ratio {
        self.volume(component) / self.total_volume()
    }
}

impl std::fmt::Display for Census {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "{:>5}  {:<28} {:<14} {:>8} {:>14} {:>14} {:>9}",
            "cell", "material", "component", "copies", "each [cm3]", "total [cm3]", "share"
        )?;
        let total = self.total_volume().get::<cubic_centimeter>();
        for r in &self.rows {
            let v = r.volume().get::<cubic_centimeter>();
            writeln!(
                f,
                "{:>5}  {:<28} {:<14} {:>8} {:>14.6e} {:>14.6e} {:>8.5}%",
                r.cell,
                r.material,
                r.component_label.as_deref().unwrap_or("-"),
                r.copies,
                r.volume_each.get::<cubic_centimeter>(),
                v,
                100.0 * v / total
            )?;
        }
        write!(f, "total volume {total:.6e} cm3")
    }
}

/// Copies of cell `i` the geometry places (distributed-cell instances).
fn instances(g: &crate::geometry::geometry::Geometry, i: usize) -> Result<usize, ModelError> {
    Ok(DistribcellOffsets::build(g, i)
        .map_err(ModelError::BadValue)?
        .n_instances)
}

impl AssembledModel {
    /// **Count and measure what the assembled geometry holds** (see the
    /// module docs for why it is counted from the geometry).
    ///
    /// # Errors
    /// [`ModelError::BadValue`] if the geometry's universe graph cannot be
    /// walked (a cycle or an index out of range; an assembled model never
    /// has one, so this is a defect if seen).
    pub fn census(&self) -> Result<Census, ModelError> {
        let mut rows = Vec::new();
        for (i, cell) in self.geometry.cells.iter().enumerate() {
            let CellFill::Material(m) = cell.fill else {
                continue;
            };
            let mut copies = instances(&self.geometry, i)?;
            // Distribcell does not number a lattice's `outer` universe: it
            // is reached once per placement of the cell holding the lattice.
            if let Some(u) = self
                .geometry
                .universes
                .iter()
                .position(|u| u.cell_indices.contains(&i))
            {
                for (li, l) in self.geometry.lattices.iter().enumerate() {
                    let outer = match l {
                        crate::geometry::lattice::Lattice::Rect(r) => r.outer,
                        crate::geometry::lattice::Lattice::Hex(h) => h.outer,
                    };
                    if outer != Some(u) {
                        continue;
                    }
                    for (c, cell) in self.geometry.cells.iter().enumerate() {
                        if cell.fill == CellFill::Lattice(li) {
                            copies += instances(&self.geometry, c)?;
                        }
                    }
                }
            }
            let component = self.cell_components[i];
            rows.push(CensusRow {
                cell: i,
                material: self.resolved[m].name.clone(),
                component,
                component_label: component
                    .and_then(|c| self.component_labels.get(c.index).cloned()),
                copies,
                volume_each: Volume::new::<cubic_centimeter>(
                    self.cell_volumes_cm3[i].unwrap_or(f64::NAN),
                ),
            });
        }
        Ok(Census { rows })
    }
}
