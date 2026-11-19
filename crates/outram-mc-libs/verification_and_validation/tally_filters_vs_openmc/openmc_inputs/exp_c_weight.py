"""gh:#261 Experiment C -- WEIGHT filter under survival biasing, OpenMC 0.15.3.

Provenance: OpenMC 0.15.3, commit 27e38e894697bb32a1dac7848d2618818b6b8daf (built from source, no MPI);
data: ENDF/B-VIII.0 Li-6 via NJOY2016 (make_li6_lib.py; NJOY's ACER refuses B-10, "Undefined law for
dlwh block", so Li-6 is the absorber), ONE nuclide, CE mode.
Model: sphere r = 10 cm, vacuum, Li-6 at 0.05 atoms/b-cm, isotropic 300 keV point source at the centre,
survival biasing ON (weight_cutoff 0.25, weight_survive 1.0, OpenMC's defaults). 20 batches x 50 000
particles, fixed-source mode, 2 threads. Tally: flux (track-length) binned by WeightFilter
[0, 0.1, 0.25, 0.5, 0.75, 1.0]. Writes <out_dir>/expC_weight<suffix>.txt: bin mean std_dev (batch standard
error, per source particle).
Usage: exp_c_weight.py <openmc_exec> <cross_sections.xml> <out_dir> [seed]
"""
import sys, os, shutil, tempfile, time
import openmc

OPENMC, XS, OUT = sys.argv[1:4]
SEED = int(sys.argv[4]) if len(sys.argv) > 4 else 261
SUFFIX = "_s%d" % SEED if len(sys.argv) > 4 else ""
os.makedirs(OUT, exist_ok=True)
openmc.reset_auto_ids()
m = openmc.Material(material_id=1); m.add_nuclide("Li6", 1.0); m.set_density("atom/b-cm", 0.05)
mats = openmc.Materials([m]); mats.cross_sections = XS
sph = openmc.Sphere(r=10.0, surface_id=1, boundary_type="vacuum")
cell = openmc.Cell(cell_id=1, fill=m, region=-sph)
geom = openmc.Geometry([cell])
t1 = openmc.Tally(tally_id=1, name="weight")
t1.filters = [openmc.WeightFilter([0.0, 0.1, 0.25, 0.5, 0.75, 1.0])]
t1.scores = ["flux"]; t1.estimator = "tracklength"
tallies = openmc.Tallies([t1])
st = openmc.Settings(); st.run_mode = "fixed source"; st.particles = 50000; st.batches = 20; st.seed = SEED
st.survival_biasing = True; st.cutoff = {"weight": 0.25, "weight_avg": 1.0}
st.source = openmc.IndependentSource(space=openmc.stats.Point((0, 0, 0)),
                                     angle=openmc.stats.Isotropic(),
                                     energy=openmc.stats.Discrete([3.0e5], [1.0]), particle="neutron")
wd = tempfile.mkdtemp(prefix="expC_")
mats.export_to_xml(wd + "/materials.xml"); geom.export_to_xml(wd + "/geometry.xml")
st.export_to_xml(wd + "/settings.xml"); tallies.export_to_xml(wd + "/tallies.xml")
t0 = time.time()
openmc.run(cwd=wd, openmc_exec=OPENMC, output=False, threads=2)
wall = time.time() - t0
sp = openmc.StatePoint(wd + "/statepoint.20.h5")
tt = sp.get_tally(id=t1.id)
mean = tt.mean.flatten(); sd = tt.std_dev.flatten()
with open(os.path.join(OUT, "expC_weight%s.txt" % SUFFIX), "w") as f:
    for b in range(len(mean)):
        f.write("%d %.17g %.17g\n" % (b, mean[b], sd[b]))
sp.close(); shutil.rmtree(wd)
print("wall_s %.1f" % wall)
