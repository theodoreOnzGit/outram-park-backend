//! Mesh and solve, off the UI thread: a background thread natively, the Web
//! Worker in the browser (`web/foam_studio/worker.js`). Every request is
//! short (a mesh, or one chunk of time steps), so a new design or a stop is
//! served within one chunk.
//!
//! **The physics is not here.** Meshing is `outram_foam_mesh::block_primitives`
//! (the pipe O-grid and the box block, through the blockMesh port); the
//! solver is `outram_foam_appbuilder_lib`'s `PimpleFoam` with the boundary
//! conditions and diagnostics of `cases::laminar_pipe`, unchanged. V&V record:
//! `crates/outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155.md`.
//! This file chooses resolutions, steps in chunks and cuts the sections the
//! page draws.
use std::sync::Arc;

use outram_foam_appbuilder_lib::cases::double_pipe::{self, DoublePipeExchanger, ExchangerReport};
use outram_foam_appbuilder_lib::cases::laminar_pipe::{max_continuity_error, patch_flux, PipeFlow};
use outram_foam_appbuilder_lib::solvers::cht_multi_region_foam::ChtMultiRegionFoam;
use outram_foam_appbuilder_lib::solvers::pimple_foam::PimpleFoam;
use outram_foam_basic_lib::fv_operators::fvm::max_non_orthogonality_deg;
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_mesh::block_mesh::PolyMesh;
use outram_foam_mesh::block_primitives::{BoxBlock, DoublePipe, PipeOGrid};
use outram_foam_mesh::section::{boundary_edges, plane_section, Axis, CellSection};
use uom::si::f64::{KinematicViscosity, Length, Velocity};
use uom::si::kinematic_viscosity::square_meter_per_second;
use uom::si::length::meter;
use uom::si::velocity::meter_per_second;

/// Default cell count: interactive (maintainer decision, 2026-11-03).
pub const DEFAULT_CELLS: usize = 20_000;
/// Largest cell count offered; above [`DEFAULT_CELLS`] it is a batch run.
pub const MAX_CELLS: usize = 50_000;
/// Inlet (bulk) velocity \[m/s\]; Re is set through ν.
pub const U_IN: f64 = 5.0e-3;
/// Largest viscous number ν Δt / h² a run uses (h the mean cell size
/// (V/N)^(1/3)): a guard for low Re, where `PimpleFoam` diverges (stage 0,
/// gh:#1162). 0.5 is where both measured-stable 20k pipe runs sit (Re = 100
/// at Co ≈ 4 and Re = 25 at Co ≈ 1: 0.48 each, gh:#1161 record); with 2,
/// Re = 10 diverged on the 20k pipe (measured 2026-11-03).
/// `FOAM_MAX_VISCOUS` (native only) overrides it for experiments.
pub const MAX_VISCOUS_DEFAULT: f64 = 0.5;

/// The steady criterion: max cell |ΔU| / U per flow-through time (stage 0).
pub const STEADY: f64 = 1.0e-3;

/// What a primitive face becomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    Inlet,
    Outlet,
    Wall,
}

impl Tag {
    pub fn name(self) -> &'static str {
        match self {
            Tag::Inlet => "inlet",
            Tag::Outlet => "outlet",
            Tag::Wall => "walls",
        }
    }
    fn code(self) -> f64 {
        match self {
            Tag::Inlet => 0.0,
            Tag::Outlet => 1.0,
            Tag::Wall => 2.0,
        }
    }
    fn from_code(c: f64) -> Self {
        match c as u8 {
            0 => Tag::Inlet,
            1 => Tag::Outlet,
            _ => Tag::Wall,
        }
    }
}

/// The primitive, in metres, positioned by its centre.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    /// A cylinder along z. Faces: z-min end, z-max end, side.
    Cylinder { radius: f64, length: f64, centre: [f64; 3] },
    /// An axis-aligned box. Faces: x-, x+, y-, y+, z-, z+.
    Box { size: [f64; 3], centre: [f64; 3] },
    /// A counterflow double-pipe heat exchanger along z (gh:#1156): hot
    /// fluid in the tube (radius `r_tube`, in at z−), a conducting tube wall
    /// of thickness `wall`, cold fluid in the annulus out to `r_shell` (in at
    /// z+), adiabatic shell. Its patches are fixed, not tagged.
    DoublePipe { r_tube: f64, wall: f64, r_shell: f64, length: f64, centre: [f64; 3] },
}

impl Shape {
    /// Number of taggable faces.
    pub fn n_faces(&self) -> usize {
        match self {
            Shape::Cylinder { .. } => 3,
            Shape::Box { .. } => 6,
            Shape::DoublePipe { .. } => 0,
        }
    }
    /// Human names of the faces.
    pub fn face_names(&self) -> &'static [&'static str] {
        match self {
            Shape::Cylinder { .. } => &["end z−", "end z+", "side"],
            Shape::Box { .. } => &["x−", "x+", "y−", "y+", "z−", "z+"],
            Shape::DoublePipe { .. } => &[],
        }
    }
    /// Bounding box `[min, max]` \[m\].
    pub fn bounds(&self) -> [[f64; 3]; 2] {
        match *self {
            Shape::Cylinder { radius, length, centre: c } | Shape::DoublePipe { r_shell: radius, length, centre: c, .. } => {
                [[c[0] - radius, c[1] - radius, c[2] - 0.5 * length], [c[0] + radius, c[1] + radius, c[2] + 0.5 * length]]
            }
            Shape::Box { size: s, centre: c } => {
                [[c[0] - 0.5 * s[0], c[1] - 0.5 * s[1], c[2] - 0.5 * s[2]], [c[0] + 0.5 * s[0], c[1] + 0.5 * s[1], c[2] + 0.5 * s[2]]]
            }
        }
    }
}

/// A design: the shape, a tag per face (only the first `n_faces` count) and
/// the cell budget.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Design {
    pub shape: Shape,
    pub tags: [Tag; 6],
    pub target_cells: usize,
}

impl Default for Design {
    /// The stage 0 pipe: D = 20 mm, L = 200 mm, inlet at z−, 20k cells.
    fn default() -> Self {
        Self {
            shape: Shape::Cylinder { radius: 0.01, length: 0.2, centre: [0.0, 0.0, 0.1] },
            tags: [Tag::Inlet, Tag::Outlet, Tag::Wall, Tag::Wall, Tag::Wall, Tag::Wall],
            target_cells: DEFAULT_CELLS,
        }
    }
}

impl Design {
    /// The demo's double-pipe exchanger: tube r = 5 mm with a 1 mm wall, shell
    /// r = 10 mm, L = 200 mm (the geometry of the V&V record
    /// `cht_double_pipe_gh1156.md`), 20k cells.
    pub fn exchanger() -> Self {
        Self {
            shape: Shape::DoublePipe { r_tube: 0.005, wall: 0.001, r_shell: 0.01, length: 0.2, centre: [0.0, 0.0, 0.1] },
            tags: [Tag::Wall; 6],
            target_cells: DEFAULT_CELLS,
        }
    }
}

/// Why a design cannot be meshed or run, in words for the page.
pub fn check(d: &Design) -> Result<(), String> {
    if d.target_cells == 0 || d.target_cells > MAX_CELLS {
        return Err(format!("cells must be 1..{MAX_CELLS}"));
    }
    if let Shape::DoublePipe { r_tube, wall, r_shell, length, .. } = d.shape {
        if !(r_tube > 1e-4 && wall > 1e-5 && r_shell > r_tube + wall + 1e-4 && length > 1e-3) {
            return Err("the tube, its wall and the shell must nest with positive gaps".into());
        }
        return Ok(());
    }
    let tags = &d.tags[..d.shape.n_faces()];
    let n_in = tags.iter().filter(|t| **t == Tag::Inlet).count();
    if n_in != 1 {
        return Err(format!("tag exactly one face as the inlet (now {n_in})"));
    }
    if !tags.contains(&Tag::Outlet) {
        return Err("tag at least one face as an outlet".into());
    }
    if d.shape == (Shape::Cylinder { radius: 0.0, length: 0.0, centre: [0.0; 3] }) {
        return Err("empty shape".into());
    }
    let [lo, hi] = d.shape.bounds();
    if (0..3).any(|i| !(hi[i] - lo[i] > 1e-6)) {
        return Err("every dimension must be positive".into());
    }
    if d.target_cells == 0 || d.target_cells > MAX_CELLS {
        return Err(format!("cells must be 1..{MAX_CELLS}"));
    }
    Ok(())
}

fn m(v: f64) -> Length {
    Length::new::<meter>(v)
}

/// The demo's O-grid of side `n`: `(n, n − 2, n·L/D)`, the stage 0 family
/// (8/6/80 is 20 480 cells at L/D = 10). Its radial cells are a third longer
/// than `PipeOGrid::level`'s (nr = n), which keeps the viscous number at
/// Co ≈ 4 under 2: on the level family the same budget needs Co ≈ 2 and
/// four times the steps (measured 2026-11-03, 20k: 136 steps).
pub fn demo_ogrid(radius: f64, length: f64, n: usize) -> PipeOGrid {
    let ld = length / (2.0 * radius);
    let nz = ((n as f64 * ld).round() as usize).max(1);
    PipeOGrid::new(m(radius), m(length), n, n.saturating_sub(2).max(1), nz)
}

/// The pipe O-grid for a cylinder: the largest [`demo_ogrid`] within 5 % of
/// the budget (n ≥ 2).
pub fn pipe_grid(radius: f64, length: f64, centre: [f64; 3], target: usize) -> PipeOGrid {
    let mut lvl = 2;
    for k in 2..128 {
        if demo_ogrid(radius, length, k).n_cells() as f64 <= 1.05 * target as f64 {
            lvl = k;
        } else {
            break;
        }
    }
    let mut g = demo_ogrid(radius, length, lvl);
    g.origin = [m(centre[0]), m(centre[1]), m(centre[2] - 0.5 * length)];
    g
}

/// The box block: near-cubic cells, at most the budget.
pub fn box_block(size: [f64; 3], centre: [f64; 3], target: usize) -> BoxBlock {
    let vol = size[0] * size[1] * size[2];
    let mut h = (vol / target.max(1) as f64).cbrt();
    let cells = loop {
        let c = size.map(|s| ((s / h).round() as usize).max(1));
        if c.iter().product::<usize>() <= target.max(1) || h > 1e9 {
            break c;
        }
        h *= 1.02;
    };
    let mut b = BoxBlock::new(size.map(m), cells);
    b.origin = [0, 1, 2].map(|i| m(centre[i] - 0.5 * size[i]));
    b
}

/// The double-pipe grid for a cell budget: `4 n` cells around every region;
/// radial cells `n − 1` (tube fluid ring), 2 (wall), `n` (annulus); axial
/// `1.5 n L / (2 r_shell)`; the largest n ≥ 3 within 5 % of the budget.
pub fn exchanger_grid(r_tube: f64, wall: f64, r_shell: f64, length: f64, centre: [f64; 3], target: usize) -> DoublePipe {
    let make = |n: usize| DoublePipe {
        r_tube_inner: m(r_tube),
        r_tube_outer: m(r_tube + wall),
        r_shell: m(r_shell),
        length: m(length),
        origin: [m(centre[0]), m(centre[1]), m(centre[2] - 0.5 * length)],
        n,
        nr: [n - 1, 2, n],
        nz: ((1.5 * n as f64 * length / (2.0 * r_shell)).round() as usize).max(2),
    };
    let mut best = make(3);
    for n in 3..64 {
        let g = make(n);
        if g.n_cells().iter().sum::<usize>() as f64 <= 1.05 * target as f64 {
            best = g;
        } else {
            break;
        }
    }
    best
}

/// The mesh of a design (checked), and the flow axis (0 x, 1 y, 2 z) with the
/// signed inlet-to-outlet direction, and the cell length along it \[m\].
pub fn mesh_design(d: &Design) -> Result<(PolyMesh, usize, f64), String> {
    check(d)?;
    let name = |i: usize| d.tags[i].name().to_string();
    match d.shape {
        Shape::Cylinder { radius, length, centre } => {
            let mut g = pipe_grid(radius, length, centre, d.target_cells);
            g.patch_names = [name(0), name(1), name(2)];
            let dz = length / g.nz as f64;
            g.mesh().map(|p| (p, 2, dz)).map_err(|e| e.to_string())
        }
        Shape::Box { size, centre } => {
            let mut b = box_block(size, centre, d.target_cells);
            b.patch_names = [name(0), name(1), name(2), name(3), name(4), name(5)];
            let inlet = d.tags.iter().take(6).position(|t| *t == Tag::Inlet).unwrap_or(4);
            let axis = inlet / 2;
            let h = size[axis] / b.cells[axis] as f64;
            b.mesh().map(|p| (p, axis, h)).map_err(|e| e.to_string())
        }
        Shape::DoublePipe { r_tube, wall, r_shell, length, centre } => {
            let g = exchanger_grid(r_tube, wall, r_shell, length, centre, d.target_cells);
            let [a, b, c] = g.meshes().map_err(|e| e.to_string())?;
            let all = PolyMesh::concatenate(&[("tube_fluid", &a), ("tube_wall", &b), ("annulus_fluid", &c)]);
            Ok((all, 2, length / g.nz as f64))
        }
    }
}

/// The three region meshes of a double-pipe design (tube fluid, tube wall,
/// annulus fluid), in the order [`mesh_design`] concatenates them.
pub fn exchanger_meshes(d: &Design) -> Result<[Arc<FvMesh>; 3], String> {
    let Shape::DoublePipe { r_tube, wall, r_shell, length, centre } = d.shape else {
        return Err("not a double pipe".into());
    };
    let g = exchanger_grid(r_tube, wall, r_shell, length, centre, d.target_cells);
    let [a, b, c] = g.meshes().map_err(|e| e.to_string())?;
    let fv = |p: PolyMesh| p.to_fv_mesh().map(Arc::new).map_err(|e| e.to_string());
    Ok([fv(a)?, fv(b)?, fv(c)?])
}

/// How the solve runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Pseudo-transient PISO at Co ≈ 4 to the steady state (the default),
    /// with Δt capped by the viscous number (see [`MAX_VISCOUS_DEFAULT`]).
    Steady,
    /// Co ≈ 1, to watch the profile develop.
    Transient,
}

impl Mode {
    pub fn courant(self) -> f64 {
        match self {
            Mode::Steady => 4.0,
            Mode::Transient => 1.0,
        }
    }
}

/// What the UI asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// Mesh `design`, replacing any case; the probe sits at fraction `probe`
    /// of the way from inlet to outlet on the axis.
    Mesh { id: u32, design: Design, probe: f64 },
    /// Set up the solve on the current mesh from rest.
    Start { id: u32, reynolds: f64, mode: Mode },
    /// Advance `steps` time steps and report.
    Step { id: u32, steps: usize },
    /// Move the probe and the cross-section to fraction `probe`.
    Probe { id: u32, probe: f64 },
}

/// A plane section, ready to draw: the plane (axis 0/1/2, offset \[m\]) and
/// one polygon per cut cell in in-plane coordinates \[m\].
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub axis: u8,
    pub offset: f64,
    pub polygons: Vec<Vec<[f32; 2]>>,
}

/// What the mesh looks like to the page.
#[derive(Clone, Debug, PartialEq)]
pub struct Meshed {
    pub id: u32,
    pub n_cells: usize,
    pub max_non_ortho_deg: f64,
    /// Flow axis 0/1/2 and the cell length along it \[m\].
    pub flow_axis: u8,
    pub cell_len: f64,
    /// Boundary edges: x0 y0 z0 x1 y1 z1 tag (0 inlet, 1 outlet, 2 wall).
    pub edges: Vec<f32>,
    pub axial: Section,
    pub cross: Section,
}

/// One report of the solve.
#[derive(Clone, Debug, PartialEq)]
pub struct Progress {
    pub id: u32,
    pub steps: usize,
    pub time_s: f64,
    pub dt_s: f64,
    /// Max cell |ΔU|/U per flow-through time (the steady metric).
    pub change: f64,
    /// Worst cell |Σ φ| over the inlet flow rate.
    pub continuity: f64,
    pub outlet_over_inlet: f64,
    /// |U| \[m/s\] and kinematic p \[m²/s²\] at the probe cell.
    pub probe_u: f64,
    pub probe_p: f64,
    /// Mean p of the inlet cells minus that of the outlet cells \[m²/s²\].
    pub dp: f64,
    pub ms_per_step: f64,
    pub steady: bool,
    /// Per section cell: |U|, kinematic p, and the axial component of U.
    pub axial_u: Vec<f32>,
    pub axial_p: Vec<f32>,
    pub cross_u: Vec<f32>,
    pub cross_p: Vec<f32>,
    pub cross_ua: Vec<f32>,
    /// 0 while the flow is marched, 1 once it is frozen and the exchanger's
    /// temperatures are iterated (the pipe stays at 0).
    pub phase: u8,
    /// Per section cell: T \[K\] (empty for the pipe).
    pub axial_t: Vec<f32>,
    pub cross_t: Vec<f32>,
    /// The exchanger's numbers (NaN for the pipe): see [`Exch`].
    pub exch: [f64; EXCH_LEN],
}

/// Indices into [`Progress::exch`].
pub struct Exch;
impl Exch {
    /// Hot (tube) and cold (annulus) mixing-cup outlet temperatures \[K\].
    pub const T_HOT_OUT: usize = 0;
    pub const T_COLD_OUT: usize = 1;
    /// Heat lost by the hot stream and gained by the cold one \[W\].
    pub const Q_HOT: usize = 2;
    pub const Q_COLD: usize = 3;
    /// Solved effectiveness, and ε-NTU (dwsim-libs, counter-current) at the
    /// local UA.
    pub const EPS: usize = 4;
    pub const EPS_NTU: usize = 5;
    /// Local UA \[W/K\] and NTU = UA/C_min.
    pub const UA: usize = 6;
    pub const NTU: usize = 7;
    /// Largest |ΔT| of the last energy pass over the inlet difference.
    pub const ENERGY_CHANGE: usize = 8;
}
/// Length of [`Progress::exch`].
pub const EXCH_LEN: usize = 9;
/// Hot and cold inlet temperatures of the exchanger \[K\] (the
/// `DoublePipeExchanger::demo` values).
pub const T_HOT_IN: f64 = 350.0;
pub const T_COLD_IN: f64 = 300.0;
/// The exchanger's energy is steady when an energy pass changes no cell by
/// more than this fraction of the inlet temperature difference.
pub const ENERGY_STEADY: f64 = 1.0e-7;
/// Energy passes per exchanger step (one request).
pub const ENERGY_PASSES_PER_STEP: usize = 10;

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Meshed(Meshed),
    /// A moved cross-section (after [`Request::Probe`]).
    Cross { id: u32, cross: Section },
    Progress(Progress),
    Error(String),
}

/// A running double-pipe exchanger: the three coupled regions, the case,
/// whether the flow is frozen yet, and where each region's cells start in
/// the concatenated drawing mesh.
pub struct ExchangerRun {
    pub cht: ChtMultiRegionFoam,
    pub spec: DoublePipeExchanger,
    pub energy_phase: bool,
    pub offsets: [usize; 3],
    pub last_energy_change: f64,
}

/// What is being solved.
pub enum Case {
    /// A pipe or duct by `PimpleFoam` (stage 1).
    Pipe(PimpleFoam, PipeFlow),
    /// The double-pipe exchanger by `ChtMultiRegionFoam` (stage 2).
    Exchanger(ExchangerRun),
}

/// The engine: at most one mesh and one case.
#[derive(Default)]
pub struct Engine {
    id: u32,
    poly: Option<PolyMesh>,
    fv: Option<Arc<FvMesh>>,
    design: Option<Design>,
    flow_axis: usize,
    cell_len: f64,
    axial: Vec<CellSection>,
    cross: Vec<CellSection>,
    probe_cell: usize,
    case: Option<Case>,
    steps: usize,
    time: f64,
}

fn axis_of(i: usize) -> Axis {
    match i {
        0 => Axis::X,
        1 => Axis::Y,
        _ => Axis::Z,
    }
}

/// One value per section cell.
fn pick(cs: &[CellSection], f: impl Fn(usize) -> f64) -> Vec<f32> {
    cs.iter().map(|c| f(c.cell) as f32).collect()
}

fn section_of(cs: &[CellSection], axis: usize, offset: f64) -> Section {
    Section {
        axis: axis as u8,
        offset,
        polygons: cs.iter().map(|c| c.polygon.iter().map(|p| [p[0] as f32, p[1] as f32]).collect()).collect(),
    }
}

/// The viscous-number cap of the run (native: `FOAM_MAX_VISCOUS` overrides).
fn max_viscous() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    let v = std::env::var("FOAM_MAX_VISCOUS").ok().and_then(|v| v.parse().ok()).unwrap_or(MAX_VISCOUS_DEFAULT);
    #[cfg(target_arch = "wasm32")]
    let v = MAX_VISCOUS_DEFAULT;
    v
}

/// The exchanger's numbers from the case report, with ε-NTU at the local UA
/// from `outram-park-fork-dwsim-libs` (counter-current).
pub fn exchanger_numbers(r: &ExchangerReport, energy_change: f64) -> [f64; EXCH_LEN] {
    use outram_park_fork_dwsim_libs::heat_exchanger::{lmtd::FlowArrangement, ntu_effectiveness::evaluate};
    use uom::si::f64::{TemperatureInterval, ThermalConductance};
    use uom::si::thermal_conductance::watt_per_kelvin;
    let w = ThermalConductance::new::<watt_per_kelvin>;
    let e = evaluate(
        FlowArrangement::CounterCurrent,
        w(r.ua_local.max(0.0)),
        w(r.c_hot),
        w(r.c_cold),
        TemperatureInterval::new::<uom::si::temperature_interval::kelvin>(T_HOT_IN - T_COLD_IN),
    );
    let mut x = [f64::NAN; EXCH_LEN];
    x[Exch::T_HOT_OUT] = r.t_hot_out;
    x[Exch::T_COLD_OUT] = r.t_cold_out;
    x[Exch::Q_HOT] = r.q_hot;
    x[Exch::Q_COLD] = r.q_cold;
    x[Exch::EPS] = r.effectiveness;
    x[Exch::EPS_NTU] = e.effectiveness.get::<uom::si::ratio::ratio>();
    x[Exch::UA] = r.ua_local;
    x[Exch::NTU] = e.ntu.get::<uom::si::ratio::ratio>();
    x[Exch::ENERGY_CHANGE] = energy_change;
    x
}

impl Engine {
    /// The in-plane axis for the axial section: y unless the flow is along y.
    fn axial_axis(&self) -> usize {
        if self.flow_axis == 1 {
            0
        } else {
            1
        }
    }

    fn centre(&self) -> [f64; 3] {
        let [lo, hi] = self.design.map_or([[0.0; 3]; 2], |d| d.shape.bounds());
        [0, 1, 2].map(|i| 0.5 * (lo[i] + hi[i]))
    }

    /// Where the probe and the cross-section sit for fraction `f`; the plane
    /// is nudged off the grid planes by 1e-4 of a cell.
    fn place_probe(&mut self, f: f64) -> Section {
        let (Some(d), Some(poly), Some(fv)) = (self.design, &self.poly, &self.fv) else {
            return section_of(&[], 2, 0.0);
        };
        let [lo, hi] = d.shape.bounds();
        let a = self.flow_axis;
        let inlet_low = match d.shape {
            Shape::Cylinder { .. } => d.tags[0] == Tag::Inlet,
            Shape::Box { .. } => d.tags[2 * a] == Tag::Inlet,
            // The hot (tube) inlet is at z−.
            Shape::DoublePipe { .. } => true,
        };
        let f = f.clamp(0.0, 1.0);
        let x = if inlet_low { lo[a] + f * (hi[a] - lo[a]) } else { hi[a] - f * (hi[a] - lo[a]) };
        let x = x.clamp(lo[a] + 0.51 * self.cell_len, hi[a] - 0.51 * self.cell_len) + 1e-4 * self.cell_len;
        self.cross = plane_section(poly, axis_of(a), m(x));
        let mut p = self.centre();
        p[a] = x;
        self.probe_cell = (0..fv.n_cells)
            .min_by(|&i, &j| {
                let di = fv.cell_centres[i] - outram_foam_basic_lib::primitives::Vector3::new(p[0], p[1], p[2]);
                let dj = fv.cell_centres[j] - outram_foam_basic_lib::primitives::Vector3::new(p[0], p[1], p[2]);
                di.mag().total_cmp(&dj.mag())
            })
            .unwrap_or(0);
        section_of(&self.cross, a, x)
    }

    fn mesh(&mut self, id: u32, design: Design, probe: f64) -> Result<Meshed, String> {
        let (poly, axis, cell_len) = mesh_design(&design)?;
        let fv = Arc::new(poly.to_fv_mesh().map_err(|e| e.to_string())?);
        self.id = id;
        self.design = Some(design);
        self.flow_axis = axis;
        self.cell_len = cell_len;
        self.case = None;
        // Edge colours: inlet 0, outlet 1, anything else a wall (2). The
        // exchanger's patches are "<region>/<patch>".
        let tag_of: Vec<f32> = poly
            .patches
            .iter()
            .map(|p| match p.name.rsplit('/').next().unwrap_or("") {
                "inlet" => 0.0,
                "outlet" => 1.0,
                _ => 2.0,
            })
            .collect();
        let mut edges = Vec::new();
        for (a, b, pi) in boundary_edges(&poly) {
            edges.extend(a.iter().chain(b.iter()).map(|v| *v as f32));
            edges.push(tag_of[pi]);
        }
        let aa = self.axial_axis();
        let off = self.centre()[aa] + 1e-7;
        self.axial = plane_section(&poly, axis_of(aa), m(off));
        let n_cells = fv.n_cells;
        let max_non_ortho_deg = max_non_orthogonality_deg(&fv);
        self.poly = Some(poly);
        self.fv = Some(fv);
        let cross = self.place_probe(probe);
        Ok(Meshed {
            id,
            n_cells,
            max_non_ortho_deg,
            flow_axis: axis as u8,
            cell_len,
            edges,
            axial: section_of(&self.axial, aa, off),
            cross,
        })
    }

    /// The flow set-up of the current design at `reynolds`, on the inlet's
    /// hydraulic diameter (the tube's, for the exchanger).
    fn flow(&self, reynolds: f64) -> PipeFlow {
        let (dh, len) = match self.design.map(|d| d.shape) {
            Some(Shape::Cylinder { radius, length, .. }) => (2.0 * radius, length),
            Some(Shape::DoublePipe { r_tube, length, .. }) => (2.0 * r_tube, length),
            Some(Shape::Box { size, .. }) => {
                let a = self.flow_axis;
                let (b, c) = (size[(a + 1) % 3], size[(a + 2) % 3]);
                (2.0 * b * c / (b + c), size[a])
            }
            None => (0.02, 0.2),
        };
        PipeFlow {
            radius: m(0.5 * dh),
            length: m(len),
            bulk_velocity: Velocity::new::<meter_per_second>(U_IN),
            nu: KinematicViscosity::new::<square_meter_per_second>(U_IN * dh / reynolds.max(1e-3)),
        }
    }

    /// The exchanger at tube Reynolds number `reynolds`: `DoublePipeExchanger::demo`
    /// with the design's length and the flow rate that gives that Re in the
    /// tube (ν fixed at 1e-6 m²/s); each fluid at Co 4 capped by the viscous
    /// number.
    fn start_exchanger(&mut self, reynolds: f64) -> Result<(), String> {
        let d = self.design.ok_or("mesh first")?;
        let Shape::DoublePipe { r_tube, length, .. } = d.shape else { return Err("not a double pipe".into()) };
        let [a, b, c] = exchanger_meshes(&d)?;
        let mut spec = DoublePipeExchanger::demo();
        spec.length = m(length);
        let nu = spec.nu.get::<square_meter_per_second>();
        spec.flow_rate = uom::si::f64::VolumeRate::new::<uom::si::volume_rate::cubic_meter_per_second>(reynolds * nu / (2.0 * r_tube) * std::f64::consts::PI * r_tube * r_tube);
        let q = spec.flow_rate.get::<uom::si::volume_rate::cubic_meter_per_second>();
        let inlet_area = |mesh: &FvMesh| -> f64 {
            mesh.patches.iter().find(|p| p.name == "inlet").map_or(1.0, |p| (0..p.size).map(|i| mesh.face_areas[p.start + i]).sum())
        };
        let dz = m(self.cell_len);
        let dt = |mesh: &FvMesh| double_pipe::pseudo_time_step(mesh, Velocity::new::<meter_per_second>(q / inlet_area(mesh)), spec.nu, dz, Mode::Steady.courant(), max_viscous());
        let (dta, dtc) = (dt(&a), dt(&c));
        let offsets = [0, a.n_cells, a.n_cells + b.n_cells];
        let cht = spec.build(a, b, c, dta, dtc).map_err(|e| e.to_string())?;
        self.case = Some(Case::Exchanger(ExchangerRun { cht, spec, energy_phase: false, offsets, last_energy_change: f64::NAN }));
        self.steps = 0;
        self.time = 0.0;
        Ok(())
    }

    fn start(&mut self, reynolds: f64, mode: Mode) -> Result<(), String> {
        if matches!(self.design.map(|d| d.shape), Some(Shape::DoublePipe { .. })) {
            return self.start_exchanger(reynolds);
        }
        let fv = self.fv.clone().ok_or("mesh first")?;
        let flow = self.flow(reynolds);
        // Co ≈ 4 (or 1), but the viscous number ν Δt / h² held at
        // `MAX_VISCOUS_DEFAULT` or below, h the mean cell size (V/N)^(1/3);
        // `cases::double_pipe::pseudo_time_step` (gh:#1162: PimpleFoam
        // diverged at ν Δt / Δy² = 8 on a uniform channel). The smallest
        // spacing is the wrong length here: the O-grid's thin ring cells at
        // the square's corners (0.37 mm at 20k) would cap Co at ≈ 1.6, while
        // the uncapped Co ≈ 4 run is stable (gh:#1161 record: 34 steps) —
        // measured 2026-11-03, 79 steps with that cap.
        let dt = double_pipe::pseudo_time_step(&fv, flow.bulk_velocity, flow.nu, m(self.cell_len), mode.courant(), max_viscous());
        let mut s = flow.build_solver(fv, dt);
        // gh:#1161: the lagged correction alone (0 correctors) went unstable
        // at Co ≈ 4 on the 50k O-grid; one corrector is stable there.
        s.solution.pimple.n_non_orthogonal_correctors = 1;
        self.case = Some(Case::Pipe(s, flow));
        self.steps = 0;
        self.time = 0.0;
        Ok(())
    }

    /// Per-cell fields on the drawing mesh: |U|, p, axial U and T (T empty
    /// for the pipe), plus the run's diagnostics.
    fn progress(&self, change: f64, ms: f64) -> Option<Progress> {
        let a = self.flow_axis;
        let comp = |v: outram_foam_basic_lib::primitives::Vector3| match a {
            0 => v.x,
            1 => v.y,
            _ => v.z,
        };
        let n = self.fv.as_ref().map_or(0, |f| f.n_cells);
        let (u, p, t, continuity, out_in, dp, dt, exch, phase): (Vec<_>, Vec<f64>, Vec<f64>, f64, f64, f64, f64, [f64; EXCH_LEN], u8) = match self.case.as_ref()? {
            Case::Pipe(s, _) => {
                let mesh = &s.mesh;
                let pp = s.p.internal.as_slice();
                let mean_p = |name: &str| {
                    mesh.patches.iter().find(|q| q.name == name).map_or(0.0, |q| {
                        let (mut s1, mut w) = (0.0, 0.0);
                        for fi in 0..q.size {
                            let a = mesh.face_areas[q.start + fi];
                            s1 += pp[mesh.owner[q.start + fi]] * a;
                            w += a;
                        }
                        s1 / w.max(1e-300)
                    })
                };
                let inflow = -patch_flux(s, "inlet");
                (
                    s.u.internal.as_slice().to_vec(),
                    pp.to_vec(),
                    Vec::new(),
                    max_continuity_error(s),
                    patch_flux(s, "outlet") / inflow.max(1e-300),
                    mean_p("inlet") - mean_p("outlet"),
                    s.control.delta_t,
                    [f64::NAN; EXCH_LEN],
                    0,
                )
            }
            Case::Exchanger(x) => {
                let mut u = vec![outram_foam_basic_lib::primitives::Vector3::ZERO; n];
                let mut p = vec![0.0; n];
                let mut t = vec![f64::NAN; n];
                for (k, f) in x.cht.fluids.iter().enumerate() {
                    let o = x.offsets[2 * k];
                    for c in 0..f.mesh.n_cells {
                        u[o + c] = f.u.internal[c];
                        p[o + c] = f.p.internal[c];
                        t[o + c] = f.energy.as_ref().map_or(f64::NAN, |e| e.t.internal[c]);
                    }
                }
                let w = &x.cht.solids[0].energy.t;
                for c in 0..w.mesh.n_cells {
                    t[x.offsets[1] + c] = w.internal[c];
                }
                let f0 = &x.cht.fluids[0];
                let cont = max_continuity_error(f0).max(max_continuity_error(&x.cht.fluids[1]));
                let oi = |f: &PimpleFoam| patch_flux(f, "outlet") / (-patch_flux(f, "inlet")).max(1e-300);
                let exch = if x.energy_phase {
                    exchanger_numbers(&double_pipe::report(&x.cht, &x.spec), x.last_energy_change)
                } else {
                    [f64::NAN; EXCH_LEN]
                };
                (u, p, t, cont, oi(f0).min(oi(&x.cht.fluids[1])), f64::NAN, f0.control.delta_t, exch, u8::from(x.energy_phase))
            }
        };
        let tt = |c: usize| t.get(c).copied().unwrap_or(f64::NAN);
        Some(Progress {
            id: self.id,
            steps: self.steps,
            time_s: self.time,
            dt_s: dt,
            change,
            continuity,
            outlet_over_inlet: out_in,
            probe_u: u.get(self.probe_cell).map_or(0.0, |v| v.mag()),
            probe_p: p.get(self.probe_cell).copied().unwrap_or(0.0),
            dp,
            ms_per_step: ms,
            steady: match self.case.as_ref()? {
                Case::Pipe(..) => change < STEADY,
                Case::Exchanger(x) => x.energy_phase && x.last_energy_change < ENERGY_STEADY,
            },
            axial_u: pick(&self.axial, |c| u[c].mag()),
            axial_p: pick(&self.axial, |c| p[c]),
            cross_u: pick(&self.cross, |c| u[c].mag()),
            cross_p: pick(&self.cross, |c| p[c]),
            cross_ua: pick(&self.cross, |c| comp(u[c])),
            phase,
            axial_t: if t.is_empty() { Vec::new() } else { pick(&self.axial, tt) },
            cross_t: if t.is_empty() { Vec::new() } else { pick(&self.cross, tt) },
            exch,
        })
    }

    /// Advance the case by up to `steps` and return (steps done, change).
    /// The exchanger marches its flows to the steady criterion, then freezes
    /// them and iterates the energy, [`ENERGY_PASSES_PER_STEP`] passes per
    /// step, to [`ENERGY_STEADY`].
    fn advance(&mut self, steps: usize) -> Result<(usize, f64), String> {
        let diverged = || "the solve diverged; try a smaller Reynolds number or fewer cells".to_string();
        let mut done = 0;
        let mut change = f64::INFINITY;
        match self.case.as_mut().ok_or("no case")? {
            Case::Pipe(s, flow) => {
                for _ in 0..steps.max(1) {
                    change = flow.step(s).map_err(|_| diverged())?;
                    if !change.is_finite() {
                        return Err(diverged());
                    }
                    done += 1;
                    self.time += s.control.delta_t;
                    if change < STEADY {
                        break;
                    }
                }
            }
            Case::Exchanger(x) => {
                for _ in 0..steps.max(1) {
                    if x.energy_phase {
                        let r = x.cht.step().map_err(|e| e.to_string())?;
                        x.last_energy_change = r.last_pass_change / (T_HOT_IN - T_COLD_IN);
                        change = x.last_energy_change;
                        done += 1;
                        if change < ENERGY_STEADY {
                            break;
                        }
                    } else {
                        change = double_pipe::step_flows(&mut x.cht, x.spec.length).map_err(|_| diverged())?;
                        if !change.is_finite() {
                            return Err(diverged());
                        }
                        done += 1;
                        self.time += x.cht.fluids[0].control.delta_t;
                        if change < STEADY {
                            double_pipe::freeze_flows_for_steady_energy(&mut x.cht);
                            x.cht.energy_outer_correctors = ENERGY_PASSES_PER_STEP;
                            x.energy_phase = true;
                            break;
                        }
                    }
                }
            }
        }
        Ok((done, change))
    }

    /// Serve one request.
    pub fn serve(&mut self, req: Request, post: &mut impl FnMut(Event)) {
        match req {
            Request::Mesh { id, design, probe } => match self.mesh(id, design, probe) {
                Ok(me) => post(Event::Meshed(me)),
                Err(e) => post(Event::Error(format!("meshing failed: {e}"))),
            },
            Request::Start { id, reynolds, mode } if id == self.id => match self.start(reynolds, mode) {
                Ok(()) => {
                    if let Some(p) = self.progress(f64::INFINITY, 0.0) {
                        post(Event::Progress(p));
                    }
                }
                Err(e) => post(Event::Error(e)),
            },
            Request::Step { id, steps } if id == self.id => {
                if self.case.is_none() {
                    return;
                }
                let t0 = bugis::web_demo::platform::now_s();
                match self.advance(steps) {
                    Ok((done, change)) => {
                        self.steps += done;
                        let ms = (bugis::web_demo::platform::now_s() - t0) * 1e3 / done.max(1) as f64;
                        if let Some(p) = self.progress(change, ms) {
                            post(Event::Progress(p));
                        }
                    }
                    Err(e) => {
                        post(Event::Error(e));
                        self.case = None;
                    }
                }
            }
            Request::Probe { id, probe } if id == self.id => {
                let cross = self.place_probe(probe);
                post(Event::Cross { id, cross });
                if self.case.is_some() {
                    if let Some(p) = self.progress(f64::NAN, 0.0) {
                        post(Event::Progress(p));
                    }
                }
            }
            // A request from a design that has since been replaced.
            _ => {}
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl bugis::web_demo::link::NativeEngine for Engine {
    type Req = Request;
    type Ev = Event;
    fn handle(&mut self, req: Request, post: &mut impl FnMut(Event)) {
        self.serve(req, post);
    }
}

#[cfg(target_arch = "wasm32")]
impl bugis::web_demo::link::WorkerEngine for Engine {
    type Req = Request;
    type Ev = Event;
    fn handle(state: &std::sync::Arc<std::sync::RwLock<Self>>, req: Request, post: bugis::web_demo::link::Poster<Event>) {
        if let Ok(mut e) = state.write() {
            e.serve(req, &mut |ev| post.post(ev));
        }
    }
    fn error(message: String) -> Event {
        Event::Error(message)
    }
}

// ─── Wire format: numbers in one Float64Array, text in one string ────────────

fn put_section(d: &mut Vec<f64>, s: &Section) {
    d.push(f64::from(s.axis));
    d.push(s.offset);
    d.push(s.polygons.len() as f64);
    for p in &s.polygons {
        d.push(p.len() as f64);
        for q in p {
            d.push(f64::from(q[0]));
            d.push(f64::from(q[1]));
        }
    }
}

struct Reader<'a> {
    d: &'a [f64],
    i: usize,
}

impl Reader<'_> {
    fn f(&mut self) -> Result<f64, String> {
        let v = self.d.get(self.i).copied().ok_or_else(|| format!("message too short ({} numbers)", self.d.len()))?;
        self.i += 1;
        Ok(v)
    }
    fn n(&mut self) -> Result<usize, String> {
        let v = self.f()?;
        if v < 0.0 || v > 1e8 {
            return Err(format!("bad count {v}"));
        }
        Ok(v as usize)
    }
    fn f32s(&mut self) -> Result<Vec<f32>, String> {
        let n = self.n()?;
        (0..n).map(|_| self.f().map(|v| v as f32)).collect()
    }
    fn section(&mut self) -> Result<Section, String> {
        let axis = self.f()? as u8;
        let offset = self.f()?;
        let n = self.n()?;
        let mut polygons = Vec::with_capacity(n);
        for _ in 0..n {
            let k = self.n()?;
            let mut p = Vec::with_capacity(k);
            for _ in 0..k {
                p.push([self.f()? as f32, self.f()? as f32]);
            }
            polygons.push(p);
        }
        Ok(Section { axis, offset, polygons })
    }
}

fn put_f32s(d: &mut Vec<f64>, v: &[f32]) {
    d.push(v.len() as f64);
    d.extend(v.iter().map(|x| f64::from(*x)));
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl Request {
    pub fn to_wire(&self) -> Vec<f64> {
        match self {
            Request::Mesh { id, design, probe } => {
                let (k, a, b) = match design.shape {
                    Shape::Cylinder { radius, length, centre } => (0.0, [radius, length, 0.0, 0.0], centre),
                    Shape::Box { size, centre } => (1.0, [size[0], size[1], size[2], 0.0], centre),
                    Shape::DoublePipe { r_tube, wall, r_shell, length, centre } => (2.0, [r_tube, wall, r_shell, length], centre),
                };
                let mut d = vec![0.0, f64::from(*id), k, a[0], a[1], a[2], a[3], b[0], b[1], b[2], design.target_cells as f64, *probe];
                d.extend(design.tags.iter().map(|t| t.code()));
                d
            }
            Request::Start { id, reynolds, mode } => {
                vec![1.0, f64::from(*id), *reynolds, if *mode == Mode::Steady { 0.0 } else { 1.0 }]
            }
            Request::Step { id, steps } => vec![2.0, f64::from(*id), *steps as f64],
            Request::Probe { id, probe } => vec![3.0, f64::from(*id), *probe],
        }
    }
    pub fn from_wire(d: &[f64]) -> Result<Self, String> {
        let mut r = Reader { d, i: 0 };
        let kind = r.f()? as u32;
        let id = r.f()? as u32;
        match kind {
            0 => {
                let k = r.f()?;
                let a = [r.f()?, r.f()?, r.f()?, r.f()?];
                let centre = [r.f()?, r.f()?, r.f()?];
                let target_cells = r.n()?;
                let probe = r.f()?;
                let mut tags = [Tag::Wall; 6];
                for t in &mut tags {
                    *t = Tag::from_code(r.f()?);
                }
                let shape = match k as u8 {
                    0 => Shape::Cylinder { radius: a[0], length: a[1], centre },
                    1 => Shape::Box { size: [a[0], a[1], a[2]], centre },
                    _ => Shape::DoublePipe { r_tube: a[0], wall: a[1], r_shell: a[2], length: a[3], centre },
                };
                Ok(Request::Mesh { id, design: Design { shape, tags, target_cells }, probe })
            }
            1 => Ok(Request::Start { id, reynolds: r.f()?, mode: if r.f()? == 0.0 { Mode::Steady } else { Mode::Transient } }),
            2 => Ok(Request::Step { id, steps: r.n()? }),
            3 => Ok(Request::Probe { id, probe: r.f()? }),
            k => Err(format!("unknown request {k}")),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl Event {
    pub fn to_wire(&self) -> (Vec<f64>, String) {
        match self {
            Event::Meshed(me) => {
                let mut d = vec![0.0, f64::from(me.id), me.n_cells as f64, me.max_non_ortho_deg, f64::from(me.flow_axis), me.cell_len];
                put_f32s(&mut d, &me.edges);
                put_section(&mut d, &me.axial);
                put_section(&mut d, &me.cross);
                (d, String::new())
            }
            Event::Cross { id, cross } => {
                let mut d = vec![1.0, f64::from(*id)];
                put_section(&mut d, cross);
                (d, String::new())
            }
            Event::Progress(p) => {
                let mut d = vec![
                    2.0,
                    f64::from(p.id),
                    p.steps as f64,
                    p.time_s,
                    p.dt_s,
                    p.change,
                    p.continuity,
                    p.outlet_over_inlet,
                    p.probe_u,
                    p.probe_p,
                    p.dp,
                    p.ms_per_step,
                    f64::from(u8::from(p.steady)),
                ];
                for v in [&p.axial_u, &p.axial_p, &p.cross_u, &p.cross_p, &p.cross_ua] {
                    put_f32s(&mut d, v);
                }
                d.push(f64::from(p.phase));
                put_f32s(&mut d, &p.axial_t);
                put_f32s(&mut d, &p.cross_t);
                d.extend_from_slice(&p.exch);
                (d, String::new())
            }
            Event::Error(m) => (vec![3.0, 0.0], m.clone()),
        }
    }
    pub fn from_wire(d: &[f64], text: &str) -> Result<Self, String> {
        let mut r = Reader { d, i: 0 };
        let kind = r.f()? as u32;
        let id = r.f()? as u32;
        match kind {
            0 => Ok(Event::Meshed(Meshed {
                id,
                n_cells: r.n()?,
                max_non_ortho_deg: r.f()?,
                flow_axis: r.f()? as u8,
                cell_len: r.f()?,
                edges: r.f32s()?,
                axial: r.section()?,
                cross: r.section()?,
            })),
            1 => Ok(Event::Cross { id, cross: r.section()? }),
            2 => Ok(Event::Progress(Progress {
                id,
                steps: r.n()?,
                time_s: r.f()?,
                dt_s: r.f()?,
                change: r.f()?,
                continuity: r.f()?,
                outlet_over_inlet: r.f()?,
                probe_u: r.f()?,
                probe_p: r.f()?,
                dp: r.f()?,
                ms_per_step: r.f()?,
                steady: r.f()? != 0.0,
                axial_u: r.f32s()?,
                axial_p: r.f32s()?,
                cross_u: r.f32s()?,
                cross_p: r.f32s()?,
                cross_ua: r.f32s()?,
                phase: r.f()? as u8,
                axial_t: r.f32s()?,
                cross_t: r.f32s()?,
                exch: {
                    let mut x = [0.0; EXCH_LEN];
                    for v in &mut x {
                        *v = r.f()?;
                    }
                    x
                },
            })),
            3 => Ok(Event::Error(text.to_string())),
            k => Err(format!("bad event {k}")),
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::{Event, Request};
    use bugis::web_demo::link::{js, Message};
    use wasm_bindgen::JsValue;

    fn wrap(d: &[f64], text: &str) -> JsValue {
        let o = js::object();
        js::set(&o, "kind", "msg");
        js::set(&o, "data", js::f64s(d));
        js::set(&o, "text", text);
        o.into()
    }

    impl Message for Request {
        fn to_js(&self) -> JsValue {
            wrap(&self.to_wire(), "")
        }
        fn from_js(v: &JsValue) -> Result<Self, String> {
            Request::from_wire(&js::get_f64s(v, "data"))
        }
    }

    impl Message for Event {
        fn to_js(&self) -> JsValue {
            let (d, t) = self.to_wire();
            wrap(&d, &t)
        }
        fn from_js(v: &JsValue) -> Result<Self, String> {
            Event::from_wire(&js::get_f64s(v, "data"), &js::get_str(v, "text"))
        }
    }
}

/// The headless run (engine CLAUDE.md: headless mode is mandatory): mesh the
/// design, solve in `mode` to steady (or `max_steps`), and report one line per
/// chunk plus the pipe diagnostics. Deterministic apart from the timings.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub fn headless(design: Design, reynolds: f64, mode: Mode, max_steps: usize) -> Result<Vec<String>, String> {
    let mut e = Engine::default();
    let mut out = Vec::new();
    let mut last: Option<Event> = None;
    let t0 = bugis::web_demo::platform::now_s();
    e.serve(Request::Mesh { id: 1, design, probe: 0.75 }, &mut |ev| last = Some(ev));
    match last.take() {
        Some(Event::Meshed(me)) => out.push(format!(
            "# mesh: {} cells, max non-orthogonality {:.1} deg, cell length {:.3e} m, {:.3} s",
            me.n_cells,
            me.max_non_ortho_deg,
            me.cell_len,
            bugis::web_demo::platform::now_s() - t0
        )),
        Some(Event::Error(m)) => return Err(m),
        _ => return Err("no mesh".into()),
    }
    e.serve(Request::Start { id: 1, reynolds, mode }, &mut |ev| last = Some(ev));
    out.push("# steps  t[s]  change  continuity  out/in  dp[m2/s2]  ms/step  phase".into());
    let t1 = bugis::web_demo::platform::now_s();
    let mut last_progress = None;
    while e.steps < max_steps {
        e.serve(Request::Step { id: 1, steps: 5 }, &mut |ev| last = Some(ev));
        match &last {
            Some(Event::Progress(p)) => {
                out.push(format!(
                    "{} {:.3} {:.3e} {:.1e} {:.9} {:.6e} {:.1} {}",
                    p.steps, p.time_s, p.change, p.continuity, p.outlet_over_inlet, p.dp, p.ms_per_step, p.phase
                ));
                last_progress = Some(p.clone());
                if p.steady {
                    break;
                }
            }
            Some(Event::Error(m)) => return Err(m.clone()),
            _ => return Err("no progress".into()),
        }
    }
    let wall = bugis::web_demo::platform::now_s() - t1;
    if let (Some(p), Shape::DoublePipe { .. }) = (&last_progress, design.shape) {
        let x = &p.exch;
        out.push(format!(
            "# exchanger: Thot_out={:.4} Tcold_out={:.4} Qhot={:.6} Qcold={:.6} eps={:.5} eps_ntu={:.5} UA={:.5} NTU={:.4}",
            x[Exch::T_HOT_OUT],
            x[Exch::T_COLD_OUT],
            x[Exch::Q_HOT],
            x[Exch::Q_COLD],
            x[Exch::EPS],
            x[Exch::EPS_NTU],
            x[Exch::UA],
            x[Exch::NTU]
        ));
    }
    if let (Some(Case::Pipe(s, flow)), Shape::Cylinder { centre, length, .. }) = (&e.case, design.shape) {
        use outram_foam_appbuilder_lib::cases::laminar_pipe::{centreline_velocity, pressure_gradient};
        use uom::si::acceleration::meter_per_second_squared;
        let z0 = centre[2] - 0.5 * length;
        let uc = centreline_velocity(s, flow, (centre[0], centre[1]), z0 + 0.75 * length, 0.05 * length).get::<meter_per_second>();
        let g = pressure_gradient(s, z0 + 0.65 * length, z0 + 0.85 * length);
        out.push(format!(
            "# pipe (u_c/2U at 0.75 L, -dp/dz/HP over 0.65-0.85 L, f Re): uc={:.4} dpdz={:.4} fRe={:.2}",
            uc / (2.0 * U_IN),
            g.get::<meter_per_second_squared>() / flow.hagen_poiseuille_gradient().get::<meter_per_second_squared>(),
            flow.friction_factor(g) * flow.reynolds()
        ));
    }
    out.push(format!("# {} steps, {:.2} s wall in the solve", e.steps, wall));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small_pipe() -> Design {
        Design { target_cells: 2000, ..Design::default() }
    }

    #[test]
    fn requests_and_events_survive_the_wire() {
        let reqs = [
            Request::Mesh { id: 3, design: Design::default(), probe: 0.5 },
            Request::Mesh {
                id: 4,
                design: Design {
                    shape: Shape::Box { size: [0.1, 0.02, 0.03], centre: [0.0, 1.0, 2.0] },
                    tags: [Tag::Inlet, Tag::Outlet, Tag::Wall, Tag::Wall, Tag::Wall, Tag::Wall],
                    target_cells: 900,
                },
                probe: 0.25,
            },
            Request::Start { id: 3, reynolds: 50.0, mode: Mode::Transient },
            Request::Mesh { id: 5, design: Design::exchanger(), probe: 0.3 },
            Request::Step { id: 3, steps: 7 },
            Request::Probe { id: 3, probe: 0.9 },
        ];
        for r in reqs {
            assert_eq!(Request::from_wire(&r.to_wire()).unwrap(), r);
        }
        assert!(Request::from_wire(&[9.0, 1.0]).is_err());
        assert!(Request::from_wire(&[0.0]).is_err());
        let sec = Section { axis: 1, offset: 0.5, polygons: vec![vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]]] };
        let evs = [
            Event::Meshed(Meshed {
                id: 2,
                n_cells: 10,
                max_non_ortho_deg: 12.5,
                flow_axis: 2,
                cell_len: 0.01,
                edges: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 2.0],
                axial: sec.clone(),
                cross: sec.clone(),
            }),
            Event::Cross { id: 2, cross: sec },
            Event::Progress(Progress {
                id: 2,
                steps: 4,
                time_s: 1.5,
                dt_s: 0.5,
                change: 0.1,
                continuity: 1e-12,
                outlet_over_inlet: 1.0,
                probe_u: 0.01,
                probe_p: 2e-5,
                dp: 8e-5,
                ms_per_step: 12.0,
                steady: false,
                axial_u: vec![1.0, 2.0],
                axial_p: vec![3.0],
                cross_u: vec![],
                cross_p: vec![4.0],
                cross_ua: vec![5.0],
                phase: 1,
                axial_t: vec![300.0],
                cross_t: vec![310.0, 320.0],
                exch: [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
            }),
        ];
        for e in evs {
            let (d, t) = e.to_wire();
            assert_eq!(Event::from_wire(&d, &t).unwrap(), e);
        }
        let (d, t) = Event::Error("x".into()).to_wire();
        assert_eq!(Event::from_wire(&d, &t).unwrap(), Event::Error("x".into()));
    }

    #[test]
    fn designs_are_checked_and_sized() {
        assert!(check(&Design::default()).is_ok());
        let mut d = Design::default();
        d.tags[0] = Tag::Wall;
        assert!(check(&d).unwrap_err().contains("inlet"));
        d.tags[0] = Tag::Inlet;
        d.tags[1] = Tag::Wall;
        assert!(check(&d).unwrap_err().contains("outlet"));
        d = Design { target_cells: MAX_CELLS + 1, ..Design::default() };
        assert!(check(&d).is_err());
        d = Design { shape: Shape::Box { size: [0.1, 0.0, 0.1], centre: [0.0; 3] }, ..Design::default() };
        assert!(check(&d).is_err());
        // The default pipe is stage 0's 8/6/80 grid.
        let g = pipe_grid(0.01, 0.2, [0.0; 3], DEFAULT_CELLS);
        assert_eq!((g.n, g.nr, g.nz, g.n_cells()), (8, 6, 80, 20480));
        assert_eq!(pipe_grid(0.01, 0.2, [0.0; 3], MAX_CELLS).n_cells(), 42000);
        assert_eq!(pipe_grid(0.01, 0.2, [0.0; 3], 2_000).n_cells(), 1920);
        let b = box_block([0.1, 0.02, 0.02], [0.0; 3], 1000);
        assert!(b.n_cells() <= 1000 && b.n_cells() > 500, "{:?}", b.cells);
        assert!(Shape::Box { size: [1.0; 3], centre: [0.0; 3] }.face_names().len() == 6);
    }

    /// The headless regression (engine CLAUDE.md): the 1.9k-cell pipe at
    /// Re = 100 reaches the steady criterion, conserves mass, and its
    /// developed numbers sit where the V&V record puts this resolution.
    #[test]
    fn headless_pipe_reaches_steady_hagen_poiseuille() {
        let lines = headless(small_pipe(), 100.0, Mode::Steady, 400).unwrap();
        let joined = lines.join("\n");
        assert!(joined.contains("1920 cells"), "{joined}");
        let pipe = lines.iter().find(|l| l.starts_with("# pipe")).expect("pipe line");
        let val = |k: &str| -> f64 {
            pipe.split_whitespace().find_map(|t| t.strip_prefix(k)).and_then(|v| v.parse().ok()).expect(k)
        };
        let uc = val("uc=");
        let ratio = val("dpdz=");
        assert!((0.97..1.02).contains(&uc), "u_c/2U {uc}");
        assert!((0.99..1.06).contains(&ratio), "dp/dz / HP {ratio}");
        let last = lines.iter().rev().find(|l| !l.starts_with('#')).unwrap();
        let out_in: f64 = last.split_whitespace().nth(4).unwrap().parse().unwrap();
        assert!((out_in - 1.0).abs() < 1e-8, "{last}");
    }

    /// The headless exchanger regression (gh:#1156): a ~2.4k-cell double
    /// pipe at the demo's Re = 30 marches its flows, freezes them, iterates
    /// the energy to steady, closes energy between the streams, and its
    /// effectiveness sits within a few per cent of ε-NTU at the local UA
    /// (the coarse-grid gap of the V&V record, which shrinks as 1/nz).
    #[test]
    fn headless_exchanger_closes_energy_and_matches_ntu() {
        let mut d = Design::exchanger();
        d.target_cells = 2_500;
        assert!(check(&d).is_ok());
        let lines = headless(d, 30.0, Mode::Steady, 2_000).unwrap();
        let joined = lines.join("\n");
        let ex = lines.iter().find(|l| l.starts_with("# exchanger")).unwrap_or_else(|| panic!("{joined}"));
        let val = |k: &str| -> f64 { ex.split_whitespace().find_map(|t| t.strip_prefix(k)).and_then(|v| v.parse().ok()).expect(k) };
        let (qh, qc) = (val("Qhot="), val("Qcold="));
        assert!((qh / qc - 1.0).abs() < 1e-6, "{ex}");
        let (eps, ntu) = (val("eps="), val("eps_ntu="));
        assert!(eps > 0.3 && eps < 0.8, "{ex}");
        assert!((eps / ntu - 1.0).abs() < 0.05, "{ex}");
        assert!(val("Thot_out=") < T_HOT_IN && val("Tcold_out=") > T_COLD_IN, "{ex}");
        // Both phases ran, and the last report is steady.
        let last = lines.iter().rev().find(|l| !l.starts_with('#')).unwrap();
        assert!(last.ends_with(" 1"), "{last}");
        assert!(lines.iter().any(|l| !l.starts_with('#') && l.ends_with(" 0")), "{joined}");
    }

    #[test]
    fn exchanger_designs_are_checked_and_sized() {
        let d = Design::exchanger();
        let Shape::DoublePipe { r_tube, wall, r_shell, length, centre } = d.shape else { panic!() };
        let g = exchanger_grid(r_tube, wall, r_shell, length, centre, DEFAULT_CELLS);
        let n: usize = g.n_cells().iter().sum();
        assert!(n <= 21_000 && n > 12_000, "{n} {:?}", g.n_cells());
        assert_eq!(g.n_cells(), [(25 + 80) * 75, 40 * 75, 100 * 75]);
        let big: usize = exchanger_grid(r_tube, wall, r_shell, length, centre, MAX_CELLS).n_cells().iter().sum();
        assert!(big <= 52_500 && big > 30_000, "{big}");
        let mut bad = d;
        bad.shape = Shape::DoublePipe { r_tube: 0.005, wall: 0.006, r_shell: 0.01, length: 0.2, centre };
        assert!(check(&bad).is_err());
        assert!(exchanger_meshes(&Design::default()).is_err());
        assert_eq!(d.shape.n_faces(), 0);
        assert!(d.shape.face_names().is_empty());
        assert_eq!(d.shape.bounds()[1][0], 0.01);
        // The drawing mesh is the three regions side by side.
        let (poly, axis, dz) = mesh_design(&d).unwrap();
        assert_eq!(poly.n_cells, n);
        assert_eq!(axis, 2);
        assert!((dz - 0.2 / 75.0).abs() < 1e-15);
    }

    #[test]
    fn engine_serves_a_box_and_moves_the_probe() {
        let mut e = Engine::default();
        let mut evs = Vec::new();
        let d = Design {
            shape: Shape::Box { size: [0.02, 0.02, 0.1], centre: [0.0, 0.0, 0.05] },
            tags: [Tag::Wall, Tag::Wall, Tag::Wall, Tag::Wall, Tag::Inlet, Tag::Outlet],
            target_cells: 800,
        };
        e.serve(Request::Mesh { id: 5, design: d, probe: 0.5 }, &mut |ev| evs.push(ev));
        let Some(Event::Meshed(me)) = evs.pop() else { panic!("no mesh") };
        assert!(me.n_cells <= 800 && me.flow_axis == 2 && me.max_non_ortho_deg < 1e-3, "{} {} {}", me.n_cells, me.flow_axis, me.max_non_ortho_deg);
        assert!(!me.axial.polygons.is_empty() && !me.cross.polygons.is_empty());
        e.serve(Request::Start { id: 5, reynolds: 20.0, mode: Mode::Transient }, &mut |ev| evs.push(ev));
        e.serve(Request::Step { id: 5, steps: 3 }, &mut |ev| evs.push(ev));
        let Some(Event::Progress(p)) = evs.pop() else { panic!("no progress") };
        assert_eq!(p.steps, 3);
        assert_eq!(p.cross_u.len(), me.cross.polygons.len());
        assert!(p.dp > 0.0 && (p.outlet_over_inlet - 1.0).abs() < 1e-8);
        e.serve(Request::Probe { id: 5, probe: 0.9 }, &mut |ev| evs.push(ev));
        assert!(evs.iter().any(|ev| matches!(ev, Event::Cross { .. })));
        // A stale id is ignored.
        let n = evs.len();
        e.serve(Request::Step { id: 4, steps: 1 }, &mut |ev| evs.push(ev));
        assert_eq!(evs.len(), n);
        // A bad design reports an error.
        e.serve(Request::Mesh { id: 6, design: Design { target_cells: 0, ..Design::default() }, probe: 0.5 }, &mut |ev| evs.push(ev));
        assert!(matches!(evs.last(), Some(Event::Error(_))));
    }
}
