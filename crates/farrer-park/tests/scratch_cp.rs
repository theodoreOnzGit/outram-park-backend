use farrer_park::crystal::*;
use farrer_park::material::*;
use farrer_park::tensor::Voigt6;
use outram_foam_basic_lib::compute::ComputeBackend;
use outram_foam_basic_lib::math::differentiate::{jacobian, DiffSettings};

fn copper(m: f64) -> CrystalPlasticity {
    CrystalPlasticity::new(
        CrystalElasticity::cubic(170.0e9, 124.0e9, 75.0e9).unwrap(),
        SlipFamily::FccOctahedral,
        PowerLawFlow::new(1.0e-3, m).unwrap(),
        SaturatingHardening::new(180.0e6, 148.0e6, 2.25, 1.0, 1.4).unwrap(),
        16.0e6,
        0.1,
    )
    .unwrap()
}

fn iso(m: f64) -> CrystalPlasticity {
    CrystalPlasticity::new(
        CrystalElasticity::Isotropic(LinearElastic::new(200.0e9, 0.3).unwrap()),
        SlipFamily::FccOctahedral,
        PowerLawFlow::new(1.0e-3, m).unwrap(),
        SaturatingHardening::new(180.0e6, 148.0e6, 2.25, 1.0, 1.4).unwrap(),
        16.0e6,
        0.1,
    )
    .unwrap()
}

/// Best single-slip axis: maximise mu_1 / mu_2.
#[test]
fn find_single_slip_axis() {
    let sys = SlipFamily::FccOctahedral.systems();
    let mut best = (0.0_f64, [0.0_f64; 3], 0.0, 0.0);
    let n = 120;
    for i in 0..=n {
        for j in 0..=n {
            let a = i as f64 / n as f64;
            let b = j as f64 / n as f64;
            let t = [a, b, 1.0];
            let mut mu: Vec<f64> = sys.iter().map(|s| s.schmid_factor(t)).collect();
            mu.sort_by(|x, y| y.partial_cmp(x).unwrap());
            if mu[1] < 1e-12 {
                continue;
            }
            let r = mu[0] / mu[1];
            if r > best.0 {
                best = (r, t, mu[0], mu[1]);
            }
        }
    }
    println!("best ratio {:.4} at t={:?} mu1={:.5} mu2={:.5}", best.0, best.1, best.2, best.3);
}

#[test]
fn rss_identity() {
    // Uniaxial stress S along sample x, isotropic elasticity so the elastic
    // strain is exact; check tau_a = S * (m.t)(n.t) through the update path.
    let cp = iso(0.02);
    let m = Material::CrystalPlasticity(cp);
    let o = Orientation::from_bunge_euler_degrees(31.0, 47.0, 13.0);
    let mut state = m.initial_state();
    state.crystal = state.crystal.with_orientation(o);
    let e = 200.0e9;
    let nu = 0.3;
    let s = 20.0e6; // well below yield (s0/0.5 = 32 MPa minimum)
    let eps = Voigt6::new(s / e, -nu * s / e, -nu * s / e, 0.0, 0.0, 0.0);
    let up = m.update(eps, &state, PlaneCondition::PlaneStrain).unwrap();
    println!("stress = {:?}", up.stress.as_array().map(|v| v / 1e6));
    let tau = state.crystal.resolved_shear_stresses(SlipFamily::FccOctahedral, &up.stress);
    // analytic: rotate the load axis into the crystal frame
    let t_crystal = o.inverse().rotate_vector([1.0, 0.0, 0.0]);
    let sys = SlipFamily::FccOctahedral.systems();
    let mut worst = 0.0_f64;
    for a in 0..12 {
        let mu = sys[a].direction.iter().zip(t_crystal.iter()).map(|(x, y)| x * y).sum::<f64>()
            * sys[a].normal.iter().zip(t_crystal.iter()).map(|(x, y)| x * y).sum::<f64>();
        let pred = up.stress.0[0] * mu;
        worst = worst.max((tau[a] - pred).abs());
        println!("a={a:2} tau={:.6} MPa  pred={:.6} MPa", tau[a] / 1e6, pred / 1e6);
    }
    println!("worst abs diff = {:.3e} Pa (relative {:.3e})", worst, worst / s);
}

#[test]
fn tangent_vs_numerical() {
    for (name, cp) in [("cubic", copper(0.02)), ("iso", iso(0.05))] {
        let m = Material::CrystalPlasticity(cp);
        let o = Orientation::from_bunge_euler_degrees(31.0, 47.0, 13.0);
        let mut state = m.initial_state();
        state.crystal = state.crystal.with_orientation(o);
        // warm it up plastically
        for i in 1..=10 {
            let e = 3.0e-4 * i as f64;
            let eps = Voigt6::new(e, -0.4 * e, -0.4 * e, 0.2 * e, 0.0, 0.3 * e);
            state = m.update(eps, &state, PlaneCondition::PlaneStrain).unwrap().state;
        }
        let eps = Voigt6::new(3.6e-3, -1.4e-3, -1.4e-3, 7.0e-4, 0.0, 1.1e-3);
        let analytic = m.update(eps, &state, PlaneCondition::PlaneStrain).unwrap();
        println!("{name}: yielding={} total_slip={:.4e}", analytic.yielding, analytic.state.crystal.total_slip);
      for (label, ds) in [
        ("central h0   ", DiffSettings::central()),
        ("central h0/2 ", DiffSettings { relative_step: DiffSettings::central().relative_step*0.5, ..DiffSettings::central() }),
        ("central h0/4 ", DiffSettings { relative_step: DiffSettings::central().relative_step*0.25, ..DiffSettings::central() }),
        ("central4th   ", DiffSettings::central_4th()),
      ] {
        let sol = jacobian(
            &eps.as_array(),
            ds,
            ComputeBackend::Serial,
            |_, v: &[f64], out: &mut Vec<f64>| {
                let e = Voigt6([v[0], v[1], v[2], v[3], v[4], v[5]]);
                let s = m.update(e, &state, PlaneCondition::PlaneStrain).unwrap().stress;
                out.extend_from_slice(&s.as_array());
            },
        );
        let num = sol.matrix().unwrap();
        let scale = analytic.tangent.abs_max();
        let mut worst = 0.0_f64;
        for i in 0..6 {
            for j in 0..6 {
                worst = worst.max((num.get(i, j) - analytic.tangent.get(i, j)).abs() / scale);
            }
        }
        println!("{name} {label}: tangent worst relative entry error = {worst:.4e}");
      }
    }
}

#[test]
fn frame_indifference() {
    let cp = copper(0.02);
    let m = Material::CrystalPlasticity(cp);
    let o = Orientation::from_bunge_euler_degrees(31.0, 47.0, 13.0);
    let q = Orientation::from_bunge_euler_degrees(115.0, 62.0, 200.0);
    let eps = Voigt6::new(2.0e-3, -8.0e-4, -6.0e-4, 5.0e-4, -3.0e-4, 9.0e-4);

    let mut sa = m.initial_state();
    sa.crystal = sa.crystal.with_orientation(o);
    let mut sb = m.initial_state();
    sb.crystal = sb.crystal.with_orientation(o.pre_rotated_by(&q));

    // eps' = Q eps Q^T (tensor form)
    let eps_t = Voigt6::new(eps.0[0], eps.0[1], eps.0[2], 0.5 * eps.0[3], 0.5 * eps.0[4], 0.5 * eps.0[5]);
    let epsr_t = q.rotate_symmetric(&eps_t);
    let epsr = Voigt6::new(epsr_t.0[0], epsr_t.0[1], epsr_t.0[2], 2.0 * epsr_t.0[3], 2.0 * epsr_t.0[4], 2.0 * epsr_t.0[5]);

    let ua = m.update(eps, &sa, PlaneCondition::PlaneStrain).unwrap();
    let ub = m.update(epsr, &sb, PlaneCondition::PlaneStrain).unwrap();
    let rotated = q.rotate_symmetric(&ua.stress);
    let d = rotated.minus(&ub.stress).abs_max();
    println!("frame indifference: |Q sigma Q^T - sigma'| = {:.4e} Pa, scale {:.4e} Pa, rel {:.3e}",
             d, ua.stress.abs_max(), d / ua.stress.abs_max());
    println!("slip a: {:?}", &ua.state.crystal.slip[..4]);
    println!("slip b: {:?}", &ub.state.crystal.slip[..4]);
}
