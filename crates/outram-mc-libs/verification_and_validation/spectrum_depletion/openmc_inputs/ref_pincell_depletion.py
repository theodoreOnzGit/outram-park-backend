"""Reference for gh:#1626 arm 2: OpenMC's examples/pincell_depletion
(run_depletion.py at e4febe9, model and steps unchanged) on the pincell
library converted from this workspace's NJOY port by pincell_ace_to_hdf5.py.

Model, verbatim from upstream's example: UO2 2.4 wt% 10.29769 g/cc, He gap,
Zircaloy-4, borated water 0.740582 g/cc with c_H_in_H2O; radii 0.39218,
0.40005, 0.45720 cm; pitch 1.25984 cm, reflective; 1000 particles,
100 batches, 10 inactive; chain_simple.xml; CoupledOperator, Predictor,
five 1-day steps at 174 W/cm (2-D: the fuel 'volume' is its area).

Two reaction-rate modes:
  flux   - reaction_rate_mode='flux', CCFE-709 group bounds: the mode the
           port implements (CoupledOperator in depletion::coupled_operator);
  direct - upstream's default (direct reaction-rate tallies), recorded to
           show what the group collapse itself costs.

Usage: python ref_pincell_depletion.py <chain_simple.xml> <out.tsv> <mode> <seed> [<seed> ...]
(run with OPENMC_CROSS_SECTIONS pointing at the converted library).

Output lines, floats as Python repr:
  edge<TAB>i<TAB>eV                          (CCFE-709, once per file)
  comp<TAB>material<TAB>nuclide<TAB>atom/b-cm (initial compositions)
  volume<TAB>v
  k<TAB>mode<TAB>seed<TAB>step<TAB>k<TAB>std
  atoms<TAB>mode<TAB>seed<TAB>step<TAB>nuclide<TAB>atoms
  rate<TAB>mode<TAB>seed<TAB>step<TAB>nuclide<TAB>reaction<TAB>rate
"""
import os
import sys
from math import pi
from pathlib import Path

import numpy as np
import openmc
import openmc.deplete
from openmc.mgxs import GROUP_STRUCTURES

chain_file, out_path, mode, seeds = sys.argv[1], sys.argv[2], sys.argv[3], [int(s) for s in sys.argv[4:]]
out = open(out_path, "a")
openmc.deplete.pool.USE_MULTIPROCESSING = False


def build(seed):
    openmc.reset_auto_ids()
    uo2 = openmc.Material(name='UO2 fuel at 2.4% wt enrichment')
    uo2.set_density('g/cm3', 10.29769)
    uo2.add_element('U', 1., enrichment=2.4)
    uo2.add_element('O', 2.)

    helium = openmc.Material(name='Helium for gap')
    helium.set_density('g/cm3', 0.001598)
    helium.add_element('He', 2.4044e-4)

    zircaloy = openmc.Material(name='Zircaloy 4')
    zircaloy.set_density('g/cm3', 6.55)
    zircaloy.add_element('Sn', 0.014, 'wo')
    zircaloy.add_element('Fe', 0.00165, 'wo')
    zircaloy.add_element('Cr', 0.001, 'wo')
    zircaloy.add_element('Zr', 0.98335, 'wo')

    borated_water = openmc.Material(name='Borated water')
    borated_water.set_density('g/cm3', 0.740582)
    borated_water.add_element('B', 4.0e-5)
    borated_water.add_element('H', 5.0e-2)
    borated_water.add_element('O', 2.4e-2)
    borated_water.add_s_alpha_beta('c_H_in_H2O')

    pitch = 1.25984
    fuel_or = openmc.ZCylinder(r=0.39218, name='Fuel OR')
    clad_ir = openmc.ZCylinder(r=0.40005, name='Clad IR')
    clad_or = openmc.ZCylinder(r=0.45720, name='Clad OR')
    box = openmc.model.RectangularPrism(pitch, pitch, boundary_type='reflective')
    fuel = openmc.Cell(fill=uo2, region=-fuel_or)
    gap = openmc.Cell(fill=helium, region=+fuel_or & -clad_ir)
    clad = openmc.Cell(fill=zircaloy, region=+clad_ir & -clad_or)
    water = openmc.Cell(fill=borated_water, region=+clad_or & -box)
    geometry = openmc.Geometry([fuel, gap, clad, water])
    uo2.volume = pi * fuel_or.r**2

    settings = openmc.Settings()
    settings.batches = 100
    settings.inactive = 10
    settings.particles = 1000
    settings.seed = seed
    bounds = [-0.62992, -0.62992, -1, 0.62992, 0.62992, 1]
    uniform_dist = openmc.stats.Box(bounds[:3], bounds[3:])
    settings.source = openmc.IndependentSource(space=uniform_dist, constraints={'fissionable': True})
    entropy_mesh = openmc.RegularMesh()
    entropy_mesh.lower_left = [-0.39218, -0.39218, -1.e50]
    entropy_mesh.upper_right = [0.39218, 0.39218, 1.e50]
    entropy_mesh.dimension = [10, 10, 1]
    settings.entropy_mesh = entropy_mesh
    model = openmc.Model(geometry=geometry, settings=settings)
    return model, [uo2, helium, zircaloy, borated_water]


edges = GROUP_STRUCTURES['CCFE-709']
first = os.path.getsize(out_path) == 0 if os.path.exists(out_path) else True
for seed in seeds:
    model, mats = build(seed)
    if first:
        for i, e in enumerate(edges):
            out.write(f"edge\t{i}\t{float(e)!r}\n")
        for m in mats:
            for nuc, d in m.get_nuclide_atom_densities().items():
                out.write(f"comp\t{m.name}\t{nuc}\t{float(d)!r}\n")
        out.write(f"volume\t{mats[0].volume!r}\n")
        first = False
    if mode == "flux":
        op = openmc.deplete.CoupledOperator(model, chain_file, reaction_rate_mode="flux",
                                            reaction_rate_opts={"energies": edges})
    else:
        op = openmc.deplete.CoupledOperator(model, chain_file)
    openmc.deplete.PredictorIntegrator(op, [1.0] * 5, 174.0, timestep_units='d').integrate(write_rates=True)
    res = openmc.deplete.Results(op.output_dir / "depletion_results.h5")
    for step in range(len(res)):
        s = res[step]
        mid = list(s.index_mat.keys())[0]
        k = np.atleast_2d(s.k)[0]
        out.write(f"k\t{mode}\t{seed}\t{step}\t{float(k[0])!r}\t{float(k[1])!r}\n")
        for nuc in s.index_nuc:
            out.write(f"atoms\t{mode}\t{seed}\t{step}\t{nuc}\t{float(s[mid, nuc])!r}\n")
        rr = s.rates[0]
        for nuc, inuc in rr.index_nuc.items():
            for rx, irx in rr.index_rx.items():
                out.write(f"rate\t{mode}\t{seed}\t{step}\t{nuc}\t{rx}\t{float(rr[inuc, irx])!r}\n")
    out.flush()
    print(f"{mode} seed {seed} done", flush=True)
out.close()
