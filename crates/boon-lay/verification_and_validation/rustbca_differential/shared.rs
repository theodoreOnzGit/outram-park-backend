// Shared by the upstream driver (RustBCA v3.0.0, `libRustBCA`) and by
// `tests/rustbca_upstream_differential.rs` (boon-lay's copy, `boon_lay::rustbca`).
// The parent module defines `rb` (the RustBCA crate or module), `Rng8` and
// `make_rng`. ONE file drives both codes, so a difference in output can only come
// from the code under test, never from the harness. Floats are written as the
// 16-hex-digit bit pattern of the f64 so that nothing is lost to printing.
// gh:#1426.
#![allow(non_snake_case)]

use super::{make_rng, rb, Rng8};

use rb::bca;
use rb::consts::{AMU, ANGSTROM, EV};
use rb::geometry::Mesh0D;
use rb::particle::Particle;
use rb::{BulkBindingModel, ElectronicStoppingMode, InteractionPotential, Options, ScatteringIntegral, SurfaceBindingModel};

use std::sync::atomic::{AtomicBool, Ordering};

/// When set, `cascade_lines` writes EVERY output particle and EVERY trajectory
/// entry (about 63 MB for the whole run) instead of the committed reduced form.
/// Used to localise a mismatch; the committed reference is the reduced form.
static FULL: AtomicBool = AtomicBool::new(false);

pub fn set_full(on: bool) {
    FULL.store(on, Ordering::Relaxed);
}

fn h(v: f64) -> String {
    format!("{:016x}", v.to_bits())
}

/// (name, Z, mass amu)
pub const PROJECTILES: [(&str, f64, f64); 3] = [("H", 1.0, 1.00794), ("He", 2.0, 4.002602), ("Ar", 18.0, 39.948)];

/// (name, Z, mass amu, Ec eV, Es eV, Eb eV, number density 1/A^3). Fixed inputs;
/// the comparison is code against code, so they need no physical justification
/// beyond being typical (cohesive energies for Es).
pub const TARGETS: [(&str, f64, f64, f64, f64, f64, f64); 3] = [
    ("W", 74.0, 183.84, 3.0, 8.79, 0.0, 0.06306),
    ("Si", 14.0, 28.0855, 2.0, 4.63, 0.0, 0.04996),
    ("C", 6.0, 12.011, 3.0, 7.37, 0.0, 0.1128),
];

pub const ENERGIES_EV: [f64; 3] = [100.0, 1000.0, 10000.0];
pub const IONS_PER_CASE: usize = 8;
pub const INCIDENCE_DEG: f64 = 45.0;
pub const SEED: u64 = 20261113;

pub fn potentials() -> Vec<(&'static str, InteractionPotential)> {
    vec![
        ("KR_C", InteractionPotential::KR_C),
        ("ZBL", InteractionPotential::ZBL),
        ("MOLIERE", InteractionPotential::MOLIERE),
        ("LENZ_JENSEN", InteractionPotential::LENZ_JENSEN),
        ("TRIDYN", InteractionPotential::TRIDYN),
    ]
}

pub fn integrals() -> Vec<(&'static str, ScatteringIntegral)> {
    vec![
        ("MW", ScatteringIntegral::MENDENHALL_WELLER),
        ("GM10", ScatteringIntegral::GAUSS_MEHLER { n_points: 10 }),
        ("GL", ScatteringIntegral::GAUSS_LEGENDRE),
        ("MAGIC", ScatteringIntegral::MAGIC),
    ]
}

fn mode_name(m: &ElectronicStoppingMode) -> &'static str {
    match m {
        ElectronicStoppingMode::INTERPOLATED => "INTERPOLATED",
        ElectronicStoppingMode::LOW_ENERGY_LOCAL => "LOW_ENERGY_LOCAL",
        ElectronicStoppingMode::LOW_ENERGY_NONLOCAL => "LOW_ENERGY_NONLOCAL",
        ElectronicStoppingMode::LOW_ENERGY_EQUIPARTITION => "LOW_ENERGY_EQUIPARTITION",
        _ => "OTHER",
    }
}

fn catch<F: FnOnce() -> Vec<String>>(f: F) -> Vec<String> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(v) => v,
        Err(_) => vec!["PANIC".to_string()],
    }
}

/// Layer 1: the deterministic collision kernel, no random numbers. One line per
/// (projectile, target, potential, integral, E, impact parameter):
/// `K <tag> theta psi psi_recoil recoil_energy asymptotic_deflection x0` or
/// `K <tag> ERR` / `K <tag> PANIC`.
pub fn kernel_lines() -> Vec<String> {
    let mut out = Vec::new();
    let impact_a = [0.01, 0.05, 0.1, 0.3, 0.7, 1.5];
    let energies = [1.0e2, 1.0e3, 1.0e4, 1.0e5];
    for &(pn, za, ma) in PROJECTILES.iter() {
        for &(tn, zb, mb, _ec, _es, _eb, _n) in TARGETS.iter() {
            for &(potn, pot) in potentials().iter() {
                for &(intn, si) in integrals().iter() {
                    for &e in energies.iter() {
                        for &p in impact_a.iter() {
                            let tag = format!("{pn} {tn} {potn} {intn} {e:e} {p:e}");
                            let mut options = Options::default_options(true);
                            options.interaction_potential = vec![vec![pot]];
                            options.scattering_integral = vec![vec![si]];
                            let p1 = Particle::new(ma * AMU, za, e * EV, 1.0 * EV, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, true, false, 0);
                            let p2 = Particle::new(mb * AMU, zb, 0.0, 1.0 * EV, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, false, false, 0);
                            let geom = bca::BinaryCollisionGeometry::new(0.0, p * ANGSTROM, 1.0 * ANGSTROM);
                            let lines = catch(|| match bca::calculate_binary_collision(&p1, &p2, &geom, &options) {
                                Ok(r) => vec![format!(
                                    "K {tag} {} {} {} {} {} {}",
                                    h(r.theta),
                                    h(r.psi),
                                    h(r.psi_recoil),
                                    h(r.recoil_energy),
                                    h(r.asymptotic_deflection),
                                    h(r.normalized_distance_of_closest_approach)
                                )],
                                Err(_) => vec![format!("K {tag} ERR")],
                            });
                            for l in lines {
                                if l.starts_with("K ") {
                                    out.push(l);
                                } else {
                                    out.push(format!("K {tag} {l}"));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

/// Layer 2: the pure functions the kernel is built from: screening lengths,
/// interaction potentials, Lindhard-Scharff and Bethe-Bloch stopping.
pub fn function_lines() -> Vec<String> {
    let mut out = Vec::new();
    let zs = [1.0, 2.0, 6.0, 14.0, 18.0, 26.0, 74.0, 92.0];
    for &(potn, pot) in potentials().iter() {
        for &za in zs.iter() {
            for &zb in zs.iter() {
                let a = rb::interactions::screening_length(za, zb, pot);
                let mut line = format!("F SCREEN {potn} {za} {zb} {}", h(a));
                for &r in [0.05, 0.2, 0.5, 1.0, 3.0].iter() {
                    let v = rb::interactions::interaction_potential(r * ANGSTROM, a, za, zb, pot);
                    line.push_str(&format!(" {}", h(v)));
                }
                out.push(line);
            }
        }
    }
    for &xi in [1.0e-3, 1.0e-2, 0.1, 0.5, 1.0, 3.0, 10.0].iter() {
        for &(potn, pot) in potentials().iter() {
            out.push(format!("F PHI {potn} {xi:e} {} {}", h(rb::interactions::phi(xi, pot)), h(rb::interactions::dphi(xi, pot))));
        }
    }
    for &(pn, za, ma) in PROJECTILES.iter() {
        for &(tn, zb, _mb, _ec, _es, _eb, _n) in TARGETS.iter() {
            for &e in [1.0e1, 1.0e2, 1.0e3, 1.0e4, 1.0e5, 1.0e6, 1.0e7].iter() {
                let ls = rb::material::lindhard_scharff_stopping_power_cross_section(za, zb, e * EV, ma * AMU);
                let bb = rb::material::bethe_bloch_stopping_power_cross_section(za, zb, e * EV, ma * AMU);
                out.push(format!("F STOP {pn} {tn} {e:e} {} {}", h(ls), h(bb)));
            }
        }
    }
    out
}

fn one_cascade_lines(case: usize, pi: usize, ti: usize, e: f64, mode: ElectronicStoppingMode) -> Vec<String> {
    let (pn, z1, m1) = PROJECTILES[pi];
    let (tn, z2, m2, ec2, es2, eb2, n2) = TARGETS[ti];
    let mut out = Vec::new();
    out.push(format!("H {case} {pn} {tn} {e:e} {}", mode_name(&mode)));
    let mut options = Options::default_options(true);
    options.electronic_stopping_mode = mode;
    options.track_trajectories = true;
    let material_parameters = rb::material::MaterialParameters {
        energy_unit: "EV".to_string(),
        mass_unit: "AMU".to_string(),
        Eb: vec![eb2],
        Es: vec![es2],
        Ec: vec![ec2],
        Ed: vec![0.0],
        Z: vec![z2],
        m: vec![m2],
        interaction_index: vec![0],
        surface_binding_model: SurfaceBindingModel::AVERAGE,
        bulk_binding_model: BulkBindingModel::INDIVIDUAL,
    };
    let geometry_input = rb::geometry::Mesh0DInput {
        length_unit: "ANGSTROM".to_string(),
        densities: vec![n2],
        electronic_stopping_correction_factor: 1.0,
    };
    let material = rb::material::Material::<Mesh0D>::new(&material_parameters, &geometry_input);
    let x0 = -2.0 * (n2 * 1.0e30).powf(-1.0 / 3.0);
    let th = INCIDENCE_DEG.to_radians();
    for ion in 0..IONS_PER_CASE {
        let mut rng: Rng8 = make_rng(SEED, (case * 1000 + ion) as u64);
        let p = Particle::new(m1 * AMU, z1, e * EV, 1.0 * EV, 0.0, 0.0, x0, 0.0, 0.0, th.cos(), th.sin(), 0.0, true, true, 0);
        let lines = catch(|| {
            let particles = bca::single_ion_bca(p, &material, &options, &mut rng);
            let mut v = vec![format!("N {case} {ion} {}", particles.len())];
            let mut digest: u64 = 0xcbf2_9ce4_8422_2325;
            for (k, q) in particles.iter().enumerate() {
                let flags = (q.incident as u8) | ((q.left as u8) << 1) | ((q.stopped as u8) << 2) | ((q.backreflected as u8) << 3);
                let line = format!(
                    "P {case} {ion} {k} {} {} {} {} {} {} {} {} {} {flags} {} {} {}",
                    h(q.Z),
                    h(q.m),
                    h(q.E),
                    h(q.pos.x),
                    h(q.pos.y),
                    h(q.pos.z),
                    h(q.dir.x),
                    h(q.dir.y),
                    h(q.dir.z),
                    q.number_collision_events,
                    q.trajectory.len(),
                    h(q.asymptotic_deflection)
                );
                // Every particle goes into the digest; only the incident ion and the
                // particles that left (reflected, sputtered) are written out in full,
                // which keeps the committed reference small (the full dump is ~63 MB).
                for b in line.bytes() {
                    digest ^= u64::from(b);
                    digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
                }
                if q.incident || q.left || FULL.load(Ordering::Relaxed) {
                    v.push(line);
                    // Collision-by-collision trajectory: digest of all entries, and the
                    // first and last five in full.
                    let n_t = q.trajectory.len();
                    let mut tdigest: u64 = 0xcbf2_9ce4_8422_2325;
                    for (j, t) in q.trajectory.iter().enumerate() {
                        let tl = format!("T {case} {ion} {k} {j} {} {} {} {}", h(t.E), h(t.x), h(t.y), h(t.z));
                        for b in tl.bytes() {
                            tdigest ^= u64::from(b);
                            tdigest = tdigest.wrapping_mul(0x0000_0100_0000_01b3);
                        }
                        if j < 5 || j + 5 >= n_t || FULL.load(Ordering::Relaxed) {
                            v.push(tl);
                        }
                    }
                    v.push(format!("U {case} {ion} {k} {n_t} {tdigest:016x}"));
                }
            }
            v.push(format!("D {case} {ion} {digest:016x}"));
            v
        });
        for l in lines {
            if l == "PANIC" {
                out.push(format!("N {case} {ion} PANIC"));
            } else {
                out.push(l);
            }
        }
    }
    out
}

/// Layer 3: seeded cascades. Same ChaCha8 seed and stream in both codes
/// (`seed_from_u64(SEED)` then `set_stream(case*1000 + ion)`, upstream's own
/// per-particle pattern in `physics_loop`). Every output particle, and the
/// incident ion's collision-by-collision trajectory (E, x, y, z).
pub fn cascade_lines() -> Vec<String> {
    let mut out = Vec::new();
    let mut case = 0usize;
    for pi in 0..PROJECTILES.len() {
        for ti in 0..TARGETS.len() {
            for &e in ENERGIES_EV.iter() {
                let mut modes = vec![ElectronicStoppingMode::LOW_ENERGY_NONLOCAL, ElectronicStoppingMode::INTERPOLATED];
                if e <= 1000.0 {
                    modes.push(ElectronicStoppingMode::LOW_ENERGY_LOCAL);
                    modes.push(ElectronicStoppingMode::LOW_ENERGY_EQUIPARTITION);
                }
                for mode in modes {
                    out.extend(one_cascade_lines(case, pi, ti, e, mode));
                    case += 1;
                }
            }
        }
    }
    out
}

pub fn all_lines() -> Vec<String> {
    let mut v = kernel_lines();
    v.extend(function_lines());
    v.extend(cascade_lines());
    v
}
