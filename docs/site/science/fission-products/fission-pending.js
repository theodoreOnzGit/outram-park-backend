// The values Fission products lesson 1 still needs a source for, in ONE
// place (maintainer design, 2026-10-20). A research pass is finding open
// sources; when one is found, put the number here with its citation in a
// comment, and the page shows it instead of "source pending". Hand-edited:
// this file is not generated. null means pending.
window.FISSION_PENDING = {
  // Time from the saddle point to scission (the neck breaking) [s].
  saddleToScissionS: null,
  // Time for the fragments to reach their final speed [s].
  fragmentAccelerationS: null,
  // When the prompt gamma cascade starts after scission [s].
  promptGammaStartS: null,
  // Ternary fissions (a light charged particle at scission) per fission. The
  // ENDF/B-VIII.0 fission yield library carries no He-4 or H-3, so it cannot
  // supply this; the demo makes a ternary event only when asked (?ternary=1
  // or the panel's button).
  ternaryPerFission: null,
  // The light particle's kinetic energy [MeV]. While null the demo uses a
  // stand-in RULE (EFR x 4 / A_CN, the alpha's mass share of the kinetic
  // energy), which is labelled on screen as a placeholder, not physics.
  ternaryAlphaEnergyMeV: null
};
