# Plotting parity: `openmc.plot_xs`, `openmc.Track.plot`, `openmc.Tracks.plot`

Code-to-code verification of `outram_mc_libs::plotter` (the Rust port of
OpenMC's non-geometry Python plotting functions) against OpenMC itself.
Recorded 2026-09-26.

**Result: 13 of 13 figures pixel-identical (0 differing pixels) and 53 of 53
plotted lines bit-identical, on the first run, with nothing adjusted.**

## What is being verified

The port does not draw anything itself. It computes in Rust the exact arrays
that OpenMC's function hands to matplotlib, records the axis calls it makes,
and emits a standalone Python script (numpy and matplotlib only, data
embedded as zlib+base64 little-endian float64) that makes the same calls.
Run with the same matplotlib, that script should write the PNG OpenMC writes.

| Rust | OpenMC (commit `d7d3284a1`, 0.16.1.dev25) |
|---|---|
| `plotter::xs::plot_xs`, `plotter::xs::calculate_cexs` | `openmc.plot_xs`, `calculate_cexs`, `_calculate_cexs_nuclide`, `_calculate_cexs_elem_mat`, `_get_legend_label`, `_get_yaxis_label`, `_get_title` (`openmc/plotter.py:1-677`) |
| `plotter::incident_neutron::IncidentNeutronData::from_ace` | `IncidentNeutron.from_ace` + `export_to_hdf5` + `from_hdf5`, `Reaction.from_ace`, `_get_fission_products_ace` |
| `plotter::tracks::tracks_plot_script`, `track_plot_script` | `openmc.Tracks.plot` (`openmc/tracks.py:255-275`), `openmc.Track.plot` (`:151-180`) |

## Methodology

**Same data on both sides.** Both codes read the same ACE files:

| nuclide | file | generator | sha256 (uncompressed) |
|---|---|---|---|
| U-234 | `reference-data/ace/reference-njoy/endf-b-viii.0/293.6K/U234.ace.gz` | NJOY2016 2016.79 `ac5adf5`, ENDF/B-VIII.0, RECONR+BROADR+PURR+ACER | `6dfcf9e227068f4b1647662ac234353b9a1ed982b7a8ecbcd303ce67ca94e6e9` |
| U-235 | `.../293.6K/U235.ace.gz` | same | `42992a06809e6a160f5aa64d80b226594acbb36cb25057ca5a56da1fc8b41fe8` |
| U-238 | `.../293.6K/U238.ace.gz` | same | `36ae9f1c0ee107fb48d0e208e15db4f024de0c83f5c0009867f03438cd52d1c1` |
| C-12 | `data/C12.ace.gz` (committed here) | this workspace's `njoy-outram-park-fork` v0.0.3 `6b71d21+` (`NuclearDataLibrary`: RECONR, BROADR to 293.6 K, HEATR heating, ACER; ENDF/B-VIII.0 `n-006_C_012`, MAT 625), header stamp `ZA=6012 njoy-outram-park-fork v0.0.3 6b71d21+ 2026-09-26` | `80f071f0b8d972d44022c53c026b2b99a3ab8964590975fd8c21ad294c6924b1` |

C-12 comes from the workspace's own NJOY port because no NJOY2016 table for a
light nuclide exists in the reference set; that does not enter the
comparison, since both sides read the same bytes. It is regenerated
deterministically by the ignored test `regenerate_c12_ace` (two runs gave
byte-identical output) and then `gzip -9 -n`.

- **OpenMC side** (`openmc_inputs/reference_driver.py`, inlined below):
  `IncidentNeutron.from_ace(ace)` then `export_to_hdf5`, a
  `cross_sections.xml` over the four `.h5` files, then `openmc.plot_xs(...,
  ce_cross_sections=xml)` and `fig.savefig(png)`. The HDF5 round trip is part
  of the reference on purpose: it is the path `plot_xs` really reads, and it
  changes the data (some redundant reactions are dropped, the total fission
  yield is re-attached to every fission MT). For tracks, the driver writes an
  OpenMC version-3 track file from `data/tracks_input.tsv` and calls
  `openmc.Tracks(file).plot()` and `tracks[0].plot()`.
- **Rust side** (`crates/outram-mc-libs/tests/xs_and_tracks_plot_parity.rs`):
  `IncidentNeutronData::from_ace_file` on the same files, the same eleven
  `plot_xs` cases, the same tracks, emitted scripts run with
  `MPLBACKEND=Agg`.
- **Tracks** come from this crate's own transport: a traced fixed-source run
  (`run_fixed_source_traced`, `TrackRecorder`), 2 MeV point source at the
  centre of a 15 cm C-12 sphere (8.5238e-2 atom/b-cm), 8 histories, seed
  20260926. They go through `PlotTrack::from(&physics::track_output::Track)`;
  the first `PlotTrack` carries the first two histories as two particle tracks
  so that `Track.plot` draws more than one line. Written once to
  `data/tracks_input.tsv` so that later changes to transport cannot move the
  comparison.
- **Same renderer**: `/opt/ompy312/bin/python` (Python 3.12) with matplotlib
  3.11.2, numpy 2.5.3, h5py 3.16.0, Pillow 12.3.0, endf 0.1.12, OpenMC
  0.16.1.dev25+gd7d3284a1, Agg backend, default rcParams, default dpi 100.

**Pass criteria, fixed before the first run:**

1. **0 differing pixels** between `ours/<case>.png` and `reference/<case>.png`
   (RGBA, any channel).
2. **Bit identity of the data**: every plotted line's x and y arrays equal
   OpenMC's, checked two ways: FNV-1a 64-bit hashes of the little-endian
   float64 bytes against `reference/line_fingerprints.tsv` (Rust test, no
   Python needed), and element-wise max absolute and relative difference
   (`compare.py`).

If the data had differed at round-off the plan was to report the numbers and
read upstream first, not to tune. It did not arise.

**Negative control.** One plotted value (U-235 total at the first grid point
at or above 1 eV) is scaled by 1.5 and re-rendered; this must produce a
nonzero pixel difference and a different fingerprint.

## The cases

| case | call | what it exercises |
|---|---|---|
| `xs_u235_basic` | `{'U235': ['total','elastic','fission','capture','absorption','nu-fission']}` | sum rules, total fission yield from the NU block |
| `xs_u235_scatter_types` | `{'U235': ['scatter','inelastic','nu-scatter',16,'(n,2n)','damage','unity','slowing-down power']}` | MT ints, `REACTION_MT` names, `DADZ` legend (`U235 (n,2n) U234`), energy-dependent yield (MT=5, TYR=-101), the two sentinel types |
| `xs_u234_partial_fission` | `{'U234': ['fission','nu-fission','total',19]}`, `energy_axis_units='keV'` | no MT=18 in the table: MT=19/20/21/38, derived total nu on each partial |
| `xs_c12_types` | `{'C12': ['total','elastic','inelastic','capture',51,'(n,a)','(n,p)','nu-scatter']}` | light nuclide, levels, `(n,a) Be9`, `(n,p) B12` |
| `xs_c12_heating` | `{'C12': ['heating']}` | MT=301 = heating number x total, "Heating Cross Section [eV-barn]" |
| `xs_two_nuclides_mev` | `{'U235': ['fission'], 'U238': ['fission','capture']}`, MeV, `figsize=(8, 5)` | two keys, title without a name, subplot kwargs |
| `xs_u235_divisor` | `{'U235': ['nu-fission','fission','elastic']}` / `['fission','absorption','total']` | `np.union1d`, `np.interp`, suffixed labels, "Microscopic Data " |
| `xs_u235_divided_by_unity` | `{'U235': ['capture']}` / `['unity']` | division by a nuclide's `'unity'` (see below) |
| `xs_material_heu` | Material 1 "HEU metal", U-234/235/238 at 4.9184e-4 / 4.4994e-2 / 2.4984e-3 atom/b-cm, `'sum'` | `get_nuclide_atom_densities`, union grid, material `'unity'` = 1 |
| `xs_material_graphite_divisor` | unnamed Material 2, C-12 at 8.5238e-2 atom/b-cm, `temperature=600`; `['elastic','total']` / `['total','unity']` | "Material 2 ..." labels, material temperature override, "Macroscopic Data " |
| `xs_element_u` | `{'U': ['total','fission','(n,gamma)']}` | `Element.expand` against the library (U-234/235/238 all present) |
| `tracks_all` | `openmc.Tracks(file).plot()` | 7 source particles, 8 particle tracks |
| `track_first` | `tracks[0].plot()` | one source particle with two particle tracks |

## Results

`compare.py`, 2026-09-26:

```text
case                              pixels lines bit-identical   max|abs|    max rel
xs_u235_basic                          0     6             6          0          0
xs_u235_scatter_types                  0     6             6          0          0
xs_u234_partial_fission                0     4             4          0          0
xs_c12_types                           0     8             8          0          0
xs_c12_heating                         0     1             1          0          0
xs_two_nuclides_mev                    0     3             3          0          0
xs_u235_divisor                        0     3             3          0          0
xs_u235_divided_by_unity               0     1             1          0          0
xs_material_heu                        0     6             6          0          0
xs_material_graphite_divisor           0     2             2          0          0
xs_element_u                           0     3             3          0          0
tracks_all                             0     8             8          0          0
track_first                            0     2             2          0          0
TOTAL differing pixels: 0; lines bit-identical: 53/53; max|abs| 0, max rel 0
NEGATIVE CONTROL (U235 total at E=1 eV x1.5): 79 differing pixels
```

The Rust test reports the same: 53 fingerprints matched, 0 differing pixels
on all 13 PNGs (decoded with `outram_mc_libs::geometry::plot::decode_png`),
negative control 79 differing pixels. Grids plotted: U-235 76 027 points,
U-238 155 207, U-234 25 393, C-12 1 616, U-234/235/238 union 254 933.

**How sensitive the gate is.** As an ablation, the `np.isclose` end-point
snap in `Tabulated1D.__call__` (`function.py:205-208`; values within a
relative 1e-5 of the first or last abscissa are set to the end ordinate) was
switched off in the port: the bit-identity check (criterion 2) then fails on the first line checked
(U-235 total), because the last grid point of every reaction is otherwise
evaluated as outside the table and comes out 0. The gate sees
one-value-in-76 000 differences.

**Interpretation.** For these inputs the port reproduces OpenMC's plotting
pipeline exactly: the reaction set that survives the HDF5 round trip, the sum
order of partial cross sections (`SUM_RULES` order), the yield products, the
interpolation including upstream's edge rules, NumPy's `interp`, `union1d`
and pairwise `sum`, and every label and axis call. This is verification of a
plotting port, and says nothing about the data being plotted.

## Upstream behaviours reproduced, not fixed

Confirmed by the reference images; the port keeps them because its contract
is "what OpenMC draws":

1. **`'unity'` and `'slowing-down power'` do not work for a nuclide.** Their
   sentinel MTs (`UNITY_MT = -1`, `XI_MT = -2`) go through
   `get_reaction_components`, which returns `[]` for an MT the nuclide lacks,
   so the branches that would return 1 or `xi` are unreachable. A nuclide's
   `'unity'` is 0 and is not plotted; its `'slowing-down power'` is elastic
   alone (`xs_u235_scatter_types`: the brown line lies on "scatter"'s). For a
   material, `'unity'` is special-cased to 1 and works (`xs_material_heu`).
2. **Dividing by a nuclide's `'unity'` divides by zero** (`xs_u235_divided_by_unity`):
   `capture / 0` becomes `inf` and then `1.797e308` via `np.nan_to_num`; the
   line is "plotted" but lands off the axes.
3. **The linear y-axis branch is unreachable with ordinary arguments.**
   `all_types` is copied before the `' / divisor'` suffix is appended, so it
   never matches `PLOT_TYPES_LINEAR`; `xs_u235_divisor` comes out log-log
   (reference driver log: `yscale=log`).
4. `'damage'` (MT=444) is absent from these tables (no HEATR damage in the
   NJOY2016 deck), so it is 0 and not plotted.

## Not ported, and why

- **Multigroup `plot_xs` (`plot_CE=False`, `calculate_mgxs`,
  `_calculate_mgxs_nuc_macro`, `_calculate_mgxs_elem_mat`).** They read an
  `openmc.MGXSLibrary`: group edges, per-temperature `XSdata`, angle
  representations, delayed-group shapes. This crate's
  `physics::physics_mg::MgxsLibrary` has no group edges or temperatures and
  none of kappa-fission, inverse-velocity, beta, decay-rate,
  chi-prompt/delayed; the workspace has an `mgxs.h5` writer but no reader.
  Returns `PlotError::NotPorted`.
- **S(a,b)** (`sab_name`, a material's `add_s_alpha_beta`): needs
  `ThermalScattering` (coherent-elastic Bragg edges, `Regions1D`). No
  thermal-scattering ACE table exists in the shared reference data for both
  sides to read, so a port could not be verified here. Returns `NotPorted`
  rather than a plot silently missing the thermal data.
- **NCrystal** (`ncrystal_cfg`): external C++ library.
- **Enrichment, weight percents and mass densities**: need
  `openmc.data.atomic_mass` (AME2020 table), which is not in the workspace.
  `'sum'`, `'atom/b-cm'`, `'atom/cm3'` and `'macro'` with `'ao'` are ported.
- **Several temperatures of one nuclide in one library**: an
  `IncidentNeutronData` holds one ACE table, so upstream's nearest-temperature
  choice is always that table.
- **`openmc/data/multipole.py:360-383`** is not a plotting API: it is a
  diagnostic inside `_vectfit_xs` that saves ACE-vs-fit PNGs as a side effect
  of windowed-multipole fitting when `path_out` is set. It returns no figure,
  draws data that exist only inside the fit, and cannot be called on its own;
  it belongs with a port of the WMP generator, not with the plotters.

## Files

| path | what |
|---|---|
| `openmc_inputs/reference_driver.py` | the OpenMC side (below) |
| `reference/*.png`, `reference/line_fingerprints.tsv` | OpenMC's output |
| `ours/*.png` | the emitted scripts' output |
| `ours/*.py` | emitted scripts of at most 1 MB (C-12, graphite, tracks, divided-by-unity). The U-234/235/238 scripts are 1.2-13 MB (the ACE grids are embedded) and are not committed: the Rust test regenerates them into `$TMPDIR/outram_xs_and_tracks_plot_parity/` on every run and asserts they are deterministic |
| `data/C12.ace.gz`, `data/tracks_input.tsv` | inputs both sides read |
| `compare.py` | pixel and array comparison, negative control |

## Reproduce

```bash
# OpenMC side (about 1 min)
MPLBACKEND=Agg /opt/ompy312/bin/python \
  crates/outram-mc-libs/verification_and_validation/python_plotting_parity/xs_and_tracks/openmc_inputs/reference_driver.py \
  --work /tmp/xs_ref
# Rust side, both gates and the negative control
OUTRAM_PYTHON=/opt/ompy312/bin/python \
  cargo test --release -p outram-mc-libs --test xs_and_tracks_plot_parity
# array-level comparison
MPLBACKEND=Agg /opt/ompy312/bin/python \
  crates/outram-mc-libs/verification_and_validation/python_plotting_parity/xs_and_tracks/compare.py \
  --work /tmp/xs_ref
```

`OUTRAM_WRITE_VV=1` on the cargo command rewrites `ours/` and
`data/tracks_input.tsv`.

## Reference driver (verbatim)

```python
#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
#
# Reference driver for the xs-and-tracks plotting parity check
# (crates/outram-mc-libs/verification_and_validation/python_plotting_parity/xs_and_tracks/).
#
# Provenance: calls OpenMC's own Python plotting functions, OpenMC commit
# d7d3284a1 (0.16.1.dev25+gd7d3284a1), as installed at /opt/ompy312 with
# matplotlib 3.11.2, numpy 2.5.3, h5py 3.16.0, endf 0.1.12. OpenMC is
# MIT-licensed (crates/outram-mc-libs/LICENSE.openmc). The functions exercised:
#   openmc.plot_xs            openmc/plotter.py:125-290
#   openmc.Track.plot         openmc/tracks.py:151-180
#   openmc.Tracks.plot        openmc/tracks.py:255-275
#
# Data route (the same ACE files the Rust side reads):
#   ACE --openmc.data.IncidentNeutron.from_ace--> export_to_hdf5 --> cross_sections.xml
#   and plot_xs(ce_cross_sections=that xml).
#
# Usage (from the repository root):
#   MPLBACKEND=Agg /opt/ompy312/bin/python \
#     crates/outram-mc-libs/verification_and_validation/python_plotting_parity/xs_and_tracks/openmc_inputs/reference_driver.py \
#     --work /some/scratch/dir
# Writes reference/<case>.png and reference/line_fingerprints.tsv next to this
# folder, and <work>/lines/<case>.npz (the exact arrays each ax.plot received;
# too large to commit, used by compare.py).

import argparse
import gzip
import shutil
from pathlib import Path

import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
import h5py  # noqa: E402
import openmc  # noqa: E402
import openmc.data  # noqa: E402

HERE = Path(__file__).resolve().parent
VV = HERE.parent
REPO = VV.parents[4]
ACE_SOURCES = {
    'U234': REPO / 'reference-data/ace/reference-njoy/endf-b-viii.0/293.6K/U234.ace.gz',
    'U235': REPO / 'reference-data/ace/reference-njoy/endf-b-viii.0/293.6K/U235.ace.gz',
    'U238': REPO / 'reference-data/ace/reference-njoy/endf-b-viii.0/293.6K/U238.ace.gz',
    'C12': VV / 'data/C12.ace.gz',
}


def fnv1a64(arr):
    """FNV-1a 64 of the little-endian float64 bytes (same as the Rust side)."""
    h = 0xcbf29ce484222325
    for b in np.ascontiguousarray(arr, dtype='<f8').tobytes():
        h ^= b
        h = (h * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    return h


def build_library(work):
    lib_dir = work / 'library'
    lib_dir.mkdir(parents=True, exist_ok=True)
    library = openmc.data.DataLibrary()
    for name, gz in ACE_SOURCES.items():
        ace = lib_dir / f'{name}.ace'
        h5 = lib_dir / f'{name}.h5'
        if not h5.exists():
            with gzip.open(gz, 'rb') as fi, open(ace, 'wb') as fo:
                shutil.copyfileobj(fi, fo)
            nuc = openmc.data.IncidentNeutron.from_ace(str(ace))
            assert nuc.name == name, (nuc.name, name)
            nuc.export_to_hdf5(str(h5), 'w')
            ace.unlink()
        library.register_file(str(h5))
    xml = lib_dir / 'cross_sections.xml'
    library.export_to_xml(str(xml))
    return str(xml)


def materials():
    # Godiva-like HEU metal, ICSBEP HEU-MET-FAST-001 number densities [atom/b-cm].
    heu = openmc.Material(material_id=1, name='HEU metal')
    heu.add_nuclide('U234', 4.9184e-4)
    heu.add_nuclide('U235', 4.4994e-2)
    heu.add_nuclide('U238', 2.4984e-3)
    heu.set_density('sum')
    # Unnamed graphite, 1.7 g/cm3 expressed as 8.5238e-2 atom/b-cm of C-12,
    # with a material temperature that overrides plot_xs' temperature.
    gr = openmc.Material(material_id=2)
    gr.add_nuclide('C12', 1.0)
    gr.set_density('atom/b-cm', 8.5238e-2)
    gr.temperature = 600.0
    return heu, gr


def cases():
    heu, gr = materials()
    return [
        ('xs_u235_basic',
         dict(reactions={'U235': ['total', 'elastic', 'fission', 'capture',
                                  'absorption', 'nu-fission']})),
        ('xs_u235_scatter_types',
         dict(reactions={'U235': ['scatter', 'inelastic', 'nu-scatter', 16,
                                  '(n,2n)', 'damage', 'unity',
                                  'slowing-down power']})),
        ('xs_u234_partial_fission',
         dict(reactions={'U234': ['fission', 'nu-fission', 'total', 19]},
              energy_axis_units='keV')),
        ('xs_c12_types',
         dict(reactions={'C12': ['total', 'elastic', 'inelastic', 'capture',
                                 51, '(n,a)', '(n,p)', 'nu-scatter']})),
        ('xs_c12_heating',
         dict(reactions={'C12': ['heating']})),
        ('xs_two_nuclides_mev',
         dict(reactions={'U235': ['fission'], 'U238': ['fission', 'capture']},
              energy_axis_units='MeV', figsize=(8.0, 5.0))),
        ('xs_u235_divisor',
         dict(reactions={'U235': ['nu-fission', 'fission', 'elastic']},
              divisor_types=['fission', 'absorption', 'total'])),
        ('xs_u235_divided_by_unity',
         dict(reactions={'U235': ['capture']}, divisor_types=['unity'])),
        ('xs_material_heu',
         dict(reactions={heu: ['total', 'elastic', 'fission', 'nu-fission',
                               'capture', 'unity']})),
        ('xs_material_graphite_divisor',
         dict(reactions={gr: ['elastic', 'total']},
              divisor_types=['total', 'unity'])),
        ('xs_element_u',
         dict(reactions={'U': ['total', 'fission', '(n,gamma)']})),
    ]


def run_xs(xml, out_png_dir, lines_dir, fp_rows):
    for name, kw in cases():
        kw = dict(kw)
        reactions = kw.pop('reactions')
        # plot_xs mutates the type lists in place when dividing; pass copies
        # so the case table stays reusable.
        reactions = {k: list(v) for k, v in reactions.items()}
        fig = openmc.plot_xs(reactions, ce_cross_sections=xml, **kw)
        ax = fig.axes[0]
        fig.savefig(out_png_dir / f'{name}.png')
        arrays = {}
        for i, line in enumerate(ax.lines):
            x = np.asarray(line.get_xdata(), dtype=float)
            y = np.asarray(line.get_ydata(), dtype=float)
            arrays[f'x{i}'] = x
            arrays[f'y{i}'] = y
            fp_rows.append((name, i, line.get_label(), x.size,
                            f'{fnv1a64(x):016x}', f'{fnv1a64(y):016x}'))
        np.savez_compressed(lines_dir / f'{name}.npz', **arrays)
        print(f'{name}: {len(ax.lines)} lines, yscale={ax.get_yscale()}, '
              f'ylabel={ax.get_ylabel()!r}')
        plt.close(fig)


def read_tracks(tsv):
    tracks = {}
    for row in open(tsv):
        if row.startswith('#') or not row.strip():
            continue
        t, p, x, y, z = row.split('\t')
        tracks.setdefault(int(t), {}).setdefault(int(p), []).append(
            (float(x), float(y), float(z)))
    return [[tracks[t][p] for p in sorted(tracks[t])] for t in sorted(tracks)]


def write_track_file(tracks, path):
    """An OpenMC track file (version 3), one dataset per source particle."""
    pos = np.dtype([('x', '<f8'), ('y', '<f8'), ('z', '<f8')])
    state = np.dtype([('r', pos), ('u', pos), ('E', '<f8'), ('time', '<f8'),
                      ('wgt', '<f8'), ('cell_id', '<i4'),
                      ('cell_instance', '<i4'), ('material_id', '<i4')])
    with h5py.File(path, 'w') as fh:
        fh.attrs['filetype'] = np.bytes_('track')
        fh.attrs['version'] = np.array([3, 0])
        for i, particle_tracks in enumerate(tracks, start=1):
            rows = [pt for part in particle_tracks for pt in part]
            arr = np.zeros(len(rows), dtype=state)
            for k, (x, y, z) in enumerate(rows):
                arr[k]['r'] = (x, y, z)
            offsets = np.cumsum([0] + [len(p) for p in particle_tracks])
            d = fh.create_dataset(f'track_1_1_{i}', data=arr)
            d.attrs['n_particles'] = len(particle_tracks)
            d.attrs['offsets'] = offsets
            d.attrs['particles'] = np.array([2112] * len(particle_tracks))


def run_tracks(work, out_png_dir, fp_rows):
    tracks = read_tracks(VV / 'data/tracks_input.tsv')
    h5 = work / 'tracks.h5'
    write_track_file(tracks, h5)
    tr = openmc.Tracks(str(h5))
    assert len(tr) == len(tracks)

    ax = tr.plot()
    ax.figure.savefig(out_png_dir / 'tracks_all.png')
    fp_lines(ax, 'tracks_all', fp_rows)
    plt.close(ax.figure)

    ax = tr[0].plot()
    ax.figure.savefig(out_png_dir / 'track_first.png')
    fp_lines(ax, 'track_first', fp_rows)
    plt.close(ax.figure)
    print(f'tracks: {len(tr)} source particles')


def fp_lines(ax, name, fp_rows):
    for i, line in enumerate(ax.lines):
        x, y, z = line.get_data_3d()
        xyz = np.concatenate([np.asarray(x, float), np.asarray(y, float),
                              np.asarray(z, float)])
        fp_rows.append((name, i, '', len(x), f'{fnv1a64(xyz):016x}', '-'))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--work', required=True, type=Path)
    ap.add_argument('--out', type=Path, default=VV / 'reference')
    args = ap.parse_args()
    args.work.mkdir(parents=True, exist_ok=True)
    lines_dir = args.work / 'lines'
    lines_dir.mkdir(exist_ok=True)
    args.out.mkdir(parents=True, exist_ok=True)
    xml = build_library(args.work)
    fp_rows = []
    run_xs(xml, args.out, lines_dir, fp_rows)
    run_tracks(args.work, args.out, fp_rows)
    with open(args.out / 'line_fingerprints.tsv', 'w') as f:
        f.write('# case\tline\tlabel\tn\tfnv1a64(x)\tfnv1a64(y)\n')
        for r in fp_rows:
            f.write('\t'.join(str(v) for v in r) + '\n')


if __name__ == '__main__':
    main()
```
