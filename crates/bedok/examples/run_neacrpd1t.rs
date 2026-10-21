//! `run_neacrpd1t.m` — the NEACRP case D1 transient runner.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `run_neacrpd1t.m`, `main_exec_diff3d_standalone` snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.
//!
//! # Why this is an example and not a module
//!
//! Like `main_exec_diff3d.m`, this is a **script** rather than a function — it
//! sets parameters, builds the case, marches the transient and prints a summary.
//! See that example's header for the reasoning.
//!
//! # Two things the reference does that this does not
//!
//! - **The `.mat` steady-state cache.** `run_neacrpd1t.m` sets
//!   `params.steadyfile = 'neacrpd1t_steady.mat'` so repeated runs skip the
//!   Phase-0 solve. That format is MATLAB's; the Rust driver takes an
//!   `initial_steady` argument instead, and this example simply passes `None`.
//! - **`save('neacrpd1t_results.mat', ...)`.** The results are returned in a
//!   [`bedok::thdiffusion_solvertimexyz::TransientOutput`] and summarised here.
//!
//! # Cost
//!
//! **This is a long run.** The full 20 s transient is 261 time steps on a
//! 17x17x14 mesh, each rebuilding the operators and factorising twice, on top of
//! a coupled steady state first. Budget hours, not minutes. Pass a shorter end
//! time to try the machinery:
//!
//! ```text
//! cargo run --release -p bedok --example run_neacrpd1t -- 0.5
//! ```

use bedok::types::Params;

fn main() {
    let tend: f64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(20.0);

    let params = Params {
        max_num_cycles: 150,
        nodalupd: 0,
        stop: 0,
        verb: 1,
        plotfig: 0,
        debugdump: 0,
        ..Default::default()
    };

    let (mut params, geometry, th, whichsigma, sigmavalues, feedback) =
        bedok::neacrpd1t::neacrpd1t(&params);

    if tend < 20.0 {
        println!("(shortened run: tend = {tend} s, uniform 10 ms steps)");
        params.tend = Some(tend);
        params.tgrid = None;
    }

    println!("===== NEACRP D1 transient: inlet cold-water injection, 0 to {tend} s =====");
    let started = std::time::Instant::now();

    let out = bedok::thdiffusion_solvertimexyz::thdiffusion_solvertimexyz(
        &geometry,
        &params,
        &th,
        &sigmavalues,
        &feedback,
        &whichsigma,
        None,
        None,
    )
    .expect("the D1 transient should run");

    let elapsed = started.elapsed();
    let last = out.time.len() - 1;

    println!();
    println!("===== transient summary =====");
    println!("termination        : {:?}", out.termination);
    println!("steady k_eff       : {:.6}", out.steady.k_eff);
    println!(
        "re-equilibrated    : {:.6} ({} power iterations)",
        out.k_eff, out.reequilibrate_iterations
    );
    println!("steps marched      : {}", out.time.len());
    println!(
        "C1 relative power  : max {:.4} at t = {:.3} s, final {:.4}",
        out.prelmax, out.tpmax, out.relpower[last]
    );
    println!(
        "C2 avg fuel temp   : {:.1} -> {:.1} K",
        out.avgfueltemp[0], out.avgfueltemp[last]
    );
    println!(
        "C3 max fuel temp   : {:.1} -> {:.1} K",
        out.maxfueltemp[0], out.maxfueltemp[last]
    );
    println!(
        "C4 coolant outlet  : {:.2} -> {:.2} K",
        out.coolouttemp[0], out.coolouttemp[last]
    );
    println!("wall time          : {:.1} s", elapsed.as_secs_f64());

    // The reference writes C1-C4 to `neacrpd1t_C1toC4_history.csv` and the C5/C6
    // radial maps to four more files. They are returned instead; the shapes are
    // reported here so the example stays a summary rather than a dump.
    println!();
    println!("C5/C6 radial maps  : {}x{} each, 4 maps returned in the output struct",
        out.rad_c5_z6.rows(), out.rad_c5_z6.cols());
    println!("(the reference writes these to CSV; this crate returns them)");
}
