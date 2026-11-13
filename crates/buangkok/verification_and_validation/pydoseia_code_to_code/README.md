# pyDOSEIA code-to-code: generator and records

`gen_pydoseia_reference.py` (with `tranche2.py`) runs upstream pyDOSEIA on synthetic inputs and
writes `tests/data/pydoseia_reference.csv`. `PREDICTIONS.md` and `RESULTS.md` record the
gh:#1429 round. Research, education and V&V use only (`RESPONSIBLE_USE.md`); never a dose to a
real person.

Run, with upstream cloned to a scratch directory and never into the repository:

```bash
python -m venv venv && venv/bin/pip install numpy pandas scipy openpyxl xlrd joblib matplotlib pyyaml
PYDOSEIA_DIR=<clone> venv/bin/python -I gen_pydoseia_reference.py --check
```

The fixture depends on the numpy build (`RESULTS.md`, P3): `--check` reports a difference on a
machine whose `np.exp` is not glibc's, in the groups that call `exp`.
