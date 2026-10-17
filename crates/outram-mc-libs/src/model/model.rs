//! **The [`Model`]**: data, shapes, components and temperature, lowered to
//! today's CSG and handed to the existing transport (gh #938, #939).
//!
//! This is a shell over existing code, not new physics: [`Model::assemble`]
//! produces the same [`Geometry`], low-level
//! [`Material`](crate::material::material::Material)s and nuclide list a
//! hand-written model would, and the run goes through
//! [`CsgPowerIteration`](crate::physics::transport_csg::CsgPowerIteration),
//! the single-thread CSG power iteration that is bit for bit
//! `run_keff_csg` (gh:#549).
//!
//! # Lowering (what the user never sees)
//!
//! Shapes are visited inside out (each shape after the shapes it contains).
//! Each shape is its surfaces (a sphere, or a box's six planes) and one
//! cell: `inside(self)` and, per contained shape, `outside(child)`, in the
//! token order the crate's concentric builders use (`outside(child),
//! inside(self), ∩`); a lattice is one universe per distinct tile and a CSG
//! `RectLattice` (the details are in the private `lower` module). One
//! outermost shape gets the **vacuum** boundary, or the reflective one it
//! asks for; several get a void sphere around them
//! with the vacuum boundary on it, so a neutron leaving one can still reach
//! another. Materials are numbered in first-use order (equal materials share
//! one entry), nuclides likewise, keyed by nuclide and thermal law.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use uom::si::f64::{Length, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::thermodynamic_temperature::kelvin;

use super::data::{DataError, NuclearData};
use super::isotope::Isotope;
use super::resolve::{MaterialError, ResolvedMaterial};
use super::lattice::Lattice;
use super::lower::{combine, half, Lowered, Lowering};
use super::shape::{Boundary, Component, Cuboid, Shape, Solid};
use crate::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use crate::geometry::geometry::Geometry;
use crate::geometry::plot::{render_material_slice, ImageData, PlotBasis, Rgb, SlicePlot};
use crate::geometry::position::Position;
use crate::geometry::surface::{BoundaryType, SurfaceKind};
use crate::geometry::universe::Universe;
use crate::material::nuclide::Nuclide;
use crate::physics::transport_csg::SourceBox;
use njoy_outram_park_fork::leapr::decks::SabMaterial;

/// **The temperature of the model**, set on the geometry only (maintainer
/// decision, #937/#938, 2026-10-17). One variant today: one temperature
/// everywhere. Position-dependent temperature (fields, fixed points) is
/// #945.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Temperature {
    /// The same temperature in every shape. Every nuclide is processed at
    /// it.
    Uniform(ThermodynamicTemperature),
}

/// Why a model could not be assembled. Geometry problems carry a picture.
#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    /// No shape was added.
    #[error("the model has no shapes: add one with model.add(..)")]
    Empty,
    /// No temperature was set.
    #[error("the model has no temperature: call model.set_temperature(Temperature::Uniform(..))")]
    NoTemperature,
    /// A non-positive or non-finite temperature or radius.
    #[error("{0}")]
    BadValue(String),
    /// A component made by another model.
    #[error(
        "a shape uses a component made by another model; make it with this model's .component(..)"
    )]
    ForeignComponent,
    /// Shapes overlap, or a contained shape pokes out of its container.
    /// `picture` is a PNG of the problem (see [`GeometryProblem`]).
    #[error("{0}")]
    Geometry(GeometryProblem),
    /// A material did not resolve against the library.
    #[error(transparent)]
    Material(#[from] MaterialError),
}

/// **A geometry problem, with a picture** (Brown's first recommendation:
/// look at the geometry in a plotter; LA-UR-08-06558, Conclusions).
#[derive(Debug, Clone)]
pub struct GeometryProblem {
    /// What is wrong, in words.
    pub message: String,
    /// An x-y slice through the two shapes at the height of the first's
    /// centre: the first shape **blue**, the second **orange**, where both
    /// claim the space **red**. 400 x 400 pixels.
    pub picture: ImageData,
}

impl std::fmt::Display for GeometryProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (picture: GeometryProblem::picture, blue = first, orange = second, red = both)",
            self.message
        )
    }
}

/// Model identities. Used only to tell one model's components from
/// another's; never reaches a number.
static NEXT_MODEL: AtomicU64 = AtomicU64::new(1);

/// **A Monte Carlo model**: a nuclear-data library, shapes, components and a
/// temperature. Run it with [`Model::keff`].
///
/// ```no_run
/// use outram_mc_libs::model::prelude::*;
/// use uom::si::{f64::{Length, ThermodynamicTemperature}, length::centimeter, thermodynamic_temperature::kelvin};
/// let mut godiva = Model::new(NuclearData::workspace_reference()?);
/// let core = godiva.component("Core");
/// let heu = Material::from_atom_densities([
///     (U234, atoms_per_barn_cm(4.9184e-4)),
///     (U235, atoms_per_barn_cm(4.4994e-2)),
///     (U238, atoms_per_barn_cm(2.4984e-3)),
/// ]);
/// godiva.add(Sphere::radius(Length::new::<centimeter>(8.7407)).filled_with(heu).in_component(core));
/// godiva.set_temperature(Temperature::Uniform(ThermodynamicTemperature::new::<kelvin>(293.6)));
/// let run = godiva.keff(KeffSettings::new(MCSettings::Auto, None))?;
/// println!("k = {}", run.keff());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone)]
pub struct Model {
    id: u64,
    data: Arc<NuclearData>,
    components: Vec<String>,
    shapes: Vec<Shape>,
    temperature: Option<Temperature>,
}

/// One transport nuclide: which nuclide, and the thermal law it carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NuclideKey {
    /// The nuclide.
    pub isotope: Isotope,
    /// The bound-atom law, when its material's [`super::Moderator`] gives one.
    pub law: Option<SabMaterial>,
}

/// **A model lowered to the transport's own types**, before nuclear data is
/// processed. Everything here can be inspected and drawn.
#[derive(Debug, Clone)]
pub struct AssembledModel {
    /// The CSG geometry.
    pub geometry: Geometry,
    /// Low-level materials; `components[j].nuclide_idx` indexes [`Self::nuclides`].
    pub materials: Vec<crate::material::material::Material>,
    /// The resolved description of each material, same order.
    pub resolved: Vec<ResolvedMaterial>,
    /// The transport nuclides, in the order the materials index them.
    pub nuclides: Vec<NuclideKey>,
    /// Where the first generation starts: the bounding box of the shapes whose
    /// material holds heavy metal (all shapes if none does). The sampler keeps
    /// only points in fissionable material, so the start is uniform over it.
    pub source_box: SourceBox,
    /// The temperature every nuclide is processed at \[K\].
    pub temperature_k: f64,
    /// Per cell: the component of the shape it came from.
    pub cell_components: Vec<Option<Component>>,
    /// Per cell: the exact volume of **one placed copy** \[cm³\] (the
    /// shape's solid minus the solids it contains); `None` for a cell filled
    /// with a lattice, or the void around several outermost shapes. Times
    /// the copies the geometry places gives a total
    /// ([`AssembledModel::census`]).
    pub cell_volumes_cm3: Vec<Option<f64>>,
    /// Component labels, by component index.
    pub component_labels: Vec<String>,
    /// The library's label (for provenance).
    pub data_label: String,
    /// Bounding box of the whole model \[cm\].
    pub bounds: ([f64; 3], [f64; 3]),
}

impl Model {
    /// A new, empty model over `data`.
    pub fn new(data: NuclearData) -> Self {
        Self {
            id: NEXT_MODEL.fetch_add(1, Ordering::Relaxed),
            data: Arc::new(data),
            components: Vec::new(),
            shapes: Vec::new(),
            temperature: None,
        }
    }

    /// **Make a component** labelled `label` (for output only).
    pub fn component(&mut self, label: &str) -> Component {
        self.components.push(label.to_string());
        Component {
            model: self.id,
            index: self.components.len() - 1,
        }
    }

    /// The label of `c`, if `c` was made by this model.
    pub fn component_label(&self, c: Component) -> Option<&str> {
        (c.model == self.id)
            .then(|| self.components.get(c.index).map(String::as_str))
            .flatten()
    }

    /// **Add an outermost shape.** Several are allowed if they do not
    /// overlap; vacuum is outside them all.
    pub fn add(&mut self, shape: Shape) {
        self.shapes.push(shape);
    }

    /// **Set the temperature** (on the geometry; materials carry none).
    pub fn set_temperature(&mut self, temperature: Temperature) {
        self.temperature = Some(temperature);
    }

    /// The nuclear-data library.
    pub fn data(&self) -> &NuclearData {
        &self.data
    }

    /// The outermost shapes.
    pub fn shapes(&self) -> &[Shape] {
        &self.shapes
    }

    /// **Check and lower the model** to CSG, materials and nuclide keys,
    /// resolving every material against the library's nuclides and masses.
    /// No nuclear data is processed.
    ///
    /// # Errors
    /// [`ModelError`]: no shape, no temperature, a bad size or temperature,
    /// a foreign component, overlapping shapes or a contained shape poking
    /// out (each with a picture), a malformed lattice or one that does not
    /// fit its container, a boundary condition anywhere but on the one
    /// outermost shape, or a material that does not resolve.
    pub fn assemble(&self) -> Result<AssembledModel, ModelError> {
        if self.shapes.is_empty() {
            return Err(ModelError::Empty);
        }
        let Some(Temperature::Uniform(t)) = self.temperature else {
            return Err(ModelError::NoTemperature);
        };
        let temperature_k = t.get::<kelvin>();
        if !(temperature_k > 0.0 && temperature_k.is_finite()) {
            return Err(ModelError::BadValue(format!(
                "temperature {temperature_k} K must be positive"
            )));
        }
        for s in &self.shapes {
            self.check_shape(s, true)?;
        }
        check_disjoint(&self.shapes)?;
        if self.shapes.len() > 1 && self.shapes.iter().any(|s| s.boundary.is_some()) {
            return Err(ModelError::BadValue(
                "a boundary condition needs one outermost shape; this model has several \
                 (put them inside one shape and set the boundary on it)"
                    .into(),
            ));
        }

        let masses = self.data.atomic_masses();
        let mut lw = Lowering::default();
        let mut root_cells = Vec::new();
        let top: Vec<Lowered> = self
            .shapes
            .iter()
            .map(|s| lw.lower(s, temperature_k, &masses, &mut root_cells))
            .collect::<Result<_, _>>()?;
        let mut lower = [f64::INFINITY; 3];
        let mut upper = [f64::NEG_INFINITY; 3];
        for s in &self.shapes {
            let (l, u) = s.solid.bounds();
            for i in 0..3 {
                lower[i] = lower[i].min(l[i]);
                upper[i] = upper[i].max(u[i]);
            }
        }
        if top.len() == 1 {
            let bc = match self.shapes[0].boundary {
                None | Some(Boundary::Vacuum) => BoundaryType::Vacuum,
                Some(Boundary::Reflective) => BoundaryType::Reflective,
            };
            for &i in &top[0].surfaces {
                set_bc(&mut lw.surfaces[i], bc);
            }
        } else {
            // A void sphere around everything, vacuum on it.
            let c: [f64; 3] = std::array::from_fn(|i| 0.5 * (lower[i] + upper[i]));
            let r = 0.5
                * (0..3)
                    .map(|i| upper[i] - lower[i])
                    .map(|w| w * w)
                    .sum::<f64>()
                    .sqrt()
                * 1.01;
            lw.surfaces.push(sphere_surface(c, r, BoundaryType::Vacuum));
            let s = lw.surfaces.len() - 1;
            let children: Vec<Vec<RegionToken>> = top.into_iter().map(|l| l.outside).collect();
            let region = combine(&[half(s, HalfSpaceSense::Inside)], &children);
            let id = lw.cells.len() as i32 + 1;
            root_cells.push(lw.cells.len());
            lw.cells
                .push(Cell::fill(id, region, CellFill::Void, Position::ZERO));
            lw.cell_components.push(None);
            lw.cell_volumes.push(None);
            for i in 0..3 {
                lower[i] = c[i] - r;
                upper[i] = c[i] + r;
            }
        }
        let fissile_bounds = lw.fissile_bounds.unwrap_or((lower, upper));
        lw.universes.push(Universe {
            id: lw.universes.len() as i32,
            cell_indices: root_cells,
        });
        let root_universe = lw.universes.len() - 1;
        Ok(AssembledModel {
            geometry: Geometry {
                surfaces: lw.surfaces,
                cells: lw.cells,
                universes: lw.universes,
                lattices: lw.lattices,
                root_universe,
            },
            materials: lw.materials,
            resolved: lw.resolved,
            nuclides: lw.nuclides,
            source_box: SourceBox {
                lower: Position::new(
                    fissile_bounds.0[0],
                    fissile_bounds.0[1],
                    fissile_bounds.0[2],
                ),
                upper: Position::new(
                    fissile_bounds.1[0],
                    fissile_bounds.1[1],
                    fissile_bounds.1[2],
                ),
            },
            temperature_k,
            cell_components: lw.cell_components,
            cell_volumes_cm3: lw.cell_volumes,
            component_labels: self.components.clone(),
            data_label: self.data.label().to_string(),
            bounds: (lower, upper),
        })
    }

    fn check_shape(&self, s: &Shape, outermost: bool) -> Result<(), ModelError> {
        if !s.solid.is_valid() {
            return Err(ModelError::BadValue(format!(
                "every size must be positive and finite, not {}",
                s.solid.describe()
            )));
        }
        if let Some(c) = s.component {
            if c.model != self.id {
                return Err(ModelError::ForeignComponent);
            }
        }
        if !outermost && s.boundary.is_some() {
            return Err(ModelError::BadValue(
                "a boundary condition belongs on the outermost shape only".into(),
            ));
        }
        for child in &s.children {
            self.check_shape(child, false)?;
            if !s.solid.encloses(&child.solid) {
                return Err(ModelError::Geometry(problem(
                    &s.solid,
                    &child.solid,
                    format!(
                        "{} pokes out of {} that contains it",
                        child.solid.describe(),
                        s.solid.describe()
                    ),
                )));
            }
        }
        check_disjoint(&s.children)?;
        if let Some(lattice) = &s.lattice {
            self.check_lattice(s, lattice)?;
        }
        Ok(())
    }

    /// A lattice must be well formed, its tiles valid shapes, and its grid
    /// inside a box container that holds nothing else.
    fn check_lattice(&self, s: &Shape, lattice: &Lattice) -> Result<(), ModelError> {
        if let Some(why) = lattice.problem() {
            return Err(ModelError::BadValue(why));
        }
        if !s.children.is_empty() {
            return Err(ModelError::BadValue(
                "a shape filled with a lattice holds nothing else; put other shapes in the tiles"
                    .into(),
            ));
        }
        let Solid::Cuboid(_) = s.solid else {
            return Err(ModelError::BadValue(
                "a lattice goes in a box today (Cube::side or Cuboid::sides); a lattice in a sphere, \
                 with whole-particle rejection, is #926's follow-up"
                    .into(),
            ));
        };
        for tile in lattice.tiles() {
            self.check_shape(tile, false)?;
        }
        let pitch = lattice.pitch_cm().unwrap_or([0.0; 3]);
        let n = lattice.dimensions();
        let c = s.solid.centre_cm();
        let cm = Length::new::<centimeter>;
        let grid = Solid::Cuboid(
            Cuboid::sides(
                cm(n[0] as f64 * pitch[0]),
                cm(n[1] as f64 * pitch[1]),
                cm(n[2] as f64 * pitch[2]),
            )
            .centred_at(cm(c[0]), cm(c[1]), cm(c[2])),
        );
        if !s.solid.encloses(&grid) {
            return Err(ModelError::Geometry(problem(
                &s.solid,
                &grid,
                format!(
                    "the {}x{}x{} lattice ({}) is larger than {} that contains it; a tile would be cut",
                    n[0],
                    n[1],
                    n[2],
                    grid.describe(),
                    s.solid.describe()
                ),
            )));
        }
        Ok(())
    }
}

/// Siblings must not overlap.
fn check_disjoint(shapes: &[Shape]) -> Result<(), ModelError> {
    for (i, a) in shapes.iter().enumerate() {
        for b in &shapes[i + 1..] {
            if a.solid.overlaps(&b.solid) {
                return Err(ModelError::Geometry(problem(
                    &a.solid,
                    &b.solid,
                    "two shapes overlap without one containing the other; nest one with .containing(..) or move it".into(),
                )));
            }
        }
    }
    Ok(())
}

/// Draw two solids on an x-y slice at the first one's centre height.
fn problem(a: &Solid, b: &Solid, message: String) -> GeometryProblem {
    const N: usize = 400;
    let (la, ua) = a.bounds();
    let (lb, ub) = b.bounds();
    let lo = [la[0].min(lb[0]), la[1].min(lb[1])];
    let hi = [ua[0].max(ub[0]), ua[1].max(ub[1])];
    let w = (hi[0] - lo[0]).max(hi[1] - lo[1]) * 1.1;
    let c = [0.5 * (lo[0] + hi[0]), 0.5 * (lo[1] + hi[1])];
    let z = a.centre_cm()[2];
    let mut img = ImageData::filled(N, N, Rgb::new(255, 255, 255));
    for y in 0..N {
        for x in 0..N {
            let p = [
                c[0] - 0.5 * w + (x as f64 + 0.5) * w / N as f64,
                c[1] + 0.5 * w - (y as f64 + 0.5) * w / N as f64,
                z,
            ];
            let colour = match (a.holds(p), b.holds(p)) {
                (true, true) => Rgb::new(220, 30, 30),
                (true, false) => Rgb::new(60, 110, 200),
                (false, true) => Rgb::new(240, 160, 40),
                (false, false) => continue,
            };
            img.set(x, y, colour);
        }
    }
    GeometryProblem {
        message,
        picture: img,
    }
}

fn sphere_surface(c: [f64; 3], r: f64, bc: BoundaryType) -> SurfaceKind {
    SurfaceKind::Sphere(crate::geometry::surface::Sphere {
        x0: c[0],
        y0: c[1],
        z0: c[2],
        r,
        bc,
    })
}

fn set_bc(s: &mut SurfaceKind, bc: BoundaryType) {
    match s {
        SurfaceKind::Sphere(p) => p.bc = bc,
        SurfaceKind::XPlane(p) => p.bc = bc,
        SurfaceKind::YPlane(p) => p.bc = bc,
        SurfaceKind::ZPlane(p) => p.bc = bc,
        _ => {}
    }
}

/// Fixed palette for the beginner plots (materials in first-use order).
const PALETTE: [Rgb; 8] = [
    Rgb {
        r: 214,
        g: 96,
        b: 77,
    },
    Rgb {
        r: 67,
        g: 147,
        b: 195,
    },
    Rgb {
        r: 120,
        g: 180,
        b: 90,
    },
    Rgb {
        r: 240,
        g: 190,
        b: 70,
    },
    Rgb {
        r: 150,
        g: 110,
        b: 190,
    },
    Rgb {
        r: 110,
        g: 110,
        b: 110,
    },
    Rgb {
        r: 230,
        g: 130,
        b: 190,
    },
    Rgb {
        r: 80,
        g: 190,
        b: 190,
    },
];

impl AssembledModel {
    /// **Draw what the solver sees**: a material-coloured slice of the
    /// assembled geometry through the model's centre, framed to the whole
    /// model, with a legend (material names) and cm axes, by the ported
    /// OpenMC plotter (`render_material_slice`). `basis` picks the plane
    /// (`PlotBasis::Xy`, `Xz`, `Yz`). Returns the annotated image; write it
    /// with `ImageData::write_png` or encode with `to_png_bytes`.
    pub fn draw_slice(&self, basis: PlotBasis, pixels: usize, title: &str) -> ImageData {
        let (l, u) = self.bounds;
        let c = Position::new(
            0.5 * (l[0] + u[0]),
            0.5 * (l[1] + u[1]),
            0.5 * (l[2] + u[2]),
        );
        let (h, v) = basis.axes();
        let w = [(u[h] - l[h]) * 1.1, (u[v] - l[v]) * 1.1];
        self.draw_window(basis, c, w, pixels, title)
    }

    /// As [`Self::draw_slice`], at a chosen centre \[cm\] and width \[cm\]
    /// (for zooming in on a level).
    pub fn draw_window(
        &self,
        basis: PlotBasis,
        centre: Position,
        width_cm: [f64; 2],
        pixels: usize,
        title: &str,
    ) -> ImageData {
        let plot = SlicePlot::new(basis, centre, width_cm, [pixels, pixels]);
        let names: Vec<&str> = self.resolved.iter().map(|r| r.name.as_str()).collect();
        let palette: Vec<(Rgb, &str)> = names
            .iter()
            .enumerate()
            .map(|(i, n)| (PALETTE[i % PALETTE.len()], *n))
            .collect();
        render_material_slice(&self.geometry, &plot, &palette, title).1
    }

    /// **Process every nuclide** this model needs from `data` at its
    /// temperature, in [`Self::nuclides`] order. On native targets the
    /// nuclides are processed in parallel (each processing call is
    /// single-threaded and independent, so the data are the same as a serial
    /// run's); on `wasm32` serially.
    ///
    /// # Errors
    /// The first nuclide that is missing or fails to process.
    pub fn load_nuclides(&self, data: &NuclearData) -> Result<Vec<Nuclide>, DataError> {
        #[cfg(not(target_arch = "wasm32"))]
        use rayon::prelude::*;
        #[cfg(target_arch = "wasm32")]
        use crate::wasm_par::prelude::*;
        let t = self.temperature_k;
        self.nuclides
            .par_iter()
            .map(|k| data.load_nuclide(k.isotope, t, k.law))
            .collect()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::isotope::isotopes::*;
    use super::super::material::atoms_per_barn_cm;
    use super::super::material::{Material, Moderator};
    use super::super::shape::Sphere;
    use super::*;
    use uom::si::f64::Length;
    use uom::si::length::centimeter;

    /// A library with fake headers (masses only): enough to assemble.
    pub(crate) fn fake_data() -> NuclearData {
        let endf = |x: f64| {
            let e = x.abs().log10().floor() as i32;
            format!("{:.6}{e:+}", x / 10f64.powi(e))
        };
        let tape = |za: f64, awr: f64| {
            format!(
                " TPID\n{:>11}{:>11}{:>11}{:>11}{:>11}{:>11}\n{:>11}{:>11}{:>11}{:>11}{:>11}{:>11}\n{:>11}{:>11}{:>11}{:>11}{:>11}{:>11}\n",
                endf(za),
                endf(awr),
                0, 0, 0, 0, "0.0", "0.0", 0, 0, 0, 6, "1.0", "2.0+7", 0, 0, 10, 8
            )
            .into_bytes()
        };
        NuclearData::from_endf_bytes(
            "fake",
            [
                ("u234".to_string(), tape(92234.0, 232.0304)),
                ("u235".to_string(), tape(92235.0, 233.0248)),
                ("u238".to_string(), tape(92238.0, 236.0058)),
                ("h1".to_string(), tape(1001.0, 0.9991673)),
                ("o16".to_string(), tape(8016.0, 15.857510)),
            ],
        )
        .unwrap()
    }

    fn cm(x: f64) -> Length {
        Length::new::<centimeter>(x)
    }

    fn heu() -> Material {
        Material::from_atom_densities([
            (U234, atoms_per_barn_cm(4.9184e-4)),
            (U235, atoms_per_barn_cm(4.4994e-2)),
            (U238, atoms_per_barn_cm(2.4984e-3)),
        ])
        .named("HEU")
    }

    fn t() -> Temperature {
        Temperature::Uniform(ThermodynamicTemperature::new::<kelvin>(293.6))
    }

    /// **Godiva lowers to exactly the hand-built CSG**: one vacuum sphere,
    /// one cell `inside(0)`, material 0 with U-234/235/238 at the spec's
    /// densities in nuclide order 0, 1, 2. (The transport bit-for-bit check
    /// on this lowering is `tests/model_lowering_bit_for_bit.rs`.)
    #[test]
    fn godiva_lowers_to_the_hand_built_csg() {
        let mut m = Model::new(fake_data());
        let core = m.component("Core");
        m.add(
            Sphere::radius(cm(8.7407))
                .filled_with(heu())
                .in_component(core),
        );
        assert!(matches!(m.assemble(), Err(ModelError::NoTemperature)));
        m.set_temperature(t());
        let a = m.assemble().unwrap();
        assert_eq!(a.geometry.surfaces.len(), 1);
        match &a.geometry.surfaces[0] {
            SurfaceKind::Sphere(s) => {
                assert_eq!((s.r, s.bc, s.x0), (8.7407, BoundaryType::Vacuum, 0.0))
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(a.geometry.cells.len(), 1);
        assert_eq!(
            a.geometry.cells[0].region,
            vec![RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Inside
            }]
        );
        assert_eq!(a.materials.len(), 1);
        let c: Vec<(usize, f64)> = a.materials[0]
            .components
            .iter()
            .map(|c| (c.nuclide_idx, c.atom_density))
            .collect();
        assert_eq!(c, vec![(0, 4.9184e-4), (1, 4.4994e-2), (2, 2.4984e-3)]);
        assert_eq!(
            a.nuclides.iter().map(|k| k.isotope).collect::<Vec<_>>(),
            vec![U234, U235, U238]
        );
        assert!(a.nuclides.iter().all(|k| k.law.is_none()));
        assert_eq!(a.cell_components, vec![Some(core)]);
        assert_eq!(m.component_label(core), Some("Core"));
        assert_eq!(
            (a.source_box.lower.x, a.source_box.upper.z),
            (-8.7407, 8.7407)
        );
        assert_eq!(a.temperature_k, 293.6);
        assert_eq!(a.data_label, "fake");
        assert_eq!(m.shapes().len(), 1);
        assert_eq!(m.data().label(), "fake");
    }

    /// Three nested spheres lower exactly as `fhr_pebble_geometry` builds
    /// HEU-SOL-THERM-009 (`vv::hst009::geometry`): same surfaces, the same
    /// region tokens per cell, the same material indices.
    #[test]
    fn nested_shells_lower_like_the_hand_built_hst009() {
        let mut m = Model::new(fake_data());
        m.set_temperature(t());
        let water = Material::from_atom_densities([
            (H1, atoms_per_barn_cm(6.6659e-2)),
            (O16, atoms_per_barn_cm(3.33e-2)),
        ])
        .bound_in(Moderator::LightWater)
        .named("water");
        let tank = Material::from_atom_densities([(O16, atoms_per_barn_cm(0.05))]).named("tank");
        let solution = Sphere::radius(cm(11.5177)).filled_with(heu());
        let tank_ball = Sphere::radius(cm(11.6764))
            .filled_with(tank)
            .containing(solution);
        let reflector = Sphere::radius(cm(35.0))
            .filled_with(water)
            .containing(tank_ball);
        m.add(reflector);
        let a = m.assemble().unwrap();
        let h = crate::vv::hst009::geometry();
        assert_eq!(a.geometry.surfaces.len(), h.surfaces.len());
        for (x, y) in a.geometry.surfaces.iter().zip(&h.surfaces) {
            match (x, y) {
                (SurfaceKind::Sphere(p), SurfaceKind::Sphere(q)) => {
                    assert_eq!((p.r, p.bc), (q.r, q.bc))
                }
                _ => panic!("not spheres"),
            }
        }
        for (x, y) in a.geometry.cells.iter().zip(&h.cells) {
            assert_eq!(x.region, y.region);
            assert_eq!(x.fill, y.fill);
        }
        let h1 = a.nuclides.iter().find(|k| k.isotope == H1).unwrap();
        assert_eq!(h1.law, Some(SabMaterial::HInH2O));
        assert_eq!(
            a.source_box.upper.x, 11.5177,
            "the source starts in the solution only"
        );
        let img = a.draw_slice(PlotBasis::Xy, 64, "hst009");
        assert!(img.width >= 64);
    }

    /// Refusals: overlaps and poking out come with a picture, a foreign
    /// component and bad values are named, and two disjoint outermost shapes
    /// get a void sphere with the vacuum boundary around them.
    #[test]
    fn refusals_and_several_outermost_shapes() {
        let mut m = Model::new(fake_data());
        m.set_temperature(t());
        assert!(matches!(m.assemble(), Err(ModelError::Empty)));
        m.add(
            Sphere::radius(cm(2.0))
                .filled_with(heu())
                .containing(Sphere::radius(cm(3.0)).filled_with(heu())),
        );
        match m.assemble() {
            Err(ModelError::Geometry(p)) => {
                assert!(p.message.contains("pokes out"));
                assert_eq!((p.picture.width, p.picture.height), (400, 400));
                assert!(p.to_string().contains("blue"));
            }
            other => panic!("{other:?}"),
        }
        let mut m = Model::new(fake_data());
        m.set_temperature(t());
        m.add(Sphere::radius(cm(2.0)).filled_with(heu()));
        m.add(
            Sphere::radius(cm(2.0))
                .centred_at(cm(3.0), cm(0.0), cm(0.0))
                .filled_with(heu()),
        );
        assert!(matches!(m.assemble(), Err(ModelError::Geometry(_))));

        let mut m = Model::new(fake_data());
        m.set_temperature(t());
        m.add(Sphere::radius(cm(1.0)).filled_with(heu()));
        m.add(
            Sphere::radius(cm(1.0))
                .centred_at(cm(5.0), cm(0.0), cm(0.0))
                .filled_with(heu()),
        );
        let a = m.assemble().unwrap();
        assert_eq!(a.geometry.cells.len(), 3);
        assert_eq!(a.geometry.cells[2].fill, CellFill::Void);
        assert_eq!(a.materials.len(), 1, "equal materials share one entry");
        match a.geometry.surfaces.last().unwrap() {
            SurfaceKind::Sphere(s) => assert_eq!(s.bc, BoundaryType::Vacuum),
            other => panic!("{other:?}"),
        }

        let mut other = Model::new(fake_data());
        let foreign = other.component("x");
        let mut m = Model::new(fake_data());
        m.set_temperature(t());
        assert_eq!(m.component_label(foreign), None);
        m.add(
            Sphere::radius(cm(1.0))
                .filled_with(heu())
                .in_component(foreign),
        );
        assert!(matches!(m.assemble(), Err(ModelError::ForeignComponent)));

        let mut m = Model::new(fake_data());
        m.set_temperature(t());
        m.add(Sphere::radius(cm(-1.0)).filled_with(heu()));
        assert!(matches!(m.assemble(), Err(ModelError::BadValue(_))));
        let mut m = Model::new(fake_data());
        m.set_temperature(Temperature::Uniform(
            ThermodynamicTemperature::new::<kelvin>(0.0),
        ));
        m.add(Sphere::radius(cm(1.0)).filled_with(heu()));
        assert!(matches!(m.assemble(), Err(ModelError::BadValue(_))));
        let mut m = Model::new(fake_data());
        m.set_temperature(t());
        m.add(
            Sphere::radius(cm(1.0)).filled_with(Material::from_atom_densities([(
                Pu239,
                atoms_per_barn_cm(0.04),
            )])),
        );
        assert!(matches!(m.assemble(), Err(ModelError::Material(_))));
    }

    /// **Repeated structures (#926): every refusal is named or drawn**, a
    /// lattice smaller than its box has the box's material around it, and
    /// the census counts that outer region once.
    #[test]
    fn repeated_structure_refusals_and_a_lattice_smaller_than_its_box() {
        use super::super::lattice::Lattice;
        use super::super::shape::{Boundary, Cube, Cuboid};
        use uom::si::volume::cubic_centimeter;
        let fuel = || Sphere::radius(cm(0.3)).filled_with(heu());
        let tile = || {
            Cube::side(cm(1.0))
                .filled_with(heu().named("matrix"))
                .containing(fuel())
        };
        let build = |s: Shape| {
            let mut m = Model::new(fake_data());
            m.set_temperature(t());
            m.add(s);
            m.assemble()
        };
        // A 2x2x2 grid in a 3 cm box: 19 cm3 of box material around it.
        let a = build(
            Cube::side(cm(3.0))
                .filled_with(heu().named("box"))
                .containing_lattice(Lattice::copies_of(tile(), [2, 2, 2]))
                .boundary(Boundary::Reflective),
        )
        .unwrap();
        let census = a.census().unwrap();
        let outer = census.rows.iter().find(|r| r.material == "box").unwrap();
        assert_eq!(outer.copies, 1);
        assert!((outer.volume().get::<cubic_centimeter>() - 19.0).abs() < 1e-12);
        let total = census.total_volume().get::<cubic_centimeter>();
        assert!((total - 27.0).abs() < 1e-12, "{total}");
        assert!(matches!(
            &a.geometry.surfaces[1],
            SurfaceKind::XPlane(p) if p.bc == BoundaryType::Reflective
        ));
        // The source starts over the whole box (tile positions are local).
        assert_eq!((a.source_box.lower.x, a.source_box.upper.z), (-1.5, 1.5));

        // Too big for its box: a picture.
        match build(
            Cube::side(cm(1.5))
                .filled_with(heu())
                .containing_lattice(Lattice::copies_of(tile(), [2, 2, 2])),
        ) {
            Err(ModelError::Geometry(p)) => assert!(p.message.contains("tile would be cut")),
            other => panic!("{other:?}"),
        }
        let refused = |s: Shape, words: &str| match build(s) {
            Err(ModelError::BadValue(m)) => assert!(m.contains(words), "{m}"),
            other => panic!("{words}: {other:?}"),
        };
        refused(
            Sphere::radius(cm(3.0))
                .filled_with(heu())
                .containing_lattice(Lattice::copies_of(tile(), [1, 1, 1])),
            "a lattice goes in a box",
        );
        refused(
            Cube::side(cm(3.0))
                .filled_with(heu())
                .containing(fuel())
                .containing_lattice(Lattice::copies_of(tile(), [1, 1, 1])),
            "holds nothing else",
        );
        refused(
            Cube::side(cm(3.0))
                .filled_with(heu())
                .containing_lattice(Lattice::copies_of(tile(), [0, 1, 1])),
            "at least one tile per axis",
        );
        refused(
            Cube::side(cm(3.0))
                .filled_with(heu())
                .containing(fuel().boundary(Boundary::Reflective)),
            "outermost shape only",
        );
        refused(Cube::side(cm(-1.0)).filled_with(heu()), "positive and finite");
        // A box poking out of a sphere, and a tile's particle poking out of
        // its tile, are drawn.
        match build(
            Sphere::radius(cm(1.0))
                .filled_with(heu())
                .containing(Cube::side(cm(1.9)).filled_with(heu())),
        ) {
            Err(ModelError::Geometry(p)) => assert!(p.message.contains("pokes out")),
            other => panic!("{other:?}"),
        }
        let fat = Cube::side(cm(1.0))
            .filled_with(heu())
            .containing(Sphere::radius(cm(0.6)).filled_with(heu()));
        assert!(matches!(
            build(
                Cube::side(cm(2.0))
                    .filled_with(heu())
                    .containing_lattice(Lattice::copies_of(fat, [2, 2, 2]))
            ),
            Err(ModelError::Geometry(_))
        ));
        // A boundary needs one outermost shape.
        let mut m = Model::new(fake_data());
        m.set_temperature(t());
        m.add(
            Sphere::radius(cm(1.0))
                .filled_with(heu())
                .boundary(Boundary::Vacuum),
        );
        m.add(
            Cuboid::sides(cm(1.0), cm(1.0), cm(1.0))
                .centred_at(cm(5.0), cm(0.0), cm(0.0))
                .filled_with(heu()),
        );
        assert!(
            matches!(m.assemble(), Err(ModelError::BadValue(w)) if w.contains("one outermost"))
        );
    }
}
