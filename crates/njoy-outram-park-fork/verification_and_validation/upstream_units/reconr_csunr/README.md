# RECONR `csunr1`, `csunr2`, `unfac`, `gnrl` against the compiled upstream (gh:#1448)

Differential test of the translation in `src/reconr/csunr.rs` against NJOY2016.80
(`ab27c6414a`, 2026-10-08), `reconr.f90` SHA-256
`054ede7a59e1c39cf3e72105d8a0b95a0fb1d8df0882eca6b949e765b62bf5db`.

## Methodology

- `extract.sh` copies `reconr.f90` lines 3826-4317 (`csunr1`, `csunr2`) and 4473-4644
  (`unfac`, `gnrl`) **verbatim** after checking the SHA-256.
- `driver.f90` holds `reconr`'s state variables (`res`, `ibaset`, `isect`, `ilast`,
  `enxt`, `elst`, `snxt`, `slst`) in a module that `include`s the extracted text, links
  NJOY's own `locale`, `mainio`, `util`, `phys` and `endf` modules unmodified, fills the
  flat `res` array in the layout `rdf2u0/1/2` store (documented in the driver) from
  `cases.in`, and prints results as hex bits. `run.sh` builds (gfortran 13.3.0, `-O2`,
  no `-march`) and writes `upstream_output.txt` (committed).
- `cases.in` is the one input both sides read: five materials (Case A with `NLS = 4`
  so `unfac`'s `l >= 3` fall-through acts; Case A with `NAPS = 1`; Case B `LFW = 1` with
  panels narrower and wider than `wide = 1.26` and energies landing exactly on
  tabulated fission-width energies; two Case C with `INT = 2` and `5` mixed across
  J-states and uneven J-state grids), each at an ascending list of energies as `genunr`
  calls them (panel state carried). `gnrl` is called directly on every branch (6
  `gbeta`/`df` regimes x `mu`, `nu`, `lamda` = 1..4 x `id` = 1..3 = 1 152 calls, and the
  three early-return guards) and `unfac` for `l = 0..3` on sentinel `vl`, `ps`.
- Not covered: `NRO = 1` (energy-dependent `AP(E)`, `terpa`); it is the already-ported
  `UnresolvedRange::scattering_radius`, gated at deck level.

## Prediction (written before the first run)

Every value bit-identical: same operation order, only IEEE `+ - * /` and `sqrt` plus
`sin`/`atan` from PETIR's `RealMath`.

## Result (2026-11-11), `tests/reconr_csunr_code_to_code.rs`

| quantity | compared | bit-identical |
|---|---|---|
| `unfac`, `l = 0..3` x 3 radii | 12 calls (24 values) | 24 / 24, `l = 3` leaves `vl`, `ps` unchanged |
| `gnrl`, all branches | 1 152 + 3 guards | all |
| `csunr1`/`csunr2`, 88 calls | 352 values | **342 / 352** |

The prediction was **wrong for 10 values**: the total and the elastic cross section of the
five Case B calls at 1.5e4, 3e4, 4.1e4, 4.2e4 and 4.4e4 eV differ by **one ulp** (worst
2.9e-16 relative). Cause, shown by experiment: those are the values that carry `sin(ps)`;
replacing PETIR's `RealMath::r_sin` by Rust `std`'s `f64::sin` (the system libm, glibc
2.39, which gfortran also calls) gives **352 / 352** bit-identical. So PETIR's `r_sin`
is not glibc's `sin` at some arguments. The crate rule (gh:#985) is that every
transcendental goes through `RealMath`, so the port keeps `r_sin`; the difference is
within the 1e-8 round-off allowance with a demonstrated cause and is **flagged for the
maintainer** (it may be worth a PETIR-vs-glibc `sin` sweep). The test pins the count
342 and bounds the worst difference at 4e-16.

## Deck level

`tests/deck_driver_tapes_vs_njoy2016.rs`: tests 07, 10, 19 PENDF tapes after BROADR/HEATR
are line-for-line identical (before: 56, 6, 6 differing lines).
