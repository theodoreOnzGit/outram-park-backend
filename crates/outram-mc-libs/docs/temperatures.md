# Temperatures in outram-mc-libs: which one takes precedence

A model can carry five temperatures. They are **not** alternatives for the
same thing: each controls a different part of the physics, and one of them
controls nothing. Checked 2026-09-27 by reading `physics/transport_csg.rs`,
`physics/keff.rs` and `material/nuclide.rs`, and by measurement in
`tests/temperature_precedence.rs`.

| Temperature | Set by | Controls | Pointwise nuclide | Multipole (`Core`) nuclide |
|---|---|---|---|---|
| **Nuclide build temperature** | `Nuclide::from_endf_file(path, name, T, tol)`; an ACE table's own temperature | Doppler broadening baked into the pointwise table | **This one wins** | not used for the resolved range |
| **S(α,β) table temperature** | `ThermalScattering::from_endf_file(.., T, ..)` / `from_leapr(.., T, ..)` | Which bound-scattering table is used (nearest tabulated, reported by `selected_temperature_k`) | used | used |
| **`Material::temperature`** | the material | Passed to every cross-section lookup (flight distance and collision) | **ignored** | **this one wins** (WMP Doppler evaluator) |
| **`KeffSettings::temperature_k`** | the run | CSG drivers: free-gas elastic target motion (`free_gas_kt`) only. Simple drivers in `keff.rs`: also the lookup temperature | kinematics only | kinematics (and, in the simple drivers, the lookup) |
| **`Cell::temperature`** | the cell | **Nothing in transport** | not read | not read |

## Measured (2026-09-27)

`tests/temperature_precedence.rs`: Godiva from the committed NJOY2016 ACE
tables at 293.6 K, 1000 × [5 + 10], one seed. One temperature changed to
1200 K per arm:

| Arm | k |
|---|---|
| everything 293.6 K | 1.014852 ± 0.019094 |
| `Cell::temperature` = 1200 K | 1.014852 ± 0.019094 (bit-identical) |
| `Material::temperature` = 1200 K | 1.014852 ± 0.019094 (bit-identical) |
| `KeffSettings::temperature_k` = 1200 K | 1.047207 ± 0.016045 (differs) |

The first two arms are asserted bit-identical and the third asserted
different, so a change in any of these behaviours fails the test.

## Re-measured (2026-09-30): the model gained H-1

~~The table above~~ is superseded. On bare Godiva the third arm had gone
blind: after the ACE-route fixes (#365, #366, #407) all four arms gave a
bit-identical k = 0.982577, because no collision in the short run reached
the free-gas range. The test now adds H-1 at 5.0e-3 atoms/(b·cm) from
`reference-data/endf`. A neutron-mass target is a free gas at every energy.
The conclusions are unchanged:

| Arm | k |
|---|---|
| everything 293.6 K | 1.014268 ± 0.017209 |
| `Cell::temperature` = 1200 K | 1.014268 ± 0.017209 (bit-identical) |
| `Material::temperature` = 1200 K | 1.014268 ± 0.017209 (bit-identical) |
| `KeffSettings::temperature_k` = 1200 K | 1.013499 ± 0.007813 (differs) |

## The rule for a user

**To run a model at temperature T, build (or read) the nuclides at T, load the
S(α,β) tables at T, and set `KeffSettings::temperature_k = T`.** Set
`Material::temperature = T` as well, which matters only for multipole
nuclides. `Cell::temperature` is descriptive only.

A model with different temperatures in different regions needs one nuclide
set per temperature, attached to the materials of those regions. The free-gas
kinematics then still use the single run temperature. That is a known
limitation (OpenMC resolves temperature per cell; gh:#269 tracks
multi-temperature treatment).

## HTR-10 (`nee_soon`)

`htr10_rmc_keff` builds every nuclide and S(α,β) law at `TEMP_K` = 300.15 K
(27 °C, as Li, Yu & Wei 2014 state) and sets the run temperature to the same.
The geometry's cells say 293.6 K, which is inert by the table above. The model
is consistent at 300.15 K.
