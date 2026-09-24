# Provenance of the reference side

- **`run.py`** — the driver that builds the model and calls `openmc.run()` on a
  nuclide `.h5` written by this crate. Verbatim copy of the deck as run on
  2026-09-24.
- **`cs.xml`** — the `cross_sections.xml` pointing at that one nuclide.
- **`ace2hdf5.py`** — how the *comparison* library (`U234/U235/U238.h5`) was
  built: `openmc.data.IncidentNeutron.from_ace(...).export_to_hdf5(...)`, i.e.
  **upstream's own** ACE→HDF5 conversion, from the NJOY2016 ACE tables in
  `reference-data/ace` (themselves built from `reference-data/endf`
  ENDF/B-VIII.0 tapes). The conversion is the reference implementation's, not a
  reimplementation of it — which is what makes that library usable as an oracle
  for the format.
- **`LICENSE.openmc`** — OpenMC's licence, carried with the copied scripts.

OpenMC version: `0.1.dev1+gafa7a14ac` (commit `afa7a14`), built at
`/opt/src/openmc`. Note this is **not** the `608a1c33` the issues cite.
