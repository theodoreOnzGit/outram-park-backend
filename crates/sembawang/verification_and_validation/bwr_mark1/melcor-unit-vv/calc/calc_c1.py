from math import *
sig=5.670374419e-8
def p(*a): print(*a)
# 01 Zr emissivity
def ezr(dr,tmax,tab=((0,0.325),(3.8799999e-6,0.808448),(1e-3,0.758642))):
    if dr<=tab[1][0]: e=tab[0][1]+(tab[1][1]-tab[0][1])*dr/tab[1][0]
    elif dr<=tab[2][0]: e=tab[1][1]+(tab[2][1]-tab[1][1])*(dr-tab[1][0])/(tab[2][0]-tab[1][0])
    else: e=tab[2][1]
    if tmax>1500: e*=exp((1500-tmax)/300)
    return min(max(e,0.325),0.9999)
p("01",ezr(0,1000),ezr(2e-6,1000),ezr(3.8799999e-6,1000),ezr(5e-4,1000),ezr(2e-3,1000),ezr(2e-3,1600),ezr(5e-4,1800),ezr(2e-3,3000))
# eq 2-7/2-8 check
p("01b",0.325+0.1246e6*2e-6, 0.808642-50*5e-4)
ours=lambda dr: 0.325+(0.808-0.325)*dr/3.88e-6 if dr<=3.88e-6 else (0.808+(0.759-0.808)*(dr-3.88e-6)/(1e-3-3.88e-6) if dr<=1e-3 else 0.759)
p("01c ours",ours(2e-6),ours(5e-4),ours(2e-3), (ours(5e-4)/ezr(5e-4,1000)-1))
# 02 steel
es=lambda T: min(max(0.25617+0.0003474*(T-616.4833),1e-4),0.9999)
p("02",es(300),es(616.4833),es(1000),es(1500),es(3000),"T at cap",616.4833+(0.9999-0.25617)/0.0003474, "T at floor",616.4833+(1e-4-0.25617)/0.0003474, "ours-melcor",(0.25617+3.474e-4*(1000-616.48))-es(1000))
p("02F", (650-32)/1.8+273.15, 0.000193*1.8)
# 04 gray two-surface with medium
def two(A1,A2,AF,e1,e2,T1,T2,em=0.0,Tm=0.0):
    tau=1-em
    Eb1,Eb2,Ebm=sig*T1**4,sig*T2**4,sig*Tm**4
    # unknown J1,J2: A1 e1/(1-e1)(Eb1-J1) = A1 em (J1-Ebm) + AF tau (J1-J2)
    a1=A1*e1/(1-e1); a2=A2*e2/(1-e2); g=AF*tau
    # a1 Eb1 + A1 em Ebm = (a1+A1 em+g)J1 - g J2
    M=[[a1+A1*em+g,-g],[-g,a2+A2*em+g]]; b=[a1*Eb1+A1*em*Ebm,a2*Eb2+A2*em*Ebm]
    det=M[0][0]*M[1][1]-M[0][1]*M[1][0]
    J1=(b[0]*M[1][1]-M[0][1]*b[1])/det; J2=(M[0][0]*b[1]-M[1][0]*b[0])/det
    q1=a1*(Eb1-J1); q12=g*(J1-J2); q1m=A1*em*(J1-Ebm); q2=a2*(Eb2-J2); q2m=A2*em*(J2-Ebm)
    return q1,q12,q1m,q2,q2m
r=two(1,1,1,0.8,0.5,1500,1000); p("04a",r, sig*(1500**4-1000**4)/(1/0.8+1/0.5-1))
r=two(1,1,0.25,0.325,0.95,1500,560); p("04b",r, 0.25*sig*(1500**4-560**4)/(1/0.325+1/0.95-1))
r=two(1,1,1,0.8,0.5,1500,1000,0.3,1200); p("04c",r, r[0]+r[3], r[2]+r[4])
# 06 FCELR
def fcelr(Acell,K,A1,A2):
    a1L1=A1/(K*Acell); a2L2=A2/(K*Acell)
    AF=Acell*K*(1-exp(-a1L1))*(1-exp(-a2L2))
    AFe=2*(Acell*K)**2/(A1+A2)*((1-(1+a1L1)*exp(-a1L1))*(1-exp(-a2L2))+(1-exp(-a1L1))*(1-(1+a2L2)*exp(-a2L2)))
    return AF,AFe
p("06",fcelr(1,1,2,3), fcelr(1,1,200,300), 4/500, fcelr(1,1,1e-3,2e-3), 1e-3*2e-3, 0.5*(1e-6+4e-6)/3e-3, fcelr(1,1,1e-3,300), 1e-3)
# 07 gap
def hgap(kf,R,kg,dr,ef,ec,Tf,Tc,hcf=None):
    hf=4*kf/R; hg=kg/dr; hr=4*sig*(0.5*(Tc+Tf))**3/(1/ef+1/ec-1)
    gas=hg if hcf is None else 1/(1/hg+1/hcf)
    return 1/(1/hf+1/(gas+hr)),hf,hg,hr
p("07",hgap(3.0,0.0052,0.3,8e-5,0.8,0.325,1500,1000))
p("07ours",1/(1/(4*3/0.0052)+1/5000), hgap(3,0.0052,5000*8e-5/1,8e-5,1e-12+0.0001,0.0001,1500,1000))
# 08 eff heat capacity
Ccl,Cfu,h,A,dt=1000.,5000.,5000.,0.5,0.1
p("08",Ccl+Cfu*h*A*dt/(Cfu+h*A*dt), Ccl+Cfu*h*A*1e6/(Cfu+h*A*1e6))
# 09 convection
def nu_forced(Re,Pr,Dh,z=1000.):
    F=z/(Dh*Re*Pr); g=1+0.00826/(F+0.0011)
    return max(4.36*g,0.023*Re**0.8*Pr**0.4),4.36*g,0.023*Re**0.8*Pr**0.4,g
p("09",nu_forced(5000,1.0,0.012),nu_forced(100,0.9,0.012),nu_forced(1000,1,0.012,z=0.1))
# Re where DB=4.36 at Pr 1
p("09x",(4.36/0.023)**(1/0.8))
# 10 free
nf=lambda Ra,L,Dh: max(0.18*Ra**0.25,0.065*Ra**(1/3))*(L/Dh)**(-1/9)
p("10",0.18*1e6**0.25*(4/0.012)**(-1/9),0.065*1e6**(1/3)*(4/0.012)**(-1/9),nf(1e6,4,.012),"cross Ra",(0.18/0.065)**12)
# 11 debris
p("11",2+0.6*100**0.5*0.7**(1/3),2+0.6*1e4**0.25*0.7**(1/3))
# 12 boiling MARCH
hb=lambda P,dT: 34.5*P**0.25*dT**1.523 if dT<23.4 else 1.41e7*P**0.25*dT**-2.575
p("12",hb(7e6,10),hb(7e6,23.39999),hb(7e6,23.4),hb(7e6,100), sig*0.4*(1100**4-560**4)/(1100-560))
