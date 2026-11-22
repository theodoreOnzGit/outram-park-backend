import numpy as np, openmc, openmc.mgxs
# One-group library, no nuclear data. Same numbers as the Rust side
# (tests/bc_vs_openmc_ensembles.rs). Run in the work directory; writes mgxs.h5.
groups = openmc.mgxs.EnergyGroups([0.0, 20.0e6])


def add(lib, name, tot, ab, sc, fis, nufis, chi):
    x = openmc.XSdata(name, groups)
    x.order = 0
    x.set_total([tot])
    x.set_absorption([ab])
    x.set_scatter_matrix(np.array([[[sc]]]))
    x.set_fission([fis])
    x.set_nu_fission([nufis])
    x.set_chi([chi])
    lib.add_xsdata(x)


lib = openmc.MGXSLibrary(groups)
add(lib, 'fuel', 1.0, 0.30, 0.70, 0.20, 0.50, 1.0)   # case A slab
add(lib, 'fuel2', 2.0, 0.60, 1.40, 0.40, 1.00, 1.0)  # case B rod
add(lib, 'mod', 0.5, 0.01, 0.49, 0.0, 0.0, 0.0)      # case B moderator
lib.export_to_hdf5('mgxs.h5')
print('wrote mgxs.h5')
