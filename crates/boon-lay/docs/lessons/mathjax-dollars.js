// Inline $...$ math for this deep dive (gh:#531): mdBook's MathJax 2
// recognises only \( \) inline by default. Same approach as the tutorials.
(function () {
  var cfg = { tex2jax: { inlineMath: [["$", "$"], ["\\(", "\\)"]], processEscapes: true } };
  if (window.MathJax && window.MathJax.Hub) {
    window.MathJax.Hub.Config(cfg);
    window.MathJax.Hub.Queue(["Typeset", window.MathJax.Hub]);
  } else {
    window.MathJax = cfg;
  }
})();
