"""Capture OpenMC 8f79d9d's input to `_vectfit_xs` for all pieces of U-238 (gh:#1390 extension).

Usage: python capture_u238_inputs.py <endf tape> <out dir> <pieces.txt>
Environment: njoy on PATH; OpenMC 8f79d9d on PYTHONPATH.
Unmodified `vectfit_nuclide` runs NJOY, the front end and the slicing; `_vectfit_xs`
is replaced by a stand-in that saves each piece's (energy, ce_xs, mts, kwargs) as
`in_<i>.npz` and checks the same checksum as ref_u238_subset.py against pieces.txt.
Nothing is fitted here.
"""
import sys, os, json
import numpy as np
from openmc.data import multipole as mp

tape, out, pieces_txt = sys.argv[1], sys.argv[2], sys.argv[3]
os.makedirs(out, exist_ok=True)
rec = [l.split() for l in open(pieces_txt) if not l.startswith('#')]


def checksum(energy, ce_xs):
    w = np.ascontiguousarray(np.concatenate([np.asarray(energy, dtype='<f8')] +
                                            [np.asarray(r, dtype='<f8') for r in ce_xs])).view('<u8')
    idx = np.arange(1, w.size + 1, dtype=np.uint64)
    with np.errstate(over='ignore'):
        return [int(np.bitwise_xor.reduce(w)), int(w.sum(dtype=np.uint64)), int((w * idx).sum(dtype=np.uint64))]


count = [0]
kws = {}


def stand_in(energy, ce_xs, mts, **kw):
    i = count[0]
    count[0] += 1
    c = checksum(energy, ce_xs)
    assert [str(x) for x in c] == rec[i][2:5], f"piece {i}: checksum differs from pieces.txt"
    kws[i] = {k: v for k, v in kw.items() if k not in ('path_out',)}
    np.savez(os.path.join(out, f"in_{i}.npz"), energy=np.asarray(energy), ce_xs=np.asarray(ce_xs), mts=np.array(mts))
    return np.zeros(0, dtype=complex), np.zeros((len(mts), 0), dtype=complex)


mp._vectfit_xs = stand_in
mp.vectfit_nuclide(tape)
json.dump({"n": count[0], "kwargs": {str(k): {a: (b if isinstance(b, (int, float, bool, type(None), str)) else repr(b)) for a, b in v.items()} for k, v in kws.items()}},
          open(os.path.join(out, "capture.json"), "w"))
print("captured", count[0], "pieces, all checksums equal pieces.txt", flush=True)
