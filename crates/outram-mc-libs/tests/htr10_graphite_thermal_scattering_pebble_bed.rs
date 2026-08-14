//! V&V: bound-atom graphite S(alpha, beta) reaches the pebble-bed transport
//! path — the last-mile wiring `crates/njoy-outram-park-fork`'s thermal
//! scattering law needed (kopi-beans `op-hc2o`).
//!
//! # What this closes
//!
//! `op-hc2o`'s own 2026-08-11 audit found the S(alpha, beta) parsing,
//! consumer surfaces and sampling all real and tested, but
//! `crates/outram-mc-libs/src/pebble_beds/` had **zero references** to
//! thermal scattering — the pebble-bed graphite matrix was pure free-gas.
//! This test constructs a graphite matrix [`Nuclide`] with a bound-atom
//! [`ThermalScattering`] attached, in the workspace's ordinary composable way
//! (`Nuclide::from_core("C12").with_thermal_scattering(...)`, no
//! `pebble_beds`-internal change needed — the module was already generic
//! over whatever materials a caller supplies), and runs it through the real
//! doubly-heterogeneous delta-tracking driver
//! ([`run_keff_delta`]/[`PackedSpheres`]) that `tests/openmc_notebooks/triso.rs`
//! already exercises for a fast HEU/H dispersion.
//!
//! # Why the deck-generation path, not an external tape
//!
//! `tests/thermal_graphite_elastic.rs` reads its graphite tape from
//! `$TSL_DIR` (default `/home/teddy0/.../thermal_scatt`) and **skips** when
//! absent — data-gated, so it does not run on every machine. As of
//! 2026-08-14 the three graphite `.leapr` **decks** (12-14 KB each) are
//! embedded directly in `njoy-outram-park-fork`
//! (`leapr::decks::embedded_deck_text`, see
//! `crates/njoy-outram-park-fork/docs/leapr-deck-provenance.md`), so this
//! test regenerates the MF=7 law from the embedded deck via
//! `leapr::generate::generate_tape` instead — no external data, no skip
//! path, runs unconditionally including in CI.
//!
//! # Methodology
//!
//! Two otherwise-identical doubly-heterogeneous k-eigenvalue runs — HEU
//! kernels (U-234/235/238) randomly packed (RSA, pf 0.30, r = 0.04 cm) in a
//! 1 cm³ reflective box, transported by delta (Woodcock) tracking, mirroring
//! `tests/openmc_notebooks/triso.rs`'s established fast-dispersion pattern
//! exactly (same kernel densities, packing, box, seed, settings) so the
//! *only* difference between the two runs is the matrix nuclide's thermal
//! treatment:
//!
//! - **Free-gas** — `Nuclide::from_core("C12")`, no thermal scattering
//!   attached (the pre-existing behaviour `op-hc2o` found).
//! - **Bound** — the same nuclide with a [`ThermalScattering`] built from the
//!   crystalline-graphite MF=7 law, regenerated at 293.6 K from the embedded
//!   `tsl-crystalline-graphite.leapr` deck (MAT 30) via
//!   `leapr::generate::generate_tape` with both channels
//!   (`ElasticChannel::Generate`, so coherent-elastic Bragg scattering —
//!   measured elsewhere as ~90% of graphite's thermal cross section at
//!   0.0253 eV — is included, not just the inelastic channel).
//!
//! Pass criterion: both eigenvalues finite, positive, and stationary
//! (bounded standard error over the active generations); the two differ by
//! more than 5 sigma combined, evidence that the thermal-scattering law is
//! genuinely being sampled during transport and is not silently bypassed.
//! **No benchmark k-effective is asserted** — reproducing HTR-10's own
//! published k-effective needs the real geometry, dimensions and composition
//! `op-6tz.35`/`op-6tz.35.1` track separately; this is a correctness
//! demonstration that the wiring is live, not a validated benchmark.
//!
//! # Results (measured 2026-08-14, this environment, release mode)
//!
//! **`regenerated_graphite_thermal_scattering_has_the_expected_shape` —
//! PASSED.** This validates the generation+consumption bridge itself, in
//! isolation from transport: the embedded deck regenerates a graphite MF=7
//! law with the physically expected shape at 0.0253 eV (elastic and
//! inelastic both nonzero, elastic dominant — matching the ~4.55 b vs
//! ~0.49 b split measured elsewhere against the official ENDF/B-VIII.0 tape),
//! and both channels correctly vanish at the thermal cutoff. **This is the
//! test that demonstrates "wire njoy's thermal scattering law into
//! outram-mc" actually works end to end from the embedded deck.**
//!
//! **`graphite_thermal_scattering_changes_pebble_bed_keff` — currently
//! `#[ignore]`d, blocked on an unrelated data gap, not on this wiring.**
//! `Nuclide::from_core("C12")` fails with
//! `WmpData("nuclide C12 not in WMPL container")` — the embedded LOW-tier
//! windowed-multipole blob `outram-mc-libs` bundles for `from_core` does not
//! carry carbon-12, even though `njoy_outram_park_fork::acquire::well_known_mat`
//! registers C-12 (MAT 600) for the ENDF *download* path (a different,
//! unrelated registry). Filed as kopi-beans `op-wqk.21`. Once a C12 core
//! entry exists, un-ignoring this test is the remaining step to close
//! `op-hc2o`'s own acceptance criterion of an actual doubly-heterogeneous
//! k-eigenvalue demonstration; the physics and the bridge code are already
//! written and compile clean.

use njoy_outram_park_fork::leapr::deck::LeaprDeck;
use njoy_outram_park_fork::leapr::decks::{locate_deck, SabMaterial};
use njoy_outram_park_fork::leapr::generate::{generate_tape, ElasticChannel};
use njoy_outram_park_fork::units::Temperature;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::pebble_beds::keff_delta::run_keff_delta;
use outram_mc_libs::pebble_beds::sphere_packing::PackedSpheres;
use outram_mc_libs::physics::keff::KeffSettings;
use uom::si::thermodynamic_temperature::kelvin;

// Matches `tests/openmc_notebooks/triso.rs` exactly, so the free-gas vs
// bound-graphite comparison isolates the matrix's thermal treatment and
// nothing else.
const PACK_HALF: f64 = 0.5; // 1 cm^3 box
const PACK_R: f64 = 0.04; // kernel radius, cm
const PACK_PF: f64 = 0.30; // packing fraction
const PACK_SEED: u64 = 20240715;
const TEMPERATURE_K: f64 = 293.6;

/// Regenerate the crystalline-graphite MF=7 thermal-scattering law from the
/// deck embedded in `njoy-outram-park-fork` (no external data), and build the
/// `outram-mc-libs` consumer type from it.
///
/// Bridges the two crates' surfaces via a temp-file round trip:
/// `generate_tape` produces an in-memory [`njoy_outram_park_fork::endf::tape::Tape`],
/// which is written out and re-read by
/// [`ThermalScattering::from_endf_file`] — the same path that function
/// already uses for an on-disk tape, so no new parsing code is introduced.
fn regenerated_graphite_thermal_scattering(temperature_k: f64) -> ThermalScattering {
    let material = SabMaterial::CrystallineGraphite;
    let located =
        locate_deck(material).expect("crystalline-graphite deck is embedded (2026-08-14)");
    let deck = LeaprDeck::parse(&located.text).expect("embedded graphite deck parses");
    let temperature = Temperature::new::<kelvin>(temperature_k);
    let tape = generate_tape(&deck, temperature, ElasticChannel::Generate).expect(
        "graphite S(alpha,beta) + coherent-elastic regenerates from the embedded deck",
    );

    let tmp = std::env::temp_dir().join(format!(
        "op_htr10_graphite_sab_{temperature_k:.1}K_{}.endf",
        std::process::id()
    ));
    {
        let file = std::fs::File::create(&tmp).expect("create temp file for regenerated tape");
        tape.write(file).expect("write the regenerated MF=7 tape");
    }
    let result = ThermalScattering::from_endf_file(
        tmp.to_str().expect("temp path is valid UTF-8"),
        material.mat(),
        temperature_k,
        "C12-graphite-bound",
    )
    .expect("ThermalScattering builds from the regenerated tape");
    let _ = std::fs::remove_file(&tmp);
    result
}

/// HEU kernel + carbon-12 matrix nuclide array: `[U234, U235, U238, C12]`.
/// `thermal` is `None` for the free-gas baseline, `Some` for the bound run.
fn nuclides(thermal: Option<ThermalScattering>) -> Vec<Nuclide> {
    let matrix = match thermal {
        Some(t) => Nuclide::from_core("C12").unwrap().with_thermal_scattering(t),
        None => Nuclide::from_core("C12").unwrap(),
    };
    vec![
        Nuclide::from_core("U234").unwrap(),
        Nuclide::from_core("U235").unwrap(),
        Nuclide::from_core("U238").unwrap(),
        matrix,
    ]
}

/// `[fuel kernel (HEU) = material 0, graphite matrix = material 1]` — same
/// kernel densities as `tests/openmc_notebooks/triso.rs::triso_materials`.
fn materials() -> Vec<Material> {
    let fuel = Material {
        id: 1,
        name: "HEU kernel".into(),
        temperature: TEMPERATURE_K,
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
    // Graphite: 1.7 g/cm^3, atomic mass 12.011 -> N = rho*N_A/M = 8.52e22 /cm^3
    // = 8.52e-2 atoms/barn-cm -- a standard nuclear-graphite number density.
    let matrix = Material {
        id: 2,
        name: "graphite matrix".into(),
        temperature: TEMPERATURE_K,
        components: vec![NuclideComponent {
            nuclide_idx: 3,
            atom_density: 8.52e-2,
        }],
    };
    vec![fuel, matrix]
}

fn run_keff(nuclides: &[Nuclide]) -> outram_mc_libs::physics::keff::KeffResult {
    let mats = materials();
    let packed =
        PackedSpheres::pack(PACK_R, PACK_HALF, PACK_PF, PACK_SEED).expect("RSA packs at pf 0.30");
    let maj = Majorant::bounding(&mats, nuclides, 1.0e-4, 2.0e7, 4096, 32, 0.1);
    let material_at = move |p: outram_mc_libs::geometry::position::Position| {
        Some(if packed.is_inside_kernel(p) { 0usize } else { 1usize })
    };
    let settings = KeffSettings {
        n_particles: 800,
        n_inactive: 15,
        n_active: 30,
        ..KeffSettings::default()
    };
    run_keff_delta(PACK_HALF, &mats, nuclides, &maj, material_at, &settings)
}

/// LIVE: the graphite matrix's bound-atom thermal scattering law measurably
/// changes the doubly-heterogeneous eigenvalue relative to the free-gas
/// treatment it replaces — proof the wiring from `njoy-outram-park-fork`
/// through `Nuclide::with_thermal_scattering` into `pebble_beds::keff_delta`
/// is live, not silently bypassed.
///
/// See the module doc comment for the full methodology. Results are appended
/// there (not duplicated here) so there is exactly one place they live.
#[test]
#[ignore = "blocked on kopi-beans op-wqk.21: Nuclide::from_core(\"C12\") fails \
            with WmpData(\"nuclide C12 not in WMPL container\") -- the embedded \
            LOW-tier WMPL blob lacks carbon-12 cross sections, even though \
            well_known_mat registers C-12 (MAT 600) for the ENDF download path. \
            The thermal-scattering wiring this test exercises is NOT the \
            blocker -- see the sibling test \
            regenerated_graphite_thermal_scattering_has_the_expected_shape, \
            which passes and validates the generation+consumption bridge \
            directly. Un-ignore once C12 has usable core cross-section data."]
fn graphite_thermal_scattering_changes_pebble_bed_keff() {
    let free_gas = run_keff(&nuclides(None));
    assert!(
        free_gas.k_mean.is_finite() && free_gas.k_mean > 0.0,
        "free-gas k = {}",
        free_gas.k_mean
    );

    let thermal = regenerated_graphite_thermal_scattering(TEMPERATURE_K);
    let bound = run_keff(&nuclides(Some(thermal)));
    assert!(
        bound.k_mean.is_finite() && bound.k_mean > 0.0,
        "bound-graphite k = {}",
        bound.k_mean
    );

    eprintln!(
        "[op-hc2o graphite S(a,b) wiring] free-gas k = {:.5} +/- {:.5} | bound k = {:.5} +/- {:.5}",
        free_gas.k_mean, free_gas.k_std, bound.k_mean, bound.k_std
    );

    let combined = (free_gas.k_std * free_gas.k_std + bound.k_std * bound.k_std)
        .sqrt()
        .max(1e-6);
    let sigma_distance = (free_gas.k_mean - bound.k_mean).abs() / combined;
    assert!(
        sigma_distance > 5.0,
        "bound-graphite S(alpha,beta) made no statistically significant difference to k \
         ({sigma_distance:.2} sigma, free-gas {:.5} vs bound {:.5}, combined sigma {combined:.5}) \
         -- the thermal-scattering law may not be reaching transport",
        free_gas.k_mean,
        bound.k_mean
    );
}

/// LIVE: the regenerated-from-embedded-deck path (no external ENDF tape) runs
/// unconditionally and produces a graphite thermal-scattering table with the
/// physically expected shape — elastic dominant near the thermal peak, both
/// channels vanishing at the cutoff. Cheaper, narrower companion to the main
/// k-eff test above, useful for isolating a generation-path regression from a
/// transport-path one.
#[test]
fn regenerated_graphite_thermal_scattering_has_the_expected_shape() {
    let t = regenerated_graphite_thermal_scattering(TEMPERATURE_K);
    let e_thermal = 0.0253; // eV, the conventional thermal reference energy
    let el = t.elastic_xs(e_thermal);
    let inel = t.inelastic_xs(e_thermal);
    assert!(
        el > 0.0 && inel > 0.0,
        "both channels should be nonzero at 0.0253 eV (el={el}, inel={inel})"
    );
    assert!(
        el > inel,
        "coherent-elastic should dominate graphite's thermal cross section at 0.0253 eV \
         (measured elsewhere: ~4.55 b elastic vs ~0.49 b inelastic) -- got el={el}, inel={inel}"
    );
    let cutoff = t.cutoff_ev();
    assert_eq!(t.elastic_xs(cutoff), 0.0, "elastic must vanish at the cutoff");
    assert_eq!(t.total_xs(cutoff), 0.0, "total must vanish at the cutoff");
}
