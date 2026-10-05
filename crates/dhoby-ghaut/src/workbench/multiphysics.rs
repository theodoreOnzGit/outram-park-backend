//! # Steps 9 and 10: the multiphysics case, its inputs and its settings
//!
//! Two things live here, both plain data (no solver):
//!
//! 1. **The hand-off from Steps 7 and 8** ([`MultiphysicsInputs`]): the meshes
//!    and the per-region group constants the coupled solve needs. Steps 7
//!    (meshes + mapping, gh:#572) and 8 (MGXS, gh:#573) are built in
//!    parallel with this module and define their own output types; the
//!    coordinator adapts those to these at merge time. **What Steps 7/8 must
//!    supply** is stated on each type below.
//! 2. **Step 9's case setup** ([`MultiphysicsSetup`]): boundary conditions,
//!    models and solver settings for the OUTRAM-Foam (GeN-Foam port) side, the
//!    neutronics side, the coupling loop and the `farrer-park` (FEM
//!    structural) side, prefilled for HTR-10 by the binary and saved in the
//!    recipe as its own kovan artifact, id `step-9` ([`MultiphysicsSetup::to_markdown_section`],
//!    [`MultiphysicsSetup::from_recipe_markdown`]).
//!
//! **Why a separate kovan section, not a field of [`super::recipe::Recipe`].**
//! Steps 6–8 are being added to the recipe by other work at the same time;
//! a self-contained `step-9` artifact appended after the others is read and
//! written through kovan's own parser exactly like the rest, and an older
//! recipe without it still loads (the section is optional, and
//! [`super::recipe::Recipe::from_markdown`] ignores artifacts it does not
//! know). It can be folded into `Recipe` once the steps settle.
//!
//! **Status (2026-10-05).** What the coupled run (Step 10) actually solves is
//! stated by [`MultiphysicsSetup::elements`]: every piece is *in model*,
//! *simplified* or *NOT in model*, never silent. In short: the porous-core
//! thermal-hydraulics runs on an r-z multi-channel march built from
//! `tampines::pebble_bed` correlations; the power shape is **prescribed**
//! (spatial diffusion on Step 8's MGXS is not wired); k comes from a lumped
//! temperature feedback; the structural side does not run.

use serde::{Deserialize, Serialize};

use super::recipe::{Citation, ElementStatus, ModelElement};

/// The kovan artifact id of Step 9's recipe section.
pub const SECTION_ID: &str = "step-9";
/// Its heading.
pub const SECTION_HEADING: &str = "Step 9: Multiphysics case setup";

// ─── The hand-off from Steps 7 and 8 ────────────────────────────────────────

/// Which physics a mesh serves (GeN-Foam's three-mesh layout).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeshRole {
    /// Neutronics (diffusion / SP3) mesh.
    Neutronics,
    /// Thermal-hydraulics (porous-medium fluid + structure) mesh.
    ThermalHydraulics,
    /// Structural FEM mesh (`farrer-park`, Tet4).
    Structural,
}

/// A mesh Step 7 built. **Step 7 must supply** one per role: where it is on
/// disk (an OpenFOAM `polyMesh` directory for the FV meshes, the FEM mesh
/// file for the structural one), its cell count, and the name of every
/// region (cell zone) it carries, in the names [`RegionConstants::region`]
/// uses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshHandle {
    /// Which physics it serves.
    pub role: MeshRole,
    /// Path of the mesh on disk (a `polyMesh` folder or FEM mesh file).
    pub path: String,
    /// Number of cells (elements).
    pub cells: usize,
    /// Cell-zone (region) names present in the mesh.
    pub regions: Vec<String>,
    /// How it was made, for the record ("tet-dual polyhedral + 3 boundary
    /// layers", "Tet4").
    pub kind: String,
}

/// One state point of one region's few-group constants. **Step 8 must
/// supply** these from Monte Carlo at each state point, in cm and 1/cm,
/// groups ordered fast to thermal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupConstantsAtT {
    /// The fuel (Doppler) temperature of the state point \[K\].
    pub temperature_k: f64,
    /// Diffusion coefficient per group \[cm\].
    pub diffusion_cm: Vec<f64>,
    /// Absorption cross section per group \[1/cm\].
    pub sigma_a_per_cm: Vec<f64>,
    /// nu × fission cross section per group \[1/cm\].
    pub nu_sigma_f_per_cm: Vec<f64>,
    /// Energy release per fission × fission cross section per group
    /// \[J/cm\], for the power density.
    pub kappa_sigma_f_j_per_cm: Vec<f64>,
    /// Fission spectrum per group (sums to 1).
    pub chi: Vec<f64>,
    /// Group-to-group scattering, `sigma_s[from][to]` \[1/cm\], P0.
    pub sigma_s_per_cm: Vec<Vec<f64>>,
}

/// What a region is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegionKind {
    /// Pebble-bed core (fuel + moderator pebbles + helium, homogenised).
    Core,
    /// Graphite reflector.
    Reflector,
    /// Boronated carbon bricks.
    BoronatedCarbon,
    /// Cavity / plenum (helium).
    Cavity,
    /// Anything else (named in the region name).
    Other,
}

/// A neutronics region and its constants against temperature. **Step 8 must
/// supply** one per region of the neutronics mesh, with at least one state
/// point (two or more for temperature feedback; Step 8 interpolates in
/// `ln T` by default).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegionConstants {
    /// Cell-zone name, as in [`MeshHandle::regions`].
    pub region: String,
    /// What it is.
    pub kind: RegionKind,
    /// State points, ascending in temperature.
    pub table: Vec<GroupConstantsAtT>,
}

/// Everything Steps 7 and 8 hand to the coupled solve.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MultiphysicsInputs {
    /// One mesh per role.
    pub meshes: Vec<MeshHandle>,
    /// Number of energy groups (every region has the same).
    pub groups: usize,
    /// Per-region constants.
    pub regions: Vec<RegionConstants>,
}

impl MultiphysicsInputs {
    /// The mesh for `role`, if Step 7 supplied one.
    #[must_use]
    pub fn mesh(&self, role: MeshRole) -> Option<&MeshHandle> {
        self.meshes.iter().find(|m| m.role == role)
    }

    /// Whether a spatial neutronics solve could be set up from these inputs:
    /// a neutronics mesh, at least one region, every table non-empty and
    /// every vector `groups` long. Returns what is missing otherwise.
    ///
    /// # Errors
    ///
    /// A sentence naming the first missing or inconsistent item.
    pub fn check_for_neutronics(&self) -> Result<(), String> {
        if self.mesh(MeshRole::Neutronics).is_none() {
            return Err("no neutronics mesh from Step 7 (gh:#572)".into());
        }
        if self.regions.is_empty() {
            return Err("no region constants from Step 8 (gh:#573)".into());
        }
        let g = self.groups;
        if g == 0 {
            return Err("zero energy groups".into());
        }
        for r in &self.regions {
            if r.table.is_empty() {
                return Err(format!("region {} has no state points", r.region));
            }
            for s in &r.table {
                let ok = s.diffusion_cm.len() == g
                    && s.sigma_a_per_cm.len() == g
                    && s.nu_sigma_f_per_cm.len() == g
                    && s.kappa_sigma_f_j_per_cm.len() == g
                    && s.chi.len() == g
                    && s.sigma_s_per_cm.len() == g
                    && s.sigma_s_per_cm.iter().all(|row| row.len() == g);
                if !ok {
                    return Err(format!(
                        "region {} at {} K: a vector is not {g} groups long",
                        r.region, s.temperature_k
                    ));
                }
            }
        }
        Ok(())
    }
}

// ─── Step 9: the case setup ─────────────────────────────────────────────────

/// Boundary condition on the core's side wall (the bed / side-reflector
/// interface).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallCondition {
    /// No heat crosses the wall (zero gradient).
    Adiabatic,
}

/// The OUTRAM-Foam side: porous-medium thermal-hydraulics of the pebble bed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FoamSide {
    /// Pebble-bed radius \[cm\].
    pub core_radius_cm: f64,
    /// Pebble-bed height \[cm\].
    pub core_height_cm: f64,
    /// Volumetric filling fraction of the balls (1 − porosity).
    pub filling_fraction: f64,
    /// Pebble diameter \[cm\].
    pub pebble_diameter_cm: f64,
    /// Share of the pebbles that carry fuel (equilibrium core: 1).
    pub fuel_pebble_fraction: f64,
    /// Inlet BC: helium temperature at the top of the bed \[°C\].
    pub inlet_temperature_c: f64,
    /// Total primary helium mass flow \[kg/s\].
    pub total_mass_flow_kg_s: f64,
    /// Inlet BC: the part of it that passes through the pebble bed \[kg/s\];
    /// the rest bypasses (control-rod holes, discharge tube, gaps) and is
    /// mixed back at the outlet at the inlet temperature.
    pub core_mass_flow_kg_s: f64,
    /// Outlet BC: system pressure at the bed exit \[MPa\].
    pub outlet_pressure_mpa: f64,
    /// Side wall.
    pub wall: WallCondition,
    /// Flow direction: true = downward (top inlet).
    pub flow_downward: bool,
    /// Fast fluence for the graphite conductivity, 10^25 n/m² (0 = fresh).
    pub fast_fluence_1e25_per_m2: f64,
    /// Models used (named for the record): friction, heat transfer,
    /// effective conductivity, pebble conduction, helium properties.
    pub friction_model: String,
    /// Pebble-to-helium heat transfer correlation.
    pub heat_transfer_model: String,
    /// Pebble internal conduction model.
    pub pebble_model: String,
    /// Helium property source.
    pub helium_properties: String,
    /// Radial rings (equal area).
    pub radial_rings: usize,
    /// Axial nodes.
    pub axial_nodes: usize,
}

/// How the power is distributed in space.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PowerShape {
    /// The bare-cylinder fundamental mode `J0(2.405 r/R_e) cos(pi z/H_e)`
    /// with one extrapolation length `delta` on the radius and on each axial
    /// end (`R_e = R + delta`, `H_e = H + 2 delta`). `delta` is solved so the
    /// peak-to-mean power density equals `peak_to_mean`, a published input.
    J0Cosine {
        /// Peak / core-mean power density.
        peak_to_mean: f64,
    },
    /// Uniform power density (an ablation).
    Uniform,
}

/// How the lumped feedback averages the core temperature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackWeighting {
    /// Weighted by local power (first-order perturbation, one group:
    /// importance ∝ flux ∝ power).
    Power,
    /// Plain volume average.
    Volume,
}

/// The neutronics side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NeutronicsSide {
    /// Core thermal power \[MW\].
    pub thermal_power_mw: f64,
    /// Spatial power distribution.
    pub shape: PowerShape,
    /// Isothermal temperature coefficient of reactivity \[1/K\].
    pub isothermal_coefficient_per_k: f64,
    /// Temperature at which the cold reference state holds \[K\] (Step 5's
    /// data temperature).
    pub reference_temperature_k: f64,
    /// How the core temperature for feedback is averaged.
    pub weighting: FeedbackWeighting,
}

/// The coupling loop (Picard over flow split, properties and feedback).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CouplingSettings {
    /// Maximum outer iterations.
    pub max_iterations: usize,
    /// Converged when the relative spread of ring pressure drops is below
    /// this …
    pub pressure_tolerance: f64,
    /// … and no temperature moved more than this between iterations \[K\].
    pub temperature_tolerance_k: f64,
    /// Under-relaxation of the ring flow update (0–1].
    pub flow_relaxation: f64,
}

/// The `farrer-park` (MOOSE-port FEM) side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructuralSide {
    /// Whether a structural solve runs. Always false in this build: see
    /// [`MultiphysicsSetup::elements`].
    pub enabled: bool,
    /// Element type (Tet4; never polyhedral for stress).
    pub element: String,
    /// Constitutive model.
    pub material_model: String,
    /// What it would load the structure with.
    pub load: String,
}

/// Step 9: the whole case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultiphysicsSetup {
    /// Porous-medium TH.
    pub foam: FoamSide,
    /// Neutronics.
    pub neutronics: NeutronicsSide,
    /// Coupling loop.
    pub coupling: CouplingSettings,
    /// Structural FEM.
    pub structural: StructuralSide,
    /// What the run models, simplifies, or leaves out.
    #[serde(default, rename = "element")]
    pub elements: Vec<ModelElement>,
    /// Sources.
    #[serde(default, rename = "cite", skip_serializing_if = "Vec::is_empty")]
    pub cites: Vec<Citation>,
}

/// Why Step 9's section could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupError {
    /// kovan reported a malformed artifact.
    Kovan(String),
    /// The section has no ` ```toml ` settings fence.
    MissingSettings,
    /// The settings do not match the schema.
    Settings(String),
    /// Serialising failed.
    Write(String),
}

impl std::fmt::Display for SetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Kovan(m) => write!(f, "kovan could not read an artifact: {m}"),
            Self::MissingSettings => {
                write!(f, "section `{SECTION_ID}` has no ```toml settings block")
            }
            Self::Settings(m) => write!(f, "section `{SECTION_ID}` settings are invalid: {m}"),
            Self::Write(m) => write!(f, "could not write section `{SECTION_ID}`: {m}"),
        }
    }
}

impl std::error::Error for SetupError {}

/// The first ` ```toml ` fence in a body.
fn settings_fence(body: &str) -> Option<String> {
    let mut lines = body.lines();
    while let Some(line) = lines.next() {
        if line.trim_start().starts_with("```toml") {
            let mut out = String::new();
            for inner in lines.by_ref() {
                if inner.trim_start().starts_with("```") {
                    return Some(out);
                }
                out.push_str(inner);
                out.push('\n');
            }
            return None;
        }
    }
    None
}

impl MultiphysicsSetup {
    /// Render the section as one kovan artifact (`# Step 9: …`, id
    /// [`SECTION_ID`]), to append after the recipe's other sections.
    ///
    /// # Errors
    ///
    /// [`SetupError::Write`] if serialising fails.
    pub fn to_markdown_section(&self, timestamp: &str) -> Result<String, SetupError> {
        let toml_text =
            toml::to_string_pretty(self).map_err(|e| SetupError::Write(e.to_string()))?;
        let payload = kovan::artifact::ArtifactToml {
            kovan: kovan::artifact::ArtifactMeta {
                id: SECTION_ID.to_string(),
                kind: kovan::artifact::ArtifactKind::Note,
                created: timestamp.to_string(),
                modified: timestamp.to_string(),
                reviewed: None,
            },
            source: None,
            classification: Default::default(),
            extraction: None,
            relation: None,
            connections: Vec::new(),
        };
        let body = format!(
            "Boundary conditions, models and solver settings of the coupled case: the \
             OUTRAM-Foam porous-core side, neutronics, the coupling loop and the \
             farrer-park structural side. Each part of the model is listed as in model, \
             simplified or NOT in model.\n\n```toml\n{toml_text}```\n"
        );
        kovan::artifact::render_artifact_block(1, SECTION_HEADING, &payload, &body)
            .map_err(SetupError::Write)
    }

    /// Read Step 9 from a whole recipe, through kovan's parser. `None` when
    /// the recipe has no `step-9` section (an older recipe).
    #[must_use]
    pub fn from_recipe_markdown(markdown: &str) -> Option<Result<Self, SetupError>> {
        let doc = kovan::artifact::parse_document(markdown);
        let a = doc.get(SECTION_ID)?;
        if let Some(p) = doc.problems.first() {
            return Some(Err(SetupError::Kovan(format!("{p:?}"))));
        }
        let Some(text) = settings_fence(&a.body) else {
            return Some(Err(SetupError::MissingSettings));
        };
        Some(toml::from_str(&text).map_err(|e| SetupError::Settings(e.to_string())))
    }

    /// Replace (or append) the `step-9` section of a rendered recipe.
    ///
    /// # Errors
    ///
    /// As [`Self::to_markdown_section`].
    pub fn write_into(&self, recipe_markdown: &str, timestamp: &str) -> Result<String, SetupError> {
        let mut out = match recipe_markdown.find(&format!("# {SECTION_HEADING}")) {
            Some(i) => recipe_markdown[..i].to_string(),
            None => recipe_markdown.to_string(),
        };
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&self.to_markdown_section(timestamp)?);
        out.push('\n');
        Ok(out)
    }

    /// Whether every listed element is in the model (never true in this build).
    #[must_use]
    pub fn fully_modelled(&self) -> bool {
        self.elements
            .iter()
            .all(|e| e.status == ElementStatus::InModel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> MultiphysicsSetup {
        MultiphysicsSetup {
            foam: FoamSide {
                core_radius_cm: 90.0,
                core_height_cm: 197.0,
                filling_fraction: 0.61,
                pebble_diameter_cm: 6.0,
                fuel_pebble_fraction: 1.0,
                inlet_temperature_c: 250.0,
                total_mass_flow_kg_s: 4.32,
                core_mass_flow_kg_s: 3.77,
                outlet_pressure_mpa: 3.0,
                wall: WallCondition::Adiabatic,
                flow_downward: true,
                fast_fluence_1e25_per_m2: 0.0,
                friction_model: "KTA".into(),
                heat_transfer_model: "Wakao".into(),
                pebble_model: "two-zone".into(),
                helium_properties: "coolprop".into(),
                radial_rings: 5,
                axial_nodes: 40,
            },
            neutronics: NeutronicsSide {
                thermal_power_mw: 10.0,
                shape: PowerShape::J0Cosine {
                    peak_to_mean: 1.285,
                },
                isothermal_coefficient_per_k: -1.4e-4,
                reference_temperature_k: 293.6,
                weighting: FeedbackWeighting::Power,
            },
            coupling: CouplingSettings {
                max_iterations: 60,
                pressure_tolerance: 1e-4,
                temperature_tolerance_k: 0.01,
                flow_relaxation: 0.7,
            },
            structural: StructuralSide {
                enabled: false,
                element: "Tet4".into(),
                material_model: "linear elastic".into(),
                load: "thermal".into(),
            },
            elements: vec![ModelElement {
                name: "power shape".into(),
                status: ElementStatus::Simplified,
                note: "prescribed".into(),
            }],
            cites: vec![Citation {
                field: "foam.inlet_temperature_c".into(),
                citekey: "gao2002htr10th".into(),
                page: None,
                what: "Table 2".into(),
            }],
        }
    }

    /// Emit → parse gives back the same settings, through kovan's parser,
    /// and the section survives being appended to (and replaced in) a recipe.
    #[test]
    fn step_9_round_trips_through_kovan_markdown() {
        let s = sample();
        let md = s.to_markdown_section("2026-10-05T00:00:00Z").expect("emit");
        let back = MultiphysicsSetup::from_recipe_markdown(&md)
            .expect("present")
            .expect("parse");
        assert_eq!(back, s);
        let doc = kovan::artifact::parse_document(&md);
        assert!(doc.problems.is_empty(), "{:?}", doc.problems);
        // Replace in place: writing twice keeps one section.
        let once = s
            .write_into("# Other\n\ntext\n", "2026-10-05T00:00:00Z")
            .expect("w1");
        let mut s2 = s.clone();
        s2.neutronics.thermal_power_mw = 12.0;
        let twice = s2.write_into(&once, "2026-10-05T00:00:00Z").expect("w2");
        assert_eq!(twice.matches(SECTION_HEADING).count(), 1);
        let got = MultiphysicsSetup::from_recipe_markdown(&twice)
            .expect("present")
            .expect("parse");
        assert_eq!(got.neutronics.thermal_power_mw, 12.0);
        assert!(twice.starts_with("# Other"));
    }

    /// A recipe without the section reads as `None`, not an error.
    #[test]
    fn an_older_recipe_has_no_step_9() {
        assert!(MultiphysicsSetup::from_recipe_markdown("# Nothing\n").is_none());
    }

    /// The input check names what Steps 7/8 have not supplied.
    #[test]
    fn neutronics_inputs_say_what_is_missing() {
        let mut i = MultiphysicsInputs::default();
        assert!(i.check_for_neutronics().unwrap_err().contains("#572"));
        i.meshes.push(MeshHandle {
            role: MeshRole::Neutronics,
            path: "n".into(),
            cells: 10,
            regions: vec!["core".into()],
            kind: "tet-dual".into(),
        });
        assert!(i.check_for_neutronics().unwrap_err().contains("#573"));
        i.groups = 2;
        i.regions.push(RegionConstants {
            region: "core".into(),
            kind: RegionKind::Core,
            table: vec![GroupConstantsAtT {
                temperature_k: 300.0,
                diffusion_cm: vec![1.0, 1.0],
                sigma_a_per_cm: vec![0.01, 0.1],
                nu_sigma_f_per_cm: vec![0.01, 0.2],
                kappa_sigma_f_j_per_cm: vec![1e-13, 1e-12],
                chi: vec![1.0, 0.0],
                sigma_s_per_cm: vec![vec![0.0, 0.02], vec![0.0, 0.0]],
            }],
        });
        assert!(i.check_for_neutronics().is_ok());
        i.regions[0].table[0].chi.pop();
        assert!(i.check_for_neutronics().is_err());
    }
}
