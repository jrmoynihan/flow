#!/usr/bin/env Rscript
# PeacoQC QC-core timing companion for compare_with_r.rs
#
# Args (flag style):
#   --case-dir <dir>   directory containing prepared.fcs
#   --warmup <n>
#   --reps <n>
#   --channels a,b,c   fluorescence channels (optional; default FL{n}-A)
#   --out-json <path>  output JSON path (default: case-dir/throughput_r.json)
#   --phase qc_core|e2e  (default qc_core; e2e includes read.FCS in the timer)

parse_args <- function(argv) {
  out <- list(
    case_dir = NULL,
    warmup = 1L,
    reps = 5L,
    channels = NULL,
    out_json = NULL,
    phase = "qc_core"
  )
  i <- 1L
  while (i <= length(argv)) {
    flag <- argv[[i]]
    if (flag == "--case-dir") {
      i <- i + 1L
      out$case_dir <- argv[[i]]
    } else if (flag == "--warmup") {
      i <- i + 1L
      out$warmup <- as.integer(argv[[i]])
    } else if (flag == "--reps") {
      i <- i + 1L
      out$reps <- as.integer(argv[[i]])
    } else if (flag == "--channels") {
      i <- i + 1L
      out$channels <- strsplit(argv[[i]], ",", fixed = TRUE)[[1]]
    } else if (flag == "--out-json") {
      i <- i + 1L
      out$out_json <- argv[[i]]
    } else if (flag == "--phase") {
      i <- i + 1L
      out$phase <- argv[[i]]
    } else if (flag == "--include-margins-doublets") {
      # Accepted for CLI symmetry; phase name also encodes this mode.
      NULL
    } else {
      stop(sprintf("unknown argument: %s", flag), call. = FALSE)
    }
    i <- i + 1L
  }
  if (is.null(out$case_dir)) {
    stop("--case-dir is required", call. = FALSE)
  }
  if (is.na(out$reps) || out$reps < 1L) {
    stop("--reps must be >= 1", call. = FALSE)
  }
  if (is.na(out$warmup) || out$warmup < 0L) {
    stop("--warmup must be >= 0", call. = FALSE)
  }
  if (is.null(out$out_json)) {
    out$out_json <- file.path(out$case_dir, "throughput_r.json")
  }
  out
}

json_escape <- function(x) {
  x <- gsub("\\", "\\\\", x, fixed = TRUE)
  x <- gsub("\"", "\\\"", x, fixed = TRUE)
  x <- gsub("\n", "\\n", x, fixed = TRUE)
  x
}

write_throughput_json <- function(path, payload) {
  lines <- c(
    "{",
    sprintf('  "config": "%s",', json_escape(payload$config)),
    sprintf('  "case_id": "%s",', json_escape(payload$case_id)),
    sprintf('  "phase": "%s",', json_escape(payload$phase)),
    sprintf('  "mean_s": %.10g,', payload$mean_s),
    sprintf('  "std_s": %.10g,', payload$std_s),
    sprintf('  "events": %d,', as.integer(payload$events)),
    sprintf('  "channels": %d,', as.integer(payload$channels)),
    sprintf('  "events_per_s": %.10g,', payload$events_per_s),
    sprintf('  "pct_removed": %.10g,', payload$pct_removed),
    sprintf('  "reps": %d,', as.integer(payload$reps)),
    sprintf('  "r_version": "%s",', json_escape(payload$r_version)),
    sprintf('  "peacoqc_version": "%s",', json_escape(payload$peacoqc_version)),
    sprintf('  "flowcore_version": "%s",', json_escape(payload$flowcore_version)),
    '  "skipped": false',
    "}"
  )
  writeLines(lines, path, useBytes = TRUE)
}

run_peacoqc_once <- function(ff, channels, outdir, include_margins_doublets) {
  working <- ff
  if (isTRUE(include_margins_doublets)) {
    working <- PeacoQC::RemoveMargins(working, channels = c("FSC-A", "SSC-A"))
    working <- PeacoQC::RemoveDoublets(working)
  }
  PeacoQC::PeacoQC(
    working,
    channels = channels,
    determine_good_cells = "all",
    plot = FALSE,
    save_fcs = FALSE,
    report = FALSE,
    output_directory = outdir,
    name_directory = "PeacoQC_timing",
    MAD = 6,
    IT_limit = 0.6,
    consecutive_bins = 5,
    remove_zeros = FALSE
  )
}

main <- function() {
  if (!requireNamespace("flowCore", quietly = TRUE)) {
    message("flowCore is not installed; install via Bioconductor")
    quit(status = 2)
  }
  if (!requireNamespace("PeacoQC", quietly = TRUE)) {
    message("PeacoQC is not installed; install via Bioconductor (BiocManager::install('PeacoQC'))")
    quit(status = 2)
  }
  suppressPackageStartupMessages({
    library(flowCore)
    library(PeacoQC)
  })

  args <- parse_args(commandArgs(trailingOnly = TRUE))
  prepared <- file.path(args$case_dir, "prepared.fcs")
  if (!file.exists(prepared)) {
    stop(sprintf("prepared FCS not found: %s", prepared), call. = FALSE)
  }

  case_id <- basename(normalizePath(args$case_dir, winslash = "/", mustWork = TRUE))
  outdir <- tempfile("peacoqc_r_timing_")
  dir.create(outdir, recursive = TRUE)

  # Load outside the QC-core timer unless phase is e2e.
  ff_preload <- NULL
  if (identical(args$phase, "qc_core")) {
    ff_preload <- read.FCS(prepared, transformation = FALSE, truncate_max_range = FALSE)
  }

  channels <- args$channels
  if (is.null(channels) || length(channels) == 0L) {
    probe <- if (!is.null(ff_preload)) {
      ff_preload
    } else {
      read.FCS(prepared, transformation = FALSE, truncate_max_range = FALSE)
    }
    channels <- grep("^FL[0-9]+-A$", colnames(probe), value = TRUE)
  }
  if (length(channels) == 0L) {
    stop("no FL{n}-A channels found for PeacoQC", call. = FALSE)
  }

  run_once <- function() {
    if (identical(args$phase, "e2e")) {
      ff <- read.FCS(prepared, transformation = FALSE, truncate_max_range = FALSE)
    } else {
      ff <- ff_preload
    }
    include_md <- identical(args$phase, "qc_core_margins_doublets")
    run_peacoqc_once(ff, channels, outdir, include_md)
  }

  if (args$warmup > 0L) {
    for (i in seq_len(args$warmup)) {
      invisible(run_once())
    }
  }

  times <- numeric(args$reps)
  last_pct <- 0
  n_events <- 0L
  for (i in seq_len(args$reps)) {
    t0 <- proc.time()[["elapsed"]]
    res <- run_once()
    times[[i]] <- proc.time()[["elapsed"]] - t0
    last_pct <- as.numeric(res$PercentageRemoved)
    n_events <- as.integer(nrow(res$FinalFF))
    if (is.na(n_events) || n_events < 1L) {
      # Fall back to GoodCells length / prepared $TOT when FinalFF missing.
      n_events <- length(res$GoodCells)
    }
  }

  mean_s <- mean(times)
  std_s <- if (length(times) > 1L) {
    sqrt(sum((times - mean_s)^2) / length(times))
  } else {
    0
  }

  # Prefer event count from the prepared file for throughput (QC-core).
  if (identical(args$phase, "qc_core") && !is.null(ff_preload)) {
    n_events <- as.integer(nrow(ff_preload))
  }

  payload <- list(
    config = "r",
    case_id = case_id,
    phase = args$phase,
    mean_s = mean_s,
    std_s = std_s,
    events = n_events,
    channels = length(channels),
    events_per_s = n_events / mean_s,
    pct_removed = last_pct,
    reps = args$reps,
    r_version = R.version.string,
    peacoqc_version = as.character(utils::packageVersion("PeacoQC")),
    flowcore_version = as.character(utils::packageVersion("flowCore"))
  )
  write_throughput_json(args$out_json, payload)
  message(sprintf("wrote %s (mean_s=%.4f, pct_removed=%.2f)", args$out_json, mean_s, last_pct))
}

tryCatch(
  main(),
  error = function(e) {
    message(conditionMessage(e))
    quit(status = 1)
  }
)
