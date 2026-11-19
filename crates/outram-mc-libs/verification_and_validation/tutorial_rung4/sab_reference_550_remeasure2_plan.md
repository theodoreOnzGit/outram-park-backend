# gh:#550 second re-measure after the S(a,b) default flip: plan and predictions (2026-11-19)

Research, education and V&V only. **Committed BEFORE any number below is
re-measured.** A prediction that misses stays in place; outcomes go in
`sab_reference_550.md` ("Second re-measure").

Maintainer decision, 2026-11-19: "Re-measure the rest". The first re-measure
(`33af080b9`) covered the LCT-008 pin cells, HST-009 and the LCT-008 lattice;
its "Not re-measured" list is this work list. The LCT-008 cases are NOT run here
(another agent owns the 2.40 cm pin-cell gap).

## The instrument for a paired old-build control

`from_tape` and `from_tape_with_options` now honour a named environment
ablation, `OUTRAM_SAB_LOG_SPACED_GRID=1` (unset/empty/`0` = default, physics
on), which builds the pre-flip log-spaced tables (`from_tape_log_spaced_grid`
arithmetic). It reaches every consumer, including the LEAPR routes and
`nee_soon`, with no second checkout (a second target dir does not fit the
disk). Neither function is in a kovan review block
(`src/material/review.md`: zero `kind = "review"` blocks). Reached by every
test that builds an ENDF law.

## Does the path reach the ENDF-route thermal build? (read from the code, 2026-11-19)

| item | reaches `from_tape`? | evidence |
|---|---|---|
| `htr10_pebble_delta_tracking` | yes | `from_endf_file` at lines 186, 201 |
| `htr10_fuel_zone_kinf` | yes | `from_endf_file` at line 212 |
| `nee_soon::htr10_rmc` (default `ThermalLawSource::Leapr`) | yes | `data.rs` `ThermalScattering::from_leapr` -> `from_tape`; jobs path `data_jobs.rs:314-325` |
| `fhr_ring_rpt_endf`, `fhr_pebble_target_at_rest_ablation` | yes | `from_endf_file` (lines 307, 246) |
| `msre_n1_first_criticality` | yes | graphite `from_embedded_leapr_deck` (429), H2O `from_endf_file` (439) |
| `icsbep_five_route_keff` | yes (routes 3-5 ENDF/LEAPR; routes 1-2 ACE do not) | doc lines 38-48 |
| `dh_keff_vv` | yes | `from_endf_file` at 364 |
| `thermal_kernel_keff_worth` | **no** | calls `from_tape_with_grids` explicitly (line 209), the log-spaced constructor: unchanged by the flip. A fifth arm (`from_tape`, the node grid) is added (`OUTRAM_TKW_PAIR=1` runs 384x64 vs node grid only) so the flip's worth on this medium is measured directly |
| `pincell_depletion_vs_openmc` | **no** | `ThermalScattering::from_ace` (line 126) and `Nuclide::from_ace_file`: ACE S(a,b), no ENDF thermal build. Does not move |
| `openmc_notebooks/pincell` | yes | `ThermalScattering::from_endf_file` (line 539); a test with its own gates, run as a test |
| `dhoby-ghaut` processed-law cache | yes, **and stale** | `DataStore::thermal` calls `from_tape`; the key carries `CODE` (`nd-2026-10-18.1`), whose doc says to bump it when THERMR or LEAPR changes what a product holds. The flip did, and `CODE` was not bumped: a browser holding a pre-flip law would serve it as current. Plan: bump `CODE`, pin a thermal-law hash |
| MSRE "NC-1" | n/a | no case of that name exists in the repository (grep of .rs/.md/.toml/.sh, 2026-11-19); the MSRE cases are N-1 and N-3. Reported as not identifiable; N-1 is run |

## Numbers recorded with the log-spaced build, protocol, predicted move

Prediction basis: the first re-measure moved water systems up (+32, +107, +64,
+39 pcm, the pooled pin cells +68 +- 26) and the graphite stationary
distribution up by +2.1 K (606.29 K against 604.17 K, nominal 600 K), a hotter
neutron temperature. In a graphite-moderated U-235 system a hotter thermal
spectrum lowers k (fewer neutrons per fission-weighted absorption at the
thermal peak, as the HTR moderator-temperature coefficient says), so the
graphite prediction is **small and negative**; the staircase noise is
zero-mean. Sizes are rough: 2 K of neutron temperature at a few pcm per K.

| id | case | old (record) | protocol | predicted move |
|---|---|---|---|---|
| G1 | graphite medium k_inf, `thermal_kernel_keff_worth`, 384x64 (log-spaced) vs node grid | 1.38449 (sd 199) log-spaced, N=6, 2026-11-17 | `-- 4000 20 80 8`, `OUTRAM_TKW_PAIR=1`, 8 replicas, paired by seed | **-15 pcm** (range -90 .. +60); resolution about 100 pcm: unresolved expected |
| G2 | `htr10_fuel_zone_kinf`, het / hom / worth | 1.57302 (sd 279) / 1.44751 / -12551 +- 123, N=12, 2000 x [20+60] | seeds 1-12, default vs `OUTRAM_SAB_LOG_SPACED_GRID=1` in the same session | het -10 pcm (-80 .. +60); worth unchanged within 150; unresolved expected |
| G3 | `htr10_pebble_delta_tracking`, five boron arms, natural-B atom ppm 1.68681 (sd 589), N=24, 300 x [20+60] | as recorded | seeds 1-12 (half the record), default vs ablation | -15 pcm, resolution about 250 pcm: **will not resolve**; boron deltas unchanged |
| G4 | `fhr_pebble_target_at_rest_ablation` MOVING 1.36307 (sd 117), AT-REST 1.36154 (sd 141), worth -153 +- 43; the same arm as `fhr_ring_rpt_endf` OUTRAM_RINGRPT_ONLY=csg (bit-identical per seed, record `sab_per_material_1581`) | N=8, 4000 x [30+80], 600 K | seeds 1-8 default vs ablation | MOVING -10 pcm (-90 .. +70); worth unchanged within 100 |
| G5 | `dh_keff_vv`, delta tracking 1.38647 +- 0.00211 and ring-RPT, 7200 histories (older doc record, hardware not the same) | single draw | `OUTRAM_DH_VV_HISTORIES=7200 OUTRAM_DH_ONLY=delta,ring`, default vs ablation, same seed | -10 pcm, sigma 210 per arm: unresolved |
| G6 | `nee_soon` `htr10_rmc_keff`, N=12 record 0.995309 +- 0.001098 (10000 x [5+135], 16 threads, 2945 s transport) | 2026-11-05 | the record protocol costs about 6.5 h on 2 shared threads (33 ms per history per thread measured there): **not affordable**. A reduced paired pair is attempted only if time remains; sigma would be about 500 pcm, unresolved | -10 pcm, unresolved by construction |
| W1 | MSRE N-1, 1.01807 +- 0.00023 (6 seeds) | 20000 x [100+200], 4 threads, about 875 s a seed | seeds 1-3 default only (not paired: the ablation would double the cost), compared with the 6-seed record | graphite at 911 K plus H2O in the shield: -10 pcm (-80 .. +60); resolution about 40 pcm |
| W2 | `icsbep_five_route_keff` route `endf`: hst009 1.00080 (sd 152, N=16), msre unit cell 1.65536 (sd 118, N=8) | 5000 x [40+120], 2 threads | hst009 16 seeds default vs ablation; msre unit cell 4 seeds default only, if time | hst009 (H2O, the first pass's +39 +- 26 was HST-009 itself on a different run) +40 pcm (-30 .. +120); msre unit cell -10 |
| W3 | five-route routes 4 and 5 (LEAPR deck) | same | the LEAPR route builds through `from_tape` too; run hst009 on route 4 if time | same as W2 |
| T1 | `openmc_notebooks/pincell` test | its own gate | run the test | passes without re-pin |
| T2 | `dhoby-ghaut` processed_cache tests | pins | run the tests; bump `CODE` | pass; hashes of neutron products unchanged (they contain no thermal data) |
| P0 | `pincell_depletion_vs_openmc` | pinned/recorded | not run: uses ACE S(a,b), so by construction it does not move | no move |

Order: G1, G2 first (graphite, cheapest resolution), then G4, G3, G5, then
W2, W1, T1, T2. At most two heavy processes at once, 2 threads each. Anything
not run is reported "not re-measured" with the reason.

## Addendum, written after block 1 of G1 and before block 2 was read (2026-11-19)

Block 1 of G1 (`OUTRAM_TKW_PAIR=1`, 8 replicas, master seeds 20260912..19):
node grid minus log-spaced **-234.1 +- 108.4 pcm (z -2.2)** against the
prediction -15 (-90 .. +60): **the prediction missed, same sign, outside its
range.** A block chosen after seeing a 2.2 sigma result is a replication, not
a second sample of the same hypothesis. Block 2 uses `OUTRAM_TKW_SEED0=1000`
(master seeds 20261912..19, non-overlapping), 8 more replicas, same settings,
run once; it was started before this paragraph was committed but its output
had not been read. Prediction fixed now: if the effect is real and about -230
the block gives -230 +- 108 (range -450 .. -10); if the true value is the
originally predicted -15 the block gives -15 +- 108 (range -230 .. +200). Both
blocks are reported, pooled N = 16 only as this pre-announced pool, and block 1
is never dropped.

Also found while planning: G2's ablation arm must reproduce the 2026-11-17
record bit for bit if the knob is a true old-build control; checked below.
