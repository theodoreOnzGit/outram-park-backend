# PURR reference data — NJOY2016 probability tables

`u238-purr-mt152-mt153.pendf` — the **MF=2/MT=152** (Bondarenko self-shielded
cross sections) and **MF=2/MT=153** (unresolved-resonance probability tables)
sections of a PENDF tape produced by NJOY2016, extracted verbatim into a
minimal ENDF tape so the crate's own `Tape` parser can read them.

## Provenance

- **Code:** NJOY2016, built from `upstream_source/NJOY2016` in-session
  (`/home/user/njoy2016-src`, commit `ac5adf5f`), run 2026-09-16.
- **Evaluation:** `reference-data/endf/n-092_U_238.endf` (ENDF/B-VIII.0,
  MAT 9237) — the same tape `njoy-outram-park-fork` reads directly, so any
  disagreement is a port defect rather than a data difference.
- **Deck:**

  ```
  reconr / 20 21 / 'U238 pendf'/ 9237 0/ .001/ 0/
  broadr / 20 21 22 / 9237 1 0 0 0./ .001/ 293.6/ 0/
  purr   / 20 22 23 / 9237 1 1 20 64 1/ 293.6/ 1.e10/ 0/
  ```

  i.e. `ntemp=1` (293.6 K), `nsigz=1` (`sig0 = 1e10`, infinite dilution),
  `nbin=20`, `nladr=64`, `iprint=1`.

## Layout of MT=153, as PURR writes it

`NW = (1 + 6*nbin) * nunx = 121 * 83 = 10043` words; 83 URR energy points.
Per energy point, in order:

1. the energy \[eV\];
2. `nbin` **bin probabilities**;
3. `nbin` **total** values;
4. `nbin` **elastic** values;
5. `nbin` **fission** values;
6. `nbin` **capture** values;
7. `nbin` **heating** values (all zero here — PURR reported
   `no heating found on pendf`, since this deck runs no HEATR).

**U-238 has `LSSF=1`, so blocks 3-6 are stored as RATIOS to the
infinite-dilution cross sections, not as absolute barns** (`purr.f90:513-522`
divides by `sigu(i-1,1,1)` under `lssf.eq.1`). This is the same `LSSF=1`
subtlety recorded in `op-mzvp.2.12`: MF=3 already carries the infinitely
dilute unresolved cross sections, so the table supplies the *self-shielding
factor* and nothing else. A comparison that treats these as barns will be
wrong by orders of magnitude.

Every value is `sigfig(...,7,0)`-rounded by NJOY, so 7 significant figures is
the most any comparison against this file can resolve.
