# gh:#1957, gh:#1967 round 83: ONE-edit (or named-pair) variants, applied IN PLACE to a case directory that already carries its
# protocol (fc_protocol.sh / boiling_mules_protocol.sh / psbt2_pcg_protocol.sh). Written and committed BEFORE the variants' real runs.
# The port's tests apply the same text edits (tests/genfoam_*_vs_upstream.rs, `r83_*`), so both codes read the same bytes.
#
#   c0..c5 (fc channel, onePhase faceCentered `h`): explicit `h` and `hFinal` smoothSolver entries after the catch-all `".*"` (exact keywords
#          win over patterns), tolerance 1e-14, with the extra keys of round 79's s0..s5 (`relTol 0` | `relTol 0.1` | `nSweeps 2` |
#          `maxIter 2` | `relTol 0.05, nSweeps 3, maxIter 7` | `nSweeps -2`).
#   b0..b5 (1D_boiling twoPhase `h.liquid`, `h.vapour`): the same, with explicit `h.liquid` and `h.vapour` entries.
#   k1..k5 (PSBT PhaseII_Ex1 twoPhase `k`/`epsilon`): the shipped `"(k|epsilon|Theta).*"` smoothSolver entry (protocol: tolerance 1e-14,
#          relTol 0, minIter 1 kept) with its `relTol 0;` line replaced by the same extra keys (k0 = the protocol case unchanged).
#   q1     (1D_boiling, gh:#1967): the `"low:top"` structureProperties entry removed and the `liquid.structure` drag zone `"low:mid:top"`
#          -> `"mid"` (named pair: without the second edit the drag of the structure-free cells would divide by Dh = SMALL). q0 = the
#          protocol case unchanged.
# usage: python3 -I make_round83_variant.py <case dir> <variant>
import sys

EXTRA = {
    "0": "        relTol          0;\n",
    "1": "        relTol          0.1;\n",
    "2": "        relTol          0;\n        nSweeps         2;\n",
    "3": "        relTol          0;\n        maxIter         2;\n",
    "4": "        relTol          0.05;\n        nSweeps         3;\n        maxIter         7;\n",
    "5": "        relTol          0;\n        nSweeps         -2;\n",
}


def edit(path, f):
    with open(path) as h:
        t = h.read()
    u = f(t)
    if u == t:
        sys.exit(f"{path}: edit changed nothing")
    with open(path, "w") as h:
        h.write(u)


def after_catch_all(names, extra):
    def f(t):
        i = t.index('".*"')
        close = i + t[i:].index("}")
        nl = close + t[close:].index("\n") + 1
        entries = "".join(
            f"    {n}\n    {{\n        solver          smoothSolver;\n        smoother        symGaussSeidel;\n        tolerance       1e-14;\n{extra}    }}\n"
            for n in names
        )
        return t[:nl] + entries + t[nl:]
    return f


def turbulence_entry(extra):
    def f(t):
        i = t.index('"(k|epsilon|Theta).*"')
        close = i + t[i:].index("}")
        block = t[i:close]
        assert block.count("relTol          0;\n") == 1, block
        return t[:i] + block.replace("        relTol          0;\n", extra) + t[close:]
    return f


def no_low_top(t):
    i = t.index('    "low:top"\n    {')
    close = i + t[i:].index("    }\n") + len("    }\n")
    return t[:i] + t[close:]


def main():
    case, v = sys.argv[1], sys.argv[2]
    fv = f"{case}/system/fluidRegion/fvSolution"
    pp = f"{case}/constant/fluidRegion/phaseProperties"
    if v[0] == "c" and v[1:] in EXTRA:
        edit(fv, after_catch_all(["h", "hFinal"], EXTRA[v[1:]]))
    elif v[0] == "b" and v[1:] in EXTRA:
        edit(fv, after_catch_all(["h.liquid", "h.vapour"], EXTRA[v[1:]]))
    elif v[0] == "k" and v[1:] in EXTRA and v != "k0":
        edit(fv, turbulence_entry(EXTRA[v[1:]]))
    elif v == "q1":
        edit(pp, no_low_top)
        edit(pp, lambda t: t.replace('            "low:mid:top"\n', '            "mid"\n', 1))
    elif v in ("k0", "q0"):
        pass
    else:
        sys.exit(f"unknown variant {v}")


main()
