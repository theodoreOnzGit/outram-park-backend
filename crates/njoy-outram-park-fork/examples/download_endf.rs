//! HIGH-fidelity data acquisition — download a raw ENDF tape from a pinned
//! upstream and reconstruct pointwise cross sections on device.
//!
//! This is the counterpart to the offline LOW tier (embedded WMP + fast MGXS):
//! instead of shipping compact data in-crate, it fetches the *authoritative* ENDF
//! evaluation from the hardcoded [`EndfLibrary`] upstream, caches it, and runs the
//! crate's RECONR to get fully resonance-reconstructed σ(E).
//!
//! Needs the network and the **`net-fetch`** feature:
//!
//! ```text
//! cargo run --release -p njoy-outram-park-fork --features net-fetch \
//!     --example download_endf -- U235 ENDF-B-VIII.0
//! ```
//!
//! Args (both optional):
//!   1. nuclide GNDS name (default `U235`)
//!   2. library: `ENDF-B-VIII.0` | `ENDF-B-VII.1` | `JEFF-3.3` | `JENDL-5`
//!      (default `ENDF-B-VIII.0`)
//!
//! The download is cached under the platform cache dir (or `OUTRAM_PARK_DATA_DIR`),
//! so re-running is instant and offline.

#[cfg(not(feature = "net-fetch"))]
fn main() {
    eprintln!(
        "this example needs the `net-fetch` feature:\n  \
         cargo run --release -p njoy-outram-park-fork --features net-fetch \
--example download_endf -- U235 ENDF-B-VIII.0"
    );
    std::process::exit(2);
}

#[cfg(feature = "net-fetch")]
fn main() {
    use njoy_outram_park_fork::acquire::{parse_nuclide, well_known_mat, EndfCache, EndfLibrary};
    use njoy_outram_park_fork::MtReaction;

    let mut args = std::env::args().skip(1);
    let name = args.next().unwrap_or_else(|| "U235".into());
    let library = match args.next().as_deref() {
        None | Some("ENDF-B-VIII.0") => EndfLibrary::EndfBVIII0,
        Some("ENDF-B-VII.1") => EndfLibrary::EndfBVII1,
        Some("JEFF-3.3") => EndfLibrary::Jeff33,
        Some("JENDL-5") => EndfLibrary::Jendl5,
        Some(other) => {
            eprintln!("unknown library '{other}'");
            std::process::exit(2);
        }
    };

    let (z, a, sym) = parse_nuclide(&name).unwrap_or_else(|e| {
        eprintln!("bad nuclide '{name}': {e}");
        std::process::exit(2);
    });
    let mat = well_known_mat(z, a).unwrap_or_else(|| {
        eprintln!(
            "MAT for {name} is not in the built-in table; only the shipped CORE \
             nuclides (H-1, O-16, Fe-56, Th-232, U-233/234/235/236/238, Pu-239/240/241) \
             can be addressed by name here."
        );
        std::process::exit(2);
    });

    let cache = EndfCache::new().expect("open cache dir");
    println!("Nuclide : {sym}-{a}  (Z={z}, MAT={mat})");
    println!("Library : {}", library.label());
    println!("URL     : {}", library.neutron_url(mat, z, a, sym));
    println!("Cache   : {}\n", cache.dir().display());

    println!("Fetching + reconstructing (RECONR, 0.1% tol, 0 K)…");
    let result = cache
        .download_and_reconstruct(library, mat, z, a, sym, 1.0e-3)
        .expect("download + reconstruct");

    // Report a few reconstructed cross sections across the fast range.
    println!("\n{sym}-{a} reconstructed pointwise σ(E):");
    println!("  {:>10}  {:>12}  {:>12}  {:>12}", "E [eV]", "σ_total", "σ_elastic", "σ_fission");
    for &e in &[1.0e-2, 1.0e0, 1.0e2, 1.0e4, 1.0e6, 2.0e6, 1.4e7] {
        let tot = result.eval_mt(MtReaction::Mt1Total, e);
        let ela = result.eval_mt(MtReaction::Mt2Elastic, e);
        let fis = result.eval_mt(MtReaction::Mt18Fission, e);
        println!("  {e:>10.2e}  {tot:>12.4}  {ela:>12.4}  {fis:>12.4}");
    }
    println!(
        "\nReconstructed {} MF=3 sections. Cached tape reused offline on re-run.",
        result.sections.len()
    );
}
