#!/usr/bin/env python3
"""round 85 (gh:#1946): edit the shipped TStruct GAMG entry of a legacy thermo-mechanics case for the S2 variants.
usage: edit_tstruct_r85.py <fvSolution> <variant>   variant: b1 b2 b3 b4 b5 b6 b7 (or s2t: the round 72 s2t protocol alone)
Every variant: solver GAMGdump, tolerance 1e-14, relTol 0, maxIter 40 (round 72's s2t), then ONE edit."""
import re, sys
p, v = sys.argv[1], sys.argv[2]
t = open(p).read()
i = t.index('"TStruct"')
j = t.index('}', i)
b = t[i:j]
b = b.replace('solver          GAMG;', 'solver          GAMGdump;')
b = b.replace('tolerance       1e-06;', 'tolerance       1e-14;')
b = b.replace('relTol          0.001;', 'relTol          0;\n        maxIter         40;')
edits = {
    's2t': None,
    'b1': ('smoother        GaussSeidel;', 'smoother        DILU;'),
    'b2': ('smoother        GaussSeidel;', 'smoother        DILUGaussSeidel;'),
    'b3': ('smoother        GaussSeidel;', 'smoother        nonBlockingGaussSeidel;'),
    'b4': ('smoother        GaussSeidel;', 'smoother        GaussSeidel;\n        interpolateCorrection true;'),
    'b5': ('smoother        GaussSeidel;', 'smoother        GaussSeidel;\n        directSolveCoarsest true;'),
    'b6': ('agglomerator    faceAreaPair;', 'agglomerator    algebraicPair;'),
    'b7': ('mergeLevels     1;', 'mergeLevels     2;'),
}
e = edits[v]
if e is not None:
    assert e[0] in b, e
    b = b.replace(e[0], e[1], 1)
assert 'GAMGdump' in b and 'maxIter         40' in b
open(p, 'w').write(t[:i] + b + t[j:])
