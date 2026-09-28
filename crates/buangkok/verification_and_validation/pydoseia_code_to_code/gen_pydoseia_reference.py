#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Generate the pyDOSEIA code-to-code reference fixture for `buangkok::pydoseia`.

This script EXECUTES the upstream pyDOSEIA Python (MIT licence, Dr. Biswajit
Sadhu and co-authors, https://github.com/BiswajitSadhu/pyDOSEIA, commit
dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce) over a grid of inputs and writes
every output it produces at full `repr()` precision. The Rust port in
`crates/buangkok/src/pydoseia/` replays the same inputs and must reproduce the
same numbers (`crates/buangkok/tests/pydoseia_code_to_code.rs`).

No upstream DATA is used. pyDOSEIA's bundled dose-coefficient tables are
ICRP-derived (inhalation) or FGR-15 (external), and its bundled met file has
no stated provenance, so this script writes SYNTHETIC tables in upstream's own
file schemas into a temporary `library/` directory and points upstream at it.
The synthetic tables are also written as CSVs to `crates/buangkok/tests/data/`
so that the Rust side reads exactly the same numbers. Every synthetic nuclide
is called `SYN-*`; none of the numbers is nuclear data.

Usage (from the workspace root, after cloning upstream into
`vendor/pyDOSEIA` at the commit above):

    python3 crates/buangkok/verification_and_validation/pydoseia_code_to_code/gen_pydoseia_reference.py
    python3 .../gen_pydoseia_reference.py --check     # regenerate and diff only

Environment: `PYDOSEIA_DIR` overrides the upstream location. Python packages
needed: numpy, pandas, scipy, openpyxl, xlrd, joblib, matplotlib (upstream
imports them). Versions used for the committed fixture are in the header row
comments of the output CSV.

This is Python that executes a third-party reference implementation to
produce V&V data -- the same shape as `crates/boon-lay/dev/*.py` -- not
documentation generation or repository accounting.
"""
import argparse
import contextlib
import io
import itertools
import os
import sys
import tempfile

import numpy as np
import pandas as pd

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import tranche2  # noqa: E402  (second tranche: ingestion, plume shine, driver)

HERE = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.abspath(os.path.join(HERE, "..", ".."))
ROOT = os.path.abspath(os.path.join(CRATE, "..", ".."))
DATA = os.path.join(CRATE, "tests", "data")
UPSTREAM = os.environ.get("PYDOSEIA_DIR", os.path.join(ROOT, "vendor", "pyDOSEIA"))
UPSTREAM_COMMIT = "dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce"
FIXTURE = os.environ.get("PYDOSEIA_FIXTURE", os.path.join(DATA, "pydoseia_reference.csv"))

AGE_COLS_INH = [
    "e_g_age_g_lt_1a_Sv/Bq", "e_g_age_g_1_2a_Sv/Bq", "e_g_age_g_2_7a_Sv/Bq",
    "e_g_age_g_7_12a_Sv/Bq", "e_g_age_g_12_17a_Sv/Bq", "e_g_age_g_gt_17a_Sv/Bq",
]
AGE_COLS_EXT = ["Newborn", "1-yr-old", "5-yr-old", "10-yr-old", "15-yr-old", "Adult"]
AGES = [0.5, 1, 1.5, 2, 5, 7, 10, 12, 15, 17, 18, 40]

# --------------------------------------------------------------------------
# Synthetic nuclides. Element symbols are chosen to hit every element branch
# in upstream's deposition-velocity and weathering code.
# --------------------------------------------------------------------------
NUCLIDES = [
    # name,   element, half-life string (upstream primary-CSV format)
    ("SYN-1", "Co", "5.0 y"),
    ("SYN-2", "I", "8.0 d"),
    ("SYN-3", "Cs", "30.0 y"),
    ("SYN-4", "Kr", "10.0 y"),
    ("SYN-5", "F", "110.0 m"),
    ("SYN-6", "Sr", "29.0 y"),
    ("SYN-7", "Tc", "6.0 h"),
    ("SYN-8", "H", "12.0 y"),
]
EXTRA_HALF_LIVES = [("SYN-10", "250.0 s"), ("SYN-11", "3.0 ms"), ("SYN-12", "2.5 h")]

# Progeny chains in upstream's dcf_corr.xlsx layout: (parent, [(daughter, yield)]).
CHAINS = {
    "SYN-1": [("SYN-1D", 0.9), ("SYN-1E", 0.1)],
    "SYN-2": [],
    "SYN-3": [("SYN-3D", 0.944)],
    "SYN-4": [],
    "SYN-5": [],
    "SYN-6": [("SYN-9", 1.0)],
    "SYN-7": [("SYN-7D", 1.0)],
    "SYN-8": [],
    # second tranche: parents with no daughters, so that the driver's report,
    # which always looks progeny up, does not crash on them
    "SYN-10": [],
    "H-3": [],
    "C-14": [],
}
# Half-life strings of the daughters, in dcf_corr.xlsx's own format (value,
# optional space, unit letter as the LAST character).
DAUGHTER_HALF_LIVES = {
    "SYN-1D": "2.5 m",
    "SYN-1E": "5.0 h",
    "SYN-3D": "2.55 m",
    "SYN-9": "64.0 h",
    "SYN-7D": "30.0 s",
}
# External-exposure DCF rows. SYN-9m exists so that upstream's SUBSTRING
# daughter lookup (`str.contains`) is exercised: "SYN-9" also matches "SYN-9m".
EXTERNAL_ROWS = [n for n, _, _ in NUCLIDES] + ["SYN-1D", "SYN-1E", "SYN-3D", "SYN-9", "SYN-9m", "SYN-7D"]


def external_dcf(name, k, kind):
    """Deterministic synthetic external DCF (Sv m^2/(Bq s) or Sv m^3/(Bq s))."""
    base = 1.0e-16 if kind == "surface" else 1.0e-14
    idx = EXTERNAL_ROWS.index(name) + 1
    return base * (1.0 + 0.37 * idx) * (1.0 - 0.043 * k)


def inhalation_rows():
    """Synthetic inhalation table rows in RadioToxicityMaster's sheet schema."""
    rows = [dict(Nuclide="Synthetium")]  # an element-heading row, all else NaN
    types = {"SYN-1": ["F", "M", "S"], "SYN-2": ["F"], "SYN-3": ["F", "M", "S"],
             "SYN-4": ["V"], "SYN-5": ["F", "M"], "SYN-6": ["F", "S"],
             "SYN-7": ["F", "M"], "SYN-8": ["V", "F"]}
    for i, (n, _, _) in enumerate(NUCLIDES):
        for j, t in enumerate(types[n]):
            r = dict(Nuclide=n, **{"Hal-life": "synthetic"}, Type=t)
            for k, col in enumerate(AGE_COLS_INH):
                # a non-monotone pattern in type so that 'Max' is not always the last row
                r[col] = 1.0e-9 * (1 + i) * (1.0 + 0.5 * ((j * 7) % 3)) * (1.3 - 0.07 * k)
            rows.append(r)
    # A row with a missing Type (NaN): upstream's `str.contains(..., na=False)` drops it.
    r = dict(Nuclide="SYN-1", Type=np.nan)
    for col in AGE_COLS_INH:
        r[col] = 1.0  # would dominate every max if it were not dropped
    rows.append(r)
    return rows


def write_synthetic_library(lib):
    os.makedirs(os.path.join(lib, "half_life"), exist_ok=True)
    # --- inhalation (RadioToxicityMaster.xls, sheet 'Inhalation CED Sv per Bq Public')
    inh = pd.DataFrame(inhalation_rows(), columns=[
        "Nuclide", "Hal-life", "Type", "f_age_g_lt_1a", AGE_COLS_INH[0], "f1_age_g_gt_1a", *AGE_COLS_INH[1:]])
    # pandas detects the format from the content, so an xlsx body under the
    # .xls name upstream hard-codes is read by openpyxl.
    with pd.ExcelWriter(os.path.join(lib, "RadioToxicityMaster.xls"), engine="openpyxl") as w:
        inh.to_excel(w, sheet_name="Inhalation CED Sv per Bq Public", index=False)
    # Write the CSV from what upstream will READ BACK, not from the frame
    # written: the xlsx round trip can move a value by one ulp, and the Rust
    # side must see exactly upstream's inputs.
    pd.read_excel(os.path.join(lib, "RadioToxicityMaster.xls"), "Inhalation CED Sv per Bq Public").to_csv(
        os.path.join(DATA, "pydoseia_synthetic_inhalation_dcf.csv"), index=False)
    # --- external (Dose_ecerman_final.xlsx, sheets surface_dose / submersion_dose)
    frames = {}
    for kind, sheet in (("surface", "surface_dose"), ("submersion", "submersion_dose")):
        rows = [dict(Nuclide="Synthetium")]
        for n in EXTERNAL_ROWS:
            r = dict(Nuclide=n)
            for k, col in enumerate(AGE_COLS_EXT):
                r[col] = external_dcf(n, k, kind)
            rows.append(r)
        frames[sheet] = pd.DataFrame(rows, columns=["Nuclide", *AGE_COLS_EXT])
    with pd.ExcelWriter(os.path.join(lib, "Dose_ecerman_final.xlsx"), engine="openpyxl") as w:
        for sheet, df in frames.items():
            df.to_excel(w, sheet_name=sheet, index=False)
    for sheet, name in (("surface_dose", "surface"), ("submersion_dose", "submersion")):
        pd.read_excel(os.path.join(lib, "Dose_ecerman_final.xlsx"), sheet).to_csv(
            os.path.join(DATA, f"pydoseia_synthetic_{name}_dcf.csv"), index=False)
    # --- progeny chains (dcf_corr.xlsx). Upstream reads sheet 0 with header=0,
    # so the first row is a header that is thrown away.
    rows = [["h0", "h1", "h2", "h3", "h4", "h5", "h6", "h7"]]
    chain_csv = []
    for parent, daughters in CHAINS.items():
        rows.append(["Synthetium", parent, "synthetic", "B-", "-", 0, 0, 0])
        for d, y in daughters:
            rows.append([None, None, d, y, None, None, None, None])
            chain_csv.append((parent, d, y))
    for d, hl in DAUGHTER_HALF_LIVES.items():
        rows.append(["Synthetium", d, hl, "B-", "-", 0, 0, 0])
    rows.append(["END", "END", "1.0 s", "-", "-", 0, 0, 0])  # terminator
    pd.DataFrame(rows).to_excel(os.path.join(lib, "dcf_corr.xlsx"), index=False, header=False)
    pd.DataFrame(chain_csv, columns=["parent", "daughter", "yield"]).to_csv(
        os.path.join(DATA, "pydoseia_synthetic_progeny_chains.csv"), index=False)
    pd.DataFrame(sorted(DAUGHTER_HALF_LIVES.items()), columns=["nuclide", "half_life"]).to_csv(
        os.path.join(DATA, "pydoseia_synthetic_progeny_half_lives.csv"), index=False)
    # --- half lives (primary CSV; empty fallback nomenclature file)
    pd.DataFrame(columns=["Fixed_nuclide_name", "DOE_STD_1196_name", "Half-life", "unit"]).to_csv(
        os.path.join(lib, "half_life", "formatted_nuclide_nomenclature.csv"), index=False)
    prim = pd.DataFrame(
        [(0, "Synthetium", n, hl) for n, _, hl in NUCLIDES] + [(0, "Synthetium", n, hl) for n, hl in EXTRA_HALF_LIVES],
        columns=["Z", "Element", "Nuclide", "Half-life"])
    prim.to_csv(os.path.join(lib, "half_life", "radionuclides_halflife_complete.csv"), index=False)
    pd.DataFrame(columns=["Nuclide"]).to_csv(os.path.join(lib, "half_life", "Table1_2_JAERI_half_life.csv"), index=False)
    # --- second tranche: ingestion, plume-shine and screening tables
    tranche2.write_library(lib, DATA)


def synthetic_met(path_xlsx):
    """Two 'years' of synthetic hourly met records with deliberate gaps and edge values."""
    rng = np.random.default_rng(20260928)
    sheets = {"2001": 30, "2002": 25}
    letters = np.array(list("ABCDEF"))
    out = []
    with pd.ExcelWriter(path_xlsx, engine="openpyxl") as w:
        for sheet, days in sheets.items():
            n = days * 24
            hour = np.tile(np.arange(24), days).astype(float)
            speed = np.round(rng.lognormal(mean=1.8, sigma=0.8, size=n), 1)  # km/h
            calm = rng.random(n) < 0.08
            speed[calm] = np.round(rng.uniform(0.0, 1.7, size=calm.sum()), 2)
            direction = np.round(rng.uniform(0, 360, size=n), 1)
            # exact bin edges, including both ends of the wrapped north sector
            direction[:6] = [0.0, 11.25, 348.75, 360.0, 33.75, 180.0]
            speed[6:10] = [1.8, 3.0, 74.5, 80.0]  # edges; 80 km/h falls outside the last bin
            stab = letters[rng.choice(6, size=n, p=[0.05, 0.1, 0.15, 0.35, 0.2, 0.15])].astype(object)
            spd = speed.astype(object)
            dr = direction.astype(object)
            for arr, frac in ((spd, 0.02), (dr, 0.02), (stab, 0.02)):
                gaps = rng.random(n) < frac
                arr[gaps] = None
            df = pd.DataFrame({"HOUR": hour, "WS 10m(kmph)": spd, "DIR at 10m": dr, "STBCLASS": stab})
            df.to_excel(w, sheet_name=sheet, index=False)
            d2 = df.copy()
            d2.insert(0, "sheet", sheet)
            out.append(d2)
    met = pd.concat(out, ignore_index=True)
    met.to_csv(os.path.join(DATA, "pydoseia_synthetic_met.csv"), index=False)
    return list(sheets), [sheets[s] for s in sheets]


# --------------------------------------------------------------------------
# Output
# --------------------------------------------------------------------------
ROWS = []


def fmt(v):
    if isinstance(v, (list, tuple, np.ndarray)):
        return "|".join(fmt(x) for x in v)
    if isinstance(v, (bool, np.bool_)):
        return "true" if v else "false"
    if isinstance(v, (float, np.floating)):
        return repr(float(v))
    if isinstance(v, (int, np.integer)):
        return str(int(v))
    return str(v)


def emit(group, inputs, output, value):
    inp = ";".join(f"{k}={fmt(v)}" for k, v in inputs.items())
    ROWS.append((group, inp, output, fmt(value)))


def base_config(**kw):
    c = dict(
        have_dilution_factor=False, have_met_data=False, long_term_release=False, single_plume=True,
        measurement_height=10, release_height=10, downwind_distances=[100], plant_boundary=100,
        run_dose_computation=False, max_conc_plume_central_line_gl=True, like_to_scale_with_mean_speed=False,
        Y=0, Z=0, ask_mean_speed_data=None, weathering_corr=False, consider_progeny=False,
        ignore_half_life=1800, exposure_period=30, type_rad=None, rads_list=None, element_list=None,
        age_group=[18], annual_discharge_bq_rad_list=None, instantaneous_release_bq_list=None,
        calm_correction=False, start_operation_time=0, end_operation_time=24, sampling_time=60,
    )
    c.update(kw)
    return c


def quiet():
    return contextlib.redirect_stdout(io.StringIO())


def run(MetFunc, met_path, met_sheets, met_days):
    D = [10.0, 50.0, 99.9, 100.0, 250.0, 800.0, 1000.0, 1000.1, 1600.0, 5000.0, 20000.0]
    m = MetFunc(None, base_config(), "x")
    # ---- sigma_y, sigma_z
    for s, x in itertools.product(range(1, 7), D):
        emit("sigmay", dict(stab=s, x=x), "sigma_y", m.sigmay(s, x)[0])
        emit("sigmaz", dict(stab=s, x=x), "sigma_z", m.sigmaz(s, x)[0])
    # ---- height correction factor
    for s, h, hm in itertools.product(range(1, 7), [0.5, 5.0, 10.0, 30.0, 100.0], [10.0, 50.0]):
        m.release_height, m.measurement_height = h, hm
        emit("height_factor", dict(stab=s, release_height=h, measurement_height=hm), "factor",
             m.height_correction_factor(s))
    # ---- master equations
    for sy, sz, f, h, y, z in [(10.0, 5.0, 1.0, 10.0, 0.0, 0.0), (35.2, 18.4, 1.26, 30.0, 0.0, 0.0),
                               (120.0, 60.0, 2.0, 100.0, 40.0, 2.0), (3.0, 1.5, 0.8, 0.0, 1.0, 0.0),
                               (500.0, 300.0, 1.5, 60.0, -250.0, 10.0)]:
        central = (y == 0.0 and z == 0.0)
        m.config.update(max_conc_plume_central_line_gl=central, Y=y, Z=z)
        m.release_height = h
        pre, ex = m.master_eq_single_plume(sy, sz, f)
        inp = dict(sigma_y=sy, sigma_z=sz, factor=f, release_height=h, y=y, z=z, central=central)
        emit("master_single", inp, "pre_expo", pre)
        emit("master_single", inp, "expo", ex)
    for x1, sz, h, z in [(100.0, 5.0, 10.0, 0.0), (1600.0, 40.0, 30.0, 0.0), (5000.0, 100.0, 100.0, 1.5)]:
        central = z == 0.0
        m.config.update(max_conc_plume_central_line_gl=central, Z=z)
        m.release_height = h
        pre, ex = m.master_eq_sector_averaged_plume(x1, sz)
        inp = dict(x=x1, sigma_z=sz, release_height=h, z=z, central=central)
        emit("master_sector", inp, "pre_expo", pre)
        emit("master_sector", inp, "expo", ex)

    # ---- dilution factor, no met data, both release modes
    mean_speeds = [1.1, 1.7, 2.3, 3.4, 2.6, 1.9]
    for mode, x, h, hm, scale in itertools.product(["single_plume", "long_term"], [80.0, 100.0, 500.0, 1600.0, 8000.0],
                                                  [5.0, 30.0, 100.0], [10.0], [False, True]):
        cfg = base_config(single_plume=mode == "single_plume", long_term_release=mode == "long_term",
                          release_height=h, measurement_height=hm, like_to_scale_with_mean_speed=scale,
                          ask_mean_speed_data=mean_speeds)
        mm = MetFunc(None, cfg, "x")
        with quiet():
            dil = mm.dilution_per_sector(x)
        inp = dict(mode=mode, x=x, release_height=h, measurement_height=hm, scale_with_mean_speed=scale)
        emit("dilution_no_met", inp, "per_stability", list(dil))
        emit("dilution_no_met", inp, "internal_max", mm.max_dilution_factor)

    # ---- met processing (two operating windows)
    for start, end in [(0, 24), (6, 18)]:
        cfg = base_config(have_met_data=True, long_term_release=True, single_plume=False, path_met_file=met_path,
                          excel_sheet_name=met_sheets, num_days=met_days,
                          column_names=["HOUR", "WS 10m(kmph)", "DIR at 10m", "STBCLASS"],
                          start_operation_time=start, end_operation_time=end, release_height=30.0,
                          measurement_height=10.0, calm_correction=False)
        mm = MetFunc(None, cfg, "x")
        with quiet():
            mm.file_preprocessing()
            mm.met_data_to_tjfd()
            mm.missing_correction()
            calm = mm.calm_correction_factor_calc()
        win = dict(start=start, end=end)
        for yi, sheet in enumerate(met_sheets):
            for s in range(6):
                emit("tjfd", dict(win, year=yi, stab=s + 1), "counts", mm.TJFD_ALL[yi][s].flatten())
                emit("tjfd_missing", dict(win, year=yi, stab=s + 1), "counts",
                     np.asarray(mm.TJFD_ALL_MISSING_CORR[yi][s]).flatten())
            emit("calm_factors", dict(win, year=yi), "factors", list(calm[yi]))
        with quiet():
            sd, means = mm.speed_distribution_list(quantile=0.90)
        emit("speed_means", win, "mean_speed_kmph", list(means))
        for x, cc in itertools.product([100.0, 800.0, 1600.0, 5000.0], [False, True]):
            mm.config["calm_correction"] = cc
            mm.plot_speed_distribution = lambda figname=None: None  # upstream writes a PNG here
            with quiet():
                dil = mm.dilution_per_sector(x)
            emit("dilution_met_long_term", dict(win, x=x, calm_correction=cc), "per_sector", list(dil))
    # Single plume scaled by the met-derived mean speeds is NOT generated:
    # upstream divides a (6,) array by `mean_speeds[:, None]`, which broadcasts
    # to (6, 6) and then fails its own `shape == (6,)` assertion at commit
    # dca4cdc3 (defect D3 in docs/pydoseia-code-to-code.md). The path cannot run.

    # ---- nuclide data, DCF lookups
    names = [n for n, _, _ in NUCLIDES]
    elements = [e for _, e, _ in NUCLIDES]
    for n, hl in [(n, hl) for n, _, hl in NUCLIDES] + EXTRA_HALF_LIVES:
        with quiet():
            info = m.get_nuclide_info(n)
        emit("half_life", dict(nuclide=n, text=hl), "half_life_s", info["Half-life (s)"])
        emit("half_life", dict(nuclide=n, text=hl), "decay_constant", info["Decay Constant (s^-1)"])
    for ty in ["Max", "F", "M", "S", "V"]:
        cfg = base_config(run_dose_computation=True, rads_list=names, element_list=elements,
                          type_rad=[ty] * len(names), instantaneous_release_bq_list=[1.0] * len(names))
        mm = MetFunc(None, cfg, "x")
        for age in AGES:
            with quiet():
                d = mm.inhalation_dcf_list(master_file="library/RadioToxicityMaster.xls",
                                           sheet_name="Inhalation CED Sv per Bq Public", age=age)
            emit("dcf_inhalation", dict(type=ty, age=age), "dcf", list(d))
    for prog, ign in [(False, 1800), (True, 1800), (True, 1.0e6)]:
        cfg = base_config(run_dose_computation=True, rads_list=names, element_list=elements,
                          consider_progeny=prog, ignore_half_life=ign)
        mm = MetFunc(None, cfg, "x")
        for age in AGES:
            with quiet():
                gs = mm.dcf_list_ecerman_ground_shine_include_progeny(
                    master_file="library/Dose_ecerman_final.xlsx", sheet_name="surface_dose", age=age,
                    consider_progeny=prog)
                sub = mm.dcf_list_ecerman_submersion_include_progeny(
                    master_file="library/Dose_ecerman_final.xlsx", sheet_name="submersion_dose", age=age)
            inp = dict(progeny=prog, ignore_half_life=ign, age=age)
            emit("dcf_surface", inp, "corrected", [t[0] for t in gs])
            emit("dcf_surface", inp, "uncorrected", [t[1] for t in gs])
            emit("dcf_submersion", inp, "corrected", [t[0] for t in sub])
            emit("dcf_submersion", inp, "uncorrected", [t[1] for t in sub])

    # ---- deposition velocity and weathering
    cfg = base_config(run_dose_computation=True, rads_list=names, element_list=elements)
    mm = MetFunc(None, cfg, "x")
    emit("deposition_velocity", dict(), "v_m_per_s", mm.deposition_velocity_of_rad())
    with quiet():
        mm.find_half_life_and_decay_const_radionuclides()
    for w, ep in itertools.product([False, True], [1.0, 30.0, 50.0]):
        mm.config.update(weathering_corr=w, exposure_period=ep)
        emit("effective_lambda", dict(weathering=w, exposure_period_y=ep), "frac_s",
             mm.apply_weathering_correction_gs())

    # ---- doses
    q = [1.0e9 * (1 + 0.5 * i) for i in range(len(names))]
    chi = [3.1e-6, 4.7e-5]
    for mode, age, ty, w, prog, c in itertools.product(["single_plume", "long_term"], [1, 2, 10, 18],
                                                       ["Max", "F"], [False, True], [False, True], chi):
        cfg = base_config(single_plume=mode == "single_plume", long_term_release=mode == "long_term",
                          run_dose_computation=True, rads_list=names, element_list=elements,
                          type_rad=[ty] * len(names), weathering_corr=w, consider_progeny=prog,
                          ignore_half_life=1800, exposure_period=30.0,
                          instantaneous_release_bq_list=q, annual_discharge_bq_rad_list=q)
        mm = MetFunc(None, cfg, "x")
        inp = dict(mode=mode, age=age, type=ty, weathering=w, progeny=prog, chi_over_q=c)
        with quiet():
            emit("dose_inhalation", inp, "msv", mm.inhalation_dose(100, age=age, max_dilutfac_for_distance_secperm3=c))
            emit("dose_ground_shine", inp, "msv", list(mm.ground_shine_dose(100, age=age, max_dilutfac_for_distance_secperm3=c)))
            emit("dose_submersion", inp, "msv", list(mm.submersion_dose(100, age=age, max_dilutfac_for_distance_secperm3=c)))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="regenerate into memory and diff against the fixture")
    args = ap.parse_args()
    sys.path.insert(0, UPSTREAM)
    os.makedirs(DATA, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        write_synthetic_library(os.path.join(tmp, "library"))
        met_path = os.path.join(tmp, "synthetic_met.xlsx")
        sheets, days = synthetic_met(met_path)
        cwd = os.getcwd()
        os.chdir(tmp)  # upstream opens "library/..." relative to the working directory
        try:
            with quiet():
                from metfunc import MetFunc
            if os.environ.get("PYDOSEIA_ONLY") != "tranche2":
                run(MetFunc, met_path, sheets, days)
            if os.environ.get("PYDOSEIA_ONLY") != "tranche1":
                tranche2.run(emit, base_config, MetFunc, met_path, sheets, days)
        finally:
            os.chdir(cwd)
    import scipy
    header = (f"# pyDOSEIA code-to-code fixture. upstream commit {UPSTREAM_COMMIT}; "
              f"python {sys.version.split()[0]}, numpy {np.__version__}, pandas {pd.__version__}, "
              f"scipy {scipy.__version__}. Generated by "
              f"verification_and_validation/pydoseia_code_to_code/gen_pydoseia_reference.py. "
              f"Synthetic inputs only; no upstream data tables.\n")
    body = header + "group,inputs,output,value\n" + "".join(f"{g},{i},{o},{v}\n" for g, i, o, v in ROWS)
    if args.check:
        with open(FIXTURE) as f:
            old = f.read()
        same = old.split("\n", 1)[1] == body.split("\n", 1)[1]
        print("fixture unchanged" if same else "FIXTURE DIFFERS")
        sys.exit(0 if same else 1)
    with open(FIXTURE, "w") as f:
        f.write(body)
    print(f"wrote {len(ROWS)} rows to {FIXTURE}")


if __name__ == "__main__":
    main()
