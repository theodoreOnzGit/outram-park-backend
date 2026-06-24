use openmc_libs::rng::{lcg::prn, distributions::sample_normal};

fn diffusion_coefficient(v: f64, lambda: f64) -> f64 {
    v * lambda / 3.0
}

fn per_component_variance_time(d: f64, t: f64) -> f64 {
    2.0 * d * t
}

/// Sample a unit direction uniformly on S^2 (isotropic).
fn sample_unit_vector(seed: &mut u64) -> [f64; 3] {
    let u: f64 = prn(seed) * 2.0 - 1.0;
    let phi: f64 = prn(seed) * 2.0 * std::f64::consts::PI;
    let rxy = (1.0 - u * u).sqrt();
    [rxy * phi.cos(), rxy * phi.sin(), u]
}

/// Sample a 3D Gaussian displacement vector X ~ N(0, sigma2 * I3).
fn sample_gaussian_vector(seed: &mut u64, sigma2: f64) -> [f64; 3] {
    let s = sigma2.sqrt();
    [s * sample_normal(seed), s * sample_normal(seed), s * sample_normal(seed)]
}

/// Sample net distance R for a 3D Gaussian displacement (Maxwell distribution).
fn sample_maxwell_radius(seed: &mut u64, sigma: f64) -> f64 {
    let x = sample_normal(seed);
    let y = sample_normal(seed);
    let z = sample_normal(seed);
    sigma * (x * x + y * y + z * z).sqrt()
}

/// Sample the net displacement over a single time horizon t.
fn sample_displacement_over_time(
    seed: &mut u64,
    v: f64,
    lambda: f64,
    t: f64,
) -> ([f64; 3], (f64, [f64; 3])) {
    let d = diffusion_coefficient(v, lambda);
    let sigma2 = per_component_variance_time(d, t);

    let x = sample_gaussian_vector(seed, sigma2);

    let sigma = sigma2.sqrt();
    let r = sample_maxwell_radius(seed, sigma);
    let u = sample_unit_vector(seed);

    (x, (r, u))
}

/// Generate a trajectory on a time grid using Gaussian increments.
fn simulate_trajectory_time_grid(
    seed: &mut u64,
    times: &[f64],
    start: [f64; 3],
    v: f64,
    lambda: f64,
) -> Result<Vec<[f64; 3]>, String> {
    if times.is_empty() {
        return Err("times must be non-empty".into());
    }
    for w in times.windows(2) {
        if w[1] < w[0] {
            return Err("times must be monotonically non-decreasing".into());
        }
    }

    let d = diffusion_coefficient(v, lambda);
    let mut positions = Vec::with_capacity(times.len());
    let mut pos = start;
    positions.push(pos);

    for k in 1..times.len() {
        let dt = times[k] - times[k - 1];
        if dt < 0.0 {
            return Err("time grid must be non-decreasing".into());
        }
        let sigma2 = per_component_variance_time(d, dt);
        let inc = sample_gaussian_vector(seed, sigma2);
        pos = [pos[0] + inc[0], pos[1] + inc[1], pos[2] + inc[2]];
        positions.push(pos);
    }

    Ok(positions)
}

#[test]
fn isotropic_scattering_time_summation() -> Result<(), String> {
    let v = 1.0;
    let lambda = 1.5;
    let t = 10.0;

    let mut seed: u64 = 42;

    let (x_vec, (r, u)) = sample_displacement_over_time(&mut seed, v, lambda, t);
    println!("Single-horizon sample over t = {:.3}:", t);
    println!("  Vector displacement: [{:.4}, {:.4}, {:.4}]", x_vec[0], x_vec[1], x_vec[2]);
    println!("  Distance-direction: R = {:.4}, u = [{:.4}, {:.4}, {:.4}]", r, u[0], u[1], u[2]);
    let x_recon = [r * u[0], r * u[1], r * u[2]];
    println!("  Recon vector from (R,u): [{:.4}, {:.4}, {:.4}]", x_recon[0], x_recon[1], x_recon[2]);

    let start = [0.0, 0.0, 0.0];
    let times = vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0];
    let positions = simulate_trajectory_time_grid(&mut seed, &times, start, v, lambda)?;

    println!("\nTrajectory positions at specified times:");
    for (k, p) in positions.iter().enumerate() {
        println!("  t = {:>4.1}: [{:.4}, {:.4}, {:.4}]", times[k], p[0], p[1], p[2]);
    }

    let num_samples = 10000;
    let d = diffusion_coefficient(v, lambda);
    let sigma2_total = per_component_variance_time(d, t);
    let mut sum = [0.0; 3];
    for _ in 0..num_samples {
        let x = sample_gaussian_vector(&mut seed, sigma2_total);
        sum[0] += x[0];
        sum[1] += x[1];
        sum[2] += x[2];
    }
    println!("\nSample mean of endpoints over {} runs (should be ~0): [{:.4}, {:.4}, {:.4}]",
             num_samples, sum[0] / num_samples as f64, sum[1] / num_samples as f64, sum[2] / num_samples as f64);

    Ok(())
}
