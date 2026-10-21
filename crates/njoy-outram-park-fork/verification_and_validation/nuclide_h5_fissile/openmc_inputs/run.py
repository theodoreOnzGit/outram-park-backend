"""OpenMC transporting a fissile nuclide written by njoy-outram-park-fork (gh#304).

SynF's .h5 was written by this workspace's `write_nuclide`: MT=2 elastic with a
tabulated cosine, MT=18 fission with an evaluated Watt spectrum, MT=51 a
discrete inelastic level above a threshold, MT=102 capture, and a total nu-bar.

Every cross section is FLAT, so k_inf is analytic:
    k_inf = nu * Sigma_f / Sigma_a = 2.5 * 2.0 / (2.0 + 0.5) = 2.0
independently of the fission spectrum, the scattering law and the grid. A
reflective boundary removes leakage, so OpenMC must return 2.0.
"""
import openmc, os, sys
os.environ['OPENMC_CROSS_SECTIONS'] = os.path.abspath('cs.xml')

m = openmc.Material(name='synf')
m.add_nuclide('SynF', 1.0)
m.set_density('atom/b-cm', 0.05)
openmc.Materials([m]).export_to_xml()

bc = sys.argv[1] if len(sys.argv) > 1 else 'reflective'
s = openmc.Sphere(r=20.0, boundary_type=bc)
c = openmc.Cell(fill=m, region=-s)
openmc.Geometry([c]).export_to_xml()

st = openmc.Settings()
st.run_mode = 'eigenvalue'
st.particles = 150
st.batches = 45
st.inactive = 15
st.seed = 20260924
st.source = openmc.IndependentSource(space=openmc.stats.Point((0., 0., 0.)))
st.output = {'tallies': False}
st.export_to_xml()

openmc.run(output=False)
sp = openmc.StatePoint('statepoint.45.h5')
k = sp.keff
print(f"OPENMC_K {bc} {k.nominal_value:.6f} +/- {k.std_dev:.6f}")
