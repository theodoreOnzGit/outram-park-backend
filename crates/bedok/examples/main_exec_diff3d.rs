//! `main_exec_diff3d.m` — the reference's top-level driver script.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `main_exec_diff3d.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//!
//! # Why this is an example and not a module
//!
//! Every other `.m` file in the snapshot is a **function** and becomes a module.
//! This one is a **script**: it sets parameters, picks a case by uncommenting a
//! line, calls a solver, and plots. There is no function to translate, and a
//! library module that ran a benchmark on import would be absurd.
//!
//! So it lands here instead, as the runnable entry point the crate otherwise
//! lacks. The workspace's "Human interface layer" rule asks for exactly this —
//! an example a reader can follow top to bottom without opening other files.
//!
//! # What it does and does not reproduce
//!
//! The reference's case selection is a block of commented-out calls with one
//! live; here it is a command-line argument, which is the same choice made
//! legible. The plotting at the end is not reproduced — see
//! [`bedok::plotreactor3dcolour`] for why, and for the scaled power map the
//! figure was coloured by, which this example prints instead.
//!
//! # Running it
//!
//! ```text
//! cargo run --release -p bedok --example main_exec_diff3d -- iaea3ds
//! cargo run --release -p bedok --example main_exec_diff3d -- neacrpd1
//! ```
//!
//! `iaea3ds` is pure neutronics and takes well under a minute. `neacrpd1` runs
//! the coupled loop and takes a few minutes.

use bedok::types::Params;

fn main() {
    let case = std::env::args().nth(1).unwrap_or_else(|| "iaea3ds".to_string());

    // The reference's user set-up block. `nodalupd = 0` selects the built-in
    // default; note defect N1 — that default is 1 on a small mesh, and 1 does
    // not converge. Every case here is far too large for that to bite.
    let params = Params {
        max_num_cycles: 150,
        nodalupd: 0,
        stop: 0,
        verb: 1,
        plotfig: 0,
        debugdump: 0,
        ..Default::default()
    };

    match case.as_str() {
        "iaea3ds" => run_iaea3ds(&params),
        "neacrpd1" => run_neacrpd1(&params),
        other => {
            eprintln!("unknown case {other:?}");
            eprintln!("available: iaea3ds (pure neutronics), neacrpd1 (coupled)");
            std::process::exit(2);
        }
    }
}

/// The IAEA-3D benchmark: a bare eigenvalue solve, no thermal hydraulics.
fn run_iaea3ds(params: &Params) {
    println!("===== IAEA-3D, 17x17x19, SANM nodal =====");
    let params = Params {
        // The reference's own default for this mesh.
        nodalupd: 6,
        ..params.clone()
    };
    let (params, geometry, whichsigma, sigmavalues) = bedok::iaea3ds::iaea3ds(&params);

    let out = bedok::sanodaldiffusion_solverxyz::sanodaldiffusion_solverxyz(
        &geometry,
        &params,
        &sigmavalues,
        &whichsigma,
        None,
        None,
    )
    .expect("the IAEA-3D case should solve");

    println!("termination  : {:?}", out.termination);
    println!("k_eff        : {:.6}", out.k_eff);
    println!(
        "  vs PARCS   : {:+.1} pcm",
        (out.k_eff - bedok::iaea3ds::REFERENCE_K_EFF_PARCS) / bedok::iaea3ds::REFERENCE_K_EFF_PARCS
            * 1e5
    );
    println!(
        "  vs ADPRES  : {:+.1} pcm",
        (out.k_eff - bedok::iaea3ds::REFERENCE_K_EFF_ADPRES)
            / bedok::iaea3ds::REFERENCE_K_EFF_ADPRES
            * 1e5
    );
    println!("iterations   : {} ({} nodal rebuilds)", out.iterations, out.nodal_updates);
    println!("residuals    : fission source {:.3e}, k_eff {:.3e}", out.residual, out.k_eff_residual);
}

/// NEACRP case D1: the coupled neutronics / thermal-hydraulics steady state.
fn run_neacrpd1(params: &Params) {
    println!("===== NEACRP D1, 17x17x14, coupled steady state =====");
    let (params, geometry, th, whichsigma, sigmavalues, feedback) =
        bedok::neacrpd1::neacrpd1(params);

    let out = bedok::thdiffusion_solverxyz::thdiffusion_solverxyz(
        &geometry,
        &params,
        &th,
        &sigmavalues,
        &feedback,
        &whichsigma,
        None,
    )
    .expect("the NEACRP D1 case should run");

    println!("termination  : {:?} after {} outer passes", out.termination, out.iterations);
    println!("k_eff        : {:.6}", out.k_eff);
    println!("residuals    : fs {:.3e}, k_eff {:.3e}, fuel temp {:.4} K",
        out.residual, out.k_eff_residual, out.fueltemp_residual);

    // `rel_power = calc_relpower3d(params, results.pwrdens)`, which the
    // reference writes to `rel_power.csv`. Printed here instead; this crate
    // does not write files as a side effect.
    let rel = bedok::calc_relpower3d::calc_relpower3d(&params, &out.pwrdens);
    let peak = (0..rel.rows())
        .flat_map(|i| (0..rel.cols()).map(move |j| (i, j)))
        .map(|(i, j)| rel.get(i, j))
        .fold(f64::NEG_INFINITY, f64::max);
    println!("peak relative assembly power: {peak:.4}");

    // The quantity `plotreactor3dcolour` colours its figure by.
    let scaled = bedok::plotreactor3dcolour::scaled_power(&params, &geometry, &out.pwrdens);
    println!("peak scaled power density   : {:.6e}", scaled.peak);
    if scaled.all_zero_from_single_group {
        println!("  (defect P1: one group, so the map is all zero)");
    }
}
