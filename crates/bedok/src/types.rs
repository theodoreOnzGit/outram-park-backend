//! The MATLAB structs — `params`, `geometry`, `th`, `constants`, `results`.
//!
//! # Why this module exists
//!
//! Like [`crate::matlab`], this has no `.m` counterpart. The reference passes
//! four loosely-typed structs through nearly every function signature, built up
//! field by field by the case files (`neacrpd1.m`, `iaea3ds.m`, …) and read
//! back with `isfield` guards. Rust has no equivalent of an open struct, so the
//! fields are collected here and the `isfield(params, 'x')` tests become
//! `Option::is_some`.
//!
//! # Growing this module
//!
//! The field set is **deliberately incomplete** and grows as modules are
//! ported. Only fields an already-translated `.m` file actually reads appear
//! here — inventing the rest up front would mean guessing at the reference,
//! which is exactly what the translation is supposed to avoid. Each field
//! records the `.m` file it was introduced by.

/// Geometry discretisation mode, as selected by which coordinate fields the
/// case file populated.
///
/// The reference expresses this as a chain of `isfield` tests in
/// `handle2dcoords.m` / `handle3dcoords.m` rather than as a value; this enum is
/// only a way of documenting the three cases the reference recognises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoordinateMode {
    /// `maxir` / `maxitheta` / `maxiz` — cylindrical.
    Cylindrical,
    /// `maxix` / `maxiy` / `maxiz` — Cartesian.
    Cartesian,
    /// `maxi1` / `maxi2` / `maxi3` — generic, already-resolved extents.
    Generic,
}

/// The `params` struct — run controls and discretisation extents.
///
/// Set up by the user block at the top of `main_exec_diff3d.m` and then
/// extended by whichever case file runs (`neacrpd1.m` and friends).
///
/// # Units
///
/// The reference carries no units. Extents are node counts; `tend` and `tgrid`
/// are seconds. Fields are documented individually where a unit applies.
#[derive(Clone, Debug, Default)]
pub struct Params {
    // --- discretisation extents (main_exec_diff3d.m) -----------------------
    /// Radial node count, cylindrical cases. `isfield(params,'maxir')`.
    pub maxir: Option<usize>,
    /// Azimuthal node count, cylindrical cases.
    pub maxitheta: Option<usize>,
    /// `x` node count, Cartesian cases.
    pub maxix: Option<usize>,
    /// `y` node count, Cartesian cases.
    pub maxiy: Option<usize>,
    /// `z` node count — shared by the cylindrical and Cartesian branches.
    pub maxiz: Option<usize>,
    /// Generic dim-1 extent, used when neither named branch applies.
    pub maxi1: Option<usize>,
    /// Generic dim-2 extent.
    pub maxi2: Option<usize>,
    /// Generic dim-3 extent.
    pub maxi3: Option<usize>,

    // --- physics sizes -----------------------------------------------------
    /// `G` — number of energy groups.
    pub g: usize,
    /// `Nc` — number of delayed-neutron precursor families.
    ///
    /// `convert_grid3d.m` guards this with `isfield` and substitutes `0`; the
    /// other readers assume it is present.
    pub nc: Option<usize>,

    // --- iteration controls (main_exec_diff3d.m) ---------------------------
    /// Outer power-iteration cycle cap.
    pub max_num_cycles: usize,
    /// Cycles per SA-nodal correction update; `0` selects the built-in default.
    ///
    /// Read by [`crate::sanodaldiffusion_solverxyz`], whose default is
    /// `ceil((maxix + maxiy + maxiz) / 10)`. **A value of `1` destabilises the
    /// solver** — see that module, and defect N1 in
    /// `docs/bedok-reference-defects.md`.
    pub nodalupd: usize,
    /// Source iterations between fission-source extrapolations; `0` selects the
    /// built-in default of 5.
    ///
    /// `isfield(params, 'fsexp')` in `sanodaldiffusion_solverxyz.m`, guarded
    /// the same `~= 0` way as [`Params::nodalupd`].
    pub fsexp: usize,
    /// Inexact inner convergence tolerance for the flux solve, dimensionless.
    ///
    /// Set by an outer coupling loop (`thdiffusion_solverxyz.m`) to avoid
    /// over-solving while the T-H feedback is still moving. `None` — and, per
    /// the reference's `params.innertol > 0` test, any non-positive value —
    /// selects the tight built-in `1e-6`. Read only by
    /// [`crate::sanodaldiffusion_solverxyz`]; [`crate::diffusion_solverxyz`]
    /// has no such switch and is always tight.
    pub innertol: Option<f64>,
    /// Force stop after this many cycles; `0` disables.
    pub stop: usize,
    /// Verbosity.
    pub verb: i32,
    /// Whether to produce figures.
    pub plotfig: i32,
    /// Whether to produce the 3-D power plot.
    pub plot3d: i32,
    /// Debug dump toggle.
    pub debugdump: i32,

    // --- transient controls (thdiffusion_solvertimexyz) --------------------
    /// End of transient, seconds. Set by the case file.
    pub tend: Option<f64>,
    /// Explicit time grid, seconds. Absent means uniform 10 ms steps over
    /// `0..tend`.
    pub tgrid: Option<Vec<f64>>,
    /// T-H feedback Picard passes per time step.
    pub timepicard: Option<usize>,
    /// SA-nodal correction update interval in steps; `0` freezes it.
    pub nodalupdtime: Option<usize>,

    // --- inert prototype switches ------------------------------------------
    /// JFNK preconditioner flag.
    ///
    /// **Read by nothing in this snapshot.** `main_exec_diff3d.m` sets it, but
    /// its only consumer — `driftflux_solverstatic1d.m` — is absent from the
    /// handover. Translated so the driver stays faithful; see
    /// `docs/bedok-reference-defects.md`.
    pub jfnkprecon: i32,
    /// JFNK relaxation factor. Inert, as [`Params::jfnkprecon`].
    pub jfnkrel: f64,
    /// JFNK verbosity. Inert, as [`Params::jfnkprecon`].
    pub jfnkverb: i32,
}

impl Params {
    /// `[maxi1, maxi2] = handle2dcoords(params)` — which coordinate branch the
    /// populated fields select.
    ///
    /// Returns `None` when no branch matches, which in the reference leaves the
    /// outputs undefined and raises `Output argument not assigned`.
    pub fn coordinate_mode_2d(&self) -> Option<CoordinateMode> {
        if self.maxir.is_some() && self.maxiz.is_some() {
            Some(CoordinateMode::Cylindrical)
        } else if self.maxix.is_some() && self.maxiy.is_some() {
            Some(CoordinateMode::Cartesian)
        } else if self.maxi1.is_some() && self.maxi2.is_some() {
            Some(CoordinateMode::Generic)
        } else {
            None
        }
    }

    /// `[maxi1, maxi2, maxi3] = handle3dcoords(params)` — which coordinate
    /// branch the populated fields select.
    pub fn coordinate_mode_3d(&self) -> Option<CoordinateMode> {
        if self.maxir.is_some() && self.maxitheta.is_some() && self.maxiz.is_some() {
            Some(CoordinateMode::Cylindrical)
        } else if self.maxix.is_some() && self.maxiy.is_some() && self.maxiz.is_some() {
            Some(CoordinateMode::Cartesian)
        } else if self.maxi1.is_some() && self.maxi2.is_some() && self.maxi3.is_some() {
            Some(CoordinateMode::Generic)
        } else {
            None
        }
    }

    /// `Nc`, defaulting to `0` when the field is absent.
    ///
    /// Only `convert_grid3d.m` guards the field this way; elsewhere the
    /// reference reads `params.Nc` directly.
    pub fn nc_or_zero(&self) -> usize {
        self.nc.unwrap_or(0)
    }
}

/// The `geometry` struct — physical extents and the per-column active-region
/// bounds computed by `geometry_ends3d.m`.
#[derive(Clone, Debug, Default)]
pub struct Geometry {
    /// Total `x` extent of the modelled quadrant. Units follow the case file.
    pub xtot: f64,
    /// Total `y` extent of the modelled quadrant.
    pub ytot: f64,

    /// `geometry.xlows(iy, iz)` — first `ix` with material present.
    pub xlows: Option<crate::matlab::Array2<usize>>,
    /// `geometry.xhis(iy, iz)` — last `ix` with material present.
    pub xhis: Option<crate::matlab::Array2<usize>>,
    /// `geometry.ylows(ix, iz)` — first `iy` with material present.
    pub ylows: Option<crate::matlab::Array2<usize>>,
    /// `geometry.yhis(ix, iz)` — last `iy` with material present.
    pub yhis: Option<crate::matlab::Array2<usize>>,
    /// `geometry.zlows(ix, iy)` — first `iz` with material present.
    pub zlows: Option<crate::matlab::Array2<usize>>,
    /// `geometry.zhis(ix, iy)` — last `iz` with material present.
    pub zhis: Option<crate::matlab::Array2<usize>>,

    /// `geometry.Lx` — node width in `x`, one entry per node.
    ///
    /// Length `maxix*maxiy*maxiz`, ordered `ix*maxiy*maxiz + iy*maxiz + iz`.
    /// The reference `repmat`s this to `G` groups at each use site rather than
    /// storing it per group. Units follow the case file, typically cm.
    pub lx: Vec<f64>,
    /// `geometry.Ly` — node width in `y`. As [`Geometry::lx`].
    pub ly: Vec<f64>,
    /// `geometry.Lz` — node height in `z`. As [`Geometry::lx`].
    pub lz: Vec<f64>,

    /// `geometry.Vi` — node volume, one entry per node.
    ///
    /// Length `maxix*maxiy*maxiz`, in the same `ix*maxiy*maxiz + iy*maxiz + iz`
    /// order as [`Geometry::lx`], and typically cm³ where the case file works
    /// in cm. The two flux solvers `repmat` it to `G` groups and multiply the
    /// converged fission source by it to get the power density.
    ///
    /// Note [`crate::makegrad_dxyz`] reads `geometry.Vi` and never uses it —
    /// that is dead code in the reference and is not why this field exists.
    pub vi: Vec<f64>,

    /// `geometry.xmin` — boundary condition on the low-`x` face.
    pub xmin: BoundaryCondition,
    /// `geometry.xmax` — boundary condition on the high-`x` face.
    pub xmax: BoundaryCondition,
    /// `geometry.ymin` — boundary condition on the low-`y` face.
    pub ymin: BoundaryCondition,
    /// `geometry.ymax` — boundary condition on the high-`y` face.
    pub ymax: BoundaryCondition,
    /// `geometry.zmin` — boundary condition on the low-`z` face.
    pub zmin: BoundaryCondition,
    /// `geometry.zmax` — boundary condition on the high-`z` face.
    pub zmax: BoundaryCondition,

    /// `geometry.adf` — assembly discontinuity factors, `philen` by **6**.
    ///
    /// Same `(minus, plus)` per-axis column layout as `gradterms`: `0, 1` for
    /// `x`, `2, 3` for `y`, `4, 5` for `z`. Dimensionless; `1` everywhere means
    /// no discontinuity.
    ///
    /// The reference guards this with `isfield` and substitutes
    /// `ones(philen, 6)` when absent, which `None` reproduces.
    pub adf: Option<crate::matlab::Array2<f64>>,
}

/// A temperature-dependent thermal conductivity, W/(cm·K).
///
/// # Why this is an enum rather than a function pointer
///
/// The reference carries these as a **cell array of anonymous function
/// handles**, `geometry.fuel.tcon{m}`, built by each case file and invoked as
/// `tcon{whichk(i)}(T)`. The set of correlations the snapshot actually ships is
/// closed — two of them, both in `neacrpd1.m` and `neacrpa2.m` — so an enum
/// gives exhaustive dispatch and keeps the workspace's no-trait-objects rule.
/// A new correlation is a new variant and a compile error at every `match`.
///
/// # The cell array is heterogeneous, and that is not reproduced
///
/// `tcon` is sized `max(whichk) + 1`, and its **last** element is not a
/// function at all: it is a bare scalar gap conductance, used as
/// `tcon{end} * <length>` and never called. `whichk` only ever takes values
/// `0`, `1`, `2` — so the last slot is unreachable by the indexed lookup and
/// exists purely to be read as `tcon{end}`.
///
/// Conflating a W/(cm·K) conductivity with a W/(cm²·K) conductance in one
/// container is the reference's own doing. Here they are split: the
/// correlations live in [`FuelGeometry::tcon`] and the gap conductance in
/// [`FuelGeometry::gap_conductance`], which have different units and different
/// meanings. This is a type-level restructuring in the same spirit as
/// [`BoundaryCondition`] replacing the reference's strings; it changes no
/// behaviour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Conductivity {
    /// UO2 fuel: `(1.05 + 2150/(T - 73.15)) / 100`, W/(cm·K), `T` in K.
    ///
    /// From `neacrpd1.m` and `neacrpa2.m`. **Singular at `T = 73.15 K`** and
    /// negative below it; the reference does not guard this and neither does
    /// the evaluation here. Fuel temperatures are hundreds of K above it.
    Uo2Fuel,
    /// Zircaloy cladding:
    /// `(7.51 + 2.09e-2 T - 1.45e-5 T^2 + 7.67e-9 T^3) / 100`, W/(cm·K).
    ///
    /// From `neacrpd1.m` and `neacrpa2.m`.
    ZircaloyClad,
    /// A temperature-independent conductivity, W/(cm·K).
    ///
    /// Not used by any case file in the snapshot; provided so a caller can
    /// supply a constant-property material without inventing a correlation.
    Constant(f64),
}

impl Conductivity {
    /// Evaluate at temperature `t` in **K**, returning W/(cm·K).
    ///
    /// The `/100` in both correlations converts W/(m·K) to W/(cm·K); the
    /// reference writes it inline and the comment on each line confirms the
    /// target unit.
    pub fn at(&self, t: f64) -> f64 {
        match self {
            Self::Uo2Fuel => (1.05 + 2150.0 / (t - 73.15)) / 100.0,
            Self::ZircaloyClad => {
                (7.51 + 2.09e-2 * t - 1.45e-5 * t * t + 7.67e-9 * t * t * t) / 100.0
            }
            Self::Constant(k) => *k,
        }
    }
}

/// `params.fuel` — the fuel-rod radial mesh sizes.
///
/// The reference passes this sub-struct where a function's signature says
/// `params`, so `makeheatlaplacian_1dcylnd(params.fuel, geometry.fuel, ...)`
/// reads `params.maxir` and means `params.fuel.maxir`.
#[derive(Clone, Copy, Debug, Default)]
pub struct FuelParams {
    /// `params.fuel.maxir` — total radial node count, fuel + gap + cladding.
    pub maxir: usize,
    /// `params.fuel.fueln` — radial nodes inside the fuel pellet.
    pub fueln: usize,
    /// `params.fuel.gapn` — radial nodes across the fuel-cladding gap.
    pub gapn: usize,
    /// `params.fuel.cladn` — radial nodes through the cladding.
    pub cladn: usize,
}

/// `geometry.fuel` — the 1-D cylindrical fuel-rod discretisation.
///
/// One radial mesh, shared by every axial node of every channel: the rod
/// geometry does not vary across the core in any case the snapshot ships. The
/// per-node quantities that *do* vary (power, coolant temperature) are passed
/// to the conduction solver as scalars.
///
/// # Units
///
/// Lengths cm, areas cm², volumes cm³ — the whole reference works in cm.
#[derive(Clone, Debug, Default)]
pub struct FuelGeometry {
    /// `geometry.fuel.Lr(ir)` — radial node thickness, cm. Length `maxir`.
    pub lr: Vec<f64>,
    /// `geometry.fuel.Ctr(ir)` — radius of each node **centre**, cm:
    /// `sum(Lr(1:ir)) - 0.5*Lr(ir)`.
    pub ctr: Vec<f64>,
    /// `geometry.fuel.Vi(ir)` — node volume per unit length, cm³/cm.
    ///
    /// The innermost is `pi*Lr(1)^2`; the rest are annular shells.
    pub vi: Vec<f64>,
    /// `geometry.fuel.whichk(ir)` — which material occupies node `ir`.
    ///
    /// **`0` means the gap**, `1` the fuel, `2` the cladding. A non-zero value
    /// `m` selects `tcon[m - 1]`; `0` selects [`FuelGeometry::gap_conductance`]
    /// instead, and marks a node the conduction stencil bridges rather than
    /// solves through.
    pub whichk: Vec<usize>,
    /// The per-material conductivity correlations, indexed by `whichk - 1`.
    ///
    /// See [`Conductivity`] for why this is not the reference's cell array.
    pub tcon: Vec<Conductivity>,
    /// The fuel-cladding **gap conductance**, W/(cm²·K) — `tcon{end}`.
    ///
    /// `0.35` in `neacrpd1.m`, attributed there to the NEACRP benchmark. Note
    /// the units differ from [`FuelGeometry::tcon`]'s: this is a conductance
    /// across a gap of unresolved width, not a conductivity.
    pub gap_conductance: f64,
    /// `geometry.fuel.fuelrad` — the fuel pellet radius, cm.
    pub fuelrad: f64,
    /// `geometry.fuel.Rtot` — the outer cladding radius, cm.
    pub rtot: f64,
    /// `geometry.fuel.pitch` — the lattice pitch, cm.
    pub pitch: f64,
    /// `geometry.fuel.subarea` — coolant flow area per pin, cm².
    ///
    /// `th_solverxyz.m` recomputes this as `pitch^2 - pi*Rtot^2` rather than
    /// reading the field, so the two can disagree; `w3chf.m` reads the field.
    pub subarea: f64,
    /// `geometry.fuel.hydia` — subchannel hydraulic diameter, cm.
    ///
    /// As [`FuelGeometry::subarea`], `th_solverxyz.m` recomputes rather than
    /// reads.
    pub hydia: f64,
    /// `geometry.fuel.doppleralpha` — the weight on the pellet-surface
    /// temperature in the Doppler average, dimensionless on `[0, 1]`.
    ///
    /// `Tdoppler = (1 - alpha)*T_centre + alpha*T_surface`.
    pub doppleralpha: f64,
}

/// A quantity carried per axis, one `philen` vector each.
///
/// The reference builds several of these as bare structs with `.x`, `.y` and
/// `.z` fields — `A2` and `A4` from the nodal expansion among them. Structurally
/// identical to [`crate::calc_transleakagexyz::Leakage`]; kept separate because
/// the two mean different things and mixing them up would type-check.
#[derive(Clone, Debug, Default)]
pub struct AxisField {
    /// The `x` component.
    pub x: Vec<f64>,
    /// The `y` component.
    pub y: Vec<f64>,
    /// The `z` component.
    pub z: Vec<f64>,
}

/// Outer boundary condition on one face of the core.
///
/// The reference carries these as the strings `'vacuum'`, `'zeroflux'` and
/// `'reflective'`, dispatched on with `switch`.
///
/// # `Vacuum` and `ZeroFlux` are not distinguished
///
/// Every `switch` in the translated code groups them — `case {'vacuum',
/// 'zeroflux'}` — so they produce identical coefficients. They are kept as
/// separate variants because the case files set them separately and the
/// distinction may matter to code not yet translated.
///
/// # An unrecognised string silently gives zero
///
/// The reference's `switch` statements have no `otherwise` branch, so a
/// boundary condition that is none of the three leaves the preallocated `0` in
/// place — a silently absent boundary term rather than an error. The enum makes
/// that unrepresentable, which narrows the input domain rather than changing
/// behaviour for any valid input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BoundaryCondition {
    /// `'vacuum'` — no incoming current.
    #[default]
    Vacuum,
    /// `'zeroflux'` — flux forced to zero at the face. Treated identically to
    /// [`BoundaryCondition::Vacuum`] everywhere in the translated code.
    ZeroFlux,
    /// `'reflective'` — zero net current, a symmetry plane.
    Reflective,
}

/// `th.coolant` — the coolant thermodynamic state, one entry per core node.
///
/// Every vector is `maxix*maxiy*maxiz` long in the usual
/// `ix*maxiy*maxiz + iy*maxiz + iz` order, except the two inlet scalars.
///
/// # Units — cm-g-s, not SI
///
/// The reference works in centimetres and grams throughout, and mixes in MPa
/// for pressure and kJ/kg for enthalpy because that is what its IAPWS
/// implementation returns. Each field states its own unit; the ones that catch
/// people out are density in **g/cm³** (not kg/m³) and velocity in **cm/s**.
///
/// # Growing this struct
///
/// As [`Params`], the field set is deliberately incomplete and grows as the
/// thermal-hydraulics modules are ported. Only fields a translated `.m` file
/// actually reads appear here.
#[derive(Clone, Debug, Default)]
pub struct Coolant {
    /// `th.coolant.inlettemp` — inlet temperature, **K**. Scalar.
    pub inlettemp: f64,
    /// `th.coolant.inletpress` — inlet pressure, **MPa**. Scalar.
    pub inletpress: f64,
    /// `th.coolant.press` — pressure per node, **MPa**.
    pub press: Vec<f64>,
    /// `th.coolant.temps` — bulk temperature per node, **K**.
    pub temps: Vec<f64>,
    /// `th.coolant.enth` — bulk specific enthalpy per node, **kJ/kg**.
    pub enth: Vec<f64>,
    /// `th.coolant.quality` — thermodynamic equilibrium quality, mass
    /// fraction. Negative in subcooled liquid, which the W-3 correlation
    /// relies on.
    pub quality: Vec<f64>,
    /// `th.coolant.alphag` — void fraction, dimensionless on `[0, 1]`.
    pub alphag: Vec<f64>,
    /// `th.coolant.vm` — mixture velocity, **cm/s**.
    pub vm: Vec<f64>,
    /// `th.coolant.ldens` — saturated **liquid** density, g/cm³.
    pub ldens: Vec<f64>,
    /// `th.coolant.gdens` — saturated **vapour** density, g/cm³.
    pub gdens: Vec<f64>,
    /// `th.coolant.dens` — mixture density, g/cm³.
    pub dens: Vec<f64>,
    /// `th.coolant.kvis` — kinematic viscosity, cm²/s.
    pub kvis: Vec<f64>,
    /// `th.coolant.pran` — Prandtl number, dimensionless.
    pub pran: Vec<f64>,
    /// `th.coolant.tcon` — coolant thermal conductivity, W/(cm·K).
    ///
    /// Distinct from [`FuelGeometry::tcon`], which is a set of correlations for
    /// solid materials; this is an already-evaluated per-node value.
    pub tcon: Vec<f64>,
}

/// The `th` struct — the thermal-hydraulic state passed through the coupling.
///
/// # Growing this struct
///
/// As [`Coolant`], deliberately incomplete.
#[derive(Clone, Debug, Default)]
pub struct Th {
    /// The coolant state.
    pub coolant: Coolant,
    /// `th.heatflux` — wall heat flux per node, **W/cm²**.
    pub heatflux: Vec<f64>,
}

/// The `sigmavalues` struct — per-**material** cross-section data, as read
/// from the benchmark case files.
///
/// This is the *input* to [`crate::makesigmadfxyz::makesigmadfxyz`], which
/// expands it onto the spatial mesh to produce [`Sigma`]. Material rows are
/// 0-based here; the identifiers stored in `whichsigma` are 1-based with `0`
/// for void, so a node holding material `m` reads row `m - 1`.
#[derive(Clone, Debug, Default)]
pub struct SigmaValues {
    /// `sigmavalues.tot(material, g)` — total cross section, cm<sup>-1</sup>.
    pub tot: crate::matlab::Array2<f64>,
    /// `sigmavalues.f(material, g)` — fission cross section, cm<sup>-1</sup>.
    pub f: crate::matlab::Array2<f64>,
    /// `sigmavalues.s(material, gt, g)` — scattering from group `g` **into**
    /// group `gt`. Note the destination index comes first.
    pub s: crate::matlab::Array3<f64>,
    /// `sigmavalues.nu(material, g)` — neutrons per fission.
    ///
    /// The reference accepts a scalar here and expands it; see
    /// [`crate::makesigmadfxyz::makesigmadfxyz`] for how, and for the
    /// inconsistent indexing that follows.
    pub nu: crate::matlab::Array2<f64>,
    /// `sigmavalues.chi(material, gt)` — fission spectrum, the fraction of
    /// fission neutrons born into group `gt`. Dimensionless, sums to 1 over
    /// `gt`.
    pub chi: crate::matlab::Array2<f64>,
    /// `sigmavalues.fp(material, g)` — prompt fission cross section.
    ///
    /// Optional in the reference, which substitutes zeros when the field is
    /// absent. `None` reproduces that.
    pub fp: Option<crate::matlab::Array2<f64>>,
}

/// The `sigma` struct — the multigroup cross-section **operators**, expanded
/// onto the spatial mesh.
///
/// Each matrix is `philenf` square over the flattened `(group, node)` index
/// space, so a single matrix carries both the within-group and the
/// group-to-group coupling. Produced by
/// [`crate::makesigmadfxyz::makesigmadfxyz`].
#[derive(Clone, Debug, Default)]
pub struct Sigma {
    /// `sigma.tot` — total cross section, diagonal. Units cm<sup>-1</sup>.
    pub tot: crate::matlab::SparseMatrix,
    /// `sigma.s` — scattering, including the group-to-group off-diagonals.
    pub s: crate::matlab::SparseMatrix,
    /// `sigma.f` — fission production `chi * nu * Sigma_f`, divided by `keff`
    /// where it enters the buckling.
    pub f: crate::matlab::SparseMatrix,
    /// `sigma.fp` — the prompt part of `sigma.f`, built as `chi * Sigma_fp`.
    ///
    /// Note this carries **no** `nu` factor, where [`Sigma::f`] does.
    pub fp: crate::matlab::SparseMatrix,
    /// `sigma.fb` — bare fission cross section on the diagonal, without the
    /// `chi` or `nu` factors.
    pub fb: crate::matlab::SparseMatrix,
    /// `sigma.sd` — the within-group scattering `Sigma_s(g -> g)` on the
    /// diagonal only.
    pub sd: crate::matlab::SparseMatrix,
    /// `sigma.nu` — neutrons per fission, one entry per `(group, node)`.
    pub nu: Vec<f64>,
    /// `sigma.chi` — fission spectrum, `G` rows by `philen` columns.
    pub chi: crate::matlab::Array2<f64>,
}
