# gh:#1926 round 66: pack a written time directory (the `100` of the steady state of 2D_fullCoupling, after `-initializeMappedFields` of the
# transient stage) into ONE text-framed file the port's test unpacks: for every file, in sorted order,
#   "FILE\t<path relative to the time directory>\t<byte count>\n" + <bytes> + "\n"
# usage: python3 -I pack_start_dir_r66.py <time dir> <out file>   (then gzip -9 -n)
import os
import sys

root, out = sys.argv[1], sys.argv[2]
files = []
for d, _, names in os.walk(root):
    for n in names:
        p = os.path.join(d, n)
        files.append((os.path.relpath(p, root), p))
files.sort()
with open(out, "wb") as o:
    for rel, p in files:
        b = open(p, "rb").read()
        o.write(("FILE\t%s\t%d\n" % (rel, len(b))).encode())
        o.write(b)
        o.write(b"\n")
print(len(files), "files")
