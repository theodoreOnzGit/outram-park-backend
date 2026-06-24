# Intro 

The purpose of this crate is to build libraries for a neutron bombardment 
simulator for nuclei where the user can watch nuclei transmute and decay 
in real-time.

Basically, it also holds all the libraries necessary for decay simulation,
neutron capture and so on. 

I intend to build a simulator that demonstrates these libraries as a testing 
ground.

# Decay Data 

Decay data was provided by OpenMC depletion chains based on endfb 8. These 
were xml files. However, the files are huge, about 27 Mb in size.

Not only that, there are thousands of Nuclides. How will this work?

We are going to use serde-xml-rs. 

This will take the Nuclide, then access the data library. 
The serde should return the nuclide decay data.


# Scattering Data and Cross Sections

For diffusion, in neutron theory,

D = 1/(3 Sigma Transport) = 1/(3 * Sigma scatter *(1-mubar))

mubar is average scattering cosine.

But for isotropic scattering, mubar = 0



D = 1/(3 Sigma Scatter) 

This helps me correlate diffusion coefficient vs macroscopic 
scattering cross section.

However, one must note though, that neutron flux diffusion coefficient  (m)
is in different units than diffusion coefficient for materials (m^2/s)

For this, we use 

D =  1/6 (lambda^2) * nu 

nu is collision frequency in per second 
lambda is called jump distance (similar to mean free path).

Jump distances are on the order of 2-3 angstroms for SiC. And about 2 
angstroms for PyC
