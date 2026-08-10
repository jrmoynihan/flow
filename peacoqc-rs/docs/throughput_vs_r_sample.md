# Sample: PeacoQC Rust vs R throughput

**Status:** checked-in sample from a release `compare_with_r` run (2026-08-10).

- Synthetic grid: events ∈ {50k, 200k, 1M} × FL channels ∈ {5, 15, 30}
- Real FCS: three anonymized stained samples (`real_01`…`real_03`, ~215k–394k events × 13 FL); source paths were passed only on the CLI and are not recorded here
- Warmup=1, reps=3; QC-core (load excluded)
- Machine: Apple M5 Max, macOS; PeacoQC 1.22.0 / flowCore 2.24.0 / peacoqc-rs 0.3.1 / rustc 1.95.0
- Rust CPU rows forced with `PEACOQC_FORCE_CPU=1`; GPU row used the same binary with `--features gpu`

## Headline (Rust Rayon vs R)

| Case | R mean (s) | Rust 1-thread (s) | Rust Rayon (s) | GPU (s) | Speedup vs R (Rayon) |
|------|------------|-------------------|----------------|---------|----------------------|
| real_01 (215k×13) | 1.53 | 0.222 | 0.103 | 12.63 | **14.9×** |
| real_02 (263k×13) | 1.40 | 0.218 | 0.091 | 8.94 | **15.3×** |
| real_03 (394k×13) | 1.78 | 0.275 | 0.114 | 10.88 | **15.7×** |
| synth 50k×15 | 0.91 | 0.134 | 0.101 | 6.41 | **9.1×** |
| synth 200k×15 | 2.27 | 0.312 | 0.214 | 11.71 | **10.6×** |
| synth 1M×15 | 3.83 | 0.400 | 0.186 | 13.91 | **20.6×** |
| synth 1M×30 | 7.32 | 0.904 | 0.399 | 29.76 | **18.3×** |

**GPU note:** On this machine and size range, the GPU PeacoQC path was **slower** than CPU (often ~50–100× behind Rayon). Treat GPU as optional / workload-dependent, not the vs-R headline.

## Full matrix

# PeacoQC Rust vs R throughput report

- Date (UTC): 2026-08-10T16:19:01Z
- CPU: Apple M5 Max
- OS: macos
- Warmup / reps: 1 / 3
- Modes: rust_only=false, gpu_requested=true
- Headline phase: `qc_core` (PeacoQC only; load excluded)

## Case `real_01` (215481 events × 13 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 1.5337 | 0.0270 | 140501 | 13.69 | 1.00× | 0.14× |
| rust-cpu-1 | 0.2220 | 0.0014 | 970775 | 15.78 | 6.91× | 1.00× |
| rust-cpu | 0.1032 | 0.0028 | 2087741 | 15.78 | 14.86× | 2.15× |
| rust-gpu | 12.6326 | 0.1382 | 17058 | 15.78 | 0.12× | 0.02× |

## Case `real_02` (263319 events × 13 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 1.3967 | 0.0086 | 188534 | 1.71 | 1.00× | 0.16× |
| rust-cpu-1 | 0.2178 | 0.0011 | 1208931 | 1.42 | 6.41× | 1.00× |
| rust-cpu | 0.0912 | 0.0009 | 2887354 | 1.42 | 15.31× | 2.39× |
| rust-gpu | 8.9375 | 0.6111 | 29462 | 40.16 | 0.16× | 0.02× |

## Case `real_03` (393849 events × 13 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 1.7827 | 0.0878 | 220932 | 10.92 | 1.00× | 0.15× |
| rust-cpu-1 | 0.2753 | 0.0014 | 1430483 | 10.63 | 6.47× | 1.00× |
| rust-cpu | 0.1137 | 0.0058 | 3464555 | 10.63 | 15.68× | 2.42× |
| rust-gpu | 10.8773 | 1.2825 | 36208 | 1.27 | 0.16× | 0.03× |

## Case `synth_1000000_x15` (1000000 events × 15 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 3.8290 | 0.1973 | 261165 | 28.68 | 1.00× | 0.10× |
| rust-cpu-1 | 0.3999 | 0.0032 | 2500380 | 0.00 | 9.57× | 1.00× |
| rust-cpu | 0.1864 | 0.0049 | 5365643 | 0.00 | 20.55× | 2.15× |
| rust-gpu | 13.9139 | 0.8908 | 71871 | 0.00 | 0.28× | 0.03× |

## Case `synth_1000000_x30` (1000000 events × 30 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 7.3187 | 0.0209 | 136637 | 60.40 | 1.00× | 0.12× |
| rust-cpu-1 | 0.9041 | 0.0465 | 1106033 | 30.02 | 8.09× | 1.00× |
| rust-cpu | 0.3993 | 0.0040 | 2504316 | 30.02 | 18.33× | 2.26× |
| rust-gpu | 29.7626 | 0.5969 | 33599 | 30.02 | 0.25× | 0.03× |

## Case `synth_1000000_x5` (1000000 events × 5 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 1.0357 | 0.0519 | 965562 | 0.00 | 1.00× | 0.13× |
| rust-cpu-1 | 0.1359 | 0.0030 | 7357574 | 0.00 | 7.62× | 1.00× |
| rust-cpu | 0.0618 | 0.0013 | 16170044 | 0.00 | 16.75× | 2.20× |
| rust-gpu | 4.5259 | 0.2087 | 220953 | 0.00 | 0.23× | 0.03× |

## Case `synth_200000_x15` (200000 events × 15 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 2.2727 | 0.0742 | 88002 | 60.25 | 1.00× | 0.14× |
| rust-cpu-1 | 0.3116 | 0.0012 | 641827 | 63.00 | 7.29× | 1.00× |
| rust-cpu | 0.2142 | 0.0036 | 933808 | 63.00 | 10.61× | 1.45× |
| rust-gpu | 11.7061 | 0.7439 | 17085 | 63.00 | 0.19× | 0.03× |

## Case `synth_200000_x30` (200000 events × 30 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 4.5573 | 0.0715 | 43885 | 60.25 | 1.00× | 0.10× |
| rust-cpu-1 | 0.4766 | 0.0037 | 419652 | 63.00 | 9.56× | 1.00× |
| rust-cpu | 0.3072 | 0.0040 | 651006 | 63.00 | 14.83× | 1.55× |
| rust-gpu | 26.0450 | 0.0964 | 7679 | 63.00 | 0.17× | 0.02× |

## Case `synth_200000_x5` (200000 events × 5 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 0.8063 | 0.0706 | 248036 | 0.00 | 1.00× | 0.13× |
| rust-cpu-1 | 0.1047 | 0.0013 | 1910774 | 63.00 | 7.70× | 1.00× |
| rust-cpu | 0.0735 | 0.0024 | 2722704 | 63.00 | 10.98× | 1.42× |
| rust-gpu | 4.0476 | 0.1503 | 49412 | 63.00 | 0.20× | 0.03× |

## Case `synth_50000_x15` (50000 events × 15 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 0.9103 | 0.0273 | 54925 | 57.00 | 1.00× | 0.15× |
| rust-cpu-1 | 0.1338 | 0.0004 | 373647 | 57.50 | 6.80× | 1.00× |
| rust-cpu | 0.1006 | 0.0018 | 496945 | 57.50 | 9.05× | 1.33× |
| rust-gpu | 6.4073 | 0.1006 | 7804 | 57.50 | 0.14× | 0.02× |

## Case `synth_50000_x30` (50000 events × 30 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 2.0833 | 0.1891 | 24000 | 57.00 | 1.00× | 0.11× |
| rust-cpu-1 | 0.2334 | 0.0018 | 214229 | 57.50 | 8.93× | 1.00× |
| rust-cpu | 0.1630 | 0.0019 | 306791 | 57.50 | 12.78× | 1.43× |
| rust-gpu | 13.1170 | 0.1977 | 3812 | 57.50 | 0.16× | 0.02× |

## Case `synth_50000_x5` (50000 events × 5 FL channels)

| Config | Mean (s) | Std (s) | Events/s | % removed | vs R | vs Rust-1 |
|---|---:|---:|---:|---:|---:|---:|
| r | 0.3207 | 0.0342 | 155925 | 57.00 | 1.00× | 0.12× |
| rust-cpu-1 | 0.0369 | 0.0002 | 1353331 | 56.00 | 8.68× | 1.00× |
| rust-cpu | 0.0252 | 0.0001 | 1980336 | 56.00 | 12.70× | 1.46× |
| rust-gpu | 2.1481 | 0.1770 | 23277 | 56.00 | 0.15× | 0.02× |

- R: R version 4.6.0 (2026-04-24)
- PeacoQC: 1.22.0
- flowCore: 2.24.0
- rustc: rustc 1.95.0 (59807616e 2026-04-14)
- peacoqc-rs: 0.3.1

See also [`comparison-with-r.md`](comparison-with-r.md) for fairness notes.
