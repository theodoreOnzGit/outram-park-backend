import os, openmc
os.environ['OPENMC_CROSS_SECTIONS'] = os.path.abspath('cs.xml')

mat = openmc.Material(name='syn')
mat.add_nuclide('Syn1', 1.0)
mat.set_density('atom/b-cm', 0.05)
mats = openmc.Materials([mat]); mats.cross_sections = os.path.abspath('cs.xml')
mats.export_to_xml()

R = 10.0
sph = openmc.Sphere(r=R, boundary_type='vacuum')
cell = openmc.Cell(fill=mat, region=-sph)
openmc.Geometry([cell]).export_to_xml()

tally = openmc.Tally(name='flux')
tally.filters = [openmc.CellFilter(cell)]
tally.scores = ['flux', 'absorption']
openmc.Tallies([tally]).export_to_xml()

s = openmc.Settings()
s.run_mode = 'fixed source'
s.particles = 20000
s.batches = 10
s.source = openmc.IndependentSource(
    space=openmc.stats.Point((0.0, 0.0, 0.0)),
    energy=openmc.stats.Discrete([2.0e6], [1.0]))
s.export_to_xml()

openmc.run(output=False)
with openmc.StatePoint('statepoint.10.h5') as sp:
    t = sp.get_tally(name='flux')
    m = t.mean.ravel(); d = t.std_dev.ravel()
    print(f"OPENMC flux       = {m[0]:.6f} +/- {d[0]:.6f}  (cm per source neutron)")
    print(f"OPENMC absorption = {m[1]:.6f} +/- {d[1]:.6f}  (per source neutron)")
