//! **The numbers of the transport lesson "At a collision"** (gh:#906): written
//! to `docs/site/science/transport/collision-data.js`.
//!
//! ```bash
//! cargo run --release -p outram-mc-libs --example lesson_collision [out.js]
//! ```
//!
//! For Godiva's material (`vv::godiva`: HEU-MET-FAST-001, U-234, U-235 and
//! U-238 at the atom densities of the five-route study, 293.6 K, ENDF/B-VIII.0
//! read from `reference-data/endf/` through the crate's ordinary constructor,
//! so URR and DBRC are on) at a few neutron energies:
//!
//! - **which nuclide**: each nuclide's share of the macroscopic total cross
//!   section, `N_i sigma_t,i / Sigma_t`, which is what
//!   `Material::sample_nuclide` weights the draw by (the unresolved-resonance
//!   band is not drawn here: these are the infinitely dilute values
//!   `Nuclide::xs_at_energy` returns; U-238 is unresolved only from about
//!   20 keV, so only the 0.1 MeV row is in that range);
//! - **which reaction**: that nuclide's partial cross sections, in the order
//!   the transport kernel's ladder takes them (fission, then the rest of
//!   absorption, inelastic, (n,2n), elastic last), as microscopic barns.
//!
//! The mean free path `1/Sigma_t` is written too. Nothing here is tuned or
//! compared with a target: it is the data path's own output.
//!
//! Driver glue (`main`): every function it calls is tested in the crate.

use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::vv::godiva;
use std::fmt::Write as _;

fn main() -> Result<(), String> {
    let out = std::env::args().nth(1).unwrap_or_else(|| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/site/science/transport/collision-data.js").to_string()
    });
    let dir = njoy_outram_park_fork::reference_data::reference_endf_dir();
    let nuclides: Vec<Nuclide> = godiva::NUCLIDES
        .iter()
        .map(|&(name, tape, _)| {
            let p = njoy_outram_park_fork::reference_data::reference_endf(tape)
                .ok_or_else(|| format!("{tape} not found under {}", dir.display()))?;
            Nuclide::from_endf_file_with_speed(&p, name, godiva::TEMPERATURE_K, outram_mc_libs::vv::bench_speed())
                .map_err(|e| format!("{name}: {e}"))
        })
        .collect::<Result<_, _>>()?;
    let mat = godiva::material();
    let mut rows = String::new();
    for &e in &[1.0e5_f64, 1.0e6, 2.0e6] {
        let sigma_t = mat.macro_xs_total(e, &nuclides);
        let mut per = String::new();
        for (c, &(name, _, n)) in mat.components.iter().zip(godiva::NUCLIDES.iter()) {
            let x = nuclides[c.nuclide_idx].xs_at_energy(e, godiva::TEMPERATURE_K);
            let capture = (x.absorption - x.fission).max(0.0);
            println!(
                "E {e:.3e} eV {name}: share {:.4}, sigma_t {:.4} b = fission {:.4} + capture {:.4} + inelastic {:.4} + (n,2n) {:.4} + elastic {:.4}",
                c.atom_density * x.total / sigma_t,
                x.total,
                x.fission,
                capture,
                x.inelastic,
                x.n2n,
                x.elastic
            );
            let _ = write!(
                per,
                "\"{name}\":{{\"n\":{n},\"total\":{},\"fission\":{},\"capture\":{capture},\"inelastic\":{},\"n2n\":{},\"elastic\":{},\"other\":{}}},",
                x.total,
                x.fission,
                x.inelastic,
                x.n2n,
                x.elastic,
                x.total - x.fission - capture - x.inelastic - x.n2n - x.elastic
            );
        }
        per.pop();
        println!("E {e:.3e} eV: Sigma_t {sigma_t:.5} 1/cm, mean free path {:.3} cm", 1.0 / sigma_t);
        let _ = write!(rows, "\"{e}\":{{\"sigma_t\":{sigma_t},\"nuclides\":{{{per}}}}},");
    }
    rows.pop();
    let file = format!(
        "// Numbers for the transport lessons, \"At a collision\" (gh:#906), written by\n\
         // crates/outram-mc-libs/examples/lesson_collision.rs (regenerate with it; do not\n\
         // edit). Generated 2026-10-18. Godiva's material (vv::godiva: HEU-MET-FAST-001,\n\
         // U-234/235/238, atoms per barn-cm n, 293.6 K), ENDF/B-VIII.0 tapes read through\n\
         // Nuclide::from_endf_file_with_speed (URR and DBRC on, as the crate's default).\n\
         // Per neutron energy [eV]: sigma_t = Material::macro_xs_total [1/cm]; per nuclide\n\
         // the microscopic Nuclide::xs_at_energy channels [b]: total, fission, capture\n\
         // (absorption - fission), inelastic, n2n, elastic, and other (what is left of the\n\
         // total, for (n,3n) and the lumped channels). Infinitely dilute values.\n\
         window.COLLISION_DATA = {{\"material\":\"{}\",\"temperature\":{},\"energies\":{{{rows}}}}};\n",
        "Godiva HEU",
        godiva::TEMPERATURE_K
    );
    std::fs::write(&out, file).map_err(|e| e.to_string())?;
    println!("wrote {out}");
    Ok(())
}
