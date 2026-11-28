#!/usr/bin/env python3
# gh:#1991 README round 88: extract the real binary's references of one 3D_HTR-10 run (run_htr10_r88_real.sh) into the refs
# submodule: the log (gzipped) and every 7th cell of the compared written fields of the LAST written time (`%.17g` text already
# in the files, copied verbatim, one value per line, gzipped). No value is recomputed. The case itself is NOT copied: the ct*
# variants are refs/htr10_r87/case with the shipped schemes restored, sh20 is the shipped rootCase.
#
# usage: make_htr10_r88_refs.py <run dir> <tag> <out dir: refs/htr10_r88>
import gzip
import os
import shutil
import sys

run, tag, out = sys.argv[1:4]
os.makedirs(out, exist_ok=True)
NCELLS = 337675
FIELDS = ["T", "p", "p_rgh", "U", "k", "epsilon", "nut", "alphat", "htc", "TpS.nuclearSteadyStatePebble", "Tfmax.nuclearSteadyStatePebble"]


def internal_tokens(path):
    """The internalField values of a written volField, as text tokens (scalars) or `x y z` triples (vectors)."""
    t = open(path).read()
    i = t.index("internalField")
    head = t[i:t.index(";", i)].split()
    if len(head) >= 3 and head[1] == "uniform":
        return [" ".join(head[2:]).strip("()")] * NCELLS
    j = t.index("(", i)
    n = int(t[t.index(">", i) + 1 : j].strip())
    body = t[j + 1 :]
    if body.lstrip().startswith("("):
        vals = []
        k = 0
        for _ in range(n):
            a = body.index("(", k)
            b = body.index(")", a)
            vals.append(" ".join(body[a + 1 : b].split()))
            k = b + 1
        return vals
    return body.split(None, n)[:n]


def last_time(d):
    ts = [x for x in os.listdir(d) if x.replace(".", "", 1).replace("e-", "", 1).isdigit() and x != "0"]
    return sorted(ts, key=float)[-1]


with open(os.path.join(run, "log.GeN-Foam"), "rb") as f, gzip.open(os.path.join(out, f"{tag}_log.gz"), "wb", 9) as g:
    shutil.copyfileobj(f, g)
td = os.path.join(run, last_time(run), "fluidRegion")
for name in FIELDS:
    p = os.path.join(td, name)
    if not os.path.exists(p):
        continue
    vals = internal_tokens(p)[::7]
    with gzip.open(os.path.join(out, f"{tag}_{name}_s7.txt.gz"), "wt", 9) as g:
        g.write("\n".join(vals) + "\n")
print("extracted", tag, "from", td)
