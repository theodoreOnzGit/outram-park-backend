//! **The diagnostics report, made and read back entirely in memory** (gh
//! #941 acceptance: the zip and PNG paths run on wasm, not only compile).
//!
//! A small Godiva run on the embedded LOW-tier data (`Nuclide::from_core`:
//! no files, no network), `Diagnostics::reactor_physics()`, then
//! `to_zip_bytes()`, then the archive read back with every CRC checked and
//! every PNG decoded. No `std::fs`: this is the browser's path.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example diagnostics_report_in_memory
//! cargo run --release -p outram-mc-libs --example diagnostics_report_in_memory --target wasm32-wasip1
//! ```
//!
//! The second line runs it as wasm under Node's WASI (`.cargo/config.toml`,
//! `scripts/wasi-run.mjs`). WASI is not a browser, but it shares the absence
//! of threads and runs the same code.
//!
//! (From a git worktree nested inside another checkout, Cargo merges both
//! `.cargo/config.toml` runners; run the module directly instead:
//! `node --no-warnings scripts/wasi-run.mjs target/wasm32-wasip1/release/examples/diagnostics_report_in_memory.wasm`.)
//!
//! **Result, 2026-10-17:** native and `wasm32-wasip1` (Node 26.11) both print
//! `k = 1.02692 ± 0.00934` and the same 27 files, and pass every check (every
//! CRC, 11 PNGs decoded). The zip is 207 271 bytes native and 207 198 on
//! wasm: `report.md` differs only in its hardware and wall-time lines (not
//! probed / not measured on wasm32). The numbers are LOW-tier and tiny-run,
//! not benchmark results.

use outram_mc_libs::geometry::plot::decode_png;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::vv::godiva;
use uom::si::f64::{Length, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::thermodynamic_temperature::kelvin;

fn main() {
    // A library of headers only (masses), enough to assemble the model.
    let header = |za: &str, awr: &str| {
        format!(
            " TPID\n{za:>11}{awr:>11}{:>11}{:>11}{:>11}{:>11}\n{:>11}{:>11}{:>11}{:>11}{:>11}{:>11}\n{:>11}{:>11}{:>11}{:>11}{:>11}{:>11}\n",
            0, 0, 0, 0, "0.0", "0.0", 0, 0, 0, 6, "1.0", "2.0+7", 0, 0, 10, 8
        )
        .into_bytes()
    };
    let data = NuclearData::from_endf_bytes(
        "headers only",
        [
            ("u234".to_string(), header("9.223400+4", "2.320304+2")),
            ("u235".to_string(), header("9.223500+4", "2.330248+2")),
            ("u238".to_string(), header("9.223800+4", "2.360058+2")),
        ],
    )
    .expect("headers");
    let mut m = Model::new(data);
    let core = m.component("Core");
    let heu = Material::from_atom_densities(
        godiva::NUCLIDES
            .iter()
            .map(|(n, _, d)| (Isotope::from_name(n).unwrap(), atoms_per_barn_cm(*d))),
    );
    m.add(
        Sphere::radius(Length::new::<centimeter>(godiva::RADIUS_CM))
            .filled_with(heu)
            .in_component(core),
    );
    m.set_temperature(Temperature::Uniform(
        ThermodynamicTemperature::new::<kelvin>(godiva::TEMPERATURE_K),
    ));
    let nuclides: Vec<Nuclide> = ["U234", "U235", "U238"]
        .iter()
        .map(|n| Nuclide::from_core(n).expect("CORE nuclide"))
        .collect();
    let settings = KeffSettings::new(
        MCSettings::Custom(CustomRun {
            particles: 1_000,
            inactive: 5,
            active: 20,
            seed: 1,
        }),
        Some(Diagnostics::reactor_physics()),
    );
    let run = m
        .assemble()
        .expect("assembles")
        .keff_with(&nuclides, &settings)
        .expect("runs");
    println!("k = {}", run.keff());

    let report = run.diagnostics_report_titled("Godiva (LOW tier, in memory)");
    let zip = report.to_zip_bytes().expect("zip");
    let back = DiagnosticsReport::read_zip(&zip).expect("reads back, every CRC checked");
    assert_eq!(back, report.files());
    let mut pngs = 0;
    for f in &back {
        if f.path.ends_with(".png") {
            decode_png(&f.bytes).expect("a PNG that decodes");
            pngs += 1;
        }
    }
    println!(
        "zip {} bytes, {} files, {pngs} PNGs decoded:",
        zip.len(),
        back.len()
    );
    for f in &back {
        println!("  {} ({} bytes)", f.path, f.bytes.len());
    }
}
