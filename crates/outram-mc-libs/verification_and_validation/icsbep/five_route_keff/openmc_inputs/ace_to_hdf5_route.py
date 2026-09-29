"""ACE library -> OpenMC HDF5 library + cross_sections.xml, for one data route.

Used for BOTH OpenMC routes of the five-route study, so the conversion cannot
differ between them:

    route 1:  python ace_to_hdf5_route.py <WORK>/njoy <WORK>/h5_njoy
    route 2:  python ace_to_hdf5_route.py <WORK>/rust <WORK>/h5_rust

Input layout (written by scripts/make_njoy_library.sh and by the njoy example
write_ace_library.rs):

    <lib>/293.6K/<name>.ace   broadened table with PURR probability tables
    <lib>/0K/<name>.ace       unbroadened companion (0 K elastic, for DBRC)
    <lib>/293.6K/HinH2O.ace   S(a,b) H in H2O, IFENG = 0

What is done, and why each step:

* ``IncidentNeutron.from_ace`` on the 293.6 K table -- OpenMC's own reader, the
  same as ``../../openmc_godiva_cross_code/ace2hdf5.py``. PURR's UNR block is
  carried into ``urr`` by that reader.
* **0 K elastic is attached from the SAME library's 0 K table**, not with
  ``add_elastic_0K_from_endf``. That helper runs NJOY2016 internally, which
  would put NJOY data into route 2 (the Rust-NJOY route) and break the point of
  having two routes. Taking the 0 K table's MT=2 does what the helper does --
  ``energy['0K']`` and ``[2].xs['0K']`` -- from the route's own generator.
* S(a,b): ``ThermalScattering.from_ace(name='c_H_in_H2O')``. The ACE reader
  leaves ``nuclides`` empty (it is an ACER ``nxtra`` field that neither
  generator here fills), so it is set to ``['H1']`` explicitly; without it
  OpenMC would apply the table to nothing.

NJOY2016 TYPE-2 (binary) TABLES. NJOY2016 2016.79's Type-1 formatter
(``change``, acefc.f90:13942) aborts on ENDF/B-VIII.0 B-10 with "Undefined law
for dlwh block: 0" in the charged-particle block, leaving a truncated file.
The same deck with ``itype = 2`` completes (no ``change`` call), so B-10's
NJOY2016 tables are binary. OpenMC's own binary reader expects MCNP's
fixed-length records, not NJOY's Fortran sequential records, so
``read_njoy_type2`` below reads NJOY's layout (acefc.f90:13029-13056) directly
into an ``openmc.data.ace.Table``. It is a container reader only: every word
is NJOY2016's own double, unrounded.

Nothing is tuned; every table present is converted.
"""
import pathlib
import struct
import sys

import numpy as np
import openmc.data
from openmc.data.ace import Table


def _records(buf):
    """Fortran sequential unformatted records (4-byte length markers)."""
    i = 0
    while i < len(buf):
        (n,) = struct.unpack_from("<i", buf, i)
        yield buf[i + 4:i + 4 + n]
        (n2,) = struct.unpack_from("<i", buf, i + 4 + n)
        assert n2 == n, "corrupt Fortran record"
        i += 8 + n


def read_njoy_type2(path):
    """One NJOY2016 Type-2 ACE table (mcnpx = 0 layout) as an OpenMC Table."""
    recs = _records(pathlib.Path(path).read_bytes())
    head = next(recs)
    name = head[0:10].decode().strip()
    aw0, tz = struct.unpack_from("<dd", head, 10)
    off = 10 + 16 + 10 + 70 + 10
    pairs = [struct.unpack_from("<id", head, off + 12 * k) for k in range(16)]
    off += 12 * 16
    ints = struct.unpack_from("<48i", head, off)
    nxs = np.array((0,) + ints[:16], dtype=int)
    jxs = np.array((0,) + ints[16:], dtype=int)
    xss = np.concatenate([np.frombuffer(r, dtype="<f8") for r in recs])
    assert xss.size == nxs[1], f"{path}: XSS {xss.size} words, NXS(1) says {nxs[1]}"
    return Table(name, aw0, tz, [(z, a) for z, a in pairs if z], nxs, jxs,
                 np.concatenate(([0.0], xss)))


def load(path):
    """An ACE table: Type 1 through OpenMC's reader, NJOY Type 2 through ours."""
    head = pathlib.Path(path).open("rb").read(16)
    try:
        head.decode("ascii")
        return openmc.data.ace.get_table(str(path))
    except UnicodeDecodeError:
        return read_njoy_type2(path)

NUCLIDES = ["U234", "U235", "U238", "F19", "O16", "H1", "Al27",
            "Si28", "Si29", "Si30", "Mn55", "B10"]


def main():
    lib, out = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    library = openmc.data.DataLibrary()
    for name in NUCLIDES:
        hot_p = lib / "293.6K" / f"{name}.ace"
        cold_p = lib / "0K" / f"{name}.ace"
        hot = openmc.data.IncidentNeutron.from_ace(load(hot_p))
        cold = openmc.data.IncidentNeutron.from_ace(load(cold_p))
        (t_cold,) = cold.temperatures
        hot.energy["0K"] = cold.energy[t_cold]
        hot[2].xs["0K"] = cold[2].xs[t_cold]
        h5 = out / f"{hot.name}.h5"
        hot.export_to_hdf5(str(h5), "w")
        library.register_file(h5)
        print(f"{name:5} -> {h5.name}  T={hot.temperatures}  urr={'yes' if hot.urr else 'no'}"
              f"  NES={len(hot.energy[hot.temperatures[0]])}  0K pts={len(hot.energy['0K'])}"
              f"  comment={getattr(hot, 'comment', '')!s:.40}")
    th_p = lib / "293.6K" / "HinH2O.ace"
    th = openmc.data.ThermalScattering.from_ace(str(th_p), name="c_H_in_H2O")
    th.nuclides = ["H1"]
    h5 = out / "c_H_in_H2O.h5"
    th.export_to_hdf5(str(h5), "w")
    library.register_file(h5)
    print(f"S(a,b) -> {h5.name}  T={th.temperatures}")
    library.export_to_xml(str(out / "cross_sections.xml"))
    print("wrote", out / "cross_sections.xml")


if __name__ == "__main__":
    main()
