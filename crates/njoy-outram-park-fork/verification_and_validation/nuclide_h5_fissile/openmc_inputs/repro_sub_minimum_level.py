"""Reproducer: OpenMC segfaults on a sub-minimum LevelInelastic emission.

LvMin.h5 breaks no format convention: the MT=51 threshold sits exactly on a grid
point, the cross section is exactly zero there and rises smoothly, and the grid
spans 1 keV - 20 MeV (a legal fast-only range).

LevelInelastic::sample returns mass_ratio*(E - threshold) unclamped, so a
collision just above the 101818 eV threshold emits a neutron far below the 1 keV
library minimum. The neutron energy_cutoff defaults to 0.0, so physics.cpp:81
does not kill it, and Material::calculate_neutron_xs computes
log(E/energy_min)/log_spacing as a NEGATIVE array index (material.cpp:832).
"""
import openmc, os
os.environ['OPENMC_CROSS_SECTIONS'] = os.path.abspath('cs.xml')

m = openmc.Material(name='lvmin')
m.add_nuclide('LvMin', 1.0)
m.set_density('atom/b-cm', 0.08)
openmc.Materials([m]).export_to_xml()

s = openmc.Sphere(r=50.0, boundary_type='vacuum')
c = openmc.Cell(fill=m, region=-s)
openmc.Geometry([c]).export_to_xml()

st = openmc.Settings()
st.run_mode = 'fixed source'
st.particles = 20000
st.batches = 5
st.seed = 20260924
# Source just above the MT=51 threshold, where the level law emits sub-minimum.
st.source = openmc.IndependentSource(
    space=openmc.stats.Point((0., 0., 0.)),
    energy=openmc.stats.Uniform(1.02e5, 1.5e5),
)
st.output = {'tallies': False}
st.export_to_xml()
