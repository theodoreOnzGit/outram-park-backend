# Tallies score the sampled URR band (gh #953)

Measured 2026-10-18. **Not yet human-reviewed.** Research, education and V&V
only (`RESPONSIBLE_USE.md`); AI-assisted, untrusted until reviewed.

## Upstream (read first)

OpenMC `Nuclide::calculate_xs` (`src/nuclide.cpp:826-828`) calls
`calculate_urr_xs` (`:865`) when `urr_ptables_on && urr_present_ && !use_mp`
and the energy is in the table range. That overwrites the cached
`micro.elastic / absorption / fission / total` with the sampled band values and
sets `micro.use_ptable`. Tallies read the cache; `get_nuclide_xs`
(`src/tallies/tally_scoring.cpp`, ~l.525-540) returns `absorption - fission`
for `(n,gamma)` when `use_ptable`, with the comment "make sure we use the
sampled value ... rather than the dilute average value". Track-length and
collision estimators therefore multiply by the sampled band values.

## Defect

`Material::macro_xs` (dilute) was used at three scoring sites:
`transport_csg.rs` (surface-tracking track length), `delta_tracking/handoff.rs`
(tentative-collision hook and `score_real_collision`). Transport flew with
`macro_xs_total_urr` and collided with `xs_at_energy_urr`, same `urr_xi`.

## Change

New `Material::macro_xs_urr(e, nuclides, urr_seed)`: per nuclide, band values
via `xs_at_energy_urr(.., urr_xi(idx, urr_seed))` when `needs_urr_draw(e)`,
else `xs_at_energy`. No new random draw. The three sites call it with the
history's current `urr_seed`. With tables ablated (`without_urr`), outside the
range, or inside a multipole range, `needs_urr_draw` is false and the result is
bit-identical to `macro_xs`, so k and every non-URR tally are unchanged.

## Prediction (posted on the issue before the change)

Sign: URR absorption/capture scored with band values is lower than dilute
(self-shielding). Magnitude guess: small, under ~1 % of URR capture.

## Test

`tests/urr_tally_scoring_953.rs`: ENDF/B-VIII.0 U-238, 293.6 K, cheap PURR
settings (20 bins, 4, 500 samples; the table values are not asserted), pure
U-238 at 0.0222 /b.cm, 50 keV, 400 URR stream seeds. Asserts per seed: tally
total equals `macro_xs_total_urr` (the flight), tally absorption and fission
equal the `xs_at_energy_urr` collision values; the dilute total differs from
the flight total (the defect was observable); bit-identity at 1 keV, 2 MeV and
with tables ablated.

## Results

| quantity (U-238 at 50 keV, 400 seeds) | value |
|---|---|
| dilute macroscopic absorption | 7.183033e-3 /cm |
| band-mean macroscopic absorption (what tallies now score) | 7.086670e-3 /cm |
| ratio band mean / dilute | **0.98658** (1.3 % lower) |
| seeds where dilute total != flight total | 400 of 400 |

Prediction held on sign; the magnitude (1.3 %) is slightly above my guess.
The 400-seed ratio carries sampling noise from the cheap tables that was not
quantified. The previously dilute-scored U-238 URR absorption was therefore
about 1.3 % high at this energy.

**Not measured: an end-to-end tallied rate on a case with real URR flux.**
No such existing cheap fixture was run; the Godiva diagnostics put little flux
in the URR and cannot resolve it (see below).

## Recorded numbers that may move

`../diagnostics/README.md` (owned by another agent, not edited): Godiva
balance closure (-2.12 sigma; +0.94 / -0.88 sigma on two other runs) and the
per-group absorption / nu-fission rates against OpenMC were taken with dilute
tally scoring. The change can move them only through the URR flux fraction,
which is small in Godiva; they were **not re-run** here. Its "Cost" paragraph
and its remark about gh #953 are now stale. Any other recorded tally in an
LEU or thermal case with URR flux (e.g. HST-009, LCT-008 tallies) is likewise
unre-measured.

## Suite

`cargo test --release -p outram-mc-libs --lib --tests --no-fail-fast`
(4 cores, GPU hidden): 128 test binaries, 895 passed, 1 failed, 14 ignored.
The failure is `pebble_beds::triso_lattice::tests::a_particle_on_a_tile_face_is_in_both_tiles_and_whole`
(`left (0.25, 0.0, 0.25)`, `right (0.25, 0.0, 0.0)`), a pure tile-coordinate
assertion that touches no cross section. Not run on the base commit, so
"pre-existing" is likely but not verified.
