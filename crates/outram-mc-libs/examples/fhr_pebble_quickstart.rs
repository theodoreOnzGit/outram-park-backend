//! **FHR pebble quickstart** — the shortest complete path from nothing to a
//! k-eigenvalue on an FHR (fluoride-salt-cooled high-temperature reactor)
//! TRISO pebble, using only the embedded CORE nuclear data (no downloads, no
//! ENDF tapes). Run it:
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example fhr_pebble_quickstart
//! ```
//!
//! # Why this example exists
//!
//! Building an FHR pebble means composing five sibling `pebble_beds`
//! submodules end to end — `crp_packing` → `sphere_packing` → `fhr_pebble` →
//! `delta_tracking` → `keff_delta` — and two independent documentation-only
//! Haiku dogfood runs (`docs/dogfood-2026-09-11-ring-rpt.md`,
//! `docs/dogfood-2026-09-11-ring-rpt-run2.md`) converged on the same gap:
//! nothing showed that pipeline in one place. This example is that missing
//! walkthrough. Read it top to bottom; each step names the function it calls
//! and why.
//!
//! # This is NOT a physics result
//!
//! Every number below exists to prove the pipeline runs, not to say anything
//! about a real FHR pebble:
//!
//! - **CORE-tier data** ([`Nuclide::from_core`]) — analytic windowed-multipole
//!   + coarse fast-group cross sections, not resonance-reconstructed ENDF.
//! - **Tiny particle counts** — a few dozen TRISO particles and a few hundred
//!   neutron histories, chosen so this finishes in seconds, not a converged
//!   Monte Carlo tally.
//! - **Invented densities and radii** — plausible orders of magnitude, not a
//!   real fuel specification.
//!
//! In particular, **do not read the gap between the two k values as a verdict
//! on ring-RPT.** They differ by thousands of pcm here only because the RPT
//! inner radius is an arbitrary number in this example, not one fitted to this
//! geometry — fitting it is the entire content of the method. Measured properly,
//! on the reference's own geometry and with a fitted radius, ring-RPT reproduces
//! the explicit pebble to **+163 ± 281 pcm (0.58σ)** and removes 94 % of the
//! double-heterogeneity error. See
//! `verification_and_validation/ring_rpt/ring_rpt_vs_openmc.md`.
//!
//! Do not quote the two k-effective values this program prints as meaning
//! anything about FHR physics. The real, ENDF-driven, code-to-code-verified
//! case is **`examples/fhr_ring_rpt_endf.rs`** — read that one for an actual
//! V&V result (`op-mzvp`, GitHub #156).

use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::BoundaryType;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::crp_packing::pack_spheres_crp;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::pebble_beds::fhr_pebble::{
    fhr_pebble_geometry, homogenise_by_volume, rpt_fuel_outer_radius, ExplicitTrisoPebble,
    TrisoMaterials, TrisoSpec,
};
use outram_mc_libs::pebble_beds::keff_delta::{run_keff_delta_in, DeltaDomain};
use outram_mc_libs::pebble_beds::sphere_packing::PackedSpheres;
use outram_mc_libs::physics::keff::KeffSettings;

/// Material-array indices, named once so every call site reads by name
/// instead of by position.
mod mi {
    pub const FUEL: usize = 0;
    pub const BUFFER: usize = 1;
    pub const IPYC: usize = 2;
    pub const SIC: usize = 3;
    pub const OPYC: usize = 4;
    pub const GRAPHITE: usize = 5;
    pub const FLIBE: usize = 6;
    pub const HOMOG: usize = 7;
}

/// Illustrative temperature \[K\] for every material and the transport lookup —
/// FHR pebbles run hot; the exact value is not load-bearing here.
const TEMP_K: f64 = 900.0;

fn main() {
    // ── 1. Nuclides and materials ──────────────────────────────────────────
    // Every microscopic cross section comes from the embedded CORE library —
    // `Nuclide::from_core` needs no file on disk. Indices below (0..7) are the
    // shared "which nuclide" key every `Material`'s `components` refer to.
    let nuclides = vec![
        Nuclide::from_core("U235").expect("U235 in CORE"),
        Nuclide::from_core("U238").expect("U238 in CORE"),
        Nuclide::from_core("O16").expect("O16 in CORE"),
        Nuclide::from_core("C0").expect("natural carbon in CORE"),
        Nuclide::from_core("Si28").expect("Si28 in CORE"),
        Nuclide::from_core("F19").expect("F19 in CORE"),
        Nuclide::from_core("Li7").expect("Li7 in CORE"),
        Nuclide::from_core("Be9").expect("Be9 in CORE"),
    ];

    // Atom densities [atoms/barn·cm] are invented but plausible: a UO2-like
    // fuel kernel, graphite coatings at increasing density (buffer < OPyC <
    // IPyC, the real TRISO ordering), a SiC layer, and a FLiBe coolant.
    // Two tiny helpers so the eight materials below read as one line each.
    let nc = |nuclide_idx: usize, atom_density: f64| NuclideComponent { nuclide_idx, atom_density };
    let mat = |id: i32, name: &str, comps: Vec<NuclideComponent>| Material {
        id, name: name.into(), temperature: TEMP_K, components: comps,
    };

    let fuel = mat(1, "fuel kernel", vec![nc(0, 0.00467), nc(1, 0.01880), nc(2, 0.04695)]); // U235, U238, O16
    let buffer = mat(2, "buffer", vec![nc(3, 0.0501)]); // C0
    let ipyc = mat(3, "IPyC", vec![nc(3, 0.0953)]); // C0
    let sic = mat(4, "SiC", vec![nc(4, 0.0481), nc(3, 0.0481)]); // Si28, C0
    let opyc = mat(5, "OPyC", vec![nc(3, 0.0938)]); // C0
    let graphite = mat(6, "graphite matrix/shell", vec![nc(3, 0.0852)]); // C0
    let flibe = mat(7, "FLiBe coolant", vec![nc(6, 0.0236), nc(5, 0.0472), nc(7, 0.0118)]); // Li7, F19, Be9

    // ── 2. A `TrisoSpec` ────────────────────────────────────────────────────
    // The five cumulative layer radii [cm] plus the whole-particle packing
    // fraction. Scaled up from a real TRISO particle so a tractable few dozen
    // particles fill the fuel zone below, instead of the ~10^5 a full-size
    // 425 µm particle would need.
    let spec = TrisoSpec {
        kernel: 0.08,
        buffer: 0.10,
        ipyc: 0.11,
        sic: 0.12,
        opyc: 0.13,
        packing_fraction: 0.30,
    };

    // ── 3. Pack particles ───────────────────────────────────────────────────
    // `pack_spheres_crp` (Close Random Packing) fills a cube of half-width
    // `pack_half` at the target packing fraction; `PackedSpheres::from_spheres`
    // wraps the result in the spatial-hash grid that gives O(1)
    // "which particle contains this point" queries.
    const R_FUEL_ZONE: f64 = 0.5; // packed-TRISO fuel sphere
    const R_PEBBLE: f64 = 0.6; // + graphite shell
    const R_ROOT: f64 = 1.0; // reflective delta-tracking boundary
    const R_RPT_INNER: f64 = 0.3; // ring-RPT inner graphite ball
    let pack_half = R_FUEL_ZONE + spec.opyc;
    let spheres = pack_spheres_crp(spec.opyc, pack_half, spec.packing_fraction, 20_260_911)
        .expect("CRP packing at pf 0.30");
    let packed = PackedSpheres::from_spheres(spheres, pack_half, spec.opyc);
    println!("packed {} TRISO particles into the fuel zone", packed.len());

    // ── 4. Build BOTH pebbles ───────────────────────────────────────────────
    // 4a. Explicit-TRISO: `ExplicitTrisoPebble` packages the packed-particle /
    // layer-resolution / matrix-fallback lookup that delta tracking needs.
    let explicit_pebble = ExplicitTrisoPebble::new(
        packed,
        spec,
        TrisoMaterials {
            kernel: mi::FUEL,
            buffer: mi::BUFFER,
            ipyc: mi::IPYC,
            sic: mi::SIC,
            opyc: mi::OPYC,
            matrix: mi::GRAPHITE,
        },
        mi::GRAPHITE, // shell
        mi::FLIBE,    // coolant
        R_FUEL_ZONE,
        R_PEBBLE,
    );

    // 4b. Ring-RPT: dissolve the same five TRISO layers into one homogenised
    // material by volume, then size a shell whose volume equals the total
    // particle volume — the "reactivity-equivalent physical transformation".
    let vols = spec.layer_volumes();
    let homog = homogenise_by_volume(
        &[(&fuel, vols[0]), (&buffer, vols[1]), (&ipyc, vols[2]), (&sic, vols[3]), (&opyc, vols[4])],
        8,
        "homogenised TRISO fuel",
        TEMP_K,
    );
    let r_rpt_fuel = rpt_fuel_outer_radius(R_RPT_INNER, R_FUEL_ZONE, spec.packing_fraction);
    // The concentric-shell CSG geometry the ring-RPT pebble is equivalent to
    // (used by the surface-tracked reactor-physics driver, not this
    // delta-tracking quickstart — see `fhr_ring_rpt_endf.rs` for that path).
    let rpt_geometry = fhr_pebble_geometry(
        R_RPT_INNER, r_rpt_fuel, R_PEBBLE, R_ROOT,
        mi::HOMOG, mi::GRAPHITE, mi::FLIBE, BoundaryType::Reflective, TEMP_K,
    );
    println!(
        "ring-RPT CSG geometry: {} surfaces, {} cells (fuel shell {R_RPT_INNER:.2}-{r_rpt_fuel:.4} cm)",
        rpt_geometry.surfaces.len(),
        rpt_geometry.cells.len()
    );

    let materials = vec![fuel, buffer, ipyc, sic, opyc, graphite, flibe, homog];

    // Point-membership closures for delta tracking, one per pebble, so both
    // are solved by the exact same transport path and their k values are
    // directly comparable.
    let explicit_at = |p: Position| explicit_pebble.material_at(p);
    let rpt_at = move |p: Position| -> Option<usize> {
        let r = p.norm();
        Some(if r < R_RPT_INNER {
            mi::GRAPHITE
        } else if r < r_rpt_fuel {
            mi::HOMOG
        } else if r < R_PEBBLE {
            mi::GRAPHITE
        } else {
            mi::FLIBE
        })
    };

    // ── 5. A `Majorant` ─────────────────────────────────────────────────────
    // Delta (Woodcock) tracking needs a bound Σ_maj(E) ≥ Σ_t(E) for every
    // material, sampled from a log-spaced energy grid spanning thermal to
    // fast.
    let (e_min, e_max, n_grid): (f64, f64, usize) = (1.0e-4, 2.0e7, 150);
    let ratio = (e_max / e_min).powf(1.0 / (n_grid as f64 - 1.0));
    let grid: Vec<f64> = (0..n_grid as i32).map(|i| e_min * ratio.powi(i)).collect();
    let majorant = Majorant::from_materials(&materials, &nuclides, &grid, 0.05);

    // ── 6. k-eigenvalue by delta tracking ───────────────────────────────────
    let domain = DeltaDomain::Sphere { radius: R_ROOT };
    let settings = KeffSettings {
        n_particles: 500,
        n_inactive: 15,
        n_active: 35,
        temperature_k: TEMP_K,
        ..KeffSettings::default()
    };

    let k_explicit = run_keff_delta_in(domain, &materials, &nuclides, &majorant, explicit_at, &settings);
    let k_rpt = run_keff_delta_in(domain, &materials, &nuclides, &majorant, rpt_at, &settings);

    println!("k_eff (explicit-TRISO pebble) = {:.5} +/- {:.5}", k_explicit.k_mean, k_explicit.k_std);
    println!("k_eff (ring-RPT pebble)       = {:.5} +/- {:.5}", k_rpt.k_mean, k_rpt.k_std);
    println!(
        "\n(teaching example only -- CORE-tier data, tiny particle counts, invented \
         densities; see examples/fhr_ring_rpt_endf.rs for the real V&V case)"
    );
}
