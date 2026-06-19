# Feedback Mechanism Readme 


# Theory 

From Lamarsh:

Lamarsh, J. R. (1975). Introduction to nuclear engineering.

3rd edition, we refer to chapter 7 page 328 to 330.

There are several feedback mechanisms 

## Short term feedback (seconds to minutes)

1. Delayed Neutrons 
2. Fuel Temperature Feedback
3. Void/Density feedback of moderator, fuel and reflector 
4. Control Rod feedback
5. Leakage feedback

I hope to make this general as possible, even with molten salt breeder 
reactors (MSBRs) such as the liquid fluoride thorium reactor (LFTR) in  
future. But for now, start with solid fuel first.


## Medium term feedback (1-2 days)

6. Reactor poison feedback 
7. Burnable poison/absorber feedback 

These would include soluble boron, Xe-135 or iodine pit and samarium 
poisoning.

## Long term feedback (3 days to several months)

8. Fuel depletion
9. Fuel breeding

# Practical parts 


## Real-Time Simulation

On the matter of real-time simulation, we can only do thermal reactors...

## Implementing Various Feedback Mechanisms in Code

Let us first not consider time requirements first, but accuracy. 
Then we deal with real-time requirements later with 
appropriate surrogate modelling.

Real-Time requirements aside, how to obtain feedback?

Well, each feedback mechanism will affect the six-factor formula in some 
way.

k\_eff = epsilon * P\_FNL * p * P_\TNL * f * eta

eg. 
1. Delayed Neutrons --> already accounted for in SixGroupPRKE struct
2. Fuel Temperature Feedback --> will affect resonance escape probability
3. Void/Density feedback of moderator, fuel and reflector  --> affects thermal utilisation factor
4. Control Rod feedback --> affects thermal utilisation factor (fast neutron absorption XS is low)
5. Leakage feedback --> affects P_\FNL, and P_\TNL
6. Reactor poison feedback --> affects thermal utilisation factor and/or eta (neutron reproduction factor)
7. Burnable poison/absorber feedback --> affects thermal utilisation factor and/or eta (neutron reproduction factor)
8. Fuel depletion --> affects neutron reproduction factor
9. Fuel breeding --> affects neutron reproduction factor

For the last two, we may use longer timesteps to calculate inventory, so 
not crucial. For short transients, not really important. I'm prioritising 
short and medium term transients first (items 1 through 7). Though 
1 is already accounted for in Six Group PRKE at least for solid fuel.
Items 2-7 can be programmed.


I suppose the general mechanism is to have a function taking in 
whatever quantifies the feedback, eg. a temperature parameter, void 
parameter (vapour fraction), etc. and returns a term in the six factor 
formula. 

The six factor formula will then be appropriately multiplied in order 
to get k\_eff. Then from this we obtain reactivity.

The PRKE function then takes in the reactivity through the 
construct\_coefficient\_matrix() function or method. Therein, the neutron 
population at the next timestep is calculated. In this manner, the timestep 
control is more of an explicit scheme. It is simple but we can make do.

Question now is how do I see how each term affects the six factor 
formula?


### Fuel Temperature Feedback

Probably OpenMC is my best friend here. I can use the tallies and roughly 
define neutrons within resonance, fast and thermal region.

Without any leakage, I can simulate an infinite lattice, and then 
see how the fuel temperature changes the four factor formula terms
which is relevant for an infinite lattice.

### Void/Density Feedback of moderator, fuel, reflector etc.



## Verification and Validation 





