"""Reference for gh:#1626 arm 1: OpenMC's own transport-free depletion
(IndependentOperator) with every integrator, on the inputs of OpenMC's
regression test tests/regression_tests/deplete_no_transport (UO2 at 4.25 %,
10.4 g/cc, volume pi*0.42^2 cm^3, tests/micro_xs_simple.csv,
tests/chain_simple.xml, flux 1164719970082145.0 n/cm^2/s times the volume).

The regression test runs one 360 s Predictor step; this runs three 5-day
steps with each of the eight integrators so the multi-step methods (LE/QI,
SI-LE/QI) take their later-step branches, in both normalisation modes:
'source-rate' (source rate 1.0, so the given flux) and 'fission-q' (174 W).
OpenMC's default solver (IPF CRAM-48) and substeps (1); SI methods with
their default n_steps (10). keff=(1.0, 0.0) is passed because the
stochastic-implicit integrators average k and fail on None; it does not
enter the compositions.

Usage: python ref_independent_operator.py <tests_dir> <out.tsv>

Output lines, floats as Python repr:
  volume<TAB>v
  initial<TAB>nuclide<TAB>atoms
  mode<TAB>integrator<TAB>step<TAB>nuclide<TAB>atoms
  rate<TAB>mode<TAB>integrator<TAB>step<TAB>nuclide<TAB>reaction<TAB>rate
"""
import sys
from pathlib import Path

import numpy as np
import openmc
import openmc.deplete
from openmc.deplete import IndependentOperator, MicroXS

tests = Path(sys.argv[1])
out = open(sys.argv[2], "w")

openmc.deplete.pool.USE_MULTIPROCESSING = False

INTEGRATORS = {
    "predictor": openmc.deplete.PredictorIntegrator,
    "cecm": openmc.deplete.CECMIntegrator,
    "cf4": openmc.deplete.CF4Integrator,
    "celi": openmc.deplete.CELIIntegrator,
    "epc_rk4": openmc.deplete.EPCRK4Integrator,
    "leqi": openmc.deplete.LEQIIntegrator,
    "si_celi": openmc.deplete.SICELIIntegrator,
    "si_leqi": openmc.deplete.SILEQIIntegrator,
}
neutron_per_cm2_sec = 1164719970082145.0
DT = [5.0, 5.0, 5.0]

wrote_initial = False
for mode in ["source-rate", "fission-q"]:
    for name, cls in INTEGRATORS.items():
        openmc.reset_auto_ids()
        fuel = openmc.Material(name="uo2")
        fuel.add_element("U", 1, percent_type="ao", enrichment=4.25)
        fuel.add_element("O", 2)
        # micro_xs_simple.csv has no O18; upstream's CI expands natural O
        # against a library without it. Removing it here is that expansion's
        # effect on the nuclide list (the port reads the initial atoms below,
        # so the two codes start from the same vector either way).
        fuel.remove_nuclide("O18")
        fuel.set_density("g/cc", 10.4)
        fuel.depletable = True
        fuel.volume = np.pi * 0.42 ** 2
        micro = MicroXS.from_csv(tests / "micro_xs_simple.csv")
        op = IndependentOperator(openmc.Materials([fuel]),
                                 [neutron_per_cm2_sec * fuel.volume],
                                 [micro], tests / "chain_simple.xml",
                                 keff=(1.0, 0.0),
                                 normalization_mode=mode)
        kw = {"power": 174.0} if mode == "fission-q" else {"source_rates": 1.0}
        cls(op, DT, timestep_units="d", **kw).integrate(write_rates=True)
        res = openmc.deplete.Results(op.output_dir / "depletion_results.h5")
        mat = res[0].index_mat
        mid = list(mat.keys())[0]
        nucs = list(res[0].index_nuc.keys())
        if not wrote_initial:
            out.write(f"volume\t{fuel.volume!r}\n")
            for nuc in nucs:
                out.write(f"initial\t{nuc}\t{float(res[0][mid, nuc])!r}\n")
            wrote_initial = True
        for step in range(len(res)):
            for nuc in nucs:
                out.write(f"{mode}\t{name}\t{step}\t{nuc}\t{float(res[step][mid, nuc])!r}\n")
            rr = res[step].rates[0]
            for nuc in rr.index_nuc:
                for rx in rr.index_rx:
                    out.write(f"rate\t{mode}\t{name}\t{step}\t{nuc}\t{rx}\t{float(rr[rr.index_nuc[nuc], rr.index_rx[rx]])!r}\n")
out.close()
