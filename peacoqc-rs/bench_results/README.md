# PeacoQC GPU vs CPU (Criterion `gpu_bench`, 2026-07-24)

## Single-channel KDE (`KernelDensity::estimate`)

CPU ≈ GPU through 2M points (~3.5–45 ms). The GPU arm currently shares most of the
FFT path; do not use these numbers to set a KDE GPU threshold.

## Batched multi-channel KDE (where GPU helps)

| Setup | Batched GPU | Sequential CPU | Speedup |
|-------|-------------|----------------|---------|
| 5 ch × 50k | 15.0 ms | 18.2 ms | 1.2× |
| 5 ch × 500k | 16.0 ms | 51.0 ms | **3.2×** |
| 5 ch × 1M | 16.7 ms | 91.4 ms | **5.5×** |
| 10 ch × 1M | 35.4 ms | 189 ms | **5.3×** |

Batched GPU time stays nearly flat as events grow (amortized upload / kernel), while CPU scales roughly linearly.

Raw log: `kde_cpu_vs_gpu.txt`.
