import math as m
MM=133.322368
def lp(T,A,B,C): return -A/T+B+C*m.log10(T)
cs1=[(600,9.4e3,21.59,-3.75),(1553,6870.778,7.994503,0.0)]
cs1_raw=[(600,9.4e4,21.59,-3.75),(1553,6870.778,7.994503,0.0)]
cs2=[(600,13600,8.895,0.0),(1229.5,12100,7.675,0.0)]
i2=[(273,3578,17.72,-2.51),(387,3205,23.66536399,-5.18),(457,2176.912045,7.63735266,0.0)]
te=[(273,13940,23.51,-3.52)]
csi=[(600,10420,19.70,-3.02),(894,9678,20.34569113,-3.52),(1553,7303.903158,7.58405103,0.0)]
def P(T,r):
    if T<r[0][0]: return 0.0
    x=[y for y in r if y[0]<=T][-1]; return 10**lp(T,*x[1:])
for name,r in (("Cs vol1 (A=9.4e3)",cs1),("Cs vol2 appx",cs2),("I2",i2),("Te",te),("CsI",csi)):
    print(name,"boundaries:",[ (r[k+1][0], round(lp(r[k+1][0],*r[k][1:]),6), round(lp(r[k+1][0],*r[k+1][1:]),6)) for k in range(len(r)-1)])
    for T in (300.,500.,800.,1000.,1500.,2000.):
        print("  T %.0f P mmHg %.6e Pa %.6e"%(T,P(T,r),P(T,r)*MM))
print("Cs raw 9.4e4 at 1553 below:",lp(1553,9.4e4,21.59,-3.75))
# extrapolation A',B' for Te above Tmax=10000
A,B,C=te[0][1:];Tm=10000.0;L=m.log10(m.e)
Ap=A+C*L*Tm;Bp=B+C*(L+m.log10(Tm))
print("Te A' %.6f B' %.6f"%(Ap,Bp))
for T in (10000.,12000.):
    print(T,"orig %.6f extrap %.6f"%(lp(T,A,B,C),-Ap/T+Bp))
h=1e-3
print("slope orig at Tm %.6e extrap %.6e"%((lp(Tm+h,A,B,C)-lp(Tm-h,A,B,C))/(2*h),Ap/Tm**2))
print("Te peak T", -A*m.log(10)/C)
# saturation conc
R=8.314462618
for name,r,M in (("CsOH",cs1,0.149913),("I2",i2,0.2538008)):
    T=1000. if name=="CsOH" else 400.
    print(name,T,"Csat kg/m3 %.6e"%(P(T,r)*MM*M/(R*T)))
# TRAP-MELT2 step
def step(Ca0,V,s,dt):
    S=sum(a for a,_ in s); al=S/V; ba=sum(a*c for a,c in s)/S; dec=m.exp(-al*dt)
    Ca=ba-(ba-Ca0)*dec
    Mi=[a*(ba-c)*dt-a*(ba-Ca0)*(1-dec)/al for a,c in s]
    return Ca,Mi
Ca,Mi=step(1e-3,100.,[(2.0,0.0),(1.0,3e-3)],30.)
print("TM2 Ca %.10e Mi %s sum %.3e"%(Ca,["%.10e"%x for x in Mi],Ca*100+sum(Mi)-1e-3*100))
# Chapman-Enskog
def DAB(sa,sb,Ma,Mb,T,Patm,Om): return 0.0018583*m.sqrt(T**3*(1/Ma+1/Mb))/(Patm*(0.5*(sa+sb))**2*Om)
print("D I2-air-like T*=1 omega 1.439: cm2/s %.6e"%DAB(4.982,3.617,253.8,28.97,m.sqrt(550*97),1.0,1.439))
print("D Xe-Xe? T=229 omega1.439 %.6e"%DAB(4.055,4.055,131.3,131.3,229.,1.0,1.439))
# area 2-102 vs representative
rho=1000.;m1=rho*m.pi/6*(1e-6)**3;m2=2*m1;Mk=1e-9  # kg/m3 section mass conc
Ap=12*m.pi*(3/(4*m.pi*rho))**(2/3)*Mk/m.log(m2/m1)*(m1**(-1/3)-m2**(-1/3))
# representative: diameter at geometric mean volume?
for lab,mr in (("m1",m1),("geo",m.sqrt(m1*m2))):
    d=(6*mr/(m.pi*rho))**(1/3); N=Mk/mr; print(lab,"rep area %.6e"%(N*m.pi*d*d))
print("2-102 area per m3 %.6e"%Ap)
