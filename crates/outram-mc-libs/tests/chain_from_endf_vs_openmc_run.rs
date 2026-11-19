#![cfg(feature = "std")]
//! **`Chain.from_endf` against OpenMC run on the same files** (gh:#1621,
//! gh:#1626's chain part).
//!
//! Reference: OpenMC `e4febe9`'s own `Chain.from_endf`, run 2026-11-17 by
//! `verification_and_validation/spectrum_depletion/openmc_inputs/ref_chain.py`
//! on the ENDF/B-VIII.0 decay and NFY sublibraries in `reference-data/ace`
//! and, for `full`, the Th-232, Pa-233, U-233, U-235 and U-238 tapes, then
//! `strip_sources.py` (decay spectra removed: the port carries none). Both
//! outputs are committed in `openmc_outputs/`.
//!
//! Prediction, written before the comparison: the port's chain equals
//! OpenMC's in every nuclide record (sources excluded), and our export is
//! OpenMC's stripped file byte for byte.
//!
//! Results, measured 2026-11-17: **held.** Decay-only chain: 3,820 nuclides,
//! 0 differ, export byte-identical to OpenMC's (sources stripped). Full
//! chain (with the five neutron tapes): 3,820 nuclides, 0 differ, export
//! byte-identical. (A first pass showed only an indentation difference,
//! traced to `strip_sources.py` dropping removed elements' whitespace; it
//! now re-indents from scratch as `export_to_xml` does.)
//!
//! OpenMC was run as an installed pure-Python package (no `libopenmc`):
//! `from_endf` never calls the library, and `openmc.lib` was loaded through
//! upstream's own documentation-build mock (`READTHEDOCS=True`). Python
//! 3.11, OpenMC `e4febe9` (`pip install --ignore-requires-python`, which
//! asks for 3.12); numpy, h5py, scipy, pandas, lxml, uncertainties, endf.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::hdf5::depletion_chain_from_endf::from_endf;
use njoy_outram_park_fork::hdf5::depletion_chain_ops::export_to_xml;
use njoy_outram_park_fork::hdf5::depletion_chain_xml::DepletionChainXml;
use std::path::{Path, PathBuf};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/ace/endf/endf-b-viii.0")
}

fn tapes(d: &Path) -> Vec<Tape> {
    let mut ps: Vec<PathBuf> = std::fs::read_dir(d).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|x| x == "endf")).collect();
    ps.sort();
    ps.iter().map(|p| Tape::from_bytes(&std::fs::read(p).unwrap()).unwrap()).collect()
}

fn compare(label: &str, ours: &DepletionChainXml, reference_file: &str) -> (usize, usize) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/spectrum_depletion/openmc_outputs").join(reference_file);
    let text = std::fs::read_to_string(&path).unwrap();
    let reference = DepletionChainXml::parse(&text).unwrap();
    assert_eq!(ours.nuclides.len(), reference.nuclides.len(), "{label}: nuclide count");
    let mut differ = Vec::new();
    for (a, b) in ours.nuclides.iter().zip(&reference.nuclides) {
        if a != b {
            differ.push((a.clone(), b.clone()));
        }
    }
    let bytes_equal = export_to_xml(ours) == text;
    println!("{label}: {} nuclides, {} differ; export byte-identical: {bytes_equal}", ours.nuclides.len(), differ.len());
    for (a, b) in differ.iter().take(4) {
        println!("  ours {a:?}\n  openmc {b:?}");
    }
    if !bytes_equal {
        let ours_text = export_to_xml(ours);
        if let Some((i, (x, y))) = ours_text.lines().zip(text.lines()).enumerate().find(|(_, (x, y))| x != y) {
            println!("  first differing line {}:\n   ours   {x}\n   openmc {y}", i + 1);
        }
    }
    (differ.len(), usize::from(!bytes_equal))
}

#[test]
fn from_endf_matches_openmc_run_on_the_same_files() {
    let d = dir();
    if !d.join("decay").is_dir() {
        println!("SKIP: reference-data/ace not checked out");
        return;
    }
    let decay = tapes(&d.join("decay"));
    let nfy = tapes(&d.join("nfy"));
    let (decay_only, _) = from_endf(&decay, &nfy, &[], None).unwrap();
    let a = compare("decay only", &decay_only, "decay_only_nosrc.xml");

    let endf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/endf");
    let read = |p: PathBuf| Tape::from_bytes(&std::fs::read(p).unwrap()).unwrap();
    let neutron = vec![
        read(d.join("n-090_Th_232-ENDF8.0.endf")),
        read(d.join("n-091_Pa_233-ENDF8.0.endf")),
        read(d.join("n-092_U_233-ENDF8.0.endf")),
        read(endf.join("n-092_U_235-ENDF8.0.endf")),
        read(endf.join("n-092_U_238.endf")),
    ];
    let (full, _) = from_endf(&decay, &nfy, &neutron, None).unwrap();
    let b = compare("full", &full, "full_nosrc.xml");
    assert_eq!((a, b), ((0, 0), (0, 0)));
}
