#!/usr/bin/env Rscript
# SPDX-License-Identifier: GPL-3.0
#
# Generate the `puff` code-to-code reference fixture for `changi`.
#
# This harness drives the **upstream Hammerling Research Group `puff` R
# package** (MIT, commit 5213d58) over a branch-covering input grid and writes
# every reference value to tests/data/puff_reference.csv. The Rust regression
# test tests/puff_code_to_code.rs replays that fixture through the Rust port in
# src/puff/ and asserts agreement.
#
# Why an R harness lives in this repo
# -----------------------------------
# The workspace rule "No Python for documentation or accounting" scopes itself
# to documentation generation and repository accounting, and says nothing about
# other languages. This script is neither: it *executes the third-party upstream
# reference implementation* to produce V&V reference data, exactly as
# crates/changi/dev/flexpart_reference.f90 compiles and runs upstream FLEXPART,
# and as crates/boon-lay/dev/gen_triso_atops_reference.py runs upstream
# TRISO-ATOPS. Upstream is written in R, so the harness is in R; rewriting it in
# another language would mean re-deriving the reference rather than measuring
# it, which is the one thing a code-to-code harness must not do.
#
# Upstream clone (gitignored, reference-only, never compiled into the crate):
#     crates/changi/upstream_source/puff  @ 5213d58
#
#     git clone https://github.com/Hammerling-Research-Group/puff.git
#     git -C puff checkout 5213d58
#
# Usage:
#     Rscript dev/gen_puff_reference.R            # writes the CSV fixture
#     Rscript dev/gen_puff_reference.R --check    # regenerate + diff only
#
# Requires: R (>= 4.1) and dplyr. Upstream's `simulate_*` functions call
# dplyr::bind_rows; nothing else in the physics path needs a package, and
# ggplot2/plotly (upstream's other Imports) are only used by R/plots.R, which is
# deliberately not ported.

UPSTREAM_COMMIT <- "5213d58"

args_cli <- commandArgs(trailingOnly = TRUE)
check_only <- "--check" %in% args_cli

crate_root <- normalizePath(file.path(dirname(sub("^--file=", "", grep("^--file=", commandArgs(FALSE), value = TRUE)[1])), ".."))
upstream <- file.path(crate_root, "upstream_source", "puff")
fixture <- file.path(crate_root, "tests", "data", "puff_reference.csv")

if (!dir.exists(file.path(upstream, "R"))) {
  stop(sprintf(paste0(
    "upstream puff clone not found at %s\n",
    "  git clone https://github.com/Hammerling-Research-Group/puff.git %s\n",
    "  git -C %s checkout %s\n"), upstream, upstream, upstream, UPSTREAM_COMMIT))
}

suppressMessages(library(dplyr))
source(file.path(upstream, "R", "helpers.R"))
source(file.path(upstream, "R", "simulate_sensor_mode.R"))
source(file.path(upstream, "R", "simulate_grid_mode.R"))

# ── fixture accumulation ─────────────────────────────────────────────────────

ROWS <- new.env(parent = emptyenv())
ROWS$fn <- character(0)
ROWS$args <- character(0)
ROWS$expected <- character(0)

fmt <- function(x) {
  x <- as.numeric(x)
  if (is.na(x)) return("nan")
  if (is.infinite(x)) return(if (x > 0) "inf" else "-inf")
  sprintf("%.17g", x)
}

case <- function(fn, args, expected) {
  ROWS$fn <- c(ROWS$fn, fn)
  ROWS$args <- c(ROWS$args, paste(vapply(args, fmt, character(1)), collapse = ";"))
  ROWS$expected <- c(ROWS$expected, fmt(expected))
}

# Stability classes are encoded as 1..6 for A..F, 0 for "absent".
class_index <- function(letter) match(toupper(letter), LETTERS[1:6])

# ── input grids ──────────────────────────────────────────────────────────────

# Wind speeds straddling every band edge in get_stab_class (2, 3, 5, 6 m/s),
# from either side and exactly on it.
SPEEDS <- sort(unique(c(
  0, 0.5, 1.0, 1.999999, 2, 2.000001, 2.5, 2.999999, 3, 3.000001,
  4, 4.999999, 5, 5.000001, 5.5, 5.999999, 6, 6.000001, 10, 25, 100
)))

# Hours spanning the is_day boundaries (7 and 18, both inclusive).
HOURS <- 0:23

# Travel distances (metres) used by the gpuff sweep.
GPUFF_DISTANCES_M <- c(0, 1, 10, 100, 500, 2000, 1e4)

# Distances in km straddling every cutoff in every class's sigma_z table, plus
# the unfitted long range where sigma_z saturates.
DISTANCES_KM <- sort(unique(c(
  1e-6, 1e-4, 0.01, 0.05,
  0.1, 0.100001, 0.15, 0.2, 0.25, 0.3, 0.4, 0.5, 0.7, 1, 2, 3, 4, 7, 10,
  15, 20, 30, 40, 60, 100, 500, 1000, 1e4, 1e5,
  0.0999999, 0.1999999, 0.2999999, 0.9999999, 2.9999999, 9.9999999,
  # Every travel distance the gpuff sweep below actually uses, converted from
  # metres to km. Without these the sigma comparison would not cover the
  # inputs gpuff feeds it -- which is how a 2.3e-12 gpuff residual at
  # total_dist = 1 m went unexplained on the first run.
  GPUFF_DISTANCES_M / 1000
)))

# ── groups ───────────────────────────────────────────────────────────────────

gen_is_day <- function() {
  for (h in HOURS) case("is_day", c(h), as.numeric(is_day(h)))
}

gen_stab_class <- function() {
  # Upstream's NA/NULL path: neutral D.
  case("get_stab_class.count", c(-1, 12), 1)          # -1 flags "missing U"
  case("get_stab_class.class1", c(-1, 12), class_index("D"))
  case("get_stab_class.class2", c(-1, 12), 0)
  for (u in SPEEDS) {
    for (h in HOURS) {
      sc <- get_stab_class(u, h)
      case("get_stab_class.count", c(u, h), length(sc))
      case("get_stab_class.class1", c(u, h), class_index(sc[1]))
      case("get_stab_class.class2", c(u, h),
           if (length(sc) > 1) class_index(sc[2]) else 0)
    }
  }
}

gen_sigmas <- function() {
  for (cl in LETTERS[1:6]) {
    for (d in DISTANCES_KM) {
      s <- compute_sigma_vals(cl, d)
      case("compute_sigma_vals.sigma_y", c(class_index(cl), d), s[1])
      case("compute_sigma_vals.sigma_z", c(class_index(cl), d), s[2])
    }
  }
  # Non-positive distances: upstream returns NA for both.
  for (cl in LETTERS[1:6]) {
    for (d in c(0, -1, -1e-9)) {
      s <- compute_sigma_vals(cl, d)
      case("compute_sigma_vals.sigma_y", c(class_index(cl), d), s[1])
      case("compute_sigma_vals.sigma_z", c(class_index(cl), d), s[2])
    }
  }
}

gen_wind_convert <- function() {
  dirs <- sort(unique(c(seq(0, 360, by = 15), 1, 44.5, 89.9, 135.25, 271.5, 359.99)))
  for (sp in c(0, 0.1, 1, 2.5, 5, 7.5, 10, 33.3)) {
    for (d in dirs) {
      w <- wind_vector_convert(sp, d)
      case("wind_vector_convert.u", c(sp, d), w$u)
      case("wind_vector_convert.v", c(sp, d), w$v)
    }
  }
}

gen_wind_interp <- function() {
  start <- as.POSIXct("2024-01-01 00:00:00", tz = "UTC")
  scenarios <- list(
    list(sp = c(2, 3), dir = c(90, 180), dur = 3600, dt = 600),
    list(sp = c(1, 5, 2), dir = c(0, 90, 270), dur = 1800, dt = 300),
    list(sp = c(4, 4, 4, 4), dir = c(10, 20, 30, 40), dur = 900, dt = 100),
    list(sp = c(0, 10), dir = c(350, 10), dur = 1200, dt = 240)
  )
  for (i in seq_along(scenarios)) {
    s <- scenarios[[i]]
    out <- interpolate_wind_data(s$sp, s$dir, start, start + s$dur, s$dt)
    n <- length(out$wind_u)
    for (k in seq_len(n)) {
      prefix <- c(length(s$sp), s$sp, s$dir, s$dur, s$dt, k - 1)
      case("interpolate_wind_data.u", prefix, out$wind_u[k])
      case("interpolate_wind_data.v", prefix, out$wind_v[k])
    }
  }
}

gen_gpuff <- function() {
  # Receptors at, near and far from the puff centre; on the ground, at source
  # height, and above; and one below-ground z to exercise the image term.
  receptors <- list(
    c(0, 0, 0), c(0, 0, 2), c(10, 0, 2), c(0, 10, 2), c(-10, -10, 2),
    c(100, 0, 2), c(100, 50, 10), c(1000, 0, 2), c(50, 50, 0), c(50, 50, 100)
  )
  for (cl in LETTERS[1:6]) {
    for (q in c(1e-6, 1e-3, 1, 9.7222e-3)) {
      for (dist in GPUFF_DISTANCES_M) {
        for (h in c(0, 2, 2.5, 30)) {
          for (r in receptors) {
            # Puff centre placed downwind along +x by its travel distance, the
            # geometry the simulate drivers actually produce.
            val <- gpuff(Q = q, stab_class = cl,
                         x_p = dist, y_p = 0,
                         x_r_vec = r[1], y_r_vec = r[2], z_r_vec = r[3],
                         total_dist = dist, H = h, U = 5)
            case("gpuff", c(class_index(cl), q, dist, h, r[1], r[2], r[3]), val)
          }
        }
      }
    }
  }
  # gpuff called with an AMBIGUOUS class vector, as get_stab_class returns.
  # Upstream silently uses only the first; pinned so the port's `primary()`
  # is verified rather than assumed.
  for (pair in list(c("A", "B"), c("E", "F"), c("B", "C"), c("D", "E"), c("C", "D"))) {
    val <- gpuff(Q = 1, stab_class = pair, x_p = 100, y_p = 0,
                 x_r_vec = 120, y_r_vec = 10, z_r_vec = 2,
                 total_dist = 100, H = 2, U = 5)
    case("gpuff.ambiguous_uses_first",
         c(class_index(pair[1]), class_index(pair[2])), val)
  }
  # gpuff's U argument is never read in the body. Demonstrated rather than
  # asserted from inspection: sweep U and show the answer never moves.
  for (u in c(0, 1e-9, 1, 5, 100, 1e6, -7)) {
    val <- gpuff(Q = 1, stab_class = "C", x_p = 100, y_p = 0,
                 x_r_vec = 150, y_r_vec = 20, z_r_vec = 2,
                 total_dist = 100, H = 2, U = u)
    case("gpuff.u_is_inert", c(u), val)
  }
}

# Scenarios shared by both simulate modes, so the two can be compared on
# identical input.
SIM_SCENARIOS <- list(
  list(id = 0, hour = 12, u = 2.0, v = 1.0, rate = 3.5, sim_dt = 10, puff_dt = 10,
       output_dt = 120, dur = 600, puff_duration = 1200, src = c(0, 0, 2.5)),
  list(id = 1, hour = 12, u = 1.0, v = 0.5, rate = 3.5, sim_dt = 10, puff_dt = 10,
       output_dt = 120, dur = 600, puff_duration = 1200, src = c(0, 0, 2.5)),
  list(id = 2, hour = 2,  u = 5.0, v = 0.0, rate = 10.0, sim_dt = 30, puff_dt = 60,
       output_dt = 300, dur = 1800, puff_duration = 600, src = c(5, -5, 3.0)),
  list(id = 3, hour = 12, u = 6.0, v = 3.0, rate = 1.0, sim_dt = 20, puff_dt = 20,
       output_dt = 100, dur = 1000, puff_duration = 200, src = c(0, 0, 1.0))
)

sim_prefix <- function(s) {
  c(s$id, s$hour, s$u, s$v, s$rate, s$sim_dt, s$puff_dt, s$output_dt,
    s$dur, s$puff_duration, s$src)
}

gen_sensor_mode <- function() {
  sensors <- matrix(c(
    20, 10, 2.0,
    100, 50, 2.0,
    -30, 0, 2.0,
    0, 0, 2.0,
    250, -120, 5.0
  ), ncol = 3, byrow = TRUE)

  for (s in SIM_SCENARIOS) {
    start <- as.POSIXct(sprintf("2024-01-01 %02d:00:00", s$hour), tz = "UTC")
    n <- s$dur / s$sim_dt + 1
    wind_data <- data.frame(wind_u = rep(s$u, n), wind_v = rep(s$v, n))
    out <- simulate_sensor_mode(
      start_time = start, end_time = start + s$dur,
      source_coords = matrix(s$src, ncol = 3, byrow = TRUE),
      emission_rate = s$rate, wind_data = wind_data,
      sensor_coords = sensors,
      sim_dt = s$sim_dt, puff_dt = s$puff_dt, output_dt = s$output_dt,
      puff_duration = s$puff_duration)
    vals <- as.matrix(out[, -1, drop = FALSE])
    for (i in seq_len(nrow(vals))) {
      for (j in seq_len(ncol(vals))) {
        case("simulate_sensor_mode", c(sim_prefix(s), i - 1, j - 1), vals[i, j])
      }
    }
    case("simulate_sensor_mode.n_rows", sim_prefix(s), nrow(vals))
  }

  # Two identical sources, to pin the source superposition.
  s <- SIM_SCENARIOS[[1]]
  start <- as.POSIXct(sprintf("2024-01-01 %02d:00:00", s$hour), tz = "UTC")
  n <- s$dur / s$sim_dt + 1
  wind_data <- data.frame(wind_u = rep(s$u, n), wind_v = rep(s$v, n))
  out <- simulate_sensor_mode(
    start_time = start, end_time = start + s$dur,
    source_coords = matrix(c(s$src, s$src), ncol = 3, byrow = TRUE),
    emission_rate = s$rate, wind_data = wind_data,
    sensor_coords = sensors,
    sim_dt = s$sim_dt, puff_dt = s$puff_dt, output_dt = s$output_dt,
    puff_duration = s$puff_duration)
  vals <- as.matrix(out[, -1, drop = FALSE])
  for (i in seq_len(nrow(vals))) {
    for (j in seq_len(ncol(vals))) {
      case("simulate_sensor_mode.two_sources", c(sim_prefix(s), i - 1, j - 1), vals[i, j])
    }
  }
}

gen_grid_mode <- function() {
  grid_coords <- list(x = c(-20, 0, 20, 100), y = c(-10, 0, 10), z = c(2.0, 10.0))
  for (s in SIM_SCENARIOS) {
    start <- as.POSIXct(sprintf("2024-01-01 %02d:00:00", s$hour), tz = "UTC")
    n <- s$dur / s$sim_dt + 1
    wind_data <- data.frame(wind_u = rep(s$u, n), wind_v = rep(s$v, n))
    out <- simulate_grid_mode(
      start_time = start, end_time = start + s$dur,
      source_coords = matrix(s$src, ncol = 3, byrow = TRUE),
      emission_rate = s$rate, wind_data = wind_data,
      grid_coords = grid_coords,
      sim_dt = s$sim_dt, puff_dt = s$puff_dt, output_dt = s$output_dt,
      puff_duration = s$puff_duration)
    for (i in seq_len(nrow(out))) {
      for (j in seq_len(ncol(out))) {
        case("simulate_grid_mode", c(sim_prefix(s), i - 1, j - 1), out[i, j])
      }
    }
    case("simulate_grid_mode.n_rows", sim_prefix(s), nrow(out))
    case("simulate_grid_mode.n_cols", sim_prefix(s), ncol(out))
  }
}

# ── main ─────────────────────────────────────────────────────────────────────

gen_is_day()
gen_stab_class()
gen_sigmas()
gen_wind_convert()
gen_wind_interp()
gen_gpuff()
gen_sensor_mode()
gen_grid_mode()

# Collapse exact duplicates (same function + same args), keeping the first.
key <- paste(ROWS$fn, ROWS$args, sep = "|")
keep <- !duplicated(key)
dup <- sum(!keep)
fn <- ROWS$fn[keep]; ar <- ROWS$args[keep]; ex <- ROWS$expected[keep]

header <- c(
  "# puff code-to-code reference values, generated by dev/gen_puff_reference.R",
  sprintf("# upstream: Hammerling-Research-Group/puff @ %s (MIT)", UPSTREAM_COMMIT),
  "# DO NOT EDIT BY HAND -- regenerate with the script above.",
  "function,args,expected"
)
body <- paste(fn, ar, ex, sep = ",")
text <- paste(c(header, body, ""), collapse = "\n")

if (check_only) {
  if (!file.exists(fixture)) stop("fixture missing; run without --check first")
  old <- paste(readLines(fixture, warn = FALSE), collapse = "\n")
  if (!identical(sub("\n$", "", text), old)) {
    stop("fixture is OUT OF DATE -- rerun without --check")
  }
  cat("fixture up to date\n")
} else {
  dir.create(dirname(fixture), recursive = TRUE, showWarnings = FALSE)
  writeLines(sub("\n$", "", text), fixture)
  if (dup > 0) cat(sprintf("  (%d duplicate cases collapsed)\n", dup))
  cat(sprintf("wrote %s (%d cases)\n", fixture, length(fn)))
}
