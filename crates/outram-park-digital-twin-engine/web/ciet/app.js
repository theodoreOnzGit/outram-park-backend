// The CIET demo page (gh:#924). This thread only draws and handles input
// (the workspace's no-lag rule): the plant steps in plant-worker.js, and in
// three-worker mode its flow solves run in two flow-worker.js workers, as the
// desktop simulator v2 runs them on threads. The page starts the three
// workers and connects them with MessageChannels (no nested workers, which
// older Safari lacks), sends controls, and draws each frame the plant worker
// streams.
//
// Mobile-first (docs/claude-md/mobile-first-tutorials-and-demos.md): a
// full-page main view with zoom in / zoom out / reset buttons, gestures as a
// supplement (drag, pinch, wheel), and a side panel holding every control,
// folded by default under 700 px.

const $ = (id) => document.getElementById(id);
const canvas = $("view");
const ctx = canvas.getContext("2d");

// ── Workers ────────────────────────────────────────────────────────────────
let plantWorker = null;
let flowWorkers = [];
let fields = [];
let ix = {};
let last = null; // { readout, stats }
let mode = "three";
let speed = 1;
let paused = false;
let failed = null;

const CONTROLS = ["heater_kw", "pump_pa", "ctah_setpoint", "tchx_setpoint", "ctah_blocked", "dhx_blocked", "coarse_heater"];

function controlValue(name) {
  const el = $(name);
  return el.type === "checkbox" ? (el.checked ? 1 : 0) : Number(el.value);
}

function startWorkers(initialControls) {
  for (const w of [plantWorker, ...flowWorkers]) if (w) w.terminate();
  failed = null;
  last = null;
  plantWorker = new Worker("./plant-worker.js", { type: "module" });
  const dracs = new MessageChannel();
  const primary = new MessageChannel();
  flowWorkers = [
    ["dracs", dracs],
    ["primary", primary],
  ].map(([kind, ch]) => {
    const w = new Worker("./flow-worker.js", { type: "module" });
    w.postMessage({ type: "init", kind, port: ch.port2 }, [ch.port2]);
    return w;
  });
  plantWorker.onmessage = (ev) => {
    const m = ev.data;
    if (m.type === "ready") {
      fields = m.fields;
      ix = Object.fromEntries(fields.map((f, i) => [f, i]));
      $("loading").style.display = "none";
    } else if (m.type === "frame") {
      last = { readout: m.readout, stats: m.stats };
      updatePanel();
      requestDraw();
    } else if (m.type === "error") {
      failed = m.message;
      requestDraw();
      updatePanel();
    }
  };
  plantWorker.onerror = (e) => {
    failed = e.message || "the plant worker stopped";
    $("loading").innerHTML = '<p class="err">Could not start: ' + failed + "</p>";
  };
  plantWorker.postMessage(
    { type: "init", mode, speed, controls: initialControls, dracsPort: dracs.port1, primaryPort: primary.port1 },
    [dracs.port1, primary.port1],
  );
}

function send(m) {
  if (plantWorker) plantWorker.postMessage(m);
}

// ── Panel ──────────────────────────────────────────────────────────────────
const fmt = {
  heater_kw: (v) => v.toFixed(1) + " kW",
  pump_pa: (v) => (v / 1000).toFixed(1) + " kPa",
  ctah_setpoint: (v) => v.toFixed(1) + " °C",
  tchx_setpoint: (v) => v.toFixed(1) + " °C",
};

const touched = new Set();
for (const name of CONTROLS) {
  const el = $(name);
  const ev = el.type === "checkbox" ? "change" : "input";
  el.addEventListener(ev, () => {
    touched.add(name);
    send({ type: "set", name, value: controlValue(name) });
    if (fmt[name]) $(name + "_v").textContent = fmt[name](Number(el.value));
  });
}

function markButtons() {
  for (const b of document.querySelectorAll("#speeds button")) b.classList.toggle("on", Number(b.dataset.speed) === speed);
  for (const b of document.querySelectorAll("#modes button")) b.classList.toggle("on", b.dataset.mode === mode);
  $("pause").textContent = paused ? "Resume" : "Pause";
  $("pause").classList.toggle("on", paused);
}
for (const b of document.querySelectorAll("#speeds button"))
  b.addEventListener("click", () => {
    speed = Number(b.dataset.speed);
    send({ type: "speed", value: speed });
    markButtons();
  });
for (const b of document.querySelectorAll("#modes button"))
  b.addEventListener("click", () => {
    mode = b.dataset.mode;
    send({ type: "mode", mode });
    markButtons();
  });
$("pause").addEventListener("click", () => {
  paused = !paused;
  send({ type: "pause", on: paused });
  markButtons();
});
$("reset").addEventListener("click", () => {
  // A fresh plant at the desktop's starting state, with the controls the
  // reader has set (an untouched set point stays the desktop's 21 degC).
  paused = false;
  markButtons();
  startWorkers([...touched].map((name) => ({ name, value: controlValue(name) })));
});

const panel = $("panel");
function setFolded(f) {
  panel.classList.toggle("folded", f);
  $("reopen").classList.toggle("shown", f);
  resize();
}
$("hide").addEventListener("click", () => setFolded(true));
$("reopen").addEventListener("click", () => setFolded(false));

function rowsHtml(rows) {
  return rows.map(([k, v]) => `<tr><td>${k}</td><td>${v}</td></tr>`).join("");
}

function val(name) {
  return last ? last.readout[ix[name]] : NaN;
}

function updatePanel() {
  if (!last) return;
  const s = last.stats;
  const ms = (x) => (x > 0 ? x.toFixed(2) + " ms" : "–");
  const perfRows = [
    ["Workers", s.mode === "three" ? "3 (plant, DRACS flow, primary flow)" : "1"],
    ["Compute per step (plant worker, incl. waiting)", ms(s.stepMs)],
    ["… slowest step in the last 3 s", ms(s.maxStepMs)],
    ["… heat transfer" + (s.mode === "one" ? " and flows" : ""), ms(s.heatMs)],
  ];
  if (s.mode === "three") perfRows.push(["… DRACS flow (its worker)", ms(s.dracsMs)], ["… primary flows (its worker)", ms(s.primaryMs)]);
  perfRows.push(
    ["Real-time budget per step", (val("timestep_s") * 1000).toFixed(0) + " ms"],
    ["Could run at", s.capacity > 0 ? s.capacity.toFixed(1) + "× real time" : "–"],
    ["Running at", s.pace.toFixed(2) + "× real time"],
    ["Behind the clock by", speed > 0 ? Math.max(0, s.behind).toFixed(2) + " s" : "–"],
  );
  $("perf").innerHTML = rowsHtml(perfRows);
  const c = (n) => val(n).toFixed(2) + " °C";
  const kg = (n) => val(n).toFixed(4) + " kg/s";
  $("readout").innerHTML = rowsHtml([
    ["Simulated time", val("time_s").toFixed(1) + " s"],
    ["Heater power", val("heater_kw").toFixed(2) + " kW"],
    ["Heater in BT-11 / out BT-12", c("bt_11_heater_inlet") + " / " + c("bt_12_heater_outlet")],
    ["DHX shell in BT-21 / out BT-27", c("bt_21_dhx_shell_inlet") + " / " + c("bt_27_dhx_shell_outlet")],
    ["DHX tube in BT-60 / out BT-21", c("bt_60_dhx_tube_inlet") + " / " + c("bt_21_dhx_tube_outlet")],
    ["TCHX in BT-65 / out BT-66", c("bt_65_tchx_inlet") + " / " + c("bt_66_tchx_outlet")],
    ["CTAH in BT-43 / out BT-41", c("bt_43_ctah_inlet") + " / " + c("bt_41_ctah_outlet")],
    ["Heater branch flow", kg("flow_heater_branch")],
    ["DHX branch flow (FM-20)", kg("flow_dhx_branch")],
    ["CTAH branch flow (FM-40)", kg("flow_ctah_branch")],
    ["DRACS flow (FM-60)", kg("flow_dracs")],
    ["CTAH air-side h", val("ctah_htc").toFixed(1) + " W/(m² K)"],
    ["TCHX air-side h", val("tchx_htc").toFixed(1) + " W/(m² K)"],
  ]);
  for (const name of ["ctah_setpoint", "tchx_setpoint"]) $(name + "_v").textContent = fmt[name](val(name));
  $("heater_kw_v").textContent = fmt.heater_kw(val("heater_kw"));
  $("pump_pa_v").textContent = fmt.pump_pa(val("pump_pa"));
  const st = $("status");
  if (failed) {
    st.innerHTML = `<span class="err big">The solver stopped: ${failed}. Press Reset.</span>`;
    return;
  }
  const keeps = s.capacity >= 1;
  st.innerHTML =
    `<span class="big">t = ${val("time_s").toFixed(1)} s</span> · ` +
    (s.paused ? "paused" : speed === 0 ? `as fast as possible: ${s.pace.toFixed(1)}× real time` : `target ${speed}× · running ${s.pace.toFixed(2)}×`) +
    `<br><span class="${keeps ? "rt-yes" : "rt-no"}">${s.stepMs.toFixed(1)} ms per 100 ms step: ` +
    (keeps ? `real time reachable (${s.capacity.toFixed(1)}×)` : `slower than real time (${s.capacity.toFixed(2)}×)`) +
    `</span> · ${s.mode === "three" ? "3 workers" : "1 worker"}`;
}

// ── View: world (m-ish drawing units, y up) to screen ──────────────────────
const WORLD = { x0: -14.6, x1: 11.8, y0: -1.2, y1: 12.6 };
const view = { s: 1, ox: 0, oy: 0, userMoved: false };
let dpr = 1;

function fit() {
  const w = canvas.width, h = canvas.height;
  const top = 58 * dpr; // the status lines
  const sx = w / (WORLD.x1 - WORLD.x0), sy = (h - top) / (WORLD.y1 - WORLD.y0);
  view.s = Math.min(sx, sy) * 0.94;
  view.ox = w / 2 - view.s * (WORLD.x0 + WORLD.x1) / 2;
  view.oy = top + (h - top) / 2 + view.s * (WORLD.y0 + WORLD.y1) / 2;
  view.userMoved = false;
}
const X = (x) => view.ox + view.s * x;
const Y = (y) => view.oy - view.s * y;

function resize() {
  dpr = window.devicePixelRatio || 1;
  const r = canvas.getBoundingClientRect();
  const w = Math.max(1, Math.round(r.width * dpr)), h = Math.max(1, Math.round(r.height * dpr));
  if (w !== canvas.width || h !== canvas.height) {
    canvas.width = w;
    canvas.height = h;
    if (!view.userMoved) fit();
  }
  requestDraw();
}
new ResizeObserver(resize).observe(canvas);

function zoomAbout(f, px, py) {
  view.ox = px - (px - view.ox) * f;
  view.oy = py - (py - view.oy) * f;
  view.s *= f;
  view.userMoved = true;
  requestDraw();
}
$("zin").addEventListener("click", () => zoomAbout(1.3, canvas.width / 2, canvas.height / 2));
$("zout").addEventListener("click", () => zoomAbout(1 / 1.3, canvas.width / 2, canvas.height / 2));
$("zfit").addEventListener("click", () => { fit(); requestDraw(); });

canvas.addEventListener("wheel", (e) => {
  e.preventDefault();
  zoomAbout(Math.exp(-e.deltaY * 0.0015), e.offsetX * dpr, e.offsetY * dpr);
}, { passive: false });

const pointers = new Map();
let pinch = null;
canvas.addEventListener("pointerdown", (e) => {
  canvas.setPointerCapture(e.pointerId);
  pointers.set(e.pointerId, { x: e.offsetX * dpr, y: e.offsetY * dpr });
  pinch = null;
});
canvas.addEventListener("pointermove", (e) => {
  const p = pointers.get(e.pointerId);
  if (!p) return;
  const q = { x: e.offsetX * dpr, y: e.offsetY * dpr };
  if (pointers.size === 1) {
    view.ox += q.x - p.x;
    view.oy += q.y - p.y;
    view.userMoved = true;
    requestDraw();
  } else if (pointers.size === 2) {
    const other = [...pointers.entries()].find(([id]) => id !== e.pointerId)[1];
    const d = Math.hypot(q.x - other.x, q.y - other.y);
    if (pinch) zoomAbout(d / pinch, (q.x + other.x) / 2, (q.y + other.y) / 2);
    pinch = d;
  }
  pointers.set(e.pointerId, q);
});
const up = (e) => { pointers.delete(e.pointerId); pinch = null; };
canvas.addEventListener("pointerup", up);
canvas.addEventListener("pointercancel", up);

// ── The loop's drawing ─────────────────────────────────────────────────────
// Topology from the v2 stepper's branch lists (heater, DHX and CTAH branches
// between the top and bottom mixing nodes; the DRACS loop through the DHX tubes
// and the TCHX). Each branch's segments are listed in the direction TUAS
// counts its flow positive: top node to bottom node for the three primary
// branches, and for the DRACS the natural-circulation direction the model
// assumes. Positions are a schematic, not plant geometry.
const BR = {
  heater: [
    ["pipe_4", [[0, 10], [0, 9.2]], "4"], ["pipe_3", [[0, 9.2], [0, 8.4]], "3"], ["pipe_2a", [[0, 8.4], [0, 7.8]], "2a"],
    ["pipe_2", [[0, 7.8], [0, 7.2]], "2"], ["pipe_1a", [[0, 7.2], [0, 6.6]], "1a"], ["@heater", [[0, 6.6], [0, 3.0]], ""],
    ["pipe_1b", [[0, 3.0], [0, 2.2]], "1b"], ["pipe_18", [[0, 2.2], [0, 0]], "18"],
  ],
  dhx: [
    ["pipe_5a", [[0, 10], [-3, 10]], "5a"], ["pipe_26", [[-3, 10], [-6, 10]], "26"], ["pipe_25a", [[-6, 10], [-6, 9.2]], "25a"],
    ["pipe_25", [[-6, 9.2], [-6, 8.6]], "25"], ["@dhx_shell", [[-6, 8.6], [-6, 5.0]], ""], ["pipe_23", [[-6, 5.0], [-6, 4.4]], "23"],
    ["pipe_23a", [[-6, 4.4], [-6, 3.8]], "23a"], ["pipe_22", [[-6, 3.8], [-6, 3.2]], "22"], ["pipe_21a", [[-6, 3.2], [-6, 2.6]], "21a"],
    ["pipe_21", [[-6, 2.6], [-6, 2.0]], "21"], ["pipe_20", [[-6, 2.0], [-6, 1.0]], "20"], ["pipe_19", [[-6, 1.0], [-6, 0]], "19"],
    ["pipe_17b", [[-6, 0], [0, 0]], "17b"],
  ],
  ctah: [
    ["pipe_5b", [[0, 10], [4, 10]], "5b"], ["pipe_6", [[4, 10], [6, 10]], "6"], ["pipe_6a", [[6, 10], [8, 10]], "6a"],
    ["@ctah", [[8, 10], [8, 6.8]], ""], ["pipe_8a", [[8, 6.8], [8, 6.2]], "8a"], ["pipe_8", [[8, 6.2], [8, 5.6]], "8"],
    ["pipe_9", [[8, 5.6], [8, 5.0]], "9"], ["pipe_10", [[8, 5.0], [8, 4.4]], "10"], ["pipe_11", [[8, 4.4], [8, 3.8]], "11"],
    ["pipe_12", [[8, 3.8], [8, 3.3]], "12"], ["pipe_ctah_pump", [[8, 3.3], [8, 2.3]], ""], ["pipe_13", [[8, 2.3], [8, 1.8]], "13"],
    ["pipe_14", [[8, 1.8], [8, 1.2]], "14"], ["pipe_14a", [[8, 1.2], [8, 0.6]], "14a"], ["pipe_15", [[8, 0.6], [8, 0]], "15"],
    ["pipe_16", [[8, 0], [5, 0]], "16"], ["pipe_17a", [[5, 0], [0, 0]], "17a"],
  ],
  dracs: [
    ["@dhx_tube", [[-6.6, 5.0], [-6.6, 8.6]], ""], ["pipe_30b", [[-6.6, 8.6], [-6.6, 9.6]], "30b"],
    ["pipe_31", [[-6.6, 9.6], [-6.6, 11.5]], "31"], ["pipe_31a", [[-6.6, 11.5], [-8, 11.5]], "31a"],
    ["pipe_32", [[-8, 11.5], [-10, 11.5]], "32"], ["pipe_33", [[-10, 11.5], [-12, 11.5]], "33"],
    ["pipe_34", [[-12, 11.5], [-12, 10.5]], "34"], ["@tchx", [[-12, 10.5], [-12, 7.5]], ""],
    ["pipe_36", [[-12, 7.5], [-12, 6.8]], "36"], ["pipe_36a", [[-12, 6.8], [-12, 6.0]], "36a"],
    ["pipe_37", [[-12, 6.0], [-12, 5.0]], "37"], ["pipe_37a", [[-12, 5.0], [-12, 4.2]], "37a"],
    ["pipe_38", [[-12, 4.2], [-12, 3.4]], "38"], ["pipe_39", [[-12, 3.4], [-12, 2.6], [-6.6, 2.6]], "39"],
    ["pipe_30a", [[-6.6, 2.6], [-6.6, 5.0]], "30a"],
  ],
};
const FLOW = { heater: "flow_heater_branch", dhx: "flow_dhx_branch", ctah: "flow_ctah_branch", dracs: "flow_dracs" };

// v2's colour map (useful_functions.rs `hot_to_cold_colour`), 20 to 100 degC.
function colour(tc) {
  let h = (tc - 20) / 80;
  if (!(h >= 0)) h = 0;
  if (h > 1) h = 1;
  return `rgb(${Math.floor(255 * h)},${Math.floor(135 * (1 - h))},${Math.floor(255 * (1 - h))})`;
}

let drawQueued = false;
function requestDraw() {
  if (!drawQueued) {
    drawQueued = true;
    requestAnimationFrame(draw);
  }
}

function pipe(pts, temp, wWorld) {
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  ctx.beginPath();
  pts.forEach(([x, y], i) => (i ? ctx.lineTo(X(x), Y(y)) : ctx.moveTo(X(x), Y(y))));
  ctx.strokeStyle = "#05070a";
  ctx.lineWidth = Math.max(4 * dpr, wWorld * view.s) + 2 * dpr;
  ctx.stroke();
  ctx.strokeStyle = colour(temp);
  ctx.lineWidth = Math.max(4 * dpr, wWorld * view.s);
  ctx.stroke();
}

// A vertical box from (x, y0) to (x, y1) shaded from tStart at y0 to tEnd at y1.
function box(x, y0, y1, half, tStart, tEnd, label) {
  const g = ctx.createLinearGradient(0, Y(y0), 0, Y(y1));
  g.addColorStop(0, colour(tStart));
  g.addColorStop(1, colour(tEnd));
  const left = X(x - half), top = Y(Math.max(y0, y1));
  const w = 2 * half * view.s, h = Math.abs(y1 - y0) * view.s;
  ctx.fillStyle = g;
  ctx.fillRect(left, top, w, h);
  ctx.strokeStyle = "#c9d1de";
  ctx.lineWidth = 1.5 * dpr;
  ctx.strokeRect(left, top, w, h);
  if (label) text(label, x, Math.max(y0, y1) + 0.15, "center", "bottom", 13, "#e8ecf3", true);
}

function text(t, x, y, align, base, px, color, bold) {
  ctx.font = `${bold ? "600 " : ""}${px * dpr}px system-ui, sans-serif`;
  ctx.textAlign = align;
  ctx.textBaseline = base;
  ctx.fillStyle = color;
  ctx.fillText(t, X(x), Y(y));
}

// Chevrons along a segment, pointing along the flow (sign of the branch flow).
function arrows(pts, sign) {
  if (sign === 0) return;
  ctx.fillStyle = "rgba(255,255,255,0.85)";
  for (let i = 0; i + 1 < pts.length; i++) {
    let [ax, ay] = pts[i], [bx, by] = pts[i + 1];
    if (sign < 0) [ax, ay, bx, by] = [bx, by, ax, ay];
    const L = Math.hypot(bx - ax, by - ay);
    if (L * view.s < 40 * dpr) continue;
    const ux = (bx - ax) / L, uy = (by - ay) / L;
    const mx = X((ax + bx) / 2), my = Y((ay + by) / 2);
    const a = 6 * dpr;
    const dx = ux, dy = -uy; // screen direction
    ctx.beginPath();
    ctx.moveTo(mx + dx * a, my + dy * a);
    ctx.lineTo(mx - dx * a - dy * a, my - dy * a + dx * a);
    ctx.lineTo(mx - dx * a + dy * a, my - dy * a - dx * a);
    ctx.closePath();
    ctx.fill();
  }
}

// On-drawing labels need room: about 34 CSS px per drawing unit. Below that
// (a phone, fitted) the drawing keeps the equipment names and the numbers go
// to a card under it; zooming in brings them back.
function detailed() {
  return view.s >= 34 * dpr;
}

function updateCard() {
  const card = $("card");
  const show = !!last && !detailed();
  card.classList.toggle("shown", show);
  if (!show) return;
  const v = (n) => last.readout[ix[n]];
  const c = (n) => v(n).toFixed(1);
  const kg = (n) => v(n).toFixed(4);
  card.innerHTML =
    `<b>Heater</b> ${v("heater_kw").toFixed(1)} kW, ${c("bt_11_heater_inlet")} → ${c("bt_12_heater_outlet")} °C · ` +
    `<b>DHX</b> shell ${c("bt_21_dhx_shell_inlet")} → ${c("bt_27_dhx_shell_outlet")} °C · ` +
    `<b>CTAH</b> ${c("bt_43_ctah_inlet")} → ${c("bt_41_ctah_outlet")} °C · ` +
    `<b>TCHX</b> ${c("bt_65_tchx_inlet")} → ${c("bt_66_tchx_outlet")} °C<br>` +
    `<b>Flows</b> (kg/s): heater ${kg("flow_heater_branch")}, DHX ${kg("flow_dhx_branch")}, CTAH ${kg("flow_ctah_branch")}, DRACS ${kg("flow_dracs")}`;
}

function draw() {
  drawQueued = false;
  updateCard();
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.fillStyle = "#0e1014";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  if (!last) return;
  const v = (n) => last.readout[ix[n]];
  const W = 0.22;
  for (const [br, segs] of Object.entries(BR)) {
    const flow = v(FLOW[br]);
    const sign = Math.abs(flow) < 1e-6 ? 0 : Math.sign(flow);
    for (const [field, pts, label] of segs) {
      if (field.startsWith("@")) continue;
      const blocked = (br === "ctah" && v("ctah_blocked")) || (br === "dhx" && v("dhx_blocked"));
      pipe(pts, v(field), W);
      if (blocked && field === (br === "ctah" ? "pipe_5b" : "pipe_5a")) {
        const [[ax, ay], [bx, by]] = pts;
        text("✕ valve shut", (ax + bx) / 2, ay + 0.35, "center", "bottom", 12, "#ffb36b", true);
      }
      arrows(pts, sign);
      if (label && detailed()) {
        const [[ax, ay], [bx, by]] = [pts[0], pts[pts.length - 1]];
        const vertical = Math.abs(ax - bx) < 1e-9;
        const left = vertical && br === "dracs"; // the DRACS labels go outside its loop
        text(label, (ax + bx) / 2 + (vertical ? (left ? -0.3 : 0.3) : 0), (ay + by) / 2 + (vertical ? 0 : 0.28), vertical ? (left ? "right" : "left") : "center", "middle", 11, "#8a93a3");
      }
    }
  }
  // Equipment: shaded from inlet to outlet thermocouple, at the sensors' ends.
  box(0, 3.0, 6.6, 0.45, v("bt_11_heater_inlet"), v("bt_12_heater_outlet"), "");
  text("Heater", -0.65, 5.0, "right", "bottom", 13, "#e8ecf3", true);
  text(v("heater_kw").toFixed(1) + " kW", -0.65, 5.0, "right", "top", 13, "#ffcf8a", true);
  box(-6, 8.6, 5.0, 0.8, v("bt_21_dhx_shell_inlet"), v("bt_27_dhx_shell_outlet"), "DHX");
  box(-6.6, 5.0, 8.6, 0.14, v("bt_60_dhx_tube_inlet"), v("bt_21_dhx_tube_outlet"), "");
  box(8, 10, 6.8, 0.45, v("bt_43_ctah_inlet"), v("bt_41_ctah_outlet"), "");
  text("CTAH", 8.55, 8.4, "left", "middle", 13, "#e8ecf3", true);
  box(-12, 10.5, 7.5, 0.45, v("bt_65_tchx_inlet"), v("bt_66_tchx_outlet"), "");
  text("TCHX", -12.55, 9.0, "right", "middle", 13, "#e8ecf3", true);
  arrows([[0, 6.6], [0, 3.0]], Math.sign(v("flow_heater_branch")) * (Math.abs(v("flow_heater_branch")) < 1e-6 ? 0 : 1));
  // pump
  ctx.beginPath();
  ctx.arc(X(8), Y(2.8), 0.42 * view.s, 0, 2 * Math.PI);
  ctx.fillStyle = colour(v("pipe_ctah_pump"));
  ctx.fill();
  ctx.strokeStyle = "#c9d1de";
  ctx.lineWidth = 1.5 * dpr;
  ctx.stroke();
  // mixing nodes
  for (const [n, y] of [["node_top", 10], ["node_bottom", 0]]) {
    ctx.beginPath();
    ctx.arc(X(0), Y(y), 0.2 * view.s + 2 * dpr, 0, 2 * Math.PI);
    ctx.fillStyle = colour(v(n));
    ctx.fill();
    ctx.stroke();
  }
  // thermocouples and flowmeters (only with room for them; else the card)
  if (detailed()) drawMeasurements(v);
  drawLegend();
  if (failed) {
    ctx.fillStyle = "rgba(0,0,0,0.55)";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
  }
}

function drawMeasurements(v) {
  const t1 = (x) => x.toFixed(1) + " °C";
  const small = 12, tc = "#e8ecf3";
  text("BT-12 " + t1(v("bt_12_heater_outlet")), 0.6, 6.4, "left", "middle", small, tc);
  text("BT-11 " + t1(v("bt_11_heater_inlet")), 0.6, 3.2, "left", "middle", small, tc);
  text("BT-21 " + t1(v("bt_21_dhx_shell_inlet")), -5.0, 8.4, "left", "middle", small, tc);
  text("BT-27 " + t1(v("bt_27_dhx_shell_outlet")), -5.0, 5.2, "left", "middle", small, tc);
  text("BT-43 " + t1(v("bt_43_ctah_inlet")), 7.4, 9.7, "right", "middle", small, tc);
  text("BT-41 " + t1(v("bt_41_ctah_outlet")), 7.4, 7.0, "right", "middle", small, tc);
  text("BT-65 " + t1(v("bt_65_tchx_inlet")), -11.4, 10.3, "left", "middle", small, tc);
  text("BT-66 " + t1(v("bt_66_tchx_outlet")), -11.4, 7.7, "left", "middle", small, tc);
  const kg = (x) => x.toFixed(4) + " kg/s";
  text("FM-20 " + kg(v("flow_dhx_branch")), -4.9, 2.9, "left", "middle", small, "#bfe3ff");
  text("FM-40 " + kg(v("flow_ctah_branch")), 7.4, 0.9, "right", "middle", small, "#bfe3ff");
  text("FM-60 " + kg(v("flow_dracs")), -11.6, 4.6, "left", "middle", small, "#bfe3ff");
  text("heater branch " + kg(v("flow_heater_branch")), 0.6, 1.2, "left", "middle", small, "#bfe3ff");
  text("DRACS", -9, 11.85, "center", "bottom", 13, "#e8ecf3", true);
  text("primary loop", 4, 10.35, "center", "bottom", 13, "#e8ecf3", true);
  text(`pump ${(v("pump_pa") / 1000).toFixed(1)} kPa`, 8.6, 2.8, "left", "middle", 12, "#e8ecf3");
}

function drawLegend() {
  const lw = 140 * dpr, lh = 10 * dpr, lx = canvas.width - lw - 12 * dpr, ly = canvas.height - 28 * dpr;
  const g = ctx.createLinearGradient(lx, 0, lx + lw, 0);
  for (let i = 0; i <= 8; i++) g.addColorStop(i / 8, colour(20 + 10 * i));
  ctx.fillStyle = g;
  ctx.fillRect(lx, ly, lw, lh);
  ctx.font = `${11 * dpr}px system-ui, sans-serif`;
  ctx.fillStyle = "#c9d1de";
  ctx.textBaseline = "top";
  ctx.textAlign = "left";
  ctx.fillText("20 °C", lx, ly + lh + 2 * dpr);
  ctx.textAlign = "right";
  ctx.fillText("100 °C", lx + lw, ly + lh + 2 * dpr);
}

// ── Start ──────────────────────────────────────────────────────────────────
for (const name of ["heater_kw", "pump_pa"]) $(name + "_v").textContent = fmt[name](Number($(name).value));
setFolded(window.innerWidth < 700);
markButtons();
// The starting plant is the desktop's (heater off, pump off, set points 21
// degC); controls the reader has not touched are not sent.
startWorkers([]);
