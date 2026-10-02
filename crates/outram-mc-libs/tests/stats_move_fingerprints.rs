//! **Bit-identity gate for moving outram-mc's generic statistics into RAFFLES**
//! (GitHub issue #500, 2026-10-02).
//!
//! # Methodology
//!
//! The move relocates pure arithmetic — the sample mean and standard error of
//! the active-generation eigenvalues (four former copies, in `physics::keff`,
//! `physics::transport_csg`, `physics::physics_mg` and
//! `pebble_beds::keff_delta`), Shannon entropy over mesh counts, seed pooling,
//! the trigger's running-sum statistics, tally error propagation, and the
//! generic samplers (`uniform`, `sample_normal`, `sample_exp`). None of it may
//! change a single bit of any result.
//!
//! Each case below runs a fixed-seed calculation through the ordinary public
//! API and folds every output `f64` (`to_bits`) into a 64-bit FNV-1a
//! fingerprint, which is pinned. The pinned values were printed by THIS FILE
//! on the commit before the move (`STATS_FP_PRINT=1 cargo test --release -p
//! outram-mc-libs --test stats_move_fingerprints -- --nocapture`), then the
//! move was made and the file re-run unchanged. A fingerprint is a hash, so a
//! mismatch says only that something moved; rerun with `STATS_FP_PRINT=1` and
//! diff the printed bits to see what.
//!
//! | case | exercises |
//! |---|---|
//! | Godiva, `run_keff` | `keff.rs` mean/stderr |
//! | Godiva, `run_keff_csg_hybrid` + entropy mesh + k trigger | `transport_csg.rs` mean/stderr, Shannon entropy, trigger statistics |
//! | two-group MG sphere, `run_keff_mg` | `physics_mg.rs` mean/stderr |
//! | FHR explicit-TRISO pebble (`examples/fhr_pebble_quickstart.rs`), `run_keff_delta_in` | `keff_delta.rs` mean/stderr, a TRISO delta-tracking case |
//! | pure statistics on fixed data | `pooled`, `bin_uncertainty`, `predict_batches`, `DerivedTally`, samplers |
//!
//! # Results
//!
//! Measured 2026-10-02 on the commit before the move (outram-mc-libs with the
//! LCG already in PETIR), single host (x86_64 Linux, glibc):
//!
//! | case | fingerprint | k (bits) |
//! |---|---|---|
//! | `godiva_run_keff` | `0xe78387f55e2fa2fd` | `0x3ff00f0eaccb9f03 +/- 0x3f737285e6855dbd` |
//! | `godiva_csg_entropy_trigger` (trigger stopped at 33 of 40 generations; 33 entropy values) | `0x0f90e5295c6cbcae` | `0x3ff067ac7ac2dc28 +/- 0x3f83ff11d76e40a5` |
//! | `mg_sphere` | `0xcafa58c94439199d` | `0x3fe5281b4e81b4e6 +/- 0x3f7a2fd2977f13b2` |
//! | `fhr_explicit_triso_delta` | `0x86e84b60e8278ec3` | `0x3fface4d94cf76ff +/- 0x3f833745b17635e5` |
//! | `pure_statistics` | `0xf67085fed4325815` | — |
//! | `shannon_entropy` | `0x5c785611104f059b` | `H = 0x4012bef1a747a582` |
//!
//! After the move: identical, all six.
//!
//! **Platform caveat.** `sample_normal` calls the platform `cos` unless the
//! `deterministic-math` feature is on, and the transport drivers call the
//! platform `cos`/`sin` too, so a different libm (macOS, Windows) can give
//! different bits. These pins are a same-host before/after gate; if they fail
//! on another platform with no code change, re-pin there rather than read it
//! as a regression.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::{run_keff, KeffResult, KeffSettings};
use outram_mc_libs::physics::physics_mg::{run_keff_mg, MgSettings, Mgxs, MgxsLibrary};
use outram_mc_libs::physics::transport_csg::{run_keff_csg_hybrid, SourceBox};
use outram_mc_libs::tally::mesh::RegularMesh;

/// FNV-1a over a stream of `f64` bit patterns.
struct Fp(u64);

impl Fp {
    fn new() -> Self {
        Fp(0xcbf2_9ce4_8422_2325)
    }
    fn f(&mut self, x: f64) {
        for b in x.to_bits().to_le_bytes() {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
    fn fs(&mut self, xs: &[f64]) {
        for &x in xs {
            self.f(x);
        }
    }
    fn keff(&mut self, r: &KeffResult) {
        self.f(r.k_mean);
        self.f(r.k_std);
        self.fs(&r.k_by_generation);
        self.fs(&r.entropy);
    }
}

/// Print with `STATS_FP_PRINT=1`; otherwise assert against the pinned value.
fn check(name: &str, got: u64, pinned: u64, detail: &str) {
    if std::env::var("STATS_FP_PRINT").is_ok() {
        eprintln!("FINGERPRINT {name} = {got:#018x}   ({detail})");
        return;
    }
    assert_eq!(got, pinned, "{name} moved: {detail}");
}

fn godiva() -> (Material, Vec<Nuclide>) {
    let nuclides = vec![
        Nuclide::from_core("U234").unwrap(),
        Nuclide::from_core("U235").unwrap(),
        Nuclide::from_core("U238").unwrap(),
    ];
    let material = Material {
        id: 1,
        name: "Godiva".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent {
                nuclide_idx: 0,
                atom_density: 4.9184e-4,
            },
            NuclideComponent {
                nuclide_idx: 1,
                atom_density: 4.4994e-2,
            },
            NuclideComponent {
                nuclide_idx: 2,
                atom_density: 2.4984e-3,
            },
        ],
    };
    (material, nuclides)
}

fn sphere(r_cm: f64) -> Geometry {
    Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: r_cm,
            bc: BoundaryType::Vacuum,
        })],
        cells: vec![Cell::material(
            1,
            vec![RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Inside,
            }],
            0,
            293.6,
        )],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

#[test]
fn godiva_run_keff_is_bit_identical() {
    let (material, nuclides) = godiva();
    let settings = KeffSettings {
        n_particles: 1500,
        n_inactive: 20,
        n_active: 40,
        ..KeffSettings::default()
    };
    let r = run_keff(8.7407, &material, &nuclides, &settings);
    let mut fp = Fp::new();
    fp.keff(&r);
    check(
        "godiva_run_keff",
        fp.0,
        0xe78387f55e2fa2fd,
        &format!(
            "k = {:#018x} +/- {:#018x}",
            r.k_mean.to_bits(),
            r.k_std.to_bits()
        ),
    );
}

#[test]
fn godiva_csg_with_entropy_and_trigger_is_bit_identical() {
    use outram_mc_libs::tally::trigger::{Trigger, TriggerMetric};
    let (material, nuclides) = godiva();
    let mesh = RegularMesh {
        lower_left: [-8.7407; 3],
        upper_right: [8.7407; 3],
        dimension: [4, 4, 4],
    };
    let settings = KeffSettings {
        n_particles: 600,
        n_inactive: 10,
        n_active: 30,
        // Loose enough to stop early, so the trigger statistics decide the
        // generation count and therefore every bit after it.
        keff_trigger: Some(Trigger {
            metric: TriggerMetric::StandardDeviation,
            threshold: 0.01,
            ignore_zeros: false,
        }),
        ..KeffSettings::default()
    };
    let src = SourceBox {
        lower: Position::new(-3.0, -3.0, -3.0),
        upper: Position::new(3.0, 3.0, 3.0),
    };
    let r = run_keff_csg_hybrid(
        &sphere(8.7407),
        &[material],
        &nuclides,
        &[],
        Some(&mesh),
        src,
        &settings,
        None,
    );
    let mut fp = Fp::new();
    fp.keff(&r);
    check(
        "godiva_csg_entropy_trigger",
        fp.0,
        0x0f90e5295c6cbcae,
        &format!(
            "k = {:#018x} +/- {:#018x}, {} generations, {} entropy values",
            r.k_mean.to_bits(),
            r.k_std.to_bits(),
            r.k_by_generation.len(),
            r.entropy.len()
        ),
    );
}

#[test]
fn two_group_mg_sphere_is_bit_identical() {
    let xs = Mgxs::new(
        "2g-fuel",
        vec![0.080, 0.180],
        vec![0.010, 0.080],
        vec![0.0032, 0.040],
        vec![0.008, 0.100],
        vec![1.0, 0.0],
        vec![0.050, 0.020, 0.000, 0.100],
    );
    let lib = MgxsLibrary::new(vec![xs]);
    let settings = MgSettings {
        n_particles: 800,
        n_inactive: 10,
        n_active: 30,
        seed: 7,
    };
    let src = SourceBox {
        lower: Position::new(-10.0, -10.0, -10.0),
        upper: Position::new(10.0, 10.0, 10.0),
    };
    let r = run_keff_mg(&sphere(40.0), &lib, src, &settings);
    let mut fp = Fp::new();
    fp.keff(&r);
    check(
        "mg_sphere",
        fp.0,
        0xcafa58c94439199d,
        &format!(
            "k = {:#018x} +/- {:#018x}",
            r.k_mean.to_bits(),
            r.k_std.to_bits()
        ),
    );
}

/// `examples/fhr_pebble_quickstart.rs`, explicit-TRISO arm, verbatim inputs.
#[test]
fn fhr_explicit_triso_delta_is_bit_identical() {
    use outram_mc_libs::pebble_beds::crp_packing::pack_spheres_crp;
    use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
    use outram_mc_libs::pebble_beds::fhr_pebble::{ExplicitTrisoPebble, TrisoMaterials, TrisoSpec};
    use outram_mc_libs::pebble_beds::keff_delta::{run_keff_delta_in, DeltaDomain};
    use outram_mc_libs::pebble_beds::sphere_packing::PackedSpheres;

    const TEMP_K: f64 = 900.0;
    let nuclides = vec![
        Nuclide::from_core("U235").unwrap(),
        Nuclide::from_core("U238").unwrap(),
        Nuclide::from_core("O16").unwrap(),
        Nuclide::from_core("C0").unwrap(),
        Nuclide::from_core("Si28").unwrap(),
        Nuclide::from_core("F19").unwrap(),
        Nuclide::from_core("Li7").unwrap(),
        Nuclide::from_core("Be9").unwrap(),
    ];
    let nc = |nuclide_idx: usize, atom_density: f64| NuclideComponent {
        nuclide_idx,
        atom_density,
    };
    let mat = |id: i32, name: &str, components: Vec<NuclideComponent>| Material {
        id,
        name: name.into(),
        temperature: TEMP_K,
        components,
    };
    let materials = vec![
        mat(
            1,
            "fuel kernel",
            vec![nc(0, 0.00467), nc(1, 0.01880), nc(2, 0.04695)],
        ),
        mat(2, "buffer", vec![nc(3, 0.0501)]),
        mat(3, "IPyC", vec![nc(3, 0.0953)]),
        mat(4, "SiC", vec![nc(4, 0.0481), nc(3, 0.0481)]),
        mat(5, "OPyC", vec![nc(3, 0.0938)]),
        mat(6, "graphite", vec![nc(3, 0.0852)]),
        mat(
            7,
            "FLiBe",
            vec![nc(6, 0.0236), nc(5, 0.0472), nc(7, 0.0118)],
        ),
    ];
    let spec = TrisoSpec {
        kernel: 0.08,
        buffer: 0.10,
        ipyc: 0.11,
        sic: 0.12,
        opyc: 0.13,
        packing_fraction: 0.30,
    };
    const R_FUEL_ZONE: f64 = 0.5;
    const R_PEBBLE: f64 = 0.6;
    const R_ROOT: f64 = 1.0;
    let pack_half = R_FUEL_ZONE + spec.opyc;
    let spheres =
        pack_spheres_crp(spec.opyc, pack_half, spec.packing_fraction, 20_260_911).unwrap();
    let packed = PackedSpheres::from_spheres(spheres, pack_half, spec.opyc);
    let pebble = ExplicitTrisoPebble::new(
        packed,
        spec,
        TrisoMaterials {
            kernel: 0,
            buffer: 1,
            ipyc: 2,
            sic: 3,
            opyc: 4,
            matrix: 5,
        },
        5,
        6,
        R_FUEL_ZONE,
        R_PEBBLE,
    );
    let (e_min, e_max, n_grid): (f64, f64, usize) = (1.0e-4, 2.0e7, 150);
    let ratio = (e_max / e_min).powf(1.0 / (n_grid as f64 - 1.0));
    let grid: Vec<f64> = (0..n_grid as i32).map(|i| e_min * ratio.powi(i)).collect();
    let majorant = Majorant::from_materials(&materials, &nuclides, &grid, 0.05);
    let settings = KeffSettings {
        n_particles: 500,
        n_inactive: 15,
        n_active: 35,
        temperature_k: TEMP_K,
        ..KeffSettings::default()
    };
    let r = run_keff_delta_in(
        DeltaDomain::Sphere { radius: R_ROOT },
        &materials,
        &nuclides,
        &majorant,
        |p: Position| pebble.material_at(p),
        &settings,
    );
    let mut fp = Fp::new();
    fp.keff(&r);
    check(
        "fhr_explicit_triso_delta",
        fp.0,
        0x86e84b60e8278ec3,
        &format!(
            "k = {:#018x} +/- {:#018x}",
            r.k_mean.to_bits(),
            r.k_std.to_bits()
        ),
    );
}

#[test]
fn pure_statistics_are_bit_identical() {
    use outram_mc_libs::rng::distributions::{sample_exp, sample_normal, uniform};
    use outram_mc_libs::tally::arithmetic::DerivedTally;
    use outram_mc_libs::tally::trigger::{bin_uncertainty, predict_batches, BinStats};
    use outram_mc_libs::vv::pooled;

    let data = [1.013, 0.998, 1.0071, 1.0022, 0.9954, 1.0103, 1.0005];
    let mut fp = Fp::new();

    let (m, sd, sem) = pooled(&data);
    fp.fs(&[m, sd, sem]);

    let stats = BinStats {
        sum: data.iter().sum(),
        sum_sq: data.iter().map(|x| x * x).sum(),
    };
    let (bm, bsd, brel) = bin_uncertainty(stats, data.len()).unwrap();
    fp.fs(&[bm, bsd, brel]);
    fp.f(predict_batches(120, 20, 1.7).unwrap() as f64);

    let a = DerivedTally::new(vec![4.0, 9.0, 0.0], vec![0.2, 0.3, 0.0]);
    let b = DerivedTally::new(vec![1.3, 2.7, 5.0], vec![0.11, 0.15, 0.4]);
    for d in [
        a.add(&b),
        a.sub(&b),
        a.mul(&b),
        a.div(&b),
        a.scalar_mul(-2.5),
    ] {
        fp.fs(&d.values);
        fp.fs(&d.std_devs);
    }
    let (s, ss) = b.sum();
    fp.fs(&[s, ss]);

    let mut seed = 0x5eed_u64;
    for _ in 0..1000 {
        fp.f(uniform(&mut seed, -2.0, 3.0));
        fp.f(sample_normal(&mut seed));
        fp.f(sample_exp(&mut seed, 2.5));
    }
    fp.f(seed as f64);

    check(
        "pure_statistics",
        fp.0,
        0xf67085fed4325815,
        "pooled, trigger stats, DerivedTally, samplers",
    );
}

#[test]
fn shannon_entropy_is_bit_identical() {
    use outram_mc_libs::geometry::position::Direction;
    use outram_mc_libs::particle::bank::BankSite;
    let mesh = RegularMesh {
        lower_left: [-1.0; 3],
        upper_right: [1.0; 3],
        dimension: [3, 3, 3],
    };
    let mut seed = 99_u64;
    let sites: Vec<BankSite> = (0..500)
        .map(|_| {
            let mut c = || 2.4 * outram_mc_libs::rng::lcg::prn(&mut seed) - 1.2;
            BankSite {
                r: Position::new(c(), c(), c()),
                u: Direction::new(0.0, 0.0, 1.0),
                e: 1.0e6,
                wgt: 1.0,
                seed: 0,
            }
        })
        .collect();
    let h = mesh.shannon_entropy(&sites).unwrap();
    let mut fp = Fp::new();
    fp.f(h);
    check(
        "shannon_entropy",
        fp.0,
        0x5c785611104f059b,
        &format!("H = {:#018x}", h.to_bits()),
    );
}
