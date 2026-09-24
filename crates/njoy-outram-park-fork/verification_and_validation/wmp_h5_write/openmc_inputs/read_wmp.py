#!/usr/bin/env python3
"""Read a `WMP_Library`-format HDF5 file **with OpenMC's own reader** and print
its evaluated cross sections — the reference side of the cross-code check for
this workspace's WMP writer (gh:#270).

Consumed by `crates/njoy-outram-park-fork/tests/wmp_h5_vs_openmc.rs`, which
writes the file, runs this, and compares against its own evaluation.

Needs no cross-section library: `WindowedMultipole.from_hdf5` plus scipy's
Faddeeva is the whole dependency, which is why this cross-code comparison can
run where a continuous-energy one cannot.

Floats are printed with `repr`, which is shortest-round-trip in Python and
parses back to the identical f64 in Rust, so nothing is lost between the two
sides. The structural facts are printed first as `meta` lines so the Rust side
can assert on the dtype and shapes it intended to write, not merely on numbers.

Usage:  python3 read_wmp.py <file.h5> <nuclide> <T1,T2,...> <E1,E2,...>

Output (one record per line, `|`-separated):

    meta|name|<name>
    meta|dtype|<data dtype>          # must be complex128 or OpenMC cannot use it
    meta|shape|<n_poles>|<n_cols>
    meta|windows|<n_windows>
    meta|fit_order|<order>
    meta|scalar_ndim|<ndim of spacing>   # must be 0, not 1
    meta|<spacing|sqrtAWR|E_min|E_max>|<repr>
    xs|<repr T>|<repr E>|<repr scatter>|<repr absorption>|<repr fission>

Provenance: OpenMC is MIT-licensed. Run against /opt/src/openmc commit afa7a14
on 2026-09-24; results in ../wmp_h5_write_2026_09_24.md.
"""
import sys

import h5py
import openmc.data


def main(path, name, temps, energies):
    # Structural facts, read with plain h5py so they are not filtered through
    # OpenMC's own interpretation.
    with h5py.File(path, "r") as f:
        g = f[name]
        print(f"meta|dtype|{g['data'].dtype}")
        print(f"meta|shape|{g['data'].shape[0]}|{g['data'].shape[1]}")
        print(f"meta|windows|{g['windows'].shape[0]}")
        print(f"meta|scalar_ndim|{g['spacing'].ndim}")
        for k in ("spacing", "sqrtAWR", "E_min", "E_max"):
            # float() first: numpy 2.x reprs a scalar as "np.float64(x)".
            print(f"meta|{k}|{float(g[k][()])!r}")

    m = openmc.data.WindowedMultipole.from_hdf5(path)
    print(f"meta|name|{m.name}")
    print(f"meta|fit_order|{m.fit_order}")

    for t in temps:
        for e in energies:
            s, a, fi = m(e, t)
            print(f"xs|{t!r}|{e!r}|{float(s)!r}|{float(a)!r}|{float(fi)!r}")


if __name__ == "__main__":
    p, nuc = sys.argv[1], sys.argv[2]
    ts = [float(x) for x in sys.argv[3].split(",")]
    es = [float(x) for x in sys.argv[4].split(",")]
    main(p, nuc, ts, es)
