import re, statistics as st, math, sys
D = __import__('os').path.dirname(__import__('os').path.abspath(__file__)) + '/'
def rd(f, lab):
    d = {}
    for l in open(D + f):
        m = re.search(lab + r"\s+seed\s+(\d+): kfull = (\S+) k = \S+ ± (\S+), p = (\S+), eps = (\S+)", l)
        if m:
            d[int(m[1])] = (float(m[2]), float(m[4]), float(m[5]))
    return d
M = rd('replay_60f6bb261_moving_hook.out', 'MOVING'); H = rd('replay_60f6bb261_moving_hook.out', 'HOOK'); S = rd('replay_60f6bb261_switch.out', 'SWITCH')
print(len(M), len(H), len(S))
print("bit-identical seeds HOOK==SWITCH (k,p,eps):", [s for s in range(1, 9) if H[s] == S[s]])
print("bit-identical k only:", [s for s in range(1, 9) if H[s][0] == S[s][0]])
def sem(x): return st.stdev(x) / math.sqrt(len(x))
for name, A in (("SWITCH", S), ("HOOK", H)):
    ka = [M[s][0] for s in range(1, 9)]; kb = [A[s][0] for s in range(1, 9)]
    d = [(b - a) * 1e5 for a, b in zip(ka, kb)]
    un = (st.mean(kb) - st.mean(ka)) * 1e5; eu = math.hypot(sem(ka), sem(kb)) * 1e5
    print(name, "mean k MOVING %.5f AT-REST %.5f" % (st.mean(ka), st.mean(kb)))
    print("  unpaired %.0f +- %.0f (%.1f sig)  paired %.0f +- %.0f (%.1f sig) sd %.0f" % (un, eu, abs(un / eu), st.mean(d), sem(d), abs(st.mean(d) / sem(d)), st.stdev(d)))
    for j, n in ((1, 'p'), (2, 'eps')):
        a = [M[s][j] for s in range(1, 9)]; b = [A[s][j] for s in range(1, 9)]
        print("  %s MOVING %.4f AT-REST %.4f diff %+.4f +- %.4f" % (n, st.mean(a), st.mean(b), st.mean(b) - st.mean(a), math.hypot(sem(a), sem(b))))
print("MOVING seed1 %.5f" % M[1][0])
print("sd MOVING %.0f pcm; sd AT-REST %.0f pcm" % (st.stdev([M[s][0] for s in M]) * 1e5, st.stdev([S[s][0] for s in S]) * 1e5))
