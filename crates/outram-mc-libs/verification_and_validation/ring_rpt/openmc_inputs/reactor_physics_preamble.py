"""Reactor-physics capture for OpenMC runs — the Python counterpart of
``outram-mc-libs``' ``physics::reactor_physics`` helper.

Attach one combined tally to a model, run it, and read back:

  1. the SIX-FACTOR FORMULA (eta, f, p, epsilon, P_FNL, P_TNL) with 1-sigma
     uncertainties, on a THREE-energy-group split (fast / resonance / thermal),
     and
  2. the LETHARGY-NORMALISED neutron flux spectrum over the whole material
     domain.

Both come from a SINGLE tally:

    filters = [EnergyFilter(fine grid), MaterialFilter(all materials)]
    scores  = ['flux', 'absorption', 'nu-fission']

so the spectrum and the group-collapsed six-factor rates are the same numbers,
read two ways.

WHY THREE GROUPS
----------------
The classical six-factor formula is two-group (thermal vs "fast"), which leaves
the *resonance escape probability* p as a bare ratio with no resonance region.
Keeping a genuine resonance group between two boundaries makes

    p = (A_thermal + L_thermal) / (A_thermal + L_thermal + A_res + L_res)

the textbook quantity. Fast absorption is folded into the fast-fission factor
epsilon. The factors telescope EXACTLY to

    eta * f * p * epsilon * P_FNL * P_TNL = P_total / (A_total + L_total) = k

so the product is checked against the k OpenMC reported; a large gap, or one in
the wrong direction, means a mis-specified tally.

RELATIONSHIP TO THE RUST HELPER
-------------------------------
The formulas here are identical to ``outram-mc-libs/src/physics/reactor_physics.rs``.
Both sides now score the SAME absorption quantity: OpenMC's ``absorption`` is
MT=27 (non-redundant disappearance reactions + fission), and the Rust crate's
``ScoreType::Absorption`` uses ``MacroXs::absorption`` = capture + fission (bead
op-mzvp.2.9). So the two consistency gaps should agree; a divergence is a
finding about one code's cross-section reconstruction, not a known mis-count.

Group-resolved leakage L_g needs a surface-current tally on the domain
boundary; for a REFLECTIVE model every L_g is 0 and that tally is skipped. The
scalar total leakage is read from ``sp.global_tallies`` regardless and asserted
~0 for reflective models.

LICENCE
-------
BSD 3-Clause, (c) 2024 theodoreOnzGit. Part of openmc_fuel_perf_project.
"""

from dataclasses import dataclass

import numpy as np
import openmc


# ---------------------------------------------------------------------------
# Config — mirrors ReactorPhysicsConfig
# ---------------------------------------------------------------------------

@dataclass
class ReactorPhysicsConfig:
    #: Thermal-group upper edge / resonance-group lower edge [eV].
    thermal_cutoff_ev: float = 0.625
    #: Resonance-group upper edge / fast-group lower edge [eV].
    resonance_upper_ev: float = 1.0e5
    #: Log-spaced fine bins before the two boundaries are forced onto edges.
    n_fine_bins: int = 500
    #: Fine-grid low edge [eV].
    energy_min_ev: float = 1.0e-3
    #: Fine-grid high edge [eV].
    energy_max_ev: float = 2.0e7


class BadEnergyGrid(ValueError):
    """energy_min < thermal_cutoff < resonance_upper < energy_max was violated."""


class NoFuelMaterial(RuntimeError):
    """No uranium-bearing material found (need a nuclide named 'U23*')."""


# ---------------------------------------------------------------------------
# Energy grid
# ---------------------------------------------------------------------------

def _insert_edge(edges, target):
    """Insert ``target`` into ascending ``edges``, or snap the nearest edge to it
    if within 1e-9 relative. Returns a new ndarray."""
    edges = np.asarray(edges, dtype=float)
    pos = int(np.searchsorted(edges, target))
    for j in (pos - 1, pos):
        if 0 <= j < len(edges) and abs(edges[j] - target) <= 1e-9 * target:
            edges = edges.copy()
            edges[j] = target
            return edges
    return np.insert(edges, pos, target)


def fine_energy_grid(cfg: ReactorPhysicsConfig) -> np.ndarray:
    """Log grid with the two group boundaries forced onto edges (ascending)."""
    if not (cfg.energy_min_ev < cfg.thermal_cutoff_ev
            < cfg.resonance_upper_ev < cfg.energy_max_ev
            and cfg.n_fine_bins >= 3):
        raise BadEnergyGrid(
            "need energy_min < thermal_cutoff < resonance_upper < energy_max "
            "and n_fine_bins >= 3")
    edges = np.logspace(np.log10(cfg.energy_min_ev),
                        np.log10(cfg.energy_max_ev),
                        cfg.n_fine_bins + 1)
    edges = _insert_edge(edges, cfg.thermal_cutoff_ev)
    edges = _insert_edge(edges, cfg.resonance_upper_ev)
    return edges


def _group_indices(edges, cfg):
    """Boolean masks (thermal, resonance, fast) over the fine bins, keyed on each
    bin's UPPER edge — every fine bin lies wholly in one group."""
    hi = edges[1:]
    ct = cfg.thermal_cutoff_ev
    cr = cfg.resonance_upper_ev
    thermal = hi <= ct * (1.0 + 1e-9)
    fast = hi > cr * (1.0 + 1e-9)
    resonance = ~thermal & ~fast
    return thermal, resonance, fast


# ---------------------------------------------------------------------------
# Fuel identification — verbatim from pipeline_triso_to_rpt.py
# ---------------------------------------------------------------------------

def _fuel_materials(model):
    """Every material containing a nuclide named 'U23*' -- i.e. the fuel."""
    fuel = []
    for material in model.materials:
        names = [n[0] if isinstance(n, tuple) else n
                 for n in material.get_nuclides()]
        if any(n.startswith('U23') for n in names):
            fuel.append(material)
    if not fuel:
        raise NoFuelMaterial(
            "no uranium-bearing material found -- cannot compute the "
            "thermal-utilisation factor f")
    return fuel


# ---------------------------------------------------------------------------
# Tally construction
# ---------------------------------------------------------------------------

def add_reactor_physics_tallies(model, cfg: ReactorPhysicsConfig):
    """Attach the single combined tally the capture needs. Modifies ``model`` in
    place and returns the fine energy grid used."""
    edges = fine_energy_grid(cfg)
    tally = openmc.Tally(name='reactor_physics')
    tally.filters = [
        openmc.EnergyFilter(edges),
        openmc.MaterialFilter(list(model.materials)),
    ]
    tally.scores = ['flux', 'absorption', 'nu-fission']
    existing = list(model.tallies) if model.tallies is not None else []
    model.tallies = openmc.Tallies(existing + [tally])
    return edges


# ---------------------------------------------------------------------------
# Read-back
# ---------------------------------------------------------------------------

def _ratio(num, num_sd, den, den_sd):
    """num/den with delta-method 1-sigma, independence assumed (conservative for
    the nested six-factor ratios). Returns (mean, std)."""
    if den == 0.0:
        return 0.0, 0.0
    m = num / den
    rel = np.hypot(num_sd / num if num else 0.0, den_sd / den if den else 0.0)
    return m, abs(m) * rel


def six_factors_from_statepoint(sp_path, model, cfg: ReactorPhysicsConfig,
                                leakage_by_group=None):
    """Compute the three-group six factors from a finished run.

    ``leakage_by_group`` is an optional length-3 array [L_thermal, L_res, L_fast]
    of neutrons-per-source-neutron; if ``None`` all L_g are taken as 0 (a
    reflective model) and P_FNL = P_TNL = 1.

    Returns a dict with each factor as (mean, std), the by-group rates, the
    telescoping product ``k_from_factors``, ``k_from_openmc``, ``consistency_gap``
    and a ``consistent`` flag.
    """
    edges = fine_energy_grid(cfg)
    thermal, resonance, fast = _group_indices(edges, cfg)
    fuel = _fuel_materials(model)
    fuel_ids = {m.id for m in fuel}
    n_e = len(edges) - 1

    with openmc.StatePoint(sp_path) as sp:
        tally = sp.get_tally(name='reactor_physics')
        mat_bins = tally.find_filter(openmc.MaterialFilter).bins
        n_mat = len(mat_bins)

        def series(score):
            v = tally.get_values(scores=[score]).reshape(n_e, n_mat)
            s = tally.get_values(scores=[score], value='std_dev').reshape(n_e, n_mat)
            return v, s

        flux, _ = series('flux')
        absn, absn_sd = series('absorption')
        prod, prod_sd = series('nu-fission')

        fuel_col = np.array([mid in fuel_ids for mid in mat_bins])
        k_from_openmc = float(sp.keff.nominal_value)

    def group_sum(v, sd, mask, cols=None):
        vv = v[mask][:, cols] if cols is not None else v[mask]
        ss = sd[mask][:, cols] if cols is not None else sd[mask]
        return float(vv.sum()), float(np.sqrt((ss ** 2).sum()))

    masks = {'thermal': thermal, 'resonance': resonance, 'fast': fast}
    A = {g: group_sum(absn, absn_sd, m) for g, m in masks.items()}
    P = {g: group_sum(prod, prod_sd, m) for g, m in masks.items()}
    A_fuel_t = group_sum(absn, absn_sd, thermal, np.where(fuel_col)[0])

    if leakage_by_group is None:
        L = {'thermal': (0.0, 0.0), 'resonance': (0.0, 0.0), 'fast': (0.0, 0.0)}
    else:
        L = {g: (float(leakage_by_group[i]), 0.0)
             for i, g in enumerate(('thermal', 'resonance', 'fast'))}

    A_tot = sum(A[g][0] for g in masks)
    P_tot = sum(P[g][0] for g in masks)
    L_tot = sum(L[g][0] for g in masks)

    s_t = A['thermal'][0] + L['thermal'][0]
    s_tr = s_t + A['resonance'][0] + L['resonance'][0]
    s_trf = s_tr + L['fast'][0]

    eta = _ratio(P['thermal'][0], P['thermal'][1], *A_fuel_t)
    f = _ratio(A_fuel_t[0], A_fuel_t[1], A['thermal'][0], A['thermal'][1])
    p_tnl = _ratio(A['thermal'][0], A['thermal'][1], s_t, 0.0)
    p_esc = _ratio(s_t, 0.0, s_tr, 0.0)
    p_fnl = _ratio(s_tr, 0.0, s_trf, 0.0)
    mult, mult_sd = _ratio(P_tot, np.sqrt(sum(P[g][1] ** 2 for g in masks)),
                           P['thermal'][0], P['thermal'][1])
    absorb_frac = (A_tot + L_tot - A['fast'][0]) / (A_tot + L_tot) if (A_tot + L_tot) else 0.0
    epsilon = (mult * absorb_frac, abs(mult * absorb_frac) * (mult_sd / mult if mult else 0.0))

    def prod_est(*pairs):
        m = np.prod([p[0] for p in pairs])
        rel = np.sqrt(sum((p[1] / p[0]) ** 2 for p in pairs if p[0]))
        return float(m), float(abs(m) * rel)

    k_from_factors = prod_est(eta, f, p_esc, epsilon, p_fnl, p_tnl)
    gap = (k_from_openmc - k_from_factors[0]) / k_from_openmc if k_from_openmc else float('nan')

    return {
        'eta': eta, 'f': f, 'p': p_esc, 'epsilon': epsilon,
        'P_FNL': p_fnl, 'P_TNL': p_tnl,
        'k_from_factors': k_from_factors,
        'k_from_openmc': k_from_openmc,
        'absorption_by_group': A, 'production_by_group': P, 'leakage_by_group': L,
        'thermal_absorption_fuel': A_fuel_t,
        'group_bounds_ev': (cfg.thermal_cutoff_ev, cfg.resonance_upper_ev),
        'consistency_gap': gap,
        # Same band as the Rust helper's CONSISTENCY_BAND; residual is (n,2n).
        'consistent': bool(-0.01 < gap < 0.05),
    }


def lethargy_spectrum_from_statepoint(sp_path, model, cfg: ReactorPhysicsConfig):
    """Lethargy-normalised flux spectrum over the material domain.

    Returns a dict with ``energy_edges_ev``, ``flux_per_lethargy`` (mean, std
    arrays, normalised so sum(psi_i * du_i) == 1), ``flux_raw`` and
    ``flux_total``.
    """
    edges = fine_energy_grid(cfg)
    n_e = len(edges) - 1
    with openmc.StatePoint(sp_path) as sp:
        tally = sp.get_tally(name='reactor_physics')
        n_mat = len(tally.find_filter(openmc.MaterialFilter).bins)
        flux = tally.get_values(scores=['flux']).reshape(n_e, n_mat).sum(axis=1)
        flux_sd = np.sqrt((tally.get_values(scores=['flux'], value='std_dev')
                           .reshape(n_e, n_mat) ** 2).sum(axis=1))

    du = np.log(edges[1:] / edges[:-1])
    total = flux.sum()
    with np.errstate(divide='ignore', invalid='ignore'):
        psi = np.where((flux > 0) & (total > 0), flux / (du * total), 0.0)
        psi_sd = np.where(flux > 0, psi * (flux_sd / flux), 0.0)
    return {
        'energy_edges_ev': edges,
        'flux_per_lethargy': (psi, psi_sd),
        'flux_raw': flux,
        'flux_total': float(total),
    }


def capture_reactor_physics(model, cfg: ReactorPhysicsConfig = None,
                            leakage_by_group=None):
    """Convenience wrapper: attach tallies, run, and return
    ``(six_factors, lethargy_spectrum, statepoint_path)``."""
    cfg = cfg or ReactorPhysicsConfig()
    add_reactor_physics_tallies(model, cfg)
    sp_path = model.run()
    return (
        six_factors_from_statepoint(sp_path, model, cfg, leakage_by_group),
        lethargy_spectrum_from_statepoint(sp_path, model, cfg),
        sp_path,
    )
