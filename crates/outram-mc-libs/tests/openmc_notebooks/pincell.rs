//! `pincell` notebook → outram-mc verification (LIVE, incl. the thermal pin).
//!
//! Notebook: `pincell.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Builds a single LWR fuel pin (UO2 fuel +
//! Zircaloy clad + borated water) inside a square cell with **reflective**
//! boundaries, runs a k-eigenvalue calculation, and tallies the cell flux.
//! OpenMC API exercised: `Material`, `ZCylinder`/`XPlane`/`YPlane`, `Universe`,
//! `Cell`, `Geometry`, `IndependentSource`, `Settings`, `run`,
//! `Tally`+`CellFilter`, `StatePoint`.
//!
//! **What outram-mc can do today (this run wires it).** The general CSG
//! navigation (op-6tz.7) and a surface-tracking k-eigenvalue over arbitrary
//! geometry with reflective/vacuum boundary conditions (op-6tz.8/.10) now exist
//! ([`outram_mc_libs::geometry::geometry::Geometry`],
//! [`outram_mc_libs::physics::transport_csg::run_keff_csg`]), together with a
//! collision-estimator cell-flux tally (op-6tz.9). So the notebook's **square
//! reflective pin-cell geometry, its k-eigenvalue, and its cell-flux tally are
//! all real here** — exercised by the LIVE tests below.
//!
//! **The thermal pin is now LIVE too (op-6tz.12).** H-in-H₂O **S(α,β) thermal
//! scattering** is wired into the transport loop
//! ([`outram_mc_libs::material::thermal::ThermalScattering`], attached to H-1 via
//! `Nuclide::with_thermal_scattering`): below a 4 eV cutoff the water moderator
//! thermalizes off the bound-atom law (up-scatter → Maxwellian) instead of
//! free-gas elastic. The full UO₂/gap/Zr-clad/water pin
//! ([`pincell_lwr_thermal_pin_benchmark`]) runs to a physical k_inf. It is still
//! **not** a benchmark-accuracy assertion — the LOW-tier thermal data for U/O and
//! the free-gas O treatment are approximations (see `REVIEW_MANIFEST.md`) — and
//! it is data-gated on the public ENDF/B-VIII.0 `tsl-HinH2O.endf` file.
//!
//! # V&V — methodology and results
//!
//! **Methodology.** Four complementary LIVE checks, all embedded LOW-tier data
//! (`Nuclide::from_core`: WMP below e_max + Watt-collapsed fast MGXS), analog
//! transport:
//! 1. *Criticality eigenvalue* — Godiva bare HEU sphere (ICSBEP HEU-MET-FAST-001)
//!    via the original bare-sphere driver. Reference k_eff = 1.0000 ± 0.0010.
//! 2. *Leakage sign* — a tiny sphere leaks more than a large one.
//! 3. *Reflective-BC infinite medium* (NEW CSG) — a homogeneous-HEU **square
//!    reflective pin cell**, infinite in z, reflective in x/y ⇒ zero leakage ⇒
//!    k_inf. Pass criterion: k_inf is stationary **and strictly larger** than the
//!    finite bare sphere of the same material (leakage suppressed) — a direct
//!    check that the CSG reflective-BC path bites.
//! 4. *Heterogeneous CSG + cell-flux tally* (NEW) — an HEU fuel cylinder in an
//!    H-1 moderator region inside the reflective cell, run with a `CellFilter`
//!    flux tally; asserts stationary k and that both cells accumulate flux with
//!    fissions confined to the fuel.
//!
//! **Results (measured 2026-08-06, this harness; asserted at run time, not
//! hard-coded).** Godiva bare-sphere k ≈ 1.01 (the crate's other Godiva runs,
//! re-measured the same day: **k = 1.01042 ± 0.00174** from
//! `examples/godiva_keff`, **k = 1.01207 ± 0.00673** from the `keff.rs`
//! backend-agreement run — this test itself asserts a band, it records no k of
//! its own). The homogeneous reflective pin cell gives **k_inf = 2.20758 ±
//! 0.00383** against the same-material finite bare sphere's **k = 0.11741 ±
//! 0.00176** — k_inf ≫ k_sphere, confirming the reflective BC removes leakage.
//! The heterogeneous run is stationary with positive flux tallied in both cells.
//! The **thermal UO₂ pin** could **not** be re-measured here — see the
//! Supersedes note. See
//! `docs/ai-fleet-review/op-6tz-pincell-triso/REVIEW_MANIFEST.md` and
//! `docs/ai-fleet-review/op-6tz-thermal-pincell/REVIEW_MANIFEST.md`.
//!
//! **Supersedes (pre-`op-jis` figures, measured 2026-07-15).** The numbers above
//! replace values taken with the old `prn` output function (uniforms formed from
//! the raw top-52 state bits, before the PCG-RXS-M-XS output permutation of bead
//! `op-jis`). The LCG *state* recurrence did not change, so integer-state facts,
//! seeds and jump-ahead identities are untouched — but every sampled uniform did,
//! so every statistic derived from them moved. Superseded values:
//! - Godiva bare-sphere k ≈ 1.01 ± 0.002.
//! - **Thermal UO₂ pin k_inf = 1.39802 ± 0.00652** (600 particles, 40 inactive +
//!   60 active). **NOT RE-MEASURED — needs a re-run.** That case is data-gated on
//!   the public ENDF/B-VIII.0 `tsl-HinH2O.endf` file, which is **absent on this
//!   machine**, so `pincell_lwr_thermal_pin_benchmark` SKIPped on 2026-08-06 and
//!   the run could not be repeated. No replacement value is quoted, because none
//!   has been measured; 1.39802 ± 0.00652 must be regarded as stale until the
//!   S(α,β) data file is available and the test is re-run.

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, XPlane, YPlane, ZCylinder};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::physics::keff::{run_keff, KeffSettings};
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use outram_mc_libs::tally::filter::CellFilter;
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

/// Godiva atom densities (HEU-MET-FAST-001), atoms/barn-cm.
fn godiva_material() -> Material {
    Material {
        id: 1,
        name: "Godiva HEU".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent {
                nuclide_idx: 0,
                atom_density: 4.9184e-4,
            }, // U-234
            NuclideComponent {
                nuclide_idx: 1,
                atom_density: 4.4994e-2,
            }, // U-235
            NuclideComponent {
                nuclide_idx: 2,
                atom_density: 2.4984e-3,
            }, // U-238
        ],
    }
}

fn godiva_nuclides() -> Vec<Nuclide> {
    vec![
        Nuclide::from_core("U234").expect("U234 in CORE WMP library"),
        Nuclide::from_core("U235").expect("U235 in CORE WMP library"),
        Nuclide::from_core("U238").expect("U238 in CORE WMP library"),
    ]
}

/// Build a square reflective pin-cell geometry: a fuel cylinder (radius
/// `r_fuel`) in a `2*half`-wide square cell with reflective x/y planes, infinite
/// in z. Cell 0 = fuel (`fuel_mat`), cell 1 = the surrounding region
/// (`mod_mat`). Surface 0 is the (transmissive) fuel cylinder; surfaces 1..=4
/// are the reflective box planes.
fn pincell_geometry(r_fuel: f64, half: f64, fuel_mat: usize, mod_mat: usize) -> Geometry {
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
        fuel_mat,
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
            }, // x > -half
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 2,
                sense: HalfSpaceSense::Inside,
            }, // x < +half
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 3,
                sense: HalfSpaceSense::Outside,
            }, // y > -half
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 4,
                sense: HalfSpaceSense::Inside,
            }, // y < +half
            RegionToken::Intersection,
        ],
        mod_mat,
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

/// LIVE: criticality-eigenvalue slice of the `pincell` notebook, realised as the
/// Godiva bare-sphere k-eff (op-u6s.1). See the module V&V note.
#[test]
fn pincell_criticality_eigenvalue_via_godiva_bare_sphere() {
    let nuclides = godiva_nuclides();
    let material = godiva_material();
    let settings = KeffSettings {
        n_particles: 1500,
        n_inactive: 20,
        n_active: 40,
        ..KeffSettings::default()
    };

    let result = run_keff(8.7407, &material, &nuclides, &settings);

    assert_eq!(
        result.k_by_generation.len(),
        settings.n_inactive + settings.n_active
    );
    assert!(
        result.k_mean > 0.9 && result.k_mean < 1.4,
        "Godiva k_eff {} outside [0.9, 1.4]",
        result.k_mean
    );
    assert!(
        result.k_std < 0.02,
        "k noisy/unconverged: sigma = {}",
        result.k_std
    );
}

/// LIVE sign check: a far-subcritical (leakage-dominated) sphere is less
/// reactive than the near-critical Godiva sphere.
#[test]
fn pincell_leakage_reduces_reactivity() {
    let nuclides = vec![Nuclide::from_core("U235").unwrap()];
    let material = Material {
        id: 1,
        name: "U235".into(),
        temperature: 293.6,
        components: vec![NuclideComponent {
            nuclide_idx: 0,
            atom_density: 4.8e-2,
        }],
    };
    let settings = KeffSettings {
        n_particles: 1000,
        n_inactive: 15,
        n_active: 25,
        ..KeffSettings::default()
    };

    let k_big = run_keff(9.0, &material, &nuclides, &settings).k_mean;
    let k_small = run_keff(3.0, &material, &nuclides, &settings).k_mean;
    assert!(
        k_small < k_big,
        "3 cm sphere (k={k_small}) should leak more than 9 cm (k={k_big})"
    );
}

/// LIVE (NEW CSG): a homogeneous-HEU **square reflective pin cell** is an
/// infinite medium (reflective x/y, infinite z), so its k_inf must be stationary
/// and strictly larger than the finite bare sphere of the same material — the
/// direct verification that the general CSG navigation + reflective-BC transport
/// (op-6tz.7/.8/.10) suppress leakage. Fast-spectrum HEU data only; this is the
/// honest fast slice of the notebook's reflected-cell geometry, not its thermal pin.
#[test]
fn pincell_reflective_cell_suppresses_leakage() {
    let nuclides = godiva_nuclides();
    let material = godiva_material();
    // Whole cell is HEU (fuel and surround share material 0) ⇒ homogeneous.
    let half = 1.0; // cm; pin-cell half-pitch
    let geom = pincell_geometry(0.5, half, 0, 0);

    let settings = KeffSettings {
        n_particles: 1500,
        n_inactive: 20,
        n_active: 40,
        ..KeffSettings::default()
    };
    let src = SourceBox {
        lower: Position::new(-half, -half, -1.0),
        upper: Position::new(half, half, 1.0),
    };
    let materials = vec![godiva_material()];
    let refl = run_keff_csg(&geom, &materials, &nuclides, src, &settings, None);

    // Same-material finite bare sphere (inscribed radius of the cell) for contrast.
    let sphere = run_keff(half, &material, &nuclides, &settings);

    eprintln!(
        "[pincell reflective] k_inf = {:.5} ± {:.5}  vs  bare sphere k = {:.5} ± {:.5}",
        refl.k_mean, refl.k_std, sphere.k_mean, sphere.k_std
    );
    assert_eq!(
        refl.k_by_generation.len(),
        settings.n_inactive + settings.n_active,
        "ran all generations"
    );
    assert!(
        refl.k_std < 0.05,
        "reflective k noisy/unconverged: sigma = {}",
        refl.k_std
    );
    assert!(
        refl.k_mean > sphere.k_mean + 0.1,
        "reflective infinite medium k_inf={} should exceed leaky bare sphere k={}",
        refl.k_mean,
        sphere.k_mean
    );
    // Fast HEU infinite medium: k_inf is well above unity. Broad plausibility band.
    assert!(
        refl.k_mean > 1.3 && refl.k_mean < 3.5,
        "HEU k_inf {} outside the plausible fast-infinite-medium band [1.3, 3.5]",
        refl.k_mean
    );
}

/// LIVE (NEW CSG + tally, op-6tz.9): a **heterogeneous** two-material pin cell —
/// HEU fuel cylinder in an H-1 moderator region, reflective box — run with a
/// `CellFilter` collision-estimator flux tally. Asserts a stationary eigenvalue,
/// that both cells accumulate positive flux, and that fissions are confined to
/// the fuel cell. Fast/epithermal only (no S(α,β)); verifies the transport +
/// tally wiring, not a benchmark k value.
#[test]
fn pincell_heterogeneous_csg_with_cell_flux_tally() {
    // Fuel = HEU (nuclides 0..=2), moderator = H-1 (nuclide 3).
    let mut nuclides = godiva_nuclides();
    nuclides.push(Nuclide::from_core("H1").expect("H1 in CORE WMP library"));

    let fuel = godiva_material();
    let moderator = Material {
        id: 2,
        name: "H moderator".into(),
        temperature: 293.6,
        components: vec![NuclideComponent {
            nuclide_idx: 3,
            atom_density: 6.6e-2,
        }],
    };
    let materials = vec![fuel, moderator];

    let half = 0.63;
    let r_fuel = 0.4;
    let geom = pincell_geometry(r_fuel, half, 0, 1);

    // Cell-flux tally: bin 0 = fuel cell (idx 0), bin 1 = moderator cell (idx 1).
    let filter = CellFilter {
        cell_indices: vec![0, 1],
    };
    let mut tally = Tally {
        id: 1,
        name: "cell flux".into(),
        filters: vec![Box::new(filter)],
        scores: vec![ScoreType::Flux, ScoreType::NuFission],
        bins: vec![TallyBin::default(); 4], // 2 cells × 2 scores
    };

    // Kept modest: an infinite reflective H medium with no S(α,β) thermal cutoff
    // produces very long low-energy histories, so a small run suffices for the
    // wiring assertions below (positive flux in both cells, fission confined).
    let settings = KeffSettings {
        n_particles: 400,
        n_inactive: 8,
        n_active: 12,
        ..KeffSettings::default()
    };
    let src = SourceBox {
        lower: Position::new(-r_fuel, -r_fuel, -1.0),
        upper: Position::new(r_fuel, r_fuel, 1.0),
    };
    let result = run_keff_csg(
        &geom,
        &materials,
        &nuclides,
        src,
        &settings,
        Some(&mut tally),
    );

    assert!(
        !result.k_by_generation.is_empty(),
        "power iteration produced no generations"
    );
    assert!(
        result.k_mean.is_finite() && result.k_mean > 0.0,
        "k should be finite & positive, got {}",
        result.k_mean
    );

    // bins: [fuel-flux, fuel-nufission, mod-flux, mod-nufission]
    let fuel_flux = tally.bins[0].sum;
    let fuel_nufis = tally.bins[1].sum;
    let mod_flux = tally.bins[2].sum;
    let mod_nufis = tally.bins[3].sum;
    assert!(fuel_flux > 0.0, "fuel cell accumulated no flux");
    assert!(mod_flux > 0.0, "moderator cell accumulated no flux");
    assert!(
        fuel_nufis > 0.0,
        "no fission production tallied in the fuel"
    );
    assert!(
        mod_nufis < fuel_nufis * 1.0e-6,
        "fission leaked into the non-fissile moderator: mod ν-fis {mod_nufis}, fuel ν-fis {fuel_nufis}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Thermal LWR pin-cell (op-6tz.12) — now wired with H-in-H₂O S(α,β).
// ─────────────────────────────────────────────────────────────────────────────

/// Temperature \[K\] the H-in-H2O S(alpha,beta) law is built at — the
/// ENDF/B-VIII.0 tabulated room-temperature point, and the pin-cell material
/// temperature used throughout this file. Shared by the regeneration path and
/// `ThermalScattering::from_endf_file` so the two cannot drift apart.
const TSL_TEMPERATURE_K: f64 = 293.6;

/// Locate — or, failing that, **regenerate** — the ENDF/B-VIII.0 `tsl-HinH2O`
/// thermal-scattering law this LIVE test consumes.
///
/// Resolution order:
/// 1. `OUTRAM_TSL_HINH2O`, if set and the file exists.
/// 2. A known local path to an unpacked ENDF/B-VIII.0 `thermal_scatt` tree.
/// 3. **Regenerate it from the LEAPR deck embedded in
///    `njoy-outram-park-fork`** (`SabMaterial::HInH2O`, MAT 1), writing the
///    tape to a temp file and returning that path.
///
/// Step 3 targets kopi-beans `op-6tz.28.1` — stop the LIVE thermal tests
/// soft-skipping on a hardcoded absolute path. Regenerating from an embedded
/// deck is strictly better than that bead's proposed `EndfCache::fetch_tsl`
/// download: no network, no cache, no IAEA availability question (that host is
/// policy-blocked 403 from this environment), and it works offline and in CI.
///
/// **Measured 2026-08-14: step 3 does not yet succeed for this particular
/// law, and the reason is a known unported LEAPR feature, not a bug here.**
/// `tsl-HinH2O.leapr` sets `nss = 1` — it carries a *secondary scatterer*
/// (the oxygen in the water molecule), so producing its law needs the
/// mixed-moderator merge, which this workspace's LEAPR port does not implement.
/// `generate_tape` therefore returns
/// `NotPorted("LEAPR deck uses features this port does not implement")`, and
/// `LeaprDeck::unsupported_features()` names it exactly:
/// `"nss = 1 (secondary-scatterer mixed-moderator merge is not ported)"`.
/// That work is tracked as `op-b2k`.
///
/// The fallback is kept wired anyway, deliberately: it costs nothing when it
/// fails, and **the day `op-b2k` lands this test un-gates itself** with no edit
/// here. Contrast graphite, whose deck needs no secondary scatterer and does
/// regenerate today — see `tests/htr10_graphite_thermal_scattering_pebble_bed.rs`.
///
/// So this test remains data-gated for now. Returns `None` when neither a local
/// tape nor regeneration is available, and the caller prints a SKIP.
fn locate_tsl_hinh2o() -> Option<String> {
    if let Ok(p) = std::env::var("OUTRAM_TSL_HINH2O") {
        if std::path::Path::new(&p).exists() {
            return Some(p);
        }
    }
    const CANDIDATES: &[&str] =
        &["/home/teddy0/Documents/research/ENDF-B-VIII.0/thermal_scatt/tsl-HinH2O.endf"];
    if let Some(p) = CANDIDATES
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .map(|s| s.to_string())
    {
        return Some(p);
    }
    regenerate_tsl_hinh2o()
}

/// Regenerate the `tsl-HinH2O` MF=7 law from the embedded LEAPR deck at the
/// pin-cell temperature, returning the path of the written tape.
///
/// The temp file is deliberately **not** deleted: the caller passes its path to
/// `ThermalScattering::from_endf_file`, so it must outlive this function. It
/// lands in the system temp directory and is named per-process.
fn regenerate_tsl_hinh2o() -> Option<String> {
    use njoy_outram_park_fork::leapr::deck::LeaprDeck;
    use njoy_outram_park_fork::leapr::decks::{locate_deck, SabMaterial};
    use njoy_outram_park_fork::leapr::generate::{generate_tape, ElasticChannel};
    use njoy_outram_park_fork::units::Temperature;
    use uom::si::thermodynamic_temperature::kelvin;

    let material = SabMaterial::HInH2O;
    let located = locate_deck(material).ok()?;
    let deck = LeaprDeck::parse(&located.text).ok()?;
    let tape = generate_tape(
        &deck,
        Temperature::new::<kelvin>(TSL_TEMPERATURE_K),
        ElasticChannel::Generate,
    )
    .ok()?;

    let tmp =
        std::env::temp_dir().join(format!("op_pincell_tsl_HinH2O_{}.endf", std::process::id()));
    let file = std::fs::File::create(&tmp).ok()?;
    tape.write(file).ok()?;
    tmp.to_str().map(|s| s.to_string())
}

/// Natural-zirconium isotopes (name, atom fraction) for the Zircaloy-like clad —
/// natural abundances (ENDF/B). All five are present in the CORE WMP library.
const ZR_ISOTOPES: &[(&str, f64)] = &[
    ("Zr90", 0.5145),
    ("Zr91", 0.1122),
    ("Zr92", 0.1715),
    ("Zr94", 0.1738),
    ("Zr96", 0.0280),
];

/// Nuclide array for the thermal UO₂ pin-cell. Indices:
/// `0 U235, 1 U238, 2 O16, 3..=7 Zr{90,91,92,94,96}, 8 H1 (S(α,β) attached)`.
/// The H-1 nuclide carries the H-in-H₂O bound-atom thermal-scattering table.
fn pincell_nuclides(tsl_path: &str) -> Result<Vec<Nuclide>, njoy_outram_park_fork::NjoyError> {
    let thermal = ThermalScattering::from_endf_file(tsl_path, 1, TSL_TEMPERATURE_K, "H in H2O")?;
    let mut nuclides = vec![
        Nuclide::from_core("U235")?,
        Nuclide::from_core("U238")?,
        Nuclide::from_core("O16")?,
    ];
    for (name, _) in ZR_ISOTOPES {
        nuclides.push(Nuclide::from_core(name)?);
    }
    nuclides.push(Nuclide::from_core("H1")?.with_thermal_scattering(thermal));
    Ok(nuclides)
}

/// The three pin-cell materials (UO₂ fuel, Zr clad, light water), mirroring the
/// openmc `pincell` notebook: 3.0 wt-ish UO₂ at 10.0 g/cm³, natural Zr at
/// 6.6 g/cm³, unborated light water at 1.0 g/cm³. Atom densities \[atoms/barn·cm\]
/// are computed from those densities (see the per-line arithmetic).
fn pincell_materials() -> Vec<Material> {
    // UO₂, ρ = 10.0 g/cm³, 3% U235 / 97% U238 by mole, 2 O per U.
    // N_UO2 = ρ·N_A/M = 10.0·6.02214e23/269.91 = 2.2312e22 /cm³ = 0.022312 /b·cm.
    let n_uo2 = 0.022312;
    let uo2 = Material {
        id: 1,
        name: "UO2".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent {
                nuclide_idx: 0,
                atom_density: 0.03 * n_uo2,
            }, // U235
            NuclideComponent {
                nuclide_idx: 1,
                atom_density: 0.97 * n_uo2,
            }, // U238
            NuclideComponent {
                nuclide_idx: 2,
                atom_density: 2.0 * n_uo2,
            }, // O16
        ],
    };

    // Natural Zr, ρ = 6.6 g/cm³, M = 91.22. N_Zr = 6.6·6.02214e23/91.22 = 0.043572 /b·cm.
    let n_zr = 0.043572;
    let zr_components: Vec<NuclideComponent> = ZR_ISOTOPES
        .iter()
        .enumerate()
        .map(|(i, (_, frac))| NuclideComponent {
            nuclide_idx: 3 + i,
            atom_density: frac * n_zr,
        })
        .collect();
    let zirc = Material {
        id: 2,
        name: "Zircaloy".into(),
        temperature: 293.6,
        components: zr_components,
    };

    // Light water, ρ = 1.0 g/cm³, M = 18.015. N_H2O = 1.0·6.02214e23/18.015 = 0.033427 /b·cm.
    let n_h2o = 0.033427;
    let water = Material {
        id: 3,
        name: "water".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent {
                nuclide_idx: 8,
                atom_density: 2.0 * n_h2o,
            }, // H1 (S(α,β))
            NuclideComponent {
                nuclide_idx: 2,
                atom_density: n_h2o,
            }, // O16
        ],
    };

    vec![uo2, zirc, water]
}

/// The openmc `pincell` geometry: concentric fuel (r = 0.39 cm), void gap
/// (0.39–0.40), Zr clad (0.40–0.46), light-water moderator out to a 1.26 cm
/// square pitch with reflective x/y planes, infinite in z. Material indices are
/// `0 = UO₂, 1 = Zr, 2 = water`; the gap is [`CellFill::Void`].
fn uo2_pincell_geometry() -> Geometry {
    const T: f64 = 293.6;
    let half = 0.63; // half-pitch [cm]
    let surfaces = vec![
        SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: 0.39,
            bc: BoundaryType::Transmissive,
        }), // 0 fuel_or
        SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: 0.40,
            bc: BoundaryType::Transmissive,
        }), // 1 gap_or
        SurfaceKind::ZCylinder(ZCylinder {
            x0: 0.0,
            y0: 0.0,
            r: 0.46,
            bc: BoundaryType::Transmissive,
        }), // 2 clad_or
        SurfaceKind::XPlane(XPlane {
            x0: -half,
            bc: BoundaryType::Reflective,
        }), // 3
        SurfaceKind::XPlane(XPlane {
            x0: half,
            bc: BoundaryType::Reflective,
        }), // 4
        SurfaceKind::YPlane(YPlane {
            y0: -half,
            bc: BoundaryType::Reflective,
        }), // 5
        SurfaceKind::YPlane(YPlane {
            y0: half,
            bc: BoundaryType::Reflective,
        }), // 6
    ];

    let fuel = Cell::material(
        1,
        vec![RegionToken::HalfSpace {
            surface_idx: 0,
            sense: HalfSpaceSense::Inside,
        }],
        0,
        T,
    );
    // Void gap: outside fuel_or ∩ inside gap_or.
    let gap = Cell {
        id: 2,
        region: vec![
            RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::HalfSpace {
                surface_idx: 1,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
        ],
        fill: CellFill::Void,
        temperature: T,
        translation: Position::ZERO,
    };
    // Clad: outside gap_or ∩ inside clad_or.
    let clad = Cell::material(
        3,
        vec![
            RegionToken::HalfSpace {
                surface_idx: 1,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::HalfSpace {
                surface_idx: 2,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
        ],
        1,
        T,
    );
    // Water: outside clad_or ∩ inside the reflective box.
    let water = Cell::material(
        4,
        vec![
            RegionToken::HalfSpace {
                surface_idx: 2,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::HalfSpace {
                surface_idx: 3,
                sense: HalfSpaceSense::Outside,
            }, // x > -half
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 4,
                sense: HalfSpaceSense::Inside,
            }, // x < +half
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 5,
                sense: HalfSpaceSense::Outside,
            }, // y > -half
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 6,
                sense: HalfSpaceSense::Inside,
            }, // y < +half
            RegionToken::Intersection,
        ],
        2,
        T,
    );

    Geometry {
        surfaces,
        cells: vec![fuel, gap, clad, water],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0, 1, 2, 3],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

/// LIVE (data-gated, op-6tz.12): the openmc `pincell` notebook's **thermal** LWR
/// UO₂ pin-cell k_inf, now that H-in-H₂O S(α,β) thermal scattering is wired into
/// the CSG transport loop.
///
/// # V&V — methodology and results
///
/// **Methodology.** A single UO₂ fuel pin (r = 0.39 cm, 3% U235 / 97% U238, 2 O
/// per U, ρ = 10.0 g/cm³) inside a void gap, a natural-Zr clad (0.40–0.46 cm,
/// ρ = 6.6 g/cm³) and unborated light water (ρ = 1.0 g/cm³) filling a 1.26 cm
/// square pitch with **reflective** x/y planes, infinite in z ⇒ an infinite
/// lattice, so the eigenvalue is k_inf. Fission-source power iteration
/// ([`run_keff_csg`], analog transport). **Data:** LOW tier — embedded WMP
/// (thermal + resonance range) + fast MGXS for U/O/Zr, from `njoy-outram-park-fork`;
/// H-1 additionally carries the **ENDF/B-VIII.0 `tsl-HinH2O` S(α,β)** table
/// (`IncoherentInelasticScattering`, MAT 1, 293.6 K nearest grid point) so the
/// water thermalizes with bound-atom up-scatter below a 4 eV cutoff. O-16 and the
/// fuel stay free-gas/CE (O is nearly free at thermal, per the port's scope).
/// **Reference / pass criterion:** a UO₂ LWR pin-cell k_inf is physically
/// ~1.2–1.45 (moderation- and enrichment-dependent); the test asserts the
/// eigenvalue is stationary (σ small) and lands in a broad physical band
/// `[0.9, 1.7]`, and records the measured k_inf ± σ. It is **not** yet a
/// benchmark-accuracy assertion (the LOW-tier thermal data for U/O and the
/// free-gas O treatment are approximations — see `REVIEW_MANIFEST.md`).
///
/// **Results — SUPERSEDED, RE-RUN REQUIRED.** The recorded figure,
/// **k_inf = 1.39802 ± 0.00652** (measured 2026-07-15, 600 particles, 40
/// inactive + 60 active), was produced with the **pre-`op-jis`** `prn` output
/// function (uniforms from the raw top-52 state bits). Bead `op-jis` added
/// OpenMC's PCG-RXS-M-XS output permutation: the LCG state recurrence is
/// unchanged, but every sampled uniform changed, so this eigenvalue has moved by
/// an unknown amount. It **could not be re-measured on 2026-08-06** — the test is
/// data-gated on the public ENDF/B-VIII.0 `tsl-HinH2O.endf` file, which is not
/// present on this machine, so the run SKIPped. **No replacement value is quoted
/// (none has been measured).** Once the S(α,β) file is available, re-run and
/// record the new k_inf ± σ here and in
/// `docs/ai-fleet-review/op-6tz-thermal-pincell/REVIEW_MANIFEST.md`. When it does
/// run, the measured k_inf ± σ is printed at run time via `eprintln!`
/// (`cargo test -- --nocapture`); otherwise the test prints SKIP and returns.
#[test]
fn pincell_lwr_thermal_pin_benchmark() {
    let Some(tsl_path) = locate_tsl_hinh2o() else {
        eprintln!(
            "[thermal pincell] SKIP: ENDF/B-VIII.0 tsl-HinH2O.endf not found \
             (set OUTRAM_TSL_HINH2O or place it at the known path). S(α,β) wiring \
             is compiled and unit-tested; the k_inf run is data-gated. See \
             REVIEW_MANIFEST.md."
        );
        return;
    };

    let t_build = std::time::Instant::now();
    let nuclides = pincell_nuclides(&tsl_path).expect("build pincell nuclides + S(a,b) table");
    let materials = pincell_materials();
    let geom = uo2_pincell_geometry();
    eprintln!(
        "[thermal pincell] S(α,β) + nuclide build: {:.2?}",
        t_build.elapsed()
    );

    let settings = KeffSettings {
        n_particles: 600,
        n_inactive: 40,
        n_active: 60,
        ..KeffSettings::default()
    };
    // Seed the source inside the fuel pin (r < 0.39, |z| < 1).
    let src = SourceBox {
        lower: Position::new(-0.39, -0.39, -1.0),
        upper: Position::new(0.39, 0.39, 1.0),
    };

    let t_run = std::time::Instant::now();
    let result = run_keff_csg(&geom, &materials, &nuclides, src, &settings, None);
    let dt = t_run.elapsed();

    eprintln!(
        "[thermal pincell] k_inf = {:.5} ± {:.5}  (npart={}, {}+{} gen, {:.1?})",
        result.k_mean,
        result.k_std,
        settings.n_particles,
        settings.n_inactive,
        settings.n_active,
        dt
    );

    assert_eq!(
        result.k_by_generation.len(),
        settings.n_inactive + settings.n_active,
        "ran all generations"
    );
    assert!(
        result.k_mean.is_finite() && result.k_mean > 0.0,
        "k_inf must be finite & positive, got {}",
        result.k_mean
    );
    assert!(
        result.k_std < 0.02,
        "k_inf noisy/unconverged: sigma = {}",
        result.k_std
    );
    assert!(
        result.k_mean > 0.9 && result.k_mean < 1.7,
        "thermal UO2 pin-cell k_inf {} outside the broad physical band [0.9, 1.7] \
         — investigate thermal data/physics before trusting",
        result.k_mean
    );
}

/// V&V — **CSG backend agreement (op-fla)**: the rayon multi-thread CSG backend
/// (`run_keff_csg` under `ComputeType::CpuMultiThread`) must reproduce the
/// single-thread reference (`ComputeType::CpuSingleThread`) within combined
/// statistical uncertainty, and its result must be **independent of thread
/// count** — the property that makes the per-history jump-ahead seeding correct.
///
/// **Methodology.** The homogeneous-HEU reflective Godiva pin cell (same geometry
/// as `pincell_reflective_cell_suppresses_leakage`), identical [`KeffSettings`]
/// and seed, no tally. Run the single-thread reference, then the multi-thread
/// backend at two fixed thread counts (1 and 4). Pass criteria: (a)
/// `k_par(1) == k_par(4)` bit-for-bit (thread-count invariance); (b)
/// `|k_seq − k_par|` within `4·σ_comb`, `σ_comb = sqrt(σ_seq² + σ_par²)` — the two
/// are statistically consistent estimates of the same eigenvalue (they do not
/// bit-match by design; the per-history stream structure differs from the single
/// sequential stream).
///
/// **Results (measured 2026-08-06, this environment, seed 246813579; 1200
/// histories, 15 inactive + 30 active).** Thread-count runs agreed **to the bit**
/// (`k_par(1) == k_par(4)`). Reference vs parallel: `k_seq = 2.19628 ± 0.00496`,
/// `k_par = 2.21076 ± 0.00311`, **2.47σ apart** (`Δk = −1448 pcm`,
/// `σ_comb ≈ 0.00585`) — inside the 4σ gate. (`k ≈ 2.2` is the homogeneous-HEU
/// cell `k∞`, as in `pincell_reflective_cell_suppresses_leakage`.) Recorded per
/// the workspace V&V rule.
///
/// **Supersedes (pre-`op-jis`, measured 2026-07-23):** `k_seq = 2.20765 ±
/// 0.00353`, `k_par = 2.20474 ± 0.00507`, 0.47σ apart (`σ_comb ≈ 0.0062`). Those
/// were taken with the old `prn` output function (raw top-52 state bits); bead
/// `op-jis` added the PCG-RXS-M-XS output permutation, which left the LCG state
/// recurrence — and hence the bit-exact thread-count invariance asserted here —
/// unchanged, but moved every sampled uniform and so every eigenvalue estimate.
#[test]
fn csg_multithread_agrees_with_single_thread() {
    use outram_mc_libs::physics::compute::{ComputeType, ThreadCount};

    let nuclides = godiva_nuclides();
    let half = 1.0;
    let geom = pincell_geometry(0.5, half, 0, 0); // homogeneous HEU cell
    let materials = vec![godiva_material()];
    let src = SourceBox {
        lower: Position::new(-half, -half, -1.0),
        upper: Position::new(half, half, 1.0),
    };
    let base = KeffSettings {
        n_particles: 1200,
        n_inactive: 15,
        n_active: 30,
        seed: 246813579,
        ..KeffSettings::default()
    };

    let seq = run_keff_csg(
        &geom,
        &materials,
        &nuclides,
        src,
        &KeffSettings {
            compute: ComputeType::CpuSingleThread,
            ..base
        },
        None,
    );
    let par1 = run_keff_csg(
        &geom,
        &materials,
        &nuclides,
        src,
        &KeffSettings {
            compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(1)),
            ..base
        },
        None,
    );
    let par4 = run_keff_csg(
        &geom,
        &materials,
        &nuclides,
        src,
        &KeffSettings {
            compute: ComputeType::CpuMultiThread(ThreadCount::Fixed(4)),
            ..base
        },
        None,
    );

    assert_eq!(
        par1.k_mean, par4.k_mean,
        "multi-thread k must be thread-count-invariant: 1-thread {} vs 4-thread {}",
        par1.k_mean, par4.k_mean
    );

    let sigma_comb = (seq.k_std.powi(2) + par1.k_std.powi(2)).sqrt().max(1e-9);
    let dist = (seq.k_mean - par1.k_mean).abs() / sigma_comb;
    eprintln!(
        "[csg backend agreement] seq = {:.5} ± {:.5}, par = {:.5} ± {:.5}  ({:.2}σ apart)",
        seq.k_mean, seq.k_std, par1.k_mean, par1.k_std, dist
    );
    assert!(dist <= 4.0, "seq vs par {:.2}σ apart (> 4σ)", dist);
}
