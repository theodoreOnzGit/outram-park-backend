#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 OUTRAM PARK contributors.
"""Read the port's collision-track and MCPL files back with the reference
readers (gh:#1392).

Usage:
    OUTRAM_COLLISION_TRACK_OUT=<dir> cargo test --release -p outram-mc-libs \
        --test collision_track_output
    python read_back.py <dir> <OpenMC clone>

Readers: `openmc.read_collision_track_hdf5` / `read_collision_track_mcpl`
(OpenMC 8f79d9d0, installed `pip install --no-deps -e <clone>`), h5py, and
the `mcpl` Python package 2.2.8 (mcpl-python). No OpenMC binary is run.
Every check raises on failure; the printout is the record.
"""
import gzip
import sys
from pathlib import Path

import h5py
import mcpl
import numpy as np
import openmc

out = Path(sys.argv[1])
clone = Path(sys.argv[2])
reg = clone / "tests" / "regression_tests"

# 1. HDF5: OpenMC's own reader, against upstream's fixture's dtype and the
#    port's records (bit for bit).
ours = openmc.read_collision_track_hdf5(out / "collision_track.h5")
theirs = openmc.read_collision_track_hdf5(
    reg / "collision_track" / "case_1_Reactions" / "collision_track_true.h5")
assert ours.dtype == theirs.dtype, (ours.dtype, theirs.dtype)
assert ours.dtype.itemsize == 128
with h5py.File(out / "collision_track.h5", "r") as f:
    assert f.attrs["filetype"] == b"collision_track", f.attrs["filetype"]
    assert list(f.attrs["version"]) == [1, 2], f.attrs["version"]
    version_dtype = f.attrs["version"].dtype
rows = [l.split(",") for l in (out / "collision_track_sites.csv").read_text().split()]
assert len(rows) == len(ours) == 200
fields = ["r", "u", "E", "dE", "time", "wgt", "event_mt", "delayed_group",
          "cell_id", "nuclide_id", "material_id", "universe_id",
          "n_collision", "particle", "parent_id", "progeny_id"]
n_values = 0
for rec, row in zip(ours, rows):
    vals = [float(x) for x in row[:10]] + [int(x) for x in row[10:]]
    got = list(rec["r"]) + list(rec["u"]) + [rec[k] for k in fields[2:]]
    for a, b in zip(got, vals):
        if isinstance(b, float):
            assert np.float64(a).tobytes() == np.float64(b).tobytes(), (a, b)
        else:
            assert int(a) == b, (a, b)
        n_values += 1
mts, counts = np.unique(ours["event_mt"], return_counts=True)
print(f"HDF5: openmc.read_collision_track_hdf5 read {len(ours)} records, "
      f"dtype identical to upstream's fixture ({ours.dtype.itemsize} bytes), "
      f"{n_values} values bit-identical to the port's; version attr dtype "
      f"{version_dtype}; MTs {dict(zip(mts.tolist(), counts.tolist()))}")

# 2. MCPL collision track: OpenMC's MCPL reader (which uses the mcpl package).
m = openmc.read_collision_track_mcpl(str(out / "collision_track.mcpl"))
assert len(m) == 200
assert (m["event_mt"].astype(int) == ours["event_mt"]).all()
for k in ["delayed_group", "cell_id", "nuclide_id", "material_id",
          "universe_id", "n_collision", "particle", "parent_id", "progeny_id"]:
    assert (m[k] == ours[k]).all(), k
for k in ["x", "y", "z"]:
    assert (m["r"][k] == ours["r"][k].astype(np.float32)).all(), k
# MCPL holds E and time as f32 (MeV, ms); the reader then multiplies the f32
# by 1e6 / 1e-3 and, under NumPy 2's promotion rules, rounds to f32 again:
# two f32 roundings, so 2 f32 ulps (2.4e-7) relative.
F32_2ULP = 2.0 ** -22
assert np.allclose(m["E"], ours["E"], rtol=F32_2ULP, atol=0)
assert np.allclose(m["time"], ours["time"], rtol=F32_2ULP, atol=0)
assert np.allclose(m["dE"], ours["dE"], rtol=1e-5, atol=0)  # 6 figures in the blob
print("MCPL collision track: openmc.read_collision_track_mcpl read 200 records; "
      "ids, MTs and counts equal, positions equal at f32, E/time to f32, "
      "dE to the blob's 6 figures")

# 3. MCPL particle files: the mcpl package on the port's re-encoding of
#    upstream's fixture (plain and gzipped) and on a double-precision file.
def arr(path):
    with mcpl.MCPLFile(str(path)) as f:
        hdr = (f.nparticles, f.sourcename, f.comments, f.opt_singleprec)
        rows = [[*p.position, *p.direction, p.ekin, p.time, p.weight, p.pdgcode]
                for p in f.particles]
    return hdr, np.array(rows)

ref_hdr, ref = arr(reg / "surface_source" / "surface_source_true.mcpl")
for name in ["surface_source_port.mcpl", "surface_source_port.mcpl.gz"]:
    hdr, a = arr(out / name)
    assert hdr == ref_hdr, (hdr, ref_hdr)
    assert np.array_equal(a, ref), name
raw = gzip.decompress((out / "surface_source_port.mcpl.gz").read_bytes())
assert raw == (reg / "surface_source" / "surface_source_true.mcpl").read_bytes()
hdr, a = arr(out / "source_point_double.mcpl")
assert hdr[0] == 1000 and not hdr[3], hdr
assert np.array_equal(a[:, :3], ref[:, :3])  # f32 positions widened exactly
assert hdr[2] == ["stat:sum:openmc_np1:                   10000"], hdr[2]
print(f"MCPL files: mcpl {mcpl.__version__} reads the port's re-encoding of "
      f"upstream's fixture (plain and gzip) identical to the fixture "
      f"({ref_hdr[0]} particles), the gzip inflates to the fixture's bytes, "
      f"and a double-precision source-point file reads with stat:sum 10000")
print("ALL CHECKS PASSED")
