#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
#
# Compare the Rust port's emitted plotting scripts against OpenMC's own output
# (crates/outram-mc-libs/verification_and_validation/python_plotting_parity/xs_and_tracks/).
#
#  1. Pixels: decodes reference/<case>.png and ours/<case>.png (RGBA) and
#     counts pixels that differ in any channel.
#  2. Data: loads each emitted script (from --scripts, default the directory
#     the Rust test writes to), calls its build_figure(), and compares every
#     plotted line's arrays with the arrays OpenMC's plot_xs handed to
#     matplotlib (<work>/lines/<case>.npz from reference_driver.py) or, for
#     tracks, with openmc.Tracks(<work>/tracks.h5): count of bit-identical
#     lines, max |abs| and max relative difference.
#  3. Negative control: re-renders xs_u235_basic with one value scaled by 1.5
#     and reports its differing pixels, which must be > 0.
#
# Usage (repository root):
#   MPLBACKEND=Agg /opt/ompy312/bin/python .../xs_and_tracks/compare.py \
#       --work <reference_driver --work dir> [--scripts DIR]

import argparse
import runpy
import sys
from pathlib import Path

import matplotlib
matplotlib.use('Agg')
import matplotlib.image as mpimg  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402

VV = Path(__file__).resolve().parent
CASES = ['xs_u235_basic', 'xs_u235_scatter_types', 'xs_u234_partial_fission',
         'xs_c12_types', 'xs_c12_heating', 'xs_two_nuclides_mev',
         'xs_u235_divisor', 'xs_u235_divided_by_unity', 'xs_material_heu',
         'xs_material_graphite_divisor', 'xs_element_u', 'tracks_all',
         'track_first']


def pixel_diff(a, b):
    ia, ib = mpimg.imread(a), mpimg.imread(b)
    if ia.shape != ib.shape:
        return None
    return int(np.count_nonzero(np.any(ia != ib, axis=-1)))


def diffs(a, b):
    a = np.asarray(a, dtype=float)
    b = np.asarray(b, dtype=float)
    if a.shape != b.shape:
        return None
    same = a.view('<u8').tobytes() == b.view('<u8').tobytes()
    d = np.abs(a - b)
    with np.errstate(divide='ignore', invalid='ignore'):
        rel = np.where(b != 0, d / np.abs(b), np.where(d == 0, 0.0, np.inf))
    return same, float(np.nanmax(d)) if d.size else 0.0, float(np.nanmax(rel)) if rel.size else 0.0


def reference_lines(case, work):
    if case.startswith('track'):
        import openmc
        tr = openmc.Tracks(str(work / 'tracks.h5'))
        ax = tr.plot() if case == 'tracks_all' else tr[0].plot()
        out = [tuple(np.asarray(v, float) for v in l.get_data_3d()) for l in ax.lines]
        plt.close(ax.figure)
        return out
    z = np.load(work / 'lines' / f'{case}.npz')
    n = len([k for k in z.files if k.startswith('x')])
    return [(z[f'x{i}'], z[f'y{i}']) for i in range(n)]


def our_lines(script):
    ns = runpy.run_path(str(script), run_name='outram_compare')
    fig = ns['build_figure']()
    ax = fig.axes[0]
    if hasattr(ax, 'get_zlim'):
        out = [tuple(np.asarray(v, float) for v in l.get_data_3d()) for l in ax.lines]
    else:
        out = [(np.asarray(l.get_xdata(), float), np.asarray(l.get_ydata(), float))
               for l in ax.lines]
    return fig, out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--work', required=True, type=Path)
    ap.add_argument('--scripts', type=Path,
                    default=Path('/tmp/outram_xs_and_tracks_plot_parity'))
    args = ap.parse_args()
    total_px = 0
    n_lines = n_same = 0
    worst_abs = worst_rel = 0.0
    print(f"{'case':32} {'pixels':>7} {'lines':>5} {'bit-identical':>13} "
          f"{'max|abs|':>10} {'max rel':>10}")
    for case in CASES:
        px = pixel_diff(VV / 'ours' / f'{case}.png', VV / 'reference' / f'{case}.png')
        script = args.scripts / f'{case}.py'
        if not script.exists():
            script = VV / 'ours' / f'{case}.py'
        fig, ours = our_lines(script)
        plt.close(fig)
        ref = reference_lines(case, args.work)
        assert len(ours) == len(ref), (case, len(ours), len(ref))
        same_c = 0
        a_c = r_c = 0.0
        for lo, lr in zip(ours, ref):
            for vo, vr in zip(lo, lr):
                s, a, r = diffs(vo, vr)
                a_c, r_c = max(a_c, a), max(r_c, r)
            same_c += all(diffs(vo, vr)[0] for vo, vr in zip(lo, lr))
        n_lines += len(ours)
        n_same += same_c
        worst_abs, worst_rel = max(worst_abs, a_c), max(worst_rel, r_c)
        total_px += px if px is not None else 10**9
        print(f'{case:32} {px!s:>7} {len(ours):5} {same_c:13} {a_c:10.3g} {r_c:10.3g}')
    print(f'TOTAL differing pixels: {total_px}; lines bit-identical: {n_same}/{n_lines}; '
          f'max|abs| {worst_abs:.3g}, max rel {worst_rel:.3g}')

    # Negative control: perturb one plotted value of xs_u235_basic.
    script = args.scripts / 'xs_u235_basic.py'
    ns = runpy.run_path(str(script), run_name='outram_compare')
    fig = ns['build_figure']()
    line = fig.axes[0].lines[0]
    y = np.array(line.get_ydata(), dtype=float)
    x = np.asarray(line.get_xdata(), dtype=float)
    i = int(np.argmax(x >= 1.0))
    y[i] *= 1.5
    line.set_ydata(y)
    out = args.work / 'negative_control.png'
    fig.savefig(out)
    plt.close(fig)
    neg = pixel_diff(out, VV / 'reference' / 'xs_u235_basic.png')
    print(f'NEGATIVE CONTROL (U235 total at E={x[i]:g} eV x1.5): {neg} differing pixels')
    ok = total_px == 0 and n_same == n_lines and neg and neg > 0
    sys.exit(0 if ok else 1)


if __name__ == '__main__':
    main()
