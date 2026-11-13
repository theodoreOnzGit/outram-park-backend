#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Verbatim transcription of DWSIM `PropertyMethods` (GPL-3.0, Daniel Wagner O. de
# Medeiros, 2009), pinned commit 1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766:
#   DWSIM.Thermodynamics/PetroleumCharacterization/PropertyMethods.vb lines 32-470
# VB `x ^ y` -> pow(x, y) (Math.Pow = libm pow); Exp/Log/Log10 -> libm. Operator
# precedence of VB (`^` above unary minus above `*`) is respected by writing the
# pow calls explicitly. usage: petroleum_driver.py <outdir>
import math, struct, sys
from math import exp, log, log10
def pw(a, b): return math.pow(a, b)
def bits(v): return struct.pack('>d', v).hex()

def Tc_RiaziDaubert(PEMe, d15):
    return 9.5233 * exp(-0.0009314 * PEMe - 0.544442 * d15 + 0.00064791 * PEMe * d15) * pw(PEMe, 0.81067) * pw(d15, 0.53691)
def Pc_RiaziDaubert(PEMe, d15):
    t1 = -0.008505 * PEMe - 4.8014 * d15 + 0.005749 * PEMe * d15
    return 31958000000.0 * exp(t1) * pw(PEMe, -0.4844) * pw(d15, 4.0846)
def Tc_Riazi(PEMe, d15):
    t1 = -0.00069 * PEMe - 1.4442 * d15 + 0.000491 * PEMe * d15
    return 35.9413 * exp(t1) * pw(PEMe, 0.7293) * pw(d15, 1.2771)
def Tc_LeeKesler(PEMe, d15):
    return 189.8 + 450.6 * d15 + (0.4244 + 0.1174 * d15) * PEMe + (0.1441 - 1.0069 * d15) * 100000.0 / PEMe
def Pc_LeeKesler(PEMe, d15):
    return 1000000.0 * 0.986923 * exp(5.689 - 0.0566 / d15 - (0.43639 + 4.1216 / d15 + 0.21343 / pw(d15, 2)) * 0.001 * PEMe + (0.47579 + 1.182 / d15 + 0.15302 / pw(d15, 2)) * 0.000001 * pw(PEMe, 2) - (2.4505 + 9.9099 / pw(d15, 2)) * 0.0000000001 * pw(PEMe, 3))
def Tc_Farah2(A, B): return 731.968 + 291.952 * A - 704.998 * B
def Tc_Farah3(A, B, PEMe): return 104.0061 + 38.75 * A - 41.6097 * B + 0.7831 * PEMe
def Tc_Farah4(A, B, d15, PEMe): return 196.793 + 90.205 * A - 221.051 * B + 309.534 * d15 + 0.524 * PEMe
def Pc_Farah2(A, B): return exp(20.0056 - 9.8758 * log(A) + 12.2326 * log(B))
def Pc_Farah3(A, B, PEMe): return exp(11.2037 - 0.5484 * A + 1.9242 * B + 510.1272 / PEMe)
def Pc_Farah4(A, B, PEMe, d15): return exp(28.7605 + 0.7158 * log(A) - 0.2796 * log(B) + 2.3129 * log(d15) - 2.4027 * log(PEMe))
def AcentricFactor_LeeKesler(Tc, Pc, PEMM):
    return (-log(Pc / 101325) - 5.92714 + 6.09648 / (PEMM / Tc) + 1.28862 * log(PEMM / Tc) - 0.169347 * pw(PEMM / Tc, 6)) / (15.2518 - 15.6875 / (PEMM / Tc) - 13.4721 * log(PEMM / Tc) + 0.43577 * pw(PEMM / Tc, 6))
def AcentricFactor_Korsten(Tc, Pc, PEMV):
    return 0.5899 * pw(PEMV / Tc, 1.3) / (1 - pw(PEMV / Tc, 1.3)) * log10(Pc / 101325) - 1
def MW_Winn(PEMe, d15): return 0.00005805 * pw(PEMe, 2.3776) / pw(d15, 0.9371)
def MW_Riazi(PEMe, d15):
    t1 = 0.0002097 * PEMe - 7.78 * d15 + 0.00208476 * PEMe * d15
    return 42.965 * exp(t1) * pw(PEMe, 1.26007) * pw(d15, 4.98308)
def MW_Riazi_visc(v37, v98, d15):
    return 223.56 * pw(v37, 1.2228 * d15 - 1.2435) * pw(v98, -3.038 * d15 + 3.4758) * pw(d15, -0.6665)
def MW_LeeKesler(PEMe, d15):
    t1 = -12272.6 + 9486.4 * d15 + (8.3741 - 5.9917 * d15) * PEMe
    t2 = (1 - 0.77084 * d15 - 0.02058 * pw(d15, 2)) * (0.7465 - 222.466 / PEMe) * 10000000.0 / PEMe
    t3 = (1 - 0.80882 * d15 - 0.02226 * pw(d15, 2)) * (0.3228 - 17.335 / PEMe) * 1000000000000.0 / pw(PEMe, 3)
    return t1 + t2 + t3
def MW_Farah2(A, B): return exp(6.8117 + 1.3372 * A - 3.6283 * B)
def MW_Farah4(A, B, d15, PEMe): return exp(4.0397 + 0.1362 * A - 0.3406 * B - 0.9988 * d15 + 0.0039 * PEMe)
def Visc37_Abbott(PEMe, d15):
    Kw = pw(1.8 * PEMe, 1 / 3) / d15
    API = 141.5 / d15 - 131.5
    v1 = pw(10, 4.39371 - 1.94733 * Kw + 0.12769 * pw(Kw, 2) + 0.00032629 * pw(API, 2) - 0.0118246 * Kw * API + (0.171617 * pw(Kw, 2) + 10.9943 * API + 0.0950663 * pw(API, 2) - 0.860218 * Kw * API) / (API + 50.3642 - 4.78231 * Kw))
    return v1 * 0.000001
def Visc98_Abbott(PEMe, d15):
    Kw = pw(1.8 * PEMe, 1 / 3) / d15
    API = 141.5 / d15 - 131.5
    v1 = pw(10, -0.463634 - 0.166532 * API + 0.000513447 * pw(API, 2) - 0.00848995 * API * Kw + (0.080325 * Kw + 1.24899 * API + 0.19768 * pw(API, 2)) / (API + 26.786 - 2.6296 * Kw))
    return v1 * 0.000001
def ViscT_Beg_Amin(T, T50ASTM, d15):
    API = 141.5 / d15 - 131.5
    B = exp(5.471 + 0.00342 * T50ASTM)
    A = -0.0339 * pw(API, 0.188) + 0.241 * T50ASTM / B
    return A * exp(B / T) * 0.000001
def CalcLogZ(Vc):
    c = d = e = f = g = h = 0.0   # VB Dim default 0
    if Vc < 2:
        c = exp(-1.14883 - 2.65868 * Vc)
        if Vc < 1.65:
            d = exp(-0.0038138 - 12.5645 * Vc)
            if Vc < 0.9:
                e = exp(5.46491 - 37.6289 * Vc)
                if Vc < 0.3:
                    f = exp(13.0458 - 74.6851 * Vc)
                    g = exp(37.4619 - 192.643 * Vc)
                    if Vc < 0.24:
                        h = exp(80.4945 - 400.468 * Vc)
    return log10(log10(Vc + 0.7 + c - d + e - f + g - h))
def ViscWaltherASTM_A(T1, v1, T2, v2):
    tt1 = log10(T1); tt2 = log10(T2)
    Vc1 = v1 * 1000000.0; Vc2 = v2 * 1000000.0
    logz1 = CalcLogZ(Vc1); logz2 = CalcLogZ(Vc2)
    B = (logz2 - logz1) / (tt1 - tt2)
    return logz2 + B * tt2
def ViscWaltherASTM_B(T1, v1, T2, v2):
    tt1 = log10(T1); tt2 = log10(T2)
    Vc1 = v1 * 1000000.0; Vc2 = v2 * 1000000.0
    logz1 = CalcLogZ(Vc1); logz2 = CalcLogZ(Vc2)
    return (logz2 - logz1) / (tt1 - tt2)
def ViscTwu(T, T1, T2, v1, v2):
    vk1 = v1 * 1000000; vk2 = v2 * 1000000
    T = 1.8 * T; T1 = 1.8 * T1; T2 = 1.8 * T2
    Z1 = vk1 + 0.7 + math.exp(-1.47 - 1.84 * vk1 - 0.51 * pw(vk1, 2))
    Z2 = vk2 + 0.7 + math.exp(-1.47 - 1.84 * vk2 - 0.51 * pw(vk2, 2))
    var1 = (math.log(math.log(Z1)) - math.log(math.log(Z2)))
    var2 = (math.log(T1) - math.log(T2))
    B = var1 / var2
    Z = math.exp(math.exp(math.log(math.log(Z1)) + B * (math.log(T) - math.log(T1))))
    tmp = Z - 0.7 - math.exp(-0.7487 - 3.295 * (Z - 0.7) + 0.6119 * pw(Z - 0.7, 2) - 0.3193 * pw(Z - 0.7, 3))
    return tmp * 0.000001
def d15_v37v98(v37, v98):
    vc98 = v98 * 1000000.0; vc37 = v37 * 1000000.0
    return 0.7717 * pw(vc37, 0.1157) * pw(vc98, -0.1616)
def d20d15(d15):
    if d15 < 0.934: return -0.0166 * pw(d15, 2) + 1.0311 * d15 - 0.0182
    return 1.2394 * pw(d15, 3) - 3.7387 * pw(d15, 2) + 4.7524 * d15 - 1.2566
def d15d20(d20):
    if d20 < 0.639: return 0.0638 * pw(d20, 2) + 0.8769 * d20 + 0.0628
    return 0.0156 * pw(d20, 2) + 0.9706 * d20 + 0.0175
def d15_Riazi(MW): return 1.07 - math.exp(3.56073 - 2.93886 * pw(MW, 0.1))

TB = [350.0, 400.0, 450.0, 500.0, 550.0, 600.0, 650.0, 700.0, 800.0]
SG = [0.70, 0.75, 0.80, 0.85, 0.90, 0.95]
AB = [(10.0, 0.3), (11.5, 0.5), (12.8, 0.8)]
VV = [(2e-6, 1e-6), (8e-6, 2.5e-6), (3e-5, 6e-6), (5e-5, 9e-6)]
MWS = [70.0, 120.0, 200.0, 300.0, 450.0]
D20 = [0.62, 0.66, 0.75, 0.85, 0.95]

def main():
    out = open(f"{sys.argv[1]}/petroleum_upstream_output.txt", 'w')
    out.write("# fn arg_bits... | result_bits   (T K, d15 -, MW g/mol, v m2/s, Pc Pa)\n")
    def emit(name, fn, *args):
        try:
            r = fn(*args)
        except (ValueError, OverflowError):   # .NET Math.Log(<0) -> NaN, Exp overflow -> +Inf
            r = float('nan')
        out.write(f"{name} {' '.join(bits(a) for a in args)} | {bits(r)}\n")
    for t in TB:
        for d in SG:
            for nm, fn in [('Tc_RiaziDaubert', Tc_RiaziDaubert), ('Pc_RiaziDaubert', Pc_RiaziDaubert), ('Tc_Riazi', Tc_Riazi), ('Tc_LeeKesler', Tc_LeeKesler), ('Pc_LeeKesler', Pc_LeeKesler),
                           ('MW_Winn', MW_Winn), ('MW_Riazi', MW_Riazi), ('MW_LeeKesler', MW_LeeKesler), ('Visc37_Abbott', Visc37_Abbott), ('Visc98_Abbott', Visc98_Abbott)]:
                emit(nm, fn, t, d)
            tc = Tc_RiaziDaubert(t, d) ; pc = Pc_RiaziDaubert(t, d)
            emit('AcentricFactor_LeeKesler', AcentricFactor_LeeKesler, tc, pc, t)
            emit('AcentricFactor_Korsten', AcentricFactor_Korsten, tc, pc, t)
            emit('ViscT_Beg_Amin', ViscT_Beg_Amin, 310.0, t, d)
            for a, b in AB:
                emit('Tc_Farah4', Tc_Farah4, a, b, d, t)
                emit('MW_Farah4', MW_Farah4, a, b, d, t)
                emit('Pc_Farah4', Pc_Farah4, a, b, t, d)
    for a, b in AB:
        emit('Tc_Farah2', Tc_Farah2, a, b); emit('Pc_Farah2', Pc_Farah2, a, b); emit('MW_Farah2', MW_Farah2, a, b)
        for t in TB:
            emit('Tc_Farah3', Tc_Farah3, a, b, t); emit('Pc_Farah3', Pc_Farah3, a, b, t)
    for v37, v98 in VV:
        for d in SG:
            emit('MW_Riazi_visc', MW_Riazi_visc, v37, v98, d)
        emit('d15_v37v98', d15_v37v98, v37, v98)
    for (v1, v2) in VV:
        emit('ViscWaltherASTM_A', ViscWaltherASTM_A, 310.9, v1, 372.0, v2)
        emit('ViscWaltherASTM_B', ViscWaltherASTM_B, 310.9, v1, 372.0, v2)
        for t in [300.0, 340.0, 420.0]:
            emit('ViscTwu', ViscTwu, t, 310.9, 372.0, v1, v2)
    # CalcLogZ branch coverage through Walther: Vc spans every nested branch
    for vc1, vc2 in [(0.2, 0.5), (0.25, 0.95), (1.0, 1.7), (1.8, 2.5), (0.29, 0.31)]:
        emit('ViscWaltherASTM_A', ViscWaltherASTM_A, 310.9, vc1 * 1e-6, 372.0, vc2 * 1e-6)
        emit('ViscWaltherASTM_B', ViscWaltherASTM_B, 310.9, vc1 * 1e-6, 372.0, vc2 * 1e-6)
    for d in SG + [0.93, 0.934, 0.94]:
        emit('d20d15', d20d15, d)
    for d in D20:
        emit('d15d20', d15d20, d)
    for m in MWS:
        emit('d15_Riazi', d15_Riazi, m)
    out.close()

if __name__ == '__main__':
    main()
