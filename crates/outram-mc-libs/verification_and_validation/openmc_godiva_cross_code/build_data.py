#!/usr/bin/env python3
"""Convert the NJOY2016-produced ACE files to OpenMC HDF5 and write a library.

The ACE comes from the SAME ENDF/B-VIII.0 tapes outram-mc-libs reads, processed
by the SAME NJOY2016 build (ac5adf5f) vendored in this repo, so the comparison
that follows isolates transport physics rather than folding in a data difference.

Fixed 2026-09-16: the nuclide paths were committed as the literal strings
"$ACE_DIR//U234/tape24" etc. Python does not expand shell variables inside a
string, and the `ACE` binding below was assigned and then never used, so this
script could not have run as committed -- it failed with
`FileNotFoundError: '$ACE_DIR//U234/tape24'` on the first nuclide. The V&V
record that cites it was therefore produced by a local copy that never reached
the repository.

That is the failure mode this crate's own CLAUDE.md warns about one step
removed: it requires the generating scripts be captured "not merely cited",
because "a cited-but-absent deck is a reference a reader cannot reproduce or
check". A deck that is present but broken is worse than an absent one, because
it looks reproducible.
"""
import os, openmc.data

ACE = os.environ.get("ACE_DIR", "./ace")
WORK = os.environ.get("WORK_DIR", "./work")
os.makedirs(WORK, exist_ok=True)

SRC = [
    ("U234", f"{ACE}/U234/tape24"),
    ("U235", f"{ACE}/U235/tape24"),
    ("U238", f"{ACE}/U238/tape24"),
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
