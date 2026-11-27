import math
R=8.314462618
mult=lambda g:(2/(g+1))**((g+1)/(2*(g-1)))
for g in [1.1,1.3,1.4,1.8]: print("CVH08 mult",g,repr(mult(g)), "dev from .58 %",repr((mult(g)/0.58-1)*100))
# N2 at 0.8 MPa 450 K, gamma 1.4, Cd .6 A 5e-4
M=0.0280134; p=0.8e6; T=450.0
rho=p*M/(R*T); cs=math.sqrt(1.4*R*T/M); G=rho*cs*mult(1.4)
print("CVH08 rho",repr(rho),"cs",repr(cs),"G",repr(G),"mdot",repr(0.6*5e-4*G))
# steam ideal gas gamma=(3.15+1)/3.15 at 7 MPa, 559 K
gs=(4.15/3.15); Ms=0.01801528; p=7e6; T=559.0
rho=p*Ms/(R*T); cs=math.sqrt(gs*R*T/Ms)
print("CVH08 steam gamma",repr(gs),"rho",repr(rho),"cs",repr(cs),"G(mult@1.4)",repr(rho*cs*mult(1.4)),"G(own gamma)",repr(rho*cs*mult(gs)))
# CVH-09 Eq 6-15 / 6-16
def g2(a,rA,rP,GA,GP):
    return (a*rA+(1-a)*rP)/(a*rA/GA+(1-a)*rP/GP)
rA=36.5; rP=740.0; GA=3000.0; GP=25000.0
for a in [0.0,0.5,0.9,1.0]:
    Gc=g2(a,rA,rP,GA,GP); print("CVH09 a",a,"G2ph",repr(Gc),"vC",repr(Gc/(a*rA+(1-a)*rP)))
# CVH-10 orifice Eq 6-17
for P,rho in [(7e6,740.0),(1e5,958.0)]: print("CVH10 G_O",P,repr(math.sqrt(2*P*rho)))
# CVH-07 bubble efficiency
def eff(zP,zJ,h,Tsat,TP):
    ez=min(max((zP-zJ-0.01)/(1.0*h),0),1); eT=min(max((Tsat-TP-0.1)/5.0,0),1); return ez,eT,ez*eT
for args in [(1.2,0.0,0.5,393.36,390.76),(0.26,0.0,0.5,393.36,380.0),(0.005,0,0.5,393.36,300.0),(3.0,0,0.5,393.36,393.30)]:
    print("CVH07",args,[repr(x) for x in eff(*args)])
