//! Code-to-code verification: GeN-Foam vs OFFBEAT small-strain mechanics,
//! on a mesh written by this crate's own `blockMesh` CLI tool (bead `op-cdte`).
//!
//! # Why these two codes are a meaningful pair
//!
//! Two crates in this workspace independently port a segregated
//! *compact-normal-stress* small-strain linear-elastic displacement solve:
//!
//! | | crate | type | upstream |
//! |---|---|---|---|
//! | GeN-Foam | `outram-foam-appbuilder-lib` | [`MechanicsMeshSolver`] | GeN-Foam `legacyThermoMechanics` / OpenFOAM `solidDisplacementFoam` |
//! | OFFBEAT | `outram-park-fork-offbeat` | [`MechanicsSolver`] | OFFBEAT `offbeatLib/.../smallStrain.C` |
//!
//! They were translated from *different* upstream codebases by different
//! routes, and they agree on the discretisation only because both upstreams
//! inherit it from OpenFOAM. They also agree on the elastic-constant algebra
//! (GeN-Foam's `RheologyOption::PlaneStrain` is the only form OFFBEAT has:
//! `λ = Eν/((1+ν)(1−2ν))`, `3K = E/(1−2ν)`) and on the stress convention
//! (`σ = 2με + λ tr(ε) I − 3K ε* I`). The thermal load enters GeN-Foam as
//! `∇(3K α ΔT)` and OFFBEAT as `∇(3K ε*)`; setting `ε* = α ΔT` makes the two
//! problems the *same* problem.
//!
//! **What this verifies and what it does not.** Both ports assemble onto the
//! same `outram_foam_basic_lib` `fvm`/`fvc` operator layer, so that layer
//! cancels out of the comparison: a bug shared by `fvm::laplacian_vec` or
//! `fvc::div_tensor` would move both answers together and go unseen here.
//! What the comparison *does* test is everything the two ports do
//! independently — constant algebra, load construction, the segregated split,
//! the corrector loop, the stress recovery — plus, in
//! [`the_two_codes_disagree_on_the_thermal_load_at_a_dirichlet_boundary`], a
//! real difference in boundary treatment. Neither test is validation against
//! experiment.
//!
//! # The meshing chain
//!
//! `tests/cases/thermo_bar/system/blockMeshDict` → the **`blockMesh` CLI tool**
//! (`src/bin/block_mesh.rs`, driven through its own `run`, exactly as
//! `mesh_tools.rs` does) → `constant/polyMesh` on disk →
//! [`read_poly_mesh`] → `Arc<FvMesh>` → **both** solvers. Nothing here builds
//! an `FvMesh` by hand; the mesh both codes see is the one the CLI wrote.
//!
//! # Results, in one line each (measured 2026-09-15)
//!
//! Common problem: a 1 m bar, 0.05 x 0.05 m section, `E = 200 GPa`, `ν = 0`,
//! `α = 1.2e-5 /K`, both axial ends clamped, `ΔT(x) = 500 sin²(πx/L)` K about
//! `T_ref = 300 K`. Full methodology and numbers in each test's doc comment.
//!
//! 1. [`genfoam_and_offbeat_agree_on_the_clamped_thermoelastic_bar`] — the two
//!    ports differ by **2.175e-19 m** in displacement (relative 4.6e-16) and
//!    **4.768e-7 Pa** in axial stress (relative 8e-16) on 20 cells. Both miss
//!    the closed form `D(x) = −(αAL/4π) sin(2πx/L)` by the same 0.814 %.
//! 2. [`the_two_codes_disagree_on_the_thermal_load_at_a_dirichlet_boundary`] —
//!    with a `FixedValue` temperature end the same two ports differ by
//!    **8.772e-7 m** (0.18 % of amplitude) and **3.324e6 Pa**, localised at the
//!    boundary cells. GeN-Foam maps the Dirichlet value into the load; OFFBEAT
//!    cannot, and extrapolates. GeN-Foam is the consistent one. Tracked as
//!    `op-cdte`.
//! 3. [`the_agreement_and_the_convergence_order_survive_refinement`] — at 10,
//!    20, 40 and 80 cells the agreement stays at round-off (`~1e-19 m`) while
//!    the error against the closed form falls at observed order
//!    **2.00 ± 0.03**.
//!
//! 4. [`the_two_codes_agree_through_a_transient_elastic_response`] — stepping a
//!    step-loaded bar 50 x 2 µs to half an axial transit, with the inertial
//!    term and the displacement history live, the two ports differ by
//!    **2.521e-18 m** while sitting 88.67 % of an amplitude away from the
//!    quasi-static answer.
//!
//! None of this is validation against experiment, and none of it exercises the
//! shared operator layer — see the caveat above.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use outram_foam_appbuilder_lib::genfoam::thermo_mechanics::{
    ElasticMaterial, LegacyThermoMechanics, MechanicsMeshSolver, RheologyOption,
    ThermalExpansionCoeff, YoungsModulus,
};
use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_basic_lib::fields::boundary::bc::PatchField;
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::fields::{VolScalarField, VolVectorField};
use outram_foam_basic_lib::ldu_matrix::SolverSettings;
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;
use outram_park_fork_offbeat::mechanics::{LinearElastic, MechanicsSolver};
use outram_foam_cli::CaseArgs;

use uom::si::f64::{
    MassDensity, Ratio, SpecificHeatCapacity, ThermalConductivity, ThermodynamicTemperature, Time,
};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::gigapascal;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

#[allow(dead_code)]
#[path = "../src/bin/block_mesh.rs"]
mod block_mesh;

// ── The common problem ──────────────────────────────────────────────────────

/// Young's modulus `E` (Pa).
const YOUNG: f64 = 200.0e9;
/// Poisson's ratio `ν` (-). Zero decouples the transverse directions, so the
/// bar reduces to the textbook 1-D thermo-elastic problem with `2μ+λ = 3K = E`
/// and an identically-zero explicit `divSigmaExp`.
const POISSON: f64 = 0.0;
/// Linear thermal-expansion coefficient `α` (1/K).
const ALPHA: f64 = 1.2e-5;
/// Stress-free reference temperature `T_ref` (K).
const T_REF: f64 = 300.0;
/// Peak temperature rise `A` (K) of the `ΔT = A sin²(πx/L)` profile.
const DELTA_T_PEAK: f64 = 500.0;
/// Bar length `L` (m) — must match the `blockMeshDict` fixture.
const LENGTH: f64 = 1.0;
/// Mass density `ρ` (kg/m³). Only the transient (inertial) form reads it, and
/// the two solvers get it by different routes — GeN-Foam from the material
/// card, OFFBEAT from `set_density` — so it is named once here.
const DENSITY: f64 = 7850.0;

/// The imposed temperature rise above `T_ref` at axial position `x` (m).
///
/// `ΔT(x) = A sin²(πx/L)`, chosen because it vanishes *and has zero slope* at
/// both ends, which keeps the first-order boundary treatment of the load field
/// from dominating the discretisation error.
fn delta_t(x: f64) -> f64 {
    let s = (std::f64::consts::PI * x / LENGTH).sin();
    DELTA_T_PEAK * s * s
}

/// Analytical displacement `D_x(x)` (m) for the clamped bar under `ΔT(x)`.
///
/// With `ν = 0` the governing equation is `E D'' = (E α ΔT)'`, so
/// `D' = α ΔT + C`; imposing `D(0) = D(L) = 0` on `∫ΔT` gives
/// `C = −αA/2` and the closed form
///
/// ```text
///   D(x) = −(α A L / 4π) · sin(2πx/L)
/// ```
fn analytic_displacement(x: f64) -> f64 {
    -(ALPHA * DELTA_T_PEAK * LENGTH / (4.0 * std::f64::consts::PI))
        * (2.0 * std::f64::consts::PI * x / LENGTH).sin()
}

/// Analytical axial stress (Pa), which for this profile is **uniform**:
/// `σ_xx = E(D' − α ΔT) = E(−αA/2·cos − αA/2·(1−cos)) = −E α A / 2`.
fn analytic_axial_stress() -> f64 {
    -YOUNG * ALPHA * DELTA_T_PEAK / 2.0
}

/// Linear-solver settings shared by both codes, so neither is advantaged.
///
/// The elastic Laplacian is stiff and the vector solve is Gauss-Seidel; a loose
/// tolerance shows up as a stiffer-looking material, not as an obvious failure.
fn settings() -> SolverSettings {
    SolverSettings {
        tolerance: 1.0e-14,
        max_iter: 200_000,
    }
}

// ── The meshing chain ───────────────────────────────────────────────────────

/// The axial resolution written into the committed `thermo_bar` fixture.
const FIXTURE_CELLS: &str = "(20 1 1)";

/// Run the `blockMesh` **CLI tool** on the `thermo_bar` fixture at `n` axial
/// cells, in a fresh temporary case named after `tag`, then read the
/// `constant/polyMesh` it wrote back as an `FvMesh`. Both solvers get this one
/// mesh.
///
/// The committed fixture is the single source of truth for the geometry and the
/// patch names; only its block resolution is rewritten, so a refinement study
/// cannot drift from the case it refines. The substitution is asserted, not
/// assumed — an edit to the fixture that changes that token fails here rather
/// than silently meshing at one resolution throughout.
fn bar_mesh_via_block_mesh_cli(tag: &str, n: usize) -> Arc<FvMesh> {
    let dict_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases/thermo_bar/system/blockMeshDict");
    let dict = fs::read_to_string(&dict_path).expect("read the thermo_bar blockMeshDict");
    assert!(
        dict.contains(FIXTURE_CELLS),
        "the thermo_bar fixture no longer contains the `{FIXTURE_CELLS}` block \
         resolution this test rewrites"
    );
    let dict = dict.replace(FIXTURE_CELLS, &format!("({n} 1 1)"));

    // Tagged per test: the harness runs tests in parallel and each one writes
    // its own `constant/polyMesh`, so a shared case directory would race.
    let case: PathBuf =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("thermo_bar_c2c_{tag}_{n}"));
    let _ = fs::remove_dir_all(&case);
    fs::create_dir_all(case.join("system")).unwrap();
    fs::write(case.join("system/blockMeshDict"), dict).unwrap();

    block_mesh::run(&CaseArgs { case: case.clone() }).expect("blockMesh CLI run");

    let poly = case.join("constant/polyMesh");
    assert!(
        poly.join("boundary").is_file(),
        "blockMesh wrote no boundary"
    );
    let mesh = read_poly_mesh(&poly).expect("read the polyMesh blockMesh wrote");
    assert_eq!(mesh.n_cells, n, "blockMesh wrote the wrong cell count");
    mesh
}

/// Index of the `xMin` / `xMax` patches in the mesh's patch order.
///
/// Resolved by name rather than assumed, so a change to the fixture's patch
/// ordering fails loudly here instead of silently clamping the wrong faces.
fn axial_patches(mesh: &FvMesh) -> (usize, usize) {
    let find = |name: &str| {
        mesh.patches
            .iter()
            .position(|p| p.name == name)
            .unwrap_or_else(|| panic!("mesh has no `{name}` patch"))
    };
    (find("xMin"), find("xMax"))
}

/// Displacement boundary conditions: both axial ends clamped to zero, the four
/// lateral faces zero-gradient.
fn clamped_ends(mesh: &FvMesh) -> Vec<PatchField<Vector3>> {
    let (x_min, x_max) = axial_patches(mesh);
    mesh.patches
        .iter()
        .enumerate()
        .map(|(i, p)| {
            if i == x_min || i == x_max {
                PatchField::fixed_value_vec(p.size, Vector3::ZERO)
            } else {
                PatchField::zero_gradient_vec(p.size)
            }
        })
        .collect()
}

/// The per-cell temperature rise `ΔT` evaluated at each cell centre.
fn cell_delta_t(mesh: &FvMesh) -> Vec<f64> {
    (0..mesh.n_cells)
        .map(|c| delta_t(mesh.cell_centres[c].x))
        .collect()
}

/// Build the GeN-Foam solver on `mesh`, with the temperature field's patches
/// set by `axial_bc`.
///
/// `axial_bc = None` leaves every patch zero-gradient, which is what makes the
/// GeN-Foam thermal load field byte-for-byte the same field OFFBEAT builds.
/// `axial_bc = Some((t_min, t_max))` puts `FixedValue` on the two axial ends,
/// which is what a real conduction solve leaves behind — and which OFFBEAT
/// cannot currently express.
///
/// `dt` (s) is read only by the transient form; the quasi-static solve ignores
/// it, so the quasi-static callers pass 1.0.
fn genfoam_solver(mesh: Arc<FvMesh>, axial_bc: Option<(f64, f64)>, dt: f64) -> MechanicsMeshSolver {
    let material = ElasticMaterial::new(
        YoungsModulus::new::<gigapascal>(YOUNG / 1.0e9),
        Ratio::new::<ratio>(POISSON),
        ThermalExpansionCoeff::new::<per_kelvin>(ALPHA),
        MassDensity::new::<kilogram_per_cubic_meter>(DENSITY),
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(500.0),
        ThermalConductivity::new::<watt_per_meter_kelvin>(15.0),
    )
    .unwrap();

    let (x_min, x_max) = axial_patches(&mesh);
    let t_boundary: Vec<PatchField<f64>> = mesh
        .patches
        .iter()
        .enumerate()
        .map(|(i, p)| match axial_bc {
            Some((t_lo, _)) if i == x_min => PatchField::fixed_value(p.size, t_lo),
            Some((_, t_hi)) if i == x_max => PatchField::fixed_value(p.size, t_hi),
            _ => PatchField::zero_gradient(p.size),
        })
        .collect();

    let dts = cell_delta_t(&mesh);
    let t_struct = VolScalarField::new(
        "TStruct",
        mesh.clone(),
        Field::from_fn(mesh.n_cells, |c| T_REF + dts[c]),
        t_boundary,
    );
    let disp = VolVectorField::new(
        "D",
        mesh.clone(),
        Field::new(vec![Vector3::ZERO; mesh.n_cells]),
        clamped_ends(&mesh),
    );

    let mut solver = MechanicsMeshSolver::new(
        LegacyThermoMechanics::new(material, RheologyOption::PlaneStrain),
        ThermodynamicTemperature::new::<kelvin>(T_REF),
        Time::new::<second>(dt),
        settings(),
        disp,
        t_struct,
    );
    solver.set_corrector_control(200, 1.0e-12);
    solver
}

/// Build the OFFBEAT solver on the same mesh with the same problem.
fn offbeat_solver(mesh: Arc<FvMesh>) -> MechanicsSolver {
    let mut solver = MechanicsSolver::new(
        mesh.clone(),
        LinearElastic::new(YOUNG, POISSON).unwrap(),
        clamped_ends(&mesh),
    );
    solver.set_linear_solver(settings());
    solver.set_corrector_control(200, 1.0e-12);
    // OFFBEAT defaults to the theoretical density of UO2; the comparison needs
    // the same steel-like density GeN-Foam reads off its material card.
    solver.set_density(DENSITY);
    let eig: Vec<f64> = cell_delta_t(&mesh).iter().map(|dt| ALPHA * dt).collect();
    solver.set_eigenstrain_field(&eig);
    solver
}

/// Largest absolute difference in the axial displacement component (m), and the
/// cell it occurs in.
fn worst_displacement_gap(a: &VolVectorField, b: &VolVectorField) -> (f64, usize) {
    (0..a.internal.len())
        .map(|c| ((a.internal[c] - b.internal[c]).mag(), c))
        .fold((0.0, 0), |acc, x| if x.0 > acc.0 { x } else { acc })
}

// ── Test 1: the agreement gate ──────────────────────────────────────────────

/// **V&V — code-to-code: the two ports agree to solver tolerance when posed the
/// identical problem.**
///
/// ## Methodology
///
/// One 20-cell bar written by the `blockMesh` CLI. Material `E = 200 GPa`,
/// `ν = 0`, `α = 1.2e-5 /K`; both axial ends clamped (`D = 0`), lateral faces
/// zero-gradient; temperature rise `ΔT(x) = 500 sin²(πx/L)` K about
/// `T_ref = 300 K`. The GeN-Foam temperature field carries **zero-gradient
/// patches on every boundary**, which is the one configuration in which its
/// `thermal_load_field` and OFFBEAT's `eigenstrain` field are the same field —
/// see test 2 for what happens otherwise. OFFBEAT's eigenstrain is set to
/// `α ΔT` cell by cell. Both use identical linear-solver settings
/// (`1e-14`, 200 000 sweeps) and corrector control (200, `1e-12 m`).
///
/// Pass criterion: max per-cell displacement difference below `1e-12 m`
/// (the corrector tolerance — i.e. the two codes are indistinguishable at the
/// level either one is converged to), and max axial-stress difference below
/// 1 Pa against a `~6e8 Pa` stress level.
///
/// Secondary, against the closed form `D(x) = −(αAL/4π) sin(2πx/L)` and
/// `σ_xx = −EαA/2`: both codes must land within 2 % of the analytical
/// displacement amplitude on this 20-cell mesh.
///
/// ## Results (measured 2026-09-15, `cargo test --release -p outram-foam-cli
/// --test mechanics_code_to_code`)
///
/// | quantity | measured |
/// |---|---|
/// | max per-cell `|ΔD|` between the two ports | **2.175e-19 m** (cell 14), against a 4.7746e-4 m field — relative 4.6e-16 |
/// | max `|Δσ_xx|` between the two ports | **4.768e-7 Pa**, against 6.0e8 Pa — relative 8e-16 |
/// | outer correctors | GeN-Foam 2, OFFBEAT 2 |
/// | GeN-Foam vs closed form, max `|D − D_exact|` | 3.885e-6 m = **0.814 %** of amplitude |
/// | OFFBEAT vs closed form, max `|D − D_exact|` | 3.885e-6 m = **0.814 %** of amplitude |
/// | GeN-Foam vs closed form, max relative `σ_xx` error | **2.417e-2** against `σ_exact = −6.0000e8 Pa` |
///
/// The two ports are *bit-level* equivalent here: 2.175e-19 m is the
/// accumulated floating-point difference of two arithmetically identical
/// assemblies, four orders below the 1e-12 m corrector tolerance, not an
/// agreement to tolerance. They also miss the closed form by the *same*
/// 0.814 %, which is the discretisation error of the shared scheme on a
/// 20-cell mesh — see
/// [`the_agreement_and_the_convergence_order_survive_refinement`] for its
/// convergence.
///
/// **Interpretation.** The two independent translations of the same OpenFOAM
/// discretisation, from two different upstream codes, produce the same numbers.
/// That is evidence against a transcription error in either port's constant
/// algebra, load construction, segregated split or stress recovery. It is *not*
/// evidence about the shared `fvm`/`fvc` operator layer, which cancels.
#[test]
fn genfoam_and_offbeat_agree_on_the_clamped_thermoelastic_bar() {
    let mesh = bar_mesh_via_block_mesh_cli("agree", 20);

    let mut gen = genfoam_solver(mesh.clone(), None, 1.0);
    let gen_report = gen.solve_displacement(true);
    let mut off = offbeat_solver(mesh.clone());
    let off_report = off.solve_quasi_static();

    assert!(
        gen_report.converged,
        "GeN-Foam did not converge: {gen_report:?}"
    );
    assert!(
        off_report.converged,
        "OFFBEAT did not converge: {off_report:?}"
    );

    let (gap, worst_cell) = worst_displacement_gap(gen.displacement(), off.displacement());
    let mut stress_gap = 0.0_f64;
    for c in 0..mesh.n_cells {
        let d = (gen.stress().internal[c].xx - off.stress().internal[c].xx).abs();
        if d > stress_gap {
            stress_gap = d;
        }
    }

    // Analytical comparison, reported for both codes.
    let mut gen_err = 0.0_f64;
    let mut off_err = 0.0_f64;
    let amplitude = ALPHA * DELTA_T_PEAK * LENGTH / (4.0 * std::f64::consts::PI);
    for c in 0..mesh.n_cells {
        let exact = analytic_displacement(mesh.cell_centres[c].x);
        gen_err = gen_err.max((gen.displacement().internal[c].x - exact).abs());
        off_err = off_err.max((off.displacement().internal[c].x - exact).abs());
    }
    let sigma_exact = analytic_axial_stress();
    let gen_sigma_err = (0..mesh.n_cells)
        .map(|c| (gen.stress().internal[c].xx - sigma_exact).abs() / sigma_exact.abs())
        .fold(0.0_f64, f64::max);

    println!(
        "code-to-code: max |ΔD| = {gap:.3e} m at cell {worst_cell}; \
         max |Δσ_xx| = {stress_gap:.3e} Pa\n\
         analytic:     GeN-Foam max |D − D_exact| = {gen_err:.3e} m \
         ({:.3} % of amplitude {amplitude:.4e} m), OFFBEAT {off_err:.3e} m \
         ({:.3} %)\n\
         analytic:     GeN-Foam max relative σ_xx error = {gen_sigma_err:.3e} \
         against σ_exact = {sigma_exact:.4e} Pa\n\
         correctors:   GeN-Foam {}, OFFBEAT {}",
        100.0 * gen_err / amplitude,
        100.0 * off_err / amplitude,
        gen_report.n_correctors,
        off_report.iterations,
    );

    assert!(
        gap < 1.0e-12,
        "the two ports disagree by {gap:.3e} m at cell {worst_cell}, above the \
         1e-12 m corrector tolerance either one is converged to"
    );
    assert!(
        stress_gap < 1.0,
        "the two ports disagree on axial stress by {stress_gap:.3e} Pa"
    );
    assert!(
        gen_err / amplitude < 0.02 && off_err / amplitude < 0.02,
        "discretisation error against the closed form exceeds 2 % of the \
         displacement amplitude: GeN-Foam {:.3} %, OFFBEAT {:.3} %",
        100.0 * gen_err / amplitude,
        100.0 * off_err / amplitude
    );
}

// ── Test 2: the discrepancy the comparison found ────────────────────────────

/// **V&V — code-to-code: a real difference in how the two ports treat the
/// thermal load on a Dirichlet boundary.**
///
/// ## What differs
///
/// GeN-Foam's [`MechanicsMeshSolver`] builds its load field by *mapping the
/// temperature field's own boundary condition* through `3Kα(·−T_ref)`: a
/// `FixedValue` temperature patch becomes a `FixedValue` load patch, so
/// `fvc::grad` sees the true boundary value.
///
/// OFFBEAT's [`MechanicsSolver`] stores the eigenstrain in a
/// `VolScalarField::uniform`, whose patches are **always zero-gradient**, and
/// `set_eigenstrain_field` writes internal cells only. There is no way for a
/// caller to give the eigenstrain a boundary value. So where the eigenstrain is
/// non-uniform up to a Dirichlet boundary, OFFBEAT extrapolates the owner cell's
/// value onto the boundary face and GeN-Foam does not.
///
/// This is not a test artefact: it is the difference a coupled fuel-performance
/// run would hit at a pellet surface whose temperature is imposed.
///
/// ## Methodology
///
/// Identical to test 1 except that the GeN-Foam temperature field carries
/// `FixedValue` patches on `xMin`/`xMax` at `T_ref + ΔT(0)` and
/// `T_ref + ΔT(L)` — here both exactly `T_ref`, since the chosen profile
/// vanishes at the ends. The difference is therefore the *smallest* this
/// profile can produce (the true boundary load is zero and OFFBEAT's
/// extrapolation is only one half-cell off); a profile with a non-zero end
/// value would separate the two codes further.
///
/// ## Results (measured 2026-09-15, 20 cells)
///
/// | quantity | measured |
/// |---|---|
/// | max per-cell `|ΔD|` | **8.772e-7 m** at cell 19 (the cell against `xMax`) = **0.1837 %** of the 4.7746e-4 m amplitude |
/// | max `|Δσ_xx|` | **3.324e6 Pa** = 0.55 % of the 6.0e8 Pa stress level |
///
/// Compare the same mesh and material in test 1, where the two ports differ by
/// 2.175e-19 m: the boundary treatment, and nothing else, is worth twelve
/// orders of magnitude here. The disagreement is localised at the boundary
/// cells, as the mechanism predicts.
///
/// **Which one is right.** GeN-Foam. The imposed temperature at `xMin`/`xMax`
/// is a known boundary value of the load field, and using it is the consistent
/// treatment; OFFBEAT's zero-gradient extrapolation is a half-cell error in a
/// quantity that is known exactly. This is a defect in OFFBEAT's API surface
/// rather than in its assembly — `set_eigenstrain_field` writes internal cells
/// and the field's patches are fixed at zero-gradient by
/// `VolScalarField::uniform`, so a caller cannot supply the boundary value even
/// when it has one.
///
/// This test **asserts the disagreement**, so that it fails the day OFFBEAT
/// gains a way to set the eigenstrain boundary — at which point it should be
/// replaced by a second agreement gate. Tracked as `op-cdte`.
#[test]
fn the_two_codes_disagree_on_the_thermal_load_at_a_dirichlet_boundary() {
    let mesh = bar_mesh_via_block_mesh_cli("dirichlet", 20);

    let ends = (T_REF + delta_t(0.0), T_REF + delta_t(LENGTH));
    let mut gen = genfoam_solver(mesh.clone(), Some(ends), 1.0);
    let gen_report = gen.solve_displacement(true);
    let mut off = offbeat_solver(mesh.clone());
    let off_report = off.solve_quasi_static();

    assert!(gen_report.converged && off_report.converged);

    let (gap, worst_cell) = worst_displacement_gap(gen.displacement(), off.displacement());
    let amplitude = ALPHA * DELTA_T_PEAK * LENGTH / (4.0 * std::f64::consts::PI);

    let mut stress_gap = 0.0_f64;
    for c in 0..mesh.n_cells {
        let d = (gen.stress().internal[c].xx - off.stress().internal[c].xx).abs();
        if d > stress_gap {
            stress_gap = d;
        }
    }

    println!(
        "Dirichlet-load difference: max |ΔD| = {gap:.3e} m at cell {worst_cell} \
         ({:.4} % of the {amplitude:.4e} m amplitude); max |Δσ_xx| = \
         {stress_gap:.3e} Pa",
        100.0 * gap / amplitude
    );

    assert!(
        gap > 1.0e-12,
        "the two codes now agree ({gap:.3e} m) with a Dirichlet temperature \
         boundary — OFFBEAT has presumably gained eigenstrain boundary control. \
         Replace this test with an agreement gate and close op-cdte."
    );
}

// ── Test 3: the agreement survives mesh refinement ──────────────────────────

/// **V&V — code-to-code agreement is not an artefact of one mesh, and both
/// ports converge on the closed form at the same observed order.**
///
/// ## Methodology
///
/// The problem of test 1 (all-zero-gradient load patches, so the two load
/// fields coincide) meshed by the `blockMesh` CLI at 10, 20, 40 and 80 axial
/// cells. At each resolution: assert the two ports still agree to `1e-12 m`,
/// and record the max-norm displacement error against
/// `D(x) = −(αAL/4π) sin(2πx/L)`. The observed order between successive
/// refinements is `log2(e_coarse / e_fine)`.
///
/// Pass criterion: agreement below `1e-12 m` at every resolution, the
/// analytical error monotonically decreasing, and the finest observed order at
/// least 1.5 — i.e. the scheme is converging at close to its nominal second
/// order and the two codes are converging to the *same* answer, not merely
/// sitting at the same tolerance.
///
/// The first-order boundary treatment of the zero-gradient load patch is the
/// reason the order is quoted with a floor of 1.5 rather than asserted at
/// 2.0 ± 0.2: the profile's zero end-slope suppresses that error term but does
/// not remove it.
///
/// ## Results (measured 2026-09-15)
///
/// | cells | code-to-code `|ΔD|` (m) | analytic error (m) | % of amplitude | observed order |
/// |---|---|---|---|---|
/// | 10 | 1.084e-19 | 1.5812e-5 | 3.3117 % | — |
/// | 20 | 2.175e-19 | 3.8850e-6 | 0.8137 % | 2.025 |
/// | 40 | 4.902e-19 | 9.7912e-7 | 0.2051 % | 1.988 |
/// | 80 | 3.817e-19 | 2.4527e-7 | 0.0514 % | 1.997 |
///
/// **Interpretation.** The observed order is 2.00 ± 0.03 across three
/// refinements, matching the nominal second order of the scheme, and the
/// code-to-code difference stays at round-off (`~1e-19 m`, i.e. `~1e-16`
/// relative) at every resolution rather than growing with problem size. The two
/// ports are therefore converging to the same solution, not merely agreeing at
/// one mesh.
#[test]
fn the_agreement_and_the_convergence_order_survive_refinement() {
    let resolutions = [10usize, 20, 40, 80];
    let amplitude = ALPHA * DELTA_T_PEAK * LENGTH / (4.0 * std::f64::consts::PI);
    let mut errors: Vec<f64> = Vec::new();

    for &n in &resolutions {
        let mesh = bar_mesh_via_block_mesh_cli("refine", n);

        let mut gen = genfoam_solver(mesh.clone(), None, 1.0);
        let gen_report = gen.solve_displacement(true);
        let mut off = offbeat_solver(mesh.clone());
        let off_report = off.solve_quasi_static();
        assert!(
            gen_report.converged && off_report.converged,
            "a solver failed to converge at n = {n}"
        );

        let (gap, worst_cell) = worst_displacement_gap(gen.displacement(), off.displacement());
        assert!(
            gap < 1.0e-12,
            "at n = {n} the two ports disagree by {gap:.3e} m at cell {worst_cell}"
        );

        let mut err = 0.0_f64;
        for c in 0..mesh.n_cells {
            let exact = analytic_displacement(mesh.cell_centres[c].x);
            err = err.max((gen.displacement().internal[c].x - exact).abs());
        }
        println!(
            "n = {n:3}: code-to-code |ΔD| = {gap:.3e} m, analytic error = \
             {err:.4e} m ({:.4} % of amplitude)",
            100.0 * err / amplitude
        );
        errors.push(err);
    }

    let mut orders = Vec::new();
    for w in errors.windows(2) {
        orders.push((w[0] / w[1]).log2());
    }
    println!("observed orders: {orders:?}");

    for (i, w) in errors.windows(2).enumerate() {
        assert!(
            w[1] < w[0],
            "refining from {} to {} cells did not reduce the error \
             ({:.4e} -> {:.4e})",
            resolutions[i],
            resolutions[i + 1],
            w[0],
            w[1]
        );
    }
    let finest = *orders.last().unwrap();
    assert!(
        finest >= 1.5,
        "observed order on the finest refinement is {finest:.3}, below the 1.5 \
         floor for a nominally second-order scheme"
    );
}

// ── Test 4: the transient (inertial) path ───────────────────────────────────

/// **V&V — code-to-code: the two ports agree through a transient elastic
/// response, not only at equilibrium.**
///
/// ## Why this is a separate test
///
/// Tests 1–3 all take the quasi-static branch, which drops `ρ ∂²D/∂t²` and
/// never touches the displacement history. The transient branch adds
/// `fvm::d2dt2_coeff` over `D`, `D_old`, `D_oldold`, and both ports rotate that
/// history themselves in `advance_time`. It also brings density into play, and
/// the two codes obtain it by different routes — GeN-Foam reads it off the
/// [`ElasticMaterial`] card, OFFBEAT takes it from `set_density` and otherwise
/// defaults to the theoretical density of UO2 (10 960 kg/m³), which is a
/// 40 % difference waiting to happen if a caller forgets. Setting it explicitly
/// here is part of posing the same problem.
///
/// ## Methodology
///
/// The bar of test 1 (20 cells, all-zero-gradient load patches) starting from
/// rest and undeformed, with the full thermal load applied instantaneously at
/// `t = 0` — a step load, so the bar rings rather than creeping to equilibrium.
///
/// The elastic wave speed is `c = sqrt((2μ+λ)/ρ) = sqrt(200e9/7850) =
/// 5048 m/s`, so one axial transit takes `L/c = 198 µs` and one cell is
/// `Δx/c = 9.9 µs`. The step is `Δt = 2 µs` (CFL ≈ 0.2) and 50 steps are taken,
/// reaching `t = 100 µs` — half a transit, so the solution is genuinely
/// mid-transient and not a disguised steady state. Both codes call
/// `advance_time` once per completed step.
///
/// Pass criterion: max per-cell displacement difference below `1e-12 m` at the
/// final step, and — as a guard against the test silently comparing two
/// stationary fields — the transient displacement must differ from the
/// quasi-static answer of test 1 by more than 1 % of its amplitude.
///
/// ## Results (measured 2026-09-15)
///
/// | quantity | measured |
/// |---|---|
/// | integration reached | `t = 100.0 µs` = 0.50 axial transits at `c = 5048 m/s` |
/// | max per-cell `|ΔD|` between the two ports | **2.521e-18 m** (cell 10) |
/// | distance from the quasi-static answer | 4.233e-4 m = **88.67 %** of the 4.7746e-4 m amplitude |
///
/// **Interpretation.** The transient field is 88.67 % of an amplitude away from
/// equilibrium — the bar is ringing, not settled — and the two ports still track
/// each other to 2.5e-18 m. The difference is an order larger than the
/// quasi-static 2.175e-19 m, which is what 50 accumulating steps of
/// floating-point history rotation should look like; it is still six orders
/// below the 1e-12 m corrector tolerance and shows no sign of divergence.
#[test]
fn the_two_codes_agree_through_a_transient_elastic_response() {
    const DT: f64 = 2.0e-6;
    const STEPS: usize = 50;

    let mesh = bar_mesh_via_block_mesh_cli("transient", 20);

    let mut gen = genfoam_solver(mesh.clone(), None, DT);
    let mut off = offbeat_solver(mesh.clone());

    for step in 0..STEPS {
        let g = gen.solve_displacement(false);
        let o = off.solve_transient(DT);
        assert!(
            g.converged && o.converged,
            "a solver failed to converge at step {step}: GeN-Foam {g:?}, OFFBEAT {o:?}"
        );
        gen.advance_time();
        off.advance_time();
    }

    let (gap, worst_cell) = worst_displacement_gap(gen.displacement(), off.displacement());

    // Guard: the transient answer must not have already collapsed onto the
    // quasi-static one, or this test would be test 1 in disguise.
    let mut steady = genfoam_solver(mesh.clone(), None, 1.0);
    steady.solve_displacement(true);
    let (from_steady, _) = worst_displacement_gap(gen.displacement(), steady.displacement());
    let amplitude = ALPHA * DELTA_T_PEAK * LENGTH / (4.0 * std::f64::consts::PI);

    let wave_speed = (YOUNG / DENSITY).sqrt();
    println!(
        "transient: after {STEPS} steps of {DT:.1e} s (t = {:.1} µs, {:.2} axial \
         transits at c = {wave_speed:.0} m/s):\n\
         \tcode-to-code max |ΔD| = {gap:.3e} m at cell {worst_cell}\n\
         \tdistance from the quasi-static answer = {from_steady:.3e} m \
         ({:.2} % of the {amplitude:.4e} m amplitude)",
        1.0e6 * DT * STEPS as f64,
        DT * STEPS as f64 * wave_speed / LENGTH,
        100.0 * from_steady / amplitude,
    );

    assert!(
        from_steady / amplitude > 0.01,
        "the transient solution is within {:.3} % of the quasi-static one, so \
         this test is not exercising the inertial term — shorten the integration \
         or raise the load",
        100.0 * from_steady / amplitude
    );
    assert!(
        gap < 1.0e-12,
        "the two ports disagree by {gap:.3e} m at cell {worst_cell} after \
         {STEPS} transient steps"
    );
}
