"""ACE library -> OpenMC HDF5 library for the pincell depletion comparison
(gh:#1626 arm 2), the same conversion as the five-route study's
../../icsbep/five_route_keff/openmc_inputs/ace_to_hdf5_route.py, whose
table reader (`load`) it imports:

* IncidentNeutron.from_ace on <lib>/293.6K/<name>.ace.gz;
* 0 K elastic attached from the same library's <lib>/0K/<name>.ace.gz
  (energy['0K'] and [2].xs['0K']), so OpenMC treats every nuclide as a
  resonant scatterer exactly where the port does (OpenMC: every nuclide
  with 0 K data, nuclide.cpp:469-473; the port: every nuclide with a 0 K
  companion);
* c_H_in_H2O from <lib>/293.6K/HinH2O.ace.gz, nuclides = ['H1'].

The tables are gzipped to save disk; each is decompressed to a scratch file,
converted and removed.

Usage: python pincell_ace_to_hdf5.py <lib> <out> <name> [<name> ...]
"""
import gzip
import importlib.util
import pathlib
import shutil
import sys
import tempfile

import openmc.data

here = pathlib.Path(__file__).resolve()
spec = importlib.util.spec_from_file_location(
    "route", here.parents[2] / "icsbep/five_route_keff/openmc_inputs/ace_to_hdf5_route.py")
route = importlib.util.module_from_spec(spec)
spec.loader.exec_module(route)


def plain(gz, tmp):
    p = pathlib.Path(tmp) / gz.name[:-3]
    with gzip.open(gz, "rb") as f, open(p, "wb") as g:
        shutil.copyfileobj(f, g)
    return p


lib, out, names = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2]), sys.argv[3:]
out.mkdir(parents=True, exist_ok=True)
library = openmc.data.DataLibrary()
with tempfile.TemporaryDirectory(dir=out) as tmp:
    for name in names:
        hot_p = plain(lib / "293.6K" / f"{name}.ace.gz", tmp)
        hot = openmc.data.IncidentNeutron.from_ace(route.load(hot_p))
        hot_p.unlink()
        cold_p = plain(lib / "0K" / f"{name}.ace.gz", tmp)
        cold = openmc.data.IncidentNeutron.from_ace(route.load(cold_p))
        cold_p.unlink()
        (t_cold,) = cold.temperatures
        hot.energy["0K"] = cold.energy[t_cold]
        hot[2].xs["0K"] = cold[2].xs[t_cold]
        h5 = out / f"{hot.name}.h5"
        hot.export_to_hdf5(str(h5), "w")
        library.register_file(h5)
        print(f"{name:6} -> {h5.name}  T={hot.temperatures}  urr={'yes' if hot.urr else 'no'}", flush=True)
    th_p = plain(lib / "293.6K" / "HinH2O.ace.gz", tmp)
    th = openmc.data.ThermalScattering.from_ace(str(th_p), name="c_H_in_H2O")
    th_p.unlink()
    th.nuclides = ["H1"]
    h5 = out / "c_H_in_H2O.h5"
    th.export_to_hdf5(str(h5), "w")
    library.register_file(h5)
library.export_to_xml(str(out / "cross_sections.xml"))
print("wrote", out / "cross_sections.xml")
