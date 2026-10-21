"""Dump OpenMC's own per-bin mesh volumes as the reference for GitHub #260.

    /opt/ompy/bin/python dump_mesh_volumes.py

Ravel order 'F' gives the FIRST axis fastest, matching the flat bin index the
Rust port uses (`i + d0*(j + d1*k)`).
"""
import numpy as np, openmc

R   = [0.0, 1.0, 2.5, 4.0]
PHI = [0.0, 1.2, 3.0, 2*np.pi]
Z   = [-2.0, 0.0, 3.0]
TH  = [0.0, 0.7, np.pi]

cyl = openmc.CylindricalMesh(r_grid=R, phi_grid=PHI, z_grid=Z)
sph = openmc.SphericalMesh(r_grid=R, theta_grid=TH, phi_grid=PHI)
rect = openmc.RectilinearMesh()
rect.x_grid = [0.0, 1.0, 3.0, 6.0]
rect.y_grid = [-1.0, 0.5, 2.0]
rect.z_grid = [0.0, 0.25, 4.0]

for name, m in [("CYL", cyl), ("SPH", sph), ("RECT", rect)]:
    v = m.volumes.ravel(order='F')
    print(f"// {name}: {len(v)} bins, OpenMC afa7a14")
    print(f"const OPENMC_{name}: [f64; {len(v)}] = [")
    for x in v:
        print(f"    {x:.15e},")
    print("];")
