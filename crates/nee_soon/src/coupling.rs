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

//! # Coupling Monte Carlo transport to the GeN-Foam deterministic solvers
//!
//! One type, [`McToGenFoam`], carries a reactor model from a Monte Carlo
//! transport run to a deterministic solve **on the same geometry and the same
//! compositions**, so the two ends cannot quietly describe different reactors.
//!
//! ## The shortest thing that works
//!
//! ```no_run
//! use nee_soon::coupling::McToGenFoam;
//! use nee_soon::mgxs::GroupStructure;
//! # use outram_mc_libs::geometry::geometry::Geometry;
//! # use outram_mc_libs::material::material::Material;
//! # use outram_mc_libs::material::nuclide::Nuclide;
//! # use outram_mc_libs::physics::transport_csg::SourceBox;
//! # fn demo(geometry: Geometry, materials: Vec<Material>, nuclides: Vec<Nuclide>, source: SourceBox)
//! # -> Result<(), Box<dyn std::error::Error>> {
//! let coupling = McToGenFoam::new(geometry, materials, nuclides, source)
//!     .with_groups(GroupStructure::two_group(2.38)?)
//!     .with_particles(2_000);
//!
//! let mgxs = coupling.generate_mgxs()?;          // Monte Carlo -> group constants
//! println!("Monte Carlo k = {:.5}", mgxs.k_eff);
//!
//! let k_det = coupling.solve_infinite_medium(&mgxs.library)?;
//! println!("GeN-Foam k_inf = {k_det:.5}");
//! # Ok(())
//! # }
//! ```
//!
//! Three calls: construct, [`generate_mgxs`](McToGenFoam::generate_mgxs),
//! [`solve_infinite_medium`](McToGenFoam::solve_infinite_medium). The builder
//! setters all have defaults, so none of them is required.
//!
//! ## What each stage costs you
//!
//! [`generate_mgxs`](McToGenFoam::generate_mgxs) runs the transport **twice**
//! at one seed — once for scalar reaction rates, once for the scattering matrix
//! and fission spectrum, which need an outgoing-energy axis the scalar rates
//! must not have. Budget accordingly: it is twice the cost of a plain
//! k-eigenvalue run.
//!
//! ## Honest limits, stated once
//!
//! - **Steady state only.** No delayed-neutron data is tallied, so a transient
//!   driven from these constants would have no delayed neutrons. See
//!   [`crate::genfoam_xs`].
//! - **`P0` scattering only**, so the diffusion coefficient is the `1/(3 Sigma_t)`
//!   approximation rather than transport-corrected.
//! - **One state point.** One Monte Carlo run is one temperature and one
//!   density; there is no feedback parametrisation.
//! - [`solve_infinite_medium`](McToGenFoam::solve_infinite_medium) is a
//!   zero-leakage collapse. It is the right check that the group constants are
//!   self-consistent, and it is **not** the reactor's eigenvalue whenever
//!   leakage matters — which for a real core it does.

use std::sync::Arc;

use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::{
    DiffusionNeutronics, DiffusionSettings,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData;
use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
use outram_foam_basic_lib::prelude::BoundaryCondition;
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{run_keff_csg, run_keff_csg_hybrid, SourceBox};
use uom::si::area::square_meter;
use uom::si::f64::{Area, Length};
use uom::si::length::meter;

use crate::genfoam_xs::{to_nuclear_data_input, GenfoamXsError};
use crate::mgxs::{condense, matrix_tally, scalar_tally, GroupStructure, MgxsError, MgxsLibrary};

/// Anything that can go wrong carrying a model from Monte Carlo to GeN-Foam.
///
/// Each variant names which stage failed and what to do about it, because the
/// stages have genuinely different remedies: a condensation failure is a tally
/// setup problem, an unvisited group is a group-structure problem, and a solver
/// failure is a numerics problem.
#[derive(Debug, thiserror::Error)]
pub enum CouplingError {
    /// The Monte Carlo tallies could not be condensed into group constants.
    #[error("condensing the Monte Carlo tallies failed: {0}")]
    Condensation(#[from] MgxsError),
    /// The group constants were rejected on the way to GeN-Foam.
    #[error("handing the cross sections to GeN-Foam failed: {0}")]
    Bridge(#[from] GenfoamXsError),
    /// GeN-Foam rejected the `nuclearData` dictionary.
    #[error("GeN-Foam rejected the cross-section data: {0}")]
    GenfoamXs(String),
    /// The deterministic solve did not produce an eigenvalue.
    #[error("the GeN-Foam eigenvalue solve failed: {0}")]
    Solve(String),
}

/// What a Monte Carlo MGXS pass produced.
#[derive(Debug, Clone)]
pub struct MgxsRun {
    /// The condensed group constants, in the group structure's ascending-energy
    /// order. Call
    /// [`in_descending_energy`](MgxsLibrary::in_descending_energy) for the
    /// reactor convention.
    pub library: MgxsLibrary,
    /// The Monte Carlo eigenvalue from the same run — the reference a
    /// deterministic solve on these constants should reproduce where the
    /// comparison is fair.
    pub k_eff: f64,
    /// Its one-sigma statistical uncertainty. Quote it: a deterministic result
    /// agreeing to better than this is agreeing with noise.
    pub k_std: f64,
}

/// Carries one reactor model from Monte Carlo transport to a GeN-Foam
/// deterministic solve.
///
/// Construct with [`new`](Self::new), adjust with the `with_*` setters (all
/// optional), then call [`generate_mgxs`](Self::generate_mgxs).
pub struct McToGenFoam {
    geometry: Geometry,
    materials: Vec<Material>,
    nuclides: Vec<Nuclide>,
    source: SourceBox,
    groups: GroupStructure,
    majorants: Vec<Majorant>,
    settings: KeffSettings,
}

impl McToGenFoam {
    /// A coupling over `geometry`, with `materials` indexed as the geometry's
    /// cells reference them, `nuclides` as the materials reference them, and
    /// `source` a box the initial fission source is rejection-sampled into.
    ///
    /// Defaults: a two-group structure split at 2.38 eV (the cadmium cut-off, a
    /// conventional thermal/fast boundary), 2000 particles, 20 inactive and 40
    /// active batches, surface tracking. Override any of them with the `with_*`
    /// setters below.
    #[must_use]
    pub fn new(
        geometry: Geometry,
        materials: Vec<Material>,
        nuclides: Vec<Nuclide>,
        source: SourceBox,
    ) -> Self {
        Self {
            geometry,
            materials,
            nuclides,
            source,
            groups: GroupStructure::two_group(2.38).expect("2.38 eV is a valid split"),
            majorants: Vec::new(),
            settings: KeffSettings {
                n_particles: 2_000,
                n_inactive: 20,
                n_active: 40,
                seed: 20_260_919,
                ..KeffSettings::default()
            },
        }
    }

    /// Use `groups` as the energy group structure.
    #[must_use]
    pub fn with_groups(mut self, groups: GroupStructure) -> Self {
        self.groups = groups;
        self
    }

    /// Transport `n` particles per batch.
    #[must_use]
    pub fn with_particles(mut self, n: usize) -> Self {
        self.settings.n_particles = n;
        self
    }

    /// Use `inactive` source-convergence batches and `active` scoring batches.
    #[must_use]
    pub fn with_batches(mut self, inactive: usize, active: usize) -> Self {
        self.settings.n_inactive = inactive;
        self.settings.n_active = active;
        self
    }

    /// Fix the random seed. Both MGXS passes use it, which is what makes the
    /// scalar pass's flux the correct denominator for the matrix pass rather
    /// than an independent estimate.
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.settings.seed = seed;
        self
    }

    /// Supply delta-tracking majorants, for a model with delta-tracked regions
    /// (a pebble bed). Without these the run is surface-tracked throughout.
    #[must_use]
    pub fn with_majorants(mut self, majorants: Vec<Majorant>) -> Self {
        self.majorants = majorants;
        self
    }

    /// The group structure in use.
    #[must_use]
    pub fn groups(&self) -> &GroupStructure {
        &self.groups
    }

    /// Run the Monte Carlo passes and condense them into group constants.
    ///
    /// Two transport passes at one seed: scalar reaction rates, then the
    /// scattering matrix and fission spectrum.
    ///
    /// # Errors
    ///
    /// [`CouplingError::Condensation`] if the tallies and the group structure
    /// disagree about bin counts.
    pub fn generate_mgxs(&self) -> Result<MgxsRun, CouplingError> {
        let zone_indices: Vec<usize> = (0..self.materials.len()).collect();
        let names: Vec<String> = self.materials.iter().map(|m| m.name.clone()).collect();

        let mut scalar = scalar_tally(1, &self.groups, zone_indices.clone());
        let mc = self.run(&mut scalar);
        let mut matrix = matrix_tally(2, &self.groups, zone_indices);
        self.run(&mut matrix);

        let library = condense(
            &self.groups,
            &names,
            &scalar,
            &matrix,
            self.settings.n_active as u64,
        )?;
        Ok(MgxsRun {
            library,
            k_eff: mc.k_mean,
            k_std: mc.k_std,
        })
    }

    /// One transport pass, dispatching on whether majorants were supplied.
    fn run(
        &self,
        tally: &mut outram_mc_libs::tally::tally::Tally,
    ) -> outram_mc_libs::physics::keff::KeffResult {
        if self.majorants.is_empty() {
            run_keff_csg(
                &self.geometry,
                &self.materials,
                &self.nuclides,
                self.source,
                &self.settings,
                Some(tally),
            )
        } else {
            run_keff_csg_hybrid(
                &self.geometry,
                &self.materials,
                &self.nuclides,
                &self.majorants,
                None,
                self.source,
                &self.settings,
                Some(tally),
            )
        }
    }

    /// Solve `k_inf` with GeN-Foam's multigroup diffusion on zone 0 of
    /// `library`, with **zero leakage**.
    ///
    /// # What this is for
    ///
    /// It is the cross-check that the group constants are self-consistent: with
    /// no leakage the diffusion coefficient multiplies a zero gradient, so the
    /// eigenvalue depends only on the constants the condensation preserved and
    /// must reproduce the Monte Carlo result to within statistics. A
    /// disagreement means the condensation, the units, the matrix orientation
    /// or the spectrum is wrong.
    ///
    /// # What it is NOT
    ///
    /// It is **not the reactor's eigenvalue** unless the reactor genuinely has
    /// no leakage. For a real core, leakage is most of the physics and this
    /// number will be far too high.
    ///
    /// # Errors
    ///
    /// [`CouplingError::Bridge`] if a group carries no flux (its cross sections
    /// were never measured, and zeros make the solve singular),
    /// [`CouplingError::GenfoamXs`] if GeN-Foam rejects the data, or
    /// [`CouplingError::Solve`] if the power iteration does not converge.
    pub fn solve_infinite_medium(&self, library: &MgxsLibrary) -> Result<f64, CouplingError> {
        let descending = library.in_descending_energy();
        let input = to_nuclear_data_input(&descending)?;
        let xs = CrossSectionData::from_input(&input)
            .map_err(|e| CouplingError::GenfoamXs(e.to_string()))?;

        let mesh = Arc::new(
            create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), 4)
                .map_err(|e| CouplingError::Solve(format!("{e:?}")))?,
        );
        let zone_of_cell = vec![0usize; mesh.n_cells];
        let bc = vec![
            BoundaryCondition::ZeroGradient,
            BoundaryCondition::ZeroGradient,
        ];
        let mut model = DiffusionNeutronics::new(
            mesh,
            &xs,
            &zone_of_cell,
            &[],
            &bc,
            DiffusionSettings::default(),
        )
        .map_err(|e| CouplingError::Solve(e.to_string()))?;
        let report = model
            .solve_eigenvalue()
            .map_err(|e| CouplingError::Solve(e.to_string()))?;
        Ok(report.k_eff)
    }
}
