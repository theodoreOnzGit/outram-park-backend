"""FNV-1a hashes of an OpenFOAM polyMesh directory, the frozen references of
tests/upstream_dev_852527377c7e.rs. usage: python3 -I dev_hashes.py POLYMESH_DIR...
points: FNV-1a over the f64 bit patterns (x y z per point), same as compare_polymesh.
topology: FNV-1a over u64 values: per face (len, verts...), then owner, then neighbour."""
import re, struct, sys

def body(d, f):
    t = open(f'{d}/{f}').read()
    return t[t.index('}') + 1:]

def fnv(vals):
    h = 0xcbf29ce484222325
    for v in vals:
        h = ((h ^ v) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    return h

def hashes(d):
    pts = re.findall(r'\(([-\d.eE+]+) ([-\d.eE+]+) ([-\d.eE+]+)\)', body(d, 'points'))
    bits = [struct.unpack('<Q', struct.pack('<d', float(x)))[0] for p in pts for x in p]
    faces = [list(map(int, m.split())) for m in re.findall(r'\d+\(([\d ]+)\)', body(d, 'faces'))]
    def ilist(f):
        t = body(d, f)
        n = int(re.search(r'^(\d+)\s*\(', t, re.M).group(1))
        return list(map(int, t[t.index('(') + 1:t.rindex(')')].split())), n
    owner, no = ilist('owner')
    nei, nn = ilist('neighbour')
    assert len(owner) == no and len(nei) == nn
    topo = [x for f in faces for x in [len(f)] + f] + owner + nei
    return len(pts), fnv(bits), len(faces), fnv(topo)

for d in sys.argv[1:]:
    n, hp, nf, ht = hashes(d)
    print(f'{d} points {n} {hp:016x} faces {nf} topo {ht:016x}')
