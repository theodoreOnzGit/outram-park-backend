# Prediction: k_eff of the simplified fresh EBR-II core (gh:#1762)

> Research, education and V&V only; not for facility operation, licensing or
> safety decisions. AI-assisted (Claude Code), not reviewed by the maintainer.
> **Written and committed before any run of this model.**

## What will be run

- **Model:** `nee_soon::ebr2_core::Study::default()`, the default study:
  - Run 138B's 637-position layout (gh:#1313);
  - every subassembly homogenised, with fresh U-5Fs at 67 wt% U-235;
  - 293.15 K;
  - the control rods at SHRT-45R's starting drop of 21.06 cm.
- **Assumptions:** the module docs list every one (half-worth fuel, B4C
  volume, reflector make-up, axial centring, blanket density).
- **Run:** `examples/ebr2_core_images.rs --keff 4000 20 80 1`, with
  ENDF/B-VIII.0 at the library's defaults (URR and RVS on), on every
  thread of this 4-vCPU container.

## Prediction, with reasons

**k = 1.08, in the range 1.02 to 1.15.**

- **Start from the real core.** EBR-II's Run 138B core was critical with
  partly burnt drivers: about 4.5 at.% average burnup, plus fission products
  and some plutonium (ARC-228).
- **Fresh fuel raises k.** It carries more U-235 and no fission-product
  poison, which should put this model several percent above critical.
- **The assumptions add a few percent of uncertainty each way:**
  - the steel-rich axial "reflectors" reflect at least as well as the real
    upper and lower regions;
  - the half-worth drivers are guessed as half fuel;
  - the B4C bundles partly inserted at 21.06 cm take some reactivity away.
- **Homogenising the pins changes little in a fast spectrum.**

**What would surprise me:** k below 1.0 or above 1.2. Either would mean a
composition or geometry error. I would look for it in the drawings and in the
fractions before believing the number.

## What this is not

- **Not a prediction of EBR-II's critical state.** It is a fresh core,
  labelled as such everywhere its k appears (maintainer, option A,
  2026-11-25).
- **Not validation.**
