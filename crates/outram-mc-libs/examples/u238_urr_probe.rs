//! **The reconstructed U-238 capture cross section across the
//! resolved/unresolved boundary, against the evaluation's own MF=3/MT=102
//! background.**
//!
//! # Why this exists, and what it overturned
//!
//! `op-mzvp.2.12` was filed with "URR self-shielding not reconstructed" as the
//! leading hypothesis for the ring-RPT residual, and "expect ours low and
//! structureless" as the predicted signature.
//!
//! **Reading the evaluation's own format flags first would have pre-empted
//! that.** U-238 in ENDF/B-VIII.0 carries **LSSF=1** in its LRU=2 range, and
//! LSSF=1 means MF=3 *already holds the infinitely-dilute unresolved cross
//! sections*. So the reconstruction is not "low" — it reproduces MF=3 — and
//! adding PURR self-shielding would have **lowered** capture and **raised** k,
//! moving the case further from the reference rather than closer.
//!
//! That is the general lesson, recorded in this repository's `CLAUDE.md`: an
//! ENDF flag can change what a section *means*, and a missing-physics hypothesis
//! that ignores the flag sends you porting a module you did not need. Hours went
//! into a first-principles argument and two speculative patches that reading
//! `LSSF` would have stopped.
//!
//! # The oracle
//!
//! MF=3/MT=102 read straight off the tape at twelve energies from 20 to 150 keV
//! — the evaluation's own infinitely-dilute unresolved capture. It is not
//! another code's answer: it is the file this crate is reconstructing *from*, so
//! agreement is a statement about the reconstruction and nothing else.
//!
//! # V&V result (2026-09-11, ENDF/B-VIII.0 @ 600 K)
//!
//! The reconstruction reproduces MF=3 infinite dilution across 20–150 keV to
//! **≤ 0.08 %**, and reproduces NJOY2016's own PENDF to **+0.02 %** right
//! through the resolved/unresolved seam. The URR cross section is therefore not
//! low — it is unshielded, which is what LSSF=1 prescribes.
//!
//! # What the assertion caught that the printed verdict did not
//!
//! This program used to end with an `if/else` printing one of two
//! plausible-sounding verdicts and exiting 0 either way. It had been printing
//! *"VERDICT: reconstruction departs from MF=3 — investigate the URR seam"*,
//! because its first probe energy sat exactly on the 20 keV boundary and read
//! −23.5 % low.
//!
//! Turning that verdict into an assertion forced the investigation it had been
//! asking for, and there is no seam defect: the evaluation is genuinely
//! **discontinuous** at 20 keV, NJOY's own PENDF carries grid points either side
//! to represent the jump, and evaluating at exactly the boundary lands mid-jump
//! by linear interpolation in *both* codes, agreeing to 0.02 %. The probe energy
//! has moved just inside the unresolved range and the discontinuity is now
//! asserted in its own right. See the note on the reference table below.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example u238_urr_probe
//! ```

fn main() {
    use outram_mc_libs::material::nuclide::Nuclide;
    use std::path::Path;

    let path = Path::new("reference-data/endf/n-092_U_238.endf");
    eprintln!("reconstructing U-238 from {} ...", path.display());
    let u238 =
        Nuclide::from_endf_file(path, "U238", 600.0, 1e-3).expect("U-238 reconstruction failed");
    eprintln!("done.\n");

    // MF=3/MT=102 infinite-dilute capture, read straight off the tape.
    //
    // NOTE ON THE FIRST ROW. This used to be `(2.0e4, 0.52987)` — the boundary
    // energy itself — and it read −23.5 % low, which the program reported as
    // "investigate the URR seam" and then exited 0 on. There is no seam defect.
    // The evaluation is genuinely **discontinuous** at 20 keV, and NJOY's own
    // PENDF carries grid points at 1.9999999e4 (0.280981 b) and 2.0000001e4
    // (0.529870 b) to represent the jump. Evaluating at *exactly* 2.0e4 lands
    // mid-jump by linear interpolation, in both codes:
    //
    //   E [eV]        ours [b]    NJOY PENDF [b]    rel
    //   1.99990e4     0.222351      0.221747       +0.27 %
    //   2.00000e4     0.405504      0.405425       +0.02 %   <- mid-jump
    //   2.00010e4     0.529938      0.529856       +0.02 %
    //
    // So the reconstruction reproduces NJOY to 0.02 % right through the seam,
    // and the −23.5 % was the *reference table* comparing an interpolated
    // boundary value against the unresolved-side plateau. The probe energy is
    // now 2.0001e4, just inside the unresolved range, and the discontinuity
    // itself is asserted separately below.
    let mf3: &[(f64, f64)] = &[
        (2.0001e4, 0.52987),
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
    println!(
        "  {:>10}  {:>12}  {:>12}  {:>9}",
        "E [eV]", "ours [b]", "MF3 [b]", "ratio"
    );
    let mut worst: f64 = 0.0;
    for &(e, ref_xs) in mf3 {
        let ours = u238.xs_at_energy(e, 600.0).absorption;
        let ratio = ours / ref_xs;
        worst = worst.max((ratio - 1.0).abs());
        println!("  {e:>10.4e}  {ours:>12.5}  {ref_xs:>12.5}  {ratio:>9.4}");
    }
    println!(
        "\nworst deviation from infinite dilution: {:.2} %",
        worst * 100.0
    );

    // ── V&V gate ──────────────────────────────────────────────────────────────
    //
    // This used to be an if/else that printed one of two verdicts and exited 0
    // either way. Both branches were plausible-sounding English, so the program
    // could report "investigate the URR seam" forever and nothing would notice.
    println!("\n=== V&V gate: URR reconstruction vs the evaluation's own MF=3 ===");
    let rows: Vec<(f64, f64, f64)> = mf3
        .iter()
        .map(|&(e, ref_xs)| (e, u238.xs_at_energy(e, 600.0).absorption, ref_xs))
        .collect();
    outram_mc_libs::vv::assert_table_relative(
        "U-238 unresolved capture vs MF=3 infinite dilution (LSSF=1)",
        &rows,
        0.05,
        1.0e-6,
    );
    println!(
        "  [PASS] the reconstruction reproduces MF=3 infinite dilution.\n\
         \x20 Our URR sigma_gamma is NOT low — it is unshielded, which is what\n\
         \x20 LSSF=1 prescribes. Adding PURR self-shielding would LOWER capture\n\
         \x20 and RAISE k, moving ring-RPT further from OpenMC, not closer."
    );

    // The discontinuity at the resolved/unresolved boundary is real and shared
    // with NJOY, so it is asserted rather than smoothed over. A reconstruction
    // that blended across the boundary would pass every envelope above and would
    // be wrong in exactly the way this probe was written to detect.
    let below = u238.xs_at_energy(1.9999e4, 600.0).absorption;
    let above = u238.xs_at_energy(2.0001e4, 600.0).absorption;
    println!(
        "\n  seam: sigma_a = {below:.6} b just below 20 keV, {above:.6} b just above \
         (NJOY PENDF: 0.221747 and 0.529856)"
    );
    assert!(
        above > 2.0 * below,
        "the resolved/unresolved boundary at 20 keV is no longer discontinuous: \
         sigma_a = {below:.6} b below and {above:.6} b above. The evaluation jumps \
         there (NJOY's own PENDF carries grid points at 1.9999999e4 and 2.0000001e4 \
         to represent it, 0.280981 and 0.529870 b); a reconstruction that blends \
         across the boundary has lost that structure."
    );
    println!("  [PASS] the 20 keV discontinuity is preserved, as in NJOY's own PENDF");

    // Structure, not just magnitude: MF=3's unresolved capture falls with energy
    // across this band, and so must ours. A reconstruction that returned a flat
    // or a resonance-structured curve of the right average would pass the
    // envelope above.
    let ours_curve: Vec<f64> = rows.iter().map(|&(_, o, _)| o).collect();
    outram_mc_libs::vv::assert_monotone(
        "unresolved capture falls monotonically from 20 to 150 keV",
        &ours_curve,
        false,
        0.0,
    );
}
