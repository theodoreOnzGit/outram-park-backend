//! **NOT COMPILED (2026-10-20): waiting for gh:#979.** `main.rs` does not
//! declare this file as a module, so nothing here is built or run yet.
//!
//! The RustBCA tracks of pages A ("A fragment's flight") and B ("The
//! lattice's side"). boon-lay's RustBCA copy takes `&mut rand::rngs::ChaCha8Rng`,
//! which no crate outside boon-lay can construct (boon-lay imports `rand`
//! privately and no other workspace crate depends on it). The maintainer
//! chose to move the copy onto petir's LCG (gh:#979, after the lockstep
//! crates.io publish). This file is written against that API: wherever it
//! says `petir::rng::lcg::Lcg64`, #979's signature is assumed to be
//! `single_ion_bca(particle, &material, &options, &mut Lcg64)`; adjust the
//! one call in [`run_pair`] if #979 lands a different shape.
//!
//! **To switch it on once #979 lands:** add `#[path = "rustbca_tracks.rs"]
//! mod rustbca_tracks;` to `main.rs`, call `rustbca_tracks::write_tracks(&out_dir,
//! efr)` from `native::run`, add a test in `tests/lesson_fission_cascades.rs`
//! that reaches `run_pair` on a 100 keV ion (cheap), and run the generator.
//!
//! **What it will produce** (`cascade-tracks.js`), all precomputed because a
//! fragment cascade costs 1.5-2 s on one desktop core (boon-lay's
//! `verification_and_validation/rustbca_cascades/`, 2026-10-16):
//!
//! - Page A: two fragments from one fission (A = 95, Z = 38 and A = 139,
//!   Z = 54, EFR split by momentum, the pair `rustbca_cascades.rs` uses),
//!   born at one point in bulk UO2 and sent back to back: each primary's
//!   trajectory (`track_trajectories`), thinned to at most 400 points, its
//!   rest point, and the electronic and nuclear energy loss per step
//!   (`track_energy_losses`: `EnergyLoss { Ee, En }`) summed into 100 bins of
//!   path length; and the flight time, the sum over steps of step length
//!   over speed `sqrt(2E/m)` at the step's energy.
//! - Page B: the recoil tree of the hardest knock-on the light fragment made
//!   (the recoil with the largest `energy_origin` in its output), re-run as a
//!   primary with `track_recoil_trajectories`: every recoil's path, its birth
//!   energy, and whether that exceeds NJOY's default displacement threshold
//!   for its element.
//!
//! **Illustrative**, as boon-lay's CLAUDE.md requires: RustBCA's high-energy
//! electronic stopping uses the bare projectile charge, and no fragment range
//! has yet been checked against independent data.
//!
//! **Material:** bulk UO2 as one thick `Mesh1D` layer (no TRISO coatings: the
//! pages are about a fragment in fuel), the kernel composition of
//! `outram_mc_libs::pebble_beds::htr10::fuel_pebble_materials` and the
//! per-element `Es`/`Eb`/`Ec` of upstream RustBCA's `scripts/materials.py`
//! (`a356280d`), as `rustbca_cascades.rs` sets them. Options as there:
//! `default_options(true)`, `INTERPOLATED` electronic stopping, `ck = 1`.

use petir::mathf::RealMath;

use boon_lay::rustbca::{bca, material, particle, ElectronicStoppingMode, Mesh1D, Options, AMU, ANGSTROM, EV, MICRON};
use petir::rng::lcg::Lcg64;

/// Slab thickness [µm] and the birth depth (its middle): far more than any
/// recorded fragment range (under 17 µm), so nothing reaches a surface.
const SLAB_UM: f64 = 200.0;

/// UO2 at the HTR-10 kernel's density: species (Z, mass u, Es, Eb, Ec in eV)
/// and atom densities [atoms/Å³]. Filled from `fuel_pebble_materials` at
/// switch-on; the per-element binding energies are upstream RustBCA's.
fn uo2_material() -> material::Material<Mesh1D> {
    unimplemented!("fill from outram_mc_libs::pebble_beds::htr10::fuel_pebble_materials when gh:#979 lands")
}

/// One track for the page: points (x, y, z in µm, E in eV), rest point,
/// binned electronic and nuclear loss, flight time [s].
pub struct Track {
    pub points: Vec<[f64; 4]>,
    pub rest_um: [f64; 3],
    pub loss_bins: Vec<(f64, f64)>,
    pub flight_s: f64,
    pub recoils: usize,
}

/// Runs the two fragments of one fission back to back from the slab's middle.
pub fn run_pair(efr_ev: f64, seed: u128) -> [Track; 2] {
    let material = uo2_material();
    let mut options = Options::default_options(true);
    options.electronic_stopping_mode = ElectronicStoppingMode::INTERPOLATED;
    options.track_trajectories = true;
    options.track_energy_losses = true;
    let (el, eh) = super::physics::fragment_energies_ev(efr_ev, 95.0, 139.0);
    let mut rng = Lcg64::new(seed);
    let x0 = 0.5 * SLAB_UM * MICRON;
    let dir = (0.6_f64, 0.8_f64, 0.0_f64);
    let mut one = |z: f64, a: f64, e: f64, s: f64| {
        let p = particle::Particle::new(a * AMU, z, e * EV, EV, 0.0, 0.0, x0, 0.0, 0.0, s * dir.0, s * dir.1, s * dir.2, true, true, 0);
        let out = bca::single_ion_bca(p, &material, &options, &mut rng);
        to_track(&out)
    };
    [one(38.0, 95.0, el, 1.0), one(54.0, 139.0, eh, -1.0)]
}

fn to_track(out: &[particle::Particle]) -> Track {
    let p = out.iter().find(|p| p.incident).expect("primary");
    let um = |v: f64| v / MICRON;
    let mut flight = 0.0;
    for w in p.trajectory.windows(2) {
        let ds = ((w[1].x - w[0].x).r_powi(2) + (w[1].y - w[0].y).r_powi(2) + (w[1].z - w[0].z).r_powi(2)).sqrt();
        let v = (2.0 * w[0].E / p.m).sqrt();
        if v > 0.0 {
            flight += ds / v;
        }
    }
    let step = (p.trajectory.len() / 400).max(1);
    let points = p.trajectory.iter().step_by(step).map(|t| [um(t.x), um(t.y), um(t.z), t.E / EV]).collect();
    let n = p.energies.len().max(1);
    let mut loss_bins = vec![(0.0, 0.0); 100];
    for (i, e) in p.energies.iter().enumerate() {
        let b = (i * 100 / n).min(99);
        loss_bins[b].0 += e.Ee / EV;
        loss_bins[b].1 += e.En / EV;
    }
    let _ = ANGSTROM;
    Track { points, rest_um: [um(p.pos.x), um(p.pos.y), um(p.pos.z)], loss_bins, flight_s: flight, recoils: out.len() - 1 }
}
