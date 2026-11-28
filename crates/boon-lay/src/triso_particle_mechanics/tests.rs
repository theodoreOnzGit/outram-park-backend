// SPDX-License-Identifier: AGPL-3.0-only
//! Verification of boon-lay particle mechanics against closed forms.
//!
//! **Inputs are synthetic.** Radii and elastic constants below are round
//! numbers chosen to look like a TRISO particle; none is cited and none is
//! a property of a real material. A verification test compares the FEM
//! with a closed form ON THE SAME INPUTS, so the inputs need no provenance,
//! and no number here may be quoted as a property of PyC or SiC. The BISON
//! 2013 laws (Eqs (10.1)–(10.6)) are used where a test is about them.
//!
//! Results are printed (run with `--nocapture`) and recorded, with the
//! methodology, in
//! `crates/boon-lay/verification_and_validation/triso_particle_mechanics/README.md`.

use super::analytic::LayeredLameSolution;
use super::geometry::{LayerSpec, ParticleMesh};
use super::history::{run_history, HistoryPoint, TrackedLayer};
use super::material::{
    pyc_creep_coefficient_bison2013, CreepLawSign, DimensionalChange, IrradiationCreep, LayerMaterial,
};
use super::mechanics::{Formulation, LoadStep, ParticleSolver, StepTemperature};
use super::thermal::{analytic_steady_temperature, solve_steady_conduction, ThermalLoad};
use super::weibull::{layer_failure_probability, WeibullParameters, WeibullStressMeasure};
use super::ParticleMechanicsError;
use petir::mathf::RealMath;
use uom::si::f64::{Length, Pressure, TemperatureCoefficient, ThermalConductivity, ThermodynamicTemperature, Volume};
use uom::si::length::micrometer;
use uom::si::pressure::gigapascal;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::volume::cubic_meter;

const UM: f64 = 1.0e-6;

fn um(x: f64) -> Length {
    Length::new::<micrometer>(x)
}

fn material(e_gpa: f64, nu: f64) -> LayerMaterial {
    match LayerMaterial::elastic(Pressure::new::<gigapascal>(e_gpa), nu) {
        Ok(m) => m,
        Err(err) => panic!("synthetic material is valid by construction: {err}"),
    }
}

fn layer(label: &str, a: f64, b: f64, n: usize, m: LayerMaterial) -> LayerSpec {
    match LayerSpec::new(label, um(a), um(b), n, m) {
        Ok(l) => l,
        Err(err) => panic!("synthetic layer is valid by construction: {err}"),
    }
}

fn mesh(layers: Vec<LayerSpec>) -> ParticleMesh {
    match ParticleMesh::new(layers) {
        Ok(m) => m,
        Err(err) => panic!("synthetic mesh is valid by construction: {err}"),
    }
}

fn t_ref() -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(300.0)
}

fn step(solver: &mut ParticleSolver, s: &LoadStep) {
    match solver.step(s) {
        Ok(()) => {}
        Err(err) => panic!("step failed: {err}"),
    }
}

fn static_step(fluence: f64, p_in: f64, p_out: f64, t: f64) -> LoadStep {
    LoadStep {
        fast_fluence_m2: fluence,
        inner_pressure_pa: p_in,
        outer_pressure_pa: p_out,
        temperature: StepTemperature::Uniform(t),
    }
}

/// Synthetic IPyC / SiC / OPyC with `n` elements per layer.
fn three_layers(n: usize, pyc: LayerMaterial, sic: LayerMaterial) -> ParticleMesh {
    mesh(vec![
        layer("IPyC", 200.0, 240.0, n, pyc),
        layer("SiC", 240.0, 275.0, n, sic),
        layer("OPyC", 275.0, 315.0, n, pyc),
    ])
}

fn order(e_coarse: f64, e_fine: f64) -> f64 {
    (e_coarse / e_fine).r_log2()
}

/// Max relative nodal-displacement error, max stress error over every Gauss
/// point, and max stress error over the element-midpoint Gauss points (the
/// middle point of the 3-point rule, `ξ = 0`), the stresses relative to
/// `scale_pa`, of `solver` against `exact`.
///
/// Two-node elements give a displacement error of O(h²) and a strain, so a
/// stress, error of O(h) at a general point, but O(h²) at the element
/// midpoint, where the derivative of linear interpolation is
/// superconvergent. Both are measured.
fn errors3(solver: &ParticleSolver, exact: &LayeredLameSolution, scale_pa: f64) -> (f64, f64, f64) {
    let (eu, es) = errors(solver, exact, scale_pa);
    let mut em: f64 = 0.0;
    for p in solver.points() {
        let mid = 0.5 * (solver.mesh().node_radius_m(p.element) + solver.mesh().node_radius_m(p.element + 1));
        if (p.radius_m - mid).abs() > 1e-9 * mid {
            continue;
        }
        let s = exact.stress_in_layer_pa(p.layer, p.radius_m);
        em = em.max((p.stress_pa[0] - s[0]).abs()).max((p.stress_pa[1] - s[1]).abs());
    }
    (eu, es, em / scale_pa)
}

/// Max relative nodal-displacement error and max Gauss-point stress error
/// (relative to `scale_pa`) of `solver` against `exact`.
fn errors(solver: &ParticleSolver, exact: &LayeredLameSolution, scale_pa: f64) -> (f64, f64) {
    let mut umax: f64 = 0.0;
    let mut eu: f64 = 0.0;
    for (i, r) in solver.mesh().node_radii_m().iter().enumerate() {
        let ue = exact.displacement_m(*r);
        umax = umax.max(ue.abs());
        eu = eu.max((solver.displacement_m()[i] - ue).abs());
    }
    let mut es: f64 = 0.0;
    for p in solver.points() {
        let s = exact.stress_in_layer_pa(p.layer, p.radius_m);
        es = es.max((p.stress_pa[0] - s[0]).abs()).max((p.stress_pa[1] - s[1]).abs());
    }
    (eu / umax, es / scale_pa)
}

/// **Lamé, one shell.** A thick sphere `a = 240 um`, `b = 275 um` under
/// `p_in = 10 MPa`, `p_out = 0.1 MPa`, against the classical closed form
/// written out independently here (not through [`LayeredLameSolution`]):
/// `σ_r = (p_a a³ − p_b b³)/(b³ − a³) − (p_a − p_b)a³b³/((b³ − a³)r³)`,
/// `σ_θ = (p_a a³ − p_b b³)/(b³ − a³) + (p_a − p_b)a³b³/(2(b³ − a³)r³)`.
/// Prediction written before the first run: displacement and Gauss-point
/// stress both second order. **The stress half was wrong** (first run,
/// 2026-11-28: stress order 0.97–0.99): two-node elements give O(h) strain
/// at a general point and O(h²) only at the element midpoint. The test now
/// checks displacement at second order, every Gauss point at first order and
/// the midpoints at second order.
#[test]
fn lame_single_shell_converges_at_second_order() {
    let (a, b, pa, pb) = (240.0 * UM, 275.0 * UM, 10.0e6, 0.1e6);
    let lame = |r: f64| -> [f64; 2] {
        let d = b * b * b - a * a * a;
        let c0 = (pa * a * a * a - pb * b * b * b) / d;
        let c1 = (pa - pb) * a * a * a * b * b * b / (d * r * r * r);
        [c0 - c1, c0 + 0.5 * c1]
    };
    let mut last = (0.0, 0.0, 0.0);
    for n in [4usize, 8, 16, 32, 64] {
        let m = mesh(vec![layer("SiC", 240.0, 275.0, n, material(400.0, 0.13))]);
        let exact = match LayeredLameSolution::solve(&m, 300.0, 0.0, &[[0.0, 0.0]], pa, pb) {
            Ok(s) => s,
            Err(err) => panic!("{err}"),
        };
        let mut s = ParticleSolver::new(m, t_ref());
        step(&mut s, &static_step(0.0, pa, pb, 300.0));
        // The layered closed form agrees with the classical one.
        for r in [a, 0.5 * (a + b), b] {
            let x = exact.stress_in_layer_pa(0, r);
            let y = lame(r);
            assert!((x[0] - y[0]).abs() < 1e-6 * pa && (x[1] - y[1]).abs() < 1e-6 * pa, "closed forms differ at r = {r}");
        }
        let mut es: f64 = 0.0;
        for p in s.points() {
            let y = lame(p.radius_m);
            es = es.max((p.stress_pa[0] - y[0]).abs()).max((p.stress_pa[1] - y[1]).abs());
        }
        let (eu, _, em) = errors3(&s, &exact, pa);
        let es = es / pa;
        if last.0 > 0.0 {
            println!(
                "lame n = {n:3}: u err {eu:.3e} (order {:.3}), sigma err/p_in all {es:.3e} (order {:.3}), midpoints {em:.3e} (order {:.3})",
                order(last.0, eu),
                order(last.1, es),
                order(last.2, em)
            );
        } else {
            println!("lame n = {n:3}: u err {eu:.3e}, sigma err/p_in all {es:.3e}, midpoints {em:.3e}");
        }
        if n == 64 {
            assert!(order(last.0, eu) > 1.9 && eu < 1e-5, "displacement {eu}, order {}", order(last.0, eu));
            assert!(order(last.1, es) > 0.9, "stress order {}", order(last.1, es));
            assert!(order(last.2, em) > 1.9, "midpoint stress order {}", order(last.2, em));
        }
        last = (eu, es, em);
    }
}

/// **Three bonded layers, isotropic eigenstrain (thermal mismatch) plus
/// pressure,** against [`LayeredLameSolution`]. Different `α` per layer,
/// heated 300 K -> 1300 K, `p_in = 20 MPa`.
#[test]
fn three_layers_with_thermal_mismatch_match_the_closed_form() {
    let pyc = material(30.0, 0.23).with_thermal_expansion(TemperatureCoefficient::new::<per_kelvin>(5.5e-6));
    let sic = material(400.0, 0.13).with_thermal_expansion(TemperatureCoefficient::new::<per_kelvin>(4.9e-6));
    let mut last = (0.0, 0.0, 0.0);
    for n in [8usize, 16, 32, 64] {
        let m = three_layers(n, pyc, sic);
        let eig: Vec<[f64; 2]> = m
            .layers()
            .iter()
            .map(|l| l.material.thermal_strain_at(1300.0, 300.0))
            .collect();
        let exact = match LayeredLameSolution::solve(&m, 300.0, 0.0, &eig, 20.0e6, 0.0) {
            Ok(s) => s,
            Err(err) => panic!("{err}"),
        };
        let mut s = ParticleSolver::new(m, t_ref());
        step(&mut s, &static_step(0.0, 20.0e6, 0.0, 1300.0));
        let (eu, es, em) = errors3(&s, &exact, 20.0e6);
        println!("thermal mismatch n = {n:3}: u err {eu:.3e}, sigma err/p all {es:.3e}, midpoints {em:.3e}");
        if n == 64 {
            assert!(order(last.0, eu) > 1.9 && eu < 1e-5, "u err {eu}, order {}", order(last.0, eu));
            assert!(order(last.1, es) > 0.9, "stress order {}", order(last.1, es));
            assert!(order(last.2, em) > 1.9, "midpoint stress order {}", order(last.2, em));
        }
        last = (eu, es, em);
    }
}

/// **Anisotropic irradiation strain: the `C r ln r` term.** BISON 2013
/// dense-PyC strain (Eqs (10.2)–(10.3)) on both PyC layers at
/// `Φ = 2·10²⁵ n/m²`, no creep, no pressure. The radial and tangential
/// strains differ, so the closed form carries `C r ln r`; a missed `C`
/// would leave an O(1) error that does not converge.
#[test]
fn anisotropic_pyc_strain_matches_the_r_ln_r_closed_form() {
    let pyc = material(30.0, 0.23).with_dimensional_change(DimensionalChange::Bison2013DensePyc);
    let sic = material(400.0, 0.13);
    let phi = 2.0e25;
    let mut last = (0.0, 0.0, 0.0);
    for n in [8usize, 16, 32, 64] {
        let m = three_layers(n, pyc, sic);
        let eig: Vec<[f64; 2]> = m.layers().iter().map(|l| l.material.dimensional_change.strain_at(phi)).collect();
        let exact = match LayeredLameSolution::solve(&m, 300.0, 0.0, &eig, 0.0, 0.0) {
            Ok(s) => s,
            Err(err) => panic!("{err}"),
        };
        let mut s = ParticleSolver::new(m, t_ref());
        step(&mut s, &static_step(phi, 0.0, 0.0, 300.0));
        let scale = 400.0e9 * 1e-3;
        let (eu, es, em) = errors3(&s, &exact, scale);
        println!("anisotropic PyC strain n = {n:3}: u err {eu:.3e}, sigma err/(E_SiC 1e-3) all {es:.3e}, midpoints {em:.3e}");
        if n == 64 {
            assert!(order(last.0, eu) > 1.9 && eu < 1e-5, "u err {eu}, order {}", order(last.0, eu));
            assert!(order(last.1, es) > 0.9, "stress order {}", order(last.1, es));
            assert!(order(last.2, em) > 1.9, "midpoint stress order {}", order(last.2, em));
        }
        last = (eu, es, em);
    }
}

/// **A solid sphere under outer pressure is in uniform hydrostatic
/// compression,** `σ_r = σ_θ = −p`, `u = −p(1−2ν)r/E`. Linear in `r`, so the
/// two-node elements represent it exactly: agreement to solver tolerance.
#[test]
fn solid_sphere_is_uniformly_compressed() {
    let e = 200.0e9;
    let nu = 0.3;
    let m = mesh(vec![layer("kernel", 0.0, 250.0, 10, material(200.0, nu))]);
    let mut s = ParticleSolver::new(m, t_ref());
    step(&mut s, &static_step(0.0, 0.0, 5.0e6, 300.0));
    for p in s.points() {
        assert!((p.stress_pa[0] + 5.0e6).abs() < 1e-6 * 5.0e6 && (p.stress_pa[1] + 5.0e6).abs() < 1e-6 * 5.0e6);
    }
    let b = 250.0 * UM;
    let ub = -5.0e6 * (1.0 - 2.0 * nu) * b / e;
    let got = s.displacement_m()[s.mesh().n_nodes() - 1];
    println!("solid sphere: u(b) = {got:.6e} m vs {ub:.6e} m");
    assert!((got - ub).abs() < 1e-8 * ub.abs());
}

/// **A free layer with a uniform isotropic eigenstrain is stress-free.**
#[test]
fn free_isotropic_eigenstrain_is_stress_free() {
    let mat = material(30.0, 0.23).with_dimensional_change(DimensionalChange::Bison2013Buffer);
    let m = mesh(vec![layer("buffer", 100.0, 200.0, 20, mat)]);
    let mut s = ParticleSolver::new(m, t_ref());
    step(&mut s, &static_step(1.0e25, 0.0, 0.0, 300.0));
    let eps = DimensionalChange::Bison2013Buffer.strain_at(1.0e25)[0];
    let scale = 30.0e9 * eps.abs();
    for p in s.points() {
        assert!(p.stress_pa[0].abs() < 1e-8 * scale && p.stress_pa[1].abs() < 1e-8 * scale, "{:?}", p.stress_pa);
    }
    let b = 200.0 * UM;
    assert!((s.displacement_m()[s.mesh().n_nodes() - 1] - eps * b).abs() < 1e-9 * (eps * b).abs());
}

/// **Volume-conserving creep under constant pressure leaves the stress at
/// the elastic one and adds `u_cr = 3KΦ(p_in − p_out)a³b³/(4(b³ − a³)r²)`.**
///
/// With `ν_c = 0.5` creep is incompressible, and the Lamé stresses of a
/// pressurised sphere do not depend on the elastic constants, so the creep
/// strain from them is compatible and the stress never changes. The creep
/// displacement is the incompressible elastic one with `G = 1/(3KΦ)`. Exact
/// in time for any step count; checked at 1 step and 20 steps.
///
/// **This test found volumetric locking** (2026-11-28): with full
/// integration the outer Gauss points' stress moved by 32 % of `p` (the
/// midpoints by 2.3e-4). The default is now mean dilatation (module docs of
/// [`super::mechanics`]); measured with it: stress drift 1.25e-7 of `p`,
/// creep displacement error 3.8e-7. The full-integration ablation is
/// [`full_integration_locks_under_incompressible_creep`].
#[test]
fn incompressible_creep_keeps_the_lame_stress() {
    let k = 1.0e-35; // (Pa n/m^2)^-1, synthetic
    let creep = IrradiationCreep::Linear {
        coefficient_per_pa_per_fluence: k,
        creep_poisson_ratio: 0.5,
        sign: CreepLawSign::VolumeConserving,
    };
    let (a, b, pa) = (240.0 * UM, 275.0 * UM, 10.0e6);
    let phi = 3.0e25;
    let mut finals = Vec::new();
    for n_steps in [1usize, 20] {
        let m = mesh(vec![layer("SiC", 240.0, 275.0, 128, material(400.0, 0.13).with_irradiation_creep(creep))]);
        let mut s = ParticleSolver::new(m, t_ref());
        step(&mut s, &static_step(0.0, pa, 0.0, 300.0));
        let elastic_u = s.displacement_m().to_vec();
        let elastic_sigma: Vec<[f64; 2]> = s.points().iter().map(|p| p.stress_pa).collect();
        for i in 1..=n_steps {
            step(&mut s, &static_step(phi * i as f64 / n_steps as f64, pa, 0.0, 300.0));
        }
        let mut ds: f64 = 0.0;
        let mut dm: f64 = 0.0;
        for (i, (p, e)) in s.points().iter().zip(&elastic_sigma).enumerate() {
            let d = (p.stress_pa[0] - e[0]).abs().max((p.stress_pa[1] - e[1]).abs());
            ds = ds.max(d);
            if i % 3 == 1 {
                dm = dm.max(d);
            }
        }
        println!("incompressible creep, {n_steps:2} steps: midpoint drift/p {:.3e}", dm / pa);
        let d = b * b * b - a * a * a;
        let mut eu: f64 = 0.0;
        let mut umax: f64 = 0.0;
        for (i, r) in s.mesh().node_radii_m().iter().enumerate() {
            let ucr = 3.0 * k * phi * pa * a * a * a * b * b * b / (4.0 * d * r * r);
            umax = umax.max(ucr);
            eu = eu.max((s.displacement_m()[i] - elastic_u[i] - ucr).abs());
        }
        println!("incompressible creep, {n_steps:2} steps: stress drift/p {:.3e}, creep u err {:.3e}", ds / pa, eu / umax);
        assert!(ds / pa < 1e-4, "stress drifted {}", ds / pa);
        assert!(eu / umax < 1e-4, "creep displacement err {}", eu / umax);
        finals.push(s.displacement_m()[0]);
    }
    let rel = (finals[0] - finals[1]).abs() / finals[1].abs();
    println!("incompressible creep: 1 step vs 20 steps, rel diff in u(a) {rel:.3e}");
    assert!(rel < 1e-6);
}

/// **The printed sign of Eq (10.4) creeps in volume.** A shell with equal
/// inner and outer pressure is in uniform hydrostatic compression. With the
/// volume-conserving sign the volumetric creep strain is zero; with the sign
/// as printed (`+ν_c`) it is `−6KΦp` (`ε_r = ε_θ = −2KΦp`).
#[test]
fn the_printed_creep_sign_creeps_in_volume() {
    let p = 10.0e6;
    let phi = 1.0e25;
    let mut vol = Vec::new();
    for sign in [CreepLawSign::VolumeConserving, CreepLawSign::AsPrinted] {
        let creep = IrradiationCreep::Bison2013Pyc { density_g_per_cm3: 1.9, sign };
        let m = mesh(vec![layer("IPyC", 200.0, 240.0, 16, material(30.0, 0.23).with_irradiation_creep(creep))]);
        let mut s = ParticleSolver::new(m, t_ref());
        step(&mut s, &static_step(0.0, p, p, 1273.15));
        step(&mut s, &static_step(phi, p, p, 1273.15));
        let c = s.points()[0].creep_strain;
        vol.push(c[0] + 2.0 * c[1]);
    }
    let k = pyc_creep_coefficient_bison2013(1000.0, 1.9);
    println!("hydrostatic creep volume strain: volume-conserving {:.3e}, as printed {:.3e} (-6 K phi p = {:.3e})", vol[0], vol[1], -6.0 * k * phi * p);
    assert!(vol[0].abs() < 1e-12);
    assert!((vol[1] + 6.0 * k * phi * p).abs() < 1e-6 * (6.0 * k * phi * p));
}

/// **Transcription of Eqs (10.5)–(10.6).** At 1000 °C, `K₀ = 1.996e-29 −
/// 4.415e-29 + 3.6544e-29 = 1.2354e-29`; with `ρ = 1.9` and `M = 2`,
/// `K = 2.4708e-29` per MPa = `2.4708e-35` per Pa. (The first draft of
/// this test wrote `1.23944e-29`, an arithmetic slip in the hand sum; the
/// code was right and the expected value was corrected, 2026-11-28.) At `ρ = 1.8` the
/// density factor is `1 + 2.38·0.1 = 1.238`.
#[test]
fn pyc_creep_coefficient_transcription() {
    let k = pyc_creep_coefficient_bison2013(1000.0, 1.9);
    assert!((k - 2.4708e-35).abs() < 1e-12 * 2.4708e-35, "{k}");
    let k18 = pyc_creep_coefficient_bison2013(1000.0, 1.8);
    assert!((k18 / k - 1.238).abs() < 1e-12);
}

/// **The closed-form strains are the integrals of the printed rates,**
/// Eqs (10.1)–(10.3), checked by composite Simpson over 0..5·10²⁵ n/m².
#[test]
fn dimensional_change_is_the_integral_of_the_printed_rate() {
    for law in [DimensionalChange::Bison2013Buffer, DimensionalChange::Bison2013DensePyc] {
        let phi_max = 5.0e25;
        let n = 2000;
        let h = phi_max / n as f64;
        let mut acc = [0.0; 2];
        for i in 0..=n {
            let w = if i == 0 || i == n { 1.0 } else if i % 2 == 1 { 4.0 } else { 2.0 };
            let r = law.rate_per_fluence(i as f64 * h);
            acc[0] += w * r[0];
            acc[1] += w * r[1];
        }
        let closed = law.strain_at(phi_max);
        for j in 0..2 {
            let num = acc[j] * h / 3.0;
            assert!((num - closed[j]).abs() < 1e-10 * closed[j].abs(), "{law:?}[{j}]: {num} vs {}", closed[j]);
        }
        println!("{law:?} at 5e25 n/m^2: strain {:?}", closed);
    }
    // Spot values from the printed rates at zero fluence.
    let r0 = DimensionalChange::Bison2013DensePyc.rate_per_fluence(0.0);
    assert!((r0[0] * 1e25 - (-0.077 + 0.031)).abs() < 1e-15);
    assert!((r0[1] * 1e25 - (-0.036 - 0.01)).abs() < 1e-15);
    let b0 = DimensionalChange::Bison2013Buffer.rate_per_fluence(0.0);
    assert!((b0[0] * 1e25 + 0.176).abs() < 1e-15);
}

/// **Steady conduction through five layers against the closed form,** with
/// heat generated in the kernel only. Prediction: nodal error second order.
/// (The first draft also asserted the nodes were exact to 1e-9, by analogy
/// with Cartesian 1D Laplace, where linear elements are nodally exact. The
/// `r²` weight breaks that: measured orders 1.64, 1.71, 1.76 on the first
/// run, 2026-11-28. The exactness claim was wrong and is removed; the order
/// is checked, rising towards 2.)
#[test]
fn steady_conduction_matches_the_closed_form() {
    let k = |x: f64| ThermalConductivity::new::<watt_per_meter_kelvin>(x);
    let mut last = 0.0;
    for n in [4usize, 8, 16, 32, 64] {
        let m = mesh(vec![
            layer("kernel", 0.0, 250.0, n, material(200.0, 0.3).with_thermal_conductivity(k(3.0))),
            layer("buffer", 250.0, 350.0, n, material(20.0, 0.23).with_thermal_conductivity(k(0.5))),
            layer("IPyC", 350.0, 390.0, n, material(30.0, 0.23).with_thermal_conductivity(k(4.0))),
            layer("SiC", 390.0, 425.0, n, material(400.0, 0.13).with_thermal_conductivity(k(30.0))),
            layer("OPyC", 425.0, 465.0, n, material(30.0, 0.23).with_thermal_conductivity(k(4.0))),
        ]);
        let load = ThermalLoad {
            layer_power_density_w_per_m3: vec![3.0e9, 0.0, 0.0, 0.0, 0.0],
            outer_surface_temperature_k: 1273.15,
        };
        let t = match solve_steady_conduction(&m, &load) {
            Ok(t) => t,
            Err(err) => panic!("{err}"),
        };
        let mut e: f64 = 0.0;
        let mut rise: f64 = 0.0;
        for (i, r) in m.node_radii_m().iter().enumerate() {
            let te = match analytic_steady_temperature(&m, &load, *r) {
                Ok(v) => v,
                Err(err) => panic!("{err}"),
            };
            rise = rise.max(te - 1273.15);
            e = e.max((t[i] - te).abs());
        }
        if last > 0.0 {
            println!("conduction n = {n:2}/layer: centre rise {rise:.4} K, max nodal err {e:.3e} K (order {:.3})", order(last, e));
        } else {
            println!("conduction n = {n:2}/layer: centre rise {rise:.4} K, max nodal err {e:.3e} K");
        }
        if n == 64 {
            assert!(order(last, e) > 1.75 && e / rise < 2e-4, "nodal err {e} of rise {rise}, order {}", order(last, e));
        }
        last = e;
    }
}

/// **The Weibull integral converges to the closed form** of the Lamé
/// tangential stress of one SiC shell, `∫(σ_θ/σ_c)^m 4πr² dr` evaluated
/// independently by composite Simpson on 20 000 intervals.
#[test]
fn weibull_integral_converges_to_the_closed_form() {
    // 20 MPa keeps P_f near 1e-3: the first draft used 100 MPa, where P_f
    // saturates at 1 and the comparison checks nothing.
    let (a, b, pa) = (240.0 * UM, 275.0 * UM, 20.0e6);
    let (m_w, sc, vc) = (6.0, 350.0e6, 1.0e-12);
    let params = match WeibullParameters::new(m_w, Pressure::new::<gigapascal>(0.35), Volume::new::<cubic_meter>(vc)) {
        Ok(p) => p,
        Err(err) => panic!("{err}"),
    };
    let d = b * b * b - a * a * a;
    let sig_t = |r: f64| pa * a * a * a / d + pa * a * a * a * b * b * b / (2.0 * d * r * r * r);
    let nint = 20000;
    let h = (b - a) / nint as f64;
    let mut acc = 0.0;
    for i in 0..=nint {
        let r = a + i as f64 * h;
        let w = if i == 0 || i == nint { 1.0 } else if i % 2 == 1 { 4.0 } else { 2.0 };
        acc += w * (sig_t(r) / sc).r_powf(m_w) * 4.0 * core::f64::consts::PI * r * r;
    }
    let exact = 1.0 - (-(acc * h / 3.0) / vc).r_exp();
    let mut last = 0.0;
    for n in [4usize, 8, 16, 32, 64] {
        let mm = mesh(vec![layer("SiC", 240.0, 275.0, n, material(400.0, 0.13))]);
        let mut s = ParticleSolver::new(mm, t_ref());
        step(&mut s, &static_step(0.0, pa, 0.0, 300.0));
        let pf = match layer_failure_probability(&s, 0, &params, WeibullStressMeasure::MaximumPrincipal) {
            Ok(v) => v,
            Err(err) => panic!("{err}"),
        };
        let e = (pf - exact).abs() / exact;
        println!("weibull n = {n:2}: P_f {pf:.6e} vs {exact:.6e}, rel err {e:.3e}");
        assert!(pf > 1e-5 && pf < 0.1, "P_f {pf} must not saturate");
        if n == 64 {
            assert!(order(last, e) > 1.8 && e < 1e-4, "err {e}, order {}", order(last, e));
        }
        last = e;
        // The radial stress is compressive here, so independent action adds
        // nothing radially, but it counts sigma_theta twice (theta and phi).
        let pia = match layer_failure_probability(&s, 0, &params, WeibullStressMeasure::IndependentAction) {
            Ok(v) => v,
            Err(err) => panic!("{err}"),
        };
        assert!(pia > pf);
    }
}

/// **Temporal convergence of the implicit creep step on the full coupled
/// problem:** IPyC/SiC/OPyC with the BISON 2013 dense-PyC strain and creep
/// (volume-conserving sign), SiC creep at the 900 °C text value with a
/// synthetic `ν_c = 0.5`, 1273 K, a pressure ramp 0 -> 30 MPa over
/// 0 -> 3·10²⁵ n/m². Prediction: backward Euler is first order, so the
/// self-convergence ratio of the SiC's end-of-life peak tangential stress
/// is near 2 (order near 1).
#[test]
fn creep_step_converges_at_first_order_in_fluence() {
    let pyc = material(30.0, 0.23)
        .with_dimensional_change(DimensionalChange::Bison2013DensePyc)
        .with_irradiation_creep(IrradiationCreep::Bison2013Pyc { density_g_per_cm3: 1.9, sign: CreepLawSign::VolumeConserving });
    let sic = material(400.0, 0.13).with_irradiation_creep(IrradiationCreep::Linear {
        coefficient_per_pa_per_fluence: super::material::SIC_CREEP_COEFFICIENT_900C_PER_PA_PER_FLUENCE,
        creep_poisson_ratio: 0.5,
        sign: CreepLawSign::VolumeConserving,
    });
    let history = [
        HistoryPoint { fast_fluence_m2: 0.0, inner_pressure_pa: 0.0, outer_pressure_pa: 0.0, temperature_k: 1273.15 },
        HistoryPoint { fast_fluence_m2: 3.0e25, inner_pressure_pa: 30.0e6, outer_pressure_pa: 0.0, temperature_k: 1273.15 },
    ];
    let mut finals = Vec::new();
    for n_steps in [10usize, 20, 40, 80, 160] {
        let mut s = ParticleSolver::new(three_layers(16, pyc, sic), t_ref());
        let rec = match run_history(&mut s, &history, n_steps, &[]) {
            Ok(r) => r,
            Err(err) => panic!("{err}"),
        };
        finals.push(rec[rec.len() - 1].tangential_stress_range_pa[1][1]);
    }
    for i in 2..finals.len() {
        let ratio = (finals[i - 2] - finals[i - 1]) / (finals[i - 1] - finals[i]);
        println!("SiC end-of-life max sigma_theta: {:.6e} Pa, self-convergence ratio {ratio:.4} (order {:.3})", finals[i], ratio.r_log2());
        if i == finals.len() - 1 {
            assert!((ratio.r_log2() - 1.0).abs() < 0.1, "order {}", ratio.r_log2());
        }
    }
}

/// **Mechanism: shrinking PyC puts the SiC into compression.** With the
/// BISON 2013 dense-PyC strain (both PyC layers shrink tangentially at low
/// fluence, Eq (10.3)) and no pressure, the SiC's tangential stress must go
/// negative. The sign is predicted from the physics before running; the
/// magnitude rests on synthetic elastic constants and is not a result.
#[test]
fn shrinking_pyc_compresses_the_sic() {
    let pyc = material(30.0, 0.23)
        .with_dimensional_change(DimensionalChange::Bison2013DensePyc)
        .with_irradiation_creep(IrradiationCreep::Bison2013Pyc { density_g_per_cm3: 1.9, sign: CreepLawSign::VolumeConserving });
    let sic = material(400.0, 0.13);
    let mut s = ParticleSolver::new(three_layers(16, pyc, sic), t_ref());
    let history = [
        HistoryPoint { fast_fluence_m2: 0.0, inner_pressure_pa: 0.0, outer_pressure_pa: 0.0, temperature_k: 1273.15 },
        HistoryPoint { fast_fluence_m2: 0.5e25, inner_pressure_pa: 0.0, outer_pressure_pa: 0.0, temperature_k: 1273.15 },
    ];
    let rec = match run_history(&mut s, &history, 10, &[]) {
        Ok(r) => r,
        Err(err) => panic!("{err}"),
    };
    let sic_max = rec[rec.len() - 1].tangential_stress_range_pa[1][1];
    println!("SiC max sigma_theta after 0.5e25 n/m^2 of PyC shrinkage: {sic_max:.4e} Pa");
    assert!(sic_max < 0.0);
}

/// **Tracked failure probabilities are reported per step and lie in [0, 1].**
#[test]
fn history_tracks_failure_probability() {
    let sic = material(400.0, 0.13);
    let mut s = ParticleSolver::new(three_layers(8, material(30.0, 0.23), sic), t_ref());
    let weibull = match WeibullParameters::new(6.0, Pressure::new::<gigapascal>(0.35), Volume::new::<cubic_meter>(1.0e-12)) {
        Ok(p) => p,
        Err(err) => panic!("{err}"),
    };
    let tracked = [TrackedLayer { layer: 1, weibull, measure: WeibullStressMeasure::MaximumPrincipal }];
    let history = [
        HistoryPoint { fast_fluence_m2: 0.0, inner_pressure_pa: 0.0, outer_pressure_pa: 0.0, temperature_k: 300.0 },
        HistoryPoint { fast_fluence_m2: 1.0e25, inner_pressure_pa: 60.0e6, outer_pressure_pa: 0.0, temperature_k: 300.0 },
    ];
    let rec = match run_history(&mut s, &history, 5, &tracked) {
        Ok(r) => r,
        Err(err) => panic!("{err}"),
    };
    assert_eq!(rec.len(), 6);
    assert_eq!(rec[0].failure_probability[0], 0.0);
    for w in rec.windows(2) {
        assert!(w[1].failure_probability[0] >= w[0].failure_probability[0]);
    }
    assert!(rec[5].failure_probability[0] > 0.0 && rec[5].failure_probability[0] < 1.0);
}

/// **Bad inputs are refused, with the reason.**
#[test]
fn bad_inputs_are_refused() {
    assert!(matches!(LayerMaterial::elastic(Pressure::new::<gigapascal>(-1.0), 0.2), Err(ParticleMechanicsError::InvalidMaterial(_))));
    assert!(matches!(LayerMaterial::elastic(Pressure::new::<gigapascal>(1.0), 0.5), Err(ParticleMechanicsError::InvalidMaterial(_))));
    assert!(matches!(LayerSpec::new("x", um(2.0), um(1.0), 4, material(1.0, 0.2)), Err(ParticleMechanicsError::InvalidGeometry(_))));
    let gap = ParticleMesh::new(vec![layer("a", 1.0, 2.0, 2, material(1.0, 0.2)), layer("b", 2.5, 3.0, 2, material(1.0, 0.2))]);
    assert!(matches!(gap, Err(ParticleMechanicsError::InvalidGeometry(_))));
    let mut s = ParticleSolver::new(mesh(vec![layer("a", 1.0, 2.0, 2, material(1.0, 0.2))]), t_ref());
    step(&mut s, &static_step(1.0e25, 0.0, 0.0, 300.0));
    assert!(matches!(s.step(&static_step(0.5e25, 0.0, 0.0, 300.0)), Err(ParticleMechanicsError::InvalidLoad(_))));
    assert!(matches!(s.step(&static_step(2.0e25, 0.0, 0.0, -1.0)), Err(ParticleMechanicsError::InvalidLoad(_))));
    assert!(matches!(s.layer_stress_range(3), Err(ParticleMechanicsError::NoSuchLayer(3))));
    let load = ThermalLoad { layer_power_density_w_per_m3: vec![0.0], outer_surface_temperature_k: 300.0 };
    assert!(matches!(solve_steady_conduction(s.mesh(), &load), Err(ParticleMechanicsError::InvalidMaterial(_))));
}

/// **Correct numerics is the default: mean dilatation.** Constructed through
/// the ordinary path, the solver uses [`Formulation::MeanDilatation`]
/// (`crates/outram-mc-libs/tests/correct_physics_is_default.rs` is the
/// pattern).
#[test]
fn mean_dilatation_is_the_default() {
    let s = ParticleSolver::new(mesh(vec![layer("SiC", 240.0, 275.0, 4, material(400.0, 0.13))]), t_ref());
    assert_eq!(s.formulation(), Formulation::MeanDilatation);
}

/// **Ablation: full integration locks.** The case of
/// [`incompressible_creep_keeps_the_lame_stress`] with
/// [`Formulation::FullIntegration`]: the Gauss-point stress drifts by tens of
/// percent of `p` (32 % on 2026-11-28), which is why it is not the default.
#[test]
fn full_integration_locks_under_incompressible_creep() {
    let creep = IrradiationCreep::Linear {
        coefficient_per_pa_per_fluence: 1.0e-35,
        creep_poisson_ratio: 0.5,
        sign: CreepLawSign::VolumeConserving,
    };
    let pa = 10.0e6;
    let m = mesh(vec![layer("SiC", 240.0, 275.0, 128, material(400.0, 0.13).with_irradiation_creep(creep))]);
    let mut s = ParticleSolver::new(m, t_ref()).with_formulation(Formulation::FullIntegration);
    step(&mut s, &static_step(0.0, pa, 0.0, 300.0));
    let before: Vec<[f64; 2]> = s.points().iter().map(|p| p.stress_pa).collect();
    step(&mut s, &static_step(3.0e25, pa, 0.0, 300.0));
    let mut ds: f64 = 0.0;
    for (p, e) in s.points().iter().zip(&before) {
        ds = ds.max((p.stress_pa[0] - e[0]).abs()).max((p.stress_pa[1] - e[1]).abs());
    }
    println!("full integration, incompressible creep: stress drift/p {:.3e}", ds / pa);
    assert!(ds / pa > 0.1, "expected locking, drift only {}", ds / pa);
}

/// **The BISON Theory Manual's Section 10 equations, point by point,**
/// against values worked by hand from the printed coefficients. The manual
/// prints no figure, table or test data for its TRISO laws (searched
/// 2026-11-28: its figures and tables cover UO2/MOX, the clad, power and the
/// gap only), so a hand evaluation of each printed formula is the check
/// available. Hand values:
///
/// - Eq (10.1) at `Φ = 1` (10²⁵ n/m²): `−0.176 e^(−1.75) = −0.0305842`.
/// - Eq (10.2) at `Φ = 1`: `−0.077 e^(−1) + 0.031 = +0.0026733`.
/// - Eq (10.3) at `Φ = 1`: `−0.036 e^(−2.1) − 0.01 = −0.0144084`.
/// - Eq (10.6) at 600 °C: `1.996e-29 − 2.649e-29 + 1.315584e-29 =
///   6.62584e-30`; Eq (10.5) with `ρ = 1.9`, `M = 2`: `1.325168e-29` per
///   MPa, `1.325168e-35` per Pa.
/// - Section 10.2: `K = 2e-37` (640 °C) and `4e-37` (900 °C) (Pa·n/m²)⁻¹.
/// - Section 10.1: `ν_c = 0.5`, `M_Irr,Creep = 2`.
#[test]
fn bison_manual_section_10_hand_values() {
    let phi1 = 1.0e25;
    let b = DimensionalChange::Bison2013Buffer.rate_per_fluence(phi1);
    assert!((b[0] * 1e25 - (-0.0305842)).abs() < 1e-7 && b[0] == b[1], "{b:?}");
    let p = DimensionalChange::Bison2013DensePyc.rate_per_fluence(phi1);
    assert!((p[0] * 1e25 - 0.0026733).abs() < 1e-7, "{p:?}");
    assert!((p[1] * 1e25 - (-0.0144084)).abs() < 1e-7, "{p:?}");
    let k600 = pyc_creep_coefficient_bison2013(600.0, 1.9);
    assert!((k600 - 1.325168e-35).abs() < 1e-9 * 1.325168e-35, "{k600}");
    assert_eq!(super::material::SIC_CREEP_COEFFICIENT_640C_PER_PA_PER_FLUENCE, 2.0e-37);
    assert_eq!(super::material::SIC_CREEP_COEFFICIENT_900C_PER_PA_PER_FLUENCE, 4.0e-37);
    assert_eq!(super::material::PYC_CREEP_POISSON_RATIO, 0.5);
    assert_eq!(super::material::PYC_CREEP_MULTIPLIER, 2.0);
    // The Bison2013Pyc law evaluates K at the local temperature, in kelvin.
    let law = IrradiationCreep::Bison2013Pyc { density_g_per_cm3: 1.9, sign: CreepLawSign::VolumeConserving };
    let (k, nu) = law.coefficient_and_ratio(873.15);
    assert!((k - 1.325168e-35).abs() < 1e-9 * 1.325168e-35 && nu == 0.5);
    let printed = IrradiationCreep::Bison2013Pyc { density_g_per_cm3: 1.9, sign: CreepLawSign::AsPrinted };
    assert_eq!(printed.coefficient_and_ratio(873.15).1, -0.5);
}

// ---- outram-park-fork-offbeat correlations (dependency added 2026-11-28) ----

use outram_park_fork_offbeat::materials::properties::conductivity::ConductivityModel;
use outram_park_fork_offbeat::materials::properties::thermal_expansion::ThermalExpansionModel;
use outram_park_fork_offbeat::materials::properties::young_modulus::YoungModulusModel;

fn offbeat_material(model: YoungModulusModel, nu: f64) -> LayerMaterial {
    match LayerMaterial::offbeat_elastic(model, nu) {
        Ok(m) => m,
        Err(err) => panic!("valid by construction: {err}"),
    }
}

/// **An offbeat modulus is evaluated at the Gauss point's state.** A SiC
/// shell with PARFUME SiC `E(T)` (offbeat `YoungModulusModel::ParfumeSiC`,
/// 375 GPa at 940 °C) under 10 MPa at 1213.15 K, against the Lamé closed
/// form built with the same modulus at that temperature. The modulus read
/// through `LayerMaterial` equals offbeat's own `value`.
#[test]
fn offbeat_sic_modulus_drives_the_lame_solution() {
    let t = 1213.15;
    let mat = offbeat_material(YoungModulusModel::ParfumeSiC, 0.13);
    let direct = YoungModulusModel::ParfumeSiC.value(&super::material::offbeat_state(t, 0.0));
    assert_eq!(mat.youngs_modulus_at(t, 0.0), direct);
    assert!((direct - 375.0e9).abs() < 1.0, "PARFUME SiC E at 940 C is 375 GPa, got {direct}");
    let m = mesh(vec![layer("SiC", 240.0, 275.0, 64, mat)]);
    let exact = match LayeredLameSolution::solve(&m, t, 0.0, &[[0.0, 0.0]], 10.0e6, 0.0) {
        Ok(s) => s,
        Err(err) => panic!("{err}"),
    };
    let mut s = ParticleSolver::new(m, t_ref());
    step(&mut s, &static_step(0.0, 10.0e6, 0.0, t));
    let (eu, _) = errors(&s, &exact, 10.0e6);
    println!("offbeat PARFUME SiC E at {t} K: u err {eu:.3e}");
    assert!(eu < 1e-5);
}

/// **Hypoelastic (manual Eq (6.1)): a change of modulus alone changes
/// nothing.** Under a fixed 10 MPa, the SiC is heated from 300 K to 1600 K
/// with no thermal strain, so only the PARFUME modulus changes (428 -> 298.7
/// GPa, the table interpolated at 1326.85 °C). The stress and displacement
/// stay those of the first step. A total-strain (secant) model would instead
/// jump to the displacement of the softer shell, `E(300 K)/E(1600 K) = 1.4327`
/// times larger. (The first draft's comment guessed 252 GPa and 1.7; the
/// interpolation was not done by hand. Corrected 2026-11-28.)
#[test]
fn modulus_change_alone_keeps_the_state() {
    let mat = offbeat_material(YoungModulusModel::ParfumeSiC, 0.13);
    let m = mesh(vec![layer("SiC", 240.0, 275.0, 32, mat)]);
    let secant = match LayeredLameSolution::solve(&m, 1600.0, 0.0, &[[0.0, 0.0]], 10.0e6, 0.0) {
        Ok(s) => s,
        Err(err) => panic!("{err}"),
    };
    let mut s = ParticleSolver::new(m, t_ref());
    step(&mut s, &static_step(0.0, 10.0e6, 0.0, 300.0));
    let u1 = s.displacement_m().to_vec();
    let sig1: Vec<[f64; 2]> = s.points().iter().map(|p| p.stress_pa).collect();
    step(&mut s, &static_step(0.0, 10.0e6, 0.0, 1600.0));
    let du = s.displacement_m().iter().zip(&u1).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
    let ds = s.points().iter().zip(&sig1).map(|(p, q)| (p.stress_pa[0] - q[0]).abs().max((p.stress_pa[1] - q[1]).abs())).fold(0.0, f64::max);
    let ratio = secant.displacement_m(240.0 * UM) / u1[0];
    println!("modulus change only: |du| {du:.3e} m (u = {:.3e} m), |dsigma| {ds:.3e} Pa; secant would scale u by {ratio:.3}", u1[0]);
    // Round-off bounds, relative to the solution's own scale (the first
    // draft used an absolute 1e-6 Pa on a 1e7 Pa stress; measured ds was
    // 1.0e-5 Pa, i.e. 1e-12 of p, which is round-off, 2026-11-28).
    assert!(du < 1e-9 * u1[0].abs() && ds < 1e-9 * 10.0e6, "du {du}, ds {ds}");
    let e_ratio = YoungModulusModel::ParfumeSiC.value(&super::material::offbeat_state(300.0, 0.0))
        / YoungModulusModel::ParfumeSiC.value(&super::material::offbeat_state(1600.0, 0.0));
    assert!((ratio / e_ratio - 1.0).abs() < 1e-4, "secant ratio {ratio} vs E ratio {e_ratio}");
}

/// **Transversely isotropic PARFUME PyC thermal expansion** (offbeat
/// `ThermalExpansionModel::PARFUMEPyC`, BAF = 1.2, stress-free at 300 K),
/// heated to 1273.15 K in a bonded IPyC/SiC/OPyC stack. The PyC's radial and
/// tangential thermal strains differ, so the closed form carries the
/// `C r ln r` term; the eigenstrain is read from offbeat's
/// `principal_strains`.
#[test]
fn parfume_pyc_anisotropic_expansion_matches_the_closed_form() {
    let pyc = material(30.0, 0.23)
        .with_offbeat_thermal_expansion(ThermalExpansionModel::PARFUMEPyC { t_ref: 300.0, anisotropy_factor: 1.2 });
    let sic = material(400.0, 0.13)
        .with_offbeat_thermal_expansion(ThermalExpansionModel::PARFUMESiC { alpha: 4.9e-6, t_ref: 300.0 });
    let t = 1273.15;
    let e_pyc = pyc.thermal_strain_at(t, 300.0);
    assert!((e_pyc[0] - e_pyc[1]).abs() > 1e-5, "BAF 1.2 must be anisotropic: {e_pyc:?}");
    let mut last = 0.0;
    for n in [16usize, 32, 64] {
        let m = three_layers(n, pyc, sic);
        let eig: Vec<[f64; 2]> = m.layers().iter().map(|l| l.material.thermal_strain_at(t, 300.0)).collect();
        let exact = match LayeredLameSolution::solve(&m, t, 0.0, &eig, 0.0, 0.0) {
            Ok(s) => s,
            Err(err) => panic!("{err}"),
        };
        let mut s = ParticleSolver::new(m, t_ref());
        step(&mut s, &static_step(0.0, 0.0, 0.0, t));
        let (eu, _) = errors(&s, &exact, 1.0);
        println!("PARFUME PyC BAF 1.2 thermal n = {n}: strains {e_pyc:?}, u err {eu:.3e}");
        if n == 64 {
            assert!(eu < 1e-5 && order(last, eu) > 1.9, "u err {eu}, order {}", order(last, eu));
        }
        last = eu;
    }
}

/// **Conduction with PARFUME SiC `k(T) = 17885/T + 2`** (offbeat
/// `ConductivityModel::ParfumeSiC`) around a heated kernel, solved by Picard
/// iteration, against the Kirchhoff closed form: across the SiC
/// `17885 ln(T_a/T_b) + 2(T_a − T_b) = Q/(4π) (1/a − 1/b)` (solved for `T_a`
/// by Newton), then the kernel's parabola `T(0) = T_a + q a²/(6 k)`.
/// Prediction: nodal error second order. Measured (2026-11-28): the SiC
/// interface error falls 4.3e-5, 1.1e-5, 3e-6, 1e-6 K (8 to 64 elements per
/// layer); the centre error is the kernel's, 4.6e-4 of its 10.4 K rise at 64.
#[test]
fn picard_conduction_with_parfume_sic_matches_kirchhoff() {
    let (a, b, q, kk, tb) = (250.0 * UM, 285.0 * UM, 3.0e9, 3.0, 1273.15);
    let qtot = q * 4.0 / 3.0 * core::f64::consts::PI * a * a * a;
    let rhs = qtot / (4.0 * core::f64::consts::PI) * (1.0 / a - 1.0 / b);
    let mut ta = tb + 10.0;
    for _ in 0..50 {
        let g = 17885.0 * (ta / tb).r_ln() + 2.0 * (ta - tb) - rhs;
        let dg = 17885.0 / ta + 2.0;
        ta -= g / dg;
    }
    let t0 = ta + q * a * a / (6.0 * kk);
    let k = |x: f64| ThermalConductivity::new::<watt_per_meter_kelvin>(x);
    let mut last = 0.0;
    for n in [8usize, 16, 32, 64] {
        let m = mesh(vec![
            layer("kernel", 0.0, 250.0, n, material(200.0, 0.3).with_thermal_conductivity(k(kk))),
            layer("SiC", 250.0, 285.0, n, material(400.0, 0.13).with_offbeat_conductivity(ConductivityModel::ParfumeSiC)),
        ]);
        let load = ThermalLoad { layer_power_density_w_per_m3: vec![q, 0.0], outer_surface_temperature_k: tb };
        let t = match solve_steady_conduction(&m, &load) {
            Ok(t) => t,
            Err(err) => panic!("{err}"),
        };
        // The k(T) layer is judged at the SiC's inner surface; the centre
        // error belongs to the constant-k kernel and is measured against the
        // kernel's own rise q a^2/(6k) (first draft normalised it by the wrong
        // rise; corrected 2026-11-28).
        let e_sic = (t[n] - ta).abs();
        let e_ker = (t[0] - t0).abs() / (t0 - ta);
        println!("Picard k(T) n = {n}: T(a) {:.7} vs {ta:.7} K (err {e_sic:.3e} K), T(0) {:.5} vs {t0:.5} K (err/kernel rise {e_ker:.3e})", t[n], t[0]);
        if n == 64 {
            assert!(order(last, e_sic) > 1.8, "SiC err {e_sic}, order {}", order(last, e_sic));
            assert!(e_ker < 1e-3, "kernel err {e_ker}");
        }
        last = e_sic;
    }
    // The closed form refuses a k(T) layer rather than give a wrong answer.
    let m = mesh(vec![layer("SiC", 250.0, 285.0, 4, material(400.0, 0.13).with_offbeat_conductivity(ConductivityModel::ParfumeSiC))]);
    let load = ThermalLoad { layer_power_density_w_per_m3: vec![0.0], outer_surface_temperature_k: tb };
    assert!(matches!(analytic_steady_temperature(&m, &load, 260.0 * UM), Err(ParticleMechanicsError::InvalidMaterial(_))));
}
