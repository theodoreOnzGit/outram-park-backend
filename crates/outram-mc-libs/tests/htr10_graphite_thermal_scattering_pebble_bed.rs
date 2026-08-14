//! V&V: bound-atom graphite S(alpha, beta) reaches the pebble-bed transport
//! path — the last-mile wiring `crates/njoy-outram-park-fork`'s thermal
//! scattering law needed (kopi-beans `op-hc2o`).
//!
//! # What this closes
//!
//! `op-hc2o`'s 2026-08-11 audit found the S(alpha, beta) parsing, consumer
//! surfaces and sampling all real and tested, but `pebble_beds/` had **zero
//! references** to thermal scattering — the graphite matrix was pure free-gas.
//! Its coordinator note asked for three things before the bead could close: an
//! actual doubly-heterogeneous k-eigenvalue demonstration, a **re-checked**
//! (not assumed) delta-tracking majorant, and an end-to-end number. All three
//! are below.
//!
//! # The wiring defect this test exposed (2026-08-14)
//!
//! An earlier revision of this file was `#[ignore]`d and so never ran. When it
//! was un-ignored, the free-gas and bound runs returned **bit-identical**
//! eigenvalues — the signature of a law that never reaches transport. Two
//! independent causes, both since fixed:
//!
//! 1. **`Nuclide::sample_thermal` had exactly one caller in the crate**
//!    (`physics::transport_csg`). Neither `physics::keff` (three collision
//!    sites) nor `pebble_beds::keff_delta` called it. So `xs_at_energy`
//!    correctly substituted the **bound cross section** — setting collision
//!    *rates* — while the **secondary energy and angle were still drawn
//!    free-gas**. That combination is internally inconsistent and, critically,
//!    admits no up-scatter, so a graphite-moderated spectrum cannot
//!    equilibrate. All five collision sites now route through the bound-atom
//!    law below the table cutoff.
//! 2. **The test problem was fast.** It mirrored
//!    `tests/openmc_notebooks/triso.rs`'s HEU dispersion (pf 0.30 in a 1 cm³
//!    box, k ~ 2.08), where neutrons are absorbed long before thermalizing, so
//!    the law was never sampled *by construction* and a null result meant
//!    nothing. See `diagnose_thermal_spectrum_reach`, which measures this
//!    rather than assuming it.
//!
//! # Why the deck-generation path, not an external tape
//!
//! `tests/thermal_graphite_elastic.rs` reads its graphite tape from `$TSL_DIR`
//! and **skips** when absent — data-gated, so it does not run on every
//! machine. The three graphite `.leapr` decks are embedded in
//! `njoy-outram-park-fork` (`leapr::decks`, see
//! `crates/njoy-outram-park-fork/docs/leapr-deck-provenance.md`), so this test
//! regenerates the MF=7 law from the embedded deck via
//! `leapr::generate::generate_tape` — no external data, no skip path.
//!
//! # Methodology
//!
//! Two otherwise-identical doubly-heterogeneous k-eigenvalue runs. HEU kernels
//! (U-234/235/238, `triso.rs`'s densities) randomly packed by RSA at
//! **pf = 5e-4, r = 0.04 cm** in a **6 cm reflective cube**, transported by
//! delta (Woodcock) tracking through `run_keff_delta`. The *only* difference
//! between the runs is the matrix nuclide's thermal treatment:
//!
//! - **Free-gas** — `Nuclide::from_core("C0")`, no thermal law attached.
//! - **Bound** — the same nuclide `.with_thermal_scattering(...)`, the MF=7 law
//!   regenerated at 293.6 K from the embedded `tsl-crystalline-graphite.leapr`
//!   deck (MAT 30) with `ElasticChannel::Generate`, so the coherent-elastic
//!   Bragg channel (~90% of graphite's thermal cross section at 0.0253 eV) is
//!   included, not just the inelastic channel.
//!
//! **The packing fraction is the load-bearing parameter, and it is low on
//! purpose.** At pf 5e-4 the C/U-235 atom ratio is ~3.8e3, in the order of
//! magnitude of a real HTR-10 pebble (~1e4). At the `triso.rs` value of 0.30 —
//! and even at 5e-3 — a thermal neutron is absorbed within a collision or two,
//! so *how* it thermalizes barely affects k and the comparison has no power to
//! detect the law at all (measured: 0.20 sigma at pf 5e-3). This is a
//! sensitivity property of the test, not a tuning knob to reach a threshold.
//!
//! Pass criterion: both eigenvalues finite and positive, and separated by more
//! than 5 sigma combined — evidence the law is genuinely sampled in transport
//! and not silently bypassed. **No benchmark k-effective is asserted.**
//! Reproducing HTR-10's published k-effective needs the real geometry,
//! dimensions and composition, tracked separately under `op-6tz.35` /
//! `op-6tz.35.1`. This is a correctness demonstration that the wiring is live.
//!
//! # Results (measured 2026-08-14, this environment, release mode)
//!
//! **`graphite_thermal_scattering_changes_pebble_bed_keff` — PASSES.**
//!
//! | Matrix treatment | k | sigma |
//! |---|---|---|
//! | Free-gas C-nat | 1.08838 | 0.01851 |
//! | Bound graphite S(alpha, beta) | 1.95254 | 0.01407 |
//!
//! Separation **37 sigma** (delta-k = 0.86416, combined sigma 0.02325); 300
//! particles, 8 inactive + 15 active generations, ~107 s. A confirming run at
//! 800 particles / 10 + 25 generations gave 1.07579 +/- 0.00833 vs
//! 1.94166 +/- 0.00626 — **83 sigma**, same physics, ~410 s.
//!
//! *Interpretation.* The direction and size are physically sensible and are
//! the reason S(alpha, beta) is mandatory for graphite-moderated systems: the
//! bound law holds a Maxwellian near 0.025 eV where U-235 fission dominates,
//! whereas the free-gas kernel — stationary-target, no up-scatter — lets
//! neutrons slide below thermal equilibrium into parasitic 1/v capture. Note
//! this is the difference between a **correct** and an **incorrect** treatment
//! of the same reactor, not a physical design change; it is a measure of how
//! badly a free-gas graphite matrix misrepresents a thermal pebble bed.
//!
//! **`majorant_still_bounds_total_xs_with_bound_graphite` — PASSES.** Worst
//! `Sigma_t / Sigma_maj` = **0.909091** over 200k log-spaced points from 1e-4
//! to 4.2 eV across both materials — exactly 1/1.1, i.e. the grid captures the
//! true per-bin peak and the full 10% margin survives. This answers
//! `op-hc2o`'s explicit warning that a Woodcock majorant which under-bounds
//! `Sigma_t` is a **silent bias, not a crash**, and had to be measured once the
//! bound channel raised `Sigma_t` below 4 eV.
//!
//! **`regenerated_graphite_thermal_scattering_has_the_expected_shape` —
//! PASSES.** Validates the generation-to-consumption bridge in isolation from
//! transport: the embedded deck regenerates a law with the expected shape at
//! 0.0253 eV (both channels nonzero, elastic dominant, matching the ~4.55 b vs
//! ~0.49 b split measured against the official ENDF/B-VIII.0 tape), and both
//! channels vanish at the cutoff.
//!
//! **`diagnose_thermal_spectrum_reach` — diagnostic, `#[ignore]`d, asserts
//! nothing.** Homogenized infinite-medium sweep, 20k histories at pf 5e-4's
//! sibling value 5e-3: **34.41%** of fission-born neutrons reach below the
//! 4 eV cutoff after a mean of **67.4** scatters (min-energy percentiles p01
//! 3.02e0, p50 1.69e1, p90 9.41e2 eV). This is what distinguishes "the wiring
//! is dead" from "the problem is a fast spectrum" — without it, a null k
//! result is uninterpretable.
//!
//! # What this does NOT establish
//!
//! No benchmark comparison. The reference for every number here is this
//! workspace's own njoy port, so these are **internal-consistency** gates, not
//! an independent NJOY/OpenMC-ACE oracle. The geometry is a packed cube, not
//! HTR-10. AI-assisted draft, not human-reviewed.

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
const PACK_HALF: f64 = 3.0; // half-edge of the reflective cube, cm (216 cm^3)
const PACK_R: f64 = 0.04; // kernel radius, cm
const PACK_PF: f64 = 0.0005; // kernel packing fraction -> C/U-235 ~ 3.8e3
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
    let tape = generate_tape(&deck, temperature, ElasticChannel::Generate)
        .expect("graphite S(alpha,beta) + coherent-elastic regenerates from the embedded deck");

    // The filename must be unique **per call**, not just per process: cargo's
    // harness runs the tests in this file as threads of ONE process, so a
    // pid-only name has every concurrent caller writing, reading and deleting
    // the same path. That races — the reader observes a partially written tape
    // and `from_endf_file` fails with `EndfParse("unexpected end of section
    // data")`. A process-wide atomic counter plus the pid keeps callers apart.
    static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = std::env::temp_dir().join(format!(
        "op_htr10_graphite_sab_{temperature_k:.1}K_{}_{seq}.endf",
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

/// HEU kernel + carbon matrix nuclide array: `[U234, U235, U238, C0]`.
/// `thermal` is `None` for the free-gas baseline, `Some` for the bound run.
///
/// The matrix nuclide is **`"C0"` — natural carbon** (ENDF/B-VII.1 ZA 006000),
/// which is how the embedded CORE windowed-multipole library names elemental
/// carbon; there is no separate `"C12"` entry (see
/// `crates/njoy-outram-park-fork/docs/wmp-nuclide-manifest.md`, "C-nat |
/// 006000"). `tests/thermal_graphite_elastic.rs` uses the same name. Attaching
/// the crystalline-graphite S(alpha, beta) law to natural carbon is the right
/// pairing: the ENDF thermal-scattering evaluation is itself for carbon *in
/// graphite* as an element, not per-isotope.
fn nuclides(thermal: Option<ThermalScattering>) -> Vec<Nuclide> {
    let matrix = match thermal {
        Some(t) => Nuclide::from_core("C0").unwrap().with_thermal_scattering(t),
        None => Nuclide::from_core("C0").unwrap(),
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
        Some(if packed.is_inside_kernel(p) {
            0usize
        } else {
            1usize
        })
    };
    let settings = KeffSettings {
        n_particles: 300,
        n_inactive: 8,
        n_active: 15,
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

/// LIVE: the delta-tracking (Woodcock) majorant still bounds the true
/// macroscopic total cross section once the bound-atom channel is attached.
///
/// `op-hc2o`'s coordinator note asks for this explicitly: *"the delta-tracking
/// majorant must be RE-CHECKED rather than assumed now that the bound channel
/// raises Sigma_t below 4 eV, because a wrong Woodcock majorant is a silent
/// bias and not a crash."* Delta tracking rejects a collision with probability
/// `1 - Sigma_t/Sigma_maj`; if `Sigma_maj < Sigma_t` anywhere, that probability
/// goes negative and the estimator is biased **without any error being
/// raised**. So this is checked, not reasoned about.
///
/// Why it is not obviously safe: [`Majorant::bounding`] samples `Sigma_t` on a
/// log energy grid (4096 bins x 32 subsamples here) and takes the per-bin max,
/// but graphite's coherent-elastic cross section is a **1/E sawtooth with 221
/// Bragg discontinuities** — it jumps upward at every edge, so a grid that
/// steps over an edge can under-sample the peak just above it. The margin
/// (10% here) is what absorbs that, and this test measures whether it does.
///
/// Method: build the bound-graphite materials/nuclides exactly as the k-eff
/// test does, construct the same majorant, then evaluate the true
/// `Material::macro_xs_total` on a **much finer** independent log grid
/// (200k points from 1e-4 eV to 4 eV, i.e. spanning the whole thermal range
/// where the bound channel is active and every Bragg edge lives) and assert
/// `Sigma_maj(E) >= Sigma_t(E)` at every point, for every material.
#[test]
fn majorant_still_bounds_total_xs_with_bound_graphite() {
    let thermal = regenerated_graphite_thermal_scattering(TEMPERATURE_K);
    let cutoff = thermal.cutoff_ev();
    let nucs = nuclides(Some(thermal));
    let mats = materials();
    // Identical majorant parameters to `run_keff`, so this measures the
    // majorant the transport actually uses.
    let maj = Majorant::bounding(&mats, &nucs, 1.0e-4, 2.0e7, 4096, 32, 0.1);

    let (e_lo, e_hi) = (1.0e-4_f64, 4.0_f64.max(cutoff * 1.05));
    let n = 200_000;
    let (ln_lo, ln_hi) = (e_lo.ln(), e_hi.ln());

    let mut worst_ratio = 0.0_f64; // max of Sigma_t / Sigma_maj; must stay <= 1
    let mut worst_e = 0.0_f64;
    let mut worst_mat = usize::MAX;

    for i in 0..=n {
        let e = (ln_lo + (ln_hi - ln_lo) * i as f64 / n as f64).exp();
        let sigma_maj = maj.at(e);
        for (m, mat) in mats.iter().enumerate() {
            let sigma_t = mat.macro_xs_total(e, &nucs);
            let ratio = if sigma_maj > 0.0 {
                sigma_t / sigma_maj
            } else if sigma_t > 0.0 {
                f64::INFINITY
            } else {
                0.0
            };
            if ratio > worst_ratio {
                worst_ratio = ratio;
                worst_e = e;
                worst_mat = m;
            }
        }
    }

    eprintln!(
        "[op-hc2o majorant check] worst Sigma_t/Sigma_maj = {worst_ratio:.6} at E = {worst_e:.6e} eV \
         (material {worst_mat}), thermal cutoff = {cutoff:.4} eV, {n} sample points over \
         [{e_lo:.1e}, {e_hi:.3}] eV"
    );

    assert!(
        worst_ratio <= 1.0,
        "delta-tracking majorant UNDER-BOUNDS the true total cross section with bound graphite \
         attached: Sigma_t/Sigma_maj = {worst_ratio:.6} > 1 at E = {worst_e:.6e} eV (material \
         {worst_mat}). Woodcock tracking would be silently biased. Raise the margin or the bin \
         count in Majorant::bounding."
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
    assert_eq!(
        t.elastic_xs(cutoff),
        0.0,
        "elastic must vanish at the cutoff"
    );
    assert_eq!(t.total_xs(cutoff), 0.0, "total must vanish at the cutoff");
}

/// DIAGNOSTIC (not a pass/fail gate): does a fission-born neutron in this
/// material mix ever reach the thermal range at all?
///
/// Motivation: the k-eigenvalue comparison above can only detect the
/// S(alpha, beta) law if neutrons actually reach energies below the table's
/// ~4 eV cutoff. If they never do, the free-gas and bound runs are identical
/// **by construction**, and a "no difference" result says nothing about
/// whether the wiring works. Distinguishing "wiring is dead" from "the problem
/// is a fast spectrum" needs the spectrum measured, not assumed.
///
/// Method: a homogenized infinite-medium random walk, which is deliberately
/// simpler than the real doubly-heterogeneous geometry — no spatial transport,
/// no kernel/matrix boundary. Volume-homogenize the fuel and matrix
/// compositions at the test's packing fraction, sample a Watt fission birth
/// energy, then repeatedly pick a nuclide proportional to its macroscopic
/// total, partition the collision (fission | capture | inelastic | (n,2n) |
/// scatter) exactly as `keff_delta`'s history loop does, and follow the
/// scattered neutron until it is absorbed, causes fission, or reaches the
/// thermal cutoff. Records what fraction of histories reach the cutoff and the
/// distribution of the minimum energy attained.
///
/// This over-estimates thermalization relative to the real geometry if
/// anything (homogenization removes the self-shielding of the fuel kernels
/// that would otherwise absorb resonance-energy neutrons), so a *negative*
/// result here — few or no histories reaching thermal — is strong evidence the
/// real geometry is also fast.
///
/// Run it with:
/// ```text
/// cargo test --release -p outram-mc-libs --test \
///     htr10_graphite_thermal_scattering_pebble_bed -- --ignored --nocapture \
///     diagnose_thermal_spectrum_reach
/// ```
#[test]
#[ignore = "diagnostic sweep, asserts nothing about pass/fail -- run explicitly with --ignored"]
fn diagnose_thermal_spectrum_reach() {
    use outram_mc_libs::material::nuclide::Inelastic;
    use outram_mc_libs::physics::scatter::{
        continuum_inelastic_scatter, elastic_scatter, rotate_direction, two_body_scatter,
        two_body_scatter_with_mu,
    };
    use outram_mc_libs::rng::distributions::{isotropic_direction, watt};
    use outram_mc_libs::rng::lcg::prn;

    let thermal = regenerated_graphite_thermal_scattering(TEMPERATURE_K);
    let cutoff = thermal.cutoff_ev();
    let nucs = nuclides(Some(thermal));
    let mats = materials();

    // Volume-homogenize: fuel occupies PACK_PF of the box, matrix the rest.
    let mut comps = Vec::new();
    for c in &mats[0].components {
        comps.push(NuclideComponent {
            nuclide_idx: c.nuclide_idx,
            atom_density: c.atom_density * PACK_PF,
        });
    }
    for c in &mats[1].components {
        comps.push(NuclideComponent {
            nuclide_idx: c.nuclide_idx,
            atom_density: c.atom_density * (1.0 - PACK_PF),
        });
    }
    let homog = Material {
        id: 99,
        name: "homogenized fuel+graphite".into(),
        temperature: TEMPERATURE_K,
        components: comps,
    };

    let n_hist = 20_000usize;
    let mut seed = 987_654_321u64;
    let mut reached_thermal = 0usize;
    let mut min_energies: Vec<f64> = Vec::with_capacity(n_hist);
    let mut total_scatters = 0u64;

    for _ in 0..n_hist {
        // Same parameters as `KeffSettings::default()`: a in eV, b in eV^-1
        // (the familiar 0.988 MeV / 2.249 MeV^-1 U-235 pair, in this crate's eV
        // convention). Passing the MeV numbers directly would birth neutrons at
        // ~1 eV, below the thermal cutoff, and silently invalidate the sweep.
        let mut e = watt(&mut seed, 0.988e6, 2.249e-6);
        let (dx, dy, dz) = isotropic_direction(&mut seed);
        let mut u = outram_mc_libs::geometry::position::Direction::new(dx, dy, dz);
        let mut e_min = e;
        // Cap so a purely-scattering excursion cannot spin forever.
        for _ in 0..100_000 {
            if e <= cutoff {
                break;
            }
            let ci = homog.sample_nuclide(e, &mut seed, &nucs);
            let nuc = &nucs[homog.components[ci].nuclide_idx];
            let x = nuc.xs_at_energy(e, TEMPERATURE_K);
            if !(x.total > 0.0) {
                break;
            }
            let xi = prn(&mut seed) * x.total;
            if xi < x.absorption {
                break; // fission or capture — history over
            } else if xi < x.absorption + x.inelastic {
                let (e2, u2) = match nuc.sample_inelastic(e, &mut seed) {
                    Inelastic::Level { q } => two_body_scatter(e, u, nuc.awr, q, &mut seed),
                    Inelastic::Continuum => continuum_inelastic_scatter(e, u, nuc.awr, &mut seed),
                };
                e = e2;
                u = u2;
            } else if xi < x.absorption + x.inelastic + x.n2n {
                let (e2, u2) = continuum_inelastic_scatter(e, u, nuc.awr, &mut seed);
                e = e2;
                u = u2;
            } else {
                let (e2, u2) = if let Some((e_out, mu_lab)) = nuc.sample_thermal(e, &mut seed) {
                    (e_out, rotate_direction(u, mu_lab, &mut seed))
                } else {
                    match nuc.sample_elastic_mu_cm(e, &mut seed) {
                        Some(mu_cm) => {
                            two_body_scatter_with_mu(e, u, nuc.awr, 0.0, mu_cm, &mut seed)
                        }
                        None => elastic_scatter(e, u, nuc.awr, &mut seed),
                    }
                };
                e = e2;
                u = u2;
            }
            total_scatters += 1;
            e_min = e_min.min(e);
        }
        if e_min <= cutoff {
            reached_thermal += 1;
        }
        min_energies.push(e_min);
    }

    min_energies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pct = |q: f64| min_energies[((min_energies.len() - 1) as f64 * q) as usize];

    eprintln!(
        "\n[op-hc2o spectrum diagnostic] homogenized medium, pf = {PACK_PF}, {n_hist} histories\n\
         thermal cutoff              : {cutoff:.4} eV\n\
         histories reaching cutoff   : {reached_thermal} / {n_hist} ({:.3} %)\n\
         mean scatters per history   : {:.1}\n\
         min-energy percentiles [eV] : p01 {:.4e}  p10 {:.4e}  p50 {:.4e}  p90 {:.4e}\n",
        100.0 * reached_thermal as f64 / n_hist as f64,
        total_scatters as f64 / n_hist as f64,
        pct(0.01),
        pct(0.10),
        pct(0.50),
        pct(0.90),
    );
}
