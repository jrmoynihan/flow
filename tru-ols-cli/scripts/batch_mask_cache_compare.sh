#!/usr/bin/env bash
# Compare wall time for stained-directory batch with shared vs per-file mask-factor cache.
# Requires: release `tru-ols` on PATH or set TRU_OLS_BIN.
#
# Environment:
#   TRU_OLS_BATCH_SHARED_FACTOR_CACHE=1 (default in code) — one cache Arc across stained files
#   TRU_OLS_BATCH_SHARED_FACTOR_CACHE=0 — fresh 512-entry cache per stained file
#   OMP_NUM_THREADS, RAYON_NUM_THREADS — set for fair BLAS/Rayon (see AGENTS.md)
#
# Usage:
#   ./batch_mask_cache_compare.sh /path/to/reference_group /path/to/stained_dir /path/to/output_parent

set -euo pipefail

REF="${1:?reference group dir}"
STAINED="${2:?stained samples dir or glob parent}"
OUT_PARENT="${3:?output parent directory}"

BIN="${TRU_OLS_BIN:-tru-ols}"
if [[ ! -x "$BIN" ]] && ! command -v "$BIN" >/dev/null 2>&1; then
  echo "Set TRU_OLS_BIN to the tru-ols executable (e.g. target/release/tru-ols)" >&2
  exit 1
fi

STAMP="$(date +%Y%m%d_%H%M%S)"
RUN_DIR="${OUT_PARENT}/mask_cache_bench_${STAMP}"
mkdir -p "${RUN_DIR}/shared_cache" "${RUN_DIR}/fresh_cache_per_file"

export OMP_NUM_THREADS="${OMP_NUM_THREADS:-1}"

{
  echo "=== shared mask-factor cache (TRU_OLS_BATCH_SHARED_FACTOR_CACHE=1) ==="
  /usr/bin/time -p env TRU_OLS_BATCH_SHARED_FACTOR_CACHE=1 "$BIN" unmix \
    --stained "$STAINED" -c "$REF" --output "${RUN_DIR}/shared_cache"
} 2>&1 | tee "${RUN_DIR}/run_shared_cache.log"

{
  echo "=== fresh mask cache per stained file (TRU_OLS_BATCH_SHARED_FACTOR_CACHE=0) ==="
  /usr/bin/time -p env TRU_OLS_BATCH_SHARED_FACTOR_CACHE=0 "$BIN" unmix \
    --stained "$STAINED" -c "$REF" --output "${RUN_DIR}/fresh_cache_per_file"
} 2>&1 | tee "${RUN_DIR}/run_fresh_cache_per_file.log"

echo "Done. Logs: ${RUN_DIR}/run_shared_cache.log ${RUN_DIR}/run_fresh_cache_per_file.log"
