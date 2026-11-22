#!/usr/bin/env python3
"""gh:#1467 round 10: extract the fields `snappyLayerDriver::writeLayerData` writes
(`writeFlags (layerFields)`, binary write) from an ESI v2506 run.

usage: python3 -I extract_layer_fields.py RUN_DIR OUT_DIR TIME

Concatenates, in file order, the data of every non-empty `nonuniform List<scalar>` block of
`TIME/nSurfaceLayers` (internal field = per cell, then one block per patch), `TIME/thickness` and
`TIME/thicknessFraction` (one block per patch; their internal field is `uniform 0`) into
`OUT_DIR/layer_fields_TIME.bin` as little-endian doubles and prints the number of values per file.
Standard library only. Tooling for V&V, reached by the recorded run, not a test.
"""
import re
import sys


def blocks(path):
    data = open(path, "rb").read()
    out = []
    for m in re.finditer(rb"List<scalar>\s*\n(\d+)\n\(", data):
        n = int(m.group(1))
        out.append(data[m.end():m.end() + 8 * n])
    return out


def main():
    run, out, t = sys.argv[1], sys.argv[2], sys.argv[3]
    raw = b""
    for name in ("nSurfaceLayers", "thickness", "thicknessFraction"):
        bs = blocks(f"{run}/{t}/{name}")
        n = sum(len(b) for b in bs) // 8
        print(name, "blocks", len(bs), "values", n)
        raw += b"".join(bs)
    open(f"{out}/layer_fields_{t}.bin", "wb").write(raw)


if __name__ == "__main__":
    main()
