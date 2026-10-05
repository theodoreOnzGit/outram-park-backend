//! # Step 7: the three meshes, their regions, and the mapping between them
//!
//! GeN-Foam solves neutronics, thermal-hydraulics and thermo-mechanics on
//! **three separate, overlapping meshes** and moves fields between them with a
//! cell-volume-weighted mesh-to-mesh map (`meshHandler`, upstream
//! `src/classes/multiRegion/meshHandler/meshHandler.C`). Step 7 builds the
//! same three for the reactor the recipe describes, and this module holds the
//! **solver-independent half**: the R-Z domain, the regions, the settings, and
//! the hand-off types the multiphysics steps (9 and 10) read. The meshing
//! itself, which calls `outram-blender` and the cfMesh port, is in the
//! `dhoby-ghaut` binary (`src/bin/dhoby-ghaut/meshing.rs`).
//!
//! ## The three meshes (maintainer defaults, 2026-10-05)
//!
//! | role | domain (HTR-10) | cells |
//! |---|---|---|
//! | [`MeshRole::Neutronics`] | the whole Monte Carlo model, `r <= 190 cm`, bottom to top | tet-dual polyhedra (cfMesh tet -> dual) |
//! | [`MeshRole::ThermalHydraulics`] | the core cavity, `r <= 90 cm`, conus top to cavity top: the pebble bed (porous) and the gas cavity above it (fluid) | tet-dual polyhedra; prism **boundary layers** on the fluid's walls only |
//! | [`MeshRole::Structural`] | the graphite and carbon-brick structure around the core cavity, conus and discharge tube | **tetrahedra** (`Tet4`, farrer-park FEM), never polyhedra |
//!
//! ## Regions
//!
//! A [`Region`] is a set of parts of the R-Z map ([`RzDomain`]): the in-core
//! parts (bed, conus, discharge tube, cavity) and the boxes of IAEA-TECDOC-1382
//! Fig. 4.10 outside it. **Basic** ([`RegionMap::basic`]) groups them into a
//! preset of eleven regions. **Advanced** lets the user move any box outside
//! the core to another region (or a new one) and split the bed into radial x
//! axial sub-regions; the in-core regions never *have* to be managed. Every
//! mesh cell takes the region of the R-Z part containing its centroid, so a
//! region on a mesh is a set of whole cells: [`MeshSummary::regions`] compares
//! each one's meshed volume with the exact R-Z volume.
//!
//! ## Hand-off to Steps 9 and 10 (the multiphysics agent reads THIS)
//!
//! [`MeshSet`] is what Step 7 produces:
//!
//! - `meshes[role].polymesh_dir`: an OpenFOAM `polyMesh` directory (points,
//!   faces, owner, neighbour, boundary, **cellZones**, metres) for each mesh.
//!   Read the FV ones with
//!   `outram_foam_appbuilder_lib::io::poly_mesh::{read_poly_mesh, read_cell_zones}`;
//!   the structural one is all `Tet4` and converts to a farrer-park mesh with
//!   `outram_blender::unstructured::convert::fem::to_fem_mesh`.
//! - `meshes[role].cell_region`: the region index of every cell (the
//!   cellZones say the same by name; a zone's name is [`Region::id`]).
//! - `mappings`: all six directed pairs, each a [`MeshMapping`] with
//!   normalised weights and [`MeshMapping::map`], the upstream `mapTgtToSrc`
//!   with `plusEqOp` semantics. `fields` lists upstream's default
//!   `multiRegionCouplingDict` fields for that direction (empty where upstream
//!   maps none).
//!
//! The MGXS of Step 8 are keyed by the same region ids (see [`super::mgxs`]).

use serde::{Deserialize, Serialize};

/// Which of the three GeN-Foam meshes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MeshRole {
    /// Neutronics (diffusion / SP3) mesh; upstream region `neutroRegion`.
    Neutronics,
    /// Thermal-hydraulics (porous / fluid) mesh; upstream `fluidRegion`.
    ThermalHydraulics,
    /// Structural FEM mesh; upstream `thermoMechanicalRegion` (here
    /// farrer-park, the MOOSE-side FEM).
    Structural,
}

impl MeshRole {
    /// The three, in order.
    pub const ALL: [Self; 3] = [Self::Neutronics, Self::ThermalHydraulics, Self::Structural];

    /// Index into `[_; 3]` arrays.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::Neutronics => 0,
            Self::ThermalHydraulics => 1,
            Self::Structural => 2,
        }
    }

    /// Human name.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Neutronics => "Neutronics",
            Self::ThermalHydraulics => "Thermal-hydraulics",
            Self::Structural => "Structural (FEM)",
        }
    }

    /// Upstream GeN-Foam region name.
    #[must_use]
    pub fn genfoam_region(self) -> &'static str {
        match self {
            Self::Neutronics => "neutroRegion",
            Self::ThermalHydraulics => "fluidRegion",
            Self::Structural => "thermoMechanicalRegion",
        }
    }

    /// Short word for file and directory names.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::Neutronics => "neutronics",
            Self::ThermalHydraulics => "thermal_hydraulics",
            Self::Structural => "structural",
        }
    }
}

/// What a region physically is, which decides its default cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegionClass {
    /// The pebble bed (and the pebble columns of the conus and discharge
    /// tube): a porous medium.
    PebbleBed,
    /// Open gas (the cavity above the bed).
    Fluid,
    /// A zone that homogenises solid with gas channels or plena (the
    /// coolant-channel band, the control-rod band, the cold-gas plenum): a
    /// porous medium or a conduction region.
    Porous,
    /// Solid graphite or carbon brick: conduction and stress.
    Solid,
}

impl RegionClass {
    /// Human label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::PebbleBed => "pebble bed (porous)",
            Self::Fluid => "fluid",
            Self::Porous => "porous / conduction",
            Self::Solid => "solid",
        }
    }

    /// The thermal-hydraulics cells this class gets by default (maintainer
    /// defaults, 2026-10-05).
    #[must_use]
    pub fn th_cells(self) -> CellType {
        match self {
            Self::Fluid => CellType::TetDualWithLayers,
            _ => CellType::TetDual,
        }
    }
}

/// Cell type of a mesh or a region of one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CellType {
    /// cfMesh tet -> polyhedral dual.
    TetDual,
    /// Tet-dual plus prism boundary layers on the walls.
    TetDualWithLayers,
    /// Linear tetrahedra (FEM).
    Tet4,
}

impl CellType {
    /// Human label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::TetDual => "tet-dual polyhedral",
            Self::TetDualWithLayers => "tet-dual polyhedral + boundary layers",
            Self::Tet4 => "tetrahedral FEM (Tet4)",
        }
    }
}

/// One rectangle of the reflector zone map in `(r, z)`, model frame \[cm\]
/// (z up, origin at the bed mid-height, as the Monte Carlo geometry).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RzBox {
    /// IAEA-TECDOC-1382 Table 4-3 zone number.
    pub zone: usize,
    /// Inner and outer radius \[cm\].
    pub r: [f64; 2],
    /// Bottom and top \[cm\].
    pub z: [f64; 2],
}

/// The axisymmetric (R-Z) description of the reactor the meshes are built
/// on, in the Monte Carlo model's frame \[cm\]. The bed top moves with the
/// loading; everything else is fixed hardware.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RzDomain {
    /// Outer radius of the model \[cm\].
    pub r_outer: f64,
    /// Core (cavity) radius \[cm\].
    pub core_radius: f64,
    /// Discharge-tube radius \[cm\].
    pub tube_radius: f64,
    /// Model bottom and top \[cm\].
    pub z_bottom: f64,
    /// Model top \[cm\].
    pub z_top: f64,
    /// Conus floor (top of the discharge tube) \[cm\].
    pub conus_floor: f64,
    /// Conus top = bed bottom \[cm\].
    pub conus_top: f64,
    /// Bed top \[cm\].
    pub bed_top: f64,
    /// Cavity top (bottom of the top reflector) \[cm\].
    pub cavity_top: f64,
    /// The zone boxes outside the pebble-filled interior.
    pub boxes: Vec<RzBox>,
}

/// One part of the R-Z map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RzPart {
    /// The pebble bed above the conus.
    Bed,
    /// The conus (sloping bottom of the bed, dummy balls).
    Conus,
    /// Graphite between the conus and the core radius (TECDOC zone 0).
    ConusGraphite,
    /// The fuel-discharge tube.
    DischargeTube,
    /// The empty cavity above the bed.
    Cavity,
    /// Box `i` of [`RzDomain::boxes`].
    Box(usize),
}

impl RzDomain {
    /// Radius of the conus at height `z` (linear from the core radius at its
    /// top to the tube radius at its floor).
    #[must_use]
    pub fn conus_radius(&self, z: f64) -> f64 {
        let t = ((z - self.conus_floor) / (self.conus_top - self.conus_floor)).clamp(0.0, 1.0);
        self.tube_radius + t * (self.core_radius - self.tube_radius)
    }

    /// The part containing `(r, z)` \[cm\], or `None` outside the model.
    #[must_use]
    pub fn part_at(&self, r: f64, z: f64) -> Option<RzPart> {
        if r > self.r_outer || z < self.z_bottom || z > self.z_top {
            return None;
        }
        if r <= self.core_radius {
            if z >= self.conus_top && z < self.bed_top {
                return Some(RzPart::Bed);
            }
            if z >= self.bed_top && z < self.cavity_top {
                return Some(RzPart::Cavity);
            }
            if z >= self.conus_floor && z < self.conus_top {
                return Some(if r <= self.conus_radius(z) {
                    RzPart::Conus
                } else {
                    RzPart::ConusGraphite
                });
            }
            if z < self.conus_floor && r <= self.tube_radius {
                return Some(RzPart::DischargeTube);
            }
        }
        self.boxes
            .iter()
            .position(|b| r >= b.r[0] && r <= b.r[1] && z >= b.z[0] && z <= b.z[1])
            .map(RzPart::Box)
    }

    /// Every part, in a fixed order (the in-core ones first).
    #[must_use]
    pub fn parts(&self) -> Vec<RzPart> {
        let mut v = vec![
            RzPart::Bed,
            RzPart::Conus,
            RzPart::ConusGraphite,
            RzPart::DischargeTube,
            RzPart::Cavity,
        ];
        v.extend((0..self.boxes.len()).map(RzPart::Box));
        v
    }

    /// The thermal-hydraulics domain's R-Z profile \[cm\]: the core cylinder
    /// from the conus top to the cavity top, with a vertex at the bed top so
    /// the side wall splits into a bed wall and a cavity (fluid) wall.
    #[must_use]
    pub fn th_profile(&self) -> Vec<[f64; 2]> {
        vec![
            [0.0, self.conus_top],
            [self.core_radius, self.conus_top],
            [self.core_radius, self.bed_top],
            [self.core_radius, self.cavity_top],
            [0.0, self.cavity_top],
        ]
    }

    /// The structural domain's R-Z profile \[cm\]: the whole model less the
    /// cavity, the bed, the conus and the discharge tube (counter-clockwise).
    #[must_use]
    pub fn structural_profile(&self) -> Vec<[f64; 2]> {
        vec![
            [0.0, self.cavity_top],
            [self.core_radius, self.cavity_top],
            [self.core_radius, self.conus_top],
            [self.tube_radius, self.conus_floor],
            [self.tube_radius, self.z_bottom],
            [self.r_outer, self.z_bottom],
            [self.r_outer, self.z_top],
            [0.0, self.z_top],
        ]
    }

    /// The neutronics domain's R-Z profile \[cm\]: the whole model.
    #[must_use]
    pub fn neutronics_profile(&self) -> Vec<[f64; 2]> {
        vec![
            [0.0, self.z_bottom],
            [self.r_outer, self.z_bottom],
            [self.r_outer, self.z_top],
            [0.0, self.z_top],
        ]
    }

    /// Exact volume \[cm³\] of every part, by midpoint quadrature of
    /// `2 pi r dr dz` on an `h` cm grid (exact for the boxes up to the grid
    /// alignment, second order on the conus).
    #[must_use]
    pub fn part_volumes(&self, h: f64) -> Vec<(RzPart, f64)> {
        let parts = self.parts();
        let mut vol = vec![0.0; parts.len()];
        let nr = (self.r_outer / h).ceil() as usize;
        let nz = ((self.z_top - self.z_bottom) / h).ceil() as usize;
        let dr = self.r_outer / nr as f64;
        let dz = (self.z_top - self.z_bottom) / nz as f64;
        for i in 0..nr {
            let r = (i as f64 + 0.5) * dr;
            for k in 0..nz {
                let z = self.z_bottom + (k as f64 + 0.5) * dz;
                if let Some(p) = self.part_at(r, z) {
                    if let Some(j) = parts.iter().position(|q| *q == p) {
                        vol[j] += 2.0 * std::f64::consts::PI * r * dr * dz;
                    }
                }
            }
        }
        parts.into_iter().zip(vol).collect()
    }
}

/// One region: a name, a class, and whether it is in the core.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Region {
    /// Identifier: an OpenFOAM word (cellZone name, nuclearData zone name).
    pub id: String,
    /// Human label.
    pub label: String,
    /// What it physically is.
    pub class: RegionClass,
    /// In-core regions are preset and never have to be managed.
    pub in_core: bool,
}

/// How regions were made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegionMode {
    /// The preset, untouched.
    Basic,
    /// User-edited outside the core and/or the bed split.
    Advanced,
}

/// Regions and which R-Z part belongs to which.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegionMap {
    /// Basic or Advanced.
    pub mode: RegionMode,
    /// The regions.
    pub regions: Vec<Region>,
    /// Region of each [`RzDomain::boxes`] entry.
    pub box_region: Vec<usize>,
    /// Region of the conus, the conus graphite, the tube and the cavity.
    pub conus: usize,
    /// Region of the graphite between the conus and the core radius.
    pub conus_graphite: usize,
    /// Region of the discharge tube.
    pub tube: usize,
    /// Region of the cavity.
    pub cavity: usize,
    /// The bed: `[radial, axial]` sub-regions (Basic: `[1, 1]`), and the
    /// index of the first; sub-region `(i, k)` is `bed_first + k * radial + i`.
    pub bed_split: [usize; 2],
    /// First bed region.
    pub bed_first: usize,
}

fn rg(id: &str, label: &str, class: RegionClass, in_core: bool) -> Region {
    Region {
        id: id.into(),
        label: label.into(),
        class,
        in_core,
    }
}

impl RegionMap {
    /// The Basic preset for the HTR-10 R-Z map: four in-core regions and
    /// seven outside, grouping the TECDOC-1382 Fig. 4.10 boxes by function.
    /// The grouping is this workbench's convention (no source prescribes
    /// regions for a multigroup model of HTR-10); the boxes keep their zone
    /// numbers, so a user can regroup them in Advanced.
    #[must_use]
    pub fn basic(d: &RzDomain) -> Self {
        let mut regions = vec![
            rg("pebble_bed", "Pebble bed", RegionClass::PebbleBed, true),
            rg("conus", "Conus (dummy balls)", RegionClass::PebbleBed, true),
            rg(
                "discharge_tube",
                "Discharge tube",
                RegionClass::PebbleBed,
                true,
            ),
            rg(
                "core_cavity",
                "Cavity above the bed",
                RegionClass::Fluid,
                true,
            ),
            rg("top_reflector", "Top reflector", RegionClass::Solid, false),
            rg(
                "cold_gas_plenum",
                "Cold-gas plenum band (zT 95-105)",
                RegionClass::Porous,
                false,
            ),
            rg(
                "side_reflector",
                "Side reflector",
                RegionClass::Solid,
                false,
            ),
            rg(
                "control_rod_band",
                "Control-rod / KLAK band (r 95.6-108.6)",
                RegionClass::Porous,
                false,
            ),
            rg(
                "coolant_band",
                "Coolant-channel band (r 140.6-148.6)",
                RegionClass::Porous,
                false,
            ),
            rg(
                "carbon_bricks",
                "Boronated carbon bricks (r > 167.8)",
                RegionClass::Solid,
                false,
            ),
            rg(
                "bottom_structures",
                "Bottom reflector and hot-gas plenum",
                RegionClass::Porous,
                false,
            ),
        ];
        let conus_graphite = regions.len();
        regions.push(rg(
            "conus_graphite",
            "Graphite around the conus (zone 0)",
            RegionClass::Solid,
            false,
        ));
        // The boxes, in model z; zT = z_top - z.
        let box_region = d
            .boxes
            .iter()
            .map(|b| {
                let zt_top = d.z_top - b.z[1];
                let zt_bot = d.z_top - b.z[0];
                let mid = |a: f64, c: f64| 0.5 * (a + c);
                let (rm, ztm) = (mid(b.r[0], b.r[1]), mid(zt_top, zt_bot));
                if b.r[0] >= 167.0 {
                    9
                } else if (95.0..=105.0).contains(&ztm) {
                    5
                } else if ztm < 130.0 {
                    4
                } else if ztm > 388.764 {
                    10
                } else if (95.6..=108.6).contains(&rm) {
                    7
                } else if (140.6..=148.6).contains(&rm) {
                    8
                } else {
                    6
                }
            })
            .collect();
        Self {
            mode: RegionMode::Basic,
            regions,
            box_region,
            conus: 1,
            conus_graphite,
            tube: 2,
            cavity: 3,
            bed_split: [1, 1],
            bed_first: 0,
        }
    }

    /// Split the bed into `radial x axial` equal-height, equal-radius
    /// sub-regions (Advanced). `[1, 1]` restores one bed region. Region
    /// indices after the bed are renumbered.
    pub fn split_bed(&mut self, radial: usize, axial: usize) {
        let (radial, axial) = (radial.max(1), axial.max(1));
        let old = self.bed_split[0] * self.bed_split[1];
        let first = self.bed_first;
        let mut bed: Vec<Region> = Vec::new();
        for k in 0..axial {
            for i in 0..radial {
                let (id, label) = if radial * axial == 1 {
                    ("pebble_bed".to_string(), "Pebble bed".to_string())
                } else {
                    (
                        format!("pebble_bed_r{i}_z{k}"),
                        format!("Pebble bed, ring {}, layer {}", i + 1, k + 1),
                    )
                };
                bed.push(Region {
                    id,
                    label,
                    class: RegionClass::PebbleBed,
                    in_core: true,
                });
            }
        }
        let new = bed.len();
        self.regions.splice(first..first + old, bed);
        let shift = |r: &mut usize| {
            if *r >= first + old {
                *r = *r + new - old;
            }
        };
        for r in &mut self.box_region {
            shift(r);
        }
        for r in [
            &mut self.conus,
            &mut self.conus_graphite,
            &mut self.tube,
            &mut self.cavity,
        ] {
            shift(r);
        }
        self.bed_split = [radial, axial];
        if radial * axial != 1 {
            self.mode = RegionMode::Advanced;
        }
    }

    /// Whether region `g` is (a sub-region of) the pebble bed above the conus.
    #[must_use]
    pub fn is_bed(&self, g: usize) -> bool {
        let n = self.bed_split[0] * self.bed_split[1];
        g >= self.bed_first && g < self.bed_first + n
    }

    /// The region at `(r, z)` \[cm\].
    #[must_use]
    pub fn region_at(&self, d: &RzDomain, r: f64, z: f64) -> Option<usize> {
        Some(match d.part_at(r, z)? {
            RzPart::Bed => {
                let [nr, nz] = self.bed_split;
                let i = ((r / d.core_radius) * nr as f64)
                    .floor()
                    .clamp(0.0, (nr - 1) as f64) as usize;
                let t = (z - d.conus_top) / (d.bed_top - d.conus_top);
                let k = (t * nz as f64).floor().clamp(0.0, (nz - 1) as f64) as usize;
                self.bed_first + k * nr + i
            }
            RzPart::Conus => self.conus,
            RzPart::ConusGraphite => self.conus_graphite,
            RzPart::DischargeTube => self.tube,
            RzPart::Cavity => self.cavity,
            RzPart::Box(b) => self.box_region[b],
        })
    }

    /// Exact volume \[cm³\] of each region, from [`RzDomain::part_volumes`]
    /// (the bed split by quadrature as well).
    #[must_use]
    pub fn exact_volumes(&self, d: &RzDomain, h: f64) -> Vec<f64> {
        let mut v = vec![0.0; self.regions.len()];
        let nr = (d.r_outer / h).ceil() as usize;
        let nz = ((d.z_top - d.z_bottom) / h).ceil() as usize;
        let dr = d.r_outer / nr as f64;
        let dz = (d.z_top - d.z_bottom) / nz as f64;
        for i in 0..nr {
            let r = (i as f64 + 0.5) * dr;
            for k in 0..nz {
                let z = d.z_bottom + (k as f64 + 0.5) * dz;
                if let Some(g) = self.region_at(d, r, z) {
                    v[g] += 2.0 * std::f64::consts::PI * r * dr * dz;
                }
            }
        }
        v
    }

    /// Move box `b` to region `to` (Advanced; refused for an in-core region).
    pub fn assign_box(&mut self, b: usize, to: usize) -> Result<(), String> {
        let r = self.regions.get(to).ok_or("no such region")?;
        if r.in_core {
            return Err(format!(
                "{} is an in-core region; boxes outside the core cannot join it",
                r.label
            ));
        }
        if let Some(slot) = self.box_region.get_mut(b) {
            *slot = to;
            self.mode = RegionMode::Advanced;
        }
        Ok(())
    }

    /// Add an outside-core region (Advanced) and return its index.
    pub fn add_region(&mut self, id: &str, label: &str, class: RegionClass) -> usize {
        self.regions.push(rg(id, label, class, false));
        self.mode = RegionMode::Advanced;
        self.regions.len() - 1
    }
}

/// Mesh settings for one role. Lengths in cm (converted to metres for cfMesh).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoleSettings {
    /// Background cell edge \[cm\].
    pub cell_size_cm: f64,
    /// Circumferential facets of the body of revolution.
    pub n_seg: usize,
    /// Prism layers on fluid walls (thermal-hydraulics only).
    pub layers: usize,
    /// First layer thickness \[cm\].
    pub first_layer_cm: f64,
    /// Layer growth ratio.
    pub expansion: f64,
    /// Octree refinement levels toward the walls (cfMesh `refinement_levels`:
    /// the wall cells are `cell_size / 2^refine`).
    pub refine: u8,
}

/// Step 7's settings: the regions and the three meshes' resolutions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshPlan {
    /// The regions.
    pub regions: RegionMap,
    /// Per [`MeshRole::index`].
    pub roles: [RoleSettings; 3],
}

impl MeshPlan {
    /// Defaults: coarse enough to mesh in well under a minute and to keep
    /// the Monte Carlo tally on the neutronics mesh cheap; refine for
    /// production. Not derived from a convergence study (none exists yet).
    #[must_use]
    pub fn default_for(d: &RzDomain) -> Self {
        Self {
            regions: RegionMap::basic(d),
            roles: [
                RoleSettings {
                    cell_size_cm: 30.0,
                    n_seg: 32,
                    layers: 0,
                    first_layer_cm: 0.0,
                    expansion: 1.0,
                    refine: 0,
                },
                RoleSettings {
                    cell_size_cm: 15.0,
                    n_seg: 32,
                    layers: 3,
                    first_layer_cm: 1.0,
                    expansion: 1.3,
                    refine: 0,
                },
                RoleSettings {
                    cell_size_cm: 25.0,
                    n_seg: 32,
                    layers: 0,
                    first_layer_cm: 0.0,
                    expansion: 1.0,
                    refine: 0,
                },
            ],
        }
    }
}

/// One region on one mesh: its cells' volume against the exact R-Z volume.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegionVolume {
    /// Region index.
    pub region: usize,
    /// Cells assigned.
    pub cells: usize,
    /// Meshed volume \[cm³\].
    pub mesh_cm3: f64,
    /// Exact R-Z volume of the region's part inside this mesh's domain \[cm³\].
    pub exact_cm3: f64,
}

/// What one mesh came out as.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshSummary {
    /// Which mesh.
    pub role: MeshRole,
    /// Cell type built.
    pub cell_type: CellType,
    /// Counts.
    pub cells: usize,
    /// Points.
    pub points: usize,
    /// Faces.
    pub faces: usize,
    /// `(element kind, count)`.
    pub kinds: Vec<(String, usize)>,
    /// Total volume \[cm³\].
    pub volume_cm3: f64,
    /// Exact volume of the domain \[cm³\].
    pub exact_cm3: f64,
    /// Cells whose centroid decomposition is not a tiling (non-star dual cells).
    pub non_star_cells: usize,
    /// Worst non-orthogonality \[deg\] and skewness.
    pub max_non_orthogonality_deg: f64,
    /// Worst skewness.
    pub max_skewness: f64,
    /// Boundary patches `(name, faces)`.
    pub patches: Vec<(String, usize)>,
    /// Pipeline stages that were skipped or degraded.
    pub notes: Vec<String>,
    /// Per region present on this mesh.
    pub regions: Vec<RegionVolume>,
    /// Region of every cell.
    pub cell_region: Vec<usize>,
    /// The written polyMesh directory (with cellZones), if written.
    pub polymesh_dir: Option<String>,
    /// Wall-clock seconds to build.
    pub seconds: f64,
}

/// A directed mesh-to-mesh map (GeN-Foam `meshToMesh`, `imCellVolumeWeight`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshMapping {
    /// Source mesh.
    pub from: MeshRole,
    /// Target mesh.
    pub to: MeshRole,
    /// Per target cell `(source cell, weight)`, weights summing to 1 on
    /// every cell with any overlap (upstream `normaliseWeights`).
    pub weights: Vec<Vec<(usize, f64)>>,
    /// Total overlap volume \[cm³\].
    pub overlap_cm3: f64,
    /// Fraction of the target's volume covered by the source.
    pub to_coverage: f64,
    /// Target cells with no overlap at all (they keep their value).
    pub uncovered_cells: usize,
    /// Upstream's default `multiRegionCouplingDict` fields for this direction,
    /// `(source field, target field)` (GeN-Foam `Tutorials/EMPTY`).
    pub fields: Vec<(String, String)>,
    /// Seconds to build (both directions of the pair share one overlap pass).
    pub seconds: f64,
}

impl MeshMapping {
    /// Map a source cell field onto the target: upstream `mapTgtToSrc` with
    /// `plusEqOp` and normalised weights. A target cell with any overlap
    /// becomes `Σ w φ_src`; one with none keeps its value.
    pub fn map(&self, from_field: &[f64], to_field: &mut [f64]) {
        for (c, row) in self.weights.iter().enumerate() {
            if row.is_empty() {
                continue;
            }
            let sw: f64 = row.iter().map(|x| x.1).sum();
            let mut r = to_field[c] * (1.0 - sw);
            for &(s, w) in row {
                r += w * from_field[s];
            }
            to_field[c] = r;
        }
    }
}

/// Upstream's default coupling fields `(source, target)` for `from -> to`
/// (GeN-Foam `Tutorials/EMPTY/constant/multiRegionCouplingDict`).
#[must_use]
pub fn genfoam_default_fields(from: MeshRole, to: MeshRole) -> Vec<(String, String)> {
    use MeshRole::{Neutronics as N, Structural as S, ThermalHydraulics as T};
    let v: &[(&str, &str)] = match (from, to) {
        (N, T) => &[
            ("powerDensity", "powerDensityNeutronics"),
            ("secondaryPowerDensity", "powerDensityNeutronicsToLiquid"),
        ],
        (T, N) => &[
            ("T", "TCool"),
            ("thermo:rho", "rhoCool"),
            ("T.fuelAvForNeutronics", "TFuel"),
            ("T.cladAvForNeutronics", "TClad"),
            ("T.passiveStructure", "TStructMech"),
        ],
        (S, N) => &[("meshDisp", "disp")],
        (T, S) => &[
            ("T.passiveStructure", "TStructFromTH"),
            ("T.fuelAvForNeutronics", "TFuel"),
        ],
        (N, S) => &[("powerDensity", "Q")],
        _ => &[],
    };
    v.iter()
        .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
        .collect()
}

/// Everything Step 7 produced: the hand-off to Steps 8-10.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshSet {
    /// The R-Z domain the meshes were built on.
    pub domain: RzDomain,
    /// The plan (regions and settings) they were built with.
    pub plan: MeshPlan,
    /// Per [`MeshRole::index`].
    pub meshes: Vec<MeshSummary>,
    /// All six directed maps.
    pub mappings: Vec<MeshMapping>,
}

impl MeshSet {
    /// The map `from -> to`.
    #[must_use]
    pub fn mapping(&self, from: MeshRole, to: MeshRole) -> Option<&MeshMapping> {
        self.mappings.iter().find(|m| m.from == from && m.to == to)
    }

    /// The summary of `role`.
    #[must_use]
    pub fn mesh(&self, role: MeshRole) -> Option<&MeshSummary> {
        self.meshes.iter().find(|m| m.role == role)
    }
}

/// Something Step 7 cannot do yet, shown as NOT available in the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotAvailable {
    /// What.
    pub what: &'static str,
    /// Why not.
    pub why: &'static str,
    /// The issue that tracks it.
    pub issue: u32,
}

/// The gaps of Step 7, each with its issue.
#[must_use]
pub fn not_available() -> Vec<NotAvailable> {
    vec![
        NotAvailable {
            what: "Body-fitted internal region boundaries",
            why: "cfMesh meshes one closed surface; regions are assigned to whole cells by centroid, so a region boundary is stair-stepped at the cell size",
            issue: 594,
        },
        NotAvailable {
            what: "Explicit borings (coolant, control-rod, KLAK channels) in the meshes",
            why: "the meshes are bodies of revolution of the R-Z map; the borings are inside their band regions (the Monte Carlo tallies carry their effect into the band's cross sections)",
            issue: 594,
        },
        NotAvailable {
            what: "Conus, discharge tube and reflector conduction on the thermal-hydraulics mesh",
            why: "boundary layers grow only on boundary patches, so the fluid must be bounded by the mesh boundary; the TH mesh is the core cavity only and the reflector is the structural mesh",
            issue: 594,
        },
        NotAvailable {
            what: "Thermal-hydraulics <-> structural coupling across the cavity wall",
            why: "the two domains only touch at the core wall, and GeN-Foam's map is a volume overlap (it transfers ~nothing here); a surface (conjugate heat transfer) coupling is needed",
            issue: 594,
        },
        NotAvailable {
            what: "In-core mesh-as-regions (one cross-section set per neutronics cell)",
            why: "per-cell Monte Carlo statistics are too poor at workbench run lengths; the bed can be split into rings x layers instead",
            issue: 594,
        },
    ]
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn htr10_like() -> RzDomain {
        RzDomain {
            r_outer: 190.0,
            core_radius: 90.0,
            tube_radius: 25.0,
            z_bottom: -350.0,
            z_top: 260.0,
            conus_floor: -130.0,
            conus_top: -93.0,
            bed_top: 0.0,
            cavity_top: 130.0,
            boxes: vec![
                RzBox {
                    zone: 1,
                    r: [0.0, 90.0],
                    z: [130.0, 260.0],
                },
                RzBox {
                    zone: 22,
                    r: [90.0, 190.0],
                    z: [-350.0, 130.0],
                },
                RzBox {
                    zone: 8,
                    r: [25.0, 90.0],
                    z: [-350.0, -130.0],
                },
                RzBox {
                    zone: 75,
                    r: [90.0, 190.0],
                    z: [130.0, 260.0],
                },
            ],
        }
    }

    /// Every point of the model belongs to exactly one part, and the parts'
    /// volumes add to the model cylinder's.
    #[test]
    fn the_parts_tile_the_model() {
        let d = htr10_like();
        let total: f64 = d.part_volumes(0.5).iter().map(|x| x.1).sum();
        let cyl = std::f64::consts::PI * 190.0f64.powi(2) * 610.0;
        assert!((total - cyl).abs() < 1e-3 * cyl, "{total} vs {cyl}");
        assert_eq!(d.part_at(10.0, -50.0), Some(RzPart::Bed));
        assert_eq!(d.part_at(10.0, 50.0), Some(RzPart::Cavity));
        assert_eq!(d.part_at(10.0, -200.0), Some(RzPart::DischargeTube));
        assert_eq!(d.part_at(85.0, -125.0), Some(RzPart::ConusGraphite));
        assert_eq!(d.part_at(20.0, -125.0), Some(RzPart::Conus));
    }

    /// Splitting the bed renumbers the regions after it consistently, and
    /// the region volumes still add to the model's.
    #[test]
    fn splitting_the_bed_keeps_every_region_consistent() {
        let d = htr10_like();
        let mut m = RegionMap::basic(&d);
        let n = m.regions.len();
        let cavity_id = m.regions[m.cavity].id.clone();
        m.split_bed(2, 3);
        assert_eq!(m.regions.len(), n + 5);
        assert_eq!(m.regions[m.cavity].id, cavity_id);
        assert_eq!(m.mode, RegionMode::Advanced);
        let v = m.exact_volumes(&d, 0.5);
        let total: f64 = v.iter().sum();
        let cyl = std::f64::consts::PI * 190.0f64.powi(2) * 610.0;
        assert!((total - cyl).abs() < 1e-3 * cyl);
        assert!(
            m.assign_box(0, m.cavity).is_err(),
            "an in-core region takes no outside box"
        );
    }

    #[test]
    fn mapping_follows_upstream_semantics() {
        let m = MeshMapping {
            from: MeshRole::Neutronics,
            to: MeshRole::ThermalHydraulics,
            weights: vec![vec![(0, 0.25), (1, 0.75)], vec![]],
            overlap_cm3: 1.0,
            to_coverage: 0.5,
            uncovered_cells: 1,
            fields: vec![],
            seconds: 0.0,
        };
        let mut t = vec![9.0, 7.0];
        m.map(&[4.0, 8.0], &mut t);
        assert_eq!(t, vec![7.0, 7.0]);
    }
}
