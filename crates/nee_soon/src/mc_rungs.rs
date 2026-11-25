//! **The reactor models of the Monte Carlo ladder's rungs** (gh:#1742,
//! maintainer 2026-11-24: "the code for the rungs shld live in nee soon,
//! then used in dhoby ghaut/the digital twin engine lib").
//!
//! **MOVED 2026-11-25** from `dhoby-ghaut`'s `examples/monte_carlo_web/`,
//! where each rung kept its model beside its drawing, so no library could
//! reuse it. The code is the example's, moved verbatim (only paths changed),
//! and the rungs there call it; their recorded tracks and k_eff pins are
//! unchanged (the gh:#1742 record).
//!
//! What lives here is the **model half** of a rung: the case's geometry and
//! materials, its tape list, its temperature, where its neutrons are born,
//! the bed it is built on, and the recorded results it is compared with.
//! What stays in `dhoby-ghaut` is the **GUI half** (the `McRung` drawing,
//! Watch and Run) and the data builders, which process tapes through the
//! browser's processed-data cache (`dhoby_ghaut::mc_web`).
//!
//! | Rung | Module | From (`dhoby-ghaut/examples/monte_carlo_web/`) |
//! |---|---|---|
//! | Godiva (HEU-MET-FAST-001) | [`godiva`] | `godiva/model.rs`, `godiva/mod.rs` |
//! | Jemima (IEU-MET-FAST-002) | [`jemima`] | `jemima/mod.rs` |
//! | HEU-SOL-THERM-009 | [`hst009`] | `hst009/mod.rs` |
//! | natural-uranium bar | [`natu`] | `natu/mod.rs` |
//! | 2D TRISO pebble cell | [`triso`] | `triso/model.rs` |
//! | uranium and graphite | [`ugraphite`] | `ugraphite/model.rs` |
//! | lumped U in graphite | [`lumped`] | `lumped/model.rs` |
//! | HTR-10 core: bed codes, DEM bed, pour | [`htr10_core`] | `htr10/core/random_bed.rs`, `htr10/core/pour.rs`, `beds.rs` |
//!
//! Not here yet (gh:#1742, deferred): `lct008`, `dhshort` and the HTR-10
//! fuel-zone rung build their models from `outram-mc-libs/examples/common/`
//! files (its V&V harness), pulled in by `#[path]`.
//!
//! GUI-free and `no_std` + `alloc` like the rest of the crate. Research,
//! education and V&V only.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::physics::transport_csg::SourceBox;
use petir::mathf::RealMath;

/// `std` only: built on `outram_mc_libs::vv`, which is `std`-only.
#[cfg(feature = "std")]
pub mod godiva;
/// `std` only: built on `outram_mc_libs::vv`, which is `std`-only.
#[cfg(feature = "std")]
pub mod hst009;
pub mod htr10_core;
/// `std` only: built on `outram_mc_libs::vv`, which is `std`-only.
#[cfg(feature = "std")]
pub mod jemima;
/// `std` only: built on `outram_mc_libs::vv`, which is `std`-only.
#[cfg(feature = "std")]
pub mod lumped;
/// `std` only: built on `outram_mc_libs::vv`, which is `std`-only.
#[cfg(feature = "std")]
pub mod natu;
pub mod triso;
/// `std` only: built on `outram_mc_libs::vv`, which is `std`-only.
#[cfg(feature = "std")]
pub mod ugraphite;

/// The tape name of the H-in-H2O law LEAPR generates (nothing downloaded):
/// the same string as `dhoby_ghaut::mc_web::tapes::LEAPR_H_IN_H2O`, which
/// the data builders compare against (pinned equal by `monte_carlo_web`'s
/// tests).
pub const LEAPR_H_IN_H2O: &str = "leapr:HinH2O";

/// A thermal scattering law a case attaches to one of its nuclides.
#[derive(Clone, Copy, Debug)]
pub struct ThermalLaw {
    /// Label shown while loading.
    pub label: &'static str,
    /// Tape in `reference-data/endf/`, or a law LEAPR generates
    /// ([`LEAPR_H_IN_H2O`], nothing downloaded).
    pub tape: &'static str,
    /// The law's name, e.g. `c_H_in_H2O`.
    pub name: &'static str,
    /// Index, in [`CsgModel::nuclides`], of the nuclide that takes it.
    pub nuclide: usize,
}

/// **One small-CSG case: everything about the model** that the rungs'
/// shared transport code needs (the model half of `dhoby-ghaut`'s
/// `csgrung::CsgCase`, which adds the recorded result shown beside a run).
pub trait CsgModel {
    /// Material temperature, K.
    const TEMPERATURE_K: f64;
    /// Index of U-235 in [`Self::nuclides`] (the Watch chain's birth spectrum).
    const N_U235: usize;
    /// Incident energy of the fission that starts a fresh chain, eV.
    const FRESH_INCIDENT_EV: f64;
    /// Track states kept per history; far above what a history needs, so a
    /// truncated track is a reportable anomaly.
    const MAX_STATES: usize;
    /// `(label shown, nuclide name, tape)`, in the order the materials index.
    fn nuclides() -> &'static [(&'static str, &'static str, &'static str)];
    /// The thermal scattering law, processed after the nuclides.
    fn thermal() -> Option<ThermalLaw> {
        None
    }
    fn materials() -> Vec<Material>;
    fn geometry() -> Geometry;
    /// Where the first generation of a k-eigenvalue run starts.
    fn source_box() -> SourceBox;
    /// The box the source-entropy mesh covers: every place a fission can
    /// happen. The source box, unless the case fissions outside it.
    fn entropy_box() -> SourceBox {
        Self::source_box()
    }
    /// A uniformly random point of the multiplying region, for a fresh chain.
    fn fresh_birth(rng: &mut u64) -> Position;
}

/// **A recorded k_eff** to compare a reader's run with: the value, its
/// standard error of the mean, one run's seed-to-seed standard deviation and
/// histories, and where it was read from. `dhoby-ghaut` turns it into the
/// page's `Reference`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecordedRun {
    pub label: &'static str,
    pub k: f64,
    pub sem: f64,
    pub sd_one_run: f64,
    pub histories_one_run: f64,
}

/// A recorded native k_eff: value, 1σ, and the record it is read from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RecordedK {
    pub k: f64,
    pub sigma: f64,
    pub record: &'static str,
}

/// SplitMix64 — a seedable, platform-independent generator for layout and
/// births (the transport has its own RNG inside outram-mc-libs).
pub fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Uniform in [0, 1).
pub fn uniform(state: &mut u64) -> f64 {
    (splitmix(state) >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// A uniform point in the sphere of radius `r` about the origin (rejection).
pub fn uniform_in_sphere(rng: &mut u64, r: f64) -> Position {
    loop {
        let p = [0; 3].map(|_| (2.0 * uniform(rng) - 1.0) * r);
        if p[0] * p[0] + p[1] * p[1] + p[2] * p[2] < r * r {
            return Position::new(p[0], p[1], p[2]);
        }
    }
}

/// A uniform point in the cylinder of radius `r` about the z axis, between
/// `z_lo` and `z_hi`.
pub fn uniform_in_cylinder(rng: &mut u64, r: f64, z_lo: f64, z_hi: f64) -> Position {
    let rho = r * (1.0 - 1.0e-9) * uniform(rng).sqrt();
    let t = core::f64::consts::TAU * uniform(rng);
    let z = z_lo + (z_hi - z_lo) * (0.001 + 0.998 * uniform(rng));
    Position::new(rho * t.r_cos(), rho * t.r_sin(), z)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SplitMix64's first outputs from seed 0 (Vigna's reference values), so
    /// the layouts and births drawn from it cannot drift.
    #[test]
    fn splitmix_gives_the_reference_sequence() {
        let mut s = 0u64;
        assert_eq!(splitmix(&mut s), 0xE220_A839_7B1D_CDAF);
        assert_eq!(splitmix(&mut s), 0x6E78_9E6A_A1B9_65F4);
    }

    /// The samplers stay inside their shapes.
    #[test]
    fn the_samplers_stay_inside() {
        let mut rng = 7u64;
        for _ in 0..1000 {
            let p = uniform_in_sphere(&mut rng, 2.0);
            assert!(p.x * p.x + p.y * p.y + p.z * p.z < 4.0);
            let c = uniform_in_cylinder(&mut rng, 3.0, -1.0, 1.0);
            assert!(c.x * c.x + c.y * c.y < 9.0 && c.z > -1.0 && c.z < 1.0);
            let u = uniform(&mut rng);
            assert!((0.0..1.0).contains(&u));
        }
    }
}
