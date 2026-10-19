//! **The `advanced` entry** (gh #942, epic #925): the one place where the
//! beginner API lets you change physics or choose your own mesh.
//!
//! Correct physics is **on and hidden** at the high level (root CLAUDE.md,
//! "Correct physics is the DEFAULT SETTING"; maintainer decisions on #925,
//! 2026-10-17). Switching any of it off is an explicit, visible act that
//! goes through this module, so a reviewer finds every ablation by
//! searching for one word, `advanced`:
//!
//! ```no_run
//! use outram_mc_libs::model::prelude::*;
//! # fn f(godiva: Model) -> Result<(), Box<dyn std::error::Error>> {
//! let ablated = godiva.advanced().without_dbrc().without_urr();
//! let run = ablated.keff(KeffSettings::new(MCSettings::Fast, None))?;
//! assert!(!run.provenance.ablations.is_empty());   // the run says what was off
//! # Ok(()) }
//! ```
//!
//! **Maps on a mesh you choose** (both the spatial mesh and its energy
//! groups, decided on #925) are built here too: [`mesh_divisions`] /
//! [`mesh_cells`] and [`fission_map`] / [`flux_map`]. The
//! `Diagnostics::reactor_physics` preset keeps its own 15³ maps without
//! any of this.
//!
//! The approximate double-heterogeneity trackings and the other `advanced`
//! choices named on #943 arrive with the shapes that need them (#926).

use uom::si::f64::Length;
use uom::si::length::centimeter;

pub use crate::physics::delta_tracking::CoverSettings;

use super::diagnostics::{EnergyGrid, MCDiagnostic, Mesh, MeshBounds, MeshSpec};
use super::model::{AssembledModel, Model, ModelError};
use super::run::{KeffRun, KeffSettings, RunError};

/// **A model with physics switched off on purpose.** Made by
/// [`Model::advanced`]; every switch is a `without_*` call, and every run
/// records them in `RunProvenance::ablations`.
#[derive(Debug, Clone)]
pub struct AdvancedModel {
    model: Model,
    dbrc: bool,
    urr: bool,
    thermal_laws: bool,
    majorant_cover: Option<CoverSettings>,
}

impl Model {
    /// **The advanced entry**: physics ablation (`without_dbrc`, ...). An
    /// ordinary model never needs it; correct physics is the default.
    pub fn advanced(self) -> AdvancedModel {
        AdvancedModel {
            model: self,
            dbrc: true,
            urr: true,
            thermal_laws: true,
            majorant_cover: None,
        }
    }
}

impl AdvancedModel {
    /// **Ablate DBRC**: resonance elastic scattering on the
    /// constant-cross-section free-gas kernel (`Nuclide::without_dbrc`).
    pub fn without_dbrc(mut self) -> Self {
        self.dbrc = false;
        self
    }

    /// **Ablate URR self-shielding**: infinitely dilute unresolved-range
    /// cross sections (`Nuclide::without_urr_probability_tables`).
    pub fn without_urr(mut self) -> Self {
        self.urr = false;
        self
    }

    /// **Ablate the thermal scattering laws**: bound atoms (H in water, C in
    /// graphite, ...) scatter as a free gas.
    pub fn without_thermal_scattering(mut self) -> Self {
        self.thermal_laws = false;
        self
    }

    /// **Cover each delta-tracked region's majorant with Chebyshev bands**
    /// (gh #945): `regions` energy bands, each a polynomial of the given
    /// `degree` in ln E, lifted so it is never below the stage-1 table.
    /// Changes the cost (build memory, virtual collisions), never the
    /// answer, so it is not an ablation and is not listed in
    /// [`Self::ablations`]. The default without this call is the table.
    /// [`CoverSettings::default`] is the measured default.
    pub fn with_majorant_cover(mut self, settings: CoverSettings) -> Self {
        self.majorant_cover = Some(settings);
        self
    }

    /// The cover requested through [`Self::with_majorant_cover`], if any.
    pub fn majorant_cover(&self) -> Option<CoverSettings> {
        self.majorant_cover
    }

    /// The model assembled with this entry's choices (the majorant cover)
    /// applied, for inspection and for [`AssembledModel::majorants`].
    ///
    /// # Errors
    /// As [`Model::assemble`].
    pub fn assemble(&self) -> Result<AssembledModel, ModelError> {
        let mut a = self.model.assemble()?;
        a.majorant_cover = self.majorant_cover;
        Ok(a)
    }

    /// The model underneath.
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// What is switched off, in words (empty: nothing).
    pub fn ablations(&self) -> Vec<String> {
        let mut v = Vec::new();
        if !self.dbrc {
            v.push("DBRC off (advanced().without_dbrc()): resonance elastic scattering on the constant-cross-section free-gas kernel".to_string());
        }
        if !self.urr {
            v.push("URR self-shielding off (advanced().without_urr()): infinitely dilute unresolved-resonance cross sections".to_string());
        }
        if !self.thermal_laws {
            v.push("thermal scattering laws off (advanced().without_thermal_scattering()): bound atoms scatter as a free gas".to_string());
        }
        v
    }

    /// **Run k_eff with the ablations**, as [`Model::keff`] otherwise. The
    /// result's `provenance.ablations` lists them and its per-nuclide
    /// physics shows them.
    ///
    /// # Errors
    /// As [`Model::keff`].
    pub fn keff(&self, settings: KeffSettings) -> Result<KeffRun, RunError> {
        let mut a = self.assemble()?;
        if !self.thermal_laws {
            for k in &mut a.nuclides {
                k.law = None;
            }
        }
        let nuclides: Vec<_> = a
            .load_nuclides(self.model.data())?
            .into_iter()
            .map(|n| {
                let n = if self.dbrc { n } else { n.without_dbrc() };
                if self.urr {
                    n
                } else {
                    n.without_urr_probability_tables()
                }
            })
            .collect();
        a.keff_with_ablations(&nuclides, &settings, self.ablations())
    }
}

/// **A mesh of `n` cells per axis** over `bounds`.
pub fn mesh_divisions(n: [usize; 3], bounds: MeshBounds) -> Mesh {
    Mesh {
        kind: MeshSpec::Divisions { n, bounds },
    }
}

/// **A mesh of cells of the given size** over `bounds`; the cell count per
/// axis is rounded up and the box grown to fit, so every cell keeps the
/// size asked for.
pub fn mesh_cells(cell: [Length; 3], bounds: MeshBounds) -> Mesh {
    Mesh {
        kind: MeshSpec::Cells {
            cell_cm: cell.map(|x| x.get::<centimeter>()),
            bounds,
        },
    }
}

/// **A fission-rate map** on your mesh.
pub fn fission_map(mesh: Mesh) -> MCDiagnostic {
    MCDiagnostic::FissionMap { mesh }
}

/// **A flux map** on your mesh and energy groups.
pub fn flux_map(mesh: Mesh, grid: EnergyGrid) -> MCDiagnostic {
    MCDiagnostic::FluxMap { mesh, grid }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The constructors build what they say; ablations are listed in words.
    #[test]
    fn advanced_builders() {
        let cm = Length::new::<centimeter>;
        let m = mesh_cells([cm(1.0); 3], MeshBounds::FitModel);
        let r = m.resolve(([0.0; 3], [2.5; 3])).unwrap();
        assert_eq!(r.dimension, [3, 3, 3]);
        let d = mesh_divisions([2, 3, 4], MeshBounds::FitModel);
        assert_eq!(
            d.resolve(([0.0; 3], [1.0; 3])).unwrap().dimension,
            [2, 3, 4]
        );
        assert_eq!(fission_map(d.clone()).name(), "FissionMap");
        assert_eq!(flux_map(d, EnergyGrid::OneGroup).name(), "FluxMap");
        let model = super::super::Model::new(super::super::model::tests::fake_data());
        let a = model.advanced();
        assert!(a.ablations().is_empty());
        let a = a.without_dbrc().without_urr().without_thermal_scattering();
        assert_eq!(a.ablations().len(), 3);
        assert!(a.model().shapes().is_empty());
        // The cover is a cost choice, not an ablation: set, readable, and not listed among the ablations.
        assert_eq!(a.majorant_cover(), None);
        let c = CoverSettings {
            regions: 8,
            degree: 3,
        };
        let a = a.with_majorant_cover(c);
        assert_eq!(a.majorant_cover(), Some(c));
        assert_eq!(a.ablations().len(), 3);
        // An empty model is refused by assemble, with the cover set or not
        // (the carried cover is pinned in tests/majorant_temperature_cover.rs).
        assert!(a.assemble().is_err());
    }
}
