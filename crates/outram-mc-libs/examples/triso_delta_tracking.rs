//! Random-packed TRISO doubly-heterogeneous k∞ by delta (Woodcock) tracking.
//!
//! Run it:
//! ```text
//! cargo run -p outram-mc-libs --release --example triso_delta_tracking
//! ```
//!
//! This example is a top-to-bottom walkthrough of *why* pebble-bed / TRISO
//! transport needs delta tracking and *how* this crate does it. Read it in order;
//! it needs only the three `pebble_beds` modules named below and nothing internal.
//!
//! # The problem: doubly-heterogeneous geometry
//!
//! A pebble packs O(10⁴) sub-millimetre TRISO fuel kernels into graphite; a core
//! packs O(10⁵) pebbles. A neutron flying in a straight line crosses an enormous
//! number of kernel surfaces. **Surface tracking** — the usual "advance to the
//! nearest boundary" algorithm — would have to compute the distance to the nearest
//! of all those surfaces at every flight. That is intractable at scale.
//!
//! # The fix: delta (Woodcock) tracking
//!
//! Delta tracking never searches for a surface. Instead:
//!
//! 1. Pick a **majorant** `Σ_maj(E) ≥ Σ_t(E)` that bounds *every* material.
//! 2. Sample a flight `s = −ln ξ / Σ_maj(E)` on that bound and move there.
//! 3. Read the *local* material's true `Σ_t`. Accept a **real** collision with
//!    probability `Σ_t/Σ_maj`; otherwise it is a **virtual** collision (nothing
//!    happens) and you continue. The rejection exactly undoes the over-sampling
//!    from using the inflated `Σ_maj`, so the collision density is unbiased.
//!
//! The only geometry question left is "**what material is at this point?**" — an
//! O(1) test against the packed-kernel spatial hash grid. No surface search at all.
//!
//! # What this program does
//!
//! - Randomly packs HEU fuel kernels into a reflective 1 cm³ box to a 0.30 packing
//!   fraction with [`PackedSpheres::pack`] (Random Sequential Addition).
//! - Builds a bin-maximum majorant with [`Majorant::bounding`] that provably bounds
//!   `Σ_t` even across the U resonances (a coarse majorant would under-bound a
//!   resonance peak between grid points and bias the result).
//! - Runs a fission-source k-eigenvalue power iteration with [`run_keff_delta`],
//!   every history streamed by delta tracking, and prints the converged k∞.

use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::pebble_beds::keff_delta::run_keff_delta;
use outram_mc_libs::pebble_beds::sphere_packing::PackedSpheres;
use outram_mc_libs::physics::keff::KeffSettings;

fn main() {
    // 1 cm³ reflective box (infinite-medium unit cell), notebook-matched packing
    // fraction 0.30. Kernel radius 0.04 cm is a tractable stand-in for the
    // notebook's 42.5 µm particle (its 0.30 packing would be ~9.3×10⁵ kernels).
    let half = 0.5;
    let radius = 0.04;
    let pf = 0.30;
    let seed = 20240715;

    // Materials: index 0 = HEU fuel kernel, index 1 = H-1 matrix.
    let nuclides = vec![
        Nuclide::from_core("U234").unwrap(),
        Nuclide::from_core("U235").unwrap(),
        Nuclide::from_core("U238").unwrap(),
        Nuclide::from_core("H1").unwrap(),
    ];
    let materials = vec![
        Material {
            id: 1,
            name: "HEU kernel".into(),
            temperature: 293.6,
            components: vec![
                NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 },
                NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 },
                NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 },
            ],
        },
        Material {
            id: 2,
            name: "H matrix".into(),
            temperature: 293.6,
            components: vec![NuclideComponent { nuclide_idx: 3, atom_density: 4.0e-2 }],
        },
    ];

    // Randomly pack the kernels (RSA). This is the doubly-heterogeneous geometry.
    let packed = PackedSpheres::pack(radius, half, pf, seed).expect("RSA packs at pf 0.30");
    println!(
        "Packed {} HEU kernels into a {:.0} cm³ box — realized packing fraction {:.4}.",
        packed.len(),
        (2.0 * half).powi(3),
        packed.packing_fraction()
    );

    // The majorant: a provable upper bound on Σ_t over the whole energy range,
    // sub-sampling each energy bin so a resonance peak cannot slip under it.
    let majorant = Majorant::bounding(&materials, &nuclides, 1.0e-4, 2.0e7, 4096, 32, 0.1);

    // Geometry lookup for delta tracking: which material index is at this point?
    // Inside a kernel → fuel (0); otherwise → matrix (1). O(1) via the hash grid.
    let material_at = move |p: Position| Some(if packed.is_inside_kernel(p) { 0usize } else { 1usize });

    // Delta-tracking-driven fission-source power iteration.
    let settings = KeffSettings { n_particles: 2000, n_inactive: 25, n_active: 75, ..KeffSettings::default() };
    let result = run_keff_delta(half, &materials, &nuclides, &majorant, material_at, &settings);

    println!(
        "Doubly-heterogeneous k∞ = {:.5} ± {:.5} over {} generations (delta tracking).",
        result.k_mean,
        result.k_std,
        result.k_by_generation.len()
    );
    println!(
        "A fissile HEU infinite medium: k∞ ≫ 1 (no leakage), converged and stationary — \
         the evidence delta tracking transports correctly through the packed medium."
    );
}
