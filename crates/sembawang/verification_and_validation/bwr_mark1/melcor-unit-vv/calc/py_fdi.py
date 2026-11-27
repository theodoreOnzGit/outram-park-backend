import math
sig=5.670374e-8; g=9.81
# FDI-01 stream geometry
ztop,zbot,U,dt,mf,D0,rf=10.0,2.0,4.0,1.5,500.0,0.1,8000.0
zhead=ztop-U*dt; ztail=zhead+mf/(math.pi*D0**2*rf/4); zlen=ztail-zhead
mflr=mf*max(0,min(1,(zbot-zhead)/zlen))
print("zhead",zhead,"ztail",repr(ztail),"zlen",repr(zlen),"mflr",repr(mflr))
zhead2=ztop-U*1.0; print("dt=1 zhead",zhead2,"> zbot -> mflr=min(mf,mCAV0)", min(mf,120.0), "mCAV=",mf+120-min(mf,120.0))
# FDI-02 Chu
rc,U,D0,sf=958.0,1.8793,0.1,0.45
r=math.sqrt(rc/rf); We=rc*U*U*D0/sf
C1m=0.1232-0.149*r; C1o=0.171-0.149*r
Zpm=C1m*We**0.254/D0*r
print("r",repr(r),"We",repr(We),"We^.254",repr(We**0.254),"C1mod",repr(C1m),"C1orig",repr(C1o),"Z per m",repr(Zpm))
for dz in [0.5,1.0,2.0]: print("D at",dz,repr(D0*math.exp(-Zpm*dz)))
# time form equivalence: tau=(U t/D0) r, D=D0 exp(-C1 tau We^.254); t=dz/U
t=1.0/U; tau=U*t/D0*r; print("time form D(1m)",repr(D0*math.exp(-C1m*tau*We**0.254)), "orig Chu", repr(D0*math.exp(-C1o*tau**0.772*We**0.246)))
print("C1mod zero at rc/rf=",(0.1232/0.149)**2)
# FDI-03 Bromley with fixed props
def hfb(kg,mug,D,Tg,Ts,rg,rc,ifg): return 0.5*kg*(kg*mug*D*(Tg-Ts)/(rg*(rc-rg)*g*ifg))**-0.25
kg,mug,rg,ifg,Ts,Tc=0.08,3.5e-5,0.25,2.257e6,373.15,373.15
D=0.01
for Tf in [1000.0,1500.0,2000.0]:
    Tg=0.5*(Tf+Ts); h=hfb(kg,mug,D,Tg,Ts,rg,rc,ifg); print("Tf",Tf,"hFB",repr(h),"qfb",repr(h*(Tf-Ts)),"qrad",repr(sig*(Tf**4-Tc**4)))
# solve TTRAN with props fixed
f=lambda T: sig*(T**4-Tc**4)-hfb(kg,mug,D,0.5*(T+Ts),Ts,rg,rc,ifg)*(T-Ts)
lo,hi=400.0,3000.0
print("f(lo)",f(lo),"f(hi)",f(hi))
for i in range(200):
    m=0.5*(lo+hi)
    if f(m)<0: lo=m
    else: hi=m
print("TTRAN",repr(0.5*(lo+hi)),"h",hfb(kg,mug,D,0.5*(lo+hi+Ts),Ts,rg,rc,ifg) )
# FDI-04 radiation-only cooling closed form (Tc neglected) const D
rho_f,c,Dc,U=8000.0,500.0,0.005,2.0
T0=2800.0; z=1.0
T=(T0**-3+18*sig*z/(rho_f*c*Dc*U))**(-1/3); print("analytic T(1m)",repr(T))
# RK4 check
def rate(T): return -6*sig*T**4/(rho_f*c*Dc*U)
Tn=T0;h=1e-3
for i in range(1000):
    k1=rate(Tn);k2=rate(Tn+0.5*h*k1);k3=rate(Tn+0.5*h*k2);k4=rate(Tn+h*k3);Tn+=h/6*(k1+2*k2+2*k3+k4)
print("rk4",repr(Tn))
# FDI-05 steam
mdeb=1000.0;cp=400.0;T0,Tb=2800.0,2600.0;Q=mdeb*cp*(T0-Tb); hscv=2.6755e6; hpool=4.19e5
m=Q/(hscv-hpool); print("Q",Q,"msteam",repr(m))
pool=30.0; Tb2=T0-pool*(hscv-hpool)/(mdeb*cp); print("capped Tb",repr(Tb2))
# FDI-06 settling
m0,S,tau,dt=5.0,2.0,10.0,3.0
m=(m0-S*tau)*math.exp(-dt/tau)+S*tau; print("settle",repr(m))
tst,tox=10.0,5.0; tso=1/(1/tst+1/tox); ml0,Sl=4.0,1.0; mk0,Sk=0.5,0.2; R=123.222/91.224
mlt=(ml0-Sl*tso)*math.exp(-dt/tso)+Sl*tso
# product exact: dmk/dt=-mk/tst + R ml/tox + Sk
a=1/tst; b=1/tso
A=(ml0-Sl*tso); 
# particular: steady mk_inf = tst*(Sk + R*Sl*tso/tox); transient from ml: coefficient K*exp(-t/tso) with K*(-b) = -a K + R A/tox -> K=R*A/tox/(a-b)
minf=tst*(Sk+R*Sl*tso/tox); K=R*A/tox/(a-b)
mkt=minf+K*math.exp(-dt/tso)+(mk0-minf-K)*math.exp(-dt/tst)
print("tso",repr(tso),"ml",repr(mlt),"mk",repr(mkt),"C1=minf",repr(minf),"K",repr(K))
# numeric check
mk,ml=mk0,ml0;h=1e-5
for i in range(int(dt/h)):
    dml=-ml/tso+Sl; dmk=-mk/tst+R*ml/tox+Sk; ml+=dml*h; mk+=dmk*h
print("num",ml,mk)
# FDI-07 oxid ratio
for W,xo,xs in [(1,0.1,0.3),(1,0,0.3),(1e6,0.1,0.3),(2,0.1,0.3)]: print("Rox",W,repr(W*xo/(W*xo+xs)))
