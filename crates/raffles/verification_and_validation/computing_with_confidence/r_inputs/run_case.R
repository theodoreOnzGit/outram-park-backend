# Non-interactive driver for Adolphus Lye's Computing-with-Confidence R scripts
# (https://github.com/Adolphus8/Computing-with-Confidence, folder
# "Coherent systems").
#
# Usage:  Rscript run_case.R <workdir> "<script file name>"
#
# <workdir> holds an untouched clone of the upstream folder (pba BETTER.r and
# the case-study .r files; NOT committed here).  The driver changes NO
# computation: it only supplies stand-ins for the interactive graphics event
# that the scripts' wait() helper calls (getGraphicsEvent, keyPressed), opens a
# pdf() device so plotting works headless, evaluates the script exactly as
# written, then prints every `interval_*` object the script computed.
# The scripts' own write.csv() calls land in <workdir>.
args <- commandArgs(trailingOnly = TRUE)
setwd(args[1])
pdf("plots.pdf")
getGraphicsEvent <- function(...) NULL
keyPressed <- "x"
options(width = 200)
source(args[2], echo = FALSE, print.eval = TRUE)  # print.eval: bare `C1` lines plot, as interactively
for (n in sort(grep("^interval_", ls(globalenv()), value = TRUE))) {
  v <- get(n, globalenv())
  cat(sprintf("%s %.12g %.12g\n", n, left(v), right(v)))
}
cat("R_VERSION", R.version.string, "\n")
