//! Diagnostic probe for `op-mzvp.2.12` — dump the reconstructed U-238 capture
//! cross section across the resolved/unresolved boundary and compare it against
//! the evaluation's own MF=3/MT=102 background.
//!
//! U-238 in ENDF/B-VIII.0 carries **LSSF=1** in its LRU=2 range, so MF=3 already
//! holds the infinitely-dilute unresolved cross sections. If the reconstruction
//! reproduces them, our URR sigma is at infinite dilution (correct magnitude,
//! no self-shielding structure) rather than "low", and the ring-RPT hypothesis
//! needs revisiting.

fn main() {
    use outram_mc_libs::material::nuclide::Nuclide;
    use std::path::Path;

    let path = Path::new("reference-data/endf/n-092_U_238.endf");
    eprintln!("reconstructing U-238 from {} ...", path.display());
    let u238 = Nuclide::from_endf_file(path, "U238", 600.0, 1e-3)
        .expect("U-238 reconstruction failed");
    eprintln!("done.\n");

    // MF=3/MT=102 infinite-dilute capture, read straight off the tape.
    let mf3: &[(f64, f64)] = &[
        (2.0e4, 0.52987),
        (2.4e4, 0.47184),
        (3.0e4, 0.43448),
        (4.5e4, 0.35736),
        (5.5e4, 0.28953),
        (6.5e4, 0.24489),
        (7.5e4, 0.21136),
        (8.5e4, 0.18787),
        (9.5e4, 0.18152),
        (1.0e5, 0.17879),
        (1.2e5, 0.16372),
        (1.5e5, 0.14114),
    ];

    println!("== resolved-range spot checks (expect real resonance structure) ==");
    for e in [6.67, 20.9, 36.7, 66.0] {
        let x = u238.xs_at_energy(e, 600.0);
        println!("  E = {e:>9.3} eV   sigma_a = {:>10.3} b", x.absorption);
    }

    println!("\n== unresolved band 20-150 keV: ours vs MF=3 infinite dilution ==");
    println!("  {:>10}  {:>12}  {:>12}  {:>9}", "E [eV]", "ours [b]", "MF3 [b]", "ratio");
    let mut worst: f64 = 0.0;
    for &(e, ref_xs) in mf3 {
        let ours = u238.xs_at_energy(e, 600.0).absorption;
        let ratio = ours / ref_xs;
        worst = worst.max((ratio - 1.0).abs());
        println!("  {e:>10.4e}  {ours:>12.5}  {ref_xs:>12.5}  {ratio:>9.4}");
    }
    println!("\nworst deviation from infinite dilution: {:.2} %", worst * 100.0);
    if worst < 0.05 {
        println!(
            "VERDICT: reconstruction reproduces MF=3 infinite dilution.\n\
             Our URR sigma_gamma is NOT low - it is unshielded. Adding PURR\n\
             self-shielding would LOWER capture and RAISE k, moving ring-RPT\n\
             further from OpenMC, not closer."
        );
    } else {
        println!("VERDICT: reconstruction departs from MF=3 - investigate the URR seam.");
    }
}
