//! # The wizard's steps
//!
//! The twelve steps the maintainer laid out on 2026-10-05, in order. Each one
//! knows its title, its phase, and whether this build implements it; one that
//! is not built yet names the GitHub issue that will build it, so the wizard
//! can show the whole road without pretending the far end exists.

/// One step of the guided build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum WizardStep {
    /// Step 0: the ENDF library ("show me your ENDF library").
    NuclearData,
    /// Step 1: DEM pebble bed, packing and pebble-type mix.
    PebbleBed,
    /// Step 2: one design per pebble type (materials, TRISO).
    PebbleDesign,
    /// Step 3: reflector, boronated bricks, borings, chutes, plenums, risers.
    Reflector,
    /// Step 4: what goes in the borings (rods, steel, absorber spheres).
    Inserts,
    /// The geometry review gate, between Phase A and Step 5.
    Review,
    /// Step 5: pure Monte Carlo k_eff runs.
    MonteCarlo,
    /// Step 6: branch, multiphysics (default) or reactivity map.
    Branch,
    /// Step 7: three meshes, regions and mesh-to-mesh mapping.
    Meshing,
    /// Step 8: multigroup cross sections per region.
    Mgxs,
    /// Step 9: multiphysics case setup.
    Setup,
    /// Step 10: run the coupled case.
    Run,
    /// Step 11: post-processing and exports.
    PostProcessing,
}

impl WizardStep {
    /// Every step, in wizard order.
    pub const ALL: [Self; 13] = [
        Self::NuclearData,
        Self::PebbleBed,
        Self::PebbleDesign,
        Self::Reflector,
        Self::Inserts,
        Self::Review,
        Self::MonteCarlo,
        Self::Branch,
        Self::Meshing,
        Self::Mgxs,
        Self::Setup,
        Self::Run,
        Self::PostProcessing,
    ];

    /// The step number shown in the top bar (`None` for the review gate,
    /// which sits between Step 4 and Step 5).
    #[must_use]
    pub fn number(self) -> Option<u8> {
        match self {
            Self::NuclearData => Some(0),
            Self::PebbleBed => Some(1),
            Self::PebbleDesign => Some(2),
            Self::Reflector => Some(3),
            Self::Inserts => Some(4),
            Self::Review => None,
            Self::MonteCarlo => Some(5),
            Self::Branch => Some(6),
            Self::Meshing => Some(7),
            Self::Mgxs => Some(8),
            Self::Setup => Some(9),
            Self::Run => Some(10),
            Self::PostProcessing => Some(11),
        }
    }

    /// The step's name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::NuclearData => "Nuclear data library",
            Self::PebbleBed => "DEM pebble bed construction",
            Self::PebbleDesign => "Pebble design",
            Self::Reflector => "Reflector and internals",
            Self::Inserts => "Inserts",
            Self::Review => "Geometry review",
            Self::MonteCarlo => "Monte Carlo",
            Self::Branch => "Parameter extraction: choose a branch",
            Self::Meshing => "Meshing and regions",
            Self::Mgxs => "Multigroup cross sections",
            Self::Setup => "Multiphysics case setup",
            Self::Run => "Case running",
            Self::PostProcessing => "Post-processing",
        }
    }

    /// Top-bar text, e.g. `Step 1: DEM pebble bed construction`.
    #[must_use]
    pub fn title(self) -> String {
        match self.number() {
            Some(n) => format!("Step {n}: {}", self.name()),
            None => format!("Review gate: {}", self.name()),
        }
    }

    /// Phase A (core geometry) or B (physics).
    #[must_use]
    pub fn phase(self) -> &'static str {
        match self {
            Self::NuclearData => "Set-up",
            Self::PebbleBed | Self::PebbleDesign | Self::Reflector | Self::Inserts => {
                "Phase A: core geometry"
            }
            _ => "Phase B: physics",
        }
    }

    /// Whether this build implements the step.
    #[must_use]
    pub fn implemented(self) -> bool {
        // Every step is built since 2026-10-05: Step 6 (gh:#571), 7 (gh:#572),
        // 8 (gh:#573), 9-10 (a SIMPLIFIED coupled case) and 11 (gh:#574);
        // their panels say what is and is not modelled.
        let _ = self;
        true
    }

    /// The GitHub issue that builds a step this build does not.
    #[must_use]
    pub fn issue(self) -> Option<u32> {
        let _ = self;
        None
    }

    /// What the step does, one paragraph, shown under the top bar.
    #[must_use]
    pub fn guide(self) -> &'static str {
        match self {
            Self::NuclearData => {
                "Point Dhoby Ghaut at a folder of unzipped ENDF tapes. It lists what is there \
                 and checks it against every nuclide and thermal-scattering law this reactor \
                 needs, so a missing tape shows up now and not at Step 5."
            }
            Self::PebbleBed => {
                "The pebble bed: how the pebbles pack, how high the bed is loaded, and the mix \
                 of pebble types. HTR-10 has fuel and moderator (dummy graphite) pebbles only. \
                 Everything is prefilled; change it in the panel on the right if you want."
            }
            Self::PebbleDesign => {
                "One design per pebble type in the mix: radii, materials and, for fuel \
                 pebbles, the TRISO particle. The voids between pebbles are helium."
            }
            Self::Reflector => {
                "Graphite reflector and boronated carbon bricks, the borings through them \
                 (control rods, absorber spheres, irradiation channels), the chutes, the \
                 plenums and the cold gas risers. Parts the HTR-10 model does not carry are \
                 listed as such, never silently dropped."
            }
            Self::Inserts => {
                "What fills the borings: control rods (B4C, steel), small absorber spheres, \
                 pebbles in the irradiation channels."
            }
            Self::Review => {
                "Look at what the solver will see before trusting anything computed on it: \
                 slices of the ASSEMBLED geometry, coloured by material. Confirm below to \
                 continue to Monte Carlo."
            }
            Self::MonteCarlo => {
                "Continuous-energy k_eff on the geometry you built. Set the state (temperature, \
                 control-rod insertion), run, and compare runs. Each run gives k_eff ± σ and \
                 the lethargy-normalised neutron spectrum."
            }
            Self::Branch => {
                "Multiphysics (default): Monte Carlo -> multigroup cross sections -> coupled \
                 neutronics, thermal-hydraulics and structural solvers. Or a reactivity map: \
                 a low-fidelity, neutronics-only surrogate exported as a TOML config."
            }
            Self::Meshing => {
                "Three meshes, GeN-Foam style (neutronics, thermal-hydraulics, structural \
                 FEM), their regions, and the mapping between every pair."
            }
            Self::Mgxs => "Multigroup cross sections per region, interpolated in ln T by default.",
            Self::Setup => {
                "Boundary conditions, solvers and models for the OUTRAM-Foam and farrer-park \
                 sides, all prefilled from the HTR-10 literature. The panel lists what the \
                 coupled run models, simplifies, and leaves out."
            }
            Self::Run => {
                "Run the coupled case off the UI thread: the porous core's thermal-hydraulics \
                 with a prescribed power shape and lumped temperature feedback, with live \
                 residuals, temperatures and k. A simplified run, labelled as one."
            }
            Self::PostProcessing => {
                "Export CSV (runs, spectra, the reactivity map on a grid), a kovan markdown \
                 report and the recipe; load a recipe back and check it round-trips."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_are_numbered_zero_to_eleven_with_the_gate_between_four_and_five() {
        let numbers: Vec<Option<u8>> = WizardStep::ALL.iter().map(|s| s.number()).collect();
        let mut want: Vec<Option<u8>> = (0..=4).map(Some).collect();
        want.push(None);
        want.extend((5..=11).map(Some));
        assert_eq!(numbers, want);
    }

    /// Every step the build does not implement names the issue that will.
    #[test]
    fn unimplemented_steps_name_their_issue() {
        for s in WizardStep::ALL {
            assert_eq!(s.implemented(), s.issue().is_none(), "{s:?}");
        }
    }

    /// Every step is built since 2026-10-05 (6: gh:#571, 7: #572, 8: #573,
    /// 9-11: #574); the panels list what each does not model.
    #[test]
    fn every_step_is_built() {
        assert!(WizardStep::ALL.iter().all(|s| s.implemented()));
    }
}
