//! Dump this crate's reconstructed microscopic cross sections to CSV, so they can
//! be compared point-by-point against the NJOY-produced ACE that OpenMC reads.
//!
//! This is the **data half** of the Godiva discrepancy investigation
//! (`verification_and_validation/openmc_godiva_cross_code/`). That study showed
//! `outram-mc-libs` sits `+250 ± 51 pcm` above OpenMC on nominally identical
//! nuclear data; this program exists to test whether the data really is
//! identical, or whether the offset is already present before a single neutron
//! is transported.
//!
//! Emits, per nuclide, on one shared log grid: `total`, `elastic`, `fission`,
//! `absorption`, `inelastic`, `n2n`, and `nu_fission` (= ν̄·σ_f, from which ν̄
//! follows by division). The companion `compare_xs.py` in that V&V directory
//! reads the same quantities out of OpenMC's HDF5 and differences them,
//! **flux-weighted over a Watt spectrum** as well as pointwise — a 5 % error at
//! 1 eV is irrelevant to Godiva, and a 0.1 % error at 1 MeV is not.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example dump_xs_for_openmc_compare > ours.csv
//! ```

#[cfg(target_os = "android")]
fn main() {
    eprintln!("dump_xs_for_openmc_compare is desktop-only (reads reference-data/endf/).");
}

#[cfg(not(target_os = "android"))]
fn main() {
    desktop::run();
}

#[cfg(not(target_os = "android"))]
mod desktop {
    use njoy_outram_park_fork::reference_data::reference_endf;
    use outram_mc_libs::material::nuclide::Nuclide;

    const TEMP_K: f64 = 293.6;
    const NUCLIDES: &[(&str, &str)] = &[
        ("n-092_U_234-ENDF8.0.endf", "U234"),
        ("n-092_U_235-ENDF8.0.endf", "U235"),
        ("n-092_U_238.endf", "U238"),
    ];

    /// Energies to sample \[eV\]: log-spaced across the whole range, which is
    /// dense enough to see systematic offsets without pretending to resolve
    /// individual resonances (two independently thinned grids will never agree
    /// point-for-point inside a resonance, and chasing that is not the point).
    fn grid() -> Vec<f64> {
        let (lo, hi, n) = (1.0e-4_f64, 1.9e7_f64, 2000usize);
        (0..=n)
            .map(|i| lo * (hi / lo).powf(i as f64 / n as f64))
            .collect()
    }

    pub fn run() {
        println!("nuclide,energy_ev,total,elastic,fission,absorption,inelastic,n2n,nu_fission");
        for (file, name) in NUCLIDES {
            let p = reference_endf(file).unwrap_or_else(|| panic!("missing tape {file}"));
            eprintln!("reconstructing {name}…");
            let nuc = Nuclide::from_endf_file(&p, name, TEMP_K, 1.0e-3)
                .unwrap_or_else(|e| panic!("from_endf_file({}): {e}", p.display()));
            for e in grid() {
                let x = nuc.xs_at_energy(e, TEMP_K);
                println!(
                    "{name},{e:.6e},{:.6e},{:.6e},{:.6e},{:.6e},{:.6e},{:.6e},{:.6e}",
                    x.total, x.elastic, x.fission, x.absorption, x.inelastic, x.n2n, x.nu_fission
                );
            }
        }
        eprintln!("done");
    }
}
