//! # Steps 9 and 10: the multiphysics case, its inputs and its settings
//!
//! Two things live here, both plain data (no solver):
//!
//! 1. **The hand-off from Steps 7 and 8** ([`MultiphysicsInputs`]): Step 7's
//!    [`MeshSet`] and Step 8's [`MgxsSet`], held as they are (one definition
//!    each, gh:#591), with the check that a spatial solve can start from them.
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
//! **Status (2026-10-05, gh:#591).** What the coupled run (Step 10) actually
//! solves is stated by [`MultiphysicsSetup::elements`]: every piece is *in
//! model*, *simplified* or *NOT in model*, never silent. In short: the
//! porous-core thermal-hydraulics runs on an r-z multi-channel march built
//! from `tampines::pebble_bed` correlations; by default
//! ([`PowerShape::Diffusion`]) the power shape and `k` come from the GeN-Foam
//! port's multigroup diffusion eigenvalue solve on Step 7's neutronics mesh
//! with Step 8's constants at each cell's temperature, Picard-coupled to the
//! march through Step 7's maps; the prescribed J0 x cosine shape with lumped
//! feedback is kept as an explicit ablation ([`PowerShape::J0Cosine`]); the
//! structural side does not run.

use serde::{Deserialize, Serialize};

use super::meshes::{MeshRole, MeshSet};
use super::mgxs::MgxsSet;
use super::recipe::{Citation, ElementStatus, ModelElement};

/// The kovan artifact id of Step 9's recipe section.
pub const SECTION_ID: &str = "step-9";
/// Its heading.
pub const SECTION_HEADING: &str = "Step 9: Multiphysics case setup";

// ─── The hand-off from Steps 7 and 8 ────────────────────────────────────────

/// Everything Steps 7 and 8 hand to the coupled solve, **as they produced
/// it**: Step 7's [`MeshSet`] and Step 8's [`MgxsSet`]. There is no second
/// copy of either; the coupled run reads these types (gh:#591).
///
/// What Step 10 takes from them:
///
/// - the neutronics and thermal-hydraulics `polyMesh` folders
///   ([`super::meshes::MeshSummary::polymesh_dir`]), read with the GeN-Foam
///   port's own reader, their cellZones (= region ids) and
///   [`super::meshes::MeshSummary::cell_region`];
/// - the two maps neutronics → TH (power) and TH → neutronics
///   (temperature), [`super::meshes::MeshMapping::map`];
/// - the bed's place in the R-Z domain ([`super::meshes::RzDomain`]) and
///   which regions are the bed ([`super::meshes::RegionMap::is_bed`]);
/// - the GeN-Foam `nuclearData` Step 8 wrote
///   ([`MgxsSet::nuclear_data_path`]), read with
///   `outram_foam_appbuilder_lib::io::nuclear_data::read_nuclear_data` and
///   interpolated per cell by the port (upstream `xsVariables`, `TFuel` with
///   [`MgxsSet::law`]); and the Monte Carlo `k` of each state point, to
///   compare the diffusion eigenvalue with.
#[derive(Debug, Clone, PartialEq)]
pub struct MultiphysicsInputs {
    /// Step 7's meshes and maps.
    pub meshes: MeshSet,
    /// Step 8's cross sections.
    pub mgxs: MgxsSet,
}

impl MultiphysicsInputs {
    /// Whether a spatial neutronics solve can be set up from these inputs.
    /// Checks that the neutronics and TH meshes were written, that both maps
    /// between them exist, that Step 8 wrote its `nuclearData`, and that
    /// every region holding neutronics cells has constants at every state
    /// point. Returns what is missing otherwise.
    ///
    /// # Errors
    ///
    /// A sentence naming the first missing or inconsistent item.
    pub fn check_for_neutronics(&self) -> Result<(), String> {
        let m = &self.meshes;
        for role in [MeshRole::Neutronics, MeshRole::ThermalHydraulics] {
            let Some(s) = m.mesh(role) else {
                return Err(format!("no {} mesh from Step 7 (gh:#572)", role.name()));
            };
            if s.polymesh_dir.is_none() {
                return Err(format!(
                    "the {} mesh was not written as a polyMesh (gh:#572)",
                    role.name()
                ));
            }
        }
        for (from, to) in [
            (MeshRole::Neutronics, MeshRole::ThermalHydraulics),
            (MeshRole::ThermalHydraulics, MeshRole::Neutronics),
        ] {
            if m.mapping(from, to).is_none() {
                return Err(format!(
                    "no {} -> {} map from Step 7 (gh:#572)",
                    from.name(),
                    to.name()
                ));
            }
        }
        let x = &self.mgxs;
        if x.states.is_empty() {
            return Err("no state points from Step 8 (gh:#573)".into());
        }
        if x.n_groups() == 0 {
            return Err("zero energy groups".into());
        }
        if x.nuclear_data_path.is_none() {
            return Err("Step 8 did not write its nuclearData (gh:#573)".into());
        }
        let n = m.mesh(MeshRole::Neutronics).expect("checked above");
        for (gi, reg) in m.plan.regions.regions.iter().enumerate() {
            if !n.cell_region.contains(&gi) {
                continue;
            }
            for s in &x.states {
                let Some(r) = s.regions.iter().find(|r| r.region == reg.id) else {
                    return Err(format!(
                        "region {} holds neutronics cells but has no constants at {} K (gh:#573)",
                        reg.id, s.temperature_k
                    ));
                };
                if r.total.len() != x.n_groups() {
                    return Err(format!(
                        "region {} at {} K: not {} groups",
                        reg.id,
                        s.temperature_k,
                        x.n_groups()
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
    /// **The default (gh:#591).** Solved: the GeN-Foam port's multigroup
    /// diffusion k-eigenvalue (`DiffusionNeutronics`) on Step 7's neutronics
    /// mesh, with Step 8's constants evaluated at each cell's temperature,
    /// coupled to the thermal-hydraulics by Picard iteration through Step 7's
    /// maps. `k` is the eigenvalue; the lumped coefficient is unused. Needs
    /// [`MultiphysicsInputs`].
    Diffusion,
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

impl PowerShape {
    /// Whether the shape is solved by neutronics (needs Steps 7 and 8), as
    /// opposed to prescribed with lumped feedback (an ablation).
    #[must_use]
    pub fn is_solved(self) -> bool {
        matches!(self, Self::Diffusion)
    }
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

/// The neutronics side. `isothermal_coefficient_per_k`,
/// `reference_temperature_k` and `weighting` drive only the lumped feedback
/// of the prescribed-shape ablations; with [`PowerShape::Diffusion`] the
/// feedback is the cross sections' own temperature dependence.
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

fn default_power_relaxation() -> f64 {
    0.5
}
fn default_power_tolerance() -> f64 {
    1e-4
}
fn default_k_tolerance() -> f64 {
    1e-6
}

/// The coupling loop (Picard over flow split, properties, power and
/// feedback).
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
    /// [`PowerShape::Diffusion`] only: under-relaxation of the node power
    /// handed from the neutronics to the march, (0–1]. (Upstream GeN-Foam
    /// couples once per time step; this loop seeks a steady fixed point, so
    /// it relaxes.)
    #[serde(default = "default_power_relaxation")]
    pub power_relaxation: f64,
    /// … converged when no node's power moved by more than this fraction of
    /// the largest node power …
    #[serde(default = "default_power_tolerance")]
    pub power_tolerance: f64,
    /// … and `k` by less than this.
    #[serde(default = "default_k_tolerance")]
    pub k_tolerance: f64,
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
                origin: None,
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
                power_relaxation: 0.5,
                power_tolerance: 1e-4,
                k_tolerance: 1e-6,
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

    /// The input check names what Steps 7/8 have not supplied, and accepts
    /// a [`MeshSet`] + [`MgxsSet`] that carry everything Step 10 reads.
    #[test]
    fn neutronics_inputs_say_what_is_missing() {
        use crate::workbench::meshes::{
            CellType, MeshMapping, MeshPlan, MeshSummary, RegionMap,
        };
        use crate::workbench::mgxs::{InterpLaw, RegionXs, StateXs};
        let d = crate::workbench::meshes::tests::htr10_like();
        let plan = MeshPlan::default_for(&d);
        let bed = plan.regions.bed_first;
        let summary = |role: MeshRole, dir: Option<&str>| MeshSummary {
            role,
            cell_type: CellType::TetDual,
            cells: 1,
            points: 4,
            faces: 4,
            kinds: vec![],
            volume_cm3: 1.0,
            exact_cm3: 1.0,
            non_star_cells: 0,
            max_non_orthogonality_deg: 0.0,
            max_skewness: 0.0,
            patches: vec![],
            notes: vec![],
            regions: vec![],
            cell_region: vec![bed],
            polymesh_dir: dir.map(String::from),
            seconds: 0.0,
        };
        let map = |from, to| MeshMapping {
            from,
            to,
            weights: vec![vec![(0, 1.0)]],
            overlap_cm3: 1.0,
            to_coverage: 1.0,
            uncovered_cells: 0,
            fields: vec![],
            seconds: 0.0,
        };
        let mut i = MultiphysicsInputs {
            meshes: MeshSet {
                domain: d.clone(),
                plan: plan.clone(),
                meshes: vec![summary(MeshRole::Neutronics, None)],
                mappings: vec![],
            },
            mgxs: MgxsSet {
                edges_ev_desc: vec![2e7, 0.625, 1e-5],
                law: InterpLaw::LnT,
                states: vec![],
                nuclear_data_path: None,
                notes: vec![],
            },
        };
        assert!(i.check_for_neutronics().unwrap_err().contains("polyMesh"));
        i.meshes.meshes = vec![
            summary(MeshRole::Neutronics, Some("n")),
            summary(MeshRole::ThermalHydraulics, Some("t")),
        ];
        assert!(i.check_for_neutronics().unwrap_err().contains("map"));
        i.meshes.mappings = vec![
            map(MeshRole::Neutronics, MeshRole::ThermalHydraulics),
            map(MeshRole::ThermalHydraulics, MeshRole::Neutronics),
        ];
        assert!(i.check_for_neutronics().unwrap_err().contains("#573"));
        let rx = |id: &str| RegionXs {
            region: id.into(),
            volume_cm3: 1.0,
            flux: vec![1.0; 2],
            flux_rel_sigma: vec![0.0; 2],
            total: vec![0.3, 0.4],
            absorption: vec![0.001, 0.005],
            nu_fission: vec![0.0005, 0.008],
            kappa_fission: vec![1e-13, 1e-12],
            chi: vec![1.0, 0.0],
            scatter: vec![vec![0.29, 0.009], vec![0.0, 0.395]],
            rel_sigma_total: vec![0.0; 2],
            rel_sigma_absorption: vec![0.0; 2],
            rel_sigma_nu_fission: vec![0.0; 2],
        };
        i.mgxs.states.push(StateXs {
            temperature_k: 300.0,
            k: 1.0,
            k_sigma: 0.0,
            histories: 0,
            data_s: 0.0,
            transport_s: 0.0,
            regions: vec![rx("not_the_bed")],
        });
        i.mgxs.nuclear_data_path = Some("nuclearData".into());
        let e = i.check_for_neutronics().unwrap_err();
        assert!(e.contains(&plan.regions.regions[bed].id), "{e}");
        let _: &RegionMap = &plan.regions;
        i.mgxs.states[0].regions.push(rx(&plan.regions.regions[bed].id));
        assert!(i.check_for_neutronics().is_ok());
    }
}
