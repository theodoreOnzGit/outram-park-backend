// Numbers for the neutronics lessons, page 22 (gh:#906), written by
// crates/njoy-outram-park-fork/examples/lesson_doppler.rs (regenerate with it;
// do not edit). U-238 (ENDF/B-VIII.0, AWR 236.0058) capture near its 6.674 eV
// resonance: RECONR at 0 K, BROADR's kernel at 293.6 and 1200 K; peak [b] at
// ep [eV], fwhm [eV], area 5.5-8 eV [b eV]. onev: the largest relative change
// of a pure 1/v cross section broadened the same way, 0.01-10 eV.
window.DOPPLER_DATA = {"awr":236.0058,"u238":{"0":{"ep":6.6742800000006675,"peak":22763.470000002275,"fwhm":0.0242042116477883,"area":860.5000457117884},"293.6":{"ep":6.6742800000006675,"peak":7195.585217133042,"fwhm":0.10271534458767739,"area":860.56102084054},"1200":{"ep":6.673524000000667,"peak":3997.208190149023,"fwhm":0.193349723943383,"area":860.5759038958879}},"onev":{"293.6":0.000051776701101013956,"1200":0.00005177670158817982}};
