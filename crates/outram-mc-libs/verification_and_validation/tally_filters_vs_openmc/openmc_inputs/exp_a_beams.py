"""gh:#261 Experiment A -- deterministic single-neutron beams, OpenMC 0.15.3.

Provenance: OpenMC 0.15.3, commit 27e38e894697bb32a1dac7848d2618818b6b8daf (built from source,
no MPI); data: ENDF/B-VIII.0 O-16 via NJOY2016 at 293.6 K (make_o16_lib.py), ONE nuclide, CE mode;
1 particle x 1 batch per run, one run per source site (mono-directional, mono-energetic, so the
track is fully determined; both materials are 1e-10 / 2e-10 atoms/b-cm O-16, i.e. transparent).

Usage: exp_a_beams.py <openmc_exec> <cross_sections.xml> <out_dir> [nx,ny,nz prefix]   (A1: no extras; A2: "5,3,1" expA2)
Writes <out_dir>/sites.txt and <out_dir>/<prefix>_<tally>.txt  (columns: site bin value).
Geometry: 3x1x1 lattice, pitch 6 cm (x in [-9,9], y,z in [-3,3], vacuum on every face), tile =
sphere r=2 cm (cell A, material M1) inside cell B (material M2).
"""
import sys, os, shutil, tempfile, time
import numpy as np
import openmc

OPENMC, XS, OUT = sys.argv[1:4]
# optional: mesh dimension "nx,ny,nz" and output prefix (A2: "5,3,1" "expA2")
DIM = tuple(int(v) for v in (sys.argv[4] if len(sys.argv) > 4 else "6,2,1").split(","))
PREFIX = sys.argv[5] if len(sys.argv) > 5 else "expA"
ONLY_MESH = len(sys.argv) > 4
os.makedirs(OUT, exist_ok=True)

# ---- sites (fixed seed; two hand-picked beams appended) ----------------------------------------
rs = np.random.RandomState(261)
sites = []
for _ in range(16):
    r = rs.uniform([-8.9, -2.9, -2.9], [8.9, 2.9, 2.9])
    mu = rs.uniform(-1, 1); ph = rs.uniform(0, 2 * np.pi); s = np.sqrt(1 - mu * mu)
    u = np.array([s * np.cos(ph), s * np.sin(ph), mu])
    e = 10 ** rs.uniform(4.0, 7.0)
    sites.append(list(r) + list(u) + [e])
sites.append([-8.5, 0.5, 0.25, 1.0, 0.0, 0.0, 1.0e6])
d = np.ones(3) / np.sqrt(3.0)
sites.append([-8.5, -2.5, -2.5, d[0], d[1], d[2], 3.0e5])
with open(os.path.join(OUT, "sites.txt"), "w") as f:
    for s in sites:
        f.write(" ".join("%.17g" % v for v in s) + "\n")

# ---- model --------------------------------------------------------------------------------------
openmc.reset_auto_ids()
m1 = openmc.Material(material_id=1); m1.add_nuclide("O16", 1.0); m1.set_density("atom/b-cm", 1.0e-10)
m2 = openmc.Material(material_id=2); m2.add_nuclide("O16", 1.0); m2.set_density("atom/b-cm", 2.0e-10)
mats = openmc.Materials([m1, m2]); mats.cross_sections = XS

px = [openmc.XPlane(-9.0, surface_id=1, boundary_type="vacuum"), openmc.XPlane(9.0, surface_id=2, boundary_type="vacuum")]
py = [openmc.YPlane(-3.0, surface_id=3, boundary_type="vacuum"), openmc.YPlane(3.0, surface_id=4, boundary_type="vacuum")]
pz = [openmc.ZPlane(-3.0, surface_id=5, boundary_type="vacuum"), openmc.ZPlane(3.0, surface_id=6, boundary_type="vacuum")]
sph = openmc.Sphere(r=2.0, surface_id=10)
cA = openmc.Cell(cell_id=1, fill=m1, region=-sph)
cB = openmc.Cell(cell_id=2, fill=m2, region=+sph)
tile = openmc.Universe(universe_id=1, cells=[cA, cB])
lat = openmc.RectLattice(lattice_id=5)
lat.lower_left = (-9.0, -3.0, -3.0); lat.pitch = (6.0, 6.0, 6.0)
lat.universes = [[[tile, tile, tile]]]  # [z][y][x]
root = openmc.Cell(cell_id=100, fill=lat, region=+px[0] & -px[1] & +py[0] & -py[1] & +pz[0] & -pz[1])
geom = openmc.Geometry([root]); geom.root_universe = openmc.Universe(universe_id=0, cells=[root])

mesh = openmc.RegularMesh(mesh_id=1)
mesh.lower_left = (-9.0, -3.0, -3.0); mesh.upper_right = (9.0, 3.0, 3.0); mesh.dimension = DIM

surf_all = [sph, px[0], px[1], py[0], py[1], pz[0], pz[1]]
mm_pairs = [(e, mid) for e in range(DIM[0] * DIM[1] * DIM[2]) for mid in (1, 2)]
ef_E = [1.0e3, 1.0e5, 1.0e6, 1.0e7, 2.0e7]; ef_y = [0.5, 2.0, 3.0, 1.0, 0.2]
mu_edges = list(np.linspace(-1.0, 1.0, 9))

def T(tid, name, filters, score):
    t = openmc.Tally(tally_id=tid, name=name); t.filters = filters; t.scores = [score]; return t
ALL = [
    T(1, "distribcell_A", [openmc.DistribcellFilter(cA)], "flux"),
    T(2, "cell_instance", [openmc.CellInstanceFilter([(cA, 0), (cA, 1), (cA, 2), (cB, 0), (cB, 1), (cB, 2)])], "flux"),
    T(3, "energy_function", [openmc.EnergyFunctionFilter(ef_E, ef_y)], "flux"),
    T(4, "mesh_material", [openmc.MeshMaterialFilter(mesh, mm_pairs)], "flux"),
    T(5, "cell_born", [openmc.CellBornFilter([cA, cB])], "flux"),
    T(6, "cell_from", [openmc.SurfaceFilter(surf_all), openmc.CellFromFilter([cA, cB])], "current"),
    T(7, "material_from", [openmc.SurfaceFilter(surf_all), openmc.MaterialFromFilter([m1, m2])], "current"),
    T(8, "mu_surface", [openmc.SurfaceFilter(surf_all), openmc.MuSurfaceFilter(mu_edges)], "current"),
    T(9, "mesh_surface", [openmc.MeshSurfaceFilter(mesh)], "current"),
    T(10, "mesh_born", [openmc.MeshBornFilter(mesh)], "flux"),
]
# A2 (post-hoc, non-degenerate mesh) runs only the three mesh-based tallies
tallies = openmc.Tallies([t for t in ALL if (not ONLY_MESH or t.name in ("mesh_material", "mesh_surface", "mesh_born"))])

results = {t.name: [] for t in tallies}
t0 = time.time()
for i, s in enumerate(sites):
    wd = tempfile.mkdtemp(prefix="expA_")
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
    with open(os.path.join(OUT, "%s_%s.txt" % (PREFIX, name)), "w") as f:
        for i, row in enumerate(rows):
            for b, v in enumerate(row):
                f.write("%d %d %.17g\n" % (i, b, v))
print("sites", len(sites), "runs", len(sites), "wall_s %.1f" % wall)
