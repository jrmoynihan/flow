#!/usr/bin/env Rscript
# Sidecar for examples/compare_with_r.rs. Times AutoSpectralRcpp joint unmix
# on in-memory exprs (qc_core) or read.FCS + unmix (e2e). Exits 0 with
# skipped=true JSON when packages are missing or the joint call fails.

args <- commandArgs(trailingOnly = TRUE)
get_arg <- function(flag, default = NULL) {
  i <- match(flag, args)
  if (is.na(i) || i >= length(args)) default else args[[i + 1]]
}

case_dir <- get_arg("--case-dir")
out_json <- get_arg("--out-json")
phase <- get_arg("--phase", "qc_core")
warmup <- as.integer(get_arg("--warmup", "1"))
reps <- as.integer(get_arg("--reps", "3"))
write_unmixed <- identical(get_arg("--write-unmixed", "0"), "1")
threads <- as.integer(get_arg("--threads", "1"))
if (is.na(threads) || threads < 1L) threads <- 1L
# Event-level OpenMP; keep BLAS single-threaded so the two pools do not nest.
Sys.setenv(
  OMP_NUM_THREADS = as.character(threads),
  OPENBLAS_NUM_THREADS = "1",
  MKL_NUM_THREADS = "1",
  VECLIB_MAXIMUM_THREADS = "1"
)

json_escape <- function(s) {
  s <- gsub("\\", "\\\\", s, fixed = TRUE)
  s <- gsub("\"", "'", s)
  s <- gsub("[\r\n\t]+", " ", s)
  s
}

write_skip <- function(reason) {
  if (is.null(out_json)) {
    cat(json_escape(reason), "\n", file = stderr())
    quit(save = "no", status = 1)
  }
  cat(
    sprintf('{"skipped":true,"reason":"%s"}\n', json_escape(reason)),
    file = out_json
  )
  quit(save = "no", status = 0)
}

if (is.null(case_dir) || is.null(out_json)) {
  write_skip("missing --case-dir or --out-json")
}

load_pkg <- function(pkg) {
  err <- tryCatch(
    {
      loadNamespace(pkg)
      NULL
    },
    error = function(e) conditionMessage(e)
  )
  if (!is.null(err)) {
    write_skip(sprintf("%s load failed: %s", pkg, err))
  }
}

load_pkg("AutoSpectralRcpp")
load_pkg("flowCore")

suppressPackageStartupMessages({
  library(AutoSpectralRcpp)
  library(flowCore)
})

read_named_mat <- function(path) {
  df <- read.csv(path, check.names = FALSE)
  rn <- df[[1]]
  m <- as.matrix(df[, -1, drop = FALSE])
  storage.mode(m) <- "double"
  rownames(m) <- as.character(rn)
  m
}

pkg_ver <- function(p) {
  if (requireNamespace(p, quietly = TRUE)) {
    as.character(utils::packageVersion(p))
  } else {
    NA_character_
  }
}

spectra <- read_named_mat(file.path(case_dir, "spectra.csv"))
af <- read_named_mat(file.path(case_dir, "af.csv"))
thr_df <- read.csv(file.path(case_dir, "thresholds.csv"), stringsAsFactors = FALSE)
thresholds <- thr_df$threshold
names(thresholds) <- thr_df$fluor

var_dir <- file.path(case_dir, "variants")
variants_list <- list()
if (dir.exists(var_dir)) {
  for (fn in list.files(var_dir, pattern = "\\.csv$", full.names = TRUE)) {
    nm <- sub("\\.csv$", "", basename(fn))
    m <- read_named_mat(fn)
    if (ncol(m) == ncol(spectra)) {
      colnames(m) <- colnames(spectra)
    }
    variants_list[[nm]] <- m
  }
}

spectra_variants <- NULL
if (length(variants_list) > 0) {
  # Let the wrapper compute deltas/norms from variants vs master spectra.
  spectra_variants <- list(
    thresholds = thresholds,
    variants = variants_list
  )
}

load_exprs <- function() {
  fcs_path <- file.path(case_dir, "prepared.fcs")
  if (file.exists(fcs_path)) {
    ff <- flowCore::read.FCS(fcs_path, transformation = FALSE, truncate_max_range = FALSE)
    ex <- flowCore::exprs(ff)
    fl <- grep("^FL[0-9]+-A$", colnames(ex), value = TRUE)
    if (length(fl) == 0) {
      fl <- colnames(ex)[colnames(ex) %in% colnames(spectra)]
    }
    if (length(fl) == 0) {
      stop("no fluorescence columns in prepared.fcs")
    }
    return(ex[, fl, drop = FALSE])
  }
  as.matrix(read.csv(file.path(case_dir, "events.csv"), check.names = FALSE))
}

align_detectors <- function(exprs) {
  if (identical(colnames(exprs), colnames(spectra))) {
    return(exprs)
  }
  if (all(colnames(spectra) %in% colnames(exprs)) && ncol(exprs) == ncol(spectra)) {
    return(exprs[, colnames(spectra), drop = FALSE])
  }
  if (ncol(exprs) == ncol(spectra)) {
    colnames(exprs) <- colnames(spectra)
    return(exprs)
  }
  stop(sprintf(
    "column mismatch: exprs=[%s] spectra=[%s]",
    paste(colnames(exprs), collapse = ","),
    paste(colnames(spectra), collapse = ",")
  ))
}

unmix_joint <- function(exprs) {
  exprs <- align_detectors(exprs)
  AutoSpectralRcpp::unmix.autospectral.rcpp(
    exprs,
    spectra,
    af.spectra = af,
    spectra.variants = spectra_variants,
    verbose = FALSE,
    parallel = threads > 1L,
    threads = as.integer(threads),
    pipeline = "joint",
    n.passes = 1L,
    n.af.passes = 1L
  )
}

tryCatch(
  {
    exprs <- load_exprs()
    n <- nrow(exprs)
    d <- ncol(exprs)
    last <- NULL
    if (identical(phase, "e2e")) {
      timed <- function() {
        ff <- flowCore::read.FCS(
          file.path(case_dir, "prepared.fcs"),
          transformation = FALSE,
          truncate_max_range = FALSE
        )
        ex <- flowCore::exprs(ff)
        fl <- grep("^FL[0-9]+-A$", colnames(ex), value = TRUE)
        last <<- unmix_joint(ex[, fl, drop = FALSE])
        last
      }
    } else {
      timed <- function() {
        last <<- unmix_joint(exprs)
        last
      }
    }
    if (warmup > 0) for (i in seq_len(warmup)) invisible(timed())
    times <- numeric(reps)
    for (i in seq_len(reps)) {
      # Sys.time() is sub-ms; proc.time()[["elapsed"]] often grains at 1 ms,
      # which makes 10k OpenMP runs print as a flat 0.003s.
      t0 <- Sys.time()
      invisible(timed())
      times[i] <- as.numeric(difftime(Sys.time(), t0, units = "secs"))
    }
    if (write_unmixed && !is.null(last)) {
      write.csv(last, file.path(case_dir, "unmixed_r.csv"), row.names = FALSE)
    }
    mean_s <- mean(times)
    std_s <- stats::sd(times)
    if (is.na(std_s)) std_s <- 0
    n_var <- if (length(variants_list) == 0) 0 else mean(vapply(variants_list, nrow, 1))
    as_ver <- pkg_ver("AutoSpectral")
    as_json <- if (is.na(as_ver)) "null" else sprintf('"%s"', as_ver)
    payload <- sprintf(
      '{"skipped":false,"mean_s":%f,"std_s":%f,"events":%d,"detectors":%d,"fluors":%d,"k_af":%d,"n_variants_mean":%f,"events_per_s":%f,"threads":%d,"r_version":"%s","autospectralrcpp_version":"%s","autospectral_version":%s}',
      mean_s, std_s, n, d, nrow(spectra), nrow(af), n_var,
      if (mean_s > 0) n / mean_s else 0,
      threads,
      json_escape(paste(R.version$major, R.version$minor, sep = ".")),
      json_escape(pkg_ver("AutoSpectralRcpp")),
      as_json
    )
    cat(payload, file = out_json)
  },
  error = function(e) write_skip(conditionMessage(e))
)
