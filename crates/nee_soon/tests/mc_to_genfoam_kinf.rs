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

//! # Monte Carlo -> MGXS -> GeN-Foam: does the deterministic solve reproduce the
//! transport eigenvalue?
//!
//! This is the end-to-end check of the first coupling stage. `outram-mc` runs an
//! **infinite homogeneous medium**, [`nee_soon::mgxs`] condenses that run into
//! multigroup constants, [`nee_soon::genfoam_xs`] hands them to GeN-Foam through
//! its own `nuclearData` input path, and GeN-Foam's multigroup diffusion solver
//! computes `k_inf` from them alone.
//!
//! ## Why an infinite medium makes this a real test
//!
//! In general a diffusion solve cannot be expected to reproduce a transport
//! eigenvalue: diffusion theory is an approximation, and the difference is
//! physics, not error. **With zero leakage that objection disappears.** In an
//! infinite homogeneous medium the diffusion coefficient multiplies a flux
//! gradient that is identically zero, so `k_inf` depends only on the group
//! constants and the group fluxes:
//!
//! ```text
//!   k_inf = sum_g nu_Sigma_f,g phi_g / sum_g Sigma_a,g phi_g
//! ```
//!
//! and those are exactly the quantities the condensation preserved by
//! construction. So the two codes must agree to within Monte Carlo statistics,
//! and a disagreement means the condensation, the unit conversion, the matrix
//! orientation or the chi normalisation is wrong. That is a check with teeth.
//!
//! In particular it is sensitive to the **cm to m unit conversion**: getting it
//! wrong changes no reaction-rate ratio in an infinite medium... which is
//! precisely why the leakage-free case cannot catch it, so the test also
//! asserts the converted values directly against their centimetre originals.
//!
//! ## Results (measured 2026-09-19)
//!
//! Printed at run time. This is **verification of the coupling path**, not
//! validation of either code: no experiment and no published benchmark is
//! involved.

use std::sync::Arc;

use nee_soon::genfoam_xs::{to_nuclear_data_input, CM_INV_TO_M_INV, CM_TO_M};
use nee_soon::mgxs::{condense, matrix_tally, scalar_tally, GroupStructure};
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::{
    DiffusionNeutronics, DiffusionSettings,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData;
use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
use outram_foam_basic_lib::prelude::{BoundaryCondition, FvMesh};
use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, XPlane, YPlane, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{run_keff_csg, SourceBox};
use uom::si::area::square_meter;
use uom::si::f64::{Area, Length};
use uom::si::length::meter;

const SEED: u64 = 20_260_919;
const N_GROUPS: usize = 2;
/// Half-width of the reflective box \[cm\]. Any value works — the medium is
/// infinite regardless — so this is chosen only to keep flights short.
const HALF: f64 = 10.0;

fn nuclides() -> Vec<Nuclide> {
    vec![
        Nuclide::from_core("U234").expect("U234 in CORE WMP library"),
        Nuclide::from_core("U235").expect("U235 in CORE WMP library"),
        Nuclide::from_core("U238").expect("U238 in CORE WMP library"),
    ]
}

/// One homogeneous HEU material — Godiva densities.
fn materials() -> Vec<Material> {
    vec![Material {
        id: 1,
        name: "HEU".into(),
        temperature: 293.6,
        components: vec![
            NuclideComponent {
                nuclide_idx: 0,
                atom_density: 4.9184e-4,
            },
            NuclideComponent {
                nuclide_idx: 1,
                atom_density: 4.4994e-2,
            },
            NuclideComponent {
                nuclide_idx: 2,
                atom_density: 2.4984e-3,
            },
        ],
    }]
}

/// A box with **all six faces reflective**: an infinite homogeneous medium, so
/// the Monte Carlo eigenvalue is `k_inf` with no leakage to model.
fn infinite_medium() -> Geometry {
    let refl = BoundaryType::Reflective;
    let surfaces = vec![
        SurfaceKind::XPlane(XPlane {
            x0: -HALF,
            bc: refl,
        }),
        SurfaceKind::XPlane(XPlane { x0: HALF, bc: refl }),
        SurfaceKind::YPlane(YPlane {
            y0: -HALF,
            bc: refl,
        }),
        SurfaceKind::YPlane(YPlane { y0: HALF, bc: refl }),
        SurfaceKind::ZPlane(ZPlane {
            z0: -HALF,
            bc: refl,
        }),
        SurfaceKind::ZPlane(ZPlane { z0: HALF, bc: refl }),
    ];
    let inside = Cell::material(
        1,
        vec![
            RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::HalfSpace {
                surface_idx: 1,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 2,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 3,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 4,
                sense: HalfSpaceSense::Outside,
            },
            RegionToken::Intersection,
            RegionToken::HalfSpace {
                surface_idx: 5,
                sense: HalfSpaceSense::Inside,
            },
            RegionToken::Intersection,
        ],
        0,
        293.6,
    );
    Geometry {
        surfaces,
        cells: vec![inside],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

fn settings() -> KeffSettings {
    KeffSettings {
        n_particles: 800,
        n_inactive: 20,
        n_active: 60,
        seed: SEED,
        ..KeffSettings::default()
    }
}

fn slab_mesh(n: i64) -> Arc<FvMesh> {
    Arc::new(
        create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), n)
            .expect("valid slab mesh"),
    )
}

#[test]
fn genfoam_reproduces_the_monte_carlo_kinf_from_its_own_mgxs() {
    let geom = infinite_medium();
    let mats = materials();
    let nucs = nuclides();
    // A structure every group POPULATES, still spanning the full 1 meV .. 20 MeV
    // range so no scatter falls outside it. A bare HEU medium has no thermal
    // flux at all, so a finer log grid leaves its low groups empty -- and an
    // empty group has no measured cross section, which the bridge now refuses
    // rather than handing a singular equation to the solver. The split at
    // 50 keV is where this spectrum actually has population on both sides.
    let groups = GroupStructure::new(vec![1.0e-3, 5.0e4, 2.0e7]).expect("group structure");
    let names: Vec<String> = mats.iter().map(|m| m.name.clone()).collect();
    let src = SourceBox {
        lower: Position::new(-HALF, -HALF, -HALF),
        upper: Position::new(HALF, HALF, HALF),
    };

    // ── 1. Monte Carlo: the reference eigenvalue, and the MGXS ───────────────
    let mut scalar = scalar_tally(1, &groups, vec![0]);
    let mc = run_keff_csg(&geom, &mats, &nucs, src, &settings(), Some(&mut scalar));

    let mut matrix = matrix_tally(2, &groups, vec![0]);
    run_keff_csg(&geom, &mats, &nucs, src, &settings(), Some(&mut matrix));

    let lib = condense(
        &groups,
        &names,
        &scalar,
        &matrix,
        settings().n_active as u64,
    )
    .expect("condensation");
    let k_mc = mc.k_mean;
    let sigma_mc = mc.k_std;
    println!("Monte Carlo  k_inf = {k_mc:.6} +/- {sigma_mc:.6}");

    // ── 2. Unit conversion, asserted directly ────────────────────────────────
    //
    // An infinite-medium eigenvalue is a RATIO of reaction rates, so it is
    // blind to a global unit error -- the very reason the k comparison below
    // cannot be trusted to catch one. Check it head-on instead.
    let desc = lib.in_descending_energy();
    let input =
        to_nuclear_data_input(&desc).expect("every group populated, so the bridge accepts it");
    let zone_in = &input.states[0].zones[0];
    for g in 0..N_GROUPS {
        let want_nsf = desc.zones[0].nu_fission[g] * CM_INV_TO_M_INV;
        assert!(
            (zone_in.nu_sigma_eff[g] - want_nsf).abs() <= 1e-12 * want_nsf.abs().max(1.0),
            "group {g}: nu_sigma_f not converted cm^-1 -> m^-1 (got {}, want {want_nsf})",
            zone_in.nu_sigma_eff[g]
        );
        if let Some(d_cm) = desc.zones[0].diffusion_coefficient(g) {
            let want_d = d_cm * CM_TO_M;
            assert!(
                (zone_in.d[g] - want_d).abs() <= 1e-12 * want_d.abs().max(1.0),
                "group {g}: D not converted cm -> m (got {}, want {want_d})",
                zone_in.d[g]
            );
        }
    }
    // The scattering matrix must arrive in the SAME orientation, [from][into]:
    // GeN-Foam documents scattering[moment][j][i] as Sigma_{s, j -> i}. An
    // accidental transpose here is worth thousands of pcm and does not look
    // like an indexing bug from the eigenvalue alone.
    for from in 0..N_GROUPS {
        for into in 0..N_GROUPS {
            let want = desc.zones[0].scatter[from][into] * CM_INV_TO_M_INV;
            assert!(
                (zone_in.scattering[0][from][into] - want).abs() <= 1e-12 * want.abs().max(1.0),
                "scattering {from}->{into} landed wrong: an orientation error"
            );
        }
    }
    println!("unit conversion and matrix orientation verified against the cm-based source");

    // ── 2b. Does the MGXS set reproduce k_inf on its OWN terms? ──────────────
    //
    // Separates "the group constants are wrong" from "GeN-Foam reads them
    // differently". In an infinite medium, using the Monte Carlo's own group
    // fluxes as the weighting spectrum,
    //     k_inf = sum_g nu_Sigma_f,g phi_g / sum_g Sigma_a,g phi_g
    // must reproduce the Monte Carlo eigenvalue, because both sides are the
    // same tallied reaction rates.
    let z = &lib.zones[0];
    let prod: f64 = (0..N_GROUPS).map(|g| z.nu_fission[g] * z.flux[g]).sum();
    let absn: f64 = (0..N_GROUPS).map(|g| z.absorption[g] * z.flux[g]).sum();
    let k_from_mgxs = prod / absn;
    println!(
        "k_inf from MGXS x MC flux = {k_from_mgxs:.6}  ({:+.1} pcm vs MC)",
        (k_from_mgxs - k_mc) / k_mc * 1.0e5
    );

    // Neutron balance per group, as GeN-Foam sees it: it derives absorption
    // implicitly as removal minus out-scatter-to-other-groups. If that implied
    // absorption differs from the tallied one, GeN-Foam is solving a different
    // problem from the one measured.
    println!("\nper-group: tallied Sigma_a vs the value implied by removal - out-scatter");
    for g in 0..N_GROUPS {
        let removal = z.removal(g).unwrap_or(0.0);
        let out_other: f64 = (0..N_GROUPS)
            .filter(|gg| *gg != g)
            .map(|gg| z.scatter[g][gg])
            .sum();
        let implied = removal - out_other;
        println!(
            "  g{g}  phi={:.3e}  Sigma_a={:.5e}  implied={:.5e}  ratio={:.4}",
            z.flux[g],
            z.absorption[g],
            implied,
            if z.absorption[g] > 0.0 {
                implied / z.absorption[g]
            } else {
                f64::NAN
            }
        );
    }
    println!();

    // ── 2c. What spectrum do MY OWN constants predict? ───────────────────────
    //
    // Two-group infinite medium, groups in DESCENDING energy:
    //   Sigma_r,0 phi0 = chi_0 S
    //   Sigma_r,1 phi1 = Sigma_s,0->1 phi0 + chi_1 S
    // Solving the second for phi1/phi0 given S from the first isolates whether
    // GeN-Foam is solving these equations correctly or whether the constants
    // are internally inconsistent with the Monte Carlo spectrum they came from.
    let d0 = &desc.zones[0];
    let sr0 = d0.removal(0).unwrap();
    let sr1 = d0.removal(1).unwrap();
    let s01 = d0.scatter[0][1];
    let s10 = d0.scatter[1][0];
    println!("\ntwo-group constants (descending): Sigma_r0={sr0:.5e} Sigma_r1={sr1:.5e}");
    println!("  Sigma_s 0->1 = {s01:.5e}   Sigma_s 1->0 = {s10:.5e}");
    println!(
        "  chi = [{:.5}, {:.5}]  nu_Sigma_f = [{:.5e}, {:.5e}]",
        d0.chi[0], d0.chi[1], d0.nu_fission[0], d0.nu_fission[1]
    );
    // S normalised so phi0 = 1: S = Sigma_r,0 / chi_0
    let s_src = sr0 / d0.chi[0].max(1e-300);
    let phi1_pred = (s01 + d0.chi[1] * s_src) / sr1;
    println!("  predicted phi1/phi0 = {phi1_pred:.5}");
    let mc_ratio = desc.zones[0].flux[1] / desc.zones[0].flux[0];
    println!("  Monte Carlo phi1/phi0 = {mc_ratio:.5}");

    // ── 3. GeN-Foam: solve k_inf from those constants alone ──────────────────
    let xs = CrossSectionData::from_input(&input).expect("GeN-Foam accepts the MGXS");
    assert_eq!(xs.energy_groups(), N_GROUPS);
    assert_eq!(xs.zone_count(), 1);

    let mesh = slab_mesh(4);
    // Zero-gradient both ends: no leakage, so this is an infinite medium and
    // the diffusion coefficient multiplies a zero gradient.
    let bc = vec![
        BoundaryCondition::ZeroGradient,
        BoundaryCondition::ZeroGradient,
    ];
    let zone_of_cell = vec![0usize; mesh.n_cells];
    let mut model = DiffusionNeutronics::new(
        mesh,
        &xs,
        &zone_of_cell,
        &[],
        &bc,
        DiffusionSettings::default(),
    )
    .expect("build the diffusion model from Monte Carlo MGXS");

    let report = model
        .solve_eigenvalue()
        .expect("GeN-Foam eigenvalue converges on Monte Carlo MGXS");
    let k_det = report.k_eff;

    // Compare the two SPECTRA. If GeN-Foam's converged group fluxes differ in
    // shape from the Monte Carlo's, the eigenvalue must differ too, and the
    // scattering matrix that sets the shape is the thing to look at.
    let gf_flux: Vec<f64> = model
        .state()
        .flux()
        .iter()
        .map(|f| f.internal.as_slice()[0])
        .collect();
    let gf_tot: f64 = gf_flux.iter().sum();
    let mc_desc = &desc.zones[0].flux;
    let mc_tot: f64 = mc_desc.iter().sum();
    println!("\nnormalised spectrum (descending energy, group 0 = fastest):");
    for g in 0..N_GROUPS {
        println!(
            "  g{g}  MC {:.5}   GeN-Foam {:.5}",
            if mc_tot > 0.0 {
                mc_desc[g] / mc_tot
            } else {
                0.0
            },
            if gf_tot > 0.0 {
                gf_flux[g] / gf_tot
            } else {
                0.0
            }
        );
    }

    let diff_pcm = (k_det - k_mc) / k_mc * 1.0e5;
    println!(
        "GeN-Foam    k_inf = {k_det:.6}  ({} outer iterations)",
        report.outer_iterations
    );
    println!(
        "difference        = {diff_pcm:+.1} pcm  (MC 1-sigma = {:.1} pcm)",
        sigma_mc / k_mc * 1.0e5
    );

    assert!(
        k_det.is_finite() && k_det > 0.0,
        "GeN-Foam returned a non-physical k_inf = {k_det}"
    );

    // With zero leakage and every group populated, the two must agree to within
    // Monte Carlo statistics. 1000 pcm is a few sigma at this particle count and
    // is tight enough that a transposed matrix, a bad chi normalisation or a
    // dropped scattering channel blows straight through it -- each of those is
    // worth thousands of pcm. The empty-group degeneracy this once hid was
    // itself 4873 pcm.
    assert!(
        diff_pcm.abs() < 1000.0,
        "GeN-Foam k_inf {k_det:.6} disagrees with Monte Carlo {k_mc:.6} by {diff_pcm:+.1} pcm \
         in a LEAKAGE-FREE medium, where the eigenvalue depends only on the group constants \
         the condensation preserved. The MGXS path is wrong, not the physics."
    );
}
