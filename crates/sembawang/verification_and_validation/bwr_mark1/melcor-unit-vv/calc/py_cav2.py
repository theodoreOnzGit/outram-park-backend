import math
g=9.8; rl=960.0; rv=0.59; nu=1.29e-5; N=0.1; hlv=2.257e6
# test inputs (assumed for test): kc=1.5, Cp=1000, Ts=2000(melt/solidus), Tsat=373.15, alphaT=1e-5, desat = Cp*(Ts-Tsat), decr=Cp*(Tcr-Tsat)
sig=6.77e7; E=1.25e11; aT=1e-5; Ts=2000.0; Tsat=373.15; Cp=1000.0; kc=1.5
Tcr=Ts-sig/(aT*E); print("Tcr",repr(Tcr), "drop", sig/(aT*E))
desat=Cp*(Ts-Tsat); decr=Cp*(Tcr-Tsat)
A=hlv*(rl-rv)*g/nu; B=N*kc**2*desat/(Cp*decr); C=aT*(Tcr-Tsat)
q=(A**5*B**4)**(1/13)*C**(15/13)
print("A",A,"B",B,"C",C,"q_dry/Cdry",repr(q))
# conduction thickness 3-10 with TB=Ts
d=kc*(Ts-Tsat)/q; print("delta/ (1/Cdry)",repr(d))
# permeability 3-15 with mu_v=nu-value numerically (1.29e-5)
kap=2*1.29e-5*q/(rv*hlv*(rl-rv)*g); print("kappa per Cdry",repr(kap))
# jmin  rho_c=8000 crust, rho_m 7000? crust floating needs rho_c<rho_m; use rho_c 7500 rho_m 8000 -> negative
mug=3e-5
print("jmin (rc-rm)=-500:", repr(kap*(7500-8000)*g/mug))
# eruption
for E_ in [0.06,0.08]:
    K=E_*math.sqrt(0.3/7000); print("Kent",E_,repr(K),"jmelt for jgas 0.05", repr(K*0.05))
# viscosity
for T in [1800.0,2000.0]:
    print("steel mu0",T,repr(1.076e-3*math.exp(3313/T)),"basalt",repr(1.94e-5*math.exp(20950/T)))
def phi(Tl,Ts,Tz,Tr): return max(0,min(1,(Tl-(Tz+Tr)/2)/(Tl-Ts)))
print("phi",phi(2800,2200,2500,2400), phi(2800,2200,2900,2900), phi(2800,2200,2000,2000))
for p in [0,0.1,0.3,0.5,0.9]:
    print("phi",p,"kunitz",repr((1+0.5*p)/(1-p)**4),"ramC=4",repr(math.exp(2.5*4*p)),"C=6",repr(math.exp(2.5*6*p)))
# spreading
C1=0.136; rho=8000; mu=0.01; V=10.0; R0=1.0; t=100.0
R_printed=(R0**8+C1*rho*g*V**3*t/(mu*math.pi**3))**0.125
R_integrated=(R0**8+8*C1*rho*g*V**3*t/(mu*math.pi**3))**0.125
R_huppert=(R0**8+C1*rho*g*V**3*t/mu)**0.125
print("R printed 3-25",repr(R_printed),"R from integrating 3-24",repr(R_integrated),"R Huppert form",repr(R_huppert))
print("0.894^8/3",0.894**8/3)
# Euler of 3-24 (MELCOR's stated integrator)
R=R0;dt=1e-3
for i in range(int(t/dt)): R+=C1*rho*g*V**3/(mu*math.pi**3*R**7)*dt
print("euler 3-24",R)
# layer heights
m=5e4; rl_=8000; Vl=m/rl_; H=Vl/(math.pi*R_printed**2); print("Vlay",Vl,"H",repr(H))
