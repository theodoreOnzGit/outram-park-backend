// SPDX-License-Identifier: GPL-3.0

//! Write a transport-ready ACE **library** with this workspace's NJOY port —
//! the Rust-generated counterpart of an NJOY2016 `RECONR -> BROADR -> PURR ->
//! ACER` set, laid out so both OpenMC (after HDF5 conversion) and
//! `outram_mc_libs::Nuclide::from_ace_file` can read it.
//!
//! # Layout written
//!
//! ```text
//! <out>/293.6K/<name>.ace   RECONR(0 K, tol) -> BROADR(T) -> HEATR -> PURR -> ACER
//! <out>/0K/<name>.ace       RECONR(0 K, tol) -> HEATR -> ACER                (kT = 0)
//! <out>/293.6K/HinH2O.ace   THERMR/ACER port on a thermal tape (thermal_from_mf7), optional
//! ```
//!
//! **Both fast tables come from `process`** (GitHub #930): one
//! `process::reconstruct`, then `process::cross_sections_at(..).to_ace()` at
//! 0 K and at `--temp`, which is `process::ace` with RECONR shared. ~~They
//! were made by `acer::build_full_with_purr` (hot) and `acer::build_full`
//! (`0K/`).~~ **CHANGED 2026-10-17:** the A/B diff below shows the switch is
//! a no-op. Both old calls end in the same `acer::assemble` as `process`.
//!
//! **A/B, 2026-10-17** (`--temp 293.6`, default `--tol`, `--thermal-leapr
//! HinH2O`): this example built before the switch and
//! after it wrote the library for the 13 five-route nuclides (U-234/235/236/
//! 238, F-19, O-16, H-1, Al-27, Si-28/29/30, Mn-55, B-10) plus Zn-70, and
//! the two were compared with `cmp`. **All 29 Type-1 files are identical
//! byte for byte** (14 at 293.6 K, 14 in `0K/`, and H in H₂O), 1 008 622 911 bytes in
//! all. With `OUTRAM_ACE_TYPE2_ALSO=1` the six Type-2 files of U-234, Zn-70
//! and B-10 are identical too, so the doubles agree to the bit, not only to
//! 12 printed figures. Wall time is unchanged (U-235 85.0 s before, 85.4 s
//! after; i9-13900K, one thread each, two runs side by side). The tape's MAT
//! is now checked against the one given on `--nuclide`.
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
//! - **HEATR is on** (`AceDeck` default). It changes ~~only the ESZ heating
//!   column~~ the ESZ heating column and (CORRECTED 2026-10-15) PURR's
//!   probability-table heating column, neither of which an eigenvalue
//!   calculation reads. Bit for bit NJOY2016's on the same deck with HEATR
//!   (`verification_and_validation/ace_five_route_parity_2026_10_15_library.md`).
//! - `OUTRAM_ACE_TYPE2_ALSO=1` also writes each table as Type 2 (`.ace2`),
//!   for a bit-for-bit comparison against an NJOY2016 Type-2 run.
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
//! # `--thermal-leapr`: the thermal table from a LEAPR deck (2026-10-14)
//!
//! Maintainer direction 2026-10-14: LEAPR always runs, and the published
//! `tsl-*.endf` tapes are oracles only. `--thermal-leapr
//! NAME:DECK:INCIDENT_TAPE:ZAID_STEM` (e.g.
//! `HinH2O:HinH2O:n-001_H_001-ENDF8.0-Beta6.endf:lwtr`) runs
//! `process::thermal_ace` on the ENDF/B-VIII.0 deck compiled into this crate
//! (`DECK` is its `SabMaterial::registry_key`) with the incident tape: LEAPR
//! at `--temp` on the deck's block for it, with the deck's own constants;
//! RECONR + BROADR of the incident tape; THERMR (tol 0.001, 16 cosines,
//! `natom` and `emax` from the law's `B(6)`/`B(4)`); ACER with 64 equally
//! probable outgoing energies (`iwt = 1`), what OpenMC's `make_ace_thermal`
//! asks NJOY2016 for. Its incident grid is ACER's own rule, so no NJOY table
//! is read. `--thermal` (below) stays for the published-tape A/B.
//!
//! # Usage
//!
//! ```text
//! cargo run --release -p njoy-outram-park-fork --example write_ace_library -- \
//!     --out <dir> --temp 293.6 \
//!     --nuclide U235:n-092_U_235-ENDF8.0.endf:9228 [--nuclide ...] \
//!     [--thermal-leapr HinH2O:HinH2O:n-001_H_001-ENDF8.0-Beta6.endf:lwtr]
//!     [--thermal HinH2O:tsl-HinH2O.endf:1:lwtr --thermal-grid-from <njoy HinH2O.ace>
//!      [--thermal-incident n-001_H_001-ENDF8.0-Beta6.endf:125]]
//! ```
//!
//! Tapes are resolved against `reference-data/endf/`. A table that already
//! exists is skipped, so an interrupted run resumes.

use std::path::{Path, PathBuf};
use std::time::Instant;

use njoy_outram_park_fork::acer::thermal::{ace_incident_grid, nxs as tnxs, jxs as tjxs, IncidentPendf, InelasticForm, ThermalAceOptions};
use njoy_outram_park_fork::acer::AceTable;
use njoy_outram_park_fork::broadr::broaden_result;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::process::{self, CrossSectionSettings, Reconstruction};
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::reference_endf_dir;
use njoy_outram_park_fork::thermr::mf7::parse_mf7_at_temperature;
use njoy_outram_park_fork::NjoyError;

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
    let tol_reconr = tol;
    let hot_dir = out.join(format!("{temp_k}K"));
    let cold_dir = out.join("0K");
    std::fs::create_dir_all(&hot_dir).expect("create hot dir");
    std::fs::create_dir_all(&cold_dir).expect("create 0K dir");
    // The fast tables' deck: process's defaults (HEATR on, ZAID `.00c`) with
    // RECONR at `--tol` and PURR at this library's 20 / 64 / 10 000.
    let settings = CrossSectionSettings::default()
        .with_reconr_tolerance(tol)
        .with_purr(PURR_NBIN, PURR_NLADR, PURR_NSAMP);

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
        // The tape's own MAT must be the one the caller named: a wrong MAT
        // would otherwise label the table with another nuclide's number.
        let tape_mat = process::single_material(&tape).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert_eq!(tape_mat, mat, "{file} holds MAT {tape_mat}, not the {mat} given for {name}");
        // One RECONR (process::reconstruct) serves both tables (GitHub #930).
        let recon0 = process::reconstruct(&tape, &settings)
            .unwrap_or_else(|e| panic!("RECONR {name}: {e}"));
        if !cold.is_file() {
            let t = ace_at(&recon0, 0.0, &settings).unwrap_or_else(|e| panic!("ACER 0 K {name}: {e}"));
            write(&t, &cold);
        }
        if !hot.is_file() {
            let t = ace_at(&recon0, temp_k, &settings)
                .unwrap_or_else(|e| panic!("ACER {temp_k} K {name}: {e}"));
            write(&t, &hot);
        }
        eprintln!(
            "  {name}: 0 K + {temp_k} K (PURR {PURR_NBIN}/{PURR_NLADR}/{PURR_NSAMP}) in {:.1} s",
            t0.elapsed().as_secs_f64()
        );
    }

    if let Some(spec) = flag(&args, "--thermal-leapr") {
        thermal_from_leapr(&spec, &hot_dir, temp_k);
        return;
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
        // THERMR card-4 `tol`: 0.001, what OpenMC's `make_ace_thermal` passes and
        // therefore what the NJOY2016 reference table was built with. The
        // writer's own default (0.05) left this table 50x coarser than the one
        // it is compared against (found 2026-09-29).
        let tol: f64 =
            flag(&args, "--thermal-tol").map_or(1.0e-3, |v| v.parse().expect("--thermal-tol"));
        eprintln!("    THERMR tol {tol}");
        // With `--thermal-incident TAPE:MAT` (2026-10-14, GitHub #865) the
        // table is built as NJOY's deck builds it: on the incident nuclide's
        // PENDF at this temperature, with emax the tape's B(4) as OpenMC's
        // `make_ace_thermal` passes it. Word for word NJOY2016's on the cases
        // in `tests/acer_thermal_word_parity.rs`. Without it, ITIX is not on
        // that grid.
        let t = match flag(&args, "--thermal-incident") {
            Some(inc) => {
                let [itape, imat] = inc.split(':').collect::<Vec<_>>()[..] else {
                    panic!("--thermal-incident wants TAPE:MAT, got {inc}")
                };
                let imat: i32 = imat.parse().expect("incident MAT");
                let it = Tape::read_file(&reference_endf_dir().join(itape))
                    .unwrap_or_else(|e| panic!("read {itape}: {e}"));
                let r = broaden_result(
                    &reconr(&it, &ReconrConfig { mat: imat, tolerance: tol_reconr, temperature: 0.0 })
                        .unwrap_or_else(|e| panic!("RECONR {itape}: {e}")),
                    temp_k,
                );
                let elastic = r.sections.iter().find(|s| s.mt.number() == 2).expect("incident MT=2").pairs.clone();
                let emax = ii.b.get(3).copied().filter(|v| *v > 0.0).expect("MF=7/MT=4 B(4) emax");
                let grid = ace_incident_grid(emax);
                if grid != grid_ev {
                    eprintln!("    WARNING: ACER's grid to B(4) = {emax} eV ({} points) is not NJOY2016's ITIE ({} points); using NJOY's", grid.len(), grid_ev.len());
                }
                let opts = ThermalAceOptions { emax_ev: emax, ..opts };
                let pendf = IncidentPendf {
                    mat: imat,
                    awr: r.material.awr,
                    moderator_za: vec![r.material.za as i32],
                    elastic,
                };
                eprintln!("    on the PENDF of {itape} (MAT {imat}), emax {emax} eV");
                AceTable::thermal_from_mf7_on_pendf(&mf7, temp_k, stem, 0, &grid_ev, opts, tol, &pendf)
            }
            None => AceTable::thermal_from_mf7_with_tolerance(&mf7, temp_k, stem, 0, &grid_ev, opts, tol),
        }
        .unwrap_or_else(|e| panic!("thermal ACE {name}: {e:?}"));
        write(&t, &path);
        eprintln!(
            "  {name}: thermal table in {:.1} s",
            t0.elapsed().as_secs_f64()
        );
    }
}

/// `--thermal-leapr NAME:DECK:INCIDENT_TAPE:ZAID_STEM`: the thermal table by
/// `process::thermal_ace` from the LEAPR deck registered as `DECK` (see the
/// module doc). CLI glue; `process::thermal_ace` itself is gated against
/// NJOY2016 by `tests/process_api.rs`.
fn thermal_from_leapr(spec: &str, hot_dir: &Path, temp_k: f64) {
    use njoy_outram_park_fork::leapr::decks::{embedded_deck_text, SabMaterial};
    use njoy_outram_park_fork::process::{thermal_ace, ThermalSettings};
    use njoy_outram_park_fork::units::Temperature;
    use uom::si::thermodynamic_temperature::kelvin;
    let [name, deck_key, itape, stem] = spec.split(':').collect::<Vec<_>>()[..] else {
        panic!("--thermal-leapr wants NAME:DECK:INCIDENT_TAPE:ZAID_STEM, got {spec}")
    };
    let path = hot_dir.join(format!("{name}.ace"));
    if path.is_file() {
        eprintln!("  {name}: present, skipped");
        return;
    }
    let material = SabMaterial::all()
        .into_iter()
        .find(|m| m.registry_key() == deck_key)
        .unwrap_or_else(|| panic!("no LEAPR deck registered as {deck_key}"));
    let deck = embedded_deck_text(material).unwrap_or_else(|| panic!("{deck_key}: deck not embedded"));
    let t0 = Instant::now();
    let incident = Tape::read_file(&reference_endf_dir().join(itape)).unwrap_or_else(|e| panic!("read {itape}: {e}"));
    let settings = ThermalSettings::default()
        .with_outgoing_energies(64)
        .with_form(InelasticForm::Equiprobable)
        .with_table_name(stem);
    let t = thermal_ace(&incident, deck, Temperature::new::<kelvin>(temp_k), &settings)
        .unwrap_or_else(|e| panic!("LEAPR -> THERMR -> ACER {name}: {e:?}"));
    write(&t, &path);
    eprintln!(
        "  {name}: thermal table from the {deck_key} LEAPR deck at {temp_k} K on {itape} (64 x 16, iwt = 1) in {:.1} s",
        t0.elapsed().as_secs_f64()
    );
}

/// The fast table at `temp_k` from the held RECONR: `process::cross_sections_at`
/// then ACER, i.e. `process::ace` without repeating RECONR. At 0 K that is
/// NJOY's `RECONR -> (HEATR) -> ACER` deck (no BROADR, no PURR); above it
/// `RECONR -> BROADR -> HEATR -> PURR -> ACER`.
fn ace_at(recon0: &Reconstruction, temp_k: f64, settings: &CrossSectionSettings) -> Result<AceTable, NjoyError> {
    use njoy_outram_park_fork::units::Temperature;
    use uom::si::thermodynamic_temperature::kelvin;
    process::cross_sections_at(recon0, Temperature::new::<kelvin>(temp_k), settings)?.to_ace()
}

/// Write to a temporary name and rename, so an interrupted write never leaves a
/// truncated table that the skip-if-present logic would then trust.
fn write(t: &AceTable, path: &Path) {
    let tmp = path.with_extension("ace.partial");
    t.write_type1(&tmp)
        .unwrap_or_else(|e| panic!("write {}: {e}", tmp.display()));
    std::fs::rename(&tmp, path).expect("rename");
    // `OUTRAM_ACE_TYPE2_ALSO=1` also writes the table as Type 2 beside it
    // (`<name>.ace2`): the unrounded doubles, for a bit-for-bit comparison
    // against an NJOY2016 Type-2 run that Type-1 text cannot make.
    if std::env::var_os("OUTRAM_ACE_TYPE2_ALSO").is_some() {
        t.write_type2(path.with_extension("ace2")).expect("write Type 2");
    }
    eprintln!(
        "    wrote {} ({:.1} MB)",
        path.display(),
        std::fs::metadata(path).map_or(0, |m| m.len()) as f64 / 1e6
    );
}
