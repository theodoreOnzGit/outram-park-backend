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
| ratio band mean / dilute | ~~**0.98658** (1.3 % lower)~~ **CORRECTED 2026-10-19**: sampling noise, see the 2026-10-19 section (stratified, the ratio is 1.00000) |
| seeds where dilute total != flight total | 400 of 400 |

Prediction held on sign; the magnitude (1.3 %) is slightly above my guess.
The 400-seed ratio carries sampling noise from the cheap tables that was not
quantified. The previously dilute-scored U-238 URR absorption was therefore
about 1.3 % high at this energy.

~~**Not measured: an end-to-end tallied rate on a case with real URR flux.**
No such existing cheap fixture was run; the Godiva diagnostics put little flux
in the URR and cannot resolve it (see below).~~ **CORRECTED 2026-10-19**: measured,
see the section below.

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

## End-to-end check on a case with real URR flux (2026-10-19)

Test: `tests/urr_tally_end_to_end_953.rs` (3 tests). Hardware: i9-13900K,
4 of 16 logical cores pinned (`taskset -c 0-3`), 64 GB, Linux 7.2.9, CPU only,
release. ENDF/B-VIII.0 at 293.6 K, URR tables with the cheap PURR settings of
the test above (20 bins, 4, 500 samples). Not yet human-reviewed.

### Two corrections to the section above

1. The "1.3 % lower" band-mean ratio (0.98658) was sampling noise of 400 random
   seeds. With 20 000 stratified band variates the band mean of Sigma_a equals
   the dilute value to 5 digits at 30, 50, 70, 100, 130 and 140 keV (the tables
   are normalised). "Slightly above my guess" in the prediction paragraph is
   therefore also withdrawn: there is no band-mean shift.
2. The real pre-#953 error is **flight-weighted**, not a band-mean shift. The old
   track-length score was `Sa_dil * d` with `d` flown on the band total, so
   old / true = `Sa_dil * E[1/St_b] / E[Sa_b/St_b]` over bands.

| E (keV) | 30 | 50 | 70 | 100 | 130 | 140 |
|---|---|---|---|---|---|---|
| old / true (U-238, tables only) | 1.1065 | 1.0645 | 1.0449 | 1.0306 | 1.0213 | 1.0219 |

### Predictions (posted on the issue before each run)

- Table-only mean over the six energies 1.048, so the old track length reads
  high by **+3 to +8 %** in 25-145 keV. First guess +1.3 % was revised before
  the end-to-end run, for the reason above.
- Current code: track length / analog = 1.000 within statistics.
- Delta cube: three estimators agree pairwise within 4 sigma, no majorant
  violation, `k` of the two delta arms bit-identical.

### (a) track length against (c) analog capture, same histories

Bare U-238 metal sphere (0.0482 /b.cm, 15 cm, vacuum), analog point source at
140 keV (inside the 20-149 keV URR), 40 batches x 5000 histories, one seed
per batch. Window 25-145 keV, wholly inside U-238's URR. The analog count is
every `Absorption` and `Fission` state of the recorded tracks whose energy lies
in the window. Why this case has URR flux: the source sits in the URR and
U-238 loses at most 1.7 % of E per elastic collision, so almost every
neutron is absorbed or inelastically scattered within it. Measured: **93.9 %
of all absorptions fall in the window** (the rest are absorbed below
25 keV).

| code | track length | analog | ratio | paired z |
|---|---|---|---|---|
| current (develop) | 0.42633 +- 0.00066 | 0.42606 +- 0.00112 | **1.00064** | +0.19 sigma |
| pre-#953 (parent `4cfb195801^`, scratch worktree) | 0.44585 +- 0.00071 | 0.42606 +- 0.00112 | **1.0464** | +13.3 sigma |

Absorptions per source neutron in the window. The analog count is the same
number on both codes, so the pre-fix scoring changed no history. Prediction
(+3 to +8 %) held: +4.6 %, inside the table-only range (1.02-1.11).
Combined sigma: unpaired z +0.21 (current), +14.9 (pre-#953).

### (b) collision estimators under delta tracking

Reflective 20 cm UO2 cube (N = 1.16e-3 U-235, 2.21e-2 U-238, 4.65e-2 O-16
/b.cm, no moderator), URR tables on U-235 and U-238, k-eigenvalue 2000 x
(20 + 30), seed 1, single thread; `R = window absorption / window flux`
(flux-weighted mean Sigma_a, 1/cm). Tentative and real-collision arms share
the histories (`k` bit-identical, asserted).

| arm | current | pre-#953 |
|---|---|---|
| surface track length | 8.530e-3 +- 4.5e-5 | 8.759e-3 +- 4.6e-5 |
| delta, tentative collision | 8.523e-3 +- 4.9e-5 (-0.08 %, 0.11 sigma) | 8.754e-3 +- 5.1e-5 (-0.06 %, 0.08 sigma) |
| delta, real collision | 8.523e-3 +- 4.8e-5 (-0.09 %, 0.11 sigma) | 8.728e-3 +- 4.9e-5 (-0.36 %, 0.46 sigma) |

`k`: surface 0.83072 +- 0.00386, delta 0.82517 +- 0.00422 (both codes,
identical); majorant violations 0; 3 638 331 virtual collisions. The sigma of
`R` takes the flux and absorption bins in quadrature (correlation ignored, so
conservative). The current code passes; the pre-#953 code **also passes**, with
every estimator about 2.7 % higher. Reading: the cross-estimator agreement
alone cannot detect this defect, because all three estimators called the same
dilute function. What detects it is comparison with the transport itself, the
analog count in (a). The 2.7 % shift is of the size of the table-only
prediction for this lower-URR-sensitivity mixture (O-16 and U-235 dilute the
flight weighting) but was not predicted in advance for this case.

### Open

- The analog count by energy exists only for fixed-source runs (recorded
  tracks); the k-eigenvalue hybrid has none, so (b) has no analog leg.
- (a) uses a cheap-table U-238 sphere, not an LEU pin cell. No pin-cell
  fixture with a URR window gate exists; building one was not needed to expose
  the defect.
- Godiva diagnostics numbers (above) are still not re-measured.
