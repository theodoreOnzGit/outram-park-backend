"""gh:#1712 -- mesh-surface currents on rectilinear, cylindrical and spherical meshes.

*** NOT YET RUN (2026-11-22). *** Written beside the port so the reference side is
fixed before any OpenMC output exists; the OpenMC build is being prepared
separately. Record the OpenMC version/commit, the library and the wall time in
../README.md when it is run.

Method (as gh:#261 Experiment A, `../../tally_filters_vs_openmc/openmc_inputs/exp_a_beams.py`):
deterministic single-neutron beams through a near-transparent box (O-16 at 1e-10
atoms/b-cm, CE mode, so one straight track per run), one run per source site,
1 particle x 1 batch, vacuum on every face. Each run tallies `current` on a
MeshSurfaceFilter over three meshes, with origins off zero. The Rust side builds
the same tracks (site -> box exit) and calls `MeshSurfaceFilter::matches`; the
comparison is bin for bin, and the prediction is identical bins except where a
track ends within ~1e-15 cm of a mesh surface (README, prediction 5).

Usage: beams_mesh_surface.py <openmc_exec> <cross_sections.xml> <out_dir>
Writes <out_dir>/sites.txt and <out_dir>/<mesh>_mesh_surface.txt (columns: site bin value).
"""
import sys, os, shutil, tempfile, time
import numpy as np
import openmc

OPENMC, XS, OUT = sys.argv[1:4]
os.makedirs(OUT, exist_ok=True)

rs = np.random.RandomState(1712)
sites = []
for _ in range(64):
    r = rs.uniform([-9.9, -9.9, -9.9], [9.9, 9.9, 9.9])
    mu = rs.uniform(-1, 1); ph = rs.uniform(0, 2 * np.pi); s = np.sqrt(1 - mu * mu)
    sites.append(list(r) + [s * np.cos(ph), s * np.sin(ph), mu, 1.0e6])
# Hand-picked: a chord through the cylinder axis, a diagonal near the sphere centre.
sites.append([-9.5, 0.5, 1.0, 1.0, 0.0, 0.0, 1.0e6])
d = np.ones(3) / np.sqrt(3.0)
sites.append([-9.0, -8.6, -8.0] + list(d) + [1.0e6])
with open(os.path.join(OUT, "sites.txt"), "w") as f:
    for s in sites:
        f.write(" ".join("%.17g" % v for v in s) + "\n")

openmc.reset_auto_ids()
m = openmc.Material(material_id=1); m.add_nuclide("O16", 1.0); m.set_density("atom/b-cm", 1.0e-10)
mats = openmc.Materials([m]); mats.cross_sections = XS
box = openmc.model.RectangularParallelepiped(-10, 10, -10, 10, -10, 10, boundary_type="vacuum")
root = openmc.Cell(cell_id=1, fill=m, region=-box)
geom = openmc.Geometry([root])

rect = openmc.RectilinearMesh(mesh_id=1)
rect.x_grid = [-8.0, -5.0, -1.0, 0.5, 4.0, 8.0]
rect.y_grid = [-7.0, -2.0, 0.0, 3.0, 7.0]
rect.z_grid = [-6.0, -1.0, 2.5, 6.0]
cyl = openmc.CylindricalMesh(mesh_id=2, r_grid=[0.0, 1.5, 3.0, 5.0, 8.0],
                             phi_grid=[0.0, 1.0, 2.5, 4.0, 2 * np.pi],
                             z_grid=[-6.0, -2.0, 0.0, 3.0, 6.0], origin=(0.5, 0.5, 1.0))
sph = openmc.SphericalMesh(mesh_id=3, r_grid=[0.0, 1.0, 3.0, 5.5, 8.0],
                           theta_grid=[0.0, 0.7, np.pi / 2, 2.4, np.pi],
                           phi_grid=[0.0, 2.0, 4.5, 2 * np.pi], origin=(-1.0, 0.4, 1.0))
MESHES = {"rectilinear": rect, "cylindrical": cyl, "spherical": sph}
tallies = openmc.Tallies()
for i, (name, mesh) in enumerate(MESHES.items()):
    t = openmc.Tally(tally_id=10 + i, name=name)
    t.filters = [openmc.MeshSurfaceFilter(mesh)]; t.scores = ["current"]
    tallies.append(t)

results = {name: [] for name in MESHES}
t0 = time.time()
for i, s in enumerate(sites):
    wd = tempfile.mkdtemp(prefix="ms1712_")
    st = openmc.Settings(); st.run_mode = "fixed source"; st.particles = 1; st.batches = 1
    st.seed = 1 + i
    st.source = openmc.IndependentSource(
        space=openmc.stats.Point(tuple(s[:3])),
        angle=openmc.stats.Monodirectional(tuple(s[3:6])),
        energy=openmc.stats.Discrete([s[6]], [1.0]), particle="neutron")
    mats.export_to_xml(wd + "/materials.xml"); geom.export_to_xml(wd + "/geometry.xml")
    st.export_to_xml(wd + "/settings.xml"); tallies.export_to_xml(wd + "/tallies.xml")
    openmc.run(cwd=wd, openmc_exec=OPENMC, output=False, threads=1)
    sp = openmc.StatePoint(wd + "/statepoint.1.h5")
    for t in tallies:
        results[t.name].append(sp.get_tally(id=t.id).sum.flatten().copy())
    sp.close(); shutil.rmtree(wd)
wall = time.time() - t0
for name, rows in results.items():
    with open(os.path.join(OUT, "%s_mesh_surface.txt" % name), "w") as f:
        for i, row in enumerate(rows):
            for b, v in enumerate(row):
                f.write("%d %d %.17g\n" % (i, b, v))
print("sites", len(sites), "wall_s %.1f" % wall)
