//! # Recipes: the workbench's input decks, as kovan markdown
//!
//! A **recipe** is everything a wizard session chose, saved so that it can be
//! loaded again: the wizard and an input deck are two views of one thing
//! (maintainer, 2026-10-05). The format is decided: **kovan-compatible
//! markdown with embedded TOML** (gh:#563). Plain TOML is kept for lighter
//! data transfer, such as the reactivity map (gh:#571, #576).
//!
//! ## The format
//!
//! One kovan artifact per section: a level-1 `#` heading, immediately followed
//! by a ` ```toml ` fence holding the `[kovan]` table (id, kind `note`,
//! timestamps). That is exactly what [`kovan::artifact::parse_document`] reads,
//! and it is parsed by kovan, not by a second parser here. The artifact's
//! **body**, which kovan keeps verbatim, carries prose for the human and one
//! more ` ```toml ` fence holding the section's settings. kovan treats a fence
//! without `[kovan]` as an ordinary code block, so the settings never confuse
//! it.
//!
//! ````markdown
//! # Step 2: Pebble design
//!
//! ```toml
//! [kovan]
//! id = "step-2"
//! kind = "note"
//! created = "2026-10-05T12:00:00Z"
//! modified = "2026-10-05T12:00:00Z"
//! ```
//!
//! One design per pebble type in the mix … (prose, ignored by the parser)
//!
//! ```toml
//! interstitial = "helium"
//! [[pebble]]
//! kind = "fuel"
//! outer_radius_cm = 3.0
//! …
//! [[cite]]
//! field = "pebble.fuel.outer_radius_cm"
//! citekey = "…"
//! page = 3
//! what = "…"
//! ```
//! ````
//!
//! Section ids are fixed (`recipe`, `step-0` … `step-5`, `review`), so a
//! reworded heading does not break a recipe (kovan §40: the id, not the
//! heading, is the identity).
//!
//! ## Units
//!
//! The file stores plain numbers whose unit is in the key (`_cm`, `_k`,
//! `_g_per_cm3`). Where a quantity leaves this module for a solver, use the
//! `uom` accessors (e.g. [`NuclearDataStep::temperature`]).
//!
//! ## Citations
//!
//! Every prefilled value can carry a [`Citation`]: a kovan citekey, a page and
//! a sentence saying what the source states there (a paraphrase, not a copied
//! passage). The literature pane resolves the citekey against the corpus. A
//! citekey the corpus does not hold is reported as such, never hidden.

use serde::{Deserialize, Serialize};
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

use super::catalogue::{Mode, ReactorType};

/// The `format` string every recipe header carries.
pub const FORMAT: &str = "dhoby-ghaut-recipe";
/// The format version this build writes and reads.
pub const VERSION: u32 = 1;

/// Where a prefilled value came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Citation {
    /// The setting it supports, as a dotted path (`"pebble.fuel.outer_radius_cm"`).
    pub field: String,
    /// kovan citekey of the source.
    pub citekey: String,
    /// 1-based page, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    /// What the source states there, in our words.
    pub what: String,
}

/// Whether a part of the reactor is in the model the solver sees.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementStatus {
    /// Built into the geometry.
    InModel,
    /// Represented in a simplified form (homogenised, smeared, ...): the note
    /// says how.
    Simplified,
    /// Part of the real reactor but not in the model; the note says why and
    /// which issue tracks it.
    NotInModel,
}

impl ElementStatus {
    /// Badge text.
    #[must_use]
    pub fn badge(self) -> &'static str {
        match self {
            Self::InModel => "in model",
            Self::Simplified => "simplified",
            Self::NotInModel => "NOT in model",
        }
    }
}

/// One named part of the reactor and its status in the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelElement {
    /// What it is ("refuelling chute").
    pub name: String,
    /// Whether the solver sees it.
    pub status: ElementStatus,
    /// How it is represented, or why it is missing.
    pub note: String,
}

/// The recipe header (`# Recipe`, id `recipe`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecipeHeader {
    /// Always [`FORMAT`].
    pub format: String,
    /// Always [`VERSION`] for files this build writes.
    pub version: u32,
    /// Human title.
    pub title: String,
    /// [`ReactorType::key`].
    pub reactor: String,
    /// Basic or Advanced.
    pub mode: Mode,
    /// The pre-built model this recipe started from, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    /// True once any value differs from the preset: the recipe is then
    /// "derived from" the preset and inherits none of its V&V standing.
    #[serde(default)]
    pub edited: bool,
    /// The preset's V&V status, stated as its own record states it.
    pub vv_status: String,
}

impl RecipeHeader {
    /// The reactor type, if the key is known.
    #[must_use]
    pub fn reactor_type(&self) -> Option<ReactorType> {
        ReactorType::from_key(&self.reactor)
    }
}

/// Step 0: the ENDF library.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NuclearDataStep {
    /// Folder of unzipped ENDF tapes.
    pub endf_dir: String,
    /// Evaluated library the run uses ("ENDF/B-VIII.0").
    pub library: String,
    /// Temperature every nuclide is reconstructed and broadened at \[K\].
    pub temperature_k: f64,
    /// Sources.
    #[serde(default, rename = "cite", skip_serializing_if = "Vec::is_empty")]
    pub cites: Vec<Citation>,
}

impl NuclearDataStep {
    /// The data temperature as a `uom` quantity.
    #[must_use]
    pub fn temperature(&self) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(self.temperature_k)
    }
}

/// The pebble-type mix of the bed (Step 1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PebbleMix {
    /// Fuel pebbles, fraction of all pebbles.
    pub fuel: f64,
    /// Moderator (dummy graphite) pebbles.
    pub moderator: f64,
    /// Fertile pebbles.
    pub fertile: f64,
    /// Poison pebbles.
    pub poison: f64,
}

impl PebbleMix {
    /// Sum of the fractions (should be 1).
    #[must_use]
    pub fn total(&self) -> f64 {
        self.fuel + self.moderator + self.fertile + self.poison
    }
}

/// Step 1: the DEM pebble bed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PebbleBedStep {
    /// How the bed is packed, in words ("Şeker hexagonal two-ball cell").
    pub packing: String,
    /// Radial ring count of the tiling.
    pub rings: usize,
    /// Loaded layers (sets the bed height).
    pub layers: usize,
    /// The packing's filling fraction the model is built to.
    pub filling_fraction: f64,
    /// Pebble-type mix.
    pub mix: PebbleMix,
    /// Where the bed comes from: the preset lattice, or a fresh DEM pour.
    /// Absent in recipes written before 2026-10-05, which read as the lattice.
    #[serde(default)]
    pub source: BedSource,
    /// The DEM pour's settings (used when `source` is `Dem`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dem: Option<DemPour>,
    /// Sources.
    #[serde(default, rename = "cite", skip_serializing_if = "Vec::is_empty")]
    pub cites: Vec<Citation>,
}

/// Where Step 1's pebble bed comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BedSource {
    /// The preset's lattice bed (HTR-10: Şeker's 13-ball hexagonal cell).
    #[default]
    Lattice,
    /// A fresh DEM pour of `DemPour::n_pebbles` pebbles, settled under
    /// gravity (`outram_park_fork_liggghts::htr10_fill`).
    Dem,
}

/// The settings of a fresh DEM pour (Step 1). Contact values default to the
/// liggghts V&V § 4.9 setting; µ there carries no specific citation, which the
/// workbench shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DemPour {
    /// Pebbles to pour.
    pub n_pebbles: usize,
    /// Sliding friction µ \[-\].
    pub friction: f64,
    /// Rolling friction µ_r \[-\].
    pub rolling_friction: f64,
    /// Young's modulus \[Pa\] (softened, see the liggghts V&V § 4.7).
    pub youngs_modulus_pa: f64,
    /// Seed of the initial loose lattice's jitter.
    pub seed: u64,
    /// Steps the pour took to settle, once it has run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settled_steps: Option<usize>,
    /// Whole-core filling fraction it settled to, once it has run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phi_whole_core: Option<f64>,
}

/// One TRISO particle design.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Triso {
    /// Kernel material ("UO2").
    pub kernel: String,
    /// U-235 enrichment, weight fraction.
    pub enrichment: f64,
    /// Kernel radius \[cm\].
    pub kernel_radius_cm: f64,
    /// Coating layers, inside out: name and outer radius \[cm\].
    pub layers: Vec<(String, f64)>,
    /// Particles per fuel pebble.
    pub particles_per_pebble: usize,
}

/// One pebble type's design (Step 2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PebbleDesign {
    /// "fuel", "moderator", "fertile" or "poison".
    pub kind: String,
    /// Pebble outer radius \[cm\].
    pub outer_radius_cm: f64,
    /// Fuelled-zone radius \[cm\] (fuel pebbles).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuel_zone_radius_cm: Option<f64>,
    /// Matrix / shell material.
    pub matrix: String,
    /// TRISO particle (fuel pebbles).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub triso: Option<Triso>,
}

/// Step 2: the pebble designs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PebbleDesignStep {
    /// What fills the voids between pebbles ("helium").
    pub interstitial: String,
    /// One design per type in the mix.
    #[serde(rename = "pebble")]
    pub pebbles: Vec<PebbleDesign>,
    /// Sources.
    #[serde(default, rename = "cite", skip_serializing_if = "Vec::is_empty")]
    pub cites: Vec<Citation>,
}

/// Step 3: reflector and internals.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReflectorStep {
    /// Core (bed) radius \[cm\].
    pub core_radius_cm: f64,
    /// Outer radius of the reflector \[cm\].
    pub reflector_outer_cm: f64,
    /// Height of the whole model \[cm\].
    pub model_height_cm: f64,
    /// Parts of the reflector and internals, with their status.
    #[serde(rename = "element")]
    pub elements: Vec<ModelElement>,
    /// Sources.
    #[serde(default, rename = "cite", skip_serializing_if = "Vec::is_empty")]
    pub cites: Vec<Citation>,
}

/// Step 4: inserts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InsertStep {
    /// Number of control rods.
    pub control_rods: usize,
    /// B4C ring inner radius \[cm\].
    pub b4c_inner_radius_cm: f64,
    /// B4C ring outer radius \[cm\].
    pub b4c_outer_radius_cm: f64,
    /// B4C density \[g/cm³\].
    pub b4c_density_g_per_cm3: f64,
    /// Parts placed in the borings, with their status.
    #[serde(rename = "element")]
    pub elements: Vec<ModelElement>,
    /// Sources.
    #[serde(default, rename = "cite", skip_serializing_if = "Vec::is_empty")]
    pub cites: Vec<Citation>,
}

/// The geometry review gate.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ReviewGate {
    /// The slices a human looked at before Step 5 (names of the views).
    #[serde(default)]
    pub viewed: Vec<String>,
    /// When the human confirmed (RFC 3339), if they did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmed_at: Option<String>,
}

impl ReviewGate {
    /// Whether Step 5 may run.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.confirmed_at.is_some()
    }
}

/// One saved k_eff run (Step 5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    /// Run label ("Run 1").
    pub label: String,
    /// Control-rod insertion during the run, fraction (0 = withdrawn).
    pub rod_insertion: f64,
    /// Data temperature \[K\].
    pub temperature_k: f64,
    /// Neutrons per generation.
    pub particles: usize,
    /// Inactive generations.
    pub inactive: usize,
    /// Active generations.
    pub active: usize,
    /// Seed.
    pub seed: u64,
    /// k_eff, mean over active generations.
    pub k: f64,
    /// Within-run 1σ of `k`.
    pub sigma: f64,
    /// Wall-clock seconds of transport.
    pub transport_s: f64,
}

/// Step 5: Monte Carlo settings and saved runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonteCarloStep {
    /// Neutrons per generation.
    pub particles: usize,
    /// Inactive generations.
    pub inactive: usize,
    /// Active generations.
    pub active: usize,
    /// Master seed.
    pub seed: u64,
    /// Worker threads.
    pub threads: usize,
    /// Control-rod insertion for the next run, fraction (0 = withdrawn).
    pub rod_insertion: f64,
    /// Energy groups of the spectrum tally, per decade.
    pub spectrum_bins_per_decade: usize,
    /// Physics switched off for a run, by name. Empty means everything the data
    /// carries is applied (the workspace default-ON rule); anything listed is
    /// shown as an explicit ablation.
    #[serde(default)]
    pub ablations: Vec<String>,
    /// Runs made so far.
    #[serde(default, rename = "run")]
    pub runs: Vec<RunRecord>,
    /// Sources.
    #[serde(default, rename = "cite", skip_serializing_if = "Vec::is_empty")]
    pub cites: Vec<Citation>,
}

/// A whole recipe: header, Steps 0–5 and the review gate.
#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    /// Header.
    pub header: RecipeHeader,
    /// Step 0.
    pub nuclear_data: NuclearDataStep,
    /// Step 1.
    pub pebble_bed: PebbleBedStep,
    /// Step 2.
    pub pebble_design: PebbleDesignStep,
    /// Step 3.
    pub reflector: ReflectorStep,
    /// Step 4.
    pub inserts: InsertStep,
    /// The review gate.
    pub review: ReviewGate,
    /// Step 5.
    pub monte_carlo: MonteCarloStep,
}

/// Why a recipe could not be read or written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipeError {
    /// kovan reported a malformed artifact.
    Kovan(String),
    /// A required section (by kovan id) is missing.
    MissingSection(&'static str),
    /// A section has no settings fence in its body.
    MissingSettings(&'static str),
    /// A section's settings do not match the schema.
    Settings {
        /// Section id.
        section: &'static str,
        /// The TOML error.
        message: String,
    },
    /// The header names another format or a newer version.
    Format(String),
    /// Serialising failed (cannot happen for these types; kept so callers
    /// need no `unwrap`).
    Write(String),
}

impl std::fmt::Display for RecipeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Kovan(m) => write!(f, "kovan could not read an artifact: {m}"),
            Self::MissingSection(id) => write!(f, "the recipe has no `{id}` section"),
            Self::MissingSettings(id) => {
                write!(
                    f,
                    "section `{id}` has no ```toml settings block in its body"
                )
            }
            Self::Settings { section, message } => {
                write!(f, "section `{section}` settings are invalid: {message}")
            }
            Self::Format(m) => write!(f, "not a readable recipe: {m}"),
            Self::Write(m) => write!(f, "could not write the recipe: {m}"),
        }
    }
}

impl std::error::Error for RecipeError {}

/// Section ids, headings and the prose written under each.
const SECTIONS: [(&str, &str, &str); 8] = [
    (
        "recipe",
        "Recipe",
        "A Dhoby Ghaut recipe: the input deck of a guided high-fidelity build. \
         Load it in the workbench to reopen the build, or read it as a record of \
         what was chosen and why. Research, education and V&V only.",
    ),
    (
        "step-0",
        "Step 0: Nuclear data library",
        "The folder of evaluated tapes the run reads, the library and the data \
         temperature.",
    ),
    (
        "step-1",
        "Step 1: DEM pebble bed construction",
        "How the pebbles pack, how high the bed is loaded and the pebble-type mix.",
    ),
    (
        "step-2",
        "Step 2: Pebble design",
        "One design per pebble type in the mix. The voids between pebbles are the \
         interstitial coolant.",
    ),
    (
        "step-3",
        "Step 3: Reflector and internals",
        "Reflector, boronated bricks, borings, chutes, plenums and risers, each \
         with its status in the model.",
    ),
    (
        "step-4",
        "Step 4: Inserts",
        "What fills the borings, each with its status in the model.",
    ),
    (
        "review",
        "Review gate: geometry",
        "The assembled-geometry slices a human looked at before Monte Carlo ran.",
    ),
    (
        "step-5",
        "Step 5: Monte Carlo",
        "Run settings, the ablations (if any) and the runs made.",
    ),
];

/// The first ` ```toml ` fence in an artifact body: the section's settings.
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

impl Recipe {
    /// Render the recipe as kovan markdown. `timestamp` (RFC 3339) stamps
    /// every section's `created`/`modified`.
    ///
    /// # Errors
    ///
    /// [`RecipeError::Write`] if a section fails to serialise.
    pub fn to_markdown(&self, timestamp: &str) -> Result<String, RecipeError> {
        fn ser<T: Serialize>(v: &T) -> Result<String, RecipeError> {
            toml::to_string_pretty(v).map_err(|e| RecipeError::Write(e.to_string()))
        }
        let settings = [
            ser(&self.header)?,
            ser(&self.nuclear_data)?,
            ser(&self.pebble_bed)?,
            ser(&self.pebble_design)?,
            ser(&self.reflector)?,
            ser(&self.inserts)?,
            ser(&self.review)?,
            ser(&self.monte_carlo)?,
        ];
        let mut out = String::new();
        for ((id, heading, prose), toml_text) in SECTIONS.iter().zip(settings) {
            let payload = kovan::artifact::ArtifactToml {
                kovan: kovan::artifact::ArtifactMeta {
                    id: (*id).to_string(),
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
            let body = format!("{prose}\n\n```toml\n{toml_text}```\n");
            let heading = if *id == "recipe" {
                format!("Recipe: {}", self.header.title)
            } else {
                (*heading).to_string()
            };
            let block = kovan::artifact::render_artifact_block(1, &heading, &payload, &body)
                .map_err(RecipeError::Write)?;
            out.push_str(&block);
            out.push('\n');
        }
        Ok(out)
    }

    /// Read a recipe from kovan markdown, through kovan's own parser.
    ///
    /// # Errors
    ///
    /// Any [`RecipeError`]: a malformed artifact, a missing section or
    /// settings block, settings that do not match the schema, or a header
    /// naming another format or a newer version.
    pub fn from_markdown(markdown: &str) -> Result<Self, RecipeError> {
        let doc = kovan::artifact::parse_document(markdown);
        if let Some(p) = doc.problems.first() {
            return Err(RecipeError::Kovan(format!("{p:?}")));
        }
        fn section<T: for<'de> Deserialize<'de>>(
            doc: &kovan::artifact::ParsedDocument,
            id: &'static str,
        ) -> Result<T, RecipeError> {
            let a = doc.get(id).ok_or(RecipeError::MissingSection(id))?;
            let text = settings_fence(&a.body).ok_or(RecipeError::MissingSettings(id))?;
            toml::from_str(&text).map_err(|e| RecipeError::Settings {
                section: id,
                message: e.to_string(),
            })
        }
        let header: RecipeHeader = section(&doc, "recipe")?;
        if header.format != FORMAT {
            return Err(RecipeError::Format(format!(
                "format is {:?}, expected {FORMAT:?}",
                header.format
            )));
        }
        if header.version > VERSION {
            return Err(RecipeError::Format(format!(
                "version {} is newer than this build reads ({VERSION})",
                header.version
            )));
        }
        Ok(Self {
            header,
            nuclear_data: section(&doc, "step-0")?,
            pebble_bed: section(&doc, "step-1")?,
            pebble_design: section(&doc, "step-2")?,
            reflector: section(&doc, "step-3")?,
            inserts: section(&doc, "step-4")?,
            review: section(&doc, "review")?,
            monte_carlo: section(&doc, "step-5")?,
        })
    }

    /// Every citation in the recipe, with the section it belongs to.
    #[must_use]
    pub fn citations(&self) -> Vec<(&'static str, &Citation)> {
        let mut v = Vec::new();
        let groups: [(&'static str, &Vec<Citation>); 6] = [
            ("step-0", &self.nuclear_data.cites),
            ("step-1", &self.pebble_bed.cites),
            ("step-2", &self.pebble_design.cites),
            ("step-3", &self.reflector.cites),
            ("step-4", &self.inserts.cites),
            ("step-5", &self.monte_carlo.cites),
        ];
        for (id, cs) in groups {
            v.extend(cs.iter().map(|c| (id, c)));
        }
        v
    }
}

/// The current time as RFC 3339 UTC (`2026-10-05T12:34:56Z`), without a
/// date-time crate: kovan stores timestamps as strings and never does
/// arithmetic on them, so neither does this.
#[must_use]
pub fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    rfc3339_from_unix(secs)
}

/// RFC 3339 UTC for `secs` since the Unix epoch (Hinnant's civil-from-days).
#[must_use]
pub fn rfc3339_from_unix(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem / 60) % 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Recipe {
        let cite = |field: &str| Citation {
            field: field.into(),
            citekey: "example2014".into(),
            page: Some(3),
            what: "states the value".into(),
        };
        Recipe {
            header: RecipeHeader {
                format: FORMAT.into(),
                version: VERSION,
                title: "Test bed".into(),
                reactor: "htgr".into(),
                mode: Mode::Basic,
                preset: Some("htr10".into()),
                edited: false,
                vv_status: "TENTATIVE".into(),
            },
            nuclear_data: NuclearDataStep {
                endf_dir: "reference-data/endf".into(),
                library: "ENDF/B-VIII.0".into(),
                temperature_k: 300.15,
                cites: vec![cite("temperature_k")],
            },
            pebble_bed: PebbleBedStep {
                packing: "hex".into(),
                rings: 14,
                layers: 12,
                filling_fraction: 0.61,
                mix: PebbleMix {
                    fuel: 0.57,
                    moderator: 0.43,
                    fertile: 0.0,
                    poison: 0.0,
                },
                source: BedSource::Dem,
                dem: Some(DemPour {
                    n_pebbles: 16_890,
                    friction: 0.1,
                    rolling_friction: 0.0,
                    youngs_modulus_pa: 5.0e8,
                    seed: 1,
                    settled_steps: Some(40_000),
                    phi_whole_core: Some(0.6047),
                }),
                cites: vec![],
            },
            pebble_design: PebbleDesignStep {
                interstitial: "helium".into(),
                pebbles: vec![
                    PebbleDesign {
                        kind: "fuel".into(),
                        outer_radius_cm: 3.0,
                        fuel_zone_radius_cm: Some(2.5),
                        matrix: "graphite".into(),
                        triso: Some(Triso {
                            kernel: "UO2".into(),
                            enrichment: 0.17,
                            kernel_radius_cm: 0.025,
                            layers: vec![("buffer".into(), 0.034), ("SiC".into(), 0.0425)],
                            particles_per_pebble: 8335,
                        }),
                    },
                    PebbleDesign {
                        kind: "moderator".into(),
                        outer_radius_cm: 3.0,
                        fuel_zone_radius_cm: None,
                        matrix: "graphite".into(),
                        triso: None,
                    },
                ],
                cites: vec![cite("pebble.fuel.outer_radius_cm")],
            },
            reflector: ReflectorStep {
                core_radius_cm: 90.0,
                reflector_outer_cm: 190.0,
                model_height_cm: 610.0,
                elements: vec![ModelElement {
                    name: "refuelling chute".into(),
                    status: ElementStatus::NotInModel,
                    note: "gh:#570".into(),
                }],
                cites: vec![],
            },
            inserts: InsertStep {
                control_rods: 10,
                b4c_inner_radius_cm: 3.0,
                b4c_outer_radius_cm: 5.25,
                b4c_density_g_per_cm3: 1.7,
                elements: vec![],
                cites: vec![],
            },
            review: ReviewGate::default(),
            monte_carlo: MonteCarloStep {
                particles: 2000,
                inactive: 30,
                active: 70,
                seed: 1,
                threads: 8,
                rod_insertion: 0.0,
                spectrum_bins_per_decade: 20,
                ablations: vec![],
                runs: vec![RunRecord {
                    label: "Run 1".into(),
                    rod_insertion: 0.0,
                    temperature_k: 300.15,
                    particles: 2000,
                    inactive: 30,
                    active: 70,
                    seed: 1,
                    k: 1.01234,
                    sigma: 0.00123,
                    transport_s: 12.5,
                }],
                cites: vec![],
            },
        }
    }

    /// Emit → parse gives back the same recipe.
    #[test]
    fn a_recipe_round_trips_through_kovan_markdown() {
        let r = sample();
        let md = r.to_markdown("2026-10-05T00:00:00Z").expect("write");
        let back = Recipe::from_markdown(&md).expect("read");
        assert_eq!(back, r);
    }

    /// kovan itself sees one well-formed note artifact per section, with the
    /// fixed ids, so the recipe is a kovan document and not merely markdown.
    #[test]
    fn kovan_reads_every_section_as_a_note_artifact() {
        let md = sample().to_markdown("2026-10-05T00:00:00Z").expect("write");
        let doc = kovan::artifact::parse_document(&md);
        assert!(doc.problems.is_empty(), "{:?}", doc.problems);
        let ids: Vec<&str> = doc.artifacts.iter().map(|a| a.id()).collect();
        let want: Vec<&str> = SECTIONS.iter().map(|s| s.0).collect();
        assert_eq!(ids, want);
        assert!(doc
            .artifacts
            .iter()
            .all(|a| a.toml.kovan.kind == kovan::artifact::ArtifactKind::Note));
    }

    #[test]
    fn a_missing_section_or_a_foreign_format_is_refused() {
        let md = sample().to_markdown("2026-10-05T00:00:00Z").expect("write");
        let cut = md.find("# Step 5").expect("step 5 heading");
        assert_eq!(
            Recipe::from_markdown(&md[..cut]),
            Err(RecipeError::MissingSection("step-5"))
        );
        let foreign = md.replacen("format = \"dhoby-ghaut-recipe\"", "format = \"other\"", 1);
        assert!(matches!(
            Recipe::from_markdown(&foreign),
            Err(RecipeError::Format(_))
        ));
    }

    #[test]
    fn rfc3339_formats_known_instants() {
        assert_eq!(rfc3339_from_unix(0), "1970-01-01T00:00:00Z");
        // 2026-10-05T00:00:00Z
        assert_eq!(rfc3339_from_unix(1_791_158_400), "2026-10-05T00:00:00Z");
        // A leap day.
        assert_eq!(rfc3339_from_unix(951_782_400), "2000-02-29T00:00:00Z");
    }

    /// A recipe written before the bed source existed still reads, as the
    /// lattice bed.
    #[test]
    fn an_older_recipe_without_a_bed_source_reads_as_the_lattice() {
        let mut r = sample();
        r.pebble_bed.source = BedSource::Lattice;
        r.pebble_bed.dem = None;
        let md = r.to_markdown("2026-10-05T00:00:00Z").expect("write");
        // An older file simply has no `source` line.
        let old: String = md.lines().filter(|l| !l.starts_with("source = ")).map(|l| format!("{l}\n")).collect();
        assert_ne!(old, md, "the test must actually remove the line");
        let back = Recipe::from_markdown(&old).expect("read");
        assert_eq!(back.pebble_bed.source, BedSource::Lattice);
    }
}
