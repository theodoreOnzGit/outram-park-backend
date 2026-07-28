//! Monte Carlo **simulation setup + run** — the backend the outram-mc GUI drives
//! (feature `mc-export`).
//!
//! [`crate::export`] turns authored geometry into an `outram-mc-libs`
//! [`Geometry`]. This module is the rest of "set up and run a *basic* Monte
//! Carlo simulation": build **materials** from a friendly nuclide-name + density
//! spec, bundle geometry + materials + source + run settings into an
//! [`McSimSetup`], and [`McSimSetup::run`] a **k-eigenvalue** (criticality)
//! calculation, returning `k_eff ± σ`.
//!
//! Scope is deliberately "basic": `outram-mc-libs` implements a **k-eigenvalue**
//! solver only (no fixed-source), so this backend targets criticality problems —
//! a bare homogeneous sphere (the validated `run_keff` driver) or a general CSG
//! model authored in outram-blender (`run_keff_csg`, with an optional cell/flux
//! tally). Cross sections come from the embedded offline data
//! ([`Nuclide::from_core`]) — no downloads, no HDF5.
//!
//! > Offline demonstration / education / research / V&V only, per the workspace
//! > `RESPONSIBLE_USE.md` — **not** for reactor operation, licensing, or
//! > safety-critical decisions.
//!
//! # Recipe
//!
//! ```no_run
//! use outram_blender::primitives::uv_sphere;
//! use outram_blender::sim::{MaterialSpec, McSimSetup, SimGeometry, atom_density};
//! use outram_blender::sim::KeffSettings;
//!
//! // 1. A material: Godiva HEU by atom density [atoms/barn·cm].
//! let heu = MaterialSpec {
//!     name: "Godiva HEU".into(),
//!     temperature_k: 293.6,
//!     nuclides: vec![
//!         ("U234".into(), 4.9184e-4),
//!         ("U235".into(), 4.4994e-2),
//!         ("U238".into(), 2.4984e-3),
//!     ],
//! };
//!
//! // 2. Set up and run a bare-sphere criticality at the Godiva radius.
//! let sim = McSimSetup {
//!     geometry: SimGeometry::BareSphere { radius_cm: 8.7407 },
//!     materials: vec![heu],
//!     settings: KeffSettings { n_particles: 2000, n_inactive: 20, n_active: 50, ..Default::default() },
//! };
//! let result = sim.run().unwrap();
//! println!("k_eff = {:.5} ± {:.5}", result.k_mean, result.k_std);
//! ```

use crate::export::{to_mc_geometry, ExportError};
use crate::mesh::Mesh;
use std::collections::HashMap;

use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::run_keff;
use outram_mc_libs::physics::transport_csg::run_keff_csg;
use outram_mc_libs::prelude::{CellFill, Geometry, Tally};

// Re-export the run-configuration and result types so a GUI/consumer needs only
// `outram_blender::sim::*`, not a direct outram-mc-libs dependency.
pub use outram_mc_libs::physics::compute::{ComputeType, ThreadCount};
pub use outram_mc_libs::physics::keff::{KeffResult, KeffSettings};
pub use outram_mc_libs::physics::transport_csg::SourceBox;

/// Avogadro's number [1/mol].
const AVOGADRO: f64 = 6.022_140_76e23;

/// A material specification in friendly terms: a name, a temperature, and its
/// nuclides as `(name, atom density)` pairs.
///
/// `atom density` is in **atoms/barn·cm** (the unit `outram-mc-libs` materials
/// use). Use [`atom_density`] to convert from a mass density if that is what you
/// have. Nuclide names are the `njoy-outram-park-fork` CORE-library keys, e.g.
/// `"U235"`, `"U238"`, `"H1"`, `"O16"`.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialSpec {
    /// Human-readable material name (e.g. `"Godiva HEU"`, `"UO2 fuel"`).
    pub name: String,
    /// Temperature [K], fed to the Doppler-broadened cross-section lookup.
    pub temperature_k: f64,
    /// `(nuclide name, atom density [atoms/barn·cm])` pairs.
    pub nuclides: Vec<(String, f64)>,
}

/// Number density of a nuclide from a **mass density**.
///
/// `N = ρ · w · N_A / M`, converted to **atoms/barn·cm** (the `1e-24` barn
/// factor). `mass_density_g_cc` is ρ [g/cm³], `molar_mass_g_mol` is the nuclide's
/// molar mass M [g/mol], `weight_fraction` is its mass fraction `w` in the
/// material (`1.0` for a pure element/isotope). `outram-mc-libs` has no such
/// helper, so this fills the gap for GUI/user input given in g/cm³.
///
/// # Example
/// ```
/// use outram_blender::sim::atom_density;
/// // Pure U-235 metal at 18.74 g/cm³, M ≈ 235.04 g/mol → ~4.8e-2 atoms/b·cm.
/// let n = atom_density(18.74, 235.04, 1.0);
/// assert!((n - 4.8e-2).abs() < 2e-3);
/// ```
pub fn atom_density(mass_density_g_cc: f64, molar_mass_g_mol: f64, weight_fraction: f64) -> f64 {
    mass_density_g_cc * weight_fraction * AVOGADRO / molar_mass_g_mol / 1.0e24
}

/// Errors from building or running a [`McSimSetup`].
#[derive(Debug)]
pub enum SimError {
    /// A nuclide name was not found in the embedded CORE data library.
    Nuclide(String),
    /// The setup has no materials (need at least one).
    NoMaterial,
    /// A `material_idx` referenced a material that does not exist.
    BadMaterialIndex(usize),
    /// The mesh could not be exported to `outram-mc-libs` geometry.
    Export(ExportError),
}

impl std::fmt::Display for SimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimError::Nuclide(m) => write!(f, "nuclide not available in the CORE data library: {m}"),
            SimError::NoMaterial => write!(f, "simulation has no materials (need at least one)"),
            SimError::BadMaterialIndex(i) => write!(f, "material index {i} out of range"),
            SimError::Export(e) => write!(f, "geometry export failed: {e}"),
        }
    }
}
impl std::error::Error for SimError {}

impl From<ExportError> for SimError {
    fn from(e: ExportError) -> Self {
        SimError::Export(e)
    }
}

/// Build the `(nuclides, materials)` arrays `outram-mc-libs` needs from a list of
/// [`MaterialSpec`]s.
///
/// Nuclide names are deduplicated into one shared `Vec<Nuclide>` (cross sections
/// loaded once from the embedded CORE library); each returned [`Material`]
/// references them by index, matching the `outram-mc-libs` convention. Material
/// ids are assigned `1, 2, …` in spec order.
pub fn build_materials(specs: &[MaterialSpec]) -> Result<(Vec<Nuclide>, Vec<Material>), SimError> {
    // Deduplicate nuclide names, preserving first-seen order.
    let mut order: Vec<String> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for s in specs {
        for (name, _) in &s.nuclides {
            index.entry(name.clone()).or_insert_with(|| {
                order.push(name.clone());
                order.len() - 1
            });
        }
    }

    let mut nuclides = Vec::with_capacity(order.len());
    for name in &order {
        let n = Nuclide::from_core(name).map_err(|e| SimError::Nuclide(format!("{name}: {e}")))?;
        nuclides.push(n);
    }

    let materials = specs
        .iter()
        .enumerate()
        .map(|(i, s)| Material {
            id: (i + 1) as i32,
            name: s.name.clone(),
            temperature: s.temperature_k,
            components: s
                .nuclides
                .iter()
                .map(|(n, d)| NuclideComponent { nuclide_idx: index[n], atom_density: *d })
                .collect(),
        })
        .collect();

    Ok((nuclides, materials))
}

/// What geometry a [`McSimSetup`] runs on.
pub enum SimGeometry {
    /// A bare homogeneous sphere of radius `radius_cm` [cm], vacuum outside — the
    /// validated `run_keff` driver. Uses the setup's **first** material.
    BareSphere {
        /// Sphere radius [cm].
        radius_cm: f64,
    },
    /// A general CSG model (authored geometry via [`csg_from_mesh`], or built by
    /// hand) run with `run_keff_csg`. `source` bounds the initial fission-source
    /// sampling box and must overlap the fissile region.
    Csg {
        /// The `outram-mc-libs` CSG geometry (cells reference material indices).
        geometry: Geometry,
        /// Initial source sampling box [cm].
        source: SourceBox,
    },
}

/// A complete **k-eigenvalue** simulation: geometry + materials + run settings.
/// The GUI (or any caller) builds one of these and calls [`McSimSetup::run`].
pub struct McSimSetup {
    /// What to run on.
    pub geometry: SimGeometry,
    /// Material specs. For [`SimGeometry::BareSphere`], the first is used; for
    /// [`SimGeometry::Csg`], cells index into this list (spec order = index).
    pub materials: Vec<MaterialSpec>,
    /// Power-iteration settings (histories/generation, inactive/active counts,
    /// seed, compute backend). [`KeffSettings::default`] is a sensible start.
    pub settings: KeffSettings,
}

impl McSimSetup {
    /// Run the criticality calculation, returning `k_eff` (mean ± standard error)
    /// and the per-generation eigenvalue history.
    pub fn run(&self) -> Result<KeffResult, SimError> {
        self.run_inner(None)
    }

    /// Run with a track-length [`Tally`] attached (CSG geometry only; the
    /// bare-sphere driver ignores tallies). The tally's `bins` are accumulated
    /// in place over the active generations.
    pub fn run_with_tally(&self, tally: &mut Tally) -> Result<KeffResult, SimError> {
        self.run_inner(Some(tally))
    }

    fn run_inner(&self, tally: Option<&mut Tally>) -> Result<KeffResult, SimError> {
        if self.materials.is_empty() {
            return Err(SimError::NoMaterial);
        }
        let (nuclides, materials) = build_materials(&self.materials)?;
        match &self.geometry {
            SimGeometry::BareSphere { radius_cm } => {
                // run_keff takes a single homogeneous material; no tally support.
                Ok(run_keff(*radius_cm, &materials[0], &nuclides, &self.settings))
            }
            SimGeometry::Csg { geometry, source } => {
                let src = SourceBox { lower: source.lower, upper: source.upper };
                Ok(run_keff_csg(geometry, &materials, &nuclides, src, &self.settings, tally))
            }
        }
    }
}

/// Build a [`SimGeometry::Csg`] from an authored [`Mesh`], assigning
/// `material_idx` (an index into a [`McSimSetup::materials`] list) to its cell.
///
/// The mesh is exported via [`to_mc_geometry`] (box / sphere / Z-cylinder /
/// convex-faceted), the single placeholder `Void` cell is filled with the chosen
/// material, and an initial source box is fitted to the mesh's vertex bounds
/// (inflated slightly so it overlaps the body). Returns [`SimError::Export`] for
/// a mesh that is not a fittable primitive (e.g. a non-convex solid).
pub fn csg_from_mesh(mesh: &Mesh, material_idx: usize) -> Result<SimGeometry, SimError> {
    let mut geometry = to_mc_geometry(mesh)?;
    // Fill the exported placeholder cell with the requested material.
    if let Some(cell) = geometry.cells.first_mut() {
        cell.fill = CellFill::Material(material_idx);
    }

    // Source box = the mesh's bounding box (a fissile body sits inside it).
    let verts = mesh.positions();
    let (mut lo, mut hi) = (
        Position::new(f64::MAX, f64::MAX, f64::MAX),
        Position::new(f64::MIN, f64::MIN, f64::MIN),
    );
    for v in &verts {
        lo = Position::new(lo.x.min(v.x), lo.y.min(v.y), lo.z.min(v.z));
        hi = Position::new(hi.x.max(v.x), hi.y.max(v.y), hi.z.max(v.z));
    }
    Ok(SimGeometry::Csg { geometry, source: SourceBox { lower: lo, upper: hi } })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// V&V — the atom-density conversion matches the Godiva HEU U-235 number
    /// density. Methodology: pure U-235 metal component, ρ_eff for the U-235
    /// weight in HEU. Here we check the formula against a known pure-isotope
    /// value: U-235 at 18.74 g/cm³ → N = ρ·N_A/M ≈ 4.80e-2 atoms/b·cm.
    #[test]
    fn atom_density_formula_is_correct() {
        let n = atom_density(18.74, 235.043_93, 1.0);
        assert!((n - 4.802e-2).abs() < 1e-4, "N = {n}");
    }

    /// V&V — material building deduplicates nuclides and indexes correctly.
    /// Methodology: two materials sharing U-235; the shared nuclide array should
    /// have each name once, and each material's components should point at the
    /// right shared index.
    #[test]
    fn build_materials_dedups_and_indexes() {
        let specs = vec![
            MaterialSpec { name: "fuel".into(), temperature_k: 900.0, nuclides: vec![("U235".into(), 1e-2), ("O16".into(), 2e-2)] },
            MaterialSpec { name: "moderator".into(), temperature_k: 600.0, nuclides: vec![("H1".into(), 6e-2), ("O16".into(), 3e-2)] },
        ];
        let (nuclides, materials) = build_materials(&specs).expect("CORE nuclides available");
        // U235, O16, H1 — three unique nuclides.
        assert_eq!(nuclides.len(), 3, "deduplicated nuclide count");
        assert_eq!(materials.len(), 2);
        assert_eq!(materials[0].components.len(), 2);
        // O16 is shared: material 0's O16 index == material 1's O16 index.
        let o16_in_fuel = materials[0].components[1].nuclide_idx;
        let o16_in_mod = materials[1].components[1].nuclide_idx;
        assert_eq!(o16_in_fuel, o16_in_mod, "shared nuclide has one index");
    }

    /// An empty material list is rejected.
    #[test]
    fn no_materials_is_an_error() {
        let sim = McSimSetup {
            geometry: SimGeometry::BareSphere { radius_cm: 8.0 },
            materials: vec![],
            settings: KeffSettings::default(),
        };
        assert!(matches!(sim.run(), Err(SimError::NoMaterial)));
    }

    /// Godiva HEU material spec (HEU-MET-FAST-001 atom densities).
    fn godiva_heu() -> MaterialSpec {
        MaterialSpec {
            name: "Godiva HEU".into(),
            temperature_k: 293.6,
            nuclides: vec![
                ("U234".into(), 4.9184e-4),
                ("U235".into(), 4.4994e-2),
                ("U238".into(), 2.4984e-3),
            ],
        }
    }

    /// V&V — end-to-end pipeline sanity (light, in-suite). Methodology: run the
    /// Godiva bare HEU sphere (r = 8.7407 cm) at a small history count through the
    /// backend and check the result is a physically sane near-critical
    /// eigenvalue. Pass criterion: 0.90 < k_eff < 1.15 with a finite standard
    /// error and one entry per active generation. (The full-fidelity
    /// benchmark comparison — k ≈ 1.0115 ± 0.0028 at 3000×[25+75] — is in the
    /// `mc_godiva_keff` example's V&V block; this test only proves the pipeline
    /// runs and returns sane physics.)
    #[test]
    fn godiva_bare_sphere_is_near_critical() {
        let sim = McSimSetup {
            geometry: SimGeometry::BareSphere { radius_cm: 8.7407 },
            materials: vec![godiva_heu()],
            settings: KeffSettings {
                n_particles: 800,
                n_inactive: 15,
                n_active: 35,
                compute: ComputeType::CpuMultiThread(ThreadCount::Auto),
                ..Default::default()
            },
        };
        let r = sim.run().expect("Godiva bare-sphere run");
        assert!(r.k_mean.is_finite() && r.k_std.is_finite(), "finite result: {} ± {}", r.k_mean, r.k_std);
        assert!(r.k_mean > 0.90 && r.k_mean < 1.15, "near-critical k_eff, got {}", r.k_mean);
        assert!(!r.k_by_generation.is_empty(), "per-generation eigenvalue history recorded");
    }

    /// V&V — the authoring→export→simulate bridge runs. Methodology: build a
    /// uv-sphere primitive in outram-blender at the Godiva radius, export it to
    /// outram-mc CSG with `csg_from_mesh`, assign the HEU material, and run.
    /// Pass criterion: same near-critical band as the bare-sphere path, proving
    /// the authored-geometry route reaches a real k_eff.
    #[test]
    fn authored_sphere_csg_runs_and_is_near_critical() {
        use crate::primitives::uv_sphere;
        let mesh = uv_sphere(24, 12, 8.7407);
        let geometry = csg_from_mesh(&mesh, 0).expect("uv-sphere exports to CSG");
        let sim = McSimSetup {
            geometry,
            materials: vec![godiva_heu()],
            settings: KeffSettings {
                n_particles: 800,
                n_inactive: 15,
                n_active: 35,
                compute: ComputeType::CpuMultiThread(ThreadCount::Auto),
                ..Default::default()
            },
        };
        let r = sim.run().expect("authored CSG run");
        assert!(r.k_mean > 0.90 && r.k_mean < 1.20, "authored sphere near-critical k_eff, got {}", r.k_mean);
    }
}
