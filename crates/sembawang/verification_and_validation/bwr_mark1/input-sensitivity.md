# `bwr_mark1`: input sensitivity, to rank F13's checks (gh:#1044, #1033)

> Research, education and V&V only. AI draft, not reviewed. This is a
> **ranking instrument**, not an uncertainty analysis and not a calibration.
> No value is chosen from it (plan rule 1: calibration only as the last step).

## Methodology

**What is computed.** For each of the 73 scalar inputs in
`bwr_mark1::params::Params::INPUTS`, `compare::elasticity` does the following:

- scales that one input by 0.99 and by 1.01 about its default, with every
  other input at its default and every mechanism on;
- runs the recorded Unit 1 sequence to 26 h each time;
- reports the elasticity `(dm/m) / (dp/p)`, by central difference, of three
  metrics: lower-head failure time, in-vessel H2 at 25 h, and peak drywell
  pressure over 0–25 h.

It also flags whether the wetwell vent or the building explosion appears in
either perturbed run (neither happens at the defaults).

**Why this instrument.** The question is which unverified inputs the answer
leans on hardest, so that F13 (#1044) checks those against the sources first.
A local, normalised derivative answers that without anyone choosing a range.

**What it cannot say.** It is local: a step of a few per cent could cross a
threshold that ±1 % does not. A relative step is also not an input's real
uncertainty: 1 % of an Arrhenius activation energy is a large change, while
1 % of a valve area is a small one. An event time is resolved to one 1 s step
(about 3e-4 h), which limits elasticities to about ±0.005 on the lower-head
time.

The tags come from each constant's doc comment, and
`params::tests::every_input_carries_its_documented_tag` checks them: \[P\]
published, \[U\] unverified, \[A\] the model's assumption.

**Run.** `cargo run --release -p sembawang --example bwr_mark1_sensitivity`

## Predictions (written 2026-10-29, before the study was run)

1. **Lower-head time** leans hardest on `lower_head_failure_k` (\[A\], 1500 K;
   it is the failure criterion itself) and `zr_melt_k` (\[U\]; it starts
   relocation). Next come `debris_quench_s` and the decay-heat scale
   (`thermal_power_w`). Elasticity of `lower_head_failure_k`: positive, order
   0.1–1.
2. **H2 at 25 h** leans on `zr_melt_k`, which ends oxidation when a node
   relocates, so the elasticity is positive and large. The Urbanic-Heidrick
   activation energies `uh_low_b_k` and `uh_high_b_k` come next, with large
   **negative** elasticities (`exp(-B/T)`: +1 % of B at 1500 K cuts the rate
   by about 10 %). `oxidation_min_k` is small.
3. **Drywell peak** leans on `drywell_volume_m3` (negative),
   `initial_containment_pa` (positive), and the leak and breach areas
   (`hot_leak_m2`, `breach_m2`).
4. **Zero effect:** `containment_design_pa`, which only the demo draws, and
   probably `h2_explosion` and `h2_flammable`, because no hydrogen reaches the
   building at the defaults.
5. **No vent or explosion appears** at ±1 % of any input.

## Results

Not yet run.
