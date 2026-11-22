"""ENDF photo-atomic + atomic relaxation -> OpenMC HDF5 photon library.
Wraps two pandas-version workarounds (Ubuntu's pandas 2.1 lacks the
`future.no_silent_downcasting` option and refuses int replacement in string
columns); the numbers written are the same."""
import contextlib, sys
import numpy as np
import pandas as pd
_orig = pd.option_context
def _oc(*a, **k):
    if a and a[0] == 'future.no_silent_downcasting':
        return contextlib.nullcontext()
    return _orig(*a, **k)
pd.option_context = _oc
import openmc.data as d
from openmc.data import photon as ph
idx = {s: float(i) for i, s in enumerate(ph._SUBSHELLS)}
def to_hdf5(self, group, shell):
    group.attrs['binding_energy'] = self.binding_energy[shell]
    group.attrs['num_electrons'] = self.num_electrons[shell]
    if shell in self.transitions:
        df = self.transitions[shell].copy()
        for c in list(df.columns[:2]):
            df[c] = [idx.get(v, 0.0) if isinstance(v, str) else (0.0 if (v is None or v is pd.NA or v != v) else v) for v in df[c].astype(object)]  # _SUBSHELLS[0] is None -> 0
        rows = [[(np.nan if (v is None or v is pd.NA or (isinstance(v, float) and v != v)) else float(v)) for v in r] for r in df.values.tolist()]
        group.create_dataset('transitions', data=np.array(rows, dtype=float))
ph.AtomicRelaxation.to_hdf5 = to_hdf5
p = d.IncidentPhoton.from_endf(sys.argv[1], None if sys.argv[2] == 'none' else sys.argv[2])
p.export_to_hdf5(sys.argv[3], 'w')
print('written', p.name)
