use super::*;

/// A 4-ring, y-orientation lattice matching the `hexagonal-lattice` notebook:
/// centre (0,0), pitch 1.25, big-pin (universe index 1) at the first element
/// of every ring, regular pin (index 0) elsewhere, outer = 2.
fn notebook_lattice() -> HexLattice {
    let outer_ring: Vec<usize> = std::iter::once(1).chain(std::iter::repeat(0).take(17)).collect();
    let ring_1: Vec<usize> = std::iter::once(1).chain(std::iter::repeat(0).take(11)).collect();
    let ring_2: Vec<usize> = std::iter::once(1).chain(std::iter::repeat(0).take(5)).collect();
    let inner_ring: Vec<usize> = vec![1];
    HexLattice::from_rings(
        4,
        HexOrientation::Y,
        Position::ZERO,
        1.25,
        &[outer_ring, ring_1, ring_2, inner_ring],
        Some(2),
    )
}

/// The hexagon holds `3*nr*(nr-1)+1` real tiles; the rest of the skewed square
/// array is [`HEX_NONE`]. For 4 rings that is 37 valid tiles.
#[test]
fn valid_tile_count_matches_hexagon_formula() {
    let lat = notebook_lattice();
    let nr = lat.n_rings as i32;
    let n = lat.n_side() as i32;
    let mut valid = 0;
    for iy in 0..n {
        for ix in 0..n {
            if lat.are_valid_indices([ix, iy, 0]) {
                valid += 1;
            }
        }
    }
    assert_eq!(valid, (3 * nr * (nr - 1) + 1) as i32, "4-ring hexagon should have 37 tiles");
    assert_eq!(valid, 37);
}

/// Round-trip: placing a particle exactly at a tile centre must recover that
/// tile's skewed index via [`HexLattice::get_indices`]. This exercises the
/// centre-offset geometry and the Voronoi nearest-centre refinement together.
#[test]
fn get_indices_recovers_every_tile_center() {
    let lat = notebook_lattice();
    let n = lat.n_side() as i32;
    let u = Direction::new(1.0, 0.0, 0.0);
    for iy in 0..n {
        for ix in 0..n {
            let i = [ix, iy, 0];
            if !lat.are_valid_indices(i) {
                continue;
            }
            // Tile centre in the lattice (global) frame = center + center_offset.
            let c = lat.center_offset(i);
            let got = lat.get_indices(Position::new(c.x, c.y, 0.0), u);
            assert_eq!(got, i, "tile centre of {i:?} located as {got:?}");
        }
    }
}

/// The notebook fills the first element of each of the 4 rings with the big
/// pin (index 1) and every other tile with the regular pin (index 0): 4 big
/// pins, 33 regular pins.
#[test]
fn ring_fill_counts_match_notebook() {
    let lat = notebook_lattice();
    let big = lat.universes.iter().filter(|&&u| u == 1).count();
    let pin = lat.universes.iter().filter(|&&u| u == 0).count();
    let none = lat.universes.iter().filter(|&&u| u == HEX_NONE).count();
    assert_eq!(big, 4, "one big pin per ring");
    assert_eq!(pin, 33, "remaining tiles are regular pins");
    assert_eq!(big + pin + none, lat.universes.len());
    // The central tile is the innermost ring's single element → a big pin.
    let centre = [lat.n_rings as i32 - 1, lat.n_rings as i32 - 1, 0];
    assert_eq!(lat.universe_at(centre), Some(1), "central tile is the big pin");
}

/// Geometry of the y-orientation tile: its flat faces have outward normals at
/// ±30°, ±90°, ±150° (never along ±x). So a particle leaving the central tile
///   - along +y (the delta face normal) exits at exactly half the flat-to-flat
///     pitch (`pitch/2`), crossing into `[0,+1,0]`;
///   - along +x exits through the +30° (beta) face at `pitch/2 / cos(30°)`,
///     crossing into `[+1,0,0]`.
/// Both are checked against the ported `HexLattice::distance`.
#[test]
fn distance_from_center_matches_hex_face_geometry() {
    let lat = notebook_lattice();
    let centre = [lat.n_rings as i32 - 1, lat.n_rings as i32 - 1, 0];
    let half = 0.5 * lat.pitch[0];

    // +y → delta face → exactly half-pitch, cross into [0,+1,0].
    let (dy, ty) = lat.distance(Position::ZERO, Direction::new(0.0, 1.0, 0.0), centre);
    assert!((dy - half).abs() < 1e-9, "+y distance {dy} should be half-pitch {half}");
    assert_eq!(ty, [0, 1, 0], "+y crosses into the +delta neighbour");

    // +x → beta face at 30° → half-pitch / cos(30°), cross into [+1,0,0].
    let (dx, tx) = lat.distance(Position::ZERO, Direction::new(1.0, 0.0, 0.0), centre);
    let expect_x = half / (30.0_f64.to_radians().cos());
    assert!((dx - expect_x).abs() < 1e-9, "+x distance {dx} should be {expect_x}");
    assert_eq!(tx, [1, 0, 0], "+x crosses into the +beta neighbour");
}

/// A 2-ring (7-tile) **X-orientation** lattice, centre (0,0), radial pitch
/// 1.0 cm, with each valid tile's universe set to its own flat storage index
/// (`universe_at([ix,iy,0]) == (2*n_rings-1)*iy + ix`) and `outer = 999`.
/// Built directly (not via [`HexLattice::from_rings`]) so the universe of a
/// tile is trivially hand-checkable and independent of the ring→tile fill.
fn x_flat_id_lattice() -> HexLattice {
    let n_rings = 2usize;
    let n_side = 2 * n_rings - 1;
    let mut lat = HexLattice {
        id: 7,
        orientation: HexOrientation::X,
        n_rings,
        n_axial: 1,
        center: Position::ZERO,
        pitch: [1.0, 0.0],
        universes: vec![HEX_NONE; n_side * n_side],
        outer: Some(999),
    };
    let n = n_side as i32;
    for iy in 0..n {
        for ix in 0..n {
            let i = [ix, iy, 0];
            if lat.are_valid_indices(i) {
                let f = lat.flat_index(i);
                lat.universes[f] = f as i32;
            }
        }
    }
    lat
}

/// Round-trip through the **X-orientation** hex-lattice geometry, the
/// counterpart of the y-orientation `get_indices_recovers_every_tile_center`
/// / `distance_from_center_matches_hex_face_geometry` tests with hand-computed
/// X indices and distances.
///
/// # Methodology
/// 2-ring, 7-tile X lattice (see [`x_flat_id_lattice`]); the tile centres
/// [`HexLattice::center_offset`] places at, and the expected `[ix,iy,0]` /
/// flat-index universe for each, are the `cases` array below. The six
/// neighbours sit at 0°,60°,…,300°, so flat faces point ±x and vertices ±y.
/// For each tile-centre point the chain `get_indices → are_valid_indices →
/// universe_at` must recover the hand-computed index + universe, and
/// `get_local_position` must recentre it to ~(0,0,0). From the central tile,
/// `distance` along ±x exits a flat face at `pitch/2 = 0.5 cm` (translation
/// ±[1,0,0]); along ±y it exits a slanted face at `pitch/2 / cos30° = 1/√3 ≈
/// 0.57735 cm`.
///
/// # Results (2026-07-24)
/// All hand-computed indices, universes, and distances match; the
/// X-orientation branches of `center_offset`/`get_indices`/
/// `get_local_position`/`distance` are exercised live. (Does not exercise
/// `fill_lattice_x`, the ring→tile fill `from_rings` uses for X — that path
/// has a separate bug tracked in op-6tz.38.)
#[test]
fn x_orientation_round_trip() {
    let lat = x_flat_id_lattice();
    let u = Direction::new(1.0, 0.0, 0.0);
    let s = 3.0_f64.sqrt() / 2.0; // √3/2

    // (tile-centre point, expected [ix,iy,0], expected universe = flat index)
    let cases: [(Position, [i32; 3], usize); 7] = [
        (Position::new(0.0, 0.0, 0.0), [1, 1, 0], 4), // centre
        (Position::new(1.0, 0.0, 0.0), [2, 1, 0], 5), // +x neighbour
        (Position::new(-1.0, 0.0, 0.0), [0, 1, 0], 3), // -x neighbour
        (Position::new(0.5, s, 0.0), [1, 2, 0], 7), // upper-right
        (Position::new(-0.5, s, 0.0), [0, 2, 0], 6), // upper-left
        (Position::new(0.5, -s, 0.0), [2, 0, 0], 2), // lower-right
        (Position::new(-0.5, -s, 0.0), [1, 0, 0], 1), // lower-left
    ];
    for (p, expect_i, expect_u) in cases {
        let gi = lat.get_indices(p, u);
        assert_eq!(gi, expect_i, "get_indices({p:?}) = {gi:?}, expected {expect_i:?}");
        assert!(lat.are_valid_indices(gi), "{gi:?} should be a valid tile");
        assert_eq!(lat.universe_at(gi), Some(expect_u), "universe at {gi:?}");
        let loc = lat.get_local_position(p, gi);
        assert!(
            loc.x.abs() < 1e-12 && loc.y.abs() < 1e-12,
            "local position of tile centre {gi:?} = {loc:?}, expected ~(0,0)"
        );
    }

    // Corners of the 3x3 skewed array are outside the hexagon → outer.
    assert!(!lat.are_valid_indices([0, 0, 0]));
    assert!(!lat.are_valid_indices([2, 2, 0]));
    assert_eq!(lat.universe_at([0, 0, 0]), Some(999), "corner → outer");

    // Distance from the central tile.
    let centre = [1, 1, 0];
    let half = 0.5 * lat.pitch[0]; // 0.5
    let slant = half / 30.0_f64.to_radians().cos(); // 1/√3 ≈ 0.57735
    let (dx, tx) = lat.distance(Position::ZERO, Direction::new(1.0, 0.0, 0.0), centre);
    assert!((dx - half).abs() < 1e-9, "+x distance {dx} should be {half}");
    assert_eq!(tx, [1, 0, 0], "+x crosses into the +x neighbour");
    let (dnx, tnx) = lat.distance(Position::ZERO, Direction::new(-1.0, 0.0, 0.0), centre);
    assert!((dnx - half).abs() < 1e-9, "-x distance {dnx} should be {half}");
    assert_eq!(tnx, [-1, 0, 0], "-x crosses into the -x neighbour");
    let (dy, _) = lat.distance(Position::ZERO, Direction::new(0.0, 1.0, 0.0), centre);
    assert!((dy - slant).abs() < 1e-9, "+y distance {dy} should be {slant}");
    let (dny, _) = lat.distance(Position::ZERO, Direction::new(0.0, -1.0, 0.0), centre);
    assert!((dny - slant).abs() < 1e-9, "-y distance {dny} should be {slant}");
}

/// [`HexLattice::from_rings_3d`] stacks independent axial levels into one
/// storage array. Two-level, 2-ring, y-orientation lattice with disjoint
/// universe sets per level, so each level's contents are traceable regardless
/// of the intra-level ring→tile placement.
///
/// # Methodology / Results (2026-07-24)
/// - Storage length is `(2*n_rings-1)^2 * n_axial` and `pitch == [1.0, 2.0]`.
/// - Each axial block is a permutation of exactly that level's input universes
///   (level 0 = {10..15, 42}, level 1 = {20..25, 84}) — no cross-level bleed,
///   confirming the axial slice offset `(2*n_rings-1)^2 * iz`.
/// - A single axial level built through `from_rings_3d` reproduces the exact
///   planar fill of the 2-D [`HexLattice::from_rings`] (back-compat: the bottom
///   block equals `from_rings` of the same rings, byte for byte).
/// - `get_indices` at the bottom-level centre `z = -1.0 cm` selects `iz = 0`,
///   at the top-level centre `z = +1.0 cm` selects `iz = 1` (level centres are
///   `center.z ∓ (n_axial-1)/2 · axial_pitch = ∓1.0`), and the SAME planar tile
///   resolves to a level-0 universe at the bottom and a level-1 universe at the
///   top — the axial dimension is honoured end to end.
///
/// NOTE: this test deliberately does not assert *which* tile holds *which* ring
/// element, because the 2-D `fill_lattice_*` ring→tile placement is buggy for
/// small ring counts (tracked in op-6tz.38); it verifies only the axial stacking
/// added by `from_rings_3d`, which composes the 2-D fill unchanged per level.
#[test]
fn from_rings_3d_stacks_axial_levels() {
    let level0 = vec![vec![10usize, 11, 12, 13, 14, 15], vec![42usize]];
    let level1 = vec![vec![20usize, 21, 22, 23, 24, 25], vec![84usize]];
    let lat = HexLattice::from_rings_3d(
        9,
        HexOrientation::Y,
        Position::ZERO,
        1.0, // radial pitch [cm]
        2.0, // axial pitch [cm]
        &[level0.clone(), level1],
        Some(7),
    );
    assert_eq!(lat.n_axial, 2);
    assert_eq!(lat.n_rings, 2);
    assert_eq!(lat.pitch, [1.0, 2.0]);
    let block = lat.n_side() * lat.n_side();
    assert_eq!(lat.universes.len(), block * 2);

    // Each axial block is a permutation of that level's input universes only.
    let filled = |b: &[i32]| {
        let mut v: Vec<i32> = b.iter().copied().filter(|&u| u != HEX_NONE).collect();
        v.sort_unstable();
        v
    };
    let set0 = filled(&lat.universes[0..block]);
    let set1 = filled(&lat.universes[block..2 * block]);
    assert_eq!(set0, vec![10, 11, 12, 13, 14, 15, 42], "bottom level holds level-0 universes");
    assert_eq!(set1, vec![20, 21, 22, 23, 24, 25, 84], "top level holds level-1 universes");

    // A single-level from_rings_3d == the 2-D from_rings planar fill.
    let planar =
        HexLattice::from_rings(9, HexOrientation::Y, Position::ZERO, 1.0, &level0, Some(7));
    assert_eq!(
        &lat.universes[0..block],
        &planar.universes[..],
        "bottom axial level must reproduce the 2-D from_rings fill byte for byte"
    );

    // get_indices selects the axial level from z; the same planar tile resolves
    // into the correct level's (disjoint) universe set.
    let cx = lat.n_rings as i32 - 1; // planar centre index
    let u = Direction::new(0.0, 0.0, 1.0);
    let gi_bot = lat.get_indices(Position::new(0.0, 0.0, -1.0), u);
    assert_eq!(gi_bot, [cx, cx, 0], "z=-1.0 → bottom level, planar centre");
    let gi_top = lat.get_indices(Position::new(0.0, 0.0, 1.0), u);
    assert_eq!(gi_top, [cx, cx, 1], "z=+1.0 → top level, planar centre");
    let u_bot = lat.universe_at(gi_bot).unwrap() as i32;
    let u_top = lat.universe_at(gi_top).unwrap() as i32;
    assert!(set0.contains(&u_bot), "bottom-centre universe {u_bot} in level-0 set");
    assert!(set1.contains(&u_top), "top-centre universe {u_top} in level-1 set");
    assert_ne!(u_bot, u_top, "same planar tile at different axial levels differs");
}

/// Tiles outside the hexagon (unused corners) and beyond the grid resolve to
/// the `outer` universe.
#[test]
fn outside_hexagon_resolves_to_outer() {
    let lat = notebook_lattice();
    // Corner [0,0] of the skewed array is outside the hexagon (0+0 <= nr-2).
    assert!(!lat.are_valid_indices([0, 0, 0]));
    assert_eq!(lat.universe_at([0, 0, 0]), Some(2), "corner → outer");
    // Far away in the lattice frame → outer.
    let far = lat.get_indices(Position::new(100.0, 100.0, 0.0), Direction::new(1.0, 0.0, 0.0));
    assert_eq!(lat.universe_at(far), Some(2), "far point → outer");
}

// ---- op-6tz.38: ring→tile fill round-trip correctness -------------------

/// Build the ring-nested input for an `n_rings` hex lattice with a **distinct
/// universe per tile**, numbered as a running counter in the exact order
/// [`HexLattice::from_rings`] consumes: outermost ring first, then each ring
/// in its listed order, the single central tile last. Returns the rings and
/// the total tile count `N = 3*n_rings*(n_rings-1) + 1`. The central tile's
/// universe is therefore always `N - 1`.
fn distinct_rings(n_rings: usize) -> (Vec<Vec<usize>>, usize) {
    let mut rings = Vec::with_capacity(n_rings);
    let mut counter = 0usize;
    for j in 0..n_rings {
        let radius = n_rings - 1 - j; // outer-first → radius counts down
        let count = if radius == 0 { 1 } else { 6 * radius };
        rings.push((counter..counter + count).collect());
        counter += count;
    }
    (rings, counter)
}

/// Core round-trip check shared by the X/Y, `n_rings ∈ {2,3}` cases.
///
/// # Methodology
/// Build a distinct-universe-per-tile lattice via [`distinct_rings`] +
/// [`HexLattice::from_rings`], then assert three independent properties that
/// together pin the ring→tile fill:
///  1. **Bijection.** Every hexagon tile holds a distinct universe in
///     `0..N`, each used exactly once; every non-hexagon corner holds
///     [`HEX_NONE`]. (No two ring elements collide, none is dropped.)
///  2. **Centre.** The central element (`universe = N-1`, last in ring
///     order) lands on the geometric centre tile
///     `[n_rings-1, n_rings-1, 0]` — the specific op-6tz.38 symptom.
///  3. **Geometric round-trip.** For every tile `i`, routing its
///     [`HexLattice::center_offset`] centre through
///     [`HexLattice::get_indices`] → [`HexLattice::universe_at`] returns
///     exactly the universe stored at `i`. This is the literal
///     "`universe_at(get_indices(tile_centre)) == placed universe`" contract.
fn assert_ring_fill_round_trip(orientation: HexOrientation, n_rings: usize) {
    let (rings, n_tiles) = distinct_rings(n_rings);
    let pitch = 1.3;
    let outer = 99_999usize;
    let lat = HexLattice::from_rings(1, orientation, Position::ZERO, pitch, &rings, Some(outer));
    let nside = lat.n_side() as i32;

    // (1) bijection over the hexagon.
    let mut seen = vec![false; n_tiles];
    let mut valid_count = 0usize;
    for iy in 0..nside {
        for ix in 0..nside {
            let i = [ix, iy, 0];
            let stored = lat.universes[lat.flat_index(i)];
            if lat.are_valid_indices(i) {
                valid_count += 1;
                assert!(
                    stored >= 0 && (stored as usize) < n_tiles,
                    "tile {i:?} holds {stored}, outside 0..{n_tiles}"
                );
                assert!(!seen[stored as usize], "universe {stored} placed on two tiles");
                seen[stored as usize] = true;
            } else {
                assert_eq!(stored, HEX_NONE, "corner {i:?} must be HEX_NONE");
            }
        }
    }
    assert_eq!(valid_count, n_tiles, "valid tile count == N for {n_rings} rings");
    assert!(seen.iter().all(|&s| s), "every universe 0..{n_tiles} placed exactly once");

    // (2) central element at the centre tile.
    let centre = [n_rings as i32 - 1, n_rings as i32 - 1, 0];
    assert_eq!(
        lat.universe_at(centre),
        Some(n_tiles - 1),
        "central element (universe {}) must land on the centre tile {centre:?}",
        n_tiles - 1
    );

    // (3) full geometric round-trip for every tile.
    let u = Direction::new(1.0, 0.0, 0.0);
    for iy in 0..nside {
        for ix in 0..nside {
            let i = [ix, iy, 0];
            if !lat.are_valid_indices(i) {
                continue;
            }
            let c = lat.center_offset(i);
            let got = lat.get_indices(Position::new(c.x, c.y, 0.0), u);
            assert_eq!(got, i, "get_indices at centre of {i:?} = {got:?}");
            let placed = lat.universes[lat.flat_index(i)] as usize;
            assert_eq!(
                lat.universe_at(got),
                Some(placed),
                "round-trip: universe_at(get_indices(centre of {i:?})) must equal placed universe {placed}"
            );
        }
    }
}

/// Round-trip correctness of the ring→tile fill for the **Y** orientation,
/// `n_rings ∈ {2, 3}` (the op-6tz.38 regression: distinct central universe
/// must land on the centre tile).
///
/// # Results (2026-07-24)
/// Passes for both. `n_rings = 2` → `N = 7` tiles, centre universe `6`;
/// `n_rings = 3` → `N = 19` tiles, centre universe `18`. Bijection over all
/// hexagon tiles holds and every tile centre round-trips through
/// `get_indices` → `universe_at`. (Angular within-ring order is the
/// documented best-effort +x-start CCW convention; not diffed against
/// OpenMC — see the caveat on [`HexLattice::from_rings`].)
#[test]
fn ring_fill_round_trip_y() {
    assert_ring_fill_round_trip(HexOrientation::Y, 2);
    assert_ring_fill_round_trip(HexOrientation::Y, 3);
}

/// Round-trip correctness of the ring→tile fill for the **X** orientation,
/// `n_rings ∈ {2, 3}`.
///
/// # Results (2026-07-24)
/// Passes for both, same tile/centre counts as the Y case. Additionally, for
/// the X orientation the "+x start" convention is *exact* (a tile sits
/// precisely along +x at each radius), so the first outer-ring element
/// (universe `0`) lands on the skewed tile `[2*n_rings-2, n_rings-1, 0]`
/// (the +x extreme of the outer ring): `[2,1,0]` for 2 rings, `[4,2,0]` for
/// 3 rings. This is asserted below independently of the fill code.
#[test]
fn ring_fill_round_trip_x() {
    assert_ring_fill_round_trip(HexOrientation::X, 2);
    assert_ring_fill_round_trip(HexOrientation::X, 3);

    // +x-start convention (X orientation only, where +x is exact): the first
    // element of the outer ring (universe 0) is on the +x extreme tile.
    for n_rings in [2usize, 3] {
        let (rings, _) = distinct_rings(n_rings);
        let lat =
            HexLattice::from_rings(1, HexOrientation::X, Position::ZERO, 1.0, &rings, Some(0));
        let plus_x = [2 * n_rings as i32 - 2, n_rings as i32 - 1, 0];
        assert_eq!(
            lat.universes[lat.flat_index(plus_x)],
            0,
            "first outer-ring element must sit on the +x tile {plus_x:?} ({n_rings} rings)"
        );
    }
}

/// [`HexLattice::from_rings_3d`] with a single axial level must reproduce the
/// 2-D [`HexLattice::from_rings`] planar fill **byte for byte**, keeping the
/// 2-D and 3-D constructors consistent (only `n_axial` / `pitch[1]` differ).
///
/// # Results (2026-07-24)
/// For both orientations and `n_rings ∈ {2, 3}` the single-level 3-D
/// `universes` array equals the 2-D array exactly; `pitch` becomes
/// `[radial, axial]` and `n_axial == 1`.
#[test]
fn from_rings_3d_single_level_matches_2d() {
    for orientation in [HexOrientation::Y, HexOrientation::X] {
        for n_rings in [2usize, 3] {
            let (rings, _) = distinct_rings(n_rings);
            let pitch = 1.3;
            let planar =
                HexLattice::from_rings(3, orientation, Position::ZERO, pitch, &rings, Some(7));
            let stacked = HexLattice::from_rings_3d(
                3,
                orientation,
                Position::ZERO,
                pitch,
                2.0,
                &[rings.clone()],
                Some(7),
            );
            assert_eq!(stacked.n_axial, 1);
            assert_eq!(stacked.pitch, [pitch, 2.0]);
            assert_eq!(
                stacked.universes, planar.universes,
                "single-level from_rings_3d must equal from_rings ({orientation:?}, {n_rings} rings)"
            );
        }
    }
}
