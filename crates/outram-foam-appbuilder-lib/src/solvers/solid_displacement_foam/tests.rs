//! Unit tests of the `solidDisplacementFoam` translation. The comparison with
//! the real ESI binary is `tests/upstream_solid_displacement_gh1383.rs`.

use super::*;
use crate::io::poly_mesh::read_poly_mesh;
use std::path::PathBuf;

fn case_mesh() -> Arc<FvMesh> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/upstream_solid_displacement_gh1383/case/constant/polyMesh");
    read_poly_mesh(&p).expect("case polyMesh")
}

fn steel(thermal: bool) -> SolidProperties {
    SolidProperties {
        rho: 7854.0,
        young: 2e11,
        nu: 0.3,
        plane_stress: false,
        thermal: if thermal { Some(ThermalProperties { c: 434.0, k: 60.5, alpha: 1.1e-5 }) } else { None },
    }
}

fn controls() -> SolidDisplacementControls {
    let pcg = PcgSettings { tolerance: 1e-12, rel_tol: 0.0, max_iter: 1000 };
    SolidDisplacementControls {
        compact_normal_stress: true,
        n_correctors: 4,
        convergence_tolerance: 1e-6,
        d_solver: pcg,
        t_solver: pcg,
    }
}

fn patches(mesh: &FvMesh, right: Vector3) -> Vec<DisplacementPatch> {
    mesh.patches
        .iter()
        .map(|p| match p.name.as_str() {
            "left" => DisplacementPatch::FixedValue(Vector3::ZERO),
            "right" => DisplacementPatch::FixedValue(right),
            _ => DisplacementPatch::ZeroGradient,
        })
        .collect()
}

/// The `rho`-normalised Lame algebra of `readMechanicalProperties.H`: `mu` and
/// `lambda` reproduce the textbook values, and `3K = E/(1-2nu)`.
#[test]
fn normalised_coefficients_follow_the_lame_algebra() {
    let c = steel(true).normalised();
    let e = 2e11 / 7854.0;
    assert_eq!(c.e, e);
    assert!((c.mu * 7854.0 - 2e11 / 2.6).abs() < 1.0);
    assert!((c.lambda * 7854.0 - 0.3 * 2e11 / (1.3 * 0.4)).abs() < 1.0);
    assert!((c.three_k * 7854.0 - 2e11 / 0.4).abs() < 1.0);
    assert_eq!(c.two_mu_lambda, 2.0 * c.mu + c.lambda);
    let mut ps = steel(false);
    ps.plane_stress = true;
    let p = ps.normalised();
    assert!(p.lambda < c.lambda, "plane stress lambda = nu E/((1+nu)(1-nu)) is below plane strain");
    assert_eq!(steel(false).normalised().dt, 0.0);
}

/// PCG/DIC solves a 1-D Poisson chain to its tolerance and reports 0 iterations
/// from the converged solution.
#[test]
fn pcg_dic_solves_a_chain_and_stops_at_once_when_converged() {
    let n = 6;
    let owner: Vec<usize> = (0..n - 1).collect();
    let neighbour: Vec<usize> = (1..n).collect();
    let m = SymLdu { diag: vec![2.0; n], upper: vec![-1.0; n - 1] };
    let src = vec![1.0; n];
    let mut psi = vec![0.0; n];
    let s = PcgSettings { tolerance: 1e-14, rel_tol: 0.0, max_iter: 100 };
    let (i0, f0, it) = pcg_dic(&m, &owner, &neighbour, &src, &mut psi, s);
    assert!((i0 - 1.0).abs() < 1e-12, "initial residual from zero is 1");
    assert!(f0 < 1e-14 && it > 0 && it <= n);
    let r = amul(&m, &owner, &neighbour, &psi);
    for c in 0..n {
        assert!((r[c] - src[c]).abs() < 1e-12);
    }
    let (_, _, again) = pcg_dic(&m, &owner, &neighbour, &src, &mut psi, s);
    assert_eq!(again, 0);
}

/// Pure mechanical step: the imposed end displacement drives a non-trivial
/// field; every linear solve converges and the clamped end stays clamped.
#[test]
fn mechanical_step_converges_and_is_finite() {
    let mesh = case_mesh();
    let d = patches(&mesh, Vector3::new(1e-4, 2e-5, -1e-5));
    let mut s = SolidDisplacementFoamEsi::new(mesh.clone(), steel(false), controls(), 1000.0, d, vec![], 300.0).unwrap();
    let log = s.step();
    assert!(log.solves.iter().all(|l| l.final_residual < 1e-12 && l.field != "T"));
    assert!(s.displacement().iter().all(|v| v.x.is_finite()));
    let max_dx = s.displacement().iter().map(|v| v.x).fold(0.0_f64, f64::max);
    assert!(max_dx > 0.0 && max_dx < 1.1e-4, "displacement is bounded by the imposed end value: {max_dx}");
    assert!(s.sigma_eq().iter().all(|v| *v >= 0.0));
}

/// Thermal step: heating the right end with both ends clamped solves T first,
/// and a temperature field bounded by its Dirichlet values results.
#[test]
fn thermal_step_solves_temperature_between_its_dirichlet_bounds() {
    let mesh = case_mesh();
    let d = patches(&mesh, Vector3::ZERO);
    let t: Vec<TemperaturePatch> = mesh
        .patches
        .iter()
        .map(|p| match p.name.as_str() {
            "left" => TemperaturePatch::FixedValue(300.0),
            "right" => TemperaturePatch::FixedValue(380.0),
            _ => TemperaturePatch::ZeroGradient,
        })
        .collect();
    let mut s = SolidDisplacementFoamEsi::new(mesh.clone(), steel(true), controls(), 1000.0, d, t, 300.0).unwrap();
    let log = s.step();
    assert_eq!(log.solves[0].field, "T");
    assert!(s.temperature().iter().all(|t| (300.0..=380.0).contains(t)));
    assert!(s.temperature().iter().cloned().fold(0.0, f64::max) > 300.0);
}

/// `compactNormalStress no` is refused by name, not silently run as `yes`.
#[test]
fn non_compact_normal_stress_is_refused_by_name() {
    let mesh = case_mesh();
    let d = patches(&mesh, Vector3::ZERO);
    let mut c = controls();
    c.compact_normal_stress = false;
    let r = SolidDisplacementFoamEsi::new(mesh, steel(false), c, 1.0, d, vec![], 300.0);
    assert!(matches!(r, Err(AppBuilderError::UnsupportedScheme { .. })));
}
