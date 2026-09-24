import openmc, os
os.environ['OPENMC_CROSS_SECTIONS'] = os.path.abspath('cs.xml')
# Energy bins straddling the 1 keV library minimum: anything scoring BELOW it is
# a particle transported with a negative cross-section grid index.
ef = openmc.EnergyFilter([1e-5, 1e-3, 1e-1, 1e1, 1e2, 9.99e2, 1e3, 1e4, 1e5, 2e7])
t = openmc.Tally(name='flux')
t.filters = [ef]
t.scores = ['flux']
openmc.Tallies([t]).export_to_xml()
