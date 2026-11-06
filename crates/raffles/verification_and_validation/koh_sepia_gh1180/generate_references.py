# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 OUTRAM PARK contributors
"""Reference outputs for the Kennedy-O'Hagan calibration port (gh:#1180).

Runs the UPSTREAM code itself -- SEPIA (LANL, github.com/lanl/SEPIA, BSD-3,
the Python successor of GPMSA, Higdon et al. 2004/2008) from a clone, plus the
numpy / scipy it calls -- on fixed inputs, and writes JSON into references/.
The Rust port (crates/raffles/src/bayesian/koh/) is compared against these by
crates/raffles/tests/koh_sepia_gh1180.rs. Python is used only here.

Command (from the workspace root; SEPIA_DIR is a clone of lanl/SEPIA):

    SEPIA_DIR=<clone> <venv>/bin/python -I \
        crates/raffles/verification_and_validation/koh_sepia_gh1180/generate_references.py

UPSTREAM FINDING (recorded in the V&V record): SEPIA's univariate-output
setup carries no discrepancy. `SepiaData.create_D_basis` returns early for
scalar output ("Model has univariate output, skipping discrepancy"), and
forcing `obs_data.D` crashes `SepiaModel.__init__` at
`self.num.v = v.reshape((n*pv, 1))` (v is empty for scalar output). Yet the
likelihood (`SepiaLogLik.compute_log_lik`, "for scalar output: SigVUgW =
[SigV+SigUgW] + SigObs/lamOs") and the dense predictor (`SepiaPredict.uvPred`,
"if num.scalar_out: if not no_D: ...") both still carry GPMSA's scalar
y = eta + delta + eps branch. So three kinds of case are generated:

  * NO_D: SEPIA's supported univariate model (no discrepancy), set up by
    SepiaModel itself; likelihood, prior, posterior, and predictions from
    BOTH SepiaFullPrediction (uvPredSep, upstream's default) and uvPred.
  * WITH_D: the same data; the model is set up by SepiaModel as NO_D, then
    the four setup fields the crash leaves unset are completed by hand
    (num.pv = 1, num.lamVzGroup = [0], num.lamVzGnum = 1, and
    set_params_full's parameters and priors with the corrections SepiaModel
    computes for scalar output, all zero). Upstream's compute_log_lik then
    runs its scalar + discrepancy branch unchanged. Predictions use upstream
    uvPred's source with ONLY its first line (the `raise RuntimeError`
    guard) removed; its body is executed unchanged.
    One upstream defect in that branch is corrected (SigUUx's block, see
    below), following upstream's own GPMSA comment.
  * SIM_ONLY: SepiaData without observations (the emulator alone).

Plus an independent check of the KOH algebra (not SEPIA): the dense
multivariate-normal log density of the stacked (u, w) under the joint
covariance, which must equal SEPIA's decomposed logLik plus
(n + m)/2 ln(2 pi), and a seeded SEPIA MCMC run on WITH_D for the
statistical comparison of samplers.
"""

import inspect
import json
import os
import platform
import subprocess
import sys
import time

SEPIA_DIR = os.environ["SEPIA_DIR"]
sys.path.insert(0, SEPIA_DIR)

import numpy as np  # noqa: E402
import scipy  # noqa: E402
import scipy.stats  # noqa: E402

import sepia.SepiaPredict as SP  # noqa: E402
from sepia.SepiaData import SepiaData  # noqa: E402
from sepia.SepiaModel import SepiaModel  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "references")


def commit():
    try:
        return subprocess.check_output(["git", "-C", SEPIA_DIR, "rev-parse", "HEAD"], text=True).strip()
    except Exception:  # noqa: BLE001
        return "unknown"


# --- the toy problem (fixed inputs, all written to the JSON) ---------------
def eta(x, t1, t2):
    return x * (1.0 + 2.0 * t1) + 0.5 * np.sin(2.0 * np.pi * x + 3.0 * t2)


rs = np.random.RandomState(1180)
M, N = 40, 10
lhs = np.empty((M, 3))
for j in range(3):
    lhs[:, j] = (rs.permutation(M) + rs.rand(M)) / M
x_sim = lhs[:, [0]]
t_sim = lhs[:, 1:3]
y_sim = eta(x_sim[:, 0], t_sim[:, 0], t_sim[:, 1]).reshape(-1, 1)
x_obs = np.linspace(0.05, 0.95, N).reshape(-1, 1)
T_TRUE = (0.3, 0.7)
y_obs = (eta(x_obs[:, 0], *T_TRUE) + 0.25 * x_obs[:, 0] ** 2 + 0.02 * rs.randn(N)).reshape(-1, 1)


def make_data(with_obs=True):
    if with_obs:
        d = SepiaData(x_sim=x_sim.copy(), t_sim=t_sim.copy(), y_sim=y_sim.copy(), x_obs=x_obs.copy(), y_obs=y_obs.copy())
    else:
        d = SepiaData(x_sim=x_sim.copy(), t_sim=t_sim.copy(), y_sim=y_sim.copy())
    d.transform_xt()
    d.standardize_y()
    return d


def complete_discrepancy(model):
    """The four setup fields SepiaModel.__init__ cannot reach for scalar output."""
    model.num.pv = 1
    model.num.lamVzGroup = [0]
    model.num.lamVzGnum = 1
    model.set_params_full(0, 0, 0, 0)


NAMES_FULL = ["theta", "betaV", "betaU", "lamVz", "lamUz", "lamWs", "lamWOs", "lamOs"]
NAMES_NOD = ["theta", "betaU", "lamUz", "lamWs", "lamWOs", "lamOs"]
NAMES_SIM = ["betaU", "lamUz", "lamWs", "lamWOs"]

PARAM_SETS = [
    "init",
    {"theta": [[0.31, 0.68]], "betaV": [[0.8]], "betaU": [[2.0], [0.5], [1.5]], "lamVz": [[30.0]], "lamUz": [[1.3]], "lamWs": [[2000.0]], "lamWOs": [[5000.0]], "lamOs": [[400.0]]},
    {"theta": [[0.6, 0.2]], "betaV": [[0.05]], "betaU": [[5.0], [0.2], [3.0]], "lamVz": [[5.0]], "lamUz": [[0.7]], "lamWs": [[300.0]], "lamWOs": [[800.0]], "lamOs": [[50.0]]},
    {"theta": [[0.1, 0.9]], "betaV": [[3.0]], "betaU": [[0.9], [1.1], [0.4]], "lamVz": [[200.0]], "lamUz": [[2.5]], "lamWs": [[9000.0]], "lamWOs": [[20000.0]], "lamOs": [[3000.0]]},
    # rho above SEPIA's 0.999 cap for one betaU (exp(-0.001/4) = 0.99975)
    {"theta": [[0.45, 0.55]], "betaV": [[0.3]], "betaU": [[0.001], [0.8], [2.2]], "lamVz": [[12.0]], "lamUz": [[1.0]], "lamWs": [[1000.0]], "lamWOs": [[1500.0]], "lamOs": [[800.0]]},
]


def set_vals(model, ps, names):
    if ps == "init":
        return
    for k in names:
        getattr(model.params, k).val = np.array(ps[k], dtype=float)


def vals(model, names):
    return {k: getattr(model.params, k).val.tolist() for k in names}


def dense_check(model, with_d):
    """Independent: ln N((u; w); 0, Sigma_joint) + (n+m)/2 ln 2 pi."""
    num, prm = model.num, model.params
    from sepia.SepiaDistCov import SepiaDistCov
    xt = np.concatenate((model.data.x, np.tile(prm.theta.val, (N, 1))), axis=1)
    bu, lu = prm.betaU.val[:, 0], prm.lamUz.val[0, 0]
    lws, lwos, los = prm.lamWs.val[0, 0], prm.lamWOs.val[0, 0], prm.lamOs.val[0, 0]
    sig_u = SepiaDistCov(xt).compute_cov_mat(bu, lu) + np.eye(N) / lws
    sig_uw = SepiaDistCov(xt, model.data.zt).compute_cov_mat(bu, lu)
    sig_w = SepiaDistCov(model.data.zt).compute_cov_mat(bu, lu) + np.eye(M) * (1 / lwos + 1 / lws)
    top = sig_u + num.SigObs / los
    if with_d:
        top = top + SepiaDistCov(model.data.x).compute_cov_mat(prm.betaV.val[:, 0], prm.lamVz.val[0, 0])
    big = np.block([[top, sig_uw], [sig_uw.T, sig_w]])
    z = np.concatenate((num.u[:, 0], num.w[:, 0]))
    lp = scipy.stats.multivariate_normal(mean=np.zeros(N + M), cov=big).logpdf(z)
    return float(lp + 0.5 * (N + M) * np.log(2 * np.pi))


def lik_cases(kind):
    out = []
    for ps in PARAM_SETS:
        if kind == "sim_only":
            model = SepiaModel(make_data(False))
            names = NAMES_SIM
        else:
            model = SepiaModel(make_data(True))
            names = NAMES_NOD
            if kind == "with_d":
                complete_discrepancy(model)
                names = NAMES_FULL
        set_vals(model, ps, names)
        ll = float(model.logLik("all"))
        lp = float(model.log_prior())
        lpost = float(model.logPost("all"))
        case = {"params": vals(model, names), "log_lik": ll, "log_prior": lp, "log_post": lpost,
                "log_lik_w": float(np.sum(model.num.SigWl))}
        if kind != "sim_only":
            case["dense_log_lik"] = dense_check(model, kind == "with_d")
        out.append(case)
    return out


# --- predictions -------------------------------------------------------------
_src = inspect.getsource(SP.uvPred)
_guard = "    raise RuntimeError('uvPred should not normally be invoked; uvPredSep is the correct method')\n"
assert _src.count(_guard) == 1
_ns = dict(SP.__dict__)
# UPSTREAM DEFECT in that dead branch (SepiaPredict.py line 704 at the commit
# above): for scalar output with discrepancy, upstream writes
#     SigCross[n*pv:,:npred*pv]=SigUUx
# (a 40-row slice given a 10-row block: ValueError on any run), where its own
# GPMSA comment, "SigCross=[SigVVx SigUUx; zeros(m*pu,npred*pv) SigWUx]", puts
# SigUUx in the first n rows and the u columns. The corrected line below is
# that comment, and is the ONLY other change to upstream's body.
_bug = "                SigCross[n*pv:,:npred*pv]=SigUUx\n"
_fix = "                SigCross[:n*pv,npred*pv:]=SigUUx\n"
assert _src.count(_bug) == 1
exec(_src.replace(_guard, "", 1).replace(_bug, _fix, 1), _ns)  # noqa: S102
uvPred_unguarded = _ns["uvPred"]

X_PRED = np.array([[0.0], [0.27], [0.5], [0.83], [1.0]])
SAMPLES = [PARAM_SETS[1], PARAM_SETS[2], PARAM_SETS[3]]


def samples_dict(names):
    d = {}
    for k in names:
        d[k] = np.array([np.array(s[k], dtype=float).reshape(-1, order="F") for s in SAMPLES])
    return d


def pred_cases():
    res = {}
    # NO_D: upstream default (uvPredSep) and the dense uvPred.
    model = SepiaModel(make_data(True))
    sd = samples_dict(NAMES_NOD)
    p_sep = SP.SepiaFullPrediction(x_pred=X_PRED, samples=sd, model=model, storeRlz=False, storeMuSigma=True)
    p_dense = SP.SepiaPrediction(x_pred=X_PRED, samples=sd, model=model, storeRlz=False, storeMuSigma=True)
    uvPred_unguarded(p_dense)
    res["no_d"] = {"uvpredsep_mu": p_sep.mu.tolist(), "uvpredsep_sigma": p_sep.sigma.tolist(),
                   "uvpred_mu": p_dense.mu.tolist(), "uvpred_sigma": p_dense.sigma.tolist()}
    # WITH_D: dense uvPred (uvPredSep cannot: SigObs is n x n against its 2n x 2n block).
    model = SepiaModel(make_data(True))
    complete_discrepancy(model)
    sd = samples_dict(NAMES_FULL)
    p_dense = SP.SepiaPrediction(x_pred=X_PRED, samples=sd, model=model, storeRlz=False, storeMuSigma=True)
    uvPred_unguarded(p_dense)
    try:
        SP.SepiaFullPrediction(x_pred=X_PRED, samples=sd, model=model, storeRlz=False, storeMuSigma=True)
        sep_status = "ran"
    except Exception as e:  # noqa: BLE001
        sep_status = f"{type(e).__name__}: {e}"
    res["with_d"] = {"uvpred_mu": p_dense.mu.tolist(), "uvpred_sigma": p_dense.sigma.tolist(),
                     "uvpredsep_status": sep_status}
    return res


# --- SEPIA's own MCMC on WITH_D (statistical comparison) -------------------
def mcmc_case(nsamp=12000, burn=2000, seed=1180):
    model = SepiaModel(make_data(True))
    complete_discrepancy(model)
    t0 = time.time()
    model.do_mcmc(nsamp, prog=False, seed=seed)
    secs = time.time() - t0
    s = model.get_samples(nburn=burn)
    out = {"nsamp": nsamp, "burn": burn, "seed": seed, "seconds": secs, "summary": {}}
    for k in ["theta", "betaV", "lamVz", "lamOs", "betaU", "lamUz", "lamWs", "lamWOs"]:
        a = np.asarray(s[k]).reshape(len(s[k]), -1)
        if k in ("betaV", "betaU"):
            a = np.exp(-0.25 * a)  # rho, the coordinate SEPIA's BetaRho step and prior act on
        nb = 40
        bl = a.shape[0] // nb
        bm = a[: nb * bl].reshape(nb, bl, -1).mean(axis=1)
        out["summary"][k if k not in ("betaV", "betaU") else k.replace("beta", "rho")] = {
            "mean": a.mean(axis=0).tolist(), "sd": a.std(axis=0, ddof=1).tolist(),
            "batch_means_se": (bm.std(axis=0, ddof=1) / np.sqrt(nb)).tolist()}
    return out


def main():
    os.makedirs(OUT, exist_ok=True)
    d = make_data(True)
    m0 = SepiaModel(make_data(True))
    ref = {
        "upstream": {"project": "SEPIA (LANL)", "repo": "https://github.com/lanl/SEPIA", "commit": commit(),
                     "licence": "BSD-3-Clause"},
        "versions": {"python": platform.python_version(), "numpy": np.__version__, "scipy": scipy.__version__},
        "problem": {"eta": "x (1 + 2 t1) + 0.5 sin(2 pi x + 3 t2)", "delta": "0.25 x^2", "noise_sd": 0.02,
                    "t_true": list(T_TRUE), "seed": 1180},
        "inputs": {"x_sim": x_sim.tolist(), "t_sim": t_sim.tolist(), "y_sim": y_sim[:, 0].tolist(),
                   "x_obs": x_obs.tolist(), "y_obs": y_obs[:, 0].tolist(), "x_pred": X_PRED.tolist()},
        "transforms": {"orig_y_mean": float(d.sim_data.orig_y_mean[0]), "orig_y_sd": float(d.sim_data.orig_y_sd),
                       "orig_x_min": d.sim_data.orig_x_min.tolist(), "orig_x_max": d.sim_data.orig_x_max.tolist(),
                       "orig_t_min": d.sim_data.orig_t_min.tolist(), "orig_t_max": d.sim_data.orig_t_max.tolist(),
                       "x_obs_trans": d.obs_data.x_trans.tolist(), "zt": m0.data.zt.tolist(),
                       "u": m0.num.u[:, 0].tolist(), "w": m0.num.w[:, 0].tolist(),
                       "sig_obs_diag": np.diag(m0.num.SigObs).tolist()},
        "log_lik": {"no_d": lik_cases("no_d"), "with_d": lik_cases("with_d"), "sim_only": lik_cases("sim_only")},
        "prediction": {"samples": SAMPLES, **pred_cases()},
    }
    ref["mcmc"] = mcmc_case()
    with open(os.path.join(OUT, "sepia_koh.json"), "w") as f:
        json.dump(ref, f, indent=1)
    print("wrote", os.path.join(OUT, "sepia_koh.json"), "mcmc", round(ref["mcmc"]["seconds"], 1), "s")


if __name__ == "__main__":
    main()
