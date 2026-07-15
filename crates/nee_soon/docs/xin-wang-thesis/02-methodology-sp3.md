<!--
PROVENANCE / AI-ASSISTED EXTRACTION NOTICE
==========================================
Source : Xin Wang, "Coupled neutronics and thermal-hydraulics modeling for
         pebble-bed Fluoride-Salt-Cooled, High-Temperature Reactor (FHR)",
         Ph.D. dissertation, UC Berkeley, 2018.
         https://escholarship.org/uc/item/40q3985m  (open literature)
This is an AI-ASSISTED, condensed extraction of Chapter 2 + Appendix D. It is
UNVERIFIED draft material — check every equation and symbol against the source
PDF before relying on it (workspace AI_USAGE.md rule). Equation numbers are the
dissertation's own. Where the original print appears to contain a typo, this is
flagged inline rather than silently "corrected".
-->

# Chapter 2 methodology — kinetics, diffusion, $SP_3$, MGXS, porous-media TH

Wang builds three fidelity levels. This file captures the parts the OUTRAM PARK
reproduction needs: the reflector-corrected point kinetics, the full-core
multi-group diffusion, the **$SP_3$** correction, the Monte-Carlo → multigroup
cross-section (MGXS) recipe and its feedback parametrisation, the porous-media TH
closures, and the multi-scale fuel-temperature model. Appendix D gives the COMSOL
user-PDE coefficient forms of the $SP_3$ system.

## 2.1 Multi-point (reflector-corrected) kinetics

Standard point kinetics (Eq. 2.1):

$$\frac{dP(t)}{dt} = \frac{\rho(t)-\beta}{\Lambda}P(t) + \sum_{i=1}^{D}\lambda_i C_i(t)$$

$$\frac{dC_i(t)}{dt} = \frac{\beta_i}{\Lambda}P(t) - \lambda_i C_i(t)$$

FHRs have large graphite reflectors that host neutrons during moderation and
return them to the core, lengthening the effective lifetime. Wang adds one
**fictitious delayed group** for the reflector return (Eq. 2.2):

$$\frac{dP(t)}{dt} = \frac{\rho(t)-\beta-\rho_R}{\Lambda_c}P(t) + \sum_{i=1}^{6}\lambda_i C_i(t) + \lambda_R C_R(t)$$

$$\frac{dC_i(t)}{dt} = \frac{\beta_i}{\Lambda}P(t) - \lambda_i C_i(t)$$

$$\frac{dC_R(t)}{dt} = \frac{\rho_R}{\Lambda_c}P(t) - \lambda_R C_R(t)$$

with reflector reactivity gain and prompt lifetime split (Eqs. 2.3–2.4):

$$\Lambda_{prt} = (1-\rho_R)\Lambda_c + \rho_R\Lambda_R \qquad \rho_R = \frac{k_{eff}-k^{c}_{eff}}{k_{eff}}$$

Here $\Lambda_c$ is the core-only generation time, $\Lambda_R$ the combined
core+reflector lifetime, $k^{c}_{eff}$ the eigenvalue of the core with no
reflector, and $\lambda_R = 1/\Lambda_R$. Table 2.1 shows how strongly the Mk1
reflectors move $k_{eff}$ and lifetime (fuel + both reflectors: $k_{eff}=1.03$,
$459\ \mu s$; fuel only: $k_{eff}=0.73$, $227\ \mu s$).

Reactivity is the sum of external insertion plus per-component temperature
feedback with **constant** coefficients (Eq. 2.5):

$$\rho(t) = \rho_{ext}(t) + \alpha_F(T_F(t)-T_{F,0}) + \alpha_M(T_M(t)-T_{M,0}) + \alpha_c(T_C(t)-T_{C,0})$$

(F = fuel/Doppler, M = moderator, C = coolant/flibe). This 0-D model is the
`teh-o-prke` analogue and drives the unit-cell studies; the full-core transients
(incl. Fig. 4.29) use the spatial model below.

## 2.2 Full-core multi-group neutron diffusion

Group flux is the energy-integrated flux over group $g$ (Eq. 2.18):
$\phi_g(r,t) = \int_{E_g}^{E_{g-1}}\phi(r,E,t)\,dE$. The eigenvalue (criticality)
multi-group diffusion equation is (Eq. 2.21):

$$0 = \nabla D_g\nabla\phi_g - \Sigma_{a,g}\phi_g - \sum_{g'\ne g}\Sigma_{s,gg'}\phi_g + \sum_{g'\ne g}\Sigma_{s,g'g}\phi_{g'} + \frac{1}{k_{eff}}(1-\beta)\chi_{t,g}\sum_{g'=1}^{G}(\nu\Sigma_f)_{g'}\phi_{g'}$$

with delayed-precursor balance (Eq. 2.20):

$$\frac{\partial C_i}{\partial t} = -\lambda_i C_i + \beta_i\sum_{g=1}^{G}(\nu\Sigma_f)_g\phi_g$$

Symbols: $v_g$ group speed; $\Sigma_{s,gg'}$ scatter $g'\to g$; $\nu$ neutrons
per fission; $\Sigma_{f,g}$ fission XS; $\chi_g$ fission spectrum (prompt $p$ /
delayed $d$); $\Sigma_{t,g}$ total XS; $D_g$ diffusion coefficient; $\beta_i$,
$\lambda_i$ delayed-group data; $G$ groups, $D$ delayed groups. (The transient
diffusion form Eq. 2.19 adds $\frac{1}{v_g}\partial_t\phi_g$; note the printed
2.19 in-scatter term reads $\phi_g$ where the standard source is $\phi_{g'}$ —
flagged as a likely print typo.)

## 2.2.2.2 The $SP_3$ correction (key for control rods)

Fick's-law diffusion breaks down in strong absorbers such as control rods. Wang
uses the **simplified $P_3$ ($SP_3$)** approximation there: more accurate than
diffusion, far cheaper than $S_N$ / $P_N$, and solvable with the same tooling.
The multi-group $SP_3$ system with delayed neutrons (Eq. 2.22) is written in two
coupled moment fields $\phi_{0g}$ (0th) and $\phi_{2g}$ (2nd):

$$-\nabla D_{1g}\nabla(\phi_{0g}+2\phi_{2g}) + \Sigma_{rg}\phi_{0g} = \frac{\chi_g}{k_{eff}}\sum_{g'=1}^{G}\nu_{g'}\Sigma_{fg'}\phi_{0g'} + \sum_{g'\ne g}^{G}\Sigma_{s,g'g,0}\phi_{0g'}$$

$$-\nabla D_{2g}\nabla\phi_{2g} + \Sigma_t\phi_{2g} = \frac{2}{5}\left(\Sigma_{rg}\phi_{0g} - \frac{\chi_g}{k_{eff}}\sum_{g'=1}^{G}\nu_{g'}\Sigma_{fg'}\phi_{0g'} - \sum_{g'\ne g}^{G}\Sigma_{s,g'g,0}\phi_{0g'}\right)$$

$$\frac{\partial C_i}{\partial t} = -\lambda_i C_i + \beta_i\sum_{g=1}^{G}(\nu\Sigma_f)_g\phi_g$$

with the moment diffusion coefficients and removal XS:

$$D_{1g} = \frac{1}{3(\Sigma_{tg}-\Sigma_{s0g})} \qquad D_{2g} = \frac{9}{35(\Sigma_{tg}-\Sigma_{s3g})} \qquad \Sigma_{rg} = \Sigma_{tg} - \sum_{g'=g}^{G}\Sigma_{s,g'g}$$

$\Sigma_{s0g}$ and $\Sigma_{s3g}$ are the $P_0$ and $P_3$ self-scatter Legendre
moments. The physical scalar flux is $\phi_{0g}$; the field that satisfies the
diffusion-like leakage operator is the composite $\phi_{0g}+2\phi_{2g}$.

> **OUTRAM PARK cross-reference.** The GeN-Foam $SP_3$ port
> (`outram_foam_appbuilder_lib::genfoam::neutronics::sp3`) solves the same system
> with GeN-Foam's variable naming: `fluxStar` $=\Phi_{0g}=\phi_{0g}+2\phi_{2g}$
> (composite 0th moment), `fluxStar2` $=\phi_{2g}$ (2nd moment), reconstruction
> $\phi_{0g}=\Phi_{0g}-2\phi_{2g}$. GeN-Foam writes the second-moment operator as
> $D_2 = (3/7)/\Sigma_t$ with $A_2 = (5/3)\Sigma_t + (4/3)\Sigma_r$, a different
> but algebraically related $SP_3$ convention from Wang's $D_{2g}=9/(35\Delta)$
> form. **The two conventions must be reconciled** when the port is validated
> against this case — flagged for the human doing Stage 3.

## 2.2.2.3 Cross sections from Monte Carlo (MGXS)

Both diffusion and $SP_N$ need homogenised multigroup constants. Monte Carlo
(Serpent → OUTRAM PARK: `outram-mc-libs` + `njoy-outram-park-fork`) produces them
by flux-weighted tallying over spatial cells and energy bins (Eq. 2.23):

$$\Sigma_{x,g} = \frac{\int_{cell}dV\int_{E_g}^{E_{g-1}}\Sigma_x(E)\Phi(E)\,dE}{\int_{cell}dV\int_{E_g}^{E_{g-1}}\Phi(E)\,dE}$$

For **transient** analysis these constants are parametrised as functions of the
operating conditions that matter — **fuel temperature** (Doppler) and **coolant
(flibe) density**. A macroscopic XS factors as $\Sigma_x = \sigma_x\rho_n$
(Eq. 2.24). Because flibe density changes strongly and roughly linearly with $T$,
its XS is fitted **linearly in density** (Eq. 2.25):

$$\hat{\Sigma}(\rho_{flibe}) = c_0 + c_1(\rho_{flibe}-\rho_0)$$

The solid fuel does not change density; its feedback is Doppler broadening, fitted
**linear in $\log T$** (Eq. 2.26):

$$\hat{\Sigma}(T_{fuel}) = c_0 + c_1(\log(T_{fuel})-\log(T_0))$$

Coefficients $c_0, c_1$ come from "computer experiments": perturb one condition
$\pm 10\%$ in a Monte Carlo model and fit the slope (Latin-hypercube / MC sampling
recommended for multi-variable coverage).

## 2.2.2.4 Porous-media thermal-hydraulics

The pebble bed is a porous medium of porosity $\epsilon = V_f/(V_f+V_s)$
(Eq. 2.27). Pressure loss uses the **Ergun** correlation (Eq. 2.31):

$$\frac{dp}{dx} = E_1\frac{(1-\epsilon)^2\mu u}{\epsilon^2 d^2} + E_2\frac{1-\epsilon}{\epsilon^3}\frac{\rho u^2}{d}$$

mapped to COMSOL's Darcy–Forchheimer form via permeability $K$ and drag $\beta_F$
(Eqs. 2.32–2.35). Two-temperature (fluid/solid) energy transport
(Eqs. 2.36–2.37):

$$\epsilon(\rho c_p)_f\frac{\partial T_f}{\partial t} + (\rho c_p)_f U\nabla T_f = \epsilon k_f\nabla\nabla T_f + \Phi + h_{sf}a(T_s-T_f)$$

$$(1-\epsilon)(\rho c_p)_s\frac{\partial T_s}{\partial t} = (1-\epsilon)k_s\nabla\nabla T_s + (1-\epsilon)q + h_{sf}a(T_f-T_s)$$

The interphase heat-transfer coefficient uses the **Wakao** packed-bed correlation
(Eqs. 2.9–2.13):

$$Nu = 2 + 1.1\,Pr^{1/3}Re^{0.6} \qquad Re = \frac{\rho d_p u}{\mu} \qquad Pr = \frac{c_p\mu}{k} \qquad h = \frac{Nu\,k}{d_p}$$

Inlet coolant temperature is imposed as a boundary condition (constant, or a
function of time to drive overcooling transients); a pressure BC is set at the
outlet; reflector walls are adiabatic. The Ergun/Wakao closures and Mk1 values
carry into [`04-transients-fig4-29.md`](04-transients-fig4-29.md).

## 2.2.2.5 Multi-scale fuel temperature

Fuel temperature is the dominant FHR feedback, so its **internal profile** inside
pebbles and TRISO kernels is resolved with 1-D spherical heat conduction (Eq. 2.39
= Eq. 2.6):

$$\rho C_p\frac{\partial T}{\partial t} = \frac{1}{r^2}\frac{\partial}{\partial r}\left(kr^2\frac{\partial T}{\partial r}\right) + g$$

with symmetric centre BC $\partial_r T|_{r=0}=0$ and a Robin surface BC to the
coolant $\partial_r T|_{r=R} = (h/k)(T-T_0)$. Each pebble is split into 3 fuel
sub-layers and each TRISO kernel into 3 sub-layers; the layer temperatures feed a
**multi-variable** linear-log XS fit (Eq. 3.1):

$$\hat{\Sigma}(T_{fuel}) = c_0 + \sum_{k} c_k\,\log(T_k)$$

where the $T_k$ run over the sub-layer temperatures $T_{nm}$ ($m$-th TRISO-kernel
layer inside the $n$-th pebble layer). During fast reactivity transients the heat
has no time to reach the coolant, so this resolved fuel profile is what stabilises
the core — which is exactly why Fig. 4.29 (max fuel temperature) is the quantity
of interest.

## Appendix D — COMSOL user-PDE form of $SP_3$

COMSOL has no neutronics module; Wang implements diffusion and $SP_N$ through the
"General Form PDE" interface. The general coefficient form (Eq. D.1, eigenvalue):

$$\lambda^2 e_a u - \lambda d_a u + \nabla\cdot(-c\nabla u - \alpha u + \gamma) + \beta\nabla u + a u = f$$

with $u$ the unknown-field vector, $c$ diffusion, $a$ absorption, $f$ source,
$d_a$ damping/mass, and eigenvalue $\lambda = 1/k_{eff}$. **Caveat (Eqs. D.3–D.5):**
in 2-D axisymmetric cylindrical coordinates COMSOL's $\nabla\cdot(c\nabla U)$ is
implemented as $\partial_r(c\,\partial_r U)+\dots$, i.e. **without** the $1/r$
metric factor of the true cylindrical divergence — the user must fold an explicit
$r$ factor into the coefficients (this is why $r$ appears throughout the $c$/$a$
matrices below).

The unknown vector stacks the 8 group fluxes and 6 precursors; for $SP_3$ it
stacks **two** moment fields per group plus precursors:
$u = [\text{Flux}1..8,\ \text{Flux}21..28,\ \text{Conc}1..6]^{T}$
(the `Flux2x` block is $\phi_{2g}$). The condensed $SP_3$ statement (Eq. D.12) is:

$$-\nabla D_1\nabla(\phi_0+2\phi_2) + \Sigma_r\phi_0 = S_0$$

$$-\nabla D_2\nabla\phi_2 + \Sigma_t\phi_2 = \frac{2}{5}(\Sigma_r\phi_0 - S_0)$$

where $S_0$ is the combined fission ($\chi_g\nu\Sigma_f/k_{eff}$, with the prompt
$(1-\beta)$ split) + in-scatter source. The COMSOL coefficient matrices (Eqs.
D.8–D.17) are: block-diagonal $c = \mathrm{diag}(D_1,\dots)\!\cdot r$ with a
$\mathrm{diag}(2D_1,\dots)$ off-block coupling $\phi_2$ into the $\phi_0$ leakage;
a block $a$ matrix whose upper-left block $B$ holds
$(\Sigma_{rg}-(1-\beta)\chi_{pg}\nu\Sigma_{fg'}\lambda)r$ on the diagonal and
$-\Sigma_{s,g'g}r$ (minus fission) off-diagonal, a $-\tfrac{2}{5}B$ / $\mathrm{diag}(\Sigma_{tg}r)$
coupling to the $\phi_2$ rows, and $\mathrm{diag}(\lambda_{di})$ for precursors;
$f = 0$; and a mode-switch mass matrix $da = \mathrm{diag}(1/v_g\,\text{eig},\dots,\text{eig})$
where the Boolean `eig` selects eigenvalue vs transient/steady mode. (These are
matrices; per the workspace no-matrix-math rule they are described here rather
than typeset — see Appendix D pp. 117–118 of the source for the exact arrays.)
