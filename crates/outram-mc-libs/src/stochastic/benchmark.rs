//! Benchmark suite — RSA vs CLS vs SCLS on a common problem (design doc §18, bead
//! `op-eby.7`).
//!
//! The three random-media models ([`super::medium::StochasticMedium`]) only differ in
//! *how much geometry they remember*, so a benchmark has to exercise the one regime
//! where memory matters: **back-scattering**. A neutron that scatters isotropically in
//! the matrix and is absorbed on entering an inclusion repeatedly re-crosses ground it
//! has already covered — exactly where classical CLS's memoryless assumption breaks and
//! SCLS's retained inclusions are meant to help.
//!
//! # The problem
//!
//! An absorbing-inclusion random walk in the packing's cube domain:
//!
//! - Inclusions are pure absorbers; the matrix is a pure isotropic scatterer with mean
//!   free path `scatter_mfp` \[cm\].
//! - Each history is born at a uniform random point (so birth-in-inclusion has
//!   probability equal to the packing fraction, consistently for every model), given an
//!   isotropic direction, and walked: sample a scatter distance, step along it querying
//!   the medium; entering an inclusion **absorbs**, leaving the cube **leaks**, and
//!   reaching `max_collisions` without either **survives**.
//! - The tally is the absorption probability.
//!
//! [`AbsorptionBenchmark::compare`] builds all three media from **one** RSA packing —
//! RSA sees the explicit geometry, CLS/SCLS see only its statistics (radius + packing
//! fraction) — and runs the identical walk on each. RSA is the reference; the gap
//! `|P_model − P_RSA|` is the model's error.
//!
//! # No accuracy claim is baked in
//!
//! This module *measures*; it does not assert that SCLS beats CLS. Whether it does, and
//! by how much, is regime-dependent (inclusion optical thickness, scattering ratio,
//! packing fraction) and is the empirical output the suite exists to produce. The unit
//! test checks the harness runs and returns physical probabilities; a representative
//! measured comparison is recorded below.
//!
//! # Representative result (2026-08-06)
//!
//! For `domain_half_width = 0.5 cm`, `particle_radius = 0.05 cm`, `packing_fraction =
//! 0.2`, `scatter_mfp = 0.3 cm`, `max_collisions = 200`, 4000 histories (seed
//! 20260721), absorption probabilities were **RSA = 0.6947, CLS = 0.7073 (+0.0126),
//! SCLS = 0.7165 (+0.0218)**. The binomial standard error `sqrt(p(1-p)/n)` is ~0.007
//! on each arm at n = 4000, so the CLS gap is ~1.2 combined-se wide and the SCLS gap
//! ~2.1 — but CLS/SCLS are deterministic functions of the packing statistics rather
//! than unbiased estimators of the explicit geometry, so those gaps report *model*
//! approximation error, not sampling noise.
//!
//! Two honest observations, not accuracy claims:
//! - Both approximate models land within ~0.022 of the explicit reference, so the harness
//!   is clearly measuring the same physics on each arm.
//! - In *this* regime **CLS is actually closer to RSA than SCLS**, and both overestimate
//!   absorption. That is a real, regime-dependent finding — SCLS's retained inclusions
//!   raise the re-encounter (and hence absorption) rate on back-scatter, which here
//!   over-corrects past the reference. Whether SCLS wins in optically-thicker or
//!   higher-packing regimes is exactly the parameter study this suite exists to run; it
//!   is not asserted here. These numbers are reproducible from the seed but are a
//!   generated result, not a committed reference (crate `CLAUDE.md` V&V-output rule).
//!
//! **Supersedes (2026-07-21): RSA = 0.684, CLS = 0.697 (+0.013), SCLS = 0.745
//! (+0.061).** Those figures were measured with the pre-`op-jis` `prn` output
//! function — the raw top-52 state bits, before [`crate::rng::lcg::prn`] gained
//! OpenMC's PCG-RXS-M-XS output permutation. The LCG *state recurrence* is unchanged,
//! so the RSA packing and the walk are structurally identical; only the uniform
//! stream every arm consumes moved, and all three arms re-drew. The qualitative
//! finding survives unchanged — both models still overestimate absorption, and CLS is
//! still the closer of the two — but SCLS's overestimate shrank markedly, from +0.061
//! to +0.0218. No tolerance was changed. (`max_collisions = 200`, matching this
//! module's own unit test, is now stated explicitly above because the superseded
//! 2026-07-21 record did not name it.)
//!
//! This module is **new work**, not an OpenMC port.

use crate::geometry::position::Position;
use crate::pebble_beds::sphere_packing::{PackedSpheres, PackingConfig, PackingError, PackingMethod};
use crate::rng::distributions::isotropic_direction;
use crate::rng::lcg::prn;
use crate::stochastic::cls::ClsMedium;
use crate::stochastic::medium::{MaterialId, RsaMedium, StochasticMedium};
use crate::stochastic::scls::SclsMedium;

/// Material id of the absorbing inclusion phase used by the benchmark.
const INCLUSION: MaterialId = MaterialId(1);
/// Material id of the scattering matrix phase used by the benchmark.
const MATRIX: MaterialId = MaterialId(0);

/// How a single history ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// Entered an inclusion (absorber).
    Absorbed,
    /// Left the cube domain.
    Leaked,
    /// Reached `max_collisions` without absorbing or leaking.
    Survived,
}

/// The measured outcome distribution for one model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BenchmarkResult {
    /// Model name ([`StochasticMedium::name`]).
    pub model: &'static str,
    /// Fraction of histories absorbed in an inclusion.
    pub absorption_probability: f64,
    /// Fraction of histories that leaked out of the domain.
    pub leakage_probability: f64,
    /// Number of histories run.
    pub histories: usize,
}

/// Absorbing-inclusion random-walk benchmark configuration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbsorptionBenchmark {
    /// Half-width of the cube domain \[cm\].
    pub domain_half_width: f64,
    /// Inclusion (absorber) radius \[cm\].
    pub particle_radius: f64,
    /// Inclusion volume (packing) fraction, in (0, 1).
    pub packing_fraction: f64,
    /// Matrix scatter mean free path \[cm\].
    pub scatter_mfp: f64,
    /// Maximum matrix collisions before a history is declared "survived".
    pub max_collisions: usize,
    /// Histories per model.
    pub histories: usize,
}

impl AbsorptionBenchmark {
    /// Fine march step \[cm\] — half an inclusion radius, so an inclusion is not stepped
    /// over. Small enough to resolve entry, large enough to keep the walk affordable.
    fn step(&self) -> f64 {
        0.5 * self.particle_radius
    }

    /// A uniform random point in the cube domain \[cm\].
    fn birth(&self, seed: &mut u64) -> Position {
        let w = self.domain_half_width;
        Position::new(
            (2.0 * prn(seed) - 1.0) * w,
            (2.0 * prn(seed) - 1.0) * w,
            (2.0 * prn(seed) - 1.0) * w,
        )
    }

    /// Whether `p` is still inside the cube domain.
    fn in_domain(&self, p: Position) -> bool {
        let w = self.domain_half_width;
        p.x.abs() <= w && p.y.abs() <= w && p.z.abs() <= w
    }

    /// Walk one history through `medium`.
    fn walk(&self, medium: &mut StochasticMedium, seed: &mut u64) -> Outcome {
        medium.begin_flight();
        let mut pos = self.birth(seed);
        let (mut du, mut dv, mut dw) = isotropic_direction(seed);
        let step = self.step();

        for _ in 0..self.max_collisions {
            // Distance to the next matrix scatter (exponential, mean = scatter_mfp).
            let d_scatter = -self.scatter_mfp * (1.0 - prn(seed)).ln();
            let mut travelled = 0.0;
            while travelled < d_scatter {
                let ds = step.min(d_scatter - travelled);
                pos = Position::new(pos.x + du * ds, pos.y + dv * ds, pos.z + dw * ds);
                travelled += ds;
                if !self.in_domain(pos) {
                    return Outcome::Leaked;
                }
                // `material_at` never errors for these three models.
                if medium.material_at(pos, seed).unwrap_or(MATRIX) == INCLUSION {
                    return Outcome::Absorbed;
                }
            }
            // Isotropic scatter in the matrix.
            let (nu, nv, nw) = isotropic_direction(seed);
            du = nu;
            dv = nv;
            dw = nw;
        }
        Outcome::Survived
    }

    /// Run `histories` walks through one medium and tally the outcomes.
    pub fn run(&self, medium: &mut StochasticMedium, seed: &mut u64) -> BenchmarkResult {
        let mut absorbed = 0usize;
        let mut leaked = 0usize;
        for _ in 0..self.histories {
            match self.walk(medium, seed) {
                Outcome::Absorbed => absorbed += 1,
                Outcome::Leaked => leaked += 1,
                Outcome::Survived => {}
            }
        }
        let n = self.histories.max(1) as f64;
        BenchmarkResult {
            model: medium.name(),
            absorption_probability: absorbed as f64 / n,
            leakage_probability: leaked as f64 / n,
            histories: self.histories,
        }
    }

    /// Build RSA, CLS and SCLS media from **one** RSA packing and run the identical
    /// walk on each. Returns `[RSA, CLS, SCLS]`; RSA is the reference arm.
    ///
    /// # Errors
    /// [`PackingError`] if the RSA packing cannot be generated at the requested packing
    /// fraction (e.g. too dense for the simple RSA sampler).
    pub fn compare(&self, seed: u64) -> Result<[BenchmarkResult; 3], PackingError> {
        let cfg = PackingConfig {
            particle_radius: self.particle_radius,
            packing_fraction: self.packing_fraction,
            domain_half_width: self.domain_half_width,
            method: PackingMethod::Rsa,
            seed,
        };
        let spheres = cfg.generate()?;
        let packing =
            PackedSpheres::from_spheres(spheres, self.domain_half_width, self.particle_radius);

        let cls_medium = ClsMedium::new(self.particle_radius, self.packing_fraction, INCLUSION, MATRIX);

        let mut rsa = StochasticMedium::Rsa(RsaMedium::new(packing, INCLUSION, MATRIX));
        let mut cls = StochasticMedium::Cls(cls_medium.clone());
        let mut scls =
            StochasticMedium::Scls(SclsMedium::new(cls_medium, Position::ZERO, self.scatter_mfp));

        // Independent LCG streams per arm (derived from the master seed).
        let mut s_rsa = seed ^ 0x5151_5151_5151_5151;
        let mut s_cls = seed ^ 0x2626_2626_2626_2626;
        let mut s_scls = seed ^ 0x3737_3737_3737_3737;

        Ok([
            self.run(&mut rsa, &mut s_rsa),
            self.run(&mut cls, &mut s_cls),
            self.run(&mut scls, &mut s_scls),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The suite runs and returns physical probabilities for all three arms, with the
    /// three landing in the same ballpark (the harness measures the same physics on
    /// each). No SCLS-beats-CLS assertion is made — that is a regime-dependent research
    /// output, not an invariant.
    #[test]
    fn compare_runs_and_returns_physical_probabilities() {
        let bench = AbsorptionBenchmark {
            domain_half_width: 0.5,
            particle_radius: 0.05,
            packing_fraction: 0.2,
            scatter_mfp: 0.3,
            max_collisions: 200,
            histories: 2000,
        };
        let [rsa, cls, scls] = bench.compare(20260721).expect("RSA packing at pf=0.2");

        for r in [rsa, cls, scls] {
            assert!(
                (0.0..=1.0).contains(&r.absorption_probability),
                "{} absorption {} out of range",
                r.model,
                r.absorption_probability
            );
            assert!((0.0..=1.0).contains(&r.leakage_probability));
            assert!(r.absorption_probability + r.leakage_probability <= 1.0 + 1e-9);
        }

        // RSA reference is non-trivial: some absorb, not all.
        assert!(
            rsa.absorption_probability > 0.05 && rsa.absorption_probability < 0.98,
            "RSA absorption {} should be a non-degenerate reference",
            rsa.absorption_probability
        );

        // The approximate models are in the same ballpark as the reference (sanity that
        // they are solving the same problem, not that they are exact).
        assert!(
            (cls.absorption_probability - rsa.absorption_probability).abs() < 0.4,
            "CLS {} implausibly far from RSA {}",
            cls.absorption_probability,
            rsa.absorption_probability
        );
        assert!(
            (scls.absorption_probability - rsa.absorption_probability).abs() < 0.4,
            "SCLS {} implausibly far from RSA {}",
            scls.absorption_probability,
            rsa.absorption_probability
        );
    }

    /// The comparison is reproducible from the master seed.
    #[test]
    fn compare_is_reproducible() {
        let bench = AbsorptionBenchmark {
            domain_half_width: 0.4,
            particle_radius: 0.05,
            packing_fraction: 0.15,
            scatter_mfp: 0.25,
            max_collisions: 100,
            histories: 300,
        };
        let a = bench.compare(42).expect("packing");
        let b = bench.compare(42).expect("packing");
        assert_eq!(a, b);
    }
}
