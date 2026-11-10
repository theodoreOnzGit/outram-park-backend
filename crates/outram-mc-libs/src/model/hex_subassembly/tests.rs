//! Tests of [`super`] (gh #1327). Every dimension is a generic round
//! number, not any reactor's design.

use super::*;
use crate::model::isotope::isotopes::{Fe56, Na23, U235, U238};
use crate::model::shape::Shape;

fn cm(x: f64) -> Length {
    Length::new::<centimeter>(x)
}

/// A generic pin: fuel 0.25 cm, clad 0.28 / 0.33 cm.
fn pin() -> PinDesign {
    PinDesign {
        fuel_radius: cm(0.25),
        clad_inner_radius: cm(0.28),
        clad_outer_radius: cm(0.33),
    }
}

/// `rings` rings at 0.8 cm pitch in a duct that leaves 0.1 cm between the
/// outer pins' cladding and the flats: `F_in = sqrt(3)(r-1)p + D + 0.2`.
fn generic(rings: usize) -> HexSubassembly {
    let f_in = 3.0_f64.sqrt() * (rings as f64 - 1.0) * 0.8 + 0.66 + 0.2;
    HexSubassembly::new(cm(f_in), cm(0.2), cm(10.0), cm(0.8), rings, pin())
}

fn materials(wire: bool) -> SubassemblyMaterials {
    let atoms = |v: Vec<(Species, f64)>| {
        Material::from_atom_densities(v.into_iter().map(|(s, n)| (s, atoms_per_barn_cm(n))))
    };
    SubassemblyMaterials {
        fuel: atoms(vec![(U235.into(), 0.0096), (U238.into(), 0.0380)]).named("fuel"),
        bond: atoms(vec![(Na23.into(), 0.022)]).named("bond"),
        clad: atoms(vec![(Fe56.into(), 0.085)]).named("clad"),
        coolant: atoms(vec![(Na23.into(), 0.022)]).named("sodium"),
        duct: atoms(vec![(Fe56.into(), 0.085)]).named("duct"),
        wire: wire.then(|| atoms(vec![(Fe56.into(), 0.085)]).named("wire")),
    }
}

/// The ring a centre lies on: the hexagon of ring `k` (pins in the `x`
/// orientation) has apothem `k p sqrt(3)/2` and face normals at 30 + 60 j
/// degrees, so `k = max_j (n_j . c) / (p sqrt(3)/2)`.
fn ring_of(c: [f64; 2], p: f64) -> usize {
    let s = 0.5 * 3.0_f64.sqrt();
    let normals = [
        [s, 0.5],
        [0.0, 1.0],
        [-s, 0.5],
        [-s, -0.5],
        [0.0, -1.0],
        [s, -0.5],
    ];
    let m = normals
        .iter()
        .map(|n| n[0] * c[0] + n[1] * c[1])
        .fold(f64::NEG_INFINITY, f64::max);
    (m / (p * s)).round() as usize
}

/// **V&V: pin count per ring count, and the bundle's structure.**
///
/// *Methodology.* For 1 to 10 rings: the builder's count, `3r(r-1)+1`
/// (1, 7, 19, 37, 61, 91, 127, 169, 217, 271), against the number of
/// centres the `HexLattice` port gives; every ring `k` holds `6k` pins
/// (ring found from the geometry, [`ring_of`], independently of the
/// lattice's own order); OpenMC's ring order puts the centre pin last and
/// the outer ring first; each pin's nearest neighbour is at exactly one
/// pitch, so the bundle is triangular, and the neighbours of the centre
/// pin are at `(+-p, 0)` (rows parallel to the duct's flats).
/// *Prediction.* All exact (the nearest-neighbour distance to 1e-12 cm).
/// *Result (2026-11-10):* passes.
#[test]
fn pin_count_and_bundle_structure() {
    let want = [1, 7, 19, 37, 61, 91, 127, 169, 217, 271];
    assert_eq!(pins_in_rings(0), 0);
    for (r, n) in (1..=10).zip(want) {
        let sa = generic(r);
        assert_eq!(sa.pin_count(), n, "{r} rings");
        let c = sa.pin_centres_cm();
        assert_eq!(c.len(), n, "{r} rings: centres");
        let mut per_ring = vec![0usize; r];
        for x in &c {
            per_ring[ring_of(*x, 0.8)] += 1;
        }
        assert_eq!(per_ring[0], 1);
        for (k, count) in per_ring.iter().enumerate().skip(1) {
            assert_eq!(*count, 6 * k, "{r} rings, ring {k}");
        }
        assert_eq!(*c.last().unwrap(), [0.0, 0.0], "centre pin last");
        if r > 1 {
            assert_eq!(ring_of(c[0], 0.8), r - 1, "outer ring first");
        }
        for (i, a) in c.iter().enumerate() {
            let nearest = c
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, b)| (a[0] - b[0]).hypot(a[1] - b[1]))
                .fold(f64::INFINITY, f64::min);
            if n > 1 {
                assert!(
                    (nearest - 0.8).abs() < 1e-12,
                    "{r} rings, pin {i}: {nearest}"
                );
            }
        }
    }
    let c = generic(2).pin_centres_cm();
    for x in [0.8, -0.8] {
        assert!(c
            .iter()
            .any(|p| (p[0] - x).abs() < 1e-12 && p[1].abs() < 1e-12));
    }
}

/// **V&V: every pin centre inside the duct, and no pin-pin or pin-duct
/// overlap, by the exact solid relations on the built shape.**
///
/// *Methodology.* Build 1, 3 and 6 rings ([`generic`]); walk the shape
/// tree: every child is enclosed by its parent and no two siblings overlap
/// (`Solid::encloses`, `Solid::overlaps`, the relations
/// [`crate::model::Model::assemble`] uses), and every pin centre is held by
/// the coolant prism. Then designs that should fail: a duct 0.01 cm too
/// small across the flats for the 0.1 cm clearance plus 0.11 (so the
/// cladding pokes out), and a pitch below the cladding diameter.
/// *Prediction.* The generic designs pass; the two broken ones are refused
/// by [`HexSubassembly::check`] (and so by `build`).
/// *Result (2026-11-10):* passes.
#[test]
fn pins_inside_duct_and_disjoint() {
    fn walk(s: &Shape, count: &mut usize) {
        let kids = s.contained();
        for (i, k) in kids.iter().enumerate() {
            assert!(
                s.solid().encloses(k.solid()),
                "{} does not hold {}",
                s.solid().describe(),
                k.solid().describe()
            );
            for o in &kids[i + 1..] {
                assert!(
                    !k.solid().overlaps(o.solid()),
                    "{} overlaps {}",
                    k.solid().describe(),
                    o.solid().describe()
                );
            }
            *count += 1;
            walk(k, count);
        }
    }
    for r in [1, 3, 6] {
        let sa = generic(r);
        let shape = sa.build(materials(false)).unwrap();
        let mut count = 0;
        walk(&shape, &mut count);
        // coolant prism + (clad, bond, fuel) per pin
        assert_eq!(count, 1 + 3 * sa.pin_count(), "{r} rings");
        let coolant = shape.contained()[0].solid().clone();
        for c in sa.pin_centres_cm() {
            assert!(coolant.holds([c[0], c[1], 0.0]));
        }
    }
    let f_tight = 3.0_f64.sqrt() * 2.0 * 0.8 + 0.66 - 0.01;
    let tight = HexSubassembly::new(cm(f_tight), cm(0.2), cm(10.0), cm(0.8), 3, pin());
    assert!(tight.check().unwrap_err().to_string().contains("pokes out"));
    assert!(tight.build(materials(false)).is_err());
    let crowded = HexSubassembly::new(cm(6.0), cm(0.2), cm(10.0), cm(0.65), 3, pin());
    assert!(crowded
        .check()
        .unwrap_err()
        .to_string()
        .contains("overlaps"));
}

/// **V&V: the wire wrap's room is checked, touching allowed.**
///
/// *Methodology.* Wire `s = 0.1 cm`, lead 20 cm. At pitch `p = D + s =
/// 0.76 cm` the wire touches the next pin (allowed); at 0.75 cm it cuts
/// into it (refused). With a duct exactly `s` clear of the outer cladding
/// (`F_in = sqrt(3)(r-1)p + D + 2s`) it passes; 0.01 cm less is refused.
/// *Prediction.* As stated. *Result (2026-11-10):* passes.
#[test]
fn wire_wrap_room_is_checked() {
    let wire = WireWrap {
        diameter: cm(0.1),
        lead: cm(20.0),
    };
    let f = |p: f64| 3.0_f64.sqrt() * 2.0 * p + 0.66 + 0.2;
    let sa = |p: f64, f_in: f64| {
        HexSubassembly::new(cm(f_in), cm(0.2), cm(10.0), cm(p), 3, pin()).wire_wrap(wire)
    };
    sa(0.76, f(0.76)).check().unwrap();
    assert!(sa(0.75, f(0.76))
        .check()
        .unwrap_err()
        .to_string()
        .contains("overlaps"));
    assert!(sa(0.76, f(0.76) - 0.01)
        .check()
        .unwrap_err()
        .to_string()
        .contains("pokes out"));
}

/// **V&V: volume fractions against the analytic areas, and against the
/// built shape sampled point by point.**
///
/// *Methodology.* 3 rings, generic design, wire `s = 0.1`, lead 20 cm,
/// pitch 0.8 cm (> D + s). By hand: `F_in = sqrt(3) 1.6 + 0.86 = 3.631281`,
/// outer `F = 4.031281`; hexagon areas `(sqrt 3/2) F^2` = 14.073977 and
/// 11.419587 cm^2; per pin fuel `pi 0.25^2 = 0.1963495`, bond
/// `pi (0.28^2 - 0.25^2) = 0.0499513`, clad `pi (0.33^2 - 0.28^2) =
/// 0.0958186`; wire `cos theta = 20 / sqrt(400 + (0.76 pi)^2) = 0.9929494`,
/// area `pi 0.01 / 4 / cos theta = 0.00790975`; times 19 pins. Then a
/// 1201 x 1201 grid over the duct's bounding box at mid-height, each point
/// classified by the built shape's own solids (`Solid::holds`, deepest
/// shape that holds it).
/// *Prediction.* Analytic fractions to 1e-6 (the hand values are rounded
/// to 7 digits); they sum to 1 to 1e-12; the grid agrees with the analytic
/// fractions without the wire (it is smeared, not drawn) to 2e-3 absolute
/// (grid step 0.0039 cm against pin radii 0.25-0.33 cm).
/// *Result (2026-11-10):* passes; worst grid difference recorded in the
/// assertion message on failure only.
#[test]
fn volume_fractions_analytic_and_sampled() {
    let sa = generic(3).wire_wrap(WireWrap {
        diameter: cm(0.1),
        lead: cm(20.0),
    });
    let v = sa.volume_fractions();
    let outer = 14.073977;
    let n = 19.0;
    let hand = [
        ("fuel", v.fuel, n * 0.1963495 / outer),
        ("bond", v.bond, n * 0.0499513 / outer),
        ("clad", v.clad, n * 0.0958186 / outer),
        ("wire", v.wire, n * 0.00790975 / outer),
        ("duct", v.duct, (outer - 11.419587) / outer),
    ];
    for (name, got, want) in hand {
        assert!(
            (got.get::<ratio>() - want).abs() < 1e-6,
            "{name}: {got:?} vs {want}"
        );
    }
    let total = v.fuel + v.bond + v.clad + v.wire + v.coolant + v.duct;
    assert!((total.get::<ratio>() - 1.0).abs() < 1e-12);
    assert!((sa.wire_area_cm2() - 0.00790975).abs() < 1e-8);

    // Sample the built shape (no wire drawn: it is in the coolant).
    let shape = sa.build(materials(true)).unwrap();
    fn deepest<'a>(s: &'a Shape, p: [f64; 3]) -> Option<&'a Shape> {
        if !s.solid().holds(p) {
            return None;
        }
        Some(
            s.contained()
                .iter()
                .find_map(|k| deepest(k, p))
                .unwrap_or(s),
        )
    }
    let f_out = 3.0_f64.sqrt() * 1.6 + 0.86 + 0.4;
    let half_x = f_out / 3.0_f64.sqrt(); // circumradius, vertices on +-x
    let half_y = 0.5 * f_out;
    let m = 1201;
    let mut counts = std::collections::HashMap::<String, usize>::new();
    let mut inside = 0usize;
    for i in 0..m {
        for j in 0..m {
            let x = -half_x + (i as f64 + 0.5) * 2.0 * half_x / m as f64;
            let y = -half_y + (j as f64 + 0.5) * 2.0 * half_y / m as f64;
            if let Some(s) = deepest(&shape, [x, y, 0.0]) {
                inside += 1;
                *counts.entry(s.material().name().to_string()).or_default() += 1;
            }
        }
    }
    let frac = |name: &str| counts.get(name).copied().unwrap_or(0) as f64 / inside as f64;
    let no_wire = sa.wire_area_cm2() * n / outer;
    for (name, want) in [
        ("fuel", v.fuel.get::<ratio>()),
        ("bond", v.bond.get::<ratio>()),
        ("clad", v.clad.get::<ratio>()),
        ("duct", v.duct.get::<ratio>()),
        (
            "sodium + wire (smeared)",
            v.coolant.get::<ratio>() + no_wire,
        ),
    ] {
        assert!(
            (frac(name) - want).abs() < 2e-3,
            "{name}: sampled {} vs {want}",
            frac(name)
        );
    }
}

/// **V&V: the smeared coolant is the volume mix, by atom density.**
///
/// *Methodology.* Coolant Na-23 at 0.022, wire Fe-56 at 0.085 atoms/b-cm,
/// the generic 3-ring design with the 0.1 cm wire. The coolant region's
/// material must be Na-23 `(1 - f) 0.022` and Fe-56 `f 0.085` with
/// `f = wire_fraction_of_coolant`; a coolant given by weight fractions is
/// refused for smearing; a wire without a material, or a material without
/// a wire, is refused.
/// *Prediction.* Densities to 1e-15 atoms/b-cm. *Result (2026-11-10):* passes.
#[test]
fn smeared_coolant_is_the_volume_mix() {
    let sa = generic(3).wire_wrap(WireWrap {
        diameter: cm(0.1),
        lead: cm(20.0),
    });
    let f = sa.wire_fraction_of_coolant();
    assert!(f > 0.0 && f < 0.1, "{f}");
    let shape = sa.build(materials(true)).unwrap();
    let coolant = shape.contained()[0].material();
    assert_eq!(coolant.name(), "sodium + wire (smeared)");
    match &coolant.composition {
        Composition::AtomDensities(v) => {
            assert_eq!(v.len(), 2);
            for (s, d) in v {
                let want = if *s == Species::from(Na23) {
                    (1.0 - f) * 0.022
                } else {
                    f * 0.085
                };
                assert!((to_atoms_per_barn_cm(*d) - want).abs() < 1e-15, "{s:?}");
            }
        }
        other => panic!("{other:?}"),
    }
    let mut by_weight = materials(true);
    by_weight.coolant = Material::from_fractions(
        crate::model::Basis::Weight,
        [(Na23, Ratio::new::<ratio>(1.0))],
    );
    assert!(sa.build(by_weight).is_err());
    assert!(sa.build(materials(false)).is_err());
    assert!(generic(3).build(materials(true)).is_err());
}

/// A pin with no gap (`clad inner = fuel`) builds without a bond cylinder;
/// bad dimensions and zero rings are refused.
#[test]
fn no_gap_and_bad_inputs() {
    let tight_pin = PinDesign {
        fuel_radius: cm(0.28),
        clad_inner_radius: cm(0.28),
        clad_outer_radius: cm(0.33),
    };
    let sa = HexSubassembly::new(cm(4.0), cm(0.2), cm(10.0), cm(0.8), 3, tight_pin);
    let shape = sa.build(materials(false)).unwrap();
    let first_pin = &shape.contained()[0].contained()[0];
    assert_eq!(first_pin.contained().len(), 1);
    assert_eq!(first_pin.contained()[0].material().name(), "fuel");
    assert!(first_pin.contained()[0].contained().is_empty());
    assert!(
        HexSubassembly::new(cm(4.0), cm(0.2), cm(10.0), cm(0.8), 0, pin())
            .check()
            .is_err()
    );
    assert!(
        HexSubassembly::new(cm(4.0), cm(-0.2), cm(10.0), cm(0.8), 3, pin())
            .check()
            .is_err()
    );
    let inverted = PinDesign {
        fuel_radius: cm(0.3),
        clad_inner_radius: cm(0.28),
        clad_outer_radius: cm(0.33),
    };
    assert!(
        HexSubassembly::new(cm(4.0), cm(0.2), cm(10.0), cm(0.8), 3, inverted)
            .check()
            .is_err()
    );
}
