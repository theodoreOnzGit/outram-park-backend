# Results: z_domain vs GNU Octave control package (gh:#1433, epic #1336)

Run 2026-11-13. Reference: Octave 8.4.0, `octave-control` **4.0.1** (Ubuntu apt) for the
main run; a second run shadows `@tf/__c2d__.m` and `__d2c__.m` with the pkg-control
**4.3.0** files (clone HEAD `7773b7b1`, "docs-html: update for release 4.3.0"; the two
`tf(zpk(..))` calls were edited to `zpk(..)` for 4.0.1 compatibility). The port cites
control 4.2.2+ commit `f39e2625`; `pkg install -forge control` failed (proxy) and that
commit's blob could not be fetched (codeload and the GitHub tool are not enabled for
that repository), so **4.3.0 is the closest available, not the cited commit.** Driver:
`octave_driver.m`; references: `octave_reference.csv` (4.0.1),
`octave_reference_pkg430_matched.csv` (4.3.0 override). Test:
`tests/octave_differential.rs` (6 tests, all pass). "Normwise" is max|p-o| / max|o| over a
sequence; "per entry" is max |p-o|/|o| over entries with o != 0.

## Predictions scorecard (PREDICTIONS.md, committed before the first run)

| # | prediction | outcome |
|---|---|---|
| P1 | filter(): not everywhere bit-identical, normwise <= 1e-14, d01 bit-identical | **bit-identical part wrong, bound held.** 1019 of 1560 samples bit-identical; worst normwise 3.96e-15 (d10, unstable). d01 is 92/120: its unstable response exceeds 2^53 (3.4^40), where association matters even for integer data |
| P2 | operation order is the whole cause | **confirmed.** Octave-side loop in the port's order = Rust port: 1560/1560. Octave-side loop in liboctave's order = builtin `filter`: 1560/1560 |
| P3 | a0 != 1 adds ulp error | not isolated: d12 77/120, d13 84/120 bit-identical, comparable to the other order>1 systems; worst per entry 2.2e-13 (d12, near-zero crossing), normwise 1.8e-16 |
| P4 | lsim/step/impulse normwise <= 1e-12 | **held.** 449/1560 bit-identical vs lsim, step()/impulse(): 312/1040; worst normwise 2.3e-14 (d10). Per entry up to 2.5e-12 at near-zero samples |
| P5 | Tustin <= 1e-14 | **held**, worst 8.9e-15 (c08, T = 0.1), 46/100 bit-identical |
| P6 | ZOH <= 1e-13 | **held**, worst 8.1e-14 (c03, T = 0.1), 45/84 bit-identical |
| P7 | prewarp <= 1e-14 | **MISSED**: worst 1.5e-13 per entry (5.1e-14 normwise), c08 third order, T = 0.1; 31/100 bit-identical. Ill-conditioned (1/tan, order 3); cause not proven |
| P8 | matched <= 1e-14 | **MISSED against 4.0.1** (4.96e-2 and 5.12e-2 on c09, pole at s = 0), **held against 4.3.0**: 73/84 bit-identical, worst 9.3e-16. See defect section: an upstream behaviour change, not a port defect |
| P9 | order > 2 ZOH/matched refused | **confirmed**, 4 refusals (c08, two T, two methods): gap |
| P10 | c09 ZOH (pole at 0) might expose a defect | **no defect**: ZOH worst 1.3e-14, Tustin 2.1e-15 |
| P11 | dcgain bit-identical | **confirmed** 9/9, including `Inf` for the integrator |
| P12 | d2c <= 1e-13 normwise | **held**: tustin 24 coefficients (8 bit-identical) 1.5e-15, prewarp (8) 1.7e-15, matched 6 (5) 5.7e-18. Per-entry shows 1.0 on e4 because one side has an exact 0 where the other has ~1e-17 |
| P13 | pole/zero/bode/margin not comparable | confirmed: no API |

## Tables

| case | quantity | compared | bit-identical | worst |
|---|---|---|---|---|
| 1 | port vs `filter` | 1560 samples (13 systems x rand/step/impulse x 40) | 1019 | 3.96e-15 normwise |
| 1 | port vs `lsim` (tf -> ss -> recurrence) | 1560 | 449 | 2.33e-14 normwise |
| 1 | port vs `step`/`impulse` | 1040 | 312 | 2.33e-14 normwise |
| 2 | c2d Tustin | 100 coefficients | 46 | 8.9e-15 |
| 2 | c2d ZOH | 84 | 45 | 8.1e-14 |
| 2 | c2d prewarp (w0 = 1 rad/s) | 100 | 31 | 1.5e-13 per entry |
| 2 | c2d matched vs 4.0.1 | 84 | 69 | 5.1e-2 (c09 only) |
| 2 | c2d matched vs 4.3.0 | 84 | 73 | 9.3e-16 |
| 2 | d2c Tustin / prewarp / matched | 24 / 24 / 6 | 8 / 8 / 5 | 1.5e-15 / 1.7e-15 / 5.7e-18 normwise |
| 3 | `steady_state_gain` vs `dcgain` | 9 | 9 | 0 |

## Localisation of mismatches

* **Discrete simulation:** first divergent operation is the update
  `s[i] = (b*u + carry) - a*y` (port) vs `(carry - a*y) + b*u` (liboctave `filter.cc`);
  proven by the two replicas above, which reproduce each side bit for bit.
* **`lsim`/`step`/`impulse`:** Octave realises the tf as a state space model, so the
  recurrence is a different algorithm; differences are round-off. Not further localised.
* **c2d/d2c other than matched:** Octave reaches these through `ss` and SLICOT
  (`AB04MD`, `MB05ND`), the port through polynomial substitution / closed forms.
  Plausible cause of the ulp-level differences; **not proven** (no SLICOT-side replica).
* **c09 matched:** first divergent operation is the gain-matching frequency.
  4.0.1 `@tf/__c2d__.m` lines 58-62 evaluate the gain at the real point `s = w_c`,
  `z = exp(w_c T)`; 4.3.0 evaluates at `s = j w_c`, `z = exp(j w_c T)`. The port
  (conversion.rs `c2d_matched`) uses `j w_c`, as 4.3.0 does. With the 4.3.0 file shadowed
  the case is bit-identical (6/6 at both T). Only c09 differs between the two reference
  runs (2 lines of the 436 differ); every other case does not reach the loop.

## Defects found

None in the port's numerics. Two stale doc claims, both corrected in place (strike-through):

1. `verification_tests.rs` module doc "no Octave installation was available ... no number
   below is an Octave output": still true of that file, now points at this comparison.
2. `conversion.rs` `d2c_matched` doc "upstream never updates `w_d` inside the loop": true
   of control 4.0.1 `__d2c__.m`, false for 4.3.0 (diffed). The port's behaviour is
   unchanged and matches 4.3.0.

Also noted: `upstream_source/README.md` had already been corrected on 2026-11-10; this
change adds the differential-run pointer. No moved numbers (no source behaviour changed).

## Gaps found (see the issue's survey)

Port refuses ZOH and matched c2d above order 2 (Octave succeeds); matched d2c of a
system with a zero at z = 0 is refused by both, but the port's error for order 3 (e4)
is `UnsupportedOrder` where Octave's is the log(0) error; no d2c ZOH; no pole/zero,
bode, margin, nyquist, step/impulse helpers for transfer functions; no `dcgain` for
discrete systems; no series/parallel/feedback algebra; SISO only; no `foh`/`impulse`
methods.
