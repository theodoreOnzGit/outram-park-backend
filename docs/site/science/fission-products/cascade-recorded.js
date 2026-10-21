// Recorded and cited values for the chapter "A fragment's story" (lessons
// 4-9), in ONE place, and the values still waiting for a source. Hand-edited:
// this file is not generated. Each value names the record or the page it was
// read from, and the date; null means "source pending", and the page shows
// that instead of a number. Checked by check-fission.js.
window.CASCADE_RECORDED = {
  // boon-lay's RustBCA cost probe (examples/rustbca_cascades.rs), results
  // table of crates/boon-lay/verification_and_validation/rustbca_cascades/
  // README.md, run 2026-10-16: fragments born in the middle of an HTR-10 UO2
  // kernel (10.4 g/cm3), 16 cascades each, mean +- standard error.
  // ILLUSTRATIVE (boon-lay CLAUDE.md): RustBCA's high-energy electronic
  // stopping uses the bare projectile charge, and nothing has been compared
  // with measured ranges.
  rustbca: {
    date: "2026-10-16",
    record: "crates/boon-lay/verification_and_validation/rustbca_cascades/",
    light: { name: "Sr-95", a: 95, z: 38, rangeUm: 10.69, semUm: 0.17, recoils: 317562, secondsPerCascade: 1.47 },
    heavy: { name: "Xe-139", a: 139, z: 54, rangeUm: 7.41, semUm: 0.12, recoils: 454318, secondsPerCascade: 2.00 },
    // Upstream RustBCA's per-element cutoff Ec [eV], below which a recoil is
    // no longer followed (scripts/materials.py at a356280d, as the example's
    // doc comment lists them; the record says "1 to 3 eV" and that they "sit
    // far below the displacement energies", which is why the tracked-recoil
    // counts are so large).
    cutoffEv: { U: 3, O: 2, C: 1 }
  },
  // DOE-HDBK-1017/2-93, Material Science vol. 2, Module 5 (open corpus,
  // kovan-standard-open-corpus), read 2026-10-20.
  doe: {
    // p. 33: "If a target or struck nucleus gains about 25 eV of kinetic
    // energy (25 eV to 30 eV for most metals) in a collision with a radiation
    // particle ... the nucleus will be displaced".
    displacementEv: [25, 30], displacementPage: 33,
    // pp. 33-34: "Most of these displacements are temporary."
    temporaryPages: "33-34",
    // p. 34: elastic collisions mostly below "A keV, where A is the mass
    // number of the knock-on"; above it, mostly ionisation.
    akevPage: 34
  },
  pending: {
    // How fast a displaced atom and its vacancy recombine after the spike
    // ("within picoseconds" in the design brief; no source in the corpus).
    recombinationTimeS: null,
    // The fraction of displaced atoms that survive the thermal spike.
    survivingFraction: null,
    // The attempt (vibration) frequency of an atom in its site, ~1e13 /s in
    // the design brief; no source in the corpus. The demos count time in
    // attempts, so they do not need it.
    attemptFrequencyHz: null,
    // That direct exchange and ring rotation are rare in crystals: listed
    // as mechanisms in MATE 664 L04 and L10, which do not rank them.
    exchangeRareSource: null,
    // A source that names alpha-recoil damage as a self-damage mechanism of
    // actinide fuels.
    alphaSelfDamageSource: null
  }
};
