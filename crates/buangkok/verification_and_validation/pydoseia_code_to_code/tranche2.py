# SPDX-License-Identifier: GPL-3.0-only
"""Second tranche of the pyDOSEIA code-to-code fixture: ingestion, plume shine,
the QUADPACK routine behind it, multi-source DCF screening, plume rise and the
driver (`outputfunc.dose_calculation_script` and `main.py`'s summaries).

Imported by `gen_pydoseia_reference.py`; see that file for the method. Same
rules: upstream (MIT, commit dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce) is
EXECUTED on SYNTHETIC tables written in its own file layouts; every committed
input CSV is written from what upstream reads back. No upstream data table is
used or committed. Names like "H-3", "C-14", "HTO" and "OBT" appear because
upstream branches on those exact strings; every number attached to them here
is synthetic.
"""
import contextlib
import io
import itertools
import math
import os

import numpy as np
import pandas as pd

AGE_COLS_INH = [
    "e_g_age_g_lt_1a_Sv/Bq", "e_g_age_g_1_2a_Sv/Bq", "e_g_age_g_2_7a_Sv/Bq",
    "e_g_age_g_7_12a_Sv/Bq", "e_g_age_g_12_17a_Sv/Bq", "e_g_age_g_gt_17a_Sv/Bq",
]

# Ingestion nuclides: (name, element, half-life text). The SYN-* ones reuse the
# first tranche's half-lives; H-3 and C-14 get synthetic half-lives.
EXTRA_PRIMARY_HALF_LIVES = [("H-3", "12.0 y"), ("C-14", "5000.0 y"), ("SCR-1", "3.0 d"),
                            ("SCR-2", "40.0 y"), ("SCR-3", "2.0 h")]
ING_ELEMENTS = {"SYN-1": "Co", "SYN-2": "I", "SYN-3": "Cs", "SYN-4": "Kr", "SYN-5": "F",
                "SYN-6": "Sr", "SYN-7": "Tc", "SYN-8": "H", "SYN-10": "Ru", "H-3": "H", "C-14": "C"}

# eco_param rows (Element, lambda_s_per_d, Fv1, Fv2, lambda_w_per_d, Fm, Ff).
# "Csx" precedes "Cs" so that upstream's substring pre-filter passes it and the
# token regex has to reject it; "Sr Y" tests the token match inside a field.
ECO_ROWS = [
    ("Co", 1.4e-4, 2.1e1, 1.1e-1, 5.0e-2, 3.1e-4, 1.2e-2),
    ("I", 1.4e-3, 3.2e-1, 2.0e-2, 5.0e-2, 1.1e-2, 6.7e-3),
    ("Csx", 9.9e-1, 9.9e0, 9.9e0, 9.9e-1, 9.9e-1, 9.9e-1),
    ("Cs", 1.4e-4, 5.3e-1, 4.0e-2, 5.0e-2, 7.9e-3, 5.0e-2),
    ("Sr Y", 1.4e-4, 1.7e0, 3.0e-1, 5.0e-2, 2.8e-3, 8.1e-3),
    ("Tc", 1.4e-3, 7.6e0, 5.0e0, 5.0e-2, 1.2e-3, 1.1e-4),
    ("F", 2.0e-4, 6.6e-2, 1.1e-2, 5.0e-2, 1.7e-3, 1.5e-1),
    ("Kr", 0.0, 0.0, 0.0, 5.0e-2, 0.0, 0.0),
]

ING_DCF_NAMES = ["SYN-1", "SYN-2", "SYN-3", "SYN-4", "SYN-5", "SYN-6", "SYN-7", "SYN-8", "SYN-10",
                 "HTO", "OBT", "C-14"]

# Gamma lines (no header row upstream): nuclide, keV, std, probability, std, blank.
GAMMA_ROWS = [
    ("SYN-1", 812.0, 0.1, 0.93, 0.01),
    ("SYN-1", 1255.0, 0.1, 0.61, 0.01),
    ("SYN-1", 30.0, 0.1, 0.20, 0.01),     # below 50 keV: neglected
    ("SYN-1", 400.0, 0.1, 0.0005, 0.0),   # probability below 1e-3: neglected
    ("SYN-1D", 520.0, 0.1, 0.40, 0.01),   # picked up for SYN-1 by the substring match
    ("SYN-3", 655.0, 0.1, 0.86, 0.01),
    ("SYN-4", 152.0, 0.1, 0.07, 0.01),
    ("SYN-2", 25.0, 0.1, 0.30, 0.01),     # its only line is neglected -> placeholder
]
# Air attenuation: energy (MeV), mu/rho, mu_en/rho (cm^2/g). Synthetic smooth
# curves, not NIST values.
ATT_E = [0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0, 1.5, 2.0, 3.0, 5.0]


def att_row(e):
    mu = 0.058 * e ** -0.43 + 4.9e-3 * e ** -2.6
    mua = 0.027 * e ** -0.12 * math.exp(-0.18 * e) + 4.6e-3 * e ** -2.9
    return (e, round(mu, 6), round(min(mua, 0.97 * mu), 6))


# Screening tables (upstream's raw positional layouts). SCR-1 has rows in
# several tables and types; SCR-2 has an alternate DOE name; "SCR-1_VAPOUR"
# is picked up by the delimited regex.
def screening_frames():
    def c(base, k):
        return base * (1.0 + 0.11 * k)
    t7 = [(0, "SCR-1_VAPOUR", "gas", "3 d", "V", 100, 1.0, c(2e-9, 0), 1.0, c(2e-9, 1), c(2e-9, 2), c(2e-9, 3), c(2e-9, 4), c(2e-9, 5)),
          (1, "SCR-3", "gas", "2 h", "V", 100, 1.0, c(4e-11, 0), 1.0, c(4e-11, 1), c(4e-11, 2), c(4e-11, 3), c(4e-11, 4), c(4e-11, 5))]
    t5 = [(0, "SCR-1", "3 d", "F", 0.1, c(1e-9, 0), 0.1, c(1e-9, 1), c(1e-9, 2), c(1e-9, 3), c(1e-9, 4), c(1e-9, 5)),
          (1, "SCR-1", "3 d", "M", 0.1, c(3e-9, 0), 0.1, c(3e-9, 1), c(3e-9, 2), c(3e-9, 3), c(3e-9, 4), c(3e-9, 5)),
          (2, "SCR-2", "40 y", "S", 0.1, c(5e-8, 0), 0.1, c(5e-8, 1), c(5e-8, 2), c(5e-8, 3), c(5e-8, 4), c(5e-8, 5))]
    a2 = [(0, "SCR-1", "F", 0.1, c(9e-9, 0), c(9e-9, 1), c(9e-9, 2), c(9e-9, 3), c(9e-9, 4), c(9e-9, 5), "x"),
          (1, "SCR-2A", "S", 0.1, c(7e-8, 0), c(7e-8, 1), c(7e-8, 2), c(7e-8, 3), c(7e-8, 4), c(7e-8, 5), "x")]
    ag = [("SCR-1", "3 d", "S", 0.1, c(6e-9, 0), 0.1, c(6e-9, 1), c(6e-9, 2), c(6e-9, 3), c(6e-9, 4), c(6e-9, 5)),
          ("SCR-2", "40 y", "M", 0.1, c(2e-8, 0), 0.1, c(2e-8, 1), c(2e-8, 2), c(2e-8, 3), c(2e-8, 4), c(2e-8, 5))]
    af = [("SCR-1", "3 d", 0.1, c(4e-9, 0), 0.1, c(4e-9, 1), c(4e-9, 2), c(4e-9, 3), c(4e-9, 4), c(4e-9, 5)),
          ("SCR-2", "40 y", 0.1, c(3e-8, 0), 0.1, c(3e-8, 1), c(3e-8, 2), c(3e-8, 3), c(3e-8, 4), c(3e-8, 5))]
    t4 = [("SCR-1", "3 d", 0.1, c(5e-9, 0), 0.1, c(5e-9, 1), c(5e-9, 2), c(5e-9, 3), c(5e-9, 4), c(5e-9, 5))]
    return t7, t5, a2, ag, af, t4


T7_COLS = ['Index', 'Nuclide', 'Chemical Form', 'Half-life', 'Type', 'percent_deposit', 'f1', 'inh_infant',
           'f1_age_g_gt_1a', 'inh_1_year', 'inh_5_years', 'inh_10_years', 'inh_15_years', 'inh_adult']
T5_COLS = ['Index', 'Nuclide', 'Half-life', 'Type', 'f1', 'inh_infant', 'f1_age_g_gt_1a', 'inh_1_year',
           'inh_5_years', 'inh_10_years', 'inh_15_years', 'inh_adult']
A2_COLS = ['Index', 'Nuclide', 'Type', 'f1', 'inh_infant', 'inh_1_year', 'inh_5_year', 'inh_10_year',
           'inh_15_year', 'inh_adult', 'Reference Person']
AG_COLS = ['Nuclide', 'Half-life', 'Type', 'f1', 'inh_infant', 'f1_age_g_gt_1a', 'inh_1_year', 'inh_5_years',
           'inh_10_years', 'inh_15_years', 'inh_adult']
ING_COLS = ['Nuclide', 'Half-life', 'f1', 'inh_infant', 'f1_age_g_gt_1a', 'inh_1_year', 'inh_5_years',
            'inh_10_years', 'inh_15_years', 'inh_adult']


def write_library(lib, data_dir):
    """Add the tranche-2 sheets and files to the synthetic library."""
    ecer = os.path.join(lib, "Dose_ecerman_final.xlsx")
    ing = [dict(Nuclide=n, **{col: 1.0e-9 * (1 + 0.37 * i) * (1.25 - 0.05 * k) for k, col in enumerate(AGE_COLS_INH)})
           for i, n in enumerate(ING_DCF_NAMES)]
    ing.insert(3, dict(Nuclide="SYN-3", **{col: 0.5e-9 for col in AGE_COLS_INH}))  # duplicate: max wins
    eco = pd.DataFrame(ECO_ROWS, columns=["Element", "lambda_s_per_d", "Fv1", "Fv2", "lambda_w_per_d",
                                          "Fm_Milk_d_per_L", "Ff_Meat_d_per_kg"])
    gam = pd.DataFrame([list(r) + [None] for r in GAMMA_ROWS])
    att = pd.DataFrame([att_row(e) for e in ATT_E], columns=["energy", "total_atten_coeff", "energy_atten_coeff"])
    with pd.ExcelWriter(ecer, engine="openpyxl", mode="a") as w:
        pd.DataFrame(ing, columns=["Nuclide", *AGE_COLS_INH]).to_excel(w, sheet_name="ingestion_gsr3", index=False)
        eco.to_excel(w, sheet_name="eco_param", index=False)
        gam.to_excel(w, sheet_name="gamma_energy_radionuclide", index=False, header=False)
        att.to_excel(w, sheet_name="mass_attenuation_coeff", index=False)
    # CSVs from what upstream reads back.
    pd.read_excel(ecer, "ingestion_gsr3").to_csv(os.path.join(data_dir, "pydoseia_synthetic_ingestion_dcf.csv"), index=False)
    pd.read_excel(ecer, "eco_param").to_csv(os.path.join(data_dir, "pydoseia_synthetic_eco_param.csv"), index=False)
    g = pd.read_excel(ecer, "gamma_energy_radionuclide", header=None,
                      names=['nuclide', 'energy_kev', 'std_energy_kev', 'emmission_prob', 'std_emmission_prob', ''])
    g.iloc[:, :-1].to_csv(os.path.join(data_dir, "pydoseia_synthetic_gamma_lines.csv"), index=False)
    pd.read_excel(ecer, "mass_attenuation_coeff", names=["energy", "total_atten_coeff", "energy_atten_coeff"]).to_csv(
        os.path.join(data_dir, "pydoseia_synthetic_attenuation.csv"), index=False)
    # Half-lives for H-3, C-14 and the screening nuclides.
    prim_path = os.path.join(lib, "half_life", "radionuclides_halflife_complete.csv")
    prim = pd.read_csv(prim_path)
    extra = pd.DataFrame([(0, "Synthetium", n, hl) for n, hl in EXTRA_PRIMARY_HALF_LIVES], columns=prim.columns)
    pd.concat([prim, extra], ignore_index=True).to_csv(prim_path, index=False)
    # Nomenclature with alternate names (screening nuclides only).
    nom = pd.DataFrame([("SCR-2", "SCR-2A", None, None, None, 40.0, "y")],
                       columns=["Fixed_nuclide_name", "DOE_STD_1196_name", "FGR_12_name", "ICRP119_107_name",
                                "ICRP_38_name", "Half-life", "unit"])
    nom.to_csv(os.path.join(lib, "half_life", "formatted_nuclide_nomenclature.csv"), index=False)
    # Screening tables.
    os.makedirs(os.path.join(lib, "inhalation_HC2"), exist_ok=True)
    os.makedirs(os.path.join(lib, "ingestion_public"), exist_ok=True)
    t7, t5, a2, ag, af, t4 = screening_frames()
    pd.DataFrame(t7, columns=T7_COLS).to_csv(os.path.join(
        lib, "inhalation_HC2", "Table_7_JAERI_dcf_inh_Public_Soluble_Reactive_Gases_Vapours.csv"), index=False)
    pd.DataFrame(t5, columns=T5_COLS).to_csv(os.path.join(
        lib, "inhalation_HC2", "Table_5_JAERI_dcf_inh_particulates_public.csv"), index=False)
    pd.DataFrame(a2, columns=A2_COLS).to_csv(os.path.join(
        lib, "inhalation_HC2", "Table_A2-DOE-STD-1196-2011_dcf_inhal.csv"), index=False)
    pd.DataFrame(ag, columns=AG_COLS).to_excel(os.path.join(
        lib, "inhalation_HC2", "Annex_G_ICRP119_dcf_inh_public.xlsx"), index=False)
    pd.DataFrame(af, columns=ING_COLS).to_csv(os.path.join(
        lib, "ingestion_public", "AnnexF_ICRP119_dcf_ingestion_public.csv"), index=False)
    pd.DataFrame(t4, columns=ING_COLS).to_csv(os.path.join(
        lib, "ingestion_public", "table_4_jaeri_ingestion_public.csv"), index=False)
    # Committed copies, with upstream's renamed headers, as read back.
    for name, path, cols, reader in [
        ("table7", "inhalation_HC2/Table_7_JAERI_dcf_inh_Public_Soluble_Reactive_Gases_Vapours.csv", T7_COLS, pd.read_csv),
        ("table5", "inhalation_HC2/Table_5_JAERI_dcf_inh_particulates_public.csv", T5_COLS, pd.read_csv),
        ("table_a2", "inhalation_HC2/Table_A2-DOE-STD-1196-2011_dcf_inhal.csv", A2_COLS, pd.read_csv),
        ("annex_g", "inhalation_HC2/Annex_G_ICRP119_dcf_inh_public.xlsx", AG_COLS, pd.read_excel),
        ("annex_f", "ingestion_public/AnnexF_ICRP119_dcf_ingestion_public.csv", ING_COLS, pd.read_csv),
        ("table4", "ingestion_public/table_4_jaeri_ingestion_public.csv", ING_COLS, pd.read_csv),
    ]:
        df = reader(os.path.join(lib, path))
        df.columns = cols
        df.to_csv(os.path.join(data_dir, f"pydoseia_synthetic_screening_{name}.csv"), index=False)


def quiet():
    return contextlib.redirect_stdout(io.StringIO())


def ing_config(base_config, names, mode, **kw):
    q = [1.0e10 * (1 + 0.3 * i) for i in range(len(names))]
    c = base_config(
        single_plume=mode == "single_plume", long_term_release=mode == "long_term", run_dose_computation=True,
        rads_list=list(names), element_list=[ING_ELEMENTS[n] for n in names], type_rad=["Max"] * len(names),
        instantaneous_release_bq_list=q, annual_discharge_bq_rad_list=q,
        animal_product_list_for_tritium=["cow_milk", "goat_meat"], animal_product_list_for_C14=["cow_milk", "goat_meat"],
        animal_feed_type="leafy_vegetables", veg_type_list="leafy_vegetables", climate="Continental",
        soiltype="peatsoil", inges_param_dict=None, inges_param_dict_adult=None, inges_param_dict_infant=None,
        run_plume_shine_dose=False, run_ps_dose_parallel=False)
    c.update(kw)
    return c


def run(emit, base_config, MetFunc, met_path, met_sheets, met_days):
    # PYDOSEIA_T2_PARTS (comma list) runs a subset, for development only; a
    # committed fixture is always generated with every part.
    parts = os.environ.get("PYDOSEIA_T2_PARTS", "quadpack,ingestion,plume_shine,screening,plume_rise,driver").split(",")
    if "quadpack" in parts:
        run_quadpack(emit)
    if "ingestion" in parts:
        run_ingestion(emit, base_config, MetFunc)
    if "plume_shine" in parts:
        run_plume_shine(emit, base_config, MetFunc, met_path, met_sheets, met_days)
    if "screening" in parts:
        run_screening(emit, base_config, MetFunc)
    if "plume_rise" in parts:
        run_plume_rise(emit, base_config, MetFunc)
    if "driver" in parts:
        run_driver(emit, base_config, met_path, met_sheets, met_days)


# ---------------------------------------------------------------------------
def quad_functions():
    return {
        "exp_neg": (lambda x: math.exp(-x), 0.0, 5.0),
        "inv_sqrt": (lambda x: 1.0 / math.sqrt(x), 0.0, 1.0),
        "log": (lambda x: math.log(x), 0.0, 1.0),
        "sin50": (lambda x: math.sin(50.0 * x), 0.0, 3.0),
        "runge": (lambda x: 1.0 / (1.0 + 25.0 * x * x), -1.0, 1.0),
        "inv_x": (lambda x: 1.0 / x, 0.0, 1.0),
        "spike": (lambda x: math.exp(-1.0e4 * (x - 0.3) ** 2), 0.0, 1.0),
    }


def run_quadpack(emit):
    from scipy.integrate import _quadpack, tplquad
    for name, (f, a, b) in quad_functions().items():
        for eps in [1.49e-8, 1.49e-3]:
            r = _quadpack._qagse(f, a, b, (), 1, eps, eps, 50)
            val, err, info, ier = r[0], r[1], r[2], r[3]
            inp = dict(func=name, a=a, b=b, eps=eps)
            emit("quadpack", inp, "value", val)
            emit("quadpack", inp, "abserr", err)
            emit("quadpack", inp, "ier", float(ier))
            emit("quadpack", inp, "neval", float(info["neval"]))
    v, _ = tplquad(lambda x, y, z: math.exp(-(x * x + 2.0 * y * y + 3.0 * z * z)) * (1.0 + x * y),
                   0.0, 1.5, -1.0, 2.0, -0.5, 1.0, epsabs=1.49e-4, epsrel=1.49e-4)
    emit("tplquad", dict(func="gauss_xy", eps=1.49e-4), "value", v)


# ---------------------------------------------------------------------------
ING_CASES = {
    "plain": ["SYN-1", "SYN-2", "SYN-3", "SYN-6", "SYN-7", "SYN-10"],
    "with_h3": ["SYN-1", "SYN-3", "SYN-6", "SYN-10", "H-3"],
    "with_c14": ["SYN-1", "SYN-3", "SYN-5", "SYN-4", "C-14"],
    "with_h3_c14": ["SYN-1", "SYN-2", "SYN-3", "H-3", "C-14"],
    "h3_only": ["H-3"],
    "c14_only": ["C-14"],
    "h3_c14_only": ["H-3", "C-14"],
    "h3_first": ["H-3", "SYN-1", "SYN-3"],
    "c14_middle": ["SYN-1", "C-14", "SYN-3"],
    "h_named_syn8": ["SYN-1", "SYN-3", "SYN-8"],
}


def ingestion_case_grid():
    """(case, mode, receiver, soil, params, chi, products_h3, products_c14, climate, veg, feed)."""
    out = []
    for case in ING_CASES:
        for mode, receiver, chi in itertools.product(["long_term", "single_plume"], ["adult", "infant"], [2.3e-6, 8.9e-5]):
            out.append((case, mode, receiver, "peatsoil", "fallback", chi, "milk_meat", "milk_meat", "Continental",
                        "leafy_vegetables", "leafy_vegetables"))
    # variations on the richest case
    for soil, params, ph3, pc14, clim, veg, feed in [
        ("othersoil", "ingen", "milk_meat", "milk_meat", "Maritime", "root_crops", "all_others"),
        ("peatsoil", "ingen", "milks_egg", "milk_only", "Arctic", "non_leafy_vegetables", "root_crops"),
        ("othersoil", "fallback", "milk_meat", "meat_only", "Mediterranean", "all_others", "non_leafy_vegetables"),
        ("peatsoil", "fallback", "meat_first", "milk_meat", "Continental", "leafy_vegetables", "leafy_vegetables"),
    ]:
        for mode, receiver in itertools.product(["long_term", "single_plume"], ["adult", "infant"]):
            out.append(("with_h3_c14", mode, receiver, soil, params, 4.1e-5, ph3, pc14, clim, veg, feed))
    return out


PRODUCT_LISTS = {
    "milk_meat": ["cow_milk", "goat_meat"],
    "milks_egg": ["goat_milk", "cow_milk", "beef_meat", "pork_meat", "egg"],
    "milk_only": ["cow_milk"],
    "meat_only": ["goat_meat", "lamb_meat"],
    "meat_first": ["broiler_meat", "cow_milk"],
}
INGEN_PARAMS = {'alpha_wet_crops': 0.3, 'alpha_dry_forage': 3, 't_e_food_crops': 60, 't_e_forage_grass': 30,
                't_b': 11000, 't_h_wet_crops': 14, 't_h_animal_pasture': 0, 't_h_animal_stored_feed': 90,
                'C_wi': 0, 'f_p': 0.7, 'alpha': 3, 't_e': 30, 't_m': 1, 't_f': 20, 'q_m': 16, 'q_w': 0.06,
                'q_f': 1.2, 'q_w_meat': 0.004}
INGEN_ADULT = {'DID_veg': 1.050, 'DID_milk': 0.500, 'DID_meat': 0.040, 'DID_fish': 0.050, 'DID_water_and_beverage': 0.002}
INGEN_INFANT = {'DID_veg': 0.215, 'DID_milk': 0.400, 'DID_meat': 0.0032876, 'DID_fish': 0.004109,
                'DID_water_and_beverage': 0.0007123}


def run_ingestion(emit, base_config, MetFunc):
    for (case, mode, receiver, soil, params, chi, ph3, pc14, clim, veg, feed) in ingestion_case_grid():
        names = ING_CASES[case]
        kw = dict(soiltype=soil, climate=clim, veg_type_list=veg, animal_feed_type=feed,
                  animal_product_list_for_tritium=PRODUCT_LISTS[ph3], animal_product_list_for_C14=PRODUCT_LISTS[pc14])
        if params == "ingen":
            kw.update(inges_param_dict=INGEN_PARAMS, inges_param_dict_adult=INGEN_ADULT, inges_param_dict_infant=INGEN_INFANT)
        m = MetFunc(None, ing_config(base_config, names, mode, **kw), "x")
        inp = dict(case=case, mode=mode, receiver=receiver, soil=soil, params=params, chi=chi, h3=ph3, c14=pc14,
                   climate=clim, veg=veg, feed=feed)
        try:
            with quiet():
                r = m.ingestion_dose(100, receiver=receiver, max_dilutfac_for_distance_secperm3=chi)
        except (IndexError, NameError) as e:
            emit("ingestion", inp, "raises", 1.0)
            continue
        if r is None:
            emit("ingestion", inp, "returns_none", 1.0)
            continue
        a = np.array(r, dtype=float)
        emit("ingestion", inp, f"shape_{a.shape[0]}x{a.shape[1]}", a.flatten())
    # dcf_list_ingestion
    names = ["SYN-1", "SYN-3", "H-3", "C-14", "SYN-99"]
    m = MetFunc(None, ing_config(base_config, ["SYN-1"], "long_term"), "x")
    m.rads_list = names
    for age in [0.5, 1, 1.5, 2, 5, 7, 10, 12, 15, 17, 18, 40]:
        with quiet():
            d = m.dcf_list_ingestion(age=age)
        flat = []
        for x in d:
            flat.extend(x if isinstance(x, list) else [x])
        emit("dcf_ingestion", dict(age=age), "dcf", flat)
    # fv_list_ecerman_ingestion over an element list with decoys and a missing element
    m.element_list = ["Co", "H", "Cs", "Sr", "Y", "Ru", "C", "Kr", "Rb"]
    with quiet():
        ls, lw, f1, f2, fm, ff, no = m.fv_list_ecerman_ingestion()
    for k, v in (("lambda_s", ls), ("lambda_w", lw), ("fv1", f1), ("fv2", f2), ("fm", fm), ("ff", ff)):
        emit("eco_lookup", dict(elements="Co|H|Cs|Sr|Y|Ru|C|Kr|Rb"), k, list(v))
    emit("eco_lookup", dict(elements="Co|H|Cs|Sr|Y|Ru|C|Kr|Rb"), "no_transfer:" + "|".join(no), 1.0)
    # unused ingestion_weathering_correction
    m = MetFunc(None, ing_config(base_config, ["SYN-1", "SYN-2", "SYN-4", "SYN-8", "SYN-5"], "long_term"), "x")
    with quiet():
        m.find_half_life_and_decay_const_radionuclides()
    for w in [False, True]:
        m.config["weathering_corr"] = w
        with quiet():
            emit("ingestion_weathering_unused", dict(weathering=w), "lambda", m.ingestion_weathering_correction())
    # zeroing_ingestion under this pandas
    m = MetFunc(None, ing_config(base_config, ["SYN-1", "SYN-10"], "long_term"), "x")
    df = pd.DataFrame([[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]], index=["SYN-1", "SYN-10"], columns=["Veg", "Milk", "Meat"])
    with quiet():
        out = m.zeroing_ingestion(df.copy(), ["Ru"])
    emit("zeroing", dict(pandas=pd.__version__), "table", out.values.flatten())


# ---------------------------------------------------------------------------
def run_plume_shine(emit, base_config, MetFunc, met_path, met_sheets, met_days):
    rads = ["SYN-1", "SYN-3", "SYN-4", "SYN-6", "SYN-2"]
    cfg = base_config(run_dose_computation=True, rads_list=rads, element_list=["Co", "Cs", "Kr", "Sr", "I"],
                      type_rad=["Max"] * 5, release_height=30.0)
    m = MetFunc(None, cfg, "x")
    with quiet():
        e, p, en, neg = m.gamma_energy_abundaces()
        e2, p2 = m.add_zero_energy_for_pure_beta(e, p)
    for i, r in enumerate(rads):
        emit("gamma_lines", dict(nuclide=r), "energy_mev", e2[i])
        emit("gamma_lines", dict(nuclide=r), "probability", p2[i])
    energies = [0.0, 0.005, 0.01, 0.0153, 0.1, 0.5, 0.812, 1.0, 1.255, 4.2, 5.0, 6.5]
    with quiet():
        d = m.get_k_mu_mua_MFP([energies])
    for en_ in energies:
        emit("attenuation", dict(energy=en_), "k_mu_mua_mfp", list(d[en_]))
    for s, x, h, mfp in itertools.product(range(1, 7), [50.0, 150.0, 800.0, 2500.0], [10.0, 30.0, 90.0], [60.0, 400.0]):
        m.release_height = h
        emit("limits_single", dict(stab=s, x=x, h=h, mfp=mfp), "limits", m.zyx_lim_for_integral_single_plume(s, x, mfp))
        emit("limits_sector", dict(stab=s, x=x, h=h, mfp=mfp), "limits",
             m.zyx_lim_for_integral_sector_averaged_plume(s, x, mfp))
        emit("limits_legacy", dict(stab=s, x=x, h=h, mfp=mfp, n=3), "limits", m.zyx_lim_for_integral(s, x, mfp, n=3))
    # plume shine, three modes
    ps_rads = ["SYN-1", "SYN-3", "SYN-6"]
    q = [2.0e12, 5.0e11, 1.0e12]
    common = dict(run_dose_computation=True, rads_list=ps_rads, element_list=["Co", "Cs", "Sr"], type_rad=["Max"] * 3,
                  instantaneous_release_bq_list=q, annual_discharge_bq_rad_list=q, run_plume_shine_dose=True,
                  run_ps_dose_parallel=False, release_height=30.0, measurement_height=10.0)
    for x, y, z in [(200.0, 0.0, 0.0), (900.0, 25.0, 1.5)]:
        m = MetFunc(None, base_config(single_plume=True, long_term_release=False, Y=y, Z=z, **common), "x")
        m.TJFD_ALL_MISSING_CORR = m.synthetic_TJFD_for_single_plume()
        with quiet():
            r = m.plumeshine_dose(spatial_distance=x)
        emit("plume_shine_single", dict(x=x, y=y, z=z), "per_class", np.array(r, dtype=float).flatten())
    for x, z in [(200.0, 0.0), (900.0, 1.5)]:
        m = MetFunc(None, base_config(single_plume=False, long_term_release=True, Z=z, **common), "x")
        m.TJFD_ALL_MISSING_CORR = 1  # anything non-None: upstream then skips the met reading
        with quiet():
            r = m.plumeshine_dose(spatial_distance=x)
        emit("plume_shine_long_term", dict(x=x, z=z), "per_class", np.array(r, dtype=float).flatten())
    cfg = base_config(single_plume=False, long_term_release=True, Z=0.0, have_met_data=True, path_met_file=met_path,
                      excel_sheet_name=met_sheets, num_days=met_days,
                      column_names=["HOUR", "WS 10m(kmph)", "DIR at 10m", "STBCLASS"], **common)
    m = MetFunc(None, cfg, "x")
    with quiet():
        m.file_preprocessing()
        m.met_data_to_tjfd()
        m.missing_correction()
        m.operation_hours_per_day = abs(m.config["start_operation_time"] - m.config["end_operation_time"])
        r = m.plumeshine_dose(spatial_distance=400.0)
    emit("plume_shine_met", dict(x=400.0), "per_sector", np.array(r, dtype=float).flatten())
    # module-level point_source_dose
    import raddcffunc
    for unit in ["mSv/hr", "mR/hr"]:
        dd = raddcffunc.point_source_dose(unit=unit)
        emit("point_source", dict(case="defaults", unit=unit), "dose", [dd[k] for k in sorted(dd)])
        dd = raddcffunc.point_source_dose(gamma_energy=[662.0, 1173.0], g_yield=[0.85, 0.5], activity_curie=12.5,
                                          dist_list=[3, 7.5, 250], damage_ratio=1.0, unit=unit)
        emit("point_source", dict(case="custom", unit=unit), "dose", [dd[k] for k in sorted(dd)])


# ---------------------------------------------------------------------------
def run_screening(emit, base_config, MetFunc):
    names = ["SCR-1", "SCR-2", "SCR-3"]
    for ty, age in itertools.product(["Max", "F", "M", "S", "V"], [1, 1.5, 5, 10, 15, 30]):
        cfg = base_config(run_dose_computation=True, rads_list=names, element_list=["Xa", "Xb", "Xc"],
                          type_rad=[ty] * 3)
        m = MetFunc(None, cfg, "x")
        with quiet():
            res = m.get_dcfs_for_radionuclides(age)
        for n in names:
            d = res[n]
            if d is None:
                emit("dcf_screening", dict(nuclide=n, type=ty, age=age), "none", 1.0)
                continue
            v = [np.nan if d.get(k) is None else d.get(k) for k in ("max_dcf_inh_public", "max_dcf_ing_public")]
            flags = "".join("n" if d.get(k) is None else "v" for k in ("max_dcf_inh_public", "max_dcf_ing_public"))
            emit("dcf_screening", dict(nuclide=n, type=ty, age=age), "inh_ing:" + flags, v)


def run_plume_rise(emit, base_config, MetFunc):
    m = MetFunc(None, base_config(), "x")
    emit("plume_rise", dict(case="neutral_defaults"), "dh", m.compute_plume_rise_neutral_unstable_cat())
    emit("plume_rise", dict(case="stable_defaults"), "dh", m.compute_plume_rise_stable_cat())
    for w0, x, u, di, de in itertools.product([2.0, 10.0, 25.0], [50.0, 300.0], [1.0, 4.5], [1.5, 5.0], [2.0, 8.0]):
        emit("plume_rise", dict(case="neutral", w0=w0, x=x, u=u, di=di, de=de), "dh",
             m.compute_plume_rise_neutral_unstable_cat(W0=w0, x=x, U=u, D_i=di, D_e=de))
    for w0, u, di in itertools.product([2.0, 10.0, 25.0], [1.0, 4.5], [1.5, 5.0]):
        emit("plume_rise", dict(case="stable", w0=w0, u=u, di=di), "dh",
             m.compute_plume_rise_stable_cat(W0=w0, U=u, D_i=di))


# ---------------------------------------------------------------------------
DRIVER_SCENARIOS = {
    "long_term_no_met": dict(single_plume=False, long_term_release=True, have_met_data=False,
                             rads=["SYN-1", "SYN-3", "SYN-2", "SYN-10"], ages=[1, 18], dists=[150, 800], boundary=500),
    "single_plume_h3_c14": dict(single_plume=True, long_term_release=False, have_met_data=False,
                                rads=["SYN-1", "SYN-3", "H-3", "C-14"], ages=[1, 18], dists=[150, 800], boundary=800),
    "long_term_met": dict(single_plume=False, long_term_release=True, have_met_data=True,
                          rads=["SYN-1", "SYN-6"], ages=[18], dists=[300], boundary=1200),
}


def driver_config(base_config, sc, met_path, met_sheets, met_days, **kw):
    rads = sc["rads"]
    q = [3.0e11 * (1 + 0.4 * i) for i in range(len(rads))]
    c = base_config(
        single_plume=sc["single_plume"], long_term_release=sc["long_term_release"], have_met_data=sc["have_met_data"],
        run_dose_computation=True, rads_list=list(rads), element_list=[ING_ELEMENTS[n] for n in rads],
        type_rad=["Max"] * len(rads), instantaneous_release_bq_list=q, annual_discharge_bq_rad_list=q,
        downwind_distances=list(sc["dists"]), plant_boundary=sc["boundary"], age_group=list(sc["ages"]),
        release_height=30, measurement_height=10, consider_progeny=True, weathering_corr=True,
        animal_product_list_for_tritium=["cow_milk", "goat_meat"], animal_product_list_for_C14=["cow_milk", "goat_meat"],
        animal_feed_type="leafy_vegetables", veg_type_list="leafy_vegetables", climate="Continental",
        soiltype="peatsoil", inges_param_dict=INGEN_PARAMS, inges_param_dict_adult=INGEN_ADULT,
        inges_param_dict_infant=INGEN_INFANT, run_plume_shine_dose=False, run_ps_dose_parallel=False,
        pickle_it=False, logdir_name=".", input_file_name="c2c", path_met_file=met_path,
        excel_sheet_name=met_sheets, num_days=met_days, column_names=["HOUR", "WS 10m(kmph)", "DIR at 10m", "STBCLASS"],
        calm_correction=True)
    c.update(kw)
    return c


def run_driver(emit, base_config, met_path, met_sheets, met_days):
    from outputfunc import OutputFunc
    import main as upstream_main
    for name, sc in DRIVER_SCENARIOS.items():
        cfg = driver_config(base_config, sc, met_path, met_sheets, met_days)
        o = OutputFunc(None, cfg, "c2c_driver.log")
        with quiet():
            DCFs, dil, DOSES, ING, *rest = o.dose_calculation_script()
        dists = list(o.downwind_distances)
        emit("driver", dict(scenario=name), "distances", dists)
        for i, dd in enumerate(dists):
            emit("driver", dict(scenario=name, distance=dd), "dilution", np.array(dil[i], dtype=float))
            emit("driver", dict(scenario=name, distance=dd), "max_chi",
                 float(o.max_dil_fac_all_dist.loc[str(dd)].iloc[0]))
            for j, ag in enumerate(sc["ages"]):
                emit("driver", dict(scenario=name, distance=dd, age=ag), "doses",
                     np.array(DOSES[i][j], dtype=float).flatten())
                emit("driver", dict(scenario=name, distance=dd, age=ag), "ingestion",
                     np.array(ING[i][j], dtype=float).flatten())
        # the coefficients the report prints: screened inhalation/ingestion
        # (None here: no SYN nuclide is in the screening tables), and the
        # ground-surface pair with progeny forced on, the submersion pair
        for j, ag in enumerate(sc["ages"]):
            gs = [v for pair in DCFs[j][1] for v in pair]
            sub = [v for pair in DCFs[j][2] for v in pair]
            emit("driver_dcf_report", dict(scenario=name, age=ag), "surface_pairs", gs)
            emit("driver_dcf_report", dict(scenario=name, age=ag), "submersion_pairs", sub)
        # main.py summaries
        with quiet():
            df_ing = upstream_main.reshape_ingestion_dose_data(ING, sc["ages"], list(sc["dists"]), sc["rads"],
                                                               sc["boundary"])
            df, df_sum = upstream_main.reshape_dose_data(DOSES, df_ing, sc["ages"], list(sc["dists"]), sc["rads"],
                                                         sc["boundary"])
        for _, r in df_sum.iterrows():
            emit("driver_summary", dict(scenario=name, distance=r["Distance (m)"], age=r["Age (y)"]), "inh_gs_sub_ing_total",
                 [r["Inhalation dose (mSv)"], r["Ground-shine (mSv)"], r["Submersion (mSv)"],
                  r["Ingestion dose (mSv)"], r["Total Dose (mSv)"]])
        # the text report's plant-boundary totals (printed at pandas' default precision)
        with quiet():
            o.output_to_txt(dil, filename="c2c_report.txt", DCFs=DCFs, DOSES=DOSES, INGESTION_DOSES=ING,
                            PLUME_DOSES=None)
        for ag, rows in parse_boundary_totals("c2c_report.txt", sc["rads"]).items():
            for rad, vals in rows.items():
                emit("boundary_totals_text", dict(scenario=name, age=ag, nuclide=rad), "inh_gs_sub_ing_total", vals)
    # driver failure modes
    sc = dict(DRIVER_SCENARIOS["long_term_no_met"], ages=[1, 10])
    o = OutputFunc(None, driver_config(base_config, sc, met_path, met_sheets, met_days), "c2c_driver.log")
    try:
        with quiet():
            o.dose_calculation_script()
        emit("driver_raises", dict(case="age_10"), "raises", 0.0)
    except Exception:  # joblib re-raises the worker's UnboundLocalError
        emit("driver_raises", dict(case="age_10"), "raises", 1.0)
    # a user-supplied maximum dilution factor per distance (MetFunc stores it
    # as dict_max_dilution_factor and every pathway reads it by distance)
    user = {150: 1.0e-5, 800: 2.0e-6, 500: 3.0e-6}
    cfg = driver_config(base_config, DRIVER_SCENARIOS["long_term_no_met"], met_path, met_sheets, met_days,
                        have_dilution_factor=True, list_max_dilution_factor=user)
    o = OutputFunc(None, cfg, "c2c_driver.log")
    with quiet():
        DCFs, dil, DOSES, ING, *rest = o.dose_calculation_script()
    for i, dd in enumerate(o.downwind_distances):
        for j, ag in enumerate(cfg["age_group"]):
            inp = dict(scenario="user_dilution_factor", distance=dd, age=ag, chi=user[dd])
            emit("driver", inp, "doses", np.array(DOSES[i][j], dtype=float).flatten())
            emit("driver", inp, "ingestion", np.array(ING[i][j], dtype=float).flatten())


def parse_boundary_totals(path, rads):
    """The 'RESULTS OF TOTAL DOSE COMPUTATION AT PLANT BOUNDARY' tables."""
    with open(path) as f:
        text = f.read()
    part = text.split("RESULTS OF TOTAL DOSE COMPUTATION AT PLANT BOUNDARY")[1]
    out = {}
    age = None
    for line in part.splitlines():
        if line.startswith("Calculated total dose values"):
            age = line.split(" for age ")[1].split(" ")[0]
            out[age] = {}
            continue
        toks = line.split()
        if age is not None and toks and toks[0] in rads and len(toks) == 6:
            out[age][toks[0]] = [float(t) for t in toks[1:]]
    return out
