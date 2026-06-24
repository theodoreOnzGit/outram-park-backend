use openmc_libs::rng::{lcg::prn, distributions::sample_exp};
use std::f64::consts::PI;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy)]
struct Vec3 {
    x: f64,
    y: f64,
    z: f64,
}

impl Vec3 {
    fn add(&self, other: Vec3) -> Vec3 {
        Vec3 { x: self.x + other.x, y: self.y + other.y, z: self.z + other.z }
    }

    fn scale(&self, s: f64) -> Vec3 {
        Vec3 { x: self.x * s, y: self.y * s, z: self.z * s }
    }

    fn norm(&self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    fn normalize(&self) -> Vec3 {
        let n = self.norm();
        if n == 0.0 {
            Vec3 { x: 1.0, y: 0.0, z: 0.0 }
        } else {
            self.scale(1.0 / n)
        }
    }
}

/// Sample an isotropic direction on the unit sphere.
/// mu = cos(theta) ~ U[-1, 1], phi ~ U[0, 2π]
fn sample_isotropic_direction(seed: &mut u64) -> Vec3 {
    let mu: f64 = prn(seed) * 2.0 - 1.0;
    let phi = prn(seed) * 2.0 * PI;
    let sin_theta: f64 = (1.0_f64 - mu * mu).sqrt();
    Vec3 {
        x: sin_theta * phi.cos(),
        y: sin_theta * phi.sin(),
        z: mu,
    }
}

/// Sample a free path length ℓ from Exp(sigma_s).
fn sample_free_path(seed: &mut u64, sigma_s: f64) -> f64 {
    sample_exp(seed, sigma_s)
}

#[derive(Debug)]
struct Particle {
    pos: Vec3,
    dir: Vec3,
}

impl Particle {
    fn new(initial_pos: Vec3, initial_dir: Option<Vec3>, seed: &mut u64) -> Self {
        let dir = match initial_dir {
            Some(d) => d.normalize(),
            None => sample_isotropic_direction(seed),
        };
        Particle { pos: initial_pos, dir }
    }
}

/// Run a simple collision-driven random walk.
fn simulate(
    sigma_s: f64,
    n_collisions: usize,
    initial_pos: Vec3,
    initial_dir: Option<Vec3>,
    seed: &mut u64,
) -> Vec<Vec3> {
    let mut particle = Particle::new(initial_pos, initial_dir, seed);
    let mut trajectory = Vec::with_capacity(n_collisions + 1);

    trajectory.push(particle.pos);

    for _ in 0..n_collisions {
        let ell = sample_free_path(seed, sigma_s);
        particle.pos = particle.pos.add(particle.dir.scale(ell));
        trajectory.push(particle.pos);
        particle.dir = sample_isotropic_direction(seed);
    }

    trajectory
}

#[test]
fn chat_gpt_sim() {
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let mut seed: u64 = t.subsec_nanos() as u64 ^ t.as_secs().wrapping_mul(0x9e3779b97f4a7c15);

    let sigma_s = 0.5_f64;
    let n_collisions = 1000;
    let initial_pos = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
    let initial_dir = None;

    let trajectory = simulate(sigma_s, n_collisions, initial_pos, initial_dir, &mut seed);

    for (i, p) in trajectory.iter().enumerate().take(10) {
        println!("step {:4}: ({:.6}, {:.6}, {:.6})", i, p.x, p.y, p.z);
    }
}
