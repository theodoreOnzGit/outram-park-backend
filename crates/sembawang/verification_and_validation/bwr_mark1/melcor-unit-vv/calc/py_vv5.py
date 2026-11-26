import math
g=9.80665
# HS-01 natural/forced/mixed atmosphere, external rectangular (C4107,C4110,C4119,C4122)
def nu_nat(Ra):
    lam=0.59*Ra**0.25; tur=0.10*Ra**(1/3)
    if Ra<1e9: return lam
    if Ra>1e10: return tur
    return lam+(Ra-1e9)/(1e10-1e9)*(tur-lam)
def nu_forced(Re,Pr):
    lam=0.664*Re**0.5*Pr**(1/3); tur=0.037*Re**0.8*Pr**(1/3)
    if Re<3e5: return lam
    if Re>6e5: return tur
    return lam+(Re-3e5)/3e5*(tur-lam)
def nu_mixed(Re,Gr,Pr):
    x=Re*Re/Gr
    if x<1.0: return nu_nat(Gr*Pr)
    if x>10.0: return nu_forced(Re,Pr)
    return (x-1)/9*(nu_forced(Re,Pr)-nu_nat(Gr*Pr))+nu_nat(Gr*Pr)
Pr=0.7
for Ra in [1e8,1e9,5e9,1e10,1e11]: print("HS01 nat Ra",Ra,repr(nu_nat(Ra)))
print("HS01 lam/turb at 1e9:",repr(0.59*1e9**0.25),repr(0.10*1e9**(1/3)),"at 1e10:",repr(0.59*1e10**0.25),repr(0.10*1e10**(1/3)))
for Re in [1e5,3e5,4.5e5,6e5,1e6]: print("HS02 forced Re",Re,repr(nu_forced(Re,Pr)))
Gr=1e10
for Re in [5e4,1e5,2e5,3e5,4e5]: print("HS01 mixed Re",Re,"Re2/Gr",Re*Re/Gr,repr(nu_mixed(Re,Gr,Pr)))
# HS-03 Sherwood
Nu=150.0;Sc=0.55;Pr=0.72;D=3.5e-5;L=2.0
Sh=Nu*Sc**(1/3)*Pr**(-1/3); hD=Sh*D/L; print("HS03 Sh",repr(Sh),"hD",repr(hD))
# HS-04 condensation Eq 2-136
rho_v=1.1;Ptot=3e5;Pstm=1.5e5;Psrf=5.0e4
mc=hD*rho_v*math.log((Ptot-Psrf)/(Ptot-Pstm)); print("HS04 mc",repr(mc))
# UG form evaporation limit: me = he*min(0,Tdew-Tsrf)/hfg; m=max(mc,me)
he=5e5;hfg=2.2e6
for Tdew,Tsrf in [(384.5,360.0),(384.5,385.5)]:
    me=he*min(0.0,Tdew-Tsrf)/hfg; print("HS04 UG me",Tdew,Tsrf,repr(me))
# Evaporation example: Psrf near Ptot
Psrf2=2.99e5; mc2=hD*rho_v*math.log((Ptot-Psrf2)/(Ptot-Pstm)); me2=he*min(0,384.5-406.0)/hfg
print("HS04 evap mc",repr(mc2),"me",repr(me2),"UG m=max",repr(max(mc2,me2)),"RM m=min(mc, he*max(0,..))",repr(min(mc2,he*max(0,384.5-406.0)/hfg)))
# HS-06 surface tension Eq 2-115
def sigma(T): TR=1-T/647.3; return 0.2358*(1-0.625*TR)*TR**1.256
for T in [300.0,373.15,500.0,647.3]: print("HS06 sigma",T,repr(sigma(T)))
# Saturated water 1 atm
rl=958.35;rv=0.5978;hfg=2.2565e6;cpl=4216.0;mul=2.817e-4;Prl=1.75
Tsat=373.15
def q_nb(dT,Csf=0.013,n=0.33,m=1.0):
    s=sigma(Tsat+dT/2)
    return mul*hfg*math.sqrt(g*(rl-rv)/s)*(cpl*dT/(Csf*hfg*Prl**m))**(1/n)
for dT in [5.0,10.0,20.0]: print("HS06 qnb dT",dT,repr(q_nb(dT)))
s=sigma(Tsat+10)
qchf=0.18*rv*hfg*(s*(rl-rv)*g/rv**2)**0.25*(rl/(rl+rv))**0.5
print("HS07 qchf(sigma@Tavg dT=10)",repr(qchf))
s0=sigma(Tsat)
print("HS07 qchf(sigma@Tsat)",repr(0.18*rv*hfg*(s0*(rl-rv)*g/rv**2)**0.25*(rl/(rl+rv))**0.5), "Zuber0.131",repr(0.131*rv*hfg*(s0*(rl-rv)*g/rv**2)**0.25))
qmin_printed=0.09*rv*hfg*(s0*(rl-rv)*g/rl**2)**0.25*(rl/(rl+rv))**0.5
qmin_classic=0.09*rv*hfg*(s0*(rl-rv)*g/(rl+rv)**2)**0.25
print("HS07 qmin as printed",repr(qmin_printed),"classic Zuber (rl+rv)^2",repr(qmin_classic))
kv=0.0251;muv=1.227e-5;cpv=2080.0;Lc=0.01
def qfilm(dT): return 0.943*(rv*(rl-rv)*g*kv**3*(hfg+0.5*cpv*dT)/(muv*Lc))**0.25*dT**0.75
for dT in [200.0,500.0]: print("HS07 Bromley dT",dT,repr(qfilm(dT)))
sig=5.669e-8
print("HS07 qrad Tsurf 873.15 Tpool 373.15",repr(sig*(873.15**4-373.15**4)))
# HS-05 laminar film Nusselt (C4211 0.943, 0.25), vertical L=1, sin=1
rf=958.35; kf=0.679; muf=2.817e-4; cpf=4216.0; Tf=373.15; Tsrf=353.15; L=1.0
hfgp=hfg+0.68*cpf*(Tf-Tsrf)
Nu=0.943*(g*rf*(rf-rv)*hfgp*L**3*1.0/(muf*kf*(Tf-Tsrf)))**0.25
print("HS05 hfg'",repr(hfgp),"Nu",repr(Nu),"h",repr(Nu*kf/L))
print("HS05 k/delta 1e-4",repr(kf/1e-4))
