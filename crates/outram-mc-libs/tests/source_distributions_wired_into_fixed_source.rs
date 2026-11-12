//! **OpenMC's source distributions, driven through the source API and a
//! fixed-source run** (gh:#1388, epic #257).
//!
//! `source::distribution` holds OpenMC-faithful `Distribution` /
//! `SpatialDistribution` (bit-identical to compiled OpenMC,
//! `source_distributions_vs_openmc_code_to_code.rs`). This test checks the WIRING:
//! `SpatialKind::Distribution`, `EnergyKind::Distribution`,
//! `IndependentSource::sample` / `sample_initial_source` and
//! `FixedSource::Independent`.
//!
//! # Methodology (predictions derived BEFORE any run)
//!
//! Source: `CylindricalIndependent` with `r ~ PowerLaw(0, a=4, n=1)` (density
//! `2r/a^2`, uniform over the disc), `phi ~ Uniform(0, 2 pi)`, `z ~ Uniform(-h, h)`
//! with `h = 3`; energy `Mixture{0.3: Uniform(1e6, 2e6), 0.7: Discrete(4e6)}`;
//! beam along `+z`.
//!
//! Analytic moments:
//! - `E[x] = E[y] = E[z] = 0`; `Var x = Var y = E[r^2]/2 = a^2/4 = 4`; `Var z = h^2/3 = 3`.
//! - `E[E] = 0.3 * 1.5e6 + 0.7 * 4e6 = 3.25e6 eV`;
//!   `E[E^2] = 0.3 * 7e12/3 + 0.7 * 16e12 = 11.9e12`, `Var E = 1.3375e12`.
//!
//! Fixed-source observable: a vacuum void sphere `R = 10`; a `+z` beam from
//! `(x, y, z)` streams `L = sqrt(R^2 - r^2) - z`, and the flux tally sums `w L`.
//! `E[sqrt(R^2 - r^2)] = 2/(3a^2) (R^3 - (R^2 - a^2)^{3/2}) = 9.58864...`,
//! `Var sqrt = E[R^2 - r^2] - (E sqrt)^2 = 92 - 91.9420 = 0.0580`, so
//! `E[L] = 9.58864`, `Var L = 3.0580`, `E[L^2] = 95.0`.
//! The energy-binned flux fraction below 2 MeV is `p = 0.3` (E independent of L);
//! delta-method `Var(p_hat) = E[L^2]/E[L]^2 * p (1 - p) / N`.
//!
//! Weights: a biased `PointCloud` (points on the axis at `z = -2, +2`, true
//! strengths `[1, 1]`, bias `[3, 1]`) has weights `[2/3, 2]`; the weighted mean path
//! is `0.5 * 12 + 0.5 * 8 = 10` exactly, `Var(w L) = 112 - 100 = 12`.
//!
//! The runs use `SeedingMode::OpenMc` (a separate source stream per particle, as
//! OpenMC does): the default Outram mode repeats one source site in a void, filed
//! as gh:#1493 and not changed here.
//!
//! Pass criterion: every sampled / tallied mean within 5 sigma of the analytic value
//! (sigma from the analytic variance and N, fixed above, not tuned). Existing sources
//! must sample bit-identically: the unit-weight path is checked by comparing
//! `IndependentSource::sample` against the raw spatial / energy / angle draws.
//!
//! # Results (2026-11-11, Rust 1.99.1 release, seed values in the tests)
//!
//! | quantity | measured | analytic | sigma | deviation |
//! |---|---|---|---|---|
//! | sampled mean x / y / z (N = 4e5) | 0.0043 / -0.0082 / 0.0002 | 0 | 0.0032 / 0.0032 / 0.0028 | 1.3 / 2.6 / 0.1 |
//! | Var x / Var z | 3.9964 / 3.0000 | 4 / 3 | within 5 sigma | pass |
//! | mean E, Var E | 3.24728e6, 1.34009e12 | 3.25e6, 1.3375e12 | within 5 sigma | pass |
//! | fixed-source mean path (N = 2e5) | 9.58478 | 9.58864 | 0.00391 | -1.0 |
//! | energy-binned flux fraction E < 2 MeV | 0.29894 | 0.3 | 0.00104 | -1.0 |
//! | biased cloud, weighted mean path | 10.00468 | 10 | 0.00775 | +0.6 |
//!
//! Existing sources: 1000 draws bit-identical (position, energy, direction, weight,
//! RNG state). Interpretation: the `Distribution` variants reach the transport with
//! the right moments and the importance weight reaches the tally.

use std::sync::Arc;

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::physics::fixed_source::{
    run_fixed_source, FixedSource, FixedSourceSettings,
};
use outram_mc_libs::prelude::{
    AngleKind, Distribution, EnergyKind, IndependentSource, SpatialDistribution, SpatialKind,
};
use outram_mc_libs::rng::openmc_seeding::SeedingMode;
use outram_mc_libs::source::angle::{IsotropicAngle, MonodirectionalAngle};
use outram_mc_libs::source::energy::{EnergyDist, Monoenergetic};
use outram_mc_libs::source::spatial::{PointSource, SpatialDist};
use outram_mc_libs::tally::filter::{CellFilter, EnergyFilter, FilterKind};
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

const R: f64 = 10.0;
const A: f64 = 4.0;
const H: f64 = 3.0;

fn void_sphere() -> Geometry {
    Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: R,
            bc: BoundaryType::Vacuum,
        })],
        cells: vec![Cell::fill(
            1,
            vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }],
            CellFill::Void,
            Position::ZERO,
        )],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    }
}

fn mixture_energy() -> Distribution {
    Distribution::mixture(vec![
        (0.3, Distribution::uniform(1.0e6, 2.0e6)),
        (0.7, Distribution::discrete(&[4.0e6], &[1.0])),
    ])
}

fn cylindrical() -> SpatialDistribution {
    SpatialDistribution::cylindrical(
        Distribution::power_law(0.0, A, 1.0),
        Distribution::uniform(0.0, std::f64::consts::TAU),
        Distribution::uniform(-H, H),
        [0.0; 3],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
    )
    .expect("perpendicular frame")
}

fn beam_source() -> IndependentSource {
    IndependentSource {
        spatial: SpatialKind::Distribution(cylindrical()),
        energy: EnergyKind::Distribution(mixture_energy()),
        angle: AngleKind::Monodirectional(MonodirectionalAngle {
            d: Direction::new(0.0, 0.0, 1.0),
        }),
        strength: 1.0,
    }
}

fn flux_tally(filters: Vec<FilterKind>, n_bins: usize) -> Tally {
    Tally {
        id: 1,
        name: "flux".into(),
        filters,
        scores: vec![ScoreType::Flux],
        bins: vec![TallyBin::default(); n_bins],
    }
}

fn five_sigma(var: f64, n: usize) -> f64 {
    5.0 * (var / n as f64).sqrt()
}

/// Sampled source moments against the analytic ones (see module docs).
#[test]
fn sampled_moments_match_the_analytic_moments() {
    let src = beam_source();
    let n = 400_000usize;
    let mut seed = 0x1388_0001u64;
    let (mut sx, mut sy, mut sz, mut sxx, mut szz, mut se, mut see) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for _ in 0..n {
        let s = src.sample(&mut seed);
        assert_eq!(s.wgt, 1.0, "unbiased source must have unit weight");
        assert_eq!((s.u.u, s.u.v, s.u.w), (0.0, 0.0, 1.0));
        sx += s.r.x;
        sy += s.r.y;
        sz += s.r.z;
        sxx += s.r.x * s.r.x;
        szz += s.r.z * s.r.z;
        se += s.e;
        see += s.e * s.e;
    }
    let nf = n as f64;
    let (mx, my, mz) = (sx / nf, sy / nf, sz / nf);
    let (vx, vz) = (sxx / nf - mx * mx, szz / nf - mz * mz);
    let (me, ve) = (se / nf, see / nf - (se / nf) * (se / nf));
    println!("MEASURED x {mx:.5} y {my:.5} z {mz:.5} Var x {vx:.4} Var z {vz:.4} E {me:.5e} Var E {ve:.5e}");
    assert!(mx.abs() < five_sigma(4.0, n), "mean x {mx}");
    assert!(my.abs() < five_sigma(4.0, n), "mean y {my}");
    assert!(mz.abs() < five_sigma(3.0, n), "mean z {mz}");
    // Var of a sample variance: (m4 - var^2)/N; x uses a generous analytic bound
    // m4(x) = E[r^4]/8 * 3 = (a^4/3)/8*3... computed: E[r^4] = a^4/3, E[cos^4]=3/8.
    let m4x = (A.powi(4) / 3.0) * 3.0 / 8.0;
    assert!((vx - 4.0).abs() < five_sigma(m4x - 16.0, n), "Var x {vx}");
    // z uniform: m4 = h^4/5.
    assert!((vz - 3.0).abs() < five_sigma(H.powi(4) / 5.0 - 9.0, n), "Var z {vz}");
    assert!((me - 3.25e6).abs() < five_sigma(1.3375e12, n), "mean E {me}");
    // Var E: E[(E-mu)^4] for the mixture bounds the sample-variance sigma; use
    // E[E^4]-based upper bound 16e24 * 0.7 + ... conservatively (E^4 <= 256e24).
    assert!((ve - 1.3375e12).abs() < five_sigma(256.0e24, n), "Var E {ve}");
}

/// Existing sources are untouched: the unit-weight wiring changes no draw and no
/// bit of the sampled site.
#[test]
fn existing_sources_sample_bit_identically() {
    let src = IndependentSource {
        spatial: SpatialKind::Point(PointSource { r: Position::new(1.0, 2.0, 3.0) }),
        energy: EnergyKind::Mono(Monoenergetic { e: 2.0e6 }),
        angle: AngleKind::Isotropic(IsotropicAngle),
        strength: 2.5,
    };
    let (mut a, mut b) = (77u64, 77u64);
    for _ in 0..1000 {
        let s = src.sample(&mut a);
        // Reference: the raw draws in the documented order.
        let r = src.spatial.sample(&mut b);
        let e = src.energy.sample(&mut b);
        let u = outram_mc_libs::source::angle::AngleDist::sample(&src.angle, &mut b, e);
        assert_eq!(s.r, r);
        assert_eq!(s.e.to_bits(), e.to_bits());
        assert_eq!(s.u, u);
        assert_eq!(s.wgt.to_bits(), 2.5f64.to_bits());
        assert_eq!(a, b, "RNG stream must advance identically");
    }
}

/// Fixed-source run: mean path and the energy-binned flux fraction.
#[test]
fn fixed_source_run_reproduces_path_and_energy_split() {
    let geom = void_sphere();
    let src = FixedSource::Independent(Arc::new(beam_source()));
    let n = 200_000usize;
    let mut total = flux_tally(vec![FilterKind::Cell(CellFilter { cell_indices: vec![0] })], 1);
    let res = run_fixed_source(
        &geom,
        &[],
        &[],
        &src,
        &FixedSourceSettings { n_particles: n, n_batches: 10, seed: 13_880_002, ..Default::default() }.with_seeding(SeedingMode::OpenMc { total_gen: 0 }),
        Some(&mut total),
    );
    assert_eq!(res.total_histories, n);
    let mean_l = total.bins[0].sum / n as f64;
    let el = (2.0 / (3.0 * A * A)) * (R.powi(3) - (R * R - A * A).powf(1.5));
    let var_l = (R * R - A * A / 2.0 - el * el) + H * H / 3.0;
    println!("MEASURED mean path {mean_l:.5} analytic {el:.5} sigma {:.5}", (var_l / n as f64).sqrt());
    assert!((mean_l - el).abs() < five_sigma(var_l, n), "mean path {mean_l} vs {el}");

    let mut split = flux_tally(
        vec![FilterKind::Energy(EnergyFilter { bins: vec![0.0, 2.0e6, 5.0e6] })],
        2,
    );
    run_fixed_source(
        &geom,
        &[],
        &[],
        &src,
        &FixedSourceSettings { n_particles: n, n_batches: 10, seed: 13_880_003, ..Default::default() }.with_seeding(SeedingMode::OpenMc { total_gen: 0 }),
        Some(&mut split),
    );
    let (lo, hi) = (split.bins[0].sum, split.bins[1].sum);
    let p = lo / (lo + hi);
    let e_l2 = var_l + el * el;
    let var_p = e_l2 / (el * el) * 0.3 * 0.7;
    println!("MEASURED fraction E<2MeV {p:.5} analytic 0.3 sigma {:.5}", (var_p / n as f64).sqrt());
    assert!((p - 0.3).abs() < five_sigma(var_p, n), "flux fraction {p}");
}

/// Importance weights from a biased spatial distribution reach the tally.
#[test]
fn biased_point_cloud_weights_reach_the_tally() {
    let geom = void_sphere();
    let cloud = SpatialDistribution::point_cloud_biased(
        vec![[0.0, 0.0, -2.0], [0.0, 0.0, 2.0]],
        &[1.0, 1.0],
        &[3.0, 1.0],
    )
    .unwrap();
    let src = IndependentSource {
        spatial: SpatialKind::Distribution(cloud),
        energy: EnergyKind::Mono(Monoenergetic { e: 1.0e6 }),
        angle: AngleKind::Monodirectional(MonodirectionalAngle { d: Direction::new(0.0, 0.0, 1.0) }),
        strength: 1.0,
    };
    let n = 200_000usize;
    let mut t = flux_tally(vec![FilterKind::Cell(CellFilter { cell_indices: vec![0] })], 1);
    run_fixed_source(
        &geom,
        &[],
        &[],
        &FixedSource::Independent(Arc::new(src)),
        &FixedSourceSettings { n_particles: n, n_batches: 10, seed: 13_880_004, ..Default::default() }.with_seeding(SeedingMode::OpenMc { total_gen: 0 }),
        Some(&mut t),
    );
    let mean = t.bins[0].sum / n as f64;
    println!("MEASURED weighted mean path {mean:.5} analytic 10 sigma {:.5}", (12.0 / n as f64).sqrt());
    assert!((mean - 10.0).abs() < five_sigma(12.0, n), "weighted mean path {mean}");
}

/// The k-eigenvalue initial source: `n` sites with the right moments, in the form
/// the distributed power iteration accepts.
#[test]
fn keff_initial_source_has_the_right_moments_and_feeds_the_iteration() {
    use outram_mc_libs::physics::keff::KeffSettings;
    use outram_mc_libs::physics::transport_csg::distributed::DistributedPowerIteration;
    let src = beam_source();
    let n = 100_000usize;
    let mut seed = 0x1388_0005u64;
    let sites = src.sample_initial_source(n, &mut seed);
    assert_eq!(sites.len(), n);
    let mean_e = sites.iter().map(|s| s.e).sum::<f64>() / n as f64;
    assert!((mean_e - 3.25e6).abs() < five_sigma(1.3375e12, n), "mean E {mean_e}");
    let mean_z = sites.iter().map(|s| s.r[2]).sum::<f64>() / n as f64;
    assert!(mean_z.abs() < five_sigma(3.0, n), "mean z {mean_z}");
    let settings = KeffSettings { n_particles: n, ..Default::default() };
    let it = DistributedPowerIteration::from_initial_source(&settings, &sites, seed);
    let chunks = it.chunks(4, 0);
    assert_eq!(chunks.iter().map(|c| c.sites.len()).sum::<usize>(), n);
    // The sampler is a pure function of the seed: a second draw from the same
    // seed reproduces the sites.
    let mut s2 = 0x1388_0005u64;
    let again = src.sample_initial_source(n, &mut s2);
    assert_eq!(again, sites);
    assert_eq!(s2, seed);
    // Keep the trait import used.
    let _ = SpatialKind::Point(PointSource { r: Position::ZERO }).sample(&mut s2);
}
