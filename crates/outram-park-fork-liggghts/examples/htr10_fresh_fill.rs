//! A fresh HTR-10 pour from empty with `htr10_fill` (gh:#561), printing the
//! progress the dhoby-ghaut workbench shows, and writing the settled centres
//! as `id,x,y,z,vx,vy,vz` (the `reference-data/liggghts/` CSV format).
//!
//! ```text
//! cargo run --release -p outram-park-fork-liggghts --example htr10_fresh_fill -- \
//!     [--n 27000] [--threads 12] [--chunk 2000] [--out target/htr10_fresh_fill.csv]
//! ```
//!
//! Defaults are `Htr10FillSettings::default()` (µ = 0.1, µ_r = 0, E = 5e8 Pa,
//! V&V § 4.9). The printed φ is a result of the run, not a target.

use outram_park_fork_liggghts::compute::ThreadCount;
use outram_park_fork_liggghts::htr10_fill::{Htr10Fill, Htr10FillSettings};
use uom::si::length::centimeter;
use uom::si::time::second;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let n: usize = arg("--n").map_or(27_000, |v| v.parse().expect("--n"));
    let threads: usize = arg("--threads").map_or(12, |v| v.parse().expect("--threads"));
    let chunk: usize = arg("--chunk").map_or(2000, |v| v.parse().expect("--chunk"));
    let out = arg("--out").unwrap_or_else(|| "target/htr10_fresh_fill.csv".into());
    let settings = Htr10FillSettings { n_pebbles: n, threads: ThreadCount::Fixed(threads), ..Htr10FillSettings::default() };
    let t0 = std::time::Instant::now();
    let mut fill = Htr10Fill::new(settings).expect("valid fill");
    println!("# HTR-10 fresh pour: {n} pebbles, mu {} mu_r {}, {threads} threads", settings.friction, settings.rolling_friction);
    println!("# step  t[s]  KE/E_drop(core)  phi_whole_core  surface[cm]  in_core  wall[s]");
    loop {
        let p = fill.advance(chunk);
        println!(
            "{:6} {:6.3} {:12.3e} {:10.4} {:9.2} {:7} {:7.1}",
            p.steps,
            p.time.get::<second>(),
            p.ke_ratio_core,
            p.phi_whole_core,
            p.surface_height.get::<centimeter>(),
            p.n_in_core,
            t0.elapsed().as_secs_f64()
        );
        if p.settled || p.gave_up {
            println!("# {}", if p.settled { "settled" } else { "GAVE UP: max_steps reached before settling" });
            break;
        }
    }
    let mut csv = String::from("id,x,y,z,vx,vy,vz\n");
    for (i, c) in fill.centres().iter().enumerate() {
        csv.push_str(&format!("{},{:.17e},{:.17e},{:.17e},0,0,0\n", i + 1, c.x, c.y, c.z));
    }
    std::fs::write(&out, csv).expect("write centres");
    println!("# wrote {out}");
}
