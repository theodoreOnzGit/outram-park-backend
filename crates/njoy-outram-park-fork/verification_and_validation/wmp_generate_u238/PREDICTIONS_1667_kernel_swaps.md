# Predictions, written and committed BEFORE any kernel swap or kernel measurement of gh:#1667 (2026-11-19)

> Research, education and V&V only (`RESPONSIBLE_USE.md`). AI-written and unreviewed.

Question (gh:#1667): on U-238 piece 296 the port's best `vectfit_xs` quality is below OpenMC's in 5 of 5 trials
(exact p = 0.008). Which numerical kernel, if any, causes that shift? Candidates left untested by the 2026-11-19
diagnosis (README, "Diagnosis of the dense end on piece 296", conclusion 4): (E) the eigensolver, petir Francis QR
against LAPACK DGEEV; (Q) the unblocked Householder QR against DGEQRF; (V) summation order in `evaluate`;
(R) the residue solves; and (L) the pole-identification least squares as a whole (petir SVD against DGELSD).

## What was already looked at before this file was written (stated so nothing is hidden)

Only the per-iteration quality columns of the existing five-trial traces (`diagnosis_296/*_296_t*.log.tsv`), re-read
with the new `openmc_inputs/tail_stats.py` on the orders 98 to 130 that every trace visited. Quality is the number
`vectfit_xs` maximises; a trace row is one of the 30 vector-fitting iterations of one order.

| code (orders 98 to 130) | iterations | unusable (`-inf`) | quality > 1.5 | per-order best, median (min, max) |
|---|---|---|---|---|
| OpenMC DGELSD (5 trials, as shipped) | 2 520 | 55.5 % | 32 (1.27 %) | 1.460 (1.086, 1.666) |
| port (5 trials) | 2 490 | 46.3 % | 4 (0.16 %) | 1.384 (0.891, 1.532) |
| OpenMC forced onto DGELSS (4 trials) | 2 040 | 53.9 % | 71 (3.48 %) | 1.511 (1.075, 1.650) |

Fisher (one-sided) OpenMC against port on the count above 1.5: p = 1e-6; Mann-Whitney on per-order bests z = +4.5.
Two things follow, and the first corrects the README: **forcing OpenMC onto DGELSS does not move it toward the port, it
moves it away (1.27 % to 3.48 %)**, so the earlier sentence "OpenMC with DGELSS stays in OpenMC's range" understates
that the tail rate is itself sensitive to which LAPACK solver is used, in the wrong direction for the rank-rule
explanation. **The tail rate is a much more powerful instrument than the five best-of-run numbers**, because every
iteration of every order is one draw (about 2 500 draws per condition).

## Instrument fixed in advance

* OpenMC's loop, `multipole._vectfit_xs`, UNMODIFIED, orders 98 to 130 step 2 (the 17 orders both codes visited in the
  unperturbed trace), 30 iterations per order, inputs `in_296_t0` and `in_296_t1` (the same files the earlier traces
  read). Only module-level names of `openmc.data.vectfit` / `openmc.data.multipole` are swapped, to calls into the
  port's own Rust kernels (`generate/npy.rs`, `generate/vectfit.rs::evaluate`) served from a test-only process, so a
  swap changes ONE kernel on identical inputs. The Rust server carries the same functions the library calls; nothing
  in the library is edited.
* Response: count of iterations with quality > 1.5 out of those run (baseline 1.27 %), the per-order best quality, and
  the share of unusable iterations. A swap "moves the shift" if the count falls to at most half the OpenMC baseline
  (at most 0.6 %) with one-sided Fisher p < 0.01 against the OpenMC baseline pool; "does not move it" if it stays
  within the 95 % binomial band of the baseline (about 0.9 % to 1.8 % for 2 500 draws, wider for fewer).
* Controls: (i) the swap server returning LAPACK's own answer (an identity swap through the same pipe) must give the
  baseline rate, else the pipe changes something; (ii) the port's trace already exists (5 trials).
* Static measurements (no chaos): for dumped matrices of pole-identification and residue solves, residual norm
  `||A x - b||` of the port against DGELSD and DGELSS (a solver that is systematically worse in residual would give
  fits that are systematically worse), and for dumped relocation matrices the eigenvalue error of the port and of DGEEV
  against a 40-digit mpmath eigensolution.

## Predictions (sign and size), with my probability that the candidate is THE cause

| id | swap (OpenMC side, port's kernel in) | probability it is the cause | predicted outcome if it is | predicted outcome if not |
|---|---|---|---|---|
| E | `scipy.linalg.eigvals` -> port `eigvals` (petir Francis QR, GSL balancing) | 25 % | tail falls to 0.1 to 0.5 % (sign: down), per-order best median 1.38 to 1.43; static: port eigenvalue error 3x or more DGEEV's | tail 0.9 to 1.8 %, no change in unusable share |
| L | `lstsq` in pole identification -> port `lstsq` (QR then Golub-Reinsch SVD, truncation `s <= eps s_max`) | 30 % | tail 0.1 to 0.6 %; unusable share about 46 % | tail 0.9 to 1.8 % |
| R | `lstsq` in the residue solve -> port `lstsq` | 20 % | tail falls 0.1 to 0.6 %; static: port residual norm larger than DGELSD's in most matrices | unchanged |
| Q | `scipy.linalg.qr` -> port `HouseholderQr` (R, and Q for the right-hand-side row) | 10 % | tail falls to 0.5 % or lower | unchanged |
| V | test-grid `evaluate` -> port `evaluate` (sequential complex sum) | 3 % | a few points of 77 890 change, tail unchanged | unchanged (the only outcome I expect: a round-off in a count at a threshold cannot move 1 % of iterations) |
| none | all of the above are null | 12 % | the shift survives every single-kernel swap; it then needs two kernels together, or it is not a kernel property | |

A combined swap of all five (OpenMC computing with port kernels throughout) is the closing test: it should reproduce the
port's tail (0.16 %) if the kernels are the whole story (prediction: 0.1 to 0.5 % if at least one of E, L, R, Q is the
cause, 0.9 to 1.8 % if the cause is outside the kernels, which would be `fit.rs` / `vectfit.rs` code that two readings
found identical).
Because forcing DGELSS raised the OpenMC tail, I expect the least-squares swap (L, R) to have the largest effect of
any single swap, and either sign is possible for R alone.

A nuisance I expect: the port's kernels are 2 to 4 times slower than LAPACK's on the 15 578 x 131 residue matrices, so
the swapped runs may be limited to fewer orders than the baseline; the baseline is then restricted to the same orders.
