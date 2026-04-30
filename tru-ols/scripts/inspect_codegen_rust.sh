#!/usr/bin/env bash
# Dump assembly (or LLVM IR) for `solve_linear_system` in flow-tru-ols under two feature sets:
#   1) faer only  — default linear algebra (no ndarray/OpenBLAS)
#   2) blas       — ndarray-linalg + system OpenBLAS (requires pkg-config openblas)
#
# Prerequisites:
#   cargo install cargo-show-asm   # provides `cargo-asm` / `cargo asm`
#
# Usage (from workspace root):
#   bash tru-ols/scripts/inspect_codegen_rust.sh
#   bash tru-ols/scripts/inspect_codegen_rust.sh 2>&1 | tee rust_codegen.txt
#
# Environment:
#   RUSTFLAGS   — defaults to -C target-cpu=native (override to compare generic vs native CPU)
#   EXTRA_ASM   — extra args passed to `cargo asm` (e.g. --llvm for LLVM IR)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
cd "${WORKSPACE_ROOT}"

export RUSTFLAGS="${RUSTFLAGS:--C target-cpu=native}"

EXTRA_ASM="${EXTRA_ASM:-}"

if ! command -v cargo-asm >/dev/null 2>&1; then
  echo "error: cargo-asm not found. Install with:" >&2
  echo "  cargo install cargo-show-asm" >&2
  exit 1
fi

run_asm() {
  local label="$1"
  shift
  echo ""
  echo "================================================================================"
  echo "${label}"
  echo "================================================================================"
  # shellcheck disable=SC2086
  cargo asm -p flow-tru-ols -q --release --no-default-features \
    --features "$1" --lib --simplify --no-color ${EXTRA_ASM} solve_linear_system
}

echo "RUSTFLAGS=${RUSTFLAGS}"
echo "Workspace: ${WORKSPACE_ROOT}"

run_asm "flow-tru-ols — features: flow-fcs (faer QR/LU path)" "flow-fcs" || {
  echo "error: cargo asm failed for flow-fcs build" >&2
  exit 1
}

echo ""
echo "--------------------------------------------------------------------------------"
echo "flow-tru-ols — features: flow-fcs,blas (OpenBLAS / ndarray-linalg path)"
echo "If this fails, install OpenBLAS and PKG_CONFIG_PATH (see tru-ols/docs/julia-and-blas-on-macos.md)."
echo "--------------------------------------------------------------------------------"

if run_asm "flow-tru-ols — features: flow-fcs,blas" "flow-fcs,blas"; then
  :
else
  echo ""
  echo "note: blas build or disassembly failed; skipping is OK if you only need the faer path."
fi

echo ""
echo "================================================================================"
echo "Done"
echo "================================================================================"
echo "Interpretation:"
echo "  - faer path: large asm blocks referencing faer / Qr / PartialPivLu / dyn_stack are expected."
echo "  - blas path: look for calls into OpenBLAS (e.g. dgesvd, dgels) or PLT stubs to libopenblas."
echo ""
echo "Fallback without cargo-show-asm — emit bitcode + asm into target/ (mangled names):"
echo "  cargo rustc -p flow-tru-ols --release --no-default-features --features flow-fcs --lib -- \\"
echo "    -C opt-level=3 --emit=llvm-ir --emit=asm"
echo "Artifacts appear under target/<profile>/deps/ as flow_tru_ols-*.ll and *.s"
