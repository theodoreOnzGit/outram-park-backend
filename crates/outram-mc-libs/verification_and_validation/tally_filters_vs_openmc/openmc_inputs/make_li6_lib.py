"""One-nuclide OpenMC library: Li-6, ENDF/B-VIII.0, NJOY2016, 293.6 K (gh:#261).
Usage: make_li6_lib.py <endf_dir> <njoy_exec> <out_dir>"""
import sys, pathlib, openmc.data
endf, njoy, out = sys.argv[1:4]
out = pathlib.Path(out); out.mkdir(parents=True, exist_ok=True)
d = openmc.data.IncidentNeutron.from_njoy(
    str(pathlib.Path(endf) / "n-003_Li_006-ENDF8.0.endf"),
    temperatures=[293.6], njoy_exec=njoy, stdout=False)
d.export_to_hdf5(str(out / "Li6.h5"), "w")
lib = openmc.data.DataLibrary(); lib.register_file(out / "Li6.h5")
lib.export_to_xml(str(out / "cross_sections.xml"))
print("done", d.temperatures)
