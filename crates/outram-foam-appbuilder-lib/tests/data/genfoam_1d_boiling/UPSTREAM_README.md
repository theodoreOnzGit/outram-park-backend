# 1D Boiling

## Description

This test case portrays the two-phase capabilities of GeN-Foam. It consists of a 1D channel with a pressure-driven flow of liquid sodium. A power source is turned on at time 0 and eventually leads to boiling, with flow excursion (as the flow is pressure driven). After a certain time elapses (see `constant/phaseProperties.structureProperties.powerOffCriterionModel`), the power is turned off.

For further info see comments in [`constant/fluidRegion/phaseProperties`](constant/fluidRegion/phaseProperties),
[`system/controlDict`](system/controlDict), [`system/fluidRegion/fvSolution`](system/fluidRegion/fvSolution).
