//! Bake the embedded CORE windowed-multipole blob (`src/data/wmp_core.wmpl`).
//!
//! This is the **offline data step**: it reads the MIT CRPG `WMP_Library` HDF5
//! files for the CORE nuclide set, serializes each with
//! [`WindowedMultipole::to_blob`], packs them into one WMPL v1 container with
//! [`WmpLibrary::pack`], and writes the result. The shipped crate then
//! `include_bytes!`s that blob (see `WmpLibrary::core`) — so the runtime build
//! needs no HDF5 and downloads nothing.
//!
//! Run it whenever the CORE set or the WMPB/WMPL format changes:
//!
//! ```text
//! cargo run --release --example bake_wmp --features wmp-hdf5 -- \
//!     /path/to/WMP_Library/WMP_Library \
//!     crates/njoy-outram-park-fork/src/data/wmp_core.wmpl
//! ```
//!
//! Both arguments are optional; the defaults match the maintainer layout above.
//!
//! # License
//! The `WMP_Library` HDF5 inputs and the baked blob are MIT CRPG data (© MIT).
//! See `LICENSE-WMP` and `NOTICE` — both must ship with the embedded blob.

fn main() {
    use njoy_outram_park_fork::wmp::{WindowedMultipole, WmpLibrary};
    use std::path::PathBuf;

    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(
        args.next()
            .unwrap_or_else(|| "/home/teddy0/Documents/research/WMP_Library/WMP_Library".into()),
    );
    let out = PathBuf::from(
        args.next()
            .unwrap_or_else(|| "crates/njoy-outram-park-fork/src/data/wmp_core.wmpl".into()),
    );

    // Load every CORE nuclide, in the manifest's order (which the container then
    // preserves). A missing or unreadable file is a hard error — a partial CORE
    // blob would silently drop nuclides from every downstream simulation.
    let mut nuclides: Vec<WindowedMultipole> = Vec::with_capacity(CORE_ZAIDS.len());
    for zaid in CORE_ZAIDS {
        let path = dir.join(format!("{zaid}.h5"));
        match WindowedMultipole::load_h5(&path) {
            Ok(w) => nuclides.push(w),
            Err(e) => {
                eprintln!("FATAL: {} ({})", path.display(), e);
                std::process::exit(1);
            }
        }
    }

    let image = WmpLibrary::pack(&nuclides);

    // Round-trip check before writing: the container must decode and expose every
    // nuclide we packed. Bake-time failure beats a corrupt embedded blob.
    let lib = WmpLibrary::from_blob(&image).expect("baked WMPL image must re-parse");
    assert_eq!(lib.len(), nuclides.len(), "container lost nuclides");
    for w in &nuclides {
        lib.get(&w.name)
            .unwrap_or_else(|e| panic!("re-decode {}: {e}", w.name));
    }

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).expect("create output directory");
    }
    std::fs::write(&out, &image).expect("write WMPL blob");

    let raw: usize = nuclides
        .iter()
        .map(|n| n.poles.len() * 8 * 8 + n.curvefit.iter().map(|w| w.len() * 3 * 8).sum::<usize>())
        .sum();
    println!("baked {} nuclides -> {}", nuclides.len(), out.display());
    println!(
        "  doubles payload ~{:.2} MB, WMPL blob {:.2} MB ({:.2}x)",
        raw as f64 / 1e6,
        image.len() as f64 / 1e6,
        raw as f64 / image.len() as f64
    );
}

/// The CORE nuclide set as `WMP_Library` ZAID stems (`ZZAAA`), matching
/// `docs/wmp-nuclide-manifest.md`. 125 nuclides: reactor-grade actinides,
/// structurals, moderators/coolants, control + burnable poisons, top fission
/// products, and the LFTR (FLiBe/FLiNaK + Th cycle) additions.
const CORE_ZAIDS: &[&str] = &[
    "001001", "001002", "002004", "003006", "003007", "004009", "005010", "005011", "006000",
    "007014", "007015", "008016", "008017", "009019", "011023", "013027", "014028", "014029",
    "014030", "019039", "019040", "019041", "024050", "024052", "024053", "024054", "025055",
    "026054", "026056", "026057", "026058", "028058", "028060", "028061", "028062", "028064",
    "036083", "040090", "040091", "040092", "040094", "040096", "041093", "042092", "042094",
    "042095", "042096", "042097", "042098", "042100", "043099", "045103", "047107", "047109",
    "048110", "048111", "048112", "048113", "048114", "048116", "049113", "049115", "050112",
    "050114", "050115", "050116", "050117", "050118", "050119", "050120", "050122", "050124",
    "053135", "054131", "054135", "055133", "055135", "060143", "060145", "061147", "061148",
    "061149", "062147", "062149", "062150", "062151", "062152", "063151", "063153", "063155",
    "064152", "064154", "064155", "064156", "064157", "064158", "064160", "072176", "072177",
    "072178", "072179", "072180", "090230", "090232", "091231", "091233", "092233", "092234",
    "092235", "092236", "092237", "092238", "093237", "093239", "094238", "094239", "094240",
    "094241", "094242", "095241", "095243", "096242", "096243", "096244", "096245",
];
