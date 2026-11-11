"""Compare the feature sets (toc order) of the port against OpenFOAM's .obj dumps.
usage: python3 -I cmpfeat.py PRIMAL_POLYMESH ESI_RUN_DIR OURS_DIR"""
import re, sys

primal, esi, ours = sys.argv[1:4]


def body(p):
    t = open(p).read()
    t = t[t.index('FoamFile'):]
    return t[t.index('}') + 1:]


pts = [tuple(map(float, m)) for m in re.findall(r'\(([-\d.eE+]+) ([-\d.eE+]+) ([-\d.eE+]+)\)', body(primal + '/points'))]
fs = [list(map(int, m.split())) for m in re.findall(r'\d+\(([\d ]+)\)', body(primal + '/faces'))]
key = lambda c: tuple(round(x, 9) for x in c)
pid = {key(p): i for i, p in enumerate(pts)}
fid = {}
for i, f in enumerate(fs):
    c = tuple(sum(pts[v][k] for v in f) / len(f) for k in range(3))
    fid[key(c)] = i


def obj_pts(path):
    try:
        return [key(map(float, l.split()[1:])) for l in open(path) if l.startswith('v ')]
    except FileNotFoundError:
        return []


def obj_edges(path):
    vs, out = [], []
    for l in open(path):
        w = l.split()
        if w[0] == 'v':
            vs.append(pid[key(map(float, w[1:]))])
        elif w[0] == 'l':
            a, b = int(w[1]) - 1, int(w[2]) - 1
            out.append((min(vs[a], vs[b]), max(vs[a], vs[b])))
    return out


def rd(name, parse):
    try:
        return [parse(l) for l in open(ours + '/' + name) if l.strip()]
    except FileNotFoundError:
        return []


report = []
e_faces = [fid[c] for c in obj_pts(esi + '/featureFaces.obj')]
o_faces = rd('featureFaces.ids', int)
report.append(('featureFaces', e_faces == o_faces, len(e_faces), len(o_faces)))
e_edges = obj_edges(esi + '/featureEdges.obj')
o_edges = rd('featureEdges.ids', lambda l: tuple(map(int, l.split())))
report.append(('featureEdges (sequence)', e_edges == o_edges, len(e_edges), len(o_edges)))
report.append(('featureEdges (as sets)', set(e_edges) == set(o_edges), len(set(e_edges)), len(set(o_edges))))
for nm in ('singleCellFeaturePoints', 'multiCellFeaturePoints'):
    e = [pid[c] for c in obj_pts(esi + '/' + nm + '.obj')]
    o = rd(nm + '.ids', int)
    report.append((nm + ' (sequence)', e == o, len(e), len(o)))
    report.append((nm + ' (as sets)', set(e) == set(o), len(set(e)), len(set(o))))
for r in report:
    print('%-40s same=%-5s esi=%d ours=%d' % r)
E = sorted({(min(f[i], f[(i + 1) % len(f)]), max(f[i], f[(i + 1) % len(f)])) for f in fs for i in range(len(f))})
eid = {e: i for i, e in enumerate(E)}
print('esi featureEdges in lexicographic edge ids:', [eid[e] for e in e_edges[:24]])
print('ours                                      :', [eid[e] for e in o_edges[:24]])
print('pairs:', {i: E[i] for i in (0, 1, 2, 559, 931, 1280, 1281, 1282, 2059, 1813)})
if not report[1][1]:
    for i, (a, b) in enumerate(zip(e_edges, o_edges)):
        if a != b:
            print('first edge-sequence difference at', i, 'esi', a, 'ours', b)
            break
