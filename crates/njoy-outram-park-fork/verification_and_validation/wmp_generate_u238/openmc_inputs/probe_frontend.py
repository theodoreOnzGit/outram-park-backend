"""Size probe of OpenMC 8f79d9d vectfit_nuclide's front end (gh:#1390): grid size, peaks, vf_pieces.
Usage: python probe_frontend.py <endf tape>   (njoy on PATH, openmc 8f79d9d on PYTHONPATH)"""
import sys, time
import numpy as np
from math import sqrt
from scipy.signal import find_peaks
from openmc.data import IncidentNeutron
from openmc.data.multipole import ResonanceRange

endf_file = sys.argv[1]
t0 = time.time()
nuc_ce = IncidentNeutron.from_njoy(endf_file, temperatures=[0.0], error=5e-4,
                                   broadr=False, heatr=False, purr=False)
print("njoy+from_njoy seconds", time.time() - t0, flush=True)
endf_res = IncidentNeutron.from_endf(endf_file).resonances
E_max = endf_res.resolved.energy_max
E_max_idx = np.searchsorted(nuc_ce.energy['0K'], E_max, side='right') - 1
for mt in nuc_ce.reactions:
    if hasattr(nuc_ce.reactions[mt].xs['0K'], '_threshold_idx'):
        ti = nuc_ce.reactions[mt].xs['0K']._threshold_idx
        if 0 < ti < E_max_idx:
            E_max_idx = ti
energy = nuc_ce.energy['0K'][:E_max_idx + 1]
total_xs = nuc_ce[1].xs['0K'](energy)
peaks, _ = find_peaks(total_xs)
n_points = energy.size
n_peaks = peaks.size
if n_peaks > 200 or n_points > 30000 or n_peaks * n_points > 100*10000:
    vf_pieces = max(5, n_peaks // 50, n_points // 2000)
else:
    vf_pieces = 1
print("name", nuc_ce.name, "awr", nuc_ce.atomic_weight_ratio, "E", energy[0], energy[-1],
      "n_points", n_points, "n_peaks", n_peaks, "vf_pieces", vf_pieces)
