"""NJOY2016 (ac5adf5) library for the MSRE full core, route 1 (gh:#1128, #1129).
OpenMC's own NJOY driver, IncidentNeutron.from_njoy, at the three model
temperatures (1131/1181/1231 F), with the 0 K elastic it adds for resonance
scattering. Graphite 30P from the published tape at its tabulated 800/1000 K;
OpenMC interpolates in temperature (route 4 runs LEAPR at the exact temperature
instead; stated in the record)."""
import sys, pathlib, multiprocessing as mp
import openmc.data
REPO = pathlib.Path("/home/snrsi-user/Documents/outram-park-backend")
ENDF = REPO / "reference-data/endf"
NJOY = str(pathlib.Path.home() / "Documents/research/NJOY2016/build/njoy")
OUT = REPO / "target/msre_route1/h5"
TEMPS = [round((f - 32) * 5 / 9 + 273.15, 3) for f in (1131.0, 1181.0, 1231.0)]
NUC = [l.split(",") for l in sys.argv[1].split(";")]

def one(item):
    name, tape = item
    out = OUT / f"{name}.h5"
    if out.exists():
        return f"{name}: present"
    try:
        d = openmc.data.IncidentNeutron.from_njoy(str(ENDF / tape), temperatures=TEMPS, njoy_exec=NJOY, stdout=False)
    except Exception as e:
        return f"{name}: FAILED {type(e).__name__}: {e}"
    d.export_to_hdf5(str(out), "w")
    return f"{name}: T={d.temperatures} 0K={'0K' in d.energy}"

if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    with mp.Pool(12) as p:
        for line in p.imap_unordered(one, NUC):
            print(line, flush=True)
    gr = OUT / "c_Graphite_30p.h5"
    if not gr.exists():
        th = openmc.data.ThermalScattering.from_njoy(str(ENDF / "n-006_C_012-ENDF8.0.endf"), str(ENDF / "tsl-reactor-graphite-30P.endf"),
                                                     temperatures=[800.0, 1000.0], njoy_exec=NJOY, stdout=False)
        th.name = "c_Graphite_30p"; th.nuclides = ["C12", "C13"]
        th.export_to_hdf5(str(gr), "w")
        print("graphite:", th.temperatures, flush=True)
    lib = openmc.data.DataLibrary()
    for f in sorted(OUT.glob("*.h5")):
        lib.register_file(f)
    lib.export_to_xml(str(OUT / "cross_sections.xml"))
    print("TEMPS", TEMPS, "done")
