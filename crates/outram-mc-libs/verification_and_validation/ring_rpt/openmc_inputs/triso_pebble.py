"""triso_pebble is responsible for creating 
triso pebble with which to fill a lattice

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
import openmc
import openmc.model
import triso


class TrisoPebbleFactory:

    """TrisoParticlesFactory is responsible for creating a triso universe to fill
    a pebble

    The idea is to build and return a materials and geometry
    for the TrisoParticlesFactory. This is automatically done by
    the constructor
    To initiate the class,
    >>> triso_obj = TrisoPebbleFactory()
    created TrisoPebbleFactory object
    """

    def __init__(self):
        print("created TrisoPebbleFactory object")

    def buildKPInspiredPebbleUniverse(self,
                                      pebble_fuel_thickness,
                                      pebble_graphite_shell_thickness,
                                      pebble_materials,
                                      fuel_temp,
                                      coolant_temp):
        """
        Builds a pebble with triso fuel with the annular
        fuel region inspired by Kairos Power Annular
        Fuel Pebble 4cm diameter

        You will need to define coolant temp both
        in the materials (to determine density)
        and here in this function (to determine cross section)

        """
        pebble_diameter = 4.0

        inner_graphite_sph_radius = pebble_diameter/2\
                - pebble_fuel_thickness\
                - pebble_graphite_shell_thickness

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

        triso_factory = triso.TrisoParticlesFactory()

        triso_fill = triso_factory.triso_fill(
                pebble_materials,
                fuel_region)

        inner_graphite = openmc.Cell(name='pebble.inner_graphite')
        inner_graphite.fill = pebble_materials.graphite
        inner_graphite.region = inner_graphite_region
        inner_graphite.temperature = fuel_temp

        triso_fuel = openmc.Cell(name='pebble.triso_fuel')
        triso_fuel.fill = triso_fill
        triso_fuel.region = fuel_region
        triso_fuel.temperature = fuel_temp

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
                       triso_fuel,
                       outer_graphite,
                       flibe_peripheral])

        return pebble_univ

    def buildNonAnnularPebbleUniv(self,
                                  pebble_graphite_shell_thickness,
                                  pebble_materials,
                                  fuel_temp,
                                  coolant_temp):

        """
        Builds a pebble with triso fuel without the annular
        region in the center

        You will need to define coolant temp both
        in the materials (to determine density)
        and here in this function (to determine cross section)

        pebble diameter here is 4cm


        >>> from triso import TrisoParticlesFactory
        >>> triso_factory = TrisoParticlesFactory()
        loading modules for triso particles...

        First thing first, you got to load your pebble materials
        Object,
        you also need to define define
        (1) fuel temperature in degrees K
        (2) coolant temperature in degrees K
        (3) pebble shell thickness (0.1 cm will do usually)

        >>> fuel_temp_deg_kelvin = 600.0
        >>> coolant_temp_deg_kelvin= 600.0
        >>> pebble_shell_thickness_cm = 0.1
        >>> flibe_lithium_7_enrichment_atom_percent = 99.995
        >>> uranium_235_enrichment_atom_percent = 19.9
        >>> pebble_materials = triso_factory.build_fhr_materials( \
                uranium_235_enrichment_atom_percent, \
                flibe_lithium_7_enrichment_atom_percent, \
                coolant_temp_deg_kelvin)


        Next, get your module or class imported and instantiate an object
        >>> pebble_factory = TrisoPebbleFactory()
        created TrisoPebbleFactory object

        Activate your method
        >>> pebble_univ = pebble_factory.buildNonAnnularPebbleUniv(\
                pebble_shell_thickness_cm,\
                pebble_materials,\
                fuel_temp_deg_kelvin,\
                coolant_temp_deg_kelvin)
        loading modules for triso particles...
        packing in spheres...
        sphere packing complete
        creating triso_fill in lattice mode
        triso_fill generation complete!

        Note that this test only works with a reflective boundary
        condition. Vacuum boundary condition results in too many
        particles lost especially with very very low source
        particles. No fissionable sites can then be
        established and we will get a runtime error.

        >>> test_sphere = openmc.Sphere(r=3.0)
        >>> test_sphere.boundary_type='reflective'
        >>> fuel_region = -test_sphere


        >>> test_pebble = openmc.Cell()
        >>> test_pebble.region = fuel_region
        >>> test_pebble.fill = pebble_univ
        >>> root_universe = openmc.Universe(cells=[test_pebble])
        >>> geometry = openmc.Geometry(root_universe)

        Now quick test in openmc
        >>> geometry = openmc.Geometry(root_universe)
        >>> materials = openmc.Materials()
        >>> materials.append(pebble_materials.fuel)
        >>> materials.append(pebble_materials.buff)
        >>> materials.append(pebble_materials.PyC1)
        >>> materials.append(pebble_materials.PyC2)
        >>> materials.append(pebble_materials.SiC)
        >>> materials.append(pebble_materials.graphite)
        >>> materials.append(pebble_materials.flibe)

        Create a source and settings.xml
        For a really good simulation, you may want
        to have 200 batches, 10000 particles a batch
        and set inactive batches to 10
        Only the fission box type source works for triso
        >>> settings = openmc.Settings()
        >>> settings.batches = 10
        >>> settings.inactive = 0
        >>> settings.particles = 10
        >>> lowerLeft, upperRight = pebble_univ.bounding_box
        >>> bounds = [lowerLeft[0],lowerLeft[1],lowerLeft[2],\
                upperRight[0],upperRight[1],upperRight[2]]
        >>> uniform_dist = openmc.stats.Box(bounds[:3], bounds[3:],\
                only_fissionable=True)
        >>> settings.source = openmc.Source(space=uniform_dist)

        Openmc plots
        >>> import matplotlib.image as img
        >>> from PIL import Image
        >>> plot = openmc.Plot()
        >>> plot.filename = 'triso_pebble_plot'
        >>> plot.width = (6,6)
        >>> plot.pixels = (1000, 1000)
        >>> plot.color_by = 'material'
        >>> plot.colors = {pebble_materials.fuel: 'yellow', \
                pebble_materials.buff: 'black', \
                pebble_materials.PyC1: 'gray', \
                pebble_materials.PyC2: 'gray', \
                pebble_materials.SiC:  'silver', \
                pebble_materials.graphite: 'black', \
                pebble_materials.flibe: 'blue'}
        >>> triso_pebble_plot = openmc.Plots([plot])

        Export all to xml
        >>> materials.export_to_xml()
        >>> geometry.export_to_xml()
        >>> settings.export_to_xml()
        >>> triso_pebble_plot.export_to_xml()

        Only after exporting to xml, we can
        plot
        >>> openmc.plot_geometry(output=False)
        >>> image = plot.to_ipython_image()


        run openmc
        >>> openmc.run(output=False)
        """

        pebble_diameter = 4.0

        fuel_sph_radius = pebble_diameter/2 - pebble_graphite_shell_thickness

        fuel_sph = openmc.Sphere(
                r=fuel_sph_radius)
        outer_sph = openmc.Sphere(
                r=pebble_diameter/2)
        outermost_sph = openmc.Sphere(
                r=pebble_diameter)
        outermost_sph.boundary_type = 'vacuum'

        fuel_region = -fuel_sph
        outer_shell_region = -outer_sph & +fuel_sph
        flibe_region = +outer_sph & -outermost_sph

        triso_factory = triso.TrisoParticlesFactory()

        triso_fill = triso_factory.triso_fill(
                pebble_materials,
                fuel_region)

        triso_fuel = openmc.Cell(name='pebble.triso_fuel')
        triso_fuel.fill = triso_fill
        triso_fuel.region = fuel_region
        triso_fuel.temperature = fuel_temp

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
                cells=[triso_fuel,
                       outer_graphite,
                       flibe_peripheral])

        return pebble_univ


if __name__ == '__main__':
    import doctest
    doctest.testmod(verbose=True)
