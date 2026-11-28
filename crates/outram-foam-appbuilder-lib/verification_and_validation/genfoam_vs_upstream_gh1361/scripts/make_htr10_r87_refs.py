#!/usr/bin/env python3
# gh:#1876 README round 87: stage the 3D_HTR-10 tight `uncorrected` variant (ut3) into the refs submodule and extract the real
# binary's references from its run directories.
#
# usage: make_htr10_r87_refs.py <upstream rootCase> <variant case dir (ut3, already run)> <step-1 run dir (ut2)> \
#                               <laminar step-1 run dir (vlam)> <out dir: refs/htr10_r87>
#
# The case (polyMesh and the start fields the solver reads) is copied gzipped, the edited system dictionaries plain. The
# references: each run's log (gzipped) and every 7th cell of the compared written fields (`%.17g` text already in the files,
# copied verbatim, one value per line, gzipped). No value is recomputed.
import gzip
import os
import shutil
import sys

root, ut3, ut2, vlam, out = sys.argv[1:6]
case = os.path.join(out, "case")
os.makedirs(case, exist_ok=True)

GZ = [
    "constant/fluidRegion/polyMesh/points", "constant/fluidRegion/polyMesh/faces", "constant/fluidRegion/polyMesh/owner",
    "constant/fluidRegion/polyMesh/neighbour", "constant/fluidRegion/polyMesh/cellZones",
    "0/fluidRegion/alpha.structure", "0/fluidRegion/powerDensityNeutronics", "0/fluidRegion/T",
]
PLAIN_FROM_ROOT = [
    "constant/fluidRegion/polyMesh/boundary", "constant/fluidRegion/g", "constant/fluidRegion/phaseProperties",
    "constant/fluidRegion/thermophysicalProperties", "constant/fluidRegion/turbulenceProperties",
    "constant/multiRegionCouplingDict", "constant/regionProperties",
    "0/fluidRegion/U", "0/fluidRegion/p_rgh", "0/fluidRegion/k", "0/fluidRegion/epsilon", "0/fluidRegion/nut", "0/fluidRegion/alphat",
]
PLAIN_FROM_VARIANT = ["system/controlDict", "system/fluidRegion/fvSchemes", "system/fluidRegion/fvSolution"]

for rel in GZ:
    dst = os.path.join(case, rel + ".gz")
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    with open(os.path.join(root, rel), "rb") as f, gzip.open(dst, "wb", 9) as g:
        shutil.copyfileobj(f, g)
for rel in PLAIN_FROM_ROOT:
    dst = os.path.join(case, rel)
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    shutil.copyfile(os.path.join(root, rel), dst)
for rel in PLAIN_FROM_VARIANT:
    dst = os.path.join(case, rel)
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    shutil.copyfile(os.path.join(ut3, rel), dst)


def internal_tokens(path):
    """The internalField values of a written volField, as the text tokens (scalars) or the `(x y z)` triples (vectors)."""
    t = open(path).read()
    i = t.index("internalField")
    head = t[i:t.index(";", i)].split()
    if len(head) >= 3 and head[1] == "uniform":
        # a uniform field (e.g. a zero `alphat` in the laminar run): the value in every cell of the 337 675
        return [" ".join(head[2:]).strip("()")] * 337675
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


FIELDS = ["T", "p", "p_rgh", "U", "k", "epsilon", "nut", "alphat", "htc", "TpS.nuclearSteadyStatePebble", "Tfmax.nuclearSteadyStatePebble"]


def sample(run_time_dir, tag):
    for name in FIELDS:
        p = os.path.join(run_time_dir, "fluidRegion", name)
        if not os.path.exists(p):
            continue
        vals = internal_tokens(p)[::7]
        with gzip.open(os.path.join(out, f"{tag}_{name}_s7.txt.gz"), "wt", 9) as g:
            g.write("\n".join(vals) + "\n")


def last_time(d):
    ts = [x for x in os.listdir(d) if x.replace(".", "", 1).replace("e-", "", 1).isdigit() and x != "0"]
    return sorted(ts, key=float)[-1]


for d, tag in [(ut3, "ut3"), (ut2, "ut2"), (vlam, "vlam")]:
    with open(os.path.join(d, "log.GeN-Foam"), "rb") as f, gzip.open(os.path.join(out, f"{tag}_log.gz"), "wb", 9) as g:
        shutil.copyfileobj(f, g)
    sample(os.path.join(d, last_time(d)), tag)
print("staged", out)
