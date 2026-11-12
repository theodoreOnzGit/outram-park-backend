#!/usr/bin/env python3
"""gh:#1459/#1467 round 5: extract the face/edge merging references from an ESI
v2506 flange snap run (`snappyHexMeshDict.snap`, `debugFlags (mesh)`, ASCII
`writePrecision 17`, so every double round-trips).

usage: python3 -I extract_merge_fixtures.py RUN_DIR OUT_DIR

Times of the run: 36 = snapped mesh (before repatch and merging), 37 = after
`mergePatchFacesUndo` (faces merged, unused points removed), 38 = after
`mergeEdgesUndo` (straight edge points removed), 40 = the final mesh (same as 38).

Writes (plain text; the caller gzips with `gzip -n`):
  esi_merge_final_faces.txt   time-40 faces in ESI order, `owner neighbour p0 p1 ..`
                              (neighbour -1-k for patch k of the 4 flange patches),
                              point labels are TIME-40 labels
  esi_merge_final_points.txt  per time-40 point its TIME-36 label (the final points are
                              an order-preserving subset of time 36; checked here with
                              exact coordinate equality)
  esi_merge_final_boundary.txt `name startFace nFaces`
  esi_merge_stage1_diff.txt   faces whose time-37 ring (in TIME-36 point labels) differs
                              from the time-40 ring (in time-36 labels): `face p0 p1 ..`
                              (the faces touched by `mergeEdgesUndo`); everything else of
                              time 37 equals time 40
  esi_merge_stage1_removed.txt time-36 labels of the points removed by
                              `mergePatchFacesUndo` (unused points)
  esi_merge_all_face_sets.txt the faces of `36/polyMesh/sets/allFaceSets` (faces in any
                              merge set), TIME-36 face labels, ascending
Standard library only. Tooling for V&V, reached by the recorded run, not a test.
"""
import re
import sys


def body(path):
    t = open(path).read()
    t = re.sub(r"/\*.*?\*/", "", t, flags=re.S)
    t = re.sub(r"//[^\n]*", "", t)
    return t


def read_list(path, kind):
    t = body(path)
    m = re.search(r"\n(\d+)\s*\n\(", t)
    n = int(m.group(1))
    s = t[m.end():]
    s = s[: s.rindex(")")]
    if kind == "points":
        return [tuple(a.split()) for a in re.findall(r"\(([^)]*)\)", s)][:n]
    if kind == "faces":
        return [[int(x) for x in a.split()] for _, a in re.findall(r"(\d+)\(([^)]*)\)", s)][:n]
    return [int(x) for x in s.split()][:n]


def read_boundary(path):
    out = []
    for m in re.finditer(r"(\w+)\s*\{([^}]*)\}", body(path)):
        d = m.group(2)
        if "nFaces" not in d:
            continue
        nf = int(re.search(r"nFaces\s+(\d+)", d).group(1))
        sf = int(re.search(r"startFace\s+(\d+)", d).group(1))
        out.append((m.group(1), sf, nf))
    return out


def subset_map(p36, pn):
    """label in p36 of each point of pn (order preserving, exact text equality)."""
    out = []
    j = 0
    for x in pn:
        while p36[j] != x:
            j += 1
        out.append(j)
        j += 1
    return out


def main():
    run, out = sys.argv[1], sys.argv[2]
    p36 = read_list(run + "/36/polyMesh/points", "points")
    maps = {}
    faces = {}
    for t in (37, 40):
        pt = read_list("%s/%d/polyMesh/points" % (run, t), "points")
        maps[t] = subset_map(p36, pt)
        faces[t] = read_list("%s/%d/polyMesh/faces" % (run, t), "faces")
    O = read_list(run + "/40/polyMesh/owner", "label")
    N = read_list(run + "/40/polyMesh/neighbour", "label")
    B = [b for b in read_boundary(run + "/40/polyMesh/boundary") if b[2] > 0]
    assert len(B) == 4 and len(O) == len(faces[40])
    patch_of = {}
    for k, (name, sf, nf) in enumerate(B):
        for f in range(sf, sf + nf):
            patch_of[f] = k
    lines = []
    for f, ring in enumerate(faces[40]):
        nei = N[f] if f < len(N) else -1 - patch_of[f]
        lines.append(" ".join(str(x) for x in [O[f], nei] + ring) + "\n")
    open(out + "/esi_merge_final_faces.txt", "w").write("".join(lines))
    open(out + "/esi_merge_final_points.txt", "w").write("".join("%d\n" % x for x in maps[40]))
    open(out + "/esi_merge_final_boundary.txt", "w").write("".join("%s %d %d\n" % b for b in B))
    diff = []
    for f in range(len(faces[40])):
        r37 = [maps[37][p] for p in faces[37][f]]
        r40 = [maps[40][p] for p in faces[40][f]]
        if r37 != r40:
            diff.append("%d %s\n" % (f, " ".join(str(x) for x in r37)))
    open(out + "/esi_merge_stage1_diff.txt", "w").write("".join(diff))
    kept = set(maps[37])
    open(out + "/esi_merge_stage1_removed.txt", "w").write(
        "".join("%d\n" % i for i in range(len(p36)) if i not in kept)
    )
    sets = read_list(run + "/36/polyMesh/sets/allFaceSets", "label")
    open(out + "/esi_merge_all_face_sets.txt", "w").write("".join("%d\n" % i for i in sorted(sets)))
    print("time37 points", len(maps[37]), "time40 points", len(maps[40]), "faces", len(faces[40]),
          "stage1-diff faces", len(diff), "set faces", len(sets), B)


main()
