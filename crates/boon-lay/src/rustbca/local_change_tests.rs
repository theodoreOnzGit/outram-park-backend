// SPDX-License-Identifier: GPL-3.0
//
// NOT upstream RustBCA. boon-lay's own tests of the local changes made to its
// copy of RustBCA (lcpp-org/RustBCA v3.0.0, a356280d); see the module docs in
// mod.rs for the list they pin. Upstream's own unit tests are in tests.rs.

//! Tests of boon-lay's local changes to the RustBCA copy (**not upstream**).

use super::*;

// ---------------------------------------------------------------------------
// Local change 6: the LazyLock tables, replaced by direct evaluation.
// ---------------------------------------------------------------------------

/// Rebuilds upstream's five `LazyLock` tables with upstream's own builder
/// loops (`interactions.rs` / `material.rs` at `a356280d`) and checks that the
/// replacement lookups return every entry **bit for bit**, for every
/// in-range index: 3 x 7260 triangular entries, 14 400 Lindhard-Scharff
/// entries and 120 mean ionisation potentials, 36 300 in all.
#[test]
fn lazylock_replacements_match_upstream_tables() {
    let mut checked = 0usize;

    // interactions.rs: COULOMB_CONSTANT_TABLE, LINDHARD_SCREENING_LENGTH_TABLE,
    // ZBL_SCREENING_LENGTH_TABLE. Upstream builder:
    //   for i in 0..Z_MAX { for j in 0..=i { array[(i*(i+1))/2 + j] = f(i, j) } }
    // Upstream lookup: table[triangular_index(Za as usize, Zb as usize)], i.e.
    // the pair reordered so that i >= j.
    for i in 0..Z_MAX {
        for j in 0..=i {
            let coulomb_table = interactions::coulomb_constant(i as f64, j as f64);
            let lindhard_table = interactions::lindhard_screening_length(i as f64, j as f64);
            let zbl_table = interactions::zbl_screening_length(i as f64, j as f64);
            // Both argument orders, and non-integer Z (upstream truncates).
            for (za, zb) in [(i as f64, j as f64), (j as f64 + 0.25, i as f64 + 0.75)] {
                assert_eq!(interactions::coulomb_constant_lookup(za, zb).to_bits(), coulomb_table.to_bits(),
                    "Coulomb constant at ({za}, {zb})");
                assert_eq!(interactions::coulomb(1.0, za, zb).to_bits(), coulomb_table.to_bits(),
                    "coulomb(r = 1) at ({za}, {zb})");
            }
            for (za, zb) in [(i as u64, j as u64), (j as u64, i as u64)] {
                assert_eq!(interactions::lindhard_screening_length_lookup(za, zb).to_bits(), lindhard_table.to_bits(),
                    "Lindhard screening length at ({za}, {zb})");
                assert_eq!(interactions::zbl_screening_length_lookup(za, zb).to_bits(), zbl_table.to_bits(),
                    "ZBL screening length at ({za}, {zb})");
            }
            checked += 3;
        }
    }
    assert_eq!(checked, 3 * TABLE_SIZE);

    // material.rs: LS_STOPPING_CONSTANT_TABLE. Upstream builder:
    //   std::array::from_fn(|i| f((i / Z_MAX) as f64, (i % Z_MAX) as f64))
    // Upstream lookup: table[Za as usize * Z_MAX + Zb as usize].
    for index in 0..Z_MAX * Z_MAX {
        let table = material::lindhard_scharff_stopping_power_constant((index / Z_MAX) as f64, (index % Z_MAX) as f64);
        let (za, zb) = ((index / Z_MAX) as f64, (index % Z_MAX) as f64);
        assert_eq!(material::ls_stopping_constant_lookup(za, zb).to_bits(), table.to_bits(),
            "Lindhard-Scharff constant at index {index}");
        // Through the public cross section: E/Ma = 1 makes it the constant itself.
        assert_eq!(material::lindhard_scharff_stopping_power_cross_section(za, zb, 1.0, 1.0).to_bits(), table.to_bits());
        checked += 1;
    }

    // material.rs: BV_EMPIRICAL_MEAN_IONIZATON_POT_TABLE. Upstream builder,
    // copied verbatim apart from the Real:: routing of powf:
    for Zb in 0..Z_MAX {
        let I0 = if (Zb as f64) < 13. {
            12. + 7./ Zb as f64
        } else {
            9.76 + 58.5*Real::powf(Zb as f64, -1.19)
        };
        let table = (Zb as f64)*I0*Q;
        assert_eq!(material::bv_empirical_mean_ionization_potential_lookup(Zb as f64).to_bits(), table.to_bits(),
            "mean ionisation potential at Zb = {Zb}");
        checked += 1;
    }
    assert_eq!(checked, 3 * TABLE_SIZE + Z_MAX * Z_MAX + Z_MAX);
    assert_eq!(checked, 36_300);
}

/// Upstream's tables panicked on an out-of-range atomic number (index out of
/// bounds); the replacement keeps that.
#[test]
#[should_panic(expected = "index out of bounds")]
fn screening_length_lookup_still_panics_past_z_max() {
    let _ = interactions::lindhard_screening_length_lookup(Z_MAX as u64, 1);
}

/// As above, for the Lindhard-Scharff and mean-ionisation-potential lookups.
#[test]
#[should_panic(expected = "index out of bounds")]
fn mean_ionisation_lookup_still_panics_past_z_max() {
    let _ = material::bv_empirical_mean_ionization_potential_lookup(Z_MAX as f64);
}

/// As above, for the Lindhard-Scharff constant (flat Z_MAX x Z_MAX index).
#[test]
#[should_panic(expected = "index out of bounds")]
fn ls_constant_lookup_still_panics_past_the_table() {
    let _ = material::ls_stopping_constant_lookup(Z_MAX as f64, 0.0);
}

// ---------------------------------------------------------------------------
// Local changes 5 and 7: no std paths, no std-only float methods.
// ---------------------------------------------------------------------------

/// Every file of the copy, as compiled into boon-lay.
const SOURCES: [(&str, &str); 16] = [
    ("mod.rs", include_str!("mod.rs")),
    ("bca.rs", include_str!("bca.rs")),
    ("consts.rs", include_str!("consts.rs")),
    ("enums.rs", include_str!("enums.rs")),
    ("geometry.rs", include_str!("geometry.rs")),
    ("input.rs", include_str!("input.rs")),
    ("interactions.rs", include_str!("interactions.rs")),
    ("material.rs", include_str!("material.rs")),
    ("math.rs", include_str!("math.rs")),
    ("output.rs", include_str!("output.rs")),
    ("particle.rs", include_str!("particle.rs")),
    ("physics.rs", include_str!("physics.rs")),
    ("simple.rs", include_str!("simple.rs")),
    ("sphere.rs", include_str!("sphere.rs")),
    ("structs.rs", include_str!("structs.rs")),
    ("tests.rs", include_str!("tests.rs")),
];

/// The code part of a line (drops a `//` comment; good enough for this tree,
/// whose strings contain no `//`).
fn code_of(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// Local change 7: no transcendental is called with method syntax, which in
/// boon-lay's std build would silently resolve to `f64`'s inherent std method
/// (the platform libm) instead of `petir::real::Real` (libm). `scripts/
/// check-rustbca-no-std.sh` cannot catch this: in a no_std build the method
/// syntax resolves to `Real` and compiles.
#[test]
fn no_transcendental_method_calls_remain() {
    let names = ["exp", "ln", "powf", "powi", "sin", "cos", "tan", "acos", "asin", "atan", "atan2",
        "sin_cos", "sinh", "cosh", "tanh", "cbrt", "log10", "log2", "exp_m1", "ln_1p", "hypot", "exp2"];
    let mut offenders = Vec::new();
    for (file, src) in SOURCES {
        for (n, line) in src.lines().enumerate() {
            let code = code_of(line);
            for name in names {
                let pat = format!(".{name}(");
                // `Real::exp(` is the routed form; `.exp(` preceded by `Real:` cannot occur.
                if code.contains(&pat) {
                    offenders.push(format!("{file}:{}: {}", n + 1, line.trim()));
                }
            }
        }
    }
    assert!(offenders.is_empty(), "method-syntax transcendentals (route through Real::):\n{}", offenders.join("\n"));
}

/// Local changes 5 and 6: no lazily initialised statics, and every `std::`
/// path or std-only macro sits on a line directly gated by `rustbca-std`, or
/// inside an item so gated, or in the test modules. The authoritative check
/// is `scripts/check-rustbca-no-std.sh`, which compiles the tree as
/// `#![no_std]`; this keeps the obvious cases out between runs of it.
#[test]
fn std_only_constructs_are_gated() {
    let gated_files = ["simple.rs", "tests.rs"]; // whole module gated (std / cfg(test))
    let mut offenders = Vec::new();
    for (file, src) in SOURCES {
        for (n, line) in src.lines().enumerate() {
            let code = code_of(line);
            if code.contains("LazyLock") || code.contains("OnceLock") || code.contains("thread_local!") {
                offenders.push(format!("{file}:{}: lazy static: {}", n + 1, line.trim()));
            }
        }
        if gated_files.contains(&file) {
            continue;
        }
        // Track whether we are inside an item gated on rustbca-std by brace depth.
        let lines: Vec<&str> = src.lines().collect();
        let mut gate_depth: Option<i64> = None;
        let mut depth: i64 = 0;
        let mut pending_gate = false;
        for (n, line) in lines.iter().enumerate() {
            let code = code_of(line);
            let trimmed = code.trim();
            if trimmed.starts_with("#[cfg(") && trimmed.contains("feature = \"rustbca-std\"") && !trimmed.contains("not(feature = \"rustbca-std\")") {
                pending_gate = true;
                continue;
            }
            let inside_gate = gate_depth.is_some() || pending_gate;
            let std_use = code.contains("std::") && !code.contains("core::") || code.contains("println!")
                || code.contains("rand::random") || code.contains("rand::rng()")
                || code.contains("getrandom::");
            if std_use && !inside_gate {
                offenders.push(format!("{file}:{}: ungated std use: {}", n + 1, line.trim()));
            }
            let opens = code.matches('{').count() as i64;
            let closes = code.matches('}').count() as i64;
            if pending_gate && !trimmed.starts_with("#[") && !trimmed.starts_with("///") && !trimmed.is_empty() {
                pending_gate = false;
                if opens > closes && gate_depth.is_none() {
                    gate_depth = Some(depth);
                }
            }
            depth += opens - closes;
            if let Some(d) = gate_depth {
                if depth <= d {
                    gate_depth = None;
                }
            }
        }
    }
    assert!(offenders.is_empty(), "std-only constructs outside a rustbca-std gate:\n{}", offenders.join("\n"));
}

/// Local change 2: every copied file carries the workspace provenance header.
#[test]
fn every_copied_file_carries_the_provenance_header() {
    for (file, src) in SOURCES {
        let head: String = src.lines().take(16).collect::<Vec<_>>().join("\n");
        assert!(head.contains("Upstream project : RustBCA"), "{file}: no provenance header");
        assert!(head.contains("a356280d"), "{file}: header does not name the upstream commit");
        assert!(head.contains("GPL"), "{file}: header does not name the licence");
    }
}

// ---------------------------------------------------------------------------
// Local change 9: entropy seeding.
// ---------------------------------------------------------------------------

/// Without `rustbca-std` there is no entropy source, and a negative seed must
/// say so rather than silently pick a fixed seed.
#[cfg(not(feature = "rustbca-std"))]
#[test]
#[should_panic(expected = "entropy seed")]
fn entropy_seed_panics_without_std() {
    let _ = entropy_seed();
}

/// With `rustbca-std` it is an OS-entropy `u64` (upstream: `rand::random()`;
/// `getrandom` since gh:#995): two draws differ (a 2^-64 chance of a false
/// failure). The same for the 32-byte seed `simple_bca` uses.
#[cfg(feature = "rustbca-std")]
#[test]
fn entropy_seed_draws_from_entropy_with_std() {
    assert_ne!(entropy_seed(), entropy_seed());
    assert_ne!(entropy_seed_bytes(), entropy_seed_bytes());
}

/// Local change 12 (gh:#995): no `rand` or `rand_distr` path remains in the
/// copy; the generator and the distributions are `petir::rng`'s.
#[test]
fn no_rand_crate_paths_remain() {
    let mut offenders = Vec::new();
    for (file, src) in SOURCES {
        for (n, line) in src.lines().enumerate() {
            let code = code_of(line);
            if code.contains("rand::") || code.contains("rand_distr") || code.contains("rand_core") {
                offenders.push(format!("{file}:{}: {}", n + 1, line.trim()));
            }
        }
    }
    assert!(offenders.is_empty(), "rand paths left in the RustBCA copy:\n{}", offenders.join("\n"));
}

// ---------------------------------------------------------------------------
// Determinism: same seed, same cascade, bit for bit.
// ---------------------------------------------------------------------------

/// A 200 keV alpha with full recoil cascades in a semi-infinite UO2-like
/// target (Mesh0D), INTERPOLATED electronic stopping. Returns every output
/// particle's state as raw bits.
fn alpha_cascade_bits(seed: u64) -> Vec<[u64; 9]> {
    let mut options = Options::default_options(true);
    options.electronic_stopping_mode = ElectronicStoppingMode::INTERPOLATED;
    let material_parameters = material::MaterialParameters {
        energy_unit: "EV".to_string(),
        mass_unit: "AMU".to_string(),
        Eb: vec![0.0, 2.58],
        Es: vec![5.19, 2.58],
        Ec: vec![3.0, 2.0],
        Ed: vec![0.0, 0.0],
        Z: vec![92.0, 8.0],
        m: vec![238.05, 15.995],
        interaction_index: vec![0, 0],
        surface_binding_model: SurfaceBindingModel::AVERAGE,
        bulk_binding_model: BulkBindingModel::INDIVIDUAL,
    };
    let geometry_input = geometry::Mesh0DInput {
        length_unit: "ANGSTROM".to_string(),
        densities: vec![0.0232, 0.0464],
        electronic_stopping_correction_factor: 1.0,
    };
    let material = material::Material::<Mesh0D>::new(&material_parameters, &geometry_input);
    let alpha = particle::Particle::new(4.0026*AMU, 2.0, 200.0e3*EV, 1.0*EV, 0.0, 0.0,
        1.0*MICRON, 0.0, 0.0, 1.0, 0.3, -0.2, true, false, 0);
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    bca::single_ion_bca(alpha, &material, &options, &mut rng)
        .iter()
        .map(|p| [p.Z.to_bits(), p.m.to_bits(), p.E.to_bits(), p.pos.x.to_bits(), p.pos.y.to_bits(),
            p.pos.z.to_bits(), p.dir.x.to_bits(), p.dir.y.to_bits(), p.dir.z.to_bits()])
        .collect()
}

/// Same seed: identical output, every particle, every bit. A different seed:
/// a different cascade (so the test is not passing vacuously).
#[test]
fn same_seed_gives_a_bit_identical_cascade() {
    let a = alpha_cascade_bits(20261016);
    let b = alpha_cascade_bits(20261016);
    assert!(a.len() > 1, "expected recoils as well as the alpha, got {} particles", a.len());
    assert_eq!(a, b);
    let c = alpha_cascade_bits(20261017);
    assert_ne!(a, c);
}

// ---------------------------------------------------------------------------
// Local change 12 (gh:#995): the RNG is petir::rng, not rand. Seeded results
// are pinned to the bits the rand 0.10.3 / rand_distr 0.6.0 build produced.
// ---------------------------------------------------------------------------

/// FNV-1a 64 over the little-endian bytes of each value.
fn fnv(values: impl IntoIterator<Item = u64>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for v in values {
        for b in v.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// Every output particle of the 200 keV alpha cascade above, for three
/// seeds, digested bit for bit, and the particle counts.
///
/// **Methodology.** The digests and counts were printed by this test on the
/// base commit `38ce4d6c59`, where the copy still drew from `rand` 0.10.3
/// (`ChaCha8Rng`, `random::<f64>()`), before the switch to `petir::rng`
/// (gh:#995). Pass criterion: identical after the switch. A cascade draws
/// thousands of `random::<f64>()` (azimuths, free-flight paths, collision
/// partners), so any difference in the generator or in the float sampling
/// changes the digest.
///
/// **Prediction (written before the switch):** identical, since PETIR's
/// streams equal `rand`'s draw for draw and a cascade samples no `Normal`.
///
/// **Result (2026-10-20, after the switch to `petir::rng`):** identical, all
/// three (781, 1379 and 765 particles).
#[test]
fn seeded_cascades_are_pinned_across_the_rng_swap() {
    let got: Vec<(u64, usize, u64)> = [20261016u64, 1, 876]
        .into_iter()
        .map(|seed| {
            let bits = alpha_cascade_bits(seed);
            (seed, bits.len(), fnv(bits.iter().flatten().copied()))
        })
        .collect();
    assert_eq!(got, PINNED_CASCADES, "seeded cascades moved: (seed, particles, digest)");
}

/// (seed, output particles, FNV-1a digest of their bits), printed on the
/// `rand` build at `38ce4d6c59`.
const PINNED_CASCADES: [(u64, usize, u64); 3] = [
    (20261016, 781, 0x8dd8977ae396a0a6),
    (1, 1379, 0x41e22a89f017be61),
    (876, 765, 0xfcfb6a23de5aa8d1),
];

// ---------------------------------------------------------------------------
// Local change 8: the std layer still works, with the workspace's toml 0.8.
// ---------------------------------------------------------------------------

/// Upstream's own `examples/titanium_dioxide_0D.toml` (at `a356280d`), with
/// `N` cut from 100000 to 20 and the `distributions` fields (a feature not
/// carried here; serde ignores unknown keys) left in place. Parsed through the
/// gated `InputFile::new`, unpacked by `process_input_file` and run by
/// `silent_physics_loop`.
#[cfg(all(feature = "rustbca-std", not(target_arch = "wasm32")))]
#[test]
fn std_layer_parses_and_runs_an_upstream_input_file() {
    let toml_text = r#"
[options]
name = "10.0"
track_trajectories = false
track_recoils = false
track_recoil_trajectories = false
write_buffer_size = 8000
weak_collision_order = 3
suppress_deep_recoils = false
high_energy_free_flight_paths = false
electronic_stopping_mode = "LOW_ENERGY_NONLOCAL"
mean_free_path_model = {"THRESHOLD"={density=1E28}}
interaction_potential = [["KR_C"]]
scattering_integral = [["MENDENHALL_WELLER"]]
root_finder = [[{"NEWTON"={max_iterations=100, tolerance=1E-6}}]]
num_threads = 4
num_chunks = 100
track_displacements = false
track_energy_losses = false
energy_min = 0.0
energy_max = 2000.0
energy_num = 20
angle_min = 0.0
angle_max = 90.0
angle_num = 20
x_min = 0.0
y_min = -0.1
z_min = -0.1
x_max = 0.1
y_max = 0.1
z_max = 0.1
x_num = 20
y_num = 20
z_num = 20

[particle_parameters]
length_unit = "1e-6"
energy_unit = "EV"
mass_unit = "AMU"
N = [ 20,]
m = [ 4.002602,]
Z = [ 2,]
E = [ 2000.0,]
Ec = [ 0.01,]
Es = [ 0.0,]
interaction_index = [0]
pos = [ [ 0.0, 0.0, 0.0,],]
dir = [ [ 0.999, 0.001, 0.0,],]
particle_input_filename = ""

[geometry_input]
length_unit = "MICRON"
densities = [ 3.0e+10, 6.0e+10, 0.0, 0.0 ]
electronic_stopping_correction_factor = 1.0

[material_parameters]
energy_unit = "EV"
mass_unit = "AMU"
Eb = [ 3.0, 2.58, 3.0, 0.0,]
Es = [ 4.84, 2.58, 3.39, 4.72,]
Ec = [ 3.5, 2.0, 3.0, 1.5,]
Z = [ 22, 8, 13, 14,]
m = [ 47.867, 15.9994, 26.98, 28.08553,]
interaction_index = [0, 0, 0, 0]
surface_binding_model = {"PLANAR"={calculation="TARGET"}}
bulk_binding_model = "AVERAGE"
"#;
    let input = <Input0D as InputFile>::new(toml_text);
    let (particles, material, options, units) = input::process_input_file::<Mesh0D>(input);
    assert_eq!(particles.len(), 20);
    assert_eq!(options.seed, 0);
    let finished = physics::silent_physics_loop(particles, material, options, units);
    // track_recoils = false: exactly the 20 incident helium ions come back.
    assert_eq!(finished.len(), 20);
    assert!(finished.iter().all(|p| p.incident && p.Z == 2.0));
}

/// The `rustbca-std` path the alpha test cannot reach: `process_input_file`
/// sampling `UNIFORM` and `NORMAL` particle distributions (`Uniform<f64>`,
/// `Normal<f64>`) from `seed_from_u64(7)`, then `silent_physics_loop`, which
/// gives each particle its own stream with `set_stream`.
///
/// **Methodology.** 20 000 helium ions with a normal energy (2000 eV, sd
/// 300 eV), a uniform-and-normal birth point and a uniform-and-normal
/// direction, so the sampler makes 60 000 normal draws (about 16 of them in
/// the ziggurat tail) and 60 000 uniform ones. The inputs are digested, then
/// the first 40 are run with recoils off and their outputs digested (sorted,
/// so the digest does not depend on thread scheduling). The constants were
/// printed on the base commit `38ce4d6c59` (`rand` 0.10.3, `rand_distr`
/// 0.6.0, whose `exp`/`ln` there were glibc's because `num-traits/std` was
/// unified on). Pass criterion: identical after the switch to `petir::rng`.
///
/// **Prediction (written before the switch):** identical with about 93 %
/// probability. The ~16 tail draws among the 60 000 normal ones each have a
/// ~0.45 % chance of differing by 1 ulp (glibc `ln` before, `libm` after;
/// measured in `petir/tests/rng_vs_rand.rs`), which would move only the
/// input digest.
///
/// **Result (2026-10-20, after the switch to `petir::rng`):** identical,
/// inputs and outputs (40 particles).
#[cfg(all(feature = "rustbca-std", not(target_arch = "wasm32")))]
#[test]
fn seeded_input_sampling_and_physics_loop_are_pinned_across_the_rng_swap() {
    let toml_text = r#"
[options]
name = "pin"
track_trajectories = false
track_recoils = false
track_recoil_trajectories = false
write_buffer_size = 8000
weak_collision_order = 3
suppress_deep_recoils = false
high_energy_free_flight_paths = false
electronic_stopping_mode = "LOW_ENERGY_NONLOCAL"
mean_free_path_model = "LIQUID"
interaction_potential = [["KR_C"]]
scattering_integral = [["MENDENHALL_WELLER"]]
root_finder = [[{"NEWTON"={max_iterations=100, tolerance=1E-6}}]]
num_threads = 1
num_chunks = 1
seed = 7

[particle_parameters]
length_unit = "1e-6"
energy_unit = "EV"
mass_unit = "AMU"
N = [ 20000,]
m = [ 4.002602,]
Z = [ 2,]
E = [ {mean = 2000.0, std = 300.0},]
Ec = [ 0.01,]
Es = [ 0.0,]
interaction_index = [0]
pos = [ [ {min = -0.001, max = 0.0}, {mean = 0.0, std = 0.01}, {min = -0.01, max = 0.01},],]
dir = [ [ {min = 0.99, max = 0.999}, {mean = 0.0, std = 0.01}, 0.0,],]
particle_input_filename = ""

[geometry_input]
length_unit = "MICRON"
densities = [ 3.0e+10, 6.0e+10,]
electronic_stopping_correction_factor = 1.0

[material_parameters]
energy_unit = "EV"
mass_unit = "AMU"
Eb = [ 3.0, 2.58,]
Es = [ 4.84, 2.58,]
Ec = [ 3.5, 2.0,]
Z = [ 22, 8,]
m = [ 47.867, 15.9994,]
interaction_index = [0, 0]
surface_binding_model = {"PLANAR"={calculation="TARGET"}}
bulk_binding_model = "AVERAGE"
"#;
    let input = <Input0D as InputFile>::new(toml_text);
    let (particles, material, options, units) = input::process_input_file::<Mesh0D>(input);
    assert_eq!(particles.len(), 20000);
    let input_digest = fnv(particles.iter().flat_map(|p| {
        [p.E, p.x, p.y, p.z, p.ux, p.uy, p.uz].map(f64::to_bits)
    }));
    let first: Vec<particle::ParticleInput> = particles.into_iter().take(40).collect();
    let finished = physics::silent_physics_loop(first, material, options, units);
    let mut out: Vec<[u64; 7]> = finished.iter().map(|p| [p.E.to_bits(), p.pos.x.to_bits(), p.pos.y.to_bits(),
        p.pos.z.to_bits(), p.dir.x.to_bits(), p.dir.y.to_bits(), p.dir.z.to_bits()]).collect();
    out.sort_unstable();
    let output_digest = fnv(out.iter().flatten().copied());
    assert_eq!((input_digest, finished.len(), output_digest), PINNED_STD_LAYER,
        "seeded input sampling or physics loop moved: (input digest, outputs, output digest)");
}

/// (input digest, output particles, output digest), printed on the `rand`
/// build at `38ce4d6c59`.
#[cfg(all(feature = "rustbca-std", not(target_arch = "wasm32")))]
const PINNED_STD_LAYER: (u64, usize, u64) = (0x145b812745bacc63, 40, 0x1b04f995f992001a);
