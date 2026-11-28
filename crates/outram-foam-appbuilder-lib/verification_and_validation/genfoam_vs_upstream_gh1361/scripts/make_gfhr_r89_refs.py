#!/usr/bin/env python3
# gh:#1876 README round 89: the references of the 3D_gFHR runs of run_gfhr_r89_real.sh, into the refs submodule.
#
#   make_gfhr_r89_refs.py case <shipped rootCase> <out dir: refs/gfhr_r89>
#       stage the shipped rootCase files the solver reads (gzipped, mtime 0) into <out>/case: 0/fluidRegion/*,
#       constant/{regionProperties,multiRegionCouplingDict}, constant/fluidRegion/{g,phaseProperties,phaseProperties_lumpedParam,
#       thermophysicalProperties,turbulenceProperties,polyMesh/*}, system/controlDict, system/fluidRegion/{fvSchemes,fvSolution}.
#       Not copied: constant/polyMesh (the same mesh again; GeN-Foam reads constant/fluidRegion/polyMesh), blockMeshDict,
#       cylinderMesh.m4, topoSetDict, setFieldsDict, decomposeParDict, phasePropertiesOld (not read by the solver).
#   make_gfhr_r89_refs.py run <run dir> <tag> <out dir>
#       the log (gzipped) and every 7th cell of the compared fields of the LAST written time (`%.17g` or `%.6g` text as written,
#       copied verbatim, one value per line, gzipped). No value is recomputed.
import gzip
import os
import shutil
import sys

NCELLS = 292500
FIELDS = ["T", "p", "p_rgh", "U", "k", "epsilon", "nut", "alphat", "htc", "TpS.nuclearSteadyStatePebble", "Tfmax.nuclearSteadyStatePebble",
          "Tmax.lumpedNuclearStructure", "Tsurface.lumpedNuclearStructure"]
CASE_FILES = ["constant/regionProperties", "constant/multiRegionCouplingDict", "system/controlDict", "system/fluidRegion/fvSchemes",
              "system/fluidRegion/fvSolution"] + [f"constant/fluidRegion/{f}" for f in
              ["g", "phaseProperties", "phaseProperties_lumpedParam", "thermophysicalProperties", "turbulenceProperties"]]


def gz(src, dst):
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    with open(src, "rb") as f, open(dst, "wb") as raw:
        with gzip.GzipFile(fileobj=raw, mode="wb", compresslevel=9, mtime=0, filename="") as g:
            shutil.copyfileobj(f, g)


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


mode = sys.argv[1]
if mode == "case":
    root, out = sys.argv[2:4]
    files = CASE_FILES + [f"0/fluidRegion/{f}" for f in sorted(os.listdir(os.path.join(root, "0/fluidRegion")))]
    files += [f"constant/fluidRegion/polyMesh/{f}" for f in sorted(os.listdir(os.path.join(root, "constant/fluidRegion/polyMesh")))]
    for rel in files:
        gz(os.path.join(root, rel), os.path.join(out, "case", rel + ".gz"))
    print("staged", len(files), "files")
elif mode == "run":
    run, tag, out = sys.argv[2:5]
    os.makedirs(out, exist_ok=True)
    gz(os.path.join(run, "log.GeN-Foam"), os.path.join(out, f"{tag}_log.gz"))
    for extra in ["log.init", "md5_before.txt", "md5_after.txt"]:
        if os.path.exists(os.path.join(run, extra)):
            gz(os.path.join(run, extra), os.path.join(out, f"{tag}_{extra}.gz"))
    # the run's own controlDict and fvSolution (the variant's edits), copied as run, so the port reads the same files
    for rel in ["system/controlDict", "system/fluidRegion/fvSolution", "constant/fluidRegion/phaseProperties"]:
        os.makedirs(os.path.dirname(os.path.join(out, f"{tag}_files", rel)), exist_ok=True)
        shutil.copyfile(os.path.join(run, rel), os.path.join(out, f"{tag}_files", rel))
    td = os.path.join(run, last_time(run), "fluidRegion")
    for name in FIELDS:
        p = os.path.join(td, name)
        if not os.path.exists(p):
            continue
        vals = internal_tokens(p)[::7]
        with open(os.path.join(out, f"{tag}_{name}_s7.txt.gz"), "wb") as raw:
            with gzip.GzipFile(fileobj=raw, mode="wb", compresslevel=9, mtime=0, filename="") as g:
                g.write(("\n".join(vals) + "\n").encode())
    print("extracted", tag, "from", td)
else:
    sys.exit("usage: make_gfhr_r89_refs.py case <rootCase> <out> | run <run dir> <tag> <out>")
