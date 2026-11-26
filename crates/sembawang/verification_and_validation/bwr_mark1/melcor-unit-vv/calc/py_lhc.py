import math
# crust profile
Tsol,Tb,Q,rho,cp,k,u,delta=2800.0,400.0,1.0e6,8000.0,500.0,3.0,1e-5,0.02
a=k/(rho*cp); x=u*delta/a
def T(z): return Tsol-(Tsol-Tb+Q*delta/(rho*cp*u))*(math.exp(u*z/a)-1)/(math.exp(x)-1)+Q*z/(rho*cp*u)
print("alpha",a,"Pe",x,"T0",T(0),"Tdelta",repr(T(delta)),"Tmid",repr(T(delta/2)))
Tbar=Tsol-(Tsol-Tb+Q*delta/(rho*cp*u))/(math.exp(x)-1)*((a/(u*delta))*(math.exp(x)-1)-1)+Q*delta/(2*rho*cp*u)
# numeric average
N=200000; s=sum(T((i+0.5)*delta/N) for i in range(N))/N
print("Tbar closed",repr(Tbar),"numeric",repr(s))
# check ODE residual: T'' - (u/a)T' = -Q/k
h=1e-6; z=0.01
d2=(T(z+h)-2*T(z)+T(z-h))/h**2; d1=(T(z+h)-T(z-h))/(2*h); print("resid",d2-(u/a)*d1, -Q/k)
# limit u->0: conduction with source: T = Tsol-(Tsol-Tb)z/d + Q z(d-z)/(2k)
u2=1e-12
def T2(z,u):
    x=u*delta/a; return Tsol-(Tsol-Tb+Q*delta/(rho*cp*u))*(math.expm1(u*z/a))/math.expm1(x)+Q*z/(rho*cp*u)
print("u->0 mid",T2(delta/2,1e-9),"exact",Tsol-(Tsol-Tb)*0.5+Q*(delta/2)**2/(2*k))
# velocity 2-16
qb,qs,hfs=2.0e5,1.0e5,2.7e5
uu=(qb-qs-Q*delta)/(rho*(hfs-cp*(Tb-Tsol)))
print("u from 2-16",repr(uu))
# cp_eff
print("cp_eff UO2-like", 500+2.74e5/0.1)
# density 2-5 one comp
M=0.27; chi=1.0; Vm=24.6e-6*1000  # cm3/mol?  
# boundary temps 2-11/2-12
kpl,dzn,km,HL,TL,Tpl,hb,Tbnd=40.0,0.01,3.0,0.2,2800.0,600.0,500.0,400.0
TB=(Tpl*(2*kpl/dzn)+TL*(2*km/HL))/(2*km/HL+2*kpl/dzn)
TT=(Tbnd*hb+TL*(2*km/HL))/(2*km/HL+hb)
print("TB",repr(TB),"TT",repr(TT))
