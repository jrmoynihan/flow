# Sample: PeacoQC Rust vs R throughput

**Status:** checked-in sample from a release `compare_with_r` run (2026-08-10, Gaussian synthetic fixtures).

- Synthetic grid: `flow_fcs::synthetic` Gaussian mixtures + mild mid-run FL intensity drop; events ∈ {50k, 200k, 1M} × FL ∈ {5, 15, 30}
- Real FCS: three anonymized stained samples (`real_01`…`real_03`, ~215k–394k events × 13 FL); source paths CLI-only
- Warmup=1, reps=3; QC-core (load excluded); CPU only (no `--gpu`)
- Machine: Apple M5 Max, macOS; PeacoQC 1.22.0 / flowCore 2.24.0 / peacoqc-rs 0.3.2 / rustc 1.95.0

## Headline (Rust vs R, means)

| Case | R | Rust single-thread | Rust multi-threaded (`rayon`) | Speedup vs R |
| ---- | - | ------------------ | ----------------------------- | ------------ |
| real_01 (215k×13) | 1.55s | 0.225s | 0.109s | **14.2×** |
| real_02 (263k×13) | 1.36s | 0.222s | 0.093s | **14.5×** |
| real_03 (394k×13) | 1.61s | 0.274s | 0.107s | **15.1×** |
| synth 50k×15 | 0.83s | 0.087s | 0.044s | **18.9×** |
| synth 200k×15 | 1.63s | 0.223s | 0.098s | **16.7×** |
| synth 1M×15 | 2.98s | 0.581s | 0.182s | **16.4×** |
| synth 1M×30 | 5.57s | 1.156s | 0.359s | **15.5×** |

### Do not use GPU for full PeacoQC (this version)

Earlier publishable runs with `--gpu` showed the optional GPU QC-core path **much slower** than Rayon CPU on every size (often ~50–100× behind). **Recommendation:** leave the `gpu` feature off for end-to-end PeacoQC in 0.3.x. Batched KDE microbenches (`bench_results/`) can still win in isolation — see beads `flow-crates-aww` / `flow-crates-g1b`.

## Result agreement (R vs Rust, `% removed`)

Same QC-core runs; coarse metric only (not a substitute for dedicated R-parity tests).

### Real FCS (publishable agreement)

| Case | R removed | Rust removed | Δ |
| ---- | --------- | ------------ | - |
| real_01 (215k×13) | 13.69% | 15.78% | +2.09% |
| real_02 (263k×13) | 1.71% | 1.42% | −0.28% |
| real_03 (394k×13) | 10.92% | 10.63% | −0.29% |

### Synthetic grid (`flow_fcs::synthetic` + mild timed FL artifact)

| Case | R removed | Rust removed | Δ |
| ---- | --------- | ------------ | - |
| synth 50k×5 | 25.00% | 39.50% | +14.50% |
| synth 50k×15 | 26.00% | 77.00% | +51.00% |
| synth 50k×30 | 26.50% | 87.00% | +60.50% |
| synth 200k×5 | 20.50% | 20.50% | 0.00% |
| synth 200k×15 | 25.00% | 31.00% | +6.00% |
| synth 200k×30 | 25.50% | 47.00% | +21.50% |
| synth 1M×5 | 21.38% | 20.25% | −1.12% |
| synth 1M×15 | 24.52% | 24.30% | −0.22% |
| synth 1M×30 | 24.52% | 24.98% | +0.45% |

Large synthetic cases (1M) now track R closely (|Δ| ≲ 1.1%). Smaller grids can still diverge when the timed artifact sits near PeacoQC binning thresholds — prefer **real FCS** for publishable agreement; use unit/parity tests for fidelity.

- Date (UTC): 2026-08-10T18:01:05Z
- CPU: Apple M5 Max
- OS: macos
- Warmup / reps: 1 / 3
- Modes: rust_only=false, gpu_requested=false
- Headline phase: `qc_core` (PeacoQC only; load excluded)

## Case `real_01` (215481 events × 13 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 1.5483 | 0.0423 | 139170 | 13.69 | 1.00× | 0.15× |
| rust-cpu-1 | 0.2251 | 0.0009 | 957230 | 15.78 | 6.88× | 1.00× |
| rust-cpu | 0.1087 | 0.0046 | 1983057 | 15.78 | 14.25× | 2.07× |

## Case `real_02` (263319 events × 13 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 1.3560 | 0.0122 | 194188 | 1.71 | 1.00× | 0.16× |
| rust-cpu-1 | 0.2219 | 0.0015 | 1186731 | 1.42 | 6.11× | 1.00× |
| rust-cpu | 0.0934 | 0.0022 | 2817825 | 1.42 | 14.51× | 2.37× |

## Case `real_03` (393849 events × 13 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 1.6093 | 0.0471 | 244728 | 10.92 | 1.00× | 0.17× |
| rust-cpu-1 | 0.2738 | 0.0030 | 1438621 | 10.63 | 5.88× | 1.00× |
| rust-cpu | 0.1066 | 0.0029 | 3693652 | 10.63 | 15.09× | 2.57× |

## Case `synth_1000000_x15` (1000000 events × 15 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 2.9843 | 0.0734 | 335083 | 24.52 | 1.00× | 0.19× |
| rust-cpu-1 | 0.5813 | 0.0052 | 1720344 | 24.30 | 5.13× | 1.00× |
| rust-cpu | 0.1822 | 0.0007 | 5489247 | 24.30 | 16.38× | 3.19× |

## Case `synth_1000000_x30` (1000000 events × 30 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 5.5670 | 0.1336 | 179630 | 24.52 | 1.00× | 0.21× |
| rust-cpu-1 | 1.1562 | 0.0047 | 864883 | 24.98 | 4.81× | 1.00× |
| rust-cpu | 0.3588 | 0.0057 | 2786906 | 24.98 | 15.51× | 3.22× |

## Case `synth_1000000_x5` (1000000 events × 5 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 1.1657 | 0.0838 | 857878 | 21.38 | 1.00× | 0.18× |
| rust-cpu-1 | 0.2074 | 0.0045 | 4820959 | 20.25 | 5.62× | 1.00× |
| rust-cpu | 0.0792 | 0.0065 | 12630170 | 20.25 | 14.72× | 2.62× |

## Case `synth_200000_x15` (200000 events × 15 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 1.6313 | 0.0451 | 122599 | 25.00 | 1.00× | 0.14× |
| rust-cpu-1 | 0.2232 | 0.0037 | 896126 | 31.00 | 7.31× | 1.00× |
| rust-cpu | 0.0976 | 0.0003 | 2049098 | 31.00 | 16.71× | 2.29× |

## Case `synth_200000_x30` (200000 events × 30 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 3.3090 | 0.1267 | 60441 | 25.50 | 1.00× | 0.13× |
| rust-cpu-1 | 0.4345 | 0.0029 | 460326 | 47.00 | 7.62× | 1.00× |
| rust-cpu | 0.1968 | 0.0044 | 1016350 | 47.00 | 16.82× | 2.21× |

## Case `synth_200000_x5` (200000 events × 5 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 0.5933 | 0.0031 | 337079 | 20.50 | 1.00× | 0.13× |
| rust-cpu-1 | 0.0756 | 0.0005 | 2647143 | 20.50 | 7.85× | 1.00× |
| rust-cpu | 0.0354 | 0.0002 | 5642777 | 20.50 | 16.74× | 2.13× |

## Case `synth_50000_x15` (50000 events × 15 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 0.8323 | 0.0986 | 60072 | 26.00 | 1.00× | 0.10× |
| rust-cpu-1 | 0.0872 | 0.0002 | 573659 | 77.00 | 9.55× | 1.00× |
| rust-cpu | 0.0440 | 0.0003 | 1135111 | 77.00 | 18.90× | 1.98× |

## Case `synth_50000_x30` (50000 events × 30 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 1.4563 | 0.0298 | 34333 | 26.50 | 1.00× | 0.12× |
| rust-cpu-1 | 0.1697 | 0.0042 | 294629 | 87.00 | 8.58× | 1.00× |
| rust-cpu | 0.0753 | 0.0005 | 663833 | 87.00 | 19.34× | 2.25× |

## Case `synth_50000_x5` (50000 events × 5 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 0.2650 | 0.0320 | 188679 | 25.00 | 1.00× | 0.11× |
| rust-cpu-1 | 0.0299 | 0.0003 | 1671427 | 39.50 | 8.86× | 1.00× |
| rust-cpu | 0.0156 | 0.0001 | 3212588 | 39.50 | 17.03× | 1.92× |

- R: R version 4.6.0 (2026-04-24)
- PeacoQC: 1.22.0
- flowCore: 2.24.0
- rustc: rustc 1.95.0 (59807616e 2026-04-14)
- peacoqc-rs: 0.3.2

See also `docs/comparison-with-r.md` for fairness notes.
