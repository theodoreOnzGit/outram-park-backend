from math import *
def p(*a): print(*a)
R=8.314462618
# 13 Lipinski
def lip(rl,rv,hlv,d,eps,L,c=0.756,zcap=0.089,emin=0.15,g=9.81):
    eps=max(eps,emin); lc=zcap*5e-4/d
    num=rv*(rl-rv)*g*d*eps**3*(1+lc/L); den=(1-eps)*(1+(rv/rl)**0.25)**4
    return c*hlv*sqrt(num/den), lc
p("13",lip(915,2.67,2.108e6,0.01,0.4,1.0),lip(915,2.67,2.108e6,5e-4,0.4,1.0),lip(915,2.67,2.108e6,0.01,0.05,1.0),lip(915,2.67,2.108e6,0.01,0.15,1.0))
# eq 2-115 capillary head check for 0.5mm: sigma=0.0589 (at 373K) cos=1 eps .4
p("13b", 6*0.0589*1*(1-0.4)/(0.4*5e-4*(958-0.6)*9.81))
# 14 quench
f=1.0
for i in range(10): f=min(1,f*exp(-10/360)+0)
p("14", f, exp(-100/360), 360*log(100), min(1,0.9*exp(-10/360)+0.05), 5-5*0.4)
# 15 ACOPO
def acopo(rho,mu,cp,k,beta,Q,H,A=0.3,n=0.22,g=9.81):
    nu=mu/rho; Pr=mu*cp/k; Ra=g*beta*Q*H**5/(k*nu*nu)*Pr; Nu=A*Ra**n; return Ra,Nu,Nu*k/H
p("15",acopo(8000,4.4e-3,500,3,1.05e-4,1e6,0.5))
p("15b kW units",acopo(8000,4.4e-3,500,3e-3,1.05e-4,1e3,0.5))
p("15c Bonnet j=2", 0.381*acopo(8000,4.4e-3,500,3,1.05e-4,1e6,0.5)[0]**0.234)
p("15d transient Q", 0.5*(1e6*1.0 + 2e6)/1.0)
# 17 UH
def K(T):
    if T<1100: return 0
    lo=lambda t:29.6*exp(-16820/t); hi=lambda t:87.9*exp(-16610/t)
    if T<=1853: return lo(T)
    if T>=1873: return hi(T)
    return (hi(1873)*(T-1853)+lo(1853)*(1873-T))/20
for T in (1099.9,1100,1500,1853,1863,1873,2000,2400): p("17",T,repr(K(T)))
W0=0.1; p("17W",sqrt(W0**2+K(1500)*100), sqrt(K(1500)*3600), sqrt(K(2000)*60))
# low branch extrapolated vs high at 1873
p("17j",29.6*exp(-16820/1873),87.9*exp(-16610/1853))
# 18 O2
KO=lambda T: 50.4*exp(-14630/T) if T>=1100 else 0
p("18",KO(1500),KO(2000), K(1500))
# rates for parabolic: dW/dt = K/(2W)
W=0.5; rs=K(1500)/(2*W); ro=KO(1500)/(2*W); p("18b",rs,ro,max(rs,ro), rs/(rs+ro))
# 19 optional models
def k174(T,T1,T2,A1,B1,A2,B2):
    if T<T1: return A1*exp(-B1/T)
    if T>T2: return A2*exp(-B2/T)
    return (A1*exp(-B1/T1)*(T2-T)+A2*exp(-B2/T2)*(T-T1))/(T2-T1)
p("19 CP",k174(1500,1853,1873,294.2,20111,87.9,16610), "CP at 1500 own",294.2*exp(-20111/1500))
p("19 LS",k174(1500,1800,1900,426.0,20953,26778.5,26461), k174(1850,1800,1900,426.0,20953,26778.5,26461))
p("19 Sok",k174(1500,1763,1783,1291.2,23040,798.5,20800))
# 20 heats
MZr=91.224e-3
HfZrO2=-1100.6; Hv=-241.826; Hl=-285.83
p("20 vap",(-HfZrO2+2*Hv)/MZr/1e3*1e3/1e3, "liq",(-HfZrO2+2*Hl)/91.224, "5797 kJ/kg -> kJ/mol",5797*0.091224, "ratio", 6763.3/5797)
p("20 exact vap MJ/kg",(1100.6-2*241.826)/0.091224/1000,"liq",(1100.6-2*285.83)/0.091224/1000)
# 21 SS
xfe,xcr,xni=0.74,0.18,0.08
p("21 heat kJ/kg",(-249.5*xfe+2442*xcr)/(xfe+xcr), "air",(4869*xfe+1091*xcr)/(xfe+xcr))
Kss=lambda T: 2.42e9*exp(-42400/T) if T>=1100 else 0
p("21 K",Kss(1100),Kss(1500),Kss(1700))
mol=xfe/55.845e-3+1.5*xcr/51.996e-3+xni/58.693e-3
p("21 mol h2o per kg",mol, mol*18.015e-3, mol*2.016e-3)
# 22 diffusion
p("22 C1003 Zr",0.091224/(2*R), "Fe n=1",0.055845/R,"Fe3O4 n=4/3",0.055845/(4/3)/R,"ours steel", 1/mol/R)
kc=0.05; P=1e5; Tf=1500
p("22 rate",0.00548*kc*P/Tf, 0.091224/(2*R)*kc*P/Tf)
# 23 B4C
def b4c(T,ph2,ph2o):
    y1=ph2/ph2o*exp(-3605/T+3.427); y2=ph2o/ph2**3*exp(-27350/T+30.50)
    xco=1/(1+1/y1+1/y2); xco2=xco/y1; xch4=1-xco-xco2
    q=(xco*(8.238e8-58380*T)+xco2*(8.674e8-67060*T)+xch4*(1.056e8-61430*T))
    return y1,y2,xco,xco2,xch4,7*xco+8*xco2+6*xch4,7*xco+8*xco2+4*xch4,q
p("23a",b4c(1600,1,1)); p("23b",b4c(1600,0.1,10)); p("23c",b4c(1200,20,0.1))
rb=lambda T: 9.973e6/60*exp(-22647.2/T)
p("23 rate",rb(1500),rb(1600),rb(1200), 9.973e6/60, "time to 2% at 1500",0.02/rb(1500))
# 24 solidus ZrO2-UO2: pairs
def tsij(xa,Ta,Tb,xe_a,Te):
    # xa mole fraction of a in the pair; eutectic composition xe_a
    xb=1-xa
    if xa>=xe_a: # a in excess
        fe=xb/(1-xe_a); return fe*Te+(1-fe)*Ta
    fe=xa/xe_a; return fe*Te+(1-fe)*Tb
p("24",tsij(0.3,2990,3113,0.5,2800),tsij(0.5,2990,3113,0.5,2800),tsij(0.8,2990,3113,0.5,2800), "simple molewt",0.3*2990+0.7*3113)
# 25 TaT
L=lambda T:1/(2.16e-11*exp(7e-3*T))
for T in (2000,2300,2500,2700): p("25",T,L(T),L(T)/3600)
# damage after 1 h at 2300 + 30 min at 2400
D=3600/L(2300)+1800/L(2400); p("25D",D)
# 26 candling
p("26",0.2*10.0, 0.5*3.0/12.0*4.0, min(1.0, 200*1/(1*0.5)))
# 27 area
p("27", 6*0.01/0.01, 6*(1.0/6550)/0.01, 6*0.1/0.002)
