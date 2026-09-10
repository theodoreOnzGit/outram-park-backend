"""TrisoParticlesFactory is responsible for creating a triso universe to fill
a pebble

The idea is to build and return a materials and geometry
for the TrisoParticlesFactory. To do so, we have four imports. Firstly,
Types is used for simpleNameSpace. Basically, its an empty
object with which to package and abstract pebble material information.

Second, numpy is used for sphere volume and other things.

Thirdly, openmc and openmc.model are used to build openmc
materials

>>> import types
>>> import openmc
>>> import openmc.model
"""
import types
import openmc
import openmc.model

class TrisoParticlesFactory:

    """TrisoParticlesFactory is responsible for creating a triso universe to fill
    a pebble

    The idea is to build and return a materials and geometry
    for the TrisoParticlesFactory. This is automatically done by
    the constructor
    To initiate the class,
    >>> triso_obj = TrisoParticlesFactory()
    loading modules for triso particles...

    """

    def __init__(self):
        """
        To initiate the class,
        >>> triso_obj = TrisoParticlesFactory()
        loading modules for triso particles...
        """
        print("loading modules for triso particles...")

    def build_fhr_materials(self,
                            uranium_enrichment_atom_percent,
                            flibe_enrichment_atom_percent,
                            coolant_temp):
        """
        This shows a documentation test, of how to use the build
        fhr materials function

        We are testing a few things here,
        first, the U235 atom percent

        >>> triso_obj = TrisoParticlesFactory()
        loading modules for triso particles...

        Now to use this, we have three inputs,
        build_fhr_materials(
        uranium_enrichment_atom_percent,
        Li7_enrichment_atom_percent,
        temperature)

        >>> pebbleMaterials = triso_obj.build_fhr_materials(19.9, 99.5, 720)
        >>> pebbleMaterials.fuel.nuclides[0].name
        'U235'
        >>> import numpy

        The amount of uranium 235 should be about 6.6 atom percent
        approximately
        >>> numpy.absolute(1.0 - pebbleMaterials.fuel.nuclides[0].percent\
                /0.066403514) > 0.01
        False

        Next is the Flibe percent of Li7
        >>> pebbleMaterials.flibe.nuclides[1].name
        'Li7'
        >>> pebbleMaterials.flibe.nuclides[1].percent
        1.99
        """

        # we want to set uranium enrichment by atoms,
        # this amount allows us to define uranium atom
        # amount by some atomic density i think, got to check
        # the triso tutorial
        o16_fraction = 0.5
        carbon_fraction = 1.6667e-01
        total_uranium_amount = 1.0 - o16_fraction - carbon_fraction
        uranium_235_fraction = uranium_enrichment_atom_percent/100

        fuel = openmc.Material(name='fuel')
        fuel.set_density('g/cm3', 10.5)
        fuel.add_nuclide(
                'U235', total_uranium_amount*uranium_235_fraction)
        fuel.add_nuclide(
                'U238', total_uranium_amount*(1-uranium_235_fraction))
        fuel.add_nuclide('O16',  o16_fraction)
        fuel.add_element('C', carbon_fraction)
        fuel.id = 1

        buff = openmc.Material(name='buffer')
        buff.set_density('g/cm3', 1.0)
        buff.add_element('C', 1.0)
        buff.add_s_alpha_beta('c_Graphite')
        buff.id = 2

        PyC1 = openmc.Material(name='PyC1')
        PyC1.set_density('g/cm3', 1.9)
        PyC1.add_element('C', 1.0)
        PyC1.add_s_alpha_beta('c_Graphite')
        PyC1.id = 3

        PyC2 = openmc.Material(name='PyC2')
        PyC2.set_density('g/cm3', 1.87)
        PyC2.add_element('C', 1.0)
        PyC2.add_s_alpha_beta('c_Graphite')
        PyC2.id = 4

        SiC = openmc.Material(name='SiC')
        SiC.set_density('g/cm3', 3.2)
        SiC.add_element('C', 0.5)
        SiC.add_element('Si', 0.5)
        SiC.id = 5

        graphite = openmc.Material(name='graphite')
        graphite.set_density('g/cm3', 1.1995)
        graphite.add_element('C', 1.0)
        graphite.add_s_alpha_beta('c_Graphite')
        graphite.id = 6

        total_lithium_ratio = 2
        lithium_7_fraction = flibe_enrichment_atom_percent/100
        flibe = openmc.Material(name='flibe')

        # this code calculates flibe density by temperature in g/cm3

        flibe_density = self.flibe_density_cm3(coolant_temp)
        flibe.set_density('g/cm3', flibe_density)
        flibe.temperature = coolant_temp
        flibe.add_nuclide('F19', 4.0)
        flibe.add_nuclide('Li7', total_lithium_ratio*lithium_7_fraction)
        flibe.add_nuclide('Li6', total_lithium_ratio*(1.0-lithium_7_fraction))
        flibe.add_nuclide('Be9', 1.0)
        flibe.id = 7

        # the last step assign the materials into the self object

        # note: i cannot just set sub-attributes for
        # customMaterials without first setting a
        # customMaterials attribute

        # so i initialise an empty object by using simplenamespace under
        # the type module
        # https://stackoverflow.com/questions/19476816/creating-an-empty-object-in-python
        # https://newbedev.com/creating-an-empty-object-in-python

        pebble_materials = types.SimpleNamespace()
        pebble_materials.fuel = fuel
        pebble_materials.buff = buff
        pebble_materials.PyC1 = PyC1
        pebble_materials.PyC2 = PyC2
        pebble_materials.SiC = SiC
        pebble_materials.graphite = graphite
        pebble_materials.flibe = flibe

        # now the mixedtrisoshell material, is
        # the material for triso shell covering
        # containing all the above materials,
        # SiC, PyC1, PyC2, buffer, graphite etc. homogenised
        # i will use a function to build
        # this mixedTrisoShellMaterial up in a function

        # this code is kind of legacy, helps to
        # build a triso shell where moderator materials are homogenised
        # self.buildMixedTrisoShellMaterial()

        return pebble_materials

    def triso_fill(self,
                   pebble_materials,
                   fuel_region,
                   packing_factor=0.3,
                   outer_radius=425.*1e-4):
        """
        builds a triso universe for filling
        a pebble region or any region in general

        The limits of this fill are 4cm by 4cm by 4cm
        I'm just going to perform a keff calculation
        as a doctest for this triso_fill

        >>> import openmc
        >>> test_sphere = openmc.Sphere(r=2.0)
        >>> test_sphere.boundary_type='reflective'
        >>> fuel_region = -test_sphere

        Next we are create a trisoparticles object
        To build pebble_materials and fuel region
        >>> triso_obj = TrisoParticlesFactory()
        loading modules for triso particles...
        >>> pebble_materials = triso_obj.build_fhr_materials( \
                19.9, \
                99.995, \
                600.0)

        Let's create and append our materials so
        we can export to xml
        >>> materials = openmc.Materials()
        >>> materials.append(pebble_materials.fuel)
        >>> materials.append(pebble_materials.buff)
        >>> materials.append(pebble_materials.PyC1)
        >>> materials.append(pebble_materials.PyC2)
        >>> materials.append(pebble_materials.SiC)
        >>> materials.append(pebble_materials.graphite)
        >>> materials.append(pebble_materials.flibe)

        Next we create our trisofills, to fill
        a test pebble
        >>> triso_fill = triso_obj.triso_fill(\
                pebble_materials, \
                fuel_region)
        packing in spheres...
        sphere packing complete
        creating triso_fill in lattice mode
        triso_fill generation complete!


        >>> test_pebble = openmc.Cell()
        >>> test_pebble.region = fuel_region
        >>> test_pebble.fill = triso_fill
        >>> root_universe = openmc.Universe(cells=[test_pebble])
        >>> geometry = openmc.Geometry(root_universe)


        Create a source and settings.xml
        For a really good simulation, you may want
        to have 200 batches, 10000 particles a batch
        and set inactive batches to 10
        Only the fission box type source works for triso
        >>> settings = openmc.Settings()
        >>> settings.batches = 10
        >>> settings.inactive = 0
        >>> settings.particles = 10
        >>> lowerLeft, upperRight = root_universe.bounding_box
        >>> bounds = [lowerLeft[0],lowerLeft[1],lowerLeft[2],\
                upperRight[0],upperRight[1],upperRight[2]]
        >>> uniform_dist = openmc.stats.Box(bounds[:3], bounds[3:],\
                only_fissionable=True)
        >>> settings.source = openmc.Source(space=uniform_dist)

        Export all to xml
        >>> materials.export_to_xml()
        >>> geometry.export_to_xml()
        >>> settings.export_to_xml()

        run openmc
        >>> openmc.run(output=False)
        """

        triso_univ = self.build_standard_triso_univ(
                pebble_materials)

        # We generate the centers first to put as
        # an input argument for triso_cells
        print('packing in spheres...')
        centers = openmc.model.pack_spheres(
                radius=outer_radius,
                region=fuel_region,
                pf=packing_factor)
        print('sphere packing complete')

        # The next step is to start building all triso cells
        triso_cells = [openmc.model.TRISO(
            outer_radius=outer_radius,
            fill=triso_univ,
            center=cntr) for cntr in centers]

        print('creating triso_fill in lattice mode')

        def triso_lattice_universe(lattice_region,
                                   triso_particles,
                                   background_material,):
            x_lattice_length = 4
            y_lattice_length = 4
            z_lattice_length = 4

            # we can then begin to construct the lattice

            lattice_cell = openmc.Cell(region=lattice_region)
            lower_left, upp_right = lattice_cell.region.bounding_box
            shape = (x_lattice_length, y_lattice_length, z_lattice_length)
            pitch = (upp_right - lower_left)/shape
            triso_latt = openmc.model.create_triso_lattice(
                    triso_particles, lower_left,
                    pitch, shape, background_material)
            lattice_cell.fill = triso_latt

            lattice_universe = openmc.Universe(cells=[lattice_cell])
            # note, most of this code is copied from VHTR tutorial
            # with minor editions
            return lattice_universe

        triso_fill = triso_lattice_universe(
                lattice_region=fuel_region,
                triso_particles=triso_cells,
                background_material=pebble_materials.graphite)

        print('triso_fill generation complete!')
        return triso_fill

    def build_standard_triso_univ(self, pebble_materials):
        """ TBD"""
        # first we load the modules

        # then we build the relevant cells
        # i will bound the cells with PyC2 at 0.07 cm to facilitate plotting

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

        # then i will build the standard triso universe and
        # place it into the object as an attribute

        standard_triso_univ = openmc.Universe(cells=cells)
        standard_triso_univ.name = 'standard_triso_univ'

        return standard_triso_univ

# Intermediate Functions to help Caclulate material properties #####

    def flibe_density_cm3(self,
                          coolant_temp):
        """calculates flibe density with coolant temp in
        degrees K in cm3 using
        1/1000*(2416-0.49072*coolant_temp)

        To initiate the class,
        >>> triso_obj = TrisoParticlesFactory()
        loading modules for triso particles...

        now let's load the flibe density function
        at 720 degrees K
        >>> triso_obj.flibe_density_cm3(720)
        2.0626816

        """

        # returns flibe density in cm3

        flibe_density_cm3 = 1/1000*(2416 - 0.49072*coolant_temp)

        return flibe_density_cm3


if __name__ == '__main__':
    import doctest
    doctest.testmod(verbose=True)
