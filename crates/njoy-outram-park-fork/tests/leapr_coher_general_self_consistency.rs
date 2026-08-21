//! Self-consistency check for the generalized coherent-elastic path
//! (`coher::coher_general_with_constants`, added 2026-08-19 for bead
//! `op-jw4a` / GitHub issue #24) against the existing hand-coded
//! `coher::coher_with_constants` built-in lattices (Al, Fe, graphite).
//!
//! DIAGNOSTIC (asserts nothing): prints the cumulative structure factor from
//! both paths at a spread of energies plus the SiC result, rather than
//! pinning tolerances. General and built-in agree to ~1e-10 relative at low
//! energy for all three lattices (expected: both integrate the same Bragg
//! edges there) and diverge at higher energy, up to O(1) relative by 5 eV.
//!
//! **That divergence is root-caused (2026-08-19) and is a limitation of the
//! ported built-in path, not of the general one.** NJOY's `coher` sweeps a
//! *fixed* index box for the cubic lattices (`i1, i2, i3` over `-15..=15`,
//! `leapr.f90:2698, 2725`) and `a`/`c`-derived bounds for the hexagonal ones.
//! Those boxes do not enclose the whole `tau^2 <= econ * E_max` sphere at
//! `E_max = 5` eV, so the built-in path silently drops edges above roughly
//! 0.8 eV (Al), 1 eV (Fe) and 0.5 eV (graphite), while the general path bounds
//! its loops from the requested `E_max` and keeps them. It is asserted, with
//! measured ratios, by
//! `leapr::coher::general::tests::builtin_index_boxes_truncate_above_their_own_lattices_reach`.
//! The two paths are therefore interchangeable in the thermal range only —
//! which is the range that matters, since the Debye-Waller factor and the
//! `1/E` envelope have made the coherent-elastic channel negligible long
//! before 1 eV. Do not mistake it for the general path over-counting.
use njoy_outram_park_fork::leapr::coher::*;
use njoy_outram_park_fork::leapr::vintage::PhysicalConstants;

fn cum(e: &[(f64, f64)], x: f64) -> f64 {
    e.iter()
        .take_while(|&&(ee, _)| ee <= x)
        .map(|&(_, f)| f)
        .sum()
}

#[ignore = "diagnostic sweep, asserts nothing -- run explicitly with --ignored"]
#[test]
fn general_vs_builtin_lattice_diagnostic() {
    let c = PhysicalConstants::default();
    let b = 1.0_f64;
    // Al fcc built from NJOY's own sigma_coh
    let bal = (1.495f64 * 100.0 / (4.0 * std::f64::consts::PI)).sqrt();
    let a = 4.04e-8;
    let al = CrystalStructure {
        cell_cm: [[a, 0.0, 0.0], [0.0, a, 0.0], [0.0, 0.0, a]],
        basis: vec![
            BasisAtom {
                fractional: [0.0, 0.0, 0.0],
                b_coh_fm: bal,
                label: "Al",
            },
            BasisAtom {
                fractional: [0.0, 0.5, 0.5],
                b_coh_fm: bal,
                label: "Al",
            },
            BasisAtom {
                fractional: [0.5, 0.0, 0.5],
                b_coh_fm: bal,
                label: "Al",
            },
            BasisAtom {
                fractional: [0.5, 0.5, 0.0],
                b_coh_fm: bal,
                label: "Al",
            },
        ],
        name: "Al",
    };
    let _ = b;
    let g = coher_general_with_constants(&al, 1, 5.0, c);
    let bi = coher_with_constants(CoherentLattice::Aluminium, 1, 5.0, c);
    for e in [0.05, 0.1, 0.3, 0.5, 0.8, 1.0, 2.0, 5.0] {
        let (sg, sb) = (cum(&g.edges, e), cum(&bi.edges, e));
        println!(
            "Al  E={e:5.2}  general={sg:.10e} builtin={sb:.10e} rel={:.3e}",
            (sg - sb).abs() / sb
        );
    }
    println!(
        "Al first allowed general: {:e}",
        g.edges.iter().find(|&&(_, f)| f > 1e-12).unwrap().0
    );
    println!(
        "Al first allowed builtin: {:e}",
        bi.edges.iter().find(|&&(_, f)| f > 1e-12).unwrap().0
    );

    // Fe bcc
    let bfe = (12.9f64 * 100.0 / (4.0 * std::f64::consts::PI)).sqrt();
    let a = 2.86e-8;
    let fe = CrystalStructure {
        cell_cm: [[a, 0.0, 0.0], [0.0, a, 0.0], [0.0, 0.0, a]],
        basis: vec![
            BasisAtom {
                fractional: [0.0, 0.0, 0.0],
                b_coh_fm: bfe,
                label: "Fe",
            },
            BasisAtom {
                fractional: [0.5, 0.5, 0.5],
                b_coh_fm: bfe,
                label: "Fe",
            },
        ],
        name: "Fe",
    };
    let g = coher_general_with_constants(&fe, 1, 5.0, c);
    let bi = coher_with_constants(CoherentLattice::Iron, 1, 5.0, c);
    for e in [0.05, 0.1, 0.3, 0.5, 1.0, 2.0, 5.0] {
        let (sg, sb) = (cum(&g.edges, e), cum(&bi.edges, e));
        println!(
            "Fe  E={e:5.2}  general={sg:.10e} builtin={sb:.10e} rel={:.3e}",
            (sg - sb).abs() / sb
        );
    }

    // graphite hexagonal: a=2.4573, c=6.700 ; 4 atoms
    let (aa, cc) = (2.4573e-8, 6.700e-8);
    let bc = (5.50f64 * 100.0 / (4.0 * std::f64::consts::PI)).sqrt();
    let s3 = 3f64.sqrt() / 2.0;
    let gr = CrystalStructure {
        cell_cm: [[aa, 0.0, 0.0], [-0.5 * aa, s3 * aa, 0.0], [0.0, 0.0, cc]],
        basis: vec![
            BasisAtom {
                fractional: [0.0, 0.0, 0.0],
                b_coh_fm: bc,
                label: "C",
            },
            BasisAtom {
                fractional: [0.0, 0.0, 0.5],
                b_coh_fm: bc,
                label: "C",
            },
            BasisAtom {
                fractional: [1.0 / 3.0, 2.0 / 3.0, 0.0],
                b_coh_fm: bc,
                label: "C",
            },
            BasisAtom {
                fractional: [2.0 / 3.0, 1.0 / 3.0, 0.5],
                b_coh_fm: bc,
                label: "C",
            },
        ],
        name: "graphite",
    };
    let g = coher_general_with_constants(&gr, 1, 5.0, c);
    let bi = coher_with_constants(CoherentLattice::Graphite, 1, 5.0, c);
    for e in [0.05, 0.1, 0.3, 0.5, 1.0, 2.0, 5.0] {
        let (sg, sb) = (cum(&g.edges, e), cum(&bi.edges, e));
        println!(
            "Gr  E={e:5.2}  general={sg:.10e} builtin={sb:.10e} rel={:.3e}",
            (sg - sb).abs() / sb
        );
    }
    println!(
        "Gr first allowed general: {:e}",
        g.edges.iter().find(|&&(_, f)| f > 1e-12).unwrap().0
    );
    println!(
        "Gr first allowed builtin: {:e}",
        bi.edges.iter().find(|&&(_, f)| f > 1e-12).unwrap().0
    );

    // SiC
    let sic = GeneralCrystal::SiliconCarbide3C.structure();
    let g = coher_general_with_constants(&sic, 1, 5.0, c);
    println!("SiC edges: {}", g.edges.len());
    for &(e, f) in g.edges.iter().take(14) {
        println!("   E={e:.6e} f={f:.6e}");
    }
    println!(
        "SiC S(0.0253)={:.6e} -> sigma(no DW) = {:.5} b",
        cum(&g.edges, 0.0253),
        cum(&g.edges, 0.0253) / 0.0253
    );
}
