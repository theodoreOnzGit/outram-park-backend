#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 OUTRAM PARK contributors
"""Reference for gh:#1389: OpenMC's own composite surfaces, evaluated by
OpenMC's own Python API (openmc/model/surface_composite.py at 8f79d9d0).

For every case below this script builds the composite with `openmc.model`,
writes the coefficients of its component surfaces (in `_surface_names`
order; general Cone/Cylinder as their Quadric coefficients, exactly what
`to_xml_element` hands the C++ code), and classifies `n` pseudo-random points
in the case's box as inside (`point in -composite`) or outside. A point within
1e-9 of any surface the region uses is written `x` and not compared: its side
is round-off, not geometry. `+composite` is checked to be the complement of
`-composite` on every kept point.

The points come from splitmix64 (seeded per case) mapped to the box with
plain IEEE double operations, so `tests/composite_surfaces_vs_openmc.rs`
regenerates exactly the same doubles.

Run from anywhere with numpy, scipy and matplotlib installed, OPENMC_PY set
to the root of an OpenMC 8f79d9d0 checkout (its `openmc/` Python package is
imported directly; the C library is not needed):

    OPENMC_PY=.../upstream_source/OpenMC python3 make_reference.py > ../reference.txt

OpenMC reads its own version from installed package metadata; when importing
from a source tree there is none, so set OPENMC_FAKE_META to a directory
holding an `openmc-<v>.dist-info/METADATA` stub (Name/Version lines only).
"""

import os
import sys

if os.environ.get("OPENMC_FAKE_META"):
    sys.path.insert(0, os.environ["OPENMC_FAKE_META"])
sys.path.insert(0, os.environ["OPENMC_PY"])

import openmc  # noqa: E402
import openmc.model as m  # noqa: E402

N_POINTS = 2000
DROP = 1e-9
MASK = (1 << 64) - 1


def splitmix64(state):
    state = (state + 0x9E3779B97F4A7C15) & MASK
    z = state
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
    return state, z ^ (z >> 31)


def points(seed, lo, hi, n):
    state = seed
    out = []
    for _ in range(n):
        p = []
        for k in range(3):
            state, z = splitmix64(state)
            u = (z >> 11) * (1.0 / 9007199254740992.0)
            p.append(lo[k] + (hi[k] - lo[k]) * u)
        out.append(tuple(p))
    return out


ARROW = [(0, 0), (3, 0), (3, -1), (5, 1.5), (3, 4), (3, 3), (0, 3), (1, 1.5)]
L_CW = [(0, 0), (0, 2), (1, 2), (1, 1), (2, 1), (2, 0)]
RZ = [(0.5, 0), (3, 0), (3, 1), (2, 1.5), (3, 3), (1, 2.5), (0.5, 3)]
# An rz polygon with an edge on the axis, (0,0)-(0,2), is refused upstream:
# openmc.ZCylinder(r=-0.0) fails its r > 0 check. The port refuses it too
# (geometry::composite::tests::upstream_value_errors_are_refused).

# name, constructor, box lower, box upper
CASES = [
    ("sector_z", lambda: m.CylinderSector(1, 3, 10, 80, center=(0.5, -0.5), axis="z"),
     (-3, -4, -1), (4, 3, 1)),
    ("sector_z_reflex", lambda: m.CylinderSector(1, 3, -30, 250, center=(0, 0), axis="z"),
     (-3.5, -3.5, -1), (3.5, 3.5, 1)),
    ("sector_y", lambda: m.CylinderSector(1, 3, 0, 90, center=(0.2, 0.3), axis="y"),
     (-3, -1, -3), (3.5, 1, 3.5)),
    ("sector_x", lambda: m.CylinderSector(1, 3, 20, 130, center=(0.2, 0.3), axis="x"),
     (-1, -3, -3), (1, 3.5, 3.5)),
    ("octagon_z", lambda: m.IsogonalOctagon((0.5, -0.25), 2.0, 2.2, axis="z"),
     (-2, -2.5, -1), (3, 2.5, 1)),
    ("octagon_y", lambda: m.IsogonalOctagon((0.5, -0.25), 2.0, 2.2, axis="y"),
     (-2, -1, -2.5), (3, 1, 2.5)),
    ("octagon_x", lambda: m.IsogonalOctagon((0.5, -0.25), 2.0, 2.2, axis="x"),
     (-1, -2, -2.5), (1, 3, 2.5)),
    ("cone_x_up", lambda: m.XConeOneSided(x0=0.5, y0=0.0, z0=0.0, r2=0.25, up=True),
     (-1.5, -1.5, -1.5), (2.5, 1.5, 1.5)),
    ("cone_y_down", lambda: m.YConeOneSided(x0=0.0, y0=0.5, z0=0.2, r2=0.36, up=False),
     (-1.5, -1.5, -1.3), (1.5, 2.5, 1.7)),
    ("cone_z_up", lambda: m.ZConeOneSided(x0=0.3, y0=-0.2, z0=-1.0, r2=1.0, up=True),
     (-2, -2.5, -2), (2.6, 2.1, 1)),
    ("rect_z_round", lambda: m.RectangularPrism(4, 3, "z", (0.5, -0.5), corner_radius=0.5),
     (-2, -2.5, -1), (3, 1.5, 1)),
    ("rect_y_round", lambda: m.RectangularPrism(4, 3, "y", (0.5, -0.5), corner_radius=0.7),
     (-2, -1, -2.5), (3, 1, 1.5)),
    ("rect_x_round", lambda: m.RectangularPrism(2, 5, "x", (0.0, 1.0), corner_radius=0.9),
     (-1, -1.5, -2), (1, 1.5, 4)),
    ("rect_z_square", lambda: m.RectangularPrism(4, 3, "z", (0.5, -0.5)),
     (-2, -2.5, -1), (3, 1.5, 1)),
    ("cruciform_z_inc", lambda: m.CruciformPrism([1, 2, 3], center=(0.5, 0.5), axis="z"),
     (-3, -3, -1), (4, 4, 1)),
    ("cruciform_x_dec", lambda: m.CruciformPrism([2.5, 1.5, 0.5], center=(0, 0), axis="x"),
     (-1, -3, -3), (1, 3, 3)),
    ("frustum_oblique", lambda: m.ConicalFrustum((1, 2, 3), (1, 1, 2), 2.0, 1.0),
     (-1, 0, 1.5), (3.5, 4.5, 6.5)),
    ("frustum_cylinder_down", lambda: m.ConicalFrustum((0, 0, 1), (0, 0, -2), 1.5, 1.5),
     (-2, -2, -1.5), (2, 2, 1.5)),
    ("frustum_z_expand", lambda: m.ConicalFrustum((0.5, 0, 0), (0, 0, 3), 1.0, 2.0),
     (-2, -2.5, -0.5), (3, 2.5, 3.5)),
    ("frustum_x", lambda: m.ConicalFrustum((0, 0, 0), (2, 0, 0), 0.5, 1.5),
     (-0.5, -2, -2), (2.5, 2, 2)),
    ("vessel_z", lambda: m.Vessel(2.0, -1.0, 3.0, 1.0, 1.5, center=(0.5, 0.5), axis="z"),
     (-2, -2, -2.5), (3, 3, 5)),
    ("vessel_x", lambda: m.Vessel(1.5, 0.0, 2.0, 0.5, 2.0, center=(0.0, -0.5), axis="x"),
     (-1, -2.5, -2), (4.5, 1.5, 2)),
    ("vessel_y", lambda: m.Vessel(1.0, -2.0, 2.0, 1.0, 1.0, center=(1.0, 0.0), axis="y"),
     (-0.5, -3.5, -1.5), (2.5, 3.5, 1.5)),
    ("polygon_xy_arrow", lambda: m.Polygon(ARROW, basis="xy"),
     (-0.5, -1.5, -1), (5.5, 4.5, 1)),
    ("polygon_yz_arrow", lambda: m.Polygon(ARROW, basis="yz"),
     (-1, -0.5, -1.5), (1, 5.5, 4.5)),
    ("polygon_xz_l_cw", lambda: m.Polygon(L_CW, basis="xz"),
     (-0.5, -1, -0.5), (2.5, 1, 2.5)),
    ("polygon_rz", lambda: m.Polygon(RZ, basis="rz"),
     (-3.5, -3.5, -0.5), (3.5, 3.5, 3.5)),
]


def coeffs(s):
    """(kind, coefficients) as the C++ side receives the surface."""
    if isinstance(s, (openmc.Cone, openmc.Cylinder)) and type(s) in (openmc.Cone, openmc.Cylinder):
        return "quadric", s._get_base_coeffs()
    kind = type(s).__name__.lower()
    return kind, tuple(getattr(s, k) for k in s._coeff_keys)


def main():
    out = sys.stdout
    out.write("# gh:#1389 composite-surface reference, OpenMC Python 8f79d9d0\n")
    out.write(f"# n={N_POINTS} drop=|f|<{DROP}\n")
    for k, (name, make, lo, hi) in enumerate(CASES):
        comp = make()
        seed = 1389 * 1000 + k
        out.write("case %s %r %r %r %r %r %r %d %d\n"
                  % (name, *[float(v) for v in lo], *[float(v) for v in hi], seed, N_POINTS))
        if not isinstance(comp, m.Polygon):
            for s in comp.component_surfaces:
                kind, c = coeffs(s)
                out.write("surface %s %s\n" % (kind, " ".join(repr(float(v)) for v in c)))
        inside = -comp
        outside = +comp
        surfs = list(inside.get_surfaces().values())
        chars = []
        for p in points(seed, lo, hi, N_POINTS):
            if any(abs(s.evaluate(p)) < DROP for s in surfs):
                chars.append("x")
                continue
            a = p in inside
            if a == (p in outside):
                raise SystemExit(f"{name}: +composite is not the complement at {p}")
            chars.append("1" if a else "0")
        out.write("inside %s\n" % "".join(chars))


if __name__ == "__main__":
    main()
