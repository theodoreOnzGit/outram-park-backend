#!/usr/bin/env python3
"""FNV-1a hashes of the `dumpQuality` probe's field files (gh:#1364).

usage: quality_hashes.py CASE_DIR...

For each `CASE_DIR/quality/<field>` prints `case field n hash`, where the hash is
FNV-1a (64 bit) over the IEEE-754 bit patterns of the values in file order
(vector fields flattened x y z), the same fold as `tests/check_mesh_esi_v2506.rs`.
Research, education and V&V only. Scratch tooling; reads only text files.
"""
import os
import struct
import sys

FIELDS = ["faceCentres", "faceAreas", "cellCentres", "cellVolumes", "ortho", "skew", "ownPyr",
          "neiPyr", "openness", "aspect", "flatness", "concavity", "determinant"]


def fnv(values):
    h = 0xcbf29ce484222325
    for v in values:
        b = struct.unpack('<Q', struct.pack('<d', v))[0]
        h = ((h ^ b) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    return h


for case in sys.argv[1:]:
    name = os.path.basename(case.rstrip('/'))
    for f in FIELDS:
        vals = [float(t) for t in open(os.path.join(case, 'quality', f)).read().split()]
        print('%s %s %d 0x%016x' % (name, f, len(vals), fnv(vals)))
