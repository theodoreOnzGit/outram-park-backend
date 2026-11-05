# Double-pipe twin browser demo (gh:#1182): record

Rung 5 of epic gh:#1177. This is a browser demo of the high-fidelity to low-fidelity
twin of the laminar double-pipe exchanger. It runs in ~~`dhoby-ghaut`~~
`outram-park-digital-twin-engine` (**MOVED 2026-11-05** with this record, its
example and its page, (maintainer, 2026-11-05: "if web examples live in dhoby ghaut, and this causes the dependency cycle, let the web examples deploy from digital twin engine instead"))
(`examples/double_pipe_twin_web/`, `web/double_pipe_twin/`) and is published at
`demos/double-pipe-twin/` (unchanged).

**Research, education and V&V only.** This is an AI draft and has not been reviewed. It
validates nothing new: every calibrated number it shows comes from gh:#1180 / #1190 /
#1191, recomputed natively from their code, with the recomputation checked against
their records (section 3).

**Look at the pictures first** (`shots/`):

| | phone 390 × 844 | desktop 1280 × 800 |
|---|---|---|
| Twin, all layers | `phone_390x844_twin.png` | `desktop_1280x800_twin.png` |
| Twin, calibrated only | `phone_390x844_twin_calibrated.png` | `desktop_1280x800_twin_calibrated.png` |
| δ(x) map | `phone_390x844_delta.png` | `desktop_1280x800_delta.png` |
| Sweep (MFK ± 2σ) | `phone_390x844_sweep.png` | `desktop_1280x800_sweep.png` |
| Filtering (P-A vs P-B) | `phone_390x844_filter.png` | `desktop_1280x800_filter.png` |
| During Play | `phone_390x844_panel_open_playing.png` (panel opened while playing, after 4 zoom-ins) | `desktop_1280x800_playing_zoomed.png` |

## 1. What runs where

| Layer | Where | Source |
|---|---|---|
| 1-D UA, uncalibrated θ = (1, 1, 0) | **live**, Web Worker | twin engine `multifidelity::double_pipe::one_d::ua`: TUAS nodal conductances, march-free, bit-identical to the marched UA (pinned there) |
| 1-D UA at the posterior-mean θ, **no δ** | **live** | the same call, θ from `koh_theta_samples.csv` |
| θ-only band (2.5–97.5 % of the 1-D UA over the 200 thinned θ samples, no δ) | **live** | the same call, every sample at the operating point, every 4th at the 33 sweep points |
| KOH predictive η + δ (+ ε), median and 95 % | committed | `koh_mfk_grid.csv` |
| δ(x) on ln UA, mean and sd | committed | `koh_mfk_grid.csv` |
| MFK mean and σ (UA) | committed | `koh_mfk_grid.csv` |
| CFD design points (44) | committed (gh:#1178) | twin engine `dataset::load_committed` |
| UKF traces P-A, P-B | committed (gh:#1191) | twin engine `.../double_pipe_filtering_gh1191/traces.csv` |

One worker message evaluates one slice: 1918 TUAS evaluations. Natively that takes about
0.1 s for the default slice (`cargo test` runs the whole example suite in 0.15–0.2 s).
In headless Chromium the panel reports 53–103 ms per slice. The KOH (~12 min of TMCMC
here) and the MFK fit never run in the browser.

**Grid.** Re_hot × Re_cold = 9 × 9 geometric levels (`lf_grid`'s 5 levels, plus the
geometric midpoint between each pair). k_wall is {1, 10, 100} W/(m K) and L is
{0.10, 0.15, 0.20, 0.25, 0.30} m, for 1215 points. The grid contains every CFD
design point exactly (test `the_grid_holds_every_design_point`). Re_cold, k_wall and L
move on these levels. Re_hot is continuous for the live 1-D. The calibrated readout
uses the nearest Re_hot node, and the page says so.

## 2. Artefacts and provenance

Generated on 2026-11-04 at commit `eab3fa739` plus this change's generator:

```
cargo run --release -p dhoby-ghaut --example double_pipe_twin_web -- --generate-artefacts
```

(As run, from `dhoby-ghaut`. Since 2026-11-05 the same generator is
`cargo run --release -p outram-park-digital-twin-engine --example double_pipe_twin_web -- --generate-artefacts`;
not re-run since the move.)

The run took 1114 s: the MFK took 122 s, the KOH calibration 722 s and the grid
predictive 270 s. Its stdout is `generate_stdout_2026-11-04.txt` and its log is
`generate_log.txt`. The log's last line names the scratch directory the run wrote to.
The files were then copied here.

- **`koh_mfk_grid.csv`** (305 kB): one row per grid point, every number in Rust's
  shortest round-trip form. Its header comments carry the settings and the held-out
  check.
  - **KOH:** `raffles::fidelity::koh::KohCalibration::calibrate(TuasOneDUa,
    ua_dataset(44 rows), design = 44 x, koh_spec(WithDiscrepancy))`. This is
    exactly gh:#1180's test call. The predictive takes every 5th of the 2000 posterior
    samples (the stride of gh:#1180's held-out check) and includes the CFD error ε, as
    that check does. It is computed in chunks of 45 points, and only each point's
    marginal is kept.
  - **MFK:** `Surrogate::fit_multi(CoKriging(MfkConfig::smt_defaults()))` on
    `lf_grid.csv` (375 TUAS rows) plus the 36 CFD training rows, **UA only**.
    gh:#1190 fitted one MFK per output, so this is its UA fit; section 3 checks this.
  - **1-D:** `one_d::ua` at nominal θ and at the posterior mean.
- **`koh_theta_samples.csv`** (13 kB): every 10th of the 2000 posterior samples
  (ln θ_h, ln θ_c, r_f), plus the posterior mean that raffles reports
  (`ThetaEstimate::point`): θ_h 1.1862, θ_c 1.4200, r_f 5.43e-4 m² K/W.
- **`headless_default_slice.csv`**: the `--headless` trace at the default slice (H7's
  Re_cold 15, k_wall 10, L 0.2).

## 3. Does the recomputation reproduce the records? (checked, not assumed)

| Quantity | Record | Regenerated here |
|---|---|---|
| KOH held-out UA RMSE (gh:#1180) | 1.28 % | **1.28 %** |
| KOH 95 % coverage, held out | 8/8 | **8/8** |
| 1-D alone, uncalibrated, held-out RMSE | 8.95 % | **8.95 %** |
| 1-D at posterior-mean θ, no δ | 6.30 % | **6.30 %** |
| Posterior ln θ_h, ln θ_c, r_f (means) | +0.164, +0.336, 5.4e-4 | θ_h 1.186, θ_c 1.420 (posterior means of θ, not of ln θ), r_f 5.43e-4 |
| δ at the training x | −0.092 … +0.090 | over the whole grid: −0.0921 … +0.0909 |
| MFK held-out UA RMSE (gh:#1190) | 3.0 % | **3.01 %**, 2σ coverage 7/8 (record: "7–8 of 8") |
| Grid KOH median at the 8 held-out x, compared with the 8-point call | — | worst relative difference **0** |

These are pinned against the committed files by tests (section 5):
- the live 1-D in the engine equals the committed `ua_1d_*` columns **bit for bit**, at 9
  nodes on 3 slices;
- the committed trace shows gh:#1191's P-B numbers: r_f bias −4.4e-4 m² K/W, NIS
  about as normal as P-A's, NEES above 100.

## 4. What the picture shows (read off the committed grid; not new validation)

- **The calibrated twin moves the 1-D by −0.8 % … +25.4 %** of the uncalibrated UA
  (KOH median / 1-D − 1 over the grid). Its 95 % interval is 4.1–6.5 % of the median
  wide.
- **The posterior-mean θ without δ over-corrects**: it lies −7.6 % … +12.6 % from the
  KOH median. At the default point it is 1.273 W/K, against KOH 1.196 [1.169, 1.223]
  and the CFD's H7 1.192 W/K. Its θ-only band [1.187, 1.337] does not reach down to
  the CFD. The twin view draws this dashed, labelled "no δ".
- **δ(x)** is resolved from zero (|δ| > 2 sd) at **282 of 1215** grid points. At the
  default slice (k_wall 10, L 0.2) δ is most negative at high Re_cold (about −0.08 on
  ln UA at Re_cold 34–45), and it is positive at low Re_cold with high Re_hot (up to
  +0.06). By k_wall level, δ runs −0.073 … +0.056 at 1, −0.084 … +0.076 at 10 and
  −0.092 … +0.091 at 100 W/(m K).
  - **Caveat (gh:#1180):** θ is only partly identified, and δ absorbs part of θ. The map
    is the discrepancy *given this calibration*, not the 1-D's physics error on its own.
  - The largest uncalibrated error is TUAS's textbook annulus Nu, because no annulus
    correlation exists (gh:#1195).
- **MFK against KOH:** the MFK mean lies −4.7 % … +5.6 % from the KOH median. Its ±2σ
  band is 0–69 % of the mean wide. It is narrow at the CFD points (σ → nugget) and wide
  away from them, in k_wall and L, where only the TUAS level informs it.

## 5. Checks run

**Tests** (`cargo test --release -p dhoby-ghaut --example double_pipe_twin_web -j 2`; since 2026-11-05 `-p outram-park-digital-twin-engine`):
11 passed.
- `artefacts`: the grid holds every design point; the committed files parse and are
  ordered (harness bounds); P-B is confidently wrong in the committed trace.
- `engine`: the wire format; **the live 1-D is the committed one** (bit-identical);
  **the headless regression test** (byte-identical twice, UA positive and rising with
  Re_hot, the band ordered: a harness check, not V&V).
- `app`: the toggles and query, Play's schedule, `Pending` (one request in flight, the
  newest input wins, stale answers ignored), the frame and tick maths, the δ colours,
  and the readout naming each number's source.
- `generate`: the UA-only MFK dataset's shape, the RMSE definition, and the θ writer
  read back through the parser.

GUI drawing and `main` glue are exempt from the reaching-test rule; each has a
one-line reason in its doc comment. `generate::run` (20 min) is the gh:#1180 and
gh:#1190 calls, which their own tests run. It is not repeated in a test here: its
output is checked in section 3, and the committed result is pinned by the tests above.

**Mobile (HARD RULE), checked at 390 × 844 with touch** (`browser_check.js`, headless
Chromium with SwiftShader, `shots/`):
- the main view fills the page;
- + / − / Reset sit top right;
- "Controls »" reopens the panel, which is folded by default on the phone and open on
  the desktop;
- the view tabs and the twin toggle are 36–38 px finger-sized buttons on the main view
  and are repeated in the panel;
- status ("evaluating…", errors) is drawn on the main view;
- when the open panel leaves the main view under ~300 px, the tabs and readout give way
  to the plot.

**No lag (HARD RULE).** Play keeps the worker busy continuously, with a new slice on
every answer. During it the page took four "+" taps with plot drags and opened the
panel. Every input was taken: the round trips, including Playwright's own stepping,
were 493–624 ms on the phone and 363–423 ms on the desktop.

Frame intervals while playing had a median of 66.6 ms on the phone and 50–66.6 ms on
the desktop. **The baseline `?spin`** redraws every frame with the worker idle, and its
median is **50 ms**. So the frame time is SwiftShader's software rendering of a
continuously redrawn page, and the worker adds at most about one vsync step (16.7 ms,
from CPU contention on this 4-core host). The UI thread itself does no model work.
Real-GPU frame times were not measured. Report: `shots/browser_report.json`.

**Builds:** the native example and its tests, and `--target wasm32-unknown-unknown`
(release), both build clean of warnings from this demo. `scripts/build-pages.sh` builds
it into `demos/double-pipe-twin/` and checks its four files; the demos page links it
under Thermal hydraulics.

## 6. Not done / limits

- **KOH for Q, ε, outlet T and Δp:** gh:#1180 calibrated UA only, so the twin shows UA
  only. Showing a "calibrated duty" would mean deriving one, and nothing here derives
  one.
- **Live KOH at an arbitrary Re_hot:** the KOH predictive is shown at the 9 committed
  nodes, joined by straight lines. Porting the SEPIA predictor to the worker
  (`raffles::bayesian::koh` compiles for wasm32) is possible, but it would cost the
  joint predictive over 400 posterior samples per point.
- **A real-GPU frame-time measurement**, and a real phone.
- The Foam Studio link opens the live CFD of the stage-2 exchanger (`?shape=exchanger`),
  not a design point of this grid.
