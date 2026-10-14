# NJOY2016 thermal ACE reference tables and their decks

Reference tables for `crates/njoy-outram-park-fork/tests/acer_thermal_word_parity.rs`
(GitHub #865) and `tests/acer_thermal_temperature_parity.rs` (GitHub #866),
and the NJOY2016 decks that made them.

- **Producer:** NJOY2016 2016.79, commit `ac5adf5`, built from source on this
  machine (`cmake`, gfortran), run 2026-10-14.
- **Inputs:** ENDF/B-VIII.0 tapes from `reference-data/endf/` (their provenance
  is in that folder's README): `n-013_Al_027-ENDF8.0.endf` with
  `tsl-013_Al_027-ENDF8.0.endf`, `n-001_H_001-ENDF8.0-Beta6.endf` with
  `tsl-HinH2O.endf`, `n-006_C_012-ENDF8.0.endf` with
  `tsl-crystalline-graphite.endf`, and (since GitHub #866)
  `n-001_H_001-ENDF8.0-Beta6.endf` with `tsl-HinZrH-ENDF8.0.endf`.
- **Deck:** `reconr` (0.001) → `broadr` (0.001, the S(α,β) base temperature,
  or the table's temperature for the #866 temperature tables) →
  `thermr` (16 cosines, `iform = 0`, card-4 `tol`, `emax = 4 eV`) → `acer`
  (`iopt = 2`), every unit positive (text). Tape 20 is the incident tape,
  tape 21 the thermal tape; run as `njoy < deck`.
- **Licence:** ENDF/B-VIII.0 is public; these are NJOY's processed output of it.

| table (in `reference-data/acer/`) | deck | SHA-256 |
|---|---|---|
| `al27_20k_thermal_tol0p05_njoy2016.ace.gz` | `al27_20k_tol0p05.njoy` | `2ea619f4e51ea2c2e6d45145b29642f697d5182374066209349ec86d30a60a35` |
| `al27_20k_thermal_tol0p001_njoy2016.ace.gz` | `al27_20k_tol0p001.njoy` | `ea350f7780bc57a6b4202b57cf14820437c598442ff5f3e6ab7ffb2897e975ef` |
| `al27_20k_thermal_tol0p001_iwt0_njoy2016.ace.gz` | `al27_20k_tol0p001_iwt0.njoy` | `2bb567894584e6788099e042abc8021e78d25375333a74c71a253a657df97aa4` |
| `hh2o_283p6k_thermal_tol0p001_njoy2016.ace.gz` | `hh2o_283p6k_tol0p001.njoy` | `5040012bf051183320a0905135a129e3472fc963f39dc132104f613285559ec0` |
| `graphite_296k_thermal_tol0p001_njoy2016.ace.gz` | `graphite_296k_tol0p001.njoy` | `76c9296f35f151533762551b88d1483a564612def453fc3e8a61ef4f83d96f70` |
| `hzrh_296k_thermal_tol0p001_njoy2016.ace.gz` | `hzrh_296k_tol0p001.njoy` | `632e1e4fba40d19e79e78c89c727d6508892858cf6bdb8d6fa5e7b8b721c1495` |
| `graphite_600k_thermal_tol0p001_njoy2016.ace.gz` | `graphite_600k_tol0p001.njoy` | `ba9f969c54d2a02cf93df2b1ba458209bbb9cb04fb8ca22f3e79dcabfe926b28` |
| `graphite_600p5k_thermal_tol0p001_njoy2016.ace.gz` | `graphite_600p5k_tol0p001.njoy` | `6cf5a577f04d33ce5493b46d1037d913b1c3146fa29813f670b6582056877d99` |
| `hh2o_293p6k_thermal_tol0p001_njoy2016.ace.gz` | `hh2o_293p6k_tol0p001.njoy` | `f968d3f38ed36d44778b3df564e60b0cb19f86ab8526ebae0d5c671d5ab7e403` |
| `hh2o_293p9k_thermal_tol0p001_njoy2016.ace.gz` | `hh2o_293p9k_tol0p001.njoy` | `78b85f4f732cb26ec67b837b96e7a1cae74c17f296be5d6b52f6d36e8d6cb732` |
| `hh2o_leapr_293p6k_thermal_tol0p001_njoy2016.ace.gz` | `hh2o_leapr_293p6k_tol0p001.njoy` | `8199044bccaf9d96e7b82e891cb2f3f0161c733f3b43dee8de55e91650480d06` |


**The LEAPR deck (GitHub #871, `tests/process_api.rs`).**
`hh2o_leapr_293p6k_tol0p001.njoy` does not read a thermal tape: it runs
`leapr` first, on the ENDF/B-VIII.0 H-in-H₂O deck cut to 293.6 K (the cards
of `reference-data/leapr/tsl-HinH2O-293.6K-njoy2016-leapr.njoy-input`, whose
LEAPR tape this run reproduces byte for byte), writing tape 24; then
`reconr` (0.001) and `broadr` (0.001, 293.6 K) of
`n-001_H_001-ENDF8.0-Beta6.endf` (tape 20); `thermr` on tape 24 with
`natom = 2` and `emax = 10.00008` eV (the LEAPR law's `B(6)` and `B(4)`,
as OpenMC's `make_ace_thermal` takes them), tol 0.001, 16 cosines; `acer`
with 16 outgoing energies, `iwt = 1`. Run 2026-10-14 (16 s) with the same
NJOY2016 build. It is the oracle for `process::thermal_ace`, which takes a
LEAPR deck and never a thermal tape (GitHub #870).

The H in ZrH deck (GitHub #866) reads `tsl-HinZrH-ENDF8.0.endf` (MAT 7,
`LTHR = 2`) with `n-001_H_001-ENDF8.0-Beta6.endf` as tape 20; THERMR card 2
has `icoh = 1` and `natom = 1`, ACER card 9 `mte = 223`, `ielas = 1`
(incoherent elastic), `nmix = 1`. Run 2026-10-14 with the same NJOY2016 build.

The `.gz` files are `gzip -9 -n` of NJOY's `tape30`.

The four tables at temperatures other than the evaluation's base temperature
(GitHub #866, `tests/acer_thermal_temperature_parity.rs`) come from the same
deck with the temperature changed on the `broadr`, `thermr` and `acer` cards.
600.5 K and 293.9 K are not on the MF=7 tapes; THERMR's `calcem` accepts them
because they are within `T/500` of 600 K and 293.6 K, and prints "difference
between temperatures desired and found".

**Decks NJOY2016 refuses** (no table; `***error in calcem***desired
temperature not found.`, exit status 77, run 2026-10-14). They are committed
because the gate asserts this crate refuses them too:

| deck | why |
|---|---|
| `graphite_450k_tol0p001.njoy` | between the tabulated 400 K and 500 K |
| `graphite_603k_tol0p001.njoy` | 3 K from 600 K: outside `calcem`'s `T/500` (1.206 K), though inside `rdelas`'s `T/1000 + 5` K |
| `hh2o_330k_tol0p001.njoy` | between the tabulated 323.6 K and 350 K |

~~**Decks whose tables are not committed**~~ **CORRECTED 2026-10-14:** these
three tables are now in the `reference-data/ace` submodule
(`reference-njoy/endf-b-viii.0/thermal/`, with stamped decks beside them) and
gated by `tests/acer_thermal_word_parity_large.rs`. The decks here differ from
those only in the comment card; the hashes below are of the unstamped runs
(2–7 MB gzipped):

| deck | table SHA-256 (gzipped, as measured) |
|---|---|
| `al27_20k_tol0p001_iwt2.njoy` | `975c77e850ca18de351742c949709cedf5b07a2565df91e0dd8b1dd647c65e59` |
| `hh2o_283p6k_tol0p001_nbint64_iwt0.njoy` | `396cac55a80335de51ea212413fbf1df94029cd0a3ed7813e73a61a029c96c81` |
| `hh2o_283p6k_tol0p001_nbint64_iwt2.njoy` | `73e81426665d37c86fdba5230b096cd1e8934cd724dab42074af34eee3af050f` |

`gzip -n` output is stable, but `tape30`'s header carries NJOY's run date, so
a re-run on another day gives a different hash; the bytes from line 2 on do
not change.
