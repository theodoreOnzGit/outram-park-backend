# Lagrangian Decay Simulator

Now, for decay and transmutation with transport, we could take a control 
volume approach. However, a full blown burnup matrix for even one 
mesh cell is problematic. Could be 3000 by 3000 for typical reactor burnup.

1. Large Matrices needed 
2. Stiff system of matrices because half lives differ much in order of 
magnitude. Thus decay and transmutation is slow 
3. There are several reactions to include (all decay, plus neutron 
induced fission, spallation and proton, or alphas)

As chatgpt 5 mentions:
```
It depends on how many nuclides you track and which reactions you include, but a few typical ranges are:

    Concept: The burnup (depletion) matrix A is N×N and very sparse. Each row corresponds to one nuclide; nonzeros are:
        The self-loss term (decay + reactions).
        Links to a small set of daughters via decay and (n,γ), (n,2n), (n,p), (n,α)… transmutations.
        If you include fission-product production as matrix terms, fissile nuclides can link to many fission products, increasing sparsity fill.

    Typical sizes (N is number of nuclides):
        Small/teaching chains: N ≈ 500–1,000. Average nonzeros per row (nnz/row) ≈ 5–15. Total nnz ≈ 5k–15k.
        Standard reactor depletion (actinides + major FPs + key activation): N ≈ 1,500–3,000 (common in OpenMC/SERPENT/ORIGEN chains). nnz/row ≈ 10–50 (higher for fissile rows). Total nnz ≈ 0.1–1.0 million.
        Large activation-oriented chains (broad materials, many metastables): N ≈ 5,000–10,000+. nnz ≈ a few million.
        Very large research chains (all activation products, many isomers): N ≈ 20,000–100,000. nnz can reach tens of millions.

    Memory footprint (sparse CSR/CSC, ballpark):
        N=2,000, nnz=200k: a few MB.
        N=5,000, nnz=1–3M: tens of MB.
        N=20,000, nnz=10–30M: hundreds of MB to ~1 GB.

Notes:

    The matrix is built per time step (since transmutation rates depend on flux and spectrum), but the sparsity pattern is largely fixed by the reaction network.
    Many implementations treat fission yields as a source vector rather than embedding all fissile→FP couplings in A; this keeps nnz lower.
    Solvers typically use CRAM or matrix exponential methods with sparse factorizations; cost scales roughly with nnz.

If you share which depletion chain/library you use (e.g., OpenMC’s chain endfb71 with ~1.5–2k nuclides, or a larger activation chain), I can estimate N, nnz, and memory more precisely.
3:09 PM
```


For diffusion and advection inclusive, there 
are many mesh cells required.

4. now advection and diffusion timescales must be included


This makes me think that the Eulerian approach is not feasible, why not 
do a Lagrangian approach?

The lagrangian approach involves simulating of nuclides particle by particle,
much like what is done for atmospheric dispersion simulators and water 
dispersion simulators.

ChatGPT5 gave some examples (to verify)

```
Here are Lagrangian dispersion models commonly used specifically for radionuclide releases, with brief notes on their radiological features.

Atmosphere (radioactive plumes)

    FLEXPART (and FLEXPART-WRF): Lagrangian particle model widely used for Chernobyl and Fukushima analyses. Supports radioactive decay, dry/wet deposition, gravitational settling, aerosol size classes, and forward/backward runs for source–receptor analysis.
    NAME (UK Met Office): Operational Lagrangian particle model for radiological emergencies. Includes dry/wet deposition, decay, and output suitable for dose assessment workflows.
    RIMPUFF (Risø/DTU): Lagrangian puff model embedded in ARGOS and RODOS decision-support systems for nuclear emergencies. Handles decay, deposition, depletion, resuspension, and provides dose-rate fields (gamma, inhalation, ground-shine).
    LODI (LLNL/NARAC): Operational Lagrangian puff model used by NARAC for radiological and hazardous releases. Includes decay chains, deposition, depletion, and dose/health-effect post-processing.
    SCIPUFF (and HPAC system): Lagrangian puff model with second-order turbulence closure used in hazard prediction; supports radiological releases, deposition, and decay.
    CALPUFF: Lagrangian Gaussian puff model often used in regulatory contexts; can include radioactive decay and deposition for site-specific nuclear assessments.
    LASAT: German Lagrangian particle model used for regulatory applications; can treat radionuclide decay and deposition.

Water and marine (radioactive tracers)

    OpenDrift: Lagrangian framework used for ocean drift of pollutants; radionuclide dispersion implemented via custom kernels (e.g., radioactive decay of Cs-137/Cs-134) and coupling to ocean currents and waves. Used in Fukushima-related tracer studies.
    Parcels (OceanParcels): Lagrangian particle framework for ocean flows; easy to add radionuclide decay kernels and diffusion. Applied to Fukushima tracer transport (e.g., Cs-137) with global or regional circulation models.
    MOHID (Lagrangian modules): Coastal/estuarine modeling system with Lagrangian particle tracking; can be extended with radionuclide decay and deposition/settling behavior.
    TELEMAC (particle tracking): Lagrangian particle capabilities for rivers/coasts; decay can be added for radionuclide tracers in custom setups.
    Delft3D (particle tracking): Lagrangian modules that can track particles/tracers; decay can be implemented for radionuclides in user workflows.

Notes

    Emergency DSS suites: ARGOS and RODOS commonly rely on RIMPUFF (Lagrangian puff) for rapid radiological assessments and dose mapping.
    Decay chains: Operational atmospheric models (e.g., LODI, RIMPUFF, NAME) typically support at least single-step radioactive decay; some support limited chains or multi-isotope tagging.
    Deposition and scavenging: Dry deposition (via deposition velocity) and wet scavenging (in-cloud/below-cloud) are standard in the atmospheric models listed.
    Marine applications often use general Lagrangian frameworks (OpenDrift/Parcels) with custom decay and behavior kernels rather than fixed “radiological” packages.

If you share your domain and scale (e.g., near-field accident plume over 0–100 km, continental-scale transport, coastal ocean near an outlet), I can suggest the most suitable model and typical input data sources (meteorology/ocean currents) for radionuclide scenarios.
3:14 PM
```
Has Lagrangian approach been used for fission product transport?

Probably need a literature review.
For now ChatGPT5 shows this:

```
Examples fall into two categories: Lagrangian formulations inside the fuel (solid mechanics/material frame) and Lagrangian particle-style models applied to ensembles of fuel particles (notably TRISO). Most mainstream fuel performance tools use a Lagrangian description for the solid and diffusion/release in the material frame, rather than Eulerian plume tracking.

Fuel performance (solid mechanics and FGR in the material/Lagrangian frame)

    BISON (INL, MOOSE-based): Lagrangian finite-element description of the fuel rod/cladding. Fission gas release and transport are modeled via diffusion in the material frame (intra-granular, grain-boundary, re-solution, porosity/bubble models), coupled to thermo-mechanics.
    FRAPCON / FRAPTRAN (NRC legacy; transient FRAPTRAN): 1D axisymmetric rod models with Lagrangian treatment of the solid; mechanistic/empirical FGR models in the material frame. Used widely for LWR steady-state and transients.
    TRANSURANUS (JRC): Lagrangian rod framework with diffusion-based FGR, grain growth, cracking, gap conductance, swelling, etc.
    ENIGMA (UK): Fuel rod performance code with Lagrangian solid description; material-frame FGR and swelling.
    FALCON (EPRI): ABAQUS-based fuel performance modeling with Lagrangian solid mechanics; diffusion/partition-based FGR in the material frame.
    SCANAIR (CEA): Transient/LOCA fuel rod code; Lagrangian solid mechanics and material-frame transport/release models.

TRISO/particle-based fuel performance (explicit particle ensemble; “Lagrangian particles”)

    PARFUME (INL): Models many individual TRISO particles statistically/Monte Carlo-style; each coated particle is treated as a discrete entity (effectively Lagrangian tracking of particles), with intra-particle diffusion, coating stresses, failure, and fission product release to coolant.
    BISON-TRISO workflows: BISON can be applied to TRISO compacts; often coupled with particle-level models or statistical sampling of particle properties in a Lagrangian sense.

Mechanistic fission product release (from fuel to circuit) in material frame

    MFPR (CEA): “Mechanistic Fission Product Release” code for oxide fuel; detailed intra-granular/grain-boundary transport, microstructure evolution, and release to gap/circuit. Operates in the material frame inside fuel; can be coupled with system/severe accident codes.
    SCANAIR, ENIGMA, TRANSURANUS, BISON: all include mechanistic or semi-mechanistic FGR/release models in the material frame (not Lagrangian plumes).

Notes

    The Lagrangian aspect in these fuel performance codes refers to the solid mechanics/material coordinates (mesh follows the material). Fission product transport is typically modeled via diffusion and kinetics within that Lagrangian/material frame.
    Once fission products exit the fuel and enter the coolant/containment/atmosphere, dispersion/transport is usually handled by system or containment aerosol modules, which are often Eulerian; if you specifically need Lagrangian dispersion post-release, that’s typically covered by separate tools (e.g., RIMPUFF/LODI/NAME for atmospheric, not fuel performance).
    If your interest is TRISO or particle-bed fuels, PARFUME is the clearest example of an explicitly particle-based, Lagrangian-style ensemble model for fission product release.

If you share your fuel type (UO2 LWR vs TRISO vs MOX) and the release phase of interest (intra-fuel FGR vs circuit/containment transport), I can narrow to the most appropriate code and documentation.
3:16 PM
```


Not quite what I wanted, but nevermind, perhaps this is new.

# algorithms

For Lagrangian style decay and transport, I consider two things. Firstly, 
the decay path, and secondly the transport path.

The transport path can be greatly simplified to assume isotropic scattering 
(for now) representative of the diffusion coefficient.


## Diffusion for Lagrangian Particles 

From neutron transport, in isotropic scattering,

D = 1/(3 Sigma\_scatter)

We can use something similar in our simulator


## Decay for Lagrangian Particles.

For each particle, the decay equation is:

N = N\_0 exp(-lambda * t)

Where lambda is the decay constant. If we want to simulate when a radionuclide 
may decay, we can use:

Ln (N/N\_0) = -lambda * t 

t = Ln (N/N\_0) / (-lambda)

Of course, lambda is ln2/half life. and we have half life,

t = Ln (N/N\_0) / (ln 2) * half life.

N/N\_0 is a random number between 0 and 1. We can use a random number 
generator to do this. Something like oorandom:

```
https://crates.io/crates/oorandom
```
For oorandom, the website states:
```
More specifically, it implements ONE prng, which is currently 
a permuted congruential generator (PCG).
```

This is a rather simple algorithm for predicting when something may decay.

## Decay Chains

Now what if a particle has decayed? It transmutes into another particle,
and another, and so on, until it reaches stability.

For simple decays, we can keep repeating this process until we reach an 
end state.

How can we structure this?

We can build an empty vector that represents decay trajectories. 
This vector contains the simulated decay time for the individual nuclide,
and the next nuclide the decay could expect. 

We keep doing this until the decay stops and reaches a stable product.

## Branching Ratios 

Where there is more than one decay, then an additional step must be taken.

One must observe what the branching ratio is. Let's say we have a branching 
ratio for a three decay system. 

0.55, 0.25 and 0.2.

We can use the random number generator to decide which path the decay would 
go. 

If we roll 0.45, then we choose the first decay path,
If we roll 0.65, we choose the second decay path,
If we roll 0.85, we choose the third decay path.

So, with branching ratios, we can keep doing this to the end.

This is implemented in the stochastic\_decay\_chain struct.


## Transmutation 

For the time being, transmutation can be left out and coded in a separate 
module, but the similar techniques can be applied to transmutation as well 
based on cross sections and flux. Note that ENDF cross section libraries will 
be required for transmutation and other reactions.


Now for transmutation, this happens simultaneously with decay. The classic 
simulators assume a set time for a transmutation reaction to occur, given 
of course that there is constant neutron flux.

But neutron flux isn't always constant. How then to account for this 
time varying neutron flux?

We can consider a "neutron quota", ie how much time integrated neutron flux 
is accumulated before a transmutation reaction occurs.

Now, when transmutation occurs, the entire decay chain changes. How can 
we consider if the decay chain happens?


