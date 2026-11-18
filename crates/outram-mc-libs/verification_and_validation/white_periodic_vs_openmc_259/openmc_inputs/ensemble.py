"""OpenMC driver for white_periodic_vs_openmc_259.

usage: ensemble.py <case> <bc> <nseeds> <first_seed> <workdir>
  case: A0.5 | A1.0 | A3.0   slab of thickness T; bc reflective | white
        B1 | B0              rod lattice (B1 off-centre, B0 centred); bc reflective | periodic
<workdir> must contain mgxs.h5 (made by mk.py). Writes
<workdir>/<case>_<bc>_s<first_seed>.json with the per-seed keff (mean over
the active batches of one run)."""
import sys, json, os, time
import numpy as np
import openmc

case, bc, nseeds, first, workdir = (sys.argv[1], sys.argv[2], int(sys.argv[3]),
                                    int(sys.argv[4]), os.path.abspath(sys.argv[5]))
PARTICLES, INACTIVE, ACTIVE = 20000, 20, 100
MGXS = os.path.join(workdir, 'mgxs.h5')


def build(case, bc):
    if case.startswith('A'):
        T = float(case[1:])
        m = openmc.Material(name='fuel')
        m.set_density('macro', 1.0)
        m.add_macroscopic('fuel')
        left = openmc.XPlane(0.0, boundary_type=bc)
        right = openmc.XPlane(T, boundary_type='vacuum')
        ym = openmc.YPlane(-1.0, boundary_type='reflective')
        yp = openmc.YPlane(1.0, boundary_type='reflective')
        zm = openmc.ZPlane(-1.0, boundary_type='reflective')
        zp = openmc.ZPlane(1.0, boundary_type='reflective')
        cell = openmc.Cell(fill=m, region=+left & -right & +ym & -yp & +zm & -zp)
        src = openmc.stats.Box((0.0, -1.0, -1.0), (T, 1.0, 1.0))
        return [m], openmc.Geometry([cell]), src
    cx, cy = {'B1': (1.2, 1.7), 'B0': (3.0, 3.0)}[case]
    fuel = openmc.Material(name='fuel2')
    fuel.set_density('macro', 1.0)
    fuel.add_macroscopic('fuel2')
    mod = openmc.Material(name='mod')
    mod.set_density('macro', 1.0)
    mod.add_macroscopic('mod')
    t = 'periodic' if bc == 'periodic' else 'reflective'
    xm = openmc.XPlane(0.0, boundary_type=t)
    xp = openmc.XPlane(6.0, boundary_type=t)
    ym = openmc.YPlane(0.0, boundary_type=t)
    yp = openmc.YPlane(6.0, boundary_type=t)
    if bc == 'periodic':
        xm.periodic_surface = xp
        xp.periodic_surface = xm
        ym.periodic_surface = yp
        yp.periodic_surface = ym
    zm = openmc.ZPlane(-1.0, boundary_type='reflective')
    zp = openmc.ZPlane(1.0, boundary_type='reflective')
    rod = openmc.ZCylinder(x0=cx, y0=cy, r=0.8)
    box = +xm & -xp & +ym & -yp & +zm & -zp
    c_f = openmc.Cell(fill=fuel, region=box & -rod)
    c_m = openmc.Cell(fill=mod, region=box & +rod)
    src = openmc.stats.Box((cx - 0.8, cy - 0.8, -1.0), (cx + 0.8, cy + 0.8, 1.0))
    return [fuel, mod], openmc.Geometry([c_f, c_m]), src


ks, t0 = [], time.time()
for seed in range(first, first + nseeds):
    wd = os.path.join(workdir, f'run_{case}_{bc}_{seed}')
    os.makedirs(wd, exist_ok=True)
    os.chdir(wd)
    mats, geom, src = build(case, bc)
    ms = openmc.Materials(mats)
    ms.cross_sections = MGXS
    ms.export_to_xml()
    geom.export_to_xml()
    s = openmc.Settings()
    s.energy_mode = 'multi-group'
    s.batches = INACTIVE + ACTIVE
    s.inactive = INACTIVE
    s.particles = PARTICLES
    s.seed = seed
    s.source = openmc.IndependentSource(space=src, constraints={'fissionable': True})
    s.output = {'tallies': False, 'summary': False}
    s.export_to_xml()
    openmc.run(output=False)
    sp = openmc.StatePoint(f'statepoint.{s.batches}.h5')
    ks.append(float(sp.keff.nominal_value))
    sp.close()
json.dump({'case': case, 'bc': bc, 'seeds': list(range(first, first + nseeds)), 'k': ks,
           'particles': PARTICLES, 'inactive': INACTIVE, 'active': ACTIVE,
           'wall_s': time.time() - t0},
          open(os.path.join(workdir, f'{case}_{bc}_s{first}.json'), 'w'))
print(case, bc, 'mean %.6f sem %.6f wall %.1fs' % (np.mean(ks),
      np.std(ks, ddof=1) / np.sqrt(len(ks)), time.time() - t0))
