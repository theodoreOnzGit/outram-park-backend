#!/usr/bin/env python3
"""Write the `tc_driver` stdin for the 15 cases of PREDICTIONS.md.

usage: python3 -I gen_inputs.py <thermochimica clone>/data <tol1> > cases.txt
`tol1` is 0 for upstream's default (1e-5) or e.g. 1e-10 for the tightened run.
"""
import sys

CASES = [
    ("CO.dat", 1000, 1, [(6, 2), (8, 1)]),
    ("CO.dat", 300, 1, [(6, 1), (8, 1)]),
    ("CO.dat", 2500, 1, [(6, 1), (8, 1)]),
    ("CO.dat", 1500, 10, [(6, 3), (8, 2)]),
    ("HO.dat", 300, 1, [(1, 2), (8, 1)]),
    ("HO.dat", 1000, 1, [(1, 2), (8, 1)]),
    ("HO.dat", 3000, 1, [(1, 2), (8, 1)]),
    ("HO.dat", 2000, 0.01, [(1, 4), (8, 1)]),
    ("WAuArO-1.dat", 1455, 1, [(74, 1.95), (79, 1), (18, 2), (8, 10)]),
    ("WAuArO-2.dat", 1000, 1, [(74, 1), (79, 3), (18, 5), (8, 2)]),
    ("WAuArNeO-1.dat", 2452, 1, [(74, 1.95), (79, 1), (18, 2), (8, 10), (10, 10)]),
    ("WAuArNeO-2.dat", 900, 2, [(74, 20), (79, 2), (18, 7), (8, 5), (10, 1)]),
    ("CsI-Pham.dat", 400, 1e-5, [(53, 1), (55, 1)]),
    ("CsI-Pham.dat", 1100, 1, [(53, 1), (55, 1)]),
    ("CsI-Pham.dat", 800, 1, [(53, 1), (55, 3)]),
]

data, tol = sys.argv[1], sys.argv[2]
for f, t, p, els in CASES:
    print(f"'{data}/{f}' {t} {p} {tol} {len(els)}")
    for z, m in els:
        print(z, m)
