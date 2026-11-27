#!/usr/bin/env python3
# gh:#1891 (README round 43): refs/msfr_prec_matrix_r43.ref.txt.gz from the dump files of the INSTRUMENTED upstream
# (scratch copy of src/classes/{common,neutronics} with `include/precEq.H` writing r43dump_prec_<n>.txt and r43dump_geom.txt;
# see the README, "Round 43", for the patch; the shipped sources and libraries are untouched).
#   extract_prec_matrix_reference.py <run dir with the dumps> <output .ref.txt.gz> [calls = 0,3,7]
# Output: `KEY v v v ...` lines (full doubles, `repr`). Shared: alpha, diffCoeffPrec, rhoCool, alphat, mu, neutroSource, phi, Ucell,
# phiB<p>, internalCoeffs<p>, boundaryCoeffs<p>, Sf, Cf, C, w (internal faces / cells). Per call k: HEADER_k (keff, deltaT),
# precStarOld_k, precStarCur_k, source_k, diag_k, upper_k, lower_k, Beta_k.
import gzip
import re
import sys


def parse(path):
    lines = open(path).read().split("\n")
    out = {}
    i = 1 if lines[0].startswith("call") else 0
    if lines[0].startswith("call"):
        w = lines[0].split()
        out["HEADER"] = [float(w[w.index("keff") + 1]), float(w[w.index("deltaT") + 1])]
    while i < len(lines):
        l = lines[i]
        if l == "" or l == ";":
            i += 1
            continue
        name, _, rest = l.partition(" ")
        if name.startswith("SfB") or name == "V":
            break  # the geometry file continues with boundary blocks and per-face debug lines, not needed
        if name.startswith("phiB"):
            j = i + 1
            while lines[j] != "(":
                j += 1
            n = int(lines[j - 1])
            out[name] = [float(x) for x in lines[j + 1 : j + 1 + n]]
            i = j + 2 + n
            continue
        if rest == "" or rest.strip() == "":
            n = int(lines[i + 1])
            assert lines[i + 2] == "(", (name, lines[i + 2])
            vals = []
            for k in range(n):
                t = lines[i + 3 + k].strip("()").split()
                vals.extend(float(x) for x in t)
            out[name] = vals
            i += 4 + n
            continue
        m = re.match(r"^(\d+)\{(.*)\}$", rest)
        if m:
            out[name] = [float(m.group(2))] * int(m.group(1))
        i += 1
    return out


def main():
    run, outp = sys.argv[1], sys.argv[2]
    calls = [int(x) for x in (sys.argv[3] if len(sys.argv) > 3 else "0,3,7").split(",")]
    first = parse(f"{run}/r43dump_prec_{calls[0]}.txt")
    geo = parse(f"{run}/r43dump_geom.txt")
    lines = []

    def emit(key, vals):
        lines.append(key + " " + " ".join(repr(v) for v in vals))

    for key in ["alpha", "diffCoeffPrec", "rhoCool", "alphat", "mu", "neutroSource", "phi", "Ucell"]:
        emit(key, first[key])
    for p in range(6):
        emit(f"phiB{p}", first[f"phiB{p}"])
        emit(f"internalCoeffs{p}", first[f"internalCoeffs{p}"])
        emit(f"boundaryCoeffs{p}", first[f"boundaryCoeffs{p}"])
    for key in ["Sf", "Cf", "C", "w"]:
        emit(key, geo[key])
    for k in calls:
        d = parse(f"{run}/r43dump_prec_{k}.txt")
        emit(f"HEADER_{k}", d["HEADER"])
        for key in ["precStarOld", "precStarCur", "source", "diag", "upper", "lower", "Beta"]:
            emit(f"{key}_{k}", d[key])
    # no file name in the header (the tests read the deflate stream at byte 10)
    with open(outp, "wb") as raw, gzip.GzipFile(filename="", mode="wb", compresslevel=9, fileobj=raw, mtime=0) as f:
        f.write(("\n".join(lines) + "\n").encode())


main()
