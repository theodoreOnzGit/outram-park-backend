# Results (gh:#1429, run 2026-11-13)

Upstream pyDOSEIA `dca4cdc3` (MIT) cloned over git into scratch and run with `python -I`.
Predictions: `PREDICTIONS.md`, committed before the Rust runs and not edited except for the
environment line added with the second venv. The full methodology is in
`crates/buangkok/docs/pydoseia-code-to-code.md`; gated by
`crates/buangkok/tests/pydoseia_code_to_code.rs`.

| # | prediction | outcome |
|---|---|---|
| P1 | `speed_means` bit-identical after switching to numpy's pairwise sum | **held**: 12 of 12 bit-identical, tolerance 2e-15 -> 0 (was 4.83e-16). Pinned by count |
| P2 | new in-memory `boundary_totals`: all 90 values bit-identical | **missed by one value**: 89 of 90 bit-identical; the 90th (`long_term_met`, SYN-6, age 18, total) is 1.1150730118948052e-16 off. Localised to numpy `exp` in the ingestion path of the generating machine (see P3). Pinned in `KNOWN_DIFFERENCES` |
| P3 | the committed fixture is environment-dependent | **held**: regenerated under numpy 2.5.3 / pandas 3.0.6 / scipy 1.18.1 (Python 3.13.18), 78 of 18 127 values differ (all in numpy-`exp` paths); `np.exp` vs glibc `exp` differ on 9 190 of 200 000 inputs. Under numpy 2.4.6 / pandas 3.1.1 also 21 `dcf_screening` and 4 `driver_summary` values. The port against the numpy-2.4.6 fixture: worst 4.65e-16 (`ingestion`), 4.19e-16 (`dilution_no_met`), all in `exp` groups |

Moved numbers, old -> new: `speed_means` max deviation 4.83e-16 -> 0, tolerance 2e-15 -> 0;
cases 1 899 -> 1 917, values 18 127 -> 18 217, groups 43 -> 44. No other tolerance moved.

Not done: case 2 of the issue (a real pyDOSEIA example met file). `met_data/Met_data_5Yr.xlsx`
has no stated provenance and may be facility data, which `DATA_POLICY.md` forbids committing
(`docs/pydoseia-port-scoping.md`, licence table), so no fixture from it can be committed.
Deferred to the maintainer: state its provenance, or accept a local-only, uncommitted check.
