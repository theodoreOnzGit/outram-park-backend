#!/usr/bin/env python3
"""gh:#1459/#1467 round 3: extract the exact-double snapping references from ESI
v2506 flange runs written with `writeFormat binary` (so the doubles are the
internal ones, not 16-digit decimals).

usage: python3 -I extract_snap_fixtures.py RUN_N1 RUN_N2 RUN_N3 OUT_DIR

RUN_Nk is a run with `snapControls { nSmoothPatch k; ... }` (the rest of the
dictionary identical). Writes little-endian float64 triples (the polyMesh/points
and pointVectorField payload, no header), to be gzipped with `gzip -n`:

  snap_points_32.bin    castellated points (input of the snap stage)
  snap_points_33_n1.bin points after preSmoothPatch iteration 1 (run n1, time 33)
  snap_points_33_n2.bin ... after iteration 2
  snap_points_33_n3.bin ... after iteration 3 (the dictionary's nSmoothPatch 3)
  snap_disp_34.bin      `34/displacement`: patch displacement (surface attraction,
                        after outwardsDisplacement) on patch points, smoothed
                        displacement elsewhere (run n3)
  snap_points_35.bin    mesh after scaling iteration 0 (full displacement, run n3)
  snap_points_36.bin    snapped mesh before face merging (scaling 0.75, run n3)

Standard library only. Tooling for V&V, reached by the recorded run, not a test.
"""
import re
import struct
import sys


def vec_payload(path):
    data = open(path, "rb").read()
    # `N\n(` followed by N*24 raw bytes (internalField of a field starts the same way)
    m = re.search(rb"\n(\d+)\n\(", data)
    n = int(m.group(1))
    start = m.end()
    raw = data[start:start + 24 * n]
    assert len(raw) == 24 * n
    assert data[start + 24 * n:start + 24 * n + 1] == b")"
    return n, raw


def main():
    n1, n2, n3, out = sys.argv[1:5]
    jobs = [
        ("snap_points_32.bin", n3 + "/32/polyMesh/points"),
        ("snap_points_33_n1.bin", n1 + "/33/polyMesh/points"),
        ("snap_points_33_n2.bin", n2 + "/33/polyMesh/points"),
        ("snap_points_33_n3.bin", n3 + "/33/polyMesh/points"),
        ("snap_disp_34.bin", n3 + "/34/displacement"),
        ("snap_points_35.bin", n3 + "/35/polyMesh/points"),
        ("snap_points_36.bin", n3 + "/36/polyMesh/points"),
    ]
    for name, path in jobs:
        n, raw = vec_payload(path)
        open(out + "/" + name, "wb").write(raw)
        v = struct.unpack("<3d", raw[:24])
        print(name, n, v)


main()
