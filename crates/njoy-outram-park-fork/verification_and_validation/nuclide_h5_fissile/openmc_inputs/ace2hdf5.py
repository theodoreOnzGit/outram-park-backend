"""ACE -> HDF5 for OpenMC, plus a cross_sections.xml pointing at the result.

OpenMC's transport solver reads HDF5, not ACE, so an ACE set produced by
NJOY2016 must be converted before OpenMC can use it. This does that with
OpenMC's own reader (openmc.data.IncidentNeutron.from_ace), so the conversion
is the reference implementation's, not a reimplementation of it.
"""
import sys, pathlib
import openmc.data

src, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
lib = openmc.data.DataLibrary()
for name in sys.argv[3:]:
    ace = src / name / "tape24"
    d = openmc.data.IncidentNeutron.from_ace(str(ace))
    h5 = out / f"{d.name}.h5"
    d.export_to_hdf5(str(h5), "w")
    lib.register_file(h5)
    print(f"{name:6} -> {d.name}.h5  atomic_weight_ratio={d.atomic_weight_ratio:.6f} "
          f"temps={[f'{t}' for t in d.temperatures]}")
lib.export_to_xml(str(out / "cross_sections.xml"))
print("wrote", out / "cross_sections.xml")
