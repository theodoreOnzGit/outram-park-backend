// the thing about constructive_solid_geometry (CSG) 
// is detemrine where a particle is relative to a shape or plane.
//
// for the sphere, the L2 norm (straight line distance) will suffice 
// as to 


pub mod norms;
use fission_yields_data::prelude::Nuclide;
pub use norms::*;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Region {
    Sphere(Sphere),
}

impl Region {
    pub fn new_sphere(center: [Length; 3], radius: Length) -> Self {

        let sphere: Sphere = Sphere {
            x: center[0],
            y: center[1],
            z: center[2],
            r: radius,
        };


        return Region::Sphere(sphere);

    }

    pub fn is_within_region(&self, point: [Length;3]) -> bool {

        match self {
            Region::Sphere(sphere) => sphere.is_point_in_sphere(point),
        }
    }

    pub fn try_return_center_and_radius_of_sphere(&self) -> 
        Option<([Length;3],Length)>{

            match self {
                Region::Sphere(sphere) => {
                    let centre = [sphere.x, sphere.y, sphere.z];
                    let radius = sphere.r;

                    Some((centre,radius))
                },

            }
    }
}


pub(crate) mod sphere;
pub(crate) use sphere::*;
use uom::ConstZero;
use uom::si::time::second;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::length::{meter, micrometer};
use uom::si::f64::*;


use crate::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::{try_get_diffusion_coeff_jiang, TrisoPebbleLayerMaterial};
use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::chatgpt_vibe_coded_sphere_crossing::{sphere_first_crossing_uom, SphereCrossing};

// for a single triso particle, 
// it is many cocentric spheres together

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct TrisoCell {
    fuel_region: Region,
    buffer_region: Region,
    ipyc_region: Region,
    sic_region: Region,
    opyc_region: Region,

    // temperatures for each region
    fuel_region_temp: ThermodynamicTemperature,
    buffer_region_temp: ThermodynamicTemperature,
    ipyc_region_temp: ThermodynamicTemperature,
    sic_region_temp: ThermodynamicTemperature,
    opyc_region_temp: ThermodynamicTemperature,

    // neutron fluence for the triso as a whole (mean free path is short,
    // I'm just going to use one neutron fluence)
    gamma_neutron_fluence: ArealNumberDensity,

}


impl TrisoCell {
    /// creates a new triso cell based on the radii
    pub fn new(fuel_radius: Length,
        buffer_radius: Length,
        ipyc_radius: Length,
        sic_radius: Length,
        opyc_radius: Length) -> Self {
        // first, need to ensure the lengths are correct
        assert!(buffer_radius > fuel_radius);
        assert!(ipyc_radius > buffer_radius);
        assert!(sic_radius > ipyc_radius);
        assert!(opyc_radius > sic_radius);

        // let's have a center 
        let center = [Length::ZERO, Length::ZERO, Length::ZERO];


        let fuel_region = Region::new_sphere(center, fuel_radius);
        let buffer_region 
            = Region::new_sphere(center, buffer_radius);
        let ipyc_region 
            = Region::new_sphere(center, ipyc_radius);
        let sic_region 
            = Region::new_sphere(center, sic_radius);
        let opyc_region 
            = Region::new_sphere(center, opyc_radius);

        // this makes the default case same as 3a in 
        // Hales, J. D., Jiang, W., Toptan, A., & Gamble, K. A. (2021). 
        // Modeling fission product diffusion in TRISO fuel particles with 
        // BISON. Journal of Nuclear Materials, 548, 152840.
        let default_temperature = ThermodynamicTemperature::new::<degree_celsius>(1600.0);

        // i thought that neutron fluence will cause more diffusion 
        // and hence the bug, 
        // but this is not the case, default fluence is zero
        let default_fluence = ArealNumberDensity::ZERO;

        
        return TrisoCell {
            fuel_region,
            buffer_region,
            ipyc_region,
            sic_region,
            opyc_region,
            fuel_region_temp: default_temperature,
            buffer_region_temp: default_temperature,
            ipyc_region_temp: default_temperature,
            sic_region_temp: default_temperature,
            opyc_region_temp: default_temperature,
            gamma_neutron_fluence: default_fluence,
        };


    }

    /// gotten typical triso geometry from:
    /// Hales, J. D., Williamson, R. L., Novascone, S. R., Perez, D. M., 
    /// Spencer, B. W., & Pastore, G. (2013). Multidimensional 
    /// multiphysics simulation of TRISO particle fuel. Journal of 
    /// Nuclear Materials, 443(1-3), 531-543.
    pub fn new_crp6_geometry() -> Self {

        // Nominal values commonly cited in literature
        let kernel_diameter: Length = Length::new::<micrometer>(425.0);   // diameter
        let buffer_thickness: Length = Length::new::<micrometer>(100.0);  // thickness
        let ipyc_thickness: Length = Length::new::<micrometer>(40.0);     // thickness
        let sic_thickness: Length = Length::new::<micrometer>(35.0);      // thickness
        let opyc_thickness: Length = Length::new::<micrometer>(40.0);     // thickness
                                                                          //
        let inner_kernel_radius: Length = 0.5 * kernel_diameter;
        let buffer_radius: Length = inner_kernel_radius + buffer_thickness;
        let ipyc_radius: Length = buffer_radius + ipyc_thickness;
        let sic_radius: Length = ipyc_radius + sic_thickness;
        let opyc_radius: Length = sic_radius + opyc_thickness;

        Self::new(inner_kernel_radius, buffer_radius, ipyc_radius, sic_radius, opyc_radius)
    }

    /// checks which region the particle is in 
    /// chatgpt fixed
    pub fn get_triso_region(&self, coordinates: [Length;3]) -> TrisoRegion {
        // Choose eps: tune based on your smallest layer thickness.
        // This is a conservative default.
        let eps = Length::new::<meter>(1e-12);

        let x = coordinates[0];
        let y = coordinates[1];
        let z = coordinates[2];
        let r = (x * x + y * y + z * z).sqrt();

        // Extract radii (meters). Assumes concentric spheres.
        let (_, r_fuel) = self.fuel_region.try_return_center_and_radius_of_sphere().unwrap();
        let (_, r_buf)  = self.buffer_region.try_return_center_and_radius_of_sphere().unwrap();
        let (_, r_ipyc) = self.ipyc_region.try_return_center_and_radius_of_sphere().unwrap();
        let (_, r_sic)  = self.sic_region.try_return_center_and_radius_of_sphere().unwrap();
        let (_, r_opyc) = self.opyc_region.try_return_center_and_radius_of_sphere().unwrap();


        // Convention: boundaries belong to the OUTER region.
        // i.e. r == r_fuel -> Buffer, r == r_buf -> IPyC, etc.
        if r < r_fuel - eps {
            TrisoRegion::Fuel
        } else if r < r_buf - eps {
            TrisoRegion::Buffer
        } else if r < r_ipyc - eps {
            TrisoRegion::IPyC
        } else if r < r_sic - eps {
            TrisoRegion::SiC
        } else if r < r_opyc - eps {
            TrisoRegion::OPyC
        } else {
            TrisoRegion::Outside
        }
    }

    /// checks the diffusion coefficient based on coordinates of the 
    /// triso particle
    pub fn try_get_diffusion_coefficient(
        &self, coordinates: [Length;3], 
        nuclide: Nuclide)
        -> Option<DiffusionCoefficient>{

        if self.fuel_region.is_within_region(coordinates) {
            // obtain diffusion coeff for kernel
            let triso_layer = TrisoPebbleLayerMaterial::KernelUO2;
            let diffusion_coeff = try_get_diffusion_coeff_jiang(
                triso_layer, nuclide, 
                self.fuel_region_temp, 
                Some(self.gamma_neutron_fluence)
            );
            return diffusion_coeff;

        } else if self.buffer_region.is_within_region(coordinates) {

            let triso_layer = TrisoPebbleLayerMaterial::Buffer;
            let diffusion_coeff = try_get_diffusion_coeff_jiang(
                triso_layer, nuclide, 
                self.buffer_region_temp, 
                Some(self.gamma_neutron_fluence)
            );
            return diffusion_coeff;
        } else if self.ipyc_region.is_within_region(coordinates) {

            let triso_layer = TrisoPebbleLayerMaterial::PyC;
            let diffusion_coeff = try_get_diffusion_coeff_jiang(
                triso_layer, nuclide, 
                self.ipyc_region_temp, 
                Some(self.gamma_neutron_fluence)
            );
            return diffusion_coeff;
        } else if self.sic_region.is_within_region(coordinates) {

            let triso_layer = TrisoPebbleLayerMaterial::SiC;
            let diffusion_coeff = try_get_diffusion_coeff_jiang(
                triso_layer, nuclide, 
                self.sic_region_temp, 
                Some(self.gamma_neutron_fluence)
            );
            return diffusion_coeff;
        } else if self.opyc_region.is_within_region(coordinates) {

            let triso_layer = TrisoPebbleLayerMaterial::PyC;
            let diffusion_coeff = try_get_diffusion_coeff_jiang(
                triso_layer, nuclide, 
                self.opyc_region_temp, 
                Some(self.gamma_neutron_fluence)
            );
            return diffusion_coeff;
        } 

        // if it is not within any of these regions

        return None;


    }

    /// for the outside region, i just get opyc radius, not going to 
    /// really bother timestepping well
    #[inline]
    pub fn get_lengthscale_for_fourier_number(&self,
        coordinates: [Length;3], ) -> Length {

        let triso_cell_region: TrisoRegion = 
            self.get_triso_region(coordinates);

        match triso_cell_region {
            TrisoRegion::Fuel => return self.get_fuel_radius(),
            TrisoRegion::Buffer => return self.get_buffer_radius() - self.get_fuel_radius(),
            TrisoRegion::IPyC => return self.get_ipyc_radius() - self.get_buffer_radius(),
            TrisoRegion::SiC => return self.get_sic_radius() - self.get_ipyc_radius(),
            TrisoRegion::OPyC => return self.get_opyc_radius() - self.get_sic_radius(),
            TrisoRegion::Outside => return self.get_opyc_radius() ,
        };

    }


    #[inline]
    pub fn get_time_to_sphere_boundary(&self,
        position: [Length;3],
        velocity: [Velocity;3],) -> Option<Time>{

        TrisoRegion::get_time_to_sphere_boundary(position, velocity, *self)
    }

    #[inline]
    /// Sets a uniform temperature across all regions of the TRISO cell.
    pub fn set_uniform_temperature(&mut self, temp: ThermodynamicTemperature) {
        self.fuel_region_temp = temp;
        self.buffer_region_temp = temp;
        self.ipyc_region_temp = temp;
        self.sic_region_temp = temp;
        self.opyc_region_temp = temp;
    }

    #[inline]
    /// Gets the current uniform temperature of the TRISO cell.
    /// Assumes all regions have the same temperature.
    pub fn get_uniform_temperature(&self) -> ThermodynamicTemperature {
        self.fuel_region_temp // Can return any of the region temperatures as they are uniform
    }

    
    // NEW: Getter methods for the radius of each layer
    #[inline]
    pub fn get_fuel_radius(&self) -> Length {
        self.fuel_region.try_return_center_and_radius_of_sphere().unwrap().1
    }

    #[inline]
    pub fn get_buffer_radius(&self) -> Length {
        self.buffer_region.try_return_center_and_radius_of_sphere().unwrap().1
    }

    #[inline]
    pub fn get_ipyc_radius(&self) -> Length {
        self.ipyc_region.try_return_center_and_radius_of_sphere().unwrap().1
    }

    #[inline]
    pub fn get_sic_radius(&self) -> Length {
        self.sic_region.try_return_center_and_radius_of_sphere().unwrap().1
    }

    #[inline]
    pub fn get_opyc_radius(&self) -> Length {
        self.opyc_region.try_return_center_and_radius_of_sphere().unwrap().1
    }
}

// question is, how to do particle tracing if the length crosses boundary 
// of the sphere?



#[derive(Debug, PartialEq, Clone, Copy)]
pub enum TrisoRegion {
    Fuel,
    Buffer,
    IPyC,
    SiC,
    OPyC,
    Outside
}

impl TrisoRegion {
    #[inline]
    pub fn get_time_to_sphere_boundary(
        position: [Length; 3],
        velocity: [Velocity; 3],
        triso_cell: TrisoCell,
    ) -> Option<Time> {
        // Filter out "hits" that are at t=0 or negative (common when starting on boundary)
        const T_EPS_S: f64 = 1e-15;
        let t_eps = Time::new::<second>(T_EPS_S);

        #[inline]
        fn crossing_time_any(c: Option<SphereCrossing>, t_eps: Time) -> Option<Time> {
            let t = match c {
                Some(SphereCrossing::Exit { t }) => Some(t),
                Some(SphereCrossing::Entry { t }) => Some(t),
                None => None,
            };
            t.filter(|&tt| tt > t_eps)
        }

        #[inline]
        fn pick_min_positive(t1: Option<Time>, t2: Option<Time>) -> Option<Time> {
            match (t1, t2) {
                (Some(a), Some(b)) => Some(if a < b { a } else { b }),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            }
        }

        let current_region = triso_cell.get_triso_region(position);

        match current_region {
            TrisoRegion::Fuel => {
                let (center, radius) = triso_cell
                    .fuel_region
                    .try_return_center_and_radius_of_sphere()
                    .unwrap();
                crossing_time_any(
                    sphere_first_crossing_uom(center, radius, position, velocity),
                    t_eps,
                )
            }

            TrisoRegion::Buffer => {
                let (c1, r1) = triso_cell
                    .fuel_region
                    .try_return_center_and_radius_of_sphere()
                    .unwrap();
                let (c2, r2) = triso_cell
                    .buffer_region
                    .try_return_center_and_radius_of_sphere()
                    .unwrap();
                assert_eq!(c1, c2);

                let t_inner =
                    crossing_time_any(sphere_first_crossing_uom(c1, r1, position, velocity), t_eps);
                let t_outer =
                    crossing_time_any(sphere_first_crossing_uom(c2, r2, position, velocity), t_eps);
                pick_min_positive(t_inner, t_outer)
            }

            TrisoRegion::IPyC => {
                let (c1, r1) = triso_cell
                    .buffer_region
                    .try_return_center_and_radius_of_sphere()
                    .unwrap();
                let (c2, r2) = triso_cell
                    .ipyc_region
                    .try_return_center_and_radius_of_sphere()
                    .unwrap();
                assert_eq!(c1, c2);

                let t_inner =
                    crossing_time_any(sphere_first_crossing_uom(c1, r1, position, velocity), t_eps);
                let t_outer =
                    crossing_time_any(sphere_first_crossing_uom(c2, r2, position, velocity), t_eps);
                pick_min_positive(t_inner, t_outer)
            }

            TrisoRegion::SiC => {
                let (c1, r1) = triso_cell
                    .ipyc_region
                    .try_return_center_and_radius_of_sphere()
                    .unwrap();
                let (c2, r2) = triso_cell
                    .sic_region
                    .try_return_center_and_radius_of_sphere()
                    .unwrap();
                assert_eq!(c1, c2);

                let t_inner =
                    crossing_time_any(sphere_first_crossing_uom(c1, r1, position, velocity), t_eps);
                let t_outer =
                    crossing_time_any(sphere_first_crossing_uom(c2, r2, position, velocity), t_eps);
                pick_min_positive(t_inner, t_outer)
            }

            TrisoRegion::OPyC => {
                let (c1, r1) = triso_cell
                    .sic_region
                    .try_return_center_and_radius_of_sphere()
                    .unwrap();
                let (c2, r2) = triso_cell
                    .opyc_region
                    .try_return_center_and_radius_of_sphere()
                    .unwrap();
                assert_eq!(c1, c2);

                let t_inner =
                    crossing_time_any(sphere_first_crossing_uom(c1, r1, position, velocity), t_eps);
                let t_outer =
                    crossing_time_any(sphere_first_crossing_uom(c2, r2, position, velocity), t_eps);
                pick_min_positive(t_inner, t_outer)
            }

            TrisoRegion::Outside => {
                let (center, radius) = triso_cell
                    .opyc_region
                    .try_return_center_and_radius_of_sphere()
                    .unwrap();
                crossing_time_any(
                    sphere_first_crossing_uom(center, radius, position, velocity),
                    t_eps,
                )
            }
        }
    }
}



/// this is a vibe coded sphere crossing code
/// to determine time to sphere crossing
pub mod chatgpt_vibe_coded_sphere_crossing;
