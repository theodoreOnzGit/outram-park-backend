import os
import openmc
import openmc.model
import numpy as np
"""
This is a code meant to contain functions or libraries
and have a main test suite in order to determine the
reactivity equivalent feedback of a triso fuel pebble

This is a leaner version of reactivity physical transform
libraries. It is only meant to produce the rpt pebble
with homogenised triso and shell.

First things first, we import import triso

>>> import triso_pebble
>>> import openmc
>>> import numpy as np
>>> import openmc.model
"""


class RingRPTPebbleFactory:

    def __init__(self):
        pass

    def generate_rpt_equivalent_xml(self,
                                    ring_rpt_inner_radius_cm=1.5):
        """
        This is ring would consist of homogenised triso material without
        the background matrix material. The volume of the triso particles
        is conserved.

        One can use the search for keff function to help determine what
        r is

        Note that k_infinty is used for Ring-RPT and RPT tests rather than
        a vacuum bc. Hence, a reflective BC should be used.
        In this pebble model, the reflective BC is

        >>> import triso_pebble
        >>> import openmc
        >>> import numpy as np
        >>> import openmc.model
        >>> factory = RingRPTPebbleFactory()
        >>> rpt_model = factory.\
                build_rpt_equivalent_model_triso_kernel_homogenised(1.4933)
        loading modules for triso particles...

        >>> materials = rpt_model.materials
        >>> geometry = rpt_model.geometry
        >>> settings = rpt_model.settings
        """

        rpt_model = self.build_rpt_equivalent_model_triso_kernel_homogenised(
                ring_rpt_inner_radius_cm)
        materials = rpt_model.materials
        geometry = rpt_model.geometry
        settings = rpt_model.settings

        materials.export_to_xml()
        geometry.export_to_xml()
        settings.export_to_xml()

    def build_rpt_equivalent_model_triso_kernel_homogenised(
            self,
            ring_rpt_inner_radius_cm=1.4933593750000003):
        """
        This builds the Reactivity Equivalent Physical Transform
        For either the non_annular fuel pebble
        diameter 4cm, graphite shell thickness 1cm

        Generates xml files for the non annular pebble fuel
        reactivity equivalent physical transform
        with vacuum boundary conditions at 3cm of the pebble
        given a value r

        This converts triso material into a ring (for a cylinder)
        or shell for a sphere

        Lou, L., Yao, D., Chai, X., Peng, X., Li, M., Li, W., ... & Wang, L.
        (2020). A novel reactivity-equivalent physical transformation method for
        homogenization of double-heterogeneous systems.
        Annals of Nuclear Energy, 142, 107396.


        ============================>     RESULTS     <========================

        k-effective (Collision)     = 1.42213 +/- 0.00115
        k-effective (Track-length)  = 1.42169 +/- 0.00121
        k-effective (Absorption)    = 1.42336 +/- 0.00068
        Combined k-effective        = 1.42312 +/- 0.00067
        Leakage Fraction            = 0.00000 +/- 0.00000

        some values: at ring_rpt_inner_radius = 1.6

        ============================>     RESULTS     <========================

        k-effective (Collision)     = 1.35729 +/- 0.00103
        k-effective (Track-length)  = 1.35641 +/- 0.00109
        k-effective (Absorption)    = 1.35685 +/- 0.00068
        Combined k-effective        = 1.35672 +/- 0.00066
        Leakage Fraction            = 0.00000 +/- 0.00000

        and we need to get...
        ============================>     RESULTS     <=======================

        k-effective (Collision)     = 1.36859 +/- 0.00089
        k-effective (Track-length)  = 1.36885 +/- 0.00131
        k-effective (Absorption)    = 1.36863 +/- 0.00065
        Combined k-effective        = 1.36864 +/- 0.00065
        Leakage Fraction            = 0.00000 +/- 0.00000

        These methods bracket keff = 1.36!!!
        So rpt does work, it's just that the flibe was over absorbent
        when i carelessly left it at 99.5% purity rather than 99.995% purity

        Using search for keff,

        1.493359375 cm is the RPT radius keff = 1.36863 +/- 0.00070
        This is well within error bounds
        I'm using this as the default value
        Note that ENDF v 7b.1 was used
        """

        from triso import TrisoParticlesFactory
        triso_particles_factory = TrisoParticlesFactory()
        pebble_materials = triso_particles_factory.build_fhr_materials(
                19.9, 99.995, 600.0)

        mixed_triso_material = self.get_mixed_triso_fuel_material(
                pebble_materials)

        materials = openmc.Materials()
        materials.append(pebble_materials.fuel)
        materials.append(pebble_materials.buff)
        materials.append(pebble_materials.PyC1)
        materials.append(pebble_materials.PyC2)
        materials.append(pebble_materials.SiC)
        materials.append(pebble_materials.graphite)
        materials.append(pebble_materials.flibe)
        materials.append(mixed_triso_material)

        # now that we've settle materials, we should get
        # a suitable reactivity equivalent pebble
        # the background material is graphite
        # The ring rpt inner_radius is defined by the user
        # and will form the innermost sphere of graphite

        # we shall then calculate the thickness
        # the thickness is calculated by constraining the
        # volume of the annular region to the volume of all triso particles
        # the outermost radius of the triso particle is 425*1e-4 cm
        # this is just for info, not used in calculation
        # for a 2cm radius, 4cm diameter nonannular fuel pebble
        # the outermost pebble shell thickness is 0.1cm

        pebble_diameter = 4.0

        pebble_shell_thickness_cm = 0.1
        fuel_radius_original_pebble_cm = pebble_diameter/2.0 -\
            pebble_shell_thickness_cm
        triso_plus_matrix_volume_cm3 = self.sphere_vol(
                fuel_radius_original_pebble_cm)
        triso_packing_factor = 0.3
        triso_volume_cm3 = triso_plus_matrix_volume_cm3 * triso_packing_factor

        inner_sphere_vol_cm3 = self.sphere_vol(ring_rpt_inner_radius_cm)
        inner_sphere_plus_triso_vol_cm3 = inner_sphere_vol_cm3\
            + triso_volume_cm3
        fuel_outer_radius = (0.75/np.pi *
                             inner_sphere_plus_triso_vol_cm3)**(1/3)

        # we must asser that the fuel outer radius is not larger than
        # the upper limit, or about
        fuel_outer_radius_upper_limit = pebble_diameter/2.0 - \
            pebble_shell_thickness_cm

        if fuel_outer_radius > fuel_outer_radius_upper_limit:
            raise ValueError("rpt radius: fuel_outer_radius more than \
                    upper limit of 1.9cm")

        pebble_fuel_thickness = fuel_outer_radius - ring_rpt_inner_radius_cm

        inner_graphite_sph_radius = ring_rpt_inner_radius_cm
        inner_graphite_sph = openmc.Sphere(
                r=inner_graphite_sph_radius)
        fuel_sph = openmc.Sphere(
                r=inner_graphite_sph.r+pebble_fuel_thickness)
        outer_sph = openmc.Sphere(
                r=pebble_diameter/2)
        outermost_sph = openmc.Sphere(
                r=pebble_diameter)
        outermost_sph.boundary_type = 'vacuum'

        inner_graphite_region = -inner_graphite_sph
        fuel_region = -fuel_sph & +inner_graphite_sph
        outer_shell_region = -outer_sph & +fuel_sph
        flibe_region = +outer_sph & -outermost_sph

        # now the geometry is set, begin cell construction
        # first define fuel and coolant temp in kelvin
        fuel_temp = 600.0
        coolant_temp = 600.0

        inner_graphite = openmc.Cell(name='pebble.inner_graphite')
        inner_graphite.fill = pebble_materials.graphite
        inner_graphite.region = inner_graphite_region
        inner_graphite.temperature = fuel_temp

        homogenised_fuel = openmc.Cell(name='pebble.homogenised_fuel')
        homogenised_fuel.fill = mixed_triso_material
        homogenised_fuel.region = fuel_region
        homogenised_fuel.temperature = fuel_temp

        outer_graphite = openmc.Cell(name='pebble.outer_graphite')
        outer_graphite.fill = pebble_materials.graphite
        outer_graphite.region = outer_shell_region
        outer_graphite.temperature = fuel_temp

        flibe_peripheral = openmc.Cell(name='pebble.flibe_peripheral')
        flibe_peripheral.fill = pebble_materials.flibe
        flibe_peripheral.region = flibe_region
        flibe_peripheral.temperature = coolant_temp

        pebble_univ = openmc.Universe(
                name='pebble_univ',
                cells=[inner_graphite,
                       homogenised_fuel,
                       outer_graphite,
                       flibe_peripheral])

        # once we constructed our pebble universe, we can make
        # a root cell and root universe (kind of extra step,
        # but good anyhow)
        root_cell = openmc.Cell(name='root_cell')
        root_cell_sph = openmc.Sphere(r=3.0)
        root_cell_sph.boundary_type = 'reflective'
        root_cell_region = -root_cell_sph
        root_cell.fill = pebble_univ
        root_cell.region = root_cell_region
        root_universe = openmc.Universe(cells=[root_cell])

        geometry = openmc.Geometry(root_universe)

        settings = openmc.Settings()
        settings.batches = 200
        settings.inactive = 50
        settings.particles = 10000
        lower_left, upper_right = root_universe.bounding_box
        bounds = [lower_left[0], lower_left[1], lower_left[2],
                  upper_right[0], upper_right[1], upper_right[2]]
        uniform_dist = openmc.stats.Box(bounds[:3],  bounds[3:],
                                        only_fissionable=True)
        settings.source = openmc.Source(space=uniform_dist)

        model = openmc.model.Model(geometry, materials, settings)

        # this part is for plotting

        plot = openmc.Plot()
        plot.filename = 'rpt_pebble_plot_homogenised_shell_and_fuel'
        plot.width = (6, 6)
        plot.pixels = (1000, 1000)
        plot.color_by = 'material'
        plot.colors = {pebble_materials.fuel: 'yellow',
                       pebble_materials.buff: 'black',
                       pebble_materials.PyC1: 'gray',
                       pebble_materials.PyC2: 'gray',
                       pebble_materials.SiC:  'silver',
                       pebble_materials.graphite: 'black',
                       pebble_materials.flibe: 'blue',
                       mixed_triso_material: 'green'}
        rpt_pebble_plot = openmc.Plots([plot])
        rpt_pebble_plot.export_to_xml()
        materials.export_to_xml()
        geometry.export_to_xml()
        settings.export_to_xml()
        # I need to remove model.xml before plotting
        # otherwise an error will occur
        path = "./model.xml"

        try:
            os.remove(path)
        except OSError as error:
            # I just want it to remove model.xml
            # otherwise don't print anything
            # here i just deleted the error object
            # which is the same as pass, but i don't get
            # linting errors
            del error

        openmc.plot_geometry(output=False)
        plot.to_ipython_image()

        return model

    def get_mixed_triso_fuel_material(self, pebble_materials):
        """
        returns material for a homogenised triso particle

        I need to know how thick each layer is.

        For this, we can reference the code used to construct
        the triso universe


        spheres = [openmc.Sphere(r=1e-4*r)
                   for r in [215., 315., 350., 385., 700.]]
        cells = [openmc.Cell(fill=pebble_materials.fuel,
                             region=-spheres[0]),
                 openmc.Cell(fill=pebble_materials.buff,
                             region=+spheres[0] & -spheres[1]),
                 openmc.Cell(fill=pebble_materials.PyC1,
                             region=+spheres[1] & -spheres[2]),
                 openmc.Cell(fill=pebble_materials.SiC,
                             region=+spheres[2] & -spheres[3]),
                 openmc.Cell(fill=pebble_materials.PyC2,
                             region=+spheres[3] & -spheres[4])]

        At first, one may think the triso radius is 700.e-4,
        However, note that when making the trisofill, the outer radius
        of the triso particle is 425.e-4:

        def triso_fill(self,
                       pebble_materials,
                       fuel_region,
                       packing_factor=0.3,
                       outer_radius=425.*1e-4):

        Units: for this code
        density is in g/cm3
        mass is in grams
        amt of substance is g-mol
        length scales such as diameter and radius are in cm
        sphere volumes are in cm3
        >>> from triso import TrisoParticlesFactory
        >>> triso_particles_factory = TrisoParticlesFactory()
        loading modules for triso particles...
        >>> pebbleMaterials = triso_particles_factory.build_fhr_materials(\
                19.9, 99.995, 600.0)


        How then to ensure that this homogenisation is done correctly?
        I am homogenising a triso particle with 425e-4 radius,
        the fuel kernel itself is only 215e-4 radius.
        I can use the fissionable mass attribute and
        volume (in cm3) method to ensure that
        fissionable mass is conserved. If this is done correctly,
        Then I am more sure that the fuel is correctly homogenised
        >>> factory = RingRPTPebbleFactory()
        >>> pebbleMaterials.fuel.volume = factory.\
                sphere_vol(radius_in_cm=215.*1e-4)

        Let's have a baseline check for fissionable material
        >>> pebbleMaterials.fuel.fissionable_mass
        0.0003880842669341953

        >>> mixed_triso_fuel_material = factory.\
                get_mixed_triso_fuel_material(\
                pebbleMaterials)

        If i homogenised correctly, the sphere of 425e-4 radius
        should have the same fissionable_mass (or almost the same)
        >>> mixed_triso_fuel_material.volume = factory.\
                sphere_vol(radius_in_cm=425.*1e-4)
        >>> import numpy as np

        If done correctly, the error percentage should be less than 0.1%
        >>> np.absolute(1.0 - \
                mixed_triso_fuel_material.fissionable_mass\
                /pebbleMaterials.fuel.fissionable_mass) > 1e-3
        False
        """

        # first let's calculate the innermost layer, the fuel
        fuel_density = pebble_materials.fuel.density
        fuel_volume = self.sphere_vol(radius_in_cm=215.*1e-4)
        fuel_mass = fuel_density*fuel_volume

        o16_atom_fraction = 0.5
        carbon_atom_fraction = 1.6667e-01
        total_uranium_fraction = 1.0 - o16_atom_fraction - carbon_atom_fraction
        uranium_enrichment_percent = 19.9
        u235_atom_fraction = uranium_enrichment_percent/100.0\
            * total_uranium_fraction
        u238_atom_fraction = (100.0-uranium_enrichment_percent)/100.0\
            * total_uranium_fraction

        # this is calculated on a per atom basis, not
        # per molecular weightS
        # Approximately, one would calculate it like this:

        # approx_fuel_molar_wt_g_per_gmol = \
        #    u235_atom_fraction * 235.04393 \
        #    + u238_atom_fraction * 238.05078826 \
        #    + o16_atom_fraction * 15.99491461956 \
        #    + carbon_atom_fraction * 12.0

        # the simpler way is to use the average_molar_mass attribute

        approx_fuel_molar_wt_g_per_gmol = \
            pebble_materials.fuel.average_molar_mass

        fuel_total_moles = fuel_mass / approx_fuel_molar_wt_g_per_gmol
        u235_moles = fuel_total_moles * u235_atom_fraction
        u238_moles = fuel_total_moles * u238_atom_fraction
        o16_moles = fuel_total_moles * o16_atom_fraction
        carbon_from_fuel_kernel_moles = fuel_total_moles * carbon_atom_fraction

        # second layer is bugger layer
        buffer_density = pebble_materials.buff.density
        buffer_carbon_volume = self.sphere_vol(radius_in_cm=315.*1e-4)\
            - self.sphere_vol(radius_in_cm=215.*1e-4)
        buffer_carbon_mass = buffer_density*buffer_carbon_volume
        buffer_carbon_moles = buffer_carbon_mass\
            / pebble_materials.buff.average_molar_mass

        # let's calculate the same for pyrolytic_carbon_1 and pyrolytic_carbon_2

        pyrolytic_carbon_1_density = pebble_materials.PyC1.density
        pyrolytic_carbon_1_volume = self.sphere_vol(radius_in_cm=350.*1e-4)\
            - self.sphere_vol(radius_in_cm=315.*1e-4)
        pyrolytic_carbon_1_carbon_mass = pyrolytic_carbon_1_density\
                * pyrolytic_carbon_1_volume
        pyrolytic_carbon_1_carbon_moles = pyrolytic_carbon_1_carbon_mass\
                / pebble_materials.PyC1.average_molar_mass

        # one tricky layer is the SiC layer,
        # the molar mass should be 40 for this one
        # 1 mole of SiC contains 1 mole of Si and 1 mole of C

        # if we use average molar mass, then we need to multiply
        # by atom fraction if we want to get the correct number of moles

        silicon_carbide_density = pebble_materials.SiC.density
        silicon_carbide_volume = self.sphere_vol(radius_in_cm=385.*1e-4)\
            - self.sphere_vol(radius_in_cm=350.*1e-4)
        silicon_carbide_mass = silicon_carbide_density*silicon_carbide_volume
        silicon_carbide_total_moles = silicon_carbide_mass\
            / pebble_materials.SiC.average_molar_mass
        silicon_carbide_carbon_moles = silicon_carbide_total_moles * 0.5
        silicon_carbide_silicon_moles = silicon_carbide_total_moles * 0.5

        pyrolytic_carbon_2_density = pebble_materials.PyC2.density
        pyrolytic_carbon_2_volume = self.sphere_vol(radius_in_cm=425.*1e-4)\
            - self.sphere_vol(radius_in_cm=385.*1e-4)
        pyrolytic_carbon_2_carbon_mass = pyrolytic_carbon_2_density\
            * pyrolytic_carbon_2_volume
        pyrolytic_carbon_2_carbon_moles = pyrolytic_carbon_2_carbon_mass/12.0

        # next up, we are using uranium oxycarbide as the fuel

        # now let's calculate the density of the homogenised fuel
        homogenised_triso_fuel_mass = fuel_mass + silicon_carbide_mass\
            + pyrolytic_carbon_1_carbon_mass + pyrolytic_carbon_2_carbon_mass\
            + buffer_carbon_mass
        homogenised_triso_fuel_volume = fuel_volume + silicon_carbide_volume\
            + pyrolytic_carbon_1_volume + pyrolytic_carbon_2_volume\
            + buffer_carbon_volume

        homogenised_triso_fuel_mass_density = homogenised_triso_fuel_mass\
            / homogenised_triso_fuel_volume

        # we also need to calculate the atomic fractions of each element here
        total_moles = fuel_total_moles + silicon_carbide_carbon_moles\
            + silicon_carbide_silicon_moles + pyrolytic_carbon_1_carbon_moles\
            + pyrolytic_carbon_2_carbon_moles + buffer_carbon_moles

        homogenised_u235_fraction = u235_moles / total_moles
        homogenised_u238_fraction = u238_moles / total_moles
        homogenised_o16_fraction = o16_moles / total_moles
        homogenised_silicon_fraction = silicon_carbide_silicon_moles / total_moles
        homogenised_carbon_fraction = carbon_from_fuel_kernel_moles / total_moles
        homogenised_carbon_fraction += silicon_carbide_carbon_moles / total_moles
        homogenised_carbon_fraction += pyrolytic_carbon_1_carbon_moles\
            / total_moles
        homogenised_carbon_fraction += pyrolytic_carbon_2_carbon_moles\
            / total_moles
        homogenised_carbon_fraction += buffer_carbon_moles / total_moles

        homogenised_fuel_total_fraction = \
            homogenised_o16_fraction\
            + homogenised_silicon_fraction\
            + homogenised_u235_fraction\
            + homogenised_u238_fraction\
            + homogenised_carbon_fraction

        # this checks if the math is correct, i.e all fractions
        # sum up to 1.0
        if np.absolute(homogenised_fuel_total_fraction - 1.0) > 1e-3:
            raise ValueError("homogenised fuel: the fractions are not \
                    equal 1, pls check code")

        homogenised_triso_fuel = openmc.Material()
        homogenised_triso_fuel.set_density('g/cm3',
                                           homogenised_triso_fuel_mass_density)
        homogenised_triso_fuel.name = 'homogenised triso fuel'
        homogenised_triso_fuel.add_element('C',
                                           homogenised_carbon_fraction)
        homogenised_triso_fuel.add_nuclide('U235',
                                           homogenised_u235_fraction)
        homogenised_triso_fuel.add_nuclide('U238',
                                           homogenised_u238_fraction)
        homogenised_triso_fuel.add_element('Si',
                                           homogenised_silicon_fraction)
        homogenised_triso_fuel.add_nuclide('O16',
                                           homogenised_o16_fraction)

        return homogenised_triso_fuel

    def sphere_vol_by_diameter(self, diameter_in_cm):
        """
        calculates sphere volume in cm3 using sphere diameter in cm

        """

        radius_in_cm = diameter_in_cm/2.0
        return self.sphere_vol(radius_in_cm)

    def sphere_vol(self, radius_in_cm):
        """
        calculates sphere volume in cm3 using radius in cm

        """
        volume_in_cm3 = 4/3*radius_in_cm**3*np.pi
        return volume_in_cm3





if __name__ == "__main__":
    print("Starting reactivity_equivalent_transform")

    # this is for generating the non annular baseline xml to run openmc
    # generate_non_annular_baseline_vacuum_xml()
    # generate_non_annular_baseline_k_infinity_xml()
    import doctest
    doctest.testmod(verbose=True)
    # generate_rpt_equivalent_xml(1.6)
    # comment out the following line to run manually
    # openmc.run()
    # print_rpt_data()

