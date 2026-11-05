//! gh:#1178: generate the double-pipe HF → LF dataset and the 1-D
//! comparison (`multifidelity::double_pipe`).
//!
//! ```text
//! cargo run --release -p outram-park-digital-twin-engine --example hf_lf_double_pipe_dataset -- [--threads N] [--one-d-only] [--mesh-check]
//! ```
//!
//! Writes, in `verification_and_validation/hf_lf_double_pipe_gh1178/`:
//! `cfd_dataset.csv` and `cfd_profiles.csv` (unless `--one-d-only`, which
//! reads the committed dataset instead), `one_d_results.csv`, and prints a
//! Markdown table of the disagreements. `--mesh-check` runs only the mesh
//! check (V&V record, "Mesh") and prints it.
//!
//! Command-line glue around library functions the tests reach
//! (`tests/hf_lf_double_pipe_gh1178.rs`); not itself tested (root
//! `CLAUDE.md`: `main`/CLI glue is the stated exception).
use outram_park_digital_twin_engine::multifidelity::double_pipe::cfd::{self, CfdMesh};
use outram_park_digital_twin_engine::multifidelity::double_pipe::{dataset, design, study};

fn progress(s: &str) {
    eprintln!("{s}");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let threads = args.iter().position(|a| a == "--threads").and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(3);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/hf_lf_double_pipe_gh1178");
    std::fs::create_dir_all(&dir).expect("output directory");
    if args.iter().any(|a| a == "--mesh-check") {
        let x = design::mesh_check_point();
        for (name, mesh) in [("default", CfdMesh::default_grid()), ("fine", CfdMesh::fine_check())] {
            let r = cfd::run(&x, mesh).expect("mesh check");
            println!("{name}: cells {:?} layers {} | {:?} | nu_mesh {:?} | {:.1} s", r.cells, r.layers, r.output, r.nu_mesh, r.wall_seconds);
        }
        return;
    }
    let csv = if args.iter().any(|a| a == "--one-d-only") {
        std::fs::read_to_string(dir.join("cfd_dataset.csv")).expect("committed dataset")
    } else {
        let pts = design::design_grid();
        let recs = study::run_cfd_design(&pts, CfdMesh::default_grid(), threads, progress).expect("CFD design");
        let csv = dataset::write_records(&recs);
        std::fs::write(dir.join("cfd_dataset.csv"), &csv).expect("write dataset");
        std::fs::write(dir.join("cfd_profiles.csv"), dataset::write_profiles(&recs)).expect("write profiles");
        csv
    };
    let rows = dataset::parse_records(&csv).expect("parse dataset");
    let cmp = study::compare(&rows, threads).expect("1-D comparison");
    std::fs::write(dir.join("one_d_results.csv"), study::comparison_csv(&cmp)).expect("write 1-D results");
    println!("{}", study::markdown_summary(&cmp));
}
