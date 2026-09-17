UC Berkeley UC Berkeley Electronic Theses and Dissertations

Title Coupled neutronics and thermal-hydraulics modeling for pebble-bed Fluoride-Salt-Cooled, High-Temperature Reactor (FHR)

Permalink https://escholarship.org/uc/item/40q3985m

Author Wang, Xin

Publication Date 2018

Peer reviewed|Thesis/dissertation

eScholarship.org Powered by the California Digital Library University of California

Coupled neutronics and thermal-hydraulics modeling for pebble-bed Fluoride-Salt-Cooled, High-Temperature Reactor (FHR)

by

Xin Wang

A dissertation submitted in partial satisfaction of the

requirements for the degree of

Doctor of Philosophy

in

Engineering - Nuclear Engineering

in the

Graduate Division

of the

University of California, Berkeley

Committee in charge:

Professor Per F. Peterson, Chair Professor Massimillio Fratoni Professor Anil Aswani

Summer 2018

Coupled neutronics and thermal-hydraulics modeling for pebble-bed Fluoride-Salt-Cooled, High-Temperature Reactor (FHR)

Copyright 2018

by

Xin Wang 1

## Abstract

Coupled neutronics and thermal-hydraulics modeling for pebble-bed Fluoride-Salt-Cooled, High-Temperature Reactor (FHR)

by

Xin Wang

Doctor of Philosophy in Engineering - Nuclear Engineering

University of California, Berkeley

Professor Per F. Peterson, Chair

Advances in computer abilities, intense competition on the energy market and stringent regulatory requirements during the last decade have spurred the development of robust numerical models to support nuclear reactor safety analysis and design optimization. This dissertation aims to develop methodologies for numerical modeling of Pebble-Bed Fluoride Salt Cooled Reactors (PB-FHR), a novel reactor design that combines TRISO particle fuel and ﬂibe salt coolant.

The use of a large number of fuel pebbles in PB-FHR cores poses great challenge in computational cost and violates the assumptions in most of the traditional deterministic codes developed for Light Water Reactors (LWRs). This project developed dedicated models with different levels of spatial and energy resolution for the broad need in FHR design and analysis, including coupled heat diffusion and point kinetics unit cell models, Monte Carlo neutronics models, and coupled multi-group neutron diffusion and multi-scale porous media model. Documentation, version control, testing, veriﬁcation are all indispensable parts of numerical modeling software development. These steps are followed meticulously to ensure high quality open source codes that promote open science and reproducible research.

Unit cell models can compute representative fuel and coolant temperatures and full core power within a short amount of time and thus is adequate for scoping analysis, or uncertainty and sensitivity analysis, where a large number of runs is required to cover the input space. FHRs have substantial graphite reﬂectors that slows down the neutron generation time by hosting

2

neutrons while they get thermalized. A ’multi-point’ correction is derived from perturbation theory to take the reﬂector effects on reactivity and neutron generation time into account.

Monte Carlo based codes, on the other extreme, can provide accurate results with minimal assumptions, but they are typically only used for generating cross-sections for deterministic models, for example point kinetics models and multi-group neutron diffusion models in this project, or to provide reference results for benchmarking lower resolution codes due to high computational cost. PB-FHR has not yet test reactors to provide experimental data for tool validation. High ﬁdelity models based on direct coupling between neutron transport and Computational Fluid-Dynamics (CFD) was used in this project as reference for code-to-code veriﬁcation.

Multi-group neutron diffusion models were developed for design optimization and safety analysis, because they are compatible with current computation resources in nuclear industry, with which simulations can be carried out on stand-alone workstations or small computation clusters within a reasonable time. For areas where the diffusion assumption is limited, e.g., in vicinity of control rods, the simpliﬁed spherical harmonics equations were implemented to improve the accuracy of diffusion equation by relaxing the isotropic assumption in neutron transport. The neutronics model is coupled to a porous media CFD module with multi-scale treatment that computes conductive heat transfer inside TRISO particles and fuel pebbles, as well as convective heat transfer between fuel pebbles and coolant. Radiative heat transfer may be signiﬁcant in the high temperature reactors, but is not modeled in this project due to lack of material properties measurement.

Results from the coupled full core model were veriﬁed with analytical solutions when they exist, for example the steady state outlet bulk average temperature, computed from energy balance, or a reference Monte Carlo model. Although the absolute value of the multiplication factor can not be accurately matched between the Monte Carlo statistics and the eigenvalue found from the coupled model, this can be corrected and more important for transient modeling is the change in the multiplication factor during a transient. In fact, the important parameters for a transient study, such as ﬂibe and fuel temperature feedback coefﬁcients, time scale, and control rod worth matches well between the coupled model and the reference model.

The multiphysics models are capable of simulating both steady state and a broad spectrum of transient scenarios, involving either coolant inlet condition or reactivity induced transients, especially Anticipated Transient Without Scram (ATWS). The methodology was applied to the TMSR SF-1 and Mk1 design, which are both FHRs that uses TRISO fuel particles in spherical fuel elements and Fluoride salt coolant. For both designs, steady state power, neutron ﬂux, and temperature distributions were computed, and reactivity insertion and overcooling transients were simulated. The study shows that FHR cores are extremely resilient to the investigated transient scenarios, for the following reasons: 3

• The graphite based fuel elements in FHRs can withstand up to 1600 ◦C without risk of radioactive release. And the ﬂibe coolant used in FHRs also has high thermal resistance, with a boiling point at 1430 ◦C . Due to the large thermal margin to the failure of TRISO particles, the thermal constraints will be more likely in the metallic structures. These temperature limits should be deﬁned based on both the temperature and the time of exposure for creep limits or reduced yield stress limits at elevated temperatures.

• In FHRs, the role of coolant and moderator are separated. The Doppler feedback, the main mechanism to stabilize the reactor during an ATWS, is more negative than in LWRs.

• Online refueling fuel management regime allows the reactor to operate at a low excess of reactivity because the reserve for compensating fuel burnup is small in this case. i

## Acknowledgments

First and foremost, I would like to thank my supportive, understanding and brilliant advisers Professor Peterson and Professor Fratoni. The joy and enthusiasm they have for the research was contagious and motivational for me. Thank you both for allowing me to explore freely on my own and yet always being there with great advises when I needed.

The Thermal Hydraulics lab has been a great source of lifelong friendships as well as good advice and collaboration. I am especially grateful for the fun group of people who spend the years with me at graduate school.

A special thanks to my family, my mother, father, my mother-in-law and father-in-law for all their love and sacriﬁces they have made on my behalf. I promise that I will ﬁnd time to go back to China more often in the future. And most of all to my husband, thank you for believing in me. I would never have even considered coming to the US for a PhD without your faithful support. And my son, Nathan, I could probably ﬁnish the dissertation sooner without your help, but the invaluable lessons you have taught me about how mesmerizing the world is and how absorbent a person’s mind can be are as precious as the nuclear reactor courses at graduate school. I am the luckiest person in the world to have you two in my life. With you, I am ready to embark on the next adventure. ii

Contents

Contents ii

List of Figures v

List of Tables viii

## 1 Introduction 1

1.1 Background . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 1 1.2 Multi-physics modeling for nuclear reactors . . . . . . . . . . . . . . . . . . . . . . 3 1.2.1 Neutronics methods . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 4 1.2.2 Thermal-hydraulics methods . . . . . . . . . . . . . . . . . . . . . . . . . . . 7 1.2.3 Coupling between thermal-hydraulics and neutronics . . . . . . . . . . . . 9 1.2.4 Numerical tools . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 11 1.3 Research scope and thesis outline . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 12

## 2 Multiphysics modeling methodology 13

2.1 Monte Carlo neutronics modeling . . . . . . . . . . . . . . . . . . . . . . . . . . . . 13 2.1.1 Serpent input generator . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 14 2.2 Coupled neutronics and thermal-hydraulics models . . . . . . . . . . . . . . . . . . 17 2.2.1 Unit cell multi-physics model . . . . . . . . . . . . . . . . . . . . . . . . . . . 18 2.2.2 Full core multi-physics model . . . . . . . . . . . . . . . . . . . . . . . . . . 24 2.3 Conclusion and future work . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 36

## 3 TMSR SF-1 core analysis 37

3.1 TMSR SF-1 design overview . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 38 3.2 Multi-physics modeling for TMSR SF-1 . . . . . . . . . . . . . . . . . . . . . . . . . . 41 3.2.1 Mesh reﬁnement study . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 44 3.3 Code-to-code veriﬁcation . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 46 3.3.1 Overview of the reference model . . . . . . . . . . . . . . . . . . . . . . . . . 46 3.3.2 Power distribution comparison . . . . . . . . . . . . . . . . . . . . . . . . . . 46 3.3.3 Multiplication factor comparison . . . . . . . . . . . . . . . . . . . . . . . . 49 3.3.4 Time constant comparison . . . . . . . . . . . . . . . . . . . . . . . . . . . . 49

iii

3.3.5 Temperature feedback comparison . . . . . . . . . . . . . . . . . . . . . . . 50 3.4 TMSR SF-1 steady state results . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 53 3.5 TMSR SF-1 transient analysis . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 55 3.5.1 Reactivity insertion transients . . . . . . . . . . . . . . . . . . . . . . . . . . 56 3.5.2 Overcooling transients . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 63 3.6 Conclusion . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 66

## 4 Mark-1 PB-FHR core analysis 68

4.1 Mk1 core design overview . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 68 4.2 Monte Carlo modeling for Mk1 . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 72 4.2.1 Center reﬂector . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 74 4.2.2 Fuel region . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 76 4.2.3 Blanket pebble region . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 76 4.2.4 Outer reﬂector . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 77 4.2.5 Outer layers . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 77 4.3 Multiphysics modeling for Mk1 . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 77 4.3.1 Porous media model for coolant . . . . . . . . . . . . . . . . . . . . . . . . . 78 4.3.2 Multi-scale model for the fuel region . . . . . . . . . . . . . . . . . . . . . . 79 4.3.3 Control rod modeling and veriﬁcation . . . . . . . . . . . . . . . . . . . . . 82 4.4 Steady state Mk1 core analysis and design optimization . . . . . . . . . . . . . . . . 85 4.4.1 Power and ﬂux distribution . . . . . . . . . . . . . . . . . . . . . . . . . . . . 85 4.4.2 Coolant ﬂow optimization . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 86 4.5 Transient results . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 95 4.5.1 Overcooling transients . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 95 4.5.2 Control rod removal transients . . . . . . . . . . . . . . . . . . . . . . . . . . 96 4.5.3 Seismic reactivity transients in Mk1 core . . . . . . . . . . . . . . . . . . . . 99 4.6 Conclusion . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 100

## 5 Conclusions and future work 102

A Serpent input generator example 105

B FHR core components material thermo-physical properties 108 B.1 Fuel elements . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 108 B.2 Coolant . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 110

C Mk1 core Monte Carlo model geometry and material speciﬁcations 112

D Implementing the diffusion and SPN algorithms via the ’user deﬁned PDEs’ interface in COMSOL 115 D.1 Diffusion equations . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 116 D.2 SP3 equations . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 117

iv

Bibliography 119

v

List of Figures

1.1 Schematic of fuel pebbles and triso particles . . . . . . . . . . . . . . . . . . . . . . . . 6

2.1 Illustration of the association between the important objects in FIG, such as core components, generators and serpent concepts like universes and cells . . . . . . . . . 15 2.2 Illustration of the material class design in FIG . . . . . . . . . . . . . . . . . . . . . . . . 16 2.3 Constituting components of the core . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 17 2.4 Illustration of levels of universes in a fuel region deﬁnition . . . . . . . . . . . . . . . . 17 2.5 Illustration of the unit cell model geometry (based on Mk1 design) . . . . . . . . . . . 23

3.1 Schematic of the TMSR core design . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 39 3.2 Schematic of fuel pebbles and triso particles in TMSR SF-1 . . . . . . . . . . . . . . . . 40 3.3 Schematic of three dimensional and two dimensional TMSR SF-1 core model geometry 41 3.4 Macroscopic cross-sections of major isotopes in the core(vertical bars are energy group bounds, MT2=scattering, MT18=ﬁssion, MT102=absorption) . . . . . . . . . . . 42 3.5 Logarithmic decrease in computation error with increase in degree of freedom for both linear elements and quadratic elements . . . . . . . . . . . . . . . . . . . . . . . . 45 3.6 An example of reference results showing the pebble wise power generation in the fuel region [4] . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 47 3.7 Steady state power proﬁle computed from the COMSOL model and the Monte Carlo reference model . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 48 3.8 Comparison of the axial distribution of normalized power . . . . . . . . . . . . . . . . 48 3.9 Comparison of the radial distribution of normalized power . . . . . . . . . . . . . . . . 49 3.10 Comparison between the diffusion based model and the Monte Carlo based model for zero power reactivity insertion transients . . . . . . . . . . . . . . . . . . . . . . . . 50 3.11 Comparison between the diffusion based model and the Monte Carlo based model for fuel and ﬂibe temperature (and void) feedback effect. Error bar shows statistical error in Monte Carlo results. . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 51 3.12 Steady state neutron ﬂux . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 53 3.13 Power distribution in TMSR SF-1 . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 54 3.14 Steady state temperatures in TMSR SF-1 core, without multiscale treatment . . . . . . 54 3.15 Steady state temperatures of coolant and selected locations in fuel element in TMSR SF-1 core, with multiscale treatment . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 55

vi

3.16 Full core power during a 1$ reactivity insertion transient in TMSR SF-1, with or without multiscale treatment . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 56 3.17 Average and maximum ﬂibe temperatures during a 1$ reactivity insertion transient in TMSR SF-1, with or without multiscale treatment . . . . . . . . . . . . . . . . . . . . 57 3.18 Average and maximum fuel temperatures during a 1$ reactivity insertion transient in TMSR SF-1, with or without multiscale treatment . . . . . . . . . . . . . . . . . . . . 58 3.19 Core average temperature inside fuel elements during 1$ reactivity insertion transient for TMSR SF-1, with a large thermal conductivity posed to all material(top) or with nominal material properties for each layer(bottom). Notation for temperatures: T Pi j represents the jth TRISO layer in the ith pebble layer. (A fuel core in a pebble is divided into three layers and within each layer, a TRISO particle is divided into four layers - three fuel layers and one coating layer.). And T Pshel l l is the temperature in the graphite shell of the pebble. . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 60 3.20 RI transients results . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 61 3.21 RI transients results . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 62 3.22 Full core power during overcooling transient . . . . . . . . . . . . . . . . . . . . . . . . 63 3.23 Core maximum and average ﬂibe temperature during overcooling transient . . . . . . 64 3.24 Core average and maximum fuel temperature during overcooling transient . . . . . . 65 3.25 Core average of temperatures inside fuel pebbles during overcooling transient for TMSR SF-1. Notation for temperatures: T Pi j represents the ith TRISO layer in the jth pebble layer. T Pshel l l is the temperature in the graphite shell of the pebble. . . . . 66

4.1 Schematic of Mk1 core geometry . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 69 4.2 Schematic of fuel pebbles and TRISO particles that are used in Mk1 PB-FHR . . . . . 71 4.3 Geometry of the Monte Carlo model for Mk1 in Serpent (View from the front, the top, and color legend), with all control rods fully inserted . . . . . . . . . . . . . . . . . 73 4.4 Center reﬂector design . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 74 4.5 Schematic of equilibrium fuel loading in a representative unit cell, numbers denote burnup levels . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 76 4.6 Schematic of the Mk1 multiphysics model geometry. Green region is the fuel pebble region and blue region is the blanket pebble region. . . . . . . . . . . . . . . . . . . . . 78 4.7 Fraction of power generated in pebbles of each burnup level . . . . . . . . . . . . . . . 80 4.8 Fuel pebble conductivity as a function of temperature and irradiation dose(data from [20]) . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 81 4.9 Steady state power distribution from the Monte Carlo reference model and from the COMSOL model (nominal conditions, no control rods inserted in the core) . . . . . . 83 4.10 Steady state neutron ﬂux distribution from the Monte Carlo reference model and from the COMSOL model (nominal conditions, no control rods in the core) . . . . . . 83 4.11 Steady state power distribution from the Monte Carlo reference model and from the COMSOL model(nominal conditions, control rods fully inserted in the core) . . . . . 84 4.12 Steady state neutron ﬂux distribution from the Monte Carlo reference model and from the COMSOL model(nominal conditions, control rods fully inserted in the core) 84

vii

4.13 Change in keff with control rod positions . . . . . . . . . . . . . . . . . . . . . . . . . . . 85 4.14 Steady state power and neutron ﬂux distribution at operating conditions . . . . . . . 86 4.15 Schematic of Mk1 core inlet and outlet locations . . . . . . . . . . . . . . . . . . . . . . 87 4.16 Steady state coolant ﬂow streamline and pressure distribution. (Color bar legend for velocity, Isobar proﬁle in black.) . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 88 4.17 Steady state local convective heat transfer coefﬁcient, computed from Wakao correlation, and the Reynolds number . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 89 4.18 Center reﬂector coolant injection velocity boundary condition . . . . . . . . . . . . . . 90 4.19 Steady state coolant temperature distribution and streamline under different inlet conditions . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 92 4.20 Steady state fuel temperature distribution and streamline . . . . . . . . . . . . . . . . 93 4.21 Histogram of coolant outlet temperature (weighted by the surface area) . . . . . . . . 94 4.22 Full core power level during an overcooling transient . . . . . . . . . . . . . . . . . . . 95 4.23 Maximum fuel temperature during an overcooling transient . . . . . . . . . . . . . . . 96 4.24 Average outlet coolant temperature during an overcooling transient . . . . . . . . . . 96 4.25 Average fuel and coolant temperature during an overcooling transient . . . . . . . . . 97 4.26 Illustration of removed control rods during the control rods removal transient . . . . 97 4.27 Full core power during a control rod removal transient . . . . . . . . . . . . . . . . . . 98 4.28 Average output ﬂibe temperatures during a control rod removal transient . . . . . . . 98 4.29 Maximum fuel temperatures during a control rod removal transient . . . . . . . . . . 98

B.1 Pebble-wise equivalent thermal conductivity as a function of temperature and irradiation dose in FHR fuel . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 109 B.2 Pebble-wise speciﬁc heat as a function of temperature and irradiation dose in FHR fuel . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 110

viii

List of Tables

1.1 Properties of difference coolant choices (Tmel t [◦C ]: melting point, Tboi l [◦C ]: boiling point, ρ [kg /m3]: density, Cp [KJ/kg/K]: speciﬁc heat capacity at constant pressure, ρCp [K J /m3/K ]: volumetric heat capacity), all values from [1] . . . . . . . . . . . . . . 3 1.2 Comparison between reactor physics computational methods . . . . . . . . . . . . . . 8

2.1 Effect of reﬂectors on keff and prompt neutron lifetime in Mk1 PB-FHR . . . . . . . . 19

3.1 TMSR design parameters . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 38 3.2 TRISO particle geometry in TMSR . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 40 3.3 Fuel pebble geometry in TMSR . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 41 3.4 Energy group structure adopted in the multi-group neutron diffusion model . . . . . 43 3.5 Nominal material properties of TMSR fuel element used in the models [17][14]. *Coating combines all the non-power-generating layers in a TRISO particle. . . . . . . . . . 43 3.6 Nominal material properties of TMSR fuel pebble used in the models [17][14]. *Pebble: pebble-wise material properties . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 44 3.7 Comparison of multiplication factor . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 49

4.1 Mk1 PB-FHR core design parameters . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 70 4.2 TRISO particle geometry in Mk1 PB-FHR [1] . . . . . . . . . . . . . . . . . . . . . . . . . 70 4.3 Fuel pebble geometry in Mk1 PB-FHR [1] . . . . . . . . . . . . . . . . . . . . . . . . . . 71 4.4 Material and temperature for each core component in the reference Mk1 Serpent model. The isotope composition of the stainless steel in core barrel and vessel is given in table 4.5[30]. . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 72 4.5 Nominal composition of stainless steel 316, density 8.03g /cm3 . . . . . . . . . . . . . 73 4.6 Design speciﬁcation for control rods in Mk1 . . . . . . . . . . . . . . . . . . . . . . . . . 75 4.7 Drop in keff due to different control rod liner materials and conﬁgurations . . . . . . . 75 4.8 Dimension and material of Mark I core outer annular layers with shield[7] . . . . . . . 77 4.9 Dimension and material of Mark I core outer annular layers without shield[1] . . . . . 77 4.10 Pressure drop correlation parameters used in the porous media model . . . . . . . . . 79 4.11 Thermal conductivity of fuel pebbles at different burnup levels . . . . . . . . . . . . . 81 4.12 Nominal material properties of Mk1 fuel element used in the models [17][14]. *Coating combines all the non-power-generating layers in a TRISO particle. . . . . . . . . . 82

ix

4.13 Nominal material properties of Mk1 fuel pebble used in the models [17][14]. *Pebble: pebble-wise material properties . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 82 4.14 Change in keff during an earthquake with different amount of fuel densiﬁcation equilibrium fuel (* ∆ keff = keff d enser - keff 60%, **uncertainty: uncertainty associated to Monte Carlo computation) . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 100 4.15 Change in keff during an earthquake with different amount of fuel densiﬁcation fresh fuel (* ∆ keff = keff d enser - keff 60%, **uncertainty: uncertainty associated to Monte Carlo computation) . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . . 100

B.1 Nominal material properties of TMSR fuel element used in the models [17][14][20]. *Coating combines all the non-power-generating layers in a TRISO particle. . . . . . . 108 B.2 Nominal material properties of TMSR fuel pebble used in the models [17][14][20]. *Pebble: pebble-wise material properties . . . . . . . . . . . . . . . . . . . . . . . . . . 109 B.3 Flibe salt thermo-physical properties and uncertainty with 95% conﬁdence . . . . . . 111

C.1 Dimensions of Mk1 center reﬂector in the Monte Carlo model . . . . . . . . . . . . . . 112 C.2 Dimension of Mk1 fuel region in the Monte Carlo model . . . . . . . . . . . . . . . . . 113 C.3 Dimension of blanket pebble region in Mk1 core . . . . . . . . . . . . . . . . . . . . . . 114 C.4 Dimension of Mk1 outer reﬂector in the Monte Carlo model . . . . . . . . . . . . . . . 114

1

Chapter 1

## Introduction

### 1.1 Background

The ever-growing demand in energy and exacerbating environmental issues create a need for efﬁcient use and optimized allocation of fossil fuels, hydro power, nuclear energy, and other sources of energy. Nuclear power and hydro-power are the only low carbon energy sources that can supply reliable base load energy on a large scale. Nuclear energy has been used since the 1950’s and will continue to be a valuable building block of national energy portfolios for many countries.

The molten salt reactor is one of the six next generation reactor types identiﬁed by the Generation IV International Forum (GIF), an international collective of 13 countries, after reviewing about one hundred concepts on criteria of sustainability, security, safety and economics. To avoid confusion, two variants of molten salt reactors must be carefully distinguished: 1) liquid fuel variant with ﬁssile material dissolved in the circulating fuel salt and 2) solid fuel variant studied in this work, where the salt is only used for cooling solid fuel elements, also referred to as Fluoride-salt-cooled High-temperature Reactors (FHRs). These two types of technology differ a lot in the forms of the fuel, but share advantages and challenges associated with the use of molten salt in the reactor core. Research on molten salt reactors dates back to the Aircraft Reactor Experiment (ARE) in 1954 and the Molten-Salt Reactor Experiment (MSRE), a prototype for a thorium fuel cycle breeder reactor nuclear power plant, during the 1960’s. The molten salt technology was chosen in these projects for the compactness that the design could provide. Both projects resulted in a critical test reactor, but unfortunately the molten salt reactor research was halted when the federal government closed down the MSRE program at Oak Ridge National Laboratory (ORNL) in 1972.

The concept of a solid-fuel salt-cooled reactor was not invented until the twenty-ﬁrst century. Since then molten salt technologies have received renewed international interest because

CHAPTER 1. INTRODUCTION 2

of their potential in intrinsic safety and economic competitiveness. In this context, over the last 6 years the U.S. Department of Energy has funded Integrated Research Projects (IRPs) to study pebble bed FHR technology. As an important part of this research, this dissertation is focused on the development, veriﬁcation, and application of numerical tools for coupled neutronics/thermal hydraulics modeling of the core in support of the understanding and licensing of FHRs.

The FHR design outperforms traditional reactors due to its fuel and coolant choices. Light Water Reactor (LWR) fuel assemblies use Zirconium cladding for its low neutron absorption property. However, if left reacting with high temperature water steam, it could produce highly ﬂammable hydrogen and cause explosions like the one in Fukushima accident. FHRs instead use TRISO-coated fuel particles in graphite matrix, which is more robust against over-heating accidents. The TRISO particles have tiny uranium fuel kernels encased in multiple layers that all provide radiation containment and ensure structural integrity. The porous graphite buffer layer serves to contain the gaseous ﬁssion products and to damp the stress from the nuclear irradiation induced swelling and contraction. The inner and outer pyrolytic layers feature high density and low permeability, providing diffusion barrier to ﬁssion products and structural support for the silicon carbide layer. The research center julich (KFA) in Germany [41] has tested fully irradiated TRISO coated fuel elements in the temperature range from 1600◦C to 1800◦C and concluded that the integrity of the fuel coatings with regard to the safety-relevant ﬁssion products retention has been proved experimentally up to 1600◦C for high burnup fuels, and the allowable temperature may be 1700-1900◦C if the maximum burnup is kept to 11% ﬁssions per initial metal atom.

In addition to the excellent thermal resistance of the material in the fuel element, pebble fueled core can operate at low or even zero excess of reactivity because the granularity of fuel pebbles. Reactor can reach and maintain criticality by adjusting the amount of fuel in the core. This reduces the risk of criticality accidents where the reactor power would surge exponentially within a short amount of time which is challenging to control.

The thermophysical properties of popular coolant choices for generation IV reactors are compared in table 1.1. Flibe has the following advantages comparing to other choices:

• Unlike the water-cooled reactors, the coolant in the FHRs (ﬂibe salt) has a high boiling point at 1430 ◦C at atmospheric pressure [42]. This property allows for low-pressure operation in the core, reducing the capital cost for the reactor vessel and the high pressure safety system.

• Flibe forms stable compounds with various ﬁssion products and minimizes the amount of radioactive release to the environment during hypothetical severe accidents.

CHAPTER 1. INTRODUCTION 3

Material Tmel t Tboi l ρ Cp ρCp

Flibe 459 1430 1940 2.34 4540 Sodium 97.8 883 790 1.27 1000 Lead 328 1750 10540 0.16 1700 Helium (7.5 MPa) 3.8 5.2 20 Water (7.5 MPa) 0 100 732 5.5 4040

Table 1.1: Properties of difference coolant choices (Tmel t [◦C ]: melting point, Tboi l [◦C ]: boiling point, ρ [kg /m3]: density, Cp [KJ/kg/K]: speciﬁc heat capacity at constant pressure, ρCp [K J /m3/K ]: volumetric heat capacity), all values from [1]

• Opposite to liquid metal coolants, liquid ﬂibe salt is transparent, enabling visual inspections of the core components, which reduces maintenance cost and risk of major problems by detecting issues early on.

• The volumetric heat capacity of ﬂibe is 4540 k J /m3.K , four times more than that of sodium, and 200 times more than that of helium [22]. A compact core reduces material consumption in nuclear power plant construction, although the cost of material only constitutes approximately 10% of the capital cost. More importantly, this reduces the risk and challenge in supply chain, transportation, and project management associated with large components.

### 1.2 Multi-physics modeling for nuclear reactors

FHRs’ ability to provide low cost heat and electricity with intrinsic safety allows for economic competitiveness in the current and future energy market. However, research and development efforts are needed before FHRs are ready for large scale commercial deployment. In fact, before the ﬁrst construction can be licensed, numerical models are needed to predict the behavior of a FHR under various conditions. Models for reactor core are needed to compute the value of safety parameters, as well as their variation due to changes in the core or to its boundaries, including the following:

• Neutron ﬂux, which provides source terms for computing power (given ﬁssion crosssections), material damage (given scattering cross-sections), radionuclide production (given production reaction cross-sections).

• Peak coolant outlet temperature, one of the most important parameters in a FHR core, which is a direct indication of the peak temperature in the metallic components, the most vulnerable material during a high temperature transient.

CHAPTER 1. INTRODUCTION 4

• Minimum coolant temperature, in particular for FHRs due to the high melting temperature of its coolant salt, to determine the safety margin for salt freezing.

• Peak fuel temperature, which is important to prevent radioactive release from fuel damage. Due to the high thermal margin to fuel damage during normal operation or passive decay heat removal, this parameter is not likely to be the limiting factor for power levels in FHRs like it is for reactor technologies that use metallic fuel cladding.

• Pressure drop across the core, together with pressure loss in other sections of the primary loop helps with the selection of the primary pump, shows the risks of loss of primary pump accidents and demonstrates the feasibility of passive heat removal during natural circulation.

• Flow distribution, which affects local heat transfer and subsequently temperature, as well as vibration induced damage.

• Temperature distribution and its variation in time that can cause thermal stress in material, which is one of the key factors affecting material life-time.

Studies of the aforementioned parameters span three domains of physics, from neutronics (for neutron population and nuclear power distribution) to hydraulics (for pressure and ﬂow distribution) and heat transfer (for temperature ﬁelds within the nuclear fuel and heat ﬂux between fuel, coolant, and other components in the core). In the next sections, we discuss modeling methods in neutronics and thermal-hydraulics, and coupling methods between these physics.

#### 1.2.1 Neutronics methods

Neutronics modeling aims to compute neutron population and movement in space, direction, time and energy. This section provides the mathematical formulation of the neutron transport equation (NTE) and discuss common simpliﬁcation methods for solving the transport equation.

The NTE characterizes the neutron balance between the various mechanisms by which neutrons can be gained or lost from a control volume. The time dependent neutron transport equation (without delayed neutrons) is shown in equation 1.1.

CHAPTER 1. INTRODUCTION 5

1 v ∂ ∂t φ(r, E , Ω, t ) = ∫ ∞

## 0 d E ′ ∫

4π d Ω′Σs(r, E ′ → E , Ω · Ω′)φ(r, E ′, Ω′, t ) (1.1)

+ χ(E ) 4π ∫ ∞

## 0 d E ′ ∫

4π d Ω′v(E ′)Σ f (r, E ′)φ(r, E ′, Ω′, t )

+Qext (r, E , Ω, t )

−Σt (r, E )φ(r, E , Ω, t )

−Ω · ∇φ(r, E , Ω, t )

where φ(r, E , Ω, t ) is the angular neutron ﬂux at location −→r with energy E and direction given by the unit vector −→ Ω at time t; Σt is the total cross section; Σs is the scattering cross section from (−−−→ Ω′, E ′) to ( −−→ Ω, E ); Σ f is the ﬁssion cross section; ν is the average number of neutrons generated per ﬁssion; κ is the ﬁssion spectrum; Qext is the external neutron source.

The NTE involves seven dimensions: three dimensions in space, one dimension in energy, two dimensions in angle and one additional dimension in time for transient analysis. No simple analytical solution is available to the NTE. Numerical methods are developed for solving the original or simpliﬁed NTE and can be categorized into Monte Carlo approaches and deterministic approaches. The Monte Carlo method simulates directly the movement and interaction with the environment for a large number of neutrons with minimal assumptions to obtain the statistics of the parameters. It can be used for detailed analysis and cross-section generation for deterministic codes, as well as provide reference results for code benchmarking. The deterministic approach, on the other hand, applies multiple discretizations in space, direction, energy, and time and solves the NTE on each resulting grid element. In general, deterministic approaches are less ﬂexible in modeling complex geometries, but take less CPU time for the same precision.

In FHRs, the fuel is encased in tiny TRISO particles that are dispersed in fuel pebbles. An FHR core can contain up to half million pebbles. The peculiar geometry of FHR cores poses great challenges to both deterministic and stochastic numerical tools and requires different methods from those used for a typical LWR core due to a unique geometric feature, double heterogeneity: The ﬁrst one is the heterogeneity in the fuel elements in coolant; the second one is the heterogeneity in the TRISO particles within a fuel element.

Monte Carlo codes, such as MCNP [44] and Serpent [29] can model every particle and pebble

CHAPTER 1. INTRODUCTION 6

Figure 1.1: Schematic of fuel pebbles and triso particles

explicitly. And if combined with discrete element methods, the Monte Carlo/ DEM code system can model randomly distributed fuel kernel locations as the way they are manufactured, although this is a computationally expensive process. Studies [12] have shown that models with ordered structure such as simple cubic, body centered cubic, or face centered cubic can give as accurate results as long as the moderator to fuel ratio is carefully preserved when the spherical objects are artiﬁcially cut at the boundaries in the models.

Unlike the Monte Carlo codes, it is more difﬁcult to use existing deterministic codes that were developed for other types of reactors. First, deterministic codes cannot model every TRISO particle explicitly and instead need to use effective homogenized cross-sections generated from other codes, that takes into account of the self-shielding effect, which suppresses the neutron ﬂux inside the fuel region and reduces the resonance neutron absorption.

In addition to the challenge in representing the complex geometry, neutron energy in a reactor core spans almost nine orders of magnitude and needs proper discretization in deterministic models. The multigroup approach is often used to divide the neutron energy into consecutive intervals with uniform cross-sections and transforms the NTE into a ﬁnite set of equations in terms of group-wise neutron ﬂux (deﬁned in equation 1.2). Studies have shown that a fewgroup model can be sufﬁcient to provide reliable results if the energy group boundaries are carefully chosen and the energy-averaged cross-sections are determined properly accounting for the energy self-shielding effect [9].

CHAPTER 1. INTRODUCTION 7

φg (r, Ω) = 1 ∆Eg ∫ g φ(r, Ω, E )d E (1.2)

g = 1, 2, · · · ,G

Furthermore, the angular distribution of neutron ﬂux also needs to be discretized in deterministic calculations. Two widely used discretization schemes for directional dependency of neutron transport are the method of discrete ordinates (SN ) and the method of spherical harmonics (PN ). The SN method divides the total solid angle into a set of solid angles and transforms the NTE into a set of simultaneous partial differential equations. Each equation describes the variation of neutron ﬂux along the direction associated with a discrete solid angle. However, the SN method may suffer from the ray-effect problem that arises from approximating a continuously varying function by a stepwise function by requiring a very ﬁne angular discretization and impractical computation cost. The PN approximation expands the angular ﬂux function into spherical harmonics and truncate to a ﬁnite length to reduce computation cost. The solution approaches the real solution of NTE as N goes to inﬁnity. However, this quickly becomes expensive, especially in three dimensional cases where the number of PN equations is N(N+1)/2. In view of the disadvantages of the two discretization schemes, Gelbard [13] proposed simpliﬁed spherical harmonic (SPN ) method in the early 60s and Larsen [26] established its theoretical basis in the 90s by demonstrating its asymptotic limit to the NTE. In particular, the diffusion equations (SP1) is widely used in nuclear reactor core modeling, and the SP3 formulation has become a popular method for modeling neutron transport with linear anisotropy where theoretical limitations of diffusion equations may exist, e.g. in control rods, burnable poisons, and other discontinuities.

Point reactor kinetics equations can be derived from the one speed diffusion model with the assumption that the spatial dependence of the ﬂux in the reactor can be described by a single spatial mode shape (the fundamental mode) that does not change in time. Despite the limitations in accuracy, the point reactor kinetics equations can be used for preliminary survey design studies for coupled thermal-hydraulics and neutron kinetics behavior or for uncertainty and sensitivity studies where a large number of trials need to be run.

The difference between these models is summarized in table 1.2. A more detailed discussion can be found in Chapter 2.

#### 1.2.2 Thermal-hydraulics methods

With the heat source computed from the neutronics methods discussed in the previous section, the thermal-hydraulic models compute the temperature distribution in the core, that can be used in turn to evaluate the cross-section in the neutronics module, to determine ﬂow pattern

CHAPTER 1. INTRODUCTION 8

Methods Space Direction Energy

Point kinetics Average Average Average Diffusion theory Homogenized Linearly anisotropy Multi-group Deterministic transport Discretized Discretized Multi-group Monte Carlo Continuous Continuous Continuous

Table 1.2: Comparison between reactor physics computational methods

and pressure drop, and to examine whether a component is too hot or too cold with respect to safety limits.

The question of how to model the complex geometry of the ﬂow passage in a pebble bed has been tackled in chemical engineering [8][32], as ﬁxed bed reactors are important equipment in chemical processes, and in nuclear engineering for analyzing advanced pebble bed reactor designs, such as HTGR, PBMR, HTR, etc. The one-dimensional heat conduction simpliﬁcation is often used in LWR fuel rods and can be used in spherical fuel elements for FHRs for quick scoping analysis on average behavior. However, it is inadequate for rigorous investigation because it ignores temperature gradients in the axial and the radial direction of the core, both of which can be signiﬁcant in a FHR core.

Two main approaches for CFD modeling of two-dimensional or three-dimensional pebble bed thermal-hydraulics are realistic modeling, the direct CFD modeling of the ﬂow passage geometry, and porous media modeling. Realistic modeling has been used to model segment of pebble bed, from a few to several hundred pebbles [28][16], but it quickly becomes impractical for a larger number of pebbles because of the large number of degree of freedom that need to be solved. Also, although the goal is to model the exact geometry, some common simpliﬁcations are usually applied in realistic modeling to reduce the computation cost. First, randomly packed pebble bed requires many more grids to assure the mesh quality, so most of the simulations use a lattice arrangement of pebbles, including simple cubic (SC), body-centered cubic (BCC) or face-centered cubic (FCC) arrangements. Second, two perfect spheres would, in reality, contact at one single point. To mesh this in CFD software would need virtually an inﬁnite number of grids or manually adding meshes at each contact point. Various studies on the effect of approximating the point by a small gap [43][28] or by an area contact [43] have concluded that the effect of the approximation on local ﬂow regime and its induced heat transfer is signiﬁcant.

To model an entire FHR core, the porous media approach requires less computational cost by computing only the average behavior of the thermal-hydraulic parameters. The number of mesh elements used in porous media is ten times less than in the realistic modeling [48]. Yet comparison between the direct CFD and porous media methods in [48] suggests that porous

CHAPTER 1. INTRODUCTION 9

media methods provide similar averaged pressure and temperature gradient across the pebble bed to the realistic approach, provided the appropriate Darcy-Forchheimer drag model is used. When appropriate safety margin is added in the design to reﬂect the fact that porous media models do not capture local vertices, ﬂow separation, variations in heat transfer coefﬁcient around the pebble surface and the resulting hot spots, the porous media is adequate for pebble bed core analysis. In fact, the porous media approach has been used in standard HTR modeling tools Thermix [39] and Tinte [15], as well as in thermal-hydraulics models for FHR [40].

In a porous media model, the convective heat transfer between the fuel pebbles and the coolant is computed from empirical correlations. The validation of the correlation in the application range will affect the results and need to be veriﬁed. The Biot number in a representative PBFHR core is approximately 9.4, using the nominal pebble bed heat transfer coefﬁcient in Mk1 PB-FHR (4700 W /m2K ), an estimation of equivalent thermal conductivity for FHR fuel pebbles (15 W/m.K), the diameter of a fuel pebble in Mk1 PB-FHR (0.03 m) and the formula in equation 1.3. Bi = hD k (1.3)

Therefore the radial variation of temperature in the pebble is not negligible comparing to the temperature difference between fuel pebbles and coolant. The temperature proﬁle inside fuel pebbles and TRISO particles is especially important for acute transients such as the reactivity insertion transients, during which the heat does not have time to escape from the fuel structure. While it is not feasible to explicitly model the temperature of each of the millions of TRISO particles in an FHR core, it is indispensable to implement appropriate multi-scale treatment to account for the non-uniform fuel temperature during a reactivity-induced transient.

#### 1.2.3 Coupling between thermal-hydraulics and neutronics

It is convenient to isolate these physics in separate domains for fundamental research, but they are highly inter-correlated in a nuclear reactor core and so should be in the numerical models that are developed for accurately simulating steady state and transient events in a reactor core. In a coupled numerical model, the thermal-hydraulics module takes information about heat generation from neutronics computation; the neutronics module in turn uses temperature distribution to assess cross-sections.

In a nuclear reactor, atomic energy is liberated as kinetic energy of ﬁssion products, neutrons, betas, gammas and neutrinos, either directly at the ﬁssion of U235 or later as neutron-rich ﬁssion fragments go through beta decay. Of the approximately 200 MeV energy per ﬁssion, about 5% is released as the ﬁssion products beta decay. The decay heat is the reason that emergency heat removal is needed to control the core temperature and to prevent melt-down, even after the reactor is shut down. Most of the released energy (except for the energy carried by neutrinos) is transformed to thermal energy as the particles interact with the atoms in the core. Fission fragments and beta particles carry about 85% of the energy and do not travel very far

CHAPTER 1. INTRODUCTION 10

from its original points. This portion of energy is deposited in the vicinity of the ﬁssion site. Neutrons have longer range and can transport the energy to coolant and reﬂectors. Gamma radiation can penetrate up to 100 cm of material in the reactor and distribute energy as they scatters, mostly in the fuel but also in the coolant, reﬂector and even in the shield. Overall, 97% of the recoverable energy is deposited in the fuel, about 2% in the coolant and reﬂector, and less than 1% in the shield due to gamma radiation. It is customary in nuclear reactor analysis to assign all the heat to the fuel region and compute the heat transfer to coolant as heat convection [9].

Globally speaking, the effect of material temperature on their neutronics behavior can be characterized by the temperature reactivity feedback coefﬁcient, which is deﬁned in equation 1.4.

αx = ∂ρ ∂x ∼ ∆ρ ∆x (1.4)

where ∆ρ is the variation in reactivity due to the change in parameter x by ∆x.

Among the various feedback mechanisms in the core, the most important ones for FHRs are the Doppler feedback and the coolant void feedback:

• Doppler feedback: Doppler feedback gets its name from the Doppler broadening of the absorption cross sections, and the most signiﬁcant change following an increase in fuel temperature is the large increase in neutron capture probabilities in 238U , which results in a negative effect on reactivity. This effect is the most rapid and direct response to temperature increase because it is directly related to nuclear reactions in the fuel. It is highly desirable to have a negative Doppler reactivity feedback for power excursion accident.

• Coolant void feedback: The coolant void feedback is due to the change in the coolant density during temperature variation. Raising the temperature would cause the coolant to expand, providing less scattering for neutrons to transfer their kinetic energy and slow down and hardening the overall spectrum in the core, resulting in more leakage near the periphery. A competing mechanism exists in the FHR coolant, which is the neutron capture by Li6 having a large cross-section. This negative effect on reactivity would also decreases as the temperature decreases. Overall, a positive feedback could stimulate the reactions when the temperature rises and could pose safety issue if not appropriately addressed in the design. That is the reason why natural lithium needs to be enriched before it can be used as FHR coolant, to limit the effect caused by Li6.

Previously, researchers at UCB have developed separated codes in their domain of research, namely neutronics [7] and thermal-hydraulics [40]. These codes can then be coupled using the operator-split method: solving one equation system at a time and transfer data between the

CHAPTER 1. INTRODUCTION 11

codes after each iteration until the results converge. However, operator split method suffers from ’nonlinear inconsistency’ [35], resulting in a loss of accuracy and stability [31]. Small time steps are required to ensure convergence. Fully coupled codes, where all the equations are solved simultaneously, have been developed to effectively model the multi-physcis phenomena in a FHR core.

#### 1.2.4 Numerical tools

The 2004 OECD report [33] reviews a list of separate and coupled codes for neutronics and thermal-hydraulics modeling for LWRs. However, the deterministic codes that are tailored for LWRs can not be used directly for FHRs due to the complex and different geometry. The following tools are used to build numerical models in this work for FHR cores:

• Python is a high-level programming language that is widely used in the scientiﬁc research community. In addition to the standard libraries, Python has many open source libraries that are useful for scientiﬁc computing, namely Numpy/Scipy for numerical operations, Cython for low-level optimization, IPython for interactive work, and MatPlotLib for plotting. Readability is another key advantages of Python, as Guido van Rossum, Python’s original author, writes [38]: ’This emphasis on readability is no accident. As an objectoriented language, Python aims to encourage the creation of reusable code. Even if we all wrote perfect documentation all of the time, code can hardly be considered reusable if it’s not readable. Many of Python’s features, in addition to its use of indentation, conspire to make Python code highly readable.’

• The COMSOL Multiphysics is a software package that solves systems of PDEs or ODEs using the Finite Element Method (FEM). COMSOL allows user to either use pre-deﬁned multiphysics modules or specify a system of PDEs. The LiveLink for MATLAB module in COMSOL enables the user to deﬁne COMSOL models through a MATLAB script, which further extends its versatility. The use of MATLAB script also improves readability and repeatability of the code; with extensive documentation, the code is self-explanatory and the procedures are repeatable. In this work, for example, neutron diffusion equations are implemented into COMSOL through the MATLAB LiveLink, while the build-in heat transfer modules are used for thermal-hydraulics calculation. Compared to the users of nuclear speciﬁc codes, the users of COMSOL and MATLAB beneﬁt from a vast online information base shared by a large cohort of fellow users of the general purpose modeling software and dedicated technical support as an exchange for license fee.

• Serpent [29] was developed at VTT Technical Research Centre of Finland, Ltd. for 3D continuous-energy Monte Carlo calculation. Its use of Woodcock delta-tracking and unionized energy grid allows the code to perform reactor physics computation with improved speed compared to legacy codes. The code is capable of calculating homogenized multi-group cross-sections and kinetic parameters that can be used in determin-

CHAPTER 1. INTRODUCTION 12

istic methods as inputs. In fact, Serpent generated group constants have been used in various diffusion codes, including PARCS and DYN3D. The Serpent website provides all relevant information including libraries, user guides, release logs, etc. Writing input ﬁles for Serpent could be tedious and error prong, therefore a Python package was developed in this project for generating input ﬁles for PB-FHRs automatically.

### 1.3 Research scope and thesis outline

The objective of this thesis is to develop, verify, and apply multi-physcis models for FHRs to provide accurate solutions in a reasonable amount of CPU time for day-by-day whole-core calculations on stand-alone workstations or small-scale parallel machines with less than a hundred cores. This corresponds to the computational resource that a typical nuclear engineering company possess. Researchers who have access to supercomputers also would appreciate the possibility of quickly testing their ideas without submitting the job to the queuing system and wait a long time for the results.

Following the introduction, Chapter 2 discusses methodologies for numerical modeling of FHR cores. Models with various levels of spatial resolution and numerical precision are explored: from zero dimensional point kinetics model to three dimensional full core model, from lumped capacitance to three dimensional CFD model. Both mathematical formulation and implementation of these different approaches are presented.

The application and credibility of the coupled neutronics and thermal-hydraulics model is demonstrated using the TMSR SF-1 design in Chapters 3. Extensive model veriﬁcation is carried out by comparing the results from the coupled models to higher resolution models or analytical solutions in various scenarios. With the models, TMSR SF-1 core performance under steady state and transient scenarios are examined.

Numerical models are built for the Mark 1 Pebble-Bed FHR (Mk1 PB-FHR) [1] design in chapter 4 and are used to investigate the effect of material selection on core neutronics, to optimize ﬂow ﬁeld across the core, as well as to understand transient behaviour of the Mk1 core during reactivity insertion and overcooling scenarios.

Finally, Chapter 5 summarizes the conclusion and lays out a plan for future work. 13

Chapter 2

Multiphysics modeling methodology

Nuclear reactor licensing requires demonstration of safety under licensing basis events (LBEs), design basis events (DBEs) and beyond design basis events (BDBE)[2] with respect to evaluation criteria, such as peak and average temperatures for components inside the reactor core, coolant outlet temperature and the directly related hot leg peak temperature, and, for FHR in particular, minimum coolant temperature to ensure adequate safety margin to freeze. Therefore, the numerical tools for understanding the core behaviours and to support decision making in reactor core design should be able to predict these parameters with reliable accuracy.

This chapter presents numerical modeling methodology for FHR core analysis. Tools with various spatial resolution and numerical precision are explored in this chapter, and their advantages and disadvantages are compared in chapter 3 and 4 through case studies based on the TMSR SF-1 and the Mk1 PB-FHR design. The Monte Carlo neutron transport method is described in section 2.1, including the development of a computer program that automates the creation of input ﬁles for Monte Carlo codes. This is useful for both stand-alone neutronics studies and parameter generation for deterministic codes, a very important use of Monte Carlo codes in reactor physics. For coupled neutronics and thermal-hydraulics modeling, this work investigates both unit cell models and full core models, with the aim to support a broad spectrum of simulation needs.

### 2.1 Monte Carlo neutronics modeling

Monte Carlo models are used in this work to perform detailed analysis on neutronics behaviours of the core, to provide reference results for code veriﬁcation, as well as to generate neutronics parameters for deterministic codes.

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 14

#### 2.1.1 Serpent input generator

The stochastic nature of the Monte Carlo neutron transport method provides great ﬂexibility in modeling complex geometries. However, writing and input ﬁles for Monte Carlo code that deﬁnes complex reactor cores and modifying them for a change in design speciﬁcation often involves changes in multiple parameters across different ﬁles. For example, a change in coolant temperature requires changes in material density, cross-section library, etc. This process can be time-consuming and error-prone, therefore a Python package the FHR Monte Carlo modeling Input Generator (FIG) [47] is developed to automatically generate Serpent readable ﬁles with minimum user inputs.

Serpent has a ’universe-based geometry model’ [29] that enables the description of complicated structures in a modular way. Thanks to this concept, users only need to deﬁne a small number of TRISO particles and pebbles in a representative unit cell and repeat that in three dimensional fuel region to simulate a regular packed bed, even though a FHR core may contain thousands of pebbles and up to millions of TRISO particles; users can also deﬁne individual components in their local coordinates and assemble them in the end by assigning them into the same universe. This has inspired the design of FIG, an open source Python package that allows users to deﬁne FHR reactor cores in an intuitive way and automatically generates the corresponding Serpent input ﬁles for Monte Carlo transport analyses. Since the package contains documentation and examples, only a brief overview of the code’s functionality, structure, and usage is provided here. As an example, the script for creating an inﬁnite Face Centered Cubic lattice of fresh fuel pebbles is attached in appendix A, followed by the generated serpent script. More examples, including the full core models used in chapter 4 can be found on github [47].

The functionalities that the FIG package aims to provide include:

• The package assigns and manages the cell and universe numbers, making sure that they are unique and retrievable.

• When a design speciﬁcation is changed, the code can modify all relevant parameters in the serpent input scipts to reﬂect the design change. For example, knowing the temperature, it can ﬁnd the right material library, deﬁne the temperature card and calculate the corresponding density for liquid materials, so that when changing the temperature of a material, the users only need to change the temperature deﬁnition of the material object, instead of having to change multiple parameters over different ﬁles, which is easy to miss and difﬁcult to notice because this would not trigger any serpent runtime errors.

• The code deﬁnes a default core conﬁguration based on the Mk1 reference design, allowing user to create a model quickly by only modifying the parameters that are changed from the baseline design. This is particularly useful for perturbation calculations, design

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 15

optimization and uncertainty studies, where only a small number of the parameters are changed every time.

• The input that FIG requires to create a model is much shorter and more interpreteable than the one for Serpent. This facilitates collaboration and model veriﬁcation across different teams.

Figure 2.1: Illustration of the association between the important objects in FIG, such as core components, generators and serpent concepts like universes and cells

The overall structure of the code is illustrated in ﬁgure 2.1. Every component in the core, from a tiny TRISO particle to a reﬂector, is deﬁned as a component (Comp) object and is associated with a generator (Gen) object that automatically generates the corresponding text in the serpent input scripts. A factory function creates surfaces, cells and universes that are associated to the component for the generator and manages their numbering and retrieval.

There are two types of components in a Serpent model. A ’simple’ component is deﬁned from their geometry and material. To simplify the deﬁnition of the geometries while reserving the ﬂexibility for customization, a component can be deﬁned either as a combination of components of primitive shapes such as cylinder, cones, truncated cones, etc or by its boundary surfaces. Figure 2.2 shows the attributes and methods associated with the material class (Mat)

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 16

Figure 2.2: Illustration of the material class design in FIG

and, as an example, ’Flibe’, one of its child classes. A material object is deﬁned by the density, temperature, name, composition as well as optional characteristics such as ’tmp’ for doppler broadening, ’moder’ for thermal scattering treatment, ’rgb’ for plotting color, etc. The material composition can be deﬁned either via an isotope list with the corresponding fractions or from an input ﬁle, which convenient to import results from other codes, e.g. depletion results.

On the other hand, the ’core’, the largest composite component in a reactor core, is deﬁned as an ensemble of other components as shown in ﬁgure 2.3. Among the components, vessel, downcomer, core barrel and reﬂectors are simple components that can be deﬁned directly by their geometry and material. The fuel and blanket regions are composite components that are ﬁlled by pebbles and coolant. These are the regions where the beneﬁt of the universe-based concept become most obvious. Taking the fuel region as an example, the different levels of universes are illustrated in ﬁgure 2.4. A TRISO particle object is deﬁned by a list of the constituent layers and are repeated in a lattice, for which the pitch is computed from the packing fraction of TRISO particles in graphite matrix. This lattice is then integrated into fuel pebbles, which in

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 17

turn form an unit cell together with coolant that ﬁlls the fuel region of the core.

Figure 2.3: Constituting components of the core

Figure 2.4: Illustration of levels of universes in a fuel region deﬁnition

### 2.2 Coupled neutronics and thermal-hydraulics models

Numerical modeling of a physical system is, in a layman’s term, to represent the laws of physics as mathematical abstractions, often in forms of partial differential equations (PDEs). A well posed mathematical problem typically include the following four groups of equations.

In section 2.2.1 and 2.2.2, the governing equations in the four categories are presented for each method:

• Conservation equations: Mass, energy, momentum and neutron ﬂux are all conserved natural quantities that can be characterized by conservation equations.

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 18

• Constitutive equations: The constitutive equations, which characterize how the materials respond to domain forces and boundary ﬂuxes, are often measured empirically and are expressed in the numerical models as correlations.

• Boundary conditions: There are three types of boundary conditions: Dirichlet boundary conditions specify the value of the function on a boundary; Neumann boundary condition specify the normal derivative of the function on a boundary; Robin boundary condition is a mix of the former two, specify a linear sum of the function and its normal derivative on a boundary. Boundary conditions are used to restrain the possible solutions to the PDE system and therefore the choice of boundary conditions has important implications to the results.

• Initial conditions: Initial conditions are used to specify the value of a function at a given time (usually at t=0) when solving for time dependent problems.

#### 2.2.1 Unit cell multi-physics model

Point kinetics equations can compute time dependent ﬂuctuation of core average neutron population and the closely linked power, and, when coupled with heat transfer model, temperatures for reactor safety analysis. The following sections describe the governing equations, including modiﬁcations to the standard point kinetics equations to account for the reﬂector effects on neutron life time in FHRs, as well as the implementation of the coupled unit cell model in an open source Python package called Python package for nuclear Reactor Kinetics (PyRK) (section 2.2.1.3).

##### 2.2.1.1 Neutronics

The (standard) point kinetics equations can compute the time dependent reactor power P(t) and the delayed neutron precursors Ci(t) concentration:

d P (t ) d t = ρ(t ) − β Λ P (t ) + D∑

i =1 λi Ci (t )

dCi (t ) d t = βi Λ P (t ) − λi Ci (t ) (2.1)

where ρ is reactivity, and β is the effective delayed neutron fraction, which is the sum of all βi , the effective delayed neutron precursor yield of group i. Λ is the prompt neutron generation time, and λi is the decay constant for the i-th delayed neutron precursor group. D is the number of delayed neutron groups (six is often used).

FHRs have substantial graphite reﬂectors that increase keff by improving neutron economy through reﬂection and slow down the effective prompt neutron lifetime by hosting neutrons during moderation. Neutrons from the core are transported to the reﬂectors, scatter within the

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 19

Conﬁguration keff Effective prompt neutron lifetime (µs) Fuel and both reﬂectors 1.03 459 Fuel and outer reﬂector 0.95 384 Fuel and inner reﬂector 0.88 399 Fuel only 0.73 227 Table 2.1: Effect of reﬂectors on keff and prompt neutron lifetime in Mk1 PB-FHR

reﬂectors for some time, and come back to the fuel region to initiate a ﬁssion reaction. As in an example shown in Table 2.1, keff and prompt neutron lifetime changes dramatically in the Mk1 FHR core with different reﬂector conﬁgurations. The effects of reﬂectors must be considered in the neutron kinetics model.

The reﬂector-induced effects can be characterized by the reactivity gain due to the reﬂector and the sum of neutron lifetime in the fuel region and in the reﬂector. The slow neutrons coming back from the reﬂectors into the fuel region can be modeled as additional delayed neutron groups from ﬁctitious neutron emitters. In this sense, a change in conﬁguration of the reﬂectors alters the dynamics of the chain reaction and can be modeled by adding a group of delayed neutrons [25]. To account for these effects, a ’multipoint’ kinetics model is formulated for FHR cores with additional ﬁctitious delayed neutron groups as follows:

d P (t ) d t = ρ(t ) − β − ρR Λc P (t ) + 6∑

i =1 λi Ci (t ) + λRCR (t )

dCi (t ) d t = βi Λ P (t ) − λi Ci (t )

dCR (t ) d t = ρR Λc P (t ) − λRCR (t ) (2.2)

In the above equation system,

ρ = reactivity β = the effective delayed neutron fraction, which is the sum of all βi , the effective delayed neutron precursor yield of group i. Λ = the prompt neutron generation time λi =s the decay constant for the i-th delayed neutron precursor group. D = the number of delayed neutron groups, where six is often used. Λc = the prompt neutron generation time in the core, without any reﬂectors. λR = 1/ΛR

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 20

ΛR = the sum of neutron lifetime in the reﬂector and neutron lifetime in the core after coming back from the reﬂector(s). This value can be calculated from Λpr t , the mean prompt neutron lifetime in the reactor including inner and outer reﬂectors, and Λc , the prompt neutron generation time in the core without reﬂector, by the following relations:

Λpr t = (1 − ρR )Λc + ρR ΛR (2.3)

where ρR is the reactivity gain by the reﬂector comparing to the core without the reﬂector:

ρR = ke f f − k c e f f ke f f (2.4)

The reactivity is calculated as the sum of external reactivity insertion and temperature reactivity feedback as in equation 2.5.

ρ(t ) = ρext (t ) + αF (TF (t ) − TF,0) + αM (TM (t ) − TM ,0) + αc (TC (t ) − Tc,0) (2.5)

##### 2.2.1.2 Heat transfer

According to the reasoning in section 1.2.3, we assume all the energy released in nuclear reactions is deposited in the fuel as thermal energy and transferred to other components via heat transfer. Due to the large ratio between core and pebble diameter, the heat generation inside a fuel pebble is assumed uniform and heat conduction at the scale of a single pebble can be assumed isotropic and can be modeled with the 1-D heat diffusion equation in spherical coordinates, written as [21]: ρCp ∂T ∂t = 1 r 2 ∂ ∂r ( kr 2 ∂T ∂r ) + g (2.6)

where r is the radial coordinate of the point, ρ is density, Cp is speciﬁc heat capacity, T is temperature, t is time, k is thermal conductivity, and g is the heat generation density.

The temperature gradient at the center is zero due to symmetry:

∂T ∂r |r =0= 0 (2.7)

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 21

A mixed boundary condition at r=R is used to compute the heat loss to coolant, which can be modeled with single-phase convection, as ﬂibe remains in liquid phase between 458 and 1400◦C . ∂T ∂r |r =R = h k (T − T0) (2.8)

k is the thermal conductivity of fuel, T0 is the coolant temperature, and h is the heat transfer coefﬁcient that characterizes the heat transfer between fuel pebble surface and coolant and is usually determined from empirical correlations.

The Wakao correlation [45] is ﬁtted from a collection of experimental data that are carefully examined and corrected for the axial ﬂuid thermal dispersion coefﬁcients. Equation 2.10 to 2.13 shows how to calculate the heat transfer coefﬁcient from dimensionless numbers. Huddar [18] has experimentally measured Nusselt number in packing pebble bed and has concluded that the widely used Wakao correlation predicts heat transfer in FHR core ’fairly well’. However, it has not been fully examined in all the conditions that an FHR operates. Its effect on the output parameters should be studied in uncertainty and sensitivity analysis.

Nu = 2 + 1.1P r 1/3Re0.6 (2.9)

Re = ρdp u

µ (2.10)

P r = cp µ

k (2.11)

h = Nu.k dp (2.12)

u = ˙m Aρ (2.13)

where dp is the pebble diameter and u is the superﬁal velocity, ˙m is the mass ﬂow rate, ρ is the density and A is the cross sectional area.

The right side of the heat diffusion equation can be transformed to a second order differentiation of r via a change of variable: U(r, t) = rT(r, t).

∂U ∂t = α ∂ 2U ∂r 2 + r g ρCp (2.14)

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 22

and the boundary conditions in terms of the new variable become:

U0 = 0 (2.15)

∂U ∂r |r =R = ( 1 R − h k )U + R k hT∞ (2.16)

This new equation system is readily solved numerically under ﬁnite volume discretization.

Coolant inlet temperature is imposed on the unit cell as a boundary condition and the bulk coolant temperature is calculated as an average between the inlet and outlet temperatures. The energy balance for the coolant is written as:

ρCpV d Tc d t = − ˙mCp (Tout − Ti n) + h A(Ts − Tc ) (2.17)

where ρ is density, Cp is speciﬁc heat capacity, V is volume of the unit cell, ˙m is the mass ﬂow rate, Tout is the outlet temperature, Ti n is the inlet temperature, h is the convective heat transfer coefﬁcient, A is the heat transfer surface area, Ts is the pebble surface temperature and Tc is the bulk coolant temperature.

##### 2.2.1.3 Implementation of the unit cell model

As shown in ﬁgure 2.5, the unit cell model represents an average fuel pebble and a portion of ﬂibe coolant that amounts to the packing fraction. To simplify the computation, the following assumptions are made:

• Due to the large ratio between the reactor diameter and the length of the unit cell, temperature difference between adjacent unit cells is negligible. Therefore, a conservative adiabatic condition is imposed on the lateral walls.

• The pebble is modeled by homogeneous layers: graphite shell, fuel and moderator kernel (if exist in the design). But the heterogeneity from TRISO particles is implicitly taken into account when computing equivalent material properties and neutronics parameters that are used as inputs in the unit cell model.

• The center and outer reﬂectors are not modeled explicitly in the unit cell, however their effect on the reactivity and on the average neutron lifetime in the core is included in the multi-point kinetics equations as discussed in the previous section.

• The inlet coolant temperature is dictated by boundary conditions that is either constant or as a function of time to simulate coolant related transients. Average between inlet

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 23

coolant tempearture and outlet temperature at the top of the unit cell is used to calculate the coolant contribution in reactivity feedback.

• The coolant temperature rise in the unit cell is only a small portion of the real core. It is scaled up by the ratio of length between the core and the unit cell to reﬂect the temperature rise in a full core. This assumption relies on symmetry of the axial heat ﬂux that coolant gets from fuel pebbles and need to be veriﬁed against more elaborated models.

• The Doppler coefﬁcient for fuel temperature reactivity feedback varies as the fuel composition changes over the course of an online refueling campaign. It should be calculated from Monte Carlo models with the appropriate fuel composition for the speciﬁc scenarios that one tries to model. However, the change in fuel composition is negligible during the time of the transient scenarios we simulate with the unit cell model, thus the parameters, once computed from Monte Carlo models can be used throughout the transients.

• In the range that we consider, changes in temperature in one component would not change the neutron spectrum and thus will only change the reactivity proportionally. In the current model, the reactivity feedback of different components are therefore computed separately with constant coefﬁcients.

Figure 2.5: Illustration of the unit cell model geometry (based on Mk1 design)

The model is implemented in the PyRK package, which has neutronics and thermal-hydraulics sub-modules that generate an ordinary differential equation (ODE) system from user deﬁned

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 24

reactor design characteristics (inlet temperature, material properties, etc.) and simulation speciﬁcations (starting time, ﬁnishing time, time steps for output logging, etc.).

The ODE system resulted from physics with disparate time scales is so stiff that explicit solvers are generally not suitable because their region of absolute stability is small and would require very small time steps to ensure convergence. The differential equation system is solved in PyRK by an implicit Runge-Kutta solver of order (4)5 with adaptive time step size, provided by the Python scientiﬁc computing library Scipy [23]. Visualization capability is also provided by open source Python packages, Matplotlib [19] and Bokeh [5]. Output data is stored in hdf5 database using PyTables [3].

#### 2.2.2 Full core multi-physics model

As discussed in the introduction, neutron diffusion equations is capable of simulating the three dimensional behaviour of neutron transport without exorbitant computation burden and is a good candidate to model FHR cores. This section presents the full core modelling methodology that couples neutron diffusion and porous media CFD, including generic governing equations and special treatment for FHRs, such as SPN treatment for control rods modeling and multiscale temperature treatment to compute more accurate cross-sections that depends on detailed temperature distribution inside fuel elements.

##### 2.2.2.1 Multi-group neutron diffusion

Equations 2.19 is the multi-group diffusion equation, written in terms of group-averaged neutron ﬂux, deﬁned as the neutron ﬂux in the gth energy interval [Eg , Eg −1]:

φg (r, t ) = ∫ Eg −1

Eg φ(r, E , t )d E (2.18)

1 vg ∂φg ∂t = ∇D g ∇φg − Σt ,g φg − ∑

g ′̸=g Σs,g g ′φg + (1 − β)χp,g G∑

g ′=1(νΣ f )g ′φg ′ + D∑

i =1 χd ,g λi ci (2.19)

Although only constituting 0.6% of the neutron population, delayed neutrons from beta decay of the ﬁssion products play an important role in slowing down the neutronic time scale and should be included in transient analysis. The delayed neutron precursor concentration equation is shown in equation 2.20. ∂Ci ∂t = −λi Ci + βi G∑

g =1(νΣ f )g φg (2.20)

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 25

The quantities in equation 2.19 and equation 2.20 are deﬁned as follows: vg = neutron speed of the g -t h group, m/s Σs,g g ′ = macroscopic scattering cross-section from group g ′ to group g , m−1

ν = mean number of neutrons generated per ﬁssion Σ f ,g = macroscopic ﬁssion cross-section of group g , m−1

χg = fraction of delayed (d ) or prompt (p) neutrons from ﬁssion generated in group g φg = neutron ﬂux in the g -t h energy interval [Eg , Eg −1], m −2s −1

Σt ,g = macroscopic total cross-section in group g , m −1

D g = diffusion coefﬁcient for energy group g , m βi = delayed neutron fraction for delayed neutron precursor group i λi = average decay constant of delayed neutron precursors, s−1

Ci = delayed neutron precursors concentration G = total number of energy groups D = total number of delayed neutron precursors groups.

For criticality calculation, the effective multiplication factor keff is inserted into the neutron diffusion equation to represent the evolution of neutron population from one generation to the next. The multi-group neutron diffusion equation for eigenvalue computation is written as below:

0 = ∇D g ∇φg − Σa,g φg − ∑

g ′̸=g Σs,g g ′φg + ∑

g ′̸=g Σs,g ′g φg ′ + 1 ke f f (1 − β)χt ,g G∑

g ′=1(νΣ f )g ′φg ′ (2.21)

##### 2.2.2.2 SPN neutron transport modeling

The Fick’s law assumption in diffusion models does not take directional variations in neutron transport into account, and this assumption is violated in strong absorbers such as control rods. This section describes a control rods modeling method based on the SP3 approximation of NTE that is more accurate than the diffusion approximation, especially in control rod regions, and yet is considerably less expensive than the discrete ordinates (SN ) or spherical harmonics (PN ) approaches. In addition, the SP3 equations can be solved in the same manner as the diffusion equations, which allows similar implementations with the same tools.

The multi-group SP3 equation with delayed neutrons are written as the following:

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 26

− ∇D1g ∇(φ0g + 2φ2g ) + Σr g φ0g = χg ke f f G∑

g ′ νg ′Σ f g ′φ0g ′ + G∑

g ′̸=g Σs,g ′g ,0φ0g ′

− ∇D2g ∇φ2g + Σt φ2g = 2/5(Σr φ0 − χg ke f f G∑

g ′ νg ′Σ f g ′φ0g ′ − G∑

g ′̸=g Σs,g ′g ,0φ0g ′)

∂Ci ∂t = −λi Ci + βi G∑

g =1(νΣ f )g φg (2.22)

where the quantities are deﬁned as below:

D1g = diffusion coefﬁcient for group g = 1 3(Σt g −Σs0g ) D2g = 9 35(Σt g −Σs3g ) Σr g = removal cross-section = Σt g − ∑G g ′=g Σs,g ′g vg = neutron speed of the g -t h group, m/s Σs,g g ′ = macroscopic scattering cross-section from group g ′ to group g , m−1

ν = mean number of neutrons generated per ﬁssion Σ f ,g = macroscopic ﬁssion cross-section of group g , m−1

χg = fraction of delayed (d ) or prompt (p) neutrons from ﬁssion generated in group g φg = neutron ﬂux in the g -t h energy interval [Eg , Eg −1], m −2s −1

Σt ,g = macroscopic total cross-section in group g , m −1

D g = diffusion coefﬁcient for energy group g , m βi = delayed neutron fraction for delayed neutron precursor group i λi = average decay constant of delayed neutron precursors, s−1

Ci = delayed neutron precursors concentration G = total number of energy groups D = total number of delayed neutron precursors groups.

##### 2.2.2.3 Parameter generation from Monte Carlo code

Both the diffusion and SPn algorithms requires neutornics parameters as input. Monte Carlo codes can generate cell-homogenized multigroup neutronics parameters for deterministic models by tallying reaction rates and ﬂux in spatial and energy bins, as shown in equation 2.23.

Σx,g = ∫ cel l dV ∫ Eg −1 Eg d E Σx(E )Φ(E )d E ∫ cel l dV ∫ Eg Eg −1 d E Φ(E )d E (2.23)

where Σx,g is a homogenized macroscopic cross-section in energy group g with an energy between Eg and Eg −1, φ is the neutron ﬂux, E is energy, and V is volume.

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 27

In order to perform transient analysis, these parameters need to be paramatrized as functions of reactor operating conditions. The most important characteristics for coupled transient models are fuel temperature and coolant density. The form of the functions depends on the mechanism in which a macroscopic cross-section is affected through either of the two factors in equation 2.24: the microscopic cross-section σx and the atomic density of the material ρn.

Σx = σxρn (2.24)

Comparing to the changes in microscopic cross-sections of the isotopes in ﬂibe, the density of liquid ﬂibe changes signiﬁcantly with temperature and results in linear changes in the macroscopic cross-sections as formulated in equation 2.25.

ˆΣ(ρ f l i be ) = c0 + c1(ρ f l i be − ρ0) (2.25)

where ˆΣ(ρ f l i be ) is the estimation of a ﬂibe macroscopic cross-section, ρ f l i be is the ﬂibe density, ρ0 is the nominal ﬂibe density, and c0 and c1 are the constants to be calculated.

Unlike the liquid salt coolant, the solid fuel do not change shape or density in the range of the considered temperatures, and the fuel temperature reactivity feedback (also called the Doppler feedback) is mainly due to the Doppler broadening of the microscopic cross-sections of the fuel material. Therefore, the fuel cross-sections are described by a linear-log function of the temperature (equation 2.26).

ˆΣ(T f uel ) = c0 + c1(l og (T f uel ) − l og (T0)) (2.26)

where ˆΣ(T f uel ) is the estimation of a fuel macroscopic cross-section, T f uel is the fuel temperature, or temperature vector in case many temperatures affect the homogenized cross-section in a fuel region, and c0, c1 are the coefﬁcients we search for.

Because of the online refueling fuel management strategy, FHRs contain pebbles with different levels of burnups. The fuel pebble thermal conductivity degrades signiﬁcantly with burnup due to radiation damage to graphite based materials, as well as their ability in generating power. Thus, older pebbles experience less power excursion but they also conduct the heat to the coolant less efﬁciently. It is important to compute their temperature proﬁle and their contribution to reactivity feedback separately because they respond differently to transients. Instead of using one average fuel temperature to compute the cross-sections, we can achieve

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 28

greater accuracy by generating cross-section functions that are dependent on the temperature of pebbles at different burnups.

Also important is to compute the temperature distribution inside TRISO particles and fuel pebbles, and use the temperatures at different depth to compute cross-sections. During fast transients such as reactivity induced accidents, the heat does not have time to reach the coolant. The core relies on the temperature reactivity feedback from the fuel inside the TRISO particles to stabilize.

We obtain these functions via processes called computer experiments, in which we change the operating conditions in a Monte Carlo model and compute the coefﬁcients from the resulting cross-sections as functions of the conditions. The relationships are ﬁtted as linear or linear-log functions from the output data, obtained by running the code multiple times with different input data sets. For cross-sections that are only dependent of one variable, we can simply perturb the condition up and down by 10% and compute the linear slope of the output values. However, more elaborated sampling methods, such as Monte Carlo sampling or Latin hypercube sampling, are needed for efﬁciently generating a representative database for cross-sections that covers the range of all the parameters.

##### 2.2.2.4 Flow and heat transfer in porous media

As discussed in section 1.2.2, direct CFD can be used for modeling a segment of the core at microscopic level for fundamental understanding of the local ﬂow phenomena in the pores formed between pebbles and veriﬁcation of lower resolution codes, but is not practical to model the ﬂow passage formed by more than 10000 pebbles in a FHR core, at least not with current computation capacities. Porous media model, on the other hand, requires much less CPU and memory resource by only computing equivalent average solution and is thus used here to compute the coolant ﬂow pattern and heat transfer characteristics.

A pebble-bed reactor core is a porous medium that is composed of equal-sized spherical solid elements and liquid ﬂibe salt. An important parameter for porous media is the porosity, ϵ, deﬁned as the ratio between the volume of the void to the total volume (equation 2.27). It appears in the mass (equation 2.28), momentum (equation 2.30) and energy conservation equations (equations 2.36 and 2.37). ϵ = V f V f + Vs (2.27)

Local porosity varies in a pebble bed and drops near the wall due to ordered packing phenomenon. This variation is not modeled explicitly in the porous media model, but the empirical correlations for pressure loss and heat transfer coefﬁcients include corrections for wall effect.

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 29

The mass conservation equation for ﬂuid in porous media is written as:

ϵ ∂ρ f ∂t + ∇(ρ f v) = Q (2.28)

The quantities in the above equation are: ρ f = the ﬂuid density, kg .m−3

ϵ = porosity t = time, s Q = mass generation or mass sink, kg /(m3.s) v = velocity vector, m.s−1

The porous media momentum equation takes different forms. Equation 2.30 shows the formulation of porous media momentum equation in COMSOL in terms of the superﬁcial velocity u, the velocity that the ﬂuid would ﬂow if the channel is free of the solid medium. It is deﬁned as the ratio between the volumetric ﬂow rate and the effective cross section area of the ﬂow channel (equation 2.29). u = Q A (2.29)

ρ f ϵ [ ∂u ∂t + (u∇) u ϵ ] = ∇ [ −p I + ( µ ϵ (∇u − (∇u) T ) − 2µ 3ϵ ∇uI ) ] − ( µ K + βF ∥u∥ + ρ∇u)u + F (2.30)

Other parameters used in equation 2.30 are: ρ f = ﬂuid density, kg/m3 ϵ = porosity t = time, s p = pressure, Pa µ = dynamic viscosity, Pa.s F = body force, N /m3

I = identity matrix K = permeability, m2

βF = Forcheimer drag coefﬁcient, kg /m4

Various pebble bed pressure drop correlations were investigated in previous studies [40] and compared with experimental data [24]. The most widely used is the Ergun correlation [10] for its wide range of validity in laminar, turbulent and transitional regions, its interpretability and convenient implementation, as well as the satisfactory results it provides. It models the pressure loss in a pebble bed as a weighted sum of the viscous energy loss and the inertial energy loss:

d p d x = E1 (1 − ϵ)2µu ϵ2d 2 + E2 1 − ϵ ϵ3 ρu2

d (2.31)

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 30

The Forcheimer drag coefﬁcient βF and the permeability K are deﬁned in the COMSOL momentum equation according to pebble bed pressure drop characteristics. They are computed as in equation 2.33 and 2.35 so that the pressure drop equation in COMSOL (equation 2.32) matches the Ergun correlation (equation 2.31).

d p d x = µ K u + βF u2 (2.32)

βF = cF ρ p K (2.33)

where cF is the non-dimensional form of the Forcheimer drag coefﬁcient that can be computed from Ergun correlation coefﬁcients E1 and E2 as:

cF = E2 ϵ1.5p E1 (2.34)

K = 1 E1 ϵ 3d 2

(1 − ϵ)2 (2.35)

The energy equations for ﬂuid and solid phases are given in equations 2.36 and 2.37.

ϵ(ρcp ) f ∂T f ∂t + (ρcp ) f U ∇T f = ϵk f ∇∇T f + Φ + hs f a(Ts − T f ) (2.36)

(1 − ϵ)(ρcp )s ∂Ts ∂t = (1 − ϵ)ks∇∇Ts + (1 − ϵ)q + hs f a(T f − Ts) (2.37)

The parameters in these energy equations are: ρcp = volumetric heat capacity, J m3K −1

T f = ﬂuid temperature, K Ts = solid temperature, K k f = ﬂuid thermal conductivity, W m−1K −1

a = speciﬁc surface area, m−1

q = heat generation, W /m3. Φ = viscous dissipation term

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 31

As discussed in section 2.2.1, nuclear power is assumed generated inside the solid fraction of the fuel region and transferred to the coolant through heat convection, reassigning gamma and neutron heating in the ﬂuid to the fuel zone. And the same Wakao correlation (equation 2.10) that is used in the unit cell model is also used here to compute local heat transfer coefﬁcient in the full core model.

Inlet coolant temperatures are imposed at inlet boundaries. This enables modeling of overcooling transient by setting the inlet coolant temperature as a time dependent function. Reﬂectors have separate coolant channels to maintain their temperature. Temperature distribution in outer layers such as graphite reﬂectors and stainless steel containment does not affect their neutronics cross-sections and therefore is not computed in the coupled simulation. However, it can be computed taking the coupled simulation results as inputs. Conservative adiabatic boundary condition is applied at the reﬂector walls.

Either velocity or ﬂow rate is speciﬁed at inlet boundaries. The nominal mass ﬂow rate is computed to achieve a reasonable temperature rise through the core under operating power. Flow straighteners perpendicular to the ﬂow area are designed to make sure that the inlet ﬂow is fully developed and as uniform as possible. Injection channel and oriﬁce dimensions can be adjusted for a designed distribution in inlet velocity proﬁle.

˙m = Q cp ∆T (2.38)

Pressure boundary condition is set to the outlet surfaces. Slip conditions are applied to the walls that contain the ﬂuid in porous media models.

##### 2.2.2.5 Multi-scale temperature distribution

In order to determine the temperature distribution inside fuel pebbles and TRISO particles, which are crucial for accurately determining temperature feedback effect from the fuel material during fast transient analysis, we use the 1-D heat diffusion equation in spherical coordinate to model the heat conduction inside solid fuel elements:

ρCp ∂T ∂t = 1 r 2 ∂ ∂r ( kr 2 ∂T ∂r ) + g (2.39)

where r is the radial coordinate of the point, ρ is density, Cp is speciﬁc heat capacity, T is temperature, t is time, k is thermal conductivity, and g is the heat generation density.

And the resulting equation from a change of variable U (r, t ) = r T (r, t ) is shown below:

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 32

∂U ∂t = α ∂ 2U ∂r 2 + r g ρCp (2.40)

with boundary conditions: U0 = 0 (2.41)

∂U ∂r |r =R = ( 1 R − h k )U + R k hT∞ (2.42)

This equation system is readily solved numerically under ﬁnite volume discretization, where spherical elements are divided into ﬁne annuli, so that the temperature in each can be considered uniform. ∂u ∂r |i + 1 2 = ui +1 − ui ∆ri ∂u ∂r |i − 1 2 = ui − ui −1 ∆ri −1 (2.43)

∂2U ∂r 2 |i = ∂U ∂r |(i + 1 2 ) − ∂U ∂r |(i − 1 2 )

1 2 (∆ri + ∆ri −1) (2.44)

= Ui +1−Ui ∆ri − Ui −Ui −1 ∆ri −1 1 2 (∆ri + ∆ri −1)

= Ui +1 ∆ri + Ui −1 ∆ri −1 1 2 (∆ri + ∆ri −1) − 2Ui ∆ri ∆ri −1

∂u ∂t = k ρcp [ ui +1 ∆ri + ui −1 ∆ri −1 1 2 (∆ri + ∆ri −1) − 2ui ∆ri ∆ri −1 ] + r g ρcp (2.45)

The resulting discretized equation in term of the temperature T is,

∂Tpi ∂t ρcp = k ri [ ri +1Ti +1 ∆ri + ri −1Ti −1 ∆ri −1 1 2 (∆ri + ∆ri −1) − 2ri Ti ∆ri ∆ri −1 ] + g (2.46)

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 33

for i = 1, 2, ...n where Tpi is temperature of each pebble layer, and in particular the outer layer Tpn is temperature of the graphite shell, which is in contact with ﬂibe coolant. Taking the convective boundary condition into account, the temperature of the graphite shell follows the relationship in Equation 2.47: Tpn = hconv kg r aphi t e ∗ T f l i be + Tp,n−1 R−rn−1

1 R−rn−1 + hconv kg r aphi t e (2.47)

Likewise, the coated particle is divided into n layers. In order to save computational cost, the coatings are combined into a non-heat-generating layer with equivalent heat transfer properties. The heat transfer equation for the jth layer in a TRISO particle that is situated in the ith layer of a fuel pebble is shown in equation 2.48.

∂T pi , j ∂t ρi , j cpi , j = ki , j ri , j [ ri , j +1Ti , j +1 ri , j +1−ri , j + ri , j −1Ti , j −1 ri , j −ri , j −1 1 2 (ri , j +1 − ri , j −1) − 2ri , j Ti , j (ri , j − ri , j −1)(ri , j +1 − ri , j ) ] + g (2.48)

where r is the radius of TRISO particles.

##### 2.2.2.6 Implementation of the model

The coupled neutron diffusion and porous media model is implemented in the COMSOL Multiphysics software. The general modeling process involves the following steps:

## 1. creating geometry and meshing

## 2. deﬁning parameters, such as material properties and group constants

3. deﬁning physics according to the speciﬁc scenarios that a user aims to simulate

## 4. solving the equations and postprocessing

All the above steps are performed in COMSOL with the aid of Matlab via the LiveLink for Matlab interface. The geometry is represented in COMSOL as a CAD model, on which a mesh is generated using the COMSOL meshing tools. Mesh quality is crucial for ﬁnite element method (FEM) simulation as the FEM approximates the real solution by solving a descretized equation system over the mesh grid. For most geometry, a free tetrahedral mesh sufﬁce, although further improvement to the mesh quality with limited element number can be accomplished with elongated or hexahedron mesh elements. The mesh size is directly linked to computation cost

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 34

and result accuracy. As the mesh element size goes to zero, the solution converges to the real solution at the cost of almost inﬁnite computational resources. To ﬁnd a mesh with optimal ﬁnesse, reﬁnement studies can be used to compare meshes with different sizes and to choose the one with the least computational requirement that satisﬁes the convergence criteria.

In addition to parameters such as material properties, the code takes temperature dependent cross-section functions as input, for which the generation process is discussed in section 2.2.2.3.

COMSOL provides built-in modules for various physics as well as the possibility for user deﬁned PDEs. Neutron diffusion and SPN equations are deﬁned in COMSOL through the ’user deﬁned PDEs’ interface. The detailed matrix deﬁnition in COMSOL can be found in appendix D. The heat transfer and momentum equations are generated directly by COMSOL’s porous media and heat transfer models, which were discussed in the previous section.

Once the equation system is assembled, the solver sequence starts by searching for an eigenvalue for the neutronics equations (equation 2.19) with cross-sections computed from an initial guess of uniform temperatures. A stationary solver then computes the temperature distribution in the core by solving the conservation equations in mass, momentum and energy with the power density distribution found by the eigenvalue solver. It is important to note that an eigenvalue solver evaluate the ﬂuxes and power density values (equation 2.49) on an arbitrary scale, so the power density has to be normalized onto the operation power in steady state solver , as shown in Equation 2.50. ˙P = G∑

g =1 κΣ f ,g φg (2.49)

˙PN = ˙P .Pop Ð

V ˙P d v (2.50)

where the quantities are deﬁned as follows: Pop = operation power, W ˙p = volumetric power density, W /m3

˙pN = normalized volumetric power density, W /m3

κ = mean energy generation per ﬁssion, MeV Σ f ,g = macroscopic ﬁssion cross-section of group g , m−1

φg = neutron ﬂux in the g -t h energy interval [Eg , Eg −1], m −2s −1

G = total number of energy groups.

Another eigenvalue search is then performed with the new cross-section set determined from the temperature distribution from the stationary solver solution. And the two steps are repeated

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 35

until the results converge, i.e. the keff differs by less than 1 pcm between two iterations.

At the end of the iterations, the ﬂuxes are scaled so that the volume integration of the power density equals to the operating power of the reactor in simulation. Normalized delayed neutron precursor concentrations are also computed, from the normalized ﬂuxes. The scaling factors for both are shown in the following equations:

φg N = φg Pop Ð

V ˙P d v (2.51)

Cd N = βd ∑G g =1(νΣ f )g φg N λd = Cd Pop Ð

V ˙P d v (2.52)

where the quantities are deﬁned as follows: Pop = operation power, W ˙p = volumetric power density, W /m3

ν = mean number of neutrons generated per ﬁssion Σ f ,g = macroscopic ﬁssion cross-section of group g , m−1

φg = neutron ﬂux in the g -t h energy interval [Eg , Eg −1], m −2s −1

βd = delayed neutron fraction for delayed neutron precursor group d λd = average decay constant of delayed neutron precursors in group d, s−1

Cd = delayed neutron precursors concentration in group d G = total number of energy groups D = total number of delayed neutron precursors groups.

Once the ﬂux and neutron precursor concentration are scaled, one can simulate transient behaviour by solving the coupled equation system with time derivative terms in COMSOL’s transient solver, with time step size being automatically chosen based on the time derivative of the dependent variables.

A FEM solver discretize the space according to the mesh and construct a linear equation system of the basis functions. Solving the system of linear equations is a crucial part and the most computationally demanding part of a simulation, for which COMSOL provides two category of methods: direct solvers (e.g. MUMPS, PARDISO, and SPOOLES ) and iterative solvers (e.g. MUMPS, GMRES, FGMRES). In principal, direct solvers are easier to set up as they do not need preconditioning but require substantial amount of memory for matrix factorization. Iterative solvers approach the solution gradually and requires signiﬁcantly less memory, but need problem speciﬁc tuning. For smaller mesh, direct solver is faster because it does not need preconditioning and takes a single stride to the solution. But when the memory requirement approaches the hardware limit, the iterative solver performs better by approaching gradually to the solution.

CHAPTER 2. MULTIPHYSICS MODELING METHODOLOGY 36

The choice of solvers and their parameters should depend on the speciﬁc applications, but all the solvers should ﬁnd the same results if enough iterations are given.

### 2.3 Conclusion and future work

The governing equations for various numerical models for FHR cores were discussed in this chapter, as well as their implementation procedures.

With the capability of simulating continuous energy and full geometry details, Monte Carlo is widely viewed as a high ﬁdelity method for neutron transport modeling. Even though the Monte Carlo codes have the capability of modeling the exact geometry, simpliﬁcations can be made to save model preparation time and computational cost without causing signiﬁcant distortion in the results. For example, structured lattice can be used to model pebble packing without bias in the results [12]. Also, the reﬂectors are constructed from graphite blocks so that the relative movement between the blocks can damp the stress from radiation and temperature induced deformation. However, they are modeled as one piece in the Monte Carlo computation because the tiny gaps between the blocks are negligible compare to the neutron mean free paths.

Coupled models were developed in the work with the objective that a simulation can be carried on small-scale parallel machines or even stand-alone work-stations, which are typically machines available to nuclear engineering companies. Unit cell modelling method that couples zero dimensional point kinetics neutronics model and a one dimensional heat conduction model inside the fuel elements can provide quick results for preliminary analysis. For more detailed study, full core models based on multi-group neutron diffusion and porous media CFD is developed, with additional treatments including the SPN method and multi-scale temperature computation for a more realistic representation of an FHR core.

Radiative heat transfer inside the reactor cavity and between the reactor vessel and the ambient environment may be important for FHRs, given the high temperature in the core and is recommended as future work, but is not treated in the current study due to lack of material properties, such as the absorption coefﬁcients of the coolant salt.

All numerical models take a number of parameters as input. And in order to deﬁne a fully solvable equation system, the models make use of closure relationships that are generally empirical correlations, for example the Ergun correlation for pressure drop in pebble bed and the Wakao correlation for convective heat transfer coefﬁcient between pebbles and the coolant. Uncertainty and sensitivity study of the input parameters can give guidance on the uncertainties of the results, enhance conﬁdence in the results and provide guidance for future research projects.

37

Chapter 3

TMSR SF-1 core analysis

The TMSR research center was founded in 2011 at Shanghai Institute of Nuclear Applied Physics (SINAP) with the aim of studying, designing and commercially deploying molten salt based reactors as a part of China’s next generation nuclear reactor ﬂeet in the context of the rapid growing demand in energy and exacerbating air pollution from fossil fuel combustion. As a ﬁrst step of a multistage commercialization plan, the TMSR Solid Fuel-1 (SF-1) reactor is a research reactor that would serve the objectives of demonstrating the capability in molten salt based reactor design and construction, gaining hand-on experiences in novel reactor development, and providing experimental data for model validation in related ﬁelds, namely thermal-hydraulics, neutronics, material, etc. To minimize risk in such an innovative project, the design team takes a conservative approach by making use of existing technology that has been tested in other types of reactors whenever possible and simple design that might not be the most economical choice for large scale commercial deployment but relatively easy to implement in the ﬁrst FHR in the world. Its simplicity and representativeness of the FHR technology makes it a good choice as our model development and veriﬁcation baseline design.

The TMSR SF-1 reactor design is described in section 3.1, with an emphasis on the features that are relevant to the core modeling. The detailed design evolves as the team actively incorporates new research results, the 2014 design [49] is used in this work. A coupled thermal-hydraulics and neutronics model is created in section 3.2 for the TMSR SF-1 core by applying methodologies developed in chapter 2. Code-to-code comparison to reference models is crucial for code veriﬁcation, especially when experimental data are not yet available, and is thus performed throughout the model development. The comparison results for both local distributions (e.g. isothermal steady state power distribution, etc.) and global parameters (e.g. multiplication factor, time constant, temperature feedback coefﬁcients) are presented in section 3.3.

Once the model is developed and veriﬁed, the core behaviour under steady state and selected transient conditions is analyzed in section 3.5. Reactivity insertion accidents are chosen because they often involve an unwanted and fast increase in power, which leads to temperature

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 38

Parameter Value

Nominal thermal power, MW 10 TRISO packing fraction 9% Uranium enrichment 17.0% Flibe Li-7 enrichment 99.99% Pebble diameter, cm 6 Approximate fuel pebble number 11000 Coolant inlet temperature, ◦C 672 Coolant outlet temperature, ◦C 700 Porosity 40% Fuel region radius, m 0.68 Coolant mass ﬂow, kg/s 150 Height of the core (including reﬂector), m 2.86 Diameter of the core (including reﬂector), m 2.6 Control rods active length, m 2.406

Table 3.1: TMSR design parameters

rise in the fuel and coolant. If the fuel integrity is compromised due to overheating, the reactivity insertion (RI) transient may result in radioactive release into the primary coolant and even the environment in severe cases. Overcooling transients occur when cold coolant is injected in the core due to check valve opening or other anomalies. The immediate effect of cold coolant is the increase of nuclear power due to negative temperature reactivity feedback that may cause overheating in the fuel. On the other hand, due to the high melting point of the coolant, understanding the core behaviour under overcooling transients is also important in FHRs to prevent salt freezing that could potentially block the circulation and cause damage.

### 3.1 TMSR SF-1 design overview

TMSR SF-1 [46] is a 10 MW thermal nuclear reactor that combines the technology of liquid ﬂibe coolant and TRISO particle fuel. The geometry of the design is shown in ﬁgure 3.1. And the important design parameters can be found in table 3.1. The active region of the core consists of two opposite truncated cones with base diameter of 136 cm and aperture of 60 degree, connected by a 180 cm high cylinder with the same diameter. The cones are both cut at 30.3 cm in height with the minimum diameter of 30.0 cm.

TMSR SF-1 uses the same 6.0 cm fuel pebbles as those in the helium gas cooled high-temperature reactor (HTR), that contain a 5.0 mm thick graphite shell and the enclosed TRISO particles in

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 39

Figure 3.1: Schematic of the TMSR core design

a graphite matrix, as shown in ﬁgure 3.2. The dimensions of the TMSR fuel pebble design can be found in table 3.3. As the ﬁgure also shows, the TRISO particles are made of multiple layers with detailed dimensions listed in table 3.2.

TMSR SF-1 adopts a once-through fuel cycle, which although may not be the most economical approach comparing to online refueling, it is less demanding on the pebble handling system and is easier for predicting fuel composition in the core. At the start of a fuel cycle, the pebbles are loaded individually from the bottom of the core into the ﬂibe salt until the core reaches

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 40

Figure 3.2: Schematic of fuel pebbles and triso particles in TMSR SF-1

Layer Dimension

Fuel kernel diameter 500 µm Buffer layer thickness 95 µm PyC inner layer thickness 40 µm SiC layer thickness 35 µm PyC outer layer thickness 40 µm

Table 3.2: TRISO particle geometry in TMSR

criticality, with about 11,000 fresh fuel pebbles. Fuel pebbles are buoyant in the salt and ﬁlls the upper region. The bottom of the fuel pebble bed has a cone shape with an angle depending on the pebble/ﬂuid interaction. The lower region of the core is left with ﬂibe salt.

The primary coolant (enriched liquid ﬂibe) enters the core from the bottom at a nominal inlet temperature of 672◦C and ﬂows upward across the pebble region with a nominal outlet temperature of coolant is 700 ◦C . The heat is then transported to the secondary loop through a heat exchanger for electricity production.

The TMSR SF-1 core is surrounded by an outer graphite reﬂector, which provides neutron moderation and reﬂection that ﬂattens the radial power distribution and improves neutron econ-

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 41

Layer Dimension Fuel diameter 50 mm Shell thickness 5 mm

Table 3.3: Fuel pebble geometry in TMSR

omy. It also hosts various channels for neutron sources, salt and pebble loading and instrumentation, as well as the 16 control rods for reactivity control.

### 3.2 Multi-physics modeling for TMSR SF-1

As shown in Figure 3.3, the multi-physics reactor core model is composed by three regions: an upper porous region that represents a fuel pebble and ﬂibe salt porous mixture, a lower region with only ﬂibe salt and an outer graphite reﬂector region. Coupled heat transfer and neutron diffusion equations are solved with homogenized material properties for each region.

Figure 3.3: Schematic of three dimensional and two dimensional TMSR SF-1 core model geometry

In order to save computational cost, some simpliﬁcations are applied on the geometry. First, TMSR SF-1 has a substantial reﬂector that isolates the core from outer layers and the reactor building with respect to neutron transport. Therefore only the three main regions are modeled. Furthermore, details such as instrumentation tubes in the reﬂector are not expected to affect the neutronics or heat transfer behaviour signiﬁcantly and thus are not modeled in the current work. Plus, because the fuel pebbles are not modeled explicitly, the interface between fuel and ﬂibe region is represented as a straight line. The implications of this simpliﬁcation is discussed in section 3.3 when the results from this model is compared with those from explicit Monte Carlo models. A light-weight two-dimensional model with (r, z) cylindrical coordinates can be

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 42

used to simulate phenomena where azimuthal variation can be neglected, while a 3D model is available for non-symmetric phenomena, such as control rods movement.

Figure 3.4: Macroscopic cross-sections of major isotopes in the core(vertical bars are energy group bounds, MT2=scattering, MT18=ﬁssion, MT102=absorption)

Multigroup neutron diffusion equations are implemented into COMSOL as user deﬁned PDEs, with the homogenized group constants for each regions deﬁned as input. Furthermore, SPN treatment can be used in regions where the diffusion assumption is questioned, such as in the vicinity of the control rods. The group constants that are used by the diffusion and SPN models are generated from a Serpent model for the TMSR SF-1 core with explicit representation of randomly packed TRISO particles and fuel pebbles, using the nuclear data library ENDF/B-VII.0.

In order to automate the process and to ensure reproducibility of the results, a MATLAB code is developed to read data from Serpent output ﬁles, ﬁt the parameters to the respective functions of reactor conditions, and input the group constants into COMSOL. The code is set up for any user deﬁned group structures and the one that is used in the current models is shown in table

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 43

Group # Lower Energy Boundary (MeV)

1 1.4E+00 2 2.5E-02 3 4.8E-05 4 4.0E-06 5 5.0E-07 6 1.9E-07 7 5.8E-08 8 0.0E+00

Table 3.4: Energy group structure adopted in the multi-group neutron diffusion model

Region Density Conductivity Speciﬁc heat [g/cc] [W/m.K] [J/kg/K]

Kernel 10.5 3.5 400 Buffer 1.0 0.5 2000 IPyC 1.9 4.0 2000 SiC 3.2 30 1300 OPyC 1.9 4.0 2000 Coating* 1.89 0.76 1736 Table 3.5: Nominal material properties of TMSR fuel element used in the models [17][14]. *Coating combines all the non-power-generating layers in a TRISO particle.

3.4. The eight-group energy structure is chosen to capture the cross section features of the major isotopes: U-235, U-238, and those in ﬂibe. Figure 3.4 shows the normalized spectrum in different reactor zones and the scattering, ﬁssion, absorption and (n, 2n) cross sections of the major isotopes, in superposition of the currently used group structure.

The thermal-hydraulics model is implemented in COMSOL as well, so that the multiphysics equation system can be solved in a fully coupled fashion. The model uses thermal property values in appendix B for ﬂibe coolant and graphite reﬂector. The homogenized pebble-wise properties and the properties of each layer of the fuel elements are listed in table 3.5 and table 3.6. These nominal values are used in most of the models where the variation in thermo-physical properties for solid materials are not signiﬁcant within the range of the temperature change. For investigations of effect of material property changes with temperature, temperature dependent material properties are listed in appendix B.

To model the ﬂow ﬁeld and heat transfer in the pebble bed, porous media approach computes

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 44

Region Density Conductivity Speciﬁc heat [g/cc] [W/m.K] [J/kg/K]

Fuel 1.81 15 1744 Shell 1.96 193 684 Pebble* 1.85 17 1700 Table 3.6: Nominal material properties of TMSR fuel pebble used in the models [17][14]. *Pebble: pebble-wise material properties

a liquid temperature and a fuel temperature at each point in the region and connect the two phases through convective heat transfer. The fuel temperature is of prime importance in FHR transient response because of the large Doppler temperature reactivity feedback. Therefore, multiscale temperature treatment is implemented for a more realistic representation of the feedback from fuel temperature changes. Temperature inside fuel pebbles are computed in three subdivided fuel layers, and likewise the fuel kernel in the TRISO particles are divided into three sub-layers, for which temperatures are tracked during a transient and are used to compute fuel region macroscopic cross-sections. The effects of this treatment on the response to reactivity insertion and overcooling transients are discussed further in section 3.5 when we analyze the TMSR transient behaviors.

A multiscale cross-section is express as a linear combination of the logarithm of fuel temperatures. ˆΣ(T f uel ) = c0 + [c1, c2, ...cn] 

      l og (T11) l og (T12) l og (T13) ... l og (Tnm) 

      (3.1)

where ˆΣ(T f uel ) is the estimation of a fuel macroscopic cross-section, T f uel is the temperature or temperature vector in case many temperatures affect the same cross-section, Tnm is the temperature of the mth layer in a TRISO particle in the nth layer of a fuel pebble and c0, c1 are the coefﬁcients we search for.

#### 3.2.1 Mesh reﬁnement study

The TMSR core model with homogenized regions is meshed with free triangular (2D) or tetrahedral (3D) elements. The effect of mesh size, as well as the order of the mesh elements, on solution accuracy is studied through mesh reﬁnement studies, where the mesh element size

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 45

Figure 3.5: Logarithmic decrease in computation error with increase in degree of freedom for both linear elements and quadratic elements

is decreased gradually, with the minimum mesh size ranging from 0.1 m to 0.004 m, and the solutions from both quadratic and linear meshes are compared.

Figure 3.5 shows the difference in the multiplication factor between the results obtained with a given mesh and with the reference one, which is the ﬁnest mesh with the highest element order. The plot shows an almost exponential decrease in computation error with the increase of degree of freedom. Also shown on the plot is that the order of the polynomial functions deﬁned over the mesh elements affects the speed at which the solution converges. The error drops more quickly with quadratic elements. This is because FEM method solves the equations by ﬁnding a solution composed of the sum of the product of shape functions and associated coefﬁcients. Higher order functions represent the solution with larger ﬂexibility and can approximate the real solution with less elements. Finer mesh provides more accurate results but also requires more computer resources. One needs to choose the mesh size by balancing the available resource and desirable accuracy.

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 46

### 3.3 Code-to-code veriﬁcation

Code-to-code comparison is crucial for code veriﬁcation when experimental data is lacking. Code veriﬁcation has been done in this work by comparing both steady state and time dependent results to a reference code under various conditions.

#### 3.3.1 Overview of the reference model

The reference model [4] couples the neutron transport code Serpent and CFD code OpenFOAM internally for best computation performance. Serpent solves the neutron transport equation with Monte Carlo method using continuous energy nuclear data with full geometry details. OpenFOAM simulates heat transfer phenomena and computes temperature distribution in the coolant and fuel. To model the reactor core as realistic as possible, the random packing of TRISO particles and fuel pebbles are also simulated, using the discrete element method (DEM).

Figure 3.6 shows an example of the power and temperature results from the reference model. The reference model can computes detailed three dimensional distribution of power, temperature, neutron ﬂux, as well as global parameters such as keff , temperature reactivity feedback coefﬁcients with high ﬁdelity and is thus used for code-to-code comparison in this work.

#### 3.3.2 Power distribution comparison

Isothermal condition, whereby the core is maintained at uniform temperature by external heat source, is a good starting point for code-to-code comparison for the neutronic modules as they do not require coupling with thermal-hydraulics. In order to verify the reliability of the neutronic modules, the power density distribution is computed.

Figure 3.7 shows the steady state power distribution on a 2.0 cm x 2.0 cm mesh, the result from the COMSOL model on the left and that from the reference model on the right, both normalized such that the full core integration is unity. Satisfying similarity has been found between the results. The diffusion-based model captures the boundary effect of the reﬂector adequately despite the fundamental ﬂaw of the diffusion assumption in vicinity of the boundaries. However, because the individual pebbles are not explicitly modeled, the diffusion model cannot represent the local variation in power due to the discrete packing of fuel pebbles. As a result, the power proﬁle seems smoother in the COMSOL model. In a randomly packed pebble bed, the porosity is slightly higher at the center and lower at the wall because of the ordering effect. Near the walls, the Monte Carlo result shows a drop in power density due to ordered pattern of pebbles, but this can not be captured by the COMSOL model because it uses homogenized cross sections in the fuel region. The variation can be better reﬂected in the COMSOL model if the core is divided into multiple radial zones, with different set of cross-sections generated for each zone. Furthermore, the interface between the fuel pebble region and the ﬂibe region

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 47

Figure 3.6: An example of reference results showing the pebble wise power generation in the fuel region [4]

is approximated by a straight line in the COMSOL model. This is different from the randomly packed fuel region in the reference model but can be calibrated for a better match.

In addition to the qualitative comparison of the two-dimensional (r, z) proﬁles, the power density is integrated along the axial or radial directions and the resulting axial or radial power distributions are compared respectively in ﬁgure 3.8 and 3.9. The difference in axial power from the two codes in ﬁgure 3.8 is mainly due to the approximation in the shape of the fuel region that the COMSOL model makes. The most obvious discrepancy is found at the fuel-ﬂibe interface, where the power density drops to zero, but this can be better calibrated in the geometry. Figure 3.9 shows the difference in radial power distribution. The step changes in power density along the radius in the reference result is due to the effect of discrete pebble packing, which causes local variations in fuel pebble packing fraction. This phenomena is not captured in the COMSOL model.

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 48

Figure 3.7: Steady state power proﬁle computed from the COMSOL model and the Monte Carlo reference model Figure 3.8: Comparison of the axial distribution of normalized power

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 49

Figure 3.9: Comparison of the radial distribution of normalized power

Parameter value

COMSOL keff 1.03029 Reference keff 1.08416± 0.00012 ∆keff 0.05397

Table 3.7: Comparison of multiplication factor

#### 3.3.3 Multiplication factor comparison

The ﬁrst global parameter that is compared under isothermal conditions is the multiplication factor, which is the most important global parameter in nuclear reactor modeling and control. At the same nominal conditions (ﬂibe density at 1.9 g /cm3 and fuel temperature at 900 K), the multiplication factor value from COMSOL and reference model is shown in table 3.7. The difference between the COMSOL model and the reference model is larger than two times of the Monte Carlo statistical error, indicating that the difference is statistically signiﬁcant. This shows that diffusion based models are inadequate in computing the neutron multiplication factor.

However, for transient analysis, which we are most interested in, the changes in the multiplication factor when the operation conditions deviates from the nominal ones is more important than the absolute values. And the time scale and amplitude of these changes are compared respectively in section 3.3.4 and section 3.3.5.

#### 3.3.4 Time constant comparison

The time constant of the system is compared via zero power reactivity insertion transient simulations. External reactivity is inserted into the core at norminal zero power conditions and without temperature reactivity feedback that would stablize the power at a certain level, the re-

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 50

Figure 3.10: Comparison between the diffusion based model and the Monte Carlo based model for zero power reactivity insertion transients

actor power increases exponentially due to excess external reactivity, as shown in equation 3.2. The speed in which the power ramps up is a direct characteristic of the reactor time scale in the model, as shown in equation 3.2. P /P0 = eρt /λ (3.2)

where P is the real time power, P0 is the initial power, ρ is the reactivity, t is time, and λ is the characteristic time constant of the system.

Figure 3.10 shows the power excursion within 0.1 second following a reactivity insertion. As we can see, the results from the COMSOL model match closely to those from the Serpent model, as well as the analytical prediction, for a large range of inserted reactivities, from 26 pcm to 3599 pcm.

#### 3.3.5 Temperature feedback comparison

The ability of accurately capturing the temperature reactivity feedback coefﬁcients is crucial for a coupled neutronics and thermal-hydraulics model because these parameters are the macroscopic manifestation of the coupling between thermal hydraulics and neutronics. They embody the effects of temperature change on core neutronics.

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 51

Figure 3.11: Comparison between the diffusion based model and the Monte Carlo based model for fuel and ﬂibe temperature (and void) feedback effect. Error bar shows statistical error in Monte Carlo results.

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 52

The two major ones in FHR, the fuel Doppler feedback and the coolant temperature feedback coefﬁcients, are computed from both models and are compared in ﬁgure 3.11. The same y-axis range is used in the two plots for a visual comparison of the order of magnitude of these two feedback mechanisms. The fuel Doppler feedback is obviously the dominant effect in TMSR SF-1 cores.

To calculate the temperature reactivity coefﬁcients, the temperature of the fuel component is varied by 300 K in the range between 300 K and 1500 K and the density of the ﬂibe is changed by 100 kg /m3 between 1700 kg /m3 and 2100 kg /m3. The coolant temperature reactivity coefﬁcient includes the effect of temperature on cross sections, as well as the effect of density change due to temperature. In order to compare the variation of reactivity due to temperature change, delta reactivity is compared. It is computed as the difference between the reactivity at a reference temperature (900K for fuel and 1051K for ﬂibe, which corresponds to a density of 1900 kg /m3) and at a given temperature.

The fuel temperature feedback curves agree well. The relative error in ﬂibe reactivity feedback is larger but the absolute error is small due to the small magnitude of ﬂibe feedback. And the discrepancy between ﬂibe feedbacks is within statistical errors of the Monte Carlo computation.

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 53

(a) Fast neutron ﬂux ([5.00E-07, 1.40] MeV) (b) Thermal neutron ﬂux ([0.00, 5.00E-07] MeV)

Figure 3.12: Steady state neutron ﬂux

### 3.4 TMSR SF-1 steady state results

After careful code-to-code veriﬁcation for the multi-physics model, parameters such as power, ﬂibe and fuel temperatures, and neutron ﬂux are computed with design parameters in table 3.1. Uniform temperature and uniform upward velocity are imposed as the inlet boundary condition. Vacuum boundary condition for neutron diffusion is used outside of the reﬂector.

The neutron ﬂux distribution in the core is plotted in ﬁgure 3.12. An eight-group energy structure is used in the neutron diffusion equations but the neutron ﬂux values are regrouped into thermal ([0.00E+00, 5.00E-07] MeV) and fast ([5.00E-07, 1.40E+00] MeV) regions for plotting purpose. As conﬁrmed by the plots, fast neutrons are generated exclusively from nuclear reactions in the fuel region and are then scattered by the matters in the core as they get thermalized. The moderation effect of the coolant is dwarfed by that of the graphite reﬂector in TMSR. As a result, a large portion of thermal neutrons is found inside the reﬂector, besides the center of the core.

The thermal neutrons that travel back from the reﬂector to the fuel region cause a peak in power density near the border as they trigger nuclear reactions. As shown in ﬁgure 3.13, the power peaks at the center of the fuel region, decreases outward and increases again next to the reﬂector boundaries due to the moderation effect of the graphite reﬂector. This conﬁrms that the neutron diffusion based model captures the boundary effect adequately. The radial power peaking in TMSR SF-1 is 1.24 at nominal conditions with fresh fuel.

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 54

Radial and axial power proﬁle

Figure 3.13: Power distribution in TMSR SF-1

(a) Flibe (b) Fuel

Figure 3.14: Steady state temperatures in TMSR SF-1 core, without multiscale treatment

Under the standard porous media model, a ﬂuid temperature and a solid temperature are computed at each two dimensional points in the core. Beyond this, the multiscale treatment in the model tracks the heat transfer inside solid fuel elements and computes a set of temperatures that represents the temperature proﬁle at various depth of the fuel elements. The resulting temperatures with or without the multiscale treatment at steady state are compared in ﬁgure 3.15 and 3.14. At steady state, it is assumed that all the nuclear power is generated in the fuel and is transported to coolant and subsequently to the secondary loop through heat exchang-

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 55

(a) Flibe (b) Fuel surface (c) Inner TRISO fuel layer

Figure 3.15: Steady state temperatures of coolant and selected locations in fuel element in TMSR SF-1 core, with multiscale treatment

ers. Therefore, the coolant temperature proﬁle are very similar with or without the multiscale treatment in fuel. Because of the large volumetric heat capacity of ﬂibe, the coolant is a very efﬁcient heat carrier in FHRs. The coolant enters the bottom of the TMSR SF-1 core at 672◦C , as imposed by the inlet boundary condition, and gradually heats up to, on average, 705◦C as it ﬂows through the fuel region.

Among all the temperatures computed in the solid fuel elements, only the coldest (in the pebble shell) and the hottest (in the center layer of TRISO particles) temperature in a pebble are shown in ﬁgure 3.15 as the upper and lower bound of the solid phase temperatures. The temperature distribution in the center of the TRISO particles resembles the power proﬁle: hottest in the center of the core and coldest at the two axial extremities, within a range between 710 and 790 ◦C . The surface temperature is, on the other hand, affected both by the nuclear power and by the coolant ﬂowing upward across the core and shifting the peak temperature upward. The hottest fuel surface temperature is attained at the top of the core, where the coolant is the hottest.

Overall, both the coolant and the fuel are far below their respective safety limits at steady state. The large thermal margins make the TMSR SF-1 core very robust to transient scenarios.

### 3.5 TMSR SF-1 transient analysis

To demonstrate the model’s capability in simulating transient behaviour and to understand the implication of the multiscale treatment on safety parameters, such as power, coolant temperature and temperature in the solid fuel elements, in comparison to the conventional porous

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 56

Figure 3.16: Full core power during a 1$ reactivity insertion transient in TMSR SF-1, with or without multiscale treatment

media approach, reactivity insertion and overcooling transients are simulated and the results are discussed in this section.

In the multiscale case, the average fuel temperature is computed as a volume average of all the solid layers that contains fuel material, i.e. the temperature of graphite layers are not included in the average. This can then be averaged over the two dimensional space to obtain the core average fuel temperature. And in a similar fashion, the core maximum fuel temperature in the multiscale case is the maximum temperature of the hottest power-generating layer, which is the inner-most layer in a TRISO particle that is in the inner-most layer of a fuel pebble at the center of the core.

#### 3.5.1 Reactivity insertion transients

Starting from steady state under nominal conditions and with the same boundary conditions described in the previous section, 1$ is inserted into the core at time 0. Although an insertion of 1$ is highly unlikely due to the designed limit of the control rod, it is useful to test the model’s ability to handle a large amount of reactivity insertion and to understand the core behaviour in this case as a bounding example. At the onset of the reactivity insertion transient, nuclear power rate increases due to excess of reactivity. Following the power excursion is the temperature rise in the core, ﬁrst in the fuel, due to the direct deposit of the nuclear energy, and then in the coolant as the energy diffuses out of the fuel pebbles. By design, both elements provide negative temperature reactivity feedback that at the end re-stabilizes the core.

Temperature reactivity feedback is the most important natural safety mechanism in the core during a reactivity insertion without SCRAM. In FHRs, the feedback from the Doppler broadening of the fuel resonance absorption cross-sections, i.e. the Doppler feedback, is much more prominent than the coolant void feedback. And it reacts promptly, because the Doppler feed-

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 57

Figure 3.17: Average and maximum ﬂibe temperatures during a 1$ reactivity insertion transient in TMSR SF-1, with or without multiscale treatment

back is directly initiated by the changes in the fuel temperature. As a result, the timescale of a reactivity insertion transient is short, shorter than the circulation time of the primary coolant, so the inlet coolant condition remains constant during the simulation.

Figure 3.16 shows that the power excursion is less severe and the power stays at a lower level after the transient, when the detailed fuel temperature proﬁle is used to compute the Doppler feedback. Also, the response is quicker in this case because the promptness of the Doppler feedback is properly captured.

Comparison of the core average and maximum ﬂibe temperatures between the conventional porous media model and the one with the multiscale treatment is shown in ﬁgure 3.17. The coolant ﬂows upward through the fuel pebbles in the TMSR SF-1 core, so the hottest coolant is found at the top. In other words, the maximum coolant temperature also represents the outlet coolant temperature, one of the most important safety parameters in a FHR because this directly relates to the temperature of the metallic structures, the most vulnerable components in a FHR in terms of temperature safety margin. The steady state ﬂibe temperature is the same in

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 58

Figure 3.18: Average and maximum fuel temperatures during a 1$ reactivity insertion transient in TMSR SF-1, with or without multiscale treatment

both models, but both the average and the outlet coolant temperature rise to a higher value during the reactivity insertion transient in the non-multiscale case, the amount of heat deposited to coolant and the temperature reactivity feedback from the coolant are overestimated.

Figure 3.18 shows the average and maximum fuel temperature in the core during the transient, obtained with or without the mutliscale treatment. The peak fuel temperature in the multiscale case is higher at steady state, because the inner layers with higher temperatures are included in the average, and it remains higher during the reactivity insertion transient because of the spread in temperature inside fuel elements. On the other hand, although the average fuel temperature in the multiscale case is higher at steady state due to the higher temperatures inside the fuel elements, it is lower at the end of the transient because the reactor core stabilize at a lower power than the non multiscale case. In general, the peak fuel temperature, from which the safety margin to fuel damage is computed, is a more important safety parameter for the fuel. The conventional porous media model underestimates the peak fuel temperature during a reactivity insertion transient and could lead to an overly optimistic safety margin with respect to fuel damage during reactivity insertion transients.

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 59

Overall, the conventional model overestimates the temperature increase in the coolant and in the average fuel temperature, whereas it underestimates the peak fuel temperature. However, when comparing the results, we need to keep in mind of the uncertainties introduced by the material properties used in both approach. In fact, pebble wise average properties are used in the non-multiscale case, whereas more detailed layer wise properties are used in the multiscale case, and large uncertainties are associated with the measurement of all these values. A large variation exists in the limited literature on TRISO fuel pebble thermal properties measurements. In addition, to simplify the computation, the outer non-power-generating layers in TRISO particles are combined into one, with an equivalent conductivity computed from the properties of the substituting materials. The contact resistance at the interfaces of the layers are not taken into account when computing the equivalent properties because of lack of available measurements.

To evaluate the sensitivity of the results to the material properties, in particular thermal conductivity values of the fuel elements, ﬁgure 3.19 shows the core average temperatures in each TRISO and pebble layer during the 1$ reactivity insertion transient, with either nominal material properties (table 3.5 and table 3.6) or an uniform thermal conductivity (193 W/m.K, a typical value for nuclear graphite material) for each layer. In each individual TRISO particle, the center is the hottest and the temperature decreases as the heat diffuses out of the fuel material. With the nominal properties, a large temperature drop occurs at the coating due to the buffer layer. The buffer layer in a TRISO particle has a porous structure that serves as a damper to the stress caused by gaseous ﬁssion production. The porous nature of this layer makes it a thermal insulator and sets the lower bound of the conductivity. The buffer layer has the most temperature variation compare to the other layers. In both cases, the temperatures deviate further from each other during the transient, because of the elevated power level. And the temperatures are further apart with the nominal thermal conductivity values.

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 60

Figure 3.19: Core average temperature inside fuel elements during 1$ reactivity insertion transient for TMSR SF-1, with a large thermal conductivity posed to all material(top) or with nominal material properties for each layer(bottom). Notation for temperatures: T Pi j represents the jth TRISO layer in the ith pebble layer. (A fuel core in a pebble is divided into three layers and within each layer, a TRISO particle is divided into four layers - three fuel layers and one coating layer.). And T Pshel l l is the temperature in the graphite shell of the pebble.

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 61

As discussed before, the multiscale treatment is important in capturing the correct feedback behaviour, so the results shown hereafter are with the treatment. Figure 3.20 shows the power and temperature evolution during a 1$ or 0.5$ reactivity insertion. During the 1$ prompt insertion transient, the core maximum fuel temperature is 1500◦C , below the safety limit(1600 ◦C ). Coolant temperature is raised by only 100 ◦C at the peak.

Figure 3.20: RI transients results

Figure 3.21 shows the effect of the timescale of the reactivity insertion on the safety parameters.

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 62

Without SCRAM, the reactor reaches the same end state due to reactivity feedback mechanisms. However, safety measures can take in place to mitigate the consequence if the transient is slow.

Figure 3.21: RI transients results

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 63

#### 3.5.2 Overcooling transients

Figure 3.22: Full core power during overcooling transient

Starting from the steady state conditions (section 3.4), the inlet coolant temperature is decreased by 100◦C in 10 seconds and remains at the lower level during the simulated overcooling transient. Within the short timescale of the simulation, it is reasonable to assume that the other inlet conditions (e.g. coolant mass ﬂow rate) and the reactor control parameters (e.g. control rod positions) remain unchanged.

The colder coolant introduces positive reactivity insertion due to negative ﬂibe temperature reactivity feedback, which causes a power increase, as shown in ﬁgure 3.22. The increase is slow at the beginning because the temperature reactivity feedback is mainly from the coolant, weaker then the Doppler feedback from the fuel, which comes into effect after about 10 seconds when the cooling reaches the fuel materials. Because the transient is initiated in the coolant, the multiscale power increases less than that in the model with uniform fuel temperature, because it can capture the feedback from fuel while the fuel material gets cooled gradually from the shell to the center kernel. The change in temperature in turn results in negative reactivity feedback that stabilize the power at a higher level than the initial rate.

Figure 3.23 shows that both the average and maximum coolant temperature drops at ﬁrst due to the colder inlet and increase later due to the increased power. The average coolant temperature drops immediately due to the decreased inlet temperature. On the other hand, the maximum coolant temperature, which is also the outlet temperature, is not affected until the colder coolant arrives at the top of the core. This parameter is directly related to the hot-leg temperature and determines the reactor safety margin. However, this is not a safety concern for over-cooling transients where the coolant temperature is below the steady state condition.

Figure 3.24 shows the variation of both the maximum and the average fuel temperature during the transient, with two different models. The average fuel temperature drops with the coolant, heats up from the reactivity feedback and in the end stabilizes at a lower level than the steady

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 64

Figure 3.23: Core maximum and average ﬂibe temperature during overcooling transient

state temperature. The location where the fuel temperature peaks changes from the center of the core to the top as the coolant ﬂows through the core. At the end of the transient, the peak fuel temperature is higher than the steady state temperature despite the colder coolant that ﬂows through the core, but is still well below the safety limit. Both the average and the maximum fuel temperature in the core is higher with the multiscale treatment, because it captures the feedback from the fuel correctly.

Overall, the end state of the fuel temperature is lower while the coolant temperature is higher in the non-multiscale model because it overestimates the contribution to the negative reactivity feedback from the coolant. In fact, the estimated maximum ﬂibe temperature in this case is much higher than in the multiscale case, even higher than the steady state value. This may lead to over conservatism in the design.

If we take a closer look at the temperature proﬁle inside the fuel pebbles during the transient, ﬁgure 3.25 shows the temperature in each solid layer at each time stamp. It takes a few seconds for the drop in the inlet coolant temperature to affect the solid temperature. The core-wise average temperatures in the fuel elements decrease at ﬁrst as the cold coolant ﬂows upward in the

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 65

Figure 3.24: Core average and maximum fuel temperature during overcooling transient

core. At around 50 second, the temperatures increase due to the surge in power. The temperatures ﬁnally stabilize after about 150 seconds. The surface temperature is much lower than the steady state temperature because the coolant temperature is much lower. The temperatures gradient inside a fuel pebble is higher after the transient because the power is at a higher level. The difference between the hottest layer and the shell changed from 60 K to 130 K after the transient. As a result, the inner most layer has a slightly higher temperature after the transients, although the safety margin to fuel damage is still very large.

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 66

Figure 3.25: Core average of temperatures inside fuel pebbles during overcooling transient for TMSR SF1. Notation for temperatures: T Pi j represents the ith TRISO layer in the jth pebble layer. T Pshel l l is the temperature in the graphite shell of the pebble.

### 3.6 Conclusion

In this section, a coupled thermal-hydraulics and neutronics model for the TMSR SF-1 is developed, veriﬁed against a reference model, and then used to perform steady state and transient computation. The model provides high resolution two dimensional or three dimensional results within a reasonable amount of time on a computer with 16.0 GB RAM and four Intel Core equipped with i7-4790K CPU at 4.00 GHz.

Steady state parameters such as power, temperatures and neutron ﬂux are calculated and discussed. The large safety margins to fuel damage or coolant boiling at steady state make the core robust to various transients. The fuel and coolant temperature during a 1$ reactivity insertion is within the safety limit. Likely, during the overcooling transient, where the coolant inlet temperature is dropped by 100 ◦C in 10 seconds, the core-wise maximum fuel temperature is 950◦C , largely below the safety limit for fuel element. The coolant temperature stabilizes at a lower level than the initial condition, but is still above the salt freezing point.

In conventional porous media formulation, only an average temperature inside the solid phase is computed. This is a reasonable assumption in steady state cases and in most slow transient problems where the fuel kernel temperature is very close to the temperature of the surrounding graphite matrix. However, in the case of fast transients involving large power excursions, the difference between fuel and graphite temperatures can be signiﬁcant. Thus a detailed temper-

CHAPTER 3. TMSR SF-1 CORE ANALYSIS 67

ature proﬁle inside the fuel pebbles and TRISO fuel kernels is necessary for accurately computing temperature, which is important in capturing the Doppler temperature reactivity feedback. Overall, the power excursion in response to either a large reactivity insertion or a overcooling transient is dampened when a temperature proﬁle in the fuel element is used, because the temperature reactivity feedback is captured more realistically at the place where the largest feedback occurs. 68

Chapter 4

Mark-1 PB-FHR core analysis

Chapter 3 has presented the development of numerical models for TMSR, one of the pebblebed FHR design and veriﬁed the credibility of the results via code-to-code comparison against a reference model. In this chapter, we develop a suite of numerical models to support the analysis of Mark-1 PB-FHR (Mk1), a 236 MW (th) pre-conceptual design for a small modular FHR that has been developed at the University of California, Berkeley since 2002 [11].

An overview of the Mk1 core design is given in section 4.1, with an emphasis on the novel design features in view of enhancing heat transfer, reducing pressure drop, optimizing fuel utilization and etc. that may have important implications in the model development. More information on the design speciﬁcations for other components can be found in the design report [1]. Section 4.2 and 4.3 present how the Monte Carlo method and the coupled thermal-hydraulics and neutronics models (discussed in chapter 2) are applied to the Mk1 core. The models are used in section 4.4 and section 4.5 to study material choices, to optimize coolant ﬂow ﬁeld, and to simulate steady state and transient scenarios, including reactivity induced transients and overcooling transients.

### 4.1 Mk1 core design overview

As shown in ﬁgure 4.1, a Mk1 core has an annular pebble bed region, consisting of fuel pebbles and graphite blanket pebbles, surrounded by center and outer graphite reﬂectors. A Mk1 core contains over 470, 000 three-centimeter diameter fuel pebbles, smaller than the six centimeter ones used in helium cooled pebble-bed reactors and TMSRs. An illustration of the Mk1 fuel pebble and TRISO particle design can be found in ﬁgure 4.2. And the detailed geometry dimensions for Mk1 fuel elements can be found in table 4.2 and 4.3. Another difference from the TMSR SF-1 fuel is that the fuel pebbles in Mk1 are composed of three layers (instead of two): a low-density graphite core, a fuel layer and an outer graphite shell. The additional center graphite kernel in the Mk1 fuel pebbles provides neutron moderation, keeps the TRISO parti-

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 69

Figure 4.1: Schematic of Mk1 core geometry

cles closer to the coolant, and adjusts the overall pebble density so that they are slightly buoyant in the ﬂibe salt. The pebble shell is made of dense graphite to protect the fuel by enhancing the structural strength of the pebble. In the annular fuel layer of each fuel pebble, thousands of TRISO particles are uniformly dispersed in a graphite matrix, each containing a fuel kernel, a porous buffer layer, an inner pyrocarbon layer, a silicon carbide layer and an outer pyrocarbon layer. The dimensions of these layers can be found in table 4.2. The smaller diameter and the annular design of the fuel pebbles doubles the heat transfer surface area per unit volume and shortens the thermal diffusion length to a half, enabling a higher power density (approximately 22.7 MW t /m3) while maintaining low peak fuel temperature, minimizing over-heating risk in postulated Anticipated Transient Without Scram (ATWS) accidents.

The fuel pebbles are loaded from the bottom of the core and circulate in a slow upward motion across the core. In the time scale of the ATWS transients we simulate, ranging from several

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 70

Parameter Value Thermal power, MW 236 Packing fraction for TRISO particles (%) 40 Packing fraction for pebbles (%) 60 Fuel enrichment (%U-235) 19.9 Flibe enrichment (%Li-7) 99.999 Number of fuel pebbles 470000 Number of graphite pebbles 218000 Number of TRISO particles per pebble 4730 Coolant inlet temperature, ◦C 600 Coolant bulk-average outlet temperature, ◦C 700 Coolant ﬂow, kg/s 976 Estimated coolant bypass, % 5 Inner reﬂector radius, cm 35 Average power density, MW /m3 23 Volume of active fuel region, m3 10.4

Table 4.1: Mk1 PB-FHR core design parameters

Layer Dimension

Fuel kernel diameter 400 µm Buffer layer thickness 100 µm PyC inner layer thickness 35 µm SiC layer thickness 35 µm PyC outer layer thickness 35 µm

Table 4.2: TRISO particle geometry in Mk1 PB-FHR [1]

seconds to a few minutes, we can assume that the pebbles are ﬁxed in space. After every pass, they are examined and reintroduced into the core until they reach the burnup limit. Meanwhile, newer pebbles are introduced in the core to compensate the deﬁciency in reactivity due to burnup. Online refueling allows the core to operate with limited excess reactivity and thus better safety prevision for control rod removal transients. Pebbles at different burnup level have different thermal and neutronic properties that need to be taken into account in the numerical models.

Because of the large heat capacity of ﬂibe salt, it is possible to have a low coolant ﬂow rate in the core. In the Mk1 core, about 30% of the coolant ﬂow enters from the downcomer at the bottom of the core whereas the rest is injected radially from the center reﬂector channels. The cross ﬂow design reduces the pressure drop across the core and therefore reduces the total salt inventory

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 71

Layer Dimension Graphite core diameter 25 mm Fuel thickness 1.5 mm Shell thickness 1 mm

Table 4.3: Fuel pebble geometry in Mk1 PB-FHR [1]

Figure 4.2: Schematic of fuel pebbles and TRISO particles that are used in Mk1 PB-FHR

and the ﬂow resistance under natural-circulation for decay heat removal. Due to 6Li ’s large absorption cross-section, the ﬂibe salt needs to be enriched in 7Li in order to achieve negative temperature reactivity feedback from the coolant. 99.999% is used as nominal ﬂibe enrichment in this work.

In addition to the graphite material inside the fuel elements, a Mk1 core contains multiple graphite components, including an outer reﬂector and a center reﬂector that surround the core, providing neutron moderation and reﬂection. Between the fuel pebbles and the outer reﬂector is a 20 cm thick ring of graphite pebbles, with the main aim of protect the ﬁxed reﬂector and to extend its lifetime. The graphite pebbles can be unloaded through the defueling chute at the top and loaded from the bottom. It can also be replaced by thorium-loaded pebbles in a breeder reactor design, a concept not investigated in this thesis. Studies on granular dynamics [27] have shown that the pebbles move upward in the core with limited radial motion. Therefore, the fuel region and the blanket regions remain separated as the pebbles circulate and is modeled as separate regions in this work.

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 72

Component Density Material Temperature [g/cm3] [K]

Fuel Shell 1.75 graphite 900 Kernel 1.59 graphite 1000 Coolant 1.95 ﬂibe 1000

TRISO Matrix 1.7 graphite 1000 Kernel 10.5 UC0.5O1.5 1000 Buffer 1.05 graphite 1000 iPyC 1.90 graphite 1000 SiC 3.18 SiC 1000 oPyC 1.90 graphite 1000

Blanket pebbles graphite 900 coolant 1.97 ﬂibe 900

Center Reﬂector 1.74 graphite 900 Control rods 2.4 Natural B4C 900 Control rods coolant 1.97 ﬂibe 900

Outer Reﬂector 1.74 graphite with boron 900 Coolant channel outer reﬂector 2.08 graphite and ﬂibe 900

Core Barrel 8 SS316 900 Reactor Vessel 8 SS316 900 Table 4.4: Material and temperature for each core component in the reference Mk1 Serpent model. The isotope composition of the stainless steel in core barrel and vessel is given in table 4.5[30].

In addition to the online refueling regime for long term reactivity compensation, Mk1 uses eight Boron Carbide control rods for operating reactivity control and eight emergency blades for reserve shut-down. The control rod channels are located in the center reﬂector, where neutron ﬂux is the highest, for an efﬁcient reactivity control via neutron absorption. The control rods are inserted from the top of the core and thus can remain effective in case of earthquake when the fuel pebbles are densiﬁed toward the top of the core.

### 4.2 Monte Carlo modeling for Mk1

A Monte Carlo model is made for Mk1 PB-FHR in Serpent [29] to study detailed neutronics properties in the core and to generate parameters used by deterministic codes. Cross-section

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 73

Isotope C Fe Ni Cr Mo Si Mn S P

Fraction(%wt) 0.080 65.345 12.000 17.000 2.500 1.000 2.000 0.030 0.045 Table 4.5: Nominal composition of stainless steel 316, density 8.03g /cm3

Figure 4.3: Geometry of the Monte Carlo model for Mk1 in Serpent (View from the front, the top, and color legend), with all control rods fully inserted

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 74

Figure 4.4: Center reﬂector design

data from the ENDF/B-VII.0 nuclear data library is used for this model. The full core model is run with 10, 000 particles per cycle for 10, 000 cycles where the ﬁrst 500 are skipped when computing result statistics. Because of the high ﬁdelity of Monte Carlo based code, these models are also used to generate reference results to benchmark other models.

As shown in ﬁgure 4.3, the model is composed of center reﬂector, fuel pebble region, blanket pebble region, outer reﬂector, vessel, downcomer and core barrel, with uniform nominal material compositions and temperatures in table 4.4. The model geometry is based on the design report [1] with some simpliﬁcations in view of computation cost. The next sections describes how each components are modeled, including the assumptions and simpliﬁcations with justiﬁcations.

#### 4.2.1 Center reﬂector

Figure 4.4 shows the design of the center reﬂector. It is composed by a number of graphite blocks, with small gaps between each other to allow for thermal expansion and radiation induced swelling of the graphite material. It is modeled as a single piece because the gaps are negligible comparing to the neutron mean free path in graphite. The center reﬂector has coolant injection channels that deliver coolant to the fuel region through the injection holes and slot, which are not modeled in this neutronic model because the small amount of ﬂibe in the channels has negligible effects on the core neutronic behaviour.

In a Mk1 core, the thermal neutron ﬂux peaks in the center reﬂector, which makes it the best

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 75

Parameter Value Number of control rods 8 Diameter of control rods channels, cm 10 Width of control rods, cm 8 Thickness of control rods, cm 2 Bottom height (fully inserted), cm 112.5 Bottom height (fully retracted), cm 492.85 Density, kg/m3 2400 Material Boron Carbide

Table 4.6: Design speciﬁcation for control rods in Mk1

Material Thickness [mm] ∆ keff

SS316 5 0.08139 SS316 3 0.06798 SS316 10 0.09639 Zr 5 0.01196 SS316 without Ni 5 0.08033 SiC 5 0.01070 Table 4.7: Drop in keff due to different control rod liner materials and conﬁgurations

location for control rod channels. In fact, the center reﬂector houses eight control rod channels (design details in table 4.6). In each channel, a cross shaped control rod can be inserted to maintain criticality under normal operation conditions or to aid shutdown blades to shutdown the reactor under accident conditions. The design of the channels and the material used in the channels need to be evaluated carefully because of the high impact on neutronics. In order to study the feasibility of adding metallic (stainless steel) or ceramic (silicon carbide) liners to the channels to provide structural reinforcement, a Monte Carlo model with equilibrium fuel composition is used to compute ∆keff, the decrease in the multiplication factor, due to the presence of the liners. To estimate the maximum absorption effect of the liners on neutron ﬂux, the control rods are not inserted in the model, i.e. the channels are ﬁlled with ﬂibe coolant.

Table 4.7 lists the drop in keff with different liner materials and thicknesses. Stainless steel reduces the keff considerably, over 6000 pcm for 3 mm and almost 10000 pcm for 10 mm due to its high capture cross-sections and the central location of the channels. Zirconium, on the other hand, is more transparent to neutrons, but is also easily corroded by the coolant salt. Adding three millimeters of zirconium liners results in smaller reduction in keff . The SiC liner has also smaller effect on keff comparing to stainless steel and can potentially be a candidate material for the liner, given its neutronic performance as well as high temperature resistance. Carbon ﬁber reinforced composites (CFRCs) are also candidates, since they will have similar neutron-

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 76

ics effects as graphite.

#### 4.2.2 Fuel region

We model the pebble packing as a Face-Centered Cubic(FCC) lattice with a packing fraction of 60%. The ﬂibe salt ﬁlls the space between fuel pebbles. 0.9999 enriched ﬂibe is used in the current model to ensure negative coolant reactivity feedback.

Figure 4.5: Schematic of equilibrium fuel loading in a representative unit cell, numbers denote burnup levels

Either the fresh or the equilibrium fuel composition can be used in the model. Mk1 adopts an online refueling regime. Starting from fresh fuel, the pebbles circulates in the core for, on average, eight times. During the online refueling cycle, the fuel pebbles gradually reaches asymptotic packing and burnup distribution. An equilibrium unit cell containing equal volume of pebbles at eight different burnup levels, illustrated in ﬁgure 4.5, was computed with a depletion model[7] under an online refueling scheme.

#### 4.2.3 Blanket pebble region

Graphite blanket pebbles are also modeled in a FCC lattice with the same packing fraction and pebble dimension as the fuel pebble region. The coolant material in this region also has the same composition and temperature as that in the fuel region. A clear cut is made between the blanket and the fuel region in the model, whereas in reality a small number of pebbles might migrate into each other zones in the vicinity of the interface.

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 77

Component Outer Radius(cm) Material Shield 164.7 Boron Carbide Core Barrel 168 SS316 Downcomer 170 Flibe Vessel 175 SS316 Table 4.8: Dimension and material of Mark I core outer annular layers with shield[7]

Component Outer Radius(cm) Material Core Barrel 167.2 SS316 Downcomer 170 Flibe Vessel 175 SS316 Table 4.9: Dimension and material of Mark I core outer annular layers without shield[1]

#### 4.2.4 Outer reﬂector

The outer reﬂector is made of graphite with boron. Coolant channels in the outer reﬂector are modeled as a ﬁctitious material with 60% (volumetric) graphite and 40% (volumetric) ﬂibe in a 10 cm annular band in the active regions and extends up to the top of the core, as shown in green in ﬁgure 4.3.

#### 4.2.5 Outer layers

Outside of the graphite reﬂector lies the layers for structural integrity, namely the core barrel and the vessel. The cavity between core barrel and the vessel is ﬁlled with ﬂibe salt. A shield can be added between the outer reﬂector and core barrel to protect the metallic material. The dimension of the outer layers with or without the shield is shown respectively in table 4.8 and table 4.9.

### 4.3 Multiphysics modeling for Mk1

A coupled thermal-hydraulics and neutronics model is implemented for Mk1 for transient analysis based on the methodology developed in section 2.2.2. The model solves multi-group diffusion equations for neutronics, where the cross-sections are generated for each component using the Serpent model discussed in the previous section. The overall geometry (ﬁgure 4.6) is the same as the Monte Carlo model discussed in the previous section, except that the sharp corners in the center reﬂector are rounded for a more realistic ﬂow ﬁeld.

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 78

Figure 4.6: Schematic of the Mk1 multiphysics model geometry. Green region is the fuel pebble region and blue region is the blanket pebble region.

#### 4.3.1 Porous media model for coolant

The porous media formulation (discussed in section 2.2.2) is used to model the pebble-bed ﬂow ﬁeld and heat transfer in Mk1. The model uses salt thermal-physical properties in appendix B.

The convective heat transfer between the fuel surface and the coolant is modeled using the Wakao correlation [45], and the Ergun correlation [10] is used for pressure loss computation. The Ergun constants, E1 and E2, depend on conditions like ﬂow regime (e.g. Reynolds number), surface texture (e.g. roughness), pebble packing (e.g. porosity) and container shape (e.g. bed-pebble diameter ratio), and should ideally be measured for each pebble bed. However, without the measurements for Mk1 pebble bed, the values in the original Ergun correlation are used in this work. Table 4.10 shows the Ergun coefﬁcients and the constant used in COMSOL for deﬁning Ergun correlation in the momentum equation. The effect of the uncertainty on the results of the coupled models should be studied to decide if dedicated measurements are necessary.

Pressure boundary conditions are imposed on outlet surfaces. A velocity proﬁle or an overall ﬂow rate is speciﬁed for inlet surfaces. The boundary conditions are optimized in section 4.4.2

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 79

Parameter Value E1 150 E2 1.75 CF 0.52 Table 4.10: Pressure drop correlation parameters used in the porous media model

to achieve efﬁcient cooling and limited pressure drop.

As shown in ﬁgure 4.16, the coolant ﬂows through the Coiled Tube Air Heaters(CTAHs) after exiting the core and deposits heat to the power conversion system before coming back to the core. Thus during long transients that last more than the coolant circulation time, the coolant inlet temperature will be affected by the coolant coming back with a different temperature. To represent the primary loop, during longer transient scenarios, a simpliﬁed heat exchanger model is added to the core. In the future, coupling between the core model and system model would give a more realistic representation of transients that affects components outside of the core. The heat transfer rate in CTAHs is computed using the following equations. The heat exchanger efﬁciency and parameters about the air ﬂow are assumed to remain constant during the simulated transients because the power conversion system is not affected.

Qr eal = Qmax ∗ η (4.1)

Qmax = Cmi n(Tsal t ,i n − Tai r,i n) (4.2)

where: Qr eal = real thermal power, W Qmax = maximum attainable thermal power in counter-ﬂow heat exchanger, W η = heat exchanger efﬁciency = 0.9 Tai r,i n = cold air inlet temperature = 418.6[◦C ] Tsal t ,i n = hot salt inlet temperature Cmi n = mi n(( ˙mCp )ai r , ( ˙mCp )sal t ) = 461540[J/K] cp = speciﬁc heat ˙m = mass ﬂow rate

#### 4.3.2 Multi-scale model for the fuel region

The fuel region is divided radially and axially into 6 zones, based on the different neutron spectrum characteristics [7], and in each zone a different set of cross-sections is generated as a multi-variable linear function of the ﬂibe density and logarithm of fuel temperatures. The beneﬁt of computing the temperature proﬁle inside the fuel elements is demonstrated in section

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 80

Figure 4.7: Fraction of power generated in pebbles of each burnup level

3.5 and thus is also used for Mk1 models. The fuel kernel in each TRISO particle is divided into three layers with equal volume, i.e. same amount of fuel, and the temperature in each layer is computed at each time step using the multi-scale heat transfer model developed in section 2.2.2.

A Mk1 core contains fuel pebbles with various levels of burnup and approaches the asymptotic packing and burnup distribution as the pebbles ﬂow slowly across the core during an online refueling campaign. To model a core with equilibrium fuel in the multiphysics model, the temperatures in pebbles at each burnup level are computed. As the fuel elements undergo irradiation, its ability in generating power degrades due to the change in fuel composition. A fresh pebble would produce almost twice of power than a pebble that is at its last pass before reaching the burnup limit, as shown in ﬁgure 4.7. The fraction of power that is generated in pebbles of each pass is tallied using the Monte Carlo model and is used as input for the multiphysics model with equilibrium fuel.

Figure 4.8 shows the fuel pebble equivalent thermal conductivity based on fast neutron dose and the temperature at which the pebble is irradiated, computed based on the empirical formula in equation 4.3. The conductivity varies between about 15 W/m.K to more than 60 W/m.K depend on the irradiation damage and temperature. Most of the degradation occurs during the

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 81

Figure 4.8: Fuel pebble conductivity as a function of temperature and irradiation dose(data from [20])

ﬁrst pass of the pebbles in the core. To simplify the model, a constant thermal conductivity is used for pebbles of each pass, as shown in table 4.11.

λ = 1.2768 ∗ ( 0.6829 − 0.3906 ∗ 10−4T d osi s + 1.931 ∗ 10−4T + 1.228 ∗ 10−4T + 0.042) (4.3)

where λ is in the unit of W/cm.K, T is the temperature in degree Celsius (T<1200◦C ), and dosis is the fast neutron irradiation dose in 1021.

Pass No. 1 2 3 4 5 6 7 8 k[W/K.m] 40 17 17 17 17 17 17 17 Table 4.11: Thermal conductivity of fuel pebbles at different burnup levels

For a core with only fresh fuel pebbles, pebble wise equivalent properties are used for general modeling and more speciﬁc layer-wise properties are used in case of multiscale mode where each layer is modeled and their temperature is computed at each time step. The thermalphysical properties of the fuel elements are listed in table 4.12 and 4.13.

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 82

Region Density Conductivity Speciﬁc heat [g/cc] [W/m.K] [J/kg/K]

Kernel 10.5 3.5 400 Buffer 1.0 0.5 2000 IPyC 1.9 4.0 2000 SiC 3.2 30 1300 OPyC 1.9 4.0 2000 Coating* 1.89 0.76 1736 Table 4.12: Nominal material properties of Mk1 fuel element used in the models [17][14]. *Coating combines all the non-power-generating layers in a TRISO particle.

Region Density Conductivity Speciﬁc heat [g/cc] [W/m.K] [J/kg/K]

Kernel 1.74 193 684 Fuel 1.81 15 1744 Shell 1.96 193 684 Pebble* 1.85 17 1700 Table 4.13: Nominal material properties of Mk1 fuel pebble used in the models [17][14]. *Pebble: pebblewise material properties

#### 4.3.3 Control rod modeling and veriﬁcation

The control rod channels are discretized axially into multiple segments and two sets of cross sections are generated for each of the segments with or without control rod. This process generates a step function of cross-section as a function of height:

Σ(z) = { Σseg, ﬂibe(z), if z < zr od Σseg, rod(z), if z >= zr od (4.4)

where Σ is a cross-section as a function of the height, zr od is the control rod position, Σseg , f l i be is the cross-section value when the channel segment is ﬁlled with ﬂibe, and Σseg ,r od is the crosssection value when the channel segment is inserted with control rod.

To verify the capability of the COMSOL model in modeling the control rod effect on power distribution, power proﬁles with different control rod conﬁgurations are computed and compared to the results from a Monte Carlo reference model. For the comparison, both models use uniform nominal material temperatures and equilibrium fuel composition.

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 83

Figure 4.9: Steady state power distribution from the Monte Carlo reference model and from the COMSOL model (nominal conditions, no control rods inserted in the core)

Figure 4.10: Steady state neutron ﬂux distribution from the Monte Carlo reference model and from the COMSOL model (nominal conditions, no control rods in the core)

Figure 4.9 and 4.10 shows the power distribution in the core when no rods are inserted. Figure 4.11 and 4.12 shows the power distribution in the core when all the rods are fully inserted. Also, for a more quantitative comparison, all the control rods are moved together to different heights and the change in the multiplication factor is compared between the COMSOL model and the Serpent model, as shown in ﬁgure 4.13.

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 84

Figure 4.11: Steady state power distribution from the Monte Carlo reference model and from the COMSOL model(nominal conditions, control rods fully inserted in the core)

Figure 4.12: Steady state neutron ﬂux distribution from the Monte Carlo reference model and from the COMSOL model(nominal conditions, control rods fully inserted in the core)

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 85

Figure 4.13: Change in keff with control rod positions

### 4.4 Steady state Mk1 core analysis and design optimization

The ﬁrst part of the section discusses steady state characteristics for Mk1 core, assuming that the control rods are not inserted in the core and the core is loaded with fresh fuel. The effect of coolant inlet and outlet channels on the core behaviour is investigated in the second part of this section using coupled thermal-hydraulics and neutronics models.

#### 4.4.1 Power and ﬂux distribution

Steady state power,thermal and fast neutron ﬂux distribution are shown in ﬁgure 4.14. As shown in the ﬁgure, the thermal ﬂux peaks in the center reﬂector, in the graphite blanket pebble area and the outer reﬂector. As a result, the power peaks in the vicinity of the reﬂectors when the thermal neutrons enters the fuel area and initiates nuclear reactions. The annular design of the Mk1 core results in a power peak in the center reﬂector, where most of the thermal ﬂux aggregates.

The fast ﬂux is generated in the fuel region and is transported into the nearby area while the neutrons get thermalized. The blanket pebbles, between the fuel pebble region and the outer reﬂector, gets higher irradiation damage from the fast neutrons, serving as a disposable reﬂector.

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 86

Figure 4.14: Steady state power and neutron ﬂux distribution at operating conditions

#### 4.4.2 Coolant ﬂow optimization

Compare to the fuel rods design in LWRs, the pebble bed geometry in the FHRs may cause higher pressure drop, which would require higher primary pump power, reduces natural circulation performance at the passive safety mode, and increases salt level swelling at free surfaces. To mitigate this shortcoming, a unique annular geometry is incorporated in the Mk1 core design. In addition to the bottom inlet and the top outlet that exist in most of the nuclear reactors, which result in mainly axial coolant ﬂow with local turbulence caused by various ﬂow mixing structures, the annular core allows inlet and outlet openings in the center and outer reﬂectors, as shown in ﬁgure 4.15 and forms cross ﬂow pattern in the core. The annular geometry of Mk1 provides a lot of design ﬂexibility in terms of inlet and outlet opening locations and coolant ﬂow boundary conditions, with the goal to achieve optimal cooling while controlling the pressure drop across the core.

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 87

Figure 4.15: Schematic of Mk1 core inlet and outlet locations

An example ﬂow ﬁeld with pressure distribution is shown in ﬁgure 4.16. We observe that the isobars are denser in the sections where the ﬂow is mainly axial. This indicates that the total pressure drop across the core can be reduced by opening up the ﬂow channels in the middle of the center and outer reﬂectors that enhances radial ﬂow. In fact, results have shown that by opening the middle inlets and outlets while keeping the pressure boundary condition ﬁxed, the ﬂow rate increases signiﬁcantly with the inlet and outlet widths. In the range that is studied, opening either outlet or inlet has similar effect on the ﬂow rate, about 62 kg/s increase with 1cm increase of outlet or 54 kg/s with 1 cm of inlet. With this trend, it is feasible to keep the pressure drop at 1 m head loss with reasonable opening width.

Another important parameter to consider during optimization is the temperature as the primary function of coolant is to transport the heat from the fuel elements to the primary loop and ultimately to the secondary loop and beyond for power conversion. The ﬂow pattern directly affects the local heat transfer properties, represented in the models by the convective heat transfer coefﬁcient. Therefore one goal for ﬂow optimization is to keep the peak fuel and coolant temperatures below their respective safety limits.

The nominal average fuel temperature in a Mk1 core is much lower than the safety limit due to the excellent thermal resistance of the TRISO type fuel. Although the fuel elements in FHRs are

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 88

Figure 4.16: Steady state coolant ﬂow streamline and pressure distribution. (Color bar legend for velocity, Isobar proﬁle in black.)

highly thermal resistant so that the peak fuel temperature is not the primary concern in a FHR core, the fuel temperature as a safety design criteria needs to be monitored in all events. The fuel temperature is determined by three factors, the power density, the local coolant temperature, and the particle to ﬂuid heat transfer coefﬁcient. Begin able to bring cold coolant up in the channels in the center reﬂector and inject to higher locations where the fuel is hot is a unique heat transfer enhancement design advantage in the Mk1 FHR core. The particle-to-coolant heat transfer coefﬁcient in a pebble bed core is dictated by the Prandtl number, an inherent properties of the salt, and the Reynolds number, which is a dimensionless number that characterizes the ﬂow regime. Figure 4.17 shows the heat transfer coefﬁcient distribution side-by-side with the Reynolds number. Only limited heat transfer occurs between the solid fuel element and the coolant where the Reynolds number approaches zero. Therefore, avoiding local hot spot in fuel temperature, which can be a result of dead-zones in the ﬂow is the key in ﬂow optimization with respect to peak fuel temperature.

The peak coolant temperature in the core is also important to damage to metallic structures in the outlet plenum and to prevent boiling, although boiling is not the limiting design criteria for

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 89

Figure 4.17: Steady state local convective heat transfer coefﬁcient, computed from Wakao correlation, and the Reynolds number

Mk1 as the coolant in Mk1 has very high boiling point. At nominal operating condition, 976 kg coolant enters the core every second at 600 ◦C and the bulk average outlet coolant temperature is 713 ◦C much lower than the ﬂibe boiling point at atmospheric pressure. In terms of temperature limit, the smallest thermal margin in the core is in the coolant outlet temperature, because the outlet coolant temperature directly affects the hot leg metallic structures, which is more vulnerable to thermal degradation than the fuel elements and graphite blocks in the core. In addition to the average outlet temperatures, the uniformity of the outlet coolant temperature is also important to extend the lifetime of the structural material at the exit of the core. Turbulent mixing of the coolant in the upper plenum is often incomplete, causing temperature ﬂuctuation and gradient in the hot leg of the main circulation pipe that can cause thermal fatigue.

The spread in coolant temperature around the average can be controlled by careful design of the inlet coolant velocity proﬁle that achieves a similar temperature increase along all the streamlines.

To study the effects of inlet boundary condition on the temperature criteria, we have tested three different inlet velocity proﬁles. The inlet velocity proﬁle at the center reﬂector is shown

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 90

Figure 4.18: Center reﬂector coolant injection velocity boundary condition

in 4.18. All these have 70% of the total inlet routed through the center reﬂector and 30% from the bottom inlet (not shown in the ﬁgure). To achieve the ﬂexibility in ﬁne-tuning the inlet injection design, a parabolic function is used to model the main center inlet (inlet 1 in ﬁgure 4.15). The ﬁrst one allows coolant injection at only limited heights. The second one is optimized for cooling efﬁciency, where the coolant injection is more evenly distributed with the highest ﬂow rate near the middle of the core where the ﬂow path between inlet and outlet is the longest and the power density is the highest, for better cooling efﬁciency. However, in practice, the ability to bring the coolant ﬂow up to the top is limited by the cross-sections area of the ﬂow channels in the center reﬂector. Thus, a bottom heavy inlet condition is more realistic.

Figure 4.19 shows the temperature of the coolant as it ﬂows through the core. Figure 4.20 shows TRISO kernel temperature distribution in the core. And histograms in ﬁgure 4.21 show the distribution of outlet coolant temperature under the different inlet conditions. With limited opening in the middle, there is a large variation in the coolant outlet temperature. In particular, the top of the core is overheated due to limited coolant ﬂow in that area, where the inlet is closed. In the contrary, with improved inlet design, the coolant can efﬁciently cover all the areas in the core and provide sufﬁcient cooling to the fuel. Although the power peaks at the center (ﬁgure 4.11), the fuel and the coolant temperatures are low in the center because of the inlet coolant along the center reﬂector. The peak temperature for both phase is found near the outlet. Under

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 91

the bottom heavy inlet condition, most of the coolant exits the core with a temperature between 680 ◦C and 720 ◦C . And peak outlet coolant are below 770 ◦C , this allows a thermal margin to structural material damage.

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 92

(a) Limited opening (b) Even inlet

(c) Bottom heavy inlet

Figure 4.19: Steady state coolant temperature distribution and streamline under different inlet conditions

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 93

(a) Limited opening (b) Even inlet

(c) Bottom heavy inlet

Figure 4.20: Steady state fuel temperature distribution and streamline

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 94

(a) Limited opening (b) Even inlet

(c) Bottom heavy inlet

Figure 4.21: Histogram of coolant outlet temperature (weighted by the surface area)

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 95

Figure 4.22: Full core power level during an overcooling transient

### 4.5 Transient results

The coupled thermal-hydraulics and neutronics model (4.3) can be used to simulate 3 dimensional transient scenarios that does not alter the power conversion system. To demonstrate this capability, two classes of transient scenarios are simulated here, including over-cooling transients and reactivity insertion (RI) transients.

#### 4.5.1 Overcooling transients

An over-cooling transient happens when colder coolant introduces positive reactivity to the core via temperature reactivity feedback and causes power to increase (shown in ﬁgure 4.22). In this study, a 100◦C drop in coolant inlet temperature in 10 seconds in a core with fresh fuel and without inserted control rods is simulated as an example case.

The change in fuel temperature is determined by local power level, which is increased, and coolant temperature, which might increase due to the higher power level or decrease due to the colder inlet. Figure 4.23 shows the core-wise peak fuel temperature. It decreases slightly at the begining and increase to almost 1150 ◦C .

The coolant outlet temperature is shown in ﬁgure 4.24. It remains constant until the changes propagate through the core. It increases at ﬁrst due to the raised power rates and thus the increased heat ﬂux transferred from the fuel, and decreases when the cold coolant arrives at the outlet.

In a ATWS transient, temperature feedback is the main mechanisms to stablize the reactor. The drop in core average ﬂibe temprature is compansated by the increase in fuel temperature. Due to the larger Doppler feedback, the increase in fuel temperature is smaller than the decrease in coolant inlet temperature or in the average coolant temperature.

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 96

Figure 4.23: Maximum fuel temperature during an overcooling transient

Figure 4.24: Average outlet coolant temperature during an overcooling transient

#### 4.5.2 Control rod removal transients

Control rod ejection caused reactivity insertion is a common concern in LWRs, where the rapid removal of a control element from the pressurized system would cause a rapid increase in local and global power and temperature and may therefore cause critical damage to core integrity. However control rod ejection is not possible in FHR cores due to its low operating pressure. A control rod removal accident in a FHR would be more likely, although still very rare, to be caused by multiple failures of the reactor control system and thus the speed of the control rod removal would be limited by the designed maximum speed of the control rod lifting system.

Control rods are partially inserted to the core during normal operation for compensating excess reactivity, which is designed to ensure ﬂexibility in operation. To ﬁnd the initial control rods position for the rod removal study, the control rods are inserted symmetrically to the height where the reactor would have 3941 pcm excess reactivity if all rods are removed. The transient is then triggered by removing three of the eight control rods from the core, illustrated in ﬁgure 4.26.

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 97

Figure 4.25: Average fuel and coolant temperature during an overcooling transient

Figure 4.26: Illustration of removed control rods during the control rods removal transient

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 98

Figure 4.27: Full core power during a control rod removal transient

Figure 4.28: Average output ﬂibe temperatures during a control rod removal transient

Figure 4.29: Maximum fuel temperatures during a control rod removal transient

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 99

Because FHR cores are not under high pressure, so the speed of extraction would be limited by the control rods lifting machinery. Although the detailed design for the lifting system is not yet available, prompt removal is simulated in this study to provide a bounding case for safety analysis. A parametric study on the effect of control rod lifting speed can provide guidance on how fast should we design the control rod lifting system.

Figure 4.27 shows the full core power during the transient. The power increases at ﬁrst due to the inserted reactivity and stabilize at a slightly higher level due to temperature feedback from the fuel elements and the coolant. The peak power during this transient is about 30% above the initial power. Figure 4.28 shows the average coolant outlet temperature weighted by the ﬂow. This parameter reﬂects the coolant temperature when it arrives at the hot leg pipes after efﬁcient mixing. The ﬂow weighted average coolant outlet temperature represents the coolant temperature at the hotleg after mixing in the upper plenum. It is increased by about 35 ◦C and remains below 750 ◦C during this simulated transient. Figure 4.29 shows the maximum fuel temperature, i.e. the temperature of the center of the fuel kernel at the hottest location in the core. It is far below the safety limit during this transient.

#### 4.5.3 Seismic reactivity transients in Mk1 core

A special class of reactivity related transients for pebble-bed FHRs is the pebble densiﬁcation during an earthquake. The safe shutdown earthquake event has been identiﬁed as a Design Basis Accident (DBA) in the licensing basis event selection for the Pebble Bed Modular Reactor (PBMR) and in the Chinese HTR-10 certiﬁcation [36]. To meet licensing requirements and to optimize the design under DBAs, this type of transient is investigated for Mk1 pebble bed reactor design.

The seismic wave causes movement of the fuel pebbles, increases the packing fraction of fuel pebbles and cause transients in the core. The density of the Mk1 fuel pebbles are adjusted so that the pebbles are buoyant in the coolant. So unlike helium cooled pebble bed reactors, the pebbles in a Mk1 core would rise towards the top of the core during an earthquake due to the motion caused by the shaking of the vessel. This results in fuel densiﬁcation toward the top and leaves the bottom of the core with only coolant. The Mk1 control rods are inserted from the top, therefore, they remain effective and may actually provide negative reactivity during this transient.

Although the exact magnitude of the compression and the timescale depends on the earthquake and needs to be determined by discrete element modeling and shake table studies, we can investigate the effect of pebble compression on core neutronics by comparing the core reactivity before and after a postulated increase in packing fraction. The impact of changes in the coolant passage geometry on coolant ﬂow is not considered in this study. We have compared the multiplication factor of the core under different pebble packing fraction, between the nominal operating packing fraction 60% and the maximum attainable pebble packing fraction due

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 100

to earthquake induced shaking - 64% ([34][6]). It is worth noting that the resulting reactivity change in a Mk1 core during an earthquake depend on the initiation point. Fuel composition, temperature, power level, Xenon concentration, etc. all play a role in determining the coolant reactivity feedback coefﬁcient, which is one of the main parameters that drives the reactivity change. In order to ensure the safety design of a Mk1 core, one would need a comprehensive study of all possible scenarios and optimize the design with respect to the bounding case. Two example cases were studied here, for a core with equilibrium fuel composition or with fresh fuel, and the change in keff with respect to the nominal packing fraction (60%) is listed in table 4.14 and table 4.15 respectively. The results show that the core reactivity decreased by a small amount after the increase in packing fraction if the core is loaded with equilibrium fuel, whereas the reactivity increase slightly after densiﬁcation of the fuel if the core is loaded with fresh fuel. In fact, the Mk1 core is optimized to have negative coolant void feedback at nominal conditions with equilibrium fuel composition. Thus if a postulated earthquake occurs at nominal condition, the reactivity would decrease due to ’loss’ of coolant in the fuel pebble region. And in the contrary, the core may have slightly positive coolant void feedback with fresh fuel and thus the increase in reactivity when the fuel pebbles are compressed. Overall, the magnitude of the changes is small in both cases.

Packing fraction ∆keff * [pcm] uncertainty** [pcm] 61% - 24 1.2 62% - 52 1.2 64% - 236 1.2 Table 4.14: Change in keff during an earthquake with different amount of fuel densiﬁcation - equilibrium fuel (* ∆ keff = keff d enser - keff 60%, **uncertainty: uncertainty associated to Monte Carlo computation)

Packing fraction ∆keff * [pcm] uncertainty** [pcm] 61% 99 1.5 62% 176 1.5 64% 186 1.5 Table 4.15: Change in keff during an earthquake with different amount of fuel densiﬁcation - fresh fuel (* ∆ keff = keff d enser - keff 60%, **uncertainty: uncertainty associated to Monte Carlo computation)

### 4.6 Conclusion

The Mk1 PB-FHR core is modeled in this chapter with Monte Carlo neutronics models and coupled neutronics/ thermal-hydraulics models. Features in the coupled thermal-hydraulics and neutronics model, such as control rod model, were compared with Monte Carlo model and the results on keff matches well. Although multi-group diffusion model cannot compute local neutronics parameters accurately at the vicinity of the control rods, but it captures the overall effect on core reactivity and power distribution, which is the most important parameter during a transient study.

CHAPTER 4. MARK-1 PB-FHR CORE ANALYSIS 101

This unique annular geometry of the fuel region enables a lot of design ﬂexibility in coolant ﬂow optimization. The model was used to optimize the design of ﬂow injection and suction to achieve efﬁcient cooling in the core with limited pressure drop. Hydrodynamic studies have shown that the pressure loss in the core can be reduced signiﬁcantly by opening ﬂow channels in the center and outer reﬂectors. Allowing injection from the entire surface of the center reﬂector improves the heat transfer and avoids local hot spot.

The coupled model was used to simulate the core behaviour during overcooling transients and reactivity insertion transients. Because of the continuous refueling strategy, the excess reactivity can be kept low in Mk1. Also the fuel temperature coefﬁcient in a graphite moderated reactor like Mk1 is more negative than LWRs. These characteristics all reduce the severity of reactivity accidents. Plus, the use of thermal resistant coolant and fuel material makes the core robust to transients.

A transient can happen at any time during a continuous fueling cycle. Likely, these can happen at any state the reactor is in, whether it is in normal operation conditions or zero-power states where the core is maintained at minimum design temperature with very low power. An exhaustive study of all scenarios would be unfeasible and in this case a PIRT for Mk1 core would provide valuable guidance in ﬁnding bounding cases for the safety analysis. 102

Chapter 5

Conclusions and future work

Advances in computer abilities, intense competition on the energy market and stringent regulatory requirements during the last decade have spurred the development of robust numerical models to support nuclear reactor safety analysis and design optimization. This dissertation aims to develop methodology for numerical modeling of Pebble-Bed Fluoride Salt Cooled Reactors(PB-FHR), a novel reactor design that combines TRISO particle fuel and ﬂibe salt coolant. The use of a large number of fuel pebbles in PB-FHR cores poses great challenge in computational cost and violates the assumptions in most of the traditional deterministic codes developed for LWRs. This project developed dedicated models with different levels of spatial and energy resolution for the broad need in FHR design and analysis, including coupled heat diffusion and point kinetics unit cell models, Monte Carlo neutronics models, and coupled multigroup neutron diffusion and multi-scale porous media model. Documentation, version control, testing, veriﬁcation are all indispensable parts of numerical modeling software development. These steps are followed meticulously to ensure high quality open source codes that promote open science and reproducible research.

Unit cell models can compute representative fuel and coolant temperatures and full core power within a short amount of time and thus is adequate for scoping analysis, or uncertainty and sensitivity analysis, where a large number of runs is required to cover the input space. FHRs have substantial graphite reﬂectors that slows down the neutron generation time by hosting neutrons while they get thermalized. A ’multi-point’ correction is derived from perturbation theory to take the reﬂector effects on reactivity and neutron generation time into account.

Monte Carlo based codes, on the other extreme, can provide accurate results with minimal assumptions, but they are typically only used for generating cross-sections for deterministic models, for example point kinetics models and multi-group neutron diffusion models in this project, or to provide reference results for benchmarking lower resolution codes due to high computational cost. PB-FHR has not yet test reactors to provide experimental data for tool validation. High ﬁdelity models based on direct coupling between neutron transport and Computational

CHAPTER 5. CONCLUSIONS AND FUTURE WORK 103

Fluid-Dynamics (CFD) was used in this project as reference for code-to-code veriﬁcation.

Multi-group neutron diffusion models were developed for design optimization and safety analysis, because they are compatible with current computation resources in nuclear industry, with which simulations can be carried out on stand-alone workstations or small computation clusters within a reasonable time. For areas where diffusion assumption is limited, e.g. in vicinity of control rods, the simpliﬁed spherical harmonics equations were implemented to improve the accuracy of diffusion equation by relaxing the isotropic assumption in neutron transport. The neutronics model is coupled to a porous media CFD module with multi-scale treatment that computes conductive heat transfer inside TRISO particles and fuel pebbles, as well as convective heat transfer between fuel pebbles and coolant. Radiative heat transfer may be signiﬁcant in the high temperature reactors, but is not modeled in this project due to lack of material properties measurement.

Results from the coupled full core model were veriﬁed with analytical solutions when they exist, for example the steady state outlet bulk average temperature, computed from energy balance, or a reference Monte Carlo model. Although the absolute value of the multiplication factor can not be accurately matched between the Monte Carlo statistics and the eigenvalue found from the coupled model, this can be corrected and more important for transient modeling is the change in the multiplication factor during a transient. In fact, the important parameters for a transient study, such as ﬂibe and fuel temperature feedback coefﬁcients, time scale, and control rod worth matches well between the coupled model and the reference model.

The multiphysics models are capable of simulating both steady state and a broad spectrum of transient scenarios, which involves either coolant inlet condition or reactivity induced transients, especially Anticipated Transient Without Scram (ATWS). Although ATWS transients, as beyond design events, are mandatory in only a small number of countries, in contract to the generic safety analysis that is required in almost all countries. However, understanding and credible demonstration of safety for these scenarios is beneﬁcial to improve conﬁdence in safety margins, ensure regulators and attract investors. For transients that affects components outside of the core, coupling to system code is desired in the future to update the core boundary condition over the course of the transient.

The methodology was applied to the TMSR SF-1(in chapter 3) and Mk1 (in chapter 4), which are both FHRs that uses TRISO fuel particles in spherical fuel elements and Fluoride salt coolant. For both designs, steady state power, neutron ﬂux, and temperature distributions were computed, and reactivity insertion and overcooling transients were simulated. The study shows that FHR cores are extremely resilient to the investigated transient scenarios, for the following reasons:

• The graphite based fuel elements in FHRs can withstand up to 1600 ◦C without risk of

CHAPTER 5. CONCLUSIONS AND FUTURE WORK 104

radioactive release. And the ﬂibe coolant used in FHRs also has high thermal resistance, with a boiling point at 1430 ◦C . Due to the large thermal margin to the failure of TRISO particles, the thermal constraints will be more likely in the metallic structures. These temperature limits should be deﬁned based on both the temperature and the time of exposure for creep limits or reduced yield stress limits at elevated temperatures.

• In FHRs, the role of coolant and moderator are separated. The Doppler feedback, the main mechanism to stabilize the reactor during an ATWS, is more negative than in LWRs.

• Online refueling fuel management regime allows the reactor to operate at a low excess of reactivity because the reserve for compensating fuel burnup is small in this case.

When building numerical models, we often face the question of which value we should use for some input parameters. Inputs for FHR models are uncertain due to lack of knowledge or intrinsic variations, including difﬁculties in measuring thermo-physical properties and nuclear data, ﬂexibility in design, material changes during operation due to radiation, fatigue, and ambient temperature, geometry change due to thermal expansion and radiation induced swelling, and etc. Correlations are often used in thermal-hydraulics modeling, and they introduces uncertainties from approximating a complex ﬂow phenomena by a empirical relationship of characteristic dimensionless numbers. Uncertainty quantiﬁcation and sensitivity analysis (UQSA) needs emerge when the inputs of a numerical model are not known with certainty. Uncertainty analysis characterizes the spread of the response function and sensitivity studies compute the contribution of an individual parameter on the uncertainty. This aligns with the shift in modern nuclear reactor modeling philosophy, from conservative assumptions to best estimate with uncertainty quantiﬁcation methods.

Our discussion has been focused on FHRs, however, the methodology can also be used for other types of reactors with minor modiﬁcations. If the conservation equations for compressible ﬂuid are used, the models can be used for Helium cooled pebble bed reactors. 105

Appendix A

Serpent input generator example

This section shows how to use the Serpent input generator library with a simple example. More elaborated uses of the FIG package can be found at https://github.com/xwa9860/FIG, including the models that are used in this thesis.

A Python script to generate a Serpent input, which deﬁnes an inﬁnite fuel medium with the pebbles in a FCC packing structure, is shown below:

#!/usr/bin/python from FIG.pb import FPb from FIG.triso import Triso from FIG.mat import Fuel from FIG.coolant import Coolant

fuel_input = 'fuel_mat1' fuel = Fuel(900, 'fuel_name1', fuel_input) TEMP = [800.0, 900.0, 1000.0, 800.0, 800.0] tr = Triso(TEMP, fuel) pb = FPb(tr, 900.0, 870.0) cool = Coolant(800.0)

# write output to file f = open('pb', 'w+') f.write(pb.generate_output()) for mat in pb.mat_list: f.write(mat.generate_output()) f.write(cool.generate_output())

APPENDIX A. SERPENT INPUT GENERATOR EXAMPLE 106

f.write(cool.mat_list[0].generate_output())

From this, a Serpent input ﬁle like the following will be generated.

%%---Fuel pebble cell 8 10 CG900 -1 cell 9 10 fill 9 1 -2 %---Triso particle particle 8 fuel_name1 0.0200 Buffer800 0.0300 iPyC900 0.0335 SiC1000 0.0370 oPyC800 0.0405 Matrix800 %%---Triso lattice lat 9 6 0. 0. 0.08860629 8 cell 10 10 Shell870 2

mat CG900 -1.594 moder grph_CG900 6000 tmp 900 rgb 255 75 134 %graphite core in fuel pebble 6000.09c 1.0 therm grph_CG900 gre7.20t

mat Shell870 -1.75 moder grph_Shell870 6000 tmp 870 rgb 255 75 134 %Graphite shell(outermost layer of fuel pebble) 6000.06c 1.0 therm grph_Shell870 gre7.20t

mat fuel_name1 -10.5 tmp 900 rgb 255 75 134 92235.09c 19.9 92238.09c 80.1 12000.09c 150.0 8016.09c 100.0

mat Buffer800 -1.05 moder grph_Buffer800 6000 tmp 800 %Buffer layer in triso particle 6000.06c 5.26449E-02 therm grph_Buffer800 gre7.18t

APPENDIX A. SERPENT INPUT GENERATOR EXAMPLE 107

mat iPyC900 -1.9 moder grph_iPyC900 6000 tmp 900 %inner pyrocarbon layer in triso particle 6000.09c 9.52621E-02 therm grph_iPyC900 gre7.20t

mat SiC1000 -3.18 tmp 1000 %silicon carbon layer in triso particle 6000.09c 4.7724E-02 14028.09c 4.77240E-02

mat oPyC800 -1.9 moder grph_oPyC800 6000 tmp 800 %outer pyrocarbon layer in triso particle 6000.06c 9.52621E-02 therm grph_oPyC800 gre7.18t

mat Matrix800 -1.704 moder grph_Matrix800 6000 tmp 800 rgb 255 75 134 %matrix in triso particle 6000.06c 8.77414E-02 5010.06c 9.64977E-09 5011.06c 3.90864E-08 therm grph_Matrix800 gre7.18t

%%---Coolant surf 4 inf cell 11 11 Flibe800 -4

mat Flibe800 -2.023 tmp 800 rgb 0 181 238 3006.06c 2.45846e-05 3007.06c 1.99998e+00 4009.06c 1.00000e+00 9019.06c 4.00000e+00 108

Appendix B

FHR core components material thermo-physical properties

This appendix lists thermo-physical properties of the materials used in a FHR core.

B.1 Fuel elements

In PB-FHRs, TRISO particles is composed with ﬁve layers. Their respective thermal properties are listed in table B.1[1]. However, most of the time, the complex geometry makes it impractical to model each TRISO particles explicitly, in this case, the fuel layer is modeled as a homogeneous material with equivalent thermal properties, as shown in table B.2.

Region Density Conductivity Speciﬁc heat [g/cc] [W/m.K] [J/kg/K]

Kernel 10.5 3.5 400 Buffer 1.0 0.5 2000 IPyC 1.9 4.0 2000 SiC 3.2 30 1300 OPyC 1.9 4.0 2000 Coating* 1.89 0.76 1736 Table B.1: Nominal material properties of TMSR fuel element used in the models [17][14][20]. *Coating combines all the non-power-generating layers in a TRISO particle.

APPENDIX B. FHR CORE COMPONENTS MATERIAL THERMO-PHYSICAL PROPERTIES 109

Region Density Conductivity Speciﬁc heat [g/cc] [W/m.K] [J/kg/K]

Fuel 1.81 15 1744 Shell 1.96 193 684 Pebble 1.85 17 1700 Table B.2: Nominal material properties of TMSR fuel pebble used in the models [17][14][20]. *Pebble: pebble-wise material properties

If further simpliﬁcation is needed, the pebble-wise thermal conductivity and speciﬁc heat capacity data are determined by the correlations B.1 and B.2.

Pebble wise thermal conductivity [20]:

Figure B.1: Pebble-wise equivalent thermal conductivity as a function of temperature and irradiation dose in FHR fuel λ = 1.2768 ∗ ( 0.6829 − 0.3906 ∗ 10−4T d osi s + 1.931 ∗ 10−4T + 1.228 ∗ 10−4T + 0.042) (B.1)

where λ is in the unit of W/cm/K, T is the temperature in degree Celsius(T<1200◦C ), and dosis is the fast neutron irradiation dose in 1021. As shown in ﬁgure B.2, the conductivity varies between

APPENDIX B. FHR CORE COMPONENTS MATERIAL THERMO-PHYSICAL PROPERTIES 110

17 W/m/K to more than 60 W/m/k depend on the irradiation damage and temperature. So 15 W/m/K is often used as a conservative value when constant fuel conductivity is used for simplicity.

Pebble-wise equivalent speciﬁc heat capacity [20]:

Figure B.2: Pebble-wise speciﬁc heat as a function of temperature and irradiation dose in FHR fuel

C p = 1.75 ∗ (0.645 + 3.14 ∗ 10−3T − 2.809 ∗ 10−6T 2 + 0.959 ∗ 10−9T 3)/ρ (B.2)

where Cp is in the unit of J/kg/K, ρ is the fuel density in kg /cm3 and T is the fuel temperature in degree celsius (T<1200◦C ). The dependence of the speciﬁc heat capacity on the fuel temperature is plotted in ﬁgure B.2. The value varies from 1425 J/kg/K to 1675 J/kg/K for a temperature range of [600, 1200] ◦C . The value at the nominal fuel temperature(900◦C ) is 1564 J/kg/K.

B.2 Coolant

Flibe salt remains in the liquid phase under FHR operation conditions and the transient conditions we studies in this project. Recommended temperature dependent properties and uncertainties of the ﬂibe salt in [37] are used in this work and are listed in the following table.

APPENDIX B. FHR CORE COMPONENTS MATERIAL THERMO-PHYSICAL PROPERTIES 111

Table B.3: Flibe salt thermo-physical properties and uncertainty with 95% conﬁdence

Property Value Uncertainty Melting point, ◦C 459 Boiling point, ◦C 1430 Viscosity, kg/m.s 1.16E-4*e3755/T [K ] 20% Heat capacity, J/kg/K 2386 3% Thermal conductivity, W/m/K 1.1 10% Density, kg/m3 2413 - 0.488T[K] 2% 112

Appendix C

Mk1 core Monte Carlo model geometry and material speciﬁcations

This appendix describes the dimensions of the components in the reference Mk1 Monte Carlo model. Entrance region Bottom height(cm) 41.60 Outer radius (cm) 45.00

Divergence region Bottom height(cm) 127.50 Angle of expansion(◦) 30.00

Active region Bottom height(inner)(cm) 144.82 Outer radius(cm) 35.00

Converging region Bottom height(cm) 430.50 Outer radius(cm) 71 Angle of converging(◦) 30

Defueling region Bottom height(cm) 492.85 Top height(cm) 572.85 Outer radius(cm) 71

Table C.1: Dimensions of Mk1 center reﬂector in the Monte Carlo model

APPENDIX C. MK1 CORE MONTE CARLO MODEL GEOMETRY AND MATERIAL SPECIFICATIONS 113

Entrance region Bottom height 41.60 Inner radius(cm) 45.00 Outer radius(cm) 75.41

Divergence region Bottom height(inner)(cm) 127.50 Bottom height(outer)(cm) 112.50 Angle of expansion(inner) 30.00 Angle of expansion(outer) 30.00

Active region Bottom height(inner)(cm) 144.82 Bottom height(outer)(cm) 180.50 Outer radius(cm) 105.00 Inner radius(cm) 35.00

Converging region Bottom height(cm) 430.50 Inner radius at the top(cm) 71 Angle of converging(inner) 30 Angle of converging(outer) 30

Defueling region Bottom height(cm) 492.85 Top height(cm) 572.85 Inner radius(cm) 71 Outer radius(cm) 80.00

Table C.2: Dimension of Mk1 fuel region in the Monte Carlo model

APPENDIX C. MK1 CORE MONTE CARLO MODEL GEOMETRY AND MATERIAL SPECIFICATIONS 114

Entrance region Bottom height 41.60 Inner radius(cm) 75.41 Outer radius(cm) 85.74

Divergence region Bottom height(cm) 112.50 Angle of expansion(inner) 30.00

Active region Bottom height(cm) 180.50 Inner radius(cm) 105.00 Outer radius(cm) 125.00

Converging region Bottom height(cm) 430.50 Angle of converging(inner) 30

Defueling region Bottom height(cm) 492.85 Top height(cm) 572.85 Inner radius(cm) 80.00 Outer radius(cm) 89.00 Table C.3: Dimension of blanket pebble region in Mk1 core

Entrance region Bottom height 41.60 Inner radius(cm) 85.74

Divergence region Bottom height(outer2)(cm) 112.50 Angle of expansion(inner) 30.00

Active, converging and defueling Bottom height(cm) 144.82 Top height(cm) 572.85 Inner radius(cm) 71 Outer radius OR of outer reﬂector(cm) 165 Table C.4: Dimension of Mk1 outer reﬂector in the Monte Carlo model 115

Appendix D

Implementing the diffusion and SPN algorithms via the ’user deﬁned PDEs’ interface in COMSOL

The COMSOL Multiphysics does not have built-in neutronics modules, but fortunately it allows users to implement neutronics equations via its ’General Form PDE’ interface and solve them in a fully coupled way with built-in physics. This appendix serves as a tutorial on how to implement neutronics equations in COMSOL, including best practice advices and potential pitfalls for future users.

COMSOL allows user to deﬁne custome PDEs in the following general form

λ 2eau − λdau + ∇(−c∇u − αu + γ) + β∇u + au = f (D.1)

for eigenvalue study, and

ea ∂2u ∂t 2 + da d u d t + ∇(−c∇u − αu + γ) + β∇u + au = f (D.2)

for transient study, by providing the values for c (the diffusion coefﬁcient), a (the absorption coefﬁcient), f (the source term), da (the damping or mass coefﬁcient), α (the conservative ﬂux convection coefﬁcient), β (the convection coefﬁcient), and γ (the conservative ﬂux source). Note: The eigenvalue λ that COMSOL solves for is the inverse of the multiplication factor keff .

Most of the time, the geometry, material properties, boundary conditions and heat sources in the reactor core are axisymmetric. Cylindrical coordinates are in these cases useful for efﬁciently solving and postprocessing rotationally symmetric problems. Signiﬁcant savings can be made for both memory and time by using 2D meshes instead of 3D ones. The COMSOL Multiphysics software has built-in support for cylindrical coordinates in the axisymmetry physics

APPENDIX D. IMPLEMENTING THE DIFFUSION AND SPN ALGORITHMS VIA THE ’USER DEFINED PDES’ INTERFACE IN COMSOL 116

interfaces. However, special attention must be paid to the deﬁnition of the ∇ operator in two dimensional axisymmetric case in COMSOL. The divergence operation is commonly deﬁned as in equation D.5 in cartisian coordinates and as in equation D.4 in cylindrical coordinates. However, the PDE interface in COMSOL treats the partial differentiation operators in ∇(c∇U ) as [d/dx+d/dy] in cartisian coordinate and [d/dr+d/dz] in cylindrical coordinate. This does not pose any problems in cartisian coordinate but is different from the divergence operation in cylindrical system. The user must account for this transformation when inputing costom PDEs.

∇(c∇U )c yl = 1 r ∂ ∂r (r c ∂U ∂r ) + 1 r ∂ ∂φ ( c r ∂U ∂φ ) + ∂ ∂z (c ∂U ∂z ) (D.3)

∇(c∇U )c yl ,COM SOL = ∂ ∂r (c ∂U ∂r ) + ∂ ∂φ (c ∂U ∂φ ) + ∂ ∂z (c ∂U ∂z ) (D.4)

∇(c∇U )car t = ∂ ∂x (c ∂U ∂x ) + ∂ ∂y (c ∂U ∂y ) + ∂ ∂z (c ∂U ∂z ) (D.5)

D.1 Diffusion equations

As discussed in chapter 2, the neutron diffusion equation system can be written as the following: 1 vg ∂φg ∂t =∇D g ∇φg − Σt ,g φg − ∑

g ′̸=g Σs,g g ′φg + (D.6)

(1 − β)χp,g G∑

g ′=1(νΣ f )g ′φg ′ + D∑

i =1 χd ,g λi ci

and the delayed neutron precursor concentration is

∂ci ∂t = − λi ci + βi G∑

g =1(νΣ f )g φg (D.7)

This PDE system can be input into COMSOL by deﬁning the coefﬁcients as in equations D.8,

D.9, D.10, D.11 with the unknown variable vector u = 

          F l ux1 F l ux2 · · · F l ux8 C onc1 · · · C onc6 

         

. And the ’Livelink for Matlab’

APPENDIX D. IMPLEMENTING THE DIFFUSION AND SPN ALGORITHMS VIA THE ’USER DEFINED PDES’ INTERFACE IN COMSOL 117

module enables us to ﬂexibly automate the process for an arbitrary number of energy groups.

c = [ d i ag (D1, · · · , D8) ∗ r ] (D.8)

a = 

    (Σr 1 − (1 − β)χp1νΣ f 1λ)r (−Σs21 − (1 − β)χp1νΣ f 2λ)r · · · (−Σs81 − (1 − β)χp1νΣ f 8λ)r (−Σs12 − (1 − β)χp2νΣ f 1λ)r (Σr 2 − (1 − β)χp2νΣ f 2λ)r · · · (−Σs82 − (1 − β)χp2νΣ f 8λ)r · · · · · · · · · · · · (−Σs18 − (1 − β)χp8νΣ f 1λ)r (−Σs28 − (1 − β)χp8νΣ f 2λ)r · · · (Σr 8 − (1 − β)χp8νΣ f 8λ)r 

   

(D.9)

f = 0 (D.10)

The ’da’ matrix is different between eigenvalue study and transient or steady state study, so a Boolean variable ’eig’ is added in the formula to switch between different modes of studies.

d a = d i ag (1/V1ei g , 1/V2ei g , · · · , 1/V8ei g , ei g , · · · , ei g ) (D.11)

D.2 SP3 equations

The SP3 equations are written as:

− ∇D1∇(φ0 + 2φ2) + Σr φ0 = S0 − ∇D2∇φ2 + Σt φ2 = 2/5(Σr φ0 − S0) (D.12)

The dependent variable with all the unknown parameters is u = 

                   F l ux1 F l ux2 · · · F l ux8 F l ux21 F l ux22 · · · F l ux28 C onc1 · · · C onc6 

                  

APPENDIX D. IMPLEMENTING THE DIFFUSION AND SPN ALGORITHMS VIA THE ’USER DEFINED PDES’ INTERFACE IN COMSOL 118

By setting the coefﬁcients to the forms in equation D.13, D.14, D.16 and D.17, the sp3 equations with delayed neutrons are implemented.

c = 

 d i ag (D1, · · · , D8) ∗ r d i ag (2D1, · · · , 2D8) ∗ r 0 0 d i ag (D21, · · · , D28) ∗ r 0 0 0 0 

 (D.13)

a = 

 B 0 0 −2/5B d i ag (Σt 1r, · · · , Σt 8r ) 0 0 0 d i ag (λd 1, · · · , λd 6) 

 (D.14)

The matrix block

B = 

    (Σr 1 − (1 − β)χp1νΣ f 1λ)r (−Σs21 − (1 − β)χp1νΣ f 2λ)r · · · (−Σs81 − (1 − β)χp1νΣ f 8λ)r (−Σs12 − (1 − β)χp2νΣ f 1λ)r (Σr 2 − (1 − β)χp2νΣ f 2λ)r · · · (−Σs82 − (1 − β)χp2νΣ f 8λ)r · · · · · · · · · · · · (−Σs18 − (1 − β)χp8νΣ f 1λ)r (−Σs28 − (1 − β)χp8νΣ f 2λ)r · · · (Σr 8 − (1 − β)χp8νΣ f 8λ)r 

   

(D.15)

Σsg ′g is the scattering cross-section from group g’ to group g

f = 0 (D.16)

da matrix is different between eigenvalue study and transient(or steady state) study, so a boolean variable ’eig’ is added in the formular to adapt to different mode of studies.

d a = d i ag (1/V1ei g , 1/V2ei g , · · · , 1/V8ei g , ei g , · · · , ei g ) (D.17)

119

## Bibliography

[1] Andreades et al. “Design summary of the Mark-I Pebble-bed, Fluoride-salt-cooled, Hightemperature reactor commercial power plant”. In: Nuclear Technology ().

[2] Todd Allen et al. Fluoride-Salt-Cooled, High-Temperature Reactor (FHR) Subsystems Deﬁnition, Functional Requirement Deﬁnition, and Licensing Basis Event (LBE) Identiﬁcation White Paper. 2013.

[3] Francesc Alted, Ivan Vilata, et al. PyTables: Hierarchical Datasets in Python. 2002–. URL: http://www.pytables.org/.

[4] Manuele Auﬁero and Massimiliano Fratoni. “Development of Multiphysics Tools for FluorideCooled High-Temperature Reactors”. In: physor. August. 2016.

[5] Bokeh Development Team. Bokeh: Python library for interactive visualization. 2014. URL: http://www.bokeh.pydata.org.

[6] Xingwei Chen et al. “Experimental investigation of the bed structure in liquid salt cooled pebble bed reactor”. In: Nuclear Engineering and Design 331.March 2017 (2018), pp. 24– 31. ISSN: 00295493. DOI: 10.1016/j.nucengdes.2018.02.003. URL: http://linkinghub. elsevier.com/retrieve/pii/S0029549318300773.

[7] Anselmo Tomas Cisneros. “Pebble Bed Reactors Design Optimization Methods and their Application to the Pebble Bed Fluoride Salt Cooled High Temperature Reactor (PB-FHR)”. PhD thesis. University of California, Berkeley, 2013.

[8] Anthony G Dixon and Michiel Nijemeisland. “CFD as a Design Tool for Fixed-Bed Reactors”. In: Industrial and Engineering Chemistry Research 40.23 (2001), pp. 5246–5254.

ISSN: 0888-5885. DOI: 10.1021/ie001035a. URL: http://pubs.acs.org/doi/abs/10. 1021/ie001035a.

[9] J.J. Duderstadt and L.J. Hamilton. Nuclear Reactor Analysis. Wiley, 1976. ISBN: 9780471223634.

URL: https://books.google.com/books?id=R057QgAACAAJ.

[10] Sabri Ergun and A. A. Orning. “Fluid Flow through Randomly Packed Columns and Fluidized Beds”. In: Industrial & Engineering Chemistry 41.6 (1949), pp. 1179–1184. DOI: 10. 1021/ie50474a011.

BIBLIOGRAPHY 120

[11] Charles W. Forsberg, Per F. Peterson, and Paul S. Pickard. “Molten-Salt-Cooled Advanced High-Temperature Reactor for Production of Hydrogen and Electricity”. In: Nuclear Technology 144.3 (2003), pp. 289–302. ISSN: 0029-5450. DOI: 10.13182/NT03-1. URL: https: //www.tandfonline.com/doi/full/10.13182/NT03-1.

[12] Massimiliano Fratoni. “development and applications of methodologies for the neutronic design of the pb-ahtr”. PhD thesis. UC Berkeley, 2007.

[13] E. M. Gelbard. “Simpliﬁed spherical harmonics equations and their use in shielding problems. Technical Report WAPD-T-1182, Bettis Atomic Power Laboratory, 1961. 52, 73”. In: ().

[14] FHR Group. “White Paper of FHR Integrated Research Project Workshop 3”. In: Integrated Research Project Workshop 3 June (2013), pp. 1–166. URL: http://fhr.nuc.berkeley. edu/wp- content/uploads/2013/08/12- 002- FHR- Workshop- 2- Report- Final. pdf%5Cnhttp://fhr.nuc.berkeley.edu/wp-content/uploads/2013/08/12-001FHR- Workshop- 1- Report- Final.pdf%5Cnhttp://fhr.nuc.berkeley.edu/wpcontent/uploads/2013/08/12-004-FHR.

[15] “H. Gerwin and W. Scherer, The Two-Dimensional Reactor Dynamics Programme TINTE. Part1: Basic Principles andMeth- ods of Solution, FZ. Julich, Julich, Germany, 1987.” In: ().

[16] Yassin A Hassan. “LARGE EDDY SIMULATION IN PEBBLE BED GAS COOLED CORE REACTORS”. In: (), pp. 331–346.

[17] Jianwei Hu and Rizwan Uddin. “3D Thermal Modeling of TRISO Fuel Coupled with Neutronic Simulation”. In: International Congress on Advance in Nuclear Power Plants meeting 836 (2010).

[18] Lakshana Ravindranath Huddar. “Heat Transfer in Pebble-Bed Nuclear Reactor Cores Cooled by Fluoride Salts Heat”. PhD thesis. UC Berkeley, 2016.

[19] J. D. Hunter. “Matplotlib: A 2D graphics environment”. In: Computing In Science & Engineering 9.3 (2007), pp. 90–95. DOI: 10.1109/MCSE.2007.55.

[20] IAEA. Heat Transport and Afterheat Removal for Gas Cooled Reactors Under Accident Conditions. Tech. rep. January. 2000.

[21] Frank P. Incropera. Fundamentals of Heat and Mass Transfer. John Wiley & Sons, 2006.

ISBN: 0470088400.

[22] D T Ingersoll et al. Status of Preconceptual Design of the Advanced High-Temperature Reactor ( AHTR ). Tech. rep. May. 2004.

[23] Eric Jones, Travis Oliphant, Pearu Peterson, et al. SciPy: Open source scientiﬁc tools for Python. [Online; accessed <today>]. 2001–. URL: http://www.scipy.org/.

[24] Changwoo Kang. Pressure Drop in a Pebbled Bed Reactor. 2010.

[25] G G Kulikov, A N Shmelev, and V A Apse. “Improving Nuclear Safety of Fast Reactors by Slowing Down Fission Chain Reaction”. In: 2014 (2014).

BIBLIOGRAPHY 121

[26] Edward W. Larsen, Jim E Morel, and John Mcghee. Asymptotic Derivation of the Simpliﬁed PN Equations. Tech. rep. 1993.

[27] Michael Laufer. “Granular Dynamics in Pebble Bed Reactor Cores”. PhD thesis. 2013.

[28] Jung-Jae Lee et al. “Numerical treatment of pebble contact in the ﬂow and heat transfer analysis of a pebble bed reactor core”. In: Nuclear Engineering and Design 237.22 (Nov. 2007), pp. 2183–2196. ISSN: 00295493. DOI: 10.1016/j.nucengdes.2007.03.046.

[29] et al. LeppÃd’nen J. “"The Serpent Monte Carlo code: Status, development and applications in 2013." Ann. Nucl. Energy, 82 (2015) 142-150.” In: ().

[30] MultiMedia LLC. Stainless Steel 316 - Alloy Composition. 2017. URL: http://www.espimetal. com / index . php / technical - data / 202 - Stainless \ %20Steel \ %20316 \ %20 - \ % 20Alloy\%20Composition (visited on 2017).

[31] Vijay S. Mahadevan, Jean C. Ragusa, and Vincent A. Mousseau. “A veriﬁcation exercise in multiphysics simulations for coupled reactor physics calculations”. In: Progress in Nuclear Energy 55 (2012), pp. 12–32. ISSN: 01491970. DOI: 10.1016/j.pnucene.2011.10. 013. URL: http://dx.doi.org/10.1016/j.pnucene.2011.10.013.

[32] Ali Reza Miroliaei, Farhad Shahraki, and Hossein Atashi. “Computational ﬂuid dynamics simulations of pressure drop and heat transfer in ﬁxed bed reactor with spherical particles”. In: Korean Journal of Chemical Engineering 28.6 (2011), pp. 1474–1479. ISSN: 02561115. DOI: 10.1007/s11814-010-0507-x.

[33] OECD. Neutronics/Thermal-hydraulics Coupling in LWR Technology: State-of-the-art Report (REAC-SOAR). Tech. rep. 5436. 2004.

[34] Charles Radin. “Random close packing of granular matter”. In: Journal of Statistical Physics 131.4 (2008), pp. 567–573. ISSN: 00224715. DOI: 10.1007/s10955- 008- 9523- 1. arXiv: 0710.2463.

[35] Jean C. Ragusa and Vijay S. Mahadevan. “Consistent and accurate schemes for coupled neutronics thermal-hydraulics reactor analysis”. In: Nuclear Engineering and Design 239.3 (2009), pp. 566–579. ISSN: 00295493. DOI: 10.1016/j.nucengdes.2008.11.006.

[36] Frederik Reitsma. “Reactivity considerations for the on-line refuelling of a pebble bed modular reactor - Illustrating safety for the most reactive core fuel load”. In: Nuclear Engineering and Design 251 (2012), pp. 18–29. ISSN: 00295493. DOI: 10.1016/j.nucengdes. 2011.11.038. URL: http://dx.doi.org/10.1016/j.nucengdes.2011.11.038.

[37] Rebecca Rose Romatoski. “Fluoride-Salt-Cooled High-Temperature Test Reactor ThermalHydraulic Licensing and Uncertainty Propagation Analysis”. In: (2017).

[38] Guido van Rossum. Foreword for "Programming Python" (1st ed.) 1996. URL: https:// www.python.org/doc/essays/foreword/ (visited on 1996).

[39] “S. Struth, Thermix-Direkt: Ein Rechenprogramm zur Instationaren Zweidimensionalen Simulation Thermohydraulischer Transienten,FZ. Julich, Julich, Germany, 1985”. In: ().

BIBLIOGRAPHY 122

[40] Raluca O. Scarlat. “Design of Complex Systems to Achieve Passive Safety : Natural Circulation Cooling of Liquid Salt Pebble Bed Reactors”. PhD thesis. UC Berkeley, 2012.

[41] Werner Schenk and Heinz Nabielek. 3.2 Safety Experiments for the Demonstration of Fission Product Retention in Fully Irradiated Fuel Elements. Tech. rep. 2000, pp. 119–135.

[42] R. Serrano-López, J. Fradera, and S. Cuesta-López. “Molten salts database for energy applications”. In: Chemical Engineering and Processing 73 (2013), pp. 87–102. ISSN: 02552701.

DOI: 10 . 1016 / j . cep . 2013 . 07 . 008. arXiv: 0801231 [arXiv:submit]. URL: http : //dx.doi.org/10.1016/j.cep.2013.07.008.

[43] A. Shams et al. “Large eddy simulation of a nuclear pebble bed conﬁguration”. In: Nuclear Engineering and Design 261 (2013), pp. 10–19. ISSN: 00295493. DOI: 10 . 1016 / j . nucengdes.2013.03.040. URL: http://dx.doi.org/10.1016/j.nucengdes.2013. 03.040.

[44] X-5 Monte Carlo Team. "MCNP - Version 5, Vol. I: Overview and Theory", LA-UR-03-1987 (2003).

[45] N. Wakao, S. Kaguei, and T. Funazkri. “Effect of ﬂuid dispersion coefﬁcients on particleto-ﬂuid heat transfer coefﬁcients in packed beds”. In: Chemical Engineering Science 34.3 (1979), pp. 325–336. ISSN: 00092509. DOI: 10.1016/0009-2509(79)85064-2.

[46] Kai Wang, Zhaozhong He, and Kun Chen. “Application of RELAP5 / MOD4 . 0 Code in a Fluoride Salt-Cooled High- Temperature Test Reactor”. In: Advances in thermal hydraulics. Reno, 2014, pp. 1–11.

[47] Xin Wang. FHR Monte Carlo code input generator(FIG). Aug. 2017. DOI: 10.5281/zenodo. 842051. URL: https://github.com/xwa9860/FIG.

[48] C.Y. Wu et al. “Investigating the advantages and disadvantages of realistic approach and porous approach for closely packed pebbles in CFD simulation”. In: Nuclear Engineering and Design 240.5 (May 2010), pp. 1151–1159. ISSN: 00295493. DOI: 10 . 1016 / j . nucengdes.2010.01.015.

[49] Xiaohan Yu and Guimin Liu. Overview of TMSR-SF1 and SF0. Tech. rep. 2014.