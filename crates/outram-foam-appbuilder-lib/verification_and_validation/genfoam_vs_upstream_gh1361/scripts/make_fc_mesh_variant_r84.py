# gh:#1938 item 3 (README round 84): two edits of the 2D_fullCoupling FLUID mesh (`constant/fluidRegion/polyMesh`, removeBaffles true, no
# duplicate faces) that `polyTopoChange::changeMesh` of `removeBaffles` meets:
#   swap   : internal faces 0 and 1 (both owned by cell 0) exchange places in `faces`, `owner`, `neighbour`: the internal faces are no longer in
#            upper-triangular order, so `getFaceOrder` RENUMBERS them back (the baffle-less copy differs from the region mesh it was copied from)
#   unused : one point `(0 0 0)` appended to `points` that no face uses: `compact` with `orderPoints = false` keeps it (polyTopoChange.C:983-995)
# Not a shipped case. usage: python3 -I make_fc_mesh_variant_r84.py <case dir> swap|unused
import sys

case, kind = sys.argv[1], sys.argv[2]
pm = case + "/constant/fluidRegion/polyMesh/"


def split(path):
    text = open(path).read()
    head_end = text.index("// * * *")
    head_end = text.index("\n", head_end) + 1
    body = text[head_end:]
    lines = body.split("\n")
    # the count line, then "(", entries, ")"
    i = next(k for k, l in enumerate(lines) if l.strip().isdigit())
    n = int(lines[i])
    assert lines[i + 1] == "("
    entries = lines[i + 2 : i + 2 + n]
    assert lines[i + 2 + n] == ")"
    return text[:head_end], lines[:i], entries, lines[i + 3 + n :]


def join(parts, entries):
    head, pre, _, post = parts
    return head + "\n".join(pre + [str(len(entries)), "("] + entries + [")"] + post)


if kind == "swap":
    for f in ("faces", "owner", "neighbour"):
        parts = split(pm + f)
        e = parts[2]
        e[0], e[1] = e[1], e[0]
        open(pm + f, "w").write(join(parts, e))
    print("swapped internal faces 0 and 1")
elif kind == "unused":
    parts = split(pm + "points")
    e = parts[2] + ["(0 0 0)"]
    open(pm + "points", "w").write(join(parts, e))
    # the `note` of owner/neighbour carries nPoints; polyMesh does not read it
    print("appended an unused point:", len(e), "points")
else:
    sys.exit("kind: swap|unused")
