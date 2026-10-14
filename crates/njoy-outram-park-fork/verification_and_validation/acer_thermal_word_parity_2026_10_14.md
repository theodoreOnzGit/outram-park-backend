# Thermal S(α,β) ACE tables vs NJOY2016, word for word (2026-10-14)

**Class:** verification, code to code against upstream. Not validation: no
experiment is compared against. AI-assisted draft, not yet reviewed by a human.
**Issue:** GitHub #865. **Gate:** `tests/acer_thermal_word_parity.rs` (5 cases,
13 s; a sixth, H in ZrH with incoherent elastic, was added later the same day
for GitHub #866, see
[`acer_thermal_incoherent_elastic_2026_10_14.md`](acer_thermal_incoherent_elastic_2026_10_14.md)). **Reference:** NJOY2016 2016.79 (`ac5adf5`), built and run here.

## Result

**Every thermal table built here is now NJOY2016's in every compared word,
and in every byte of its Type-1 text but the date and the comment.** Before
this date the port matched NJOY only in the incident-energy grid (ITIE). The
`ITXE` emission block (28 832 words on Al-27) differed in every word; ITIX
differed by up to 2.2 %; the Bragg block had 568 edges where NJOY has 309.

| case | T (K) | tol | natom | form (`iwt`) | committed | result |
|---|---|---|---|---|---|---|
| Al-27, Al-27 incident | 20 | 0.05 | 1 | equiprobable (1), 16 x 16 | yes | identical |
| Al-27 | 20 | 0.001 | 1 | equiprobable (1), 16 x 16 | yes | identical |
| Al-27 | 20 | 0.001 | 1 | skewed (0), 16 x 16 | yes | identical |
| H in H₂O, H-1 incident | 283.6 | 0.001 | 2 | equiprobable (1), 16 x 16 | yes | identical |
| crystalline graphite, C-12 incident | 296 | 0.001 | 1 | equiprobable (1), 16 x 16 | yes | identical |
| Al-27 | 20 | 0.001 | 1 | continuous (2), 16 bins | no (3.1 MB gz) | identical |
| H in H₂O | 283.6 | 0.001 | 2 | skewed (0), 64 x 16 | no | identical |
| H in H₂O | 283.6 | 0.001 | 2 | continuous (2), 64 x 16 | no (6.6 MB gz) | identical |

"Identical" means `acer::compare` reports every header field (ZAID, AWR, kT,
material, IZ/AW), NXS, JXS, every block and the whole XSS the same, with no
tolerance. The committed cases are also asserted byte for byte as text. The
three uncommitted cases were compared once with a different table name, and
only the ZAID differed; their decks are committed in
`reference-data/acer/thermal_decks/`, with the tables' SHA-256 in its README.

## Methodology

Each NJOY2016 deck is `reconr` (0.001) → `broadr` (0.001, the S(α,β) base
temperature) → `thermr` (16 cosines, `iform = 0`, card-4 `tol`,
`emax = 4 eV`) → `acer` (`iopt = 2`, card 8a the scatterer's ZA), every unit
positive, as OpenMC's `make_ace_thermal` writes its decks. This crate runs
RECONR + BROADR on the same incident tape, reads the same thermal tape, and
calls `AceTable::thermal_from_mf7_on_pendf` with the same settings, on the
incident energies `ace_incident_grid(emax)` keeps.

**How the differences were found.** A copy of NJOY2016's `thermr.f90` was
instrumented to write, at full precision:
- `sig`'s inputs and output (`fort.98`);
- each `sigl` result (`fort.97`);
- `tpend`'s `xsi` (`fort.96`);
- `tpend`'s raw densities (`fort.95`).

The patch is `acer_thermal_word_parity/thermr_instrument.diff`. Its tables
are byte-identical to the uninstrumented build's. Our code was then evaluated
at exactly the logged inputs:
- `sig`: 300 000 consecutive calls, then 38 877 sampled across the whole run;
  all bit-identical after the fixes;
- `sigl`: 32 541 calls, all identical;
- `tpend`'s MF=6 records: 107 of 107 identical, read from NJOY's own
  intermediate PENDF.

Each stage was made exact before the next was looked at. That is why the
defects below are known one by one, rather than as one residual.

## What differed, and why (each fixed by following upstream)

1. **`sig` had no `sigmin` cut.** Upstream ends with `if (sig.lt.sigmin) sig=0`,
   `sigmin = 1e-10`. Without it every Al-27 record carried about 32 more
   outgoing energies, down to 1e-29 b/eV. The cut is applied on THERMR's path
   (`thermr::calcem::sig`), not in the shared kernel.
2. **`sig`'s arithmetic** (`thermr::inelastic`):
   - its own `terp1` grouped `(y2-y1)*(x-x1)/(x2-x1)` where `endf.f90:1431`
     has `(x-x1)*(y2-y1)/(x2-x1)`;
   - `bb` and `sigc` divided by `tev` where upstream multiplies by `rtev`;
   - α was not floored at `amin = 1e-6`;
   - `ln S` was floored at `S > 0`, not `S > exp(sabflg)`
     (`thermr.f90:1748-1751`);
   - an `arg > 20` cap had no upstream counterpart;
   - the short-collision-time branch recomputed α and β, where upstream scales
     the LAT = 1 table variables back (`b*tevz*rtev`), up to 2e-14 different.
3. **`sigl`'s `third` is the literal `.333333333`** (`thermr.f90:2687`), not
   `1/3`. That moved every equally-probable cosine by about 5e-10.
4. **`gral=gral+A+B` is `(gral+A)+B`.** In both `sigl` and `acesix` the port
   added `gral+(A+B)`, which put one ulp in the last cosine.
5. **`tpend` normalises with `xsi`, rounds, and the text rounds again**
   (`thermr.f90:3302-3335`). Each density is divided by the stored `xsi(ie)`
   and rounded with `sigfig(·,7)`, or 6 figures below 1e-9. Before that,
   `calcem`'s rows have already been through the scratch tape `nscr`, which
   is text when the output tape is (`:297-298`). The port divided by a
   recomputed trapezoid at full precision. Now `acer::acesix::tpend_rows`.
6. **`acesix`'s arithmetic** (`aceth.f90:434-453`): the moment's intercept,
   the order in which `grall` is accumulated, and the cosine interpolation,
   each now grouped as upstream writes them.
7. **ITIX is read off THERMR's MF=3, not computed at each energy.** ACER
   interpolates linearly (`gety1`) on the PENDF. That PENDF is:
   - the incident nuclide's elastic grid up to `emax`, plus `emax` and
     `1.00001·emax`;
   - `calcem`'s `terp(esi,xsi,·,5)` at those points;
   - `coh`'s points merged in, each carrying `terp(x,z,5,E,4)`;
   - all rounded by `tpend` (`sigfig(·,7)`) and the text.

   Port: `thermr::mf3_grid` (`inelastic_mf3`, `itix_at`), fed by the new
   `IncidentPendf`. Without the incident PENDF, ITIX differs by up to 1.1e-3
   on Al-27.
8. **The Bragg edges are ACER's, not the tape's** (`aceth.f90:188-214`).
   `coh` puts PENDF points one 7th-figure unit either side of each edge.
   `tpend` rounds them, and ACER keeps a point only where `E·σ` rises by more
   than `(clast+1)/10⁶`, stopping at `emax`. Al-27: 568 → 309.
   `acer::thermal::aceth_bragg_edges` follows those points. Running ACER's
   loop over NJOY2016's own PENDF and over this two-points-per-edge grid
   keeps the same 309 edges.
9. **The header.** Each field now comes from where ACER takes it:
   - AWR is the incident PENDF's (`aceth.f90:121`): graphite 11.89365, not
     the MF=7 tape's 11.898;
   - the material tag is the incident MAT;
   - IZ is card 8a's ZA list;
   - the ZAID's `tname` is right-justified in 6 columns
     (`acer.f90:386-387`).
10. **The demo's grid had one incident energy too many.** `egrid_count` is
   `calcem`'s `nne`, one point past `emax`; ACER drops it
   (`aceth.f90:329-330`). Now `acer::thermal::ace_incident_grid`.

## Effects elsewhere

- `outram-mc-libs`' H₂O anchor σ_inel(0.0253 eV, 293.6 K): 51.6870 → 51.6875 b,
  i.e. onto NJOY2016's 51.68752 b. Re-pinned in `thermal_graphite_elastic.rs`;
  the other 20 thermal tests there pass unchanged.
- `tests/acer_thermal_vs_njoy2016.rs` (CSV oracle, no incident PENDF):
  - inelastic worst 2.238e-2 → 1.081e-3;
  - edges 568 → 309;
  - cumulative `S` worst 3.253e-6 → 4.4e-10.

  Its gates were not moved.
- **Not re-measured:** the five-route ICSBEP `k` values with thermal
  scattering. The one thermal cross section checked here moved by 1e-5
  relative; what that does to `k` has not been measured.

## What is NOT covered

- Only each evaluation's base temperature (as before).
- ~~Incoherent elastic (H in ZrH, `IDPNC = 3`) and mixed elastic (`IDPNC = 5`)
  are not in the word-for-word gate.~~ **CORRECTED 2026-10-14** (GitHub #866):
  H in ZrH at 296 K (`IDPNC = 3`) is in the gate and identical in every word
  and every byte but the date and comment; record
  [`acer_thermal_incoherent_elastic_2026_10_14.md`](acer_thermal_incoherent_elastic_2026_10_14.md).
  Mixed elastic (`IDPNC = 5`) is still not gated: no evaluation in
  `reference-data/endf/` has `LTHR = 3`.
- LASYM = 1 evaluations and `nmix > 1` are not exercised.
- Decks with binary (negative) units: NJOY then skips the text round trips,
  and this port reproduces the text deck only.
- OpenMC's complete deck is not run. It adds a free-gas `thermr` pass
  (MT 221) and a 64-bin, `iwt = 2` `acer`; the 64-bin, `iwt = 2` table itself
  was measured identical above.
- ~~The IFENG = 2 tables are measured, not gated, because of their size.~~
  **CORRECTED 2026-10-14:** gated by `tests/acer_thermal_word_parity_large.rs`
  (Al-27 `iwt = 2`; H in H₂O 64 bins `iwt = 0` and `2`), tables in the
  `reference-data/ace` submodule; all three identical (43 s).

## Reproducing

```bash
cargo test --release -p njoy-outram-park-fork --test acer_thermal_word_parity -- --nocapture
# a reference table: NJOY2016 on a committed deck
cd <scratch> && ln -s <repo>/reference-data/endf/n-013_Al_027-ENDF8.0.endf tape20 \
  && ln -s <repo>/reference-data/endf/tsl-013_Al_027-ENDF8.0.endf tape21 \
  && njoy < <repo>/reference-data/acer/thermal_decks/al27_20k_tol0p001.njoy
```
