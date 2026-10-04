// SPDX-License-Identifier: GPL-3.0

//! **LEU-COMP-THERM-008's pin in an infinite square lattice: k∞ and the six
//! factors against pitch** — under- and over-moderation, for tutorial rung 4
//! (GitHub #526).
//!
//! # What it computes
//!
//! One LCT-008 fuel rod — UO₂ (2.459 w/o) to `r = 0.514858 cm`, Al-6061 clad to
//! `0.602996 cm`, case-1 borated water outside — centred in a square cell whose
//! four side planes (and two z-planes) are **reflective**. A reflective square
//! cell is an infinite lattice of identical pins, so the eigenvalue is k∞: no
//! leakage, only the competition between moderation and absorption. The pitch
//! is the one free variable; for each pitch the run reports k∞ and the
//! three-group six-factor decomposition of `physics::reactor_physics`
//! (`η`, `f`, `p`, `ε`, with `P_FNL = P_TNL = 1` up to tallied leakage, which
//! must be zero here).
//!
//! # Where the inputs come from (reused, not retyped)
//!
//! - **Materials:** the case-1 `materials.xml` of the committed
//!   `mit-crpg/benchmarks` OpenMC model, parsed by
//!   `common/lct008_model.rs::parse_materials` / `build_materials` — the same
//!   code the LCT-008 lattice examples run. Water keeps its 1511 ppm soluble
//!   boron unless `--no-soluble-boron` is given (an explicit ablation that drops
//!   B-10 and B-11 from the **water** only; the fuel's B-10 impurity stays).
//! - **Radii:** `lct008_model::R_FUEL`, `R_CLAD`. No gap: LCT-008's pin has none
//!   (`PIN_SHELLS`, universe 2).
//! - **Nuclear data:** ENDF/B-VIII.0 tapes in `reference-data/endf/`, RECONR +
//!   BROADR at 293.6 K (`lct008_model::TEMP_K`, the tabulated temperature of
//!   the `H(H2O)` law) through `Nuclide::from_endf_file_with_speed`, which
//!   applies URR probability tables and DBRC **by default**. H-1 in the water
//!   carries the ENDF/B-VIII.0 `H in H2O` S(α,β) law (`tsl-HinH2O.endf`).
//! - **Nuclide tier:** every nuclide the model names (`lct008_model::TAPES`, 36)
//!   by default; `--cheap-nuclides` selects the 11-nuclide `TAPES_CHEAP` tier
//!   that the five-route study and the OpenMC NJOY2016 library use, for the
//!   code-to-code points. Nuclides outside the tier are dropped, not
//!   renormalised, exactly as in `lct008_keff.rs`.
//!
//! The loader below mirrors `lct008_keff.rs::load_nuclides` (no ACE route, no
//! resonance-flag rebuild); that file is the authority for the LCT-008 data path.
//!
//! # Methodology and results
//!
//! The prediction (written before any run), the settings, the measured table,
//! timings with hardware, and the OpenMC comparison are in
//! `verification_and_validation/tutorial_rung4/pitch_sweep.md`. The OpenMC
//! deck is committed in `verification_and_validation/tutorial_rung4/openmc_inputs/`.
//! This is **verification** (code-to-code); there is no measured reference for
//! an infinite lattice of this pin at other pitches.
//!
//! **Results (2026-10-04, seed 1, 5000 × [50 + 200], 11-nuclide tier, cores
//! 8–9 of a shared i9-13900K, 2 threads):**
//!
//! - **Borated case-1 water.** k∞ peaks between 1.35 and 1.45 cm pitch
//!   (V_m/V_f ≈ 0.8–1.15): 1.11886 ± 0.00125 at 1.45 cm.
//! - **LCT-008's own pitch.** At 1.63576 cm (V_m/V_f = 1.84) k∞ is
//!   **1.06403 ± 0.00117**, so in that water the lattice is
//!   **over-moderated**, about 5500 pcm below the peak.
//! - **With the soluble boron removed** the peak moves to ≈ 1.75 cm (OpenMC).
//!   LCT-008's pitch is then slightly under-moderated (1.34838 ± 0.00112).
//! - **OpenMC (NJOY2016 ACE, same tier).** The 11 borated points differ by
//!   −409 to +342 pcm with no trend: mean −53 pcm, χ² = 21.9 for 11 points.
//!   Three points sit at 2.0–2.4σ of the single-run σ. This is mild
//!   tension, not a clean pass, and is unresolved without a seed ensemble.
//!   The 4 unborated points agree within 1σ.
//! - **Predictions 3 and 4** (borated peak at V_m/V_f ≈ 2–3, LCT-008
//!   under-moderated) were **refuted**.
//!
//! Full tables, timings and the 36-nuclide points are in `pitch_sweep.md`.
//!
//! # Usage
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example lct008_pitch_sweep -- [--pitches 1.25,1.63576,2.4] \
//!     [--particles 4000 --inactive 50 --active 150] [--seed 1] \
//!     [--cheap-nuclides] [--no-soluble-boron] [--csv out.csv] [--plots DIR]
//! ```

use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, Rgb, SlicePlot};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, XPlane, YPlane, ZCylinder, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::physics::compute::ComputeType;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::reactor_physics::{run_keff_reactor_physics, ReactorPhysicsConfig};
use outram_mc_libs::physics::transport_csg::SourceBox;
use std::collections::BTreeMap;
use std::io::Write;
use std::time::Instant;

#[path = "common/lct008_model.rs"]
mod lct008_model;
use lct008_model::{R_CLAD, R_FUEL, TEMP_K};

/// LCT-008's own lattice pitch [cm], from the committed `geometry.xml`
/// (every 15 × 15 assembly lattice there has `<pitch>1.63576 1.63576</pitch>`).
/// Asserted against the file at start-up, not trusted.
const PITCH_LCT008: f64 = 1.63576;

/// Default sweep [cm]: from just above touching rods (d = 1.206 cm) to well
/// over-moderated. Chosen on geometry alone (V_mod/V_fuel 0.5 → 11.7) before
/// any run; see the prediction in `pitch_sweep.md`.
const DEFAULT_PITCHES: [f64; 11] = [
    1.25,
    1.35,
    1.45,
    1.55,
    PITCH_LCT008,
    1.75,
    1.90,
    2.10,
    2.40,
    2.80,
    3.30,
];

/// Half-height of the reflective cell [cm]. With reflective z-planes the value
/// does not change k∞; it only bounds the initial source box.
const HALF_Z: f64 = 10.0;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    let value = |f: &str| -> Option<&str> {
        let i = args.iter().position(|a| a == f)?;
        args.get(i + 1).map(String::as_str)
    };
    let usize_of = |f: &str, d: usize| value(f).map_or(d, |v| v.parse().expect(f));
    let cheap = flag("--cheap-nuclides");
    let no_boron = flag("--no-soluble-boron");
    let n_particles = usize_of("--particles", 4000);
    let n_inactive = usize_of("--inactive", 50);
    let n_active = usize_of("--active", 150);
    let seed = value("--seed").map_or(KeffSettings::default().seed, |v| v.parse().expect("seed"));
    let pitches: Vec<f64> = value("--pitches").map_or(DEFAULT_PITCHES.to_vec(), |v| {
        v.split(',')
            .map(|t| t.trim().parse().expect("pitch"))
            .collect()
    });
    let csv = value("--csv").map(std::path::PathBuf::from);
    let plots = value("--plots").map(std::path::PathBuf::from);

    // The pitch constant is checked against the model file, not trusted.
    let geo_xml = lct008_model::strip_comments(lct008_model::GEOMETRY_XML_CASE1);
    let file_pitches: Vec<f64> = lct008_model::elements(&geo_xml, "lattice")
        .iter()
        .filter_map(|(_, body)| lct008_model::block::<f64>(body, "pitch").first().copied())
        .filter(|p| *p < 2.0)
        .collect();
    assert!(
        !file_pitches.is_empty() && file_pitches.iter().all(|p| (p - PITCH_LCT008).abs() < 1e-9),
        "assembly pitch in geometry.xml is not {PITCH_LCT008}: {file_pitches:?}"
    );
    for p in &pitches {
        assert!(*p > 2.0 * R_CLAD, "pitch {p} cm is below the rod diameter");
    }

    let tier = if cheap {
        lct008_model::TAPES_CHEAP
    } else {
        lct008_model::TAPES
    };
    eprintln!("LEU-COMP-THERM-008 pin cell, reflective square lattice: k∞ against pitch");
    eprintln!(
        "  nuclide tier: {} ({} tapes); soluble boron: {}",
        if cheap {
            "CHEAP (--cheap-nuclides)"
        } else {
            "FULL (default)"
        },
        tier.len(),
        if no_boron {
            "REMOVED (--no-soluble-boron ablation)"
        } else {
            "case-1, 1511 ppm"
        }
    );

    // ── materials, from the committed case-1 cards ─────────────────────────
    let _ = lct008_model::ACTIVE_CASE.set(1);
    let mut spec = lct008_model::parse_materials(lct008_model::MATERIALS_XML_CASE1);
    assert_eq!(spec.len(), 3, "case 1 has water, fuel and clad");
    if no_boron {
        // Water is the material carrying the S(a,b) law.
        let w = spec.iter_mut().find(|m| m.sab.is_some()).expect("water");
        let before = w.nuclides.len();
        w.nuclides.retain(|(n, _)| n != "B10" && n != "B11");
        eprintln!(
            "  removed {} boron nuclides from \"{}\"",
            before - w.nuclides.len(),
            w.name
        );
    }
    let mut slots: BTreeMap<String, usize> = BTreeMap::new();
    let mut omitted: BTreeMap<String, f64> = BTreeMap::new();
    for m in &spec {
        for (n, ao) in &m.nuclides {
            if tier.iter().any(|(t, _)| t == n) {
                let k = slots.len();
                slots.entry(n.clone()).or_insert(k);
            } else {
                *omitted.entry(n.clone()).or_insert(0.0) += ao;
            }
        }
    }
    let (materials, clad_idx) = lct008_model::build_materials(&spec, &slots, &omitted, false);
    lct008_model::report_omissions(&spec, &omitted);
    let ids: Vec<i32> = materials.iter().map(|m| m.id).collect();
    assert_eq!(
        ids,
        vec![1, 2, 3],
        "case-1 materials are water 1, fuel 2, clad 3"
    );
    assert_eq!(clad_idx, 2);

    let nuclides = load_nuclides(&spec, &slots, tier);

    let mut csv_file = csv.map(|p| {
        let new = !p.is_file();
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&p)
            .expect("csv");
        if new {
            writeln!(
                f,
                "pitch_cm,p_over_d,vm_over_vf,tier,boron,seed,particles,inactive,active,\
                 k_inf,k_std,eta,eta_std,f,f_std,p,p_std,eps,eps_std,p_fnl,p_tnl,\
                 k_factors,consistency_gap,leakage,transport_s"
            )
            .expect("csv header");
        }
        f
    });

    println!(
        "\n  pitch    p/d    Vm/Vf   k_inf ± σ              eta     f       p       eps     \
         leak     t[s]"
    );
    for &pitch in &pitches {
        let geom = pin_cell(0.5 * pitch);
        let vm_vf = (pitch * pitch - std::f64::consts::PI * R_CLAD * R_CLAD)
            / (std::f64::consts::PI * R_FUEL * R_FUEL);
        if let Some(dir) = &plots {
            draw(&geom, pitch, dir, no_boron);
        }
        let settings = KeffSettings {
            n_particles,
            n_inactive,
            n_active,
            temperature_k: TEMP_K,
            compute: ComputeType::CpuMultiThread(Default::default()),
            seed,
            ..KeffSettings::default()
        };
        let cfg = ReactorPhysicsConfig {
            keff: settings,
            source_box: SourceBox {
                lower: Position::new(-R_FUEL, -R_FUEL, -HALF_Z),
                upper: Position::new(R_FUEL, R_FUEL, HALF_Z),
            },
            thermal_cutoff_ev: 0.625,
            ..Default::default()
        };
        let t = Instant::now();
        let rep = run_keff_reactor_physics(&geom, &materials, &nuclides, &cfg)
            .unwrap_or_else(|e| panic!("reactor-physics run at pitch {pitch}: {e:?}"));
        let secs = t.elapsed().as_secs_f64();
        let sf = &rep.six_factors;
        println!(
            "  {pitch:<7.5}  {:.3}  {vm_vf:6.3}  {:.5} ± {:.5}  {:.4}  {:.4}  {:.4}  {:.4}  \
             {:.1e}  {secs:.0}",
            pitch / (2.0 * R_CLAD),
            rep.keff.k_mean,
            rep.keff.k_std,
            sf.eta.mean,
            sf.f.mean,
            sf.p.mean,
            sf.epsilon.mean,
            rep.leakage_total.mean,
        );
        if let Some(f) = csv_file.as_mut() {
            writeln!(
                f,
                "{pitch},{:.4},{vm_vf:.4},{},{},{seed},{n_particles},{n_inactive},{n_active},\
                 {:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},\
                 {:.6},{:.5},{:.3e},{secs:.1}",
                pitch / (2.0 * R_CLAD),
                if cheap { "cheap11" } else { "full36" },
                if no_boron { "none" } else { "case1" },
                rep.keff.k_mean,
                rep.keff.k_std,
                sf.eta.mean,
                sf.eta.std,
                sf.f.mean,
                sf.f.std,
                sf.p.mean,
                sf.p.std,
                sf.epsilon.mean,
                sf.epsilon.std,
                sf.p_fnl.mean,
                sf.p_tnl.mean,
                sf.k_from_factors.mean,
                rep.consistency_gap,
                rep.leakage_total.mean,
            )
            .expect("csv row");
        }
    }
}

/// The reflective square pin cell, centred on the origin: fuel, clad, water.
/// Material slots are the case-1 order (water 0, fuel 1, clad 2).
fn pin_cell(half: f64) -> Geometry {
    let hs = |surface_idx: usize, inside: bool| RegionToken::HalfSpace {
        surface_idx,
        sense: if inside {
            HalfSpaceSense::Inside
        } else {
            HalfSpaceSense::Outside
        },
    };
    let cyl = |r: f64| {
        SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r,
            bc: BoundaryType::Transmissive,
        })
    };
    let refl = BoundaryType::Reflective;
    let surfaces = vec![
        cyl(R_FUEL), // 0
        cyl(R_CLAD), // 1
        SurfaceKind::XPlane(XPlane {
            x0: -half,
            bc: refl,
        }), // 2
        SurfaceKind::XPlane(XPlane { x0: half, bc: refl }), // 3
        SurfaceKind::YPlane(YPlane {
            y0: -half,
            bc: refl,
        }), // 4
        SurfaceKind::YPlane(YPlane { y0: half, bc: refl }), // 5
        SurfaceKind::ZPlane(ZPlane {
            z0: -HALF_Z,
            bc: refl,
        }), // 6
        SurfaceKind::ZPlane(ZPlane {
            z0: HALF_Z,
            bc: refl,
        }), // 7
    ];
    // Every cell is bounded by the box, so nothing is defined outside it.
    let in_box = |mut r: Vec<RegionToken>| {
        for (s, inside) in [
            (2, false),
            (3, true),
            (4, false),
            (5, true),
            (6, false),
            (7, true),
        ] {
            r.push(hs(s, inside));
            r.push(RegionToken::Intersection);
        }
        r
    };
    let cells = vec![
        Cell::material(1, in_box(vec![hs(0, true)]), 1, TEMP_K),
        Cell::material(
            2,
            in_box(vec![hs(0, false), hs(1, true), RegionToken::Intersection]),
            2,
            TEMP_K,
        ),
        Cell::material(3, in_box(vec![hs(1, false)]), 0, TEMP_K),
    ];
    Geometry {
        surfaces,
        cells,
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0, 1, 2],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

/// Draw what the solver sees: an x-y slice of the ASSEMBLED pin cell, by
/// material, with legend and cm axes (crate geometry-drawing rule).
fn draw(geom: &Geometry, pitch: f64, dir: &std::path::Path, no_boron: bool) {
    std::fs::create_dir_all(dir).expect("plot dir");
    let w = pitch * 1.1;
    let plot = SlicePlot {
        origin: Position::new(0.0, 0.0, 0.0),
        basis: PlotBasis::Xy,
        width: [w, w],
        pixels: [600, 600],
        level: None,
        show_overlaps: true,
        meshlines: None,
    };
    let palette = [
        (
            Rgb::new(70, 130, 220),
            if no_boron {
                "WATER (NO SOLUBLE B)"
            } else {
                "WATER (1511 PPM B)"
            },
        ),
        (Rgb::new(220, 60, 40), "UO2 2.459 W/O"),
        (Rgb::new(150, 150, 150), "AL-6061 CLAD"),
    ];
    let title = format!("LCT-008 PIN CELL, PITCH {pitch:.5} CM, REFLECTIVE");
    let (_raw, img) = render_material_slice(geom, &plot, &palette, &title);
    let tag = if no_boron { "_noboron" } else { "" };
    let path = dir.join(format!("lct008_pin_cell_pitch_{pitch:.3}cm{tag}.png"));
    img.write_png(&path).expect("write png");
    eprintln!("  drew {}", path.display());
}

/// Reconstruct the tier's nuclides that the materials name, with `H in H2O`
/// S(α,β) on H-1. Mirrors `lct008_keff.rs::load_nuclides` (ENDF route only).
fn load_nuclides(
    spec: &[lct008_model::MaterialSpec],
    slots: &BTreeMap<String, usize>,
    tier: &[(&str, &str)],
) -> Vec<Nuclide> {
    let t0 = Instant::now();
    eprintln!("Reconstructing nuclides (RECONR + BROADR @ {TEMP_K} K, URR + DBRC by default):");
    let sab_needed = spec.iter().any(|m| m.sab.as_deref() == Some("c_H_in_H2O"));
    let mut sab = sab_needed.then(|| {
        ThermalScattering::from_endf_file(
            reference_endf("tsl-HinH2O.endf")
                .expect("H(H2O) tape")
                .to_str()
                .expect("path"),
            1, // MAT 1 — H in H2O, ENDF/B-VIII.0
            TEMP_K,
            "c_H_in_H2O",
        )
        .expect("H(H2O) S(a,b)")
    });
    let mut by_slot: Vec<(usize, &str)> = slots.iter().map(|(n, &i)| (i, n.as_str())).collect();
    by_slot.sort_unstable();
    let mut out = Vec::with_capacity(by_slot.len());
    for (i, name) in by_slot {
        assert_eq!(i, out.len());
        let file = tier.iter().find(|(n, _)| *n == name).expect("tape").1;
        let p = reference_endf(file).unwrap_or_else(|| panic!("missing reference tape {file}"));
        let t = Instant::now();
        let mut n =
            Nuclide::from_endf_file_with_speed(&p, name, TEMP_K, outram_mc_libs::vv::bench_speed())
                .unwrap_or_else(|e| panic!("from_endf_file_with_speed({}): {e}", p.display()));
        if name == "H1" {
            if let Some(s) = sab.take() {
                n = n.with_thermal_scattering(s);
            }
        }
        eprintln!(
            "  {name:<6} {:6.1} s  urr={} dbrc={} sab={}",
            t.elapsed().as_secs_f64(),
            n.urr_range_ev().is_some(),
            n.has_dbrc(),
            name == "H1" && sab_needed
        );
        out.push(n);
    }
    eprintln!("Nuclear data ready in {:.1} s.", t0.elapsed().as_secs_f64());
    out
}
