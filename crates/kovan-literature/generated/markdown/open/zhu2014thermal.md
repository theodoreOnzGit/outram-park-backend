## ABSTRACT

ZHU, YUWEI.  Thermal Neutron Scattering Cross Sections for Silicon Carbide.  (Under the direction of Ayman I. Hawari).

Silicon carbide based  materials are proposed as  a promising  fuel and cladding

material  for fission and fusion applications.    While there has been significant  research and

development work on  the  manufacturing and determination of radiation effects  in  SiC, the

details of neutron scattering behavior  of  SiC  are  still absent.   In this  situation, neutronics

codes such as MCNP will use the default free-gas neutron cross section libraries, which will

usually result  in  significant inaccuracies in the prediction of the neutron spectrum.   The

predicted thermal spectrum will influence fuel and core design.  Therefore, it is important to

develop thermal scattering cross section libraries for SiC.

In this work,  the  scattering  cross section libraries for 3C-SiC were  fully  developed

using ab-initio and lattice dynamics methods.  Phonon properties of 3C-SiC are estimated by

using ab-initio calculated forces  for  the lattice.   Both the coherent elastic and the inelastic

cross section  in  the incoherent approximation  are computed and  ENDF/B-VII  libraries are

generated.  A program was developed for calculating the coherent elastic component that is

applicable  to all polycrystalline materials.   This routine has been  implemented into  the

LEAPR module of the NJOY code system.

© Copyright 2014 Yuwei Zhu

All Rights Reserved

Thermal Neutron Scattering Cross Sections for Silicon Carbide

by Yuwei Zhu

A thesis submitted to the Graduate Faculty of North Carolina State University in partial fulfillment of the requirements for the degree of Master of Science

Nuclear Engineering

Raleigh, North Carolina

2014

APPROVED BY:

_______________________________    ______________________________

Dr. Gail C. McLaughlin        Dr. Bernard Wehring

________________________________

Dr.  Ayman I.  Hawari Chair of Advisory Committee

BIOGRAPHY

Yuwei Zhu  was born on February 2, 1990 in  Deyang, Sichuan Province, People’s

Republic of China. Yuwei graduated from  No. 5 Middle  School  of  Deyang in  2008 and

subsequently enrolled in University of Science and Technology of China. In 2012, he

graduated from USTC with Bachelor  in Nuclear Engineering. Immediately following

graduation, he came to North Carolina State University for his graduate study. Yuwei began

working with Dr. Ayman Hawari on the thermal neutron scattering cross section of 3C-SiC

as his master research.

ii

## ACKNOWLEDGMENTS

I would like to give my wholehearted gratitude to Dr. Ayman Hawari for guiding me

in this project.    Without his patients and guidance, this project would be impossible.    Dr.

Hawari’s professional outlook and abundant knowledge always shed light on our discussion.

I would like to thank him for supporting me to attend the ANS meeting, where I had a chance

to see and listen to others and really learned a lot.

I would  also like to thank members of my research group,  Jonathan Wormald and

Jesse Holmes.  Their ample knowledge in nuclear engineering and physics always gives me

insight of problems  in  discussion.   Jonathan has been working with me  on inelastic cross

section.  He taught me a lot in research.  Jesse’s dedication to his career and knowledge of

the field inspire me.   Many thanks to Dr.  Victor Gillette, his knowledge in  technical  and

software issues support me throughout this project.

I would also like to thank my family and  friends.   Their support gives me

determination to overcome obstacles. iii

TABLE OF CONTENTS

LIST OF TABLES  …………………………………………………………………...……vi

LIST OF FIGURES ………………………………………………………………………vii

Chapter  1  Introduction…………………………………………………………………………………………….1

1.1  Overview  ....................................................................................................................... 1

1.2  Structure o f 3C-SiC ..................................................................................................... 3

1.3  Nuclear Cross Section  .................................................................................................. 6

Chapter 2  Thermal Neutron Scattering  ............................................................................... 9

2.1  Derivation of the Scattering Cross Section from First Principles  ........................... 9

2.2  Theory of inelastic scattering cross section  ............................................................. 13

2.3  Theory of Coherent Elastic Scattering Cross Section  ............................................ 15

2.4  Derivation of Debye -Waller Factor  .......................................................................... 19

Chapter 3  Computational method  ...................................................................................... 24

3.1  Computation of Coherent Elastic Scattering Cross Section  .................................. 24

3.1.1  Coherent Elastic Scattering Cross Section with Cubic Approximation  ........ 24

3.1.2  Exact Coherent Elastic Scattering Cross Section  ............................................ 27

3.2  Computation of Inelastic Scattering Cross Section  ................................................ 30

Chapter 4  Results  ................................................................................................................. 34

4.1  Development of New Coherent Elastic Routine in LEAPR/NJOY  ......................... 34

iv

4.2  Phonon Properties for 3C -SiC .................................................................................. 36

4.3  Inelastic Scattering Cross Section for 3C -SiC ......................................................... 38

4.4  Coherent Elastic Scattering Cross Section for 3 -C SiC  .......................................... 42

Chapter 5  Conclusion and Future Work  ........................................................................... 45

REFERENCES  ...................................................................................................................... 47

Appendix A  Comprehending the Coherent Elastic Scattering Formula  .................... 51

Appendix B  Comparison of Different Coherent Elastic Scattering Formula  ............ 54

Appendix C  Discussion of ENDF Format  ...................................................................... 57

v

LIST OF TABLES

Table 1. Bound coherent and incoherent scattering cross section for C and Si atom……......14

Table 2.  Comparison of old routine and updated routine…………………………………...34

Table 3.  Input card 5 for LEAPR/NJOY……………………………………………………..35

Table 4. Beginning section of ENDF library……………………………………….………..57

Table 5. Coherent elastic section of ENDF library……………………………………..……60

Table 6. Inelastic section of ENDF library…………………………………………….....….61

vi

LIST OF FIGURES

Fig.  1.  Unit cell of 3C-SiC………………………….………………………………………4

Fig.  2.  Symmetry element of space group  43Fm ……………………….…………...…….5

Fig.  3.  Scattering in reciprocal space…………………….…………………………………6

Fig.  4.  The Neutron scattering system………………………….…………………………..7

Fig.  5. Reciprocal  space construction for a powder scattering experiment…………....…..20

Fig.  6.  The elastic scattering in reciprocal space………………………….…………...….26

Fig. 7.  Calculation flow chart for inelastic cross section……………….………………....30

Fig. 8.  Flow chart of generating phonon DOS…………………………….………………32

Fig. 9.  Phonon dispersion curve compared to experimental data from Ref. [1, 2]…….….36

Fig. 10.  Phonon density of states for 3C-SiC……………………………….……………….38

Fig. 11.  Scattering law of 3C-SiC vs. β for various α………….……………….…………39

Fig. 12.  Secondary neutron spectra of C atoms in 3C-SiC……………………….……….40

Fig. 13.  Inelastic cross section for 3C-SiC unit cell……………………………….……..…41

Fig.  14.  Coherent elastic  cross section of 3C-SiC…………………………………….…..42

vii

Chapter  1  Introduction

### 1.1  Overview

Due to its excellent thermal and chemical stability, outstanding mechanical properties,

radiation resistance and low activation under neutron irradiation, silicon carbide  and its

composites are proposed as fuel and structural material in next generation fission and fusion

reactors  [3].   In the past decade, silicon carbide has enjoyed rising interest in both

fundamental modeling and practical experimental study.  This thesis focuses particularly on

the thermal neutron scattering cross section of SiC material for its development as a nuclear

material.

SiC is utilized in nuclear reactor as a fuel material.  For example, the fully ceramic

microencapsulated (FCM) fuel [4, 5] concept is based on tri-structural isotropic (TRISO) fuel

particles embedded in a silicon carbide matrix.  SiC is used as supporting matrix as wells as a

micro-pressure vessel in TRISO fuel particles.  The innermost core of a TRISO fuel particle

is the fuel kernel.  The neighboring layer is a porous carbon buffer, i.e., low-density pyrolytic

carbon (PyC), containing about 50% void.  This layer is surrounded by a high-density PyC

layer followed by a SiC layer.  The outermost layer is a layer of high-density PyC [6].  The

SiC layer serves as a barrier to fission product retention as well as a structure layer to prevent

the fuel from cracking.   This SiC layer  is  critical to TRISO fuel performance  as it  renders

fuel  pebbles  the capability  of holding the fission production gases in place and withstands

stresses from inner layers.  Under the FCM design, the graphite matrix is replaced with SiC

matrix to offer improved stability, fission product resistance and thermal conductivity.

SiC is also proposed  as a  cladding material  [7, 8].   The  unique neutronic and

1

mechanical properties of zirconium facilitate Zircaloy as a standard material for cladding

nuclear fuels.  However, due to corrosion and radiation damage of typical cladding materials,

fuel rods are often forced to be exchanged before reaching optimum burn up.  Investigation

suggests that SiC composites have better ability to handle the corrosion and degradation [9].

SiC/SiC  composite stands for  SiC-matrix  reinforced  with SiC-fiber.   SiC-fibers  of high

strength are embedded in SiC  matrixes  which  is of lower strength  [10].   The  fibers offer

enough strength for the material while the matrix provides stress transfer and load dispersion.

There is also an intermediate layer between fiber and matrix which separates the fiber from

the matrix.  This layer prevents composite friction and catastrophic failure.

Recent development in SiC enables applications of  4H-SiC  as  an  outstanding

microelectronic device  material and detection  material  [11, 12].   Cubic SiC owns  a wide

bandgap of 2.4 eV, while 4H-SiC owns even higher 3.26 eV bandgap [13].  Since SiC is more

stable under  an extreme thermal-chemical environment than Si or C, these microelectronic

devices and  radiation detectors  made from  SiC  are capable to operate  in  a  hostile

environment such as a nuclear reactor.

In the past decades, many methods of fabricating SiC material have been established.

Moreover, research  reveals  that both the non-irradiated and irradiated properties  of SiC

strongly depend on  the  fabrication process.  In this sense, it  is  essential  to distinguish

different material fabrication processes.   Methods  such as reaction sintering, direct

conversion, polymer  pyrolysis, nano-infiltration and transient eutectic-phase (NITE),  and

chemical vapor processing are used in SiC synthesis [6].  Among these available industrial

production processes, chemical vapor processing is  the most  widely  used in synthesizing

2

nuclear-grade SiC and SiC composites.

Chemical vapor processing includes chemical vapor deposition (CVD) and chemical

vapor infiltration (CVI) [9].  The CVD process is one of the most commonly used processes

to grow monolithic high-purity β-SiC [14]. This process is basically accumulating SiC onto

an oriented crystal surface.  The CVI process is used to fabricate the matrix inside the SiC

composite [15].  Instead of deposition on a surface, chemical vapor infiltrates between fibers

of SiC to form SiC matrix.

Under growing interest of utilizing SiC as a fuel or a core material, evaluation of the

operational  and safety behavior of  the reactor  requires  accurate nuclear scattering cross

section of SiC.  Due to the lack of knowledge with this material, all simulations are currently

conducted using the free atom cross section of silicon and carbon atoms.  In this work, both

the inelastic and elastic cross sections of 3C-SiC are produced.  The inelastic cross section of

3C-SiC is  produced by ab-initio  simulation  and phonon dynamics matrix method.  The

coherent elastic cross section is produced with  a program  developed from basic scattering

theory.   The cross sections of 3C-SiC are prepared in  ENDF/B-VII  format  which can be

easily put into use.

### 1.2  Structure of 3C -SiC

There are currently over 100  polytypes of SiC known to exist.   Among these

polytypes, only three are commonly used in industry because they are the only polytypes that

can be produced in large quantities in bulk single crystal, polycrystalline or fiber/matrix form

[9].  One of the three polytypes has a cubic lattice unit cell.  It is commonly referred to 3C-

3

SiC, also known as  β-SiC.   Because the  lattice  of cubic SiC is close packed face  centered

cubic  (fcc), the  stacking sequence  is denoted  as ABCABC… The number 3 in its name

herein indicates the number of periodical layers.  The other two with hexagonal unit cells are

usually called 4H-SiC and 6H-SiC.   Because they both have  a hexagonally packed layer

configuration, they are also collectively referred to α-SiC [16].  Of particular interest is 3C-

SiC, which is more widely proposed in nuclear reactor as a structure and fuel material [17].

Therefore this thesis will focus only on the discussion of 3C-SiC.

The 3C-SiC  form has a face-centered cubic unit cell with  a lattice constant  of  a  =

4.3593Å.  As shown in Fig. 1, there are 8 atoms in each 3C-SiC unit cell.  These atoms are

tetragonally bonded to each other and  have a coordination number of 4, i.e.,  there are 4 Si

atoms around a C atom and vice versa.  The point group of 3C-SiC is T 2d under Schönflies

notation or  43Fm  under Hermann–Mauguin notation.

Fig. 1.  Unit cell of 3C -SiC . 4

The standard symmetry elements of space group 43Fm  taken from the International

Tables of Crystallography are shown in Fig. 2.  A detailed compilation of the crystallography

terminology can be  found in Ref.   [18].   This figure is a projection of  the  43Fm  unit cell

along its c direction.  Four-fold rotoinversion axes are shown to exist on each unit cell edge

and diagonals across the body center and face centers.  There are four three-fold rotation axes

on the body diagonal.  Mirror planes are perpendicular to the projection plane on each body

diagonal plane.  Glide planes and screw axes are also demonstrated in the figure.  Since 3C-

SiC  shows  high  structural symmetry, ab-initio  calculation of electronic structure can be

efficiently performed.

Fig . 2.  Symmetry element of space group  43Fm . 5

### 1.3  Nuclear  Cross Section

For the purpose of deriving the cross section from predictive methodology, neutrons

need to be considered as quantum waves as well as particles.  Therefore, in a scattering event,

the initial and final neutron wave vectors are assigned ki  and kf .  The scattering of a neutron

by a sample is characterized by the change  of its momentum,  P, and energy, E .    Their

fundamental relationships with wave vector k are expressed as

ifPk k κ  ,  (1.1)

i fκ kk ,  (1.2)

22

## 2 n

E m

ifkk .  (1.3)

Fig. 3.  Scattering in reciprocal space.

A steady  stream of neutrons of wavelength  λ  traveling in the  z  direction can be described

mathematically by the complex plane wave

0i ie   ikz    (1.4)

with  0( )

## 00 ite   , where the incident flux  2

0  ,  2ik  .  The general expression

ki

kf  κ

## 2 θ

6

for the scattered wave in quantum mechanics is

0 i

f e b r

fkr ,  (1.5)

in which the quantity b in  f  is known as the scattering length.

Now consider a beam of thermal neutrons incident on a target.  The target is a general

collection of atoms.  The scattering system could be gas, liquid, a crystal or an amorphous

material.   Incoming neutrons are scattered out of the target in  all 4  directions with  all

possible energies, as shown in Fig. 4.  Suppose a perfect neutron spectrometer is set up in a

particular direction.   The distance of the  spectrometer  from the scattering system is large

compared to the dimension of target, thus the spectrometer subtends only a small solid angle

d.  The spectrometer counts all neutrons within a certain energy range from E to E+dE .

Fig. 4.  The Neutron scattering system .

x d 2

z y

Scattered neutron Incident neutron 7

The double-differential cross section [19] is defined by

## 2 number of neutrons scattered into the direction 2 ,  within

solid angle   with energy between  '  and  ' '  per second '' d d E E dE dE d d dE

,   (1.6)

where    is the  flux  of the incident  neutrons,  which is given by  the  multiplication of  the

velocity and neutron density.  It should be noted that the double differential cross section is

sometimes also called  a partial differential cross section.   The differential cross section is

defined as

number of neutrons scattered into direction 2 ,  within solid angle   per second ' d d d d dE

.    (1.7)

This cross section is  measured  by  a  neutron diffractometer, which counts the number of

neutrons regardless of the scattered neutron energy.  The total cross section is an integral of

differential cross sections over all 4 direction.  It is given by

number of neutrons scattered in all directions per second

tot   .    (1.8)

The differential and total cross section hence is

2

2

2

f

i dAd b d d

,   (1.9)

24tot b  ,   (1.10)

where b is the scattering length in equation (1.5).

8

Chapter 2   Thermal Neutron Scattering

In this chapter, a  derivation of an expression that describes the neutron scattering

cross section will be established.    To start  the derivation,  the Born  approximation will be

applied.  The scattering potential of a neutron  versus a nucleus is assumed to be Fermi’s

pseudopotential.  Based on the two basic approximations above, a computationally friendly

expression will be derived.  Interpretations and discussions of important parameters such as

Debye-Waller factor and scattering law are made.

2.1  Derivation of the  Scatteri ng Cross Section from First Principle s

The derivation of thermal neutron scattering cross sections  is  majorly adopted from

Ref. [20].  Consider a scattering process in which the  state of the scattering  target  changes

from λ to λ′, and the state of neutrons change from k to k′.  The cross section in this particular

scenario is given by

, ', ' ' in d' 11 kk k

d W dd

,   (2.1)

where  , ', 'kkW   is the number of transitions from the state k, λ to the state k′, λ′.  To make the

formula of  the cross section derivable,  as the first step, the Born approximation from  first

order time dependent perturbation theory is applied.   , ', 'kkW   is obtained analytically as

2

, ', ' ' ' in d 2 ', ' ,kk k k W k Vk

 ,   (2.2)

where ρk′ is the number of momentum states in dΩ per unit energy range for neutrons in the

states k′, V(r ) is the nuclear potential from which a neutron scatters away.  As the second step

9

to obtain an analytical formula for cross section,  V(r)  is  approximated as  Fermi’s

pseudopotential

22 ( ) ( )Vb m rr .    (2.3)

The  -function comes from the concept  that the interaction range of  the nucleus

potential is short compare to the scale of atom (1×10 -10 m).  As a neutral particle, a neutron

interacts with the nucleus through the strong nuclear force.  The range of the strong nuclear

force,  which extends only to the order of  femtometer  (1×10 -15  m),  allows the use of  a  -

function to represent the potential.

Inserting these values back into  (2.1)  along with some  mathematical treatment will

give

' ' ''

2 //1 '' ' '2 jjiiiHt iH j t i t j jj

dk b b e e e e e dt d dE k

R R .    (2.4)

The subscript  '  in the double differential cross section indicates that the scattering

cross  section  is for the specific case when the system is changing  from state    to  .

Obtaining Eq. (2.4) means that the Born approximation and Fermi’s pseudopotential will be

implicitly  implied in all of our later formulas.   In application, the scattering system is an

assembly of atoms in which the probability of finding an atom with energy E adheres to the

Boltzmann distribution.  After scattering, the final states of atoms are determined by quantum

mechanics.  Accordingly, to evaluate the double differential scattering cross section, all the

final states  need to be sum over and averaged over all the possible initial states .  In order

to achieve this goal, the Boltzmann distribution and Heisenberg operators are applied, 10

' 2 (0) ( ) ' '

1 ' '2 jji i t j i j jj tdk b b e e e dt d dE k

κ R κ R  ,   (2.5)

in which the Rj(t) is the Heisenberg notation of atom position

//() iHt iHt jjte eRR  .    (2.6)

The average notation in (2.5) of all the initial states of the system is denoted by

A pA ,    (2.7)

where  p is the probability of state λ in Boltzmann distribution.  Equation (2.5) is the most

compact form for the double-differential cross section.  Information about the potential of a

scattering system is contained  in  the Hamiltonian  H,  which is  contained in  a  Heisenberg

operator.  Properties of the cross section of each nucleus in the scattering system are given in

the summation of  bj, as  defined in Eq.  (1.5).   Energy information about  the incident  and

scattered  neutron is included in k  and  k′.   Direction information of  the  scattering is

incorporated in .

Further evaluations of the cross section will originate from this expression.  However,

before proceeding to evaluate  Eq.  (2.5), a  separation  of  the coherent and incoherent cross

section should be made.   This  separation  can be comprehended  physically  and  later

evaluation will benefit from it.

The scattering length is a physical attribute to each isotope of every chemical element.

Moreover,  nuclei with different spin states also have different scattering  lengths.   For

example, if a nucleus has a spin up (+) state and a spin down (-) state, these states will have

scattering  lengths b+ and b-, respectively.   Hence, the scattering system is  quite  often a

11

mixture of isotopes with different scattering lengths.

Consider Eq. (2.5), if it is written that

'' 22

', ', jj jj jj jj bb bb b b   ,   (2.8)

where  ' 2 ,   'j jb bb j j ;  '

2 ',   'j j j jb b b bb j j  .    Thus Eq.  (2.5)  can  be expressed in  two

terms:

2 22

' 2'1 '1 ', , ' 2 2

i t i t

jj j

dk k b j j e dt b b j j e dt d dE k k

.    (2.9)

The first summation in Eq. (2.9) represents the coherent scattering cross section, in which the

waves scattered from each nucleus interfere with each other.  The physical interpretation of

the coherent cross section is the cross section of a scattering system which consists of nuclei

with single scattering length b .   The second summation is known as the incoherent  cross

section.  It does not give rise to any interference between nuclei.  Moreover, its magnitude is

completely determined by  the mean square deviation  of scattering length from  the average

value.  It can be written that

' 2 0

' ( ) ()'1 ' 42 jji i t

jjco i tch

h odk e e e dt d dE k

R R ,   (2.10)

2 (0) ( )'1 ' 42 j j i t

jinc i i tincdk e e e dt d dE k

R R ,   (2.11)

where  2 4coh b ,

224inc bb  .

It can be seen that Eq. (2.10) gives correlation between every atom in the summation.

On the other hand,  the equation of incoherent scattering, Eq.  (2.11), gives only the

12

correlation of a  single  atom with different time.  Thus  it is  said  that coherent scattering

contributes  to interference effects and yields space and time correlations.   Incoherent

scattering does not contain interference information.  It arises from the random distribution of

scattering lengths and their deviation from the average scattering length.

### 2.2  Theory of inelastic scattering cross section

By combining Eq. (2.10) and Eq. (2.11) together, the scattering cross section can be

written in a compact way

2 1 (, ) (, ' ' )

## 4 coh inc sSS

d k d dE k

κ κ ,   (2.12)

where  S(κ      , i.e.,  the dynamic structure factor.   The

scattering law denotes the dynamic  information  from the scattering system, independent of

incoming neutron properties.  S(κs [21, 22] and can be

written as

(, ) (, ) (, )sdS S S  κ κ κ .  (2.13)

By comparing Eq. (2.10) and Eq. (2.11) with Eq. (2.13), the self and distinct scattering law

should correspond to

'

', (0) ( )

'

1 (, ) 2 jj i t d jj j j i i t S e e e dt

κ R κ R κ  ,  (2.14)

(0) ( )1 (, ) 2 j j i t s j i i t S e e e dt

κ R κ R κ  .  (2.15)

As can be shown here, the scattering law is basically Fourier transform of the atomic spatial

13

distribution (which is contained in the average notation as shown in Eq. (2.7)).  In the case

when the harmonic approximation is introduced, where interatomic forces are proportional to

displacement from equilibrium position,  the  scattering law in a crystal  can be further

expanded as

01 2 3 s ss s sS SS S S   ,   (2.16)

01 2 3 d dd d dS SS S S   .   (2.17)

This is known as the phonon expansion, where the superscript represents number of phonons

created or annihilated.  For example, the 0 term corresponds to elastic scattering while the 1

term corresponds to the situation where one phonon is excited or deexcited.

To calculate the inelastic  cross  section in  a crystal  material, the incoherent

approximation is introduced.  The incoherent approximation assumes that the distinct

scattering law Sd is small compared to the self-scattering law Ss, in another word, Sd = 0.  By

ignoring the  contribution  of distinct scattering law  Sd, Eq.  (2.12)  can be written as the

following form using phonon expansion

2 0 () ' 4 , ' s nin n

e

dk d S dE k

Q ,   (2.18)

where  coh inc    .   The comparison of  bound coherent and bound incoherent scattering cross section for Si and C atom is shown in Table 1.

Table 1. Bound coherent and incoherent scattering cross section for C and Si atom.

Element  coh /barn  inc /barn  /barn

C  5.551  0.001  5.551 Si  2.163  0.004  2.167 14

Therefore, it is safe to say that  coh   in the case of silicon carbide.  That is to say,

for inelastic scattering cross section under incoherent approximation, the major contribution

of the cross section comes from  coh .    It should also be  emphasized  that the incoherent

approximation only renders distinct scattering law Sd to be zero.  It made no assumption on

the significance of the contribution from bound incoherent cross section  inc .

### 2.3  Theory  of Coherent Elastic Scattering Cross Section

The corresponding equation for coherent scattering expressed by the scattering law is

1 2 0'() '4 coh cohdE SS d dE E

.  (2.19)

The coherent elastic scattering cross section is obtained by ignoring all  the phonon

generation or annihilation terms.  That is to say, in Eq. (2.19), only the 0Sd and 0Ss terms are

left for elastic scattering,

0 2 '4

coh

coh

d S d dE

.  (2.20)

Therefore,  the goal of this section is to derive  an expression for the elastic scattering law

term 0S.

As an easier strategy to deduce a computational expression for coherent elastic cross

section, firstly a Bravais crystal is assumed and then a non-Bravais crystal formula  will be

developed.  To proceed to evaluate Eq. (2.20), an analytic expression of atom position R j(t)

must be obtained.   ul(t)  is  assigned  as the atomic displacement and  l  as  the  atomic

15

equilibrium position.  Therefore, in a Bravais crystal, it can be written that

( ) (t)jlt  R lu .    (2.21)

So in Eq. (2.10), the correlation term can be put as

'0(0)

' ( ) (0) (t)ll li it i ii

ll l e e N ee e

κ R κ R κ u κ uκ l .    (2.22)

The summation is started from l  =0, because in a crystal system, transformation symmetry

enables us to use relative position l -  l  to determine all the positions in the crystal regardless

of the value of l.

In order to achieve our goal  to analytically express R j(t), the next step is to assume

that interatomic  potential  in the crystal system  is  harmonic, i.e.,  forces are  proportional  to

displacements from  each atom’s  equilibrium  position.   This is only true when atoms are

vibrating at a relative small range compared to their lattice constant.  The harmonic potential

approximation is  a  basis of  phonon theory from  solid state physics.   It renders us the

capability  to calculate theoretical values of physical properties, including the cross section,

utilizing  phonons.   The main justification for  this is the already successful  predictions of

many of the observed properties and predictions.  In the particular situation of SiC, 3C-SiC

as a semiconductor material lacks electrons in conduction band.  The internal energy carrier

in SiC, therefore, is majorly phonon.

In order to  proceed to evaluate elastic cross section using phonon expansion in Eq.

(2.16)  and Eq. (2.17),  model of quantum harmonic oscillator  is applied.   By considering

every atom as a quantum harmonic oscillator, u l can be found in most quantum texts as 16

† 2 iis l ss s s ae a e MN ql qle u  ,   (2.23)

where  q  is its wave vector,  e s  is  its polarization vector,  s  indexes both  q  and polarization

index (1,2,3).  For another word, the sum of s is over N points of q  in the first Brillouin zone

and 3 polarization directions.  as and as† stand for the annihilation operator and the creation

operator for the state  s.   The  time dependent  displacement  u l(t)  can be  expressed by

Heisenberg operator which will eventually operate on as and as†.

//() iHt iHt ssa t e ae ,   (2.24)

† /† /() iHt iHt ssa t e a e .    (2.25)

If not stated explicitly, both as and as † shall stand for the time dependent as(t) and as†(t) from

now on.  Another set of short-hand notations that is applied is

† 0(0) i ss ss s i ga gaU   u ,   (2.26)

†*(t) il ss s s s i haV ha  u ,   (2.27)

2 s s s g MN

e ,   (2.28)

() 2 ss s s ithe MN

ql e .    (2.29)

By substituting Eq. (2.21) ~ (2.29) into Eq. (2.10), after somewhat lengthy calculation

22 ' ' 42 U i i t

lcoh UVcohd kN e e e dt d dE k e

l .  (2.30)

If the term  UVe  in Eq. (2.30) is Taylor expanded, 17

2 1 1 ! 1 2! pUV e UV UV p UV  ,   (2.31)

in which  the  very first  term gives the elastic  scattering process and the  rest term with  p th

power gives p-phonon process.  The p-phonon terms contribute only a small portion to the

total coherent cross section of 3C-SiC.  Hence of particular interest is the elastic scattering

process in  our research.   Therefore,  theoretical deduction will  be proceeded  only with the

elastic term in  Eq.  (2.31).   By replacing  UVe  with 1  and apply the  k=k  relation in elastic

scattering, the differential coherent elastic cross section will follow by integral of E  over all

scattered neutron energy

2

4 i

lcoh e Ucoh

l

d Ne e d

l .    (2.32)

With the periodicity of crystal lattice, the summation of l  can be largely simplified as

a δ-function, which changes the coherent elastic cross section to

3

0 2 4 ()

## 2 Wcoh

coh el

d Ne dv

τ  τ ,   (2.33)

where N is the number of unit cells in the crystal.  The δ-function δ(κ - τ)  is the Bragg’s law,

i.e.,  2 sindn , written in reciprocal space.  It should be noted that because Eq. (2.10) and

Eq. (2.22) are summing over all the atoms in the scattering system, in this case a crystal, the

coefficient N should be taken off for the cross section of a single unit cell.  The exponential

term  2We  is called the Debye-Waller factor, which is of paramount importance in our cross-

section calculation.   The exponential term  22 02{ ()}0WU u is  called Debye-

Waller coefficient. 18

Equation (2.33)  is derived from  the first principle  based on several  approximations.

However, readers may find it hard to  interpret the  physics meaning behind each term.   An

easier approach is offered by Ref. [23] and extended in Appendix A.  This methodology of

derivation may help readers comprehend how the physics is built into each equation.

The derivation of the coherent elastic cross section for non-Bravais crystal follows the

same philosophy as Eq.  (2.33).   Since, for a non-Bravais crystal,  there are more than  one

atom per unit cell, the cross section should take into account all the atom positions in a unit

cell.  This leads to

2 3

0

2 ( ) ( )

coh el

d FN dv

τ κ τ κ    (2.34)

where F(κ) is known as the nuclear structure factor.

( ) WiF be e

κ dκ    (2.35)

in which W is the Debye-Waller coefficient of atom  in a non-Bravais crystal.

### 2.4  Derivation of Debye -Waller Facto r

As shown in  Eq.  (2.32)  and  Eq.  (2.33), Debye-Waller factor 2

## 2 UWe e

is  the

exponential  of  the mean square displacement  along the   direction.   In another word, in a

crystal system, Debye-Waller factor takes into account of thermal oscillation of atoms around

their equilibrium positions and their zero point energy.  Calculating Debye-Waller factor is

one of the most important steps to correctly evaluate the cross section.  For the convenience

of writing a program,  there are some approximations that  should be made on  crystalline

19

materials for Debye-Waller factor.

Fig. 5. Reciprocal space construction for a powder scattering experiment .

In a polycrystalline material where the dimension of randomly orientated crystal

mosaic is small enough, it can be assumed that the incident neutron beam is able to “see” all

crystal directions.   Thus, in  reciprocal space,  spherical reflection shells  are constructed by

randomly orientated lattice points. These spheres are isotropic to incoming neutron direction

(please refer to Fig. 5).

This isotropic symmetry of incident neutron beam can also be obtained in case of a

single crystal made up  of cubic unit cells.   Therefore, the expression for the Debye-Waller

factor of a polycrystalline material is exactly the same as that of a single cubic crystal.  From

historical convention, this assumption of using isotropic symmetry to calculate Debye-Waller

factor is called cubic approximation.  In the field of nuclear engineering, almost all nuclear

materials used in reactor are polycrystalline materials.  Therefore, it is safe to apply the cubic

approximation on any  crystalline  materials used in nuclear engineering.   After  the cubic

20

approximation is applied, the Debye-Waller coefficient  in  Eq.  (2.33)  for a polycrystalline

material is

2 0

## 2 coth d

2 ( 2 )m B W M k T

κ ,   (2.36)

where () implies the phonon density of states (DOS).  The phonon DOS gives probability

density  of modes  available at  frequency  .   It should be noted that ()  should be

normalized so that integral over all frequencies equals to 1 before applied in Eq. (2.36):

() 1d   .  (2.37)

Since  Eq.  (2.33)  is derived under  Bravais crystal assumption,  Eq.  (2.36)  holds  only  for

Bravais polycrystalline materials.   However, in case of  non-Bravais crystal, as appeared  in

Eq. (2.35), the Debye-Waller coefficient should be

2

## 0 coth

42 () d m B W M k T

κ .  (2.38)

As shown here, instead of having a universal Debye-Waller coefficient for the whole unit cell,

W is calculated at each atom position in a unit cell.

To sum up, there are two approximations applied in Eq. (2.36) and Eq. (2.38).  The

first one is that displacements of atoms are independent of lattice sites and atom types.  This

means for every atom in the scattering system, its displacement is isotropic in all directions,

i.e.,

2 2 2 222 ( ) 13Wu  κκ u κ u .  (2.39)

The benefit of this assumption is  that a  universal  DOS  () instead of  several different

partial DOS can be applied to evaluate a universal value of Debye-Waller factor.  However,

21

this is not always true due to existence  of off-diagonal DOS  resulting from non-

homogeneous forces in the crystal.

The second assumption applied, as mentioned above, is the  cubic approximation.

However, in the situation of a  non-cubic single crystal material, the cubic approximation

cannot be applied.   Or in  a directionally oriented material, e.g.,  lamellar structure  graphite

composed by two-dimension layers, the approximation cannot be applied either.  In this case,

the polarization of phonon  modes  is no longer symmetric.   In the  above  cases, a  more

specific Debye-Waller factor shall be used.  This Debye-Waller factor is illustrated below.

Recall that in Eq. (2.38),  it uses total DOS (), which is the summation of partial

DOS.  In a more general theory, the DOS is a 3×3 matrix.  The partial DOS are phonon DOS

on three Cartesian polarization directions (x), (y), (z) locating on the diagonal of the DOS

matrix.   They describe  the contribution  from  different polarization direction to the total

phonon DOS.  They are defined as

2

, ,

1 ( ) ( , ; ()( ),)ii j ej j nd

k k k ,   (2.40)

where ei(k,  j; ) is the i th Cartesian component of the polarization vector for the th particle in

the unit cell, (k,  j) is the phonon frequency, and d is the dimension of the dynamical matrix.

Moreover, there are also off-diagonal DOS which are defined as

* , ,

1 ( ) ( , ; ) ( , ; ) ( )( , )il i l j je je j nd

k k kk .    (2.41)

The off-diagonal  terms denotes the  correlation  between phonons polarized in three

orthogonal directions.  Combined with diagonal terms, the phonon DOS can be written as a

22

3×3 matrix.  If there is no correlation between forces along x, y, z directions in the crystal, the

off-diagonal terms should be zeros.  However, due to different kinds of crystal symmetry and

different electron distribution of chemical elements, the forces are not always isotropic in the

crystal.  That is to say, the off-diagonal terms are not always zeros.  For example, in cubic

lattice (3C-SiC is this case) they are in orders of magnitude smaller than the diagonal terms.

Hence, it is safe to  apply the isotropic approximation  and  only consider the existence of

partial DOS.   In  the  cases of randomly oriented polycrystalline materials  and single cubic

crystal, this  approximation holds.   But in the situation of  a single crystal with correlation

between forces  along x,  y,  z  directions, the matrix  of phonon DOS  should be  applied to

calculate the  Debye-Waller factor.   It should be noted again that phonon DOS should be

normalized before applied to calculate the Debye-Waller factor.

Now the Debye-Waller matrix B() can be calculated using the phonon DOS matrix.

Elements of the 3× 3 matrix B() represent the mean square displacement of an atom   in

each direction and their correlation.  It is expressed as

,

0 ( ) coth () d 22

m il il B B M k T

 .    (2.42)

The Debye-Waller coefficient is then

1 () 2 W  Bτ τ    (2.43)

The  new  Debye-Waller factor then can be plugged into  Eq.  (2.35)  to calculate  nuclear

structure factor.

23

Chapter 3   Computational method

The theories  of  coherent elastic and inelastic cross section are  fully proposed in

Chapter 2.  In this chapter,  methods on how to  apply these theories to  calculate  the cross

sections will be described.  Important algorithms concerning Debye-Waller factor and

coherent elastic cross section will also be discussed.

### 3.1  Computation  of Coherent Elastic Scattering Cross Section

For application purpose, our calculated cross section will be processed and published

using ENDF/B-VII format.  Therefore, a good choice is to implement our program into the

existing nuclear data processing code, i.e.,  NJOY  [24, 25]  in this research.   However, the

LEAPR module in NJOY uses a different set of nomenclature from what is shown beforehand.

A transformation  from the standard terminology to  NJOY  form,  therefore, is  necessary

(shown  in Appendix B).    In this  work, a set of program-friendly equations will be  derived

and applied.   The sections below will  try  to demonstrate the philosophy of how  they are

applied in our program.

As  mentioned in  Section 2.3, there are different  strategies  that can  be applied to

calculate the Debye-Waller factor when different approximations are applied.

#### 3.1.1  Coherent Elastic Scattering Cross Section  with Cubic Approximation

Cross section is the final station of our trip.  Thus to reach it, calculation of Debye-

Waller factor should be the first stop.  As shown in Eq. (2.38), when the cubic approximation

24

is applied, there is an universal Debye-Waller factor for all atom types.  The calculation of

Debye-Waller factor is carried out as shown below:

0 1 ( )coth( )

## 2 d

,   (3.1)

B w Ak T

,   (3.2)

tot

n ww n

,   (3.3)

where  '2 n

B

EE m EE Ak T

is the unitless momentum transfer,  '

B

EE k T

is the unitless

energy transfer, A  is the atom  mass in  amu,  kB  is the Boltzmann Constant and  T  is the

temperature.  This calculation routine is essentially the same as the one proposed by Squires

in  Eq.  (2.38).   The prof can be found in  Appendix B.   As shown in  Eq.  (3.3),  universal

Debye-Waller coefficient  is a summation over  each  atom type weighted by  corresponding

atomic ratio.  It should be mentioned that the exact value of A input into the program should

be the mass ratio of atom  to neutron.  However, it is still a good approximation to just use A

as atomic mass in amu, errors are negligible.

The integral of  which gives  is accomplished in two steps in the program.  In the

first step, the phonon DOS is divided by ()ee

.   Then the intermediate  value is

multiplied by ee and integrated  in a loop.   It is important to emphasize that  though

NJOY  requires the first point of phonon DOS to be (0,  0), this point is never used  by the

program.  The first point of phonon DOS is instead prepared by applying the Debye parabolic

25

model using the second point.  Therefore, in order to achieve comparable results with NJOY,

our program follows  this convention of  NJOY.   The  next step  is to calculate the

crystallographic structure factor fi  in corresponding reciprocal space position i.   The

structural factor defined in NJOY is

22 2

## 2 i

i n fF m NV

,   (3.4)

where N is the number of atoms in the unit cell, mn is the mass of neutron, V is the volume of

the unit cell.  The sum extends over all reciprocal lattice vectors of the given length i.  The

absolute square is given by

2 2 1 () j N i j j Fe

r .    (3.5)

Fig. 6.  The elastic scattering in reciprocal space.

Then the coherent elastic scattering cross section can be easily calculated with

i

ki

kf 26

41 , i

i wEi coh i EE i

f E e E

,  (3.6)

where the   indicates cosine of scattering angle  , =cos( ).  The -function is the Bragg

condition.  Ei are the so-called “Bragg Edges”, and

1 iiEE  .    (3.7)

The Bragg  Edge energy  Ei  is defined as the  smallest energy for a neutron to  elastically

scattered by a corresponding reciprocal space vector of magnitude i.  As shown in Fig. 6, the

smallest neutron energy can be obtained when ki is the smallest of all possible cases, that is,

the case of backscattering: ki = i/2.  Therefore, Ei is given by

2 2 22

28

ii i nn

k E mm

 .    (3.8)

#### 3.1.2   Exact Coherent Elastic Scattering Cross Section

Instead of applying cubic approximation, there is no uniform Debye-Waller factor in

this section.  When the Debye-Waller factor is calculated in the exact way, the cross section

can be applied to single crystal and  inhomogeneous  materials.   The tradeoff  for this

generality is that the Debye-Waller factor now is different for every reciprocal lattice point τ.

Correspondingly, the cross section (τ) should be a function of each reciprocal lattice point

position.  For example, if there are 50 3=125,000 reciprocal lattice points, the Debye-Waller

factor and the cross section have to be calculated for 125,000 times at each reciprocal space

position τ.  Then summation should be made of the cross section with the same  τ . 27

As shown from  Eq.  (2.40)  to  Eq.  (2.43), the Debye-Waller  factor can be calculated

from matrix Bij() which is integral of the partial phonon DOS and the off-diagonal DOS.  In

this work, the matrix Bij() is directly output by the software PHONON 5.1.2 [26, 27] in the

output file “*.d33”.   The reciprocal space vector  h k l 1 23 b bb  is achieved  by the

following steps.

First, the real space unit cell matrix A is read directly from input with the following

format

12 3

12 3

12 3

x xx

y yy

z zz

aa a

aa a

aa a

12 3A aa a .    (3.9)

Then, the reciprocal space matrix B should have the relation with A : B=A -1

11 1

22 2

33 3

x yz

x yz

x yz

bb b

bb b

bb b

1

2

3

b

Bb

b .    (3.10)

Next, τ is

11 1

22 2

33 3

x yz

x yz

x yz

bb b hk l hk l b b b bb b

1

2

3

b b b .    (3.11)

Following by the calculation of τ, calculation of the Debye-Waller coefficient W(τ) can then

be carried out  for  each  τ  by applying  Eq.  (2.43).   After point specified Debye-Waller

coefficients  for each atom type  are obtained, W(τ)  should be weighted over all types of

atoms using Eq. (3.12) to acquire the Debye-Waller factor w(τ) for the material: 28

() () tot

n wW n

τ τ .  (3.12)

Finally Eq. (3.13) can be applied to calculate the cross section (τ) for each reciprocal point τ

utilizing w(τ)

4 ( )1 , , i

i wEi coh i EE f E e E

τ τ τ ,  (3.13)

in which, the magnitude of |τ | can also be easily obtained by

2 2 2 2 2 2 2 2 2 2 2 2 2

22

2 2

1 [ sin sin sin

2( (2 , , , , ) 2 , , )]

k l

h h bc a c ab V abc F kla bcF cl abkh F

τ ,   (3.14)

where

, , ) cos cos c( osF       ,   (3.15)

V is the volume of the unit cell,

2 2 22 2 2 2 cos cos 2cos cos co(1 cos s)V abc       .    (3.16)

{a, b, c,  , ,  }  is  the  standard crystallographic notation of a crystal unit cell [28].   The

reciprocal space dependent cross section   , ,coh Eτ  can then be summed over each τ with

the same magnitude to obtain the energy dependent cross section   ,coh E.

Up to this point,  calculation strategy of  the coherent elastic cross section  is well

established.  One last point that should be pointed out is that the cross section shall be output

directly by a standalone program developed by this work.  However, only after implemented

into LEAPR module in NJOY,  useful cross  section data file in  ENDF/B-VII  format can  be

obtained.   Though following the convention of  NJOY, the  LEAPR  module outputs  E(E)

29

instead of  σ(E),  Eσ(E) is  divided by  E  in the  THERMER module.  Therefore, the output

passing through THERMER module shall be the cross section  instead of the cross section

multiplied by the energy.

### 3.2  Computation  of Inelastic Scattering Cross Section

The inelastic  cross  section under incoherent approximation  can be simplified to the

following form using unitless momentum transfer α and energy transfer β,

2 0

' '4 (, )s nel n

Bin S k dE d dE ET

,   (3.17)

Fig. 7.  Calculation flow chart for inelastic cross section . 30

As shown in Fig. 7 the left hand side is the incoherent inelastic routine while the right

hand side is the coherent one-phonon routine.   In this work, only  the incoherent inelastic

routine will be adopted because in 3C-SiC, the coherent inelastic cross section is in orders of

magnitude  smaller than incoherent inelastic cross section.   After  phonon DOS  ρ(β)  is

obtained from lattice dynamics program,  calculations  of  the scattering law  Ss(α,β) and the

cross section  are handled by  LEAPR module in  NJOY.   The scattering law  Ss(α,β)  uses  a

100×100 (α, β) mesh. Calculation of Ss(α,β) proceed with phonon expansion to the order of

100.  All calculation is set under room temperature 300 K.

An  Ab Initio  program named VASP [29-32]  is used with combination of lattice

dynamics program PHONON 5.1.2 to generate the phonon DOS.  Lattice dynamics method is

an important predictive  method commonly used to calculate phonon frequencies ω(τ) in

reciprocal lattice point [33].  The eigenvalues of the dynamical matrix D(τ) give the squares

of  allowed phonon frequencies  ω(τ)2  for a given reciprocal point  τ  in first Brillouin zone.

Therefore, the more points that are sampled in reciprocal space the more accurate the phonon

DOS will be.  The dynamical matrix D(τ) is a 3×3 matrix generated from secondary partial

derivative of crystal potential to atom displacement.  The potential and forces in the crystal,

therefore, need to be defined beforehand in order to fulfill the goal of generating dynamical

matrix.  There are generally two routines  that can be  followed  nowadays to generate

interatomic forces.

The conventional classical method extracts atomic forces from fitting experiment data.

This method, however,  might bring in unnecessary uncertainty from experiment.   It also

renders huge percentage of variation in the low frequency range of phonon DOS, which has a

31

huge impact on the scattering law and scattering cross section.  In this research, the ab-initio

methods are deployed to provide atomic potential and analyze the forces.  Program VASP is

chosen because it  exhibits  the best  agreement  with interatomic potential according to our

experience.

Fig. 8.  Flow chart of generating phonon DOS .

Figure 8 demonstrates a flow chart of how the phonon DOS is fulfilled by utilizing

PHONON and VASP.  A 3C-SiC unit cell with the lattice parameter a=4.395 Å is first built in

VASP.  Then atoms in the unit cell are relaxed to their equilibrium position with the lattice

parameter  minimized to its  lowest energy.   The electronic structure calculation utilized the

generated gradient approximation (GGA) with a plane wave cut off energy of  900 eV.  A

3×3×3 Monkhorst-Pack k-mesh and tetrahedron smearing scheme were used to for

integration.   The convergence of energy is  set to be 1×10 -5 eV.   Thus a minimized 3C-SiC

32

unit cell with a=4.379  Å at 0 K is built.  The minimized unit cell is then put into PHONON to

generate a 3×3×3 supercell with 216 atoms in it.   Because the unit cell holds  only 2  non-

equivalent atoms at high symmetry positions, a total of 4 displacements are sufficient enough

to construct the dynamical matrix.   One of the carbon atoms is  displaced  by ±0.02  Å  and

another silicon  atom is  displaced  by  ±0.02  Å.   Four position cards with displaced atom

position are then generated by phonon program.   These  position cards are put into  VASP

using VASP’s  pseudopotential to calculate the Hellmann-Feynman forces of each displaced

system.   These outputs from  VASP  are again input into  PHONON which yields dynamical

matrix.   With the dynamical  matrix generated,  Monte-Carlo sampling  in the first Brillouin

zone can be carried out to produce phonon dispersion curve as wells as phonon DOS.  When

generating phonon dispersion curve,  LO-TO splitting is applied to split longitudinal optical

and transverse optical dispersion curves.  In this research, the phonon DOS is sampled with 1

million points and sorted into 0.001 eV energy bars.

33

Chapter 4   Results

This  study  proposed the theory and  strategy  to calculate the  cross  section of

polycrystalline materials.   Based  on the derivation of  Eq.  (3.6), a complete routine of

calculating coherent elastic cross  section is developed.   In order to generate necessary

information to calculate Debye-Waller factor, phonon DOS is extrapolated from VASP and

PHONON.   Both inelastic and coherent elastic cross sections are processed by NJOY code

system and available as ENDF/B-VII library.

### 4.1  Development of New  Coherent  Elastic  Routine in LEAPR/NJOY

The new coherent elastic routine is completely rewritten and is more sophisticated

and versatile than the original NJOY routine.  Comparison of the new routine and the old one

is made in Table 2.

Table 2.  Comparison of old routine and updated routine.

Old routine  New routine

Supported structure  Hexagonal, FCC, BCC  Any crystal structure

Supported material  Graphite, beryllium, beryllium oxide, aluminum, lead, iron  Any material

Debye-Waller Factor  Approximated  Exact Need to modify source code if calculating other materials  Yes  No

There are two basic routines you can choose to accomplish the coherent elastic cross

section calculation.   One is  the cubic  approximation routine.    Another is the single  crystal

routine.  The cubic approximation, in general, applies to any polycrystalline structure.  Even

34

for non-cubic crystal  the approximation is often close enough that the  algorithm  is still

correct.    In case of  a non-isotropic crystal, where  there is an orientation preference in a

particular direction  throughout the  whole crystal, the second “exact Debye-Waller Factor”

routine is recommended to calculate the coherent elastic cross section.  In the input card of

NJOY, the 4 th entry of card 5 of LEAPR input card controls the coherent elastic routine (see

Table 3 below).  It should be specified in NJOY input card which coherent elastic routine to

execute.

Table 3.  Input card 5 for LEAPR/NJOY.

* card 5 - principal scatterer control                                                                                             * *    awr     weight ratio to neutron for principal scatterer                                                              * *    spr     free atom cross section for principal scatterer                                                              * *    npr     number of principal scattering atoms in compound                                                     * *    iel     coherent elastic option                                                                                                   * *                   0  none (default)                                                                                                       * *                   1  cubic approximation                                                                                            * *                   2  exact Debye-Waller Factor                                                                                  * *    ncold   cold hydrogen option                                                                                                  * *                   0   none (default)                                                                                                      * *                   1   ortho hydrogen                                                                                                    * *                   2   para hydrogen                                                                                                     * *                   3   otho deuterium                                                                                                    * *                   4   para deuterium                                                                                                    * *    nsk         0   none (default)                                                                                                      * *                   1   vinyard                                                                                                                * *                   2   skold                                                                                                                    *

Despite the NJOY’s original input card, there is an independent input card you need

to prepare for the new coherent elastic routine.  It should be named “coh_input”; otherwise it

will not be recognized by the program.  Depending on the approximation you choose in the

program, the input card varies.   The input card contains important information regarding

35

structure parameters, bound cross sections and dynamic force information.

### 4.2  Phonon Properties  for 3C -SiC

Though the phonon DOS  ρ(ω), a property directly determines  inelastic  scattering

cross section, cannot be benchmarked due  to  lack of current experiment data,  the phonon

dispersion curve can be compared to current existing experiment to prove the trustiness  of

our calculation.  Figure 9 shows a comparison of phonon dispersion curve with experiment

data [1, 2].

Frequency (THz) Direction

0.0 0.5 1.0 1.5 2.0 2.5 0

5

10

15

20

25

30 q=[111]q=[100]q=[110] K X L

Fig. 9.  Phonon dispersion curve compared to experimental data  from Ref. [1, 2] . 36

It is important to point out that the frequency output from PHONON in Fig. 9 is not

the angular frequency ω in Eq. (2.36), but rather ν, where  and ω are related by  2   .

As  k  approaches zero (the long-wavelength limit), acoustic branches of phonon  dispersion

curve exhibit linear response to its frequency.  The parabolic potential from  VASP  which

assumes force is proportional to displacement gives rise to  this linear response between ω

and k.  It gives rise to the parabolic shape in low frequency region of phonon DOS.  This is

due  to the  fact that in the very  low frequency  range the DOS is proportional  to square of

frequency multiplied by group velocity, which is the derivative of ω to k

2)( k

, ω.  (4.1)

When linearity between ω and k renders the partial derivative a constant in Eq. (4.1),

the phonon DOS becomes proportional to ω

## 2.  Figure 10 illustrates phonon DOS of 3C-SiC

from 1,000,000 sampling points in k-space and then distributed into 1×10-3  eV  interval

energy bars.  As shown in the DOS, low energy region DOS is primarily contributed by Si

atoms while high energy region DOS is mainly contributed by C atoms.  Since C is lighter

than Si, it is easier for C to vibrate with higher frequency under the same magnitude of force.

Therefore, the acoustic branches of phonon dispersion curve are mainly results of Si atoms

vibration while  the optical branches  mainly stems  from C  atoms.  It should be noted that

though the  Debye temperature of 3C-SiC is 1200  K, which corresponds to 0.1  eV,  the

parabolic region ends much earlier around 0.03 eV.  Thus, the Debye model is applied in this

research with the phonon DOS fitted into a parabolic curve only in the energy range 0 eV ~

0.02 eV. 37

0.00 0.02 0.04 0.06 0.08 0.10 0.12 0.00

0.05

0.10

0.15

0.20

0.25

0.30

0.35

0.40 Energy (eV)Phonon Density of States Si partial DOS C partial DOS Total DOS

Fig. 10.  Phonon density of states for 3C -SiC .

### 4.3  Inelastic  Scattering Cross Section for 3C -SiC

There are  many necessary steps in obtaining the inelastic scattering cross section.

The first is to calculate the partial DOS of Si and C atoms as shown in the previous section.

The second step is  to  subtract  scattering law from incoherent approximation and Gaussian

approximation.  The scattering law is calculated from a Fourier transform in LEAPR  module

and is partly shown in Fig. 11. 38

39

0.1 1 10 1E-5

1E-4

1E-3

0.01

0.1S(,) 

0.0004

0.0016

0.0052

0.0165

0.0517

0.1654

0.5293

1.6933 =

Fig. 11.  Scattering law of 3C-SiC vs. β for various α.

The inelastic cross section is then directly obtained by the following equation

  ' ', ( , ) 2 b

B E E E S k T E      .    (4.2)

Equation (4.2) shows that the cross section is basically product of three independent variables

and  is  proportional  to  the  scattering  law  S(α,β).    The  first  variable  is  σb,  i.e.,  bound  cross

section of neutron, gives the information of the interaction between neutrons and scattering

material.  The second variable

'EE depends on the change of energy of neutron during the

scattering process.  The third factor, scattering law S(α,β), is independent on neutron property,

that is, neither its intrinsic property nor its interaction with scattering system.   The only

attribution to the scattering law is the scattering system, i.e., its atom vibration mode, forces

between atoms, phonon distribution and crystal lattice structure.   Therefore, S(α,β) is a

separated factor that contributes to inelastic cross section from scattering system.

Secondary  scattering cross sections  can then be  evaluated  by  Eq.  (4.2).   The

secondary neutron  spectra are partly  shown in  Fig.  12. They  are integrals of double

differential cross sections over all possible scattering directions.

0.060.080.10.120.140.160.180.2

## 1 E - 4

## 1 E - 3

0. 01

0. 1 0.01

0.1

1

Incident Energy

### 0.050 eV

### 0.062 eV

### 0.084 eV

### 0.114 eV

### 0.164 eVSec. Energy (eV)d/dE'

I nc i dent  E nergy  (eV )

Fig . 12.  Secondary neutron spectra  of C atoms in 3C - SiC . 40

In Fig. 12 an incident neutron energy range just above the thermal neutron energy is

selected to show the up-scatter and thermalization behavior.  It can be seen that the down-

scattering peak is flattened out to lower energy region  when incident neutron energy

decreases.   The thermalization behavior is directly  exhibited  by the fact that integral over

down-scattering peak is larger than  that of  up-scattering peak.   By integrating  every

secondary spectrum over  scattered neutron  energies E′,  a  total scattering cross section for

each atom type can be obtained.  The total cross section for C and Si and SiC is plotted in Fig.

13.

Energy (eV)

1E-5 1E-4 1E-3 0.01 0.1 1

0.1

1

10Cross section (b) C Si tot

Fig . 13.  I nelastic cross section for 3C - SiC unit cell. 41

42

The  inelastic  cross  section  shown  in  Fig.  13  is  the  one  averaged  per  atom  pair.    In

another word, the cross section is averaged over all four pairs of Si and C atoms in the unit cell.

As shown in the plot, a minimum inelastic cross section can be found around 0.01 eV.  Thermal

neutrons around 0.01 eV will therefore have minimum probability of inelastic scattering with

silicon carbide.   This  “transparency” to  thermal  neutrons  renders 3C-SiC capability to  be  a

promising nuclear structural and fuel material candidate.

4.4  Coherent Elastic Scattering Cross Section for 3-C SiC  1E-5 1E-4 1E-3 0.01 0.1 1

0.1

1

10 Inelastic Coherent elastic Total Energy (eV)Cross section (b)

Fig. 14.  Coherent elastic cross section of 3C-SiC.

The coherent elastic scattering cross section for a polycrystalline 3C-SiC is illustrated

in Fig. 14 together with the inelastic cross section.  Unlike the inelastic cross section, which

is extrapolated from partial DOS of Si and C atoms, the coherent elastic  cross section  is

calculated by Eq. (3.1) using total phonon DOS.  Both the coherent elastic and inelastic cross

sections are prepared in ENDF/B-VII libraries (Please refer to Appendix C).

The manner in which neutrons scattered by polycrystalline material can be explained

by Eq. (3.6).  As shown by Fig. 6 and Eq. (1.2), maximum momentum change occurs in the

situation of back scattering.  Hence, when wave vector is less than one half of the smallest i,

coherent elastic scattering could not happen.  The first coherent elastic scattering occurs

when energy increases till the wave length of a neutron equals to two times of the maximum

of plane spacing in the lattice: =2d max (equivalent to 2ki=τmin).  In this situation, the incident

neutron wave direction ki is perpendicular to the plane with spacing dmax.  At slightly higher

energy,  the  cross section results from planes with  spacing dmax  decrease  with increasing ki,

being proportional to  1/E.   As  the incident energy  increases  to the second Bragg edge,  the

cross section jumps to  another maximum value as it  is a summation of all contributions of

suitable reflection planes in  Eq.  (3.6).   As energy further  increases  to several  eV, cross

section appears as a visually smooth curve decreasing with E.  This is because there are so

many planes contributing to the summation  in Eq.  (3.6)  and they are all very small due to

damping from  the Debye-Waller factor.   As energy  approaches several eV, the coherent

elastic cross section  becomes  very small.   The total cross section remains near  free atom

cross section due to the compensation from the increase in inelastic cross section.  When the

neutron energy is large enough, neutron scattering is essentially the same from that by free

43

atoms at rest because the phonon energy is negligible compared to the neutron energy.  Thus,

phonons can hardly make any significant contribution to the cross section.

44

Chapter  5  Conclusion and Future Work

In this work, computational analysis of thermal neutron scattering in 3C-SiC was

performed.  Starting from the basic definition of the cross section, two basic assumptions are

made to deduce an expression for the cross section.  The first one is the Born approximation

from  first order time perturbation theory.  The second is Fermi’s pseudopotential  which

-function.  Based upon these assumptions, quantum

mechanics is utilized to mathematically quantify cross section in the form of scattering law.

Approximations and assumptions are needed to transform the theory to  a

programmable  equation.   The first is the  harmonic  approximation, which assumes that the

forces in crystal lattice are proportional to displacements.  This approximation allows us to

use phonon theory and expand scattering as shown in Eq. (2.16) and Eq. (2.17).  The second

is the cubic approximation, which assumes that the sample takes the form of a polycrystalline

material.  This  approximation is in fact  accurate for polycrystalline materials.   The third

approximation is that there exists a single universal Debye-Waller factor that can be applied

to every  atom in the sample and generate correct  averaged  cross sections.  This

approximation generally averages  out the effect of  the different scattering length of each

atom and the orientation preferred micro-crystalline arrangement.

With these assumptions and approximations applied, the phonon properties of 3C-SiC

are calculated and presented as dispersion relations and phonon density of states (DOS).  The

phonon dispersion curves show consistent agreement with existing experimental data, which

ensures the correctness of our partial phonon DOS of C and Si atoms in 3C-SiC.  The phonon

properties are then implemented into the NJOY program to calculate the cross sections as

45

well as generate the  ENDF standard libraries.   Modifications and  explanations of

LEAPR/NJOY  module are made to  make the code more  flexible and general.   A  more

versatile standalone  code that is capable  of calculating  the cross  sections for both single

crystal and polycrystalline materials is developed.  Though 3C-SiC is the focus of the present

work, the newly developed routine in  LEAPR/NJOY is  applicable to all polycrystalline

materials.

Due to the lack of experimental thermal scattering cross section data for 3C-SiC, the

current work can benefit from performing  measurements.   Experiments for measuring the

total cross section are currently being considered at the NCSU PULSTAR reactor.

46

## REFERENCES

[1] S. Bagci, S. Duman, H.M. Tutuncu, G.P. Srivastava, "Theoretical studies of SiC, AlN and their (110) surfaces", DIAM. RELAT. MAT. 18, 1057 (2009).

[2] K. Karch, P. Pavone, W. Windl, D. Strauch, F. Bechstedt, "Ab initio calculation of structural, lattice dynamical, and thermal properties of cubic silicon carbide", INTERNATIONAL JOURNAL OF QUANTUM CHEMISTRY. 56, 801 (1995).

[3] R. Jones, L. Snead, A. Kohyama, P. Fenici, "Recent advances in the development of SiC/SiC as a fusion structural material", FUSION ENG. DES. 41, 15 (1998).

[4] N.R. Brown, H. Ludewig, A. Aronson, G. Raitses, M. Todosow, "Neutronic evaluation of a PWR with fully ceramic microencapsulated fuel. Part I: Lattice benchmarking, cycle length, and reactivity coefficients", ANN. NUCL. ENERGY. 62, 538 (2013).

[5] K.A. Terrani, J.O. Kiggans, Y. Katoh, K. Shimoda, F.C. Montgomery, B.L. Armstrong, C.M. Parish, T. Hinoki, J.D. Hunn, L.L. Snead, "Fabrication and characterization of fully ceramic microencapsulated fuels", J. NUCL. MATER. 426, 268 (2012).

[6] L.L. Snead, T. Nozawa, Y. Katoh, T. Byun, S. Kondo, D.A. Petti, "Handbook of SiC properties for fuel performance modeling", J. NUCL. MATER. 371, 329 (2007).

[7] L. Charpentier, M. Balat-Pichelin, H. Glénat, E. Bêche, E. Laborde, F. Audubert, "High temperature oxidation of SiC under helium with low-β-SiC", JOURNAL OF THE EUROPEAN CERAMIC SOCIETY. 30, 2661 (2010).

[8] L. Charpentier, M. Balat-Pichelin, F. Audubert, "High temperature oxidation of SiC under helium with low-pressure oxygen—α-SiC", JOURNAL OF THE EUROPEAN CERAMIC SOCIETY. 30, 2653 (2010).

[9] J. Selvakumar, D. Sathiyamoorthy, "Prospects of chemical vapor grown silicon carbide thin films using halogen-free single sources in nuclear reactor applications: A review", J. MATER. RES. 28, 136 (2012).

[10] S. Sharafat, R.H. Jones, A. Kohyama, P. Fenici, "Status and prospects for SiC/SiC composite materials development for fusion applications", FUSION ENG. DES. 29, 411 (1995).

[11] A. Owens, A. Peacock, "Compound semiconductor radiation detectors", NUCLEAR INSTRUMENTS AND METHODS IN PHYSICS RESEARCH SECTION A: ACCELERATORS, SPECTROMETERS, DETECTORS AND ASSOCIATED EQUIPMENT. 531, 18 (2004). 47

[12] F.H. Ruddy, A.R. Dulloo, J.G. Seidel, S. Seshadri, L.B. Rowland, "Development of a silicon carbide radiation detector", NUCLEAR SCIENCE, IEEE TRANSACTIONS ON. 45, 536 (1998).

[13] J.B. Casady, R.W. Johnson, "Status of silicon carbide (SiC) as a wide-bandgap semiconductor for high-temperature applications: A review", SOLID-STATE ELECTRONICS. 39, 1409 (1996).

[14] H. Matsunami, T. Kimoto, "Step-controlled epitaxial growth of SiC: High quality homoepitaxy", MATERIALS SCIENCE AND ENGINEERING: REPORTS. 20, 125(1997).

[15] Y. Xu, L. Cheng, L. Zhang, "Carbon/silicon carbide composites prepared by chemical vapor infiltration combined with silicon melt infiltration", CARBON. 37, 1179 (1999).

[16] C.A. Zorman, R.J. Parro, "Micro- and nanomechanical structures for silicon carbide MEMS and NEMS". STATUS SOLIDI B-BASIC SOLID STATE PHYS. 245, 1404 (2008).

[17] J.P. de Villiers, J. Roberts, N. Ngoepe, A.S. Tuling, "Evaluation of the Phase Composition, Crystallinity, and Trace Isotope Variation of SiC in Experimental TRISO Coated Particles", JOURNAL OF ENGINEERING FOR GAS TURBINES AND POWER. 131 (2009).

[18] Z. Dauter, M. Jaskolski, "How to read (and understand) Volume A of International Tables for Crystallography: an introduction for nonspecialists", J. APPL. CRYSTALLOGR. 43, 1150 (2010).

[19] J.K. Shultis, R.E. Faw, "Fundamentals of nuclear science and engineering", MARCEL DEKKER NEW YORK, 2002.

[20] G.L. Squires, "Introduction to the theory of thermal neutron scattering", CAMBRIDGE UNIVERSITY PRESS, 2012.

[21] A.I. Hawari, I. Al-Qasir, V.H. Gillette, B.W. Wehring, T. Zhou, "Ab initio generation of thermal neutron scattering cross sections", PHYSOR 2004: THE PHYSICS OF FUEL CYCLES

AND ADVANCED NUCLEAR SYSTEMS - GLOBAL DEVELOPMENTS, 551 (2004).

[22] A.I. Hawari, I. Al-Qasir, "Graphite thermal neutron scattering cross section calculations including coherent 1-phonon effects". PHYSOR 2008: INTERNATIONAL CONFERENCE ON THE PHYSICS OF REACTORS 2008, 1, 347 (2008).

[23] E. Amaldi, "The production and slowing down of neutrons, in: Neutrons and Related Gamma Ray Problems", SPRINGER, 1959.

[24] R. Macfarlane, R. Boicourt, "NJOY - Neutron and Photon Cross-Section Processing System", TRANSACTIONS OF THE AMERICAN NUCLEAR SOCIETY. 22, 720 (1975). 48

[25] R.E. MacFarlane, A.C. Kahler, "Methods for Processing ENDF/B-VII with NJOY", NUCL. DATA SHEETS. 111, 2739 (2010).

[26] K. Parlinski, Z. Li, Y. Kawazoe, "First-principles determination of the soft mode in cubic ZrO2", PHYS. REV. LETT. 78, 4063 (1997).

[27] K. Parlinski, "Software Phonon", CRACOW (2010).

[28] M. De Graef, M.E. McHenry, "Structure of materials: an introduction to crystallography, diffraction and symmetry", CAMBRIDGE UNIVERSITY PRESS, 2007.

[29] G. Kresse, J. Hafner, "Ab initio molecular dynamics for liquid metals, PHYSICAL REVIEW B". 47, 558 (1993).

[30] G. Kresse, J. Hafner, "Ab initio molecular-dynamics simulation of the liquid-metal– amorphous-semiconductor transition in germanium", PHYSICAL REVIEW B. 49, 14251 (1994).

[31] G. Kresse, J. Furthmüller, "Efficiency of ab-initio total energy calculations for metals and semiconductors using a plane-wave basis set", COMPUTATIONAL MATERIALS SCIENCE. 6, 15 (1996).

[32] G. Kresse, J. Furthmüller, "Efficient iterative schemes for ab initio total-energy calculations using a plane-wave basis set", PHYSICAL REVIEW B. 54, 11169 (1996).

[33] A.A. Maradudin, E.W. Montroll, G.H. Weiss, I. Ipatova, "Theory of lattice dynamics in the harmonic approximation", ACADEMIC PRESS NEW YORK, 1963. 49

APPENDICES 50

Appendix A   Comprehending the C oherent E lastic S cattering F ormula

Equation (1.5)  gives  the form of neutron waves  scattered  from one atom  fixed at

origin.   However, in a scattering  experiment, a beam of  neutrons  is  scattered by a set of

atoms in a sample.  In the situation of multi-atom scattering, each atom, labeled by index j,

will make a contribution to the scattered wave.  When an atom is located at Rj, the scattered

wave contributed by this atom would be

()

,0 0

i i i jj j i

f ee e b eb

fj f j

ij k rR R kr

j Q k R jr R r R .    (A.1)

For simplification purpose, it is assumed that the sample has only one atom per unit cell and

the scattering length is a constant for all atoms.  Under this assumption, the subscript j for bj

can be dropped.  In a neutron experiment, our detector is always far away enough from the

sample to ignore the distance between atom positions.  Then the denominator can be put as r.

Eq. (A.1) is simplified as

0 ,f j i ieb e r

f j r Q k R .    (A.2)

We will start from a two-atom sample to  illustrate  how the interference term in

coherent elastic scattering cross section comes into place.  The differential cross section is

2 2

2 2

2

2 2 2 cos 2 i f

el i i

dAd d d

b ee

b

12QR 1

R 2

Q QR R .

In the situation of three atoms, 51

2

2 2

2 3 2cos 2cos 2cos 3 ii

el idb ee e d b

## 312 QRQR

1 2 3 2

QR 1 3QR R QR R QR R .

As can  be seen,  the first constant  combined with scattering length coefficient  outside of

bracket is Eq. (1.9).  This term attributes scattering without interaction between atoms.  The

rest are cosine terms rising from interference of scattered wave from different atoms.  When

there are N atoms in the sample located at R1…RN, elastic cross section is

2 2

1

N

je i

l

db e dN

jQR .    (A.3)

It should be remarked that three  assumptions  are applied thus far.   The  first is  that

each atom is located at their position without thermal vibration.  The second is that there is

only one atom type with a constant scattering length in the sample, in which there is only one

atom per unit cell (Bravais lattice).  The third approximation should be very accurate, which

is the far field approximation.

In the situation of  a crystal structure, summation in  Eq.  (A.3)  can be greatly

simplified by the periodicity of a crystal lattice.  Utilizing the same trick from Eq. (2.32) to

(2.33), Eq. (A.3) is

, 0 3 2() (2 ( ) ) el cohd b dv

fk Q τ .    (A.4)

An upper index  has been attached to the symbol of the differential cross section in (A.4) in

order to  recall that  it represents the contribution arising from  the reflection  of planes of

Miller indices  (h,k,l).   In a polycrystalline material, scattering with all directions  of    is

52

equally possible.   Equation (A.4)  is, therefore, another  version of Eq.  (2.34)  with the

proposed approximation applied.  Hence, expression  (A.4)  should  be averaged over all

direction of τ in reciprocal space.

,,

0

2 2

## 1 sin

4 (2 sin 2 )

el coh el coh

av i

dd

dd d

b k v d

.    (A.5)

Equation (A.5)  tells a scattering  event with the  magnitude  of  , and scatter angle of  .   In

order to obtain the total scattering cross section regardless the direction of scattered neutron,

Eq. (A.5) should be integrated over all directions of scattered neutron in real space.

2 0

2 , , 2 sin

2 22

1 el coh el coh av av

d d

b v d

.    (A.6)

The total coherent elastic cross section is finally obtained by summing all the

22

, 20 1 2 el coh b v

.    (A.7)

When introducing multiple atoms into a unit cell, the structure factor should come into place

in  Eq.  (A.7).   Furthermore, by  considering thermal vibration of atoms around their

equilibrium position, Debye-Waller factor will be introduced.  In conclusion,

, 20

2 2 ( ) 1 2 el coh v F

(A.8)

in which  ( )F   is shown in Eq. (2.35). 53

Appendix B   Comparison of D ifferent C oherent E lastic S cattering F ormula

In this appendix, it will be demonstrated that Eq. (2.34), (A.8) and (3.6) (listed as Eq.

(B.1) ~ (B.3) below) are essentially the same.  Though complete theoretical derivation yields

Eq.  (2.34), this expression is not capable to capture the characteristic of  programming

language.  Therefore, a programming friendly equation, i.e., Eq. (3.6), is needed to actualize

out our calculation.

2 3

0

2 ( ) ( )

coh el

d FN dv

τ κ τ κ .    (B.1)

, 20

2 2 ( ) 1 2 el coh v F

κ , where  ( ) WiF be e

κ dκ .  (B.2)

41 , i

i wEi coh i EE i

f E e E

.    (B.3)

By comparing Eq. (B.1) and (B.2),  it can be found that the difference between them

is the same as that between (A.4) and (A.7).  That is, Eq. (B.1) is the differential cross section

per unit cell with fixed neutron momentum change =τ, while Eq. (B.2) is the total scattering

cross section per unit cell with all possible neutron momentum  change.   By applying the

same integral from Eq. (A.4) to (A.7), Eq. (B.2) can be derived from Eq. (B.1).

Starting from Eq. (B.2), by using  2Em  , plugging in F(), and assuming that

there exists  a  universal  Debye-Waller factor for all atoms in  the unit cell, following

derivation can be made 54

2 2

2

, 20 2 2 2 22 2

20 2 2

0

2 0

22

1 2

11

11

1 ( ) (

1 ) Wi Wi

el coh v

E mv be e E mv be e E m

F F

v

κ d

κ d

κ κ 

  ,

while Eq. (B.3) is

2 2

10

2

4 4

1 ,

## 11 i

i j

i

i wEi coh EE i N i wE j EE j

f E e E ee E N mv

τ r .

Comparing the above two equations,  the  only difference except  the Debye-Waller factor  is

that Eq. (B.3) from NJOY is calculating averaged total coherent elastic cross section in a unit

cell, while Eq. (B.1) from Squires is calculating total cross section from all atoms in a unit

cell.   The  Debye-Waller factor in both  equations  looks  different because of different

terminology.   This difference  can  be examined by comparing  Eq.  (3.1)  ~  (3.3)  with Eq.

(2.38).

NJOY Eq. (3.1) ~ (3.3):

22

22 0 1 ( ) cot

4

## 2 h( )

4 2

8Bn

B

wE Ak T m

M d k T



 .

Squire Eq. (2.38): 55

By using  Bk T ,  () ( ) Bk T   ,  Bd k Td , it can be written that

2 2 0

0 0

2

2

2 2 coth 42

## 1 coth

22

2 () d

( ) d

1 ( ) coth( ) 2

m

m B B

BB

B

W M k T k T M k T k T d M k T

.

The two Debye-Waller factors are exactly the same.  Therefore, the only difference between

Squire’s cross section and NJOY’s cross section is the first one per unit cell and the second

one per atom.

56

Appendix C   Discussion of ENDF F ormat

In this appendix, features of ENDF library are discussed.  It should be addressed to the

reader beforehand that this appendix is not  intended  to substitute the “ENDF-6 Formats

Manual” published by National Nuclear Data Center,  Brookhaven National Laboratory.

What this appendix will do is, a) explain basic structure of ENDF libraries, b) address some

of the important parameters in the ENDF libraries, and c) give readers general directions on

how to generate ENDF libraries based on NJOY code system.   Readers should understand

when talking about ENDF libraries in this work, it is always referred to the Thermal Neutron

Scattering Sublibrary, which is also called the “file 7 thermal neutron scattering law data”.

Tables shown in this appendix are parts of an already published ENDF library of Be.

57

Table 4. Beginning section of ENDF library.

Every ENDF library begins with a description section looks like the one shown above.

a

b  c d

g e  f f f 58

The number 26 on the right hand side column denotes the material number for this library.  In

this case, it is number  26 for  Be.  The number 1451 in the next  column refers to the

description section of an ENDF library.  The right most column is the line number.

The first line of the library gives information on the version of the library and the date

the library is published.  From line 5 to line 47, there are 43 lines of text description of the

library.  Line number 099999 given below line 50 indicates the end of a section.

Between line 1 to 4 and line  48 to 50,  there are numbers given in the table.  These

numbers have different meanings and they might be mistakenly outputted by NJOY program.

Therefore, it is important for users to check their meaning and write the right number when

generating  the library.    This appendix will only emphasize  known parameters that  will be

probably modified by hand by the user of NJOY program.

a.  NMOD: Modification number for this material:

NMOD=0, evaluation converted from a previous version;

NMOD=1, new or revised evaluation for the current library version;

NMODσ 2, for successive modifications.

NOTE: For a newly generated library, users should put 1. NJOY outputs 0.

b.  LREL: Library release number; for example, LREL=2 for the ENDF/B-VI.2 library.

NJOY default outputs  0, users should change  it  to 1 for  the current library release

number.

c.  NVER: Library version number; for example, NVER=7 for version ENDF/B-VII.

NJOY default outputs 6, users should revise it to 7 for the current library version.

d.  NWD: Number of records with descriptive text for this material. Each record contains

59

up to 66 characters.

NJOY default output is 0. Users should change it to number of descriptive lines in

this section.  In this example, it begins at line 5, ends at line 47.  There are 43 lines of

records, therefore users should put 43 here.

e.  NC1: ENDF reaction designation of the 1 st section, which indicates number of lines

for the first description section.

Default description section will always be modified, in this sense users must change

this number to corresponding line number.  In this example, it is 43 lines.

f.  MODn:  Modification indicator for the nth section. The value of MODn  is equal to

NMOD if the corresponding section was changed in this revision. MODn must always

be less than or equal to NMOD.

NJOY default outputs 0. For a new library, change it to 1.

g.  NC1: This depends on specific situation. But for a file 7 library, if users made

modifications on  section 2, i.e.,  the coherent elastic scattering cross  section, users

must change this number to corresponding line numbers in section 2.

60

Table 5. Coherent elastic section of ENDF library.

Numerical modification of this section is needed if the users intend to publish a library by a

set of separated libraries.  For example, the library of 3C-SiC is generated by two parts, i.e.,

Si cross section and C cross section.  By generating the total coherent elastic cross section for

SiC from NJOY, users must divide the cross section by two and put them separately into Si

and C libraries.  In this example, there are 149 entries for the record of cross section.  The

record begins at line 4.   1.592731×10-3  eV  is the  energy and  1.50846×10-16  barn   is the

associated cross section.  These two numbers are called an entry for the record.  As discussed

above,  in the 149  entries, users may want to divide  the cross section  values  and  put them

back at the same place. 61

Table 6. Inelastic section of ENDF library.

There is nothing known for the users to revise from NJOY output in file 7 section 4.

However, one thing to must be reminded to the readers is the α grid which is in the red box

shown above appears only once in  the library.   In the rest  of the library, it will implicitly

default that all cross sections follow the same α grid.

62