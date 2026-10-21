import numpy as np, openmc, openmc.mgxs
# One-group library: scattering + fission, no data files needed.
groups = openmc.mgxs.EnergyGroups([0.0, 20.0e6])
xsd = openmc.XSdata('fuel', groups)
xsd.order = 0
xsd.set_total([1.0])
xsd.set_absorption([0.30])
xsd.set_scatter_matrix(np.array([[[0.70]]]))
xsd.set_fission([0.20])
xsd.set_nu_fission([0.50])
xsd.set_chi([1.0])
lib = openmc.MGXSLibrary(groups); lib.add_xsdata(xsd); lib.export_to_hdf5('mgxs.h5')
print("wrote mgxs.h5")
