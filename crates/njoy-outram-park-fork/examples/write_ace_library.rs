// SPDX-License-Identifier: GPL-3.0

//! Write a transport-ready ACE **library** with this workspace's NJOY port —
//! the Rust-generated counterpart of an NJOY2016 `RECONR -> BROADR -> PURR ->
//! ACER` set, laid out so both OpenMC (after HDF5 conversion) and
//! `outram_mc_libs::Nuclide::from_ace_file` can read it.
//!
//! # Layout written
//!
//! ```text
//! <out>/293.6K/<name>.ace   RECONR(0 K, tol) -> BROADR(T) -> PURR -> ACER   (build_full_with_purr)
//! <out>/0K/<name>.ace       RECONR(0 K, tol) -> ACER                        (build_full, kT = 0)
//! <out>/293.6K/HinH2O.ace   THERMR/ACER port on a thermal tape (thermal_from_mf7), optional
//! ```
//!
//! The `0K/` sibling is where `Nuclide::from_ace_file` looks for the
//! unbroadened elastic that DBRC needs (`zero_kelvin_companion_candidates`),
//! so a table read back through that entry point gets DBRC with no extra call.
//!
//! # The deck, and why each setting
//!
//! - **RECONR tolerance** `--tol` (default `0.001`), matching the NJOY2016 decks
//!   in `outram-mc-libs/verification_and_validation/openmc_godiva_cross_code/`.
//! - **PURR** `20 bins / 64 ladders`, the reference library's own card
//!   (`purr / MAT 1 1 20 64 /`), with `10 000` samples: the count
//!   `njoy-outram-park-fork`'s PURR-vs-NJOY verification uses (GitHub #325).
//!   `build_full_with_purr`'s docs record the bit-exact UNR-block comparison.
//! - **HEATR is on** (`AceDeck` default). It changes only the ESZ heating column,
//!   which no eigenvalue calculation reads.
//!
//! # The thermal table borrows NJOY2016's GRID and DIMENSIONS, not its data
//!
//! `AceTable::thermal_from_mf7` takes its incident-energy grid and bin counts
//! from the caller. `--thermal-grid-from <njoy.ace>` reads them out of an
//! NJOY2016 thermal table (ITIE, `NIEB`, `NIL`, `IFENG`), exactly as the
//! verified comparator `examples/thermal_ace_vs_njoy2016.rs` does, so the two
//! libraries differ in the *physics* computed on a grid and not in the grid.
//! Every number in the table (σ_inel, the emission bins) is computed by this
//! port from the tape.
//!
//! # Usage
//!
//! ```text
//! cargo run --release -p njoy-outram-park-fork --example write_ace_library -- \
//!     --out <dir> --temp 293.6 \
//!     --nuclide U235:n-092_U_235-ENDF8.0.endf:9228 [--nuclide ...] \
//!     [--thermal HinH2O:tsl-HinH2O.endf:1:lwtr --thermal-grid-from <njoy HinH2O.ace>]
//! ```
//!
//! Tapes are resolved against `reference-data/endf/`. A table that already
//! exists is skipped, so an interrupted run resumes.

use std::path::{Path, PathBuf};
use std::time::Instant;

use njoy_outram_park_fork::acer::thermal::{nxs as tnxs, jxs as tjxs, InelasticForm, ThermalAceOptions};
use njoy_outram_park_fork::acer::{build_full, build_full_with_purr, AceTable};
use njoy_outram_park_fork::broadr::broaden_result;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::reference_endf_dir;
use njoy_outram_park_fork::thermr::mf7::parse_mf7_at_temperature;

const K_B_MEV_PER_K: f64 = 8.617_333_262e-11;
const PURR_NBIN: usize = 20;
const PURR_NLADR: usize = 64;
const PURR_NSAMP: usize = 10_000;

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn flags(args: &[String], name: &str) -> Vec<String> {
    args.windows(2)
        .filter(|w| w[0] == name)
        .map(|w| w[1].clone())
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = PathBuf::from(flag(&args, "--out").expect("--out <dir> is required"));
    let temp_k: f64 = flag(&args, "--temp").map_or(293.6, |s| s.parse().expect("--temp"));
    let tol: f64 = flag(&args, "--tol").map_or(1.0e-3, |s| s.parse().expect("--tol"));
    let hot_dir = out.join(format!("{temp_k}K"));
    let cold_dir = out.join("0K");
    std::fs::create_dir_all(&hot_dir).expect("create hot dir");
    std::fs::create_dir_all(&cold_dir).expect("create 0K dir");
    let kt_mev = K_B_MEV_PER_K * temp_k;

    for spec in flags(&args, "--nuclide") {
        let parts: Vec<&str> = spec.split(':').collect();
        let [name, file, mat] = parts[..] else {
            panic!("--nuclide wants NAME:TAPE:MAT, got {spec}")
        };
        let mat: i32 = mat.parse().expect("MAT");
        let hot = hot_dir.join(format!("{name}.ace"));
        let cold = cold_dir.join(format!("{name}.ace"));
        if hot.is_file() && cold.is_file() {
            eprintln!("  {name}: both tables present, skipped");
            continue;
        }
        let t0 = Instant::now();
        let tape = Tape::read_file(&reference_endf_dir().join(file))
            .unwrap_or_else(|e| panic!("read {file}: {e}"));
        let recon0 = reconr(
            &tape,
            &ReconrConfig {
                mat,
                tolerance: tol,
                temperature: 0.0,
            },
        )
        .unwrap_or_else(|e| panic!("RECONR {name}: {e}"));
        if !cold.is_file() {
            let t = build_full(&tape, mat, &recon0, 0.0, 0)
                .unwrap_or_else(|e| panic!("ACER 0 K {name}: {e}"));
            write(&t, &cold);
        }
        if !hot.is_file() {
            let recon = broaden_result(&recon0, temp_k);
            let t = build_full_with_purr(
                &tape, mat, &recon, kt_mev, 0, PURR_NBIN, PURR_NLADR, PURR_NSAMP,
            )
            .unwrap_or_else(|e| panic!("ACER {temp_k} K {name}: {e}"));
            write(&t, &hot);
        }
        eprintln!(
            "  {name}: 0 K + {temp_k} K (PURR {PURR_NBIN}/{PURR_NLADR}/{PURR_NSAMP}) in {:.1} s",
            t0.elapsed().as_secs_f64()
        );
    }

    if let Some(spec) = flag(&args, "--thermal") {
        let parts: Vec<&str> = spec.split(':').collect();
        let [name, file, mat, stem] = parts[..] else {
            panic!("--thermal wants NAME:TSL_TAPE:MAT:ZAID_STEM, got {spec}")
        };
        let mat: i32 = mat.parse().expect("MAT");
        let path = hot_dir.join(format!("{name}.ace"));
        if path.is_file() {
            eprintln!("  {name}: present, skipped");
            return;
        }
        let grid_src = flag(&args, "--thermal-grid-from")
            .expect("--thermal needs --thermal-grid-from <NJOY2016 thermal ACE>");
        let njoy = njoy_outram_park_fork::acer::read::read(&grid_src)
            .unwrap_or_else(|e| panic!("read {grid_src}: {e}"));
        let (x, j, n) = (&njoy.xss, &njoy.jxs, &njoy.nxs);
        let form = match n[tnxs::IFENG] {
            0 => InelasticForm::Equiprobable,
            1 => InelasticForm::Skewed,
            2 => InelasticForm::Continuous,
            other => panic!("IFENG = {other} is not defined by aceth.f90"),
        };
        let nil = n[tnxs::NIL];
        let nang = match form {
            InelasticForm::Continuous => (nil - 1) as usize,
            _ => (nil + 1) as usize,
        };
        let itie = (j[tjxs::ITIE] - 1) as usize;
        let nei = x[itie] as usize;
        let grid_ev: Vec<f64> = x[itie + 1..itie + 1 + nei]
            .iter()
            .map(|e| e * 1.0e6)
            .collect();
        let t0 = Instant::now();
        let tape = Tape::read_file(&reference_endf_dir().join(file))
            .unwrap_or_else(|e| panic!("read {file}: {e}"));
        // `parse_mf7` would return the tape's BASE-temperature S(a,b) (283.6 K
        // for H in H2O), and `thermal_from_mf7` would then evaluate it with
        // 293.6 K kinematics: a table that is neither temperature. Measured on
        // the first build of this library: sigma_inel 1.68x NJOY2016's at
        // 1e-5 eV. The tables must be resolved at the requested temperature.
        let mf7 = parse_mf7_at_temperature(&tape, mat, Some(temp_k))
            .unwrap_or_else(|e| panic!("MF=7 {file} at {temp_k} K: {e:?}"));
        let ii = mf7
            .incoherent_inelastic
            .as_ref()
            .expect("MF=7/MT=4 incoherent inelastic");
        eprintln!(
            "    MF=7 S(a,b) resolved at {} K (requested {temp_k} K)",
            ii.temperature_k
        );
        // `natom` is the evaluation's own B(6), the number of principal atoms,
        // which is what THERMR is given on its card (OpenMC's
        // `make_ace_thermal` reads the same B(6) off MF=7/MT=4). H in H2O has
        // B(6) = 2; passing 1 doubles sigma_b / natom and with it sigma_inel --
        // measured on this library's second build: 2.0x NJOY2016 across the grid.
        let natom = ii.b.get(5).copied().filter(|v| *v > 0.0).unwrap_or(1.0);
        let opts = ThermalAceOptions {
            n_outgoing: n[tnxs::NIEB] as usize,
            n_cosines: nang,
            natom,
            emax_ev: *grid_ev.last().expect("non-empty grid"),
            form,
        };
        eprintln!(
            "  {name}: NJOY2016 grid {nei} points to {:.3} eV, NIEB {} x {nang} cosines, {form:?}, natom {natom}",
            opts.emax_ev, opts.n_outgoing
        );
        let t = AceTable::thermal_from_mf7(&mf7, temp_k, stem, 0, &grid_ev, opts)
            .unwrap_or_else(|e| panic!("thermal ACE {name}: {e:?}"));
        write(&t, &path);
        eprintln!(
            "  {name}: thermal table in {:.1} s",
            t0.elapsed().as_secs_f64()
        );
    }
}

/// Write to a temporary name and rename, so an interrupted write never leaves a
/// truncated table that the skip-if-present logic would then trust.
fn write(t: &AceTable, path: &Path) {
    let tmp = path.with_extension("ace.partial");
    t.write_type1(&tmp)
        .unwrap_or_else(|e| panic!("write {}: {e}", tmp.display()));
    std::fs::rename(&tmp, path).expect("rename");
    eprintln!(
        "    wrote {} ({:.1} MB)",
        path.display(),
        std::fs::metadata(path).map_or(0, |m| m.len()) as f64 / 1e6
    );
}
