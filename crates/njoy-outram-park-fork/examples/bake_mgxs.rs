//! Bake the embedded fast-range MGXS blob (`src/data/mgxs_core.mgxl`).
//!
//! The **offline data step** for the high-energy "ACE-lite" fallback. For each
//! CORE nuclide it:
//!   1. finds the ENDF/B-VIII.0 neutron tape in the local library,
//!   2. reconstructs the **MF=3 background only** ([`reconr_background`] — no
//!      resonance formats, which are irrelevant above WMP's `e_max`),
//!   3. reads the nuclide's WMP `e_max` from the embedded CORE WMP library, so the
//!      fast MGXS set begins exactly where the multipole form ends,
//!   4. parses ν̄(E) from MF=1/MT=452, and
//!   5. collapses to 10 log-spaced groups (`e_max` → 20 MeV) with a Watt weight
//!      ([`Mgxs::collapse_from_reconr`]).
//! The sets are packed into one MGXL v1 container and written out; the shipped
//! crate `include_bytes!`s it (see `MgxsLibrary::core`).
//!
//! ```text
//! cargo run --release --example bake_mgxs -- \
//!     /home/teddy0/Documents/research/ENDF-B-VIII.0/neutrons \
//!     crates/njoy-outram-park-fork/src/data/mgxs_core.mgxl
//! ```
//! Both arguments are optional; the defaults match the maintainer layout.
//!
//! # License / data provenance
//! ENDF/B-VIII.0 is public evaluated data (NNDC/CSEWG). The baked MGXS blob is a
//! derived, heavily reduced group-collapse — not the ENDF files themselves.

use njoy_outram_park_fork::{
    nuclear_data::{
        secondary::FissionSpectrum, secondary::NuBar, Mgxs, MgxsLibrary, ENDF_MAX_ENERGY_EV,
    },
    reconr::reconr_background,
    wmp::WmpLibrary,
    endf::tape::Tape,
};
use std::fs::File;
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| {
        "/home/teddy0/Documents/research/ENDF-B-VIII.0/neutrons".into()
    }));
    let out = PathBuf::from(
        args.next()
            .unwrap_or_else(|| "crates/njoy-outram-park-fork/src/data/mgxs_core.mgxl".into()),
    );

    let wmp = WmpLibrary::core();
    let spectrum = FissionSpectrum::default();

    // Index the neutron directory once: map (Z, A) -> path by parsing filenames
    // of the form `n-<ZZZ>_<Sym>_<AAA>.endf`.
    let mut by_za: std::collections::HashMap<(u32, u32), (PathBuf, String)> =
        std::collections::HashMap::new();
    for entry in std::fs::read_dir(&dir).expect("read ENDF neutron dir") {
        let path = entry.unwrap().path();
        let fname = match path.file_name().and_then(|s| s.to_str()) {
            Some(f) if f.starts_with("n-") && f.ends_with(".endf") => f,
            _ => continue,
        };
        // n-092_U_238.endf  ->  ["n-092", "U", "238.endf"]
        let stem = &fname[2..fname.len() - 5]; // strip "n-" and ".endf"
        let parts: Vec<&str> = stem.split('_').collect();
        if parts.len() != 3 {
            continue;
        }
        let (z, sym, a) = (parts[0], parts[1], parts[2]);
        if let (Ok(z), Ok(a)) = (z.parse::<u32>(), a.parse::<u32>()) {
            by_za.insert((z, a), (path.clone(), sym.to_string()));
        }
    }

    let mut baked: Vec<Mgxs> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    for zaid in CORE_ZAIDS {
        let z: u32 = zaid[..3].parse().unwrap();
        let a: u32 = zaid[3..].parse().unwrap();

        // ENDF/B-VIII.0 has no C-nat evaluation (006000): use C-12 (98.9%).
        let (path, sym) = match by_za.get(&(z, a)).or_else(|| {
            if a == 0 && z == 6 {
                by_za.get(&(6, 12))
            } else {
                None
            }
        }) {
            Some(p) => p.clone(),
            None => {
                skipped.push(format!("{zaid} (no ENDF tape)"));
                continue;
            }
        };

        // WMP name = symbol + A with leading zeros stripped (e.g. "U238", "H1").
        let name = format!("{sym}{a}");
        let e_max = match wmp.get(&name) {
            Ok(w) => w.e_max,
            Err(_) => {
                skipped.push(format!("{name} (not in WMP CORE — no e_max)"));
                continue;
            }
        };
        // Non-resonant nuclides whose WMP already spans the full sublibrary
        // (e_max = 20 MeV, e.g. H-1) have no fast gap to fill — WMP covers them.
        if e_max >= ENDF_MAX_ENERGY_EV {
            skipped.push(format!("{name} (WMP covers to 20 MeV — no fast range)"));
            continue;
        }

        let tape = match File::open(&path).map_err(|e| e.to_string()).and_then(|f| {
            Tape::read(f).map_err(|e| e.to_string())
        }) {
            Ok(t) => t,
            Err(e) => {
                skipped.push(format!("{name} (tape read: {e})"));
                continue;
            }
        };
        let mat = match tape.sections().iter().map(|s| s.key.mat).find(|&m| m > 0) {
            Some(m) => m,
            None => {
                skipped.push(format!("{name} (no MAT)"));
                continue;
            }
        };

        let res = match reconr_background(&tape, mat, 0.001) {
            Ok(r) => r,
            Err(e) => {
                skipped.push(format!("{name} (reconr: {e})"));
                continue;
            }
        };
        let nu = NuBar::from_endf(&tape, mat).ok().flatten().unwrap_or_default();

        let mg = Mgxs::collapse_from_reconr(&res, name.clone(), e_max, &nu, &spectrum);
        baked.push(mg);
    }

    let image = MgxsLibrary::pack(&baked);
    // Round-trip before writing.
    let lib = MgxsLibrary::from_blob(&image).expect("baked MGXL must re-parse");
    assert_eq!(lib.len(), baked.len());

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).expect("create output dir");
    }
    std::fs::write(&out, &image).expect("write MGXL blob");

    println!(
        "baked {} fast-MGXS nuclides -> {} ({:.1} KB)",
        baked.len(),
        out.display(),
        image.len() as f64 / 1024.0
    );
    if !skipped.is_empty() {
        println!("skipped {} nuclides:", skipped.len());
        for s in &skipped {
            println!("  - {s}");
        }
    }
}

/// The CORE nuclide set as ZAID stems (`ZZZAAA`), matching `bake_wmp.rs` and
/// `docs/wmp-nuclide-manifest.md`.
const CORE_ZAIDS: &[&str] = &[
    "001001", "001002", "002004", "003006", "003007", "004009", "005010", "005011",
    "006000", "007014", "007015", "008016", "008017", "009019", "011023", "013027",
    "014028", "014029", "014030", "019039", "019040", "019041", "024050", "024052",
    "024053", "024054", "025055", "026054", "026056", "026057", "026058", "028058",
    "028060", "028061", "028062", "028064", "036083", "040090", "040091", "040092",
    "040094", "040096", "041093", "042092", "042094", "042095", "042096", "042097",
    "042098", "042100", "043099", "045103", "047107", "047109", "048110", "048111",
    "048112", "048113", "048114", "048116", "049113", "049115", "050112", "050114",
    "050115", "050116", "050117", "050118", "050119", "050120", "050122", "050124",
    "053135", "054131", "054135", "055133", "055135", "060143", "060145", "061147",
    "061148", "061149", "062147", "062149", "062150", "062151", "062152", "063151",
    "063153", "063155", "064152", "064154", "064155", "064156", "064157", "064158",
    "064160", "072176", "072177", "072178", "072179", "072180", "090230", "090232",
    "091231", "091233", "092233", "092234", "092235", "092236", "092237", "092238",
    "093237", "093239", "094238", "094239", "094240", "094241", "094242", "095241",
    "095243", "096242", "096243", "096244", "096245",
];
