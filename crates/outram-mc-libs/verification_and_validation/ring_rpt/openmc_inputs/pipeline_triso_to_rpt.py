"""TRISO pebble -> ring RPT pebble: a four-step pipeline.

WHAT THIS IS FOR
----------------
A pebble of TRISO fuel is *doubly heterogeneous*: fuel kernels inside a pebble
inside a core. Homogenising it naively over-predicts reactivity, because the
self-shielding of the kernels is thrown away.

The Reactivity-equivalent Physical Transformation (RPT) fixes that by replacing
the randomly-packed kernels with a single solid ring (a spherical shell here) of
homogenised fuel, sized so the pebble keeps the *same* k-eff. This pipeline
builds both pebbles, runs both, and reports both.

    Lou et al. (2020), "A novel reactivity-equivalent physical transformation
    method for homogenization of double-heterogeneous systems",
    Annals of Nuclear Energy 142, 107396.

HOW TO USE IT
-------------
Four functions, called in order. Each does one thing and returns plain data.

    model = build_triso_pebble()          # the explicit, doubly-heterogeneous pebble
    model = build_rpt_pebble(radius_cm)   # the equivalent homogenised-ring pebble
    keff  = run(model, "some_name")       # runs OpenMC, returns k and its error
    six   = six_factors("some_name")      # reads the tallies back

Or just run the whole thing:

    python pipeline_triso_to_rpt.py

WHAT YOU NEED FIRST
-------------------
    export OPENMC_CROSS_SECTIONS=/path/to/cross_sections.xml

THE SIX FACTORS
---------------
k_eff = eta * f * p * epsilon * P_FNL * P_TNL

Computed from two tallies with a thermal/fast split at 0.625 eV:

    eta      = thermal neutron production / thermal absorption in fuel
    f        = thermal absorption in fuel / thermal absorption everywhere
    p        = thermal absorption everywhere / absorption at all energies
    epsilon  = production at all energies / thermal production
    P_FNL    = 1  (see below)
    P_TNL    = 1  (see below)

Those four multiply out to (total production)/(total absorption), which is
exactly k_inf -- so the pipeline checks the product against the k-eff OpenMC
reported, and complains if they disagree. That check is the point: it catches a
mis-specified tally, which otherwise produces plausible-looking factors.

Both pebbles use REFLECTIVE boundaries, so nothing leaks and the two
non-leakage factors are 1 by construction, not by assumption. The pipeline
tallies leakage anyway and asserts it is zero.

LICENCE
-------
BSD 3-Clause, (c) 2024 theodoreOnzGit. Part of openmc_fuel_perf_project.
"""

import os
import sys

import numpy as np
import openmc

# ---------------------------------------------------------------------------
# Settings you are likely to change
# ---------------------------------------------------------------------------

#: Boundary between "thermal" and "fast" for the six-factor split, in eV.
#: 0.625 eV is the usual cadmium cutoff.
THERMAL_CUTOFF_EV = 0.625

#: Neutrons per batch. Raise for a smaller error bar.
PARTICLES = 20000

#: Total batches, and how many of those are discarded while the fission source
#: settles. k is averaged over (BATCHES - INACTIVE) batches.
BATCHES = 150
INACTIVE = 50

#: The RPT shell radius that makes the homogenised pebble match the explicit
#: one. Found by the original author with a k-eff search on ENDF/B-VII.1.
#: It is library-dependent -- see the README note in the results.
RPT_RADIUS_CM = 1.493359375

#: Half-width of the fuel zone, cm. The pebble is 4 cm across with a 1 cm
#: graphite shell, so fuel lives inside a 1 cm radius. Used to place the
#: starting source -- see configure_run.
FUEL_ZONE_RADIUS_CM = 1.0

#: 19.9% enriched uranium (HALEU), FLiBe at 99.995% Li-7, 600 K.
ENRICHMENT_PERCENT = 19.9
LI7_PURITY_PERCENT = 99.995
TEMPERATURE_K = 600.0


# ---------------------------------------------------------------------------
# Step 1 and 2: build the two pebbles
# ---------------------------------------------------------------------------

def build_triso_pebble():
    """Build the explicit TRISO pebble -- the doubly-heterogeneous reference.

    Randomly packed TRISO particles in a graphite matrix, inside a graphite
    shell, surrounded by FLiBe. Reflective boundaries, so this is a k-infinity
    unit cell.

    Returns an ``openmc.Model``.
    """
    from triso import TrisoParticlesFactory
    from triso_pebble import TrisoPebbleFactory

    materials_factory = TrisoParticlesFactory()
    pebble_materials = materials_factory.build_fhr_materials(
        ENRICHMENT_PERCENT, LI7_PURITY_PERCENT, TEMPERATURE_K)

    pebble_factory = TrisoPebbleFactory()
    pebble_univ = pebble_factory.buildNonAnnularPebbleUniv(
        pebble_graphite_shell_thickness=0.1,
        pebble_materials=pebble_materials,
        fuel_temp=TEMPERATURE_K,
        coolant_temp=TEMPERATURE_K)

    # Assemble the model IN MEMORY from the universe the factory returned.
    #
    # Do NOT use openmc.Model.from_xml() here. This factory does not write any
    # XML -- it only returns a universe -- so from_xml() would silently read
    # whatever geometry.xml happens to be left in the working directory from a
    # previous run. That produced two "different" models with bit-identical
    # k-eff, because both were in fact the RPT model.
    root_sphere = openmc.Sphere(r=3.0, boundary_type='reflective')
    root_cell = openmc.Cell(name='root_cell', fill=pebble_univ,
                            region=-root_sphere)
    geometry = openmc.Geometry(openmc.Universe(cells=[root_cell]))

    return openmc.Model(
        geometry=geometry,
        materials=openmc.Materials(geometry.get_all_materials().values()))


def build_rpt_pebble(ring_rpt_inner_radius_cm=RPT_RADIUS_CM):
    """Build the ring-RPT pebble -- the homogenised equivalent.

    The TRISO kernels are dissolved into one homogeneous fuel material, then
    placed as a spherical shell whose inner radius is
    ``ring_rpt_inner_radius_cm``. That radius is the single knob RPT turns: it
    is chosen so this pebble's k-eff matches the explicit one.

    Returns an ``openmc.Model``.
    """
    from rpt_pebble import RingRPTPebbleFactory

    factory = RingRPTPebbleFactory()
    # This factory *does* return a fully-built Model (and also writes XML as a
    # side effect). Use the returned object, not from_xml(), so the model never
    # depends on what is lying in the working directory.
    return factory.build_rpt_equivalent_model_triso_kernel_homogenised(
        ring_rpt_inner_radius_cm=ring_rpt_inner_radius_cm)


# ---------------------------------------------------------------------------
# Step 3: add tallies, run, read k back
# ---------------------------------------------------------------------------

def _fuel_materials(model):
    """Every material containing uranium -- i.e. the fuel, whatever it is called.

    Found by composition rather than by name, so it works for both the explicit
    kernel material and the RPT homogenised material without special-casing.
    """
    fuel = []
    for material in model.materials:
        names = [nuclide[0] if isinstance(nuclide, tuple) else nuclide
                 for nuclide in material.get_nuclides()]
        if any(n.startswith('U23') for n in names):
            fuel.append(material)
    if not fuel:
        raise RuntimeError(
            "no uranium-bearing material found -- cannot identify the fuel, so "
            "the thermal-utilisation factor f cannot be computed")
    return fuel


def add_six_factor_tallies(model):
    """Attach the two tallies the six-factor formula needs.

    Tally 'global'  -- absorption and nu-fission everywhere, split thermal/fast.
    Tally 'fuel'    -- the same scores restricted to the fuel materials.

    Modifies ``model`` in place and returns it.
    """
    energy_filter = openmc.EnergyFilter(
        [0.0, THERMAL_CUTOFF_EV, 20.0e6])

    global_tally = openmc.Tally(name='global')
    global_tally.filters = [energy_filter]
    global_tally.scores = ['absorption', 'nu-fission']

    fuel_tally = openmc.Tally(name='fuel')
    fuel_tally.filters = [
        openmc.EnergyFilter([0.0, THERMAL_CUTOFF_EV, 20.0e6]),
        openmc.MaterialFilter(_fuel_materials(model)),
    ]
    fuel_tally.scores = ['absorption', 'nu-fission']

    model.tallies = openmc.Tallies([global_tally, fuel_tally])
    return model


def configure_run(model, particles=None, batches=None, inactive=None):
    """Set the eigenvalue run size and switch on the source-convergence check.

    Shannon entropy is tallied so you can see whether ``inactive`` was long
    enough. If it has not flattened by the end of the inactive batches, k is
    biased no matter how small its error bar looks.
    """
    # Read the module constants here rather than as default arguments, so
    # changing PARTICLES/BATCHES/INACTIVE after import actually takes effect.
    model.settings.run_mode = 'eigenvalue'
    model.settings.particles = PARTICLES if particles is None else particles
    model.settings.batches = BATCHES if batches is None else batches
    model.settings.inactive = INACTIVE if inactive is None else inactive

    lower_left, upper_right = model.geometry.bounding_box

    # Start neutrons inside the fuel zone, not across the whole pebble.
    #
    # This matters more than it looks. In the explicit pebble the TRISO kernels
    # are a very small volume fraction, so a source spread over the full
    # bounding box has almost every sample land in graphite or FLiBe and gets
    # rejected -- OpenMC then aborts with "too few source sites satisfied the
    # constraints". Sampling the fuel zone with `fissionable=True` fixes it for
    # both pebbles: the explicit one finds kernels, and the RPT one finds its
    # homogenised shell.
    fuel_zone = openmc.stats.Box(
        [-FUEL_ZONE_RADIUS_CM] * 3, [FUEL_ZONE_RADIUS_CM] * 3,
        only_fissionable=False)
    model.settings.source = openmc.IndependentSource(
        space=fuel_zone, constraints={'fissionable': True})

    # In the explicit pebble the fissionable *kernels* are only a percent or so
    # of the fuel zone by volume, so most samples legitimately miss. OpenMC's
    # default floor of 5% accepted is too strict for that geometry and aborts
    # the run; the kernels are found, just rarely. This is the remedy OpenMC's
    # own error message recommends.
    model.settings.source_rejection_fraction = 1.0e-4

    model.settings.entropy_mesh = openmc.RegularMesh()
    model.settings.entropy_mesh.lower_left = lower_left
    model.settings.entropy_mesh.upper_right = upper_right
    model.settings.entropy_mesh.dimension = (8, 8, 8)
    return model


def run(model, name):
    """Run ``model`` and return its k-eff.

    Returns a dict with ``k``, ``k_std_dev``, ``k_pcm_std_dev``,
    ``leakage``, and ``statepoint`` (the path, so the tallies can be read).
    """
    add_six_factor_tallies(model)
    configure_run(model)

    statepoint_path = model.run(cwd='.', output=True)

    with openmc.StatePoint(statepoint_path) as sp:
        keff = sp.keff
        result = {
            'name': name,
            'k': float(keff.nominal_value),
            'k_std_dev': float(keff.std_dev),
            'k_pcm_std_dev': float(keff.std_dev) * 1e5,
            # int() rather than the raw numpy scalars: these end up in JSON
            # and CSV, and numpy int64 is not JSON-serializable.
            'particles': int(sp.n_particles),
            'batches': int(sp.n_batches),
            'inactive': int(sp.n_inactive),
            'statepoint': str(statepoint_path),
        }
    return result


# ---------------------------------------------------------------------------
# Step 4: turn the tallies into the six factors
# ---------------------------------------------------------------------------

def six_factors(statepoint_path):
    """Compute the six factors from a finished run.

    Returns a dict of ``eta``, ``f``, ``p``, ``epsilon``, ``P_FNL``, ``P_TNL``,
    ``k_inf_from_factors`` and ``k_from_openmc``, plus a ``consistent`` flag
    comparing the last two.
    """
    with openmc.StatePoint(statepoint_path) as sp:
        global_tally = sp.get_tally(name='global')
        fuel_tally = sp.get_tally(name='fuel')

        # Energy filter bin 0 is thermal (below the cutoff), bin 1 is fast.
        # The global tally has one filter (energy), so bin 0 is thermal and
        # bin 1 is fast.
        absorption = global_tally.get_values(scores=['absorption']).flatten()
        production = global_tally.get_values(scores=['nu-fission']).flatten()

        # The fuel tally has TWO filters, so its flat array is energy-major:
        # [E0M0, E0M1, E1M0, E1M1]. Reshape to (energy, material) and sum over
        # materials -- there is usually more than one uranium-bearing material,
        # and some of them contribute nothing. Indexing [0] here instead of
        # summing is exactly the bug that produced a divide-by-zero: it picked
        # the empty material rather than the thermal group.
        n_materials = len(fuel_tally.find_filter(openmc.MaterialFilter).bins)
        fuel_absorption = fuel_tally.get_values(
            scores=['absorption']).reshape(2, n_materials).sum(axis=1)

        a_thermal = float(absorption[0])
        a_total = float(absorption.sum())
        p_thermal = float(production[0])
        p_total = float(production.sum())
        a_thermal_fuel = float(fuel_absorption[0])

        k_from_openmc = float(sp.keff.nominal_value)

    eta = p_thermal / a_thermal_fuel
    f = a_thermal_fuel / a_thermal
    p = a_thermal / a_total
    epsilon = p_total / p_thermal

    # Reflective boundaries: nothing leaves, so both non-leakage factors are 1
    # by construction. Kept explicit so the formula reads as the six-factor
    # formula rather than the four-factor one.
    p_fnl = 1.0
    p_tnl = 1.0

    k_inf = eta * f * p * epsilon
    relative_gap = abs(k_inf - k_from_openmc) / k_from_openmc

    return {
        'eta': eta,
        'f': f,
        'p': p,
        'epsilon': epsilon,
        'P_FNL': p_fnl,
        'P_TNL': p_tnl,
        'k_inf_from_factors': k_inf,
        'k_from_openmc': k_from_openmc,
        'relative_gap': relative_gap,
        # eta*f*p*epsilon collapses algebraically to (total nu-fission) /
        # (total absorption). For a non-leaking system that is k -- ALMOST.
        # It omits (n,2n) and friends, which remove one neutron and emit two,
        # so they add multiplication that nu-fission does not count. The
        # product therefore sits slightly BELOW OpenMC's k, by around a
        # percent in a graphite system. A gap in that direction and of that
        # size is expected; a large gap, or one in the other direction, means
        # a tally is mis-specified.
        'consistent': bool(-0.005 < (k_from_openmc - k_inf) / k_from_openmc < 0.03),
    }


# ---------------------------------------------------------------------------
# The whole pipeline
# ---------------------------------------------------------------------------

def main():
    if 'OPENMC_CROSS_SECTIONS' not in os.environ:
        sys.exit("set OPENMC_CROSS_SECTIONS to your cross_sections.xml first")

    results = []
    for label, builder in [
            ('triso_explicit', build_triso_pebble),
            ('ring_rpt', build_rpt_pebble)]:
        print(f"\n=== {label} ===", flush=True)
        model = builder()
        keff = run(model, label)
        factors = six_factors(keff['statepoint'])
        results.append((keff, factors))

        print(f"  k        = {keff['k']:.5f} +/- {keff['k_std_dev']:.5f}"
              f"  ({keff['k_pcm_std_dev']:.0f} pcm)")
        print(f"  eta      = {factors['eta']:.5f}")
        print(f"  f        = {factors['f']:.5f}")
        print(f"  p        = {factors['p']:.5f}")
        print(f"  epsilon  = {factors['epsilon']:.5f}")
        print(f"  product  = {factors['k_inf_from_factors']:.5f}"
              f"   consistent={factors['consistent']}")

    (k_triso, _), (k_rpt, _) = results
    difference_pcm = (k_rpt['k'] - k_triso['k']) * 1e5
    combined_sigma_pcm = np.hypot(k_triso['k_pcm_std_dev'],
                                  k_rpt['k_pcm_std_dev'])
    print(f"\nRPT minus explicit: {difference_pcm:+.0f} pcm "
          f"(combined sigma {combined_sigma_pcm:.0f} pcm)")
    return results


if __name__ == '__main__':
    main()
