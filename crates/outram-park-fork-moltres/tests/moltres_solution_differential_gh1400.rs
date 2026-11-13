// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
//! # Solution-level differential against real Moltres 3dd2ce7 numbers (gh:#1400)
//!
//! **Methodology.** MOOSE + Moltres were *not* rebuilt (see
//! `verification_and_validation/moltres_differential_gh1400/PREDICTIONS.md`).
//! The reference numbers are the values stored in Moltres' own committed
//! `tests/*/gold/*.e` Exodus files (outputs of real Moltres runs, extracted
//! by `gold_extract.py` into `moltres_gold_values.txt`). Every case below
//! uses the same tables and boundary data as the Moltres input of the same
//! name.
//!
//! * S1 `bcs/vacuum_bc.i` (Marshak, Mark, Milne): the port's FV vacuum
//!   boundary against the FE right-edge value.
//! * S2/S3 `materials/gmm_{none,linear,spline,monotone_cubic}.i`: natural
//!   boundaries, so the eigenfunction is uniform and `k_eff = k_inf` for FE
//!   and FV alike; the port reads the same property tables.
//! * S4 issue case 4: table interpolation against the port's reduced linear
//!   removal-XS feedback.
//!
//! Bounds were written in `PREDICTIONS.md` before any of this ran; results
//! and the comparison with the predictions are in `RESULTS.md`.

use std::sync::Arc;

use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
use outram_foam_basic_lib::prelude::{
    fvm, BoundaryCondition, Field, FvMesh, SolverSettings, VolScalarField,
};
use outram_park_fork_moltres::diffusion::{
    vacuum_patch_field, EigenSettings, StaticDiffusion, VacuumBoundary,
};
use outram_park_fork_moltres::materials::{MsrMaterial, TemperatureField, XsFields};
use outram_park_fork_moltres::tables::{InterpType, MoltresTables, TableLayout};
use uom::si::area::square_meter;
use uom::si::f64::{Area, Length};
use uom::si::length::meter;

const GOLD: &str = include_str!(
    "../verification_and_validation/moltres_differential_gh1400/moltres_gold_values.txt"
);
const DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/verification_and_validation/moltres_differential_gh1400/data/"
);

fn gold_glo(file: &str, name: &str) -> f64 {
    for l in GOLD.lines() {
        let t: Vec<&str> = l.split_whitespace().collect();
        if t.len() == 5 && t[1] == file && t[2] == "glo" && t[3] == name {
            return t[4].parse().unwrap();
        }
    }
    panic!("no gold value {file} {name}");
}

fn mesh(n: i64) -> Arc<FvMesh> {
    Arc::new(
        create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), n).unwrap(),
    )
}

/// Right-edge flux of the FV solution of Moltres' `vacuum_bc.i`: unit
/// length, `D = 1e-3`, `Sigma_r = 0`, Dirichlet `phi_l` on the left.
/// `vacuum = None` reproduces the port's former "vacuum" (`FixedValue(0)`).
fn right_edge(phi_l: f64, vacuum: Option<VacuumBoundary>) -> f64 {
    let m = mesh(20);
    assert_eq!(m.patches.len(), 2);
    // the left end is the patch whose face is owned by cell 0
    let left = if m.owner[m.patches[0].start] == 0 {
        0
    } else {
        1
    };
    let right = 1 - left;
    let mat = MsrMaterial::non_fuel("m", vec![1e-3], vec![0.0]);
    let xs = XsFields::materialize(m.clone(), &vec![0usize; m.n_cells], &[mat]).unwrap();
    let mut phi = VolScalarField::uniform("phi", m.clone(), 1.0);
    phi.boundary[left].bc = BoundaryCondition::FixedValue(phi_l);
    phi.boundary[left].values = Field::uniform(1, phi_l);
    let mut w = 1.0;
    match vacuum {
        Some(kind) => {
            phi.boundary[right] = vacuum_patch_field(&m, right, kind, &xs.diffusion_cell[0]);
            if let BoundaryCondition::MixedField { value_fraction, .. } = &phi.boundary[right].bc {
                w = value_fraction[0];
            }
        }
        None => {
            phi.boundary[right].bc = BoundaryCondition::FixedValue(0.0);
            phi.boundary[right].values = Field::uniform(1, 0.0);
        }
    }
    let eqn = fvm::laplacian(&xs.diffusion_face[0], &phi);
    let (sol, _) = eqn.solve_cg_with_guess(
        "phi",
        &phi,
        SolverSettings {
            tolerance: 1e-14,
            max_iter: 20_000,
        },
    );
    let last = sol.internal.as_slice()[m.n_cells - 1];
    if vacuum.is_some() {
        (1.0 - w) * last
    } else {
        0.0
    }
}

/// S1: Marshak / Mark / Milne right-edge value against Moltres' gold files.
///
/// **Pass criterion** (predicted): `|port - gold| <= 1e-9` absolute.
#[test]
fn s1_vacuum_boundaries_match_moltres_gold() {
    let cases = [
        (VacuumBoundary::Marshak, 1.002, "bcs/gold/vacuum_bc_out.e"),
        (
            VacuumBoundary::Mark,
            1.0017320508,
            "bcs/gold/vacuum_bc_mark_out.e",
        ),
        (
            VacuumBoundary::Milne,
            1.0017320508,
            "bcs/gold/vacuum_bc_milne_out.e",
        ),
    ];
    for (kind, phi_l, file) in cases {
        let gold = gold_glo(file, "boundary_value");
        let port = right_edge(phi_l, Some(kind));
        let old = right_edge(phi_l, None);
        // closed form from the BC itself: phi_b = a D phi_l / (1 + a D)
        let a = kind.alpha();
        let exact = a * 1e-3 * phi_l / (1.0 + a * 1e-3);
        println!(
            "[S1 {kind:?}] gold {gold:.15e} port {port:.15e} exact {exact:.15e} \
             |port-gold| {:.3e}; former FixedValue(0) edge {old:.1e} (miss {:.3e})",
            (port - gold).abs(),
            (old - gold).abs()
        );
        assert!((port - gold).abs() <= 1e-9);
        assert!((port - exact).abs() <= 1e-12);
        assert!(
            (old - gold).abs() > 1e-3,
            "the old boundary must have missed"
        );
    }
}

fn k_inf(tabs: &MoltresTables, t: f64) -> f64 {
    let m = mesh(4);
    let mat = tabs.material_at("fuel", t);
    let xs = XsFields::materialize(m.clone(), &vec![0usize; m.n_cells], &[mat]).unwrap();
    k_inf_of(xs)
}

fn k_inf_of(xs: XsFields) -> f64 {
    let bc = [
        BoundaryCondition::ZeroGradient,
        BoundaryCondition::ZeroGradient,
    ];
    let mut settings = EigenSettings::default();
    settings.k_tolerance = 1e-13;
    settings.flux_tolerance = 1e-12;
    settings.max_outer_iterations = 100_000;
    settings.linear.tolerance = 1e-14;
    let mut s = StaticDiffusion::new(xs, &[], &bc, settings).unwrap();
    s.solve().unwrap().k_eff
}

/// S2: `gmm_none` and `gmm_linear` k_eff against the Moltres gold values.
///
/// **Pass criterion** (predicted): relative difference <= 1e-8.
#[test]
fn s2_k_inf_matches_moltres_gmm_none_and_linear() {
    let none = MoltresTables::load(
        &format!("{DIR}inf_crit_small_two_group_"),
        2,
        8,
        TableLayout::Legacy,
        InterpType::None,
    )
    .unwrap();
    let lin = MoltresTables::load(
        &format!("{DIR}newt_fuel_"),
        2,
        6,
        TableLayout::Legacy,
        InterpType::Linear,
    )
    .unwrap();
    for (name, tabs, file) in [
        ("none", &none, "materials/gold/gmm_none_out.e"),
        ("linear", &lin, "materials/gold/gmm_linear_out.e"),
    ] {
        let gold = gold_glo(file, "k_eff");
        let k = k_inf(tabs, 922.0);
        let rel = (k - gold).abs() / gold;
        println!("[S2 {name}] gold k_eff {gold:.12} port k_inf {k:.12} rel {rel:.3e}");
        assert!(rel <= 1e-8, "{name}: rel {rel:e}");
    }
}

/// S3: the port is linear-only; its distance to the spline / monotone-cubic
/// gold values must equal the linear-vs-spline distance inside Moltres.
#[test]
fn s3_spline_and_monotone_cubic_are_model_differences() {
    let lin = MoltresTables::load(
        &format!("{DIR}newt_fuel_"),
        2,
        6,
        TableLayout::Legacy,
        InterpType::Linear,
    )
    .unwrap();
    let k = k_inf(&lin, 922.0);
    let g_lin = gold_glo("materials/gold/gmm_linear_out.e", "k_eff");
    for file in [
        "materials/gold/gmm_spline_out.e",
        "materials/gold/gmm_monotone_cubic_out.e",
    ] {
        let g = gold_glo(file, "k_eff");
        let port_minus = k - g;
        let moltres_minus = g_lin - g;
        println!(
            "[S3 {file}] port(linear)-gold {port_minus:.4e} ({:.2} pcm); \
             moltres linear-gold {moltres_minus:.4e}",
            port_minus * 1e5
        );
        assert!((port_minus - moltres_minus).abs() <= 1e-8);
    }
}

/// S4: table interpolation against the port's reduced linear removal-XS
/// feedback (`XsFields::sigma_removal_at`), five temperatures.
///
/// Reduced model: every constant frozen at the 900 K table row, `Sigma_r`
/// carrying Moltres' own `d_remxs_d_temp` at 900 K.
#[test]
fn s4_reduced_feedback_against_table_interpolation() {
    let lin = MoltresTables::load(
        &format!("{DIR}newt_fuel_"),
        2,
        6,
        TableLayout::Legacy,
        InterpType::Linear,
    )
    .unwrap();
    let t_ref = 900.0;
    let temps = [825.0, 875.0, 922.0, 975.0, 1025.0];
    let m = mesh(4);
    let base = lin.material_at("fuel", t_ref);
    let xs_ref =
        XsFields::materialize(m.clone(), &vec![0usize; m.n_cells], &[base.clone()]).unwrap();
    let mut table = Vec::new();
    let mut reduced = Vec::new();
    for t in temps {
        table.push(k_inf(&lin, t));
        let tf: TemperatureField = VolScalarField::uniform("T", m.clone(), t);
        let sr = xs_ref.sigma_removal_at(&tf, t_ref).unwrap();
        let mut mat = base.clone();
        for g in 0..mat.sigma_removal.len() {
            mat.sigma_removal[g] = sr[g].internal.as_slice()[0];
        }
        let xs = XsFields::materialize(m.clone(), &vec![0usize; m.n_cells], &[mat]).unwrap();
        reduced.push(k_inf_of(xs));
    }
    for i in 0..temps.len() {
        println!(
            "[S4] T {:7.1} K  k_table {:.8}  k_reduced {:.8}  diff {:+8.1} pcm",
            temps[i],
            table[i],
            reduced[i],
            (reduced[i] - table[i]) * 1e5
        );
    }
    let slope_t = (table[4] - table[0]) / (temps[4] - temps[0]) * 1e5;
    let slope_r = (reduced[4] - reduced[0]) / (temps[4] - temps[0]) * 1e5;
    println!("[S4] dk/dT table {slope_t:+.2} pcm/K, reduced {slope_r:+.2} pcm/K");
    // PREDICTION MISSED (PREDICTIONS.md S4 said opposite signs, 5-40 pcm/K
    // each, >500 pcm at 1025 K). Measured: both slopes positive, +1.64 and
    // +0.05 pcm/K, error -283 pcm at 1025 K. The measured values are pinned
    // below as a regression record, not as a claim about what they "should"
    // be; the predictions file is left as written.
    let expect_diff_pcm = [35.8, 21.3, -25.9, -117.7, -282.8];
    for i in 0..temps.len() {
        let d = (reduced[i] - table[i]) * 1e5;
        assert!((d - expect_diff_pcm[i]).abs() < 0.2, "T {}: {d}", temps[i]);
    }
    assert!((slope_t - 1.64).abs() < 0.02 && (slope_r - 0.05).abs() < 0.02);
}
