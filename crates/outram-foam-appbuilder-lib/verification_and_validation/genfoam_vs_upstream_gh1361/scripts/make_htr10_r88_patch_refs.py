#!/usr/bin/env python3
# gh:#1992 README round 88: the PATCH values of the written step-1 fields of a real 3D_HTR-10 run (run_htr10_r88_real.sh ct2), for
# the boundary-value probe of the 2e-12 turbulent first-p_rgh difference. One gzipped text file per field,
# `<tag>_<field>_patches.txt.gz`, a line `<patch> <value>` per face (a vector: its three components), the `%.17g` text of the
# written file copied verbatim (a `uniform` patch value repeated over the patch's faces). No value is recomputed.
#
# usage: make_htr10_r88_patch_refs.py <run dir> <time name> <tag> <out dir: refs/htr10_r88>
import gzip
import os
import re
import sys

run, tname, tag, out = sys.argv[1:5]
FIELDS = ["nut", "alphat", "k", "epsilon", "U", "p_rgh", "p", "T"]
bnd = open(os.path.join(run, "constant/fluidRegion/polyMesh/boundary")).read()
sizes = {m.group(1): int(m.group(2)) for m in re.finditer(r"\n\s+(\w+)\s*\n\s*\{[^}]*?nFaces\s+(\d+);", bnd)}


def patch_values(text, name, n):
    i = re.search(r"\n\s+" + name + r"\s*\n\s*\{", text).end()
    body = text[i:]
    # the patch's own block (a `zeroGradient` patch writes no `value`: nothing to compare)
    m = re.search(r"\bvalue\b", body[:body.index("}")])
    if m is None:
        return []
    j = m.start()
    rest = body[j + len("value"):].lstrip()
    if rest.startswith("uniform"):
        v = rest[len("uniform"):rest.index(";")].strip().strip("()")
        return [" ".join(v.split())] * n
    k = rest.index("(")
    lst = rest[k + 1:]
    if lst.lstrip().startswith("("):
        vals, pos = [], 0
        for _ in range(n):
            a = lst.index("(", pos)
            b = lst.index(")", a)
            vals.append(" ".join(lst[a + 1:b].split()))
            pos = b + 1
        return vals
    return lst.split(None, n)[:n]


for f in FIELDS:
    p = os.path.join(run, tname, "fluidRegion", f)
    if not os.path.exists(p):
        continue
    text = open(p).read()
    text = text[text.index("boundaryField"):]
    with gzip.open(os.path.join(out, f"{tag}_{f}_patches.txt.gz"), "wt", 9) as g:
        for name, n in sizes.items():
            for v in patch_values(text, name, n):
                g.write(f"{name} {v}\n")
print("patch values of", tag, "written")
