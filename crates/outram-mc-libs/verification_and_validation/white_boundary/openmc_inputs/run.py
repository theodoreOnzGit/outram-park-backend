import sys, numpy as np, openmc
bc = sys.argv[1]
T   = float(sys.argv[2])
mat = openmc.Material(name='fuel'); mat.set_density('macro', 1.0)
mat.add_macroscopic('fuel')
mats = openmc.Materials([mat]); mats.cross_sections = 'mgxs.h5'; mats.export_to_xml()

# Slab: x in [0,T]. LEFT face carries the BC under test, RIGHT face is vacuum.
# The vacuum side makes the angular flux at the left face anisotropic, which is
# the whole point: with a symmetric problem reflective and white agree.
left  = openmc.XPlane(0.0, boundary_type=bc)
right = openmc.XPlane(T,   boundary_type='vacuum')
# y,z reflective => infinite in those directions, so this is a 1-D slab.
ymin = openmc.YPlane(-1.0, boundary_type='reflective'); ymax = openmc.YPlane(1.0, boundary_type='reflective')
zmin = openmc.ZPlane(-1.0, boundary_type='reflective'); zmax = openmc.ZPlane(1.0, boundary_type='reflective')
cell = openmc.Cell(fill=mat, region=+left & -right & +ymin & -ymax & +zmin & -zmax)
openmc.Geometry([cell]).export_to_xml()

s = openmc.Settings()
s.energy_mode = 'multi-group'; s.batches = 220; s.inactive = 20; s.particles = 40000
s.source = openmc.IndependentSource(
    space=openmc.stats.Box((0.0,-1.0,-1.0),(T,1.0,1.0)))
s.seed = 1
s.export_to_xml()
openmc.run(output=False)
sp = openmc.StatePoint(f'statepoint.{s.batches}.h5')
print(f"{bc} k = {sp.keff.nominal_value:.6f} +/- {sp.keff.std_dev:.6f}")
