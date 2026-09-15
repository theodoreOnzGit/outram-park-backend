#!/usr/bin/env python3
"""Convert the NJOY2016-produced ACE files to OpenMC HDF5 and write a library.

The ACE comes from the SAME ENDF/B-VIII.0 tapes outram-mc-libs reads, processed
by the SAME NJOY2016 build (ac5adf5f) vendored in this repo, so the comparison
that follows isolates transport physics rather than folding in a data difference.
"""
import os, sys, openmc.data

ACE = os.environ.get("ACE_DIR", "./ace")
WORK = os.environ.get("WORK_DIR", "./work")
os.makedirs(WORK, exist_ok=True)

SRC = [
    ("U234", "$ACE_DIR//U234/tape24"),
    ("U235", "$ACE_DIR//U235/tape24"),
    ("U238", "$ACE_DIR//U238/tape24"),
]

lib = openmc.data.DataLibrary()
for name, path in SRC:
    print(f"  {name}: reading {path}")
    data = openmc.data.IncidentNeutron.from_ace(path)
    out = f"{WORK}/{name}.h5"
    data.export_to_hdf5(out, "w")
    lib.register_file(out)
    temps = sorted(data.temperatures)
    print(f"    -> {out}   name={data.name}  temperatures={temps}")
lib.export_to_xml(f"{WORK}/cross_sections.xml")
print(f"wrote {WORK}/cross_sections.xml")
