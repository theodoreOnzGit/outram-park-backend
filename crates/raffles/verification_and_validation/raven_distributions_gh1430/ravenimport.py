# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 OUTRAM PARK contributors
"""Import RAVEN modules standalone, without RAVEN's build or its framework driver.

* `ravenframework/__init__.py` imports the whole driver (PythonRaven); a bare
  package object with the right __path__ is put in its place.
* `crow_modules` (the retired C++/SWIG layer) is stubbed: RAVEN's default
  stochastic environment is numpy (utils/randomUtils.py:37), and nothing this
  generator calls reaches crow. Any call into the stub raises.
"""
import os, sys, types

RAVEN = os.environ["RAVEN_DIR"]
pkg = types.ModuleType("ravenframework")
pkg.__path__ = [os.path.join(RAVEN, "ravenframework")]
sys.modules["ravenframework"] = pkg


class _Unavailable:
    def __init__(self, *a, **k):
        pass

    def __getattr__(self, name):
        raise RuntimeError("crow stub reached: " + name)


crow = types.ModuleType("crow_modules")
crow.__path__ = []
for sub in ("distribution1D", "randomENG", "interpolationND"):
    m = types.ModuleType("crow_modules." + sub)
    m.RandomClass = _Unavailable
    m.__getattr__ = lambda name: _Unavailable
    sys.modules["crow_modules." + sub] = m
    setattr(crow, sub, m)
sys.modules["crow_modules"] = crow


# Optional heavy dependencies that RAVEN's SupervisedLearning/Samplers
# packages import at module load (plotting, data frames, time series, DOE)
# but that no routine this generator calls ever uses. They are stubbed so the
# real RAVEN classes can be imported; a call into any stub raises.
import importlib.abc
import importlib.machinery

_STUBBED = ("matplotlib", "pandas", "xarray", "statsmodels", "tensorflow",
            "pyDOE3", "AMSC", "dask", "sympy", "netCDF4", "h5py", "psutil",
            "joblib_stub")


class _Stub(types.ModuleType):
    def __getattr__(self, name):
        if name.startswith("__"):
            raise AttributeError(name)
        return _Stub(self.__name__ + "." + name)

    def __call__(self, *a, **k):
        raise RuntimeError("stubbed optional dependency reached: " + self.__name__)


STUBBED_SEEN = set()


class _StubFinder(importlib.abc.MetaPathFinder, importlib.abc.Loader):
    """Last on sys.meta_path: reached only when no real finder has the
    module. Anything outside ravenframework / numpy / scipy / sklearn that is
    missing is stubbed, and recorded in STUBBED_SEEN for the reference file."""

    def find_spec(self, fullname, path, target=None):
        top = fullname.split(".")[0]
        if top in ("ravenframework", "numpy", "scipy", "sklearn", "ray", "PySide", "PySide2", "PySide6", "PyQt5", "PyQt6"):
            return None
        # Stub only imports made by RAVEN's own modules; a library's optional
        # import (scipy probing for Cython, say) must still fail normally.
        f = sys._getframe(1)
        while f is not None and "importlib" in f.f_code.co_filename:
            f = f.f_back
        if f is None or not str(f.f_globals.get("__name__", "")).startswith(("ravenframework", "crow_modules")):
            return None
        STUBBED_SEEN.add(top)
        return importlib.machinery.ModuleSpec(fullname, self, is_package=True)

    def create_module(self, spec):
        m = _Stub(spec.name)
        m.__path__ = []
        return m

    def exec_module(self, module):
        pass


sys.meta_path.append(_StubFinder())
