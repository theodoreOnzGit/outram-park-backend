//! Tests for the simple depletion API (gh:#1629), on the thorium cycle.
//! Predictions are in each doc comment; measured values are printed and
//! recorded.

use super::*;
use crate::depletion::external_rates::{RateUnit, TransferRates};
use crate::prelude::*;
use njoy_outram_park_fork::hdf5::depletion_chain_from_endf::from_endf;
use petir::mathf::RealMath;
use std::path::PathBuf;

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


fn data() -> Option<(PathBuf, Vec<Tape>, Vec<Tape>)> {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/ace/endf/endf-b-viii.0");
    if !d.join("decay").is_dir() {
        std::println!("SKIP: reference-data/ace sublibraries not checked out");
        return None;
    }
    let read = |sub: &str| -> Vec<Tape> {
        let mut ps: Vec<PathBuf> = std::fs::read_dir(d.join(sub)).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|x| x == "endf")).collect();
        ps.sort();
        ps.iter().map(|p| Tape::from_bytes(&std::fs::read(p).unwrap()).unwrap()).collect()
    };
    Some((d.clone(), read("decay"), read("nfy")))
}

/// Thorium breeding through the simple API, and the Pa-233 removal pair.
///
/// Chain: `from_endf` on the full decay and NFY sublibraries with the
/// Th-232, Pa-233 and U-233 evaluations, reduced from Th-232 and Xe-135 to
/// level 3 (Th-232 → Th-233 → Pa-233 → U-233). Spectrum: one transport run
/// on a CORE pin cell (U-235, U-238, H-1; the thorium nuclides are not in
/// CORE, so the spectrum is a stand-in LWR pin). One-group data: collapsed
/// from RECONR on the three evaluations. Fuel starts at Th-232 2e-2 and
/// U-233 1e-3 atoms/(b·cm); 3 kW in 1 cm³ (flux ~1e15) for 10 × 3 d.
///
/// Predictions, written before the run:
/// 1. Th-233, Pa-233 and U-233 are produced from Th-232 (U-233 above what
///    the seed alone would leave);
/// 2. every chain nuclide without an evaluation is named in `no_data`;
/// 3. Pa removal into an out-of-flux tank at 1/d lowers core Pa-233 and
///    puts U-233 in the tank;
/// 4. total U-233 (core + tank) is HIGHER with removal: Pa-233 then decays
///    instead of capturing (its capture is a few percent of its decay at
///    this flux).
///
/// Measured 2026-11-17, all four held: chain 1175 nuclides (3 with
/// evaluations, 1172 decay-only, all named); collapsed on the pin's fuel
/// spectrum, Th-232 σ_c 3.019 b, Pa-233 σ_c 28.26 b, U-233 σ_f 70.04 b and
/// σ_c 7.786 b; flux 1.386e15 n/cm²/s at step 1. End of life without
/// removal: Th-233 1.97e-7, Pa-233 1.623e-4, U-233 7.924e-4 (above the
/// seed-only bound). With Pa removed to the tank: core Pa-233 9.11e-6,
/// tank U-233 6.53e-5, total U-233 7.9638e-4 against 7.9241e-4,
/// **+0.50 %**. The spectrum is a stand-in (CORE U pin), so the numbers are
/// a demonstration of the API and the sign, not an LFTR result.
#[test]
fn thorium_breeding_and_pa_removal() {
    let Some((d, decay, nfy)) = data() else { return };
    let tape = |f: &str| Tape::from_bytes(&std::fs::read(d.join(f)).unwrap()).unwrap();
    let (th, pa, u3) = (tape("n-090_Th_232-ENDF8.0.endf"), tape("n-091_Pa_233-ENDF8.0.endf"), tape("n-092_U_233-ENDF8.0.endf"));
    let (chain_xml, _) = from_endf(&decay, &nfy, &[th.clone(), pa.clone(), u3.clone()], None).unwrap();

    let nuclides = vec![Nuclide::from_core("U235").unwrap(), Nuclide::from_core("U238").unwrap(), Nuclide::from_core("H1").unwrap()];
    let (r_fuel, half) = (0.4_f64, 0.63_f64);
    let geom = pincell(r_fuel, half);
    let comp = |i: usize, x: f64| NuclideComponent { nuclide_idx: i, atom_density: x };
    let materials = vec![
        Material { id: 1, name: "fuel".into(), temperature: 293.6, components: vec![comp(0, 1.0e-3), comp(1, 2.2e-2)] },
        Material { id: 2, name: "moderator".into(), temperature: 293.6, components: vec![comp(2, 6.6e-2)] },
    ];
    let ks = KeffSettings { n_particles: 400, n_inactive: 8, n_active: 20, ..KeffSettings::default() };
    let src = SourceBox { lower: Position::new(-r_fuel, -r_fuel, -1.0), upper: Position::new(r_fuel, r_fuel, 1.0) };

    let starting = vec![("Th232".to_string(), 2.0e-2), ("U233".to_string(), 1.0e-3)];
    let study = SpectrumDepletion::new(0, starting, chain_xml)
        .with_reduce_from(&["Th232", "Xe135"])
        .with_reduce_level(Some(3))
        .with_evaluation("Th232", th)
        .with_evaluation("Pa233", pa)
        .with_evaluation("U233", u3)
        .with_group_edges(log_grid(1.0e-5, 2.0e7, 40))
        .with_power(3000.0, 1.0)
        .with_steps(vec![3.0 * 86_400.0; 10]);
    let plain = study.run(&geom, &materials, &nuclides, src, &ks).unwrap();
    let last = plain.history.len() - 1;
    let og = |n: &str| plain.one_group.iter().find(|(x, _)| x == n).map(|(_, x)| *x).unwrap();
    std::println!("chain {} nuclides; no data for {}; dropped {:?}", plain.chain.nuclide_names().len(), plain.no_data.len(), plain.dropped);
    for n in ["Th232", "Pa233", "U233"] {
        std::println!("{n}: sigma_c {:.4} b, sigma_f {:.4} b", og(n).gamma, og(n).fission);
    }
    std::println!("flux step 1 {:.3e}", plain.history[1].1);
    for n in ["Th233", "Pa233", "U233"] {
        std::println!("EOL {n} {:.4e}", plain.density(last, 0, n));
    }
    assert!(plain.dropped.is_empty());
    assert!(plain.density(last, 0, "Th233") > 0.0 && plain.density(last, 0, "Pa233") > 0.0);
    // 1: U-233 above the seed burnt alone (fission only, same flux history).
    let burn: f64 = plain.history.iter().skip(1).map(|h| h.1 * og("U233").absorption * 1e-24 * 3.0 * 86_400.0).sum();
    let seed_only = 1.0e-3 * (-burn).r_exp();
    assert!(plain.density(last, 0, "U233") > seed_only, "{} vs {seed_only}", plain.density(last, 0, "U233"));
    // 2
    let names = plain.chain.nuclide_names();
    assert!(plain.no_data.iter().all(|n| !["Th232", "Pa233", "U233"].contains(&n.as_str())));
    assert_eq!(plain.no_data.len() + 3, names.len());

    // 3 and 4: Pa removed to an out-of-flux tank.
    let chain_names: Vec<String> = names.iter().map(|s| (*s).to_owned()).collect();
    let mut tr = TransferRates::new(vec!["core".into(), "tank".into()], chain_names, 10);
    tr.set_transfer_rate("core", &["Pa"], 1.0, RateUnit::PerDay, None, Some("tank")).unwrap();
    let removed = study.clone().with_transfer_rates(tr).run(&geom, &materials, &nuclides, src, &ks).unwrap();
    let total = |o: &DepletionOutcome, n: &str| o.density(last, 0, n) + o.history[last].2.get(1).map_or(0.0, |_| o.density(last, 1, n));
    std::println!(
        "Pa233 core: removal {:.4e}, none {:.4e}; U233 tank {:.4e}; U233 total: removal {:.6e}, none {:.6e} ({:+.3} %)",
        removed.density(last, 0, "Pa233"),
        plain.density(last, 0, "Pa233"),
        removed.density(last, 1, "U233"),
        total(&removed, "U233"),
        total(&plain, "U233"),
        100.0 * (total(&removed, "U233") / total(&plain, "U233") - 1.0)
    );
    assert!(removed.density(last, 0, "Pa233") < plain.density(last, 0, "Pa233"));
    assert!(removed.density(last, 1, "U233") > 0.0);
    assert!(total(&removed, "U233") > total(&plain, "U233"));

    // gh:#1628 default: these evaluations carry no MF=8/9/10, so every
    // evaluated (n,gamma) is recorded as NoData and the preset ablation is
    // bit-identical.
    for n in ["Th232", "Pa233", "U233"] {
        assert!(plain.branching.iter().any(|b| b.parent == n && b.reaction == "(n,gamma)" && b.outcome == BranchingOutcome::NoData), "{n}: {:?}", plain.branching);
    }
    let preset = study.clone().with_preset_branch_ratios().run(&geom, &materials, &nuclides, src, &ks).unwrap();
    assert!(preset.branching.is_empty());
    assert_eq!(preset.history[last].2, plain.history[last].2);
}

/// gh:#1628 as the default: Zn-68 (n,γ) has MF=9 multiplicities to Zn-69
/// and Zn-69m. Chain: `from_endf` with the Zn-68 and U-233 evaluations
/// (U-233 to carry the power), reduced from Zn-68 and U-233 to level 1.
/// Predictions: by default the chain's Zn-68 capture splits between Zn69
/// and Zn69_m1, the isomer share between the thermal 0.066 and the fast
/// 0.244 (closer to 0.066 in this thermal pin), and Zn-69m is produced;
/// with the preset ablation the split is not applied and Zn-69m is not
/// produced by capture (0 at end of life, as nothing else feeds it in this
/// chain).
///
/// Measured 2026-11-17: Zn-69m share **0.068839** (prediction held). The
/// "0 under presets" prediction was **wrong**: U-233's fission yields feed
/// Zn-69m (it is a level-1 fission product), giving 2.5802e-15 at end of
/// life under presets, against **1.2005e-9** by default, so capture
/// branching is ~465× the fission feed here. The gate is therefore
/// "default above preset", not "preset zero".
///
/// Found on the way: `DepletionChain::from_chain_xml` leaves a branched
/// channel out of the matrix entirely, so a spectrum-derived split removed
/// Zn-68's capture altogether (default and preset first came out equal).
/// `from_chain_xml_with_branches` now carries branched (n,γ)/(n,2n) at
/// their ratios; `from_chain_xml` is unchanged.
#[test]
fn zinc68_isomer_branching_is_the_default() {
    let Some((d, decay, nfy)) = data() else { return };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");
    let zn = Tape::from_bytes(&std::fs::read(root.join("n-030_Zn_068-ENDF8.0.endf")).unwrap()).unwrap();
    let u3 = Tape::from_bytes(&std::fs::read(d.join("n-092_U_233-ENDF8.0.endf")).unwrap()).unwrap();
    let (chain_xml, _) = from_endf(&decay, &nfy, &[zn.clone(), u3.clone()], None).unwrap();
    let nuclides = vec![Nuclide::from_core("U235").unwrap(), Nuclide::from_core("U238").unwrap(), Nuclide::from_core("H1").unwrap()];
    let (r_fuel, half) = (0.4_f64, 0.63_f64);
    let geom = pincell(r_fuel, half);
    let comp = |i: usize, x: f64| NuclideComponent { nuclide_idx: i, atom_density: x };
    let materials = vec![
        Material { id: 1, name: "fuel".into(), temperature: 293.6, components: vec![comp(0, 1.0e-3), comp(1, 2.2e-2)] },
        Material { id: 2, name: "moderator".into(), temperature: 293.6, components: vec![comp(2, 6.6e-2)] },
    ];
    let ks = KeffSettings { n_particles: 400, n_inactive: 8, n_active: 20, ..KeffSettings::default() };
    let src = SourceBox { lower: Position::new(-r_fuel, -r_fuel, -1.0), upper: Position::new(r_fuel, r_fuel, 1.0) };
    let study = SpectrumDepletion::new(0, vec![("Zn68".into(), 1.0e-3), ("U233".into(), 1.0e-3)], chain_xml)
        .with_reduce_level(Some(1))
        .with_evaluation("Zn68", zn)
        .with_evaluation("U233", u3)
        .with_group_edges(log_grid(1.0e-5, 2.0e7, 40))
        .with_power(3000.0, 1.0)
        .with_steps(vec![86_400.0; 3]);
    let def = study.run(&geom, &materials, &nuclides, src, &ks).unwrap();
    let rec = def.branching.iter().find(|b| b.parent == "Zn68" && b.reaction == "(n,gamma)").unwrap();
    std::println!("Zn68 (n,gamma) branching: {:?}", rec.outcome);
    let BranchingOutcome::Applied { set, .. } = &rec.outcome else { panic!("{rec:?}") };
    let m1 = set.iter().find(|(p, _)| p == "Zn69_m1").map(|(_, b)| *b).unwrap();
    assert!(m1 > 0.066 && m1 < 0.244, "{m1}");
    let last = def.history.len() - 1;
    let pre = study.clone().with_preset_branch_ratios().run(&geom, &materials, &nuclides, src, &ks).unwrap();
    std::println!("EOL Zn69_m1: default {:.4e}, preset {:.4e}", def.density(last, 0, "Zn69_m1"), pre.density(last, 0, "Zn69_m1"));
    assert!(def.density(last, 0, "Zn69_m1") > 0.0);
    assert!(def.density(last, 0, "Zn69_m1") > 100.0 * pre.density(last, 0, "Zn69_m1"));
}
