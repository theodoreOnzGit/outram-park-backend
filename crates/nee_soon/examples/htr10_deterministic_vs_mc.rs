// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # HTR-10: deterministic (diffusion and SP3) against Monte Carlo, 8 groups
//!
//! Phase 1 of the deterministic programme. The question is narrow and
//! deliberately so: **does the MGXS condensation chain reproduce the Monte
//! Carlo answer when geometry is taken out of the comparison?**
//!
//! ```bash
//! cargo run --release -p nee_soon --example htr10_deterministic_vs_mc
//! ```
//!
//! ## Why an INFINITE MEDIUM, and why that is the right first test
//!
//! The obvious comparison — deterministic `k_eff` against the full-core Monte
//! Carlo `k_eff` — confounds two entirely different things: whether the group
//! constants are right, and whether the deterministic geometry and boundary
//! treatment reproduce the leakage. The HTR-10 is leakage-dominated, so a
//! disagreement would be unattributable.
//!
//! This example removes leakage from **both** ends instead:
//!
//! - ~~the Monte Carlo model is run with `OUTRAM_HTR10_REFLECTIVE` and
//!   `OUTRAM_HTR10_NOREFL`, i.e. an infinite medium of the bed~~ **CORRECTED
//!   2026-09-20** — the code does not do this and has not for some time. An
//!   ordinary *leaky* run is used (vacuum boundary, reflector present) and
//!   `k_inf(MC)` is recovered from its tallies as
//!   `sum nu_Sigma_f phi / sum Sigma_a phi`. Forcing a literal infinite medium
//!   was abandoned for a measured reason recorded at the call site: analog
//!   histories in graphite terminate only by absorption, and it failed to
//!   finish one of two tally passes in 30 minutes;
//! - the deterministic solve uses zero-gradient boundaries on a slab, which is
//!   also zero leakage, giving `k_inf(det)`.
//!
//! Both then describe the **same infinite medium**, from the **same tallied
//! group constants**. Any difference is condensation or solver, and nothing
//! else. Leakage geometry is a separate, later stage; adding it before this
//! passes would mean debugging two things at once.
//!
//! # V&V RESULTS — measured 2026-09-20
//!
//! 8-group WIMS structure, 4000 histories, 4 rings x 4 layers, ENDF/B-VIII.0.
//! Reference: the Monte Carlo tallied `k_inf = 1.143836` (zero leakage), which
//! is the **exact** answer a zero-leakage deterministic solve of the same
//! constants must reproduce.
//!
//! | arm | `k_inf(det)` | vs MC `k_inf` |
//! |---|---|---|
//! | raw condensation | 0.972502 | **-17133 pcm** |
//! | rebalanced ([`nee_soon::mgxs::MgxsLibrary::rebalanced`]) | 1.140305 | **-353 pcm** |
//!
//! **Interpretation.** The -17133 pcm was a *data* defect, not a solver defect:
//! [`nee_soon::genfoam_xs`] never reads the tallied absorption, so the solver's
//! effective absorption is `Sigma_t - Sigma_s,row` — a difference of two large
//! tallied numbers. In graphite that residual is ~5x the absorption itself.
//! Rebalancing the total to `Sigma_a + Sigma_s,row` recovers **+16780 pcm**
//! without fitting anything, and the remaining -353 pcm is discretisation and
//! convergence. Tracked as `op-q6yy`.
//!
//! The diagnosis was predictive, which is what makes it evidence: `k_inf`
//! computed from the *inferred* absorption was 0.955711 (-18813 pcm) against a
//! raw solver result of 0.972502 — right sign, right order, before the fix was
//! written. A small shift would have refuted it.
//!
//! **Buckled arm — a different reference.** `with_buckling` deliberately adds
//! the measured leakage (`0.0307` per history, `B^2 = 5.195e-5 cm^-2`), so that
//! arm predicts `k_eff` and its reference is the **leaky** MC `k_eff =
//! 1.130447`, never `k_inf`. Raw -18106 pcm, rebalanced **-2130 pcm**. The
//! ~2130 pcm residual belongs to the leakage model and is untouched by the
//! condensation fix.
//!
//! **What this does NOT establish.** Both ends are this workspace's own codes,
//! so this is **verification, not validation** — nothing here is compared
//! against a published HTR-10 benchmark, and `k_inf` is not the HTR-10
//! eigenvalue. The MC number is one seed, and its quoted sigma is the
//! statistical error of that seed, not run-to-run scatter.
//!
//! ## Why SP3 must equal diffusion here, and what it tests
//!
//! SP3 reduces to diffusion wherever the flux is near-isotropic, which is
//! exactly what an infinite homogeneous medium is. So the SP3 arm is not
//! expected to improve on diffusion here — it is expected to **agree with it**,
//! and a disagreement would indicate a defect in the SP3 wiring rather than
//! transport physics the diffusion arm is missing. It is a harness check, and
//! it is the only place in this programme where the right answer is known in
//! advance.
//!
//! ## Group structure
//!
//! Eight groups on a WIMS-style thermal-reactor structure rather than a
//! log-uniform grid:
//!
//! ```text
//!   1e-5  0.14  0.625  4.0  29.0  130.0  9.12e3  1.35e6  2e7   [eV]
//! ```
//!
//! The boundaries are placed where the physics is: 0.625 eV is the classical
//! thermal cutoff, 4-130 eV brackets the large U-238 capture resonances
//! (6.67, 20.9, 36.7 eV), and 1.35 MeV is near the U-238 fast-fission
//! threshold. A log-uniform grid would put most of its resolution where
//! nothing happens and none across the resonances, which is what the previous
//! `htr10_mgxs_genfoam` example did above two groups.
//!
//! Every group must carry flux or the bridge refuses the data -- correctly, an
//! unvisited group has no measured cross section.
//!
//! ## What this does NOT establish
//!
//! Not a benchmark comparison: the RMC reference is a leakage-dominated
//! `k_eff` and nothing here is comparable to it. Not a validation -- both ends
//! are this workspace's own codes. `k_inf` agreement says the condensation and
//! the solvers are consistent; it says nothing yet about the HTR-10.

use std::sync::Arc;


use nee_soon::genfoam_xs::to_nuclear_data_input;
use nee_soon::htr10_rmc::core_model::{
    HTR10_AXIAL_REFLECTOR_CM,
    assemble, mat, HTR10_BORED_BORON, HTR10_BORED_CARBON, PAPER_FILLING_FRACTION,
};
use nee_soon::htr10_rmc::reflector::zone_composition;
use nee_soon::mgxs::{condense, matrix_tally, scalar_tally, GroupStructure, MgxsLibrary};
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::{
    DiffusionNeutronics, DiffusionSettings,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::sp3::{Sp3Neutronics, Sp3Settings};
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData;
use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
use outram_foam_basic_lib::prelude::BoundaryCondition;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::thermal::ThermalScattering;
use outram_mc_libs::pebble_beds::htr10::{fuel_pebble_materials, BoronReading, Htr10Nuclides};
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::physics::transport_csg::{run_keff_csg_hybrid, SourceBox};
use uom::si::area::square_meter;
use uom::si::f64::{Area, Length};
use uom::si::length::meter;

const TEMP_K: f64 = 293.6;
const B10_OF_NATURAL: f64 = 0.199;
const NUC: Htr10Nuclides = Htr10Nuclides {
    u235: 0,
    u238: 1,
    o16: 2,
    c_free: 3,
    c_graphite: 4,
    si28: 5,
    b10: 6,
};

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn nuclides() -> Option<Vec<Nuclide>> {
    let dir =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");
    let load = |n: &str, f: &str| Nuclide::from_endf_file(&dir.join(f), n, TEMP_K, 1.0e-3).ok();
    let sab = ThermalScattering::from_endf_file(
        dir.join("tsl-crystalline-graphite.endf").to_str()?,
        30,
        TEMP_K,
        "c_Graphite",
    )
    .ok()?;
    Some(vec![
        load("U235", "n-092_U_235-ENDF8.0.endf")?,
        load("U238", "n-092_U_238.endf")?,
        load("O16", "n-008_O_016-ENDF8.0.endf")?,
        load("C12", "n-006_C_012-ENDF8.0.endf")?,
        load("C12", "n-006_C_012-ENDF8.0.endf")?.with_thermal_scattering(sab),
        load("Si28", "n-014_Si_028-ENDF8.0.endf")?,
        load("B10", "n-005_B_010-ENDF8.0.endf")?,
    ])
}

/// The eleven materials `htr10_rmc`'s geometry indexes, in `mat::` order.
fn materials() -> Vec<Material> {
    let mut mats = fuel_pebble_materials(NUC, BoronReading::Natural, TEMP_K);
    mats.truncate(6); // 0..5: kernel, buffer, iPyC, SiC, oPyC, graphite
    mats.push(Material {
        id: 70,
        name: "helium".into(),
        components: vec![],
        temperature: TEMP_K,
    });
    let graphite_zone = |id: i32, name: &str, z_carbon: f64, z_boron: f64| Material {
        id,
        name: name.into(),
        components: vec![
            NuclideComponent {
                nuclide_idx: NUC.c_graphite,
                atom_density: z_carbon,
            },
            NuclideComponent {
                nuclide_idx: NUC.b10,
                atom_density: z_boron * B10_OF_NATURAL,
            },
        ],
        temperature: TEMP_K,
    };
    let z22 = zone_composition(22).expect("TECDOC zone 22");
    mats.push(graphite_zone(
        71,
        "reflector graphite (TECDOC zone 22)",
        z22.carbon,
        z22.natural_boron,
    ));
    let z17 = zone_composition(17).expect("TECDOC zone 17");
    mats.push(graphite_zone(
        72,
        "boronated carbon brick (TECDOC zone 17)",
        z17.carbon,
        z17.natural_boron,
    ));
    mats.push(graphite_zone(
        73,
        "bored side reflector (TECDOC zones 31-40)",
        HTR10_BORED_CARBON,
        HTR10_BORED_BORON,
    ));
    let dummy = mats[mat::GRAPHITE].clone();
    mats.push(Material {
        id: 74,
        name: "homogenised dummy pebbles".into(),
        components: dummy
            .components
            .iter()
            .map(|c| NuclideComponent {
                nuclide_idx: c.nuclide_idx,
                atom_density: c.atom_density * PAPER_FILLING_FRACTION,
            })
            .collect(),
        temperature: TEMP_K,
    });
    assert_eq!(
        mats.len(),
        mat::HOMOG_DUMMY + 1,
        "material order must match mat::"
    );
    mats
}

fn main() {
    let histories = env_usize("OUTRAM_HTR10_HISTORIES", 2000);
    let rings = env_usize("OUTRAM_HTR10_RINGS", 6);
    let layers = env_usize("OUTRAM_HTR10_LAYERS", 8);
    let n_groups = env_usize("OUTRAM_HTR10_GROUPS", 8);

    // A NORMAL, leakage-terminated run -- vacuum boundary, reflector present.
    //
    // An earlier version of this example forced REFLECTIVE + NOREFL to build a
    // literal infinite medium. That was abandoned for a measured reason: with
    // no leakage, histories in graphite terminate only by absorption, and
    // graphite's Sigma_a/Sigma_s is of order 1e-4, so a single history runs
    // thousands of collisions. It did not finish one of two tally passes in 30
    // minutes on a 3268-tile geometry. Analog Monte Carlo in an infinite
    // moderator is impractical, and brute-forcing it would have bought nothing.
    //
    // It is not needed. `k_inf` is recoverable from the tallies of an ordinary
    // leaky run -- see the comparison below.
    println!("HTR-10: deterministic (diffusion + SP3) vs Monte Carlo, k_inf");
    println!("=============================================================");

    let Some(nucs) = nuclides() else {
        println!("SKIP: reference-data/endf/ not in this checkout.");
        return;
    };
    let mats = materials();
    // OUTRAM_HTR10_SURFACE=1 surface-tracks the bed instead of delta-tracking
    // it, on the SAME geometry. `assemble`'s own docs call this the
    // discriminator: if the reaction-rate balance closes under surface tracking
    // and not under delta tracking, the delta path is scoring virtual
    // collisions as real ones. That is the leading candidate for `op-ra9f`.
    let surface_only = std::env::var("OUTRAM_HTR10_SURFACE").is_ok();
    let core = assemble(rings, layers, if surface_only { usize::MAX } else { 0 });
    if surface_only {
        println!("  TRACKING: surface-only (delta tracking disabled) -- op-ra9f discriminator");
    }
    println!(
        "  geometry: {} tiles, {} cells, {} universes; bed r={:.1} cm, half-height={:.1} cm",
        core.tiles, core.cells, core.universes, core.bed_radius, core.bed_half_height
    );

    // Region-local majorant over the BED materials only: the reflector is
    // surface-tracked and must not raise the bed's delta-tracking cost.
    let grid: Vec<f64> = (0..4096)
        .map(|i| (1.0e-4_f64.ln() + (2.0e7_f64.ln() - 1.0e-4_f64.ln()) * i as f64 / 4095.0).exp())
        .collect();
    let bed_mats: Vec<usize> = (0..=mat::HELIUM).collect();
    let maj = Majorant::over_indices(&mats, &bed_mats, &nucs, &grid, 0.3);

    let settings = KeffSettings {
        n_particles: histories,
        n_inactive: 20,
        n_active: 40,
        seed: 20_260_919,
        ..KeffSettings::default()
    };
    let src = SourceBox {
        lower: Position::new(-core.bed_radius, -core.bed_radius, core.conus_floor),
        upper: Position::new(core.bed_radius, core.bed_radius, core.bed_half_height),
    };

    // A thermal reactor populates the whole range, so a coarse structure with
    // a thermal/fast split is used rather than a fine log grid -- every group
    // must carry flux or the bridge refuses it (and rightly: an unvisited group
    // has no measured cross section).
    // Boundaries placed where the physics is, not on a log grid. 0.625 eV is
    // the classical thermal cutoff; 4-130 eV brackets the large U-238 capture
    // resonances (6.67, 20.9, 36.7 eV); 1.35 MeV is near the U-238 fast-fission
    // threshold. A log-uniform grid spends its resolution where nothing happens
    // and puts no boundary across the resonances at all.
    const WIMS8: [f64; 9] = [
        1.0e-5, 0.14, 0.625, 4.0, 29.0, 130.0, 9.12e3, 1.35e6, 2.0e7,
    ];
    let edges: Vec<f64> = match n_groups {
        2 => vec![1.0e-5, 2.38, 2.0e7],
        8 => WIMS8.to_vec(),
        n => {
            // Any other count falls back to log-uniform, and says so: the
            // placed structure above only exists for 8.
            println!("  NOTE: {n} groups requested -- no placed structure for that count,");
            println!("        falling back to a log-uniform grid. Use 8 for the WIMS-style one.");
            let (l0, l1) = (1.0e-5_f64.ln(), 2.0e7_f64.ln());
            (0..=n).map(|i| (l0 + (l1 - l0) * i as f64 / n as f64).exp()).collect()
        }
    };
    let groups = GroupStructure::new(edges).expect("ascending edges");
    let names: Vec<String> = mats.iter().map(|m| m.name.clone()).collect();
    let all: Vec<usize> = (0..mats.len()).collect();

    println!("\n  Monte Carlo pass 1/2 (scalar reaction rates)...");
    let mut scalar = scalar_tally(1, &groups, all.clone());
    let mc = run_keff_csg_hybrid(
        &core.geometry,
        &mats,
        &nucs,
        &[maj.clone()],
        None,
        src,
        &settings,
        Some(&mut scalar),
    );
    println!("  Monte Carlo pass 2/2 (scatter matrix + fission spectrum)...");
    let mut matrix = matrix_tally(2, &groups, all);
    run_keff_csg_hybrid(
        &core.geometry,
        &mats,
        &nucs,
        &[maj],
        None,
        src,
        &settings,
        Some(&mut matrix),
    );

    println!(
        "\n  Monte Carlo k_eff = {:.6} +/- {:.6}",
        mc.k_mean, mc.k_std
    );

    let lib = match condense(&groups, &names, &scalar, &matrix, settings.n_active as u64) {
        Ok(l) => l,
        Err(e) => {
            println!("  condensation failed: {e}");
            return;
        }
    };

    println!(
        "\n  condensed {} zones x {} groups:",
        lib.zones.len(),
        groups.n_groups()
    );
    for z in &lib.zones {
        let visited = z.flux.iter().filter(|p| **p > 0.0).count();
        println!(
            "    {:<44} flux {:>10.3e}  groups visited {}/{}",
            z.name,
            z.flux.iter().sum::<f64>(),
            visited,
            groups.n_groups()
        );
    }

    let sys_all = lib.clone();
    // TWO homogeneous media, one per PHYSICAL REGION -- not one for everything.
    //
    // Collapsing the whole library into a single medium smears the reflector
    // graphite and the boronated carbon bricks into the fuel, where they become
    // a pure parasitic absorber with no fission and no spatial separation. In
    // the real core those regions are separate and neutrons that leak into them
    // come BACK. Measured at 2 groups, the single-medium model sat ~27000 pcm
    // below the Monte Carlo eigenvalue AFTER leakage (450 pcm), condensation
    // balance (0.4 %) and the solver (SP3 = diffusion to 0 pcm) had each been
    // excluded as the cause -- so the smear is what is left.
    //
    // mat:: indices: 0..=5 pebble layers, 6 helium, 10 dummy pebbles -> BED;
    // 7 reflector graphite, 8 boronated brick, 9 bored side band -> REFLECTOR.
    let bed_idx: Vec<usize> = vec![0, 1, 2, 3, 4, 5, mat::HELIUM, mat::HOMOG_DUMMY];
    let refl_idx: Vec<usize> = vec![mat::REFLECTOR, 8, 9];
    let bed_zone = sys_all.homogenised_subset(&bed_idx, "bed (homogenised)");
    let refl_zone = sys_all.homogenised_subset(&refl_idx, "reflector (homogenised)");
    println!(
        "\n  two-region model: bed k_inf = {:.6}, reflector k_inf = {:.6}",
        bed_zone.k_inf(),
        refl_zone.k_inf()
    );
    println!("  (a k_inf ~ 1.48 fuel region averaged with a k_inf = 0 absorber is");
    println!("   what a single-medium model actually solves -- hence the gap below)");

    // The SPATIAL model: two zones side by side on one mesh, rather than one
    // medium that is the average of both.
    let two_zone = MgxsLibrary {
        groups: bed_zone.groups.clone(),
        zones: vec![bed_zone.zones[0].clone(), refl_zone.zones[0].clone()],
    };

    // ONE homogeneous medium, because k_inf is a WHOLE-SYSTEM quantity.
    // `condense` gives one zone per material, and no single material's k_inf is
    // the core's -- the kernel alone is wildly supercritical, the graphite
    // alone is subcritical. `homogenised` flux-weights them into the medium the
    // Monte Carlo number actually describes.
    let sys_all = lib.clone();
    let sys = lib.homogenised("HTR-10 core, homogenised");

    // THE MONTE CARLO ANSWER, from the same tallies.
    //
    // `Sigma_x,g = R_x,g / phi_g`, so `Sigma_x,g * phi_g` recovers the raw
    // tallied reaction rate and `k_inf = sum nu_Sigma_f phi / sum Sigma_a phi`
    // is exactly the Monte Carlo production/absorption ratio. No separate run
    // is needed, and no infinite-medium geometry.
    let k_inf_mc = sys.k_inf();

    // GIVE THE DETERMINISTIC SOLVE THE MONTE CARLO'S OWN LEAKAGE.
    //
    // The constants above were condensed under a LEAKY full-core spectrum. A
    // zero-leakage solve would re-derive an infinite-medium spectrum instead,
    // which is a different problem -- measured at -25578 pcm on this model, and
    // a mismatch of questions rather than a defect in either code.
    //
    // `B^2` is solved from the leakage the Monte Carlo actually MEASURED, never
    // searched for a value that makes the eigenvalues agree. Searching it would
    // turn this comparison into a fit and destroy the check.
    let leak_frac = if mc.histories > 0 {
        (mc.leak_vacuum + mc.leak_infinity) as f64 / mc.histories as f64
    } else {
        0.0
    };
    let b2 = sys.buckling_from_leakage(leak_frac);
    let solved = match b2 {
        Some(b) => {
            println!(
                "\n  measured leakage {:.4} per history -> B^2 = {:.6e} cm^-2",
                leak_frac, b
            );
            sys.with_buckling(b)
        }
        None => {
            println!("\n  NOTE: no leakage measured, solving zero-leakage (k_inf, not k_eff).");
            sys.clone()
        }
    };

    // BALANCE CHECK -- the decisive diagnostic for a solver/library mismatch.
    //
    // The bridge builds removal as `Sigma_t - Sigma_s,g->g`, which is only
    // correct if `Sigma_t = Sigma_a + Sigma_s,total`. Those come from two
    // SEPARATE Monte Carlo passes and nothing enforces the identity. If the
    // matrix pass undercounts scattering, removal is too large and the
    // eigenvalue too LOW, while `Sigma_a` -- and hence the library `k_inf` --
    // is untouched. That is precisely the signature of a solver disagreeing
    // with the algebraic k_inf of its own input.
    println!("\n  balance: Sigma_t vs Sigma_a + sum_g' Sigma_s,g->g'");
    for (name, rows) in sys.balance_check() {
        let worst = rows
            .iter()
            .map(|(_, _, r)| r.abs())
            .fold(0.0_f64, f64::max);
        println!("    {name:<38} worst |discrepancy| = {:.1} %", 100.0 * worst);
        for (g, (t, rebuilt, rel)) in rows.iter().enumerate() {
            println!(
                "      g{g}: Sigma_t {t:.5e}   Sigma_a+Sigma_s {rebuilt:.5e}   {:+.1} %",
                100.0 * rel
            );
        }
    }

    let desc = solved.in_descending_energy();
    let input = match to_nuclear_data_input(&desc) {
        Ok(i) => i,
        Err(e) => {
            println!("\n  GeN-Foam bridge refused the data:\n    {e}");
            println!("\n  That is the correct outcome, not a crash: a group no neutron visited");
            println!("  has no measured cross section, and zeros make its diffusion equation");
            println!("  singular. Raise OUTRAM_HTR10_HISTORIES or coarsen OUTRAM_HTR10_GROUPS.");
            return;
        }
    };

    let xs = match CrossSectionData::from_input(&input) {
        Ok(x) => x,
        Err(e) => {
            println!("  GeN-Foam rejected the nuclearData input: {e}");
            return;
        }
    };
    println!(
        "\n  GeN-Foam accepted the MGXS: {} groups, {} zones",
        xs.energy_groups(),
        xs.zone_count()
    );

    // ---- ABLATION ARM: the same data, rebalanced ----
    //
    // `Sigma_t,g := Sigma_a,g + sum_g' Sigma_s,g->g'`. Nothing is fitted: the
    // tallied absorption and the tallied scattering matrix are both kept
    // exactly, and the total is set to the sum they imply. See
    // `MgxsLibrary::rebalanced` for why the total is the right place to put the
    // residual and the absorption is the worst place.
    let solved_rb = solved.rebalanced();
    // The unbuckled pair isolates the CONDENSATION from the leakage model: a
    // zero-leakage solve of these has an exact expected answer, `k_inf_mc`.
    let sys_rb = sys.rebalanced();
    let xs_rb = to_nuclear_data_input(&solved_rb.in_descending_energy())
        .ok()
        .and_then(|i| CrossSectionData::from_input(&i).ok());
    if xs_rb.is_none() {
        println!("  (rebalanced arm unavailable: the bridge refused the rebalanced data)");
    }

    // Zero-leakage slab: every cell in zone 0, zero-gradient on both faces.
    // This is the deterministic half of the infinite-medium comparison -- the
    // Monte Carlo half is the REFLECTIVE + NOREFL configuration set in main().
    let mesh = Arc::new(
        create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), 4)
            .expect("slab mesh"),
    );
    let zone_of_cell = vec![0usize; mesh.n_cells];
    let bc = vec![
        BoundaryCondition::ZeroGradient,
        BoundaryCondition::ZeroGradient,
    ];

    let sig_mc = mc.k_std;
    let pcm = |a: f64, b: f64| (a - b) * 1.0e5;

    println!("\n=== k_inf: Monte Carlo tallies vs deterministic solvers ===\n");
    println!("  (the leaky Monte Carlo k_eff for this geometry was {:.6}; it is NOT", mc.k_mean);
    println!("   the comparison target -- the deterministic solve below has no leakage)\n");
    // ---- WHERE DOES THE ABSORPTION THE SOLVER SEES COME FROM? ----
    //
    // A zero-leakage homogeneous solve has an exact answer: the algebraic
    // k_inf of its own input. When it does not reproduce that, the fault is in
    // the data handed to it, and this block says so with numbers rather than
    // by elimination. The bridge never reads the tallied `absorption`; it
    // infers absorption as Sigma_t - (scatter row sum). Those are two large,
    // separately tallied numbers whose DIFFERENCE is the small quantity that
    // sets k.
    {
        let k_tallied = sys.k_inf();
        let k_inferred = sys.k_inf_inferred_absorption();
        println!("\n  --- absorption: tallied vs what the bridge infers ---");
        println!(
            "  k_inf from TALLIED absorption               = {k_tallied:.6}   (the exact answer)"
        );
        println!(
            "  k_inf from INFERRED Sigma_t - Sigma_s,row   = {k_inferred:.6}   ({:+.0} pcm)",
            (k_inferred - k_tallied) * 1.0e5
        );
        for z in &sys.zones {
            for g in 0..z.flux.len() {
                let tal = z.absorption[g];
                let inf = z.inferred_absorption(g).unwrap_or(0.0);
                if z.flux[g] <= 0.0 {
                    continue;
                }
                println!(
                    "    {:<10} g{g}  tallied Sigma_a = {tal:.6e}   inferred = {inf:.6e}   ratio {:.2}x",
                    z.name,
                    if tal.abs() > 0.0 { inf / tal } else { f64::NAN }
                );
            }
        }
        println!("  A ratio far from 1.00 means the solver is absorbing with a number");
        println!("  that is mostly condensation residual, not physics.");
    }

    println!("  Monte Carlo tallies, production/absorption   k_inf = {k_inf_mc:.6}");

    // ---- BALANCE CHECK: is the tallied k_inf consistent with the eigenvalue? ----
    //
    // In a k-eigenvalue run every source neutron is either absorbed or leaks,
    // so A + L = 1 and P = k_eff. The infinite-medium value the run ITSELF
    // implies is therefore k_inf = k_eff / (1 - L). If the condensed tallies do
    // not reproduce that, the condensation is not a faithful summary of the
    // transport, and no agreement measured against it can be read as agreement
    // with the Monte Carlo.
    //
    // This check exists because that failure went unnoticed: `k_eff > k_inf`
    // was printed for two configurations -- physically impossible, a leaking
    // system cannot exceed its own infinite-medium multiplication -- and was
    // read as a curiosity rather than a defect. Tracked as `op-ra9f`.
    {
        let implied = if leak_frac < 1.0 {
            mc.k_mean / (1.0 - leak_frac)
        } else {
            f64::NAN
        };
        let err = (k_inf_mc - implied) * 1.0e5;
        println!(
            "  balance: k_eff/(1-L) = {implied:.6} implies k_inf; tallied is {k_inf_mc:.6} ({err:+.0} pcm)"
        );
        if k_inf_mc < mc.k_mean {
            println!(
                "  *** IMPOSSIBLE: tallied k_inf < k_eff. A leaking system cannot exceed its"
            );
            println!("  *** own infinite-medium multiplication. The condensation is wrong (op-ra9f),");
            println!("  *** and nothing below may be quoted as agreement with the Monte Carlo.");
        } else if err.abs() > 1000.0 {
            println!("  *** WARNING: condensation and eigenvalue disagree by >1000 pcm (op-ra9f).");
        }
    }
    println!("  Monte Carlo eigenvalue (leaky, the target)   k_eff = {:.6} +/- {:.6}",
             mc.k_mean, mc.k_std);
    // WHICH REFERENCE GOES WITH WHICH ARM -- this has been got wrong twice.
    //
    // `solved` is `sys.with_buckling(b)`: leakage has been DELIBERATELY added
    // to the library, so its eigenvalue is a predicted **k_eff**, and its
    // reference is the leaky Monte Carlo `k_eff`. A zero-leakage (unbuckled)
    // library's eigenvalue is a **k_inf**, and its reference is the tallied
    // `k_inf`. Pairing a buckled solve with `k_inf` -- or an unbuckled solve
    // with `k_eff` -- is worth exactly the leakage, in whichever direction
    // flatters or penalises by accident.
    //
    // The arms below are labelled with the reference each uses. Do not
    // "simplify" them onto one reference.
    let k_mc = mc.k_mean;

    let k_diff = match DiffusionNeutronics::new(
        mesh.clone(),
        &xs,
        &zone_of_cell,
        &[],
        &bc,
        DiffusionSettings::default(),
    ) {
        Ok(mut model) => match model.solve_eigenvalue() {
            Ok(r) => {
                println!(
                    "  GeN-Foam diffusion                      k_inf = {:.6}   ({} outers)",
                    r.k_eff, r.outer_iterations
                );
                Some(r.k_eff)
            }
            Err(e) => {
                println!("  diffusion eigenvalue solve failed: {e}");
                None
            }
        },
        Err(e) => {
            println!("  could not build the diffusion model: {e}");
            None
        }
    };

    let k_sp3 = match Sp3Neutronics::with_cross_sections(
        mesh.clone(),
        &xs,
        &zone_of_cell,
        &[],
        &bc,
        Sp3Settings::default(),
    ) {
        Ok(mut model) => match model.solve_eigenvalue() {
            Ok(r) => {
                println!(
                    "  GeN-Foam SP3                            k_inf = {:.6}   ({} outers)",
                    r.k_eff, r.outer_iterations
                );
                Some(r.k_eff)
            }
            Err(e) => {
                println!("  SP3 eigenvalue solve failed: {e}");
                None
            }
        },
        Err(e) => {
            println!("  could not build the SP3 model: {e}");
            None
        }
    };

    println!("\n  differences [pcm]:");
    if let Some(kd) = k_diff {
        println!(
            "    diffusion - MC = {:+8.0}   ({:.1} MC sigma)",
            pcm(kd, k_mc),
            (kd - k_mc).abs() / sig_mc.max(1.0e-9)
        );
    }
    if let Some(ks) = k_sp3 {
        println!(
            "    SP3       - MC = {:+8.0}   ({:.1} MC sigma)",
            pcm(ks, k_mc),
            (ks - k_mc).abs() / sig_mc.max(1.0e-9)
        );
    }
    if let (Some(kd), Some(ks)) = (k_diff, k_sp3) {
        let d = pcm(ks, kd);
        println!("    SP3 - diffusion = {d:+7.0}");
        println!();
        if d.abs() < 10.0 {
            println!("  SP3 reproduces diffusion to {:.0} pcm, as it must in an isotropic", d.abs());
            println!("  medium. That is a HARNESS CHECK on the SP3 wiring, not evidence that");
            println!("  SP3 adds anything here -- it cannot, and is not expected to.");
        } else {
            println!("  WARNING: SP3 and diffusion differ by {d:+.0} pcm in an INFINITE HOMOGENEOUS");
            println!("  MEDIUM, where the flux is isotropic and SP3 must reduce to diffusion.");
            println!("  That is a defect in the SP3 wiring or the second-moment boundary");
            println!("  treatment, NOT transport physics diffusion is missing. Do not read the");
            println!("  difference as an SP3 correction.");
        }
    }

    // ---- ABLATION: what is the absorption rebalancing worth? ----
    println!("\n=== ablation: raw condensation vs rebalanced condensation ===");
    println!("  (single medium, zero leakage -- the exact answer is MC k_inf = {k_inf_mc:.6})");
    if let Some(ref xsr) = xs_rb {
        println!(
            "  rebalanced library algebraic check: k_inf {:.6} vs inferred-absorption k_inf {:.6}",
            solved_rb.k_inf(),
            solved_rb.k_inf_inferred_absorption()
        );
        for (name, built) in [
            (
                "diffusion",
                DiffusionNeutronics::new(
                    mesh.clone(),
                    xsr,
                    &zone_of_cell,
                    &[],
                    &bc,
                    DiffusionSettings::default(),
                )
                .ok()
                .and_then(|mut m| m.solve_eigenvalue().ok().map(|r| r.k_eff)),
            ),
            (
                "SP3",
                Sp3Neutronics::with_cross_sections(
                    mesh.clone(),
                    xsr,
                    &zone_of_cell,
                    &[],
                    &bc,
                    Sp3Settings::default(),
                )
                .ok()
                .and_then(|mut m| m.solve_eigenvalue().ok().map(|r| r.k_eff)),
            ),
        ] {
            match built {
                Some(k) => println!(
                    "  {name:<10} REBALANCED  k_inf = {k:.6}  ({:+.0} pcm vs MC k_inf)",
                    pcm(k, k_inf_mc)
                ),
                None => println!("  {name:<10} REBALANCED  solve unavailable"),
            }
        }
        // ---- the zero-leakage pair: an EXACT expected answer ----
        println!("\n  zero-leakage arms (no buckling) -- exact answer is MC k_inf = {k_inf_mc:.6}");
        for (label, l) in [("RAW       ", &sys), ("REBALANCED", &sys_rb)] {
            let built = to_nuclear_data_input(&l.in_descending_energy())
                .ok()
                .and_then(|i| CrossSectionData::from_input(&i).ok())
                .and_then(|x| {
                    DiffusionNeutronics::new(
                        mesh.clone(),
                        &x,
                        &zone_of_cell,
                        &[],
                        &bc,
                        DiffusionSettings::default(),
                    )
                    .ok()
                    .and_then(|mut m| m.solve_eigenvalue().ok().map(|r| r.k_eff))
                });
            match built {
                Some(k) => println!(
                    "  diffusion, no buckling, {label}  k_inf = {k:.6}  ({:+.0} pcm vs MC k_inf; library's own algebraic k_inf {:.6})",
                    pcm(k, k_inf_mc),
                    l.k_inf_inferred_absorption()
                ),
                None => println!("  diffusion, no buckling, {label}  unavailable"),
            }
        }

        println!("\n  Compare against the raw-condensation lines above. The shift between");
        println!("  them is what the Sigma_t - Sigma_s,row absorption inference was costing;");
        println!("  it is an ablation, so a small shift would refute the diagnosis.");
    }

    // ---- SPATIAL two-zone solve: bed cells then reflector cells ----
    //
    // The reference on the other side of this comparison is `k_inf`, which is a
    // ZERO-LEAKAGE quantity. So the physics-matched boundary condition here is
    // reflective on BOTH faces. An earlier version of this block put a vacuum
    // face on the outer edge and reported the result as the two-zone answer;
    // that was wrong -- it added leakage the reference does not have, and the
    // resulting -22177 pcm was mostly an artefact of the mismatch rather than a
    // property of the model. Both arms are run below so the size of that
    // artefact is visible rather than asserted.
    println!("\n=== spatial two-zone solve (bed | reflector on one mesh) ===");
    // Rebalance here too. The zero-leakage ablation above measures this repair
    // as worth +16780 pcm on the homogenised medium; leaving the two-zone arm
    // on the raw condensation would compare a repaired model against an
    // unrepaired one and read the difference as geometry.
    let two_zone_rb = two_zone.rebalanced();
    let desc2 = two_zone_rb.in_descending_energy();
    match to_nuclear_data_input(&desc2) {
        Ok(input2) => match CrossSectionData::from_input(&input2) {
            Ok(xs2) => {
                // 190 cm total: bed to 90 cm, reflector 90 -> 190 cm.
                const N: usize = 38;
                let mesh2 = Arc::new(
                    create_one_d_mesh(
                        Length::new::<meter>(1.90),
                        Area::new::<square_meter>(1.0),
                        N as i64,
                    )
                    .expect("two-zone mesh"),
                );
                let zone2: Vec<usize> =
                    (0..N).map(|c| if c * 5 < 90 { 0 } else { 1 }).collect();

                // The comparison that matches the reference: zero leakage.
                let bc_refl = vec![
                    BoundaryCondition::ZeroGradient,
                    BoundaryCondition::ZeroGradient,
                ];
                // Kept only to show how much the wrong boundary was worth.
                let bc_vac = vec![
                    BoundaryCondition::ZeroGradient,
                    BoundaryCondition::FixedValue(0.0),
                ];

                let solve_pair = |label: &str, bc: &Vec<BoundaryCondition<f64>>| {
                    match DiffusionNeutronics::new(
                        mesh2.clone(),
                        &xs2,
                        &zone2,
                        &[],
                        bc,
                        DiffusionSettings::default(),
                    ) {
                        Ok(mut m) => match m.solve_eigenvalue() {
                            Ok(r) => println!(
                                "  diffusion, two zones, {label:<18} k_eff = {:.6}  ({:+.0} pcm vs MC k_inf)",
                                r.k_eff,
                                (r.k_eff - k_inf_mc) * 1.0e5
                            ),
                            Err(e) => println!("  two-zone diffusion solve failed ({label}): {e}"),
                        },
                        Err(e) => println!("  could not build the two-zone model ({label}): {e}"),
                    }
                    match Sp3Neutronics::with_cross_sections(
                        mesh2.clone(),
                        &xs2,
                        &zone2,
                        &[],
                        bc,
                        Sp3Settings::default(),
                    ) {
                        Ok(mut m) => match m.solve_eigenvalue() {
                            Ok(r) => println!(
                                "  SP3,       two zones, {label:<18} k_eff = {:.6}  ({:+.0} pcm vs MC k_inf)",
                                r.k_eff,
                                (r.k_eff - k_inf_mc) * 1.0e5
                            ),
                            Err(e) => println!("  two-zone SP3 solve failed ({label}): {e}"),
                        },
                        Err(e) => println!("  could not build the two-zone SP3 model ({label}): {e}"),
                    }
                };

                solve_pair("reflective (QUOTE)", &bc_refl);
                solve_pair("vacuum face", &bc_vac);

                // ---- VOLUME-MATCHED SLAB ----
                //
                // The 190 cm slab above gives the reflector 100 cm against the
                // bed's 90 -- a reflector:bed volume ratio of 1.11. The r-z
                // model it stands in for has the bed at r <= 90 cm and the
                // reflector out to r = 190 cm, i.e. (190^2 - 90^2)/90^2 =
                // 3.457. The slab therefore carries barely a THIRD of the
                // reflector it should, per unit fuel.
                //
                // PREDICTION, stated before the measurement: too little
                // reflector means too little reflector absorption, so the slab
                // eigenvalue should be too HIGH, and adding the missing
                // reflector should bring it DOWN toward the Monte Carlo k_inf.
                // If it moves up, or barely moves, this explanation is wrong.
                //
                // 400 cm at 5 cm/cell: 18 cells of bed (90 cm), 62 of
                // reflector (310 cm), ratio 3.444 against the target 3.457.
                // RETRACTION, measured: the volume-matched arm below moved the
                // eigenvalue by only ~86 pcm, so the "too little reflector
                // VOLUME" explanation is WRONG and is withdrawn. The reason is
                // that flux in graphite decays as exp(-x/L) with L ~ 50 cm, so
                // 100 cm is already ~2 L and the integral is 43.2 of a possible
                // 50: additional reflector is optically dark and absorbs almost
                // nothing. What governs is reflector THICKNESS IN DIFFUSION
                // LENGTHS, not volume. The arm is kept because a refuted
                // prediction that was actually run is worth more than a tidy
                // story, and because it bounds the volume effect at ~86 pcm.
                const NV: usize = 80;
                let mesh_v = Arc::new(
                    create_one_d_mesh(
                        Length::new::<meter>(4.00),
                        Area::new::<square_meter>(1.0),
                        NV as i64,
                    )
                    .expect("volume-matched mesh"),
                );
                let zone_v: Vec<usize> =
                    (0..NV).map(|c| if c * 5 < 90 { 0 } else { 1 }).collect();
                println!(
                    "  volume-matched slab: reflector:bed = {:.3} (target {:.3} from r-z)",
                    (NV - 18) as f64 / 18.0,
                    (190.0_f64.powi(2) - 90.0_f64.powi(2)) / 90.0_f64.powi(2)
                );
                match DiffusionNeutronics::new(
                    mesh_v.clone(),
                    &xs2,
                    &zone_v,
                    &[],
                    &bc_refl,
                    DiffusionSettings::default(),
                ) {
                    Ok(mut m) => match m.solve_eigenvalue() {
                        Ok(r) => println!(
                            "  diffusion, VOLUME-MATCHED, reflective  k_eff = {:.6}  ({:+.0} pcm vs MC k_inf)",
                            r.k_eff,
                            (r.k_eff - k_inf_mc) * 1.0e5
                        ),
                        Err(e) => println!("  volume-matched diffusion failed: {e}"),
                    },
                    Err(e) => println!("  volume-matched build failed: {e}"),
                }
                match Sp3Neutronics::with_cross_sections(
                    mesh_v,
                    &xs2,
                    &zone_v,
                    &[],
                    &bc_refl,
                    Sp3Settings::default(),
                ) {
                    Ok(mut m) => match m.solve_eigenvalue() {
                        Ok(r) => println!(
                            "  SP3,       VOLUME-MATCHED, reflective  k_eff = {:.6}  ({:+.0} pcm vs MC k_inf)",
                            r.k_eff,
                            (r.k_eff - k_inf_mc) * 1.0e5
                        ),
                        Err(e) => println!("  volume-matched SP3 failed: {e}"),
                    },
                    Err(e) => println!("  volume-matched SP3 build failed: {e}"),
                }

                // ---- AXIAL SLAB: the shape the Monte Carlo geometry ACTUALLY has ----
                //
                // The bed in this run is r = 90 cm by half-height
                // `core.bed_half_height` -- a squat DISC, not a cube. The slabs
                // above gave it a 90 cm half-thickness, which for a bed only a
                // few cm tall is an order of magnitude too thick and
                // under-exposes the fuel to the reflector enormously.
                //
                // A 1-D slab can represent the AXIAL direction of that disc:
                // half-height of bed, then reflector, symmetry at the midplane.
                //
                // PREDICTION, before measuring: a far thinner bed means far
                // more reflector interaction per unit fuel, hence more
                // reflector absorption, so k should come DOWN substantially
                // from ~1.30 toward the Monte Carlo k_inf. If it stays near
                // 1.30, thickness is not the explanation either.
                {
                    let half = core.bed_half_height;
                    let refl_cm = 100.0_f64;
                    let total_cm = half + refl_cm;
                    let na = 100usize;
                    let cell = total_cm / na as f64;
                    let mesh_a = Arc::new(
                        create_one_d_mesh(
                            Length::new::<meter>(total_cm / 100.0),
                            Area::new::<square_meter>(1.0),
                            na as i64,
                        )
                        .expect("axial mesh"),
                    );
                    let zone_a: Vec<usize> = (0..na)
                        .map(|c| usize::from((c as f64 + 0.5) * cell >= half))
                        .collect();
                    let n_bed = zone_a.iter().filter(|&&z| z == 0).count();
                    println!(
                        "  axial slab: bed half-height {half:.2} cm ({n_bed} cells), reflector {refl_cm:.0} cm"
                    );
                    match DiffusionNeutronics::new(
                        mesh_a.clone(),
                        &xs2,
                        &zone_a,
                        &[],
                        &bc_refl,
                        DiffusionSettings::default(),
                    ) {
                        Ok(mut m) => match m.solve_eigenvalue() {
                            Ok(r) => println!(
                                "  diffusion, AXIAL disc slab, reflective k_eff = {:.6}  ({:+.0} pcm vs MC k_inf)",
                                r.k_eff,
                                (r.k_eff - k_inf_mc) * 1.0e5
                            ),
                            Err(e) => println!("  axial diffusion failed: {e}"),
                        },
                        Err(e) => println!("  axial build failed: {e}"),
                    }
                    match Sp3Neutronics::with_cross_sections(
                        mesh_a,
                        &xs2,
                        &zone_a,
                        &[],
                        &bc_refl,
                        Sp3Settings::default(),
                    ) {
                        Ok(mut m) => match m.solve_eigenvalue() {
                            Ok(r) => println!(
                                "  SP3,       AXIAL disc slab, reflective k_eff = {:.6}  ({:+.0} pcm vs MC k_inf)",
                                r.k_eff,
                                (r.k_eff - k_inf_mc) * 1.0e5
                            ),
                            Err(e) => println!("  axial SP3 failed: {e}"),
                        },
                        Err(e) => println!("  axial SP3 build failed: {e}"),
                    }
                }

                // ================= LEAKY MODEL -- REAL boundary conditions ========
                //
                // k_eff against k_eff. No k_inf anywhere in this block: the
                // HTR-10 leaks, RMC models it leaking, and a zero-leakage
                // comparison answers a question nobody asked.
                //
                // The mesh is RADIAL, 0 -> 190 cm, with the reflector's real
                // material layering rather than one smeared "reflector":
                //
                //     0      - 90      bed
                //     90     - 95.6    reflector graphite
                //     95.6   - 108.6   bored control-rod band
                //     108.6  - 140.6   reflector graphite
                //     140.6  - 148.6   cold helium annulus
                //     148.6  - 167.793 reflector graphite
                //     167.793- 190     boronated carbon bricks   <- the absorber
                //
                // Zero-gradient at r = 0 (symmetry) and VACUUM at r = 190 cm.
                // The radial direction is chosen for the explicit mesh because
                // it carries all the material structure and contains no void;
                // the axial direction has a ~99 cm void cavity that 1-D
                // diffusion cannot represent, so it is treated as a buckling.
                //
                // KNOWN GEOMETRIC APPROXIMATION: `create_one_d_mesh` is a
                // CARTESIAN slab, so cell volumes do not carry the cylindrical
                // r-weighting. Outer zones are therefore under-weighted
                // relative to a true r-z model. This is stated, not corrected.
                println!("\n=== LEAKY model: k_eff vs k_eff, real boundary conditions ===");
                {
                    let g_zone = sys_all.homogenised_subset(&[mat::REFLECTOR], "reflector graphite");
                    let bored = sys_all.homogenised_subset(&[9usize], "bored band");
                    let boronated = sys_all.homogenised_subset(&[8usize], "boronated carbon");
                    let helium = sys_all.homogenised_subset(&[mat::HELIUM], "coolant helium");
                    let radial = MgxsLibrary {
                        groups: sys_all.groups.clone(),
                        // Each `homogenised_subset` is a one-zone library;
                        // take that zone so they compose into one mesh.
                        zones: [&bed_zone, &g_zone, &bored, &boronated, &helium]
                            .iter()
                            .filter_map(|l| l.zones.first().cloned())
                            .collect(),
                    }
                    .rebalanced();

                    // 1 cm cells out to the 190 cm vacuum boundary.
                    const NR: usize = 190;
                    let zone_r: Vec<usize> = (0..NR)
                        .map(|c| {
                            let r = c as f64 + 0.5;
                            if r < 90.0 {
                                0
                            } else if r < 95.6 {
                                1
                            } else if r < 108.6 {
                                2
                            } else if r < 140.6 {
                                1
                            } else if r < 148.6 {
                                4
                            } else if r < 167.793 {
                                1
                            } else {
                                3
                            }
                        })
                        .collect();
                    let bc_r = vec![
                        BoundaryCondition::ZeroGradient,
                        BoundaryCondition::FixedValue(0.0),
                    ];

                    // Axial leakage as a buckling. Reported BOTH ways so the
                    // reader sees the bracket rather than one number:
                    //   * none  -> infinite cylinder, an UPPER bound on k;
                    //   * derived -> B_z^2 = (pi / (H + 2 delta_z))^2 with the
                    //     axial graphite reflector's own saving.
                    let thermal_g = radial.groups.n_groups().saturating_sub(1);
                    let delta_z = radial
                        .reflector_savings(0, 1, thermal_g, HTR10_AXIAL_REFLECTOR_CM)
                        .unwrap_or(0.0);
                    let h_eff = 2.0 * core.bed_half_height + 2.0 * delta_z;
                    let b2_z = (std::f64::consts::PI / h_eff).powi(2);
                    println!(
                        "  axial saving delta_z = {delta_z:.2} cm -> H_eff = {h_eff:.2} cm, B_z^2 = {b2_z:.6e} cm^-2"
                    );
                    println!("  radial mesh: {NR} cells, 0 -> 190 cm, vacuum at the outer face");

                    for (label, lib) in [
                        ("no axial leakage ", radial.clone()),
                        ("axial B_z^2      ", radial.with_buckling(b2_z)),
                    ] {
                        let Some(xs_r) = to_nuclear_data_input(&lib.in_descending_energy())
                            .ok()
                            .and_then(|i| CrossSectionData::from_input(&i).ok())
                        else {
                            println!("  {label} bridge refused the library");
                            continue;
                        };
                        let mesh_r = Arc::new(
                            create_one_d_mesh(
                                Length::new::<meter>(1.90),
                                Area::new::<square_meter>(1.0),
                                NR as i64,
                            )
                            .expect("radial mesh"),
                        );
                        for (solver, k) in [
                            (
                                "diffusion",
                                DiffusionNeutronics::new(
                                    mesh_r.clone(),
                                    &xs_r,
                                    &zone_r,
                                    &[],
                                    &bc_r,
                                    DiffusionSettings::default(),
                                )
                                .ok()
                                .and_then(|mut m| m.solve_eigenvalue().ok().map(|r| r.k_eff)),
                            ),
                            (
                                "SP3      ",
                                Sp3Neutronics::with_cross_sections(
                                    mesh_r.clone(),
                                    &xs_r,
                                    &zone_r,
                                    &[],
                                    &bc_r,
                                    Sp3Settings::default(),
                                )
                                .ok()
                                .and_then(|mut m| m.solve_eigenvalue().ok().map(|r| r.k_eff)),
                            ),
                        ] {
                            match k {
                                Some(k) => println!(
                                    "  {solver} {label} k_eff = {k:.6}   ({:+.0} pcm vs MC k_eff {:.6})",
                                    (k - mc.k_mean) * 1.0e5,
                                    mc.k_mean
                                ),
                                None => println!("  {solver} {label} solve unavailable"),
                            }
                        }
                    }
                    println!("  Both arms use the REBALANCED condensation (op-q6yy).");
                    println!("  Reference throughout: the leaky Monte Carlo k_eff.");
                }

                println!("  The reflective pair is the one comparable to the MC k_inf above.");
                println!("  The vacuum pair adds leakage the reference does NOT have; the");
                println!("  spread between the two pairs IS that artefact, measured.");
                println!("  NOTE: a 1-D SLAB is not a cylinder. Even the reflective arm tests");
                println!("  only whether SPATIAL separation of fuel and reflector recovers the");
                println!("  gap; it is not yet a geometric model of the HTR-10.");
            }
            Err(e) => println!("  two-zone cross-section build failed: {e}"),
        },
        Err(e) => println!("  two-zone bridge refused the data: {e}"),
    }

    println!("\n  READ THIS BEFORE QUOTING ANYTHING ABOVE");
    println!("  - This is k_inf, NOT the HTR-10 eigenvalue. The real core is");
    println!("    leakage-dominated and nothing here is comparable to the RMC paper.");
    println!("  - Both ends are this workspace's own codes: verification, not validation.");
    println!("  - The MC number is ONE seed. Its sigma is the reported MC statistical");
    println!("    error, which is not the same as run-to-run scatter.");
}
