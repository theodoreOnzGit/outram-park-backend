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
