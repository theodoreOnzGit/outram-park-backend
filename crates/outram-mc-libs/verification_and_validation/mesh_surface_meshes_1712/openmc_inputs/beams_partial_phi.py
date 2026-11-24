"""gh:#1712 finding B -- partial-phi meshes: does OpenMC lose a track that starts above the last phi plane?

Written 2026-11-23 before any run. Same method as beams_mesh_surface.py (deterministic single-neutron
beams, O-16 at 1e-10 atoms/b-cm, vacuum box [-10, 10]^3, one run per site, 1 particle x 1 batch),
on a cylindrical and a spherical mesh whose phi grids cover only part of the circle.
Prediction (from reading mesh.h:649 and the port's pinned test): OpenMC scores nothing for a track
that starts at phi above the last phi plane and later enters the mesh, exactly as the port does.

Usage: beams_partial_phi.py <openmc_exec> <cross_sections.xml> <out_dir>
Writes <out_dir>/partial_phi_sites.txt and <out_dir>/partial_<mesh>_mesh_surface.txt (site bin value, nonzero only).
"""
import sys, os, shutil, tempfile, time
import numpy as np
import openmc

OPENMC, XS, OUT = sys.argv[1:4]
os.makedirs(OUT, exist_ok=True)

# Origins at zero so phi of a point is plain atan2(y, x).
sites = []
# Hand-picked: start at phi = 3.5 (above phi_max = 2.5) and fly towards phi in (0, 1).
for r0, z0 in [(6.0, 0.5), (4.0, -1.0), (7.0, 2.0)]:
    p = np.array([r0 * np.cos(3.5), r0 * np.sin(3.5), z0])
    tgt = np.array([3.0 * np.cos(0.5), 3.0 * np.sin(0.5), z0 * 0.5])
    d = tgt - p; d /= np.linalg.norm(d)
    sites.append(list(p) + list(d) + [1.0e6])
# Controls: start below phi_min (phi = -0.6) and fly into the mesh.
for r0, z0 in [(6.0, 0.5), (4.0, -1.0)]:
    p = np.array([r0 * np.cos(-0.6), r0 * np.sin(-0.6), z0])
    tgt = np.array([3.0 * np.cos(1.8), 3.0 * np.sin(1.8), z0])
    d = tgt - p; d /= np.linalg.norm(d)
    sites.append(list(p) + list(d) + [1.0e6])
rs = np.random.RandomState(17121)
for _ in range(40):
    r = rs.uniform([-9.9, -9.9, -9.9], [9.9, 9.9, 9.9])
    mu = rs.uniform(-1, 1); ph = rs.uniform(0, 2 * np.pi); s = np.sqrt(1 - mu * mu)
    sites.append(list(r) + [s * np.cos(ph), s * np.sin(ph), mu, 1.0e6])
with open(os.path.join(OUT, "partial_phi_sites.txt"), "w") as f:
    for s in sites:
        f.write(" ".join("%.17g" % v for v in s) + "\n")

openmc.reset_auto_ids()
m = openmc.Material(material_id=1); m.add_nuclide("O16", 1.0); m.set_density("atom/b-cm", 1.0e-10)
mats = openmc.Materials([m]); mats.cross_sections = XS
box = openmc.model.RectangularParallelepiped(-10, 10, -10, 10, -10, 10, boundary_type="vacuum")
geom = openmc.Geometry([openmc.Cell(cell_id=1, fill=m, region=-box)])
cyl = openmc.CylindricalMesh(mesh_id=2, r_grid=[0.0, 2.0, 5.0, 8.0], phi_grid=[0.0, 1.0, 2.5],
                             z_grid=[-4.0, 0.0, 4.0])
sph = openmc.SphericalMesh(mesh_id=3, r_grid=[0.0, 2.0, 5.0, 8.0], theta_grid=[0.0, 1.2, 2.0, np.pi],
                           phi_grid=[0.0, 1.0, 2.5])
MESHES = {"cylindrical": cyl, "spherical": sph}
tallies = openmc.Tallies()
for i, (name, mesh) in enumerate(MESHES.items()):
    t = openmc.Tally(tally_id=10 + i, name=name)
    t.filters = [openmc.MeshSurfaceFilter(mesh)]; t.scores = ["current"]
    tallies.append(t)
results = {name: [] for name in MESHES}
t0 = time.time()
for i, s in enumerate(sites):
    wd = tempfile.mkdtemp(prefix="msB_")
    st = openmc.Settings(); st.run_mode = "fixed source"; st.particles = 1; st.batches = 1; st.seed = 1 + i
    st.source = openmc.IndependentSource(space=openmc.stats.Point(tuple(s[:3])),
                                         angle=openmc.stats.Monodirectional(tuple(s[3:6])),
                                         energy=openmc.stats.Discrete([s[6]], [1.0]), particle="neutron")
    mats.export_to_xml(wd + "/materials.xml"); geom.export_to_xml(wd + "/geometry.xml")
    st.export_to_xml(wd + "/settings.xml"); tallies.export_to_xml(wd + "/tallies.xml")
    openmc.run(cwd=wd, openmc_exec=OPENMC, output=False, threads=1)
    sp = openmc.StatePoint(wd + "/statepoint.1.h5")
    for t in tallies:
        results[t.name].append(sp.get_tally(id=t.id).sum.flatten().copy())
    sp.close(); shutil.rmtree(wd)
for name, rows in results.items():
    with open(os.path.join(OUT, "partial_%s_mesh_surface.txt" % name), "w") as f:
        for i, row in enumerate(rows):
            for b, v in enumerate(row):
                if v != 0.0:
                    f.write("%d %d %.17g\n" % (i, b, v))
print("sites", len(sites), "wall_s %.1f" % (time.time() - t0))
