//! Compare this crate's reconstructed U-238 cross sections against **NJOY2016's
//! own PENDF** for the same tape, temperature and tolerance.
//!
//! # Why
//!
//! The FHR ring-RPT pebble sits ~+1700 pcm above the OpenMC reference, and the
//! offset is shared by both the explicit-TRISO and ring-RPT pebbles (they differ
//! by 194 pcm, inside noise), so it is a property of the cross sections rather
//! than of the pebble model (`op-mzvp.2.12`).
//!
//! The obvious oracle — the NNDC HDF5 library the OpenMC deck used — is
//! unreachable from this environment (403). NJOY2016 is not: it is built
//! in-session, and this crate is a port of it, so "does our reconstruction match
//! NJOY's on the same input?" is both answerable and exactly the right question.
//!
//! # Generating the oracle
//!
//! ```text
//! cd /home/user/u238oracle && cp .../n-092_U_238.endf tape20
//! njoy <<'EOF'
//! reconr
//!  20 21/
//!  'pendf for u238, err 0.001'/
//!  9237 0/
//!  0.001/
//!  0/
//! broadr
//!  20 21 22/
//!  9237 1/
//!  0.001/
//!  600./
//!  0/
//! stop
//! EOF
//! ```
//!
//! `tape22` is then the 600 K PENDF. Point `U238_PENDF` at it:
//!
//! ```text
//! U238_PENDF=/home/user/u238oracle/tape22 cargo run --release \
//!     -p outram-mc-libs --features endf-pebble-cases --example u238_vs_njoy_pendf
//! ```
//!
//! A disagreement here is a port defect. Agreement means our data matches the
//! code we are a port of, and the OpenMC offset lives somewhere else — either in
//! transport, or in the difference between RECONR-on-device and the NNDC
//! processing chain that produced OpenMC's library.

fn main() {
    use njoy_outram_park_fork::endf::tape::Tape;
    use njoy_outram_park_fork::groupr::panel::PointwiseXs;
    use njoy_outram_park_fork::groupr::pendf_feed::read_pendf_cross_section;
    use njoy_outram_park_fork::reference_data::reference_endf;
    use outram_mc_libs::material::nuclide::Nuclide;

    // Defaults are U-238; override to check another nuclide against its own
    // NJOY PENDF (U-235 in particular -- it is the fissile driver, so an error
    // there moves k directly, and checking only U-238 would have missed it).
    let mat: i32 = std::env::var("NJOY_MAT").ok().and_then(|v| v.parse().ok()).unwrap_or(9237);
    let tape_name = std::env::var("NJOY_TAPE").unwrap_or_else(|_| "n-092_U_238.endf".into());
    let nuc_name = std::env::var("NJOY_NUCLIDE").unwrap_or_else(|_| "U238".into());
    const TEMP: f64 = 600.0;

    let pendf_path = match std::env::var("U238_PENDF") {
        Ok(p) => p,
        Err(_) => {
            eprintln!("SKIP: set U238_PENDF to an NJOY 600 K PENDF (see the module docs)");
            return;
        }
    };

    eprintln!("reading NJOY PENDF {pendf_path} ...");
    let pendf = Tape::read_file(std::path::Path::new(&pendf_path)).expect("PENDF parses");

    let tape = reference_endf(&tape_name).expect("ENDF tape");
    eprintln!("reconstructing {nuc_name} (MAT {mat}) with this crate @ {TEMP} K, tol 1e-3 ...");
    let ours = Nuclide::from_endf_file(&tape, &nuc_name, TEMP, 1.0e-3).expect("reconstruction");
    eprintln!("done.\n");

    // (MT, label, how to pull it out of our MicroXS)
    let reactions: [(i32, &str); 4] = [
        (1, "total"),
        (2, "elastic"),
        (18, "fission"),
        (102, "capture"),
    ];

    // Energies spanning thermal, the big low-lying resonances, the
    // resolved/unresolved seam, the URR band and fast.
    const PROBE_EV: &[f64] = &[
        0.0253, 1.0, 6.674, 20.87, 36.68, 66.03, 102.6, 1.0e3, 1.0e4, 1.9e4, 2.0e4,
        3.0e4, 5.0e4, 1.0e5, 1.5e5, 5.0e5, 1.0e6, 2.0e6, 1.4e7,
    ];

    for (mt, label) in reactions {
        let njoy = match read_pendf_cross_section(&pendf, mat, mt) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("== MT={mt} ({label}): NJOY PENDF has no such section ({e:?})");
                continue;
            }
        };
        let PointwiseXs::LinLin(pairs) = &njoy.xs else {
            eprintln!("== MT={mt} ({label}): not a tabulated section, skipping");
            continue;
        };

        println!("\n== MT={mt}  {label}  (NJOY grid: {} points)", pairs.len());
        println!("{:>11}  {:>14}  {:>14}  {:>9}", "E [eV]", "NJOY [b]", "ours [b]", "rel diff");
        let mut worst = (0.0_f64, 0.0_f64);
        for &e in PROBE_EV {
            let n = interp_linlin(pairs, e);
            let x = ours.xs_at_energy(e, TEMP);
            let o = match mt {
                1 => x.total,
                2 => x.elastic,
                18 => x.fission,
                102 => x.absorption - x.fission,
                _ => unreachable!(),
            };
            let rel = if n.abs() > 1e-30 { (o - n) / n } else { 0.0 };
            if rel.abs() > worst.0.abs() {
                worst = (rel, e);
            }
            println!("{e:>11.4e}  {n:>14.6e}  {o:>14.6e}  {:>+8.2}%", 100.0 * rel);
        }
        println!(
            "   worst: {:+.2}% at {:.4e} eV",
            100.0 * worst.0, worst.1
        );
    }

    // nu-bar sanity check. This CANNOT be validated against the PENDF: NJOY
    // copies MF=1/452 through unchanged and this crate reads the same section,
    // so the two agree by construction and the comparison would be circular.
    // Check it against published physics instead -- nu-bar multiplies k
    // directly, so an error here would be invisible in every sigma comparison
    // above and still move the eigenvalue.
    println!("\n== nu-bar (from MF=1/452; vs published values, NOT vs the PENDF)");
    println!("{:>11}  {:>12}", "E [eV]", "nu-bar");
    for &e in &[0.0253, 1.0e3, 1.0e6, 2.0e6, 1.4e7] {
        let x = ours.xs_at_energy(e, TEMP);
        let nu = if x.fission > 0.0 { x.nu_fission / x.fission } else { 0.0 };
        println!("{e:>11.4e}  {nu:>12.5}");
    }
    println!(
        "   reference points: U-235 thermal nu-bar is 2.43-2.44, rising to ~2.6\n\
         at 1 MeV and ~3.1 at 5 MeV; U-238 is ~2.49 at its fission threshold,\n\
         rising to ~3.0 at 6 MeV. A value outside those bands is a real defect."
    );
}

/// Linear interpolation on an ascending `(E, sigma)` table; zero outside it,
/// matching `gety1`.
fn interp_linlin(pairs: &[(f64, f64)], e: f64) -> f64 {
    if pairs.is_empty() || e < pairs[0].0 || e > pairs[pairs.len() - 1].0 {
        return 0.0;
    }
    let i = match pairs.binary_search_by(|p| p.0.partial_cmp(&e).unwrap()) {
        Ok(i) => return pairs[i].1,
        Err(i) => i,
    };
    let (e0, s0) = pairs[i - 1];
    let (e1, s1) = pairs[i];
    if e1 == e0 {
        return s1;
    }
    s0 + (s1 - s0) * (e - e0) / (e1 - e0)
}
