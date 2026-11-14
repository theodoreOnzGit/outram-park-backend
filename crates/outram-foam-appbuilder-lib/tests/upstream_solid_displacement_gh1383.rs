//! `SolidDisplacementFoamEsi` (and, as a second row, GeN-Foam's
//! `MechanicsMeshSolver`) against the real ESI OpenFOAM v2506 `solidDisplacementFoam`
//! binary on a 6x4x3 graded block, 6 steps (gh:#1383, epic #1336).
//!
//! # Method
//! `verification_and_validation/upstream_solid_displacement_gh1383/README.md` holds the case,
//! the predictions (committed before this test was first run) and the results.
//! `reference/<variant>/` holds the written fields of the unmodified binary at
//! `writePrecision 17` (round-trips IEEE doubles) and its log, which also prints every
//! residual with 17 digits. The port is built from the same `polyMesh` and the same
//! constants; D, T, sigma and sigmaEq per cell are compared bit for bit at each step, and
//! every linear solve (initial residual, final residual, PCG iterations) against the log.
//!
//! # Results
//! See the README ("Results").

use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_appbuilder_lib::solvers::solid_displacement_foam::{
    DisplacementPatch, PcgSettings, SolidDisplacementControls, SolidDisplacementFoamEsi, SolidProperties, StepLog,
    TemperaturePatch, ThermalProperties,
};
use outram_foam_basic_lib::prelude::*;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/upstream_solid_displacement_gh1383")
}

/// Numbers inside an OpenFOAM list after `internalField nonuniform List<...> N (`.
fn parse_internal(text: &str, per: usize) -> Vec<Vec<f64>> {
    let at = text.find("internalField").expect("internalField");
    let rest = &text[at..];
    let open = rest.find('(').expect("list");
    let end = rest.find("boundaryField").unwrap_or(rest.len());
    let body = &rest[open + 1..end];
    let nums: Vec<f64> = body
        .replace(['(', ')', ';'], " ")
        .split_whitespace()
        .filter_map(|t| t.parse::<f64>().ok())
        .collect();
    nums.chunks(per).map(|c| c.to_vec()).collect()
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a == 0.0 && b == 0.0)
}

#[derive(Default, Debug)]
struct Tally {
    n: usize,
    identical: usize,
    worst_abs: f64,
    worst_rel: f64,
}

impl Tally {
    fn add(&mut self, port: f64, up: f64) {
        self.n += 1;
        if same(port, up) {
            self.identical += 1;
        }
        let d = (port - up).abs();
        self.worst_abs = self.worst_abs.max(d);
        if up != 0.0 {
            self.worst_rel = self.worst_rel.max(d / up.abs());
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct UpSolve {
    field: String,
    initial: f64,
    fin: f64,
    iters: usize,
}

/// Parse the solver log: per "Iteration:" block the list of `Solving for` lines.
fn parse_log(text: &str) -> Vec<Vec<UpSolve>> {
    let mut steps: Vec<Vec<UpSolve>> = Vec::new();
    for line in text.lines() {
        if line.starts_with("Iteration:") {
            steps.push(Vec::new());
        } else if line.contains("Solving for") {
            let field = line.split("Solving for ").nth(1).unwrap().split(',').next().unwrap().to_string();
            let num = |key: &str| -> f64 {
                let s = line.split(key).nth(1).unwrap().trim_start();
                s.split(|c: char| c == ',' || c.is_whitespace()).next().unwrap().parse().unwrap()
            };
            steps.last_mut().unwrap().push(UpSolve {
                field,
                initial: num("Initial residual = "),
                fin: num("Final residual = "),
                iters: num("No Iterations ") as usize,
            });
        }
    }
    steps
}

fn build(thermal: bool, top_held: bool) -> SolidDisplacementFoamEsi {
    let mesh = read_poly_mesh(&root().join("case/constant/polyMesh")).expect("polyMesh");
    let props = SolidProperties {
        rho: 7854.0,
        young: 2e11,
        nu: 0.3,
        plane_stress: false,
        thermal: if thermal { Some(ThermalProperties { c: 434.0, k: 60.5, alpha: 1.1e-5 }) } else { None },
    };
    let pcg_d = PcgSettings { tolerance: 1e-12, rel_tol: 0.0, max_iter: 1000 };
    let pcg_t = PcgSettings { tolerance: 1e-14, rel_tol: 0.0, max_iter: 1000 };
    let controls = SolidDisplacementControls {
        compact_normal_stress: true,
        n_correctors: 4,
        convergence_tolerance: 1e-6,
        d_solver: pcg_d,
        t_solver: pcg_t,
    };
    let d: Vec<DisplacementPatch> = mesh
        .patches
        .iter()
        .map(|p| match p.name.as_str() {
            "left" => DisplacementPatch::FixedValue(Vector3::ZERO),
            "right" => DisplacementPatch::FixedValue(Vector3::new(1e-4, 2e-5, -1e-5)),
            "top" if top_held => DisplacementPatch::FixedValue(Vector3::new(0.0, 3e-5, 0.0)),
            _ => DisplacementPatch::ZeroGradient,
        })
        .collect();
    let t: Vec<TemperaturePatch> = mesh
        .patches
        .iter()
        .map(|p| match p.name.as_str() {
            "left" => TemperaturePatch::FixedValue(300.0),
            "right" => TemperaturePatch::FixedValue(380.0),
            _ => TemperaturePatch::ZeroGradient,
        })
        .collect();
    SolidDisplacementFoamEsi::new(mesh, props, controls, 1000.0, d, t, 300.0).expect("port")
}

struct Totals {
    d: Tally,
    t: Tally,
    sigma: Tally,
    sigma_eq: Tally,
    solves: usize,
    solves_exact: usize,
    worst_residual_rel: f64,
}

fn run_variant(variant: &str, thermal: bool, top_held: bool) -> Totals {
    let base = root().join("reference").join(variant);
    let log = parse_log(&std::fs::read_to_string(base.join("log.solidDisplacementFoam")).expect("log"));
    assert_eq!(log.len(), 6);
    let mut port = build(thermal, top_held);
    let mut tot = Totals {
        d: Tally::default(),
        t: Tally::default(),
        sigma: Tally::default(),
        sigma_eq: Tally::default(),
        solves: 0,
        solves_exact: 0,
        worst_residual_rel: 0.0,
    };
    for step in 1..=6usize {
        let sl: StepLog = port.step();
        // linear solves, in order
        let up = &log[step - 1];
        assert_eq!(sl.solves.len(), up.len(), "step {step}: number of linear solves");
        for (a, b) in sl.solves.iter().zip(up) {
            tot.solves += 1;
            assert_eq!(a.field, b.field, "step {step}: solve order");
            if a.n_iterations == b.iters && same(a.initial_residual, b.initial) && same(a.final_residual, b.fin) {
                tot.solves_exact += 1;
            }
            tot.worst_residual_rel = tot
                .worst_residual_rel
                .max(((a.initial_residual - b.initial) / b.initial.abs().max(1e-300)).abs());
            println!(
                "[{variant}] step {step} {:>2}: port it {:>3} init {:.17e} fin {:.17e} | up it {:>3} init {:.17e} fin {:.17e}",
                a.field, a.n_iterations, a.initial_residual, a.final_residual, b.iters, b.initial, b.fin
            );
        }
        let dir = base.join(format!("{}", step * 1000));
        let d_up = parse_internal(&std::fs::read_to_string(dir.join("D")).unwrap(), 3);
        for (c, v) in port.displacement().iter().enumerate() {
            tot.d.add(v.x, d_up[c][0]);
            tot.d.add(v.y, d_up[c][1]);
            tot.d.add(v.z, d_up[c][2]);
        }
        if thermal {
            let t_up = parse_internal(&std::fs::read_to_string(dir.join("T")).unwrap(), 1);
            for (c, v) in port.temperature().iter().enumerate() {
                tot.t.add(*v, t_up[c][0]);
            }
        }
        let s_up = parse_internal(&std::fs::read_to_string(dir.join("sigma")).unwrap(), 6);
        for (c, s) in port.sigma().iter().enumerate() {
            for (k, v) in [s.xx, s.xy, s.xz, s.yy, s.yz, s.zz].iter().enumerate() {
                tot.sigma.add(*v, s_up[c][k]);
            }
        }
        let e_up = parse_internal(&std::fs::read_to_string(dir.join("sigmaEq")).unwrap(), 1);
        for (c, v) in port.sigma_eq().iter().enumerate() {
            tot.sigma_eq.add(*v, e_up[c][0]);
        }
    }
    println!("[{variant}] D {:?}\n[{variant}] T {:?}\n[{variant}] sigma {:?}\n[{variant}] sigmaEq {:?}", tot.d, tot.t, tot.sigma, tot.sigma_eq);
    println!("[{variant}] solves exact {}/{} (worst relative initial residual {:e})", tot.solves_exact, tot.solves, tot.worst_residual_rel);
    tot
}

/// The mechanical variant (`thermalStress no`).
#[test]
fn mechanical_variant_matches_the_esi_binary_bit_for_bit() {
    let t = run_variant("mech", false, false);
    assert_eq!(t.d.identical, t.d.n, "D: {:?}", t.d);
    assert_eq!(t.sigma.identical, t.sigma.n, "sigma: {:?}", t.sigma);
    assert_eq!(t.sigma_eq.identical, t.sigma_eq.n, "sigmaEq: {:?}", t.sigma_eq);
    assert_eq!(t.solves_exact, t.solves);
}

/// The three-dimensional mechanical variant: the top patch is held at (0, 3e-5, 0), so the
/// field depends on all three coordinates and the explicit `divSigmaExp` and the patch-normal
/// gradient correction are active (the first variant's converged field is one-dimensional).
#[test]
fn three_dimensional_mechanical_variant_matches_the_esi_binary_bit_for_bit() {
    let t = run_variant("mech3d", false, true);
    assert_eq!(t.d.identical, t.d.n, "D: {:?}", t.d);
    assert_eq!(t.sigma.identical, t.sigma.n, "sigma: {:?}", t.sigma);
    assert_eq!(t.sigma_eq.identical, t.sigma_eq.n, "sigmaEq: {:?}", t.sigma_eq);
    assert_eq!(t.solves_exact, t.solves);
}

/// The thermal variant (`thermalStress yes`, T solved with `fvm::ddt == fvm::laplacian`).
#[test]
fn thermal_variant_matches_the_esi_binary_bit_for_bit() {
    let t = run_variant("thermal", true, false);
    assert_eq!(t.t.identical, t.t.n, "T: {:?}", t.t);
    assert_eq!(t.d.identical, t.d.n, "D: {:?}", t.d);
    assert_eq!(t.sigma.identical, t.sigma.n, "sigma: {:?}", t.sigma);
    assert_eq!(t.sigma_eq.identical, t.sigma_eq.n, "sigmaEq: {:?}", t.sigma_eq);
    assert_eq!(t.solves_exact, t.solves);
}

/// Control: the comparison can fail. The mechanical port is not the thermal
/// reference (a different problem), and the tally reports it.
#[test]
fn control_a_different_problem_is_not_reported_identical() {
    let mut port = build(false, false);
    let _ = port.step();
    let dir = root().join("reference/thermal/1000");
    let d_up = parse_internal(&std::fs::read_to_string(dir.join("D")).unwrap(), 3);
    let mut t = Tally::default();
    for (c, v) in port.displacement().iter().enumerate() {
        t.add(v.x, d_up[c][0]);
    }
    assert!(t.identical < t.n && t.worst_abs > 0.0, "{t:?}");
}

/// Second row: GeN-Foam's `MechanicsMeshSolver` (quasi-static, converged tightly)
/// against the converged ESI fixed point (400 steps, `mechlong`). Not expected to
/// match to round-off (README prediction 2); the measured difference is printed and
/// asserted to be non-zero and below the displacement amplitude.
#[test]
fn mechanics_mesh_solver_against_the_converged_esi_field() {
    for (variant, top_held) in [("mechlong", false), ("mech3dlong", true)] {
        let _ = mechanics_mesh_solver_row(variant, top_held);
    }
}

fn mechanics_mesh_solver_row(variant: &str, top_held: bool) -> Vec<Vector3> {
    use outram_foam_appbuilder_lib::genfoam::thermo_mechanics::{
        ElasticMaterial, LegacyThermoMechanics, MechanicsMeshSolver, RheologyOption, ThermalExpansionCoeff, YoungsModulus,
    };
    use outram_foam_basic_lib::fields::boundary::bc::{BoundaryCondition, PatchField};
    use outram_foam_basic_lib::fields::field::Field;
    use outram_foam_basic_lib::fields::vol_field::{VolScalarField, VolVectorField};
    use uom::si::f64::{MassDensity, Ratio, SpecificHeatCapacity, ThermalConductivity, ThermodynamicTemperature, Time};
    use uom::si::{
        mass_density::kilogram_per_cubic_meter, pressure::pascal, ratio::ratio,
        specific_heat_capacity::joule_per_kilogram_kelvin, temperature_coefficient::per_kelvin,
        thermal_conductivity::watt_per_meter_kelvin, thermodynamic_temperature::kelvin, time::second,
    };

    let mesh = read_poly_mesh(&root().join("case/constant/polyMesh")).expect("polyMesh");
    let material = ElasticMaterial::new(
        YoungsModulus::new::<pascal>(2e11),
        Ratio::new::<ratio>(0.3),
        ThermalExpansionCoeff::new::<per_kelvin>(1.1e-5),
        MassDensity::new::<kilogram_per_cubic_meter>(7854.0),
        SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(434.0),
        ThermalConductivity::new::<watt_per_meter_kelvin>(60.5),
    )
    .unwrap();
    let model = LegacyThermoMechanics::new(material, RheologyOption::PlaneStrain);
    let right = Vector3::new(1e-4, 2e-5, -1e-5);
    let bnd = mesh
        .patches
        .iter()
        .map(|p| match p.name.as_str() {
            "left" => PatchField { bc: BoundaryCondition::FixedValue(Vector3::ZERO), values: Field::uniform(p.size, Vector3::ZERO) },
            "right" => PatchField { bc: BoundaryCondition::FixedValue(right), values: Field::uniform(p.size, right) },
            "top" if top_held => {
                let v = Vector3::new(0.0, 3e-5, 0.0);
                PatchField { bc: BoundaryCondition::FixedValue(v), values: Field::uniform(p.size, v) }
            }
            _ => PatchField::zero_gradient_vec(p.size),
        })
        .collect();
    let disp = VolVectorField::new("D", mesh.clone(), Field::uniform(mesh.n_cells, Vector3::ZERO), bnd);
    let t = VolScalarField::uniform("T", mesh.clone(), 300.0);
    let settings = SolverSettings { tolerance: 1e-13, max_iter: 50_000 };
    let mut solver = MechanicsMeshSolver::new(
        model,
        ThermodynamicTemperature::new::<kelvin>(300.0),
        Time::new::<second>(1.0),
        settings,
        disp,
        t,
    );
    solver.set_corrector_control(3000, 1e-15);
    let rep = solver.solve_displacement(true);
    let up = parse_internal(&std::fs::read_to_string(root().join("reference").join(variant).join("400000/D")).unwrap(), 3);
    let mut tally = Tally::default();
    let mut amp = 0.0_f64;
    for (c, v) in solver.displacement().internal.iter().enumerate() {
        tally.add(v.x, up[c][0]);
        tally.add(v.y, up[c][1]);
        tally.add(v.z, up[c][2]);
        amp = amp.max(up[c][0].abs()).max(up[c][1].abs()).max(up[c][2].abs());
    }
    println!(
        "[{}] MechanicsMeshSolver correctors {} converged {} | D vs converged ESI: {:?} amplitude {:e} worst_abs/amplitude {:e}",
        variant, rep.n_correctors, rep.converged, tally, amp, tally.worst_abs / amp
    );
    assert!(tally.worst_abs > 0.0, "{variant}: not bit-identical: a different discretisation");
    assert!(tally.worst_abs / amp < 1.0, "{variant}: same problem: difference below the amplitude");
    solver.displacement().internal.iter().copied().collect()
}

/// Localisation of the 3-D difference (README "Results", row 2): the ESI-form port with ONLY
/// upstream's patch-normal boundary gradient correction removed (owner-cell boundary values,
/// as `fvc::grad_vec` leaves them) is run to its fixed point; it must agree with
/// `MechanicsMeshSolver` to round-off-level accumulation and sit as far from ESI as that solver
/// does. The cause of the miss is then the boundary gradient, not the algebra.
#[test]
fn the_boundary_gradient_correction_accounts_for_the_mechanics_mesh_solver_difference() {
    let mms = mechanics_mesh_solver_row("mech3dlong", true);
    let mut ablated = build(false, true).without_boundary_gradient_correction();
    for _ in 0..400 {
        let _ = ablated.step();
    }
    let up = parse_internal(&std::fs::read_to_string(root().join("reference/mech3dlong/400000/D")).unwrap(), 3);
    let (mut vs_mms, mut vs_esi) = (Tally::default(), Tally::default());
    for (c, v) in ablated.displacement().iter().enumerate() {
        for (k, x) in [v.x, v.y, v.z].iter().enumerate() {
            vs_esi.add(*x, up[c][k]);
            vs_mms.add(*x, [mms[c].x, mms[c].y, mms[c].z][k]);
        }
    }
    println!("ablated ESI-form port vs MechanicsMeshSolver: {vs_mms:?}\nablated ESI-form port vs ESI binary: {vs_esi:?}");
    assert!(vs_mms.worst_abs < 1e-3 * vs_esi.worst_abs, "ablation reproduces the other solver: {vs_mms:?} vs {vs_esi:?}");
}
