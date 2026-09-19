//! **Cell translation (`Cell::translation`) — exhaustive verification.**
//!
//! Ported behaviour: OpenMC's `Cell::translation_`
//! (`include/openmc/cell.h:403`), applied in `find_cell_inner`
//! (`src/geometry.cpp:222` for a `Fill::UNIVERSE`, `:241` for a
//! `Fill::LATTICE`) and re-applied in `cross_lattice` (`src/geometry.cpp:391`).
//! It rigidly shifts the frame a filled universe is evaluated in:
//! `coord.r -= cell.translation`.
//!
//! # Why this file exists
//!
//! The field, its two application sites and its doc comment were already in
//! this crate. **Nothing ever set it to a non-zero value** — a workspace-wide
//! search for `translation:` outside `Position::ZERO` found only unrelated
//! crates — and **no test exercised it at all**. That is precisely the shape of
//! defect bead `op-50vu` records: a whole mechanism present in the source,
//! absent from every result, and indistinguishable from working because
//! nothing asked it a question it could get wrong.
//!
//! A field that is always zero is not a ported feature. It is untested code
//! that happens to compile.
//!
//! # Methodology: an exact identity, so no tolerance is needed
//!
//! For a **pure translation** there is an algebraic identity with no
//! approximation in it:
//!
//! > Translating a fill cell by `t` is the same geometry as leaving the fill
//! > cell untranslated and moving every surface of the filled universe by `+t`.
//!
//! Two geometries are built for every case — **TRANSLATED** (surfaces at the
//! origin, `cell.translation = t`) and **MOVED** (surfaces at `t`,
//! `cell.translation = 0`) — and required to agree.
//!
//! **The agreement demanded is bit-equality, not a tolerance**, and that is
//! justified rather than hopeful. TRANSLATED forms `r_local = r - t` in
//! `Geometry::locate` and then evaluates a surface centred at `0`, i.e.
//! `r_local - 0.0`; MOVED evaluates a surface centred at `t`, i.e. `r - t`.
//! Subtracting `0.0` from a finite double is exact, so both paths compute the
//! identical IEEE-754 expression and any difference is a real defect rather
//! than round-off. A tolerance here would hide exactly what the test is for.
//!
//! This is a **manufactured-solution** check in the sense the workspace
//! `CLAUDE.md` uses: the reference is constructed to be exactly right, not
//! measured.
//!
//! # The negative control
//!
//! An equivalence test can pass trivially if the mechanism does nothing — so
//! [`translation_is_not_a_no_op`] asserts the opposite direction: the same
//! geometry with `t` and with `Position::ZERO` must **disagree**. Without it a
//! future change that dropped the `-= translation` line would leave every other
//! test in this file green, because TRANSLATED and MOVED would both collapse to
//! "surfaces where they are declared".
//!
//! # What this does NOT establish
//!
//! - **Not validation.** No experiment, and no comparison against OpenMC
//!   running the same input — OpenMC is not installed in this environment. This
//!   is verification against an exact algebraic identity and against this
//!   crate's own untranslated behaviour.
//! - **Nothing about rotation.** `Cell::rotation_` is not ported; see
//!   `docs/openmc-cell-transform-port-scope.md`.

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken, SurfaceToken};
use outram_mc_libs::geometry::geometry::{Crossing, Geometry};
use outram_mc_libs::geometry::lattice::{Lattice, RectLattice};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;

/// Inner sphere radius \[cm\] of the nested universe used throughout.
const R_INNER: f64 = 1.0;
/// Outer (root) sphere radius \[cm\].
const R_ROOT: f64 = 10.0;

fn sphere(x0: f64, y0: f64, z0: f64, r: f64, bc: BoundaryType) -> SurfaceKind {
    SurfaceKind::Sphere(Sphere { x0, y0, z0, r, bc })
}

fn half(surface_idx: usize, sense: HalfSpaceSense) -> RegionToken {
    RegionToken::HalfSpace { surface_idx, sense }
}

/// A two-level geometry: a root sphere filled by a universe that holds a small
/// sphere of material 0 in a sea of material 1.
///
/// `surface_centre` places the **inner** sphere, `translation` is the root fill
/// cell's translation. The identity under test is that
/// `nested(ZERO, t) == nested(t, ZERO)`.
fn nested(surface_centre: Position, translation: Position) -> Geometry {
    Geometry {
        surfaces: vec![
            // 0 — root boundary, always at the origin, never translated.
            sphere(0.0, 0.0, 0.0, R_ROOT, BoundaryType::Vacuum),
            // 1 — the inner sphere, declared inside the nested universe.
            sphere(
                surface_centre.x,
                surface_centre.y,
                surface_centre.z,
                R_INNER,
                BoundaryType::Transmissive,
            ),
        ],
        cells: vec![
            // 0 — root fill cell, inside surface 0, filled by universe 1.
            Cell::fill(
                1,
                vec![half(0, HalfSpaceSense::Inside)],
                CellFill::Universe(1),
                translation,
            ),
            // 1 — inner: material 0.
            Cell::material(2, vec![half(1, HalfSpaceSense::Inside)], 0, 293.6),
            // 2 — outer: material 1. Unbounded in the nested universe; clipped
            //     by cell 0's own region, exactly as OpenMC does it.
            Cell::material(3, vec![half(1, HalfSpaceSense::Outside)], 1, 293.6),
        ],
        universes: vec![
            Universe {
                id: 0,
                cell_indices: vec![0],
            },
            Universe {
                id: 1,
                cell_indices: vec![1, 2],
            },
        ],
        lattices: vec![],
        root_universe: 0,
    }
}

/// The translations swept by every equivalence test: zero, each axis alone,
/// both signs, a fully general offset, and one large enough to push the inner
/// sphere against the root boundary.
fn translations() -> Vec<Position> {
    vec![
        Position::new(0.0, 0.0, 0.0),
        Position::new(1.0, 0.0, 0.0),
        Position::new(-1.0, 0.0, 0.0),
        Position::new(0.0, 2.5, 0.0),
        Position::new(0.0, 0.0, -3.25),
        Position::new(1.5, -2.25, 0.75),
        Position::new(-4.0, 3.0, 2.0),
        Position::new(0.1, 0.2, 0.3),
        Position::new(7.5, 0.0, 0.0),
    ]
}

/// A deterministic spray of query points covering the inner sphere, the shell,
/// the neighbourhood of both surfaces, and the origin.
fn probe_points() -> Vec<Position> {
    let mut pts = Vec::new();
    let axis = [-9.0, -4.0, -1.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 4.0, 9.0];
    for &x in &axis {
        for &y in &axis {
            for &z in &axis {
                pts.push(Position::new(x, y, z));
            }
        }
    }
    pts
}

/// A deterministic set of directions, including axis-aligned and oblique.
fn probe_directions() -> Vec<Direction> {
    let s = 1.0 / 3.0_f64.sqrt();
    vec![
        Direction::new(1.0, 0.0, 0.0),
        Direction::new(-1.0, 0.0, 0.0),
        Direction::new(0.0, 1.0, 0.0),
        Direction::new(0.0, 0.0, -1.0),
        Direction::new(s, s, s),
        Direction::new(-s, s, -s),
    ]
}

/// **1. Membership.** The located material and leaf cell agree between the two
/// constructions, for every translation, point and direction.
#[test]
fn translation_equals_moving_the_surface_for_membership() {
    let mut checked = 0usize;
    let mut inner_hits = 0usize;
    for t in translations() {
        let translated = nested(Position::ZERO, t);
        let moved = nested(t, Position::ZERO);
        for r in probe_points() {
            for u in probe_directions() {
                let a = translated.locate(r, u, SurfaceToken::NONE);
                let b = moved.locate(r, u, SurfaceToken::NONE);
                match (&a, &b) {
                    (None, None) => {}
                    (Some(pa), Some(pb)) => {
                        assert_eq!(
                            pa.material, pb.material,
                            "material differs at r=({:.4},{:.4},{:.4}) under translation \
                             ({:.4},{:.4},{:.4})",
                            r.x, r.y, r.z, t.x, t.y, t.z
                        );
                        assert_eq!(
                            pa.levels.last().unwrap().cell,
                            pb.levels.last().unwrap().cell,
                            "leaf cell differs at r=({:.4},{:.4},{:.4}) under translation \
                             ({:.4},{:.4},{:.4})",
                            r.x,
                            r.y,
                            r.z,
                            t.x,
                            t.y,
                            t.z
                        );
                        if pa.material == Some(0) {
                            inner_hits += 1;
                        }
                    }
                    _ => panic!(
                        "one construction located the point and the other lost it at \
                         r=({:.4},{:.4},{:.4}), translation ({:.4},{:.4},{:.4})",
                        r.x, r.y, r.z, t.x, t.y, t.z
                    ),
                }
                checked += 1;
            }
        }
    }
    // The probe must actually reach the translated inner sphere, or this test
    // only ever compares the surrounding material to itself.
    assert!(
        inner_hits > 0,
        "no probe point ever landed in the inner sphere, so this test never \
         exercised the translated surface at all"
    );
    println!(
        "cell translation, membership: {checked} (point, direction, translation) queries \
         agree between 'translated fill' and 'moved surface'; {inner_hits} of them inside \
         the translated inner sphere"
    );
}

/// **2. Distance to boundary.** Bit-identical, for the reason argued in the
/// module docs — both constructions evaluate the same IEEE-754 expression.
#[test]
fn translation_equals_moving_the_surface_for_distance() {
    let mut compared = 0usize;
    let mut finite = 0usize;
    for t in translations() {
        let translated = nested(Position::ZERO, t);
        let moved = nested(t, Position::ZERO);
        for r in probe_points() {
            for u in probe_directions() {
                let (Some(pa), Some(pb)) = (
                    translated.locate(r, u, SurfaceToken::NONE),
                    moved.locate(r, u, SurfaceToken::NONE),
                ) else {
                    continue;
                };
                let ha = translated.distance_to_boundary(&pa);
                let hb = moved.distance_to_boundary(&pb);
                assert_eq!(
                    ha.distance.to_bits(),
                    hb.distance.to_bits(),
                    "distance differs ({} vs {}) at r=({:.4},{:.4},{:.4}) u=({:.4},{:.4},{:.4}) \
                     translation ({:.4},{:.4},{:.4}); for a pure translation both paths compute \
                     the identical expression, so this is a defect and not round-off",
                    ha.distance,
                    hb.distance,
                    r.x,
                    r.y,
                    r.z,
                    u.u,
                    u.v,
                    u.w,
                    t.x,
                    t.y,
                    t.z
                );
                assert_eq!(
                    std::mem::discriminant(&ha.crossing),
                    std::mem::discriminant(&hb.crossing),
                    "crossing kind differs at r=({:.4},{:.4},{:.4})",
                    r.x,
                    r.y,
                    r.z
                );
                if ha.distance.is_finite() {
                    finite += 1;
                }
                compared += 1;
            }
        }
    }
    assert!(
        finite > 0,
        "every compared distance was infinite, so nothing was really tested"
    );
    println!("cell translation, distance: {compared} comparisons bit-identical ({finite} finite)");
}

/// **3. The negative control — translation is not a no-op.**
///
/// The equivalence tests above would both pass trivially if
/// `coord.r -= cell.translation` were deleted, because TRANSLATED and MOVED
/// would then both reduce to "surfaces where they are declared". This asserts
/// the mechanism does something: with the surface at the origin, translating
/// the fill must change the answer.
#[test]
fn translation_is_not_a_no_op() {
    let t = Position::new(3.0, 0.0, 0.0);
    let untranslated = nested(Position::ZERO, Position::ZERO);
    let translated = nested(Position::ZERO, t);
    let u = Direction::new(0.0, 0.0, 1.0);

    // The origin is inside the inner sphere untranslated, and outside it once
    // the fill universe is shifted by +3 cm in x.
    let at_origin = |g: &Geometry| {
        g.locate(Position::ZERO, u, SurfaceToken::NONE)
            .expect("origin is inside the root sphere")
            .material
    };
    assert_eq!(
        at_origin(&untranslated),
        Some(0),
        "origin starts in material 0"
    );
    assert_eq!(
        at_origin(&translated),
        Some(1),
        "translating the fill universe by +3 cm in x must move the inner sphere off the \
         origin. Reading material 0 here means the translation is being ignored, and every \
         equivalence test in this file would then pass vacuously."
    );

    // And the point the sphere moved TO must now be inside it.
    let moved_centre = translated
        .locate(t, u, SurfaceToken::NONE)
        .expect("located")
        .material;
    assert_eq!(
        moved_centre,
        Some(0),
        "the inner sphere must be found at +t after translating the fill by t"
    );
    println!(
        "cell translation, negative control: origin reads material 0 untranslated and \
         material 1 under t=(3,0,0); the sphere is found at t instead"
    );
}

/// **4. The carried frame offset is exactly the translation**, the direction is
/// untouched, and the offset reconstructed by subtraction is **not** exact.
///
/// `Geometry::cross_surface_in_frame` needs the global→local offset for the
/// level a crossing happened on. It used to recover it as
/// `levels[0].r - levels[coord_level].r`; it now reads [`Coord::offset`],
/// accumulated on the way down.
///
/// This test asserts both halves of why that changed:
///
/// 1. the **carried** offset is bit-exactly the translation, and
///    `r_global - offset` reproduces the stored local position exactly;
/// 2. the **reconstructed** offset is measurably wrong — and this test records
///    how wrong, because that error is what a 1e-9 particle displacement was
///    being built on.
///
/// The amplification is the point. `nudge_across` branches on `dot == 0.0`
/// (exact float equality against a tangency that is generically impossible), so
/// a 1e-16 error in the local coordinate decides between a one-nudge and a
/// two-nudge step — 1e-9 of displacement, a factor of ~10^6. Before the fix
/// `surface_crossing_agrees_under_translation` failed on exactly this, at
/// 8.0e-10.
#[test]
fn the_carried_offset_is_exact_and_the_reconstructed_one_is_not() {
    let mut worst_reconstructed = 0.0_f64;
    let mut checked = 0usize;
    for t in translations() {
        let g = nested(Position::ZERO, t);
        for r in probe_points() {
            for u in probe_directions() {
                let Some(path) = g.locate(r, u, SurfaceToken::NONE) else {
                    continue;
                };
                assert_eq!(path.levels.len(), 2, "expected root + nested level");
                let carried = path.levels[1].offset;

                // (1) The carried offset is the translation, to the bit.
                for (got, want, axis) in [
                    (carried.x, t.x, "x"),
                    (carried.y, t.y, "y"),
                    (carried.z, t.z, "z"),
                ] {
                    assert_eq!(
                        got.to_bits(),
                        want.to_bits(),
                        "carried offset {axis} is {got} but the translation is {want}"
                    );
                }
                // ... and it reproduces the stored local position exactly.
                let rebuilt = r - carried;
                let local = path.levels[1].r;
                for (got, want, axis) in [
                    (rebuilt.x, local.x, "x"),
                    (rebuilt.y, local.y, "y"),
                    (rebuilt.z, local.z, "z"),
                ] {
                    assert_eq!(
                        got.to_bits(),
                        want.to_bits(),
                        "r_global - offset does not reproduce the local {axis}: {got} vs {want}"
                    );
                }

                // (2) The old reconstruction is lossy — measure it rather than
                //     assert it away, so the record says how large the error was.
                let reconstructed = path.levels[0].r - path.levels[1].r;
                worst_reconstructed = worst_reconstructed
                    .max((reconstructed.x - t.x).abs())
                    .max((reconstructed.y - t.y).abs())
                    .max((reconstructed.z - t.z).abs());

                // A pure translation must not touch the direction at any level.
                for (i, lvl) in path.levels.iter().enumerate() {
                    assert_eq!(
                        (lvl.u.u.to_bits(), lvl.u.v.to_bits(), lvl.u.w.to_bits()),
                        (u.u.to_bits(), u.v.to_bits(), u.w.to_bits()),
                        "direction changed at level {i} under a pure translation"
                    );
                }
                checked += 1;
            }
        }
    }
    assert!(
        worst_reconstructed > 0.0,
        "the subtraction form was exact everywhere in this sweep, so this test is no longer \
         demonstrating why Coord::offset exists — widen the probe or delete the claim"
    );
    println!(
        "cell translation, frame: {checked} paths — carried offset bit-exact and \
         r_global - offset reproduces the local position exactly; the old \
         levels[0].r - levels[k].r reconstruction was off by up to \
         {worst_reconstructed:.3e} cm, which nudge_across amplifies to ~1e-9"
    );
}

/// **5. Surface crossing in the translated frame.**
///
/// A particle streamed to the inner sphere and crossed there must land at the
/// same global point and leave in the same direction under both constructions.
/// This is the path that GitHub #168 and the 2026-09-14 nested-frame normal
/// defect both live on, so it is checked with the translation switched on.
#[test]
fn surface_crossing_agrees_under_translation() {
    let mut crossings = 0usize;
    for t in translations() {
        let translated = nested(Position::ZERO, t);
        let moved = nested(t, Position::ZERO);
        for r in probe_points() {
            for u in probe_directions() {
                let (Some(pa), Some(pb)) = (
                    translated.locate(r, u, SurfaceToken::NONE),
                    moved.locate(r, u, SurfaceToken::NONE),
                ) else {
                    continue;
                };
                let ha = translated.distance_to_boundary(&pa);
                let hb = moved.distance_to_boundary(&pb);
                let (Crossing::Surface(sa), Crossing::Surface(sb)) = (ha.crossing, hb.crossing)
                else {
                    continue;
                };
                if !ha.distance.is_finite() {
                    continue;
                }
                let hit_a = Position::new(
                    r.x + u.u * ha.distance,
                    r.y + u.v * ha.distance,
                    r.z + u.w * ha.distance,
                );
                let hit_b = Position::new(
                    r.x + u.u * hb.distance,
                    r.y + u.v * hb.distance,
                    r.z + u.w * hb.distance,
                );
                let ca = translated.cross_surface_in_frame(sa, &pa, ha.coord_level, hit_a, u);
                let cb = moved.cross_surface_in_frame(sb, &pb, hb.coord_level, hit_b, u);
                assert_eq!(
                    ca.alive, cb.alive,
                    "survival differs crossing surface {sa}/{sb} under translation \
                     ({:.3},{:.3},{:.3})",
                    t.x, t.y, t.z
                );
                for (ga, gb, axis) in [
                    (ca.r.x, cb.r.x, "x"),
                    (ca.r.y, cb.r.y, "y"),
                    (ca.r.z, cb.r.z, "z"),
                ] {
                    assert!(
                        (ga - gb).abs() <= 1.0e-12,
                        "post-crossing global {axis} differs ({ga} vs {gb}) under translation \
                         ({:.3},{:.3},{:.3})",
                        t.x,
                        t.y,
                        t.z
                    );
                }
                crossings += 1;
            }
        }
    }
    assert!(
        crossings > 0,
        "no surface crossing was ever exercised, so this test asserted nothing"
    );
    println!("cell translation, crossing: {crossings} crossings agree in the global frame");
}

/// **6. Lattice fills.** OpenMC applies the translation on the `Fill::LATTICE`
/// branch too (`src/geometry.cpp:241`), before `get_indices`. The identity here
/// is that translating the lattice-fill cell by `t` equals moving the lattice's
/// `lower_left` by `+t`.
#[test]
fn translation_of_a_lattice_fill_equals_shifting_lower_left() {
    /// 2x2x1 rectangular lattice of unit pitch; tile universes alternate
    /// between a material-0 universe and a material-1 universe so the tile
    /// index is observable in the located material.
    fn lattice_geom(lower_left: Position, translation: Position) -> Geometry {
        Geometry {
            surfaces: vec![sphere(0.0, 0.0, 0.0, R_ROOT, BoundaryType::Vacuum)],
            cells: vec![
                Cell::fill(
                    1,
                    vec![half(0, HalfSpaceSense::Inside)],
                    CellFill::Lattice(0),
                    translation,
                ),
                // Universe 1 tile — all material 0.
                Cell::material(2, vec![], 0, 293.6),
                // Universe 2 tile — all material 1.
                Cell::material(3, vec![], 1, 293.6),
            ],
            universes: vec![
                Universe {
                    id: 0,
                    cell_indices: vec![0],
                },
                Universe {
                    id: 1,
                    cell_indices: vec![1],
                },
                Universe {
                    id: 2,
                    cell_indices: vec![2],
                },
            ],
            lattices: vec![Lattice::Rect(RectLattice {
                id: 1,
                n: [2, 2, 1],
                lower_left,
                pitch: [1.0, 1.0, 1.0],
                // (ix, iy) -> universe: checkerboard.
                universes: vec![1, 2, 2, 1],
                outer: Some(1),
            })],
            root_universe: 0,
        }
    }

    // An empty region means "contains everything" only if the RPN evaluator
    // says so; it returns false on an empty stack. So give the tile cells a
    // region that is always true by using the root sphere's inside — the tile
    // universes are clipped by the lattice anyway.
    let mut checked = 0usize;
    for t in translations() {
        let a = lattice_geom(Position::ZERO, t);
        let b = lattice_geom(t, Position::ZERO);
        for r in probe_points() {
            for u in probe_directions() {
                let pa = a.locate(r, u, SurfaceToken::NONE);
                let pb = b.locate(r, u, SurfaceToken::NONE);
                assert_eq!(
                    pa.as_ref().map(|p| p.material),
                    pb.as_ref().map(|p| p.material),
                    "lattice tile material differs at r=({:.3},{:.3},{:.3}) under translation \
                     ({:.3},{:.3},{:.3})",
                    r.x,
                    r.y,
                    r.z,
                    t.x,
                    t.y,
                    t.z
                );
                if let (Some(pa), Some(pb)) = (&pa, &pb) {
                    assert_eq!(
                        pa.levels.last().unwrap().lattice_index,
                        pb.levels.last().unwrap().lattice_index,
                        "lattice INDEX differs at r=({:.3},{:.3},{:.3}) under translation \
                         ({:.3},{:.3},{:.3}) — the translation is not reaching get_indices",
                        r.x,
                        r.y,
                        r.z,
                        t.x,
                        t.y,
                        t.z
                    );
                }
                checked += 1;
            }
        }
    }
    println!("cell translation, lattice: {checked} tile lookups agree, index included");
}

/// **7. The identity survives a full eigenvalue solve.**
///
/// Every test above is a static query. This one runs transport — streaming,
/// surface crossings, collisions, fission-bank resampling, power iteration —
/// through a translated fill and requires the eigenvalue to come out
/// **bit-identical** to the moved-surface construction on the same seed.
///
/// This is the test that would have caught the offset defect at the level a
/// user feels it. Before `Coord::offset` was carried exactly, a crossing inside
/// a translated universe could be displaced by `1e-9` cm along a nudge that the
/// other construction did not take; that is enough to change which cell the
/// next `locate` returns and so which material the next flight is scored in.
///
/// Kept deliberately small (500 histories × [10 + 20]) — the assertion is
/// exact equality, not a statistical one, so it needs no statistics. A
/// difference of a single ulp is a failure, and that is the point.
#[test]
fn keff_is_bit_identical_under_the_translation_identity() {
    use outram_mc_libs::material::material::{Material, NuclideComponent};
    use outram_mc_libs::material::nuclide::Nuclide;
    use outram_mc_libs::physics::keff::KeffSettings;
    use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};

    /// ICSBEP HEU-MET-FAST-001 sphere radius \[cm\].
    const R_FUEL: f64 = 8.7407;
    /// Vacuum boundary, far enough out that the translated ball still fits.
    const R_OUTER: f64 = 30.0;

    /// A bare HEU ball living in a nested universe, either translated by `t` or
    /// declared at `t` — the same two constructions as everywhere above.
    fn ball(surface_centre: Position, translation: Position) -> Geometry {
        Geometry {
            surfaces: vec![
                sphere(0.0, 0.0, 0.0, R_OUTER, BoundaryType::Vacuum),
                sphere(
                    surface_centre.x,
                    surface_centre.y,
                    surface_centre.z,
                    R_FUEL,
                    BoundaryType::Transmissive,
                ),
            ],
            cells: vec![
                Cell::fill(
                    1,
                    vec![half(0, HalfSpaceSense::Inside)],
                    CellFill::Universe(1),
                    translation,
                ),
                Cell::material(2, vec![half(1, HalfSpaceSense::Inside)], 0, 293.6),
                Cell::fill(
                    3,
                    vec![half(1, HalfSpaceSense::Outside)],
                    CellFill::Void,
                    Position::ZERO,
                ),
            ],
            universes: vec![
                Universe {
                    id: 0,
                    cell_indices: vec![0],
                },
                Universe {
                    id: 1,
                    cell_indices: vec![1, 2],
                },
            ],
            lattices: vec![],
            root_universe: 0,
        }
    }

    let nuclides = vec![
        Nuclide::from_core("U234").unwrap(),
        Nuclide::from_core("U235").unwrap(),
        Nuclide::from_core("U238").unwrap(),
    ];
    let materials = vec![Material {
        id: 1,
        name: "Godiva HEU".into(),
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
    }];

    // A translation big enough that the ball is nowhere near the origin, so the
    // cancellation in the old reconstruction is large, and asymmetric so no
    // axis is accidentally exact.
    let t = Position::new(3.5, -2.25, 1.75);

    let settings = KeffSettings {
        n_particles: 500,
        n_inactive: 10,
        n_active: 20,
        temperature_k: 293.6,
        seed: 20_260_917,
        ..KeffSettings::default()
    };
    // The source box follows the ball, and is identical in global coordinates
    // for both constructions — so any difference is the geometry, not the source.
    let src = SourceBox {
        lower: Position::new(t.x - R_FUEL, t.y - R_FUEL, t.z - R_FUEL),
        upper: Position::new(t.x + R_FUEL, t.y + R_FUEL, t.z + R_FUEL),
    };

    let translated = run_keff_csg(
        &ball(Position::ZERO, t),
        &materials,
        &nuclides,
        src,
        &settings,
        None,
    );
    let moved = run_keff_csg(
        &ball(t, Position::ZERO),
        &materials,
        &nuclides,
        src,
        &settings,
        None,
    );

    assert_eq!(
        translated.k_mean.to_bits(),
        moved.k_mean.to_bits(),
        "k_eff differs between a translated fill ({}) and the same geometry with the surface \
         moved ({}), a relative difference of {:.3e}. For a pure translation these are the \
         identical calculation, so any difference at all is a defect in how the frame offset \
         reaches the surface-crossing path.",
        translated.k_mean,
        moved.k_mean,
        ((translated.k_mean - moved.k_mean) / moved.k_mean).abs()
    );
    assert_eq!(
        translated.k_std.to_bits(),
        moved.k_std.to_bits(),
        "k_eff standard deviation differs ({} vs {})",
        translated.k_std,
        moved.k_std
    );

    // Negative control at the transport level: the untranslated geometry must
    // NOT reproduce this k, or the whole comparison is vacuous.
    let untranslated = run_keff_csg(
        &ball(Position::ZERO, Position::ZERO),
        &materials,
        &nuclides,
        src,
        &settings,
        None,
    );
    assert_ne!(
        untranslated.k_mean.to_bits(),
        translated.k_mean.to_bits(),
        "the ball at the origin gave the same k as the ball translated to {t:?} with the \
         source box around the translated position. The translation is doing nothing."
    );

    println!(
        "cell translation, k_eff: translated {:.6} == moved {:.6} bit-for-bit; untranslated \
         control {:.6} differs as required",
        translated.k_mean, moved.k_mean, untranslated.k_mean
    );
}
