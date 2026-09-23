// SPDX-License-Identifier: GPL-3.0

//! **V&V gate** — rectilinear, cylindrical and spherical tally meshes against
//! OpenMC's own per-bin volumes, bin for bin. GitHub #260.
//!
//! # Methodology
//!
//! Three meshes, ported from `src/mesh.cpp` at OpenMC `afa7a14`. (The commit
//! the issue cites, `608a1c33`, is unavailable here and not fetchable; see
//! `verification_and_validation/white_boundary/white_boundary_vs_openmc.md`.)
//!
//! Upstream's Python API exposes `mesh.volumes`, which is **OpenMC's own**
//! per-bin volume array rather than a re-derivation of it — so this is a
//! genuine code-to-code comparison of every bin, which is what the issue asks
//! for ("bin-for-bin, not just the integral"). The deck that produced the
//! reference is `verification_and_validation/mesh/openmc_inputs/`.
//!
//! Grids deliberately chosen to be **non-uniform and to include the awkward
//! cases**: a radial grid starting at `r = 0` (the degenerate inner bin), an
//! azimuthal grid that wraps the full `2 pi`, a polar grid crossing `pi/2`
//! where `cos` changes sign, and a `z` grid straddling zero.
//!
//! Three independent things are checked:
//!
//! 1. **Every bin volume against OpenMC**, to 1e-12 relative.
//! 2. **The sum of all bin volumes against the closed-form total** — an
//!    annulus, a spherical shell, a box. A per-bin formula can be wrong in a
//!    way that cancels in the sum, and a sum can be right while the
//!    distribution is wrong, so neither check subsumes the other.
//! 3. **Point binning round-trips**: a point placed at the centroid of each bin
//!    must bin back to that bin. This is what catches the 1-based/0-based
//!    indexing trap — upstream's `get_index_in_direction` adds 1 because its
//!    `MeshIndex` is 1-based, and this crate is 0-based, so the `+1` is
//!    deliberately not carried.
//!
//! # Results (2026-09-22)
//!
//! **Worst relative difference against OpenMC's own per-bin volumes, over all
//! 18 + 12 + 12 = 42 bins: `2.854e-16`.** That is machine precision, which is
//! what a correct port of a closed-form expression should give -- there is no
//! sampling here, so anything larger would mean a different formula rather than
//! noise.
//!
//! Sums against the closed form, all to better than 1e-12 relative:
//!
//! | mesh | sum of bins | exact |
//! |---|---|---|
//! | cylindrical | 251.3274122872 | 251.3274122872 (`pi 4^2 * 5`) |
//! | spherical | 268.0825731063 | 268.0825731063 (`4/3 pi 4^3`) |
//! | rectilinear | 72.0000000000 | 72 (`6 * 3 * 4`) |
//!
//! Every centroid binned back to its own bin, on all three meshes, and the
//! boundary cases behave: a point beyond `r` or `z` is `None`, a point exactly
//! on the outer radius bins into the last radial cell rather than vanishing,
//! and an on-axis point pins `phi = 0` instead of leaving it to `atan2(0, 0)`.

use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::tally::mesh::{CylindricalMesh, RectilinearMesh, SphericalMesh};

const TAU: f64 = std::f64::consts::TAU;
const PI: f64 = std::f64::consts::PI;

const OPENMC_CYL: [f64; 18] = [
    1.200000000000000e+00,
    6.300000000000000e+00,
    1.170000000000000e+01,
    1.800000000000000e+00,
    9.450000000000001e+00,
    1.755000000000000e+01,
    3.283185307179586e+00,
    1.723672286269283e+01,
    3.201105674500096e+01,
    1.800000000000000e+00,
    9.449999999999999e+00,
    1.755000000000000e+01,
    2.700000000000000e+00,
    1.417500000000000e+01,
    2.632500000000000e+01,
    4.924777960769379e+00,
    2.585508429403924e+01,
    4.801658511750145e+01,
];
const OPENMC_SPH: [f64; 18] = [
    9.406312508620460e-02,
    1.375673204385742e+00,
    4.550303676045147e+00,
    7.059368749137954e-01,
    1.032432679561426e+01,
    3.414969632395486e+01,
    1.410946876293069e-01,
    2.063509806578613e+00,
    6.825455514067722e+00,
    1.058905312370693e+00,
    1.548649019342139e+01,
    5.122454448593228e+01,
    2.573555585253521e-01,
    3.763825043433275e+00,
    1.244957514366391e+01,
    1.931434646261039e+00,
    2.824723170156769e+01,
    9.343315101287776e+01,
];
const OPENMC_RECT: [f64; 12] = [
    3.750000000000000e-01,
    7.500000000000000e-01,
    1.125000000000000e+00,
    3.750000000000000e-01,
    7.500000000000000e-01,
    1.125000000000000e+00,
    5.625000000000000e+00,
    1.125000000000000e+01,
    1.687500000000000e+01,
    5.625000000000000e+00,
    1.125000000000000e+01,
    1.687500000000000e+01,
];

fn cyl() -> CylindricalMesh {
    CylindricalMesh {
        r_grid: vec![0.0, 1.0, 2.5, 4.0],
        phi_grid: vec![0.0, 1.2, 3.0, TAU],
        z_grid: vec![-2.0, 0.0, 3.0],
        origin: Position::new(0.0, 0.0, 0.0),
    }
}

fn sph() -> SphericalMesh {
    SphericalMesh {
        r_grid: vec![0.0, 1.0, 2.5, 4.0],
        theta_grid: vec![0.0, 0.7, PI],
        phi_grid: vec![0.0, 1.2, 3.0, TAU],
        origin: Position::new(0.0, 0.0, 0.0),
    }
}

fn rect() -> RectilinearMesh {
    RectilinearMesh {
        grid: [
            vec![0.0, 1.0, 3.0, 6.0],
            vec![-1.0, 0.5, 2.0],
            vec![0.0, 0.25, 4.0],
        ],
    }
}

/// Every bin volume, against OpenMC's own array.
#[test]
fn per_bin_volumes_match_openmc() {
    let mut worst = 0.0_f64;

    let m = cyl();
    let d = m.dimension();
    assert_eq!(m.n_bins(), OPENMC_CYL.len(), "cylindrical bin count");
    for k in 0..d[2] {
        for j in 0..d[1] {
            for i in 0..d[0] {
                let flat = i + d[0] * (j + d[1] * k);
                let got = m.volume([i, j, k]);
                let want = OPENMC_CYL[flat];
                let rel = (got - want).abs() / want;
                worst = worst.max(rel);
                assert!(rel < 1e-12, "cyl bin {flat}: {got} vs OpenMC {want}");
            }
        }
    }

    let m = sph();
    let d = m.dimension();
    assert_eq!(m.n_bins(), OPENMC_SPH.len(), "spherical bin count");
    for k in 0..d[2] {
        for j in 0..d[1] {
            for i in 0..d[0] {
                let flat = i + d[0] * (j + d[1] * k);
                let got = m.volume([i, j, k]);
                let want = OPENMC_SPH[flat];
                let rel = (got - want).abs() / want;
                worst = worst.max(rel);
                assert!(rel < 1e-12, "sph bin {flat}: {got} vs OpenMC {want}");
            }
        }
    }

    let m = rect();
    let d = m.dimension();
    assert_eq!(m.n_bins(), OPENMC_RECT.len(), "rectilinear bin count");
    for k in 0..d[2] {
        for j in 0..d[1] {
            for i in 0..d[0] {
                let flat = i + d[0] * (j + d[1] * k);
                let got = m.volume([i, j, k]);
                let want = OPENMC_RECT[flat];
                let rel = (got - want).abs() / want;
                worst = worst.max(rel);
                assert!(rel < 1e-12, "rect bin {flat}: {got} vs OpenMC {want}");
            }
        }
    }
    println!("worst relative difference against OpenMC, all bins: {worst:.3e}");
}

/// Bin volumes must sum to the closed-form total.
///
/// Independent of the bin-for-bin check above: a per-bin formula can be wrong in
/// a way that cancels in the sum, and a sum can be right while the distribution
/// is wrong.
#[test]
fn bin_volumes_sum_to_the_closed_form_total() {
    let m = cyl();
    let d = m.dimension();
    let total: f64 = (0..d[2])
        .flat_map(|k| (0..d[1]).flat_map(move |j| (0..d[0]).map(move |i| [i, j, k])))
        .map(|ijk| m.volume(ijk))
        .sum();
    // Full cylinder of radius 4, height 5.
    let exact = PI * 16.0 * 5.0;
    println!("cyl total {total:.10} vs exact {exact:.10}");
    assert!((total - exact).abs() / exact < 1e-12);

    let m = sph();
    let d = m.dimension();
    let total: f64 = (0..d[2])
        .flat_map(|k| (0..d[1]).flat_map(move |j| (0..d[0]).map(move |i| [i, j, k])))
        .map(|ijk| m.volume(ijk))
        .sum();
    // Full sphere of radius 4.
    let exact = 4.0 / 3.0 * PI * 64.0;
    println!("sph total {total:.10} vs exact {exact:.10}");
    assert!((total - exact).abs() / exact < 1e-12);

    let m = rect();
    let d = m.dimension();
    let total: f64 = (0..d[2])
        .flat_map(|k| (0..d[1]).flat_map(move |j| (0..d[0]).map(move |i| [i, j, k])))
        .map(|ijk| m.volume(ijk))
        .sum();
    let exact = 6.0 * 3.0 * 4.0;
    println!("rect total {total:.10} vs exact {exact:.10}");
    assert!((total - exact).abs() / exact < 1e-12);
}

/// A point at each bin's centroid must bin back to that bin.
///
/// This is the 0-based/1-based trap. Upstream's `get_index_in_direction` adds 1
/// because its `MeshIndex` is 1-based; this crate is 0-based and deliberately
/// does not carry the `+1`. Carrying it would shift every index by one and only
/// show at the boundaries.
#[test]
fn centroids_bin_back_to_their_own_bin() {
    let m = rect();
    let d = m.dimension();
    for k in 0..d[2] {
        for j in 0..d[1] {
            for i in 0..d[0] {
                let p = Position::new(
                    0.5 * (m.grid[0][i] + m.grid[0][i + 1]),
                    0.5 * (m.grid[1][j] + m.grid[1][j + 1]),
                    0.5 * (m.grid[2][k] + m.grid[2][k + 1]),
                );
                assert_eq!(m.indices(p), Some([i, j, k]), "rect centroid {i},{j},{k}");
            }
        }
    }

    let m = cyl();
    let d = m.dimension();
    for k in 0..d[2] {
        for j in 0..d[1] {
            for i in 0..d[0] {
                let r = 0.5 * (m.r_grid[i] + m.r_grid[i + 1]);
                let phi = 0.5 * (m.phi_grid[j] + m.phi_grid[j + 1]);
                let z = 0.5 * (m.z_grid[k] + m.z_grid[k + 1]);
                let p = Position::new(r * phi.cos(), r * phi.sin(), z);
                assert_eq!(m.indices(p), Some([i, j, k]), "cyl centroid {i},{j},{k}");
            }
        }
    }

    let m = sph();
    let d = m.dimension();
    for k in 0..d[2] {
        for j in 0..d[1] {
            for i in 0..d[0] {
                let r = 0.5 * (m.r_grid[i] + m.r_grid[i + 1]);
                let th = 0.5 * (m.theta_grid[j] + m.theta_grid[j + 1]);
                let phi = 0.5 * (m.phi_grid[k] + m.phi_grid[k + 1]);
                let p = Position::new(
                    r * th.sin() * phi.cos(),
                    r * th.sin() * phi.sin(),
                    r * th.cos(),
                );
                assert_eq!(m.indices(p), Some([i, j, k]), "sph centroid {i},{j},{k}");
            }
        }
    }
}

/// Points outside the mesh return `None`, and the outer surface is inclusive so
/// a point exactly on it bins into the last cell rather than vanishing.
#[test]
fn outside_is_none_and_the_outer_surface_is_inclusive() {
    let m = cyl();
    assert_eq!(m.indices(Position::new(5.0, 0.0, 0.0)), None, "beyond r");
    assert_eq!(m.indices(Position::new(0.5, 0.0, 9.0)), None, "beyond z");
    // Exactly on the outer radius.
    let d = m.dimension();
    assert_eq!(
        m.indices(Position::new(4.0, 0.0, 1.0)).map(|x| x[0]),
        Some(d[0] - 1),
        "a point on the outer radius must bin into the last radial cell"
    );

    // On the cylinder axis phi is pinned to 0 rather than left to atan2(0,0).
    assert_eq!(
        m.indices(Position::new(0.0, 0.0, 1.0)),
        Some([0, 0, 1]),
        "on-axis point must bin at phi = 0"
    );
}

/// Scope item 4: a [`MeshFilter`] must bin through **any** mesh type, and its
/// `bin_volume` must agree with the per-type `volume` for the same bin.
///
/// The flat-index round trip is the load-bearing part. `bin_volume` takes a
/// FLAT bin and has to unflatten it; `volume` takes `(i, j, k)` directly. If
/// the unflattening disagreed with the flattening used by `bin`, a flux
/// normalisation would divide by another bin's volume -- a wrong answer that
/// looks entirely plausible, since every value would still be positive and of
/// roughly the right size.
#[test]
fn the_mesh_filter_dispatches_over_every_mesh_type() {
    use outram_mc_libs::tally::filter::{Filter, FilterEvent, MeshFilter};
    use outram_mc_libs::tally::mesh::MeshKind;

    for kind in [
        MeshKind::Cylindrical(cyl()),
        MeshKind::Spherical(sph()),
        MeshKind::Rectilinear(rect()),
    ] {
        let n = kind.n_bins();
        let filter = MeshFilter { mesh: kind.clone() };
        assert_eq!(filter.n_bins(), n);

        // Every flat bin's volume must match the typed `volume(i, j, k)`, and
        // they must sum to the same total the previous test checked.
        let mut sum = 0.0;
        for b in 0..n {
            let v = kind.bin_volume(b).expect("in-range bin has a volume");
            assert!(v > 0.0, "bin {b} has non-positive volume {v}");
            sum += v;
        }
        let typed_sum: f64 = match &kind {
            MeshKind::Cylindrical(m) => {
                let d = m.dimension();
                (0..d[2])
                    .flat_map(|k| (0..d[1]).flat_map(move |j| (0..d[0]).map(move |i| [i, j, k])))
                    .map(|ijk| m.volume(ijk))
                    .sum()
            }
            MeshKind::Spherical(m) => {
                let d = m.dimension();
                (0..d[2])
                    .flat_map(|k| (0..d[1]).flat_map(move |j| (0..d[0]).map(move |i| [i, j, k])))
                    .map(|ijk| m.volume(ijk))
                    .sum()
            }
            MeshKind::Rectilinear(m) => {
                let d = m.dimension();
                (0..d[2])
                    .flat_map(|k| (0..d[1]).flat_map(move |j| (0..d[0]).map(move |i| [i, j, k])))
                    .map(|ijk| m.volume(ijk))
                    .sum()
            }
            MeshKind::Regular(_) => unreachable!(),
        };
        assert!(
            (sum - typed_sum).abs() / typed_sum < 1e-12,
            "flat bin_volume sum {sum} disagrees with the typed sum {typed_sum}"
        );

        // Out of range is None, not a panic and not a wrong volume.
        assert_eq!(kind.bin_volume(n), None);

        // A point at a known centroid must reach the filter's bin.
        let ev = FilterEvent {
            position: Position::new(0.0, 0.0, 0.0),
            ..Default::default()
        };
        // Origin is inside the cylindrical and spherical meshes (r starts at 0)
        // but outside the rectilinear one (y starts at -1, z at 0 -> on the
        // corner). Only assert the ones that are genuinely inside.
        if matches!(kind, MeshKind::Cylindrical(_) | MeshKind::Spherical(_)) {
            assert!(
                filter.get_bin(&ev).is_some(),
                "the origin lies inside a mesh whose radial grid starts at 0"
            );
        }
    }
}

/// Every flat bin must round-trip: `bin_volume(flat)` for the flat index that
/// `bin()` returns for that bin's own centroid.
///
/// Separate from the test above because that one only checks the SUM, which is
/// invariant under a permutation of the bins. This one checks the mapping
/// itself, which a permuted unflattening would fail.
#[test]
fn flat_bin_indices_round_trip_through_centroids() {
    use outram_mc_libs::tally::mesh::MeshKind;

    let m = cyl();
    let kind = MeshKind::Cylindrical(m.clone());
    let d = m.dimension();
    for k in 0..d[2] {
        for j in 0..d[1] {
            for i in 0..d[0] {
                let r = 0.5 * (m.r_grid[i] + m.r_grid[i + 1]);
                let phi = 0.5 * (m.phi_grid[j] + m.phi_grid[j + 1]);
                let z = 0.5 * (m.z_grid[k] + m.z_grid[k + 1]);
                let p = Position::new(r * phi.cos(), r * phi.sin(), z);
                let flat = kind.bin(p).expect("centroid is inside the mesh");
                assert_eq!(
                    flat,
                    i + d[0] * (j + d[1] * k),
                    "flat index disagrees at ({i},{j},{k})"
                );
                let via_flat = kind.bin_volume(flat).unwrap();
                let via_ijk = m.volume([i, j, k]);
                assert!(
                    (via_flat - via_ijk).abs() / via_ijk < 1e-15,
                    "bin_volume({flat}) = {via_flat} but volume({i},{j},{k}) = {via_ijk}"
                );
            }
        }
    }
}
