#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Reference images for the outram-mc-libs port of OpenMC's PYTHON slice plot,
``openmc.Model.plot`` (and ``Universe.plot``, ``Cell.plot``, ``Region.plot``).

Each case below calls OpenMC's own Python function on a small CSG model and
saves the figure with ``plt.savefig(<case>.png)`` (matplotlib defaults), and
saves the id map ``Model.plot`` coloured (``Model.slice_data`` with the same
arguments) as ``<case>_idmap.npy``. The Rust side
(``tests/python_plot_parity.rs``) builds the SAME models, emits one standalone
matplotlib script per case with ``outram_mc_libs::geometry::plot::ModelPlot``,
and ``compare.py`` runs those scripts with the same Python and compares every
pixel and every id-map entry.

The models are the four of ``../../geometry_plotting/openmc_inputs/
make_references.py`` (Godiva; a 3x3 pin lattice in a slab reflector; two
deliberately overlapping spheres with a void cell) plus a three-ring hex
lattice, which checks ``get_unique_universes`` ordering through ``seed=``.

Usage:  python make_references.py <out_dir>

Provenance: OpenMC 0.16.1.dev25, commit d7d3284a1 (MIT, notice in
../../geometry_plotting/openmc_inputs/LICENSE.openmc), C++ library built from
source with libpng; matplotlib 3.11.2, numpy (see README); run 2026-09-26.
This driver is new work, GPL-3.0-only like the rest of the crate.
"""
import os
import sys
from types import SimpleNamespace

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
import openmc


def _mat(mid, name, density):
    m = openmc.Material(material_id=mid, name=name)
    m.add_nuclide("U234", 1.0)  # placeholder: a plot reads no data
    m.set_density("g/cm3", density)
    return m


def godiva():
    heu = _mat(1, "heu", 18.74)
    s = openmc.Sphere(surface_id=1, r=8.7407, boundary_type="vacuum")
    c = openmc.Cell(cell_id=1, fill=heu, region=-s)
    return openmc.Model(geometry=openmc.Geometry([c]),
                        materials=openmc.Materials([heu])), {"heu": heu}


def pin_lattice():
    fuel = _mat(1, "fuel", 10.4)
    clad = _mat(2, "clad", 6.55)
    water = _mat(3, "water", 1.0)
    mats = openmc.Materials([fuel, clad, water])
    fuel_or = openmc.ZCylinder(surface_id=1, r=0.4096)
    clad_or = openmc.ZCylinder(surface_id=2, r=0.475)
    gt_ir = openmc.ZCylinder(surface_id=3, r=0.56)
    gt_or = openmc.ZCylinder(surface_id=4, r=0.60)
    xl = openmc.XPlane(surface_id=5, x0=-1.89)
    xr = openmc.XPlane(surface_id=6, x0=1.89)
    yl = openmc.YPlane(surface_id=7, y0=-1.89)
    yr = openmc.YPlane(surface_id=8, y0=1.89)
    zb = openmc.ZPlane(surface_id=9, z0=-10.0, boundary_type="vacuum")
    zt = openmc.ZPlane(surface_id=10, z0=10.0, boundary_type="vacuum")
    xo_l = openmc.XPlane(surface_id=11, x0=-3.0, boundary_type="vacuum")
    xo_r = openmc.XPlane(surface_id=12, x0=3.0, boundary_type="vacuum")
    yo_l = openmc.YPlane(surface_id=13, y0=-3.0, boundary_type="vacuum")
    yo_r = openmc.YPlane(surface_id=14, y0=3.0, boundary_type="vacuum")
    c = {}
    c[1] = openmc.Cell(cell_id=1, fill=fuel, region=-fuel_or)
    c[2] = openmc.Cell(cell_id=2, fill=clad, region=+fuel_or & -clad_or)
    c[3] = openmc.Cell(cell_id=3, fill=water, region=+clad_or)
    pin = openmc.Universe(universe_id=1, cells=[c[1], c[2], c[3]])
    gt = openmc.Universe(universe_id=2, cells=[
        openmc.Cell(cell_id=4, fill=water, region=-gt_ir),
        openmc.Cell(cell_id=5, fill=clad, region=+gt_ir & -gt_or),
        openmc.Cell(cell_id=6, fill=water, region=+gt_or),
    ])
    lat = openmc.RectLattice(lattice_id=10)
    lat.lower_left = (-1.89, -1.89)
    lat.pitch = (1.26, 1.26)
    lat.universes = [[pin, pin, pin], [pin, gt, pin], [pin, pin, pin]]
    zslab = +zb & -zt
    root = openmc.Universe(universe_id=0, cells=[
        openmc.Cell(cell_id=7, fill=lat, region=+xl & -xr & +yl & -yr & zslab),
        openmc.Cell(cell_id=8, fill=water, region=+xr & -xo_r & +yo_l & -yo_r & zslab),
        openmc.Cell(cell_id=9, fill=water, region=+xo_l & -xl & +yo_l & -yo_r & zslab),
        openmc.Cell(cell_id=10, fill=water, region=+xl & -xr & +yr & -yo_r & zslab),
        openmc.Cell(cell_id=11, fill=water, region=+xl & -xr & +yo_l & -yl & zslab),
    ])
    model = openmc.Model(geometry=openmc.Geometry(root), materials=mats)
    return model, {"fuel": fuel, "clad": clad, "water": water, "pin": pin,
                   "cells": c}


def overlap():
    a = _mat(1, "a", 1.0)
    b = _mat(2, "b", 2.0)
    cm = _mat(3, "c", 3.0)
    mats = openmc.Materials([a, b, cm])
    s1 = openmc.Sphere(surface_id=1, x0=-1.0, r=2.0)
    s2 = openmc.Sphere(surface_id=2, x0=1.0, r=2.0)
    s3 = openmc.Sphere(surface_id=3, y0=3.0, r=0.5)
    box = [
        openmc.XPlane(surface_id=4, x0=-4.0, boundary_type="vacuum"),
        openmc.XPlane(surface_id=5, x0=4.0, boundary_type="vacuum"),
        openmc.YPlane(surface_id=6, y0=-4.0, boundary_type="vacuum"),
        openmc.YPlane(surface_id=7, y0=4.0, boundary_type="vacuum"),
        openmc.ZPlane(surface_id=8, z0=-4.0, boundary_type="vacuum"),
        openmc.ZPlane(surface_id=9, z0=4.0, boundary_type="vacuum"),
    ]
    inbox = +box[0] & -box[1] & +box[2] & -box[3] & +box[4] & -box[5]
    geom = openmc.Geometry([
        openmc.Cell(cell_id=1, fill=a, region=-s1),
        openmc.Cell(cell_id=2, fill=b, region=-s2),
        openmc.Cell(cell_id=3, fill=cm, region=+s1 & +s2 & +s3 & inbox),
        openmc.Cell(cell_id=4, fill=None, region=-s3),
    ])
    return openmc.Model(geometry=geom, materials=mats), {"s1": s1, "s2": s2}


def hex_lattice(orientation="y", axial=False):
    """Three-ring hex lattice of three distinct pin universes,
    inside a z-cylinder. Ring contents are chosen so that the order in which
    get_unique_universes meets the universes (outer ring first, clockwise from
    the top) differs from their id order."""
    m = [_mat(i, f"m{i}", float(i)) for i in (1, 2, 3, 4)]
    mats = openmc.Materials(m)
    pr = openmc.ZCylinder(surface_id=1, r=0.5)
    univ = []
    for k, (inner, outer) in enumerate([(m[0], m[3]), (m[1], m[3]), (m[2], m[3])]):
        univ.append(openmc.Universe(universe_id=k + 1, cells=[
            openmc.Cell(cell_id=10 * (k + 1) + 1, fill=inner, region=-pr),
            openmc.Cell(cell_id=10 * (k + 1) + 2, fill=outer, region=+pr),
        ]))
    a, b, c = univ
    lat = openmc.HexLattice(lattice_id=5)
    lat.orientation = orientation
    level0 = [
        [c, a, b, c, a, b, c, a, b, c, a, b],
        [b, c, a, b, c, a],
        [a],
    ]
    if axial:
        # Two levels, z in [-2, 0] and [0, 2]; level 1 is level 0 with every
        # ring rotated by one position and a different centre.
        level1 = [ring[1:] + ring[:1] for ring in level0[:-1]] + [[b]]
        lat.center = (0.0, 0.0, 0.0)
        lat.pitch = (1.5, 2.0)
        lat.universes = [level0, level1]
    else:
        lat.center = (0.0, 0.0)
        lat.pitch = (1.5,)
        lat.universes = level0
    lat.outer = a
    bound = openmc.ZCylinder(surface_id=2, r=4.2, boundary_type="vacuum")
    region = -bound
    if axial:
        region = region & +openmc.ZPlane(surface_id=3, z0=-2.0, boundary_type="vacuum") \
            & -openmc.ZPlane(surface_id=4, z0=2.0, boundary_type="vacuum")
    root = openmc.Universe(universe_id=0, cells=[
        openmc.Cell(cell_id=1, fill=lat, region=region)])
    return openmc.Model(geometry=openmc.Geometry(root), materials=mats), {}


def source_points():
    """The positions both sides scatter (Model.plot's n_samples, see README)."""
    return [((i % 13 - 6) * 0.9, (i % 7 - 3) * 1.7, (i % 5 - 2) * 0.8)
            for i in range(60)]


def run_case(out_dir, name, model, call, idmap_kwargs):
    plt.close("all")
    call()
    plt.savefig(os.path.join(out_dir, name + ".png"))
    plt.close("all")
    ids, _ = model.slice_data(include_properties=False, **idmap_kwargs)
    np.save(os.path.join(out_dir, name + "_idmap.npy"), ids)
    print("wrote", name)


def main(out_dir):
    os.makedirs(out_dir, exist_ok=True)

    m, _ = godiva()
    run_case(out_dir, "godiva_default", m, lambda: m.plot(), {})

    m, d = pin_lattice()
    run_case(out_dir, "lattice_default", m, lambda: m.plot(), {})
    kw = dict(origin=(0.0, 0.0, 0.0), width=(6.4, 6.4), pixels=(160, 160))
    run_case(out_dir, "lattice_material_colors_legend", m,
             lambda: m.plot(color_by="material", legend=True,
                            colors={d["fuel"]: (255, 0, 0), d["clad"]: "gray",
                                    d["water"]: "LightBlue"}, **kw), kw)
    kw = dict(basis="xz", origin=(0.0, 0.2, 0.0), width=(6.4, 22.0),
              pixels=(100, 300))
    run_case(out_dir, "lattice_xz_seed_legend_mm", m,
             lambda: m.plot(seed=3, legend=True, axis_units="mm", **kw), kw)
    kw = dict(basis="yz", origin=(0.1, 0.0, 5.0), width=(6.4, 6.4), pixels=22500)
    run_case(out_dir, "lattice_yz_outline", m,
             lambda: m.plot(color_by="material", outline=True, **kw), kw)
    kw = dict(origin=(0.0, 0.0, 0.0), width=(6.4, 6.4), pixels=(120, 120))
    run_case(out_dir, "lattice_outline_only", m,
             lambda: m.plot(outline="only", **kw), kw)
    run_case(out_dir, "lattice_imshow_kwargs", m,
             lambda: m.plot(alpha=0.6, interpolation="nearest", **kw), kw)

    pin = d["pin"]
    pm = openmc.Model(geometry=openmc.Geometry(pin), materials=m.materials)
    run_case(out_dir, "universe_default", pm,
             lambda: pin.plot(pixels=(100, 100)), dict(pixels=(100, 100)))
    clad_cell = d["cells"][2]
    cmod = openmc.Model(geometry=openmc.Geometry(openmc.Universe(cells=[clad_cell])),
                        materials=m.materials)
    run_case(out_dir, "cell_default", cmod,
             lambda: clad_cell.plot(pixels=(80, 80)), dict(pixels=(80, 80)))

    m, d = overlap()
    kw = dict(origin=(0.0, 0.0, 0.0), width=(10.0, 10.0), pixels=(100, 100))
    run_case(out_dir, "overlap_show_yellow", m,
             lambda: m.plot(show_overlaps=True, overlap_color="yellow", **kw),
             dict(show_overlaps=True, **kw))
    run_case(out_dir, "overlap_material_seed_legend", m,
             lambda: m.plot(color_by="material", seed=5, legend=True,
                            legend_kwargs={"loc": "lower left"}, **kw), kw)
    region = -d["s1"] | -d["s2"]
    rkw = dict(width=(8.0, 8.0), pixels=(100, 100))
    rcell = openmc.Cell(region=region)
    rmod = openmc.Model(geometry=openmc.Geometry(openmc.Universe(cells=[rcell])),
                        materials=m.materials)
    run_case(out_dir, "region_union", rmod, lambda: region.plot(**rkw), rkw)

    m, _ = godiva()
    pts = source_points()
    m.sample_external_source = lambda n_samples: [
        SimpleNamespace(r=np.array(p)) for p in pts]
    run_case(out_dir, "godiva_source_scatter", m,
             lambda: m.plot(n_samples=len(pts), plane_tolerance=2.0,
                            source_kwargs={"color": "k", "s": 4},
                            pixels=(120, 120)), dict(pixels=(120, 120)))

    m, _ = hex_lattice()
    kw = dict(origin=(0.0, 0.0, 0.0), width=(9.0, 9.0), pixels=(150, 150))
    run_case(out_dir, "hex_seed_legend", m,
             lambda: m.plot(color_by="cell", seed=7, legend=True, **kw), kw)

    m, _ = hex_lattice(orientation="x")
    run_case(out_dir, "hex_x_seed_legend", m,
             lambda: m.plot(color_by="cell", seed=7, legend=True, **kw), kw)

    m, _ = hex_lattice(axial=True)
    kw = dict(origin=(0.0, 0.0, 1.0), width=(9.0, 9.0), pixels=(150, 150))
    run_case(out_dir, "hex3d_xy_upper", m,
             lambda: m.plot(color_by="material", **kw), kw)
    kw = dict(basis="xz", origin=(0.0, 0.0, 0.0), width=(9.0, 4.4), pixels=(180, 88))
    run_case(out_dir, "hex3d_xz", m,
             lambda: m.plot(color_by="material", seed=2, legend=True, **kw), kw)


if __name__ == "__main__":
    main(sys.argv[1])
