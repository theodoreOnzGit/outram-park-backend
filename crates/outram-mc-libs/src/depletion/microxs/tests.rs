//! Analytic tests for the `MicroXS` / `collapse_rate` port (gh:#1625).
//! Each prediction is in the doc comment; all measured equal to 1e-15
//! (2026-11-17).

use super::*;

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

fn table(grid: &[f64], xs: &[f64], threshold: usize) -> NuclideReaction {
    NuclideReaction { low: ReactionTable { grid: grid.to_vec(), xs: xs.to_vec(), threshold }, high: None }
}

fn one(rx: NuclideReaction, energies: &[f64], flux: &[f64]) -> f64 {
    let m = MultigroupMicroXs::from_multigroup_flux(energies, flux, &["U235"], &["(n,gamma)"], |_, _| Some(rx.clone())).unwrap();
    m.get("U235", "(n,gamma)", 0).unwrap()
}

/// A constant 3 b in any group flux collapses to 3 b.
#[test]
fn constant_cross_section_collapses_to_itself() {
    let v = one(table(&[1.0, 10.0, 100.0], &[3.0, 3.0, 3.0], 0), &[1.0, 10.0, 100.0], &[2.0, 5.0]);
    assert!((v - 3.0).abs() < 1e-15, "{v}");
}

/// `σ = E` on `[1, 3]`. Two flat groups of equal flux: `0.5·1.5 + 0.5·2.5
/// = 2`; the group edge at 2 splits a grid segment, which upstream handles
/// with `max`/`min` of the bounds.
#[test]
fn linear_cross_section_split_by_a_group_edge() {
    let v = one(table(&[1.0, 3.0], &[1.0, 3.0], 0), &[1.0, 2.0, 3.0], &[1.0, 1.0]);
    assert!((v - 2.0).abs() < 1e-15, "{v}");
}

/// A threshold reaction (5 b from E = 3) in one flat group `[1, 4]`:
/// `5·1/3 = 5/3`. With the group entirely below threshold: 0.
#[test]
fn threshold_is_respected() {
    let rx = table(&[1.0, 2.0, 3.0, 4.0], &[5.0, 5.0], 2);
    assert!((one(rx.clone(), &[1.0, 4.0], &[1.0]) - 5.0 / 3.0).abs() < 1e-15);
    assert_eq!(one(rx, &[1.0, 2.0, 2.5], &[1.0, 1.0]), 0.0);
}

/// Temperature interpolation at `f = 0.25` between 2 b and 6 b: 3 b.
#[test]
fn temperature_interpolates_the_rate() {
    let low = ReactionTable { grid: vec![1.0, 2.0], xs: vec![2.0, 2.0], threshold: 0 };
    let high = ReactionTable { grid: vec![1.0, 2.0], xs: vec![6.0, 6.0], threshold: 0 };
    let rx = NuclideReaction { low, high: Some((high, 0.25)) };
    assert!((nuclide_collapse_rate(&rx, &[1.0, 2.0], &[1.0]) - 3.0).abs() < 1e-15);
}

/// A first group edge above the grid gives 0 (departure 2), a zero flux
/// gives zeros, an unknown reaction and a descending grid are refused.
#[test]
fn edge_cases_follow_upstream_or_the_documented_departure() {
    assert_eq!(one(table(&[1.0, 2.0], &[1.0, 1.0], 0), &[5.0, 6.0], &[1.0]), 0.0);
    assert_eq!(one(table(&[1.0, 2.0], &[1.0, 1.0], 0), &[1.0, 2.0], &[0.0]), 0.0);
    assert!(MultigroupMicroXs::from_multigroup_flux(&[1.0, 2.0], &[1.0], &["U235"], &["(n,foo)"], |_, _| None).is_err());
    assert!(MultigroupMicroXs::from_multigroup_flux(&[2.0, 1.0], &[1.0], &["U235"], &["fission"], |_, _| None).is_err());
    let none = MultigroupMicroXs::from_multigroup_flux(&[1.0, 2.0], &[1.0], &["Xe135"], &["fission"], |_, _| None).unwrap();
    assert_eq!(none.get("Xe135", "fission", 0), Some(0.0), "no data: a zero row, as upstream");
    assert_eq!(reaction_mt("(n,gamma)"), Some(102));
    assert_eq!(MICROXS_REACTIONS.len(), 86);
}

/// `merge`: union in upstream's order, `prefer` decides overlaps only.
#[test]
fn merge_takes_the_union() {
    let a = MultigroupMicroXs::new(vec![1.0, 2.0], names(&["U235"]), names(&["fission", "(n,gamma)"]), 1).unwrap();
    let b = MultigroupMicroXs::new(vec![10.0, 20.0], names(&["U235", "Xe135"]), names(&["(n,gamma)"]), 1).unwrap();
    let m = a.merge(&b, true).unwrap();
    assert_eq!(m.nuclides(), &names(&["U235", "Xe135"])[..]);
    assert_eq!(m.get("U235", "(n,gamma)", 0), Some(10.0));
    assert_eq!(m.get("U235", "fission", 0), Some(1.0));
    assert_eq!(m.get("Xe135", "(n,gamma)", 0), Some(20.0));
    assert_eq!(m.get("Xe135", "fission", 0), Some(0.0));
    let s = a.merge(&b, false).unwrap();
    assert_eq!(s.get("U235", "(n,gamma)", 0), Some(2.0));
    assert_eq!(s.get("Xe135", "(n,gamma)", 0), Some(20.0));
    let c = MultigroupMicroXs::new(vec![0.0; 2], names(&["U235"]), names(&["fission"]), 2).unwrap();
    assert!(a.merge(&c, true).is_err());
}

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}
