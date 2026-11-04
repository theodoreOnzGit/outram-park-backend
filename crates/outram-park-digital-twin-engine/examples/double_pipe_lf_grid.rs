//! gh:#1190: run the TUAS 1-D double pipe at nominal θ on the low-fidelity
//! grid (`multifidelity::double_pipe::lf_grid`) and write
//! `verification_and_validation/double_pipe_mfk_gh1190/lf_grid.csv`.
//!
//! ```text
//! cargo run --release -p outram-park-digital-twin-engine --example double_pipe_lf_grid -- [--threads N]
//! ```
//!
//! Command-line glue around library functions the tests reach
//! (`lf_grid::tests::the_committed_low_fidelity_grid_regenerates`); not
//! itself tested (root `CLAUDE.md`: `main`/CLI glue is the stated exception).
use outram_park_digital_twin_engine::multifidelity::double_pipe::lf_grid;

fn progress(s: &str) {
    eprintln!("{s}");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let threads = args.iter().position(|a| a == "--threads").and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(2);
    let pts = lf_grid::design();
    let t0 = std::time::Instant::now();
    let out = lf_grid::run(&pts, threads, progress).expect("1-D grid");
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/double_pipe_mfk_gh1190");
    std::fs::create_dir_all(&dir).expect("output directory");
    std::fs::write(dir.join("lf_grid.csv"), lf_grid::write_csv(&pts, &out)).expect("write");
    eprintln!("{} points, {:.0} s wall on {threads} threads", pts.len(), t0.elapsed().as_secs_f64());
}
