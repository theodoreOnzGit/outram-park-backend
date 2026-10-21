# Delayed-neutron spectra on both data routes (GitHub #365 audit)

*2026-09-29. Research, education and V&V only; not for any operational,
licensing or safety use (see `RESPONSIBLE_USE.md`).*

## What was wrong

Every fission neutron, delayed ones included, was born with the prompt χ on
**both** routes:
- **ACE route:** DNEDL/DNED (JXS 26/27) were not read.
- **ENDF route:** MF=5/455 was parsed, but the spectrum was kept only as
  `(E', g)` pairs. For ENDF/B-VIII.0 U-234/235/238 those pairs are a
  **histogram**, so they were not samplable as stored.

OpenMC's `sample_fission_neutron` (`src/physics.cpp`) does three things:
1. Draws a delayed neutron with probability `nu_d/nu_t`.
2. Picks its precursor group by the group yield.
3. Samples that group's law.

The delayed spectra average about 0.51 MeV, against about 2.0 MeV for prompt.

## The change

- **ACE route:** `acer::delayed::decode_delayed` reads one law per group
  through DNEDL, using the same `decode_law_chain` DLW uses
  (`reaction.py:355-357`). NJOY writes LAW=4.
- **ENDF route:** `nuclear_data::delayed::DelayedChiGroup::law` holds each
  group's spectrum as an exact `FissionSpectrum`.
  - LF=5 is used only when `theta(E) == 1` is **checked**, not assumed. Then
    `E' = x` and `g` is the spectrum, kept with its own INT (histogram here).
  - LF=1 keeps all incident tables, not only the lowest.
  - Anything else gives `None`. The nuclide then keeps the prompt χ for **all**
    groups, never a mixture of exact and substituted groups.
- **Transport:** `DelayedData::spectra` feeds `Nuclide::sample_fission_energy`,
  which follows OpenMC's scheme. It is **on by default** on both routes, and
  pinned by `tests/correct_physics_is_default.rs`. `without_delayed_spectra()`
  is the explicit ablation.

## Verification

`tests/delayed_spectra_vs_openmc.rs`. The reference is the exact mean birth
energy under OpenMC's scheme, `(1-beta)<E>_p + beta sum_k w_k <E>_k`.
- **Sources:**
  - ACE rows: OpenMC's ACE reader of the NJOY2016 tables.
  - ENDF rows: OpenMC's **ENDF** reader of the evaluations, which is
    independent of NJOY and of this crate's parser.
  - Script: [`openmc_inputs/delayed_spectra_reference.py`](openmc_inputs/delayed_spectra_reference.py),
    written to [`data/delayed_spectra_openmc.csv`](data/delayed_spectra_openmc.csv).
- **Checks.** Each case draws `N = 2e6` samples.
  - The mean lies within 5σ of the mixture reference.
  - The ablated arm lies within 5σ of the prompt-only reference.
  - The two references are at least 5σ apart, so the check can fail.

| route | nuclide | E_in | z vs mixture | ablated z vs prompt | separation |
|---|---|---|---|---|---|
| ACE | U-235 | 1 MeV | −1.62 | +0.12 | 8.8σ |
| ACE | U-235 | 5 MeV | −0.23 | −2.32 | 6.2σ |
| ENDF | U-235 | 1 MeV | −0.09 | +0.85 | 8.8σ |
| ENDF | U-235 | 5 MeV | −1.00 | −0.53 | 6.2σ |
| ACE | U-238 | 1 MeV | −1.61 | −1.18 | 23.0σ |
| ACE | U-238 | 5 MeV | +0.38 | +0.70 | 17.6σ |
| ENDF | U-238 | 1 MeV | +0.53 | −0.83 | 23.0σ |
| ENDF | U-238 | 5 MeV | +1.72 | +0.10 | 17.6σ |

**The test can fail.** A mutation that drew delayed neutrons from the prompt χ
failed the test at the first row (z = +7.27). It was run and reverted.

**Correction to this test's first version.** It required the ablated arm to
miss the mixture by more than 5σ of one run. At U-235 5 MeV, where the expected
miss is about 6.8σ, a draw gave 3.9σ. That was a power criterion subject to its
own noise. Each arm is now compared with its own exact reference, and the power
requirement sits on the noise-free references.

**Analog regression pin.** `tests/variance_reduction_is_bit_identical_when_analog.rs`
was re-recorded. With `without_delayed_spectra()` it reproduces the previous
record bit for bit, which proves the cause.

## k_eff (32 seeds, campaign settings, 4 threads, `target/ace_parity_366/`)

**Prediction, posted on #365 before the run:** negative, 10–50 pcm, on
Godiva and Jemima, routes 3 and 4. The reasoning was a low-energy delayed
neutron with lower importance, at about −0.05 × beta.

**The prediction was wrong.** Paired against the post-URR runs:

| case | route 3 (ACE) | route 4 (ENDF) |
|---|---|---|
| Godiva | +45 ± 40 pcm | **+87 ± 37 pcm** |
| Jemima | **−193 ± 45 pcm** | −96 ± 43 pcm |

The sign differs between the two systems, and Jemima's magnitude exceeds the
predicted bound. A post-hoc reading, **not** a tested hypothesis:
- in bare HEU Godiva, a softer delayed neutron leaks less, which raises k;
- in U-238-rich Jemima, it is born below the U-238 fission threshold and
  loses importance, which lowers k.

The prediction treated the importance loss as dominant everywhere.

**Against OpenMC (route 1), after this change:**

| case | route 3 | route 4 |
|---|---|---|
| Godiva | +14 ± 37 | +33 ± 40 |
| Jemima | **−89 ± 34 (2.6σ)** | −1 ± 39 |

Delayed spectra carry OpenMC's physics, so both routes should now agree with
OpenMC. Godiva on both routes and Jemima route 4 do. **Jemima route 3 does
not.** Route 3 − route 4 is −88 ± 45 pcm, whereas the two routes agreed before
this change. That residual is under investigation on #365.
