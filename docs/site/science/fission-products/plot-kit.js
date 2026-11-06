// Fission products, chapters 4 and 5: the plot helpers the widgets share
// (log/linear plot boxes, ticks, legends, number and time formatting).
// Load after ../neutronics/lesson-kit.js; exposes window.LessonPlot.
(function () {
  "use strict";
  var L = window.Lesson, css = L.css;
  var YEAR = 3.15576e7;
  function fmtT(s) {
    if (!isFinite(s)) return "—";
    var u = [[1e6 * YEAR, "My"], [1e3 * YEAR, "ky"], [YEAR, "y"], [86400, "d"], [3600, "h"], [60, "min"], [1, "s"]];
    for (var i = 0; i < u.length; i++) if (s >= u[i][0]) return (s / u[i][0]).toPrecision(3) + " " + u[i][1];
    return s.toPrecision(3) + " s";
  }
  function sci(x, p) { return x === 0 ? "0" : (Math.abs(x) >= 1e-3 && Math.abs(x) < 1e4 ? x.toPrecision(p || 3) : x.toExponential((p || 3) - 1)); }
  function wrap(ctx, text, x, y, maxW, lh) {
    var words = String(text).split(" "), line = "";
    words.forEach(function (wd) {
      var t = line ? line + " " + wd : wd;
      if (ctx.measureText(t).width > maxW && line) { ctx.fillText(line, x, y); y += lh; line = wd; } else line = t;
    });
    if (line) { ctx.fillText(line, x, y); y += lh; }
    return y;
  }
  // A plot box: { x0, y0, x1, y1, lx: [lo, hi] (log10 if logx), ly: [...] }.
  function Box(x0, y0, x1, y1, xr, yr, logx, logy) {
    this.x0 = x0; this.y0 = y0; this.x1 = x1; this.y1 = y1; this.xr = xr; this.yr = yr; this.logx = logx; this.logy = logy;
  }
  Box.prototype.X = function (v) { var a = this.logx ? Math.log10(Math.max(v, 1e-300)) : v; return this.x0 + (a - this.xr[0]) / (this.xr[1] - this.xr[0]) * (this.x1 - this.x0); };
  Box.prototype.Y = function (v) { var a = this.logy ? Math.log10(Math.max(v, 1e-300)) : v; return this.y1 - (a - this.yr[0]) / (this.yr[1] - this.yr[0]) * (this.y1 - this.y0); };
  Box.prototype.frame = function (ctx, xlab, ylab, xt, yt) {
    var mut = css("--muted", "#5b636b"), self = this;
    ctx.strokeStyle = mut; ctx.lineWidth = 1; ctx.strokeRect(this.x0, this.y0, this.x1 - this.x0, this.y1 - this.y0);
    ctx.fillStyle = mut; ctx.font = "10px system-ui, sans-serif";
    (xt || []).forEach(function (t) { var x = self.X(t[0]); if (x < self.x0 - 1 || x > self.x1 + 1) return; ctx.fillText(t[1], x - ctx.measureText(t[1]).width / 2, self.y1 + 12); ctx.beginPath(); ctx.moveTo(x, self.y1); ctx.lineTo(x, self.y1 - 4); ctx.stroke(); });
    (yt || []).forEach(function (t) { var y = self.Y(t[0]); if (y < self.y0 - 1 || y > self.y1 + 1) return; ctx.fillText(t[1], self.x0 - 4 - ctx.measureText(t[1]).width, y + 3); ctx.beginPath(); ctx.moveTo(self.x0, y); ctx.lineTo(self.x0 + 4, y); ctx.stroke(); });
    ctx.font = "11px system-ui, sans-serif";
    if (xlab) ctx.fillText(xlab, (this.x0 + this.x1) / 2 - ctx.measureText(xlab).width / 2, this.y1 + 25);
    if (ylab) { ctx.save(); ctx.translate(this.x0 - 34, (this.y0 + this.y1) / 2); ctx.rotate(-Math.PI / 2); ctx.fillText(ylab, -ctx.measureText(ylab).width / 2, 0); ctx.restore(); }
  };
  Box.prototype.line = function (ctx, xs, ys, col, w, dash) {
    var self = this, started = false;
    ctx.strokeStyle = col; ctx.lineWidth = w || 2; ctx.setLineDash(dash || []);
    ctx.beginPath();
    xs.forEach(function (x, i) {
      var y = ys[i];
      if (y === null || y === undefined || !isFinite(y) || (self.logy && y <= 0)) { started = false; return; }
      var px = self.X(x), py = Math.max(self.y0, Math.min(self.y1, self.Y(y)));
      if (started) ctx.lineTo(px, py); else { ctx.moveTo(px, py); started = true; }
    });
    ctx.stroke(); ctx.setLineDash([]);
  };
  function decTicks(lo, hi, every) { var t = []; for (var e = Math.ceil(lo); e <= hi; e += (every || 1)) t.push([Math.pow(10, e), "1e" + e]); return t; }
  function linTicks(lo, hi, step, fmt) { var t = []; for (var v = Math.ceil(lo / step) * step; v <= hi + 1e-9; v += step) t.push([v, fmt ? fmt(v) : String(v)]); return t; }
  function legend(ctx, items, x, y, w) {
    ctx.font = "11px system-ui, sans-serif";
    var cx = x, cy = y;
    items.forEach(function (it) {
      var tw = ctx.measureText(it[0]).width + 26;
      if (cx + tw > x + w) { cx = x; cy += 15; }
      ctx.strokeStyle = it[1]; ctx.lineWidth = 2; ctx.setLineDash(it[2] || []);
      ctx.beginPath(); ctx.moveTo(cx, cy - 4); ctx.lineTo(cx + 18, cy - 4); ctx.stroke(); ctx.setLineDash([]);
      ctx.fillStyle = css("--fg", "#1d2125"); ctx.fillText(it[0], cx + 22, cy); cx += tw + 6;
    });
    return cy + 15;
  }
  window.LessonPlot = { YEAR: YEAR, fmtT: fmtT, sci: sci, wrap: wrap, Box: Box, decTicks: decTicks, linTicks: linTicks, legend: legend };
})();
