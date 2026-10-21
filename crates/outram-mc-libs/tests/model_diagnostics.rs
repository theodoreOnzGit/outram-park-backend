//! **The beginner `Diagnostics`, the report zip and `advanced`** (gh #940,
//! #941, #942).
//!
//! # Methodology
//!
//! Godiva (ICSBEP HEU-MET-FAST-001, `vv::godiva`: one bare sphere of the
//! spec's three uranium nuclides) built through `model::Model`, run with
//! `MCSettings::Custom` on the embedded LOW-tier data (`Nuclide::from_core`,
//! so it runs anywhere, in seconds). What is checked, and why it can fail:
//!
//! 1. **Diagnostics change nothing and are independent.** `k` per cycle with
//!    `Diagnostics::reactor_physics()` (+ a per-component spectrum) is the
//!    `k` with `None`, bit for bit; and every diagnostic requested **alone**
//!    gives the same result, bit for bit, as when requested beside all the
//!    others (the #940 acceptance; run health is compared without its wall
//!    time).
//! 2. **Internal consistency, each a check that could fail:**
//!    - the six-factor product times `(A + L)` is `P`, and `P` (the
//!      track-length k) agrees with the power-iteration k within 4σ;
//!    - the whole-model spectrum, summed over all its bins (in and out of
//!      the grid), equals the neutron balance's total flux to 1e-12
//!      (two tallies, the same track lengths);
//!    - the normalised lethargy spectrum integrates to 1;
//!    - the balance closes: `(absorption + leakage)/ends − 1` is 0 within 4σ
//!      (track-length absorption against the analog count of ends);
//!    - the three-group flux map sums to its total, and the six-factor
//!      leakage sums to the leakage diagnostic's.
//! 3. **The report**: the zip reads back with every CRC checked, holds
//!    `report.md`, the CSVs and PNGs that decode; a result not requested is
//!    an `Err` naming the variant.
//! 4. **`advanced`** (#942, `long-tests`, ENDF/B-VIII.0): an ablated Godiva
//!    run lists its ablations in the provenance, and its nuclides show URR
//!    and DBRC off, while the ordinary run of the same model has them on.
//!
//! # Results (2026-10-17, i9-13900K)
//!
//! All three pass (69 s, the ENDF processing of test 4 dominating). Test 1 on
//! the LOW tier, 2 000 × [10 + 40], seed 3: `k = 1.01416 ± 0.00504`,
//! `P = 1.01342 ± 0.00312` (−0.1σ from k), `P/(A+L) = 1.01356 ± 0.00251`;
//! spectrum and balance flux both 6.760446777 cm per source neutron; closure
//! −1.4e-4 ± 2.4e-3; EALF 6.64e5 eV; Λ 6.27 ns; β_eff exactly 0 (the LOW-tier
//! nuclides sample no delayed neutron); six factors undefined (no thermal
//! absorption), as they must be for Godiva; the zip holds 31 files. These are
//! not benchmark numbers (LOW tier, tiny run). The ENDF/B-VIII.0 run and the
//! OpenMC comparison are in `verification_and_validation/diagnostics/README.md`.

use petir::mathf::RealMath;
use njoy_outram_park_fork::reference_data::reference_endf_dir;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::model::run::RunError;
use outram_mc_libs::vv::godiva;
use uom::si::f64::{Length, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::thermodynamic_temperature::kelvin;

fn header_only_library() -> NuclearData {
    let tape = |za: &str, awr: &str| {
        format!(
            " TPID\n{za:>11}{awr:>11}{:>11}{:>11}{:>11}{:>11}\n{:>11}{:>11}{:>11}{:>11}{:>11}{:>11}\n{:>11}{:>11}{:>11}{:>11}{:>11}{:>11}\n",
            0, 0, 0, 0, "0.0", "0.0", 0, 0, 0, 6, "1.0", "2.0+7", 0, 0, 10, 8
        )
        .into_bytes()
    };
    NuclearData::from_endf_bytes(
        "headers only",
        [
            ("u234".to_string(), tape("9.223400+4", "2.320304+2")),
            ("u235".to_string(), tape("9.223500+4", "2.330248+2")),
            ("u238".to_string(), tape("9.223800+4", "2.360058+2")),
        ],
    )
    .expect("three incident-neutron headers")
}

fn godiva_model(data: NuclearData) -> (Model, Component) {
    let mut m = Model::new(data);
    let core = m.component("Core");
    let heu = Material::from_atom_densities(
        godiva::NUCLIDES
            .iter()
            .map(|(n, _, d)| (Isotope::from_name(n).unwrap(), atoms_per_barn_cm(*d))),
    )
    .named("Godiva HEU");
    m.add(
        Sphere::radius(Length::new::<centimeter>(godiva::RADIUS_CM))
            .filled_with(heu)
            .in_component(core),
    );
    m.set_temperature(Temperature::Uniform(
        ThermodynamicTemperature::new::<kelvin>(godiva::TEMPERATURE_K),
    ));
    (m, core)
}

fn low_tier() -> Vec<Nuclide> {
    ["U234", "U235", "U238"]
        .iter()
        .map(|n| Nuclide::from_core(n).expect("CORE nuclide"))
        .collect()
}

fn custom() -> MCSettings {
    MCSettings::Custom(CustomRun {
        particles: 2_000,
        inactive: 10,
        active: 40,
        seed: 3,
    })
}

/// Run health without its wall time (which differs run to run).
fn timeless(r: &MCDiagnosticResult) -> MCDiagnosticResult {
    match r {
        MCDiagnosticResult::RunHealth(h) => {
            let mut h = h.clone();
            h.transport_seconds = None;
            MCDiagnosticResult::RunHealth(h)
        }
        other => other.clone(),
    }
}

fn within(a: f64, sa: f64, b: f64, sb: f64, z: f64) -> bool {
    (a - b).abs() <= z * (sa * sa + sb * sb).sqrt()
}

#[test]
fn diagnostics_change_nothing_are_independent_and_consistent() {
    let (model, core) = godiva_model(header_only_library());
    let a = model.assemble().unwrap();
    let nuc = low_tier();
    let plain = a
        .keff_with(&nuc, &KeffSettings::new(custom(), None))
        .unwrap();
    let all = Diagnostics::reactor_physics()
        .with_spectrum_per_component(&model, EnergyGrid::default_spectrum());
    let run = a
        .keff_with(&nuc, &KeffSettings::new(custom(), Some(all.clone())))
        .unwrap();
    assert_eq!(
        plain.k_by_cycle, run.k_by_cycle,
        "diagnostics must not change k"
    );
    assert_eq!(run.diagnostics.len(), all.items().len());

    // 1. Each diagnostic alone = beside the others, bit for bit.
    for (d, together) in &run.diagnostics {
        let alone = a
            .keff_with(
                &nuc,
                &KeffSettings::new(custom(), Some(Diagnostics::new().with(d.clone()))),
            )
            .unwrap();
        assert_eq!(alone.k_by_cycle, plain.k_by_cycle);
        // Debug form, so a NaN (an undefined factor) compares equal to itself.
        assert_eq!(
            format!("{:?}", timeless(&alone.diagnostics[0].1)),
            format!("{:?}", timeless(together)),
            "{} differs alone and together",
            d.name()
        );
    }

    // 2. Consistency.
    let k = run.keff();
    let sf = run.six_factors().unwrap();
    let a_l: f64 = sf
        .absorption
        .iter()
        .chain(&sf.leakage)
        .map(|e| e.mean)
        .sum();
    assert!((sf.production_over_losses.mean * a_l - sf.k_tracklength.mean).abs() < 1e-12);
    // Godiva is fast: no thermal absorption, so the factors are undefined
    // (NaN with the reason), never a zero that reads as a result.
    assert!(sf
        .undefined_because
        .as_deref()
        .is_some_and(|w| w.contains("thermal")));
    assert!(sf.eta.mean.is_nan() && sf.k_from_factors.mean.is_nan());
    println!(
        "k = {k}; P (track length) = {:.5} ± {:.5}; P/(A+L) = {:.5} ± {:.5}; A + L = {a_l:.5}",
        sf.k_tracklength.mean,
        sf.k_tracklength.std,
        sf.production_over_losses.mean,
        sf.production_over_losses.std
    );
    println!(
        "eta {:.4} f {:.4} p {:.4} eps {:.4} P_FNL {:.4} P_TNL {:.4}",
        sf.eta.mean, sf.f.mean, sf.p.mean, sf.epsilon.mean, sf.p_fnl.mean, sf.p_tnl.mean
    );
    assert!(within(
        sf.k_tracklength.mean,
        sf.k_tracklength.std,
        k.mean,
        k.std,
        4.0
    ));

    let spec = run.spectrum(Region::WholeModel).unwrap();
    let all_bins = spec.total_in_grid.mean + spec.outside_grid.0 + spec.outside_grid.1;
    let bal = run.neutron_balance().unwrap();
    println!(
        "spectrum flux {all_bins:.9} vs balance flux {:.9} cm per source neutron",
        bal.total.flux.mean
    );
    assert!((all_bins - bal.total.flux.mean).abs() < 1e-12 * bal.total.flux.mean);
    let integral: f64 = spec
        .per_lethargy
        .iter()
        .zip(spec.edges_ev.windows(2))
        .map(|(e, w)| e.mean * (w[1] / w[0]).r_ln())
        .sum();
    assert!((integral - 1.0).abs() < 1e-12);
    println!(
        "balance closure {:+.2e} ± {:.2e}; ends per source {:.5}",
        bal.closure.mean, bal.closure.std, bal.ends_per_source.mean
    );
    assert!(bal.closure.mean.abs() <= 4.0 * bal.closure.std);
    assert_eq!(bal.rows.len(), 1);
    let core_spec = run.spectrum(Region::Component(core)).unwrap();
    assert_eq!(core_spec.label, "Core");
    assert_eq!(
        core_spec.flux, spec.flux,
        "one cell: the core is the whole model"
    );

    let leak = run.leakage().unwrap();
    let l3: f64 = sf.leakage.iter().map(|e| e.mean).sum();
    assert!((l3 - leak.per_source.mean).abs() < 1e-12);
    let ealf = run.ealf().unwrap();
    println!(
        "EALF {:.4e} ± {:.1e} eV",
        ealf.ealf_ev.mean, ealf.ealf_ev.std
    );
    assert!(
        ealf.ealf_ev.mean > 1.0e5 && ealf.ealf_ev.mean < 2.0e6,
        "a fast system"
    );

    let fm = run.flux_map().unwrap();
    assert_eq!(fm.mesh.dimension, [15, 15, 15]);
    assert_eq!(fm.values.len(), 4);
    let c = 7 + 15 * (7 + 15 * 7);
    let sum3: f64 = fm.values[..3].iter().map(|v| v[c].mean).sum();
    assert!((sum3 - fm.values[3][c].mean).abs() < 1e-12 * sum3);
    assert!(run.fission_map().unwrap().values[0][c].mean > 0.0);
    match run.kinetics() {
        Ok(kin) => println!(
            "beta_eff {:.5} ± {:.5}, Lambda {:.3e} ± {:.1e} s ({} scoring cycles)",
            kin.beta_eff.mean,
            kin.beta_eff.std,
            kin.generation_time_s.mean,
            kin.generation_time_s.std,
            kin.scoring_cycles
        ),
        Err(e) => panic!("{e}"),
    }
    let h = run.run_health().unwrap();
    assert!(h.clean());

    // 3. The report.
    let report = run.diagnostics_report_titled("Godiva");
    let zip = report.to_zip_bytes().unwrap();
    let back = DiagnosticsReport::read_zip(&zip).unwrap();
    assert_eq!(back, report.files());
    let md = String::from_utf8(report.file("report.md").unwrap().bytes.clone()).unwrap();
    assert!(md.starts_with("# Godiva"));
    assert!(md.contains("```csv"));
    for f in report.files() {
        if f.path.ends_with(".png") {
            assert!(
                md.contains(&format!("]({})", f.path)),
                "{} is embedded",
                f.path
            );
            outram_mc_libs::geometry::plot::decode_png(&f.bytes).expect("a PNG");
        }
    }
    assert!(report.file("data/run_summary.csv").is_some());
    // The native writers: the zip on disk is the in-memory one, and the
    // unzipped folder holds the same files.
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("model_diagnostics");
    report.write_zip(dir.join("nested/godiva.zip")).unwrap();
    assert_eq!(std::fs::read(dir.join("nested/godiva.zip")).unwrap(), zip);
    report.write_unzipped_to_dir(dir.join("unzipped")).unwrap();
    assert_eq!(
        std::fs::read(dir.join("unzipped/report.md")).unwrap(),
        report.file("report.md").unwrap().bytes
    );
    // The uom accessors.
    use uom::si::{energy::electronvolt, length::centimeter as cm, time::second};
    assert_eq!(ealf.energy().get::<electronvolt>(), ealf.ealf_ev.mean);
    assert!((fm.cell_size()[0].get::<cm>() - 2.0 * godiva::RADIUS_CM / 15.0).abs() < 1e-12);
    let kin = run.kinetics().unwrap();
    assert_eq!(
        kin.generation_time().get::<second>(),
        kin.generation_time_s.mean
    );
    println!("zip: {} bytes, {} files", zip.len(), back.len());

    // Not requested is an error naming the variant.
    match plain.six_factors() {
        Err(DiagnosticError::NotRequested { variant, .. }) => assert_eq!(variant, "SixFactors"),
        other => panic!("{other:?}"),
    }
    assert!(plain.spectrum(Region::WholeModel).is_err());
    assert!(run
        .six_factors_with_edges(
            uom::si::f64::Energy::new::<uom::si::energy::electronvolt>(1.0),
            uom::si::f64::Energy::new::<uom::si::energy::electronvolt>(10.0)
        )
        .is_err());
    for e in [
        plain.run_health().err(),
        plain.ealf().err(),
        plain.leakage().err(),
        plain.kinetics().err(),
        plain.fission_map().err(),
        plain.flux_map().err(),
        plain.neutron_balance().err(),
    ] {
        assert!(e.is_some());
    }
    assert!(plain.diagnostics_report().file("report.md").is_some());
}

/// A component of another model, or one no shape is in, is refused before
/// any transport.
#[test]
fn a_foreign_component_is_refused() {
    let (model, _) = godiva_model(header_only_library());
    let mut other = Model::new(header_only_library());
    let foreign = other.component("elsewhere");
    let a = model.assemble().unwrap();
    let d = Diagnostics::new().with(MCDiagnostic::spectrum(
        Region::Component(foreign),
        EnergyGrid::OneGroup,
    ));
    match a.keff_with(&low_tier(), &KeffSettings::new(custom(), Some(d))) {
        Err(RunError::Diagnostic(DiagnosticError::Component(m))) => assert!(m.contains("no shape")),
        other => panic!("{other:?}"),
    }
    let two = Diagnostics::new()
        .with(MCDiagnostic::kinetics(3).unwrap())
        .with(MCDiagnostic::kinetics(4).unwrap());
    assert!(a
        .keff_with(&low_tier(), &KeffSettings::new(custom(), Some(two)))
        .is_err());
    // IFP that never reaches its depth has no value rather than a zero.
    let deep = Diagnostics::new().with(MCDiagnostic::kinetics(500).unwrap());
    assert!(matches!(
        a.keff_with(&low_tier(), &KeffSettings::new(custom(), Some(deep))),
        Err(RunError::Diagnostic(DiagnosticError::NoValue(_)))
    ));
    // A map on your own mesh, through `advanced`.
    let mine = Diagnostics::new().with(advanced::flux_map(
        advanced::mesh_divisions([4, 4, 2], MeshBounds::FitModel),
        EnergyGrid::OneGroup,
    ));
    let run = a
        .keff_with(&low_tier(), &KeffSettings::new(custom(), Some(mine)))
        .unwrap();
    let m = run.flux_map().unwrap();
    assert_eq!((m.mesh.dimension, m.values.len()), ([4, 4, 2], 1));
}

/// **#942 acceptance**: Godiva through the high-level API carries DBRC and
/// URR without being asked; the same model through `advanced()` with both
/// ablated runs without them, and its provenance names the ablations.
#[test]
#[cfg_attr(
    not(feature = "long-tests"),
    ignore = "processes U-234/235/238 from ENDF twice (~2 min); runs by default, skipped under --no-default-features"
)]
fn an_ablated_run_says_so_and_the_ordinary_one_has_the_physics() {
    if !reference_endf_dir()
        .join("n-092_U_235-ENDF8.0.endf")
        .exists()
    {
        eprintln!("SKIP: reference tapes not present");
        return;
    }
    let tiny = MCSettings::Custom(CustomRun {
        particles: 200,
        inactive: 2,
        active: 3,
        seed: 1,
    });
    let (m, _) = godiva_model(NuclearData::workspace_reference().unwrap());
    let ordinary = m.keff(KeffSettings::new(tiny, None)).unwrap();
    assert!(ordinary.provenance.ablations.is_empty());
    assert!(ordinary.provenance.physics.iter().all(|n| n.dbrc.is_ok()));
    let ablated = m
        .advanced()
        .without_dbrc()
        .without_urr()
        .keff(KeffSettings::new(tiny, None))
        .unwrap();
    println!("{ablated}");
    assert_eq!(ablated.provenance.ablations.len(), 2);
    assert!(ablated.provenance.ablations[0].contains("DBRC off"));
    assert!(ablated.provenance.physics.iter().all(|n| n.dbrc.is_err()));
    assert!(ablated
        .provenance
        .physics
        .iter()
        .all(|n| !n.urr_probability_tables));
    assert!(ablated.to_string().contains("ABLATED"));
}
