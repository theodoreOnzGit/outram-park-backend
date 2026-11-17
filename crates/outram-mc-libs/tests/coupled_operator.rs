#![cfg(feature = "std")]
//! `depletion::coupled_operator::CoupledOperator` on a CORE-data pincell
//! (gh:#1626): the normalisation identities upstream's helpers define, the
//! zero-source short cut, and one Predictor step driven through it. Its
//! comparison against OpenMC is `tests/pincell_depletion_vs_openmc.rs`.

use petir::mathf::RealMath;
use outram_mc_libs::depletion::chain::DepletionChain;
use outram_mc_libs::depletion::coupled_operator::{ChainNuclideData, CoupledNormalization, CoupledOperator};
use outram_mc_libs::depletion::helpers::JOULE_PER_EV;
use outram_mc_libs::depletion::integrators::{CramSolver, DepletionIntegrator, Integrator, TransportOperator};
use outram_mc_libs::depletion::microxs::{NuclideReaction, ReactionTable};
use outram_mc_libs::prelude::*;
use std::sync::Arc;

fn log_grid(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    (0..=n)
        .map(|i| (lo.r_ln() + (hi.r_ln() - lo.r_ln()) * i as f64 / n as f64).r_exp())
        .collect()
}

fn pincell(r_fuel: f64, half: f64) -> Geometry {
    let surfaces = vec![
        SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: r_fuel,
            bc: BoundaryType::Transmissive,
        }),
        SurfaceKind::XPlane(XPlane {
            x0: -half,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::XPlane(XPlane {
            x0: half,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::YPlane(YPlane {
            y0: -half,
            bc: BoundaryType::Reflective,
        }),
        SurfaceKind::YPlane(YPlane {
            y0: half,
            bc: BoundaryType::Reflective,
        }),
    ];
    let fuel = Cell::material(
        1,
        vec![RegionToken::HalfSpace {
            surface_idx: 0,
            sense: HalfSpaceSense::Inside,
        }],
        0,
        293.6,
    );
    let moder = Cell::material(
        2,
        vec![
            RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::HalfSpace {
                surface_idx: 1,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 2,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 3,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 4,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
        ],
        1,
        293.6,
    );
    Geometry {
        surfaces,
        cells: vec![fuel, moder],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0, 1],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}


fn operator(normalization: CoupledNormalization) -> (CoupledOperator, Vec<f64>) {
    let nuclides = vec![
        Nuclide::from_core("U235").expect("U235 in CORE"),
        Nuclide::from_core("U238").expect("U238 in CORE"),
        Nuclide::from_core("H1").expect("H1 in CORE"),
    ];
    let (r_fuel, half) = (0.4_f64, 0.63_f64);
    let comp = |i: usize, d: f64| NuclideComponent { nuclide_idx: i, atom_density: d };
    let materials = vec![
        Material { id: 1, name: "fuel".into(), temperature: 293.6, components: vec![comp(0, 1.0e-3), comp(1, 2.2e-2)] },
        Material { id: 2, name: "moderator".into(), temperature: 293.6, components: vec![comp(2, 6.6e-2)] },
    ];
    let ks = KeffSettings { n_particles: 300, n_inactive: 5, n_active: 10, ..KeffSettings::default() };
    let src = SourceBox { lower: Position::new(-r_fuel, -r_fuel, -1.0), upper: Position::new(r_fuel, r_fuel, 1.0) };
    let grid = log_grid(1.0e-4, 3.0e7, 200);
    let constant = |b: f64| Some(NuclideReaction { low: ReactionTable { grid: grid.clone(), xs: vec![b; grid.len()], threshold: 0 }, high: None });
    let chain = DepletionChain::simple();
    let names: Vec<String> = chain.nuclide_names().iter().map(|s| (*s).to_owned()).collect();
    let data: Vec<ChainNuclideData> = names
        .iter()
        .map(|n| match n.as_str() {
            "U235" => ChainNuclideData { transport: Some(0), gamma: constant(10.0), fission: constant(50.0), n2n: None, fission_q: 2.0e8 },
            "U238" => ChainNuclideData { transport: Some(1), gamma: constant(1.0), fission: constant(0.1), n2n: None, fission_q: 2.0e8 },
            _ => ChainNuclideData::default(),
        })
        .collect();
    let volume = std::f64::consts::PI * r_fuel * r_fuel;
    let n0: Vec<f64> = names
        .iter()
        .map(|n| match n.as_str() {
            "U235" => 1.0e-3 * 1e24 * volume,
            "U238" => 2.2e-2 * 1e24 * volume,
            _ => 0.0,
        })
        .collect();
    let op = CoupledOperator::new(Arc::new(pincell(r_fuel, half)), Arc::new(nuclides), materials, src, ks, vec![0], vec![volume], chain, data, log_grid(1.0e-3, 2.0e7, 30), normalization).unwrap();
    (op, n0)
}

#[test]
fn coupled_operator_normalises_as_upstream() {
    // fission-q: the fission power of the returned rates is the source rate.
    let (mut op, n0) = operator(CoupledNormalization::FissionQ);
    let names = op.names().to_vec();
    let res = op.evaluate(&[n0.clone()], 174.0).unwrap();
    let rr = &res.rates.0[0];
    let power: f64 = names.iter().zip(&n0).map(|(n, a)| rr.rate_for(n).fission * a * 2.0e8 * JOULE_PER_EV).sum();
    println!("fission-q: power from rates {power} W, k {}", res.k.value);
    assert!(((power - 174.0) / 174.0).abs() < 1e-12, "{power}");
    // Constant cross sections: per-atom rates in the ratio of the barns.
    let (u5, u8) = (rr.rate_for("U235"), rr.rate_for("U238"));
    assert!((u5.gamma / u5.fission - 0.2).abs() < 1e-12);
    assert!((u8.fission / u5.fission - 0.002).abs() < 1e-12);
    assert!(res.k.value > 0.5 && res.k.std_dev > 0.0);

    // source-rate: rates scale linearly with the source rate.
    let (mut op1, _) = operator(CoupledNormalization::SourceRate);
    let (mut op2, _) = operator(CoupledNormalization::SourceRate);
    let a = op1.evaluate(&[n0.clone()], 1.0).unwrap().rates.0[0].rate_for("U235").fission;
    let b = op2.evaluate(&[n0.clone()], 3.0).unwrap().rates.0[0].rate_for("U235").fission;
    assert!((b / a - 3.0).abs() < 1e-12, "{a} {b}");

    // Zero source: zero rates, no transport.
    let calls = op.calls;
    let z = op.evaluate(&[n0.clone()], 0.0).unwrap();
    assert_eq!(op.calls, calls);
    assert_eq!(z.rates.0[0].rate_for("U235").fission, 0.0);
    assert!(op.evaluate(&[n0.clone(), n0.clone()], 1.0).is_err());
    assert!(op.form_matrix(&z.rates, 1).is_err());
}

#[test]
fn predictor_step_through_the_coupled_operator() {
    let (mut op, n0) = operator(CoupledNormalization::FissionQ);
    let names = op.names().to_vec();
    let i5 = names.iter().position(|n| n == "U235").unwrap();
    let mut integ = DepletionIntegrator::new(Integrator::Predictor, CramSolver::Cram48, vec![86_400.0 * 10.0], vec![174.0]).unwrap();
    let records = integ.integrate(&mut op, vec![n0.clone()], true).unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(op.calls, 2);
    let burnt = n0[i5] - records[1].n[0][i5];
    // chain_simple's U-235 lists fission only, so capture removes nothing
    // (upstream form_matrix takes losses from the chain's reactions). U-235's
    // share of the fissions is 50e-3 / (50e-3 + 0.1 * 22e-3) = 0.958; over
    // 10 days the U-235 burn-up lowers it slightly.
    let fissions = 174.0 * 864_000.0 / (2.0e8 * JOULE_PER_EV);
    println!("U235 burnt {burnt:e}, fissions {fissions:e}");
    assert!(burnt > 0.94 * fissions && burnt < 0.96 * fissions, "{burnt} {fissions}");
}

/// `reaction_from_ace` builds U-234's MT=18 the way OpenMC's
/// `IncidentNeutron.from_ace` does: NJOY2016's table carries only the partial
/// fissions, its photon block names MT=18, so MT=18 is their sum from the
/// lowest partial threshold (`neutron.py:612-632`, `:877-906`). Checked
/// bit-for-bit against libopenmc's `collapse_rate` in
/// `tests/collapse_rate_vs_libopenmc.rs`; here on the repository's own
/// NJOY2016 table (skips if the `reference-data/ace` submodule is absent).
#[test]
fn redundant_fission_is_the_sum_of_the_partials() {
    use njoy_outram_park_fork::acer::ce_decode::decode_ce;
    use outram_mc_libs::depletion::coupled_operator::reaction_from_ace;
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/ace/reference-njoy/endf-b-viii.0/293.6K/U234.ace.gz");
    if !p.is_file() {
        println!("SKIP: {} absent (submodule not fetched)", p.display());
        return;
    }
    let raw = njoy_outram_park_fork::acer::read::read(&p).unwrap();
    let ace = decode_ce(&raw).unwrap();
    assert!(ace.reactions.iter().all(|r| r.mt != 18), "U-234 should carry partial fissions only");
    let rx = reaction_from_ace(&raw, &ace, 18).expect("redundant MT=18");
    let partials: Vec<_> = ace.reactions.iter().filter(|r| [19, 20, 21, 38].contains(&r.mt)).collect();
    assert!(!partials.is_empty());
    let thr = partials.iter().map(|r| r.threshold_index).min().unwrap();
    assert_eq!(rx.low.threshold, thr);
    for (k, v) in rx.low.xs.iter().enumerate() {
        let i = thr + k;
        let mut sum = 0.0;
        for r in &partials {
            if i >= r.threshold_index {
                sum += r.xs[i - r.threshold_index];
            }
        }
        assert_eq!(v.to_bits(), sum.to_bits(), "grid point {i}");
    }
    // A plain reaction is copied; an MT neither present nor redundant is None.
    let g = reaction_from_ace(&raw, &ace, 102).unwrap();
    assert_eq!(g.low.xs, ace.reactions.iter().find(|r| r.mt == 102).unwrap().xs);
    assert!(reaction_from_ace(&raw, &ace, 999).is_none());
}
