// SPDX-License-Identifier: GPL-3.0

//! NP-MHTGR fission-product release under a prescribed heat-up: activity
//! released per nuclide, in Bq and Ci, and as a fraction of the core inventory.
//!
//! ```bash
//! cargo run --release -p sembawang --example npmhtgr_release
//! ```
//!
//! This is `sembawang`'s end-to-end example (GitHub issues #229, #232). The
//! release physics is `boon-lay`'s TRISO-ATOPS fork, verified code-to-code
//! against upstream; the orchestration in `sembawang` has **no upstream and no
//! code-to-code verification**.
//!
//! **Research and education only.** No dose quantity is computed. Every input
//! below is prescribed, and two of them are not physical:
//!
//! - **Inventory:** [`CoreInventory::unit`], exactly 1 Ci of each nuclide per
//!   radial ring. The released *fraction* is the meaningful output; the
//!   absolute Bq/Ci columns scale linearly with whatever real inventory is put
//!   in its place.
//! - **Accident-phase failure fractions:** `1e-4` and `1e-4`, a shakedown
//!   choice, **not cited values** — the NP-MHTGR reference case does not supply
//!   them.
//! - **Temperature:** an analytic ramp and hold, not a thermal-hydraulic
//!   solution. Nothing here computes the transient.
//! - Normal-operation pools start **empty**, which under-predicts the early
//!   release.
//!
//! The source term this produces is the `changi::activity::source::SourceTerm`
//! that `changi` carries downwind; see `changi`'s `site_activity_survey`
//! example for that half.

use sembawang::accident::release::{accident_release, PlantParameters};
use sembawang::inventory::CoreInventory;
use sembawang::scenario::TemperatureTransient;
use uom::si::f64::{ThermodynamicTemperature, Time};
use uom::si::radioactivity::{becquerel, curie};
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;

/// Nuclides carried. Each must be one of TRISO-ATOPS' 84.
const NUCLIDES: [&str; 5] = ["Kr-85", "Kr-88", "I-131", "Te-132", "Cs-137"];

/// Radial rings and axial nodes.
const N_RADIAL: usize = 2;
const N_AXIAL: usize = 3;

/// The prescribed transient: start, peak, ramp time, total time.
const START_C: f64 = 700.0;
const PEAK_C: f64 = 1600.0;
const RAMP_S: f64 = 1.0e4;
const TOTAL_S: f64 = 1.0e5;
const SAMPLES: usize = 51;

fn main() {
    let inventory = CoreInventory::unit(&NUCLIDES, N_RADIAL, N_AXIAL);
    let transient = TemperatureTransient::from_ramp(
        ThermodynamicTemperature::new::<degree_celsius>(START_C),
        ThermodynamicTemperature::new::<degree_celsius>(PEAK_C),
        Time::new::<second>(RAMP_S),
        Time::new::<second>(TOTAL_S),
        SAMPLES,
        N_RADIAL,
        N_AXIAL,
    )
    .expect("the transient has enough samples");
    // Accident-phase fractions 1e-4 / 1e-4 are a shakedown choice, not cited.
    let plant = PlantParameters::np_mhtgr_reference(1.0e-4, 1.0e-4, 0.0);

    let out = accident_release(&inventory, &transient, &plant)
        .expect("the release calculation runs");
    let term = &out.source_term;

    println!("NP-MHTGR fission-product release -- TRISO-ATOPS (boon-lay fork) via sembawang");
    println!("RESEARCH AND EDUCATION ONLY. Unit inventory (1 Ci per nuclide per ring); no dose computed.");
    println!(
        "Prescribed transient: {START_C:.0} C -> {PEAK_C:.0} C over {RAMP_S:.0} s, held to \
         {TOTAL_S:.0} s; {} release windows",
        term.windows.len()
    );
    println!();

    println!("Released over the whole transient");
    println!(
        "  {:<7}  {:>10}  {:>12}  {:>12}  {:>12}  {:>10}",
        "nuclide", "inv [Ci]", "rel [Bq]", "rel [Ci]", "fraction", "group"
    );
    for n in &term.nuclides {
        let q = n.total_released();
        let inv_ci = inventory
            .nuclides
            .iter()
            .find(|i| i.name == n.label)
            .map_or(f64::NAN, |i| i.total().get::<curie>());
        println!(
            "  {:<7}  {:>10.3}  {:>12.4e}  {:>12.4e}  {:>12.4e}  {:>10}",
            n.label,
            inv_ci,
            q.get::<becquerel>(),
            q.get::<curie>(),
            q.get::<curie>() / inv_ci,
            n.deposition_group.label()
        );
    }
    for name in &out.screened_out {
        println!("  {name:<7}  screened out by the half-life screen");
    }
    println!();

    // Cumulative release against time, at a handful of window ends.
    println!("Cumulative release [Ci] against time");
    print!("  {:>10}", "t [s]");
    for n in &term.nuclides {
        print!("  {:>11}", n.label);
    }
    println!();
    let n_win = term.windows.len();
    let step = (n_win / 8).max(1);
    let mut rows: Vec<usize> = (step - 1..n_win).step_by(step).collect();
    if rows.last() != Some(&(n_win - 1)) {
        rows.push(n_win - 1);
    }
    for w in rows {
        print!("  {:>10.0}", term.windows[w].end.get::<second>());
        for n in &term.nuclides {
            let cum: f64 = n.released[..=w].iter().map(|a| a.get::<curie>()).sum();
            print!("  {cum:>11.4e}");
        }
        println!();
    }
    println!();

    println!("Upstream behaviours that affected this result:");
    for line in out.caveats.lines() {
        println!("  * {line}");
    }
    println!();
    println!("Reading these numbers:");
    println!("  * Linear in the inventory: multiply a fraction by a real inventory to get Ci.");
    println!("  * Kr, I and Te release the same fraction: TRISO-ATOPS assigns kernel diffusivity");
    println!("    per element group (Se/Kr/Te/I/Xe share one), and the half-life cancels in the");
    println!("    activity -> atoms -> activity bookkeeping. See tests/accident_release.rs.");
    println!("  * Empty normal-operation pools: the early release is under-predicted.");
    println!("  * No parent-to-daughter chaining during the accident, and no progression");
    println!("    physics (melt, relocation, vessel failure) -- only TRISO release.");
}
