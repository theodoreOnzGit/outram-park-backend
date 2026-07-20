//! One-time extraction of the OpenMC U-238 capture (MT=102) reference for the
//! Doppler code-to-code verification (`tests/u238_doppler_verification/`).
//!
//! Reads OpenMC's ENDF/B-VIII.0 HDF5 library (`tests/resources/U238.h5`), samples
//! the pointwise radiative-capture cross section at 900 K and 1200 K onto a
//! **100,000-point log-spaced energy grid** (10⁻⁵ eV → 20 MeV), and writes
//! `tests/u238_doppler_verification/reference/openmc_capture_{900,1200}K.csv`
//! (columns `energy_eV, sigma_b`). At ~8100 points/decade the resolved
//! resonances (~0.2 eV wide) are well sampled (~100 points across each).
//!
//! Those CSVs are the *committed* reference — once they exist, the large
//! `U238.h5` (≈115 MB, too big for GitHub) can be deleted and the verification
//! test still runs. Re-extract (if the h5 is present again) with:
//!
//! ```bash
//! cargo run --release --example extract_u238_doppler_ref
//! ```

use hdf5_pure::File;
use std::io::Write;
use std::path::PathBuf;

/// Log-spaced sampling grid: 100,000 points, 10⁻⁵ eV → 20 MeV. Resonance-resolved
/// (~8100 pts/decade) while ~4.5× smaller than the native 448k-point 0 K grid.
const N_POINTS: usize = 100_000;
const E_LO: f64 = 1.0e-5; // eV (OpenMC grid minimum)
const E_HI: f64 = 2.0e7; // eV = 20 MeV (h5 data runs to 30 MeV)

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The shared log-spaced energy grid [eV].
fn log_energies() -> Vec<f64> {
    let (a, b) = (E_LO.ln(), E_HI.ln());
    (0..N_POINTS)
        .map(|i| (a + (b - a) * i as f64 / (N_POINTS - 1) as f64).exp())
        .collect()
}

/// Lin-lin interpolation of a sorted `(energy, sigma)` grid at `e` (OpenMC's own
/// interpolation law for continuous-energy cross sections). Clamps at the ends.
fn interp_linlin(grid: &[(f64, f64)], e: f64) -> f64 {
    if e <= grid[0].0 {
        return grid[0].1;
    }
    let n = grid.len();
    if e >= grid[n - 1].0 {
        return grid[n - 1].1;
    }
    let i = grid.partition_point(|&(x, _)| x < e); // first index with x >= e
    let (x0, y0) = grid[i - 1];
    let (x1, y1) = grid[i];
    y0 + (y1 - y0) * (e - x0) / (x1 - x0)
}

fn read_f64(file: &File, path: &str) -> Vec<f64> {
    file.dataset(path)
        .and_then(|d| d.read_f64())
        .unwrap_or_else(|e| panic!("read dataset {path}: {e}"))
}

/// OpenMC pointwise capture (MT=102) at `temp_k` [K] on its own union grid.
fn openmc_capture_fine(file: &File, temp_k: u32) -> Vec<(f64, f64)> {
    let energy = read_f64(file, &format!("/U238/energy/{temp_k}K"));
    let xs = read_f64(file, &format!("/U238/reactions/reaction_102/{temp_k}K/xs"));
    // MT=102 has threshold_idx = 0, so xs aligns 1:1 with the energy grid.
    assert_eq!(energy.len(), xs.len(), "capture xs/grid length mismatch at {temp_k}K");
    energy.into_iter().zip(xs).collect()
}

fn main() {
    let h5 = manifest().join("tests/resources/U238.h5");
    if !h5.exists() {
        eprintln!("U238.h5 not present at {} — nothing to extract.", h5.display());
        eprintln!("(The reference CSVs are committed; this extractor is only needed to regenerate them.)");
        std::process::exit(0);
    }

    let file = File::from_bytes(std::fs::read(&h5).expect("read U238.h5")).expect("parse U238.h5");
    let out_dir = manifest().join("tests/u238_doppler_verification/reference");
    std::fs::create_dir_all(&out_dir).expect("create reference dir");

    let grid = log_energies();
    for temp_k in [900u32, 1200u32] {
        let fine = openmc_capture_fine(&file, temp_k);
        let path = out_dir.join(format!("openmc_capture_{temp_k}K.csv"));
        let mut w = std::io::BufWriter::new(std::fs::File::create(&path).expect("create csv"));
        writeln!(w, "energy_eV,sigma_b").unwrap();
        for &e in &grid {
            writeln!(w, "{e:.6e},{:.6e}", interp_linlin(&fine, e)).unwrap();
        }
        w.flush().unwrap();
        println!(
            "wrote {} ({} points, {:.3e}–{:.3e} eV) from {} fine-grid points",
            path.display(),
            grid.len(),
            E_LO,
            E_HI,
            fine.len()
        );
    }
}
