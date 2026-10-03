//! Tests of the transport-state geometry work (`GeometryExt`): crossings,
//! reflections and the GitHub #168 shell regression. Moved with that code
//! from the former `geometry/geometry.rs` on 2026-10-02 (GitHub #486).

use super::*;
use crate::geometry::cell::{Cell, HalfSpaceSense, RegionToken, SurfaceToken};
use crate::geometry::geometry::{Crossing, Geometry};
use crate::geometry::position::{Direction, Position};
use crate::geometry::surface::{BoundaryType, Sphere, SurfaceKind, XPlane};
use crate::geometry::universe::Universe;

/// A reflective XPlane must send a +x particle back along −x.
#[test]
fn reflective_plane_flips_direction() {
    let surfaces = vec![SurfaceKind::XPlane(XPlane {
        x0: 1.0,
        bc: BoundaryType::Reflective,
    })];
    let cells = vec![Cell::material(
        1,
        vec![RegionToken::HalfSpace {
            surface_idx: 0,
            sense: HalfSpaceSense::Inside,
        }],
        0,
        293.6,
    )];
    let universes = vec![Universe {
        id: 0,
        cell_indices: vec![0],
    }];
    let geom = Geometry {
        surfaces,
        cells,
        universes,
        lattices: vec![],
        root_universe: 0,
    };
    let crossed = geom.cross_surface(
        0,
        Position::new(1.0, 0.0, 0.0),
        Direction::new(1.0, 0.0, 0.0),
        &mut 0x5EED_0259_u64,
    );
    assert!(crossed.alive);
    assert!(
        (crossed.u.u + 1.0).abs() < 1e-12,
        "reflected u.u = {}",
        crossed.u.u
    );
    // The particle bounced back to the inside (negative) half-space.
    assert_eq!(
        crossed.on_surface,
        SurfaceToken::on(0, HalfSpaceSense::Inside)
    );
}

/// A sphere with a vacuum BC must kill the particle on crossing.
#[test]
fn vacuum_sphere_leaks() {
    let surfaces = vec![SurfaceKind::Sphere(Sphere {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        r: 5.0,
        bc: BoundaryType::Vacuum,
    })];
    let cells = vec![Cell::material(
        1,
        vec![RegionToken::HalfSpace {
            surface_idx: 0,
            sense: HalfSpaceSense::Inside,
        }],
        0,
        293.6,
    )];
    let universes = vec![Universe {
        id: 0,
        cell_indices: vec![0],
    }];
    let geom = Geometry {
        surfaces,
        cells,
        universes,
        lattices: vec![],
        root_universe: 0,
    };
    let crossed = geom.cross_surface(
        0,
        Position::new(5.0, 0.0, 0.0),
        Direction::new(1.0, 0.0, 0.0),
        &mut 0x5EED_0259_u64,
    );
    assert!(!crossed.alive, "vacuum crossing should kill the particle");
}

// ── GitHub #168 / op-mzvp.2.11 regression ─────────────────────────────
//
// A concentric-shell geometry — the shape that exposed the surface-tracking
// defect. Four spherical regions, the outermost reflective, so nothing can
// legitimately escape:
//
//   surface 0: r = 1  (transmissive)   cell 0: inside(0)               ball
//   surface 1: r = 2  (transmissive)   cell 1: outside(0) & inside(1)  shell
//   surface 2: r = 3  (transmissive)   cell 2: outside(1) & inside(2)  shell
//   surface 3: r = 4  (reflective)     cell 3: outside(2) & inside(3)  shell
//
// The failure mode this guards against: a particle that has just crossed an
// internal sphere sits exactly on it, `locate` re-derives the sign of
// `evaluate` there, picks the cell it was LEAVING, and the next
// `distance_to_boundary` finds no forward surface (the coincident one is
// suppressed, the other bounds the cell behind it) — so the particle streams
// to infinity and the history leaks.
fn concentric_shells() -> Geometry {
    let tr = BoundaryType::Transmissive;
    let sphere = |r: f64, bc| {
        SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r,
            bc,
        })
    };
    let inside = |i| RegionToken::HalfSpace {
        surface_idx: i,
        sense: HalfSpaceSense::Inside,
    };
    let outside = |i| RegionToken::HalfSpace {
        surface_idx: i,
        sense: HalfSpaceSense::Outside,
    };
    let shell = |id, i_in, i_out, mat| {
        Cell::material(
            id,
            vec![outside(i_in), inside(i_out), RegionToken::Intersection],
            mat,
            293.6,
        )
    };
    Geometry {
        surfaces: vec![
            sphere(1.0, tr),
            sphere(2.0, tr),
            sphere(3.0, tr),
            sphere(4.0, BoundaryType::Reflective),
        ],
        cells: vec![
            Cell::material(1, vec![inside(0)], 0, 293.6),
            shell(2, 0, 1, 1),
            shell(3, 1, 2, 2),
            shell(4, 2, 3, 3),
        ],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0, 1, 2, 3],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

/// **GitHub #168 regression.** Crossing every internal transmissive boundary
/// of a concentric-shell geometry, in **both** directions, must land the
/// particle in the adjacent cell — never back in the one it left — and must
/// leave a finite forward boundary distance.
///
/// Swept over near-radial *and* strongly grazing incidence: the grazing rays
/// are the ones that broke, because there the tangential motion dominates and
/// the sign of `evaluate` at the crossing point is pure round-off.
#[test]
fn crossing_a_shell_boundary_lands_in_the_adjacent_cell() {
    let geom = concentric_shells();
    // (surface index, radius, cell inside it, cell outside it)
    let boundaries = [
        (0usize, 1.0, 0usize, 1usize),
        (1, 2.0, 1, 2),
        (2, 3.0, 2, 3),
    ];
    // Radial through near-tangent. `tan` is the tangential component per unit
    // radial component: 1e6 is ~1e-6 rad off tangent.
    let tangential = [0.0, 1.0, 1.0e3, 1.0e6];

    for (i_surf, radius, cell_in, cell_out) in boundaries {
        for tan in tangential {
            for outward in [true, false] {
                let radial = if outward { 1.0 } else { -1.0 };
                // Hit point on the +x axis; radial along x, tangential along y.
                let r_hit = Position::new(radius, 0.0, 0.0);
                let u = Direction::from_unnormalised(radial, tan, 0.0);

                let crossed = geom.cross_surface(i_surf, r_hit, u, &mut 0x5EED_0259_u64);
                assert!(crossed.alive, "internal boundary must be transmissive");

                let expect_cell = if outward { cell_out } else { cell_in };
                let path = geom
                    .locate(crossed.r, crossed.u, crossed.on_surface)
                    .unwrap_or_else(|| {
                        panic!(
                            "lost after crossing surface {i_surf} (r={radius}, tan={tan}, \
                             outward={outward}) — GH #168 regressed"
                        )
                    });
                assert_eq!(
                    path.leaf().cell,
                    expect_cell,
                    "crossing surface {i_surf} (r={radius}, tan={tan}, outward={outward}) \
                     landed in cell {} not {expect_cell} — GH #168 regressed",
                    path.leaf().cell
                );

                // And the new cell must present a finite forward boundary:
                // an INFINITY here is exactly how the leak started.
                let hit = geom.distance_to_boundary(&path);
                assert!(
                    hit.distance.is_finite(),
                    "no forward boundary from cell {expect_cell} after crossing surface \
                     {i_surf} (tan={tan}, outward={outward}) — GH #168 regressed"
                );
                assert!(
                    matches!(hit.crossing, Crossing::Surface(_)),
                    "expected a surface crossing ahead, got {:?}",
                    hit.crossing
                );
            }
        }
    }
}

/// **GitHub #168 regression.** The outermost reflective sphere must send the
/// particle back into the outer shell with its side recorded as `Inside`,
/// at grazing incidence as well as head-on.
#[test]
fn reflecting_off_the_outer_sphere_stays_in_the_outer_shell() {
    let geom = concentric_shells();
    for tan in [0.0, 1.0, 1.0e3, 1.0e6] {
        let r_hit = Position::new(4.0, 0.0, 0.0);
        let u = Direction::from_unnormalised(1.0, tan, 0.0);
        let crossed = geom.cross_surface(3, r_hit, u, &mut 0x5EED_0259_u64);
        assert!(
            crossed.alive,
            "reflective sphere must not kill the particle"
        );
        assert_eq!(
            crossed.on_surface,
            SurfaceToken::on(3, HalfSpaceSense::Inside),
            "reflection must record the inward side (tan={tan})"
        );
        let path = geom
            .locate(crossed.r, crossed.u, crossed.on_surface)
            .unwrap_or_else(|| panic!("lost after reflecting (tan={tan}) — GH #168 regressed"));
        assert_eq!(
            path.leaf().cell,
            3,
            "must stay in the outer shell (tan={tan})"
        );
        assert!(
            geom.distance_to_boundary(&path).distance.is_finite(),
            "no forward boundary after reflecting (tan={tan}) — GH #168 regressed"
        );
    }
}

/// **GitHub #168 regression, end to end.** A full `run_keff_csg` power
/// iteration over an all-reflective concentric-shell geometry must lose
/// essentially no neutrons.
///
/// # Methodology
///
/// The [`concentric_shells`] model with fissile Godiva-like HEU in every
/// region (so a lost history is not masked by absorption), transported
/// through [`crate::physics::transport_csg::run_keff_csg_reactor_physics`]
/// with the leakage spectrum enabled — 400 particles, 5 inactive + 10 active
/// generations. Every surface but the outermost is transmissive and the
/// outermost is reflective, so the *only* way to score leakage is a tracking
/// failure. Pass criterion: total leakage < 1e-3 per source neutron.
///
/// This is a **harness conservation check, not physics V&V** — it constrains
/// the tracker, not the eigenvalue.
///
/// # Results
///
/// Measured 2026-09-10. With the two fixes below disabled — the
/// [`SurfaceToken`] membership override and the direction-aware
/// [`SurfaceKind::sense`] fallback — this geometry leaks **0.0860** per
/// source neutron (k_eff 2.03432 ± 0.01644). With both in place it leaks
/// **exactly 0.0** (k_eff 2.22599 ± 0.01072).
///
/// The same disabled-mechanism run reproduces the reported 0.8463
/// leakage on the four-region pebble
/// ([`crate::pebble_beds::fhr_pebble`]'s
/// `reflective_pebble_transport_does_not_leak`), so it is a faithful
/// emulation of the pre-fix tracker rather than an approximation of it.
/// The pebble is the more sensitive gate of the two; this test is here
/// because it is minimal and needs no fissile-shell tuning to reproduce.
#[test]
fn reflective_concentric_shells_do_not_leak() {
    use crate::material::material::{Material, NuclideComponent};
    use crate::material::nuclide::Nuclide;
    use crate::physics::keff::KeffSettings;
    use crate::physics::transport_csg::{run_keff_csg_reactor_physics, SourceBox};
    use crate::tally::filter::{EnergyFilter, FilterKind};
    use crate::tally::tally::{ScoreType, Tally, TallyBin};

    let nuclides = vec![
        Nuclide::from_core("U235").unwrap(),
        Nuclide::from_core("U238").unwrap(),
    ];
    let heu = |id| Material {
        id,
        name: "HEU".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent {
                nuclide_idx: 0,
                atom_density: 4.4994e-2,
            },
            NuclideComponent {
                nuclide_idx: 1,
                atom_density: 2.4984e-3,
            },
        ],
    };
    let materials = vec![heu(1), heu(2), heu(3), heu(4)];

    let geom = concentric_shells();
    let edges = vec![0.0, 0.625, 2.0e7];
    let mut tally = Tally {
        id: 0,
        name: "shells".into(),
        filters: vec![FilterKind::Energy(EnergyFilter {
            bins: edges.clone(),
        })],
        scores: vec![ScoreType::Flux],
        bins: vec![TallyBin::default(); 2],
    };
    let mut leak = vec![TallyBin::default(); edges.len() - 1];
    let settings = KeffSettings {
        n_particles: 400,
        n_inactive: 5,
        n_active: 10,
        ..KeffSettings::default()
    };
    let source = SourceBox {
        lower: Position::new(-3.5, -3.5, -3.5),
        upper: Position::new(3.5, 3.5, 3.5),
    };
    let res = run_keff_csg_reactor_physics(
        &geom, &materials, &nuclides, source, &settings, &mut tally, &edges, &mut leak,
    );
    let leaked = leak
        .iter()
        .map(|b| b.mean(settings.n_active as u64))
        .sum::<f64>()
        / settings.n_particles as f64;
    eprintln!(
        "[GH #168] concentric shells: k_eff = {:.5} +/- {:.5}, leakage = {leaked:.3e}",
        res.k_mean, res.k_std
    );
    assert!(
        leaked < 1.0e-3,
        "all-reflective concentric shells leaked {leaked} per source neutron — GH #168 regressed"
    );
}
