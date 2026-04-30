#!/usr/bin/env julia
#
# Print BLAS configuration and dump Julia codegen for:
#   1) a small pure-Julia loop (LLVM can vectorize here)
#   2) a dense least-squares solve A \\ b (typically dispatches to OpenBLAS; expect calls, not vector loops)
#
# Usage (from workspace root):
#   julia tru-ols/scripts/inspect_codegen_julia.jl
#   julia tru-ols/scripts/inspect_codegen_julia.jl --which   # also print @which for dispatch
#
# Capture to a file:
#   julia tru-ols/scripts/inspect_codegen_julia.jl 2>&1 | tee julia_codegen.txt

using LinearAlgebra
using InteractiveUtils
using Random

function print_header(title::AbstractString)
    println()
    println("="^72)
    println(title)
    println("="^72)
end

# --- BLAS / runtime ---

print_header("BLAS configuration")
BLAS.set_num_threads(1)
println(BLAS.get_config())
println()

# --- Pure Julia: loop that LLVM may auto-vectorize ---

function simd_demo_sum(x::Float64)
    s = zero(Float64)
    @inbounds for i in 1:32
        s += x * Float64(i)
    end
    s
end

print_header("Pure-Julia loop: simd_demo_sum — @code_typed")
println(@code_typed debuginfo = :none simd_demo_sum(1.0))

print_header("Pure-Julia loop: simd_demo_sum — @code_llvm")
println(@code_llvm debuginfo = :none simd_demo_sum(1.0))

print_header("Pure-Julia loop: simd_demo_sum — @code_native")
println(@code_native debuginfo = :none simd_demo_sum(1.0))

# --- Least squares: same shape as typical small TRU-OLS inner solve (overdetermined) ---

function _parse_dim(env_key::AbstractString, default::Int)
    v = get(ENV, env_key, nothing)
    v === nothing && return default
    try
        return max(1, parse(Int, v))
    catch
        @warn "Invalid $env_key=$(repr(v)); using default $default"
        return default
    end
end

const M = _parse_dim("INSPECT_LS_ROWS", 10)
const N = _parse_dim("INSPECT_LS_COLS", 8)

function solve_ls(A::Matrix{Float64}, b::Vector{Float64})
    A \ b
end

Random.seed!(42)
A = randn(M, N)
b = randn(M)

print_header("Least squares solve_ls(A,b) with size ($M × $N) — @code_typed (expect call into BLAS/LAPACK)")
println(@code_typed debuginfo = :none solve_ls(A, b))

print_header("Least squares solve_ls(A,b) — @code_llvm")
println(@code_llvm debuginfo = :none solve_ls(A, b))

print_header("Least squares solve_ls(A,b) — @code_native (often a stub + call; SIMD is inside libopenblas)")
println(@code_native debuginfo = :none solve_ls(A, b))

if "--which" in ARGS
    print_header("Dispatch: @which (optional)")
    println(@which A \ b)
    println(@which solve_ls(A, b))
end

print_header("Done")
println("Tip: set INSPECT_LS_ROWS / INSPECT_LS_COLS to match a panel (overdetermined: rows > cols).")
println("Re-run with argument --which to print @which lines for the backslash operator.")
