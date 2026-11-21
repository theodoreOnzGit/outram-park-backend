"""gh:#261 Experiment B -- statistical comparison, CE O-16 gas, OpenMC 0.15.3.

Provenance: OpenMC 0.15.3, commit 27e38e894697bb32a1dac7848d2618818b6b8daf (built from source, no MPI);
data: ENDF/B-VIII.0 O-16 via NJOY2016 at 293.6 K (make_o16_lib.py), ONE nuclide, CE mode.
Model: sphere r = 10 cm, vacuum, O-16 at 0.03 atoms/b-cm, isotropic 1 MeV point source at the centre,
analog. 20 batches x 50 000 particles, fixed-source mode, 2 threads.
Tallies: (1) flux by collision number 0..5 (CollisionFilter, track-length); (2) Legendre order 3 x
scatter (analog).  Writes <out_dir>/expB_<tally>.txt with columns  bin mean std_dev  (std_dev = standard
error over the 20 batches, per source particle).
Usage: exp_b_collisions.py <openmc_exec> <cross_sections.xml> <out_dir> [seed]   (first run: no seed = 261; replicas: 1001..1008, files get suffix _s<seed>)
"""
import sys, os, shutil, tempfile, time
import openmc

OPENMC, XS, OUT = sys.argv[1:4]
SEED = int(sys.argv[4]) if len(sys.argv) > 4 else 261
SUFFIX = "_s%d" % SEED if len(sys.argv) > 4 else ""
os.makedirs(OUT, exist_ok=True)
openmc.reset_auto_ids()
m = openmc.Material(material_id=1); m.add_nuclide("O16", 1.0); m.set_density("atom/b-cm", 0.03)
mats = openmc.Materials([m]); mats.cross_sections = XS
sph = openmc.Sphere(r=10.0, surface_id=1, boundary_type="vacuum")
cell = openmc.Cell(cell_id=1, fill=m, region=-sph)
geom = openmc.Geometry([cell])

t1 = openmc.Tally(tally_id=1, name="collision"); t1.filters = [openmc.CollisionFilter([0, 1, 2, 3, 4, 5])]
t1.scores = ["flux"]; t1.estimator = "tracklength"
t2 = openmc.Tally(tally_id=2, name="legendre"); t2.filters = [openmc.LegendreFilter(3)]
t2.scores = ["scatter"]; t2.estimator = "analog"
tallies = openmc.Tallies([t1, t2])

st = openmc.Settings(); st.run_mode = "fixed source"; st.particles = 50000; st.batches = 20; st.seed = SEED
st.source = openmc.IndependentSource(space=openmc.stats.Point((0, 0, 0)),
                                     angle=openmc.stats.Isotropic(),
                                     energy=openmc.stats.Discrete([1.0e6], [1.0]), particle="neutron")
wd = tempfile.mkdtemp(prefix="expB_")
mats.export_to_xml(wd + "/materials.xml"); geom.export_to_xml(wd + "/geometry.xml")
st.export_to_xml(wd + "/settings.xml"); tallies.export_to_xml(wd + "/tallies.xml")
t0 = time.time()
openmc.run(cwd=wd, openmc_exec=OPENMC, output=False, threads=2)
wall = time.time() - t0
sp = openmc.StatePoint(wd + "/statepoint.20.h5")
for t in (t1, t2):
    tt = sp.get_tally(id=t.id)
    mean = tt.mean.flatten(); sd = tt.std_dev.flatten()
    with open(os.path.join(OUT, "expB_%s%s.txt" % (t.name, SUFFIX)), "w") as f:
        for b in range(len(mean)):
            f.write("%d %.17g %.17g\n" % (b, mean[b], sd[b]))
sp.close(); shutil.rmtree(wd)
print("wall_s %.1f" % wall)
