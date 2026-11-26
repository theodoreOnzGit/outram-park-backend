import math as m
g=9.81
print("== 01 SS yield 5-16")
def sy(T): f=lambda t:1/(1+(t/800.)**3); return max(0.0,260e6*(f(T)-f(1700.)))
for T in (300.,800.,1000.,1273.15,1600.,1700.,1800.): print(T, "%.9g"%sy(T))
print("== 02 PLATEB/PLATEG 5-3")
def sig_plate(K,x,h,A,W): return 6*K*x*x/(A*h*h)*W
A=m.pi*1.72**2  # arbitrary
print("A=%.9g"%(m.pi*1.72**2))
W=1.0e6; print("PLATEB K=.0833 x=.3 h=.05 A=9.29 W=1e6 ->","%.9g"%sig_plate(0.0833,0.3,0.05,9.29,W))
print("PLATEG K=.0513 same ->","%.9g"%sig_plate(0.0513,0.3,0.05,9.29,W))
# temperature where yield = stress (bisection)
s=sig_plate(0.0833,0.3,0.05,9.29,W)
a,b=1000.,1700.
for _ in range(200):
    c=0.5*(a+b)
    if sy(c)>s: a=c
    else: b=c
print("T_fail(PLATEB 1e6 N)=%.9g"%a)
print("== 03 Ksupport 6-32, ligament 6-33")
def ks(nu,xc): return (1+nu)*(m.log(xc)-0.811)/(4*m.pi)
print("%.9g"%ks(0.3,5.0), "%.9g"%ks(0.3,10.0))
print("eps lig b=.01 d=.1 ->", 1-0.01/0.1, " K/eps=%.9g"%(ks(0.3,5)/0.9))
print("== 04 PLATE 5-6,5-11,5-12")
nu=0.3; K0=(3+nu)/16; K1=(1+3*nu)/(3+nu); print("K0=%.9g K1=%.9g"%(K0,K1))
h=0.05; Wt=1e6
print("ring1 sigma (K0=.206)=%.9g"%(6*0.206/(m.pi*h*h)*Wt))
def s_i(r_im1,a,r0,Wt,h,K0=0.206,K1=0.576): return 6*K0/(m.pi*h*h)*(1-K1*(r_im1/a)**2)*(1+(r0/r_im1)**2)*Wt
print("ring i r_{i-1}=1 a=2 r0=0 ->%.9g"%s_i(1,2,0,Wt,h), " r0=1 ->%.9g"%s_i(1,2,1,Wt,h))
print("== 05 SS creep 5-21/5-22")
def tR_ss(T,s): P=81000-7500*m.log10(s); return 10**(P/T-16.44)
for T,s in ((1000.,10e6),(1200.,1.94e6),(1100.,63.5e6)): print(T,s,"%.9g"%tR_ss(T,s))
print("== 06 E(T) 5-20, buckling")
def E(T): f=lambda t:1/(1+(t/1650.)**3); return max(0.0,370e9*(f(T)-f(1700.)))
for T in (300.,1000.,1700.): print(T,"%.9g"%E(T))
ro,ri,N,l=0.14,0.13,10,4.0
Ac=N*m.pi*(ro**2-ri**2); I=m.pi/4*(ro**4-ri**4)
print("Ac=%.9g I(single)=%.9g"%(Ac,I))
Wb=m.pi**2*E(1000.)*I/l**2; print("Wbuckle single col 1000K=%.9g"%Wb)
sb=m.pi**2*(ro**2+ri**2)/(4*N*l**2)*E(1000.); print("sigma_buckle (5-19, N=10)=%.9g"%sb)
print("check N*Wb/Ac=%.9g"%(N*Wb/Ac), " vs 5-19 w/o N: %.9g"%(m.pi**2*(ro**2+ri**2)/(4*l**2)*E(1000.)))
W=2e6; print("column stress W=2e6 =%.9g ; Mratio 0.8 ->%.9g"%(W/Ac, W/Ac/0.8))
print("== 07 LH LM 6-23..6-26")
def tR_lh(T,s): P=4.812e4-4.725e3*m.log10(s); return 10**(P/T-7.042)
print("tR(1273,10MPa)=%.9g"%tR_lh(1273.,10e6))
print("tR(1273.15,10MPa)=%.9g"%tR_lh(1273.15,10e6))
print("tR(1000,50MPa)=%.9g"%tR_lh(1000.,50e6))
Ri,Ro=2.4,2.62; dP=7e6; rho_d=4000.; dz=1.0
se=(dP+rho_d*g*dz)*Ri**2/(Ro**2-Ri**2); print("sigma_e=%.9g"%se, "tR(1100)=%.9g"%tR_lh(1100.,se))
print("strain per 600 s at tR 6e4: %.9g"%(0.18*600/6e4))
# 5-node mean vs mass-average
Ts=[1400,1300,1200,1100,1000]; w=[0.5,1,1,1,0.5]
ar=sum(Ts)/5; ma=sum(a*b for a,b in zip(Ts,w))/sum(w)
print("arith mean %.9g mass-avg(half end nodes) %.9g ; tR ratio %.6g"%(ar,ma,tR_lh(ar,se)/tR_lh(ma,se)))
Ts=[1500,1200,1000,900,800]
ar=sum(Ts)/5; ma=sum(a*b for a,b in zip(Ts,w))/sum(w)
print("nonlinear profile: arith %.9g mass %.9g ratio tR %.6g"%(ar,ma,tR_lh(ar,10e6)/tR_lh(ma,10e6)))
print("== 08 6-27, 6-28, 6-31")
def E_lh(T): f=lambda t:1/(1+(t/900.)**6); return max(0.0,2.0e11*(f(T)-f(1800.)))
def SY_lh(T): f=lambda t:1/(1+(t/900.)**6); return max(0.0,4.0e8*(f(T)-f(1800.)))
for T in (300.,900.,1273.15,1800.): print(T,"%.9g %.9g"%(E_lh(T),SY_lh(T)))
print("eps_th(1273.15-560)=%.9g"%(1e-5*(1273.15-560)))
print("== 09 debris->LH, pen")
hdh=1000; Ah=10.; Td=2000.; Ths=800.; print("q_dh=%.9g"%(hdh*Ah*(Td-Ths)))
print("q_dp=%.9g"%(1000*(0.2/0.5)*0.5*(2000-900)))
kp=20.; Aph=0.01; dz1=0.5; print("q_ph=%.9g"%(kp*Aph*(900-800)/dz1))
print("== 10 downward boiling 6-9,6-10, 6-13,6-14, 6-17")
# saturated water at 0.101325 MPa (approx): rho_l 958.35, rho_v 0.5976, hlv 2.2565e6, sigma 0.0589, kv 0.0251, muv 1.227e-5
rl,rv,hlv,sg,kv,muv=958.35,0.5976,2.2565e6,0.0589,0.0251,1.227e-5
def qchf(th): return (0.034+0.0037*th**0.656)*rv*hlv*(g*sg*(rl-rv)/rv**2)**0.25
def qmin(th): return (4.8e-4+8.2e-4*th**0.407)*rv*hlv*(g*sg*(rl-rv)/rv**2)**0.25
base=rv*hlv*(g*sg*(rl-rv)/rv**2)**0.25
print("Zuber-type base rho_v hlv [g sig drho/rho_v^2]^1/4 = %.9g"%base)
for th in (0.,30.,90.): print(th,"qCHF=%.9g qMIN=%.9g"%(qchf(th),qmin(th)))
# alternative form with rho_v^0.5 outside [g sigma drho]^0.25 
alt=rv**0.5*hlv*(g*sg*(rl-rv))**0.25
print("alt base rho_v^1/2 hlv [g sig drho]^1/4 = %.9g (identical)"%alt)
def hflm(th,dT): return 0.142*kv*(hlv*rv*g*(rl-rv)/(muv*kv*dT))**(1/3)*m.sin(m.radians(th))**0.3333333
def hflm2(th,dT): return (0.055+0.016*th**0.5)*kv*(hlv*rv*g*(rl-rv)/(muv*kv*dT))**(1/3)
print("hFLM(90deg,500K)=%.9g alt=%.9g ; hFLM(0deg)=%.3g alt(0)=%.9g"%(hflm(90,500),hflm2(90,500),hflm(0,500),hflm2(0,500)))
def htr(dT,qc,qm,dTc,dTm): return qm/dT*(dT/dTm)**(m.log(qc/qm)/m.log(dTc/dTm))
qc,qm,dTc,dTm=1.0e6,2.0e4,30.,300.
for dT in (30.,100.,300.): print("htr dT=%g -> h=%.9g q=%.9g"%(dT,htr(dT,qc,qm,dTc,dTm),htr(dT,qc,qm,dTc,dTm)*dT))
print("h_atm*A*dT outer: %.9g"%(10*1*(600-330)))
# angle averaging 6-15 of qCHF over 0..30 deg
import functools
def avg(f,a,b,n=20000):
    s=0;dt=(b-a)/n
    for i in range(n):
        t=a+(i+0.5)*dt; s+=f(t)*m.sin(m.radians(t))*m.radians(dt)
    return s/(m.cos(m.radians(a))-m.cos(m.radians(b)))
print("avg qCHF 0-30deg=%.9g ; avg hFLM(500K) 0-30deg=%.9g"%(avg(qchf,0,30),avg(lambda t:hflm(t,500),0,30)))
print("== 11 LH melting fraction")
Eml=1.0e6; ELHF=2.7e5
fl=max(0,(1.135e6-Eml)/ELHF); fr=max(0,(1.27e6-Eml)/ELHF); print("fl=%.9g fr=%.9g f=%.9g dV=%.9g"%(fl,fr,min(1,0.5*(min(1,fl)+min(1,fr))),0.5*(fl+min(fr,1))*1000/7900))
print("== 12 ejection 6-34,6-35")
def vej(Cd,dP,rho,dz): x=2*dP/rho+2*g*dz; return Cd*m.sqrt(x) if x>0 else 0.0
Af=m.pi*0.1**2/4; print("Af(0.1 m)=%.9g"%Af)
v=vej(1.0,1.0e6,8000.,1.0); print("vej=%.9g Mej(dt=1)=%.9g"%(v,8000*Af*v*1.0))
print("vej dP=0 dz=1: %.9g ; dP=-1e5,dz=0.5: %.9g"%(vej(1,0,8000,1),vej(1,-1e5,8000,0.5)))
print("fraction of 50 t: %.9g"%min(1,8000*Af*v/5e4))
print("== 13 ablation 6-36..6-38")
k,rho,mu,Pr=3.0,8000.,4.4e-3,0.7333
Kp=k*(rho/mu)**0.8*Pr**(1/3); print("Kp=%.9g"%Kp)
Df=0.1; dzh=0.22
ht=0.023*Kp*v**0.8/Df**0.2; hp=0.0292*Kp*v**0.8/dzh**0.2; print("htube=%.9g hplate=%.9g"%(ht,hp))
habl=max(ht,hp); rs,cps,hfs,Tms,Thavg,Td=7850.,600.,2.7e5,1700.,800.,2800.
dD=2*habl*(Td-Tms)/(rs*(cps*(Tms-Thavg)+hfs)); print("dDf/dt=%.9g m/s"%dD)
print("== 15 falling debris")
print("z_d at 2 s, zcsp 4, v=1 ->",4-1*2, " v=5 ->",4-5*2)
f=1.0
for dt in (1.0,): pass
print("f after 1000 s no inflow: %.9g; t to 0.01: %.9g s"%(m.exp(-1000/360),360*m.log(100)))
print("f after one step 10s, f0=0.5 Vcor/VLP=0.2: %.9g"%min(1,0.5*m.exp(-10/360)+0.2))
mf,mc=1.0e5,3.0e4; V=mf/10412+mc/6550; Ap=6*V/0.01; print("V=%.9g Ap=%.9g Q(h=100,dT=1500)=%.9g"%(V,Ap,100*Ap*1500))
print("== 16 Lipinski")
def lip(rl,rv,hlv,d,eps,L,lam0=0.089,dref=5e-4,C=0.756):
    eps=max(eps,0.15); lc=lam0*dref/d
    return C*hlv*m.sqrt(rv*(rl-rv)*g*d*eps**3*(1+lc/L)/((1-eps)*(1+(rv/rl)**0.25)**4))
for args in ((915.29,2.6680,2.1074e6,0.01,0.4,1.0),(915.,2.67,2.108e6,0.01,0.4,1.0),(915.29,2.6680,2.1074e6,0.0005,0.4,0.1),(915.29,2.6680,2.1074e6,0.01,0.1,1.0),(915.29,2.6680,2.1074e6,0.01,0.15,1.0)):
    print(args,"%.9g"%lip(*args))
# full 2-115 capillary head for comparison
def lamc(sig,th,eps,d,rl,rv): return 6*sig*m.cos(th)*(1-eps)/(eps*d*(rl-rv)*g)
print("lambda_c 2-115 sigma=.0589 theta=0 eps=.4 d=5e-4 @1atm: %.9g"%lamc(0.0589,0,0.4,5e-4,958.35,0.5976))
print("lambda_c 2-115 at eps=.4 d=.01: %.9g vs scaled 0.089*5e-4/0.01=%.9g"%(lamc(0.0589,0,0.4,0.01,958.35,0.5976),0.089*5e-4/0.01))
print("== 17 ACOPO")
k,mu,beta,cp,rho=3.0,4.4e-3,1.05e-4,500.,8000.
nu=mu/rho; Pr=mu*cp/k
def hpool(Q,H,A=0.3,n=0.22):
    Ra=g*beta*Q*H**5/(k*nu*nu)*Pr; return Ra, A*Ra**n*k/H
for Q,H in ((1e6,0.5),(1e6,1.0),(2e6,0.5)): Ra,h=hpool(Q,H); print(Q,H,"Ra=%.9g h=%.9g Nu=%.9g"%(Ra,h,h*H/k))
Ra,_=hpool(1e6,0.5); print("Bonnet .381 Ra^.234 Nu=%.9g h=%.9g"%(0.381*Ra**0.234,0.381*Ra**0.234*k/0.5))
h1,h2=1609.,500.; h12=h1*h2/(h1+h2); print("h12=%.9g"%h12)
print("== 18 B4C")
def b4c(T,ph2,ph2o):
    yco2=ph2/ph2o*m.exp(-3605/T+3.427); ych4=ph2o/ph2**3*m.exp(-27350/T+30.5)
    xco=1/(1+1/yco2+1/ych4); xco2=xco/yco2; xch4=1-xco-xco2
    q=xco*(8.238e8-58380*T)+xco2*(8.674e8-67060*T)+xch4*(1.056e8-61430*T)
    st=7*xco+8*xco2+6*xch4; h2=7*xco+8*xco2+4*xch4
    return yco2,ych4,xco,xco2,xch4,st,h2,q
for a in ((1600,1,1),(1600,0.1,1.0),(1200,1.0,0.01),(1800,0.5,5.0)):
    print(a,["%.9g"%v for v in b4c(*a)])
rate=lambda T:9.973e6/60*m.exp(-22647.2/T)
print("C1006(1) exact 9.973e6/60=%.9g"%(9.973e6/60))
for T in (1200.,1500.,1600.,2000.): print(T,"rate=%.9g s^-1, time to 2%%=%.6g s"%(rate(T),0.02/rate(T)))
print("rate with table 1.662e5 at 1600: %.9g ratio %.9g"%(1.662e5*m.exp(-22647.2/1600),1.662e5/(9.973e6/60)))
print("== 20 AIC")
def Kc(r): return 0.32*(1-r) if r<0.4712 else r
for r in (0.01,0.4712,0.5):
    G=m.sqrt(2*g/(1+Kc(r)-r*r)); print(r,"Kc=%.9g Vr(h=1)=%.9g gamma=%.9g"%(Kc(r),G,r*G))
r=0.01; gam=r*m.sqrt(2*g/(1+Kc(r)-r*r)); h0=2.0
for t in (0,50,100): print(t,"h=%.9g"%((m.sqrt(h0)-gam*t/2)**2))
print("t_empty=%.9g"%(2*m.sqrt(h0)/gam))
print("Kc left limit at .4712: %.9g vs right %.9g"%(0.32*(1-0.4712),0.4712))
print("mdot rho=10170 A2=1e-4 Vr(h=1,r=.01)=%.9g"%(10170*1e-4*m.sqrt(2*g*1/(1+Kc(0.01)-1e-4))))
print("== 22 Gelbard")
def Fg(Dt):
    return 1.0006964*(6*m.sqrt(Dt/m.pi)-3*Dt) if Dt<0.155 else 1-6/m.pi**2*m.exp(-m.pi**2*Dt)
def Fexact(Dt,n=20000): return 1-6/m.pi**2*sum(m.exp(-k*k*m.pi**2*Dt)/k**2 for k in range(1,n))
for Dt in (1e-4,0.01,0.1,0.155,0.1551,0.3,1.0): print(Dt,"%.9g exact %.9g"%(Fg(Dt),Fexact(Dt)))
print("jump at .155: left %.9g right %.9g"%(1.0006964*(6*m.sqrt(0.155/m.pi)-3*0.155),1-6/m.pi**2*m.exp(-m.pi**2*0.155)))
print("== 23 HP HX")
mdot,cp,hA,Tw,Tin=2.0,1200.,5000.,900.,600.
b=mdot*cp/hA; To=(Tw+b*Tin)/(1+b); print("beta=%.9g To=%.9g Q=%.9g Qlim=%.9g"%(b,To,mdot*cp*(To-Tin),mdot*cp*(Tw-Tin)))
print("== 24 settling, quench time constants")
print("candling secondary: dMs=0.2*dMm for dMm=100 ->",0.2*100, " F2 form Ms/Mtot*dMm (F2=1, Ms=30,Mtot=100,dMm=10) ->",1*30/100*10)
print("dt_contact=min(dt, Mm dz/(Gmax Asurf)) : Mm=50,dz=0.3,G=1,A=20 -> ",min(5.0,50*0.3/(1*20)))
