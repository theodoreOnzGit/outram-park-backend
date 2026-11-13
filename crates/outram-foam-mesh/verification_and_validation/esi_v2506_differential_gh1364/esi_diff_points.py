#!/usr/bin/env python3
"""Pin-table builder for the gh:#1364 ESI v2506 blockMesh differential.

usage: esi_diff_points.py NAME UPSTREAM_POLYMESH_DIR PORT_DUMP

PORT_DUMP is the file written by `DUMP_POINTS=path compare_polymesh ...`
(lines `index xbits ybits zbits`, hex). Prints, to stdout, one line
`NAME index xbits ybits zbits` for every point where the port differs from the
ESI v2506 binary, with the BINARY's bit patterns, plus a `# NAME n worst` comment.
Research, education and V&V only. Scratch tooling; reads only text files.
"""
import struct
import sys


def bits(x):
    return struct.unpack('<Q', struct.pack('<d', x))[0]


def upstream_points(path):
    text = open(path).read()
    body = text[text.index('}', text.index('FoamFile')) + 1:]
    body = ' '.join(l for l in body.splitlines() if not l.strip().startswith('//'))
    body = body.replace('(', ' ').replace(')', ' ').split()
    n = int(body[0])
    nums = [float(t) for t in body[1:1 + 3 * n]]
    return [(nums[3 * i], nums[3 * i + 1], nums[3 * i + 2]) for i in range(n)]


def main():
    name, d, dump = sys.argv[1:4]
    up = upstream_points(d + '/points')
    ours = []
    for line in open(dump):
        t = line.split()
        ours.append(tuple(int(h, 16) for h in t[1:4]))
    assert len(ours) == len(up), (len(ours), len(up))
    n = 0
    worst = 0.0
    for i, (u, o) in enumerate(zip(up, ours)):
        ub = tuple(bits(c) for c in u)
        if ub != o:
            n += 1
            of = tuple(struct.unpack('<d', struct.pack('<Q', b))[0] for b in o)
            worst = max(worst, max(abs(a - b) for a, b in zip(u, of)))
            print(name, i, *('%016x' % b for b in ub))
    print('# %s differing %d worst %r' % (name, n, worst))


main()
