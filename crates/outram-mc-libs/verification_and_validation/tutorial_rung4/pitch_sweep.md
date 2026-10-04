# LCT-008 pin cell: k∞ against lattice pitch (under- and over-moderation)

**Class:** verification (code-to-code against OpenMC on a few points, and a
qualitative physics expectation). **Not validation**: no measurement exists
for an infinite lattice of this pin at other pitches. AI-assisted draft under
`RESPONSIBLE_USE.md`; no human V&V sign-off. Education and research only.

Tutorial rung 4 (GitHub #526, epic #520). Example:
`crates/outram-mc-libs/examples/lct008_pitch_sweep.rs`.

## Prediction (written 2026-10-04 22:10 +08:00, before any run)

Geometry ratios of the sweep, computed from the pin radii in
`examples/common/lct008_model.rs` (`R_FUEL = 0.514858`, `R_CLAD = 0.602996` cm)
before anything was transported:

| pitch [cm] | p/d | V_mod/V_fuel |
|---|---|---|
| 1.25 | 1.036 | 0.505 |
| 1.35 | 1.119 | 0.817 |
| 1.45 | 1.202 | 1.153 |
| 1.55 | 1.285 | 1.513 |
| **1.63576 (LCT-008)** | **1.356** | **1.841** |
| 1.75 | 1.451 | 2.306 |
| 1.90 | 1.575 | 2.963 |
| 2.10 | 1.741 | 3.924 |
| 2.40 | 1.990 | 5.545 |
| 2.80 | 2.322 | 8.043 |
| 3.30 | 2.736 | 11.705 |

Predictions, with the reasoning that makes each one capable of failing:

1. **Shape.** k∞ rises with pitch from the near-touching lattice, peaks, then
   falls. Rising side: more water per pin means faster slowing down past the
   U-238 resonances (p rises) and fewer neutrons absorbed in the resonances.
   Falling side: the water's own thermal absorption (H-1 and, here, the case-1
   1511 ppm soluble B-10) takes a growing share of thermal neutrons (f falls).
   A curve that is monotone over 0.5 ≤ V_m/V_f ≤ 12 refutes this.
2. **Factors.** Across the sweep `p` increases monotonically with pitch and `f`
   decreases monotonically. `η` and `ε` change much less than either (ε falls
   slowly as the fuel rods decouple, η moves only through the thermal spectrum).
3. **Where the peak is.** Because the case-1 water is borated, the peak sits at
   a smaller moderator ratio than an unborated lattice would have: I expect it
   between V_m/V_f ≈ 2 and 3 (pitch ≈ 1.7–1.9 cm).
4. **Where LCT-008 sits.** LCT-008's pitch (V_m/V_f = 1.84) is on the
   **under-moderated** side of that peak, but close to it: within ~1000 pcm of
   the maximum k∞.
5. **Magnitude.** k∞ at the LCT-008 pitch is above 1 (the real core is critical
   with leakage), roughly 1.05–1.20.
6. **Code-to-code.** On the points run with OpenMC on the same 11-nuclide tier,
   outram-mc and OpenMC agree within 2σ of the combined uncertainty, or within
   the ~10–40 pcm route-4 vs route-1 offsets the five-route record shows on the
   full lattice (`icsbep/five_route_keff_2026_09_29.md`).

The measured result, including where these predictions fail, is recorded
below. Nothing in the example is adjusted after seeing it.
