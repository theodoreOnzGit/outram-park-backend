//! **One fixed temperature per region, transported correctly** (gh #945,
//! stage 1). Full record, with methodology and results:
//! `verification_and_validation/region_temperature_945/README.md`.
//!
//! # Methodology
//!
//! 1. **Uniform by region is the uniform model, bit for bit.** Two nested
//!    HEU spheres with every shape and component given the model's own
//!    temperature, against the same model with the model temperature only:
//!    the same `k` per cycle to the last bit (LOW-tier data, so it runs
//!    anywhere; the lowering's own equality is pinned in
//!    `src/model/region_temperature_tests.rs`).
//! 2. **A two-region model through the beginner API is the low-level
//!    path.** An LEU cell (UO2 sphere at 1200 K in a reflective water cube
//!    at 293.6 K, H in H2O on) run through `Model`, against a hand-built
//!    CSG with each material at its temperature and each nuclide built with
//!    `Nuclide::from_endf_file_with_speed` at that temperature (and
//!    `ThermalScattering::from_embedded_leapr_deck` for H in H2O), run by
//!    `run_keff_csg` at the same seed: the same `k` per cycle, bit for bit.
//!    The global nuclide order is the model's, because the URR stream is
//!    keyed by nuclide index.
//! 3. **Doppler sign.** The same cell with the fuel at 293.6 K and at
//!    1200 K (water at 293.6 K in both): `k` must fall, by more than 3
//!    combined sigma.
//! 4. **The guard.** Nuclides processed at 293.6 K handed to the 1200 K
//!    model are refused (`RunError::NuclideTemperature`), never transported.
//! 5. **Majorant at a region's temperature.** A delta-tracked `DhRegion`
//!    (UO2 kernels in bound graphite) at 1200 K in a 293.6 K model: its
//!    majorant (`AssembledModel::majorants`) audited (`Majorant::audit`)
//!    against the region's 1200 K materials must not be under anywhere.
//!    Control, which must fail: the same region's majorant built at
//!    293.6 K, audited against the 1200 K materials.
//!
//! Predictions were posted on gh #945 before any run (comment of
//! 2026-10-18): 1 and 2 bit-identical; 3 a fall of -1500 to -4000 pcm; 5
//! worst ratio <= 1, and the control under by 1.3x to 4x.
//!
//! # Results
//!
//! 2026-10-18, commit after `78b3db6eb9`, i9-13900K, 6 of 32 logical cores
//! (`taskset -c 4-9`, rayon 6 threads), 62 GB RAM, Linux 7.2.9, CPU only;
//! ENDF/B-VIII.0 from `reference-data/endf/`. All five pass.
//!
//! 1. Bit-identical (as predicted).
//! 2. Bit-identical: `k = 1.49292 +/- 0.00172` both ways (as predicted).
//! 3. `k(fuel 293.6 K) = 1.51212 +/- 0.00192`, `k(fuel 1200 K) =
//!    1.49292 +/- 0.00172`: **-1920 +/- 258 pcm in k (7.5 sigma)**, inside
//!    the predicted -1500 to -4000 and in its lower half, as predicted for
//!    a cell more moderated than a PWR pin. In reactivity -851 pcm, an
//!    average -0.94 pcm/K over 293.6-1200 K.
//! 4. Refused, naming U-235 at 293.6 K against a 1200 K region.
//! 5. Own majorant: worst `Σ_t/Σ_maj` **0.769** (at 9.9e-5 eV, UO2) over
//!    857 176 energies; control (293.6 K majorant): **1.314 under at
//!    6.48 eV** (UO2, the U-238 6.67 eV resonance's wing), inside the
//!    predicted 1.3x to 4x and at its low end.

use njoy_outram_park_fork::leapr::decks::SabMaterial;
use njoy_outram_park_fork::reference_data::reference_endf_dir;
use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{
    BoundaryType, Sphere as SphereSurface, SurfaceKind, XPlane, YPlane, ZPlane,
};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material as LowMaterial, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::speed::SpeedTier;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings as LowKeffSettings, ThreadCount};
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use uom::si::f64::{Length, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::thermodynamic_temperature::kelvin;

const T_ROOM: f64 = 293.6;
const T_HOT: f64 = 1200.0;

fn cm(x: f64) -> Length {
    Length::new::<centimeter>(x)
}

fn at(t: f64) -> Temperature {
    Temperature::Uniform(ThermodynamicTemperature::new::<kelvin>(t))
}

/// A library of three uranium headers (masses only), enough to assemble.
fn header_only_library() -> NuclearData {
    let tape = |za: &str, awr: &str| {
        format!(
            " TPID\n{za:>11}{awr:>11}{:>11}{:>11}{:>11}{:>11}\n{:>11}{:>11}{:>11}{:>11}{:>11}{:>11}\n{:>11}{:>11}{:>11}{:>11}{:>11}{:>11}\n",
            0, 0, 0, 0, "0.0", "0.0", 0, 0, 0, 6, "1.0", "2.0+7", 0, 0, 10, 8
        )
        .into_bytes()
    };
    NuclearData::from_endf_bytes(
        "headers only",
        [
            ("u235".to_string(), tape("9.223500+4", "2.330248+2")),
            ("u238".to_string(), tape("9.223800+4", "2.360058+2")),
        ],
    )
    .expect("two incident-neutron headers")
}

fn custom(particles: usize, inactive: usize, active: usize, seed: u64) -> KeffSettings {
    KeffSettings::new(
        MCSettings::Custom(CustomRun {
            particles,
            inactive,
            active,
            seed,
        }),
        None,
    )
    .silent()
}

/// **1. Uniform by region is the uniform model, bit for bit.**
///
/// *Result (2026-10-18):* identical `k` per cycle, mean and std (LOW tier,
/// 300 x [3 + 6]).
#[test]
fn every_region_at_the_model_temperature_is_the_uniform_run_bit_for_bit() {
    let build = |explicit: bool| {
        let mut m = Model::new(header_only_library());
        m.set_temperature(at(T_ROOM));
        let core = m.component("core");
        let heu = |d235: f64| {
            Material::from_atom_densities([
                (U235, atoms_per_barn_cm(d235)),
                (U238, atoms_per_barn_cm(2.5e-3)),
            ])
        };
        let mut inner = Sphere::radius(cm(6.0))
            .filled_with(heu(4.5e-2))
            .in_component(core);
        let mut outer = Sphere::radius(cm(8.7)).filled_with(heu(4.0e-2));
        if explicit {
            m.set_component_temperature(core, at(T_ROOM));
            inner = inner.at_temperature(at(T_ROOM));
            outer = outer.at_temperature(at(T_ROOM));
        }
        m.add(outer.containing(inner));
        m.assemble().unwrap()
    };
    let nuclides: Vec<Nuclide> = ["U235", "U238"]
        .iter()
        .map(|n| Nuclide::from_core(n).unwrap())
        .collect();
    let settings = custom(300, 3, 6, 11);
    let plain = build(false).keff_with(&nuclides, &settings).unwrap();
    let explicit = build(true).keff_with(&nuclides, &settings).unwrap();
    assert_eq!(plain.k_by_cycle, explicit.k_by_cycle);
    assert_eq!(plain.keff().mean.to_bits(), explicit.keff().mean.to_bits());
    assert_eq!(plain.keff().std.to_bits(), explicit.keff().std.to_bits());
    assert_eq!(explicit.provenance.temperatures_k, vec![T_ROOM]);
    assert!(explicit
        .provenance
        .physics
        .iter()
        .all(|n| n.temperature_k == T_ROOM));
}

// ---------------------------------------------------------------------
// The LEU cell (tests 2-4).
// ---------------------------------------------------------------------

/// UO2 at 10.4 g/cm3, 4.5 wt% U-235 (atom densities from those two inputs
/// and the AWRs: N_UO2 = 0.023204 /b-cm, U-235 atom fraction 0.04555).
fn uo2() -> Material {
    Material::from_atom_densities([
        (U235, atoms_per_barn_cm(1.0570e-3)),
        (U238, atoms_per_barn_cm(2.2147e-2)),
        (O16, atoms_per_barn_cm(4.6408e-2)),
    ])
    .named("UO2 4.5 %")
}

/// Light water at 0.998 g/cm3, H in H2O on.
fn water() -> Material {
    Material::from_atom_densities([
        (H1, atoms_per_barn_cm(6.6735e-2)),
        (O16, atoms_per_barn_cm(3.3368e-2)),
    ])
    .bound_in(Moderator::LightWater)
    .named("water")
}

const FUEL_R: f64 = 0.6;
const CELL_SIDE: f64 = 1.6;

/// The cell: a UO2 sphere at `fuel_k` in a reflective water cube; the model
/// (and so the water) at 293.6 K.
fn leu_cell(data: NuclearData, fuel_k: f64) -> Model {
    let mut m = Model::new(data);
    m.set_temperature(at(T_ROOM));
    let fuel = m.component("fuel");
    m.set_component_temperature(fuel, at(fuel_k));
    m.add(
        Cube::side(cm(CELL_SIDE))
            .filled_with(water())
            .containing(
                Sphere::radius(cm(FUEL_R))
                    .filled_with(uo2())
                    .in_component(fuel),
            )
            .boundary(Boundary::Reflective),
    );
    m
}

/// The same cell built by hand, the way a low-level user writes it, in the
/// lowering's surface and token order.
fn hand_built_cell() -> (Geometry, SourceBox) {
    let h = 0.5 * CELL_SIDE;
    let bc = BoundaryType::Reflective;
    let half = |surface_idx, sense| RegionToken::HalfSpace { surface_idx, sense };
    let (o, i) = (HalfSpaceSense::Outside, HalfSpaceSense::Inside);
    let and = RegionToken::Intersection;
    let water_region = vec![
        half(0, o),
        half(1, o),
        half(2, i),
        and,
        half(3, o),
        and,
        half(4, i),
        and,
        half(5, o),
        and,
        half(6, i),
        and,
        and,
    ];
    let geometry = Geometry {
        surfaces: vec![
            SurfaceKind::Sphere(SphereSurface {
                x0: 0.0,
                y0: 0.0,
                z0: 0.0,
                r: FUEL_R,
                bc: BoundaryType::Transmissive,
            }),
            SurfaceKind::XPlane(XPlane { x0: -h, bc }),
            SurfaceKind::XPlane(XPlane { x0: h, bc }),
            SurfaceKind::YPlane(YPlane { y0: -h, bc }),
            SurfaceKind::YPlane(YPlane { y0: h, bc }),
            SurfaceKind::ZPlane(ZPlane { z0: -h, bc }),
            SurfaceKind::ZPlane(ZPlane { z0: h, bc }),
        ],
        cells: vec![
            Cell::material(1, vec![half(0, i)], 0, T_HOT),
            Cell::material(2, water_region, 1, T_ROOM),
        ],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0, 1],
        }],
        lattices: vec![],
        root_universe: 0,
    };
    let src = SourceBox {
        lower: Position::new(-FUEL_R, -FUEL_R, -FUEL_R),
        upper: Position::new(FUEL_R, FUEL_R, FUEL_R),
    };
    (geometry, src)
}

fn endf(file: &str) -> std::path::PathBuf {
    reference_endf_dir().join(file)
}

fn have_tapes(files: &[&str]) -> bool {
    let ok = files.iter().all(|f| endf(f).exists());
    if !ok {
        eprintln!("SKIP: reference ENDF tapes not present");
    }
    ok
}

const U235_TAPE: &str = "n-092_U_235-ENDF8.0.endf";
const U238_TAPE: &str = "n-092_U_238.endf";
const O16_TAPE: &str = "n-008_O_016-ENDF8.0.endf";
const H1_TAPE: &str = "n-001_H_001-ENDF8.0-Beta6.endf";
const C12_TAPE: &str = "n-006_C_012-ENDF8.0.endf";

/// The hand path's nuclides, in the model's order: fuel's U-235, U-238,
/// O-16 at 1200 K, then water's H-1 (H in H2O) and O-16 at 293.6 K.
fn hand_nuclides() -> Vec<Nuclide> {
    use rayon::prelude::*;
    let build = |(file, name, t, law): (&str, &str, f64, Option<SabMaterial>)| {
        let n = Nuclide::from_endf_file_with_speed(&endf(file), name, t, SpeedTier::Fast).unwrap();
        match law {
            None => n,
            Some(m) => n.with_thermal_scattering(
                ThermalScattering::from_embedded_leapr_deck(m, t, &format!("{m:?}")).unwrap(),
            ),
        }
    };
    vec![
        (U235_TAPE, "U235", T_HOT, None),
        (U238_TAPE, "U238", T_HOT, None),
        (O16_TAPE, "O16", T_HOT, None),
        (H1_TAPE, "H1", T_ROOM, Some(SabMaterial::HInH2O)),
        (O16_TAPE, "O16", T_ROOM, None),
    ]
    .into_par_iter()
    .map(build)
    .collect()
}

fn hand_materials(a: &AssembledModel) -> Vec<LowMaterial> {
    let comp = |nuclide_idx, atom_density| NuclideComponent {
        nuclide_idx,
        atom_density,
    };
    // The densities as the model resolved them (atoms/b-cm round trip), so
    // the two sides read the same numbers.
    let d = |m: usize, k: usize| a.materials[m].components[k].atom_density;
    vec![
        LowMaterial {
            id: 1,
            name: "UO2 4.5 %".into(),
            components: vec![comp(0, d(0, 0)), comp(1, d(0, 1)), comp(2, d(0, 2))],
            temperature: T_HOT,
        },
        LowMaterial {
            id: 2,
            name: "water".into(),
            components: vec![comp(3, d(1, 0)), comp(4, d(1, 1))],
            temperature: T_ROOM,
        },
    ]
}

/// **2, 3 and 4: the two-region LEU cell.**
///
/// Run size 5000 x [20 + 100] (seed 945) for the three transport runs.
///
/// *Result (2026-10-18):* PASS. Model and hand path bit-identical,
/// `k = 1.49292 +/- 0.00172`; cold fuel `1.51212 +/- 0.00192`; Doppler
/// -1920 +/- 258 pcm (7.5 sigma); the guard refused. Data 148 s (hot
/// model), 72 s (cold), transport ~5 s per run, hardware as in the module
/// docs. Record: `verification_and_validation/region_temperature_945/`.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "processes 10 nuclide x temperature sets from ENDF and runs three LEU-cell k runs (~minutes); runs by default"
)]
fn leu_cell_two_regions_match_the_low_level_path_and_doppler_lowers_k() {
    if !have_tapes(&[U235_TAPE, U238_TAPE, O16_TAPE, H1_TAPE]) {
        return;
    }
    let data = || NuclearData::workspace_reference().unwrap();
    let hot_model = leu_cell(data(), T_HOT);
    let hot = hot_model.assemble().unwrap();
    assert_eq!(hot.temperatures_k(), vec![T_ROOM, T_HOT]);
    let names: Vec<(String, f64)> = hot
        .nuclides
        .iter()
        .map(|k| (k.isotope.name(), k.temperature.get::<kelvin>()))
        .collect();
    assert_eq!(
        names,
        vec![
            ("U235".into(), T_HOT),
            ("U238".into(), T_HOT),
            ("O16".into(), T_HOT),
            ("H1".into(), T_ROOM),
            ("O16".into(), T_ROOM),
        ]
    );
    let clock = std::time::Instant::now();
    let hot_nuc = hot.load_nuclides(hot_model.data()).unwrap();
    println!("hot model data: {:.1} s", clock.elapsed().as_secs_f64());

    let (particles, inactive, active, seed) = (5000, 20, 100, 945);
    let settings = custom(particles, inactive, active, seed);
    let clock = std::time::Instant::now();
    let hot_run = hot.keff_with(&hot_nuc, &settings).unwrap();
    println!(
        "hot (fuel {T_HOT} K) through the model: {} ({:.1} s)",
        hot_run.keff(),
        clock.elapsed().as_secs_f64()
    );
    println!("{hot_run}");
    assert_eq!(hot_run.provenance.temperatures_k, vec![T_ROOM, T_HOT]);
    assert!(hot_run.provenance.physics.iter().all(|n| n.dbrc.is_ok()));

    // 2. The low-level path, bit for bit.
    let clock = std::time::Instant::now();
    let nuc = hand_nuclides();
    println!("hand-built data: {:.1} s", clock.elapsed().as_secs_f64());
    let (geometry, src) = hand_built_cell();
    let low = LowKeffSettings {
        n_particles: particles,
        n_inactive: inactive,
        n_active: active,
        temperature_k: T_ROOM,
        seed,
        compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(6)),
        ..LowKeffSettings::default()
    };
    let reference = run_keff_csg(&geometry, &hand_materials(&hot), &nuc, src, &low, None);
    println!(
        "hot through run_keff_csg by hand: {:.5} +/- {:.5}",
        reference.k_mean, reference.k_std
    );
    assert_eq!(
        hot_run.k_by_cycle, reference.k_by_generation,
        "the model and the hand-built low-level path give the same k per cycle"
    );
    assert_eq!(hot_run.keff().mean.to_bits(), reference.k_mean.to_bits());

    // 4. The guard: data at the wrong temperature are refused.
    let cold_model = leu_cell(data(), T_ROOM);
    let cold = cold_model.assemble().unwrap();
    assert_eq!(cold.temperatures_k(), vec![T_ROOM]);
    let clock = std::time::Instant::now();
    let cold_nuc = cold.load_nuclides(cold_model.data()).unwrap();
    println!("cold model data: {:.1} s", clock.elapsed().as_secs_f64());
    // The cold model has 4 nuclides (O16 once); the hot one 5. Build a
    // 5-entry list with the fuel's at 293.6 K to hit the temperature check.
    let wrong: Vec<Nuclide> = vec![
        cold_nuc[0].clone(),
        cold_nuc[1].clone(),
        cold_nuc[2].clone(),
        cold_nuc[3].clone(),
        cold_nuc[2].clone(),
    ];
    match hot.keff_with(&wrong, &settings) {
        Err(RunError::NuclideTemperature {
            index,
            got_k,
            wanted_k,
            ..
        }) => {
            assert_eq!(index, 0);
            assert!((got_k - T_ROOM).abs() < 1e-6 && wanted_k == T_HOT);
            println!("guard: refused U235 at {got_k} K for a {wanted_k} K region");
        }
        other => panic!(
            "expected NuclideTemperature, got {:?}",
            other.map(|r| r.keff())
        ),
    }

    // 3. Doppler: the fuel at room temperature.
    let clock = std::time::Instant::now();
    let cold_run = cold.keff_with(&cold_nuc, &settings).unwrap();
    println!(
        "cold (fuel {T_ROOM} K) through the model: {} ({:.1} s)",
        cold_run.keff(),
        clock.elapsed().as_secs_f64()
    );
    let (kc, kh) = (cold_run.keff(), hot_run.keff());
    let dk = kh.mean - kc.mean;
    let sigma = (kc.std * kc.std + kh.std * kh.std).sqrt();
    let rho = 1.0 / kc.mean - 1.0 / kh.mean;
    println!(
        "DOPPLER: k(1200 K) - k(293.6 K) = {:+.0} +/- {:.0} pcm ({:.1} sigma); \
         reactivity {:+.0} pcm, {:+.2} pcm/K",
        dk * 1e5,
        sigma * 1e5,
        dk / sigma,
        rho * 1e5,
        rho * 1e5 / (T_HOT - T_ROOM)
    );
    assert!(
        dk < -3.0 * sigma,
        "raising the fuel temperature must lower k beyond 3 sigma: {dk} +/- {sigma}"
    );
}

// ---------------------------------------------------------------------
// The delta-tracked region (test 5).
// ---------------------------------------------------------------------

fn graphite() -> Material {
    Material::from_atom_densities([(C12, atoms_per_barn_cm(8.5e-2))])
        .bound_in(Moderator::Graphite(GraphiteKind::Crystalline))
        .named("graphite")
}

/// A graphite sphere holding a delta-tracked zone of UO2 kernels (a
/// lattice of 0.025 cm kernels in 0.05 cm graphite particles, pitch
/// 0.15 cm) at `zone_k`; the model and the outer shell at 293.6 K;
/// reflective outer surface.
fn dh_model(data: NuclearData, zone_k: Option<f64>) -> Model {
    let mut m = Model::new(data);
    m.set_temperature(at(T_ROOM));
    let particle = Sphere::radius(cm(0.05))
        .filled_with(graphite())
        .containing(Sphere::radius(cm(0.025)).filled_with(uo2()));
    let mut zone = DhRegion::new(Sphere::radius(cm(0.5)), graphite())
        .with_particles(
            particle,
            Packing::Lattice {
                pitch: cm(0.15),
                offset: [0.5; 3],
            },
        )
        .delta_tracking();
    if let Some(t) = zone_k {
        zone = zone.at_temperature(at(t));
    }
    m.add(
        Sphere::radius(cm(0.6))
            .filled_with(graphite())
            .containing(zone)
            // Reflective: a bare 0.6 cm sphere leaks nearly every neutron
            // (the first version of this test died at generation 0).
            .boundary(Boundary::Reflective),
    );
    m
}

/// **5. The majorant of a delta-tracked region at 1200 K bounds it; one
/// built at 293.6 K does not.** Audit: `Majorant::audit` over
/// 1e-5 eV to 20 MeV, 200 000 log points plus every breakpoint of every
/// nuclide in the region, its one-ulp neighbours and the interval
/// midpoints, against `Σ_t` of the region's materials at 1200 K.
///
/// *Result (2026-10-18):* PASS. Own majorant worst ratio 0.769 (at
/// 9.9e-5 eV, below the 1e-4 eV floor where `Majorant::at` extrapolates
/// as 1/v); 293.6 K control 1.314 under at 6.48 eV. The short delta run
/// gives `k = 1.26 +/- 0.04` (500 x [2 + 3], not a result). The first
/// version had a vacuum shell and died at generation 0; the shell is
/// reflective since. Data 102 s, audit 18 s, hardware as in the module docs.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "processes U-235/238, O-16 and bound C-12 at two temperatures from ENDF (~minutes); runs by default"
)]
fn a_delta_tracked_region_at_1200_k_has_a_majorant_that_bounds_it() {
    if !have_tapes(&[U235_TAPE, U238_TAPE, O16_TAPE, C12_TAPE]) {
        return;
    }
    let data = || NuclearData::workspace_reference().unwrap();
    let hot_model = dh_model(data(), Some(T_HOT));
    let hot = hot_model.assemble().unwrap();
    assert_eq!(hot.delta_regions().len(), 1);
    let region: Vec<LowMaterial> = hot.delta_regions()[0]
        .iter()
        .map(|&i| hot.materials[i].clone())
        .collect();
    assert!(region.iter().all(|m| m.temperature == T_HOT));
    let clock = std::time::Instant::now();
    let hot_nuc = hot.load_nuclides(hot_model.data()).unwrap();
    println!("hot DH data: {:.1} s", clock.elapsed().as_secs_f64());
    let clock = std::time::Instant::now();
    let own = hot.majorants(&hot_nuc).remove(0);
    let audit = |m: &outram_mc_libs::physics::delta_tracking::Majorant| {
        m.audit(&region, &hot_nuc, 1.0e-5, 2.0e7, 200_000)
    };
    let a_own = audit(&own);
    println!(
        "MAJORANT at 1200 K: worst sigma_t/sigma_maj {:.4} at {:.4e} eV (material {}), {} energies ({:.1} s)",
        a_own.worst_ratio,
        a_own.energy_ev,
        region[a_own.material].name,
        a_own.energies_checked,
        clock.elapsed().as_secs_f64()
    );
    assert!(
        a_own.worst_ratio <= 1.0,
        "the region's own majorant is under its 1200 K materials: {a_own:?}"
    );

    // Control: the same region at 293.6 K builds a majorant that must be
    // under the 1200 K materials somewhere, or this audit could not fail.
    let cold_model = dh_model(data(), None);
    let cold = cold_model.assemble().unwrap();
    let cold_nuc = cold.load_nuclides(cold_model.data()).unwrap();
    let stale = cold.majorants(&cold_nuc).remove(0);
    let a_stale = audit(&stale);
    println!(
        "CONTROL 293.6 K majorant against 1200 K materials: worst {:.4} at {:.4e} eV (material {})",
        a_stale.worst_ratio, a_stale.energy_ev, region[a_stale.material].name
    );
    assert!(
        a_stale.worst_ratio > 1.0,
        "the control must show the under-bound a wrong-temperature majorant gives: {a_stale:?}"
    );

    // A short run in the region at 1200 K: the delta-tracked path runs on
    // the region's own majorant, and the provenance lists both temperatures.
    let run = hot.keff_with(&hot_nuc, &custom(500, 2, 3, 7)).unwrap();
    println!("{run}");
    assert_eq!(run.provenance.temperatures_k, vec![T_ROOM, T_HOT]);
    assert!(run.provenance.backend.contains("delta-tracked"));
}

/// **The pictures** (crate rule: geometry is drawn for a human). Writes
/// x-y slices of the two test geometries, as assembled, when
/// `OUTRAM_945_IMAGES` names a directory; otherwise draws them in memory
/// only. Needs only the tape headers.
#[test]
fn the_test_geometries_draw() {
    if !have_tapes(&[U235_TAPE, U238_TAPE, O16_TAPE, H1_TAPE, C12_TAPE]) {
        return;
    }
    use outram_mc_libs::geometry::plot::PlotBasis;
    let out = std::env::var_os("OUTRAM_945_IMAGES").map(std::path::PathBuf::from);
    let data = || NuclearData::workspace_reference().unwrap();
    let cell = leu_cell(data(), T_HOT).assemble().unwrap();
    let dh = dh_model(data(), Some(T_HOT)).assemble().unwrap();
    let images = [
        (
            "leu_cell_xy.png",
            cell.draw_slice(PlotBasis::Xy, 400, "LEU cell: UO2 1200 K, water 293.6 K"),
        ),
        // At z = 0.075 cm, through a layer of particle centres (pitch
        // 0.15 cm, offset half a pitch); z = 0 passes between the layers.
        (
            "dh_region_xy.png",
            dh.draw_window(
                PlotBasis::Xy,
                Position::new(0.0, 0.0, 0.075),
                [1.32, 1.32],
                600,
                "DhRegion at 1200 K in a 293.6 K shell, z = 0.075 cm",
            ),
        ),
    ];
    for (name, img) in &images {
        assert!(img.width >= 400);
        if let Some(dir) = &out {
            img.write_png(&dir.join(name)).unwrap();
            println!("wrote {}", dir.join(name).display());
        }
    }
}
