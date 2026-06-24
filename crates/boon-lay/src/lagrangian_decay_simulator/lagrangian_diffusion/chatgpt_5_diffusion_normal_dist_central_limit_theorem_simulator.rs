use openmc_libs::rng::{lcg::prn, distributions::sample_normal};

fn per_component_variance_from_m2(n: usize, e_s2: f64) -> f64 {
    (n as f64) * e_s2 / 3.0
}

fn per_component_variance_exponential(n: usize, lambda: f64) -> f64 {
    let e_s2 = 2.0 * lambda * lambda;
    per_component_variance_from_m2(n, e_s2)
}

/// Sample a 3D Gaussian displacement vector X ~ N(0, sigma2 * I3).
fn sample_gaussian_vector(seed: &mut u64, sigma2: f64) -> [f64; 3] {
    let s = sigma2.sqrt();
    [s * sample_normal(seed), s * sample_normal(seed), s * sample_normal(seed)]
}

/// Sample a unit direction uniformly on S^2.
fn sample_unit_vector(seed: &mut u64) -> [f64; 3] {
    let u: f64 = prn(seed) * 2.0 - 1.0;
    let phi: f64 = prn(seed) * 2.0 * std::f64::consts::PI;
    let rxy = (1.0 - u * u).sqrt();
    [rxy * phi.cos(), rxy * phi.sin(), u]
}

/// Sample net distance R for a 3D Gaussian displacement (Maxwell distribution).
fn sample_maxwell_radius(seed: &mut u64, sigma: f64) -> f64 {
    let x = sample_normal(seed);
    let y = sample_normal(seed);
    let z = sample_normal(seed);
    sigma * (x * x + y * y + z * z).sqrt()
}

/// Sample (distance, direction) for the Gaussian net displacement after n isotropic steps.
fn sample_distance_and_direction(seed: &mut u64, sigma2: f64) -> (f64, [f64; 3]) {
    let sigma = sigma2.sqrt();
    let r = sample_maxwell_radius(seed, sigma);
    let u = sample_unit_vector(seed);
    (r, u)
}

#[test]
fn diffusion_gaussian_sum() {
    let mut seed: u64 = 42;

    let n = 1000usize;
    let lambda = 1.0;
    let sigma2 = per_component_variance_exponential(n, lambda);

    println!("Gaussian vectors X ~ N(0, sigma2 I3), with sigma2 = {:.6}", sigma2);
    for i in 0..5 {
        let x = sample_gaussian_vector(&mut seed, sigma2);
        println!("vec #{i}: [{:.4}, {:.4}, {:.4}]", x[0], x[1], x[2]);
    }

    println!("\nDistance-direction samples (Maxwell distance, isotropic direction):");
    for i in 0..5 {
        let (r, u) = sample_distance_and_direction(&mut seed, sigma2);
        let x = [r * u[0], r * u[1], r * u[2]];
        println!("pair #{i}: R = {:.4}, u = [{:.4}, {:.4}, {:.4}], X = [{:.4}, {:.4}, {:.4}]",
                 r, u[0], u[1], u[2], x[0], x[1], x[2]);
    }
}
