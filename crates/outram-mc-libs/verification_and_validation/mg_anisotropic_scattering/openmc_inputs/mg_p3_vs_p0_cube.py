#!/usr/bin/env python
"""Reference deck for GitHub #265 — OpenMC multigroup mode, P3 vs P0.

Builds the SAME 2-group macroscopic set that
`crates/outram-mc-libs/examples/mg_scatter_anisotropy_ablation.rs` uses, in
both angular representations, and runs OpenMC's own MG transport on the same
two cubes.

Multigroup mode reads no nuclear data, so this runs with no
`cross_sections.xml` and no ENDF/ACE library present — which is what makes a
genuine code-to-code comparison possible in this container.

Usage:  python mg_p3_vs_p0_cube.py
"""
import os
import sys
import numpy as np
import openmc
import openmc.mgxs

# ── the 2-group set (cm^-1); index 0 = fast, index 1 = thermal ──────────────
TOTAL = [0.080, 0.180]
ABSORPTION = [0.010, 0.080]
FISSION = [0.0032, 0.040]
NU_FISSION = [0.008, 0.100]
CHI = [1.0, 0.0]
SCATTER = [[0.050, 0.020],   # 0 -> 0, 0 -> 1
           [0.000, 0.100]]   # 1 -> 0, 1 -> 1
P3 = [1.0, 0.3, 0.1, 0.03]   # normalised Legendre kernel, <mu> = 0.3

GROUPS = openmc.mgxs.EnergyGroups([1e-5, 1.0e3, 20.0e6])


def make_library(order):
    """order = 3 -> the P3 set; order = 0 -> the isotropic ablation."""
    xsdata = openmc.XSdata('mat', GROUPS)
    xsdata.order = order
    xsdata.scatter_format = 'legendre'
    xsdata.set_total(np.array(TOTAL))
    xsdata.set_absorption(np.array(ABSORPTION))
    xsdata.set_fission(np.array(FISSION))
    xsdata.set_nu_fission(np.array(NU_FISSION))
    xsdata.set_chi(np.array(CHI))

    scatter = np.zeros((2, 2, order + 1))
    for gin in range(2):
        for gout in range(2):
            for l in range(order + 1):
                scatter[gin, gout, l] = SCATTER[gin][gout] * P3[l]
    xsdata.set_scatter_matrix(scatter)

    lib = openmc.MGXSLibrary(GROUPS)
    lib.add_xsdata(xsdata)
    name = f'mgxs_p{order}.h5'
    lib.export_to_hdf5(name)
    return name


def cube(a, bc):
    xs = [openmc.XPlane(-a, boundary_type=bc), openmc.XPlane(a, boundary_type=bc)]
    ys = [openmc.YPlane(-a, boundary_type=bc), openmc.YPlane(a, boundary_type=bc)]
    zs = [openmc.ZPlane(-a, boundary_type=bc), openmc.ZPlane(a, boundary_type=bc)]
    return +xs[0] & -xs[1] & +ys[0] & -ys[1] & +zs[0] & -zs[1]


def run(order, a, bc, tag):
    lib_name = make_library(order)

    mat = openmc.Material(name='mat')
    mat.set_density('macro', 1.0)
    mat.add_macroscopic(openmc.Macroscopic('mat'))
    materials = openmc.Materials([mat])
    materials.cross_sections = lib_name
    materials.export_to_xml()

    cell = openmc.Cell(fill=mat, region=cube(a, bc))
    openmc.Geometry([cell]).export_to_xml()

    settings = openmc.Settings()
    settings.energy_mode = 'multi-group'
    settings.batches = 100
    settings.inactive = 30
    settings.particles = 20000
    settings.source = openmc.IndependentSource(
        space=openmc.stats.Box((-a, -a, -a), (a, a, a)))
    settings.export_to_xml()

    openmc.run(output=False)
    with openmc.StatePoint(f'statepoint.{settings.batches}.h5') as sp:
        k = sp.keff
    print(f'{tag}: k = {k.nominal_value:.5f} +/- {k.std_dev:.5f}')
    return k.nominal_value, k.std_dev


if __name__ == '__main__':
    out = {}
    out['bare_p3'] = run(3, 35.0, 'vacuum', 'BARE  a=35 vacuum      P3')
    out['bare_p0'] = run(0, 35.0, 'vacuum', 'BARE  a=35 vacuum      P0')
    out['refl_p3'] = run(3, 10.0, 'reflective', 'CTRL  a=10 reflective  P3')
    out['refl_p0'] = run(0, 10.0, 'reflective', 'CTRL  a=10 reflective  P0')

    for pair, label in [(('bare_p3', 'bare_p0'), 'bare cube'),
                        (('refl_p3', 'refl_p0'), 'reflective control')]:
        (ka, sa), (ki, si) = out[pair[0]], out[pair[1]]
        d = ka - ki
        s = (sa ** 2 + si ** 2) ** 0.5
        print(f'{label}: paired worth = {1e5*d:+.0f} +/- {1e5*s:.0f} pcm '
              f'({abs(d/s):.1f} sigma)')
