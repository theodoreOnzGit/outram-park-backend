#!/usr/bin/env python3
"""gh:#1929 round 65: patch (boundary) values of `fluidRegion` fields of written steps, appended to a reference file as
`P <time> <field> <patch> v1 v2 ...` (a `uniform v` entry is expanded to the patch size read from `constant/fluidRegion/polyMesh/boundary`).
Patches without a `value` entry (zeroGradient written without one cannot occur: OpenFOAM writes `value` for every non-constraint patch) are skipped.
usage: extract_patch_values.py <run dir> <out.txt (appended)> "<time dirs>" "<field names>"
"""
import re, sys, os

def sizes(run):
    t = open(os.path.join(run, 'constant/fluidRegion/polyMesh/boundary')).read()
    out = {}
    for m in re.finditer(r'\n\s+(\w+)\s*\{([^}]*)\}', t):
        n = re.search(r'nFaces\s+(\d+)', m.group(2))
        if n:
            out[m.group(1)] = int(n.group(1))
    return out

def patches(text):
    i = text.index('boundaryField')
    body = text[text.index('{', i) + 1:]
    res = {}
    for m in re.finditer(r'(\w+)\s*\{', body):
        j = m.end(); depth = 1; k = j
        while depth:
            c = body[k]; depth += (c == '{') - (c == '}'); k += 1
        res.setdefault(m.group(1), body[j:k - 1])
    return res

def main():
    run, out, times, fields = sys.argv[1], sys.argv[2], sys.argv[3].split(), sys.argv[4].split()
    sz = sizes(run)
    with open(out, 'a') as o:
        for t in times:
            for f in fields:
                p = os.path.join(run, t, 'fluidRegion', f)
                if not os.path.exists(p):
                    p += '.gz'
                if p.endswith('.gz'):
                    import gzip
                    text = gzip.open(p, 'rt').read()
                else:
                    text = open(p).read()
                for name, blk in patches(text).items():
                    if name not in sz:
                        continue
                    m = re.search(r'value\s+uniform\s+([-+0-9.eE]+)\s*;', blk)
                    if m:
                        vals = [m.group(1)] * sz[name]
                    else:
                        m = re.search(r'value\s+nonuniform\s+List<scalar>\s*(\d+)\s*\(([^)]*)\)', blk, re.S)
                        if not m:
                            continue
                        vals = m.group(2).split()
                    o.write('P %s %s %s %s\n' % (t, f, name, ' '.join(vals)))

main()
