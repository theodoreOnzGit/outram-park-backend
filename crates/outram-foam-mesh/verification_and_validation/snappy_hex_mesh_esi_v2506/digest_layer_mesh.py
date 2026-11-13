#!/usr/bin/env python3
"""gh:#1467 round 10: FNV-1a 64-bit digests of ESI v2506 meshes, to pin large references in a test
without committing the meshes.

usage: python3 -I digest_layer_mesh.py OUT_DIR TIME ...

OUT_DIR holds `layer_mesh_TIME.txt` and `layer_points_TIME.bin` as written by
`extract_layer_fixtures.py`. For each TIME prints
  `TIME faces <hex> <n lines> points <hex> <n bytes>`
where the face digest runs over the bytes of the `layer_mesh_TIME.txt` text and the point digest
over the raw little-endian doubles. The Rust tests (`tests/snappy_hex_mesh_esi_layers_multi.rs`)
serialise the port's mesh into the same bytes and hash them with the same function.
Standard library only. Tooling for V&V, reached by the recorded run, not a test.
"""
import sys

OFFSET = 0xCBF29CE484222325
PRIME = 0x100000001B3
MASK = (1 << 64) - 1


def fnv1a(data):
    h = OFFSET
    for b in data:
        h = ((h ^ b) * PRIME) & MASK
    return h


def main():
    out = sys.argv[1]
    for t in sys.argv[2:]:
        res = [t]
        try:
            f = open(f"{out}/layer_mesh_{t}.txt", "rb").read()
            res += ["faces", "%016x" % fnv1a(f), str(f.count(b"\n"))]
        except FileNotFoundError:
            pass
        p = open(f"{out}/layer_points_{t}.bin", "rb").read()
        res += ["points", "%016x" % fnv1a(p), str(len(p))]
        print(" ".join(res))


if __name__ == "__main__":
    main()
